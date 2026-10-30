//! Décor et extérieur : pans de mur (avec porte, avec fenêtre), escalier, praticable, colonne,
//! borniol, arbres, buissons, rochers, mobilier urbain (portage de
//! `web/js/viewport/props/set.js`). Face avant vers +Z, posé au sol.

use super::kit::*;
use arklight_core::library::PropType;
use glam::{Mat3, Quat, Vec3};
use std::f32::consts::PI;

fn frame_mat(k: &mut Kit) -> M {
    k.mat("doorframe", "#f1efe9", mo().rough(0.6))
}

fn glass_mat(k: &mut Kit) -> M {
    k.mat("glass", "#9fb8c8", mo().rough(0.05).metal(0.2).opacity(0.35))
}

/// Mur de largeur w et hauteur h, avec ou sans baie (x0..x1, y0..y1), en panneaux simples.
fn wall_with_opening(k: &mut Kit, mat: M, w: f32, h: f32, t: f32, hole: Option<[f32; 4]>) {
    let Some([x0, x1, y0, y1]) = hole else {
        k.boxed(w, h, t, mat, [0.0, h / 2.0, 0.0], o());
        return;
    };
    let mut part = |xa: f32, xb: f32, ya: f32, yb: f32| {
        if xb - xa > 0.001 && yb - ya > 0.001 {
            k.boxed(xb - xa, yb - ya, t, mat, [(xa + xb) / 2.0, (ya + yb) / 2.0, 0.0], o());
        }
    };
    part(-w / 2.0, x0, 0.0, h);
    part(x1, w / 2.0, 0.0, h);
    part(x0, x1, y1, h);
    part(x0, x1, 0.0, y0);
}

fn wall(ty: &PropType, props: Props) -> Built {
    let mut k = Kit::new();
    let paint = k.mat("paint", props.col("color", "#d9d4c8"), mo().rough(0.9));
    let w = props.num("width").unwrap_or(ty.size[0] as f32);
    let h = props.num("height").unwrap_or(ty.size[1] as f32);
    let t = 0.1;
    let trim = k.mat("trim", shade(props.col("color", "#d9d4c8"), 0.75), mo().rough(0.8));
    if ty.model.as_deref() == Some("walldoor") {
        let dw = 0.9;
        wall_with_opening(&mut k, paint, w, h, t, Some([-dw / 2.0, dw / 2.0, 0.0, 2.05]));
        door_leaf(&mut k, dw, 2.05, t, 0.55, "#f1efe9");
    } else if ty.model.as_deref() == Some("wallwindow") {
        let ww = 1.2f32.min(w - 0.4);
        wall_with_opening(&mut k, paint, w, h, t, Some([-ww / 2.0, ww / 2.0, 0.95, 2.15]));
        window_frame(&mut k, ww, 1.2, t, 0.95, "#f1efe9");
    } else {
        wall_with_opening(&mut k, paint, w, h, t, None);
    }
    k.boxed(w, 0.1, t + 0.02, trim, [0.0, 0.05, 0.0], o()); // plinthe
    // jambes de force à l'arrière (décor de studio)
    let strut = k.mat("strut", "#a88a62", mo().rough(0.8));
    for x in [-w / 2.0 + 0.3, w / 2.0 - 0.3] {
        k.rod([x, h * 0.7, -t / 2.0], [x, 0.02, -t / 2.0 - h * 0.45], 0.025, strut, o());
    }
    k.build()
}

