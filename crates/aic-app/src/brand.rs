//! Identidade visual: Anakin Skywalker, o cavaleiro Jedi. Azul do sabre, areia de Tatooine e o
//! preto do espaço. Sem logos nem imagens oficiais: o sabre da barra é desenhado aqui.

use aic_core::SessionState;
use egui::{CornerRadius, Pos2, Rect, Ui, pos2, vec2};

use crate::theme::{Theme, c, term_from};

pub const NAME: &str = "Anakin";
pub const TAGLINE: &str = "now this is podracing";

/// Manual (aba Ajuda, F1).
pub const HELP: &str = include_str!("../../../docs/MANUAL.md");

pub fn theme() -> Theme {
    Theme {
        dark: true,
        bg: c("#0a0e14"),
        bg2: c("#10151e"),
        bg3: c("#18202c"),
        bg4: c("#222c3b"),
        border: c("#263142"),
        border_active: c("#4da3ff"),
        ink: c("#dbe6f2"),
        dim: c("#8493a8"),
        faint: c("#4f5c70"),
        accent: c("#2f8fff"),
        accent_hi: c("#7cc4ff"),
        accent_lo: c("#12335c"),
        ok: c("#6fc7a0"),
        warn: c("#e3c27f"),
        err: c("#e5533d"),
        attention: c("#ff9d4d"),
        agents: [
            "#2f8fff", "#6fc7a0", "#e3c27f", "#ff9d4d", "#5fd0d6", "#b79cff", "#a8b6c8",
        ]
        .map(c)
        .to_vec(),
        radius: 0,
        hard_shadow: None,
        term: term_from(
            "#0a0e14",
            "#dbe6f2",
            "#7cc4ff",
            "#12335c",
            [
                "#18202c", "#e5533d", "#6fc7a0", "#e3c27f", "#4da3ff", "#b79cff", "#5fd0d6",
                "#c9d6e5", "#4f5c70", "#ff7a66", "#8fe0bb", "#f2d9a0", "#7cc4ff", "#cdb8ff",
                "#8be4e9", "#f4f8fc",
            ],
        ),
    }
}

/// Nome do estado.
pub fn state_label(s: SessionState) -> &'static str {
    match s {
        SessionState::Boot => "IGNITING",
        SessionState::Working => "PODRACING",
        SessionState::Idle => "MEDITATING",
        SessionState::Waiting => "BAD FEELING",
        SessionState::Done => "YIPPEE",
        SessionState::GameOver => "HIGH GROUND",
    }
}

/// O sabre de luz da barra: empunhadura e lâmina azul, com brilho.
pub fn saber(ui: &Ui, pos: Pos2, height: f32, theme: &Theme) -> Rect {
    let p = ui.painter();
    let cy = pos.y + height / 2.0;
    let hilt_w = 10.0;
    let blade_w = 26.0;
    let x = pos.x;
    let tip = x + hilt_w + blade_w;
    let blade = |half: f32, color| {
        p.rect_filled(
            Rect::from_min_max(pos2(x + hilt_w, cy - half), pos2(tip, cy + half)),
            CornerRadius::same(half.ceil() as u8),
            color,
        );
    };
    blade(3.5, theme.accent.gamma_multiply(0.22));
    blade(2.0, theme.accent);
    blade(0.9, c("#eaf6ff"));
    p.rect_filled(
        Rect::from_min_size(pos2(x, cy - 2.0), vec2(hilt_w, 4.0)),
        0.0,
        c("#9aa7b8"),
    );
    p.rect_filled(
        Rect::from_min_size(pos2(x + 2.0, cy - 2.0), vec2(1.5, 4.0)),
        0.0,
        c("#39424f"),
    );
    p.rect_filled(
        Rect::from_min_size(pos2(x + hilt_w - 2.0, cy - 2.6), vec2(2.0, 5.2)),
        0.0,
        c("#c9d3df"),
    );
    Rect::from_min_size(pos, vec2(tip - x, height))
}
