//! Identidade visual do AIC: cores, mascote (o pote de moscas), fundo de película, letreiro
//! e nomes dos estados. Inspirado no grunge de Seattle; nada de logos ou capas oficiais.

use std::f32::consts::PI;

use egui::epaint::TextShape;
use egui::{Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, Ui, Vec2, pos2, vec2};
use aic_core::SessionState;

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
        agents: ["#b5522b", "#8a9a4e", "#c9a227", "#9b7bb0", "#5f8b99", "#b0362b", "#a08a70"].map(c).to_vec(),
        radius: 2,
        hard_shadow: Some(2.0),
        term: term_from(
            "#110e0b",
            "#e8dac4",
            "#d06a3d",
            "#4a2414",
            [
                "#241d16", "#b0362b", "#8a9a4e", "#c9a227", "#5f8b99", "#9b7bb0", "#6f9a88", "#d6c8b0",
                "#65574a", "#d04a3c", "#a6b862", "#e3bc3c", "#7eaabb", "#b899cc", "#8bb8a4", "#f6ead6",
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

/// Mascote dentro de `rect` (t = segundos desde o início, pra animar).
pub fn mascot(ui: &Ui, rect: Rect, state: SessionState, t: f64, theme: &Theme) {
    draw_jar(ui, rect, state, t as f32, theme);
}

/// Efeito de fundo, desenhado atrás de tudo.
pub fn background(ui: &Ui, rect: Rect, t: f64, theme: &Theme) {
    film(ui, rect, t as f32, theme);
}

/// "AIC" com cara de cartaz xerocado: letras tortas, impressão desalinhada e riscos.
pub fn wordmark(ui: &Ui, pos: Pos2, size: f32, theme: &Theme) -> Rect {
    let p = ui.painter();
    let font = FontId::new(size * 1.3, FontFamily::Monospace);
    let letters = [('A', -0.09f32, 1.0f32), ('I', 0.06, -1.5), ('C', -0.04, 0.5)];
    let mut x = pos.x;
    let mut h: f32 = 0.0;
    let ghost = theme.err.gamma_multiply(0.55);
    for (ch, ang, dy) in letters {
        let g = p.layout_no_wrap(ch.to_string(), font.clone(), theme.ink);
        let w = g.size().x;
        h = h.max(g.size().y);
        let at = pos2(x, pos.y + dy);
        // impressão fantasma desalinhada
        p.add(TextShape::new(at + vec2(1.5, 1.0), g.clone(), ghost).with_override_text_color(ghost).with_angle(ang * 1.4));
        // bold "na mão": a mesma letra 3x com meio pixel de diferença
        for d in [vec2(0.0, 0.0), vec2(0.6, 0.0), vec2(0.0, 0.5)] {
            p.add(TextShape::new(at + d, g.clone(), theme.accent_hi).with_override_text_color(theme.accent_hi).with_angle(ang));
        }
        x += w * 1.02;
    }
    let r = Rect::from_min_size(pos, vec2(x - pos.x, h));
    // riscos e falhas de tinta (na cor do fundo)
    let scratch = Stroke::new(1.0, theme.bg);
    p.line_segment([pos2(r.left() - 1.0, r.top() + h * 0.42), pos2(r.right() + 1.0, r.top() + h * 0.36)], scratch);
    p.line_segment([pos2(r.left() + r.width() * 0.3, r.top() + h * 0.75), pos2(r.right() - 2.0, r.top() + h * 0.80)], Stroke::new(0.8, theme.bg));
    for i in 0..14 {
        let fx = rnd(i as f32 * 3.3 + 0.7);
        let fy = 0.2 + 0.65 * rnd(i as f32 * 5.1 + 0.2);
        p.circle_filled(pos2(r.left() + fx * r.width(), r.top() + fy * h), 0.5 + rnd(i as f32 * 1.7) * 0.6, theme.bg);
    }
    r
}

// ───────────────────────── o pote ─────────────────────────

/// Elipse rotacionada como polígono convexo.
fn ellipse(c: Pos2, rx: f32, ry: f32, ang: f32) -> Vec<Pos2> {
    let (sn, cs) = ang.sin_cos();
    (0..14)
        .map(|i| {
            let a = i as f32 / 14.0 * PI * 2.0;
            let (x, y) = (a.cos() * rx, a.sin() * ry);
            c + vec2(x * cs - y * sn, x * sn + y * cs)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
/// Uma mosca. `ang` = direção do voo; `wing` 0..1 = batida da asa; `dead` = de barriga pra cima.
fn fly(p: &Painter, c: Pos2, fs: f32, ang: f32, wing: f32, dead: bool, body: Color32, alpha: f32) {
    let dir = vec2(ang.cos(), ang.sin());
    let side = vec2(-dir.y, dir.x);
    if dead {
        // pernas pra cima
        let leg = Stroke::new((fs * 0.22).max(0.7), body.gamma_multiply(alpha));
        for k in [-0.5f32, 0.0, 0.5] {
            let base = c + dir * (k * fs);
            p.line_segment([base, base + vec2(k * fs * 0.4, -fs * 0.9)], leg);
        }
        p.add(Shape::convex_polygon(ellipse(c, fs * 0.95, fs * 0.55, ang), body.gamma_multiply(alpha), Stroke::NONE));
        p.add(Shape::convex_polygon(ellipse(c + vec2(0.0, fs * 0.15), fs * 0.6, fs * 0.25, ang), Color32::from_white_alpha((90.0 * alpha) as u8), Stroke::NONE));
        return;
    }
    // asas (atrás do corpo)
    let spread = 0.35 + 0.65 * wing;
    let wing_c = Color32::from_white_alpha((120.0 * alpha) as u8);
    for sgn in [-1.0f32, 1.0] {
        let wc = c - dir * (fs * 0.35) + side * (sgn * fs * 0.55 * spread);
        let wa = ang + sgn * (0.6 + 0.7 * spread);
        p.add(Shape::convex_polygon(ellipse(wc, fs * 0.75, fs * 0.38, wa), wing_c, Stroke::NONE));
    }
    p.add(Shape::convex_polygon(ellipse(c, fs * 0.8, fs * 0.5, ang), body.gamma_multiply(alpha), Stroke::NONE));
    p.circle_filled(c + dir * (fs * 0.75), fs * 0.38, body.gamma_multiply(alpha));
    if fs >= 3.0 {
        // olhos vermelhos de mosca
        let eye = Color32::from_rgb(140, 40, 30).gamma_multiply(alpha);
        p.circle_filled(c + dir * (fs * 0.9) + side * (fs * 0.22), fs * 0.16, eye);
        p.circle_filled(c + dir * (fs * 0.9) - side * (fs * 0.22), fs * 0.16, eye);
    }
}

/// O pote de vidro com moscas, inspirado no clima do "Jar of Flies" (desenho original).
pub fn draw_jar(ui: &Ui, rect: Rect, state: SessionState, t: f32, theme: &Theme) {
    use SessionState::*;
    let s = rect.width().min(rect.height());
    let p = ui.painter_at(rect.expand(4.0));
    let mut c = rect.center() + vec2(0.0, s * 0.03);

    // BOOT: o pote sobe do chão e aparece
    let mut alpha = 1.0;
    if state == Boot {
        let k = ((t % 3.0) / 1.4).min(1.0);
        let e = 1.0 - (1.0 - k).powi(3);
        c.y += (1.0 - e) * s * 0.35;
        alpha = e;
    }
    // WAITING: o pote treme quando elas batem no vidro
    if state == Waiting {
        c.x += ((t * 37.0).sin() * (t * 3.1).sin().max(0.0)) * s * 0.012;
    }

    let dark = theme.dark;
    let amber = c_("#d9b878");
    let interior = if dark { mix(theme.bg, amber, if state == GameOver { 0.28 } else { 0.5 }) } else { mix(theme.bg, amber, if state == GameOver { 0.15 } else { 0.32 }) };
    let edge = if dark { c_("#efe2c8") } else { c_("#4a3a28") };
    let rust = theme.accent;
    let rust_dk = mix(rust, Color32::BLACK, 0.4);
    let fly_c = c_("#15100b");
    let lw = (s * 0.022).max(1.0);

    let at = |x: f32, y: f32| c + vec2(x * s, y * s);
    let body = Rect::from_min_max(at(-0.29, -0.22), at(0.29, 0.40));
    let neck = Rect::from_min_max(at(-0.21, -0.31), at(0.21, -0.19));

    // sombra no chão
    p.add(Shape::ellipse_filled(at(0.0, 0.42), vec2(s * 0.32, s * 0.035), Color32::from_black_alpha((70.0 * alpha) as u8)));

    // vidro: corpo + pescoço
    let rad = (s * 0.09).min(255.0) as u8;
    p.rect_filled(neck, (s * 0.03) as u8, interior.gamma_multiply(alpha));
    p.rect_filled(body, rad, interior.gamma_multiply(alpha));
    // luz entrando pelo topo (faixa mais clara)
    let glow = Rect::from_min_max(at(-0.27, -0.20), at(0.27, 0.0));
    p.rect_filled(glow, rad, amber.gamma_multiply(0.10 * alpha));

    // moscas (dentro do vidro)
    let fs = (s * 0.046).max(1.4);
    let inner = Rect::from_min_max(at(-0.23, -0.14), at(0.23, 0.33));
    let map = |u: f32, v: f32| pos2(inner.center().x + u * inner.width() * 0.5, inner.center().y + v * inner.height() * 0.5);
    match state {
        GameOver => {
            for (i, u) in [-0.55f32, 0.1, 0.62].into_iter().enumerate() {
                let ang = 0.15 * (i as f32 - 1.0);
                fly(&p, pos2(map(u, 0.0).x, inner.bottom() + s * 0.025), fs, ang, 0.0, true, fly_c, alpha);
            }
        }
        _ => {
            let (n, speed) = match state {
                Working => (5, 2.6),
                Idle => (3, 0.45),
                Waiting => (4, 1.0),
                Done => (3, 1.6),
                _ => (4, 0.8),
            };
            for i in 0..n {
                let fi = i as f32;
                let ph = fi * 2.17 + 0.4;
                let (mut u, mut v, ang);
                if state == Waiting {
                    // dispara contra o vidro e quica (onda "dente de serra" afiada na parede)
                    let side = if i % 2 == 0 { 1.0 } else { -1.0 };
                    let k = (t * (1.1 + fi * 0.17) + ph).fract();
                    let hit = 1.0 - (1.0 - k).powi(3); // acelera até a parede
                    u = side * (0.15 + 0.82 * hit);
                    v = ((fi * 0.9 + t * 0.3).sin()) * 0.6;
                    ang = if side > 0.0 { 0.0 } else { PI };
                    if k > 0.86 && s >= 30.0 {
                        // "tic" no vidro
                        let wx = if side > 0.0 { body.right() } else { body.left() };
                        let wy = map(0.0, v).y;
                        let tick = Stroke::new(lw * 0.8, edge.gamma_multiply(0.8 * alpha));
                        for d in [-1.0f32, 0.0, 1.0] {
                            let a = wx + side * s * 0.02;
                            p.line_segment([pos2(a, wy + d * s * 0.025), pos2(a + side * s * 0.035, wy + d * s * 0.045)], tick);
                        }
                    }
                } else {
                    let tt = t * speed;
                    u = (tt * (1.0 + fi * 0.31) + ph).sin() * 0.8;
                    v = (tt * (1.3 + fi * 0.23) + ph * 1.7).cos() * 0.75;
                    if state == Working {
                        u += (t * 23.0 + fi).sin() * 0.04;
                        v += (t * 19.0 + fi * 2.0).cos() * 0.04;
                    }
                    if state == Idle && i == 2 {
                        // uma pousada no vidro, só mexendo as asas de vez em quando
                        u = 0.9;
                        v = 0.25 + ((t * 0.7).sin() > 0.95) as i32 as f32 * 0.03;
                    }
                    let du = (tt * (1.0 + fi * 0.31) + ph).cos() * (1.0 + fi * 0.31);
                    let dv = -(tt * (1.3 + fi * 0.23) + ph * 1.7).sin() * (1.3 + fi * 0.23);
                    ang = dv.atan2(du);
                }
                if state == Done {
                    // saem voando pela boca do pote
                    let k = (t * 0.55 + fi * 0.33).fract();
                    let base = map(u * (1.0 - k), v);
                    let pos = pos2(base.x + (t * 6.0 + fi).sin() * s * 0.03, base.y - k * s * 0.75);
                    let fade = (1.0 - k).min(1.0) * alpha;
                    fly(&p, pos, fs, -PI / 2.0 + (t * 8.0 + fi).sin() * 0.5, ((t * 30.0) as usize % 2) as f32, false, fly_c, fade);
                    continue;
                }
                let pos = map(u.clamp(-0.97, 0.97), v.clamp(-0.97, 0.97));
                let wing_rate = match state {
                    Working => 40.0,
                    Waiting => 30.0,
                    Idle => 8.0,
                    _ => 16.0,
                };
                let wing = if state == Idle && i == 2 { 0.0 } else { ((t * wing_rate + fi) as usize % 2) as f32 };
                if state == Working && s >= 30.0 {
                    // rastro de zumbido
                    let back = pos - vec2(ang.cos(), ang.sin()) * fs * 3.0;
                    p.line_segment([back, pos], Stroke::new((fs * 0.35).max(0.6), fly_c.gamma_multiply(0.25 * alpha)));
                }
                fly(&p, pos, fs, ang, wing, false, fly_c, alpha);
            }
        }
    }

    // contorno do vidro e reflexos
    p.rect_stroke(body, rad, Stroke::new(lw, edge.gamma_multiply(0.85 * alpha)), egui::StrokeKind::Middle);
    p.rect_stroke(neck, (s * 0.03) as u8, Stroke::new(lw, edge.gamma_multiply(0.85 * alpha)), egui::StrokeKind::Middle);
    let shine = Color32::from_white_alpha((if dark { 70.0 } else { 110.0 } * alpha) as u8);
    p.rect_filled(Rect::from_min_max(at(-0.235, -0.15), at(-0.195, 0.30)), (s * 0.02) as u8, shine);
    if s >= 30.0 {
        p.rect_filled(Rect::from_min_max(at(0.19, -0.12), at(0.21, 0.02)), (s * 0.01) as u8, shine);
    }
    // rachadura no GAME OVER
    if state == GameOver && s >= 30.0 {
        let crack = Stroke::new((lw * 0.7).max(0.8), edge.gamma_multiply(0.8));
        p.add(Shape::line(vec![at(0.12, -0.20), at(0.07, -0.10), at(0.13, -0.03), at(0.06, 0.07)], crack));
        p.line_segment([at(0.07, -0.10), at(0.01, -0.07)], crack);
    }
    // rótulo de papel rasgado (só no grande)
    if s >= 60.0 {
        let paper = if dark { c_("#d8c7a4") } else { c_("#f1e6cc") };
        let lab = [at(-0.15, 0.19), at(0.15, 0.18), at(0.145, 0.30), at(0.03, 0.31), at(0.0, 0.298), at(-0.14, 0.307)];
        p.add(Shape::convex_polygon(lab.to_vec(), paper.gamma_multiply(0.92 * alpha), Stroke::new(1.0, rust_dk.gamma_multiply(0.5 * alpha))));
        // rabisco à mão
        let ink = Stroke::new((s * 0.012).max(1.0), rust_dk.gamma_multiply(alpha));
        let pts: Vec<Pos2> = (0..16).map(|i| at(-0.11 + i as f32 * 0.014, 0.225 + ((i as f32 * 0.9).sin() * 0.012))).collect();
        p.add(Shape::line(pts, ink));
        p.line_segment([at(-0.09, 0.268), at(0.07, 0.265)], Stroke::new((s * 0.008).max(1.0), rust_dk.gamma_multiply(0.7 * alpha)));
    }

    // tampa de metal enferrujada (salta no DONE)
    let (mut lid_dy, mut lid_ang) = (0.0, 0.0);
    if state == Done {
        let k = (t * 1.3).fract();
        let hop = (k * PI).sin().max(0.0);
        lid_dy = -hop * s * 0.14;
        lid_ang = hop * 0.35 * if ((t * 1.3) as usize).is_multiple_of(2) { 1.0 } else { -1.0 };
    }
    let lid_c = at(0.0, -0.355) + vec2(0.0, lid_dy);
    let (sn, cs) = f32::sin_cos(lid_ang);
    let rot = |v: Vec2| lid_c + vec2(v.x * cs - v.y * sn, v.x * sn + v.y * cs);
    let (hw, hh) = (s * 0.25, s * 0.055);
    let corners = [rot(vec2(-hw, -hh)), rot(vec2(hw, -hh)), rot(vec2(hw, hh)), rot(vec2(-hw, hh))];
    p.add(Shape::convex_polygon(corners.to_vec(), rust.gamma_multiply(alpha), Stroke::new(lw * 0.8, rust_dk.gamma_multiply(alpha))));
    if s >= 30.0 {
        let n = if s >= 100.0 { 9 } else { 5 };
        for i in 1..n {
            let x = -hw + 2.0 * hw * i as f32 / n as f32;
            p.line_segment([rot(vec2(x, -hh * 0.6)), rot(vec2(x, hh * 0.7))], Stroke::new((s * 0.008).max(0.8), rust_dk.gamma_multiply(0.8 * alpha)));
        }
        // manchas de ferrugem
        for i in 0..3 {
            let x = (rnd(i as f32 + 0.3) - 0.5) * 1.6 * hw;
            p.circle_filled(rot(vec2(x, (rnd(i as f32 + 4.0) - 0.5) * hh)), s * 0.012, c_("#5a2a12").gamma_multiply(0.8 * alpha));
        }
    }
    // furinhos no topo da tampa (pra elas respirarem)
    if s >= 60.0 && state != Done {
        for i in -1..=1 {
            p.circle_filled(rot(vec2(i as f32 * hw * 0.35, -hh)), s * 0.008, rust_dk.gamma_multiply(alpha));
        }
    }
}

fn c_(h: &str) -> Color32 {
    c(h)
}

// ───────────────────────── granulação de filme ─────────────────────────

/// Fundo: grão de filme (troca 12x por segundo), poeira, riscos verticais de película
/// e dois raios de sol empoeirados entrando pelo canto.
pub fn film(ui: &Ui, rect: Rect, t: f32, theme: &Theme) {
    let p = ui.painter_at(rect);
    let warm = if theme.dark { c_("#e8c88a") } else { c_("#fff3d6") };

    // raios de sol (faixas diagonais bem fracas, respirando)
    for (i, (x0, w, a)) in [(0.10f32, 0.10f32, 0.035f32), (0.26, 0.06, 0.028), (0.38, 0.13, 0.02)].into_iter().enumerate() {
        let breathe = 0.75 + 0.25 * (t * 0.35 + i as f32 * 1.9).sin();
        let a = a * breathe * if theme.dark { 1.0 } else { 1.8 };
        let top_l = pos2(rect.left() + rect.width() * x0, rect.top());
        let top_r = top_l + vec2(rect.width() * w, 0.0);
        let drop = rect.height() * 1.1;
        let pts = vec![top_l, top_r, top_r + vec2(drop * 0.55 + rect.width() * w * 0.6, drop), top_l + vec2(drop * 0.55, drop)];
        p.add(Shape::convex_polygon(pts, warm.gamma_multiply(a), Stroke::NONE));
    }
    // ciscos flutuando na luz
    let motes = ((rect.width() * rect.height()) / 30_000.0).clamp(6.0, 60.0) as usize;
    for i in 0..motes {
        let sd = i as f32 + 0.37;
        let x = rect.left() + (rnd(sd * 1.3) * rect.width() * 0.75 + (t * (2.0 + rnd(sd) * 4.0))).rem_euclid(rect.width());
        let y = rect.top() + (rnd(sd * 2.9) * rect.height() + t * (3.0 + rnd(sd * 4.1) * 5.0)).rem_euclid(rect.height());
        let tw = 0.5 + 0.5 * (t * 1.3 + sd * 3.0).sin();
        p.circle_filled(pos2(x + (t * 0.7 + sd).sin() * 4.0, y), 0.6 + rnd(sd * 7.0) * 0.9, warm.gamma_multiply(0.10 + 0.14 * tw));
    }

    // grão: muda por quadro, como película
    let frame = (t * 12.0).floor();
    let grain = if theme.dark { theme.ink } else { theme.ink.gamma_multiply(0.8) };
    let n = ((rect.width() * rect.height()) / 2_200.0).clamp(20.0, 700.0) as usize;
    for i in 0..n {
        let sd = i as f32 * 1.618 + frame * 0.731;
        let x = rect.left() + rnd(sd) * rect.width();
        let y = rect.top() + rnd(sd * 1.77 + 3.0) * rect.height();
        let a = 0.03 + rnd(sd * 2.31) * 0.06;
        let sz = if rnd(sd * 5.5) > 0.9 { 2.0 } else { 1.0 };
        p.rect_filled(Rect::from_min_size(pos2(x.round(), y.round()), vec2(sz, sz)), 0.0, grain.gamma_multiply(a));
    }
    // poeira grossa (fica alguns quadros)
    let slow = (t * 3.0).floor();
    for i in 0..((rect.width() / 260.0).ceil() as usize).clamp(1, 8) {
        let sd = i as f32 * 9.13 + slow * 0.913;
        if rnd(sd * 0.7) > 0.55 {
            continue;
        }
        let c0 = pos2(rect.left() + rnd(sd) * rect.width(), rect.top() + rnd(sd + 1.3) * rect.height());
        let pts: Vec<Pos2> = (0..4).map(|k| c0 + vec2(k as f32 * 1.6, (k as f32 * 2.1 + sd).sin() * 1.5)).collect();
        p.add(Shape::line(pts, Stroke::new(1.0, grain.gamma_multiply(0.10))));
    }
    // risco vertical de película: aparece por uns quadros de vez em quando
    let slot = (t / 2.3).floor();
    if rnd(slot * 1.31 + 0.5) > 0.4 && (t / 2.3).fract() < 0.18 {
        let x = rect.left() + rnd(slot * 7.7) * rect.width() + (t * 40.0).sin() * 2.0;
        p.line_segment([pos2(x, rect.top()), pos2(x + 3.0, rect.bottom())], Stroke::new(1.0, grain.gamma_multiply(0.07)));
    }
}

/// Mistura linear de cores.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Hash barato e determinístico em [0, 1).
pub fn rnd(seed: f32) -> f32 {
    ((seed * 12.9898).sin() * 43758.545).fract().abs()
}
