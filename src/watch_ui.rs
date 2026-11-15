//! "Ordner überwachen": which folders PrintZilla watches, and the hint in the sidebar
//! when new files were found there.

use crate::app::DamApp;
use crate::library::WatchFolder;
use crate::widgets::*;
use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};

impl DamApp {
    /// Blue hint in the sidebar: "3 neue Dateien gefunden – Übernehmen".
    pub fn watch_hint(&mut self, ui: &mut egui::Ui) {
        let n = self.watcher.as_ref().map(|w| w.pending.len()).unwrap_or(0);
        if n == 0 {
            return;
        }
        let text = if n == 1 { "1 neue Datei gefunden".to_string() } else { format!("{n} neue Dateien gefunden") };
        let mut open = false;
        egui::Frame::new()
            .fill(Color32::from_rgb(30, 62, 112))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(format!("⬇  {text}")).strong().color(Color32::WHITE));
                ui.label(RichText::new("in den überwachten Ordnern").size(11.5).color(Color32::from_rgb(190, 210, 240)));
                ui.add_space(4.0);
                let b = egui::Button::new(RichText::new("Ansehen und übernehmen …").color(Color32::WHITE)).fill(ACCENT);
                if ui.add_sized([ui.available_width(), 26.0], b).clicked() {
                    open = true;
                }
            });
        ui.add_space(8.0);
        if open {
            self.open_import_found();
        }
    }

    pub fn watch_window(&mut self, ctx: &egui::Context) {
        if !self.watch_open {
            return;
        }
        let mut close = false;
        let mut changed = false;
        let mut remove: Option<usize> = None;
        let mut add: Option<std::path::PathBuf> = None;
        let mut reset_ignored = false;
        let downloads = crate::watcher::downloads_folder();
        let (pending, last) = self.watcher.as_ref().map(|w| (w.pending.len(), w.last_check)).unwrap_or((0, None));

        egui::Modal::new(egui::Id::unique("watch-dialog"))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .corner_radius(14.0)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(70, 70, 76)))
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(560.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(30.0, 30.0), Sense::hover());
                    paint_folder(ui.painter(), r.center(), ACCENT);
                    ui.label(RichText::new("Ordner überwachen").size(19.0).strong().color(TEXT));
                });
                ui.label(
                    RichText::new(
                        "PrintZilla schaut regelmäßig in diese Ordner. Sobald dort eine neue 3D-Druck-Datei fertig \
                         heruntergeladen ist, bekommst du einen Hinweis und kannst sie mit einem Klick übernehmen. \
                         Ohne deine Bestätigung wird nichts kopiert.",
                    )
                    .size(12.5)
                    .color(MUTED),
                );
                ui.add_space(12.0);
                changed |= ui.checkbox(&mut self.cfg.watch_enabled, "Überwachung eingeschaltet").changed();
                changed |= ui
                    .checkbox(&mut self.cfg.watch_notify, "Zusätzlich eine Mitteilung des Systems anzeigen")
                    .changed();
                ui.add_space(10.0);

                egui::Frame::new().fill(FIELD).corner_radius(10.0).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if self.cfg.watch_folders.is_empty() {
                        ui.label(RichText::new("Noch kein Ordner gewählt.").color(MUTED));
                    }
                    for (i, wf) in self.cfg.watch_folders.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::hover());
                            paint_folder(ui.painter(), r.center(), if wf.path.is_dir() { TEXT } else { DANGER });
                            let w = ui.available_width() - 190.0;
                            ui.allocate_ui(Vec2::new(w.max(80.0), 22.0), |ui| {
                                let mut t = RichText::new(wf.path.to_string_lossy()).color(TEXT);
                                if !wf.path.is_dir() {
                                    t = t.color(DANGER);
                                }
                                ui.add(egui::Label::new(t).truncate()).on_hover_text(wf.path.to_string_lossy());
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if tool_button(ui, "×").on_hover_text("Nicht mehr überwachen").clicked() {
                                    remove = Some(i);
                                }
                                changed |= ui.checkbox(&mut wf.recursive, "mit Unterordnern").changed();
                            });
                        });
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Ordner hinzufügen …").clicked() {
                        if let Some(p) = rfd::FileDialog::new().set_title("Ordner zum Überwachen wählen").pick_folder() {
                            add = Some(p);
                        }
                    }
                    if let Some(d) = &downloads {
                        if !self.cfg.watch_folders.iter().any(|w| &w.path == d) && ui.button("Downloads-Ordner hinzufügen").clicked() {
                            add = Some(d.clone());
                        }
                    }
                });

                ui.add_space(10.0);
                let status = match (self.cfg.watch_enabled, self.cfg.watch_folders.is_empty()) {
                    (false, _) => "Überwachung ist ausgeschaltet.".to_string(),
                    (true, true) => "Füge einen Ordner hinzu, um die Überwachung zu starten.".to_string(),
                    (true, false) => {
                        let when = last.map(crate::library::format_date).unwrap_or_else(|| "gleich".into());
                        format!(
                            "Geprüft wird alle {} Sekunden (zuletzt {when}). Gerade warten {pending} neue Dateien.",
                            crate::watcher::INTERVAL.as_secs()
                        )
                    }
                };
                ui.label(RichText::new(status).size(12.0).color(MUTED));
                if !self.cfg.watch_ignored.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} Dateien werden ignoriert.", self.cfg.watch_ignored.len())).size(12.0).color(MUTED));
                        if ui.small_button("Wieder anzeigen").clicked() {
                            reset_ignored = true;
                        }
                    });
                }

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if pending > 0
                        && ui
                            .add(
                                egui::Button::new(RichText::new(format!("{pending} neue Dateien ansehen …")).color(Color32::WHITE).strong())
                                    .fill(ACCENT)
                                    .min_size(Vec2::new(0.0, 30.0)),
                            )
                            .clicked()
                    {
                        close = true;
                        self.open_import_found();
                    }
                    if ui.add(egui::Button::new("Fertig").min_size(Vec2::new(100.0, 30.0))).clicked() {
                        close = true;
                    }
                });
            });

        if let Some(i) = remove {
            self.cfg.watch_folders.remove(i);
            changed = true;
        }
        if let Some(p) = add {
            if !self.cfg.watch_folders.iter().any(|w| w.path == p) {
                self.cfg.watch_folders.push(WatchFolder { path: p, recursive: true });
            }
            self.cfg.watch_enabled = true;
            changed = true;
        }
        if reset_ignored {
            self.cfg.watch_ignored.clear();
            changed = true;
        }
        if changed {
            self.watch_settings_changed();
        }
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.watch_open = false;
        }
    }
}
