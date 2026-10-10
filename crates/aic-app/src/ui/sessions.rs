//! Aba Sessões: modo foco (lista lateral + uma sessão grande) e modo grade.

use egui::{Align, Color32, FontId, Layout as ELayout, Rect, RichText, Sense, Stroke, Ui, UiBuilder, Vec2};
use aic_core::{Id, Layout, SessionKind, SessionState, TaskStatus};
use aic_term::TerminalView;

use super::statusbar::truncate;
use super::widgets::{self, I_AGENTS, I_BRANCH, I_CLOSE, I_LINK, I_RESTART, I_TASK, I_WORKTREE};
use crate::actions::Action;
use crate::app::App;

pub fn show(app: &mut App, ui: &mut Ui) {
    let ids = app.ws_sessions();
    if ids.is_empty() && app.ws_blocks().is_empty() {
        return empty(app, ui);
    }
    let focused = app.focused();
    if app.maximized && focused.is_some() {
        if let Some(id) = focused {
            let rect = ui.available_rect_before_wrap();
            pane(app, ui, id, rect, true);
        }
        return;
    }
    match app.cfg.view {
        aic_core::SessionView::Blocks => return super::blocks::list(app, ui),
        aic_core::SessionView::Tabs => return super::blocks::tabs(app, ui),
        _ => {}
    }
    if ids.is_empty() {
        return empty(app, ui);
    }
    let layout = if app.cfg.view == aic_core::SessionView::Grid { Layout::Grid } else { Layout::Focus };

    if layout == Layout::Grid {
        grid(app, ui, &ids);
    } else {
        if app.sidebar {
            egui::Panel::left("sidebar")
                .resizable(true)
                .default_size(290.0)
                .size_range(220.0..=480.0)
                .frame(egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 8, top: 0, bottom: 0 }))
                .show(ui, |ui| sidebar(app, ui, &ids));
        }
        if let Some(id) = focused {
            let rect = ui.available_rect_before_wrap();
            pane(app, ui, id, rect, true);
        }
    }
}

pub fn empty(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    let t = app.t();
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.12);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(180.0), Sense::hover());
        app.skin.mascot(ui, rect, SessionState::Idle, t, &theme);
        ui.add_space(8.0);
        ui.label(RichText::new(app.skin.display_name()).font(FontId::monospace(26.0)).color(theme.accent_hi));
        ui.label(widgets::subtle(app.skin.tagline(), &theme));
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            let w = 3.0 * 210.0;
            ui.add_space((ui.available_width() - w).max(0.0) / 2.0);
            if ui.add_sized([200.0, 34.0], egui::Button::new(format!("{}  Nova sessão Claude", widgets::I_PLUS))).clicked() {
                app.run_action(ui.ctx(), Action::NewSession);
            }
            if ui.add_sized([200.0, 34.0], egui::Button::new(format!("{}  Nova tarefa", widgets::I_TASK))).clicked() {
                app.open_quick_add(None);
            }
            if ui.add_sized([200.0, 34.0], egui::Button::new(format!("{}  Board", widgets::I_BOARD))).clicked() {
                app.set_view(crate::app::View::Board);
            }
        });
        ui.add_space(8.0);
        ui.label(widgets::subtle(
            format!(
                "{} nova sessão · {} nova tarefa · {} paleta · F1 ajuda",
                app.keymap.label(Action::NewSession),
                app.keymap.label(Action::QuickAdd),
                app.keymap.label(Action::Palette)
            ),
            &theme,
        ));
        now_list(app, ui, 520.0);
    });
}

/// "Agora": o que está em andamento, pra responder "o que eu tô fazendo?".
fn now_list(app: &mut App, ui: &mut Ui, width: f32) {
    let theme = app.theme.clone();
    let mut doing: Vec<_> = app.tasks.iter().filter(|t| t.status == TaskStatus::Doing).cloned().collect();
    if doing.is_empty() {
        return;
    }
    doing.sort_by(|a, b| b.priority.cmp(&a.priority));
    ui.add_space(14.0);
    ui.allocate_ui(Vec2::new(width, 0.0), |ui| {
        ui.label(RichText::new("AGORA").font(FontId::monospace(11.0)).color(theme.dim));
        for t in doing.iter().take(6) {
            ui.horizontal(|ui| {
                let state = t.session.filter(|s| app.sessions.contains_key(s)).map(|s| app.state_of(s));
                match state {
                    Some(s) => {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                        widgets::state_dot(ui, r.center(), 4.0, s, &theme, app.t());
                    }
                    None => {
                        ui.label(RichText::new("○").color(theme.faint));
                    }
                }
                let label = match &t.card_key {
                    Some(k) => format!("{k} · {}", t.title),
                    None => t.title.clone(),
                };
                if ui.link(RichText::new(truncate(&label, 60)).color(theme.ink)).clicked() {
                    app.open_task(t.id);
                }
            });
        }
    });
}

