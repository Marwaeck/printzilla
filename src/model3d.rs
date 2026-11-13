//! Loading 3D print files (3MF, STL, OBJ) into a simple triangle mesh.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

/// A flattened triangle mesh ready for drawing.
#[derive(Clone, Default)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub tris: Vec<[u32; 3]>,
    /// One colour per triangle.
    pub colors: Vec<[u8; 3]>,
}

impl Mesh {
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        if self.tris.is_empty() {
            return None;
        }
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for t in &self.tris {
            for &i in t {
                let p = self.positions[i as usize];
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
        Some((lo, hi))
    }
}

#[derive(Clone)]
pub struct ObjectInfo {
    pub name: String,
    pub triangles: usize,
    pub color: [u8; 3],
}

/// Everything we learned from a file.
#[derive(Clone, Default)]
pub struct Model {
    pub mesh: Mesh,
    pub objects: Vec<ObjectInfo>,
    pub metadata: Vec<(String, String)>,
    /// Embedded preview picture (RGBA, width, height).
    pub thumbnail: Option<(Vec<u8>, u32, u32)>,
    #[allow(dead_code)]
    pub unit: String,
}

pub const PALETTE: [[u8; 3]; 8] = [
    [235, 140, 52],
    [74, 158, 222],
    [120, 190, 90],
    [210, 90, 120],
    [170, 130, 220],
    [230, 200, 70],
    [80, 190, 180],
    [200, 200, 200],
];

pub fn can_preview(ext: &str) -> bool {
    matches!(ext, "3mf" | "stl" | "obj")
}

pub fn load(path: &Path) -> Result<Model, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "3mf" => load_3mf(path),
        "stl" => load_stl(path),
        "obj" => load_obj(path),
        _ => Err("Für diesen Dateityp gibt es keine 3D-Ansicht.".into()),
    }
}

// ---------------------------------------------------------------------------
// Tiny XML scanner (enough for 3MF model files, fast on huge meshes)

pub struct Tag<'a> {
    pub name: &'a str,
    pub attrs: Vec<(&'a str, &'a str)>,
    pub closing: bool,
    pub self_closing: bool,
    /// Text that follows this tag until the next '<'.
    pub text_after: &'a str,
}

impl<'a> Tag<'a> {
    pub fn local(&self) -> &'a str {
        match self.name.rfind(':') {
            Some(i) => &self.name[i + 1..],
            None => self.name,
        }
    }
    pub fn attr(&self, key: &str) -> Option<&'a str> {
        self.attrs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).or_else(|| {
            // match by local name (ignoring namespace prefix)
            self.attrs
                .iter()
                .find(|(k, _)| k.rsplit(':').next() == Some(key))
                .map(|(_, v)| *v)
        })
    }
}

