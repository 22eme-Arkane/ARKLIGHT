//! Parité des outils du plan avec la version JavaScript : les mêmes gestes (souris, clavier,
//! réglages de l'élément sélectionné, menu contextuel) donnent le même état de la vue après chacun
//! — outil, indication, sélection, tracé en cours, poignées, cadre de sélection, éléments du plan,
//! image de fond, projecteurs et objets (données de `node tools/rust/export-plot-editor-data.mjs`
//! dans `tests/data/`).

use arklight_core::Project;
use arklight_core::document::Edit;
use arklight_plot::*;
use serde_json::{Value, json};
use std::collections::HashMap;

fn data(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{name} : {e}"))
}

/// Même nombre (le JSON relu peut différer d'une unité sur le dernier chiffre).
fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// Même JSON, aux arrondis de lecture des nombres près.
fn same_json(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => close(x.as_f64().unwrap(), y.as_f64().unwrap()),
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| same_json(a, b)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same_json(v, w))),
        _ => a == b,
    }
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("nombre attendu : {v}"))
}

/// Projecteurs et objets déplacés ou tournés par les outils.
fn apply_doc(p: &mut Project, e: Edit) {
    match e {
        Edit::UpdateFixtures(list) => {
            for (id, changes) in list {
                apply_doc(p, Edit::UpdateFixture { id, changes });
            }
        }
        Edit::UpdateFixture { id, changes } => {
            let fx = p.fixtures.iter_mut().find(|x| x.id == id).unwrap();
            fx.position = changes.position.unwrap_or(fx.position);
            fx.rotation = changes.rotation.unwrap_or(fx.rotation);
        }
        Edit::UpdateObject { id, changes } => {
            let o = p.objects.iter_mut().find(|x| x.id == id).unwrap();
            o.position = changes.position.unwrap_or(o.position);
            o.rotation = changes.rotation.unwrap_or(o.rotation);
        }
        other => panic!("modification inattendue : {other:?}"),
    }
}

struct Run {
    project: Project,
    sel: Selection,
    options: PlotOptions,
    editor: Editor,
    view: View,
    /// éléments créés : identifiant Rust → identifiant JavaScript
    ids: HashMap<String, String>,
    /// identifiants du projet de départ
    original: Vec<String>,
    /// derniers éléments, image de fond, projecteurs et objets notés par la version JavaScript
    expected: HashMap<&'static str, Value>,
}

