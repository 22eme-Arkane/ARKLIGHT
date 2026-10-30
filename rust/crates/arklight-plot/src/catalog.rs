//! Ce que le plan lit dans la bibliothèque (portage des parties utiles de `shared/objects.js`,
//! `shared/grip.js`, `shared/props.js`, `shared/fixtures/supports.js` et `web/js/ui/patch.js`) :
//! types d'objets (grip ou décor), toiles, pieds, encombrement du décor, ligne de patch.

use crate::js::{max_of, num_truthy, text_of};
use arklight_core::accessories::{Cloth, Filter};
use arklight_core::library::{FixtureType, GripType, PropType, library};
use arklight_core::project::{Fixture, SceneObject};
use serde_json::{Map, Value};

/// Type d'un objet de la scène : grip (cadre, drapeau, pied…) ou décor.
#[derive(Debug, Clone, Copy)]
pub enum ObjectType {
    Grip(&'static GripType),
    Prop(&'static PropType),
}

/// `getObjectType` : grip d'abord, puis décor.
pub fn object_type(id: &str) -> Option<ObjectType> {
    let lib = library();
    lib.grip(id).map(ObjectType::Grip).or_else(|| lib.prop(id).map(ObjectType::Prop))
}

impl ObjectType {
    /// `isProp` : décor (personnage, véhicule, meuble…) plutôt que grip.
    pub fn is_prop(&self) -> bool {
        matches!(self, Self::Prop(p) if p.category.as_deref().is_some_and(|c| !c.is_empty()))
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::Grip(g) => &g.id,
            Self::Prop(p) => &p.id,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Grip(g) => &g.name,
            Self::Prop(p) => &p.name,
        }
    }

    pub fn short(&self) -> Option<&'static str> {
        match self {
            Self::Grip(g) => g.short.as_deref(),
            Self::Prop(p) => p.short.as_deref(),
        }
    }

    /// Famille du grip (« frame », « flag », « bounce », « box », « bag », « stand »).
    pub fn family(&self) -> Option<&'static str> {
        match self {
            Self::Grip(g) => g.family.as_deref(),
            Self::Prop(_) => None,
        }
    }

    /// Catégorie du décor (« people », « vehicles »…).
    pub fn cat(&self) -> Option<&'static str> {
        match self {
            Self::Prop(p) => p.category.as_deref(),
            Self::Grip(_) => None,
        }
    }

    /// Modèle du décor (« person », « tree »…).
    pub fn model(&self) -> Option<&'static str> {
        match self {
            Self::Prop(p) => p.model.as_deref(),
            Self::Grip(_) => None,
        }
    }

    fn extra(&self) -> &'static Map<String, Value> {
        match self {
            Self::Grip(g) => &g.extra,
            Self::Prop(p) => &p.extra,
        }
    }

    /// Toiles possibles (cadres, drapeaux).
    pub fn cloths(&self) -> Option<&'static Vec<Value>> {
        self.extra().get("cloths").and_then(Value::as_array)
    }
}

/// Largeur et hauteur d'un cadre, drapeau ou réflecteur (absentes : NaN, comme `undefined`).
pub fn grip_wh(g: &GripType) -> (f64, f64) {
    (g.w.unwrap_or(f64::NAN), g.h.unwrap_or(f64::NAN))
}

/// Dimensions d'un cube ou d'un sac (`size`).
pub fn grip_size(g: &GripType) -> [f64; 3] {
    let a = g.extra.get("size").and_then(Value::as_array);
    let v = |i: usize| a.and_then(|a| a.get(i)).and_then(Value::as_f64).unwrap_or(f64::NAN);
    [v(0), v(1), v(2)]
}

// ------------------------------------------------------------------ toiles et filtres
/// Toile d'un cadre (`CLOTHS[id]`).
pub fn cloth(id: &str) -> Option<&'static Cloth> {
    library().cloths.get(id)
}

/// Filtre du nuancier (`getFilter`).
pub fn filter(id: &str) -> Option<&'static Filter> {
    library().filter(id)
}

/// `clothName` : « Soie 1/4 (1/4 Silk) », « 216 — Full White Diffusion ».
pub fn cloth_name(id: &str) -> String {
    cloth(id).map(|c| c.name.clone()).or_else(|| filter(id).map(|f| f.name.clone())).unwrap_or_else(|| id.to_string())
}

