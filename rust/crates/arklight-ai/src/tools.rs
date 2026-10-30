//! Outils de l'assistant (portage de `server/ai/tools.js`) : ce que l'IA peut lire et faire dans
//! le projet (ajouter, régler, adresser, grouper, décor, ambiance, séquences et plans). Chaque outil
//! renvoie un résultat court (JSON) que le modèle lit pour continuer, avec un résumé lisible pour
//! l'interface (`summary`), ou une erreur (`error`).
//!
//! Les modifications passent par des [`Edit`] appliqués au [`Document`] : tout ce que fait
//! l'assistant s'annule (Ctrl+Z). Un outil fait une étape d'annulation (deux quand des projecteurs
//! ajoutés reçoivent aussi un réglage de lumière ou un groupe : leurs identifiants n'existent
//! qu'après l'ajout). Les séquences et plans (`manage_plans`) ne s'annulent pas, comme dans la
//! version JavaScript (changer de plan repart d'un historique vide).

use crate::util::{clamp, finite, fmt_num, fmt_value, jnum, js_number, js_string, jvec, norm, num, obj, r2, r3, take_chars, truthy, vec3, words};
use arklight_core::addressing::{Options as AddressingOptions, Order, Scope, UNIVERSE_SIZE, footprint_of, plan_addressing};
use arklight_core::document::{Changes, Document, Edit};
use arklight_core::engine::console::console_held;
use arklight_core::engine::js_round;
use arklight_core::engine::output::{Source, control_source};
use arklight_core::library::{DmxMode, GripType, PropType};
use arklight_core::plans::Start;
use arklight_core::project::Vec3;
use arklight_core::{Fixture, FixtureType, SceneObject, library};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::sync::OnceLock;

/// Le comédien par défaut : son visage.
pub const ACTOR_AIM: Vec3 = [0.0, 1.6, 0.0];

// ------------------------------------------------------------------ orientation
/// Orientation d'un projecteur qui vise un point (son axe +Z vers la cible) : comme
/// `Object3D.lookAt` de three.js (matrice, quaternion puis angles d'Euler XYZ), arrondie au
/// millième.
pub fn aim_rotation(from: Vec3, to: Vec3) -> Vec3 {
    let sub = |a: Vec3, b: Vec3| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let len_sq = |v: Vec3| v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    let normalize = |v: Vec3| {
        let l = len_sq(v).sqrt();
        if l == 0.0 { v } else { [v[0] / l, v[1] / l, v[2] / l] }
    };
    let cross = |a: Vec3, b: Vec3| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let up = [0.0, 1.0, 0.0];
    // Matrix4.lookAt(eye = cible, target = position, up)
    let mut z = sub(to, from);
    if len_sq(z) == 0.0 {
        z[2] = 1.0;
    }
    z = normalize(z);
    let mut x = cross(up, z);
    if len_sq(x) == 0.0 {
        z[2] += 0.0001; // up et z parallèles (|up.z| ≠ 1)
        z = normalize(z);
        x = cross(up, z);
    }
    x = normalize(x);
    let y = cross(z, x);
    let (m11, m12, m13) = (x[0], y[0], z[0]);
    let (m21, m22, m23) = (x[1], y[1], z[1]);
    let (m31, m32, m33) = (x[2], y[2], z[2]);
    // Quaternion.setFromRotationMatrix
    let trace = m11 + m22 + m33;
    let (qw, qx, qy, qz);
    if trace > 0.0 {
        let s = 0.5 / (trace + 1.0).sqrt();
        qw = 0.25 / s;
        qx = (m32 - m23) * s;
        qy = (m13 - m31) * s;
        qz = (m21 - m12) * s;
    } else if m11 > m22 && m11 > m33 {
        let s = 2.0 * (1.0 + m11 - m22 - m33).sqrt();
        qw = (m32 - m23) / s;
        qx = 0.25 * s;
        qy = (m12 + m21) / s;
        qz = (m13 + m31) / s;
    } else if m22 > m33 {
        let s = 2.0 * (1.0 + m22 - m11 - m33).sqrt();
        qw = (m13 - m31) / s;
        qx = (m12 + m21) / s;
        qy = 0.25 * s;
        qz = (m23 + m32) / s;
    } else {
        let s = 2.0 * (1.0 + m33 - m11 - m22).sqrt();
        qw = (m21 - m12) / s;
        qx = (m13 + m31) / s;
        qy = (m23 + m32) / s;
        qz = 0.25 * s;
    }
    // Matrix4.makeRotationFromQuaternion puis Euler.setFromRotationMatrix (ordre XYZ)
    let (x2, y2, z2) = (qx + qx, qy + qy, qz + qz);
    let (xx, xy, xz) = (qx * x2, qx * y2, qx * z2);
    let (yy, yz, zz) = (qy * y2, qy * z2, qz * z2);
    let (wx, wy, wz) = (qw * x2, qw * y2, qw * z2);
    let r11 = 1.0 - (yy + zz);
    let r12 = xy - wz;
    let r13 = xz + wy;
    let r22 = 1.0 - (xx + zz);
    let r23 = yz - wx;
    let r32 = yz + wx;
    let r33 = 1.0 - (xx + yy);
    let ey = r13.clamp(-1.0, 1.0).asin();
    let (ex, ez) = if r13.abs() < 0.999_999_9 { ((-r23).atan2(r33), (-r12).atan2(r11)) } else { (r32.atan2(r22), 0.0) };
    [r3(ex), r3(ey), r3(ez)]
}

/// Lacet pour qu'un objet (personnage, meuble) fasse face à un point.
fn face_yaw(from: Vec3, to: Vec3) -> f64 {
    r3((to[0] - from[0]).atan2(to[2] - from[2]))
}

fn is_tube(ty: Option<&FixtureType>) -> bool {
    ty.is_some_and(|t| t.geometry.kind == "tube")
}

/// `type.geometry.length || 1`
fn tube_length(ty: Option<&FixtureType>) -> f64 {
    ty.and_then(|t| t.dim("length")).filter(|l| *l != 0.0 && !l.is_nan()).unwrap_or(1.0)
}

/// Un projecteur ne s'enfonce pas dans le sol (un tube debout pose sur sa base).
fn lift_fixture(ty: Option<&FixtureType>, p: Vec3) -> Vec3 {
    let min = if is_tube(ty) { r3(tube_length(ty) / 2.0 + 0.02) } else { 0.15 };
    [p[0], crate::util::js_max(p[1], min), p[2]]
}

/// Visée : un tube pivote autour de la verticale (il reste debout), les autres visent le point.
fn fixture_rotation(ty: Option<&FixtureType>, from: Vec3, to: Vec3) -> Vec3 {
    if is_tube(ty) { [0.0, face_yaw(from, to), 0.0] } else { aim_rotation(from, to) }
}

// ------------------------------------------------------------------ objets (décor, grip)
/// Un type d'objet : décor (personnage, véhicule…) ou grip.
#[derive(Clone, Copy)]
enum ObjType {
    Prop(&'static PropType),
    Grip(&'static GripType),
}

impl ObjType {
    fn id(self) -> &'static str {
        match self {
            ObjType::Prop(t) => &t.id,
            ObjType::Grip(t) => &t.id,
        }
    }
    fn name(self) -> &'static str {
        match self {
            ObjType::Prop(t) => &t.name,
            ObjType::Grip(t) => &t.name,
        }
    }
    fn short(self) -> Option<&'static str> {
        match self {
            ObjType::Prop(t) => t.short.as_deref(),
            ObjType::Grip(t) => t.short.as_deref(),
        }
        .filter(|s| !s.is_empty())
    }
    /// `type.short || type.name`
    fn label(self) -> &'static str {
        self.short().unwrap_or(self.name())
    }
    fn model(self) -> Option<&'static str> {
        match self {
            ObjType::Prop(t) => t.model.as_deref(),
            ObjType::Grip(t) => t.extra.get("model").and_then(Value::as_str),
        }
    }
    fn is_prop(self) -> bool {
        matches!(self, ObjType::Prop(t) if t.category.as_deref().is_some_and(|c| !c.is_empty()) || t.extra.get("cat").is_some_and(|c| truthy(Some(c))))
    }
    /// `type.h` (grip)
    fn h(self) -> Option<f64> {
        match self {
            ObjType::Grip(t) => t.h.filter(|h| *h != 0.0),
            ObjType::Prop(_) => None,
        }
    }
    /// `type.size?.[1]` du catalogue
    fn size_y(self) -> Option<f64> {
        let size = match self {
            ObjType::Grip(t) => t.extra.get("size"),
            ObjType::Prop(t) => t.extra.get("baseSize"),
        };
        size.and_then(|s| s.get(1)).and_then(Value::as_f64)
    }
    fn cloths(self) -> Vec<&'static str> {
        match self {
            ObjType::Grip(t) => t.extra.get("cloths").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default(),
            ObjType::Prop(_) => Vec::new(),
        }
    }
    fn short_or_name_text(self) -> String {
        format!("{} {} {}", self.id(), self.name(), self.short().unwrap_or(""))
    }
}

/// `getObjectType` : grip d'abord, puis décor.
fn object_type(id: &str) -> Option<ObjType> {
    let lib = library();
    lib.grip(id).map(ObjType::Grip).or_else(|| lib.prop(id).map(ObjType::Prop))
}

fn object_type_of(o: &SceneObject) -> Option<ObjType> {
    object_type(&o.type_id)
}

fn is_person(t: Option<ObjType>) -> bool {
    t.and_then(ObjType::model) == Some("person")
}

/// Personnages et caméras sont posés au sol (la pose « assis » et la hauteur d'objectif font le reste).
fn grounded(t: Option<ObjType>) -> bool {
    matches!(t.and_then(ObjType::model), Some("person" | "camera" | "dolly"))
}

fn is_camera(t: Option<ObjType>) -> bool {
    matches!(t.and_then(ObjType::model), Some("camera" | "dolly"))
}

/// Réglages valides d'un objet (`cleanObjectProps`) : ceux de son type (décor : types, bornes,
/// listes de choix), ou ceux du grip (pied proposé pour ce cadre).
fn clean_object_props(t: ObjType, props: &Map<String, Value>) -> Map<String, Value> {
    match t {
        ObjType::Grip(g) => {
            let mut out = props.clone();
            if let Some(stand) = out.get("stand").filter(|s| !s.is_null()) {
                let ok = g.extra.get("standChoices").and_then(Value::as_array).is_some_and(|c| c.contains(stand));
                if !ok {
                    out.remove("stand");
                }
            }
            out
        }
        ObjType::Prop(p) => {
            let mut out = Map::new();
            for param in p.extra.get("params").and_then(Value::as_array).into_iter().flatten() {
                let Some(key) = param.get("key").and_then(Value::as_str) else { continue };
                let Some(v) = props.get(key) else { continue };
                match param.get("type").and_then(Value::as_str) {
                    Some("number") => {
                        let n = js_number(Some(v));
                        if n.is_finite() {
                            let (min, max) = (param.get("min").and_then(Value::as_f64).unwrap_or(f64::NEG_INFINITY), param.get("max").and_then(Value::as_f64).unwrap_or(f64::INFINITY));
                            out.insert(key.into(), jnum(clamp(n, min, max)));
                        }
                    }
                    Some("color") => {
                        if let Some(s) = v.as_str().filter(|s| s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|c| c.is_ascii_hexdigit())) {
                            out.insert(key.into(), s.to_lowercase().into());
                        }
                    }
                    Some("select") => {
                        let ok = param.get("options").and_then(Value::as_array).is_some_and(|o| o.iter().any(|opt| opt.get(0) == Some(v)));
                        if ok {
                            out.insert(key.into(), v.clone());
                        }
                    }
                    _ => {}
                }
            }
            out
        }
    }
}

// ------------------------------------------------------------------ définitions (schémas JSON)
/// Définitions des outils (schémas JSON, format neutre `{ name, description, parameters }`).
pub fn tool_defs() -> &'static [Value] {
    static DEFS: OnceLock<Vec<Value>> = OnceLock::new();
    DEFS.get_or_init(build_tool_defs)
}

/// Noms des outils.
pub fn tool_names() -> Vec<&'static str> {
    tool_defs().iter().filter_map(|t| t.get("name").and_then(Value::as_str)).collect()
}