pub fn scan_xml<'a>(src: &'a str, mut f: impl FnMut(&Tag<'a>)) {
    let b = src.as_bytes();
    let n = b.len();
    let mut i = 0;
    while i < n {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        // comments, processing instructions, CDATA, doctype
        if src[i..].starts_with("<!--") {
            i = src[i..].find("-->").map(|p| i + p + 3).unwrap_or(n);
            continue;
        }
        if i + 1 < n && (b[i + 1] == b'?' || b[i + 1] == b'!') {
            i = src[i..].find('>').map(|p| i + p + 1).unwrap_or(n);
            continue;
        }
        let mut j = i + 1;
        let closing = j < n && b[j] == b'/';
        if closing {
            j += 1;
        }
        let name_start = j;
        while j < n && !b[j].is_ascii_whitespace() && b[j] != b'>' && b[j] != b'/' {
            j += 1;
        }
        let name = &src[name_start..j];
        let mut attrs = Vec::new();
        let mut self_closing = false;
        loop {
            while j < n && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= n {
                break;
            }
            if b[j] == b'>' {
                j += 1;
                break;
            }
            if b[j] == b'/' {
                self_closing = true;
                j += 1;
                continue;
            }
            let ks = j;
            while j < n && b[j] != b'=' && !b[j].is_ascii_whitespace() && b[j] != b'>' && b[j] != b'/' {
                j += 1;
            }
            let key = &src[ks..j];
            while j < n && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < n && b[j] == b'=' {
                j += 1;
                while j < n && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < n && (b[j] == b'"' || b[j] == b'\'') {
                    let q = b[j];
                    j += 1;
                    let vs = j;
                    while j < n && b[j] != q {
                        j += 1;
                    }
                    attrs.push((key, &src[vs..j]));
                    j += 1;
                } else {
                    let vs = j;
                    while j < n && !b[j].is_ascii_whitespace() && b[j] != b'>' {
                        j += 1;
                    }
                    attrs.push((key, &src[vs..j]));
                }
            } else if !key.is_empty() {
                attrs.push((key, ""));
            } else {
                j += 1;
            }
        }
        let text_end = src[j..].find('<').map(|p| j + p).unwrap_or(n);
        let tag = Tag {
            name,
            attrs,
            closing,
            self_closing,
            text_after: &src[j..text_end],
        };
        f(&tag);
        i = text_end;
    }
}

pub fn xml_unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.trim().to_string();
    }
    s.trim()
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn pf(s: Option<&str>) -> f32 {
    s.and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0)
}

fn parse_color(s: &str) -> Option<[u8; 3]> {
    let s = s.trim().trim_start_matches('#');
    if s.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some([r, g, b])
}

// ---------------------------------------------------------------------------
// 3MF

/// 4x3 affine matrix in 3MF row-vector order.
type Mat = [f32; 12];
const IDENT: Mat = [1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.];

fn parse_mat(s: Option<&str>) -> Mat {
    let Some(s) = s else { return IDENT };
    let v: Vec<f32> = s.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    if v.len() == 12 {
        let mut m = [0.0; 12];
        m.copy_from_slice(&v);
        m
    } else {
        IDENT
    }
}

fn apply(m: &Mat, p: [f32; 3]) -> [f32; 3] {
    [
        p[0] * m[0] + p[1] * m[3] + p[2] * m[6] + m[9],
        p[0] * m[1] + p[1] * m[4] + p[2] * m[7] + m[10],
        p[0] * m[2] + p[1] * m[5] + p[2] * m[8] + m[11],
    ]
}

/// a then b
fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut r = [0.0; 12];
    for row in 0..3 {
        for col in 0..3 {
            r[row * 3 + col] = (0..3).map(|k| a[row * 3 + k] * b[k * 3 + col]).sum();
        }
    }
    for col in 0..3 {
        r[9 + col] = (0..3).map(|k| a[9 + k] * b[k * 3 + col]).sum::<f32>() + b[9 + col];
    }
    r
}

#[derive(Default)]
struct Obj3mf {
    name: String,
    pid: Option<String>,
    pindex: usize,
    verts: Vec<[f32; 3]>,
    tris: Vec<[u32; 3]>,
    /// per triangle (pid, index) override
    tri_props: Vec<Option<(Option<String>, usize)>>,
    components: Vec<(String, Mat, Option<String>)>,
}

#[derive(Default)]
struct ModelFile {
    objects: HashMap<String, Obj3mf>,
    build: Vec<(String, Mat, Option<String>)>,
    materials: HashMap<String, Vec<[u8; 3]>>,
    metadata: Vec<(String, String)>,
    unit: String,
}

