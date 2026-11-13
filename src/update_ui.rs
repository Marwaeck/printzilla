//! The "Updates" window.

use crate::app::DamApp;
use crate::updater::{UpdateState, Updater, CURRENT_VERSION};
use crate::widgets::*;
use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};

impl DamApp {
    /// A small hint in the toolbar when a new version was found by itself.
    pub fn update_hint(&mut self, ui: &mut egui::Ui) {
        if let UpdateState::Available { version, .. } = &self.updater.state {
            let b = egui::Button::new(RichText::new(format!("⬆  Update auf Version {version}")).size(13.0).color(Color32::WHITE))
                .fill(Color32::from_rgb(46, 140, 90))
                .corner_radius(8.0)
                .min_size(Vec2::new(ui.available_width(), 30.0));
            if ui.add(b).on_hover_text("Neue Version ansehen und installieren").clicked() {
                self.updater.open = true;
            }
            ui.add_space(8.0);
        }
    }

    pub fn open_updates(&mut self, ctx: &egui::Context) {
        self.updater.open = true;
        if !self.updater.busy() && !matches!(self.updater.state, UpdateState::Available { .. } | UpdateState::Installed { .. }) {
            let repo = self.cfg.update_repo.clone();
            self.updater.start_check(ctx, &repo, false);
        }
    }

    pub fn update_window(&mut self, ctx: &egui::Context) {
        if !self.updater.open {
            return;
        }
        let mut close = false;
        let mut check = false;
        let mut install: Option<String> = None;
        let mut restart = false;
        let mut save = false;
        egui::Modal::new(egui::Id::unique("update-dialog"))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .corner_radius(14.0)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(70, 70, 76)))
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(480.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(44.0), Sense::hover());
                    draw_logo(ui.painter(), r);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("PrintZilla").size(19.0).strong().color(TEXT));
                        ui.label(RichText::new(format!("Installierte Version {CURRENT_VERSION}")).color(MUTED));
                    });
                });
                ui.add_space(14.0);

                let state = self.updater.state.clone();
                egui::Frame::new().fill(FIELD).corner_radius(10.0).inner_margin(egui::Margin::same(14)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    match &state {
                        UpdateState::Idle => {
                            ui.label(RichText::new("Noch nicht nach Updates gesucht.").color(TEXT));
                        }
                        UpdateState::Checking => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(RichText::new("Suche nach einer neuen Version …").color(TEXT));
                            });
                        }
                        UpdateState::UpToDate => {
                            ui.label(RichText::new("✔  PrintZilla ist auf dem neuesten Stand.").color(Color32::from_rgb(120, 210, 150)));
                        }
                        UpdateState::Available { version, notes } => {
                            ui.label(RichText::new(format!("Version {version} ist verfügbar.")).strong().size(15.0).color(TEXT));
                            if !notes.trim().is_empty() {
                                ui.add_space(6.0);
                                ui.label(RichText::new("Neu in dieser Version:").color(MUTED));
                                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                                    ui.label(RichText::new(notes.trim()).size(12.5).color(TEXT));
                                });
                            }
                        }
                        UpdateState::Installing { version } => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(RichText::new(format!("Version {version} wird heruntergeladen und installiert …")).color(TEXT));
                            });
                        }
                        UpdateState::Installed { version } => {
                            ui.label(RichText::new(format!("✔  Version {version} ist installiert.")).strong().color(Color32::from_rgb(120, 210, 150)));
                            ui.label(RichText::new("Starte PrintZilla neu, um sie zu benutzen. Deine Bibliothek bleibt dabei unverändert.").color(TEXT));
                        }
                        UpdateState::Failed(msg) => {
                            ui.label(RichText::new(msg).color(DANGER));
                        }
                    }
                });
                if self.updater.busy() {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
                }

                ui.add_space(10.0);
                egui::CollapsingHeader::new(RichText::new("Einstellungen").color(MUTED)).show(ui, |ui| {
                    save |= ui.checkbox(&mut self.cfg.check_updates, "Beim Start automatisch nach Updates suchen").changed();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Bezugsquelle (GitHub)").color(MUTED));
                        save |= ui
                            .add(egui::TextEdit::singleline(&mut self.cfg.update_repo).hint_text("Benutzer/Repository").desired_width(220.0))
                            .lost_focus();
                    });
                });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let busy = self.updater.busy();
                    match &state {
                        UpdateState::Available { version, .. } => {
                            if ui
                                .add_enabled(
                                    !busy,
                                    egui::Button::new(RichText::new("Jetzt installieren").color(Color32::WHITE).strong()).fill(ACCENT).min_size(Vec2::new(160.0, 32.0)),
                                )
                                .clicked()
                            {
                                install = Some(version.clone());
                            }
                        }
                        UpdateState::Installed { .. } => {
                            if ui
                                .add(egui::Button::new(RichText::new("Jetzt neu starten").color(Color32::WHITE).strong()).fill(ACCENT).min_size(Vec2::new(160.0, 32.0)))
                                .clicked()
                            {
                                restart = true;
                            }
                        }
                        _ => {
                            if ui.add_enabled(!busy, egui::Button::new("Erneut suchen").min_size(Vec2::new(130.0, 32.0))).clicked() {
                                check = true;
                            }
                        }
                    }
                    let installing = matches!(state, UpdateState::Installing { .. });
                    if ui.add_enabled(!installing, egui::Button::new("Schließen").min_size(Vec2::new(100.0, 32.0))).clicked() {
                        close = true;
                    }
                });
            });

        let repo = self.cfg.update_repo.clone();
        if check {
            self.updater.start_check(ctx, &repo, false);
        }
        if let Some(v) = install {
            self.updater.start_install(ctx, &repo, &v);
        }
        if save {
            self.save_cfg();
        }
        if restart {
            Updater::restart(ctx);
        }
        let installing = matches!(self.updater.state, UpdateState::Installing { .. });
        if (close || ctx.input(|i| i.key_pressed(egui::Key::Escape))) && !installing {
            self.updater.open = false;
            self.save_cfg();
        }
    }
}
