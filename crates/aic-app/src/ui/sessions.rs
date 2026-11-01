//! A tela dos terminais: os terminais da sessão atual, de ponta a ponta, sem cabeçalho nenhum (o
//! que importa sobre o terminal em foco está na barra). Só uma borda fina diz quem está em foco.

use aic_core::{Id, SessionState, Split};
use aic_term::TerminalView;
use egui::{FontId, Pos2, Rect, RichText, Sense, Stroke, Ui, UiBuilder, Vec2};

use super::widgets::{self, I_CLOSE, I_PLUS, I_RESTART, I_TERM};
use crate::actions::Action;
use crate::app::App;

const GAP: f32 = 2.0;

pub fn show(app: &mut App, ui: &mut Ui) {
    let Some(block) = app.current_block().and_then(|b| app.blocks.get(&b)) else {
        return empty(app, ui);
    };
    let (panes, split, id) = (block.panes.clone(), block.split, block.id);
    let area = ui.available_rect_before_wrap();
    if panes.is_empty() {
        return empty_block(app, ui, id, area);
    }
    let focused = app.focused();
    if app.maximized
        && let Some(f) = focused.filter(|f| panes.contains(f))
    {
        pane(app, ui, f, area);
    } else {
        let rects = split_rects(area, panes.len(), split, GAP);
        for (p, r) in panes.iter().zip(rects) {
            pane(app, ui, *p, r);
        }
    }
    ui.allocate_rect(area, Sense::hover());
}

/// Retângulos dos terminais conforme o arranjo.
pub fn split_rects(area: Rect, n: usize, split: Split, gap: f32) -> Vec<Rect> {
    if n == 0 {
        return vec![];
    }
    if n == 1 {
        return vec![area];
    }
    let mut out = Vec::with_capacity(n);
    match split {
        Split::Row => {
            let w = (area.width() - gap * (n - 1) as f32) / n as f32;
            for i in 0..n {
                out.push(Rect::from_min_size(
                    area.min + Vec2::new(i as f32 * (w + gap), 0.0),
                    Vec2::new(w, area.height()),
                ));
            }
        }
        Split::Column => {
            let h = (area.height() - gap * (n - 1) as f32) / n as f32;
            for i in 0..n {
                out.push(Rect::from_min_size(
                    area.min + Vec2::new(0.0, i as f32 * (h + gap)),
                    Vec2::new(area.width(), h),
                ));
            }
        }
        Split::Main => {
            let w = (area.width() - gap) * 0.6;
            out.push(Rect::from_min_size(area.min, Vec2::new(w, area.height())));
            let rest = Rect::from_min_max(Pos2::new(area.left() + w + gap, area.top()), area.max);
            out.extend(split_rects(rest, n - 1, Split::Column, gap));
        }
        Split::Grid => {
            let cols = (n as f32).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            let w = (area.width() - gap * (cols - 1) as f32) / cols as f32;
            let h = (area.height() - gap * (rows - 1) as f32) / rows as f32;
            for i in 0..n {
                let (r, c) = (i / cols, i % cols);
                out.push(Rect::from_min_size(
                    area.min + Vec2::new(c as f32 * (w + gap), r as f32 * (h + gap)),
                    Vec2::new(w, h),
                ));
            }
        }
    }
    out
}

pub fn move_block(app: &mut App, id: Id, delta: i32) {
    if let Some(w) = app.current_ws_mut()
        && let Some(i) = w.blocks.iter().position(|b| *b == id)
    {
        let j = (i as i32 + delta).clamp(0, w.blocks.len() as i32 - 1) as usize;
        w.blocks.swap(i, j);
    }
    app.persist_ws();
}

/// Sem nenhuma sessão: só o sabre e as teclas.
pub fn empty(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        ui.label(
            RichText::new(crate::brand::TAGLINE)
                .font(FontId::monospace(16.0))
                .color(theme.accent_hi),
        );
        ui.add_space(12.0);
        if ui
            .add_sized(
                [220.0, 30.0],
                egui::Button::new(format!("{I_PLUS}  Nova sessão Claude")),
            )
            .on_hover_text(app.keymap.tip(Action::NewBlock, "Nova sessão"))
            .clicked()
        {
            app.run_action(ui.ctx(), Action::NewBlock);
        }
        ui.add_space(8.0);
        ui.label(widgets::subtle(
            format!(
                "{} nova sessão · {} tarefas · {} atalhos · F1 manual",
                super::prefix::chord(app, Action::NewBlock),
                super::prefix::chord(app, Action::ViewTasks),
                super::prefix::chord(app, Action::Catalog),
            ),
            &theme,
        ));
    });
}

fn empty_block(app: &mut App, ui: &mut Ui, id: Id, r: Rect) {
    let theme = app.theme.clone();
    let mut c = ui.new_child(
        UiBuilder::new()
            .max_rect(r.shrink(20.0))
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    c.add_space(r.height() * 0.3);
    c.label(widgets::subtle("Sessão sem terminais.", &theme));
    c.add_space(6.0);
    c.horizontal(|ui| {
        ui.add_space((ui.available_width() - 300.0).max(0.0) / 2.0);
        if ui
            .button(format!("{I_PLUS} Claude"))
            .on_hover_text(
                app.keymap
                    .tip(Action::NewSession, "Novo Claude nesta sessão"),
            )
            .clicked()
        {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::NewSession);
        }
        if ui
            .button(format!("{I_TERM} Terminal"))
            .on_hover_text(
                app.keymap
                    .tip(Action::NewShell, "Novo terminal nesta sessão"),
            )
            .clicked()
        {
            app.focus_block(id);
            app.run_action(ui.ctx(), Action::NewShell);
        }
        if ui
            .button(format!("{I_CLOSE} Fechar"))
            .on_hover_text(app.keymap.tip(Action::CloseBlock, "Fechar a sessão"))
            .clicked()
        {
            app.kill_block(id, false);
        }
    });
    ui.allocate_rect(r, Sense::hover());
}