/// `clothShort` : nom court pour le plan de feu (« Soie 1/4 », « 216 », « CTO »).
pub fn cloth_short(id: &str) -> String {
    if let Some(c) = cloth(id) {
        return c.short.clone().unwrap_or_else(|| c.name.split(' ').next().unwrap_or("").to_string());
    }
    match filter(id) {
        Some(f) => {
            let s = f.name.split(" — ").next().unwrap_or("");
            match s.strip_prefix("Rosco ") {
                Some(rest) => format!("R{rest}"),
                None => s.to_string(),
            }
        }
        None => id.to_string(),
    }
}

/// Pieds sous un cadre, un drapeau ou un réflecteur.
#[derive(Debug, Clone, PartialEq)]
pub struct Stands {
    pub id: String,
    pub count: u32,
    /// libellé propre au type (« C-stand », « Pied de 1000 + pince ») ; None si un autre pied est choisi
    pub label: Option<String>,
    pub chosen: bool,
}

/// `gripStands` : pieds choisis (`props.stand`), sinon ceux du type ; None pour les éléments posés et
/// les pieds seuls.
pub fn grip_stands(g: &GripType, props: &Value) -> Option<Stands> {
    let stands = g.extra.get("stands").filter(|v| crate::js::truthy(Some(v)))?;
    if g.family.as_deref() == Some("stand") {
        return None;
    }
    let own = stands.get("id").map(crate::js::string_of).unwrap_or_default();
    let wanted = props.get("stand").and_then(Value::as_str);
    let choices = g.extra.get("standChoices").and_then(Value::as_array);
    let id = match wanted {
        Some(w) if choices.is_some_and(|c| c.iter().any(|x| x.as_str() == Some(w))) => w.to_string(),
        _ => own.clone(),
    };
    let count = num_truthy(stands.get("count")).unwrap_or(1.0) as u32;
    let label = if id == own { stands.get("label").and_then(Value::as_str).map(str::to_string) } else { None };
    Some(Stands { chosen: id != own, id, count, label })
}

// ------------------------------------------------------------------ pieds des projecteurs
/// Fiche d'un pied (`SUPPORTS[id]`).
pub fn support_def(id: &str) -> Option<&'static Value> {
    library().supports.iter().find(|s| s.get("id").and_then(Value::as_str) == Some(id))
}

/// Pied retenu pour un projecteur (`resolveSupport`) : identifiant (« windup », « hanging »,
/// « none »…) et fiche s'il s'agit d'un pied.
#[derive(Debug, Clone, PartialEq)]
pub struct Support {
    pub id: String,
    pub def: Option<&'static Value>,
}

impl Support {
    pub fn name(&self) -> Option<&'static str> {
        self.def.and_then(|d| d.get("name")).and_then(Value::as_str)
    }

    pub fn short(&self) -> Option<&'static str> {
        self.def.and_then(|d| d.get("short")).and_then(Value::as_str)
    }
}

/// `resolveSupport` : le choix du projet, ou en automatique selon le poids, la fixation et la
/// hauteur `top` du bas du projecteur.
pub fn resolve_support(ty: &FixtureType, choice: Option<&str>, top: f64) -> Support {
    let w = num_truthy(ty.extra.get("weight")).unwrap_or(0.0);
    let mount = text_of(ty.extra.get("mount")).unwrap_or("");
    let heavy = mount.contains("28") || mount.to_lowercase().contains("junior");
    let id = match choice.filter(|c| !c.is_empty()).unwrap_or("auto") {
        "auto" => {
            if top < 0.6 || ty.category == "device" {
                "none"
            } else if top < 1.0 {
                "lowboy"
            } else if top > 4.5 || w > 80.0 {
                "long-john"
            } else if w > 60.0 {
                "super-windup"
            } else if w > 20.0 || top > 3.76 {
                "windup"
            } else if heavy || w > 9.0 {
                if top <= 2.0 && w <= 35.0 && !heavy { "stand-baby-5k" } else { "stand-junior" }
            } else {
                "stand-1000"
            }
        }
        other => other,
    };
    Support { id: id.to_string(), def: support_def(id) }
}

/// Pied d'un projecteur sans vue 3D : recalculé d'après la hauteur du bas de son corps
/// (`supportOf` de la liste du matériel).
pub fn support_of(f: &Fixture, ty: &FixtureType) -> Support {
    let half = crate::geometry::body_half_extents(ty);
    let choice = f.settings.extra.get("support").and_then(Value::as_str);
    resolve_support(ty, choice, f.position[1] - max_of(half))
}

