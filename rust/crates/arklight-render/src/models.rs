//! Modèles 3D des projecteurs et de leurs pieds, portage de `web/js/viewport/models.js` et
//! `stands.js` (mêmes formes, mêmes matériaux, mêmes lumières). Un modèle est construit une fois
//! par type de projecteur ; à chaque image il reçoit la couleur de ses pixels.

use crate::mesh::{self, MeshData};
use crate::{LightKind, Material, Surface};
use arklight_core::accessories::Effective;
use arklight_core::color::srgb_to_linear;
use arklight_core::library::FixtureType;
use glam::{Mat4, Quat, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

/// Calibrage : 1 candela = lux à 1 m × 0,02 (un éclairement d'environ 160 lux donne un gris moyen
/// correctement exposé, exposition 1).
pub const LIGHT_SCALE: f32 = 0.02;
/// Gain des faces émettrices.
pub const EMISSIVE_GAIN: f32 = 1.6;
/// Longueur des faisceaux visibles (mètres).
pub const BEAM_LENGTH: f32 = 7.0;

/// Couleur hexadécimale sRGB (0xRRGGBB) → sRGB linéaire.
pub fn hex(c: u32) -> [f32; 3] {
    let ch = |s: u32| srgb_to_linear(((c >> s) & 0xff) as f64 / 255.0) as f32;
    [ch(16), ch(8), ch(0)]
}

pub fn standard(color: u32, roughness: f32, metalness: f32) -> Material {
    Material { color: hex(color), roughness, metalness, ..Material::default() }
}

fn body_mat() -> Material {
    standard(0x1c1f23, 0.55, 0.35)
}

/// Ce que reçoit une pièce à chaque image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Role {
    /// rien : pièce inerte
    Body,
    /// face émettrice d'un pixel
    Pixel(usize),
    /// face émettrice de la couleur moyenne
    Average,
}

/// Une pièce : maillage, position dans le modèle, matériau.
#[derive(Clone)]
pub struct Part {
    pub mesh: Arc<MeshData>,
    pub local: Mat4,
    pub material: Material,
    pub role: Role,
    /// compte pour l'encombrement (sélection, hauteur du pied)
    pub pick: bool,
}

/// Une lumière du modèle, alimentée par la moyenne des pixels `from..to`.
#[derive(Clone, Copy)]
pub struct ModelLight {
    pub position: Vec3,
    pub direction: Vec3,
    pub kind: LightKind,
    pub candela: f32,
    pub from: usize,
    pub to: usize,
}

/// Un faisceau visible (brume), alimenté par la moyenne des pixels `from..to`.
#[derive(Clone)]
pub struct ModelBeam {
    pub mesh: Arc<MeshData>,
    pub position: Vec3,
    /// intensité = min(1,5, niveau) × brume × gain
    pub gain: f32,
    pub from: usize,
    pub to: usize,
}

#[derive(Clone, Default)]
pub struct FixtureModel {
    pub parts: Vec<Part>,
    pub lights: Vec<ModelLight>,
    pub beams: Vec<ModelBeam>,
    /// teinte d'une gélatine montée : toute la lumière émise la prend
    pub tint: Option<[f32; 3]>,
}

fn at(p: [f32; 3]) -> Mat4 {
    Mat4::from_translation(Vec3::from(p))
}

impl FixtureModel {
    fn part(&mut self, mesh: Arc<MeshData>, local: Mat4, material: Material, role: Role) {
        self.parts.push(Part { mesh, local, material, role, pick: true });
    }

    /// Encombrement dans le repère du monde (pièces sélectionnables), pour `root`.
    pub fn bounds(&self, root: Mat4) -> Option<(Vec3, Vec3)> {
        let mut lo = Vec3::splat(f32::INFINITY);
        let mut hi = Vec3::splat(f32::NEG_INFINITY);
        for p in self.parts.iter().filter(|p| p.pick) {
            let (a, b) = p.mesh.bounds;
            let m = root * p.local;
            for i in 0..8 {
                let c = Vec3::new(if i & 1 == 0 { a.x } else { b.x }, if i & 2 == 0 { a.y } else { b.y }, if i & 4 == 0 { a.z } else { b.z });
                let w = m.transform_point3(c);
                lo = lo.min(w);
                hi = hi.max(w);
            }
        }
        lo.x.is_finite().then_some((lo, hi))
    }

