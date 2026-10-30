//! Ce que voit un panneau à chaque appel (projecteur, scène, état d'exécution) et ce qu'il rend
//! (changements des réglages et de l'état, à appliquer par l'application).

use crate::js;
use arklight_core::document::Changes;
use arklight_core::engine::Runtime;
use arklight_core::engine::astera::Input;
use arklight_core::project::Fixture;
use serde_json::{Map, Value};
use std::collections::HashSet;

/// La scène autour du panneau, à l'instant de l'appel.
#[derive(Debug, Clone, Copy)]
pub struct Env<'a> {
    /// heure (secondes, même base que le moteur lumière) : appuis longs, messages, clignotements
    pub time: f64,
    /// tous les projecteurs de la scène (l'AsteraBox lie et délie les Astera de son univers)
    pub fixtures: &'a [Fixture],
    /// univers dont les trames DMX arrivent en ce moment (voyants DATA / DMX, page « DMX Live »)
    pub live: &'a HashSet<u32>,
    /// l'appli Astera est connectée (voyant APP de l'AsteraBox, ondes Bluetooth de l'écran V5)
    pub app_connected: bool,
}

impl<'a> Env<'a> {
    /// Scène vide (essais, panneau isolé).
    pub fn at(time: f64, live: &'a HashSet<u32>) -> Env<'a> {
        Env { time, fixtures: &[], live, app_connected: false }
    }

    pub fn dmx_live(&self, universe: Option<f64>) -> bool {
        universe.is_some_and(|u| u >= 0.0 && u.fract() == 0.0 && self.live.contains(&(u as u32)))
    }
}

/// État d'exécution d'un Astera propre aux menus (le reste est dans `arklight_core::engine::Runtime`).
/// Non sauvegardé, comme le `rt` de la version JavaScript.
#[derive(Debug, Clone, Default)]
pub struct AsteraRuntime {
    /// « CRMX: NOT LINKED » après UNLINK CRMX (touche, AsteraBox ou application)
    pub just_unlinked: bool,
    /// entrée verrouillée par la télécommande IR (« REMOTE CONTROL ») : le moteur la voit comme
    /// STANDALONE (`Input` n'a pas de variante Remote)
    pub remote: bool,
    /// heure de BLUE MODE 1/2 (télécommande ARC3)
    pub ir_blue1: Option<f64>,
}

impl AsteraRuntime {
    /// `rt.latch = 'STANDALONE'`
    pub(crate) fn latch_standalone(&mut self, rt: &mut Runtime) {
        rt.latch = Some(Input::Standalone);
        self.remote = false;
    }

    /// `rt.latch = null`
    pub(crate) fn unlatch(&mut self, rt: &mut Runtime) {
        rt.latch = None;
        self.remote = false;
    }

    /// `rt.latch = 'REMOTE'`
    pub(crate) fn latch_remote(&mut self, rt: &mut Runtime) {
        rt.latch = Some(Input::Standalone);
        self.remote = true;
    }

    /// Verrou d'entrée tel que l'écrit la version JavaScript (« REMOTE », « STANDALONE », « CRMX »…).
    pub fn latch_name(&self, rt: &Runtime) -> Option<&'static str> {
        match rt.latch {
            Some(Input::Standalone) if self.remote => Some("REMOTE"),
            Some(i) => Some(i.as_str()),
            None => None,
        }
    }
}

/// Tout ce qu'un panneau lit et modifie pendant un appel.
pub struct Ctx<'a> {
    pub fixture: &'a Fixture,
    /// état d'exécution du moteur lumière (verrou d'entrée, couleur du BlueMode…)
    pub runtime: &'a mut Runtime,
    pub astera: &'a mut AsteraRuntime,
    pub env: &'a Env<'a>,
}

/// Changements demandés à un autre projecteur (AsteraBox : liaison et déliaison CRMX).
#[derive(Debug, Clone, PartialEq)]
pub struct Relayed {
    pub id: String,
    pub settings: Map<String, Value>,
    /// l'entrée du projecteur est à redétecter : `runtime.latch = None` et
    /// `AsteraRuntime::just_unlinked = unlinked` (voir `Onboard::relatch`)
    pub relatch: bool,
    pub unlinked: bool,
}

