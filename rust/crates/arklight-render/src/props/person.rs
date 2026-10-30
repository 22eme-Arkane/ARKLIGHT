//! Personnage articulé (mannequin) : squelette simple calculé selon la pose, habillé de gélules
//! (portage de `web/js/viewport/props/person.js`). Face vers +Z, pieds au sol. Proportions
//! d'après les canons anatomiques courants (en fraction de la taille H). La position de la tête
//! sert de cible pour « Viser ».

use super::kit::*;
use glam::{Mat4, Vec3};
use std::f32::consts::PI;

// ------------------------------------------------------------------ silhouettes
/// Silhouette : longueurs et rayons en fraction de la taille H.
#[derive(Clone, Copy)]
struct Build {
    /// (inutilisée par le modèle, gardée comme dans la version JavaScript)
    #[allow(dead_code)]
    ankle: f32,
    shin: f32,
    thigh: f32,
    hip: f32,
    torso: f32,
    neck: f32,
    head: f32,
    shoulder: f32,
    upper: f32,
    fore: f32,
    hand: f32,
    waist: f32,
    chest: f32,
    hips: f32,
    limb: f32,
}

// Silhouettes : longueurs et rayons en fraction de la taille H
#[rustfmt::skip]
const BUILD: [(&str, Build); 3] = [
    ("man", Build { ankle: 0.045, shin: 0.243, thigh: 0.245, hip: 0.058, torso: 0.3, neck: 0.04, head: 0.062, shoulder: 0.118, upper: 0.172, fore: 0.148, hand: 0.075, waist: 0.083, chest: 0.1, hips: 0.093, limb: 1.0 }),
    ("woman", Build { ankle: 0.045, shin: 0.245, thigh: 0.248, hip: 0.064, torso: 0.295, neck: 0.042, head: 0.063, shoulder: 0.106, upper: 0.17, fore: 0.145, hand: 0.072, waist: 0.074, chest: 0.093, hips: 0.1, limb: 0.9 }),
    ("child", Build { ankle: 0.04, shin: 0.225, thigh: 0.225, hip: 0.06, torso: 0.3, neck: 0.04, head: 0.078, shoulder: 0.11, upper: 0.165, fore: 0.14, hand: 0.075, waist: 0.09, chest: 0.098, hips: 0.095, limb: 1.05 }),
];

/// Silhouette `name` (`BUILD[name] || BUILD.man`).
fn build_of(name: Option<&str>) -> Build {
    BUILD.iter().find(|(n, _)| Some(*n) == name).map_or(BUILD[0].1, |(_, b)| *b)
}

// ------------------------------------------------------------------ poses
/// Pose : flexion de hanche, de genou, d'épaule (vers l'avant), abduction (bras écartés), coude,
/// croisement des avant-bras (vers l'intérieur), inclinaison du buste. Indices [gauche, droite].
#[derive(Clone, Copy)]
struct Pose {
    hip: [f32; 2],
    knee: [f32; 2],
    sh: [f32; 2],
    abd: [f32; 2],
    elbow: [f32; 2],
    cross: [f32; 2],
    lean: f32,
    /// écart des jambes ; 0 : valeur par défaut (0,03)
    spread: f32,
    lying: bool,
    phone: bool,
}

/// Pose sans croisement, inclinaison ni écart particulier (champs absents de la version JavaScript).
const fn pose(hip: [f32; 2], knee: [f32; 2], sh: [f32; 2], abd: [f32; 2], elbow: [f32; 2]) -> Pose {
    Pose { hip, knee, sh, abd, elbow, cross: [0.0, 0.0], lean: 0.0, spread: 0.0, lying: false, phone: false }
}

