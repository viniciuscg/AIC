//! Subcomandos de linha de comando (rodam sem abrir janela):
//!
//! ```text
//! aic add "pagar o boleto @sexta"               # nova tarefa
//! aic tasks [--all] [--json]                    # lista tarefas
//! aic done <id>                                 # conclui (prefixo do id serve)
//! aic ws ls|new|go …                            # workspaces (new/go pedem ao app aberto)
//! aic session [--repo r] [--ws w] [-- prompt]   # abre um Claude/shell no app aberto
//! aic sessions | repos                          # o que está aberto / repos conhecidos
//! aic anakin [--send] "texto" | voz | show       # fala com o Anakin (assistente)
//! aic hook                                      # (hooks do Claude) grava evento
//! aic statusline                                # (statusLine do Claude) grava o snapshot, sem imprimir
//! aic export-icon <dir>                         # PNGs do ícone (16..512) em <dir>
//! ```

use std::io::Read;

use aic_core::claude::StatusSnapshot;
use aic_core::events::{self, HookEvent};
use aic_core::inbox::{self, Command as Cmd};
use aic_core::quickadd;
use aic_core::{Config, Store, Task, paths};
use anyhow::{Result, bail};
use chrono::Local;

pub enum Cli {
    Gui { fresh: bool },
    Run(Box<dyn FnOnce() -> Result<()>>),
}

