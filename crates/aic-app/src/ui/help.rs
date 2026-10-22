//! Aba Ajuda: o manual, os estados das sessões e os atalhos principais ao vivo.

use egui::{FontId, RichText, ScrollArea, Ui};
use egui_commonmark::CommonMarkViewer;
use aic_core::SessionState;

use super::widgets;
use crate::actions::Action;
use crate::app::App;
use crate::brand;

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    egui::Panel::right("help-side").resizable(false).exact_size(300.0).show(ui, |ui| {
        ui.label(RichText::new(format!("{}  ·  {}", brand::NAME, brand::TAGLINE)).font(FontId::monospace(12.0)).color(theme.accent_hi));
        ui.add_space(8.0);
        ui.label(RichText::new("ESTADOS").font(FontId::monospace(11.0)).color(theme.dim));
        for s in [SessionState::Boot, SessionState::Working, SessionState::Idle, SessionState::Waiting, SessionState::Done, SessionState::GameOver] {
            ui.horizontal(|ui| {
                widgets::state_badge(ui, brand::state_label(s), s, &theme);
                ui.label(widgets::subtle(state_desc(s), &theme).small());
            });
        }
        ui.add_space(8.0);
        ui.label(RichText::new(format!("PREFIXO  {}", super::prefix::label(app))).font(FontId::monospace(11.0)).color(theme.dim));
        for a in [
            Action::NewSession,
            Action::NewShell,
            Action::ToggleNotes,
            Action::CloseFocused,
            Action::NextView,
            Action::GotoWorkspace(1),
            Action::NextWaiting,
            Action::ToggleMaximize,
            Action::SearchSettings,
            Action::Catalog,
        ] {
            let keys = app.keymap.prefix_label(a);
            if keys.is_empty() {
                continue;
            }
            ui.horizontal(|ui| {
                widgets::kbd(ui, &keys, &theme);
                ui.label(RichText::new(a.meta().name).small());
            });
        }
        ui.add_space(8.0);
        ui.label(RichText::new("CATÁLOGO").font(FontId::monospace(11.0)).color(theme.dim));
        ui.horizontal(|ui| {
            widgets::kbd(ui, &super::leader::label(app), &theme);
            ui.label(RichText::new("todos os atalhos, agrupados").small());
        });
    });

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(780.0);
        CommonMarkViewer::new().show(ui, &mut app.md_cache, brand::HELP);
    });
}

fn state_desc(s: SessionState) -> &'static str {
    match s {
        SessionState::Boot => "abrindo",
        SessionState::Working => "Claude trabalhando",
        SessionState::Idle => "esperando você digitar",
        SessionState::Waiting => "pediu permissão / pergunta",
        SessionState::Done => "terminou, você não viu ainda",
        SessionState::GameOver => "processo morreu",
    }
}
