//! O Anakin no canto da tela: um boneco em pixel art (desenhado aqui, sem imagem de filme) que
//! anda de vez em quando, pisca, pula quando uma sessão acaba, acende o sabre vermelho enquanto
//! alguém trabalha e solta falas num balão. Clicar abre a conversa.
//!
//! De vez em quando ele apronta uma cena: duelo com o Obi-Wan, mortal, Força num stormtrooper ou
//! uns passos pesados de Vader, além de outras surpresas (`avatar_eggs`). Os outros bonecos são só desenho: o clique passa direto por eles.
//!
//! Custo: parado ele só acorda quando a próxima travessura vence. Só anima em surtos curtos, com
//! a janela em foco.

use std::time::{Duration, Instant};

use aic_core::SessionState;
use egui::{Color32, Context, FontId, Id, Order, Pos2, Rect, Sense, Stroke, pos2, vec2};

use super::avatar_eggs::{self as eggs, Egg};
use super::avatar_rivals::{self as rivals, Rival};
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
/// Intervalo entre quadros enquanto anima.
const ANIM_FRAME: Duration = Duration::from_millis(125);
/// Intervalo entre quadros durante uma cena: precisa ser fluido, e dura poucos segundos.
const SCENE_FRAME: Duration = Duration::from_millis(33);
const VADER_SPEED: f32 = 14.0;
const FLIP_SPEED: f32 = 55.0;
/// Janela mais estreita que isso não tem espaço pro outro boneco.
const MIN_SCENE_WIDTH: f32 = 420.0;
const SABER_RED: &str = "#e8262d";
const SABER_RED_CORE: &str = "#ffe2df";
const SABER_BLUE: &str = "#3f9dff";
const SABER_BLUE_CORE: &str = "#eaf6ff";

const FALAS: [&str; 5] = [
    "This is where the fun begins.",
    "Now this is podracing!",
    "I don't like sand.",
    "Are you an angel?",
    "I've got a bad feeling about this.",
];

pub(super) const HEAD: [&str; 9] = [
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
pub(super) const BODY: [&str; 7] = [
    "...ddbttttbdd...",
    "..dddbttttbddd..",
    "..dbdbbttbbdbd..",
    "..dbddBBBBddbd..",
    "..sb.dBggBd.bs..",
    "..g..dddddd..g..",
    ".....dddddd.....",
];
pub(super) const LEGS_STAND: [&str; 5] = [
    ".....dd..dd.....",
    ".....pp..pp.....",
    ".....pp..pp.....",
    ".....pp..pp.....",
    "....kkk..kkk....",
];
pub(super) const LEGS_WALK_A: [&str; 5] = [
    ".....dd..dd.....",
    "....pp....pp....",
    "....pp....pp....",
    "...kpp.....pp...",
    "...kk.....kkk...",
];
pub(super) const LEGS_WALK_B: [&str; 5] = [
    ".....dd..dd.....",
    ".....pp..pp.....",
    "......pppp......",
    "......pp.pp.....",
    ".....kkk.kkk....",
];

/// Com o braço estendido, usando a Força.
pub(super) const BODY_FORCE: [&str; 7] = [
    "...ddbttttbdd...",
    "..dddbttttbddd..",
    "..dbdbbttbbdddss",
    "..dbddBBBBdd....",
    "..sb.dBggBd.....",
    "..g..dddddd.....",
    ".....dddddd.....",
];

pub(super) const OBI_HEAD: [&str; 9] = [
    ".....oooooo.....",
    "...ooOOOOOOoo...",
    "..oOOOOOOOOOOo..",
    "..oOossssssoOo..",
    "..oossssssssoo..",
    "..oosesssesoo...",
    "..oossssssssoo..",
    "...oOOnnOOoo....",
    ".....OOOOOO.....",
];
pub(super) const OBI_BODY: [&str; 7] = [
    "...TTbWWWWbTT...",
    "..TTTbWWWWbTTT..",
    "..TbTbbWWbbTbT..",
    "..TbTTBBBBTTbT..",
    "..sb.TBggBT.bs..",
    "..g..TTTTTT..g..",
    ".....TTTTTT.....",
];

const TROOPER_HEAD: [&str; 9] = [
    ".....wwwwww.....",
    "...wwWWWWWWww...",
    "..wWWWWWWWWWWw..",
    "..wWWWWWWWWWWw..",
    "..wWggWWWWggWw..",
    "..wWggWWWWggWw..",
    "..wWWWWwwWWWWw..",
    "...wWggWWggWw...",
    ".....wWWWWw.....",
];
const TROOPER_BODY: [&str; 7] = [
    "...WWwWWWWwWW...",
    "..WWWwWWWWwWWW..",
    "..WWWWwggwWWWW..",
    "..WWWggggggWWW..",
    "..Wg.WwggwW.gW..",
    "..g..WWWWWW..g..",
    ".....WWWWWW.....",
];
const TROOPER_STAND: [&str; 5] = [
    ".....WW..WW.....",
    ".....WW..WW.....",
    ".....Ww..Ww.....",
    ".....WW..WW.....",
    "....ggg..ggg....",
];
const TROOPER_WALK_A: [&str; 5] = [
    ".....WW..WW.....",
    "....WW....WW....",
    "....Ww....Ww....",
    "...gWW.....WW...",
    "...gg.....ggg...",
];
const TROOPER_WALK_B: [&str; 5] = [
    ".....WW..WW.....",
    ".....WW..WW.....",
    "......WWWW......",
    "......WW.WW.....",
    ".....ggg.ggg....",
];

const VADER_HEAD: [&str; 9] = [
    "......vvvv......",
    "....vvVVVVvv....",
    "...vVVVVVVVVv...",
    "...vVVVVVVVVv...",
    "...vVEEVVEEVv...",
    "...vvVVaaVVvv...",
    "..vvvVaggaVvvv..",
    ".vvvv.VaaV.vvvv.",
    "..vvv.VVVV.vvv..",
];
const VADER_BODY: [&str; 7] = [
    "..ccVVVVVVVVcc..",
    ".cccVVaGRVVVccc.",
    ".cVcVVaaRVVVcVc.",
    ".cVcvvVVVVvvcVc.",
    ".cgcvaaggaavcgc.",
    ".cccvvvvvvvvccc.",
    ".cccvvvvvvvvccc.",
];
const VADER_STAND: [&str; 5] = [
    ".cccvvv..vvvccc.",
    ".cccVV....VVccc.",
    ".cccVV....VVccc.",
    ".ccc.VV..VV.ccc.",
    "..c.vvvv.vvvv.c.",
];
const VADER_WALK_A: [&str; 5] = [
    ".cccvvv..vvvccc.",
    ".ccVV......VVcc.",
    ".ccVV......VVcc.",
    ".cVV........VVc.",
    ".vvvv......vvvv.",
];
const VADER_WALK_B: [&str; 5] = [
    ".cccvvv..vvvccc.",
    ".cccVV....VVccc.",
    ".ccc..VVVV..ccc.",
    ".ccc.VV.VV..ccc.",
    "..c.vvvvvvvv..c.",
];

type Head = [&'static str; 9];
type Body = [&'static str; 7];
pub(super) type Legs = [&'static str; 5];

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
        b'o' => c("#8a5a2b"),
        b'O' => c("#b47a3c"),
        b'T' => c("#d8c7a3"),
        b'W' => c("#eef0f2"),
        b'w' => c("#aeb4bd"),
        b'v' => c("#262a33"),
        b'V' => c("#4c5362"),
        b'c' => c("#30333c"),
        b'E' => c("#8a2229"),
        b'a' => c("#8b93a1"),
        b'R' => c("#d8343c"),
        b'G' => c("#5ed17a"),
        _ => return None,
    })
}

/// As travessuras de vez em quando.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    /// Duelo de sabres com o Obi-Wan.
    Duel,
    /// Um ou dois mortais no lugar.
    Flip,
    /// Levanta um stormtrooper com a Força e arremessa.
    Force,
    /// Vira Vader e dá uns passos pesados.
    Vader,
    /// Surpresa.
    Egg(Egg),
    /// Luta contra um rival.
    Fight(Rival),
}

