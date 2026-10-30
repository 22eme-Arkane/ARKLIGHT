//! Éléments du plan de feu tels qu'enregistrés dans le projet (`scene.plot`) : portage des
//! fonctions du plan de `server/scene-store.js` (nettoyage d'un élément, ajout, modification,
//! suppression, image de fond, couleur du fond, cartouche), des réglages de l'élément sélectionné
//! de `web/js/plot/plot-view.js` (couleur, remplissage, pointillés, trait, taille du texte,
//! hauteur 3D) et de la nature des modifications de `server/history.js` (fusion d'un geste
//! continu, libellé d'« Annuler »).
//!
//! Fonctions pures : elles renvoient le nouveau `plot` du projet, au même JSON que la version
//! JavaScript (les fichiers restent lisibles par les deux versions).

use crate::geometry::P2;
use crate::js::{round, truthy};
use crate::render::{Selection, View};
use arklight_core::document::Edit;
use arklight_core::project::Project;
use serde_json::{Map, Value, json};

/// Natures des éléments du plan (`PLOT_KINDS`).
pub const PLOT_KINDS: [&str; 8] = ["rect", "ellipse", "polygon", "line", "freehand", "text", "arrow", "measure"];

/// Épaisseurs de trait proposées (px).
pub const WIDTHS: [f64; 5] = [1.0, 2.0, 3.0, 5.0, 8.0];

/// Tailles de texte proposées (px).
pub const SIZES: [f64; 6] = [12.0, 16.0, 20.0, 28.0, 40.0, 60.0];

/// Couleur des formes tracées.
pub const ACCENT: &str = "#ffad3b";

/// Début de l'adresse d'une image de fond (images envoyées au serveur, dossier `scenes/assets`).
pub const ASSET_PREFIX: &str = "/assets/";

/// Arrondi au millimètre (`r3`).
pub fn r3(v: f64) -> f64 {
    round(v * 1000.0) / 1000.0
}

/// Nombre JSON écrit comme `JSON.stringify` : entier sans « .0 », NaN et infinis en `null`.
pub fn jnum(x: f64) -> Value {
    if x.is_finite() && x == x.trunc() && x.abs() < 9_007_199_254_740_992.0 {
        return Value::from(x as i64); // -0 compris : « 0 »
    }
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// Point [x, z] en JSON.
pub fn jpoint([x, z]: P2) -> Value {
    Value::Array(vec![jnum(x), jnum(z)])
}

/// `Number(v)` de JavaScript (absent : NaN).
pub fn js_number(v: Option<&Value>) -> f64 {
    match v {
        None => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => number_of_text(s),
        Some(Value::Array(a)) => match a.as_slice() {
            [] => 0.0,
            [one] => match one {
                Value::Null => 0.0,
                Value::Array(_) | Value::String(_) | Value::Number(_) => js_number(Some(one)),
                Value::Bool(b) => number_of_text(&b.to_string()),
                Value::Object(_) => f64::NAN,
            },
            _ => f64::NAN,
        },
        Some(Value::Object(_)) => f64::NAN,
    }
}

/// `Number(texte)` : décimal (avec exposant), « Infinity », hexadécimal, octal, binaire ; vide : 0.
fn number_of_text(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = t.strip_prefix(prefix) {
            return u64::from_str_radix(digits, radix).map_or(f64::NAN, |n| n as f64);
        }
    }
    if t.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-')) {
        t.parse().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// `num(v, d)` du serveur : le nombre s'il est fini, sinon la valeur par défaut.
fn num_or(v: Option<&Value>, d: f64) -> f64 {
    let n = js_number(v);
    if n.is_finite() { n } else { d }
}

/// `texte.slice(0, n)` (unités UTF-16).
fn slice16(s: &str, n: usize) -> String {
    let mut len = 0;
    let mut out = String::new();
    for c in s.chars() {
        len += c.len_utf16();
        if len > n {
            break;
        }
        out.push(c);
    }
    out
}

/// `p?.[i]` : élément d'un tableau, lettre d'un texte, champ « 0 » d'un objet.
fn index(p: &Value, i: usize) -> Option<Value> {
    match p {
        Value::Array(a) => a.get(i).cloned(),
        Value::String(s) => s.chars().nth(i).map(|c| Value::String(c.to_string())),
        Value::Object(m) => m.get(&i.to_string()).cloned(),
        _ => None,
    }
}

/// `isPlainObject`
fn plain(v: Option<&Value>) -> Option<&Map<String, Value>> {
    v.and_then(Value::as_object)
}

/// `cleanColor` : « #rrggbb » en minuscules, sinon rien.
pub fn clean_color(c: Option<&Value>) -> Value {
    match c.and_then(Value::as_str) {
        Some(s) if s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit()) => Value::String(s.to_ascii_lowercase()),
        _ => Value::Null,
    }
}

