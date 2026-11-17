//! The window: state, panels and the frame loop.

use crate::library::{self, Config, Entry, Found, ImportJob, ImportMsg, ImportReport, Library};
use crate::model3d::{self, Model};
use crate::render::{self, Camera, Prepared};
use crate::widgets::*;
use eframe::egui::{self, Align2, Color32, RichText, TextureHandle, Vec2};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

#[derive(Clone, PartialEq)]
pub enum Filter {
    All,
    Untagged,
    Favorites,
    Recent,
    Type(String),
    Tag(String),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Sort {
    Newest,
    Name,
    Size,
    Type,
}

pub struct Loaded {
    pub model: Model,
    pub prepared: Option<Prepared>,
}

#[derive(Default)]
pub struct Viewer {
    pub entry_id: Option<u64>,
    pub loading: Option<Receiver<Result<Loaded, String>>>,
    pub loaded: Option<Arc<Loaded>>,
    pub error: Option<String>,
    pub cam: Camera,
    pub plate: bool,
    pub tex: Option<TextureHandle>,
    pub key: Option<(u64, [u32; 5], usize, usize, bool)>,
    pub embedded_tex: Option<TextureHandle>,
}

pub struct ImportDialog {
    pub folder: Option<PathBuf>,
    pub found: Vec<Found>,
    pub scanned: bool,
    pub skip_duplicates: bool,
    pub confirm_delete: bool,
    pub show_options: bool,
    /// Files come from the watched folders (not from one chosen folder).
    pub from_watch: bool,
    pub running: Option<Receiver<ImportMsg>>,
    pub progress: (usize, usize, String),
    pub report: Option<ImportReport>,
}

struct Screenshot {
    path: PathBuf,
    frame: u32,
    requested: bool,
    done: Arc<AtomicBool>,
}

pub struct DamApp {
    pub cfg: Config,
    pub lib: Library,
    pub persist: bool,
    pub search: String,
    pub filter: Filter,
    pub sort: Sort,
    pub selected: Option<u64>,
    pub thumbs: HashMap<u64, Option<TextureHandle>>,
    pub viewer: Viewer,
    pub import: Option<ImportDialog>,
    pub confirm_remove: Option<u64>,
    pub big_view: bool,
    pub tag_input: String,
    pub toast: Option<(String, f64)>,
    pub thumb_size: f32,
    pub history: Vec<Filter>,
    pub hist_pos: usize,
    pub side_filter: String,
    pub chip_types: Vec<String>,
    pub chip_tags: Vec<String>,
    pub chip_fav: bool,
    pub show_groups: bool,
    pub updater: crate::updater::Updater,
    card_tex: Option<Option<TextureHandle>>,
    /// Programs for "Öffnen mit" (looked up at start and when the list changes).
    pub programs: Vec<crate::externals::Program>,
    /// The symbol in the menu bar / notification area (None when switched off).
    pub menu_icon: Option<crate::tray::MenuIcon>,
    pub about: crate::about_ui::About,
    /// Background watcher for new files (None in preview pictures).
    pub watcher: Option<crate::watcher::Watcher>,
    /// The "Ordner überwachen" window is open.
    pub watch_open: bool,
    last_pending: usize,
    screenshot: Option<Screenshot>,
    /// Older entries being read again in the background (print facts, download page).
    refresh: Option<Receiver<Vec<Entry>>>,
    /// The "Umbenennen" window: entry and the name being typed.
    pub rename: Option<(u64, String)>,
}

impl DamApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        setup_style(&cc.egui_ctx);
        let persist = screenshot.is_none();
        let cfg = if persist {
            Config::load()
        } else {
            let dir = std::env::temp_dir().join(format!("printzilla-vorschau-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            Config::with_dir(dir)
        };
        let _ = std::fs::create_dir_all(&cfg.library_dir);
        let mut lib = Library::open(&cfg.library_dir);
        let mut selected = None;
        if !persist {
            crate::examples::create_examples(&mut lib);
            // preview pictures: PRINTZILLA_DEMO_SELECT=<name> picks another file than the first 3MF
            let want = std::env::var("PRINTZILLA_DEMO_SELECT").ok();
            selected = lib
                .index
                .entries
                .iter()
                .find(|e| match &want {
                    Some(w) => e.name.contains(w.as_str()),
                    None => e.ext == "3mf",
                })
                .map(|e| e.id);
        } else {
            cfg.save();
        }
        let mut app = DamApp {
            cfg,
            lib,
            persist,
            search: String::new(),
            filter: Filter::All,
            sort: Sort::Newest,
            selected,
            thumbs: HashMap::new(),
            viewer: Viewer {
                plate: true,
                ..Default::default()
            },
            import: None,
            confirm_remove: None,
            big_view: false,
            tag_input: String::new(),
            toast: None,
            thumb_size: 150.0,
            history: vec![Filter::All],
            hist_pos: 0,
            side_filter: String::new(),
            chip_types: Vec::new(),
            chip_tags: Vec::new(),
            chip_fav: false,
            show_groups: true,
            updater: Default::default(),
            card_tex: None,
            programs: Vec::new(),
            menu_icon: None,
            about: Default::default(),
            watcher: None,
            watch_open: false,
            last_pending: 0,
            refresh: None,
            rename: None,
            screenshot: screenshot.map(|path| Screenshot {
                path,
                frame: 0,
                requested: false,
                done: Arc::new(AtomicBool::new(false)),
            }),
        };
        app.programs = crate::externals::programs(&app.cfg);
        app.start_refresh(&cc.egui_ctx);
        if app.persist {
            app.watcher = Some(crate::watcher::Watcher::start(&cc.egui_ctx, &app.cfg, &app.lib));
        }
        if app.persist && app.cfg.show_menu_icon {
            app.menu_icon = crate::tray::MenuIcon::create(&cc.egui_ctx);
        }
        // look for a new version in the background (never while taking preview pictures)
        if app.persist && app.cfg.check_updates {
            let repo = app.cfg.update_repo.clone();
            app.updater.start_check(&cc.egui_ctx, &repo, true);
        }
        // preview pictures only: pretend the watched Downloads folder has new files
        if let Some(mode) = std::env::var_os("PRINTZILLA_DEMO_WATCH").filter(|_| !app.persist) {
            let demo = std::env::temp_dir().join(format!("printzilla-watchdemo-{}", std::process::id())).join("Downloads");
            let _ = std::fs::create_dir_all(&demo);
            for name in ["Schraubendose.3mf", "Halter_v2.stl", "Werkzeugwand.step"] {
                let _ = std::fs::write(demo.join(name), format!("Beispiel {name}"));
            }
            app.cfg.watch_folders = vec![crate::library::WatchFolder { path: demo.clone(), recursive: true }];
            let lib_dir = app.lib.dir.clone();
            let found = crate::library::scan_known(&demo, true, &app.cfg.import_types, &crate::library::known_set(&app.lib), &lib_dir)
                .into_iter()
                .map(|mut f| {
                    f.rel_dir = "Downloads".into();
                    f
                })
                .collect();
            app.watcher = Some(crate::watcher::Watcher::with_pending(found));
            if mode == "window" {
                app.watch_open = true;
            } else if mode == "import" {
                app.open_import_found();
            }
        }
        // preview pictures only: show the "Über PrintZilla" window
        if !app.persist && std::env::var_os("PRINTZILLA_DEMO_ABOUT").is_some() {
            app.about.open = true;
        }
        // preview pictures only: show the update window
        if !app.persist && std::env::var_os("PRINTZILLA_DEMO_UPDATE").is_some() {
            app.updater.open = true;
            app.updater.state = crate::updater::UpdateState::Available {
                version: "0.2.0".into(),
                notes: "- Beispiel: schnellere 3D-Ansicht\n- Beispiel: neue Filter".into(),
            };
        }
        // preview pictures only: open the import window on a folder of example files
        if !app.persist && std::env::var_os("PRINTZILLA_DEMO_IMPORT").is_some() {
            let demo = std::env::temp_dir().join(format!("printzilla-importdemo-{}", std::process::id()));
            crate::examples::write_example_files(&demo);
            for (sub, name) in [("Downloads", "Schraubendose.3mf"), ("Downloads", "Halter_v2.stl"), ("Deko", "Spiralvase.3mf"), ("Projekte/Werkstatt", "Werkzeughalter.step")] {
                let d = demo.join(sub);
                let _ = std::fs::create_dir_all(&d);
                let _ = std::fs::write(d.join(name), format!("Beispiel {name}"));
            }
            app.open_import(Some(demo));
        }
        app
    }

    pub fn save_cfg(&self) {
        if self.persist {
            self.cfg.save();
        }
    }

    pub fn notify(&mut self, ctx: &egui::Context, text: impl Into<String>) {
        let now = ctx.input(|i| i.time);
        self.toast = Some((text.into(), now + 4.0));
    }

    /// Switches the view and remembers it for the back/forward arrows.
    pub fn set_filter(&mut self, f: Filter) {
        self.big_view = false;
        if self.filter == f {
            return;
        }
        self.history.truncate(self.hist_pos + 1);
        self.history.push(f.clone());
        self.hist_pos = self.history.len() - 1;
        self.filter = f;
    }

    pub fn go_history(&mut self, delta: isize) {
        let n = self.hist_pos as isize + delta;
        if n >= 0 && (n as usize) < self.history.len() {
            self.hist_pos = n as usize;
            self.filter = self.history[self.hist_pos].clone();
        }
    }

    pub fn view_title(&self) -> String {
        match &self.filter {
            Filter::All => "Alle Dateien".into(),
            Filter::Untagged => "Ohne Schlagwort".into(),
            Filter::Favorites => "Favoriten".into(),
            Filter::Recent => "Zuletzt importiert".into(),
            Filter::Type(t) => type_name(t).to_string(),
            Filter::Tag(t) => t.clone(),
        }
    }

    /// Opens a library file in the program with this key (asks where it is if needed).
    pub fn open_in(&mut self, ctx: &egui::Context, key: &str, id: u64) {
        let Some(e) = self.entry(id).cloned() else { return };
        let Some(prog) = self.programs.iter().find(|p| p.key == key).cloned() else { return };
        let location = match prog.location.clone() {
            Some(l) => l,
            None => {
                let title = format!("Wo ist {} installiert?", prog.name);
                match crate::externals::pick_program(&title) {
                    Some(l) => {
                        self.cfg.program_paths.insert(prog.key.clone(), l.clone());
                        self.save_cfg();
                        self.programs = crate::externals::programs(&self.cfg);
                        l
                    }
                    None => return,
                }
            }
        };
        match crate::externals::launch(&location, &self.lib.file_path(&e)) {
            Ok(()) => self.notify(ctx, format!("»{}« wird in {} geöffnet", e.name, prog.name)),
            Err(err) => self.notify(ctx, err),
        }
    }

    /// Lets the person pick any program, adds it to "Öffnen mit" and opens the file in it.
    pub fn add_program_and_open(&mut self, ctx: &egui::Context, id: u64) {
        let Some(path) = crate::externals::pick_program("Programm zum Öffnen wählen") else { return };
        let name = crate::externals::name_from_path(&path);
        let custom = crate::externals::CustomProgram { name, path };
        if !self.cfg.custom_programs.contains(&custom) {
            self.cfg.custom_programs.push(custom.clone());
            self.save_cfg();
        }
        self.programs = crate::externals::programs(&self.cfg);
        if let Some(key) = self.programs.iter().find(|p| p.location.as_deref() == Some(custom.path.as_path())).map(|p| p.key.clone()) {
            self.open_in(ctx, &key, id);
        }
    }

    /// Opens the import window with the new files the watcher found.
    pub fn open_import_found(&mut self) {
        let found: Vec<Found> = self.watcher.as_ref().map(|w| w.pending.clone()).unwrap_or_default();
        self.import = Some(ImportDialog {
            folder: None,
            found,
            scanned: true,
            skip_duplicates: true,
            confirm_delete: false,
            show_options: false,
            from_watch: true,
            running: None,
            progress: (0, 0, String::new()),
            report: None,
        });
    }

    /// Tells the watcher about changed folders, file types or ignored files.
    pub fn watch_settings_changed(&mut self) {
        self.save_cfg();
        if let Some(w) = &self.watcher {
            w.update_settings(&self.cfg);
            w.check_now();
        }
    }

    fn handle_watcher(&mut self, ctx: &egui::Context) {
        let Some(w) = self.watcher.as_mut() else { return };
        let fresh = w.poll();
        let n = w.pending.len();
        if fresh > 0 {
            let text = if fresh == 1 {
                let name = w.pending.iter().rev().next().map(|f| f.name.clone()).unwrap_or_default();
                format!("Neue Datei gefunden: {name}")
            } else {
                format!("{fresh} neue Dateien gefunden")
            };
            self.notify(ctx, format!("{text} – links auf »Übernehmen« klicken"));
            if self.cfg.watch_notify {
                crate::watcher::system_notification("PrintZilla", &format!("{text}. Jetzt in die Bibliothek übernehmen?"));
            }
        }
        if n != self.last_pending {
            self.last_pending = n;
            if let Some(m) = &self.menu_icon {
                m.set_pending(n);
            }
            // number on the app icon (Dock / taskbar); never in preview pictures
            if self.persist {
                crate::badge::set(ctx, n);
            }
        }
    }

    /// Switches the menu bar symbol on or off and remembers the choice.
    pub fn set_menu_icon(&mut self, ctx: &egui::Context, on: bool) {
        self.cfg.show_menu_icon = on;
        self.menu_icon = if on { crate::tray::MenuIcon::create(ctx) } else { None };
        if let Some(m) = &self.menu_icon {
            m.set_pending(self.last_pending);
        }
        self.save_cfg();
    }

    fn handle_menu_icon(&mut self, ctx: &egui::Context) {
        let actions = self.menu_icon.as_ref().map(|m| m.poll()).unwrap_or_default();
        for a in actions {
            use crate::tray::TrayAction;
            if a == TrayAction::Quit {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                continue;
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            match a {
                TrayAction::Import => self.open_import(None),
                TrayAction::Updates => self.open_updates(ctx),
                TrayAction::ImportNew => self.open_import_found(),
                TrayAction::Watch => self.watch_open = true,
                _ => {}
            }
        }
    }

    /// Reads older entries again in the background (once per INFO_REV).
    fn start_refresh(&mut self, ctx: &egui::Context) {
        let old: Vec<Entry> = self.lib.index.entries.iter().filter(|e| e.info_rev < library::INFO_REV).cloned().collect();
        if old.is_empty() {
            return;
        }
        let dir = self.lib.dir.clone();
        let (tx, rx) = channel();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(library::refresh_old_entries(&dir, old));
            c.request_repaint();
        });
        self.refresh = Some(rx);
    }

