//! Véhicules (portage de `web/js/viewport/props/vehicles.js`) : voitures (profil latéral extrudé
//! et habitacle vitré), utilitaires, camion, remorques, deux-roues. Avant vers +Z, roues au sol.
//! Dimensions des modèles courants.

use super::kit::*;
use glam::Quat;
use std::f32::consts::{FRAC_PI_2, PI};

/// profil (x = longueur) → avant vers +Z
fn turn() -> Quat {
    euler(0.0, -PI / 2.0, 0.0)
}

/// Matières communes : peinture, vitres, pneus, jantes, phares, feux.
#[derive(Clone, Copy)]
struct Mats {
    paint: M,
    trim: M,
    glass: M,
    tire: M,
    rim: M,
    head: M,
    tail: M,
    plate: M,
}

fn materials(k: &mut Kit, paint: Col) -> Mats {
    Mats {
        paint: k.mat("paint", paint, mo().rough(0.35).metal(0.35)),
        trim: k.mat("trim", "#1b1c1f", mo().rough(0.6)),
        glass: k.mat("glass", "#1d2a33", mo().rough(0.08).metal(0.4)),
        tire: k.mat("tire", "#141416", mo().rough(0.9)),
        rim: k.mat("rim", "#b9bdc2", mo().rough(0.3).metal(0.8)),
        head: k.mat("headlight", "#f4f1e6", mo().rough(0.2).emissive("#2a2a22")),
        tail: k.mat("taillight", "#a3121a", mo().rough(0.3).emissive("#3a0306")),
        plate: k.mat("plate", "#eef0f2", mo().rough(0.5)),
    }
}

fn wheel(k: &mut Kit, m: &Mats, x: f32, z: f32, r: f32, w: f32) {
    let q = euler(0.0, 0.0, FRAC_PI_2);
    k.cyl(r, r, w, m.tire, [x, r, z], o().q(q).seg(20));
    k.cyl(r * 0.62, r * 0.62, w + 0.012, m.rim, [x, r, z], o().q(q).seg(16));
}

/// Silhouette latérale d'une voiture (x de l'arrière 0 à l'avant L) avec passages de roues.
/// body : points du haut de caisse de l'arrière vers l'avant ; axles : positions des essieux.
fn body_shape(l: f32, clr: f32, r: f32, axles: &[f32], top: &[[f32; 2]]) -> Shape {
    let mut s = Shape::new();
    let ar = r * 1.12;
    s.move_to(l * 0.985, clr + 0.05);
    // dessous et passages de roues, de l'avant vers l'arrière
    let mut sorted = axles.to_vec();
    sorted.sort_by(|a, b| b.total_cmp(a));
    for ax in sorted {
        s.line_to(ax + ar, clr);
        s.line_to(ax + ar, r);
        s.absarc(ax, r, ar, 0.0, PI, false);
        s.line_to(ax - ar, clr);
    }
    s.line_to(l * 0.015, clr + 0.05);
    // ligne du haut : de l'arrière vers l'avant
    for &[x, y] in top {
        s.line_to(x, y);
    }
    s.line_to(l * 0.985, clr + 0.05);
    s
}

/// Cotes d'une voiture particulière.
struct CarDims {
    l: f32,
    w: f32,
    h: f32,
    clr: f32,
    r: f32,
    axles: [f32; 2],
    belt: f32,
    hood: [f32; 2],
    ws: [f32; 2],
    roof: [f32; 2],
    rear: [f32; 2],
}