// Poses : flexion de hanche, de genou, d'épaule (vers l'avant), abduction (bras écartés), coude,
// croisement des avant-bras (vers l'intérieur), inclinaison du buste. Indices [gauche, droite].
#[rustfmt::skip]
const POSES: [(&str, Pose); 8] = [
    ("standing", pose([0.0, 0.0], [0.03, 0.03], [0.06, 0.06], [0.1, 0.1], [0.18, 0.18])),
    ("walking", Pose { lean: 0.05, ..pose([0.36, -0.3], [0.08, 0.5], [-0.38, 0.38], [0.08, 0.08], [0.25, 0.5]) }),
    ("sitting", Pose { lean: 0.05, spread: 0.07, ..pose([1.5, 1.5], [1.5, 1.5], [0.5, 0.5], [0.05, 0.05], [0.85, 0.85]) }),
    ("lying", Pose { lying: true, ..pose([0.0, 0.0], [0.04, 0.04], [0.04, 0.04], [0.2, 0.2], [0.1, 0.1]) }),
    ("crossed", Pose { cross: [1.6, 1.6], ..pose([0.0, 0.0], [0.03, 0.03], [0.3, 0.3], [0.02, 0.02], [1.25, 1.25]) }),
    ("phone", Pose { cross: [0.0, 0.55], phone: true, ..pose([0.0, 0.0], [0.03, 0.03], [0.06, 0.95], [0.1, 0.35], [0.18, 2.35]) }),
    ("pointing", pose([0.0, 0.0], [0.03, 0.03], [0.06, 1.45], [0.1, 0.1], [0.18, 0.05])),
    ("handsup", pose([0.0, 0.0], [0.03, 0.03], [-0.1, -0.1], [2.75, 2.75], [0.12, 0.12])),
];

/// Pose `name` (`POSES[name] || POSES.standing`).
fn pose_of(name: Option<&str>) -> Pose {
    POSES.iter().find(|(n, _)| Some(*n) == name).map_or(POSES[0].1, |(_, p)| *p)
}

// ------------------------------------------------------------------ squelette
/// Une jambe : côté (1 : gauche, -1 : droite), directions des segments, articulations.
struct Leg {
    s: f32,
    thigh_dir: Vec3,
    shin_dir: Vec3,
    foot_dir: Vec3,
    hip: Vec3,
    knee: Vec3,
    ankle: Vec3,
    toe: Vec3,
}

/// Un bras : articulations et direction de l'avant-bras.
struct Arm {
    shoulder: Vec3,
    elbow: Vec3,
    wrist: Vec3,
    hand: Vec3,
    fore: Vec3,
}

struct Skeleton {
    pelvis: Vec3,
    up: Vec3,
    neck_base: Vec3,
    head_c: Vec3,
    legs: [Leg; 2],
    arms: [Arm; 2],
    lying: bool,
    phone: bool,
}

/// Squelette (positions des articulations) dans le repère du personnage debout, face +Z.
fn skeleton(h: f32, b: &Build, pose: Option<&str>) -> Skeleton {
    let p = pose_of(pose);
    let l = |x: f32| x * h;
    let spread = if p.spread != 0.0 { p.spread } else { 0.03 };
    let mut legs = [(1.0f32, 0usize), (-1.0, 1)].map(|(s, i)| {
        let a = p.hip[i];
        let th = a - p.knee[i];
        Leg {
            s,
            thigh_dir: v(s * spread.sin(), -a.cos() * spread.cos(), a.sin() * spread.cos()),
            shin_dir: v(s * spread.sin() * 0.5, -th.cos(), th.sin()),
            foot_dir: v(0.0, th.sin(), th.cos()),
            hip: Vec3::ZERO,
            knee: Vec3::ZERO,
            ankle: Vec3::ZERO,
            toe: Vec3::ZERO,
        }
    });
    let place_legs = |legs: &mut [Leg; 2], pelvis_y: f32| {
        for lg in legs.iter_mut() {
            lg.hip = v(lg.s * l(b.hip), pelvis_y, 0.0);
            lg.knee = lg.hip + lg.thigh_dir * l(b.thigh);
            lg.ankle = lg.knee + lg.shin_dir * l(b.shin);
            lg.toe = lg.ankle + lg.foot_dir * (0.13 * h);
        }
    };
    // le point le plus bas des pieds (talon ou pointe) touche le sol
    place_legs(&mut legs, 0.0);
    let sole = legs.iter().map(|lg| lg.ankle.y.min(lg.toe.y)).fold(f32::INFINITY, f32::min) - 0.03 * h;
    let pelvis = v(0.0, -sole, 0.0);
    place_legs(&mut legs, pelvis.y);
    let lean = p.lean;
    let up = v(0.0, lean.cos(), lean.sin());
    let neck_base = pelvis + up * l(b.torso);
    let shoulder_y = neck_base + up * (-0.035 * h);
    let head_c = neck_base + up * (l(b.neck) + l(b.head) * 1.05);
    let arms = [(1.0f32, 0usize), (-1.0, 1)].map(|(s, i)| {
        let al = p.abd[i];
        let f = p.sh[i];
        let e = p.elbow[i];
        // (normalize de three.js : un vecteur nul reste nul)
        let upper = v(s * al.sin(), -al.cos() * f.cos(), al.cos() * f.sin()).normalize_or_zero();
        let fore = v(s * al.sin() - s * p.cross[i], -al.cos() * (f + e).cos(), al.cos() * (f + e).sin()).normalize_or_zero();
        let shoulder = shoulder_y + v(s * l(b.shoulder), 0.0, 0.0);
        let elbow = shoulder + upper * l(b.upper);
        let wrist = elbow + fore * l(b.fore);
        let hand = wrist + fore * l(b.hand);
        Arm { shoulder, elbow, wrist, hand, fore }
    });
    Skeleton { pelvis, up, neck_base, head_c, legs, arms, lying: p.lying, phone: p.phone }
}

