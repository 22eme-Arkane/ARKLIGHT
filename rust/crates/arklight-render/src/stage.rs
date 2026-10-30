//! Du projet ARKLIGHT à la scène du moteur : plateau, projecteurs (modèles de `models.rs` avec leur
//! zoom et leurs accessoires, éclairés par la sortie du moteur lumière pixel par pixel), pieds,
//! grip (et son effet sur la lumière : `grip_light.rs`), décor.

use crate::grip::grip_object;
use crate::mesh;
use crate::models::{FixtureModel, Role, standard, stand};
use crate::props::{build_prop, kit::Built};
use crate::{Aim, Beam, Item, Light, Material, Pick, Scene, Surface, grip_light, transform, Lens};
use arklight_core::accessories::{Effective, Front, effective};
use arklight_core::engine::Lights;
use arklight_core::{FixtureType, Project, library};
use glam::{Mat4, Vec2, Vec3};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Clé d'un modèle : type, zoom et accessoires (un modèle par combinaison, construit une fois).
fn model_key(ty: &FixtureType, eff: &Effective) -> String {
    let acc = eff.accessories.map(|a| {
        let front = match a.front {
            Some(Front::None) => "sans".to_owned(),
            Some(Front::Filter(f)) => f.id.clone(),
            None => String::new(),
        };
        format!("{}|{}|{}|{}", a.modifier.map_or("", |m| m.id.as_str()), a.grid.map_or("", |g| g.id.as_str()), a.gel.map_or("", |g| g.id.as_str()), front)
    });
    format!("{}|{:?}|{:?}|{:?}|{}", ty.id, eff.beam_angle, eff.lux_at1m, eff.lumens, acc.unwrap_or_default())
}

/// Modèle d'un objet de décor pour ses réglages (construit une fois par combinaison).
fn prop_for(t: &arklight_core::library::PropType, props: &Value) -> Arc<Built> {
    static PROPS: OnceLock<Mutex<HashMap<String, Arc<Built>>>> = OnceLock::new();
    let cache = PROPS.get_or_init(Default::default);
    let key = format!("{}|{}", t.id, props);
    if let Some(b) = cache.lock().expect("décor").get(&key) {
        return b.clone();
    }
    let b = Arc::new(build_prop(t, props));
    cache.lock().expect("décor").insert(key, b.clone());
    b
}

fn model_for(ty: &FixtureType, eff: &Effective) -> Arc<FixtureModel> {
    static MODELS: OnceLock<Mutex<HashMap<String, Arc<FixtureModel>>>> = OnceLock::new();
    let models = MODELS.get_or_init(Default::default);
    let key = model_key(ty, eff);
    if let Some(m) = models.lock().expect("modèles").get(&key) {
        return m.clone();
    }
    let m = Arc::new(FixtureModel::build(ty, eff));
    models.lock().expect("modèles").insert(key, m.clone());
    m
}

/// Couleur moyenne des pixels `from..to` (sortie du moteur, pixels × 3).
fn average(px: &[f32], from: usize, to: usize) -> [f32; 3] {
    let n = to.saturating_sub(from).max(1) as f32;
    let mut c = [0.0f32; 3];
    for p in from..to {
        for k in 0..3 {
            c[k] += px.get(p * 3 + k).copied().unwrap_or(0.0);
        }
    }
    c.map(|v| v / n)
}

fn max3(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2])
}

/// Boîte englobante (monde) de maillages placés.
fn bounds(items: &[Item]) -> Option<(Vec3, Vec3)> {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for it in items {
        let (a, b) = it.mesh.bounds;
        for i in 0..8 {
            let c = Vec3::new(if i & 1 == 0 { a.x } else { b.x }, if i & 2 == 0 { a.y } else { b.y }, if i & 4 == 0 { a.z } else { b.z });
            let w = it.transform.transform_point3(c);
            lo = lo.min(w);
            hi = hi.max(w);
        }
    }
    lo.x.is_finite().then_some((lo, hi))
}

