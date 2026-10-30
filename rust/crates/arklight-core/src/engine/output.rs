//! Sortie lumineuse d'un projecteur à un instant donné (couleur de chaque pixel, sRGB linéaire),
//! portage de `shared/engine/output.js`.

use super::astera::compute_astera_output;
use super::console::{active_console_values, fader_buffer, values_for};
use super::dmx::{Curve, Decode, decode_dmx_output, read_attributes};
use super::gels::astera_index_colors;
use super::{Ctx, Runtime, field, num_or, num_or_falsy, rgb_of, text, truthy};
use crate::color::{cct_to_rgb, clamp01, hs_to_rgb};
use crate::library::FixtureType;
use crate::project::Fixture;
use serde_json::Value;

/// Ce qui commande le projecteur en ce moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// boîtier sans lumière (AsteraBox…)
    Device,
    Off,
    Manual,
    Dmx,
    Console,
    Standalone,
    App,
    BlueMode,
    Standby,
    Emergency,
    Blackout,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Device => "device",
            Self::Off => "off",
            Self::Manual => "manual",
            Self::Dmx => "dmx",
            Self::Console => "console",
            Self::Standalone => "standalone",
            Self::App => "app",
            Self::BlueMode => "bluemode",
            Self::Standby => "standby",
            Self::Emergency => "emergency",
            Self::Blackout => "blackout",
        }
    }
}

fn setting<'a>(f: &'a Fixture, key: &str) -> Option<&'a Value> {
    f.settings.extra.get(key).filter(|v| !v.is_null())
}

/// Source de commande effective : DMX ou réglage manuel. En mode auto, le DMX prend la main dès
/// qu'un univers a été reçu (et le garde : « hold last look »).
pub fn control_source(f: &Fixture, has_dmx: bool) -> Source {
    match text(setting(f, "source")).filter(|s| !s.is_empty()).unwrap_or("auto") {
        "dmx" => Source::Dmx,
        "manual" => Source::Manual,
        _ if has_dmx => Source::Dmx,
        _ => Source::Manual,
    }
}

// ---------------------------------------------------------------- sources traditionnelles
/// Température de couleur d'une halogène gradée : CCT ≈ CCT nominale × flux^0,107
/// (3200 K → environ 2970 K à 50 % du flux ; brevet US 2004/0169477, Marumo / NHK).
pub fn tungsten_cct(nominal: f64, flux: f64) -> f64 {
    nominal * flux.max(1e-4).powf(0.107)
}

/// Inertie du filament (approximation, non mesurée) : environ 0,1 s pour 1 kW, plus lente pour les
/// grosses lampes.
fn filament_tau(watts: f64) -> f64 {
    let w = if watts == 0.0 || watts.is_nan() { 1000.0 } else { watts };
    0.04 + 0.06 * (w / 1000.0).sqrt()
}

/// HMI (approximation, à vérifier sur le vrai matériel) : ballast gradable de 50 à 100 % de
/// puissance, lumière plus froide quand on gradue, montée en régime d'environ 1 minute.
pub mod hmi {
    pub const MIN_POWER: f64 = 0.5;
    pub const DIM_SHIFT_K: f64 = 800.0;
    pub const WARMUP_TAU: f64 = 15.0;
    pub const WARMUP_START: f64 = 0.3;
}

