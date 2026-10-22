//! Notas da sessão em foco: um painel à direita com um editor de Markdown, uma nota por
//! sessão. `Ctrl+B e` abre (já escrevendo) e fecha; trocar de sessão troca a nota; `Esc`
//! devolve o teclado pro terminal. Cada mudança é salva na hora, junto da sessão.

use egui::{FontId, Key, RichText, TextEdit, Ui};

use super::widgets;
use crate::actions::Action;
use crate::app::App;

pub fn visible(app: &App) -> bool {
    app.notes_open
        && app.view == crate::app::View::Sessions
        && app.cfg.on(aic_core::Module::Notes)
        && app.focused().is_some()
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let Some(id) = app.focused() else { return };
    let Some(rt) = app.sessions.get(&id) else {
        return;
    };
    let name = rt.s.name.clone();
    let mut notes = rt.s.notes.clone();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("NOTAS")
                .font(FontId::monospace(11.0))
                .color(theme.dim),
        );
        ui.label(
            RichText::new(widgets::truncate(&name, 30))
                .font(FontId::monospace(11.0))
                .color(theme.accent_hi),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(
                ui,
                widgets::I_CLOSE,
                "Fechar notas",
                &app.keymap.tip(Action::ToggleNotes, "Fechar as notas"),
            )
            .clicked()
            {
                app.run_action(ui.ctx(), Action::ToggleNotes);
            }
        });
    });
    ui.add_space(2.0);
    let r = egui::ScrollArea::vertical()
        .id_salt(("notes-scroll", id))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_sized(
                ui.available_size(),
                TextEdit::multiline(&mut notes)
                    .id(egui::Id::new(("notes", id)))
                    .hint_text(
                        "Notas desta sessão (Markdown). Salvas sozinhas.\nEsc volta pro terminal.",
                    )
                    .font(FontId::monospace(12.5))
                    .frame(egui::Frame::NONE),
            )
        })
        .inner;
    if std::mem::take(&mut app.notes_focus) {
        r.request_focus();
        app.term_focus = false;
    }
    if r.gained_focus() {
        app.term_focus = false;
    }
    // o próprio editor solta o foco no Esc: por isso vale também no frame em que perdeu
    if (r.has_focus() || r.lost_focus()) && ui.input(|i| i.key_pressed(Key::Escape)) {
        r.surrender_focus();
        app.term_focus = true;
    }
    if r.changed() {
        app.set_notes(id, notes);
    }
}
