//! Subcomandos de linha de comando (rodam sem abrir janela):
//!
//! ```text
//! aic add [--session] "corrigir juros #api !alta @hoje"   # nova tarefa
//! aic tasks [--all] [--json]                    # lista tarefas
//! aic done <id>                                 # conclui (prefixo do id serve)
//! aic hook                                      # (hooks do Claude) grava evento
//! aic statusline                                # (statusLine do Claude) grava snapshot + chama a sua
//! aic export-icon <dir>                         # PNGs do ícone (16..512) em <dir>
//! ```

use std::io::{Read, Write};
use std::process::{Command, Stdio};

use anyhow::{Result, bail};
use chrono::Local;
use aic_core::claude::StatusSnapshot;
use aic_core::events::{self, HookEvent};
use aic_core::quickadd;
use aic_core::{Config, Id, Store, TaskStatus, paths};

pub enum Cli {
    Gui { fresh: bool },
    Run(Box<dyn FnOnce() -> Result<()>>),
}

pub fn parse(args: Vec<String>) -> Cli {
    let mut it = args.into_iter().skip(1);
    let first = it.next();
    match first.as_deref() {
        Some("add" | "a" | "t") => {
            let (flags, text): (Vec<String>, Vec<String>) = it.partition(|a| a == "--session");
            Cli::Run(Box::new(move || add(&text.join(" "), !flags.is_empty())))
        }
        Some("tasks" | "ls") => {
            let rest: Vec<String> = it.collect();
            Cli::Run(Box::new(move || tasks(rest.iter().any(|a| a == "--all"), rest.iter().any(|a| a == "--json"))))
        }
        Some("done") => {
            let id = it.next().unwrap_or_default();
            Cli::Run(Box::new(move || done(&id)))
        }
        Some("hook") => Cli::Run(Box::new(hook)),
        Some("statusline") => Cli::Run(Box::new(statusline)),
        Some("export-icon") => {
            let dir = it.next().unwrap_or_default();
            Cli::Run(Box::new(move || export_icon(&dir)))
        }
        Some("--version" | "-V" | "version") => Cli::Run(Box::new(|| {
            println!("aic {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        })),
        Some("--help" | "-h" | "help") => Cli::Run(Box::new(|| {
            println!("{}", HELP);
            Ok(())
        })),
        _ => {
            let mut fresh = false;
            let mut all: Vec<String> = first.into_iter().collect();
            all.extend(it);
            for a in &all {
                if a == "--fresh" {
                    fresh = true;
                }
            }
            Cli::Gui { fresh }
        }
    }
}

const HELP: &str = "AIC — central de vibecode pessoal

Uso:
  aic [--fresh]                                          abre o app
  aic add \"texto #repo @hoje !alta +workspace ^DEV-1 ~tag\"   cria tarefa
  aic tasks [--all] [--json]                             lista tarefas
  aic done <id>                                          conclui tarefa
  aic export-icon <dir>                                  gera <dir>/aic-<N>.png (16..512)
  aic hook | statusline                                  (usados pelo Claude Code)";

fn store() -> Result<Store> {
    paths::ensure_dirs()?;
    Store::open(&paths::db_path())
}

fn add(text: &str, link_session: bool) -> Result<()> {
    if text.trim().is_empty() {
        bail!("uso: aic add \"texto da tarefa\"");
    }
    let cfg = Config::load();
    let store = store()?;
    let workspaces = store.workspaces()?;
    let ws_names: Vec<String> = workspaces.iter().map(|w| w.name.clone()).collect();
    let parsed = quickadd::parse(text, Local::now().date_naive(), &cfg.repos(), &ws_names);
    if parsed.title.trim().is_empty() {
        bail!("a tarefa precisa de um título além dos marcadores");
    }
    let chips: Vec<String> = parsed.chips.iter().map(|c| c.label.clone()).collect();
    let ws_name = parsed.workspace.clone();
    let mut task = parsed.into_task();

    // Chamado de dentro de uma sessão do AIC (/tarefa): já vincula.
    let env_id = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<Id>().ok());
    task.workspace = ws_name
        .and_then(|n| workspaces.iter().find(|w| w.name == n).map(|w| w.id))
        .or_else(|| env_id("AIC_WORKSPACE"));
    if link_session {
        task.session = env_id("AIC_SESSION");
    }
    store.save_task(&task)?;
    let short = &task.id.to_string()[..8];
    if chips.is_empty() {
        println!("✓ tarefa criada [{short}]: {}", task.title);
    } else {
        println!("✓ tarefa criada [{short}]: {}  ({})", task.title, chips.join(" · "));
    }
    Ok(())
}

fn tasks(all: bool, json: bool) -> Result<()> {
    let store = store()?;
    let mut list = store.tasks()?;
    list.retain(|t| all || !matches!(t.status, TaskStatus::Done | TaskStatus::Archived));
    list.sort_by(|a, b| a.status.cmp(&b.status).then(b.priority.cmp(&a.priority)).then(a.rank.total_cmp(&b.rank)));
    if json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(());
    }
    if list.is_empty() {
        println!("nenhuma tarefa aberta. crie uma com: aic add \"...\"");
    }
    let today = Local::now().date_naive();
    for t in list {
        let mut extra = Vec::new();
        if let Some(r) = &t.repo {
            extra.push(format!("#{r}"));
        }
        if let Some(d) = t.due {
            extra.push(format!("@{}", quickadd::due_label(d, today)));
        }
        if let Some(k) = &t.card_key {
            extra.push(format!("^{k}"));
        }
        println!(
            "{}  {:<9} {:<7} {}  {}",
            &t.id.to_string()[..8],
            t.status.label(),
            t.priority.label(),
            t.title,
            extra.join(" ")
        );
    }
    Ok(())
}

fn done(prefix: &str) -> Result<()> {
    if prefix.len() < 4 {
        bail!("passe pelo menos 4 caracteres do id (veja `aic tasks`)");
    }
    let store = store()?;
    let matches: Vec<_> = store.tasks()?.into_iter().filter(|t| t.id.to_string().starts_with(prefix)).collect();
    match matches.as_slice() {
        [t] => {
            let mut t = t.clone();
            t.status = TaskStatus::Done;
            t.touch();
            store.save_task(&t)?;
            println!("✓ concluída: {}", t.title);
            Ok(())
        }
        [] => bail!("nenhuma tarefa com id {prefix}…"),
        _ => bail!("mais de uma tarefa com id {prefix}…; use mais caracteres"),
    }
}

/// Hook do Claude Code: nunca falha alto (não pode atrapalhar a sessão).
fn hook() -> Result<()> {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&input) else { return Ok(()) };
    let session = std::env::var("AIC_SESSION").ok().and_then(|s| s.parse().ok());
    let e = HookEvent::from_hook_input(&v, session, aic_term::now_ms());
    let _ = events::append(&e);
    Ok(())
}

/// Wrapper da statusLine: grava o snapshot e repassa o JSON pra statusline do usuário.
fn statusline() -> Result<()> {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&input) {
        let snap = StatusSnapshot::from_json(&v);
        if !snap.session_id.is_empty() {
            let _ = paths::ensure_dirs();
            let path = StatusSnapshot::path_for(&snap.session_id);
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, serde_json::to_vec(&snap)?).is_ok() {
                let _ = std::fs::rename(tmp, path);
            }
        }
    }

    let user = Config::load().user_statusline.or_else(crate::injection::user_statusline_from_claude_settings);
    if let Some(cmd) = user {
        let cmd = cmd.replace('~', &paths::home_dir().to_string_lossy());
        if let Ok(mut child) = Command::new("sh").arg("-c").arg(&cmd).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(input.as_bytes());
            }
            if let Ok(out) = child.wait_with_output() {
                let _ = std::io::stdout().write_all(&out.stdout);
                return Ok(());
            }
        }
    }
    println!("󰧑 aic");
    Ok(())
}

/// Tamanhos do tema hicolor.
pub const ICON_SIZES: [u32; 7] = [16, 32, 48, 64, 128, 256, 512];

/// Gera `<dir>/aic-<N>.png` para cada tamanho de `ICON_SIZES`.
fn export_icon(dir: &str) -> Result<()> {
    if dir.is_empty() {
        bail!("uso: aic export-icon <dir>");
    }
    let dir = std::path::Path::new(dir);
    std::fs::create_dir_all(dir)?;
    for size in ICON_SIZES {
        let path = dir.join(format!("aic-{size}.png"));
        image::save_buffer(&path, &crate::icon_rgba(size), size, size, image::ExtendedColorType::Rgba8)?;
        println!("{}", path.display());
    }
    Ok(())
}
