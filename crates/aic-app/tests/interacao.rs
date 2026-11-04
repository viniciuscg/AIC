//! Cliques e teclas de verdade (egui_kittest) em cada botão das telas: o clique chega
//! no botão certo e muda o estado do app? Pega fundo clicável roubando clique, atalho
//! engolido, botão sem efeito.
//!
//!   cargo test -p aic-app --test interacao

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use aic_app::app::{App, View};
use aic_app::ui::modals::Modal;
use aic_core::{Id, SessionKind};
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// Os testes mexem em variáveis de ambiente: um de cada vez.
static LOCK: Mutex<()> = Mutex::new(());

fn sandbox(name: &str, extra_config: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aic-click-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for d in ["config", "data", "state", "claude/projects", "repos"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    // SAFETY: protegido pelo LOCK (um teste por vez mexe no ambiente)
    unsafe {
        std::env::set_var("AIC_CONFIG_DIR", dir.join("config"));
        std::env::set_var("AIC_DATA_DIR", dir.join("data"));
        std::env::set_var("AIC_STATE_DIR", dir.join("state"));
        std::env::set_var("CLAUDE_CONFIG_DIR", dir.join("claude"));
    }
    let cfg = format!(
        "setup_done = true\nrepos_dirs = [\"{}\"]\n{extra_config}\n[modules]\ntmux = false\nanakin = false\nsound = false\nnotifications = false\n",
        dir.join("repos").display()
    );
    std::fs::write(dir.join("config/config.toml"), cfg).unwrap();
    dir
}

type H = Harness<'static, Option<App>>;

fn harness(size: [f32; 2]) -> H {
    let mut h = Harness::builder()
        .with_size(egui::vec2(size[0], size[1]))
        .build_ui_state(
            |ui, app: &mut Option<App>| {
                let a = app.get_or_insert_with(|| App::new(ui.ctx(), true).expect("app"));
                aic_app::ui::draw(a, ui);
            },
            None,
        );
    h.run_steps(2);
    h
}

fn app(h: &mut H) -> &mut App {
    h.state_mut().as_mut().unwrap()
}

/// Alguns frames (o clique enfileirado vira evento, o efeito aparece no frame seguinte).
fn frames(h: &mut H, n: usize) {
    for _ in 0..n {
        h.step();
    }
}

fn click(h: &mut H, label: &str) {
    h.get_by_label(label).click();
    frames(h, 3);
}

/// Primeiro de vários botões com o mesmo rótulo.
fn click_first(h: &mut H, label: &str) {
    h.get_all_by_label(label)
        .next()
        .unwrap_or_else(|| panic!("sem botão \"{label}\""))
        .click();
    frames(h, 3);
}

fn exists(h: &H, label: &str) -> bool {
    h.query_all_by_label(label).next().is_some()
}

fn key(h: &mut H, m: Modifiers, k: Key) {
    h.key_press_modifiers(m, k);
    frames(h, 2);
}

/// Ctrl como o winit manda no Linux (`command` espelha o `ctrl`).
fn ctrl() -> Modifiers {
    Modifiers::CTRL | Modifiers::COMMAND
}

fn ctrl_shift() -> Modifiers {
    ctrl() | Modifiers::SHIFT
}

/// Bloco com um terminal rodando `cmd` (sem Claude: não depende do binário).
fn terminal(h: &mut H, name: &str, cmd: &str) -> Id {
    h.step();
    let ctx = h.ctx.clone();
    let a = app(h);
    let block = a.new_block(name);
    let id = a.create_session_in(
        &ctx,
        block,
        name.into(),
        SessionKind::Command(cmd.into()),
        std::env::temp_dir(),
        None,
        vec![],
        None,
    );
    frames(h, 3);
    id
}

fn wait_until(h: &mut H, what: &str, mut f: impl FnMut(&mut App) -> bool) {
    for _ in 0..100 {
        if f(app(h)) {
            return;
        }
        h.step();
        std::thread::sleep(Duration::from_millis(30));
    }
    panic!("esperando: {what}");
}

fn kill_all(h: &mut H) {
    let a = app(h);
    let ids: Vec<_> = a.sessions.keys().copied().collect();
    for id in ids {
        a.kill_session(id, false);
    }
}

fn is_modal(h: &mut H, f: impl Fn(&Modal) -> bool) -> bool {
    app(h).modal.as_ref().is_some_and(f)
}

// ───────────────────────── terminal ─────────────────────────

/// Mais um terminal na sessão em foco.
fn split(h: &mut H, name: &str) -> Id {
    let ctx = h.ctx.clone();
    let a = app(h);
    let block = a.current_block();
    let id = a.create_session_in(
        &ctx,
        block,
        name.into(),
        SessionKind::Command("sleep 60".into()),
        std::env::temp_dir(),
        None,
        vec![],
        None,
    );
    frames(h, 3);
    id
}

/// O bug da v1: fechar um terminal jogava o foco no último terminal do workspace, e a sessão
/// parecia ter sido fechada inteira.
#[test]
fn fechar_terminal_nao_fecha_a_sessao() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("fechar", "");
    let mut h = harness([1400.0, 900.0]);
    let a1 = terminal(&mut h, "hermes", "sleep 60");
    let a2 = split(&mut h, "hermes-shell");
    let block_a = app(&mut h).block_of(a1).unwrap();
    let b1 = terminal(&mut h, "wallet", "sleep 60");
    let block_b = app(&mut h).block_of(b1).unwrap();

    // volta pra hermes e fecha o terminal em foco pelo Ctrl+B x
    app(&mut h).focus(a2);
    frames(&mut h, 2);
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::X);
    assert!(
        is_modal(
            &mut h,
            |m| matches!(m, Modal::CloseSession { id, .. } if *id == a2)
        ),
        "pede confirmação do terminal em foco"
    );
    click(&mut h, "Cancelar");
    assert!(app(&mut h).sessions.contains_key(&a2), "cancelar mantém");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::X);
    click(&mut h, "Fechar  ⏎");

    assert!(!app(&mut h).sessions.contains_key(&a2), "o terminal fechou");
    assert!(app(&mut h).sessions.contains_key(&a1), "o vizinho continua");
    assert!(
        app(&mut h).sessions.contains_key(&b1),
        "a outra sessão continua"
    );
    assert_eq!(
        app(&mut h).current_block(),
        Some(block_a),
        "continua na mesma sessão"
    );
    assert_eq!(app(&mut h).focused(), Some(a1), "foco no terminal vizinho");
    assert_eq!(app(&mut h).blocks[&block_a].panes, vec![a1]);

    // o último terminal: agora sim a sessão some da barra (como workspace vazio no i3)
    app(&mut h).kill_session(a1, false);
    frames(&mut h, 2);
    assert!(!app(&mut h).blocks.contains_key(&block_a));
    assert_eq!(
        app(&mut h).current_block(),
        Some(block_b),
        "vai pra vizinha"
    );
    assert_eq!(app(&mut h).focused(), Some(b1));
    kill_all(&mut h);
}

