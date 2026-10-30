//! Projets ARKLIGHT (format 2) : les mêmes fichiers `.json` que la version JavaScript.
//!
//! Le plan ouvert est à la racine du fichier (projecteurs, décor, groupes, ambiance, plan 2D) ;
//! les autres plans sont dans `sequences`. Tout champ que cette version ne connaît pas encore est
//! gardé tel quel (`extra`) et réécrit à l'enregistrement : un projet passe d'une version à
//! l'autre sans rien perdre.

use crate::library::{FixtureType, library};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::path::Path;

pub type Vec3 = [f64; 3];

/// Le projet, tel qu'il est enregistré (plan ouvert à la racine).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub environment: Environment,
    #[serde(default)]
    pub fixtures: Vec<Fixture>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub objects: Vec<SceneObject>,
    /// plan 2D, séquences et plans, vignette, `app`, `format`, `savedAt`…
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Ambiance de la vue (sol, cyclo, brume, exposition…) ; valeurs par défaut d'un projet neuf de
/// la version JavaScript (`emptyScene`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    #[serde(default = "yes")]
    pub floor: bool,
    #[serde(default = "yes")]
    pub cyc: bool,
    /// brume (0..1) : faisceaux visibles
    #[serde(default = "default_haze")]
    pub haze: f64,
    #[serde(default = "default_ambient")]
    pub ambient: f64,
    /// exposition de la caméra (multiplicateur ; ±IL dans l'interface)
    #[serde(default = "default_exposure")]
    pub exposure: f64,
    /// pieds affichés, étiquettes…
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

fn yes() -> bool {
    true
}
fn default_haze() -> f64 {
    0.25
}
fn default_ambient() -> f64 {
    0.04
}
fn default_exposure() -> f64 {
    1.0
}

impl Default for Environment {
    fn default() -> Self {
        let mut extra = Map::new();
        for (k, v) in [("stands", true), ("labels", false), ("labelPatch", false)] {
            extra.insert(k.into(), Value::Bool(v));
        }
        Self { floor: true, cyc: true, haze: default_haze(), ambient: default_ambient(), exposure: default_exposure(), extra }
    }
}

impl Environment {
    /// Pieds des projecteurs affichés (réglage « stands »).
    pub fn stands(&self) -> bool {
        self.extra.get("stands") != Some(&Value::Bool(false))
    }
}

/// Un projecteur placé dans la scène.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    pub id: String,
    /// identifiant du modèle dans la bibliothèque (« astera.titan », « arri.m18 »…)
    #[serde(rename = "type")]
    pub type_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub position: Vec3,
    /// angles d'Euler (radians, ordre X puis Y puis Z) ; le faisceau sort selon +Z
    #[serde(default)]
    pub rotation: Vec3,
    #[serde(default)]
    pub settings: Settings,
    /// état simulé (allumage, réglage manuel, console…) : lu par `look`
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub state: Value,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Patch et réglages d'un projecteur.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub universe: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmx_address: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmx_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixture_number: Option<u32>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Un groupe (sélection nommée de projecteurs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Décor, personnage, véhicule, grip ou pied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneObject {
    pub id: String,
    #[serde(rename = "type")]
    pub type_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub position: Vec3,
    #[serde(default)]
    pub rotation: Vec3,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub props: Value,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug)]
pub enum ProjectError {
    Io(std::io::Error),
    Json(serde_json::Error),
    /// le fichier n'est pas un projet ARKLIGHT
    NotAProject,
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Json(e) => write!(f, "JSON invalide : {e}"),
            Self::NotAProject => write!(f, "ce fichier n'est pas un projet ARKLIGHT"),
        }
    }
}

impl std::error::Error for ProjectError {}

impl Project {
    /// Lit un projet depuis son texte JSON.
    pub fn from_json(text: &str) -> Result<Self, ProjectError> {
        let v: Value = serde_json::from_str(text).map_err(ProjectError::Json)?;
        // même règle que la version JavaScript (server/projects.js, isProjectData)
        let ok = v.get("fixtures").is_some_and(Value::is_array) && (v.get("app").and_then(Value::as_str) == Some("ARKLIGHT") || v.get("environment").is_some_and(Value::is_object));
        if !ok {
            return Err(ProjectError::NotAProject);
        }
        serde_json::from_value(v).map_err(ProjectError::Json)
    }

    pub fn open(path: &Path) -> Result<Self, ProjectError> {
        let text = std::fs::read_to_string(path).map_err(ProjectError::Io)?;
        let mut p = Self::from_json(&text)?;
        p.sanitize();
        if p.name.is_empty() {
            p.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        }
        Ok(p)
    }

    /// Texte JSON du fichier (format 2), champs inconnus compris.
    pub fn to_json(&self) -> Result<String, ProjectError> {
        let mut v = serde_json::to_value(self).map_err(ProjectError::Json)?;
        if let Value::Object(m) = &mut v {
            m.insert("app".into(), "ARKLIGHT".into());
            m.insert("format".into(), 2.into());
        }
        serde_json::to_string_pretty(&v).map_err(ProjectError::Json)
    }

