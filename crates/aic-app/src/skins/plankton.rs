//! Tema Plankton. Laboratório do Siri Cascudo... digo, do Balde de Lixo.

use std::f32::consts::PI;
use std::path::PathBuf;

use egui::epaint::{CubicBezierShape, PathStroke};
use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Shape, Stroke, Ui, Vec2, pos2, vec2};
use aic_core::{SessionState, paths};

use super::Skin;
use crate::theme::{Theme, c, term_from};

pub struct Plankton;

impl Skin for Plankton {
    fn id(&self) -> &'static str {
        "plankton"
    }
    fn name(&self) -> &'static str {
        "Plankton"
    }
    fn creator(&self) -> &'static str {
        ""
    }
    fn tagline(&self) -> &'static str {
        "central de vibecode — o plano infalível pra roubar a fórmula do código"
    }
    fn variants(&self) -> &'static [&'static str] {
        &["balde", "karen"]
    }

    fn theme(&self, variant: &str) -> Theme {
        match variant {
            // Karen: o computador-esposa. Fósforo verde-água num CRT.
            "karen" => Theme {
                variant: "karen",
                dark: true,
                bg: c("#061514"),
                bg2: c("#0a1f1e"),
                bg3: c("#0f2b29"),
                bg4: c("#153a37"),
                border: c("#1d4a46"),
                border_active: c("#46e0cf"),
                ink: c("#c8fff6"),
                dim: c("#6fb3aa"),
                faint: c("#3f7c75"),
                accent: c("#2fc4b2"),
                accent_hi: c("#5ff0dd"),
                accent_lo: c("#13554e"),
                ok: c("#6ee07a"),
                warn: c("#f2d74e"),
                err: c("#ff5e57"),
                attention: c("#ff9df0"),
                agents: ["#2fc4b2", "#f2d74e", "#6ee07a", "#ff9df0", "#7fb7ff", "#ff8a5c", "#c3a6ff", "#e0453a"]
                    .map(c)
                    .to_vec(),
                radius: 4,
                hard_shadow: None,
                term: term_from(
                    "#061514",
                    "#c8fff6",
                    "#5ff0dd",
                    "#13554e",
                    [
                        "#0a1f1e", "#ff5e57", "#6ee07a", "#f2d74e", "#5aa9ff", "#ff9df0", "#2fc4b2", "#c8fff6",
                        "#3f7c75", "#ff8a84", "#9af5a3", "#ffe789", "#8cc6ff", "#ffc2f6", "#5ff0dd", "#ffffff",
                    ],
                ),
            },
            // Balde de Lixo: metal escuro, verde Plankton e o vermelho do olho.
            _ => Theme {
                variant: "balde",
                dark: true,
                bg: c("#0b1110"),
                bg2: c("#111a17"),
                bg3: c("#17231f"),
                bg4: c("#1f2f29"),
                border: c("#263a32"),
                border_active: c("#6fd36a"),
                ink: c("#e3f5e9"),
                dim: c("#86a596"),
                faint: c("#557063"),
                accent: c("#4caf50"),
                accent_hi: c("#7ee07a"),
                accent_lo: c("#1f4d24"),
                ok: c("#7ee07a"),
                warn: c("#f2d74e"),
                err: c("#e0453a"),
                attention: c("#3fd0c9"),
                agents: ["#7ee07a", "#e0453a", "#f2d74e", "#3fd0c9", "#b48cf2", "#ff8a5c", "#5e9be0", "#e07aa8"]
                    .map(c)
                    .to_vec(),
                radius: 6,
                hard_shadow: None,
                term: term_from(
                    "#0b1110",
                    "#e3f5e9",
                    "#7ee07a",
                    "#1f4d24",
                    [
                        "#17231f", "#e0453a", "#7ee07a", "#f2d74e", "#5e9be0", "#b48cf2", "#3fd0c9", "#d6e8dd",
                        "#557063", "#ff6b5e", "#a6f5a1", "#ffe680", "#8bbcff", "#d3b5ff", "#7ff0e9", "#ffffff",
                    ],
                ),
            },
        }
    }

    fn help(&self) -> &'static str {
        include_str!("../../../../skins/plankton/HELP.md")
    }

    fn state_label(&self, s: SessionState) -> &'static str {
        match s {
            SessionState::Boot => "LIGANDO",
            SessionState::Working => "TRAMANDO",
            SessionState::Idle => "À ESPREITA",
            SessionState::Waiting => "KAREN?!",
            SessionState::Done => "MUAHAHA!",
            SessionState::GameOver => "DERROTADO",
        }
    }

    fn mascot(&self, ui: &Ui, rect: Rect, state: SessionState, t: f64, theme: &Theme) {
        if let Some(path) = sprite_for(state) {
            egui::Image::new(format!("file://{}", path.display()))
                .fit_to_exact_size(rect.size())
                .paint_at(ui, rect);
            return;
        }
        draw_plankton(ui, rect, state, t, theme);
    }

    fn background(&self, ui: &Ui, rect: Rect, t: f64, theme: &Theme) {
        bubbles(ui, rect, t, theme.accent.gamma_multiply(0.10));
    }

    /// Letreiro do Balde de Lixo: letras verdes com contorno grosso, sombra deslocada
    /// e o "O" virando o olho do Plankton.
    fn wordmark(&self, ui: &Ui, pos: Pos2, size: f32, theme: &Theme) -> Rect {
        let p = ui.painter();
        let word = "AIC";
        let font = FontId::new(size * 1.1, FontFamily::Monospace);
        let ink = c("#101410");
        let layout = |col: Color32| p.layout_no_wrap(word.into(), font.clone(), col);
        let fill = layout(theme.accent_hi);
        let size_v = fill.size();
        let o = (size * 0.07).max(1.0);
        let shadow = layout(theme.accent_lo);
        p.galley(pos + vec2(o * 2.5, o * 2.5), shadow, theme.accent_lo);
        let outline = layout(ink);
        for d in [vec2(-o, 0.0), vec2(o, 0.0), vec2(0.0, -o), vec2(0.0, o), vec2(-o, -o), vec2(o, o), vec2(-o, o), vec2(o, -o)] {
            p.galley(pos + d, outline.clone(), ink);
        }
        p.galley(pos, fill, theme.accent_hi);
        // o "O" (7ª letra) vira o olho: amarelo, íris vermelha, pupila
        let cw = size_v.x / word.len() as f32;
        let eye_c = pos + vec2(cw * 6.5, size_v.y * 0.52);
        let r = cw * 0.36;
        p.circle_filled(eye_c, r, c("#f4e36b"));
        p.circle_stroke(eye_c, r, Stroke::new(o, ink));
        p.circle_filled(eye_c, r * 0.5, c("#d3302f"));
        p.circle_filled(eye_c, r * 0.2, ink);
        Rect::from_min_size(pos, size_v + vec2(o * 3.0, o * 3.0))
    }
}