/// Uma sequência fixa em que nada se repete em seguida.
const SCENE_ORDER: [Scene; 12] = [
    Scene::Flip,
    Scene::Fight(Rival::Curly),
    Scene::Duel,
    Scene::Force,
    Scene::Fight(Rival::Red),
    Scene::Vader,
    Scene::Flip,
    Scene::Fight(Rival::Afro),
    Scene::Force,
    Scene::Duel,
    Scene::Fight(Rival::Cap),
    Scene::Vader,
];

impl Scene {
    pub const fn duration(self) -> f32 {
        match self {
            Scene::Duel => 5.6,
            Scene::Flip => 1.3,
            Scene::Force => 4.6,
            Scene::Vader => 6.5,
            Scene::Egg(e) => e.duration(),
            Scene::Fight(r) => r.duration(),
        }
    }

    /// Nome no menu de animações.
    pub fn name(self) -> &'static str {
        match self {
            Scene::Duel => "Duelo com o Obi-Wan",
            Scene::Flip => "Mortal",
            Scene::Force => "Força no stormtrooper",
            Scene::Vader => "Passos de Vader",
            Scene::Egg(e) => e.name(),
            Scene::Fight(r) => r.name(),
        }
    }
}

/// Tudo o que dá pra escolher no menu, com a tecla de cada um enquanto o menu está aberto.
pub fn menu() -> Vec<(char, Scene)> {
    let scenes = rivals::ALL
        .into_iter()
        .map(Scene::Fight)
        .chain([Scene::Duel, Scene::Flip, Scene::Force, Scene::Vader])
        .chain(eggs::ALL.into_iter().map(Scene::Egg));
    "1234qwerasdfghjklzxcvbnmtyuiop"
        .chars()
        .zip(scenes)
        .collect()
}

