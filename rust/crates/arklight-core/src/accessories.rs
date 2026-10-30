//! Accessoires de projecteur et filtres (portage de `shared/modifiers.js`, `shared/filters.js` et
//! d'`effectiveType` dans `shared/fixtures/index.js`) : boîtes à lumière (Chimera), dômes et
//! lanternes, louvers / grilles, porte-filtre (gélatine ou diffusion), zoom mécanique. Effet calculé :
//! angle d'ouverture, éclairement dans l'axe, couleur.

use crate::color::Rgb;
use crate::engine::{field, num, text};
use crate::library::{FixtureType, Library, library};
use crate::project::Fixture;
use serde::Deserialize;
use serde_json::Value;

/// Un filtre du nuancier (diffusion, correction, neutre, couleur).
#[derive(Debug, Clone, Deserialize)]
pub struct Filter {
    pub id: String,
    pub name: String,
    pub group: String,
    /// couleur « #rrggbb »
    pub color: String,
    /// transmission (0..1)
    pub t: f64,
    /// part diffusée (0 net, 1 très diffusant)
    #[serde(default)]
    pub scatter: f64,
    /// correction de couleur (mireds ; CTB négatif, CTO positif)
    #[serde(default)]
    pub mired: Option<f64>,
}

/// Boîte à lumière, dôme ou lanterne.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Modifier {
    pub id: String,
    pub kind: String,
    /// famille (« Dômes (monture Bowens) »…), pour les listes de choix
    #[serde(default)]
    pub group: String,
    pub name: String,
    pub short: String,
    /// diamètre (dômes, lanternes)
    #[serde(default)]
    pub d: f64,
    #[serde(default)]
    pub w: f64,
    #[serde(default)]
    pub h: f64,
    #[serde(default)]
    pub depth: f64,
    pub angle: f64,
    /// part de lumière restituée avec la toile d'origine
    pub t: f64,
    /// à la taille du panneau
    #[serde(default)]
    pub fit: bool,
    #[serde(default)]
    pub panel_only: bool,
}

/// Louver / grille.
#[derive(Debug, Clone, Deserialize)]
pub struct Grid {
    pub id: String,
    pub name: String,
    pub short: String,
    pub angle: f64,
    pub t: f64,
    pub depth: f64,
}

/// Toile des cadres et drapeaux.
#[derive(Debug, Clone, Deserialize)]
pub struct Cloth {
    pub name: String,
    /// nom court (plan de feu)
    #[serde(default)]
    pub short: Option<String>,
    /// famille : silk, grid, frost, bounce, cut (rien : le cadre vide)
    #[serde(default)]
    pub group: Option<String>,
    /// 0xRRGGBB
    pub color: u32,
    pub opacity: f64,
    /// perte de la lumière qui la traverse, en diaphragmes (indicative)
    #[serde(default)]
    pub stops: Option<f64>,
    /// part qui passe en ligne droite
    #[serde(default)]
    pub t: f64,
    /// part réémise en lumière douce de l'autre côté
    #[serde(default)]
    pub diffuse: f64,
    /// part renvoyée côté source
    #[serde(default)]
    pub reflect: f64,
    /// opaque : ombre nette
    #[serde(default)]
    pub solid: bool,
    /// couleur prise par la lumière (lamé or, muslin écru)
    #[serde(default)]
    pub tint: Option<Rgb>,
}

/// Toile avant d'une boîte à lumière ou d'un dôme : d'origine.
pub const FRONT_DEFAULT_T: f64 = 0.55;

impl Library {
    pub fn filter(&self, id: &str) -> Option<&Filter> {
        self.filters.iter().find(|f| f.id == id)
    }

    pub fn grid_def(&self, id: &str) -> Option<&Grid> {
        self.grids.iter().find(|g| g.id == id)
    }
}

/// Couleur [r, g, b] (0–1) normalisée d'un filtre : la teinte, sans la perte de lumière (portée par t).
pub fn filter_tint(f: &Filter) -> Rgb {
    if f.group == "diffusion" || f.group == "nd" {
        return [1.0; 3];
    }
    let n = u32::from_str_radix(f.color.trim_start_matches('#'), 16).unwrap_or(0xffffff);
    let rgb = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(|v| (v as f64 / 255.0).powf(2.2)); // sRGB → linéaire
    let m = rgb[0].max(rgb[1]).max(rgb[2]);
    let m = if m == 0.0 { 1.0 } else { m };
    rgb.map(|v| v / m)
}