/// Voiture particulière : caisse, habitacle vitré, toit, roues, phares et feux.
fn car(kind: &str, paint: Col) -> Built {
    let dims = match kind {
        "sedan" => CarDims { l: 4.72, w: 1.84, h: 1.45, clr: 0.14, r: 0.32, axles: [0.95, 3.75], belt: 0.86, hood: [3.75, 0.8], ws: [3.05, 1.4], roof: [1.35, 1.42], rear: [0.55, 0.9] },
        "suv" => CarDims { l: 4.6, w: 1.9, h: 1.74, clr: 0.22, r: 0.37, axles: [0.85, 3.6], belt: 1.08, hood: [3.65, 1.02], ws: [3.0, 1.68], roof: [0.45, 1.7], rear: [0.12, 1.08] },
        "pickup" => CarDims { l: 5.3, w: 1.92, h: 1.84, clr: 0.24, r: 0.39, axles: [1.0, 4.25], belt: 1.1, hood: [4.3, 1.06], ws: [3.65, 1.78], roof: [2.55, 1.8], rear: [2.45, 1.1] },
        // "city"
        _ => CarDims { l: 3.95, w: 1.75, h: 1.48, clr: 0.15, r: 0.3, axles: [0.62, 3.15], belt: 0.86, hood: [3.2, 0.82], ws: [2.55, 1.42], roof: [0.55, 1.45], rear: [0.12, 0.88] },
    };
    let mut k = Kit::new();
    let m = materials(&mut k, paint);
    let CarDims { l, w, h, clr, r, axles, belt, hood, ws, roof, rear } = dims;
    let pickup = kind == "pickup";
    // caisse jusqu'à la ligne de ceinture
    let top: Vec<[f32; 2]> = if pickup {
        vec![[0.02, belt * 0.85], [0.05, belt], [rear[0], belt], [hood[0], hood[1]], [l * 0.985, hood[1] - 0.12], [l, 0.6]]
    } else {
        vec![[0.0, 0.6 * h], [0.05, belt], [hood[0], hood[1]], [l * 0.975, hood[1] - 0.06], [l, hood[1] - 0.22]]
    };
    k.extrude(&body_shape(l, clr, r, &axles, &top), w, m.paint, o().bevel(0.05).q(turn()).p([0.0, 0.0, -l / 2.0]).curve(14));
    // habitacle vitré (plus étroit), toit et montants
    let cab = [[rear[0], rear[1]], [hood[0] - 0.05, belt], [ws[0], ws[1]], [roof[0], roof[1]]];
    let cab_w = w * 0.86;
    k.extrude_points(&[cab[0], cab[1], cab[2], cab[3]], cab_w, m.glass, o().q(turn()).p([0.0, 0.0, -l / 2.0]));
    k.extrude_points(
        &[[roof[0] - 0.02, roof[1] - 0.03], [ws[0] + 0.02, ws[1] - 0.03], [ws[0] - 0.01, ws[1] + 0.025], [roof[0], roof[1] + 0.025]],
        cab_w + 0.02,
        m.paint,
        o().q(turn()).p([0.0, 0.0, -l / 2.0]),
    );
    let bx = ((roof[0] + ws[0]) / 2.0) - l / 2.0;
    k.boxed(cab_w + 0.025, roof[1] - belt, 0.09, m.paint, [0.0, (roof[1] + belt) / 2.0, bx], o());
    if pickup {
        // benne ouverte
        let bed_l = rear[0] - 0.08;
        let zc = -l / 2.0 + bed_l / 2.0 + 0.05;
        for s in [-1.0f32, 1.0] {
            k.boxed(0.06, 0.42, bed_l, m.paint, [s * (w / 2.0 - 0.03), belt - 0.21, zc], o());
        }
        k.boxed(w, 0.42, 0.06, m.paint, [0.0, belt - 0.21, -l / 2.0 + 0.06], o());
        k.boxed(w - 0.1, 0.03, bed_l, m.trim, [0.0, belt - 0.4, zc], o());
    }
    // roues, phares, feux, plaques, rétroviseurs
    for ax in axles {
        for s in [-1.0f32, 1.0] {
            wheel(&mut k, &m, s * (w / 2.0 - 0.12), ax - l / 2.0, r, 0.22);
        }
    }
    for s in [-1.0f32, 1.0] {
        k.rbox(0.28, 0.1, 0.06, 0.02, m.head, [s * (w / 2.0 - 0.22), hood[1] - 0.16, l / 2.0 - 0.04], o());
        k.rbox(0.26, 0.09, 0.05, 0.02, m.tail, [s * (w / 2.0 - 0.2), (if pickup { belt } else { 0.62 * h }) - 0.12, -l / 2.0 + 0.03], o());
        k.boxed(0.04, 0.09, 0.16, m.trim, [s * (cab_w / 2.0 + 0.1), belt + 0.08, hood[0] - l / 2.0 - 0.12], o());
    }
    k.boxed(0.52, 0.11, 0.02, m.plate, [0.0, 0.42, l / 2.0 + 0.005], o());
    k.boxed(0.52, 0.11, 0.02, m.plate, [0.0, 0.55, -l / 2.0 - 0.005], o());
    k.boxed(w * 0.98, 0.12, 0.08, m.trim, [0.0, clr + 0.12, l / 2.0 - 0.02], o());
    k.boxed(w * 0.98, 0.12, 0.08, m.trim, [0.0, clr + 0.12, -l / 2.0 + 0.02], o());
    k.build().aim(v(0.0, h * 0.55, 0.0))
}

