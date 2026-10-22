//! Ponte com o Claude Code: montar o comando da sessão, achar o transcript para
//! `--resume` e ler o snapshot que o wrapper de statusline grava.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Session;
use crate::paths;

/// Snapshot do JSON que o Claude Code entrega à statusline. É a única fonte dos
/// limites de 5h/semana e do % de contexto.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StatusSnapshot {
    pub session_id: String,
    pub model: String,
    pub effort: Option<String>,
    pub cwd: Option<PathBuf>,
    pub context_pct: Option<f32>,
    pub context_size: Option<u64>,
    pub cost_usd: f64,
    pub duration_ms: u64,
    pub lines_added: u64,
    pub lines_removed: u64,
    pub five_hour: Option<Limit>,
    pub seven_day: Option<Limit>,
    pub spend: Option<Limit>,
    pub permission_mode: Option<String>,
    /// Epoch (s) de quando o snapshot foi gravado.
    pub at: i64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Limit {
    pub used_pct: f32,
    /// Epoch (s) do reset.
    pub resets_at: Option<i64>,
}

fn limit(v: &Value) -> Option<Limit> {
    let pct = v.get("used_percentage")?.as_f64()? as f32;
    let resets_at = v
        .get("resets_at")
        .and_then(|r| r.as_i64().or_else(|| r.as_str().and_then(parse_ts)));
    Some(Limit {
        used_pct: pct,
        resets_at,
    })
}

fn parse_ts(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp())
        .or_else(|| s.parse().ok())
}