/// Identifiant neuf (`newId` : préfixe, heure en base 36, quatre caractères au hasard).
pub fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    // mélange (splitmix64) de l'heure fine et du compteur : quatre caractères en base 36
    let mut x = (now.as_nanos() as u64) ^ n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let tail = radix36(x % 1_679_616 + 1_679_616); // 5 chiffres, le premier (« 1 ») retiré
    format!("{prefix}{}{}", radix36(now.as_millis() as u64), &tail[1..])
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

// ------------------------------------------------------------------ nettoyage
/// Plan vide (`emptyPlot`).
pub fn empty_plot() -> Value {
    json!({ "items": [], "underlay": null, "background": null, "title": { "production": "", "author": "", "notes": "" } })
}

/// `cleanPlotItem` : élément du plan nettoyé (coordonnées au sol en mètres, arrondies au
/// millimètre ; style complet ; hauteur du volume 3D entre 0 et 100 m) ; None si l'élément n'est
/// pas valable (nature inconnue, aucun point). `id` : identifiant imposé.
pub fn clean_plot_item(it: &Value, id: Option<&str>) -> Option<Value> {
    let o = it.as_object()?;
    let kind = o.get("kind").and_then(Value::as_str).filter(|k| PLOT_KINDS.contains(k))?;
    let points: Vec<Value> = o
        .get("points")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .take(5000)
        .map(|p| jpoint([r3(num_or(index(p, 0).as_ref(), 0.0)), r3(num_or(index(p, 1).as_ref(), 0.0))]))
        .collect();
    if points.is_empty() {
        return None;
    }
    let empty = Map::new();
    let style = plain(o.get("style")).unwrap_or(&empty);
    let text_or = |v: Option<&Value>, n: usize, d: &str| v.and_then(Value::as_str).map_or_else(|| d.to_string(), |s| slice16(s, n));
    let id = match id {
        Some(id) => Value::String(id.to_string()),
        None => o.get("id").filter(|v| truthy(Some(v))).cloned().unwrap_or_else(|| Value::String(new_id("p"))),
    };
    let mut m = Map::new();
    m.insert("id".into(), id);
    m.insert("kind".into(), kind.into());
    m.insert("points".into(), Value::Array(points));
    m.insert("text".into(), text_or(o.get("text"), 500, "").into());
    m.insert(
        "style".into(),
        json!({
            "stroke": text_or(style.get("stroke"), 20, ACCENT),
            "fill": text_or(style.get("fill"), 20, "none"),
            "width": jnum(num_or(style.get("width"), 2.0).clamp(0.5, 20.0)),
            "dash": truthy(style.get("dash")),
            "size": jnum(num_or(style.get("size"), 16.0).clamp(6.0, 200.0)),
        }),
    );
    // > 0 : volume extrudé dans la vue 3D
    m.insert("height".into(), jnum(num_or(o.get("height"), 0.0).clamp(0.0, 100.0)));
    Some(Value::Object(m))
}

/// Nettoyage du plan d'un projet ouvert (`sanitize` du serveur) : plan vide complété, éléments
/// nettoyés (les invalides retirés), cartouche complété, couleur du fond vérifiée.
pub fn clean_plot(plot: Option<&Value>) -> Value {
    let mut out = match empty_plot() {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    if let Some(p) = plain(plot) {
        for (k, v) in p {
            out.insert(k.clone(), v.clone());
        }
    }
    let items: Vec<Value> = plain(plot).and_then(|p| p.get("items")).and_then(Value::as_array).map(|a| a.iter().filter_map(|it| clean_plot_item(it, None)).collect()).unwrap_or_default();
    out.insert("items".into(), Value::Array(items));
    let mut title = json!({ "production": "", "author": "", "notes": "" });
    if let (Some(t), Value::Object(tm)) = (plain(plot).and_then(|p| plain(p.get("title"))), &mut title) {
        for (k, v) in t {
            tm.insert(k.clone(), v.clone());
        }
    }
    out.insert("title".into(), title);
    let bg = clean_color(out.get("background"));
    out.insert("background".into(), bg);
    Value::Object(out)
}

/// Plan du projet (vide s'il n'en a pas).
fn plot_of(plot: Option<&Value>) -> Map<String, Value> {
    match plot {
        Some(Value::Object(m)) => m.clone(),
        _ => match empty_plot() {
            Value::Object(m) => m,
            _ => Map::new(),
        },
    }
}

fn items_mut(p: &mut Map<String, Value>) -> &mut Vec<Value> {
    let entry = p.entry("items").or_insert_with(|| Value::Array(Vec::new()));
    if !entry.is_array() {
        *entry = Value::Array(Vec::new());
    }
    match entry {
        Value::Array(a) => a,
        _ => unreachable!("tableau des éléments"),
    }
}

/// Éléments du plan (JSON du projet).
pub fn plot_items(project: &Project) -> &[Value] {
    project.extra.get("plot").and_then(|p| p.get("items")).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

/// Un élément du plan (JSON du projet).
pub fn plot_item<'a>(project: &'a Project, id: &str) -> Option<&'a Value> {
    plot_items(project).iter().find(|x| x.get("id").and_then(Value::as_str) == Some(id))
}

