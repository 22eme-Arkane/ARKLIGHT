//! Dessin du plan de feu en SVG (portage de `web/js/plot/render.js`) : à l'écran (thème sombre) ou
//! pour l'export et l'impression (fond blanc), ou sur un fond de couleur choisi (les traits
//! s'adaptent pour rester lisibles). Les étiquettes (noms, numéros de fixture, adresses DMX, pieds)
//! sont placées autour des éléments sans les recouvrir, avec un trait de rappel quand elles s'en
//! éloignent.

use crate::catalog::{ObjectType, Support, camera_fov, cloth, cloth_short, filter, grip_stands, grip_wh, object_type, patch_line, props_of, support_def, support_of};
use crate::geometry::{P2, fixture_plan, frame_ears, object_plan, point_in_polygon};
use crate::js::{esc, f1, hypot, len16, max_of, min2, min_of, num, num_truthy, round, string_of, text_of, to_fixed, truthy};
use crate::symbols::prop_symbol;
use arklight_core::accessories::{Front, accessory_effect};
use arklight_core::color::linear_to_srgb;
use arklight_core::library::{FixtureType, library};
use arklight_core::project::{Fixture, Project, SceneObject};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::f64::consts::PI;

/// Traduction des textes (langue de l'interface) ; sans traduction : le français d'origine.
pub type Tr<'a> = &'a dyn Fn(&str) -> String;

/// Texte traduit (`tr(s)`).
pub(crate) fn tr(t: Option<Tr>, s: &str) -> String {
    t.map_or_else(|| s.to_string(), |f| f(s))
}

// ------------------------------------------------------------------ thèmes
/// Couleurs du dessin.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub bg: String,
    pub minor: String,
    pub major: String,
    pub text: String,
    pub muted: String,
    pub body: String,
    pub body_fill: String,
    pub sel: String,
    pub cyc: String,
    pub prop: String,
    pub addr: String,
    pub beam: String,
    pub beam_alpha: f64,
    pub light: bool,
}

/// Thème de base : à l'écran (sombre) ou pour l'export et l'impression (fond blanc).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Screen,
    Print,
}

impl Theme {
    pub fn screen() -> Self {
        let s = |v: &str| v.to_string();
        Self {
            bg: s("#0f1216"),
            minor: s("#1a1f26"),
            major: s("#29303a"),
            text: s("#d9dde3"),
            muted: s("#8a919c"),
            body: s("#e6e9ee"),
            body_fill: s("#1d2229"),
            sel: s("#ffad3b"),
            cyc: s("#3a414c"),
            prop: s("#2a3038"),
            addr: s("#ffad3b"),
            beam: s("#ffd27a"),
            beam_alpha: 0.22,
            light: false,
        }
    }

    pub fn print() -> Self {
        let s = |v: &str| v.to_string();
        Self {
            bg: s("#ffffff"),
            minor: s("#eef0f2"),
            major: s("#d5d9de"),
            text: s("#111418"),
            muted: s("#5d6470"),
            body: s("#111418"),
            body_fill: s("#ffffff"),
            sel: s("#d97a00"),
            cyc: s("#9aa1ab"),
            prop: s("#e9ebee"),
            addr: s("#9a5b00"),
            beam: s("#f0b040"),
            beam_alpha: 0.16,
            light: true,
        }
    }
}

fn rgb_of(hex: &str) -> Option<[f64; 3]> {
    let h = hex.trim();
    let h = h.strip_prefix('#').unwrap_or(h);
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([((n >> 16) & 255) as f64, ((n >> 8) & 255) as f64, (n & 255) as f64])
}

fn lum(rgb: [f64; 3]) -> f64 {
    (0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]) / 255.0
}

fn mix(a: &str, b: &str, k: f64) -> String {
    let (Some(x), Some(y)) = (rgb_of(a), rgb_of(b)) else { return a.to_string() };
    let c: Vec<String> = (0..3).map(|i| format!("{:02x}", round(x[i] + (y[i] - x[i]) * k) as i64)).collect();
    format!("#{}", c.concat())
}

/// `themeFor` : thème du dessin. `bg` : couleur de fond choisie (None : automatique — sombre à
/// l'écran, blanc à l'export). Sur un fond clair, projecteurs, traits et textes passent en foncé.
pub fn theme_for(theme: ThemeName, bg: Option<&str>) -> Theme {
    let base = match theme {
        ThemeName::Screen => Theme::screen(),
        ThemeName::Print => Theme::print(),
    };
    let Some((bg, rgb)) = bg.and_then(|b| rgb_of(b).map(|r| (b, r))) else { return base };
    let light = lum(rgb) > 0.5;
    let mut t = if light { Theme::print() } else { Theme::screen() };
    t.bg = bg.to_string();
    t.minor = mix(bg, &t.text, 0.08);
    t.major = mix(bg, &t.text, 0.2);
    t.body_fill = mix(bg, &t.text, 0.07);
    t.prop = mix(bg, &t.text, 0.12);
    t.cyc = mix(bg, &t.text, 0.4);
    t
}

/// Couleur choisie par l'utilisateur, remplacée par celle du texte si elle se confond avec le fond.
fn readable(color: &str, t: &Theme) -> String {
    match (rgb_of(color), rgb_of(&t.bg)) {
        (Some(c), Some(b)) if (lum(c) - lum(b)).abs() < 0.22 => t.text.clone(),
        _ => color.to_string(),
    }
}

// ------------------------------------------------------------------ vue 3D
/// Ce que la vue 3D sait d'un projecteur (`viewport.outputOf`) : sa sortie (teinte du faisceau) et
/// son pied. Par défaut : pas de sortie (faisceaux aux couleurs du thème), pied recalculé d'après la
/// hauteur du projecteur.
pub trait Viewport {
    /// Sortie du projecteur `id` : r, g, b (sRGB linéaire) de chaque pixel ; None : inconnue.
    fn output(&self, _id: &str) -> Option<Vec<f64>> {
        None
    }

    /// Pied retenu pour le projecteur.
    fn support(&self, f: &Fixture, ty: &FixtureType) -> Support {
        support_of(f, ty)
    }
}

/// Sans vue 3D : pieds recalculés d'après la hauteur, faisceaux aux couleurs du thème.
#[derive(Debug, Clone, Copy, Default)]
pub struct ByHeight;

impl Viewport for ByHeight {}

/// `ByHeight`, à prêter aux options par défaut.
pub static BY_HEIGHT: ByHeight = ByHeight;

/// Couleur moyenne émise par un projecteur (`outputCss`, pour teinter son faisceau sur le plan).
pub fn output_css(out: &[f64], pixels: usize, fallback: &str) -> String {
    if pixels == 0 {
        return fallback.to_string();
    }
    let v = |i: usize| out.get(i).copied().unwrap_or(f64::NAN);
    let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
    for p in 0..pixels {
        r += v(p * 3);
        g += v(p * 3 + 1);
        b += v(p * 3 + 2);
    }
    let n = pixels as f64;
    let m = max_of([r, g, b]) / n;
    if m < 0.02 || m.is_nan() {
        return fallback.to_string();
    }
    let c = |x: f64| round(255.0 * linear_to_srgb(min2(1.0, x / n / m)));
    format!("rgb({},{},{})", num(c(r)), num(c(g)), num(c(b)))
}

