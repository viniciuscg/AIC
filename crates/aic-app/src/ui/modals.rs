//! Diálogos. Todos fecham com Esc e funcionam só no teclado.

use chrono::Local;
use egui::{Align, Context, FontId, Key, Layout, RichText, TextEdit, Ui, Vec2};
use aic_core::quickadd::{self, ChipKind};
use aic_core::{Id, SessionKind, Task, TaskStatus, git};

use super::widgets::{self, I_TASK, I_WORKTREE};
use crate::actions::Action;
use crate::app::App;

pub enum Modal {
    QuickAdd { text: String, status: TaskStatus, link: Option<Id>, link_enabled: bool },
    NewSession(NewSessionForm),
    CloseSession { id: Id, remove_worktree: bool, has_worktree: bool },
    Rename { id: Id, text: String, first: bool },
    RenameBlock { id: Id, text: String, first: bool },
    CloseBlock { id: Id, remove_worktrees: bool, has_worktrees: bool },
    Workspaces { query: String, selected: usize },
    RenameWorkspace { text: String, first: bool },
    Palette { query: String, selected: usize },
    Restore,
    ConfirmQuit,
}

pub struct NewSessionForm {
    pub name: String,
    pub kind: usize,
    pub command: String,
    pub repo_query: String,
    pub repo_sel: usize,
    pub worktree: bool,
    pub branch: String,
    pub task: Option<Id>,
    pub perm: usize,
    pub model: usize,
    pub first: bool,
}

const KINDS: [&str; 3] = ["Claude", "Shell", "Comando"];
const PERMS: [(&str, &str); 4] = [("padrão", ""), ("plan", "plan"), ("aceitar edições", "acceptEdits"), ("bypass", "bypassPermissions")];
const MODELS: [(&str, &str); 4] = [("padrão", ""), ("opus", "opus"), ("sonnet", "sonnet"), ("haiku", "haiku")];

impl NewSessionForm {
    pub fn new(app: &App, kind: SessionKind) -> Self {
        // começa no repo da sessão em foco (o mais provável)
        let repo_query = app.focused().and_then(|f| app.sessions.get(&f)).and_then(|r| r.s.repo.clone()).unwrap_or_default();
        Self {
            name: String::new(),
            kind: match kind {
                SessionKind::Claude => 0,
                SessionKind::Shell => 1,
                SessionKind::Command(_) => 2,
            },
            command: String::new(),
            repo_query,
            repo_sel: 0,
            worktree: false,
            branch: String::new(),
            task: None,
            perm: 0,
            model: 0,
            first: true,
        }
    }

    pub fn for_task(app: &App, t: &Task) -> Self {
        let mut f = Self::new(app, SessionKind::Claude);
        f.name = t.card_key.clone().unwrap_or_else(|| widgets::truncate(&t.title, 24));
        f.repo_query = t.repo.clone().unwrap_or_default();
        f.worktree = t.repo.is_some();
        f.branch = git::branch_slug(t.card_key.as_deref(), &t.title);
        f.task = Some(t.id);
        f
    }
}

/// Opções de pasta: repos + raiz da CLI + home.
fn repo_options(app: &App, query: &str) -> Vec<(String, std::path::PathBuf)> {
    let q = query.to_lowercase();
    let mut v: Vec<(String, std::path::PathBuf)> =
        app.cfg.repo_paths();
    v.push(("~ (home)".into(), aic_core::paths::home_dir()));
    if q.is_empty() {
        return v;
    }
    let mut scored: Vec<_> = v
        .into_iter()
        .filter_map(|(n, p)| {
            let l = n.to_lowercase();
            let score = if l == q { 0 } else if l.starts_with(&q) { 1 } else if l.contains(&q) { 2 } else { return None };
            Some((score, n, p))
        })
        .collect();
    scored.sort_by_key(|(s, n, _)| (*s, n.clone()));
    let mut out: Vec<_> = scored.into_iter().map(|(_, n, p)| (n, p)).collect();
    // caminho digitado direto
    let typed = std::path::PathBuf::from(shellexpand(query));
    if typed.is_dir() {
        out.insert(0, (query.to_string(), typed));
    }
    out
}

fn shellexpand(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        format!("{}/{rest}", aic_core::paths::home_dir().display())
    } else {
        s.to_string()
    }
}

