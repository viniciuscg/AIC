//! Barra superior no espírito do i3bar, uma linha só:
//!
//! ```text
//!  ▣ AIC │ 1 Principal │ 2 api ●2 │ 3 front │ +   PREFIXO   s Sessões  g Worktrees …      ● 2 aguardando │ 🔥 1 │ $3.20 │ 5h ███░ 40% │ sem ░░ 5% │ 14:32
//! ```
//!
//! - **workspaces** à esquerda e em destaque: o atual aceso na cor de destaque, os que têm
//!   alguém precisando de você em roxo (o "urgent" do i3). O número é o de `Ctrl+B N`;
//! - **PREFIXO** acende enquanto o `Ctrl+B` espera a próxima tecla (o modo do i3);
//! - **abas** com a tecla de prefixo de cada uma;
//! - **status** à direita, em blocos separados como no i3status.

use egui::text::LayoutJob;
use egui::{Color32, CornerRadius, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};
use aic_core::SessionState;

use super::widgets::{self, I_5H, I_WEEK, bar_job, push};
use crate::actions::Action;
use crate::app::{App, View};
use crate::theme::Theme;

const H: f32 = 24.0;

/// Bloco retangular clicável com texto (workspace, aba, modo).
/// `label` é o nome acessível (leitor de tela e testes).
fn block(ui: &mut Ui, label: &str, job: LayoutJob, fill: Color32, stroke: Stroke, underline: Option<Color32>) -> egui::Response {
    let galley = ui.painter().layout_job(job);
    let size = Vec2::new(galley.size().x + 16.0, H);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let hovered = resp.hovered();
    let p = ui.painter();
    let fill = if hovered && fill == Color32::TRANSPARENT { ui.visuals().widgets.hovered.weak_bg_fill } else { fill };
    p.rect(rect, CornerRadius::same(2), fill, stroke, egui::StrokeKind::Inside);
    if let Some(c) = underline {
        p.rect_filled(Rect::from_min_max(egui::pos2(rect.left() + 2.0, rect.bottom() - 2.0), egui::pos2(rect.right() - 2.0, rect.bottom())), 0.0, c);
    }
    let pos = rect.center() - galley.size() / 2.0;
    p.galley(pos, galley, Color32::WHITE);
    resp
}

fn sep(ui: &mut Ui, theme: &Theme) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(9.0, H), Sense::hover());
    ui.painter().vline(r.center().x, r.y_range().shrink(5.0), Stroke::new(1.0, theme.border));
}

