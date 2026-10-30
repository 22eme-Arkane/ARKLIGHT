//! Les outils de l'assistant sur le projet d'exemple « demo-night » : ajouter, viser, couleurs,
//! ambiance, adressage, suppression, plans… et tout s'annule.

mod common;

use arklight_ai::tools::{is_error, result_summary};
use arklight_core::addressing::{conflicts, footprint_of};
use arklight_ai::{DmxState, NoHooks, ToolHooks, ToolRunner};
use arklight_core::Project;
use arklight_core::document::{Changes, Document, Edit};
use common::demo;
use serde_json::{Map, Value, json};

fn run(doc: &mut Document, runner: &mut ToolRunner, name: &str, args: Value) -> Value {
    let r = runner.run(doc, &mut NoHooks, name, &args);
    assert!(!is_error(&r), "{name} : {}", result_summary(&r));
    r
}

fn json_of(doc: &Document) -> String {
    serde_json::to_string(&doc.project).unwrap()
}

#[test]
fn ajouter_viser_regler_et_annuler() {
    let mut doc = demo();
    let start = json_of(&doc);
    let n0 = doc.project.fixtures.len();
    let mut runner = ToolRunner::default();

    // 3 SkyPanel S60 en arc, visant la passante (« comédienne » désignerait le premier personnage),
    // en 3200 K à 45 %, groupés
    let r = run(&mut doc, &mut runner, "add_fixtures", json!({ "type": "S60", "count": 3, "aim": "passante", "cct": 3200, "intensity": 45, "group": "Faces S60", "universe": 3 }));
    assert_eq!(result_summary(&r), "3 × ARRI Skypanel S60C ajouté(s)");
    assert_eq!(doc.project.fixtures.len(), n0 + 3);
    assert_eq!(runner.last_added.len(), 3);
    assert!(runner.changed.fixtures && runner.changed.groups);
    let added: Vec<_> = doc.project.fixtures[n0..].to_vec();
    let passante = doc.project.objects.iter().find(|o| o.name == "Passante au téléphone").unwrap().position;
    let fp = footprint_of(&added[0]);
    for (i, f) in added.iter().enumerate() {
        assert_eq!(f.type_id, "arri.skypanel-s60c");
        assert_eq!(f.settings.universe, Some(3));
        assert_eq!(f.settings.dmx_address, Some(1 + fp * i as u32), "à la suite du patch");
        let manual = &f.state["manual"];
        assert_eq!((manual["mode"].as_str(), manual["cct"].as_f64(), manual["intensity"].as_f64()), (Some("cct"), Some(3200.0), Some(0.45)));
        // l'axe +Z du projecteur (angles d'Euler XYZ) passe par le visage de la passante (1,55 m)
        let [rx, ry, _] = f.rotation;
        let dir = [ry.sin(), -rx.sin() * ry.cos(), rx.cos() * ry.cos()];
        let to = [passante[0] - f.position[0], 1.55 - f.position[1], passante[2] - f.position[2]];
        let len = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2]).sqrt();
        let dot = (dir[0] * to[0] + dir[1] * to[1] + dir[2] * to[2]) / len;
        assert!(dot > 0.999, "projecteur {i} mal visé : {dot}");
    }
    let g = doc.project.groups.iter().find(|g| g.name == "Faces S60").unwrap();
    assert_eq!(g.ids, runner.last_added);

    // Titan en couleur (mode autonome des Astera) : une seule étape
    let r = run(&mut doc, &mut runner, "add_fixtures", json!({ "type": "astera.titan", "positions": [[-2, 0, -3], [2, 0, -3]], "hue": 220, "saturation": 80, "intensity": 70 }));
    assert_eq!(r["added"].as_array().unwrap().len(), 2, "nombre pris aux positions");
    let titan = doc.project.fixtures.last().unwrap();
    assert!(titan.position[1] > 0.5, "un tube debout pose sur sa base : {:?}", titan.position);
    assert_eq!(titan.rotation[0], 0.0, "un tube reste debout");
    let sa = &titan.settings.extra["standalone"];
    assert_eq!((sa["program"].as_f64(), sa["intensity"].as_f64()), (Some(0.0), Some(70.0)));
    assert_eq!(sa["colors"][3], json!({ "kind": "hsi", "hue": 220, "sat": 80, "cct": 5600 }));

    // régler, viser, déplacer un projecteur existant (par son nom)
    run(&mut doc, &mut runner, "update_fixtures", json!({ "target": { "ids": ["S60 lune"] }, "hue": 30, "saturation": 100, "aim": [0, 1.6, 0], "move": [0, 0.5, 0] }));
    let lune = doc.project.fixtures.iter().find(|f| f.name == "S60 lune").unwrap();
    assert_eq!(lune.position, [5.2, 3.7, -3.0]);
    assert_eq!((lune.state["manual"]["mode"].as_str(), lune.state["manual"]["hue"].as_f64(), lune.state["manual"]["sat"].as_f64()), (Some("hsi"), Some(30.0), Some(1.0)));

    // éteindre un groupe ; ambiance
    run(&mut doc, &mut runner, "update_fixtures", json!({ "target": { "group": "practicals" }, "power": false }));
    assert!(doc.project.fixtures.iter().filter(|f| f.name.starts_with("NYX")).all(|f| f.state["power"] == false));
    run(&mut doc, &mut runner, "set_environment", json!({ "haze": 60, "exposure": -1.3, "cyc": true }));
    assert_eq!((doc.project.environment.haze, doc.project.environment.exposure, doc.project.environment.cyc), (0.6, -1.5, true));

    // ré-adresser tout : plus aucun chevauchement, fixtures renumérotées
    let r = run(&mut doc, &mut runner, "readdress", json!({ "target": { "all": true }, "fixture_start": 1 }));
    assert!(result_summary(&r).contains("renuméroté(s)"));
    assert!(conflicts(&doc.project.fixtures).ids.is_empty(), "plus aucun chevauchement");
    let mut numbers: Vec<u32> = doc.project.fixtures.iter().filter_map(|f| f.settings.fixture_number).collect();
    numbers.sort_unstable();
    numbers.dedup();
    assert_eq!(numbers.len(), doc.project.fixtures.iter().filter(|f| f.settings.fixture_number.is_some()).count(), "numéros uniques");

    // décor et grip, suppression
    run(&mut doc, &mut runner, "add_objects", json!({ "type": "frame-8x8", "positions": [[1, 2, 2], [-1, 2, 2]], "props": { "cloth": "grid" }, "face": "actor" }));
    let frames: Vec<_> = doc.project.objects.iter().filter(|o| o.type_id == "frame-8x8").collect();
    assert_eq!(frames.len(), 3, "deux cadres ajoutés (nombre pris aux positions)");
    assert_eq!(frames[2].props["cloth"], "grid");
    let r = run(&mut doc, &mut runner, "remove_items", json!({ "ids": ["Chien", "SL1 regard"] }));
    assert_eq!(r["removed"], json!(["Chien", "SL1 regard"]));
    assert!(!doc.project.groups.iter().any(|g| g.ids.iter().any(|id| doc.fixture(id).is_none())), "plus de projecteur supprimé dans les groupes");

    // tout s'annule, étape par étape
    let mut steps = 0;
    while doc.undo() {
        steps += 1;
    }
    assert!(steps >= 9, "{steps} étapes");
    assert_eq!(json_of(&doc), start, "retour au projet de départ");
    // et se rétablit
    while doc.redo() {}
    assert_eq!(doc.project.fixtures.len(), n0 + 5 - 1);
}

