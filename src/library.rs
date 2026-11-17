//! The library: where collected files live, the index of what's in it, and the import.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const ALL_TYPES: &[(&str, &str)] = &[
    ("3mf", "3MF-Projekte"),
    ("stl", "STL-Modelle"),
    ("obj", "OBJ-Modelle"),
    ("step", "STEP-Dateien"),
    ("stp", "STP-Dateien"),
    ("gcode", "G-Code"),
    ("bgcode", "Binärer G-Code"),
    ("scad", "OpenSCAD"),
    ("f3d", "Fusion-Dateien"),
    ("blend", "Blender-Dateien"),
];

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ModelSummary {
    pub objects: usize,
    pub triangles: usize,
    pub size_mm: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Entry {
    pub id: u64,
    pub name: String,
    /// Path inside the library folder.
    pub file: String,
    pub ext: String,
    pub size: u64,
    pub hash: String,
    pub imported: i64,
    pub source: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub thumb: Option<String>,
    #[serde(default)]
    pub summary: Option<ModelSummary>,
    /// What the slicer wrote into the file (print time, filament, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print: Option<crate::printinfo::PrintInfo>,
    /// Web page the file came from (MakerWorld, Printables, …).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    /// Which version of the file reading filled this entry (see INFO_REV).
    #[serde(default)]
    pub info_rev: u32,
}

/// Raised whenever PrintZilla learns to read more from files; older entries are read again.
pub const INFO_REV: u32 = 1;

#[derive(Serialize, Deserialize, Default)]
pub struct Index {
    pub next_id: u64,
    pub entries: Vec<Entry>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub library_dir: PathBuf,
    #[serde(default)]
    pub last_import_dir: Option<PathBuf>,
    #[serde(default = "default_types")]
    pub import_types: Vec<String>,
    #[serde(default = "yes")]
    pub recursive: bool,
    #[serde(default)]
    pub after_import: AfterImport,
    #[serde(default = "yes")]
    pub folder_as_tag: bool,
    /// Where new versions are published (GitHub "user/repository").
    #[serde(default = "default_repo")]
    pub update_repo: String,
    /// Look for a new version when the program starts.
    #[serde(default = "yes")]
    pub check_updates: bool,
    /// Where programs were found when PrintZilla had to ask (key -> location).
    #[serde(default)]
    pub program_paths: std::collections::HashMap<String, PathBuf>,
    /// Programs the person added for "Öffnen mit".
    #[serde(default)]
    pub custom_programs: Vec<crate::externals::CustomProgram>,
    /// Show the PrintZilla symbol in the menu bar (macOS) / notification area (Windows).
    #[serde(default = "yes")]
    pub show_menu_icon: bool,
    /// Folders PrintZilla keeps an eye on for new print files.
    #[serde(default)]
    pub watch_folders: Vec<WatchFolder>,
    #[serde(default = "yes")]
    pub watch_enabled: bool,
    /// Also show a system message when new files turn up.
    #[serde(default = "yes")]
    pub watch_notify: bool,
    /// Found files the person chose not to be told about again.
    #[serde(default)]
    pub watch_ignored: Vec<IgnoredFile>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WatchFolder {
    pub path: PathBuf,
    #[serde(default = "yes")]
    pub recursive: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct IgnoredFile {
    pub path: String,
    pub size: u64,
}

fn default_repo() -> String {
    crate::updater::DEFAULT_REPO.to_string()
}

fn yes() -> bool {
    true
}

fn default_types() -> Vec<String> {
    ["3mf", "stl", "obj", "step", "stp", "gcode", "bgcode"].iter().map(|s| s.to_string()).collect()
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum AfterImport {
    #[default]
    Keep,
    Trash,
    Delete,
}

impl Config {
    fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("Druckarchiv")
            .join("einstellungen.json")
    }

    pub fn load() -> Config {
        if let Ok(text) = std::fs::read_to_string(Self::path()) {
            if let Ok(c) = serde_json::from_str::<Config>(&text) {
                return c;
            }
        }
        let base = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(std::env::temp_dir);
        Config::with_dir(base.join("3D-Druck-Bibliothek"))
    }

    pub fn with_dir(dir: PathBuf) -> Config {
        Config {
            library_dir: dir,
            last_import_dir: None,
            import_types: default_types(),
            recursive: true,
            after_import: AfterImport::Keep,
            folder_as_tag: true,
            update_repo: default_repo(),
            check_updates: true,
            program_paths: Default::default(),
            custom_programs: Vec::new(),
            show_menu_icon: true,
            watch_folders: Vec::new(),
            watch_enabled: true,
            watch_notify: true,
            watch_ignored: Vec::new(),
        }
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, text);
        }
    }
}

pub struct Library {
    pub dir: PathBuf,
    pub index: Index,
}

impl Library {
    pub fn meta_dir(dir: &Path) -> PathBuf {
        dir.join(".druckarchiv")
    }
    pub fn thumbs_dir(dir: &Path) -> PathBuf {
        Self::meta_dir(dir).join("vorschau")
    }

    pub fn open(dir: &Path) -> Library {
        let _ = std::fs::create_dir_all(Self::thumbs_dir(dir));
        let index = std::fs::read_to_string(Self::meta_dir(dir).join("bibliothek.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Library {
            dir: dir.to_path_buf(),
            index,
        }
    }

    pub fn save(&self) {
        let p = Self::meta_dir(&self.dir).join("bibliothek.json");
        if let Ok(text) = serde_json::to_string_pretty(&self.index) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(tmp, p);
            }
        }
    }

    pub fn file_path(&self, e: &Entry) -> PathBuf {
        self.dir.join(&e.file)
    }

    pub fn thumb_path(&self, e: &Entry) -> Option<PathBuf> {
        e.thumb.as_ref().map(|t| Self::thumbs_dir(&self.dir).join(t))
    }

    pub fn hashes(&self) -> HashSet<String> {
        self.index.entries.iter().map(|e| e.hash.clone()).collect()
    }

    pub fn add_entries(&mut self, mut new: Vec<Entry>) {
        for e in &mut new {
            self.index.next_id = self.index.next_id.max(e.id + 1);
        }
        self.index.entries.extend(new);
        self.save();
    }

    /// Removes an entry; the library copy goes to the trash.
    pub fn remove(&mut self, id: u64) -> Result<(), String> {
        let Some(pos) = self.index.entries.iter().position(|e| e.id == id) else {
            return Ok(());
        };
        let e = self.index.entries[pos].clone();
        let p = self.file_path(&e);
        if p.exists() {
            if trash::delete(&p).is_err() {
                std::fs::remove_file(&p).map_err(|err| format!("Datei konnte nicht entfernt werden: {err}"))?;
            }
        }
        if let Some(t) = self.thumb_path(&e) {
            let _ = std::fs::remove_file(t);
        }
        self.index.entries.remove(pos);
        self.save();
        Ok(())
    }

    /// Gives an entry a new name and renames its file in the library to match.
    /// Returns the new file name.
    pub fn rename(&mut self, id: u64, new_name: &str) -> Result<String, String> {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err("Der Name darf nicht leer sein.".into());
        }
        let Some(pos) = self.index.entries.iter().position(|e| e.id == id) else {
            return Err("Datei nicht gefunden.".into());
        };
        let e = self.index.entries[pos].clone();
        let stem = safe_file_name(new_name);
        if stem.is_empty() {
            return Err("Dieser Name eignet sich nicht als Dateiname.".into());
        }
        let old_path = self.file_path(&e);
        let dir = old_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.dir.clone());
        let ext = Path::new(&e.file).extension().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| e.ext.clone());
        let fname = if ext.is_empty() { stem.clone() } else { format!("{stem}.{ext}") };
        let current = old_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut new_file = e.file.clone();
        if fname != current {
            // only the spelling changes ("halter" → "Halter"): same file on macOS/Windows
            let target = if fname.to_lowercase() == current.to_lowercase() { dir.join(&fname) } else { unique_target(&dir, &fname) };
            if old_path.exists() {
                std::fs::rename(&old_path, &target).map_err(|err| format!("Umbenennen fehlgeschlagen: {err}"))?;
            }
            new_file = target
                .strip_prefix(&self.dir)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| fname.clone());
        }
        let m = &mut self.index.entries[pos];
        m.name = new_name.to_string();
        m.file = new_file.clone();
        self.save();
        Ok(Path::new(&new_file).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or(new_file))
    }

    pub fn all_tags(&self) -> Vec<(String, usize)> {
        let mut map: std::collections::BTreeMap<String, usize> = Default::default();
        for e in &self.index.entries {
            for t in &e.tags {
                *map.entry(t.clone()).or_default() += 1;
            }
        }
        map.into_iter().collect()
    }
}

