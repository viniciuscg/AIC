//! Aba do fork: como usar, o que esse fork muda, e os atalhos ao vivo.

use egui::{FontId, RichText, ScrollArea, Sense, Ui, Vec2};
use egui_commonmark::CommonMarkViewer;
use aic_core::SessionState;

use super::widgets;
use crate::actions::Action;
use crate::app::App;
use crate::skins;

pub fn show(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let t = app.t();
    let all = skins::all();

    // abas dos forks (cada fork tem a sua página)
    let current = ui.data(|d| d.get_temp::<String>(egui::Id::new("help-fork"))).unwrap_or_else(|| app.skin.id().to_string());
    ui.horizontal(|ui| {
        for f in &all {
            let th = f.theme(f.variants()[0]);
            let (r, resp) = ui.allocate_exact_size(Vec2::new(150.0, 40.0), Sense::click());
            let sel = current == f.id();
            ui.painter().rect_filled(r, theme.radius(), if sel { th.bg3 } else { theme.bg2 });
            ui.painter().rect_stroke(r, theme.radius(), egui::Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { th.accent_hi } else { theme.border }), egui::StrokeKind::Inside);
            let icon = egui::Rect::from_min_size(r.min + Vec2::new(4.0, 4.0), Vec2::splat(32.0));
            f.icon(ui, icon, t, &th);
            ui.painter().text(r.min + Vec2::new(42.0, 8.0), egui::Align2::LEFT_TOP, f.name(), FontId::monospace(13.0), th.accent_hi);
            if !f.creator().is_empty() {
                ui.painter().text(r.min + Vec2::new(42.0, 24.0), egui::Align2::LEFT_TOP, format!("por {}", f.creator()), FontId::proportional(10.5), th.dim);
            }
            if resp.clicked() {
                ui.data_mut(|d| d.insert_temp(egui::Id::new("help-fork"), f.id().to_string()));
            }
        }
    });
    ui.add_space(10.0);

    let fork = skins::by_id(&current);
    let th = fork.theme(fork.variants()[0]);
    egui::Panel::right("help-side").resizable(false).exact_size(300.0).show(ui, |ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(280.0, 200.0), Sense::hover());
        let state = [SessionState::Idle, SessionState::Working, SessionState::Waiting, SessionState::Done, SessionState::GameOver][((t / 3.0) as usize) % 5];
        ui.painter().rect_filled(r, th.radius(), th.bg2);
        fork.background(ui, r, t, &th);
        fork.mascot(ui, r.shrink(20.0), state, t, &th);
        ui.label(RichText::new(fork.state_label(state)).font(FontId::monospace(14.0)).color(th.state_color(state)));
        ui.add_space(8.0);
        ui.label(RichText::new("ESTADOS").font(FontId::monospace(11.0)).color(theme.dim));
        for s in [SessionState::Boot, SessionState::Working, SessionState::Idle, SessionState::Waiting, SessionState::Done, SessionState::GameOver] {
            ui.horizontal(|ui| {
                widgets::state_badge(ui, fork.state_label(s), s, &th, t);
                ui.label(widgets::subtle(state_desc(s), &theme).small());
            });
        }
        ui.add_space(8.0);
        ui.label(RichText::new("VARIANTES").font(FontId::monospace(11.0)).color(theme.dim));
        ui.horizontal_wrapped(|ui| {
            for v in fork.variants() {
                let vt = fork.theme(v);
                if widgets::chip(ui, *v, vt.accent_hi, &theme).clicked() {
                    let ctx = ui.ctx().clone();
                    app.set_theme(&ctx, fork.id().to_string(), Some(v.to_string()));
                }
            }
        });
        ui.add_space(8.0);
        if app.skin.id() != fork.id() && ui.button(format!("Usar o {} agora", fork.display_name())).clicked() {
            let ctx = ui.ctx().clone();
            app.set_theme(&ctx, fork.id().to_string(), None);
        }
        ui.add_space(8.0);
        ui.label(RichText::new("ATALHOS PRINCIPAIS").font(FontId::monospace(11.0)).color(theme.dim));
        for a in [Action::NewSession, Action::QuickAdd, Action::NextWaiting, Action::ViewBoard, Action::ToggleLayout, Action::Palette, Action::ViewSettings] {
            ui.horizontal(|ui| {
                widgets::kbd(ui, &app.keymap.label(a), &theme);
                ui.label(RichText::new(a.meta().name).small());
            });
        }
    });

    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(780.0);
        CommonMarkViewer::new().show(ui, &mut app.md_cache, fork.help());
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