    /// Candelas d'une source omnidirectionnelle (lux à 1 m si connu, sinon lumens / 4π).
    fn omni_candela(eff: &Effective, share: f32) -> f32 {
        let cd = eff.lux_at1m.unwrap_or_else(|| eff.lumens.filter(|l| *l != 0.0).unwrap_or(500.0) / (4.0 * std::f64::consts::PI));
        cd as f32 * share * LIGHT_SCALE
    }

    /// Candelas sur l'axe d'une source à faisceau (lux à 1 m si connu, sinon lumens / angle solide).
    fn spot_candela(eff: &Effective, half: f32) -> f32 {
        let cd = eff.lux_at1m.unwrap_or_else(|| eff.lumens.filter(|l| *l != 0.0).unwrap_or(500.0) / (2.0 * std::f64::consts::PI * (1.0 - (half as f64).cos())));
        cd as f32 * LIGHT_SCALE
    }

    /// Spot (et faisceau visible si l'angle fait au plus 90°) à l'avant d'une optique.
    fn add_spot(&mut self, ty: &FixtureType, eff: &Effective, lens_radius: f32, lens_z: f32) {
        let deg = eff.beam_angle.filter(|a| *a != 0.0).unwrap_or(40.0).min(170.0) as f32;
        let half = (deg / 2.0).to_radians();
        self.lights.push(ModelLight {
            position: Vec3::new(0.0, 0.0, lens_z),
            direction: Vec3::Z,
            kind: LightKind::Spot { angle: (half * 1.25).min(FRAC_PI_2 - 0.01), penumbra: 0.55 },
            candela: Self::spot_candela(eff, half),
            from: 0,
            to: ty.pixels,
        });
        if eff.beam_angle.is_some_and(|a| a <= 90.0) {
            let r = lens_radius * 0.9;
            self.beams.push(ModelBeam { mesh: mesh::beam(r, half.tan() * BEAM_LENGTH + r, BEAM_LENGTH), position: Vec3::new(0.0, 0.0, lens_z), gain: 0.35, from: 0, to: ty.pixels });
        }
    }

