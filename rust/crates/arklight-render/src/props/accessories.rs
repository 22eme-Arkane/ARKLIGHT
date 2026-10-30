//! Accessoires (valise, carton, bouteille, verre, livres, ordinateur, guitare, micro) et matériel
//! de plateau (caméra sur trépied, travelling, chaise de réalisateur, moniteur, perche son),
//! portage de `web/js/viewport/props/accessories.js`.
//! Face avant vers +Z, posés au sol.

use super::kit::*;
use arklight_core::library::PropType;
use std::f32::consts::{FRAC_PI_2, PI};

fn metal(k: &mut Kit) -> M {
    k.mat("metal", "#a9adb2", mo().rough(0.35).metal(0.8))
}

fn black(k: &mut Kit) -> M {
    k.mat("black", "#18191c", mo().rough(0.55))
}

fn suitcase(c: Col) -> Built {
    let mut k = Kit::new();
    let shell = k.mat("shell", c, mo().rough(0.35).metal(0.1));
    k.rbox(0.45, 0.62, 0.25, 0.04, shell, [0.0, 0.36, 0.0], o());
    for x in [-0.12, 0.0, 0.12] {
        let rib = k.mat("rib", shade(c, 0.8), mo().rough(0.4));
        k.boxed(0.01, 0.6, 0.255, rib, [x, 0.36, 0.0], o());
    }
    let m = black(&mut k);
    let alu = metal(&mut k);
    k.rod([-0.08, 0.67, 0.0], [-0.08, 0.95, 0.0], 0.008, alu, o());
    k.rod([0.08, 0.67, 0.0], [0.08, 0.95, 0.0], 0.008, alu, o());
    k.boxed(0.2, 0.025, 0.03, m, [0.0, 0.96, 0.0], o());
    for sx in [-1.0_f32, 1.0] {
        for sz in [-1.0_f32, 1.0] {
            k.cyl(0.025, 0.025, 0.02, m, [sx * 0.18, 0.025, sz * 0.09], o().r([0.0, 0.0, FRAC_PI_2]).seg(10));
        }
    }
    k.build()
}

fn cardboard(c: Col) -> Built {
    let mut k = Kit::new();
    let card = k.mat("card", c, mo().rough(0.95));
    k.boxed(0.5, 0.4, 0.4, card, [0.0, 0.2, 0.0], o());
    let tape = k.mat("tape", "#d9c08a", mo().rough(0.5));
    k.boxed(0.06, 0.405, 0.405, tape, [0.0, 0.2, 0.0], o());
    k.build()
}

fn crate_(c: Col) -> Built {
    let mut k = Kit::new();
    let w = k.mat("wood", c, mo().rough(0.85));
    let w2 = k.mat("wood2", shade(c, 0.78), mo().rough(0.85));
    k.boxed(0.78, 0.58, 0.58, w2, [0.0, 0.3, 0.0], o());
    for i in 0..4 {
        for s in [-1.0_f32, 1.0] {
            k.boxed(0.8, 0.12, 0.02, w, [0.0, 0.08 + i as f32 * 0.145, s * 0.3], o());
        }
    }
    for s in [-1.0_f32, 1.0] {
        for sz in [-1.0_f32, 1.0] {
            k.boxed(0.05, 0.6, 0.05, w, [s * 0.375, 0.3, sz * 0.28], o());
        }
    }
    k.build()
}

fn flightcase(c: Col) -> Built {
    let mut k = Kit::new();
    let panel = k.mat("panel", c, mo().rough(0.6));
    let alu = metal(&mut k);
    let y0: f32 = 0.08; // roulettes
    k.boxed(0.78, 0.56, 0.58, panel, [0.0, y0 + 0.29, 0.0], o());
    for (w, h, d, p) in [
        (0.8, 0.03, 0.03, [0.0, y0 + 0.575, 0.29]),
        (0.8, 0.03, 0.03, [0.0, y0 + 0.575, -0.29]),
        (0.8, 0.03, 0.03, [0.0, y0 + 0.015, 0.29]),
        (0.8, 0.03, 0.03, [0.0, y0 + 0.015, -0.29]),
        (0.03, 0.59, 0.03, [0.39, y0 + 0.295, 0.29]),
        (0.03, 0.59, 0.03, [-0.39, y0 + 0.295, 0.29]),
        (0.03, 0.59, 0.03, [0.39, y0 + 0.295, -0.29]),
        (0.03, 0.59, 0.03, [-0.39, y0 + 0.295, -0.29]),
        (0.8, 0.02, 0.6, [0.0, y0 + 0.4, 0.0]),
    ] {
        k.boxed(w, h, d, alu, p, o());
    }
    let m = black(&mut k);
    for sx in [-1.0_f32, 1.0] {
        for sz in [-1.0_f32, 1.0] {
            k.cyl(0.04, 0.04, 0.05, m, [sx * 0.32, 0.04, sz * 0.22], o().r([0.0, 0.0, FRAC_PI_2]).seg(12));
        }
    }
    k.build()
}

