//! Persistência via tmux: cada terminal do AIC roda numa sessão de um servidor tmux só
//! do AIC (`tmux -L aic`). Fechar a janela só desconecta o cliente; o processo continua.
//! Ao abrir de novo, `new-session -A` reconecta na mesma sessão.
//!
//! A config do servidor (`~/.local/share/aic/tmux.conf`) deixa o tmux invisível: sem barra,
//! sem prefixo (nenhuma tecla é interceptada), mouse nativo do tmux (roda rola, arrastar copia),
//! histórico grande.
//! O seu tmux pessoal (servidor padrão) não é afetado.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use aic_core::{Id, paths};

pub const SOCKET: &str = "aic";

const CONF: &str = r#"# Gerado pelo AIC — servidor tmux próprio (tmux -L aic). Não edite: é reescrito ao abrir.
set -g status off
set -g prefix None
set -g prefix2 None
unbind-key -a -T prefix
set -g mouse on
set -g history-limit 100000
set -sg escape-time 0
set -g focus-events on
set -g default-terminal "tmux-256color"
set -as terminal-features ",xterm-256color:RGB:extkeys:clipboard"
set -g extended-keys on
set -g allow-passthrough on
set -g set-clipboard on
set -g remain-on-exit off
set -g destroy-unattached off
set -g detach-on-destroy on
set -g window-size latest
set -g aggressive-resize on
# mouse: arrastar seleciona e copia (vai pro clipboard via OSC 52), roda rola o histórico
"#;

pub fn conf_path() -> PathBuf {
    paths::data_dir().join("tmux.conf")
}

/// tmux instalado? (cacheado)
pub fn available() -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| {
        Command::new("tmux")
            .arg("-V")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

/// Grava a config e recarrega no servidor, se ele já estiver de pé.
pub fn prepare() {
    let _ = paths::ensure_dirs();
    let _ = std::fs::write(conf_path(), CONF);
    let _ = tmux(&["source-file", &conf_path().to_string_lossy()]);
}

/// Nome da sessão tmux de um terminal do AIC.
pub fn session_name(id: Id) -> String {
    format!("aic-{}", id.simple())
}

fn tmux(args: &[&str]) -> Option<String> {
    let out = Command::new("tmux")
        .args(["-L", SOCKET, "-f"])
        .arg(conf_path())
        .args(args)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

/// A sessão ainda está rodando no servidor?
pub fn alive(id: Id) -> bool {
    tmux(&["has-session", "-t", &format!("={}", session_name(id))]).is_some()
}

/// Todas as sessões do AIC vivas no servidor.
pub fn list() -> Vec<String> {
    tmux(&["list-sessions", "-F", "#{session_name}"])
        .map(|s| s.lines().map(String::from).collect())
        .unwrap_or_default()
}

pub fn kill(id: Id) {
    let _ = tmux(&["kill-session", "-t", &format!("={}", session_name(id))]);
}

/// Mata o servidor inteiro do AIC (todas as sessões).
pub fn kill_all() {
    let _ = tmux(&["kill-server"]);
}

/// Monta `program` + `args` que conectam (ou criam) a sessão tmux rodando `inner`.
/// `env` vai pra sessão via `-e` (só vale na criação).
pub fn wrap(
    id: Id,
    cwd: &std::path::Path,
    env: &[(String, String)],
    inner_program: &str,
    inner_args: &[String],
) -> (String, Vec<String>) {
    let mut args: Vec<String> = vec![
        "-L".into(),
        SOCKET.into(),
        "-f".into(),
        conf_path().to_string_lossy().into(),
        "new-session".into(),
        "-A".into(),
    ];
    args.extend([
        "-s".into(),
        session_name(id),
        "-c".into(),
        cwd.to_string_lossy().into(),
    ]);
    for (k, v) in env {
        args.push("-e".into());
        args.push(format!("{k}={v}"));
    }
    args.push("--".into());
    args.push(inner_program.into());
    args.extend(inner_args.iter().cloned());
    ("tmux".into(), args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_monta_new_session_attach() {
        let id = aic_core::new_id();
        let (p, a) = wrap(
            id,
            std::path::Path::new("/tmp"),
            &[("AIC_SESSION".into(), "x".into())],
            "claude",
            &["--resume".into(), "abc".into()],
        );
        assert_eq!(p, "tmux");
        let s = a.join(" ");
        assert!(s.contains("-L aic"));
        assert!(s.contains("new-session -A -s aic-"));
        assert!(s.contains("-e AIC_SESSION=x"));
        assert!(s.ends_with("-- claude --resume abc"));
    }

    #[test]
    fn nome_sem_caracteres_proibidos() {
        let n = session_name(aic_core::new_id());
        assert!(n.starts_with("aic-"));
        assert!(!n.contains('.') && !n.contains(':') && !n[4..].contains('-'));
    }
}