fn parse_model_xml(src: &str) -> ModelFile {
    let mut mf = ModelFile {
        unit: "millimeter".into(),
        ..Default::default()
    };
    let mut cur: Option<(String, Obj3mf)> = None;
    let mut cur_mat: Option<String> = None;
    let mut in_build = false;
    let mut in_object_metadata = false;
    scan_xml(src, |t| {
        let local = t.local();
        if t.closing {
            match local {
                "object" => {
                    if let Some((id, o)) = cur.take() {
                        mf.objects.insert(id, o);
                    }
                }
                "basematerials" | "colorgroup" => cur_mat = None,
                "build" => in_build = false,
                "metadatagroup" => in_object_metadata = false,
                _ => {}
            }
            return;
        }
        match local {
            "model" => {
                if let Some(u) = t.attr("unit") {
                    mf.unit = u.to_string();
                }
            }
            "metadata" => {
                if cur.is_none() && !in_object_metadata {
                    if let Some(name) = t.attr("name") {
                        let v = xml_unescape(t.text_after);
                        if !v.is_empty() {
                            let name = name.rsplit(':').next().unwrap_or(name).to_string();
                            mf.metadata.push((name, v));
                        }
                    }
                }
            }
            "metadatagroup" => {
                if !t.self_closing {
                    in_object_metadata = true
                }
            }
            "basematerials" | "colorgroup" => {
                if let Some(id) = t.attr("id") {
                    mf.materials.insert(id.to_string(), Vec::new());
                    if !t.self_closing {
                        cur_mat = Some(id.to_string());
                    }
                }
            }
            "base" | "color" => {
                if let Some(id) = &cur_mat {
                    let c = t
                        .attr("displaycolor")
                        .or_else(|| t.attr("color"))
                        .and_then(parse_color)
                        .unwrap_or([200, 200, 200]);
                    if let Some(list) = mf.materials.get_mut(id) {
                        list.push(c);
                    }
                }
            }
            "object" => {
                let id = t.attr("id").unwrap_or("").to_string();
                let o = Obj3mf {
                    name: t.attr("name").map(xml_unescape).unwrap_or_default(),
                    pid: t.attr("pid").map(|s| s.to_string()),
                    pindex: t.attr("pindex").and_then(|s| s.parse().ok()).unwrap_or(0),
                    ..Default::default()
                };
                if t.self_closing {
                    mf.objects.insert(id, o);
                } else {
                    cur = Some((id, o));
                }
            }
            "vertex" => {
                if let Some((_, o)) = cur.as_mut() {
                    o.verts.push([pf(t.attr("x")), pf(t.attr("y")), pf(t.attr("z"))]);
                }
            }
            "triangle" => {
                if let Some((_, o)) = cur.as_mut() {
                    let g = |k: &str| t.attr(k).and_then(|s| s.trim().parse::<u32>().ok());
                    if let (Some(a), Some(b), Some(c)) = (g("v1"), g("v2"), g("v3")) {
                        o.tris.push([a, b, c]);
                        let pid = t.attr("pid").map(|s| s.to_string());
                        let p1 = t.attr("p1").and_then(|s| s.parse::<usize>().ok());
                        o.tri_props.push(if pid.is_some() || p1.is_some() {
                            Some((pid, p1.unwrap_or(0)))
                        } else {
                            None
                        });
                    }
                }
            }
            "component" => {
                if let Some((_, o)) = cur.as_mut() {
                    if let Some(oid) = t.attr("objectid") {
                        o.components.push((
                            oid.to_string(),
                            parse_mat(t.attr("transform")),
                            t.attr("path").map(|s| s.trim_start_matches('/').to_string()),
                        ));
                    }
                }
            }
            "build" => in_build = !t.self_closing,
            "item" => {
                if in_build {
                    if let Some(oid) = t.attr("objectid") {
                        mf.build.push((
                            oid.to_string(),
                            parse_mat(t.attr("transform")),
                            t.attr("path").map(|s| s.trim_start_matches('/').to_string()),
                        ));
                    }
                }
            }
            _ => {}
        }
    });
    mf
}

struct Flattener<'z> {
    zip: &'z mut zip::ZipArchive<std::fs::File>,
    files: HashMap<String, ModelFile>,
    mesh: Mesh,
    tri_budget: usize,
}

