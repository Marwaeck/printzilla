//! Example files, used only for the preview picture (in a temporary library that is removed afterwards).

use crate::library::{self, AfterImport, ImportJob, Library};
use crate::model3d::{self, shape_box, shape_gear, shape_lathe, write_3mf};

pub fn create_examples(lib: &mut Library) {
    let src = std::env::temp_dir().join(format!("printzilla-beispiele-{}", std::process::id()));
    write_example_files(&src);
    import_examples(lib, &src);
}

/// Writes a small folder of example print files (3MF, STL, STEP, G-code) into `src`.
pub fn write_example_files(src: &std::path::Path) {
    let src = src.to_path_buf();
    let _ = std::fs::remove_dir_all(&src);
    let _ = std::fs::create_dir_all(src.join("Halterungen"));
    let _ = std::fs::create_dir_all(src.join("Deko"));

    let gear_a = shape_gear(0.0, 0.0, 22.0, 8.0, 14);
    let gear_b = shape_gear(36.0, 0.0, 14.0, 8.0, 9);
    let axle = shape_lathe(0.0, 0.0, &[(4.0, 8.0), (4.0, 16.0)], 24, 0, 0.0);
    let _ = write_3mf(
        &src.join("Zahnrad-Set.3mf"),
        "Zahnrad-Set",
        &[
            ("Großes Zahnrad".into(), gear_a, [235, 140, 52]),
            ("Kleines Zahnrad".into(), gear_b, [74, 158, 222]),
            ("Achse".into(), axle, [210, 210, 215]),
        ],
    );
    let vase_profile: Vec<(f32, f32)> = (0..=40)
        .map(|i| {
            let t = i as f32 / 40.0;
            (18.0 + 10.0 * (t * 3.0).sin() + 6.0 * t, t * 90.0)
        })
        .collect();
    let _ = write_3mf(
        &src.join("Deko").join("Spiralvase.3mf"),
        "Spiralvase",
        &[("Vase".into(), shape_lathe(0.0, 0.0, &vase_profile, 72, 6, 0.08), [170, 120, 220])],
    );
    let hook = vec![
        ("Grundplatte".to_string(), shape_box(0.0, 26.0, 0.0, 60.0, 8.0, 40.0), [120, 190, 90]),
        ("Haken".to_string(), shape_box(22.0, 0.0, 4.0, 16.0, 26.0, 8.0), [120, 190, 90]),
        ("Nase".to_string(), shape_box(22.0, 0.0, 12.0, 16.0, 6.0, 12.0), [90, 160, 70]),
    ];
    let _ = write_3mf(&src.join("Halterungen").join("Wandhaken.3mf"), "Wandhaken", &hook);
    let tower: Vec<(f32, f32)> = vec![
        (16.0, 0.0),
        (16.0, 4.0),
        (12.0, 7.0),
        (9.0, 12.0),
        (8.0, 30.0),
        (10.0, 34.0),
        (11.0, 36.0),
        (11.0, 44.0),
        (8.0, 44.5),
    ];
    let _ = write_3mf(
        &src.join("Deko").join("Schachturm.3mf"),
        "Schachturm",
        &[("Turm".into(), shape_lathe(0.0, 0.0, &tower, 48, 0, 0.0), [225, 225, 230])],
    );
    let lid: Vec<(f32, f32)> = vec![(30.0, 0.0), (30.0, 6.0), (27.0, 6.0), (27.0, 12.0), (8.0, 14.0)];
    let _ = write_stl(&src.join("Dosendeckel.stl"), &shape_lathe(0.0, 0.0, &lid, 64, 0, 0.0));
    let _ = write_stl(
        &src.join("Halterungen").join("Kabelclip.stl"),
        &shape_lathe(0.0, 0.0, &[(10.0, 0.0), (10.0, 6.0), (6.0, 6.0), (6.0, 18.0)], 6, 0, 0.0),
    );
    let _ = std::fs::write(
        src.join("Halterungen").join("Regalwinkel.step"),
        "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n",
    );
    let _ = std::fs::write(src.join("Zahnrad-Set_0.2mm_PLA.gcode"), "; Beispiel\nG28\nG1 Z0.2\n".repeat(400));

}

fn import_examples(lib: &mut Library, src: &std::path::Path) {
    let src = src.to_path_buf();
    let types: Vec<String> = library::ALL_TYPES.iter().map(|(t, _)| t.to_string()).collect();
    let found = library::scan(&src, true, &types, lib);
    let job = ImportJob {
        files: found,
        library_dir: lib.dir.clone(),
        known_hashes: lib.hashes(),
        first_id: 1,
        after: AfterImport::Keep,
        skip_duplicates: true,
        folder_as_tag: true,
    };
    let mut report = library::run_import(job, |_| {});
    let base = chrono::Local::now().timestamp();
    for (i, e) in report.entries.iter_mut().enumerate() {
        e.imported = base - (i as i64) * 3700;
        if e.name == "Spiralvase" || e.name == "Zahnrad-Set" {
            e.favorite = true;
        }
    }
    lib.add_entries(report.entries);
    let _ = std::fs::remove_dir_all(&src);
}

pub fn write_stl(path: &std::path::Path, m: &model3d::Mesh) -> std::io::Result<()> {
    let mut out = vec![0u8; 80];
    out.extend_from_slice(&(m.tris.len() as u32).to_le_bytes());
    for t in &m.tris {
        out.extend_from_slice(&[0u8; 12]);
        for &i in t {
            for v in m.positions[i as usize] {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&[0u8; 2]);
    }
    std::fs::write(path, out)
}
