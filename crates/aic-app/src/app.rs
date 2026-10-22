//! Estado do app e tudo que não é desenho: sessões, tarefas, eventos, ações, cache.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::Local;
use egui::Context;
use aic_core::claude::{Injection, StatusSnapshot};
use aic_core::events::{Effect, EventTail};
use aic_core::{Block, Config, Id, Layout, Module, Session, SessionKind, SessionState, SessionView, Store, Task, TaskStatus, Workspace, git, paths};
use aic_term::{BindingsLayout, TerminalFont};

use crate::actions::{Action, Keymap};
use crate::injection;
use crate::notify;
use crate::session::Runtime;
use crate::theme::Theme;
use crate::ui;

/// Abas da barra superior, na ordem em que aparecem (e em que `Ctrl+B n/p` anda).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Sessions,
    Board,
    Worktrees,
    Changes,
    Usage,
    Settings,
    Help,
}

impl View {
    pub const ALL: [View; 7] = [View::Sessions, View::Board, View::Worktrees, View::Changes, View::Usage, View::Settings, View::Help];

    pub fn label(self) -> &'static str {
        match self {
            View::Sessions => "Sessões",
            View::Board => "Board",
            View::Worktrees => "Worktrees",
            View::Changes => "Changes",
            View::Usage => "Tokens",
            View::Settings => "Config",
            View::Help => "Ajuda",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            View::Sessions => ui::widgets::I_TERM,
            View::Board => ui::widgets::I_BOARD,
            View::Worktrees => ui::widgets::I_WORKTREE,
            View::Changes => ui::widgets::I_DIFF,
            View::Usage => ui::widgets::I_CHART,
            View::Settings => ui::widgets::I_GEAR,
            View::Help => ui::widgets::I_HELP,
        }
    }

    /// Ação que abre a aba (a tecla de prefixo dela aparece na barra).
    pub fn action(self) -> Action {
        match self {
            View::Sessions => Action::ViewSessions,
            View::Board => Action::ViewBoard,
            View::Worktrees => Action::ViewWorktrees,
            View::Changes => Action::ViewChanges,
            View::Usage => Action::ViewUsage,
            View::Settings => Action::ViewSettings,
            View::Help => Action::ViewHelp,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Ok,
    Warn,
    Err,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub until: Instant,
}

pub enum Phase {
    /// Assistente de primeiro uso (visualização, módulos, pastas, atalhos).
    Setup(ui::setup::SetupState),
    Main,
}

pub struct App {
    pub cfg: Config,
    pub store: Store,
    pub theme: Theme,
    pub font: TerminalFont,
    pub term_bindings: BindingsLayout,
    pub keymap: Keymap,
    pub inj: Injection,

    pub workspaces: Vec<Workspace>,
    pub ws: usize,
    /// Workspace de antes da última troca (`Ctrl+B Tab` volta pra ele).
    pub last_ws: Option<Id>,
    pub sessions: HashMap<Id, Runtime>,
    pub blocks: HashMap<Id, Block>,
    /// Bloco vazio selecionado (sem terminal pra focar).
    pub empty_block_focus: Option<Id>,
    /// Pedido de foco nas notas de um bloco (atalho).
    pub focus_notes: Option<Id>,
    pub tasks: Vec<Task>,
    pub repos: Vec<String>,

    pub view: View,
    pub modal: Option<ui::modals::Modal>,
    pub toasts: Vec<Toast>,
    pub phase: Phase,

    /// Teclado vai pro terminal da sessão focada.
    pub term_focus: bool,
    pub maximized: bool,
    pub sidebar: bool,
    pub window_focused: bool,

    pub board: ui::board::BoardState,
    pub worktrees: ui::worktrees::WorktreeState,
    pub changes: ui::changes::ChangesState,
    pub settings: ui::settings::SettingsState,
    /// Menu da tecla líder aberto (caminho dentro da árvore de atalhos).
    pub leader: Option<ui::leader::LeaderState>,
    /// Prefixo (`Ctrl+B`) apertado, esperando a tecla da ação.
    pub prefix_pending: bool,
    pub usage: ui::usage::UsageState,
    /// Sync com o Notion em background (ver `notion_ui`).
    pub notion: crate::notion_ui::NotionState,
    /// Buscas de card (Abrir card / Buscar no Notion) em background (ver `connector`).
    pub connector: crate::connector::ConnectorState,
    /// Histórico de tokens/custo lido dos transcripts numa thread.
    pub usage_history: crate::usage_history::UsageHistory,
    /// O assistente (painel à direita, sessão sempre aberta).
    pub layne: crate::layne::LayneState,

    pub account: Option<StatusSnapshot>,
    pub md_cache: egui_commonmark::CommonMarkCache,
    account_at: Option<Instant>,
    tail: EventTail,
    db_version: i64,
    db_checked: Instant,
    events_checked: Instant,
    next_term_id: u64,
    pub start: Instant,
    fonts_dirty: bool,
    pub quitting: bool,
    pending_restore: bool,
}

impl App {
    pub fn new(ctx: &Context, fresh: bool) -> anyhow::Result<Self> {
        paths::ensure_dirs()?;
        egui_extras::install_image_loaders(ctx);
        let mut cfg = Config::load();
        if cfg.user_statusline.is_none() {
            cfg.user_statusline = injection::user_statusline_from_claude_settings();
        }
        let store = Store::open(&paths::db_path())?;

        let needs_setup = !cfg.setup_done;
        let theme = crate::brand::theme();
        theme.apply(ctx, cfg.ui_scale);
        let font = aic_term::fonts::install(ctx, &cfg.font_family, None, cfg.font_size);

        let mut tail = EventTail::new();
        tail.skip_existing();

        let had_cache = store.has_cache();
        if fresh && had_cache {
            store.clear_cache()?;
        }

        let mut app = Self {
            repos: cfg.repos(),
            cfg,
            store,
            theme,
            font,
            term_bindings: BindingsLayout::new(),
            keymap: Keymap::load(),
            inj: {
                crate::tmux::prepare();
                injection::prepare()
            },
            workspaces: vec![],
            ws: 0,
            last_ws: None,
            sessions: HashMap::new(),
            blocks: HashMap::new(),
            empty_block_focus: None,
            focus_notes: None,
            tasks: vec![],
            view: View::Sessions,
            modal: None,
            toasts: vec![],
            phase: if needs_setup { Phase::Setup(Default::default()) } else { Phase::Main },
            term_focus: true,
            maximized: false,
            sidebar: true,
            window_focused: true,
            board: Default::default(),
            worktrees: Default::default(),
            changes: Default::default(),
            settings: Default::default(),
            leader: None,
            prefix_pending: false,
            usage: Default::default(),
            notion: Default::default(),
            connector: Default::default(),
            usage_history: Default::default(),
            layne: Default::default(),
            account: StatusSnapshot::latest(),
            md_cache: Default::default(),
            account_at: None,
            tail,
            db_version: 0,
            db_checked: Instant::now(),
            events_checked: Instant::now(),
            next_term_id: 1,
            start: Instant::now(),
            fonts_dirty: false,
            quitting: false,
            pending_restore: had_cache && !fresh,
        };
        app.reload_tasks();
        aic_core::inbox::mark_running();
        if !needs_setup {
            app.enter_main(ctx);
        }
        Ok(app)
    }

    /// Sai da splash: pergunta sobre o cache ou começa limpo.
    pub fn enter_main(&mut self, ctx: &Context) {
        self.phase = Phase::Main;
        if self.pending_restore {
            self.pending_restore = false;
            self.modal = Some(ui::modals::Modal::Restore);
        } else {
            self.start_fresh(ctx);
        }
    }

    pub fn start_fresh(&mut self, _ctx: &Context) {
        let _ = self.store.clear_cache();
        // sessões que sobraram rodando no tmux de uma vez anterior (sem o módulo, o servidor
        // não é nosso: os testes rodam com tmux desligado e não podem derrubar o AIC aberto)
        if self.cfg.on(aic_core::Module::Tmux) {
            crate::tmux::kill_all();
        }
        self.blocks.clear();
        let ws = Workspace::new("Principal");
        let _ = self.store.save_workspace(&ws);
        self.workspaces = vec![ws];
        self.ws = 0;
        let _ = self.store.mark_cache(true);
        let prefix = self.cfg.prefix.clone();
        self.toast(&format!("Sessão nova. {prefix} c abre um Claude, {prefix} ? mostra os atalhos."), ToastKind::Info);
    }

    /// Reabre workspaces e sessões do cache (`claude --resume`).
    pub fn restore(&mut self, ctx: &Context) {
        self.workspaces = self.store.workspaces().unwrap_or_default();
        if self.workspaces.is_empty() {
            return self.start_fresh(ctx);
        }
        self.blocks = self.store.blocks().unwrap_or_default().into_iter().map(|b| (b.id, b)).collect();
        let saved = self.store.sessions().unwrap_or_default();
        let (mut count, mut alive) = (0, 0);
        for s in saved {
            let id = s.id;
            let mut rt = Runtime::new(s);
            rt.restored = self.store.scrollback(id);
            self.spawn(ctx, &mut rt);
            if rt.reattached {
                alive += 1;
            }
            self.sessions.insert(id, rt);
            count += 1;
        }
        // sessões órfãs (workspace apagado) somem; ids sem runtime saem da ordem
        for w in &mut self.workspaces {
            w.sessions.retain(|id| self.sessions.contains_key(id));
        }
        self.fix_blocks();
        self.ws = 0;
        let _ = self.store.mark_cache(true);
        if alive > 0 {
            self.toast(&format!("{alive} de {count} sessões continuavam rodando (tmux) e foram reconectadas"), ToastKind::Ok);
        } else {
            self.toast(&format!("{count} sessões reabertas"), ToastKind::Ok);
        }
    }

    /// Id do próximo terminal (cada `TerminalBackend` tem o seu).
    pub fn alloc_term_id(&mut self) -> u64 {
        let id = self.next_term_id;
        self.next_term_id += 1;
        id
    }

    fn spawn(&mut self, ctx: &Context, rt: &mut Runtime) {
        let task = rt.s.task.and_then(|id| self.tasks.iter().find(|t| t.id == id)).cloned();
        let id = self.next_term_id;
        self.next_term_id += 1;
        rt.spawn(ctx, id, &self.inj, &self.theme.term, task.as_ref(), self.cfg.on(Module::Tmux));
        // prompt inicial só vale na criação
        if rt.s.initial_prompt.take().is_some() {
            let _ = self.store.save_session(&rt.s);
        }
    }

    // ───────────────────────── blocos ─────────────────────────

    /// Blocos do workspace atual, em ordem.
    pub fn ws_blocks(&self) -> Vec<Id> {
        self.current_ws().map(|w| w.blocks.iter().copied().filter(|b| self.blocks.contains_key(b)).collect()).unwrap_or_default()
    }

    pub fn block_of(&self, session: Id) -> Option<Id> {
        self.sessions.get(&session).and_then(|r| r.s.block)
    }

    /// Bloco em foco: o da sessão em foco, ou o primeiro.
    pub fn current_block(&self) -> Option<Id> {
        let ws = self.ws_blocks();
        self.empty_block_focus
            .filter(|b| ws.contains(b))
            .or_else(|| self.focused().and_then(|s| self.block_of(s)))
            .or_else(|| ws.first().copied())
    }

    pub fn new_block(&mut self, name: &str, task: Option<Id>) -> Option<Id> {
        let ws = self.current_ws()?.id;
        let mut b = Block::new(ws, if name.trim().is_empty() { "Novo bloco" } else { name.trim() });
        b.task = task;
        b.color = (self.blocks.len() % self.theme.agents.len().max(1)) as u8;
        let id = b.id;
        let _ = self.store.save_block(&b);
        self.blocks.insert(id, b);
        if let Some(w) = self.current_ws_mut() {
            w.blocks.push(id);
        }
        self.persist_ws();
        Some(id)
    }

    pub fn save_block(&mut self, id: Id) {
        if let Some(b) = self.blocks.get(&id) {
            let _ = self.store.save_block(b);
        }
    }

    pub fn update_block(&mut self, id: Id, f: impl FnOnce(&mut Block)) {
        if let Some(b) = self.blocks.get_mut(&id) {
            f(b);
        }
        self.save_block(id);
    }

    /// Fecha o bloco inteiro (todos os terminais).
    pub fn kill_block(&mut self, id: Id, remove_worktrees: bool) {
        let panes = self.blocks.get(&id).map(|b| b.panes.clone()).unwrap_or_default();
        for p in panes {
            self.kill_session(p, remove_worktrees);
        }
        self.blocks.remove(&id);
        let _ = self.store.delete_block(id);
        for w in &mut self.workspaces {
            w.blocks.retain(|b| *b != id);
        }
        self.persist_ws();
    }

    /// Garante: toda sessão tem bloco do mesmo workspace; blocos listados nos workspaces.
    pub fn fix_blocks(&mut self) {
        let ids: Vec<Id> = self.sessions.keys().copied().collect();
        for sid in ids {
            let (ws, block) = {
                let r = &self.sessions[&sid];
                (r.s.workspace, r.s.block)
            };
            let ok = block.and_then(|b| self.blocks.get(&b)).is_some_and(|b| b.workspace == ws);
            let bid = if ok {
                block.unwrap()
            } else {
                // bloco "Geral" do workspace
                let existing = self.blocks.values().find(|b| b.workspace == ws && b.name == "Geral").map(|b| b.id);
                existing.unwrap_or_else(|| {
                    let b = Block::new(ws, "Geral");
                    let id = b.id;
                    let _ = self.store.save_block(&b);
                    self.blocks.insert(id, b);
                    id
                })
            };
            if let Some(r) = self.sessions.get_mut(&sid) {
                if r.s.block != Some(bid) {
                    r.s.block = Some(bid);
                    let _ = self.store.save_session(&r.s);
                }
            }
            if let Some(b) = self.blocks.get_mut(&bid) {
                if !b.panes.contains(&sid) {
                    b.panes.push(sid);
                    let _ = self.store.save_block(b);
                }
            }
        }
        for b in self.blocks.values_mut() {
            let before = b.panes.len();
            b.panes.retain(|p| self.sessions.contains_key(p));
            if b.panes.len() != before {
                let _ = self.store.save_block(b);
            }
        }
        for w in &mut self.workspaces {
            w.blocks.retain(|b| self.blocks.contains_key(b));
            let mut mine: Vec<_> = self.blocks.values().filter(|b| b.workspace == w.id && !w.blocks.contains(&b.id)).map(|b| (b.created_at, b.id)).collect();
            mine.sort();
            w.blocks.extend(mine.into_iter().map(|(_, id)| id));
        }
        self.persist_ws();
    }

    // ───────────────────────── workspaces ─────────────────────────

    pub fn current_ws(&self) -> Option<&Workspace> {
        self.workspaces.get(self.ws)
    }

    pub fn current_ws_mut(&mut self) -> Option<&mut Workspace> {
        self.workspaces.get_mut(self.ws)
    }

    pub fn ws_sessions(&self) -> Vec<Id> {
        self.current_ws().map(|w| w.sessions.clone()).unwrap_or_default()
    }

    pub fn focused(&self) -> Option<Id> {
        let w = self.current_ws()?;
        w.focused.filter(|f| w.sessions.contains(f)).or_else(|| w.sessions.first().copied())
    }

    pub fn focus(&mut self, id: Id) {
        // a sessão pode estar em outro workspace
        if let Some(rt) = self.sessions.get(&id) {
            if let Some(i) = self.workspaces.iter().position(|w| w.id == rt.s.workspace) {
                if i != self.ws {
                    self.last_ws = self.current_ws().map(|w| w.id);
                    self.ws = i;
                }
            }
        }
        if let Some(w) = self.current_ws_mut() {
            w.focused = Some(id);
        }
        self.empty_block_focus = None;
        if let Some(b) = self.block_of(id) {
            if let Some(bl) = self.blocks.get_mut(&b) {
                bl.focused = Some(id);
            }
        }
        self.layne.focus = false;
        if let Some(rt) = self.sessions.get_mut(&id) {
            rt.live.seen();
            rt.unseen = false;
        }
        self.view = View::Sessions;
        self.term_focus = true;
        self.persist_ws();
    }

    pub fn new_workspace(&mut self, name: &str) {
        let ws = Workspace::new(if name.trim().is_empty() { "Novo workspace" } else { name.trim() });
        let _ = self.store.save_workspace(&ws);
        self.last_ws = self.current_ws().map(|w| w.id);
        self.workspaces.push(ws);
        self.ws = self.workspaces.len() - 1;
        self.toast(&format!("Workspace \"{}\" criado", self.workspaces[self.ws].name), ToastKind::Ok);
    }

    pub fn switch_ws(&mut self, i: usize) {
        if i < self.workspaces.len() {
            if i != self.ws {
                self.last_ws = self.current_ws().map(|w| w.id);
            }
            self.ws = i;
            self.empty_block_focus = None;
            self.term_focus = self.view == View::Sessions;
        }
    }

    /// Volta pro workspace de antes (vai e volta, como o `back_and_forth` do i3).
    pub fn last_workspace(&mut self) {
        match self.last_ws.and_then(|id| self.workspaces.iter().position(|w| w.id == id)) {
            Some(i) => self.switch_ws(i),
            None => self.toast("Nenhum workspace anterior ainda", ToastKind::Info),
        }
    }

    pub fn rename_workspace(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(w) = self.current_ws_mut() {
            w.name = name.to_string();
        }
        self.persist_ws();
    }

    /// Workspaces com alguém precisando de você (pra destacar na barra, como o "urgent" do i3).
    pub fn ws_needs_you(&self, i: usize) -> usize {
        self.workspaces.get(i).map_or(0, |w| {
            w.sessions.iter().filter(|id| self.sessions.get(id).is_some_and(|r| r.state() == SessionState::Waiting || (r.unseen && r.state() == SessionState::Done))).count()
        })
    }

    pub fn delete_workspace(&mut self, i: usize) {
        if self.workspaces.len() <= 1 || i >= self.workspaces.len() {
            return self.toast("Precisa sobrar pelo menos um workspace", ToastKind::Warn);
        }
        let ws = self.workspaces.remove(i);
        for id in ws.sessions {
            self.kill_session(id, false);
        }
        let _ = self.store.delete_workspace(ws.id);
        self.ws = self.ws.min(self.workspaces.len() - 1);
    }

    pub fn persist_ws(&self) {
        for w in &self.workspaces {
            let _ = self.store.save_workspace(w);
        }
    }

    // ───────────────────────── sessões ─────────────────────────

    /// Novo terminal no bloco em foco (ou num bloco novo, se não houver).
    #[allow(clippy::too_many_arguments)]
    pub fn create_session(
        &mut self,
        ctx: &Context,
        name: String,
        kind: SessionKind,
        cwd: PathBuf,
        worktree: Option<PathBuf>,
        task: Option<Id>,
        claude_args: Vec<String>,
    ) -> Id {
        let block = self.current_block();
        self.create_session_in(ctx, block, name, kind, cwd, worktree, task, claude_args, None)
    }

    /// Novo terminal num bloco específico (`None` = bloco novo com o nome da sessão).
    #[allow(clippy::too_many_arguments)]
    pub fn create_session_in(
        &mut self,
        ctx: &Context,
        block: Option<Id>,
        name: String,
        kind: SessionKind,
        cwd: PathBuf,
        worktree: Option<PathBuf>,
        task: Option<Id>,
        claude_args: Vec<String>,
        initial_prompt: Option<String>,
    ) -> Id {
        let Some(ws) = self.current_ws().map(|w| w.id) else { return Id::nil() };
        let block = match block.filter(|b| self.blocks.contains_key(b)) {
            Some(b) => b,
            None => match self.new_block(&name, task) {
                Some(b) => b,
                None => return Id::nil(),
            },
        };
        let mut s = Session::new(ws, name, kind, cwd.clone());
        s.block = Some(block);
        s.initial_prompt = initial_prompt;
        s.repo = self.cfg.repo_of(&cwd);
        s.worktree = worktree;
        s.task = task;
        s.claude_args = claude_args;
        s.color = (self.sessions.len() % self.theme.agents.len().max(1)) as u8;
        let id = s.id;
        let _ = self.store.save_session(&s);
        let mut rt = Runtime::new(s);
        self.spawn(ctx, &mut rt);
        if let Some(e) = &rt.spawn_error {
            self.toast(&format!("Falha ao abrir: {e}"), ToastKind::Err);
        }
        self.sessions.insert(id, rt);
        if let Some(w) = self.current_ws_mut() {
            w.sessions.push(id);
        }
        self.update_block(block, |b| {
            b.panes.push(id);
            b.focused = Some(id);
            if b.task.is_none() {
                b.task = task;
            }
        });
        if let Some(tid) = task {
            self.update_task(tid, |t| {
                t.session = Some(id);
                if t.status == TaskStatus::Todo {
                    t.status = TaskStatus::Doing;
                }
            });
        }
        self.focus(id);
        id
    }

    /// Fecha a sessão. `remove_worktree` remove a pasta do worktree (a branch fica).
    pub fn kill_session(&mut self, id: Id, remove_worktree: bool) {
        if let Some(rt) = self.sessions.remove(&id) {
            if let Some(t) = &rt.term {
                t.kill();
            }
            crate::tmux::kill(id);
            if let Some(b) = rt.s.block.and_then(|b| self.blocks.get_mut(&b)) {
                b.panes.retain(|p| *p != id);
                if b.focused == Some(id) {
                    b.focused = b.panes.last().copied();
                }
                let _ = self.store.save_block(b);
            }
            if remove_worktree {
                if let Some(wt) = &rt.s.worktree {
                    let root = git::main_root(wt);
                    match root.map(|r| git::remove_worktree(&r, wt, false)) {
                        Some(Ok(())) => self.toast("Worktree removido (a branch continua)", ToastKind::Ok),
                        Some(Err(e)) => self.toast(&format!("Worktree não removido: {e}"), ToastKind::Warn),
                        None => {}
                    }
                }
            }
        }
        for w in &mut self.workspaces {
            w.sessions.retain(|s| *s != id);
            if w.focused == Some(id) {
                w.focused = w.sessions.last().copied();
            }
        }
        let linked: Vec<Id> = self.tasks.iter().filter(|t| t.session == Some(id)).map(|t| t.id).collect();
        for t in linked {
            self.update_task(t, |t| t.session = None);
        }
        let _ = self.store.delete_session(id);
        self.tail.forget(id);
        let _ = std::fs::remove_file(StatusSnapshot::path_for(&id.to_string()));
        self.persist_ws();
    }

    pub fn restart_session(&mut self, ctx: &Context, id: Id) {
        if let Some(mut rt) = self.sessions.remove(&id) {
            rt.term = None;
            crate::tmux::kill(id);
            self.spawn(ctx, &mut rt);
            self.sessions.insert(id, rt);
        }
    }

    pub fn rename_session(&mut self, id: Id, name: &str) {
        if let Some(rt) = self.sessions.get_mut(&id) {
            rt.s.name = name.trim().to_string();
            let _ = self.store.save_session(&rt.s);
        }
    }

    pub fn session_label(&self, id: Id) -> String {
        self.sessions.get(&id).map(|r| r.s.name.clone()).unwrap_or_else(|| "—".into())
    }

    pub fn state_of(&self, id: Id) -> SessionState {
        self.sessions.get(&id).map(Runtime::state).unwrap_or(SessionState::GameOver)
    }

    pub fn move_focus(&mut self, dx: i32, dy: i32) {
        if matches!(self.cfg.view, SessionView::Blocks | SessionView::Tabs) {
            return self.move_focus_blocks(dx, dy);
        }
        let ids = self.ws_sessions();
        if ids.is_empty() {
            return;
        }
        let cur = self.focused().and_then(|f| ids.iter().position(|i| *i == f)).unwrap_or(0) as i32;
        let n = ids.len() as i32;
        let next = match self.current_ws().map(|w| w.layout) {
            Some(Layout::Grid) => {
                let cols = (n as f32).sqrt().ceil() as i32;
                let (r, c) = (cur / cols, cur % cols);
                let (r, c) = (r + dy, c + dx);
                let i = r * cols + c;
                if (0..n).contains(&i) && (0..cols).contains(&c) { i } else { cur }
            }
            _ => (cur + dx + dy).rem_euclid(n),
        };
        self.focus(ids[next as usize]);
    }

    /// Blocos/abas: ←/→ andam entre os terminais do bloco, ↑/↓ trocam de bloco.
    fn move_focus_blocks(&mut self, dx: i32, dy: i32) {
        let blocks = self.ws_blocks();
        let Some(cur_b) = self.current_block() else { return };
        if dy != 0 {
            let i = blocks.iter().position(|b| *b == cur_b).unwrap_or(0) as i32;
            let n = blocks.len() as i32;
            let next = blocks[((i + dy).rem_euclid(n)) as usize];
            return self.focus_block(next);
        }
        let panes = self.blocks.get(&cur_b).map(|b| b.panes.clone()).unwrap_or_default();
        if panes.is_empty() {
            return;
        }
        let cur = self.focused().and_then(|f| panes.iter().position(|p| *p == f)).unwrap_or(0) as i32;
        let next = panes[((cur + dx).rem_euclid(panes.len() as i32)) as usize];
        self.focus(next);
    }

    /// Foca um bloco (o terminal que tinha foco nele, ou o primeiro).
    pub fn focus_block(&mut self, id: Id) {
        let target = self.blocks.get(&id).and_then(|b| b.focused.filter(|f| b.panes.contains(f)).or_else(|| b.panes.first().copied()));
        match target {
            Some(s) => self.focus(s),
            None => {
                // bloco vazio: guarda como "atual" mexendo no foco do workspace
                if let Some(w) = self.current_ws_mut() {
                    w.focused = None;
                }
                self.empty_block_focus = Some(id);
            }
        }
    }

    pub fn cycle_session(&mut self, delta: i32) {
        let ids = self.ws_sessions();
        if ids.is_empty() {
            return;
        }
        let cur = self.focused().and_then(|f| ids.iter().position(|i| *i == f)).unwrap_or(0) as i32;
        let next = (cur + delta).rem_euclid(ids.len() as i32) as usize;
        self.focus(ids[next]);
    }

    /// Próxima sessão (em qualquer workspace) que precisa de você.
    pub fn next_waiting(&mut self) {
        let order: Vec<Id> = self.workspaces.iter().flat_map(|w| w.sessions.clone()).collect();
        let cur = self.focused();
        let start = cur.and_then(|c| order.iter().position(|i| *i == c)).map(|i| i + 1).unwrap_or(0);
        let n = order.len();
        let found = (0..n)
            .map(|k| order[(start + k) % n.max(1)])
            .find(|id| Some(*id) != cur && self.state_of(*id).needs_attention());
        match found {
            Some(id) => self.focus(id),
            None => self.toast("Ninguém precisando de você. Tudo sob controle (por enquanto)…", ToastKind::Info),
        }
    }

    // ───────────────────────── tarefas ─────────────────────────

    pub fn reload_tasks(&mut self) {
        if let Ok(t) = self.store.tasks() {
            self.tasks = t;
        }
        self.db_version = self.store.data_version();
    }

    pub fn save_task(&mut self, t: Task) {
        let _ = self.store.save_task(&t);
        match self.tasks.iter_mut().find(|x| x.id == t.id) {
            Some(x) => *x = t,
            None => self.tasks.push(t),
        }
    }

    pub fn update_task(&mut self, id: Id, f: impl FnOnce(&mut Task)) {
        if let Some(mut t) = self.tasks.iter().find(|t| t.id == id).cloned() {
            f(&mut t);
            t.touch();
            self.save_task(t);
        }
    }

    pub fn delete_task(&mut self, id: Id) {
        let _ = self.store.delete_task(id);
        self.tasks.retain(|t| t.id != id);
    }

    /// Captura rápida → tarefa salva. `status`: coluna onde entra.
    pub fn quick_add(&mut self, text: &str, status: TaskStatus, link_session: Option<Id>) -> Option<Id> {
        let ws_names: Vec<String> = self.workspaces.iter().map(|w| w.name.clone()).collect();
        let parsed = aic_core::quickadd::parse(text, Local::now().date_naive(), &self.repos, &ws_names);
        if parsed.title.trim().is_empty() {
            self.toast("Escreve alguma coisa além dos marcadores 😉", ToastKind::Warn);
            return None;
        }
        let ws_name = parsed.workspace.clone();
        let mut t = parsed.into_task();
        t.status = status;
        t.workspace = ws_name
            .and_then(|n| self.workspaces.iter().find(|w| w.name == n).map(|w| w.id))
            .or_else(|| self.current_ws().map(|w| w.id));
        if let Some(s) = link_session {
            t.session = Some(s);
            if t.repo.is_none() {
                t.repo = self.sessions.get(&s).and_then(|r| r.s.repo.clone());
            }
            if let Some(rt) = self.sessions.get_mut(&s) {
                rt.s.task = Some(t.id);
                let _ = self.store.save_session(&rt.s);
            }
        }
        let id = t.id;
        let title = t.title.clone();
        self.save_task(t);
        self.toast(&format!("✓ {title}"), ToastKind::Ok);
        Some(id)
    }

    /// Vincula tarefa ↔ sessão (nos dois lados).
    pub fn link(&mut self, task: Id, session: Id) {
        let repo = self.sessions.get(&session).and_then(|r| r.s.repo.clone());
        self.update_task(task, |t| {
            t.session = Some(session);
            if t.repo.is_none() {
                t.repo = repo;
            }
            if t.status == TaskStatus::Todo {
                t.status = TaskStatus::Doing;
            }
        });
        if let Some(rt) = self.sessions.get_mut(&session) {
            rt.s.task = Some(task);
            let _ = self.store.save_session(&rt.s);
        }
    }

    pub fn task_of(&self, session: Id) -> Option<&Task> {
        let rt = self.sessions.get(&session)?;
        rt.s.task.and_then(|id| self.tasks.iter().find(|t| t.id == id))
            .or_else(|| self.tasks.iter().find(|t| t.session == Some(session)))
    }

    /// Abre (ou foca) a sessão de uma tarefa. Sem sessão: abre o diálogo já preenchido.
    pub fn open_task(&mut self, id: Id) {
        let Some(t) = self.tasks.iter().find(|t| t.id == id).cloned() else { return };
        if let Some(s) = t.session.filter(|s| self.sessions.contains_key(s)) {
            return self.focus(s);
        }
        self.modal = Some(ui::modals::Modal::NewSession(ui::modals::NewSessionForm::for_task(self, &t)));
    }

    /// Escreve `.aic/context.md` no diretório da sessão com card, thread e notas.
    pub fn write_context(&mut self, session: Id) {
        let Some(rt) = self.sessions.get(&session) else { return };
        let Some(t) = self.task_of(session).cloned() else {
            return self.toast("Essa sessão não tem tarefa vinculada (Ctrl+Shift+K)", ToastKind::Warn);
        };
        let dir = rt.s.cwd.join(".aic");
        let mut md = format!("# {}\n\n", t.title);
        if let Some(k) = &t.card_key {
            md.push_str(&format!("- Card: {k}\n"));
        }
        if let Some(u) = &t.notion_url {
            md.push_str(&format!("- Notion: {u}\n"));
        }
        if let Some(u) = &t.slack_thread {
            md.push_str(&format!("- Thread do Slack: {u}\n"));
        }
        if let Some(r) = &t.repo {
            md.push_str(&format!("- Repo: {r}\n"));
        }
        md.push_str(&format!("- Status: {} · prioridade {}\n", t.status.label(), t.priority.label()));
        if !t.notes.trim().is_empty() {
            md.push_str(&format!("\n## Notas\n\n{}\n", t.notes.trim()));
        }
        if !t.subtasks.is_empty() {
            md.push_str("\n## Subtarefas\n\n");
            for s in &t.subtasks {
                md.push_str(&format!("- [{}] {}\n", if s.done { "x" } else { " " }, s.text));
            }
        }
        md.push_str("\n> Gerado pelo AIC. Use a skill `aic:contexto` para puxar o card e a thread completos.\n");
        let result = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join("context.md"), md));
        // .aic/ fora do git sem mexer no .gitignore do repo
        let _ = ensure_git_exclude(&rt.s.cwd);
        match result {
            Ok(()) => {
                if let Some(term) = &self.sessions.get(&session).and_then(|r| r.term.as_ref()) {
                    term.paste("@.aic/context.md ");
                }
                self.toast("Contexto escrito em .aic/context.md e citado no prompt", ToastKind::Ok);
            }
            Err(e) => self.toast(&format!("Não deu pra escrever o contexto: {e}"), ToastKind::Err),
        }
    }

    // ───────────────────────── loop ─────────────────────────

    pub fn toast(&mut self, text: &str, kind: ToastKind) {
        self.toasts.push(Toast { text: text.to_string(), kind, until: Instant::now() + Duration::from_secs(4) });
    }

    pub fn t(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// Trabalho de fundo de cada frame: banco, eventos, git, sons.
    pub fn tick(&mut self, ctx: &Context) {
        self.window_focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));

        if self.db_checked.elapsed() > Duration::from_millis(800) {
            self.db_checked = Instant::now();
            if self.store.data_version() != self.db_version {
                self.reload_tasks();
            }
        }

        if self.events_checked.elapsed() > Duration::from_millis(250) {
            self.events_checked = Instant::now();
            let events = self.tail.poll();
            for e in events {
                let target = e.aic_session.filter(|id| self.sessions.contains_key(id)).or_else(|| {
                    self.sessions.values().find(|r| r.s.claude_session.to_string() == e.claude_session).map(|r| r.s.id)
                });
                let Some(id) = target else {
                    crate::layne::on_event(self, &e);
                    continue;
                };
                let effect = self.sessions.get_mut(&id).and_then(|rt| rt.live.apply(&e));
                if let Some(effect) = effect {
                    self.on_effect(id, effect);
                }
            }
        }

        let visible = self.visible_sessions();
        for (id, rt) in self.sessions.iter_mut() {
            rt.refresh();
            if let Some(t) = &rt.term {
                // tmux/programas copiando via OSC 52 → clipboard do sistema
                if let Some(text) = t.shared.clipboard.lock().unwrap().take() {
                    ctx.copy_text(text);
                }
                let bells = t.shared.bells.load(std::sync::atomic::Ordering::Relaxed);
                if bells > rt.bells_seen {
                    rt.bells_seen = bells;
                    if !visible.contains(id) {
                        rt.unseen = true;
                    }
                }
            }
        }

        if self.account_at.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
            self.account_at = Some(Instant::now());
            if let Some(s) = StatusSnapshot::latest() {
                self.account = Some(s);
            }
        }

        self.usage_history.poll();
        crate::connector::poll(self, ctx);
        crate::layne::tick(self, ctx);
        self.toasts.retain(|t| t.until > Instant::now());

        if self.fonts_dirty {
            self.fonts_dirty = false;
            self.font = aic_term::fonts::install(ctx, &self.cfg.font_family, None, self.cfg.font_size);
        }
    }

    pub fn visible_sessions(&self) -> HashSet<Id> {
        if self.view != View::Sessions || matches!(self.phase, Phase::Setup(_)) {
            return HashSet::new();
        }
        match self.current_ws() {
            Some(w) if w.layout == Layout::Grid && !self.maximized => w.sessions.iter().copied().collect(),
            _ => self.focused().into_iter().collect(),
        }
    }

    fn on_effect(&mut self, id: Id, effect: Effect) {
        let visible = self.visible_sessions().contains(&id) && self.window_focused;
        if visible {
            if let Some(rt) = self.sessions.get_mut(&id) {
                rt.live.seen();
            }
        } else if let Some(rt) = self.sessions.get_mut(&id) {
            rt.unseen = true;
        }
        let quiet = visible && self.cfg.notify.only_when_unfocused;
        if quiet {
            return;
        }
        let name = self.session_label(id);
        let task = self.task_of(id).map(|t| t.title.clone());
        match effect {
            Effect::Finished => {
                if self.cfg.sound.enabled && self.cfg.on(aic_core::Module::Sound) {
                    if let Some(p) = crate::sounds::resolve(self.cfg.sound.done.as_deref().unwrap_or(crate::sounds::DEFAULT_DONE)) {
                        notify::play(&p, self.cfg.sound.volume);
                    }
                }
                if self.cfg.notify.desktop {
                    let body = match task {
                        Some(t) => format!("{name} terminou: {t}"),
                        None => format!("{name} terminou e está te esperando."),
                    };
                    notify::desktop(&format!("{} — {}", crate::brand::NAME, crate::brand::state_label(SessionState::Done)), &body, icon_path());
                }
            }
            Effect::NeedsYou(msg) => {
                if self.cfg.sound.enabled {
                    if let Some(p) = self.cfg.sound.attention.as_deref().and_then(crate::sounds::resolve) {
                        notify::play(&p, self.cfg.sound.volume);
                    }
                }
                if self.cfg.notify.desktop {
                    notify::desktop(&format!("{} precisa de você", name), &msg, icon_path());
                }
            }
        }
    }

    pub fn run_action(&mut self, ctx: &Context, a: Action) {
        use Action::*;
        // módulo desligado: a ação fica inerte (o atalho é consumido e não vaza pro terminal)
        if let Some(m) = a.module().filter(|m| !self.cfg.on(*m)) {
            let name = aic_core::modules::info(m).name;
            return self.toast(&format!("\"{}\" está desligado (Config → Módulos)", name), ToastKind::Warn);
        }
        match a {
            CloseFocused => self.close_focused(),
            NewSession => self.modal = Some(ui::modals::Modal::NewSession(ui::modals::NewSessionForm::new(self, SessionKind::Claude))),
            NewShell => {
                // split rápido: shell na pasta do terminal em foco, no mesmo bloco
                let cwd = self.focused().and_then(|f| self.sessions.get(&f)).map(|r| r.s.cwd.clone()).unwrap_or_else(paths::home_dir);
                let block = self.current_block();
                let name = cwd.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "shell".into());
                self.create_session_in(ctx, block, name, SessionKind::Shell, cwd, None, None, vec![], None);
            }
            NewBlock => {
                if let Some(b) = self.new_block("", None) {
                    self.focus_block(b);
                    self.modal = Some(ui::modals::Modal::RenameBlock { id: b, text: "Novo bloco".into(), first: true });
                }
            }
            CloseBlock => {
                if let Some(b) = self.current_block() {
                    let has_wt = self.blocks.get(&b).is_some_and(|bl| bl.panes.iter().any(|p| self.sessions.get(p).is_some_and(|r| r.s.worktree.is_some())));
                    self.modal = Some(ui::modals::Modal::CloseBlock { id: b, remove_worktrees: false, has_worktrees: has_wt });
                }
            }
            RenameBlock => {
                if let Some(b) = self.current_block() {
                    let name = self.blocks.get(&b).map(|x| x.name.clone()).unwrap_or_default();
                    self.modal = Some(ui::modals::Modal::RenameBlock { id: b, text: name, first: true });
                }
            }
            CycleSplit => {
                if let Some(b) = self.current_block() {
                    self.update_block(b, |x| x.split = x.split.next());
                    let label = self.blocks.get(&b).map(|x| x.split.label()).unwrap_or_default();
                    self.toast(&format!("Arranjo: {label}"), ToastKind::Info);
                }
            }
            ToggleBlockSide => {
                if let Some(b) = self.current_block() {
                    self.update_block(b, |x| x.side = !x.side);
                }
            }
            FocusNotes => {
                if let Some(b) = self.current_block() {
                    self.update_block(b, |x| x.side = true);
                    self.focus_notes = Some(b);
                    self.view = View::Sessions;
                }
            }
            OpenCard => crate::connector::open_card_dialog(self),
            CloseSession => {
                if let Some(id) = self.focused() {
                    let has_wt = self.sessions.get(&id).is_some_and(|r| r.s.worktree.is_some());
                    self.modal = Some(ui::modals::Modal::CloseSession { id, remove_worktree: false, has_worktree: has_wt });
                }
            }
            RenameSession => {
                if let Some(id) = self.focused() {
                    self.modal = Some(ui::modals::Modal::Rename { id, text: self.session_label(id), first: true });
                }
            }
            RestartSession => {
                if let Some(id) = self.focused() {
                    self.restart_session(ctx, id);
                }
            }
            SendContext => {
                if let Some(id) = self.focused() {
                    self.write_context(id);
                }
            }
            CollapseBlock => {
                if let Some(b) = self.current_block() {
                    self.update_block(b, |x| x.collapsed = !x.collapsed);
                }
            }
            MoveBlockUp | MoveBlockDown => {
                if let Some(b) = self.current_block() {
                    ui::blocks::move_block(self, b, if a == MoveBlockUp { -1 } else { 1 });
                }
            }
            ToggleHints => {
                self.cfg.hints_bar = !self.cfg.hints_bar;
                let _ = self.cfg.save();
            }
            NextSession => self.cycle_session(1),
            PrevSession => self.cycle_session(-1),
            NextWaiting => self.next_waiting(),
            FocusLeft => self.move_focus(-1, 0),
            FocusRight => self.move_focus(1, 0),
            FocusUp => self.move_focus(0, -1),
            FocusDown => self.move_focus(0, 1),
            Goto(n) => {
                if matches!(self.cfg.view, SessionView::Blocks | SessionView::Tabs) {
                    if let Some(b) = self.ws_blocks().get(n as usize - 1).copied() {
                        self.focus_block(b);
                        self.view = View::Sessions;
                    }
                } else if let Some(id) = self.ws_sessions().get(n as usize - 1).copied() {
                    self.focus(id);
                }
            }
            ToggleLayout => {
                if let Some(w) = self.current_ws_mut() {
                    w.layout = if w.layout == Layout::Grid { Layout::Focus } else { Layout::Grid };
                }
                self.maximized = false;
                self.persist_ws();
            }
            CycleView => {
                let all = aic_core::SessionView::ALL;
                let i = all.iter().position(|v| *v == self.cfg.view).unwrap_or(0);
                self.cfg.view = all[(i + 1) % all.len()];
                let _ = self.cfg.save();
                self.toast(&format!("Visualização: {} — {}", self.cfg.view.label(), self.cfg.view.desc()), ToastKind::Info);
            }
            ToggleMaximize => self.maximized = !self.maximized,
            ReleaseTerminal => {
                self.term_focus = false;
                self.layne.focus = false;
            }
            ToggleLayne => crate::layne::toggle(self, ctx),
            LayneVoice => crate::layne::toggle_voice(self, ctx),
            QuickAdd => self.open_quick_add(None),
            LinkTaskToSession => {
                if let Some(id) = self.focused() {
                    self.modal = Some(ui::modals::Modal::LinkTask { session: id, query: String::new(), selected: 0 });
                }
            }
            Workspaces => self.modal = Some(ui::modals::Modal::Workspaces { query: String::new(), selected: self.ws }),
            NewWorkspace => self.modal = Some(ui::modals::Modal::Workspaces { query: String::new(), selected: usize::MAX }),
            NextWorkspace => self.switch_ws((self.ws + 1) % self.workspaces.len().max(1)),
            PrevWorkspace => self.switch_ws((self.ws + self.workspaces.len().max(1) - 1) % self.workspaces.len().max(1)),
            GotoWorkspace(n) => {
                let i = n as usize - 1;
                if i < self.workspaces.len() {
                    self.switch_ws(i);
                } else {
                    self.toast(&format!("Não existe workspace {n} ({} abertos)", self.workspaces.len()), ToastKind::Info);
                }
            }
            LastWorkspace => self.last_workspace(),
            RenameWorkspace => {
                let name = self.current_ws().map(|w| w.name.clone()).unwrap_or_default();
                self.modal = Some(ui::modals::Modal::RenameWorkspace { text: name, first: true });
            }
            ViewSessions => {
                self.view = View::Sessions;
                self.term_focus = true;
            }
            ViewBoard => self.set_view(View::Board),
            ViewWorktrees => self.set_view(View::Worktrees),
            ViewChanges => self.set_view(View::Changes),
            ViewUsage => self.set_view(View::Usage),
            ViewSettings => self.set_view(View::Settings),
            ViewHelp => self.set_view(View::Help),
            NextView => self.cycle_view(1),
            PrevView => self.cycle_view(-1),
            SearchSettings => {
                self.set_view(View::Settings);
                self.settings.focus_search = true;
            }
            Catalog => self.leader = Some(Default::default()),
            Palette => self.modal = Some(ui::modals::Modal::Palette { query: String::new(), selected: 0 }),
            ZoomIn => self.set_font_size(self.cfg.font_size + 1.0),
            ZoomOut => self.set_font_size(self.cfg.font_size - 1.0),
            ZoomReset => self.set_font_size(14.0),
            ToggleSidebar => self.sidebar = !self.sidebar,
            Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    pub fn set_view(&mut self, v: View) {
        self.view = v;
        self.term_focus = v == View::Sessions;
    }

    /// Abas visíveis na barra (as de módulos desligados somem).
    pub fn tabs(&self) -> Vec<View> {
        View::ALL.into_iter().filter(|v| v.action().enabled(&self.cfg)).collect()
    }

    /// Próxima/anterior aba da barra, dando a volta.
    pub fn cycle_view(&mut self, delta: i32) {
        let tabs = self.tabs();
        let i = tabs.iter().position(|v| *v == self.view).unwrap_or(0) as i32;
        let next = tabs[(i + delta).rem_euclid(tabs.len() as i32) as usize];
        self.set_view(next);
    }

    /// Fecha o que está em foco, do mais específico pro mais geral:
    /// cartão aberto no board → aba que não é Sessões → terminal em foco → bloco vazio.
    pub fn close_focused(&mut self) {
        match self.view {
            View::Board if self.board.selected.is_some() => {
                self.board.selected = None;
            }
            View::Sessions => {
                if let Some(b) = self.empty_block_focus.filter(|b| self.blocks.get(b).is_some_and(|x| x.panes.is_empty())) {
                    self.modal = Some(ui::modals::Modal::CloseBlock { id: b, remove_worktrees: false, has_worktrees: false });
                } else if let Some(id) = self.focused() {
                    let has_wt = self.sessions.get(&id).is_some_and(|r| r.s.worktree.is_some());
                    self.modal = Some(ui::modals::Modal::CloseSession { id, remove_worktree: false, has_worktree: has_wt });
                } else if let Some(b) = self.current_block() {
                    self.modal = Some(ui::modals::Modal::CloseBlock { id: b, remove_worktrees: false, has_worktrees: false });
                } else {
                    self.toast("Nada em foco pra fechar", ToastKind::Info);
                }
            }
            _ => self.set_view(View::Sessions),
        }
    }

    pub fn open_quick_add(&mut self, status: Option<TaskStatus>) {
        let link = (self.view == View::Sessions).then(|| self.focused()).flatten();
        self.modal = Some(ui::modals::Modal::QuickAdd { text: String::new(), status: status.unwrap_or_default(), link, link_enabled: false });
    }

    pub fn set_font_size(&mut self, size: f32) {
        self.cfg.font_size = size.clamp(8.0, 32.0);
        self.font.regular.size = self.cfg.font_size;
        if let Some(b) = &mut self.font.bold {
            b.size = self.cfg.font_size;
        }
        let _ = self.cfg.save();
    }

    pub fn reload_fonts(&mut self) {
        self.fonts_dirty = true;
    }

    /// Fechar a janela: "sem salvar" (cache some) ou "salvar" (scrollback + layout pro próximo start).
    pub fn quit(&mut self, ctx: &Context, save: bool) {
        if save {
            for b in self.blocks.values() {
                let _ = self.store.save_block(b);
            }
            for (id, rt) in &self.sessions {
                if let Some(t) = &rt.term {
                    let _ = self.store.save_scrollback(*id, &t.dump_text(2000));
                }
                let _ = self.store.save_session(&rt.s);
            }
            self.persist_ws();
            let _ = self.store.mark_cache(true);
        } else {
            let _ = self.store.clear_cache();
            if self.cfg.on(aic_core::Module::Tmux) {
                crate::tmux::kill_all();
            }
        }
        let _ = self.cfg.save();
        let _ = self.keymap.save();
        self.quitting = true;
        crate::layne::quit(self);
        aic_core::inbox::clear_running();
        for rt in self.sessions.values() {
            if let Some(t) = &rt.term {
                t.kill();
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

pub fn repo_name(cwd: &Path, repos_dir: &Path) -> Option<String> {
    // dentro de repos/ → primeiro componente; worktree `<repo>.worktrees/x` → `<repo>`
    let rel = cwd.strip_prefix(repos_dir).ok()?;
    let first = rel.components().next()?.as_os_str().to_string_lossy().to_string();
    Some(first.trim_end_matches(".worktrees").to_string())
}

pub(crate) fn ensure_git_exclude(cwd: &Path) -> std::io::Result<()> {
    let Some(top) = git::toplevel(cwd) else { return Ok(()) };
    let out = std::process::Command::new("git").arg("-C").arg(&top).args(["rev-parse", "--git-path", "info/exclude"]).output()?;
    let rel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if rel.is_empty() {
        return Ok(());
    }
    let path = if Path::new(&rel).is_absolute() { PathBuf::from(rel) } else { top.join(rel) };
    let current = std::fs::read_to_string(&path).unwrap_or_default();
    if !current.lines().any(|l| l.trim() == ".aic/") {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&path, format!("{current}{}.aic/\n", if current.ends_with('\n') || current.is_empty() { "" } else { "\n" }))?;
    }
    Ok(())
}

/// Ícone instalado pelo `install.sh` (pra notificação do sistema).
pub fn icon_path() -> Option<PathBuf> {
    let p = paths::data_dir().parent()?.join("icons/hicolor/128x128/apps/aic.png");
    p.exists().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_do_repo_e_do_worktree() {
        let base = Path::new("/h/dev");
        assert_eq!(repo_name(Path::new("/h/dev/api-pagamentos/src"), base).as_deref(), Some("api-pagamentos"));
        assert_eq!(repo_name(Path::new("/h/dev/notificador.worktrees/DEV-1"), base).as_deref(), Some("notificador"));
        assert_eq!(repo_name(Path::new("/tmp"), base), None);
    }
}
