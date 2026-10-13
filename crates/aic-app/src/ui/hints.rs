//! Barra de dicas do rodapé (módulo Hints): os atalhos que valem no contexto atual.

use egui::{FontId, RichText, Ui};

use super::widgets;
use crate::actions::Action;
use crate::app::{App, View};

/// Dicas do contexto atual: (teclas, o que faz). Ações de módulos desligados ficam de fora.
/// Prefere o acorde de prefixo (`Ctrl+B c`); sem tecla de prefixo, usa o atalho direto.
pub fn context(app: &App) -> Vec<(String, &'static str)> {
    let mut v: Vec<(String, &'static str)> = Vec::new();
    // com o prefixo armado: só a tecla (o prefixo já foi)
    if app.prefix_pending {
        let key = |a: Action| app.keymap.prefix_keys(a).first().cloned();
        let mut out = vec![];
        for (a, desc) in [
            (Action::NewSession, "Claude"),
            (Action::NewShell, "split"),
            (Action::CloseFocused, "fechar"),
            (Action::NextView, "próxima aba"),
            (Action::PrevView, "aba anterior"),
            (Action::FocusLeft, "foco"),
            (Action::ToggleMaximize, "maximizar"),
            (Action::NextWaiting, "quem precisa de você"),
            (Action::Workspaces, "workspaces"),
            (Action::LastWorkspace, "workspace anterior"),
            (Action::QuickAdd, "tarefa"),
            (Action::Catalog, "todos"),
        ] {
            if let Some(k) = key(a).filter(|_| a.enabled(&app.cfg)) {
                out.push((k, desc));
            }
        }
        if app.cfg.on(aic_core::Module::Workspaces) {
            out.insert(5, ("1-9".into(), "workspace"));
        }
        out.push(("Esc".into(), "cancelar"));
        return out;
    }
    let act = |v: &mut Vec<(String, &'static str)>, a: Action, desc: &'static str| {
        if !a.enabled(&app.cfg) {
            return;
        }
        let keys = Some(super::prefix::chord(app, a)).filter(|k| !k.is_empty()).unwrap_or_else(|| app.keymap.label(a));
        if !keys.is_empty() {
            v.push((keys, desc));
        }
    };
    if app.leader.is_some() {
        return vec![("letra".into(), "escolher"), ("Backspace".into(), "voltar"), ("Esc".into(), "fechar")];
    }
    if app.settings.is_capturing() {
        return vec![("tecla".into(), "gravar"), ("Esc".into(), "cancelar"), ("Backspace".into(), "limpar")];
    }
    if app.modal.is_some() {
        return vec![("Enter".into(), "confirmar"), ("Esc".into(), "cancelar"), ("↑ / ↓".into(), "escolher na lista")];
    }
    v.push((super::leader::label(app), "catálogo"));
    match app.view {
        View::Sessions if app.term_focus => {
            act(&mut v, Action::NewSession, "Claude");
            act(&mut v, Action::NewShell, "split");
            act(&mut v, Action::CloseFocused, "fechar");
            act(&mut v, Action::FocusRight, "foco");
            act(&mut v, Action::NextView, "aba");
            if app.cfg.on(aic_core::Module::Workspaces) && !super::prefix::label(app).is_empty() {
                v.push((format!("{} 1-9", super::prefix::label(app)), "workspace"));
            }
            act(&mut v, Action::NextWaiting, "quem precisa de você");
            act(&mut v, Action::ToggleMaximize, "maximizar");
        }
        View::Sessions => {
            if app.cfg.on(aic_core::Module::Board) {
                v.push(("t".into(), "nova tarefa"));
            }
            v.push(("clique no terminal".into(), "digitar"));
            act(&mut v, Action::NewSession, "Claude");
            act(&mut v, Action::NextView, "aba");
            v.push(("Alt+1-9".into(), "bloco"));
        }
        View::Board => {
            for (keys, desc) in [("j/k", "mover"), ("h/l", "coluna"), ("Shift+H/L", "mover cartão"), ("Enter", "abrir"), ("e", "editar"), ("x", "concluir"), ("t", "nova"), ("/", "buscar")] {
                v.push((keys.into(), desc));
            }
            act(&mut v, Action::CloseFocused, "fechar");
            act(&mut v, Action::NextView, "aba");
        }
        View::Worktrees => {
            for (keys, desc) in [("j/k", "escolher"), ("Enter", "abrir sessão"), ("n", "novo"), ("Delete", "remover"), ("r", "atualizar")] {
                v.push((keys.into(), desc));
            }
            act(&mut v, Action::NextView, "aba");
        }
        View::Changes => {
            for (keys, desc) in [("j/k", "arquivo"), ("s", "stage"), ("u", "unstage"), ("r", "atualizar")] {
                v.push((keys.into(), desc));
            }
            act(&mut v, Action::NextView, "aba");
        }
        View::Settings => {
            v.push(("/".into(), "pesquisar"));
            v.push(("alterar".into(), "gravar atalho"));
            act(&mut v, Action::CloseFocused, "voltar às sessões");
            act(&mut v, Action::NextView, "aba");
        }
        View::Usage | View::Help => {
            act(&mut v, Action::CloseFocused, "voltar às sessões");
            act(&mut v, Action::NextView, "aba");
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
            if i == 0 && app.leader.is_none() && app.modal.is_none() && !app.prefix_pending {
                // a primeira dica é o catálogo: clicável
                if ui.add(egui::Button::new(text).frame(false)).on_hover_text("Abrir o catálogo de atalhos").clicked() {
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