fn sidebar(app: &mut App, ui: &mut Ui, ids: &[Id]) {
    let theme = app.theme.clone();
    let t = app.t();
    let focused = app.focused();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("SESSÕES · {}", ids.len())).font(FontId::monospace(11.0)).color(theme.dim));
            ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
                if ui.small_button(widgets::I_PLUS).on_hover_text(app.keymap.label(Action::NewSession)).clicked() {
                    app.run_action(ui.ctx(), Action::NewSession);
                }
                if ui.small_button("▦").on_hover_text(format!("Grade ({})", app.keymap.label(Action::ToggleLayout))).clicked() {
                    app.run_action(ui.ctx(), Action::ToggleLayout);
                }
            });
        });
        ui.add_space(4.0);
        for (i, id) in ids.iter().enumerate() {
            let Some(rt) = app.sessions.get(id) else { continue };
            let state = rt.state();
            let active = Some(*id) == focused;
            let color = theme.agent(rt.s.color);
            let name = rt.s.name.clone();
            let dir = rt.display_dir();
            let branch = rt.git.as_ref().and_then(|g| g.branch.clone().map(|b| (b, g.dirty)));
            let preview = rt.term.as_ref().map(|t| t.tail_lines(2)).unwrap_or_default();
            let agents = rt.live.agents.len();
            let unseen = rt.unseen;
            let has_wt = rt.s.worktree.is_some();
            let task = app.task_of(*id).map(|t| t.card_key.clone().unwrap_or_else(|| truncate(&t.title, 26)));

            let resp = widgets::card(ui, &theme, active, color, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                    if app.cfg.on(aic_core::Module::Mascot) {
                        app.skin.mascot(ui, r, state, t + i as f64 * 0.7, &theme);
                    } else {
                        widgets::state_dot(ui, r.center(), 6.0, state, &theme, t);
                    }
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{}", i + 1)).font(FontId::monospace(10.0)).color(theme.faint));
                            ui.label(RichText::new(truncate(&name, 22)).strong().color(theme.ink));
                            if unseen {
                                ui.label(RichText::new("●").color(theme.warn));
                            }
                        });
                        ui.horizontal(|ui| {
                            widgets::state_badge(ui, app.skin.state_label(state), state, &theme, t);
                            if agents > 0 {
                                ui.label(RichText::new(format!("{I_AGENTS} {agents}")).small().color(theme.attention));
                            }
                        });
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(RichText::new(dir).small().color(theme.dim));
                    if let Some((b, dirty)) = &branch {
                        let mut s = format!("{I_BRANCH} {}", truncate(b, 22));
                        if *dirty > 0 {
                            s.push_str(&format!(" ±{dirty}"));
                        }
                        ui.label(RichText::new(s).small().color(theme.dim));
                    }
                    if has_wt {
                        ui.label(RichText::new(I_WORKTREE).small().color(theme.accent));
                    }
                });
                if let Some(task) = &task {
                    ui.label(RichText::new(format!("{I_TASK} {task}")).small().color(theme.accent_hi));
                }
                for line in &preview {
                    ui.label(RichText::new(truncate(line.trim(), 40)).font(FontId::monospace(10.5)).color(theme.faint));
                }
            });
            if resp.clicked() {
                app.focus(*id);
            }
            session_menu(app, &resp, *id);
            ui.add_space(6.0);
        }
        now_list(app, ui, ui.available_width());
    });
}