fn bottle(c: Col) -> Built {
    let mut k = Kit::new();
    let glass = k.mat("glass", c, mo().rough(0.1).metal(0.1).opacity(0.8));
    k.lathe(&[[0.001, 0.0], [0.038, 0.0], [0.04, 0.02], [0.04, 0.2], [0.03, 0.24], [0.014, 0.27], [0.014, 0.3], [0.001, 0.31]], glass, [0.0, 0.0, 0.0], o().seg(20));
    let label = k.mat("label", "#efe6d0", mo().rough(0.6));
    k.cyl(0.0405, 0.0405, 0.08, label, [0.0, 0.12, 0.0], o().seg(20));
    k.build()
}

fn wineglass(c: Col) -> Built {
    let mut k = Kit::new();
    let glass = k.mat("glass", "#dfe8ee", mo().rough(0.05).opacity(0.35).double());
    k.lathe(&[[0.001, 0.0], [0.035, 0.002], [0.004, 0.006], [0.004, 0.1], [0.025, 0.12], [0.04, 0.16], [0.038, 0.2], [0.036, 0.2]], glass, [0.0, 0.0, 0.0], o().seg(20));
    let wine = k.mat("wine", c, mo().rough(0.2));
    k.lathe(&[[0.001, 0.115], [0.026, 0.125], [0.033, 0.145], [0.001, 0.145]], wine, [0.0, 0.0, 0.0], o().seg(20));
    k.build()
}

fn mug(c: Col) -> Built {
    let mut k = Kit::new();
    let m = k.mat("mug", c, mo().rough(0.35));
    k.cyl(0.042, 0.04, 0.095, m, [0.0, 0.0475, 0.0], o().seg(20));
    k.torus(0.03, 0.008, m, [0.05, 0.05, 0.0], o().r([0.0, 0.0, 0.0]).tube(8).seg(14));
    let coffee = k.mat("coffee", "#3a2416", mo().rough(0.3));
    k.cyl(0.036, 0.036, 0.004, coffee, [0.0, 0.09, 0.0], o().seg(20));
    k.build()
}

fn books(c: Col) -> Built {
    let mut k = Kit::new();
    let mut rand = rng(23);
    let covers = [c, Col::from("#2e4a7a"), Col::from("#3e6a3a"), Col::from("#c9a24a"), Col::from("#e8e2d6")];
    let mut y = 0.0;
    for (i, cover) in covers.into_iter().enumerate() {
        let h = 0.025 + rand() * 0.02;
        // même ordre de tirage que la version JavaScript (arguments de gauche à droite)
        let w = 0.22 + rand() * 0.06;
        let d = 0.16 + rand() * 0.04;
        let m = k.mat(&format!("b{i}"), cover, mo().rough(0.8));
        let x = (rand() - 0.5) * 0.03;
        let z = (rand() - 0.5) * 0.03;
        let ry = (rand() - 0.5) * 0.3;
        k.boxed(w, h, d, m, [x, y + h / 2.0, z], o().r([0.0, ry, 0.0]));
        y += h;
    }
    k.build()
}

fn laptop(c: Col) -> Built {
    let mut k = Kit::new();
    let shell = k.mat("shell", c, mo().rough(0.35).metal(0.6));
    k.rbox(0.33, 0.018, 0.23, 0.006, shell, [0.0, 0.009, 0.02], o());
    let keys = black(&mut k);
    k.boxed(0.28, 0.002, 0.1, keys, [0.0, 0.019, 0.04], o());
    let open: f32 = 1.85; // ~105°
    k.rbox(0.33, 0.22, 0.008, 0.005, shell, [0.0, 0.018 + open.sin() * 0.11, -0.095 + open.cos() * 0.11], o().r([-(PI - open), 0.0, 0.0]));
    let screen = k.mat("screen", "#20324a", mo().rough(0.1).emissive("#0d1622"));
    k.boxed(0.3, 0.19, 0.002, screen, [0.0, 0.018 + open.sin() * 0.11, -0.09 + open.cos() * 0.11], o().r([-(PI - open), 0.0, 0.0]));
    k.build()
}

