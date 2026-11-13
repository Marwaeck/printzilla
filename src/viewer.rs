//! The interactive 3D view of the selected file.

use crate::app::DamApp;
use crate::model3d;
use crate::render::{self, Camera};
use crate::widgets::*;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

impl DamApp {
    pub fn viewer_ui(&mut self, ui: &mut egui::Ui, size: Vec2) {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, CornerRadius::same(8), Color32::from_rgb(40, 40, 44));

        let entry = self.selected.and_then(|id| self.entry(id)).cloned();
        let Some(entry) = entry else {
            painter.text(rect.center(), Align2::CENTER_CENTER, "Keine Datei ausgewählt", FontId::proportional(14.0), MUTED);
            return;
        };
        if !model3d::can_preview(&entry.ext) {
            painter.text(rect.center() - Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, type_icon(&entry.ext), FontId::proportional(40.0), MUTED);
            painter.text(
                rect.center() + Vec2::new(0.0, 26.0),
                Align2::CENTER_CENTER,
                format!("Für {}-Dateien gibt es keine 3D-Ansicht", entry.ext.to_uppercase()),
                FontId::proportional(13.0),
                MUTED,
            );
            return;
        }
        if self.viewer.loading.is_some() {
            painter.text(rect.center(), Align2::CENTER_CENTER, "Modell wird geladen …", FontId::proportional(14.0), MUTED);
            ui.ctx().request_repaint();
            return;
        }
        if let Some(err) = &self.viewer.error {
            painter.text(rect.center(), Align2::CENTER_CENTER, err, FontId::proportional(13.0), DANGER);
            return;
        }
        let Some(loaded) = self.viewer.loaded.clone() else { return };
        let Some(prep) = &loaded.prepared else {
            // a 3MF without geometry: show the picture stored inside it
            if let Some(t) = &self.viewer.embedded_tex {
                let s = t.size_vec2();
                let k = (rect.width() / s.x).min(rect.height() / s.y);
                let r = Rect::from_center_size(rect.center(), s * k);
                painter.image(t.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            return;
        };

        // mouse: drag = orbit, right/middle/shift drag = pan, wheel/pinch = zoom
        let shift = ui.input(|i| i.modifiers.shift);
        if resp.dragged_by(egui::PointerButton::Primary) && !shift {
            let d = resp.drag_delta();
            self.viewer.cam.yaw -= d.x * 0.01;
            self.viewer.cam.pitch = (self.viewer.cam.pitch + d.y * 0.01).clamp(-1.5, 1.5);
        }
        if resp.dragged_by(egui::PointerButton::Secondary)
            || resp.dragged_by(egui::PointerButton::Middle)
            || (shift && resp.dragged_by(egui::PointerButton::Primary))
        {
            let d = resp.drag_delta();
            let k = 2.0 * self.viewer.cam.zoom / rect.height();
            self.viewer.cam.pan[0] += d.x * k;
            self.viewer.cam.pan[1] -= d.y * k;
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            let pinch = ui.input(|i| i.zoom_delta());
            if scroll != 0.0 {
                self.viewer.cam.zoom = (self.viewer.cam.zoom * (-scroll * 0.002).exp()).clamp(0.08, 8.0);
            }
            if pinch != 1.0 {
                self.viewer.cam.zoom = (self.viewer.cam.zoom / pinch).clamp(0.08, 8.0);
            }
        }
        if resp.double_clicked() {
            self.viewer.cam = Camera::default();
        }

        let ppp = ui.ctx().pixels_per_point();
        let heavy = prep.mesh.tris.len() > 250_000 && resp.dragged();
        let scale = if heavy { ppp * 0.5 } else { ppp };
        let w = (rect.width() * scale) as usize;
        let h = (rect.height() * scale) as usize;
        let c = self.viewer.cam;
        let key = (
            entry.id,
            [c.yaw.to_bits(), c.pitch.to_bits(), c.zoom.to_bits(), c.pan[0].to_bits(), c.pan[1].to_bits()],
            w,
            h,
            self.viewer.plate,
        );
        if self.viewer.key != Some(key) || self.viewer.tex.is_none() {
            let fr = render::render(prep, &c, w, h, [40, 40, 44], self.viewer.plate);
            let ci = egui::ColorImage::from_rgba_unmultiplied([fr.width, fr.height], &fr.rgba);
            match &mut self.viewer.tex {
                Some(t) => t.set(ci, egui::TextureOptions::LINEAR),
                None => self.viewer.tex = Some(ui.ctx().load_texture("viewer", ci, egui::TextureOptions::LINEAR)),
            }
            self.viewer.key = Some(key);
        }
        if let Some(t) = &self.viewer.tex {
            painter.image(t.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        painter.rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, Color32::from_rgb(64, 64, 70)), StrokeKind::Inside);

        let s = prep.size;
        painter.text(
            rect.left_bottom() + Vec2::new(10.0, -10.0),
            Align2::LEFT_BOTTOM,
            format!("{:.1} × {:.1} × {:.1} mm", s[0], s[1], s[2]),
            FontId::proportional(12.0),
            Color32::from_rgb(214, 214, 220),
        );
        resp.on_hover_text("Ziehen: drehen · Rechts ziehen oder Umschalt+Ziehen: verschieben · Mausrad: zoomen · Doppelklick: Ausgangsansicht");

        let mut bx = rect.left() + 8.0;
        let views: [(&str, &str, f32, f32); 4] = [
            ("3D", "Schräg von oben (auch per Doppelklick)", -0.75, 0.55),
            ("Vorne", "Von vorne", 0.0, 0.0),
            ("Seite", "Von rechts", -std::f32::consts::FRAC_PI_2, 0.0),
            ("Oben", "Von oben", 0.0, 1.5),
        ];
        let btn_fill = Color32::from_rgba_unmultiplied(30, 30, 33, 210);
        for (label, tip, yaw, pitch) in views {
            let wbtn = if label == "3D" { 34.0 } else { 50.0 };
            let r = Rect::from_min_size(Pos2::new(bx, rect.top() + 8.0), Vec2::new(wbtn, 22.0));
            bx += wbtn + 4.0;
            let b = overlay(ui, r, egui::Button::new(RichText::new(label).size(11.5)).fill(btn_fill));
            if b.on_hover_text(tip).clicked() {
                self.viewer.cam = Camera { yaw, pitch, ..Camera::default() };
            }
        }
        let r = Rect::from_min_size(Pos2::new(rect.right() - 8.0 - 84.0, rect.bottom() - 30.0), Vec2::new(84.0, 22.0));
        let label = if self.viewer.plate { "Platte: an" } else { "Platte: aus" };
        if overlay(ui, r, egui::Button::new(RichText::new(label).size(11.5)).fill(btn_fill))
            .on_hover_text("Druckplatte unter dem Modell zeigen")
            .clicked()
        {
            self.viewer.plate = !self.viewer.plate;
        }
    }
}