    /// Modèle d'un type de projecteur (selon sa forme), avec son zoom et ses accessoires.
    pub fn build(ty: &FixtureType, eff: &Effective) -> FixtureModel {
        let d = |k: &str, v: f64| ty.dim(k).unwrap_or(v) as f32;
        let mut m = FixtureModel::default();
        let n = ty.pixels;
        match ty.geometry.kind.as_str() {
            "box-device" => {
                let (w, h, dp) = (d("width", 0.1), d("height", 0.1), d("depth", 0.03));
                m.part(mesh::boxed(w, h, dp), Mat4::IDENTITY, standard(0x111316, 0.45, 0.6), Role::Body);
                for x in [-w / 2.0 + 0.011, -w / 2.0 + 0.066] {
                    m.part(mesh::cylinder(0.0025, 0.0045, 0.054, 12), at([x, h / 2.0 + 0.027, 0.0]), standard(0x0b0c0e, 0.8, 0.0), Role::Body);
                }
                // logo blanc (triangle) sur la face avant
                let logo = Material { color: hex(0xe8eaed), surface: Surface::Unlit, ..Material::default() };
                let local = at([0.0, h * 0.22, dp / 2.0 + 0.0005]) * Mat4::from_rotation_z(PI / 2.0);
                m.parts.push(Part { mesh: mesh::circle(0.009, 3), local, material: logo, role: Role::Body, pick: false });
            }
            "tube" => {
                // axe = Y local, pixel 1 en bas
                let (l, r) = (d("length", 1.0), d("diameter", 0.04) / 2.0);
                let cap = (0.035f32).min(l * 0.04);
                let zone = l - 2.0 * cap;
                let seg = zone / n.max(1) as f32;
                let lens = Material { color: hex(0x8e939b), roughness: 0.35, metalness: 0.0, emissive_gain: EMISSIVE_GAIN, ..Material::default() };
                for i in 0..n {
                    m.part(mesh::tube(r, r, seg * 0.985, 20), at([0.0, -zone / 2.0 + seg * (i as f32 + 0.5), 0.0]), lens, Role::Pixel(i));
                }
                for sgn in [-1.0, 1.0] {
                    m.part(mesh::cylinder(r * 1.1, r * 1.1, cap, 20), at([0.0, sgn * (zone / 2.0 + cap / 2.0), 0.0]), body_mat(), Role::Body);
                }
                // lumières réparties le long du tube (qualité 2 de la version three.js)
                let count = n.clamp(1, 2);
                for i in 0..count {
                    m.lights.push(ModelLight {
                        position: Vec3::new(0.0, -zone / 2.0 + zone / count as f32 * (i as f32 + 0.5), 0.0),
                        direction: Vec3::Y,
                        kind: LightKind::Point,
                        candela: Self::omni_candela(eff, 1.0 / count as f32),
                        from: n * i / count,
                        to: n * (i + 1) / count,
                    });
                }
            }
            "par" => {
                let (r, depth) = (d("diameter", 0.2) / 2.0, d("depth", 0.2));
                m.part(mesh::cylinder(r, r * 0.85, depth, 28), Mat4::from_rotation_x(FRAC_PI_2), body_mat(), Role::Body);
                let lens = Material { color: hex(0x30343a), roughness: 0.2, metalness: 0.0, emissive_gain: EMISSIVE_GAIN, ..Material::default() };
                let lens_at = |x: f32, y: f32| at([x, y, depth / 2.0 + 0.001]);
                let lenses = d("lenses", 1.0) as u32;
                if lenses <= 1 {
                    m.part(mesh::circle(r * 0.86, 32), lens_at(0.0, 0.0), lens, Role::Average);
                } else if lenses <= 4 {
                    // couronne sans optique centrale : 3 en triangle (AX5), 4 en carré (AX7)
                    let lr = r * if lenses == 3 { 0.34 } else { 0.3 };
                    for i in 0..lenses {
                        let a = i as f32 / lenses as f32 * TAU + if lenses == 4 { PI / 4.0 } else { PI / 2.0 };
                        m.part(mesh::circle(lr, 24), lens_at(a.cos() * r * 0.42, a.sin() * r * 0.42), lens, Role::Average);
                    }
                } else {
                    // optique centrale + couronne (AX9 : 7, AX10 : 9)
                    let lr = r * 0.27;
                    let ring = lenses - 1;
                    m.part(mesh::circle(lr, 24), lens_at(0.0, 0.0), lens, Role::Average);
                    for i in 0..ring {
                        let a = i as f32 / ring as f32 * TAU;
                        m.part(mesh::circle(lr, 24), lens_at(a.cos() * r * 0.58, a.sin() * r * 0.58), lens, Role::Average);
                    }
                }
                // lyre
                for sgn in [-1.0, 1.0] {
                    m.part(mesh::boxed(0.012, r * 2.3, 0.03), at([sgn * (r + 0.015), -r * 0.25, 0.0]), body_mat(), Role::Body);
                }
                m.part(mesh::boxed(r * 2.0 + 0.042, 0.012, 0.03), at([0.0, -r * 1.4, 0.0]), body_mat(), Role::Body);
                m.add_spot(ty, eff, r * 0.86, depth / 2.0 + 0.002);
            }
            "panel" | "brick" => {
                let (w, h, dp) = (d("width", 0.3), d("height", 0.3), d("depth", 0.1));
                m.part(mesh::boxed(w, h, dp), Mat4::IDENTITY, body_mat(), Role::Body);
                // face émettrice : une zone par pixel (grille de 2 rangées si plusieurs pixels)
                let layout = ty.extra.get("pixelLayout");
                let lay = |k: &str| layout.and_then(|l| l.get(k)).and_then(|v| v.as_f64()).map(|v| v as usize);
                let cols = lay("cols").unwrap_or(if n > 1 { n.div_ceil(2) } else { 1 });
                let rows = lay("rows").unwrap_or(if n > 1 { 2 } else { 1 });
                let by_column = layout.and_then(|l| l.get("order")).and_then(|v| v.as_str()) == Some("column");
                let (cw, ch) = (w * 0.9 / cols as f32, h * 0.86 / rows as f32);
                for p in 0..n {
                    let (col, row) = if by_column { (p / rows, p % rows) } else { (p % cols, p / cols) };
                    let mat = Material { color: hex(0x30343a), roughness: 0.25, metalness: 0.0, emissive_gain: EMISSIVE_GAIN, ..Material::default() };
                    m.part(mesh::plane(cw * 0.94, ch * 0.92), at([-w * 0.45 + cw * (col as f32 + 0.5), h * 0.43 - ch * (row as f32 + 0.5), dp / 2.0 + 0.001]), mat, Role::Pixel(p));
                }
                m.add_spot(ty, eff, w.min(h) * 0.43, dp / 2.0 + 0.002);
            }
            "bar" => {
                // barre à pixels (AX2) : axe = X, LED sur la face +Z
                let (l, h, dp) = (d("length", 1.0), d("height", 0.05), d("depth", 0.05));
                m.part(mesh::boxed(l, h, dp), Mat4::IDENTITY, body_mat(), Role::Body);
                let pitch = l * 0.96 / n.max(1) as f32;
                for p in 0..n {
                    let mat = Material { color: hex(0x2a2e33), roughness: 0.2, metalness: 0.0, emissive_gain: EMISSIVE_GAIN, ..Material::default() };
                    m.part(mesh::circle(pitch.min(h) * 0.38, 24), at([-l * 0.48 + pitch * (p as f32 + 0.5), 0.0, dp / 2.0 + 0.001]), mat, Role::Pixel(p));
                }
                // quelques spots répartis le long de la barre (chacun moyenne un groupe de pixels)
                let half = (eff.beam_angle.filter(|a| *a != 0.0).unwrap_or(21.0) as f32 / 2.0).to_radians();
                let count = n.clamp(1, 4);
                let cd = eff.lux_at1m.unwrap_or(2000.0) as f32 * LIGHT_SCALE / count as f32;
                for i in 0..count {
                    let x = -l / 2.0 + l / count as f32 * (i as f32 + 0.5);
                    let position = Vec3::new(x, 0.0, dp / 2.0 + 0.002);
                    let (from, to) = (n * i / count, n * (i + 1) / count);
                    m.lights.push(ModelLight { position, direction: Vec3::Z, kind: LightKind::Spot { angle: (half * 1.4).min(FRAC_PI_2 - 0.01), penumbra: 0.5 }, candela: cd, from, to });
                    let r = (l / count as f32).min(h) * 0.35;
                    m.beams.push(ModelBeam { mesh: mesh::beam(r, half.tan() * BEAM_LENGTH + r, BEAM_LENGTH), position, gain: 0.3, from, to });
                }
            }
            _ => {
                // ampoule (culot en haut), et forme inconnue
                let (dia, l) = (d("diameter", 0.1), d("length", 0.15));
                let r = dia / 2.0;
                let glass = Material { color: hex(0xbfc3c9), roughness: 0.3, metalness: 0.0, emissive_gain: EMISSIVE_GAIN, ..Material::default() };
                m.part(mesh::sphere(r, 28, 20), Mat4::IDENTITY, glass, Role::Average);
                m.part(mesh::cylinder(r * 0.55, r * 0.8, l - dia, 24), at([0.0, r + (l - dia) / 2.0 - r * 0.3, 0.0]), body_mat(), Role::Body);
                m.lights.push(ModelLight { position: Vec3::ZERO, direction: Vec3::NEG_Y, kind: LightKind::Point, candela: Self::omni_candela(eff, 1.0), from: 0, to: n });
            }
        }
        m.add_accessories(ty, eff);
        m
    }