fn build_tool_defs() -> Vec<Value> {
    let point = || json!({ "type": "array", "items": { "type": "number" }, "minItems": 3, "maxItems": 3, "description": "[x, y, z] en mètres" });
    let point_with = |d: &str| {
        let mut p = point();
        p["description"] = d.into();
        p
    };
    let target = json!({
        "type": "object",
        "description": "Projecteurs visés : ids (ou noms, ou numéros de fixture), et/ou un groupe, et/ou un modèle, ou tous",
        "properties": {
            "ids": { "type": "array", "items": { "type": "string" }, "description": "ids donnés par get_scene (les noms et numéros de fixture marchent aussi)" },
            "group": { "type": "string", "description": "nom d’un groupe" },
            "type": { "type": "string", "description": "id de modèle (ex. astera.titan) : tous les projecteurs de ce modèle" },
            "all": { "type": "boolean", "description": "tous les projecteurs du plan" },
        },
    });
    let layout = json!({
        "type": "object",
        "description": "Répartition automatique quand positions n’est pas donné",
        "properties": {
            "kind": { "type": "string", "enum": ["line", "arc", "grid"] },
            "from": point(),
            "to": point(),
            "center": point(),
            "radius": { "type": "number", "description": "rayon de l’arc (m)" },
            "start_angle": { "type": "number", "description": "degrés ; 0 = vers la caméra (+z), négatif = jardin (x<0)" },
            "end_angle": { "type": "number" },
            "cols": { "type": "number" },
            "spacing": { "type": "number", "description": "écart (m) pour la grille" },
        },
    });
    let look = |m: &mut Map<String, Value>| {
        m.insert("intensity".into(), json!({ "type": "number", "description": "intensité en % (0–100)" }));
        m.insert("cct".into(), json!({ "type": "number", "description": "température de couleur en kelvins (blanc)" }));
        m.insert("hue".into(), json!({ "type": "number", "description": "teinte en degrés (0–360) pour une couleur saturée" }));
        m.insert("saturation".into(), json!({ "type": "number", "description": "saturation en % (0–100), avec hue" }));
        m.insert("power".into(), json!({ "type": "boolean", "description": "allumé / éteint" }));
    };
    let with_look = |mut v: Value| {
        if let Some(Value::Object(m)) = v.get_mut("properties") {
            look(m);
        }
        v
    };
    vec![
        json!({
            "name": "get_scene",
            "description": "État du plan en cours : projecteurs (id, nom, modèle, fixture, univers, adresse, canaux, position, réglage), décor et grip, groupes, ambiance, séquence et plan. À appeler avant de modifier des éléments existants.",
            "parameters": { "type": "object", "properties": { "include_objects": { "type": "boolean", "description": "inclure le décor et le grip (par défaut oui)" } } },
        }),
        json!({
            "name": "release_control",
            "description": "Rend la main aux projecteurs : relâche la console ARKLIGHT (qui les pilote canal par canal et passe avant tout) et oublie le dernier DMX conservé quand plus aucune console ni appli n’envoie de signal. À utiliser quand des projecteurs restent noirs ou ne réagissent plus à leurs réglages (champ control de get_scene).",
            "parameters": { "type": "object", "properties": { "target": target } },
        }),
        json!({
            "name": "search_catalog",
            "description": "Cherche dans la bibliothèque : projecteurs (avec leurs modes DMX et nombres de canaux) ou objets (décor, personnages, grip, pieds).",
            "parameters": { "type": "object", "properties": { "query": { "type": "string" }, "kind": { "type": "string", "enum": ["fixture", "object"] } }, "required": ["query"] },
        }),
        json!({
            "name": "add_fixtures",
            "description": "Ajoute des projecteurs d’un modèle de la bibliothèque, placés (positions ou répartition), orientés et adressés à la suite.",
            "parameters": with_look(json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "description": "id du modèle (voir le catalogue), ex. arri.skypanel-s60c" },
                    "count": { "type": "number", "description": "nombre (1 par défaut)" },
                    "name": { "type": "string", "description": "nom de base (numéroté s’il y en a plusieurs)" },
                    "positions": { "type": "array", "items": point() },
                    "layout": layout,
                    "aim": { "description": "\"actor\" (le comédien), le nom d’un objet de la scène, ou un point [x, y, z]", "anyOf": [{ "type": "string" }, point()] },
                    "universe": { "type": "number" },
                    "start_address": { "type": "number", "description": "première adresse DMX (sinon : à la suite du patch)" },
                    "mode": { "type": "string", "description": "id ou nom du mode DMX" },
                    "group": { "type": "string", "description": "crée un groupe avec ces projecteurs" },
                },
                "required": ["type"],
            })),
        }),
        json!({
            "name": "update_fixtures",
            "description": "Règle des projecteurs existants : nom, position, visée, intensité, couleur, allumage, mode DMX, pied.",
            "parameters": with_look(json!({
                "type": "object",
                "properties": {
                    "target": target,
                    "name": { "type": "string" },
                    "position": point_with("nouvelle position [x, y, z] ; pour plusieurs projecteurs : leur centre (ils gardent leurs écarts)"),
                    "move": point_with("déplacement relatif [dx, dy, dz] en mètres"),
                    "aim": { "description": "\"actor\", le nom d’un objet, ou un point [x, y, z]", "anyOf": [{ "type": "string" }, point()] },
                    "mode": { "type": "string", "description": "id ou nom du mode DMX" },
                    "support": { "type": "string", "description": "auto, hanging (accroché), none (au sol), ou id de pied" },
                },
                "required": ["target"],
            })),
        }),
        json!({
            "name": "readdress",
            "description": "Adresse des projecteurs à la suite (sans chevauchement, en passant à l’univers suivant si besoin), et/ou renumérote leurs fixtures.",
            "parameters": {
                "type": "object",
                "properties": {
                    "target": target,
                    "universe": { "type": "number", "description": "univers de départ (1 par défaut)" },
                    "start_address": { "type": "number", "description": "adresse de départ (1 par défaut)" },
                    "order": { "type": "string", "enum": ["scene", "name", "position", "type", "address"], "description": "ordre : scène, nom, de gauche à droite, modèle, adresse actuelle" },
                    "fixture_start": { "type": "number", "description": "renumérote aussi les fixtures à partir de ce numéro" },
                },
                "required": ["target"],
            },
        }),
        json!({
            "name": "add_objects",
            "description": "Ajoute du décor, des personnages, des véhicules, du grip (cadres, drapeaux, réflecteurs) ou des pieds.",
            "parameters": {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "description": "id de l’objet (voir le catalogue)" },
                    "count": { "type": "number" },
                    "name": { "type": "string" },
                    "positions": { "type": "array", "items": point() },
                    "layout": layout,
                    "face": { "description": "\"actor\", le nom d’un objet, ou un point : l’objet se tourne vers lui", "anyOf": [{ "type": "string" }, point()] },
                    "props": { "type": "object", "description": "réglages propres à l’objet (couleur, pose, toile d’un cadre…)" },
                },
                "required": ["type"],
            },
        }),
        json!({
            "name": "update_objects",
            "description": "Déplace, tourne ou renomme des objets existants (décor, personnages, grip).",
            "parameters": {
                "type": "object",
                "properties": {
                    "ids": { "type": "array", "items": { "type": "string" }, "description": "ids ou noms des objets" },
                    "position": point(),
                    "move": point_with("déplacement relatif [dx, dy, dz]"),
                    "face": { "anyOf": [{ "type": "string" }, point()] },
                    "name": { "type": "string" },
                    "props": { "type": "object" },
                },
                "required": ["ids"],
            },
        }),
        json!({
            "name": "remove_items",
            "description": "Supprime des projecteurs ou des objets (ids ou noms). Seulement si l’utilisateur le demande.",
            "parameters": { "type": "object", "properties": { "ids": { "type": "array", "items": { "type": "string" } } }, "required": ["ids"] },
        }),
        json!({
            "name": "create_group",
            "description": "Crée un groupe (sélection nommée) de projecteurs.",
            "parameters": { "type": "object", "properties": { "name": { "type": "string" }, "target": target }, "required": ["name", "target"] },
        }),
        json!({
            "name": "set_environment",
            "description": "Ambiance de la vue : brume, exposition de la caméra, lumière ambiante, sol, cyclo.",
            "parameters": {
                "type": "object",
                "properties": {
                    "haze": { "type": "number", "description": "brume en % (0–100)" },
                    "exposure": { "type": "number", "description": "exposition en IL (−10 à +3)" },
                    "ambient": { "type": "number", "description": "lumière ambiante en % (0–100)" },
                    "floor": { "type": "boolean" },
                    "cyc": { "type": "boolean" },
                },
            },
        }),
        json!({
            "name": "place_from_drawing",
            "description": "Reconstruit un plan de feu DESSINÉ (photo jointe) en un seul appel : liste chaque élément vu sur le dessin avec sa place dans l’image, x et y de 0 à 1 (0,0 = coin en haut à gauche ; le haut du dessin = le fond du plateau, le bas = côté caméra). ARKLIGHT convertit en mètres, choisit le modèle de la bibliothèque, oriente les projecteurs selon leurs flèches.",
            "parameters": {
                "type": "object",
                "properties": {
                    "width_m": { "type": "number", "description": "largeur réelle couverte par l’image, en mètres (d’après les cotes ou l’échelle écrite ; sinon 10)" },
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "kind": { "type": "string", "enum": ["fixture", "grip", "person", "camera", "set", "shape"], "description": "shape : une forme dessinée (maison, bâtiment, estrade, praticable…) : un rectangle (x, y → x2, y2 aux coins opposés) ou un contour (points) ; elle va sur le plan 2D et devient un volume de sa hauteur dans la vue 3D" },
                                "label": { "type": "string", "description": "ce qui est écrit à côté, ou ce qui est dessiné (« SKY S60 face 3200K », « 8x8 diff », « COMÉDIENNE », « canapé »)" },
                                "type": { "type": "string", "description": "id de la bibliothèque si tu le connais" },
                                "x": { "type": "number" },
                                "y": { "type": "number" },
                                "x2": { "type": "number", "description": "fin d’une rangée d’éléments, ou d’un mur" },
                                "y2": { "type": "number" },
                                "count": { "type": "number", "description": "nombre, si l’élément est multiple (« 4 x TITAN »)" },
                                "aim_x": { "type": "number", "description": "pointe de la flèche du projecteur, ou point regardé (caméra, personnage)" },
                                "aim_y": { "type": "number" },
                                "height": { "type": "number", "description": "hauteur en mètres, si elle est écrite (forme : hauteur du volume, une maison ≈ 3)" },
                                "points": { "type": "array", "items": { "type": "array", "items": { "type": "number" } }, "description": "contour d’une forme (kind shape) : [[x, y], …] de 0 à 1, dans l’ordre du tracé" },
                                "cct": { "type": "number" },
                                "hue": { "type": "number" },
                                "saturation": { "type": "number" },
                                "intensity": { "type": "number" },
                                "focal": { "type": "number", "description": "focale de la caméra (mm)" },
                            },
                            "required": ["kind", "label", "x", "y"],
                        },
                    },
                },
                "required": ["items"],
            },
        }),
        json!({
            "name": "manage_plans",
            "description": "Séquences et plans du projet (un plan de feu par plan) : lister, ouvrir, créer, renommer.",
            "parameters": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "switch", "new_plan", "new_sequence", "rename_plan", "rename_sequence"] },
                    "id": { "type": "string", "description": "id ou nom du plan / de la séquence" },
                    "name": { "type": "string" },
                    "mode": { "type": "string", "enum": ["copy", "decor", "empty"], "description": "départ d’un nouveau plan : copie du plan actuel, même décor sans lumière, vide" },
                },
                "required": ["action"],
            },
        }),
    ]
}

// ------------------------------------------------------------------ catalogue (prompt système, recherche)
/// Le catalogue de la bibliothèque, pour le prompt système.
pub fn catalog_text() -> &'static str {
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(|| {
        let lib = library();
        let fx: Vec<String> = lib.fixtures.iter().map(|t| format!("{} — {} {}{}", t.id, t.brand, t.name, if t.dmx.modes.is_empty() { " (sans DMX)" } else { "" })).collect();
        let props: Vec<String> = lib.props.iter().map(|t| format!("{} — {}", t.id, ObjType::Prop(t).label())).collect();
        let grip: Vec<String> = lib.grip.iter().map(|t| format!("{} — {}", t.id, ObjType::Grip(t).label())).collect();
        format!("PROJECTEURS (id — marque modèle) :\n{}\n\nDÉCOR, PERSONNAGES, VÉHICULES (id — nom) : {}\n\nGRIP ET PIEDS (id — nom) : {}", fx.join("\n"), props.join("; "), grip.join("; "))
    })
}

/// Score d'un texte pour une recherche : 3 par mot entier trouvé, 1 par morceau de mot.
fn score(text: &str, words: &[String]) -> i32 {
    let t = norm(text);
    let parts: Vec<&str> = t.split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit())).collect();
    words.iter().map(|w| if t.contains(w.as_str()) { if parts.contains(&w.as_str()) { 3 } else { 1 } } else { 0 }).sum()
}

