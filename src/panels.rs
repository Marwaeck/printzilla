//! Sidebar, toolbar and the row of filter chips.

use crate::app::{DamApp, Filter, Sort};
use crate::library::Library;
use crate::widgets::*;
use eframe::egui::{self, Color32, RichText, Sense, Vec2};

pub fn sort_label(s: Sort) -> &'static str {
    match s {
        Sort::Newest => "Neueste zuerst",
        Sort::Name => "Name A–Z",
        Sort::Size => "Größte zuerst",
        Sort::Type => "Nach Dateityp",
    }
}

impl DamApp {
    pub fn change_library(&mut self) {
        if let Some(dir) = rfd::FileDialog::new().set_title("Bibliotheksordner wählen").pick_folder() {
            self.cfg.library_dir = dir.clone();
            self.save_cfg();
            self.lib = Library::open(&dir);
            self.thumbs.clear();
            self.selected = None;
            self.set_filter(Filter::All);
        }
    }

    pub fn side_bar(&mut self, ui: &mut egui::Ui) {
        let now = chrono::Local::now().timestamp();
        let (total, untagged, favs, recent) = {
            let es = &self.lib.index.entries;
            (
                es.len(),
                es.iter().filter(|e| e.tags.is_empty()).count(),
                es.iter().filter(|e| e.favorite).count(),
                es.iter().filter(|e| now - e.imported < 7 * 24 * 3600).count(),
            )
        };
        let mut types: Vec<(String, usize)> = Vec::new();
        for e in &self.lib.index.entries {
            match types.iter_mut().find(|(t, _)| *t == e.ext) {
                Some(x) => x.1 += 1,
                None => types.push((e.ext.clone(), 1)),
            }
        }
        types.sort();
        let tags = self.lib.all_tags();
        let q = self.side_filter.trim().to_lowercase();
        let keep = |s: &str| q.is_empty() || s.to_lowercase().contains(&q);

        // header: library switcher + quick actions
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(30.0, 30.0), Sense::hover());
            draw_logo(ui.painter(), r);
            ui.menu_button(RichText::new("PrintZilla  ▾").strong().size(15.0).color(TEXT), |ui| {
                ui.label(RichText::new(self.lib.dir.to_string_lossy()).size(11.0).color(MUTED));
                ui.separator();
                if ui.button(reveal_label()).clicked() {
                    open_path(&self.lib.dir, false);
                    ui.close();
                }
                if ui.button("Anderen Bibliotheksordner wählen …").clicked() {
                    ui.close();
                    self.change_library();
                }
                ui.separator();
                if ui.button("Nach Updates suchen …").clicked() {
                    ui.close();
                    let ctx = ui.ctx().clone();
                    self.open_updates(&ctx);
                }
                ui.label(RichText::new(format!("Version {}", crate::updater::CURRENT_VERSION)).size(11.0).color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if tool_button(ui, "+").on_hover_text("Dateien importieren").clicked() {
                    self.open_import(None);
                }
            });
        });
        ui.add_space(8.0);
        self.update_hint(ui);

        egui::Panel::bottom("side_filter")
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 0, top: 8, bottom: 2 }))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.side_filter)
                        .hint_text("▽  Filter")
                        .desired_width(f32::INFINITY),
                );
            });

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut go: Option<Filter> = None;
            let cur = self.filter.clone();
            let mut item = |ui: &mut egui::Ui, f: Filter, icon: Icon, label: &str, n: usize| {
                if nav_item(ui, cur == f, icon, label, n) {
                    go = Some(f);
                }
            };
            item(ui, Filter::All, Icon::Glyph("▣"), "Alle", total);
            item(ui, Filter::Untagged, Icon::Glyph("◇"), "Ohne Schlagwort", untagged);
            item(ui, Filter::Favorites, Icon::Glyph("☆"), "Favoriten", favs);
            item(ui, Filter::Recent, Icon::Glyph("◷"), "Zuletzt importiert", recent);

            ui.add_space(14.0);
            section(ui, &format!("Dateitypen ({})", types.len()));
            for (t, n) in types.iter().filter(|(t, _)| keep(type_name(t)) || keep(t)) {
                item(ui, Filter::Type(t.clone()), Icon::Glyph(type_icon(t)), type_name(t), *n);
            }

            ui.add_space(14.0);
            section(ui, &format!("Ordner ({})", tags.len()));
            if tags.is_empty() {
                ui.label(RichText::new("Entstehen beim Import aus Ordnernamen").size(11.5).color(MUTED));
            }
            for (t, n) in tags.iter().filter(|(t, _)| keep(t)) {
                item(ui, Filter::Tag(t.clone()), Icon::Folder, t, *n);
            }
            if let Some(f) = go {
                self.set_filter(f);
            }
        });
    }

    pub fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let back = self.hist_pos > 0;
            let fwd = self.hist_pos + 1 < self.history.len();
            if ui.add_enabled(back, egui::Button::new(RichText::new("‹").size(20.0)).frame(false)).on_hover_text("Zurück").clicked() {
                self.go_history(-1);
            }
            if ui.add_enabled(fwd, egui::Button::new(RichText::new("›").size(20.0)).frame(false)).on_hover_text("Vor").clicked() {
                self.go_history(1);
            }
            ui.add_space(6.0);
            ui.label(RichText::new(self.view_title()).size(15.0).color(TEXT));
            ui.add_space(20.0);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let import_btn = egui::Button::new(RichText::new("⬇  Importieren").color(Color32::WHITE))
                    .fill(ACCENT)
                    .min_size(Vec2::new(0.0, 28.0));
                if ui.add(import_btn).on_hover_text("Ordner durchsuchen und 3D-Druck-Dateien übernehmen").clicked() {
                    self.open_import(None);
                }
                ui.add_space(6.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("🔍  Suchen")
                        .desired_width(210.0)
                        .min_size(Vec2::new(0.0, 28.0)),
                );
                if !self.search.is_empty() && tool_button(ui, "×").on_hover_text("Suche leeren").clicked() {
                    self.search.clear();
                }
                ui.add_space(18.0);
                if tool_button(ui, "+").on_hover_text("Größere Vorschaubilder").clicked() {
                    self.thumb_size = (self.thumb_size + 30.0).min(320.0);
                }
                ui.spacing_mut().slider_width = 110.0;
                ui.add(egui::Slider::new(&mut self.thumb_size, 90.0..=320.0).show_value(false))
                    .on_hover_text("Größe der Vorschaubilder");
                if tool_button(ui, "−").on_hover_text("Kleinere Vorschaubilder").clicked() {
                    self.thumb_size = (self.thumb_size - 30.0).max(90.0);
                }
            });
        });
        ui.add_space(4.0);
    }

    pub fn chip_row(&mut self, ui: &mut egui::Ui) {
        let mut types: Vec<String> = self.lib.index.entries.iter().map(|e| e.ext.clone()).collect();
        types.sort();
        types.dedup();
        let tags: Vec<String> = self.lib.all_tags().into_iter().map(|(t, _)| t).collect();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 14.0;
            let chip = |active: bool, text: &str| {
                RichText::new(text).size(13.5).color(if active { ACCENT } else { Color32::from_rgb(205, 205, 211) })
            };
            ui.menu_button(chip(!self.chip_types.is_empty(), &chip_title("◈ Typen", self.chip_types.len())), |ui| {
                for t in &types {
                    let mut on = self.chip_types.contains(t);
                    if ui.checkbox(&mut on, format!("{}  ({})", type_name(t), t.to_uppercase())).changed() {
                        if on {
                            self.chip_types.push(t.clone());
                        } else {
                            self.chip_types.retain(|x| x != t);
                        }
                    }
                }
                if types.is_empty() {
                    ui.label("Noch keine Dateien");
                }
            });
            ui.menu_button(chip(!self.chip_tags.is_empty(), &chip_title("# Schlagworte", self.chip_tags.len())), |ui| {
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    for t in &tags {
                        let mut on = self.chip_tags.contains(t);
                        if ui.checkbox(&mut on, t.as_str()).changed() {
                            if on {
                                self.chip_tags.push(t.clone());
                            } else {
                                self.chip_tags.retain(|x| x != t);
                            }
                        }
                    }
                });
                if tags.is_empty() {
                    ui.label("Noch keine Schlagworte");
                }
            });
            if ui
                .add(egui::Button::new(chip(self.chip_fav, if self.chip_fav { "★ Favoriten" } else { "☆ Favoriten" })).frame(false))
                .clicked()
            {
                self.chip_fav = !self.chip_fav;
            }
            ui.menu_button(chip(false, &format!("⇅ {}", sort_label(self.sort))), |ui| {
                for s in [Sort::Newest, Sort::Name, Sort::Size, Sort::Type] {
                    if ui.selectable_value(&mut self.sort, s, sort_label(s)).clicked() {
                        ui.close();
                    }
                }
            });
            if (!self.chip_types.is_empty() || !self.chip_tags.is_empty() || self.chip_fav)
                && ui.add(egui::Button::new(chip(false, "× Zurücksetzen")).frame(false)).clicked()
            {
                self.chip_types.clear();
                self.chip_tags.clear();
                self.chip_fav = false;
            }
        });
        ui.add_space(6.0);
    }
}

fn chip_title(base: &str, n: usize) -> String {
    if n > 0 {
        format!("{base} · {n}")
    } else {
        base.to_string()
    }
}