impl Run {
    fn ctx(&self) -> Ctx<'_> {
        Ctx { project: &self.project, view: self.view, options: &self.options, sel: &self.sel }
    }

    fn apply(&mut self, ops: Vec<EditOp>) {
        for op in ops {
            match op {
                EditOp::Select(s) => self.sel = s,
                EditOp::Doc(e) => apply_doc(&mut self.project, e),
                op => {
                    apply_to_project(&mut self.project, &op);
                }
            }
        }
    }

    /// Point du plan sous la souris (`toWorld`).
    fn world(&self, step: &Value) -> [f64; 2] {
        let v = &self.view;
        [v.cx + (f(&step["x"]) - v.w / 2.0) / v.s, v.cz + (f(&step["y"]) - v.h / 2.0) / v.s]
    }

    fn js_id(&self, id: &str) -> String {
        self.ids.get(id).cloned().unwrap_or_else(|| id.to_string())
    }

    fn step(&mut self, step: &Value) {
        let m = Modifiers { ctrl: step["ctrl"].as_bool().unwrap_or(false), shift: step["shift"].as_bool().unwrap_or(false) };
        let p = if step.get("x").is_some() { self.world(step) } else { [0.0; 2] };
        let ops = match step["do"].as_str().unwrap() {
            "tool" => {
                self.editor.set_tool(Tool::from_id(step["tool"].as_str().unwrap()).unwrap(), &mut self.options);
                Vec::new()
            }
            "calibrate" => {
                self.editor.start_calibration();
                Vec::new()
            }
            "prop" => {
                let id = self.sel.plot_id.clone().expect("élément sélectionné");
                let item = plot_item(&self.project, &id).cloned().unwrap();
                let v = &step["value"];
                let change = match step["change"].as_str().unwrap() {
                    "text" => ItemChange::Text(v.as_str().unwrap().into()),
                    "color" => ItemChange::Color(v.as_str().unwrap().into()),
                    "fill" => ItemChange::Filled(v.as_bool().unwrap()),
                    "dash" => ItemChange::Dash(v.as_bool().unwrap()),
                    "width" => ItemChange::Width(f(v)),
                    "size" => ItemChange::Size(f(v)),
                    "height" => ItemChange::Height(f(v)),
                    "delete" => return self.apply(self.editor.remove(&self.ctx(), &id)),
                    other => panic!("réglage inconnu : {other}"),
                };
                edit_item(&item, &change).into_iter().collect()
            }
            "menu" => {
                let (hit, ops) = self.editor.context_menu(&self.ctx(), p);
                self.apply(ops);
                let labels: Vec<Value> = match &hit {
                    Some(Hit::Item(id)) => {
                        let kind = plot_item(&self.project, id).and_then(|it| it["kind"].as_str()).unwrap_or("");
                        vec![kind_name(kind).unwrap_or("Élément").into(), "Dupliquer".into(), "Supprimer".into()]
                    }
                    _ => Vec::new(),
                };
                assert_eq!(Value::Array(labels), step["labels"], "menu contextuel");
                match (&hit, step["action"].as_str()) {
                    (Some(Hit::Item(id)), Some("Dupliquer")) => self.editor.duplicate(&self.ctx(), id),
                    (Some(Hit::Item(id)), Some("Supprimer")) => self.editor.remove(&self.ctx(), id),
                    (_, None) => Vec::new(),
                    (h, a) => panic!("menu : {a:?} sur {h:?}"),
                }
            }
            gesture => {
                let Run { project, sel, options, editor, view, .. } = self;
                let ctx = Ctx { project, view: *view, options, sel };
                match gesture {
                    "down" => editor.pointer_down(&ctx, p, m),
                    "move" => editor.pointer_move(&ctx, p, m),
                    "up" => editor.pointer_up(&ctx, p),
                    "dblclick" => editor.double_click(&ctx),
                    "distance" => editor.apply_calibration(&ctx, f(&step["value"])),
                    "key" => {
                        let key = match step["key"].as_str().unwrap() {
                            "Escape" => Key::Escape,
                            "Enter" => Key::Enter,
                            _ => Key::Delete,
                        };
                        editor.key(&ctx, key).unwrap_or_default()
                    }
                    other => panic!("geste inconnu : {other}"),
                }
            }
        };
        self.apply(ops);
    }

    /// Compare l'état à celui de la version JavaScript après le geste.
    fn check(&mut self, after: &Value, what: &str) {
        for k in ["items", "underlay", "fixtures", "objects"] {
            if let Some(v) = after.get(k) {
                self.expected.insert(k, v.clone());
            }
        }
        // éléments du plan (les identifiants des éléments créés diffèrent d'une version à l'autre)
        let items = plot_items(&self.project).to_vec();
        let expected = self.expected["items"].as_array().unwrap().clone();
        assert_eq!(items.len(), expected.len(), "{what} : nombre d'éléments du plan");
        for (it, e) in items.iter().zip(&expected) {
            let id = it["id"].as_str().unwrap().to_string();
            if !self.original.contains(&id) && !self.ids.contains_key(&id) {
                self.ids.insert(id.clone(), e["id"].as_str().unwrap().to_string());
            }
            let mut mine = it.clone();
            mine["id"] = self.js_id(&id).into();
            assert!(same_json(&mine, e), "{what} : élément du plan\n  JavaScript : {e}\n  Rust       : {mine}");
        }
        let underlay = self.project.extra["plot"].get("underlay").cloned().unwrap_or(Value::Null);
        assert!(same_json(&underlay, &self.expected["underlay"]), "{what} : image de fond\n  JavaScript : {}\n  Rust       : {underlay}", self.expected["underlay"]);
        let fixtures: Vec<Value> = self.project.fixtures.iter().map(|x| json!({ "id": x.id, "position": x.position, "rotation": x.rotation })).collect();
        assert!(same_json(&Value::Array(fixtures), &self.expected["fixtures"]), "{what} : projecteurs");
        let objects: Vec<Value> = self.project.objects.iter().map(|x| json!({ "id": x.id, "position": x.position, "rotation": x.rotation })).collect();
        assert!(same_json(&Value::Array(objects), &self.expected["objects"]), "{what} : objets");

        // outil, indication, annotations affichées
        assert_eq!(after["tool"].as_str(), Some(self.editor.tool().id()), "{what} : outil");
        assert_eq!(after["hint"].as_str(), Some(self.editor.hint()), "{what} : indication");
        assert_eq!(after["annotations"].as_bool(), Some(self.options.annotations), "{what} : annotations");
        assert_eq!(after["calibrating"].as_bool(), Some(self.editor.calibration_ready()), "{what} : calage");

        // sélection
        let fixtures: Vec<String> = self.project.fixtures.iter().filter(|x| self.sel.fixtures.contains(&x.id)).map(|x| x.id.clone()).collect();
        let sel = json!({ "fixtures": fixtures, "objectId": self.sel.object_id, "plotId": self.sel.plot_id.as_deref().map(|id| self.js_id(id)) });
        assert_eq!(sel, after["sel"], "{what} : sélection");

        // tracé en cours
        let preview = self.editor.preview().map(|pv| json!({ "kind": pv.kind, "points": pv.points }));
        assert!(same_json(&preview.clone().unwrap_or(Value::Null), &after["preview"]), "{what} : tracé en cours\n  JavaScript : {}\n  Rust       : {preview:?}", after["preview"]);

        // poignées (sin et cos : à l'arrondi près)
        let handles: Vec<Value> = self
            .editor
            .edit_handles(&self.ctx())
            .iter()
            .map(|h| {
                let target = match &h.kind {
                    HandleKind::Rotate(Target::Fixture(id)) => json!({ "kind": "fixture", "id": id }),
                    HandleKind::Rotate(Target::Object(id)) => json!({ "kind": "object", "id": id }),
                    HandleKind::Vertex { .. } => panic!("pas de sommets dans la version JavaScript"),
                };
                json!({ "kind": h.to_render().kind, "x": h.x, "z": h.z, "target": target })
            })
            .collect();
        assert!(same_json(&Value::Array(handles.clone()), &after["handles"]), "{what} : poignées\n  JavaScript : {}\n  Rust       : {handles:?}", after["handles"]);

        // cadre de sélection (pixels) ; Ctrl+clic dans le vide : la version JavaScript ne redessine
        // pas encore le plan, le cadre (vide) n'apparaît qu'au premier déplacement
        let marquee = self.editor.marquee_rect(&self.view).filter(|r| !(after["marquee"].is_null() && r[2] == 0.0 && r[3] == 0.0)).map(|r| json!(r));
        assert!(same_json(&marquee.clone().unwrap_or(Value::Null), &after["marquee"]), "{what} : cadre de sélection\n  JavaScript : {}\n  Rust       : {marquee:?}", after["marquee"]);
    }
}

