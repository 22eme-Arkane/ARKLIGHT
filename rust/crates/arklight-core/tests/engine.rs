//! Essais de parité du moteur lumière : mêmes entrées, mêmes sorties que la version JavaScript.
//! Données calculées par `node tools/rust/export-engine-data.mjs` (tests/data/engine.json).

use arklight_core::engine::console::{self, ColorChange, Look, Options};
use arklight_core::engine::dmx::{Attrs, Curve, Decode, FAN_MODES, Func, FxState, decode_dmx_output, read_attributes};
use arklight_core::engine::gels::{astera_index_colors, color_spec_rgb, gel_color};
use arklight_core::engine::{Ctx, Runtime, Universe, Universes, compute_output};
use arklight_core::library::{DmxMode, FixtureType};
use arklight_core::project::{Fixture, Project};
use arklight_core::library;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

fn data() -> &'static Value {
    static D: OnceLock<Value> = OnceLock::new();
    D.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/engine.json");
        serde_json::from_str(&std::fs::read_to_string(path).expect("engine.json")).expect("engine.json illisible")
    })
}

/// Écarts relevés (tous affichés à la fin plutôt que l'arrêt au premier).
#[derive(Default)]
struct Report {
    checked: usize,
    fails: Vec<String>,
}

impl Report {
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        self.checked += 1;
        if !ok {
            self.fails.push(what());
        }
    }

    fn finish(self, name: &str) {
        let n = self.fails.len();
        let shown: Vec<_> = self.fails.into_iter().take(15).collect();
        assert!(n == 0, "{name} : {n} écarts sur {} essais :\n{}", self.checked, shown.join("\n"));
        eprintln!("{name} : {} essais conformes", self.checked);
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
}

fn f64s(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect()).unwrap_or_default()
}

fn all_close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| close(*x, *y))
}

fn rgb_opt(v: &Value) -> Option<[f64; 3]> {
    let a = f64s(v);
    (a.len() == 3).then(|| [a[0], a[1], a[2]])
}

fn ty(id: &str) -> &'static FixtureType {
    library().fixture(id).unwrap_or_else(|| panic!("modèle {id}"))
}

fn mode<'a>(t: &'a FixtureType, id: &str) -> &'a DmxMode {
    t.dmx.modes.iter().find(|m| m.id == id).unwrap_or_else(|| panic!("mode {id} de {}", t.id))
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

fn hex_bytes(h: &str) -> Vec<u8> {
    (0..h.len() / 2).map(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).expect("hex")).collect()
}

fn universe(h: &str) -> Universe {
    let mut u = [0u8; 512];
    for (i, b) in hex_bytes(h).into_iter().enumerate().take(512) {
        u[i] = b;
    }
    u
}

fn values(v: &Value) -> Vec<u16> {
    v.as_array().expect("valeurs").iter().map(|x| x.as_f64().expect("valeur") as u16).collect()
}

fn options(v: &Value) -> Options {
    Options { white_k: v["whiteK"].as_f64().unwrap_or(5600.0), curve: Curve::parse(s(&v["curve"])).unwrap_or(Curve::Linear) }
}

fn look_of(v: &Value) -> Look {
    Look { level: v["level"].as_f64(), cct: v["cct"].as_f64(), tint: v["tint"].as_f64(), hue: v["hue"].as_f64(), sat: v["sat"].as_f64(), rgb: rgb_opt(&v["rgb"]) }
}

fn same_look(a: &Look, b: &Look) -> bool {
    let o = |x: Option<f64>, y: Option<f64>| match (x, y) {
        (Some(x), Some(y)) => close(x, y),
        (None, None) => true,
        _ => false,
    };
    o(a.level, b.level) && o(a.cct, b.cct) && o(a.tint, b.tint) && o(a.hue, b.hue) && o(a.sat, b.sat) && a.rgb.map(Vec::from).unwrap_or_default().len() == b.rgb.map(Vec::from).unwrap_or_default().len() && all_close(&a.rgb.map(Vec::from).unwrap_or_default(), &b.rgb.map(Vec::from).unwrap_or_default())
}

/// Attributs en JSON, comme la version JavaScript (ventilateur : son nom).
fn attrs_json(a: &Attrs) -> BTreeMap<String, Value> {
    a.iter()
        .map(|(k, v)| {
            let value = if k.func == Func::Fan { Value::from(FAN_MODES[v as usize]) } else { Value::from(v) };
            (k.name(), value)
        })
        .collect()
}

