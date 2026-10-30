//! Projet ouvert et ses modifications, avec annuler / rétablir (portage de
//! `server/scene-store.js` et `server/history.js`). Partagé par l'application PC et l'app mobile :
//! l'interface ne modifie jamais le projet directement, elle envoie des [`Edit`].
//!
//! Annuler : un instantané du projet avant chaque modification (100 étapes au plus). Un geste
//! continu (glisser une poignée, un curseur) ne fait qu'une étape : les modifications de même
//! nature sur les mêmes éléments, à moins de 1,2 s d'intervalle, sont fusionnées.

use crate::library::library;
use crate::project::{Fixture, Group, Project, SceneObject, Vec3};
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const HISTORY_LIMIT: usize = 100;
const MERGE: Duration = Duration::from_millis(1200);

/// Changements d'un projecteur ou d'un objet : champs absents = inchangés ; `settings`, `state`
/// et `props` sont fusionnés champ par champ.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Changes {
    pub position: Option<Vec3>,
    pub rotation: Option<Vec3>,
    pub name: Option<String>,
    pub settings: Option<Map<String, Value>>,
    pub state: Option<Map<String, Value>>,
    pub props: Option<Map<String, Value>>,
}

impl Changes {
    pub fn moved(position: Vec3, rotation: Option<Vec3>) -> Changes {
        Changes { position: Some(position), rotation, ..Changes::default() }
    }

    /// Nature des changements (« position,settings.dmxAddress »), pour fusionner un geste continu.
    fn kinds(&self) -> String {
        let mut k: Vec<String> = Vec::new();
        for (name, present) in [("position", self.position.is_some()), ("rotation", self.rotation.is_some()), ("name", self.name.is_some())] {
            if present {
                k.push(name.into());
            }
        }
        for (name, map) in [("settings", &self.settings), ("state", &self.state), ("props", &self.props)] {
            if let Some(m) = map {
                k.extend(m.keys().map(|s| format!("{name}.{s}")));
            }
        }
        k.sort();
        k.join(",")
    }

    fn only_moves(&self) -> bool {
        (self.position.is_some() || self.rotation.is_some()) && self.name.is_none() && self.settings.is_none() && self.state.is_none() && self.props.is_none()
    }
}

/// Une modification du projet.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    AddFixture { type_id: String, position: Vec3, rotation: Vec3, name: Option<String>, settings: Map<String, Value> },
    UpdateFixture { id: String, changes: Changes },
    UpdateFixtures(Vec<(String, Changes)>),
    RemoveFixtures(Vec<String>),
    AddObject { type_id: String, position: Vec3, rotation: Vec3, name: Option<String>, props: Map<String, Value> },
    UpdateObject { id: String, changes: Changes },
    RemoveObject(String),
    CreateGroup { name: Option<String>, ids: Vec<String> },
    UpdateGroup { id: String, name: Option<String>, ids: Option<Vec<String>> },
    DeleteGroup(String),
    Environment(Map<String, Value>),
    Rename(String),
    /// plan de feu 2D (`plot` du projet : éléments dessinés, image de fond, cartouche), remplacé en
    /// entier ; `key` : geste continu fusionné (glisser une forme), `label` : ce qu'annule l'étape
    Plot { plot: Value, key: Option<String>, label: String },
    /// plusieurs modifications en une seule étape (déplacer une sélection, dupliquer…)
    Batch(Vec<Edit>),
}

