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
}

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

pub fn scan(dir: &Path, recursive: bool, types: &[String], lib: &Library) -> Vec<Found> {
    let known: HashSet<(String, u64)> = lib
        .index
        .entries
        .iter()
        .map(|e| {
            let fname = Path::new(&e.source)
                .file_name()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            (fname, e.size)
        })
        .collect();
    let lib_dir = lib.dir.canonicalize().unwrap_or(lib.dir.clone());
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
        if p.canonicalize().map(|c| c.starts_with(&lib_dir)).unwrap_or(false) {
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

/// Creates the preview picture and model summary for a library file.
pub fn make_preview(lib_dir: &Path, file: &Path, id: u64) -> (Option<String>, Option<ModelSummary>) {
    let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !crate::model3d::can_preview(&ext) {
        return (None, None);
    }
    let Ok(model) = crate::model3d::load(file) else {
        return (None, None);
    };
    let thumb_name = format!("{id}.png");
    let thumb_file = Library::thumbs_dir(lib_dir).join(&thumb_name);
    let _ = std::fs::create_dir_all(Library::thumbs_dir(lib_dir));
    let mut summary = None;
    let mut saved = false;
    if !model.mesh.tris.is_empty() {
        let prepared = crate::render::prepare(model.mesh.clone());
        summary = Some(ModelSummary {
            objects: model.objects.len(),
            triangles: prepared.mesh.tris.len(),
            size_mm: prepared.size,
        });
        let fr = crate::render::thumbnail(&prepared, 256);
        saved = image::save_buffer(&thumb_file, &fr.rgba, fr.width as u32, fr.height as u32, image::ExtendedColorType::Rgba8).is_ok();
    } else if let Some((rgba, w, h)) = &model.thumbnail {
        saved = image::save_buffer(&thumb_file, rgba, *w, *h, image::ExtendedColorType::Rgba8).is_ok();
    }
    (saved.then_some(thumb_name), summary)
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
                    let (thumb, summary) = make_preview(&job.library_dir, &target, id);
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
                        thumb,
                        summary,
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