/// Résultat d'un geste : réglages et état modifiés (fusion champ par champ de premier niveau,
/// comme `Object.assign` : chaque clé remplace la précédente).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    pub settings: Map<String, Value>,
    pub state: Map<String, Value>,
    /// projecteurs liés ou déliés par une AsteraBox
    pub others: Vec<Relayed>,
}

impl Outcome {
    pub fn is_empty(&self) -> bool {
        self.settings.is_empty() && self.state.is_empty() && self.others.is_empty()
    }

    /// Changements du projecteur pour `Document` (`Edit::UpdateFixture`).
    pub fn changes(&self) -> Changes {
        Changes {
            settings: (!self.settings.is_empty()).then(|| self.settings.clone()),
            state: (!self.state.is_empty()).then(|| self.state.clone()),
            ..Changes::default()
        }
    }

    /// Ajoute les changements d'un geste suivant.
    pub fn merge(&mut self, next: Outcome) {
        self.settings.extend(next.settings);
        self.state.extend(next.state);
        self.others.extend(next.others);
    }

    /// Applique les changements à un projecteur comme la version JavaScript (`Object.assign`
    /// sur `settings` et `state`). L'application passe plutôt par `Document`.
    pub fn apply_to(&self, f: &mut Fixture) {
        apply_shallow(f, &self.settings, &self.state);
    }
}

/// `Object.assign(f.settings, settings); Object.assign(f.state, state)`
pub fn apply_shallow(f: &mut Fixture, settings: &Map<String, Value>, state: &Map<String, Value>) {
    if !settings.is_empty() {
        let mut all = match serde_json::to_value(&f.settings) {
            Ok(Value::Object(m)) => m,
            _ => Map::new(),
        };
        all.extend(settings.iter().map(|(k, v)| (k.clone(), v.clone())));
        if let Ok(s) = serde_json::from_value(Value::Object(all)) {
            f.settings = s;
        }
    }
    if !state.is_empty() {
        if !f.state.is_object() {
            f.state = Value::Object(Map::new());
        }
        if let Value::Object(m) = &mut f.state {
            m.extend(state.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
    }
}

/// Copie de travail d'un projecteur pendant un appel : chaque changement émis y est appliqué
/// aussitôt (les lectures suivantes le voient, comme dans la version JavaScript) et noté.
#[derive(Debug, Clone, Default)]
pub(crate) struct Fx {
    pub id: String,
    pub rotation: [f64; 3],
    pub settings: Map<String, Value>,
    pub state: Map<String, Value>,
    pub out: Outcome,
}

impl Fx {
    pub fn of(f: &Fixture) -> Fx {
        let settings = match serde_json::to_value(&f.settings) {
            Ok(Value::Object(m)) => m,
            _ => Map::new(),
        };
        Fx { id: f.id.clone(), rotation: f.rotation, settings, state: f.state.as_object().cloned().unwrap_or_default(), out: Outcome::default() }
    }

    /// Réglage (null compris ; absent : None).
    pub fn s(&self, key: &str) -> Option<&Value> {
        self.settings.get(key)
    }

    pub fn st(&self, key: &str) -> Option<&Value> {
        self.state.get(key)
    }

    /// `emit({ settings })`
    pub fn set(&mut self, changes: Map<String, Value>) {
        for (k, v) in changes {
            self.settings.insert(k.clone(), v.clone());
            self.out.settings.insert(k, v);
        }
    }

    /// `emit({ state })`
    pub fn set_state(&mut self, changes: Map<String, Value>) {
        for (k, v) in changes {
            self.state.insert(k.clone(), v.clone());
            self.out.state.insert(k, v);
        }
    }

    pub fn set1(&mut self, key: &str, v: Value) {
        self.set(obj([(key, v)]));
    }

    pub fn set_state1(&mut self, key: &str, v: Value) {
        self.set_state(obj([(key, v)]));
    }

    /// Projecteur allumé (`!!state.power`).
    pub fn power(&self) -> bool {
        js::truthy(self.st("power"))
    }

    pub fn take(&mut self) -> Outcome {
        std::mem::take(&mut self.out)
    }
}

/// Objet JSON à partir de paires.
pub(crate) fn obj<const N: usize>(pairs: [(&str, Value); N]) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}