// ------------------------------------------------------------------ décor
/// Réglages d'un objet (`o.props || {}`).
pub fn props_of(o: &SceneObject) -> &Value {
    static EMPTY: Value = Value::Null;
    if o.props.is_object() { &o.props } else { &EMPTY }
}

fn prop_num(props: &Value, key: &str) -> Option<f64> {
    props.get(key).filter(|v| !v.is_null()).and_then(Value::as_f64)
}

/// Dimensions du catalogue d'un objet de décor (`type.size`, avant réglages).
fn base_size(p: &PropType) -> [f64; 3] {
    match p.extra.get("baseSize").and_then(Value::as_array) {
        Some(a) => {
            let v = |i: usize| a.get(i).and_then(Value::as_f64).unwrap_or(f64::NAN);
            [v(0), v(1), v(2)]
        }
        None => p.size,
    }
}

/// `propSize` : encombrement [largeur, hauteur, profondeur] selon les réglages (pose, taille…).
pub fn prop_size(p: &PropType, props: &Value) -> [f64; 3] {
    if p.model.as_deref() == Some("person") {
        let h = num_truthy(props.get("height")).unwrap_or(175.0) / 100.0;
        let k = h / 1.75;
        return match props.get("pose").and_then(Value::as_str) {
            Some("walking") => [0.58 * k, h, 1.0 * k],
            Some("sitting") => [0.52 * k, 0.76 * h, 0.82 * k],
            Some("lying") => [0.72 * k, 0.4 * k, h],
            Some("pointing") => [0.6 * k, h, 0.85 * k],
            Some("handsup") => [0.92 * k, 1.16 * h, 0.38 * k],
            _ => [0.6 * k, h, 0.4 * k],
        };
    }
    let [w, h, d] = base_size(p);
    [prop_num(props, "width").unwrap_or(w), prop_num(props, "height").unwrap_or(h), d]
}

/// `cameraFov` : champ horizontal et vertical (radians) d'une caméra, rapport largeur / hauteur.
pub fn camera_fov(props: &Value) -> (f64, f64, f64) {
    let (sw, sh) = match props.get("sensor").and_then(Value::as_str) {
        Some("ff") => (36.0, 20.25),
        Some("m43") => (17.3, 9.73),
        _ => (24.89, 14.0),
    };
    // Number(props.focal) || 35
    let focal = match props.get("focal") {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse::<f64>().ok().or(if s.trim().is_empty() { Some(0.0) } else { None }),
        _ => None,
    };
    let f = focal.filter(|x| *x != 0.0 && !x.is_nan()).unwrap_or(35.0).max(4.0);
    (2.0 * (sw / (2.0 * f)).atan(), 2.0 * (sh / (2.0 * f)).atan(), sw / sh)
}

// ------------------------------------------------------------------ patch
/// Nombre de canaux du mode d'un projecteur (`modeFootprint`).
pub fn mode_footprint(f: &Fixture) -> usize {
    library().fixture(&f.type_id).and_then(|t| t.mode(f.settings.dmx_mode.as_deref())).map_or(0, |m| m.footprint())
}

fn pad3(n: i64) -> String {
    format!("{n:0>3}")
}

/// `patchLabel` : « U1 · 001–006 », « TX U1 » sans DMX.
pub fn patch_label(f: &Fixture) -> String {
    let fp = mode_footprint(f) as i64;
    let universe = f.settings.universe.unwrap_or(1);
    if fp == 0 {
        return format!("TX U{universe}");
    }
    let start = f.settings.dmx_address.unwrap_or(1) as i64;
    let end = start + fp - 1;
    format!("U{universe} · {}{}", pad3(start), if end > start { format!("–{}", pad3(end)) } else { String::new() })
}

/// `patchLine` : « Fix 12 · DMX U1 · 001–006 ».
pub fn patch_line(f: &Fixture, fixture: bool, dmx: bool) -> String {
    let mut parts = Vec::new();
    match f.settings.fixture_number {
        Some(n) if fixture && n != 0 => parts.push(format!("Fix {n}")),
        _ => {}
    }
    if dmx {
        parts.push(if mode_footprint(f) != 0 { format!("DMX {}", patch_label(f)) } else { patch_label(f) });
    }
    parts.join(" · ")
}
