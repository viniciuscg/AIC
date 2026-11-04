//! Surpresas do boneco. Sem spoiler na documentação: quem usa descobre vendo.
//!
//! Tudo aqui é só desenho, em cima da tela: nada recebe clique.

use egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, pos2, vec2};

use super::avatar::{
    BODY, BODY_FORCE, HEAD, HEIGHT, LEGS_STAND, LEGS_WALK_A, LEGS_WALK_B, Legs, OBI_BODY, OBI_HEAD,
    Place, WIDTH, ease, lerp, paint_force, paint_saber, paint_saber_col, paint_sparks,
    paint_sprite, rows,
};
use crate::theme::c;

const TAU: f32 = std::f32::consts::TAU;
const PI: f32 = std::f32::consts::PI;
const GREEN: (&str, &str) = ("#57d13f", "#eaffdf");
const BLUE: (&str, &str) = ("#3f9dff", "#eaf6ff");
const RED: (&str, &str) = ("#e8262d", "#ffe2df");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Egg {
    Pod,
    Sand,
    Small,
    Astro,
    Angel,
    Lava,
    Moon,
    Dogfight,
    Roger,
    Power,
    General,
    Boomerang,
    Holo,
    Fur,
    Horns,
    Pram,
    Clumsy,
    Sleep,
    Fireworks,
}

/// As que entram no rodízio. As outras dependem da hora ou do dia.
pub const ROTATION: [Egg; 17] = [
    Egg::Small,
    Egg::Moon,
    Egg::Roger,
    Egg::Sand,
    Egg::General,
    Egg::Pod,
    Egg::Astro,
    Egg::Power,
    Egg::Clumsy,
    Egg::Lava,
    Egg::Boomerang,
    Egg::Pram,
    Egg::Dogfight,
    Egg::Horns,
    Egg::Angel,
    Egg::Fur,
    Egg::Holo,
];

pub const ALL: [Egg; 19] = [
    Egg::Pod,
    Egg::Sand,
    Egg::Small,
    Egg::Astro,
    Egg::Angel,
    Egg::Lava,
    Egg::Moon,
    Egg::Dogfight,
    Egg::Roger,
    Egg::Power,
    Egg::General,
    Egg::Boomerang,
    Egg::Holo,
    Egg::Fur,
    Egg::Horns,
    Egg::Pram,
    Egg::Clumsy,
    Egg::Sleep,
    Egg::Fireworks,
];

type Line = Option<&'static str>;

impl Egg {
    /// Nome no menu de animações: dá a pista sem contar a cena.
    pub const fn name(self) -> &'static str {
        match self {
            Egg::Pod => "Corrida",
            Egg::Sand => "Areia",
            Egg::Small => "Mestre pequenino",
            Egg::Astro => "Droide amigo",
            Egg::Angel => "Anjo",
            Egg::Lava => "Mustafar",
            Egg::Moon => "Lua",
            Egg::Dogfight => "Naves",
            Egg::Roger => "Roger roger",
            Egg::Power => "Poder ilimitado",
            Egg::General => "Hello there",
            Egg::Boomerang => "Bumerangue",
            Egg::Holo => "Holograma",
            Egg::Fur => "Peludo",
            Egg::Horns => "Chifres",
            Egg::Pram => "Carona",
            Egg::Clumsy => "Desastrado",
            Egg::Sleep => "Cochilo",
            Egg::Fireworks => "Fogos",
        }
    }

    pub const fn duration(self) -> f32 {
        match self {
            Egg::Pod => 4.6,
            Egg::Sand => 4.2,
            Egg::Small => 5.5,
            Egg::Astro => 5.2,
            Egg::Angel => 5.5,
            Egg::Lava => 6.2,
            Egg::Moon => 8.0,
            Egg::Dogfight => 3.6,
            Egg::Roger => 6.0,
            Egg::Power => 6.0,
            Egg::General => 6.0,
            Egg::Boomerang => 4.0,
            Egg::Holo => 5.6,
            Egg::Fur => 5.5,
            Egg::Horns => 6.0,
            Egg::Pram => 5.8,
            Egg::Clumsy => 5.0,
            Egg::Sleep => 9.0,
            Egg::Fireworks => 7.0,
        }
    }

    /// Quem fala o quê: (Anakin, o outro).
    pub fn lines(self, t: f32) -> (Line, Line) {
        let at = |a: f32, b: f32| (a..b).contains(&t);
        match self {
            Egg::Pod if at(3.2, 4.6) => (Some("Now this is podracing!"), None),
            Egg::Sand if at(0.4, 2.2) => (Some("I don't like sand."), None),
            Egg::Sand if at(2.4, 4.2) => (Some("It's coarse and rough."), None),
            Egg::Small if at(0.4, 2.4) => (None, Some("Size matters not.")),
            Egg::Small if at(3.0, 5.0) => (Some("This is outrageous! It's unfair!"), None),
            Egg::Astro if at(1.2, 3.2) => (None, Some("Bwoop beep beep!")),
            Egg::Astro if at(3.3, 4.9) => (Some("Good job, Artoo!"), None),
            Egg::Angel if at(0.8, 2.8) => (Some("Are you an angel?"), None),
            Egg::Angel if at(3.0, 4.8) => (None, Some("You're a funny little boy.")),
            Egg::Lava if at(1.0, 2.6) => (None, Some("It's over, Anakin!")),
            Egg::Lava if at(3.5, 5.2) => (Some("I hate youuu!"), None),
            Egg::Moon if at(0.6, 2.6) => (Some("That's no moon."), None),
            Egg::Moon if at(5.0, 7.0) => (Some("I've got a bad feeling about this."), None),
            Egg::Dogfight if at(2.5, 3.6) => (Some("Whoa!"), None),
            Egg::Roger if at(0.5, 1.2) => (None, Some("Roger roger.")),
            Egg::Roger if at(4.0, 5.4) => (None, Some("Uh-oh.")),
            Egg::Power if at(0.3, 1.1) => (None, Some("Do it.")),
            Egg::Power if at(1.2, 3.6) => (None, Some("UNLIMITED POWER!")),
            Egg::Power if at(4.3, 5.8) => (Some("Not today."), None),
            Egg::General if at(0.3, 2.0) => (Some("Hello there!"), None),
            Egg::General if at(2.2, 3.6) => (None, Some("General Kenobi!")),
            Egg::General if at(3.7, 4.7) => (Some("Wrong guy."), None),
            Egg::General if at(4.8, 6.0) => (None, Some("Cof cof!")),
            Egg::Boomerang if at(2.8, 4.0) => (Some("Yippee!"), None),
            Egg::Holo if at(0.8, 3.6) => (None, Some("Help me, Obi-Wan Kenobi...")),
            Egg::Holo if at(3.8, 5.4) => (Some("I'm not him."), None),
            Egg::Fur if at(1.0, 2.8) => (None, Some("Rrraaarghh!")),
            Egg::Fur if at(3.0, 4.8) => (Some("Easy, big guy!"), None),
            Egg::Horns if at(0.8, 2.6) => (None, Some("At last...")),
            Egg::Horns if at(3.0, 4.6) => (Some("Who invited you?"), None),
            Egg::Pram if at(1.6, 3.6) => (Some("Hey! Put me down!"), None),
            Egg::Pram if at(3.9, 5.0) => (None, Some("Patu!")),
            Egg::Clumsy if at(2.0, 3.6) => (None, Some("Ooh mooey mooey!")),
            Egg::Clumsy if at(3.6, 4.8) => (Some("Ugh."), None),
            Egg::Fireworks if at(1.0, 5.0) => (Some("May the Fourth be with you!"), None),
            _ => (None, None),
        }
    }

    /// Desenha no tempo `t`. Devolve onde está o outro boneco, pro balão dele.
    pub fn paint(self, p: &Painter, t: f32, base: Pos2, screen: Rect) -> Option<Pos2> {
        let cx = Cx { p, t, base, screen };
        match self {
            Egg::Pod => cx.pod(),
            Egg::Sand => cx.sand(),
            Egg::Small => cx.small(),
            Egg::Astro => cx.astro(),
            Egg::Angel => cx.angel(),
            Egg::Lava => cx.lava(),
            Egg::Moon => cx.moon(),
            Egg::Dogfight => cx.dogfight(),
            Egg::Roger => cx.roger(),
            Egg::Power => cx.power(),
            Egg::General => cx.general(),
            Egg::Boomerang => cx.boomerang(),
            Egg::Holo => cx.holo(),
            Egg::Fur => cx.fur(),
            Egg::Horns => cx.horns(),
            Egg::Pram => cx.pram(),
            Egg::Clumsy => cx.clumsy(),
            Egg::Sleep => cx.sleep(),
            Egg::Fireworks => cx.fireworks(),
        }
    }
}

