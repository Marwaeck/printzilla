//! What the slicer wrote into a file: print time, filament, material, nozzle, printer …
//! Read from G-Code (text and Prusa's binary format) and from sliced 3MF projects
//! (Bambu Studio, OrcaSlicer, PrusaSlicer).

use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct PrintInfo {
    /// Estimated print time in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_s: Option<u64>,
    /// Filament in grams.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filament_g: Option<f32>,
    /// True when the grams were worked out from the length (no weight in the file).
    #[serde(default, skip_serializing_if = "is_false")]
    pub filament_g_estimated: bool,
    /// Filament in metres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filament_m: Option<f32>,
    /// Materials, e.g. ["PLA", "PETG"].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<String>,
    /// Filament colours as "#RRGGBB".
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nozzle_mm: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_mm: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub printer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slicer: Option<String>,
    /// Number of build plates (3MF projects).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plates: Option<usize>,
    /// True when the time/filament come from a sliced file (not just settings).
    #[serde(default, skip_serializing_if = "is_false")]
    pub sliced: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl PrintInfo {
    pub fn is_empty(&self) -> bool {
        *self == PrintInfo::default()
    }

    /// Short line for the cards, e.g. "4 h 12 min · 38 g".
    pub fn short(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(t) = self.time_s {
            parts.push(format_duration(t));
        }
        if let Some(g) = self.filament_g {
            parts.push(format!("{}{:.0} g", if self.filament_g_estimated { "≈ " } else { "" }, g));
        } else if let Some(m) = self.filament_m {
            parts.push(format!("{m:.1} m"));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }

    /// Fills gaps from another source (values already set win).
    fn merge(&mut self, o: PrintInfo) {
        self.time_s = self.time_s.or(o.time_s);
        if self.filament_g.is_none() {
            self.filament_g = o.filament_g;
            self.filament_g_estimated = o.filament_g_estimated;
        }
        self.filament_m = self.filament_m.or(o.filament_m);
        if self.materials.is_empty() {
            self.materials = o.materials;
        }
        if self.colors.is_empty() {
            self.colors = o.colors;
        }
        self.nozzle_mm = self.nozzle_mm.or(o.nozzle_mm);
        self.layer_mm = self.layer_mm.or(o.layer_mm);
        self.printer = self.printer.take().or(o.printer);
        self.slicer = self.slicer.take().or(o.slicer);
        self.plates = self.plates.or(o.plates);
        self.sliced |= o.sliced;
    }

    /// Without a weight in the file: grams from the length (1.75 mm, density of PLA).
    fn estimate_grams(&mut self) {
        if self.filament_g.is_none() {
            if let Some(m) = self.filament_m.filter(|m| *m > 0.0) {
                let r_cm = 0.175 / 2.0;
                let vol_cm3 = std::f32::consts::PI * r_cm * r_cm * m * 100.0;
                self.filament_g = Some(vol_cm3 * 1.24);
                self.filament_g_estimated = true;
            }
        }
    }
}

pub fn format_duration(s: u64) -> String {
    let d = s / 86_400;
    let h = (s % 86_400) / 3600;
    let m = (s % 3600) / 60;
    if d > 0 {
        format!("{d} d {h} h")
    } else if h > 0 {
        format!("{h} h {m} min")
    } else if m > 0 {
        format!("{m} min")
    } else {
        format!("{s} s")
    }
}

// ---------------------------------------------------------------------------
// Small helpers

/// "1d 2h 3m 4s", "2h 3m", "45m 10s", "1:02:03" or plain seconds.
pub fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<f64>() {
        return (n >= 0.0).then_some(n as u64);
    }
    if s.contains(':') && s.chars().all(|c| c.is_ascii_digit() || c == ':') {
        let mut total = 0u64;
        for p in s.split(':') {
            total = total * 60 + p.parse::<u64>().ok()?;
        }
        return Some(total);
    }
    let mut total = 0f64;
    let mut num = String::new();
    let mut any = false;
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
        } else if c.is_alphabetic() {
            if num.is_empty() {
                continue;
            }
            let n: f64 = num.parse().ok()?;
            num.clear();
            total += n * match c.to_ascii_lowercase() {
                'd' => 86_400.0,
                'h' => 3600.0,
                'm' => 60.0,
                's' => 1.0,
                _ => return None,
            };
            any = true;
        }
    }
    any.then_some(total as u64)
}