#[test]
fn erreurs_lisibles_par_le_modele() {
    let mut doc = demo();
    let mut runner = ToolRunner::default();
    let r = runner.run(&mut doc, &mut NoHooks, "update_fixtures", &json!({ "target": { "ids": ["inexistant"] } }));
    assert_eq!(r["error"], "Aucun projecteur trouvé : appelle get_scene pour connaître les ids.");
    let r = runner.run(&mut doc, &mut NoHooks, "add_objects", &json!({ "type": "zzz" }));
    assert_eq!(r["error"], "Objet inconnu : zzz. Cherche avec search_catalog (kind: object).");
    let r = runner.run(&mut doc, &mut NoHooks, "outil", &json!(null));
    assert_eq!(r["error"], "Outil inconnu : outil");
    assert!(!doc.can_undo(), "rien n'a changé");
}

#[test]
fn plan_dessine_sur_un_projet_neuf() {
    let mut doc = Document::new(Project::empty("Essai"), None);
    let mut runner = ToolRunner::new(Some(1.0));
    let r = run(
        &mut doc,
        &mut runner,
        "place_from_drawing",
        json!({ "width_m": 10, "items": [
            { "kind": "person", "label": "Chanteuse", "x": 0.5, "y": 0.5 },
            { "kind": "fixture", "label": "M18 contre", "x": 0.5, "y": 0.1 },
            { "kind": "camera", "label": "travelling 50mm", "x": 0.5, "y": 0.9 },
        ] }),
    );
    assert_eq!(result_summary(&r), "3 élément(s) placé(s) d’après le dessin");
    // le comédien par défaut a laissé la place à la chanteuse dessinée
    let names: Vec<&str> = doc.project.objects.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, vec!["Chanteuse", "travelling 50mm"]);
    assert_eq!(doc.project.objects[0].type_id, "person-woman");
    let cam = &doc.project.objects[1];
    assert_eq!((cam.type_id.as_str(), cam.position, cam.props["focal"].as_f64()), ("camera-dolly", [0.0, 0.0, 4.0], Some(50.0)));
    let m18 = &doc.project.fixtures[0];
    assert_eq!((m18.type_id.as_str(), m18.position), ("arri.m18", [0.0, 2.6, -4.0]));
    assert_eq!(runner.last_added, vec![m18.id.clone()]);
    assert!(doc.undo());
    assert_eq!(doc.project.objects.len(), 1, "une seule étape : le comédien revient");
}