fn same_attrs(rust: &Attrs, js: &Value) -> bool {
    let r = attrs_json(rust);
    let Some(j) = js.as_object() else { return false };
    r.len() == j.len()
        && j.iter().all(|(k, v)| match (r.get(k), v) {
            (Some(Value::String(a)), Value::String(b)) => a == b,
            (Some(a), b) => a.as_f64().zip(b.as_f64()).is_some_and(|(a, b)| close(a, b)),
            _ => false,
        })
}

#[test]
fn gels_et_couleurs() {
    let d = &data()["gels"];
    let mut r = Report::default();
    let ic = astera_index_colors();
    for (v, e) in d["index"].as_array().unwrap().iter().enumerate() {
        let rust = ic.color(v as f64);
        let js = rgb_opt(&e["rgb"]);
        r.check(rust.is_some() == js.is_some() && all_close(&rust.map(Vec::from).unwrap_or_default(), &js.map(Vec::from).unwrap_or_default()) && ic.name(v as f64) == s(&e["name"]), || format!("index {v} : {rust:?} au lieu de {js:?}"));
    }
    for e in d["names"].as_array().unwrap() {
        let rust = gel_color(s(&e["name"]));
        r.check(all_close(&rust, &f64s(&e["rgb"])), || format!("gel « {} » : {rust:?} au lieu de {}", e["name"], e["rgb"]));
    }
    for e in d["specs"].as_array().unwrap() {
        let rust = color_spec_rgb(Some(&e["spec"]));
        r.check(all_close(&rust, &f64s(&e["rgb"])), || format!("couleur {} : {rust:?} au lieu de {}", e["spec"], e["rgb"]));
    }
    r.finish("gels et couleurs");
}

#[test]
fn modes_a_plat() {
    let mut r = Report::default();
    for e in data()["modes"].as_array().unwrap() {
        let m = mode(ty(s(&e["type"])), s(&e["mode"]));
        let c = m.compiled();
        let slots: Vec<String> = c.slots.iter().map(|sl| format!("{}/{}/{}", sl.key().name(), sl.px, sl.bytes())).collect();
        r.check(c.footprint as f64 == e["footprint"].as_f64().unwrap() && slots.join(" ") == s(&e["slots"]), || format!("{} mode {} : {} canaux", e["type"], e["mode"], c.footprint));
    }
    r.finish("modes");
}

#[test]
fn decodage_dmx() {
    let mut r = Report::default();
    let ic = astera_index_colors();
    for e in data()["decode"].as_array().unwrap() {
        let t = ty(s(&e["type"]));
        let m = mode(t, s(&e["mode"]));
        let pixels = e["pixels"].as_f64().unwrap() as usize;
        let address = e["address"].as_f64().unwrap() as i64;
        let mut frame = [0u8; 512];
        let start = e["data"][0].as_f64().unwrap() as usize;
        for (i, b) in hex_bytes(s(&e["data"][1])).into_iter().enumerate() {
            if start + i < 512 {
                frame[start + i] = b;
            }
        }
        let white = arklight_core::color::cct_to_rgb(e["white"].as_f64().unwrap(), 0.0);
        let curve = Curve::parse(s(&e["curve"])).expect("courbe");
        let time = e["time"].as_f64().unwrap();
        let mut fx = FxState::default();
        let mut scratch = vec![0.0; pixels * 3];
        let p = |time| Decode { mode: m, data: &frame, address, pixel_count: pixels, time, white_rgb: white, curve, index_colors: Some(ic) };
        decode_dmx_output(&p(0.5), Some(&mut fx), &mut scratch);
        let mut out = vec![0.0; pixels * 3];
        decode_dmx_output(&p(time), Some(&mut fx), &mut out);
        let js = f64s(&e["out"]);
        r.check(all_close(&out, &js), || format!("{} mode {} @{address} t={time} : {:?} au lieu de {:?}", e["type"], e["mode"], &out[..out.len().min(6)], &js[..js.len().min(6)]));
        if !e["fixture"].is_null() {
            let (fa, px) = read_attributes(m, &frame, address, m.pixels_on(pixels));
            let js_px = e["attrs"].as_array().unwrap();
            let ok = same_attrs(&fa, &e["fixture"]) && px.len() == js_px.len() && px.iter().zip(js_px).all(|(a, b)| same_attrs(a, b));
            r.check(ok, || format!("{} mode {} : attributs {:?} au lieu de {}", e["type"], e["mode"], attrs_json(&fa), e["fixture"]));
        }
    }
    r.finish("décodage DMX");
}

