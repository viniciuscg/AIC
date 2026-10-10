//! Ponte com Notion/Slack: "Abrir card" e "Buscar no Notion" do bloco.
//!
//! A busca (ver `aic_core::card`) roda numa thread: `claude -p` com os conectores da conta, ou
//! a API do Notion com token. O resultado volta por canal; a UI nunca espera. O card lido fica
//! em cache no `kv` (`card:<task-id>`), que alimenta o resumo do painel lateral.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{Duration, Instant};

use aic_core::card::{self, CardInfo, CreateOptions, PlannedKind};
use aic_core::{Connector, Id, Module, SessionKind, Task, TaskSource, TaskStatus, git, paths};
use egui::{Align, Context, FontId, Key, Layout, Modifiers, RichText, TextEdit, Ui, Vec2};

use crate::app::{App, ToastKind};
use crate::theme::Theme;
use crate::ui::modals::Modal;
use crate::ui::widgets::{self, I_CARD, I_CHECK, I_NOTION, I_SLACK, I_TERM, I_WORKTREE};

type FetchResult = Result<CardInfo, String>;

/// Buscas do "Buscar no Notion" (vive em `App::connector`).
pub struct ConnectorState {
    tx: Sender<(Id, FetchResult)>,
    rx: Receiver<(Id, FetchResult)>,
    ctx: Option<Context>,
    /// Tarefas com busca em andamento.
    pending: HashSet<Id>,
    /// Resumo por tarefa (lido do `kv` uma vez).
    summaries: RefCell<HashMap<Id, Option<String>>>,
}

impl Default for ConnectorState {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx, ctx: None, pending: HashSet::new(), summaries: RefCell::new(HashMap::new()) }
    }
}

fn cache_key(task: Id) -> String {
    format!("card:{task}")
}

/// Card em cache de uma tarefa.
pub fn cached_card(app: &App, task: Id) -> Option<CardInfo> {
    app.store.get_kv(&cache_key(task)).and_then(|s| serde_json::from_str(&s).ok())
}

fn cache_card(app: &mut App, task: Id, card: &CardInfo) {
    if let Ok(s) = serde_json::to_string(card) {
        let _ = app.store.set_kv(&cache_key(task), &s);
    }
    app.connector.summaries.borrow_mut().insert(task, Some(card.summary_text()).filter(|s| !s.is_empty()));
}

/// "no Notion pelo Claude" / "na API do Notion".
fn via(app: &App) -> &'static str {
    match app.cfg.connector {
        Connector::Claude => "no Notion pelo Claude",
        Connector::Token => "na API do Notion",
    }
}

/// Dispara a busca numa thread. `done` recebe o resultado; depois a UI é acordada.
fn spawn_fetch(app: &App, input: String, cancel: Arc<AtomicBool>, ctx: Option<Context>, done: impl FnOnce(FetchResult) + Send + 'static) {
    let connector = app.cfg.connector;
    let repos = app.cfg.repos();
    let notion_cfg = app.cfg.notion.clone();
    let slack = app.cfg.on(Module::Slack);
    let _ = std::thread::Builder::new().name("aic-card".into()).spawn(move || {
        done(card::fetch(connector, &input, &repos, &notion_cfg, slack, &cancel));
        if let Some(c) = ctx {
            c.request_repaint();
        }
    });
}

// ───────────────────────── buscar no Notion (painel do bloco) ─────────────────────────

/// Aplica resultados de busca; chamado a cada frame.
pub fn poll(app: &mut App, ctx: &Context) {
    if app.connector.ctx.is_none() {
        app.connector.ctx = Some(ctx.clone());
    }
    while let Ok((id, r)) = app.connector.rx.try_recv() {
        app.connector.pending.remove(&id);
        match r {
            Ok(c) => {
                apply_card(app, id, &c);
                app.toast(&format!("Card {} atualizado", c.key.as_deref().unwrap_or(&c.title)), ToastKind::Ok);
            }
            Err(e) => app.toast(&format!("Buscar card: {e}"), ToastKind::Err),
        }
    }
}

