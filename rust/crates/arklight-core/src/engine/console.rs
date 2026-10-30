//! Console ARKLIGHT (portage de `shared/engine/console.js`) : la console intégrée pilote les
//! projecteurs canal par canal, comme une console branchée sur leur entrée DMX, mais sans passer
//! par un univers : seuls les projecteurs qu'elle a pris en main changent. Ses valeurs sont rangées
//! dans l'état du projecteur (`state.console = { mode, values }`) : elles s'enregistrent avec le
//! projet et passent avant tout le reste (réglages, DMX reçu). « Relâcher » les efface.
//!
//! Quand la console prend un projecteur, elle part de ce qu'il fait déjà (réglage manuel, mode
//! autonome Astera, DMX reçu) : rien ne saute à l'image.

use super::dmx::{CompiledMode, Curve, Decode, Func, Key, MapKind, Slot, StrobeDef, decode_dmx_output, default_range, raw_value, read_attributes};
use super::gels::{astera_index_colors, color_spec_rgb};
use super::{Runtime, Universe, Universes, field, js_round, num, num_or, num_or_falsy, rgb_of, text, truthy};
use crate::color::{Rgb, cct_to_rgb, clamp01, hs_to_rgb};
use crate::library::{DmxMode, FixtureType};
use crate::project::Fixture;
use serde_json::Value;

/// Valeurs brutes des canaux d'un mode (0..255, ou 0..65535 en 16 bits).
pub type Values = Vec<u16>;

/// Valeurs de départ d'une console vide : plein feu, teinte neutre, température au milieu.
pub fn default_faders(mode: &DmxMode) -> Values {
    let c = mode.compiled();
    let has_cct = c.has(Func::Cct);
    c.slots
        .iter()
        .map(|slot| {
            let max = slot.max() as f64;
            let v = match slot.func() {
                Func::Dimmer => max,
                Func::Cct | Func::Tint => js_round(slot.def.neutral.map_or(max / 2.0, |n| n * max)),
                Func::Red | Func::Green | Func::Blue | Func::White if !has_cct => max,
                _ => 0.0,
            };
            v as u16
        })
        .collect()
}

/// Valeurs brutes par canal → trame DMX virtuelle du mode (adresse 1) ; sans valeurs (ou valeurs
/// d'un autre mode) : valeurs de départ.
pub fn fader_buffer(mode: &DmxMode, values: Option<&[u16]>) -> Universe {
    let mut buf = [0u8; 512];
    let defaults;
    let values = match values {
        Some(v) => v,
        None => {
            defaults = default_faders(mode);
            &defaults
        }
    };
    for (i, slot) in mode.compiled().slots.iter().enumerate() {
        if slot.offset >= 512 {
            continue;
        }
        let v = values.get(i).copied().unwrap_or(0).min(slot.max());
        if slot.bytes() == 2 {
            buf[slot.offset] = (v >> 8) as u8;
            if slot.offset + 1 < 512 {
                buf[slot.offset + 1] = (v & 255) as u8;
            }
        } else {
            buf[slot.offset] = v as u8;
        }
    }
    buf
}

/// Valeur JSON d'un canal → valeur brute (`Math.round(Number(v) || 0)`, bornée).
fn raw_of(v: &Value) -> u16 {
    let x = v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0.0);
    if x.is_nan() { 0 } else { js_round(x).clamp(0.0, 65535.0) as u16 }
}

/// Valeurs d'un réglage `{ mode, values }` si elles sont pour `mode`.
pub fn values_for(faders: Option<&Value>, mode: &DmxMode) -> Option<Values> {
    let f = faders?;
    if text(field(Some(f), "mode")) != Some(mode.id.as_str()) {
        return None;
    }
    Some(f.get("values")?.as_array()?.iter().map(raw_of).collect())
}

/// Blanc et courbe de gradation du projecteur (conversions de valeurs).
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub white_k: f64,
    pub curve: Curve,
}

impl Default for Options {
    fn default() -> Self {
        Self { white_k: 5600.0, curve: Curve::Linear }
    }
}

fn setting<'a>(f: &'a Fixture, key: &str) -> Option<&'a Value> {
    f.settings.extra.get(key).filter(|v| !v.is_null())
}

pub fn console_options(f: &Fixture, ty: &FixtureType) -> Options {
    let curve = if ty.is_astera_v5() { Curve::named(text(setting(f, "dimmerCurve")), Curve::Standard) } else { Curve::named(text(setting(f, "dimmingCurve")), Curve::Linear) };
    Options { white_k: num_or_falsy(setting(f, "whitePoint"), 5600.0), curve }
}