fn frame(ctx: &Context, id: &str, width: f32, add: impl FnOnce(&mut Ui)) -> bool {
    let resp = egui::Modal::new(egui::Id::new(id)).show(ctx, |ui| {
        ui.set_width(width);
        add(ui);
    });
    resp.should_close()
}

pub fn show(app: &mut App, ctx: &Context) {
    let Some(modal) = app.modal.take() else { return };
    let esc = ctx.input(|i| i.key_pressed(Key::Escape));
    let keep = match modal {
        Modal::QuickAdd { text, status, link, link_enabled } => quick_add(app, ctx, text, status, link, link_enabled),
        Modal::NewSession(f) => new_session(app, ctx, f),
        Modal::CloseSession { id, remove_worktree, has_worktree } => close_session(app, ctx, id, remove_worktree, has_worktree),
        Modal::Rename { id, text, first } => rename(app, ctx, id, text, first),
        Modal::RenameBlock { id, text, first } => rename_block(app, ctx, id, text, first),
        Modal::CloseBlock { id, remove_worktrees, has_worktrees } => close_block(app, ctx, id, remove_worktrees, has_worktrees),
        Modal::Workspaces { query, selected } => workspaces(app, ctx, query, selected),
        Modal::RenameWorkspace { text, first } => rename_workspace(app, ctx, text, first),
        Modal::Palette { query, selected } => palette(app, ctx, query, selected),
        Modal::Restore => restore(app, ctx),
        Modal::ConfirmQuit => confirm_quit(app, ctx),
    };
    // Esc fecha qualquer diálogo (menos o de reabrir, que precisa de resposta)
    match keep {
        Some(m) if esc && !matches!(m, Modal::Restore) => {
            if app.modal.is_none() {
                app.modal = None;
            }
        }
        Some(m) => {
            if app.modal.is_none() {
                app.modal = Some(m);
            }
        }
        None => {}
    }
}

/// Botões de confirmar/cancelar pros diálogos que só tinham teclado.
fn ok_cancel(ui: &mut Ui, ok: &str, done: &mut bool, cancel: &mut bool) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui.button(ok).clicked() {
            *done = true;
        }
        if ui.button("Cancelar  (Esc)").clicked() {
            *cancel = true;
        }
    });
}

fn list_nav(ui: &Ui, selected: &mut usize, len: usize) -> bool {
    let (down, up, enter) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, Key::ArrowDown) || i.consume_key(egui::Modifiers::CTRL, Key::N),
            i.consume_key(egui::Modifiers::NONE, Key::ArrowUp) || i.consume_key(egui::Modifiers::CTRL, Key::P),
            i.key_pressed(Key::Enter),
        )
    });
    if len > 0 {
        if down {
            *selected = (*selected + 1) % len;
        }
        if up {
            *selected = (*selected + len - 1) % len;
        }
        *selected = (*selected).min(len - 1);
    }
    enter
}

// ───────────────────────── captura rápida ─────────────────────────

fn quick_add(app: &mut App, ctx: &Context, mut text: String, mut status: TaskStatus, link: Option<Id>, mut link_enabled: bool) -> Option<Modal> {
    let theme = app.theme.clone();
    let mut submit = false;
    let mut close = false;
    let ws_names: Vec<String> = app.workspaces.iter().map(|w| w.name.clone()).collect();
    let parsed = quickadd::parse(&text, Local::now().date_naive(), &app.repos, &ws_names);
    let closed = frame(ctx, "quick-add", 620.0, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{I_TASK}  Nova tarefa")).font(FontId::monospace(16.0)).color(theme.accent_hi));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for s in TaskStatus::ALL.iter().rev() {
                    if ui.selectable_label(status == *s, s.label()).clicked() {
                        status = *s;
                    }
                }
            });
        });
        ui.add_space(6.0);
        let edit = ui.add(
            TextEdit::singleline(&mut text)
                .hint_text("o que precisa ser feito?  #repo @hoje !alta +workspace ^DEV-123 ~tag")
                .font(FontId::proportional(17.0))
                .desired_width(f32::INFINITY)
                .margin(Vec2::new(10.0, 10.0)),
        );
        edit.request_focus();
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            submit = true;
        }
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if parsed.chips.is_empty() {
                ui.label(widgets::subtle("Enter cria · Esc cancela · marcadores são opcionais", &theme).small());
            }
            for c in &parsed.chips {
                let (icon, color) = match c.kind {
                    ChipKind::Repo => ("", theme.accent),
                    ChipKind::Due => ("󰃭", theme.warn),
                    ChipKind::Priority => ("󰈻", theme.err),
                    ChipKind::Workspace => ("󰕰", theme.attention),
                    ChipKind::Card => ("󰈙", theme.accent_hi),
                    ChipKind::Tag => ("󰓹", theme.dim),
                    ChipKind::Unknown => ("?", theme.faint),
                };
                widgets::chip(ui, format!("{icon} {}", c.label), color, &theme);
            }
        });
        if let Some(sid) = link {
            ui.add_space(4.0);
            ui.checkbox(&mut link_enabled, format!("vincular à sessão \"{}\"", app.session_label(sid)));
        }
        if ui.input(|i| i.modifiers.alt && i.key_pressed(Key::L)) {
            link_enabled = !link_enabled;
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Criar  ⏎").clicked() {
                submit = true;
            }
            if ui.button("Cancelar").clicked() {
                close = true;
            }
        });
    });
    if submit && !text.trim().is_empty() {
        let l = if link_enabled { link } else { None };
        app.quick_add(&text, status, l);
        return None;
    }
    if closed || close {
        return None;
    }
    Some(Modal::QuickAdd { text, status, link, link_enabled })
}

