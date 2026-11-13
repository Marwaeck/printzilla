//! The import window: pick a folder, see what was found (grouped by folder),
//! choose files and decide what happens to the originals.

use crate::app::{DamApp, ImportDialog};
use crate::library::{self, AfterImport, Config, Library};
use crate::widgets::*;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};

const GREEN: Color32 = Color32::from_rgb(76, 175, 110);

fn after_label(a: AfterImport) -> &'static str {
    match a {
        AfterImport::Keep => "Originale behalten",
        AfterImport::Trash => "Originale in den Papierkorb legen",
        AfterImport::Delete => "Originale endgültig löschen",
    }
}

fn do_scan(d: &mut ImportDialog, cfg: &Config, lib: &Library) {
    if let Some(f) = &d.folder {
        d.found = library::scan(f, cfg.recursive, &cfg.import_types, lib);
        d.scanned = true;
    }
}

/// 0 = empty, 1 = partly, 2 = checked
fn paint_check(p: &egui::Painter, r: Rect, state: u8) {
    if state == 0 {
        p.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.5, Color32::from_rgb(130, 130, 138)), StrokeKind::Inside);
        return;
    }
    p.rect_filled(r, CornerRadius::same(4), ACCENT);
    let s = Stroke::new(2.0, Color32::WHITE);
    if state == 1 {
        p.line_segment([Pos2::new(r.left() + 4.0, r.center().y), Pos2::new(r.right() - 4.0, r.center().y)], s);
    } else {
        p.line_segment([Pos2::new(r.left() + 3.5, r.center().y + 0.5), Pos2::new(r.left() + 6.5, r.bottom() - 4.0)], s);
        p.line_segment([Pos2::new(r.left() + 6.5, r.bottom() - 4.0), Pos2::new(r.right() - 3.5, r.top() + 4.0)], s);
    }
}

fn pill(p: &egui::Painter, right_center: Pos2, text: &str, fill: Color32, col: Color32) -> f32 {
    let w = 14.0 + text.chars().count() as f32 * 6.6;
    let r = Rect::from_min_size(Pos2::new(right_center.x - w, right_center.y - 10.0), Vec2::new(w, 20.0));
    p.rect_filled(r, CornerRadius::same(10), fill);
    p.text(r.center(), Align2::CENTER_CENTER, text, FontId::proportional(11.5), col);
    w
}

fn one_line(p: &egui::Painter, pos: Pos2, text: &str, size: f32, col: Color32, max_w: f32) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), FontId::proportional(size), col);
    job.wrap.max_width = max_w.max(20.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let g = p.layout_job(job);
    p.galley(Pos2::new(pos.x, pos.y - g.size().y / 2.0), g, col);
}

fn stat_tile(ui: &mut egui::Ui, number: usize, label: &str, col: Color32) {
    egui::Frame::new().fill(FIELD).corner_radius(10.0).inner_margin(egui::Margin::symmetric(18, 12)).show(ui, |ui| {
        ui.set_min_width(150.0);
        ui.vertical(|ui| {
            ui.label(RichText::new(number.to_string()).size(26.0).strong().color(col));
            ui.label(RichText::new(label).size(12.5).color(MUTED));
        });
    });
}

