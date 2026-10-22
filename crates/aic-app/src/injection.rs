//! O que o AIC injeta em cada sessão do Claude, sem tocar no `~/.claude` global:
//!
//! - `--settings <data>/claude-settings.json`: hooks (todos `async`, só observam) e a
//!   statusLine apontando pro `aic statusline`, que grava o snapshot pro AIC e não imprime nada:
//!   dentro do AIC o terminal fica sem statusline (os dados vão pra barra de cima). Fora do AIC,
//!   a sua statusline continua a mesma.
//! - `--plugin-dir <plugin>`: skills, commands e agents do AIC (`claude/` do repo).

use std::path::PathBuf;

use aic_core::claude::Injection;
use aic_core::paths;
use serde_json::json;

/// Eventos observados. Nome desconhecido é ignorado pelo Claude Code (não quebra o arquivo).
pub const HOOK_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
    "PermissionDenied",
    "Notification",
    "SubagentStart",
    "SubagentStop",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

pub fn exe() -> String {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "aic".into())
}

pub fn settings_json(exe: &str) -> serde_json::Value {
    let hook = |_: &str| json!([{ "hooks": [{ "type": "command", "command": format!("'{exe}' hook"), "timeout": 10, "async": true }] }]);
    let hooks: serde_json::Map<String, serde_json::Value> = HOOK_EVENTS.iter().map(|e| (e.to_string(), hook(e))).collect();
    json!({
        "hooks": hooks,
        "statusLine": { "type": "command", "command": format!("'{exe}' statusline"), "refreshInterval": 15 }
    })
}

/// Diretório do plugin: env > instalado pelo setup > `claude/` ao lado do código (dev).
pub fn plugin_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("AIC_PLUGIN_DIR").map(PathBuf::from) {
        return Some(p);
    }
    let installed = paths::data_dir().join("plugin");
    if installed.join(".claude-plugin/plugin.json").exists() {
        return Some(installed);
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../claude");
    dev.join(".claude-plugin/plugin.json").exists().then(|| dev.canonicalize().unwrap_or(dev))
}

/// Escreve o settings gerado e devolve o que passar pro `claude`.
pub fn prepare() -> Injection {
    let _ = paths::ensure_dirs();
    let path = paths::data_dir().join("claude-settings.json");
    let body = serde_json::to_string_pretty(&settings_json(&exe())).unwrap_or_default();
    let settings = match std::fs::write(&path, body) {
        Ok(()) => Some(path),
        Err(e) => {
            eprintln!("aic: não consegui gravar {}: {e}", path.display());
            None
        }
    };
    Injection { settings, plugin_dir: plugin_dir() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_tem_todos_os_hooks_async() {
        let v = settings_json("/x/aic");
        for e in HOOK_EVENTS {
            let h = &v["hooks"][e][0]["hooks"][0];
            assert_eq!(h["async"], true, "{e}");
            assert_eq!(h["command"], "'/x/aic' hook");
        }
        assert_eq!(v["statusLine"]["command"], "'/x/aic' statusline");
    }
}