pub fn show(app: &mut App, ui: &mut Ui) {
    let t = app.t();
    let theme = app.theme.clone();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.set_min_height(H);

        // ── marca: mascote reflete o "humor geral" ──
        let states: Vec<SessionState> = app.sessions.values().map(|r| r.state()).collect();
        let mood = if states.contains(&SessionState::Waiting) {
            SessionState::Waiting
        } else if states.contains(&SessionState::Working) {
            SessionState::Working
        } else if app.sessions.values().any(|r| r.unseen) {
            SessionState::Done
        } else {
            SessionState::Idle
        };
        if app.cfg.on(aic_core::Module::Mascot) {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(H), Sense::click());
            crate::brand::mascot(ui, rect, mood, t, &theme);
            if resp.on_hover_text(app.keymap.tip(Action::ViewHelp, "Manual")).clicked() {
                app.set_view(View::Help);
            }
        }
        let pos = ui.cursor().min + Vec2::new(3.0, 0.0);
        let wm = crate::brand::wordmark(ui, pos, 15.0, &theme);
        ui.allocate_rect(Rect::from_min_size(wm.min, wm.size() + Vec2::new(6.0, 0.0)), Sense::hover());
        sep(ui, &theme);

        // ── workspaces ──
        if app.cfg.on(aic_core::Module::Workspaces) {
            workspaces(app, ui, &theme, t);
        } else if let Some(w) = app.current_ws() {
            let mut job = LayoutJob::default();
            push(&mut job, &w.name, theme.dim, 12.5);
            block(ui, &format!("Workspace {}", w.name), job, Color32::TRANSPARENT, Stroke::NONE, None);
        }

        // ── modo: prefixo armado ──
        if app.prefix_pending {
            ui.add_space(4.0);
            let a = 0.75 + 0.25 * ((t * 6.0).sin() as f32 * 0.5 + 0.5);
            let mut job = LayoutJob::default();
            push(&mut job, &format!("{} {}", widgets::I_KEY, super::prefix::label(app)), theme.bg, 12.5);
            block(ui, "Prefixo armado", job, theme.warn.gamma_multiply(a), Stroke::NONE, None)
                .on_hover_text("Prefixo armado: aperte a tecla da ação (Esc cancela, ? mostra todas)");
        }
        sep(ui, &theme);

        // ── abas ──
        for view in app.tabs() {
            let active = app.view == view;
            let key = app.keymap.prefix_keys(view.action()).first().cloned().unwrap_or_default();
            let mut job = LayoutJob::default();
            if !key.is_empty() {
                push(&mut job, &format!("{key} "), if active { theme.accent } else { theme.faint }, 11.5);
            }
            let color = if active { theme.accent_hi } else { theme.dim };
            push(&mut job, &format!("{} {}", view.icon(), view.label()), color, 12.5);
            let fill = if active { theme.bg3 } else { Color32::TRANSPARENT };
            let resp = block(ui, &format!("Aba {}", view.label()), job, fill, Stroke::NONE, active.then_some(theme.accent));
            if resp.on_hover_text(app.keymap.tip(view.action(), view.action().meta().name)).clicked() {
                app.set_view(view);
            }
        }

        // ── status (direita → esquerda) ──
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            if app.cfg.on(aic_core::Module::Layne) {
                layne_button(app, ui, &theme, t);
                sep(ui, &theme);
            }
            ui.label(RichText::new(chrono::Local::now().format("%H:%M").to_string()).font(widgets::mono(12.5)).color(theme.ink));
            if let Some(acc) = app.account.clone() {
                if let Some(w) = acc.seven_day {
                    sep(ui, &theme);
                    let mut job = LayoutJob::default();
                    push(&mut job, &format!("{I_WEEK} "), theme.dim, 12.0);
                    bar_job(&mut job, w.used_pct, 5, &theme, 12.0);
                    push(&mut job, &format!(" {:.0}%", w.used_pct), theme.usage_color(w.used_pct), 12.0);
                    if ui.add(egui::Label::new(job).sense(Sense::click())).on_hover_text(format!("Semana · reseta em {} (aba Tokens)", widgets::until(w.resets_at))).clicked() {
                        app.set_view(View::Usage);
                    }
                }
                if let Some(h) = acc.five_hour {
                    sep(ui, &theme);
                    let mut job = LayoutJob::default();
                    push(&mut job, &format!("{I_5H} "), theme.dim, 12.0);
                    bar_job(&mut job, h.used_pct, 5, &theme, 12.0);
                    push(&mut job, &format!(" {:.0}%", h.used_pct), theme.usage_color(h.used_pct), 12.0);
                    if ui.add(egui::Label::new(job).sense(Sense::click())).on_hover_text(format!("5 horas · reseta em {} (aba Tokens)", widgets::until(h.resets_at))).clicked() {
                        app.set_view(View::Usage);
                    }
                }
            }
            if let Some(cost) = app.usage_history.today_cost() {
                sep(ui, &theme);
                let text = RichText::new(format!("{} {}", widgets::I_COST, crate::usage_history::fmt_usd(cost))).font(widgets::mono(12.0)).color(theme.dim);
                if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_text("Custo estimado de hoje pelos transcripts (aba Tokens)").clicked() {
                    app.set_view(View::Usage);
                }
            }
            let count = |s: SessionState| app.sessions.values().filter(|r| r.state() == s).count();
            let working = count(SessionState::Working);
            if working > 0 {
                sep(ui, &theme);
                ui.label(RichText::new(format!("{} {working}", widgets::I_FIRE)).font(widgets::mono(12.0)).color(theme.ok))
                    .on_hover_text(format!("{working} trabalhando"));
            }
            let waiting = count(SessionState::Waiting) + app.sessions.values().filter(|r| r.unseen && r.state() == SessionState::Done).count();
            if waiting > 0 {
                sep(ui, &theme);
                let a = 0.6 + 0.4 * ((t * 4.0).sin() as f32 * 0.5 + 0.5);
                let r = ui
                    .add(egui::Label::new(RichText::new(format!("● {waiting} aguardando")).font(widgets::mono(12.0)).color(theme.attention.gamma_multiply(a)).strong()).sense(Sense::click()))
                    .on_hover_text(app.keymap.tip(Action::NextWaiting, "Ir pra próxima"));
                if r.clicked() {
                    app.next_waiting();
                }
            }
        });
    });
}

