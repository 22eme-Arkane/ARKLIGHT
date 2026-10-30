//! Parité avec la version JavaScript : mêmes entrées, mêmes résultats (données calculées par
//! `node tools/rust/export-midi-data.mjs`).

use arklight_core::Fixture;
use arklight_core::project::Group;
use arklight_midi::controller::{Action, ColorChange, DeviceInfo, EnvChange, Learning, Midi, MidiContext, WizardKind};
use arklight_midi::message::{self, Held, Mapping, RouteEvent, Source, TARGET_KINDS, Target, clean_mappings, describe_source, note_name, parse_midi, route_midi, scale_value, target_groups};
use arklight_midi::presets::{MIDI_PRESETS, preset_label, preset_mappings};
use serde_json::{Value, json};
use std::collections::HashSet;

fn data() -> Value {
    let text = std::fs::read_to_string(format!("{}/tests/data/midi.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Égalité de deux documents JSON (nombres à 1e-12 près, relatif) ; premier écart sinon.
fn diff(a: &Value, b: &Value, path: &str) -> Option<String> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
            ((x - y).abs() > 1e-12 * x.abs().max(1.0)).then(|| format!("{path} : {x} ≠ {y}"))
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path} : {} éléments ≠ {} — {a} ≠ {b}", x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().find_map(|(i, (p, q))| diff(p, q, &format!("{path}[{i}]")))
        }
        (Value::Object(x), Value::Object(y)) => {
            let keys: HashSet<&String> = x.keys().chain(y.keys()).collect();
            let mut keys: Vec<&String> = keys.into_iter().collect();
            keys.sort();
            keys.into_iter().find_map(|k| diff(x.get(k).unwrap_or(&Value::Null), y.get(k).unwrap_or(&Value::Null), &format!("{path}.{k}")))
        }
        _ => (a != b).then(|| format!("{path} : {a} ≠ {b}")),
    }
}

fn same(rust: &Value, js: &Value, what: &str) {
    if let Some(d) = diff(rust, js, "") {
        panic!("{what} : {d}\n  Rust       : {rust}\n  JavaScript : {js}");
    }
}

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u8).collect()
}

/// Correspondance sans identifiant (commande, fonction, repère).
fn bare(m: &Mapping) -> Value {
    let mut v = m.to_json();
    let o = v.as_object_mut().unwrap();
    o.remove("id");
    o.remove("preset");
    v
}

fn bares(l: &[Mapping]) -> Value {
    Value::Array(l.iter().map(bare).collect())
}

/// Correspondance écrite telle quelle (sans nettoyage).
fn raw_mapping(v: &Value) -> Mapping {
    Mapping {
        id: v["id"].as_str().unwrap_or_default().to_owned(),
        source: serde_json::from_value::<Option<Source>>(v["source"].clone()).unwrap(),
        target: Target::from_json(v["target"].as_object().unwrap()),
        label: v["label"].as_str().map(str::to_owned),
        preset: v["preset"] == true,
    }
}

#[test]
fn fonctions_et_rubriques() {
    let v = data();
    let kinds: Vec<Value> = TARGET_KINDS.iter().map(|k| json!([k.id, k.group, k.label, k.mode.name()])).collect();
    same(&Value::Array(kinds), &v["kinds"]["kinds"], "fonctions");
    same(&json!(target_groups()), &v["kinds"]["groups"], "rubriques");
}

#[test]
fn messages_decodes() {
    let v = data();
    for p in v["parse"].as_array().unwrap() {
        let data = bytes(&p["data"]);
        let msg = parse_midi(&data);
        let got = msg.map_or(Value::Null, |m| json!({ "type": m.kind.name(), "channel": m.channel, "number": m.number, "value": m.value, "on": m.on }));
        same(&got, &p["msg"], &format!("parseMidi({data:?})"));
        same(&json!(msg.map(|m| m.key())), &p["key"], "clé");
        same(&json!(msg.map(|m| describe_source(&m.source()))), &p["text"], "description");
    }
    let notes: Vec<String> = (0..128).map(note_name).collect();
    same(&json!(notes), &v["notes"], "noms des notes");
}

