//! A number on the app icon when new files wait in the watched folders:
//! macOS shows the red bubble on the Dock icon, Windows a red circle on the taskbar icon.

use eframe::egui;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // Dock text on macOS
pub fn label(n: usize) -> String {
    if n > 99 {
        "99+".into()
    } else {
        n.to_string()
    }
}

/// Shows `n` on the app icon (0 removes the number).
pub fn set(ctx: &egui::Context, n: usize) {
    #[cfg(target_os = "macos")]
    {
        let _ = ctx;
        use objc2::MainThreadMarker;
        use objc2_app_kit::NSApplication;
        use objc2_foundation::NSString;
        // only possible from the main thread, where the window loop runs
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            let tile = app.dockTile();
            let text = (n > 0).then(|| NSString::from_str(&label(n)));
            #[allow(unused_unsafe)]
            unsafe {
                tile.setBadgeLabel(text.as_deref());
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Windows and others: swap the window/taskbar icon for one with the number drawn in
        let size = 64;
        let rgba = crate::logo::logo_with_badge(size, n);
        ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(std::sync::Arc::new(egui::IconData {
            rgba,
            width: size as u32,
            height: size as u32,
        }))));
    }
}