/// Battant de porte entrouvert dans une baie de largeur dw (charnière à gauche).
fn door_leaf(k: &mut Kit, dw: f32, dh: f32, t: f32, open: f32, color: impl Into<Col>) {
    let frame = frame_mat(k);
    let leaf = k.mat("leaf", color, mo().rough(0.55));
    for s in [-1.0f32, 1.0] {
        k.boxed(0.05, dh, t + 0.03, frame, [s * (dw / 2.0 + 0.025), dh / 2.0, 0.0], o());
    }
    k.boxed(dw + 0.1, 0.05, t + 0.03, frame, [0.0, dh + 0.025, 0.0], o());
    let w = dw - 0.02;
    let hinge = v(-dw / 2.0 + 0.01, 0.0, t / 2.0);
    let dir = v(open.cos(), 0.0, open.sin());
    let c = hinge + dir * (w / 2.0);
    k.boxed(w, dh - 0.02, 0.04, leaf, [c.x, dh / 2.0, c.z + 0.02], o().r([0.0, -open, 0.0]));
    let knob = hinge + dir * (w - 0.08);
    let knob_mat = k.mat("knob", "#b9a46a", mo().rough(0.3).metal(0.8));
    k.ball(0.03, knob_mat, [knob.x + 0.04 * open.sin(), 1.0, knob.z + 0.06], o());
}

/// Châssis de fenêtre (2 vantaux, croisillon) dans une baie ww × wh posée à la hauteur y0
/// (`wall_t` : épaisseur du mur).
fn window_frame(k: &mut Kit, ww: f32, wh: f32, wall_t: f32, y0: f32, color: impl Into<Col>) {
    let frame = k.mat("frame", color, mo().rough(0.6));
    let glass = glass_mat(k);
    let t = 0.05;
    k.boxed(ww, t, wall_t + 0.02, frame, [0.0, y0 + t / 2.0, 0.0], o());
    k.boxed(ww, t, wall_t + 0.02, frame, [0.0, y0 + wh - t / 2.0, 0.0], o());
    for x in [-ww / 2.0 + t / 2.0, 0.0, ww / 2.0 - t / 2.0] {
        k.boxed(t, wh, wall_t + 0.02, frame, [x, y0 + wh / 2.0, 0.0], o());
    }
    k.boxed(ww - 0.04, 0.03, 0.03, frame, [0.0, y0 + wh * 0.55, 0.0], o());
    k.boxed(ww - 0.04, wh - 0.04, 0.008, glass, [0.0, y0 + wh / 2.0, 0.0], o());
}

fn door(props: Props) -> Built {
    let mut k = Kit::new();
    door_leaf(&mut k, 0.83, 2.04, 0.12, 0.6, props.col("color", "#f1efe9"));
    k.build()
}

fn window_alone(props: Props) -> Built {
    let mut k = Kit::new();
    window_frame(&mut k, 1.2, 1.4, 0.1, 0.0, props.col("color", "#f1efe9"));
    k.build()
}

fn stairs(props: Props) -> Built {
    let mut k = Kit::new();
    let w = k.mat("wood", props.col("color", "#9a7650"), mo().rough(0.7));
    let side = k.mat("side", shade(props.col("color", "#9a7650"), 0.7), mo().rough(0.75));
    let n = 6;
    let rise = 0.18;
    let run = 0.28;
    let nf = n as f32;
    for i in 0..n {
        let i = i as f32;
        k.boxed(1.0, rise * (i + 1.0), run, w, [0.0, (rise * (i + 1.0)) / 2.0, (run * nf) / 2.0 - run * (i + 0.5)], o());
    }
    // rampes : poteau sur la première et la dernière marche, main courante entre les deux
    let z_front = (run * nf) / 2.0 - run / 2.0;
    let z_back = -(run * nf) / 2.0 + run / 2.0;
    for s in [-1.0f32, 1.0] {
        let x = s * 0.47;
        k.rod([x, rise, z_front], [x, rise + 0.9, z_front], 0.018, side, o());
        k.rod([x, rise * nf, z_back], [x, rise * nf + 0.9, z_back], 0.018, side, o());
        k.rod([x, rise + 0.9, z_front], [x, rise * nf + 0.9, z_back], 0.022, side, o());
    }
    k.build()
}

