//! Mobilier : assises, tables, lits, rangements, cuisine, électroménager, piano, plantes…
//! Face avant vers +Z, posé au sol. Dimensions des meubles courants.
//! (portage de `web/js/viewport/props/furniture.js`)

use super::kit::*;
use arklight_core::library::PropType;
use serde_json::Value;
use std::f32::consts::PI;

fn wood(k: &mut Kit, c: impl Into<Col>) -> M {
    k.mat("wood", c, mo().rough(0.7))
}

fn fabric(k: &mut Kit, c: impl Into<Col>) -> M {
    k.mat("fabric", c, mo().rough(0.95))
}

fn metal(k: &mut Kit) -> M {
    k.mat("metal", "#a9adb2", mo().rough(0.35).metal(0.8))
}

fn dark(k: &mut Kit) -> M {
    k.mat("dark", "#1d1e21", mo().rough(0.6))
}

/// Quatre pieds droits sous un plateau (w × d), de hauteur h.
#[allow(clippy::too_many_arguments)]
fn legs4(k: &mut Kit, w: f32, d: f32, h: f32, r: f32, mat: M, inset: f32, square: bool) {
    for sx in [-1.0f32, 1.0] {
        for sz in [-1.0f32, 1.0] {
            let p = [sx * (w / 2.0 - inset), h / 2.0, sz * (d / 2.0 - inset)];
            if square {
                k.boxed(r * 2.0, h, r * 2.0, mat, p, o());
            } else {
                k.cyl(r, r * 0.85, h, mat, p, o().seg(10));
            }
        }
    }
}

fn chair(c: Col) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    legs4(&mut k, 0.42, 0.42, 0.45, 0.018, w, 0.03, true);
    k.boxed(0.45, 0.035, 0.44, w, [0.0, 0.46, 0.0], o());
    for s in [-1.0f32, 1.0] {
        k.boxed(0.035, 0.45, 0.035, w, [s * 0.195, 0.69, -0.195], o());
    }
    k.boxed(0.42, 0.22, 0.025, w, [0.0, 0.77, -0.205], o().r([-0.08, 0.0, 0.0]));
    k.build()
}

fn stool(c: Col) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    k.cyl(0.18, 0.18, 0.035, w, [0.0, 0.445, 0.0], o().seg(20));
    for i in 0..4 {
        let a = (i as f32 / 4.0) * PI * 2.0 + PI / 4.0;
        k.rod([a.cos() * 0.1, 0.43, a.sin() * 0.1], [a.cos() * 0.17, 0.0, a.sin() * 0.17], 0.016, w, o());
    }
    k.build()
}

fn barstool(c: Col) -> Built {
    let mut k = Kit::new();
    let m = metal(&mut k);
    let f = fabric(&mut k, c);
    k.cyl(0.21, 0.19, 0.08, f, [0.0, 0.76, 0.0], o().seg(22));
    k.cyl(0.025, 0.025, 0.72, m, [0.0, 0.36, 0.0], o());
    k.torus(0.16, 0.012, m, [0.0, 0.3, 0.0], o().r([PI / 2.0, 0.0, 0.0]));
    k.cyl(0.2, 0.22, 0.02, m, [0.0, 0.01, 0.0], o().seg(22));
    k.build()
}

fn armchair(c: Col) -> Built {
    let mut k = Kit::new();
    let f = fabric(&mut k, c);
    let fd = k.mat("fabric2", shade(c, 0.82), mo().rough(0.95));
    k.rbox(0.85, 0.3, 0.8, 0.06, fd, [0.0, 0.25, 0.0], o());
    k.rbox(0.62, 0.15, 0.64, 0.06, f, [0.0, 0.47, 0.05], o());
    k.rbox(0.85, 0.5, 0.2, 0.08, f, [0.0, 0.62, -0.32], o().r([-0.12, 0.0, 0.0]));
    for s in [-1.0f32, 1.0] {
        k.rbox(0.13, 0.32, 0.78, 0.06, f, [s * 0.36, 0.55, 0.0], o());
    }
    let d = dark(&mut k);
    legs4(&mut k, 0.75, 0.7, 0.1, 0.02, d, 0.04, false);
    k.build()
}