// ---------------------------------------------------------------------------
// Scanning a folder

#[derive(Clone)]
pub struct Found {
    pub path: PathBuf,
    pub rel_dir: String,
    pub name: String,
    pub ext: String,
    pub size: u64,
    pub selected: bool,
    /// Same name and size already in the library.
    pub maybe_known: bool,
}

/// (file name in lower case, size) of everything already in the library.
pub fn known_set(lib: &Library) -> HashSet<(String, u64)> {
    lib.index
        .entries
        .iter()
        .map(|e| {
            let fname = Path::new(&e.source)
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            (fname, e.size)
        })
        .collect()
}

pub fn scan(dir: &Path, recursive: bool, types: &[String], lib: &Library) -> Vec<Found> {
    let lib_dir = lib.dir.canonicalize().unwrap_or(lib.dir.clone());
    scan_known(dir, recursive, types, &known_set(lib), &lib_dir)
}

/// Like `scan`, but without the library at hand (used by the folder watcher in the background).
pub fn scan_known(dir: &Path, recursive: bool, types: &[String], known: &HashSet<(String, u64)>, lib_dir: &Path) -> Vec<Found> {
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(dir)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .follow_links(false);
    for item in walker.into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        !(e.depth() > 0 && n.starts_with('.'))
    }) {
        let Ok(item) = item else { continue };
        if !item.file_type().is_file() {
            continue;
        }
        let p = item.path();
        // never import from the library itself
        if p.canonicalize().map(|c| c.starts_with(lib_dir)).unwrap_or(false) {
            continue;
        }
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !types.iter().any(|t| *t == ext) {
            continue;
        }
        let size = item.metadata().map(|m| m.len()).unwrap_or(0);
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let rel_dir = p
            .parent()
            .and_then(|par| par.strip_prefix(dir).ok())
            .map(|r| r.to_string_lossy().to_string())
            .unwrap_or_default();
        let maybe_known = known.contains(&(name.to_lowercase(), size));
        out.push(Found {
            path: p.to_path_buf(),
            rel_dir,
            name,
            ext,
            size,
            selected: !maybe_known,
            maybe_known,
        });
    }
    out.sort_by(|a, b| a.rel_dir.cmp(&b.rel_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    out
}

// ---------------------------------------------------------------------------
// Importing

pub enum ImportMsg {
    Progress { done: usize, total: usize, current: String },
    Finished(ImportReport),
}

#[derive(Default, Clone)]
pub struct ImportReport {
    pub entries: Vec<Entry>,
    pub duplicates: usize,
    pub deleted: usize,
    pub errors: Vec<String>,
}

pub struct ImportJob {
    pub files: Vec<Found>,
    pub library_dir: PathBuf,
    pub known_hashes: HashSet<String>,
    pub first_id: u64,
    pub after: AfterImport,
    pub skip_duplicates: bool,
    pub folder_as_tag: bool,
}

fn hash_file(p: &Path) -> std::io::Result<String> {
    // FNV-1a 64 + length: quick and good enough to find identical files
    let mut f = std::fs::File::open(p)?;
    let mut h: u64 = 0xcbf29ce484222325;
    let mut h2: u64 = 0x84222325cbf29ce4;
    let mut buf = vec![0u8; 1 << 16];
    let mut len = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        len += n as u64;
        for &b in &buf[..n] {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
            h2 = h2.rotate_left(5) ^ (b as u64);
            h2 = h2.wrapping_mul(0x9E3779B97F4A7C15);
        }
    }
    Ok(format!("{h:016x}{h2:016x}-{len}"))
}