impl Edit {
    /// Nature de la modification ; None : jamais fusionnée.
    fn key(&self) -> Option<String> {
        Some(match self {
            Edit::UpdateFixture { id, changes } => format!("fixture|{id}|{}", changes.kinds()),
            Edit::UpdateObject { id, changes } => format!("object|{id}|{}", changes.kinds()),
            Edit::UpdateFixtures(items) => {
                let ids: Vec<&str> = items.iter().map(|(id, _)| id.as_str()).collect();
                let mut kinds: Vec<String> = items.iter().map(|(_, c)| c.kinds()).collect();
                kinds.dedup();
                format!("fixtures|{}|{}", ids.join(","), kinds.join(";"))
            }
            Edit::UpdateGroup { id, name: Some(_), ids: None } => format!("group|{id}|name"),
            Edit::Environment(m) => {
                let mut k: Vec<&str> = m.keys().map(String::as_str).collect();
                k.sort();
                format!("environment|{}", k.join(","))
            }
            Edit::Batch(list) => {
                let keys: Option<Vec<String>> = list.iter().map(Edit::key).collect();
                format!("batch|{}", keys?.join("+"))
            }
            Edit::Plot { key, .. } => key.clone()?,
            _ => return None,
        })
    }

    /// Ce que l'étape annule, pour le menu (« le déplacement »).
    pub fn label(&self) -> String {
        let fixture_change = |list: &[&Changes]| {
            if list.iter().all(|c| c.only_moves()) {
                return "le déplacement".to_owned();
            }
            if list.iter().all(|c| c.name.is_some() && c.kinds() == "name") {
                return "le changement de nom".to_owned();
            }
            let addressing = list.iter().all(|c| c.settings.as_ref().is_some_and(|s| !s.is_empty() && s.keys().all(|k| k == "dmxAddress" || k == "universe")) && c.position.is_none() && c.state.is_none());
            if addressing { "l’adressage DMX".to_owned() } else { "le réglage du projecteur".to_owned() }
        };
        match self {
            Edit::AddFixture { .. } => "l’ajout du projecteur".into(),
            Edit::RemoveFixtures(ids) if ids.len() > 1 => format!("la suppression de {} projecteurs", ids.len()),
            Edit::RemoveFixtures(_) => "la suppression du projecteur".into(),
            Edit::UpdateFixture { changes, .. } => fixture_change(&[changes]),
            Edit::UpdateFixtures(items) => fixture_change(&items.iter().map(|(_, c)| c).collect::<Vec<_>>()),
            Edit::AddObject { type_id, .. } => {
                if library().prop(type_id).is_some() { "l’ajout de l’objet".into() } else { "l’ajout du grip".into() }
            }
            Edit::UpdateObject { changes, .. } => {
                if changes.only_moves() {
                    "le déplacement".into()
                } else if changes.kinds() == "name" {
                    "le changement de nom".into()
                } else {
                    "le réglage de l’objet".into()
                }
            }
            Edit::RemoveObject(_) => "la suppression de l’objet".into(),
            Edit::CreateGroup { .. } => "la création du groupe".into(),
            Edit::UpdateGroup { .. } => "la modification du groupe".into(),
            Edit::DeleteGroup(_) => "la suppression du groupe".into(),
            Edit::Environment(_) => "le réglage de l’ambiance".into(),
            Edit::Rename(_) => "le changement de nom".into(),
            Edit::Plot { label, .. } => label.clone(),
            Edit::Batch(list) => match list.first() {
                Some(Edit::AddFixture { .. } | Edit::AddObject { .. }) if list.len() > 1 => format!("l’ajout de {} éléments", list.len()),
                Some(Edit::RemoveFixtures(_) | Edit::RemoveObject(_)) if list.len() > 1 => "la suppression de la sélection".into(),
                Some(first) => first.label(),
                None => "la modification".into(),
            },
        }
    }
}

/// `deepMerge` : les objets sont fusionnés champ par champ, le reste remplacé.
pub fn deep_merge(target: &mut Map<String, Value>, patch: &Map<String, Value>) {
    for (k, v) in patch {
        match (target.get_mut(k), v) {
            (Some(Value::Object(t)), Value::Object(p)) => deep_merge(t, p),
            _ => {
                target.insert(k.clone(), v.clone());
            }
        }
    }
}

fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}{}{}", radix36(t), radix36(n * 7919 % 1_679_616 + 46_656))
}