/// Atualiza a tarefa com o card lido. As notas só mudam se estavam vazias ou eram o resumo anterior.
fn apply_card(app: &mut App, id: Id, c: &CardInfo) {
    let old = cached_card(app, id).map(|o| o.summary_text());
    let summary = c.summary_text();
    app.update_task(id, |t| {
        t.title = c.title.clone();
        t.card_key = c.key.clone().or(t.card_key.take());
        t.notion_url = c.url.clone().or(t.notion_url.take());
        t.notion_page = c.page_id().or(t.notion_page.take());
        if t.slack_thread.is_none() {
            t.slack_thread = c.slack_threads.first().cloned();
        }
        if t.notes.trim().is_empty() || old.as_deref() == Some(t.notes.as_str()) {
            t.notes = summary.clone();
        }
    });
    cache_card(app, id, c);
}

pub fn refresh_card(app: &mut App, task: Id) {
    let Some(t) = app.tasks.iter().find(|t| t.id == task) else { return };
    if !app.cfg.on(Module::Notion) {
        return app.toast("Módulo Notion desligado (Config → Módulos)", ToastKind::Warn);
    }
    let Some(input) = t.notion_url.clone().or_else(|| t.notion_page.clone()).or_else(|| t.card_key.clone()) else {
        return app.toast("Essa tarefa não tem card (link ou chave) pra buscar", ToastKind::Warn);
    };
    let label = t.card_key.clone().unwrap_or_else(|| card::ref_label(&input));
    if !app.connector.pending.insert(task) {
        return;
    }
    let tx = app.connector.tx.clone();
    spawn_fetch(app, input, Arc::new(AtomicBool::new(false)), app.connector.ctx.clone(), move |r| {
        let _ = tx.send((task, r));
    });
    app.toast(&format!("Buscando {label} {}…", via(app)), ToastKind::Info);
}

pub fn summary_of(app: &App, task: Id) -> Option<String> {
    if app.connector.pending.contains(&task) {
        return Some(format!("Buscando {}…", via(app)));
    }
    app.connector
        .summaries
        .borrow_mut()
        .entry(task)
        .or_insert_with(|| cached_card(app, task).map(|c| c.summary_text()).filter(|s| !s.is_empty()))
        .clone()
}

// ───────────────────────── abrir card (diálogo) ─────────────────────────

/// Estado do diálogo "Abrir card".
#[derive(Default)]
pub struct OpenCardForm {
    pub link: String,
    pub stage: Stage,
    pub error: Option<String>,
}

#[derive(Default)]
pub enum Stage {
    /// Colando o link.
    #[default]
    Input,
    /// Esperando a busca (Esc cancela).
    Fetching(Fetch),
    /// Card lido: escolher repos e opções.
    Review(Review),
}

pub struct Fetch {
    rx: Receiver<FetchResult>,
    cancel: Arc<AtomicBool>,
    started: Instant,
    label: String,
}

pub struct Review {
    pub card: CardInfo,
    /// Repos (nome, pasta, marcado). Os sugeridos vêm primeiro.
    pub repos: Vec<(String, PathBuf, bool)>,
    /// Linha selecionada: repos, depois as opções.
    pub sel: usize,
    pub opts: CreateOptions,
}

/// Linhas de opção depois dos repos (worktree, shell, notas, permissão).
const OPT_ROWS: usize = 4;

impl Review {
    pub fn new(app: &App, card: CardInfo) -> Self {
        let mut repos: Vec<(String, PathBuf, bool)> = app
            .cfg
            .repo_paths()
            .into_iter()
            .map(|(n, p)| {
                let on = card.repos.contains(&n);
                (n, p, on)
            })
            .collect();
        // sugeridos primeiro, na ordem do Claude
        repos.sort_by_key(|(n, _, _)| card.repos.iter().position(|r| r == n).unwrap_or(usize::MAX));
        let opts = CreateOptions { worktree: app.cfg.on(Module::Worktrees), notes: app.cfg.on(Module::Notes), ..Default::default() };
        Self { card, repos, sel: 0, opts }
    }

    fn rows(&self) -> usize {
        self.repos.len() + OPT_ROWS
    }

    fn toggle(&mut self, row: usize) {
        let n = self.repos.len();
        match row {
            r if r < n => self.repos[r].2 = !self.repos[r].2,
            r if r == n => self.opts.worktree = !self.opts.worktree,
            r if r == n + 1 => self.opts.shell = !self.opts.shell,
            r if r == n + 2 => self.opts.notes = !self.opts.notes,
            _ => self.opts.plan_mode = !self.opts.plan_mode,
        }
    }

    fn picked(&self) -> Vec<(String, PathBuf)> {
        self.repos.iter().filter(|r| r.2).map(|(n, p, _)| (n.clone(), p.clone())).collect()
    }
}

