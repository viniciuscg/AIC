//! Renderiza as telas do AIC sem janela (wgpu/lavapipe) e salva PNGs em
//! `target/snapshots/`. Serve de teste de fumaça e de revisão visual.
//!
//!   cargo test -p aic-app --test snapshots

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use egui_kittest::Harness;
use aic_app::app::{App, View};
use aic_core::events::{self, HookEvent};
use aic_core::{Priority, SessionKind, Subtask, Task, TaskStatus};

/// Os testes mexem em variáveis de ambiente: um de cada vez.
static LOCK: Mutex<()> = Mutex::new(());

fn sandbox(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aic-snap-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for d in ["config", "data", "state"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    // SAFETY: protegido pelo LOCK (um teste por vez mexe no ambiente)
    unsafe {
        std::env::set_var("AIC_CONFIG_DIR", dir.join("config"));
        std::env::set_var("AIC_DATA_DIR", dir.join("data"));
        std::env::set_var("AIC_STATE_DIR", dir.join("state"));
        // transcripts do Claude Code: sandbox com dados sintéticos (histórico de uso determinístico)
        std::env::set_var("CLAUDE_CONFIG_DIR", dir.join("claude"));
    }
    std::fs::create_dir_all(dir.join("claude/projects")).unwrap();
    std::fs::write(dir.join("config/config.toml"), "setup_done = true\nanimations = true\n\n[modules]\ntmux = false\nlayne = false\n").unwrap();
    dir
}

fn out(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/snapshots");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{name}.png"))
}

fn tasks(app: &mut App) {
    let mut a = Task::new("Corrigir cálculo de juros no checkout");
    a.repo = Some("api-pagamentos".into());
    a.card_key = Some("DEV-142".into());
    a.priority = Priority::High;
    a.status = TaskStatus::Doing;
    a.due = Some(chrono::Local::now().date_naive());
    a.subtasks = vec![
        Subtask { text: "reproduzir com pedido 9912".into(), done: true },
        Subtask { text: "ajustar arredondamento".into(), done: false },
        Subtask { text: "teste de regressão".into(), done: false },
    ];
    a.notes = "Diferença de centavos no juros diário quando o pedido cruza o mês.".into();
    let mut b = Task::new("Tela de pedidos: filtro por data");
    b.repo = Some("web-app".into());
    b.card_key = Some("DEV-150".into());
    b.tags = vec!["front".into()];
    let mut c = Task::new("Revisar PR do notificador (retry de webhooks)");
    c.repo = Some("notificador".into());
    c.status = TaskStatus::Review;
    c.priority = Priority::Urgent;
    let mut d = Task::new("Atualizar docker-compose do infra");
    d.status = TaskStatus::Done;
    let mut e = Task::new("Ler a thread sobre o rate limit da API");
    e.priority = Priority::Low;
    for t in [a, b, c, d, e] {
        app.save_task(t);
    }
}

fn hook(session: aic_core::Id, name: &str, extra: &[(&str, &str)]) {
    let mut v = serde_json::json!({ "session_id": session.to_string(), "hook_event_name": name });
    for (k, val) in extra {
        v[*k] = serde_json::Value::String(val.to_string());
    }
    let e = HookEvent::from_hook_input(&v, Some(session), aic_term::now_ms());
    events::append(&e).unwrap();
}

fn harness(size: [f32; 2]) -> Harness<'static, Option<App>> {
    Harness::builder().with_size(egui::vec2(size[0], size[1])).build_ui_state(
        move |ui, app: &mut Option<App>| {
            let a = app.get_or_insert_with(|| App::new(ui.ctx(), true).expect("app"));
            aic_app::ui::draw(a, ui);
        },
        None,
    )
}

fn settle(h: &mut Harness<'static, Option<App>>, ms: u64) {
    for _ in 0..(ms / 50) {
        h.step();
        std::thread::sleep(Duration::from_millis(50));
    }
    h.run_steps(2);
}

fn shot(h: &mut Harness<'static, Option<App>>, name: &str) {
    let img = h.render().expect("render");
    img.save(out(name)).unwrap();
}

