//! Maillages : formes de base équivalentes à celles de three.js (boîte, cylindre, sphère, disque,
//! plan), sol et cyclo. Chaque forme n'est calculée qu'une fois (cache) et porte un numéro unique :
//! le moteur ne l'envoie qu'une fois à la carte graphique.

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
}

/// Un maillage (faces dans le sens trigonométrique vues de l'extérieur, comme three.js).
pub struct MeshData {
    /// numéro unique (cache des maillages envoyés à la carte graphique)
    pub id: u64,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// boîte englobante (min, max)
    pub bounds: (Vec3, Vec3),
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Construction d'un maillage.
#[derive(Default)]
pub struct Builder {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Builder {
    pub fn vertex(&mut self, pos: Vec3, normal: Vec3) -> u32 {
        self.vertices.push(Vertex { pos: pos.to_array(), normal: normal.normalize_or_zero().to_array() });
        self.vertices.len() as u32 - 1
    }

    /// Triangle orienté vers `facing` (l'ordre des sommets est corrigé au besoin).
    pub fn tri(&mut self, a: u32, b: u32, c: u32, facing: Vec3) {
        let p = |i: u32| Vec3::from(self.vertices[i as usize].pos);
        let n = (p(b) - p(a)).cross(p(c) - p(a));
        if n.dot(facing) >= 0.0 { self.indices.extend([a, b, c]) } else { self.indices.extend([a, c, b]) }
    }

    /// Quadrilatère plat a, b, c, d (dans l'ordre du contour), face vers `normal`.
    pub fn quad(&mut self, corners: [Vec3; 4], normal: Vec3) {
        let i: Vec<u32> = corners.iter().map(|&p| self.vertex(p, normal)).collect();
        self.tri(i[0], i[1], i[2], normal);
        self.tri(i[0], i[2], i[3], normal);
    }

