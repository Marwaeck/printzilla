//! Checks for the import and the 3MF reader. They work in target/testdaten inside the project.

use crate::library::*;
use crate::model3d::*;
use std::io::Write;
use std::path::PathBuf;

fn workdir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("testdaten").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn import_copies_skips_duplicates_and_deletes_originals() {
    let base = workdir("import");
    let src = base.join("quelle");
    let libdir = base.join("bibliothek");
    std::fs::create_dir_all(src.join("Unterordner")).unwrap();
    std::fs::write(src.join("a.stl"), b"solid a\nendsolid a\n").unwrap();
    std::fs::write(src.join("Unterordner").join("b.gcode"), b"G28\n").unwrap();
    std::fs::write(src.join("Unterordner").join("kopie.stl"), b"solid a\nendsolid a\n").unwrap();
    std::fs::write(src.join("notiz.txt"), b"nicht importieren").unwrap();
    let mut lib = Library::open(&libdir);
    let types: Vec<String> = vec!["stl".into(), "gcode".into()];
    let found = scan(&src, true, &types, &lib);
    assert_eq!(found.len(), 3);
    let report = run_import(
        ImportJob {
            files: found,
            library_dir: libdir.clone(),
            known_hashes: lib.hashes(),
            first_id: 1,
            after: AfterImport::Delete,
            skip_duplicates: true,
            folder_as_tag: true,
        },
        |_| {},
    );
    assert_eq!(report.entries.len(), 2, "errors: {:?}", report.errors);
    assert_eq!(report.duplicates, 1);
    assert_eq!(report.deleted, 3);
    assert!(report.entries.iter().any(|e| e.tags == vec!["Unterordner".to_string()]));
    lib.add_entries(report.entries);
    assert!(!src.join("a.stl").exists());
    assert!(src.join("notiz.txt").exists());
    let lib2 = Library::open(&libdir);
    assert_eq!(lib2.index.entries.len(), 2);
    for e in &lib2.index.entries {
        assert!(lib2.file_path(e).exists());
    }
    // scanning only the top folder
    let again = scan(&src, false, &types, &lib2);
    assert!(again.is_empty());
}

