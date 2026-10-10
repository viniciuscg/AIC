use aic_app::cli::{self, Cli};
use aic_app::{App, AicApp};

fn main() {
    match cli::parse(std::env::args().collect()) {
        Cli::Run(cmd) => {
            if let Err(e) = cmd() {
                eprintln!("aic: {e:#}");
                std::process::exit(1);
            }
        }
        Cli::Gui { fork, fresh } => {
            let options = eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_title("AIC")
                    .with_app_id("aic")
                    .with_inner_size([1480.0, 920.0])
                    .with_min_inner_size([900.0, 560.0])
                    .with_icon(aic_app::window_icon()),
                ..Default::default()
            };
            let result = eframe::run_native(
                "aic",
                options,
                Box::new(move |cc| Ok(Box::new(AicApp(App::new(&cc.egui_ctx, fork, fresh)?)))),
            );
            if let Err(e) = result {
                eprintln!("aic: {e}");
                std::process::exit(1);
            }
        }
    }
}