#[test]
fn console() {
    let c = &data()["console"];
    let mut r = Report::default();
    for e in c["defaults"].as_array().unwrap() {
        let m = mode(ty(s(&e["type"])), s(&e["mode"]));
        let rust = console::default_faders(m);
        r.check(rust == values(&e["values"]), || format!("départ {} {} : {rust:?} au lieu de {}", e["type"], e["mode"], e["values"]));
    }
    for e in c["toValues"].as_array().unwrap() {
        let m = mode(ty(s(&e["type"])), s(&e["mode"]));
        let looks: Vec<Look> = e["looks"].as_array().unwrap().iter().map(look_of).collect();
        let rust = console::look_to_values(m, &looks, options(&e["opts"]));
        r.check(rust == values(&e["values"]), || format!("réglage → valeurs {} {} {} : {rust:?} au lieu de {}", e["type"], e["mode"], e["looks"], e["values"]));
    }
    for e in c["remap"].as_array().unwrap() {
        let t = ty(s(&e["type"]));
        let rust = console::remap_values(mode(t, s(&e["from"])), mode(t, s(&e["to"])), &values(&e["values"]), e["pixels"].as_f64().unwrap() as usize, options(&e["opts"]));
        r.check(rust == values(&e["result"]), || format!("profil {} {} → {} : {rust:?} au lieu de {}", e["type"], e["from"], e["to"], e["result"]));
    }
    for e in c["lookOf"].as_array().unwrap() {
        let m = mode(ty(s(&e["type"])), s(&e["mode"]));
        let rust = console::look_of_values(m, &values(&e["values"]), e["pixels"].as_f64().unwrap() as usize, options(&e["opts"]));
        r.check(same_look(&rust, &look_of(&e["look"])), || format!("réglage de {} {} : {rust:?} au lieu de {}", e["type"], e["mode"], e["look"]));
    }
    for e in c["adjust"].as_array().unwrap() {
        let m = mode(ty(s(&e["type"])), s(&e["mode"]));
        let ch = &e["change"];
        let change = ColorChange { cct: ch["cct"].as_f64(), tint: ch["tint"].as_f64(), hue: ch["hue"].as_f64(), sat: ch["sat"].as_f64() };
        let (vals, look) = console::adjust_color(m, &values(&e["values"]), change, e["pixels"].as_f64().unwrap() as usize, options(&e["opts"]), None);
        r.check(vals == values(&e["result"]) && same_look(&look, &look_of(&e["look"])), || format!("couleur {} {} {} : {vals:?} {look:?} au lieu de {} {}", e["type"], e["mode"], ch, e["result"], e["look"]));
    }
    r.finish("console");
}

fn fixture_of(v: &Value) -> Fixture {
    serde_json::from_value(v.clone()).expect("projecteur")
}

#[test]
fn scenarios() {
    let mut r = Report::default();
    for sc in data()["scenarios"].as_array().unwrap() {
        let f = fixture_of(&sc["fixture"]);
        let t = ty(&f.type_id);
        let mut rt = Runtime::default();
        for (i, step) in sc["steps"].as_array().unwrap().iter().enumerate() {
            let dmx: Universes = step["dmx"].as_object().unwrap().iter().map(|(u, h)| (u.parse().unwrap(), universe(s(h)))).collect();
            let live: HashSet<u32> = f64s(&step["live"]).into_iter().map(|u| u as u32).collect();
            let ctx = Ctx { dmx: &dmx, live: &live, time: step["time"].as_f64().unwrap() };
            let mut out = vec![0.0; t.pixels * 3];
            let src = compute_output(&f, t, &ctx, &mut out, &mut rt);
            let js = f64s(&step["out"]);
            r.check(all_close(&out, &js) && src.as_str() == s(&step["source"]), || format!("{} (image {i}) : {} {:?} au lieu de {} {:?}", sc["name"], src.as_str(), &out[..out.len().min(6)], step["source"], &js[..js.len().min(6)]));
        }
    }
    r.finish("scénarios");
}

#[test]
fn depart_de_la_console() {
    let mut r = Report::default();
    for e in data()["seeds"].as_array().unwrap() {
        let f = fixture_of(&e["fixture"]);
        let t = ty(&f.type_id);
        let dmx: Option<Universes> = e["dmx"].as_object().map(|o| o.iter().map(|(u, h)| (u.parse().unwrap(), universe(s(h)))).collect());
        let rust = console::seed_console(&f, t, mode(t, s(&e["mode"])), dmx.as_ref(), e["source"].as_str());
        r.check(rust == values(&e["values"]), || format!("{} : {rust:?} au lieu de {}", e["name"], e["values"]));
    }
    r.finish("départ de la console");
}