#[test]
fn sequences_et_plans() {
    let mut doc = demo();
    let mut runner = ToolRunner::default();
    let r = run(&mut doc, &mut runner, "manage_plans", json!({ "action": "new_plan", "mode": "decor", "name": "Contrechamp" }));
    assert_eq!(result_summary(&r), "Nouveau plan : Contrechamp");
    assert!(doc.project.fixtures.is_empty(), "même décor, sans lumière");
    assert_eq!(r["sequences"][0]["plans"].as_array().unwrap().len(), 2);
    let r = run(&mut doc, &mut runner, "manage_plans", json!({ "action": "switch", "id": "plan 1" }));
    assert_eq!(result_summary(&r), "Plan ouvert : Séquence 1 › Plan 1");
    assert_eq!(doc.project.fixtures.len(), 15);
    run(&mut doc, &mut runner, "manage_plans", json!({ "action": "rename_sequence", "name": "Nuit" }));
    assert_eq!(doc.current_plan_label(), "Nuit › Plan 1");
    assert!(runner.changed.plans);
}

/// Crochets : DMX reçu sur l'univers 1, oublié à la demande.
#[derive(Default)]
struct Dmx {
    forgotten: Vec<u32>,
}

impl ToolHooks for Dmx {
    fn dmx_state(&self) -> DmxState {
        DmxState { data: [1].into_iter().collect(), live: Default::default() }
    }
    fn forget_dmx(&mut self, universes: &[u32]) -> Vec<u32> {
        self.forgotten = universes.to_vec();
        universes.to_vec()
    }
}

#[test]
fn qui_commande_et_rendre_la_main() {
    let mut doc = demo();
    let id = doc.project.fixtures[1].id.clone();
    // la console pilote le S360
    let mut console = Map::new();
    console.insert("console".into(), json!({ "values": [255, 0, 0] }));
    doc.apply(Edit::UpdateFixture { id: id.clone(), changes: Changes { state: Some(console), ..Changes::default() } });
    let mut hooks = Dmx::default();
    let mut runner = ToolRunner::default();
    let scene = runner.run(&mut doc, &mut hooks, "get_scene", &json!({ "include_objects": false }));
    assert!(scene.get("objects").is_none());
    assert_eq!(scene["fixtures"][1]["control"], "console ARKLIGHT");
    assert_eq!(scene["fixtures"][0]["control"], "DMX conservé, plus de signal (univers 1) : release_control pour rendre la main");
    // Titan relié en CRMX : il suit aussi le DMX conservé
    assert!(scene["fixtures"][7]["control"].as_str().unwrap().starts_with("DMX conservé"), "{}", scene["fixtures"][7]);
    assert_eq!(scene["fixtures"][1]["dmx"], "U1 2-13 (12 c., P01: CCT & RGBW 8bit)");
    let r = runner.run(&mut doc, &mut hooks, "release_control", &json!({}));
    assert_eq!(r["summary"], "1 projecteur(s) relâché(s) de la console ; DMX conservé oublié (univers 1)");
    assert_eq!(hooks.forgotten, vec![1]);
    assert!(doc.fixture(&id).unwrap().state["console"].is_null());
}

#[test]
fn formes_du_plan_dessine() {
    // une maison dessinée en rectangle et une estrade en contour : formes du plan 2D, avec leur
    // hauteur (volumes de la vue 3D)
    let mut doc = Document::new(Project::empty("Essai"), None);
    let mut runner = ToolRunner::new(Some(1.0));
    let r = run(
        &mut doc,
        &mut runner,
        "place_from_drawing",
        json!({ "width_m": 10, "items": [
            { "kind": "person", "label": "Comédienne", "x": 0.5, "y": 0.5 },
            { "kind": "shape", "label": "MAISON", "x": 0.1, "y": 0.1, "x2": 0.4, "y2": 0.3 },
            { "kind": "shape", "label": "estrade", "height": 0.6, "x": 0.6, "y": 0.6, "points": [[0.6, 0.6], [0.9, 0.6], [0.9, 0.8]] },
            { "kind": "shape", "label": "sans taille", "x": 0.2, "y": 0.8 },
        ] }),
    );
    assert_eq!(result_summary(&r), "3 élément(s) placé(s) d’après le dessin (1 à vérifier)");
    let items = doc.project.extra["plot"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!((items[0]["kind"].as_str(), items[0]["text"].as_str(), items[0]["height"].as_f64()), (Some("rect"), Some("MAISON"), Some(3.0)));
    assert_eq!(items[0]["points"], json!([[-4.0, -4.0], [-1.0, -2.0]]));
    assert_eq!((items[1]["kind"].as_str(), items[1]["height"].as_f64(), items[1]["points"].as_array().map(Vec::len)), (Some("polygon"), Some(0.6), Some(3)));
    assert!(runner.changed.any());
}