// ------------------------------------------------------------------ modèle
/// Modèle 3D d'un personnage. props : pose, height (cm), build, skin, hair, hairColor, top, pants.
pub fn build_person(p: Props) -> Built {
    let h = (p.num("height").filter(|x| *x != 0.0).unwrap_or(175.0) / 100.0).max(0.6);
    let b = build_of(p.str("build"));
    let sk = skeleton(h, &b, p.str("pose"));
    let mut k = Kit::new();
    let skin = k.mat("skin", p.col("skin", "#e0aa83"), mo().rough(0.55));
    let top = k.mat("top", p.col("top", "#3d4f6b"), mo().rough(0.85));
    let pants = k.mat("pants", p.col("pants", "#2a2e35"), mo().rough(0.85));
    let shoes = k.mat("shoes", "#1c1c1f", mo().rough(0.5));
    let hair = k.mat("hair", p.col("hairColor", "#2b1d14"), mo().rough(0.95));
    let l = |x: f32| x * h;
    let lr = b.limb * h;

    // buste : bassin (bas) et torse (haut), profils de révolution aplatis
    let q = from_to(v(0.0, 1.0, 0.0), sk.up);
    let lower = [
        [0.001, -0.03 * h],
        [l(b.hips) * 0.75, -0.025 * h],
        [l(b.hips), 0.02 * h],
        [l(b.hips) * 0.98, 0.07 * h],
        [l(b.waist) * 1.02, 0.12 * h],
    ];
    let upper = [
        [l(b.waist) * 1.02, 0.11 * h],
        [l(b.waist), 0.15 * h],
        [l(b.chest) * 0.97, 0.21 * h],
        [l(b.chest), 0.245 * h],
        [l(b.chest) * 0.95, l(b.torso) - 0.02 * h],
        [l(b.chest) * 0.55, l(b.torso) - 0.004 * h],
        [0.028 * h, l(b.torso)],
    ];
    k.lathe(&lower, pants, sk.pelvis, o().q(q).s([1.0, 1.0, 0.66]).seg(18));
    k.lathe(&upper, top, sk.pelvis, o().q(q).s([1.0, 1.0, 0.62]).seg(18));

    // cou, tête, nez (sens du regard), cheveux
    let head_r = l(b.head);
    k.rod(sk.neck_base + sk.up * (-0.01 * h), sk.head_c + sk.up * (-head_r * 0.6), 0.026 * h, skin, o());
    k.ball(head_r, skin, sk.head_c, o().q(q).s([0.9, 1.12, 1.0]).seg(20));
    k.ball(0.011 * h, skin, sk.head_c + q * v(0.0, -0.012 * h, head_r * 0.96), o());
    if p.str("hair") != Some("bald") {
        let cap = q * euler(-0.45, 0.0, 0.0);
        k.ball(head_r * 1.06, hair, sk.head_c + q * v(0.0, head_r * 0.06, -head_r * 0.05), o().q(cap).s([0.92, 1.1, 1.02]).seg(20).theta(PI * 0.55));
        if p.str("hair") == Some("long") {
            k.rbox(head_r * 1.75, 0.2 * h, head_r * 0.7, head_r * 0.3, hair, sk.head_c + q * v(0.0, -0.075 * h, -head_r * 0.62), o().q(q));
        }
    }

    // bras : épaules, bras (manches), mains
    for a in &sk.arms {
        k.ball(0.034 * lr, top, a.shoulder, o());
        k.capsule(a.shoulder, a.elbow, 0.027 * lr, top, o());
        k.capsule(a.elbow, a.wrist, 0.022 * lr, top, o());
        k.capsule(a.wrist, a.hand, 0.018 * lr, skin, o().s([1.15, 1.0, 0.65]));
    }
    if sk.phone {
        let a = &sk.arms[1];
        k.boxed(0.035 * h, 0.08 * h, 0.008 * h, shoes, a.hand + a.fore * (-0.01 * h), o().q(from_to(v(0.0, 1.0, 0.0), a.fore)));
    }

    // jambes et chaussures
    for lg in &sk.legs {
        k.capsule(lg.hip, lg.knee, 0.037 * lr, pants, o());
        k.capsule(lg.knee, lg.ankle, 0.028 * lr, pants, o());
        let heel = lg.ankle + lg.foot_dir * (-0.025 * h) + v(0.0, -0.012 * h, 0.0);
        k.capsule(heel, lg.toe + v(0.0, -0.012 * h, 0.0), 0.024 * h, shoes, o().s([1.05, 1.0, 0.75]));
    }

    let mut built = k.build();
    let mut aim = sk.head_c;
    if sk.lying {
        // allongé sur le dos, la tête vers -Z, centré sur l'objet
        built.root = Mat4::from_translation(v(0.0, 0.072 * h, h * 0.47)) * Mat4::from_rotation_x(-PI / 2.0);
        aim = built.root.transform_point3(sk.head_c);
    }
    built.aim(aim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const ALL_POSES: [&str; 8] = ["standing", "walking", "sitting", "lying", "crossed", "phone", "pointing", "handsup"];

    /// Chaque personnage du catalogue, avec ses réglages puis dans chaque pose : des maillages
    /// non vides aux sommets finis, une cible pour « Viser ».
    #[test]
    fn every_person_in_every_pose() {
        let types: Vec<_> = arklight_core::library().props.iter().filter(|t| t.category.as_deref() == Some("people")).collect();
        assert!(!types.is_empty(), "aucun personnage dans le catalogue");
        for t in types {
            let defaults = t.extra.get("defaults").cloned().unwrap_or_else(|| json!({}));
            let mut variants = vec![(String::from("(défaut)"), defaults.clone())];
            for pose in ALL_POSES {
                let mut props = defaults.clone();
                match &mut props {
                    Value::Object(m) => {
                        m.insert("pose".into(), json!(pose));
                    }
                    _ => props = json!({ "pose": pose }),
                }
                variants.push((pose.to_owned(), props));
            }
            for (name, props) in &variants {
                let built = build_person(Props(props));
                assert!(!built.parts.is_empty(), "{} / {name} : aucun maillage", t.id);
                for (mesh, _) in &built.parts {
                    assert!(!mesh.vertices.is_empty(), "{} / {name} : maillage vide", t.id);
                    assert!(mesh.vertices.iter().all(|vx| vx.pos.iter().all(|c| c.is_finite())), "{} / {name} : sommet non fini", t.id);
                }
                assert!(built.root.is_finite(), "{} / {name} : transformation non finie", t.id);
                let aim = built.aim.expect("cible de la tête");
                assert!(aim.is_finite(), "{} / {name} : cible non finie", t.id);
                if props.get("pose").and_then(Value::as_str) == Some("lying") {
                    // allongé : la tête près du sol, vers -Z
                    assert!(aim.y < 0.5 && aim.z < 0.0, "{} / {name} : tête mal placée {aim:?}", t.id);
                } else {
                    assert!(aim.y > 0.5, "{} / {name} : tête trop basse {aim:?}", t.id);
                }
            }
        }
    }
}
