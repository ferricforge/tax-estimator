mod app;
mod screens;
mod widgets;

use app::TaxApp;
use eframe::NativeOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> eframe::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([600.0, 400.0])
            .with_title("Tax Estimator"),
        ..Default::default()
    };

    eframe::run_native(
        "Tax Estimator",
        options,
        Box::new(|cc| Ok(Box::new(TaxApp::new(cc)))),
    )
}
