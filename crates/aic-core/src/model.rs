//! Tipos do domínio. Persistidos como JSON no SQLite (ver `store`).

use std::path::PathBuf;

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Id = Uuid;

pub fn new_id() -> Id {
    Uuid::new_v4()
}

// ───────────────────────────── Workspace ─────────────────────────────

/// Nível acima das sessões: um contexto de trabalho (ex: "Sprint 42", "Infra").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub icon: String,
    /// Ordem das sessões na grade/lista.
    #[serde(default)]
    pub sessions: Vec<Id>,
    /// Sessão com foco quando o workspace foi deixado.
    #[serde(default)]
    pub focused: Option<Id>,
    /// Ordem dos blocos (contextos) deste workspace.
    #[serde(default)]
    pub blocks: Vec<Id>,
    pub created_at: DateTime<Utc>,
}

impl Workspace {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            icon: String::new(),
            sessions: vec![],
            focused: None,
            blocks: vec![],
            created_at: Utc::now(),
        }
    }
}

// ───────────────────────────── Bloco ─────────────────────────────

/// Como os terminais de um bloco se arrumam.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Split {
    /// Lado a lado.
    #[default]
    Row,
    /// Um embaixo do outro.
    Column,
    /// O primeiro grande à esquerda, o resto empilhado à direita (main-vertical do tmux).
    Main,
    /// Grade.
    Grid,
}

impl Split {
    pub const ALL: [Split; 4] = [Split::Row, Split::Main, Split::Column, Split::Grid];

    pub fn label(self) -> &'static str {
        match self {
            Split::Row => "lado a lado",
            Split::Column => "empilhado",
            Split::Main => "principal + pilha",
            Split::Grid => "grade",
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// Uma sessão da barra: um contexto de trabalho (um repo, um card) com seus terminais.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: Id,
    pub workspace: Id,
    pub name: String,
    #[serde(default)]
    pub color: u8,
    /// Terminais, em ordem.
    #[serde(default)]
    pub panes: Vec<Id>,
    #[serde(default)]
    pub split: Split,
    #[serde(default)]
    pub focused: Option<Id>,
    pub created_at: DateTime<Utc>,
}

impl Block {
    pub fn new(workspace: Id, name: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            workspace,
            name: name.into(),
            color: 0,
            panes: vec![],
            split: Split::Row,
            focused: None,
            created_at: Utc::now(),
        }
    }
}

// ───────────────────────────── Sessão ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(tag = "type", content = "cmd")]
pub enum SessionKind {
    /// `claude --session-id/--resume`
    #[default]
    Claude,
    /// Shell interativo.
    Shell,
    /// Comando qualquer (ex: `npm run dev`). Relançado ao restaurar o cache.
    Command(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub id: Id,
    pub workspace: Id,
    /// Bloco ao qual o terminal pertence.
    #[serde(default)]
    pub block: Option<Id>,
    pub name: String,
    pub kind: SessionKind,
    pub cwd: PathBuf,
    /// Nome do repo (`api-pagamentos`) quando o cwd está em `repos/`.
    #[serde(default)]
    pub repo: Option<String>,
    /// Worktree criado para esta sessão (removível ao fechar).
    #[serde(default)]
    pub worktree: Option<PathBuf>,
    /// Session id do Claude Code (`--session-id`). Igual ao `id` por padrão.
    pub claude_session: Id,
    #[serde(default)]
    pub color: u8,
    /// Args extras pro claude (ex: `--model opus`, `--permission-mode plan`).
    #[serde(default)]
    pub claude_args: Vec<String>,
    /// Primeira mensagem pro Claude (só na criação; o app limpa depois de usar).
    #[serde(default)]
    pub initial_prompt: Option<String>,
    /// Notas em Markdown da sessão (painel de notas, `Ctrl+B e`).
    #[serde(default)]
    pub notes: String,
    pub created_at: DateTime<Utc>,
}

impl Session {
    pub fn new(workspace: Id, name: impl Into<String>, kind: SessionKind, cwd: PathBuf) -> Self {
        let id = new_id();
        Self {
            id,
            workspace,
            block: None,
            name: name.into(),
            kind,
            cwd,
            repo: None,
            worktree: None,
            claude_session: id,
            color: 0,
            claude_args: vec![],
            initial_prompt: None,
            notes: String::new(),
            created_at: Utc::now(),
        }
    }
}

/// Estado ao vivo da sessão (não persistido).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionState {
    #[default]
    Boot,
    /// Claude trabalhando (prompt enviado, ainda sem Stop).
    Working,
    /// Esperando o usuário digitar.
    Idle,
    /// Pediu permissão / mandou notificação: precisa de você.
    Waiting,
    /// Terminou o turno e você ainda não olhou.
    Done,
    /// Processo morreu.
    GameOver,
}

impl SessionState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Boot => "BOOT",
            Self::Working => "WORKING",
            Self::Idle => "IDLE",
            Self::Waiting => "WAITING",
            Self::Done => "DONE",
            Self::GameOver => "EXITED",
        }
    }

    /// Precisa da atenção do usuário (entra no "próxima aguardando").
    pub fn needs_attention(self) -> bool {
        matches!(self, Self::Waiting | Self::Done)
    }
}

// ───────────────────────────── Tarefa ─────────────────────────────

/// Uma linha da lista de tarefas: título e data. Não tem ligação com sessões.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: Id,
    pub title: String,
    #[serde(default)]
    pub due: Option<NaiveDate>,
    #[serde(default)]
    pub done: bool,
    /// Notas de antes da v2 (aparecem ao passar o mouse; a lista não cria mais).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// Status de antes da v2: só é lido, pra virar `done` (ver `Store::tasks`).
    #[serde(default, skip_serializing)]
    pub status: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Task {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: new_id(),
            title: title.into(),
            due: None,
            done: false,
            notes: String::new(),
            status: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    /// Tarefa salva pela v1: `Done`/`Archived` viram concluída.
    pub fn migrate(&mut self) {
        if matches!(self.status.take().as_deref(), Some("Done" | "Archived")) {
            self.done = true;
        }
    }
}