fn radix36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut s = Vec::new();
    loop {
        s.push(DIGITS[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Nom de groupe : espaces réduits, 40 caractères au plus.
fn group_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(40).collect()
}

/// Numéro de fixture : entier de 1 à 9999, ou rien.
fn clean_fixture_number(v: &Value) -> Value {
    let n = v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()));
    match n.map(crate::engine::js_round) {
        Some(n) if (1.0..=9999.0).contains(&n) => Value::from(n as u32),
        _ => Value::Null,
    }
}

struct Step {
    json: String,
    label: String,
}

/// Le projet ouvert : modifications, annuler / rétablir, enregistrement.
pub struct Document {
    pub project: Project,
    /// fichier du projet (None : jamais enregistré)
    pub path: Option<PathBuf>,
    /// numéro de version du projet en mémoire (change à chaque modification)
    pub revision: u64,
    saved_revision: u64,
    undo: Vec<Step>,
    redo: Vec<Step>,
    last: Option<(String, Instant)>,
}

impl Document {
    pub fn new(mut project: Project, path: Option<PathBuf>) -> Document {
        project.sanitize();
        let mut doc = Document { project, path, revision: 1, saved_revision: 1, undo: Vec::new(), redo: Vec::new(), last: None };
        doc.normalize_plans();
        doc
    }

