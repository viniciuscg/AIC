//! Modelo de fork novo. Copie este arquivo, troque os nomes e registre em `skins::all()`.
//!
//! O que você pode sobrescrever (tudo tem padrão herdado do Plankton):
//! - `theme(variant)`: cores, raio, sombra e paleta do terminal
//! - `modules()`: desligar board, worktrees, tokens, Notion, mascote, som, fundo
//! - `state_label(estado)`: como o fork chama BOOT/WORKING/IDLE/WAITING/DONE/GAME OVER
//! - `mascot(...)`: desenho/animação do mascote por estado
//! - `background(...)`: efeito de fundo (bolhas, dragões, granulação...)
//! - `wordmark(...)`: o letreiro da barra superior
//! - `icon(...)`: ícone pequeno (seletor de fork, aba de ajuda)
//!
//! Pixel art (fonte bitmap 5x7, sprites em grade de caracteres): veja `skins::pixel`.
//! Referências de mascote: `plankton.rs` (vetor), `clark.rs` (pixel art), `aic.rs` (vetor + fundo).
//! - `help()`: a aba de ajuda do fork (Markdown em `forks/<id>/HELP.md`)

use super::{Skin, plankton::Plankton};
use crate::theme::Theme;

#[allow(dead_code)]
pub struct Template;

impl Skin for Template {
    fn id(&self) -> &'static str {
        "template"
    }
    fn name(&self) -> &'static str {
        "Template"
    }
    fn creator(&self) -> &'static str {
        "Você"
    }
    fn tagline(&self) -> &'static str {
        "um fork novo, herdando tudo do Plankton"
    }
    fn variants(&self) -> &'static [&'static str] {
        Plankton.variants()
    }
    fn theme(&self, variant: &str) -> Theme {
        Plankton.theme(variant)
    }
    fn help(&self) -> &'static str {
        "# Template\n\nFork de exemplo."
    }
}
