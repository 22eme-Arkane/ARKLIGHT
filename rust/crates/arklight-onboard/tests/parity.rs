//! Parité avec la version JavaScript : mêmes gestes, mêmes écrans, mêmes voyants, mêmes
//! changements des réglages et de l'état (données calculées par
//! `node tools/rust/export-onboard-data.mjs`, tests/data/onboard.json).
//!
//! Chaque pas est rejoué comme dans le script : geste, sortie du moteur lumière (verrou d'entrée
//! des Astera), tick, puis relevé de l'écran, des voyants, du message, des pixels remplacés et du
//! projecteur. Le script n'écrit un relevé que s'il a changé : la valeur attendue est la dernière
//! relevée (réglages et état : champs changés, fusionnés ici).

use arklight_core::engine::{Ctx as EngineCtx, Runtime, Universes, compute_output};
use arklight_core::library::library;
use arklight_core::project::Fixture;
use arklight_onboard::{AsteraRuntime, Ctx, Env, Outcome, PANEL_TYPES, Panel, PanelKind, Screen, SliderZone, apply_ir_key, apply_shallow};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

fn data() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/onboard.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("onboard.json (lancer node tools/rust/export-onboard-data.mjs)")).expect("onboard.json illisible")
}

/// Premier écart entre deux documents JSON (null, false et absent sont équivalents ; nombres à
/// `eps` près).
fn diff(a: &Value, b: &Value, path: &str, eps: f64) -> Option<String> {
    let empty = |v: &Value| matches!(v, Value::Null | Value::Bool(false));
    // alignement à gauche : valeur par défaut des textes
    let norm = |k: &str, v: Option<&Value>| -> Value {
        match v {
            Some(Value::String(s)) if k == "align" && s == "l" => Value::Null,
            Some(v) => v.clone(),
            None => Value::Null,
        }
    };
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                let (va, vb) = (&norm(k, x.get(k)), &norm(k, y.get(k)));
                if let Some(d) = diff(va, vb, &format!("{path}.{k}"), eps) {
                    return Some(d);
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path} : {} éléments ≠ {} (JavaScript)\n    Rust : {a}\n    JS   : {b}", x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().find_map(|(i, (va, vb))| diff(va, vb, &format!("{path}[{i}]"), eps))
        }
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap_or(f64::NAN), y.as_f64().unwrap_or(f64::NAN));
            let tol = eps * x.abs().max(y.abs()).max(1.0);
            if (x - y).abs() <= tol { None } else { Some(format!("{path} : {x} ≠ {y} (JavaScript)")) }
        }
        _ if empty(a) && empty(b) => None,
        _ if a == b => None,
        _ => Some(format!("{path} : {a} ≠ {b} (JavaScript)")),
    }
}

#[derive(Default)]
struct Report {
    checked: usize,
    fails: Vec<String>,
}

impl Report {
    fn finish(self, what: &str) {
        let n = self.fails.len();
        eprintln!("{what} : {} relevés comparés, {n} écarts", self.checked);
        let shown: Vec<_> = self.fails.into_iter().take(12).collect();
        assert!(n == 0, "{what} : {n} écarts sur {} relevés\n{}", self.checked, shown.join("\n"));
        assert!(self.checked > 0, "{what} : aucun relevé");
    }
}

fn leds_json(l: &arklight_onboard::Leds) -> Value {
    Value::Object(l.iter().map(|(k, v)| (k.clone(), json!({ "color": v.color, "blinkHz": v.blink_hz }))).collect())
}

fn screen_json(s: &Screen, generic: bool) -> Value {
    if generic {
        return match s {
            Screen::Text(t) => json!({ "lines": t.to_json()["lines"] }),
            _ => json!({ "lines": [] }),
        };
    }
    s.to_json()
}

fn fixture_of(v: &Value) -> Fixture {
    serde_json::from_value(v.clone()).expect("projecteur du scénario")
}

fn settings_json(f: &Fixture) -> Value {
    serde_json::to_value(&f.settings).unwrap()
}

fn apply(f: &mut Fixture, out: &Outcome) {
    apply_shallow(f, &out.settings, &out.state);
}

fn set_path(f: &mut Fixture, path: &str, value: &Value) {
    let (root, key) = path.split_once('.').unwrap_or((path, ""));
    match root {
        "rotation" => f.rotation = serde_json::from_value(value.clone()).unwrap(),
        "settings" => apply_shallow(f, &Map::from_iter([(key.to_string(), value.clone())]), &Map::new()),
        "state" => apply_shallow(f, &Map::new(), &Map::from_iter([(key.to_string(), value.clone())])),
        _ => panic!("chemin inconnu {path}"),
    }
}

