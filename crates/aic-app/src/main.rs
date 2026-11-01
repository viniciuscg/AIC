use aic_app::cli::{self, Cli};
use aic_app::{AicApp, App};

fn main() {
    aic_core::paths::load_dotenv();
    match cli::parse(std::env::args().collect()) {
        Cli::Run(cmd) => {
            if let Err(e) = cmd() {
                eprintln!("aic: {e:#}");
                std::process::exit(1);
            }
        }
        Cli::Gui { fresh } => {
            // Duas janelas disputando o mesmo banco e o mesmo tmux bagunçam (ou apagam) as sessões.
            if aic_core::inbox::other_instance() {
                let _ = aic_core::inbox::send(aic_core::inbox::Command::Focus);
                eprintln!("aic: já está aberto; trazendo a janela pra frente");
                return;
            }
            let options = eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_title(aic_app::brand::NAME)
                    .with_app_id("aic")
                    .with_inner_size([1480.0, 920.0])
                    .with_min_inner_size([900.0, 560.0])
                    .with_icon(aic_app::window_icon()),
                ..Default::default()
            };
            let result = eframe::run_native(
                "aic",
                options,
                Box::new(move |cc| Ok(Box::new(AicApp(App::new(&cc.egui_ctx, fresh)?)))),
            );
            if let Err(e) = result {
                eprintln!("aic: {e}");
                std::process::exit(1);
            }
        }
    }
}