/// Transcripts sintéticos (só campos de uso) dos últimos 30 dias, pra aba Tokens.
/// `sessions`: (session id do Claude, cwd). Gerador fixo: mesmo resultado a cada execução.
fn usage_fixtures(sessions: &[(String, String)]) {
    let root = aic_core::paths::claude_dir().join("projects");
    let _ = std::fs::remove_dir_all(&root);
    let models = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-1", "claude-opus-5"];
    let today = chrono::Local::now().date_naive();
    let mut seed: u64 = 0x5eed;
    let mut rnd = |n: u64| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) % n.max(1)
    };
    for (si, (sid, cwd)) in sessions.iter().enumerate() {
        let dir = aic_core::claude::project_dir(std::path::Path::new(cwd));
        std::fs::create_dir_all(&dir).unwrap();
        let mut text = String::new();
        for back in 0..30i64 {
            // semana de trabalho: menos uso no fim de semana, sessões com ritmos diferentes
            let day = today - chrono::Duration::days(back);
            let weekend = matches!(chrono::Datelike::weekday(&day), chrono::Weekday::Sat | chrono::Weekday::Sun);
            let calls = if weekend { rnd(2) } else { 2 + rnd(6 - si as u64 % 3) };
            for c in 0..calls {
                let model = match (si, rnd(10)) {
                    (_, 0) => models[2],
                    (0, _) => models[0],
                    (1, 1..=6) => models[1],
                    (1, _) => models[0],
                    (2, 1) => models[3],
                    (2, _) => models[1],
                    _ if back > 18 => models[4],
                    _ => models[0],
                };
                let ts = day.and_hms_opt(9 + (c as u32 % 9), (rnd(60)) as u32, 0).unwrap().and_local_timezone(chrono::Local).single().unwrap();
                if ts > chrono::Local::now() {
                    continue;
                }
                let k = 1 + rnd(4);
                let usage = serde_json::json!({
                    "input_tokens": 20 + rnd(400), "output_tokens": 3_000 * k + rnd(9_000),
                    "cache_creation_input_tokens": 40_000 * k, "cache_read_input_tokens": 400_000 * k + rnd(300_000),
                    "cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 40_000 * k},
                    "service_tier": "standard", "speed": "standard"
                });
                let line = serde_json::json!({
                    "type": "assistant", "timestamp": ts.to_rfc3339(), "sessionId": sid, "cwd": cwd,
                    "requestId": format!("req_{si}_{back}_{c}"), "uuid": format!("u{si}{back}{c}"),
                    "message": {"id": format!("msg_{si}_{back}_{c}"), "model": model, "role": "assistant", "content": [], "usage": usage}
                });
                text += &format!("{line}\n");
            }
        }
        std::fs::write(dir.join(format!("{sid}.jsonl")), text).unwrap();
    }
}

