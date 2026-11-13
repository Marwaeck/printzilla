// no console window next to the program on Windows
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod card;
mod content;
mod inspector;
mod examples;
mod import_ui;
mod library;
mod logo;
mod model3d;
mod panels;
mod render;
#[cfg(test)]
mod tests;
mod update_ui;
mod updater;
mod viewer;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("PrintZilla – 3D-Druck-Bibliothek")
        .with_inner_size([1360.0, 840.0])
        .with_min_inner_size([900.0, 560.0])
        .with_icon(egui::IconData { rgba: logo::logo_rgba(256), width: 256, height: 256 });

    if card::enabled() {
        viewport = viewport.with_min_inner_size([320.0, 160.0]).with_inner_size(card::SIZE).with_resizable(false);
    }

    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "PrintZilla",
        options,
        Box::new(move |cc| Ok(Box::new(app::DamApp::new(cc, screenshot)))),
    )
}