/// Scène du moteur pour le plan ouvert du projet ; `lights` : sortie de chaque projecteur (moteur
/// lumière d'`arklight-core`).
pub fn scene_from_project(p: &Project, lights: &Lights) -> Scene {
    let lib = library();
    let env = &p.environment;
    let mut s = Scene { ambient: env.ambient as f32, exposure: env.exposure.max(0.0) as f32, bloom: true, ..Default::default() };
    if env.floor {
        let floor = Material { surface: Surface::Grid, receive_shadow: true, ..standard(0x45484e, 0.88, 0.0) };
        s.items.push(Item { mesh: mesh::floor(), transform: Mat4::IDENTITY, material: floor });
    }
    if env.cyc {
        let cyc = Material { double_sided: true, receive_shadow: true, ..standard(0x9a9da3, 0.95, 0.0) };
        s.items.push(Item { mesh: mesh::cyc(), transform: Mat4::IDENTITY, material: cyc });
    }
    let haze = env.haze as f32;

    for f in &p.fixtures {
        let Some(ty) = lib.fixture(&f.type_id) else { continue };
        let eff = effective(ty, f);
        let model = model_for(ty, &eff);
        let root = transform(f.position, f.rotation, [1.0; 3]);
        let (_, rot, _) = root.to_scale_rotation_translation();
        // sortie du moteur ; gélatine montée : toute la lumière émise prend sa couleur
        let mut px: Vec<f32> = lights.get(&f.id).map_or_else(Vec::new, |l| l.pixels.iter().map(|v| *v as f32).collect());
        if let Some(t) = model.tint {
            for (i, v) in px.iter_mut().enumerate() {
                *v *= t[i % 3];
            }
        }
        let all = average(&px, 0, ty.pixels);
        let start = s.items.len();
        for part in &model.parts {
            let mut material = part.material;
            let glow = match part.role {
                Role::Body => [0.0; 3],
                Role::Pixel(i) => average(&px, i, i + 1),
                Role::Average => all,
            };
            material.emissive = glow.map(|c| c * material.emissive_gain);
            s.items.push(Item { mesh: part.mesh.clone(), transform: root * part.local, material });
        }
        let pick = model.bounds(root).map(|(lo, hi)| Pick { id: f.id.clone(), object: false, lo, hi, items: 0..0 });
        // lumières : couleur normalisée, intensité selon le plus fort des trois canaux
        for l in &model.lights {
            let rgb = average(&px, l.from, l.to);
            let m = max3(rgb);
            if m <= 1e-4 {
                continue;
            }
            s.lights.push(Light { position: root.transform_point3(l.position), direction: rot * l.direction, color: rgb.map(|c| c / m), candela: l.candela * m, kind: l.kind, shadow: false });
        }
        for b in &model.beams {
            let rgb = average(&px, b.from, b.to);
            let m = max3(rgb);
            if m <= 1e-4 || haze <= 0.0 {
                continue;
            }
            let half_angle = eff.beam_angle.filter(|a| *a != 0.0).map_or(0.35, |a| (a.min(170.0) as f32 / 2.0).to_radians());
            s.beams.push(Beam { mesh: b.mesh.clone(), transform: root * Mat4::from_translation(b.position), color: rgb.map(|c| c / m), intensity: m.min(1.5) * haze * b.gain, half_angle, occluders: Vec::new() });
        }
        // pied : sous le centre du projecteur, jusqu'à son bas
        if env.stands()
            && let Some((lo, hi)) = model.bounds(root)
        {
            let choice = f.settings.extra.get("support").and_then(Value::as_str);
            let look = lib.resolve_support(ty, choice, lo.y as f64).and_then(|sup| sup.get("look")).and_then(Value::as_str);
            if let Some(look) = look {
                let base = Mat4::from_translation(Vec3::new((lo.x + hi.x) / 2.0, 0.0, (lo.z + hi.z) / 2.0));
                for part in stand(look, lo.y) {
                    s.items.push(Item { mesh: part.mesh, transform: base * part.local, material: part.material });
                }
            }
        }
        // sélection : le projecteur et son pied
        if let Some(mut pick) = pick {
            pick.items = start..s.items.len();
            s.picks.push(pick);
        }
    }

    // grip (et ses surfaces qui agissent sur la lumière), décor
    let mut occluders = Vec::new();
    for o in &p.objects {
        if let Some(g) = lib.grip(&o.type_id) {
            let (items, occ) = grip_object(o, g);
            if let Some((lo, hi)) = bounds(&items) {
                s.picks.push(Pick { id: o.id.clone(), object: true, lo, hi, items: s.items.len()..s.items.len() + items.len() });
            }
            s.items.extend(items);
            occluders.extend(occ);
        } else if let Some(t) = lib.prop(&o.type_id) {
            // décor, personnages, véhicules : modèle selon les réglages, posé comme l'objet
            let built = prop_for(t, &o.props);
            let body = transform(o.position, o.rotation, [1.0; 3]) * built.root;
            let start = s.items.len();
            for (mesh, material) in &built.parts {
                s.items.push(Item { mesh: mesh.clone(), transform: body, material: *material });
            }
            if let Some((lo, hi)) = bounds(&s.items[start..]) {
                s.picks.push(Pick { id: o.id.clone(), object: true, lo, hi, items: start..s.items.len() });
            }
            if let Some(lens) = built.lens {
                s.lenses.push(Lens { id: o.id.clone(), position: body.transform_point3(lens), forward: body.transform_vector3(Vec3::Z).normalize_or(Vec3::Z) });
            }
            if t.extra.get("aimable").and_then(Value::as_bool) == Some(true)
                && let Some(aim) = built.aim
            {
                s.aims.push(Aim { id: o.id.clone(), point: body.transform_point3(aim), person: t.model.as_deref() == Some("person") });
            }
        }
    }
    s.areas = grip_light::apply(&mut s.lights, &mut s.beams, &occluders);
    plot_volumes(p, &mut s);
    s
}