/// Fechar um terminal que NÃO está em foco não mexe no foco.
#[test]
fn fechar_terminal_sem_foco_mantem_o_foco() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("fechar2", "");
    let mut h = harness([1400.0, 900.0]);
    let a1 = terminal(&mut h, "a", "sleep 60");
    let a2 = split(&mut h, "a2");
    assert_eq!(app(&mut h).focused(), Some(a2));
    app(&mut h).kill_session(a1, false);
    frames(&mut h, 2);
    assert_eq!(app(&mut h).focused(), Some(a2));
    kill_all(&mut h);
}

#[test]
fn shell_que_sai_fecha_e_claude_morto_oferece_reabrir() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = sandbox("terminal", "");
    let mut h = harness([1400.0, 900.0]);

    // `exit` num shell: o terminal fecha sozinho, e o vizinho fica
    let keep = terminal(&mut h, "fica", "sleep 60");
    let block = app(&mut h).block_of(keep).unwrap();
    let ctx = h.ctx.clone();
    let gone = app(&mut h).create_session_in(
        &ctx,
        Some(block),
        "sai".into(),
        SessionKind::Command("exit 0".into()),
        std::env::temp_dir(),
        None,
        vec![],
        None,
    );
    wait_until(&mut h, "terminal fechar sozinho", |a| {
        !a.sessions.contains_key(&gone)
    });
    assert_eq!(app(&mut h).focused(), Some(keep));

    // saiu com erro: fica na tela, com a opção de reabrir
    let marker = dir.join("marker");
    let cmd = format!(
        "test -f {m} || {{ touch {m}; exit 3; }}; sleep 60",
        m = marker.display()
    );
    let id = terminal(&mut h, "reinicia", &cmd);
    wait_until(&mut h, "processo sair", |a| {
        a.sessions[&id].term.as_ref().is_some_and(|t| t.is_exited())
    });
    frames(&mut h, 2);
    let reabrir = format!("{} Reabrir (Enter)", aic_app::ui::widgets::I_RESTART);
    assert!(exists(&h, &reabrir), "tela de processo encerrado");
    click(&mut h, &reabrir);
    wait_until(&mut h, "terminal reiniciado", |a| {
        a.sessions[&id]
            .term
            .as_ref()
            .is_some_and(|t| !t.is_exited())
    });
    kill_all(&mut h);
}

// ───────────────────────── a barra ─────────────────────────

/// Sem cromo: o que estava marcado de vermelho sumiu da tela.
#[test]
fn tela_so_tem_terminal_e_barra() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("cromo", "");
    let mut h = harness([1500.0, 900.0]);
    terminal(&mut h, "hermes", "sleep 60");
    for gone in [
        "Aba Worktrees",
        "Aba Changes",
        "Aba Config",
        "Fechar terminal",
        "Maximizar terminal",
        "Reiniciar terminal",
        "Fechar bloco",
        "Painel do bloco",
        "TAREFAS",
    ] {
        assert!(!exists(&h, gone), "{gone} não devia estar na tela");
    }
    assert!(exists(&h, "Sessão 1 hermes"));
    for tab in ["Aba Tarefas", "Aba Sessões", "Aba Tokens"] {
        assert!(exists(&h, tab), "{tab}");
    }
    // a linha das sessões só existe na aba Sessões
    app(&mut h).set_view(View::Tasks);
    frames(&mut h, 2);
    assert!(!exists(&h, "Sessão 1 hermes"));
    app(&mut h).set_view(View::Sessions);
    assert!(
        !exists(&h, "Workspace 1 Principal"),
        "um workspace só não aparece"
    );
    kill_all(&mut h);
}

