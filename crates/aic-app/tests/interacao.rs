//! Cliques e teclas de verdade (egui_kittest) em cada botão das telas: o clique chega
//! no botão certo e muda o estado do app? Pega fundo clicável roubando clique, atalho
//! engolido, botão sem efeito.
//!
//!   cargo test -p aic-app --test interacao

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use aic_app::app::{App, View};
use aic_app::ui::modals::Modal;
use aic_app::ui::widgets::{I_PLUS, I_TERM};
use aic_core::{Id, SessionKind};

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
        "setup_done = true\nanimations = false\nrepos_dirs = [\"{}\"]\n{extra_config}\n[modules]\ntmux = false\nlayne = false\nsound = false\nnotifications = false\n",
        dir.join("repos").display()
    );
    std::fs::write(dir.join("config/config.toml"), cfg).unwrap();
    dir
}

type H = Harness<'static, Option<App>>;

fn harness(size: [f32; 2]) -> H {
    let mut h = Harness::builder().with_size(egui::vec2(size[0], size[1])).build_ui_state(
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
    h.get_all_by_label(label).next().unwrap_or_else(|| panic!("sem botão \"{label}\"")).click();
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
    let block = a.new_block(name, None);
    let id = a.create_session_in(&ctx, block, name.into(), SessionKind::Command(cmd.into()), std::env::temp_dir(), None, None, vec![], None);
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

#[test]
fn cabecalho_do_terminal() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = sandbox("terminal", "");
    let mut h = harness([1400.0, 900.0]);
    let id = terminal(&mut h, "api", "sleep 60");

    click(&mut h, "Maximizar terminal");
    assert!(app(&mut h).maximized, "maximizar");
    click(&mut h, "Restaurar terminal");
    assert!(!app(&mut h).maximized, "restaurar");

    click(&mut h, "Fechar terminal");
    assert!(is_modal(&mut h, |m| matches!(m, Modal::CloseSession { .. })), "✕ pede confirmação");
    click(&mut h, "Cancelar");
    assert!(app(&mut h).modal.is_none() && app(&mut h).sessions.contains_key(&id), "cancelar mantém o terminal");
    click(&mut h, "Fechar terminal");
    click(&mut h, "Fechar  ⏎");
    assert!(!app(&mut h).sessions.contains_key(&id), "fechar encerra o terminal");

    // reiniciar: o processo sai na 1ª vez e fica rodando na 2ª
    let marker = dir.join("marker");
    let cmd = format!("test -f {m} || {{ touch {m}; exit 0; }}; sleep 60", m = marker.display());
    let id = terminal(&mut h, "reinicia", &cmd);
    wait_until(&mut h, "processo sair", |a| a.sessions[&id].term.as_ref().is_some_and(|t| t.is_exited()));
    frames(&mut h, 2);
    assert!(exists(&h, &format!("{} Reabrir (Enter)", aic_app::ui::widgets::I_RESTART)), "tela de processo encerrado");
    click(&mut h, "Reiniciar terminal");
    wait_until(&mut h, "terminal reiniciado", |a| a.sessions[&id].term.as_ref().is_some_and(|t| !t.is_exited()));

    kill_all(&mut h);
}

// ───────────────────────── bloco ─────────────────────────

#[test]
fn cabecalho_do_bloco() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("bloco", "");
    let mut h = harness([1500.0, 900.0]);
    let id = terminal(&mut h, "bloco-a", "sleep 60");
    let block = app(&mut h).block_of(id).unwrap();

    let side = app(&mut h).blocks[&block].side;
    click(&mut h, "Painel do bloco");
    assert_ne!(app(&mut h).blocks[&block].side, side, "◧ alterna o painel");

    let split = app(&mut h).blocks[&block].split;
    click(&mut h, &format!("⊞ {}", split.label()));
    assert_ne!(app(&mut h).blocks[&block].split, split, "⊞ muda o arranjo");

    click(&mut h, &format!("{I_PLUS} Claude"));
    assert!(is_modal(&mut h, |m| matches!(m, Modal::NewSession(_))), "+ Claude abre o diálogo");
    click(&mut h, "Cancelar");
    assert!(app(&mut h).modal.is_none(), "cancelar fecha a nova sessão");

    let n = app(&mut h).blocks[&block].panes.len();
    click(&mut h, &format!("{I_TERM} terminal"));
    assert_eq!(app(&mut h).blocks[&block].panes.len(), n + 1, "terminal abre um split no bloco");

    click(&mut h, "Recolher bloco");
    assert!(app(&mut h).blocks[&block].collapsed, "recolher");
    click(&mut h, "Expandir bloco");
    assert!(!app(&mut h).blocks[&block].collapsed, "expandir");

    click(&mut h, "Fechar bloco");
    assert!(is_modal(&mut h, |m| matches!(m, Modal::CloseBlock { .. })), "✕ do bloco pede confirmação");
    click(&mut h, "Cancelar  (Esc)");
    assert!(app(&mut h).modal.is_none() && app(&mut h).blocks.contains_key(&block));
    click(&mut h, "Fechar bloco");
    click(&mut h, "Fechar  ⏎");
    assert!(!app(&mut h).blocks.contains_key(&block), "fechar bloco remove o bloco");

    // bloco novo pelo + da lista e renomear pelo diálogo
    let b = app(&mut h).new_block("vazio", None).unwrap();
    frames(&mut h, 3);
    let label = format!("{I_PLUS}  Novo bloco  ({})", app(&mut h).keymap.label(aic_app::actions::Action::NewBlock));
    click(&mut h, &label);
    assert!(is_modal(&mut h, |m| matches!(m, Modal::RenameBlock { .. })), "novo bloco pede o nome");
    click(&mut h, "Cancelar  (Esc)");
    assert!(app(&mut h).modal.is_none());
    assert!(app(&mut h).blocks.contains_key(&b));

    kill_all(&mut h);
}