// ------------------------------------------------------------------ modifications
/// `addPlotItem` : ajoute un élément (nettoyé) ; renvoie le plan et l'identifiant de l'élément
/// (celui de `item` s'il est libre, sinon un neuf). None : élément invalide.
pub fn add_item(plot: Option<&Value>, item: &Value) -> Option<(Value, String)> {
    let mut p = plot_of(plot);
    let items = items_mut(&mut p);
    let wanted = item.get("id").and_then(Value::as_str).filter(|id| !id.is_empty() && !items.iter().any(|x| x.get("id").and_then(Value::as_str) == Some(id)));
    let id = wanted.map_or_else(|| new_id("p"), str::to_string);
    let it = clean_plot_item(item, Some(&id))?;
    items.push(it);
    Some((Value::Object(p), id))
}

/// `updatePlotItem` : champs changés (le style fusionné champ par champ), élément nettoyé de
/// nouveau. None : élément introuvable ou devenu invalide (rien ne change).
pub fn update_item(plot: Option<&Value>, id: &str, changes: &Value) -> Option<Value> {
    let mut p = plot_of(plot);
    let items = items_mut(&mut p);
    let i = items.iter().position(|x| x.get("id").and_then(Value::as_str) == Some(id))?;
    let cur = &items[i];
    let mut merged = cur.as_object().cloned().unwrap_or_default();
    if let Some(c) = changes.as_object() {
        for (k, v) in c {
            merged.insert(k.clone(), v.clone());
        }
    }
    let mut style = plain(cur.get("style")).cloned().unwrap_or_default();
    if let Some(s) = plain(changes.get("style")) {
        for (k, v) in s {
            style.insert(k.clone(), v.clone());
        }
    }
    merged.insert("style".into(), Value::Object(style));
    items[i] = clean_plot_item(&Value::Object(merged), Some(id))?;
    Some(Value::Object(p))
}

/// `removePlotItem` : None si l'élément n'existe pas.
pub fn remove_item(plot: Option<&Value>, id: &str) -> Option<Value> {
    let mut p = plot_of(plot);
    let items = items_mut(&mut p);
    let i = items.iter().position(|x| x.get("id").and_then(Value::as_str) == Some(id))?;
    items.remove(i);
    Some(Value::Object(p))
}

/// `updatePlotMeta` : image de fond (null : retirée ; acceptée seulement si son adresse commence
/// par `/assets/`), couleur du fond, cartouche (production, chef électro, notes).
pub fn update_meta(plot: Option<&Value>, meta: &Value) -> Value {
    let mut p = plot_of(plot);
    if let Some(bg) = meta.get("background") {
        p.insert("background".into(), clean_color(Some(bg)));
    }
    match meta.get("underlay") {
        Some(Value::Null) => {
            p.insert("underlay".into(), Value::Null);
        }
        Some(Value::Object(u)) if u.get("url").and_then(Value::as_str).is_some_and(|s| s.starts_with(ASSET_PREFIX)) => {
            let n = |k: &str, d: f64| num_or(u.get(k), d);
            p.insert(
                "underlay".into(),
                json!({
                    "url": u["url"],
                    "x": jnum(n("x", 0.0)),
                    "z": jnum(n("z", 0.0)),
                    "width": jnum(n("width", 10.0).max(0.1)),
                    "aspect": jnum(n("aspect", 1.0).max(0.01)),
                    "rotation": jnum(n("rotation", 0.0)),
                    "opacity": jnum(n("opacity", 0.6).clamp(0.05, 1.0)),
                    "floor3d": truthy(u.get("floor3d")),
                }),
            );
        }
        _ => {}
    }
    if let Some(t) = plain(meta.get("title")) {
        let title = p.entry("title").or_insert_with(|| json!({}));
        if !title.is_object() {
            *title = json!({});
        }
        for k in ["production", "author", "notes"] {
            if let Some(s) = t.get(k).and_then(Value::as_str) {
                title[k] = slice16(s, 300).into();
            }
        }
    }
    Value::Object(p)
}