/// Utilitaire, fourgon : nez court et grand caisson.
fn van(kind: &str, paint: Col) -> Built {
    // (L, W, H, R, essieux, nez)
    let (l, w, h, r, axles, nose): (f32, f32, f32, f32, [f32; 2], f32) =
        if kind == "bigvan" { (6.0, 2.02, 2.68, 0.36, [1.2, 4.75], 0.95) } else { (5.0, 1.96, 1.98, 0.34, [0.9, 4.0], 0.85) };
    let mut k = Kit::new();
    let m = materials(&mut k, paint);
    let clr = 0.2;
    let top = [[0.0, h - 0.06], [0.05, h], [l - nose - 0.25, h], [l - nose * 0.45, h * 0.6], [l - 0.05, h * 0.47], [l, h * 0.3]];
    k.extrude(&body_shape(l, clr, r, &axles, &top), w, m.paint, o().bevel(0.06).q(turn()).p([0.0, 0.0, -l / 2.0]).curve(14));
    // pare-brise (juste devant la tôle inclinée) et vitres de cabine
    let z0 = -l / 2.0;
    let a = [l - nose * 0.45, h * 0.6];
    let b = [l - nose - 0.25, h];
    let out = 0.025; // décalage vers l'avant, perpendiculaire au pare-brise
    let len = (b[0] - a[0]).hypot(b[1] - a[1]);
    let n = [(b[1] - a[1]) / len, -(b[0] - a[0]) / len];
    let on = |t: f32| [a[0] + (b[0] - a[0]) * t + n[0] * out, a[1] + (b[1] - a[1]) * t + n[1] * out];
    k.extrude_points(
        &[on(0.04), on(0.9), [l - nose - 0.75, on(0.9)[1]], [l - nose - 0.75, h * 0.62], [on(0.04)[0] - 0.05, h * 0.62]],
        w + 0.012,
        m.glass,
        o().q(turn()).p([0.0, 0.0, z0]),
    );
    for ax in axles {
        for s in [-1.0f32, 1.0] {
            wheel(&mut k, &m, s * (w / 2.0 - 0.13), ax - l / 2.0, r, 0.24);
        }
    }
    for s in [-1.0f32, 1.0] {
        k.rbox(0.3, 0.13, 0.05, 0.02, m.head, [s * (w / 2.0 - 0.25), h * 0.4, l / 2.0 - 0.02], o());
        k.rbox(0.12, 0.3, 0.05, 0.02, m.tail, [s * (w / 2.0 - 0.1), h * 0.55, -l / 2.0 - 0.01], o());
        k.boxed(0.05, 0.22, 0.18, m.trim, [s * (w / 2.0 + 0.08), h * 0.68, l / 2.0 - nose - 0.35], o());
    }
    k.boxed(0.08, h * 0.82, 0.02, m.trim, [0.0, h * 0.5, -l / 2.0 - 0.012], o()); // portes arrière
    k.boxed(w * 0.98, 0.14, 0.1, m.trim, [0.0, clr + 0.12, l / 2.0 - 0.02], o());
    k.boxed(0.52, 0.11, 0.02, m.plate, [0.0, 0.45, l / 2.0 + 0.01], o());
    k.build().aim(v(0.0, h * 0.55, 0.0))
}

