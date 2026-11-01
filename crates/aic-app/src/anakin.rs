//! Anakin: o assistente pessoal. Um Claude com o agente `aic:anakin`, sempre aberto, que aparece
//! como um boneco no canto da tela (`ui::avatar`). Clicar abre a conversa numa janela flutuante:
//! texto (digita no terminal dele) ou voz (grava, transcreve e manda).
//!
//! Ele mexe no AIC pelo `aic` de linha de comando (skill `aic:anakin`), que fala com o app pela
//! caixa de entrada (`aic_core::inbox`). Este módulo também executa esses comandos.
//!
//! A sessão tem id fixo (guardado no `kv`), então volta com `--resume` a cada abertura: a
//! conversa com o Anakin continua de onde parou. Ela não pertence a nenhum workspace.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use aic_core::events::HookEvent;
use aic_core::inbox::{self, Command, Envelope};
use aic_core::voice::{self, Mic, Recording};
use aic_core::{Id, Module, Session, SessionKind, SessionState, git, paths};
use aic_term::TerminalView;
use egui::{Align, Context, FontId, Layout, Margin, RichText, Sense, Ui, UiBuilder, Vec2};

use crate::actions::Action;
use crate::app::{App, Phase, ToastKind, View};
use crate::session::Runtime;
use crate::ui::widgets::{self, I_CLOSE, I_RESTART};

pub const NAME: &str = "Anakin";
/// Agente do plugin (`claude/agents/anakin.md`).
pub const AGENT: &str = "aic:anakin";
pub const I_MIC: &str = "\u{f036c}";
pub const I_STOP: &str = "\u{f04db}";

/// Chave e pasta mantêm o nome antigo: a conversa continua de onde parou (o Claude acha o
/// transcript pelo id e pela pasta).
const KV_SESSION: &str = "layne:session";

pub struct AnakinState {
    /// Janela de conversa aberta.
    pub open: bool,
    /// O boneco do canto.
    pub avatar: crate::ui::avatar::AvatarState,
    /// Teclado vai pro terminal do Anakin (e não pro terminal em foco das sessões).
    pub focus: bool,
    pub rt: Option<Runtime>,
    pub rec: Option<Recording>,
    /// Transcrição rodando numa thread.
    transcribing: Option<(Receiver<Result<String, String>>, Instant)>,
    /// Último texto transcrito (mostrado no painel).
    pub last_heard: Option<String>,
    /// Microfones (lidos sob demanda pela Config).
    pub mics: Option<Vec<Mic>>,
    inbox_at: Instant,
    /// A sessão já foi iniciada nesta execução (não tenta de novo a cada frame).
    tried: bool,
}

impl Default for AnakinState {
    fn default() -> Self {
        Self {
            open: false,
            avatar: Default::default(),
            focus: false,
            rt: None,
            rec: None,
            transcribing: None,
            last_heard: None,
            mics: None,
            inbox_at: Instant::now(),
            tried: false,
        }
    }
}

impl AnakinState {
    pub fn recording(&self) -> bool {
        self.rec.is_some()
    }

    pub fn transcribing(&self) -> bool {
        self.transcribing.is_some()
    }

    pub fn state(&self) -> SessionState {
        self.rt
            .as_ref()
            .map(Runtime::state)
            .unwrap_or(SessionState::Boot)
    }

    pub fn session_id(&self) -> Option<Id> {
        self.rt.as_ref().map(|r| r.s.id)
    }
}

/// Pasta da sessão do Anakin (o cwd dele).
pub fn dir() -> PathBuf {
    paths::data_dir().join("layne")
}

fn session_id(app: &App) -> Id {
    if let Some(id) = app.store.get_kv(KV_SESSION).and_then(|s| s.parse().ok()) {
        return id;
    }
    let id = aic_core::new_id();
    let _ = app.store.set_kv(KV_SESSION, &id.to_string());
    id
}

/// Args do `claude` do Anakin: o agente, permissão pro `aic` e o modelo escolhido.
pub fn claude_args(model: Option<&str>) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "--agent".into(),
        AGENT.into(),
        "--allowedTools".into(),
        "Bash(aic:*)".into(),
    ];
    if let Some(m) = model.map(str::trim).filter(|m| !m.is_empty()) {
        a.extend(["--model".into(), m.to_string()]);
    }
    a
}