// ------------------------------------------------------------------ desenhos

type Art = &'static [&'static str];
type Pal = fn(u8) -> Option<Color32>;

const ASTRO: Art = &[
    "...wwwww...",
    "..wBBBBBw..",
    ".wBBBBBBBw.",
    ".wwwKwwwww.",
    ".WBBWWWBBW.",
    ".WWWWWWWWW.",
    ".WBWBBBWBW.",
    ".WBWWWWWBW.",
    ".WWBBBBBWW.",
    ".WWWWWWWWW.",
    ".BWWWWWWWB.",
    "BB.WWWWW.BB",
    "BB.WWWWW.BB",
    "WW.......WW",
];
fn pal_astro(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'w' => c("#aeb4bd"),
        b'W' => c("#eef0f2"),
        b'B' => c("#2b5dab"),
        b'K' => c("#15171c"),
        _ => return None,
    })
}

const SMALL: Art = &[
    "....gggg....",
    "GGgGGGGGGgGG",
    ".GGGkGGkGGG.",
    "...GGGGGG...",
    "....GGGG....",
    "...rrrrrr...",
    "..rrRrrRrr..",
    "..rrRrrRrr..",
    "..g.rRRr.g..",
    "....rrrr....",
    "....rrrr....",
    "....gg.gg...",
];
fn pal_small(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'g' => c("#6f8f3a"),
        b'G' => c("#8bad4a"),
        b'k' => c("#15171c"),
        b'r' => c("#9c8868"),
        b'R' => c("#6e5c42"),
        _ => return None,
    })
}

const ANGEL: Art = &[
    "....kkkkkkkk....",
    "...kkkkkkkkkk...",
    "..kkkkkkkkkkkk..",
    "..kkksssssskkk..",
    "..kkssssssskkk..",
    "..kksessesskkk..",
    "..kkssssssskkk..",
    "..kkkssrssskkk..",
    "...kk.ssss.kk...",
    "...WWWWWWWWWW...",
    "..WWWWWWWWWWWW..",
    "..sWWWWWWWWWWs..",
    "..sWWWWWWWWWWs..",
    "....WWWWWWWW....",
    "....WWWWWWWW....",
    "....WWwwwwWW....",
    "...WWWWWWWWWW...",
    "...WWWWWWWWWW...",
    "..WWWWwWWwWWWW..",
    "..WWWWWWWWWWWW..",
    "..WWWWWWWWWWWW..",
];
fn pal_angel(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'k' => c("#3a241a"),
        b's' => c("#e6b78f"),
        b'e' => c("#1c2430"),
        b'r' => c("#b53a4a"),
        b'W' => c("#eef0f2"),
        b'w' => c("#aeb4bd"),
        _ => return None,
    })
}
fn pal_holo(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'k' | b'e' | b'r' => c("#2f7fd0"),
        b's' | b'w' => c("#5bb6ff"),
        b'W' => c("#a8dcff"),
        _ => return None,
    })
}

const ROGER: Art = &[
    "......nnnn......",
    "....nnNNNNnn....",
    "..nnNNNNNNNNnn..",
    "....nnNNkNN.....",
    ".....nNNNNn.....",
    "......NNNN......",
    ".......nn.......",
    "......nNNn......",
    ".....nNNNNn.....",
    "....nNNNNNNn....",
    "...n.NNNNNN.n...",
    "..n...NNNN...n..",
    "..n....nn....n..",
    ".......nn.......",
    "......nNNn......",
    ".....n....n.....",
    ".....n....n.....",
    "....n......n....",
    "....n......n....",
    "....n......n....",
    "...nn......nn...",
];
fn pal_roger(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'n' => c("#8e7b4f"),
        b'N' => c("#c4ad72"),
        b'k' => c("#3a2c15"),
        _ => return None,
    })
}

const POWER: Art = &[
    "....cccccccc....",
    "...cCCCCCCCCc...",
    "..cCCCCCCCCCCc..",
    "..cCCsssssCCCc..",
    "..cCsjsjsjsCCc..",
    "..cCsysssyCCCc..",
    "..cCCsjjjsCCCc..",
    "...cCCssssCCc...",
    "....cCCCCCCc....",
    "...cCCCCCCCCc...",
    "..cCCCCCCCCCCc..",
    "..cCCCCCCCCCCCss",
    "..cCCCCCCCCc....",
    "..cCCCCCCCCc....",
    "..cCCCCCCCCc....",
    "...cCCCCCCc.....",
    "...cCCCCCCc.....",
    "...cCCCCCCc.....",
    "..cCCCCCCCCc....",
    "..cCCCCCCCCc....",
    "..cCCCCCCCCc....",
];
fn pal_power(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'c' => c("#1f2127"),
        b'C' => c("#3a3d47"),
        b's' => c("#d9cdb8"),
        b'j' => c("#9f927b"),
        b'y' => c("#f2c230"),
        _ => return None,
    })
}

