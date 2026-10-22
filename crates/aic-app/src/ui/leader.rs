//! Tecla líder (estilo which-key): aperta `cfg.leader` e aparece o catálogo com os atalhos
//! agrupados por letra. Cada item mostra a tecla, o que faz, o atalho direto e o acorde de
//! prefixo equivalentes (pra ir decorando o caminho rápido).
//!
//! Enquanto o menu está aberto ele engole TODO o teclado (nada vaza pro terminal).

use egui::{Align2, Context, Event, FontId, Frame, Key, Margin, RichText, Vec2};

use super::widgets;
use crate::actions::{Action, consume_exact, format_shortcut, parse_shortcut};
use crate::app::App;

#[derive(Debug, Clone, Default)]
pub struct LeaderState {
    /// Teclas dos grupos abertos (vazio = raiz).
    pub path: Vec<char>,
    /// Aviso de tecla inválida.
    pub error: Option<String>,
}

pub enum Item {
    Group(char, &'static str, Vec<Item>),
    Run(char, &'static str, Action),
}

impl Item {
    pub fn key(&self) -> char {
        match self {
            Item::Group(k, ..) | Item::Run(k, ..) => *k,
        }
    }
}

/// A árvore inteira, sem filtro.
fn tree() -> Vec<Item> {
    use Action::*;
    use Item::{Group, Run};
    vec![
        Run('x', "Fechar o que está em foco", CloseFocused),
        Group(
            's',
            "Sessões (terminais)",
            vec![
                Run('n', "Novo Claude", NewSession),
                Run('t', "Novo terminal ao lado", NewShell),
                Run('x', "Fechar terminal", CloseSession),
                Run('r', "Renomear terminal", RenameSession),
                Run('R', "Reiniciar terminal", RestartSession),
                Run('c', "Mandar contexto da tarefa", SendContext),
                Run('a', "Próximo que precisa de você", NextWaiting),
                Run('m', "Maximizar / restaurar", ToggleMaximize),
                Run(']', "Próximo terminal", NextSession),
                Run('[', "Terminal anterior", PrevSession),
                Run('h', "Foco à esquerda", FocusLeft),
                Run('j', "Foco abaixo", FocusDown),
                Run('k', "Foco acima", FocusUp),
                Run('l', "Foco à direita", FocusRight),
                Run('z', "Soltar o teclado do terminal", ReleaseTerminal),
                Run('s', "Ir pra aba Sessões", ViewSessions),
            ],
        ),
        Group(
            'b',
            "Blocos",
            vec![
                Run('n', "Novo bloco", NewBlock),
                Run('x', "Fechar bloco", CloseBlock),
                Run('r', "Renomear bloco", RenameBlock),
                Run('a', "Arranjo dos terminais", CycleSplit),
                Run('p', "Painel do bloco", ToggleBlockSide),
                Run('c', "Recolher / expandir", CollapseBlock),
                Run('k', "Subir bloco", MoveBlockUp),
                Run('j', "Descer bloco", MoveBlockDown),
                Run('1', "Ir pro bloco 1", Goto(1)),
                Run('2', "Ir pro bloco 2", Goto(2)),
                Run('3', "Ir pro bloco 3", Goto(3)),
                Run('4', "Ir pro bloco 4", Goto(4)),
                Run('5', "Ir pro bloco 5", Goto(5)),
                Run('6', "Ir pro bloco 6", Goto(6)),
                Run('7', "Ir pro bloco 7", Goto(7)),
                Run('8', "Ir pro bloco 8", Goto(8)),
                Run('9', "Ir pro bloco 9", Goto(9)),
            ],
        ),
        Group('t', "Tarefas", vec![Run('n', "Nova tarefa", QuickAdd)]),
        Group(
            'v',
            "Visualização",
            vec![
                Run('v', "Trocar visualização", CycleView),
                Run('l', "Foco ⇄ grade", ToggleLayout),
                Run('d', "Lista lateral", ToggleSidebar),
                Run('+', "Aumentar fonte", ZoomIn),
                Run('-', "Diminuir fonte", ZoomOut),
                Run('0', "Fonte padrão", ZoomReset),
            ],
        ),
        Group(
            'w',
            "Workspaces",
            vec![
                Run('w', "Trocar de workspace", Workspaces),
                Run('n', "Criar workspace", NewWorkspace),
                Run('r', "Renomear workspace", RenameWorkspace),
                Run(']', "Próximo workspace", NextWorkspace),
                Run('[', "Workspace anterior", PrevWorkspace),
                Run('l', "Voltar pro anterior (vai e volta)", LastWorkspace),
                Run('1', "Workspace 1", GotoWorkspace(1)),
                Run('2', "Workspace 2", GotoWorkspace(2)),
                Run('3', "Workspace 3", GotoWorkspace(3)),
                Run('4', "Workspace 4", GotoWorkspace(4)),
                Run('5', "Workspace 5", GotoWorkspace(5)),
                Run('6', "Workspace 6", GotoWorkspace(6)),
                Run('7', "Workspace 7", GotoWorkspace(7)),
                Run('8', "Workspace 8", GotoWorkspace(8)),
                Run('9', "Workspace 9", GotoWorkspace(9)),
            ],
        ),
        Group(
            'a',
            "Abas",
            vec![
                Run('s', "Sessões", ViewSessions),
                Run('g', "Worktrees", ViewWorktrees),
                Run('d', "Changes (diff)", ViewChanges),
                Run('k', "Tokens", ViewUsage),
                Run(',', "Config", ViewSettings),
                Run('/', "Pesquisar na Config", SearchSettings),
                Run('?', "Manual", ViewHelp),
                Run('n', "Próxima aba", NextView),
                Run('p', "Aba anterior", PrevView),
            ],
        ),
        Run('g', "Worktrees", ViewWorktrees),
        Run('d', "Changes (diff)", ViewChanges),
        Run('l', "Layne (assistente)", ToggleLayne),
        Run('m', "Falar com o Layne", LayneVoice),
        Run('n', "Notas da sessão", ToggleNotes),
        Run('k', "Tokens e custo", ViewUsage),
        Run('p', "Paleta de comandos", Palette),
        Run(',', "Configurações", ViewSettings),
        Run('?', "Manual", ViewHelp),
        Run('/', "Pesquisar na Config", SearchSettings),
        Run('q', "Sair", Quit),
    ]
}

/// Árvore com as ações de módulos desligados removidas (e grupos que ficaram vazios).
pub fn items(app: &App) -> Vec<Item> {
    fn filter(app: &App, v: Vec<Item>) -> Vec<Item> {
        v.into_iter()
            .filter_map(|i| match i {
                Item::Run(_, _, a) if !a.enabled(&app.cfg) => None,
                Item::Group(k, l, sub) => {
                    let sub = filter(app, sub);
                    (!sub.is_empty()).then_some(Item::Group(k, l, sub))
                }
                i => Some(i),
            })
            .collect()
    }
    filter(app, tree())
}

/// Itens do nível aberto (seguindo `path`) e os nomes dos grupos no caminho.
fn level(app: &App, path: &[char]) -> (Vec<Item>, Vec<&'static str>) {
    let mut cur = items(app);
    let mut names = vec![];
    for k in path {
        let Some(pos) = cur
            .iter()
            .position(|i| matches!(i, Item::Group(g, ..) if g == k))
        else {
            break;
        };
        if let Item::Group(_, l, sub) = cur.swap_remove(pos) {
            names.push(l);
            cur = sub;
        }
    }
    (cur, names)
}

/// Tecla líder normalizada ("Ctrl+Space").
pub fn label(app: &App) -> String {
    parse_shortcut(&app.cfg.leader)
        .map(|s| format_shortcut(&s))
        .unwrap_or_else(|| app.cfg.leader.clone())
}

/// Caractere de uma tecla (pela tecla lógica, então respeita o layout do teclado).
fn key_char(key: Key, shift: bool) -> Option<char> {
    Some(match key {
        Key::Comma => ',',
        Key::Period => '.',
        Key::Minus => '-',
        Key::Plus | Key::Equals => '+',
        Key::OpenBracket => '[',
        Key::CloseBracket => ']',
        Key::Questionmark => '?',
        Key::Slash if shift => '?',
        Key::Slash => '/',
        k => {
            let name = k.name();
            let c = name.chars().next().filter(|_| name.chars().count() == 1)?;
            if !c.is_ascii_alphanumeric() {
                return None;
            }
            if shift {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            }
        }
    })
}

/// Trata o teclado da líder. Devolve `true` se a líder tomou o teclado neste frame
/// (aí os atalhos normais não rodam).
pub fn handle(app: &mut App, ctx: &Context) -> bool {
    if !app.cfg.on(aic_core::Module::Hints) {
        app.leader = None;
        return false;
    }
    let Some(sc) = parse_shortcut(&app.cfg.leader) else {
        return false;
    };
    if app.leader.is_none() {
        if ctx.input_mut(|i| consume_exact(i, sc.modifiers, sc.logical_key)) {
            app.leader = Some(LeaderState::default());
            // o resto do teclado deste frame também não vai pro terminal
            ctx.input_mut(|i| {
                i.events
                    .retain(|e| !matches!(e, Event::Key { .. } | Event::Text(_)))
            });
            return true;
        }
        return false;
    }
    // menu aberto: engole todas as teclas e o texto
    let keys: Vec<(Key, egui::Modifiers)> = ctx.input_mut(|i| {
        let mut out = vec![];
        i.events.retain(|e| match e {
            Event::Key {
                key,
                pressed,
                modifiers,
                ..
            } => {
                if *pressed {
                    out.push((*key, *modifiers));
                }
                false
            }
            Event::Text(_) | Event::Paste(_) => false,
            _ => true,
        });
        out
    });
    let mut run = None;
    for (key, mods) in keys {
        let Some(st) = app.leader.as_mut() else { break };
        if key == sc.logical_key && crate::actions::same_mods(mods, sc.modifiers) {
            app.leader = None;
            break;
        }
        match key {
            Key::Escape => {
                app.leader = None;
                break;
            }
            Key::Backspace => {
                st.error = None;
                if st.path.pop().is_none() {
                    app.leader = None;
                    break;
                }
                continue;
            }
            _ => {}
        }
        let Some(c) = key_char(key, mods.shift) else {
            continue;
        };
        let path = st.path.clone();
        let (lvl, _) = level(app, &path);
        // tecla em maiúscula sem item próprio vale a minúscula (Shift esquecido)
        let found = lvl
            .iter()
            .position(|i| i.key() == c)
            .or_else(|| lvl.iter().position(|i| i.key() == c.to_ascii_lowercase()));
        let st = app.leader.as_mut().expect("líder aberta");
        match found.map(|p| &lvl[p]) {
            Some(Item::Group(k, ..)) => {
                st.path.push(*k);
                st.error = None;
            }
            Some(Item::Run(_, _, a)) => {
                run = Some(*a);
                app.leader = None;
                break;
            }
            None => st.error = Some(format!("\"{c}\" não faz nada aqui")),
        }
    }
    if let Some(a) = run {
        app.run_action(ctx, a);
    }
    true
}

/// Desenha o menu (se aberto). Itens também funcionam com clique.
pub fn show(app: &mut App, ctx: &Context) {
    let Some(st) = app.leader.clone() else { return };
    let theme = app.theme.clone();
    let (lvl, names) = level(app, &st.path);
    let mut run = None;
    let mut open_group = None;
    let mut back = false;
    let mut close = false;
    egui::Area::new(egui::Id::new("leader"))
        .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -70.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            Frame::new()
                .fill(theme.bg2)
                .stroke(egui::Stroke::new(1.5, theme.accent))
                .corner_radius(theme.radius())
                .inner_margin(Margin::symmetric(16, 12))
                .shadow(ctx.global_style().visuals.popup_shadow)
                .show(ui, |ui| {
                    ui.set_min_width(560.0);
                    ui.horizontal(|ui| {
                        widgets::kbd(ui, &label(app), &theme);
                        let crumb = std::iter::once("Atalhos")
                            .chain(names.iter().copied())
                            .collect::<Vec<_>>()
                            .join("  ›  ");
                        ui.label(
                            RichText::new(crumb)
                                .font(FontId::monospace(13.0))
                                .color(theme.accent_hi)
                                .strong(),
                        );
                    });
                    ui.add_space(6.0);
                    // duas colunas quando a lista é grande
                    let half = if lvl.len() > 9 {
                        lvl.len().div_ceil(2)
                    } else {
                        lvl.len()
                    };
                    ui.horizontal_top(|ui| {
                        for chunk in lvl.chunks(half.max(1)) {
                            egui::Grid::new((
                                "leader-col",
                                chunk.first().map(|i| i.key()).unwrap_or(' '),
                            ))
                            .num_columns(4)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                for item in chunk {
                                    widgets::kbd(ui, &item.key().to_string(), &theme);
                                    match item {
                                        Item::Group(k, l, sub) => {
                                            let text = RichText::new(format!("{l}  ›"))
                                                .color(theme.accent_hi);
                                            if ui
                                                .add(egui::Button::new(text).frame(false))
                                                .on_hover_text(format!("{} itens", sub.len()))
                                                .clicked()
                                            {
                                                open_group = Some(*k);
                                            }
                                            ui.label("");
                                            ui.label("");
                                        }
                                        Item::Run(_, l, a) => {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new(*l).color(theme.ink),
                                                    )
                                                    .frame(false),
                                                )
                                                .on_hover_text(a.meta().name)
                                                .clicked()
                                            {
                                                run = Some(*a);
                                            }
                                            ui.label(
                                                RichText::new(app.keymap.label(*a))
                                                    .font(FontId::monospace(11.0))
                                                    .color(theme.faint),
                                            );
                                            ui.label(
                                                RichText::new(super::prefix::chord(app, *a))
                                                    .font(FontId::monospace(11.0))
                                                    .color(theme.accent),
                                            );
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                            ui.add_space(18.0);
                        }
                    });
                    ui.add_space(6.0);
                    if let Some(e) = &st.error {
                        ui.label(RichText::new(format!("⚠ {e}")).color(theme.warn));
                    }
                    ui.horizontal(|ui| {
                        ui.label(
                            widgets::subtle("letra escolhe · Backspace volta · Esc fecha", &theme)
                                .small(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Fechar menu").clicked() {
                                close = true;
                            }
                            if !st.path.is_empty() && ui.small_button("Voltar").clicked() {
                                back = true;
                            }
                        });
                    });
                });
        });
    if close {
        app.leader = None;
    } else if back {
        if let Some(l) = app.leader.as_mut() {
            l.path.pop();
            l.error = None;
        }
    } else if let Some(k) = open_group {
        if let Some(l) = app.leader.as_mut() {
            l.path.push(k);
            l.error = None;
        }
    } else if let Some(a) = run {
        app.leader = None;
        app.run_action(ctx, a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(v: &[Item]) -> Vec<char> {
        v.iter().map(Item::key).collect()
    }

    /// Nenhuma tecla repetida dentro do mesmo nível.
    #[test]
    fn teclas_unicas_por_nivel() {
        fn check(v: &[Item], at: &str) {
            let mut seen = std::collections::HashSet::new();
            for k in keys(v) {
                assert!(seen.insert(k), "tecla {k} repetida em {at}");
            }
            for i in v {
                if let Item::Group(_, l, sub) = i {
                    check(sub, l);
                }
            }
        }
        check(&tree(), "raiz");
    }

    #[test]
    fn todas_as_teclas_sao_digitaveis() {
        fn check(v: &[Item]) {
            for i in v {
                let c = i.key();
                assert!(
                    c.is_ascii_alphanumeric() || ",.-+[]?/".contains(c),
                    "tecla {c}"
                );
                if let Item::Group(_, _, sub) = i {
                    check(sub);
                }
            }
        }
        check(&tree());
    }
}