impl<'z> Flattener<'z> {
    fn ensure(&mut self, path: &str) {
        if self.files.contains_key(path) {
            return;
        }
        let mf = read_zip_string(self.zip, path)
            .map(|s| parse_model_xml(&s))
            .unwrap_or_default();
        self.files.insert(path.to_string(), mf);
    }

    /// Adds the object's triangles; returns how many were added.
    fn add(&mut self, path: &str, id: &str, m: &Mat, fallback: [u8; 3], depth: usize) -> usize {
        if depth > 16 || self.mesh.tris.len() >= self.tri_budget {
            return 0;
        }
        self.ensure(path);
        let (comps, base, color) = {
            let mf = &self.files[path];
            let Some(o) = mf.objects.get(id) else { return 0 };
            let obj_color = o
                .pid
                .as_ref()
                .and_then(|pid| mf.materials.get(pid))
                .and_then(|l| l.get(o.pindex))
                .copied()
                .unwrap_or(fallback);
            (o.components.clone(), self.mesh.positions.len() as u32, obj_color)
        };
        let mut added = 0;
        {
            let mf = &self.files[path];
            let o = &mf.objects[id];
            if !o.tris.is_empty() {
                for v in &o.verts {
                    self.mesh.positions.push(apply(m, *v));
                }
                let nv = o.verts.len() as u32;
                for (ti, t) in o.tris.iter().enumerate() {
                    if t[0] >= nv || t[1] >= nv || t[2] >= nv {
                        continue;
                    }
                    self.mesh.tris.push([t[0] + base, t[1] + base, t[2] + base]);
                    let c = match o.tri_props.get(ti).and_then(|p| p.as_ref()) {
                        Some((pid, idx)) => pid
                            .as_ref()
                            .or(o.pid.as_ref())
                            .and_then(|pid| mf.materials.get(pid))
                            .and_then(|l| l.get(*idx))
                            .copied()
                            .unwrap_or(color),
                        None => color,
                    };
                    self.mesh.colors.push(c);
                    added += 1;
                }
            }
        }
        for (cid, cm, cpath) in comps {
            let total = mul(&cm, m);
            let p = cpath.unwrap_or_else(|| path.to_string());
            added += self.add(&p, &cid, &total, color, depth + 1);
        }
        added
    }
}

