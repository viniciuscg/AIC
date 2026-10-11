//! Barra de dicas do rodapé (módulo Hints): os atalhos que valem no contexto atual.

use egui::{FontId, RichText, Ui};

use super::widgets;
use crate::actions::Action;
use crate::app::{App, View};

/// Dicas do contexto atual: (teclas, o que faz). Ações de módulos desligados ficam de fora.
pub fn context(app: &App) -> Vec<(String, &'static str)> {
    let k = |a: Action| app.keymap.label(a);
    let mut v: Vec<(String, &'static str)> = Vec::new();
    let act = |v: &mut Vec<(String, &'static str)>, a: Action, desc: &'static str| {
        if a.enabled(&app.cfg) && !k(a).is_empty() {
            v.push((k(a), desc));
        }
    };
    if app.leader.is_some() {
        return vec![("letra".into(), "escolher"), ("Backspace".into(), "voltar"), ("Esc".into(), "fechar")];
    }
    if app.settings.capturing.is_some() || app.settings.capturing_leader {
        return vec![("combinação".into(), "gravar"), ("Esc".into(), "cancelar"), ("Backspace".into(), "limpar")];
    }
    if app.modal.is_some() {
        return vec![("Enter".into(), "confirmar"), ("Esc".into(), "cancelar"), ("↑ / ↓".into(), "escolher na lista")];
    }
    v.push((super::leader::label(app), "menu de atalhos"));
    match app.view {
        View::Sessions if app.term_focus => {
            act(&mut v, Action::NewSession, "Claude");
            act(&mut v, Action::NewShell, "split");
            v.push(("Alt+1-9".into(), "bloco"));
            act(&mut v, Action::NextWaiting, "quem precisa de você");
            act(&mut v, Action::ReleaseTerminal, "soltar teclado");
        }
        View::Sessions => {
            if app.cfg.on(aic_core::Module::Board) {
                v.push(("t".into(), "nova tarefa"));
            }
            v.push(("clique no terminal".into(), "digitar"));
            act(&mut v, Action::NewSession, "Claude");
            v.push(("Alt+1-9".into(), "bloco"));
        }
        View::Board => {
            for (keys, desc) in [
                ("j/k", "mover"),
                ("h/l", "coluna"),
                ("Shift+H/L", "mover cartão"),
                ("Enter", "abrir"),
                ("e", "editar"),
                ("x", "concluir"),
                ("d", "arquivar"),
                ("t", "nova"),
                ("/", "buscar"),
            ] {
                v.push((keys.into(), desc));
            }
        }
        View::Worktrees => {
            for (keys, desc) in [("j/k", "escolher"), ("Enter", "abrir sessão"), ("n", "novo"), ("Delete", "remover"), ("r", "atualizar")] {
                v.push((keys.into(), desc));
            }
        }
        View::Settings => {
            v.push(("alterar".into(), "gravar atalho"));
            act(&mut v, Action::ViewSessions, "voltar às sessões");
        }
        View::Usage | View::ForkHelp => {
            act(&mut v, Action::ViewSessions, "voltar às sessões");
            act(&mut v, Action::Palette, "paleta");
        }
    }
    v
}

pub fn visible(app: &App) -> bool {
    app.cfg.on(aic_core::Module::Hints) && app.cfg.hints_bar
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let hints = context(app);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, (keys, desc)) in hints.iter().enumerate() {
            if i > 0 {
                ui.label(RichText::new("·").color(theme.faint));
            }
            let text = RichText::new(keys).font(FontId::monospace(11.5)).color(theme.accent_hi);
            if i == 0 && app.leader.is_none() && app.modal.is_none() {
                // a primeira dica é a líder: clicável
                if ui.add(egui::Button::new(text).frame(false)).on_hover_text("Abrir o menu de atalhos").clicked() {
                    app.leader = Some(Default::default());
                }
            } else {
                ui.label(text);
            }
            ui.label(widgets::subtle(*desc, &theme).small());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let tip = app.keymap.tip(Action::ToggleHints, "Esconder a barra de dicas");
            if widgets::icon_button(ui, widgets::I_CLOSE, "Esconder dicas", &tip).clicked() {
                app.run_action(ui.ctx(), Action::ToggleHints);
            }
        });
    });
}