// ───────────────────────── tarefas ─────────────────────────

#[test]
fn captura_rapida_e_tarefas_do_bloco() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("tarefas", "");
    let mut h = harness([1500.0, 900.0]);
    let id = terminal(&mut h, "api", "sleep 60");
    let block = app(&mut h).block_of(id).unwrap();

    // Ctrl+Shift+Space → captura rápida → Criar
    key(&mut h, ctrl_shift(), Key::Space);
    assert!(is_modal(&mut h, |m| matches!(m, Modal::QuickAdd { .. })), "Ctrl+Shift+Space abre a captura");
    if let Some(Modal::QuickAdd { text, link_enabled, .. }) = &mut app(&mut h).modal {
        *text = "tarefa criada no clique".into();
        *link_enabled = true;
    }
    frames(&mut h, 2);
    click(&mut h, "Criar  ⏎");
    assert!(app(&mut h).modal.is_none());
    let t = app(&mut h).tasks.iter().find(|t| t.title == "tarefa criada no clique").cloned().expect("Criar cria a tarefa");
    assert_eq!(t.session, Some(id), "vinculada ao terminal em foco");

    key(&mut h, ctrl_shift(), Key::Space);
    click(&mut h, "Cancelar");
    assert!(app(&mut h).modal.is_none(), "Cancelar fecha a captura");

    // painel Tarefas do bloco: a tarefa aparece e o check conclui
    app(&mut h).update_block(block, |b| b.side = true);
    frames(&mut h, 3);
    assert!(exists(&h, "tarefa criada no clique"), "a tarefa aparece no painel do bloco");

    kill_all(&mut h);
}

// ───────────────────────── modais, toasts, topbar ─────────────────────────

#[test]
fn modais_toasts_e_abas() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _dir = sandbox("modais", "");
    let mut h = harness([1500.0, 900.0]);
    let _id = terminal(&mut h, "x", "sleep 60");

    // abas da topbar
    for (label, view) in [
        ("Aba Worktrees", View::Worktrees),
        ("Aba Tokens", View::Usage),
        ("Aba Config", View::Settings),
        ("Aba Ajuda", View::Help),
        ("Aba Sessões", View::Sessions),
    ] {
        click(&mut h, label);
        assert_eq!(app(&mut h).view, view, "aba {label}");
    }

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
    assert!(is_modal(&mut h, |m| matches!(m, Modal::Workspaces { .. })), "Ctrl+Shift+O abre workspaces");
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
    app(&mut h).modal = Some(Modal::Rename { id, text: "novo nome".into(), first: false });
    frames(&mut h, 2);
    click(&mut h, "Salvar  ⏎");
    assert_eq!(app(&mut h).session_label(id), "novo nome", "Salvar renomeia");

    // módulo desligado: aba some e atalho fica inerte (não vaza)
    app(&mut h).cfg.modules.insert("worktrees".into(), false);
    frames(&mut h, 2);
    assert!(!exists(&h, "Aba Worktrees"), "aba Worktrees some com o módulo desligado");
    key(&mut h, ctrl_shift(), Key::G);
    assert_ne!(app(&mut h).view, View::Worktrees);
    app(&mut h).cfg.modules.insert("tasks".into(), false);
    key(&mut h, ctrl_shift(), Key::Space);
    assert!(app(&mut h).modal.is_none(), "captura rápida inerte com Tarefas desligado");

    kill_all(&mut h);
}

// ───────────────────────── worktrees ─────────────────────────