#[test]
fn echelles_des_faders() {
    let v = data();
    for s in v["scale"].as_array().unwrap() {
        let kind = s["kind"].as_str().unwrap();
        for pair in s["values"].as_array().unwrap() {
            let x = pair[0].as_f64().unwrap();
            assert_eq!(scale_value(kind, x), pair[1].as_f64().unwrap(), "scaleValue({kind}, {x})");
        }
    }
}

#[test]
fn preréglages_identiques() {
    let v = data();
    let presets = v["presets"].as_array().unwrap();
    assert_eq!(presets.len(), MIDI_PRESETS.len());
    for (p, js) in MIDI_PRESETS.iter().zip(presets) {
        assert_eq!(p.id, js["id"]);
        assert_eq!(preset_label(p), js["label"]);
        same(&json!(p.summary), &js["summary"], "résumé");
        assert_eq!(p.source, js["source"]);
        assert_eq!(p.channel, js["channel"].as_i64().unwrap() as i32);
        for m in js["matches"].as_array().unwrap() {
            assert_eq!(p.matches(m[0].as_str().unwrap()), m[1].as_bool().unwrap(), "{} reconnaît {}", p.id, m[0]);
        }
        for c in js["mappings"].as_array().unwrap() {
            let channel = c["channel"].as_i64().map(|x| x as i32);
            let got: Vec<Value> = preset_mappings(p, channel).iter().map(Mapping::to_json).collect();
            same(&Value::Array(got), &c["mappings"], &format!("{} canal {channel:?}", p.id));
        }
    }
}

#[test]
fn correspondances_lues() {
    let v = data();
    for c in v["clean"].as_array().unwrap() {
        let input = &c["input"];
        let known: HashSet<&str> = input.as_array().into_iter().flatten().filter_map(|x| x.get("id").and_then(Value::as_str)).collect();
        let got: Vec<Value> = clean_mappings(input)
            .iter()
            .map(|m| {
                let mut j = m.to_json();
                if !known.contains(m.id.as_str()) {
                    j["id"] = Value::Null;
                }
                j
            })
            .collect();
        same(&Value::Array(got), &c["output"], &format!("cleanMappings({input})"));
    }
}

#[test]
fn aiguillage_identique() {
    let v = data();
    let maps: Vec<Mapping> = v["route"]["mappings"].as_array().unwrap().iter().map(raw_mapping).collect();
    let mut held = Held::new();
    for s in v["route"]["steps"].as_array().unwrap() {
        let data = bytes(&s["data"]);
        let routes = parse_midi(&data).map_or(Vec::new(), |m| route_midi(&maps, &m, &mut held));
        let got: Vec<Value> = routes
            .iter()
            .map(|r| {
                let (value, press, release, latch) = match r.event {
                    RouteEvent::Value(x) => (json!(x), false, false, Value::Null),
                    RouteEvent::Press => (Value::Null, true, false, Value::Null),
                    RouteEvent::Release => (Value::Null, false, true, Value::Null),
                    RouteEvent::Latch(b) => (Value::Null, false, false, json!(b)),
                };
                json!({ "id": r.mapping.id, "value": value, "press": press, "release": release, "latch": latch })
            })
            .collect();
        same(&Value::Array(got), &s["routes"], &format!("routeMidi({data:?})"));
    }
}

// ------------------------------------------------------------------ contrôleur complet

/// Le projet et la sélection, comme le `Store` de l'interface.
struct Scene {
    fixtures: Vec<Fixture>,
    groups: Vec<Group>,
    order: Vec<String>,
    plans: Vec<String>,
    fader_count: usize,
    selection: Vec<String>,
    primary: Option<String>,
}