// ───────────────────────── nova sessão ─────────────────────────

fn new_session(app: &mut App, ctx: &Context, mut f: NewSessionForm) -> Option<Modal> {
    let theme = app.theme.clone();
    let options = repo_options(app, &f.repo_query);
    let mut create = false;
    let mut cancel = false;
    let closed = frame(ctx, "new-session", 640.0, |ui| {
        ui.label(RichText::new(format!("{}  Nova sessão", widgets::I_TERM)).font(FontId::monospace(16.0)).color(theme.accent_hi));
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            for (i, k) in KINDS.iter().enumerate() {
                if ui.selectable_label(f.kind == i, *k).clicked() {
                    f.kind = i;
                }
            }
            ui.label(widgets::subtle("(Alt+1/2/3)", &theme).small());
        });
        for (i, key) in [Key::Num1, Key::Num2, Key::Num3].iter().enumerate() {
            if ui.input_mut(|inp| inp.consume_key(egui::Modifiers::ALT, *key)) {
                f.kind = i;
            }
        }
        ui.add_space(4.0);
        egui::Grid::new("ns").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Pasta / repo");
            let r = ui.add(TextEdit::singleline(&mut f.repo_query).hint_text("digite pra filtrar (ex: wal) ou um caminho").desired_width(420.0));
            if f.first {
                r.request_focus();
                f.first = false;
            }
            if r.changed() {
                f.repo_sel = 0;
            }
            ui.end_row();

            ui.label("");
            ui.vertical(|ui| {
                egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                    for (i, (name, path)) in options.iter().enumerate().take(40) {
                        let sel = i == f.repo_sel;
                        let resp = ui.selectable_label(sel, RichText::new(name).color(if sel { theme.accent_hi } else { theme.ink }));
                        if sel {
                            resp.scroll_to_me(None);
                        }
                        if resp.on_hover_text(aic_core::paths::pretty(path)).clicked() {
                            f.repo_sel = i;
                        }
                    }
                });
            });
            ui.end_row();

            if f.kind == 2 {
                ui.label("Comando");
                ui.add(TextEdit::singleline(&mut f.command).hint_text("npm run dev").desired_width(420.0));
                ui.end_row();
            }
            ui.label("Nome");
            ui.add(TextEdit::singleline(&mut f.name).hint_text("(automático)").desired_width(420.0));
            ui.end_row();

            ui.label(format!("{I_WORKTREE} Worktree"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.worktree, "isolar num worktree (Alt+W)");
                if f.worktree {
                    ui.add(TextEdit::singleline(&mut f.branch).hint_text("branch (ex: DEV-123-ajuste)").desired_width(260.0));
                }
            });
            ui.end_row();

            if f.kind == 0 {
                ui.label("Permissões");
                ui.horizontal(|ui| {
                    for (i, (l, _)) in PERMS.iter().enumerate() {
                        if ui.selectable_label(f.perm == i, *l).clicked() {
                            f.perm = i;
                        }
                    }
                });
                ui.end_row();
                ui.label("Modelo");
                ui.horizontal(|ui| {
                    for (i, (l, _)) in MODELS.iter().enumerate() {
                        if ui.selectable_label(f.model == i, *l).clicked() {
                            f.model = i;
                        }
                    }
                });
                ui.end_row();
            }

            ui.label("Tarefa");
            let label = f.task.and_then(|id| app.tasks.iter().find(|t| t.id == id)).map(|t| t.title.clone()).unwrap_or_else(|| "nenhuma".into());
            egui::ComboBox::from_id_salt("ns-task").selected_text(widgets::truncate(&label, 50)).width(420.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut f.task, None, "nenhuma");
                for t in app.tasks.iter().filter(|t| !matches!(t.status, TaskStatus::Done | TaskStatus::Archived)) {
                    ui.selectable_value(&mut f.task, Some(t.id), widgets::truncate(&t.title, 60));
                }
            });
            ui.end_row();
        });
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::ALT, Key::W)) {
            f.worktree = !f.worktree;
        }
        let enter = list_nav(ui, &mut f.repo_sel, options.len().min(40));
        if enter {
            create = true;
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Abrir  ⏎").clicked() {
                create = true;
            }
            if ui.button("Cancelar").clicked() {
                cancel = true;
            }
            ui.label(widgets::subtle("↑↓ escolhe a pasta · Enter abre", &theme).small());
        });
    });
    if create {
        let Some((repo_label, mut cwd)) = options.get(f.repo_sel).cloned() else {
            app.toast("Escolha uma pasta", crate::app::ToastKind::Warn);
            return Some(Modal::NewSession(f));
        };
        let mut worktree = None;
        if f.worktree {
            let branch = if f.branch.trim().is_empty() { format!("aic-{}", &uuid::Uuid::new_v4().to_string()[..6]) } else { f.branch.trim().to_string() };
            match git::main_root(&cwd).map(|root| git::add_worktree(&root, &branch, None)) {
                Some(Ok(p)) => {
                    cwd = p.clone();
                    worktree = Some(p);
                }
                Some(Err(e)) => {
                    app.toast(&format!("Worktree falhou: {e}"), crate::app::ToastKind::Err);
                    return Some(Modal::NewSession(f));
                }
                None => {
                    app.toast("Essa pasta não é um repositório git", crate::app::ToastKind::Warn);
                    return Some(Modal::NewSession(f));
                }
            }
        }
        let kind = match f.kind {
            0 => SessionKind::Claude,
            1 => SessionKind::Shell,
            _ => SessionKind::Command(f.command.clone()),
        };
        let name = if f.name.trim().is_empty() {
            match &worktree {
                Some(_) => f.branch.clone(),
                None => repo_label.trim_end_matches(" (CLI)").trim_end_matches(" (home)").to_string(),
            }
        } else {
            f.name.trim().to_string()
        };
        let mut args = Vec::new();
        if f.kind == 0 {
            if !PERMS[f.perm].1.is_empty() {
                args.extend(["--permission-mode".to_string(), PERMS[f.perm].1.to_string()]);
            }
            if !MODELS[f.model].1.is_empty() {
                args.extend(["--model".to_string(), MODELS[f.model].1.to_string()]);
            }
        }
        app.create_session(ctx, name, kind, cwd, worktree, f.task, args);
        return None;
    }
    if closed || cancel {
        return None;
    }
    Some(Modal::NewSession(f))
}