const GENERAL: Art = &[
    "....WWWWWWWW....",
    "...WWWWWWWWWW...",
    "...WWkkWWkkWW...",
    "...WWyyWWyyWW...",
    "....WWWWWWWW....",
    ".....WkWkWk.....",
    "......wWWw......",
    "....wWWwwWWw....",
    "...wWwwWWwwWw...",
    "..wW.wwWWww.Ww..",
    "..W..wwWWww..W..",
    "..W...wWWw...W..",
    "..w....WW....w..",
    ".......ww.......",
    "......wWWw......",
    ".....wW..Ww.....",
    ".....W....W.....",
    ".....w....w.....",
    "....wW....Ww....",
    "....W......W....",
    "...WWW....WWW...",
];
fn pal_general(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'W' => c("#d9d4c5"),
        b'w' => c("#8b93a1"),
        b'k' => c("#1c1c1c"),
        b'y' => c("#f2c230"),
        _ => return None,
    })
}

const FUR: Art = &[
    "....ffffffff....",
    "...fFFFFFFFFf...",
    "..fFFFFFFFFFFf..",
    "..fFFkFFFkFFFf..",
    "..fFFFFFFFFFFf..",
    "..fFFFFkkFFFFf..",
    "..fFFFkkkkFFFf..",
    "...fFFFFFFFFf...",
    "..fFFFFFFFFFFf..",
    ".fFFFFFFFFFFFFf.",
    ".fFFbbFFFFFFFFf.",
    ".fFFFbbFFFFFFFf.",
    ".fFFFFbbFFFFFFf.",
    ".fFFFFFbbFFFFFf.",
    ".fF.FFFFbbFF.Ff.",
    ".fF.FFFFFbbF.Ff.",
    ".ff.FFFFFFFF.ff.",
    "....FFFFFFFF....",
    "....FFF..FFF....",
];
const FUR_A: Art = &[
    "...FFF....FFF...",
    "...FFF....FFF...",
    "..FFF......FFF..",
    "..kkk......kkk..",
];
const FUR_B: Art = &[
    "....FFF..FFF....",
    "....FFF..FFF....",
    "....FFF..FFF....",
    "...kkkk..kkkk...",
];
fn pal_fur(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'f' => c("#4a2f1a"),
        b'F' => c("#6e4826"),
        b'b' => c("#2b1b10"),
        b'k' => c("#1c1410"),
        _ => return None,
    })
}

const HORNS: Art = &[
    "...g..gggg..g...",
    "...gg.gggg.gg...",
    "...rrrrrrrrrr...",
    "..rrkrrrrrrkrr..",
    "..rkkrrrrrrkkr..",
    "..ryyrkkryyrr...",
    "..rrkrrrrrrkrr..",
    "...rrkkkkkkrr...",
    ".....rrrrrr.....",
    "...cccccccccc...",
    "..cCCCCCCCCCCc..",
    "..cCccCCCCccCc..",
    "..cCccCCCCccCc..",
    "..rc.cCCCCc.cr..",
    "....cCCCCCCc....",
    "....cCCCCCCc....",
    ".....cc..cc.....",
    ".....cc..cc.....",
    ".....cc..cc.....",
    ".....cc..cc.....",
    "....ccc..ccc....",
];
fn pal_horns(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'r' => c("#b52a2a"),
        b'k' => c("#15171c"),
        b'g' => c("#d8c7a3"),
        b'y' => c("#f2c230"),
        b'c' => c("#1f2127"),
        b'C' => c("#3a3d47"),
        _ => return None,
    })
}

const CLUMSY: Art = &[
    "......oooo......",
    ".....oOOOOo.....",
    "..ooOOOOOOOOoo..",
    ".ooOOkOOOkOOOoo.",
    "oo.oOOOOOOOOo.oo",
    "o...oOOOOOOOo..o",
    "o....oOOOOOo...o",
    "o.....OOOOO....o",
    "......tttt......",
    ".....ttttttt....",
    "....OOttttttOO..",
    "....O.ttttt.O...",
    "....O.bbbbb.O...",
    "......bbbbb.....",
    "......bb.bb.....",
    "......bb.bb.....",
    "......OO.OO.....",
    "......OO.OO.....",
    "......OO.OO.....",
    ".....OOO.OOO....",
    "....OOOO.OOOO...",
];
fn pal_clumsy(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'o' => c("#c96a2b"),
        b'O' => c("#e58a3e"),
        b'k' => c("#1c1c1c"),
        b't' => c("#8a7a55"),
        b'b' => c("#5a4a30"),
        _ => return None,
    })
}

const PRAM: Art = &[
    "GG..ggg..GG",
    ".GGgGGGgGG.",
    "..gGkGkGg..",
    "..gGGGGGg..",
    "...ttttt...",
    ".aaatttaaa.",
    "aAAAAAAAAAa",
    "aAAAAAAAAAa",
    ".aAAAAAAAa.",
    "..aaaaaaa..",
];
fn pal_pram(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'G' => c("#8bad4a"),
        b'g' => c("#6f8f3a"),
        b'k' => c("#15171c"),
        b't' => c("#c9b28a"),
        b'a' => c("#8b93a1"),
        b'A' => c("#c7cdd6"),
        _ => return None,
    })
}

const POD: Art = &[
    "............eeeeeee...",
    "...........eEEEEEEEF..",
    "............eeeeeee...",
    ".yYYsYYllllllllllll...",
    "yYYYYYY...........l...",
    ".yyyyy....eeeeeee.....",
    "..........eEEEEEEEF...",
    "...........eeeeeee....",
];
fn pal_pod(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'e' => c("#8a8f98"),
        b'E' => c("#c7cdd6"),
        b'F' => c("#ff9a3a"),
        b'y' => c("#b4792a"),
        b'Y' => c("#d9a545"),
        b's' => c("#e6b78f"),
        b'l' => c("#6e7480"),
        _ => return None,
    })
}

const XWING: Art = &[
    "rrrrrrr.....",
    "..WWW.......",
    "..WWWWWWWWw.",
    "..WWBWWWWWWW",
    "..WWWWWWWWw.",
    "..WWW.......",
    "rrrrrrr.....",
];
fn pal_xwing(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'r' => c("#c8473f"),
        b'W' => c("#e4e6ea"),
        b'w' => c("#aeb4bd"),
        b'B' => c("#2b5dab"),
        _ => return None,
    })
}

const TIE: Art = &[
    "kk.......kk",
    "kk.......kk",
    "kk..www..kk",
    "kkwwwKwwwkk",
    "kk..www..kk",
    "kk.......kk",
    "kk.......kk",
];
fn pal_tie(ch: u8) -> Option<Color32> {
    Some(match ch {
        b'k' => c("#5c6372"),
        b'K' => c("#15171c"),
        b'w' => c("#aeb4bd"),
        _ => return None,
    })
}

const HEART: Art = &[".r.r.", "rrrrr", ".rrr.", "..r.."];
fn pal_heart(ch: u8) -> Option<Color32> {
    (ch == b'r').then(|| c("#e5476a"))
}

#[derive(Clone, Copy)]
struct Opt {
    flip: bool,
    angle: f32,
    alpha: f32,
    px: f32,
}

