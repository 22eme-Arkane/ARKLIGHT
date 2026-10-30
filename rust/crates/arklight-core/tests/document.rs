//! Projet ouvert : modifications, adressage automatique, annuler / rétablir (comme
//! `server/scene-store.js` et `server/history.js`).

use arklight_core::Project;
use arklight_core::document::{Changes, Document, Edit};
use serde_json::{Map, Value, json};

fn add(doc: &mut Document, type_id: &str) -> String {
    doc.apply(Edit::AddFixture { type_id: type_id.into(), position: [0.0, 1.5, 0.0], rotation: [0.0; 3], name: None, settings: Map::new() }).expect("projecteur ajouté")
}

#[test]
fn adressage_a_la_suite_et_numeros() {
    let mut doc = Document::new(Project::empty("essai"), None);
    let a = add(&mut doc, "astera.titan");
    let b = add(&mut doc, "astera.titan");
    let (fa, fb) = (doc.fixture(&a).unwrap(), doc.fixture(&b).unwrap());
    assert_eq!(fa.settings.dmx_address, Some(1));
    // profil 1 du Titan : 3 canaux → le suivant commence en 4
    let fp = arklight_core::library().fixture("astera.titan").unwrap().mode(fa.settings.dmx_mode.as_deref()).unwrap().footprint() as u32;
    assert_eq!(fb.settings.dmx_address, Some(1 + fp));
    assert_eq!((fa.settings.fixture_number, fb.settings.fixture_number), (Some(1), Some(2)));
    assert_eq!(fb.name, "Titan Tube 2");
    assert_eq!(fa.state["power"], json!(true), "état d'un projecteur neuf");
}

#[test]
fn annuler_retablir() {
    let mut doc = Document::new(Project::empty("essai"), None);
    assert!(!doc.can_undo());
    let id = add(&mut doc, "arri.m18");
    assert!(doc.dirty());
    assert_eq!(doc.undo_label(), Some("l’ajout du projecteur"));
    doc.apply(Edit::UpdateFixture { id: id.clone(), changes: Changes { name: Some("Rebond".into()), ..Changes::default() } });
    assert_eq!(doc.undo_label(), Some("le changement de nom"));
    assert!(doc.undo());
    assert!(doc.fixture(&id).unwrap().name.ends_with(" 1"), "nom d'origine revenu");
    assert!(doc.redo());
    assert_eq!(doc.fixture(&id).unwrap().name, "Rebond");
    assert!(doc.undo() && doc.undo());
    assert!(doc.project.fixtures.is_empty());
    assert!(!doc.can_undo() && doc.can_redo());
}

#[test]
fn un_glisser_ne_fait_qu_une_etape() {
    let mut doc = Document::new(Project::empty("essai"), None);
    let id = add(&mut doc, "astera.helios");
    for i in 0..10 {
        doc.apply(Edit::UpdateFixture { id: id.clone(), changes: Changes::moved([i as f64 * 0.1, 1.5, 0.0], None) });
    }
    assert_eq!(doc.undo_label(), Some("le déplacement"));
    assert!(doc.undo(), "un seul retour pour tout le glisser");
    assert_eq!(doc.fixture(&id).unwrap().position, [0.0, 1.5, 0.0]);
    assert_eq!(doc.undo_label(), Some("l’ajout du projecteur"));
}

#[test]
fn reglages_fusionnes_et_console_relachee() {
    let mut doc = Document::new(Project::empty("essai"), None);
    let id = add(&mut doc, "astera.titan");
    let mut state = Map::new();
    state.insert("console".into(), json!({ "mode": "1", "values": [255, 0, 0] }));
    doc.apply(Edit::UpdateFixture { id: id.clone(), changes: Changes { state: Some(state), ..Changes::default() } });
    assert!(arklight_core::engine::console::console_held(doc.fixture(&id).unwrap()));
    // un réglage manuel rend la main au projecteur
    let mut state = Map::new();
    state.insert("manual".into(), json!({ "intensity": 0.3 }));
    doc.apply(Edit::UpdateFixture { id: id.clone(), changes: Changes { state: Some(state), ..Changes::default() } });
    let f = doc.fixture(&id).unwrap();
    assert!(!arklight_core::engine::console::console_held(f));
    assert_eq!(f.state["manual"]["intensity"], json!(0.3));
    assert_eq!(f.state["manual"]["mode"], json!("cct"), "les autres champs du réglage restent");
}

#[test]
fn objets_groupes_et_ambiance() {
    let mut doc = Document::new(Project::empty("essai"), None);
    let a = add(&mut doc, "astera.titan");
    let o = doc.apply(Edit::AddObject { type_id: "frame-8x8".into(), position: [0.0, 1.5, 2.0], rotation: [0.0; 3], name: None, props: Map::new() }).unwrap();
    assert_eq!(doc.object(&o).unwrap().props["cloth"], json!("silk"), "toile par défaut du cadre");
    let g = doc.apply(Edit::CreateGroup { name: None, ids: vec![a.clone(), "inconnu".into()] }).unwrap();
    assert_eq!(doc.project.groups[0].ids, vec![a.clone()]);
    assert_eq!(doc.project.groups[0].name, "Groupe 1");
    doc.apply(Edit::RemoveFixtures(vec![a]));
    assert!(doc.project.groups.iter().find(|x| x.id == g).unwrap().ids.is_empty(), "retiré des groupes");
    let mut env = Map::new();
    env.insert("haze".into(), Value::from(0.6));
    doc.apply(Edit::Environment(env));
    assert_eq!(doc.project.environment.haze, 0.6);
}

