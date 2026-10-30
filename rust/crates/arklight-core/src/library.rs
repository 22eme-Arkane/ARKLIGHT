//! Bibliothèque : projecteurs (Astera, autres marques, tungstène et HMI), décor, grip, pieds.
//! Les données viennent de la version JavaScript (`shared/`), exportées par
//! `tools/rust/export-library.mjs` dans `data/library.json` et intégrées au programme.

use crate::accessories::{Cloth, Filter, Grid, Modifier};
use crate::engine::dmx::CompiledMode;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::sync::OnceLock;

const LIBRARY_JSON: &str = include_str!("../data/library.json");

#[derive(Debug, Deserialize)]
pub struct Library {
    pub fixtures: Vec<FixtureType>,
    pub props: Vec<PropType>,
    pub grip: Vec<GripType>,
    pub supports: Vec<Value>,
    /// couleurs index Astera : [valeur DMX ou plage « 0..1 », nom du gel LEE]
    #[serde(default, rename = "indexColors")]
    pub index_colors: Vec<(Value, String)>,
    /// couleurs du menu STATIC COLOR des Astera (« AsteraRGB » 0..255)
    #[serde(default, rename = "staticColors")]
    pub static_colors: Vec<(String, [f64; 3])>,
    /// toiles des cadres et drapeaux
    #[serde(default)]
    pub cloths: std::collections::BTreeMap<String, Cloth>,
    /// nuancier (diffusions, corrections, neutres, couleurs)
    #[serde(default)]
    pub filters: Vec<Filter>,
    /// boîtes à lumière, dômes, lanternes
    #[serde(default)]
    pub modifiers: Vec<Modifier>,
    /// louvers / grilles
    #[serde(default)]
    pub grids: Vec<Grid>,
}