/// Cria sessões de shell que imprimem algo parecido com o Claude e simula os estados via hooks.
fn demo_sessions(h: &mut Harness<'static, Option<App>>) {
    h.step();
    let ctx = h.ctx.clone();
    let app = h.state_mut().as_mut().unwrap();
    tasks(app);
    let script = r#"printf '\e[38;2;215;119;87m✻\e[0m Welcome to \e[1mClaude Code\e[0m!\n\n> corrige o cálculo do juros quando cruza o mês\n\n\e[38;2;215;119;87m⏺\e[0m Vou olhar o serviço de juros primeiro.\n\n\e[38;2;215;119;87m⏺\e[0m \e[1mRead\e[0m(src/checkout/interest.ts)\n  ⎿  Read 214 lines\n\n\e[38;2;215;119;87m⏺\e[0m \e[1mUpdate\e[0m(src/checkout/interest.ts)\n  ⎿  Updated with \e[32m6 additions\e[0m and \e[31m2 removals\e[0m\n\n╭──────────────────────────────────────────────╮\n│ > \e[2mTry \"write a test for interest.ts\"\e[0m          │\n╰──────────────────────────────────────────────╯\n  \e[2m⏵⏵ accept edits on (shift+tab to cycle)\e[0m\n'; sleep 60"#;
    // 3 blocos: um card com 2 terminais, uma revisão, infra
    let plan = [
        ("api juros", "api-pagamentos", "DEV-142 · juros no checkout", true),
        ("web filtros", "web-app", "", false),
        ("review", "notificador", "Revisão do notificador", true),
        ("infra", "infra", "infra", true),
    ];
    let mut ids = vec![];
    let mut block = None;
    for (name, repo, block_name, new_block) in plan {
        let cwd = app.cfg.repo_path(repo).unwrap_or_else(std::env::temp_dir);
        if new_block {
            block = app.new_block(block_name, None);
        }
        let id = app.create_session_in(&ctx, block, name.into(), SessionKind::Command(script.into()), cwd, None, None, vec![], None);
        ids.push(id);
    }
    let b0 = app.block_of(ids[0]).unwrap();
    app.update_block(b0, |b| b.notes = "## Plano\n- reproduzir com a pedido 9912\n- arredondar no fim, não por parcela\n- teste de regressão no checkout".into());
    let t0 = app.tasks.iter().find(|t| t.card_key.as_deref() == Some("DEV-142")).map(|t| t.id).unwrap();
    app.update_task(t0, |t| t.session = Some(ids[0]));
    app.update_block(b0, |b| b.task = Some(t0));
    // estados via hooks
    hook(ids[0], "SessionStart", &[]);
    hook(ids[0], "UserPromptSubmit", &[("prompt", "corrige o juros"), ("permission_mode", "acceptEdits")]);
    hook(ids[0], "SubagentStart", &[("agent_id", "a1")]);
    hook(ids[0], "SubagentStart", &[("agent_id", "a2")]);
    hook(ids[1], "SessionStart", &[]);
    hook(ids[1], "UserPromptSubmit", &[]);
    hook(ids[1], "Notification", &[("notification_type", "permission_prompt"), ("message", "Claude precisa de permissão para rodar npm test")]);
    hook(ids[2], "SessionStart", &[]);
    hook(ids[2], "UserPromptSubmit", &[]);
    hook(ids[2], "Stop", &[]);
    app.focus(ids[0]);
    // histórico de uso: duas sessões abertas no AIC + duas de fora (só projeto)
    let mut usage_sessions: Vec<(String, String)> = ids[..2].iter().zip(["api-pagamentos", "web-app"]).map(|(id, repo)| (app.sessions[id].s.claude_session.to_string(), format!("/home/dev/projetos/{repo}"))).collect();
    usage_sessions.push(("7c1e9a52-0000-4000-8000-000000000001".into(), "/home/dev/projetos/notificador".into()));
    usage_sessions.push(("9d2f0b63-0000-4000-8000-000000000002".into(), "/home/dev/dotfiles".into()));
    usage_fixtures(&usage_sessions);
    app.usage_history.ensure_started(&ctx);
    // snapshot de statusline falso pra sessão em foco
    let snap = serde_json::json!({
        "session_id": app.sessions[&ids[0]].s.claude_session.to_string(),
        "model": {"display_name": "Opus 5.5"}, "effort": {"level": "medium"},
        "context_window": {"used_percentage": 17, "context_window_size": 1000000},
        "cost": {"total_cost_usd": 3.83, "total_duration_ms": 63_900_000u64, "total_lines_added": 6, "total_lines_removed": 2},
        "rate_limits": {"five_hour": {"used_percentage": 15, "resets_at": chrono::Utc::now().timestamp() + 11580},
                        "seven_day": {"used_percentage": 2, "resets_at": chrono::Utc::now().timestamp() + 561600}}
    });
    let s = aic_core::claude::StatusSnapshot::from_json(&snap);
    std::fs::write(aic_core::claude::StatusSnapshot::path_for(&s.session_id), serde_json::to_vec(&s).unwrap()).unwrap();
}

