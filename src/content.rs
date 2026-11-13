//! The folder strip, the thumbnail grid and the big 3D view.

use crate::app::{DamApp, Filter};
use crate::library::{self, Entry};
use crate::model3d;
use crate::widgets::*;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, Vec2};

enum CardAction {
    Select,
    Big,
    Open,
    Reveal,
    Favorite,
    Remove,
}

impl DamApp {
    pub fn grid(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.lib.index.entries.is_empty() {
            if empty_state(ui) {
                self.open_import(None);
            }
            return;
        }
        let entries = self.visible_entries();
        let gap = 10.0;
        let mut budget = 8usize;
        let mut action: Option<(u64, CardAction)> = None;
        let mut open_group: Option<String> = None;

        // groups ("folders") only on the overview
        let groups: Vec<(String, usize, Option<Entry>)> = if self.filter == Filter::All && self.search.is_empty() {
            self.lib
                .all_tags()
                .into_iter()
                .map(|(t, n)| {
                    let first = self.lib.index.entries.iter().find(|e| e.tags.contains(&t) && e.thumb.is_some()).cloned();
                    (t, n, first)
                })
                .collect()
        } else {
            Vec::new()
        };

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.add_space(4.0);
            let avail = ui.available_width() - 4.0;
            let cols = ((avail + gap) / (self.thumb_size + gap)).floor().max(1.0) as usize;
            let cell = (avail - gap * (cols as f32 - 1.0)) / cols as f32;

            if !groups.is_empty() {
                let arrow = if self.show_groups { "▾" } else { "▸" };
                if ui
                    .add(egui::Button::new(RichText::new(format!("Ordner ({})  {arrow}", groups.len())).size(13.0).color(MUTED)).frame(false))
                    .clicked()
                {
                    self.show_groups = !self.show_groups;
                }
                if self.show_groups {
                    ui.add_space(4.0);
                    for row in groups.chunks(cols) {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for (t, n, first) in row {
                                let tex = first.as_ref().and_then(|e| self.thumb(&ctx, e, &mut budget));
                                if group_card(ui, t, *n, tex.as_ref(), cell) {
                                    open_group = Some(t.clone());
                                }
                            }
                        });
                        ui.add_space(gap);
                    }
                    ui.add_space(6.0);
                }
            }

            ui.label(RichText::new(format!("Inhalt ({})", entries.len())).size(13.0).color(MUTED));
            ui.add_space(6.0);
            if entries.is_empty() {
                ui.add_space(30.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Nichts gefunden").size(16.0).color(MUTED));
                    ui.label(RichText::new("Andere Suchwörter oder Filter probieren.").color(MUTED));
                });
                return;
            }
            for row in entries.chunks(cols) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for e in row {
                        let tex = self.thumb(&ctx, e, &mut budget);
                        if let Some(a) = tile(ui, e, tex.as_ref(), self.selected == Some(e.id), cell) {
                            action = Some((e.id, a));
                        }
                    }
                });
                ui.add_space(gap);
            }
            ui.add_space(10.0);
        });

        if let Some(t) = open_group {
            self.set_filter(Filter::Tag(t));
        }
        if let Some((id, a)) = action {
            match a {
                CardAction::Select => self.selected = Some(id),
                CardAction::Big => {
                    self.selected = Some(id);
                    if self.entry(id).map(|e| model3d::can_preview(&e.ext)).unwrap_or(false) {
                        self.big_view = true;
                    }
                }
                CardAction::Open => {
                    if let Some(e) = self.entry(id) {
                        open_path(&self.lib.file_path(e), false);
                    }
                }
                CardAction::Reveal => {
                    if let Some(e) = self.entry(id) {
                        open_path(&self.lib.file_path(e), true);
                    }
                }
                CardAction::Favorite => {
                    if let Some(e) = self.entry_mut(id) {
                        e.favorite = !e.favorite;
                    }
                    self.lib.save();
                }
                CardAction::Remove => self.confirm_remove = Some(id),
            }
        }
    }

    pub fn big_view_ui(&mut self, ui: &mut egui::Ui) {
        let name = self.selected.and_then(|id| self.entry(id)).map(|e| e.name.clone()).unwrap_or_default();
        ui.horizontal(|ui| {
            if ui.button("←  Zurück zur Übersicht").clicked() {
                self.big_view = false;
            }
            ui.label(RichText::new(name).size(15.0).strong().color(TEXT));
        });
        ui.add_space(8.0);
        let mut size = ui.available_size();
        size.y -= 14.0;
        self.viewer_ui(ui, size);
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.big_view = false;
        }
    }
}