/// Qual cena vem na vez `turn`: alterna as de sempre com as surpresas. `shuffle` muda a ordem das
/// surpresas a cada vez que o app abre.
pub fn pick_scene(turn: u32, shuffle: u32, hour: u32, may_fourth: bool) -> Scene {
    if may_fourth && turn == 1 {
        return Scene::Egg(Egg::Fireworks);
    }
    if hour < 5 && turn.is_multiple_of(3) {
        return Scene::Egg(Egg::Sleep);
    }
    let half = (turn / 2) as usize;
    if turn.is_multiple_of(2) {
        let i = half.wrapping_add(shuffle as usize) % eggs::ROTATION.len();
        Scene::Egg(eggs::ROTATION[i])
    } else {
        SCENE_ORDER[half % SCENE_ORDER.len()]
    }
}

pub struct AvatarState {
    /// Quanto andou pra esquerda a partir do canto.
    x: f32,
    dir: f32,
    walk_until: Option<Instant>,
    blink_until: Option<Instant>,
    jump_until: Option<Instant>,
    scene: Option<(Scene, Instant)>,
    /// A cena atual foi pedida no menu: roda mesmo com a conversa aberta.
    manual: bool,
    /// Menu de animações aberto.
    pub picker: bool,
    shuffle: u32,
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
            scene: None,
            manual: false,
            picker: false,
            shuffle: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.subsec_nanos() >> 8),
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

    /// Começa uma cena já com `elapsed` de andamento.
    pub fn play_from(&mut self, scene: Scene, elapsed: Duration) {
        let now = Instant::now();
        self.scene = Some((scene, now.checked_sub(elapsed).unwrap_or(now)));
    }

    /// Roda uma cena agora, pedida pelo menu. A próxima travessura espera essa acabar.
    pub fn play(&mut self, scene: Scene) {
        let now = Instant::now();
        self.scene = Some((scene, now));
        self.manual = true;
        self.walk_until = None;
        let after = now + Duration::from_secs_f32(scene.duration()) + Duration::from_secs(6);
        self.next_idle = self.next_idle.max(after);
    }

    pub fn scene(&self) -> Option<Scene> {
        self.scene.map(|(s, _)| s)
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
        if self
            .scene
            .is_some_and(|(sc, at)| now.duration_since(at).as_secs_f32() >= sc.duration())
        {
            self.scene = None;
        }
        if self.scene.is_none() {
            self.manual = false;
        }
        if !animate {
            self.walk_until = None;
            if !self.manual {
                self.scene = None;
            }
            return self.scene.is_some();
        }
        if now >= self.next_idle {
            self.seq = self.seq.wrapping_add(1);
            // intervalo que varia sem precisar de um gerador aleatório
            let gap = 6 + (self.seq.wrapping_mul(2654435761) >> 28) as u64;
            self.next_idle = now + Duration::from_secs(gap);
            match self.seq % 3 {
                1 => self.blink_until = Some(now + Duration::from_millis(180)),
                2 => {
                    self.walk_until =
                        Some(now + Duration::from_millis(1800 + 700 * (self.seq % 3) as u64))
                }
                _ => {
                    use chrono::{Datelike, Timelike};
                    let turn = self.seq / 3;
                    let date = chrono::Local::now();
                    let may_fourth = date.month() == 5 && date.day() == 4;
                    let scene = pick_scene(turn, self.shuffle, date.hour(), may_fourth);
                    self.scene = Some((scene, now));
                    self.walk_until = None;
                    if scene == Scene::Flip && self.say.is_none() {
                        self.speak(FALAS[turn as usize % FALAS.len()], 3);
                    }
                }
            }
        }
        if self.walk_until.is_some() {
            self.advance(WALK_SPEED, dt);
        }
        if let Some((scene, at)) = self.scene {
            let t = now.duration_since(at).as_secs_f32();
            match scene {
                Scene::Vader if (0.8..5.7).contains(&t) => self.advance(VADER_SPEED, dt),
                Scene::Flip if (0.2..1.1).contains(&t) => self.advance(FLIP_SPEED, dt),
                _ => {}
            }
        }
        self.walking()
            || self.blink_until.is_some()
            || self.jump_until.is_some()
            || self.scene.is_some()
    }

    /// Anda no sentido atual e dá meia-volta nas pontas.
    fn advance(&mut self, speed: f32, dt: f32) {
        self.x += self.dir * speed * dt;
        if self.x >= WALK_RANGE {
            self.dir = -1.0;
        } else if self.x <= 0.0 {
            self.x = 0.0;
            self.dir = 1.0;
        }
    }

    /// Daqui a quanto tempo algo muda sozinho, parado.
    fn next_wake(&self, now: Instant) -> Duration {
        let mut at = self.next_idle;
        if let Some((_, until)) = self.say {
            at = at.min(until);
        }
        at.saturating_duration_since(now)
    }

    fn walking(&self) -> bool {
        self.walk_until.is_some()
    }
}

/// Cada linha de um boneco, de cima pra baixo.
pub(super) fn rows(
    head: &'static Head,
    body: &'static Body,
    legs: &'static Legs,
    blink: bool,
) -> impl Iterator<Item = &'static str> {
    head.iter()
        .enumerate()
        .map(move |(i, r)| if blink && i == 5 { HEAD_BLINK_ROW } else { *r })
        .chain(body.iter().copied())
        .chain(legs.iter().copied())
}

