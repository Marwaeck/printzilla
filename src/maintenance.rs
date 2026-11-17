//! Looking after the library: backups as a ZIP file, restoring them, and checking
//! that the index still matches the files on disk.

use crate::library::{self, Entry, Index, Library, INFO_REV};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Progress for long jobs: (done, total, what is being worked on).
pub type Progress<'a> = &'a mut dyn FnMut(usize, usize, &str);

// ---------------------------------------------------------------------------
// Backup

pub const MANIFEST: &str = "printzilla-sicherung.json";
const INDEX: &str = "bibliothek.json";
const THUMBS: &str = "vorschau/";
const FILES: &str = "dateien/";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Manifest {
    pub format: u32,
    pub app_version: String,
    pub created: i64,
    pub library_dir: String,
    pub entries: usize,
    pub with_files: bool,
}

/// Paths inside the ZIP always use "/".
fn zip_name(rel: &str) -> String {
    rel.replace('\\', "/")
}

/// Writes a backup of the library to `dest`. With `with_files` the 3D files themselves
/// go into it too; otherwise names, tags, notes, favourites, sources and previews.
pub fn create_backup(lib_dir: &Path, index: &Index, dest: &Path, with_files: bool, progress: Progress) -> Result<u64, String> {
    let tmp = dest.with_extension("zip.part");
    let res = write_backup(lib_dir, index, &tmp, with_files, progress);
    match res {
        Ok(()) => {
            let _ = std::fs::remove_file(dest);
            std::fs::rename(&tmp, dest).map_err(|e| format!("Sicherung konnte nicht gespeichert werden: {e}"))?;
            Ok(std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

fn write_backup(lib_dir: &Path, index: &Index, dest: &Path, with_files: bool, progress: Progress) -> Result<(), String> {
    let err = |e: &dyn std::fmt::Display| format!("Sicherung fehlgeschlagen: {e}");
    let f = std::fs::File::create(dest).map_err(|e| err(&e))?;
    let mut z = zip::ZipWriter::new(f);
    let deflate = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let manifest = Manifest {
        format: 1,
        app_version: crate::updater::CURRENT_VERSION.to_string(),
        created: chrono::Local::now().timestamp(),
        library_dir: lib_dir.to_string_lossy().to_string(),
        entries: index.entries.len(),
        with_files,
    };
    z.start_file(MANIFEST, deflate).map_err(|e| err(&e))?;
    z.write_all(serde_json::to_string_pretty(&manifest).unwrap_or_default().as_bytes()).map_err(|e| err(&e))?;
    z.start_file(INDEX, deflate).map_err(|e| err(&e))?;
    z.write_all(serde_json::to_string_pretty(index).unwrap_or_default().as_bytes()).map_err(|e| err(&e))?;

    let thumbs = Library::thumbs_dir(lib_dir);
    let total = index.entries.len();
    for (i, e) in index.entries.iter().enumerate() {
        progress(i, total, &e.name);
        if let Some(t) = &e.thumb {
            if let Ok(bytes) = std::fs::read(thumbs.join(t)) {
                z.start_file(format!("{THUMBS}{t}"), stored).map_err(|e| err(&e))?;
                z.write_all(&bytes).map_err(|e| err(&e))?;
            }
        }
        if with_files {
            let p = lib_dir.join(&e.file);
            let Ok(mut src) = std::fs::File::open(&p) else { continue };
            let size = src.metadata().map(|m| m.len()).unwrap_or(0);
            // 3MF and binary G-Code are packed already
            let opts = if matches!(e.ext.as_str(), "3mf" | "bgcode" | "f3d") { stored } else { deflate };
            let opts = opts.large_file(size >= 0xFFFF_FFFF);
            z.start_file(format!("{FILES}{}", zip_name(&e.file)), opts).map_err(|e| err(&e))?;
            std::io::copy(&mut src, &mut z).map_err(|e| err(&e))?;
        }
    }
    progress(total, total, "");
    z.finish().map_err(|e| err(&e))?;
    Ok(())
}

/// What a backup file contains (shown before restoring).
pub struct BackupInfo {
    pub manifest: Manifest,
    pub index: Index,
    /// File names (inside "dateien/") that are in the backup.
    pub files: HashSet<String>,
}

pub fn read_backup(path: &Path) -> Result<BackupInfo, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("Datei lässt sich nicht öffnen: {e}"))?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| "Das ist keine ZIP-Datei.".to_string())?;
    let mut read = |name: &str| -> Option<String> {
        let mut s = String::new();
        z.by_name(name).ok()?.read_to_string(&mut s).ok()?;
        Some(s)
    };
    let index: Index = read(INDEX)
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or("Das ist keine PrintZilla-Sicherung (bibliothek.json fehlt).")?;
    let manifest = read(MANIFEST).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Manifest {
        format: 1,
        app_version: String::new(),
        created: 0,
        library_dir: String::new(),
        entries: index.entries.len(),
        with_files: false,
    });
    let files = z.file_names().filter_map(|n| n.strip_prefix(FILES)).map(|s| s.to_string()).collect();
    Ok(BackupInfo { manifest, index, files })
}