    /// Accessoires (boîte à lumière, dôme, lanterne, grille, gélatine) posés devant la face avant
    /// (+Z) ; la toile avant s'allume de la couleur émise et la source passe sur elle
    /// (`fixture-accessories.js`).
    fn add_accessories(&mut self, ty: &FixtureType, eff: &Effective) {
        let Some(acc) = eff.accessories else { return };
        let [hx, hy, hz] = body_half_extents(ty);
        let round = ty.geometry.kind == "par";
        let base = at([0.0, 0.0, hz]);
        let fabric = Material { double_sided: true, ..standard(0x151517, 0.95, 0.0) };
        let diffuser = Material { double_sided: true, emissive_gain: EMISSIVE_GAIN, ..standard(0xf2f2ee, 0.9, 0.0) };
        let mut parts = Vec::new();
        let mut add = |mesh: Arc<MeshData>, local: Mat4, material: Material, role: Role| parts.push(Part { mesh, local: base * local, material, role, pick: false });
        // frustum (boîte à lumière ou dôme) le long de +Z : ouverture arrière, face avant, profondeur
        let frustum = |back: f32, front: f32, depth: f32, sides: u32| {
            let turn = if sides == 4 { Mat4::from_rotation_z(PI / 4.0) } else { Mat4::IDENTITY };
            (mesh::cylinder_part(front, back, depth, sides, true, 0.0, TAU), at([0.0, 0.0, depth / 2.0]) * turn * Mat4::from_rotation_x(FRAC_PI_2))
        };
        let mut light_z = hz;
        let mut soft = 0.0;
        // surface où se posent grille et gélatine : (largeur, hauteur, z)
        let mut face = Some((hx * 2.0, hy * 2.0, 0.0));
        if let Some(m) = acc.modifier {
            let (depth, d) = (m.depth as f32, m.d as f32);
            match m.kind.as_str() {
                "softbox" => {
                    let (w, h) = if m.fit { (hx * 2.0 * 1.08, hy * 2.0 * 1.08) } else { (m.w as f32, m.h as f32) };
                    let back = if round { (hx.min(hy) * 0.9).max(0.09) } else { hx.max(hy) * std::f32::consts::SQRT_2 };
                    let (shell, local) = frustum(back, w.max(h) / 2.0 * std::f32::consts::SQRT_2, depth, 4);
                    let squash = if w >= h { Vec3::new(1.0, h / w, 1.0) } else { Vec3::new(w / h, 1.0, 1.0) };
                    add(shell, Mat4::from_scale(squash) * local, fabric, Role::Body);
                    add(mesh::plane(w, h), at([0.0, 0.0, depth]), diffuser, Role::Average);
                    face = Some((w, h, depth));
                    light_z = hz + depth * 0.6;
                    soft = w.max(h);
                }
                "dome" => {
                    let (shell, local) = frustum(0.1, d / 2.0, depth, 16);
                    add(shell, local, fabric, Role::Body);
                    add(mesh::circle(d / 2.0, 40), at([0.0, 0.0, depth]), diffuser, Role::Average);
                    face = Some((d * 0.72, d * 0.72, depth));
                    light_z = hz + depth * 0.6;
                    soft = d;
                }
                _ => {
                    // lanterne : boule de tissu, col noir vers le projecteur
                    add(mesh::sphere(d / 2.0, 28, 18), at([0.0, 0.0, d / 2.0 + 0.08]), diffuser, Role::Average);
                    let (neck, local) = frustum(0.1, 0.16, 0.12, 16);
                    add(neck, local, fabric, Role::Body);
                    face = None;
                    light_z = hz + d / 2.0 + 0.08;
                    soft = d;
                }
            }
        }
        if let (Some(g), Some((w, h, z))) = (acc.grid, face) {
            // grille (nid d'abeille), cellules d'environ 5 cm
            let gd = g.depth as f32;
            let black = standard(0x0d0d0e, 0.8, 0.0);
            let nx = ((w / 0.06).round() as u32).max(3);
            let ny = ((h / 0.06).round() as u32).max(3);
            for i in 0..=nx {
                add(mesh::boxed(0.004, h, gd), at([-w / 2.0 + w / nx as f32 * i as f32, 0.0, z + gd / 2.0]), black, Role::Body);
            }
            for j in 0..=ny {
                add(mesh::boxed(w, 0.004, gd), at([0.0, -h / 2.0 + h / ny as f32 * j as f32, z + gd / 2.0]), black, Role::Body);
            }
        }
        if let Some(gel) = acc.gel {
            let n = u32::from_str_radix(gel.color.trim_start_matches('#'), 16).unwrap_or(0xffffff);
            let material = Material { opacity: if gel.group == "diffusion" { 0.75 } else { 0.5 }, double_sided: true, depth_write: false, ..standard(n, 0.3, 0.0) };
            let (fw, fh) = face.map_or((hx * 2.0, hy * 2.0), |f| (f.0, f.1));
            let z = face.map_or(0.0, |f| f.2) + 0.012 + acc.grid.map_or(0.0, |g| g.depth as f32);
            let sheet = if round && acc.modifier.is_none() { mesh::circle(hx.min(hy) * 1.02, 32) } else { mesh::plane(fw * 1.02, fh * 1.02) };
            add(sheet, at([0.0, 0.0, z]), material, Role::Body);
        }
        self.parts.extend(parts);
        // la source passe sur la toile ; une grande source fait des ombres douces
        if matches!(ty.geometry.kind.as_str(), "par" | "panel" | "brick")
            && let Some(spot) = self.lights.first_mut()
        {
            spot.position.z = light_z;
            if soft > 0.0
                && let LightKind::Spot { angle, .. } = spot.kind
            {
                spot.kind = LightKind::Spot { angle, penumbra: 1.0 };
            }
        }
        self.tint = acc.tint.map(|t| t.map(|c| c as f32));
    }
}