impl OpenCardForm {
    /// Diálogo já com um card lido (usado também nos testes visuais).
    pub fn with_card(app: &App, card: CardInfo) -> Self {
        Self { link: card.url.clone().or(card.key.clone()).unwrap_or_default(), stage: Stage::Review(Review::new(app, card)), error: None }
    }
}

pub fn open_card_dialog(app: &mut App) {
    if !app.cfg.on(Module::Launcher) {
        return app.toast("Módulo \"Abrir card\" desligado (Config → Módulos)", ToastKind::Warn);
    }
    app.modal = Some(Modal::OpenCard(OpenCardForm::default()));
}

enum Act {
    None,
    Fetch,
    Cancel,
    Create,
}

pub fn open_card_modal(app: &mut App, ctx: &Context, mut f: OpenCardForm) -> Option<Modal> {
    let esc = ctx.input(|i| i.key_pressed(Key::Escape));
    // a busca terminou?
    if let Stage::Fetching(job) = &f.stage {
        match job.rx.try_recv() {
            Ok(Ok(c)) => f.stage = Stage::Review(Review::new(app, c)),
            Ok(Err(e)) => {
                f.error = Some(e);
                f.stage = Stage::Input;
            }
            Err(TryRecvError::Disconnected) => {
                f.error = Some("a busca parou sem resposta".into());
                f.stage = Stage::Input;
            }
            Err(TryRecvError::Empty) => {
                if esc {
                    // Esc cancela a busca e volta pro link, sem fechar o diálogo
                    job.cancel.store(true, Ordering::Relaxed);
                    f.stage = Stage::Input;
                    f.error = Some("Busca cancelada.".into());
                    app.modal = Some(Modal::OpenCard(f));
                    return None;
                }
                ctx.request_repaint_after(Duration::from_millis(250));
            }
        }
    }
    let theme = app.theme.clone();
    let mode = match app.cfg.connector {
        Connector::Claude => "via Claude Code (conectores da sua conta)",
        Connector::Token => "via API do Notion (token)",
    };
    let mut act = Act::None;
    let closed = egui::Modal::new(egui::Id::new("open-card"))
        .show(ctx, |ui| {
            ui.set_width(700.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{I_CARD}  Abrir card")).font(FontId::monospace(16.0)).color(theme.accent_hi));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(widgets::subtle(mode, &theme).small());
                });
            });
            ui.add_space(8.0);
            act = match &mut f.stage {
                Stage::Input => input_stage(ui, &mut f.link, f.error.as_deref(), &theme),
                Stage::Fetching(job) => fetching_stage(ui, job, &theme),
                Stage::Review(r) => review_stage(ui, r, &theme),
            };
        })
        .should_close();
    match act {
        Act::Fetch => start_fetch(app, ctx, &mut f),
        Act::Cancel => {
            if let Stage::Fetching(job) = &f.stage {
                job.cancel.store(true, Ordering::Relaxed);
            }
            return None;
        }
        Act::Create => {
            if let Stage::Review(r) = f.stage {
                launch(app, ctx, &r.card, r.picked(), &r.opts);
            }
            return None;
        }
        Act::None => {}
    }
    if closed && !esc {
        // clique fora: fecha (e cancela a busca)
        if let Stage::Fetching(job) = &f.stage {
            job.cancel.store(true, Ordering::Relaxed);
        }
        return None;
    }
    Some(Modal::OpenCard(f))
}

fn start_fetch(app: &mut App, ctx: &Context, f: &mut OpenCardForm) {
    let input = f.link.trim().to_string();
    f.error = if input.is_empty() {
        Some("Cole o link do card ou a chave (ex: DEV-123).".into())
    } else if matches!(card::card_ref(&input), card::CardRef::Unknown) {
        Some("Não reconheci: cole um link do Notion ou uma chave tipo DEV-123.".into())
    } else if !app.cfg.on(Module::Notion) {
        Some("Módulo Notion desligado (Config → Módulos).".into())
    } else {
        None
    };
    if f.error.is_some() {
        return;
    }
    let (tx, rx) = channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let label = format!("Buscando {} {}…", card::ref_label(&input), via(app));
    spawn_fetch(app, input, cancel.clone(), Some(ctx.clone()), move |r| {
        let _ = tx.send(r);
    });
    f.stage = Stage::Fetching(Fetch { rx, cancel, started: Instant::now(), label });
}