/// Un modèle de projecteur.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureType {
    pub id: String,
    #[serde(default)]
    pub brand: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub category: String,
    /// forme et dimensions (tube, panneau, fresnel…), en mètres
    #[serde(default)]
    pub geometry: Geometry,
    /// angle du faisceau (degrés) ; absent : selon la forme
    #[serde(default)]
    pub beam_angle: Option<f64>,
    #[serde(default)]
    pub output: Option<Output>,
    #[serde(default)]
    pub dmx: Dmx,
    /// nombre de pixels (zones de couleur) ; 0 : boîtier sans lumière (AsteraBox…)
    #[serde(default)]
    pub pixels: usize,
    /// « tungsten », « hmi » ; absent : LED
    #[serde(default)]
    pub technology: Option<String>,
    /// plateforme du firmware (« astera-v5-oled »…) ou « generic »
    #[serde(default)]
    pub platform: Option<String>,
    /// « mains » : alimenté sur secteur (pas de bridage par l'autonomie)
    #[serde(default)]
    pub power: Option<String>,
    /// lampe des projecteurs tungstène et HMI
    #[serde(default)]
    pub lamp: Option<Lamp>,
    /// autonomie en plein feu (heures)
    #[serde(default)]
    pub max_brightness_runtime: Option<f64>,
    /// gain du réglage OUTPUT GAIN sur OFF (QuikSpot, QuikPunch…)
    #[serde(default)]
    pub output_gain: Option<f64>,
    /// réglages et état d'un projecteur neuf propres au modèle
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Lamp {
    #[serde(default)]
    pub cct: Option<f64>,
    #[serde(default)]
    pub watts: Option<f64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Defaults {
    #[serde(default)]
    pub settings: Map<String, Value>,
    #[serde(default)]
    pub state: Map<String, Value>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Geometry {
    #[serde(default)]
    pub kind: String,
    #[serde(flatten)]
    pub dims: Map<String, Value>,
}

impl Geometry {
    fn num(&self, key: &str) -> Option<f64> {
        self.dims.get(key).and_then(Value::as_f64)
    }

    /// Encombrement (x, y, z en mètres) d'une boîte simple, comme les modèles de la version
    /// three.js : faisceau selon +Z, longueur des tubes et des barres selon X.
    pub fn size(&self) -> [f64; 3] {
        let d = |k: &str, v: f64| self.num(k).unwrap_or(v);
        match self.kind.as_str() {
            "tube" => {
                let dia = d("diameter", 0.04);
                [d("length", 1.0), dia, dia]
            }
            "bar" => [d("length", 1.0), d("height", 0.05), d("depth", 0.05)],
            "par" => {
                let dia = d("diameter", 0.2);
                [dia, dia, d("depth", 0.2)]
            }
            "bulb" => {
                let dia = d("diameter", 0.1);
                [dia, dia, d("length", dia)]
            }
            _ => [d("width", 0.3), d("height", 0.3), d("depth", 0.2)],
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Output {
    #[serde(default)]
    pub lux_at1m: Option<f64>,
    #[serde(default)]
    pub lumens: Option<f64>,
    /// sortie estimée (pas de fiche photométrique)
    #[serde(default)]
    pub estimated: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct Dmx {
    #[serde(default)]
    pub modes: Vec<DmxMode>,
}

#[derive(Debug, Deserialize)]
pub struct DmxMode {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// pixels pilotés : un nombre, « effet » (modes d'effet Astera) ou absent (tous)
    #[serde(default)]
    pub pixels: Option<Value>,
    /// canaux dans l'ordre (voir `engine::dmx`)
    #[serde(default)]
    pub channels: Vec<Value>,
    /// numéro, libellé, stroboscope, ventilateur…
    #[serde(flatten)]
    pub extra: Map<String, Value>,
    #[serde(skip)]
    compiled: OnceLock<CompiledMode>,
}

impl DmxMode {
    /// Canaux à plat (calculés une fois).
    pub fn compiled(&self) -> &CompiledMode {
        self.compiled.get_or_init(|| CompiledMode::compile(&self.channels))
    }

    /// Nombre de canaux occupés.
    pub fn footprint(&self) -> usize {
        self.compiled().footprint
    }

    /// Nombre de pixels pilotés sur un projecteur à `pixel_count` pixels.
    pub fn pixels_on(&self, pixel_count: usize) -> usize {
        match self.pixels.as_ref().and_then(Value::as_f64) {
            Some(n) => (n.min(pixel_count as f64).max(1.0)) as usize,
            None => pixel_count,
        }
    }

    /// Le mode fixe-t-il son nombre de pixels (nombre) ?
    pub fn has_pixel_count(&self) -> bool {
        self.pixels.as_ref().is_some_and(Value::is_number)
    }
}

/// Décor, personnage, véhicule (boîte d'encombrement en attendant les modèles 3D).
#[derive(Debug, Deserialize)]
pub struct PropType {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub short: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    pub size: [f64; 3],
    /// espèce, genre de véhicule, réglages…
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Cadre, drapeau, réflecteur…
#[derive(Debug, Deserialize)]
pub struct GripType {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub short: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub w: Option<f64>,
    #[serde(default)]
    pub h: Option<f64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Library {
    pub fn fixture(&self, id: &str) -> Option<&FixtureType> {
        self.fixtures.iter().find(|t| t.id == id)
    }
    pub fn prop(&self, id: &str) -> Option<&PropType> {
        self.props.iter().find(|t| t.id == id)
    }
    pub fn grip(&self, id: &str) -> Option<&GripType> {
        self.grip.iter().find(|t| t.id == id)
    }
}

impl FixtureType {
    /// Angle du faisceau en degrés (valeur de la fiche, sinon selon la forme).
    pub fn beam_degrees(&self) -> f64 {
        self.beam_angle.unwrap_or(match self.geometry.kind.as_str() {
            "tube" | "bulb" => 120.0,
            _ => 60.0,
        })
    }

    /// Éclairement à 1 m (lux) en plein feu, pour l'intensité des lumières de la vue.
    pub fn lux_at_1m(&self) -> f64 {
        self.output.as_ref().and_then(|o| o.lux_at1m).unwrap_or(2000.0)
    }

    /// Mode DMX `id`, sinon le premier du modèle (comme `getMode`).
    pub fn mode(&self, id: Option<&str>) -> Option<&DmxMode> {
        let modes = &self.dmx.modes;
        id.and_then(|id| modes.iter().find(|m| m.id == id)).or(modes.first())
    }

    /// Projecteur Astera à firmware V5 (menus, mode autonome, entrées CRMX / filaire / appli).
    pub fn is_astera_v5(&self) -> bool {
        self.platform.as_deref().is_some_and(|p| p.starts_with("astera-v5"))
    }

    /// Tungstène ou HMI (pas une LED).
    pub fn is_traditional(&self) -> bool {
        matches!(self.technology.as_deref(), Some("tungsten" | "hmi"))
    }

    pub fn is_hmi(&self) -> bool {
        self.technology.as_deref() == Some("hmi")
    }

    /// Poids (kg) ; inconnu : 0.
    pub fn weight(&self) -> f64 {
        self.extra.get("weight").and_then(Value::as_f64).unwrap_or(0.0)
    }

    /// Dimension de la forme (`geometry.length`, `diameter`…).
    pub fn dim(&self, key: &str) -> Option<f64> {
        self.geometry.dims.get(key).and_then(Value::as_f64)
    }
}

impl Library {
    /// Pied d'un projecteur dont le bas est à `top` mètres : le choix du projet (`support`), ou en
    /// automatique selon son poids, sa fixation et sa hauteur (comme `resolveSupport`). None : posé
    /// au sol ou tenu à la main.
    pub fn resolve_support(&self, ty: &FixtureType, choice: Option<&str>, top: f64) -> Option<&Value> {
        let w = ty.weight();
        let mount = ty.extra.get("mount").and_then(Value::as_str).unwrap_or("");
        let heavy_mount = mount.contains("28") || mount.to_lowercase().contains("junior");
        let id = match choice.filter(|c| !c.is_empty()).unwrap_or("auto") {
            "auto" => {
                if top < 0.6 || ty.category == "device" {
                    return None;
                } else if top < 1.0 {
                    "lowboy"
                } else if top > 4.5 || w > 80.0 {
                    "long-john"
                } else if w > 60.0 {
                    "super-windup"
                } else if w > 20.0 || top > 3.76 {
                    "windup"
                } else if heavy_mount || w > 9.0 {
                    if top <= 2.0 && w <= 35.0 && !heavy_mount { "stand-baby-5k" } else { "stand-junior" }
                } else {
                    "stand-1000"
                }
            }
            other => other,
        };
        self.supports.iter().find(|s| s.get("id").and_then(Value::as_str) == Some(id))
    }
}

/// La bibliothèque, lue une fois.
pub fn library() -> &'static Library {
    static LIB: OnceLock<Library> = OnceLock::new();
    LIB.get_or_init(|| serde_json::from_str(LIBRARY_JSON).expect("data/library.json illisible"))
}