// ───────────────────────── fechar / renomear ─────────────────────────

fn close_session(app: &mut App, ctx: &Context, id: Id, mut remove_wt: bool, has_wt: bool) -> Option<Modal> {
    let theme = app.theme.clone();
    let name = app.session_label(id);
    let mut done = None;
    let closed = frame(ctx, "close-session", 440.0, |ui| {
        ui.label(RichText::new(format!("Fechar \"{name}\"?")).size(16.0));
        ui.label(widgets::subtle("O processo é encerrado. A conversa do Claude fica salva (dá pra retomar com /resume).", &theme));
        if has_wt {
            ui.add_space(6.0);
            ui.checkbox(&mut remove_wt, format!("{I_WORKTREE} remover o worktree também (a branch continua)"));
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("Fechar  ⏎").color(theme.err)).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                done = Some(true);
            }
            if ui.button("Cancelar").clicked() {
                done = Some(false);
            }
        });
    });
    match done {
        Some(true) => {
            app.kill_session(id, remove_wt);
            None
        }
        Some(false) => None,
        None if closed => None,
        None => Some(Modal::CloseSession { id, remove_worktree: remove_wt, has_worktree: has_wt }),
    }
}

fn rename(app: &mut App, ctx: &Context, id: Id, mut text: String, first: bool) -> Option<Modal> {
    let mut done = false;
    let mut cancel = false;
    let closed = frame(ctx, "rename", 380.0, |ui| {
        ui.label("Renomear sessão");
        let r = ui.add(TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
        if first {
            r.request_focus();
            if let Some(mut state) = TextEdit::load_state(ui.ctx(), r.id) {
                let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
                state.cursor.set_char_range(Some(range));
                state.store(ui.ctx(), r.id);
            }
        }
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            done = true;
        }
        ok_cancel(ui, "Salvar  ⏎", &mut done, &mut cancel);
    });
    if done {
        app.rename_session(id, &text);
        return None;
    }
    (!closed && !cancel).then_some(Modal::Rename { id, text, first: false })
}