fn unique_target(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let stem = Path::new(name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = Path::new(name).extension().map(|s| s.to_string_lossy().to_string());
    for i in 2.. {
        let n = match &ext {
            Some(e) => format!("{stem} ({i}).{e}"),
            None => format!("{stem} ({i})"),
        };
        let p = dir.join(n);
        if !p.exists() {
            return p;
        }
    }
    unreachable!()
}

/// What make_preview found out about a file.
#[derive(Default)]
pub struct Preview {
    pub thumb: Option<String>,
    pub summary: Option<ModelSummary>,
    pub print: Option<crate::printinfo::PrintInfo>,
}

fn save_thumb(lib_dir: &Path, id: u64, rgba: &[u8], w: u32, h: u32) -> Option<String> {
    let thumb_name = format!("{id}.png");
    let _ = std::fs::create_dir_all(Library::thumbs_dir(lib_dir));
    let thumb_file = Library::thumbs_dir(lib_dir).join(&thumb_name);
    image::save_buffer(&thumb_file, rgba, w, h, image::ExtendedColorType::Rgba8).is_ok().then_some(thumb_name)
}

/// Creates the preview picture, model summary and print facts for a library file.
pub fn make_preview(lib_dir: &Path, file: &Path, id: u64) -> Preview {
    let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "gcode" | "bgcode") {
        let Ok((info, thumb)) = crate::printinfo::load_gcode(file) else {
            return Preview::default();
        };
        return Preview {
            thumb: thumb.and_then(|(rgba, w, h)| save_thumb(lib_dir, id, &rgba, w, h)),
            summary: None,
            print: (!info.is_empty()).then_some(info),
        };
    }
    if !crate::model3d::can_preview(&ext) {
        return Preview::default();
    }
    let Ok(model) = crate::model3d::load(file) else {
        return Preview::default();
    };
    let mut out = Preview {
        print: model.print.clone(),
        ..Default::default()
    };
    if !model.mesh.tris.is_empty() {
        let prepared = crate::render::prepare(model.mesh.clone());
        out.summary = Some(ModelSummary {
            objects: model.objects.len(),
            triangles: prepared.mesh.tris.len(),
            size_mm: prepared.size,
        });
        let fr = crate::render::thumbnail(&prepared, 256);
        out.thumb = save_thumb(lib_dir, id, &fr.rgba, fr.width as u32, fr.height as u32);
    } else if let Some((rgba, w, h)) = &model.thumbnail {
        out.thumb = save_thumb(lib_dir, id, rgba, *w, *h);
    }
    out
}

