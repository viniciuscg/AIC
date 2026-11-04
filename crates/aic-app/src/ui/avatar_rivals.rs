//! Lutas do boneco contra quatro rivais. Cada rival é um boneco em pixel art no mesmo molde do
//! Anakin (16×21), com o jeitão de uma pessoa: cabelo, barba, roupa. Desenho só, sem foto.
//!
//! Tudo aqui é por cima da tela: nada recebe clique.

use egui::{Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};

use super::avatar::{
    BODY, BODY_FORCE, HEAD, HEIGHT, LEGS_STAND, Place, WIDTH, ease, lerp, paint_force, paint_saber,
    paint_saber_col, paint_sparks, paint_sprite, paint_sprite_pal, rows,
};
use crate::theme::c;

const PI: f32 = std::f32::consts::PI;
const TAU: f32 = std::f32::consts::TAU;
const GREEN: (&str, &str) = ("#57d13f", "#eaffdf");
const PURPLE: (&str, &str) = ("#a64bff", "#f3e6ff");
const SABER: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rival {
    /// Cabelo ondulado, cavanhaque, camiseta grafite. Sabre verde.
    Curly,
    /// Cabelo curto, bigode fino, camisa vermelha. Luta no braço.
    Red,
    /// Black power, camisa polo preta. Sabre roxo e acrobacia.
    Afro,
    /// Boné, óculos redondos, barba, camiseta branca e corrente.
    Cap,
}

pub const ALL: [Rival; 4] = [Rival::Curly, Rival::Red, Rival::Afro, Rival::Cap];