fn guitar(c: Col) -> Built {
    let mut k = Kit::new();
    let body = k.mat("body", c, mo().rough(0.4));
    let neck = k.mat("neck", "#3a2416", mo().rough(0.6));
    let tilt = euler(-0.25, 0.0, 0.0);
    let at = |x: f32, y: f32, z: f32| tilt * v(x, y, z) + v(0.0, 0.08, 0.05);
    k.ball(1.0, body, at(0.0, 0.28, 0.0), o().q(tilt).s([0.19, 0.2, 0.05]).seg(24));
    k.ball(1.0, body, at(0.0, 0.5, 0.0), o().q(tilt).s([0.145, 0.15, 0.05]).seg(24));
    let hole = black(&mut k);
    k.cyl(0.045, 0.045, 0.01, hole, at(0.0, 0.4, 0.05), o().q(euler(FRAC_PI_2 - 0.25, 0.0, 0.0)).seg(18));
    k.boxed(0.05, 0.5, 0.025, neck, at(0.0, 0.88, 0.0), o().q(tilt));
    k.boxed(0.08, 0.17, 0.02, neck, at(0.0, 1.2, -0.01), o().q(tilt));
    // support
    let m = black(&mut k);
    k.rod([0.0, 0.0, -0.22], [0.0, 0.85, -0.1], 0.01, m, o());
    for s in [-1.0_f32, 1.0] {
        k.rod([0.0, 0.15, -0.18], [s * 0.18, 0.0, 0.12], 0.01, m, o());
    }
    k.build()
}

fn micstand(c: Col) -> Built {
    let mut k = Kit::new();
    let m = k.mat("stand", c, mo().rough(0.5).metal(0.4));
    for i in 0..3 {
        let a = (i as f32 / 3.0) * PI * 2.0;
        k.rod([0.0, 0.35, 0.0], [a.cos() * 0.3, 0.0, a.sin() * 0.3], 0.01, m, o());
    }
    k.cyl(0.012, 0.012, 1.1, m, [0.0, 0.9, 0.0], o());
    k.rod([0.0, 1.42, 0.0], [0.0, 1.5, 0.35], 0.009, m, o());
    let mic = black(&mut k);
    k.capsule([0.0, 1.5, 0.37], [0.0, 1.53, 0.5], 0.02, mic, o());
    let grill = k.mat("grill", "#8a8d92", mo().rough(0.4).metal(0.7));
    k.ball(0.028, grill, [0.0, 1.535, 0.53], o());
    k.build()
}

// ------------------------------------------------------------------ plateau
/// Trépied à trois branches (hauteur de la tête h) avec araignée au sol.
fn tripod(k: &mut Kit, h: f32, m: M) {
    for i in 0..3 {
        let a = (i as f32 / 3.0) * PI * 2.0 + PI / 6.0;
        k.rod([0.0, h, 0.0], [a.cos() * 0.42, 0.02, a.sin() * 0.42], 0.016, m, o());
        k.rod([a.cos() * 0.12, 0.35, a.sin() * 0.12], [0.0, 0.3, 0.0], 0.008, m, o());
    }
}