impl StatusSnapshot {
    pub fn from_json(v: &Value) -> Self {
        let s = |p: &str| v.pointer(p).and_then(Value::as_str).map(String::from);
        let f = |p: &str| v.pointer(p).and_then(Value::as_f64);
        let rl = v.get("rate_limits");
        Self {
            session_id: s("/session_id").unwrap_or_default(),
            model: s("/model/display_name")
                .or_else(|| s("/model/id"))
                .unwrap_or_else(|| "?".into()),
            effort: s("/effort/level"),
            cwd: s("/workspace/current_dir")
                .or_else(|| s("/cwd"))
                .map(PathBuf::from),
            context_pct: f("/context_window/used_percentage").map(|x| x as f32),
            context_size: v
                .pointer("/context_window/context_window_size")
                .and_then(Value::as_u64),
            cost_usd: f("/cost/total_cost_usd").unwrap_or(0.0),
            duration_ms: v
                .pointer("/cost/total_duration_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            lines_added: v
                .pointer("/cost/total_lines_added")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            lines_removed: v
                .pointer("/cost/total_lines_removed")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            five_hour: rl.and_then(|r| r.get("five_hour")).and_then(limit),
            seven_day: rl.and_then(|r| r.get("seven_day")).and_then(limit),
            spend: rl.and_then(|r| r.get("spend_limit")).and_then(limit),
            permission_mode: s("/permission_mode").or_else(|| s("/permissionMode")),
            at: chrono::Utc::now().timestamp(),
        }
    }

    pub fn path_for(session_id: &str) -> PathBuf {
        paths::statusline_dir().join(format!("{session_id}.json"))
    }

    pub fn load(session_id: &str) -> Option<Self> {
        let text = std::fs::read_to_string(Self::path_for(session_id)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// O snapshot mais recente de qualquer sessão (limites são da conta, não da sessão).
    pub fn latest() -> Option<Self> {
        let dir = std::fs::read_dir(paths::statusline_dir()).ok()?;
        dir.flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .filter_map(|e| {
                serde_json::from_str::<Self>(&std::fs::read_to_string(e.path()).ok()?).ok()
            })
            .filter(|s| s.five_hour.is_some() || s.seven_day.is_some())
            .max_by_key(|s| s.at)
    }
}

/// Diretório de projeto do Claude Code para um cwd (`/a/b.c` → `-a-b-c`).
pub fn project_dir(cwd: &Path) -> PathBuf {
    let enc: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    paths::claude_dir().join("projects").join(enc)
}

/// Acha o `.jsonl` de uma sessão: primeiro no projeto do cwd, depois em qualquer projeto.
pub fn find_transcript(cwd: &Path, session_id: &str) -> Option<PathBuf> {
    let file = format!("{session_id}.jsonl");
    let direct = project_dir(cwd).join(&file);
    if direct.exists() {
        return Some(direct);
    }
    std::fs::read_dir(paths::claude_dir().join("projects"))
        .ok()?
        .flatten()
        .map(|e| e.path().join(&file))
        .find(|p| p.exists())
}

/// Config global do Claude Code (`~/.claude.json`, ou dentro do `CLAUDE_CONFIG_DIR`).
fn global_config_path() -> PathBuf {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(d) => PathBuf::from(d).join(".claude.json"),
        None => paths::home_dir().join(".claude.json"),
    }
}

/// Marca a pasta como confiável no Claude Code, como se o usuário tivesse respondido "Yes, I
/// trust this folder". Serve pras pastas que o próprio AIC cria (a do Layne): sem isso o Claude
/// abre parado na pergunta. Só escreve se a pasta (ou uma acima dela) ainda não for confiável.
pub fn ensure_trusted(dir: &Path) -> anyhow::Result<()> {
    let path = global_config_path();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|_| "{}".into());
    let mut v: Value = serde_json::from_str(&text)?;
    let trusted = |v: &Value, d: &Path| {
        v.pointer("/projects")
            .and_then(|p| p.get(d.to_string_lossy().as_ref()))
            .and_then(|p| p.get("hasTrustDialogAccepted"))
            .and_then(Value::as_bool)
            == Some(true)
    };
    if dir.ancestors().any(|d| trusted(&v, d)) {
        return Ok(());
    }
    let root = v
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("{} não é um objeto", path.display()))?;
    let projects = root
        .entry("projects")
        .or_insert_with(|| Value::Object(Default::default()));
    let entry = projects
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("projects não é um objeto"))?
        .entry(dir.to_string_lossy().to_string())
        .or_insert_with(|| Value::Object(Default::default()));
    if let Some(o) = entry.as_object_mut() {
        o.insert("hasTrustDialogAccepted".into(), Value::Bool(true));
    }
    // grava num temporário e renomeia: o Claude Code lê esse arquivo o tempo todo
    let tmp = path.with_extension(format!("json.aic-{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(&v)?)?;
    if let Ok(m) = std::fs::metadata(&path) {
        let _ = std::fs::set_permissions(&tmp, m.permissions());
    }
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Onde ficam o plugin e o settings que o AIC injeta nas sessões.
#[derive(Debug, Clone)]
pub struct Injection {
    /// `--settings` (hooks + statusLine)
    pub settings: Option<PathBuf>,
    /// `--plugin-dir` (skills, commands, agents)
    pub plugin_dir: Option<PathBuf>,
}

/// Monta `program` + `args` para abrir/retomar a sessão no Claude Code.
pub fn command(
    session: &Session,
    inj: &Injection,
    extra_prompt: Option<&str>,
) -> (String, Vec<String>) {
    let id = session.claude_session.to_string();
    let mut args = Vec::new();
    if find_transcript(&session.cwd, &id).is_some() {
        args.push("--resume".into());
    } else {
        args.push("--session-id".into());
    }
    args.push(id);
    args.push("--name".into());
    args.push(session.name.clone());
    if let Some(s) = &inj.settings {
        args.push("--settings".into());
        args.push(s.to_string_lossy().into());
    }
    if let Some(p) = &inj.plugin_dir {
        args.push("--plugin-dir".into());
        args.push(p.to_string_lossy().into());
    }
    if let Some(p) = extra_prompt.filter(|p| !p.is_empty()) {
        args.push("--append-system-prompt".into());
        args.push(p.to_string());
    }
    args.extend(session.claude_args.iter().cloned());
    if let Some(p) = session
        .initial_prompt
        .as_deref()
        .filter(|p| !p.trim().is_empty())
    {
        args.push("--".into());
        args.push(p.to_string());
    }
    ("claude".into(), args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_da_statusline() {
        let v: Value = serde_json::from_str(
            r#"{
            "session_id": "abc",
            "model": {"id": "claude-opus-5-5", "display_name": "Opus 5.5"},
            "effort": {"level": "medium"},
            "workspace": {"current_dir": "/home/x/dev/api-pagamentos"},
            "context_window": {"used_percentage": 17.4, "context_window_size": 1000000},
            "cost": {"total_cost_usd": 3.83, "total_duration_ms": 63900000},
            "rate_limits": {
                "five_hour": {"used_percentage": 15, "resets_at": 1791580000},
                "seven_day": {"used_percentage": 2.2, "resets_at": "2026-10-15T20:00:00Z"}
            }
        }"#,
        )
        .unwrap();
        let s = StatusSnapshot::from_json(&v);
        assert_eq!(s.model, "Opus 5.5");
        assert_eq!(s.effort.as_deref(), Some("medium"));
        assert_eq!(s.context_size, Some(1_000_000));
        assert_eq!(s.five_hour.unwrap().used_pct, 15.0);
        assert_eq!(s.five_hour.unwrap().resets_at, Some(1791580000));
        assert!(s.seven_day.unwrap().resets_at.is_some());
        assert!((s.cost_usd - 3.83).abs() < 1e-9);
    }

    #[test]
    fn diretorio_de_projeto() {
        let p = project_dir(Path::new("/home/dev/dev/api-pagamentos"));
        assert!(p.ends_with("-home-dev-dev-api-pagamentos"));
    }
}
