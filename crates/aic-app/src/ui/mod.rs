//! Desenho. `draw` é chamado uma vez por frame com o `Ui` raiz.

pub mod blocks;
pub mod changes;
pub mod help;
pub mod hints;
pub mod leader;
pub mod modals;
pub mod prefix;
pub mod sessions;
pub mod settings;
pub mod setup;
pub mod statusbar;
pub mod topbar;
pub mod usage;
pub mod widgets;
pub mod worktrees;

use std::time::Duration;

use egui::{Align2, Frame, Key, Margin, RichText, Ui, Vec2};

use crate::app::{App, Phase, ToastKind, View};

/// Redesenho de fundo, sem nada acontecendo: relógio, estados dos hooks, avisos. Saída de
/// terminal e input redesenham na hora (a saída limitada a ~30 fps, ver `aic_term::backend`).
pub const IDLE_REPAINT: Duration = Duration::from_millis(500);

pub fn draw(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();

    if ctx.input(|i| i.viewport().close_requested()) && !app.quitting {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        app.modal = Some(modals::Modal::ConfirmQuit);
    }

    app.tick(&ctx);

    if matches!(app.phase, Phase::Setup(_)) {
        setup::show(app, ui);
        ctx.request_repaint_after(IDLE_REPAINT);
        return;
    }

    // aba de um módulo que foi desligado: volta pras sessões
    let view_module = match app.view {
        View::Worktrees => Some(aic_core::Module::Worktrees),
        View::Changes => Some(aic_core::Module::Changes),
        View::Usage => Some(aic_core::Module::Usage),
        _ => None,
    };
    if view_module.is_some_and(|m| !app.cfg.on(m)) {
        app.set_view(View::Sessions);
    }

    // Gravando um atalho na Config: o teclado é da captura (senão a combinação disparava a ação).
    let capturing = app.view == View::Settings && app.settings.is_capturing();
    if app.modal.is_some() || capturing {
        app.leader = None;
        app.prefix_pending = false;
    } else if !prefix::handle(app, &ctx) && !leader::handle(app, &ctx) {
        for a in app.keymap.pressed(&ctx) {
            app.run_action(&ctx, a);
        }
        let typing = ctx.egui_wants_keyboard_input();
        let free = !(app.view == View::Sessions && app.term_focus) && !app.layne.focus && !typing && app.modal.is_none();
        if free && app.cfg.on(aic_core::Module::Tasks) && ctx.input_mut(|i| crate::actions::consume_exact(i, egui::Modifiers::NONE, Key::T)) {
            app.open_quick_add(None);
        }
    }

    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, app.theme.bg);
    if app.cfg.animations && app.cfg.on(aic_core::Module::Background) {
        crate::brand::background(ui, rect, app.t(), &app.theme);
    }

    egui::Panel::top("topbar")
        .frame(Frame::new().fill(app.theme.bg2).inner_margin(Margin::symmetric(6, 4)).stroke(egui::Stroke::new(1.0, app.theme.border)))
        .show(ui, |ui| topbar::show(app, ui));

    if hints::visible(app) {
        egui::Panel::bottom("hints")
            .frame(Frame::new().fill(app.theme.bg).inner_margin(Margin::symmetric(12, 3)))
            .show(ui, |ui| hints::show(app, ui));
    }

    egui::Panel::bottom("statusbar")
        .frame(Frame::new().fill(app.theme.bg2).inner_margin(Margin::symmetric(12, 5)).stroke(egui::Stroke::new(1.0, app.theme.border)))
        .show(ui, |ui| statusbar::show(app, ui));

    if app.layne.open && app.cfg.on(aic_core::Module::Layne) {
        let resp = egui::Panel::right("layne")
            .resizable(true)
            .default_size(app.cfg.layne.width)
            .size_range(320.0..=1100.0)
            .frame(crate::layne::frame(app))
            .show(ui, |ui| crate::layne::panel(app, ui));
        let w = resp.response.rect.width();
        if (w - app.cfg.layne.width).abs() > 4.0 && !ctx.input(|i| i.pointer.any_down()) {
            app.cfg.layne.width = w;
            let _ = app.cfg.save();
        }
    }

    egui::CentralPanel::default()
        .frame(Frame::new().inner_margin(Margin::same(if app.view == View::Sessions { 6 } else { 16 })))
        .show(ui, |ui| match app.view {
            View::Sessions => sessions::show(app, ui),
            View::Worktrees => worktrees::show(app, ui),
            View::Changes => changes::show(app, ui),
            View::Usage => {
                app.usage_history.ensure_started(ui.ctx());
                usage::show(app, ui)
            }
            View::Settings => settings::show(app, ui),
            View::Help => help::show(app, ui),
        });

    modals::show(app, &ctx);
    leader::show(app, &ctx);
    toasts(app, &ctx);

    ctx.request_repaint_after(IDLE_REPAINT);
}

fn toasts(app: &mut App, ctx: &egui::Context) {
    if app.toasts.is_empty() {
        return;
    }
    let mut dismiss = None;
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -64.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            for (i, t) in app.toasts.iter().enumerate() {
                let color = match t.kind {
                    ToastKind::Info => app.theme.accent,
                    ToastKind::Ok => app.theme.ok,
                    ToastKind::Warn => app.theme.warn,
                    ToastKind::Err => app.theme.err,
                };
                // clique em qualquer lugar do aviso fecha (o sentido vem antes do conteúdo)
                let resp = ui
                    .scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| {
                        Frame::new()
                            .fill(app.theme.bg3)
                            .stroke(egui::Stroke::new(1.0, color))
                            .corner_radius(app.theme.radius())
                            .inner_margin(Margin::symmetric(12, 8))
                            .shadow(ctx.global_style().visuals.popup_shadow)
                            .show(ui, |ui| {
                                ui.set_max_width(420.0);
                                ui.horizontal(|ui| {
                                    ui.add(egui::Label::new(RichText::new(&t.text).color(app.theme.ink)).selectable(false));
                                    if widgets::icon_button(ui, widgets::I_CLOSE, "Fechar aviso", "Fechar aviso").clicked() {
                                        dismiss = Some(i);
                                    }
                                });
                            });
                    })
                    .response;
                if resp.on_hover_text("Clique pra fechar").clicked() {
                    dismiss = Some(i);
                }
                ui.add_space(6.0);
            }
        });
    if let Some(i) = dismiss {
        app.toasts.remove(i);
    }
}