// ------------------------------------------------------------------ opérations
/// Changement demandé par les outils du plan.
#[derive(Debug, Clone, PartialEq)]
pub enum EditOp {
    /// ajoute un élément (`plot.add`) : élément déjà nettoyé, avec un identifiant neuf
    AddItem(Value),
    /// modifie un élément (`plot.update`) : champs changés, `style` fusionné champ par champ
    UpdateItem { id: String, changes: Value },
    /// retire un élément (`plot.remove`)
    RemoveItem(String),
    /// image de fond, couleur du fond, cartouche (`plot.meta`) : `{ underlay }`, `{ background }`
    /// ou `{ title }`
    Meta(Value),
    /// projecteurs ou objets déplacés, tournés : à appliquer tel quel au document
    Doc(Edit),
    /// nouvelle sélection (projecteurs, objet, élément du plan) : à reporter dans l'application
    Select(Selection),
}

impl EditOp {
    /// Opération sur le `plot` du projet (à appliquer avec [`apply_op`]).
    pub fn is_plot(&self) -> bool {
        matches!(self, Self::AddItem(_) | Self::UpdateItem { .. } | Self::RemoveItem(_) | Self::Meta(_))
    }

    /// Nature de la modification (`historyKey`) : deux modifications de même nature à moins de
    /// 1,2 s d'intervalle ne font qu'une étape d'annulation. None : jamais fusionnée.
    pub fn history_key(&self) -> Option<String> {
        match self {
            Self::UpdateItem { id, changes } => Some(format!("plot.update|{id}|{}", change_keys(&changes_entries(changes)))),
            Self::Meta(m) => {
                let undefined = |v: Option<&Value>| v.filter(|v| !v.is_null()).cloned();
                let entries = vec![("underlay".to_string(), undefined(m.get("underlay"))), ("title".to_string(), undefined(m.get("title"))), ("background".to_string(), m.get("background").map(|_| json!(1)))];
                Some(format!("plot.meta|{}", change_keys(&entries)))
            }
            _ => None,
        }
    }

    /// Ce que l'étape annule, pour le menu (« l’ajout au plan de feu »).
    pub fn label(&self) -> String {
        match self {
            Self::AddItem(_) => "l’ajout au plan de feu".into(),
            Self::UpdateItem { .. } => "la modification du plan de feu".into(),
            Self::RemoveItem(_) => "la suppression dans le plan de feu".into(),
            Self::Meta(m) => {
                if truthy(m.get("title")) {
                    "le cartouche du plan".into()
                } else if m.get("background").is_some() {
                    "la couleur du fond du plan".into()
                } else {
                    "l’image de fond du plan".into()
                }
            }
            Self::Doc(e) => e.label(),
            Self::Select(_) => "la sélection".into(),
        }
    }
}

fn changes_entries(changes: &Value) -> Vec<(String, Option<Value>)> {
    changes.as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect()).unwrap_or_default()
}

/// `keys(changes)` de l'historique : « points », « style.fill,style.stroke »…
fn change_keys(entries: &[(String, Option<Value>)]) -> String {
    let mut k: Vec<String> = entries
        .iter()
        .flat_map(|(k, v)| match v {
            Some(Value::Object(m)) => m.keys().map(|s| format!("{k}.{s}")).collect::<Vec<_>>(),
            _ => vec![k.clone()],
        })
        .collect();
    k.sort();
    k.join(",")
}

/// Applique une opération au `plot` d'un projet ; renvoie le nouveau `plot` (None : rien ne
/// change, ou opération qui ne touche pas au plan).
pub fn apply_op(plot: Option<&Value>, op: &EditOp) -> Option<Value> {
    match op {
        EditOp::AddItem(item) => add_item(plot, item).map(|(p, _)| p),
        EditOp::UpdateItem { id, changes } => update_item(plot, id, changes),
        EditOp::RemoveItem(id) => remove_item(plot, id),
        EditOp::Meta(m) => Some(update_meta(plot, m)),
        EditOp::Doc(_) | EditOp::Select(_) => None,
    }
}

