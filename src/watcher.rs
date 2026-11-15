//! Keeps an eye on chosen folders (e.g. Downloads) and collects new 3D print files.
//!
//! A background thread looks through the folders every few seconds. A file only counts
//! once it is complete: it must have the same size and date on two checks in a row and
//! must not have changed for a few seconds (so half-finished downloads are not reported).
//! Nothing is copied here – the person decides in the import window.

use crate::library::{self, Config, Found, IgnoredFile, Library, WatchFolder};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, SystemTime};

pub const INTERVAL: Duration = Duration::from_secs(10);
const SETTLE: Duration = Duration::from_secs(4);

enum Cmd {
    Settings { folders: Vec<WatchFolder>, types: Vec<String>, enabled: bool },
    Known(HashSet<(String, u64)>, PathBuf),
    Ignored(HashSet<IgnoredFile>),
    CheckNow,
}

struct Report {
    pending: Vec<Found>,
    checked_at: i64,
}

pub struct Watcher {
    tx: Sender<Cmd>,
    rx: Receiver<Report>,
    /// Complete new files waiting to be transferred.
    pub pending: Vec<Found>,
    pub last_check: Option<i64>,
    /// Files already announced (so each file is announced once).
    announced: HashSet<PathBuf>,
}

fn ignored_set(cfg: &Config) -> HashSet<IgnoredFile> {
    cfg.watch_ignored.iter().cloned().collect()
}

impl Watcher {
    pub fn start(ctx: &eframe::egui::Context, cfg: &Config, lib: &Library) -> Watcher {
        let (tx, cmd_rx) = channel::<Cmd>();
        let (rep_tx, rx) = channel::<Report>();
        let c = ctx.clone();
        std::thread::spawn(move || {
            let mut folders: Vec<WatchFolder> = Vec::new();
            let mut types: Vec<String> = Vec::new();
            let mut enabled = false;
            let mut known: HashSet<(String, u64)> = HashSet::new();
            let mut lib_dir = PathBuf::new();
            let mut ignored: HashSet<IgnoredFile> = HashSet::new();
            let mut seen: HashMap<PathBuf, (u64, Option<SystemTime>)> = HashMap::new();
            loop {
                match cmd_rx.recv_timeout(INTERVAL) {
                    Ok(cmd) => {
                        match cmd {
                            Cmd::Settings { folders: f, types: t, enabled: e } => {
                                folders = f;
                                types = t;
                                enabled = e;
                            }
                            Cmd::Known(k, d) => {
                                known = k;
                                lib_dir = d;
                            }
                            Cmd::Ignored(i) => ignored = i,
                            Cmd::CheckNow => {}
                        }
                        // take several quick updates together before looking
                        while let Ok(more) = cmd_rx.try_recv() {
                            match more {
                                Cmd::Settings { folders: f, types: t, enabled: e } => {
                                    folders = f;
                                    types = t;
                                    enabled = e;
                                }
                                Cmd::Known(k, d) => {
                                    known = k;
                                    lib_dir = d;
                                }
                                Cmd::Ignored(i) => ignored = i,
                                Cmd::CheckNow => {}
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }

                let mut pending = Vec::new();
                let mut now_seen = HashMap::new();
                if enabled {
                    for wf in &folders {
                        if !wf.path.is_dir() {
                            continue;
                        }
                        let label = wf.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| wf.path.to_string_lossy().to_string());
                        for mut f in library::scan_known(&wf.path, wf.recursive, &types, &known, &lib_dir) {
                            if f.maybe_known {
                                continue;
                            }
                            let key = IgnoredFile { path: f.path.to_string_lossy().to_string(), size: f.size };
                            if ignored.contains(&key) {
                                continue;
                            }
                            let modified = std::fs::metadata(&f.path).and_then(|m| m.modified()).ok();
                            let state = (f.size, modified);
                            let settled = modified.map(|m| m.elapsed().map(|e| e >= SETTLE).unwrap_or(true)).unwrap_or(true);
                            let unchanged = seen.get(&f.path) == Some(&state);
                            now_seen.insert(f.path.clone(), state);
                            if unchanged && settled && f.size > 0 {
                                f.rel_dir = if f.rel_dir.is_empty() { label.clone() } else { format!("{label}/{}", f.rel_dir) };
                                f.selected = true;
                                pending.push(f);
                            }
                        }
                    }
                }
                seen = now_seen;
                if rep_tx.send(Report { pending, checked_at: chrono::Local::now().timestamp() }).is_err() {
                    return;
                }
                c.request_repaint();
            }
        });
        let w = Watcher {
            tx,
            rx,
            pending: Vec::new(),
            last_check: None,
            announced: HashSet::new(),
        };
        w.update_known(lib);
        w.update_settings(cfg);
        w
    }

    /// A watcher without a background thread that just holds the given files (preview pictures).
    pub fn with_pending(pending: Vec<Found>) -> Watcher {
        let (tx, _) = channel();
        let (_, rx) = channel();
        Watcher {
            tx,
            rx,
            pending,
            last_check: Some(chrono::Local::now().timestamp()),
            announced: HashSet::new(),
        }
    }

    pub fn update_settings(&self, cfg: &Config) {
        let _ = self.tx.send(Cmd::Ignored(ignored_set(cfg)));
        let _ = self.tx.send(Cmd::Settings {
            folders: cfg.watch_folders.clone(),
            types: cfg.import_types.clone(),
            enabled: cfg.watch_enabled,
        });
    }

    /// Call after imports, so files now in the library are no longer reported.
    pub fn update_known(&self, lib: &Library) {
        let dir = lib.dir.canonicalize().unwrap_or(lib.dir.clone());
        let _ = self.tx.send(Cmd::Known(library::known_set(lib), dir));
    }

    pub fn check_now(&self) {
        let _ = self.tx.send(Cmd::CheckNow);
    }

    /// Takes the newest result. Returns how many files turned up that were not announced yet.
    pub fn poll(&mut self) -> usize {
        let mut fresh = 0;
        while let Ok(r) = self.rx.try_recv() {
            self.last_check = Some(r.checked_at);
            for f in &r.pending {
                if self.announced.insert(f.path.clone()) {
                    fresh += 1;
                }
            }
            self.pending = r.pending;
        }
        fresh
    }
}

/// The person's Downloads folder, if there is one.
pub fn downloads_folder() -> Option<PathBuf> {
    dirs::download_dir().filter(|d| d.is_dir())
}

/// A system message (macOS Mitteilungen / Windows notification). Errors are ignored:
/// the hint in PrintZilla and on the menu bar symbol always works.
pub fn system_notification(title: &str, body: &str) {
    #[cfg(any(windows, target_os = "macos"))]
    {
        let (title, body) = (title.to_string(), body.to_string());
        std::thread::spawn(move || {
            let _ = notify_rust::Notification::new().appname("PrintZilla").summary(&title).body(&body).show();
        });
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (title, body);
    }
}