impl Default for Opt {
    fn default() -> Self {
        Self {
            flip: false,
            angle: 0.0,
            alpha: 1.0,
            px: 3.0,
        }
    }
}

/// Desenha um boneco de qualquer tamanho, com o canto de cima à esquerda em `at`.
fn paint_art(p: &Painter, art: &[&str], pal: Pal, at: Pos2, o: Opt) {
    let cols = art.first().map_or(0, |r| r.len()) as f32;
    let half = vec2(cols, art.len() as f32) * o.px / 2.0;
    let (sin, cos) = o.angle.sin_cos();
    let map = |col: f32, row: f32| {
        let col = if o.flip { cols - col } else { col };
        let v = vec2(col, row) * o.px - half;
        at + half + vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    };
    let mut mesh = egui::Mesh::default();
    for (y, row) in art.iter().enumerate() {
        let b = row.as_bytes();
        let mut x = 0;
        while x < b.len() {
            let ch = b[x];
            let mut end = x + 1;
            while end < b.len() && b[end] == ch {
                end += 1;
            }
            if let Some(col) = pal(ch) {
                let col = col.gamma_multiply(o.alpha);
                let (t, u) = (y as f32, y as f32 + 1.0);
                let i = mesh.vertices.len() as u32;
                mesh.colored_vertex(map(x as f32, t), col);
                mesh.colored_vertex(map(end as f32, t), col);
                mesh.colored_vertex(map(end as f32, u), col);
                mesh.colored_vertex(map(x as f32, u), col);
                mesh.add_triangle(i, i + 1, i + 2);
                mesh.add_triangle(i, i + 2, i + 3);
            }
            x = end;
        }
    }
    p.add(mesh);
}

/// Número "sorteado" de 0 a 1, sempre o mesmo pra cada `i`.
fn rnd(i: u32) -> f32 {
    (i.wrapping_add(1).wrapping_mul(2654435761) >> 16 & 0xffff) as f32 / 65535.0
}

/// Altura de um arco de salto, com `u` de 0 a 1.
fn arc(u: f32, height: f32) -> f32 {
    if (0.0..=1.0).contains(&u) {
        (u * PI).sin() * height
    } else {
        0.0
    }
}

/// Distância de quem entra pela esquerda, para em `stay` e sai no fim.
fn visit(t: f32, dur: f32, stay: f32) -> (f32, f32, bool) {
    const FAR: f32 = 180.0;
    const WALK: f32 = 0.8;
    let gap = if t < WALK {
        lerp(FAR, stay, ease(t / WALK))
    } else if t > dur - WALK {
        lerp(stay, FAR, ease((t - dur + WALK) / WALK))
    } else {
        stay
    };
    let alpha = (t / 0.2).min(1.0) * ((dur - t) / 0.3).clamp(0.0, 1.0);
    (gap, alpha, t < WALK || t > dur - WALK)
}

fn walk_legs(t: f32) -> &'static Legs {
    if ((t * 8.0) as u32).is_multiple_of(2) {
        &LEGS_WALK_A
    } else {
        &LEGS_WALK_B
    }
}

struct Cx<'a> {
    p: &'a Painter,
    t: f32,
    base: Pos2,
    screen: Rect,
}