/// Onde e como um boneco é desenhado. As coordenadas de boneco são colunas e linhas do desenho,
/// que sempre olha pra direita; `flip` espelha.
#[derive(Clone, Copy)]
pub(super) struct Place {
    pub(super) at: Pos2,
    pub(super) flip: bool,
    /// Giro em volta do centro do boneco, em radianos, no sentido horário.
    pub(super) angle: f32,
    pub(super) alpha: f32,
    /// Olhos amarelos.
    pub(super) sith: bool,
}

impl Place {
    pub(super) fn new(at: Pos2, flip: bool) -> Self {
        Self {
            at,
            flip,
            angle: 0.0,
            alpha: 1.0,
            sith: false,
        }
    }

    /// Da coordenada de boneco pra tela.
    pub(super) fn point(&self, col: f32, row: f32) -> Pos2 {
        let col = if self.flip { COLS as f32 - col } else { col };
        let v = vec2(col * PX - WIDTH / 2.0, row * PX - HEIGHT / 2.0);
        let (sin, cos) = self.angle.sin_cos();
        self.at
            + vec2(WIDTH / 2.0, HEIGHT / 2.0)
            + vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
    }

    /// A mão do sabre.
    pub(super) fn hand(&self) -> Pos2 {
        self.point(13.5, 15.0)
    }

    /// Ângulo na tela de um sabre inclinado `a` pra frente do boneco.
    pub(super) fn blade(&self, a: f32) -> f32 {
        self.angle + if self.flip { -a } else { a }
    }
}

/// Desenha um boneco numa malha só, com um retângulo por trecho de mesma cor.
pub(super) fn paint_sprite(
    p: &egui::Painter,
    rows: impl Iterator<Item = &'static str>,
    place: Place,
) {
    paint_sprite_pal(p, rows, place, color);
}

/// O mesmo, com outras cores.
pub(super) fn paint_sprite_pal(
    p: &egui::Painter,
    rows: impl Iterator<Item = &'static str>,
    place: Place,
    pal: fn(u8) -> Option<Color32>,
) {
    let mut mesh = egui::Mesh::default();
    for (y, row) in rows.enumerate() {
        let b = row.as_bytes();
        let mut x = 0;
        while x < b.len() {
            let ch = b[x];
            let mut end = x + 1;
            while end < b.len() && b[end] == ch {
                end += 1;
            }
            let col = if place.sith && ch == b'e' {
                Some(c("#f2c230"))
            } else {
                pal(ch)
            };
            if let Some(col) = col {
                let col = col.gamma_multiply(place.alpha);
                let (top, bottom) = (y as f32, y as f32 + 1.0);
                let (left, right) = (x as f32, end as f32);
                let i = mesh.vertices.len() as u32;
                mesh.colored_vertex(place.point(left, top), col);
                mesh.colored_vertex(place.point(right, top), col);
                mesh.colored_vertex(place.point(right, bottom), col);
                mesh.colored_vertex(place.point(left, bottom), col);
                mesh.add_triangle(i, i + 1, i + 2);
                mesh.add_triangle(i, i + 2, i + 3);
            }
            x = end;
        }
    }
    p.add(mesh);
}

/// Um sabre aceso saindo de `hand`. `angle` é 0 pra cima, positivo no sentido horário.
pub(super) fn paint_saber(
    p: &egui::Painter,
    hand: Pos2,
    angle: f32,
    len: f32,
    blue: bool,
    alpha: f32,
) {
    let col = if blue {
        (SABER_BLUE, SABER_BLUE_CORE)
    } else {
        (SABER_RED, SABER_RED_CORE)
    };
    paint_saber_col(p, hand, angle, len, col, alpha);
}

/// O mesmo, com a cor (brilho, miolo) escolhida.
pub(super) fn paint_saber_col(
    p: &egui::Painter,
    hand: Pos2,
    angle: f32,
    len: f32,
    (glow, core): (&str, &str),
    alpha: f32,
) {
    let (glow, core) = (c(glow), c(core));
    let dir = vec2(angle.sin(), -angle.cos());
    let tip = hand + dir * len;
    let line = |width: f32, a: Pos2, b: Pos2, col: Color32| {
        p.line_segment([a, b], Stroke::new(width, col.gamma_multiply(alpha)));
    };
    line(7.0, hand, tip, glow.gamma_multiply(0.25));
    line(3.2, hand, tip, glow);
    line(1.4, hand, tip, core);
    line(4.0, hand + dir * 1.0, hand - dir * 7.0, c("#9aa7b8"));
}

/// Interpola suave de 0 a 1.
pub(super) fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

pub(super) fn lerp(a: f32, b: f32, u: f32) -> f32 {
    a + (b - a) * u
}

/// Faíscas do choque dos sabres. `age` em segundos desde o choque.
pub(super) fn paint_sparks(p: &egui::Painter, at: Pos2, age: f32, seed: u32) {
    let alpha = (1.0 - age / 0.12).clamp(0.0, 1.0);
    for i in 0..6u32 {
        let a = i as f32 * 1.05 + seed as f32 * 2.4;
        let r = 3.0 + age * 90.0 * (0.6 + (i % 3) as f32 * 0.25);
        let col = if i % 2 == 0 {
            c("#fff4d6")
        } else {
            c("#ffa23a")
        };
        p.rect_filled(
            Rect::from_center_size(at + vec2(a.cos(), a.sin()) * r, vec2(2.0, 2.0)),
            0.0,
            col.gamma_multiply(alpha),
        );
    }
}