/// Ícone do Layne (o assistente): clica e abre/fecha o painel. A cor diz o estado dele:
/// aceso com o painel aberto, pulsando quando ele precisa de você ou está gravando.
fn layne_button(app: &mut App, ui: &mut Ui, theme: &Theme, t: f64) {
    let state = app.layne.state();
    let pulse = 0.55 + 0.45 * ((t * 4.0).sin() as f32 * 0.5 + 0.5);
    let (color, fill, tip) = if app.layne.recording() {
        (theme.bg, theme.err.gamma_multiply(pulse), "Layne: gravando (clique pra abrir o painel)")
    } else if state == SessionState::Waiting {
        (theme.attention.gamma_multiply(pulse), theme.attention.gamma_multiply(0.18), "Layne precisa de você")
    } else if app.layne.open {
        (theme.bg, theme.accent, "Fechar o Layne")
    } else if state == SessionState::Working {
        (theme.ok, Color32::TRANSPARENT, "Layne trabalhando")
    } else {
        (theme.ink, Color32::TRANSPARENT, "Abrir o Layne (assistente)")
    };
    let mut job = LayoutJob::default();
    push(&mut job, crate::layne::I_LAYNE, color, 14.0);
    let resp = block(ui, "Layne", job, fill, Stroke::NONE, None);
    if resp.on_hover_text(app.keymap.tip(Action::ToggleLayne, tip)).clicked() {
        let ctx = ui.ctx().clone();
        if app.layne.open {
            app.layne.open = false;
            app.layne.focus = false;
        } else {
            crate::layne::show(app, &ctx);
        }
    }
}

/// Um bloco por workspace: `1 Principal`, o atual aceso; quem tem alguém esperando fica roxo.
fn workspaces(app: &mut App, ui: &mut Ui, theme: &Theme, t: f64) {
    ui.label(RichText::new(widgets::I_WS).font(widgets::mono(12.0)).color(theme.faint)).on_hover_text("Workspaces: as sessões mostradas são só as do workspace aceso");
    let mut pick = None;
    let names: Vec<(String, usize)> = app.workspaces.iter().map(|w| (w.name.clone(), w.sessions.len())).collect();
    for (i, (name, n)) in names.iter().enumerate() {
        let current = i == app.ws;
        let urgent = app.ws_needs_you(i);
        let mut job = LayoutJob::default();
        let (num_c, name_c, fill, stroke) = if current {
            (theme.bg, theme.bg, theme.accent, Stroke::NONE)
        } else if urgent > 0 {
            let a = 0.55 + 0.45 * ((t * 3.0).sin() as f32 * 0.5 + 0.5);
            (theme.attention, theme.ink, theme.attention.gamma_multiply(0.18 * a), Stroke::new(1.0, theme.attention.gamma_multiply(a)))
        } else {
            (theme.faint, theme.dim, theme.bg3, Stroke::new(1.0, theme.border))
        };
        let size = if current { 13.5 } else { 12.5 };
        if i < 9 {
            push(&mut job, &format!("{} ", i + 1), num_c, size);
        }
        job.append(name, 0.0, egui::TextFormat { font_id: FontId::monospace(size), color: name_c, ..Default::default() });
        if urgent > 0 && !current {
            push(&mut job, &format!(" ●{urgent}"), theme.attention, 11.5);
        } else if *n > 0 {
            push(&mut job, &format!(" {n}"), if current { theme.bg.gamma_multiply(0.7) } else { theme.faint }, 11.0);
        }
        let resp = block(ui, &format!("Workspace {} {name}", i + 1), job, fill, stroke, None);
        let key = app.keymap.prefix_keys(Action::GotoWorkspace((i + 1).min(9) as u8)).first().cloned();
        let hint = match key {
            Some(k) if i < 9 => format!("Workspace {} · {n} terminais  ({} {k})", name, super::prefix::label(app)),
            _ => format!("Workspace {name} · {n} terminais"),
        };
        let resp = resp.on_hover_text(hint);
        if resp.clicked() {
            pick = Some(i);
        }
        resp.context_menu(|ui| {
            if ui.button("Renomear").clicked() {
                app.switch_ws(i);
                app.run_action(ui.ctx(), Action::RenameWorkspace);
                ui.close();
            }
            if ui.button("Gerenciar workspaces…").clicked() {
                app.run_action(ui.ctx(), Action::Workspaces);
                ui.close();
            }
        });
    }
    if let Some(i) = pick {
        app.switch_ws(i);
    }
    let mut job = LayoutJob::default();
    push(&mut job, widgets::I_PLUS, theme.faint, 11.0);
    if block(ui, "Novo workspace", job, Color32::TRANSPARENT, Stroke::NONE, None).on_hover_text(app.keymap.tip(Action::NewWorkspace, "Novo workspace")).clicked() {
        app.run_action(ui.ctx(), Action::NewWorkspace);
    }
}