#[test]
fn telas() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("telas");
    let mut h = harness([1480.0, 900.0]);
    demo_sessions(&mut h);
    settle(&mut h, 2500);
    shot(&mut h, "blocos");
    for (v, name) in [(aic_core::SessionView::Tabs, "abas"), (aic_core::SessionView::Focus, "foco"), (aic_core::SessionView::Grid, "grade")] {
        h.state_mut().as_mut().unwrap().cfg.view = v;
        settle(&mut h, 300);
        shot(&mut h, name);
    }
    h.state_mut().as_mut().unwrap().cfg.view = aic_core::SessionView::Blocks;
    // prefixo armado: indicador na barra
    h.state_mut().as_mut().unwrap().prefix_pending = true;
    settle(&mut h, 200);
    shot(&mut h, "prefixo");
    h.state_mut().as_mut().unwrap().prefix_pending = false;
    // dois workspaces, um com alguém esperando (fica roxo na barra)
    {
        let app = h.state_mut().as_mut().unwrap();
        let ws0 = app.ws;
        app.new_workspace("front");
        app.switch_ws(ws0);
    }
    settle(&mut h, 200);
    shot(&mut h, "workspaces");
    for (view, name) in [(View::Worktrees, "worktrees"), (View::Usage, "tokens"), (View::Settings, "config"), (View::Help, "ajuda")] {
        let app = h.state_mut().as_mut().unwrap();
        app.set_view(view);
        settle(&mut h, 300);
        shot(&mut h, name);
    }
    // Changes: repo de mentira com um arquivo staged, um modificado e um novo
    {
        let repo = std::env::temp_dir().join(format!("aic-snap-repo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&repo);
        std::fs::create_dir_all(repo.join("src")).unwrap();
        let git = |args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(&repo).args(args).output().unwrap().status.success(), "git {args:?}");
        git(&["init", "-q"]);
        std::fs::write(repo.join("src/juros.rs"), "pub fn juros(v: f64) -> f64 {\n    v * 0.01\n}\n").unwrap();
        std::fs::write(repo.join("README.md"), "# api\n").unwrap();
        git(&["add", "."]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "init"]);
        std::fs::write(repo.join("src/juros.rs"), "pub fn juros(v: f64, dias: u32) -> f64 {\n    (v * 0.01 * dias as f64 * 100.0).round() / 100.0\n}\n").unwrap();
        std::fs::write(repo.join("README.md"), "# api\n\nPagamentos.\n").unwrap();
        git(&["add", "README.md"]);
        std::fs::write(repo.join("src/novo.rs"), "// arquivo novo\n").unwrap();
        let app = h.state_mut().as_mut().unwrap();
        app.changes.pinned = Some(repo);
        app.cfg.layne.auto_start = false;
        app.cfg.modules.insert("layne".into(), true);
        app.set_view(View::Changes);
        settle(&mut h, 300);
        let app = h.state_mut().as_mut().unwrap();
        app.changes.message = "fix: juros por dia".into();
        settle(&mut h, 200);
        shot(&mut h, "changes");
        h.state_mut().as_mut().unwrap().cfg.modules.insert("layne".into(), false);
    }
    let app = h.state_mut().as_mut().unwrap();
    app.set_view(View::Settings);
    app.settings.search = "prefixo".into();
    settle(&mut h, 200);
    shot(&mut h, "config-busca");
    let app = h.state_mut().as_mut().unwrap();
    app.settings.search.clear();
    app.set_view(View::Sessions);
    app.open_quick_add(None);
    if let Some(aic_app::ui::modals::Modal::QuickAdd { text, .. }) = &mut app.modal {
        *text = "corrigir cálculo de juros #api !alta @sexta ^dev-142".into();
    }
    settle(&mut h, 200);
    shot(&mut h, "captura-rapida");
    let app = h.state_mut().as_mut().unwrap();
    app.modal = Some(aic_app::ui::modals::Modal::ConfirmQuit);
    settle(&mut h, 200);
    shot(&mut h, "fechar");
    let app = h.state_mut().as_mut().unwrap();
    let ids: Vec<_> = app.sessions.keys().copied().collect();
    for id in ids {
        app.kill_session(id, false);
    }
}

