//! L'API telle que l'application l'utilise : registre `Onboard`, changements appliqués par
//! `Document` (fusion champ par champ, annuler / rétablir), état d'exécution du moteur.

use arklight_core::document::{Changes, Document, Edit};
use arklight_core::engine::{Ctx as EngineCtx, Runtime, Universes, compute_output};
use arklight_core::library::library;
use arklight_core::project::Project;
use arklight_onboard::screen::{DisplayKind, Element};
use arklight_onboard::{Env, Onboard, Outcome, PanelKind, Screen};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

struct App {
    doc: Document,
    onboard: Onboard,
    runtimes: HashMap<String, Runtime>,
    live: HashSet<u32>,
    time: f64,
}

impl App {
    fn new() -> App {
        App { doc: Document::new(Project::empty("essai"), None), onboard: Onboard::new(), runtimes: HashMap::new(), live: HashSet::new(), time: 10.0 }
    }

    fn add(&mut self, type_id: &str, settings: Value) -> String {
        let settings: Map<String, Value> = settings.as_object().cloned().unwrap_or_default();
        self.doc.apply(Edit::AddFixture { type_id: type_id.into(), position: [0.0, 1.0, 0.0], rotation: [0.0; 3], name: None, settings }).expect("projecteur ajouté")
    }

    fn setting(&self, id: &str, key: &str) -> Value {
        let f = self.doc.fixture(id).unwrap();
        serde_json::to_value(&f.settings).unwrap().get(key).cloned().unwrap_or(Value::Null)
    }

    /// Applique les changements d'un geste comme l'application (`Edit::UpdateFixture`).
    fn apply(&mut self, id: &str, out: Outcome) {
        if !out.settings.is_empty() || !out.state.is_empty() {
            self.doc.apply(Edit::UpdateFixture { id: id.into(), changes: out.changes() });
        }
        for r in out.others {
            self.doc.apply(Edit::UpdateFixture { id: r.id.clone(), changes: Changes { settings: Some(r.settings.clone()), ..Changes::default() } });
            if r.relatch {
                let rt = self.runtimes.entry(r.id.clone()).or_default();
                self.onboard.relatch(&r.id, rt, r.unlinked);
            }
        }
    }

    /// Une image : moteur lumière puis tick de chaque panneau ouvert.
    fn frame(&mut self) {
        let dmx = Universes::new();
        let fixtures = self.doc.project.fixtures.clone();
        for f in &fixtures {
            let ty = library().fixture(&f.type_id).unwrap();
            let rt = self.runtimes.entry(f.id.clone()).or_default();
            let mut out = vec![0.0; ty.pixels * 3];
            compute_output(f, ty, &EngineCtx { dmx: &dmx, live: &self.live, time: self.time }, &mut out, rt);
        }
        for f in &fixtures {
            let env = Env { time: self.time, fixtures: &fixtures, live: &self.live, app_connected: false };
            let rt = self.runtimes.entry(f.id.clone()).or_default();
            let out = self.onboard.tick(f, rt, &env);
            self.apply(&f.id, out);
        }
    }

    fn gesture(&mut self, id: &str, f: impl FnOnce(&mut Onboard, &arklight_core::project::Fixture, &mut Runtime, &Env) -> Outcome) {
        let fixtures = self.doc.project.fixtures.clone();
        let fixture = fixtures.iter().find(|x| x.id == id).unwrap().clone();
        let env = Env { time: self.time, fixtures: &fixtures, live: &self.live, app_connected: false };
        let rt = self.runtimes.entry(id.to_string()).or_default();
        let out = f(&mut self.onboard, &fixture, rt, &env);
        self.apply(id, out);
        self.frame();
    }

    fn tap(&mut self, id: &str, key: &str) {
        self.time += 0.1;
        self.gesture(id, |o, f, rt, env| o.press(f, rt, env, key));
        self.time += 0.05;
        self.gesture(id, |o, f, rt, env| o.release(f, rt, env, key));
    }

    fn turn(&mut self, id: &str, knob: &str, steps: i32) {
        self.time += 0.3;
        self.gesture(id, |o, f, rt, env| o.turn(f, rt, env, knob, steps));
    }

    fn screen(&mut self, id: &str) -> Screen {
        let fixtures = self.doc.project.fixtures.clone();
        let fixture = fixtures.iter().find(|x| x.id == id).unwrap().clone();
        let env = Env { time: self.time, fixtures: &fixtures, live: &self.live, app_connected: false };
        let rt = self.runtimes.entry(id.to_string()).or_default();
        self.onboard.display(&fixture, rt, &env)
    }

    fn lines(&mut self, id: &str) -> Vec<String> {
        match self.screen(id) {
            Screen::Text(t) => t.lines.into_iter().map(|l| l.text).collect(),
            other => panic!("écran inattendu : {other:?}"),
        }
    }
}