type Line = Option<&'static str>;
type Head = [&'static str; 9];
type Body = [&'static str; 7];
type Legs = [&'static str; 5];
type Pal = fn(u8) -> Option<Color32>;

impl Rival {
    pub const fn duration(self) -> f32 {
        match self {
            Rival::Curly => 6.4,
            Rival::Red => 5.8,
            Rival::Afro => 6.6,
            Rival::Cap => 6.6,
        }
    }

    /// Nome no menu de animações.
    pub const fn name(self) -> &'static str {
        match self {
            Rival::Curly => "Luta: o do cavanhaque",
            Rival::Red => "Luta: camisa vermelha",
            Rival::Afro => "Luta: black power",
            Rival::Cap => "Luta: boné e óculos",
        }
    }

    /// Quem fala o quê: (Anakin, o rival).
    pub fn lines(self, t: f32) -> (Line, Line) {
        let at = |a: f32, b: f32| (a..b).contains(&t);
        match self {
            Rival::Curly if at(0.9, 2.3) => (None, Some("Hoje o verde ganha!")),
            Rival::Curly if at(2.5, 3.6) => (Some("Você não está pronto."), None),
            Rival::Curly if at(4.3, 5.9) => (None, Some("Pera, pera, pera!")),
            Rival::Red if at(0.9, 2.6) => (None, Some("Sem sabre! No braço!")),
            Rival::Red if at(3.3, 4.3) => (Some("Prefiro a Força."), Some("Aaaaa!")),
            Rival::Red if at(4.3, 5.6) => (Some("Prefiro a Força."), None),
            Rival::Afro if at(0.9, 2.3) => (None, Some("Cuidado com o black!")),
            Rival::Afro if at(3.0, 4.2) => (Some("Não prometo nada."), None),
            Rival::Afro if at(4.5, 6.3) => (None, Some("MEU CABELO!!")),
            Rival::Cap if at(0.9, 1.8) => (None, Some("Pega esse boné!")),
            Rival::Cap if at(1.9, 3.0) => (Some("Era um boné."), None),
            Rival::Cap if at(4.2, 6.2) => (None, Some("Cadê meus óculos?")),
            _ => (None, None),
        }
    }

    /// Desenha no tempo `t`, com o Anakin em `base`. Devolve onde está o rival, pro balão dele.
    pub fn paint(self, p: &Painter, t: f32, base: Pos2) -> Option<Pos2> {
        let cx = Cx { p, t, base };
        match self {
            Rival::Curly => cx.curly(),
            Rival::Red => cx.red(),
            Rival::Afro => cx.afro(),
            Rival::Cap => cx.cap(),
        }
    }
}

// ------------------------------------------------------------------ desenhos

const CURLY_HEAD: Head = [
    "....hHhhHhhH....",
    "...hhhHhhhHhhh..",
    "..hHhhhhHhhhhH..",
    "..hhhhhhhhhhhh..",
    "..hhSsssssssh...",
    "..hhsesssess....",
    "..hhsssssssss...",
    "...hssbbbbss....",
    "......bbs.......",
];
const RED_HEAD: Head = [
    "................",
    ".....hhhhhh.....",
    "...hhHhhHhhh....",
    "..hhhhhhhhhhh...",
    "..hhSsssssss....",
    "..hSsesssess....",
    "..Ssssssssss....",
    "...Ssbbbbss.....",
    ".....ssss.......",
];
const AFRO_HEAD: Head = [
    "...hhHhhhHhh....",
    ".hhHhhhhhhhHhh..",
    "hhhhhHhhhhhhhhh.",
    "hhHhhhhhhhhHhhh.",
    ".hhhhssssssssh..",
    "..hhsesssessh...",
    "...hssssssss....",
    "....sbbbnss.....",
    ".....ssss.......",
];
/// De boné e óculos.
const CAP_HEAD: Head = [
    "................",
    "....ccCCCCcc....",
    "...cCCCCCCCCc...",
    "..cccccccccccccc",
    "..hhssssssss....",
    "..hsgegsgegs....",
    "..hbssssssss....",
    "...bbbnnbbb.....",
    "....bbbbbb......",
];
/// Sem o boné, ainda de óculos.
const CAP_HEAD_HAIR: Head = [
    "................",
    "................",
    "....hhhhhhhh....",
    "..hhhhhhhhhhh...",
    "..hhssssssss....",
    "..hsgegsgegs....",
    "..hbssssssss....",
    "...bbbnnbbb.....",
    "....bbbbbb......",
];
/// Sem boné e sem óculos: olho apertado.
const CAP_HEAD_BLIND: Head = [
    "................",
    "................",
    "....hhhhhhhh....",
    "..hhhhhhhhhhh...",
    "..hhssssssss....",
    "..hssqsssqss....",
    "..hbssssssss....",
    "...bbbnnbbb.....",
    "....bbbbbb......",
];

/// Camiseta de manga curta (`t`), braço à mostra.
const TEE: Body = [
    "...ttttTTtttt...",
    "..tttttTTttttt..",
    "..tttttttttttt..",
    "..sTttttttttTs..",
    "..s..tttttt..s..",
    ".....PPPPPP.....",
    ".....pppppp.....",
];
/// Camisa polo: gola em `T`.
const POLO: Body = [
    "...tttTTTTttt...",
    "..ttttTttTtttt..",
    "..tttttttttttt..",
    "..sTttttttttTs..",
    "..s..tttttt..s..",
    ".....PPPPPP.....",
    ".....pppppp.....",
];
/// Camiseta com corrente (`y`).
const CHAIN: Body = [
    "...ttytttyttt...",
    "..tttyttyttttt..",
    "..ttttyytttttt..",
    "..sTttttttttTs..",
    "..s..tttttt..s..",
    ".....PPPPPP.....",
    ".....pppppp.....",
];
const STAND: Legs = [
    ".....pp..pp.....",
    ".....pp..pp.....",
    ".....pp..pp.....",
    ".....pp..pp.....",
    "....kkk..kkk....",
];
const WALK_A: Legs = [
    ".....pp..pp.....",
    "....pp....pp....",
    "....pp....pp....",
    "...kpp.....pp...",
    "...kk.....kkk...",
];
const WALK_B: Legs = [
    ".....pp..pp.....",
    ".....pp..pp.....",
    "......pppp......",
    "......pp.pp.....",
    ".....kkk.kkk....",
];

const CAP_ART: &[&str] = &[".cCCCc..", "cCCCCCc.", "cccccccc"];
const GLASSES_ART: &[&str] = &["ggg.ggg", "g.ggg.g", "ggg.ggg"];

/// Cores que todos usam.
fn common(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'e' => c("#1c2430"),
        b'q' => c("#3a2a22"),
        b'k' => c("#1a1b20"),
        b'g' => c("#b9a47a"),
        b'y' => c("#d4af37"),
        b'c' => c("#17171a"),
        b'C' => c("#3c3c45"),
        _ => return None,
    })
}