#[test]
fn projets_d_exemple_eclaires_pareil() {
    use arklight_core::engine::Lights;
    let mut r = Report::default();
    for name in ["night", "clip"] {
        let path = format!("{}/tests/data/demo-{name}.json", env!("CARGO_MANIFEST_DIR"));
        let mut p = Project::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
        p.sanitize();
        let (dmx, live) = (Universes::new(), HashSet::new());
        let mut lights = Lights::default();
        lights.update(&p.fixtures, &Ctx { dmx: &dmx, live: &live, time: 1.0 });
        for e in data()["demos"][name].as_array().unwrap() {
            let light = lights.get(s(&e["id"]));
            let out = light.map(|l| l.pixels.clone()).unwrap_or_default();
            let src = light.map_or("", |l| l.source.as_str());
            r.check(all_close(&out, &f64s(&e["out"])) && src == s(&e["source"]), || format!("{name} : {} {src} {out:?} au lieu de {} {}", e["id"], e["source"], e["out"]));
        }
    }
    r.finish("projets d'exemple");
}

#[test]
fn projets_d_exemple_deja_complets() {
    // les projets enregistrés par la version JavaScript ont déjà leurs valeurs par défaut :
    // les compléter ne doit rien changer
    for name in ["demo-night", "demo-clip"] {
        let path = format!("{}/tests/data/{name}.json", env!("CARGO_MANIFEST_DIR"));
        let mut p = Project::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let before = serde_json::to_value(&p).unwrap();
        p.sanitize();
        let after = serde_json::to_value(&p).unwrap();
        for (a, b) in before["fixtures"].as_array().unwrap().iter().zip(after["fixtures"].as_array().unwrap()) {
            assert_eq!(a, b, "{name} : projecteur {} complété", a["id"]);
        }
    }
}

#[test]
fn zoom_et_accessoires() {
    use arklight_core::accessories::{Front, effective};
    let mut r = Report::default();
    let opt = |a: Option<f64>, b: &Value| match (a, b.as_f64()) {
        (Some(x), Some(y)) => close(x, y),
        (None, None) => true,
        (Some(x), None) => x.is_nan() && !b.is_null(),
        _ => false,
    };
    for e in data()["effective"].as_array().unwrap() {
        let f = fixture_of(&e["fixture"]);
        let t = ty(&f.type_id);
        let eff = effective(t, &f);
        let mut ok = opt(eff.beam_angle, &e["beamAngle"]) && opt(eff.lux_at1m, &e["luxAt1m"]) && opt(eff.lumens, &e["lumens"]);
        match (&eff.accessories, &e["acc"]) {
            (None, Value::Null) => {}
            (Some(a), j) if j.is_object() => {
                let front = match a.front {
                    Some(Front::None) => Some("none".to_owned()),
                    Some(Front::Filter(fl)) => Some(fl.id.clone()),
                    None => None,
                };
                ok &= close(a.angle, j["angle"].as_f64().unwrap())
                    && close(a.lux, j["lux"].as_f64().unwrap())
                    && close(a.transmission, j["transmission"].as_f64().unwrap())
                    && a.tint.map(Vec::from).unwrap_or_default().len() == f64s(&j["tint"]).len()
                    && all_close(&a.tint.map(Vec::from).unwrap_or_default(), &f64s(&j["tint"]))
                    && a.modifier.map(|m| m.id.as_str()) == j["modifier"].as_str()
                    && a.grid.map(|g| g.id.as_str()) == j["grid"].as_str()
                    && a.gel.map(|g| g.id.as_str()) == j["gel"].as_str()
                    && front.as_deref() == j["front"].as_str();
            }
            _ => ok = false,
        }
        r.check(ok, || format!("{} {:?} : {:?} {:?} {:?} au lieu de {} {} {} {}", f.type_id, f.settings.extra.iter().filter(|(k, _)| ["zoom", "modifier", "front", "grid", "gel"].contains(&k.as_str())).collect::<Vec<_>>(), eff.beam_angle, eff.lux_at1m, eff.lumens, e["beamAngle"], e["luxAt1m"], e["lumens"], e["acc"]));
    }
    r.finish("zoom et accessoires");
}
