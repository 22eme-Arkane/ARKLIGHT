//! Contrôleur MIDI de l'application (portage de la logique de `web/js/midi.js` et de la fenêtre
//! `web/js/ui/midi-panel.js`, sans l'interface) : un contrôleur branché en USB — boîte de faders,
//! DMX Operator 384 en mode MIDI… — pilote la console ARKLIGHT, l'intensité des groupes, la
//! sélection et les commandes du menu.
//!
//! Les correspondances sont gardées sur ce poste (`settings` / `from_settings` : le contrôleur est
//! celui du PC, pas du projet) ; les groupes et projecteurs y sont désignés par leur nom (ou leur
//! rang), pour servir dans tous les projets. Un préréglage (console connue, ou préréglage enregistré
//! par l'utilisateur) forme une couche : les correspondances apprises à la main passent avant lui.
//!
//! [`Midi::receive`] ne touche pas au projet : il renvoie des [`Action`] que l'application applique
//! (fader de la console, intensité, couleur, sélection, plan, ambiance, commande du menu).

use crate::message::{Held, Mapping, MidiMessage, MsgType, RouteEvent, Source, Target, base36, clean_mappings, describe_source, is_continuous, js_slice, new_id, parse_midi, route_midi, scale_value};
use crate::presets::{MIDI_PRESETS, Preset, preset_by_id, preset_label, preset_mappings};
use arklight_core::engine::js_round;
use arklight_core::project::Group;
use arklight_core::{Fixture, Project};
use serde_json::{Value, json};
use std::collections::HashSet;

/// Format des fichiers de préréglage partagés.
pub const PRESET_FORMAT: &str = "arklight-midi-preset";
/// faders appris à la suite au plus (fenêtre MIDI)
pub const MAX_FADERS: u32 = 32;
/// touches apprises à la suite (les 12 touches « projecteur » d'une DMX Operator 384)
pub const MAX_KEYS: usize = 12;

/// Un appareil MIDI branché.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
}

/// Préréglage actif : identifiant (intégré ou enregistré) et premier canal MIDI (intégrés).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePreset {
    pub id: String,
    pub channel: i32,
}

/// Préréglage enregistré par l'utilisateur.
#[derive(Debug, Clone, PartialEq)]
pub struct UserPreset {
    pub id: String,
    pub name: String,
    pub mappings: Vec<Mapping>,
}

/// Préréglage actif : intégré ou enregistré.
#[derive(Debug, Clone, Copy)]
pub enum PresetRef<'a> {
    Builtin(&'static Preset),
    User(&'a UserPreset),
}

impl PresetRef<'_> {
    pub fn id(&self) -> &str {
        match self {
            PresetRef::Builtin(p) => p.id,
            PresetRef::User(p) => &p.id,
        }
    }

    /// Nom affiché (« ADJ DMX Operator 384 — logiciel V2.6 et suivants », ou le nom choisi).
    pub fn label(&self) -> String {
        match self {
            PresetRef::Builtin(p) => preset_label(p),
            PresetRef::User(p) => p.name.clone(),
        }
    }

    pub fn is_builtin(&self) -> bool {
        matches!(self, PresetRef::Builtin(_))
    }
}

/// Repère d'un fader de la console commandé par le contrôleur (« B1·3 » d'un préréglage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaderMark {
    pub index: u32,
    pub label: Option<String>,
}

/// Couleur de la sélection réglée par un fader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorChange {
    /// Kelvin (2000..10000, par 50)
    Cct(f64),
    /// teinte ±vert (−1..1)
    Tint(f64),
    /// teinte 0..360°
    Hue(f64),
    /// saturation 0..1
    Sat(f64),
}

/// Ambiance de la vue réglée par un fader.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvChange {
    /// brume 0..1
    Haze(f64),
    /// exposition (multiplicateur : 2^IL)
    Exposure(f64),
    /// lumière ambiante 0..0,5
    Ambient(f64),
}

/// Ce que l'application doit faire après un message MIDI.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// sélection remplacée (projecteurs, dans l'ordre ; principal : celui de l'inspecteur)
    Select { ids: Vec<String>, primary: Option<String> },
    /// blackout général (tout dans le noir, les réglages ne bougent pas)
    Blackout(bool),
    /// fader de la console (n° 1.., valeur 0..1)
    Fader { index: u32, value: f64 },
    /// faders de la console commandés par le contrôleur (repères de l'onglet Console) et page
    FaderMarks { marks: Vec<FaderMark>, page: u32 },
    /// modèle suivant (onglet Console, groupe de plusieurs modèles)
    NextFamily,
    /// flash des projecteurs (vrai : plein feu tant qu'on appuie)
    Flash { fixtures: Vec<String>, on: bool },
    /// intensité de projecteurs (0..1)
    Intensity { fixtures: Vec<String>, value: f64 },
    /// couleur de projecteurs
    Color { fixtures: Vec<String>, change: ColorChange },
    /// ouvre un plan du projet
    OpenPlan(String),
    Env(EnvChange),
    /// commande d'ARKLIGHT (menu)
    Command(String),
    /// correspondances changées (une commande vient d'être apprise) : à enregistrer (`settings`)
    SettingsChanged,
}