fn input_stage(ui: &mut Ui, link: &mut String, error: Option<&str>, theme: &Theme) -> Act {
    let mut act = Act::None;
    let edit = ui.add(
        TextEdit::singleline(link)
            .hint_text("cole o link do card no Notion ou a chave (DEV-123)")
            .font(FontId::proportional(17.0))
            .desired_width(f32::INFINITY)
            .margin(Vec2::new(10.0, 10.0)),
    );
    edit.request_focus();
    if ui.input(|i| i.key_pressed(Key::Enter)) {
        act = Act::Fetch;
    }
    ui.add_space(6.0);
    match error {
        Some(e) => ui.label(RichText::new(e).color(theme.err)),
        None => ui.label(widgets::subtle("O AIC lê o card (e as threads do Slack citadas), sugere os repos e monta o bloco.", theme).small()),
    };
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Buscar  ⏎").clicked() {
            act = Act::Fetch;
        }
        if ui.button("Cancelar  Esc").clicked() {
            act = Act::Cancel;
        }
    });
    act
}

fn fetching_stage(ui: &mut Ui, job: &Fetch, theme: &Theme) -> Act {
    let mut act = Act::None;
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(18.0).color(theme.accent));
        ui.label(RichText::new(&job.label).size(15.0).color(theme.ink));
        ui.label(widgets::subtle(format!("{}s", job.started.elapsed().as_secs()), theme).monospace());
    });
    ui.add_space(4.0);
    ui.label(widgets::subtle("Pode levar até um minuto: o Claude abre o card, as threads e escolhe os repos. Nada é alterado no Notion/Slack.", theme).small());
    ui.add_space(10.0);
    if ui.button("Cancelar busca  Esc").clicked() {
        act = Act::Cancel;
    }
    act
}

fn check(on: bool) -> &'static str {
    // nf-md-checkbox_marked / checkbox_blank_outline
    if on { "\u{f0132}" } else { "\u{f0131}" }
}

/// Linha marcável da lista, na largura toda; `hint` (atalho) à direita.
fn check_row(ui: &mut Ui, selected: bool, on: bool, text: &str, hint: &str, theme: &Theme) -> egui::Response {
    let mut job = egui::text::LayoutJob::default();
    let fmt = |color| egui::TextFormat { font_id: FontId::monospace(13.5), color, ..Default::default() };
    job.append(check(on), 0.0, fmt(if on { theme.accent } else { theme.faint }));
    job.append(&format!("  {text}"), 0.0, fmt(if on { theme.ink } else { theme.dim }));
    let resp = ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| ui.selectable_label(selected, job)).inner;
    if !hint.is_empty() {
        let pos = resp.rect.right_center() - Vec2::new(10.0, 0.0);
        widgets::paint_text(ui, pos, egui::Align2::RIGHT_CENTER, hint, 11.0, theme.faint);
    }
    if selected {
        resp.scroll_to_me(None);
    }
    resp
}

