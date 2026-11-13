//! The PrintZilla symbol in the macOS menu bar / Windows notification area, with a small menu:
//! open PrintZilla, import files, look for updates, quit.

use eframe::egui;
use std::sync::mpsc::{channel, Receiver};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrayAction {
    Show,
    Import,
    Updates,
    Quit,
}

pub struct MenuIcon {
    #[cfg(any(windows, target_os = "macos"))]
    _tray: tray_icon::TrayIcon,
    rx: Receiver<TrayAction>,
}

/// What the setting is called on this system.
pub fn setting_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "Symbol im Infobereich anzeigen"
    } else {
        "Symbol in der Menüleiste anzeigen"
    }
}

impl MenuIcon {
    /// Puts the symbol into the menu bar. Returns None where this isn't supported.
    pub fn create(ctx: &egui::Context) -> Option<MenuIcon> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            use std::sync::Mutex;
            use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};

            let show = MenuItem::new("PrintZilla öffnen", true, None);
            let import = MenuItem::new("Dateien importieren …", true, None);
            let updates = MenuItem::new("Nach Updates suchen …", true, None);
            let version = MenuItem::new(format!("Version {}", crate::updater::CURRENT_VERSION), false, None);
            let quit = MenuItem::new("PrintZilla beenden", true, None);
            let menu = Menu::new();
            menu.append_items(&[
                &show,
                &import,
                &PredefinedMenuItem::separator(),
                &updates,
                &version,
                &PredefinedMenuItem::separator(),
                &quit,
            ])
            .ok()?;

            let actions = vec![
                (show.id().clone(), TrayAction::Show),
                (import.id().clone(), TrayAction::Import),
                (updates.id().clone(), TrayAction::Updates),
                (quit.id().clone(), TrayAction::Quit),
            ];
            let (tx, rx) = channel();
            let tx = Mutex::new(tx);
            let c = ctx.clone();
            MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
                if let Some((_, a)) = actions.iter().find(|(id, _)| *id == e.id) {
                    if let Ok(tx) = tx.lock() {
                        let _ = tx.send(*a);
                    }
                    c.request_repaint();
                }
            }));

            // macOS: a black "template" symbol the system colours for light and dark menu bars;
            // Windows: the coloured app icon.
            let builder = tray_icon::TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("PrintZilla");
            #[cfg(target_os = "macos")]
            let builder = builder.with_icon_templated(tray_icon::Icon::from_rgba(crate::logo::menu_icon_rgba(36), 36, 36).ok()?);
            #[cfg(windows)]
            let builder = builder.with_icon(tray_icon::Icon::from_rgba(crate::logo::logo_rgba(32), 32, 32).ok()?);
            let tray = builder.build().ok()?;
            Some(MenuIcon { _tray: tray, rx })
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = ctx;
            let (_tx, rx) = channel::<TrayAction>();
            let _ = rx;
            None
        }
    }

    pub fn poll(&self) -> Vec<TrayAction> {
        self.rx.try_iter().collect()
    }
}
