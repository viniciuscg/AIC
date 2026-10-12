//! Paleta ANSI do terminal (as cores do AIC vêm de `aic_app::brand`).

use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use egui::Color32;

/// 16 cores ANSI + fundo/texto/cursor/seleção, já em `Color32`.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalTheme {
    pub background: Color32,
    pub foreground: Color32,
    pub cursor: Color32,
    pub selection: Color32,
    /// 0..8 normais, 8..16 brilhantes.
    pub ansi: [Color32; 16],
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self::from_hex(
            "#181818",
            "#d8d8d8",
            "#d8d8d8",
            "#3a3a3a",
            [
                "#181818", "#ac4242", "#90a959", "#f4bf75", "#6a9fb5", "#aa759f", "#75b5aa",
                "#d8d8d8", "#6b6b6b", "#c55555", "#aac474", "#feca88", "#82b8c8", "#c28cb8",
                "#93d3c3", "#f8f8f8",
            ],
        )
    }
}

impl TerminalTheme {
    pub fn from_hex(bg: &str, fg: &str, cursor: &str, selection: &str, ansi: [&str; 16]) -> Self {
        Self {
            background: hex(bg),
            foreground: hex(fg),
            cursor: hex(cursor),
            selection: hex(selection),
            ansi: ansi.map(hex),
        }
    }

    /// Resolve uma cor de célula. `dynamic` são as cores trocadas via OSC pelo programa.
    pub fn resolve(&self, c: Color, dynamic: &Colors) -> Color32 {
        match c {
            Color::Spec(rgb) => rgb32(rgb),
            Color::Indexed(i) => {
                if let Some(rgb) = dynamic[i as usize] {
                    return rgb32(rgb);
                }
                self.indexed(i)
            }
            Color::Named(n) => {
                if let Some(rgb) = dynamic[n as usize] {
                    return rgb32(rgb);
                }
                self.named(n)
            }
        }
    }

    pub fn indexed(&self, i: u8) -> Color32 {
        match i {
            0..=15 => self.ansi[i as usize],
            16..=231 => {
                let i = i - 16;
                let step = |v: u8| if v == 0 { 0 } else { v * 40 + 55 };
                Color32::from_rgb(step(i / 36), step((i / 6) % 6), step(i % 6))
            }
            _ => {
                let v = (i - 232) * 10 + 8;
                Color32::from_rgb(v, v, v)
            }
        }
    }

    pub fn named(&self, n: NamedColor) -> Color32 {
        use NamedColor::*;
        match n {
            Foreground | BrightForeground => self.foreground,
            Background => self.background,
            Cursor => self.cursor,
            DimForeground => self.foreground.gamma_multiply(0.66),
            Black => self.ansi[0],
            Red => self.ansi[1],
            Green => self.ansi[2],
            Yellow => self.ansi[3],
            Blue => self.ansi[4],
            Magenta => self.ansi[5],
            Cyan => self.ansi[6],
            White => self.ansi[7],
            BrightBlack => self.ansi[8],
            BrightRed => self.ansi[9],
            BrightGreen => self.ansi[10],
            BrightYellow => self.ansi[11],
            BrightBlue => self.ansi[12],
            BrightMagenta => self.ansi[13],
            BrightCyan => self.ansi[14],
            BrightWhite => self.ansi[15],
            DimBlack => self.ansi[0].gamma_multiply(0.66),
            DimRed => self.ansi[1].gamma_multiply(0.66),
            DimGreen => self.ansi[2].gamma_multiply(0.66),
            DimYellow => self.ansi[3].gamma_multiply(0.66),
            DimBlue => self.ansi[4].gamma_multiply(0.66),
            DimMagenta => self.ansi[5].gamma_multiply(0.66),
            DimCyan => self.ansi[6].gamma_multiply(0.66),
            DimWhite => self.ansi[7].gamma_multiply(0.66),
        }
    }

    /// Cor no formato que o terminal devolve em resposta a OSC 4/10/11.
    pub(crate) fn rgb_of(&self, index: usize) -> Rgb {
        let c = match index {
            0..=255 => self.indexed(index as u8),
            i if i == NamedColor::Foreground as usize => self.foreground,
            i if i == NamedColor::Background as usize => self.background,
            i if i == NamedColor::Cursor as usize => self.cursor,
            _ => self.foreground,
        };
        Rgb { r: c.r(), g: c.g(), b: c.b() }
    }
}

fn rgb32(rgb: Rgb) -> Color32 {
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

/// `#rrggbb` → `Color32`. Hex inválido vira magenta, para o erro aparecer na tela.
pub fn hex(s: &str) -> Color32 {
    let s = s.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(s.get(i..i + 2).unwrap_or("ff"), 16).unwrap_or(255);
    if s.len() != 6 {
        return Color32::from_rgb(255, 0, 255);
    }
    Color32::from_rgb(p(0), p(2), p(4))
}
