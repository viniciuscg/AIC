//! Aba Tokens: limites da conta (5h, semana) + uso por sessão.

use aic_core::SessionKind;
use egui::text::LayoutJob;
use egui::{FontId, RichText, Ui};

use super::widgets::{self, I_5H, I_COST, I_CTX, I_WEEK, bar_job, push, until};
use crate::app::App;

#[derive(Default)]
pub struct UsageState {}

pub fn show(app: &mut App, ui: &mut Ui) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| page(app, ui));
}

fn page(app: &mut App, ui: &mut Ui) {
    let theme = app.theme.clone();
    widgets::heading(
        ui,
        &format!("{}  Tokens e limites", widgets::I_CHART),
        &theme,
    );
    ui.label(widgets::subtle("Os limites vêm da statusline do Claude Code (só assinaturas Pro/Max, depois da 1ª resposta).", &theme).small());
    ui.add_space(12.0);

    match &app.account {
        Some(acc) => {
            for (icon, label, lim) in [
                (I_5H, "Janela de 5 horas", acc.five_hour),
                (I_WEEK, "Semana", acc.seven_day),
                (widgets::I_SPEND, "Limite de gasto", acc.spend),
            ] {
                let Some(l) = lim else { continue };
                let mut job = LayoutJob::default();
                push(&mut job, &format!("{icon}  {label:<20}"), theme.ink, 16.0);
                bar_job(&mut job, l.used_pct, 30, &theme, 16.0);
                push(
                    &mut job,
                    &format!("  {:>3.0}%", l.used_pct),
                    theme.usage_color(l.used_pct),
                    16.0,
                );
                push(
                    &mut job,
                    &format!("   reseta em {}", until(l.resets_at)),
                    theme.dim,
                    14.0,
                );
                ui.label(job);
                ui.add_space(4.0);
            }
            ui.label(
                widgets::subtle(
                    format!(
                        "modelo da última leitura: {} · atualizado há {}s",
                        acc.model,
                        chrono::Utc::now().timestamp() - acc.at
                    ),
                    &theme,
                )
                .small(),
            );
        }
        None => {
            ui.label(RichText::new("Ainda sem leitura dos limites.").color(theme.warn));
            ui.label(widgets::subtle("Abra uma sessão Claude pelo AIC e mande uma mensagem: a statusline passa a gravar os números.", &theme));
        }
    }

    ui.add_space(18.0);
    ui.label(
        RichText::new("POR SESSÃO")
            .font(FontId::monospace(11.0))
            .color(theme.dim),
    );
    ui.add_space(4.0);
    let mut total_cost = 0.0;
    egui::Grid::new("usage-sessions")
        .striped(true)
        .num_columns(7)
        .spacing([18.0, 6.0])
        .show(ui, |ui| {
            for h in [
                "Sessão",
                "Workspace",
                "Modelo",
                "Contexto",
                "Custo",
                "Tempo",
                "Linhas",
            ] {
                ui.label(RichText::new(h).strong().color(theme.dim));
            }
            ui.end_row();
            for w in &app.workspaces {
                for id in &w.sessions {
                    let Some(rt) = app.sessions.get(id) else {
                        continue;
                    };
                    if !matches!(rt.s.kind, SessionKind::Claude) {
                        continue;
                    }
                    if ui.link(&rt.s.name).clicked() {
                        let id = *id;
                        app.focus(id);
                        return;
                    }
                    ui.label(&w.name);
                    match &rt.snap {
                        Some(s) => {
                            ui.label(format!(
                                "{}{}",
                                s.model,
                                s.effort
                                    .as_ref()
                                    .map(|e| format!(" · {e}"))
                                    .unwrap_or_default()
                            ));
                            let mut job = LayoutJob::default();
                            let pct = s.context_pct.unwrap_or(0.0);
                            push(&mut job, &format!("{I_CTX} "), theme.dim, 13.0);
                            bar_job(&mut job, pct, 10, &theme, 13.0);
                            push(&mut job, &format!(" {pct:.0}%"), theme.dim, 13.0);
                            ui.label(job);
                            ui.label(format!("{I_COST} ${:.2}", s.cost_usd));
                            ui.label(format!("{}min", s.duration_ms / 60_000));
                            ui.label(
                                RichText::new(format!("+{} -{}", s.lines_added, s.lines_removed))
                                    .color(theme.dim),
                            );
                            total_cost += s.cost_usd;
                        }
                        None => {
                            for _ in 0..5 {
                                ui.label(widgets::subtle("—", &theme));
                            }
                        }
                    }
                    ui.end_row();
                }
            }
        });
    ui.add_space(8.0);
    ui.label(RichText::new(format!("Total das sessões abertas: ${total_cost:.2}")).strong());
    crate::usage_history::show(app, ui);
}