    /// Nombre de sommets.
    pub fn len(&self) -> u32 {
        self.vertices.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Triangles d'une géométrie dont les sommets commencent à `base`.
    pub fn indices_offset(&mut self, base: u32, indices: &[u32]) {
        self.indices.extend(indices.iter().map(|i| i + base));
    }

    pub fn finish(self) -> MeshData {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for v in &self.vertices {
            lo = lo.min(Vec3::from(v.pos));
            hi = hi.max(Vec3::from(v.pos));
        }
        if self.vertices.is_empty() {
            (lo, hi) = (Vec3::ZERO, Vec3::ZERO);
        }
        MeshData { id: NEXT_ID.fetch_add(1, Ordering::Relaxed), vertices: self.vertices, indices: self.indices, bounds: (lo, hi) }
    }
}

// ------------------------------------------------------------------ cache des formes
#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Box([u32; 3]),
    Cylinder([u32; 7]),
    Sphere([u32; 3]),
    Circle([u32; 2]),
    Plane([u32; 2]),
    Beam([u32; 3]),
    Capsule([u32; 4]),
    Floor,
    Cyc,
}

fn bits<const N: usize>(v: [f32; N]) -> [u32; N] {
    v.map(f32::to_bits)
}

fn cached(key: Key, build: impl FnOnce() -> MeshData) -> Arc<MeshData> {
    static CACHE: OnceLock<Mutex<HashMap<Key, Arc<MeshData>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(m) = cache.lock().expect("cache des maillages").get(&key) {
        return m.clone();
    }
    let m = Arc::new(build());
    cache.lock().expect("cache des maillages").insert(key, m.clone());
    m
}

/// Boîte centrée (BoxGeometry).
pub fn boxed(w: f32, h: f32, d: f32) -> Arc<MeshData> {
    cached(Key::Box(bits([w, h, d])), || {
        let mut b = Builder::default();
        let (x, y, z) = (w / 2.0, h / 2.0, d / 2.0);
        let v = Vec3::new;
        b.quad([v(x, -y, -z), v(x, y, -z), v(x, y, z), v(x, -y, z)], Vec3::X);
        b.quad([v(-x, -y, z), v(-x, y, z), v(-x, y, -z), v(-x, -y, -z)], Vec3::NEG_X);
        b.quad([v(-x, y, -z), v(-x, y, z), v(x, y, z), v(x, y, -z)], Vec3::Y);
        b.quad([v(-x, -y, z), v(-x, -y, -z), v(x, -y, -z), v(x, -y, z)], Vec3::NEG_Y);
        b.quad([v(-x, -y, z), v(x, -y, z), v(x, y, z), v(-x, y, z)], Vec3::Z);
        b.quad([v(x, -y, -z), v(-x, -y, -z), v(-x, y, -z), v(x, y, -z)], Vec3::NEG_Z);
        b.finish()
    })
}

/// Cylindre (ou tronc de cône) d'axe Y centré (CylinderGeometry) : rayon du haut, du bas, hauteur,
/// segments, sans fonds (`open`), portion d'angle (départ, ouverture ; 0 et 2π : complet).
pub fn cylinder_part(r_top: f32, r_bot: f32, h: f32, segments: u32, open: bool, theta_start: f32, theta_len: f32) -> Arc<MeshData> {
    let segments = segments.max(3);
    cached(Key::Cylinder(bits([r_top, r_bot, h, segments as f32, if open { 1.0 } else { 0.0 }, theta_start, theta_len])), || {
        let mut b = Builder::default();
        let half = h / 2.0;
        let slope = (r_bot - r_top) / h.max(1e-6);
        let mut top = Vec::new();
        let mut bottom = Vec::new();
        for i in 0..=segments {
            let t = theta_start + i as f32 / segments as f32 * theta_len;
            let (s, c) = t.sin_cos();
            let n = Vec3::new(s, slope, c);
            top.push(b.vertex(Vec3::new(r_top * s, half, r_top * c), n));
            bottom.push(b.vertex(Vec3::new(r_bot * s, -half, r_bot * c), n));
        }
        for i in 0..segments as usize {
            let mid = (theta_start + (i as f32 + 0.5) / segments as f32 * theta_len).sin_cos();
            let out = Vec3::new(mid.0, slope, mid.1);
            b.tri(top[i], bottom[i], top[i + 1], out);
            b.tri(bottom[i], bottom[i + 1], top[i + 1], out);
        }
        if !open {
            for (r, y, n) in [(r_top, half, Vec3::Y), (r_bot, -half, Vec3::NEG_Y)] {
                if r <= 0.0 {
                    continue;
                }
                let center = b.vertex(Vec3::new(0.0, y, 0.0), n);
                let ring: Vec<u32> = (0..=segments)
                    .map(|i| {
                        let t = theta_start + i as f32 / segments as f32 * theta_len;
                        b.vertex(Vec3::new(r * t.sin(), y, r * t.cos()), n)
                    })
                    .collect();
                for i in 0..segments as usize {
                    b.tri(center, ring[i], ring[i + 1], n);
                }
            }
        }
        b.finish()
    })
}

pub fn cylinder(r_top: f32, r_bot: f32, h: f32, segments: u32) -> Arc<MeshData> {
    cylinder_part(r_top, r_bot, h, segments, false, 0.0, TAU)
}

pub fn tube(r_top: f32, r_bot: f32, h: f32, segments: u32) -> Arc<MeshData> {
    cylinder_part(r_top, r_bot, h, segments, true, 0.0, TAU)
}

/// Sphère centrée (SphereGeometry).
pub fn sphere(r: f32, w_seg: u32, h_seg: u32) -> Arc<MeshData> {
    let (w_seg, h_seg) = (w_seg.max(3), h_seg.max(2));
    cached(Key::Sphere(bits([r, w_seg as f32, h_seg as f32])), || {
        let mut b = Builder::default();
        let mut grid = Vec::new();
        for y in 0..=h_seg {
            let v = y as f32 / h_seg as f32;
            let row: Vec<u32> = (0..=w_seg)
                .map(|x| {
                    let u = x as f32 / w_seg as f32;
                    let p = Vec3::new(-(u * TAU).cos() * (v * PI).sin(), (v * PI).cos(), (u * TAU).sin() * (v * PI).sin());
                    b.vertex(p * r, p)
                })
                .collect();
            grid.push(row);
        }
        for y in 0..h_seg as usize {
            for x in 0..w_seg as usize {
                let (a, bb, c, d) = (grid[y][x + 1], grid[y][x], grid[y + 1][x], grid[y + 1][x + 1]);
                let centre = |i: u32| Vec3::from(b.vertices[i as usize].pos);
                let out = (centre(a) + centre(bb) + centre(c) + centre(d)).normalize_or_zero();
                if y != 0 {
                    b.tri(a, bb, d, out);
                }
                if y != h_seg as usize - 1 {
                    b.tri(bb, c, d, out);
                }
            }
        }
        b.finish()
    })
}

/// Disque dans le plan XY, face vers +Z (CircleGeometry ; 3 segments : un triangle).
pub fn circle(r: f32, segments: u32) -> Arc<MeshData> {
    let segments = segments.max(3);
    cached(Key::Circle(bits([r, segments as f32])), || {
        let mut b = Builder::default();
        let center = b.vertex(Vec3::ZERO, Vec3::Z);
        let ring: Vec<u32> = (0..=segments)
            .map(|i| {
                let t = i as f32 / segments as f32 * TAU;
                b.vertex(Vec3::new(r * t.cos(), r * t.sin(), 0.0), Vec3::Z)
            })
            .collect();
        for i in 0..segments as usize {
            b.tri(center, ring[i], ring[i + 1], Vec3::Z);
        }
        b.finish()
    })
}

/// Rectangle dans le plan XY, face vers +Z (PlaneGeometry).
pub fn plane(w: f32, h: f32) -> Arc<MeshData> {
    cached(Key::Plane(bits([w, h])), || {
        let mut b = Builder::default();
        let (x, y) = (w / 2.0, h / 2.0);
        b.quad([Vec3::new(-x, -y, 0.0), Vec3::new(x, -y, 0.0), Vec3::new(x, y, 0.0), Vec3::new(-x, y, 0.0)], Vec3::Z);
        b.finish()
    })
}

/// Gélule d'axe Y centrée (CapsuleGeometry) : rayon, longueur de la partie droite.
pub fn capsule(r: f32, length: f32, cap_segments: u32, radial: u32) -> Arc<MeshData> {
    let (cap_segments, radial) = (cap_segments.max(1), radial.max(3));
    cached(Key::Capsule(bits([r, length, cap_segments as f32, radial as f32])), || {
        let mut b = Builder::default();
        // profil du bas vers le haut : (rayon, hauteur, normale radiale, normale verticale)
        let mut profile = Vec::new();
        for i in 0..=cap_segments {
            let a = -FRAC_PI_2 + i as f32 / cap_segments as f32 * FRAC_PI_2;
            profile.push((r * a.cos(), -length / 2.0 + r * a.sin(), a.cos(), a.sin()));
        }
        for i in 0..=cap_segments {
            let a = i as f32 / cap_segments as f32 * FRAC_PI_2;
            profile.push((r * a.cos(), length / 2.0 + r * a.sin(), a.cos(), a.sin()));
        }
        let rows: Vec<Vec<u32>> = profile
            .iter()
            .map(|&(rad, y, nr, ny)| {
                (0..=radial)
                    .map(|j| {
                        let (s, c) = (j as f32 / radial as f32 * TAU).sin_cos();
                        b.vertex(Vec3::new(rad * s, y, rad * c), Vec3::new(nr * s, ny, nr * c))
                    })
                    .collect()
            })
            .collect();
        for k in 0..rows.len() - 1 {
            for j in 0..radial as usize {
                let (s, c) = ((j as f32 + 0.5) / radial as f32 * TAU).sin_cos();
                let out = Vec3::new(s, (profile[k].3 + profile[k + 1].3) / 2.0, c);
                b.tri(rows[k][j], rows[k + 1][j], rows[k][j + 1], out);
                b.tri(rows[k + 1][j], rows[k + 1][j + 1], rows[k][j + 1], out);
            }
        }
        b.finish()
    })
}

/// Cône ouvert d'un faisceau : sommet à l'origine (rayon de l'optique), axe +Z, longueur `length`
/// (createBeam de la version three.js : 40 segments).
pub fn beam(lens_r: f32, far_r: f32, length: f32) -> Arc<MeshData> {
    cached(Key::Beam(bits([lens_r, far_r, length])), || {
        let mut b = Builder::default();
        let segments = 40;
        let slope = (far_r - lens_r) / length;
        let (mut near, mut far) = (Vec::new(), Vec::new());
        for i in 0..=segments {
            let (s, c) = (i as f32 / segments as f32 * TAU).sin_cos();
            let n = Vec3::new(s, c, -slope);
            near.push(b.vertex(Vec3::new(lens_r * s, lens_r * c, 0.0), n));
            far.push(b.vertex(Vec3::new(far_r * s, far_r * c, length), n));
        }
        for i in 0..segments as usize {
            let (s, c) = ((i as f32 + 0.5) / segments as f32 * TAU).sin_cos();
            let out = Vec3::new(s, c, -slope);
            b.tri(near[i], far[i], near[i + 1], out);
            b.tri(far[i], far[i + 1], near[i + 1], out);
        }
        b.finish()
    })
}

/// Sol de 40 × 40 m (y = 0).
pub fn floor() -> Arc<MeshData> {
    cached(Key::Floor, || {
        let mut b = Builder::default();
        let s = 20.0;
        b.quad([Vec3::new(-s, 0.0, s), Vec3::new(s, 0.0, s), Vec3::new(s, 0.0, -s), Vec3::new(-s, 0.0, -s)], Vec3::Y);
        b.finish()
    })
}

/// Cyclo : 16 m de large, sol de 3 m, gorge de rayon 1,2 m, mur de 6 m à z = −7.
pub fn cyc() -> Arc<MeshData> {
    cached(Key::Cyc, || {
        const W: f32 = 8.0;
        const R: f32 = 1.2;
        const BACK: f32 = -7.0;
        // profil (z, y, normale z, normale y) du devant vers le haut du mur
        let mut profile: Vec<[f32; 4]> = vec![[BACK + R + 3.0, 0.003, 0.0, 1.0]];
        let steps = 24;
        for i in 0..=steps {
            let t = i as f32 / steps as f32 * std::f32::consts::FRAC_PI_2;
            let (s, c) = t.sin_cos();
            // centre (BACK + R, R) ; du bas (t = 0) vers le fond (t = 90°), normale vers le centre
            profile.push([BACK + R - R * s, R - R * c + 0.003 * c, s, c]);
        }
        profile.push([BACK, R + 6.0, 1.0, 0.0]);
        let mut b = Builder::default();
        let mut prev: Option<(u32, u32)> = None;
        for p in &profile {
            let n = Vec3::new(0.0, p[3], p[2]);
            let l = b.vertex(Vec3::new(-W, p[1], p[0]), n);
            let r = b.vertex(Vec3::new(W, p[1], p[0]), n);
            if let Some((pl, pr)) = prev {
                b.tri(pl, pr, r, n);
                b.tri(pl, r, l, n);
            }
            prev = Some((l, r));
        }
        b.finish()
    })
}