/// Effet d'un filtre sur la lumière qui le traverse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Optics {
    /// part qui passe en ligne droite (0 : coupé net)
    pub t: f64,
    /// part renvoyée côté source (réflecteurs)
    pub reflect: f64,
    /// part réémise en lumière douce de l'autre côté (soie, grid cloth)
    pub diffuse: f64,
    /// surface opaque : ombre nette
    pub solid: bool,
    pub tint: Option<Rgb>,
}

pub fn filter_optics(f: &Filter) -> Optics {
    Optics { t: f.t * (1.0 - f.scatter), reflect: 0.0, diffuse: f.t * f.scatter, solid: false, tint: Some(filter_tint(f)) }
}

/// Accessoires possibles pour un type de projecteur.
pub struct Allowed<'a> {
    pub modifiers: Vec<&'a Modifier>,
    pub grids: bool,
    pub gel: bool,
}

pub fn accessories_for(ty: &FixtureType) -> Allowed<'static> {
    let lib = library();
    let kind = ty.geometry.kind.as_str();
    let none = Allowed { modifiers: Vec::new(), grids: false, gel: false };
    if ty.pixels == 0 || matches!(kind, "tube" | "bulb" | "box-device") || matches!(ty.category.as_str(), "device" | "bulb" | "tube") {
        return none;
    }
    if kind == "bar" || ty.category == "bar" {
        return Allowed { modifiers: Vec::new(), grids: false, gel: true };
    }
    let panel = matches!(kind, "panel" | "brick") || ty.category == "panel";
    let mount = ty.extra.get("mount").and_then(Value::as_str).unwrap_or("");
    let bowens = mount.to_lowercase().contains("bowens") || (ty.category == "spot" && !ty.is_traditional());
    let modifiers = lib
        .modifiers
        .iter()
        .filter(|m| {
            if panel {
                m.kind == "softbox" && (m.panel_only || m.id == "chimera-s" || m.id == "chimera-m")
            } else if bowens {
                !m.panel_only
            } else {
                m.kind == "softbox" && !m.panel_only // fresnels, PAR, open face (couronne)
            }
        })
        .collect();
    Allowed { modifiers, grids: true, gel: true }
}

fn tan_half(deg: f64) -> f64 {
    (deg.min(170.0) * std::f64::consts::PI / 360.0).tan()
}

/// Toile avant d'une boîte à lumière ou d'un dôme.
#[derive(Debug, Clone, Copy)]
pub enum Front<'a> {
    /// sans toile
    None,
    Filter(&'a Filter),
}

/// Effet des accessoires réglés sur un projecteur.
#[derive(Debug, Clone, Copy)]
pub struct Effect<'a> {
    /// angle d'ouverture (degrés)
    pub angle: f64,
    /// facteur d'éclairement dans l'axe
    pub lux: f64,
    /// part de lumière restituée (sans l'effet de l'ouverture du faisceau)
    pub transmission: f64,
    pub tint: Option<Rgb>,
    pub modifier: Option<&'a Modifier>,
    pub grid: Option<&'a Grid>,
    pub gel: Option<&'a Filter>,
    pub front: Option<Front<'a>>,
}

impl Effect<'_> {
    pub fn any(&self) -> bool {
        self.modifier.is_some() || self.grid.is_some() || self.gel.is_some()
    }
}

/// Effet des accessoires (réglages `modifier`, `front`, `grid`, `gel`) ; `base_angle` : angle du
/// faisceau sans accessoire.
pub fn accessory_effect(ty: &FixtureType, settings: Option<&Value>, base_angle: f64) -> Effect<'static> {
    let lib = library();
    let allowed = accessories_for(ty);
    let s = |k: &str| text(field(settings, k));
    let modifier = s("modifier").and_then(|id| allowed.modifiers.iter().find(|m| m.id == id).copied());
    let grid = if allowed.grids { s("grid").and_then(|id| lib.grid_def(id)) } else { None };
    let gel = if allowed.gel { s("gel").and_then(|id| lib.filter(id)) } else { None };
    let front = match (modifier, s("front")) {
        (Some(m), Some(f)) if m.kind != "lantern" && !f.is_empty() => {
            if f == "none" {
                Some(Front::None)
            } else {
                lib.filter(f).map(Front::Filter)
            }
        }
        _ => None,
    };
    let mut angle = base_angle;
    let mut lux = 1.0;
    let mut transmission = 1.0;
    if let Some(m) = modifier {
        angle = m.angle;
        transmission *= m.t;
        // lanterne : lumière dans toutes les directions (environ 4 diaph de moins dans l'axe)
        lux *= m.t * if m.kind == "lantern" { 0.06 } else { (tan_half(base_angle) / tan_half(m.angle)).powi(2).min(1.0) };
        let k = match front {
            Some(Front::None) => 1.0 / FRONT_DEFAULT_T,
            Some(Front::Filter(f)) => f.t / FRONT_DEFAULT_T,
            None => 1.0,
        };
        lux *= k;
        transmission *= k;
    }
    if let Some(g) = gel {
        lux *= g.t;
        transmission *= g.t;
        if g.scatter > 0.0 && modifier.is_none() {
            // diffusion devant un projecteur nu : le faisceau s'ouvre
            let wider = (angle + g.scatter * 60.0).max(angle).min(120.0);
            lux *= (tan_half(angle) / tan_half(wider)).powi(2).min(1.0);
            angle = wider;
        }
    }
    match grid {
        Some(g) if g.angle < angle => {
            angle = g.angle;
            lux *= g.t;
            transmission *= g.t;
        }
        Some(g) => {
            lux *= (g.t + 0.15).min(1.0);
            transmission *= (g.t + 0.15).min(1.0);
        }
        None => {}
    }
    Effect { angle, lux, transmission, tint: gel.map(filter_tint), modifier, grid, gel, front }
}