/// Ondas da Força, indo de `from` até `to`.
pub(super) fn paint_force(p: &egui::Painter, from: Pos2, to: Pos2, t: f32) {
    let axis = (to - from).normalized();
    let normal = axis.rot90();
    for i in 0..3 {
        let u = (t * 1.4 + i as f32 / 3.0).fract();
        let center = from + (to - from) * u;
        let size = 4.0 + u * 8.0;
        let points: Vec<Pos2> = (-2..=2)
            .map(|k| {
                let k = k as f32 / 2.0;
                center + normal * k * size - axis * k * k * size * 0.5
            })
            .collect();
        p.add(egui::Shape::line(
            points,
            Stroke::new(2.0, c("#bfd8ff").gamma_multiply(1.0 - u * 0.7)),
        ));
    }
}

/// Desenha a cena no tempo `t`, com o Anakin em `base`. Devolve onde está o outro boneco.
fn paint_scene(
    p: &egui::Painter,
    scene: Scene,
    t: f32,
    base: Pos2,
    facing_left: bool,
    seq: u32,
    screen: Rect,
) -> Option<Pos2> {
    let tau = std::f32::consts::TAU;
    match scene {
        Scene::Egg(e) => e.paint(p, t, base, screen),
        Scene::Fight(r) => r.paint(p, t, base),
        Scene::Flip => {
            let u = (t - 0.2) / 0.9;
            let air = (0.0..=1.0).contains(&u);
            let hop = if air {
                (u * tau / 2.0).sin() * 46.0
            } else {
                0.0
            };
            // agacha antes de saltar e ao cair
            let sink = if air { 0.0 } else { 3.0 };
            let turns = (seq / 3 % 2 + 1) as f32;
            let spin = ease(u) * turns * tau;
            let mut me = Place::new(base + vec2(0.0, sink - hop), facing_left);
            me.angle = if facing_left { spin } else { -spin };
            paint_sprite(p, rows(&HEAD, &BODY, &LEGS_STAND, false), me);
            paint_saber(p, me.hand(), me.blade(0.0), 44.0, false, 1.0);
            None
        }
        Scene::Duel => {
            let gap = if t < 0.6 {
                lerp(170.0, 80.0, ease(t / 0.6))
            } else if t > 5.0 {
                lerp(80.0, 170.0, ease((t - 5.0) / 0.6))
            } else {
                80.0 + (t * 1.7).sin() * 6.0
            };
            let fighting = (0.6..5.0).contains(&t);
            let running = !fighting;
            let beat = (t - 0.6) / 0.4;
            let frac = beat.fract();
            // os dois sabres se encontram no meio, no fim de cada golpe
            let hit = (((gap - 33.0) / 2.0 / 40.0).min(0.95)).asin();
            let (mine, his) = if fighting {
                let s = (frac * tau / 2.0).sin();
                (lerp(hit, 0.1, s), lerp(hit, 0.1, s))
            } else {
                (0.15, 0.15)
            };
            let u = (t - 2.7) / 0.8;
            let flipping = (0.0..=1.0).contains(&u);
            let hop = if flipping {
                (u * tau / 2.0).sin() * 34.0
            } else {
                0.0
            };
            let mut me = Place::new(base - vec2(0.0, hop), true);
            me.angle = if flipping { ease(u) * tau } else { 0.0 };
            me.sith = true;
            let obi_at = base - vec2(gap, 0.0);
            let mut obi = Place::new(obi_at, false);
            obi.alpha = (t / 0.2).min(1.0) * (1.0 - ((t - 5.2) / 0.4).clamp(0.0, 1.0));
            let legs: &'static Legs = if running && ((t * 9.0) as u32).is_multiple_of(2) {
                &LEGS_WALK_A
            } else if running {
                &LEGS_WALK_B
            } else {
                &LEGS_STAND
            };
            paint_sprite(p, rows(&OBI_HEAD, &OBI_BODY, legs, false), obi);
            paint_sprite(p, rows(&HEAD, &BODY, &LEGS_STAND, false), me);
            paint_saber(p, obi.hand(), obi.blade(his), 40.0, true, obi.alpha);
            paint_saber(p, me.hand(), me.blade(mine), 40.0, false, 1.0);
            if fighting && !flipping && frac < 0.3 {
                let h = me.hand();
                let tip = h + vec2(-hit.sin(), -hit.cos()) * 40.0;
                paint_sparks(p, tip, frac * 0.4, beat as u32);
            }
            Some(obi_at)
        }
        Scene::Force => {
            let walking = t < 0.9;
            let gap = lerp(170.0, 100.0, ease(t / 0.9));
            let lifted = (0.9..3.1).contains(&t);
            let thrown = t >= 3.1;
            let raise = ease((t - 0.9) / 0.6) * 26.0;
            let shake = if lifted { (t * 40.0).sin() * 1.5 } else { 0.0 };
            let u = (t - 3.1) / 0.9;
            let fly = if thrown { u * u * 260.0 } else { 0.0 };
            let mut tr_at = base - vec2(gap + fly - shake, 0.0);
            if t >= 0.9 {
                tr_at.y -= raise + if thrown { u * 34.0 } else { 0.0 };
            }
            let mut trooper = Place::new(tr_at, false);
            trooper.alpha = (t / 0.2).min(1.0) * (1.0 - u.clamp(0.0, 1.0));
            trooper.angle = if thrown { -u * tau * 1.5 } else { 0.0 };
            let legs: &'static Legs = if walking || lifted {
                if ((t * if walking { 8.0 } else { 14.0 }) as u32).is_multiple_of(2) {
                    &TROOPER_WALK_A
                } else {
                    &TROOPER_WALK_B
                }
            } else {
                &TROOPER_STAND
            };
            if t < 4.0 {
                paint_sprite(p, rows(&TROOPER_HEAD, &TROOPER_BODY, legs, false), trooper);
            }
            let reaching = (0.8..3.3).contains(&t);
            let mut me = Place::new(base, true);
            me.sith = true;
            if reaching {
                paint_sprite(p, rows(&HEAD, &BODY_FORCE, &LEGS_STAND, false), me);
                // o sabre fica pra trás, na outra mão
                paint_saber(p, me.point(2.5, 15.0), me.blade(-0.35), 40.0, false, 1.0);
                if lifted {
                    let from = me.point(16.0, 11.5);
                    paint_force(p, from, tr_at + vec2(WIDTH / 2.0, HEIGHT * 0.3), t);
                }
            } else {
                paint_sprite(p, rows(&HEAD, &BODY, &LEGS_STAND, false), me);
                paint_saber(p, me.hand(), me.blade(0.15), 44.0, false, 1.0);
            }
            Some(tr_at)
        }
        Scene::Vader => {
            let solid = (0.8..5.7).contains(&t);
            let vader = solid || ((t * 12.0) as u32).is_multiple_of(2);
            // passo pesado: desce o pé, para um instante, troca de perna
            let step = ((t - 0.8) / 0.7).max(0.0);
            let frac = step.fract();
            let odd = step as u32 % 2 == 1;
            let bob = if solid {
                (frac * tau / 2.0).sin() * 3.0
            } else {
                0.0
            };
            let place = Place::new(base - vec2(0.0, bob), facing_left);
            if vader {
                let legs: &'static Legs = match (solid, odd) {
                    (false, _) => &VADER_STAND,
                    (true, false) => &VADER_WALK_A,
                    (true, true) => &VADER_WALK_B,
                };
                paint_sprite(p, rows(&VADER_HEAD, &VADER_BODY, legs, false), place);
            } else {
                paint_sprite(p, rows(&HEAD, &BODY, &LEGS_STAND, false), place);
            }
            paint_saber(p, place.hand(), place.blade(0.3), 44.0, false, 1.0);
            if solid && frac < 0.18 {
                // poeira do pisão
                let alpha = 1.0 - frac / 0.18;
                let ground = base.y + HEIGHT;
                for (dx, up) in [
                    (-6.0, 1.0),
                    (-2.0, 3.0),
                    (WIDTH + 1.0, 2.0),
                    (WIDTH + 5.0, 1.0),
                ] {
                    p.rect_filled(
                        Rect::from_center_size(pos2(base.x + dx, ground - up), vec2(2.5, 2.5)),
                        0.0,
                        c("#8d8370").gamma_multiply(alpha),
                    );
                }
            }
            None
        }
    }
}