fn centered_text(p: &egui::Painter, top_center: Pos2, text: &str, size: f32, col: Color32, width: f32, rows: usize) -> f32 {
    let mut job = egui::text::LayoutJob::simple(text.to_string(), FontId::proportional(size), col, width);
    job.wrap.max_rows = rows;
    job.wrap.break_anywhere = true;
    job.halign = egui::Align::Center;
    let g = p.layout_job(job);
    let h = g.size().y;
    p.galley(top_center, g, col);
    h
}

/// A file in the grid: picture without frame, name and size centred below.
fn tile(ui: &mut egui::Ui, e: &Entry, tex: Option<&TextureHandle>, selected: bool, cell: f32) -> Option<CardAction> {
    let size = Vec2::new(cell, cell + 50.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let img = Rect::from_min_size(rect.min, Vec2::splat(cell));
    let p = ui.painter();
    match tex {
        Some(t) => {
            p.rect_filled(img, CornerRadius::same(4), Color32::from_rgb(44, 44, 48));
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            egui::Image::new((t.id(), img.size())).uv(uv).corner_radius(4).paint_at(ui, img);
        }
        None => {
            p.rect_filled(img, CornerRadius::same(4), CARD);
            // a plain document symbol
            let d = Rect::from_center_size(img.center(), Vec2::new(cell * 0.16, cell * 0.21));
            p.rect_filled(d, CornerRadius::same(2), Color32::from_rgb(235, 235, 238));
            p.text(d.center() + Vec2::new(0.0, d.height() * 0.9), Align2::CENTER_TOP, type_name(&e.ext), FontId::proportional(11.0), MUTED);
        }
    }
    let p = ui.painter();
    if resp.hovered() && !selected {
        p.rect_filled(img, CornerRadius::same(4), Color32::from_white_alpha(10));
    }
    // type badge
    let badge = e.ext.to_uppercase();
    let br = Rect::from_min_size(img.min + Vec2::splat(6.0), Vec2::new(10.0 + badge.len() as f32 * 7.2, 17.0));
    p.rect_filled(br, CornerRadius::same(3), Color32::from_black_alpha(150));
    p.text(br.center(), Align2::CENTER_CENTER, &badge, FontId::proportional(10.5), Color32::WHITE);
    if e.favorite {
        p.text(img.right_top() + Vec2::new(-7.0, 4.0), Align2::RIGHT_TOP, "★", FontId::proportional(15.0), Color32::from_rgb(250, 200, 70));
    }
    if selected {
        p.rect_stroke(img.expand(1.0), CornerRadius::same(5), Stroke::new(2.5, ACCENT), StrokeKind::Outside);
    }
    let name_col = if selected { Color32::WHITE } else { TEXT };
    let h = centered_text(p, Pos2::new(rect.center().x, img.bottom() + 7.0), &e.name, 13.0, name_col, cell - 4.0, 2);
    let info = match &e.summary {
        Some(s) => format!("{} · {:.0}×{:.0}×{:.0} mm", library::human_size(e.size), s.size_mm[0], s.size_mm[1], s.size_mm[2]),
        None => library::human_size(e.size),
    };
    centered_text(p, Pos2::new(rect.center().x, img.bottom() + 9.0 + h), &info, 11.0, MUTED, cell - 4.0, 1);

    let mut action = None;
    if resp.clicked() {
        action = Some(CardAction::Select);
    }
    if resp.double_clicked() {
        action = Some(CardAction::Big);
    }
    resp.context_menu(|ui| {
        if model3d::can_preview(&e.ext) && ui.button("In 3D ansehen").clicked() {
            action = Some(CardAction::Big);
        }
        if ui.button("Mit Standardprogramm öffnen").clicked() {
            action = Some(CardAction::Open);
        }
        if ui.button(reveal_label()).clicked() {
            action = Some(CardAction::Reveal);
        }
        if ui.button(if e.favorite { "Kein Favorit mehr" } else { "Als Favorit markieren" }).clicked() {
            action = Some(CardAction::Favorite);
        }
        ui.separator();
        if ui.button("Entfernen …").clicked() {
            action = Some(CardAction::Remove);
        }
    });
    action
}

/// A group card: a stacked preview, the folder name and how many files it holds.
fn group_card(ui: &mut egui::Ui, name: &str, n: usize, tex: Option<&TextureHandle>, cell: f32) -> bool {
    let ph = (cell * 0.72).round();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(cell, ph + 46.0), Sense::click());
    let p = ui.painter();
    let frame = Rect::from_min_size(rect.min + Vec2::new(0.0, 6.0), Vec2::new(cell, ph - 6.0));
    // stacked sheets behind
    p.rect_filled(frame.shrink2(Vec2::new(14.0, 0.0)).translate(Vec2::new(0.0, -6.0)), CornerRadius::same(5), Color32::from_rgb(70, 70, 75));
    p.rect_filled(frame.shrink2(Vec2::new(7.0, 0.0)).translate(Vec2::new(0.0, -3.0)), CornerRadius::same(5), Color32::from_rgb(82, 82, 88));
    let fill = if resp.hovered() { CARD_HOVER } else { CARD };
    p.rect_filled(frame, CornerRadius::same(5), fill);
    match tex {
        Some(t) => {
            let s = frame.height() - 12.0;
            let r = Rect::from_center_size(frame.center(), Vec2::splat(s));
            egui::Image::new((t.id(), r.size())).corner_radius(3).paint_at(ui, r);
        }
        None => paint_folder(ui.painter(), frame.center(), MUTED),
    }
    let p = ui.painter();
    let h = centered_text(p, Pos2::new(rect.center().x, frame.bottom() + 7.0), name, 13.0, TEXT, cell - 4.0, 1);
    centered_text(p, Pos2::new(rect.center().x, frame.bottom() + 9.0 + h), &format!("{n} Dateien"), 11.0, MUTED, cell, 1);
    resp.on_hover_text("Ordner öffnen").clicked()
}

fn empty_state(ui: &mut egui::Ui) -> bool {
    let mut clicked = false;
    ui.add_space(70.0);
    ui.vertical_centered(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(110.0, 110.0), Sense::hover());
        draw_logo(ui.painter(), r);
        ui.add_space(10.0);
        ui.label(RichText::new("Deine Bibliothek ist noch leer").size(20.0).strong().color(TEXT));
        ui.add_space(4.0);
        ui.label(
            RichText::new("Wähle einen Ordner (z. B. Downloads). PrintZilla sammelt alle 3MF-, STL-, OBJ-, STEP- und G-Code-Dateien daraus ein.")
                .color(MUTED),
        );
        ui.label(RichText::new("Du kannst einen Ordner auch einfach auf dieses Fenster ziehen.").color(MUTED));
        ui.add_space(14.0);
        let b = egui::Button::new(RichText::new("⬇  Ersten Ordner importieren").size(15.0).color(Color32::WHITE).strong())
            .fill(ACCENT)
            .min_size(Vec2::new(240.0, 38.0));
        clicked = ui.add(b).clicked();
    });
    clicked
}