fn compute_traditional(f: &Fixture, ty: &FixtureType, ctx: &Ctx, out: &mut [f64], rt: &mut Runtime) -> Source {
    let n = ty.pixels;
    let st = Some(&f.state);
    let power_on = truthy(field(st, "power"));
    let nominal = ty.lamp.as_ref().and_then(|l| l.cct).filter(|c| *c != 0.0).unwrap_or(if ty.is_hmi() { 5600.0 } else { 3200.0 });
    let curve = Curve::named(text(setting(f, "dimmingCurve")), Curve::Linear);
    let data = f.settings.universe.and_then(|u| ctx.dmx.get(&u));
    let mode = ty.mode(f.settings.dmx_mode.as_deref());
    let con_values = active_console_values(f, ty, mode, rt);
    let src = if con_values.is_some() { Source::Console } else { control_source(f, data.is_some()) };

    // niveau demandé par circuit (pixel) : canal Dimmer en DMX (ou console), intensité manuelle sinon
    let mut levels = vec![0.0; n];
    if power_on {
        if matches!(src, Source::Dmx | Source::Console) {
            let console_buf;
            let buf: Option<&[u8]> = match &con_values {
                Some(values) => {
                    console_buf = mode.map(|m| fader_buffer(m, Some(values)));
                    console_buf.as_ref().map(|b| &b[..])
                }
                None => data.map(|d| &d[..]),
            };
            if let (Some(mode), Some(buf)) = (mode, buf) {
                let mode_px = if mode.has_pixel_count() { mode.pixels_on(n) } else { 1 };
                let address = if src == Source::Console { 1 } else { f.settings.dmx_address.unwrap_or(1) as i64 };
                let (fa, pixels) = read_attributes(mode, buf, address, mode_px);
                for (p, level) in levels.iter_mut().enumerate() {
                    let v = pixels.get((mode_px - 1).min(p * mode_px / n)).and_then(|a| a.get(super::dmx::Func::Dimmer)).or(fa.get(super::dmx::Func::Dimmer)).unwrap_or(0.0);
                    *level = curve.apply(v);
                }
            }
        } else {
            levels.fill(curve.apply(clamp01(num_or(field(field(st, "manual"), "intensity"), 0.0))));
        }
    }

    let dt = rt.last_time.map_or(1.0, |last| (ctx.time - last).clamp(0.0, 1.0));
    rt.last_time = Some(ctx.time);
    if rt.flux.len() != n {
        rt.flux = vec![0.0; n];
    }
    let result = if src != Source::Manual {
        src
    } else if power_on {
        Source::Manual
    } else {
        Source::Off
    };

    if ty.is_hmi() {
        // allumage : la lampe monte en régime ; extinction immédiate
        if !power_on {
            rt.hmi_on = None;
        } else if rt.hmi_on.is_none() {
            rt.hmi_on = Some(ctx.time);
        }
        let warm = match rt.hmi_on {
            Some(on) if power_on => 1.0 - (1.0 - hmi::WARMUP_START) * (-(ctx.time - on) / hmi::WARMUP_TAU).exp(),
            _ => 0.0,
        };
        for p in 0..n {
            // niveau 0 : lampe éteinte ; au-dessus, puissance de 50 à 100 %
            let power = if levels[p] > 0.0 { hmi::MIN_POWER + (1.0 - hmi::MIN_POWER) * levels[p] } else { 0.0 };
            rt.flux[p] = power * warm;
            let cct = if power > 0.0 { nominal + (hmi::DIM_SHIFT_K * (1.0 - power)) / (1.0 - hmi::MIN_POWER) } else { nominal };
            let rgb = cct_to_rgb(cct, 0.0);
            for k in 0..3 {
                out[p * 3 + k] = rgb[k] * rt.flux[p];
            }
        }
        return result;
    }

    // tungstène : le filament suit le gradateur avec un léger retard, la couleur suit le flux
    let watts = ty.lamp.as_ref().and_then(|l| l.watts).unwrap_or(f64::NAN);
    let k = 1.0 - (-dt / filament_tau(watts)).exp();
    for p in 0..n {
        rt.flux[p] += (levels[p] - rt.flux[p]) * k;
        let fl = rt.flux[p];
        let rgb = cct_to_rgb(super::js_round(tungsten_cct(nominal, fl) / 10.0) * 10.0, 0.0);
        for c in 0..3 {
            out[p * 3 + c] = rgb[c] * fl;
        }
    }
    result
}

/// Sortie d'un projecteur : `out` reçoit `pixels` × 3 valeurs (sRGB linéaire) ; `rt` garde son état
/// d'une image à l'autre. Renvoie la source effective.
pub fn compute_output(f: &Fixture, ty: &FixtureType, ctx: &Ctx, out: &mut [f64], rt: &mut Runtime) -> Source {
    if ty.pixels == 0 {
        return Source::Device; // boîtier sans lumière (AsteraBox…)
    }
    if ty.is_astera_v5() {
        return compute_astera_output(f, ty, ctx, out, rt);
    }
    if ty.is_traditional() {
        return compute_traditional(f, ty, ctx, out, rt);
    }
    let st = Some(&f.state);
    if !truthy(field(st, "power")) {
        out.fill(0.0);
        return Source::Off;
    }
    let curve = Curve::named(text(setting(f, "dimmingCurve")), Curve::Linear);
    let white_rgb = cct_to_rgb(num_or_falsy(setting(f, "whitePoint"), 5600.0), 0.0);
    let data = f.settings.universe.and_then(|u| ctx.dmx.get(&u));
    let mode = ty.mode(f.settings.dmx_mode.as_deref());
    let decode = |buf: &[u8], address: i64, rt: &mut Runtime, out: &mut [f64]| {
        if let Some(mode) = mode {
            let d = Decode { mode, data: buf, address, pixel_count: ty.pixels, time: ctx.time, white_rgb, curve, index_colors: Some(astera_index_colors()) };
            decode_dmx_output(&d, Some(&mut rt.fx), out);
        }
    };

    // console ARKLIGHT : elle passe avant tout le reste tant qu'elle pilote le projecteur
    if let (Some(values), Some(m)) = (active_console_values(f, ty, mode, rt), mode) {
        decode(&fader_buffer(m, Some(&values)), 1, rt, out);
        return Source::Console;
    }

    if control_source(f, data.is_some()) == Source::Dmx {
        match (mode, data) {
            (Some(_), Some(data)) => decode(data, f.settings.dmx_address.unwrap_or(1) as i64, rt, out),
            _ => out.fill(0.0),
        }
        return Source::Dmx;
    }

    let m = field(st, "manual");
    if text(field(m, "mode")) == Some("faders")
        && let Some(mode) = mode
    {
        // ancien réglage « Canaux » (projets des premières versions) : lu comme un DMX reçu
        let values = values_for(field(m, "faders"), mode);
        decode(&fader_buffer(mode, values.as_deref()), 1, rt, out);
        return Source::Manual;
    }
    let rgb = match text(field(m, "mode")) {
        Some("hsi") => hs_to_rgb(num_or_falsy(field(m, "hue"), 0.0), num_or(field(m, "sat"), 1.0), white_rgb),
        Some("rgb") => rgb_of(field(m, "rgb")).unwrap_or([1.0; 3]),
        _ => cct_to_rgb(num_or_falsy(field(m, "cct"), 5600.0), num_or_falsy(field(m, "tint"), 0.0)),
    };
    let k = curve.apply(clamp01(num_or_falsy(field(m, "intensity"), 0.0)));
    for p in 0..ty.pixels {
        for c in 0..3 {
            out[p * 3 + c] = rgb[c] * k;
        }
    }
    Source::Manual
}