/// Slicer-style 3MF: the build item points to an object whose parts live in another file.
#[test]
fn reads_components_from_sub_files() {
    let p = workdir("komponenten").join("teile.3mf");
    let f = std::fs::File::create(&p).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let o = zip::write::SimpleFileOptions::default();
    z.start_file("_rels/.rels", o).unwrap();
    z.write_all(br#"<Relationships><Relationship Target="/3D/3dmodel.model" Id="r" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>"#).unwrap();
    z.start_file("3D/3dmodel.model", o).unwrap();
    z.write_all(br#"<?xml version="1.0"?><model unit="millimeter" xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06"><metadata name="Title">Test</metadata><resources><object id="2" type="model"><components><component p:path="/3D/Objects/object_1.model" objectid="1" transform="1 0 0 0 1 0 0 0 1 10 0 0"/></components></object></resources><build><item objectid="2" transform="1 0 0 0 1 0 0 0 1 100 100 0"/></build></model>"#).unwrap();
    z.start_file("3D/Objects/object_1.model", o).unwrap();
    z.write_all(br#"<model unit="millimeter"><resources><object id="1" type="model"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="5" y="0" z="0"/><vertex x="0" y="5" z="2"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#).unwrap();
    z.finish().unwrap();
    let m = load_3mf(&p).unwrap();
    assert_eq!(m.mesh.tris.len(), 1);
    assert_eq!(m.mesh.positions[0], [110.0, 100.0, 0.0]);
    assert_eq!(m.metadata[0], ("Title".to_string(), "Test".to_string()));
}

#[test]
fn logo_renders() {
    let n = 512;
    let px = crate::logo::logo_rgba(n);
    assert_eq!(px.len(), n * n * 4);
    let out = workdir("logo").join("logo.png");
    image::save_buffer(&out, &px, n as u32, n as u32, image::ExtendedColorType::Rgba8).unwrap();
}

/// Writes the program icon files used by the Windows build and the macOS app bundle.
#[test]
fn writes_icon_files() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    std::fs::create_dir_all(&dir).unwrap();
    let big = crate::logo::logo_rgba(1024);
    image::save_buffer(dir.join("printzilla.png"), &big, 1024, 1024, image::ExtendedColorType::Rgba8).unwrap();
    let mac = crate::logo::logo_rgba_macos(1024);
    image::save_buffer(dir.join("printzilla-macos.png"), &mac, 1024, 1024, image::ExtendedColorType::Rgba8).unwrap();
    let small = crate::logo::logo_rgba(256);
    image::save_buffer(dir.join("printzilla.ico"), &small, 256, 256, image::ExtendedColorType::Rgba8).unwrap();
}

#[test]
fn solidcraft_is_offered_for_print_files() {
    let cfg = Config::with_dir(workdir("programme"));
    let progs = crate::externals::programs(&cfg);
    let sc = progs.iter().find(|p| p.key == "solidcraft").expect("SolidCraft wird immer angeboten");
    assert!(sc.supports("3mf") && sc.supports("stl") && sc.supports("step"));
    assert!(!sc.supports("gcode"));
    if cfg!(target_os = "macos") && std::path::Path::new("/Applications/SolidCraft.app").exists() {
        assert!(sc.location.is_some(), "installiertes SolidCraft wurde nicht gefunden");
    }
}

#[test]
fn menu_icon_renders() {
    let n = 44;
    let px = crate::logo::menu_icon_rgba(n);
    assert_eq!(px.len(), n * n * 4);
    assert!(px.chunks(4).any(|p| p[3] > 200), "symbol must not be empty");
    let big = crate::logo::menu_icon_rgba(256);
    image::save_buffer(workdir("menuicon").join("menu.png"), &big, 256, 256, image::ExtendedColorType::Rgba8).unwrap();
}

#[test]
fn license_texts_are_built_in() {
    assert!(crate::about_ui::LICENSE_TEXT.starts_with("MIT License"));
    assert!(crate::about_ui::LICENSE_TEXT.contains("Marwaeck"));
    let tp = crate::about_ui::THIRD_PARTY_TEXT;
    for name in ["- eframe ", "- egui ", "- epaint_default_fonts ", "- self_update ", "- tray-icon "] {
        assert!(tp.contains(name), "{name} fehlt in THIRD_PARTY_LICENSES.txt");
    }
    assert!(tp.contains("OFL.txt") && tp.contains("UFL.txt"), "Schriftlizenzen fehlen");
}

#[test]
fn watcher_reports_finished_new_files_and_respects_ignore() {
    use crate::library::{IgnoredFile, WatchFolder};
    let base = workdir("watcher");
    let watched = base.join("Downloads");
    std::fs::create_dir_all(&watched).unwrap();
    let lib = Library::open(&base.join("bibliothek"));
    std::fs::write(watched.join("halter.stl"), b"solid h\nendsolid h\n").unwrap();
    std::fs::write(watched.join("notiz.txt"), b"keine Druckdatei").unwrap();
    // let the file "finish downloading"
    std::thread::sleep(std::time::Duration::from_millis(4500));

    let mut cfg = Config::with_dir(base.join("bibliothek"));
    cfg.watch_folders = vec![WatchFolder { path: watched.clone(), recursive: true }];
    let ctx = eframe::egui::Context::default();
    let mut w = crate::watcher::Watcher::start(&ctx, &cfg, &lib);
    // first look only notes the file, the second confirms it is unchanged
    let wait = |w: &mut crate::watcher::Watcher| {
        let mut fresh = 0;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            fresh += w.poll();
        }
        fresh
    };
    wait(&mut w);
    w.check_now();
    let fresh = wait(&mut w);
    assert_eq!(fresh, 1, "die neue STL-Datei soll genau einmal gemeldet werden");
    assert_eq!(w.pending.len(), 1);
    assert_eq!(w.pending[0].name, "halter.stl");
    assert_eq!(w.pending[0].rel_dir, "Downloads");

    // "Nicht mehr anzeigen"
    cfg.watch_ignored.push(IgnoredFile { path: watched.join("halter.stl").to_string_lossy().to_string(), size: w.pending[0].size });
    w.update_settings(&cfg);
    w.check_now();
    wait(&mut w);
    assert!(w.pending.is_empty(), "ignorierte Datei darf nicht mehr gemeldet werden");
}

#[test]
fn icon_with_number_renders() {
    let dir = workdir("badge");
    for n in [0usize, 3, 12] {
        let px = crate::logo::logo_with_badge(256, n);
        assert_eq!(px.len(), 256 * 256 * 4);
        image::save_buffer(dir.join(format!("badge-{n}.png")), &px, 256, 256, image::ExtendedColorType::Rgba8).unwrap();
    }
    assert_eq!(crate::logo::logo_with_badge(64, 0), crate::logo::logo_rgba(64));
}

/// Outside PrintZilla.app there is no app identifier: then no system message is sent,
/// so macOS never asks "Which program …?" (the old "use_default" lookup).
#[cfg(target_os = "macos")]
#[test]
fn no_program_question_outside_the_app_package() {
    assert_eq!(crate::watcher::own_bundle_id(), None);
    assert!(!crate::watcher::macos_sender_ready());
    // must return at once without asking anything
    crate::watcher::system_notification("PrintZilla", "Test");
}

#[test]
fn rename_changes_name_and_file() {
    let base = workdir("umbenennen");
    let src = base.join("quelle");
    let libdir = base.join("bibliothek");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("model_v2_final(3).stl"), b"solid a\nendsolid a\n").unwrap();
    std::fs::write(src.join("anderes.stl"), b"solid b\nendsolid b\n").unwrap();
    let mut lib = Library::open(&libdir);
    let found = scan(&src, false, &["stl".to_string()], &lib);
    let report = run_import(
        ImportJob {
            files: found,
            library_dir: libdir.clone(),
            known_hashes: lib.hashes(),
            first_id: 1,
            after: AfterImport::Keep,
            skip_duplicates: true,
            folder_as_tag: false,
        },
        |_| {},
    );
    lib.add_entries(report.entries);
    let id = lib.index.entries.iter().find(|e| e.file.starts_with("model")).unwrap().id;
    let other = lib.index.entries.iter().find(|e| e.file.starts_with("anderes")).unwrap().id;

    assert_eq!(lib.rename(id, "Kabelhalter: Schreibtisch").unwrap(), "Kabelhalter_ Schreibtisch.stl");
    let e = lib.index.entries.iter().find(|e| e.id == id).unwrap().clone();
    assert_eq!(e.name, "Kabelhalter: Schreibtisch");
    assert!(lib.file_path(&e).exists());
    assert!(!libdir.join("model_v2_final(3).stl").exists());

    // same name as another file: no overwriting
    assert_eq!(lib.rename(other, "Kabelhalter_ Schreibtisch").unwrap(), "Kabelhalter_ Schreibtisch (2).stl");
    // only the spelling
    assert_eq!(lib.rename(id, "kabelhalter_ schreibtisch").unwrap(), "kabelhalter_ schreibtisch.stl");
    assert!(lib.rename(id, "   ").is_err());
    // stays after opening the library again
    let lib2 = Library::open(&libdir);
    assert_eq!(lib2.index.entries.iter().filter(|e| lib2.file_path(e).exists()).count(), 2);
}