#[test]
fn ctrl_tab_anda_so_entre_as_abas() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("ctrltab", "");
    let mut h = harness([1500.0, 900.0]);
    let a = terminal(&mut h, "a", "sleep 60");
    let b = terminal(&mut h, "b", "sleep 60");
    let (ba, bb) = (app(&mut h).block_of(a), app(&mut h).block_of(b));
    assert_eq!(app(&mut h).current_block(), bb);

    // Sessões → Tokens → Tarefas → Sessões, sem trocar de sessão no caminho
    key(&mut h, ctrl(), Key::Tab);
    assert_eq!(app(&mut h).view, View::Usage);
    key(&mut h, ctrl(), Key::Tab);
    assert_eq!(app(&mut h).view, View::Tasks);
    key(&mut h, ctrl(), Key::Tab);
    assert_eq!(app(&mut h).view, View::Sessions);
    assert_eq!(app(&mut h).current_block(), bb, "continua na mesma sessão");
    assert!(app(&mut h).term_focus, "o teclado vai pro terminal");
    key(&mut h, ctrl_shift(), Key::Tab);
    assert_eq!(app(&mut h).view, View::Tasks, "Ctrl+Shift+Tab volta");

    // Alt+Tab faz o mesmo
    key(&mut h, Modifiers::ALT, Key::Tab);
    assert_eq!(app(&mut h).view, View::Sessions, "Alt+Tab");
    key(&mut h, Modifiers::ALT | Modifiers::SHIFT, Key::Tab);
    assert_eq!(app(&mut h).view, View::Tasks, "Alt+Shift+Tab");
    app(&mut h).set_view(View::Sessions);
    frames(&mut h, 2);

    // trocar de sessão tem tecla própria
    let ctx = h.ctx.clone();
    app(&mut h).run_action(&ctx, aic_app::actions::Action::NextBlock);
    assert_eq!(app(&mut h).current_block(), ba, "próxima sessão dá a volta");
    app(&mut h).run_action(&ctx, aic_app::actions::Action::PrevBlock);
    assert_eq!(app(&mut h).current_block(), bb);

    // de uma tela fora da barra, vai pras Sessões
    app(&mut h).set_view(View::Settings);
    frames(&mut h, 2);
    assert!(exists(&h, "Aba Config"), "o nome da tela aparece embaixo");
    key(&mut h, ctrl(), Key::Tab);
    assert_eq!(app(&mut h).view, View::Sessions);
    kill_all(&mut h);
}

/// Reabrir com lixo salvo: workspace vazio, sessão vazia e um bloco com terminais dos outros.
#[test]
fn reabrir_limpa_o_que_ficou_vazio() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("reabrir", "");
    let mut h = harness([1500.0, 900.0]);
    let a = terminal(&mut h, "a", "sleep 60");
    let block = app(&mut h).block_of(a).unwrap();
    {
        let x = app(&mut h);
        let ws0 = x.ws;
        x.new_workspace("Principal"); // o segundo "Principal"
        x.switch_ws(ws0);
        let geral = x.new_block("Geral").unwrap();
        x.update_block(geral, |b| b.panes.push(a));
        x.new_block("Novo bloco");
        let saved = x.sessions[&a].s.clone();
        let _ = x.store.save_session(&saved);
        x.flush_ws();
    }
    let ctx = h.ctx.clone();
    {
        let x = app(&mut h);
        if let Some(t) = x.sessions.get(&a).and_then(|r| r.term.as_ref()) {
            t.kill();
        }
        x.sessions.clear();
        x.blocks.clear();
        x.restore(&ctx);
    }
    frames(&mut h, 2);
    let x = app(&mut h);
    assert_eq!(x.workspaces.len(), 1, "o workspace vazio some");
    assert_eq!(x.ws_blocks(), vec![block], "as sessões vazias somem");
    assert_eq!(x.blocks[&block].panes, vec![a]);
    assert_eq!(x.store.workspaces().unwrap().len(), 1, "e somem do banco");
    kill_all(&mut h);
}

/// O boneco: aparece, abre a conversa no clique e fala quando uma sessão acaba.
#[test]
fn anakin_no_canto() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("anakin", "[anakin]\nauto_start = false\n");
    let mut h = harness([1500.0, 900.0]);
    assert!(!exists(&h, "Anakin"), "módulo desligado: sem boneco");
    app(&mut h).cfg.modules.remove("anakin");
    frames(&mut h, 2);
    assert!(exists(&h, "Anakin"));
    assert!(!app(&mut h).anakin.open);
    click(&mut h, "Anakin");
    assert!(app(&mut h).anakin.open, "clicar abre a conversa");
    assert!(exists(&h, "Fechar o Anakin"));
    click(&mut h, "Fechar o Anakin");
    assert!(!app(&mut h).anakin.open);
    app(&mut h).anakin.avatar.speak("hermes acabou. Yippee!", 5);
    frames(&mut h, 2);
    assert_eq!(
        app(&mut h).anakin.avatar.speech(),
        Some("hermes acabou. Yippee!")
    );

    // botão direito abre o menu; uma tecla escolhe a animação e fecha
    h.get_by_label("Anakin").click_secondary();
    frames(&mut h, 2);
    assert!(
        app(&mut h).anakin.avatar.picker,
        "botão direito abre o menu"
    );
    assert!(exists(&h, "1   Luta: o do cavanhaque"));
    h.event(egui::Event::Text("1".into()));
    frames(&mut h, 2);
    assert!(!app(&mut h).anakin.avatar.picker);
    assert_eq!(
        app(&mut h).anakin.avatar.scene(),
        Some(aic_app::ui::avatar::Scene::Fight(
            aic_app::ui::avatar_rivals::Rival::Curly
        ))
    );
    // pelo atalho também abre, e Esc fecha sem rodar nada
    let ctx = h.ctx.clone();
    app(&mut h).run_action(&ctx, aic_app::actions::Action::AvatarScenes);
    frames(&mut h, 1);
    assert!(app(&mut h).anakin.avatar.picker);
    h.key_press(egui::Key::Escape);
    frames(&mut h, 1);
    assert!(!app(&mut h).anakin.avatar.picker);

    // o boneco some e volta pela tecla
    let ctx = h.ctx.clone();
    app(&mut h).run_action(&ctx, aic_app::actions::Action::ToggleAvatar);
    frames(&mut h, 2);
    assert!(!exists(&h, "Anakin"));
    kill_all(&mut h);
}