/// Sobe (ou reconecta) a sessão do Anakin.
pub fn start(app: &mut App, ctx: &Context) {
    app.anakin.tried = true;
    if app.inj.plugin_dir.is_none() {
        return app.toast(
            "O Anakin precisa do plugin do AIC (rode ./install.sh)",
            ToastKind::Warn,
        );
    }
    let cwd = dir();
    let _ = std::fs::create_dir_all(&cwd);
    // a pasta é do AIC: sem isso o Claude abre parado em "Do you trust this folder?"
    if let Err(e) = aic_core::claude::ensure_trusted(&cwd) {
        app.toast(&format!("Anakin: não consegui marcar a pasta como confiável ({e:#}); responda a pergunta no painel"), ToastKind::Warn);
    }
    let mut s = Session::new(Id::nil(), NAME, SessionKind::Claude, cwd);
    s.id = session_id(app);
    s.claude_session = s.id;
    s.claude_args = claude_args(app.cfg.anakin.model.as_deref());
    let mut rt = Runtime::new(s);
    let term_id = app.alloc_term_id();
    rt.spawn(
        ctx,
        term_id,
        &app.inj,
        &app.theme.term,
        app.cfg.on(Module::Tmux),
    );
    if let Some(e) = &rt.spawn_error {
        app.toast(&format!("O Anakin não abriu: {e}"), ToastKind::Err);
    }
    app.anakin.rt = Some(rt);
}

/// Reinicia o processo (a conversa continua: mesmo id, `--resume`).
pub fn restart(app: &mut App, ctx: &Context) {
    if let Some(rt) = app.anakin.rt.take() {
        if let Some(t) = &rt.term {
            t.kill();
        }
        crate::tmux::kill(rt.s.id);
    }
    start(app, ctx);
}

/// Começa uma conversa nova (esquece a anterior).
pub fn reset(app: &mut App, ctx: &Context) {
    let _ = app
        .store
        .set_kv(KV_SESSION, &aic_core::new_id().to_string());
    restart(app, ctx);
}

fn ensure(app: &mut App, ctx: &Context) {
    if app.anakin.rt.is_none() {
        start(app, ctx);
    }
}

pub fn show(app: &mut App, ctx: &Context) {
    if !app.cfg.on(Module::Anakin) {
        return app.toast(
            "Módulo \"Anakin\" desligado (Config → Módulos)",
            ToastKind::Warn,
        );
    }
    ensure(app, ctx);
    app.anakin.open = true;
    app.anakin.focus = true;
}

/// Abre/fecha o painel. Aberto e sem o teclado: só pega o teclado.
pub fn toggle(app: &mut App, ctx: &Context) {
    if app.anakin.open && app.anakin.focus {
        app.anakin.open = false;
        app.anakin.focus = false;
    } else {
        show(app, ctx);
    }
}

/// Liga/desliga a gravação. Ao parar, transcreve numa thread e manda pro Anakin.
pub fn toggle_voice(app: &mut App, ctx: &Context) {
    if !app.cfg.on(Module::Anakin) {
        return app.toast(
            "Módulo \"Anakin\" desligado (Config → Módulos)",
            ToastKind::Warn,
        );
    }
    if let Some(rec) = app.anakin.rec.take() {
        return finish_recording(app, ctx, rec);
    }
    if app.anakin.transcribing.is_some() {
        return app.toast("Ainda transcrevendo o último áudio…", ToastKind::Info);
    }
    show(app, ctx);
    match Recording::start(app.cfg.anakin.voice.mic.as_deref()) {
        Ok(r) => app.anakin.rec = Some(r),
        Err(e) => app.toast(&format!("Gravar: {e:#}"), ToastKind::Err),
    }
}

/// Descarta a gravação em andamento.
pub fn cancel_voice(app: &mut App) {
    if let Some(r) = app.anakin.rec.take() {
        r.cancel();
        app.toast("Gravação descartada", ToastKind::Info);
    }
}

