//! The panel on the right: 3D view, name, description and properties of the selected file.

use crate::app::DamApp;
use crate::library;
use crate::model3d;
use crate::widgets::*;
use eframe::egui::{self, Color32, FontId, RichText, Sense, Vec2};

fn prop_row(ui: &mut egui::Ui, k: &str, v: &str) {
    ui.label(RichText::new(k).size(12.0).color(MUTED));
    ui.add(egui::Label::new(RichText::new(v).family(egui::FontFamily::Monospace).size(11.5).color(TEXT)).wrap());
    ui.end_row();
}

fn wide_button(ui: &mut egui::Ui, text: RichText) -> egui::Response {
    ui.add_sized([ui.available_width(), 28.0], egui::Button::new(text).fill(FIELD))
}

impl DamApp {
    pub fn inspector(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected else {
            self.library_properties(ui);
            return;
        };
        let Some(e) = self.entry(id).cloned() else {
            self.selected = None;
            return;
        };

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if !self.big_view {
                let w = ui.available_width();
                self.viewer_ui(ui, Vec2::new(w, (w * 0.85).min(340.0)));
                ui.add_space(10.0);
            }

            // name + description, like editable fields
            ui.horizontal(|ui| {
                let mut name = e.name.clone();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut name)
                        .font(FontId::proportional(14.0))
                        .margin(egui::Margin::symmetric(8, 7))
                        .desired_width(ui.available_width() - 30.0),
                );
                if r.changed() {
                    if let Some(m) = self.entry_mut(id) {
                        m.name = name;
                    }
                }
                if r.lost_focus() {
                    self.lib.save();
                }
                let (star, col) = if e.favorite { ("★", Color32::from_rgb(250, 200, 70)) } else { ("☆", MUTED) };
                if ui
                    .add(egui::Button::new(RichText::new(star).size(18.0).color(col)).frame(false))
                    .on_hover_text("Favorit")
                    .clicked()
                {
                    if let Some(m) = self.entry_mut(id) {
                        m.favorite = !m.favorite;
                    }
                    self.lib.save();
                }
            });
            ui.add_space(4.0);
            let mut notes = e.notes.clone();
            let r = ui.add(
                egui::TextEdit::multiline(&mut notes)
                    .hint_text("Beschreibung")
                    .margin(egui::Margin::symmetric(8, 7))
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            if r.changed() {
                if let Some(m) = self.entry_mut(id) {
                    m.notes = notes;
                }
            }
            if r.lost_focus() {
                self.lib.save();
            }

            // tags
            ui.add_space(8.0);
            let mut remove_tag = None;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                for t in &e.tags {
                    let b = egui::Button::new(RichText::new(format!("{t}  ✕")).size(12.0)).fill(FIELD).corner_radius(10.0);
                    if ui.add(b).on_hover_text("Schlagwort entfernen").clicked() {
                        remove_tag = Some(t.clone());
                    }
                }
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.tag_input)
                        .hint_text("+ Schlagwort")
                        .desired_width(110.0)
                        .frame(egui::Frame::NONE),
                );
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !self.tag_input.trim().is_empty() {
                    let t = self.tag_input.trim().to_string();
                    if let Some(m) = self.entry_mut(id) {
                        if !m.tags.contains(&t) {
                            m.tags.push(t);
                        }
                    }
                    self.tag_input.clear();
                    self.lib.save();
                }
            });
            if let Some(t) = remove_tag {
                if let Some(m) = self.entry_mut(id) {
                    m.tags.retain(|x| *x != t);
                }
                self.lib.save();
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(RichText::new("Eigenschaften").size(13.0).color(MUTED));
            ui.add_space(2.0);
            egui::Grid::new("props").num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
                prop_row(ui, "Typ", &format!("{} ({})", type_name(&e.ext), e.ext.to_uppercase()));
                prop_row(ui, "Größe", &library::human_size(e.size));
                if let Some(s) = &e.summary {
                    prop_row(ui, "Maße", &format!("{:.1} × {:.1} × {:.1} mm", s.size_mm[0], s.size_mm[1], s.size_mm[2]));
                    prop_row(ui, "Objekte", &s.objects.to_string());
                    prop_row(ui, "Dreiecke", &format_count(s.triangles));
                }
                prop_row(ui, "Importdatum", &library::format_date(e.imported));
                prop_row(ui, "Herkunft", &e.source);
            });

            if let Some(l) = self.viewer.loaded.clone().filter(|_| self.viewer.entry_id == Some(id)) {
                if l.model.objects.len() > 1 {
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new(RichText::new(format!("Objekte ({})", l.model.objects.len())).color(MUTED))
                        .default_open(true)
                        .show(ui, |ui| {
                            for o in l.model.objects.iter().take(40) {
                                ui.horizontal(|ui| {
                                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                                    ui.painter().circle_filled(r.center(), 5.0, Color32::from_rgb(o.color[0], o.color[1], o.color[2]));
                                    ui.label(RichText::new(&o.name).size(12.5));
                                    ui.label(RichText::new(format_count(o.triangles)).size(11.0).color(MUTED));
                                });
                            }
                        });
                }
                if !l.model.metadata.is_empty() {
                    ui.add_space(4.0);
                    egui::CollapsingHeader::new(RichText::new("Angaben aus der 3MF-Datei").color(MUTED)).show(ui, |ui| {
                        egui::Grid::new("meta").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                            for (k, v) in l.model.metadata.iter().take(30) {
                                let v = if v.chars().count() > 160 { format!("{}…", v.chars().take(160).collect::<String>()) } else { v.clone() };
                                prop_row(ui, &meta_label(k), &v);
                            }
                        });
                    });
                }
                if let Some(t) = &self.viewer.embedded_tex {
                    egui::CollapsingHeader::new(RichText::new("Vorschaubild aus der Datei").color(MUTED)).show(ui, |ui| {
                        let s = t.size_vec2();
                        let k = (ui.available_width() / s.x).min(200.0 / s.y);
                        ui.image((t.id(), s * k));
                    });
                }
            }

            ui.add_space(12.0);
            if model3d::can_preview(&e.ext) && !self.big_view && wide_button(ui, RichText::new("⛶  Großansicht")).clicked() {
                self.big_view = true;
            }
            ui.add_space(2.0);
            let programs = self.programs.clone();
            let mut open_in: Option<String> = None;
            let mut add_program = false;
            if let Some(p) = programs.iter().find(|p| p.key == crate::externals::PREFERRED && p.supports(&e.ext)) {
                let b = egui::Button::new(RichText::new(format!("In {} öffnen", p.name)).color(Color32::WHITE).strong()).fill(ACCENT);
                if ui.add_sized([ui.available_width(), 30.0], b).clicked() {
                    open_in = Some(p.key.clone());
                }
                ui.add_space(2.0);
            }
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                ui.menu_button(RichText::new("Öffnen mit  ▾").color(TEXT), |ui| {
                    for p in programs.iter().filter(|p| p.key != crate::externals::PREFERRED && p.supports(&e.ext)) {
                        if ui.button(&p.name).clicked() {
                            open_in = Some(p.key.clone());
                        }
                    }
                    if ui.button("Standardprogramm").clicked() {
                        open_path(&self.lib.file_path(&e), false);
                    }
                    ui.separator();
                    if ui.button("Anderes Programm hinzufügen …").clicked() {
                        add_program = true;
                    }
                });
            });
            let ctx = ui.ctx().clone();
            if let Some(key) = open_in {
                self.open_in(&ctx, &key, id);
            }
            if add_program {
                self.add_program_and_open(&ctx, id);
            }
            ui.add_space(2.0);
            if wide_button(ui, RichText::new("⮉  Exportieren …")).on_hover_text("Eine Kopie an einem anderen Ort speichern").clicked() {
                let src = self.lib.file_path(&e);
                let fname = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if let Some(dest) = rfd::FileDialog::new().set_title("Kopie speichern unter").set_file_name(&fname).save_file() {
                    let ctx = ui.ctx().clone();
                    match std::fs::copy(&src, &dest) {
                        Ok(_) => self.notify(&ctx, "Kopie gespeichert"),
                        Err(err) => self.notify(&ctx, format!("Kopieren fehlgeschlagen: {err}")),
                    }
                }
            }
            ui.add_space(2.0);
            if wide_button(ui, RichText::new(reveal_label())).clicked() {
                open_path(&self.lib.file_path(&e), true);
            }
            ui.add_space(10.0);
            if ui.add(egui::Button::new(RichText::new("Aus der Bibliothek entfernen …").color(DANGER)).frame(false)).clicked() {
                self.confirm_remove = Some(id);
            }
            ui.add_space(10.0);
        });
    }

    /// Shown when nothing is selected: facts about the current view and the library.
    fn library_properties(&mut self, ui: &mut egui::Ui) {
        let entries = self.visible_entries();
        let size: u64 = entries.iter().map(|e| e.size).sum();
        let newest = entries.iter().map(|e| e.imported).max();
        ui.add(
            egui::TextEdit::singleline(&mut self.view_title())
                .font(FontId::proportional(14.0))
                .margin(egui::Margin::symmetric(8, 7))
                .desired_width(f32::INFINITY)
                .interactive(false),
        );
        ui.add_space(14.0);
        ui.separator();
        ui.label(RichText::new("Eigenschaften").size(13.0).color(MUTED));
        egui::Grid::new("libprops").num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
            prop_row(ui, "Dateien", &format_count(entries.len()));
            prop_row(ui, "Größe", &library::human_size(size));
            if let Some(t) = newest {
                prop_row(ui, "Importdatum", &library::format_date(t));
            }
            prop_row(ui, "Speicherort", &self.lib.dir.to_string_lossy());
        });
        ui.add_space(12.0);
        if wide_button(ui, RichText::new(reveal_label())).clicked() {
            open_path(&self.lib.dir, false);
        }
        ui.add_space(2.0);
        if wide_button(ui, RichText::new("Bibliotheksordner wechseln …")).clicked() {
            self.change_library();
        }
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Wähle in der Übersicht eine Datei,\num sie in 3D anzusehen.").color(MUTED));
        });
    }
}