fn read_zip_bytes(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Vec<u8>> {
    let idx = (0..zip.len()).find(|&i| {
        zip.name_for_index(i)
            .map(|n| n.trim_start_matches('/').eq_ignore_ascii_case(name))
            .unwrap_or(false)
    })?;
    let mut f = zip.by_index(idx).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn read_zip_string(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    read_zip_bytes(zip, name).map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn decode_image(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((img.into_raw(), w, h))
}

pub fn load_3mf(path: &Path) -> Result<Model, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Datei lässt sich nicht öffnen: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "Keine gültige 3MF-Datei (kein ZIP-Archiv).".to_string())?;

    let mut root = "3D/3dmodel.model".to_string();
    let mut thumb_path: Option<String> = None;
    if let Some(rels) = read_zip_string(&mut zip, "_rels/.rels") {
        scan_xml(&rels, |t| {
            if t.local() == "Relationship" {
                let ty = t.attr("Type").unwrap_or("");
                let target = t.attr("Target").unwrap_or("").trim_start_matches('/').to_string();
                if ty.ends_with("/3dmodel") && !target.is_empty() {
                    root = target;
                } else if ty.ends_with("/thumbnail") && !target.is_empty() {
                    thumb_path = Some(target);
                }
            }
        });
    }

    let mut thumbnail = None;
    let mut candidates: Vec<String> = thumb_path.into_iter().collect();
    candidates.extend(
        ["Metadata/thumbnail.png", "Metadata/plate_1.png", "Metadata/thumbnail.jpg"]
            .iter()
            .map(|s| s.to_string()),
    );
    for c in candidates {
        if let Some(bytes) = read_zip_bytes(&mut zip, &c) {
            if let Some(img) = decode_image(&bytes) {
                thumbnail = Some(img);
                break;
            }
        }
    }

    let mut fl = Flattener {
        zip: &mut zip,
        files: HashMap::new(),
        mesh: Mesh::default(),
        tri_budget: 6_000_000,
    };
    fl.ensure(&root);
    let build = fl.files[&root].build.clone();
    let metadata = fl.files[&root].metadata.clone();
    let unit = fl.files[&root].unit.clone();
    let scale = match unit.as_str() {
        "micron" => 0.001,
        "centimeter" => 10.0,
        "inch" => 25.4,
        "foot" => 304.8,
        "meter" => 1000.0,
        _ => 1.0,
    };
    let mut objects = Vec::new();
    let mut items = build.clone();
    if items.is_empty() {
        // no build section: show every object
        let mut ids: Vec<String> = fl.files[&root].objects.keys().cloned().collect();
        ids.sort();
        items = ids.into_iter().map(|id| (id, IDENT, None)).collect();
    }
    for (i, (oid, m, p)) in items.iter().enumerate() {
        let p = p.clone().unwrap_or_else(|| root.clone());
        let fallback = PALETTE[i % PALETTE.len()];
        let before = fl.mesh.tris.len();
        let n = fl.add(&p, oid, m, fallback, 0);
        let name = fl
            .files
            .get(&p)
            .and_then(|mf| mf.objects.get(oid))
            .map(|o| o.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("Objekt {}", i + 1));
        let color = fl.mesh.colors.get(before).copied().unwrap_or(fallback);
        objects.push(ObjectInfo { name, triangles: n, color });
    }
    let mut mesh = std::mem::take(&mut fl.mesh);
    if scale != 1.0 {
        for p in &mut mesh.positions {
            for v in p.iter_mut() {
                *v *= scale;
            }
        }
    }
    if mesh.tris.is_empty() && thumbnail.is_none() {
        return Err("In dieser 3MF-Datei wurde kein Modell gefunden.".into());
    }
    Ok(Model {
        mesh,
        objects,
        metadata,
        thumbnail,
        unit,
    })
}

// ---------------------------------------------------------------------------
// STL

pub fn load_stl(path: &Path) -> Result<Model, String> {
    let data = std::fs::read(path).map_err(|e| format!("Datei lässt sich nicht lesen: {e}"))?;
    let mut mesh = Mesh::default();
    let color = PALETTE[1];
    let is_binary = data.len() >= 84 && {
        let n = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        84 + n * 50 == data.len()
    };
    if is_binary {
        let n = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
        mesh.positions.reserve(n * 3);
        for i in 0..n {
            let o = 84 + i * 50 + 12;
            let base = mesh.positions.len() as u32;
            for v in 0..3 {
                let q = o + v * 12;
                let f = |k: usize| f32::from_le_bytes([data[q + k], data[q + k + 1], data[q + k + 2], data[q + k + 3]]);
                mesh.positions.push([f(0), f(4), f(8)]);
            }
            mesh.tris.push([base, base + 1, base + 2]);
            mesh.colors.push(color);
        }
    } else {
        let text = String::from_utf8_lossy(&data);
        let mut cur: Vec<[f32; 3]> = Vec::new();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            match it.next() {
                Some("vertex") => {
                    let v: Vec<f32> = it.filter_map(|x| x.parse().ok()).collect();
                    if v.len() >= 3 {
                        cur.push([v[0], v[1], v[2]]);
                    }
                }
                Some("endfacet") => {
                    if cur.len() >= 3 {
                        let base = mesh.positions.len() as u32;
                        mesh.positions.extend_from_slice(&cur[..3]);
                        mesh.tris.push([base, base + 1, base + 2]);
                        mesh.colors.push(color);
                    }
                    cur.clear();
                }
                _ => {}
            }
        }
    }
    if mesh.tris.is_empty() {
        return Err("In dieser STL-Datei wurden keine Dreiecke gefunden.".into());
    }
    let n = mesh.tris.len();
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    Ok(Model {
        mesh,
        objects: vec![ObjectInfo { name, triangles: n, color }],
        unit: "millimeter".into(),
        ..Default::default()
    })
}