/// `n` places, largeur `W`.
#[allow(non_snake_case)]
fn sofa(c: Col, n: f32, W: f32) -> Built {
    let mut k = Kit::new();
    let f = fabric(&mut k, c);
    let fd = k.mat("fabric2", shade(c, 0.82), mo().rough(0.95));
    const D: f32 = 0.9;
    k.rbox(W, 0.28, D, 0.05, fd, [0.0, 0.24, 0.0], o());
    let seat_w = (W - 0.36) / n;
    // i < n (comme la boucle JavaScript, même pour un nombre de places non entier)
    for i in 0..n.ceil() as u32 {
        let x = -W / 2.0 + 0.18 + seat_w * (i as f32 + 0.5);
        k.rbox(seat_w - 0.02, 0.14, 0.62, 0.06, f, [x, 0.45, 0.08], o());
        k.rbox(seat_w - 0.04, 0.42, 0.18, 0.07, f, [x, 0.68, -0.27], o().r([-0.15, 0.0, 0.0]));
    }
    k.rbox(W, 0.45, 0.18, 0.06, fd, [0.0, 0.6, -0.36], o());
    for s in [-1.0f32, 1.0] {
        k.rbox(0.18, 0.36, D, 0.07, f, [s * (W / 2.0 - 0.09), 0.56, 0.0], o());
    }
    let d = dark(&mut k);
    legs4(&mut k, W - 0.1, D - 0.1, 0.1, 0.02, d, 0.04, false);
    k.build()
}

/// Plateau `W` × `D` à la hauteur `H` (0,75 m pour une table courante).
#[allow(non_snake_case)]
fn table(c: Col, W: f32, D: f32, H: f32) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    k.boxed(W, 0.04, D, w, [0.0, H - 0.02, 0.0], o());
    let apron = k.mat("apron", shade(c, 0.85), mo().rough(0.7));
    k.boxed(W - 0.12, 0.08, D - 0.12, apron, [0.0, H - 0.08, 0.0], o());
    legs4(&mut k, W, D, H - 0.04, 0.025, w, 0.07, true);
    k.build()
}

fn roundtable(c: Col) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    k.cyl(0.5, 0.5, 0.04, w, [0.0, 0.73, 0.0], o().seg(32));
    k.cyl(0.05, 0.06, 0.69, w, [0.0, 0.36, 0.0], o().seg(14));
    for i in 0..4 {
        let a = (i as f32 / 4.0) * PI * 2.0;
        k.boxed(0.06, 0.04, 0.32, w, [a.sin() * 0.16, 0.02, a.cos() * 0.16], o().r([0.0, a, 0.0]));
    }
    k.build()
}

fn coffeetable(c: Col) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    k.rbox(1.1, 0.05, 0.6, 0.01, w, [0.0, 0.4, 0.0], o());
    k.boxed(1.0, 0.025, 0.5, w, [0.0, 0.12, 0.0], o());
    legs4(&mut k, 1.06, 0.56, 0.38, 0.022, w, 0.04, true);
    k.build()
}

fn desk(c: Col) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    let m = metal(&mut k);
    k.boxed(1.4, 0.03, 0.7, w, [0.0, 0.735, 0.0], o());
    k.boxed(0.42, 0.68, 0.62, w, [0.47, 0.37, 0.0], o()); // caisson
    for i in 0..3 {
        k.boxed(0.12, 0.015, 0.01, m, [0.47, 0.6 - i as f32 * 0.2, 0.315], o());
    }
    for sz in [-1.0f32, 1.0] {
        k.boxed(0.04, 0.72, 0.04, m, [-0.66, 0.36, sz * 0.31], o());
    }
    k.boxed(0.9, 0.35, 0.015, w, [-0.2, 0.52, -0.3], o()); // voile de fond
    k.build()
}

