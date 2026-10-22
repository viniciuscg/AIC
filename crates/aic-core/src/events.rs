//! Eventos dos hooks do Claude Code → estado ao vivo das sessões.
//!
//! O `aic hook` (configurado via `--settings` em cada sessão) anexa uma linha
//! JSON em `~/.local/state/aic/events/<sessão>.jsonl`. O app lê esses arquivos
//! incrementalmente e roda a máquina de estados abaixo.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Id, SessionState};
use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookEvent {
    pub ts_ms: u64,
    /// Sessão do AIC (env `AIC_SESSION`), quando a sessão foi aberta pelo app.
    pub aic_session: Option<Id>,
    pub claude_session: String,
    pub event: String,
    #[serde(default)]
    pub notification_type: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub permission_mode: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

impl HookEvent {
    /// Monta a partir do JSON que o Claude Code manda no stdin do hook.
    pub fn from_hook_input(v: &Value, aic_session: Option<Id>, ts_ms: u64) -> Self {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        let message = s("message")
            .or_else(|| s("prompt"))
            .or_else(|| s("last_assistant_message"))
            .map(|m| truncate(&m, 500));
        Self {
            ts_ms,
            aic_session,
            claude_session: s("session_id").unwrap_or_default(),
            event: s("hook_event_name").unwrap_or_default(),
            notification_type: s("notification_type"),
            message,
            permission_mode: s("permission_mode"),
            agent_id: s("agent_id"),
            tool: s("tool_name"),
            cwd: s("cwd").map(PathBuf::from),
        }
    }

    /// Arquivo onde o hook grava: por sessão do AIC, ou do Claude se aberta fora do app.
    pub fn file(&self) -> PathBuf {
        let name = self
            .aic_session
            .map(|i| i.to_string())
            .unwrap_or_else(|| self.claude_session.clone());
        paths::events_dir().join(format!("{name}.jsonl"))
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// O que o app deve fazer quando o estado muda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Terminou o turno: som do AIC + notificação.
    Finished,
    /// Precisa de você (permissão, pergunta).
    NeedsYou(String),
}

/// Estado ao vivo de uma sessão, derivado dos hooks.
#[derive(Debug, Clone, Default)]
pub struct Live {
    pub state: SessionState,
    pub agents: HashSet<String>,
    pub permission_mode: Option<String>,
    pub last_prompt: Option<String>,
    pub last_message: Option<String>,
    pub last_tool: Option<String>,
    pub last_event_ms: u64,
    /// Teve evento de hook (senão o app usa a heurística de atividade do terminal).
    pub has_hooks: bool,
    pub turns: u32,
}

impl Live {
    pub fn apply(&mut self, e: &HookEvent) -> Option<Effect> {
        self.has_hooks = true;
        self.last_event_ms = e.ts_ms;
        if e.permission_mode.is_some() {
            self.permission_mode = e.permission_mode.clone();
        }
        match e.event.as_str() {
            "SessionStart" => {
                if matches!(self.state, SessionState::Boot | SessionState::GameOver) {
                    self.state = SessionState::Idle;
                }
                None
            }
            "UserPromptSubmit" => {
                self.state = SessionState::Working;
                self.last_prompt = e.message.clone();
                self.turns += 1;
                None
            }
            "PreToolUse" | "PostToolUse" | "PostToolUseFailure" | "PostToolBatch"
            | "PermissionDenied" => {
                if e.tool.is_some() {
                    self.last_tool = e.tool.clone();
                }
                self.state = SessionState::Working;
                None
            }
            "PermissionRequest" => {
                self.state = SessionState::Waiting;
                Some(Effect::NeedsYou(format!(
                    "Pedindo permissão: {}",
                    e.tool.clone().unwrap_or_default()
                )))
            }
            "Notification" => match e.notification_type.as_deref() {
                Some("permission_prompt" | "elicitation_dialog" | "agent_needs_input") => {
                    let was = self.state;
                    self.state = SessionState::Waiting;
                    (was != SessionState::Waiting).then(|| {
                        Effect::NeedsYou(
                            e.message
                                .clone()
                                .unwrap_or_else(|| "Precisa de você".into()),
                        )
                    })
                }
                _ => None,
            },
            "SubagentStart" => {
                if let Some(a) = &e.agent_id {
                    self.agents.insert(a.clone());
                }
                None
            }
            "SubagentStop" => {
                if let Some(a) = &e.agent_id {
                    self.agents.remove(a);
                }
                None
            }
            "Stop" | "StopFailure" => {
                self.state = SessionState::Done;
                if e.message.is_some() {
                    self.last_message = e.message.clone();
                }
                Some(Effect::Finished)
            }
            "SessionEnd" => {
                self.agents.clear();
                self.state = SessionState::Idle;
                None
            }
            _ => None,
        }
    }

    /// O usuário olhou a sessão: DONE vira IDLE.
    pub fn seen(&mut self) {
        if self.state == SessionState::Done {
            self.state = SessionState::Idle;
        }
    }
}

/// Lê os `.jsonl` de eventos incrementalmente (guarda o offset de cada arquivo).
#[derive(Default)]
pub struct EventTail {
    offsets: HashMap<PathBuf, u64>,
    dir: Option<PathBuf>,
}

impl EventTail {
    pub fn new() -> Self {
        Self {
            offsets: HashMap::new(),
            dir: None,
        }
    }

    pub fn with_dir(dir: PathBuf) -> Self {
        Self {
            offsets: HashMap::new(),
            dir: Some(dir),
        }
    }

    /// Começa do fim dos arquivos existentes (não reprocessa histórico antigo).
    pub fn skip_existing(&mut self) {
        let dir = self.dir();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                if let Ok(m) = e.metadata() {
                    self.offsets.insert(e.path(), m.len());
                }
            }
        }
    }

    fn dir(&self) -> PathBuf {
        self.dir.clone().unwrap_or_else(paths::events_dir)
    }

    pub fn poll(&mut self) -> Vec<HookEvent> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(self.dir()) else {
            return out;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "jsonl") {
                continue;
            }
            let Ok(len) = entry.metadata().map(|m| m.len()) else {
                continue;
            };
            let off = self.offsets.get(&path).copied().unwrap_or(0);
            if len < off {
                // arquivo truncado/recriado
                self.offsets.insert(path.clone(), 0);
            }
            let off = self.offsets.get(&path).copied().unwrap_or(0);
            if len == off {
                continue;
            }
            if let Some((events, consumed)) = read_from(&path, off) {
                self.offsets.insert(path, off + consumed);
                out.extend(events);
            }
        }
        out.sort_by_key(|e| e.ts_ms);
        out
    }

    pub fn forget(&mut self, session: Id) {
        let path = self.dir().join(format!("{session}.jsonl"));
        self.offsets.remove(&path);
        let _ = std::fs::remove_file(path);
    }
}

