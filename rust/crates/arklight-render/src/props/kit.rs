//! Petit atelier de modélisation pour le décor (portage de `web/js/viewport/props/kit.js`) :
//! pièces simples (boîtes, cylindres, gélules, tours, extrusions…) posées dans le repère de
//! l'objet (sol en y = 0, face avant vers +Z), regroupées par matière puis fusionnées : un seul
//! maillage par matière.
//!
//! Correspondances avec la version JavaScript :
//! `k.mat('nom', couleur, { rough, metal, opacity, emissive, flat, double })` → `k.mat("nom", couleur, mo().rough(..)…)`,
//! `k.box(w, h, d, m, p, { q })` → `k.boxed(w, h, d, m, p, o().q(q))`, `{ seg: 20, open: true }` →
//! `o().seg(20).open()`, `new THREE.Vector3(x, y, z)` → `v(x, y, z)`, `shade(c, k)` → `shade(c, k)`.

use crate::mesh::{Builder, MeshData};
use crate::{Material, Surface};
use arklight_core::color::{linear_to_srgb, srgb_to_linear};
use glam::{EulerRot, Mat3, Mat4, Quat, Vec2, Vec3};
use serde_json::Value;
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

pub fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Quaternion d'angles d'Euler (ordre XYZ, comme three.js).
pub fn euler(x: f32, y: f32, z: f32) -> Quat {
    Quat::from_euler(EulerRot::XYZ, x, y, z)
}

/// `setFromUnitVectors(a, b)`
pub fn from_to(a: Vec3, b: Vec3) -> Quat {
    Quat::from_rotation_arc(a.normalize_or(Vec3::Y), b.normalize_or(Vec3::Y))
}

// ------------------------------------------------------------------ couleurs
/// Couleur (stockée en sRGB linéaire, comme three.js avec la gestion des couleurs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Col(pub [f32; 3]);

impl Col {
    pub fn hex(c: u32) -> Col {
        let ch = |s: u32| srgb_to_linear(((c >> s) & 0xff) as f64 / 255.0) as f32;
        Col([ch(16), ch(8), ch(0)])
    }

    /// « #rrggbb » (ou « #rgb ») ; illisible : gris.
    pub fn parse(s: &str) -> Col {
        let t = s.trim().trim_start_matches('#');
        let full = if t.len() == 3 { t.chars().flat_map(|c| [c, c]).collect::<String>() } else { t.to_owned() };
        u32::from_str_radix(&full, 16).map_or(Col::hex(0x8a8f96), Col::hex)
    }

    /// Couleur hexadécimale « #rrggbb » (comme `getHexString`).
    pub fn to_hex(self) -> String {
        let c = self.0.map(|v| (linear_to_srgb(v.clamp(0.0, 1.0) as f64) * 255.0).round() as u8);
        format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
    }
}

impl From<u32> for Col {
    fn from(c: u32) -> Col {
        Col::hex(c)
    }
}

impl From<&str> for Col {
    fn from(s: &str) -> Col {
        Col::parse(s)
    }
}

impl From<&String> for Col {
    fn from(s: &String) -> Col {
        Col::parse(s)
    }
}

impl From<String> for Col {
    fn from(s: String) -> Col {
        Col::parse(&s)
    }
}

/// Couleur un peu plus sombre ou plus claire (k < 1 : plus sombre), comme `shade`.
pub fn shade(c: impl Into<Col>, k: f32) -> Col {
    let Col(c) = c.into();
    if k < 1.0 {
        Col(c.map(|v| v * k))
    } else {
        let t = (k - 1.0).min(1.0);
        Col(c.map(|v| v + (1.0 - v) * t))
    }
}

/// Petit générateur pseudo-aléatoire stable (mêmes livres, mêmes feuilles à chaque construction).
pub fn rng(seed: u32) -> impl FnMut() -> f32 {
    let mut s: u32 = if seed == 0 { 1 } else { seed };
    move || {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        (s as f64 / 4294967296.0) as f32
    }
}

// ------------------------------------------------------------------ réglages des objets (JSON)
/// Lecture des réglages d'un objet de décor (`props.color || '#…'`, `props.width ?? 3`).
#[derive(Clone, Copy)]
pub struct Props<'a>(pub &'a Value);

impl<'a> Props<'a> {
    pub fn str(&self, key: &str) -> Option<&'a str> {
        self.0.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
    }

    pub fn num(&self, key: &str) -> Option<f32> {
        self.0.get(key).and_then(Value::as_f64).map(|v| v as f32)
    }

    /// Couleur `key`, sinon `default`.
    pub fn col(&self, key: &str, default: &str) -> Col {
        Col::parse(self.str(key).unwrap_or(default))
    }
}

// ------------------------------------------------------------------ géométrie
/// Géométrie indexée (positions, normales), comme une BufferGeometry de three.js.
#[derive(Clone, Default)]
pub struct Geo {
    pub pos: Vec<Vec3>,
    pub nrm: Vec<Vec3>,
    pub idx: Vec<u32>,
}

impl Geo {
    /// Ajoute un sommet (position, normale) ; renvoie son numéro.
    pub fn vertex(&mut self, p: Vec3, n: Vec3) -> u32 {
        self.pos.push(p);
        self.nrm.push(n.normalize_or_zero());
        self.pos.len() as u32 - 1
    }