/// Ce que le contrôleur sait du projet ouvert au moment d'un message.
#[derive(Debug, Clone, Copy)]
pub struct MidiContext<'a> {
    /// projecteurs de la scène (ordre du projet)
    pub fixtures: &'a [Fixture],
    pub groups: &'a [Group],
    /// projecteurs dans l'ordre de la liste Scène (tri choisi) ; None : ordre du projet
    pub fixture_order: Option<&'a [String]>,
    /// projecteurs sélectionnés (dans l'ordre où ils l'ont été)
    pub selection: &'a [String],
    /// projecteur principal de la sélection (inspecteur)
    pub selected_id: Option<&'a str>,
    /// plans du projet, dans l'ordre (voir [`plan_ids`])
    pub plans: &'a [String],
    /// nombre de faders de l'onglet Console
    pub fader_count: usize,
}

impl<'a> MidiContext<'a> {
    /// Contexte d'un projet (sans sélection, plans ni faders : à compléter).
    pub fn new(project: &'a Project) -> Self {
        MidiContext { fixtures: &project.fixtures, groups: &project.groups, fixture_order: None, selection: &[], selected_id: None, plans: &[], fader_count: 0 }
    }

    fn has(&self, id: &str) -> bool {
        self.fixtures.iter().any(|f| f.id == id)
    }

    /// Projecteurs dans l'ordre de la liste Scène.
    fn order(&self) -> Vec<&'a str> {
        match self.fixture_order {
            Some(o) => o.iter().map(String::as_str).collect(),
            None => self.fixtures.iter().map(|f| f.id.as_str()).collect(),
        }
    }

    fn group_by_name(&self, name: Option<&str>) -> Option<&'a Group> {
        let name = name?;
        self.groups.iter().find(|g| g.name == name)
    }

    fn fixture_by_name(&self, name: Option<&str>) -> Option<&'a Fixture> {
        let name = name?;
        self.fixtures.iter().find(|f| f.name == name)
    }
}

/// Plans du projet dans l'ordre (séquences puis plans), comme `flatPlans`.
pub fn plan_ids(project: &Project) -> Vec<String> {
    let seqs = project.extra.get("sequences").and_then(Value::as_array);
    seqs.into_iter().flatten().flat_map(|s| s.get("plans").and_then(Value::as_array).into_iter().flatten()).filter_map(|p| p.get("id").and_then(Value::as_str).map(str::to_owned)).collect()
}

/// Apprentissage en cours (fenêtre MIDI).
#[derive(Debug, Clone, PartialEq)]
pub enum Learning {
    /// la prochaine commande du contrôleur devient une nouvelle correspondance
    New,
    /// la prochaine commande remplace celle de cette correspondance (id)
    Relearn(String),
    /// assistant : faders, groupes ou projecteurs appris à la suite
    Wizard(Wizard),
}

/// Sorte d'assistant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardKind {
    /// faders 1 → 8 de la console
    Faders,
    /// touches → groupes (ordre de la liste Scène)
    Groups,
    /// touches → projecteurs (12 au plus)
    Fixtures,
}

/// Élément à apprendre : fader n°, ou nom du groupe / du projecteur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WizardItem {
    Fader(u32),
    Name(String),
}

impl std::fmt::Display for WizardItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WizardItem::Fader(n) => write!(f, "{n}"),
            WizardItem::Name(s) => f.write_str(s),
        }
    }
}

/// Assistant d'apprentissage en cours.
#[derive(Debug, Clone, PartialEq)]
pub struct Wizard {
    pub kind: WizardKind,
    pub items: Vec<WizardItem>,
    /// prochain élément à apprendre
    pub next: usize,
    /// commandes déjà apprises (refusées pour la suite)
    keys: HashSet<String>,
}

/// Sélection pendant le traitement d'un message (comme `Store.setSelection` : une sélection qui
/// change remet les faders à la première page).
struct Run<'c> {
    ctx: &'c MidiContext<'c>,
    ids: Vec<String>,
    primary: Option<String>,
    actions: Vec<Action>,
    /// repères des faders à rafraîchir à la fin (sélection ou correspondances changées)
    deferred: bool,
}

impl Run<'_> {
    /// Projecteurs sélectionnés, dans l'ordre de la scène.
    fn selected(&self) -> Vec<String> {
        self.ctx.fixtures.iter().filter(|f| self.ids.contains(&f.id)).map(|f| f.id.clone()).collect()
    }
}