/// Quem fala o quê em cada momento da cena: (Anakin, o outro).
fn scene_lines(scene: Scene, t: f32) -> (Option<&'static str>, Option<&'static str>) {
    match scene {
        Scene::Egg(e) => e.lines(t),
        Scene::Fight(r) => r.lines(t),
        Scene::Duel if (0.7..2.6).contains(&t) => (None, Some("I have the high ground!")),
        Scene::Duel if (3.0..4.9).contains(&t) => (Some("You underestimate my power!"), None),
        Scene::Force if (1.1..2.9).contains(&t) => (None, Some("Urk!")),
        Scene::Force if (3.3..4.6).contains(&t) => (Some("Apology accepted."), None),
        Scene::Vader if (1.0..2.8).contains(&t) => (Some("Kohhh... pshhh..."), None),
        Scene::Vader if (3.0..5.6).contains(&t) => {
            (Some("I find your lack of faith disturbing."), None)
        }
        _ => (None, None),
    }
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
    if screen.width() < MIN_SCENE_WIDTH {
        app.anakin.avatar.scene = None;
    }
    let av = &app.anakin.avatar;
    let top_left = pos2(
        screen.right() - 14.0 - WIDTH - av.x,
        screen.bottom() - 8.0 - HEIGHT,
    );
    let hop = av.jump_until.map_or(0.0, |t| {
        let left = t.duration_since(now).as_secs_f32();
        (left / 0.3 * std::f32::consts::PI).sin().abs() * 12.0
    });
    let legs: &'static Legs = if av.walking() {
        let frame = now.duration_since(av.last).as_millis() as f32 + av.x;
        if ((frame / 7.0) as u32).is_multiple_of(2) {
            &LEGS_WALK_A
        } else {
            &LEGS_WALK_B
        }
    } else {
        &LEGS_STAND
    };
    let blink = av.blink_until.is_some();
    let flip = av.dir > 0.0;
    let seq = av.seq;
    let scene = av
        .scene
        .map(|(sc, at)| (sc, now.saturating_duration_since(at).as_secs_f32()));
    let (my_line, other_line) = scene.map_or((None, None), |(sc, t)| scene_lines(sc, t));
    // o que o app tem a dizer vem antes da brincadeira, e cala o outro boneco
    let other_line = other_line.filter(|_| av.speech().is_none());
    let speech = av
        .speech()
        .or(my_line)
        .map(str::to_owned)
        .filter(|_| !app.anakin.open);
    let working = anyone_working(app);
    let wake = av.next_wake(now);
    let idle_wake = app.window_focused && !app.anakin.open;
    let theme = app.theme.clone();

    let mut clicked = false;
    let mut other_at = None;
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
            let p = ui.painter();
            if let Some((sc, t)) = scene {
                other_at = paint_scene(p, sc, t, at, flip, seq, screen);
            } else {
                let place = Place::new(at, flip);
                paint_sprite(p, rows(&HEAD, &BODY, legs, blink), place);
                if working {
                    paint_saber(p, place.hand(), 0.0, 44.0, false, 1.0);
                }
            }
            clicked = resp.clicked();
            resp
        })
        .inner;
    let resp = resp.on_hover_text(format!(
        "{}\n{}",
        app.keymap.tip(
            Action::ToggleAnakin,
            if app.anakin.open {
                "Fechar a conversa com o Anakin"
            } else {
                "Falar com o Anakin"
            },
        ),
        app.keymap
            .tip(Action::AvatarScenes, "Botão direito: menu e animações")
    ));
    if resp.secondary_clicked() {
        app.anakin.avatar.picker = !app.anakin.avatar.picker;
    }
    if app.anakin.avatar.picker {
        picker(app, ctx, top_left - vec2(0.0, hop), resp.rect);
    }
    if clicked {
        crate::anakin::toggle(app, ctx);
    }

    if let Some(text) = speech {
        bubble(ctx, &theme, "anakin-fala", top_left - vec2(0.0, hop), &text);
    }
    if let (Some(text), Some(at)) = (other_line, other_at) {
        bubble(ctx, &theme, "anakin-fala-outro", at, text);
    }
    if scene.is_some() {
        ctx.request_repaint_after(SCENE_FRAME);
    } else if moving {
        ctx.request_repaint_after(ANIM_FRAME);
    } else if idle_wake {
        ctx.request_repaint_after(wake);
    }
}

