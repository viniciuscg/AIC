//! Pixel art no painter do egui: fonte bitmap 5x7 e sprites em grade de caracteres.
//!
//! Tudo é desenhado com retângulos em escala inteira e posições arredondadas, sem
//! antialiasing nem blur. A fonte e o formato de sprite vêm do `clark.py` do Biel.

use egui::{Color32, Painter, Pos2, Rect, pos2, vec2};

/// Fonte bitmap 5x7 do Clark. Cada glifo: 7 linhas de 5 bits.
const FONT: &[(char, [&str; 7])] = &[
    ('A', ["01110", "10001", "10001", "11111", "10001", "10001", "10001"]),
    ('B', ["11110", "10001", "10001", "11110", "10001", "10001", "11110"]),
    ('C', ["01110", "10001", "10000", "10000", "10000", "10001", "01110"]),
    ('D', ["11110", "10001", "10001", "10001", "10001", "10001", "11110"]),
    ('E', ["11111", "10000", "10000", "11110", "10000", "10000", "11111"]),
    ('F', ["11111", "10000", "10000", "11110", "10000", "10000", "10000"]),
    ('G', ["01110", "10001", "10000", "10111", "10001", "10001", "01111"]),
    ('H', ["10001", "10001", "10001", "11111", "10001", "10001", "10001"]),
    ('I', ["01110", "00100", "00100", "00100", "00100", "00100", "01110"]),
    ('J', ["00111", "00010", "00010", "00010", "00010", "10010", "01100"]),
    ('K', ["10001", "10010", "10100", "11000", "10100", "10010", "10001"]),
    ('L', ["10000", "10000", "10000", "10000", "10000", "10000", "11111"]),
    ('M', ["10001", "11011", "10101", "10101", "10001", "10001", "10001"]),
    ('N', ["10001", "10001", "11001", "10101", "10011", "10001", "10001"]),
    ('O', ["01110", "10001", "10001", "10001", "10001", "10001", "01110"]),
    ('P', ["11110", "10001", "10001", "11110", "10000", "10000", "10000"]),
    ('Q', ["01110", "10001", "10001", "10001", "10101", "10010", "01101"]),
    ('R', ["11110", "10001", "10001", "11110", "10100", "10010", "10001"]),
    ('S', ["01111", "10000", "10000", "01110", "00001", "00001", "11110"]),
    ('T', ["11111", "00100", "00100", "00100", "00100", "00100", "00100"]),
    ('U', ["10001", "10001", "10001", "10001", "10001", "10001", "01110"]),
    ('V', ["10001", "10001", "10001", "10001", "10001", "01010", "00100"]),
    ('W', ["10001", "10001", "10001", "10101", "10101", "10101", "01010"]),
    ('X', ["10001", "10001", "01010", "00100", "01010", "10001", "10001"]),
    ('Y', ["10001", "10001", "01010", "00100", "00100", "00100", "00100"]),
    ('Z', ["11111", "00001", "00010", "00100", "01000", "10000", "11111"]),
    ('0', ["01110", "10001", "10011", "10101", "11001", "10001", "01110"]),
    ('1', ["00100", "01100", "00100", "00100", "00100", "00100", "01110"]),
    ('2', ["01110", "10001", "00001", "00010", "00100", "01000", "11111"]),
    ('3', ["11111", "00010", "00100", "00010", "00001", "10001", "01110"]),
    ('4', ["00010", "00110", "01010", "10010", "11111", "00010", "00010"]),
    ('5', ["11111", "10000", "11110", "00001", "00001", "10001", "01110"]),
    ('6', ["00110", "01000", "10000", "11110", "10001", "10001", "01110"]),
    ('7', ["11111", "00001", "00010", "00100", "01000", "01000", "01000"]),
    ('8', ["01110", "10001", "10001", "01110", "10001", "10001", "01110"]),
    ('9', ["01110", "10001", "10001", "01111", "00001", "00010", "01100"]),
    (' ', ["00000", "00000", "00000", "00000", "00000", "00000", "00000"]),
    ('.', ["00000", "00000", "00000", "00000", "00000", "01100", "01100"]),
    (':', ["00000", "01100", "01100", "00000", "01100", "01100", "00000"]),
    ('-', ["00000", "00000", "00000", "11111", "00000", "00000", "00000"]),
    ('!', ["00100", "00100", "00100", "00100", "00100", "00000", "00100"]),
    ('?', ["01110", "10001", "00001", "00010", "00100", "00000", "00100"]),
    ('/', ["00001", "00001", "00010", "00100", "01000", "10000", "10000"]),
    ('+', ["00000", "00100", "00100", "11111", "00100", "00100", "00000"]),
    ('>', ["01000", "00100", "00010", "00001", "00010", "00100", "01000"]),
    ('<', ["00010", "00100", "01000", "10000", "01000", "00100", "00010"]),
    ('_', ["00000", "00000", "00000", "00000", "00000", "00000", "11111"]),
    ('%', ["11001", "11001", "00010", "00100", "01000", "10011", "10011"]),
    ('\'', ["00100", "00100", "01000", "00000", "00000", "00000", "00000"]),
    ('(', ["00010", "00100", "01000", "01000", "01000", "00100", "00010"]),
    (')', ["01000", "00100", "00010", "00010", "00010", "00100", "01000"]),
    ('[', ["01110", "01000", "01000", "01000", "01000", "01000", "01110"]),
    (']', ["01110", "00010", "00010", "00010", "00010", "00010", "01110"]),
    ('#', ["01010", "01010", "11111", "01010", "11111", "01010", "01010"]),
    ('*', ["00000", "00100", "10101", "01110", "10101", "00100", "00000"]),
    ('=', ["00000", "00000", "11111", "00000", "11111", "00000", "00000"]),
    (',', ["00000", "00000", "00000", "00000", "01100", "00100", "01000"]),
];

