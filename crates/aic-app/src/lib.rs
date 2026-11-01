//! AIC (marca: Anakin) — central de vibecode pessoal. Biblioteca do app (o binário é `main.rs`).

pub mod actions;
pub mod anakin;
pub mod app;
pub mod brand;
pub mod cli;
pub mod injection;
pub mod notify;
pub mod session;
pub mod sounds;
pub mod theme;
pub mod tmux;
pub mod ui;
pub mod usage_history;

pub use app::App;

/// eframe::App em volta do `App`.
pub struct AicApp(pub App);

impl eframe::App for AicApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui::draw(&mut self.0, ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.0.cfg.save();
        let _ = self.0.keymap.save();
    }

    /// Só o tamanho da janela é guardado: o `Memory` do egui era 67 KB reescritos a cada 30 s.
    fn persist_egui_memory(&self) -> bool {
        false
    }
}

/// Ícone da janela (sem depender de arquivo).
pub fn window_icon() -> egui::IconData {
    egui::IconData {
        rgba: icon_rgba(64),
        width: 64,
        height: 64,
    }
}

/// O sabre na diagonal em RGBA `size`×`size` (usado pela janela e pelo `export-icon`).
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    let k = 64.0 / size as f32;
    // distância até o segmento a→b e posição ao longo dele (0..1)
    let seg = |x: f32, y: f32, a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((x - a.0 - t * dx).hypot(y - a.1 - t * dy), t)
    };
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = ((x as f32 + 0.5) * k, (y as f32 + 0.5) * k);
            let i = ((y * size + x) * 4) as usize;
            // fundo: quadrado de cantos arredondados, preto do espaço
            let (qx, qy) = ((fx - 32.0).abs() - 26.0, (fy - 32.0).abs() - 26.0);
            let corner = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - 4.0;
            if corner > 0.0 {
                continue;
            }
            let mut c = [10.0, 14.0, 20.0];
            let (d, _) = seg(fx, fy, (22.0, 42.0), (50.0, 14.0));
            // brilho → lâmina → miolo quase branco
            c = mix(
                c,
                [47.0, 143.0, 255.0],
                (1.0 - d / 8.0).clamp(0.0, 1.0).powi(2) * 0.8,
            );
            if d < 2.6 {
                c = [124.0, 196.0, 255.0];
            }
            if d < 1.3 {
                c = [234.0, 246.0, 255.0];
            }
            let (h, t) = seg(fx, fy, (12.0, 52.0), (23.0, 41.0));
            if h < 3.2 {
                c = if (0.15..0.25).contains(&t) || t > 0.85 {
                    [57.0, 66.0, 79.0]
                } else {
                    [170.0, 182.0, 198.0]
                };
            }
            rgba[i..i + 4].copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, 255]);
        }
    }
    rgba
}