/// Sprites do usuário: `~/.local/share/aic/temas/plankton/sprites/<estado>.(gif|png)`.
/// Estados: boot, working, idle, waiting, done, gameover. Qualquer um que faltar cai no desenho.
fn sprite_for(state: SessionState) -> Option<PathBuf> {
    let name = match state {
        SessionState::Boot => "boot",
        SessionState::Working => "working",
        SessionState::Idle => "idle",
        SessionState::Waiting => "waiting",
        SessionState::Done => "done",
        SessionState::GameOver => "gameover",
    };
    let dir = paths::data_dir().join("temas/plankton/sprites");
    ["gif", "png"].iter().map(|ext| dir.join(format!("{name}.{ext}"))).find(|p| p.exists())
}

/// Bolhas subindo da Fenda do Biquíni.
pub fn bubbles(ui: &Ui, rect: Rect, t: f64, color: Color32) {
    let p = ui.painter_at(rect);
    for i in 0..18 {
        let seed = i as f32 * 12.9898;
        let rnd = |k: f32| ((seed * k).sin() * 43758.545).fract().abs();
        let speed = 12.0 + rnd(1.3) * 22.0;
        let x0 = rect.left() + rnd(2.1) * rect.width();
        let travel = rect.height() + 40.0;
        let y = rect.bottom() + 20.0 - ((t as f32 * speed + rnd(3.7) * travel) % travel);
        let x = x0 + ((t as f32 * 0.8 + seed).sin() * 6.0);
        let r = 2.0 + rnd(4.4) * 5.0;
        p.circle_stroke(pos2(x, y), r, Stroke::new(1.0, color));
        p.circle_filled(pos2(x - r * 0.35, y - r * 0.35), r * 0.22, color);
    }
}

