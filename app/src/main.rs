#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod abr;
mod brush;
mod model;
mod presets;
mod raster;
mod vector;
mod render;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    app::install_panic_hook();
    let mut screenshot: Option<PathBuf> = None;
    let mut open: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--screenshot" {
            screenshot = args.next().map(PathBuf::from);
        } else if !a.starts_with("--") {
            open = Some(PathBuf::from(a));
        }
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("LeonToon — Estudio de animación 2D")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 620.0])
            // Icono de la ventana y de la barra de tareas (kit de UI del prototipo, opción A).
            .with_icon(eframe::egui::IconData { rgba: include_bytes!("../assets/icon_256.rgba").to_vec(), width: 256, height: 256 }),
        ..Default::default()
    };
    eframe::run_native(
        "LeonToon",
        options,
        Box::new(move |cc| Ok(Box::new(app::LeonApp::new(cc, screenshot, open)))),
    )
}