    /// Triangle orienté vers `facing`.
    pub fn tri(&mut self, a: u32, b: u32, c: u32, facing: Vec3) {
        let p = |i: u32| self.pos[i as usize];
        let n = (p(b) - p(a)).cross(p(c) - p(a));
        if n.dot(facing) >= 0.0 { self.idx.extend([a, b, c]) } else { self.idx.extend([a, c, b]) }
    }

    /// Normales lissées (moyenne des faces), comme `computeVertexNormals`.
    pub fn compute_vertex_normals(&mut self) {
        let mut acc = vec![Vec3::ZERO; self.pos.len()];
        for t in self.idx.chunks_exact(3) {
            let (a, b, c) = (self.pos[t[0] as usize], self.pos[t[1] as usize], self.pos[t[2] as usize]);
            let n = (b - a).cross(c - a);
            for &i in t {
                acc[i as usize] += n;
            }
        }
        self.nrm = acc.into_iter().map(|n| n.normalize_or(Vec3::Y)).collect();
    }

    fn transform(mut self, m: Mat4) -> Geo {
        let nm = Mat3::from_mat4(m).inverse().transpose();
        for p in &mut self.pos {
            *p = m.transform_point3(*p);
        }
        for n in &mut self.nrm {
            *n = (nm * *n).normalize_or_zero();
        }
        if m.determinant() < 0.0 {
            for t in self.idx.chunks_exact_mut(3) {
                t.swap(1, 2);
            }
        }
        self
    }

    /// Rectangle dans le plan XY (face +Z), découpé en `ws` × `hs` (PlaneGeometry).
    pub fn plane(w: f32, h: f32, ws: u32, hs: u32) -> Geo {
        let mut g = Geo::default();
        let (ws, hs) = (ws.max(1), hs.max(1));
        let mut grid = Vec::new();
        for iy in 0..=hs {
            let y = h / 2.0 - iy as f32 / hs as f32 * h;
            grid.push((0..=ws).map(|ix| g.vertex(v(-w / 2.0 + ix as f32 / ws as f32 * w, y, 0.0), Vec3::Z)).collect::<Vec<_>>());
        }
        for iy in 0..hs as usize {
            for ix in 0..ws as usize {
                let (a, b, c, d) = (grid[iy][ix], grid[iy + 1][ix], grid[iy + 1][ix + 1], grid[iy][ix + 1]);
                g.idx.extend([a, b, d, b, c, d]);
            }
        }
        g
    }

    pub fn boxed(w: f32, h: f32, d: f32) -> Geo {
        let mut g = Geo::default();
        let (x, y, z) = (w / 2.0, h / 2.0, d / 2.0);
        let faces: [([Vec3; 4], Vec3); 6] = [
            ([v(x, -y, -z), v(x, y, -z), v(x, y, z), v(x, -y, z)], Vec3::X),
            ([v(-x, -y, z), v(-x, y, z), v(-x, y, -z), v(-x, -y, -z)], Vec3::NEG_X),
            ([v(-x, y, -z), v(-x, y, z), v(x, y, z), v(x, y, -z)], Vec3::Y),
            ([v(-x, -y, z), v(-x, -y, -z), v(x, -y, -z), v(x, -y, z)], Vec3::NEG_Y),
            ([v(-x, -y, z), v(x, -y, z), v(x, y, z), v(-x, y, z)], Vec3::Z),
            ([v(x, -y, -z), v(-x, -y, -z), v(-x, y, -z), v(x, y, -z)], Vec3::NEG_Z),
        ];
        for (c, n) in faces {
            let i: Vec<u32> = c.iter().map(|&p| g.vertex(p, n)).collect();
            g.tri(i[0], i[1], i[2], n);
            g.tri(i[0], i[2], i[3], n);
        }
        g
    }

    /// Boîte aux arêtes arrondies (RoundedBoxGeometry) : chaque face quadrillée, chaque point
    /// ramené sur la surface arrondie autour de la boîte intérieure.
    pub fn rounded_box(w: f32, h: f32, d: f32, segments: u32, radius: f32) -> Geo {
        let half = v(w / 2.0, h / 2.0, d / 2.0);
        let inner = (half - Vec3::splat(radius)).max(Vec3::ZERO);
        let seg = segments.max(1) * 2;
        // coordonnées le long d'un axe : arrondi, plat, arrondi
        let coords = |a: f32| -> Vec<f32> {
            let ia = (a - radius).max(0.0);
            let mut c: Vec<f32> = (0..=seg).map(|k| -ia - radius * (FRAC_PI_2 * (1.0 - k as f32 / seg as f32)).sin()).collect();
            c.extend((0..=seg).map(|k| ia + radius * (FRAC_PI_2 * k as f32 / seg as f32).sin()));
            c.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
            c
        };
        let (cx, cy, cz) = (coords(half.x), coords(half.y), coords(half.z));
        let mut g = Geo::default();
        let mut face = |axis: usize, sign: f32, us: &[f32], vs: &[f32]| {
            let mut grid = Vec::new();
            for &a in vs {
                let row: Vec<u32> = us
                    .iter()
                    .map(|&b| {
                        let mut p = [0.0; 3];
                        p[axis] = sign * half[axis];
                        p[(axis + 1) % 3] = b;
                        p[(axis + 2) % 3] = a;
                        let p = Vec3::from(p);
                        let core = p.clamp(-inner, inner);
                        let n = (p - core).normalize_or_zero();
                        let n = if n == Vec3::ZERO { Vec3::from({ let mut f = [0.0; 3]; f[axis] = sign; f }) } else { n };
                        g.vertex(core + n * radius, n)
                    })
                    .collect();
                grid.push(row);
            }
            let mut out = [0.0; 3];
            out[axis] = sign;
            let out = Vec3::from(out);
            for j in 0..grid.len() - 1 {
                for i in 0..grid[0].len() - 1 {
                    g.tri(grid[j][i], grid[j][i + 1], grid[j + 1][i + 1], out);
                    g.tri(grid[j][i], grid[j + 1][i + 1], grid[j + 1][i], out);
                }
            }
        };
        face(0, 1.0, &cy, &cz);
        face(0, -1.0, &cy, &cz);
        face(1, 1.0, &cz, &cx);
        face(1, -1.0, &cz, &cx);
        face(2, 1.0, &cx, &cy);
        face(2, -1.0, &cx, &cy);
        g
    }

