//! O Anakin no canto da tela: um boneco em pixel art (desenhado aqui, sem imagem de filme) que
//! anda de vez em quando, pisca, pula quando uma sessão acaba, acende o sabre enquanto alguém
//! trabalha e solta falas num balão. Clicar abre a conversa.
//!
//! Custo: parado ele não pede frame nenhum. Só anima em surtos curtos, com a janela em foco.

use std::time::{Duration, Instant};

use aic_core::SessionState;
use egui::{Color32, Context, FontId, Id, Order, Pos2, Rect, Sense, Stroke, pos2, vec2};

use crate::actions::Action;
use crate::app::App;
use crate::theme::c;

const PX: f32 = 3.0;
const COLS: usize = 16;
const ROWS: usize = 21;
pub const WIDTH: f32 = COLS as f32 * PX;
pub const HEIGHT: f32 = ROWS as f32 * PX;
const WALK_SPEED: f32 = 26.0;
const WALK_RANGE: f32 = 110.0;
/// Quadros por segundo enquanto anima.
const ANIM_FRAME: Duration = Duration::from_millis(125);

const FALAS: [&str; 5] = [
    "This is where the fun begins.",
    "Now this is podracing!",
    "I don't like sand.",
    "Are you an angel?",
    "I've got a bad feeling about this.",
];

const HEAD: [&str; 9] = [
    ".....hhhhhh.....",
    "...hhHHHHHHhh...",
    "..hHHHHHHHHHHh..",
    "..hHHhhsshhHHh..",
    "..hHhsssssshHh..",
    "..hhsesssessh...",
    "..hhssssssssh...",
    "...h.ssnnss.h...",
    ".....ssss.......",
];
const HEAD_BLINK_ROW: &str = "..hhsxsssxssh...";
const BODY: [&str; 7] = [
    "...ddbttttbdd...",
    "..dddbttttbddd..",
    "..dbdbbttbbdbd..",
    "..dbddBBBBddbd..",
    "..sb.dBggBd.bs..",
    "..g..dddddd..g..",
    ".....dddddd.....",
];
const LEGS_STAND: [&str; 5] = [
    ".....dd..dd.....",
    ".....pp..pp.....",
    ".....pp..pp.....",
    ".....pp..pp.....",
    "....kkk..kkk....",
];
const LEGS_WALK_A: [&str; 5] = [
    ".....dd..dd.....",
    "....pp....pp....",
    "....pp....pp....",
    "...kpp.....pp...",
    "...kk.....kkk...",
];
const LEGS_WALK_B: [&str; 5] = [
    ".....dd..dd.....",
    ".....pp..pp.....",
    "......pppp......",
    "......pp.pp.....",
    ".....kkk.kkk....",
];

fn color(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'h' => c("#5b3d24"),
        b'H' => c("#83592f"),
        b's' => c("#e6b78f"),
        b'e' => c("#1c2430"),
        b'x' => c("#c99a73"),
        b'n' => c("#b5714f"),
        b'd' => c("#33261d"),
        b'b' => c("#5e412a"),
        b't' => c("#bfa27a"),
        b'B' => c("#3d2c1d"),
        b'g' => c("#15171c"),
        b'p' => c("#3a2f26"),
        b'k' => c("#20242b"),
        _ => return None,
    })
}

pub struct AvatarState {
    /// Quanto andou pra esquerda a partir do canto.
    x: f32,
    dir: f32,
    walk_until: Option<Instant>,
    blink_until: Option<Instant>,
    jump_until: Option<Instant>,
    next_idle: Instant,
    say: Option<(String, Instant)>,
    seq: u32,
    last: Instant,
    greeted: bool,
}

impl Default for AvatarState {
    fn default() -> Self {
        Self {
            x: 0.0,
            dir: 1.0,
            walk_until: None,
            blink_until: None,
            jump_until: None,
            next_idle: Instant::now() + Duration::from_secs(8),
            say: None,
            seq: 0,
            last: Instant::now(),
            greeted: false,
        }
    }
}

impl AvatarState {
    /// Fala num balão por `secs`.
    pub fn speak(&mut self, text: impl Into<String>, secs: u64) {
        self.say = Some((text.into(), Instant::now() + Duration::from_secs(secs)));
    }

    /// Comemora: pula.
    pub fn jump(&mut self) {
        self.jump_until = Some(Instant::now() + Duration::from_millis(900));
    }

    pub fn speech(&self) -> Option<&str> {
        self.say.as_ref().map(|(s, _)| s.as_str())
    }

