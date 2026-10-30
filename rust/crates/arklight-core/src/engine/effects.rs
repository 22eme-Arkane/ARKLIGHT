//! Programmes d'effets Astera (mode « EFFECT MODE » en DMX et programmes STANDALONE du menu),
//! portage de `shared/engine/effects.js`. Liste et ordre des programmes : documents « DMX
//! Profiles » Astera (canal Program). Le rendu exact des programmes n'est pas documenté : ce
//! moteur en donne une interprétation visuelle.

use crate::color::Rgb;
use std::f64::consts::PI;

pub const PROGRAMS: [&str; 20] = [
    "One Color Static",
    "Two Color Static",
    "Three Color Static",
    "Four Color Static",
    "One Color Fade",
    "Two Color Fade",
    "Three Color Fade",
    "Four Color Fade",
    "Simple Running",
    "Double Running",
    "Two Col Running",
    "Flag Running",
    "Double Flag Running",
    "Spiral 4 Color",
    "Spiral 2 Color",
    "Rainbow",
    "Fire",
    "Rotor",
    "Rotor Split 2",
    "Rotor Split 4",
];

/// Plages DMX du canal Program (début de chaque plage).
const PROGRAM_STARTS: [f64; 20] = [0.0, 8.0, 16.0, 24.0, 32.0, 40.0, 48.0, 56.0, 64.0, 72.0, 80.0, 88.0, 96.0, 102.0, 110.0, 118.0, 126.0, 134.0, 142.0, 150.0];

pub fn program_from_dmx(v: f64) -> usize {
    PROGRAM_STARTS.iter().rposition(|s| v >= *s).unwrap_or(0)
}

/// Vitesse 0..1 (lent → rapide) → durée d'un cycle en secondes (10 s .. 0,09 s, échelle log).
pub fn cycle_seconds(speed: f64) -> f64 {
    10.0 * (0.09f64 / 10.0).powf(speed.clamp(0.0, 1.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Direction {
    pub reverse: bool,
    pub once: bool,
}

/// Canal Direction (valeur DMX) → sens et lecture unique.
pub fn direction_from_dmx(v: f64) -> Direction {
    let (reverse, once) = if v < 64.0 {
        (false, false)
    } else if v < 128.0 {
        (false, true)
    } else if v < 191.0 {
        (true, true)
    } else {
        (true, false)
    };
    Direction { reverse, once }
}

fn mix(a: Rgb, b: Rgb, k: f64) -> Rgb {
    [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k]
}

const BLACK: Rgb = [0.0; 3];

fn hash(n: f64) -> f64 {
    let x = (n * 127.1 + 311.7).sin() * 43758.5453;
    x - x.floor()
}

fn hue_rgb(h: f64) -> Rgb {
    let k = |n: f64| (n + h * 6.0) % 6.0;
    let f = |n: f64| 1.0 - k(n).min(4.0 - k(n)).min(1.0).max(0.0);
    [f(5.0), f(3.0), f(1.0)]
}

/// Un programme à dessiner.
pub struct Program<'a> {
    /// indice dans [`PROGRAMS`]
    pub program: usize,
    /// jusqu'à 4 couleurs (sRGB linéaire)
    pub colors: &'a [Rgb],
    pub pixels: usize,
    /// secondes depuis le (re)départ du programme
    pub time: f64,
    /// 0..1
    pub speed: f64,
    /// 0 (pas à pas) .. 1 (fondu)
    pub fade: f64,
    pub reverse: bool,
    pub once: bool,
    /// taille virtuelle (1..4 projecteurs)
    pub size: f64,
    /// 0..1 : position dans le programme virtuel
    pub offset: f64,
}

impl Default for Program<'_> {
    fn default() -> Self {
        Self { program: 0, colors: &[], pixels: 1, time: 0.0, speed: 0.5, fade: 0.5, reverse: false, once: false, size: 1.0, offset: 0.0 }
    }
}

/// Couleurs des pixels pour un programme : `out` reçoit `pixels` × 3 valeurs.
pub fn render_program(p: &Program, out: &mut [f64]) {
    let first = p.colors.first().copied();
    let cols: [Rgb; 4] = std::array::from_fn(|i| p.colors.get(i).copied().or(first).unwrap_or(BLACK));
    let fade = p.fade;
    let mut phase = p.time / cycle_seconds(p.speed);
    if p.once {
        phase = phase.min(0.999);
    }
    if p.reverse {
        phase = -phase;
    }
    let size = p.size.max(1.0);
    let pixels = p.pixels as f64;
    let total = pixels * size;
    let shift = super::js_round(p.offset * (size - 1.0)) * pixels;
    // avance par paliers avec un fondu réglable
    let step = |x: f64| {
        let i = x.floor();
        let f = x - i;
        let k = if fade <= 0.0 { 0.0 } else { ((f - (1.0 - fade)) / fade.max(1e-3)).max(0.0).min(1.0) };
        (i as usize, k)
    };
    for p_i in 0..p.pixels {
        let pf = p_i as f64;
        let u = (pf + shift) / total; // position 0..1 dans le programme virtuel
        let c = match p.program {
            0..=3 => {
                let n = (p.program + 1) as f64;
                cols[(n - 1.0).min((u * n).floor()) as usize]
            }
            4 => {
                let k = 0.5 - 0.5 * (phase * 2.0 * PI).cos();
                mix(BLACK, cols[0], if fade > 0.0 { k } else if k > 0.5 { 1.0 } else { 0.0 })
            }
            5..=7 => {
                let n = (p.program - 3) as f64;
                let (i, k) = step((((phase * n) % n) + n) % n);
                let n = n as usize;
                mix(cols[i % n], cols[(i + 1) % n], k)
            }
            8 | 9 => {
                let head = (((phase % 1.0) + 1.0) % 1.0) * total;
                let d = (pf + shift - head).abs();
                let dots = if p.program == 9 { d.min((total - 1.0 - (pf + shift) - head).abs()) } else { d };
                let width = 1.0 + fade * 2.0;
                mix(BLACK, cols[0], (1.0 - dots / width).max(0.0))
            }
            10 => {
                let x = u * 4.0 - phase * 2.0;
                let (i, k) = step(((x % 2.0) + 2.0) % 2.0);
                mix(cols[i % 2], cols[(i + 1) % 2], k)
            }
            11 | 12 => {
                let x = if p.program == 12 { (u - 0.5).abs() * 2.0 - phase } else { u - phase };
                let (i, k) = step((((x * 4.0) % 4.0) + 4.0) % 4.0);
                mix(cols[i % 4], cols[(i + 1) % 4], k)
            }
            13 | 14 => {
                let n = if p.program == 13 { 4.0 } else { 2.0 };
                let x = pf + shift + (phase * total).floor();
                cols[(((x % n) + n) % n) as usize]
            }
            15 => hue_rgb((((u - phase) % 1.0) + 1.0) % 1.0),
            16 => {
                let t = (p.time * (6.0 + p.speed * 20.0)).floor();
                let r = hash(pf * 13.7 + t);
                let g = 0.45 + 0.55 * hash(pf + t * 3.1);
                mix([1.0, 0.18, 0.0], [1.0, 0.55, 0.05], r).map(|v| v * g)
            }
            _ => {
                let n = match p.program {
                    18 => 2.0,
                    19 => 4.0,
                    _ => 1.0,
                };
                let w = 0.5 + 0.5 * (2.0 * PI * (u * n - phase)).cos();
                mix(cols[1], cols[0], w)
            }
        };
        out[p_i * 3..p_i * 3 + 3].copy_from_slice(&c);
    }
}