#[derive(Default)]
pub struct RestoreResult {
    /// Entries already in the library that got names, tags … from the backup.
    pub updated: Vec<Entry>,
    /// Entries whose files came out of the backup.
    pub added: Vec<Entry>,
    /// In the backup, but the file is neither in the backup nor in the library.
    pub unavailable: Vec<String>,
    /// Copy of the index from before restoring.
    pub safety_copy: Option<PathBuf>,
    pub errors: Vec<String>,
}

/// Brings a backup back. Nothing is deleted: files already in the library keep their
/// place and get names, tags, notes, favourite and source from the backup; files that
/// are only in the backup are unpacked into the library.
pub fn restore_backup(lib_dir: &Path, current: &[Entry], backup: &Path, first_id: u64, progress: Progress) -> Result<RestoreResult, String> {
    let info = read_backup(backup)?;
    let f = std::fs::File::open(backup).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| e.to_string())?;
    let mut out = RestoreResult::default();

    // keep the index as it was, in case the person wants it back
    let meta = Library::meta_dir(lib_dir);
    let _ = std::fs::create_dir_all(&meta);
    let idx = meta.join("bibliothek.json");
    if idx.exists() {
        let copy = meta.join(format!("bibliothek-vor-wiederherstellung-{}.json", chrono::Local::now().format("%Y-%m-%d-%H%M%S")));
        if std::fs::copy(&idx, &copy).is_ok() {
            out.safety_copy = Some(copy);
        }
    }

    let by_hash: HashMap<&str, &Entry> = current.iter().map(|e| (e.hash.as_str(), e)).collect();
    let mut id = first_id;
    let total = info.index.entries.len();
    for (i, b) in info.index.entries.iter().enumerate() {
        progress(i, total, &b.name);
        if let Some(cur) = by_hash.get(b.hash.as_str()) {
            let mut e = (*cur).clone();
            e.name = b.name.clone();
            e.tags = b.tags.clone();
            e.notes = b.notes.clone();
            e.favorite = b.favorite;
            if !b.url.is_empty() {
                e.url = b.url.clone();
            }
            out.updated.push(e);
            continue;
        }
        let zname = zip_name(&b.file);
        if !info.files.contains(&zname) {
            out.unavailable.push(b.name.clone());
            continue;
        }
        // unpack into the same sub-folder as before
        let rel = Path::new(&zname);
        let dir = rel.parent().map(|p| lib_dir.join(p)).unwrap_or_else(|| lib_dir.to_path_buf());
        let _ = std::fs::create_dir_all(&dir);
        let fname = rel.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| b.name.clone());
        let target = library::unique_target(&dir, &fname);
        let res = (|| -> Result<(), String> {
            let mut src = z.by_name(&format!("{FILES}{zname}")).map_err(|e| e.to_string())?;
            let mut dst = std::fs::File::create(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut src, &mut dst).map_err(|e| e.to_string())?;
            Ok(())
        })();
        if let Err(err) = res {
            let _ = std::fs::remove_file(&target);
            out.errors.push(format!("{}: {err}", b.name));
            continue;
        }
        let mut e = b.clone();
        e.id = id;
        id += 1;
        e.file = target.strip_prefix(lib_dir).map(|p| p.to_string_lossy().to_string()).unwrap_or(fname);
        e.size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(b.size);
        let p = library::make_preview(lib_dir, &target, e.id);
        e.thumb = p.thumb;
        e.summary = p.summary.or(e.summary);
        e.print = p.print.or(e.print);
        e.info_rev = INFO_REV;
        out.added.push(e);
    }
    progress(total, total, "");
    Ok(out)
}

