//! Parité avec la version JavaScript (`server/ai/tools.js`) : définitions des outils, catalogue du
//! prompt système, orientations (three.js), et une suite d'appels d'outils sur le projet d'exemple
//! « demo-night » (résultat de chaque appel, état du plan après). Données écrites par
//! `node tools/rust/export-ai-data.mjs`.

use arklight_ai::tools::{aim_rotation, catalog_text, tool_defs};
use arklight_ai::{NoHooks, ToolRunner};
use arklight_core::Project;
use arklight_core::document::Document;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::path::PathBuf;

fn data(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} : {e}", p.display()))
}

fn demo() -> Document {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("arklight-core").join("tests").join("data").join("demo-night.json");
    let text = std::fs::read_to_string(&p).expect("demo-night.json");
    Document::new(Project::from_json(&text).expect("projet"), None)
}

/// Retire les valeurs null (la version JavaScript écrit `fixtureNumber: null`, la version Rust
/// omet le champ).
fn strip_nulls(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(m.iter().filter(|(_, x)| !x.is_null()).map(|(k, x)| (k.clone(), strip_nulls(x))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(strip_nulls).collect()),
        other => other.clone(),
    }
}

/// Compare deux valeurs JSON (nombres à 1e-9 près) ; les différences vont dans `out`.
fn diff(path: &str, js: &Value, rs: &Value, out: &mut Vec<String>) {
    match (js, rs) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap_or(f64::NAN), b.as_f64().unwrap_or(f64::NAN));
            if (a - b).abs() > 1e-9 {
                out.push(format!("{path} : JS {a}, Rust {b}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            for (k, x) in a {
                match b.get(k) {
                    Some(y) => diff(&format!("{path}.{k}"), x, y, out),
                    None => out.push(format!("{path}.{k} : absent en Rust (JS {x})")),
                }
            }
            for k in b.keys().filter(|k| !a.contains_key(*k)) {
                out.push(format!("{path}.{k} : absent en JS (Rust {})", b[k]));
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                out.push(format!("{path} : {} éléments en JS, {} en Rust", a.len(), b.len()));
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                diff(&format!("{path}[{i}]"), x, y, out);
            }
        }
        (a, b) if a == b => {}
        (a, b) => out.push(format!("{path} : JS {a}, Rust {b}")),
    }
}

/// Les ids créés pendant la suite deviennent « #new ».
fn normalize(v: &Value, new_ids: &HashSet<String>) -> Value {
    match v {
        Value::String(s) if new_ids.contains(s) => Value::String("#new".into()),
        Value::Object(m) => Value::Object(m.iter().map(|(k, x)| (k.clone(), normalize(x, new_ids))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(|x| normalize(x, new_ids)).collect()),
        other => other.clone(),
    }
}

fn all_ids(doc: &Document) -> HashSet<String> {
    let p = &doc.project;
    p.fixtures.iter().map(|f| f.id.clone()).chain(p.objects.iter().map(|o| o.id.clone())).chain(p.groups.iter().map(|g| g.id.clone())).collect()
}

fn snapshot(doc: &Document) -> Value {
    let p = &doc.project;
    let pick = |v: Value, keys: &[&str]| {
        let m: Map<String, Value> = keys.iter().filter_map(|k| v.get(*k).map(|x| ((*k).to_owned(), x.clone()))).collect();
        Value::Object(m)
    };
    let fixtures: Vec<Value> = p.fixtures.iter().map(|f| pick(serde_json::to_value(f).unwrap(), &["id", "type", "name", "position", "rotation", "settings", "state"])).collect();
    let objects: Vec<Value> = p.objects.iter().map(|o| pick(serde_json::to_value(o).unwrap(), &["id", "type", "name", "position", "rotation", "props"])).collect();
    let groups: Vec<Value> = p.groups.iter().map(|g| pick(serde_json::to_value(g).unwrap(), &["id", "name", "ids"])).collect();
    serde_json::json!({ "fixtures": fixtures, "objects": objects, "groups": groups, "environment": serde_json::to_value(&p.environment).unwrap() })
}

#[test]
fn definitions_des_outils() {
    let js: Value = serde_json::from_str(&data("tool-defs.json")).unwrap();
    let rs = Value::Array(tool_defs().to_vec());
    let mut out = Vec::new();
    diff("TOOL_DEFS", &js, &rs, &mut out);
    assert!(out.is_empty(), "{}", out.join("\n"));
}

#[test]
fn catalogue() {
    assert_eq!(catalog_text(), data("catalog.txt"));
}

#[test]
fn orientations() {
    let list: Vec<Value> = serde_json::from_str(&data("aim.json")).unwrap();
    let v3 = |v: &Value| -> [f64; 3] { serde_json::from_value(v.clone()).unwrap() };
    let mut bad = Vec::new();
    for a in &list {
        let (from, to, js) = (v3(&a["from"]), v3(&a["to"]), v3(&a["rotation"]));
        let rs = aim_rotation(from, to);
        if (0..3).any(|i| (rs[i] - js[i]).abs() > 1e-9) {
            bad.push(format!("{from:?} → {to:?} : JS {js:?}, Rust {rs:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn suite_d_appels_d_outils() {
    let runs: Value = serde_json::from_str(&data("tool-runs.json")).unwrap();
    let mut doc = demo();
    let original = all_ids(&doc);
    let mut runner = ToolRunner::new(runs["imageAspect"].as_f64());
    let mut problems = Vec::new();
    for (i, step) in runs["steps"].as_array().unwrap().iter().enumerate() {
        let name = step["name"].as_str().unwrap();
        let result = runner.run(&mut doc, &mut NoHooks, name, &step["args"]);
        let new_ids: HashSet<String> = all_ids(&doc).difference(&original).cloned().collect();
        let mut out = Vec::new();
        diff("résultat", &strip_nulls(&step["result"]), &strip_nulls(&normalize(&result, &new_ids)), &mut out);
        diff("après", &strip_nulls(&step["after"]), &strip_nulls(&normalize(&snapshot(&doc), &new_ids)), &mut out);
        if !out.is_empty() {
            problems.push(format!("appel {i} ({name}) :\n  {}", out.into_iter().take(25).collect::<Vec<_>>().join("\n  ")));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