fn fixture_model(t: &FixtureType) -> &str {
    t.extra.get("model").and_then(Value::as_str).unwrap_or("")
}

/// Libellé d'un mode DMX (`label || name`).
fn mode_label(m: &DmxMode) -> String {
    m.extra.get("label").filter(|l| truthy(Some(l))).map_or_else(|| m.name.clone(), js_string)
}

fn search_catalog(a: &Value) -> Value {
    let query = a.get("query").map(js_string).unwrap_or_default();
    let ws = words(&query);
    let kind = a.get("kind");
    if kind.and_then(Value::as_str) == Some("object") {
        let lib = library();
        let all = lib.props.iter().map(|t| (ObjType::Prop(t), "décor")).chain(lib.grip.iter().map(|t| (ObjType::Grip(t), "grip")));
        let mut found: Vec<(Value, i32)> = all
            .map(|(t, k)| {
                let cat = match t {
                    ObjType::Prop(p) => p.extra.get("cat").and_then(Value::as_str).or(p.category.as_deref()).unwrap_or(""),
                    ObjType::Grip(g) => g.family.as_deref().unwrap_or(""),
                };
                let s = score(&format!("{} {} {} {}", t.id(), t.name(), t.short().unwrap_or(""), cat), &ws);
                (obj([("id", t.id().into()), ("name", t.name().into()), ("kind", k.into())]), s)
            })
            .filter(|(_, s)| *s > 0)
            .collect();
        found.sort_by_key(|x| std::cmp::Reverse(x.1));
        found.truncate(12);
        let n = found.len();
        return obj([("results", Value::Array(found.into_iter().map(|(v, _)| v).collect())), ("summary", format!("{n} objet(s) trouvé(s)").into())]);
    }
    let mut found: Vec<(&FixtureType, i32)> = library().fixtures.iter().map(|t| (t, score(&format!("{} {} {} {} {}", t.id, t.brand, t.name, fixture_model(t), t.category), &ws))).filter(|(_, s)| *s > 0).collect();
    found.sort_by_key(|x| std::cmp::Reverse(x.1));
    found.truncate(8);
    let results: Vec<Value> = found
        .iter()
        .map(|(t, _)| {
            let modes: Vec<Value> = t.dmx.modes.iter().map(|m| obj([("id", m.id.clone().into()), ("name", mode_label(m).into()), ("channels", jnum(m.footprint() as f64))])).collect();
            obj([("id", t.id.clone().into()), ("name", format!("{} {}", t.brand, t.name).into()), ("category", t.category.clone().into()), ("modes", Value::Array(modes))])
        })
        .collect();
    let n = results.len();
    obj([("results", Value::Array(results)), ("summary", format!("{n} projecteur(s) trouvé(s)").into())])
}

/// `String(q || '')`
fn query_text(q: Option<&Value>) -> String {
    q.filter(|v| truthy(Some(v))).map(js_string).unwrap_or_default()
}

/// Modèle de projecteur : id exact, ou meilleure correspondance du nom (« s60 » → arri.skypanel-s60c).
fn find_fixture_type(q: Option<&Value>) -> Option<&'static FixtureType> {
    let q = query_text(q);
    let lib = library();
    if let Some(t) = lib.fixture(q.trim()) {
        return Some(t);
    }
    let ws = words(&q);
    let mut best: Option<(&FixtureType, i32)> = None;
    for t in &lib.fixtures {
        let s = score(&format!("{} {} {} {}", t.id, t.brand, t.name, fixture_model(t)), &ws);
        if best.is_none_or(|(_, b)| s > b) {
            best = Some((t, s));
        }
    }
    best.filter(|(_, s)| *s > 0).map(|(t, _)| t)
}

fn find_object_type(q: Option<&Value>) -> Option<ObjType> {
    let q = query_text(q);
    if let Some(t) = object_type(q.trim()) {
        return Some(t);
    }
    let ws = words(&q);
    let lib = library();
    let mut best: Option<(ObjType, i32)> = None;
    for t in lib.props.iter().map(ObjType::Prop).chain(lib.grip.iter().map(ObjType::Grip)) {
        let s = score(&t.short_or_name_text(), &ws);
        if best.is_none_or(|(_, b)| s > b) {
            best = Some((t, s));
        }
    }
    best.filter(|(_, s)| *s > 0).map(|(t, _)| t)
}

// ------------------------------------------------------------------ plan dessiné
const COLOR_WORDS: [(&str, f64); 9] = [
    ("bleu|blue", 220.0),
    ("cyan|turquoise", 190.0),
    ("vert|green", 120.0),
    ("jaune|yellow", 55.0),
    ("ambre|amber|orange", 30.0),
    ("rouge|red", 0.0),
    ("magenta|fuchsia", 300.0),
    ("rose|pink", 330.0),
    ("violet|purple|mauve", 275.0),
];

fn re(p: &str) -> regex::Regex {
    regex::Regex::new(p).expect("motif")
}

/// Couleur et température écrites sur le dessin (« 3200K », « bleu »).
fn look_from_label(label: Option<&Value>) -> Map<String, Value> {
    let t = norm(&label.filter(|v| !v.is_null()).map(js_string).unwrap_or_default());
    let mut out = Map::new();
    if let Some(c) = re(r"(\d{4,5})\s*k\b").captures(&t) {
        out.insert("cct".into(), jnum(c[1].parse::<f64>().unwrap_or(0.0)));
    }
    for (words, hue) in COLOR_WORDS {
        if re(words).is_match(&t) {
            out.insert("hue".into(), jnum(hue));
            out.insert("saturation".into(), jnum(100.0));
            break;
        }
    }
    if let Some(c) = re(r"(\d{1,3})\s*%").captures(&t) {
        out.insert("intensity".into(), jnum(c[1].parse::<f64>().unwrap_or(0.0)));
    }
    out
}

/// Grip d'après ce qui est écrit : taille de cadre, drapeau, réflecteur… et sa toile.
fn grip_from_label(label: Option<&Value>) -> (Option<ObjType>, Map<String, Value>) {
    let t = norm(&label.filter(|v| !v.is_null()).map(js_string).unwrap_or_default());
    let t = re(r"\s*[x×]\s*").replace_all(&t, "x").into_owned();
    let has = |p: &str| re(p).is_match(&t);
    let id = if let Some(c) = re(r"\b(6x6|8x8|12x12|20x20)\b").captures(&t) {
        format!("frame-{}", &c[1])
    } else if has("floppy") {
        "floppy".into()
    } else if has("drapeau|flag|cutter") {
        "flag-24x36".into()
    } else if has("poly|4x8") {
        "poly-4x8".into()
    } else if has("reflec|reflet|bounce") {
        "reflector-1m".into()
    } else if has("sac|sandbag") {
        "sandbag".into()
    } else if has("cadre|frame|butterfly") {
        "frame-8x8".into()
    } else {
        "frame-6x6".into()
    };
    // « 1/4 », « quart », « 1/2 », « demi » : la toile existe en plusieurs densités
    let quarter = has(r"1/4|quart");
    let half = has(r"1/2|demi|half");
    let cloth = if has("ultra") {
        Some("ultrabounce")
    } else if has("griff|grifl") {
        Some("griffolyn")
    } else if has("magic") {
        Some("magic-cloth")
    } else if has("muslin|mousseline") {
        Some(if has("ecru|unbleach") { "muslin-unbleached" } else { "muslin-bleached" })
    } else if has("lisa marie|damier|checker") {
        Some("checkerboard")
    } else if has(r"soft (gold|or)\b") {
        Some("soft-gold")
    } else if has(r"soft (silver|argent)") {
        Some("soft-silver")
    } else if has("lame") {
        Some(if has(r"gold|\bor\b") { "gold-lame" } else { "silver-lame" })
    } else if has("silent") {
        Some(if quarter { "silent-grid-1-4" } else if half || has("light") { "silent-grid-1-2" } else { "silent-grid" })
    } else if has("grid") {
        Some(if quarter { "grid-1-4" } else if half || has("light") { "grid-1-2" } else { "grid" })
    } else if has("lavand|lavender") {
        Some("net-lavender")
    } else if has("noir|black|solid|duv") {
        Some("solid")
    } else if has(r"\b216\b") {
        Some("lee-216")
    } else if has(r"\b250\b") {
        Some("lee-250")
    } else if has(r"\b251\b") {
        Some("lee-251")
    } else if has(r"\b252\b") {
        Some("lee-252")
    } else if has("china") {
        Some("china-silk")
    } else if has("frost") {
        Some(if half { "soft-frost-1-2" } else { "soft-frost" })
    } else if has("sli?k|soie|diff") {
        Some(if quarter { "silk-1-4" } else if half { "silk-1-2" } else { "silk" })
    } else if has("net|filet|tulle") {
        Some(if has("triple") { "net-triple" } else if has("double") { "net-double" } else { "net-single" })
    } else {
        None
    };
    let ty = object_type(&id);
    let mut props = Map::new();
    if let (Some(c), Some(t)) = (cloth, ty)
        && t.cloths().contains(&c) {
            props.insert("cloth".into(), c.into());
        }
    (ty, props)
}

fn person_from_label(label: Option<&Value>) -> &'static str {
    let t = norm(&label.filter(|v| !v.is_null()).map(js_string).unwrap_or_default());
    if re("enfant|child|kid|bebe").is_match(&t) {
        "person-child"
    } else if re("assis|sitting|seated").is_match(&t) {
        "person-sitting"
    } else if re(r"femme|comedienne|actrice|chanteuse|danseuse|woman|girl|mere|fille|she\b").is_match(&t) {
        "person-woman"
    } else {
        "person-man"
    }
}

// ------------------------------------------------------------------ exécution
/// Ce que l'interface fournit aux outils : l'état du DMX reçu (champ « control » de get_scene,
/// release_control), et les changements de plan (manage_plans).
pub trait ToolHooks {
    /// Univers reçus (dernier état conservé compris) et univers dont le signal arrive encore.
    fn dmx_state(&self) -> DmxState {
        DmxState::default()
    }
    /// Oublie le DMX conservé de ces univers ; rend ceux oubliés.
    fn forget_dmx(&mut self, _universes: &[u32]) -> Vec<u32> {
        Vec::new()
    }
    /// Avant d'ouvrir ou de créer un autre plan.
    fn plan_changing(&mut self) {}
    /// Après.
    fn plan_changed(&mut self) {}
}

/// Univers DMX : reçus (`data`, dernier état conservé compris) et vivants (`live`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DmxState {
    pub data: HashSet<u32>,
    pub live: HashSet<u32>,
}

/// Sans DMX ni plans à prévenir (essais, outils seuls).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoHooks;

impl ToolHooks for NoHooks {}

/// Ce que les outils ont changé (pour rafraîchir l'interface).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Changed {
    pub fixtures: bool,
    pub objects: bool,
    pub groups: bool,
    pub environment: bool,
    pub plans: bool,
}

impl Changed {
    pub fn any(&self) -> bool {
        self.fixtures || self.objects || self.groups || self.environment || self.plans
    }
}

/// Le résultat d'un outil est-il une erreur ?
pub fn is_error(result: &Value) -> bool {
    result.get("error").is_some()
}

/// Résumé d'un résultat pour l'interface (l'erreur, sinon `summary`).
pub fn result_summary(result: &Value) -> String {
    result.get("error").or_else(|| result.get("summary")).map(js_string).unwrap_or_default()
}

fn error(msg: impl Into<String>) -> Value {
    obj([("error", Value::String(msg.into()))])
}

/// `a[k]` (absent : None ; null : Some(Null), comme `undefined` / `null` en JavaScript).
fn arg<'a>(a: &'a Value, k: &str) -> Option<&'a Value> {
    a.get(k)
}

/// `a[k] !== undefined && a[k] !== null`
fn has(a: &Value, k: &str) -> bool {
    a.get(k).is_some_and(|v| !v.is_null())
}

fn state_obj<'a>(f: &'a Fixture, k: &str) -> Option<&'a Value> {
    f.state.get(k).filter(|v| !v.is_null())
}

fn setting<'a>(f: &'a Fixture, k: &str) -> Option<&'a Value> {
    f.settings.extra.get(k).filter(|v| !v.is_null())
}

/// `${f.name}${f.settings.fixtureNumber ? ` (Fix ${n})` : ''}`
fn fixture_label(f: &Fixture) -> String {
    match f.settings.fixture_number.filter(|n| *n != 0) {
        Some(n) => format!("{} (Fix {n})", f.name),
        None => f.name.clone(),
    }
}