fn grid(app: &mut App, ui: &mut Ui, ids: &[Id]) {
    let n = ids.len();
    let cols = (n as f32).sqrt().ceil() as usize;
    let rows = n.div_ceil(cols);
    let area = ui.available_rect_before_wrap();
    let gap = 6.0;
    let w = (area.width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let h = (area.height() - gap * (rows as f32 - 1.0)) / rows as f32;
    for (i, id) in ids.iter().enumerate() {
        let (r, c) = (i / cols, i % cols);
        let rect = Rect::from_min_size(area.min + Vec2::new(c as f32 * (w + gap), r as f32 * (h + gap)), Vec2::new(w, h));
        pane(app, ui, *id, rect, true);
    }
    ui.allocate_rect(area, Sense::hover());
}

/// Uma sessão: cabeçalho + terminal dentro de `rect`.
pub fn pane(app: &mut App, ui: &mut Ui, id: Id, rect: Rect, header: bool) {
    let theme = app.theme.clone();
    let t = app.t();
    let is_focused = app.focused() == Some(id);
    let Some(rt) = app.sessions.get(&id) else { return };
    let state = rt.state();
    let color = theme.agent(rt.s.color);
    let border = if is_focused { color } else { theme.border };

    // moldura (sombra dura nos forks retrô)
    if let Some(o) = theme.hard_shadow {
        ui.painter().rect_filled(rect.translate(Vec2::splat(o)), theme.radius(), Color32::from_black_alpha(220));
    }
    ui.painter().rect_filled(rect, theme.radius(), theme.term.background);
    let glow = if state.needs_attention() { theme.state_color(state).gamma_multiply(0.5 + 0.5 * ((t * 3.0).sin() as f32 * 0.5 + 0.5)) } else { border };
    ui.painter().rect_stroke(rect, theme.radius(), Stroke::new(if is_focused || state.needs_attention() { 2.0 } else { 1.0 }, glow), egui::StrokeKind::Inside);

    let mut inner = rect.shrink(2.0);
    if header {
        let hrect = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 30.0));
        inner.min.y += 30.0;
        ui.painter().rect_filled(hrect, egui::CornerRadius::ZERO, theme.bg3);
        // O fundo clicável do cabeçalho é registrado ANTES dos botões: no egui quem é
        // registrado por último ganha o clique, então assim os botões continuam funcionando.
        let resp = ui.interact(hrect, egui::Id::new(("pane-head", id)), Sense::click());
        let mut child = ui.new_child(UiBuilder::new().max_rect(hrect.shrink2(Vec2::new(6.0, 3.0))).layout(ELayout::left_to_right(Align::Center)));
        header_row(app, &mut child, id, state, color, t);
        if resp.clicked() {
            app.focus(id);
        }
        session_menu(app, &resp, id);
    }

    let term_focus = app.term_focus && is_focused && app.modal.is_none();
    let Some(rt) = app.sessions.get_mut(&id) else { return };
    if let Some(err) = rt.spawn_error.clone() {
        let mut child = ui.new_child(UiBuilder::new().max_rect(inner.shrink(16.0)));
        child.label(RichText::new(format!("Não deu pra iniciar a sessão:\n{err}")).color(theme.err));
        if child.button(format!("{I_RESTART} Tentar de novo")).clicked() {
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

    if exited {
        let over = Rect::from_center_size(inner.center(), Vec2::new(340.0, 150.0).min(inner.size()));
        ui.painter().rect_filled(over, theme.radius(), theme.bg2.gamma_multiply(0.95));
        ui.painter().rect_stroke(over, theme.radius(), Stroke::new(1.0, theme.err), egui::StrokeKind::Inside);
        let mascot = Rect::from_min_size(over.min + Vec2::new(10.0, 10.0), Vec2::splat(64.0));
        app.skin.mascot(ui, mascot, SessionState::GameOver, t, &theme);
        let mut c = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_max(over.min + Vec2::new(84.0, 16.0), over.max - Vec2::new(10.0, 10.0))));
        c.label(RichText::new(app.skin.state_label(SessionState::GameOver)).font(FontId::monospace(18.0)).color(theme.err));
        c.label(widgets::subtle("O processo terminou.", &theme));
        c.horizontal(|ui| {
            if ui.button(format!("{I_RESTART} Reabrir (Enter)")).clicked() || (is_focused && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                let ctx = ui.ctx().clone();
                app.restart_session(&ctx, id);
            }
            if ui.button(format!("{I_CLOSE} Fechar")).clicked() {
                app.kill_session(id, false);
            }
        });
    }
}