    /// Enregistre (d'abord à côté, puis remplacement : jamais de fichier à moitié écrit).
    pub fn save(&self, path: &Path) -> Result<(), ProjectError> {
        let json = self.to_json()?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json.as_bytes()).map_err(ProjectError::Io)?;
        if std::fs::rename(&tmp, path).is_err() {
            std::fs::write(path, json.as_bytes()).map_err(ProjectError::Io)?;
            let _ = std::fs::remove_file(&tmp);
        }
        Ok(())
    }

    /// Projet vide : le plateau et un comédien au centre, comme un nouveau projet JavaScript.
    pub fn empty(name: &str) -> Self {
        Self {
            name: name.to_string(),
            objects: vec![SceneObject {
                id: "actor".into(),
                type_id: "person-man".into(),
                name: "Comédien".into(),
                position: [0.0, 0.0, 0.0],
                rotation: [0.0, 0.0, 0.0],
                props: Value::Object(Map::new()),
                extra: Map::new(),
            }],
            ..Self::default()
        }
    }
}

impl Project {
    /// Ajoute les réglages et l'état par défaut qui manquent aux projecteurs (projets d'une version
    /// antérieure), comme la version JavaScript au chargement (`SceneStore._sanitize`).
    pub fn sanitize(&mut self) {
        for (k, v) in Environment::default().extra {
            self.environment.extra.entry(k).or_insert(v);
        }
        let lib = library();
        for f in &mut self.fixtures {
            if let Some(ty) = lib.fixture(&f.type_id) {
                f.apply_defaults(ty);
            }
        }
        for o in &mut self.objects {
            o.apply_defaults();
        }
    }
}

impl SceneObject {
    /// Réglages d'un objet neuf (décor : ceux du catalogue ; grip : toile et hauteur par défaut),
    /// complétés sans toucher à ceux du projet (`defaultObjectProps`).
    pub fn default_props(type_id: &str) -> Map<String, Value> {
        let lib = library();
        if let Some(t) = lib.prop(type_id) {
            return t.extra.get("defaults").and_then(Value::as_object).cloned().unwrap_or_default();
        }
        let mut m = Map::new();
        if let Some(g) = lib.grip(type_id) {
            if g.extra.get("cloths").is_some()
                && let Some(c) = g.extra.get("defaultCloth")
            {
                m.insert("cloth".into(), c.clone());
            }
            if g.family.as_deref() == Some("stand")
                && let Some(h) = g.extra.get("height")
            {
                m.insert("height".into(), h.clone());
            }
        }
        m
    }

    pub fn apply_defaults(&mut self) {
        let mut props = Self::default_props(&self.type_id);
        if let Value::Object(own) = &self.props {
            props.extend(own.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        self.props = Value::Object(props);
    }
}

/// `deepMerge` : les objets sont fusionnés champ par champ, le reste remplacé.
fn deep_merge(target: &mut Map<String, Value>, patch: &Map<String, Value>) {
    for (k, v) in patch {
        match (target.get_mut(k), v) {
            (Some(Value::Object(t)), Value::Object(p)) => deep_merge(t, p),
            _ => {
                target.insert(k.clone(), v.clone());
            }
        }
    }
}

impl Fixture {
    /// Réglages d'un projecteur neuf (ce que modifient ses menus, l'appli ou le patch).
    pub fn default_settings(ty: &FixtureType) -> Map<String, Value> {
        let first_mode = ty.dmx.modes.first().map_or(Value::Null, |m| Value::String(m.id.clone()));
        let mut s = json!({ "universe": 1, "dmxAddress": 1, "dmxMode": first_mode, "whitePoint": 5600, "dimmingCurve": "linear", "support": "auto" });
        let map = s.as_object_mut().expect("objet");
        map.extend(ty.defaults.settings.iter().map(|(k, v)| (k.clone(), v.clone())));
        std::mem::take(map)
    }

    /// État « physique » simulé d'un projecteur neuf (alimentation, batterie, commande manuelle).
    pub fn default_state(ty: &FixtureType) -> Map<String, Value> {
        let mut s = json!({
            "power": true,
            "battery": 100,
            "charging": false,
            "manual": { "mode": "cct", "intensity": 0.6, "cct": 5600, "tint": 0, "hue": 0, "sat": 1 },
        });
        let map = s.as_object_mut().expect("objet");
        map.extend(ty.defaults.state.iter().map(|(k, v)| (k.clone(), v.clone())));
        std::mem::take(map)
    }

    /// Complète réglages et état avec ceux d'un projecteur neuf ; mode DMX disparu (bibliothèque
    /// mise à jour) : retour au mode par défaut du modèle.
    pub fn apply_defaults(&mut self, ty: &FixtureType) {
        let defaults = Self::default_settings(ty);
        let mut settings = match serde_json::to_value(&self.settings) {
            Ok(Value::Object(m)) => m,
            _ => Map::new(),
        };
        for (k, v) in &defaults {
            settings.entry(k.clone()).or_insert_with(|| v.clone());
        }
        let mode = settings.get("dmxMode").and_then(Value::as_str);
        if !ty.dmx.modes.is_empty() && !ty.dmx.modes.iter().any(|m| Some(m.id.as_str()) == mode) {
            settings.insert("dmxMode".into(), defaults.get("dmxMode").cloned().unwrap_or(Value::Null));
        }
        if let Ok(s) = serde_json::from_value(Value::Object(settings)) {
            self.settings = s;
        }
        let mut state = Self::default_state(ty);
        if let Value::Object(patch) = &self.state {
            deep_merge(&mut state, patch);
        }
        self.state = Value::Object(state);
    }
}
