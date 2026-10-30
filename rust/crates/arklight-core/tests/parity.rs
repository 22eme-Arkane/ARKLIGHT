//! Parité avec la version JavaScript : mêmes entrées, mêmes résultats (données calculées par
//! `node tools/rust/export-test-data.mjs`).

use arklight_core::color::{cct_to_rgb, cct_to_xy, hs_to_rgb, linear_to_srgb, srgb_to_linear};
use arklight_core::engine::{Ctx, Lights, Universes};
use arklight_core::{Project, library};
use std::collections::HashSet;
use serde_json::Value;

const EPS: f64 = 1e-9;

fn data(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn arr3(v: &Value) -> [f64; 3] {
    let a = v.as_array().unwrap();
    [a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), a[2].as_f64().unwrap()]
}

fn close(a: [f64; 3], b: [f64; 3], what: &str) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < EPS, "{what} : {a:?} ≠ {b:?} (JavaScript)");
    }
}

#[test]
fn couleurs_identiques_a_la_version_javascript() {
    let v: Value = serde_json::from_str(&data("color.json")).unwrap();
    for c in v["cct"].as_array().unwrap() {
        let (k, t) = (c["k"].as_f64().unwrap(), c["t"].as_f64().unwrap());
        close(cct_to_rgb(k, t), arr3(&c["rgb"]), &format!("cct_to_rgb({k}, {t})"));
    }
    for c in v["xy"].as_array().unwrap() {
        let k = c["k"].as_f64().unwrap();
        let (x, y) = cct_to_xy(k);
        let xy = c["xy"].as_array().unwrap();
        assert!((x - xy[0].as_f64().unwrap()).abs() < EPS && (y - xy[1].as_f64().unwrap()).abs() < EPS, "cct_to_xy({k})");
    }
    let white = cct_to_rgb(5600.0, 0.0);
    for c in v["hs"].as_array().unwrap() {
        let (h, s) = (c["h"].as_f64().unwrap(), c["s"].as_f64().unwrap());
        close(hs_to_rgb(h, s, white), arr3(&c["rgb"]), &format!("hs_to_rgb({h}, {s})"));
    }
    for c in v["srgb"].as_array().unwrap() {
        let x = c["c"].as_f64().unwrap();
        assert!((srgb_to_linear(x) - c["lin"].as_f64().unwrap()).abs() < EPS);
        assert!((linear_to_srgb(x) - c["back"].as_f64().unwrap()).abs() < EPS);
    }
}

#[test]
fn projets_d_exemple_lus_et_reecrits_sans_perte() {
    for name in ["demo-night.json", "demo-clip.json"] {
        let text = data(name);
        let p = Project::from_json(&text).unwrap();
        assert!(!p.fixtures.is_empty(), "{name} : projecteurs");
        assert!(!p.objects.is_empty(), "{name} : décor");
        // chaque modèle est connu de la bibliothèque
        for f in &p.fixtures {
            assert!(library().fixture(&f.type_id).is_some(), "{name} : modèle inconnu {}", f.type_id);
        }
        // réécrit puis relu : tout le contenu d'origine est là (sauf la date d'enregistrement)
        let mut before: Value = serde_json::from_str(&text).unwrap();
        let mut after: Value = serde_json::from_str(&p.to_json().unwrap()).unwrap();
        for v in [&mut before, &mut after] {
            v.as_object_mut().unwrap().remove("savedAt");
        }
        if let Some(d) = first_diff(&before, &after, String::new()) {
            panic!("{name} : contenu perdu ou modifié à l'enregistrement — {d}");
        }
    }
}

/// Premier écart entre deux documents JSON : « chemin : avant → après ».
fn first_diff(a: &Value, b: &Value, path: String) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                let (va, vb) = (x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null));
                if x.contains_key(k) != y.contains_key(k) {
                    return Some(format!("{path}.{k} : {va} → {vb}"));
                }
                if let Some(d) = first_diff(va, vb, format!("{path}.{k}")) {
                    return Some(d);
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => x.iter().zip(y).enumerate().find_map(|(i, (va, vb))| first_diff(va, vb, format!("{path}[{i}]"))),
        // nombres : 1 et 1.0 sont le même nombre
        (Value::Number(x), Value::Number(y)) if x.as_f64() == y.as_f64() => None,
        _ if a == b => None,
        _ => Some(format!("{path} : {a} → {b}")),
    }
}

#[test]
fn bibliotheque_complete() {
    let lib = library();
    assert!(lib.fixtures.len() >= 70);
    assert!(lib.props.len() >= 90);
    assert!(lib.grip.len() >= 20);
    let titan = lib.fixture("astera.titan").unwrap();
    assert_eq!(titan.geometry.kind, "tube");
    assert!(titan.dmx.modes.len() > 10);
    assert!(lib.prop("person-man").is_some());
}

#[test]
fn projecteurs_allumes() {
    let p = Project::from_json(&data("demo-night.json")).unwrap();
    let (dmx, live) = (Universes::new(), HashSet::new());
    let mut lights = Lights::default();
    lights.update(&p.fixtures, &Ctx { dmx: &dmx, live: &live, time: 0.0 });
    let lit = p.fixtures.iter().filter_map(|f| lights.get(&f.id)).filter(|l| l.pixels.iter().any(|v| *v > 0.0)).count();
    assert!(lit > 0, "des projecteurs allumés dans l'exemple de nuit");
}