fn rename_workspace(app: &mut App, ctx: &Context, mut text: String, first: bool) -> Option<Modal> {
    let mut done = false;
    let mut cancel = false;
    let closed = frame(ctx, "rename-workspace", 380.0, |ui| {
        ui.label("Renomear workspace");
        let r = ui.add(TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
        if first {
            r.request_focus();
            if let Some(mut state) = TextEdit::load_state(ui.ctx(), r.id) {
                let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
                state.cursor.set_char_range(Some(range));
                state.store(ui.ctx(), r.id);
            }
        }
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            done = true;
        }
        ok_cancel(ui, "Salvar  ⏎", &mut done, &mut cancel);
    });
    if done {
        app.rename_workspace(&text);
        return None;
    }
    (!closed && !cancel).then_some(Modal::RenameWorkspace { text, first: false })
}

fn rename_block(app: &mut App, ctx: &Context, id: Id, mut text: String, first: bool) -> Option<Modal> {
    let mut done = false;
    let mut cancel = false;
    let closed = frame(ctx, "rename-block", 420.0, |ui| {
        ui.label("Nome do bloco (ex: o card ou o assunto)");
        let r = ui.add(TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
        if first {
            r.request_focus();
            if let Some(mut state) = TextEdit::load_state(ui.ctx(), r.id) {
                let range = egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(text.chars().count()));
                state.cursor.set_char_range(Some(range));
                state.store(ui.ctx(), r.id);
            }
        }
        if ui.input(|i| i.key_pressed(Key::Enter)) {
            done = true;
        }
        ui.label(super::widgets::subtle("Enter salva · Esc cancela", &app.theme).small());
        ok_cancel(ui, "Salvar  ⏎", &mut done, &mut cancel);
    });
    if done {
        let name = text.trim().to_string();
        if !name.is_empty() {
            app.update_block(id, |b| b.name = name);
        }
        return None;
    }
    (!closed && !cancel).then_some(Modal::RenameBlock { id, text, first: false })
}

fn close_block(app: &mut App, ctx: &Context, id: Id, mut remove_wt: bool, has_wt: bool) -> Option<Modal> {
    let theme = app.theme.clone();
    let (name, n) = app.blocks.get(&id).map(|b| (b.name.clone(), b.panes.len())).unwrap_or_default();
    let mut done = None;
    let closed = frame(ctx, "close-block", 460.0, |ui| {
        ui.label(RichText::new(format!("Fechar o bloco \"{name}\"?")).size(16.0));
        ui.label(widgets::subtle(format!("{n} terminal(is) serão encerrados. As conversas do Claude ficam salvas; notas e tarefas também."), &theme));
        if has_wt {
            ui.checkbox(&mut remove_wt, format!("{I_WORKTREE} remover os worktrees também (as branches continuam)"));
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(RichText::new("Fechar  ⏎").color(theme.err)).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                done = Some(true);
            }
            if ui.button("Cancelar  (Esc)").clicked() {
                done = Some(false);
            }
        });
    });
    match done {
        Some(true) => {
            app.kill_block(id, remove_wt);
            None
        }
        Some(false) => None,
        None if closed => None,
        None => Some(Modal::CloseBlock { id, remove_worktrees: remove_wt, has_worktrees: has_wt }),
    }
}