pub fn parse(args: Vec<String>) -> Cli {
    let mut it = args.into_iter().skip(1);
    let first = it.next();
    match first.as_deref() {
        Some("add" | "a" | "t") => {
            let rest: Vec<String> = it.collect();
            Cli::Run(Box::new(move || add(&rest.join(" "))))
        }
        Some("tasks" | "ls") => {
            let rest: Vec<String> = it.collect();
            Cli::Run(Box::new(move || {
                tasks(
                    rest.iter().any(|a| a == "--all"),
                    rest.iter().any(|a| a == "--json"),
                )
            }))
        }
        Some("done") => {
            let id = it.next().unwrap_or_default();
            Cli::Run(Box::new(move || done(&id)))
        }
        Some("ws" | "workspace" | "workspaces") => {
            let rest: Vec<String> = it.collect();
            Cli::Run(Box::new(move || workspace(rest)))
        }
        Some("session" | "sessao" | "sessão" | "s") => {
            let rest: Vec<String> = it.collect();
            Cli::Run(Box::new(move || session(rest)))
        }
        Some("sessions" | "sessoes" | "sessões") => {
            let json = it.any(|a| a == "--json");
            Cli::Run(Box::new(move || sessions(json)))
        }
        Some("repos") => Cli::Run(Box::new(repos)),
        Some("anakin") => {
            let rest: Vec<String> = it.collect();
            Cli::Run(Box::new(move || anakin(rest)))
        }
        Some("voz" | "voice") => Cli::Run(Box::new(|| ask(Cmd::Voice, 5).map(|_| ()))),
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

const HELP: &str = "aic (Anakin) — central de vibecode pessoal

Uso:
  aic [--fresh]                                          abre o app
  aic add \"texto @amanhã\"                                cria tarefa (a data é opcional)
  aic tasks [--all] [--json]                             lista tarefas
  aic done <id>                                          conclui tarefa
  aic ws ls [--json]                                     lista workspaces
  aic ws new <nome> [--go]                               cria workspace (--go: vai pra ele)
  aic ws go <nome>                                       vai pro workspace
  aic session [--ws W] [--repo R | --cwd P] [--branch B] [--name N] [--shell] [-- prompt]
                                                         abre um Claude (ou shell) numa sessão nova
  aic sessions [--json]                                  terminais abertos, por workspace
  aic repos                                              repos das pastas de projetos
  aic anakin [--send] \"texto\"                             escreve pro Anakin (assistente)
  aic anakin show | aic voz                               mostra o Anakin / liga-desliga a gravação
  aic export-icon <dir>                                  gera <dir>/aic-<N>.png (16..512)
  aic hook | statusline                                  (usados pelo Claude Code)";

/// Tira `--flag valor` (ou `--flag=valor`) de `args`.
fn take_opt(args: &mut Vec<String>, names: &[&str]) -> Option<String> {
    let i = args.iter().position(|a| {
        names.contains(&a.as_str()) || names.iter().any(|n| a.starts_with(&format!("{n}=")))
    })?;
    let a = args.remove(i);
    match a.split_once('=') {
        Some((_, v)) if a.starts_with("--") => Some(v.to_string()),
        _ => (i < args.len()).then(|| args.remove(i)),
    }
}

/// Tira `--flag` (sem valor) de `args`.
fn take_flag(args: &mut Vec<String>, names: &[&str]) -> bool {
    let before = args.len();
    args.retain(|a| !names.contains(&a.as_str()));
    args.len() != before
}

/// Manda um comando pro app aberto e espera a resposta (até `secs`).
fn ask(cmd: Cmd, secs: u64) -> Result<String> {
    if !inbox::app_running() {
        bail!("o AIC não está aberto (abra com `aic`)");
    }
    let id = inbox::send(cmd)?;
    match inbox::wait_reply(&id, std::time::Duration::from_secs(secs)) {
        Some(r) if r.ok => {
            println!("✓ {}", r.message);
            Ok(r.message)
        }
        Some(r) => bail!("{}", r.message),
        None => {
            bail!("o AIC não respondeu em {secs}s (o pedido fica na fila e roda quando ele puder)")
        }
    }
}

fn workspace(mut args: Vec<String>) -> Result<()> {
    let sub = if args.is_empty() {
        "ls".to_string()
    } else {
        args.remove(0)
    };
    match sub.as_str() {
        "ls" | "list" => {
            let store = store()?;
            let wss = store.workspaces()?;
            let sessions = store.sessions()?;
            if args.iter().any(|a| a == "--json") {
                let v: Vec<serde_json::Value> = wss
                    .iter()
                    .map(|w| {
                        serde_json::json!({
                            "id": w.id,
                            "name": w.name,
                            "sessions": sessions.iter().filter(|s| s.workspace == w.id).count(),
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&v)?);
                return Ok(());
            }
            if wss.is_empty() {
                println!("nenhum workspace salvo ainda (o app cria o \"Principal\" ao abrir)");
            }
            for (i, w) in wss.iter().enumerate() {
                let s = sessions.iter().filter(|s| s.workspace == w.id).count();
                println!("{}  {:<24} {s} terminais", i + 1, w.name);
            }
            Ok(())
        }
        "new" | "novo" | "add" => {
            let switch = take_flag(&mut args, &["--go", "-g"]);
            let name = args.join(" ");
            if name.trim().is_empty() {
                bail!("uso: aic ws new <nome> [--go]");
            }
            ask(Cmd::NewWorkspace { name, switch }, 10).map(|_| ())
        }
        "go" | "ir" => {
            let name = args.join(" ");
            if name.trim().is_empty() {
                bail!("uso: aic ws go <nome>");
            }
            ask(Cmd::GotoWorkspace { name }, 10).map(|_| ())
        }
        other => bail!("subcomando desconhecido: ws {other} (ls, new, go)"),
    }
}

fn session(mut args: Vec<String>) -> Result<()> {
    // tudo depois de `--` é a primeira mensagem pro Claude
    let prompt = args.iter().position(|a| a == "--").map(|i| {
        args.split_off(i)
            .into_iter()
            .skip(1)
            .collect::<Vec<_>>()
            .join(" ")
    });
    let workspace = take_opt(&mut args, &["--ws", "--workspace", "-w"]);
    let repo = take_opt(&mut args, &["--repo", "-r"]);
    let cwd = take_opt(&mut args, &["--cwd", "--dir", "-C"]).map(|p| {
        let p = p
            .strip_prefix("~/")
            .map(|r| paths::home_dir().join(r))
            .unwrap_or_else(|| p.into());
        std::path::absolute(&p).unwrap_or(p)
    });
    let branch = take_opt(&mut args, &["--branch", "-b"]);
    let name = take_opt(&mut args, &["--name", "--nome", "-n"]);
    let shell = take_flag(&mut args, &["--shell"]);
    if let Some(extra) = args.first() {
        bail!("argumento desconhecido: {extra} (veja `aic help`)");
    }
    ask(
        Cmd::NewSession {
            workspace,
            name,
            repo,
            cwd,
            shell,
            branch,
            prompt: prompt.filter(|p| !p.trim().is_empty()),
        },
        20,
    )
    .map(|_| ())
}

fn sessions(json: bool) -> Result<()> {
    let store = store()?;
    let wss = store.workspaces()?;
    let list = store.sessions()?;
    if json {
        let v: Vec<serde_json::Value> = list
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id, "name": s.name, "kind": s.kind, "cwd": s.cwd, "repo": s.repo,
                    "workspace": wss.iter().find(|w| w.id == s.workspace).map(|w| w.name.clone()),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&v)?);
        return Ok(());
    }
    for w in &wss {
        let mine: Vec<_> = list.iter().filter(|s| s.workspace == w.id).collect();
        println!("{} ({})", w.name, mine.len());
        for s in mine {
            let kind = match &s.kind {
                aic_core::SessionKind::Claude => "claude",
                aic_core::SessionKind::Shell => "shell",
                aic_core::SessionKind::Command(_) => "cmd",
            };
            println!(
                "  {}  {:<6} {:<28} {}",
                &s.id.to_string()[..8],
                kind,
                s.name,
                paths::pretty(&s.cwd)
            );
        }
    }
    if !inbox::app_running() {
        println!("(o AIC está fechado: esta é a lista salva da última vez)");
    }
    Ok(())
}

fn repos() -> Result<()> {
    for (name, path) in Config::load().repo_paths() {
        println!("{name:<32} {}", paths::pretty(&path));
    }
    Ok(())
}

fn anakin(mut args: Vec<String>) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("show" | "mostrar") => ask(Cmd::Show, 5).map(|_| ()),
        Some("voz" | "voice") => ask(Cmd::Voice, 5).map(|_| ()),
        _ => {
            let send = take_flag(&mut args, &["--send", "-s"]);
            let text = args.join(" ");
            if text.trim().is_empty() {
                return ask(Cmd::Show, 5).map(|_| ());
            }
            ask(Cmd::Say { text, send }, 5).map(|_| ())
        }
    }
}

