//! "Über PrintZilla": version, copyright, PrintZilla's licence and the licences of
//! all libraries and fonts built into the program.

use crate::app::DamApp;
use crate::updater::CURRENT_VERSION;
use crate::widgets::*;
use eframe::egui::{self, Color32, FontId, RichText, Sense, Stroke, Vec2};

pub const LICENSE_TEXT: &str = include_str!("../LICENSE");
pub const THIRD_PARTY_TEXT: &str = include_str!("../THIRD_PARTY_LICENSES.txt");
pub const REPO_URL: &str = "https://github.com/Marwaeck/printzilla";

#[derive(Default, Clone, Copy, PartialEq)]
pub enum AboutPage {
    #[default]
    Overview,
    License,
    ThirdParty,
}

#[derive(Default)]
pub struct About {
    pub open: bool,
    pub page: AboutPage,
    lines: Vec<&'static str>,
}

impl DamApp {
    pub fn about_window(&mut self, ctx: &egui::Context) {
        if !self.about.open {
            return;
        }
        if self.about.lines.is_empty() {
            self.about.lines = THIRD_PARTY_TEXT.lines().collect();
        }
        let mut close = false;
        let screen = ctx.content_rect();
        egui::Modal::new(egui::Id::unique("about-dialog"))
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .corner_radius(14.0)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(70, 70, 76)))
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width((screen.width() - 160.0).clamp(420.0, 720.0));
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                    draw_logo(ui.painter(), r);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("PrintZilla").size(22.0).strong().color(TEXT));
                        ui.label(RichText::new(format!("Version {CURRENT_VERSION}")).color(MUTED));
                        ui.label(RichText::new("© 2026 Marwaeck · MIT-Lizenz").color(MUTED));
                    });
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    let p = &mut self.about.page;
                    ui.selectable_value(p, AboutPage::Overview, "Über");
                    ui.selectable_value(p, AboutPage::License, "Lizenz");
                    ui.selectable_value(p, AboutPage::ThirdParty, "Lizenzen von Drittanbietern");
                });
                ui.separator();
                let h = (screen.height() - 330.0).clamp(160.0, 420.0);
                match self.about.page {
                    AboutPage::Overview => {
                        egui::ScrollArea::vertical().max_height(h).show(ui, |ui| {
                            ui.label(RichText::new("Deine Bibliothek für 3D-Druck-Dateien.").size(14.0).color(TEXT));
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(
                                    "PrintZilla ist freie Software unter der MIT-Lizenz: Du darfst das Programm benutzen, \
                                     weitergeben und verändern, solange der Urheberrechtsvermerk erhalten bleibt. \
                                     Es wird ohne Gewährleistung bereitgestellt.",
                                )
                                .color(TEXT),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(format!(
                                    "PrintZilla enthält {} freie Bibliotheken und Schriften. Ihre Lizenzen und \
                                     Urheberrechtsvermerke stehen unter »Lizenzen von Drittanbietern«.",
                                    THIRD_PARTY_TEXT.lines().filter(|l| l.starts_with("- ")).count()
                                ))
                                .color(TEXT),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(
                                    "SolidCraft, Bambu Studio, OrcaSlicer, PrusaSlicer und UltiMaker Cura sind Marken \
                                     ihrer jeweiligen Inhaber. PrintZilla steht in keiner Verbindung zu ihnen und öffnet \
                                     Dateien nur in diesen Programmen, wenn sie installiert sind.",
                                )
                                .size(12.5)
                                .color(MUTED),
                            );
                            ui.add_space(8.0);
                            ui.hyperlink_to("Projektseite auf GitHub", REPO_URL);
                        });
                    }
                    AboutPage::License => {
                        egui::ScrollArea::vertical().max_height(h).auto_shrink([false, true]).show(ui, |ui| {
                            ui.label(RichText::new(LICENSE_TEXT).font(FontId::monospace(11.5)).color(TEXT));
                        });
                    }
                    AboutPage::ThirdParty => {
                        let row_h = 15.0;
                        let lines = &self.about.lines;
                        egui::ScrollArea::vertical().max_height(h).auto_shrink([false, true]).show_rows(ui, row_h, lines.len(), |ui, range| {
                            for l in &lines[range] {
                                ui.add(egui::Label::new(RichText::new(*l).font(FontId::monospace(11.0)).color(TEXT)).truncate());
                            }
                        });
                    }
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Schließen").min_size(Vec2::new(100.0, 30.0))).clicked() {
                        close = true;
                    }
                });
            });
        if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.about.open = false;
        }
    }
}
