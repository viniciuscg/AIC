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
    #[serde(default)]
    pub layout: Layout,
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
            layout: Layout::default(),
            sessions: vec![],
            focused: None,
            blocks: vec![],
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Layout {
    /// Uma sessão grande + lista lateral.
    #[default]
    Focus,
    /// Todas lado a lado.
    Grid,
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

/// Um contexto de trabalho (normalmente um card): vários terminais, notas, a tarefa vinculada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Block {
    pub id: Id,
    pub workspace: Id,
    pub name: String,
    #[serde(default)]
    pub color: u8,
    /// Tarefa/card deste bloco.
    #[serde(default)]
    pub task: Option<Id>,
    /// Anotações em Markdown.
    #[serde(default)]
    pub notes: String,
    /// Terminais, em ordem.
    #[serde(default)]
    pub panes: Vec<Id>,
    #[serde(default)]
    pub split: Split,
    /// Recolhido na visualização de blocos.
    #[serde(default)]
    pub collapsed: bool,
    /// Painel lateral (card, notas, tarefas) aberto.
    #[serde(default = "yes")]
    pub side: bool,
    /// Altura na visualização de blocos (px).
    #[serde(default = "default_height")]
    pub height: f32,
    #[serde(default)]
    pub focused: Option<Id>,
    pub created_at: DateTime<Utc>,
}

fn yes() -> bool {
    true
}

fn default_height() -> f32 {
    380.0
}

impl Block {
    pub fn new(workspace: Id, name: impl Into<String>) -> Self {
        Self {
            id: new_id(),
            workspace,
            name: name.into(),
            color: 0,
            task: None,
            notes: String::new(),
            panes: vec![],
            split: Split::Row,
            collapsed: false,
            side: true,
            height: default_height(),
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
    pub task: Option<Id>,
    #[serde(default)]
    pub color: u8,
    /// Args extras pro claude (ex: `--model opus`, `--permission-mode plan`).
    #[serde(default)]
    pub claude_args: Vec<String>,
    /// Primeira mensagem pro Claude (só na criação; o app limpa depois de usar).
    #[serde(default)]
    pub initial_prompt: Option<String>,
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
            task: None,
            color: 0,
            claude_args: vec![],
            initial_prompt: None,
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
            Self::Done => "DONE!",
            Self::GameOver => "GAME OVER",
        }
    }

    /// Precisa da atenção do usuário (entra no "próxima aguardando").
    pub fn needs_attention(self) -> bool {
        matches!(self, Self::Waiting | Self::Done)
    }
}

// ───────────────────────────── Tarefa ─────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum TaskStatus {
    #[default]
    Todo,
    Doing,
    Review,
    Done,
    Archived,
}

impl TaskStatus {
    pub const BOARD: [TaskStatus; 4] = [Self::Todo, Self::Doing, Self::Review, Self::Done];

    pub fn label(self) -> &'static str {
        match self {
            Self::Todo => "A fazer",
            Self::Doing => "Fazendo",
            Self::Review => "Revisão",
            Self::Done => "Feito",
            Self::Archived => "Arquivado",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Todo => Self::Doing,
            Self::Doing => Self::Review,
            Self::Review => Self::Done,
            s => s,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Doing => Self::Todo,
            Self::Review => Self::Doing,
            Self::Done => Self::Review,
            s => s,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
    Urgent,
}

impl Priority {
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "baixa",
            Self::Normal => "normal",
            Self::High => "alta",
            Self::Urgent => "urgente",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum TaskSource {
    /// Criada no AIC (atalho, CLI, /tarefa).
    #[default]
    Local,
    /// Espelho de um card do Notion.
    Notion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Subtask {
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: Id,
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub status: TaskStatus,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub due: Option<NaiveDate>,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub workspace: Option<Id>,
    #[serde(default)]
    pub session: Option<Id>,
    #[serde(default)]
    pub source: TaskSource,
    /// Chave do card (ex: `DEV-142`).
    #[serde(default)]
    pub card_key: Option<String>,
    /// Page id do Notion, quando vinculado.
    #[serde(default)]
    pub notion_page: Option<String>,
    #[serde(default)]
    pub notion_url: Option<String>,
    #[serde(default)]
    pub slack_thread: Option<String>,
    #[serde(default)]
    pub subtasks: Vec<Subtask>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Ordem dentro da coluna (menor = mais em cima).
    #[serde(default)]
    pub rank: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Task {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: new_id(),
            title: title.into(),
            notes: String::new(),
            status: TaskStatus::Todo,
            priority: Priority::Normal,
            due: None,
            repo: None,
            workspace: None,
            session: None,
            source: TaskSource::Local,
            card_key: None,
            notion_page: None,
            notion_url: None,
            slack_thread: None,
            subtasks: vec![],
            tags: vec![],
            rank: now.timestamp_millis() as f64,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }

    pub fn subtasks_progress(&self) -> Option<(usize, usize)> {
        (!self.subtasks.is_empty())
            .then(|| (self.subtasks.iter().filter(|s| s.done).count(), self.subtasks.len()))
    }

    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        self.due.is_some_and(|d| d < today) && !matches!(self.status, TaskStatus::Done | TaskStatus::Archived)
    }
}