/// Applique une opération du plan directement au projet (sans annuler / rétablir) ; true si le
/// plan a changé.
pub fn apply_to_project(project: &mut Project, op: &EditOp) -> bool {
    match apply_op(project.extra.get("plot"), op) {
        Some(plot) => {
            project.extra.insert("plot".into(), plot);
            true
        }
        None => false,
    }
}

// ------------------------------------------------------------------ réglages de l'élément sélectionné
/// Réglage de l'élément sélectionné (barre des propriétés du plan).
#[derive(Debug, Clone, PartialEq)]
pub enum ItemChange {
    /// texte (« Texte » d'une annotation, étiquette facultative d'une forme)
    Text(String),
    /// couleur du trait (et du remplissage d'une forme remplie)
    Color(String),
    /// forme remplie (rectangle, ellipse, forme, main levée)
    Filled(bool),
    Dash(bool),
    /// épaisseur du trait (px)
    Width(f64),
    /// taille du texte (px)
    Size(f64),
    /// hauteur du volume dans la vue 3D (m ; 0 : forme plate)
    Height(f64),
}

/// Forme fermée : se remplit et peut devenir un volume 3D.
pub fn is_closed(kind: &str) -> bool {
    matches!(kind, "rect" | "ellipse" | "polygon" | "freehand")
}

/// Nom d'une nature d'élément (`KIND_NAMES`).
pub fn kind_name(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "rect" => "Rectangle",
        "ellipse" => "Ellipse",
        "polygon" => "Forme",
        "freehand" => "Main levée",
        "line" => "Ligne",
        "arrow" => "Flèche",
        "text" => "Texte",
        "measure" => "Mesure",
        _ => return None,
    })
}

/// Couleur affichée par le sélecteur de couleur (celle du trait si c'est « #rrggbb »).
pub fn color_value(item: &Value) -> String {
    match item.get("style").and_then(|s| s.get("stroke")).and_then(Value::as_str) {
        Some(s) if s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit()) => s.to_string(),
        _ => ACCENT.into(),
    }
}

/// Champs changés par un réglage (`changes` de `plot.update`).
pub fn item_changes(item: &Value, change: &ItemChange) -> Value {
    let st = item.get("style");
    let fill = st.and_then(|s| s.get("fill"));
    match change {
        ItemChange::Text(t) => json!({ "text": t }),
        ItemChange::Color(c) => {
            let mut style = json!({ "stroke": c });
            if truthy(fill) && fill.and_then(Value::as_str) != Some("none") {
                style["fill"] = c.clone().into();
            }
            json!({ "style": style })
        }
        ItemChange::Filled(on) => {
            let stroke = st.and_then(|s| s.get("stroke")).filter(|v| truthy(Some(v))).cloned().unwrap_or_else(|| ACCENT.into());
            json!({ "style": { "fill": if *on { stroke } else { "none".into() } } })
        }
        ItemChange::Dash(on) => json!({ "style": { "dash": on } }),
        ItemChange::Width(w) => json!({ "style": { "width": jnum(*w) } }),
        ItemChange::Size(s) => json!({ "style": { "size": jnum(*s) } }),
        ItemChange::Height(h) => json!({ "height": jnum(if h.is_nan() { 0.0 } else { h.max(0.0) }) }),
    }
}

/// Opération d'un réglage de l'élément sélectionné.
pub fn edit_item(item: &Value, change: &ItemChange) -> Option<EditOp> {
    let id = item.get("id")?.as_str()?;
    Some(EditOp::UpdateItem { id: id.to_string(), changes: item_changes(item, change) })
}

// ------------------------------------------------------------------ image de fond, fond, cartouche
/// Image de fond importée (`uploadUnderlay`) : centrée sur la vue, 20 m de large, à caler ensuite.
/// `url` : adresse de l'image (« /assets/… »), `aspect` : largeur / hauteur de l'image.
pub fn underlay_import(url: &str, aspect: f64, view: &View) -> EditOp {
    EditOp::Meta(json!({ "underlay": { "url": url, "x": jnum(view.cx), "z": jnum(view.cz), "width": 20, "aspect": jnum(aspect), "rotation": 0, "opacity": 0.6, "floor3d": false } }))
}

/// Réglage de l'image de fond (`{ width }`, `{ rotation }`, `{ opacity }`, `{ floor3d }`, ou
/// `{ x, z }` pour « Centrer sur la vue ») ; None sans image de fond.
pub fn underlay_change(plot: Option<&Value>, changes: &Value) -> Option<EditOp> {
    let mut u = plot?.get("underlay")?.as_object()?.clone();
    for (k, v) in changes.as_object()? {
        u.insert(k.clone(), v.clone());
    }
    Some(EditOp::Meta(json!({ "underlay": u })))
}