#[test]
fn sessao_nova_e_arranjo_pelo_teclado() {
    use aic_app::actions::Action;
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("bloco", "");
    let mut h = harness([1500.0, 900.0]);
    let id = terminal(&mut h, "a", "sleep 60");
    let block = app(&mut h).block_of(id).unwrap();

    let split_before = app(&mut h).blocks[&block].split;
    let ctx = h.ctx.clone();
    app(&mut h).run_action(&ctx, Action::CycleSplit);
    assert_ne!(app(&mut h).blocks[&block].split, split_before);

    // Nova sessão pelo + da barra abre o diálogo, marcando que é sessão nova
    click(&mut h, "Nova sessão");
    assert!(is_modal(
        &mut h,
        |m| matches!(m, Modal::NewSession(f) if f.new_block)
    ));
    click(&mut h, "Cancelar");
    assert!(app(&mut h).modal.is_none());

    let n = app(&mut h).blocks[&block].panes.len();
    key(&mut h, ctrl_shift(), Key::T);
    assert_eq!(
        app(&mut h).blocks[&block].panes.len(),
        n + 1,
        "split na sessão"
    );

    key(&mut h, ctrl_shift(), Key::X);
    assert!(is_modal(&mut h, |m| matches!(m, Modal::CloseBlock { .. })));
    click(&mut h, "Fechar  ⏎");
    assert!(
        !app(&mut h).blocks.contains_key(&block),
        "fecha a sessão inteira"
    );
    kill_all(&mut h);
}

// ───────────────────────── tarefas ─────────────────────────

#[test]
fn tarefas_nao_tem_vinculo_com_sessao() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("tarefas", "");
    let mut h = harness([1500.0, 900.0]);
    terminal(&mut h, "api", "sleep 60");

    // Ctrl+B t abre a aba com o campo em foco, de qualquer lugar
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::T);
    assert_eq!(app(&mut h).view, View::Tasks);
    frames(&mut h, 2);
    h.key_press(Key::A); // nada de atalho: o campo pegou o teclado
    frames(&mut h, 1);

    let today = chrono::Local::now().date_naive();
    app(&mut h).tasks_ui.input = "pagar boleto".into();
    app(&mut h).tasks_ui.date = "amanhã".into();
    app(&mut h).tasks_ui.focus_input = true;
    frames(&mut h, 2);
    h.key_press(Key::Enter);
    frames(&mut h, 3);
    let t = app(&mut h)
        .tasks
        .iter()
        .find(|t| t.title == "pagar boleto")
        .cloned()
        .expect("Enter cria");
    assert_eq!(t.due, Some(today + chrono::Duration::days(1)));
    assert_eq!(app(&mut h).tasks_ui.input, "", "o campo limpa");
    assert!(exists(&h, "pagar boleto"), "aparece na lista");

    // @data no texto vence o campo de data
    app(&mut h).tasks_ui.input = "ligar pro banco @hoje".into();
    app(&mut h).tasks_ui.focus_input = true;
    frames(&mut h, 2);
    h.key_press(Key::Enter);
    frames(&mut h, 3);
    let t2 = app(&mut h)
        .tasks
        .iter()
        .find(|t| t.title == "ligar pro banco")
        .cloned()
        .unwrap();
    assert_eq!(t2.due, Some(today));

    // no teclado: fora do campo, x conclui, u desfaz o apagar, Esc volta pros terminais
    h.ctx
        .memory_mut(|m| m.surrender_focus(egui::Id::new("tarefa-titulo")));
    app(&mut h).tasks_ui.selected = Some(t.id);
    frames(&mut h, 2);
    h.key_press(Key::Period);
    frames(&mut h, 1);
    key(&mut h, Modifiers::NONE, Key::D);
    assert!(!app(&mut h).tasks.iter().any(|x| x.id == t.id), "d apaga");
    key(&mut h, Modifiers::NONE, Key::U);
    assert!(app(&mut h).tasks.iter().any(|x| x.id == t.id), "u desfaz");
    app(&mut h).tasks_ui.selected = Some(t.id);
    frames(&mut h, 2);
    key(&mut h, Modifiers::NONE, Key::X);
    assert!(
        app(&mut h)
            .tasks
            .iter()
            .find(|x| x.id == t.id)
            .unwrap()
            .done,
        "x conclui"
    );
    key(&mut h, Modifiers::NONE, Key::Escape);
    assert_eq!(app(&mut h).view, View::Sessions, "Esc volta");

    kill_all(&mut h);
}

// ───────────────────────── notas por sessão ─────────────────────────

#[test]
fn notas_por_sessao() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("notas", "");
    let mut h = harness([1500.0, 900.0]);
    let a = terminal(&mut h, "api", "sleep 60");
    let b = terminal(&mut h, "front", "sleep 60");
    assert_eq!(app(&mut h).focused(), Some(b));

    // Ctrl+B e abre o painel já escrevendo (o terminal solta o teclado)
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::E);
    assert!(app(&mut h).notes_open, "Ctrl+B e abre as notas");
    frames(&mut h, 2);
    assert!(!app(&mut h).term_focus, "o editor fica com o teclado");
    h.event(egui::Event::Text("lembrar do CORS".into()));
    frames(&mut h, 2);
    assert_eq!(
        app(&mut h).sessions[&b].s.notes,
        "lembrar do CORS",
        "digitar grava na sessão"
    );
    let saved = app(&mut h)
        .store
        .sessions()
        .unwrap()
        .into_iter()
        .find(|s| s.id == b)
        .unwrap();
    assert_eq!(saved.notes, "lembrar do CORS", "salvo no banco sem pedir");

    // Esc devolve o teclado pro terminal
    key(&mut h, Modifiers::NONE, Key::Escape);
    assert!(app(&mut h).term_focus, "Esc volta pro terminal");

    // Ctrl+B e de novo fecha
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::E);
    assert!(!app(&mut h).notes_open, "Ctrl+B e fecha");
    assert!(app(&mut h).term_focus);

    // outra sessão, outra nota
    app(&mut h).focus(a);
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::E);
    frames(&mut h, 2);
    h.event(egui::Event::Text("outra".into()));
    frames(&mut h, 2);
    assert_eq!(app(&mut h).sessions[&a].s.notes, "outra");
    assert_eq!(
        app(&mut h).sessions[&b].s.notes,
        "lembrar do CORS",
        "a nota da outra sessão não muda"
    );

    kill_all(&mut h);
}

