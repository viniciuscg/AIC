//! Caminhos XDG do AIC.
//!
//! - config: `~/.config/aic` (config.toml, keybinds.toml, secrets.toml, .env)
//! - data:   `~/.local/share/aic` (aic.db, sons, cache)
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
    data_dir().join("aic.db")
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

pub fn dotenv_path() -> PathBuf {
    config_dir().join(".env")
}

/// Carrega `~/.config/aic/.env` no ambiente do processo. Não sobrescreve o que já
/// estiver definido. Chame no início do `main`, antes de abrir threads.
pub fn load_dotenv() {
    let Ok(text) = std::fs::read_to_string(dotenv_path()) else { return };
    for (k, v) in parse_dotenv(&text) {
        if std::env::var_os(&k).is_none() {
            // SAFETY: chamado no início do main, ainda com uma thread só.
            unsafe { std::env::set_var(k, v) };
        }
    }
}

/// `KEY=VALUE` por linha; ignora vazias e `#`; aceita `export ` e aspas simples/duplas.
pub fn parse_dotenv(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let (k, v) = line.split_once('=')?;
            let k = k.trim();
            if k.is_empty() || !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return None;
            }
            let v = v.trim();
            let v = match v.chars().next() {
                Some(q @ ('"' | '\'')) => v[1..].split(q).next().unwrap_or(""),
                // sem aspas: comentário no fim da linha (` # ...`) sai
                _ => v.split(" #").next().unwrap_or("").trim_end(),
            };
            Some((k.to_string(), v.to_string()))
        })
        .collect()
}

/// `~/foo` → caminho curto pra exibir.
pub fn pretty(p: &std::path::Path) -> String {
    let h = home();
    match p.strip_prefix(&h) {
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => p.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_dotenv;

    #[test]
    fn dotenv() {
        let text = "# comentário\n\nNOTION_TOKEN=abc\nexport A = \"com espaço\"\nB='x # y'\nC=valor # nota\nD=\n=sem\nruim chave=1\n";
        let v = parse_dotenv(text);
        let get = |k: &str| v.iter().find(|(a, _)| a == k).map(|(_, b)| b.as_str());
        assert_eq!(get("NOTION_TOKEN"), Some("abc"));
        assert_eq!(get("A"), Some("com espaço"));
        assert_eq!(get("B"), Some("x # y"));
        assert_eq!(get("C"), Some("valor"));
        assert_eq!(get("D"), Some(""));
        assert_eq!(v.len(), 5);
    }
}