/// Le projecteur est-il piloté par la console ?
pub fn console_held(f: &Fixture) -> bool {
    field(Some(&f.state), "console").and_then(|c| c.get("values")).is_some_and(Value::is_array)
}

/// Valeurs de la console suivies après un changement de profil (calculées une fois).
#[derive(Debug, Clone)]
pub(crate) struct Remapped {
    from_mode: Option<String>,
    from: Values,
    to: String,
    values: Values,
}

/// Valeurs de la console pour le mode actuel du projecteur, ou None s'il n'est pas piloté par elle.
/// Profil changé depuis : les valeurs suivent (fonction par fonction).
pub fn active_console_values(f: &Fixture, ty: &FixtureType, mode: Option<&DmxMode>, rt: &mut Runtime) -> Option<Values> {
    let mode = mode?;
    let con = field(Some(&f.state), "console")?;
    let values: Values = con.get("values")?.as_array()?.iter().map(raw_of).collect();
    let con_mode = text(field(Some(con), "mode"));
    if con_mode == Some(mode.id.as_str()) {
        return Some(values);
    }
    let cached = rt.console.as_ref().is_some_and(|r| r.from == values && r.from_mode.as_deref() == con_mode && r.to == mode.id);
    if !cached {
        let remapped = match ty.mode(con_mode) {
            Some(from) => remap_values(from, mode, &values, ty.pixels, console_options(f, ty)),
            None => default_faders(mode),
        };
        rt.console = Some(Remapped { from_mode: con_mode.map(str::to_owned), from: values, to: mode.id.clone(), values: remapped });
    }
    rt.console.as_ref().map(|r| r.values.clone())
}

// ------------------------------------------------------------------ valeur d'une fonction → canal
fn inverse_map(map: MapKind, v: f64) -> f64 {
    let clamp = |x: f64, lo: f64, hi: f64| x.clamp(lo, hi);
    match map {
        MapKind::AsteraStrobe => {
            if v == 0.0 || v.is_nan() {
                0.0
            } else if v == -1.0 {
                4.0
            } else if v == -2.0 {
                5.0
            } else if v == -3.0 {
                6.0
            } else {
                clamp(js_round(7.0 + ((v - 0.4) / 24.6) * 248.0), 7.0, 255.0)
            }
        }
        MapKind::AsteraCct => clamp(js_round((v - 1750.0) / 32.0), 0.0, 255.0),
        MapKind::AsteraCctInd => clamp(js_round((v - 2000.0) / 20.0), 5.0, 255.0),
        MapKind::AsteraGm => {
            if v == 0.0 || v.is_nan() {
                0.0
            } else {
                clamp(js_round((v + 1.0) * 128.0), 5.0, 255.0)
            }
        }
        MapKind::AsteraIdx => clamp(js_round(v), 2.0, 255.0),
        MapKind::AsteraFan => [0.0, 20.0, 50.0, 90.0, 120.0].get(v as usize).copied().filter(|_| v >= 0.0 && v.fract() == 0.0).unwrap_or(0.0),
        MapKind::Raw => {
            if v.is_nan() {
                0.0
            } else {
                clamp(js_round(v), 0.0, 255.0)
            }
        }
    }
}

/// Stroboscope générique : fréquence (Hz, 0 = ouvert) → canal.
fn encode_strobe(slot: &Slot, hz: f64) -> u16 {
    if hz.is_nan() || hz <= 0.0 {
        return 0;
    }
    let s: StrobeDef = slot.def.strobe.unwrap_or_default();
    let span = if s.max - s.min == 0.0 { 1.0 } else { s.max - s.min };
    let v = s.open + 1.0 + ((hz.clamp(s.min, s.max) - s.min) / span) * (255.0 - s.open - 1.0);
    js_round(v.clamp(0.0, 255.0) * (slot.max() as f64 / 255.0)) as u16
}

/// Valeur brute d'un canal pour une valeur de sa fonction (inverse du décodage DMX).
pub fn encode_slot(slot: &Slot, value: f64) -> u16 {
    let max = slot.max() as f64;
    let def = &slot.def;
    if let Some(map) = def.map {
        let v8 = inverse_map(map, value);
        return if def.bytes == 2 { js_round(v8 * 257.0) as u16 } else { v8 as u16 };
    }
    if !value.is_finite() {
        return 0;
    }
    if def.func == Func::Strobe {
        return encode_strobe(slot, value);
    }
    let range = def.range.or(default_range(def.func));
    let n = if let Some(neutral) = def.neutral {
        // zone neutre en bas de course : la valeur « neutre » y reste, les autres au-dessus du seuil
        if (value - def.neutral_value.unwrap_or(0.0)).abs() < 1e-6 {
            0.0
        } else {
            let r = range.unwrap_or([0.0, 1.0]);
            let k = clamp01((value - r[0]) / (r[1] - r[0]));
            (neutral + 1.0 / 255.0).max(neutral + k * (1.0 - neutral))
        }
    } else {
        match range {
            Some(r) => (value - r[0]) / (r[1] - r[0]),
            None => value,
        }
    };
    js_round(clamp01(n) * max) as u16
}

