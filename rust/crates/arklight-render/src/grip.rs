//! Grip dans la vue 3D (portage de `web/js/viewport/grip-models.js`) : cadres, drapeaux,
//! réflecteurs, cubes, sacs, pieds seuls ; et l'effet de chaque toile sur la lumière.

use crate::mesh;
use crate::models::{hex, stand, standard};
use crate::{Item, Material, transform};
use arklight_core::accessories::{Optics, filter_optics};
use arklight_core::library::{GripType, library};
use arklight_core::project::SceneObject;
use glam::{Mat4, Quat, Vec3};
use serde_json::Value;
use std::f32::consts::PI;

/// Comportement optique d'une toile (valeurs de la bibliothèque, approchées) ; le cadre vide n'a
/// aucun effet.
fn cloth_optics(id: &str) -> Option<Optics> {
    let c = library().cloths.get(id).filter(|c| c.opacity > 0.0)?;
    Some(Optics { t: c.t, reflect: c.reflect, diffuse: c.diffuse, solid: c.solid, tint: c.tint })
}

/// Surface du grip qui agit sur la lumière, dans le repère du monde.
#[derive(Debug, Clone, Copy)]
pub struct Occluder {
    pub center: Vec3,
    /// demi-largeur et demi-hauteur (vecteurs)
    pub u: Vec3,
    pub v: Vec3,
    pub n: Vec3,
    pub radius: f32,
    pub w: f32,
    pub h: f32,
    pub optics: Optics,
}

fn num(t: &GripType, k: &str) -> Option<f64> {
    t.extra.get(k).and_then(Value::as_f64)
}

fn text<'a>(t: &'a GripType, k: &str) -> Option<&'a str> {
    t.extra.get(k).and_then(Value::as_str)
}

fn frame_mat() -> Material {
    standard(0x9ea3a9, 0.35, 0.8)
}

/// Toile tendue sur un cadre : toile du grip, ou filtre du nuancier (diffusion, gélatine).
fn cloth_mat(id: Option<&str>) -> Material {
    let lib = library();
    let base = Material { double_sided: true, receive_shadow: true, ..Material::default() };
    if let Some(f) = id.filter(|id| !lib.cloths.contains_key(*id)).and_then(|id| lib.filter(id)) {
        let diffusion = f.group == "diffusion";
        let color = u32::from_str_radix(f.color.trim_start_matches('#'), 16).unwrap_or(0xffffff);
        return Material { color: hex(color), roughness: if diffusion { 0.9 } else { 0.25 }, opacity: if diffusion { 0.55 + f.scatter as f32 * 0.35 } else { 0.45 }, depth_write: false, ..base };
    }
    let c = id.and_then(|id| lib.cloths.get(id)).or_else(|| lib.cloths.get("silk"));
    let (color, opacity) = c.map_or((0xf2f2ee, 0.55), |c| (c.color, c.opacity as f32));
    Material { color: hex(color), roughness: 0.9, opacity, depth_write: opacity >= 1.0, ..base }
}