    /// Cylindre (CylinderGeometry) d'axe Y centré.
    pub fn cylinder(r_top: f32, r_bot: f32, h: f32, segments: u32, open: bool, theta_start: f32, theta_len: f32) -> Geo {
        let segments = segments.max(3);
        let mut g = Geo::default();
        let half = h / 2.0;
        let slope = (r_bot - r_top) / h.max(1e-6);
        let (mut top, mut bottom) = (Vec::new(), Vec::new());
        for i in 0..=segments {
            let t = theta_start + i as f32 / segments as f32 * theta_len;
            let (s, c) = t.sin_cos();
            let n = v(s, slope, c);
            top.push(g.vertex(v(r_top * s, half, r_top * c), n));
            bottom.push(g.vertex(v(r_bot * s, -half, r_bot * c), n));
        }
        for i in 0..segments as usize {
            let (s, c) = (theta_start + (i as f32 + 0.5) / segments as f32 * theta_len).sin_cos();
            let out = v(s, slope, c);
            g.tri(top[i], bottom[i], top[i + 1], out);
            g.tri(bottom[i], bottom[i + 1], top[i + 1], out);
        }
        if !open {
            for (r, y, n) in [(r_top, half, Vec3::Y), (r_bot, -half, Vec3::NEG_Y)] {
                if r <= 0.0 {
                    continue;
                }
                let center = g.vertex(v(0.0, y, 0.0), n);
                let ring: Vec<u32> = (0..=segments)
                    .map(|i| {
                        let t = theta_start + i as f32 / segments as f32 * theta_len;
                        g.vertex(v(r * t.sin(), y, r * t.cos()), n)
                    })
                    .collect();
                for i in 0..segments as usize {
                    g.tri(center, ring[i], ring[i + 1], n);
                }
            }
        }
        g
    }

    /// Sphère (SphereGeometry), éventuellement partielle (angles θ depuis le haut).
    pub fn sphere(r: f32, w_seg: u32, h_seg: u32, theta_start: f32, theta_len: f32) -> Geo {
        let (w_seg, h_seg) = (w_seg.max(3), h_seg.max(2));
        let mut g = Geo::default();
        let theta_end = (theta_start + theta_len).min(PI);
        let mut grid = Vec::new();
        for iy in 0..=h_seg {
            let vv = iy as f32 / h_seg as f32;
            let th = theta_start + vv * theta_len;
            let row: Vec<u32> = (0..=w_seg)
                .map(|ix| {
                    let u = ix as f32 / w_seg as f32 * TAU;
                    let p = v(-u.cos() * th.sin(), th.cos(), u.sin() * th.sin());
                    g.vertex(p * r, p)
                })
                .collect();
            grid.push(row);
        }
        for iy in 0..h_seg as usize {
            for ix in 0..w_seg as usize {
                let (a, b, c, d) = (grid[iy][ix + 1], grid[iy][ix], grid[iy + 1][ix], grid[iy + 1][ix + 1]);
                let out = (g.pos[a as usize] + g.pos[b as usize] + g.pos[c as usize] + g.pos[d as usize]).normalize_or(Vec3::Y);
                if iy != 0 || theta_start > 0.0 {
                    g.tri(a, b, d, out);
                }
                if iy != h_seg as usize - 1 || theta_end < PI {
                    g.tri(b, c, d, out);
                }
            }
        }
        g
    }

    /// Tore dans le plan XY (TorusGeometry) : rayon, rayon du tube, segments du tube, segments, arc.
    pub fn torus(radius: f32, tube: f32, radial: u32, tubular: u32, arc: f32) -> Geo {
        let (radial, tubular) = (radial.max(3), tubular.max(3));
        let mut g = Geo::default();
        let mut grid = Vec::new();
        for j in 0..=radial {
            let vv = j as f32 / radial as f32 * TAU;
            let row: Vec<u32> = (0..=tubular)
                .map(|i| {
                    let u = i as f32 / tubular as f32 * arc;
                    let p = v((radius + tube * vv.cos()) * u.cos(), (radius + tube * vv.cos()) * u.sin(), tube * vv.sin());
                    let center = v(radius * u.cos(), radius * u.sin(), 0.0);
                    g.vertex(p, p - center)
                })
                .collect();
            grid.push(row);
        }
        for j in 0..radial as usize {
            for i in 0..tubular as usize {
                let (a, b, c, d) = (grid[j][i], grid[j + 1][i], grid[j + 1][i + 1], grid[j][i + 1]);
                let out = g.nrm[a as usize] + g.nrm[b as usize] + g.nrm[c as usize] + g.nrm[d as usize];
                g.tri(a, b, d, out);
                g.tri(b, c, d, out);
            }
        }
        g
    }