/// Projecteur neuf (pas encore ajouté) : ses réglages et son état de départ, pour calculer son
/// réglage de lumière avant l'ajout.
fn new_fixture(ty: &FixtureType, settings: &Map<String, Value>) -> Fixture {
    let mut all = Fixture::default_settings(ty);
    all.extend(settings.clone());
    Fixture {
        id: String::new(),
        type_id: ty.id.clone(),
        name: String::new(),
        position: [0.0; 3],
        rotation: [0.0; 3],
        settings: serde_json::from_value(Value::Object(all)).unwrap_or_default(),
        state: Value::Object(Fixture::default_state(ty)),
        extra: Map::new(),
    }
}

/// Réglage de couleur et d'intensité, selon le projecteur (Astera autonome, tungstène, LED).
fn look_changes(f: &Fixture, a: &Value) -> Changes {
    let ty = library().fixture(&f.type_id);
    let mut changes = Changes::default();
    if has(a, "power") {
        let mut s = Map::new();
        s.insert("power".into(), truthy(arg(a, "power")).into());
        changes.state = Some(s);
    }
    if !["intensity", "cct", "hue", "saturation"].iter().any(|k| has(a, k)) {
        return changes;
    }
    let intensity = if has(a, "intensity") { Some(clamp(num(arg(a, "intensity"), 60.0), 0.0, 100.0)) } else { None };
    let colorful = has(a, "hue") || (has(a, "saturation") && num(arg(a, "saturation"), 0.0) > 0.0);
    if f.type_id.starts_with("astera.") && ty.is_some_and(|t| t.category != "device") {
        // Astera sans DMX : couleur du mode autonome (programme 0, couleur fixe)
        let sa = setting(f, "standalone").and_then(Value::as_object).cloned().unwrap_or_default();
        let c0 = sa.get("colors").and_then(|c| c.get(0)).filter(|c| truthy(Some(c))).cloned().unwrap_or_else(|| json!({ "kind": "hsi", "hue": 0, "sat": 0, "cct": 5600 }));
        let spec = if colorful {
            let c0_hue = c0.get("hue").filter(|h| truthy(Some(h))).map_or(0.0, |h| js_number(Some(h)));
            json!({ "kind": "hsi", "hue": jnum(clamp(num(arg(a, "hue"), c0_hue), 0.0, 360.0)), "sat": jnum(clamp(num(arg(a, "saturation"), 100.0), 1.0, 100.0)), "cct": 5600 })
        } else if has(a, "cct") {
            json!({ "kind": "hsi", "hue": 0, "sat": 0, "cct": jnum(clamp(num(arg(a, "cct"), 5600.0), 1750.0, 20000.0)) })
        } else {
            c0
        };
        let mut standalone = Map::new();
        standalone.insert("speed".into(), 5.into());
        standalone.insert("fade".into(), 50.into());
        standalone.extend(sa.clone());
        standalone.insert("program".into(), 0.into());
        let level = intensity.map(jnum).or_else(|| sa.get("intensity").filter(|v| !v.is_null()).cloned()).unwrap_or_else(|| 100.into());
        standalone.insert("intensity".into(), level);
        standalone.insert("colors".into(), Value::Array(vec![spec.clone(), spec.clone(), spec.clone(), spec]));
        let mut s = Map::new();
        s.insert("standalone".into(), Value::Object(standalone));
        changes.settings = Some(s);
        return changes;
    }
    let m = state_obj(f, "manual").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut manual = m.clone();
    if let Some(i) = intensity {
        manual.insert("intensity".into(), jnum(r3(i / 100.0)));
    }
    if !ty.is_some_and(FixtureType::is_traditional) {
        if colorful {
            let m_hue = m.get("hue").filter(|h| truthy(Some(h))).map_or(0.0, |h| js_number(Some(h)));
            manual.insert("mode".into(), "hsi".into());
            manual.insert("hue".into(), jnum(clamp(num(arg(a, "hue"), m_hue), 0.0, 360.0)));
            manual.insert("sat".into(), jnum(r3(clamp(num(arg(a, "saturation"), 100.0), 0.0, 100.0) / 100.0)));
        } else if has(a, "cct") {
            manual.insert("mode".into(), "cct".into());
            manual.insert("cct".into(), jnum(clamp(num(arg(a, "cct"), 5600.0), 1700.0, 20000.0)));
            manual.insert("tint".into(), 0.into());
        }
    }
    changes.state.get_or_insert_with(Map::new).insert("manual".into(), Value::Object(manual));
    changes
}

fn mode_of<'a>(ty: Option<&'a FixtureType>, q: Option<&Value>) -> Option<&'a DmxMode> {
    let ty = ty?;
    if !truthy(q) || ty.dmx.modes.is_empty() {
        return None;
    }
    let key = norm(&q.map(js_string).unwrap_or_default());
    let modes = &ty.dmx.modes;
    modes.iter().find(|m| norm(&m.id) == key).or_else(|| modes.iter().find(|m| norm(&mode_label(m)) == key)).or_else(|| modes.iter().find(|m| norm(&mode_label(m)).contains(&key)))
}

/// Positions : explicites, ou réparties (ligne, arc, grille) ; par défaut un arc devant le comédien.
fn layout_positions(count: usize, positions: Option<&Value>, layout: Option<&Value>, default_y: f64) -> Vec<Vec3> {
    let mut list: Vec<Vec3> = positions.and_then(Value::as_array).map(|a| a.iter().filter_map(|p| vec3(Some(p))).take(count).collect()).unwrap_or_default();
    if list.len() >= count {
        return list;
    }
    let n = count - list.len();
    let empty = Value::Null;
    let l = layout.unwrap_or(&empty);
    let g = |k: &str| l.get(k);
    let kind = g("kind").and_then(Value::as_str);
    let mut pts = Vec::with_capacity(n);
    if let (Some("line"), Some(a), Some(b)) = (kind, vec3(g("from")), vec3(g("to"))) {
        for i in 0..n {
            let k = if n == 1 { 0.5 } else { i as f64 / (n - 1) as f64 };
            pts.push([r3(a[0] + (b[0] - a[0]) * k), r3(a[1] + (b[1] - a[1]) * k), r3(a[2] + (b[2] - a[2]) * k)]);
        }
    } else if kind == Some("grid") {
        let c = vec3(g("center")).unwrap_or([0.0, default_y, 2.0]);
        let cols = crate::util::js_max(1.0, js_round(num(g("cols"), (n as f64).sqrt().ceil())));
        let rows = (n as f64 / cols).ceil();
        let sp = num(g("spacing"), 1.0);
        for i in 0..n {
            let col = (i as f64) % cols;
            let row = (i as f64 / cols).floor();
            pts.push([r3(c[0] + (col - (cols - 1.0) / 2.0) * sp), c[1], r3(c[2] + (row - (rows - 1.0) / 2.0) * sp)]);
        }
    } else {
        // arc autour du centre (le comédien) ; 0° = vers la caméra
        let c = vec3(g("center")).unwrap_or([0.0, default_y, 0.0]);
        let r = num(g("radius"), 3.5);
        let a0 = num(g("start_angle"), if n == 1 { -35.0 } else { -60.0 });
        let a1 = num(g("end_angle"), if n == 1 { -35.0 } else { 60.0 });
        for i in 0..n {
            let deg = if n == 1 { a0 } else { a0 + ((a1 - a0) * i as f64) / (n - 1) as f64 };
            let a = (deg * std::f64::consts::PI) / 180.0;
            pts.push([r3(c[0] + r * a.sin()), c[1], r3(c[2] + r * a.cos())]);
        }
    }
    list.extend(pts);
    list
}

/// Point visé par le comédien : le premier personnage de la scène (à 1,55 m), sinon ACTOR_AIM.
fn actor_point(person: Option<Vec3>) -> Vec3 {
    person.map_or(ACTOR_AIM, |p| [p[0], 1.55, p[2]])
}

const ACTOR_WORDS: [&str; 5] = ["actor", "acteur", "comedien", "comedienne", "personnage"];

/// Point visé : le comédien, un objet de la scène (par son nom), ou des coordonnées.
fn resolve_point(doc: &Document, v: Option<&Value>) -> Option<Vec3> {
    if let Some(p) = vec3(v) {
        return Some(p);
    }
    let v = v.filter(|v| !v.is_null() && v.as_str() != Some(""))?;
    let key = norm(&js_string(v));
    let objects = &doc.project.objects;
    if ACTOR_WORDS.contains(&key.as_str()) {
        let person = objects.iter().find(|o| is_person(object_type_of(o)));
        return Some(actor_point(person.map(|o| o.position)));
    }
    let o = objects.iter().find(|x| norm(&x.name) == key).or_else(|| objects.iter().find(|x| norm(&x.name).contains(&key)));
    if let Some(o) = o {
        let person = is_person(object_type_of(o));
        return Some([o.position[0], if person { 1.55 } else { o.position[1] + 0.5 }, o.position[2]]);
    }
    doc.project.fixtures.iter().find(|x| norm(&x.name).contains(&key)).map(|f| f.position)
}

/// Projecteurs visés (ids, noms, numéros de fixture, groupe, modèle, tous) ; `skip` : ids déjà
/// retirés (suppressions en série).
fn resolve_target(doc: &Document, target: Option<&Value>, skip: &HashSet<String>) -> Vec<String> {
    let all: Vec<&Fixture> = doc.project.fixtures.iter().filter(|f| !skip.contains(&f.id)).collect();
    let mut out: Vec<String> = Vec::new();
    let mut add = |id: &str| {
        if !out.iter().any(|x| x == id) {
            out.push(id.to_owned());
        }
    };
    let (ids, group, ty, every): (Vec<Value>, Option<&Value>, Option<&Value>, bool) = match target {
        Some(Value::String(s)) => (vec![Value::String(s.clone())], None, None, false),
        Some(Value::Array(a)) => (a.clone(), None, None, false),
        Some(t @ Value::Object(_)) => {
            let ids = match t.get("ids") {
                Some(Value::Array(a)) => a.clone(),
                Some(v) if truthy(Some(v)) => vec![v.clone()],
                _ => Vec::new(),
            };
            (ids, t.get("group"), t.get("type"), truthy(t.get("all")))
        }
        _ => (Vec::new(), None, None, false),
    };
    if every {
        for f in &all {
            add(&f.id);
        }
    }
    for raw in &ids {
        let key = js_string(raw);
        let nk = norm(&key);
        // `key.replace(/^fix\s*/i, '')`
        let fix = match key.get(..3) {
            Some(p) if p.eq_ignore_ascii_case("fix") => key[3..].trim_start().to_owned(),
            _ => key.clone(),
        };
        let f = all
            .iter()
            .find(|x| x.id == key)
            .or_else(|| all.iter().find(|x| norm(&x.name) == nk))
            .or_else(|| all.iter().find(|x| x.settings.fixture_number.is_some_and(|n| n.to_string() == fix)))
            .or_else(|| all.iter().find(|x| norm(&x.name).contains(&nk)));
        if let Some(f) = f {
            add(&f.id);
        }
    }
    if truthy(group) {
        let gk = norm(&group.map(js_string).unwrap_or_default());
        let groups = &doc.project.groups;
        let g = groups.iter().find(|x| norm(&x.name) == gk).or_else(|| groups.iter().find(|x| norm(&x.name).contains(&gk)));
        for id in g.map(|g| g.ids.as_slice()).unwrap_or_default() {
            if all.iter().any(|f| f.id == *id) {
                add(id);
            }
        }
    }
    if truthy(ty)
        && let Some(t) = find_fixture_type(ty) {
            for f in &all {
                if f.type_id == t.id {
                    add(&f.id);
                }
            }
        }
    out
}

/// Ce que l'assistant a fait pendant une demande, et ce qu'il fait encore.
#[derive(Debug, Clone, Default)]
pub struct ToolRunner {
    /// largeur / hauteur de la photo jointe (plan dessiné)
    pub image_aspect: Option<f64>,
    /// ce qui a changé depuis le dernier [`ToolRunner::take_changed`]
    pub changed: Changed,
    /// projecteurs ajoutés (à sélectionner à la fin)
    pub last_added: Vec<String>,
}

impl ToolRunner {
    pub fn new(image_aspect: Option<f64>) -> ToolRunner {
        ToolRunner { image_aspect, ..ToolRunner::default() }
    }

    /// Ce qui a changé depuis le dernier appel (pour rafraîchir l'interface).
    pub fn take_changed(&mut self) -> Changed {
        std::mem::take(&mut self.changed)
    }

