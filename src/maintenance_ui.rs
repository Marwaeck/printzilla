//! Windows for "Sicherung erstellen", "Sicherung wiederherstellen" and "Bibliothek prüfen".

use crate::app::DamApp;
use crate::library;
use crate::maintenance::{self, CheckReport, Manifest, Repair, RepairResult, RestoreResult};
use crate::widgets::*;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Backup,
    Restore,
    Check,
}

pub enum MaintMsg {
    Progress(usize, usize, String),
    Backup(Result<u64, String>),
    Restore(Result<RestoreResult, String>),
    Checked(CheckReport),
    Repaired(RepairResult),
}

/// What a chosen backup would do, worked out before restoring.
pub struct RestorePlan {
    pub path: PathBuf,
    pub manifest: Manifest,
    pub entries: usize,
    pub matching: usize,
    pub from_backup: usize,
    pub unavailable: usize,
}

#[derive(Default)]
pub struct Maint {
    pub mode: Option<Mode>,
    pub with_files: bool,
    pub backup_path: Option<PathBuf>,
    pub plan: Option<Result<RestorePlan, String>>,
    pub report: Option<CheckReport>,
    pub how: Repair,
    pub running: Option<Receiver<MaintMsg>>,
    pub progress: (usize, usize, String),
    /// Text shown when the job is finished.
    pub done: Option<String>,
    pub errors: Vec<String>,
}

fn sender_progress(tx: &Sender<MaintMsg>, ctx: &egui::Context) -> impl FnMut(usize, usize, &str) {
    let tx = tx.clone();
    let ctx = ctx.clone();
    move |d, t, s| {
        let _ = tx.send(MaintMsg::Progress(d, t, s.to_string()));
        ctx.request_repaint();
    }
}

fn list_box(ui: &mut egui::Ui, items: &[String]) {
    egui::Frame::new().fill(FIELD).corner_radius(8.0).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for it in items.iter().take(6) {
            ui.add(egui::Label::new(RichText::new(it).size(12.0).color(TEXT)).truncate());
        }
        if items.len() > 6 {
            ui.label(RichText::new(format!("… und {} weitere", items.len() - 6)).size(12.0).color(MUTED));
        }
    });
}