/// O Plankton em vetor. Tudo proporcional a `s` (lado do quadrado útil).
pub fn draw_plankton(ui: &Ui, rect: Rect, state: SessionState, t: f64, theme: &Theme) {
    let p = ui.painter_at(rect.expand(4.0));
    let t = t as f32;
    let s = rect.width().min(rect.height());
    let mut center = rect.center();

    let body_green = if state == SessionState::GameOver { c("#56645a") } else { c("#3f8f3a") };
    let body_dark = c("#1f4d1c");
    let eye_white = if state == SessionState::GameOver { c("#b9b9a0") } else { c("#f4e36b") };
    let iris = c("#d3302f");
    let ink = c("#101410");

    // Animações por estado
    let bounce = match state {
        SessionState::Done => -(t * 9.0).sin().abs() * s * 0.08,
        SessionState::Working => (t * 4.0).sin() * s * 0.012,
        SessionState::Boot => (1.0 - (t * 1.5).min(1.0)) * s * 0.3,
        _ => (t * 1.6).sin() * s * 0.01,
    };
    center.y += bounce;
    let tilt: f32 = if state == SessionState::GameOver { 0.35 } else { 0.0 };
    let rot = |v: Vec2| -> Pos2 {
        let (sn, cs) = tilt.sin_cos();
        center + vec2(v.x * cs - v.y * sn, v.x * sn + v.y * cs)
    };

    let body_w = s * 0.24;
    let body_h = s * 0.34;
    let body_c = rot(vec2(0.0, s * 0.02));

    // Pernas
    let leg = Stroke::new((s * 0.022).max(1.0), body_dark);
    for dx in [-0.07, 0.07] {
        let top = rot(vec2(s * dx, s * 0.30));
        let foot = rot(vec2(s * dx * 1.4, s * 0.45));
        p.line_segment([top, foot], leg);
        p.line_segment([foot, foot + vec2(s * dx * 0.5, 0.0)], leg);
    }

    // Braços (esfregando as mãos quando trama; pra cima quando gargalha)
    let arm = Stroke::new((s * 0.02).max(1.0), body_dark);
    for side in [-1.0f32, 1.0] {
        let shoulder = rot(vec2(side * body_w * 0.95, s * 0.06));
        let hand = match state {
            SessionState::Working => rot(vec2(side * s * 0.05 + (t * 10.0).sin() * s * 0.015 * side, s * 0.16)),
            SessionState::Done => rot(vec2(side * s * 0.36, -s * 0.18 + (t * 9.0).sin() * s * 0.03)),
            SessionState::Waiting => rot(vec2(side * s * 0.34, s * 0.0)),
            SessionState::GameOver => rot(vec2(side * s * 0.32, s * 0.30)),
            _ => rot(vec2(side * s * 0.30, s * 0.20 + (t * 1.6 + side).sin() * s * 0.01)),
        };
        let mid = shoulder.lerp(hand, 0.5) + vec2(side * s * 0.04, -s * 0.03);
        p.add(CubicBezierShape::from_points_stroke([shoulder, mid, mid, hand], false, Color32::TRANSPARENT, arm));
        p.circle_filled(hand, s * 0.018, body_dark);
    }

    // Corpo
    p.add(Shape::ellipse_filled(body_c, vec2(body_w, body_h), body_green));
    p.add(Shape::ellipse_stroke(body_c, vec2(body_w, body_h), Stroke::new((s * 0.012).max(1.0), body_dark)));

    // Antenas
    let wob = match state {
        SessionState::Waiting => (t * 14.0).sin() * 0.25,
        SessionState::Working => (t * 6.0).sin() * 0.12,
        _ => (t * 2.0).sin() * 0.06,
    };
    for side in [-1.0f32, 1.0] {
        let base = rot(vec2(side * s * 0.05, -body_h + s * 0.04));
        let ang = -PI / 2.0 + side * (0.35 + wob * side);
        let len = s * 0.20;
        let tip = base + vec2(ang.cos(), ang.sin()) * len;
        let ctrl = base + vec2(side * s * 0.02, -len * 0.6);
        p.add(CubicBezierShape::from_points_stroke(
            [base, ctrl, ctrl, tip],
            false,
            Color32::TRANSPARENT,
            PathStroke::new((s * 0.012).max(1.0), body_dark),
        ));
    }

    // Olho (um só, gigante)
    let eye_c = rot(vec2(0.0, -s * 0.06));
    let eye_r = match state {
        SessionState::Waiting => s * (0.135 + (t * 8.0).sin().abs() * 0.01),
        _ => s * 0.125,
    };
    let blink = state != SessionState::GameOver && ((t % 4.3) < 0.12);
    p.circle_filled(eye_c, eye_r, eye_white);
    p.circle_stroke(eye_c, eye_r, Stroke::new((s * 0.012).max(1.0), body_dark));
    if state == SessionState::GameOver {
        let k = eye_r * 0.55;
        let x = Stroke::new((s * 0.025).max(1.5), ink);
        p.line_segment([eye_c + vec2(-k, -k), eye_c + vec2(k, k)], x);
        p.line_segment([eye_c + vec2(-k, k), eye_c + vec2(k, -k)], x);
    } else if blink {
        p.line_segment(
            [eye_c + vec2(-eye_r, 0.0), eye_c + vec2(eye_r, 0.0)],
            Stroke::new((s * 0.02).max(1.0), body_dark),
        );
    } else {
        // pupila olha em volta quando trama
        let look = match state {
            SessionState::Working => vec2((t * 1.7).sin() * eye_r * 0.35, (t * 1.1).cos() * eye_r * 0.1),
            SessionState::Waiting => vec2(0.0, eye_r * 0.15),
            _ => vec2((t * 0.5).sin() * eye_r * 0.15, 0.0),
        };
        p.circle_filled(eye_c + look, eye_r * 0.52, iris);
        p.circle_filled(eye_c + look, eye_r * 0.22, ink);
        p.circle_filled(eye_c + look + vec2(-eye_r * 0.15, -eye_r * 0.18), eye_r * 0.09, Color32::WHITE);
    }

    // Monocelha (o humor dele mora aqui)
    let brow_y = -s * 0.06 - eye_r - s * 0.03;
    let (l, r) = match state {
        SessionState::Working => (vec2(-eye_r * 1.0, brow_y - s * 0.02), vec2(eye_r * 1.0, brow_y + s * 0.03)),
        SessionState::Waiting => (vec2(-eye_r, brow_y - s * 0.05), vec2(eye_r, brow_y - s * 0.05)),
        SessionState::Done => (vec2(-eye_r, brow_y + s * 0.03), vec2(eye_r, brow_y + s * 0.03)),
        SessionState::GameOver => (vec2(-eye_r, brow_y - s * 0.01), vec2(eye_r, brow_y + s * 0.04)),
        _ => (vec2(-eye_r, brow_y), vec2(eye_r, brow_y)),
    };
    let mid = (l + r) * 0.5 + vec2(0.0, if state == SessionState::Done { s * 0.02 } else { -s * 0.015 });
    p.add(CubicBezierShape::from_points_stroke(
        [rot(l), rot(mid), rot(mid), rot(r)],
        false,
        Color32::TRANSPARENT,
        PathStroke::new((s * 0.04).max(2.0), ink),
    ));

    // Boca
    let mouth_c = rot(vec2(0.0, s * 0.15));
    match state {
        SessionState::Done => {
            let open = 0.5 + (t * 18.0).sin().abs() * 0.5;
            p.add(Shape::ellipse_filled(mouth_c, vec2(s * 0.06, s * 0.035 * open + s * 0.01), ink));
        }
        SessionState::Waiting => {
            p.circle_filled(mouth_c, s * 0.022, ink);
        }
        SessionState::GameOver => {
            p.add(CubicBezierShape::from_points_stroke(
                [mouth_c + vec2(-s * 0.05, s * 0.02), mouth_c + vec2(0.0, -s * 0.03), mouth_c + vec2(0.0, -s * 0.03), mouth_c + vec2(s * 0.05, s * 0.02)],
                false,
                Color32::TRANSPARENT,
                PathStroke::new((s * 0.012).max(1.0), ink),
            ));
        }
        _ => {
            let smirk = if state == SessionState::Working { s * 0.02 } else { 0.0 };
            p.add(CubicBezierShape::from_points_stroke(
                [mouth_c + vec2(-s * 0.05, 0.0), mouth_c + vec2(0.0, s * 0.02 + smirk), mouth_c + vec2(0.0, s * 0.02), mouth_c + vec2(s * 0.05, -smirk)],
                false,
                Color32::TRANSPARENT,
                PathStroke::new((s * 0.012).max(1.0), ink),
            ));
        }
    }

    // Balões
    if s >= 48.0 {
        let font = FontId::new((s * 0.11).max(9.0), FontFamily::Monospace);
        match state {
            SessionState::Done => {
                let a = ((t * 3.0).sin() * 0.5 + 0.5) * 0.6 + 0.4;
                p.text(rect.center_top() + vec2(s * 0.28, s * 0.08), Align2::CENTER_CENTER, "HA HA!", font, theme.warn.gamma_multiply(a));
            }
            SessionState::Waiting => {
                p.text(rect.center_top() + vec2(s * 0.30, s * 0.10), Align2::CENTER_CENTER, "!", FontId::new(s * 0.2, FontFamily::Monospace), theme.attention);
            }
            SessionState::Working => {
                let dots = ".".repeat(((t * 3.0) as usize % 3) + 1);
                p.text(rect.center_top() + vec2(s * 0.30, s * 0.10), Align2::LEFT_CENTER, dots, font, theme.dim);
            }
            SessionState::GameOver => {
                p.text(rect.center_top() + vec2(s * 0.30, s * 0.10), Align2::CENTER_CENTER, "z", font, theme.dim);
            }
            _ => {}
        }
    }
}
