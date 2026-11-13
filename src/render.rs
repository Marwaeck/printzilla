//! A small software renderer: draws a mesh with depth buffer and soft lighting into an RGBA image.

use crate::model3d::Mesh;

#[derive(Clone, Copy, PartialEq)]
pub struct Camera {
    /// rotation around the vertical axis (radians)
    pub yaw: f32,
    /// tilt (radians), positive = looking from above
    pub pitch: f32,
    /// 1.0 = whole model fits
    pub zoom: f32,
    /// pan offset in model-radius units
    pub pan: [f32; 2],
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            yaw: -0.75,
            pitch: 0.55,
            zoom: 1.0,
            pan: [0.0, 0.0],
        }
    }
}

pub struct Prepared {
    pub mesh: Mesh,
    pub normals: Vec<[f32; 3]>,
    pub center: [f32; 3],
    pub radius: f32,
    pub size: [f32; 3],
    plate: Mesh,
    plate_normals: Vec<[f32; 3]>,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 1e-12 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

fn normals_of(mesh: &Mesh) -> Vec<[f32; 3]> {
    mesh.tris
        .iter()
        .map(|t| {
            let a = mesh.positions[t[0] as usize];
            let b = mesh.positions[t[1] as usize];
            let c = mesh.positions[t[2] as usize];
            norm(cross(sub(b, a), sub(c, a)))
        })
        .collect()
}

pub fn prepare(mesh: Mesh) -> Prepared {
    let (lo, hi) = mesh.bounds().unwrap_or(([-1.0; 3], [1.0; 3]));
    let center = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let radius = (dot(size, size).sqrt() / 2.0).max(0.001);
    let normals = normals_of(&mesh);

    // A checkered build plate under the model.
    let mut plate = Mesh::default();
    let ext = radius * 1.6;
    let cells = 12;
    let step = ext * 2.0 / cells as f32;
    let z = lo[2] - radius * 0.002;
    for iy in 0..cells {
        for ix in 0..cells {
            let x0 = center[0] - ext + ix as f32 * step;
            let y0 = center[1] - ext + iy as f32 * step;
            let b = plate.positions.len() as u32;
            plate.positions.extend_from_slice(&[[x0, y0, z], [x0 + step, y0, z], [x0 + step, y0 + step, z], [x0, y0 + step, z]]);
            plate.tris.push([b, b + 1, b + 2]);
            plate.tris.push([b, b + 2, b + 3]);
            let c = if (ix + iy) % 2 == 0 { [62, 62, 67] } else { [53, 53, 58] };
            plate.colors.push(c);
            plate.colors.push(c);
        }
    }
    let plate_normals = normals_of(&plate);
    Prepared {
        mesh,
        normals,
        center,
        radius,
        size,
        plate,
        plate_normals,
    }
}

pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

struct View {
    right: [f32; 3],
    up: [f32; 3],
    fwd: [f32; 3],
    eye: [f32; 3],
    focal: f32,
    cx: f32,
    cy: f32,
}

impl View {
    fn project(&self, p: [f32; 3]) -> [f32; 3] {
        let d = sub(p, self.eye);
        let z = dot(d, self.fwd);
        let x = dot(d, self.right);
        let y = dot(d, self.up);
        let zz = z.max(1e-4);
        [self.cx + x / zz * self.focal, self.cy - y / zz * self.focal, z]
    }
}

pub fn render(p: &Prepared, cam: &Camera, width: usize, height: usize, bg: [u8; 3], with_plate: bool) -> Frame {
    let width = width.max(1);
    let height = height.max(1);
    let mut rgba = vec![0u8; width * height * 4];
    // background gradient
    for y in 0..height {
        let t = y as f32 / height as f32;
        let k = 1.12 - 0.24 * t;
        let c = [
            (bg[0] as f32 * k).min(255.0) as u8,
            (bg[1] as f32 * k).min(255.0) as u8,
            (bg[2] as f32 * k).min(255.0) as u8,
        ];
        for x in 0..width {
            let o = (y * width + x) * 4;
            rgba[o] = c[0];
            rgba[o + 1] = c[1];
            rgba[o + 2] = c[2];
            rgba[o + 3] = 255;
        }
    }
    let mut depth = vec![f32::MAX; width * height];

    let fov = 35f32.to_radians();
    let dist = p.radius / (fov / 2.0).sin() * cam.zoom.max(0.05);
    let (sy, cy) = cam.yaw.sin_cos();
    let (sp, cp) = cam.pitch.sin_cos();
    // direction from target to eye (Z is up)
    let back = [cp * sy, -cp * cy, sp];
    let fwd = [-back[0], -back[1], -back[2]];
    let right = norm(cross(fwd, [0.0, 0.0, 1.0]));
    let up = cross(right, fwd);
    let target = [
        p.center[0] - (right[0] * cam.pan[0] + up[0] * cam.pan[1]) * p.radius,
        p.center[1] - (right[1] * cam.pan[0] + up[1] * cam.pan[1]) * p.radius,
        p.center[2] - (right[2] * cam.pan[0] + up[2] * cam.pan[1]) * p.radius,
    ];
    let eye = [target[0] + back[0] * dist, target[1] + back[1] * dist, target[2] + back[2] * dist];
    let focal = (width.min(height) as f32 / 2.0) / (fov / 2.0).tan();
    let view = View {
        right,
        up,
        fwd,
        eye,
        focal,
        cx: width as f32 / 2.0,
        cy: height as f32 / 2.0,
    };

    // light comes from over the viewer's left shoulder
    let light = norm([
        back[0] * 0.8 - right[0] * 0.4 + 0.5 * up[0],
        back[1] * 0.8 - right[1] * 0.4 + 0.5 * up[1],
        back[2] * 0.8 - right[2] * 0.4 + 0.5 * up[2] + 0.3,
    ]);

    if with_plate && cam.pitch > 0.0 {
        draw_mesh(&p.plate, &p.plate_normals, &view, light, back, &mut rgba, &mut depth, width, height, true);
    }
    draw_mesh(&p.mesh, &p.normals, &view, light, back, &mut rgba, &mut depth, width, height, false);
    Frame { width, height, rgba }
}

#[allow(clippy::too_many_arguments)]
fn draw_mesh(
    mesh: &Mesh,
    normals: &[[f32; 3]],
    view: &View,
    light: [f32; 3],
    to_eye: [f32; 3],
    rgba: &mut [u8],
    depth: &mut [f32],
    w: usize,
    h: usize,
    flat: bool,
) {
    let proj: Vec<[f32; 3]> = mesh.positions.iter().map(|&q| view.project(q)).collect();
    let wf = w as f32;
    let hf = h as f32;
    for (ti, t) in mesh.tris.iter().enumerate() {
        let a = proj[t[0] as usize];
        let b = proj[t[1] as usize];
        let c = proj[t[2] as usize];
        if a[2] <= 1e-4 || b[2] <= 1e-4 || c[2] <= 1e-4 {
            continue;
        }
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if area.abs() < 1e-9 {
            continue;
        }
        let minx = a[0].min(b[0]).min(c[0]).floor().max(0.0);
        let maxx = a[0].max(b[0]).max(c[0]).ceil().min(wf - 1.0);
        let miny = a[1].min(b[1]).min(c[1]).floor().max(0.0);
        let maxy = a[1].max(b[1]).max(c[1]).ceil().min(hf - 1.0);
        if minx > maxx || miny > maxy {
            continue;
        }
        // shading
        let mut n = normals[ti];
        if dot(n, to_eye) < 0.0 {
            n = [-n[0], -n[1], -n[2]];
        }
        let base = mesh.colors.get(ti).copied().unwrap_or([200, 200, 200]);
        let col = if flat {
            base
        } else {
            let diff = dot(n, light).max(0.0);
            let rim = (1.0 - dot(n, to_eye).abs()).powf(3.0) * 0.18;
            let k = 0.32 + 0.72 * diff;
            let half = norm([light[0] + to_eye[0], light[1] + to_eye[1], light[2] + to_eye[2]]);
            let spec = dot(n, half).max(0.0).powf(40.0) * 0.25 * 255.0;
            [
                (base[0] as f32 * k + spec + rim * 255.0).min(255.0) as u8,
                (base[1] as f32 * k + spec + rim * 255.0).min(255.0) as u8,
                (base[2] as f32 * k + spec + rim * 255.0).min(255.0) as u8,
            ]
        };
        let inv_area = 1.0 / area;
        let (iza, izb, izc) = (1.0 / a[2], 1.0 / b[2], 1.0 / c[2]);
        let y0 = miny as usize;
        let y1 = maxy as usize;
        let x0 = minx as usize;
        let x1 = maxx as usize;
        for py in y0..=y1 {
            let fy = py as f32 + 0.5;
            for px in x0..=x1 {
                let fx = px as f32 + 0.5;
                let w0 = ((b[0] - fx) * (c[1] - fy) - (b[1] - fy) * (c[0] - fx)) * inv_area;
                let w1 = ((c[0] - fx) * (a[1] - fy) - (c[1] - fy) * (a[0] - fx)) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let iz = w0 * iza + w1 * izb + w2 * izc;
                let z = 1.0 / iz;
                let idx = py * w + px;
                if z < depth[idx] {
                    depth[idx] = z;
                    let o = idx * 4;
                    rgba[o] = col[0];
                    rgba[o + 1] = col[1];
                    rgba[o + 2] = col[2];
                    rgba[o + 3] = 255;
                }
            }
        }
    }
}

/// Renders a square preview picture (with 2x supersampling) for the library grid.
pub fn thumbnail(p: &Prepared, size: usize) -> Frame {
    let big = render(p, &Camera::default(), size * 2, size * 2, [44, 44, 48], true);
    let mut out = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            for ch in 0..4 {
                let mut s = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    s += big.rgba[((y * 2 + dy) * size * 2 + x * 2 + dx) * 4 + ch] as u32;
                }
                out[(y * size + x) * 4 + ch] = (s / 4) as u8;
            }
        }
    }
    Frame { width: size, height: size, rgba: out }
}