fn finish_recording(app: &mut App, ctx: &Context, rec: Recording) {
    let file = match rec.stop() {
        Ok(f) => f,
        Err(e) => return app.toast(&format!("Gravar: {e:#}"), ToastKind::Warn),
    };
    let cfg = app.cfg.anakin.voice.clone();
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    let _ = std::thread::Builder::new()
        .name("aic-voz".into())
        .spawn(move || {
            let _ = tx.send(voice::transcribe(&cfg, &file).map_err(|e| format!("{e:#}")));
            ctx.request_repaint();
        });
    app.anakin.transcribing = Some((rx, Instant::now()));
}

/// Escreve no terminal do Anakin; `send` aperta Enter.
pub fn say(app: &mut App, ctx: &Context, text: &str, send: bool) -> bool {
    show(app, ctx);
    let Some(term) = app.anakin.rt.as_ref().and_then(|r| r.term.as_ref()) else {
        return false;
    };
    term.paste(text);
    if send {
        // o Enter vai separado do colar: no Claude Code, Enter dentro do bracketed paste vira quebra de linha
        term.write(b"\r".as_slice());
    }
    true
}

/// Trabalho de cada frame: sobe a sessão, lê a caixa de entrada, acompanha a voz.
pub fn tick(app: &mut App, ctx: &Context) {
    if !matches!(app.phase, Phase::Main) {
        return;
    }
    let on = app.cfg.on(Module::Anakin);
    if on && app.cfg.anakin.auto_start && !app.anakin.tried && app.modal.is_none() {
        start(app, ctx);
    }
    let open = app.anakin.open;
    if let Some(rt) = app.anakin.rt.as_mut() {
        rt.refresh(false, open);
        if let Some(t) = &rt.term {
            t.set_visible(open);
        }
        // o Claude do Anakin copiando (OSC 52) → clipboard do sistema, como nas sessões
        if let Some(text) = rt
            .term
            .as_ref()
            .and_then(|t| t.shared.clipboard.lock().unwrap().take())
        {
            ctx.copy_text(text);
        }
    }

    if app.anakin.inbox_at.elapsed() > Duration::from_millis(300) {
        app.anakin.inbox_at = Instant::now();
        for env in inbox::drain() {
            run(app, ctx, env);
        }
    }

    // gravação: limite de tempo, gravador que caiu
    if let Some(rec) = app.anakin.rec.as_mut() {
        if let Some(err) = rec.failed() {
            app.anakin.rec = None;
            app.toast(&format!("Gravação parou: {err}"), ToastKind::Err);
        } else if rec.elapsed()
            > Duration::from_secs(app.cfg.anakin.voice.max_seconds.max(5) as u64)
        {
            let rec = app.anakin.rec.take().expect("gravando");
            finish_recording(app, ctx, rec);
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    if let Some((rx, _)) = &app.anakin.transcribing {
        match rx.try_recv() {
            Ok(r) => {
                app.anakin.transcribing = None;
                match r {
                    Ok(text) => {
                        let send = app.cfg.anakin.voice.auto_send;
                        if !say(app, ctx, &text, send) {
                            app.toast(
                                "O Anakin não está rodando; o texto ficou só na conversa",
                                ToastKind::Warn,
                            );
                        }
                        app.anakin.last_heard = Some(text);
                    }
                    Err(e) => app.toast(&format!("Transcrever: {e}"), ToastKind::Err),
                }
            }
            Err(TryRecvError::Disconnected) => app.anakin.transcribing = None,
            Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(200)),
        }
    }
}

/// Evento de hook que não é de nenhuma sessão de workspace: é do Anakin?
pub fn on_event(app: &mut App, e: &HookEvent) {
    let Some(rt) = app.anakin.rt.as_mut() else {
        return;
    };
    let mine =
        e.aic_session == Some(rt.s.id) || e.claude_session == rt.s.claude_session.to_string();
    if !mine {
        return;
    }
    let effect = rt.live.apply(e);
    let visible = app.anakin.open && app.window_focused;
    if visible {
        rt.live.seen();
        return;
    }
    // a conversa fechada: o boneco fala o que ele respondeu
    let last = rt
        .live
        .last_message
        .as_deref()
        .and_then(|m| m.lines().find(|l| !l.trim().is_empty()))
        .map(|l| truncate(l.trim(), 160));
    match effect {
        Some(aic_core::events::Effect::NeedsYou(msg)) => app.anakin.avatar.speak(msg, 10),
        Some(aic_core::events::Effect::Finished) => {
            app.anakin
                .avatar
                .speak(last.unwrap_or_else(|| "Pronto.".into()), 10);
            app.anakin.avatar.jump();
        }
        None => {}
    }
}

// ───────────────────────── comandos da caixa de entrada ─────────────────────────

fn find_ws(app: &App, name: &str) -> Option<usize> {
    let n = name.trim().to_lowercase();
    if n.is_empty() {
        return None;
    }
    let names: Vec<String> = app
        .workspaces
        .iter()
        .map(|w| w.name.to_lowercase())
        .collect();
    names
        .iter()
        .position(|w| *w == n)
        .or_else(|| names.iter().position(|w| w.starts_with(&n)))
}

/// Vai pro workspace pelo nome, criando se não existir. Devolve o nome final.
fn goto_or_create(app: &mut App, name: &str) -> String {
    match find_ws(app, name) {
        Some(i) => app.switch_ws(i),
        None => app.new_workspace(name),
    }
    app.current_ws().map(|w| w.name.clone()).unwrap_or_default()
}

fn run(app: &mut App, ctx: &Context, env: Envelope) {
    let id = env.id.clone();
    let r: Result<String, String> = match env.cmd {
        Command::NewWorkspace { name, switch } => {
            let name = name.trim().to_string();
            if name.is_empty() {
                Err("o workspace precisa de um nome".into())
            } else if let Some(i) = app
                .workspaces
                .iter()
                .position(|w| w.name.eq_ignore_ascii_case(&name))
            {
                if switch {
                    app.switch_ws(i);
                }
                Ok(format!(
                    "workspace \"{}\" já existia",
                    app.workspaces[i].name
                ))
            } else {
                let before = app.ws;
                app.new_workspace(&name);
                if !switch {
                    app.switch_ws(before);
                }
                Ok(format!("workspace \"{name}\" criado"))
            }
        }
        Command::GotoWorkspace { name } => match find_ws(app, &name) {
            Some(i) => {
                app.switch_ws(i);
                app.set_view(View::Sessions);
                Ok(format!("no workspace \"{}\"", app.workspaces[i].name))
            }
            None => Err(format!(
                "nenhum workspace chamado \"{name}\" (veja `aic ws ls`)"
            )),
        },
        Command::NewSession {
            workspace,
            name,
            repo,
            cwd,
            shell,
            branch,
            prompt,
        } => new_session(app, ctx, workspace, name, repo, cwd, shell, branch, prompt),
        Command::Say { text, send } => {
            if say(app, ctx, &text, send) {
                Ok(format!("mandado pra {NAME}"))
            } else {
                Err(format!("A {NAME} não está rodando"))
            }
        }
        Command::Voice => {
            toggle_voice(app, ctx);
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            Ok(if app.anakin.recording() {
                "gravando…".into()
            } else {
                "gravação parada".into()
            })
        }
        Command::Focus => {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            Ok("janela em foco".into())
        }
        Command::Show => {
            show(app, ctx);
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            Ok("painel aberto".into())
        }
    };
    match r {
        Ok(m) => inbox::reply(&id, true, m),
        Err(e) => inbox::reply(&id, false, e),
    }
}

#[allow(clippy::too_many_arguments)]
fn new_session(
    app: &mut App,
    ctx: &Context,
    workspace: Option<String>,
    name: Option<String>,
    repo: Option<String>,
    cwd: Option<PathBuf>,
    shell: bool,
    branch: Option<String>,
    prompt: Option<String>,
) -> Result<String, String> {
    let mut dir = match (&repo, &cwd) {
        (Some(r), _) => app.cfg.repo_path(r).or_else(|| {
            // prefixo, como no #repo
            let all = app.cfg.repo_paths();
            let hits: Vec<_> = all
                .iter()
                .filter(|(n, _)| n.starts_with(r.as_str()))
                .collect();
            (hits.len() == 1).then(|| hits[0].1.clone())
        }),
        (None, Some(c)) => Some(c.clone()),
        (None, None) => Some(paths::home_dir()),
    }
    .ok_or_else(|| {
        format!(
            "repo \"{}\" não encontrado nas pastas de projetos (veja `aic repos`)",
            repo.clone().unwrap_or_default()
        )
    })?;
    if !dir.is_dir() {
        return Err(format!("pasta {} não existe", dir.display()));
    }
    let mut worktree = None;
    if let Some(b) = branch.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        let root = git::main_root(&dir).unwrap_or_else(|| dir.clone());
        let p = git::add_worktree(&root, b, None).map_err(|e| format!("worktree {b}: {e:#}"))?;
        dir = p.clone();
        worktree = Some(p);
    }
    let ws = match workspace.as_deref().filter(|w| !w.trim().is_empty()) {
        Some(w) => goto_or_create(app, w),
        None => app.current_ws().map(|w| w.name.clone()).unwrap_or_default(),
    };
    let kind = if shell {
        SessionKind::Shell
    } else {
        SessionKind::Claude
    };
    let label = name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
        let base = app
            .cfg
            .repo_of(&dir)
            .or_else(|| dir.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "home".into());
        if shell {
            format!("{base} (shell)")
        } else {
            base
        }
    });
    app.set_view(View::Sessions);
    let id = app.create_session_in(
        ctx,
        None,
        label.clone(),
        kind,
        dir.clone(),
        worktree,
        vec![],
        prompt.filter(|p| !p.trim().is_empty()),
    );
    if id.is_nil() {
        return Err("não deu pra abrir o terminal".into());
    }
    // o terminal novo pega o foco; o Anakin solta o teclado
    app.anakin.focus = false;
    Ok(format!(
        "\"{label}\" aberto no workspace \"{ws}\" em {}",
        paths::pretty(&dir)
    ))
}