fn header_row(app: &mut App, ui: &mut Ui, id: Id, state: SessionState, color: Color32, t: f64) {
    let theme = app.theme.clone();
    let Some(rt) = app.sessions.get(&id) else { return };
    let name = rt.s.name.clone();
    let kind = rt.s.kind.clone();
    let agents = rt.live.agents.len();
    let branch = rt.git.as_ref().and_then(|g| g.branch.clone());
    let task = app.task_of(id).cloned();

    let (r, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
    if app.cfg.on(aic_core::Module::Mascot) {
        app.skin.mascot(ui, r, state, t, &theme);
    } else {
        widgets::state_dot(ui, r.center(), 5.0, state, &theme, t);
    }
    ui.label(RichText::new(&name).strong().color(color));
    widgets::state_badge(ui, app.skin.state_label(state), state, &theme, t);
    if agents > 0 {
        ui.label(RichText::new(format!("{I_AGENTS} {agents}")).color(theme.attention));
    }
    if let Some(b) = branch {
        ui.label(RichText::new(format!("{I_BRANCH} {}", truncate(&b, 28))).small().color(theme.dim));
    }
    if let Some(t) = &task {
        let label = t.card_key.clone().unwrap_or_else(|| truncate(&t.title, 30));
        if widgets::chip(ui, format!("{I_TASK} {label}"), theme.accent_hi, &theme).on_hover_text(&t.title).clicked() {
            app.board.selected = Some(t.id);
            app.set_view(crate::app::View::Board);
        }
    }
    ui.with_layout(ELayout::right_to_left(Align::Center), |ui| {
        if ui.small_button(I_CLOSE).on_hover_text(format!("Fechar ({})", app.keymap.label(Action::CloseSession))).clicked() {
            app.focus(id);
            app.run_action(ui.ctx(), Action::CloseSession);
        }
        if ui.small_button(I_RESTART).on_hover_text("Reiniciar (retoma a conversa)").clicked() {
            let ctx = ui.ctx().clone();
            app.restart_session(&ctx, id);
        }
        if matches!(kind, SessionKind::Claude) {
            if task.is_some() {
                if ui.small_button("󰈙").on_hover_text("Mandar contexto da tarefa pro Claude (.aic/context.md)").clicked() {
                    app.write_context(id);
                }
            }
            if ui.small_button(I_LINK).on_hover_text(format!("Vincular tarefa ({})", app.keymap.label(Action::LinkTaskToSession))).clicked() {
                app.focus(id);
                app.run_action(ui.ctx(), Action::LinkTaskToSession);
            }
        }
        if ui.small_button("󰊓").on_hover_text(format!("Maximizar ({})", app.keymap.label(Action::ToggleMaximize))).clicked() {
            app.focus(id);
            app.maximized = !app.maximized;
        }
    });
}

pub fn session_menu(app: &mut App, resp: &egui::Response, id: Id) {
    resp.context_menu(|ui| {
        if ui.button("Renomear").clicked() {
            app.modal = Some(super::modals::Modal::Rename { id, text: app.session_label(id), first: true });
            ui.close();
        }
        if ui.button("Vincular tarefa").clicked() {
            app.modal = Some(super::modals::Modal::LinkTask { session: id, query: String::new(), selected: 0 });
            ui.close();
        }
        if ui.button("Nova tarefa vinculada").clicked() {
            app.focus(id);
            app.open_quick_add(None);
            if let Some(super::modals::Modal::QuickAdd { link_enabled, .. }) = &mut app.modal {
                *link_enabled = true;
            }
            ui.close();
        }
        if ui.button("Mandar contexto da tarefa").clicked() {
            app.write_context(id);
            ui.close();
        }
        if ui.button("Reiniciar").clicked() {
            let ctx = ui.ctx().clone();
            app.restart_session(&ctx, id);
            ui.close();
        }
        ui.separator();
        if ui.button(RichText::new("Fechar").color(app.theme.err)).clicked() {
            app.focus(id);
            app.run_action(ui.ctx(), Action::CloseSession);
            ui.close();
        }
    });
}