/// Aba Tokens numa janela alta: a página inteira do histórico de uso.
#[test]
fn historico_de_uso() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("historico");
    let mut h = harness([1480.0, 1500.0]);
    demo_sessions(&mut h);
    settle(&mut h, 600);
    h.state_mut().as_mut().unwrap().set_view(View::Usage);
    settle(&mut h, 600);
    shot(&mut h, "historico");
    let app = h.state_mut().as_mut().unwrap();
    let report = app.usage_history.report.clone().expect("relatório do histórico");
    assert_eq!(report.by_session(30).len(), 4);
    assert!(report.totals(30).cost > 0.0);
    let ids: Vec<_> = app.sessions.keys().copied().collect();
    for id in ids {
        app.kill_session(id, false);
    }
}

#[test]
fn splash() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("splash");
    std::fs::write(std::env::var("AIC_CONFIG_DIR").unwrap() + "/config.toml", "setup_done = false\n").unwrap();
    let mut h: Harness<'static, Option<App>> = Harness::builder().with_size(egui::vec2(1400.0, 860.0)).build_ui_state(
        |ui, app: &mut Option<App>| {
            let a = app.get_or_insert_with(|| App::new(ui.ctx(), true).expect("app"));
            aic_app::ui::draw(a, ui);
        },
        None,
    );
    settle(&mut h, 1200);
    if let Some(app) = h.state_mut().as_mut() {
        let _ = &mut app.phase;
    }
    settle(&mut h, 300);
    shot(&mut h, "assistente");
}

/// Folha do mascote em cada estado e nos tamanhos reais da UI
/// (22 cabeçalho, 24 topbar, 34 card, 64, 150 splash), mais o fundo e o wordmark.
/// Gera `mascotes.png` (instante t=1.3) e `mascotes-b.png` (t=2.05, outro quadro).
#[test]
fn mascotes() {
    use egui::{Align2, FontId, Pos2, Rect, Vec2};
    use aic_app::brand;
    use aic_core::SessionState;

    const STATES: [SessionState; 6] =
        [SessionState::Boot, SessionState::Working, SessionState::Idle, SessionState::Waiting, SessionState::Done, SessionState::GameOver];
    const SIZES: [f32; 5] = [22.0, 24.0, 34.0, 64.0, 150.0];
    let col_w = 170.0;
    let label_w = 120.0;
    let height: f32 = 70.0 + SIZES.iter().map(|s| s + 16.0).sum::<f32>();
    let width = label_w + col_w * STATES.len() as f32;
    for (name, t) in [("mascotes", 1.3f64), ("mascotes-b", 2.05)] {
        let mut h = Harness::builder().with_size(egui::vec2(width, height)).build_ui(move |ui| {
            let th = brand::theme();
            let block = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
            ui.painter().rect_filled(block, 0.0, th.bg);
            brand::background(ui, block, t, &th);
            let wm = brand::wordmark(ui, Pos2::new(12.0, 12.0), 18.0, &th);
            ui.painter().text(Pos2::new(wm.right() + 16.0, 16.0), Align2::LEFT_TOP, brand::TAGLINE, FontId::proportional(13.0), th.dim);
            let mut y = 60.0;
            for s in SIZES {
                ui.painter().text(Pos2::new(12.0, y + s / 2.0), Align2::LEFT_CENTER, format!("{s}px"), FontId::monospace(11.0), th.dim);
                for (si, st) in STATES.iter().enumerate() {
                    let x = label_w + si as f32 * col_w;
                    if s == SIZES[0] {
                        ui.painter().text(Pos2::new(x + col_w / 2.0, 44.0), Align2::CENTER_CENTER, brand::state_label(*st), FontId::monospace(11.0), th.state_color(*st));
                    }
                    let r = Rect::from_min_size(Pos2::new(x + (col_w - s) / 2.0, y), Vec2::splat(s));
                    ui.painter().rect_filled(r, 0.0, th.bg2);
                    brand::mascot(ui, r, *st, t, &th);
                }
                y += s + 16.0;
            }
        });
        h.run_steps(2);
        let img = h.render().expect("render");
        img.save(out(name)).unwrap();
    }
}