/// Caméra cinéma : corps, objectif, pare-soleil, viseur, poignée. Visée vers +Z.
fn camera_body(k: &mut Kit, y: f32, c: impl Into<Col>) {
    let body = k.mat("camera", c, mo().rough(0.5).metal(0.2));
    let glass = k.mat("lens", "#0a0c10", mo().rough(0.05).metal(0.5));
    let m = black(k);
    k.rbox(0.16, 0.18, 0.3, 0.02, body, [0.0, y, -0.02], o());
    k.cyl(0.055, 0.06, 0.2, m, [0.0, y, 0.22], o().r([FRAC_PI_2, 0.0, 0.0]).seg(20));
    k.cyl(0.05, 0.05, 0.01, glass, [0.0, y, 0.325], o().r([FRAC_PI_2, 0.0, 0.0]).seg(20));
    k.boxed(0.24, 0.18, 0.12, m, [0.0, y, 0.4], o()); // pare-soleil
    k.boxed(0.2, 0.14, 0.02, glass, [0.0, y, 0.45], o());
    k.capsule([0.1, y + 0.07, -0.08], [0.1, y + 0.07, -0.22], 0.025, m, o()); // viseur
    k.rod([-0.05, y + 0.1, 0.04], [-0.05, y + 0.14, -0.1], 0.012, m, o()); // poignée
    let screen = k.mat("monitor", "#0f1114", mo().rough(0.2));
    k.boxed(0.07, 0.05, 0.1, screen, [-0.12, y + 0.06, 0.02], o());
}

fn camera(c: Col, lens: f32) -> Built {
    let mut k = Kit::new();
    let m = black(&mut k);
    let head = lens - 0.21;
    tripod(&mut k, head, m);
    k.cyl(0.07, 0.08, 0.12, m, [0.0, head + 0.06, 0.0], o().seg(16)); // tête fluide
    k.rod([0.0, head + 0.08, -0.05], [0.0, head - 0.07, -0.45], 0.012, m, o()); // manche
    camera_body(&mut k, lens, c);
    k.build().aim(v(0.0, lens, 0.3)).lens(v(0.0, lens, 0.47))
}

fn dolly(c: Col) -> Built {
    let mut k = Kit::new();
    let rail = metal(&mut k);
    let m = black(&mut k);
    let body = k.mat("dolly", c, mo().rough(0.5).metal(0.3));
    for s in [-1.0_f32, 1.0] {
        k.rod([s * 0.31, 0.04, -1.5], [s * 0.31, 0.04, 1.5], 0.024, rail, o());
    }
    let sleeper = k.mat("sleeper", "#5a4a3a", mo().rough(0.9));
    let mut z: f32 = -1.4;
    while z <= 1.41 {
        k.boxed(0.8, 0.025, 0.06, sleeper, [0.0, 0.012, z], o());
        z += 0.4;
    }
    k.rbox(0.8, 0.12, 0.9, 0.03, body, [0.0, 0.2, 0.0], o());
    for sx in [-1.0_f32, 1.0] {
        for sz in [-1.0_f32, 1.0] {
            k.cyl(0.05, 0.05, 0.05, m, [sx * 0.31, 0.11, sz * 0.32], o().r([0.0, 0.0, FRAC_PI_2]).seg(12));
        }
    }
    k.cyl(0.05, 0.06, 0.95, rail, [0.0, 0.75, 0.15], o().seg(14)); // colonne
    k.boxed(0.38, 0.06, 0.32, m, [0.0, 0.04 + 0.27, -0.3], o()); // siège opérateur
    k.cyl(0.02, 0.02, 0.35, m, [0.0, 0.45, -0.3], o());
    camera_body(&mut k, 1.36, "#25272b");
    k.build().aim(v(0.0, 1.36, 0.3)).lens(v(0.0, 1.36, 0.62))
}

fn directorchair(c: Col) -> Built {
    let mut k = Kit::new();
    let w = k.mat("wood", "#c9a46a", mo().rough(0.6));
    let canvas = k.mat("canvas", c, mo().rough(0.95));
    for s in [-1.0_f32, 1.0] {
        k.rod([s * 0.27, 0.0, -0.2], [s * 0.27, 0.72, 0.2], 0.016, w, o());
        k.rod([s * 0.27, 0.0, 0.2], [s * 0.27, 0.72, -0.2], 0.016, w, o());
        k.boxed(0.04, 0.03, 0.5, w, [s * 0.28, 0.72, 0.0], o()); // accoudoirs
        k.rod([s * 0.27, 0.72, -0.22], [s * 0.27, 1.12, -0.24], 0.016, w, o());
    }
    k.boxed(0.5, 0.01, 0.38, canvas, [0.0, 0.62, 0.0], o());
    k.boxed(0.52, 0.2, 0.01, canvas, [0.0, 1.0, -0.24], o());
    k.build()
}

