//! Comportement lumineux des projecteurs Astera (plateforme firmware « V5 »), portage de
//! `shared/engine/astera-light.js` : sélection d'entrée et verrouillage AUTO, mode autonome, DMX
//! (CRMX / filaire), DMX FAIL, limitation par l'autonomie choisie, BlueMode, demande
//! « ACCEPT CONFIG ? ».

use super::console::{active_console_values, fader_buffer};
use super::dmx::{Curve, Decode, decode_dmx_output};
use super::effects::{Program, render_program};
use super::gels::{astera_index_colors, color_spec_rgb};
use super::output::Source;
use super::{Ctx, Runtime, field, num, num_or, num_or_falsy, rgb_of, text, truthy};
use crate::color::{Rgb, cct_to_rgb};
use crate::library::FixtureType;
use crate::project::Fixture;
use serde_json::Value;

/// Entrée active d'un Astera (ce qu'affiche son écran).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Detecting,
    Crmx,
    Wired,
    App,
    Standalone,
    Art3,
    Off,
    BlueMode,
    Standby,
}

impl Input {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detecting => "DETECTING",
            Self::Crmx => "CRMX",
            Self::Wired => "WIRED",
            Self::App => "APP",
            Self::Standalone => "STANDALONE",
            Self::Art3 => "ART3",
            Self::Off => "OFF",
            Self::BlueMode => "BLUEMODE",
            Self::Standby => "STANDBY",
        }
    }
}

/// Réglage INPUT SELECT du menu → entrée.
fn input_of_select(v: &str) -> Option<Input> {
    Some(match v {
        "STANDALONE" => Input::Standalone,
        "APP CONTROL" => Input::App,
        "Wired DMX" => Input::Wired,
        "ART3 DMX" => Input::Art3,
        "CRMX wirel. DMX" => Input::Crmx,
        _ => return None,
    })
}

/// Durée de cycle de la vitesse autonome (0 = 0,09 s … 9 = 10 s).
pub fn standalone_speed_seconds(i: f64) -> f64 {
    0.09 * (10.0f64 / 0.09).powf(i / 9.0)
}

/// Facteur de sortie maximal imposé par l'autonomie choisie (la lampe se bride pour tenir la durée).
pub fn runtime_factor(ty: &FixtureType, runtime_hours: f64) -> f64 {
    let full = ty.max_brightness_runtime.filter(|r| *r != 0.0).unwrap_or(1.5);
    let hours = if runtime_hours == 0.0 || runtime_hours.is_nan() { 5.0 } else { runtime_hours };
    (full / hours.max(1.0)).min(1.0)
}

fn fill(out: &mut [f64], pixels: usize, rgb: Rgb, k: f64) {
    for p in 0..pixels {
        out[p * 3] = rgb[0] * k;
        out[p * 3 + 1] = rgb[1] * k;
        out[p * 3 + 2] = rgb[2] * k;
    }
}

fn zero(out: &mut [f64]) {
    out.fill(0.0);
}

fn setting<'a>(f: &'a Fixture, key: &str) -> Option<&'a Value> {
    f.settings.extra.get(key).filter(|v| !v.is_null())
}

/// Rendu du « look » autonome (programme interne + palette de 4 couleurs).
pub fn render_standalone(f: &Fixture, ty: &FixtureType, time: f64, rt: &mut Runtime, out: &mut [f64]) {
    let sa = setting(f, "standalone");
    let colors: Vec<Rgb> = field(sa, "colors").and_then(Value::as_array).map(|c| c.iter().map(|spec| color_spec_rgb(Some(spec))).collect()).unwrap_or_default();
    let program = num(field(sa, "program"));
    if rt.sa_program != program {
        rt.sa_program = program;
        rt.sa_start = Some(time);
    }
    let seconds = standalone_speed_seconds(num_or(field(sa, "speed"), 5.0));
    // vitesse 0..1 du moteur d'effets telle que le cycle dure `seconds`
    let speed = (seconds / 10.0).ln() / (0.09f64 / 10.0).ln();
    render_program(
        &Program {
            program: num_or_falsy(field(sa, "program"), 0.0) as usize,
            colors: &colors,
            pixels: ty.pixels,
            time: time - rt.sa_start.unwrap_or(0.0),
            speed,
            fade: num_or(field(sa, "fade"), 50.0) / 100.0,
            ..Program::default()
        },
        out,
    );
    let k = (num_or(field(sa, "intensity"), 100.0) / 100.0) * (num_or(setting(f, "brightness"), 100.0) / 100.0);
    for v in out.iter_mut().take(ty.pixels * 3) {
        *v *= k;
    }
}