/// Maillages d'un élément de grip et la surface qui agit sur la lumière.
pub fn grip_object(o: &SceneObject, t: &GripType) -> (Vec<Item>, Option<Occluder>) {
    let lib = library();
    let body = transform(o.position, o.rotation, [1.0; 3]);
    let family = t.family.as_deref().unwrap_or("");
    let (w, h) = (t.w.unwrap_or(1.0) as f32, t.h.unwrap_or(1.0) as f32);
    let cloth = o.props.get("cloth").and_then(Value::as_str);
    let mut items = Vec::new();
    // pièces du corps (suivent la position et l'orientation de l'objet) ; `plane` : toile ou disque
    let mut part = |mesh: std::sync::Arc<mesh::MeshData>, local: Mat4, material: Material| items.push(Item { mesh, transform: body * local, material });

    // effet sur la lumière : surfaces planes (cadres, drapeaux, réflecteurs)
    let filter = cloth.filter(|c| !lib.cloths.contains_key(*c)).and_then(|c| lib.filter(c));
    let optics = match family {
        "bounce" => Some(Optics { t: 0.0, reflect: if t.extra.get("round") == Some(&Value::Bool(true)) { 0.85 } else { 0.8 }, diffuse: 0.0, solid: true, tint: None }),
        "frame" | "flag" => match filter {
            Some(f) => Some(filter_optics(f)),
            None => cloth.and_then(cloth_optics),
        },
        _ => None,
    };
    let solid = optics.is_some_and(|o| o.solid);
    // ombre nette : toile opaque, réflecteurs, cubes et sacs (pas les tubes du cadre)
    let shadowed = |m: Material, plane: bool| Material { receive_shadow: true, cast_shadow: (solid && (plane || family == "bounce")) || family == "box" || family == "bag", ..m };

    match family {
        "frame" | "flag" => {
            let fm = if family == "frame" { frame_mat() } else { standard(0x2a2c30, 0.6, 0.4) };
            let th = if family == "frame" { 0.03 } else { 0.012 };
            // cadre rectangulaire en tube (plan local XY, face vers +Z)
            for (sx, sy, x, y) in [(w + th, th, 0.0, h / 2.0), (w + th, th, 0.0, -h / 2.0), (th, h, -w / 2.0, 0.0), (th, h, w / 2.0, 0.0)] {
                part(mesh::boxed(sx, sy, th), Mat4::from_translation(Vec3::new(x, y, 0.0)), shadowed(fm, false));
            }
            if cloth != Some("empty") {
                part(mesh::plane(w, h), Mat4::IDENTITY, shadowed(cloth_mat(cloth), true));
            }
        }
        "bounce" => {
            let color = num(t, "color").map_or(0xf4f4f0, |c| c as u32);
            let m = Material { double_sided: true, ..standard(color, 0.85, 0.0) };
            if t.extra.get("round") == Some(&Value::Bool(true)) {
                part(mesh::circle(w / 2.0, 40), Mat4::IDENTITY, shadowed(m, true));
            } else {
                part(mesh::boxed(w, h, 0.05), Mat4::IDENTITY, shadowed(m, false));
            }
        }
        "box" | "bag" => {
            let size: Vec<f32> = t.extra.get("size").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).map(|v| v as f32).collect()).unwrap_or_default();
            let [sw, sh, sd] = [0, 1, 2].map(|i| size.get(i).copied().unwrap_or(0.3));
            if family == "box" {
                part(mesh::boxed(sw, sh, sd), Mat4::IDENTITY, shadowed(standard(0x8a6a45, 0.8, 0.0), false));
            } else {
                // gélule couchée, aplatie
                let local = Mat4::from_scale(Vec3::new(1.0, 1.0, sd / sh)) * Mat4::from_rotation_z(PI / 2.0);
                part(mesh::capsule(sh / 2.0, sw - sh, 4, 10), local, shadowed(standard(0x6b4a2a, 0.95, 0.0), false));
            }
        }
        "stand" => {
            let height = o.props.get("height").and_then(Value::as_f64).filter(|h| *h != 0.0).or(num(t, "height")).unwrap_or(2.0) as f32;
            for p in stand(text(t, "standLook").unwrap_or("baby"), height) {
                part(p.mesh, p.local, Material { receive_shadow: true, ..p.material });
            }
        }
        _ => {}
    }

    // pieds sous les cadres, drapeaux et réflecteurs : ceux choisis, sinon ceux du type ; toujours verticaux
    if family != "stand"
        && let Some(stands) = t.extra.get("stands")
    {
        let choices: Vec<&str> = t.extra.get("standChoices").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let chosen = o.props.get("stand").and_then(Value::as_str).filter(|s| choices.contains(s));
        let id = chosen.or(stands.get("id").and_then(Value::as_str)).unwrap_or("");
        let count = stands.get("count").and_then(Value::as_u64).unwrap_or(1) as usize;
        let look = lib.supports.iter().find(|s| s.get("id").and_then(Value::as_str) == Some(id)).and_then(|s| s.get("look")).and_then(Value::as_str).unwrap_or("combo");
        // oreilles au milieu des côtés (cadres) ; un seul pied à gauche, décalé et relié par le bras (C-stand)
        let pts: Vec<Vec3> = if family == "frame" { vec![Vec3::new(-w / 2.0, 0.0, 0.0), Vec3::new(w / 2.0, 0.0, 0.0)] } else { vec![Vec3::new(-t.w.unwrap_or(1.0) as f32 / 2.0, 0.0, 0.0)] };
        let (_, q, _) = body.to_scale_rotation_translation();
        let away = if family == "frame" { 0.0 } else { 0.55 };
        let dir = (q * Vec3::NEG_X).with_y(0.0).normalize_or_zero();
        for (i, p) in pts.iter().take(count).enumerate() {
            let world = body.transform_point3(*p);
            let at = Vec3::new(world.x + dir.x * away, 0.0, world.z + dir.z * away);
            for sp in stand(look, world.y.max(0.3)) {
                items.push(Item { mesh: sp.mesh, transform: Mat4::from_translation(at) * sp.local, material: sp.material });
            }
            if look == "cstand" && i == 0 {
                // bras du C-stand jusqu'à l'élément
                let a = Vec3::new(at.x, world.y, at.z);
                let d = world - a;
                let local = Mat4::from_scale_rotation_translation(Vec3::new(1.0, d.length().max(0.01), 1.0), Quat::from_rotation_arc(Vec3::Y, d.normalize_or(Vec3::Y)), a + d * 0.5);
                items.push(Item { mesh: mesh::cylinder(0.008, 0.008, 1.0, 8), transform: local, material: frame_mat() });
            }
        }
    }

    let occluder = optics.map(|optics| {
        let center = body.transform_point3(Vec3::ZERO);
        let u = body.x_axis.truncate().normalize_or_zero() * (w / 2.0);
        let v = body.y_axis.truncate().normalize_or_zero() * (h / 2.0);
        Occluder { center, u, v, n: u.cross(v).normalize_or_zero(), radius: w.hypot(h) / 2.0, w, h, optics }
    });
    (items, occluder)
}