fn monitor(c: Col) -> Built {
    let mut k = Kit::new();
    let m = k.mat("stand", c, mo().rough(0.5).metal(0.4));
    tripod(&mut k, 1.25, m);
    k.cyl(0.015, 0.015, 0.25, m, [0.0, 1.37, 0.0], o());
    let frame = black(&mut k);
    k.rbox(0.45, 0.3, 0.06, 0.015, frame, [0.0, 1.62, 0.0], o());
    let screen = k.mat("screen", "#20324a", mo().rough(0.1).emissive("#0d1622"));
    k.boxed(0.4, 0.25, 0.004, screen, [0.0, 1.62, 0.031], o());
    k.boxed(0.46, 0.04, 0.2, frame, [0.0, 1.8, 0.07], o()); // casquette
    k.build()
}

fn boom(c: Col) -> Built {
    let mut k = Kit::new();
    let m = black(&mut k);
    tripod(&mut k, 1.4, m);
    k.cyl(0.018, 0.018, 0.6, m, [0.0, 1.7, 0.0], o());
    let base = v(0.0, 2.0, 0.0);
    let tip = v(0.0, 2.45, 2.2);
    let pole = metal(&mut k);
    k.rod(base + v(0.0, -0.15, -0.8), tip, 0.014, pole, o());
    let zeppelin = k.mat("zeppelin", c, mo().rough(1.0));
    k.capsule(tip + v(0.0, -0.12, 0.05), tip + v(0.0, -0.12, 0.42), 0.055, zeppelin, o()); // bonnette
    k.rod(tip, tip + v(0.0, -0.07, 0.2), 0.006, m, o());
    k.build()
}

/// Modèle 3D d'un accessoire ou d'un élément de plateau.
pub fn build_accessory(t: &PropType, p: Props) -> Option<Built> {
    let c = p.col("color", "#8a8f96");
    Some(match t.model.as_deref() {
        Some("suitcase") => suitcase(c),
        Some("cardboard") => cardboard(c),
        Some("crate") => crate_(c),
        Some("flightcase") => flightcase(c),
        Some("bottle") => bottle(c),
        Some("wineglass") => wineglass(c),
        Some("mug") => mug(c),
        Some("books") => books(c),
        Some("laptop") => laptop(c),
        Some("guitar") => guitar(c),
        Some("micstand") => micstand(c),
        Some("camera") => camera(c, p.num("lens").unwrap_or(1.45)),
        Some("dolly") => dolly(c),
        Some("directorchair") => directorchair(c),
        Some("monitor") => monitor(c),
        Some("boom") => boom(c),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn tous_les_accessoires_du_catalogue() {
        let empty = Value::Object(Default::default());
        let mut n = 0;
        for t in arklight_core::library().props.iter().filter(|t| matches!(t.category.as_deref(), Some("props" | "filmset"))) {
            let props = t.extra.get("defaults").unwrap_or(&empty);
            let built = build_accessory(t, Props(props)).unwrap_or_else(|| panic!("{} : modèle {:?} inconnu", t.id, t.model));
            assert!(!built.parts.is_empty(), "{} : aucune pièce", t.id);
            for (mesh, _) in &built.parts {
                assert!(!mesh.vertices.is_empty() && !mesh.indices.is_empty(), "{} : maillage vide", t.id);
                assert!(mesh.vertices.iter().all(|v| v.pos.iter().chain(&v.normal).all(|x| x.is_finite())), "{} : sommet non fini", t.id);
            }
            if matches!(t.model.as_deref(), Some("camera" | "dolly")) {
                assert!(built.aim.is_some() && built.lens.is_some(), "{} : visée ou objectif manquant", t.id);
            }
            n += 1;
        }
        assert!(n >= 16, "{n} accessoires seulement");
    }

    #[test]
    fn hauteur_d_objectif_reglable() {
        let t = arklight_core::library().props.iter().find(|t| t.model.as_deref() == Some("camera")).expect("caméra au catalogue");
        let props = serde_json::json!({ "lens": 1.8 });
        let built = build_accessory(t, Props(&props)).expect("caméra");
        assert!((built.lens.expect("objectif").y - 1.8).abs() < 1e-6);
        // modèle inconnu : rien (la vue retombe sur un simple volume)
        let inconnu: PropType = serde_json::from_value(serde_json::json!({ "id": "x", "name": "x", "category": "props", "model": "inconnu", "size": [1, 1, 1] })).expect("type");
        assert!(build_accessory(&inconnu, Props(&props)).is_none());
    }
}