// ───────────────────────── modais, toasts, topbar ─────────────────────────

#[test]
fn modais_toasts_e_abas() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("modais", "");
    let mut h = harness([1500.0, 900.0]);
    let _id = terminal(&mut h, "x", "sleep 60");

    // paradas da barra
    for (label, view) in [("Aba Tokens", View::Usage), ("Aba Tarefas", View::Tasks)] {
        click(&mut h, label);
        assert_eq!(app(&mut h).view, view, "aba {label}");
    }
    click(&mut h, "Aba Sessões");
    assert_eq!(app(&mut h).view, View::Sessions);
    key(&mut h, Modifiers::NONE, Key::F1);
    assert_eq!(app(&mut h).view, View::Help, "F1 abre a Ajuda");
    click(&mut h, "Aba Sessões");

    // toasts: clique fecha
    app(&mut h).toasts.clear();
    app(&mut h).toast("primeiro aviso", aic_app::app::ToastKind::Info);
    app(&mut h).toast("segundo aviso", aic_app::app::ToastKind::Ok);
    frames(&mut h, 2);
    click_first(&mut h, "Fechar aviso");
    assert_eq!(app(&mut h).toasts.len(), 1, "✕ do aviso");
    click(&mut h, "segundo aviso");
    assert!(app(&mut h).toasts.is_empty(), "clique no texto do aviso");

    // workspaces: criar pelo diálogo e fechar
    let n = app(&mut h).workspaces.len();
    key(&mut h, ctrl_shift(), Key::O);
    assert!(
        is_modal(&mut h, |m| matches!(m, Modal::Workspaces { .. })),
        "Ctrl+Shift+O abre workspaces"
    );
    click(&mut h, "Fechar  (Esc)");
    assert!(app(&mut h).modal.is_none());
    key(&mut h, ctrl_shift(), Key::O);
    click(&mut h, "+ novo workspace");
    assert_eq!(app(&mut h).workspaces.len(), n + 1, "+ novo workspace");

    // paleta: Fechar
    key(&mut h, ctrl_shift(), Key::P);
    assert!(is_modal(&mut h, |m| matches!(m, Modal::Palette { .. })));
    click(&mut h, "Fechar  (Esc)");
    assert!(app(&mut h).modal.is_none(), "Fechar a paleta");

    // sair: Não
    app(&mut h).modal = Some(Modal::ConfirmQuit);
    frames(&mut h, 2);
    click(&mut h, "Não  (Esc)");
    assert!(app(&mut h).modal.is_none(), "Não fecha o diálogo de sair");

    // renomear bloco/sessão pelos botões
    let id = terminal(&mut h, "renomeia", "sleep 60");
    app(&mut h).modal = Some(Modal::Rename {
        id,
        text: "novo nome".into(),
        first: false,
    });
    frames(&mut h, 2);
    click(&mut h, "Salvar  ⏎");
    assert_eq!(
        app(&mut h).session_label(id),
        "novo nome",
        "Salvar renomeia"
    );

    // módulo desligado: parada some e atalho fica inerte (não vaza)
    app(&mut h).cfg.modules.insert("worktrees".into(), false);
    frames(&mut h, 2);
    key(&mut h, ctrl_shift(), Key::G);
    assert_ne!(app(&mut h).view, View::Worktrees);
    app(&mut h).cfg.modules.insert("tasks".into(), false);
    frames(&mut h, 2);
    assert!(
        !exists(&h, "Aba Tarefas"),
        "Tarefas some com o módulo desligado"
    );
    key(&mut h, ctrl_shift(), Key::Space);
    assert_ne!(app(&mut h).view, View::Tasks, "atalho inerte");

    kill_all(&mut h);
}

// ───────────────────────── worktrees ─────────────────────────

fn git(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git")
        .status
        .success();
    assert!(ok, "git {args:?}");
}

#[test]
fn worktrees() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = sandbox("worktrees", "");
    let repo = dir.join("repos/proj");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("a.txt"), "a").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    let wt = dir.join("repos/proj.worktrees/feat");
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "feat", wt.to_str().unwrap()],
    );

    let mut h = harness([1400.0, 900.0]);
    app(&mut h).repos = app(&mut h).cfg.repos();
    app(&mut h).set_view(View::Worktrees);
    frames(&mut h, 3);

    click(&mut h, "Atualizar worktrees");
    click(&mut h, "Remover worktree");
    assert!(
        exists(&h, "Forçar (descarta mudanças)"),
        "🗑 pede confirmação"
    );
    click(&mut h, "Cancelar");
    assert!(!exists(&h, "Forçar (descarta mudanças)"), "Cancelar fecha");
    assert!(wt.exists());
    click(&mut h, "Remover worktree");
    click(&mut h, "Remover");
    assert!(!wt.exists(), "Remover apaga a pasta do worktree");
}

