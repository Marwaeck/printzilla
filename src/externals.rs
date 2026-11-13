//! Opening a library file directly in another program (SolidCraft, slicers, own choices).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The program offered first in the context menu.
pub const PREFERRED: &str = "solidcraft";

/// A program the person added themselves ("Anderes Programm hinzufügen …").
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CustomProgram {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub key: String,
    pub name: String,
    /// Where it is installed; None = not found yet (PrintZilla asks once).
    pub location: Option<PathBuf>,
    /// File types it opens; None = all.
    pub exts: Option<Vec<&'static str>>,
}

impl Program {
    pub fn supports(&self, ext: &str) -> bool {
        self.exts.as_ref().map(|l| l.contains(&ext)).unwrap_or(true)
    }
}

struct Known {
    key: &'static str,
    name: &'static str,
    mac: &'static [&'static str],
    win: &'static [&'static str],
    exts: &'static [&'static str],
    /// Always listed, even when not installed (then PrintZilla asks where it is).
    always: bool,
}

const KNOWN: &[Known] = &[
    Known {
        key: "solidcraft",
        name: "SolidCraft",
        mac: &["SolidCraft.app"],
        win: &["SolidCraft\\SolidCraft.exe", "Tangency Labs\\SolidCraft\\SolidCraft.exe", "Programs\\SolidCraft\\SolidCraft.exe"],
        exts: &["3mf", "stl", "step", "stp", "iges", "igs"],
        always: true,
    },
    Known {
        key: "bambu",
        name: "Bambu Studio",
        mac: &["BambuStudio.app", "Bambu Studio.app"],
        win: &["Bambu Studio\\bambu-studio.exe"],
        exts: &["3mf", "stl", "obj", "step", "stp"],
        always: false,
    },
    Known {
        key: "orca",
        name: "OrcaSlicer",
        mac: &["OrcaSlicer.app"],
        win: &["OrcaSlicer\\orca-slicer.exe"],
        exts: &["3mf", "stl", "obj", "step", "stp"],
        always: false,
    },
    Known {
        key: "prusa",
        name: "PrusaSlicer",
        mac: &["PrusaSlicer.app", "Original Prusa Drivers/PrusaSlicer.app"],
        win: &["Prusa3D\\PrusaSlicer\\prusa-slicer.exe"],
        exts: &["3mf", "stl", "obj", "step", "stp", "gcode", "bgcode"],
        always: false,
    },
    Known {
        key: "cura",
        name: "UltiMaker Cura",
        mac: &["UltiMaker Cura.app", "Ultimaker Cura.app"],
        win: &["UltiMaker Cura\\UltiMaker-Cura.exe"],
        exts: &["3mf", "stl", "obj"],
        always: false,
    },
];

fn find_installed(k: &Known) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Applications"));
        if let Some(h) = dirs::home_dir() {
            roots.push(h.join("Applications"));
        }
        return k.mac.iter().flat_map(|n| roots.iter().map(move |r| r.join(n))).find(|p| p.exists());
    }
    if cfg!(target_os = "windows") {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Some(v) = std::env::var_os(var) {
                roots.push(PathBuf::from(v));
            }
        }
        return k.win.iter().flat_map(|n| roots.iter().map(move |r| r.join(n))).find(|p| p.exists());
    }
    None
}

/// All programs to offer: known ones (found or remembered), then the person's own.
pub fn programs(cfg: &crate::library::Config) -> Vec<Program> {
    let mut out = Vec::new();
    for k in KNOWN {
        let remembered = cfg.program_paths.get(k.key).cloned().filter(|p| p.exists());
        let location = remembered.or_else(|| find_installed(k));
        if location.is_some() || k.always {
            out.push(Program {
                key: k.key.to_string(),
                name: k.name.to_string(),
                location,
                exts: Some(k.exts.to_vec()),
            });
        }
    }
    for (i, c) in cfg.custom_programs.iter().enumerate() {
        out.push(Program {
            key: format!("custom-{i}"),
            name: c.name.clone(),
            location: Some(c.path.clone()),
            exts: None,
        });
    }
    out
}

/// A readable program name from its path ("SolidCraft.app" -> "SolidCraft").
pub fn name_from_path(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Programm".into())
}

/// Asks where a program is installed.
pub fn pick_program(title: &str) -> Option<PathBuf> {
    let mut dlg = rfd::FileDialog::new().set_title(title);
    if cfg!(target_os = "macos") {
        dlg = dlg.set_directory("/Applications");
    } else if cfg!(target_os = "windows") {
        dlg = dlg.add_filter("Programme", &["exe"]);
        if let Some(pf) = std::env::var_os("ProgramFiles") {
            dlg = dlg.set_directory(pf);
        }
    }
    dlg.pick_file()
}

/// Opens `file` in the program at `program`.
pub fn launch(program: &Path, file: &Path) -> Result<(), String> {
    let res = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg("-a").arg(program).arg(file).spawn()
    } else {
        std::process::Command::new(program).arg(file).spawn()
    };
    res.map(|_| ()).map_err(|e| format!("Programm konnte nicht gestartet werden: {e}"))
}