impl Scene {
    fn ctx(&self) -> MidiContext<'_> {
        MidiContext { fixtures: &self.fixtures, groups: &self.groups, fixture_order: Some(&self.order), selection: &self.selection, selected_id: self.primary.as_deref(), plans: &self.plans, fader_count: self.fader_count }
    }

    /// `Store.setSelection` : vrai si la sélection change.
    fn set_selection(&mut self, ids: &[String], primary: Option<String>) -> bool {
        let mut next: Vec<String> = Vec::new();
        for id in ids {
            if self.fixtures.iter().any(|f| f.id == *id) && !next.contains(id) {
                next.push(id.clone());
            }
        }
        let p = primary.filter(|p| next.contains(p)).or_else(|| next.last().cloned());
        if p == self.primary && next.len() == self.selection.len() && next.iter().all(|id| self.selection.contains(id)) {
            return false;
        }
        self.selection = next;
        self.primary = p;
        true
    }
}

fn log_of(a: &Action) -> Option<Value> {
    Some(match a {
        Action::Select { ids, primary } => json!(["selection", ids, primary]),
        Action::Blackout(on) => json!(["blackout", on]),
        Action::Fader { index, value } => json!(["setFader", index, value]),
        Action::FaderMarks { marks, page } => json!(["setMidiFaders", marks.iter().map(|m| json!({ "index": m.index, "label": m.label })).collect::<Vec<_>>(), page]),
        Action::NextFamily => json!(["nextFamily"]),
        Action::Flash { fixtures, on } => json!(["flash", fixtures, on]),
        Action::Intensity { fixtures, value } => json!(["setIntensity", fixtures, value]),
        Action::Color { fixtures, change } => {
            let c = match change {
                ColorChange::Cct(v) => json!({ "cct": v }),
                ColorChange::Tint(v) => json!({ "tint": v }),
                ColorChange::Hue(v) => json!({ "hue": v }),
                ColorChange::Sat(v) => json!({ "sat": v }),
            };
            json!(["setColor", fixtures, c])
        }
        Action::OpenPlan(id) => json!(["openPlan", id]),
        Action::Env(e) => json!(["setEnv", match e {
            EnvChange::Haze(v) => json!({ "haze": v }),
            EnvChange::Exposure(v) => json!({ "exposure": v }),
            EnvChange::Ambient(v) => json!({ "ambient": v }),
        }]),
        Action::Command(id) => json!(["run", id]),
        Action::SettingsChanged => return None,
    })
}

fn marks(midi: &Midi) -> Value {
    log_of(&Action::FaderMarks { marks: midi.fader_marks(), page: midi.page }).unwrap()
}

fn preset_name(midi: &Midi, id: &str) -> String {
    if id.starts_with("user:") { format!("user#{}", midi.user_presets().iter().position(|p| p.id == id).map_or(-1, |i| i as i64)) } else { id.to_owned() }
}

fn state(midi: &Midi, scene: &Scene) -> Value {
    json!({
        "page": midi.page,
        "mappings": bares(midi.mappings()),
        "preset": midi.preset().map(|p| json!({ "id": preset_name(midi, &p.id), "channel": p.channel })),
        "userPresets": midi.user_presets().iter().map(|p| json!({ "name": p.name, "mappings": bares(&p.mappings) })).collect::<Vec<_>>(),
        "layer": midi.layer().len(),
        "all": bares(midi.all()),
        "suggest": midi.suggest,
        "faderIndexes": midi.fader_indexes(),
        "last": midi.last_text(),
        "learning": midi.learning_status(),
        "learningOn": midi.learning().is_some(),
        "pageInfo": midi.page_info(),
        "selection": scene.selection,
        "selectedId": scene.primary,
    })
}

