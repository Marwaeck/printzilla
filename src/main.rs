// no console window next to the program on Windows
#![cfg_attr(windows, windows_subsystem = "windows")]

mod about_ui;
mod app;
mod badge;
mod card;
mod content;
mod inspector;
mod examples;
mod externals;
mod import_ui;
mod library;
mod logo;
mod model3d;
mod origin;
mod panels;
mod printinfo;
mod render;
#[cfg(test)]
mod tests;
mod tray;
mod update_ui;
mod updater;
mod viewer;
mod watch_ui;
mod watcher;
mod widgets;

use eframe::egui;

/// The icon PrintZilla gives the system while it runs. macOS gets the version with
/// Apple's margins, otherwise the Dock icon looks larger than all others.
fn app_icon() -> egui::IconData {
    let size = if cfg!(target_os = "macos") { 512 } else { 256 };
    let rgba = if cfg!(target_os = "macos") { logo::logo_rgba_macos(size) } else { logo::logo_rgba(size) };
    egui::IconData { rgba, width: size as u32, height: size as u32 }
}

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
        .with_icon(app_icon());

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
