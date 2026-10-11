//! Peças visuais reutilizadas em todas as telas.

use egui::text::LayoutJob;
use egui::{Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Margin, Rect, Response, RichText, Sense, Stroke, TextFormat, Ui, Vec2};
use aic_core::SessionState;

use crate::theme::Theme;

// Ícones Nerd Font (os mesmos da statusline do terminal).
pub const I_CTX: &str = "\u{f09d1}";
pub const I_5H: &str = "\u{f1442}";
pub const I_WEEK: &str = "\u{f00ed}";
pub const I_COST: &str = "\u{f0126}";
pub const I_SPEND: &str = "\u{f0820}";
pub const I_DIR: &str = "\u{f07b}";
pub const I_BRANCH: &str = "\u{e725}";
pub const I_CARD: &str = "\u{f0e5c}";
pub const I_AGENTS: &str = "\u{f0bc9}";
pub const I_TASK: &str = "\u{f0ae}";
pub const I_SLACK: &str = "\u{f198}";
pub const I_NOTION: &str = "\u{f0ff9}";
pub const I_WORKTREE: &str = "\u{f126}";
pub const I_CLOCK: &str = "\u{f017}";
pub const I_PLUS: &str = "\u{f067}";
pub const I_KEY: &str = "\u{f11c}";
pub const I_GEAR: &str = "\u{f013}";
pub const I_BOARD: &str = "\u{f0db}";
pub const I_TERM: &str = "\u{f120}";
pub const I_CHART: &str = "\u{f080}";
pub const I_HELP: &str = "\u{f059}";
pub const I_WS: &str = "\u{f009}";
pub const I_LINK: &str = "\u{f0c1}";
pub const I_RESTART: &str = "\u{f021}";
pub const I_CLOSE: &str = "\u{f00d}";
pub const I_FIRE: &str = "\u{f06d}";
pub const I_CHECK: &str = "\u{f00c}";
pub const I_PERM: &str = "\u{f0e8d}";

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Barra `█░` igual à da statusline: verde < 50, amarelo < 80, vermelho.
pub fn bar_job(job: &mut LayoutJob, pct: f32, cells: usize, theme: &Theme, size: f32) {
    let filled = ((pct.clamp(0.0, 100.0) / 100.0) * cells as f32).floor() as usize;
    let color = theme.usage_color(pct);
    let fmt = |c: Color32| TextFormat { font_id: mono(size), color: c, ..Default::default() };
    job.append(&"█".repeat(filled), 0.0, fmt(color));
    job.append(&"░".repeat(cells - filled.min(cells)), 0.0, fmt(theme.faint));
}

pub fn push(job: &mut LayoutJob, text: &str, color: Color32, size: f32) {
    job.append(text, 0.0, TextFormat { font_id: mono(size), color, ..Default::default() });
}

/// "3h13m", "6d11h", "42m" até o epoch.
pub fn until(epoch: Option<i64>) -> String {
    let Some(e) = epoch else { return "—".into() };
    let s = (e - chrono::Utc::now().timestamp()).max(0);
    if s >= 86400 {
        format!("{}d{}h", s / 86400, s % 86400 / 3600)
    } else if s >= 3600 {
        format!("{}h{:02}m", s / 3600, s % 3600 / 60)
    } else {
        format!("{}m", s / 60)
    }
}

pub fn chip(ui: &mut Ui, text: impl Into<String>, color: Color32, theme: &Theme) -> Response {
    let text = text.into();
    let font = FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(text, font, color);
    let size = galley.size() + Vec2::new(12.0, 4.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let r = CornerRadius::same(if theme.radius == 0 { 0 } else { 99 });
    ui.painter().rect(rect, r, color.gamma_multiply(0.14), Stroke::new(1.0, color.gamma_multiply(0.45)), egui::StrokeKind::Inside);
    ui.painter().galley(rect.min + Vec2::new(6.0, 2.0), galley, color);
    resp
}

pub fn state_badge(ui: &mut Ui, label: &str, state: SessionState, theme: &Theme, t: f64) -> Response {
    let color = theme.state_color(state);
    let pulse = matches!(state, SessionState::Waiting | SessionState::Done);
    let a = if pulse { 0.55 + 0.45 * ((t * 4.0).sin() as f32 * 0.5 + 0.5) } else { 1.0 };
    let font = FontId::new(10.5, FontFamily::Monospace);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, theme.bg);
    let size = galley.size() + Vec2::new(10.0, 4.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(theme.radius.min(4)), color.gamma_multiply(a));
    ui.painter().galley(rect.min + Vec2::new(5.0, 2.0), galley, theme.bg);
    resp
}