// ───────────────────────── config ─────────────────────────

#[test]
fn config_alterar_atalho() {
    use aic_app::actions::Action;
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("config", "");
    // janela alta: o catálogo de atalhos fica lá embaixo da página
    let mut h = harness([1400.0, 9000.0]);
    app(&mut h).set_view(View::Settings);
    frames(&mut h, 3);

    click(&mut h, "Alterar atalho: Sair do AIC");
    assert_eq!(app(&mut h).settings.capturing, Some(Action::Quit));
    // a combinação de outra ação é GRAVADA, não executada (antes abria a Nova sessão)
    key(&mut h, ctrl_shift(), Key::N);
    assert!(
        app(&mut h).modal.is_none(),
        "gravar Ctrl+Shift+N não pode abrir a nova sessão"
    );
    assert_eq!(app(&mut h).keymap.label(Action::Quit), "Ctrl+Shift+N");
    assert_eq!(app(&mut h).settings.capturing, None);

    click(&mut h, "Atalho padrão: Sair do AIC");
    assert_eq!(
        app(&mut h).keymap.label(Action::Quit),
        "Ctrl+Shift+Q",
        "padrão volta o atalho"
    );
    click(&mut h, "Restaurar padrões");
    assert_eq!(
        app(&mut h).keymap.label(Action::NewSession),
        "Ctrl+Shift+N",
        "restaurar padrões"
    );

    // tecla líder
    click(&mut h, "Alterar a tecla líder");
    key(&mut h, Modifiers::ALT, Key::Space);
    assert_eq!(app(&mut h).cfg.leader, "Alt+Space");
    key(&mut h, Modifiers::ALT, Key::Space);
    assert!(app(&mut h).leader.is_some(), "a nova líder abre o menu");
    key(&mut h, Modifiers::NONE, Key::Escape);
    click(&mut h, "Tecla líder padrão");
    assert_eq!(app(&mut h).cfg.leader, "Ctrl+Space");

    // tecla de prefixo de uma ação: grava a próxima tecla (texto vence: "q")
    click(&mut h, "Alterar tecla de prefixo: Sair do AIC");
    assert_eq!(app(&mut h).settings.capturing_prefix, Some(Action::Quit));
    key(&mut h, Modifiers::NONE, Key::X);
    assert_eq!(app(&mut h).keymap.prefix_keys(Action::Quit), ["x"]);
    assert!(
        app(&mut h)
            .keymap
            .prefix_keys(Action::CloseFocused)
            .is_empty(),
        "x sai de quem tinha"
    );
    click(
        &mut h,
        "Tecla de prefixo padrão: Fechar o que está em foco (o terminal, nunca a sessão inteira)",
    );
    assert_eq!(app(&mut h).keymap.prefix_keys(Action::CloseFocused), ["x"]);
    assert!(
        app(&mut h).keymap.prefix_keys(Action::Quit).is_empty(),
        "e volta pro dono"
    );

    // o prefixo em si: não aceita colidir com a líder
    click(&mut h, "Alterar o prefixo");
    key(&mut h, ctrl(), Key::Space);
    assert_eq!(app(&mut h).cfg.prefix, "Ctrl+B", "Ctrl+Space é a líder");
    click(&mut h, "Alterar o prefixo");
    key(&mut h, ctrl(), Key::A);
    assert_eq!(app(&mut h).cfg.prefix, "Ctrl+A");
    click(&mut h, "Prefixo padrão");
    assert_eq!(app(&mut h).cfg.prefix, "Ctrl+B");
}

// ───────────────────────── busca na Config ─────────────────────────

#[test]
fn busca_na_config() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("busca", "");
    let mut h = harness([1400.0, 9000.0]);
    app(&mut h).set_view(View::Settings);
    frames(&mut h, 3);
    assert!(exists(&h, "Fonte do terminal"));

    // "/" foca a busca; digitar filtra
    key(&mut h, Modifiers::NONE, Key::Slash);
    h.event(egui::Event::Text("fonte".into()));
    frames(&mut h, 3);
    assert_eq!(app(&mut h).settings.search, "fonte");
    assert!(exists(&h, "Fonte do terminal"), "a linha buscada fica");
    assert!(!exists(&h, "Escala da interface"), "o resto da seção some");
    assert!(
        !exists(&h, "Alterar atalho: Sair do AIC"),
        "outras seções somem"
    );

    // sem acento e por palavra-chave: "notificacao" acha a linha de notificação
    app(&mut h).settings.search = "notificacao".into();
    frames(&mut h, 3);
    assert!(exists(&h, "testar notificação"));
    assert!(!exists(&h, "Fonte do terminal"));

    // busca que acha atalhos pela tecla
    app(&mut h).settings.search = "workspace 3".into();
    frames(&mut h, 3);
    assert!(exists(&h, "Alterar atalho: Ir pro workspace 3"));
    assert!(!exists(&h, "Alterar atalho: Ir pro workspace 4"));

    app(&mut h).settings.search = "xyzzy".into();
    frames(&mut h, 3);
    assert!(exists(&h, "Nada encontrado para \"xyzzy\""));
    click(&mut h, "Limpar busca");
    assert!(app(&mut h).settings.search.is_empty());

    // Ctrl+B / abre a Config com a busca focada, de qualquer aba
    app(&mut h).set_view(View::Worktrees);
    frames(&mut h, 2);
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::Slash);
    assert_eq!(app(&mut h).view, View::Settings);
    h.event(egui::Event::Text("som".into()));
    frames(&mut h, 3);
    assert_eq!(
        app(&mut h).settings.search,
        "som",
        "a busca já estava focada"
    );
}