    /// Exécute un outil sur le projet ; les erreurs deviennent un résultat que le modèle peut lire.
    pub fn run(&mut self, doc: &mut Document, hooks: &mut dyn ToolHooks, name: &str, args: &Value) -> Value {
        let empty = Value::Object(Map::new());
        let a = if args.is_object() { args } else { &empty };
        match name {
            "get_scene" => {
                let mut s = scene_summary(doc, &*hooks, a.get("include_objects") != Some(&Value::Bool(false)));
                let n = doc.project.fixtures.len();
                s["summary"] = format!("{n} projecteur(s), {} objet(s)", doc.project.objects.len()).into();
                s
            }
            "search_catalog" => search_catalog(a),
            "release_control" => self.release_control(doc, hooks, a),
            "add_fixtures" => self.add_fixtures(doc, a),
            "update_fixtures" => self.update_fixtures(doc, a),
            "readdress" => self.readdress(doc, a),
            "add_objects" => self.add_objects(doc, a),
            "update_objects" => self.update_objects(doc, a),
            "remove_items" => self.remove_items(doc, a),
            "create_group" => self.create_group(doc, a),
            "set_environment" => self.set_environment(doc, a),
            "place_from_drawing" => self.place_from_drawing(doc, a),
            "manage_plans" => self.manage_plans(doc, hooks, a),
            _ => error(format!("Outil inconnu : {name}")),
        }
    }

    fn release_control(&mut self, doc: &mut Document, hooks: &mut dyn ToolHooks, a: &Value) -> Value {
        let list: Vec<String> = if truthy(a.get("target")) { resolve_target(doc, a.get("target"), &HashSet::new()) } else { doc.project.fixtures.iter().map(|f| f.id.clone()).collect() };
        let held: Vec<(String, String)> = list.iter().filter_map(|id| doc.fixture(id)).filter(|f| console_held(f)).map(|f| (f.id.clone(), f.name.clone())).collect();
        if !held.is_empty() {
            let items = held
                .iter()
                .map(|(id, _)| {
                    let mut s = Map::new();
                    s.insert("console".into(), Value::Null);
                    (id.clone(), Changes { state: Some(s), ..Changes::default() })
                })
                .collect();
            doc.apply(Edit::UpdateFixtures(items));
            self.changed.fixtures = true;
        }
        let mut universes: Vec<u32> = Vec::new();
        for id in &list {
            if let Some(u) = doc.fixture(id).and_then(|f| f.settings.universe)
                && !universes.contains(&u) {
                    universes.push(u);
                }
        }
        let forgotten = hooks.forget_dmx(&universes);
        let mut parts = Vec::new();
        if !held.is_empty() {
            parts.push(format!("{} projecteur(s) relâché(s) de la console", held.len()));
        }
        if !forgotten.is_empty() {
            parts.push(format!("DMX conservé oublié (univers {})", forgotten.iter().map(u32::to_string).collect::<Vec<_>>().join(", ")));
        }
        let summary = if parts.is_empty() { "rien à relâcher : les projecteurs suivent déjà leurs réglages (ou un DMX encore envoyé)".to_owned() } else { parts.join(" ; ") };
        obj([("released", json!(held.iter().map(|(_, n)| n).collect::<Vec<_>>())), ("dmx_forgotten", json!(forgotten)), ("summary", summary.into())])
    }

    fn add_fixtures(&mut self, doc: &mut Document, a: &Value) -> Value {
        let Some(ty) = find_fixture_type(a.get("type")) else {
            return error(format!("Modèle inconnu : {}. Cherche avec search_catalog.", fmt_value(a.get("type"))));
        };
        // nombre oublié par le modèle : celui des positions données
        let given = a.get("positions").and_then(Value::as_array).map_or(0, |p| p.iter().filter(|x| vec3(Some(x)).is_some()).count());
        let count = clamp(js_round(num(a.get("count"), if given > 1 { given as f64 } else { 1.0 })), 1.0, 64.0) as usize;
        let tube = is_tube(Some(ty));
        let default_y = if tube { r3(tube_length(Some(ty)) / 2.0 + 0.05) } else { 2.4 };
        let pts: Vec<Vec3> = layout_positions(count, a.get("positions"), a.get("layout"), default_y).into_iter().map(|p| lift_fixture(Some(ty), p)).collect();
        let target = match a.get("aim") {
            None => (!tube).then_some(ACTOR_AIM),
            Some(v) => resolve_point(doc, Some(v)),
        };
        let mode = mode_of(Some(ty), a.get("mode"));
        let universe = crate::util::js_max(1.0, js_round(num(a.get("universe"), 1.0)));
        let mut address = a.get("start_address").map(|v| clamp(js_round(num(Some(v), 1.0)), 1.0, 512.0));
        let base_name = a.get("name").filter(|n| truthy(Some(n))).map(js_string);
        let mut adds = Vec::new();
        let mut looks = Vec::new();
        for (i, pt) in pts.iter().enumerate().take(count) {
            let name = base_name.as_ref().map(|n| if count > 1 { format!("{n} {}", i + 1) } else { n.clone() });
            let mut settings = Map::new();
            settings.insert("universe".into(), jnum(universe));
            if let Some(m) = mode {
                settings.insert("dmxMode".into(), m.id.clone().into());
            }
            if let Some(ad) = address {
                settings.insert("dmxAddress".into(), jnum(ad));
            }
            let virt = new_fixture(ty, &settings);
            if let Some(ad) = address.as_mut() {
                *ad += ty.mode(virt.settings.dmx_mode.as_deref()).map_or(0, DmxMode::footprint) as f64;
            }
            // le réglage d'un Astera (mode autonome) part avec l'ajout ; l'état (lumière manuelle) après
            let look = look_changes(&virt, a);
            if let Some(s) = &look.settings {
                settings.extend(s.clone());
            }
            looks.push(look.state);
            let rotation = target.map_or([0.0; 3], |t| fixture_rotation(Some(ty), *pt, t));
            adds.push(Edit::AddFixture { type_id: ty.id.clone(), position: *pt, rotation, name, settings });
        }
        let before = doc.project.fixtures.len();
        doc.apply(Edit::Batch(adds));
        let ids: Vec<String> = doc.project.fixtures[before..].iter().map(|f| f.id.clone()).collect();
        let mut second = Vec::new();
        let updates: Vec<(String, Changes)> = ids.iter().zip(looks).filter_map(|(id, s)| s.map(|s| (id.clone(), Changes { state: Some(s), ..Changes::default() }))).collect();
        if !updates.is_empty() {
            second.push(Edit::UpdateFixtures(updates));
        }
        self.changed.fixtures = true;
        if truthy(a.get("group")) {
            second.push(Edit::CreateGroup { name: a.get("group").map(js_string), ids: ids.clone() });
            self.changed.groups = true;
        }
        if !second.is_empty() {
            doc.apply(if second.len() == 1 { second.remove(0) } else { Edit::Batch(second) });
        }
        self.last_added = ids.clone();
        let added: Vec<Value> = ids
            .iter()
            .filter_map(|id| doc.fixture(id))
            .map(|f| {
                obj([
                    ("id", f.id.clone().into()),
                    ("name", f.name.clone().into()),
                    ("fix", f.settings.fixture_number.map_or(Value::Null, Value::from)),
                    ("universe", f.settings.universe.map_or(Value::Null, Value::from)),
                    ("address", f.settings.dmx_address.map_or(Value::Null, Value::from)),
                    ("pos", jvec(f.position)),
                ])
            })
            .collect();
        let n = added.len();
        obj([("added", Value::Array(added)), ("summary", format!("{n} × {} {} ajouté(s)", ty.brand, ty.name).into())])
    }

    fn update_fixtures(&mut self, doc: &mut Document, a: &Value) -> Value {
        let list = resolve_target(doc, a.get("target"), &HashSet::new());
        if list.is_empty() {
            return error("Aucun projecteur trouvé : appelle get_scene pour connaître les ids.");
        }
        let target = a.get("aim").and_then(|v| resolve_point(doc, Some(v)));
        if let (Some(aim), None) = (a.get("aim"), target) {
            return error(format!("Cible inconnue : {}", serde_json::to_string(aim).unwrap_or_default()));
        }
        let fixtures: Vec<Fixture> = list.iter().filter_map(|id| doc.fixture(id).cloned()).collect();
        // plusieurs projecteurs et une seule position : c'est le centre du groupe (ils gardent leurs écarts)
        let position = vec3(a.get("position"));
        let group_move = position.filter(|_| fixtures.len() > 1).map(|p| {
            let n = fixtures.len() as f64;
            let mean = |i: usize| fixtures.iter().map(|f| f.position[i]).sum::<f64>() / n;
            [p[0] - mean(0), p[1] - mean(1), p[2] - mean(2)]
        });
        let movement = vec3(a.get("move"));
        let name = a.get("name").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
        let mut items = Vec::new();
        for (idx, f) in fixtures.iter().enumerate() {
            let ty = library().fixture(&f.type_id);
            let mut changes = look_changes(f, a);
            let mut pos = f.position;
            let mut moved = false;
            if let Some(g) = group_move {
                pos = [r3(pos[0] + g[0]), r3(pos[1] + g[1]), r3(pos[2] + g[2])];
                moved = true;
            } else if let Some(p) = position {
                pos = p;
                moved = true;
            }
            if let Some(m) = movement {
                pos = [r3(pos[0] + m[0]), r3(pos[1] + m[1]), r3(pos[2] + m[2])];
                moved = true;
            }
            if moved {
                pos = lift_fixture(ty, pos);
                changes.position = Some(pos);
            }
            if let Some(t) = target {
                changes.rotation = Some(fixture_rotation(ty, pos, t));
            }
            if let Some(n) = name {
                changes.name = Some(if fixtures.len() > 1 { format!("{n} {}", idx + 1) } else { n.to_owned() });
            }
            let mode = mode_of(ty, a.get("mode"));
            let support = a.get("support").filter(|s| truthy(Some(s)));
            if mode.is_some() || support.is_some() {
                let s = changes.settings.get_or_insert_with(Map::new);
                if let Some(m) = mode {
                    s.insert("dmxMode".into(), m.id.clone().into());
                }
                if let Some(sp) = support {
                    s.insert("support".into(), js_string(sp).into());
                }
            }
            items.push((f.id.clone(), changes));
        }
        doc.apply(Edit::UpdateFixtures(items));
        self.changed.fixtures = true;
        let updated: Vec<Value> = list.iter().filter_map(|id| doc.fixture(id)).map(|f| fixture_label(f).into()).collect();
        let n = updated.len();
        obj([("updated", Value::Array(updated)), ("summary", format!("{n} projecteur(s) réglé(s)").into())])
    }

    fn readdress(&mut self, doc: &mut Document, a: &Value) -> Value {
        let list = resolve_target(doc, a.get("target"), &HashSet::new());
        if list.is_empty() {
            return error("Aucun projecteur trouvé.");
        }
        let order = a.get("order").filter(|o| truthy(Some(o))).map_or(Order::Scene, |o| Order::from_id(&js_string(o)));
        // `Math.max(1, Math.round(Number(u) || 1))`, `Math.min(512, Math.max(1, Math.round(Number(a) || 1)))`
        let or_one = |v: f64| if v == 0.0 || v.is_nan() { 1.0 } else { v };
        let universe = js_round(or_one(num(a.get("universe"), 1.0))).clamp(1.0, f64::from(u32::MAX)) as u32;
        let address = js_round(or_one(num(a.get("start_address"), 1.0))).clamp(1.0, f64::from(UNIVERSE_SIZE)) as u32;
        let plan = plan_addressing(&doc.project.fixtures, &AddressingOptions { scope: Scope::Ids(list.clone()), order, universe, address, pack: true, align: 0, overflow: true });
        // patch après les déplacements (univers, adresse), puis numéros de fixture
        let mut patch: Vec<(String, u32, u32)> = list.iter().filter_map(|id| doc.fixture(id)).map(|f| (f.id.clone(), f.settings.universe.unwrap_or(1), f.settings.dmx_address.unwrap_or(1))).collect();
        let mut changes: Vec<(String, Map<String, Value>)> = Vec::new();
        let mut set = |id: &str, k: &str, v: Value| match changes.iter_mut().find(|(x, _)| x == id) {
            Some((_, m)) => {
                m.insert(k.into(), v);
            }
            None => {
                let mut m = Map::new();
                m.insert(k.into(), v);
                changes.push((id.to_owned(), m));
            }
        };
        for m in &plan.moves {
            set(&m.id, "universe", m.to.universe.into());
            set(&m.id, "dmxAddress", m.to.address.into());
            if let Some(p) = patch.iter_mut().find(|p| p.0 == m.id) {
                p.1 = m.to.universe;
                p.2 = m.to.address;
            }
        }
        let mut numbers: Vec<(String, u32)> = Vec::new();
        if let Some(start) = a.get("fixture_start") {
            let first = crate::util::js_max(1.0, js_round(num(Some(start), 1.0))) as u32;
            let order_of = |id: &str| plan.moves.iter().position(|m| m.id == id).unwrap_or(0);
            let mut sorted: Vec<&(String, u32, u32)> = patch.iter().filter(|p| doc.fixture(&p.0).and_then(|f| library().fixture(&f.type_id)).is_some_and(|t| !t.dmx.modes.is_empty())).collect();
            sorted.sort_by(|x, y| x.1.cmp(&y.1).then(x.2.cmp(&y.2)).then(order_of(&x.0).cmp(&order_of(&y.0))));
            numbers.extend(sorted.into_iter().zip(first..).map(|(p, n)| (p.0.clone(), n)));
        }
        for (id, n) in &numbers {
            set(id, "fixtureNumber", (*n).into());
        }
        if !changes.is_empty() {
            doc.apply(Edit::UpdateFixtures(changes.into_iter().map(|(id, s)| (id, Changes { settings: Some(s), ..Changes::default() })).collect()));
        }
        self.changed.fixtures = true;
        let lines: Vec<Value> = list
            .iter()
            .filter_map(|id| doc.fixture(id))
            .filter(|f| footprint_of(f) > 0)
            .map(|f| format!("{} : U{}.{:03}", fixture_label(f), f.settings.universe.unwrap_or(1), f.settings.dmx_address.unwrap_or(1)).into())
            .collect();
        let numbered = numbers.len();
        let summary = format!("{} projecteur(s) ré-adressé(s){}", plan.moves.len(), if numbered > 0 { format!(", {numbered} renuméroté(s)") } else { String::new() });
        obj([("patch", Value::Array(lines)), ("unplaced", plan.unplaced.len().into()), ("conflicts_after", plan.conflicts_after.into()), ("summary", summary.into())])
    }