// ---------------------------------------------------------------------------
// OBJ

pub fn load_obj(path: &Path) -> Result<Model, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("Datei lässt sich nicht lesen: {e}"))?;
    let mut mesh = Mesh::default();
    let mut objects: Vec<ObjectInfo> = Vec::new();
    let mut color = PALETTE[2];
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => {
                let v: Vec<f32> = it.take(3).filter_map(|x| x.parse().ok()).collect();
                if v.len() == 3 {
                    mesh.positions.push([v[0], v[1], v[2]]);
                }
            }
            Some("o") | Some("g") => {
                let name = it.collect::<Vec<_>>().join(" ");
                color = PALETTE[(objects.len() + 2) % PALETTE.len()];
                objects.push(ObjectInfo { name, triangles: 0, color });
            }
            Some("f") => {
                let nv = mesh.positions.len() as i64;
                let idx: Vec<u32> = it
                    .filter_map(|x| x.split('/').next()?.parse::<i64>().ok())
                    .map(|i| if i < 0 { nv + i } else { i - 1 })
                    .filter(|&i| i >= 0 && i < nv)
                    .map(|i| i as u32)
                    .collect();
                for k in 1..idx.len().saturating_sub(1) {
                    mesh.tris.push([idx[0], idx[k], idx[k + 1]]);
                    mesh.colors.push(color);
                    if let Some(o) = objects.last_mut() {
                        o.triangles += 1;
                    }
                }
            }
            _ => {}
        }
    }
    if mesh.tris.is_empty() {
        return Err("In dieser OBJ-Datei wurden keine Flächen gefunden.".into());
    }
    objects.retain(|o| o.triangles > 0);
    if objects.is_empty() {
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        objects.push(ObjectInfo { name, triangles: mesh.tris.len(), color });
    }
    // OBJ files are usually Y-up; turn them so they stand on the plate (Z-up).
    for p in &mut mesh.positions {
        *p = [p[0], -p[2], p[1]];
    }
    Ok(Model {
        mesh,
        objects,
        unit: "millimeter".into(),
        ..Default::default()
    })
}

// ---------------------------------------------------------------------------
// Writing a simple 3MF (used to create the example files)

pub fn write_3mf(path: &Path, title: &str, parts: &[(String, Mesh, [u8; 3])]) -> std::io::Result<()> {
    use std::fmt::Write as _;
    use std::io::Write;
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"de-DE\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n");
    let _ = writeln!(xml, " <metadata name=\"Title\">{title}</metadata>");
    xml.push_str(" <metadata name=\"Designer\">PrintZilla</metadata>\n <metadata name=\"Application\">PrintZilla</metadata>\n <resources>\n  <basematerials id=\"1\">\n");
    for (name, _, c) in parts {
        let _ = writeln!(xml, "   <base name=\"{name}\" displaycolor=\"#{:02X}{:02X}{:02X}\"/>", c[0], c[1], c[2]);
    }
    xml.push_str("  </basematerials>\n");
    for (i, (name, mesh, _)) in parts.iter().enumerate() {
        let _ = writeln!(xml, "  <object id=\"{}\" name=\"{name}\" type=\"model\" pid=\"1\" pindex=\"{i}\">\n   <mesh>\n    <vertices>", i + 2);
        for p in &mesh.positions {
            let _ = writeln!(xml, "     <vertex x=\"{:.4}\" y=\"{:.4}\" z=\"{:.4}\"/>", p[0], p[1], p[2]);
        }
        xml.push_str("    </vertices>\n    <triangles>\n");
        for t in &mesh.tris {
            let _ = writeln!(xml, "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>", t[0], t[1], t[2]);
        }
        xml.push_str("    </triangles>\n   </mesh>\n  </object>\n");
    }
    xml.push_str(" </resources>\n <build>\n");
    for i in 0..parts.len() {
        let _ = writeln!(xml, "  <item objectid=\"{}\"/>", i + 2);
    }
    xml.push_str(" </build>\n</model>\n");

    let file = std::fs::File::create(path)?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zw.start_file("[Content_Types].xml", opts)?;
    zw.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/></Types>"#)?;
    zw.start_file("_rels/.rels", opts)?;
    zw.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>"#)?;
    zw.start_file("3D/3dmodel.model", opts)?;
    zw.write_all(xml.as_bytes())?;
    zw.finish().map_err(std::io::Error::other)?;
    Ok(())
}