// ───────────────────────── prefixo (tmux) ─────────────────────────

#[test]
fn prefixo_estilo_tmux() {
    use aic_app::actions::Action;
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("prefixo", "");
    let mut h = harness([1480.0, 900.0]);
    let a = terminal(&mut h, "api", "sleep 60");
    assert!(app(&mut h).term_focus, "terminal focado");

    // Ctrl+B arma (com o terminal focado) e acende o indicador
    key(&mut h, ctrl(), Key::B);
    assert!(app(&mut h).prefix_pending);
    assert!(exists(&h, "Prefixo armado"));
    key(&mut h, Modifiers::NONE, Key::Escape);
    assert!(!app(&mut h).prefix_pending, "Esc cancela");

    // x fecha o que está em foco: o terminal (pede confirmação)
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::X);
    assert!(
        is_modal(
            &mut h,
            |m| matches!(m, Modal::CloseSession { id, .. } if *id == a)
        ),
        "Ctrl+B x fecha o terminal em foco"
    );
    key(&mut h, Modifiers::NONE, Key::Escape);
    app(&mut h).modal = None;

    // n/p andam pelas abas: Tarefas, Sessões, Tokens; x numa tela volta pros terminais
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::N);
    assert_eq!(app(&mut h).view, View::Usage, "Ctrl+B n");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::N);
    assert_eq!(app(&mut h).view, View::Tasks, "dá a volta");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::P);
    assert_eq!(app(&mut h).view, View::Usage, "Ctrl+B p");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::X);
    assert_eq!(app(&mut h).view, View::Sessions, "Ctrl+B x na tela volta");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::P);
    assert_eq!(app(&mut h).view, View::Tasks, "Ctrl+B p");
    app(&mut h).set_view(View::Sessions);

    // workspaces: criar, Ctrl+B 1 / 2, Tab vai e volta
    app(&mut h).new_workspace("front");
    frames(&mut h, 2);
    assert_eq!(app(&mut h).ws, 1);
    assert!(exists(&h, "Workspace 2 front"), "workspace na barra");
    assert!(
        app(&mut h).ws_sessions().is_empty(),
        "o workspace novo só mostra as sessões dele"
    );
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::Num1);
    assert_eq!(app(&mut h).ws, 0, "Ctrl+B 1");
    assert_eq!(app(&mut h).ws_sessions(), vec![a]);
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::Tab);
    assert_eq!(app(&mut h).ws, 1, "Ctrl+B Tab volta pro anterior");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::Tab);
    assert_eq!(app(&mut h).ws, 0, "e vai de novo");
    click(&mut h, "Workspace 2 front");
    assert_eq!(app(&mut h).ws, 1, "clique no workspace da barra");
    // ( e ) vêm como texto (Shift+9/0 no teclado)
    key(&mut h, ctrl(), Key::B);
    h.event(egui::Event::Text("(".into()));
    frames(&mut h, 2);
    assert_eq!(app(&mut h).ws, 0, "Ctrl+B (");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::Num9);
    assert_eq!(app(&mut h).ws, 0, "workspace que não existe não muda nada");

    // $ renomeia o workspace atual
    key(&mut h, ctrl(), Key::B);
    h.event(egui::Event::Text("$".into()));
    frames(&mut h, 2);
    assert!(
        is_modal(&mut h, |m| matches!(m, Modal::RenameWorkspace { .. })),
        "Ctrl+B $"
    );
    app(&mut h).modal = None;
    app(&mut h).rename_workspace("api");
    assert_eq!(app(&mut h).current_ws().unwrap().name, "api");

    // tecla sem ação: avisa e desarma
    app(&mut h).toasts.clear();
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::Y);
    assert!(!app(&mut h).prefix_pending);
    assert!(
        app(&mut h)
            .toasts
            .iter()
            .any(|t| t.text.contains("não faz nada"))
    );

    // ? abre o catálogo; Ctrl+B Ctrl+B não dispara nada (vai pro terminal)
    key(&mut h, ctrl(), Key::B);
    h.event(egui::Event::Text("?".into()));
    frames(&mut h, 2);
    assert!(app(&mut h).leader.is_some(), "Ctrl+B ? abre o catálogo");
    key(&mut h, Modifiers::NONE, Key::Escape);
    key(&mut h, ctrl(), Key::B);
    key(&mut h, ctrl(), Key::B);
    assert!(
        !app(&mut h).prefix_pending && app(&mut h).modal.is_none() && app(&mut h).leader.is_none()
    );

    // prefixo remapeado pelo keybinds: vale a tecla nova
    app(&mut h)
        .keymap
        .set_prefix(Action::NewWorkspace, vec!["N".into()]);
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::SHIFT, Key::N);
    assert!(
        is_modal(&mut h, |m| matches!(m, Modal::Workspaces { .. })),
        "Ctrl+B N (remapeado)"
    );
    app(&mut h).modal = None;

    kill_all(&mut h);
}

// ───────────────────────── tecla líder ─────────────────────────

