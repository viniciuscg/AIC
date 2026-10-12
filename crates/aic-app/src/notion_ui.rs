//! Pontos de integração do Notion com a UI do board.
//!
//! Toda chamada de rede roda numa thread separada (`spawn`); o resultado volta por canal e é
//! aplicado em `tick`, a cada frame. Sem token configurado, nada aqui faz chamada de rede.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use egui::{RichText, TextEdit, Ui};
use aic_core::config::NotionStatusMap;
use aic_core::notion::{self, Client, Detected, NotionCard, SyncOutcome, TokenSource};
use aic_core::{Id, Task, TaskSource, TaskStatus};

use crate::app::{App, ToastKind, View};
use crate::ui::widgets;

/// Quanto tempo o segundo clique/Shift+P confirma a promoção.
const CONFIRM_WINDOW: Duration = Duration::from_secs(5);

enum Msg {
    Synced { manual: bool, result: Result<SyncOutcome, String> },
    Pushed { id: Id, label: String, result: Result<String, String> },
    Promoted { id: Id, result: Result<NotionCard, String> },
    Tested(Result<String, String>),
    Detected(Result<Detected, String>),
}

/// Estado do sync com o Notion (vive em `App::notion`).
pub struct NotionState {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    /// Pra acordar a UI quando uma thread termina.
    ctx: Option<egui::Context>,
    pub syncing: bool,
    last_sync: Option<Instant>,
    pub last_ok: Option<DateTime<Local>>,
    pub last_error: Option<String>,
    pub last_summary: Option<String>,
    on_board: bool,
    /// Token em cache (`None` = ainda não lido). Nunca vai pra log.
    token: Option<Option<(String, TokenSource)>>,
    token_input: String,
    /// Status enviados ao Notion e ainda sem resposta (o sync não sobrescreve).
    pushing: HashMap<Id, TaskStatus>,
    promoting: Vec<Id>,
    confirm_promote: Option<(Id, Instant)>,
    testing: bool,
    test_result: Option<Result<String, String>>,
    detecting: bool,
    detected: Option<Result<Detected, String>>,
    /// Buffers dos campos "status por coluna" (texto separado por vírgula).
    status_bufs: Option<[String; 5]>,
}

impl Default for NotionState {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            ctx: None,
            syncing: false,
            last_sync: None,
            last_ok: None,
            last_error: None,
            last_summary: None,
            on_board: false,
            token: None,
            token_input: String::new(),
            pushing: HashMap::new(),
            promoting: vec![],
            confirm_promote: None,
            testing: false,
            test_result: None,
            detecting: false,
            detected: None,
            status_bufs: None,
        }
    }
}

impl NotionState {
    fn token(&mut self) -> Option<&(String, TokenSource)> {
        if self.token.is_none() {
            self.token = Some(notion::load_token());
        }
        self.token.as_ref().and_then(|t| t.as_ref())
    }

    fn spawn(&self, f: impl FnOnce() -> Msg + Send + 'static) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let _ = std::thread::Builder::new().name("aic-notion".into()).spawn(move || {
            let _ = tx.send(f());
            if let Some(c) = ctx {
                c.request_repaint();
            }
        });
    }
}

/// Token, se o módulo estiver ligado e a database configurada.
fn ready(app: &mut App) -> Option<String> {
    if !app.cfg.on(aic_core::Module::Notion) || app.cfg.notion.tasks_database.as_deref().is_none_or(|s| s.trim().is_empty()) {
        return None;
    }
    app.notion.token().map(|(t, _)| t.clone())
}

fn task_label(t: &Task) -> String {
    t.card_key.clone().unwrap_or_else(|| {
        let mut s: String = t.title.chars().take(30).collect();
        if t.title.chars().count() > 30 {
            s.push('…');
        }
        s
    })
}

fn start_sync(app: &mut App, manual: bool) {
    let Some(token) = ready(app) else {
        if manual {
            app.toast("Notion não configurado (Config → Integrações)", ToastKind::Warn);
        }
        return;
    };
    if app.notion.syncing {
        return;
    }
    app.notion.syncing = true;
    app.notion.last_sync = Some(Instant::now());
    let cfg = app.cfg.notion.clone();
    app.notion.spawn(move || {
        let result = Client::new(token).and_then(|c| c.sync(&cfg)).map_err(|e| format!("{e:#}"));
        Msg::Synced { manual, result }
    });
}