fn riser(props: Props) -> Built {
    let mut k = Kit::new();
    let h = props.num("height").unwrap_or(0.4);
    let top = k.mat("top", props.col("color", "#2a2a2c"), mo().rough(0.8));
    let alu = k.mat("alu", "#a9adb2", mo().rough(0.35).metal(0.8));
    k.boxed(2.0, 0.03, 1.0, top, [0.0, h - 0.015, 0.0], o());
    k.boxed(2.0, 0.05, 1.0, alu, [0.0, h - 0.055, 0.0], o());
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            k.boxed(0.05, h - 0.06, 0.05, alu, [sx * 0.95, (h - 0.06) / 2.0, sz * 0.45], o());
        }
    }
    let skirt = k.mat("skirt", "#111112", mo().rough(1.0));
    k.boxed(2.0, h - 0.08, 0.005, skirt, [0.0, (h - 0.08) / 2.0, 0.5], o()); // jupe noire
    k.build()
}

fn column(props: Props) -> Built {
    let mut k = Kit::new();
    let h = props.num("height").unwrap_or(3.0);
    let m = k.mat("stone", props.col("color", "#e8e4da"), mo().rough(0.85));
    k.boxed(0.5, 0.12, 0.5, m, [0.0, 0.06, 0.0], o());
    k.cyl(0.19, 0.21, h - 0.3, m, [0.0, h / 2.0, 0.0], o().seg(24));
    k.cyl(0.24, 0.2, 0.08, m, [0.0, h - 0.14, 0.0], o().seg(24));
    k.boxed(0.5, 0.1, 0.5, m, [0.0, h - 0.05, 0.0], o());
    k.build()
}

fn drape(props: Props) -> Built {
    let mut k = Kit::new();
    let w = props.num("width").unwrap_or(3.0);
    let h = props.num("height").unwrap_or(3.0);
    let cloth = k.mat("cloth", props.col("color", "#141414"), mo().rough(1.0).double());
    // tissu plissé : plan ondulé
    let mut geo = Geo::plane(w, h - 0.05, ((w * 12.0).round() as u32).max(8), 1);
    for q in &mut geo.pos {
        q.z = ((q.x / w) * PI * w * 4.0).sin() * 0.05;
    }
    geo.compute_vertex_normals();
    k.add(geo, cloth, o().p([0.0, h / 2.0, 0.0]));
    let pipe = k.mat("pipe", "#2a2a2c", mo().rough(0.5).metal(0.6));
    k.rod([-w / 2.0 - 0.1, h, 0.0], [w / 2.0 + 0.1, h, 0.0], 0.02, pipe, o());
    k.build()
}

// ------------------------------------------------------------------ extérieur
fn tree(props: Props) -> Built {
    let mut k = Kit::new();
    let h = props.num("height").unwrap_or(5.0);
    let bark = k.mat("bark", "#5a4330", mo().rough(0.95));
    let leaf = k.mat("leaf", props.col("color", "#3f6b2f"), mo().rough(0.85).flat());
    let leaf2 = k.mat("leaf2", shade(props.col("color", "#3f6b2f"), 0.8), mo().rough(0.85).flat());
    k.cyl(0.1 * (h / 5.0), 0.16 * (h / 5.0), h * 0.5, bark, [0.0, h * 0.25, 0.0], o().seg(10));
    let mut rand = rng(13);
    let cr = h * 0.3;
    for i in 0..14 {
        // même ordre de tirage que la version JavaScript : angle, rayon, taille, hauteur
        let a = rand() * PI * 2.0;
        let r = rand() * cr * 0.6;
        let size = cr * (0.45 + rand() * 0.25);
        let y = h * 0.62 + (rand() - 0.3) * cr * 0.8;
        k.ico(size, if i % 2 == 1 { leaf } else { leaf2 }, [a.cos() * r, y, a.sin() * r], o().detail(1));
    }
    k.build()
}

fn pine(props: Props) -> Built {
    let mut k = Kit::new();
    let h = props.num("height").unwrap_or(4.5);
    let needles = k.mat("needles", props.col("color", "#2c4a2e"), mo().rough(0.9).flat());
    let bark = k.mat("bark", "#4a3626", mo());
    k.cyl(0.07 * (h / 4.5), 0.11 * (h / 4.5), h * 0.25, bark, [0.0, h * 0.125, 0.0], o().seg(8));
    for i in 0..4 {
        let t = i as f32 / 3.0;
        // hauteur de l'étage (cône)
        let hc = h * 0.38 * (1.0 - t * 0.2);
        k.cone(h * 0.24 * (1.0 - t * 0.55), hc, needles, [0.0, h * (0.25 + t * 0.55) + (h * 0.38 - hc) / 2.0, 0.0], o().seg(10));
    }
    k.build()
}

