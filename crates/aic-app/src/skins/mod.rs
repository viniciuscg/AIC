//! Temas: identidade visual do AIC (cores, mascote, nomes dos estados, fundo, ajuda).
//!
//! Para criar um tema: copie `template.rs`, ajuste e registre em `all()`. O que um tema
//! liga/desliga de funcionalidade fica nos módulos (`aic_core::modules`), configuráveis por tema.

use egui::{Color32, Pos2, Rect, Ui};
use aic_core::SessionState;

use crate::theme::Theme;

pub mod aic;
pub mod pixel;
pub mod plankton;
pub mod template;

pub trait Skin: Send + Sync {
    /// Id estável (config, `--tema`).
    fn id(&self) -> &'static str;
    /// Nome do fork ("Clark").
    fn name(&self) -> &'static str;
    /// Criador ("Biel"). Vazio no Plankton.
    fn creator(&self) -> &'static str;
    fn tagline(&self) -> &'static str;
    /// Variantes de tema; a primeira é a padrão.
    fn variants(&self) -> &'static [&'static str];
    fn theme(&self, variant: &str) -> Theme;
    /// Markdown da aba de ajuda do fork.
    fn help(&self) -> &'static str;

    /// "AIC" / "Plankton".
    fn display_name(&self) -> String {
        if self.creator().is_empty() {
            self.name().to_string()
        } else {
            format!("{} - {}", self.name(), self.creator())
        }
    }

    /// Nome do estado na linguagem do fork.
    fn state_label(&self, s: SessionState) -> &'static str {
        s.label()
    }

    /// Desenha o mascote dentro de `rect` (t = segundos desde o início, pra animar).
    fn mascot(&self, ui: &Ui, rect: Rect, state: SessionState, t: f64, theme: &Theme) {
        plankton::draw_plankton(ui, rect, state, t, theme);
    }

    /// Ícone pequeno (seletor de fork, aba de ajuda). Por padrão, o mascote parado.
    fn icon(&self, ui: &Ui, rect: Rect, t: f64, theme: &Theme) {
        self.mascot(ui, rect, SessionState::Idle, t, theme);
    }

    /// Efeito de fundo (bolhas, dragões, granulação). Desenhado atrás de tudo.
    fn background(&self, _ui: &Ui, _rect: Rect, _t: f64, _theme: &Theme) {}

    /// Logo/título na barra superior e na splash.
    fn wordmark(&self, ui: &Ui, pos: Pos2, size: f32, theme: &Theme) -> Rect {
        let galley = ui.painter().layout_no_wrap(
            self.name().to_uppercase(),
            egui::FontId::new(size, egui::FontFamily::Monospace),
            theme.accent_hi,
        );
        let rect = Rect::from_min_size(pos, galley.size());
        ui.painter().galley(pos, galley, Color32::WHITE);
        rect
    }
}

pub fn all() -> Vec<Box<dyn Skin>> {
    vec![
        Box::new(aic::Aic),
        Box::new(plankton::Plankton),
    ]
}

pub fn by_id(id: &str) -> Box<dyn Skin> {
    all().into_iter().find(|f| f.id() == id).unwrap_or_else(|| Box::new(aic::Aic))
}