fn apply_sync(app: &mut App, manual: bool, result: Result<SyncOutcome, String>) {
    app.notion.syncing = false;
    match result {
        Ok(out) => {
            let plan = notion::plan_upsert(&app.tasks, &out.cards);
            for mut t in plan.save {
                // Mudança local ainda a caminho do Notion: o local vence.
                if let Some(st) = app.notion.pushing.get(&t.id) {
                    t.status = *st;
                }
                app.save_task(t);
            }
            let sprint = out.sprint.as_ref().map(|(_, n)| format!(" · {n}")).unwrap_or_default();
            let summary = format!("{} cards{sprint} · +{} ~{} arquivados {}", out.cards.len(), plan.created, plan.updated, plan.archived);
            app.notion.last_ok = Some(Local::now());
            app.notion.last_error = None;
            app.notion.last_summary = Some(summary.clone());
            if manual || plan.created + plan.archived > 0 {
                app.toast(&format!("Notion sincronizado: {summary}"), ToastKind::Ok);
            }
            if manual {
                for n in out.notes {
                    app.toast(&format!("Notion: {n}"), ToastKind::Warn);
                }
            }
        }
        Err(e) => {
            app.toast(&format!("Notion: falha no sync — {e}"), ToastKind::Err);
            app.notion.last_error = Some(e);
        }
    }
}

/// Botões na barra do board (sincronizar etc.). O layout aqui é da direita pra esquerda.
pub fn toolbar(app: &mut App, ui: &mut Ui) {
    app.notion.ctx = Some(ui.ctx().clone());
    if !app.cfg.on(aic_core::Module::Notion) {
        return;
    }
    if ready(app).is_none() {
        if ui.button(format!("{}  Conectar Notion", widgets::I_NOTION)).on_hover_text("Config → Integrações").clicked() {
            app.set_view(View::Settings);
        }
        return;
    }
    let theme = app.theme.clone();
    let st = &app.notion;
    let hover = match (&st.last_error, &st.last_summary) {
        (Some(e), _) => format!("Último sync falhou: {e}"),
        (None, Some(s)) => format!("Último sync: {s}"),
        _ => "Ainda não sincronizado".into(),
    };
    let label = if st.syncing { format!("{}  Sincronizando…", widgets::I_RESTART) } else { format!("{}  Sincronizar", widgets::I_NOTION) };
    if ui.add_enabled(!st.syncing, egui::Button::new(label)).on_hover_text(hover).clicked() {
        start_sync(app, true);
    }
    if app.notion.syncing {
        ui.spinner();
    } else if app.notion.last_error.is_some() {
        ui.label(RichText::new("sync falhou").color(theme.err).small());
    } else if let Some(t) = app.notion.last_ok {
        ui.label(widgets::subtle(format!("sync {}", t.format("%H:%M")), &theme).small());
    }
}

/// Status mudou no AIC → refletir no card do Notion.
pub fn on_status_change(app: &mut App, id: Id, status: TaskStatus) {
    let Some(task) = app.tasks.iter().find(|t| t.id == id).cloned() else { return };
    let Some(page) = task.notion_page.clone().filter(|_| task.source == TaskSource::Notion) else { return };
    let Some(token) = ready(app) else {
        app.toast("Card do Notion mudou só aqui: integração não configurada", ToastKind::Warn);
        return;
    };
    app.notion.pushing.insert(id, status);
    let cfg = app.cfg.notion.clone();
    let label = task_label(&task);
    app.notion.spawn(move || {
        let result = Client::new(token).and_then(|c| c.update_status(&page, &cfg, status)).map_err(|e| format!("{e:#}"));
        Msg::Pushed { id, label, result }
    });
}