/// Demi-encombrement du corps d'un projecteur (x, y, z), comme `bodyHalfExtents`.
pub fn body_half_extents(ty: &FixtureType) -> [f32; 3] {
    let d = |k: &str| ty.dim(k).map(|v| v as f32);
    let or = |v: Option<f32>, x: f32| v.filter(|v| *v != 0.0).unwrap_or(x);
    match ty.geometry.kind.as_str() {
        "tube" => {
            let dia = or(d("diameter"), 0.04);
            [dia / 2.0, or(d("length"), 1.0) / 2.0, dia / 2.0]
        }
        "bar" => [or(d("length"), 1.0) / 2.0, or(d("height"), 0.05) / 2.0, or(d("depth"), 0.05) / 2.0],
        "par" => {
            let dia = or(d("diameter"), 0.2);
            [dia / 2.0, dia / 2.0, or(d("depth"), 0.2) / 2.0]
        }
        "bulb" => {
            let dia = or(d("diameter"), 0.1);
            [dia / 2.0, or(d("length"), dia) / 2.0, dia / 2.0]
        }
        _ => [or(d("width"), 0.3) / 2.0, or(d("height"), 0.3) / 2.0, or(d("depth"), 0.1) / 2.0],
    }
}


// ------------------------------------------------------------------ pieds
/// Allure d'une famille de pieds : jambes (rayon au sol, hauteur du moyeu, épaisseur), mât
/// (diamètres des sections), roues, entretoises, manivelle (échelle).
struct Look {
    mat: fn() -> Material,
    leg_r: f32,
    hub: f32,
    leg: f32,
    risers: &'static [f32],
    cup: bool,
    wheels: Option<f32>,
    brace: bool,
    crank: Option<f32>,
    turtle: bool,
    head: bool,
}