#[test]
fn gcode_import_reads_print_facts() {
    let base = workdir("gcode");
    let src = base.join("quelle");
    let libdir = base.join("bibliothek");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("teil.gcode"),
        b"; generated by OrcaSlicer 2.1.1\n; total estimated time: 2h 30m\n; total filament weight [g] : 41.2\n; filament_type = PETG\nG28\n",
    )
    .unwrap();
    let lib = Library::open(&libdir);
    let found = scan(&src, false, &["gcode".to_string()], &lib);
    let report = run_import(
        ImportJob {
            files: found,
            library_dir: libdir.clone(),
            known_hashes: lib.hashes(),
            first_id: 1,
            after: AfterImport::Keep,
            skip_duplicates: true,
            folder_as_tag: false,
        },
        |_| {},
    );
    let e = &report.entries[0];
    let p = e.print.as_ref().expect("print facts");
    assert_eq!(p.time_s, Some(9000));
    assert_eq!(p.filament_g, Some(41.2));
    assert_eq!(p.materials, vec!["PETG"]);
    assert_eq!(p.slicer.as_deref(), Some("OrcaSlicer 2.1.1"));
    assert_eq!(e.info_rev, INFO_REV);

    // an entry from an older version gets read again
    let mut old = e.clone();
    old.print = None;
    old.info_rev = 0;
    let fresh = refresh_old_entries(&libdir, vec![old]);
    assert_eq!(fresh[0].print, e.print);
}

/// A Bambu Studio project saved after slicing: print time and filament from slice_info.
#[test]
fn sliced_3mf_has_print_facts() {
    let p = workdir("bambu3mf").join("projekt.3mf");
    let f = std::fs::File::create(&p).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let o = zip::write::SimpleFileOptions::default();
    z.start_file("3D/3dmodel.model", o).unwrap();
    z.write_all(br#"<?xml version="1.0"?><model unit="millimeter"><metadata name="Application">BambuStudio-01.10.01.50</metadata><resources><object id="1" type="model"><mesh><vertices><vertex x="0" y="0" z="0"/><vertex x="10" y="0" z="0"/><vertex x="0" y="10" z="0"/><vertex x="0" y="0" z="10"/></vertices><triangles><triangle v1="0" v2="1" v3="2"/><triangle v1="0" v2="1" v3="3"/></triangles></mesh></object></resources><build><item objectid="1"/></build></model>"#).unwrap();
    z.start_file("Metadata/slice_info.config", o).unwrap();
    z.write_all(br##"<config><plate><metadata key="prediction" value="4500"/><metadata key="weight" value="12.3"/><filament id="1" type="PLA" color="#112233" used_m="4.1" used_g="12.3"/></plate></config>"##).unwrap();
    z.start_file("Metadata/project_settings.config", o).unwrap();
    z.write_all(br#"{"layer_height":"0.2","nozzle_diameter":["0.4"],"printer_model":"Bambu Lab A1 mini"}"#).unwrap();
    z.finish().unwrap();
    let m = load(&p).unwrap();
    let info = m.print.expect("print facts");
    assert_eq!(info.time_s, Some(4500));
    assert_eq!(info.filament_g, Some(12.3));
    assert_eq!(info.materials, vec!["PLA"]);
    assert_eq!(info.colors, vec!["#112233"]);
    assert_eq!(info.layer_mm, Some(0.2));
    assert_eq!(info.printer.as_deref(), Some("Bambu Lab A1 mini"));
    assert_eq!(info.slicer.as_deref(), Some("Bambu Studio 1.10.01.50"));
    assert!(info.sliced);
}
