//! Tokens visuais do AIC e a conversão pro `Visuals` do egui. As cores ficam em `brand`.

use egui::{Color32, CornerRadius, FontFamily, FontId, Shadow, Stroke, Style, TextStyle, Visuals};
use aic_core::SessionState;
use aic_term::{TerminalTheme, hex};

#[derive(Debug, Clone)]
pub struct Theme {
    pub dark: bool,
    /// Fundo da janela → painéis → cabeçalhos → elevado.
    pub bg: Color32,
    pub bg2: Color32,
    pub bg3: Color32,
    pub bg4: Color32,
    pub border: Color32,
    pub border_active: Color32,
    pub ink: Color32,
    pub dim: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_hi: Color32,
    pub accent_lo: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    /// "Precisa de você" (roxo).
    pub attention: Color32,
    /// Cores por sessão (borda/etiqueta).
    pub agents: Vec<Color32>,
    pub radius: u8,
    /// Sombra dura deslocada (ex: 3px). `None` = sombra suave padrão.
    pub hard_shadow: Option<f32>,
    pub term: TerminalTheme,
}

impl Theme {
    pub fn agent(&self, i: u8) -> Color32 {
        self.agents.get(i as usize % self.agents.len().max(1)).copied().unwrap_or(self.accent)
    }

    pub fn state_color(&self, s: SessionState) -> Color32 {
        match s {
            SessionState::Boot => self.dim,
            SessionState::Working => self.ok,
            SessionState::Idle => self.faint,
            SessionState::Waiting => self.attention,
            SessionState::Done => self.warn,
            SessionState::GameOver => self.err,
        }
    }

    /// verde < 50, amarelo < 80, vermelho — igual à statusline do terminal.
    pub fn usage_color(&self, pct: f32) -> Color32 {
        if pct >= 80.0 {
            self.err
        } else if pct >= 50.0 {
            self.warn
        } else {
            self.ok
        }
    }

    pub fn radius(&self) -> CornerRadius {
        CornerRadius::same(self.radius)
    }

    pub fn apply(&self, ctx: &egui::Context, ui_scale: f32) {
        let mut v = if self.dark { Visuals::dark() } else { Visuals::light() };
        v.override_text_color = Some(self.ink);
        v.panel_fill = self.bg;
        v.window_fill = self.bg2;
        v.extreme_bg_color = self.bg3;
        v.faint_bg_color = self.bg2;
        v.code_bg_color = self.bg3;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.window_corner_radius = self.radius();
        v.menu_corner_radius = self.radius();
        v.hyperlink_color = self.accent_hi;
        v.warn_fg_color = self.warn;
        v.error_fg_color = self.err;
        v.selection.bg_fill = self.accent_lo;
        v.selection.stroke = Stroke::new(1.0, self.ink);
        v.window_shadow = match self.hard_shadow {
            Some(o) => Shadow { offset: [o as i8, o as i8], blur: 0, spread: 0, color: Color32::from_black_alpha(200) },
            None => Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(if self.dark { 140 } else { 50 }) },
        };
        v.popup_shadow = v.window_shadow;

        let r = self.radius();
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.bg2;
        w.noninteractive.weak_bg_fill = self.bg2;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.ink);
        w.noninteractive.corner_radius = r;
        w.inactive.bg_fill = self.bg3;
        w.inactive.weak_bg_fill = self.bg3;
        w.inactive.bg_stroke = Stroke::new(1.0, self.border);
        w.inactive.fg_stroke = Stroke::new(1.0, self.ink);
        w.inactive.corner_radius = r;
        w.hovered.bg_fill = self.bg4;
        w.hovered.weak_bg_fill = self.bg4;
        w.hovered.bg_stroke = Stroke::new(1.0, self.accent);
        w.hovered.fg_stroke = Stroke::new(1.5, self.ink);
        w.hovered.corner_radius = r;
        w.active.bg_fill = self.accent_lo;
        w.active.weak_bg_fill = self.accent_lo;
        w.active.bg_stroke = Stroke::new(1.0, self.accent_hi);
        w.active.fg_stroke = Stroke::new(2.0, self.ink);
        w.active.corner_radius = r;
        w.open = w.active;

        let mut style = Style { visuals: v, ..Style::default() };
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.window_margin = egui::Margin::same(14);
        // Rótulo selecionável sente clique e arrasto: o título de um cartão, o nome do bloco
        // ou do terminal roubavam o clique do fundo (clicar no texto não fazia nada) e o
        // arrasto do cartão no board. Texto pra copiar fica no terminal e nos campos.
        style.interaction.selectable_labels = false;
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
        ]
        .into();
        ctx.set_global_style(style);
        ctx.set_zoom_factor(ui_scale);
    }
}

/// Paleta ANSI do terminal.
pub fn term_from(bg: &str, fg: &str, cursor: &str, sel: &str, ansi: [&str; 16]) -> TerminalTheme {
    TerminalTheme::from_hex(bg, fg, cursor, sel, ansi)
}

/// Atalho: `c("#ffffff")`.
pub fn c(s: &str) -> Color32 {
    hex(s)
}