/// Lê linhas completas a partir de `off`. Linha sem `\n` no fim (escrita em andamento) fica pra depois.
fn read_from(path: &Path, off: u64) -> Option<(Vec<HookEvent>, u64)> {
    let mut f = std::fs::File::open(path).ok()?;
    f.seek(SeekFrom::Start(off)).ok()?;
    let mut buf = String::new();
    f.read_to_string(&mut buf).ok()?;
    let complete = buf.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let events = buf[..complete]
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    Some((events, complete as u64))
}

/// Usado pelo `aic hook`: anexa o evento no arquivo da sessão.
pub fn append(e: &HookEvent) -> std::io::Result<()> {
    use std::io::Write;
    let path = e.file();
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let mut line = serde_json::to_string(e).map_err(std::io::Error::other)?;
    line.push('\n');
    f.write_all(line.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(name: &str) -> HookEvent {
        HookEvent {
            ts_ms: 1,
            aic_session: None,
            claude_session: "c".into(),
            event: name.into(),
            notification_type: None,
            message: None,
            permission_mode: None,
            agent_id: None,
            tool: None,
            cwd: None,
        }
    }

    #[test]
    fn ciclo_de_um_turno() {
        let mut l = Live::default();
        l.apply(&ev("SessionStart"));
        assert_eq!(l.state, SessionState::Idle);
        l.apply(&ev("UserPromptSubmit"));
        assert_eq!(l.state, SessionState::Working);
        let mut perm = ev("Notification");
        perm.notification_type = Some("permission_prompt".into());
        assert!(matches!(l.apply(&perm), Some(Effect::NeedsYou(_))));
        assert_eq!(l.state, SessionState::Waiting);
        // repetir não notifica de novo
        assert_eq!(l.apply(&perm), None);
        l.apply(&ev("PostToolUse"));
        assert_eq!(l.state, SessionState::Working);
        assert_eq!(l.apply(&ev("Stop")), Some(Effect::Finished));
        assert_eq!(l.state, SessionState::Done);
        l.seen();
        assert_eq!(l.state, SessionState::Idle);
    }

    #[test]
    fn contagem_de_agentes() {
        let mut l = Live::default();
        for id in ["a", "b"] {
            let mut e = ev("SubagentStart");
            e.agent_id = Some(id.into());
            l.apply(&e);
        }
        let mut e = ev("SubagentStop");
        e.agent_id = Some("a".into());
        l.apply(&e);
        assert_eq!(l.agents.len(), 1);
    }

    #[test]
    fn tail_le_so_linhas_novas_e_completas() {
        let dir = std::env::temp_dir().join(format!("aic-ev-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s.jsonl");
        let line = serde_json::to_string(&ev("Stop")).unwrap();
        std::fs::write(&file, format!("{line}\n{line}")).unwrap(); // 2ª incompleta
        let mut tail = EventTail::with_dir(dir.clone());
        assert_eq!(tail.poll().len(), 1);
        assert_eq!(tail.poll().len(), 0);
        std::fs::write(&file, format!("{line}\n{line}\n")).unwrap();
        assert_eq!(tail.poll().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