#[test]
fn outils_du_plan_identiques() {
    let scene = data("editor-scene.json");
    let v: Value = serde_json::from_str(&data("editor.json")).unwrap();
    let mut gestures = 0;
    for sc in v["scenarios"].as_array().unwrap() {
        let name = sc["name"].as_str().unwrap();
        let project = Project::from_json(&scene).unwrap();
        let options: PlotOptions = serde_json::from_value(sc["options"].clone()).unwrap();
        // vue cadrée sur la scène, comme à l'ouverture du plan
        let view = compute_fit(&project, f(&sc["W"]), f(&sc["H"]));
        let jv = &sc["view"];
        assert!(close(view.cx, f(&jv["cx"])) && close(view.cz, f(&jv["cz"])) && close(view.s, f(&jv["s"])), "{name} : vue {view:?} / {jv}");
        let original = plot_items(&project).iter().map(|it| it["id"].as_str().unwrap().to_string()).collect();
        let raw: Value = serde_json::from_str(&scene).unwrap();
        let expected = HashMap::from([
            ("items", raw["plot"]["items"].clone()),
            ("underlay", raw["plot"]["underlay"].clone()),
            ("fixtures", Value::Array(raw["fixtures"].as_array().unwrap().iter().map(|x| json!({ "id": x["id"], "position": x["position"], "rotation": x["rotation"] })).collect())),
            ("objects", Value::Array(raw["objects"].as_array().unwrap().iter().map(|x| json!({ "id": x["id"], "position": x["position"], "rotation": x["rotation"] })).collect())),
        ]);
        // sans les poignées des sommets, absentes de la version JavaScript
        let mut editor = Editor::new();
        editor.vertex_handles = false;
        let mut run = Run { project, sel: Selection::default(), options, editor, view, ids: HashMap::new(), original, expected };
        for (i, step) in sc["steps"].as_array().unwrap().iter().enumerate() {
            run.step(step);
            let mut desc = step.clone();
            desc.as_object_mut().unwrap().remove("after");
            run.check(&step["after"], &format!("{name}, geste {} {desc}", i + 1));
            gestures += 1;
        }
    }
    assert!(gestures > 200, "{gestures} gestes");
}

