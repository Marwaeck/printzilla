//! The PrintZilla logo: a big cube made of many small orange cubes on a print bed. Drawn from shapes, so it works at any size
//! (window icon and in-app logo).

type Col = [u8; 4];

struct Canvas {
    n: usize,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(n: usize) -> Self {
        Canvas { n, px: vec![[0.0; 4]; n * n] }
    }

    fn blend(&mut self, x: usize, y: usize, c: Col) {
        let a = c[3] as f32 / 255.0;
        let p = &mut self.px[y * self.n + x];
        for k in 0..3 {
            p[k] = p[k] * (1.0 - a) + c[k] as f32 * a;
        }
        p[3] = p[3] * (1.0 - a) + 255.0 * a;
    }

    /// Fills a polygon given in 0..1 coordinates (even-odd rule); `col` may depend on the position.
    fn poly(&mut self, pts: &[(f32, f32)], col: &dyn Fn(f32, f32) -> Col) {
        let n = self.n as f32;
        let ys: Vec<f32> = pts.iter().map(|p| p.1 * n).collect();
        let y0 = ys.iter().cloned().fold(f32::MAX, f32::min).floor().max(0.0) as usize;
        let y1 = (ys.iter().cloned().fold(f32::MIN, f32::max).ceil() as usize).min(self.n);
        for y in y0..y1 {
            let fy = y as f32 + 0.5;
            let mut xs: Vec<f32> = Vec::new();
            for i in 0..pts.len() {
                let (ax, ay) = (pts[i].0 * n, pts[i].1 * n);
                let (bx, by) = (pts[(i + 1) % pts.len()].0 * n, pts[(i + 1) % pts.len()].1 * n);
                if (ay <= fy && by > fy) || (by <= fy && ay > fy) {
                    xs.push(ax + (fy - ay) / (by - ay) * (bx - ax));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for pair in xs.chunks(2) {
                if pair.len() < 2 {
                    continue;
                }
                let xa = pair[0].round().max(0.0) as usize;
                let xb = (pair[1].round() as usize).min(self.n);
                for x in xa..xb {
                    let c = col((x as f32 + 0.5) / n, fy / n);
                    self.blend(x, y, c);
                }
            }
        }
    }

    fn rounded_rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, col: &dyn Fn(f32, f32) -> Col) {
        let mut pts = Vec::new();
        let corners = [(x1 - r, y0 + r, -0.25f32), (x1 - r, y1 - r, 0.0), (x0 + r, y1 - r, 0.25), (x0 + r, y0 + r, 0.5)];
        for (cx, cy, start) in corners {
            for i in 0..=24 {
                let a = (start + i as f32 / 24.0 * 0.25) * std::f32::consts::TAU;
                pts.push((cx + r * a.cos(), cy + r * a.sin()));
            }
        }
        self.poly(&pts, col);
    }
}

/// Renders the logo as RGBA pixels (size × size): the tile nearly fills the picture
/// (used in the app, for Windows and the website).
pub fn logo_rgba(size: usize) -> Vec<u8> {
    logo_draw(size, 0.03, 0.2)
}

/// The app icon in Apple's proportions: the tile takes 824 of 1024 pixels in the middle,
/// with a transparent margin and a soft shadow, so it is exactly as big as the other
/// Dock icons (macOS draws icons edge to edge otherwise and they look too large).
pub fn logo_rgba_macos(size: usize) -> Vec<u8> {
    let inner = ((size as f32) * 824.0 / 1024.0).round() as usize;
    let off = (size - inner) / 2;
    let tile = logo_draw(inner, 0.0, 0.225);

    // soft shadow under the tile
    let ss = 4;
    let mut c = Canvas::new(size * ss);
    let m = off as f32 / size as f32;
    let dy = 0.01f32;
    for k in 0..8 {
        let e = 0.001 + k as f32 * 0.003;
        let r = 0.225 * (1.0 - 2.0 * m) + e;
        c.rounded_rect(m - e, m - e + dy, 1.0 - m + e, 1.0 - m + e + dy, r, &|_, _| [0, 0, 0, 5]);
    }
    let mut out = finish(&c, size, ss);

    // the tile on top (straight-alpha "over")
    for y in 0..inner {
        for x in 0..inner {
            let t = &tile[(y * inner + x) * 4..(y * inner + x) * 4 + 4];
            let o = ((y + off) * size + x + off) * 4;
            let ta = t[3] as f32 / 255.0;
            let ba = out[o + 3] as f32 / 255.0;
            let a = ta + ba * (1.0 - ta);
            if a > 0.0 {
                for k in 0..3 {
                    out[o + k] = ((t[k] as f32 * ta + out[o + k] as f32 * ba * (1.0 - ta)) / a).round().min(255.0) as u8;
                }
            }
            out[o + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

/// Draws the logo with the tile `inset` from the edges and the given corner radius.
fn logo_draw(size: usize, inset: f32, radius: f32) -> Vec<u8> {
    let ss = 4;
    let mut c = Canvas::new(size * ss);

    // app-icon tile with a soft blue gradient
    c.rounded_rect(inset, inset, 1.0 - inset, 1.0 - inset, radius, &|_, y| {
        [(52.0 + 10.0 * y) as u8, (124.0 - 30.0 * y) as u8, (238.0 - 40.0 * y) as u8, 255]
    });

    // print bed: an angled light-grey plate with a little thickness
    let (pc, phw, phh, th) = (0.695f32, 0.35f32, 0.196f32, 0.035f32);
    c.poly(&[(0.5 - phw, pc), (0.5, pc + phh), (0.5, pc + phh + th), (0.5 - phw, pc + th)], &|_, _| [150, 160, 178, 255]);
    c.poly(&[(0.5, pc + phh), (0.5 + phw, pc), (0.5 + phw, pc + th), (0.5, pc + phh + th)], &|_, _| [118, 128, 148, 255]);
    c.poly(&[(0.5, pc - phh), (0.5 + phw, pc), (0.5, pc + phh), (0.5 - phw, pc)], &|_, _| [222, 228, 238, 255]);

    // a big cube built from many small cubes in orange tones
    let (cx, top, hw, hh, h) = (0.50f32, 0.23f32, 0.26f32, 0.145f32, 0.32f32);
    let n = 4usize;
    let gap = 0.07f32;
    // (corner, edge U, edge V, base colour, seam colour, face number)
    let faces: [((f32, f32), (f32, f32), (f32, f32), [f32; 3], Col, u32); 3] = [
        ((cx, top + 2.0 * hh), (hw, -hh), (-hw, -hh), [255.0, 184.0, 96.0], [176, 92, 30, 255], 1),
        ((cx - hw, top + hh), (hw, hh), (0.0, h), [240.0, 136.0, 48.0], [140, 64, 18, 255], 2),
        ((cx, top + 2.0 * hh), (hw, -hh), (0.0, h), [200.0, 98.0, 30.0], [112, 48, 12, 255], 3),
    ];
    for (p0, u, v, base, seam, fid) in faces {
        let at = |a: f32, b: f32| (p0.0 + u.0 * a + v.0 * b, p0.1 + u.1 * a + v.1 * b);
        // seams: the whole face in a dark tone first
        c.poly(&[at(0.0, 0.0), at(1.0, 0.0), at(1.0, 1.0), at(0.0, 1.0)], &move |_, _| seam);
        for i in 0..n {
            for j in 0..n {
                let (a0, a1) = ((i as f32 + gap) / n as f32, (i as f32 + 1.0 - gap) / n as f32);
                let (b0, b1) = ((j as f32 + gap) / n as f32, (j as f32 + 1.0 - gap) / n as f32);
                // a little variation from cube to cube
                let hsh = (i as u32 * 73 + j as u32 * 151 + fid * 37).wrapping_mul(2654435761u32) >> 24;
                let k = 0.9 + (hsh as f32 / 255.0) * 0.18;
                let col: Col = [
                    (base[0] * k).min(255.0) as u8,
                    (base[1] * k).min(255.0) as u8,
                    (base[2] * k).min(255.0) as u8,
                    255,
                ];
                c.poly(&[at(a0, b0), at(a1, b0), at(a1, b1), at(a0, b1)], &move |_, _| col);
                // soft highlight along one edge of each little cube
                let hl: Col = [255, 236, 200, 70];
                let e = (a1 - a0) * 0.18;
                c.poly(&[at(a0, b0), at(a1, b0), at(a1, b0 + e), at(a0, b0 + e)], &move |_, _| hl);
            }
        }
    }

    finish(&c, size, ss)
}

/// Averages the supersampled canvas down to `size` × `size` RGBA pixels.
fn finish(c: &Canvas, size: usize, ss: usize) -> Vec<u8> {
    let mut out = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0.0f32; 4];
            for dy in 0..ss {
                for dx in 0..ss {
                    let p = c.px[(y * ss + dy) * size * ss + x * ss + dx];
                    // premultiplied accumulation
                    let a = p[3] / 255.0;
                    acc[0] += p[0] * a;
                    acc[1] += p[1] * a;
                    acc[2] += p[2] * a;
                    acc[3] += p[3];
                }
            }
            let n = (ss * ss) as f32;
            let a = acc[3] / n;
            let o = (y * size + x) * 4;
            if a > 0.0 {
                let k = 255.0 / a;
                out[o] = (acc[0] / n * k).min(255.0) as u8;
                out[o + 1] = (acc[1] / n * k).min(255.0) as u8;
                out[o + 2] = (acc[2] / n * k).min(255.0) as u8;
            }
            out[o + 3] = a as u8;
        }
    }
    out
}

/// The small symbol for the menu bar (macOS) or the notification area (Windows):
/// the same cube of little cubes, as a black shape with three shades on transparent
/// ground. macOS turns it white on dark menu bars by itself ("template image").
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // menu bar symbol on macOS
pub fn menu_icon_rgba(size: usize) -> Vec<u8> {
    let ss = 8;
    let mut c = Canvas::new(size * ss);
    let (cx, top, hw, hh, h) = (0.5f32, 0.01f32, 0.37f32, 0.205f32, 0.37f32);
    let n = 3usize;
    let gap = 0.09f32;
    // the print plate under the cube (flat, lighter), like in the app icon
    let (pc, phw, phh, th) = (top + hh + h + 0.02, 0.49f32, 0.27f32, 0.055f32);
    // outline of the cube; the plate is left out there so it never shows through the gaps
    let hex = [
        (cx, top),
        (cx + hw, top + hh),
        (cx + hw, top + hh + h),
        (cx, top + 2.0 * hh + h),
        (cx - hw, top + hh + h),
        (cx - hw, top + hh),
    ];
    let under_cube = move |x: f32, y: f32| {
        (0..hex.len()).all(|i| {
            let (a, b) = (hex[i], hex[(i + 1) % hex.len()]);
            (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0) >= -0.004
        })
    };
    let plate = move |alpha: u8| move |x: f32, y: f32| -> Col { [0, 0, 0, if under_cube(x, y) { 0 } else { alpha }] };
    c.poly(&[(cx - phw, pc), (cx, pc + phh), (cx, pc + phh + th), (cx - phw, pc + th)], &plate(170));
    c.poly(&[(cx, pc + phh), (cx + phw, pc), (cx + phw, pc + th), (cx, pc + phh + th)], &plate(120));
    c.poly(&[(cx, pc - phh), (cx + phw, pc), (cx, pc + phh), (cx - phw, pc)], &plate(80));
    let faces: [((f32, f32), (f32, f32), (f32, f32), u8); 3] = [
        ((cx, top + 2.0 * hh), (hw, -hh), (-hw, -hh), 255),
        ((cx - hw, top + hh), (hw, hh), (0.0, h), 205),
        ((cx, top + 2.0 * hh), (hw, -hh), (0.0, h), 140),
    ];
    for (p0, u, v, alpha) in faces {
        let at = |a: f32, b: f32| (p0.0 + u.0 * a + v.0 * b, p0.1 + u.1 * a + v.1 * b);
        for i in 0..n {
            for j in 0..n {
                let (a0, a1) = ((i as f32 + gap) / n as f32, (i as f32 + 1.0 - gap) / n as f32);
                let (b0, b1) = ((j as f32 + gap) / n as f32, (j as f32 + 1.0 - gap) / n as f32);
                c.poly(&[at(a0, b0), at(a1, b0), at(a1, b1), at(a0, b1)], &move |_, _| [0, 0, 0, alpha]);
            }
        }
    }
    finish(&c, size, ss)
}

/// The app icon with a red circle and a number in the top right corner
/// (Windows taskbar: shows how many new files wait). 10 and more show as "9+".
#[cfg_attr(target_os = "macos", allow(dead_code))] // used on Windows
pub fn logo_with_badge(size: usize, n: usize) -> Vec<u8> {
    let mut px = logo_rgba(size);
    if n == 0 {
        return px;
    }
    let ss = 4;
    let mut c = Canvas::new(size * ss);
    let (cx, cy, r) = (0.73f32, 0.27f32, 0.25f32);
    let ring: Vec<(f32, f32)> = (0..48)
        .map(|i| {
            let a = i as f32 / 48.0 * std::f32::consts::TAU;
            (cx + (r + 0.035) * a.cos(), cy + (r + 0.035) * a.sin())
        })
        .collect();
    c.poly(&ring, &|_, _| [255, 255, 255, 255]);
    let disc: Vec<(f32, f32)> = ring.iter().map(|&(x, y)| (cx + (x - cx) * r / (r + 0.035), cy + (y - cy) * r / (r + 0.035))).collect();
    c.poly(&disc, &|_, _| [226, 46, 46, 255]);

    // the number, drawn with simple segments (like a digital clock)
    let text: Vec<char> = if n > 9 { vec!['9', '+'] } else { n.to_string().chars().collect() };
    let (h, w) = (0.24f32, 0.13f32);
    let gap = 0.03f32;
    let total = text.len() as f32 * w + (text.len() as f32 - 1.0) * gap;
    let mut x = cx - total / 2.0;
    let y = cy - h / 2.0;
    let t = w * 0.26;
    let white = |_: f32, _: f32| -> Col { [255, 255, 255, 255] };
    for ch in text {
        let seg = |c: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32| c.poly(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], &white);
        if ch == '+' {
            seg(&mut c, x, y + h / 2.0 - t / 2.0, x + w, y + h / 2.0 + t / 2.0);
            seg(&mut c, x + w / 2.0 - t / 2.0, y + h * 0.2, x + w / 2.0 + t / 2.0, y + h * 0.8);
        } else {
            // segments a b c d e f g
            let on: &[bool; 7] = match ch {
                '0' => &[true, true, true, true, true, true, false],
                '1' => &[false, true, true, false, false, false, false],
                '2' => &[true, true, false, true, true, false, true],
                '3' => &[true, true, true, true, false, false, true],
                '4' => &[false, true, true, false, false, true, true],
                '5' => &[true, false, true, true, false, true, true],
                '6' => &[true, false, true, true, true, true, true],
                '7' => &[true, true, true, false, false, false, false],
                '8' => &[true; 7],
                _ => &[true, true, true, true, false, true, true],
            };
            let mid = y + h / 2.0;
            if on[0] { seg(&mut c, x, y, x + w, y + t); }
            if on[1] { seg(&mut c, x + w - t, y, x + w, mid + t / 2.0); }
            if on[2] { seg(&mut c, x + w - t, mid - t / 2.0, x + w, y + h); }
            if on[3] { seg(&mut c, x, y + h - t, x + w, y + h); }
            if on[4] { seg(&mut c, x, mid - t / 2.0, x + t, y + h); }
            if on[5] { seg(&mut c, x, y, x + t, mid + t / 2.0); }
            // a shorter middle bar keeps the 3 from looking like a mirrored E
            let gx = if ch == '3' { x + w * 0.35 } else { x };
            if on[6] { seg(&mut c, gx, mid - t / 2.0, x + w, mid + t / 2.0); }
        }
        x += w + gap;
    }
    let badge = finish(&c, size, ss);
    // lay the badge over the icon
    for (p, b) in px.chunks_mut(4).zip(badge.chunks(4)) {
        let a = b[3] as f32 / 255.0;
        if a > 0.0 {
            for k in 0..3 {
                p[k] = (p[k] as f32 * (1.0 - a) + b[k] as f32 * a) as u8;
            }
            p[3] = p[3].max(b[3]);
        }
    }
    px
}