/// Lit de largeur de matelas `bw`.
#[allow(non_snake_case)]
fn bed(c: Col, bw: f32) -> Built {
    let mut k = Kit::new();
    let frame = wood(&mut k, "#6e4a2c");
    let sheet = k.mat("sheet", "#f1efe9", mo().rough(0.9));
    let duvet = fabric(&mut k, c);
    const L: f32 = 2.0;
    let W = bw + 0.08;
    k.boxed(W, 0.25, L + 0.05, frame, [0.0, 0.2, 0.0], o());
    k.rbox(bw, 0.22, L - 0.02, 0.06, sheet, [0.0, 0.42, 0.0], o());
    k.rbox(bw + 0.02, 0.12, L * 0.62, 0.05, duvet, [0.0, 0.55, 0.36], o());
    let n = if bw > 1.2 { 2 } else { 1 };
    for i in 0..n {
        let x = -bw / 2.0 + (bw / n as f32) * (i as f32 + 0.5);
        k.rbox(bw / n as f32 - 0.12, 0.12, 0.4, 0.05, sheet, [x, 0.6, -L / 2.0 + 0.3], o());
    }
    k.boxed(W, 0.95, 0.06, frame, [0.0, 0.47, -L / 2.0 - 0.02], o()); // tête de lit
    legs4(&mut k, W - 0.04, L, 0.08, 0.03, frame, 0.05, true);
    k.build()
}

fn nightstand(c: Col) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    k.boxed(0.45, 0.45, 0.4, w, [0.0, 0.32, 0.0], o());
    let d = dark(&mut k);
    k.boxed(0.41, 0.015, 0.01, d, [0.0, 0.42, 0.2], o());
    let m = metal(&mut k);
    k.cyl(0.012, 0.012, 0.02, m, [0.0, 0.36, 0.21], o().r([PI / 2.0, 0.0, 0.0]));
    legs4(&mut k, 0.43, 0.38, 0.1, 0.015, w, 0.02, true);
    k.build()
}

/// Meuble de rangement `W` × `H` × `D` : à tiroirs (`drawers` > 0) ou à deux portes.
#[allow(non_snake_case)]
fn cabinet(c: Col, W: f32, H: f32, D: f32, drawers: u32) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    let line = k.mat("line", shade(c, 0.6), mo().rough(0.7));
    let m = metal(&mut k);
    k.boxed(W, H - 0.06, D, w, [0.0, (H + 0.06) / 2.0, 0.0], o());
    k.boxed(W + 0.02, 0.025, D + 0.02, w, [0.0, H, 0.0], o());
    k.boxed(W - 0.04, 0.05, D - 0.04, line, [0.0, 0.03, 0.0], o()); // socle
    if drawers > 0 {
        let step = (H - 0.06) / drawers as f32;
        for i in 1..drawers {
            k.boxed(W - 0.04, 0.008, 0.005, line, [0.0, 0.06 + step * i as f32, D / 2.0 + 0.002], o());
        }
        for i in 0..drawers {
            k.boxed(0.14, 0.018, 0.02, m, [0.0, 0.06 + step * (i as f32 + 0.5), D / 2.0 + 0.012], o());
        }
    } else {
        k.boxed(0.006, H - 0.12, 0.005, line, [0.0, H / 2.0 + 0.03, D / 2.0 + 0.002], o());
        for s in [-1.0f32, 1.0] {
            k.boxed(0.018, 0.22, 0.02, m, [s * 0.05, H * 0.55, D / 2.0 + 0.012], o());
        }
    }
    k.build()
}

fn bookcase(c: Col) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    const W: f32 = 0.9;
    const H: f32 = 2.0;
    const D: f32 = 0.32;
    for s in [-1.0f32, 1.0] {
        k.boxed(0.025, H, D, w, [s * (W / 2.0 - 0.0125), H / 2.0, 0.0], o());
    }
    k.boxed(W, H - 0.02, 0.012, w, [0.0, H / 2.0, -D / 2.0 + 0.006], o());
    let shelves = 5;
    let mut rand = rng(11);
    let colors: Vec<M> = ["#7a2e2e", "#2e4a7a", "#3e6a3a", "#c9a24a", "#5a3a6a", "#d8d2c6", "#1f2a3a", "#8a5a2a"]
        .iter()
        .enumerate()
        .map(|(i, col)| k.mat(&format!("book{i}"), *col, mo().rough(0.8)))
        .collect();
    for i in 0..=shelves {
        let y = 0.04 + (i as f32 * (H - 0.08)) / shelves as f32;
        k.boxed(W - 0.05, 0.022, D - 0.02, w, [0.0, y, 0.005], o());
        if i == shelves {
            break;
        }
        // livres sur l'étagère
        let mut x = -W / 2.0 + 0.05;
        while x < W / 2.0 - 0.08 {
            let bw = 0.02 + rand() * 0.035;
            let bh = 0.2 + rand() * 0.12;
            if rand() < 0.08 {
                x += 0.06;
                continue;
            }
            // même ordre de tirage qu'en JavaScript : profondeur, puis couleur
            let bd = 0.18 + rand() * 0.06;
            let col = colors[((rand() * colors.len() as f32) as usize).min(colors.len() - 1)];
            k.boxed(bw, bh, bd, col, [x + bw / 2.0, y + 0.011 + bh / 2.0, 0.02], o());
            x += bw + 0.003;
        }
    }
    k.build()
}

