use anyhow::Result;
use eframe::egui;
use seed_finder::{install_diagnostics, log_diag, ui::SeedFinderApp};

fn main() -> Result<()> {
    install_diagnostics();
    log_diag("app_start");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 920.0])
            .with_min_inner_size([1200.0, 760.0])
            .with_title("Minecraft Seed Finder"),
        ..Default::default()
    };

    eframe::run_native(
        "Minecraft Seed Finder",
        options,
        Box::new(|cc| Ok(Box::new(SeedFinderApp::new(cc)))),
    )
    .map_err(|err| anyhow::anyhow!("failed to start UI: {err}"))?;

    log_diag("app_exit");
    Ok(())
}