#[test]
fn poignees_des_sommets() {
    // en plus de la version JavaScript : retouche des sommets de l'élément sélectionné
    let mut project = Project::from_json(&data("editor-scene.json")).unwrap();
    let options = PlotOptions::show_all();
    let view = compute_fit(&project, 1000.0, 700.0);
    let mut sel = Selection { plot_id: Some("p-poly".into()), ..Selection::default() };
    let mut ed = Editor::new();
    let ctx = Ctx { project: &project, view, options: &options, sel: &sel };
    let hs = ed.handles(&ctx);
    assert_eq!(hs.len(), 3);
    assert!(hs.iter().all(|h| h.kind == "vertex"));
    // le deuxième sommet de la forme [[1, 4.5], [2.5, 5], [2, 6.5]] tiré de 50 cm vers la droite
    let m = Modifiers::default();
    assert!(ed.pointer_down(&ctx, [2.5, 5.0], m).is_empty());
    let ops = ed.pointer_move(&ctx, [3.0, 5.0], m);
    assert_eq!(ops, vec![EditOp::UpdateItem { id: "p-poly".into(), changes: json!({ "points": [[1, 4.5], [3, 5], [2, 6.5]] }) }]);
    assert_eq!(ed.pointer_up(&ctx, [3.0, 5.0]), ops);
    apply_to_project(&mut project, &ops[0]);
    // une ligne : ses deux extrémités ; une main levée, un texte : aucune
    for (id, n) in [("p-line", 2), ("p-free", 0), ("p-text", 0), ("p-ell", 4)] {
        sel.plot_id = Some(id.into());
        assert_eq!(ed.handles(&Ctx { project: &project, view, options: &options, sel: &sel }).len(), n, "{id}");
    }
    // annotations masquées : pas de poignées
    let hidden = PlotOptions::default();
    assert!(ed.handles(&Ctx { project: &project, view, options: &hidden, sel: &sel }).is_empty());
}

#[test]
fn volumes_du_plan() {
    let project = Project::from_json(&data("editor-scene.json")).unwrap();
    let v = plot_volumes(&project);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].id, "p-fill");
    assert_eq!(v[0].height, 2.5);
    assert_eq!(v[0].outline, vec![[5.0, 3.0], [7.0, 3.0], [7.0, 4.5], [5.0, 4.5]]);
}