// ---------------------------------------------------------------------------
// Checking the library against the disk

#[derive(Clone, Debug)]
pub struct Missing {
    pub id: u64,
    pub name: String,
    pub file: String,
    /// The same file (same content) lies somewhere else in the library: moved or renamed.
    pub found_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Untracked {
    /// Path inside the library folder.
    pub file: String,
    pub size: u64,
    pub hash: String,
    /// Same content as a file already in the library.
    pub duplicate_of: Option<u64>,
    /// This is where a missing entry went.
    pub relinks: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct CheckReport {
    pub missing: Vec<Missing>,
    pub untracked: Vec<Untracked>,
    /// Entries whose file changed on disk (different size).
    pub changed: Vec<u64>,
    /// Preview pictures that belong to no entry.
    pub stray_thumbs: Vec<String>,
}

impl CheckReport {
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.untracked.is_empty() && self.changed.is_empty() && self.stray_thumbs.is_empty()
    }
    pub fn moved(&self) -> usize {
        self.missing.iter().filter(|m| m.found_at.is_some()).count()
    }
    pub fn gone(&self) -> usize {
        self.missing.iter().filter(|m| m.found_at.is_none()).count()
    }
    pub fn new_files(&self) -> usize {
        self.untracked.iter().filter(|u| u.relinks.is_none() && u.duplicate_of.is_none()).count()
    }
    pub fn duplicates(&self) -> usize {
        self.untracked.iter().filter(|u| u.relinks.is_none() && u.duplicate_of.is_some()).count()
    }
}

/// Paths are compared like the file system does: macOS and Windows ignore upper/lower case.
fn key(rel: &str) -> String {
    let s = rel.replace('\\', "/");
    if cfg!(any(target_os = "macos", windows)) {
        s.to_lowercase()
    } else {
        s
    }
}

pub fn check_library(lib_dir: &Path, entries: &[Entry], progress: Progress) -> CheckReport {
    let mut r = CheckReport::default();
    let types: HashSet<&str> = library::ALL_TYPES.iter().map(|(t, _)| *t).collect();
    let known: HashSet<String> = entries.iter().map(|e| key(&e.file)).collect();

    // files in the folder that the index doesn't know
    let mut candidates = Vec::new();
    for item in walkdir::WalkDir::new(lib_dir)
        .into_iter()
        .filter_entry(|d| d.depth() == 0 || !d.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|d| d.ok())
    {
        if !item.file_type().is_file() {
            continue;
        }
        let p = item.path();
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !types.contains(ext.as_str()) {
            continue;
        }
        let Ok(rel) = p.strip_prefix(lib_dir) else { continue };
        let rel = rel.to_string_lossy().to_string();
        if !known.contains(&key(&rel)) {
            candidates.push((rel, item.metadata().map(|m| m.len()).unwrap_or(0)));
        }
    }

    let present: HashMap<&str, u64> = entries
        .iter()
        .filter(|e| lib_dir.join(&e.file).exists())
        .map(|e| (e.hash.as_str(), e.id))
        .collect();
    let total = candidates.len();
    for (i, (rel, size)) in candidates.into_iter().enumerate() {
        progress(i, total, &rel);
        let Ok(hash) = library::hash_file(&lib_dir.join(&rel)) else { continue };
        r.untracked.push(Untracked {
            duplicate_of: present.get(hash.as_str()).copied(),
            file: rel,
            size,
            hash,
            relinks: None,
        });
    }
    progress(total, total, "");

    // entries without file, and files changed from outside
    for e in entries {
        let p = lib_dir.join(&e.file);
        match std::fs::metadata(&p) {
            Ok(m) if m.is_file() => {
                if m.len() != e.size {
                    r.changed.push(e.id);
                }
            }
            _ => {
                let mut m = Missing {
                    id: e.id,
                    name: e.name.clone(),
                    file: e.file.clone(),
                    found_at: None,
                };
                if let Some(u) = r.untracked.iter_mut().find(|u| u.hash == e.hash && u.relinks.is_none()) {
                    u.relinks = Some(e.id);
                    m.found_at = Some(u.file.clone());
                }
                r.missing.push(m);
            }
        }
    }

    // preview pictures nobody uses
    let used: HashSet<&str> = entries.iter().filter_map(|e| e.thumb.as_deref()).collect();
    if let Ok(rd) = std::fs::read_dir(Library::thumbs_dir(lib_dir)) {
        for d in rd.flatten() {
            let n = d.file_name().to_string_lossy().to_string();
            if d.path().is_file() && !used.contains(n.as_str()) {
                r.stray_thumbs.push(n);
            }
        }
    }
    r.stray_thumbs.sort();
    r
}

/// What the person wants done with the findings.
#[derive(Clone, Copy, Debug)]
pub struct Repair {
    /// Point moved/renamed entries to their new place.
    pub relink: bool,
    /// Take entries out of the index whose file is gone.
    pub remove_gone: bool,
    /// Add files that were put into the folder from outside.
    pub add_new: bool,
    /// Also add files whose content is already in the library.
    pub add_duplicates: bool,
    /// Read changed files again (size, preview, print facts).
    pub refresh_changed: bool,
    /// Delete preview pictures that belong to no entry.
    pub remove_stray_thumbs: bool,
    /// Use the sub-folder name as a tag for added files.
    pub folder_as_tag: bool,
}

impl Default for Repair {
    fn default() -> Self {
        Repair {
            relink: true,
            remove_gone: true,
            add_new: true,
            add_duplicates: false,
            refresh_changed: true,
            remove_stray_thumbs: true,
            folder_as_tag: true,
        }
    }
}

#[derive(Default)]
pub struct RepairResult {
    /// (entry, new path inside the library)
    pub relinked: Vec<(u64, String)>,
    pub removed: Vec<u64>,
    pub added: Vec<Entry>,
    pub refreshed: Vec<Entry>,
    pub thumbs_removed: usize,
}

impl RepairResult {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.relinked.is_empty() {
            parts.push(format!("{} wiedergefunden", self.relinked.len()));
        }
        if !self.added.is_empty() {
            parts.push(format!("{} aufgenommen", self.added.len()));
        }
        if !self.removed.is_empty() {
            parts.push(format!("{} Einträge entfernt", self.removed.len()));
        }
        if !self.refreshed.is_empty() {
            parts.push(format!("{} aktualisiert", self.refreshed.len()));
        }
        if self.thumbs_removed > 0 {
            parts.push(format!("{} alte Vorschaubilder gelöscht", self.thumbs_removed));
        }
        if parts.is_empty() {
            "Nichts geändert".into()
        } else {
            parts.join(" · ")
        }
    }
}