#[test]
fn controleur_identique_a_la_version_javascript() {
    let v = data();
    let sc = &v["scenario"];
    let mut scene = Scene {
        fixtures: serde_json::from_value(sc["scene"]["fixtures"].clone()).unwrap(),
        groups: serde_json::from_value(sc["scene"]["groups"].clone()).unwrap(),
        order: serde_json::from_value(sc["fixtureOrder"].clone()).unwrap(),
        plans: serde_json::from_value(sc["plans"].clone()).unwrap(),
        fader_count: sc["faderCount"].as_u64().unwrap() as usize,
        selection: Vec::new(),
        primary: None,
    };
    let mut midi = Midi::new();
    let mut learned = 0;
    for (n, step) in sc["steps"].as_array().unwrap().iter().enumerate() {
        let (op, args) = (step["op"].as_str().unwrap(), &step["args"]);
        let what = format!("étape {n} ({op} {args})");
        let own_id = |midi: &Midi| midi.mappings()[args["index"].as_u64().unwrap() as usize].id.clone();
        let user_id = |midi: &Midi| midi.user_presets()[args["userIndex"].as_u64().unwrap() as usize].id.clone();
        let mut log: Vec<Value> = Vec::new();
        let mut result = Value::Null;
        match op {
            "init" => log.push(marks(&midi)),
            "inputs" => {
                let list = args["list"].as_array().unwrap().iter().enumerate().map(|(i, d)| DeviceInfo { id: format!("in{i}"), name: d["name"].as_str().unwrap().into(), manufacturer: d["manufacturer"].as_str().unwrap().into() }).collect();
                midi.set_inputs(list);
            }
            "setPreset" => {
                let id = if args.get("userIndex").is_some() { Some(user_id(&midi)) } else { args["id"].as_str().map(str::to_owned) };
                midi.set_preset(id.as_deref(), args["channel"].as_i64().map(|c| c as i32));
                log.push(marks(&midi));
            }
            "receive" => {
                let data = bytes(&args["data"]);
                let actions = midi.receive(&data, args["device"].as_str().unwrap(), &scene.ctx());
                for a in &actions {
                    if let Action::Select { ids, primary } = a {
                        assert!(scene.set_selection(ids, primary.clone()), "{what} : sélection inchangée");
                    }
                    if *a == Action::SettingsChanged {
                        learned += 1;
                    }
                }
                log.extend(actions.iter().filter_map(log_of));
            }
            "add" => {
                let source = serde_json::from_value::<Option<Source>>(args["source"].clone()).unwrap();
                let id = midi.add(source, Target::from_json(args["target"].as_object().unwrap()));
                if let Some(l) = args["label"].as_str() {
                    midi.update(&id, |m| m.label = Some(l.to_owned()));
                }
                log.push(marks(&midi));
            }
            "select" => {
                let id = args["id"].as_str().map(str::to_owned);
                if scene.set_selection(&id.clone().into_iter().collect::<Vec<_>>(), id) {
                    log.push(json!(["selection", scene.selection, scene.primary]));
                    midi.selection_changed();
                    log.push(marks(&midi));
                }
            }
            "learnNew" => midi.learn_new(),
            "relearn" => {
                let id = own_id(&midi);
                midi.relearn(&id);
            }
            "wizard" => {
                let kind = match args["kind"].as_str().unwrap() {
                    "faders" => WizardKind::Faders,
                    "groups" => WizardKind::Groups,
                    _ => WizardKind::Fixtures,
                };
                midi.toggle_wizard(kind, &scene.ctx());
            }
            "stopLearning" => midi.stop_learning(),
            "update" => {
                let id = own_id(&midi);
                if let Some(t) = args["target"].as_object() {
                    midi.update(&id, |m| m.target = Target::from_json(t));
                }
                if let Some(l) = args["label"].as_str() {
                    midi.update(&id, |m| m.label = Some(l.to_owned()));
                }
                log.push(marks(&midi));
            }
            "remove" => {
                let id = own_id(&midi);
                midi.remove(&id);
                log.push(marks(&midi));
            }
            "exportPreset" => result = midi.export_preset(args["name"].as_str().unwrap()),
            "saveUserPreset" => {
                midi.save_user_preset(args["name"].as_str().unwrap());
                log.push(marks(&midi));
            }
            "importPreset" => match midi.import_preset(&args["json"]) {
                Ok(name) => {
                    result = json!(name);
                    log.push(marks(&midi));
                }
                Err(e) => result = json!({ "error": e }),
            },
            "deleteUserPreset" => {
                let id = user_id(&midi);
                midi.delete_user_preset(&id);
                log.push(marks(&midi));
            }
            "clear" => {
                midi.clear();
                log.push(marks(&midi));
            }
            other => panic!("étape inconnue : {other}"),
        }
        same(&Value::Array(log), &step["log"], &format!("{what} : appels à l'application"));
        same(&result, &step["result"], &format!("{what} : résultat"));
        let mut st = state(&midi, &scene);
        // listes notées par la version JavaScript seulement quand elles changent
        for key in ["mappings", "userPresets", "all"] {
            if step["state"][key].is_null() {
                st[key] = Value::Null;
            }
        }
        same(&st, &step["state"], &format!("{what} : état"));
    }
    assert!(learned >= 20, "commandes apprises : {learned}");
    assert!(midi.learning().is_none() || matches!(midi.learning(), Some(Learning::New)));
}