/// Ações do boneco que aparecem no topo do menu.
const MENU_ACTIONS: [(&str, Action); 3] = [
    ("Falar por voz", Action::AnakinVoice),
    ("Reiniciar a conversa", Action::RestartAnakin),
    ("Esconder o boneco", Action::ToggleAvatar),
];

/// Abre/fecha o menu (ação `AvatarScenes`). Mostra o boneco, se estava escondido.
pub fn toggle_picker(app: &mut App) {
    if !app.cfg.anakin.avatar {
        app.cfg.anakin.avatar = true;
        let _ = app.cfg.save();
        app.anakin.avatar.picker = true;
    } else {
        app.anakin.avatar.picker = !app.anakin.avatar.picker;
    }
}

/// Teclado com o menu aberto: a tecla de uma animação roda ela, Esc fecha, o resto é engolido
/// (não vaza pro terminal). Devolve `true` se o menu tomou o teclado.
pub fn picker_keys(app: &mut App, ctx: &Context) -> bool {
    if !app.anakin.avatar.picker {
        return false;
    }
    if !app.cfg.on(aic_core::Module::Anakin) || !app.cfg.anakin.avatar {
        app.anakin.avatar.picker = false;
        return false;
    }
    let mut close = false;
    let mut typed = String::new();
    ctx.input_mut(|i| {
        i.events.retain(|e| match e {
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                ..
            } => {
                close = true;
                false
            }
            egui::Event::Text(t) => {
                typed.push_str(t);
                false
            }
            egui::Event::Key { .. } | egui::Event::Paste(_) => false,
            _ => true,
        })
    });
    let menu = menu();
    if let Some(scene) = typed
        .chars()
        .find_map(|ch| menu.iter().find(|(k, _)| *k == ch.to_ascii_lowercase()))
        .map(|(_, s)| *s)
    {
        app.anakin.avatar.play(scene);
        close = true;
    }
    if close {
        app.anakin.avatar.picker = false;
    }
    true
}