/// Does the slow part of the repair (hashes, previews). The index itself is changed
/// afterwards with `apply_repair`, on the main thread.
pub fn repair(lib_dir: &Path, entries: &[Entry], report: &CheckReport, how: Repair, first_id: u64, progress: Progress) -> RepairResult {
    let mut out = RepairResult::default();
    if how.relink {
        for m in &report.missing {
            if let Some(f) = &m.found_at {
                out.relinked.push((m.id, f.clone()));
            }
        }
    }
    if how.remove_gone {
        out.removed = report.missing.iter().filter(|m| m.found_at.is_none() || !how.relink).map(|m| m.id).collect();
    }
    let to_add: Vec<&Untracked> = report
        .untracked
        .iter()
        .filter(|u| u.relinks.is_none() || !how.relink)
        .filter(|u| how.add_new && (u.duplicate_of.is_none() || how.add_duplicates))
        .collect();
    let changed: Vec<&Entry> = if how.refresh_changed { entries.iter().filter(|e| report.changed.contains(&e.id)).collect() } else { Vec::new() };
    let total = to_add.len() + changed.len();
    let mut done = 0;
    let mut id = first_id;
    for u in to_add {
        progress(done, total, &u.file);
        done += 1;
        let path = lib_dir.join(&u.file);
        let p = library::make_preview(lib_dir, &path, id);
        let rel = Path::new(&u.file);
        let mut tags = Vec::new();
        if how.folder_as_tag {
            if let Some(parent) = rel.parent().and_then(|p| p.file_name()) {
                tags.push(parent.to_string_lossy().to_string());
            }
        }
        out.added.push(Entry {
            id,
            name: rel.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| u.file.clone()),
            file: u.file.clone(),
            ext: rel.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default(),
            size: u.size,
            hash: u.hash.clone(),
            imported: chrono::Local::now().timestamp(),
            source: path.to_string_lossy().to_string(),
            tags,
            notes: String::new(),
            favorite: false,
            thumb: p.thumb,
            summary: p.summary,
            print: p.print,
            url: crate::origin::download_url(&path).unwrap_or_default(),
            info_rev: INFO_REV,
        });
        id += 1;
    }
    for e in changed {
        progress(done, total, &e.name);
        done += 1;
        let path = lib_dir.join(&e.file);
        let mut n = e.clone();
        n.size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(e.size);
        n.hash = library::hash_file(&path).unwrap_or_else(|_| e.hash.clone());
        let p = library::make_preview(lib_dir, &path, e.id);
        n.thumb = p.thumb.or(n.thumb);
        n.summary = p.summary;
        n.print = p.print;
        n.info_rev = INFO_REV;
        out.refreshed.push(n);
    }
    progress(total, total, "");
    if how.remove_stray_thumbs {
        let dir = Library::thumbs_dir(lib_dir);
        // added entries may just have written a picture with a name from the list
        let fresh: HashSet<String> = out.added.iter().chain(&out.refreshed).filter_map(|e| e.thumb.clone()).collect();
        for t in &report.stray_thumbs {
            if !fresh.contains(t) && std::fs::remove_file(dir.join(t)).is_ok() {
                out.thumbs_removed += 1;
            }
        }
    }
    out
}