    fn add_objects(&mut self, doc: &mut Document, a: &Value) -> Value {
        let Some(ty) = find_object_type(a.get("type")) else {
            return error(format!("Objet inconnu : {}. Cherche avec search_catalog (kind: object).", fmt_value(a.get("type"))));
        };
        // nombre oublié par le modèle : celui des positions données
        let given = a.get("positions").and_then(Value::as_array).map_or(0, |p| p.iter().filter(|x| vec3(Some(x)).is_some()).count());
        let default_count = if a.get("count").is_none() && given > 1 { given as f64 } else { 1.0 };
        let count = clamp(js_round(num(a.get("count"), default_count)), 1.0, 64.0) as usize;
        let prop = ty.is_prop();
        let min_y = if prop { 0.0 } else { r3(ty.h().or(ty.size_y()).unwrap_or(0.0) / 2.0) };
        let y = if prop { 0.0 } else { 1.6 };
        let default_layout = json!({ "kind": "line", "from": [-1.5, y, -1], "to": [1.5, y, -1] });
        let layout = a.get("layout").filter(|l| truthy(Some(l))).unwrap_or(&default_layout);
        let pts: Vec<Vec3> = layout_positions(count, a.get("positions"), Some(layout), y).into_iter().map(|p| [p[0], crate::util::js_max(min_y, p[1]), p[2]]).collect();
        // une caméra regarde le comédien si rien d'autre n'est demandé
        let face = match a.get("face") {
            Some(v) => resolve_point(doc, Some(v)),
            None if is_camera(Some(ty)) => resolve_point(doc, Some(&Value::from("actor"))),
            None => None,
        };
        let props = a.get("props").and_then(Value::as_object).map(|p| clean_object_props(ty, p)).unwrap_or_default();
        let base_name = a.get("name").filter(|n| truthy(Some(n))).map(js_string);
        let mut adds = Vec::new();
        for (i, pt) in pts.iter().enumerate().take(count) {
            let pos = if grounded(Some(ty)) { [pt[0], 0.0, pt[2]] } else { *pt };
            let name = base_name.as_ref().map(|n| if count > 1 { format!("{n} {}", i + 1) } else { n.clone() });
            let yaw = face.map_or(0.0, |f| face_yaw(pos, f));
            adds.push(Edit::AddObject { type_id: ty.id().to_owned(), position: pos, rotation: [0.0, yaw, 0.0], name, props: props.clone() });
        }
        let before = doc.project.objects.len();
        doc.apply(Edit::Batch(adds));
        self.changed.objects = true;
        let added: Vec<Value> = doc.project.objects[before..].iter().map(|o| obj([("id", o.id.clone().into()), ("name", o.name.clone().into()), ("pos", jvec(o.position))])).collect();
        let n = added.len();
        obj([("added", Value::Array(added)), ("summary", format!("{n} × {} ajouté(s)", ty.label()).into())])
    }

    fn update_objects(&mut self, doc: &mut Document, a: &Value) -> Value {
        let keys: Vec<String> = match a.get("ids") {
            Some(Value::Array(l)) => l.iter().map(js_string).collect(),
            Some(v) if truthy(Some(v)) => vec![js_string(v)],
            _ => Vec::new(),
        };
        let objects = &doc.project.objects;
        let objs: Vec<SceneObject> = keys
            .iter()
            .filter_map(|k| {
                let nk = norm(k);
                objects.iter().find(|o| o.id == *k).or_else(|| objects.iter().find(|o| norm(&o.name) == nk)).or_else(|| objects.iter().find(|o| norm(&o.name).contains(&nk))).cloned()
            })
            .collect();
        if objs.is_empty() {
            return error("Aucun objet trouvé : appelle get_scene.");
        }
        let face = a.get("face").and_then(|v| resolve_point(doc, Some(v)));
        let position = vec3(a.get("position"));
        let group_move = position.filter(|_| objs.len() > 1).map(|p| {
            let n = objs.len() as f64;
            let mean = |i: usize| objs.iter().map(|o| o.position[i]).sum::<f64>() / n;
            [p[0] - mean(0), p[1] - mean(1), p[2] - mean(2)]
        });
        let movement = vec3(a.get("move"));
        let mut edits = Vec::new();
        for o in &objs {
            let ty = object_type_of(o);
            let mut pos = o.position;
            if let Some(g) = group_move {
                pos = [r3(pos[0] + g[0]), r3(pos[1] + g[1]), r3(pos[2] + g[2])];
            } else if let Some(p) = position {
                pos = p;
            }
            if let Some(m) = movement {
                pos = [r3(pos[0] + m[0]), r3(pos[1] + m[1]), r3(pos[2] + m[2])];
            }
            if grounded(ty) {
                pos = [pos[0], 0.0, pos[2]];
            }
            let props = match (a.get("props").filter(|p| truthy(Some(p))), ty) {
                (Some(Value::Object(p)), Some(t)) => Some(clean_object_props(t, p)),
                (Some(_), _) => Some(Map::new()),
                _ => None,
            };
            let changes = Changes { position: Some(pos), rotation: face.map(|f| [0.0, face_yaw(pos, f), 0.0]), name: a.get("name").and_then(Value::as_str).map(str::to_owned), props, ..Changes::default() };
            edits.push(Edit::UpdateObject { id: o.id.clone(), changes });
        }
        doc.apply(Edit::Batch(edits));
        self.changed.objects = true;
        let updated: Vec<Value> = objs.iter().map(|o| doc.object(&o.id).map_or_else(|| o.name.clone(), |x| x.name.clone()).into()).collect();
        let n = updated.len();
        obj([("updated", Value::Array(updated)), ("summary", format!("{n} objet(s) modifié(s)").into())])
    }

    fn remove_items(&mut self, doc: &mut Document, a: &Value) -> Value {
        let keys: Vec<String> = match a.get("ids") {
            Some(Value::Array(l)) => l.iter().map(js_string).collect(),
            Some(v) if truthy(Some(v)) => vec![js_string(v)],
            _ => Vec::new(),
        };
        let mut gone_fixtures: HashSet<String> = HashSet::new();
        let mut gone_objects: HashSet<String> = HashSet::new();
        let mut fixture_ids = Vec::new();
        let mut edits = Vec::new();
        let mut removed: Vec<Value> = Vec::new();
        for k in &keys {
            if let Some(id) = resolve_target(doc, Some(&json!({ "ids": [k] })), &gone_fixtures).into_iter().next() {
                if let Some(f) = doc.fixture(&id) {
                    removed.push(f.name.clone().into());
                }
                gone_fixtures.insert(id.clone());
                fixture_ids.push(id);
                self.changed.fixtures = true;
                self.changed.groups = true;
                continue;
            }
            let nk = norm(k);
            let live = || doc.project.objects.iter().filter(|o| !gone_objects.contains(&o.id));
            let o = live().find(|o| o.id == *k).or_else(|| live().find(|o| norm(&o.name) == nk)).map(|o| (o.id.clone(), o.name.clone()));
            if let Some((id, name)) = o {
                removed.push(name.into());
                edits.push(Edit::RemoveObject(id.clone()));
                gone_objects.insert(id);
                self.changed.objects = true;
            }
        }
        if !fixture_ids.is_empty() {
            edits.insert(0, Edit::RemoveFixtures(fixture_ids));
        }
        if !edits.is_empty() {
            doc.apply(if edits.len() == 1 { edits.remove(0) } else { Edit::Batch(edits) });
        }
        let n = removed.len();
        obj([("removed", Value::Array(removed)), ("summary", format!("{n} élément(s) supprimé(s)").into())])
    }

    fn create_group(&mut self, doc: &mut Document, a: &Value) -> Value {
        let list = resolve_target(doc, a.get("target"), &HashSet::new());
        if list.is_empty() {
            return error("Aucun projecteur trouvé.");
        }
        let name = a.get("name").filter(|v| !v.is_null()).map(js_string);
        let before = doc.project.groups.len();
        doc.apply(Edit::CreateGroup { name, ids: list });
        self.changed.groups = true;
        let Some(g) = doc.project.groups.get(before) else { return error("Groupe non créé.") };
        let (name, count) = (g.name.clone(), g.ids.len());
        obj([("group", name.clone().into()), ("count", count.into()), ("summary", format!("Groupe « {name} » ({count})").into())])
    }

    fn set_environment(&mut self, doc: &mut Document, a: &Value) -> Value {
        let mut c = Map::new();
        if let Some(v) = a.get("haze") {
            c.insert("haze".into(), jnum(r3(clamp(num(Some(v), 25.0), 0.0, 100.0) / 100.0)));
        }
        if let Some(v) = a.get("exposure") {
            c.insert("exposure".into(), jnum(clamp(js_round(num(Some(v), 0.0) * 2.0) / 2.0, -10.0, 3.0)));
        }
        if let Some(v) = a.get("ambient") {
            c.insert("ambient".into(), jnum(r3(clamp(num(Some(v), 4.0), 0.0, 50.0) / 100.0)));
        }
        for k in ["floor", "cyc"] {
            if let Some(b) = a.get(k).and_then(Value::as_bool) {
                c.insert(k.into(), b.into());
            }
        }
        doc.apply(Edit::Environment(c.clone()));
        self.changed.environment = true;
        obj([("environment", Value::Object(c)), ("summary", "Ambiance réglée".into())])
    }

