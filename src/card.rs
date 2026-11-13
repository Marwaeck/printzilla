//! The social preview card for the GitHub page (1280 × 640). Drawn by the program itself
//! when started with `PRINTZILLA_SOCIAL_CARD=1 printzilla --screenshot assets/social-card.png`.
//! Everything important stays at least 80 px away from the edges, as GitHub recommends.

use crate::widgets::*;
use eframe::egui::{self, epaint, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, StrokeKind, TextureHandle, Vec2};

pub const SIZE: [f32; 2] = [1280.0, 640.0];

pub fn enabled() -> bool {
    std::env::var_os("PRINTZILLA_SOCIAL_CARD").is_some()
}

fn gradient(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = epaint::Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bottom);
    m.colored_vertex(r.left_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(m));
}

/// Text drawn twice with a tiny offset so it reads as bold.
fn bold(p: &egui::Painter, pos: Pos2, text: &str, size: f32, col: Color32) -> Rect {
    p.text(pos + Vec2::new(0.8, 0.0), Align2::LEFT_TOP, text, FontId::proportional(size), col);
    p.text(pos, Align2::LEFT_TOP, text, FontId::proportional(size), col)
}

pub fn draw(ui: &mut egui::Ui, shot: Option<&TextureHandle>) {
    let full = ui.max_rect();
    let p = ui.painter();
    gradient(p, full, Color32::from_rgb(40, 44, 58), Color32::from_rgb(22, 23, 30));
    // soft blue glow behind the app picture
    for i in 0..12 {
        let r = 420.0 - i as f32 * 26.0;
        p.circle_filled(Pos2::new(full.left() + 1000.0, full.top() + 330.0), r, Color32::from_rgba_unmultiplied(58, 132, 246, 4));
    }

    // the app itself on the right, running off the edge
    if let Some(t) = shot {
        let w = 720.0;
        let h = w * t.size_vec2().y / t.size_vec2().x;
        let r = Rect::from_min_size(Pos2::new(full.left() + 640.0, full.center().y - h / 2.0 + 6.0), Vec2::new(w, h));
        for k in 1..8 {
            let s = k as f32 * 3.0;
            p.rect_filled(r.expand(s).translate(Vec2::new(0.0, 8.0)), CornerRadius::same(16 + k as u8), Color32::from_black_alpha(14));
        }
        egui::Image::new((t.id(), r.size())).corner_radius(14).paint_at(ui, r);
        ui.painter().rect_stroke(r, CornerRadius::same(14), Stroke::new(1.5, Color32::from_rgb(78, 82, 96)), StrokeKind::Outside);
    }

    let p = ui.painter();
    let x = full.left() + 80.0;
    // icon + name
    draw_logo(p, Rect::from_min_size(Pos2::new(x, full.top() + 92.0), Vec2::splat(112.0)));
    bold(p, Pos2::new(x, full.top() + 222.0), "PrintZilla", 78.0, Color32::WHITE);
    p.text(Pos2::new(x, full.top() + 318.0), Align2::LEFT_TOP, "Deine Bibliothek für", FontId::proportional(32.0), Color32::from_rgb(206, 210, 222));
    p.text(Pos2::new(x, full.top() + 356.0), Align2::LEFT_TOP, "3D-Druck-Dateien", FontId::proportional(32.0), Color32::from_rgb(206, 210, 222));

    // file types as coloured badges
    let mut bx = x;
    for ext in ["3mf", "stl", "obj", "step", "gcode"] {
        let label = if ext == "gcode" { "G-CODE".to_string() } else { ext.to_uppercase() };
        let w = 22.0 + label.len() as f32 * 11.5;
        let r = Rect::from_min_size(Pos2::new(bx, full.top() + 424.0), Vec2::new(w, 34.0));
        p.rect_filled(r, CornerRadius::same(7), type_color(ext));
        p.text(r.center(), Align2::CENTER_CENTER, label, FontId::proportional(18.0), Color32::from_rgb(22, 22, 26));
        bx += w + 10.0;
    }

    // what it does
    let lines = ["3D-Ansicht · Import aus Ordnern · Updates", "Für Windows & macOS"];
    for (i, l) in lines.iter().enumerate() {
        p.text(
            Pos2::new(x, full.top() + 482.0 + i as f32 * 32.0),
            Align2::LEFT_TOP,
            *l,
            FontId::proportional(22.0),
            if i == 0 { Color32::from_rgb(170, 176, 190) } else { Color32::from_rgb(130, 136, 150) },
        );
    }
}