    fn poll_refresh(&mut self) {
        let Some(rx) = &self.refresh else { return };
        let Ok(updated) = rx.try_recv() else { return };
        self.refresh = None;
        for u in updated {
            let mut new_thumb = false;
            if let Some(e) = self.entry_mut(u.id) {
                e.print = u.print;
                if e.thumb.is_none() && u.thumb.is_some() {
                    e.thumb = u.thumb;
                    new_thumb = true;
                }
                if e.url.is_empty() {
                    e.url = u.url;
                }
                e.info_rev = u.info_rev;
            }
            if new_thumb {
                self.thumbs.remove(&u.id);
            }
        }
        if self.persist {
            self.lib.save();
        }
    }

    /// Renames an entry (and its file) and tells the person how it went.
    pub fn apply_rename(&mut self, ctx: &egui::Context, id: u64, name: &str) {
        let before = self.entry(id).map(|e| e.file.clone());
        match self.lib.rename(id, name) {
            Ok(file) => {
                if before.as_deref() != self.entry(id).map(|e| e.file.as_str()) {
                    self.notify(ctx, format!("Umbenannt in „{file}“"));
                }
            }
            Err(err) => self.notify(ctx, err),
        }
    }

    fn rename_window(&mut self, ctx: &egui::Context) {
        let Some((id, mut name)) = self.rename.clone() else { return };
        let ext = self.entry(id).map(|e| e.ext.clone()).unwrap_or_default();
        let mut close = false;
        let mut apply = false;
        egui::Window::new("Umbenennen")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(320.0).margin(egui::Margin::symmetric(8, 6)));
                    if !r.has_focus() && !r.lost_focus() {
                        r.request_focus();
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        apply = true;
                    }
                    ui.label(RichText::new(format!(".{ext}")).color(MUTED));
                });
                ui.label(RichText::new("Die Datei in der Bibliothek bekommt denselben Namen.").size(12.0).color(MUTED));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Abbrechen").clicked() {
                        close = true;
                    }
                    if ui.add(egui::Button::new(RichText::new("Umbenennen").color(Color32::WHITE)).fill(ACCENT)).clicked() {
                        apply = true;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if apply {
            self.apply_rename(ctx, id, &name);
            close = true;
        }
        self.rename = if close { None } else { Some((id, name)) };
    }

    pub fn entry(&self, id: u64) -> Option<&Entry> {
        self.lib.index.entries.iter().find(|e| e.id == id)
    }

    pub fn entry_mut(&mut self, id: u64) -> Option<&mut Entry> {
        self.lib.index.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn visible_entries(&self) -> Vec<Entry> {
        let q = self.search.trim().to_lowercase();
        let now = chrono::Local::now().timestamp();
        let mut v: Vec<Entry> = self
            .lib
            .index
            .entries
            .iter()
            .filter(|e| match &self.filter {
                Filter::All => true,
                Filter::Untagged => e.tags.is_empty(),
                Filter::Favorites => e.favorite,
                Filter::Recent => now - e.imported < 7 * 24 * 3600,
                Filter::Type(t) => &e.ext == t,
                Filter::Tag(t) => e.tags.contains(t),
            })
            .filter(|e| self.chip_types.is_empty() || self.chip_types.contains(&e.ext))
            .filter(|e| self.chip_tags.is_empty() || self.chip_tags.iter().any(|t| e.tags.contains(t)))
            .filter(|e| !self.chip_fav || e.favorite)
            .filter(|e| {
                q.is_empty()
                    || e.name.to_lowercase().contains(&q)
                    || e.file.to_lowercase().contains(&q)
                    || e.notes.to_lowercase().contains(&q)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&q))
                    || e.url.to_lowercase().contains(&q)
                    || e.print.as_ref().map(|p| p.materials.iter().any(|m| m.to_lowercase() == q)).unwrap_or(false)
            })
            .cloned()
            .collect();
        match self.sort {
            Sort::Newest => v.sort_by(|a, b| b.imported.cmp(&a.imported).then(b.id.cmp(&a.id))),
            Sort::Name => v.sort_by_key(|a| a.name.to_lowercase()),
            Sort::Size => v.sort_by(|a, b| b.size.cmp(&a.size)),
            Sort::Type => v.sort_by(|a, b| a.ext.cmp(&b.ext).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))),
        }
        v
    }

    pub fn thumb(&mut self, ctx: &egui::Context, e: &Entry, budget: &mut usize) -> Option<TextureHandle> {
        if let Some(t) = self.thumbs.get(&e.id) {
            return t.clone();
        }
        if *budget == 0 {
            ctx.request_repaint();
            return None;
        }
        *budget -= 1;
        let tex = self.lib.thumb_path(e).and_then(|p| image::open(p).ok()).map(|img| {
            let img = img.to_rgba8();
            let (w, h) = img.dimensions();
            let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
            ctx.load_texture(format!("thumb-{}", e.id), ci, egui::TextureOptions::LINEAR)
        });
        self.thumbs.insert(e.id, tex.clone());
        tex
    }

    pub fn open_import(&mut self, folder: Option<PathBuf>) {
        let mut d = ImportDialog {
            folder: folder.or_else(|| self.cfg.last_import_dir.clone()),
            found: Vec::new(),
            scanned: false,
            skip_duplicates: true,
            confirm_delete: false,
            show_options: false,
            from_watch: false,
            running: None,
            progress: (0, 0, String::new()),
            report: None,
        };
        match d.folder.clone() {
            Some(f) if f.is_dir() => {
                d.found = library::scan(&f, self.cfg.recursive, &self.cfg.import_types, &self.lib);
                d.scanned = true;
            }
            _ => d.folder = None,
        }
        self.import = Some(d);
    }

    pub fn start_import(&mut self, ctx: &egui::Context) {
        let Some(d) = self.import.as_mut() else { return };
        let files: Vec<Found> = d.found.iter().filter(|f| f.selected).cloned().collect();
        if files.is_empty() {
            return;
        }
        let job = ImportJob {
            files,
            library_dir: self.lib.dir.clone(),
            known_hashes: self.lib.hashes(),
            first_id: self.lib.index.next_id.max(1),
            after: self.cfg.after_import,
            skip_duplicates: d.skip_duplicates,
            folder_as_tag: self.cfg.folder_as_tag,
        };
        let (tx, rx) = channel();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let tx2 = tx.clone();
            let c3 = ctx2.clone();
            let report = library::run_import(job, move |m| {
                let _ = tx2.send(m);
                c3.request_repaint();
            });
            let _ = tx.send(ImportMsg::Finished(report));
            ctx2.request_repaint();
        });
        d.running = Some(rx);
        d.confirm_delete = false;
        self.save_cfg();
    }

    fn poll_import(&mut self, ctx: &egui::Context) {
        let mut finished = None;
        if let Some(d) = self.import.as_mut() {
            if let Some(rx) = &d.running {
                while let Ok(m) = rx.try_recv() {
                    match m {
                        ImportMsg::Progress { done, total, current } => d.progress = (done, total, current),
                        ImportMsg::Finished(r) => finished = Some(r),
                    }
                }
            }
        }
        if let Some(report) = finished {
            let n = report.entries.len();
            let first = report.entries.first().map(|e| e.id);
            self.lib.add_entries(report.entries.clone());
            if let Some(w) = &self.watcher {
                w.update_known(&self.lib);
            }
            if let Some(d) = self.import.as_mut() {
                d.running = None;
                d.report = Some(report);
            }
            if let Some(id) = first {
                self.selected = Some(id);
            }
            self.notify(ctx, format!("{n} Dateien in die Bibliothek übernommen"));
        }
    }

    fn ensure_viewer(&mut self, ctx: &egui::Context) {
        let want = self.selected;
        if self.viewer.entry_id != want {
            self.viewer.entry_id = want;
            self.viewer.loaded = None;
            self.viewer.error = None;
            self.viewer.tex = None;
            self.viewer.key = None;
            self.viewer.embedded_tex = None;
            self.viewer.cam = Camera::default();
            self.viewer.loading = None;
            if let Some(e) = want.and_then(|id| self.entry(id)).cloned() {
                if model3d::can_preview(&e.ext) {
                    let path = self.lib.file_path(&e);
                    let (tx, rx) = channel();
                    let c = ctx.clone();
                    std::thread::spawn(move || {
                        let res = model3d::load(&path).map(|model| {
                            let prepared = (!model.mesh.tris.is_empty()).then(|| render::prepare(model.mesh.clone()));
                            Loaded { model, prepared }
                        });
                        let _ = tx.send(res);
                        c.request_repaint();
                    });
                    self.viewer.loading = Some(rx);
                }
            }
        }
        let mut result = None;
        if let Some(rx) = &self.viewer.loading {
            if let Ok(res) = rx.try_recv() {
                result = Some(res);
            }
        }
        if let Some(res) = result {
            self.viewer.loading = None;
            match res {
                Ok(l) => {
                    if let Some((rgba, w, h)) = &l.model.thumbnail {
                        let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
                        self.viewer.embedded_tex = Some(ctx.load_texture("embedded", ci, egui::TextureOptions::LINEAR));
                    }
                    self.viewer.loaded = Some(Arc::new(l));
                }
                Err(e) => self.viewer.error = Some(e),
            }
        }
    }

    fn remove_dialog(&mut self, ctx: &egui::Context) {
        let Some(id) = self.confirm_remove else { return };
        let name = self.entry(id).map(|e| e.name.clone()).unwrap_or_default();
        let mut close = false;
        let mut remove = false;
        egui::Window::new("Aus der Bibliothek entfernen")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("»{name}« aus der Bibliothek entfernen?"));
                ui.label(RichText::new("Die Kopie in der Bibliothek wandert in den Papierkorb.").color(MUTED));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new(RichText::new("Entfernen").color(Color32::WHITE)).fill(Color32::from_rgb(180, 60, 50)))
                        .clicked()
                    {
                        remove = true;
                    }
                    if ui.button("Abbrechen").clicked() {
                        close = true;
                    }
                });
            });
        if remove {
            match self.lib.remove(id) {
                Ok(()) => {
                    self.thumbs.remove(&id);
                    if self.selected == Some(id) {
                        self.selected = None;
                        self.big_view = false;
                    }
                    self.notify(ctx, format!("»{name}« entfernt"));
                }
                Err(e) => self.notify(ctx, e),
            }
            close = true;
        }
        if close {
            self.confirm_remove = None;
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(s) = self.screenshot.as_mut() else { return };
        ctx.request_repaint();
        s.frame += 1;
        if s.done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if s.frame >= 120 && !s.requested {
            s.requested = true;
            let path = s.path.clone();
            let done = s.done.clone();
            let c = ctx.clone();
            ctx.request_screenshot(move |img| {
                let [w, h] = img.size;
                let mut buf = Vec::with_capacity(w * h * 4);
                for p in &img.pixels {
                    buf.extend_from_slice(&p.to_array());
                }
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                // the GitHub card must be exactly 1280 × 640
                let (buf, w, h) = if crate::card::enabled() {
                    let src = image::RgbaImage::from_raw(w as u32, h as u32, buf).expect("Bildgröße");
                    let out = image::imageops::resize(&src, crate::card::SIZE[0] as u32, crate::card::SIZE[1] as u32, image::imageops::FilterType::Lanczos3);
                    (out.into_raw(), crate::card::SIZE[0] as usize, crate::card::SIZE[1] as usize)
                } else {
                    (buf, w, h)
                };
                if let Err(e) = image::save_buffer(&path, &buf, w as u32, h as u32, image::ExtendedColorType::Rgba8) {
                    eprintln!("Bild konnte nicht gespeichert werden: {e}");
                }
                done.store(true, Ordering::SeqCst);
                c.request_repaint();
            });
        }
        if s.frame > 1500 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl Drop for DamApp {
    fn drop(&mut self) {
        if !self.persist {
            let _ = std::fs::remove_dir_all(&self.lib.dir);
        }
    }
}

impl eframe::App for DamApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_screenshot(&ctx);
        if crate::card::enabled() {
            // picture for the GitHub page instead of the normal window
            let tex = self.card_tex.get_or_insert_with(|| {
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join("screenshot.png");
                image::open(path).ok().map(|img| {
                    let img = img.to_rgba8();
                    let (w, h) = img.dimensions();
                    let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
                    ctx.load_texture("card-shot", ci, egui::TextureOptions::LINEAR)
                })
            });
            let tex = tex.clone();
            egui::CentralPanel::no_frame().show(ui, |ui| crate::card::draw(ui, tex.as_ref()));
            return;
        }
        self.poll_import(&ctx);
        self.poll_refresh();
        self.ensure_viewer(&ctx);
        // F2 renames the selected file (as in Windows Explorer)
        if self.rename.is_none() && !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F2)) {
            if let Some(e) = self.selected.and_then(|id| self.entry(id)) {
                self.rename = Some((e.id, e.name.clone()));
            }
        }
        if self.import.as_ref().map(|d| d.running.is_some()).unwrap_or(false) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // a folder dropped onto the window opens the import
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| { let p = f.path(); (!p.as_os_str().is_empty()).then(|| p.to_path_buf()) }).collect());
        if let Some(p) = dropped.into_iter().next() {
            if self.import.as_ref().map(|d| d.running.is_none()).unwrap_or(true) {
                let dir = if p.is_dir() { Some(p) } else { p.parent().map(|x| x.to_path_buf()) };
                self.open_import(dir);
            }
        }

        // full-height sidebar | toolbar + content | inspector
        egui::Panel::left("left")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(10, 10)))
            .resizable(true)
            .default_size(250.0)
            .show(ui, |ui| self.side_bar(ui));
        egui::Panel::right("right")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::same(14)))
            .resizable(true)
            .default_size(330.0)
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin { left: 18, right: 18, top: 10, bottom: 0 }))
            .show(ui, |ui| {
                self.toolbar(ui);
                if self.big_view {
                    self.big_view_ui(ui);
                } else {
                    self.chip_row(ui);
                    self.grid(ui);
                }
            });

        self.updater.poll();
        self.handle_watcher(&ctx);
        self.handle_menu_icon(&ctx);
        self.import_window(&ctx);
        self.remove_dialog(&ctx);
        self.rename_window(&ctx);
        self.update_window(&ctx);
        self.about_window(&ctx);
        self.watch_window(&ctx);

        if let Some((text, until)) = self.toast.clone() {
            let now = ctx.input(|i| i.time);
            if now > until {
                self.toast = None;
            } else {
                egui::Area::new(egui::Id::unique("toast"))
                    .anchor(Align2::CENTER_BOTTOM, Vec2::new(0.0, -20.0))
                    .show(&ctx, |ui| {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(55, 60, 72))
                            .corner_radius(8.0)
                            .inner_margin(egui::Margin::symmetric(14, 8))
                            .show(ui, |ui| ui.label(RichText::new(text).color(Color32::WHITE)));
                    });
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
        }
    }
}