/// Bolinha de estado (com halo pulsando quando precisa de você).
pub fn state_dot(ui: &Ui, center: egui::Pos2, r: f32, state: SessionState, theme: &Theme, t: f64) {
    let color = theme.state_color(state);
    if state.needs_attention() {
        let k = ((t * 3.0).sin() as f32 * 0.5 + 0.5) * r * 1.6;
        ui.painter().circle_filled(center, r + k, color.gamma_multiply(0.18));
    }
    ui.painter().circle_filled(center, r, color);
}

/// Frame de card. Com `hard_shadow` (tema AIC) desenha a sombra deslocada sólida.
pub fn card(ui: &mut Ui, theme: &Theme, active: bool, accent: Color32, add: impl FnOnce(&mut Ui)) -> Response {
    let stroke = if active { Stroke::new(1.5, accent) } else { Stroke::new(1.0, theme.border) };
    let frame = Frame::new()
        .fill(if active { theme.bg3 } else { theme.bg2 })
        .stroke(stroke)
        .corner_radius(theme.radius())
        .inner_margin(Margin::same(10));
    let shadow_at = ui.painter().add(egui::Shape::Noop);
    // `UiBuilder::sense` registra o clique do card ANTES do conteúdo: botões dentro dele
    // continuam clicáveis (antes o `interact` depois do conteúdo roubava os cliques).
    let inner = ui.scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| {
        frame.show(ui, add);
    });
    let rect = inner.response.rect;
    if let Some(o) = theme.hard_shadow {
        ui.painter().set(
            shadow_at,
            egui::Shape::rect_filled(rect.translate(Vec2::splat(o)), theme.radius(), Color32::from_black_alpha(if theme.dark { 230 } else { 70 })),
        );
    }
    // faixa colorida à esquerda (cor da sessão)
    let strip = Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
    ui.painter().rect_filled(strip, CornerRadius::ZERO, accent);
    inner.response
}

/// Tecla desenhada (`Ctrl+Shift+N`).
pub fn kbd(ui: &mut Ui, text: &str, theme: &Theme) {
    if text.is_empty() {
        ui.label(RichText::new("—").color(theme.faint));
        return;
    }
    let font = FontId::new(11.0, FontFamily::Monospace);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, theme.ink);
    let size = galley.size() + Vec2::new(10.0, 6.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let r = CornerRadius::same(theme.radius.min(4));
    ui.painter().rect(rect.translate(Vec2::new(0.0, 1.5)), r, theme.border, Stroke::NONE, egui::StrokeKind::Inside);
    ui.painter().rect(rect, r, theme.bg3, Stroke::new(1.0, theme.border), egui::StrokeKind::Inside);
    ui.painter().galley(rect.min + Vec2::new(5.0, 3.0), galley, theme.ink);
}

pub fn heading(ui: &mut Ui, text: &str, theme: &Theme) {
    ui.label(RichText::new(text).font(FontId::new(18.0, FontFamily::Monospace)).color(theme.accent_hi).strong());
}

pub fn subtle(text: impl Into<String>, theme: &Theme) -> RichText {
    RichText::new(text.into()).color(theme.dim)
}

/// Texto centralizado grande (estados vazios).
pub fn empty_state(ui: &mut Ui, title: &str, hint: &str, theme: &Theme) {
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        ui.label(RichText::new(title).size(18.0).color(theme.ink));
        ui.add_space(4.0);
        ui.label(RichText::new(hint).color(theme.dim));
    });
}

pub fn paint_text(ui: &Ui, pos: egui::Pos2, align: Align2, text: &str, size: f32, color: Color32) -> Rect {
    ui.painter().text(pos, align, text, mono(size), color)
}
