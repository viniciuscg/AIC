//! AIC — central de vibecode pessoal. Biblioteca do app (o binário é `main.rs`).

pub mod actions;
pub mod app;
pub mod brand;
pub mod cli;
pub mod connector;
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

/// Ícone da janela: o pote de moscas (sem depender de arquivo).
pub fn window_icon() -> egui::IconData {
    egui::IconData { rgba: icon_rgba(64), width: 64, height: 64 }
}

/// O pote em RGBA `size`×`size` (desenhado em 64 px e escalado; usado pela janela e pelo `export-icon`):
/// tampa de ferrugem, vidro âmbar e três moscas.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let k = 64.0 / size as f32;
    // retângulo de cantos arredondados: distância com sinal (<= 0 = dentro)
    let rrect = |x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (hx, hy) = ((x1 - x0) / 2.0 - r, (y1 - y0) / 2.0 - r);
        let (dx, dy) = (((x - cx).abs() - hx).max(0.0), ((y - cy).abs() - hy).max(0.0));
        (dx * dx + dy * dy).sqrt() - r
    };
    let flies = [(24.0f32, 33.0f32), (38.0, 40.0), (30.0, 49.0)];
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = ((x as f32 + 0.5) * k, (y as f32 + 0.5) * k);
            let i = ((y * size + x) * 4) as usize;
            let lid = rrect(fx, fy, 14.0, 6.0, 50.0, 15.0, 2.0) <= 0.0;
            let neck = rrect(fx, fy, 18.0, 13.0, 46.0, 20.0, 2.0);
            let body = rrect(fx, fy, 11.0, 18.0, 53.0, 60.0, 8.0);
            let glass = neck.min(body);
            let fly = flies.iter().any(|(cx, cy)| ((fx - cx) / 3.2).powi(2) + ((fy - cy) / 2.2).powi(2) <= 1.0);
            let shine = (15.0..18.0).contains(&fx) && (24.0..52.0).contains(&fy);
            let px: [u8; 4] = if lid {
                if (fx as i32) % 6 == 0 { [90, 42, 18, 255] } else { [181, 82, 43, 255] }
            } else if glass <= -1.6 {
                if fly {
                    [21, 16, 11, 255]
                } else if shine {
                    [246, 236, 214, 255]
                } else {
                    [201, 160, 96, 255]
                }
            } else if glass <= 0.0 {
                [239, 226, 200, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba[i..i + 4].copy_from_slice(&px);
        }
    }
    rgba
}