/// Reads entries again that were imported before PrintZilla knew how to read
/// print facts, preview pictures from G-Code or the download page.
/// Returns the updated entries (only the fields that are filled in here change).
pub fn refresh_old_entries(lib_dir: &Path, entries: Vec<Entry>) -> Vec<Entry> {
    let mut out = Vec::new();
    for mut e in entries {
        let path = lib_dir.join(&e.file);
        if matches!(e.ext.as_str(), "3mf" | "gcode" | "bgcode") && path.exists() {
            if matches!(e.ext.as_str(), "gcode" | "bgcode") {
                if let Ok((info, thumb)) = crate::printinfo::load_gcode(&path) {
                    e.print = (!info.is_empty()).then_some(info);
                    if e.thumb.is_none() {
                        e.thumb = thumb.and_then(|(rgba, w, h)| save_thumb(lib_dir, e.id, &rgba, w, h));
                    }
                }
            } else if let Ok(f) = std::fs::File::open(&path) {
                if let Ok(mut zip) = zip::ZipArchive::new(f) {
                    e.print = crate::printinfo::from_3mf(&mut zip);
                }
            }
        }
        if e.url.is_empty() {
            // the original may still be where it was downloaded to
            for p in [Path::new(&e.source), path.as_path()] {
                if let Some(u) = crate::origin::download_url(p) {
                    e.url = u;
                    break;
                }
            }
        }
        e.info_rev = INFO_REV;
        out.push(e);
    }
    out
}