// ───────────────────────── workspaces ─────────────────────────

fn workspaces(app: &mut App, ctx: &Context, mut query: String, mut selected: usize) -> Option<Modal> {
    let theme = app.theme.clone();
    let q = query.to_lowercase();
    let list: Vec<(usize, String, usize)> = app
        .workspaces
        .iter()
        .enumerate()
        .filter(|(_, w)| q.is_empty() || w.name.to_lowercase().contains(&q))
        .map(|(i, w)| (i, w.name.clone(), w.sessions.len()))
        .collect();
    if selected == usize::MAX {
        selected = list.len();
    }
    let mut pick = None;
    let mut create = false;
    let mut delete = None;
    let mut close = false;
    let closed = frame(ctx, "workspaces", 480.0, |ui| {
        ui.label(RichText::new(format!("{}  Workspaces", widgets::I_WS)).font(FontId::monospace(16.0)).color(theme.accent_hi));
        ui.label(widgets::subtle("Um workspace agrupa sessões, tarefas e layout. Trocar não mata processos.", &theme).small());
        let r = ui.add(TextEdit::singleline(&mut query).hint_text("buscar ou nome do novo workspace").desired_width(f32::INFINITY));
        r.request_focus();
        let enter = list_nav(ui, &mut selected, list.len() + 1);
        for (k, (i, name, n)) in list.iter().enumerate() {
            ui.horizontal(|ui| {
                let current = *i == app.ws;
                let resp = ui.selectable_label(k == selected, format!("{}{name}  ·  {n} sessões", if current { "▸ " } else { "  " }));
                if resp.clicked() {
                    pick = Some(*i);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if app.workspaces.len() > 1 && widgets::icon_button(ui, "🗑", &format!("Apagar workspace {name}"), "Apagar (fecha as sessões dele)").clicked() {
                        delete = Some(*i);
                    }
                });
            });
        }
        let new_label = if query.trim().is_empty() { "+ novo workspace".to_string() } else { format!("+ criar \"{}\"", query.trim()) };
        if ui.selectable_label(selected == list.len(), new_label).clicked() {
            create = true;
        }
        if ui.button("Fechar  (Esc)").clicked() {
            close = true;
        }
        if enter {
            match list.get(selected) {
                Some((i, _, _)) => pick = Some(*i),
                None => create = true,
            }
        }
    });
    if let Some(i) = delete {
        app.delete_workspace(i);
        return Some(Modal::Workspaces { query, selected: 0 });
    }
    if let Some(i) = pick {
        app.switch_ws(i);
        return None;
    }
    if create {
        app.new_workspace(&query);
        return None;
    }
    (!closed && !close).then_some(Modal::Workspaces { query, selected })
}

// ───────────────────────── paleta ─────────────────────────

enum PaletteItem {
    Action(Action),
    Session(Id),
    Task(Id),
}

