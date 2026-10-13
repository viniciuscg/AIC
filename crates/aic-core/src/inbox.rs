//! Caixa de entrada de comandos: o `aic` de linha de comando (usado pelo Layne, por você ou por
//! um atalho do i3) pede coisas ao app aberto.
//!
//! Cada pedido é um `.json` em `~/.local/state/aic/inbox/`; o app lê a pasta a cada ~300ms,
//! executa e apaga. A resposta (ok/erro + texto) vai pra `replies/<id>.json`, que o
//! `aic … --wait` espera. O app grava o próprio pid em `app.pid` pra o CLI saber se ele está aberto.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Cria um workspace (e vai pra ele, se `switch`).
    NewWorkspace { name: String, #[serde(default)] switch: bool },
    /// Vai pro workspace (nome exato, depois prefixo, sem diferenciar maiúsculas).
    GotoWorkspace { name: String },
    /// Abre um terminal num bloco novo.
    NewSession {
        /// Workspace pelo nome (criado se não existir). Vazio = o atual.
        #[serde(default)]
        workspace: Option<String>,
        #[serde(default)]
        name: Option<String>,
        /// Repo pelo nome (das pastas de projetos).
        #[serde(default)]
        repo: Option<String>,
        #[serde(default)]
        cwd: Option<PathBuf>,
        /// Shell em vez de Claude.
        #[serde(default)]
        shell: bool,
        /// Cria um worktree nesta branch (precisa de repo ou cwd num repo git).
        #[serde(default)]
        branch: Option<String>,
        /// Primeira mensagem pro Claude.
        #[serde(default)]
        prompt: Option<String>,
        /// Tarefa pra vincular (prefixo do id, como no `aic done`).
        #[serde(default)]
        task: Option<String>,
    },
    /// "Abrir card": com `auto`, busca o card e monta o bloco sem perguntar (repos sugeridos ou os de
    /// `repos`); sem `auto`, abre o diálogo já com o link, buscando.
    OpenCard {
        link: String,
        #[serde(default)]
        workspace: Option<String>,
        #[serde(default)]
        repos: Vec<String>,
        #[serde(default)]
        auto: bool,
    },
    /// Escreve no terminal do Layne (e manda, se `send`).
    Say { text: String, #[serde(default)] send: bool },
    /// Liga/desliga a gravação de voz pro Layne.
    Voice,
    /// Mostra o painel do Layne (traz a janela pra frente).
    Show,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub id: String,
    pub at_ms: u64,
    pub cmd: Command,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reply {
    pub ok: bool,
    pub message: String,
}

pub fn inbox_dir() -> PathBuf {
    paths::state_dir().join("inbox")
}

pub fn replies_dir() -> PathBuf {
    paths::state_dir().join("replies")
}

fn pid_path() -> PathBuf {
    paths::state_dir().join("app.pid")
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Grava atômico (tmp + rename): o leitor nunca vê arquivo pela metade.
fn write_atomic(path: &Path, body: &[u8]) -> Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Enfileira um comando. Devolve o id (pra esperar a resposta).
pub fn send(cmd: Command) -> Result<String> {
    send_in(&inbox_dir(), cmd)
}

fn send_in(dir: &Path, cmd: Command) -> Result<String> {
    let id = crate::new_id().simple().to_string();
    let env = Envelope { id: id.clone(), at_ms: now_ms(), cmd };
    let path = dir.join(format!("{:013}-{id}.json", env.at_ms));
    write_atomic(&path, &serde_json::to_vec(&env)?).with_context(|| format!("gravando {}", path.display()))?;
    Ok(id)
}

/// Lê e apaga os comandos pendentes, na ordem em que chegaram.
pub fn drain() -> Vec<Envelope> {
    drain_in(&inbox_dir())
}

fn drain_in(dir: &Path) -> Vec<Envelope> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok();
            let _ = std::fs::remove_file(&p);
            serde_json::from_str(&text?).ok()
        })
        .collect()
}

pub fn reply(id: &str, ok: bool, message: impl Into<String>) {
    let r = Reply { ok, message: message.into() };
    if let Ok(body) = serde_json::to_vec(&r) {
        let _ = write_atomic(&replies_dir().join(format!("{id}.json")), &body);
    }
}

/// Espera a resposta do app (e apaga o arquivo). `None` = estourou o tempo.
pub fn wait_reply(id: &str, timeout: Duration) -> Option<Reply> {
    let path = replies_dir().join(format!("{id}.json"));
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Ok(text) = std::fs::read_to_string(&path) {
            let _ = std::fs::remove_file(&path);
            return serde_json::from_str(&text).ok();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// O app marca que está aberto (chamado ao iniciar).
pub fn mark_running() {
    let _ = write_atomic(&pid_path(), std::process::id().to_string().as_bytes());
}

pub fn clear_running() {
    if std::fs::read_to_string(pid_path()).ok().and_then(|s| s.trim().parse::<u32>().ok()) == Some(std::process::id()) {
        let _ = std::fs::remove_file(pid_path());
    }
}

/// Tem um AIC aberto? (pid gravado e o processo ainda vivo)
pub fn app_running() -> bool {
    let Some(pid) = std::fs::read_to_string(pid_path()).ok().and_then(|s| s.trim().parse::<u32>().ok()) else { return false };
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manda_e_drena_em_ordem() {
        let dir = std::env::temp_dir().join(format!("aic-inbox-{}", crate::new_id().simple()));
        let a = send_in(&dir, Command::NewWorkspace { name: "Pessoal".into(), switch: true }).unwrap();
        std::thread::sleep(Duration::from_millis(2));
        let b = send_in(&dir, Command::Say { text: "oi".into(), send: false }).unwrap();
        let got = drain_in(&dir);
        assert_eq!(got.iter().map(|e| e.id.clone()).collect::<Vec<_>>(), vec![a, b]);
        assert_eq!(got[0].cmd, Command::NewWorkspace { name: "Pessoal".into(), switch: true });
        assert!(drain_in(&dir).is_empty(), "drenar apaga");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn json_com_campos_opcionais() {
        let c: Command = serde_json::from_str(r#"{"cmd":"new_session","repo":"api"}"#).unwrap();
        assert!(matches!(c, Command::NewSession { repo: Some(r), shell: false, .. } if r == "api"));
        let c: Command = serde_json::from_str(r#"{"cmd":"voice"}"#).unwrap();
        assert_eq!(c, Command::Voice);
    }
}