/// Valeurs brutes des canaux d'un projecteur dans une trame DMX reçue.
pub fn read_raw_values(mode: &DmxMode, data: &[u8], address: i64) -> Values {
    let start = address - 1;
    mode.compiled().slots.iter().map(|slot| if start + slot.offset as i64 >= 512 { 0 } else { raw_value(data, start, slot) as u16 }).collect()
}

/// Nombre de pixels pilotés par un mode sur un projecteur à `pixel_count` pixels.
pub fn mode_pixels(mode: &DmxMode, pixel_count: usize) -> usize {
    mode.pixels_on(pixel_count)
}

/// Recopie fonction par fonction (même façon de faire la couleur dans les deux modes).
fn copy_attributes(from: &DmxMode, to: &DmxMode, values: &[u16], pixel_count: usize) -> Values {
    let buf = fader_buffer(from, Some(values));
    let from_px = mode_pixels(from, pixel_count);
    let to_px = mode_pixels(to, pixel_count);
    let (fa, pixels) = read_attributes(from, &buf, 1, from_px);
    let target = &to.compiled().slots;
    let global_dimmer = target.iter().any(|s| s.px < 0 && s.key() == Key::of(Func::Dimmer));
    let defaults = default_faders(to);
    target
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            if !slot.active() {
                return defaults[i];
            }
            let key = slot.key();
            let v = if slot.px >= 0 {
                let p = (from_px - 1).min(slot.px as usize * from_px / to_px);
                let v = pixels.get(p).and_then(|a| a.key(key));
                // gradateur par pixel quand le nouveau mode a aussi un gradateur général : pixel au maximum
                if v.is_none() && slot.func() == Func::Dimmer && global_dimmer {
                    return slot.max();
                }
                v.or(fa.key(key))
            } else {
                fa.key(key).or(pixels.first().and_then(|a| a.key(key)))
            };
            v.map_or(defaults[i], |v| encode_slot(slot, v))
        })
        .collect()
}

// ------------------------------------------------------------------ réglage actuel → valeurs
/// Niveaux d'émetteurs (0..1) dont le mélange donne `target` (moindres carrés positifs bornés).
pub fn solve_emitters(target: Rgb, vectors: &[Rgb]) -> Vec<f64> {
    let n = vectors.len();
    let mut e = vec![0.0; n];
    let mut l = 0.0;
    for v in vectors {
        l += v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    }
    if l == 0.0 {
        return e;
    }
    let step = 1.0 / l;
    for _ in 0..2000 {
        let mut r = [-target[0], -target[1], -target[2]];
        for i in 0..n {
            r[0] += e[i] * vectors[i][0];
            r[1] += e[i] * vectors[i][1];
            r[2] += e[i] * vectors[i][2];
        }
        let mut moved = 0.0;
        for i in 0..n {
            let g = r[0] * vectors[i][0] + r[1] * vectors[i][1] + r[2] * vectors[i][2];
            let next = clamp01(e[i] - step * g);
            moved += (next - e[i]).abs();
            e[i] = next;
        }
        if moved < 1e-7 {
            break;
        }
    }
    e
}