/// O menu do botão direito: as ações do boneco e todas as animações, cada uma com sua tecla.
fn picker(app: &mut App, ctx: &Context, sprite: Pos2, avatar: Rect) {
    let menu = menu();
    let fights = rivals::ALL.len();
    let classics = fights + 4;
    let eggs_half = classics + (menu.len() - classics).div_ceil(2);
    let mut action = None;
    let mut play = None;
    let item = |ui: &mut egui::Ui, (key, scene): (char, Scene), play: &mut Option<Scene>| {
        let b = egui::Button::new(format!("{key}   {}", scene.name())).frame(false);
        if ui
            .add(b)
            .on_hover_text(format!("Tecla {key} com o menu aberto"))
            .clicked()
        {
            *play = Some(scene);
        }
    };
    let area = egui::Area::new(Id::new("anakin-picker"))
        .order(Order::Foreground)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .fixed_pos(pos2(sprite.x + WIDTH, sprite.y - 6.0))
        .constrain(true)
        .show(ctx, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        // senão os separadores esticam até a borda da tela
                        ui.set_max_width(250.0);
                        for (label, a) in MENU_ACTIONS {
                            let keys = app.keymap.keys_of(a);
                            let b = egui::Button::new(format!("{label}  ({keys})")).frame(false);
                            if ui.add(b).clicked() {
                                action = Some(a);
                            }
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Lutas").strong());
                        for &m in &menu[..fights] {
                            item(ui, m, &mut play);
                        }
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Cenas").strong());
                        for &m in &menu[fights..classics] {
                            item(ui, m, &mut play);
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Surpresas").strong());
                        for &m in &menu[classics..eggs_half] {
                            item(ui, m, &mut play);
                        }
                    });
                    ui.vertical(|ui| {
                        ui.label(" ");
                        for &m in &menu[eggs_half..] {
                            item(ui, m, &mut play);
                        }
                    });
                });
                ui.add_space(6.0);
                ui.weak(app.keymap.tip(
                    Action::AvatarScenes,
                    "Tecla escolhe a animação · Esc fecha · abrir pelo teclado",
                ));
            });
        });
    let outside = ctx.input(|i| {
        i.pointer.any_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !area.response.rect.contains(p) && !avatar.contains(p))
    });
    if let Some(scene) = play {
        app.anakin.avatar.play(scene);
    }
    if play.is_some() || action.is_some() || outside {
        app.anakin.avatar.picker = false;
    }
    if let Some(a) = action {
        app.run_action(ctx, a);
    }
}

fn bubble(ctx: &Context, theme: &crate::theme::Theme, id: &str, sprite: Pos2, text: &str) {
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
    egui::Area::new(Id::new(id))
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
        let bonecos: [(&'static Head, &'static Body, &[&'static Legs]); 5] = [
            (&HEAD, &BODY, &[&LEGS_STAND, &LEGS_WALK_A, &LEGS_WALK_B]),
            (&HEAD, &BODY_FORCE, &[&LEGS_STAND]),
            (&OBI_HEAD, &OBI_BODY, &[&LEGS_STAND]),
            (
                &TROOPER_HEAD,
                &TROOPER_BODY,
                &[&TROOPER_STAND, &TROOPER_WALK_A, &TROOPER_WALK_B],
            ),
            (
                &VADER_HEAD,
                &VADER_BODY,
                &[&VADER_STAND, &VADER_WALK_A, &VADER_WALK_B],
            ),
        ];
        for (head, body, legs) in bonecos {
            for l in legs {
                let v: Vec<_> = rows(head, body, l, false).collect();
                assert_eq!(v.len(), ROWS);
                for r in v {
                    assert_eq!(r.len(), COLS, "{r}");
                    for ch in r.bytes().filter(|ch| *ch != b'.') {
                        assert!(color(ch).is_some(), "sem cor: {}", ch as char);
                    }
                }
            }
        }
        assert_eq!(HEAD_BLINK_ROW.len(), COLS);
    }

    #[test]
    fn de_vez_em_quando_apronta() {
        let mut a = AvatarState::default();
        let mut now = Instant::now();
        let mut scenes = 0;
        for _ in 0..30 {
            now += Duration::from_secs(25);
            a.step(now, true);
            scenes += u32::from(a.scene().is_some());
        }
        assert!(scenes >= 5, "{scenes}");
    }

    #[test]
    fn o_rodizio_passa_por_tudo() {
        let mut seen = Vec::new();
        for turn in 1..200 {
            let s = pick_scene(turn, 12345, 12, false);
            if !seen.contains(&s) {
                seen.push(s);
            }
        }
        assert_eq!(
            seen.len(),
            4 + rivals::ALL.len() + eggs::ROTATION.len(),
            "{seen:?}"
        );
        assert_eq!(
            pick_scene(3, 0, 2, false),
            Scene::Egg(Egg::Sleep),
            "de madrugada ele cochila"
        );
        assert_eq!(
            pick_scene(1, 0, 12, true),
            Scene::Egg(Egg::Fireworks),
            "no dia certo tem festa"
        );
    }

    #[test]
    fn cena_acaba_sozinha_e_sem_foco_para() {
        let mut a = AvatarState::default();
        a.play_from(Scene::Flip, Duration::ZERO);
        let now = Instant::now();
        assert!(a.step(now + Duration::from_millis(100), true), "anima");
        assert!(!a.step(now + Duration::from_secs(3), true), "acabou");
        assert_eq!(a.scene(), None);
        a.play_from(Scene::Duel, Duration::ZERO);
        a.step(Instant::now(), false);
        assert_eq!(a.scene(), None, "sem foco não tem cena");
    }

    #[test]
    fn parado_acorda_so_quando_a_proxima_vence() {
        let a = AvatarState::default();
        let now = Instant::now();
        let wake = a.next_wake(now);
        assert!(wake > Duration::from_secs(5) && wake <= Duration::from_secs(8));
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
