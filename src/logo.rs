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
            for i in 0..=8 {
                let a = (start + i as f32 / 8.0 * 0.25) * std::f32::consts::TAU;
                pts.push((cx + r * a.cos(), cy + r * a.sin()));
            }
        }
        self.poly(&pts, col);
    }
}

/// Renders the logo as RGBA pixels (size × size).
pub fn logo_rgba(size: usize) -> Vec<u8> {
    let ss = 4;
    let mut c = Canvas::new(size * ss);

    // app-icon tile with a soft blue gradient
    c.rounded_rect(0.03, 0.03, 0.97, 0.97, 0.2, &|_, y| {
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

    // downsample
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