fn palm(props: Props) -> Built {
    let mut k = Kit::new();
    let bark = k.mat("bark", "#7a6248", mo().rough(0.95));
    let frond = k.mat("frond", props.col("color", "#4f7a34"), mo().rough(0.8).double().flat());
    let mut p = Vec3::ZERO;
    for i in 0..6 {
        let i = i as f32;
        let n = p + v(0.06 * i * 0.3, 0.7, 0.04 * i);
        k.rod(p, n, 0.16 - i * 0.013, bark, o().r2(0.15 - i * 0.013));
        p = n;
    }
    for i in 0..9 {
        let a = (i as f32 / 9.0) * PI * 2.0;
        let dir = v(a.cos(), 0.0, a.sin());
        let mid = p + dir * 0.75 + v(0.0, 0.25, 0.0);
        let tip = p + dir * 1.5 + v(0.0, -0.35, 0.0);
        for (a0, a1) in [(p, mid), (mid, tip)] {
            // palme à plat : largeur horizontale, perpendiculaire à la nervure
            let d = a1 - a0;
            let len = d.length();
            let d = d.normalize();
            let x = d.cross(Vec3::Y).normalize();
            let z = x.cross(d).normalize();
            let q = Quat::from_mat3(&Mat3::from_cols(x, d, z));
            k.add(Geo::plane(0.32, len, 1, 1), frond, o().q(q).p((a0 + a1) * 0.5));
        }
    }
    k.build()
}

fn bush(props: Props) -> Built {
    let mut k = Kit::new();
    let leaf = k.mat("leaf", props.col("color", "#46703a"), mo().rough(0.85).flat());
    let leaf2 = k.mat("leaf2", shade(props.col("color", "#46703a"), 0.82), mo().rough(0.85).flat());
    let mut rand = rng(17);
    for i in 0..9 {
        // même ordre de tirage que la version JavaScript : taille, puis x, y, z
        let r = 0.28 + rand() * 0.14;
        let at = [(rand() - 0.5) * 0.8, 0.26 + rand() * 0.22, (rand() - 0.5) * 0.6];
        k.ico(r, if i % 2 == 1 { leaf } else { leaf2 }, at, o());
    }
    k.build()
}

fn rock(props: Props) -> Built {
    let mut k = Kit::new();
    let stone = k.mat("stone", props.col("color", "#7d7a74"), mo().rough(0.95).flat());
    // (géométrie non indexée, comme DodecahedronGeometry : chaque sommet de chaque face est déplacé)
    let mut geo = Geo::dodecahedron(0.5, 1);
    let mut rand = rng(21);
    for q in &mut geo.pos {
        let x = q.x * (0.9 + rand() * 0.25);
        let y = q.y * (0.85 + rand() * 0.2);
        let z = q.z * (0.9 + rand() * 0.2);
        *q = v(x, y, z);
    }
    geo.compute_vertex_normals();
    k.add(geo, stone, o().p([0.0, 0.42, 0.0]).s([1.2, 0.85, 1.0]));
    k.build()
}

fn streetlamp(props: Props) -> Built {
    let mut k = Kit::new();
    let m = k.mat("cast", props.col("color", "#2b2f33"), mo().rough(0.5).metal(0.6));
    k.cyl(0.14, 0.18, 0.5, m, [0.0, 0.25, 0.0], o().seg(12));
    k.cyl(0.05, 0.07, 3.6, m, [0.0, 2.25, 0.0], o().seg(12));
    k.rod([0.0, 3.9, 0.0], [0.0, 4.05, 0.55], 0.035, m, o());
    k.cone(0.22, 0.25, m, [0.0, 3.95, 0.62], o().seg(14));
    let lamp = k.mat("lamp", "#e9e2c8", mo().rough(0.3));
    k.cyl(0.12, 0.12, 0.05, lamp, [0.0, 3.8, 0.62], o().seg(14));
    k.build()
}