fn counter(c: Col) -> Built {
    let mut k = Kit::new();
    let face = wood(&mut k, c);
    let top = k.mat("top", "#2b2b2e", mo().rough(0.3).metal(0.2));
    let m = metal(&mut k);
    k.boxed(2.4, 1.05, 0.55, face, [0.0, 0.525, 0.0], o());
    k.boxed(2.5, 0.05, 0.7, top, [0.0, 1.08, 0.04], o());
    k.rod([-1.2, 0.2, 0.36], [1.2, 0.2, 0.36], 0.02, m, o());
    k.build()
}

fn kitchen(c: Col) -> Built {
    let mut k = Kit::new();
    let face = k.mat("face", c, mo().rough(0.5));
    let top = k.mat("top", "#4a4642", mo().rough(0.4));
    let steel = metal(&mut k);
    let line = k.mat("line", shade(c, 0.7), mo().rough(0.6));
    const W: f32 = 2.4;
    k.boxed(W, 0.86, 0.6, face, [0.0, 0.47, 0.0], o());
    let plinth = k.mat("plinth", "#2a2a2c", mo().rough(0.8));
    k.boxed(W, 0.1, 0.55, plinth, [0.0, 0.05, -0.02], o());
    k.boxed(W + 0.02, 0.04, 0.64, top, [0.0, 0.92, 0.01], o());
    for i in 1..4 {
        k.boxed(0.006, 0.84, 0.004, line, [-W / 2.0 + (W / 4.0) * i as f32, 0.47, 0.301], o());
    }
    k.boxed(0.5, 0.02, 0.42, steel, [-0.5, 0.945, 0.02], o()); // évier
    k.rod([-0.5, 0.94, -0.2], [-0.5, 1.15, -0.12], 0.012, steel, o());
    let hob = k.mat("hob", "#111214", mo().rough(0.2));
    k.boxed(0.58, 0.012, 0.5, hob, [0.55, 0.945, 0.02], o()); // plaque
    k.boxed(W, 0.7, 0.35, face, [0.0, 1.85, -0.125], o()); // meubles hauts
    for i in 1..4 {
        k.boxed(0.006, 0.68, 0.004, line, [-W / 2.0 + (W / 4.0) * i as f32, 1.85, 0.051], o());
    }
    k.build()
}

fn fridge(c: Col) -> Built {
    let mut k = Kit::new();
    let body = k.mat("body", c, mo().rough(0.3).metal(0.3));
    let line = dark(&mut k);
    k.rbox(0.6, 1.78, 0.64, 0.03, body, [0.0, 0.9, 0.0], o());
    k.boxed(0.58, 0.008, 0.005, line, [0.0, 1.25, 0.322], o());
    for y in [0.85, 1.55] {
        let m = metal(&mut k);
        k.boxed(0.025, 0.3, 0.03, m, [0.25, y, 0.335], o());
    }
    k.build()
}

fn tv(c: Col) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    let screen = k.mat("screen", "#0a0b0d", mo().rough(0.15).metal(0.3));
    k.boxed(1.6, 0.45, 0.42, w, [0.0, 0.225, 0.0], o());
    k.boxed(1.25, 0.72, 0.04, screen, [0.0, 0.45 + 0.08 + 0.36, 0.0], o());
    let d = dark(&mut k);
    k.boxed(0.3, 0.08, 0.2, d, [0.0, 0.49, 0.0], o());
    k.build()
}