/// Camion porteur à caisse (type camion électro) : cabine, caisse, hayon, 6 roues.
fn truck(paint: Col) -> Built {
    let mut k = Kit::new();
    let m = materials(&mut k, paint);
    let box_m = k.mat("box", "#f2f2f0", mo().rough(0.7));
    let l = 8.4;
    let w = 2.5;
    let r = 0.5;
    let cab_l = 2.0;
    let zc = l / 2.0 - cab_l / 2.0;
    // cabine
    k.rbox(w, 2.2, cab_l, 0.12, m.paint, [0.0, 0.9 + 1.1, zc], o());
    k.boxed(w - 0.2, 0.9, 0.05, m.glass, [0.0, 2.35, l / 2.0 + 0.005], o());
    for s in [-1.0f32, 1.0] {
        k.boxed(0.05, 0.75, 1.0, m.glass, [s * (w / 2.0 + 0.004), 2.4, zc + 0.3], o());
    }
    k.boxed(w - 0.3, 0.45, 0.06, m.trim, [0.0, 1.15, l / 2.0 + 0.01], o()); // calandre
    for s in [-1.0f32, 1.0] {
        k.rbox(0.32, 0.18, 0.06, 0.03, m.head, [s * (w / 2.0 - 0.3), 1.0, l / 2.0 + 0.01], o());
        k.boxed(0.06, 0.4, 0.06, m.trim, [s * (w / 2.0 + 0.15), 2.3, l / 2.0 - 0.25], o());
    }
    // caisse
    let box_l = l - cab_l - 0.15;
    let bz = -l / 2.0 + box_l / 2.0;
    k.rbox(w + 0.05, 2.5, box_l, 0.05, box_m, [0.0, 1.1 + 1.25, bz], o());
    k.boxed(w + 0.07, 0.08, box_l, m.trim, [0.0, 1.12, bz], o());
    k.boxed(w - 0.2, 0.06, 0.04, m.trim, [0.0, 1.2, -l / 2.0 - 0.02], o()); // hayon
    for s in [-1.0f32, 1.0] {
        k.rbox(0.12, 0.3, 0.05, 0.02, m.tail, [s * (w / 2.0 - 0.12), 1.0, -l / 2.0 - 0.01], o());
    }
    // châssis et roues (jumelées à l'arrière)
    k.boxed(1.0, 0.3, l - 0.6, m.trim, [0.0, 0.75, 0.0], o());
    wheel(&mut k, &m, w / 2.0 - 0.2, l / 2.0 - 1.2, r, 0.32);
    wheel(&mut k, &m, -(w / 2.0 - 0.2), l / 2.0 - 1.2, r, 0.32);
    for s in [-1.0f32, 1.0] {
        for dx in [0.2, 0.55] {
            wheel(&mut k, &m, s * (w / 2.0 - dx), -l / 2.0 + 2.0, r, 0.3);
        }
    }
    k.build().aim(v(0.0, 1.8, 0.0))
}

/// Remorque : groupe électrogène capoté ou loge (caravane).
fn trailer(kind: &str, paint: Col) -> Built {
    let mut k = Kit::new();
    let m = materials(&mut k, paint);
    let is_gen = kind == "generator";
    let l = if is_gen { 3.2 } else { 5.6 };
    let w = if is_gen { 1.7 } else { 2.3 };
    let h = if is_gen { 1.55 } else { 2.25 };
    let r = if is_gen { 0.33 } else { 0.36 };
    let zc = if is_gen { -0.3 } else { -0.4 };
    let y0 = r + 0.25;
    k.rbox(w, h, l, if is_gen { 0.06 } else { 0.18 }, m.paint, [0.0, y0 + h / 2.0, zc], o());
    if is_gen {
        for i in 0..6 {
            for s in [-1.0f32, 1.0] {
                k.boxed(0.02, 0.05, 1.2, m.trim, [s * (w / 2.0 + 0.005), y0 + 0.4 + i as f32 * 0.12, zc - 0.6], o());
            }
        }
        k.boxed(0.5, 0.35, 0.03, m.trim, [w * 0.2, y0 + h * 0.6, zc + l / 2.0 + 0.01], o()); // tableau électrique
        k.cyl(0.07, 0.07, 0.35, m.trim, [-w * 0.3, y0 + h + 0.17, zc - l * 0.3], o()); // échappement
    } else {
        for s in [-1.0f32, 1.0] {
            for z in [-1.6, 0.4, 1.9] {
                k.boxed(0.02, 0.6, 0.9, m.glass, [s * (w / 2.0 + 0.006), y0 + 1.35, zc + z], o());
            }
        }
        k.boxed(0.02, 1.85, 0.75, m.trim, [w / 2.0 + 0.008, y0 + 1.0, zc - 0.5], o()); // porte
        let roof = k.mat("roof", "#d8d8d4", mo().rough(0.6));
        k.boxed(w * 0.6, 0.12, 0.6, roof, [0.0, y0 + h + 0.06, zc], o()); // lanterneau
    }
    // timon, roues, béquille
    k.rod([0.0, y0 + 0.1, zc + l / 2.0], [0.0, y0 - 0.05, zc + l / 2.0 + 1.2], 0.05, m.trim, o());
    k.cyl(0.03, 0.03, y0, m.trim, [0.0, y0 / 2.0, zc + l / 2.0 + 1.1], o());
    for s in [-1.0f32, 1.0] {
        wheel(&mut k, &m, s * (w / 2.0 - 0.12), zc, r, 0.22);
    }
    k.build().aim(v(0.0, y0 + h / 2.0, zc))
}