// ───────────────────────── painel ─────────────────────────

pub fn panel(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let state = app.anakin.state();

    // cabeçalho: nome, estado, microfone, reiniciar, fechar
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
        widgets::state_dot(ui, r.center(), 5.0, state, &theme);
        ui.label(
            RichText::new(NAME)
                .font(FontId::monospace(14.0))
                .color(theme.accent_hi),
        );
        ui.label(
            RichText::new(crate::brand::state_label(state))
                .font(FontId::monospace(10.5))
                .color(theme.state_color(state)),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if widgets::icon_button(
                ui,
                I_CLOSE,
                "Fechar o Anakin",
                &app.keymap.tip(Action::ToggleAnakin, "Fechar a conversa"),
            )
            .clicked()
            {
                app.anakin.open = false;
                app.anakin.focus = false;
            }
            if widgets::icon_button(
                ui,
                I_RESTART,
                "Reiniciar o Anakin",
                &app.keymap
                    .tip(Action::RestartAnakin, "Reiniciar (a conversa continua)"),
            )
            .clicked()
            {
                let ctx = ui.ctx().clone();
                restart(app, &ctx);
            }
            mic_button(app, ui);
        });
    });

    // linha da voz: gravando / transcrevendo / último texto
    let rec = app
        .anakin
        .rec
        .as_ref()
        .map(|r| (r.elapsed(), r.mic.clone()));
    if let Some((el, _)) = rec {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("● gravando {}s", el.as_secs())).color(theme.err));
            ui.label(
                widgets::subtle(
                    format!(
                        "{} para · Esc descarta",
                        app.keymap.label(Action::AnakinVoice)
                    ),
                    &theme,
                )
                .small(),
            );
            if ui.small_button("descartar").on_hover_text("Esc").clicked() {
                cancel_voice(app);
            }
        });
        if app.anakin.focus && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel_voice(app);
        }
    } else if let Some((_, since)) = &app.anakin.transcribing {
        let secs = since.elapsed().as_secs();
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new().size(12.0).color(theme.accent));
            ui.label(widgets::subtle(format!("transcrevendo… {secs}s"), &theme).small());
        });
    } else if let Some(h) = &app.anakin.last_heard {
        ui.label(widgets::subtle(format!("{I_MIC} \"{}\"", truncate(h, 90)), &theme).small());
    }
    ui.add_space(4.0);

    let rect = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(rect, theme.radius(), theme.term.background);
    let border = if app.anakin.focus {
        theme.accent
    } else {
        theme.border
    };
    ui.painter().rect_stroke(
        rect,
        theme.radius(),
        egui::Stroke::new(if app.anakin.focus { 2.0 } else { 1.0 }, border),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(3.0);

    let Some(rt) = app.anakin.rt.as_mut() else {
        let mut c = ui.new_child(UiBuilder::new().max_rect(inner.shrink(12.0)));
        c.label(widgets::subtle("O Anakin não está rodando.", &theme));
        if c.button(format!("{I_RESTART} Iniciar"))
            .on_hover_text(app.keymap.tip(Action::RestartAnakin, "Iniciar"))
            .clicked()
        {
            let ctx = c.ctx().clone();
            start(app, &ctx);
        }
        ui.allocate_rect(rect, Sense::hover());
        return;
    };
    if let Some(err) = rt.spawn_error.clone() {
        let mut c = ui.new_child(UiBuilder::new().max_rect(inner.shrink(12.0)));
        c.label(RichText::new(format!("Não deu pra iniciar o Anakin:\n{err}")).color(theme.err));
        if c.button(format!("{I_RESTART} Tentar de novo"))
            .on_hover_text(app.keymap.tip(Action::RestartAnakin, "Tentar de novo"))
            .clicked()
        {
            let ctx = c.ctx().clone();
            restart(app, &ctx);
        }
        ui.allocate_rect(rect, Sense::hover());
        return;
    }
    let focus = app.anakin.focus && app.modal.is_none();
    let Some(term) = rt.term.as_mut() else { return };
    let exited = term.is_exited();
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner));
    let resp = TerminalView::new(term, &app.theme.term, &app.font, &app.term_bindings)
        .focused(focus && !exited)
        .show(&mut child, inner.size());
    if resp.clicked() || resp.drag_started() {
        app.anakin.focus = true;
    }
    ui.allocate_rect(rect, Sense::hover());
    if exited {
        let mut c = ui.new_child(
            UiBuilder::new()
                .max_rect(egui::Rect::from_center_size(
                    inner.center(),
                    Vec2::new(260.0, 60.0),
                ))
                .layout(Layout::top_down(Align::Center)),
        );
        c.label(RichText::new("O Anakin saiu.").color(theme.err));
        if c.button(format!("{I_RESTART} Reabrir"))
            .on_hover_text(app.keymap.tip(Action::RestartAnakin, "Reabrir"))
            .clicked()
        {
            let ctx = c.ctx().clone();
            restart(app, &ctx);
        }
    }
}