// Procedural shapes for the example files ----------------------------------

pub fn shape_box(x0: f32, y0: f32, z0: f32, sx: f32, sy: f32, sz: f32) -> Mesh {
    let p = |i: u32| {
        [
            x0 + if i & 1 != 0 { sx } else { 0.0 },
            y0 + if i & 2 != 0 { sy } else { 0.0 },
            z0 + if i & 4 != 0 { sz } else { 0.0 },
        ]
    };
    let positions = (0..8).map(p).collect();
    let tris = vec![
        [0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4],
        [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5],
    ];
    Mesh { positions, tris, colors: vec![] }
}

/// Surface of revolution around Z: profile of (radius, z) points, closed at both ends.
pub fn shape_lathe(cx: f32, cy: f32, profile: &[(f32, f32)], segs: u32, twist_lobes: u32, lobe_amp: f32) -> Mesh {
    let mut m = Mesh::default();
    let rings = profile.len() as u32;
    for (ri, &(r, z)) in profile.iter().enumerate() {
        for s in 0..segs {
            let a = s as f32 / segs as f32 * std::f32::consts::TAU;
            let tw = ri as f32 * 0.06;
            let rr = r * (1.0 + lobe_amp * ((a + tw) * twist_lobes as f32).cos());
            m.positions.push([cx + rr * a.cos(), cy + rr * a.sin(), z]);
        }
    }
    for ri in 0..rings - 1 {
        for s in 0..segs {
            let a = ri * segs + s;
            let b = ri * segs + (s + 1) % segs;
            let c = a + segs;
            let d = b + segs;
            m.tris.push([a, b, d]);
            m.tris.push([a, d, c]);
        }
    }
    let bottom = m.positions.len() as u32;
    m.positions.push([cx, cy, profile[0].1]);
    let top = bottom + 1;
    m.positions.push([cx, cy, profile[profile.len() - 1].1]);
    for s in 0..segs {
        m.tris.push([bottom, (s + 1) % segs, s]);
        let o = (rings - 1) * segs;
        m.tris.push([top, o + s, o + (s + 1) % segs]);
    }
    m
}

pub fn shape_gear(cx: f32, cy: f32, r: f32, h: f32, teeth: u32) -> Mesh {
    let segs = teeth * 4;
    let mut prof = Vec::new();
    for s in 0..segs {
        let rr = if (s / 2) % 2 == 0 { r } else { r * 0.82 };
        prof.push(rr);
    }
    let mut m = Mesh::default();
    for &z in &[0.0, h] {
        for (s, rr) in prof.iter().enumerate() {
            let a = s as f32 / segs as f32 * std::f32::consts::TAU;
            m.positions.push([cx + rr * a.cos(), cy + rr * a.sin(), z]);
        }
    }
    for s in 0..segs {
        let a = s;
        let b = (s + 1) % segs;
        m.tris.push([a, b, b + segs]);
        m.tris.push([a, b + segs, a + segs]);
    }
    let c0 = m.positions.len() as u32;
    m.positions.push([cx, cy, 0.0]);
    m.positions.push([cx, cy, h]);
    for s in 0..segs {
        m.tris.push([c0, (s + 1) % segs, s]);
        m.tris.push([c0 + 1, s + segs, (s + 1) % segs + segs]);
    }
    m
}
