//! Caminhos XDG do Plankton.
//!
//! - config: `~/.config/aic` (config.toml, keybinds.toml, secrets.toml)
//! - data:   `~/.local/share/aic` (plankton.db, sons, cache)
//! - state:  `~/.local/state/aic` (events/ dos hooks, statusline/ por sessão)

use std::path::PathBuf;

fn home() -> PathBuf {
    directories::BaseDirs::new()
        .map(|b| b.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
        .join("aic")
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("AIC_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| xdg("XDG_CONFIG_HOME", ".config"))
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("AIC_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| xdg("XDG_DATA_HOME", ".local/share"))
}

pub fn state_dir() -> PathBuf {
    std::env::var_os("AIC_STATE_DIR").map(PathBuf::from).unwrap_or_else(|| xdg("XDG_STATE_HOME", ".local/state"))
}

pub fn db_path() -> PathBuf {
    data_dir().join("plankton.db")
}

pub fn events_dir() -> PathBuf {
    state_dir().join("events")
}

pub fn statusline_dir() -> PathBuf {
    state_dir().join("statusline")
}

pub fn sounds_dir() -> PathBuf {
    data_dir().join("sounds")
}

pub fn claude_dir() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| home().join(".claude"))
}

pub fn home_dir() -> PathBuf {
    home()
}

/// Cria os diretórios que o app e os hooks escrevem.
pub fn ensure_dirs() -> std::io::Result<()> {
    for d in [config_dir(), data_dir(), events_dir(), statusline_dir(), sounds_dir()] {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}

/// `~/foo` → caminho curto pra exibir.
pub fn pretty(p: &std::path::Path) -> String {
    let h = home();
    match p.strip_prefix(&h) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}