fn mic_button(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let rec = app.anakin.recording();
    let busy = app.anakin.transcribing();
    let (icon, color, tip) = if rec {
        (I_STOP, theme.err, "Parar e mandar pro Anakin")
    } else {
        (
            I_MIC,
            if busy { theme.faint } else { theme.ink },
            "Falar com o Anakin (grava até você parar)",
        )
    };
    let b = egui::Button::new(
        RichText::new(icon)
            .font(FontId::monospace(15.0))
            .color(color),
    )
    .frame(rec);
    if ui
        .add_enabled(!busy, b)
        .on_hover_text(app.keymap.tip(Action::AnakinVoice, tip))
        .clicked()
    {
        let ctx = ui.ctx().clone();
        toggle_voice(app, &ctx);
    }
}

// ───────────────────────── Config → Anakin ─────────────────────────

/// Seção da Config. `row(i)` diz se a linha i passa no filtro da busca.
pub fn settings(app: &mut App, ui: &mut Ui, row: impl Fn(usize) -> bool) {
    let theme = app.theme.clone();
    let mut save = false;
    if !app.cfg.on(Module::Anakin) {
        ui.label(widgets::subtle(
            "Módulo desligado (Config → Módulos → Anakin).",
            &theme,
        ));
    }
    egui::Grid::new("anakin-cfg").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
        if row(0) {
            ui.label("Sessão do Anakin");
            ui.vertical(|ui| {
                save |= ui.checkbox(&mut app.cfg.anakin.auto_start, "subir junto com o AIC").changed();
                ui.horizontal(|ui| {
                    ui.label("modelo");
                    let mut m = app.cfg.anakin.model.clone().unwrap_or_default();
                    if ui.add(egui::TextEdit::singleline(&mut m).hint_text("padrão da conta (ex: sonnet, opus)").desired_width(220.0)).lost_focus() {
                        app.cfg.anakin.model = (!m.trim().is_empty()).then(|| m.trim().to_string());
                        save = true;
                    }
                });
                ui.horizontal(|ui| {
                    let ctx = ui.ctx().clone();
                    if ui.button(format!("{I_RESTART} Reiniciar")).on_hover_text(app.keymap.tip(Action::RestartAnakin, "Reabre o processo; a conversa continua")).clicked() {
                        restart(app, &ctx);
                    }
                    if ui.button("Conversa nova").on_hover_text("Esquece a conversa atual e começa outra").clicked() {
                        reset(app, &ctx);
                    }
                });
                ui.label(widgets::subtle(format!("agente {AGENT} · pasta {}", paths::pretty(&dir())), &theme).small());
            });
            ui.end_row();
        }
        if row(1) {
            ui.label("Microfone");
            ui.horizontal(|ui| {
                if app.anakin.mics.is_none() {
                    app.anakin.mics = Some(voice::list_mics());
                }
                let mics = app.anakin.mics.clone().unwrap_or_default();
                let cur = app.cfg.anakin.voice.mic.clone();
                let label = match &cur {
                    None => "padrão do sistema".to_string(),
                    Some(n) => mics.iter().find(|m| &m.name == n).map(|m| m.desc.clone()).unwrap_or_else(|| format!("{n} (desconectado)")),
                };
                egui::ComboBox::from_id_salt("anakin-mic").selected_text(label).width(340.0).show_ui(ui, |ui| {
                    let def = voice::default_mic().and_then(|d| mics.iter().find(|m| m.name == d).map(|m| m.desc.clone())).unwrap_or_default();
                    if ui.selectable_label(cur.is_none(), format!("padrão do sistema  {def}")).clicked() {
                        app.cfg.anakin.voice.mic = None;
                        save = true;
                    }
                    for m in &mics {
                        if ui.selectable_label(cur.as_deref() == Some(&m.name), &m.desc).on_hover_text(&m.name).clicked() {
                            app.cfg.anakin.voice.mic = Some(m.name.clone());
                            save = true;
                        }
                    }
                });
                if widgets::icon_button(ui, I_RESTART, "", "Ler os microfones de novo").clicked() {
                    app.anakin.mics = None;
                }
                let ctx = ui.ctx().clone();
                let testing = app.anakin.recording();
                if ui.button(if testing { format!("{I_STOP} parar e transcrever") } else { format!("{I_MIC} testar") }).on_hover_text("Grava, transcreve e manda pro Anakin").clicked() {
                    toggle_voice(app, &ctx);
                }
            });
            ui.end_row();
        }
        if row(2) {
            ui.label("Transcrição");
            ui.vertical(|ui| {
                let v = &mut app.cfg.anakin.voice;
                ui.horizontal(|ui| {
                    for t in aic_core::Transcriber::ALL {
                        save |= ui.selectable_value(&mut v.transcriber, t, t.label()).changed();
                    }
                });
                match v.transcriber {
                    aic_core::Transcriber::WhisperCpp => {
                        ui.horizontal(|ui| {
                            ui.label("binário");
                            save |= ui.add(egui::TextEdit::singleline(&mut v.whisper_bin).desired_width(160.0)).lost_focus();
                            ui.label("modelo");
                            let mut m = v.whisper_model.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                            if ui.add(egui::TextEdit::singleline(&mut m).hint_text(paths::pretty(&v.model_path())).desired_width(300.0)).lost_focus() {
                                v.whisper_model = (!m.trim().is_empty()).then(|| m.trim().replace('~', &paths::home_dir().to_string_lossy()).into());
                                save = true;
                            }
                        });
                        let ok = v.model_path().exists();
                        ui.label(if ok {
                            widgets::subtle(format!("modelo encontrado: {}", paths::pretty(&v.model_path())), &theme).small()
                        } else {
                            RichText::new(format!("modelo não encontrado em {} — baixe um ggml (ex: ggml-small.bin) do huggingface.co/ggerganov/whisper.cpp", paths::pretty(&v.model_path()))).small().color(theme.warn)
                        });
                    }
                    aic_core::Transcriber::Api => {
                        ui.horizontal(|ui| {
                            if ui.small_button("OpenAI").clicked() {
                                v.api_url = "https://api.openai.com/v1/audio/transcriptions".into();
                                v.api_model = "gpt-4o-mini-transcribe".into();
                                v.api_key_env = "OPENAI_API_KEY".into();
                                save = true;
                            }
                            if ui.small_button("Groq").clicked() {
                                v.api_url = "https://api.groq.com/openai/v1/audio/transcriptions".into();
                                v.api_model = "whisper-large-v3-turbo".into();
                                v.api_key_env = "GROQ_API_KEY".into();
                                save = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("url");
                            save |= ui.add(egui::TextEdit::singleline(&mut v.api_url).desired_width(420.0)).lost_focus();
                        });
                        ui.horizontal(|ui| {
                            ui.label("modelo");
                            save |= ui.add(egui::TextEdit::singleline(&mut v.api_model).desired_width(200.0)).lost_focus();
                            ui.label("chave na variável");
                            save |= ui.add(egui::TextEdit::singleline(&mut v.api_key_env).desired_width(160.0)).lost_focus();
                        });
                        let has_key = std::env::var(&v.api_key_env).is_ok_and(|k| !k.trim().is_empty());
                        ui.label(if has_key {
                            widgets::subtle(format!("{} definida", v.api_key_env), &theme).small()
                        } else {
                            RichText::new(format!("{} não definida: ponha no {} e reabra o AIC", v.api_key_env, paths::pretty(&paths::dotenv_path()))).small().color(theme.warn)
                        });
                    }
                    aic_core::Transcriber::Command => {
                        save |= ui.add(egui::TextEdit::singleline(&mut v.command).hint_text("meu-transcritor {file} --lang {lang}").desired_width(420.0)).lost_focus();
                        ui.label(widgets::subtle("{file} = .wav 16 kHz mono, {lang} = idioma; o que sair no stdout vira a mensagem.", &theme).small());
                    }
                }
                ui.horizontal(|ui| {
                    ui.label("idioma");
                    save |= ui.add(egui::TextEdit::singleline(&mut v.language).hint_text("vazio = detectar").desired_width(60.0)).lost_focus();
                    save |= ui.checkbox(&mut v.auto_send, "mandar sozinho depois de transcrever").changed();
                    ui.label("limite");
                    save |= ui.add(egui::DragValue::new(&mut v.max_seconds).range(10..=1800).suffix(" s")).changed();
                });
            });
            ui.end_row();
        }
    });
    if save {
        let _ = app.cfg.save();
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// A conversa, numa janela flutuante acima do boneco.
pub fn floating(app: &mut App, ctx: &Context) {
    if !app.anakin.open {
        return;
    }
    let screen = ctx.content_rect();
    let w = app.cfg.anakin.width.min(screen.width() - 24.0).max(320.0);
    let h = (screen.height() * 0.62).clamp(240.0, 800.0);
    let bottom = screen.bottom() - crate::ui::avatar::HEIGHT - 16.0;
    let pos = egui::pos2(screen.right() - 12.0 - w, bottom - h);
    egui::Area::new(egui::Id::new("anakin-conversa"))
        .order(egui::Order::Middle)
        .fixed_pos(pos)
        .constrain(false)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(app.theme.bg2)
                .inner_margin(Margin::same(6))
                .stroke(egui::Stroke::new(1.0, app.theme.accent))
                .show(ui, |ui| {
                    // o tamanho total inclui a moldura
                    let inner = Vec2::new(w - 14.0, h - 14.0);
                    ui.set_min_size(inner);
                    ui.set_max_size(inner);
                    panel(app, ui);
                });
        });
}

/// Ao fechar o app sem manter as sessões, a do Anakin também morre (o tmux é derrubado inteiro);
/// mantendo, ela segue rodando e reconecta na próxima abertura.
pub fn quit(app: &mut App) {
    if let Some(r) = app.anakin.rec.take() {
        r.cancel();
    }
    if let Some(t) = app.anakin.rt.as_ref().and_then(|r| r.term.as_ref()) {
        t.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_do_claude() {
        let a = claude_args(Some(" sonnet "));
        assert_eq!(a[..2], ["--agent".to_string(), AGENT.to_string()]);
        assert!(a.contains(&"Bash(aic:*)".to_string()));
        assert!(a.ends_with(&["--model".to_string(), "sonnet".to_string()]));
        assert!(!claude_args(Some("")).contains(&"--model".to_string()));
    }
}