/// Cria um card no Notion a partir de uma tarefa local (pede confirmação: 2º clique/Shift+P).
pub fn promote(app: &mut App, id: Id) {
    let Some(task) = app.tasks.iter().find(|t| t.id == id).cloned() else { return };
    if task.notion_page.is_some() {
        app.toast("Essa tarefa já está no Notion", ToastKind::Info);
        return;
    }
    let Some(token) = ready(app) else {
        app.toast("Integração com o Notion ainda não configurada (Config → Integrações)", ToastKind::Warn);
        return;
    };
    if app.notion.promoting.contains(&id) {
        return;
    }
    let confirmed = app.notion.confirm_promote.is_some_and(|(cid, at)| cid == id && at.elapsed() < CONFIRM_WINDOW);
    if !confirmed {
        app.notion.confirm_promote = Some((id, Instant::now()));
        app.toast(&format!("Criar \"{}\" no Notion? Clique de novo (ou Shift+P) pra confirmar", task_label(&task)), ToastKind::Info);
        return;
    }
    app.notion.confirm_promote = None;
    app.notion.promoting.push(id);
    app.toast("Criando card no Notion…", ToastKind::Info);
    let cfg = app.cfg.notion.clone();
    app.notion.spawn(move || {
        let result = Client::new(token).and_then(|c| c.create_card(&cfg, &task)).map_err(|e| format!("{e:#}"));
        Msg::Promoted { id, result }
    });
}

/// Botões extras no editor de tarefa.
pub fn editor_buttons(app: &mut App, ui: &mut Ui, task: &Task) {
    app.notion.ctx = Some(ui.ctx().clone());
    if task.notion_page.is_none() {
        if app.notion.promoting.contains(&task.id) {
            ui.spinner();
            ui.label(widgets::subtle("criando no Notion…", &app.theme).small());
            return;
        }
        let confirming = app.notion.confirm_promote.is_some_and(|(id, at)| id == task.id && at.elapsed() < CONFIRM_WINDOW);
        let label = if confirming { "󰈙 Confirmar: criar no Notion" } else { "󰈙 Promover pro Notion" };
        if ui.button(label).on_hover_text("Shift+P").clicked() {
            promote(app, task.id);
        }
    } else if app.notion.pushing.contains_key(&task.id) {
        ui.spinner();
    }
}

/// Chamado a cada frame: aplica resultados das threads e dispara syncs.
pub fn tick(app: &mut App) {
    while let Ok(msg) = app.notion.rx.try_recv() {
        match msg {
            Msg::Synced { manual, result } => apply_sync(app, manual, result),
            Msg::Pushed { id, label, result } => {
                app.notion.pushing.remove(&id);
                match result {
                    Ok(name) => app.toast(&format!("Notion: {label} → {name}"), ToastKind::Ok),
                    Err(e) => app.toast(&format!("Notion: não atualizou {label} — {e}"), ToastKind::Err),
                }
            }
            Msg::Promoted { id, result } => {
                app.notion.promoting.retain(|x| *x != id);
                match result {
                    Ok(card) => {
                        let key = card.key.clone().unwrap_or_else(|| "novo card".into());
                        app.update_task(id, |t| {
                            t.source = TaskSource::Notion;
                            t.notion_page = Some(card.page_id.clone());
                            t.notion_url = card.url.clone().or(t.notion_url.take());
                            t.card_key = card.key.clone().or(t.card_key.take());
                        });
                        app.toast(&format!("Card {key} criado no Notion"), ToastKind::Ok);
                    }
                    Err(e) => app.toast(&format!("Notion: não criou o card — {e}"), ToastKind::Err),
                }
            }
            Msg::Tested(r) => {
                app.notion.testing = false;
                match &r {
                    Ok(s) => app.toast(&format!("Notion OK: {s}"), ToastKind::Ok),
                    Err(e) => app.toast(&format!("Notion: {e}"), ToastKind::Err),
                }
                app.notion.test_result = Some(r);
            }
            Msg::Detected(r) => {
                app.notion.detecting = false;
                if let Err(e) = &r {
                    app.toast(&format!("Notion: {e}"), ToastKind::Err);
                }
                app.notion.detected = Some(r);
            }
        }
    }

    let on_board = app.view == View::Board;
    let entered = on_board && !app.notion.on_board;
    app.notion.on_board = on_board;
    if app.notion.confirm_promote.is_some_and(|(_, at)| at.elapsed() >= CONFIRM_WINDOW) {
        app.notion.confirm_promote = None;
    }
    if app.notion.syncing {
        return;
    }
    let every = Duration::from_secs(app.cfg.notion.sync_minutes as u64 * 60);
    let due = app.cfg.notion.sync_minutes > 0 && app.notion.last_sync.is_some_and(|t| t.elapsed() >= every);
    if entered || due {
        start_sync(app, false);
    } else if let (Some(ctx), Some(t)) = (&app.notion.ctx, app.notion.last_sync)
        && app.cfg.notion.sync_minutes > 0
    {
        ctx.request_repaint_after(every.saturating_sub(t.elapsed()));
    }
}