/// « Retirer l’image »
pub fn underlay_remove() -> EditOp {
    EditOp::Meta(json!({ "underlay": null }))
}

/// Couleur du fond du plan (None : automatique, sombre à l'écran et blanche à l'export).
pub fn background_change(color: Option<&str>) -> EditOp {
    EditOp::Meta(json!({ "background": color }))
}

/// Champ du cartouche (« production », « author »).
pub fn title_change(plot: Option<&Value>, key: &str, value: &str) -> EditOp {
    let mut t = plot.and_then(|p| plain(p.get("title"))).cloned().unwrap_or_default();
    t.insert(key.into(), value.into());
    EditOp::Meta(json!({ "title": t }))
}

/// Calage de l'image de fond : deux points cliqués `a`, `b` dont la distance réelle est `real`
/// mètres ; le premier point reste en place, l'image grandit ou rétrécit autour de lui. None :
/// distance nulle ou pas d'image.
pub fn calibrate(plot: Option<&Value>, a: P2, b: P2, real: f64) -> Option<EditOp> {
    let measured = crate::js::hypot(b[0] - a[0], b[1] - a[1]);
    if !(real > 0.0 && measured > 0.0) {
        return None;
    }
    let u = plot?.get("underlay")?.as_object()?;
    let k = real / measured;
    let n = |key: &str| js_number(u.get(key));
    let mut next = u.clone();
    next.insert("width".into(), jnum(n("width") * k));
    next.insert("x".into(), jnum(a[0] + (n("x") - a[0]) * k));
    next.insert("z".into(), jnum(a[1] + (n("z") - a[1]) * k));
    Some(EditOp::Meta(json!({ "underlay": next })))
}

// ------------------------------------------------------------------ volumes 3D
/// Forme du plan montée en volume dans la vue 3D (`buildPlotVolumes`).
#[derive(Debug, Clone, PartialEq)]
pub struct PlotVolume {
    pub id: String,
    /// contour au sol [x, z] (m)
    pub outline: Vec<P2>,
    /// hauteur (m), extrudée vers le haut depuis le sol
    pub height: f64,
}