    /// Avança a animação. Devolve `true` enquanto tem movimento (pede o próximo quadro).
    fn step(&mut self, now: Instant, animate: bool) -> bool {
        let dt = now.duration_since(self.last).as_secs_f32().min(0.25);
        self.last = now;
        if self.say.as_ref().is_some_and(|(_, t)| *t <= now) {
            self.say = None;
        }
        for t in [
            &mut self.walk_until,
            &mut self.blink_until,
            &mut self.jump_until,
        ] {
            if t.is_some_and(|t| t <= now) {
                *t = None;
            }
        }
        if !animate {
            self.walk_until = None;
            return false;
        }
        if now >= self.next_idle {
            self.seq = self.seq.wrapping_add(1);
            // intervalo que varia sem precisar de um gerador aleatório
            let gap = 6 + (self.seq.wrapping_mul(2654435761) >> 28) as u64;
            self.next_idle = now + Duration::from_secs(gap);
            match self.seq % 4 {
                1 | 3 => self.blink_until = Some(now + Duration::from_millis(180)),
                2 => {
                    self.walk_until =
                        Some(now + Duration::from_millis(1800 + 700 * (self.seq % 3) as u64))
                }
                _ => {
                    if self.seq % 24 == 0 && self.say.is_none() {
                        let i = (self.seq / 24) as usize % FALAS.len();
                        self.speak(FALAS[i], 5);
                    }
                }
            }
        }
        if self.walk_until.is_some() {
            self.x += self.dir * WALK_SPEED * dt;
            if self.x >= WALK_RANGE {
                self.dir = -1.0;
            } else if self.x <= 0.0 {
                self.x = 0.0;
                self.dir = 1.0;
            }
        }
        self.walking() || self.blink_until.is_some() || self.jump_until.is_some()
    }

    fn walking(&self) -> bool {
        self.walk_until.is_some()
    }
}

/// Cada linha do boneco, de cima pra baixo, no quadro `t`.
fn rows(blink: bool, legs: &'static [&'static str; 5]) -> impl Iterator<Item = &'static str> {
    HEAD.iter()
        .enumerate()
        .map(move |(i, r)| if blink && i == 5 { HEAD_BLINK_ROW } else { *r })
        .chain(BODY.iter().copied())
        .chain(legs.iter().copied())
}

/// Desenha o boneco com uma retângulo por trecho de mesma cor.
fn paint_sprite(
    p: &egui::Painter,
    pos: Pos2,
    blink: bool,
    legs: &'static [&'static str; 5],
    flip: bool,
) {
    for (y, row) in rows(blink, legs).enumerate() {
        let b = row.as_bytes();
        let mut x = 0;
        while x < b.len() {
            let ch = b[x];
            let mut end = x + 1;
            while end < b.len() && b[end] == ch {
                end += 1;
            }
            if let Some(col) = color(ch) {
                let (a, z) = if flip {
                    (COLS - end, COLS - x)
                } else {
                    (x, end)
                };
                p.rect_filled(
                    Rect::from_min_size(
                        pos + vec2(a as f32 * PX, y as f32 * PX),
                        vec2((z - a) as f32 * PX, PX),
                    ),
                    0.0,
                    col,
                );
            }
            x = end;
        }
    }
}

/// O sabre aceso na mão, com o mesmo brilho do azul do tema.
fn paint_saber(p: &egui::Painter, hand: Pos2, accent: Color32) {
    let tip = hand - vec2(0.0, 44.0);
    p.rect_filled(
        Rect::from_min_max(pos2(hand.x - 3.5, tip.y), pos2(hand.x + 3.5, hand.y)),
        3.0,
        accent.gamma_multiply(0.25),
    );
    p.rect_filled(
        Rect::from_min_max(pos2(hand.x - 1.6, tip.y), pos2(hand.x + 1.6, hand.y)),
        1.5,
        accent,
    );
    p.rect_filled(
        Rect::from_min_max(pos2(hand.x - 0.7, tip.y), pos2(hand.x + 0.7, hand.y)),
        0.0,
        c("#eaf6ff"),
    );
    p.rect_filled(
        Rect::from_center_size(hand + vec2(0.0, 3.0), vec2(4.0, 8.0)),
        0.0,
        c("#9aa7b8"),
    );
}

/// Alguém trabalhando agora? (o sabre fica aceso)
fn anyone_working(app: &App) -> bool {
    app.anakin.state() == SessionState::Working
        || app
            .sessions
            .values()
            .any(|r| r.state() == SessionState::Working)
}