/// "1 Datei" / "3 Dateien"
fn n(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

impl DamApp {
    pub fn open_backup(&mut self) {
        self.maint = Maint {
            mode: Some(Mode::Backup),
            ..Default::default()
        };
    }

    pub fn open_restore(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Sicherung wählen").add_filter("PrintZilla-Sicherung", &["zip"]).pick_file() else {
            return;
        };
        let plan = maintenance::read_backup(&path).map(|info| {
            let hashes = self.lib.hashes();
            let mut matching = 0;
            let mut from_backup = 0;
            let mut unavailable = 0;
            for e in &info.index.entries {
                if hashes.contains(&e.hash) {
                    matching += 1;
                } else if info.files.contains(&e.file.replace('\\', "/")) {
                    from_backup += 1;
                } else {
                    unavailable += 1;
                }
            }
            RestorePlan {
                path: path.clone(),
                entries: info.index.entries.len(),
                manifest: info.manifest,
                matching,
                from_backup,
                unavailable,
            }
        });
        self.maint = Maint {
            mode: Some(Mode::Restore),
            plan: Some(plan),
            ..Default::default()
        };
    }

    pub fn open_check(&mut self, ctx: &egui::Context) {
        self.maint = Maint {
            mode: Some(Mode::Check),
            how: Repair {
                folder_as_tag: self.cfg.folder_as_tag,
                ..Default::default()
            },
            ..Default::default()
        };
        self.start_check(ctx);
    }

    fn start_check(&mut self, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let dir = self.lib.dir.clone();
        let entries = self.lib.index.entries.clone();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let mut p = sender_progress(&tx, &c);
            let r = maintenance::check_library(&dir, &entries, &mut p);
            let _ = tx.send(MaintMsg::Checked(r));
            c.request_repaint();
        });
        self.maint.report = None;
        self.maint.done = None;
        self.maint.progress = (0, 0, String::new());
        self.maint.running = Some(rx);
    }

    fn start_backup(&mut self, ctx: &egui::Context, dest: PathBuf) {
        let (tx, rx) = channel();
        let dir = self.lib.dir.clone();
        let index = library::Index {
            next_id: self.lib.index.next_id,
            entries: self.lib.index.entries.clone(),
        };
        let with_files = self.maint.with_files;
        let c = ctx.clone();
        let d2 = dest.clone();
        std::thread::spawn(move || {
            let mut p = sender_progress(&tx, &c);
            let r = maintenance::create_backup(&dir, &index, &d2, with_files, &mut p);
            let _ = tx.send(MaintMsg::Backup(r));
            c.request_repaint();
        });
        self.maint.backup_path = Some(dest);
        self.maint.running = Some(rx);
    }

    fn start_restore(&mut self, ctx: &egui::Context, path: PathBuf) {
        let (tx, rx) = channel();
        let dir = self.lib.dir.clone();
        let entries = self.lib.index.entries.clone();
        let first = self.lib.index.next_id.max(1);
        let c = ctx.clone();
        std::thread::spawn(move || {
            let mut p = sender_progress(&tx, &c);
            let r = maintenance::restore_backup(&dir, &entries, &path, first, &mut p);
            let _ = tx.send(MaintMsg::Restore(r));
            c.request_repaint();
        });
        self.maint.running = Some(rx);
    }

    fn start_repair(&mut self, ctx: &egui::Context) {
        let Some(report) = self.maint.report.clone() else { return };
        let (tx, rx) = channel();
        let dir = self.lib.dir.clone();
        let entries = self.lib.index.entries.clone();
        let first = self.lib.index.next_id.max(1);
        let how = self.maint.how;
        let c = ctx.clone();
        std::thread::spawn(move || {
            let mut p = sender_progress(&tx, &c);
            let r = maintenance::repair(&dir, &entries, &report, how, first, &mut p);
            let _ = tx.send(MaintMsg::Repaired(r));
            c.request_repaint();
        });
        self.maint.running = Some(rx);
    }

    /// Picks up messages from the background jobs.
    pub fn poll_maint(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.maint.running else { return };
        let mut msgs = Vec::new();
        while let Ok(m) = rx.try_recv() {
            msgs.push(m);
        }
        for m in msgs {
            match m {
                MaintMsg::Progress(d, t, s) => self.maint.progress = (d, t, s),
                MaintMsg::Checked(r) => {
                    self.maint.running = None;
                    self.maint.report = Some(r);
                }
                MaintMsg::Backup(r) => {
                    self.maint.running = None;
                    match r {
                        Ok(size) => {
                            self.maint.done = Some(format!("Sicherung gespeichert ({}).", library::human_size(size)));
                        }
                        Err(e) => self.maint.errors.push(e),
                    }
                }
                MaintMsg::Restore(r) => {
                    self.maint.running = None;
                    match r {
                        Ok(res) => {
                            maintenance::apply_restore(&mut self.lib, &res);
                            self.after_library_change();
                            let mut t = Vec::new();
                            if !res.updated.is_empty() {
                                t.push(format!("{} Einträge haben Namen, Schlagworte und Notizen zurückbekommen", res.updated.len()));
                            }
                            if !res.added.is_empty() {
                                t.push(format!("{} Dateien wurden aus der Sicherung zurückgeholt", res.added.len()));
                            }
                            if !res.unavailable.is_empty() {
                                t.push(format!("{} Einträge ließen sich nicht wiederherstellen (Datei fehlt)", res.unavailable.len()));
                            }
                            if t.is_empty() {
                                t.push("Die Sicherung enthielt nichts Neues.".into());
                            }
                            self.maint.done = Some(t.join(".\n") + ".");
                            self.maint.errors = res.errors;
                            self.notify(ctx, "Sicherung wiederhergestellt");
                        }
                        Err(e) => self.maint.errors.push(e),
                    }
                }
                MaintMsg::Repaired(r) => {
                    self.maint.running = None;
                    maintenance::apply_repair(&mut self.lib, &r);
                    self.after_library_change();
                    let s = r.summary();
                    self.notify(ctx, s.clone());
                    self.maint.done = Some(s);
                }
            }
        }
    }

    /// After entries were added, removed or moved by a background job.
    fn after_library_change(&mut self) {
        self.thumbs.clear();
        if let Some(id) = self.selected {
            if self.entry(id).is_none() {
                self.selected = None;
            }
        }
        // the 3D view may show a file that has moved
        self.viewer.entry_id = None;
        if let Some(w) = &self.watcher {
            w.update_known(&self.lib);
        }
    }

    pub fn maint_window(&mut self, ctx: &egui::Context) {
        let Some(mode) = self.maint.mode else { return };
        let mut close = false;
        let mut action: Option<Box<dyn FnOnce(&mut DamApp, &egui::Context)>> = None;
        let running = self.maint.running.is_some();

        egui::Modal::new(egui::Id::unique("maint-dialog"))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .corner_radius(14.0)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(70, 70, 76)))
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(540.0);
                let title = match mode {
                    Mode::Backup => "Bibliothek sichern",
                    Mode::Restore => "Sicherung wiederherstellen",
                    Mode::Check => "Bibliothek prüfen",
                };
                ui.label(RichText::new(title).size(19.0).strong().color(TEXT));
                ui.add_space(6.0);

                match mode {
                    Mode::Backup => self.backup_body(ui, &mut action),
                    Mode::Restore => self.restore_body(ui, &mut action),
                    Mode::Check => self.check_body(ui, &mut action),
                }

                if running {
                    ui.add_space(10.0);
                    let (d, t, s) = &self.maint.progress;
                    let frac = if *t > 0 { *d as f32 / *t as f32 } else { 0.0 };
                    ui.add(egui::ProgressBar::new(frac).show_percentage());
                    ui.add(egui::Label::new(RichText::new(s).size(11.5).color(MUTED)).truncate());
                }
                if let Some(d) = &self.maint.done {
                    ui.add_space(10.0);
                    ui.label(RichText::new(format!("✔  {d}")).color(Color32::from_rgb(140, 210, 120)));
                }
                for e in &self.maint.errors {
                    ui.label(RichText::new(e).size(12.0).color(DANGER));
                }

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let label = if self.maint.done.is_some() { "Fertig" } else { "Schließen" };
                    if ui.add_enabled(!running, egui::Button::new(label).min_size(Vec2::new(100.0, 30.0))).clicked() {
                        close = true;
                    }
                });
            });

        if let Some(a) = action {
            a(self, ctx);
        }
        if close || (!running && ctx.input(|i| i.key_pressed(egui::Key::Escape))) {
            self.maint = Maint::default();
        }
    }

    fn primary(ui: &mut egui::Ui, enabled: bool, text: &str) -> bool {
        ui.add_enabled(enabled, egui::Button::new(RichText::new(text).color(Color32::WHITE).strong()).fill(ACCENT).min_size(Vec2::new(0.0, 30.0)))
            .clicked()
    }

    fn backup_body(&mut self, ui: &mut egui::Ui, action: &mut Option<Box<dyn FnOnce(&mut DamApp, &egui::Context)>>) {
        let running = self.maint.running.is_some();
        ui.label(
            RichText::new(
                "Speichert Namen, Schlagworte, Beschreibungen, Favoriten, Quellen und Vorschaubilder in einer ZIP-Datei. \
                 Damit lässt sich die Ordnung der Bibliothek wiederherstellen – etwa auf einem neuen Rechner.",
            )
            .size(12.5)
            .color(MUTED),
        );
        ui.add_space(10.0);
        let total: u64 = self.lib.index.entries.iter().map(|e| e.size).sum();
        ui.add_enabled_ui(!running && self.maint.done.is_none(), |ui| {
            ui.checkbox(&mut self.maint.with_files, format!("Auch alle 3D-Dateien sichern (etwa {})", library::human_size(total)));
        });
        ui.label(
            RichText::new(if self.maint.with_files {
                "Vollständige Sicherung: enthält die Dateien selbst und reicht allein zum Wiederherstellen."
            } else {
                "Ohne Dateien bleibt die Sicherung klein. Die Dateien selbst sicherst du z. B. mit Time Machine oder dem Dateiversionsverlauf."
            })
            .size(11.5)
            .color(MUTED),
        );
        ui.add_space(10.0);
        if let (Some(p), Some(_)) = (&self.maint.backup_path, &self.maint.done) {
            ui.add(egui::Label::new(RichText::new(p.to_string_lossy()).size(12.0).color(TEXT)).wrap());
            if ui.button(reveal_label()).clicked() {
                open_path(p, true);
            }
        } else if Self::primary(ui, !running, &format!("{} sichern …", n(self.lib.index.entries.len(), "Eintrag", "Einträge"))) {
            if let Some(dest) = rfd::FileDialog::new()
                .set_title("Sicherung speichern unter")
                .set_file_name(maintenance::default_backup_name())
                .add_filter("ZIP", &["zip"])
                .save_file()
            {
                *action = Some(Box::new(move |app, ctx| app.start_backup(ctx, dest)));
            }
        }
    }

    fn restore_body(&mut self, ui: &mut egui::Ui, action: &mut Option<Box<dyn FnOnce(&mut DamApp, &egui::Context)>>) {
        let running = self.maint.running.is_some();
        let Some(plan) = &self.maint.plan else { return };
        let plan = match plan {
            Ok(p) => p,
            Err(e) => {
                ui.label(RichText::new(e).color(DANGER));
                return;
            }
        };
        let m = &plan.manifest;
        let when = if m.created > 0 { library::format_date(m.created) } else { "unbekanntem Datum".into() };
        ui.label(RichText::new(plan.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()).strong().color(TEXT));
        ui.label(
            RichText::new(format!(
                "Sicherung vom {when}{} · {} Einträge · {}",
                if m.app_version.is_empty() { String::new() } else { format!(" (PrintZilla {})", m.app_version) },
                plan.entries,
                if m.with_files { "mit Dateien" } else { "ohne Dateien" }
            ))
            .size(12.0)
            .color(MUTED),
        );
        ui.add_space(10.0);
        egui::Grid::new("restore-plan").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label(RichText::new(plan.matching.to_string()).strong().color(TEXT));
            ui.label("schon in der Bibliothek – bekommen Namen, Schlagworte, Beschreibung, Favorit und Quelle zurück");
            ui.end_row();
            ui.label(RichText::new(plan.from_backup.to_string()).strong().color(TEXT));
            ui.label("werden aus der Sicherung in die Bibliothek zurückgeholt");
            ui.end_row();
            if plan.unavailable > 0 {
                ui.label(RichText::new(plan.unavailable.to_string()).strong().color(DANGER));
                ui.label("fehlen auf der Festplatte und in der Sicherung – werden übersprungen");
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        ui.label(
            RichText::new("Es wird nichts gelöscht. Der bisherige Stand der Bibliothek wird vorher als Kopie aufbewahrt.")
                .size(11.5)
                .color(MUTED),
        );
        ui.add_space(10.0);
        if self.maint.done.is_none() && Self::primary(ui, !running && plan.matching + plan.from_backup > 0, "Wiederherstellen") {
            let path = plan.path.clone();
            *action = Some(Box::new(move |app, ctx| app.start_restore(ctx, path)));
        }
    }

    fn check_body(&mut self, ui: &mut egui::Ui, action: &mut Option<Box<dyn FnOnce(&mut DamApp, &egui::Context)>>) {
        let running = self.maint.running.is_some();
        ui.label(
            RichText::new("Vergleicht die Bibliothek mit dem, was wirklich im Bibliotheksordner liegt.")
                .size(12.5)
                .color(MUTED),
        );
        let Some(r) = self.maint.report.clone() else {
            if running {
                ui.add_space(6.0);
                ui.label("Bibliothek wird geprüft …");
            }
            return;
        };
        ui.add_space(10.0);
        if self.maint.done.is_some() {
            if ui.button("Noch einmal prüfen").clicked() {
                *action = Some(Box::new(|app, ctx| app.start_check(ctx)));
            }
            return;
        }
        if r.is_clean() {
            ui.label(RichText::new("✔  Alles in Ordnung: Bibliothek und Ordner passen zusammen.").color(Color32::from_rgb(140, 210, 120)));
            return;
        }
        let how = &mut self.maint.how;
        let lib = &self.lib;
        let name_of = |id: u64| lib.index.entries.iter().find(|e| e.id == id).map(|e| e.name.clone()).unwrap_or_default();

        if r.moved() > 0 {
            ui.checkbox(&mut how.relink, RichText::new(format!("{} verschoben oder umbenannt – wieder zuordnen", r.moved())).strong());
            let items: Vec<String> = r.missing.iter().filter_map(|m| m.found_at.as_ref().map(|f| format!("{}  →  {f}", m.file))).collect();
            list_box(ui, &items);
            ui.add_space(8.0);
        }
        if r.gone() > 0 {
            ui.checkbox(&mut how.remove_gone, RichText::new(format!("{} ohne Datei – aus der Bibliothek entfernen", n(r.gone(), "Eintrag", "Einträge"))).strong());
            let items: Vec<String> = r.missing.iter().filter(|m| m.found_at.is_none()).map(|m| format!("{}  ({})", m.name, m.file)).collect();
            list_box(ui, &items);
            ui.add_space(8.0);
        }
        if r.new_files() > 0 {
            ui.checkbox(&mut how.add_new, RichText::new(format!("{} von außen dazugelegt – aufnehmen", n(r.new_files(), "Datei", "Dateien"))).strong());
            let items: Vec<String> = r
                .untracked
                .iter()
                .filter(|u| u.relinks.is_none() && u.duplicate_of.is_none())
                .map(|u| format!("{}  ({})", u.file, library::human_size(u.size)))
                .collect();
            list_box(ui, &items);
            ui.add_space(8.0);
        }
        if r.duplicates() > 0 {
            ui.add_enabled_ui(how.add_new, |ui| {
                ui.checkbox(
                    &mut how.add_duplicates,
                    RichText::new(format!("{} (gleicher Inhalt schon in der Bibliothek) – trotzdem aufnehmen", n(r.duplicates(), "doppelte Datei", "doppelte Dateien"))).strong(),
                );
            });
            let items: Vec<String> = r
                .untracked
                .iter()
                .filter(|u| u.relinks.is_none())
                .filter_map(|u| u.duplicate_of.map(|id| format!("{}  =  {}", u.file, name_of(id))))
                .collect();
            list_box(ui, &items);
            ui.add_space(8.0);
        }
        if !r.changed.is_empty() {
            ui.checkbox(&mut how.refresh_changed, RichText::new(format!("{} – neu einlesen", n(r.changed.len(), "Datei wurde verändert", "Dateien wurden verändert"))).strong());
            let items: Vec<String> = r.changed.iter().map(|id| name_of(*id)).collect();
            list_box(ui, &items);
            ui.add_space(8.0);
        }
        if !r.stray_thumbs.is_empty() {
            ui.checkbox(&mut how.remove_stray_thumbs, RichText::new(format!("{} ohne Eintrag – löschen", n(r.stray_thumbs.len(), "altes Vorschaubild", "alte Vorschaubilder"))).strong());
            ui.add_space(8.0);
        }
        ui.add_space(4.0);
        if Self::primary(ui, !running, "Ausgewähltes übernehmen") {
            *action = Some(Box::new(|app, ctx| app.start_repair(ctx)));
        }
    }
}
