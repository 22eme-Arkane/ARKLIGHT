//! Interaction de la lumière avec le grip, recalculée à chaque image (portage de
//! `web/js/viewport/light-grip.js`) :
//!  - drapeaux et toiles noires coupent le faisceau visible et font une ombre nette (spots) ;
//!  - tulles et soies laissent passer une partie de la lumière (faisceau et éclairement atténués) ;
//!  - réflecteurs (poly, ultrabounce, griffolyn, lamés, réflecteur pliable) renvoient la lumière reçue
//!    en lumière douce ; muslins et magic cloth renvoient et laissent passer à la fois ;
//!  - soies, grids et frosts réémettent de l'autre côté une lumière douce (source large).
//! Les rebonds sont des sources rectangulaires dont la luminance vaut ρ·E/π (E : éclairement reçu).
//! Limites (comme la version three.js) : les tubes et ampoules ne font pas d'ombre.

use crate::grip::Occluder;
use crate::{AreaLight, Beam, BeamOccluder, Light, LightKind, MAX_AREA_LIGHTS, MAX_BEAM_OCCLUDERS, MAX_SHADOWS};
use glam::Vec3;
use std::f32::consts::PI;

/// Luminance en dessous de laquelle un rebond est ignoré.
const AREA_MIN: f32 = 0.002;

/// Intersection du segment [a, b] avec le rectangle d'un élément.
fn segment_hits_rect(a: Vec3, b: Vec3, o: &Occluder) -> bool {
    let d = b - a;
    let denom = o.n.dot(d);
    if denom.abs() < 1e-9 {
        return false;
    }
    let t = o.n.dot(o.center - a) / denom;
    if t <= 0.0 || t >= 1.0 {
        return false;
    }
    let p = a + d * t - o.center;
    (p.dot(o.u) / o.u.length_squared()).abs() <= 1.0 && (p.dot(o.v) / o.v.length_squared()).abs() <= 1.0
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if x <= e0 {
        0.0
    } else if x >= e1 {
        1.0
    } else {
        let t = (x - e0) / (e1 - e0);
        t * t * (3.0 - 2.0 * t)
    }
}