    /// Polyèdre régulier subdivisé et ramené sur la sphère (PolyhedronGeometry) ; normales
    /// lissées dès la première subdivision, plates sinon (comme three.js).
    fn polyhedron(vertices: &[f32], faces: &[u32], r: f32, detail: u32) -> Geo {
        let p = |i: u32| v(vertices[i as usize * 3], vertices[i as usize * 3 + 1], vertices[i as usize * 3 + 2]);
        let mut g = Geo::default();
        let cols = detail as usize + 1;
        for f in faces.chunks_exact(3) {
            let (a, b, c) = (p(f[0]), p(f[1]), p(f[2]));
            let mut grid: Vec<Vec<Vec3>> = Vec::new();
            for i in 0..=cols {
                let aj = a.lerp(c, i as f32 / cols as f32);
                let bj = b.lerp(c, i as f32 / cols as f32);
                let rows = cols - i;
                grid.push((0..=rows).map(|j| if rows == 0 { aj } else { aj.lerp(bj, j as f32 / rows as f32) }).collect());
            }
            let mut emit = |x: Vec3, y: Vec3, z: Vec3| {
                let pts = [x, y, z].map(|q| q.normalize_or(Vec3::Y) * r);
                let face_n = (pts[1] - pts[0]).cross(pts[2] - pts[0]);
                let out = (pts[0] + pts[1] + pts[2]).normalize_or(Vec3::Y);
                let facing = if face_n.dot(out) >= 0.0 { face_n } else { -face_n };
                let ids: Vec<u32> = pts.iter().map(|q| g.vertex(*q, if detail == 0 { facing } else { *q })).collect();
                g.tri(ids[0], ids[1], ids[2], out);
            };
            for i in 0..cols {
                for j in 0..2 * (cols - i) - 1 {
                    let k = j / 2;
                    if j % 2 == 0 {
                        emit(grid[i][k + 1], grid[i + 1][k], grid[i][k]);
                    } else {
                        emit(grid[i][k + 1], grid[i + 1][k + 1], grid[i + 1][k]);
                    }
                }
            }
        }
        g
    }

    pub fn icosahedron(r: f32, detail: u32) -> Geo {
        let t = (1.0 + 5f32.sqrt()) / 2.0;
        #[rustfmt::skip]
        let vertices = [-1.0, t, 0.0, 1.0, t, 0.0, -1.0, -t, 0.0, 1.0, -t, 0.0, 0.0, -1.0, t, 0.0, 1.0, t, 0.0, -1.0, -t, 0.0, 1.0, -t, t, 0.0, -1.0, t, 0.0, 1.0, -t, 0.0, -1.0, -t, 0.0, 1.0];
        #[rustfmt::skip]
        let faces = [0, 11, 5, 0, 5, 1, 0, 1, 7, 0, 7, 10, 0, 10, 11, 1, 5, 9, 5, 11, 4, 11, 10, 2, 10, 7, 6, 7, 1, 8, 3, 9, 4, 3, 4, 2, 3, 2, 6, 3, 6, 8, 3, 8, 9, 4, 9, 5, 2, 4, 11, 6, 2, 10, 8, 6, 7, 9, 8, 1];
        Self::polyhedron(&vertices, &faces, r, detail)
    }

    pub fn dodecahedron(r: f32, detail: u32) -> Geo {
        let t = (1.0 + 5f32.sqrt()) / 2.0;
        let q = 1.0 / t;
        #[rustfmt::skip]
        let vertices = [
            -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0,
            0.0, -q, -t, 0.0, -q, t, 0.0, q, -t, 0.0, q, t, -q, -t, 0.0, -q, t, 0.0, q, -t, 0.0, q, t, 0.0, -t, 0.0, -q, t, 0.0, -q, -t, 0.0, q, t, 0.0, q,
        ];
        #[rustfmt::skip]
        let faces = [
            3, 11, 7, 3, 7, 15, 3, 15, 13, 7, 19, 17, 7, 17, 6, 7, 6, 15, 17, 4, 8, 17, 8, 10, 17, 10, 6, 8, 0, 16, 8, 16, 2, 8, 2, 10, 0, 12, 1, 0, 1, 18, 0, 18, 16,
            6, 10, 2, 6, 2, 13, 6, 13, 15, 2, 16, 18, 2, 18, 3, 2, 3, 13, 18, 1, 9, 18, 9, 11, 18, 11, 3, 4, 14, 12, 4, 12, 0, 4, 0, 8, 11, 9, 5, 11, 5, 19, 11, 19, 7,
            19, 5, 14, 19, 14, 4, 19, 4, 17, 1, 12, 14, 1, 14, 5, 1, 5, 9,
        ];
        Self::polyhedron(&vertices, &faces, r, detail)
    }

    /// Gélule d'axe Y centrée (CapsuleGeometry) : rayon, longueur de la partie droite.
    pub fn capsule(r: f32, length: f32, cap_segments: u32, radial: u32) -> Geo {
        let (cap_segments, radial) = (cap_segments.max(1), radial.max(3));
        let mut profile = Vec::new();
        for i in 0..=cap_segments {
            let a = -FRAC_PI_2 + i as f32 / cap_segments as f32 * FRAC_PI_2;
            profile.push((r * a.cos(), -length / 2.0 + r * a.sin(), a.cos(), a.sin()));
        }
        for i in 0..=cap_segments {
            let a = i as f32 / cap_segments as f32 * FRAC_PI_2;
            profile.push((r * a.cos(), length / 2.0 + r * a.sin(), a.cos(), a.sin()));
        }
        Self::revolve(&profile, radial)
    }