// ───────────────────────────── Config → Integrações ─────────────────────────────

fn opt_field(ui: &mut Ui, v: &mut Option<String>, hint: &str) -> bool {
    let mut s = v.clone().unwrap_or_default();
    let r = ui.add(TextEdit::singleline(&mut s).hint_text(hint).desired_width(320.0));
    if r.changed() {
        *v = (!s.trim().is_empty()).then_some(s);
    }
    r.lost_focus()
}

fn status_bufs(m: &NotionStatusMap) -> [String; 5] {
    [m.todo.join(", "), m.doing.join(", "), m.review.join(", "), m.done.join(", "), m.archived.join(", ")]
}

fn parse_list(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// Seção "Integrações" da aba Config.
pub fn settings(app: &mut App, ui: &mut Ui) {
    app.notion.ctx = Some(ui.ctx().clone());
    let theme = app.theme.clone();
    if !app.cfg.on(aic_core::Module::Notion) {
        ui.label(widgets::subtle("Módulo Notion desligado (Config → Módulos).", &theme));
        return;
    }
    ui.label(RichText::new(format!("{}  Notion", widgets::I_NOTION)).strong());
    ui.label(
        widgets::subtle(
            "Crie uma integração interna em notion.so/profile/integrations (ler, atualizar e inserir conteúdo; ler usuários com e-mail), \
             copie o token e compartilhe as databases de Backlog e Sprints com ela (••• → Conexões).",
            &theme,
        )
        .small(),
    );
    ui.add_space(6.0);

    let mut save = false;
    let token_src = app.notion.token().map(|(_, s)| *s);
    egui::Grid::new("notion-cfg").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        ui.label("Token");
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add(TextEdit::singleline(&mut app.notion.token_input).password(true).hint_text("ntn_…").desired_width(240.0));
                if ui.add_enabled(!app.notion.token_input.trim().is_empty(), egui::Button::new("Salvar token")).clicked() {
                    match notion::save_token(&app.notion.token_input) {
                        Ok(()) => {
                            app.notion.token_input.clear();
                            app.notion.token = None;
                            app.toast("Token do Notion salvo (secrets.toml, 600)", ToastKind::Ok);
                        }
                        Err(e) => app.toast(&format!("Não salvou o token: {e}"), ToastKind::Err),
                    }
                }
                if token_src == Some(TokenSource::File) && ui.button("Remover").clicked() {
                    match notion::save_token("") {
                        Ok(()) => {
                            app.notion.token = None;
                            app.toast("Token do Notion removido", ToastKind::Info);
                        }
                        Err(e) => app.toast(&format!("Não removeu o token: {e}"), ToastKind::Err),
                    }
                }
            });
            let (txt, color) = match token_src {
                Some(TokenSource::Env) => ("configurado pela env NOTION_TOKEN".to_string(), theme.ok),
                Some(TokenSource::File) => (format!("configurado em {}", aic_core::paths::pretty(&notion::secrets_path())), theme.ok),
                None => ("não configurado".to_string(), theme.warn),
            };
            ui.label(RichText::new(txt).color(color).small());
        });
        ui.end_row();

        let n = &mut app.cfg.notion;
        ui.label("Database de tasks");
        save |= opt_field(ui, &mut n.tasks_database, "id da database ou do data source");
        ui.end_row();
        ui.label("Database de sprints");
        save |= opt_field(ui, &mut n.sprints_database, "id (pra achar a sprint atual)");
        ui.end_row();
        ui.label("Eu (me)");
        save |= opt_field(ui, &mut n.me, "seu e-mail ou id de usuário no Notion");
        ui.end_row();
        ui.label("Filtro");
        ui.horizontal(|ui| {
            save |= ui.checkbox(&mut n.only_mine, "só meus cards").changed();
            save |= ui.checkbox(&mut n.only_current_sprint, "só da sprint atual").changed();
        });
        ui.end_row();
        ui.label("Sync automático");
        ui.horizontal(|ui| {
            let r = ui.add(egui::DragValue::new(&mut n.sync_minutes).range(0..=240).suffix(" min"));
            save |= r.drag_stopped() || r.lost_focus();
            ui.label(widgets::subtle("0 = só manual e ao abrir o board", &theme).small());
        });
        ui.end_row();

        for (label, field, hint) in [
            ("Prop. título", &mut n.title_property, "vazio = a propriedade title"),
            ("Prop. status", &mut n.status_property, "Status"),
            ("Prop. ID (chave)", &mut n.key_property, "ID (unique_id)"),
            ("Prefixo da chave", &mut n.key_prefix, "DEV"),
            ("Prop. responsável", &mut n.assignee_property, "Responsável"),
            ("Prop. sprint", &mut n.sprint_property, "Sprint (relação)"),
            ("Prop. prioridade", &mut n.priority_property, "Prioridade"),
            ("Prop. tipo", &mut n.type_property, "Tipo"),
            ("Status da sprint", &mut n.sprint_status_property, "Sprint status"),
            ("Valor da sprint atual", &mut n.sprint_current_value, "Current"),
        ] {
            ui.label(label);
            save |= opt_field(ui, field, hint);
            ui.end_row();
        }
    });

    // Status por coluna (o primeiro nome é o que o AIC grava no Notion).
    ui.add_space(6.0);
    ui.label(widgets::subtle("Status do Notion por coluna — separados por vírgula; o primeiro é o gravado ao mover o card.", &theme).small());
    let mut bufs = app.notion.status_bufs.take().unwrap_or_else(|| status_bufs(&app.cfg.notion.status_map));
    egui::Grid::new("notion-status").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
        for (i, st) in [TaskStatus::Todo, TaskStatus::Doing, TaskStatus::Review, TaskStatus::Done, TaskStatus::Archived].iter().enumerate() {
            ui.label(st.label());
            if ui.add(TextEdit::singleline(&mut bufs[i]).desired_width(420.0)).lost_focus() {
                let m = &mut app.cfg.notion.status_map;
                let list = parse_list(&bufs[i]);
                match st {
                    TaskStatus::Todo => m.todo = list,
                    TaskStatus::Doing => m.doing = list,
                    TaskStatus::Review => m.review = list,
                    TaskStatus::Done => m.done = list,
                    TaskStatus::Archived => m.archived = list,
                }
                save = true;
            }
            ui.end_row();
        }
    });
    app.notion.status_bufs = Some(bufs);

    if save {
        match app.cfg.save() {
            Ok(()) => {}
            Err(e) => app.toast(&format!("Não salvou a config: {e}"), ToastKind::Err),
        }
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let has_token = token_src.is_some();
        if ui.add_enabled(has_token && !app.notion.testing, egui::Button::new("Testar conexão")).clicked() {
            test_connection(app);
        }
        if ui.add_enabled(has_token && !app.notion.detecting, egui::Button::new("Detectar propriedades")).clicked() {
            detect(app);
        }
        if ui.add_enabled(has_token && !app.notion.syncing, egui::Button::new("Sincronizar agora")).clicked() {
            start_sync(app, true);
        }
        if app.notion.testing || app.notion.detecting || app.notion.syncing {
            ui.spinner();
        }
    });
    match &app.notion.test_result {
        Some(Ok(s)) => {
            ui.label(RichText::new(format!("✔ {s}")).color(theme.ok).small());
        }
        Some(Err(e)) => {
            ui.label(RichText::new(format!("✘ {e}")).color(theme.err).small());
        }
        None => {}
    }
    if let Some(e) = &app.notion.last_error {
        ui.label(RichText::new(format!("último sync falhou: {e}")).color(theme.err).small());
    } else if let (Some(t), Some(s)) = (app.notion.last_ok, &app.notion.last_summary) {
        ui.label(widgets::subtle(format!("último sync {} — {s}", t.format("%d/%m %H:%M")), &theme).small());
    }

    let mut apply = None;
    let mut discard = false;
    match &app.notion.detected {
        Some(Ok(d)) => {
            ui.add_space(6.0);
            ui.label(RichText::new(format!("Sugestão para \"{}\"", d.name)).strong());
            let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "—".into());
            let mut lines = vec![
                format!("título: {}", show(&d.title)),
                format!("status: {}", show(&d.status)),
                format!("ID: {} (prefixo {})", show(&d.key), show(&d.key_prefix)),
                format!("responsável: {}", show(&d.assignee)),
                format!("sprint: {}", show(&d.sprint)),
                format!("prioridade: {}", show(&d.priority)),
                format!("tipo: {}", show(&d.kind)),
            ];
            if let Some(m) = &d.status_map {
                lines.push(format!(
                    "colunas: A fazer [{}] · Fazendo [{}] · Revisão [{}] · Feito [{}] · Arquivado [{}]",
                    m.todo.join(", "),
                    m.doing.join(", "),
                    m.review.join(", "),
                    m.done.join(", "),
                    m.archived.join(", ")
                ));
            }
            for l in lines {
                ui.label(widgets::subtle(l, &theme).small());
            }
            ui.horizontal(|ui| {
                if ui.button("Aplicar sugestão").clicked() {
                    apply = Some(d.clone());
                }
                if ui.button("Descartar").clicked() {
                    discard = true;
                }
            });
        }
        Some(Err(e)) => {
            ui.label(RichText::new(format!("✘ {e}")).color(theme.err).small());
        }
        None => {}
    }
    if discard {
        app.notion.detected = None;
    }
    if let Some(d) = apply {
        d.apply(&mut app.cfg.notion);
        app.notion.status_bufs = None;
        app.notion.detected = None;
        let _ = app.cfg.save();
        app.toast("Mapeamento do Notion aplicado", ToastKind::Ok);
    }

    ui.add_space(10.0);
    ui.label(widgets::subtle("Slack: cole o link da thread na tarefa; o Claude lê pela skill aic:contexto.", &theme));
}