/// Um terminal dentro de `rect`.
pub fn pane(app: &mut App, ui: &mut Ui, id: Id, rect: Rect) {
    let theme = app.theme.clone();
    let is_focused = app.focused() == Some(id);
    let Some(rt) = app.sessions.get(&id) else {
        return;
    };
    let state = rt.state();
    let many = app
        .block_of(id)
        .and_then(|b| app.blocks.get(&b))
        .is_some_and(|b| b.panes.len() > 1);
    let inner = if many && !app.maximized {
        rect.shrink(1.0)
    } else {
        rect
    };

    let term_focus = app.term_focus && is_focused && app.modal.is_none() && !app.anakin.focus;
    let Some(rt) = app.sessions.get_mut(&id) else {
        return;
    };
    if let Some(err) = rt.spawn_error.clone() {
        let mut child = ui.new_child(UiBuilder::new().max_rect(inner.shrink(16.0)));
        child
            .label(RichText::new(format!("Não deu pra iniciar a sessão:\n{err}")).color(theme.err));
        if child
            .button(format!("{I_RESTART} Tentar de novo"))
            .clicked()
        {
            let ctx = child.ctx().clone();
            app.restart_session(&ctx, id);
        }
        return;
    }
    let Some(term) = rt.term.as_mut() else { return };
    let exited = term.is_exited();
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner));
    let resp = TerminalView::new(term, &app.theme.term, &app.font, &app.term_bindings)
        .focused(term_focus && !exited)
        .show(&mut child, inner.size());
    if resp.clicked() || resp.drag_started() {
        app.focus(id);
        app.term_focus = true;
    }
    session_menu(app, &resp, id);

    // moldura por cima do terminal, só havendo mais de um
    if many && !app.maximized {
        let color = if state.needs_attention() && !is_focused {
            theme.state_color(state)
        } else if is_focused {
            theme.accent
        } else {
            theme.border
        };
        ui.painter()
            .rect_stroke(rect, 0.0, Stroke::new(1.0, color), egui::StrokeKind::Inside);
    }

    if exited {
        exited_overlay(app, ui, id, inner, is_focused);
    }
}

fn exited_overlay(app: &mut App, ui: &mut Ui, id: Id, inner: Rect, is_focused: bool) {
    let theme = app.theme.clone();
    let over = Rect::from_center_size(inner.center(), Vec2::new(340.0, 110.0).min(inner.size()));
    ui.painter().rect_filled(over, 0.0, theme.bg2);
    ui.painter().rect_stroke(
        over,
        0.0,
        Stroke::new(1.0, theme.err),
        egui::StrokeKind::Inside,
    );
    let mut c = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_max(
        over.min + Vec2::new(14.0, 12.0),
        over.max - Vec2::new(10.0, 8.0),
    )));
    c.label(
        RichText::new(crate::brand::state_label(SessionState::GameOver))
            .font(FontId::monospace(14.0))
            .color(theme.err),
    );
    c.label(widgets::subtle("O processo terminou.", &theme));
    c.horizontal(|ui| {
        if ui
            .button(format!("{I_RESTART} Reabrir (Enter)"))
            .on_hover_text(app.keymap.tip(Action::RestartSession, "Reabrir o processo"))
            .clicked()
            || (is_focused && app.modal.is_none() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
        {
            let ctx = ui.ctx().clone();
            app.restart_session(&ctx, id);
        }
        if ui
            .button(format!("{I_CLOSE} Fechar"))
            .on_hover_text(app.keymap.tip(Action::CloseFocused, "Fechar o terminal"))
            .clicked()
        {
            app.kill_session(id, false);
        }
    });
}

/// Clique direito no terminal: cada item diz a tecla.
pub fn session_menu(app: &mut App, resp: &egui::Response, id: Id) {
    resp.context_menu(|ui| {
        let ctx = ui.ctx().clone();
        for (label, a) in [
            ("Novo terminal ao lado", Action::NewShell),
            ("Novo Claude ao lado", Action::NewSession),
            ("Maximizar / restaurar", Action::ToggleMaximize),
            ("Renomear", Action::RenameSession),
            ("Reiniciar", Action::RestartSession),
            ("Fechar este terminal", Action::CloseSession),
        ] {
            let keys = app.keymap.keys_of(a);
            if ui.button(format!("{label}  ({keys})")).clicked() {
                app.focus(id);
                app.run_action(&ctx, a);
                ui.close();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arranjos_cobrem_a_area() {
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 600.0));
        for split in Split::ALL {
            for n in 1..=5 {
                let rs = split_rects(area, n, split, GAP);
                assert_eq!(rs.len(), n);
                for r in &rs {
                    assert!(area.expand(0.5).contains_rect(*r), "{split:?} {n}");
                    assert!(r.width() > 50.0 && r.height() > 50.0);
                }
            }
        }
    }
}
