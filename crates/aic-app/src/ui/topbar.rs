//! Barra superior: mascote + nome do fork, abas, workspace, contadores e limites da conta.

use egui::text::LayoutJob;
use egui::{Rect, RichText, Sense, Ui, Vec2};
use aic_core::SessionState;

use super::widgets::{self, I_5H, I_WEEK, bar_job, push};
use crate::actions::Action;
use crate::app::{App, View};

pub fn show(app: &mut App, ui: &mut Ui) {
    let t = app.t();
    let theme = app.theme.clone();
    ui.horizontal(|ui| {
        // Mascote reflete o "humor geral": alguém precisa de você > trabalhando > terminou > parado.
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
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::click());
        if app.cfg.on(aic_core::Module::Mascot) {
            app.skin.mascot(ui, rect, mood, t, &theme);
        }
        if resp.on_hover_text("Ajuda do fork (F1)").clicked() {
            app.set_view(View::ForkHelp);
        }

        let pos = ui.cursor().min + Vec2::new(0.0, 4.0);
        let wm = app.skin.wordmark(ui, pos, 18.0, &theme);
        ui.allocate_rect(Rect::from_min_size(wm.min, wm.size() + Vec2::new(4.0, 0.0)), Sense::hover());
        if !app.skin.creator().is_empty() {
            ui.label(widgets::subtle(format!("por {}", app.skin.creator()), &theme).size(11.0));
        }
        ui.add_space(14.0);

        // Abas
        let tabs = [
            (View::Sessions, widgets::I_TERM, "Sessões", Action::ViewSessions),
            (View::Board, widgets::I_BOARD, "Board", Action::ViewBoard),
            (View::Worktrees, widgets::I_WORKTREE, "Worktrees", Action::ViewWorktrees),
            (View::Usage, widgets::I_CHART, "Tokens", Action::ViewUsage),
            (View::Settings, widgets::I_GEAR, "Config", Action::ViewSettings),
            (View::ForkHelp, widgets::I_HELP, app.skin.name(), Action::ViewForkHelp),
        ];
        for (view, icon, label, action) in tabs {
            let active = app.view == view;
            let mut text = RichText::new(format!("{icon}  {label}"));
            text = if active { text.color(theme.accent_hi).strong() } else { text.color(theme.dim) };
            let resp = ui.add(egui::Button::new(text).frame(active).fill(theme.bg3));
            let resp = resp.on_hover_text(app.keymap.label(action));
            if resp.clicked() {
                app.set_view(view);
            }
            if view == View::Board {
                let open = app.tasks.iter().filter(|t| t.status == aic_core::TaskStatus::Doing).count();
                if open > 0 {
                    ui.label(RichText::new(open.to_string()).small().color(theme.accent));
                }
            }
        }

        ui.add_space(10.0);
        let ws_name = app.current_ws().map(|w| w.name.clone()).unwrap_or_default();
        let ws_btn = ui
            .add(egui::Button::new(RichText::new(format!("{}  {ws_name}  ▾", widgets::I_WS)).color(theme.ink)))
            .on_hover_text(format!("Workspaces ({})", app.keymap.label(Action::Workspaces)));
        if ws_btn.clicked() {
            app.run_action(ui.ctx(), Action::Workspaces);
        }

        // Direita: contadores + limites da conta
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(acc) = &app.account {
                let mut job = LayoutJob::default();
                if let Some(h) = acc.five_hour {
                    push(&mut job, &format!("{I_5H} 5h "), theme.dim, 12.0);
                    bar_job(&mut job, h.used_pct, 8, &theme, 12.0);
                    push(&mut job, &format!(" {:.0}% ", h.used_pct), theme.usage_color(h.used_pct), 12.0);
                    push(&mut job, &format!("↻{}   ", widgets::until(h.resets_at)), theme.faint, 11.0);
                }
                if let Some(w) = acc.seven_day {
                    push(&mut job, &format!("{I_WEEK} sem "), theme.dim, 12.0);
                    bar_job(&mut job, w.used_pct, 8, &theme, 12.0);
                    push(&mut job, &format!(" {:.0}% ", w.used_pct), theme.usage_color(w.used_pct), 12.0);
                    push(&mut job, &format!("↻{}", widgets::until(w.resets_at)), theme.faint, 11.0);
                }
                if !job.is_empty() && ui.add(egui::Label::new(job).sense(Sense::click())).on_hover_text("Limites da conta (aba Tokens)").clicked() {
                    app.set_view(View::Usage);
                }
            }
            if let Some(cost) = app.usage_history.today_cost() {
                ui.add_space(6.0);
                let text = RichText::new(format!("{} {} hoje", widgets::I_COST, crate::usage_history::fmt_usd(cost))).font(widgets::mono(12.0)).color(theme.dim);
                if ui.add(egui::Label::new(text).sense(Sense::click())).on_hover_text("Custo estimado de hoje pelos transcripts (aba Tokens)").clicked() {
                    app.set_view(View::Usage);
                }
            }
            ui.add_space(12.0);
            let count = |s: SessionState| app.sessions.values().filter(|r| r.state() == s).count();
            let waiting = count(SessionState::Waiting) + app.sessions.values().filter(|r| r.unseen && r.state() == SessionState::Done).count();
            let working = count(SessionState::Working);
            if waiting > 0 {
                let a = 0.6 + 0.4 * ((t * 4.0).sin() as f32 * 0.5 + 0.5);
                let r = ui
                    .add(egui::Label::new(RichText::new(format!("● {waiting} aguardando")).color(theme.attention.gamma_multiply(a)).strong()).sense(Sense::click()))
                    .on_hover_text(format!("Ir pra próxima ({})", app.keymap.label(Action::NextWaiting)));
                if r.clicked() {
                    app.next_waiting();
                }
            }
            if working > 0 {
                ui.label(RichText::new(format!("{} {working} trabalhando", widgets::I_FIRE)).color(theme.ok));
            }
        });
    });
}
