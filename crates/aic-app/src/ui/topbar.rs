//! Barra superior no espírito do i3bar, uma linha só:
//!
//! ```text
//!  AIC │ 1 Principal │ 2 api ●2 │ 3 front │ +   PREFIXO   s Sessões  g Worktrees …    THEM BONES · api · Opus 5.5 · 17% · main ±3 · $3.83 │ L
//! ```
//!
//! - **workspaces** à esquerda e em destaque: o atual aceso na cor de destaque, os que têm
//!   alguém precisando de você em roxo (o "urgent" do i3). O número é o de `Ctrl+B N`;
//! - **PREFIXO** acende enquanto o `Ctrl+B` espera a próxima tecla (o modo do i3);
//! - **abas** (só texto) com a tecla de prefixo de cada uma; a Ajuda fica fora da barra (F1);
//! - à direita, **a janela em foco**: estado, nome, modelo, contexto, branch e custo da sessão
//!   (o que a statusline do Claude mostrava dentro do terminal), e o ícone do Layne.

use egui::text::LayoutJob;
use egui::{Color32, CornerRadius, FontId, Rect, Sense, Stroke, Ui, Vec2};
use aic_core::{SessionKind, SessionState};

use super::widgets::{self, I_AGENTS, I_BRANCH, I_CTX, push};
use crate::actions::Action;
use crate::app::App;
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
    let theme = app.theme.clone();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.set_min_height(H);

        // ── marca ──
        let pos = ui.cursor().min + Vec2::new(3.0, 0.0);
        let wm = crate::brand::wordmark(ui, pos, 15.0, &theme);
        ui.allocate_rect(Rect::from_min_size(wm.min, wm.size() + Vec2::new(6.0, 0.0)), Sense::hover());
        sep(ui, &theme);

        // ── workspaces ──
        if app.cfg.on(aic_core::Module::Workspaces) {
            workspaces(app, ui, &theme);
        } else if let Some(w) = app.current_ws() {
            let mut job = LayoutJob::default();
            push(&mut job, &w.name, theme.dim, 12.5);
            block(ui, &format!("Workspace {}", w.name), job, Color32::TRANSPARENT, Stroke::NONE, None);
        }

        // ── modo: prefixo armado ──
        if app.prefix_pending {
            ui.add_space(4.0);
            let mut job = LayoutJob::default();
            push(&mut job, &format!("{} {}", widgets::I_KEY, super::prefix::label(app)), theme.bg, 12.5);
            block(ui, "Prefixo armado", job, theme.warn, Stroke::NONE, None)
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
            push(&mut job, view.label(), if active { theme.accent_hi } else { theme.dim }, 12.5);
            let resp = block(ui, &format!("Aba {}", view.label()), job, Color32::TRANSPARENT, Stroke::NONE, active.then_some(theme.accent));
            if resp.on_hover_text(app.keymap.tip(view.action(), view.action().meta().name)).clicked() {
                app.set_view(view);
            }
        }

        // ── à direita: a janela em foco e o Layne ──
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            if app.cfg.on(aic_core::Module::Layne) {
                layne_button(app, ui, &theme);
                sep(ui, &theme);
            }
            focus_status(app, ui, &theme);
        });
    });
}

/// A janela em foco numa linha, no lugar da statusline do Claude: estado, nome, modelo, contexto,
/// branch e custo da sessão. Os números vêm do snapshot que o `aic statusline` grava.
fn focus_status(app: &App, ui: &mut Ui, theme: &Theme) {
    let Some(rt) = app.focused().and_then(|id| app.sessions.get(&id)) else { return };
    let state = rt.state();
    let mut job = LayoutJob::default();
    let dot = |job: &mut LayoutJob| push(job, "  ·  ", theme.faint, 12.0);
    push(&mut job, crate::brand::state_label(state), theme.state_color(state), 12.0);
    dot(&mut job);
    push(&mut job, &widgets::truncate(&rt.s.name, 28), theme.ink, 12.5);
    match (&rt.s.kind, &rt.snap) {
        (SessionKind::Claude, Some(s)) => {
            dot(&mut job);
            push(&mut job, &s.model, theme.dim, 12.0);
            if let Some(pct) = s.context_pct {
                dot(&mut job);
                push(&mut job, &format!("{I_CTX} {pct:.0}%"), theme.usage_color(pct), 12.0);
            }
        }
        (SessionKind::Claude, None) => {}
        (SessionKind::Shell, _) => {
            dot(&mut job);
            push(&mut job, "shell", theme.dim, 12.0);
        }
        (SessionKind::Command(c), _) => {
            dot(&mut job);
            push(&mut job, &format!("$ {}", widgets::truncate(c.lines().next().unwrap_or(""), 24)), theme.dim, 12.0);
        }
    }
    if let Some(g) = &rt.git {
        if let Some(b) = &g.branch {
            dot(&mut job);
            push(&mut job, &format!("{I_BRANCH} {}", widgets::truncate(b, 32)), theme.dim, 12.0);
            if g.dirty > 0 {
                push(&mut job, &format!(" ±{}", g.dirty), theme.warn, 12.0);
            }
        }
    }
    if let Some(s) = rt.snap.as_ref().filter(|s| s.cost_usd > 0.0) {
        dot(&mut job);
        push(&mut job, &crate::usage_history::fmt_usd(s.cost_usd), theme.dim, 12.0);
    }
    let agents = rt.live.agents.len();
    if agents > 0 {
        dot(&mut job);
        push(&mut job, &format!("{I_AGENTS} {agents}"), theme.attention, 12.0);
    }
    ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Truncate)).on_hover_text(aic_core::paths::pretty(&rt.s.cwd));
}

/// Ícone do Layne (o assistente): clica e abre/fecha o painel. A cor diz o estado dele:
/// aceso com o painel aberto, em destaque quando ele precisa de você ou está gravando.
fn layne_button(app: &mut App, ui: &mut Ui, theme: &Theme) {
    let state = app.layne.state();
    let (color, fill, tip) = if app.layne.recording() {
        (theme.bg, theme.err, "Layne: gravando (clique pra abrir o painel)")
    } else if state == SessionState::Waiting {
        (theme.attention, theme.attention.gamma_multiply(0.18), "Layne precisa de você")
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
fn workspaces(app: &mut App, ui: &mut Ui, theme: &Theme) {
    let mut pick = None;
    let names: Vec<(String, usize)> = app.workspaces.iter().map(|w| (w.name.clone(), w.sessions.len())).collect();
    for (i, (name, n)) in names.iter().enumerate() {
        let current = i == app.ws;
        let urgent = app.ws_needs_you(i);
        let mut job = LayoutJob::default();
        let (num_c, name_c, fill, stroke) = if current {
            (theme.bg, theme.bg, theme.accent, Stroke::NONE)
        } else if urgent > 0 {
            (theme.attention, theme.ink, theme.attention.gamma_multiply(0.18), Stroke::new(1.0, theme.attention))
        } else {
            (theme.faint, theme.dim, Color32::TRANSPARENT, Stroke::NONE)
        };
        let size = if current { 13.5 } else { 12.5 };
        if i < 9 {
            push(&mut job, &format!("{} ", i + 1), num_c, size);
        }
        job.append(name, 0.0, egui::TextFormat { font_id: FontId::monospace(size), color: name_c, ..Default::default() });
        if urgent > 0 && !current {
            push(&mut job, &format!(" ●{urgent}"), theme.attention, 11.5);
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
}