#[test]
fn titan_menus_par_le_registre_et_le_document() {
    let mut app = App::new();
    let titan = app.add("astera.titan", json!({}));
    app.frame();
    assert_eq!(app.lines(&titan), ["CRMX: NO DMX", "SET:---  DMX:001"]);
    // ENTER : DMX ADDRESS, ENTER : réglage, + , ENTER : enregistré (et annulable)
    app.tap(&titan, "enter");
    assert_eq!(app.lines(&titan), ["Main menu:", "DMX ADDRESS"]);
    app.tap(&titan, "enter");
    app.tap(&titan, "plus");
    assert_eq!(app.lines(&titan), ["DMX ADDRESS:", "002"]);
    app.tap(&titan, "enter");
    assert_eq!(app.setting(&titan, "dmxAddress"), json!(2));
    assert!(app.doc.can_undo());
    // palette : STANDALONE verrouillé, couleur HSI enregistrée dans le mode autonome
    app.tap(&titan, "palette");
    app.tap(&titan, "plus");
    assert_eq!(app.setting(&titan, "standalone")["colors"][0], json!({ "kind": "hsi", "cct": 5650, "green": 0, "hue": 0, "sat": 0 }));
    app.tap(&titan, "menu");
    assert_eq!(app.lines(&titan)[0], "STANDALONE");
    // télécommande IR : refusée en CRMX, acceptée en AUTO
    let f = app.doc.fixture(&titan).unwrap().clone();
    let rt = app.runtimes.get_mut(&titan).unwrap();
    let r = app.onboard.ir_key(&f, rt, "FIRE", app.time);
    assert!(r.accepted);
    assert_eq!(r.changes.unwrap().settings["standalone"]["program"], json!(16));
    let face = app.onboard.panel(&f).unwrap().face().clone();
    assert_eq!(face.style, "tube");
    assert_eq!(face.display.as_ref().map(|d| d.kind), Some(DisplayKind::Oled));
    assert!(face.elements().iter().any(|e| matches!(e, Element::Key { id, .. } if id == "palette")));
}

#[test]
fn asterabox_lie_les_astera_de_son_univers() {
    let mut app = App::new();
    let box_id = app.add("astera.art7", json!({ "universe": 2 }));
    let a = app.add("astera.titan", json!({ "crmxLinked": false }));
    let b = app.add("astera.helios", json!({ "crmxLinked": false, "inputSelect": "STANDALONE" }));
    app.frame();
    app.tap(&box_id, "link");
    assert_eq!(app.setting(&a, "crmxLinked"), json!(true));
    assert_eq!(app.setting(&a, "universe"), json!(2));
    assert_eq!(app.setting(&b, "crmxLinked"), json!(false));
    // maintien 3 s : déliaison de l'univers ; l'écran du Titan affiche « NOT LINKED »
    app.time += 11.0;
    app.frame();
    app.time += 0.1;
    app.gesture(&box_id, |o, f, rt, env| o.press(f, rt, env, "link"));
    for _ in 0..13 {
        app.time += 0.25;
        app.frame();
    }
    app.gesture(&box_id, |o, f, rt, env| o.release(f, rt, env, "link"));
    assert_eq!(app.setting(&a, "crmxLinked"), json!(false));
    assert_eq!(app.lines(&a)[0], "CRMX: NOT LINKED");
    assert_eq!(app.onboard.panel(app.doc.fixture(&box_id).unwrap()).map(|p| p.kind()), Some(PanelKind::AsteraBox));
}

#[test]
fn skypanel_menu_et_molettes() {
    let mut app = App::new();
    let sky = app.add("arri.skypanel-s60c", json!({}));
    app.tap(&sky, "menu");
    app.tap(&sky, "is");
    app.tap(&sky, "is");
    app.turn(&sky, "is", 4);
    app.tap(&sky, "is");
    assert_eq!(app.setting(&sky, "dmxAddress"), json!(5));
    app.tap(&sky, "menu");
    app.turn(&sky, "center", 3);
    let f = app.doc.fixture(&sky).unwrap();
    assert_eq!(f.state["manual"]["cct"], json!(5630));
    let Screen::Draw(d) = app.screen(&sky) else { panic!("écran dessiné") };
    assert!(!d.prims.is_empty());
    let panel = app.onboard.panel(app.doc.fixture(&sky).unwrap()).unwrap();
    assert_eq!(panel.kind(), PanelKind::Brand("arri-skypanel"));
    assert_eq!(panel.face().display.as_ref().map(|d| (d.w, d.h)), Some((128.0, 64.0)));
}

#[test]
fn modeles_sans_panneau() {
    let mut onboard = Onboard::new();
    let lib = library();
    for id in ["astera.lunabulb", "arri.650-plus", "generic.par64-cp62"] {
        let ty = lib.fixture(id).unwrap();
        let f = arklight_core::project::Fixture {
            id: format!("f-{id}"),
            type_id: id.into(),
            name: String::new(),
            position: [0.0; 3],
            rotation: [0.0; 3],
            settings: serde_json::from_value(Value::Object(arklight_core::project::Fixture::default_settings(ty))).unwrap(),
            state: Value::Object(arklight_core::project::Fixture::default_state(ty)),
            extra: Map::new(),
        };
        assert!(onboard.panel(&f).is_none(), "{id} : pas de commandes embarquées");
    }
}