impl Cx<'_> {
    fn ground(&self) -> f32 {
        self.base.y + HEIGHT
    }

    /// O Anakin olhando pra esquerda, deslocado de `off`.
    fn me(&self, off: egui::Vec2) -> Place {
        Place::new(self.base + off, true)
    }

    fn draw_me(&self, place: Place, saber: Option<f32>) {
        paint_sprite(self.p, rows(&HEAD, &BODY, &LEGS_STAND, false), place);
        if let Some(a) = saber {
            paint_saber(self.p, place.hand(), place.blade(a), 44.0, false, 1.0);
        }
    }

    /// De pé no chão, a `gap` à esquerda do Anakin. Devolve o canto do boneco.
    fn feet(&self, art_rows: usize, gap: f32) -> Pos2 {
        pos2(self.base.x - gap, self.ground() - art_rows as f32 * 3.0)
    }

    fn pod(&self) -> Option<Pos2> {
        let t = self.t;
        let y = self.ground() - 30.0 + (t * 14.0).sin() * 2.0;
        let width = POD[0].len() as f32 * 3.0;
        let out = self.screen.left() - width - 10.0;
        let (x, flip) = if t < 1.6 {
            let u = (t - 0.1) / 1.4;
            (lerp(self.screen.right() + 10.0, out, u * u), true)
        } else if t < 3.1 {
            let u = (t - 1.7) / 1.4;
            (lerp(out, self.base.x, ease(u)), false)
        } else {
            let u = (t - 3.1) / 0.5;
            (lerp(self.base.x, self.screen.right() + 10.0, u * u), false)
        };
        let boarded = (t >= 0.1 && x < self.base.x + WIDTH) || t >= 1.6;
        if !boarded || t >= 3.1 {
            let hop = arc((t - 3.1) / 0.6, 22.0);
            self.draw_me(self.me(vec2(0.0, -hop)), None);
        }
        let behind = if flip { width } else { 0.0 };
        let back = if flip { 1.0 } else { -1.0 };
        for i in 0..14u32 {
            let d = i as f32 * 9.0 + rnd(i) * 6.0;
            self.p.rect_filled(
                Rect::from_center_size(
                    pos2(x + behind + back * d, y + 22.0 + rnd(i + 40) * 6.0),
                    vec2(3.0, 3.0),
                ),
                0.0,
                c("#b89b6a").gamma_multiply(0.7 - i as f32 * 0.045),
            );
        }
        paint_art(
            self.p,
            POD,
            pal_pod,
            pos2(x, y),
            Opt {
                flip,
                ..Default::default()
            },
        );
        None
    }

    fn sand(&self) -> Option<Pos2> {
        let t = self.t;
        let raining = t < 2.5;
        let shaking = (2.5..3.6).contains(&t);
        let dx = if shaking { (t * 30.0).sin() * 2.0 } else { 0.0 };
        self.draw_me(self.me(vec2(dx, 0.0)), None);
        let sand = c("#d6b97c");
        if raining {
            let h = HEIGHT + 40.0;
            for i in 0..40u32 {
                let x = self.base.x - 15.0 + rnd(i) * (WIDTH + 30.0);
                let v = 50.0 + rnd(i + 90) * 50.0;
                let y = (t * v + rnd(i + 200) * h) % h;
                self.p.rect_filled(
                    Rect::from_min_size(pos2(x, self.base.y - 40.0 + y), vec2(2.0, 2.0)),
                    0.0,
                    sand,
                );
            }
        }
        if shaking {
            // sacudindo: os grãos voam pros lados
            let u = (t - 2.5) / 1.1;
            for i in 0..24u32 {
                let a = rnd(i) * TAU;
                let r = u * (20.0 + rnd(i + 30) * 50.0);
                let center = self.base + vec2(WIDTH / 2.0, HEIGHT / 2.0);
                self.p.rect_filled(
                    Rect::from_min_size(center + vec2(a.cos(), a.sin()) * r, vec2(2.0, 2.0)),
                    0.0,
                    sand.gamma_multiply(1.0 - u),
                );
            }
        }
        let pile = if raining {
            t / 2.5
        } else {
            1.0 - ((t - 2.5) / 1.1).clamp(0.0, 1.0)
        };
        for i in 0..7 {
            let k = 1.0 - (i as f32 - 3.0).abs() / 4.0;
            let h = pile * k * 10.0;
            self.p.rect_filled(
                Rect::from_min_size(
                    pos2(self.base.x + 3.0 + i as f32 * 6.0, self.ground() - h),
                    vec2(6.0, h),
                ),
                0.0,
                sand,
            );
        }
        None
    }

    fn small(&self) -> Option<Pos2> {
        let t = self.t;
        const POINTS: [f32; 6] = [190.0, 115.0, 60.0, 125.0, 58.0, 190.0];
        let seg = ((t / 1.0) as usize).min(POINTS.len() - 2);
        let u = (t / 1.0 - seg as f32).clamp(0.0, 1.0);
        let gap = lerp(POINTS[seg], POINTS[seg + 1], u);
        let hop = arc(u, 52.0);
        let h = SMALL.len() as f32 * 3.0;
        let at = pos2(self.base.x - gap, self.ground() - h - hop);
        let spin = if seg.is_multiple_of(2) {
            ease(u) * TAU
        } else {
            -ease(u) * TAU
        };
        let alpha = (t / 0.2).min(1.0) * ((5.5 - t) / 0.4).clamp(0.0, 1.0);
        let block = 0.25 + (t * 5.0).sin().abs() * 0.5;
        self.draw_me(self.me(Default::default()), Some(block));
        paint_art(
            self.p,
            SMALL,
            pal_small,
            at,
            Opt {
                angle: spin,
                alpha,
                ..Default::default()
            },
        );
        let center = at + vec2(18.0, 20.0);
        let (sin, cos) = spin.sin_cos();
        let hand = center + vec2(14.0 * cos, 14.0 * sin);
        paint_saber_col(self.p, hand, spin + 0.5, 30.0, GREEN, alpha);
        if gap < 70.0 && u > 0.8 {
            paint_sparks(
                self.p,
                self.base + vec2(0.0, 12.0),
                (u - 0.8) * 0.5,
                seg as u32,
            );
        }
        Some(pos2(at.x, at.y - 8.0))
    }

    fn astro(&self) -> Option<Pos2> {
        let t = self.t;
        let (gap, alpha, moving) = visit(t, 5.2, 90.0);
        let at = self.feet(ASTRO.len(), gap);
        let wobble = if moving { (t * 14.0).sin() * 0.1 } else { 0.0 };
        let happy = (t * 12.0).sin() * 3.0 * f32::from(u8::from((1.4..3.2).contains(&t)));
        let hop = arc((t - 3.3) / 0.5, 18.0);
        self.draw_me(self.me(vec2(0.0, -hop)), None);
        paint_art(
            self.p,
            ASTRO,
            pal_astro,
            at + vec2(happy, 0.0),
            Opt {
                angle: wobble,
                alpha,
                ..Default::default()
            },
        );
        if (1.4..3.2).contains(&t) {
            // notinhas saindo do domo
            for i in 0..3 {
                let u = ((t - 1.4) * 0.9 + i as f32 / 3.0).fract();
                let n = at + vec2(16.0 + (i as f32 - 1.0) * 10.0, -u * 26.0);
                self.p.rect_filled(
                    Rect::from_center_size(n, vec2(3.0, 3.0)),
                    0.0,
                    c("#5bb6ff").gamma_multiply(1.0 - u),
                );
            }
        }
        Some(at)
    }

    fn angel(&self) -> Option<Pos2> {
        let t = self.t;
        let (gap, alpha, _) = visit(t, 5.5, 72.0);
        let at = self.feet(ANGEL.len(), gap);
        paint_art(
            self.p,
            ANGEL,
            pal_angel,
            at,
            Opt {
                alpha,
                ..Default::default()
            },
        );
        self.draw_me(self.me(Default::default()), None);
        if t > 0.8 {
            for i in 0..6u32 {
                let u = ((t - 0.8) * 0.5 + i as f32 / 6.0).fract();
                let n = self.base
                    + vec2(
                        -6.0 - 8.0 * i as f32 + (u * 9.0 + i as f32).sin() * 5.0,
                        6.0 - u * 50.0,
                    );
                paint_art(
                    self.p,
                    HEART,
                    pal_heart,
                    n,
                    Opt {
                        alpha: (1.0 - u) * alpha,
                        px: 2.0,
                        ..Default::default()
                    },
                );
            }
        }
        Some(at)
    }

    fn lava(&self) -> Option<Pos2> {
        let t = self.t;
        let dur = 6.2;
        let level = ease(t / 1.0) * ease((dur - t) / 0.8) * 22.0;
        let vis = ease(t / 0.5) * ease((dur - t) / 0.5);
        // a pedra do outro lado, mais alta
        let rock = pos2(self.base.x - 170.0, self.ground());
        self.p.add(egui::Shape::convex_polygon(
            vec![
                rock + vec2(-16.0, 0.0),
                rock + vec2(0.0, -46.0),
                rock + vec2(56.0, -46.0),
                rock + vec2(96.0, 0.0),
            ],
            c("#2b2522").gamma_multiply(vis),
            Stroke::NONE,
        ));
        let obi_at = pos2(rock.x + 4.0, self.ground() - 46.0 - HEIGHT);
        let mut obi = Place::new(obi_at, false);
        obi.alpha = vis;
        paint_sprite(self.p, rows(&OBI_HEAD, &OBI_BODY, &LEGS_STAND, false), obi);
        paint_saber(self.p, obi.hand(), obi.blade(0.2), 40.0, true, vis);
        // salta, não alcança e volta
        let u = (t - 2.6) / 0.9;
        let leap = (0.0..=1.0).contains(&u);
        let back = if leap { (u * PI).sin() * 70.0 } else { 0.0 };
        let hot = if leap {
            arc(u, 60.0)
        } else {
            (t * 7.0).sin().abs() * 9.0 * f32::from(u8::from(t > 0.8))
        };
        let mut me = self.me(vec2(-back, -hot));
        me.sith = true;
        if leap {
            me.angle = ease(u) * TAU;
        }
        self.draw_me(me, Some(0.2));
        // a lava, por cima dos pés
        let mut x = self.screen.left();
        while x < self.screen.right() {
            let h = level + (x * 0.1 + t * 3.0).sin() * 2.0 * vis;
            let col = if ((x / 8.0) as i32).rem_euclid(3) == 0 {
                c("#ff9a2e")
            } else {
                c("#e8501f")
            };
            self.p.rect_filled(
                Rect::from_min_max(
                    pos2(x, self.ground() - h),
                    pos2(x + 8.0, self.ground() + 8.0),
                ),
                0.0,
                col.gamma_multiply(vis),
            );
            x += 8.0;
        }
        for i in 0..14u32 {
            let u = (t * (0.3 + rnd(i) * 0.5) + rnd(i + 9)).fract();
            let px = self.base.x + WIDTH - rnd(i + 50) * 300.0;
            self.p.rect_filled(
                Rect::from_center_size(pos2(px, self.ground() - level - u * 60.0), vec2(2.0, 2.0)),
                0.0,
                c("#ffb347").gamma_multiply((1.0 - u) * vis),
            );
        }
        Some(obi_at)
    }

    fn moon(&self) -> Option<Pos2> {
        let t = self.t;
        let s = self.screen;
        let u = t / 8.0;
        let center = pos2(
            lerp(s.right() + 60.0, s.left() - 60.0, u),
            s.top() + s.height() * 0.3,
        );
        let r = 46.0;
        let p = self.p;
        p.circle_filled(center, r, c("#6e7480"));
        p.line_segment(
            [center - vec2(r, 0.0), center + vec2(r, 0.0)],
            Stroke::new(3.0, c("#3c414c")),
        );
        let dish = center + vec2(-16.0, -18.0);
        p.circle_filled(dish, 13.0, c("#4c5362"));
        p.circle_filled(dish, 4.0, c("#2b2f38"));
        let target = pos2(self.base.x - 140.0, self.ground());
        if (3.5..4.2).contains(&t) {
            p.line_segment(
                [dish, target],
                Stroke::new(5.0, c("#57d13f").gamma_multiply(0.35)),
            );
            p.line_segment([dish, target], Stroke::new(2.0, c("#caffb8")));
        }
        if (4.2..5.2).contains(&t) {
            let k = t - 4.2;
            p.circle_filled(target, 8.0 + k * 42.0, c("#ff9a2e").gamma_multiply(1.0 - k));
            p.circle_filled(target, k * 22.0, c("#fff4d6").gamma_multiply(1.0 - k));
        }
        let scared = arc((t - 4.2) / 0.5, 14.0);
        self.draw_me(self.me(vec2(0.0, -scared)), None);
        None
    }

    fn dogfight(&self) -> Option<Pos2> {
        let t = self.t;
        let s = self.screen;
        let u = t / 3.2;
        let y = s.top() + s.height() * 0.45 + (t * 3.0).sin() * 24.0;
        let x = lerp(s.left() - 60.0, s.right() + 140.0, u);
        let tie_x = x - 110.0;
        let tie_y = s.top() + s.height() * 0.45 + ((t - 0.3) * 3.0).sin() * 24.0;
        self.draw_me(self.me(Default::default()), None);
        paint_art(self.p, XWING, pal_xwing, pos2(x, y), Opt::default());
        paint_art(self.p, TIE, pal_tie, pos2(tie_x, tie_y), Opt::default());
        for i in 0..4 {
            let k = (t * 2.8 + i as f32 * 0.25).fract();
            let a = pos2(lerp(tie_x + 33.0, x, k), lerp(tie_y + 10.0, y + 10.0, k));
            self.p
                .line_segment([a, a + vec2(10.0, 0.0)], Stroke::new(2.0, c("#57d13f")));
        }
        None
    }

    fn roger(&self) -> Option<Pos2> {
        let t = self.t;
        let gap = lerp(180.0, 130.0, ease(t / 1.0));
        let at = pos2(self.base.x - gap, self.base.y);
        const SHOTS: [f32; 4] = [1.2, 1.9, 2.6, 3.3];
        let mut swing: f32 = 0.2;
        let muzzle = at + vec2(WIDTH, 36.0);
        let target = self.base + vec2(4.0, 30.0);
        for (i, s) in SHOTS.iter().enumerate() {
            let k = t - s;
            if (0.0..0.35).contains(&k) {
                let a = muzzle + (target - muzzle) * (k / 0.35);
                self.p
                    .line_segment([a, a + vec2(10.0, 0.0)], Stroke::new(2.0, c("#ff6a3a")));
            }
            if (0.25..0.55).contains(&k) {
                swing = 0.2 + ((k - 0.25) / 0.3 * PI).sin() * 0.9;
            }
            if (0.35..0.47).contains(&k) {
                paint_sparks(self.p, target, k - 0.35, i as u32);
            }
            // o último volta certeiro na cabeça
            let dest = if i == 3 {
                at + vec2(24.0, 8.0)
            } else {
                at + vec2(-40.0, -30.0 + 16.0 * i as f32)
            };
            if (0.35..0.6).contains(&k) {
                let a = target + (dest - target) * ((k - 0.35) / 0.25);
                self.p
                    .line_segment([a, a - vec2(10.0, 0.0)], Stroke::new(2.0, c("#ff6a3a")));
            }
        }
        let hit = t >= 3.9;
        let alpha = (t / 0.2).min(1.0) * ((6.0 - t) / 0.6).clamp(0.0, 1.0);
        let head = 7;
        if hit {
            let u = t - 3.9;
            let fly = pos2(
                at.x - u * 60.0,
                at.y - (u * 2.6).sin().max(0.0) * 44.0 + (u * u * 18.0).min(40.0),
            );
            paint_art(
                self.p,
                &ROGER[..head],
                pal_roger,
                fly,
                Opt {
                    angle: u * 8.0,
                    alpha,
                    ..Default::default()
                },
            );
            let fall = ease((t - 4.1) / 0.5);
            paint_art(
                self.p,
                &ROGER[head..],
                pal_roger,
                at + vec2(0.0, head as f32 * 3.0 + fall * 16.0),
                Opt {
                    angle: -fall * PI / 2.0,
                    alpha,
                    ..Default::default()
                },
            );
        } else {
            paint_art(
                self.p,
                ROGER,
                pal_roger,
                at,
                Opt {
                    alpha,
                    ..Default::default()
                },
            );
        }
        self.draw_me(self.me(Default::default()), Some(swing));
        Some(at)
    }

    fn zigzag(&self, from: Pos2, to: Pos2, seed: u32) {
        let normal = (to - from).normalized().rot90();
        let points: Vec<Pos2> = (0..=8)
            .map(|i| {
                let u = i as f32 / 8.0;
                let off = if i == 0 || i == 8 {
                    0.0
                } else {
                    (rnd(seed.wrapping_mul(13) + i) - 0.5) * 18.0
                };
                from + (to - from) * u + normal * off
            })
            .collect();
        self.p.add(egui::Shape::line(
            points.clone(),
            Stroke::new(4.0, c("#6fb4ff").gamma_multiply(0.35)),
        ));
        self.p
            .add(egui::Shape::line(points, Stroke::new(1.5, c("#e8f4ff"))));
    }

    fn power(&self) -> Option<Pos2> {
        let t = self.t;
        let (gap, alpha, _) = visit(t, 6.0, 120.0);
        let at = pos2(self.base.x - gap, self.base.y);
        let casting = (1.2..3.8).contains(&t);
        let back = (3.8..4.6).contains(&t);
        let shake = if casting { (t * 34.0).sin() * 1.5 } else { 0.0 };
        let his = if back { (t * 34.0).sin() * 2.0 } else { 0.0 };
        paint_art(
            self.p,
            POWER,
            pal_power,
            at + vec2(his, 0.0),
            Opt {
                alpha,
                ..Default::default()
            },
        );
        let mut me = self.me(vec2(shake, 0.0));
        me.sith = casting || back;
        let guard = if casting || back { 0.55 } else { 0.1 };
        self.draw_me(me, Some(guard));
        let hand = at + vec2(WIDTH - 2.0, 35.0);
        let blade = me.hand() + vec2(-guard.sin(), -guard.cos()) * 26.0;
        let frame = (t * 16.0) as u32;
        if casting {
            for k in 0..3 {
                self.zigzag(hand, blade, frame + k * 7);
            }
        }
        if back {
            for k in 0..3 {
                self.zigzag(blade, at + vec2(24.0, 20.0), frame + k * 7);
            }
        }
        Some(at)
    }

    fn general(&self) -> Option<Pos2> {
        let t = self.t;
        let gap = if t < 3.5 {
            lerp(220.0, 92.0, ease((t - 1.8) / 1.7))
        } else if t < 4.8 {
            92.0
        } else {
            lerp(92.0, 240.0, ease((t - 4.8) / 1.0))
        };
        let alpha = ((t - 1.6) / 0.2).clamp(0.0, 1.0) * ((6.0 - t) / 0.3).clamp(0.0, 1.0);
        let at = pos2(self.base.x - gap, self.base.y);
        paint_art(
            self.p,
            GENERAL,
            pal_general,
            at,
            Opt {
                alpha,
                ..Default::default()
            },
        );
        if alpha > 0.0 {
            let center = at + vec2(24.0, 30.0);
            let colors = [GREEN, BLUE, GREEN, BLUE];
            for (i, col) in colors.into_iter().enumerate() {
                let a = t * 11.0 + i as f32 * TAU / 4.0;
                let hand = center + vec2(a.sin(), -a.cos()) * 10.0;
                paint_saber_col(self.p, hand, a, 32.0, col, alpha);
            }
        }
        let block = 0.5 + (t * 6.0).sin() * 0.25;
        self.draw_me(self.me(Default::default()), Some(block));
        if (3.0..4.8).contains(&t) {
            let k = (t * 4.0).fract();
            if k < 0.4 {
                paint_sparks(
                    self.p,
                    self.base + vec2(-14.0, 14.0),
                    k * 0.3,
                    (t * 4.0) as u32,
                );
            }
        }
        Some(at)
    }

    fn boomerang(&self) -> Option<Pos2> {
        let t = self.t;
        let u = (t - 0.4) / 2.4;
        let flying = (0.0..1.0).contains(&u);
        let me = self.me(Default::default());
        if flying {
            paint_sprite(self.p, rows(&HEAD, &BODY_FORCE, &LEGS_STAND, false), me);
            let hand = me.point(15.5, 11.5);
            let pos = hand
                + vec2(
                    -(u * PI).sin() * 210.0,
                    -(u * TAU).sin() * 14.0 - (u * PI).sin() * 26.0,
                );
            let spin = t * 22.0;
            let d = vec2(spin.sin(), -spin.cos());
            paint_saber(self.p, pos - d * 16.0, spin, 34.0, false, 1.0);
        } else {
            let hop = arc((t - 2.8) / 0.5, 16.0);
            self.draw_me(self.me(vec2(0.0, -hop)), Some(0.0));
        }
        None
    }

    fn holo(&self) -> Option<Pos2> {
        let t = self.t;
        let (gap, alpha, _) = visit(t, 5.6, 110.0);
        let at = self.feet(ASTRO.len(), gap);
        paint_art(
            self.p,
            ASTRO,
            pal_astro,
            at,
            Opt {
                alpha,
                ..Default::default()
            },
        );
        self.draw_me(self.me(Default::default()), None);
        let on = (0.8..4.7).contains(&t);
        let mut bubble_at = at;
        if on {
            let flicker = 0.55 + rnd((t * 15.0) as u32) * 0.3;
            let px = 1.6;
            let (w, h) = (16.0 * px, 21.0 * px);
            let source = at + vec2(16.0, 10.0);
            let top = pos2(source.x - w / 2.0 + 30.0, source.y - h - 14.0);
            self.p.add(egui::Shape::convex_polygon(
                vec![source, top + vec2(0.0, h), top + vec2(w, h)],
                c("#5bb6ff").gamma_multiply(0.14),
                Stroke::NONE,
            ));
            paint_art(
                self.p,
                ANGEL,
                pal_holo,
                top,
                Opt {
                    alpha: flicker,
                    px,
                    ..Default::default()
                },
            );
            let mut y = top.y;
            while y < top.y + h {
                self.p.line_segment(
                    [pos2(top.x, y), pos2(top.x + w, y)],
                    Stroke::new(1.0, c("#0a0e14").gamma_multiply(0.45)),
                );
                y += 3.0;
            }
            bubble_at = pos2(at.x, top.y);
        }
        Some(bubble_at)
    }

    fn fur(&self) -> Option<Pos2> {
        let t = self.t;
        let (gap, alpha, moving) = visit(t, 5.5, 58.0);
        let total = FUR.len() + FUR_A.len();
        let at = self.feet(total, gap);
        let legs = if moving && ((t * 6.0) as u32).is_multiple_of(2) {
            FUR_A
        } else {
            FUR_B
        };
        let o = Opt {
            alpha,
            ..Default::default()
        };
        paint_art(self.p, FUR, pal_fur, at, o);
        paint_art(
            self.p,
            legs,
            pal_fur,
            at + vec2(0.0, FUR.len() as f32 * 3.0),
            o,
        );
        // o abraço levanta o Anakin do chão
        let lift = ease((t - 2.6) / 0.3) * ease((4.2 - t) / 0.3) * 18.0;
        self.draw_me(self.me(vec2(0.0, -lift)), None);
        Some(at)
    }

    fn horns(&self) -> Option<Pos2> {
        let t = self.t;
        let (gap, alpha, _) = visit(t, 6.0, 82.0);
        let at = pos2(self.base.x - gap, self.base.y);
        paint_art(
            self.p,
            HORNS,
            pal_horns,
            at,
            Opt {
                alpha,
                ..Default::default()
            },
        );
        let fighting = (1.4..5.0).contains(&t);
        let spin = if fighting {
            0.7 + (t * 7.0).sin() * 0.5
        } else {
            t * 12.0
        };
        let center = at + vec2(32.0, 39.0);
        let d = vec2(spin.sin(), -spin.cos());
        paint_saber_col(self.p, center + d * 5.0, spin, 34.0, RED, alpha);
        paint_saber_col(self.p, center - d * 5.0, spin + PI, 34.0, RED, alpha);
        let u = (t - 2.9) / 0.8;
        let flipping = (0.0..=1.0).contains(&u);
        let mut me = self.me(vec2(0.0, -arc(u, 34.0)));
        me.sith = true;
        if flipping {
            me.angle = ease(u) * TAU;
        }
        self.draw_me(me, Some(0.6 + (t * 7.0).sin() * 0.3));
        if fighting && !flipping {
            let k = (t * 3.5).fract();
            if k < 0.35 {
                paint_sparks(
                    self.p,
                    self.base + vec2(-16.0, 12.0),
                    k * 0.3,
                    (t * 3.5) as u32,
                );
            }
        }
        Some(at)
    }

    fn pram(&self) -> Option<Pos2> {
        let t = self.t;
        let (gap, alpha, _) = visit(t, 5.8, 96.0);
        let bob = (t * 2.5).sin() * 3.0;
        let at = pos2(
            self.base.x - gap,
            self.ground() - PRAM.len() as f32 * 3.0 - 14.0 + bob,
        );
        paint_art(
            self.p,
            PRAM,
            pal_pram,
            at,
            Opt {
                alpha,
                ..Default::default()
            },
        );
        let lifted = (1.2..3.8).contains(&t);
        let lift = ease((t - 1.2) / 0.5) * ease((3.8 - t) / 0.3) * 34.0;
        let wobble = if lifted { (t * 3.0).sin() * 0.25 } else { 0.0 };
        let mut me = self.me(vec2(0.0, -lift));
        me.angle = wobble;
        let legs = if lifted {
            walk_legs(t * 1.6)
        } else {
            &LEGS_STAND
        };
        paint_sprite(self.p, rows(&HEAD, &BODY, legs, false), me);
        if lifted {
            paint_force(
                self.p,
                at + vec2(30.0, 12.0),
                self.base + vec2(10.0, 26.0 - lift),
                t,
            );
        }
        Some(at)
    }

    fn clumsy(&self) -> Option<Pos2> {
        let t = self.t;
        let gap = if t < 1.6 {
            lerp(190.0, 90.0, t / 1.6)
        } else if t < 3.6 {
            90.0
        } else {
            lerp(90.0, 200.0, ease((t - 3.6) / 1.2))
        };
        let fall = ease((t - 1.6) / 0.4) * ease((3.6 - t) / 0.3);
        let at = self.feet(CLUMSY.len(), gap) + vec2(0.0, fall * 7.0);
        let hop = arc((t - 2.0) / 0.3, 8.0);
        let walking = !(1.6..3.6).contains(&t);
        let bob = if walking {
            (t * 11.0).sin().abs() * 3.0
        } else {
            0.0
        };
        let alpha = (t / 0.2).min(1.0) * ((5.0 - t) / 0.3).clamp(0.0, 1.0);
        paint_art(
            self.p,
            CLUMSY,
            pal_clumsy,
            at - vec2(0.0, hop + bob),
            Opt {
                angle: fall * PI / 2.0,
                alpha,
                ..Default::default()
            },
        );
        self.draw_me(self.me(Default::default()), None);
        Some(at)
    }

    fn sleep(&self) -> Option<Pos2> {
        let t = self.t;
        let down = ease(t / 0.6) * ease((9.0 - t) / 0.6);
        let mut me = self.me(vec2(0.0, down * 8.0));
        me.angle = -down * PI / 2.0;
        paint_sprite(self.p, rows(&HEAD, &BODY, &LEGS_STAND, down > 0.5), me);
        if t > 0.8 && t < 8.3 {
            let head = me.point(8.0, 2.0);
            for i in 0..3 {
                let u = ((t - 0.8) * 0.4 + i as f32 / 3.0).fract();
                self.p.text(
                    head + vec2(-4.0 + (u * 7.0).sin() * 5.0 + u * 8.0, -8.0 - u * 34.0),
                    egui::Align2::CENTER_CENTER,
                    "z",
                    FontId::monospace(10.0 + u * 8.0),
                    c("#9fb7d8").gamma_multiply(1.0 - u),
                );
            }
        }
        None
    }

    fn fireworks(&self) -> Option<Pos2> {
        let t = self.t;
        let s = self.screen;
        const COLORS: [&str; 5] = ["#ff5a5a", "#ffd34d", "#5bb6ff", "#7ee081", "#e58aff"];
        for b in 0..11u32 {
            let start = b as f32 * 0.55;
            let k = t - start;
            if !(0.0..1.2).contains(&k) {
                continue;
            }
            let at = pos2(
                lerp(s.left() + s.width() * 0.25, s.right() - 40.0, rnd(b * 3)),
                s.top() + s.height() * (0.15 + rnd(b * 5 + 1) * 0.4),
            );
            let col = c(COLORS[b as usize % COLORS.len()]);
            for i in 0..40u32 {
                let a = i as f32 / 40.0 * TAU;
                let ring = if i.is_multiple_of(2) { 1.0 } else { 0.6 };
                let r = ease(k / 0.7) * (46.0 + rnd(i) * 18.0) * ring;
                let pos = at + vec2(a.cos(), a.sin()) * r + vec2(0.0, k * k * 12.0);
                self.p.rect_filled(
                    Rect::from_center_size(pos, vec2(3.0, 3.0)),
                    0.0,
                    col.gamma_multiply((1.0 - (k - 0.5).max(0.0) / 0.7).max(0.0)),
                );
            }
        }
        let hop = (t * 5.0).sin().abs() * 12.0;
        self.draw_me(self.me(vec2(0.0, -hop)), Some(0.0));
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desenhos_sao_retangulares_e_tem_cor() {
        let arts: [(Art, Pal); 16] = [
            (ASTRO, pal_astro),
            (SMALL, pal_small),
            (ANGEL, pal_angel),
            (ANGEL, pal_holo),
            (ROGER, pal_roger),
            (POWER, pal_power),
            (GENERAL, pal_general),
            (FUR, pal_fur),
            (FUR_A, pal_fur),
            (FUR_B, pal_fur),
            (HORNS, pal_horns),
            (CLUMSY, pal_clumsy),
            (PRAM, pal_pram),
            (POD, pal_pod),
            (XWING, pal_xwing),
            (TIE, pal_tie),
        ];
        for (art, pal) in arts {
            let w = art[0].len();
            for r in art {
                assert_eq!(r.len(), w, "{r}");
                for ch in r.bytes().filter(|ch| *ch != b'.') {
                    assert!(pal(ch).is_some(), "sem cor: {} em {r}", ch as char);
                }
            }
        }
        assert!(HEART.iter().all(|r| r.len() == 5));
    }

    #[test]
    fn todo_o_rodizio_esta_na_lista() {
        for e in ROTATION {
            assert!(ALL.contains(&e));
            assert!(e.duration() > 1.0);
        }
    }
}