fn review_stage(ui: &mut Ui, r: &mut Review, theme: &Theme) -> Act {
    let mut act = Act::None;
    // teclado: ↑↓ andam, Espaço marca, Enter cria, Alt+letra liga as opções
    let (down, up, space, enter) = ui.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown) || i.consume_key(Modifiers::CTRL, Key::N),
            i.consume_key(Modifiers::NONE, Key::ArrowUp) || i.consume_key(Modifiers::CTRL, Key::P),
            i.consume_key(Modifiers::NONE, Key::Space),
            i.key_pressed(Key::Enter),
        )
    });
    let rows = r.rows();
    if down {
        r.sel = (r.sel + 1) % rows;
    }
    if up {
        r.sel = (r.sel + rows - 1) % rows;
    }
    if space {
        r.toggle(r.sel);
    }
    if enter {
        act = Act::Create;
    }
    for (key, row) in [(Key::W, 0), (Key::S, 1), (Key::N, 2), (Key::P, 3)] {
        if ui.input_mut(|i| i.consume_key(Modifiers::ALT, key)) {
            r.toggle(r.repos.len() + row);
        }
    }

    card_box(ui, &r.card, theme);
    ui.add_space(10.0);

    let n = r.repos.len();
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new("REPOS").font(FontId::monospace(10.5)).color(theme.dim));
        ui.label(widgets::subtle("↑↓ navega · Espaço marca · ✦ sugerido pelo Claude", theme).small());
    });
    egui::ScrollArea::vertical().id_salt("oc-repos").max_height(150.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if n == 0 {
            ui.label(widgets::subtle("Nenhum repo nas pastas de Config → Pastas: o Claude abre na home.", theme).small());
        }
        for (i, (name, path, on)) in r.repos.iter().enumerate() {
            let hint = if r.card.repos.contains(name) { "✦ sugerido" } else { "" };
            if check_row(ui, r.sel == i, *on, name, hint, theme).on_hover_text(paths::pretty(path)).clicked() {
                clicked = Some(i);
            }
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("OPÇÕES").font(FontId::monospace(10.5)).color(theme.dim));
    let branch = git::branch_slug(r.card.key.as_deref(), &r.card.title);
    let opts = [
        (r.opts.worktree, format!("{I_WORKTREE} worktree por repo → {branch}"), "Alt+W"),
        (r.opts.shell, format!("{I_TERM} + shell no primeiro repo"), "Alt+S"),
        (r.opts.notes, "notas do bloco com o resumo".to_string(), "Alt+N"),
        (r.opts.plan_mode, "Claude em modo plan (só planeja até você liberar)".to_string(), "Alt+P"),
    ];
    for (k, (on, text, key)) in opts.iter().enumerate() {
        let row = n + k;
        if check_row(ui, r.sel == row, *on, text, key, theme).clicked() {
            clicked = Some(row);
        }
    }
    ui.label(widgets::subtle(format!("{I_CHECK} um Claude por repo, com o contexto do card e pedindo um plano antes de editar"), theme).small());
    if let Some(i) = clicked {
        r.sel = i;
        r.toggle(i);
    }
    ui.add_space(8.0);
    let picked = r.repos.iter().filter(|x| x.2).count();
    let claudes = picked.max(1);
    let mut plan = vec![format!("{claudes} Claude{}", if claudes > 1 { "s" } else { "" })];
    if r.opts.worktree && picked > 0 {
        plan.push(format!("{picked} worktree{}", if picked > 1 { "s" } else { "" }));
    }
    if r.opts.shell {
        plan.push("1 shell".into());
    }
    plan.push("tarefa em Fazendo".into());
    ui.label(widgets::subtle(format!("Vai criar o bloco \"{}\" · {}", card::block_name(&r.card), plan.join(" · ")), theme).small());
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button(RichText::new("Criar bloco  ⏎").strong()).clicked() {
            act = Act::Create;
        }
        if ui.button("Cancelar  Esc").clicked() {
            act = Act::Cancel;
        }
    });
    act
}

/// Caixa com chave, status, título, resumo, critérios e links do card.
fn card_box(ui: &mut Ui, c: &CardInfo, theme: &Theme) {
    egui::Frame::new()
        .fill(theme.bg3)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .corner_radius(theme.radius)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if let Some(k) = &c.key {
                    ui.label(RichText::new(k).font(FontId::monospace(13.0)).color(theme.accent_hi).strong());
                }
                if let Some(s) = &c.status {
                    widgets::chip(ui, s, theme.accent, theme);
                }
                if let Some(a) = &c.assignee {
                    widgets::chip(ui, a, theme.dim, theme);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for u in c.slack_threads.iter().take(2).rev() {
                        if ui.small_button(format!("{I_SLACK} Thread ↗")).on_hover_text(u).clicked() {
                            let _ = open::that_detached(u);
                        }
                    }
                    if let Some(u) = &c.url
                        && ui.small_button(format!("{I_NOTION} Notion ↗")).on_hover_text(u).clicked()
                    {
                        let _ = open::that_detached(u);
                    }
                });
            });
            ui.add_space(2.0);
            ui.label(RichText::new(&c.title).size(16.0).strong().color(theme.ink));
            if !c.summary.trim().is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(c.summary.trim()).color(theme.ink));
            }
            if !c.criteria.is_empty() {
                ui.add_space(4.0);
                for x in c.criteria.iter().take(5) {
                    ui.label(RichText::new(format!("• {x}")).small().color(theme.dim));
                }
                if c.criteria.len() > 5 {
                    ui.label(widgets::subtle(format!("+{} critérios no card", c.criteria.len() - 5), theme).small());
                }
            }
        });
}

// ───────────────────────── montar o bloco ─────────────────────────