fn bench(props: Props) -> Built {
    let mut k = Kit::new();
    let slat = k.mat("slat", props.col("color", "#7a5232"), mo().rough(0.8));
    let iron = k.mat("iron", "#2a2c30", mo().rough(0.5).metal(0.6));
    for i in 0..4 {
        k.boxed(1.8, 0.035, 0.09, slat, [0.0, 0.44, -0.2 + i as f32 * 0.12], o());
    }
    for i in 0..3 {
        k.boxed(1.8, 0.09, 0.03, slat, [0.0, 0.58 + i as f32 * 0.11, -0.3], o().r([-0.15, 0.0, 0.0]));
    }
    for s in [-1.0f32, 1.0] {
        k.boxed(0.05, 0.44, 0.5, iron, [s * 0.75, 0.22, 0.0], o());
        k.boxed(0.05, 0.4, 0.05, iron, [s * 0.75, 0.62, -0.31], o());
    }
    k.build()
}

fn bin(props: Props) -> Built {
    let mut k = Kit::new();
    let m = k.mat("bin", props.col("color", "#3c5a3c"), mo().rough(0.6).metal(0.3));
    k.cyl(0.24, 0.22, 0.85, m, [0.0, 0.45, 0.0], o().seg(20));
    let lid = k.mat("lid", "#2a2c30", mo().rough(0.6));
    k.cyl(0.26, 0.26, 0.06, lid, [0.0, 0.9, 0.0], o().seg(20));
    k.build()
}

fn barrier(props: Props) -> Built {
    let mut k = Kit::new();
    let m = k.mat("steel", props.col("color", "#a9adb2"), mo().rough(0.35).metal(0.8));
    k.rod([-1.0, 0.12, 0.0], [-1.0, 1.1, 0.0], 0.02, m, o());
    k.rod([1.0, 0.12, 0.0], [1.0, 1.1, 0.0], 0.02, m, o());
    k.rod([-1.0, 1.1, 0.0], [1.0, 1.1, 0.0], 0.02, m, o());
    k.rod([-1.0, 0.25, 0.0], [1.0, 0.25, 0.0], 0.02, m, o());
    for i in 1..14 {
        let x = -1.0 + i as f32 * (2.0 / 14.0);
        k.rod([x, 0.25, 0.0], [x, 1.1, 0.0], 0.008, m, o());
    }
    for x in [-0.8f32, 0.8] {
        k.rod([x, 0.12, 0.0], [x, 0.0, 0.3], 0.02, m, o());
        k.rod([x, 0.12, 0.0], [x, 0.0, -0.3], 0.02, m, o());
    }
    k.build()
}

fn cone(props: Props) -> Built {
    let mut k = Kit::new();
    let orange = k.mat("orange", props.col("color", "#f26a1b"), mo().rough(0.6));
    let white = k.mat("white", "#f4f4f2", mo().rough(0.5));
    let base = k.mat("base", "#1f1f22", mo());
    k.boxed(0.34, 0.03, 0.34, base, [0.0, 0.015, 0.0], o());
    k.cone(0.14, 0.47, orange, [0.0, 0.265, 0.0], o().seg(16));
    k.cyl(0.08, 0.1, 0.07, white, [0.0, 0.28, 0.0], o().seg(16));
    k.build()
}

fn sign(props: Props) -> Built {
    let mut k = Kit::new();
    let pole = k.mat("pole", "#a9adb2", mo().rough(0.4).metal(0.7));
    k.cyl(0.03, 0.03, 2.3, pole, [0.0, 1.15, 0.0], o().seg(10));
    let border = k.mat("border", props.col("color", "#c8202a"), mo().rough(0.5));
    k.cyl(0.32, 0.32, 0.02, border, [0.0, 2.25, 0.03], o().r([PI / 2.0, 0.0, 0.0]).seg(28));
    let face = k.mat("face", "#f4f4f2", mo().rough(0.5));
    k.cyl(0.25, 0.25, 0.022, face, [0.0, 2.25, 0.035], o().r([PI / 2.0, 0.0, 0.0]).seg(28));
    k.build()
}