fn pal_curly(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'h' => c("#1f1714"),
        b'H' => c("#3d2c23"),
        b's' => c("#dcaa86"),
        b'S' => c("#c48f6c"),
        b'b' => c("#2a1d17"),
        b'n' => c("#a86a55"),
        b't' => c("#3d3d44"),
        b'T' => c("#2b2b31"),
        b'P' => c("#2c3a52"),
        b'p' => c("#3b4f6e"),
        _ => return common(ch),
    })
}

fn pal_red(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'h' => c("#141210"),
        b'H' => c("#2c2520"),
        b's' => c("#a5704f"),
        b'S' => c("#8a5a3e"),
        b'b' => c("#2a1c14"),
        b'n' => c("#7a4632"),
        b't' => c("#b3232f"),
        b'T' => c("#86161f"),
        b'P' => c("#23242a"),
        b'p' => c("#33353d"),
        _ => return common(ch),
    })
}

fn pal_afro(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'h' => c("#241b16"),
        b'H' => c("#3f3129"),
        b's' => c("#8f5d40"),
        b'S' => c("#774a31"),
        b'b' => c("#24170f"),
        b'n' => c("#6e3d2c"),
        b't' => c("#1d1d22"),
        b'T' => c("#34343c"),
        b'P' => c("#2a2a30"),
        b'p' => c("#3a3a42"),
        _ => return common(ch),
    })
}

fn pal_cap(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'h' => c("#16120f"),
        b'H' => c("#2a221d"),
        b's' => c("#b07a54"),
        b'S' => c("#946244"),
        b'b' => c("#1d1612"),
        b'n' => c("#7d4636"),
        b't' => c("#e9e9ec"),
        b'T' => c("#c6c9d0"),
        b'P' => c("#3b4f6e"),
        b'p' => c("#4c6489"),
        _ => return common(ch),
    })
}

fn sprite(
    head: &'static Head,
    body: &'static Body,
    legs: &'static Legs,
) -> impl Iterator<Item = &'static str> {
    head.iter().chain(body.iter()).chain(legs.iter()).copied()
}

/// Desenho solto (boné, óculos), com o centro em `at`.
fn paint_bit(p: &Painter, art: &[&str], pal: Pal, at: Pos2, angle: f32, alpha: f32) {
    let cols = art[0].len() as f32;
    let half = vec2(cols, art.len() as f32) * 1.5;
    let (sin, cos) = angle.sin_cos();
    let map = |col: f32, row: f32| {
        let v = vec2(col, row) * 3.0 - half;
        at + vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    };
    let mut mesh = egui::Mesh::default();
    for (y, row) in art.iter().enumerate() {
        for (x, ch) in row.bytes().enumerate() {
            let Some(col) = pal(ch) else { continue };
            let col = col.gamma_multiply(alpha);
            let (x, y) = (x as f32, y as f32);
            let i = mesh.vertices.len() as u32;
            mesh.colored_vertex(map(x, y), col);
            mesh.colored_vertex(map(x + 1.0, y), col);
            mesh.colored_vertex(map(x + 1.0, y + 1.0), col);
            mesh.colored_vertex(map(x, y + 1.0), col);
            mesh.add_triangle(i, i + 1, i + 2);
            mesh.add_triangle(i, i + 2, i + 3);
        }
    }
    p.add(mesh);
}

fn walk(t: f32) -> &'static Legs {
    if ((t * 9.0) as u32).is_multiple_of(2) {
        &WALK_A
    } else {
        &WALK_B
    }
}

/// Entra correndo da esquerda até `stay` em `secs`.
fn approach(t: f32, secs: f32, stay: f32) -> f32 {
    lerp(190.0, stay, ease(t / secs))
}

/// Inclinação dos sabres pra que as pontas se encontrem no meio, com `gap` entre os bonecos.
fn clash_angle(gap: f32) -> f32 {
    ((gap - 33.0) / 2.0 / SABER).clamp(-0.95, 0.95).asin()
}