fn store() -> Result<Store> {
    paths::ensure_dirs()?;
    Store::open(&paths::db_path())
}

fn add(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        bail!("uso: aic add \"texto da tarefa @data\"");
    }
    let store = store()?;
    let parsed = quickadd::parse(text, Local::now().date_naive());
    if parsed.title.trim().is_empty() {
        bail!("a tarefa precisa de um título além da data");
    }
    let mut task = Task::new(parsed.title);
    task.due = parsed.due;
    store.save_task(&task)?;
    let short = &task.id.to_string()[..8];
    match task.due {
        Some(d) => println!(
            "✓ tarefa criada [{short}]: {}  ({})",
            task.title,
            quickadd::due_label(d, Local::now().date_naive())
        ),
        None => println!("✓ tarefa criada [{short}]: {}", task.title),
    }
    Ok(())
}

fn tasks(all: bool, json: bool) -> Result<()> {
    let store = store()?;
    let mut list = store.tasks()?;
    list.retain(|t| all || !t.done);
    // com data primeiro, a mais próxima em cima
    list.sort_by_key(|t| (t.done, t.due.is_none(), t.due, t.created_at));
    if json {
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(());
    }
    if list.is_empty() {
        println!("nenhuma tarefa aberta. crie uma com: aic add \"...\"");
    }
    let today = Local::now().date_naive();
    for t in list {
        let due = t
            .due
            .map(|d| quickadd::due_label(d, today))
            .unwrap_or_default();
        println!(
            "{}  {} {:<12} {}",
            &t.id.to_string()[..8],
            if t.done { "✓" } else { " " },
            due,
            t.title,
        );
    }
    Ok(())
}

fn done(prefix: &str) -> Result<()> {
    if prefix.len() < 4 {
        bail!("passe pelo menos 4 caracteres do id (veja `aic tasks`)");
    }
    let store = store()?;
    let matches: Vec<_> = store
        .tasks()?
        .into_iter()
        .filter(|t| t.id.to_string().starts_with(prefix))
        .collect();
    match matches.as_slice() {
        [t] => {
            let mut t = t.clone();
            t.done = true;
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
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&input) else {
        return Ok(());
    };
    let session = std::env::var("AIC_SESSION")
        .ok()
        .and_then(|s| s.parse().ok());
    let e = HookEvent::from_hook_input(&v, session, aic_term::now_ms());
    let _ = events::append(&e);
    Ok(())
}

/// statusLine das sessões do AIC: grava o snapshot (modelo, contexto, custo, limites) que a barra
/// de cima e a aba Tokens leem. Não imprime nada: o terminal fica sem statusline.
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
        image::save_buffer(
            &path,
            &crate::icon_rgba(size),
            size,
            size,
            image::ExtendedColorType::Rgba8,
        )?;
        println!("{}", path.display());
    }
    Ok(())
}