fn chrome() -> Material {
    standard(0xb8bdc4, 0.32, 0.85)
}
fn black() -> Material {
    standard(0x17191c, 0.5, 0.55)
}
// acier et alu brossés du Long John Silver : clair même sans reflets d'environnement
fn steel() -> Material {
    standard(0xc4c9d0, 0.42, 0.45)
}
fn rubber() -> Material {
    standard(0x0d0e10, 0.9, 0.1)
}

const BASE: Look = Look { mat: chrome, leg_r: 0.42, hub: 0.55, leg: 0.009, risers: &[0.016, 0.013, 0.01], cup: false, wheels: None, brace: false, crank: None, turtle: false, head: false };

fn look(name: &str) -> Look {
    match name {
        "combo" => Look { mat: black, leg_r: 0.58, hub: 0.6, leg: 0.013, risers: &[0.024, 0.019], cup: true, ..BASE },
        // junior roulant (Avenger Roller 34) : trois sections, roues, entretoises
        "junior" => Look { mat: black, leg_r: 0.72, hub: 0.62, leg: 0.016, risers: &[0.032, 0.027, 0.022], cup: true, wheels: Some(0.05), brace: true, ..BASE },
        "windup" => Look { mat: black, leg_r: 0.8, hub: 0.8, leg: 0.018, risers: &[0.04, 0.032], cup: true, crank: Some(1.0), ..BASE },
        // Super Wind-Up (Manfrotto / Avenger 387XU) : manivelle, base roulante large
        "superwindup" => Look { mat: black, leg_r: 0.85, hub: 0.78, leg: 0.024, risers: &[0.055, 0.045, 0.036], cup: true, crank: Some(1.4), wheels: Some(0.065), brace: true, ..BASE },
        // Long John Silver (Avenger B7057FF) : acier et alu, cinq sections, grosses roues freinées
        "longjohn" => Look { mat: steel, leg_r: 0.95, hub: 0.9, leg: 0.026, risers: &[0.052, 0.046, 0.04, 0.034, 0.028], cup: true, wheels: Some(0.08), brace: true, ..BASE },
        "cstand" => Look { mat: chrome, leg_r: 0.38, hub: 0.22, leg: 0.011, risers: &[0.017, 0.014], turtle: true, head: true, ..BASE },
        "lowboy" => Look { mat: black, leg_r: 0.5, hub: 0.16, leg: 0.014, risers: &[0.024], cup: true, ..BASE },
        _ => BASE,
    }
}