/// Writes the outcome of `repair` into the index.
pub fn apply_repair(lib: &mut Library, r: &RepairResult) {
    for (id, file) in &r.relinked {
        if let Some(e) = lib.index.entries.iter_mut().find(|e| e.id == *id) {
            e.file = file.clone();
        }
    }
    lib.index.entries.retain(|e| !r.removed.contains(&e.id));
    for n in &r.refreshed {
        if let Some(e) = lib.index.entries.iter_mut().find(|e| e.id == n.id) {
            e.size = n.size;
            e.hash = n.hash.clone();
            e.thumb = n.thumb.clone();
            e.summary = n.summary.clone();
            e.print = n.print.clone();
            e.info_rev = n.info_rev;
        }
    }
    for e in &r.added {
        lib.index.next_id = lib.index.next_id.max(e.id + 1);
    }
    lib.index.entries.extend(r.added.iter().cloned());
    lib.save();
}

/// Writes the outcome of `restore_backup` into the index.
pub fn apply_restore(lib: &mut Library, r: &RestoreResult) {
    for u in &r.updated {
        if let Some(e) = lib.index.entries.iter_mut().find(|e| e.id == u.id) {
            e.name = u.name.clone();
            e.tags = u.tags.clone();
            e.notes = u.notes.clone();
            e.favorite = u.favorite;
            e.url = u.url.clone();
        }
    }
    for e in &r.added {
        lib.index.next_id = lib.index.next_id.max(e.id + 1);
    }
    lib.index.entries.extend(r.added.iter().cloned());
    lib.save();
}

/// "PrintZilla-Sicherung-2026-11-17.zip"
pub fn default_backup_name() -> String {
    format!("PrintZilla-Sicherung-{}.zip", chrono::Local::now().format("%Y-%m-%d"))
}