/// Entrée active (verrouillage AUTO : la première source active est retenue jusqu'à l'extinction
/// ou un changement manuel).
pub fn resolve_input(f: &Fixture, ctx: &Ctx, rt: &mut Runtime) -> Input {
    let live = f.settings.universe.is_some_and(|u| ctx.live.contains(&u));
    let select = text(setting(f, "inputSelect")).filter(|s| !s.is_empty()).unwrap_or("AUTO");
    if select != "AUTO" {
        rt.latch = None;
        return input_of_select(select).unwrap_or(Input::Standalone);
    }
    if rt.latch.is_none() {
        if truthy(setting(f, "crmxLinked")) && live {
            rt.latch = Some(Input::Crmx);
        } else if truthy(setting(f, "wired")) && live {
            rt.latch = Some(Input::Wired);
        } else if truthy(field(Some(&f.state), "app")) {
            rt.latch = Some(Input::App);
        }
    }
    rt.latch.unwrap_or(Input::Detecting)
}

/// Sortie d'un projecteur Astera V5 (`out` : pixels × 3).
pub fn compute_astera_output(f: &Fixture, ty: &FixtureType, ctx: &Ctx, out: &mut [f64], rt: &mut Runtime) -> Source {
    let st = Some(&f.state);
    let time = ctx.time;
    let px = ty.pixels;
    if !truthy(field(st, "power")) {
        rt.latch = None;
        rt.input = Some(Input::Off);
        zero(out);
        return Source::Off;
    }
    if truthy(field(st, "blueMode")) {
        // clignotement bleu rapide, ou couleur choisie (BlueMode de l'AX1)
        let on = if (time * 3.0) % 1.0 < 0.5 { 1.0 } else { 0.08 };
        fill(out, px, rt.blue_color.unwrap_or([0.0, 0.12, 1.0]), on * 0.8);
        rt.input = Some(Input::BlueMode);
        return Source::BlueMode;
    }
    if truthy(field(st, "standby")) {
        zero(out);
        rt.input = Some(Input::Standby);
        return Source::Standby;
    }

    let input = resolve_input(f, ctx, rt);
    rt.input = Some(input);
    let curve = Curve::named(text(setting(f, "dimmerCurve")), Curve::Standard);
    let white_rgb = cct_to_rgb(num_or_falsy(setting(f, "whitePoint"), 5600.0), 0.0);
    let mut src = Source::Standalone;
    let mode = ty.mode(f.settings.dmx_mode.as_deref());
    let address = f.settings.dmx_address.unwrap_or(1) as i64;
    let decode = |data: &[u8], address: i64, rt: &mut Runtime, out: &mut [f64]| {
        if let Some(mode) = mode {
            let d = Decode { mode, data, address, pixel_count: px, time, white_rgb, curve, index_colors: Some(astera_index_colors()) };
            decode_dmx_output(&d, Some(&mut rt.fx), out);
        }
    };

    if let Some(values) = active_console_values(f, ty, mode, rt) {
        // console ARKLIGHT : elle passe avant l'entrée choisie (comme un DMX reçu, profil du projecteur)
        let buf = fader_buffer(mode.expect("mode de la console"), Some(&values));
        decode(&buf, 1, rt, out);
        src = Source::Console;
    } else if matches!(input, Input::Crmx | Input::Wired) {
        let connected = truthy(setting(f, if input == Input::Crmx { "crmxLinked" } else { "wired" }));
        let data = f.settings.universe.and_then(|u| ctx.dmx.get(&u));
        let signal = connected && f.settings.universe.is_some_and(|u| ctx.live.contains(&u));
        rt.signal = signal;
        match data {
            Some(data) if signal => {
                decode(data, address, rt, out);
                src = Source::Dmx;
            }
            _ => match text(setting(f, "dmxFail")) {
                Some("EMERGENCY LIGHT") => {
                    fill(out, px, cct_to_rgb(4000.0, 0.0), 1.0);
                    src = Source::Emergency;
                }
                Some("BLACKOUT") => {
                    zero(out);
                    src = Source::Blackout;
                }
                Some("STANDALONE") => render_standalone(f, ty, time, rt, out),
                // HOLD : dernier état reçu
                _ => match data {
                    Some(data) if connected => {
                        decode(data, address, rt, out);
                        src = Source::Dmx;
                    }
                    _ => zero(out),
                },
            },
        }
    } else if input == Input::App && truthy(field(st, "app")) {
        let app = field(st, "app");
        fill(out, px, rgb_of(field(app, "rgb")).unwrap_or([1.0; 3]), num_or(field(app, "intensity"), 1.0));
        src = Source::App;
    } else {
        render_standalone(f, ty, time, rt, out);
    }

    // demande de l'appli « ACCEPT CONFIG ? » : flash blanc toutes les 2 s
    if truthy(field(st, "acceptConfig")) && time % 2.0 < 0.15 {
        fill(out, px, [1.0; 3], 1.0);
    }

    // bridage par l'autonomie choisie (toujours actif, même sur secteur)
    let gain = match ty.output_gain {
        Some(g) if g != 0.0 && text(setting(f, "outputGain")) == Some("OFF") => g,
        _ => 1.0,
    };
    let limited = setting(f, "runtimeLimit") != Some(&Value::Bool(false)) && ty.power.as_deref() != Some("mains");
    let k = if limited { runtime_factor(ty, num(setting(f, "runtime")).unwrap_or(f64::NAN)) } else { 1.0 } * gain;
    for v in out.iter_mut().take(px * 3) {
        *v *= k;
    }
    rt.src = Some(src);
    src
}