fn test_connection(app: &mut App) {
    let Some((token, _)) = app.notion.token().cloned() else { return };
    app.notion.testing = true;
    app.notion.test_result = None;
    let cfg = app.cfg.notion.clone();
    app.notion.spawn(move || {
        let r = (|| -> anyhow::Result<String> {
            let c = Client::new(token)?;
            let bot = c.me_bot()?;
            let mut parts = vec![format!("conectado como {bot}")];
            if let Some(db) = cfg.tasks_database.as_deref().filter(|s| !s.trim().is_empty()) {
                let ds = c.data_source(db)?;
                let d = notion::detect_schema(&ds);
                parts.push(format!("database \"{}\"", d.name));
            }
            if let Some(me) = cfg.me.as_deref().filter(|s| !s.trim().is_empty()) {
                parts.push(match c.resolve_user(me) {
                    Ok(_) => "usuário 'me' encontrado".into(),
                    Err(e) => format!("'me': {e}"),
                });
            }
            if cfg.only_current_sprint {
                parts.push(match c.current_sprint(&cfg) {
                    Ok(Some((_, n))) => format!("sprint atual: {n}"),
                    Ok(None) => "sprint atual não encontrada".into(),
                    Err(e) => format!("sprints: {e}"),
                });
            }
            Ok(parts.join(" · "))
        })();
        Msg::Tested(r.map_err(|e| format!("{e:#}")))
    });
}

fn detect(app: &mut App) {
    let Some((token, _)) = app.notion.token().cloned() else { return };
    let Some(db) = app.cfg.notion.tasks_database.clone().filter(|s| !s.trim().is_empty()) else {
        app.toast("Preencha a database de tasks primeiro", ToastKind::Warn);
        return;
    };
    app.notion.detecting = true;
    app.notion.spawn(move || {
        let r = Client::new(token).and_then(|c| c.data_source(&db)).map(|ds| notion::detect_schema(&ds));
        Msg::Detected(r.map_err(|e| format!("{e:#}")))
    });
}