/// Sums a list like "12.3, 4.5" or "12.3;4.5" (one value per extruder).
fn sum_list(s: &str) -> Option<f32> {
    let mut total = 0f32;
    let mut any = false;
    for p in s.split([',', ';']) {
        if let Ok(v) = p.trim().parse::<f32>() {
            total += v;
            any = true;
        }
    }
    any.then_some(total)
}

fn first_num(s: &str) -> Option<f32> {
    s.split([',', ';']).next().and_then(|p| p.trim().trim_matches('"').parse().ok())
}

fn list(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in s.split([',', ';']) {
        let p = p.trim().trim_matches('"').to_string();
        if !p.is_empty() && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

fn colors(s: &str) -> Vec<String> {
    s.split([',', ';'])
        .map(|p| p.trim().trim_matches('"').to_uppercase())
        .filter(|p| p.starts_with('#') && (p.len() == 7 || p.len() == 9))
        .map(|p| p[..7].to_string())
        .fold(Vec::new(), |mut v, c| {
            if !v.contains(&c) {
                v.push(c);
            }
            v
        })
}

fn clean(s: &str) -> Option<String> {
    let s = s.trim().trim_matches('"').trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// Applies one "key = value" setting (G-Code comments, PrusaSlicer configs, binary G-Code).
fn apply_setting(info: &mut PrintInfo, key: &str, val: &str) {
    let k = key.trim().to_lowercase();
    let v = val.trim();
    match k.as_str() {
        "estimated printing time (normal mode)" | "estimated printing time" | "total estimated time" | "print time" | "time" => {
            if info.time_s.is_none() {
                info.time_s = parse_duration(v);
            }
            info.sliced = true;
        }
        "filament used [g]" | "total filament used [g]" | "total filament weight [g]" | "filament weight" => {
            if let Some(g) = sum_list(v) {
                // "total" lines come last in PrusaSlicer files and win
                if k.starts_with("total") || info.filament_g.is_none() {
                    info.filament_g = Some(g);
                    info.filament_g_estimated = false;
                }
            }
            info.sliced = true;
        }
        "filament used [mm]" | "total filament length [mm]" | "total filament used [mm]" => {
            if let Some(mm) = sum_list(v) {
                if k.starts_with("total") || info.filament_m.is_none() {
                    info.filament_m = Some(mm / 1000.0);
                }
            }
            info.sliced = true;
        }
        "filament used" => {
            // Cura: "1.23456m"
            if let Some(m) = sum_list(&v.replace('m', "")) {
                info.filament_m = Some(m);
            }
            info.sliced = true;
        }
        "filament_type" | "filament type" => {
            if info.materials.is_empty() {
                info.materials = list(v);
            }
        }
        "filament_colour" | "filament_color" | "extruder_colour" => {
            if info.colors.is_empty() {
                info.colors = colors(v);
            }
        }
        "nozzle_diameter" => info.nozzle_mm = info.nozzle_mm.or_else(|| first_num(v)),
        "layer_height" | "layer height" => info.layer_mm = info.layer_mm.or_else(|| first_num(v)),
        "printer_model" | "printer_settings_id" | "target_machine.name" | "machine_name" => {
            if info.printer.is_none() {
                info.printer = clean(v);
            }
        }
        _ => {}
    }
}

/// Finds out which program made the file from a "generated by …" style line.
fn detect_slicer(line: &str) -> Option<String> {
    let l = line.trim_start_matches(';').trim();
    let low = l.to_lowercase();
    for (needle, name) in [
        ("bambustudio", "Bambu Studio"),
        ("bambu studio", "Bambu Studio"),
        ("orcaslicer", "OrcaSlicer"),
        ("prusaslicer", "PrusaSlicer"),
        ("superslicer", "SuperSlicer"),
        ("cura_steamengine", "Cura"),
        ("ultimaker cura", "Cura"),
        ("simplify3d", "Simplify3D"),
        ("ideamaker", "ideaMaker"),
        ("slic3r", "Slic3r"),
    ] {
        if let Some(pos) = low.find(needle) {
            // keep a version number if one follows
            let rest = &l[pos + needle.len()..];
            let ver: String = rest
                .trim_start_matches(|c: char| c == ' ' || c == '-' || c == '_')
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let ver = ver.trim_start_matches('0');
            let ver = if ver.starts_with('.') { format!("0{ver}") } else { ver.to_string() };
            return Some(if ver.trim_matches('.').is_empty() { name.to_string() } else { format!("{name} {ver}") });
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Text G-Code

/// Reads the settings from G-Code comments.
pub fn parse_gcode_text(text: &str) -> PrintInfo {
    let mut info = PrintInfo::default();
    for raw in text.lines() {
        let line = raw.trim();
        if !line.starts_with(';') {
            continue;
        }
        let body = line.trim_start_matches(';').trim();
        if info.slicer.is_none() {
            let low = body.to_lowercase();
            if low.starts_with("generated") || low.starts_with("bambustudio") || low.starts_with("orcaslicer") || low.contains("slicer") || low.contains("cura") {
                info.slicer = detect_slicer(body);
            }
        }
        // Bambu: "model printing time: 1h 2m 3s; total estimated time: 1h 10m 3s"
        for part in body.split("; ") {
            let part = part.trim().trim_start_matches(';').trim();
            let sep = match (part.find(" = "), part.find(':'), part.find('=')) {
                (Some(i), _, _) => Some((i, 3)),
                (None, Some(c), Some(e)) => Some(if c < e { (c, 1) } else { (e, 1) }),
                (None, Some(c), None) => Some((c, 1)),
                (None, None, Some(e)) => Some((e, 1)),
                _ => None,
            };
            if let Some((i, n)) = sep {
                apply_setting(&mut info, &part[..i], &part[i + n..]);
            }
        }
    }
    info.estimate_grams();
    info
}

/// Pulls the first good preview picture out of "; thumbnail begin WxH len" blocks.
pub fn gcode_thumbnail(text: &str) -> Option<(Vec<u8>, u32, u32)> {
    let mut best: Option<(Vec<u8>, u32, u32)> = None;
    let mut data = String::new();
    let mut inside = false;
    for raw in text.lines() {
        let l = raw.trim().trim_start_matches(';').trim();
        let low = l.to_lowercase();
        if (low.starts_with("thumbnail begin") || low.starts_with("thumbnail_png begin") || low.starts_with("thumbnail_jpg begin")) && !inside {
            inside = true;
            data.clear();
        } else if inside && (low.starts_with("thumbnail end") || low.starts_with("thumbnail_png end") || low.starts_with("thumbnail_jpg end")) {
            inside = false;
            if let Some(bytes) = base64_decode(&data) {
                if let Ok(img) = image::load_from_memory(&bytes) {
                    let img = img.to_rgba8();
                    let (w, h) = img.dimensions();
                    // keep the biggest picture
                    if best.as_ref().map(|b| b.1 * b.2 < w * h).unwrap_or(true) {
                        best = Some((img.into_raw(), w, h));
                    }
                }
            }
        } else if inside {
            data.push_str(l);
        }
    }
    best
}

pub fn base64_encode(d: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(d.len() * 4 / 3 + 4);
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            s.push(if i <= c.len() { T[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    s
}

pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        })
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        acc = (acc << 6) | val(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Reads the start and the end of a G-Code file (settings sit there, not in between).
fn read_head_tail(path: &Path) -> std::io::Result<String> {
    const PART: u64 = 1_500_000;
    let mut f = std::fs::File::open(path)?;
    let len = f.metadata()?.len();
    let mut buf = Vec::new();
    if len <= PART * 2 {
        f.read_to_end(&mut buf)?;
    } else {
        let mut head = vec![0u8; PART as usize];
        f.read_exact(&mut head)?;
        f.seek(SeekFrom::Start(len - PART))?;
        let mut tail = Vec::new();
        f.read_to_end(&mut tail)?;
        buf = head;
        buf.push(b'\n');
        buf.extend(tail);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Print facts and preview picture of a G-Code file (text or binary).
pub fn load_gcode(path: &Path) -> Result<(PrintInfo, Option<(Vec<u8>, u32, u32)>), String> {
    let mut magic = [0u8; 4];
    {
        let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let _ = f.read(&mut magic);
    }
    if &magic == b"GCDE" {
        let data = std::fs::read(path).map_err(|e| e.to_string())?;
        return parse_bgcode(&data);
    }
    let text = read_head_tail(path).map_err(|e| e.to_string())?;
    Ok((parse_gcode_text(&text), gcode_thumbnail(&text)))
}

// ---------------------------------------------------------------------------
// Binary G-Code (PrusaSlicer .bgcode)

fn u16le(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}
fn u32le(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub fn parse_bgcode(d: &[u8]) -> Result<(PrintInfo, Option<(Vec<u8>, u32, u32)>), String> {
    if d.get(0..4) != Some(b"GCDE") {
        return Err("Kein binärer G-Code".into());
    }
    let checksum = u16le(d, 8).ok_or("Datei zu kurz")?;
    let crc_len = if checksum == 1 { 4 } else { 0 };
    let mut pos = 10;
    let mut info = PrintInfo::default();
    let mut thumb: Option<(Vec<u8>, u32, u32)> = None;
    while pos + 8 <= d.len() {
        let ty = u16le(d, pos).unwrap_or(0);
        let comp = u16le(d, pos + 2).unwrap_or(0);
        let usize_ = u32le(d, pos + 4).unwrap_or(0) as usize;
        let mut hdr = 8;
        let csize = if comp != 0 {
            hdr = 12;
            u32le(d, pos + 8).ok_or("Datei beschädigt")? as usize
        } else {
            usize_
        };
        let params = if ty == 5 { 6 } else { 2 };
        let data_at = pos + hdr + params;
        let end = data_at + csize;
        if end > d.len() {
            break;
        }
        if ty == 1 {
            // G-Code itself: all metadata blocks come before it
            break;
        }
        let raw = &d[data_at..end];
        let bytes: Option<Vec<u8>> = match comp {
            0 => Some(raw.to_vec()),
            1 => {
                let mut out = Vec::with_capacity(usize_);
                flate2::read::ZlibDecoder::new(raw).read_to_end(&mut out).ok().map(|_| out).or_else(|| {
                    let mut out = Vec::new();
                    flate2::read::DeflateDecoder::new(raw).read_to_end(&mut out).ok().map(|_| out)
                })
            }
            _ => None, // heatshrink: not supported
        };
        if let Some(bytes) = bytes {
            match ty {
                0 | 2 | 3 | 4 => {
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.lines() {
                        if let Some((k, v)) = line.split_once('=') {
                            apply_setting(&mut info, k, v);
                            if k.trim() == "Producer" {
                                info.slicer = detect_slicer(v).or_else(|| clean(v));
                            }
                        }
                    }
                }
                5 => {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let img = img.to_rgba8();
                        let (w, h) = img.dimensions();
                        if thumb.as_ref().map(|b| b.1 * b.2 < w * h).unwrap_or(true) {
                            thumb = Some((img.into_raw(), w, h));
                        }
                    }
                }
                _ => {}
            }
        }
        pos = end + crc_len;
    }
    info.estimate_grams();
    Ok((info, thumb))
}

// ---------------------------------------------------------------------------
// 3MF projects

/// Slicer facts from a 3MF archive (Bambu/Orca: slice_info + project_settings,
/// PrusaSlicer: Slic3r_PE.config).
pub fn from_3mf(zip: &mut zip::ZipArchive<std::fs::File>) -> Option<PrintInfo> {
    let names: Vec<String> = zip.file_names().map(|s| s.trim_start_matches('/').to_string()).collect();
    let find = |want: &str| names.iter().find(|n| n.eq_ignore_ascii_case(want)).cloned();
    let read = |zip: &mut zip::ZipArchive<std::fs::File>, name: &str| -> Option<String> {
        let mut f = zip.by_name(name).ok()?;
        let mut s = String::new();
        f.read_to_string(&mut s).ok()?;
        Some(s)
    };
    let mut info = PrintInfo::default();

    // results of slicing (only in files saved after slicing, or .gcode.3mf)
    if let Some(n) = find("Metadata/slice_info.config") {
        if let Some(xml) = read(zip, &n) {
            info.merge(parse_slice_info(&xml));
        }
    }
    // settings of the project
    if let Some(n) = find("Metadata/project_settings.config") {
        if let Some(text) = read(zip, &n) {
            info.merge(parse_project_settings(&text));
        }
    }
    for cfg in ["Metadata/Slic3r_PE.config", "Metadata/Prusa_Slicer.config"] {
        if let Some(n) = find(cfg) {
            if let Some(text) = read(zip, &n) {
                let mut p = parse_gcode_text(&text);
                p.sliced = false;
                p.time_s = None;
                info.merge(p);
            }
        }
    }
    if info.slicer.is_none() {
        if let Some(n) = find("3D/3dmodel.model") {
            if let Ok(f) = zip.by_name(&n) {
                // the application line sits in the first few hundred bytes
                let mut head = vec![0u8; 4096];
                let k = f.take(4096).read(&mut head).unwrap_or(0);
                let s = String::from_utf8_lossy(&head[..k]);
                const NEEDLE: &str = "name=\"Application\">";
                if let Some(i) = s.find(NEEDLE) {
                    let rest = &s[i + NEEDLE.len()..];
                    if let Some(e) = rest.find('<') {
                        info.slicer = detect_slicer(&rest[..e]);
                    }
                }
            }
        }
    }
    // plates: one preview picture per plate in Bambu/Orca projects
    let plates = names
        .iter()
        .filter(|n| {
            let l = n.to_lowercase();
            l.starts_with("metadata/plate_") && l.ends_with(".png") && !l.contains("small") && !l.contains("no_light") && !l.contains("top") && !l.contains("pick")
        })
        .count();
    let plates = plates.max(info.plates.unwrap_or(0));
    info.plates = (plates > 1).then_some(plates);
    info.estimate_grams();
    (!info.is_empty()).then_some(info)
}

pub fn parse_slice_info(xml: &str) -> PrintInfo {
    let mut info = PrintInfo::default();
    let mut time = 0u64;
    let mut grams = 0f32;
    let mut metres = 0f32;
    let mut plates = 0usize;
    let mut any_time = false;
    let mut any_g = false;
    let mut in_header = false;
    crate::model3d::scan_xml(xml, |t| {
        match t.local() {
            "header" => in_header = !t.closing,
            "plate" if !t.closing => plates += 1,
            "header_item" => {
                if t.attr("key") == Some("X-BBL-Client-Version") {
                    if let Some(v) = t.attr("value") {
                        info.slicer = Some(format!("Bambu Studio {}", v.trim_start_matches('0')));
                    }
                }
            }
            "metadata" if !in_header => {
                let k = t.attr("key").unwrap_or("");
                let v = t.attr("value").unwrap_or("");
                match k {
                    "prediction" => {
                        if let Ok(n) = v.trim().parse::<f64>() {
                            time += n as u64;
                            any_time = true;
                        }
                    }
                    "weight" => {
                        if let Ok(n) = v.trim().parse::<f32>() {
                            grams += n;
                            any_g = true;
                        }
                    }
                    "printer_model_id" => {
                        if info.printer.is_none() && !v.is_empty() {
                            info.printer = Some(bambu_printer(v));
                        }
                    }
                    "nozzle_diameters" => info.nozzle_mm = info.nozzle_mm.or_else(|| first_num(v)),
                    _ => {}
                }
            }
            "filament" if !t.closing => {
                if let Some(ty) = t.attr("type").and_then(clean) {
                    if !info.materials.contains(&ty) {
                        info.materials.push(ty);
                    }
                }
                if let Some(c) = t.attr("color") {
                    for c in colors(c) {
                        if !info.colors.contains(&c) {
                            info.colors.push(c);
                        }
                    }
                }
                if let Some(m) = t.attr("used_m").and_then(|v| v.trim().parse::<f32>().ok()) {
                    metres += m;
                }
            }
            _ => {}
        }
    });
    if any_time {
        info.time_s = Some(time);
        info.sliced = true;
    }
    if any_g && grams > 0.0 {
        info.filament_g = Some(grams);
        info.sliced = true;
    }
    if metres > 0.0 {
        info.filament_m = Some(metres);
    }
    if plates > 0 && any_time {
        info.plates = Some(plates);
    }
    info
}

/// Bambu printer codes to names.
fn bambu_printer(code: &str) -> String {
    match code {
        "BL-P001" => "Bambu Lab X1 Carbon",
        "BL-P002" => "Bambu Lab X1",
        "C11" => "Bambu Lab P1P",
        "C12" => "Bambu Lab P1S",
        "C13" => "Bambu Lab X1E",
        "N1" => "Bambu Lab A1 mini",
        "N2S" => "Bambu Lab A1",
        "O1D" => "Bambu Lab H2D",
        "N7" => "Bambu Lab P2S",
        other => return other.to_string(),
    }
    .to_string()
}

/// project_settings.config is JSON with string or list values.
pub fn parse_project_settings(text: &str) -> PrintInfo {
    let mut info = PrintInfo::default();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
        // older PrusaSlicer-style "key = value" lines
        let mut p = parse_gcode_text(text);
        p.time_s = None;
        p.sliced = false;
        return p;
    };
    let get = |k: &str| -> Option<String> {
        match v.get(k)? {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Array(a) => Some(a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(";")),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    };
    for k in ["filament_type", "filament_colour", "nozzle_diameter", "layer_height", "printer_model"] {
        if let Some(val) = get(k) {
            apply_setting(&mut info, k, &val);
        }
    }
    if info.printer.is_none() {
        if let Some(p) = get("printer_settings_id").and_then(|s| clean(&s)) {
            info.printer = Some(p);
        }
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(4, 3, image::Rgba([200, 100, 50, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("1d 2h 3m 4s"), Some(93784));
        assert_eq!(parse_duration("2h 3m"), Some(7380));
        assert_eq!(parse_duration("12345"), Some(12345));
        assert_eq!(parse_duration("1:02:03"), Some(3723));
        assert_eq!(format_duration(4 * 3600 + 12 * 60), "4 h 12 min");
        assert_eq!(format_duration(90_000), "1 d 1 h");
    }

    #[test]
    fn prusaslicer_gcode() {
        let png = base64_encode(&tiny_png());
        assert_eq!(base64_decode(&png).unwrap(), tiny_png());
        let text = format!(
            "; generated by PrusaSlicer 2.7.1+win64 on 2024-01-01 at 10:00:00 UTC\n\
             ;\n; thumbnail begin 4x3 {}\n; {}\n; thumbnail end\n\
             G1 X10 Y10\n\
             ; filament used [mm] = 5000.12\n; filament used [g] = 14.91\n\
             ; estimated printing time (normal mode) = 1h 23m 45s\n\
             ; filament_type = PETG\n; filament_colour = #FF8000\n; nozzle_diameter = 0.4\n; layer_height = 0.2\n\
             ; printer_model = MK4\n",
            png.len(),
            png
        );
        let info = parse_gcode_text(&text);
        assert_eq!(info.time_s, Some(5025));
        assert_eq!(info.filament_g, Some(14.91));
        assert!((info.filament_m.unwrap() - 5.0).abs() < 0.01);
        assert_eq!(info.materials, vec!["PETG"]);
        assert_eq!(info.colors, vec!["#FF8000"]);
        assert_eq!(info.nozzle_mm, Some(0.4));
        assert_eq!(info.layer_mm, Some(0.2));
        assert_eq!(info.printer.as_deref(), Some("MK4"));
        assert_eq!(info.slicer.as_deref(), Some("PrusaSlicer 2.7.1"));
        assert_eq!(info.short().as_deref(), Some("1 h 23 min · 15 g"));
        let (_, w, h) = gcode_thumbnail(&text).expect("thumbnail");
        assert_eq!((w, h), (4, 3));
    }

    #[test]
    fn bambu_and_orca_gcode() {
        let text = "; HEADER_BLOCK_START\n; BambuStudio 01.09.07.52\n; model printing time: 1h 2m 3s; total estimated time: 1h 10m 3s\n\
                    ; total filament length [mm] : 1234.5\n; total filament weight [g] : 3.71\n; HEADER_BLOCK_END\n\
                    ; filament_type = PLA;PLA;PETG\n; filament_colour = #FFFFFF;#000000;#FFFFFF\n; nozzle_diameter = 0.4\n";
        let info = parse_gcode_text(text);
        assert_eq!(info.slicer.as_deref(), Some("Bambu Studio 1.09.07.52"));
        assert_eq!(info.time_s, Some(4203));
        assert_eq!(info.filament_g, Some(3.71));
        assert_eq!(info.materials, vec!["PLA", "PETG"]);
        assert_eq!(info.colors, vec!["#FFFFFF", "#000000"]);
    }

    #[test]
    fn cura_gcode() {
        let text = ";FLAVOR:Marlin\n;TIME:6000\n;Filament used: 2.5m\n;Layer height: 0.12\n;Generated with Cura_SteamEngine 5.4.0\n;LAYER:0\n";
        let info = parse_gcode_text(text);
        assert_eq!(info.time_s, Some(6000));
        assert_eq!(info.filament_m, Some(2.5));
        assert!(info.filament_g_estimated);
        assert!((info.filament_g.unwrap() - 7.46).abs() < 0.1);
        assert_eq!(info.layer_mm, Some(0.12));
        assert_eq!(info.slicer.as_deref(), Some("Cura 5.4.0"));
    }

    #[test]
    fn binary_gcode() {
        fn block(out: &mut Vec<u8>, ty: u16, params: &[u8], data: &[u8]) {
            out.extend(ty.to_le_bytes());
            out.extend(0u16.to_le_bytes());
            out.extend((data.len() as u32).to_le_bytes());
            out.extend(params);
            out.extend(data);
            out.extend([0u8; 4]); // CRC (not checked)
        }
        let mut d = b"GCDE".to_vec();
        d.extend(1u32.to_le_bytes());
        d.extend(1u16.to_le_bytes());
        block(&mut d, 0, &[0, 0], b"Producer=PrusaSlicer 2.8.0\n");
        block(&mut d, 3, &[0, 0], b"printer_model=MK4S\nfilament_type=PLA\nnozzle_diameter=0.4\n");
        let png = tiny_png();
        block(&mut d, 5, &[0, 0, 4, 0, 3, 0], &png);
        block(&mut d, 4, &[0, 0], b"filament used [g]=12.5\nestimated printing time (normal mode)=2h 5m 0s\n");
        block(&mut d, 1, &[0, 0], b"G1 X1\n");
        let (info, thumb) = parse_bgcode(&d).unwrap();
        assert_eq!(info.slicer.as_deref(), Some("PrusaSlicer 2.8.0"));
        assert_eq!(info.printer.as_deref(), Some("MK4S"));
        assert_eq!(info.filament_g, Some(12.5));
        assert_eq!(info.time_s, Some(7500));
        assert_eq!(info.materials, vec!["PLA"]);
        assert_eq!(thumb.map(|t| (t.1, t.2)), Some((4, 3)));
    }

    #[test]
    fn bambu_3mf() {
        let slice = r##"<?xml version="1.0" encoding="UTF-8"?>
<config>
  <header>
    <header_item key="X-BBL-Client-Type" value="slicer"/>
    <header_item key="X-BBL-Client-Version" value="01.10.01.50"/>
  </header>
  <plate>
    <metadata key="index" value="1"/>
    <metadata key="printer_model_id" value="C12"/>
    <metadata key="nozzle_diameters" value="0.4"/>
    <metadata key="prediction" value="3600"/>
    <metadata key="weight" value="20.50"/>
    <filament id="1" type="PLA" color="#FF0000" used_m="6.5" used_g="20.5"/>
  </plate>
  <plate>
    <metadata key="index" value="2"/>
    <metadata key="prediction" value="1800"/>
    <metadata key="weight" value="5.00"/>
    <filament id="2" type="PETG" color="#00FF00" used_m="1.5" used_g="5"/>
  </plate>
</config>"##;
        let info = parse_slice_info(slice);
        assert_eq!(info.time_s, Some(5400));
        assert_eq!(info.filament_g, Some(25.5));
        assert_eq!(info.filament_m, Some(8.0));
        assert_eq!(info.materials, vec!["PLA", "PETG"]);
        assert_eq!(info.colors, vec!["#FF0000", "#00FF00"]);
        assert_eq!(info.printer.as_deref(), Some("Bambu Lab P1S"));
        assert_eq!(info.plates, Some(2));
        assert_eq!(info.slicer.as_deref(), Some("Bambu Studio 1.10.01.50"));

        let settings = r##"{"filament_type":["PLA","PLA"],"filament_colour":["#FFFFFF","#FFFFFF"],"layer_height":"0.16","nozzle_diameter":["0.4"],"printer_model":"Bambu Lab A1"}"##;
        let p = parse_project_settings(settings);
        assert_eq!(p.layer_mm, Some(0.16));
        assert_eq!(p.materials, vec!["PLA"]);
        assert_eq!(p.printer.as_deref(), Some("Bambu Lab A1"));
        assert!(!p.sliced);
    }
}