    /// Oublie annuler / rétablir (autre plan ouvert).
    pub(crate) fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last = None;
    }

    /// Des modifications ne sont pas enregistrées.
    pub fn dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    pub fn mark_saved(&mut self, path: PathBuf) {
        self.path = Some(path);
        self.saved_revision = self.revision;
    }

    fn snapshot(&self) -> String {
        serde_json::to_string(&self.project).unwrap_or_default()
    }

    fn restore(&mut self, json: &str) {
        if let Ok(p) = serde_json::from_str::<Project>(json) {
            self.project = p;
            self.revision += 1;
        }
    }

    /// Applique une modification et l'inscrit dans l'historique ; renvoie l'identifiant d'un
    /// élément créé.
    pub fn apply(&mut self, edit: Edit) -> Option<String> {
        let key = edit.key();
        let now = Instant::now();
        let merge = matches!((&key, &self.last), (Some(k), Some((lk, at))) if k == lk && now.duration_since(*at) < MERGE);
        let before = if merge { None } else { Some(self.snapshot()) };
        let label = edit.label();
        let created = self.perform(edit);
        if merge {
            if let Some((_, at)) = &mut self.last {
                *at = now;
            }
            self.revision += 1;
            return created;
        }
        let before = before.unwrap_or_default();
        if self.snapshot() == before {
            return created; // rien n'a changé
        }
        self.undo.push(Step { json: before, label });
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.last = key.map(|k| (k, now));
        self.revision += 1;
        created
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Ce qu'« Annuler » et « Rétablir » défont ou refont (« le déplacement »).
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    pub fn undo(&mut self) -> bool {
        let Some(step) = self.undo.pop() else { return false };
        self.redo.push(Step { json: self.snapshot(), label: step.label.clone() });
        self.restore(&step.json);
        self.last = None;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(step) = self.redo.pop() else { return false };
        self.undo.push(Step { json: self.snapshot(), label: step.label.clone() });
        self.restore(&step.json);
        self.last = None;
        true
    }

    // ---------------------------------------------------------------- opérations
    pub fn fixture(&self, id: &str) -> Option<&Fixture> {
        self.project.fixtures.iter().find(|f| f.id == id)
    }

    pub fn object(&self, id: &str) -> Option<&SceneObject> {
        self.project.objects.iter().find(|o| o.id == id)
    }

    /// Prochaine adresse libre dans un univers, après le dernier projecteur patché.
    pub fn next_free_address(&self, universe: u32, footprint: usize) -> u32 {
        let lib = library();
        let mut end = 0usize;
        for f in &self.project.fixtures {
            if f.settings.universe != Some(universe) {
                continue;
            }
            let fp = lib.fixture(&f.type_id).and_then(|t| t.mode(f.settings.dmx_mode.as_deref())).map_or(0, |m| m.footprint());
            end = end.max(f.settings.dmx_address.unwrap_or(1) as usize + fp.max(1) - 1);
        }
        if end + footprint <= 512 { end as u32 + 1 } else { 1 }
    }

    /// Prochain numéro de fixture libre (après le plus grand).
    pub fn next_fixture_number(&self) -> u32 {
        self.project.fixtures.iter().filter_map(|f| f.settings.fixture_number).max().unwrap_or(0) + 1
    }

    fn perform(&mut self, edit: Edit) -> Option<String> {
        let lib = library();
        match edit {
            Edit::AddFixture { type_id, position, rotation, name, settings } => {
                let ty = lib.fixture(&type_id)?;
                let count = self.project.fixtures.iter().filter(|f| f.type_id == type_id).count();
                let mut all = Fixture::default_settings(ty);
                all.extend(settings.clone());
                let mut f = Fixture {
                    id: new_id("f"),
                    type_id,
                    name: name.unwrap_or_else(|| format!("{} {}", ty.name, count + 1)),
                    position,
                    rotation,
                    settings: serde_json::from_value(Value::Object(all)).ok()?,
                    state: Value::Object(Fixture::default_state(ty)),
                    extra: Map::new(),
                };
                if !settings.contains_key("dmxAddress") {
                    let fp = ty.mode(f.settings.dmx_mode.as_deref()).map_or(0, |m| m.footprint());
                    f.settings.dmx_address = Some(self.next_free_address(f.settings.universe.unwrap_or(1), fp));
                }
                if !settings.contains_key("fixtureNumber") && !ty.dmx.modes.is_empty() {
                    f.settings.fixture_number = Some(self.next_fixture_number());
                }
                let id = f.id.clone();
                self.project.fixtures.push(f);
                Some(id)
            }
            Edit::UpdateFixture { id, changes } => {
                self.update_fixture(&id, changes);
                None
            }
            Edit::UpdateFixtures(items) => {
                for (id, c) in items {
                    self.update_fixture(&id, c);
                }
                None
            }
            Edit::RemoveFixtures(ids) => {
                self.project.fixtures.retain(|f| !ids.contains(&f.id));
                for g in &mut self.project.groups {
                    g.ids.retain(|x| !ids.contains(x));
                }
                None
            }
            Edit::AddObject { type_id, position, rotation, name, props } => {
                let short = lib.prop(&type_id).map(|t| t.short.clone().unwrap_or_else(|| t.name.clone())).or_else(|| lib.grip(&type_id).map(|g| g.short.clone().unwrap_or_else(|| g.name.clone())))?;
                let count = self.project.objects.iter().filter(|o| o.type_id == type_id).count();
                let mut o = SceneObject {
                    id: new_id("o"),
                    name: name.filter(|n| !n.trim().is_empty()).map_or_else(|| format!("{short} {}", count + 1), |n| n.trim().chars().take(60).collect()),
                    type_id,
                    position,
                    rotation,
                    props: Value::Object(props),
                    extra: Map::new(),
                };
                o.apply_defaults();
                let id = o.id.clone();
                self.project.objects.push(o);
                Some(id)
            }
            Edit::UpdateObject { id, changes } => {
                let o = self.project.objects.iter_mut().find(|o| o.id == id)?;
                if let Some(p) = changes.position {
                    o.position = p;
                }
                if let Some(r) = changes.rotation {
                    o.rotation = r;
                }
                if let Some(n) = changes.name.filter(|n| !n.trim().is_empty()) {
                    o.name = n.trim().chars().take(60).collect();
                }
                if let Some(p) = changes.props {
                    if !o.props.is_object() {
                        o.props = Value::Object(Map::new());
                    }
                    if let Value::Object(m) = &mut o.props {
                        deep_merge(m, &p);
                    }
                }
                None
            }
            Edit::RemoveObject(id) => {
                self.project.objects.retain(|o| o.id != id);
                None
            }
            Edit::CreateGroup { name, ids } => {
                let mut n = self.project.groups.len() + 1;
                while self.project.groups.iter().any(|g| g.name == format!("Groupe {n}")) {
                    n += 1;
                }
                let ids = self.valid_ids(&ids);
                let name = name.map(|s| group_name(&s)).filter(|s| !s.is_empty()).unwrap_or_else(|| format!("Groupe {n}"));
                let g = Group { id: new_id("g"), name, ids, extra: Map::new() };
                let id = g.id.clone();
                self.project.groups.push(g);
                Some(id)
            }
            Edit::UpdateGroup { id, name, ids } => {
                let ids = ids.map(|i| self.valid_ids(&i));
                let g = self.project.groups.iter_mut().find(|g| g.id == id)?;
                if let Some(n) = name.map(|s| group_name(&s)).filter(|s| !s.is_empty()) {
                    g.name = n;
                }
                if let Some(ids) = ids {
                    g.ids = ids;
                }
                None
            }
            Edit::DeleteGroup(id) => {
                self.project.groups.retain(|g| g.id != id);
                None
            }
            Edit::Environment(changes) => {
                let mut env = match serde_json::to_value(&self.project.environment) {
                    Ok(Value::Object(m)) => m,
                    _ => Map::new(),
                };
                deep_merge(&mut env, &changes);
                if let Ok(e) = serde_json::from_value(Value::Object(env)) {
                    self.project.environment = e;
                }
                None
            }
            Edit::Batch(list) => {
                let mut first = None;
                for e in list {
                    let id = self.perform(e);
                    first = first.or(id);
                }
                first
            }
            Edit::Plot { plot, .. } => {
                self.project.extra.insert("plot".into(), plot);
                None
            }
            Edit::Rename(name) => {
                let n: String = name.trim().chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).take(80).collect();
                if !n.is_empty() {
                    self.project.name = n;
                }
                None
            }
        }
    }

    fn valid_ids(&self, ids: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for id in ids {
            if !out.contains(id) && self.fixture(id).is_some() {
                out.push(id.clone());
            }
        }
        out
    }

    fn update_fixture(&mut self, id: &str, mut changes: Changes) {
        let Some(f) = self.project.fixtures.iter_mut().find(|f| f.id == id) else { return };
        // un réglage de la lumière fait à la main rend la main à la console (le dernier geste l'emporte)
        let held = crate::engine::console::console_held(f);
        let look = changes.state.as_ref().is_some_and(|s| s.contains_key("manual") || s.contains_key("app")) || changes.settings.as_ref().is_some_and(|s| s.contains_key("standalone"));
        if held && look && !changes.state.as_ref().is_some_and(|s| s.contains_key("console")) {
            changes.state.get_or_insert_with(Map::new).insert("console".into(), Value::Null);
        }
        if let Some(p) = changes.position {
            f.position = p;
        }
        if let Some(r) = changes.rotation {
            f.rotation = r;
        }
        if let Some(n) = changes.name {
            f.name = n.chars().take(60).collect();
        }
        if let Some(mut s) = changes.settings {
            if let Some(n) = s.get("fixtureNumber") {
                let clean = clean_fixture_number(n);
                s.insert("fixtureNumber".into(), clean);
            }
            let mut all = match serde_json::to_value(&f.settings) {
                Ok(Value::Object(m)) => m,
                _ => Map::new(),
            };
            deep_merge(&mut all, &s);
            if let Ok(settings) = serde_json::from_value(Value::Object(all)) {
                f.settings = settings;
            }
        }
        if let Some(s) = changes.state {
            if !f.state.is_object() {
                f.state = Value::Object(Map::new());
            }
            if let Value::Object(m) = &mut f.state {
                deep_merge(m, &s);
            }
        }
    }
}