fn trafficlight(props: Props) -> Built {
    let mut k = Kit::new();
    let m = k.mat("mast", props.col("color", "#3a3e44"), mo().rough(0.5).metal(0.5));
    k.cyl(0.06, 0.07, 2.4, m, [0.0, 1.2, 0.0], o().seg(12));
    let case = k.mat("box", "#1c1d20", mo().rough(0.5));
    k.rbox(0.3, 0.85, 0.22, 0.04, case, [0.0, 2.75, 0.0], o());
    for (i, (c, y)) in [("#7a1010", 3.0f32), ("#7a5a10", 2.75), ("#106a2a", 2.5)].into_iter().enumerate() {
        let lamp = k.mat(&format!("l{i}"), c, mo().rough(0.3));
        k.cyl(0.085, 0.085, 0.03, lamp, [0.0, y, 0.115], o().r([PI / 2.0, 0.0, 0.0]).seg(16));
    }
    k.build()
}

/// Modèle 3D d'un élément de décor ou d'extérieur.
pub fn build_set(t: &PropType, p: Props) -> Option<Built> {
    let built = match t.model.as_deref() {
        Some("wall" | "walldoor" | "wallwindow") => wall(t, p),
        Some("door") => door(p),
        Some("window") => window_alone(p),
        Some("stairs") => stairs(p),
        Some("riser") => riser(p),
        Some("column") => column(p),
        Some("drape") => drape(p),
        Some("tree") => tree(p),
        Some("pine") => pine(p),
        Some("palm") => palm(p),
        Some("bush") => bush(p),
        Some("rock") => rock(p),
        Some("streetlamp") => streetlamp(p),
        Some("bench") => bench(p),
        Some("bin") => bin(p),
        Some("barrier") => barrier(p),
        Some("cone") => cone(p),
        Some("sign") => sign(p),
        Some("trafficlight") => trafficlight(p),
        _ => return None,
    };
    Some(built)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Chaque élément de décor et d'extérieur du catalogue se construit, avec ses réglages par
    /// défaut puis sans aucun réglage : des maillages non vides, aucune valeur aberrante.
    #[test]
    fn every_catalogue_set_and_outdoor_type_builds() {
        let empty = json!({});
        let mut count = 0;
        for t in arklight_core::library().props.iter().filter(|t| matches!(t.category.as_deref(), Some("set" | "outdoor"))) {
            let defaults = t.extra.get("defaults").unwrap_or(&empty);
            for props in [defaults, &empty] {
                let built = build_set(t, Props(props)).unwrap_or_else(|| panic!("{} : aucun modèle", t.id));
                assert!(!built.parts.is_empty(), "{} : aucun maillage", t.id);
                for (mesh, _) in &built.parts {
                    assert!(!mesh.vertices.is_empty() && !mesh.indices.is_empty(), "{} : maillage vide", t.id);
                    assert!(mesh.vertices.iter().all(|vx| vx.pos.iter().all(|c| c.is_finite())), "{} : position non finie", t.id);
                    assert!(mesh.vertices.iter().all(|vx| vx.normal.iter().all(|c| c.is_finite())), "{} : normale non finie", t.id);
                }
            }
            count += 1;
        }
        assert!(count > 0, "aucun décor dans le catalogue");
    }

    /// Un modèle inconnu de ce module : rien (le volume par défaut prend le relais).
    #[test]
    fn unknown_model_is_none() {
        let t = PropType {
            id: "inconnu".into(),
            name: "Inconnu".into(),
            short: None,
            category: Some("set".into()),
            model: Some("inconnu".into()),
            size: [1.0, 1.0, 1.0],
            extra: serde_json::Map::new(),
        };
        assert!(build_set(&t, Props(&json!({}))).is_none());
    }
}