fn palette(app: &mut App, ctx: &Context, mut query: String, mut selected: usize) -> Option<Modal> {
    let theme = app.theme.clone();
    let q = query.to_lowercase();
    let matches = |s: &str| q.split_whitespace().all(|w| s.to_lowercase().contains(w));
    let mut items: Vec<(String, String, PaletteItem)> = Vec::new();
    for a in Action::all().into_iter().filter(|a| a.enabled(&app.cfg)) {
        let m = a.meta();
        if matches(m.name) || matches(m.group) {
            items.push((format!("{} › {}", m.group, m.name), app.keymap.label(a), PaletteItem::Action(a)));
        }
    }
    // só as sessões do workspace atual: as de outro aparecem quando você troca pra ele
    for id in app.ws_sessions() {
        if let Some(rt) = app.sessions.get(&id) {
            let label = format!("Sessão › {}", rt.s.name);
            if matches(&label) {
                items.push((label, crate::brand::state_label(rt.state()).to_string(), PaletteItem::Session(id)));
            }
        }
    }
    for t in app.tasks.iter().filter(|t| !matches!(t.status, TaskStatus::Archived)) {
        let label = format!("Tarefa › {}{}", t.card_key.as_ref().map(|k| format!("{k} · ")).unwrap_or_default(), t.title);
        if !q.is_empty() && matches(&label) {
            items.push((label, t.status.label().to_string(), PaletteItem::Task(t.id)));
        }
    }
    let mut run = None;
    let mut close = false;
    let closed = frame(ctx, "palette", 600.0, |ui| {
        let r = ui.add(
            TextEdit::singleline(&mut query)
                .hint_text("ação, sessão ou tarefa…")
                .font(FontId::proportional(16.0))
                .desired_width(f32::INFINITY)
                .margin(Vec2::new(8.0, 8.0)),
        );
        r.request_focus();
        let enter = list_nav(ui, &mut selected, items.len());
        egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
            for (i, (label, hint, _)) in items.iter().enumerate() {
                let sel = i == selected;
                let resp = ui.horizontal(|ui| {
                    let r = ui.selectable_label(sel, label);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(hint).small().color(theme.dim));
                    });
                    r
                });
                if sel {
                    resp.inner.scroll_to_me(None);
                }
                if resp.inner.clicked() {
                    run = Some(i);
                }
            }
        });
        if enter {
            run = Some(selected);
        }
        if ui.button("Fechar  (Esc)").clicked() {
            close = true;
        }
    });
    if let Some(i) = run {
        if let Some((_, _, item)) = items.into_iter().nth(i) {
            match item {
                PaletteItem::Action(a) => app.run_action(ctx, a),
                PaletteItem::Session(id) => app.focus(id),
                PaletteItem::Task(id) => app.open_task(id),
            }
        }
        return app.modal.take();
    }
    (!closed && !close).then_some(Modal::Palette { query, selected })
}

// ───────────────────────── cache ─────────────────────────

fn restore(app: &mut App, ctx: &Context) -> Option<Modal> {
    let theme = app.theme.clone();
    let n = app.store.sessions().map(|s| s.len()).unwrap_or(0);
    let ws = app.store.workspaces().map(|w| w.len()).unwrap_or(0);
    let mut choice = None;
    egui::Modal::new(egui::Id::new("restore")).show(ctx, |ui| {
        ui.set_width(520.0);
        ui.label(RichText::new("Reabrir a última sessão?").size(18.0).strong());
        ui.label(widgets::subtle(format!("{n} sessões em {ws} workspace(s) ficaram salvas."), &theme));
        ui.add_space(4.0);
        ui.label(RichText::new("Se não reabrir, uma sessão nova começa e a antiga será perdida.").color(theme.warn));
        ui.label(widgets::subtle("(as tarefas nunca se perdem, só as sessões, as notas delas e o layout)", &theme).small());
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Reabrir  ⏎").strong()).fill(theme.accent_lo)).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                choice = Some(true);
            }
            if ui.button("Começar nova  (N)").clicked() || ui.input(|i| i.key_pressed(Key::N)) {
                choice = Some(false);
            }
        });
    });
    match choice {
        Some(true) => {
            app.restore(ctx);
            None
        }
        Some(false) => {
            app.start_fresh(ctx);
            None
        }
        None => Some(Modal::Restore),
    }
}

fn confirm_quit(app: &mut App, ctx: &Context) -> Option<Modal> {
    let theme = app.theme.clone();
    let mut choice: Option<u8> = None;
    let closed = frame(ctx, "quit", 560.0, |ui| {
        ui.label(RichText::new("Tem certeza que quer fechar?").size(18.0).strong());
        ui.label(widgets::subtle(format!("{} sessões abertas serão encerradas.", app.sessions.len()), &theme));
        ui.label(widgets::subtle("Salvando o cache, da próxima vez o AIC pergunta se quer reabrir tudo (as conversas do Claude voltam com --resume).", &theme).small());
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if ui.add(egui::Button::new(RichText::new("Sim e salvar cache  (S)").strong()).fill(theme.accent_lo)).clicked() || ui.input(|i| i.key_pressed(Key::S) || i.key_pressed(Key::Enter)) {
                choice = Some(2);
            }
            if ui.button("Sim e não salvar cache  (D)").clicked() || ui.input(|i| i.key_pressed(Key::D)) {
                choice = Some(1);
            }
            if ui.button("Não  (Esc)").clicked() {
                choice = Some(0);
            }
        });
    });
    match choice {
        Some(2) => {
            app.quit(ctx, true);
            None
        }
        Some(1) => {
            app.quit(ctx, false);
            None
        }
        Some(_) => None,
        None if closed => None,
        None => Some(Modal::ConfirmQuit),
    }
}