/// Volumes 3D du plan : formes fermées d'une hauteur > 0 (rectangle : 4 coins, ellipse : 40
/// points, forme et main levée : leurs points ; au moins 3 points).
pub fn plot_volumes(project: &Project) -> Vec<PlotVolume> {
    let mut out = Vec::new();
    for it in plot_items(project) {
        let height = it.get("height").and_then(Value::as_f64).unwrap_or(0.0);
        let kind = it.get("kind").and_then(Value::as_str).unwrap_or("");
        // `!(it.height > 0)` : absente, nulle, négative ou NaN
        if height.is_nan() || height <= 0.0 || !is_closed(kind) {
            continue;
        }
        let pts: Vec<P2> = it.get("points").and_then(Value::as_array).map(|a| a.iter().filter_map(|p| Some([p.get(0)?.as_f64()?, p.get(1)?.as_f64()?])).collect()).unwrap_or_default();
        let outline = match (kind, pts.as_slice()) {
            ("rect", [[x0, z0], [x1, z1], ..]) => vec![[*x0, *z0], [*x1, *z0], [*x1, *z1], [*x0, *z1]],
            ("ellipse", [[x0, z0], [x1, z1], ..]) => {
                let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
                let (rx, rz) = ((x1 - x0).abs() / 2.0, (z1 - z0).abs() / 2.0);
                (0..40).map(|i| f64::from(i) / 40.0 * std::f64::consts::TAU).map(|a| [cx + rx * a.cos(), cz + rz * a.sin()]).collect()
            }
            _ => pts,
        };
        if outline.len() >= 3 {
            out.push(PlotVolume { id: it.get("id").map(crate::js::string_of).unwrap_or_default(), outline, height });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_comme_javascript() {
        assert_eq!(jnum(2.0).to_string(), "2");
        assert_eq!(jnum(-0.0).to_string(), "0");
        assert_eq!(jnum(1.25).to_string(), "1.25");
        assert_eq!(jnum(f64::NAN), Value::Null);
        assert_eq!(js_number(Some(&json!(" 12.5 "))), 12.5);
        assert_eq!(js_number(Some(&json!(""))), 0.0);
        assert_eq!(js_number(Some(&json!("0x10"))), 16.0);
        assert!(js_number(Some(&json!("inf"))).is_nan());
        assert!(js_number(Some(&json!("12px"))).is_nan());
        assert_eq!(js_number(Some(&json!(null))), 0.0);
        assert_eq!(js_number(Some(&json!(true))), 1.0);
        assert_eq!(js_number(Some(&json!(["7"]))), 7.0);
        assert!(js_number(None).is_nan());
        assert_eq!(r3(1.23456), 1.235);
    }

    #[test]
    fn element_nettoye() {
        let it = json!({ "kind": "rect", "points": [[1.23456, "2"], [null, [3]], "ab", 5], "style": { "stroke": "#123456789012345678901234", "width": 50, "size": 2, "dash": 1 }, "text": 12, "height": -3, "extra": true });
        let c = clean_plot_item(&it, Some("p1")).unwrap();
        assert_eq!(
            c,
            json!({ "id": "p1", "kind": "rect", "points": [[1.235, 2], [0, 3], [0, 0], [0, 0]], "text": "", "style": { "stroke": "#1234567890123456789", "fill": "none", "width": 20, "dash": true, "size": 6 }, "height": 0 })
        );
        assert!(clean_plot_item(&json!({ "kind": "cloud", "points": [[0, 0]] }), None).is_none());
        assert!(clean_plot_item(&json!({ "kind": "line", "points": [] }), None).is_none());
        assert!(clean_plot_item(&json!("rect"), None).is_none());
        // identifiant existant gardé, sinon un neuf
        assert_eq!(clean_plot_item(&json!({ "id": "x", "kind": "text", "points": [[0, 0]] }), None).unwrap()["id"], "x");
        let fresh = clean_plot_item(&json!({ "kind": "text", "points": [[0, 0]] }), None).unwrap();
        assert!(fresh["id"].as_str().unwrap().starts_with('p'));
        assert_ne!(new_id("p"), new_id("p"));
    }

    #[test]
    fn ajout_modification_suppression() {
        let (plot, id) = add_item(None, &json!({ "kind": "line", "points": [[0, 0], [1, 1]], "style": { "stroke": "#ff0000" } })).unwrap();
        assert_eq!(plot["items"].as_array().unwrap().len(), 1);
        assert_eq!(plot["title"], json!({ "production": "", "author": "", "notes": "" }));
        let plot = update_item(Some(&plot), &id, &json!({ "style": { "width": 5 }, "text": "Câble" })).unwrap();
        let it = &plot["items"][0];
        assert_eq!(it["style"], json!({ "stroke": "#ff0000", "fill": "none", "width": 5, "dash": false, "size": 16 }));
        assert_eq!(it["text"], "Câble");
        assert!(update_item(Some(&plot), &id, &json!({ "points": [] })).is_none(), "devenu invalide : inchangé");
        assert!(update_item(Some(&plot), "nope", &json!({})).is_none());
        // identifiant demandé déjà pris : un neuf
        let (plot2, id2) = add_item(Some(&plot), &json!({ "id": id, "kind": "text", "points": [[0, 0]] })).unwrap();
        assert_ne!(id2, id);
        let plot3 = remove_item(Some(&plot2), &id).unwrap();
        assert_eq!(plot3["items"].as_array().unwrap().len(), 1);
        assert!(remove_item(Some(&plot3), &id).is_none());
    }

    #[test]
    fn image_de_fond_et_cartouche() {
        let p = update_meta(None, &json!({ "underlay": { "url": "/assets/a.png", "x": "1", "width": 0, "opacity": 3, "floor3d": 1 }, "background": "#ABCDEF", "title": { "production": "Pub", "author": 3 } }));
        assert_eq!(p["underlay"], json!({ "url": "/assets/a.png", "x": 1, "z": 0, "width": 0.1, "aspect": 1, "rotation": 0, "opacity": 1, "floor3d": true }));
        assert_eq!(p["background"], "#abcdef");
        assert_eq!(p["title"], json!({ "production": "Pub", "author": "", "notes": "" }));
        // adresse refusée : l'image ne change pas ; null la retire
        let q = update_meta(Some(&p), &json!({ "underlay": { "url": "http://x/a.png" } }));
        assert_eq!(q["underlay"], p["underlay"]);
        assert_eq!(update_meta(Some(&p), &json!({ "underlay": null }))["underlay"], Value::Null);
        assert_eq!(update_meta(Some(&p), &json!({ "background": "red" }))["background"], Value::Null);
        // calage : 2 m mesurés pour 4 m réels, le premier point reste en place
        let Some(EditOp::Meta(m)) = calibrate(Some(&p), [0.0, 0.0], [2.0, 0.0], 4.0) else { panic!("calage") };
        assert_eq!(m["underlay"]["width"], json!(0.2));
        assert_eq!(m["underlay"]["x"], json!(2));
        assert!(calibrate(Some(&p), [0.0, 0.0], [0.0, 0.0], 4.0).is_none());
        assert!(calibrate(Some(&p), [0.0, 0.0], [1.0, 0.0], 0.0).is_none());
    }

    #[test]
    fn nature_et_libelle_des_modifications() {
        let up = EditOp::UpdateItem { id: "p1".into(), changes: json!({ "style": { "stroke": "#fff", "fill": "#fff" }, "points": [] }) };
        assert_eq!(up.history_key().unwrap(), "plot.update|p1|points,style.fill,style.stroke");
        assert_eq!(EditOp::Meta(json!({ "background": "#000000" })).history_key().unwrap(), "plot.meta|background,title,underlay");
        assert_eq!(EditOp::Meta(json!({ "underlay": { "x": 1, "url": "u" } })).history_key().unwrap(), "plot.meta|background,title,underlay.url,underlay.x");
        assert_eq!(EditOp::Meta(json!({ "background": null })).label(), "la couleur du fond du plan");
        assert_eq!(EditOp::Meta(json!({ "title": {} })).label(), "le cartouche du plan");
        assert_eq!(EditOp::Meta(json!({ "underlay": null })).label(), "l’image de fond du plan");
        assert_eq!(EditOp::RemoveItem("x".into()).history_key(), None);
    }

    #[test]
    fn reglages_de_l_element() {
        let filled = json!({ "id": "a", "kind": "rect", "style": { "stroke": "#112233", "fill": "#112233" } });
        assert_eq!(item_changes(&filled, &ItemChange::Color("#ff0000".into())), json!({ "style": { "stroke": "#ff0000", "fill": "#ff0000" } }));
        let empty = json!({ "id": "b", "kind": "line", "style": { "fill": "none" } });
        assert_eq!(item_changes(&empty, &ItemChange::Color("#ff0000".into())), json!({ "style": { "stroke": "#ff0000" } }));
        assert_eq!(item_changes(&empty, &ItemChange::Filled(true)), json!({ "style": { "fill": ACCENT } }));
        assert_eq!(item_changes(&filled, &ItemChange::Filled(false)), json!({ "style": { "fill": "none" } }));
        assert_eq!(item_changes(&filled, &ItemChange::Height(-2.0)), json!({ "height": 0 }));
        assert_eq!(item_changes(&filled, &ItemChange::Height(2.5)), json!({ "height": 2.5 }));
        assert_eq!(color_value(&empty), ACCENT);
        assert_eq!(color_value(&filled), "#112233");
        assert_eq!(kind_name("polygon"), Some("Forme"));
        assert!(is_closed("freehand") && !is_closed("line"));
    }

    #[test]
    fn plan_nettoye_a_l_ouverture() {
        let p = clean_plot(Some(&json!({ "items": [{ "kind": "line", "points": [[0, 0]] }, { "kind": "nope" }], "background": "#FFF", "title": { "author": "Lou" }, "other": 1 })));
        assert_eq!(p["items"].as_array().unwrap().len(), 1);
        assert_eq!(p["background"], Value::Null);
        assert_eq!(p["title"], json!({ "production": "", "author": "Lou", "notes": "" }));
        assert_eq!(p["other"], 1);
        assert_eq!(p["underlay"], Value::Null);
        assert_eq!(clean_plot(None), empty_plot());
    }

    #[test]
    fn volumes_3d() {
        let mut p = Project::default();
        p.extra.insert(
            "plot".into(),
            json!({ "items": [
                { "id": "r", "kind": "rect", "points": [[0, 0], [2, 1]], "height": 3 },
                { "id": "e", "kind": "ellipse", "points": [[0, 0], [2, 2]], "height": 1 },
                { "id": "l", "kind": "line", "points": [[0, 0], [2, 2], [3, 3]], "height": 1 },
                { "id": "f", "kind": "polygon", "points": [[0, 0], [2, 2]], "height": 1 },
                { "id": "z", "kind": "polygon", "points": [[0, 0], [2, 2], [3, 0]], "height": 0 }
            ] }),
        );
        let v = plot_volumes(&p);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].outline, vec![[0.0, 0.0], [2.0, 0.0], [2.0, 1.0], [0.0, 1.0]]);
        assert_eq!(v[1].outline.len(), 40);
        assert!((v[1].outline[10][1] - 2.0).abs() < 1e-12);
    }
}