    fn place_from_drawing(&mut self, doc: &mut Document, a: &Value) -> Value {
        // identifiant neuf d'un élément du plan (comme `newId('i')`)
        fn shape_id() -> String {
            use std::sync::atomic::{AtomicU64, Ordering};
            static N: AtomicU64 = AtomicU64::new(0);
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
            format!("i{:x}{:x}", t % 0xffff_ffff, N.fetch_add(1, Ordering::Relaxed))
        }
        let items: Vec<&Value> = a.get("items").and_then(Value::as_array).map(|l| l.iter().filter(|it| finite(it.get("x")) && finite(it.get("y"))).collect()).unwrap_or_default();
        if items.is_empty() {
            return error("Aucun élément : donne items avec x et y entre 0 et 1.");
        }
        let w = clamp(num(a.get("width_m"), 10.0), 2.0, 80.0);
        let aspect = self.image_aspect.filter(|x| *x != 0.0 && x.is_finite()).unwrap_or(4.0 / 3.0);
        let h = w / aspect;
        // origine : le premier personnage dessiné devient le centre du plateau (sinon le centre de l'image)
        let kind = |it: &Value| it.get("kind").and_then(Value::as_str).map(str::to_owned);
        let who = items.iter().find(|it| kind(it).as_deref() == Some("person"));
        let (ox, oy) = who.map_or((0.5, 0.5), |w| (num(w.get("x"), 0.5), num(w.get("y"), 0.5)));
        let at = |x: Option<&Value>, y: Option<&Value>, hh: f64| -> Vec3 { [r3((clamp(num(x, 0.5), -0.25, 1.25) - ox) * w), hh, r3((clamp(num(y, 0.5), -0.25, 1.25) - oy) * h)] };
        // positions d'un élément multiple : le long de x,y → x2,y2, sinon une rangée de 1 m d'écart
        let spots = |it: &Value, hh: f64| -> Vec<Vec3> {
            let n = clamp(js_round(num(it.get("count"), 1.0)), 1.0, 32.0) as usize;
            let a0 = at(it.get("x"), it.get("y"), hh);
            if n == 1 {
                return vec![a0];
            }
            if finite(it.get("x2")) && finite(it.get("y2")) {
                let b = at(it.get("x2"), it.get("y2"), hh);
                return (0..n).map(|i| [0, 1, 2].map(|j| r3(a0[j] + ((b[j] - a0[j]) * i as f64) / (n - 1) as f64))).collect();
            }
            (0..n).map(|i| [r3(a0[0] + i as f64 - (n - 1) as f64 / 2.0), hh, a0[2]]).collect()
        };
        let aim_of = |it: &Value| (finite(it.get("aim_x")) && finite(it.get("aim_y"))).then(|| at(it.get("aim_x"), it.get("aim_y"), 1.55));
        let label = |it: &Value| -> String {
            let raw = it.get("label").filter(|l| truthy(Some(l))).map(js_string).unwrap_or_default();
            take_chars(raw.split_whitespace().collect::<Vec<_>>().join(" ").trim(), 40)
        };
        enum Made {
            Fixture(usize, &'static FixtureType, Vec3),
            Object(usize, ObjType),
        }
        let mut made: Vec<Made> = Vec::new();
        let mut made_shapes: Vec<String> = Vec::new();
        let mut guessed: Vec<Value> = Vec::new();
        let mut edits: Vec<Edit> = Vec::new();
        let mut removed_actor: Option<String> = None;

        // le comédien par défaut d'un projet neuf laisse la place aux personnages dessinés
        if who.is_some() {
            let persons: Vec<&SceneObject> = doc.project.objects.iter().filter(|o| is_person(object_type_of(o))).collect();
            if let [d] = persons.as_slice()
                && d.type_id == "person-man" && d.position.iter().all(|v| *v == 0.0) && matches!(norm(&d.name).as_str(), "comedien" | "actor") {
                    edits.push(Edit::RemoveObject(d.id.clone()));
                    removed_actor = Some(d.id.clone());
                    self.changed.objects = true;
                }
        }
        // le comédien visé : le premier personnage de la scène (existant, sinon le premier dessiné)
        let existing_person = doc.project.objects.iter().filter(|o| Some(&o.id) != removed_actor.as_ref()).find(|o| is_person(object_type_of(o))).map(|o| o.position);
        let mut first_new_person: Option<Vec3> = None;
        let order = |it: &Value| match kind(it).as_deref() {
            Some("person") => 0,
            Some("set") => 1,
            Some("camera") => 2,
            Some("grip") => 3,
            Some("fixture") => 4,
            _ => 5,
        };
        let mut sorted = items.clone();
        sorted.sort_by_key(|it| order(it));
        let mut new_fixtures = 0usize;
        let mut new_objects = 0usize;
        let mut fixture_looks: Vec<Option<Map<String, Value>>> = Vec::new();
        let mut shapes: Vec<Value> = Vec::new();
        for it in sorted {
            let name = label(it);
            // forme dessinée : rectangle (deux coins) ou contour (points), en mètres sur le plan
            if kind(it).as_deref() == Some("shape") {
                let contour: Vec<Value> = it
                    .get("points")
                    .and_then(Value::as_array)
                    .map(|l| l.iter().filter_map(|p| Some(at(Some(p.get(0)?), Some(p.get(1)?), 0.0))).map(|v| json!([v[0], v[2]])).collect())
                    .unwrap_or_default();
                let (shape_kind, points) = if contour.len() >= 3 {
                    ("polygon", contour)
                } else if finite(it.get("x2")) && finite(it.get("y2")) {
                    let (a0, b) = (at(it.get("x"), it.get("y"), 0.0), at(it.get("x2"), it.get("y2"), 0.0));
                    ("rect", vec![json!([a0[0], a0[2]]), json!([b[0], b[2]])])
                } else {
                    guessed.push(format!("{name} : forme sans taille (donne x2, y2 ou points)").into());
                    continue;
                };
                let height = clamp(num(it.get("height"), 3.0), 0.0, 100.0);
                shapes.push(json!({ "id": shape_id(), "kind": shape_kind, "points": points, "style": { "stroke": "#c9ccd1", "width": 2 }, "text": name, "height": r2(height) }));
                continue;
            }
            let actor = actor_point(existing_person.or(first_new_person));
            if kind(it).as_deref() == Some("fixture") {
                let ty = match find_fixture_type(it.get("type")).or_else(|| find_fixture_type(it.get("label"))) {
                    Some(t) => t,
                    None => {
                        let Some(t) = library().fixture("arri.skypanel-s60c") else { continue };
                        guessed.push(format!("{name} → {} {}", t.brand, t.name).into());
                        t
                    }
                };
                let tube = is_tube(Some(ty));
                let hh = num(it.get("height"), if tube { r3(tube_length(Some(ty)) / 2.0 + 0.05) } else { 2.6 });
                let target = aim_of(it).unwrap_or(actor);
                let mut look = look_from_label(it.get("label"));
                for k in ["cct", "hue", "saturation", "intensity"] {
                    if finite(it.get(k)) {
                        look.insert(k.into(), jnum(js_number(it.get(k))));
                    }
                }
                let look = Value::Object(look);
                let pts = spots(it, hh);
                for (i, pt) in pts.iter().enumerate() {
                    let pos = lift_fixture(Some(ty), *pt);
                    let n = if pts.len() > 1 { format!("{name} {}", i + 1) } else { name.clone() };
                    let mut settings = Map::new();
                    let ch = look_changes(&new_fixture(ty, &settings), &look);
                    if let Some(s) = ch.settings {
                        settings.extend(s);
                    }
                    fixture_looks.push(ch.state);
                    edits.push(Edit::AddFixture { type_id: ty.id.clone(), position: pos, rotation: fixture_rotation(Some(ty), pos, target), name: (!n.is_empty()).then_some(n), settings });
                    made.push(Made::Fixture(new_fixtures, ty, pos));
                    new_fixtures += 1;
                }
                self.changed.fixtures = true;
                continue;
            }
            let k = kind(it);
            let mut props = Map::new();
            let ty = match k.as_deref() {
                Some("grip") => {
                    let (t, p) = grip_from_label(it.get("label"));
                    props = p;
                    t
                }
                Some("person") => object_type(person_from_label(it.get("label"))),
                Some("camera") => {
                    let l = norm(&it.get("label").filter(|v| !v.is_null()).map(js_string).unwrap_or_default());
                    let t = object_type(if re("travelling|dolly|rail").is_match(&l) { "camera-dolly" } else { "camera-tripod" });
                    let focal = js_number(it.get("focal"));
                    let mm = if focal != 0.0 && !focal.is_nan() { focal } else { re(r"(\d{2,3})\s*mm").captures(&l).and_then(|c| c[1].parse::<f64>().ok()).unwrap_or(f64::NAN) };
                    if mm != 0.0 && !mm.is_nan() {
                        props.insert("focal".into(), jnum(clamp(mm, 8.0, 300.0)));
                    }
                    t
                }
                _ => {
                    let by_id = it.get("type").filter(|t| truthy(Some(t))).and_then(Value::as_str).and_then(object_type);
                    let l = norm(&it.get("label").filter(|v| !v.is_null()).map(js_string).unwrap_or_default());
                    by_id.or_else(|| if re("mur|wall|cloison").is_match(&l) { object_type("wall-flat") } else { None }).or_else(|| find_object_type(it.get("label")))
                }
            };
            let Some(ty) = ty else {
                guessed.push(format!("{name} : non reconnu (ignoré)").into());
                continue;
            };
            let is_grip = k.as_deref() == Some("grip");
            let hh = if is_grip { r3(ty.h().unwrap_or(0.0) / 2.0 + 0.6) } else { 0.0 };
            let mut pts = spots(it, hh);
            let mut rotation = None;
            if ty.id().starts_with("wall") && finite(it.get("x2")) && finite(it.get("y2")) {
                // mur dessiné comme un trait : longueur et direction du trait ; un pan de mur fait
                // 8 m au plus : un long trait devient plusieurs pans alignés
                let a0 = at(it.get("x"), it.get("y"), 0.0);
                let b = at(it.get("x2"), it.get("y2"), 0.0);
                let len = clamp((b[0] - a0[0]).hypot(b[2] - a0[2]), 0.5, 80.0);
                let n = (len / 8.0).ceil() as usize;
                pts = (0..n).map(|i| [r3(a0[0] + ((b[0] - a0[0]) * (i as f64 + 0.5)) / n as f64), 0.0, r3(a0[2] + ((b[2] - a0[2]) * (i as f64 + 0.5)) / n as f64)]).collect();
                rotation = Some([0.0, r3(-(b[2] - a0[2]).atan2(b[0] - a0[0])), 0.0]);
                props = Map::new();
                props.insert("width".into(), jnum(r2(len / n as f64)));
            }
            let look = aim_of(it).or(match k.as_deref() {
                Some("camera" | "grip") => Some(actor),
                Some("person") => Some([0.0, 1.6, 50.0]),
                _ => None,
            });
            let props = clean_object_props(ty, &props);
            for (i, pt) in pts.iter().enumerate() {
                let n = if pts.len() > 1 { format!("{name} {}", i + 1) } else { name.clone() };
                let rot = rotation.unwrap_or([0.0, look.map_or(0.0, |l| face_yaw(*pt, l)), 0.0]);
                edits.push(Edit::AddObject { type_id: ty.id().to_owned(), position: *pt, rotation: rot, name: Some(n), props: props.clone() });
                made.push(Made::Object(new_objects, ty));
                new_objects += 1;
                if is_person(Some(ty)) && first_new_person.is_none() {
                    first_new_person = Some(*pt);
                }
            }
            self.changed.objects = true;
        }
        let (before_f, before_o) = (doc.project.fixtures.len(), doc.project.objects.len() - usize::from(removed_actor.is_some()));
        if !edits.is_empty() {
            doc.apply(Edit::Batch(edits));
        }
        // formes : ajoutées aux éléments du plan 2D
        if !shapes.is_empty() {
            let mut plot = doc.project.extra.get("plot").filter(|p| p.is_object()).cloned().unwrap_or_else(|| json!({}));
            if !plot["items"].is_array() {
                plot["items"] = json!([]);
            }
            let shape_names: Vec<String> = shapes.iter().map(|s| format!("{} (forme, {} m de haut)", s["text"].as_str().unwrap_or(""), fmt_num(s["height"].as_f64().unwrap_or(0.0)))).collect();
            if let Some(list) = plot["items"].as_array_mut() {
                list.extend(shapes);
            }
            doc.apply(Edit::Plot { plot, key: None, label: "les formes du dessin".into() });
            self.changed.objects = true;
            made_shapes.extend(shape_names);
        }
        let fixtures: Vec<Fixture> = doc.project.fixtures.get(before_f..).unwrap_or_default().to_vec();
        let objects: Vec<SceneObject> = doc.project.objects.get(before_o..).unwrap_or_default().to_vec();
        let updates: Vec<(String, Changes)> = fixtures.iter().zip(fixture_looks).filter_map(|(f, s)| s.map(|s| (f.id.clone(), Changes { state: Some(s), ..Changes::default() }))).collect();
        if !updates.is_empty() {
            doc.apply(Edit::UpdateFixtures(updates));
        }
        self.last_added.extend(fixtures.iter().map(|f| f.id.clone()));
        let pos_text = |p: Vec3| p.iter().map(|v| fmt_num(r2(*v))).collect::<Vec<_>>().join(" / ");
        let created: Vec<Value> = made
            .iter()
            .filter_map(|m| match m {
                Made::Fixture(i, t, pos) => fixtures.get(*i).map(|f| format!("{} ({} {}) {}", f.name, t.brand, t.name, pos_text(*pos))),
                Made::Object(i, t) => objects.get(*i).map(|o| format!("{} ({}) {}", o.name, t.label(), pos_text(o.position))),
            })
            .map(Value::from)
            .chain(made_shapes.into_iter().map(Value::from))
            .collect();
        let summary = format!("{} élément(s) placé(s) d’après le dessin{}", created.len(), if guessed.is_empty() { String::new() } else { format!(" ({} à vérifier)", guessed.len()) });
        obj([("created", Value::Array(created)), ("guessed", Value::Array(guessed)), ("scale", format!("{} m × {} m", fmt_num(r2(w)), fmt_num(r2(h))).into()), ("summary", summary.into())])
    }

    fn manage_plans(&mut self, doc: &mut Document, hooks: &mut dyn ToolHooks, a: &Value) -> Value {
        let info = doc.plan_info();
        let key = a.get("id").filter(|v| !v.is_null()).map(js_string).unwrap_or_default();
        let nk = norm(&key);
        let flat: Vec<(String, String, String, String)> = info.iter().flat_map(|s| s.plans.iter().map(move |p| (s.id.clone(), s.name.clone(), p.id.clone(), p.name.clone()))).collect();
        let find_plan = || flat.iter().find(|x| x.2 == key).or_else(|| flat.iter().find(|x| norm(&x.3) == nk)).or_else(|| flat.iter().find(|x| norm(&x.3).contains(&nk))).cloned();
        let current = || flat.iter().find(|x| info.iter().flat_map(|s| &s.plans).any(|p| p.current && p.id == x.2)).cloned();
        let list = |doc: &Document| {
            let info = doc.plan_info();
            let sequences: Vec<Value> = info
                .iter()
                .map(|s| obj([("id", s.id.clone().into()), ("name", s.name.clone().into()), ("plans", Value::Array(s.plans.iter().map(|p| obj([("id", p.id.clone().into()), ("name", p.name.clone().into()), ("fixtures", p.fixtures.into())])).collect()))]))
                .collect();
            let cur = info.iter().find_map(|s| s.plans.iter().find(|p| p.current).map(|p| (s.id.clone(), p.id.clone())));
            let mut v = obj([("sequences", Value::Array(sequences))]);
            v["current"] = cur.map_or(Value::Null, |(s, p)| obj([("seq", s.into()), ("plan", p.into())]));
            v
        };
        let start = |default: Start| match a.get("mode").and_then(Value::as_str) {
            Some("copy") => Start::Copy,
            Some("decor") => Start::Decor,
            Some("empty") => Start::Empty,
            _ => default,
        };
        let new_name = a.get("name").filter(|v| truthy(Some(v))).map(js_string);
        let action = a.get("action").and_then(Value::as_str).unwrap_or("");
        let summary = match action {
            "switch" => {
                let Some(p) = find_plan() else { return error(format!("Plan introuvable : {}", fmt_value(a.get("id")))) };
                hooks.plan_changing();
                doc.switch_plan(&p.2);
                hooks.plan_changed();
                self.changed.plans = true;
                format!("Plan ouvert : {} › {}", p.1, p.3)
            }
            "new_plan" => {
                hooks.plan_changing();
                let id = doc.new_plan(start(Start::Copy));
                if let (Some(id), Some(n)) = (&id, &new_name) {
                    doc.rename_plan(id, n);
                }
                hooks.plan_changed();
                self.changed.plans = true;
                let name = doc.plan_info().iter().flat_map(|s| s.plans.iter()).find(|p| Some(&p.id) == id.as_ref()).map(|p| p.name.clone()).unwrap_or_default();
                format!("Nouveau plan : {name}")
            }
            "new_sequence" => {
                hooks.plan_changing();
                let id = doc.new_sequence(start(Start::Empty));
                if let (Some(id), Some(n)) = (&id, &new_name) {
                    doc.rename_plan(id, n);
                }
                hooks.plan_changed();
                self.changed.plans = true;
                let name = doc.plan_info().iter().find(|s| Some(&s.id) == id.as_ref()).map(|s| s.name.clone()).unwrap_or_default();
                format!("Nouvelle séquence : {name}")
            }
            "rename_plan" => {
                if let (Some(p), Some(n)) = (find_plan().filter(|_| !key.is_empty()).or_else(current), a.get("name")) {
                    doc.rename_plan(&p.2, &js_string(n));
                }
                self.changed.plans = true;
                "Plan renommé".to_owned()
            }
            "rename_sequence" => {
                let seq = info.iter().find(|s| s.id == key).or_else(|| info.iter().find(|s| !key.is_empty() && norm(&s.name).contains(&nk))).map(|s| s.id.clone()).or_else(|| current().map(|c| c.0));
                if let (Some(s), Some(n)) = (seq, a.get("name")) {
                    doc.rename_plan(&s, &js_string(n));
                }
                self.changed.plans = true;
                "Séquence renommée".to_owned()
            }
            _ => format!("{} plan(s)", flat.len()),
        };
        let mut v = list(doc);
        v["summary"] = summary.into();
        v
    }
}

/// Qui commande le projecteur : la console ARKLIGHT, le DMX reçu (ou conservé), ou ses propres réglages.
fn control_of(f: &Fixture, ty: Option<&FixtureType>, dmx: &DmxState) -> String {
    if console_held(f) {
        return "console ARKLIGHT".into();
    }
    let u = f.settings.universe.unwrap_or(1);
    let v5 = ty.is_some_and(FixtureType::is_astera_v5);
    let via_dmx = if v5 {
        (truthy(setting(f, "crmxLinked")) || truthy(setting(f, "wired"))) && dmx.data.contains(&u)
    } else {
        ty.is_some_and(|t| t.pixels > 0) && control_source(f, dmx.data.contains(&u)) == Source::Dmx
    };
    if via_dmx {
        return if dmx.live.contains(&u) { format!("DMX reçu (univers {u})") } else { format!("DMX conservé, plus de signal (univers {u}) : release_control pour rendre la main") };
    }
    if v5 { "mode autonome".into() } else { "réglage manuel".into() }
}

/// L'état du plan pour `get_scene`.
fn scene_summary(doc: &Document, hooks: &dyn ToolHooks, include_objects: bool) -> Value {
    let p = &doc.project;
    let env = &p.environment;
    let info = doc.plan_info();
    let cur = info.iter().find_map(|s| s.plans.iter().find(|x| x.current).map(|x| (s.name.clone(), x.name.clone())));
    let dmx = hooks.dmx_state();
    let fixtures: Vec<Value> = p
        .fixtures
        .iter()
        .map(|f| {
            let ty = library().fixture(&f.type_id);
            let mode = ty.and_then(|t| t.mode(f.settings.dmx_mode.as_deref()));
            let fp = mode.map_or(0, DmxMode::footprint);
            let (u, ad) = (f.settings.universe.unwrap_or(1), f.settings.dmx_address.unwrap_or(1) as usize);
            let dmx_text = match mode {
                Some(m) if fp > 0 => format!("U{u} {ad}-{} ({fp} c., {})", ad + fp - 1, mode_label(m)),
                _ => "sans DMX".into(),
            };
            let m = state_obj(f, "manual").cloned().unwrap_or(Value::Null);
            let sa = setting(f, "standalone").filter(|s| truthy(Some(s)));
            let look = match sa {
                Some(sa) if f.type_id.starts_with("astera.") => format!("autonome {} %", sa.get("intensity").filter(|v| !v.is_null()).map_or_else(|| "100".into(), js_string)),
                _ => {
                    let mode = m.get("mode").filter(|v| truthy(Some(v))).map_or_else(|| "cct".into(), js_string);
                    let level = js_round(m.get("intensity").filter(|v| !v.is_null()).map_or(0.6, |v| js_number(Some(v))) * 100.0);
                    let tail = if m.get("mode").and_then(Value::as_str) == Some("hsi") {
                        format!(" teinte {}°", fmt_num(js_round(m.get("hue").filter(|v| truthy(Some(v))).map_or(0.0, |v| js_number(Some(v))))))
                    } else {
                        format!(" {} K", m.get("cct").filter(|v| truthy(Some(v))).map_or_else(|| "5600".into(), js_string))
                    };
                    format!("{mode} {} %{tail}", fmt_num(level))
                }
            };
            obj([
                ("id", f.id.clone().into()),
                ("name", f.name.clone().into()),
                ("type", f.type_id.clone().into()),
                ("fix", f.settings.fixture_number.map_or(Value::Null, Value::from)),
                ("dmx", dmx_text.into()),
                ("pos", Value::Array(f.position.iter().map(|v| jnum(r2(*v))).collect())),
                ("on", (f.state.get("power") != Some(&Value::Bool(false))).into()),
                ("control", control_of(f, ty, &dmx).into()),
                ("look", look.into()),
            ])
        })
        .collect();
    let mut v = obj([
        ("project", p.name.clone().into()),
        ("sequence", cur.as_ref().map_or(Value::Null, |c| c.0.clone().into())),
        ("plan", cur.as_ref().map_or(Value::Null, |c| c.1.clone().into())),
        ("plans_total", info.iter().map(|s| s.plans.len()).sum::<usize>().into()),
        (
            "environment",
            obj([("haze_pct", jnum(js_round(env.haze * 100.0))), ("exposure_ev", jnum(env.exposure)), ("ambient_pct", jnum(js_round(env.ambient * 100.0))), ("floor", env.floor.into()), ("cyc", env.cyc.into())]),
        ),
        ("fixtures", Value::Array(fixtures)),
        ("groups", Value::Array(p.groups.iter().map(|g| obj([("name", g.name.clone().into()), ("count", g.ids.len().into())])).collect())),
    ]);
    if include_objects {
        v["objects"] = Value::Array(p.objects.iter().map(|o| obj([("id", o.id.clone().into()), ("name", o.name.clone().into()), ("type", o.type_id.clone().into()), ("pos", Value::Array(o.position.iter().map(|x| jnum(r2(*x))).collect()))])).collect());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visee() {
        // droit devant : aucun angle
        assert_eq!(aim_rotation([0.0, 1.6, -3.0], [0.0, 1.6, 0.0]), [0.0, 0.0, 0.0]);
        // de face, au-dessus et à droite du comédien
        let r = aim_rotation([3.0, 3.0, 3.0], [0.0, 1.6, 0.0]);
        assert!(r[1] < 0.0 && r[0] > 0.0, "{r:?}");
        // à la verticale
        let r = aim_rotation([0.0, 5.0, 0.0], [0.0, 0.0, 0.0]);
        assert!((r[0] - std::f64::consts::FRAC_PI_2).abs() < 0.01, "{r:?}");
        assert_eq!(face_yaw([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]), r3(std::f64::consts::FRAC_PI_2));
    }

    #[test]
    fn etiquettes_du_dessin() {
        let l = look_from_label(Some(&json!("SKY S60 face 3200K bleu 50 %")));
        assert_eq!(l.get("cct"), Some(&json!(3200)));
        assert_eq!(l.get("hue"), Some(&json!(220)));
        assert_eq!(l.get("intensity"), Some(&json!(50)));
        let (t, p) = grip_from_label(Some(&json!("8 x 8 Diff")));
        assert_eq!(t.map(ObjType::id), Some("frame-8x8"));
        assert_eq!(p.get("cloth"), Some(&json!("silk")));
        // comme écrit sur un plan papier
        let (t, p) = grip_from_label(Some(&json!("Cadre 6x6 Slik 1/4")));
        assert_eq!(t.map(ObjType::id), Some("frame-6x6"));
        assert_eq!(p.get("cloth"), Some(&json!("silk-1-4")));
        for (label, cloth) in [("12x12 Full Grid", "grid"), ("8x8 light grid", "grid-1-2"), ("6x6 quarter grid", "grid-1-4"), ("20x20 griflon", "griffolyn"), ("8x8 muslin écru", "muslin-unbleached"), ("12x12 lamé or", "gold-lame"), ("6x6 soie 1/2", "silk-1-2"), ("8x8 tulle double", "net-double"), ("8x8 Lisa Marie", "checkerboard")] {
            assert_eq!(grip_from_label(Some(&json!(label))).1.get("cloth"), Some(&json!(cloth)), "{label}");
        }
        let (t, p) = grip_from_label(Some(&json!("drapeau noir")));
        assert_eq!(t.map(ObjType::id), Some("flag-24x36"));
        assert_eq!(p.get("cloth"), Some(&json!("solid")));
        assert_eq!(person_from_label(Some(&json!("COMÉDIENNE"))), "person-woman");
        assert_eq!(person_from_label(Some(&json!("Comédien"))), "person-man");
        assert_eq!(person_from_label(Some(&json!("enfant assis"))), "person-child");
    }

    #[test]
    fn recherche() {
        assert_eq!(find_fixture_type(Some(&json!("s60"))).map(|t| t.id.as_str()), Some("arri.skypanel-s60c"));
        assert_eq!(find_fixture_type(Some(&json!("astera.titan"))).map(|t| t.id.as_str()), Some("astera.titan"));
        assert!(find_fixture_type(Some(&json!("zzz"))).is_none());
        assert!(find_object_type(Some(&json!("canapé"))).map(ObjType::id).is_some());
        assert!(catalog_text().starts_with("PROJECTEURS (id — marque modèle) :\n"));
        assert_eq!(tool_names().len(), 13);
    }
}