fn piano(c: Col) -> Built {
    let mut k = Kit::new();
    let lac = k.mat("laque", c, mo().rough(0.15).metal(0.1));
    let ivory = k.mat("ivory", "#f2efe6", mo().rough(0.4));
    let ebony = dark(&mut k);
    k.boxed(1.5, 1.25, 0.38, lac, [0.0, 0.625, -0.12], o());
    k.boxed(1.5, 0.12, 0.3, lac, [0.0, 0.68, 0.18], o()); // clavier
    k.boxed(1.36, 0.02, 0.15, ivory, [0.0, 0.75, 0.26], o());
    for i in 0..36 {
        if matches!(i % 12, 1 | 3 | 6 | 8 | 10) {
            k.boxed(0.02, 0.025, 0.09, ebony, [-0.66 + i as f32 * 0.038, 0.77, 0.23], o());
        }
    }
    for s in [-1.0f32, 1.0] {
        k.boxed(0.06, 0.62, 0.06, lac, [s * 0.68, 0.31, 0.26], o());
    }
    k.build()
}

fn floorlamp(c: Col) -> Built {
    let mut k = Kit::new();
    let m = dark(&mut k);
    k.cyl(0.16, 0.18, 0.03, m, [0.0, 0.015, 0.0], o().seg(24));
    k.cyl(0.012, 0.012, 1.4, m, [0.0, 0.72, 0.0], o());
    let lampshade = k.mat("shade", c, mo().rough(0.9).double());
    k.cyl(0.14, 0.22, 0.28, lampshade, [0.0, 1.5, 0.0], o().open().seg(24));
    k.build()
}

fn plant(c: Col) -> Built {
    let mut k = Kit::new();
    let pot = k.mat("pot", c, mo().rough(0.8));
    let leaf = k.mat("leaf", "#3f6b34", mo().rough(0.85).flat());
    let leaf2 = k.mat("leaf2", "#2f5527", mo().rough(0.85).flat());
    k.lathe(&[[0.001, 0.0], [0.16, 0.0], [0.22, 0.42], [0.2, 0.42], [0.001, 0.4]], pot, [0.0, 0.0, 0.0], o().seg(20));
    let stem = k.mat("stem", "#5a4a2a", mo());
    k.cyl(0.02, 0.025, 0.5, stem, [0.0, 0.65, 0.0], o());
    let mut rand = rng(5);
    for i in 0..14 {
        let a = rand() * PI * 2.0;
        let r = 0.08 + rand() * 0.2;
        // même ordre de tirage qu'en JavaScript : taille, puis hauteur
        let size = 0.12 + rand() * 0.1;
        let y = 0.75 + rand() * 0.55;
        k.ico(size, if i % 2 == 1 { leaf } else { leaf2 }, [a.cos() * r, y, a.sin() * r], o().s([1.0, 0.8, 1.0]));
    }
    k.build()
}

fn rug(c: Col) -> Built {
    let mut k = Kit::new();
    let rug = k.mat("rug", c, mo().rough(1.0));
    k.boxed(2.0, 0.012, 1.4, rug, [0.0, 0.006, 0.0], o());
    let rug2 = k.mat("rug2", shade(c, 1.35), mo().rough(1.0));
    k.boxed(1.7, 0.014, 1.1, rug2, [0.0, 0.007, 0.0], o());
    let rug3 = k.mat("rug3", c, mo().rough(1.0));
    k.boxed(1.5, 0.016, 0.9, rug3, [0.0, 0.008, 0.0], o());
    k.build()
}

fn coatrack(c: Col) -> Built {
    let mut k = Kit::new();
    let w = wood(&mut k, c);
    k.cyl(0.025, 0.03, 1.75, w, [0.0, 0.875, 0.0], o());
    for i in 0..4 {
        let a = (i as f32 / 4.0) * PI * 2.0;
        k.rod([0.0, 0.12, 0.0], [a.cos() * 0.25, 0.0, a.sin() * 0.25], 0.018, w, o());
        k.rod([0.0, 1.62, 0.0], [(a + 0.6).cos() * 0.14, 1.7, (a + 0.6).sin() * 0.14], 0.012, w, o());
    }
    let coat = fabric(&mut k, "#3a3f4a");
    k.capsule([0.09, 1.55, 0.02], [0.12, 1.05, 0.06], 0.07, coat, o().s([1.4, 1.0, 0.6])); // manteau
    k.build()
}