/// Cylindre unité (axe Y, hauteur 1) placé entre deux points.
fn between(a: Vec3, b: Vec3) -> Mat4 {
    let d = b - a;
    let q = Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y));
    Mat4::from_scale_rotation_translation(Vec3::new(1.0, d.length(), 1.0), q, a + d * 0.5)
}

/// Pièces d'un pied dont le haut (bas du projecteur) est à `top` mètres, posé en (0, 0, 0) :
/// les jambes reposent au sol (ou sur leurs roues), le mât s'allonge jusqu'au projecteur.
pub fn stand(look_name: &str, top: f32) -> Vec<Part> {
    let l = look(look_name);
    let mat = (l.mat)();
    let mut parts = Vec::new();
    let mut add = |mesh: Arc<MeshData>, local: Mat4, material: Material| parts.push(Part { mesh, local, material, role: Role::Body, pick: false });
    let hub = l.hub.min(top * 0.55);
    let foot_lift = l.wheels.map_or(0.0, |w| w * 2.5); // la jambe s'arrête au-dessus de la roulette
    let collar_y = hub * 0.42;
    for i in 0..3 {
        let a = i as f32 * TAU / 3.0 + PI / 6.0;
        let foot_y = if l.turtle { 0.02 + i as f32 * 0.06 } else { foot_lift };
        let foot = Vec3::new(a.cos() * l.leg_r, foot_y, a.sin() * l.leg_r);
        let from = if l.turtle { Vec3::new(0.0, foot_y + 0.01, 0.0) } else { Vec3::new(0.0, hub, 0.0) };
        add(mesh::cylinder(l.leg, l.leg * 1.1, 1.0, 8), between(from, foot), mat);
        if l.turtle {
            // C-stand : pieds plats décalés en hauteur (base « tortue »)
            add(mesh::cylinder(0.016, 0.016, 0.02, 10), at([foot.x, foot_y / 2.0 + 0.01, foot.z]), mat);
        }
        if l.brace {
            add(mesh::cylinder(l.leg * 0.55, l.leg * 0.55, 1.0, 6), between(Vec3::new(0.0, collar_y, 0.0), from.lerp(foot, 0.55)), mat);
        }
        if let Some(r) = l.wheels {
            // roulette : roue en caoutchouc et chape (pivot vertical) au bout de la jambe
            let caster = Mat4::from_translation(Vec3::new(foot.x, 0.0, foot.z)) * Mat4::from_rotation_y(-(i as f32 * TAU / 3.0 + PI / 6.0));
            add(mesh::cylinder(r, r, r * 0.55, 18), caster * at([0.0, r, 0.0]) * Mat4::from_rotation_z(PI / 2.0), rubber());
            add(mesh::boxed(r * 0.75, r * 1.15, r * 0.35), caster * at([0.0, r * 1.45, 0.0]), mat);
            add(mesh::cylinder(r * 0.16, r * 0.16, r * 0.6, 8), caster * at([0.0, r * 2.2, 0.0]), mat);
        }
    }
    let r0 = l.risers[0];
    add(mesh::cylinder(r0 * 1.6, r0 * 1.6, 0.07, 12), at([0.0, hub, 0.0]), mat);
    if l.brace {
        add(mesh::cylinder(r0 * 1.45, r0 * 1.45, 0.05, 12), at([0.0, collar_y, 0.0]), mat);
    }
    // mât télescopique : sections qui se recouvrent, la plus fine en haut
    let base = if l.turtle { 0.0 } else { (hub - 0.05).min(if l.brace { collar_y - 0.03 } else { hub - 0.05 }) };
    let n = l.risers.len();
    let span = (top - base).max(0.05);
    let seg = span / n as f32 + 0.08;
    for (i, r) in l.risers.iter().enumerate() {
        let y0 = base + span / n as f32 * i as f32;
        let len = seg.min(top - y0).max(0.01);
        add(mesh::cylinder(*r, *r, 1.0, 12), Mat4::from_scale_rotation_translation(Vec3::new(1.0, len, 1.0), Quat::IDENTITY, Vec3::new(0.0, y0 + len / 2.0, 0.0)), mat);
    }
    if l.cup {
        // réception : 16 mm (combo) ou 28 mm (junior et plus gros)
        let big = r0 >= 0.03;
        let m = if big { mesh::cylinder(0.034, 0.03, 0.08, 12) } else { mesh::cylinder(0.026, 0.022, 0.06, 12) };
        add(m, at([0.0, top - 0.04, 0.0]), mat);
    }
    if let Some(k) = l.crank {
        let c = at([0.05 * k, hub + 0.3, 0.0]);
        add(mesh::boxed(0.1 * k, 0.14 * k, 0.08 * k), c, mat);
        add(mesh::cylinder(0.006 * k, 0.006 * k, 0.12 * k, 8), c * at([0.11 * k, -0.03 * k, 0.0]) * Mat4::from_rotation_z(PI / 2.0), mat);
    }
    if l.head {
        add(mesh::cylinder(0.03, 0.03, 0.025, 14), at([0.0, top - 0.04, 0.03]) * Mat4::from_rotation_x(PI / 2.0), mat);
    }
    parts
}
