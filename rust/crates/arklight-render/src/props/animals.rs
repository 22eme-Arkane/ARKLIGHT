//! Animaux (portage de `web/js/viewport/props/animals.js`) : quadrupède paramétré par espèce
//! (corps, pattes, cou, tête, oreilles, queue et particularités : crinière, cornes, bois, laine,
//! taches…) et oiseaux. Face vers +Z, pattes au sol.

use super::kit::*;
use std::f32::consts::{FRAC_PI_2, PI};

/// Espèce de quadrupède. Dimensions en mètres. neck : [longueur, angle au-dessus de l'horizontale,
/// rayon] ; head : demi-axes [x, y, z] et inclinaison ; tail : (longueur, rayon, angle, style).
#[derive(Clone, Copy)]
struct Species {
    l: f32,
    r: f32,
    w: f32,
    leg: f32,
    leg_r: f32,
    neck: [f32; 3],
    head: [f32; 4],
    /// demi-axes [x, y, z] et style (« disk » : groin)
    snout: ([f32; 3], &'static str),
    /// style et taille
    ears: (&'static str, f32),
    tail: (f32, f32, f32, &'static str),
    nose: Option<&'static str>,
    hooves: Option<&'static str>,
    mane: bool,
    horns: Option<&'static str>,
    beard: bool,
    antlers: bool,
    udder: bool,
    spots: Option<&'static str>,
    wool: bool,
    dark_head: Option<&'static str>,
}

/// Valeurs absentes de la table JavaScript (pas de crinière, de cornes…).
const BASE: Species = Species {
    l: 0.0,
    r: 0.0,
    w: 0.0,
    leg: 0.0,
    leg_r: 0.0,
    neck: [0.0; 3],
    head: [0.0; 4],
    snout: ([0.0; 3], ""),
    ears: ("", 0.0),
    tail: (0.0, 0.0, 0.0, ""),
    nose: None,
    hooves: None,
    mane: false,
    horns: None,
    beard: false,
    antlers: false,
    udder: false,
    spots: None,
    wool: false,
    dark_head: None,
};

#[rustfmt::skip]
static SPECIES: [(&str, Species); 10] = [
    ("dog", Species { l: 0.62, r: 0.15, w: 0.82, leg: 0.34, leg_r: 0.033, neck: [0.17, 0.95, 0.07], head: [0.085, 0.085, 0.1, 0.15], snout: ([0.05, 0.045, 0.085], ""), ears: ("floppy", 0.085), tail: (0.32, 0.022, 0.45, ""), nose: Some("#1f1a17"), ..BASE }),
    ("smalldog", Species { l: 0.34, r: 0.085, w: 0.85, leg: 0.17, leg_r: 0.019, neck: [0.09, 0.9, 0.042], head: [0.058, 0.055, 0.065, 0.1], snout: ([0.032, 0.028, 0.05], ""), ears: ("pointy", 0.05), tail: (0.15, 0.012, 1.0, ""), nose: Some("#1f1a17"), ..BASE }),
    ("cat", Species { l: 0.36, r: 0.08, w: 0.8, leg: 0.16, leg_r: 0.016, neck: [0.07, 0.75, 0.034], head: [0.055, 0.05, 0.055, 0.05], snout: ([0.026, 0.02, 0.025], ""), ears: ("pointy", 0.045), tail: (0.3, 0.012, 1.0, "up"), nose: Some("#b9787a"), ..BASE }),
    ("horse", Species { l: 1.45, r: 0.3, w: 0.78, leg: 0.95, leg_r: 0.045, neck: [0.62, 0.95, 0.15], head: [0.11, 0.12, 0.3, 0.75], snout: ([0.09, 0.09, 0.13], ""), ears: ("small", 0.1), tail: (0.75, 0.055, -0.35, "hair"), mane: true, hooves: Some("#2a2420"), nose: Some("#2a2420"), ..BASE }),
    ("donkey", Species { l: 1.05, r: 0.25, w: 0.8, leg: 0.68, leg_r: 0.038, neck: [0.42, 0.8, 0.12], head: [0.1, 0.11, 0.24, 0.6], snout: ([0.085, 0.085, 0.1], ""), ears: ("long", 0.24), tail: (0.55, 0.02, -0.4, ""), mane: true, hooves: Some("#2a2420"), nose: Some("#d8d0c4"), ..BASE }),
    ("cow", Species { l: 1.6, r: 0.36, w: 0.85, leg: 0.72, leg_r: 0.06, neck: [0.32, 0.35, 0.17], head: [0.14, 0.15, 0.25, 0.45], snout: ([0.12, 0.1, 0.1], ""), ears: ("side", 0.13), horns: Some("cow"), tail: (0.8, 0.022, -0.2, ""), udder: true, spots: Some("#1e1c1a"), hooves: Some("#2a2420"), nose: Some("#c99a94"), ..BASE }),
    ("sheep", Species { l: 0.85, r: 0.27, w: 0.9, leg: 0.4, leg_r: 0.026, neck: [0.14, 0.6, 0.08], head: [0.075, 0.085, 0.13, 0.35], snout: ([0.06, 0.06, 0.06], ""), ears: ("side", 0.08), tail: (0.12, 0.035, -0.6, ""), wool: true, dark_head: Some("#2a2522"), hooves: Some("#2a2420"), nose: Some("#2a2522"), ..BASE }),
    ("goat", Species { l: 0.82, r: 0.2, w: 0.75, leg: 0.5, leg_r: 0.026, neck: [0.25, 1.0, 0.07], head: [0.065, 0.075, 0.15, 0.55], snout: ([0.05, 0.05, 0.06], ""), ears: ("side", 0.09), horns: Some("goat"), beard: true, tail: (0.1, 0.018, 0.9, ""), hooves: Some("#2a2420"), nose: Some("#5a4a3c"), ..BASE }),
    ("pig", Species { l: 1.0, r: 0.26, w: 0.92, leg: 0.27, leg_r: 0.045, neck: [0.06, 0.3, 0.2], head: [0.15, 0.15, 0.17, 0.12], snout: ([0.075, 0.065, 0.07], "disk"), ears: ("floppy", 0.1), tail: (0.12, 0.012, 0.3, "curl"), hooves: Some("#6b4a3c"), nose: Some("#d98f84"), ..BASE }),
    ("deer", Species { l: 1.12, r: 0.24, w: 0.75, leg: 0.88, leg_r: 0.03, neck: [0.45, 1.05, 0.09], head: [0.085, 0.095, 0.19, 0.55], snout: ([0.06, 0.06, 0.08], ""), ears: ("side", 0.12), antlers: true, tail: (0.12, 0.03, 0.9, ""), hooves: Some("#2a2420"), nose: Some("#1f1a17"), ..BASE }),
];

/// Oiseau : demi-axes du corps, hauteur du corps, rayon de la tête, longueur du cou, couleurs du
/// bec et des pattes, crête, bec plat (et pattes palmées), style de queue.
#[derive(Clone, Copy)]
struct Bird {
    body: [f32; 3],
    y: f32,
    head: f32,
    neck: f32,
    beak: &'static str,
    legs: &'static str,
    comb: bool,
    flat_beak: bool,
    tail: &'static str,
}

#[rustfmt::skip]
static BIRDS: [(&str, Bird); 3] = [
    ("chicken", Bird { body: [0.11, 0.12, 0.16], y: 0.25, head: 0.05, neck: 0.1, beak: "#e3a531", legs: "#e3b43a", comb: true, flat_beak: false, tail: "fan" }),
    ("duck", Bird { body: [0.11, 0.09, 0.2], y: 0.17, head: 0.052, neck: 0.12, beak: "#f0a330", legs: "#f0a330", comb: false, flat_beak: true, tail: "short" }),
    ("pigeon", Bird { body: [0.055, 0.058, 0.1], y: 0.115, head: 0.032, neck: 0.045, beak: "#2a2a2a", legs: "#b5524f", comb: false, flat_beak: false, tail: "long" }),
];

fn quadruped(sp: &Species, coat: Col) -> Built {
    let mut k = Kit::new();
    let fur = k.mat("fur", coat, if sp.wool { mo().rough(0.95).flat() } else { mo().rough(0.95) });
    let dark = k.mat("dark", shade(coat, 0.55), mo().rough(0.9));
    let hoof = k.mat("hoof", sp.hooves.map_or_else(|| shade(coat, 0.45), Col::from), mo().rough(0.6));
    let nose = k.mat("nose", sp.nose.unwrap_or("#2a2420"), mo().rough(0.5));
    let eye = k.mat("eye", "#0b0b0c", mo().rough(0.2));
    let head_mat = match sp.dark_head {
        Some(c) => k.mat("head", c, mo().rough(0.9)),
        None => fur,
    };
    let Species { l, r, w, leg, leg_r, .. } = *sp;
    let body_y = leg + r;
    // corps
    if sp.wool {
        let mut rand = rng(7);
        for _ in 0..26 {
            let a = rand() * PI * 2.0;
            let z = (rand() - 0.5) * (l - r * 0.6);
            k.ico(r * (0.42 + rand() * 0.18), fur, [a.cos() * r * w * 0.62, body_y + a.sin() * r * 0.62, z], o());
        }
        k.capsule([0.0, body_y, -l / 2.0 + r], [0.0, body_y, l / 2.0 - r], r * 0.86, fur, o().s([w, 1.0, 1.0]));
    } else {
        k.capsule([0.0, body_y, -l / 2.0 + r], [0.0, body_y, l / 2.0 - r], r, fur, o().s([w, 1.0, 1.0]).seg(16));
    }
    // pattes : cuisse épaisse, canon fin, sabot ou patte
    let leg_x = r * w * 0.55;
    let leg_z = l / 2.0 - r * 0.75;
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            let top = v(sx * leg_x, body_y - r * 0.2, sz * leg_z);
            let mid = v(sx * leg_x, leg * 0.5, sz * leg_z + if sz < 0.0 { -leg_r * 0.6 } else { leg_r * 0.3 });
            let bot = v(sx * leg_x, leg_r * 0.6, sz * leg_z);
            k.capsule(top, mid, leg_r * 1.6, fur, o());
            k.capsule(mid, bot, leg_r, if sp.wool { head_mat } else { fur }, o());
            k.cyl(leg_r * 1.15, leg_r * 1.3, leg_r * 1.2, hoof, [bot.x, leg_r * 0.6, bot.z + leg_r * 0.2], o());
        }
    }
    // cou et tête
    let [n_len, n_ang, n_r] = sp.neck;
    let n_dir = v(0.0, n_ang.sin(), n_ang.cos());
    let n_base = v(0.0, body_y + r * 0.35, l / 2.0 - r * 0.45);
    let n_end = n_base + n_dir * n_len;
    k.capsule(n_base, n_end, n_r, if sp.wool { fur } else { head_mat }, o());
    let [hx, hy, hz, pitch] = sp.head;
    let hq = euler(pitch, 0.0, 0.0);
    let head = n_end + hq * v(0.0, hy * 0.3, hz * 0.45);
    let at = |x: f32, y: f32, z: f32| head + hq * v(x, y, z);
    k.ball(1.0, head_mat, head, o().q(hq).s([hx, hy, hz]).seg(18));
    let ([sx, sy, sz], snout_style) = sp.snout;
    let snout = at(0.0, -hy * 0.3, hz * 0.85);
    k.ball(1.0, head_mat, snout, o().q(hq).s([sx, sy, sz]).seg(14));
    if snout_style == "disk" {
        k.cyl(sx * 0.9, sx * 0.9, sz * 0.5, nose, at(0.0, -hy * 0.3, hz * 0.85 + sz * 0.95), o().q(euler(FRAC_PI_2 + pitch, 0.0, 0.0)).seg(14));
    } else {
        k.ball(sx * 0.45, nose, at(0.0, -hy * 0.3 + sy * 0.35, hz * 0.85 + sz * 0.85), o());
    }
    for s in [-1.0f32, 1.0] {
        k.ball((hx * 0.14).max(0.006), eye, at(s * hx * 0.72, hy * 0.28, hz * 0.42), o());
    }
    // oreilles
    let (ear_style, ear) = sp.ears;
    for s in [-1.0f32, 1.0] {
        match ear_style {
            "pointy" | "small" | "long" => {
                let base = at(s * hx * 0.5, hy * 0.82, -hz * if ear_style == "small" { 0.35 } else { 0.15 });
                k.cone(ear * if ear_style == "long" { 0.22 } else { 0.42 }, ear, head_mat, base + v(0.0, ear * 0.45, 0.0), o().r([-0.15, 0.0, -s * 0.25]).seg(8));
            }
            "floppy" => {
                k.ball(1.0, dark, at(s * hx * 1.0, hy * 0.15, -hz * 0.05), o().q(hq).s([ear * 0.22, ear * 0.9, ear * 0.55]).seg(10));
            }
            "side" => {
                k.ball(1.0, head_mat, at(s * (hx + ear * 0.4), hy * 0.45, -hz * 0.25), o().s([ear * 0.6, ear * 0.2, ear * 0.32]).seg(10));
            }
            _ => {}
        }
    }
    // queue
    let (t_len, t_r, t_ang, t_style) = sp.tail;
    let t_base = v(0.0, body_y + r * 0.55, -l / 2.0 + r * 0.25);
    match t_style {
        "up" => {
            let mid = t_base + v(0.0, t_len * 0.25, -t_len * 0.45);
            k.capsule(t_base, mid, t_r, fur, o());
            k.capsule(mid, mid + v(0.0, t_len * 0.55, -t_len * 0.05), t_r, fur, o());
        }
        "hair" => {
            let end = t_base + v(0.0, -t_len * 0.85, -t_len * 0.35);
            k.capsule(t_base, end, t_r, dark, o().s([0.7, 1.0, 1.2]));
        }
        "curl" => {
            k.torus(t_len * 0.3, t_r, fur, t_base + v(0.0, 0.0, -t_len * 0.2), o().r([0.0, FRAC_PI_2, 0.0]).tube(6).seg(14));
        }
        _ => {
            k.rod(t_base, t_base + v(0.0, t_ang.sin() * t_len, -t_ang.cos() * t_len), t_r, fur, o().r2(t_r * 0.6));
        }
    }
    // particularités
    if sp.mane {
        k.capsule(n_base + v(0.0, n_r * 0.9, -0.02), at(0.0, hy * 0.9, -hz * 0.6), n_r * 0.35, dark, o().s([0.5, 1.0, 1.0]));
    }
    if sp.horns == Some("cow") {
        for s in [-1.0f32, 1.0] {
            let horn = k.mat("horn", "#e8dcc0", mo().rough(0.5));
            k.cone(0.03, 0.16, horn, at(s * hx * 1.05, hy * 0.85, -hz * 0.25), o().r([0.0, 0.0, -s * 1.1]).seg(8));
        }
    }
    if sp.horns == Some("goat") {
        for s in [-1.0f32, 1.0] {
            let horn = k.mat("horn", "#6b5a48", mo().rough(0.6));
            k.cone(0.022, 0.2, horn, at(s * hx * 0.45, hy * 1.05, -hz * 0.45), o().r([-0.9, 0.0, -s * 0.15]).seg(8));
        }
    }
    if sp.beard {
        k.cone(0.025, 0.09, dark, at(0.0, -hy * 1.05, hz * 0.45), o().r([PI, 0.0, 0.0]).seg(8));
    }
    if sp.antlers {
        let bone = k.mat("antler", "#d8c8a8", mo().rough(0.7));
        for s in [-1.0f32, 1.0] {
            let base = at(s * hx * 0.45, hy * 0.85, -hz * 0.3);
            let top = base + v(s * 0.22, 0.42, -0.22);
            k.rod(base, top, 0.022, bone, o().r2(0.012));
            for t in [0.35, 0.65, 0.9] {
                let p = base.lerp(top, t);
                k.rod(p, p + v(s * 0.05, 0.14, 0.1), 0.012, bone, o().r2(0.006));
            }
        }
    }
    if sp.udder {
        let udder = k.mat("udder", "#e2a8a2", mo().rough(0.6));
        k.ball(0.13, udder, [0.0, leg - 0.02, -l / 2.0 + r * 1.3], o().s([1.0, 0.7, 1.0]));
    }
    if let Some(spots) = sp.spots {
        let spot = k.mat("spot", spots, mo().rough(0.95));
        let mut rand = rng(3);
        for _ in 0..9 {
            let a = rand() * PI * 2.0;
            let z = (rand() - 0.5) * (l - r * 0.8);
            k.ball(r * (0.35 + rand() * 0.25), spot, [a.cos() * r * w * 0.72, body_y + a.sin() * r * 0.72, z], o().s([1.0, 0.8, 1.3]));
        }
    }
    k.build().aim(head)
}

fn bird(b: &Bird, coat: Col) -> Built {
    let mut k = Kit::new();
    let fur = k.mat("plume", coat, mo().rough(0.9));
    let dark = k.mat("dark", shade(coat, 0.6), mo().rough(0.9));
    let beak = k.mat("beak", b.beak, mo().rough(0.5));
    let legs = k.mat("legs", b.legs, mo().rough(0.6));
    let eye = k.mat("eye", "#0b0b0c", mo().rough(0.2));
    let [bx, by, bz] = b.body;
    let c = v(0.0, b.y, 0.0);
    k.ball(1.0, fur, c, o().s([bx, by, bz]).seg(18));
    for s in [-1.0f32, 1.0] {
        k.ball(1.0, dark, [s * bx * 0.85, b.y + by * 0.1, -bz * 0.1], o().s([bx * 0.25, by * 0.6, bz * 0.85]).seg(12));
    }
    // cou et tête
    let neck_base = v(0.0, b.y + by * 0.5, bz * 0.6);
    let head = neck_base + v(0.0, b.neck, b.head * 0.6);
    k.capsule(neck_base, head, b.head * 0.55, fur, o());
    k.ball(b.head, fur, head, o());
    for s in [-1.0f32, 1.0] {
        k.ball(b.head * 0.16, eye, head + v(s * b.head * 0.75, b.head * 0.2, b.head * 0.4), o());
    }
    if b.flat_beak {
        k.rbox(b.head * 0.8, b.head * 0.25, b.head * 1.3, 0.01, beak, head + v(0.0, -b.head * 0.25, b.head * 1.2), o());
    } else {
        k.cone(b.head * 0.32, b.head * 0.9, beak, head + v(0.0, -b.head * 0.1, b.head * 1.25), o().r([FRAC_PI_2, 0.0, 0.0]).seg(8));
    }
    if b.comb {
        let red = k.mat("comb", "#c8202a", mo().rough(0.6));
        for z in [-0.4, 0.0, 0.4] {
            k.ball(b.head * 0.32, red, head + v(0.0, b.head * 1.0, z * b.head), o());
        }
        k.ball(b.head * 0.25, red, head + v(0.0, -b.head * 0.85, b.head * 0.75), o().s([0.6, 1.3, 0.8]));
    }
    // queue
    let tail = v(0.0, b.y + by * 0.35, -bz * 0.95);
    if b.tail == "fan" {
        for a in [-0.35f32, 0.0, 0.35] {
            k.ball(1.0, dark, tail + v(a.sin() * 0.05, 0.08, -0.03), o().r([-0.7, 0.0, a]).s([0.025, 0.12, 0.06]).seg(10));
        }
    } else {
        k.cone(bx * 0.6, bz * if b.tail == "long" { 0.8 } else { 0.4 }, dark, tail + v(0.0, 0.0, -bz * 0.2), o().r([-FRAC_PI_2 - 0.25, 0.0, 0.0]).s([1.0, 1.0, 0.35]).seg(8));
    }
    // pattes
    let leg_top = b.y - by * 0.7;
    for s in [-1.0f32, 1.0] {
        let x = s * bx * 0.4;
        k.rod([x, leg_top, 0.0], [x, 0.01, bz * 0.05], (bx * 0.06).max(0.004), legs, o());
        if b.flat_beak {
            k.boxed(bx * 0.45, 0.006, bz * 0.35, legs, [x, 0.004, bz * 0.2], o());
        } else {
            for a in [-0.5f32, 0.0, 0.5] {
                k.rod([x, 0.006, bz * 0.05], [x + a.sin() * bz * 0.3, 0.004, bz * 0.05 + a.cos() * bz * 0.3], (bx * 0.04).max(0.003), legs, o());
            }
        }
    }
    k.build().aim(head)
}

/// Modèle 3D d'un animal (espèce du catalogue, couleur du pelage).
pub fn build_animal(species: &str, p: Props) -> Built {
    if let Some((_, b)) = BIRDS.iter().find(|(n, _)| *n == species) {
        return bird(b, p.col("color", "#9a9a9a"));
    }
    let sp = SPECIES.iter().find(|(n, _)| *n == species).map_or(&SPECIES[0].1, |(_, s)| s);
    quadruped(sp, p.col("color", "#8a6a4a"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn check(species: &str, props: &Value) {
        let b = build_animal(species, Props(props));
        assert!(!b.parts.is_empty(), "{species} : aucune pièce");
        assert!(b.aim.is_some_and(|a| a.is_finite()), "{species} : point visé absent");
        for (mesh, _) in &b.parts {
            assert!(!mesh.vertices.is_empty(), "{species} : maillage vide");
            assert!(mesh.vertices.iter().all(|v| v.pos.iter().all(|c| c.is_finite())), "{species} : position non finie");
        }
    }

    #[test]
    fn every_catalogue_animal_builds() {
        let empty = json!({});
        let mut n = 0;
        for t in arklight_core::library().props.iter().filter(|t| t.category.as_deref() == Some("animals")) {
            let species = t.extra.get("species").and_then(Value::as_str).unwrap_or("");
            check(species, t.extra.get("defaults").unwrap_or(&empty));
            n += 1;
        }
        assert!(n > 0, "aucun animal dans le catalogue");
        // espèce inconnue : chien ; sans couleur : pelage par défaut
        check("licorne", &empty);
        check("", &json!({ "color": "" }));
    }
}
