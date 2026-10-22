//! Identidade visual do AIC: cores, letreiro e nomes dos estados. Inspirado no grunge de
//! Seattle e em Alice in Chains; nada de logos ou capas oficiais.

use aic_core::SessionState;
use egui::epaint::TextShape;
use egui::{FontFamily, FontId, Pos2, Rect, Stroke, Ui, pos2, vec2};

use crate::theme::{Theme, c, term_from};

pub const NAME: &str = "AIC";
pub const TAGLINE: &str = "down in a hole, up in the stack";

/// Manual (aba Ajuda, F1).
pub const HELP: &str = include_str!("../../../docs/MANUAL.md");

/// Terra seca, ferrugem e mostarda queimada.
pub fn theme() -> Theme {
    Theme {
        dark: true,
        bg: c("#110e0b"),
        bg2: c("#1a1510"),
        bg3: c("#241d16"),
        bg4: c("#30271d"),
        border: c("#3a2e22"),
        border_active: c("#c0602f"),
        ink: c("#e8dac4"),
        dim: c("#9a8a74"),
        faint: c("#65574a"),
        accent: c("#b5522b"),
        accent_hi: c("#d06a3d"),
        accent_lo: c("#4a2414"),
        ok: c("#8a9a4e"),
        warn: c("#c9a227"),
        err: c("#b0362b"),
        attention: c("#9b7bb0"),
        agents: [
            "#b5522b", "#8a9a4e", "#c9a227", "#9b7bb0", "#5f8b99", "#b0362b", "#a08a70",
        ]
        .map(c)
        .to_vec(),
        radius: 2,
        hard_shadow: Some(2.0),
        term: term_from(
            "#110e0b",
            "#e8dac4",
            "#d06a3d",
            "#4a2414",
            [
                "#241d16", "#b0362b", "#8a9a4e", "#c9a227", "#5f8b99", "#9b7bb0", "#6f9a88",
                "#d6c8b0", "#65574a", "#d04a3c", "#a6b862", "#e3bc3c", "#7eaabb", "#b899cc",
                "#8bb8a4", "#f6ead6",
            ],
        ),
    }
}

/// Nome do estado na linguagem do AIC.
pub fn state_label(s: SessionState) -> &'static str {
    match s {
        SessionState::Boot => "ROOSTER",
        SessionState::Working => "THEM BONES",
        SessionState::Idle => "NUTSHELL",
        SessionState::Waiting => "WOULD?",
        SessionState::Done => "NO EXCUSES",
        SessionState::GameOver => "DOWN IN A HOLE",
    }
}

/// "AIC" com cara de cartaz xerocado: letras tortas, impressão desalinhada e riscos.
pub fn wordmark(ui: &Ui, pos: Pos2, size: f32, theme: &Theme) -> Rect {
    let p = ui.painter();
    let font = FontId::new(size * 1.3, FontFamily::Monospace);
    let letters = [
        ('A', -0.09f32, 1.0f32),
        ('I', 0.06, -1.5),
        ('C', -0.04, 0.5),
    ];
    let mut x = pos.x;
    let mut h: f32 = 0.0;
    let ghost = theme.err.gamma_multiply(0.55);
    for (ch, ang, dy) in letters {
        let g = p.layout_no_wrap(ch.to_string(), font.clone(), theme.ink);
        let w = g.size().x;
        h = h.max(g.size().y);
        let at = pos2(x, pos.y + dy);
        // impressão fantasma desalinhada
        p.add(
            TextShape::new(at + vec2(1.5, 1.0), g.clone(), ghost)
                .with_override_text_color(ghost)
                .with_angle(ang * 1.4),
        );
        // bold "na mão": a mesma letra 3x com meio pixel de diferença
        for d in [vec2(0.0, 0.0), vec2(0.6, 0.0), vec2(0.0, 0.5)] {
            p.add(
                TextShape::new(at + d, g.clone(), theme.accent_hi)
                    .with_override_text_color(theme.accent_hi)
                    .with_angle(ang),
            );
        }
        x += w * 1.02;
    }
    let r = Rect::from_min_size(pos, vec2(x - pos.x, h));
    // riscos e falhas de tinta (na cor do fundo)
    let scratch = Stroke::new(1.0, theme.bg);
    p.line_segment(
        [
            pos2(r.left() - 1.0, r.top() + h * 0.42),
            pos2(r.right() + 1.0, r.top() + h * 0.36),
        ],
        scratch,
    );
    p.line_segment(
        [
            pos2(r.left() + r.width() * 0.3, r.top() + h * 0.75),
            pos2(r.right() - 2.0, r.top() + h * 0.80),
        ],
        Stroke::new(0.8, theme.bg),
    );
    for i in 0..14 {
        let fx = rnd(i as f32 * 3.3 + 0.7);
        let fy = 0.2 + 0.65 * rnd(i as f32 * 5.1 + 0.2);
        p.circle_filled(
            pos2(r.left() + fx * r.width(), r.top() + fy * h),
            0.5 + rnd(i as f32 * 1.7) * 0.6,
            theme.bg,
        );
    }
    r
}

/// Hash barato e determinístico em [0, 1).
pub fn rnd(seed: f32) -> f32 {
    ((seed * 12.9898).sin() * 43758.545).fract().abs()
}