/// Type « effectif » d'un projecteur selon son zoom et ses accessoires.
#[derive(Debug, Clone, Copy)]
pub struct Effective {
    /// angle du faisceau (degrés) ; None : celui de la forme
    pub beam_angle: Option<f64>,
    pub lux_at1m: Option<f64>,
    pub lumens: Option<f64>,
    pub accessories: Option<Effect<'static>>,
}

/// `Math.round` de JavaScript.
fn round(x: f64) -> f64 {
    crate::engine::js_round(x)
}

/// Zoom mécanique (SolaBulb, Pluto, Leo…) : éclairement interpolé entre les mesures du fabricant.
fn zoomed(ty: &FixtureType, f: &Fixture) -> Effective {
    let mut e = Effective { beam_angle: ty.beam_angle, lux_at1m: ty.output.as_ref().and_then(|o| o.lux_at1m), lumens: ty.output.as_ref().and_then(|o| o.lumens), accessories: None };
    let Some(zoom) = ty.extra.get("zoom") else { return e };
    let (Some(min), Some(max)) = (num(zoom.get("min")), num(zoom.get("max"))) else { return e };
    let pts: Vec<(f64, f64)> = zoom.get("luxAt1m").and_then(Value::as_array).map(|a| a.iter().filter_map(|p| Some((p.get(0)?.as_f64()?, p.get(1)?.as_f64()?))).collect()).unwrap_or_default();
    if pts.is_empty() {
        return e;
    }
    let wanted = f.settings.extra.get("zoom").filter(|v| !v.is_null()).map_or(ty.beam_angle.unwrap_or(f64::NAN), |v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(f64::NAN));
    let z = wanted.max(min).min(max);
    let z = if wanted.is_nan() { f64::NAN } else { z };
    let mut lux = pts[0].1;
    for w in pts.windows(2) {
        let ((a, la), (b, lb)) = (w[0], w[1]);
        if z >= a && z <= b {
            // interpolation logarithmique (l'éclairement varie fortement avec l'angle)
            let k = (z - a) / (b - a);
            lux = (la.ln() + k * (lb.ln() - la.ln())).exp();
        }
    }
    if z >= pts[pts.len() - 1].0 {
        lux = pts[pts.len() - 1].1;
    }
    e.beam_angle = Some(z);
    e.lux_at1m = Some(round(lux));
    e
}

/// Type effectif (`effectiveType`) : zoom, puis accessoires (angle, éclairement, lumens).
pub fn effective(ty: &FixtureType, f: &Fixture) -> Effective {
    let mut t = zoomed(ty, f);
    let s = |k: &str| f.settings.extra.get(k).is_some_and(|v| crate::engine::truthy(Some(v)));
    if !s("modifier") && !s("grid") && !s("gel") {
        return t;
    }
    let settings = Value::Object(f.settings.extra.clone());
    // angle sans accessoire (celui du zoom) ; inconnu : 40°
    let e = accessory_effect(ty, Some(&settings), t.beam_angle.unwrap_or(40.0));
    if !e.any() {
        return t;
    }
    if let Some(l) = t.lux_at1m.filter(|l| *l != 0.0) {
        t.lux_at1m = Some(round(l * e.lux).max(1.0));
    }
    if let Some(l) = t.lumens.filter(|l| *l != 0.0) {
        t.lumens = Some(round(l * e.transmission));
    }
    t.beam_angle = Some(e.angle);
    t.accessories = Some(e);
    t
}