#[test]
fn reglages_relus() {
    let v = data();
    let js = &v["scenario"]["settings"];
    let midi = Midi::from_settings(js);
    let mut got = midi.settings();
    let mut want = js.clone();
    // préréglages personnels : les correspondances enregistrées sans identifiant en reçoivent un
    for doc in [&mut got, &mut want] {
        for p in doc["userPresets"].as_array_mut().unwrap() {
            for m in p["mappings"].as_array_mut().unwrap() {
                m.as_object_mut().unwrap().remove("id");
            }
        }
    }
    same(&got, &want, "réglages relus puis réécrits");
    // réglages abîmés : rien ne casse
    let bad = Midi::from_settings(&json!({ "mappings": "x", "preset": 5, "userPresets": [null, { "name": 3 }, { "name": "ok", "mappings": [] }] }));
    assert!(bad.mappings().is_empty() && bad.preset().is_none());
    assert_eq!(bad.user_presets().len(), 1);
    assert_eq!(bad.user_presets()[0].id, "user:ok");
}

#[test]
fn autre_sorte_de_fonction() {
    let v = data();
    let sc = &v["scenario"];
    let fixtures: Vec<Fixture> = serde_json::from_value(sc["scene"]["fixtures"].clone()).unwrap();
    let groups: Vec<Group> = serde_json::from_value(sc["scene"]["groups"].clone()).unwrap();
    for r in v["retarget"].as_array().unwrap() {
        let selected = r["selectedId"].as_str();
        let ctx = MidiContext { fixtures: &fixtures, groups: &groups, fixture_order: None, selection: &[], selected_id: selected, plans: &[], fader_count: 0 };
        let previous = Target::from_json(r["previous"].as_object().unwrap());
        let got = Midi::target_for_kind(r["kind"].as_str().unwrap(), &previous, &ctx);
        same(&got.to_json(), &r["target"], &format!("{} après {}", r["kind"], r["previous"]));
    }
}

#[test]
fn entrees_sans_appareil() {
    // aucun appareil réel n'est ouvert : messages ajoutés à la main, décodage des messages courts WinMM
    let inputs = arklight_midi::MidiInputs::new();
    inputs.inject("essai", &[0xb0, 7, 64]);
    let got = inputs.poll();
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].device.as_str(), got[0].data.as_slice()), ("essai", &[0xb0u8, 7, 64][..]));
    assert!(inputs.poll().is_empty());
    use arklight_midi::input::unpack_short_message as unpack;
    assert_eq!(unpack(0x0040_07b0), vec![0xb0, 7, 0x40]);
    assert_eq!(unpack(0x0000_05c2), vec![0xc2, 5]);
    assert_eq!(unpack(0x0000_00f8), vec![0xf8]);
    assert_eq!(unpack(0x0012_34e0), vec![0xe0, 0x34, 0x12]);
    assert_eq!(message::MsgType::parse("cc"), Some(message::MsgType::Cc));
}