fn glyph(ch: char) -> &'static [&'static str; 7] {
    let up = fold(ch);
    FONT.iter()
        .find(|(c, _)| *c == up)
        .or_else(|| FONT.iter().find(|(c, _)| *c == '?'))
        .map(|(_, g)| g)
        .expect("fonte tem '?'")
}

/// Maiúscula sem acento (o `norm` do clark.py).
fn fold(ch: char) -> char {
    let c = match ch {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
        'é' | 'è' | 'ê' | 'É' | 'È' | 'Ê' => 'E',
        'í' | 'ì' | 'î' | 'Í' | 'Ì' | 'Î' => 'I',
        'ó' | 'ò' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ô' | 'Õ' => 'O',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
        'ç' | 'Ç' => 'C',
        c => c,
    };
    c.to_ascii_uppercase()
}

/// Largura em pontos de `s` na escala `sc` (glifo 5 + 1 de espaço).
pub fn text_w(s: &str, sc: f32) -> f32 {
    (s.chars().count() as f32 * 6.0 * sc - sc).max(0.0)
}

/// Desenha `s` na fonte 5x7 com o canto superior esquerdo em `pos`.
pub fn text(p: &Painter, s: &str, pos: Pos2, sc: f32, color: Color32) {
    let o = snap(pos);
    for (i, ch) in s.chars().enumerate() {
        let ox = o.x + i as f32 * 6.0 * sc;
        for (r, row) in glyph(ch).iter().enumerate() {
            // junta pixels vizinhos da linha num retângulo só
            let bytes = row.as_bytes();
            let mut c = 0;
            while c < 5 {
                if bytes[c] == b'1' {
                    let start = c;
                    while c < 5 && bytes[c] == b'1' {
                        c += 1;
                    }
                    let r0 = pos2(ox + start as f32 * sc, o.y + r as f32 * sc);
                    p.rect_filled(Rect::from_min_size(r0, vec2((c - start) as f32 * sc, sc)), 0.0, color);
                } else {
                    c += 1;
                }
            }
        }
    }
}

/// Texto com sombra dura deslocada de 1 pixel da escala (o `draw_text(..., shadow=)`).
pub fn text_shadow(p: &Painter, s: &str, pos: Pos2, sc: f32, color: Color32, shadow: Color32) {
    text(p, s, pos + vec2(sc, sc), sc, shadow);
    text(p, s, pos, sc, color);
}

/// Arredonda pro pixel inteiro (sem meio-pixel borrado).
pub fn snap(p: Pos2) -> Pos2 {
    pos2(p.x.round(), p.y.round())
}

/// Desenha um sprite em grade de caracteres. `pal` traduz cada caractere numa cor
/// (`None` = transparente). `flip` espelha na horizontal. Linhas adjacentes da mesma
/// cor viram um retângulo só.
pub fn sprite(p: &Painter, rows: &[String], origin: Pos2, px: f32, flip: bool, pal: &dyn Fn(char) -> Option<Color32>) {
    let o = snap(origin);
    let cols = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    for (r, row) in rows.iter().enumerate() {
        let mut chars: Vec<char> = row.chars().collect();
        chars.resize(cols, '.');
        if flip {
            chars.reverse();
        }
        let mut c = 0;
        while c < cols {
            let ch = chars[c];
            let Some(color) = pal(ch) else {
                c += 1;
                continue;
            };
            let start = c;
            while c < cols && chars[c] == ch {
                c += 1;
            }
            let r0 = pos2(o.x + start as f32 * px, o.y + r as f32 * px);
            p.rect_filled(Rect::from_min_size(r0, vec2((c - start) as f32 * px, px)), 0.0, color);
        }
    }
}

/// Contorno de 1 pixel em volta das células cheias (o contorno do ícone do Clark).
pub fn outline(p: &Painter, rows: &[String], origin: Pos2, px: f32, flip: bool, filled: &dyn Fn(char) -> bool, color: Color32) {
    let o = snap(origin);
    let cols = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32;
    let grid: Vec<Vec<bool>> = rows
        .iter()
        .map(|r| {
            let mut v: Vec<bool> = r.chars().map(filled).collect();
            v.resize(cols as usize, false);
            if flip {
                v.reverse();
            }
            v
        })
        .collect();
    let at = |c: i32, r: i32| -> bool {
        r >= 0 && c >= 0 && (r as usize) < grid.len() && c < cols && grid[r as usize][c as usize]
    };
    for r in -1..=grid.len() as i32 {
        for c in -1..=cols {
            if at(c, r) {
                continue;
            }
            if at(c + 1, r) || at(c - 1, r) || at(c, r + 1) || at(c, r - 1) {
                let r0 = pos2(o.x + c as f32 * px, o.y + r as f32 * px);
                p.rect_filled(Rect::from_min_size(r0, vec2(px, px)), 0.0, color);
            }
        }
    }
}

/// Mistura linear de cores (o `mix` do clark.py).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Hash barato e determinístico em [0, 1).
pub fn rnd(seed: f32) -> f32 {
    ((seed * 12.9898).sin() * 43758.545).fract().abs()
}