// ------------------------------------------------------------------ options d'affichage
/// Ce qui s'affiche sur le plan (« Affichage ») ; par défaut « Matériel seulement », comme la
/// version JavaScript : projecteurs, pieds et grip, sans le décor, le studio ni les annotations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlotOptions {
    pub names: bool,
    /// numéros de fixture (Fix)
    pub fixtures: bool,
    pub addresses: bool,
    /// pieds, hauteurs et accessoires
    pub supports: bool,
    pub beams: bool,
    pub grid: bool,
    pub underlay: bool,
    /// pieds seuls
    pub stands: bool,
    pub grip: bool,
    pub sandbags: bool,
    /// cyclo
    pub studio: bool,
    /// formes, textes, flèches, mesures
    pub annotations: bool,
    // décor, par catégorie
    pub people: bool,
    pub animals: bool,
    pub vehicles: bool,
    pub furniture: bool,
    pub set: bool,
    pub outdoor: bool,
    pub props: bool,
    pub filmset: bool,
}

impl Default for PlotOptions {
    fn default() -> Self {
        Self::equipment_only()
    }
}

/// Catégories du décor (`PROP_CATEGORIES`) : identifiant et nom.
pub const DECOR: [(&str, &str); 8] = [
    ("people", "Personnages"),
    ("animals", "Animaux"),
    ("vehicles", "Véhicules"),
    ("furniture", "Mobilier"),
    ("set", "Décor et architecture"),
    ("outdoor", "Extérieur et nature"),
    ("props", "Accessoires"),
    ("filmset", "Plateau (caméra, son…)"),
];

impl PlotOptions {
    /// Préréglage « Matériel seulement » (le plan de feu qu'on envoie).
    pub fn equipment_only() -> Self {
        Self {
            names: true,
            fixtures: true,
            addresses: true,
            supports: true,
            beams: true,
            grid: true,
            underlay: true,
            stands: true,
            grip: true,
            sandbags: false,
            studio: false,
            annotations: false,
            people: false,
            animals: false,
            vehicles: false,
            furniture: false,
            set: false,
            outdoor: false,
            props: false,
            filmset: false,
        }
    }

    /// Préréglage « Tout afficher » (décor, studio, annotations, sacs de sable).
    pub fn show_all() -> Self {
        Self { sandbags: true, studio: true, annotations: true, people: true, animals: true, vehicles: true, furniture: true, set: true, outdoor: true, props: true, filmset: true, ..Self::equipment_only() }
    }

    /// Catégorie de décor affichée.
    pub fn decor(&self, cat: &str) -> bool {
        match cat {
            "people" => self.people,
            "animals" => self.animals,
            "vehicles" => self.vehicles,
            "furniture" => self.furniture,
            "set" => self.set,
            "outdoor" => self.outdoor,
            "props" => self.props,
            "filmset" => self.filmset,
            _ => false,
        }
    }

    /// Préréglage en cours : « equipment », « all » ou None (personnalisé). Seuls comptent les
    /// éléments du préréglage (pas les étiquettes).
    pub fn preset(&self) -> Option<&'static str> {
        let key = |o: &Self| [o.stands, o.grip, o.sandbags, o.studio, o.annotations, o.people, o.animals, o.vehicles, o.furniture, o.set, o.outdoor, o.props, o.filmset];
        let mine = key(self);
        if mine == key(&Self::equipment_only()) {
            Some("equipment")
        } else if mine.iter().all(|v| *v) {
            Some("all")
        } else {
            None
        }
    }
}

/// `objectShown` : le matériel (projecteurs, pieds, grip) par défaut, sans les sacs de sable ; le
/// décor par catégorie seulement si on le coche.
pub fn object_shown(ty: &ObjectType, options: &PlotOptions) -> bool {
    if ty.family() == Some("bag") {
        return options.sandbags;
    }
    if !ty.is_prop() {
        return if ty.family() == Some("stand") { options.stands } else { options.grip };
    }
    ty.cat().is_some_and(|c| options.decor(c))
}

// ------------------------------------------------------------------ données du plan
/// Vue : centre (m), échelle (px/m), taille (px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub cx: f64,
    pub cz: f64,
    pub s: f64,
    pub w: f64,
    pub h: f64,
}

/// Sélection affichée.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Selection {
    pub fixtures: Vec<String>,
    pub object_id: Option<String>,
    pub plot_id: Option<String>,
}

/// Poignée (rotation, sommet) en mètres.
#[derive(Debug, Clone, PartialEq)]
pub struct Handle {
    /// « rotate » : flèche de rotation dessinée dedans
    pub kind: String,
    pub x: f64,
    pub z: f64,
}

/// Style d'un élément du plan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemStyle {
    pub stroke: Option<String>,
    pub fill: Option<String>,
    pub width: Option<f64>,
    pub dash: bool,
    pub size: Option<f64>,
}

/// Élément du plan (forme, texte, flèche, mesure) : `scene.plot.items`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlotItem {
    pub id: Option<String>,
    /// rect, ellipse, polygon, freehand, line, arrow, measure, text
    pub kind: String,
    /// points [x, z] en mètres
    pub points: Vec<P2>,
    pub text: Option<String>,
    pub style: ItemStyle,
    /// hauteur du volume dans la vue 3D (0 : forme plate)
    pub height: f64,
    /// tracé en cours
    pub preview: bool,
}

impl PlotItem {
    /// Lecture d'un élément du projet (règles de JavaScript : valeurs absentes, nulles ou vides).
    pub fn from_json(v: &Value) -> Self {
        let st = v.get("style");
        let s = |k: &str| st.and_then(|s| s.get(k));
        Self {
            id: text_of(v.get("id")).map(str::to_string),
            kind: v.get("kind").and_then(Value::as_str).unwrap_or("").to_string(),
            points: v.get("points").and_then(Value::as_array).map(|a| a.iter().filter_map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect()).unwrap_or_default(),
            text: v.get("text").filter(|t| truthy(Some(t))).map(string_of),
            style: ItemStyle {
                stroke: s("stroke").filter(|t| truthy(Some(t))).map(string_of),
                fill: s("fill").filter(|t| truthy(Some(t))).map(string_of),
                width: num_truthy(s("width")),
                dash: truthy(s("dash")),
                size: num_truthy(s("size")),
            },
            height: v.get("height").and_then(Value::as_f64).unwrap_or(0.0),
            preview: truthy(v.get("preview")),
        }
    }