/// Characters that are not allowed in file names on Windows or macOS.
pub fn safe_file_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() { '_' } else { c })
        .collect();
    s.trim().trim_end_matches('.').trim().to_string()
}

pub fn run_import(job: ImportJob, mut progress: impl FnMut(ImportMsg)) -> ImportReport {
    let mut report = ImportReport::default();
    let mut known = job.known_hashes.clone();
    let mut id = job.first_id;
    let total = job.files.len();
    let _ = std::fs::create_dir_all(&job.library_dir);
    for (i, f) in job.files.iter().enumerate() {
        progress(ImportMsg::Progress {
            done: i,
            total,
            current: f.name.clone(),
        });
        let hash = match hash_file(&f.path) {
            Ok(h) => h,
            Err(e) => {
                report.errors.push(format!("{}: {e}", f.name));
                continue;
            }
        };
        let mut safe_to_delete = false;
        if job.skip_duplicates && known.contains(&hash) {
            report.duplicates += 1;
            safe_to_delete = true;
        } else {
            let target = unique_target(&job.library_dir, &f.name);
            match std::fs::copy(&f.path, &target) {
                Ok(n) if n == f.size || f.size == 0 => {
                    let file = target
                        .strip_prefix(&job.library_dir)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| f.name.clone());
                    let preview = make_preview(&job.library_dir, &target, id);
                    // ask the system before the original may go to the trash
                    let url = crate::origin::download_url(&f.path).unwrap_or_default();
                    let mut tags = Vec::new();
                    if job.folder_as_tag {
                        if let Some(last) = Path::new(&f.rel_dir).file_name() {
                            let t = last.to_string_lossy().to_string();
                            if !t.is_empty() {
                                tags.push(t);
                            }
                        }
                    }
                    let name = Path::new(&f.name)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| f.name.clone());
                    report.entries.push(Entry {
                        id,
                        name,
                        file,
                        ext: f.ext.clone(),
                        size: f.size,
                        hash: hash.clone(),
                        imported: chrono::Local::now().timestamp(),
                        source: f.path.to_string_lossy().to_string(),
                        tags,
                        notes: String::new(),
                        favorite: false,
                        thumb: preview.thumb,
                        summary: preview.summary,
                        print: preview.print,
                        url,
                        info_rev: INFO_REV,
                    });
                    id += 1;
                    known.insert(hash);
                    safe_to_delete = true;
                }
                Ok(_) => {
                    let _ = std::fs::remove_file(&target);
                    report.errors.push(format!("{}: Kopie unvollständig, Original bleibt erhalten", f.name));
                }
                Err(e) => report.errors.push(format!("{}: {e}", f.name)),
            }
        }
        if safe_to_delete && job.after != AfterImport::Keep {
            let res = match job.after {
                AfterImport::Trash => trash::delete(&f.path).map_err(|e| e.to_string()),
                AfterImport::Delete => std::fs::remove_file(&f.path).map_err(|e| e.to_string()),
                AfterImport::Keep => Ok(()),
            };
            match res {
                Ok(()) => report.deleted += 1,
                Err(e) => report.errors.push(format!("{}: Löschen fehlgeschlagen ({e})", f.name)),
            }
        }
    }
    progress(ImportMsg::Progress {
        done: total,
        total,
        current: String::new(),
    });
    report
}

pub fn human_size(n: u64) -> String {
    let f = n as f64;
    if f >= 1e9 {
        format!("{:.1} GB", f / 1e9)
    } else if f >= 1e6 {
        format!("{:.1} MB", f / 1e6)
    } else if f >= 1e3 {
        format!("{:.0} KB", f / 1e3)
    } else {
        format!("{n} B")
    }
}

pub fn format_date(ts: i64) -> String {
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(ts, 0) {
        chrono::LocalResult::Single(d) => d.format("%d.%m.%Y %H:%M").to_string(),
        _ => "–".into(),
    }
}