#[test]
fn tecla_lider() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("lider", "");
    let mut h = harness([1480.0, 900.0]);
    let _a = terminal(&mut h, "api", "printf 'pronto\\n'; sleep 60");
    assert!(app(&mut h).term_focus, "terminal focado");

    // abre com o terminal focado
    key(&mut h, ctrl(), Key::Space);
    assert!(app(&mut h).leader.is_some(), "Ctrl+Space abre o menu");
    key(&mut h, Modifiers::NONE, Key::S);
    assert_eq!(
        app(&mut h).leader.as_ref().unwrap().path,
        vec!['s'],
        "s entra em Sessões"
    );
    key(&mut h, Modifiers::NONE, Key::Q);
    assert!(
        app(&mut h).leader.as_ref().unwrap().error.is_some(),
        "tecla inválida avisa"
    );
    key(&mut h, Modifiers::NONE, Key::Backspace);
    assert!(
        app(&mut h).leader.as_ref().unwrap().path.is_empty(),
        "Backspace volta"
    );
    key(&mut h, Modifiers::NONE, Key::Escape);
    assert!(app(&mut h).leader.is_none(), "Esc fecha");

    // líder → b → n: nova sessão (abre o diálogo)
    key(&mut h, ctrl(), Key::Space);
    key(&mut h, Modifiers::NONE, Key::B);
    key(&mut h, Modifiers::NONE, Key::N);
    assert!(
        is_modal(&mut h, |m| matches!(m, Modal::NewSession(f) if f.new_block)),
        "líder b n abre a nova sessão"
    );
    key(&mut h, Modifiers::NONE, Key::Escape);

    // clique nos itens do menu
    key(&mut h, ctrl(), Key::Space);
    click(&mut h, "Paleta de comandos");
    assert!(
        is_modal(&mut h, |m| matches!(m, Modal::Palette { .. })),
        "clique no item roda a ação"
    );
    key(&mut h, Modifiers::NONE, Key::Escape);
    key(&mut h, ctrl(), Key::Space);
    click(&mut h, "Fechar menu");
    assert!(app(&mut h).leader.is_none(), "Fechar menu");

    // módulo desligado: a tecla dele vira inválida no menu
    app(&mut h).cfg.modules.insert("tasks".into(), false);
    key(&mut h, ctrl(), Key::Space);
    key(&mut h, Modifiers::NONE, Key::T);
    assert!(
        app(&mut h).leader.as_ref().unwrap().error.is_some(),
        "t vira tecla inválida"
    );
    key(&mut h, Modifiers::NONE, Key::Escape);
    app(&mut h).cfg.modules.remove("tasks");

    // Hints desligado: a líder não abre (o Ctrl+Space vai pro terminal)
    app(&mut h).cfg.modules.insert("hints".into(), false);
    key(&mut h, ctrl(), Key::Space);
    assert!(app(&mut h).leader.is_none());
    app(&mut h).cfg.modules.remove("hints");

    // shot: menu aberto em Sessões
    key(&mut h, ctrl(), Key::Space);
    frames(&mut h, 3);
    let img = h.render().expect("render");
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/snapshots");
    std::fs::create_dir_all(&out).unwrap();
    img.save(out.join("lider.png")).unwrap();
    key(&mut h, Modifiers::NONE, Key::S);
    frames(&mut h, 3);
    h.render()
        .expect("render")
        .save(out.join("lider-sessoes.png"))
        .unwrap();

    kill_all(&mut h);
}

/// O `aic` de linha de comando (Anakin) pede ao app aberto pela caixa de entrada.
#[test]
fn caixa_de_entrada_do_anakin() {
    use aic_core::inbox::{self, Command};
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = sandbox("inbox", "");
    std::fs::create_dir_all(dir.join("repos/api/.git")).unwrap();
    let mut h = harness([1400.0, 900.0]);
    assert!(inbox::app_running(), "o app marca que está aberto");

    let id = inbox::send(Command::NewWorkspace {
        name: "Pessoal".into(),
        switch: false,
    })
    .unwrap();
    wait_until(&mut h, "workspace criado", |a| {
        a.workspaces.iter().any(|w| w.name == "Pessoal")
    });
    let r = inbox::wait_reply(&id, Duration::from_secs(2)).expect("resposta");
    assert!(r.ok, "{}", r.message);
    assert_eq!(
        app(&mut h).current_ws().unwrap().name,
        "Principal",
        "sem --go fica onde estava"
    );

    let id = inbox::send(Command::GotoWorkspace {
        name: "pess".into(),
    })
    .unwrap();
    wait_until(&mut h, "trocar de workspace", |a| {
        a.current_ws().is_some_and(|w| w.name == "Pessoal")
    });
    assert!(inbox::wait_reply(&id, Duration::from_secs(2)).unwrap().ok);

    let id = inbox::send(Command::GotoWorkspace {
        name: "nada".into(),
    })
    .unwrap();
    frames(&mut h, 2);
    std::thread::sleep(Duration::from_millis(350));
    frames(&mut h, 2);
    assert!(
        !inbox::wait_reply(&id, Duration::from_secs(2)).unwrap().ok,
        "workspace inexistente é erro"
    );

    let id = inbox::send(Command::NewSession {
        workspace: Some("Trabalho".into()),
        name: None,
        repo: Some("ap".into()),
        cwd: None,
        shell: true,
        branch: None,
        prompt: None,
    })
    .unwrap();
    wait_until(&mut h, "sessão aberta", |a| {
        a.sessions.values().any(|r| r.s.name == "api (shell)")
    });
    let r = inbox::wait_reply(&id, Duration::from_secs(2)).unwrap();
    assert!(r.ok && r.message.contains("Trabalho"), "{}", r.message);
    assert_eq!(
        app(&mut h).current_ws().unwrap().name,
        "Trabalho",
        "workspace criado e aberto"
    );

    let id = inbox::send(Command::NewSession {
        workspace: None,
        name: None,
        repo: Some("naoexiste".into()),
        cwd: None,
        shell: true,
        branch: None,
        prompt: None,
    })
    .unwrap();
    frames(&mut h, 2);
    std::thread::sleep(Duration::from_millis(350));
    frames(&mut h, 2);
    assert!(
        !inbox::wait_reply(&id, Duration::from_secs(2)).unwrap().ok,
        "repo inexistente é erro"
    );
    kill_all(&mut h);
}
