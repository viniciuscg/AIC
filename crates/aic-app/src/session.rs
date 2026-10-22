//! Sessão ao vivo: o PTY + estado derivado dos hooks + git + snapshot da statusline.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use aic_core::claude::{self, Injection, StatusSnapshot};
use aic_core::events::Live;
use aic_core::git::{self, GitStatus};
use aic_core::{Session, SessionKind, SessionState, Task};
use aic_term::{TermSettings, TerminalBackend, TerminalTheme, now_ms};

pub struct Runtime {
    pub s: Session,
    pub term: Option<TerminalBackend>,
    pub live: Live,
    pub git: Option<GitStatus>,
    git_at: Option<Instant>,
    /// `git status` rodando numa thread (nunca na thread da UI).
    git_rx: Option<Receiver<Option<GitStatus>>>,
    pub snap: Option<StatusSnapshot>,
    snap_at: Option<Instant>,
    pub started: Instant,
    /// Texto do scrollback restaurado do cache (mostrado acima do terminal novo).
    pub restored: Option<String>,
    pub spawn_error: Option<String>,
    /// Último "bell" já processado.
    pub bells_seen: u64,
    /// Visto pelo usuário desde o último DONE.
    pub unseen: bool,
    /// Reconectou numa sessão tmux que já estava rodando.
    pub reattached: bool,
}

impl Runtime {
    pub fn new(s: Session) -> Self {
        Self {
            s,
            term: None,
            live: Live::default(),
            git: None,
            git_at: None,
            git_rx: None,
            snap: None,
            snap_at: None,
            started: Instant::now(),
            restored: None,
            spawn_error: None,
            bells_seen: 0,
            unseen: false,
            reattached: false,
        }
    }

    /// Abre o terminal. Com `use_tmux`, roda dentro do servidor tmux do AIC; se a sessão já
    /// existir lá (o app foi fechado mantendo tudo rodando), só reconecta.
    pub fn spawn(
        &mut self,
        ctx: &egui::Context,
        term_id: u64,
        inj: &Injection,
        theme: &TerminalTheme,
        task: Option<&Task>,
        use_tmux: bool,
    ) {
        let mut env: HashMap<String, String> = HashMap::new();
        env.insert("AIC".into(), "1".into());
        env.insert("AIC_SESSION".into(), self.s.id.to_string());
        // o Layne não tem workspace: sem a variável, `aic add` cai no workspace que ele escolher
        if !self.s.workspace.is_nil() {
            env.insert("AIC_WORKSPACE".into(), self.s.workspace.to_string());
        }
        if let Ok(exe) = std::env::current_exe() {
            env.insert("AIC_BIN".into(), exe.to_string_lossy().into());
        }
        if let Some(t) = task {
            env.insert("AIC_TASK".into(), t.id.to_string());
        }

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
        let (program, args) = match &self.s.kind {
            SessionKind::Claude => claude::command(&self.s, inj, task.map(task_prompt).as_deref()),
            SessionKind::Shell => (shell.clone(), vec!["-l".into()]),
            SessionKind::Command(cmd) => (
                shell.clone(),
                vec!["-lc".into(), format!("{cmd}; exec {shell} -l")],
            ),
        };

        let cwd = if self.s.cwd.is_dir() {
            self.s.cwd.clone()
        } else {
            aic_core::paths::home_dir()
        };
        self.reattached = false;
        let (program, args, env) = if use_tmux && crate::tmux::available() {
            self.reattached = crate::tmux::alive(self.s.id);
            let pairs: Vec<(String, String)> =
                env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let (p, a) = crate::tmux::wrap(self.s.id, &cwd, &pairs, &program, &args);
            (p, a, env)
        } else {
            (program, args, env)
        };
        let settings = TermSettings {
            program,
            args,
            cwd: Some(cwd),
            env,
            scrollback: 20_000,
        };
        match TerminalBackend::spawn(term_id, ctx.clone(), settings, theme.clone()) {
            Ok(t) => {
                self.term = Some(t);
                self.spawn_error = None;
                self.started = Instant::now();
                self.live = Live::default();
            }
            Err(e) => self.spawn_error = Some(format!("{e:#}")),
        }
    }

    pub fn state(&self) -> SessionState {
        let Some(t) = &self.term else {
            return if self.spawn_error.is_some() {
                SessionState::GameOver
            } else {
                SessionState::Boot
            };
        };
        if t.is_exited() {
            return SessionState::GameOver;
        }
        if self.live.has_hooks {
            return self.live.state;
        }
        // Sem hooks (shell, comando, ou claude fora do plugin): heurística de atividade.
        let since = now_ms().saturating_sub(t.shared.last_output_ms.load(Ordering::Relaxed));
        if self.started.elapsed() < Duration::from_millis(1500) {
            SessionState::Boot
        } else if since < 1200 {
            SessionState::Working
        } else {
            SessionState::Idle
        }
    }

    /// Atualiza git (a cada 5s, numa thread) e snapshot da statusline (a cada 2s).
    pub fn refresh(&mut self) {
        if let Some(rx) = &self.git_rx
            && let Ok(st) = rx.try_recv()
        {
            self.git = st;
            self.git_rx = None;
        }
        if self.git_rx.is_none()
            && self
                .git_at
                .is_none_or(|t| t.elapsed() > Duration::from_secs(5))
        {
            self.git_at = Some(Instant::now());
            let (tx, rx) = mpsc::channel();
            let cwd = self.s.cwd.clone();
            if std::thread::Builder::new()
                .name("aic-git".into())
                .spawn(move || tx.send(git::status(&cwd)))
                .is_ok()
            {
                self.git_rx = Some(rx);
            }
        }
        if matches!(self.s.kind, SessionKind::Claude)
            && self
                .snap_at
                .is_none_or(|t| t.elapsed() > Duration::from_secs(2))
        {
            self.snap_at = Some(Instant::now());
            if let Some(s) = StatusSnapshot::load(&self.s.claude_session.to_string()) {
                self.snap = Some(s);
            }
        }
    }

    pub fn force_git_refresh(&mut self) {
        self.git_at = None;
    }

    pub fn display_dir(&self) -> String {
        self.s
            .repo
            .clone()
            .or_else(|| {
                self.s
                    .cwd
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "~".into())
    }

    pub fn worktree_branch(&self) -> Option<String> {
        self.git.as_ref().and_then(|g| g.branch.clone())
    }

    pub fn cwd(&self) -> PathBuf {
        self.s.cwd.clone()
    }
}

/// Contexto da tarefa vinculada, entregue ao Claude via `--append-system-prompt`.
pub fn task_prompt(t: &Task) -> String {
    let mut p = format!(
        "Você está numa sessão aberta pelo AIC (gerenciador de sessões do Claude).\nTarefa vinculada: \"{}\"",
        t.title
    );
    if let Some(k) = &t.card_key {
        p.push_str(&format!(" (card {k})"));
    }
    p.push('.');
    if !t.notes.trim().is_empty() {
        p.push_str(&format!("\nNotas da tarefa:\n{}", t.notes.trim()));
    }
    if !t.subtasks.is_empty() {
        p.push_str("\nSubtarefas:");
        for s in &t.subtasks {
            p.push_str(&format!(
                "\n- [{}] {}",
                if s.done { "x" } else { " " },
                s.text
            ));
        }
    }
    p.push_str("\nSe existir .aic/context.md no diretório, leia antes de começar.");
    p
}