/// Rejoue un scénario ; renvoie les écarts.
fn replay(sc: &Value, rep: &mut Report) {
    let name = sc["name"].as_str().unwrap();
    let family = sc["family"].as_str().unwrap();
    let generic = family == "generic";
    let mut f = fixture_of(&sc["fixture"]);
    let mut others: Vec<Fixture> = sc["others"].as_array().unwrap().iter().map(fixture_of).collect();
    let mut panel = if generic { Panel::generic(&f.type_id, 4, sc["controls"].clone()) } else { Panel::new(&f).unwrap_or_else(|| panic!("{name} : pas de panneau")) };
    let ty = library().fixture(&f.type_id);
    let pixels = if generic { 4 } else { ty.map_or(0, |t| t.pixels) };
    let mut live: HashSet<u32> = sc["live"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect();
    let app = sc["appConnected"].as_bool().unwrap_or(false);
    let mut rts: HashMap<String, (Runtime, AsteraRuntime)> = HashMap::new();
    for x in std::iter::once(&f).chain(&others) {
        rts.insert(x.id.clone(), Default::default());
    }
    let dmx = Universes::new();
    // valeurs attendues (dernier relevé)
    let mut want: BTreeMap<String, Value> = BTreeMap::new();

    for (i, step) in sc["steps"].as_array().unwrap().iter().enumerate() {
        let t = step["t"].as_f64().unwrap();
        let op = step["op"].as_str().unwrap();
        let mut got: BTreeMap<String, Value> = BTreeMap::new();
        let all: Vec<Fixture> = std::iter::once(f.clone()).chain(others.iter().cloned()).collect();
        // ---- geste
        let out = {
            let env = Env { time: t, fixtures: &all, live: &live, app_connected: app };
            let (rt, ext) = rts.get_mut(&f.id).unwrap();
            let mut cx = Ctx { fixture: &f, runtime: rt, astera: ext, env: &env };
            let id = step["id"].as_str().unwrap_or("");
            match op {
                "press" => panel.press(&mut cx, id),
                "release" => panel.release(&mut cx, id),
                "cancel" => {
                    panel.cancel(id);
                    Outcome::default()
                }
                "turn" => panel.turn(&mut cx, id, step["steps"].as_i64().unwrap() as i32),
                "shake" => {
                    panel.shake(&mut cx);
                    Outcome::default()
                }
                "slider" => {
                    let zone = match step["zone"].as_str().unwrap() {
                        "top" => SliderZone::Top,
                        "middle" => SliderZone::Middle,
                        _ => SliderZone::Bottom,
                    };
                    let (ok, out) = panel.slider(&mut cx, zone, step["steps"].as_u64().unwrap() as u32);
                    got.insert("ok".into(), json!(ok));
                    out
                }
                "ir" => {
                    let r = apply_ir_key(&f, ty.unwrap(), cx.runtime, cx.astera, step["key"].as_str().unwrap(), t);
                    got.insert("ir".into(), json!({ "accepted": r.accepted, "note": r.note }));
                    r.changes.unwrap_or_default()
                }
                _ => Outcome::default(),
            }
        };
        apply(&mut f, &out);
        relay(&mut others, &mut rts, &out);
        match op {
            "set" => set_path(&mut f, step["path"].as_str().unwrap(), &step["value"]),
            "live" => live = step["value"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect(),
            _ => {}
        }
        // ---- image : moteur lumière, tick, pixels remplacés
        let ectx = EngineCtx { dmx: &dmx, live: &live, time: t };
        for x in std::iter::once(&f).chain(&others) {
            if let Some(xt) = library().fixture(&x.type_id)
                && xt.pixels > 0
            {
                let mut buf = vec![0.0; xt.pixels * 3];
                compute_output(x, xt, &ectx, &mut buf, &mut rts.get_mut(&x.id).unwrap().0);
            }
        }
        let all: Vec<Fixture> = std::iter::once(f.clone()).chain(others.iter().cloned()).collect();
        let env = Env { time: t, fixtures: &all, live: &live, app_connected: app };
        let out = {
            let (rt, ext) = rts.get_mut(&f.id).unwrap();
            panel.tick(&mut Ctx { fixture: &f, runtime: rt, astera: ext, env: &env })
        };
        apply(&mut f, &out);
        relay(&mut others, &mut rts, &out);
        let all: Vec<Fixture> = std::iter::once(f.clone()).chain(others.iter().cloned()).collect();
        let env = Env { time: t, fixtures: &all, live: &live, app_connected: app };
        let (rt, ext) = rts.get_mut(&f.id).unwrap();
        let mut cx = Ctx { fixture: &f, runtime: rt, astera: ext, env: &env };
        if pixels > 0 {
            let mut buf = vec![0.0; pixels * 3];
            let on = panel.override_pixels(&mut cx, &mut buf, pixels);
            got.insert("px".into(), if on { json!(buf) } else { Value::Null });
        }
        got.insert("d".into(), screen_json(&panel.display(&mut cx), generic));
        got.insert("l".into(), leds_json(&panel.leds(&mut cx)));
        got.insert("h".into(), json!(panel.hint(&mut cx)));
        got.insert("s".into(), settings_json(&f));
        got.insert("st".into(), f.state.clone());
        got.insert("latch".into(), json!(cx.astera.latch_name(cx.runtime)));
        got.insert("ju".into(), json!(cx.astera.just_unlinked));
        if !others.is_empty() {
            let o: Map<String, Value> = others
                .iter()
                .map(|x| {
                    let (r, e) = &rts[&x.id];
                    let s = settings_json(x);
                    (x.id.clone(), json!({ "crmxLinked": s.get("crmxLinked"), "universe": s.get("universe"), "latch": e.latch_name(r), "ju": e.just_unlinked }))
                })
                .collect();
            got.insert("o".into(), Value::Object(o));
        }
        let screen = panel.screen_name();
        if !screen.is_empty() {
            got.insert("screen".into(), json!(screen));
        }
        // ---- comparaison avec le dernier relevé JavaScript
        if let Some(e) = step["e"].as_object() {
            for (k, v) in e {
                if k == "dp" {
                    // écran dessiné : primitives changées depuis le relevé précédent
                    let d = want.get_mut("d").expect("écran avant ses changements");
                    let n = v["n"].as_u64().unwrap() as usize;
                    let prims = d["prims"].as_array_mut().unwrap();
                    prims.resize(n, Value::Null);
                    for (i, p) in v["p"].as_object().unwrap() {
                        prims[i.parse::<usize>().unwrap()] = p.clone();
                    }
                    for f in ["off", "dim"] {
                        if let Some(x) = v.get(f) {
                            d[f] = x.clone();
                        }
                    }
                } else if k == "s" || k == "st" {
                    // réglages et état : champs changés depuis le relevé précédent
                    let cur = want.entry(k.clone()).or_insert_with(|| json!({}));
                    for (f, x) in v.as_object().into_iter().flatten() {
                        cur[f] = x.clone();
                    }
                } else {
                    want.insert(k.clone(), v.clone());
                }
            }
        }
        for (k, w) in &want {
            if (k == "ok" || k == "ir") && !step["e"].as_object().is_some_and(|e| e.contains_key(k)) {
                continue;
            }
            let g = got.get(k).cloned().unwrap_or(Value::Null);
            rep.checked += 1;
            let eps = if k == "px" { 2e-6 } else { 1e-9 };
            if let Some(d) = diff(&g, w, k, eps) {
                rep.fails.push(format!("« {name} », pas {i} ({op} {} t={t}) : {d}", step["id"].as_str().or(step["key"].as_str()).unwrap_or("")));
            }
        }
    }
}

/// Changements demandés aux autres projecteurs (AsteraBox).
fn relay(others: &mut [Fixture], rts: &mut HashMap<String, (Runtime, AsteraRuntime)>, out: &Outcome) {
    for r in &out.others {
        if let Some(x) = others.iter_mut().find(|x| x.id == r.id) {
            apply_shallow(x, &r.settings, &Map::new());
        }
        if r.relatch
            && let Some((rt, ext)) = rts.get_mut(&r.id)
        {
            rt.latch = None;
            ext.remote = false;
            ext.just_unlinked = r.unlinked;
        }
    }
}

fn family(name: &str) {
    let d = data();
    let mut rep = Report::default();
    let list: Vec<&Value> = d["scenarios"].as_array().unwrap().iter().filter(|s| s["family"] == name).collect();
    assert!(!list.is_empty(), "aucun scénario {name}");
    for sc in list {
        replay(sc, &mut rep);
    }
    rep.finish(name);
}

#[test]
fn astera_v5_comme_la_version_javascript() {
    family("v5");
}

#[test]
fn astera_bouton_unique_comme_la_version_javascript() {
    family("ax1");
}

#[test]
fn astera_box_comme_la_version_javascript() {
    family("box");
}

#[test]
fn commandes_generiques_comme_la_version_javascript() {
    family("generic");
}

#[test]
fn panneaux_des_marques_comme_la_version_javascript() {
    family("panel");
}

#[test]
fn faces_avant_des_marques_comme_la_version_javascript() {
    let d = data();
    let js: Vec<&str> = d["panelTypes"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(js, PANEL_TYPES.to_vec(), "modèles qui ont un panneau");
    let mut rep = Report::default();
    for f in d["faces"].as_array().unwrap() {
        let id = f["type"].as_str().unwrap();
        let ty = library().fixture(id).unwrap_or_else(|| panic!("{id} absent de la bibliothèque"));
        let panel = Panel::for_type(ty).unwrap_or_else(|| panic!("{id} : pas de panneau"));
        assert_eq!(panel.kind(), PanelKind::Brand(Box::leak(f["id"].as_str().unwrap().to_string().into_boxed_str())), "{id} : définition");
        let mut got = serde_json::to_value(panel.face()).unwrap();
        if let Some(d) = got.get_mut("display").and_then(Value::as_object_mut) {
            d.remove("kind");
        }
        rep.checked += 1;
        if let Some(e) = diff(&got, &f["face"], "face", 1e-9) {
            rep.fails.push(format!("{id} : {e}"));
        }
    }
    rep.finish("faces");
}