    /// Élément tel qu'enregistré dans le projet.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        if let Some(id) = &self.id {
            m.insert("id".into(), id.clone().into());
        }
        m.insert("kind".into(), self.kind.clone().into());
        m.insert("points".into(), json!(self.points));
        if let Some(t) = &self.text {
            m.insert("text".into(), t.clone().into());
        }
        let st = &self.style;
        m.insert(
            "style".into(),
            json!({ "stroke": st.stroke.clone().unwrap_or_else(|| "#ffad3b".into()), "fill": st.fill.clone().unwrap_or_else(|| "none".into()), "width": st.width.unwrap_or(2.0), "dash": st.dash, "size": st.size.unwrap_or(14.0) }),
        );
        m.insert("height".into(), json!(self.height));
        Value::Object(m)
    }
}

/// Image de fond (photo, plan d'architecte) calée à l'échelle.
#[derive(Debug, Clone, PartialEq)]
pub struct Underlay {
    pub url: String,
    pub x: f64,
    pub z: f64,
    /// largeur (m)
    pub width: f64,
    /// largeur / hauteur
    pub aspect: f64,
    /// degrés
    pub rotation: f64,
    pub opacity: f64,
    /// affichée au sol dans la vue 3D
    pub floor3d: bool,
}

/// Cartouche : production, chef électro, séquence et plan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TitleInfo {
    pub production: Option<String>,
    pub scene: Option<String>,
    pub author: Option<String>,
    /// « Séquence 1 › Plan 2 » (projet à plusieurs plans)
    pub plan: Option<String>,
}

/// Le plan 2D d'un projet (`scene.plot`) : éléments, image de fond, couleur du fond, cartouche.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlotData {
    pub items: Vec<PlotItem>,
    pub underlay: Option<Underlay>,
    pub background: Option<String>,
    pub title: TitleInfo,
}

impl PlotData {
    pub fn of(project: &Project) -> Self {
        let Some(plot) = project.extra.get("plot") else { return Self::default() };
        let items = plot.get("items").and_then(Value::as_array).map(|a| a.iter().map(PlotItem::from_json).collect()).unwrap_or_default();
        let underlay = plot.get("underlay").filter(|u| u.is_object()).map(|u| {
            let n = |k: &str| u.get(k).and_then(Value::as_f64).unwrap_or(f64::NAN);
            Underlay {
                url: u.get("url").filter(|v| truthy(Some(v))).map(string_of).unwrap_or_default(),
                x: n("x"),
                z: n("z"),
                width: n("width"),
                aspect: num_truthy(u.get("aspect")).unwrap_or(1.0),
                rotation: num_truthy(u.get("rotation")).unwrap_or(0.0),
                opacity: u.get("opacity").and_then(Value::as_f64).unwrap_or(0.6),
                floor3d: truthy(u.get("floor3d")),
            }
        });
        let t = plot.get("title");
        let s = |k: &str| t.and_then(|t| t.get(k)).filter(|v| !v.is_null()).map(string_of);
        Self { items, underlay, background: plot.get("background").and_then(Value::as_str).map(str::to_string), title: TitleInfo { production: s("production"), scene: None, author: s("author"), plan: None } }
    }
}

/// Réglages du dessin (`renderPlot({ theme, options, sel, preview, handles, viewport })`).
pub struct RenderOptions<'a> {
    pub theme: ThemeName,
    pub options: PlotOptions,
    pub sel: Selection,
    /// élément en cours de tracé
    pub preview: Option<PlotItem>,
    pub handles: Vec<Handle>,
    /// sortie et pieds des projecteurs ; None : ni faisceaux teintés, ni pieds dans les étiquettes
    pub viewport: Option<&'a dyn Viewport>,
    pub tr: Option<Tr<'a>>,
    /// image de fond à dessiner à la place de son adresse du projet (« /assets/… », que resvg ne
    /// sait pas lire) : data URL (voir [`crate::data_url`]) ou chemin du fichier
    pub underlay_href: Option<String>,
}

impl Default for RenderOptions<'_> {
    fn default() -> Self {
        Self { theme: ThemeName::Screen, options: PlotOptions::default(), sel: Selection::default(), preview: None, handles: Vec::new(), viewport: Some(&BY_HEIGHT), tr: None, underlay_href: None }
    }
}

// ------------------------------------------------------------------ placement des étiquettes
const LINE_H: f64 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Cls {
    /// nom du projecteur
    N,
    /// patch
    A,
    /// nom d'un objet
    O,
    /// pieds, accessoires
    S,
}

/// Largeur approximative d'un texte (px) selon sa police.
fn text_w(t: &str, cls: Cls) -> f64 {
    len16(t) as f64
        * match cls {
            Cls::N => 6.4,
            Cls::A => 6.0,
            Cls::O => 5.6,
            Cls::S => 5.1,
        }
}