fn divider(c: Col) -> Built {
    let mut k = Kit::new();
    let frame = wood(&mut k, "#4a3424");
    let cloth = k.mat("cloth", c, mo().rough(0.95).double());
    let pw: f32 = 0.5;
    let angles: [f32; 3] = [0.35, 0.0, -0.35];
    for (i, &angle) in angles.iter().enumerate() {
        let cx = -0.5 + i as f32 * 0.5;
        let z = if i == 1 { -0.08 } else { 0.05 };
        k.boxed(pw - 0.04, 1.6, 0.01, cloth, [cx, 0.92, z], o().r([0.0, angle, 0.0]));
        for s in [-1.0f32, 1.0] {
            let p = [cx + s * (pw / 2.0 - 0.015) * angle.cos(), 0.875, z - s * (pw / 2.0 - 0.015) * angle.sin()];
            k.boxed(0.03, 1.75, 0.03, frame, p, o());
        }
    }
    k.build()
}

/// Modèle 3D d'un meuble du catalogue.
pub fn build_furniture(t: &PropType, p: Props) -> Option<Built> {
    let c = p.col("color", "#8a6a4a");
    // `type.seats || 2` : absent ou nul → valeur par défaut
    let num = |key: &str| t.extra.get(key).and_then(Value::as_f64).map(|v| v as f32).filter(|v| *v != 0.0);
    Some(match t.model.as_deref() {
        Some("chair") => chair(c),
        Some("stool") => stool(c),
        Some("barstool") => barstool(c),
        Some("armchair") => armchair(c),
        Some("sofa") => sofa(c, num("seats").unwrap_or(2.0), t.size[0] as f32),
        Some("table") => table(c, 1.6, 0.9, 0.75),
        Some("roundtable") => roundtable(c),
        Some("coffeetable") => coffeetable(c),
        Some("desk") => desk(c),
        Some("bed") => bed(c, num("bedW").unwrap_or(0.9)),
        Some("nightstand") => nightstand(c),
        Some("wardrobe") => cabinet(c, 1.2, 2.0, 0.6, 0),
        Some("dresser") => cabinet(c, 1.0, 0.85, 0.48, 4),
        Some("bookcase") => bookcase(c),
        Some("counter") => counter(c),
        Some("kitchen") => kitchen(c),
        Some("fridge") => fridge(c),
        Some("tv") => tv(c),
        Some("piano") => piano(c),
        Some("floorlamp") => floorlamp(c),
        Some("plant") => plant(c),
        Some("rug") => rug(c),
        Some("coatrack") => coatrack(c),
        Some("divider") => divider(c),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque meuble du catalogue a un modèle : des pièces non vides, des sommets finis.
    #[test]
    fn tous_les_meubles_du_catalogue() {
        let empty = Value::Object(Default::default());
        let mut count = 0;
        for t in arklight_core::library().props.iter().filter(|t| t.category.as_deref() == Some("furniture")) {
            let props = t.extra.get("defaults").unwrap_or(&empty);
            let built = build_furniture(t, Props(props)).unwrap_or_else(|| panic!("{} : pas de modèle", t.id));
            assert!(!built.parts.is_empty(), "{} : aucune pièce", t.id);
            for (mesh, _) in &built.parts {
                assert!(!mesh.vertices.is_empty() && !mesh.indices.is_empty(), "{} : maillage vide", t.id);
                assert!(mesh.vertices.iter().all(|v| v.pos.iter().all(|x| x.is_finite())), "{} : sommet non fini", t.id);
                assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.vertices.len()), "{} : indice hors maillage", t.id);
            }
            count += 1;
        }
        assert!(count > 0, "aucun meuble dans le catalogue");
    }

    /// Modèle inconnu : rien (l'appelant pose un simple volume).
    #[test]
    fn modele_inconnu() {
        let t: PropType = serde_json::from_value(serde_json::json!({ "id": "x", "name": "x", "model": "inconnu", "size": [1, 1, 1] })).unwrap();
        assert!(build_furniture(&t, Props(&Value::Null)).is_none());
    }
}