/// Cria tarefa, bloco, worktrees, um Claude por repo, shell e notas. Falha de um repo vira aviso.
fn launch(app: &mut App, ctx: &Context, c: &CardInfo, picks: Vec<(String, PathBuf)>, opts: &CreateOptions) {
    let plan = card::plan_creation(c, &picks, &paths::home_dir(), opts);
    let task = upsert_task(app, c, picks.first().map(|(n, _)| n.clone()));
    let Some(block) = app.new_block(&plan.block_name, Some(task)) else {
        return app.toast("Não deu pra criar o bloco (sem workspace?)", ToastKind::Err);
    };
    if let Some(notes) = &plan.notes {
        app.update_block(block, |b| b.notes = notes.clone());
    }
    let mut cwd_of: HashMap<String, PathBuf> = HashMap::new();
    let (mut first, mut opened, mut worktrees) = (None, 0, 0);
    for s in plan.sessions {
        let (cwd, worktree) = match (&s.branch, s.kind) {
            (Some(branch), _) => {
                let root = git::main_root(&s.path).unwrap_or_else(|| s.path.clone());
                match git::add_worktree(&root, branch, None) {
                    Ok(p) => {
                        worktrees += 1;
                        (p.clone(), Some(p))
                    }
                    Err(e) => {
                        app.toast(&format!("Worktree de {} falhou ({e:#}); abrindo direto no repo", s.name), ToastKind::Warn);
                        (s.path.clone(), None)
                    }
                }
            }
            // o shell vai pra mesma pasta do Claude do repo (o worktree, se houver)
            (None, PlannedKind::Shell) => (s.repo.as_ref().and_then(|r| cwd_of.get(r)).cloned().unwrap_or_else(|| s.path.clone()), None),
            (None, PlannedKind::Claude) => (s.path.clone(), None),
        };
        if let Some(r) = &s.repo {
            cwd_of.entry(r.clone()).or_insert_with(|| cwd.clone());
        }
        let (kind, task_link) = match s.kind {
            PlannedKind::Claude => {
                if let Err(e) = write_context(&cwd, &plan.context_md) {
                    app.toast(&format!("{}: não escrevi .aic/context.md ({e})", s.name), ToastKind::Warn);
                }
                (SessionKind::Claude, Some(task))
            }
            PlannedKind::Shell => (SessionKind::Shell, None),
        };
        let is_claude = kind == SessionKind::Claude;
        let id = app.create_session_in(ctx, Some(block), s.name.clone(), kind, cwd, worktree, task_link, s.claude_args, s.prompt);
        if id.is_nil() {
            app.toast(&format!("{}: não abriu o terminal", s.name), ToastKind::Err);
            continue;
        }
        opened += 1;
        if is_claude && first.is_none() {
            first = Some(id);
        }
    }
    match first {
        Some(id) => app.focus(id),
        None => app.focus_block(block),
    }
    let wt = if worktrees > 0 { format!(", {worktrees} worktree{}", if worktrees > 1 { "s" } else { "" }) } else { String::new() };
    app.toast(&format!("Bloco \"{}\" montado: {opened} terminais{wt}", plan.block_name), ToastKind::Ok);
}

/// Tarefa do card: reaproveita a que já existe (mesma chave ou página, ex: vinda do sync) ou cria.
fn upsert_task(app: &mut App, c: &CardInfo, repo: Option<String>) -> Id {
    let page = c.page_id();
    let existing = app
        .tasks
        .iter()
        .find(|t| (c.key.is_some() && t.card_key == c.key) || (page.is_some() && t.notion_page.as_deref().map(aic_core::notion::norm_id) == page))
        .map(|t| t.id);
    let id = existing.unwrap_or_else(|| {
        let mut t = Task::new(c.title.clone());
        t.workspace = app.current_ws().map(|w| w.id);
        let id = t.id;
        app.save_task(t);
        id
    });
    app.update_task(id, |t| {
        t.source = TaskSource::Notion;
        t.status = TaskStatus::Doing;
        if t.repo.is_none() {
            t.repo = repo;
        }
    });
    apply_card(app, id, c);
    id
}

/// `.aic/context.md` na pasta (fora do git pelo `info/exclude`).
fn write_context(cwd: &Path, md: &str) -> std::io::Result<()> {
    let dir = cwd.join(".aic");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("context.md"), md)?;
    crate::app::ensure_git_exclude(cwd)
}