#[derive(Debug, Clone, Copy)]
struct Bx {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

fn boxes_overlap(a: &Bx, b: &Bx) -> bool {
    a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1
}

fn seg_intersect(p: P2, q: P2, r: P2, s: P2) -> bool {
    let d = |a: P2, b: P2, c: P2| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    let (d1, d2, d3, d4) = (d(r, s, p), d(r, s, q), d(p, q, r), d(p, q, s));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Polygone que les étiquettes évitent (corps, faisceau, objet).
struct Poly {
    pts: Vec<P2>,
    owner: String,
    bbox: Bx,
}

impl Poly {
    fn new(pts: Vec<P2>, owner: String) -> Self {
        let bbox = Bx { x0: min_of(pts.iter().map(|p| p[0])), x1: max_of(pts.iter().map(|p| p[0])), y0: min_of(pts.iter().map(|p| p[1])), y1: max_of(pts.iter().map(|p| p[1])) };
        Self { pts, owner, bbox }
    }

    /// Une boîte (étiquette) touche-t-elle le polygone ?
    fn hit(&self, b: &Bx) -> bool {
        if !boxes_overlap(b, &self.bbox) {
            return false;
        }
        let corners = [[b.x0, b.y0], [b.x1, b.y0], [b.x1, b.y1], [b.x0, b.y1]];
        if self.pts.iter().any(|&[x, y]| x > b.x0 && x < b.x1 && y > b.y0 && y < b.y1) {
            return true;
        }
        if self.pts.len() >= 3 && corners.iter().any(|c| point_in_polygon(*c, &self.pts)) {
            return true;
        }
        let n = self.pts.len();
        for i in 0..n {
            let (p, q) = (self.pts[i], self.pts[(i + 1) % n]);
            for j in 0..4 {
                if seg_intersect(p, q, corners[j], corners[(j + 1) % 4]) {
                    return true;
                }
            }
        }
        false
    }
}

struct LabelReq {
    owner: String,
    x: f64,
    y: f64,
    radius: f64,
    lines: Vec<(String, Cls)>,
    sel: bool,
    pref: Option<P2>,
}

struct Placed {
    req: LabelReq,
    bx: Bx,
    u: P2,
    d: f64,
    ext: f64,
}

// 0,7071 et non 1/√2 exact : la valeur de la version JavaScript (mêmes positions au pixel près)
#[allow(clippy::approx_constant)]
const DIRS: [P2; 8] = [[0.0, 1.0], [0.0, -1.0], [1.0, 0.0], [-1.0, 0.0], [0.7071, 0.7071], [-0.7071, 0.7071], [0.7071, -0.7071], [-0.7071, -0.7071]];

/// Place les étiquettes : plusieurs positions autour de l'élément sont essayées et la moins gênante
/// est retenue (aucun chevauchement d'étiquette, puis aucun élément recouvert, puis le moins de
/// faisceaux traversés, au plus près).
fn place_labels(reqs: Vec<LabelReq>, solids: &[Poly], softs: &[Poly], w_px: f64, h_px: f64) -> Vec<Placed> {
    let mut placed: Vec<Placed> = Vec::new();
    for r in reqs {
        let w = max_of(r.lines.iter().map(|(t, c)| text_w(t, *c))) + 4.0;
        let h = r.lines.len() as f64 * LINE_H + 2.0;
        let dirs: Vec<P2> = r.pref.into_iter().chain(DIRS).collect();
        let mut best: Option<(f64, Bx, P2, f64, f64)> = None;
        for (di, u) in dirs.iter().enumerate() {
            for extra in [4.0, 14.0, 28.0, 46.0] {
                let d = r.radius + extra;
                let ext = u[0].abs() * (w / 2.0) + u[1].abs() * (h / 2.0);
                let cx = r.x + u[0] * (d + ext);
                let cy = r.y + u[1] * (d + ext);
                let b = Bx { x0: cx - w / 2.0, x1: cx + w / 2.0, y0: cy - h / 2.0, y1: cy + h / 2.0 };
                let mut score = di as f64 * 0.6 + extra * 0.9;
                for p in &placed {
                    if boxes_overlap(&b, &p.bx) {
                        score += 1000.0;
                    }
                }
                for s in solids {
                    if s.owner != r.owner && s.hit(&b) {
                        score += 380.0;
                    }
                }
                for s in softs {
                    if s.owner != r.owner && s.hit(&b) {
                        score += 18.0;
                    }
                }
                if b.x0 < 2.0 || b.y0 < 2.0 || b.x1 > w_px - 2.0 || b.y1 > h_px - 2.0 {
                    score += 250.0;
                }
                if best.is_none_or(|bb| score < bb.0) {
                    best = Some((score, b, *u, d, ext));
                }
            }
        }
        let (_, bx, u, d, ext) = best.expect("au moins une direction");
        placed.push(Placed { req: r, bx, u, d, ext });
    }
    placed
}

/// Rayon (px) d'un élément vu de son centre : les étiquettes se placent au-delà.
fn radius_of(poly: &[P2], x: f64, y: f64) -> f64 {
    max_of(std::iter::once(6.0).chain(poly.iter().map(|[px, py]| hypot(px - x, py - y))))
}

/// Bande étroite autour d'un segment (tube d'un cadre, que les étiquettes évitent).
fn strip([ax, ay]: P2, [bx, by]: P2, r: f64) -> Vec<P2> {
    let l = hypot(bx - ax, by - ay);
    let l = if l == 0.0 || l.is_nan() { 1.0 } else { l };
    let nx = (-(by - ay) / l) * r;
    let ny = ((bx - ax) / l) * r;
    vec![[ax + nx, ay + ny], [bx + nx, by + ny], [bx - nx, by - ny], [ax - nx, ay - ny]]
}

/// Polygone resserré vers son centre (liseré intérieur).
fn inset_poly(poly: &[P2], k: f64) -> Vec<P2> {
    let n = poly.len() as f64;
    let cx = poly.iter().fold(0.0, |a, p| a + p[0]) / n;
    let cz = poly.iter().fold(0.0, |a, p| a + p[1]) / n;
    poly.iter().map(|[x, z]| [x + (cx - x) * k, z + (cz - z) * k]).collect()
}

fn stand_glyph(x: f64, z: f64, color: &str) -> String {
    format!(
        r#"<g stroke="{color}" stroke-width="1.4"><line x1="{x}" y1="{z}" x2="{}" y2="{}"/><line x1="{x}" y1="{z}" x2="{}" y2="{}"/><line x1="{x}" y1="{z}" x2="{x}" y2="{}"/></g>"#,
        num(x - 7.0),
        num(z + 5.0),
        num(x + 7.0),
        num(z + 5.0),
        num(z - 8.0),
        x = num(x),
        z = num(z)
    )
}

/// `accessoryLabel` : accessoires montés, en clair (« Chimera M + 216 · Grille 40° · CTO ») ; vide
/// s'il n'y en a pas.
pub fn accessory_label(ty: &FixtureType, f: &Fixture, t: Option<Tr>) -> String {
    let settings = Value::Object(f.settings.extra.clone());
    let e = accessory_effect(ty, Some(&settings), ty.beam_angle.filter(|a| *a != 0.0).unwrap_or(40.0));
    let mut parts = Vec::new();
    if let Some(m) = e.modifier {
        let front = match e.front {
            Some(Front::None) => format!(" + {}", tr(t, "sans toile")),
            Some(Front::Filter(fl)) => format!(" + {}", fl.name.split(" — ").next().unwrap_or("")),
            None => String::new(),
        };
        parts.push(format!("{}{front}", tr(t, &m.short)));
    }
    if let Some(g) = e.grid {
        parts.push(tr(t, &g.short));
    }
    if let Some(g) = e.gel {
        let mut it = g.name.split(" — ");
        let first = it.next().unwrap_or("");
        parts.push(match it.next().filter(|s| !s.is_empty()) {
            Some(second) => format!("{first} {}", strip_parenthesis(second)),
            None => g.name.clone(),
        });
    }
    parts.join(" · ")
}

/// `s.replace(/\s*\(.*\)/, '')` : retire la première parenthèse (jusqu'à la dernière fermante).
fn strip_parenthesis(s: &str) -> String {
    let Some(open) = s.find('(') else { return s.to_string() };
    let Some(close) = s.rfind(')').filter(|c| *c > open) else { return s.to_string() };
    let start = s[..open].trim_end_matches(char::is_whitespace).len();
    format!("{}{}", &s[..start], &s[close + 1..])
}

/// Cartouche du plan (`titleBlock`) : cadre de titre en bas à droite de l'export.
pub fn title_block(w: f64, h: f64, title: &TitleInfo, theme: ThemeName, date: &str, background: Option<&str>, t: Option<Tr>) -> String {
    let th = theme_for(theme, background);
    let bw = 300.0;
    let plan = title.plan.as_deref().filter(|p| !p.is_empty());
    let extra = if plan.is_some() { 16.0 } else { 0.0 };
    let bh = 74.0 + extra;
    let x = w - bw - 16.0;
    let y = h - bh - 16.0;
    let plan_line = plan.map_or(String::new(), |p| format!("\n<text x=\"{}\" y=\"{}\" font-size=\"11\" font-weight=\"600\" fill=\"{}\">{}</text>", num(x + 10.0), num(y + 54.0), th.text, esc(p)));
    let production = title.production.as_deref().filter(|p| !p.is_empty()).map_or_else(|| tr(t, "Plan de feu"), str::to_string);
    let author = title.author.as_deref().filter(|a| !a.is_empty()).map_or(String::new(), |a| tr(t, "Chef électro : {0}").replace("{0}", a));
    format!(
        "<g><rect x=\"{x}\" y=\"{y}\" width=\"{}\" height=\"{}\" fill=\"{bg}\" stroke=\"{text}\" stroke-width=\"1.2\"/>\n\
<text x=\"{x10}\" y=\"{}\" font-size=\"14\" font-weight=\"700\" fill=\"{text}\">{}</text>\n\
<text x=\"{x10}\" y=\"{}\" font-size=\"11\" fill=\"{text}\">{}</text>{plan_line}\n\
<text x=\"{x10}\" y=\"{}\" font-size=\"10\" fill=\"{muted}\">{}</text>\n\
<text x=\"{}\" y=\"{}\" font-size=\"9\" fill=\"{muted}\" text-anchor=\"end\">{} · ARKLIGHT</text></g>",
        num(bw),
        num(bh),
        num(y + 20.0),
        esc(&production),
        num(y + 38.0),
        esc(title.scene.as_deref().unwrap_or("")),
        num(y + 54.0 + extra),
        esc(&author),
        num(x + bw - 10.0),
        num(y + 66.0 + extra),
        esc(date),
        x = num(x),
        y = num(y),
        x10 = num(x + 10.0),
        bg = th.bg,
        text = th.text,
        muted = th.muted,
    )
}

// ------------------------------------------------------------------ dessin
/// Repère écran : mètres → pixels (arrondis au dixième).
struct Frame {
    cx: f64,
    cz: f64,
    s: f64,
    w: f64,
    h: f64,
}

impl Frame {
    fn x(&self, x: f64) -> f64 {
        f1((x - self.cx) * self.s + self.w / 2.0)
    }

    fn z(&self, z: f64) -> f64 {
        f1((z - self.cz) * self.s + self.h / 2.0)
    }

    fn pts(&self, list: &[P2]) -> String {
        list.iter().map(|[x, z]| format!("{},{}", num(self.x(*x)), num(self.z(*z)))).collect::<Vec<_>>().join(" ")
    }
}

/// `renderPlot` : contenu SVG du plan (sans la balise `<svg>` ; voir [`crate::svg_document`]).
pub fn render_plot(scene: &Project, view: &View, opts: &RenderOptions) -> String {
    let mut plot = PlotData::of(scene);
    if let (Some(u), Some(href)) = (plot.underlay.as_mut(), opts.underlay_href.as_ref()) {
        u.url = href.clone();
    }
    render_with(scene, &plot, view, opts)
}

/// `renderPlot` avec le plan déjà lu (export : image de fond remplacée par son contenu).
pub(crate) fn render_with(scene: &Project, plot: &PlotData, view: &View, opts: &RenderOptions) -> String {
    let th = theme_for(opts.theme, plot.background.as_deref());
    let options = &opts.options;
    let t = opts.tr;
    let fr = Frame { cx: view.cx, cz: view.cz, s: view.s, w: view.w, h: view.h };
    let (s, w_px, h_px) = (view.s, view.w, view.h);
    let mut out: Vec<String> = Vec::new();
    out.push(format!(r#"<rect x="0" y="0" width="{}" height="{}" fill="{}"/>"#, num(w_px), num(h_px), th.bg));

    // ---- grille (1 m, traits forts tous les 5 m)
    if options.grid {
        let x0 = (view.cx - w_px / 2.0 / s).floor();
        let x1 = (view.cx + w_px / 2.0 / s).ceil();
        let z0 = (view.cz - h_px / 2.0 / s).floor();
        let z1 = (view.cz + h_px / 2.0 / s).ceil();
        if s >= 6.0 {
            let mut x = x0;
            while x <= x1 {
                out.push(format!(r#"<line x1="{0}" y1="0" x2="{0}" y2="{1}" stroke="{2}" stroke-width="1"/>"#, num(fr.x(x)), num(h_px), if x % 5.0 != 0.0 { &th.minor } else { &th.major }));
                x += 1.0;
            }
            let mut z = z0;
            while z <= z1 {
                out.push(format!(r#"<line x1="0" y1="{0}" x2="{1}" y2="{0}" stroke="{2}" stroke-width="1"/>"#, num(fr.z(z)), num(w_px), if z % 5.0 != 0.0 { &th.minor } else { &th.major }));
                z += 1.0;
            }
        }
    }

    // ---- image de fond (photo, plan d'architecte) calée à l'échelle
    if let Some(u) = plot.underlay.as_ref().filter(|u| !u.url.is_empty() && options.underlay) {
        let w = u.width * s;
        let h = (u.width / u.aspect) * s;
        out.push(format!(
            r#"<image href="{}" x="{}" y="{}" width="{}" height="{}" opacity="{}" transform="rotate({} {} {})" preserveAspectRatio="none"/>"#,
            esc(&u.url),
            num(f1(fr.x(u.x) - w / 2.0)),
            num(f1(fr.z(u.z) - h / 2.0)),
            num(f1(w)),
            num(f1(h)),
            num(u.opacity),
            num(u.rotation),
            num(fr.x(u.x)),
            num(fr.z(u.z))
        ));
    }

    // ---- studio : cyclo
    if scene.environment.cyc && options.studio {
        out.push(format!(r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="0.25"/>"#, num(fr.x(-8.0)), num(fr.z(-7.0)), num(f1(16.0 * s)), num(f1(1.2 * s)), th.cyc));
        out.push(format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="3"/>"#, num(fr.x(-8.0)), num(fr.z(-7.0)), num(fr.x(8.0)), num(fr.z(-7.0)), th.cyc));
        out.push(format!(r#"<text x="{}" y="{}" fill="{}" font-size="11" text-anchor="middle">{}</text>"#, num(fr.x(0.0)), num(fr.z(-7.0) - 6.0), th.muted, tr(t, "Cyclo (fond)")));
    }

    // ---- formes, textes, flèches, mesures (masquables ; le tracé en cours reste visible)
    let mut items: Vec<&PlotItem> = if options.annotations { plot.items.iter().collect() } else { Vec::new() };
    let preview = opts.preview.clone().map(|p| PlotItem { preview: true, ..p });
    items.extend(preview.as_ref());
    for it in items {
        let selected = it.id.is_some() && it.id == opts.sel.plot_id;
        out.push(draw_item(it, &fr, &th, selected));
    }

    let mut solids: Vec<Poly> = Vec::new(); // corps des projecteurs et objets : les étiquettes les évitent
    let mut softs: Vec<Poly> = Vec::new(); // faisceaux : évités si possible
    let mut fixture_labels: Vec<LabelReq> = Vec::new();
    let mut object_labels: Vec<LabelReq> = Vec::new();

    // ---- objets : décor (personnages, véhicules, mobilier…) et grip
    for o in &scene.objects {
        let Some(ty) = object_type(&o.type_id) else { continue };
        if !object_shown(&ty, options) {
            continue;
        }
        let p = object_plan(o, ty);
        let is_sel = opts.sel.object_id.as_deref() == Some(o.id.as_str());
        let poly: Vec<P2> = p.poly.iter().map(|[x, z]| [fr.x(*x), fr.z(*z)]).collect();
        let props = props_of(o);
        let frame = ty.family() == Some("frame");
        let is_prop = ty.is_prop();
        let st = match ty {
            ObjectType::Grip(g) if !is_prop => grip_stands(g, props),
            _ => None,
        };
        let cloth_id = props.get("cloth").filter(|c| !c.is_null()).map(string_of);
        let cloth_def = cloth_id.as_deref().and_then(cloth);
        let filter_def = if cloth_def.is_none() { cloth_id.as_deref().and_then(filter) } else { None };
        if is_prop {
            if let ObjectType::Prop(pt) = ty {
                out.push(prop_glyph(o, pt, &fr, &th, is_sel));
            }
        } else if frame {
            // cadre transparent : on ne dessine que les tubes (la toile ne cache rien du plan) ;
            // une gélatine se devine à un liseré de sa couleur, les pieds sont aux oreilles du cadre
            out.push(format!(r#"<polygon points="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linejoin="round"/>"#, fr.pts(&p.poly), if is_sel { &th.sel } else { &th.body }, if is_sel { 3 } else { 2 }));
            if let Some(fl) = filter_def {
                out.push(format!(r#"<polygon points="{}" fill="none" stroke="{}" stroke-width="1.5" stroke-dasharray="6 3"/>"#, fr.pts(&inset_poly(&p.poly, 0.06)), fl.color));
            }
            if let ObjectType::Grip(g) = ty {
                for [x, z] in frame_ears(o, grip_wh(g).0) {
                    out.push(stand_glyph(fr.x(x), fr.z(z), &th.body));
                }
            }
        } else {
            let family = ty.family();
            let fill = match family {
                Some("box") => "#8a6a45".to_string(),
                Some("bag") => "#6b4a2a".to_string(),
                Some("bounce") => "#f4f4f0".to_string(),
                _ => match (filter_def, cloth_def) {
                    (Some(fl), _) => fl.color.clone(),
                    (None, Some(c)) if c.opacity >= 1.0 => (if c.color == 0x111111 { "#222" } else { "#f4f4f0" }).to_string(),
                    _ => "none".to_string(),
                },
            };
            let dashed = filter_def.is_some() || cloth_def.is_some_and(|c| c.opacity < 1.0 && family != Some("bounce"));
            out.push(format!(
                r#"<polygon points="{}" fill="{fill}" {} stroke="{}" stroke-width="{}" {}/>"#,
                fr.pts(&p.poly),
                if filter_def.is_some() { r#"fill-opacity="0.45""# } else { "" },
                if is_sel { &th.sel } else { &th.body },
                if is_sel { "2.5" } else { "1.5" },
                if dashed { r#"stroke-dasharray="5 3""# } else { "" }
            ));
            if family == Some("stand") {
                out.push(stand_glyph(fr.x(o.position[0]), fr.z(o.position[2]), &th.body));
            }
        }
        // le décor (voiture, canapé…) peut passer sous une étiquette ; le grip, non. Un cadre est vide :
        // les étiquettes peuvent se poser dedans, en évitant ses tubes (y compris la sienne)
        if frame {
            for k in 0..poly.len() {
                softs.push(Poly::new(strip(poly[k], poly[(k + 1) % poly.len()], 3.0), format!("{}#tube", o.id)));
            }
        } else if is_prop {
            softs.push(Poly::new(poly.clone(), o.id.clone()));
        } else {
            solids.push(Poly::new(poly.clone(), o.id.clone()));
        }
        if options.names {
            let text = if is_prop {
                o.name.clone()
            } else {
                let cl = match &cloth_id {
                    Some(c) if ty.cloths().is_some() && truthy(props.get("cloth")) => format!(" · {}", tr(t, &cloth_short(c))),
                    _ => String::new(),
                };
                format!("{}{cl}", tr(t, ty.short().unwrap_or("")))
            };
            let (x, y) = (fr.x(p.center[0]), fr.z(p.center[1]));
            let mut lines = vec![(text, Cls::O)];
            if let Some(st) = st.as_ref().filter(|_| frame && options.supports) {
                let short = support_def(&st.id).and_then(|d| d.get("short")).and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or(&st.id);
                lines.push((format!("{} × {}", st.count, tr(t, short)), Cls::S));
            }
            let r = radius_of(&poly, x, y) * 0.75;
            object_labels.push(LabelReq { owner: o.id.clone(), x, y, radius: if frame { min2(r, 20.0) } else { r }, lines, sel: is_sel, pref: None });
        }
    }

    // ---- projecteurs : faisceau et corps
    let lib = library();
    for f in &scene.fixtures {
        let Some(ty) = lib.fixture(&f.type_id) else { continue };
        let p = fixture_plan(f, ty);
        let is_sel = opts.sel.fixtures.contains(&f.id);
        let color = opts.viewport.and_then(|v| v.output(&f.id)).map_or_else(|| th.beam.clone(), |out| output_css(&out, ty.pixels, &th.beam));
        if let Some([dx, dz]) = p.dir.filter(|_| options.beams) {
            let half = (min2(120.0, ty.beam_angle.filter(|a| *a != 0.0 && !a.is_nan()).unwrap_or(40.0)) / 2.0) * (PI / 180.0);
            let l = 1.6;
            let rot = |a: f64| [dx * a.cos() - dz * a.sin(), dx * a.sin() + dz * a.cos()];
            let [ax, az] = rot(half);
            let [bx, bz] = rot(-half);
            let [fx, fz] = p.front;
            let tri = [[fx, fz], [fx + ax * l, fz + az * l], [fx + bx * l, fz + bz * l]];
            out.push(format!(r#"<polygon points="{}" fill="{color}" opacity="{}" stroke="none"/>"#, fr.pts(&tri), num(th.beam_alpha)));
            out.push(format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{color}" stroke-width="1" stroke-dasharray="4 3" opacity="0.8"/>"#, num(fr.x(fx)), num(fr.z(fz)), num(fr.x(fx + dx * l)), num(fr.z(fz + dz * l))));
            softs.push(Poly::new(tri.iter().map(|[x, z]| [fr.x(*x), fr.z(*z)]).collect(), f.id.clone()));
        }
        out.push(format!(r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#, fr.pts(&p.poly), th.body_fill, if is_sel { &th.sel } else { &th.body }, if is_sel { "2.5" } else { "1.4" }));
        if p.down {
            out.push(format!(r#"<circle cx="{}" cy="{}" r="3" fill="{color}"/>"#, num(fr.x(p.center[0])), num(fr.z(p.center[1])))); // pointe vers le sol
        }
        if f.state.get("power") == Some(&Value::Bool(false)) {
            out.push(format!(r#"<text x="{}" y="{}" fill="{}" font-size="9" text-anchor="middle">{}</text>"#, num(fr.x(p.center[0])), num(fr.z(p.center[1]) + 3.0), th.muted, tr(t, "éteint")));
        }
        let poly: Vec<P2> = p.poly.iter().map(|[x, z]| [fr.x(*x), fr.z(*z)]).collect();
        solids.push(Poly::new(poly.clone(), f.id.clone()));
        let mut lines = Vec::new();
        if options.names {
            lines.push((f.name.clone(), Cls::N));
        }
        if !ty.dmx.modes.is_empty() && (options.fixtures || options.addresses) {
            let line = patch_line(f, options.fixtures, options.addresses);
            if !line.is_empty() {
                lines.push((line, Cls::A));
            }
        }
        if options.supports {
            // accessoires montés (Chimera, dôme, louver, gélatine) puis pied et hauteur
            let sup = opts.viewport.map(|v| v.support(f, ty));
            let stand = match sup.as_ref().and_then(|s| s.short().map(|short| (s, short))) {
                Some((_, short)) => format!("{} · h {} m", tr(t, short), num(f1(p.height)).replacen('.', ",", 1)),
                None => String::new(),
            };
            let line = [accessory_label(ty, f, t), stand].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
            if !line.is_empty() {
                lines.push((line, Cls::S));
            }
        }
        if !lines.is_empty() {
            let (x, y) = (fr.x(p.center[0]), fr.z(p.center[1]));
            fixture_labels.push(LabelReq { owner: f.id.clone(), x, y, radius: radius_of(&poly, x, y), lines, sel: is_sel, pref: p.dir.map(|d| [-d[0], -d[1]]) });
        }
    }

    // ---- étiquettes : projecteurs d'abord, puis objets ; trait de rappel si elles s'éloignent
    let halo = format!(r#"stroke="{}" stroke-width="3" stroke-linejoin="round" paint-order="stroke""#, th.bg);
    fixture_labels.extend(object_labels);
    for l in place_labels(fixture_labels, &solids, &softs, w_px, h_px) {
        let (bx, dir, r) = (l.bx, l.u, &l.req);
        let mx = (bx.x0 + bx.x1) / 2.0;
        if l.d > r.radius + 12.0 {
            let ex = mx - dir[0] * l.ext;
            let ey = (bx.y0 + bx.y1) / 2.0 - dir[1] * l.ext;
            out.push(format!(
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="0.8" opacity="0.7"/>"#,
                num(f1(r.x + dir[0] * r.radius * 0.9)),
                num(f1(r.y + dir[1] * r.radius * 0.9)),
                num(f1(ex)),
                num(f1(ey)),
                th.muted
            ));
        }
        for (i, (text, cls)) in r.lines.iter().enumerate() {
            let y = bx.y0 + 11.0 + i as f64 * LINE_H;
            let style = match cls {
                Cls::N => format!(r#"font-weight="700" font-size="11" fill="{}""#, if r.sel { &th.sel } else { &th.text }),
                Cls::A => format!(r#"font-family="ui-monospace, Consolas, monospace" font-size="10" fill="{}""#, th.addr),
                Cls::O => format!(r#"font-size="10" fill="{}""#, if r.sel { &th.sel } else { &th.muted }),
                Cls::S => format!(r#"font-size="9" fill="{}""#, th.muted),
            };
            out.push(format!(r#"<text x="{}" y="{}" {style} {halo} text-anchor="middle">{}</text>"#, num(f1(mx)), num(f1(y)), esc(text)));
        }
    }

    // ---- poignées (rotation, sommets)
    for hd in &opts.handles {
        let (x, z) = (fr.x(hd.x), fr.z(hd.z));
        let arrow = if hd.kind == "rotate" { format!(r#"<path d="M{},{} a3,3 0 1,1 3,4" fill="none" stroke="{}" stroke-width="1.4"/>"#, num(x - 3.0), num(z - 1.0), th.sel) } else { String::new() };
        out.push(format!(r#"<circle cx="{}" cy="{}" r="6" fill="{}" stroke="{}" stroke-width="2"/>{arrow}"#, num(x), num(z), th.bg, th.sel));
    }

    // ---- échelle
    let nice = [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0].into_iter().find(|m| m * s >= 70.0).unwrap_or(100.0);
    let bx = 16.0;
    let by = h_px - 18.0;
    let end = num(f1(bx + nice * s));
    out.push(format!(
        r#"<line x1="{bx}" y1="{by}" x2="{end}" y2="{by}" stroke="{c}" stroke-width="2"/><line x1="{bx}" y1="{}" x2="{bx}" y2="{}" stroke="{c}" stroke-width="2"/><line x1="{end}" y1="{}" x2="{end}" y2="{}" stroke="{c}" stroke-width="2"/>"#,
        num(by - 4.0),
        num(by + 4.0),
        num(by - 4.0),
        num(by + 4.0),
        bx = num(bx),
        by = num(by),
        c = th.text
    ));
    out.push(format!(r#"<text x="{}" y="{}" fill="{}" font-size="11" text-anchor="middle">{} m</text>"#, num(f1(bx + (nice * s) / 2.0)), num(by - 7.0), th.text, num(nice).replacen('.', ",", 1)));
    out.join("\n")
}

/// Objet de décor vu de dessus (`propGlyph`) : son symbole, et le champ des caméras.
fn prop_glyph(o: &SceneObject, ty: &arklight_core::library::PropType, fr: &Frame, th: &Theme, is_sel: bool) -> String {
    let stroke = if is_sel { &th.sel } else { &th.muted };
    let yaw = if o.rotation[1].is_nan() { 0.0 } else { o.rotation[1] };
    let fwd = [yaw.sin(), yaw.cos()]; // avant de l'objet (+Z local) vu de dessus
    let side = [yaw.cos(), -yaw.sin()]; // +X local
    let pt = |lx: f64, lz: f64| {
        let x = o.position[0] + side[0] * lx + fwd[0] * lz;
        let z = o.position[2] + side[1] * lx + fwd[1] * lz;
        (num(fr.x(x)), num(fr.z(z)))
    };
    let mut parts = vec![prop_symbol(o, ty, fr.x(o.position[0]), fr.z(o.position[2]), fr.s, stroke, is_sel)];
    // champ de la caméra : deux bords du cadre horizontal, sur 6 m
    if matches!(ty.model.as_deref(), Some("camera" | "dolly")) {
        let (fov_h, _, _) = camera_fov(props_of(o));
        let l = 6.0;
        let tn = (fov_h / 2.0).tan() * l;
        let (lx, ly) = pt(0.0, 0.35);
        let (ax, ay) = pt(-tn, l);
        let (bx, by) = pt(tn, l);
        parts.push(format!(r#"<polygon points="{lx},{ly} {ax},{ay} {bx},{by}" fill="{}" opacity="0.06"/>"#, th.sel));
        parts.push(format!(r#"<polyline points="{ax},{ay} {lx},{ly} {bx},{by}" fill="none" stroke="{}" stroke-width="1" stroke-dasharray="6 4" opacity="0.8"/>"#, th.sel));
    }
    parts.concat()
}

/// Un élément du plan (forme, texte, flèche, mesure).
fn draw_item(it: &PlotItem, fr: &Frame, th: &Theme, selected: bool) -> String {
    let st = &it.style;
    let stroke = if selected { th.sel.clone() } else { readable(st.stroke.as_deref().unwrap_or("#ffad3b"), th) };
    let width = st.width.unwrap_or(2.0) + if selected { 1.0 } else { 0.0 };
    let dash = if st.dash || it.preview { r#"stroke-dasharray="6 4""# } else { "" };
    let filled = st.fill.as_deref().filter(|f| *f != "none");
    let fill = filled.map_or_else(|| r#"fill="none""#.to_string(), |f| format!(r#"fill="{f}" fill-opacity="0.25""#));
    let pts = &it.points;
    let mut parts: Vec<String> = Vec::new();
    let label = |x: f64, z: f64, text: Option<&str>| match text.filter(|t| !t.is_empty()) {
        Some(text) => format!(r#"<text x="{}" y="{}" fill="{}" font-size="{}" text-anchor="middle">{}</text>"#, num(f1(x)), num(f1(z)), th.text, num(st.size.unwrap_or(12.0)), esc(text)),
        None => String::new(),
    };
    let text = it.text.as_deref();
    match it.kind.as_str() {
        "rect" if pts.len() >= 2 => {
            let ([x0, z0], [x1, z1]) = (pts[0], pts[1]);
            let x = min2(fr.x(x0), fr.x(x1));
            let y = min2(fr.z(z0), fr.z(z1));
            parts.push(format!(r#"<rect x="{}" y="{}" width="{}" height="{}" stroke="{stroke}" stroke-width="{}" {fill} {dash}/>"#, num(x), num(y), num(f1((fr.x(x1) - fr.x(x0)).abs())), num(f1((fr.z(z1) - fr.z(z0)).abs())), num(width)));
            parts.push(label((fr.x(x0) + fr.x(x1)) / 2.0, (fr.z(z0) + fr.z(z1)) / 2.0 + 4.0, text));
        }
        "ellipse" if pts.len() >= 2 => {
            let ([x0, z0], [x1, z1]) = (pts[0], pts[1]);
            parts.push(format!(
                r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" stroke="{stroke}" stroke-width="{}" {fill} {dash}/>"#,
                num(f1((fr.x(x0) + fr.x(x1)) / 2.0)),
                num(f1((fr.z(z0) + fr.z(z1)) / 2.0)),
                num(f1((fr.x(x1) - fr.x(x0)).abs() / 2.0)),
                num(f1((fr.z(z1) - fr.z(z0)).abs() / 2.0)),
                num(width)
            ));
            parts.push(label((fr.x(x0) + fr.x(x1)) / 2.0, (fr.z(z0) + fr.z(z1)) / 2.0 + 4.0, text));
        }
        "polygon" | "freehand" => {
            let closed = it.kind == "polygon" && !it.preview;
            parts.push(format!(
                r#"<{} points="{}" stroke="{stroke}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="round" {} {dash}/>"#,
                if closed { "polygon" } else { "polyline" },
                fr.pts(pts),
                num(width),
                if closed || filled.is_some() { fill.as_str() } else { r#"fill="none""# }
            ));
            if text.is_some_and(|t| !t.is_empty()) {
                let n = pts.len() as f64;
                let cx = pts.iter().fold(0.0, |a, p| a + p[0]) / n;
                let cz = pts.iter().fold(0.0, |a, p| a + p[1]) / n;
                parts.push(label(fr.x(cx), fr.z(cz) + 4.0, text));
            }
        }
        "line" | "arrow" | "measure" if pts.len() >= 2 => {
            let ([ax, az], [bx, bz]) = (pts[0], pts[1]);
            let (pax, paz, pbx, pbz) = (fr.x(ax), fr.z(az), fr.x(bx), fr.z(bz));
            parts.push(format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{stroke}" stroke-width="{}" stroke-linecap="round" {dash}/>"#, num(pax), num(paz), num(pbx), num(pbz), num(width)));
            let ang = (pbz - paz).atan2(pbx - pax);
            if it.kind == "arrow" {
                let k = 8.0 + width * 2.0;
                let p1 = [pbx - k * (ang - 0.45).cos(), pbz - k * (ang - 0.45).sin()];
                let p2 = [pbx - k * (ang + 0.45).cos(), pbz - k * (ang + 0.45).sin()];
                parts.push(format!(r#"<polygon points="{},{} {},{} {},{}" fill="{stroke}"/>"#, num(pbx), num(pbz), num(f1(p1[0])), num(f1(p1[1])), num(f1(p2[0])), num(f1(p2[1]))));
                if text.is_some_and(|t| !t.is_empty()) {
                    parts.push(label(pax, paz - 6.0, text));
                }
            } else if it.kind == "measure" {
                let len = hypot(bx - ax, bz - az);
                let nx = -ang.sin() * 6.0;
                let nz = ang.cos() * 6.0;
                for [px, pz] in [[pax, paz], [pbx, pbz]] {
                    parts.push(format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{stroke}" stroke-width="{}"/>"#, num(f1(px - nx)), num(f1(pz - nz)), num(f1(px + nx)), num(f1(pz + nz)), num(width)));
                }
                let more = text.filter(|t| !t.is_empty()).map_or(String::new(), |t| format!(" — {t}"));
                let m = format!("{} m{more}", to_fixed(len, 2).replacen('.', ",", 1));
                parts.push(label((pax + pbx) / 2.0 - nx * 2.0, (paz + pbz) / 2.0 - nz * 2.0, Some(&m)));
            } else {
                parts.push(label((pax + pbx) / 2.0, (paz + pbz) / 2.0 - 6.0, text));
            }
        }
        "text" if !pts.is_empty() => {
            let color = if selected { th.sel.clone() } else { readable(st.stroke.as_deref().unwrap_or(&th.text), th) };
            parts.push(format!(
                r#"<text x="{}" y="{}" fill="{color}" font-size="{}" font-weight="600">{}</text>"#,
                num(fr.x(pts[0][0])),
                num(fr.z(pts[0][1])),
                num(st.size.unwrap_or(16.0)),
                esc(text.filter(|t| !t.is_empty()).unwrap_or("Texte"))
            ));
        }
        _ => {}
    }
    if it.height > 0.0 && !pts.is_empty() {
        parts.push(format!(r#"<text x="{}" y="{}" fill="{}" font-size="9">h {} m</text>"#, num(fr.x(pts[0][0]) + 4.0), num(fr.z(pts[0][1]) - 4.0), th.muted, num(it.height).replacen('.', ",", 1)));
    }
    parts.concat()
}