/// Valeur du gradateur qui donne le niveau `level` (0..1) à travers la courbe de gradation.
fn invert_curve(curve: Curve, level: f64) -> f64 {
    if level <= 0.0 {
        return 0.0;
    }
    if level >= 1.0 {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..30 {
        let mid = (lo + hi) / 2.0;
        if curve.apply(mid) < level {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// Coordonnées xy (CIE 1931) d'une couleur sRGB linéaire.
fn rgb_to_xy([r, g, b]: Rgb) -> [f64; 2] {
    let x = 0.4124 * r + 0.3576 * g + 0.1805 * b;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = 0.0193 * r + 0.1192 * g + 0.9505 * b;
    let s = x + y + z;
    if s > 0.0 { [x / s, y / s] } else { [0.3127, 0.329] }
}

fn dist2(a: Rgb, b: Rgb) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

fn max3(c: Rgb) -> f64 {
    c[0].max(c[1]).max(c[2])
}

/// `Math.max(...c) || 1`
fn max_or_one(c: Rgb) -> f64 {
    let m = max3(c);
    if m == 0.0 || m.is_nan() { 1.0 } else { m }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HueSat {
    pub hue: f64,
    pub sat: f64,
    d: f64,
}

/// Teinte / saturation dont la couleur (mélangée au blanc `white`) approche le mieux `target`.
fn fit_hue_sat(target: Rgb, white: Rgb) -> HueSat {
    fn scan(best: &mut HueSat, target: Rgb, white: Rgb, (h0, h1, hs): (f64, f64, f64), (s0, s1, ss): (f64, f64, f64)) {
        let mut h = h0;
        while h <= h1 {
            let mut sat = s0.max(0.0);
            while sat <= s1.min(1.0) + 1e-9 {
                let d = dist2(hs_to_rgb(h, sat, white), target);
                if d < best.d {
                    *best = HueSat { hue: (h + 360.0) % 360.0, sat, d };
                }
                sat += ss;
            }
            h += hs;
        }
    }
    let mut best = HueSat { hue: 0.0, sat: 0.0, d: dist2(hs_to_rgb(0.0, 0.0, white), target) };
    scan(&mut best, target, white, (0.0, 354.0, 6.0), (0.0, 1.0, 0.05));
    let HueSat { hue, sat, .. } = best;
    scan(&mut best, target, white, (hue - 6.0, hue + 6.0, 0.5), (sat - 0.05, sat + 0.05, 0.005));
    best
}

/// Couleur index (gélatine) la plus proche de `target` (modes d'effet FIX des Astera).
fn nearest_index(target: Rgb) -> u16 {
    let ic = astera_index_colors();
    let mut best = (0u16, f64::INFINITY);
    for v in 2..256u16 {
        let Some(c) = ic.color(v as f64) else { continue };
        let m = max_or_one(c);
        let d = dist2(c.map(|x| x / m), target);
        if d < best.1 {
            best = (v, d);
        }
    }
    best.0
}

/// Réglage de lumière : niveau de sortie (0..1) et couleur — blanc (Kelvin et teinte ±vert),
/// teinte et saturation (mélangées au blanc `cct`), ou couleur RVB.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Look {
    pub level: Option<f64>,
    pub cct: Option<f64>,
    pub tint: Option<f64>,
    pub hue: Option<f64>,
    pub sat: Option<f64>,
    pub rgb: Option<Rgb>,
}

struct Prepared {
    level: f64,
    cct: f64,
    tint: f64,
    target: Rgb,
    hs: (f64, f64),
    xy: [f64; 2],
    white_by_cct: bool,
}

/// Réglage → ce qu'il faut demander au mode (couleur visée, teinte, xy…).
fn prepare_look(look: &Look, c: &CompiledMode, white_k: f64, white_rgb: Rgb) -> Prepared {
    let has = |f| c.has(f);
    let level = clamp01(look.level.unwrap_or(1.0));
    let is_color = look.rgb.is_some() || look.sat.is_some_and(|s| s > 0.0);
    let cct = look.cct.unwrap_or(5600.0);
    let tint = look.tint.unwrap_or(0.0);
    let white = cct_to_rgb(cct, tint);
    let target = match look.rgb {
        Some(rgb) => {
            let m = max_or_one(rgb);
            rgb.map(|c| c.max(0.0) / m)
        }
        None => match look.sat {
            Some(sat) if sat > 0.0 => hs_to_rgb(look.hue.unwrap_or(0.0), sat, white),
            _ => white,
        },
    };
    // teinte / saturation : celles du réglage quand le mode mélange au même blanc, sinon les plus proches
    let no_tint = tint == 0.0 || tint.is_nan();
    let exact_white = if has(Func::Cct) { no_tint || has(Func::Tint) } else { (cct - white_k).abs() < 1.0 && no_tint };
    let mut hs = (0.0, 0.0);
    if has(Func::Hue) {
        if let (Some(hue), true) = (look.hue, exact_white) {
            hs = (hue, look.sat.unwrap_or(0.0));
        } else if is_color || !exact_white {
            let w = if has(Func::Cct) { cct_to_rgb(cct, if has(Func::Tint) { tint } else { 0.0 }) } else { white_rgb };
            let f = fit_hue_sat(target, w);
            hs = (f.hue, f.sat);
        }
    }
    // le blanc passe par le canal Kelvin quand le mode a un fondu Kelvin ↔ couleur
    Prepared { level, cct, tint, target, hs, xy: rgb_to_xy(target), white_by_cct: !is_color && has(Func::Cct) && has(Func::Crossfade) }
}

/// Valeurs d'un mode qui reproduisent un réglage (un seul, ou un par pixel du mode).
pub fn look_to_values(mode: &DmxMode, looks: &[Look], o: Options) -> Values {
    let c = mode.compiled();
    let slots = &c.slots;
    let mut values = vec![0u16; slots.len()];
    let global_dimmer = slots.iter().any(|s| s.func() == Func::Dimmer && s.px < 0);
    let pixel_dimmer = slots.iter().any(|s| s.func() == Func::Dimmer && s.px >= 0);
    let white_rgb = cct_to_rgb(o.white_k, 0.0); // blanc du projecteur (canal W, modes sans Kelvin)
    let single = [Look::default()];
    let looks = if looks.is_empty() { &single[..] } else { looks };
    let prepared: Vec<Prepared> = looks.iter().map(|l| prepare_look(l, c, o.white_k, white_rgb)).collect();
    let at = |px: i32| &prepared[if px >= 0 { (prepared.len() - 1).min(px as usize) } else { 0 }];
    let top = prepared.iter().map(|l| l.level).fold(0.0, f64::max); // le gradateur général donne le plus fort
    // part du niveau portée par les couleurs (le reste par les gradateurs)
    let color_scale = |l: &Prepared, per_pixel: bool| {
        if per_pixel && pixel_dimmer {
            1.0
        } else if global_dimmer {
            if top > 0.0 { l.level / top } else { 0.0 }
        } else {
            l.level
        }
    };

    for (i, slot) in slots.iter().enumerate() {
        let l = at(slot.px);
        values[i] = match slot.func() {
            Func::Dimmer if slot.px < 0 => encode_slot(slot, invert_curve(o.curve, top)),
            Func::Dimmer => encode_slot(slot, invert_curve(o.curve, if global_dimmer { if top > 0.0 { l.level / top } else { 0.0 } } else { l.level })),
            Func::Cct => encode_slot(slot, l.cct),
            Func::Tint => encode_slot(slot, l.tint),
            Func::Crossfade => encode_slot(slot, if l.white_by_cct { 0.0 } else { 1.0 }),
            Func::Hue => encode_slot(slot, l.hs.0),
            Func::Sat => encode_slot(slot, l.hs.1),
            Func::X => encode_slot(slot, l.xy[0]),
            Func::Y => encode_slot(slot, l.xy[1]),
            // modes d'effet : programme 0 (« One Color Static ») dans la couleur 1
            f @ (Func::FxR | Func::FxG | Func::FxB) if slot.def.slot == 1 => {
                let ch = match f {
                    Func::FxR => 0,
                    Func::FxG => 1,
                    _ => 2,
                };
                encode_slot(slot, l.target[ch] * color_scale(l, false))
            }
            Func::FxIndex if slot.def.slot == 1 => nearest_index(l.target) * if slot.bytes() == 2 { 257 } else { 1 },
            _ => 0, // stroboscope ouvert, programme 0, pas de couleur index…
        };
    }

    // émetteurs (rouge, vert, bleu, blanc, ambre…) : mélange qui donne la couleur, pixel par pixel
    let mut groups: Vec<(i32, Vec<usize>)> = Vec::new();
    for (i, slot) in slots.iter().enumerate() {
        if !slot.func().is_emitter() {
            continue;
        }
        match groups.iter_mut().find(|(px, _)| *px == slot.px) {
            Some((_, list)) => list.push(i),
            None => groups.push((slot.px, vec![i])),
        }
    }
    for (px, list) in groups {
        let l = at(px);
        if l.white_by_cct {
            continue;
        }
        let vectors: Vec<Rgb> = list.iter().map(|&i| slots[i].func().emitter_rgb(white_rgb)).collect();
        let k = color_scale(l, px >= 0);
        let e = solve_emitters(l.target.map(|c| c * k), &vectors);
        for (j, &i) in list.iter().enumerate() {
            values[i] = encode_slot(&slots[i], e[j]);
        }
    }
    values
}

#[derive(Debug, Clone, Copy)]
struct White {
    cct: f64,
    tint: f64,
    d: f64,
}

/// Blanc (Kelvin + teinte ±vert) le plus proche d'une couleur normalisée.
fn fit_white(target: Rgb) -> White {
    fn scan(best: &mut White, target: Rgb, (k0, k1, k_step): (f64, f64, f64), (t0, t1, t_step): (f64, f64, f64)) {
        let mut k = k0;
        while k <= k1 {
            let mut t = t0.max(-1.0);
            while t <= t1.min(1.0) + 1e-9 {
                let d = dist2(cct_to_rgb(k, t), target);
                if d < best.d {
                    *best = White { cct: k, tint: t, d };
                }
                t += t_step;
            }
            k *= k_step;
        }
    }
    let mut best = White { cct: 5600.0, tint: 0.0, d: f64::INFINITY };
    scan(&mut best, target, (1750.0, 20000.0, 1.05), (-1.0, 1.0, 0.1));
    let White { cct, tint, .. } = best;
    scan(&mut best, target, (cct / 1.05, cct * 1.05, 1.004), (tint - 0.1, tint + 0.1, 0.01));
    best
}

/// Fonctions qui font la couleur et l'intensité.
fn is_color_function(f: Func) -> bool {
    f.is_emitter()
        || matches!(
            f,
            Func::Dimmer | Func::Cct | Func::Tint | Func::Crossfade | Func::Hue | Func::Sat | Func::X | Func::Y | Func::Index | Func::CctIndex | Func::FxProgram | Func::FxIndex | Func::FxR | Func::FxG | Func::FxB
        )
}

/// Lumière produite par des valeurs (décodées comme une trame reçue à l'instant 0), en simple
/// précision comme la version JavaScript (`Float32Array`) : les mêmes réglages en sortent.
fn light_of(mode: &DmxMode, values: &[u16], pixel_count: usize, o: Options) -> Vec<f64> {
    let mut out = vec![0.0; pixel_count * 3];
    let data = fader_buffer(mode, Some(values));
    let d = Decode { mode, data: &data, address: 1, pixel_count, time: 0.0, white_rgb: cct_to_rgb(o.white_k, 0.0), curve: o.curve, index_colors: Some(astera_index_colors()) };
    decode_dmx_output(&d, None, &mut out);
    out.iter().map(|v| *v as f32 as f64).collect()
}

/// Valeurs d'un mode → un autre (changement de profil). Même façon de faire la couleur : on recopie
/// fonction par fonction. Sinon on repart de la lumière produite, pixel par pixel (blanc retrouvé en
/// Kelvin quand c'en est un), et les autres fonctions (stroboscope, vitesse…) suivent si elles existent.
pub fn remap_values(from: &DmxMode, to: &DmxMode, values: &[u16], pixel_count: usize, o: Options) -> Values {
    let copied = copy_attributes(from, to, values, pixel_count);
    let from_keys: Vec<(Key, bool)> = from.compiled().slots.iter().filter(|s| is_color_function(s.func())).map(|s| (s.key(), s.px >= 0)).collect();
    let to_slots = &to.compiled().slots;
    let same_colors = to_slots.iter().filter(|s| is_color_function(s.func())).all(|s| from_keys.contains(&(s.key(), s.px >= 0)));
    if same_colors && mode_pixels(from, pixel_count) == mode_pixels(to, pixel_count) {
        return copied;
    }

    // lumière produite par l'ancien mode, moyenne sur chaque pixel du nouveau
    let out = light_of(from, values, pixel_count, o);
    let to_px = mode_pixels(to, pixel_count);
    let mut looks = Vec::with_capacity(to_px);
    for p in 0..to_px {
        let a = p * pixel_count / to_px;
        let b = (a + 1).max((p + 1) * pixel_count / to_px);
        let mut c = [0.0; 3];
        for j in a..b {
            for k in 0..3 {
                c[k] += out[j * 3 + k] / (b - a) as f64;
            }
        }
        let m = max3(c);
        let target = if m > 0.0 { c.map(|x| x / m) } else { [1.0; 3] };
        let w = fit_white(target);
        looks.push(if w.d < 2e-4 { Look { level: Some(m.min(1.0)), cct: Some(w.cct), tint: Some(w.tint), ..Look::default() } } else { Look { level: Some(m.min(1.0)), rgb: Some(target), ..Look::default() } });
    }
    let colored = look_to_values(to, &looks, o);
    to_slots.iter().enumerate().map(|(i, s)| if is_color_function(s.func()) { colored[i] } else { copied[i] }).collect()
}

/// Réglage que donnent des valeurs de console : intensité de sortie et couleur (blanc en Kelvin et
/// teinte ±vert quand c'en est un, sinon teinte et saturation), d'après la lumière produite.
pub fn look_of_values(mode: &DmxMode, values: &[u16], pixel_count: usize, o: Options) -> Look {
    let out = light_of(mode, values, pixel_count, o);
    let mut c = [0.0; 3];
    for p in 0..pixel_count {
        for k in 0..3 {
            c[k] += out[p * 3 + k] / pixel_count as f64;
        }
    }
    let m = max3(c);
    if !(m > 0.0) {
        return Look { level: Some(0.0), cct: Some(o.white_k), tint: Some(0.0), ..Look::default() };
    }
    let target = c.map(|x| x / m);
    let w = fit_white(target);
    if w.d < 2e-4 {
        return Look { level: Some(m.min(1.0)), cct: Some(js_round(w.cct)), tint: Some(js_round(w.tint * 100.0) / 100.0), ..Look::default() };
    }
    let hs = fit_hue_sat(target, cct_to_rgb(o.white_k, 0.0));
    Look { level: Some(m.min(1.0)), hue: Some(hs.hue), sat: Some(hs.sat), cct: Some(o.white_k), ..Look::default() }
}

/// Changement de couleur demandé : Kelvin ou teinte ±vert (un blanc), teinte ou saturation (une couleur).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColorChange {
    pub cct: Option<f64>,
    pub tint: Option<f64>,
    pub hue: Option<f64>,
    pub sat: Option<f64>,
}

/// Change la couleur de valeurs de console sans toucher à l'intensité. Les canaux du mode servent
/// quand ils existent (le reste ne bouge pas) ; sinon la couleur est refaite avec ses émetteurs.
/// `look` : réglage de départ (sinon retrouvé d'après les valeurs).
pub fn adjust_color(mode: &DmxMode, values: &[u16], change: ColorChange, pixel_count: usize, o: Options, look: Option<Look>) -> (Values, Look) {
    let c = mode.compiled();
    let slots = &c.slots;
    let has = |f| c.has(f);
    let base = look.unwrap_or_else(|| look_of_values(mode, values, pixel_count, o));
    let level = base.level;
    let mut cct = base.cct.unwrap_or(o.white_k);
    let mut tint = base.tint.unwrap_or(0.0);
    let mut hue = base.hue.unwrap_or(0.0);
    let mut sat = base.sat.unwrap_or(0.0);
    let white = change.cct.is_some() || change.tint.is_some();
    if white {
        cct = change.cct.unwrap_or(cct);
        tint = change.tint.unwrap_or(tint);
        hue = change.hue.unwrap_or(hue);
        sat = 0.0; // un réglage de blanc : la lumière redevient blanche
    } else {
        if let Some(h) = change.hue {
            hue = h;
            if !(sat > 0.0) {
                sat = 1.0; // on tourne la teinte d'un blanc : la couleur apparaît
            }
        }
        if let Some(s) = change.sat {
            sat = s;
        }
    }
    let emitters = slots.iter().any(|s| s.func().is_emitter());
    let mut out = values.to_vec();
    if out.len() < slots.len() {
        out.resize(slots.len(), 0);
    }
    let set_all = |f: Func, v: f64, out: &mut Values| {
        for (i, s) in slots.iter().enumerate() {
            if s.func() == f {
                out[i] = encode_slot(s, v);
            }
        }
    };
    // canaux du mode : Kelvin / teinte ±vert, fondu blanc ↔ couleur, teinte / saturation
    let white_direct = white && has(Func::Cct) && (!emitters || has(Func::Crossfade)) && (change.tint.is_none() || has(Func::Tint));
    let color_direct = !white && has(Func::Hue) && !emitters;
    if white_direct {
        set_all(Func::Cct, cct, &mut out);
        if has(Func::Tint) {
            set_all(Func::Tint, tint, &mut out);
        }
        if has(Func::Crossfade) {
            set_all(Func::Crossfade, 0.0, &mut out);
        }
        if has(Func::Sat) {
            set_all(Func::Sat, 0.0, &mut out);
        }
        return (out, Look { level, cct: Some(cct), tint: Some(tint), ..Look::default() });
    }
    if color_direct {
        set_all(Func::Hue, hue, &mut out);
        if has(Func::Sat) {
            set_all(Func::Sat, sat, &mut out);
        }
        if has(Func::Crossfade) {
            set_all(Func::Crossfade, 1.0, &mut out);
        }
        return (out, Look { level, hue: Some(hue), sat: Some(sat), cct: Some(cct), ..Look::default() });
    }
    // sinon : même intensité, couleur refaite ; les autres fonctions (stroboscope, effets…) ne bougent pas
    let target = if sat > 0.0 { Look { level, hue: Some(hue), sat: Some(sat), cct: Some(cct), ..Look::default() } } else { Look { level, cct: Some(cct), tint: Some(tint), ..Look::default() } };
    let colored = look_to_values(mode, &[target], o);
    let values = slots.iter().enumerate().map(|(i, s)| if is_color_function(s.func()) { colored[i] } else { values.get(i).copied().unwrap_or(0) }).collect();
    (values, target)
}

/// Réglage actuel (hors DMX) : niveau de sortie et couleur. `source` : source actuelle (« app »…).
pub fn current_look(f: &Fixture, ty: &FixtureType, source: Option<&str>) -> Look {
    let st = Some(&f.state);
    if ty.is_astera_v5() {
        let app = field(st, "app");
        if source == Some("app") && truthy(app) {
            return Look { level: Some(num_or(field(app, "intensity"), 1.0)), rgb: Some(rgb_of(field(app, "rgb")).unwrap_or([1.0; 3])), ..Look::default() };
        }
        let sa = setting(f, "standalone");
        let first = field(sa, "colors").and_then(Value::as_array).and_then(|c| c.first()).filter(|c| truthy(Some(c)));
        let level = clamp01((num_or(field(sa, "intensity"), 100.0) / 100.0) * (num_or(setting(f, "brightness"), 100.0) / 100.0));
        let Some(spec) = first else {
            return Look { level: Some(level), cct: Some(5600.0), tint: Some(0.0), ..Look::default() };
        };
        if text(field(Some(spec), "kind")) == Some("hsi") {
            let sat = num(field(Some(spec), "sat"));
            let mut look = Look { level: Some(level), cct: Some(num_or(field(Some(spec), "cct"), 5600.0)), tint: Some(num_or(field(Some(spec), "green"), 0.0)), ..Look::default() };
            if let Some(s) = sat.filter(|s| *s > 0.0) {
                look.hue = Some(num_or(field(Some(spec), "hue"), 0.0));
                look.sat = Some(s / 100.0);
            }
            return look;
        }
        return Look { level: Some(level), rgb: Some(color_spec_rgb(Some(spec))), ..Look::default() };
    }
    let m = field(st, "manual");
    let curve = Curve::named(text(setting(f, "dimmingCurve")), Curve::Linear);
    let level = Some(curve.apply(clamp01(num_or(field(m, "intensity"), 0.0))));
    if ty.is_traditional() {
        let nominal = ty.lamp.as_ref().and_then(|l| l.cct).filter(|c| *c != 0.0).unwrap_or(if ty.is_hmi() { 5600.0 } else { 3200.0 });
        return Look { level, cct: Some(nominal), ..Look::default() };
    }
    match text(field(m, "mode")) {
        Some("hsi") => Look { level, hue: Some(num_or_falsy(field(m, "hue"), 0.0)), sat: Some(num_or(field(m, "sat"), 1.0)), cct: Some(num_or_falsy(setting(f, "whitePoint"), 5600.0)), ..Look::default() },
        Some("rgb") => Look { level, rgb: Some(rgb_of(field(m, "rgb")).unwrap_or([1.0; 3])), ..Look::default() },
        _ => Look { level, cct: Some(num_or_falsy(field(m, "cct"), 5600.0)), tint: Some(num_or_falsy(field(m, "tint"), 0.0)), ..Look::default() },
    }
}

/// Valeurs de la console pour `mode` au plus près de ce que fait le projecteur.
/// `dmx` : univers reçus ; `source` : source actuelle (« dmx », « manual », « app »…).
pub fn seed_console(f: &Fixture, ty: &FixtureType, mode: &DmxMode, dmx: Option<&Universes>, source: Option<&str>) -> Values {
    let cur = ty.mode(f.settings.dmx_mode.as_deref());
    if console_held(f) {
        return active_console_values(f, ty, Some(mode), &mut Runtime::default()).unwrap_or_default();
    }
    if let (Some("dmx"), Some(cur)) = (source, cur)
        && let Some(data) = dmx.and_then(|d| d.get(&f.settings.universe?))
    {
        let raw = read_raw_values(cur, data, f.settings.dmx_address.unwrap_or(1) as i64);
        return if cur.id == mode.id { raw } else { remap_values(cur, mode, &raw, ty.pixels, console_options(f, ty)) };
    }
    let m = field(Some(&f.state), "manual");
    if !ty.is_astera_v5()
        && text(field(m, "mode")) == Some("faders")
        && let Some(cur) = cur
        && let Some(values) = values_for(field(m, "faders"), cur)
    {
        // ancien mode « Canaux » des réglages : mêmes valeurs
        return if cur.id == mode.id { values } else { remap_values(cur, mode, &values, ty.pixels, console_options(f, ty)) };
    }
    look_to_values(mode, &[current_look(f, ty, source)], console_options(f, ty))
}