pub fn show(app: &mut App, ctx: &Context) {
    if !app.cfg.anakin.avatar {
        return;
    }
    if !app.anakin.avatar.greeted {
        app.anakin.avatar.greeted = true;
        app.anakin.avatar.speak("This is where the fun begins.", 4);
    }
    let now = Instant::now();
    let moving = app
        .anakin
        .avatar
        .step(now, app.window_focused && !app.anakin.open);
    let screen = ctx.content_rect();
    let av = &app.anakin.avatar;
    let top_left = pos2(
        screen.right() - 14.0 - WIDTH - av.x,
        screen.bottom() - 8.0 - HEIGHT,
    );
    let hop = av.jump_until.map_or(0.0, |t| {
        let left = t.duration_since(now).as_secs_f32();
        (left / 0.3 * std::f32::consts::PI).sin().abs() * 12.0
    });
    let legs: &'static [&'static str; 5] = if av.walking() {
        let frame = now.duration_since(av.last).as_millis() as f32 + av.x;
        if (frame / 7.0) as u32 % 2 == 0 {
            &LEGS_WALK_A
        } else {
            &LEGS_WALK_B
        }
    } else {
        &LEGS_STAND
    };
    let blink = av.blink_until.is_some();
    let flip = av.dir > 0.0;
    let speech = av.speech().map(str::to_owned).filter(|_| !app.anakin.open);
    let working = anyone_working(app);
    let theme = app.theme.clone();

    let mut clicked = false;
    let resp = egui::Area::new(Id::new("anakin-avatar"))
        .order(Order::Middle)
        .fixed_pos(top_left)
        .constrain(false)
        .show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(vec2(WIDTH, HEIGHT), Sense::click());
            resp.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Anakin")
            });
            let at = rect.min - vec2(0.0, hop);
            paint_sprite(ui.painter(), at, blink, legs, flip);
            if working {
                let hand_x = if flip { 2.5 } else { 13.5 };
                paint_saber(
                    ui.painter(),
                    at + vec2(hand_x * PX, 15.0 * PX),
                    theme.accent,
                );
            }
            clicked = resp.clicked();
            resp
        })
        .inner;
    let resp = resp.on_hover_text(app.keymap.tip(
        Action::ToggleAnakin,
        if app.anakin.open {
            "Fechar a conversa com o Anakin"
        } else {
            "Falar com o Anakin"
        },
    ));
    resp.context_menu(|ui| {
        let ctx = ui.ctx().clone();
        for (label, a) in [
            ("Falar por voz", Action::AnakinVoice),
            ("Reiniciar a conversa", Action::RestartAnakin),
            ("Esconder o boneco", Action::ToggleAvatar),
        ] {
            let keys = app.keymap.keys_of(a);
            if ui.button(format!("{label}  ({keys})")).clicked() {
                app.run_action(&ctx, a);
                ui.close();
            }
        }
    });
    if clicked {
        crate::anakin::toggle(app, ctx);
    }

    if let Some(text) = speech {
        bubble(ctx, &theme, top_left - vec2(0.0, hop), &text);
    }
    if moving {
        ctx.request_repaint_after(ANIM_FRAME);
    }
}

fn bubble(ctx: &Context, theme: &crate::theme::Theme, sprite: Pos2, text: &str) {
    let galley = ctx.fonts_mut(|f| {
        f.layout(
            text.to_string(),
            FontId::proportional(13.0),
            theme.ink,
            300.0,
        )
    });
    let size = galley.size() + vec2(20.0, 14.0);
    let right = sprite.x + WIDTH;
    let bottom = sprite.y - 8.0;
    let rect = Rect::from_min_size(pos2(right - size.x, bottom - size.y), size);
    egui::Area::new(Id::new("anakin-fala"))
        .order(Order::Middle)
        .fixed_pos(rect.min)
        .interactable(false)
        .constrain(false)
        .show(ctx, |ui| {
            ui.allocate_exact_size(size, Sense::hover());
            let p = ui.painter();
            p.rect(
                rect,
                6.0,
                theme.bg3,
                Stroke::new(1.0, theme.accent),
                egui::StrokeKind::Inside,
            );
            let tail = pos2(right - WIDTH / 2.0, rect.bottom());
            p.add(egui::Shape::convex_polygon(
                vec![
                    tail + vec2(-5.0, -1.0),
                    tail + vec2(5.0, -1.0),
                    tail + vec2(0.0, 6.0),
                ],
                theme.bg3,
                Stroke::NONE,
            ));
            p.galley(rect.min + vec2(10.0, 7.0), galley, theme.ink);
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todas_as_linhas_tem_a_largura_certa() {
        for legs in [&LEGS_STAND, &LEGS_WALK_A, &LEGS_WALK_B] {
            let v: Vec<_> = rows(false, legs).collect();
            assert_eq!(v.len(), ROWS);
            for r in v {
                assert_eq!(r.len(), COLS, "{r}");
            }
        }
        assert_eq!(HEAD_BLINK_ROW.len(), COLS);
    }

    #[test]
    fn parado_nao_pede_frame() {
        let mut a = AvatarState::default();
        let now = Instant::now();
        assert!(!a.step(now, true), "sem nada acontecendo, nenhum quadro");
        a.jump();
        assert!(a.step(now + Duration::from_millis(100), true));
        assert!(
            !a.step(now + Duration::from_secs(2), false),
            "sem foco não anima"
        );
    }

    #[test]
    fn fala_some_sozinha() {
        let mut a = AvatarState::default();
        a.speak("oi", 1);
        assert_eq!(a.speech(), Some("oi"));
        a.step(Instant::now() + Duration::from_secs(2), false);
        assert_eq!(a.speech(), None);
    }
}