impl DamApp {
    pub fn import_window(&mut self, ctx: &egui::Context) {
        if self.import.is_none() {
            return;
        }
        let screen = ctx.content_rect();
        let width = (screen.width() - 120.0).clamp(520.0, 880.0);
        let list_h = (screen.height() - 470.0).clamp(150.0, 520.0);
        let mut close = false;
        let mut start = false;
        let running = self.import.as_ref().map(|d| d.running.is_some()).unwrap_or(false);
        let cfg = &mut self.cfg;
        let lib = &self.lib;
        let d = self.import.as_mut().unwrap();

        egui::Modal::new(egui::Id::unique("import-dialog"))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .corner_radius(14.0)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(70, 70, 76)))
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(width);

                // header
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
                    draw_logo(ui.painter(), r);
                    ui.label(RichText::new("Dateien importieren").size(19.0).strong().color(TEXT));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !running && tool_button(ui, "×").on_hover_text("Schließen (Esc)").clicked() {
                            close = true;
                        }
                    });
                });
                ui.label(
                    RichText::new("PrintZilla durchsucht einen Ordner nach 3D-Druck-Dateien und kopiert sie in deine Bibliothek.")
                        .size(12.5)
                        .color(MUTED),
                );
                ui.add_space(14.0);

                // --- running -------------------------------------------------
                if running {
                    let (done, total, cur) = d.progress.clone();
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Dateien werden übernommen …").size(17.0).color(TEXT));
                        ui.add_space(10.0);
                    });
                    let f = if total > 0 { done as f32 / total as f32 } else { 0.0 };
                    ui.add(egui::ProgressBar::new(f).desired_height(14.0).fill(ACCENT).corner_radius(7.0));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{done} von {total} Dateien")).color(TEXT));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(cur).color(MUTED));
                        });
                    });
                    ui.add_space(40.0);
                    return;
                }

                // --- finished -------------------------------------------------
                if let Some(r) = d.report.clone() {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let (rc, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                        ui.painter().circle_filled(rc.center(), 17.0, GREEN);
                        paint_check(ui.painter(), Rect::from_center_size(rc.center(), Vec2::splat(18.0)), 2);
                        ui.label(RichText::new("Import abgeschlossen").size(18.0).strong().color(TEXT));
                    });
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        stat_tile(ui, r.entries.len(), "neu in der Bibliothek", GREEN);
                        stat_tile(ui, r.duplicates, "schon vorhanden, übersprungen", TEXT);
                        stat_tile(ui, r.deleted, "Originale entfernt", TEXT);
                    });
                    if !r.errors.is_empty() {
                        ui.add_space(12.0);
                        ui.label(RichText::new(format!("{} Probleme", r.errors.len())).strong().color(DANGER));
                        egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                            for e in &r.errors {
                                ui.label(RichText::new(e).size(12.0).color(TEXT));
                            }
                        });
                    }
                    ui.add_space(18.0);
                    let mut again = false;
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Button::new(RichText::new("Fertig").color(Color32::WHITE).strong()).fill(ACCENT).min_size(Vec2::new(120.0, 32.0)))
                            .clicked()
                        {
                            close = true;
                        }
                        if ui.add(egui::Button::new("Weiteren Ordner importieren").min_size(Vec2::new(0.0, 32.0))).clicked() {
                            again = true;
                        }
                    });
                    if again {
                        d.report = None;
                        d.found.clear();
                        d.scanned = false;
                        d.folder = None;
                    }
                    return;
                }

                // --- 1: folder -----------------------------------------------
                let mut rescan = false;
                egui::Frame::new().fill(FIELD).corner_radius(10.0).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(26.0, 30.0), Sense::hover());
                        paint_folder(ui.painter(), r.center(), if d.folder.is_some() { ACCENT } else { MUTED });
                        let btn_w = 150.0;
                        ui.allocate_ui(Vec2::new(ui.available_width() - btn_w - 10.0, 34.0), |ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Quellordner").size(11.5).color(MUTED));
                                let text = d.folder.as_ref().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| "Noch kein Ordner gewählt".into());
                                ui.add(egui::Label::new(RichText::new(text).size(13.5).color(TEXT)).truncate());
                            });
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let primary = d.folder.is_none();
                            let b = egui::Button::new(RichText::new("Ordner wählen …").color(if primary { Color32::WHITE } else { TEXT }))
                                .fill(if primary { ACCENT } else { Color32::from_rgb(70, 70, 76) })
                                .min_size(Vec2::new(btn_w, 32.0));
                            if ui.add(b).clicked() {
                                let mut dlg = rfd::FileDialog::new().set_title("Ordner zum Importieren wählen");
                                if let Some(f) = &d.folder {
                                    dlg = dlg.set_directory(f);
                                }
                                if let Some(f) = dlg.pick_folder() {
                                    d.folder = Some(f.clone());
                                    cfg.last_import_dir = Some(f);
                                    rescan = true;
                                }
                            }
                        });
                    });
                });
                ui.add_space(10.0);

                // --- summary + settings ---------------------------------------
                let total = d.found.len();
                let new = d.found.iter().filter(|f| !f.maybe_known).count();
                let mut folders: Vec<&str> = d.found.iter().map(|f| f.rel_dir.as_str()).collect();
                folders.dedup();
                ui.horizontal(|ui| {
                    if d.scanned {
                        ui.label(RichText::new(format!("{total} Dateien gefunden")).strong().color(TEXT));
                        if total > 0 {
                            ui.label(RichText::new(format!("· {new} neu")).color(GREEN));
                            if total > new {
                                ui.label(RichText::new(format!("· {} schon in der Bibliothek", total - new)).color(MUTED));
                            }
                            ui.label(RichText::new(format!("· {} Ordner", folders.len())).color(MUTED));
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let arrow = if d.show_options { "▴" } else { "▾" };
                        if ui
                            .add(egui::Button::new(RichText::new(format!("Einstellungen {arrow}")).color(TEXT)).frame(false))
                            .on_hover_text("Unterordner, Dateitypen, doppelte Dateien")
                            .clicked()
                        {
                            d.show_options = !d.show_options;
                        }
                    });
                });
                if d.show_options {
                    ui.add_space(4.0);
                    egui::Frame::new().fill(FIELD).corner_radius(10.0).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_wrapped(|ui| {
                            rescan |= ui.checkbox(&mut cfg.recursive, "Unterordner mit durchsuchen").changed();
                            ui.add_space(10.0);
                            ui.checkbox(&mut cfg.folder_as_tag, "Ordnername als Schlagwort")
                                .on_hover_text("Dateien aus dem Ordner »Halterungen« landen im Ordner »Halterungen« der Bibliothek");
                            ui.add_space(10.0);
                            ui.checkbox(&mut d.skip_duplicates, "Doppelte Dateien überspringen");
                        });
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Dateitypen").color(MUTED));
                            ui.spacing_mut().item_spacing.x = 5.0;
                            for (ext, desc) in library::ALL_TYPES {
                                let on = cfg.import_types.iter().any(|t| t == ext);
                                let b = egui::Button::new(RichText::new(ext.to_uppercase()).size(12.0).color(if on { Color32::WHITE } else { MUTED }))
                                    .fill(if on { Color32::from_rgb(40, 92, 175) } else { Color32::from_rgb(64, 64, 69) })
                                    .corner_radius(10.0);
                                if ui.add(b).on_hover_text(*desc).clicked() {
                                    if on {
                                        cfg.import_types.retain(|t| t != ext);
                                    } else {
                                        cfg.import_types.push(ext.to_string());
                                    }
                                    rescan = true;
                                }
                            }
                        });
                    });
                }
                if rescan {
                    do_scan(d, cfg, lib);
                }
                ui.add_space(8.0);

                // --- 2: file list --------------------------------------------
                let (list_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), list_h), Sense::hover());
                ui.painter().rect_filled(list_rect, CornerRadius::same(10), Color32::from_rgb(32, 32, 35));
                let mut list_ui = ui.new_child(egui::UiBuilder::new().max_rect(list_rect.shrink(6.0)));
                if !d.scanned {
                    list_ui.vertical_centered(|ui| {
                        ui.add_space(list_h * 0.32);
                        ui.label(RichText::new("Wähle oben einen Ordner, z. B. deinen Download-Ordner.").size(14.0).color(MUTED));
                        ui.label(RichText::new("Er wird sofort durchsucht – kopiert wird erst, wenn du auf »Importieren« klickst.").size(12.5).color(MUTED));
                    });
                } else if d.found.is_empty() {
                    list_ui.vertical_centered(|ui| {
                        ui.add_space(list_h * 0.32);
                        ui.label(RichText::new("In diesem Ordner wurden keine passenden Dateien gefunden.").size(14.0).color(MUTED));
                        ui.label(RichText::new("Unter »Einstellungen« kannst du weitere Dateitypen einschalten.").size(12.5).color(MUTED));
                    });
                } else {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut list_ui, |ui| {
                        let w = ui.available_width();
                        let mut i = 0;
                        while i < d.found.len() {
                            let dir = d.found[i].rel_dir.clone();
                            let end = (i..d.found.len()).find(|&k| d.found[k].rel_dir != dir).unwrap_or(d.found.len());
                            // group header
                            let sel = d.found[i..end].iter().filter(|f| f.selected).count();
                            let n = end - i;
                            let gnew = d.found[i..end].iter().filter(|f| !f.maybe_known).count();
                            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
                            let p = ui.painter();
                            if resp.hovered() {
                                p.rect_filled(r, CornerRadius::same(6), Color32::from_rgb(44, 44, 48));
                            }
                            let state = if sel == 0 { 0 } else if sel == n { 2 } else { 1 };
                            paint_check(p, Rect::from_center_size(Pos2::new(r.left() + 16.0, r.center().y), Vec2::splat(16.0)), state);
                            paint_folder(p, Pos2::new(r.left() + 42.0, r.center().y), MUTED);
                            let title = if dir.is_empty() { "Hauptordner".to_string() } else { dir.clone() };
                            one_line(p, Pos2::new(r.left() + 60.0, r.center().y), &title, 13.5, TEXT, w - 260.0);
                            p.text(
                                Pos2::new(r.right() - 10.0, r.center().y),
                                Align2::RIGHT_CENTER,
                                format!("{n} Dateien · {gnew} neu"),
                                FontId::proportional(12.0),
                                MUTED,
                            );
                            if resp.clicked() {
                                let v = sel < n;
                                d.found[i..end].iter_mut().for_each(|f| f.selected = v);
                            }
                            // files
                            for f in d.found[i..end].iter_mut() {
                                let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 34.0), Sense::click());
                                let p = ui.painter();
                                if resp.hovered() {
                                    p.rect_filled(r, CornerRadius::same(6), Color32::from_rgb(44, 44, 48));
                                }
                                let x = r.left() + 30.0;
                                paint_check(p, Rect::from_center_size(Pos2::new(x + 12.0, r.center().y), Vec2::splat(16.0)), if f.selected { 2 } else { 0 });
                                let badge = Rect::from_min_size(Pos2::new(x + 32.0, r.center().y - 10.0), Vec2::new(58.0, 20.0));
                                p.rect_filled(badge, CornerRadius::same(4), type_color(&f.ext));
                                p.text(badge.center(), Align2::CENTER_CENTER, f.ext.to_uppercase(), FontId::proportional(11.0), Color32::from_rgb(25, 25, 28));
                                let name_col = if f.selected { TEXT } else { MUTED };
                                one_line(p, Pos2::new(x + 102.0, r.center().y), &f.name, 13.5, name_col, w - 30.0 - 102.0 - 230.0);
                                p.text(
                                    Pos2::new(r.right() - 10.0, r.center().y),
                                    Align2::RIGHT_CENTER,
                                    library::human_size(f.size),
                                    FontId::monospace(11.5),
                                    MUTED,
                                );
                                if f.maybe_known {
                                    pill(p, Pos2::new(r.right() - 90.0, r.center().y), "schon vorhanden", Color32::from_rgb(64, 64, 69), MUTED);
                                } else {
                                    pill(p, Pos2::new(r.right() - 90.0, r.center().y), "neu", Color32::from_rgb(38, 82, 56), Color32::from_rgb(150, 230, 170));
                                }
                                let resp = resp.on_hover_text(f.path.to_string_lossy());
                                if resp.clicked() {
                                    f.selected = !f.selected;
                                }
                            }
                            ui.add_space(4.0);
                            i = end;
                        }
                    });
                }
                ui.add_space(6.0);
                if d.scanned && !d.found.is_empty() {
                    let sel = d.found.iter().filter(|f| f.selected).count();
                    let sel_size: u64 = d.found.iter().filter(|f| f.selected).map(|f| f.size).sum();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Auswählen:").color(MUTED));
                        if ui.small_button("Alle").clicked() {
                            d.found.iter_mut().for_each(|f| f.selected = true);
                        }
                        if ui.small_button("Nur neue").clicked() {
                            d.found.iter_mut().for_each(|f| f.selected = !f.maybe_known);
                        }
                        if ui.small_button("Keine").clicked() {
                            d.found.iter_mut().for_each(|f| f.selected = false);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(format!("{sel} von {total} ausgewählt · {}", library::human_size(sel_size))).color(TEXT));
                        });
                    });
                }

                // --- 3: footer -----------------------------------------------
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(8.0);
                let sel = d.found.iter().filter(|f| f.selected).count();
                if d.confirm_delete {
                    egui::Frame::new().fill(Color32::from_rgb(70, 36, 34)).corner_radius(10.0).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(RichText::new(format!("Wirklich {sel} Originaldateien nach dem Kopieren endgültig löschen?")).strong().color(Color32::WHITE));
                        ui.label(RichText::new("Sie landen nicht im Papierkorb. Die Kopien in der Bibliothek bleiben erhalten.").size(12.0).color(Color32::from_rgb(230, 200, 196)));
                        ui.add_space(6.0);
                        let mut back = false;
                        ui.horizontal(|ui| {
                            if ui
                                .add(egui::Button::new(RichText::new("Ja, importieren und löschen").color(Color32::WHITE)).fill(Color32::from_rgb(180, 60, 50)).min_size(Vec2::new(0.0, 30.0)))
                                .clicked()
                            {
                                start = true;
                            }
                            if ui.add(egui::Button::new("Zurück").min_size(Vec2::new(0.0, 30.0))).clicked() {
                                back = true;
                            }
                        });
                        if back {
                            d.confirm_delete = false;
                        }
                    });
                } else {
                    let mut ask = false;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Nach dem Import:").color(MUTED));
                        egui::ComboBox::from_id_salt("after-import")
                            .width(260.0)
                            .selected_text(after_label(cfg.after_import))
                            .show_ui(ui, |ui| {
                                for a in [AfterImport::Keep, AfterImport::Trash, AfterImport::Delete] {
                                    ui.selectable_value(&mut cfg.after_import, a, after_label(a));
                                }
                            });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let label = if sel == 1 { "1 Datei importieren".to_string() } else { format!("{sel} Dateien importieren") };
                            let b = egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
                                .fill(ACCENT)
                                .min_size(Vec2::new(190.0, 34.0));
                            if ui.add_enabled(sel > 0, b).clicked() {
                                if cfg.after_import == AfterImport::Delete {
                                    ask = true;
                                } else {
                                    start = true;
                                }
                            }
                            if ui.add(egui::Button::new("Abbrechen").min_size(Vec2::new(100.0, 34.0))).clicked() {
                                close = true;
                            }
                        });
                    });
                    let (hint, col) = match cfg.after_import {
                        AfterImport::Keep => ("Die Dateien werden kopiert, der Quellordner bleibt unverändert.", MUTED),
                        AfterImport::Trash => ("Ein Original wandert erst in den Papierkorb, wenn seine Kopie sicher in der Bibliothek liegt.", MUTED),
                        AfterImport::Delete => ("Achtung: Ein Original wird endgültig gelöscht, sobald seine Kopie sicher in der Bibliothek liegt.", Color32::from_rgb(235, 160, 90)),
                    };
                    ui.label(RichText::new(hint).size(12.0).color(col));
                    if ask {
                        d.confirm_delete = true;
                    }
                }
            });

        if !running && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if start {
            self.start_import(ctx);
        }
        let running = self.import.as_ref().map(|d| d.running.is_some()).unwrap_or(false);
        if close && !running {
            self.import = None;
            self.save_cfg();
        }
    }
}