fn git(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
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
    git(&repo, &["worktree", "add", "-q", "-b", "feat", wt.to_str().unwrap()]);

    let mut h = harness([1400.0, 900.0]);
    app(&mut h).repos = app(&mut h).cfg.repos();
    app(&mut h).set_view(View::Worktrees);
    frames(&mut h, 3);

    click(&mut h, "Atualizar worktrees");
    click(&mut h, "Remover worktree");
    assert!(exists(&h, "Forçar (descarta mudanças)"), "🗑 pede confirmação");
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
    assert!(app(&mut h).modal.is_none(), "gravar Ctrl+Shift+N não pode abrir a nova sessão");
    assert_eq!(app(&mut h).keymap.label(Action::Quit), "Ctrl+Shift+N");
    assert_eq!(app(&mut h).settings.capturing, None);

    click(&mut h, "Atalho padrão: Sair do AIC");
    assert_eq!(app(&mut h).keymap.label(Action::Quit), "Ctrl+Shift+Q", "padrão volta o atalho");
    click(&mut h, "Restaurar padrões");
    assert_eq!(app(&mut h).keymap.label(Action::NewSession), "Ctrl+Shift+N", "restaurar padrões");

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
    assert!(app(&mut h).keymap.prefix_keys(Action::CloseFocused).is_empty(), "x sai de quem tinha");
    click(&mut h, "Tecla de prefixo padrão: Fechar o que está em foco (terminal, bloco vazio ou a aba)");
    assert_eq!(app(&mut h).keymap.prefix_keys(Action::CloseFocused), ["x"]);
    assert!(app(&mut h).keymap.prefix_keys(Action::Quit).is_empty(), "e volta pro dono");

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
    assert!(!exists(&h, "Alterar atalho: Sair do AIC"), "outras seções somem");

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
    assert_eq!(app(&mut h).settings.search, "som", "a busca já estava focada");
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
    assert!(is_modal(&mut h, |m| matches!(m, Modal::CloseSession { id, .. } if *id == a)), "Ctrl+B x fecha o terminal em foco");
    key(&mut h, Modifiers::NONE, Key::Escape);
    app(&mut h).modal = None;

    // n/p andam pelas abas da barra; x numa aba volta pras Sessões
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::N);
    assert_eq!(app(&mut h).view, View::Worktrees, "Ctrl+B n");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::N);
    assert_eq!(app(&mut h).view, View::Changes);
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::P);
    assert_eq!(app(&mut h).view, View::Worktrees, "Ctrl+B p");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::X);
    assert_eq!(app(&mut h).view, View::Sessions, "Ctrl+B x na aba volta pras sessões");
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::P);
    let last = *app(&mut h).tabs().last().unwrap();
    assert_eq!(app(&mut h).view, last, "p dá a volta");
    app(&mut h).set_view(View::Sessions);

    // workspaces: criar, Ctrl+B 1 / 2, Tab vai e volta
    app(&mut h).new_workspace("front");
    frames(&mut h, 2);
    assert_eq!(app(&mut h).ws, 1);
    assert!(exists(&h, "Workspace 2 front"), "workspace na barra");
    assert!(app(&mut h).ws_sessions().is_empty(), "o workspace novo só mostra as sessões dele");
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
    assert!(is_modal(&mut h, |m| matches!(m, Modal::RenameWorkspace { .. })), "Ctrl+B $");
    app(&mut h).modal = None;
    app(&mut h).rename_workspace("api");
    assert_eq!(app(&mut h).current_ws().unwrap().name, "api");

    // tecla sem ação: avisa e desarma
    app(&mut h).toasts.clear();
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::NONE, Key::Y);
    assert!(!app(&mut h).prefix_pending);
    assert!(app(&mut h).toasts.iter().any(|t| t.text.contains("não faz nada")));

    // ? abre o catálogo; Ctrl+B Ctrl+B não dispara nada (vai pro terminal)
    key(&mut h, ctrl(), Key::B);
    h.event(egui::Event::Text("?".into()));
    frames(&mut h, 2);
    assert!(app(&mut h).leader.is_some(), "Ctrl+B ? abre o catálogo");
    key(&mut h, Modifiers::NONE, Key::Escape);
    key(&mut h, ctrl(), Key::B);
    key(&mut h, ctrl(), Key::B);
    assert!(!app(&mut h).prefix_pending && app(&mut h).modal.is_none() && app(&mut h).leader.is_none());

    // prefixo remapeado pelo keybinds: vale a tecla nova
    app(&mut h).keymap.set_prefix(Action::NewWorkspace, vec!["N".into()]);
    key(&mut h, ctrl(), Key::B);
    key(&mut h, Modifiers::SHIFT, Key::N);
    assert!(is_modal(&mut h, |m| matches!(m, Modal::Workspaces { .. })), "Ctrl+B N (remapeado)");
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
    assert_eq!(app(&mut h).leader.as_ref().unwrap().path, vec!['s'], "s entra em Sessões");
    key(&mut h, Modifiers::NONE, Key::Q);
    assert!(app(&mut h).leader.as_ref().unwrap().error.is_some(), "tecla inválida avisa");
    key(&mut h, Modifiers::NONE, Key::Backspace);
    assert!(app(&mut h).leader.as_ref().unwrap().path.is_empty(), "Backspace volta");
    key(&mut h, Modifiers::NONE, Key::Escape);
    assert!(app(&mut h).leader.is_none(), "Esc fecha");

    // líder → b → n: novo bloco (abre o nome)
    let blocks = app(&mut h).blocks.len();
    key(&mut h, ctrl(), Key::Space);
    key(&mut h, Modifiers::NONE, Key::B);
    key(&mut h, Modifiers::NONE, Key::N);
    assert_eq!(app(&mut h).blocks.len(), blocks + 1, "líder b n cria bloco");
    assert!(is_modal(&mut h, |m| matches!(m, Modal::RenameBlock { .. })));
    key(&mut h, Modifiers::NONE, Key::Escape);

    // clique nos itens do menu
    key(&mut h, ctrl(), Key::Space);
    click(&mut h, "Paleta de comandos");
    assert!(is_modal(&mut h, |m| matches!(m, Modal::Palette { .. })), "clique no item roda a ação");
    key(&mut h, Modifiers::NONE, Key::Escape);
    key(&mut h, ctrl(), Key::Space);
    click(&mut h, "Fechar menu");
    assert!(app(&mut h).leader.is_none(), "Fechar menu");

    // módulo desligado: o grupo Tarefas some do menu
    app(&mut h).cfg.modules.insert("tasks".into(), false);
    key(&mut h, ctrl(), Key::Space);
    assert!(!exists(&h, "Tarefas  ›"), "grupo de módulo desligado não aparece");
    key(&mut h, Modifiers::NONE, Key::T);
    assert!(app(&mut h).leader.as_ref().unwrap().error.is_some(), "t vira tecla inválida");
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
    h.render().expect("render").save(out.join("lider-sessoes.png")).unwrap();

    kill_all(&mut h);
}