fn fade(t: f32, dur: f32) -> f32 {
    (t / 0.2).min(1.0) * ((dur - t) / 0.4).clamp(0.0, 1.0)
}

struct Cx<'a> {
    p: &'a Painter,
    t: f32,
    base: Pos2,
}

impl Cx<'_> {
    fn me(&self, off: egui::Vec2) -> Place {
        let mut me = Place::new(self.base + off, true);
        me.sith = true;
        me
    }

    fn draw_me(&self, me: Place, saber: Option<f32>) {
        paint_sprite(self.p, rows(&HEAD, &BODY, &LEGS_STAND, false), me);
        if let Some(a) = saber {
            paint_saber(self.p, me.hand(), me.blade(a), SABER, false, 1.0);
        }
    }

    /// Troca de golpes, como no duelo com o Obi-Wan. Devolve (ângulo dos dois sabres, faísca).
    fn exchange(&self, from: f32, gap: f32) -> (f32, Option<(f32, u32)>) {
        let hit = clash_angle(gap);
        let beat = (self.t - from) / 0.4;
        let frac = beat.fract();
        let s = (frac * PI).sin();
        let spark = (frac < 0.3).then_some((frac * 0.4, beat as u32));
        (lerp(hit, 0.1, s), spark)
    }

    fn spark_between(&self, me: Place, gap: f32, spark: Option<(f32, u32)>) {
        if let Some((age, seed)) = spark {
            let hit = clash_angle(gap);
            let tip = me.hand() + vec2(-hit.sin(), -hit.cos()) * SABER;
            paint_sparks(self.p, tip, age, seed);
        }
    }

    /// Sabre verde. Troca de golpes, o Anakin dá um mortal e arranca o sabre da mão dele.
    fn curly(&self) -> Option<Pos2> {
        let t = self.t;
        let dur = Rival::Curly.duration();
        let fleeing = t >= 4.5;
        let gap = if t < 0.8 {
            approach(t, 0.8, 80.0)
        } else if fleeing {
            lerp(80.0, 280.0, ease((t - 4.5) / 1.7))
        } else {
            80.0 + (t * 1.7).sin() * 4.0
        };
        let at = pos2(self.base.x - gap, self.base.y);
        let mut him = Place::new(at, fleeing);
        him.alpha = fade(t, dur);
        let legs = if t < 0.8 || fleeing { walk(t) } else { &STAND };
        paint_sprite_pal(self.p, sprite(&CURLY_HEAD, &TEE, legs), him, pal_curly);

        // mortal por cima do golpe final
        let u = (t - 3.4) / 0.6;
        let flipping = (0.0..=1.0).contains(&u);
        let hop = if flipping { (u * PI).sin() * 28.0 } else { 0.0 };
        let mut me = self.me(vec2(0.0, -hop));
        if flipping {
            me.angle = -ease(u) * TAU;
        }
        let fighting = (0.8..3.4).contains(&t);
        let (angle, spark) = if fighting {
            self.exchange(0.8, gap)
        } else {
            (0.15, None)
        };
        let disarmed = t >= 3.9;
        if disarmed {
            // o sabre dele sai girando pra cima e cai fora da tela
            let k = t - 3.9;
            let from = at + vec2(40.5, 45.0);
            let pos = from + vec2(-k * 70.0, -150.0 * k + 160.0 * k * k);
            let spin = k * 14.0;
            let a = (1.0 - (k - 1.4) / 0.4).clamp(0.0, 1.0);
            paint_saber_col(self.p, pos, spin, SABER - 6.0, GREEN, a);
        } else {
            paint_saber_col(
                self.p,
                him.hand(),
                him.blade(angle),
                SABER,
                GREEN,
                him.alpha,
            );
        }
        self.draw_me(me, Some(if flipping { 0.0 } else { angle }));
        if fighting {
            self.spark_between(me, gap, spark);
        }
        if (3.9..4.05).contains(&t) {
            paint_sparks(self.p, at + vec2(40.0, 30.0), t - 3.9, 7);
        }
        Some(at)
    }

    /// No braço: ele dá socos, o Anakin desvia e responde com a Força.
    fn red(&self) -> Option<Pos2> {
        let t = self.t;
        let dur = Rival::Red.duration();
        const CLOSE: f32 = 54.0;
        let pushed = t >= 3.2;
        let k = (t - 3.2).max(0.0);
        let gap = if t < 0.8 {
            approach(t, 0.8, CLOSE)
        } else if pushed {
            CLOSE + k * 60.0 + k * k * 200.0
        } else {
            CLOSE
        };
        // soco: estica e recolhe a cada batida
        let punching = (0.8..3.0).contains(&t);
        let beat = (t - 0.8) / 0.45;
        let reach = if punching {
            (beat.fract() * PI).sin().powi(2)
        } else {
            0.0
        };
        let lift = if pushed {
            (k * 3.0).min(PI).sin() * 22.0
        } else {
            0.0
        };
        let at = pos2(self.base.x - gap + reach * 4.0, self.base.y - lift);
        let mut him = Place::new(at, false);
        him.alpha = fade(t, dur) * (1.0 - (k - 1.6) / 0.4).clamp(0.0, 1.0);
        him.angle = -k * 6.0;
        let legs = if t < 0.8 { walk(t) } else { &STAND };
        paint_sprite_pal(self.p, sprite(&RED_HEAD, &TEE, legs), him, pal_red);
        if reach > 0.05 {
            let shoulder = him.point(13.0, 12.5);
            let fist = him.point(14.0 + reach * 5.0, 12.5);
            let skin = pal_red(b's').unwrap_or_default();
            self.p
                .line_segment([shoulder, fist], Stroke::new(3.0, skin));
            self.p.rect_filled(
                Rect::from_center_size(fist, vec2(5.0, 5.0)),
                1.0,
                pal_red(b'S').unwrap_or_default(),
            );
        }

        // o Anakin desvia: numa batida inclina pra trás, na outra se abaixa
        let odd = (beat as u32) % 2 == 1;
        let (lean, duck) = match (punching, odd) {
            (true, false) => (reach * 0.3, 0.0),
            (true, true) => (0.0, reach * 7.0),
            _ => (0.0, 0.0),
        };
        let mut me = self.me(vec2(0.0, duck));
        me.angle = lean;
        let reaching = (3.0..4.4).contains(&t);
        if reaching {
            paint_sprite(self.p, rows(&HEAD, &BODY_FORCE, &LEGS_STAND, false), me);
            if (3.05..3.7).contains(&t) {
                let from = me.point(16.0, 11.5);
                paint_force(self.p, from, at + vec2(WIDTH / 2.0, HEIGHT * 0.35), t);
            }
        } else {
            paint_sprite(self.p, rows(&HEAD, &BODY, &LEGS_STAND, false), me);
        }
        Some(at)
    }

    /// Sabre roxo, rodopio e salto. O sabre do Anakin raspa no cabelo e sai fumaça.
    fn afro(&self) -> Option<Pos2> {
        let t = self.t;
        let dur = Rival::Afro.duration();
        const GAP: f32 = 84.0;
        let fleeing = t >= 4.9;
        let gap = if t < 0.8 {
            approach(t, 0.8, GAP)
        } else if fleeing {
            lerp(GAP, 290.0, ease((t - 4.9) / 1.6))
        } else {
            GAP
        };
        // os dois saltam juntos e se encontram no alto
        let u = (t - 2.0) / 0.9;
        let jumping = (0.0..=1.0).contains(&u);
        let hop = if jumping { (u * PI).sin() * 38.0 } else { 0.0 };
        // ele foge pulando e batendo na cabeça
        let bounce = if fleeing {
            ((t * 9.0).sin().abs()) * 6.0
        } else {
            0.0
        };
        let at = pos2(self.base.x - gap, self.base.y - hop - bounce);
        let mut him = Place::new(at, fleeing);
        him.alpha = fade(t, dur);
        let legs = if t < 0.8 || fleeing { walk(t) } else { &STAND };
        paint_sprite_pal(self.p, sprite(&AFRO_HEAD, &POLO, legs), him, pal_afro);

        let me = self.me(vec2(0.0, -hop));
        let fighting = (2.9..4.3).contains(&t);
        let (mut mine, his, spark) = if fighting {
            let (a, s) = self.exchange(2.9, gap);
            (a, a, s)
        } else if jumping {
            let hit = clash_angle(gap);
            let s = (u * PI).sin();
            (lerp(0.1, hit, s), lerp(0.1, hit, s), None)
        } else if (0.8..2.0).contains(&t) {
            // rodopio do sabre na mão
            (0.15, t * 12.0, None)
        } else {
            (0.15, 0.15, None)
        };
        // o golpe alto que pega no cabelo
        let w = (t - 4.3) / 0.5;
        if (0.0..=1.0).contains(&w) {
            mine = lerp(0.2, 1.25, (w * PI).sin());
        }
        if !fleeing {
            let angle = if (0.8..2.0).contains(&t) {
                his
            } else {
                him.blade(his)
            };
            paint_saber_col(self.p, him.hand(), angle, SABER, PURPLE, him.alpha);
        }
        self.draw_me(me, Some(mine));
        if fighting {
            self.spark_between(me, gap, spark);
        }
        if jumping && (0.4..0.6).contains(&u) {
            self.spark_between(me, gap, Some(((u - 0.4) * 0.6, 3)));
        }
        // fumaça e fogo no cabelo
        if t >= 4.55 {
            let k = t - 4.55;
            let top = him.point(7.0, 0.0);
            for i in 0..5u32 {
                let life = ((k * 1.3 + i as f32 * 0.2) % 1.0).max(0.0);
                let dx = (i as f32 * 2.1 + k * 3.0).sin() * 6.0;
                let r = 2.0 + life * 5.0;
                let a = (1.0 - life) * 0.55 * him.alpha;
                self.p.circle_filled(
                    top + vec2(dx, -life * 26.0),
                    r,
                    c("#9a9aa0").gamma_multiply(a),
                );
            }
            if k < 1.4 {
                for i in 0..4u32 {
                    let flick = ((t * 20.0 + i as f32 * 1.7).sin() * 0.5 + 0.5) * 4.0;
                    let x = him.point(4.0 + i as f32 * 2.5, 0.0).x;
                    let col = if i % 2 == 0 { "#ff8a1f" } else { "#ffd23a" };
                    self.p.rect_filled(
                        Rect::from_min_size(pos2(x, top.y - flick - 2.0), vec2(3.0, flick + 2.0)),
                        0.0,
                        c(col).gamma_multiply(him.alpha),
                    );
                }
            }
        }
        Some(at)
    }

    /// Joga o boné (o Anakin corta no ar), atira, e o próprio tiro volta e leva os óculos.
    fn cap(&self) -> Option<Pos2> {
        let t = self.t;
        let dur = Rival::Cap.duration();
        const GAP: f32 = 120.0;
        let leaving = t >= 4.4;
        let gap = if t < 0.8 {
            approach(t, 0.8, GAP)
        } else if leaving {
            // sai devagar, tateando, pro lado errado
            GAP + (t - 4.4) * 70.0
        } else {
            GAP
        };
        let wobble = if leaving { (t * 6.0).sin() * 0.08 } else { 0.0 };
        let at = pos2(self.base.x - gap, self.base.y);
        let mut him = Place::new(at, leaving);
        him.alpha = fade(t, dur);
        him.angle = wobble;
        let head: &'static Head = if t < 1.0 {
            &CAP_HEAD
        } else if t < 3.8 {
            &CAP_HEAD_HAIR
        } else {
            &CAP_HEAD_BLIND
        };
        let legs = if t < 0.8 || leaving {
            walk(t * 0.7)
        } else {
            &STAND
        };
        paint_sprite_pal(self.p, sprite(head, &CHAIN, legs), him, pal_cap);

        let me = self.me(vec2(0.0, 0.0));
        let mut swing: f32 = 0.15;

        // o boné voa girando até o Anakin, que corta no ar
        let target = me.point(15.0, 4.0);
        let k = (t - 1.0) / 0.6;
        if (0.0..1.0).contains(&k) {
            let from = him.point(8.0, 2.0);
            let pos = from + (target - from) * k - vec2(0.0, (k * PI).sin() * 24.0);
            paint_bit(self.p, CAP_ART, pal_cap, pos, t * 18.0, 1.0);
        }
        let s = (t - 1.5) / 0.3;
        if (0.0..=1.0).contains(&s) {
            swing = lerp(0.15, 1.3, (s * PI).sin());
        }
        let cut = t - 1.6;
        if (0.0..1.2).contains(&cut) {
            let a = 1.0 - (cut / 1.2);
            for (half, dir) in [(0usize, -1.0f32), (4, 1.0)] {
                let part: Vec<&str> = CAP_ART.iter().map(|r| &r[half..half + 4]).collect();
                let pos = target + vec2(dir * (6.0 + cut * 30.0), cut * cut * 140.0);
                paint_bit(self.p, &part, pal_cap, pos, dir * cut * 6.0, a);
            }
            if cut < 0.12 {
                paint_sparks(self.p, target, cut, 5);
            }
        }

        // três tiros; o último volta certeiro nos óculos
        let armed = (2.4..4.2).contains(&t);
        let muzzle = him.point(17.5, 13.0);
        if armed {
            let grip = him.point(14.0, 13.0);
            self.p
                .line_segment([grip, muzzle], Stroke::new(3.5, c("#5d636e")));
        }
        let guard = me.point(13.0, 7.0);
        let glasses = him.point(8.0, 5.5);
        for (i, shot) in [2.7f32, 3.1, 3.5].into_iter().enumerate() {
            let k = t - shot;
            if (0.0..0.22).contains(&k) {
                let a = muzzle + (guard - muzzle) * (k / 0.22);
                self.p
                    .line_segment([a, a + vec2(9.0, 0.0)], Stroke::new(2.0, c("#ff6a3a")));
            }
            if (0.15..0.4).contains(&k) {
                swing = 0.2 + ((k - 0.15) / 0.25 * PI).sin() * 0.9;
            }
            if (0.22..0.32).contains(&k) {
                paint_sparks(self.p, guard, k - 0.22, i as u32);
            }
            let dest = if i == 2 {
                glasses
            } else {
                at + vec2(-30.0, -40.0 + 20.0 * i as f32)
            };
            if (0.22..0.3).contains(&k) {
                let a = guard + (dest - guard) * ((k - 0.22) / 0.08);
                self.p
                    .line_segment([a, a - vec2(9.0, 0.0)], Stroke::new(2.0, c("#ff6a3a")));
            }
        }
        // os óculos voam
        let g = t - 3.8;
        if (0.0..1.3).contains(&g) {
            let pos = glasses + vec2(-g * 50.0, -90.0 * g + 120.0 * g * g);
            paint_bit(self.p, GLASSES_ART, pal_cap, pos, -g * 9.0, 1.0 - g / 1.3);
            if g < 0.12 {
                paint_sparks(self.p, glasses, g, 9);
            }
        }
        self.draw_me(me, Some(swing));
        Some(at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desenhos_tem_a_largura_certa_e_cor() {
        let heads: [(&Head, Pal); 6] = [
            (&CURLY_HEAD, pal_curly),
            (&RED_HEAD, pal_red),
            (&AFRO_HEAD, pal_afro),
            (&CAP_HEAD, pal_cap),
            (&CAP_HEAD_HAIR, pal_cap),
            (&CAP_HEAD_BLIND, pal_cap),
        ];
        for (head, pal) in heads {
            for body in [&TEE, &POLO, &CHAIN] {
                for legs in [&STAND, &WALK_A, &WALK_B] {
                    let v: Vec<_> = sprite(head, body, legs).collect();
                    assert_eq!(v.len(), 21);
                    for r in v {
                        assert_eq!(r.len(), 16, "{r}");
                        for ch in r.bytes().filter(|ch| *ch != b'.') {
                            assert!(pal(ch).is_some(), "sem cor: {} em {r}", ch as char);
                        }
                    }
                }
            }
        }
        for art in [CAP_ART, GLASSES_ART] {
            assert!(art.iter().all(|r| r.len() == art[0].len()));
        }
    }

    #[test]
    fn toda_luta_dura_e_fala() {
        for r in ALL {
            assert!(r.duration() > 3.0);
            let falou = (0..100).any(|i| r.lines(i as f32 / 10.0) != (None, None));
            assert!(falou, "{r:?}");
        }
    }
}