    /// Volume de révolution autour de Y d'un profil (rayon, hauteur, normale radiale, normale verticale).
    fn revolve(profile: &[(f32, f32, f32, f32)], segments: u32) -> Geo {
        let mut g = Geo::default();
        let rows: Vec<Vec<u32>> = profile
            .iter()
            .map(|&(rad, y, nr, ny)| {
                (0..=segments)
                    .map(|j| {
                        let (s, c) = (j as f32 / segments as f32 * TAU).sin_cos();
                        g.vertex(v(rad * s, y, rad * c), v(nr * s, ny, nr * c))
                    })
                    .collect()
            })
            .collect();
        for k in 0..rows.len().saturating_sub(1) {
            for j in 0..segments as usize {
                let (s, c) = ((j as f32 + 0.5) / segments as f32 * TAU).sin_cos();
                let (nr, ny) = ((profile[k].2 + profile[k + 1].2) / 2.0, (profile[k].3 + profile[k + 1].3) / 2.0);
                let out = v(nr * s, ny, nr * c);
                g.tri(rows[k][j], rows[k + 1][j], rows[k][j + 1], out);
                g.tri(rows[k + 1][j], rows[k + 1][j + 1], rows[k][j + 1], out);
            }
        }
        g
    }

    /// Tour (LatheGeometry) : profil [(rayon, hauteur)…] autour de l'axe Y.
    pub fn lathe(points: &[[f32; 2]], segments: u32) -> Geo {
        let n = points.len();
        let profile: Vec<(f32, f32, f32, f32)> = (0..n)
            .map(|i| {
                let a = points[i.saturating_sub(1)];
                let b = points[(i + 1).min(n - 1)];
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let len = (dx * dx + dy * dy).sqrt().max(1e-9);
                (points[i][0], points[i][1], dy / len, -dx / len)
            })
            .collect();
        Self::revolve(&profile, segments.max(3))
    }

    /// Profil plat extrudé (ExtrudeGeometry) : contour dans le plan XY, épaisseur `depth` centrée
    /// sur Z, arêtes chanfreinées de `bevel` (approximation de l'arrondi de three.js).
    pub fn extrude(contour: &[Vec2], depth: f32, bevel: f32) -> Geo {
        let mut pts: Vec<Vec2> = Vec::new();
        for p in contour {
            if pts.last().is_none_or(|q: &Vec2| q.distance(*p) > 1e-6) {
                pts.push(*p);
            }
        }
        if pts.len() > 2 && pts[0].distance(pts[pts.len() - 1]) < 1e-6 {
            pts.pop();
        }
        let mut g = Geo::default();
        if pts.len() < 3 {
            return g;
        }
        let area: f32 = (0..pts.len()).map(|i| pts[i].perp_dot(pts[(i + 1) % pts.len()])).sum::<f32>() / 2.0;
        if area < 0.0 {
            pts.reverse(); // sens trigonométrique : extérieur à droite des arêtes
        }
        let n = pts.len();
        // contour élargi (milieu du chanfrein) : chaque sommet poussé selon la bissectrice des arêtes
        let grown: Vec<Vec2> = (0..n)
            .map(|i| {
                let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
                let n1 = (b - a).perp().normalize_or_zero() * -1.0;
                let n2 = (c - b).perp().normalize_or_zero() * -1.0;
                let m = (n1 + n2).normalize_or_zero();
                let k = m.dot(n1).max(0.3);
                b + m * (bevel / k)
            })
            .collect();
        let half = depth / 2.0;
        let rings: Vec<(Vec<Vec2>, f32)> = if bevel > 0.0 {
            vec![(pts.clone(), -half), (grown.clone(), -half + bevel), (grown, half - bevel), (pts.clone(), half)]
        } else {
            vec![(pts.clone(), -half), (pts.clone(), half)]
        };
        // côtés : faces plates (comme computeVertexNormals sur une géométrie non indexée)
        for w in rings.windows(2) {
            let ((ra, za), (rb, zb)) = (&w[0], &w[1]);
            for i in 0..n {
                let j = (i + 1) % n;
                let (a0, a1, b0, b1) = (ra[i].extend(*za), ra[j].extend(*za), rb[i].extend(*zb), rb[j].extend(*zb));
                let out = (a1 - a0).cross(b0 - a0);
                let edge = (pts[j] - pts[i]).perp() * -1.0;
                let facing = if out.dot(edge.extend(0.0)) >= 0.0 { out } else { -out };
                let ids = [a0, a1, b1, b0].map(|p| g.vertex(p, facing));
                g.tri(ids[0], ids[1], ids[2], facing);
                g.tri(ids[0], ids[2], ids[3], facing);
            }
        }
        // faces avant et arrière : triangulation du contour (oreilles)
        let tris = triangulate(&pts);
        for (z, n_cap) in [(half, Vec3::Z), (-half, Vec3::NEG_Z)] {
            let ids: Vec<u32> = pts.iter().map(|p| g.vertex(p.extend(z), n_cap)).collect();
            for t in &tris {
                g.tri(ids[t[0]], ids[t[1]], ids[t[2]], n_cap);
            }
        }
        g
    }

