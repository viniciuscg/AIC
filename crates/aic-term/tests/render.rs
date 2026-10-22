//! Renderiza o terminal sem janela (wgpu/lavapipe) e salva um PNG em target/snapshots.
//! Rode com: cargo test -p aic-term --test render -- --nocapture

use std::time::Duration;

use aic_term::{
    BindingsLayout, TermSettings, TerminalBackend, TerminalFont, TerminalTheme, TerminalView,
};
use egui_kittest::Harness;

#[test]
fn renderiza_cores_caixas_e_icones() {
    let script = r#"
printf '\e[31mvermelho \e[1;32mverde-bold \e[0;4;33msublinhado\e[0m \e[38;2;255;100;0mtruecolor\e[0m \e[7minverso\e[0m\n'
echo '╭──────────╮  󰧑 █░░░░░░░░░ 17% de 1M   main ±8'
echo '│ caixa ✻ ⎿│  󱑂 5h █░░░░░░ 15%  ⏵⏵ bypass  🦀 中文'
echo '╰──────────╯'
for i in 1 2 3; do echo "linha $i"; done
printf 'prompt> '
sleep 5
"#;
    let theme = TerminalTheme::default();
    let font = std::sync::Arc::new(std::sync::Mutex::new(TerminalFont::default()));
    let font_ui = font.clone();
    let bindings = BindingsLayout::new();
    let mut term: Option<TerminalBackend> = None;

    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 260.0))
        .build_ui(move |ui| {
            let t = term.get_or_insert_with(|| {
                let settings = TermSettings {
                    program: "bash".into(),
                    args: vec!["-c".into(), script.into()],
                    ..Default::default()
                };
                TerminalBackend::spawn(1, ui.ctx().clone(), settings, theme.clone()).unwrap()
            });
            let size = ui.available_size();
            let f = font_ui.lock().unwrap().clone();
            TerminalView::new(t, &theme, &f, &bindings)
                .focused(true)
                .show(ui, size);
        });

    *font.lock().unwrap() =
        aic_term::fonts::install(&harness.ctx, "JetBrainsMono Nerd Font Mono", None, 14.0);

    for _ in 0..10 {
        harness.step();
        std::thread::sleep(Duration::from_millis(100));
    }
    harness.run_steps(3);

    let image = harness.render().expect("render wgpu");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/snapshots");
    std::fs::create_dir_all(&dir).unwrap();
    image.save(dir.join("terminal.png")).unwrap();
}
