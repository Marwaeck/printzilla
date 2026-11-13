//! Colours, small drawing helpers and labels shared by the panels.

use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(58, 132, 246);
pub const BG: Color32 = Color32::from_rgb(46, 46, 50);
pub const PANEL: Color32 = Color32::from_rgb(38, 38, 41);
pub const CARD: Color32 = Color32::from_rgb(58, 58, 62);
pub const CARD_HOVER: Color32 = Color32::from_rgb(66, 66, 71);
pub const FIELD: Color32 = Color32::from_rgb(54, 54, 58);
pub const TEXT: Color32 = Color32::from_rgb(226, 226, 230);
pub const MUTED: Color32 = Color32::from_rgb(150, 150, 157);
pub const DANGER: Color32 = Color32::from_rgb(235, 110, 100);

/// Small grey section heading, e.g. "Dateitypen (4)".
pub fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).size(12.0).color(MUTED));
    ui.add_space(1.0);
}

pub enum Icon<'a> {
    Glyph(&'a str),
    Folder,
}

/// Paints a little folder outline.
pub fn paint_folder(p: &egui::Painter, c: Pos2, col: Color32) {
    let s = Stroke::new(1.3, col);
    let body = Rect::from_center_size(c + Vec2::new(0.0, 1.0), Vec2::new(14.0, 10.0));
    p.rect_stroke(body, CornerRadius::same(2), s, egui::StrokeKind::Middle);
    p.line_segment([body.left_top() + Vec2::new(1.0, -2.5), body.left_top() + Vec2::new(5.5, -2.5)], s);
}

/// One row of the sidebar: icon, label and a count on the right. Returns true when clicked.
pub fn nav_item(ui: &mut egui::Ui, selected: bool, icon: Icon, label: &str, count: usize) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 27.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(62, 62, 67));
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(50, 50, 54));
    }
    let icon_col = if selected { ACCENT } else { Color32::from_rgb(185, 185, 192) };
    let ic = rect.left_center() + Vec2::new(17.0, 0.0);
    match icon {
        Icon::Glyph(g) => {
            p.text(ic, Align2::CENTER_CENTER, g, FontId::proportional(14.0), icon_col);
        }
        Icon::Folder => paint_folder(p, ic, icon_col),
    }
    let max_w = rect.width() - 80.0;
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), FontId::proportional(13.5), TEXT);
    job.wrap.max_width = max_w;
    job.wrap.max_rows = 1;
    let g = p.layout_job(job);
    p.galley(Pos2::new(rect.left() + 34.0, rect.center().y - g.size().y / 2.0), g, TEXT);
    p.text(rect.right_center() - Vec2::new(10.0, 0.0), Align2::RIGHT_CENTER, format_count(count), FontId::monospace(11.5), MUTED);
    resp.clicked()
}

pub fn format_count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// A flat toolbar button with an icon (and optional text).
pub fn tool_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(14.0).color(Color32::from_rgb(200, 200, 206))).frame(false))
}

static LOGO: std::sync::OnceLock<egui::TextureHandle> = std::sync::OnceLock::new();

/// Paints the PrintZilla logo into `r`.
pub fn draw_logo(p: &egui::Painter, r: Rect) {
    let tex = LOGO.get_or_init(|| {
        let n = 256;
        let img = egui::ColorImage::from_rgba_unmultiplied([n, n], &crate::logo::logo_rgba(n));
        p.ctx().load_texture("printzilla-logo", img, egui::TextureOptions::LINEAR)
    });
    p.image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
}

pub fn type_icon(ext: &str) -> &'static str {
    match ext {
        "3mf" => "◈",
        "stl" | "obj" => "◆",
        "step" | "stp" | "f3d" => "⬢",
        "gcode" | "bgcode" => "≡",
        "scad" => "{ }",
        "blend" => "◉",
        _ => "□",
    }
}

pub fn type_color(ext: &str) -> Color32 {
    match ext {
        "3mf" => ACCENT,
        "stl" => Color32::from_rgb(110, 175, 235),
        "obj" => Color32::from_rgb(140, 205, 110),
        "step" | "stp" => Color32::from_rgb(190, 150, 235),
        "gcode" | "bgcode" => Color32::from_rgb(230, 205, 90),
        _ => Color32::from_rgb(170, 175, 185),
    }
}

pub fn type_name(ext: &str) -> &'static str {
    match ext {
        "3mf" => "3MF-Projekt",
        "stl" => "STL-Modell",
        "obj" => "OBJ-Modell",
        "step" | "stp" => "CAD-Datei (STEP)",
        "gcode" => "G-Code",
        "bgcode" => "Binärer G-Code",
        "scad" => "OpenSCAD-Skript",
        "f3d" => "Fusion-Projekt",
        "blend" => "Blender-Datei",
        _ => "Datei",
    }
}

pub fn meta_label(k: &str) -> String {
    match k {
        "Title" => "Titel".into(),
        "Designer" => "Gestaltet von".into(),
        "Description" => "Beschreibung".into(),
        "Copyright" => "Copyright".into(),
        "LicenseTerms" | "License" => "Lizenz".into(),
        "CreationDate" => "Erstellt".into(),
        "ModificationDate" => "Geändert".into(),
        "Application" => "Programm".into(),
        "Rating" => "Bewertung".into(),
        _ => k.to_string(),
    }
}

/// Opens a file with its default program, or shows it in the file manager.
pub fn open_path(p: &std::path::Path, reveal: bool) {
    #[cfg(target_os = "macos")]
    {
        let mut c = std::process::Command::new("open");
        if reveal && p.is_file() {
            c.arg("-R");
        }
        let _ = c.arg(p).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        if reveal && p.is_file() {
            let _ = std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn();
        } else {
            let _ = std::process::Command::new("explorer").arg(p).spawn();
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let target = if reveal && p.is_file() { p.parent().unwrap_or(p).to_path_buf() } else { p.to_path_buf() };
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}

pub fn setup_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        let v = &mut style.visuals;
        v.panel_fill = BG;
        v.window_fill = Color32::from_rgb(44, 44, 48);
        v.extreme_bg_color = FIELD;
        v.selection.bg_fill = Color32::from_rgb(40, 92, 175);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;
        v.widgets.inactive.corner_radius = CornerRadius::same(6);
        v.widgets.hovered.corner_radius = CornerRadius::same(6);
        v.widgets.active.corner_radius = CornerRadius::same(6);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(60, 60, 64);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(72, 72, 77);
        v.window_corner_radius = CornerRadius::same(10);
    });
}

/// Places a widget at an exact spot without moving the surrounding layout.
pub fn overlay(ui: &mut egui::Ui, rect: Rect, widget: impl egui::Widget) -> egui::Response {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.spacing_mut().button_padding = Vec2::new(4.0, 2.0);
    child.add_sized(rect.size(), widget)
}

/// "Show in Finder" / "Show in Explorer", depending on the system.
pub fn reveal_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "Im Explorer zeigen"
    } else if cfg!(target_os = "macos") {
        "Im Finder zeigen"
    } else {
        "Im Ordner zeigen"
    }
}