    /// Géométrie non indexée à normales plates (matière « flat »).
    fn flat(&self) -> Geo {
        let mut g = Geo::default();
        for t in self.idx.chunks_exact(3) {
            let (a, b, c) = (self.pos[t[0] as usize], self.pos[t[1] as usize], self.pos[t[2] as usize]);
            let n = (b - a).cross(c - a);
            for p in [a, b, c] {
                g.vertex(p, n);
            }
            let k = g.pos.len() as u32;
            g.idx.extend([k - 3, k - 2, k - 1]);
        }
        g
    }
}

/// Triangulation d'un polygone simple (sens trigonométrique) par oreilles.
fn triangulate(pts: &[Vec2]) -> Vec<[usize; 3]> {
    let mut idx: Vec<usize> = (0..pts.len()).collect();
    let mut out = Vec::new();
    let inside = |p: Vec2, a: Vec2, b: Vec2, c: Vec2| {
        let d1 = (b - a).perp_dot(p - a);
        let d2 = (c - b).perp_dot(p - b);
        let d3 = (a - c).perp_dot(p - c);
        d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0
    };
    let mut guard = 0;
    while idx.len() > 3 && guard < 10_000 {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for i in 0..n {
            let (ia, ib, ic) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if (b - a).perp_dot(c - b) <= 1e-12 {
                continue; // angle rentrant ou plat
            }
            if idx.iter().any(|&k| k != ia && k != ib && k != ic && inside(pts[k], a, b, c)) {
                continue;
            }
            out.push([ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            // polygone dégénéré : on retire un sommet plat pour avancer
            out.push([idx[0], idx[1], idx[2]]);
            idx.remove(1);
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

/// Contour fait de segments et d'arcs (THREE.Shape) : `move_to`, `line_to`, `absarc`.
#[derive(Clone, Default)]
pub struct Shape {
    items: Vec<ShapeItem>,
}

#[derive(Clone)]
enum ShapeItem {
    Point(Vec2),
    Arc { center: Vec2, r: f32, start: f32, end: f32, clockwise: bool },
}

impl Shape {
    pub fn new() -> Shape {
        Shape::default()
    }

    pub fn from_points(points: &[[f32; 2]]) -> Shape {
        Shape { items: points.iter().map(|p| ShapeItem::Point(Vec2::from(*p))).collect() }
    }

    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.items.push(ShapeItem::Point(Vec2::new(x, y)));
        self
    }

    pub fn line_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.items.push(ShapeItem::Point(Vec2::new(x, y)));
        self
    }

    /// Arc de cercle (centre, rayon, angles de début et de fin, sens horaire).
    pub fn absarc(&mut self, x: f32, y: f32, r: f32, start: f32, end: f32, clockwise: bool) -> &mut Self {
        self.items.push(ShapeItem::Arc { center: Vec2::new(x, y), r, start, end, clockwise });
        self
    }

    /// Points du contour ; les arcs en `2 × divisions` segments (comme `getPoints`).
    pub fn points(&self, divisions: u32) -> Vec<Vec2> {
        let mut out = Vec::new();
        for it in &self.items {
            match *it {
                ShapeItem::Point(p) => out.push(p),
                ShapeItem::Arc { center, r, start, end, clockwise } => {
                    let mut delta = end - start;
                    while delta < 0.0 {
                        delta += TAU;
                    }
                    while delta > TAU {
                        delta -= TAU;
                    }
                    if delta < f32::EPSILON {
                        delta = if (end - start).abs() < f32::EPSILON { 0.0 } else { TAU };
                    }
                    if clockwise {
                        delta = if delta == 0.0 { -TAU } else { delta - TAU };
                    }
                    let steps = divisions.max(1) * 2;
                    for k in 0..=steps {
                        let a = start + delta * k as f32 / steps as f32;
                        out.push(center + Vec2::new(a.cos(), a.sin()) * r);
                    }
                }
            }
        }
        out
    }
}

// ------------------------------------------------------------------ options et matières
/// Placement et options d'une pièce (`{ p, r, q, s, seg, open, … }` de la version JavaScript).
#[derive(Clone, Copy, Default)]
pub struct O {
    pub p: Option<Vec3>,
    pub r: Option<[f32; 3]>,
    pub q: Option<Quat>,
    pub s: Option<Vec3>,
    pub seg: Option<u32>,
    pub open: bool,
    pub start: f32,
    pub arc: Option<f32>,
    pub theta0: f32,
    pub theta: Option<f32>,
    pub tube: Option<u32>,
    pub detail: Option<u32>,
    pub cap: Option<u32>,
    pub r2: Option<f32>,
    pub curve: Option<u32>,
    pub bevel: f32,
}

pub fn o() -> O {
    O::default()
}

impl O {
    pub fn p(mut self, p: impl Into<Vec3>) -> O {
        self.p = Some(p.into());
        self
    }
    /// angles d'Euler XYZ
    pub fn r(mut self, r: [f32; 3]) -> O {
        self.r = Some(r);
        self
    }
    pub fn q(mut self, q: Quat) -> O {
        self.q = Some(q);
        self
    }
    pub fn s(mut self, s: impl Into<Vec3>) -> O {
        self.s = Some(s.into());
        self
    }
    pub fn seg(mut self, n: u32) -> O {
        self.seg = Some(n);
        self
    }
    pub fn open(mut self) -> O {
        self.open = true;
        self
    }
    pub fn start(mut self, a: f32) -> O {
        self.start = a;
        self
    }
    pub fn arc(mut self, a: f32) -> O {
        self.arc = Some(a);
        self
    }
    pub fn theta0(mut self, a: f32) -> O {
        self.theta0 = a;
        self
    }
    pub fn theta(mut self, a: f32) -> O {
        self.theta = Some(a);
        self
    }
    pub fn tube(mut self, n: u32) -> O {
        self.tube = Some(n);
        self
    }
    pub fn detail(mut self, n: u32) -> O {
        self.detail = Some(n);
        self
    }
    pub fn cap(mut self, n: u32) -> O {
        self.cap = Some(n);
        self
    }
    pub fn r2(mut self, r: f32) -> O {
        self.r2 = Some(r);
        self
    }
    pub fn curve(mut self, n: u32) -> O {
        self.curve = Some(n);
        self
    }
    pub fn bevel(mut self, b: f32) -> O {
        self.bevel = b;
        self
    }

    fn matrix(&self, p: Option<Vec3>) -> Mat4 {
        let q = self.q.unwrap_or_else(|| self.r.map_or(Quat::IDENTITY, |r| euler(r[0], r[1], r[2])));
        Mat4::from_scale_rotation_translation(self.s.unwrap_or(Vec3::ONE), q, p.or(self.p).unwrap_or(Vec3::ZERO))
    }
}

/// Finition d'une matière (`{ rough, metal, opacity, emissive, flat, double }`).
#[derive(Clone, Copy)]
pub struct MatOpts {
    pub rough: f32,
    pub metal: f32,
    pub opacity: f32,
    pub emissive: Option<Col>,
    pub flat: bool,
    pub double: bool,
}

pub fn mo() -> MatOpts {
    MatOpts { rough: 0.8, metal: 0.0, opacity: 1.0, emissive: None, flat: false, double: false }
}

impl MatOpts {
    pub fn rough(mut self, v: f32) -> MatOpts {
        self.rough = v;
        self
    }
    pub fn metal(mut self, v: f32) -> MatOpts {
        self.metal = v;
        self
    }
    pub fn opacity(mut self, v: f32) -> MatOpts {
        self.opacity = v;
        self
    }
    pub fn emissive(mut self, c: impl Into<Col>) -> MatOpts {
        self.emissive = Some(c.into());
        self
    }
    pub fn flat(mut self) -> MatOpts {
        self.flat = true;
        self
    }
    pub fn double(mut self) -> MatOpts {
        self.double = true;
        self
    }
}

/// Une matière déclarée (`k.mat(…)` renvoie ce repère).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct M(usize);

/// Un objet construit : un maillage par matière, point visé, centre de l'objectif (caméras).
pub struct Built {
    pub parts: Vec<(Arc<MeshData>, Material)>,
    /// transformation de l'ensemble (personnage allongé…)
    pub root: Mat4,
    pub aim: Option<Vec3>,
    pub lens: Option<Vec3>,
}

impl Built {
    pub fn aim(mut self, p: Vec3) -> Built {
        self.aim = Some(p);
        self
    }

    pub fn lens(mut self, p: Vec3) -> Built {
        self.lens = Some(p);
        self
    }
}

#[derive(Default)]
pub struct Kit {
    mats: Vec<(String, Col, MatOpts)>,
    parts: Vec<Vec<Geo>>,
}

impl Kit {
    pub fn new() -> Kit {
        Kit::default()
    }

    /// Déclare une matière (couleur et finition) ; un même nom remplace la précédente.
    pub fn mat(&mut self, name: &str, color: impl Into<Col>, opts: MatOpts) -> M {
        let color = color.into();
        if let Some(i) = self.mats.iter().position(|(n, _, _)| n == name) {
            self.mats[i] = (name.to_owned(), color, opts);
            return M(i);
        }
        self.mats.push((name.to_owned(), color, opts));
        self.parts.push(Vec::new());
        M(self.mats.len() - 1)
    }

    /// Ajoute une géométrie placée (p, r ou q, s).
    pub fn add(&mut self, geo: Geo, m: M, o: O) -> &mut Self {
        let placed = geo.transform(o.matrix(None));
        self.parts[m.0].push(placed);
        self
    }

    fn put(&mut self, geo: Geo, m: M, p: Vec3, o: O) -> &mut Self {
        let placed = geo.transform(o.matrix(Some(p)));
        self.parts[m.0].push(placed);
        self
    }

    pub fn boxed(&mut self, w: f32, h: f32, d: f32, m: M, p: impl Into<Vec3>, o: O) -> &mut Self {
        self.put(Geo::boxed(w, h, d), m, p.into(), o)
    }

    /// Boîte aux arêtes arrondies (coussins, carrosseries, meubles).
    pub fn rbox(&mut self, w: f32, h: f32, d: f32, radius: f32, m: M, p: impl Into<Vec3>, o: O) -> &mut Self {
        let rad = radius.min(w / 2.0 - 0.001).min(h / 2.0 - 0.001).min(d / 2.0 - 0.001).max(0.001);
        self.put(Geo::rounded_box(w, h, d, 2, rad), m, p.into(), o)
    }

    /// Cylindre vertical centré en p (r_top en haut, r_bot en bas).
    pub fn cyl(&mut self, r_top: f32, r_bot: f32, h: f32, m: M, p: impl Into<Vec3>, o: O) -> &mut Self {
        self.put(Geo::cylinder(r_top, r_bot, h, o.seg.unwrap_or(16), o.open, o.start, o.arc.unwrap_or(TAU)), m, p.into(), o)
    }

    pub fn ball(&mut self, r: f32, m: M, p: impl Into<Vec3>, o: O) -> &mut Self {
        let seg = o.seg.unwrap_or(16);
        self.put(Geo::sphere(r, seg, ((seg as f32 * 0.7).round() as u32).max(6), o.theta0, o.theta.unwrap_or(PI)), m, p.into(), o)
    }

    pub fn cone(&mut self, r: f32, h: f32, m: M, p: impl Into<Vec3>, o: O) -> &mut Self {
        self.put(Geo::cylinder(0.0, r, h, o.seg.unwrap_or(16), o.open, 0.0, TAU), m, p.into(), o)
    }

    pub fn torus(&mut self, big_r: f32, r: f32, m: M, p: impl Into<Vec3>, o: O) -> &mut Self {
        self.put(Geo::torus(big_r, r, o.tube.unwrap_or(8), o.seg.unwrap_or(24), o.arc.unwrap_or(TAU)), m, p.into(), o)
    }

    /// Pierre, feuillage : polyèdre facetté.
    pub fn ico(&mut self, r: f32, m: M, p: impl Into<Vec3>, o: O) -> &mut Self {
        self.put(Geo::icosahedron(r, o.detail.unwrap_or(1)), m, p.into(), o)
    }

    /// Orientation qui amène l'axe Y sur le segment a → b : longueur, rotation, milieu.
    pub fn along(a: Vec3, b: Vec3) -> (f32, Quat, Vec3) {
        let d = b - a;
        let len = d.length();
        let q = if len > 1e-6 { Quat::from_rotation_arc(Vec3::Y, d / len) } else { Quat::IDENTITY };
        (len, q, (a + b) * 0.5)
    }

    /// Gélule (membre, accoudoir…) entre deux points ; o.s : aplatissement éventuel.
    pub fn capsule(&mut self, a: impl Into<Vec3>, b: impl Into<Vec3>, r: f32, m: M, o: O) -> &mut Self {
        let (len, q, mid) = Kit::along(a.into(), b.into());
        let geo = Geo::capsule(r, len.max(0.0005), o.cap.unwrap_or(4), o.seg.unwrap_or(12));
        self.add(geo, m, O { q: Some(q), p: Some(mid), s: o.s, ..O::default() })
    }

    /// Tube droit entre deux points (rayon r en a, r2 en b).
    pub fn rod(&mut self, a: impl Into<Vec3>, b: impl Into<Vec3>, r: f32, m: M, o: O) -> &mut Self {
        let (len, q, mid) = Kit::along(a.into(), b.into());
        let geo = Geo::cylinder(o.r2.unwrap_or(r), r, len.max(0.0005), o.seg.unwrap_or(10), o.open, 0.0, TAU);
        self.add(geo, m, O { q: Some(q), p: Some(mid), s: o.s, ..O::default() })
    }

    /// Volume de révolution : profil [[rayon, hauteur], …] autour de l'axe Y.
    pub fn lathe(&mut self, points: &[[f32; 2]], m: M, p: impl Into<Vec3>, o: O) -> &mut Self {
        let pts: Vec<[f32; 2]> = points.iter().map(|[r, y]| [r.max(0.0005), *y]).collect();
        self.put(Geo::lathe(&pts, o.seg.unwrap_or(20)), m, p.into(), o)
    }

    /// Profil extrudé dans le plan XY, épaisseur `depth` centrée sur Z (o.bevel : arêtes adoucies,
    /// o.curve : finesse des arcs).
    pub fn extrude(&mut self, shape: &Shape, depth: f32, m: M, o: O) -> &mut Self {
        let pts = shape.points(o.curve.unwrap_or(10));
        self.add(Geo::extrude(&pts, depth, o.bevel), m, o)
    }

    /// Profil extrudé à partir d'une liste de points.
    pub fn extrude_points(&mut self, points: &[[f32; 2]], depth: f32, m: M, o: O) -> &mut Self {
        self.extrude(&Shape::from_points(points), depth, m, o)
    }

    /// Objet final : un maillage par matière, qui porte et reçoit les ombres.
    pub fn build(self) -> Built {
        let mut parts = Vec::new();
        for ((_, color, spec), geos) in self.mats.iter().zip(self.parts) {
            if geos.is_empty() {
                continue;
            }
            let mut b = Builder::default();
            for g in geos {
                let g = if spec.flat { g.flat() } else { g };
                let base = b.len();
                for (p, n) in g.pos.iter().zip(&g.nrm) {
                    b.vertex(*p, *n);
                }
                b.indices_offset(base, &g.idx);
            }
            let transparent = spec.opacity < 1.0;
            let material = Material {
                color: color.0,
                roughness: spec.rough,
                metalness: spec.metal,
                opacity: spec.opacity,
                depth_write: !transparent,
                double_sided: spec.double,
                surface: if spec.flat { Surface::Flat } else { Surface::Lit },
                emissive: spec.emissive.map_or([0.0; 3], |c| c.0.map(|x| x * 0.5)),
                cast_shadow: !transparent || spec.opacity >= 0.5,
                receive_shadow: true,
                ..Material::default()
            };
            parts.push((Arc::new(b.finish()), material));
        }
        Built { parts, root: Mat4::IDENTITY, aim: None, lens: None }
    }
}
