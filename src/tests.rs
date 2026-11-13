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