/// Deux-roues : moto, scooter, vélo.
fn two_wheeler(kind: &str, paint: Col) -> Built {
    let mut k = Kit::new();
    let m = materials(&mut k, paint);
    let metal = k.mat("metal", "#9da2a8", mo().rough(0.3).metal(0.8));
    let is_bike = kind == "bicycle";
    let r = if is_bike { 0.34 } else if kind == "scooter" { 0.24 } else { 0.32 };
    let wb = if is_bike { 1.05 } else if kind == "scooter" { 1.3 } else { 1.45 };
    let zf = wb / 2.0;
    let zr = -wb / 2.0;
    let q = euler(0.0, PI / 2.0, 0.0);
    for z in [zf, zr] {
        k.torus(r - (if is_bike { 0.015 } else { 0.05 }), if is_bike { 0.018 } else { 0.055 }, m.tire, [0.0, r, z], o().q(q).tube(10).seg(28));
        if is_bike {
            for i in 0..8 {
                let a = (i as f32 / 8.0) * PI * 2.0;
                k.rod([0.0, r, z], [0.0, r + a.sin() * (r - 0.03), z + a.cos() * (r - 0.03)], 0.003, metal, o());
            }
        } else {
            k.cyl(r * 0.55, r * 0.55, 0.06, m.rim, [0.0, r, z], o().r([0.0, 0.0, FRAC_PI_2]).seg(16));
        }
    }
    if is_bike {
        let frame = k.mat("frame", paint, mo().rough(0.4).metal(0.5));
        let bb = v(0.0, r, 0.02);
        let seat = v(0.0, r + 0.52, -0.18);
        let head = v(0.0, r + 0.5, zf - 0.22);
        k.rod(bb, seat, 0.017, frame, o());
        k.rod(bb, head, 0.02, frame, o());
        k.rod(seat + v(0.0, -0.05, 0.02), head + v(0.0, 0.02, 0.0), 0.016, frame, o());
        for s in [-1.0f32, 1.0] {
            k.rod(bb, v(s * 0.05, r, zr), 0.011, frame, o());
            k.rod(seat + v(0.0, -0.04, 0.0), v(s * 0.05, r, zr), 0.01, frame, o());
            k.rod(head, v(s * 0.04, r, zf), 0.013, frame, o());
        }
        k.rod(seat, seat + v(0.0, 0.12, -0.03), 0.012, metal, o());
        k.rbox(0.13, 0.05, 0.26, 0.02, m.trim, seat + v(0.0, 0.15, -0.01), o());
        k.rod(head, head + v(0.0, 0.18, -0.05), 0.014, metal, o());
        k.rod(head + v(-0.27, 0.18, -0.06), head + v(0.27, 0.18, -0.06), 0.012, metal, o());
        k.cyl(0.09, 0.09, 0.02, metal, [0.05, r, 0.02], o().r([0.0, 0.0, FRAC_PI_2])); // plateau
        return k.build().aim(v(0.0, 0.8, 0.0));
    }
    if kind == "scooter" {
        k.rbox(0.36, 0.45, 0.85, 0.14, m.paint, [0.0, r + 0.32, -0.2], o()); // coque arrière
        k.rbox(0.34, 0.12, 0.6, 0.05, m.trim, [0.0, r + 0.6, -0.25], o()); // selle
        k.rbox(0.32, 0.08, 0.45, 0.03, m.trim, [0.0, r + 0.12, 0.25], o()); // plancher
        k.rbox(0.42, 0.9, 0.22, 0.1, m.paint, [0.0, r + 0.5, zf - 0.18], o().r([-0.25, 0.0, 0.0])); // tablier
        k.rod([0.0, r, zf], [0.0, r + 0.95, zf - 0.25], 0.025, metal, o());
        k.rod([-0.33, r + 0.95, zf - 0.27], [0.33, r + 0.95, zf - 0.27], 0.016, m.trim, o());
        k.rbox(0.18, 0.1, 0.06, 0.02, m.head, [0.0, r + 0.82, zf - 0.08], o());
        return k.build().aim(v(0.0, 0.8, 0.0));
    }
    // moto
    k.rbox(0.3, 0.22, 0.55, 0.1, m.paint, [0.0, r + 0.62, 0.1], o()); // réservoir
    k.rbox(0.26, 0.09, 0.65, 0.04, m.trim, [0.0, r + 0.55, -0.42], o()); // selle
    k.rbox(0.32, 0.35, 0.42, 0.08, metal, [0.0, r + 0.2, 0.05], o()); // moteur
    k.rod([0.0, r + 0.5, 0.35], [0.0, r, zf], 0.03, metal, o()); // fourche
    k.rod([0.0, r + 0.35, -0.1], [0.0, r, zr], 0.035, m.trim, o()); // bras oscillant
    k.rod([-0.38, r + 0.78, 0.42], [0.38, r + 0.78, 0.42], 0.016, m.trim, o()); // guidon
    k.rod([0.0, r + 0.5, 0.35], [0.0, r + 0.78, 0.42], 0.025, metal, o());
    k.rbox(0.2, 0.18, 0.12, 0.05, m.head, [0.0, r + 0.62, 0.55], o());
    k.rod([0.14, r + 0.12, 0.0], [0.17, r + 0.25, -0.8], 0.045, metal, o()); // échappement
    k.rbox(0.24, 0.1, 0.28, 0.04, m.paint, [0.0, r + 0.42, zr + 0.05], o()); // garde-boue arrière
    k.build().aim(v(0.0, 0.9, 0.0))
}