/// Applique le grip aux lumières et aux faisceaux de la scène ; renvoie les rebonds.
pub fn apply(lights: &mut [Light], beams: &mut [Beam], occ: &[Occluder]) -> Vec<AreaLight> {
    // ---- 1. ombres nettes : les spots qui éclairent une surface opaque (les plus utiles d'abord)
    let solids: Vec<&Occluder> = occ.iter().filter(|o| o.optics.solid).collect();
    let mut candidates: Vec<(usize, f32)> = Vec::new();
    for (i, l) in lights.iter_mut().enumerate() {
        l.shadow = false;
        let LightKind::Spot { angle, .. } = l.kind else { continue };
        let intensity = l.candela;
        let mut score = 0.0f32;
        if intensity > 0.0 {
            for o in &solids {
                let d = o.center - l.position;
                let dist = d.length();
                if dist > 20.0 || dist <= 0.0 {
                    continue;
                }
                let ang = (l.direction.dot(d) / dist).min(1.0).acos();
                if ang <= angle + (o.radius / dist).atan() {
                    score = score.max(intensity / (dist * dist));
                }
            }
        }
        if score > 0.0 {
            candidates.push((i, score));
        }
    }
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (i, _) in candidates.iter().take(MAX_SHADOWS) {
        lights[*i].shadow = true;
    }

    // ---- 2. tulles, soies (et surfaces opaques sans ombre calculée) : éclairement atténué
    let i0: Vec<f32> = lights.iter().map(|l| l.candela).collect(); // avant le grip (pour les rebonds)
    if !occ.is_empty() {
        for l in lights.iter_mut() {
            if !matches!(l.kind, LightKind::Spot { .. }) || l.candela <= 0.0 {
                continue;
            }
            let end = l.position + l.direction * 30.0;
            let mut k = 1.0;
            let mut tint = [1.0f32; 3];
            for o in occ {
                if (o.optics.solid && l.shadow) || !segment_hits_rect(l.position, end, o) {
                    continue;
                }
                k *= o.optics.t as f32;
                if let Some(t) = o.optics.tint {
                    // gélatine : la lumière prend sa couleur
                    for c in 0..3 {
                        tint[c] *= t[c] as f32;
                    }
                }
            }
            l.candela *= k;
            for c in 0..3 {
                l.color[c] *= tint[c];
            }
        }
    }

    // ---- 3. faisceaux visibles coupés derrière le grip
    for b in beams.iter_mut() {
        b.occluders.clear();
        if occ.is_empty() || b.intensity <= 0.0 {
            continue;
        }
        let apex = b.transform.transform_point3(Vec3::ZERO);
        let axis = b.transform.transform_vector3(Vec3::Z).normalize_or_zero();
        for o in occ {
            if b.occluders.len() >= MAX_BEAM_OCCLUDERS {
                break;
            }
            let d = o.center - apex;
            let along = d.dot(axis);
            if along <= 0.0 || along > 8.0 {
                continue;
            }
            let dist = d.length();
            if (along / dist).min(1.0).acos() > b.half_angle + (o.radius / dist).atan() + 0.05 {
                continue;
            }
            b.occluders.push(BeamOccluder { center: o.center, u: o.u, v: o.v, t: o.optics.t as f32 });
        }
    }

    // ---- 4. rebonds (réflecteurs) et diffusion (soies) : sources douces rectangulaires
    struct Emitter<'a> {
        o: &'a Occluder,
        dir: f32,
        lum: f32,
        color: [f32; 3],
    }
    let mut emitters: Vec<Emitter> = Vec::new();
    for o in occ {
        if o.optics.reflect <= 0.0 && o.optics.diffuse <= 0.0 {
            continue;
        }
        // éclairement reçu sur chaque face, avec sa couleur
        let mut faces = [(0.0f32, [0.0f32; 3]); 2];
        for (l, &intensity) in lights.iter().zip(&i0) {
            if intensity <= 0.0 {
                continue;
            }
            let d = o.center - l.position;
            let dist = d.length().max(0.3);
            let d = d / dist;
            let f = match l.kind {
                LightKind::Spot { angle, penumbra } => smoothstep(angle.cos(), (angle * (1.0 - penumbra)).cos(), l.direction.dot(d)),
                LightKind::Point => 1.0,
            };
            let e = intensity * f * o.n.dot(d).abs() / (dist * dist);
            if e <= 0.0 {
                continue;
            }
            let face = if o.n.dot(d) < 0.0 { 0 } else { 1 }; // 0 : la source est du côté de la normale
            faces[face].0 += e;
            for c in 0..3 {
                faces[face].1[c] += l.color[c] * e;
            }
        }
        for (i, (e, c)) in faces.iter().enumerate() {
            if *e <= 0.0 {
                continue;
            }
            let side = if i == 0 { 1.0 } else { -1.0 }; // côté de la source
            let color = c.map(|v| v / e);
            // gélatine, lamé or, muslin écru : la lumière renvoyée ou réémise prend leur couleur
            let tinted = match o.optics.tint {
                Some(t) => [color[0] * t[0] as f32, color[1] * t[1] as f32, color[2] * t[2] as f32],
                None => color,
            };
            if o.optics.reflect > 0.0 {
                emitters.push(Emitter { o, dir: side, lum: o.optics.reflect as f32 * e / PI, color: tinted });
            }
            if o.optics.diffuse > 0.0 {
                emitters.push(Emitter { o, dir: -side, lum: o.optics.diffuse as f32 * e / PI, color: tinted });
            }
        }
    }
    emitters.sort_by(|a, b| b.lum.total_cmp(&a.lum));
    emitters
        .iter()
        .take(MAX_AREA_LIGHTS)
        .filter(|em| em.lum >= AREA_MIN)
        .map(|em| {
            let o = em.o;
            let out = o.n * em.dir;
            let position = o.center + out * 0.03;
            // la source rectangulaire éclaire vers son −Z, tournée vers l'extérieur de la surface (lookAt)
            let z = -out;
            let up = o.v.normalize_or(Vec3::Y);
            let x = up.cross(z).normalize_or(Vec3::X);
            let y = z.cross(x);
            AreaLight { position, half_width: x * (o.w / 2.0), half_height: y * (o.h / 2.0), color: em.color.map(|c| c * em.lum) }
        })
        .collect()
}