/// O `aic` de linha de comando (Layne) pede ao app aberto pela caixa de entrada.
#[test]
fn caixa_de_entrada_do_layne() {
    use aic_core::inbox::{self, Command};
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = sandbox("inbox", "");
    std::fs::create_dir_all(dir.join("repos/api/.git")).unwrap();
    let mut h = harness([1400.0, 900.0]);
    assert!(inbox::app_running(), "o app marca que está aberto");

    let id = inbox::send(Command::NewWorkspace { name: "Pessoal".into(), switch: false }).unwrap();
    wait_until(&mut h, "workspace criado", |a| a.workspaces.iter().any(|w| w.name == "Pessoal"));
    let r = inbox::wait_reply(&id, Duration::from_secs(2)).expect("resposta");
    assert!(r.ok, "{}", r.message);
    assert_eq!(app(&mut h).current_ws().unwrap().name, "Principal", "sem --go fica onde estava");

    let id = inbox::send(Command::GotoWorkspace { name: "pess".into() }).unwrap();
    wait_until(&mut h, "trocar de workspace", |a| a.current_ws().is_some_and(|w| w.name == "Pessoal"));
    assert!(inbox::wait_reply(&id, Duration::from_secs(2)).unwrap().ok);

    let id = inbox::send(Command::GotoWorkspace { name: "nada".into() }).unwrap();
    frames(&mut h, 2);
    std::thread::sleep(Duration::from_millis(350));
    frames(&mut h, 2);
    assert!(!inbox::wait_reply(&id, Duration::from_secs(2)).unwrap().ok, "workspace inexistente é erro");

    let id = inbox::send(Command::NewSession {
        workspace: Some("Trabalho".into()),
        name: None,
        repo: Some("ap".into()),
        cwd: None,
        shell: true,
        branch: None,
        prompt: None,
        task: None,
    })
    .unwrap();
    wait_until(&mut h, "sessão aberta", |a| a.sessions.values().any(|r| r.s.name == "api (shell)"));
    let r = inbox::wait_reply(&id, Duration::from_secs(2)).unwrap();
    assert!(r.ok && r.message.contains("Trabalho"), "{}", r.message);
    assert_eq!(app(&mut h).current_ws().unwrap().name, "Trabalho", "workspace criado e aberto");

    let id = inbox::send(Command::NewSession { workspace: None, name: None, repo: Some("naoexiste".into()), cwd: None, shell: true, branch: None, prompt: None, task: None }).unwrap();
    frames(&mut h, 2);
    std::thread::sleep(Duration::from_millis(350));
    frames(&mut h, 2);
    assert!(!inbox::wait_reply(&id, Duration::from_secs(2)).unwrap().ok, "repo inexistente é erro");
    kill_all(&mut h);
}