/// Modèle 3D d'un véhicule du catalogue.
pub fn build_vehicle(kind: &str, p: Props) -> Built {
    let paint = p.col("color", "#8a8f96");
    match kind {
        "city" | "sedan" | "suv" | "pickup" => car(kind, paint),
        "van" | "bigvan" => van(kind, paint),
        "truck" => truck(paint),
        "generator" | "caravan" => trailer(kind, paint),
        _ => two_wheeler(kind, paint),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// Chaque véhicule du catalogue se construit : des maillages, des sommets, aucune valeur aberrante.
    #[test]
    fn every_catalogue_vehicle_builds() {
        let empty = json!({});
        let mut count = 0;
        for t in arklight_core::library().props.iter().filter(|t| t.category.as_deref() == Some("vehicles")) {
            let kind = t.extra.get("kind").and_then(Value::as_str).unwrap_or("");
            let props = t.extra.get("defaults").unwrap_or(&empty);
            let built = build_vehicle(kind, Props(props));
            assert!(!built.parts.is_empty(), "{} : aucun maillage", t.id);
            for (mesh, _) in &built.parts {
                assert!(!mesh.vertices.is_empty(), "{} : maillage vide", t.id);
                assert!(mesh.vertices.iter().all(|vx| vx.pos.iter().all(|c| c.is_finite())), "{} : position non finie", t.id);
            }
            assert!(built.aim.is_some_and(|a| a.is_finite()), "{} : point visé", t.id);
            count += 1;
        }
        assert!(count > 0, "aucun véhicule dans le catalogue");
    }
}