/// Contrôleur MIDI : correspondances, préréglage, état des touches, page de faders, apprentissage.
#[derive(Debug, Clone, Default)]
pub struct Midi {
    /// page de faders : les faders du contrôleur commandent les faders (page × taille) + n de la console
    pub page: u32,
    /// appareils branchés
    pub inputs: Vec<DeviceInfo>,
    /// dernier message reçu et son appareil
    pub last: Option<(MidiMessage, String)>,
    /// préréglage proposé pour un contrôleur connu branché (rien n'est réglé)
    pub suggest: Option<String>,
    mappings: Vec<Mapping>,
    preset: Option<ActivePreset>,
    user_presets: Vec<UserPreset>,
    held: Held,
    /// touches à bascule allumées (Fixture, Chase…) : clé, fonction, n° (dans l'ordre d'allumage)
    latched: Vec<(String, String, u32)>,
    learning: Option<Learning>,
    layer: Vec<Mapping>,
    all: Vec<Mapping>,
}

/// Texte d'une valeur comme `String(v)` en JavaScript (approché pour les objets).
fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        Value::Array(a) => a.iter().map(|x| if x.is_null() { String::new() } else { js_string(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
        other => other.to_string(),
    }
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// Correspondance « partageable » : commande, fonction, repère (sans identifiant).
fn bare(m: &Mapping) -> Mapping {
    Mapping { id: String::new(), source: m.source, target: m.target.clone(), label: m.label.clone().filter(|l| !l.is_empty()), preset: false }
}

fn now_base36() -> String {
    base36(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64))
}

impl Midi {
    pub fn new() -> Self {
        Self::default()
    }

    // ------------------------------------------------------------------ réglages du poste

    /// Lit les réglages enregistrés (`{ mappings, preset, userPresets }`) ; ce qui est mal formé est écarté.
    pub fn from_settings(v: &Value) -> Self {
        let mut m = Midi { mappings: clean_mappings(&v["mappings"]), ..Default::default() };
        m.preset = v["preset"].as_object().and_then(|p| {
            let id = p.get("id")?.as_str().filter(|s| !s.is_empty())?;
            let channel = p.get("channel").filter(|c| c.is_number()).map_or(1, |c| crate::message::js_int(Some(c)));
            Some(ActivePreset { id: id.to_owned(), channel })
        });
        m.user_presets = v["userPresets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                let name = p.get("name")?.as_str()?;
                let id = p.get("id").filter(|x| truthy(Some(x))).map_or_else(|| format!("user:{name}"), js_string);
                Some(UserPreset { id, name: js_slice(name, 60), mappings: clean_mappings(&p["mappings"]) })
            })
            .collect();
        m.rebuild();
        m
    }

    /// Réglages à enregistrer sur le poste : `{ mappings, preset, userPresets }`.
    pub fn settings(&self) -> Value {
        let maps = |l: &[Mapping]| Value::Array(l.iter().map(Mapping::to_json).collect());
        json!({
            "mappings": maps(&self.mappings),
            "preset": self.preset.as_ref().map(|p| json!({ "id": p.id, "channel": p.channel })),
            "userPresets": self.user_presets.iter().map(|p| json!({ "id": p.id, "name": p.name, "mappings": maps(&p.mappings) })).collect::<Vec<_>>(),
        })
    }

    /// Correspondances apprises à la main.
    pub fn mappings(&self) -> &[Mapping] {
        &self.mappings
    }

    pub fn preset(&self) -> Option<&ActivePreset> {
        self.preset.as_ref()
    }

    pub fn user_presets(&self) -> &[UserPreset] {
        &self.user_presets
    }

    /// Recalcule la couche du préréglage et l'ensemble (après chaque changement).
    fn rebuild(&mut self) {
        self.layer = match self.active_preset() {
            None => Vec::new(),
            Some(PresetRef::Builtin(p)) => preset_mappings(p, Some(self.preset.as_ref().map_or(1, |a| a.channel))),
            Some(PresetRef::User(p)) => p.mappings.iter().enumerate().map(|(i, m)| Mapping { id: format!("{}:{i}", p.id), preset: true, ..m.clone() }).collect(),
        };
        let own: HashSet<String> = self.mappings.iter().filter_map(|m| m.source.map(|s| s.key())).collect();
        let mut all = self.mappings.clone();
        all.extend(self.layer.iter().filter(|m| !m.source.is_some_and(|s| own.contains(&s.key()))).cloned());
        self.all = all;
    }

    /// Préréglage actif : intégré ou enregistré par l'utilisateur.
    pub fn active_preset(&self) -> Option<PresetRef<'_>> {
        let id = &self.preset.as_ref()?.id;
        preset_by_id(id).map(PresetRef::Builtin).or_else(|| self.user_presets.iter().find(|p| p.id == *id).map(PresetRef::User))
    }

    /// Correspondances du préréglage actif.
    pub fn layer(&self) -> &[Mapping] {
        &self.layer
    }

    /// Toutes les correspondances : les siennes, puis celles du préréglage pour les autres commandes.
    pub fn all(&self) -> &[Mapping] {
        &self.all
    }

    // ------------------------------------------------------------------ appareils

    /// Appareils branchés : un contrôleur connu est branché et rien n'est réglé → préréglage proposé.
    pub fn set_inputs(&mut self, list: Vec<DeviceInfo>) {
        self.suggest = if self.preset.is_none() && self.mappings.is_empty() {
            MIDI_PRESETS.iter().find(|p| list.iter().any(|d| p.matches(&format!("{} {}", d.name, d.manufacturer)))).map(|p| p.id.to_owned())
        } else {
            None
        };
        self.inputs = list;
    }

    // ------------------------------------------------------------------ pages de faders

    fn fader_maps(&self) -> impl Iterator<Item = &Mapping> {
        self.all.iter().filter(|m| m.target.kind == "fader")
    }

    /// Faders du contrôleur par page (le plus grand n° de fader commandé).
    pub fn page_size(&self) -> u32 {
        self.fader_maps().map(|m| m.target.index_or_1()).max().unwrap_or(1).max(1)
    }

    /// Faders de la console commandés en ce moment, triés.
    pub fn fader_marks(&self) -> Vec<FaderMark> {
        let size = self.page_size();
        let mut marks: Vec<FaderMark> = self.fader_maps().map(|m| FaderMark { index: m.target.index_or_1() + self.page * size, label: m.label.clone().filter(|l| !l.is_empty()) }).collect();
        marks.sort_by_key(|m| m.index);
        marks
    }

    pub fn fader_indexes(&self) -> Vec<u32> {
        self.fader_marks().iter().map(|m| m.index).collect()
    }

    fn marks_action(&self) -> Action {
        Action::FaderMarks { marks: self.fader_marks(), page: self.page }
    }

    /// Page de faders suivante (`step` 1) ou précédente (−1), bornée par les faders de la console.
    pub fn turn_page(&mut self, step: i32, fader_count: usize) -> Action {
        let size = self.page_size() as i64;
        let last = ((fader_count as i64 + size - 1) / size - 1).max(0);
        self.page = (self.page as i64 + step as i64).min(last).max(0) as u32;
        self.marks_action()
    }

    /// La sélection a changé (dans l'application) : on repart des premiers faders. Vrai si la page change.
    pub fn selection_changed(&mut self) -> bool {
        std::mem::replace(&mut self.page, 0) != 0
    }

    // ------------------------------------------------------------------ messages reçus

    /// Un message reçu (d'un contrôleur, ou simulé) → ce que l'application doit faire.
    pub fn receive(&mut self, data: &[u8], device: &str, ctx: &MidiContext) -> Vec<Action> {
        let Some(msg) = parse_midi(data) else { return Vec::new() };
        self.last = Some((msg, device.to_owned()));
        let mut run = Run { ctx, ids: ctx.selection.to_vec(), primary: ctx.selected_id.map(str::to_owned), actions: Vec::new(), deferred: false };
        if self.learning.is_some() {
            self.learn_message(&msg, ctx, &mut run);
        } else {
            let routes: Vec<(Target, RouteEvent)> = route_midi(&self.all, &msg, &mut self.held).into_iter().map(|r| (r.mapping.target.clone(), r.event)).collect();
            for (t, event) in routes {
                self.apply(&t, event, &mut run);
            }
        }
        if run.deferred {
            let a = self.marks_action();
            run.actions.push(a);
        }
        run.actions
    }

    /// Remplace la sélection (comme `Store.setSelection` : projecteurs connus, sans doublon ;
    /// principal : `primary` s'il en fait partie, sinon le dernier) ; rien si elle ne change pas.
    fn set_selection(&mut self, run: &mut Run, ids: Vec<String>, primary: Option<String>) {
        let mut next: Vec<String> = Vec::new();
        for id in ids {
            if run.ctx.has(&id) && !next.contains(&id) {
                next.push(id);
            }
        }
        let p = primary.filter(|p| next.contains(p)).or_else(|| next.last().cloned());
        if p == run.primary && next.len() == run.ids.len() && next.iter().all(|id| run.ids.contains(id)) {
            return;
        }
        run.ids = next.clone();
        run.primary = p.clone();
        run.actions.push(Action::Select { ids: next, primary: p });
        self.page = 0; // nouvelle sélection : on repart des premiers faders
        run.deferred = true;
    }

    fn select(&mut self, run: &mut Run, id: &str) {
        self.set_selection(run, vec![id.to_owned()], Some(id.to_owned()));
    }

    fn select_group(&mut self, run: &mut Run, gid: &str) {
        let Some(g) = run.ctx.groups.iter().find(|g| g.id == gid) else { return };
        let ids: Vec<String> = g.ids.iter().filter(|id| run.ctx.has(id)).cloned().collect();
        let first = run.ctx.fixtures.iter().find(|f| ids.contains(&f.id)).map(|f| f.id.clone());
        self.set_selection(run, ids, first);
    }

    /// La sélection suit les touches allumées : leurs projecteurs, et les membres de leurs groupes.
    fn apply_latched(&mut self, run: &mut Run) {
        let order = run.ctx.order();
        let mut ids: Vec<String> = Vec::new();
        let mut add = |id: &str| {
            if !ids.iter().any(|x| x == id) {
                ids.push(id.to_owned());
            }
        };
        for (_, kind, index) in &self.latched {
            let i = *index as usize;
            if kind == "fixtureLatch"
                && let Some(id) = i.checked_sub(1).and_then(|i| order.get(i))
            {
                add(id);
            }
            if kind == "groupLatch"
                && let Some(g) = i.checked_sub(1).and_then(|i| run.ctx.groups.get(i))
            {
                for id in &g.ids {
                    add(id);
                }
            }
        }
        self.set_selection(run, ids, None);
    }

    /// Sélection suivante / précédente dans une liste (boucle).
    fn cycle<'b>(ids: &[&'b str], current: Option<&str>, step: i32) -> Option<&'b str> {
        if ids.is_empty() {
            return None;
        }
        let n = ids.len() as i64;
        match ids.iter().position(|x| Some(*x) == current) {
            None => Some(if step > 0 { ids[0] } else { ids[ids.len() - 1] }),
            Some(i) => Some(ids[((i as i64 + step as i64 + n) % n) as usize]),
        }
    }

    fn apply(&mut self, t: &Target, event: RouteEvent, run: &mut Run) {
        let ctx = run.ctx;
        let value = if let RouteEvent::Value(v) = event { v } else { 0.0 };
        let press = event == RouteEvent::Press;
        let release = event == RouteEvent::Release;
        let latch = event == RouteEvent::Latch(true);
        let index = t.index_or_1();
        let name = t.name.as_deref();
        let at = |n: u32| (n as usize).checked_sub(1);
        match t.kind.as_str() {
            "fixtureLatch" | "groupLatch" => {
                let key = format!("{}:{index}", t.kind);
                let pos = self.latched.iter().position(|e| e.0 == key);
                match (latch, pos) {
                    (true, Some(i)) => self.latched[i] = (key, t.kind.clone(), index),
                    (true, None) => self.latched.push((key, t.kind.clone(), index)),
                    (false, Some(i)) => {
                        self.latched.remove(i);
                    }
                    (false, None) => {}
                }
                self.apply_latched(run);
            }
            "blackout" => run.actions.push(Action::Blackout(latch)),
            "fader" => run.actions.push(Action::Fader { index: index + self.page * self.page_size(), value }),
            "pageNext" => run.actions.push(self.turn_page(1, ctx.fader_count)),
            "pagePrev" => run.actions.push(self.turn_page(-1, ctx.fader_count)),
            "familyNext" => run.actions.push(Action::NextFamily),
            "flash" => run.actions.push(Action::Flash { fixtures: run.selected(), on: press && !release }),
            "groupLevel" => {
                if let Some(g) = ctx.group_by_name(name) {
                    run.actions.push(Action::Intensity { fixtures: g.ids.iter().filter(|id| ctx.has(id)).cloned().collect(), value });
                }
            }
            "fixtureLevel" => {
                if let Some(f) = ctx.fixture_by_name(name) {
                    run.actions.push(Action::Intensity { fixtures: vec![f.id.clone()], value });
                }
            }
            "selectionLevel" => run.actions.push(Action::Intensity { fixtures: run.selected(), value }),
            "selCct" => run.actions.push(Action::Color { fixtures: run.selected(), change: ColorChange::Cct(scale_value("selCct", value)) }),
            "selTint" => run.actions.push(Action::Color { fixtures: run.selected(), change: ColorChange::Tint(scale_value("selTint", value)) }),
            "selHue" => run.actions.push(Action::Color { fixtures: run.selected(), change: ColorChange::Hue(scale_value("selHue", value)) }),
            "selSat" => run.actions.push(Action::Color { fixtures: run.selected(), change: ColorChange::Sat(value) }),
            "group" => {
                if let Some(g) = ctx.group_by_name(name) {
                    self.select_group(run, &g.id);
                }
            }
            "fixture" => {
                if let Some(f) = ctx.fixture_by_name(name) {
                    self.select(run, &f.id);
                }
            }
            "fixtureIndex" => {
                if let Some(id) = at(index).and_then(|i| ctx.order().get(i).copied()) {
                    self.select(run, id);
                }
            }
            "groupIndex" => {
                if let Some(g) = at(index).and_then(|i| ctx.groups.get(i)) {
                    self.select_group(run, &g.id);
                }
            }
            "planIndex" => {
                if let Some(p) = at(index).and_then(|i| ctx.plans.get(i)) {
                    run.actions.push(Action::OpenPlan(p.clone()));
                }
            }
            "groupNext" | "groupPrev" => {
                let groups: Vec<&str> = ctx.groups.iter().filter(|g| !g.ids.is_empty()).map(|g| g.id.as_str()).collect();
                let current = ctx.groups.iter().find(|g| !g.ids.is_empty() && g.ids.len() == run.ids.len() && g.ids.iter().all(|id| run.ids.contains(id))).map(|g| g.id.as_str());
                if let Some(next) = Self::cycle(&groups, current, if t.kind == "groupNext" { 1 } else { -1 }) {
                    self.select_group(run, next);
                }
            }
            "fixtureNext" | "fixturePrev" => {
                let order = ctx.order();
                let current = run.primary.clone();
                if let Some(next) = Self::cycle(&order, current.as_deref(), if t.kind == "fixtureNext" { 1 } else { -1 }) {
                    self.select(run, next);
                }
            }
            "haze" => run.actions.push(Action::Env(EnvChange::Haze(js_round(value * 100.0) / 100.0))),
            "exposure" => run.actions.push(Action::Env(EnvChange::Exposure(2f64.powf(scale_value("exposure", value))))),
            "ambient" => run.actions.push(Action::Env(EnvChange::Ambient(scale_value("ambient", value)))),
            "command" => {
                if let Some(id) = t.id.as_ref().filter(|id| !id.is_empty()) {
                    run.actions.push(Action::Command(id.clone()));
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------ correspondances

    /// Après un changement des correspondances ou du préréglage.
    fn save(&mut self) {
        self.rebuild();
    }

    /// Ajoute une correspondance (identifiant neuf) ; renvoie son identifiant.
    pub fn add(&mut self, source: Option<Source>, target: Target) -> String {
        let id = new_id("m");
        self.mappings.push(Mapping { id: id.clone(), ..Mapping::new(source, target) });
        self.save();
        id
    }

    /// Modifie une correspondance (commande, fonction, repère).
    pub fn update(&mut self, id: &str, change: impl FnOnce(&mut Mapping)) {
        if let Some(m) = self.mappings.iter_mut().find(|m| m.id == id) {
            change(m);
        }
        self.save();
    }

    pub fn remove(&mut self, id: &str) {
        self.mappings.retain(|m| m.id != id);
        self.save();
    }

    /// Efface toutes les correspondances apprises (le préréglage reste).
    pub fn clear(&mut self) {
        self.mappings.clear();
        self.save();
    }

    /// Remplace ce qui commandait déjà la même chose, ou ce que faisait déjà ce fader / cette touche.
    pub fn assign(&mut self, source: Source, target: Target) {
        let key = source.key();
        let same = |m: &Mapping| m.target.kind == target.kind && if target.kind == "fader" { m.target.index == target.index } else { m.target.name == target.name };
        let gone: Vec<String> = self.mappings.iter().filter(|m| m.source.is_some_and(|s| s.key() == key) || same(m)).map(|m| m.id.clone()).collect();
        for id in gone {
            self.remove(&id);
        }
        self.add(Some(source), target);
    }

    // ------------------------------------------------------------------ préréglages

    /// Active un préréglage (None : aucun) ; `channel` : premier canal MIDI d'un préréglage intégré.
    pub fn set_preset(&mut self, id: Option<&str>, channel: Option<i32>) {
        let found: Option<(String, i32)> = id.and_then(|id| match preset_by_id(id) {
            Some(p) => Some((p.id.to_owned(), p.channel)),
            None => self.user_presets.iter().find(|x| x.id == id).map(|p| (p.id.clone(), 1)),
        });
        self.preset = found.map(|(id, default)| ActivePreset { id, channel: channel.or(self.preset.as_ref().map(|p| p.channel)).unwrap_or(default).clamp(1, 16) });
        self.suggest = None;
        self.save();
    }

    fn user_preset_id(&self) -> String {
        let base = format!("user:{}", now_base36());
        let mut id = base.clone();
        let mut n = 1;
        while self.user_presets.iter().any(|p| p.id == id) {
            id = format!("{base}{n}");
            n += 1;
        }
        id
    }

    /// Enregistre l'ensemble en cours (préréglage + correspondances apprises) comme préréglage
    /// personnel, qui devient le préréglage actif ; renvoie son identifiant.
    pub fn save_user_preset(&mut self, name: &str) -> String {
        let id = self.user_preset_id();
        let mappings = self.all.iter().map(bare).collect();
        self.user_presets.push(UserPreset { id: id.clone(), name: js_slice(name, 60), mappings });
        self.preset = Some(ActivePreset { id: id.clone(), channel: 1 });
        self.mappings.clear(); // tout est maintenant dans le préréglage
        self.save();
        id
    }

    pub fn delete_user_preset(&mut self, id: &str) {
        self.user_presets.retain(|p| p.id != id);
        if self.preset.as_ref().is_some_and(|p| p.id == id) {
            self.preset = None;
        }
        self.save();
    }

    /// Fichier de préréglage à partager (autre poste, autre utilisateur de la même console).
    pub fn export_preset(&self, name: &str) -> Value {
        json!({ "format": PRESET_FORMAT, "version": 1, "name": name, "mappings": self.all.iter().map(|m| bare(m).to_json()).collect::<Vec<_>>() })
    }

    /// Ajoute un préréglage lu dans un fichier (il devient actif) ; renvoie son nom, ou l'erreur.
    pub fn import_preset(&mut self, json: &Value) -> Result<String, String> {
        if json.get("format").and_then(Value::as_str) != Some(PRESET_FORMAT) || !json["mappings"].is_array() {
            return Err("Ce fichier n’est pas un préréglage MIDI ARKLIGHT".into());
        }
        let mappings: Vec<Mapping> = clean_mappings(&json["mappings"]).into_iter().filter(|m| m.source.is_some()).collect();
        if mappings.is_empty() {
            return Err("Ce préréglage ne contient aucune commande".into());
        }
        let name = js_slice(&json.get("name").filter(|n| truthy(Some(n))).map_or("Préréglage importé".into(), js_string), 60);
        let id = self.user_preset_id();
        self.user_presets.push(UserPreset { id: id.clone(), name: name.clone(), mappings });
        self.preset = Some(ActivePreset { id, channel: 1 });
        self.save();
        Ok(name)
    }

    // ------------------------------------------------------------------ apprentissage (fenêtre MIDI)

    pub fn learning(&self) -> Option<&Learning> {
        self.learning.as_ref()
    }

    pub fn stop_learning(&mut self) {
        self.learning = None;
    }

    /// La prochaine commande du contrôleur devient une nouvelle correspondance.
    pub fn learn_new(&mut self) {
        self.learning = Some(Learning::New);
    }

    /// La prochaine commande remplace celle de la correspondance `id`.
    pub fn relearn(&mut self, id: &str) {
        self.learning = Some(Learning::Relearn(id.to_owned()));
    }

    /// Assistant : apprend à la suite les faders 1 à 8 de la console, les groupes ou les projecteurs
    /// (12 au plus) ; le relance arrête un assistant de la même sorte. Faux si rien à apprendre.
    pub fn toggle_wizard(&mut self, kind: WizardKind, ctx: &MidiContext) -> bool {
        if matches!(&self.learning, Some(Learning::Wizard(w)) if w.kind == kind) {
            self.stop_learning();
            return false;
        }
        let items: Vec<WizardItem> = match kind {
            WizardKind::Faders => (1..=8).map(WizardItem::Fader).collect(),
            WizardKind::Groups => ctx.groups.iter().map(|g| WizardItem::Name(g.name.clone())).collect(),
            WizardKind::Fixtures => ctx.order().iter().filter_map(|id| ctx.fixtures.iter().find(|f| f.id == *id)).map(|f| f.name.clone()).filter(|n| !n.is_empty()).take(MAX_KEYS).map(WizardItem::Name).collect(),
        };
        if items.is_empty() {
            return false;
        }
        self.learning = Some(Learning::Wizard(Wizard { kind, items, next: 0, keys: HashSet::new() }));
        true
    }

    /// Fonction proposée pour une commande qu'on vient d'apprendre : fader suivant de la console
    /// (32 au plus) pour un fader, groupe pas encore commandé pour une touche.
    pub fn default_target(&self, source: &Source, ctx: &MidiContext) -> Target {
        let used = &self.mappings;
        if is_continuous(source.kind) {
            let next = used.iter().filter(|m| m.target.kind == "fader").map(|m| m.target.index.unwrap_or(0)).max().unwrap_or(0) + 1;
            return if next <= MAX_FADERS { Target::with_index("fader", next) } else { Target::new("selectionLevel") };
        }
        let free = ctx.groups.iter().find(|g| !used.iter().any(|m| m.target.kind == "group" && m.target.name.as_deref() == Some(g.name.as_str())));
        free.map_or_else(|| Target::command("console.full"), |g| Target::with_name("group", &g.name))
    }

    /// Un message pendant l'apprentissage (les relâchements de note sont ignorés).
    fn learn_message(&mut self, msg: &MidiMessage, ctx: &MidiContext, run: &mut Run) {
        if msg.kind == MsgType::Note && !msg.on {
            return;
        }
        let source = msg.source();
        match self.learning.clone() {
            Some(Learning::New) => {
                let target = self.default_target(&source, ctx);
                self.add(Some(source), target);
                self.stop_learning();
            }
            Some(Learning::Relearn(id)) => {
                self.update(&id, |m| m.source = Some(source));
                self.stop_learning();
            }
            Some(Learning::Wizard(mut w)) => {
                let key = msg.key();
                let accept = !w.keys.contains(&key) && if w.kind == WizardKind::Faders { is_continuous(msg.kind) } else { msg.is_press() };
                if !accept {
                    return;
                }
                w.keys.insert(key);
                let target = match (&w.items[w.next], w.kind) {
                    (WizardItem::Fader(n), _) => Target::with_index("fader", *n),
                    (WizardItem::Name(n), WizardKind::Groups) => Target::with_name("group", n),
                    (item, _) => Target::with_name("fixture", &item.to_string()),
                };
                self.assign(source, target);
                w.next += 1;
                self.learning = if w.next >= w.items.len() { None } else { Some(Learning::Wizard(w)) };
            }
            None => return,
        }
        run.actions.push(Action::SettingsChanged);
        run.deferred = true;
    }

    /// Fonction proposée quand on choisit une autre sorte de fonction pour une correspondance
    /// (`previous` : la fonction d'avant ; garde son fader, son groupe s'il existe encore…).
    pub fn target_for_kind(kind: &str, previous: &Target, ctx: &MidiContext) -> Target {
        match kind {
            "fader" => Target::with_index(kind, previous.index_or_1()),
            "group" | "groupLevel" => {
                let keep = previous.name.as_ref().filter(|n| !n.is_empty() && ctx.groups.iter().any(|g| g.name == **n));
                Target::with_name(kind, &keep.cloned().unwrap_or_else(|| ctx.groups.first().map(|g| g.name.clone()).unwrap_or_default()))
            }
            "fixture" | "fixtureLevel" => {
                let selected = ctx.selected_id.and_then(|id| ctx.fixtures.iter().find(|f| f.id == id)).map(|f| f.name.as_str()).filter(|n| !n.is_empty());
                Target::with_name(kind, selected.or(ctx.fixtures.first().map(|f| f.name.as_str())).unwrap_or(""))
            }
            "command" => Target::command("console.full"),
            "fixtureIndex" | "groupIndex" | "planIndex" | "fixtureLatch" | "groupLatch" => Target::with_index(kind, 1),
            _ => Target::new(kind),
        }
    }

    // ------------------------------------------------------------------ textes de la fenêtre MIDI

    /// Consigne pendant l'apprentissage (vide pour une correspondance réapprise : elle s'affiche sur sa ligne).
    pub fn learning_status(&self) -> String {
        match &self.learning {
            Some(Learning::New) => "Bouge un fader ou appuie sur une touche du contrôleur…".into(),
            Some(Learning::Wizard(w)) => {
                let item = w.items.get(w.next).map(|i| i.to_string()).unwrap_or_default();
                match w.kind {
                    WizardKind::Faders => format!("Bouge le fader du contrôleur qui doit commander le fader {item} de la console…"),
                    WizardKind::Groups => format!("Appuie sur la touche du contrôleur pour le groupe « {item} » ({} sur {})…", w.next + 1, w.items.len()),
                    WizardKind::Fixtures => format!("Appuie sur la touche du contrôleur pour « {item} » ({} sur {})…", w.next + 1, w.items.len()),
                }
            }
            _ => String::new(),
        }
    }

    /// Dernier message reçu : « CC 7 · canal 1 = 64 — nom de l'appareil ».
    pub fn last_text(&self) -> String {
        let Some((msg, device)) = &self.last else { return "—".into() };
        let mut s = format!("{} = {}", describe_source(&msg.source()), js_round(msg.value * 127.0));
        if !device.is_empty() {
            s.push_str(&format!(" — {device}"));
        }
        s
    }

    /// « Faders du contrôleur → faders 1 à 8 de la console (page 1) » (sans préréglage).
    pub fn page_info(&self) -> String {
        let idx = self.fader_indexes();
        match (idx.first(), idx.last()) {
            (Some(a), Some(b)) if idx.len() <= MAX_FADERS as usize && self.active_preset().is_none() => format!("Faders du contrôleur → faders {a} à {b} de la console (page {})", self.page + 1),
            _ => String::new(),
        }
    }
}

/// Réglage d'une fonction sans paramètre : ce qu'elle commande (fenêtre MIDI).
pub fn param_note(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "selectionLevel" | "selCct" | "selTint" | "selHue" | "selSat" | "flash" => "les projecteurs sélectionnés",
        "blackout" => "tout dans le noir, les réglages ne bougent pas",
        "pageNext" => "faders du contrôleur → canaux suivants de la console",
        "pagePrev" => "faders du contrôleur → canaux précédents de la console",
        "familyNext" => "onglet Console",
        "groupNext" | "groupPrev" | "fixtureNext" | "fixturePrev" => "dans l’ordre de la liste Scène",
        _ => return None,
    })
}