#[test]
fn sequences_et_plans() {
    use arklight_core::plans::Start;
    let mut doc = Document::new(Project::empty("essai"), None);
    let info = doc.plan_info();
    assert_eq!((info.len(), info[0].plans.len()), (1, 1), "un projet neuf : Séquence 1 › Plan 1");
    assert_eq!(doc.current_plan_label(), "Séquence 1 › Plan 1");
    let first = info[0].plans[0].id.clone();
    add(&mut doc, "astera.titan");
    // nouveau plan, copie du plan ouvert : il devient le plan ouvert
    let p2 = doc.new_plan(Start::Copy).unwrap();
    assert!(!doc.can_undo(), "l'historique repart avec le nouveau plan");
    assert_eq!(doc.project.fixtures.len(), 1);
    assert_eq!(doc.current_plan_label(), "Séquence 1 › Plan 2");
    add(&mut doc, "arri.m18");
    assert_eq!(doc.project.fixtures.len(), 2);
    // retour au premier plan : sa scène revient
    assert!(doc.switch_plan(&first));
    assert_eq!(doc.project.fixtures.len(), 1);
    assert_eq!(doc.project.name, "essai", "le nom du projet suit");
    let counts: Vec<usize> = doc.plan_info()[0].plans.iter().map(|p| p.fixtures).collect();
    assert_eq!(counts, vec![1, 2]);
    // séquence vide, renommage, suppression
    doc.new_sequence(Start::Empty).unwrap();
    assert!(doc.project.fixtures.is_empty());
    assert_eq!(doc.plan_info().len(), 2);
    assert!(doc.rename_plan(&p2, "Plan large"));
    assert!(doc.plan_info()[0].plans.iter().any(|p| p.name == "Plan large"));
    let cur = doc.plan_info()[1].plans[0].id.clone();
    assert!(doc.remove_plan(&cur));
    assert_eq!(doc.plan_info().len(), 1, "séquence vidée disparue");
    assert!(doc.step_plan(true) || doc.step_plan(false));
}

#[test]
fn plans_de_l_exemple_clip() {
    let text = std::fs::read_to_string(format!("{}/tests/data/demo-clip.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut doc = Document::new(Project::from_json(&text).unwrap(), None);
    let info = doc.plan_info();
    let total: usize = info.iter().map(|s| s.plans.len()).sum();
    assert!(total > 1, "l'exemple a plusieurs plans");
    let before = serde_json::to_value(&doc.project.fixtures).unwrap();
    let start = doc.plan_info().iter().flat_map(|s| s.plans.iter()).find(|p| p.current).unwrap().id.clone();
    let other = doc.plan_info().iter().flat_map(|s| s.plans.iter()).find(|p| !p.current).unwrap().id.clone();
    assert!(doc.switch_plan(&other));
    assert!(doc.switch_plan(&start));
    assert_eq!(serde_json::to_value(&doc.project.fixtures).unwrap(), before, "aller-retour sans perte");
}

#[test]
fn plans_dupliquer_deplacer_supprimer() {
    use arklight_core::plans::Start;
    let mut doc = Document::new(Project::empty("essai"), None);
    let s1 = doc.plan_info()[0].id.clone();
    let p1 = doc.current_plan_id();
    add(&mut doc, "astera.titan");
    // copie juste après le plan, on reste sur le plan ouvert ; la vignette suit
    doc.set_plan_thumbnail(&p1, "data:image/png;base64,AAAA");
    assert!(doc.duplicate_plan(&p1));
    let plans = &doc.plan_info()[0].plans;
    assert_eq!(plans.len(), 2);
    assert_eq!(plans[1].name, "Plan 1 (copie)");
    assert_eq!((plans[1].fixtures, plans[1].thumbnail.is_some()), (1, true));
    assert_eq!(doc.current_plan_id(), p1);
    let copy = plans[1].id.clone();
    // déplacer à gauche, puis dans une nouvelle séquence (à la fin)
    assert!(doc.move_plan(&copy, Some(&s1), Some(0)));
    assert_eq!(doc.plan_info()[0].plans[0].id, copy);
    let s2 = doc.new_sequence(Start::Empty).unwrap();
    let p3 = doc.current_plan_id();
    assert!(doc.move_plan(&copy, Some(&s2), None));
    assert_eq!(doc.plan_info()[0].plans.len(), 1);
    assert_eq!(doc.plan_info()[1].plans.iter().map(|p| p.id.clone()).collect::<Vec<_>>(), vec![p3.clone(), copy.clone()]);
    // nouveau plan dans une autre séquence que celle du plan ouvert : à sa fin
    let p4 = doc.new_plan_in(Some(&s1), Start::Empty).unwrap();
    assert_eq!(doc.plan_info()[0].plans.last().unwrap().id, p4);
    assert_eq!(doc.current_plan_id(), p4);
    // séquences : déplacer, supprimer celle du plan ouvert (on passe à une autre)
    assert!(doc.move_sequence(&s2, 0));
    assert_eq!(doc.plan_info()[0].id, s2);
    assert!(doc.remove_sequence(&s1));
    assert_eq!(doc.plan_info().len(), 1);
    assert!(doc.plan_info()[0].plans.iter().any(|p| p.current));
    assert!(!doc.remove_sequence(&s2), "le projet garde au moins une séquence");
}
