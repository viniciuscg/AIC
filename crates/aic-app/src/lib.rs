//! AIC — central de vibecode pessoal. Biblioteca do app (o binário é `main.rs`).

pub mod actions;
pub mod app;
pub mod cli;
pub mod connector;
pub mod skins;
pub mod sounds;
pub mod injection;
pub mod notify;
pub mod notion_ui;
pub mod session;
pub mod theme;
pub mod tmux;
pub mod ui;
pub mod usage_history;

pub use app::App;

/// eframe::App em volta do `App`.
pub struct AicApp(pub App);

impl eframe::App for AicApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        crate::notion_ui::tick(&mut self.0);
        ui::draw(&mut self.0, ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.0.cfg.save();
        let _ = self.0.keymap.save();
    }
}

/// Ícone da janela gerado do mascote (sem depender de arquivo).
pub fn window_icon() -> egui::IconData {
    egui::IconData { rgba: icon_rgba(64), width: 64, height: 64 }
}

/// Mascote em RGBA `size`×`size` (desenhado em 64 px e escalado; usado pela janela e pelo `export-icon`).
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let k = 64.0 / size as f32;
    let c = 32.0;
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = ((x as f32 + 0.5) * k - c, (y as f32 + 0.5) * k - c);
            let i = ((y * size + x) * 4) as usize;
            let body = (fx / 18.0).powi(2) + ((fy - 4.0) / 26.0).powi(2) <= 1.0;
            let eye = (fx * fx + (fy + 2.0).powi(2)).sqrt();
            let px: [u8; 4] = if eye <= 4.0 {
                [16, 20, 16, 255]
            } else if eye <= 7.5 {
                [211, 48, 47, 255]
            } else if eye <= 12.0 {
                [244, 227, 107, 255]
            } else if body {
                [63, 143, 58, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba[i..i + 4].copy_from_slice(&px);
        }
    }
    rgba
}