/// Volumes des formes du plan qui ont une hauteur (maison, kiosque, mur…) : `buildPlotVolumes`.
fn plot_volumes(p: &Project, s: &mut Scene) {
    static VOLUMES: OnceLock<Mutex<HashMap<String, Arc<mesh::MeshData>>>> = OnceLock::new();
    let Some(items) = p.extra.get("plot").and_then(|pl| pl.get("items")).and_then(Value::as_array) else { return };
    let material = Material { opacity: 0.88, ..standard(0x8d9299, 0.9, 0.0) };
    for it in items {
        let h = it.get("height").and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let kind = it.get("kind").and_then(Value::as_str).unwrap_or("");
        if !(h > 0.0) || !matches!(kind, "rect" | "ellipse" | "polygon" | "freehand") {
            continue;
        }
        let pts: Vec<[f32; 2]> = it.get("points").and_then(Value::as_array).into_iter().flatten().filter_map(|q| Some([q.get(0)?.as_f64()? as f32, q.get(1)?.as_f64()? as f32])).collect();
        let pts: Vec<[f32; 2]> = match (kind, pts.as_slice()) {
            ("rect", [a, b, ..]) => vec![[a[0], a[1]], [b[0], a[1]], [b[0], b[1]], [a[0], b[1]]],
            ("ellipse", [a, b, ..]) => {
                let (cx, cz, rx, rz) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (b[0] - a[0]).abs() / 2.0, (b[1] - a[1]).abs() / 2.0);
                (0..40).map(|i| i as f32 / 40.0 * std::f32::consts::TAU).map(|t| [cx + rx * t.cos(), cz + rz * t.sin()]).collect()
            }
            _ => pts,
        };
        if pts.len() < 3 {
            continue;
        }
        let key = format!("{pts:?}|{h}");
        let built = {
            let mut cache = VOLUMES.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
            cache
                .entry(key)
                .or_insert_with(|| {
                    // forme dans le plan (x, −z), extrudée sur sa hauteur, puis couchée : elle monte selon +Y
                    let contour: Vec<Vec2> = pts.iter().map(|q| Vec2::new(q[0], -q[1])).collect();
                    let geo = crate::props::kit::Geo::extrude(&contour, h, 0.0);
                    let mut b = mesh::Builder::default();
                    for (pos, nrm) in geo.pos.iter().zip(&geo.nrm) {
                        b.vertex(*pos, *nrm);
                    }
                    b.indices_offset(0, &geo.idx);
                    Arc::new(b.finish())
                })
                .clone()
        };
        let transform = Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2) * Mat4::from_translation(Vec3::new(0.0, 0.0, h / 2.0));
        s.items.push(Item { mesh: built, transform, material });
    }
}
