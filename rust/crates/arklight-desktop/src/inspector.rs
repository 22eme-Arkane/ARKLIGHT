//! Inspecteur : réglages de l'élément choisi et modifications qu'ils envoient au projet. Portage
//! de l'inspecteur de la version JavaScript (`web/js/ui/inspector.js`) : en-tête (nom, dupliquer,
//! supprimer), face avant du projecteur (menus embarqués), position et support, accessoires, zoom,
//! patch DMX (CRMX, entrée active, signal), réglages du projecteur (CCT, HSI, RVB ou une tirette par
//! canal), réglages rapides des Astera, état simulé, données du modèle ; sélection multiple
//! (adressage à la suite, allumer, CRMX) ; objets (réglages du décor, toile, pieds, conseils).
//! Les champs sont génériques : la fenêtre Slint (ui/inspector.slint) les dessine selon leur genre.

use crate::tr;
use arklight_core::accessories::{accessories_for, effective};
use arklight_core::document::{Changes, Document, Edit};
use arklight_core::engine::Runtime;
use arklight_core::engine::console::console_held;
use arklight_core::engine::output::{Source, control_source, tungsten_cct};
use arklight_core::library::{FixtureType, library};
use arklight_core::project::{Fixture, SceneObject};
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// Un champ (voir `Field` dans ui/common.slint).
#[derive(Debug, Clone, Default)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: &'static str,
    pub text: String,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    /// libellés d'une liste de choix ou d'une rangée de boutons (ou les trois valeurs d'un x / y / z)
    pub options: Vec<String>,
    /// clés des boutons d'une rangée
    pub keys: Vec<String>,
    /// bouton enfoncé (rangée de boutons), choix (liste)
    pub index: i32,
    pub on: bool,
    /// bulle d'aide
    pub tip: String,
}

/// Ce que l'utilisateur a fait dans un champ.
pub enum Input {
    Text(String),
    Value(f32),
    Index(usize),
    Toggle(bool),
}

/// Champs de l'inspecteur et valeurs des listes de choix (clé → valeur de chaque choix).
pub struct Panel {
    pub title: String,
    pub fields: Vec<Field>,
    pub choices: HashMap<String, Vec<Value>>,
}

/// Ce que l'inspecteur montre de ce qui vit hors du projet : signal DMX, entrée active des Astera,
/// lampe, face avant, tirettes de la console.
#[derive(Default)]
pub struct Live {
    /// des données DMX sont arrivées sur l'univers du projecteur
    pub has_data: bool,
    /// elles arrivent en ce moment
    pub live: bool,
    /// consoles et applis qui envoient cet univers (« Luminair (Art-Net) »)
    pub sources: Vec<String>,
    pub runtime: Option<Runtime>,
    /// bas du projecteur (choix automatique du pied)
    pub bottom: Option<f64>,
    /// le projecteur a une face avant (menus embarqués) : montrée en tête de l'inspecteur
    pub front_panel: bool,
    /// une tirette par canal du profil : libellé, valeur, maximum, valeur lisible
    pub faders: Vec<(String, f32, f32, String)>,
}

/// Bouton de l'inspecteur : ce qu'il demande à la fenêtre.
pub enum Press {
    Edit(Edit),
    /// action de la fenêtre (« aim », « drop », « frame », « group », « number », « readdress-selection »…)
    Action(String),
    Duplicate,
    Delete,
    /// la console ARKLIGHT prend le projecteur (une tirette par canal)
    TakeConsole,
    ReleaseConsole,
    /// n'afficher que ce projecteur (liste de la sélection multiple)
    Select(String),
    /// adresser la sélection à la suite (univers, adresse saisis)
    AddressSequential,
    /// CRMX lié ou délié depuis l'inspecteur : l'entrée des Astera se recalcule
    Crmx(Edit, String),
}

// ------------------------------------------------------------------ champs

fn header(label: &str) -> Field {
    Field { kind: "header", label: label.to_owned(), ..Field::default() }
}

fn note(text: &str) -> Field {
    Field { kind: "info", text: text.to_owned(), ..Field::default() }
}

fn mono(text: &str) -> Field {
    Field { kind: "mono", text: text.to_owned(), ..Field::default() }
}

fn warn(text: &str) -> Field {
    Field { kind: "warn", text: text.to_owned(), ..Field::default() }
}

fn sub(text: &str) -> Field {
    Field { kind: "sub", text: text.to_owned(), ..Field::default() }
}

fn number(key: &str, label: &str, value: f64) -> Field {
    Field { kind: "number", key: key.into(), label: label.into(), text: fmt(value), ..Field::default() }
}

fn xyz(key: &str, label: &str, v: [f64; 3], decimals: usize) -> Field {
    Field { kind: "xyz", key: key.into(), label: label.into(), options: v.iter().map(|x| format!("{x:.decimals$}")).collect(), ..Field::default() }
}

fn slider(key: &str, label: &str, value: f64, (min, max, step): (f32, f32, f32), shown: String) -> Field {
    Field { kind: "slider", key: key.into(), label: label.into(), value: value as f32, min, max, step, text: shown, ..Field::default() }
}

fn toggle(key: &str, label: &str, on: bool) -> Field {
    Field { kind: "toggle", key: key.into(), label: label.into(), on, ..Field::default() }
}

/// Rangée de boutons : (clé, libellé) ; `active` : celui qui est enfoncé ; `primary` : le premier en orange.
fn buttons(list: &[(&str, &str)], active: Option<&str>, primary: bool) -> Field {
    Field {
        kind: "buttons",
        options: list.iter().map(|b| b.1.to_owned()).collect(),
        keys: list.iter().map(|b| b.0.to_owned()).collect(),
        index: active.and_then(|a| list.iter().position(|b| b.0 == a)).map_or(-1, |i| i as i32),
        on: primary,
        ..Field::default()
    }
}

/// Libellé à gauche, un bouton à droite (CRMX).
fn action(key: &str, label: &str, button: &str) -> Field {
    Field { kind: "action", key: key.into(), label: label.into(), text: button.into(), ..Field::default() }
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// « 12 400 » en français, « 12,400 » en anglais.
fn thousands(n: f64) -> String {
    let digits = format!("{}", n.round() as i64);
    let sep = tr("\u{202f}", ",");
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push_str(sep);
        }
        out.push(c);
    }
    out
}

fn tr_owned(fr: String, en: String) -> String {
    if tr("fr", "en") == "en" { en } else { fr }
}

/// Liste de choix : (valeur, libellé) ; l'entrée qui vaut `current` est choisie.
fn combo(panel: &mut Panel, key: &str, label: &str, choices: Vec<(Value, String)>, current: &Value) {
    let index = choices.iter().position(|(v, _)| v == current).unwrap_or(0) as i32;
    panel.fields.push(Field { kind: "combo", key: key.into(), label: label.into(), options: choices.iter().map(|(_, l)| l.clone()).collect(), index, ..Field::default() });
    panel.choices.insert(key.into(), choices.into_iter().map(|(v, _)| v).collect());
}

fn get<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    let mut cur = v;
    for k in path {
        cur = cur.get(*k).unwrap_or(&Value::Null);
    }
    cur
}

fn num(v: &Value, path: &[&str], d: f64) -> f64 {
    get(v, path).as_f64().unwrap_or(d)
}

fn settings_value(f: &Fixture) -> Value {
    serde_json::to_value(&f.settings).unwrap_or(Value::Null)
}

fn extra_str<'a>(t: &'a FixtureType, key: &str) -> Option<&'a str> {
    t.extra.get(key).and_then(Value::as_str)
}

/// Adresse Art-Net « net:subnet:univers » d'un univers ARKLIGHT (`artnetLabel`).
fn artnet_label(universe: u32, offset: u32) -> String {
    let Some(pa) = universe.checked_sub(offset) else { return "—".into() };
    format!("{}:{}:{}", pa >> 8, (pa >> 4) & 15, pa & 15)
}

/// Pied retenu pour un projecteur (`resolveSupport`) : libellé et avertissements.
fn support_info(t: &FixtureType, choice: &str, top: f64) -> (String, Vec<String>) {
    let lib = library();
    let def = lib.resolve_support(t, Some(choice), top);
    let Some(def) = def else {
        return (if choice == "hanging" { tr("Accroché : pas de pied dans la liste du matériel.", "Hanging: no stand in the equipment list.") } else { tr("Pas de pied.", "No stand.") }.to_owned(), Vec::new());
    };
    let s = |k: &str| def.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    let n = |k: &str| def.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let name = crate::i18n::t(&s("name"));
    let spec = tr_owned(
        format!("{name} — {}–{} m, {} kg maxi, spigot {}", fmt(n("minH")), fmt(n("maxH")), fmt(n("load")), s("spigot")),
        format!("{name} — {}–{} m, {} kg max, {} spigot", fmt(n("minH")), fmt(n("maxH")), fmt(n("load")), s("spigot")),
    );
    let what = if choice == "auto" { tr_owned(format!("Choisi : {spec}"), format!("Picked: {spec}")) } else { spec };
    let short = s("short");
    let w = t.weight();
    let mut warnings = Vec::new();
    if w > n("load") {
        warnings.push(tr_owned(format!("{} kg : plus que la charge du {short} ({} kg)", fmt(w), fmt(n("load"))), format!("{} kg: more than the {short} can carry ({} kg)", fmt(w), fmt(n("load")))));
    }
    if top > n("maxH") + 0.01 {
        warnings.push(tr_owned(format!("hauteur {top:.2} m : au-dessus du maximum du {short} ({} m)", fmt(n("maxH"))), format!("height {top:.2} m: above the {short} maximum ({} m)", fmt(n("maxH")))));
    } else if top < n("minH") - 0.01 {
        warnings.push(tr_owned(format!("hauteur {top:.2} m : en dessous du minimum du {short} ({} m)", fmt(n("minH"))), format!("height {top:.2} m: below the {short} minimum ({} m)", fmt(n("minH")))));
    }
    (what, warnings)
}

// ------------------------------------------------------------------ inspecteur

/// Inspecteur de la sélection : un projecteur, plusieurs, ou un objet.
pub fn panel(doc: &Document, selection: &[String], live: &Live) -> Panel {
    let mut p = Panel { title: String::new(), fields: Vec::new(), choices: HashMap::new() };
    let fixtures: Vec<&Fixture> = selection.iter().filter_map(|id| doc.fixture(id)).collect();
    if fixtures.len() > 1 {
        multi_panel(&mut p, doc, &fixtures);
    } else if let Some(f) = fixtures.first() {
        fixture_panel(&mut p, doc, f, live);
    } else if let Some(o) = selection.first().and_then(|id| doc.object(id)) {
        object_panel(&mut p, o);
    } else {
        p.title = tr("Aucun projecteur sélectionné", "No fixture selected").into();
        p.fields.push(note(tr("1. Glisse des projecteurs de la bibliothèque (à gauche) dans la vue 3D.", "1. Drag fixtures from the library (on the left) into the 3D view.")));
        p.fields.push(note(tr(
            "2. Place-les (W : déplacer, E : pivoter). Ctrl+clic pour en sélectionner plusieurs, clic droit pour les actions.",
            "2. Place them (W: move, E: rotate). Ctrl+click to select several, right-click for actions.",
        )));
        p.fields.push(note(tr(
            "3. Pilote-les : console de test (en bas), appli Art-Net / sACN, ou l’AsteraApp via la fausse AsteraBox.",
            "3. Control them: test console (at the bottom), Art-Net / sACN app, or the AsteraApp through the fake AsteraBox.",
        )));
    }
    p.fields = wrap_buttons(std::mem::take(&mut p.fields));
    p
}

/// Rangées de boutons trop larges pour l'inspecteur : coupées en plusieurs rangées (comme le
/// retour à la ligne de `.btn-row`).
fn wrap_buttons(fields: Vec<Field>) -> Vec<Field> {
    const BUDGET: f32 = 282.0;
    let width = |label: &str| label.chars().count() as f32 * 6.6 + 18.0;
    let mut out = Vec::new();
    for f in fields {
        if f.kind != "buttons" || f.options.iter().map(|l| width(l) + 6.0).sum::<f32>() <= BUDGET {
            out.push(f);
            continue;
        }
        let mut row = Field { options: Vec::new(), keys: Vec::new(), index: -1, ..f.clone() };
        let mut used = 0.0;
        for (i, (label, key)) in f.options.iter().zip(&f.keys).enumerate() {
            let w = width(label);
            if !row.options.is_empty() && used + 6.0 + w > BUDGET {
                out.push(std::mem::replace(&mut row, Field { options: Vec::new(), keys: Vec::new(), index: -1, on: false, ..f.clone() }));
                used = 0.0;
            }
            if i as i32 == f.index {
                row.index = row.options.len() as i32;
            }
            used += if row.options.is_empty() { w } else { 6.0 + w };
            row.options.push(label.clone());
            row.keys.push(key.clone());
        }
        out.push(row);
    }
    out
}

fn head(p: &mut Panel, name: &str, list: &[(&str, &str)]) {
    p.fields.push(Field { kind: "head", key: "name".into(), text: name.into(), options: list.iter().map(|b| b.1.to_owned()).collect(), keys: list.iter().map(|b| b.0.to_owned()).collect(), ..Field::default() });
}

fn fixture_panel(p: &mut Panel, doc: &Document, f: &Fixture, live: &Live) {
    let lib = library();
    let Some(t) = lib.fixture(&f.type_id) else { return };
    let s = settings_value(f);
    let st = &f.state;
    let v5 = t.is_astera_v5();
    let is_box = t.platform.as_deref() == Some("astera-box");
    let eff = effective(t, f);

    // ---- en-tête
    p.title = f.name.clone();
    head(p, &f.name, &[("dup", tr("Dupliquer (Ctrl+D)", "Duplicate (Ctrl+D)")), ("del", tr("Supprimer (Suppr)", "Delete (Del)"))]);
    let model = extra_str(t, "model").unwrap_or("");
    let mut line = if model.is_empty() { format!("{} {}", t.brand, t.name) } else { format!("{} {} ({model})", t.brand, t.name) };
    if t.pixels > 0 {
        line += &tr_owned(format!(" · {} pixel{}", t.pixels, if t.pixels > 1 { "s" } else { "" }), format!(" · {} pixel{}", t.pixels, if t.pixels > 1 { "s" } else { "" }));
    }
    if let Some(o) = t.output.as_ref().filter(|o| o.lux_at1m.is_some() && !o.estimated) {
        let _ = o;
        let lux = eff.lux_at1m.unwrap_or(0.0);
        line += &tr_owned(format!(" · {} lux à 1 m", thousands(lux)), format!(" · {} lux at 1 m", thousands(lux)));
        if t.extra.get("zoom").is_some_and(Value::is_object) {
            let z = num(&s, &["zoom"], t.beam_angle.unwrap_or(0.0));
            line += &format!(" (zoom {}°)", fmt(z));
        }
    }
    p.fields.push(sub(&line));

    // ---- panneau de commande embarqué (menus du projecteur)
    if live.front_panel {
        p.fields.push(Field { kind: "panel", ..Field::default() });
    }

    // ---- réglages du projecteur, sans passer par le menu simulé
    let held = console_held(f);
    let m = &st["manual"];
    let console_note = tr(
        "La console ARKLIGHT pilote ce projecteur canal par canal (mêmes valeurs que l’onglet Console, en bas). Un autre mode : il reprend ses propres réglages.",
        "The ARKLIGHT console drives this fixture channel by channel (same values as the Console tab, at the bottom). Another mode: it goes back to its own settings.",
    );
    let fader_rows = |p: &mut Panel| {
        for (i, (label, value, max, shown)) in live.faders.iter().enumerate() {
            p.fields.push(slider(&format!("ch.{i}"), label, *value as f64, (0.0, *max, 1.0), shown.clone()));
        }
    };
    let has_mode = t.mode(f.settings.dmx_mode.as_deref()).is_some();
    if t.is_traditional() && !v5 && !is_box {
        p.fields.push(header(tr("Réglages du projecteur", "Fixture settings")));
        let i = num(m, &["intensity"], 0.0);
        p.fields.push(slider("intensity", tr("Intensité", "Intensity"), i, (0.0, 1.0, 0.005), format!("{:.0} %", i * 100.0)));
        p.fields.push(note(tr("Comme un gradateur manuel, quand le projecteur n’est pas piloté en DMX.", "Like a manual dimmer, when the fixture is not driven over DMX.")));
    } else if !v5 && !is_box && t.pixels > 0 {
        p.fields.push(header(tr("Réglages du projecteur", "Fixture settings")));
        let mode = m["mode"].as_str().unwrap_or("cct");
        let active = if held || mode == "faders" { "faders" } else { mode };
        let mut list = vec![("mode:cct", "CCT"), ("mode:hsi", "HSI"), ("mode:rgb", tr("RVB", "RGB"))];
        if has_mode {
            list.push(("mode:faders", tr("Canaux", "Channels")));
        }
        p.fields.push(buttons(&list, Some(&format!("mode:{active}")), false));
        if active == "faders" && has_mode {
            fader_rows(p);
        } else {
            let i = num(m, &["intensity"], 0.0);
            p.fields.push(slider("intensity", tr("Intensité", "Intensity"), i, (0.0, 1.0, 0.005), format!("{:.0} %", i * 100.0)));
            match mode {
                "hsi" => {
                    let h = num(m, &["hue"], 0.0);
                    p.fields.push(slider("hue", tr("Teinte (°)", "Hue (°)"), h, (0.0, 360.0, 1.0), format!("{h:.0}°")));
                    let sat = num(m, &["sat"], 1.0);
                    p.fields.push(slider("sat", tr("Saturation", "Saturation"), sat, (0.0, 1.0, 0.01), format!("{:.0} %", sat * 100.0)));
                }
                "rgb" => {
                    for (i, (k, l)) in [("r", "R"), ("g", tr("V", "G")), ("b", tr("B", "B"))].iter().enumerate() {
                        let v = get(m, &["rgb"]).get(i).and_then(Value::as_f64).unwrap_or(1.0);
                        p.fields.push(slider(k, l, v, (0.0, 1.0, 0.005), format!("{:.0} %", v * 100.0)));
                    }
                }
                _ => {
                    let k = num(m, &["cct"], 5600.0);
                    p.fields.push(slider("cct", tr("Kelvin", "Kelvin"), k, (1750.0, 20000.0, 50.0), format!("{k:.0} K")));
                    let g = num(m, &["tint"], 0.0);
                    p.fields.push(slider("tint", tr("Teinte ±vert", "±Green tint"), g, (-1.0, 1.0, 0.01), format!("{g:+.2}")));
                }
            }
        }
        p.fields.push(note(if active == "faders" { console_note } else { tr("Sans passer par le menu. Utilisé quand le projecteur n’est pas piloté en DMX.", "Without going through the menu. Used when the fixture is not driven over DMX.") }));
    }
    if v5 && t.pixels > 0 {
        // Astera : intensité et couleur du mode autonome (STANDALONE)
        let sa = &s["standalone"];
        let c0 = get(sa, &["colors"]).get(0).cloned().unwrap_or(Value::Null);
        let hsi = c0["kind"].as_str() == Some("hsi") && num(&c0, &["sat"], 0.0) > 0.0;
        p.fields.push(header(tr("Réglages du projecteur", "Fixture settings")));
        let mut list = vec![("sa:cct", "CCT"), ("sa:hsi", "HSI")];
        if has_mode {
            list.push(("sa:faders", tr("Canaux", "Channels")));
        }
        let active = if held { "sa:faders" } else if hsi { "sa:hsi" } else { "sa:cct" };
        p.fields.push(buttons(&list, Some(active), false));
        if held && has_mode {
            fader_rows(p);
        } else {
            let k = num(sa, &["intensity"], 100.0);
            p.fields.push(slider("sa.intensity", tr("Intensité", "Intensity"), k, (0.0, 100.0, 1.0), format!("{k:.0} %")));
            if hsi {
                let h = num(&c0, &["hue"], 0.0);
                p.fields.push(slider("sa.hue", tr("Teinte (°)", "Hue (°)"), h, (0.0, 360.0, 1.0), format!("{h:.0}°")));
                let sat = num(&c0, &["sat"], 100.0);
                p.fields.push(slider("sa.sat", tr("Saturation", "Saturation"), sat, (1.0, 100.0, 1.0), format!("{sat:.0} %")));
            } else {
                let k = num(&c0, &["cct"], 5600.0);
                p.fields.push(slider("sa.cct", tr("Kelvin", "Kelvin"), k, (1750.0, 20000.0, 50.0), format!("{k:.0} K")));
                let g = num(&c0, &["green"], 0.0);
                p.fields.push(slider("sa.green", tr("Teinte ±vert", "±Green tint"), g, (-1.0, 1.0, 0.01), format!("{g:+.2}")));
            }
        }
        p.fields.push(note(if held {
            console_note
        } else {
            tr(
                "Sans passer par le menu : règle le mode autonome (STANDALONE, couleur fixe). Canaux : tous les canaux du profil DMX, comme depuis une console. Programmes et effets : menu du projecteur.",
                "Without going through the menu: sets the standalone mode (STANDALONE, fixed colour). Channels: every channel of the DMX profile, as from a console. Programs and effects: the fixture's menu.",
            )
        }));
    }

    // ---- patch DMX
    let universe = f.settings.universe.unwrap_or(1);
    let address = f.settings.dmx_address.unwrap_or(1);
    p.fields.push(header(if is_box { tr("Liaison CRMX", "CRMX link") } else { tr("Patch DMX", "DMX patch") }));
    if is_box {
        p.fields.push(number("universe", tr("Entrée DMX", "DMX input"), universe as f64));
        p.fields.push(note(tr(
            "Univers que la console / l’appli envoie dans la prise DMX de la box. En mode TX, la box le transmet en CRMX aux projecteurs liés par la touche LINK.",
            "Universe the console / app sends into the box's DMX socket. In TX mode the box passes it on over CRMX to the fixtures linked with the LINK key.",
        )));
    } else {
        let mut fix = number("fixtureNumber", tr("Fixture (Fix)", "Fixture (Fix)"), f.settings.fixture_number.unwrap_or(0) as f64);
        if f.settings.fixture_number.unwrap_or(0) == 0 {
            fix.text = String::new();
        }
        fix.tip = tr("Numéro du projecteur dans la console ou l’appli (Luminair, Blackout…) ; sur les Astera : réglage FIXTURE NUMBER", "Fixture number in the console or app (Luminair, Blackout…); on Asteras: FIXTURE NUMBER setting").into();
        p.fields.push(fix);
        p.fields.push(number("universe", if v5 { tr("Univers CRMX", "CRMX universe") } else { tr("Univers", "Universe") }, universe as f64));
        p.fields.push(number("dmxAddress", tr("Adresse", "Address"), address as f64));
        if !t.dmx.modes.is_empty() {
            let modes = t.dmx.modes.iter().map(|m| (Value::String(m.id.clone()), format!("{} — {} c.", if m.name.is_empty() { &m.id } else { &m.name }, m.footprint()))).collect();
            combo(p, "dmxMode", tr("Profil", "Profile"), modes, &s["dmxMode"]);
        }
        if v5 {
            let linked = s["crmxLinked"].as_bool().unwrap_or(false);
            let label = if linked { tr_owned(format!("Lié (univers {universe}) — délier"), format!("Linked (universe {universe}) — unlink")) } else { tr("Non lié — lier (touche LINK)", "Not linked — link (LINK key)").to_owned() };
            p.fields.push(action("crmx", "CRMX", &label));
            if t.platform.as_deref() != Some("astera-v5-button") {
                let wired = t
                    .extra
                    .get("controls")
                    .and_then(|c| c.get("wiredLabel"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| if t.category == "bar" { "XLR DMX IN".into() } else if model.starts_with("FP3") { tr("DataLink (filaire)", "DataLink (wired)").into() } else { tr("PowerBox (filaire)", "PowerBox (wired)").into() });
                p.fields.push(toggle("wired", &wired, s["wired"].as_bool().unwrap_or(false)));
            }
        } else {
            let sources = vec![
                (json!("auto"), tr("Auto (DMX si reçu, sinon manuel)", "Auto (DMX if received, else manual)").to_owned()),
                (json!("dmx"), tr("DMX uniquement", "DMX only").to_owned()),
                (json!("manual"), tr("Manuel uniquement", "Manual only").to_owned()),
            ];
            let cur = if s["source"].is_string() { s["source"].clone() } else { json!("auto") };
            combo(p, "source", tr("Commande", "Control"), sources, &cur);
        }
        // canaux, Art-Net, RDM ; problèmes de patch ; signal reçu
        let mode = t.mode(f.settings.dmx_mode.as_deref());
        let fp = mode.map_or(0, |m| m.footprint()) as u32;
        let mode_name = mode.map_or("—".to_owned(), |m| if m.name.is_empty() { m.id.clone() } else { m.name.clone() });
        let uid = arklight_net::rdm::Uid::new(arklight_net::rdm::PROTOTYPE_MANUFACTURER, arklight_net::rdm::device_id_of(&f.id)).text();
        p.fields.push(mono(&tr_owned(
            format!("Canaux {address}–{} ({fp} c., {mode_name}) · Art-Net {} · RDM {uid}", (address + fp).saturating_sub(1).max(address), artnet_label(universe, 1)),
            format!("Channels {address}–{} ({fp} ch., {mode_name}) · Art-Net {} · RDM {uid}", (address + fp).saturating_sub(1).max(address), artnet_label(universe, 1)),
        )));
        let issues = crate::scene_panel::issues(f, &doc.project.fixtures);
        if !issues.is_empty() {
            p.fields.push(warn(&format!("⚠ {}", issues.join(" · "))));
        }
        p.fields.push(note(&live_text(f, t, live)));
    }

    // ---- position
    p.fields.push(header(tr("Position", "Position")));
    p.fields.push(xyz("position", tr("X / Y / Z (m)", "X / Y / Z (m)"), f.position, 2));
    p.fields.push(xyz("rotation", tr("Rotation (°)", "Rotation (°)"), f.rotation.map(f64::to_degrees), 0));
    if t.geometry.kind == "tube" {
        p.fields.push(buttons(&[("stand-up", tr("Debout", "Upright")), ("lay-down", tr("Couché", "Lying")), ("drop", tr("Poser au sol", "Put on the floor"))], None, false));
    } else {
        p.fields.push(buttons(&[("aim", tr("Viser le personnage", "Aim at the actor")), ("drop", tr("Poser au sol", "Put on the floor"))], None, false));
    }
    let mut supports = vec![(json!("auto"), tr("Automatique (selon poids, hauteur, montage)", "Automatic (weight, height, mount)").to_owned())];
    supports.extend(lib.supports.iter().filter_map(|x| Some((x.get("id")?.clone(), crate::i18n::t(x.get("name")?.as_str()?)))));
    supports.push((json!("hanging"), tr("Accroché (perche, pince, grill)", "Hanging (pole, clamp, grid)").to_owned()));
    supports.push((json!("none"), tr("Aucun (posé au sol ou tenu à la main)", "None (on the floor or hand-held)").to_owned()));
    let choice = s["support"].as_str().filter(|c| !c.is_empty()).unwrap_or("auto").to_owned();
    combo(p, "support", tr("Support", "Support"), supports, &json!(choice));
    if let Some(bottom) = live.bottom {
        let (what, warnings) = support_info(t, &choice, bottom);
        p.fields.push(note(&what));
        for w in warnings {
            p.fields.push(warn(&format!("⚠ {w}")));
        }
    }

    // ---- accessoires : boîte à lumière, dôme, lanterne, louver, gélatine
    let allowed = accessories_for(t);
    if !allowed.modifiers.is_empty() || allowed.grids || allowed.gel {
        p.fields.push(header(tr("Accessoires", "Accessories")));
        let modifier = s["modifier"].as_str().and_then(|id| allowed.modifiers.iter().find(|m| m.id == id).copied());
        if !allowed.modifiers.is_empty() {
            let mut list = vec![(Value::Null, tr("Aucun", "None").to_owned())];
            list.extend(allowed.modifiers.iter().map(|m| (json!(m.id), if m.group.is_empty() { m.name.clone() } else { format!("{} › {}", crate::i18n::t(&m.group), m.name) })));
            combo(p, "modifier", tr("Boîte / dôme", "Softbox / dome"), list, &s["modifier"]);
        }
        if modifier.is_some_and(|m| m.kind != "lantern") {
            let mut fronts = vec![(Value::Null, tr("Toile d’origine", "Original front").to_owned())];
            fronts.extend(lib.filters.iter().filter(|x| x.group == "diffusion").map(|x| (json!(x.id), x.name.clone())));
            fronts.push((json!("none"), tr("Sans toile avant", "No front diffusion").to_owned()));
            combo(p, "front", tr("Toile avant", "Front diffusion"), fronts, &s["front"]);
        }
        if allowed.grids {
            let mut list = vec![(Value::Null, tr("Aucun", "None").to_owned())];
            list.extend(lib.grids.iter().map(|g| (json!(g.id), g.name.clone())));
            combo(p, "grid", tr("Louver / grille", "Louver / grid"), list, &s["grid"]);
        }
        if allowed.gel {
            let mut list = vec![(Value::Null, tr("Aucune", "None").to_owned())];
            list.extend(lib.filters.iter().map(|x| (json!(x.id), x.name.clone())));
            combo(p, "gel", if modifier.is_some() { tr("Gélatine (devant)", "Gel (front)") } else { tr("Gélatine", "Gel") }, list, &s["gel"]);
        }
        if eff.accessories.is_some() {
            let angle = eff.beam_angle.unwrap_or(t.beam_degrees());
            let mut line = tr_owned(format!("Ouverture {}°", angle.round()), format!("Beam {}°", angle.round()));
            if let Some(lux) = eff.lux_at1m {
                line += &tr_owned(format!(" · ≈ {} lux à 1 m", thousands(lux)), format!(" · ≈ {} lux at 1 m", thousands(lux)));
            }
            if let Some(mired) = s["gel"].as_str().and_then(|g| lib.filter(g)).and_then(|g| g.mired) {
                line += &format!(" · {}{} mireds", if mired > 0.0 { "+" } else { "" }, mired);
            }
            line += tr(" (valeurs approchées)", " (approximate values)");
            p.fields.push(note(&line));
        }
    }

    // ---- zoom mécanique (SolaBulb, Pluto, Leo)
    if let Some(z) = t.extra.get("zoom").filter(|z| z.is_object()) {
        let (min, max) = (num(z, &["min"], 10.0), num(z, &["max"], 60.0));
        let v = num(&s, &["zoom"], t.beam_angle.unwrap_or(min));
        let shown = if z.get("hotBeam").and_then(Value::as_f64) == Some(v) {
            "HotBeam".to_owned()
        } else if z.get("detent").and_then(Value::as_f64) == Some(v) {
            tr_owned(format!("{}° (cran)", fmt(v)), format!("{}° (detent)", fmt(v)))
        } else {
            format!("{}°", fmt(v))
        };
        p.fields.push(header(tr("Zoom (bague manuelle)", "Zoom (manual ring)")));
        p.fields.push(slider("zoom", &format!("{}° – {}°", fmt(min), fmt(max)), v, (min as f32, max as f32, 1.0), shown));
    }

    // ---- état simulé
    p.fields.push(header(tr("État simulé", "Simulated state")));
    p.fields.push(toggle("power", tr("Allumé", "On"), st["power"].as_bool().unwrap_or(true)));
    if t.platform.as_deref().is_some_and(|x| x.starts_with("astera")) {
        let b = num(st, &["battery"], 100.0);
        p.fields.push(slider("battery", tr("Batterie (%)", "Battery (%)"), b, (0.0, 100.0, 1.0), format!("{b:.0} %")));
        p.fields.push(toggle("charging", tr("Sur secteur", "On mains power"), st["charging"].as_bool().unwrap_or(false)));
    }
    if is_box {
        p.fields.push(toggle("dmxCable", tr("Câble DMX", "DMX cable"), st["dmxCable"].as_bool() != Some(false)));
    }
    if v5 {
        let mut b = buttons(&[("tap", tr("Demande « tap » de l’appli", "App “tap” request"))], None, false);
        b.tip = tr("Comme quand l’appli demande de « taper » les lampes (DMX Configuration, Sets)", "As when the app asks you to “tap” the lights (DMX Configuration, Sets)").into();
        p.fields.push(b);
    }

    // ---- origine des données (autres marques, tungstène, HMI)
    let source = t.extra.get("source");
    let estimated = t.output.as_ref().is_some_and(|o| o.estimated);
    if source.is_some() || estimated {
        p.fields.push(header(tr("Données du modèle", "Model data")));
        if source.and_then(|x| x.get("kind")).and_then(Value::as_str) == Some("ofl") {
            p.fields.push(note(tr("Profils DMX : Open Fixture Library.", "DMX profiles: Open Fixture Library.")));
        }
        if let Some(c) = extra_str(t, "confidence") {
            p.fields.push(note(&tr_owned(format!("Fiabilité : {c}."), format!("Reliability: {}.", crate::i18n::t(c)))));
        }
        if let Some(n) = source.and_then(|x| x.get("notes")).and_then(Value::as_str) {
            p.fields.push(note(&crate::i18n::t(n)));
        }
        if estimated {
            p.fields.push(note(tr("Sortie lumineuse estimée : à remplacer par la fiche photométrique ou une mesure.", "Estimated light output: to be replaced by the photometric sheet or a measurement.")));
        }
        if let Some(url) = source.and_then(|x| x.get("url")).and_then(Value::as_str) {
            p.fields.push(Field { kind: "link", text: url.into(), label: tr("Source", "Source").into(), ..Field::default() });
        }
    }
}

/// Ligne « signal » du patch (`refreshLive`) : DMX reçu, entrée active des Astera, lampe.
fn live_text(f: &Fixture, t: &FixtureType, live: &Live) -> String {
    let power = f.state["power"].as_bool().unwrap_or(true);
    if console_held(f) && power {
        return tr("Piloté par la console ARKLIGHT (onglet Console).", "Driven by the ARKLIGHT console (Console tab).").into();
    }
    let rt = live.runtime.clone().unwrap_or_default();
    let from = live.sources.join(", ");
    if t.is_astera_v5() {
        if !power {
            return tr("Projecteur éteint.", "Fixture off.").into();
        }
        let sel = f.settings.extra.get("inputSelect").and_then(Value::as_str).unwrap_or("AUTO");
        use arklight_core::engine::astera::Input;
        let name = match rt.input {
            None => "—",
            Some(Input::Detecting) => tr("aucune (détection en cours)", "none (detecting)"),
            Some(Input::Crmx) => tr("CRMX sans fil", "wireless CRMX"),
            Some(Input::Wired) => tr("DMX filaire", "wired DMX"),
            Some(Input::App) => tr("appli", "app"),
            Some(Input::Standalone) => tr("autonome", "standalone"),
            Some(Input::Art3) => "ART3",
            Some(Input::BlueMode) => "BlueMode",
            Some(Input::Standby) => tr("veille", "standby"),
            Some(Input::Off) => "—",
        };
        let mut out = tr_owned(format!("INPUT SELECT = {sel} · entrée active : {name}"), format!("INPUT SELECT = {sel} · active input: {name}"));
        if matches!(rt.input, Some(Input::Crmx | Input::Wired)) {
            out += &if rt.signal { tr_owned(format!(" — signal de {from}"), format!(" — signal from {from}")) } else { tr(" — pas de signal", " — no signal").to_owned() };
        }
        return out;
    }
    let src = control_source(f, live.has_data);
    if t.is_traditional() && power {
        // flux réel du filament / de la lampe et température de couleur qui en découle
        let flux = rt.flux.first().copied().unwrap_or(0.0);
        let nominal = t.lamp.as_ref().and_then(|l| l.cct).unwrap_or(3200.0);
        let tungsten = t.technology.as_deref() == Some("tungsten");
        let kelvin = if tungsten { (tungsten_cct(nominal, flux) / 10.0).round() * 10.0 } else { nominal };
        let lamp = if flux > 0.001 {
            let mut l = tr_owned(format!("lampe à {:.0} %", flux * 100.0), format!("lamp at {:.0} %", flux * 100.0));
            if tungsten {
                l += &format!(" · ≈ {kelvin:.0} K");
            }
            l
        } else {
            tr("lampe éteinte", "lamp off").to_owned()
        };
        let how = if src == Source::Dmx { if live.live { tr("Piloté en DMX", "Driven over DMX") } else { tr("DMX (dernier état conservé)", "DMX (last state held)") } } else { tr("Commande manuelle", "Manual control") };
        return format!("{how} — {lamp}.");
    }
    if !power {
        return tr("Projecteur éteint.", "Fixture off.").into();
    }
    if src == Source::Dmx {
        if live.live {
            tr_owned(format!("Piloté en DMX — signal reçu de {from}"), format!("Driven over DMX — signal from {from}"))
        } else if live.has_data {
            tr("Piloté en DMX — plus de signal, dernier état conservé.", "Driven over DMX — no more signal, last state held.").into()
        } else {
            tr("DMX uniquement — aucun signal reçu sur cet univers.", "DMX only — no signal received on this universe.").into()
        }
    } else if live.has_data {
        tr("Commande manuelle (DMX ignoré).", "Manual control (DMX ignored).").into()
    } else {
        tr_owned(format!("Commande manuelle — aucun signal DMX sur l’univers {}.", f.settings.universe.unwrap_or(1)), format!("Manual control — no DMX signal on universe {}.", f.settings.universe.unwrap_or(1)))
    }
}

/// Sélection multiple : réglages communs (patch à la suite, profil, état, position).
fn multi_panel(p: &mut Panel, doc: &Document, list: &[&Fixture]) {
    let lib = library();
    let mut types: Vec<&str> = list.iter().map(|f| f.type_id.as_str()).collect();
    types.dedup();
    types.sort();
    types.dedup();
    let same = (types.len() == 1).then(|| lib.fixture(types[0])).flatten();
    p.title = tr_owned(format!("{} projecteurs sélectionnés", list.len()), format!("{} fixtures selected", list.len()));
    p.fields.push(Field {
        kind: "head-title",
        text: p.title.clone(),
        options: vec![tr("Grouper la sélection (Ctrl+G)", "Group the selection (Ctrl+G)").into(), tr("Dupliquer (Ctrl+D)", "Duplicate (Ctrl+D)").into(), tr("Supprimer (Suppr)", "Delete (Del)").into()],
        keys: vec!["group".into(), "dup".into(), "del".into()],
        ..Field::default()
    });
    p.fields.push(sub(&match same {
        Some(t) => format!("{} {} × {}", t.brand, t.name, list.len()),
        None => tr_owned(format!("{} modèles différents", types.len()), format!("{} different models", types.len())),
    }));

    // ---- la liste : un clic n'affiche que ce projecteur
    p.fields.push(header(tr("Projecteurs", "Fixtures")));
    let all = &doc.project.fixtures;
    let mut conflicts = 0;
    for f in list {
        let issues = crate::scene_panel::issues(f, all);
        if !issues.is_empty() {
            conflicts += 1;
        }
        p.fields.push(Field {
            kind: "item",
            key: f.id.clone(),
            label: f.name.clone(),
            text: format!("{}{}", if issues.is_empty() { "" } else { "⚠ " }, arklight_plot::catalog::patch_label(f)),
            on: !issues.is_empty(),
            tip: issues.join("\n"),
            ..Field::default()
        });
    }
    p.fields.push(note(tr("Ctrl+clic : ajouter / retirer · clic droit : actions", "Ctrl+click: add / remove · right-click: actions")));

    // ---- patch : adressage à la suite à partir d'une adresse de départ
    p.fields.push(header(tr("Patch DMX", "DMX patch")));
    let first = list[0];
    let u = first.settings.universe.unwrap_or(1);
    let a = list.iter().filter(|f| f.settings.universe.unwrap_or(1) == u).filter_map(|f| f.settings.dmx_address).min().unwrap_or(1);
    p.fields.push(number("m.universe", tr("Univers", "Universe"), u as f64));
    p.fields.push(number("m.address", tr("Adresse de départ", "Start address"), a as f64));
    if let Some(t) = same.filter(|t| !t.dmx.modes.is_empty()) {
        let mut modes: Vec<Option<&str>> = list.iter().map(|f| f.settings.dmx_mode.as_deref()).collect();
        modes.dedup();
        let common = (modes.len() == 1).then(|| modes[0]).flatten();
        let mut choices: Vec<(Value, String)> = Vec::new();
        if common.is_none() {
            choices.push((Value::Null, tr("— profils différents —", "— different profiles —").into()));
        }
        choices.extend(t.dmx.modes.iter().map(|m| (Value::String(m.id.clone()), format!("{} — {} c.", if m.name.is_empty() { &m.id } else { &m.name }, m.footprint()))));
        combo(p, "m.dmxMode", tr("Profil", "Profile"), choices, &common.map_or(Value::Null, |c| json!(c)));
    }
    p.fields.push(buttons(&[("address", tr("Adresser à la suite", "Address one after another")), ("readdress", tr("Options…", "Options…")), ("number", tr("Numéroter (Fix)…", "Number (Fix)…"))], None, true));
    if conflicts > 0 {
        p.fields.push(warn(&tr_owned(format!("⚠ {conflicts} projecteur(s) de la sélection en conflit de patch"), format!("⚠ {conflicts} selected fixture(s) with a patch conflict"))));
    }

    // ---- état
    p.fields.push(header(tr("État simulé", "Simulated state")));
    let mut list_b = vec![("on", tr("Allumer", "Turn on")), ("off", tr("Éteindre", "Turn off"))];
    if list.iter().any(|f| lib.fixture(&f.type_id).is_some_and(|t| t.is_astera_v5())) {
        list_b.push(("crmx-link", tr("Lier CRMX", "Link CRMX")));
        list_b.push(("crmx-unlink", tr("Délier CRMX", "Unlink CRMX")));
    }
    let held = list.iter().filter(|f| console_held(f)).count();
    let release = tr_owned(format!("Relâcher la console ({held})"), format!("Release the console ({held})"));
    if held > 0 {
        list_b.push(("release", &release));
    }
    p.fields.push(buttons(&list_b, None, false));

    // ---- position
    p.fields.push(header(tr("Position", "Position")));
    p.fields.push(note(tr("La barre d’axes de la vue 3D déplace toute la sélection.", "The axis handles in the 3D view move the whole selection.")));
    p.fields.push(buttons(&[("aim", tr("Viser le personnage", "Aim at the actor")), ("drop", tr("Poser au sol", "Put on the floor")), ("frame", tr("Cadrer", "Frame"))], None, false));
}

/// Objet choisi (grip ou décor) : nom, réglages (toile, pose, couleurs…), position, conseils.
fn object_panel(p: &mut Panel, o: &SceneObject) {
    let lib = library();
    let props = &o.props;
    p.title = o.name.clone();
    head(p, &o.name, &[("dup", tr("Dupliquer (Ctrl+D)", "Duplicate (Ctrl+D)")), ("del", tr("Supprimer (Suppr)", "Delete (Del)"))]);
    let prop = lib.prop(&o.type_id);
    let grip = lib.grip(&o.type_id);
    match (prop, grip) {
        (Some(t), _) => p.fields.push(sub(&format!("{} · {} · {}", tr("Décor", "Set"), crate::library_panel::category_label(t.category.as_deref().unwrap_or("")), t.name))),
        (_, Some(g)) => p.fields.push(sub(&format!("Grip · {}", g.name))),
        _ => {}
    }
    p.fields.push(header(if prop.is_some() { tr("Réglages", "Settings") } else { tr("Élément", "Item") }));
    // décor : pose, taille, couleurs… selon le type
    if let Some(t) = prop {
        for prm in t.extra.get("params").and_then(Value::as_array).cloned().unwrap_or_default() {
            let (Some(key), Some(label)) = (prm["key"].as_str(), prm["label"].as_str()) else { continue };
            let label = crate::i18n::t(label);
            let k = format!("p.{key}");
            match prm["type"].as_str() {
                Some("number") => {
                    let v = props[key].as_f64().or(prm["default"].as_f64()).unwrap_or(0.0);
                    p.fields.push(number(&k, &label, v));
                }
                Some("select") => {
                    let list = prm["options"].as_array().map(|a| a.iter().filter_map(|o| Some((o.get(0)?.clone(), crate::i18n::t(o.get(1)?.as_str()?)))).collect()).unwrap_or_default();
                    let cur = if props[key].is_null() { prm["default"].clone() } else { props[key].clone() };
                    combo(p, &k, &label, list, &cur);
                }
                _ => {
                    let v = props[key].as_str().or(prm["default"].as_str()).unwrap_or("#ffffff");
                    p.fields.push(Field { kind: "color", key: k, label, text: v.into(), ..Field::default() });
                }
            }
        }
    }
    let mut stand_note = None;
    if let Some(g) = grip {
        if let Some(cloths) = g.extra.get("cloths").and_then(Value::as_array) {
            let list = cloths
                .iter()
                .filter_map(|c| {
                    let id = c.as_str()?;
                    let name = lib.cloths.get(id).map(|x| x.name.clone()).or_else(|| lib.filter(id).map(|f| f.name.clone()))?;
                    Some((c.clone(), crate::i18n::t(&name)))
                })
                .collect();
            let cur = if props["cloth"].is_null() { g.extra.get("defaultCloth").cloned().unwrap_or(Value::Null) } else { props["cloth"].clone() };
            combo(p, "cloth", if g.family.as_deref() == Some("frame") { tr("Toile", "Cloth") } else { tr("Garniture", "Fill") }, list, &cur);
            // ce qu'elle fait à la lumière (valeurs indicatives)
            if let Some(c) = cur.as_str().and_then(|id| lib.cloths.get(id)) {
                let stops = c.stops.map(|s| format!("{s:.1}").replace('.', tr(",", ".")));
                let text = match (stops, c.reflect > 0.05) {
                    (Some(s), true) => tr_owned(format!("Laisse passer : −{s} diaph. Renvoie environ {:.0} %.", c.reflect * 100.0), format!("Lets through: −{s} stops. Bounces about {:.0}%.", c.reflect * 100.0)),
                    (Some(s), false) => tr_owned(format!("Laisse passer : −{s} diaph."), format!("Lets through: −{s} stops.")),
                    (None, true) => tr_owned(format!("Opaque. Renvoie environ {:.0} %.", c.reflect * 100.0), format!("Opaque. Bounces about {:.0}%.", c.reflect * 100.0)),
                    (None, false) => String::new(),
                };
                if !text.is_empty() {
                    p.fields.push(note(&text));
                }
            }
        }
        let stands = g.extra.get("stands");
        let count = stands.and_then(|s| s.get("count")).and_then(Value::as_u64).unwrap_or(1);
        let default = stands.and_then(|s| s.get("id")).and_then(Value::as_str).unwrap_or("");
        let current = props["stand"].as_str().unwrap_or(default).to_owned();
        let support = |id: &str| lib.supports.iter().find(|s| s.get("id").and_then(Value::as_str) == Some(id));
        let name = |id: &str| support(id).and_then(|s| s.get("name")).and_then(Value::as_str).map(crate::i18n::t).unwrap_or_else(|| id.to_owned());
        if let Some(choices) = g.extra.get("standChoices").and_then(Value::as_array) {
            let list = choices
                .iter()
                .filter_map(Value::as_str)
                .map(|id| (if id == default { Value::Null } else { json!(id) }, format!("{count} × {}{}", name(id), if id == default { tr(" (conseillé)", " (recommended)") } else { "" })))
                .collect();
            combo(p, "stand", tr("Pieds", "Stands"), list, &if current == default { Value::Null } else { json!(current) });
        }
        if !current.is_empty() {
            stand_note = Some((count, name(&current), support(&current).cloned()));
        }
        if g.family.as_deref() == Some("stand") {
            let h = props["height"].as_f64().or(g.h).unwrap_or(2.0);
            p.fields.push(number("p.height", tr("Hauteur (m)", "Height (m)"), h));
        }
    }
    p.fields.push(xyz("position", tr("X / Y / Z (m)", "X / Y / Z (m)"), o.position, 2));
    p.fields.push(xyz("rotation", tr("Rotation (°)", "Rotation (°)"), o.rotation.map(f64::to_degrees), 0));
    p.fields.push(buttons(&[("drop", tr("Poser au sol", "Put on the floor"))], None, false));

    // ---- conseils
    p.fields.push(header(if prop.is_some() { tr("Bon à savoir", "Good to know") } else { tr("Liste du matériel", "Equipment list") }));
    if let Some(t) = prop {
        p.fields.push(note(tr("Décor : ne compte pas dans la liste du matériel. Il reçoit la lumière et fait de l’ombre.", "Set dressing: not counted in the equipment list. It receives light and casts shadows.")));
        if t.extra.get("aimable").and_then(Value::as_bool) == Some(true) {
            p.fields.push(note(tr("Les projecteurs peuvent le viser : clic droit sur un projecteur › Viser.", "Fixtures can aim at it: right-click a fixture › Aim.")));
        }
    } else if let Some(g) = grip {
        if let Some((count, name, def)) = stand_note {
            p.fields.push(note(&tr_owned(format!("Support compté dans la liste du matériel : {count} × {name}."), format!("Support counted in the equipment list: {count} × {name}."))));
            // cadre : les oreilles (milieu des côtés) sont à la hauteur de son centre
            if let Some(def) = def.filter(|_| g.family.as_deref() == Some("frame")) {
                let y = o.position[1];
                let (min, max) = (def.get("minH").and_then(Value::as_f64).unwrap_or(0.0), def.get("maxH").and_then(Value::as_f64).unwrap_or(99.0));
                let short = def.get("short").and_then(Value::as_str).unwrap_or("");
                let m = |v: f64| tr_owned(format!("{} m", format!("{v:.2}").replace('.', ",")), format!("{v:.2} m"));
                if y > max + 0.01 {
                    p.fields.push(warn(&tr_owned(format!("⚠ Oreilles du cadre à {} : au-dessus du maximum du {short} ({}). Prends des Long John Silver ou baisse le cadre.", m(y), m(max)), format!("⚠ Frame ears at {}: above the {short} maximum ({}). Use Long John Silvers or lower the frame.", m(y), m(max)))));
                } else if y < min - 0.01 {
                    p.fields.push(warn(&tr_owned(format!("⚠ Oreilles du cadre à {} : en dessous du minimum du {short} ({}).", m(y), m(min)), format!("⚠ Frame ears at {}: below the {short} minimum ({}).", m(y), m(min)))));
                }
            }
        }
        if let Some(n) = g.extra.get("sandbags").and_then(|v| v.as_u64().map(|n| n.to_string()).or_else(|| v.as_str().map(str::to_owned))) {
            p.fields.push(note(&tr_owned(format!("Sacs de sable conseillés : {n}."), format!("Recommended sandbags: {n}."))));
        }
        if g.extra.get("cloths").is_some() {
            p.fields.push(note(tr(
                "La toile agit sur la lumière : noir et réflecteurs coupent et font de l’ombre, soies, grids et frosts diffusent, muslins et magic cloth diffusent et renvoient, tulles atténuent.",
                "The cloth acts on the light: black and reflectors cut and cast shadows, silks, grids and frosts diffuse, muslins and magic cloth diffuse and bounce, nets reduce.",
            )));
        }
    }
}

// ------------------------------------------------------------------ gestes

fn parse(t: &str) -> Option<f64> {
    t.trim().replace(',', ".").parse().ok()
}

fn map(pairs: &[(&str, Value)]) -> Map<String, Value> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect()
}

/// « Le dernier geste l'emporte sur la console » (`releaseOnLookChange`) : changer le réglage
/// manuel ou le mode autonome d'un projecteur que la console pilote le lui rend.
fn release_on_look(f: &Fixture, mut changes: Changes) -> Changes {
    if !console_held(f) {
        return changes;
    }
    let look = changes.state.as_ref().is_some_and(|s| s.contains_key("manual") || s.contains_key("app")) || changes.settings.as_ref().is_some_and(|s| s.contains_key("standalone"));
    if look && !changes.state.as_ref().is_some_and(|s| s.contains_key("console")) {
        changes.state.get_or_insert_with(Map::new).insert("console".into(), Value::Null);
    }
    changes
}

/// Mode autonome d'un Astera avec la couleur 1 remplacée (programme couleur fixe).
fn standalone_color(f: &Fixture, spec: Value) -> Changes {
    let s = settings_value(f);
    let mut sa = s["standalone"].as_object().cloned().unwrap_or_default();
    let mut colors = sa.get("colors").and_then(Value::as_array).cloned().unwrap_or_default();
    if colors.is_empty() {
        colors.push(spec);
    } else {
        colors[0] = spec;
    }
    sa.insert("program".into(), json!(0));
    sa.insert("colors".into(), Value::Array(colors));
    Changes { settings: Some(map(&[("standalone", Value::Object(sa))])), ..Changes::default() }
}

/// Couleur 1 du mode autonome : (teinte, saturation, Kelvin, ±vert).
fn standalone_c0(f: &Fixture) -> (f64, f64, f64, f64) {
    let s = settings_value(f);
    let c0 = get(&s, &["standalone", "colors"]).get(0).cloned().unwrap_or(Value::Null);
    (num(&c0, &["hue"], 0.0), num(&c0, &["sat"], 100.0), num(&c0, &["cct"], 5600.0), num(&c0, &["green"], 0.0))
}

/// Un bouton de l'inspecteur.
pub fn press(doc: &Document, selection: &[String], key: &str) -> Option<Press> {
    let first = selection.first()?;
    let fixtures: Vec<&Fixture> = selection.iter().filter_map(|id| doc.fixture(id)).collect();
    let ids: Vec<String> = fixtures.iter().map(|f| f.id.clone()).collect();
    let state_all = |pairs: &[(&str, Value)]| Edit::UpdateFixtures(ids.iter().map(|id| (id.clone(), Changes { state: Some(map(pairs)), ..Changes::default() })).collect());
    Some(match key {
        "dup" => Press::Duplicate,
        "del" => Press::Delete,
        "group" | "aim" | "drop" | "frame" | "number" | "crmx-link" | "crmx-unlink" => Press::Action(key.to_owned()),
        "readdress" => Press::Action("readdress-selection".into()),
        "address" => Press::AddressSequential,
        "release" => Press::ReleaseConsole,
        "on" => Press::Edit(state_all(&[("power", json!(true))])),
        "off" => Press::Edit(state_all(&[("power", json!(false))])),
        "tap" => Press::Edit(state_all(&[("acceptConfig", json!(true))])),
        "mode:faders" | "sa:faders" => Press::TakeConsole,
        _ if key.starts_with("select:") => Press::Select(key[7..].to_owned()),
        _ => {
            let f = doc.fixture(first)?;
            let one = |changes: Changes| Press::Edit(Edit::UpdateFixture { id: f.id.clone(), changes: release_on_look(f, changes) });
            match key {
                "stand-up" | "lay-down" => {
                    let z = if key == "lay-down" { std::f64::consts::FRAC_PI_2 } else { 0.0 };
                    Press::Edit(Edit::UpdateFixture { id: f.id.clone(), changes: Changes { rotation: Some([0.0, f.rotation[1], z]), ..Changes::default() } })
                }
                "crmx" => {
                    let linked = !settings_value(f)["crmxLinked"].as_bool().unwrap_or(false);
                    Press::Crmx(Edit::UpdateFixture { id: f.id.clone(), changes: Changes { settings: Some(map(&[("crmxLinked", json!(linked))])), ..Changes::default() } }, f.id.clone())
                }
                "mode:cct" | "mode:hsi" | "mode:rgb" => {
                    let mut state = map(&[("manual", Value::Object(map(&[("mode", json!(&key[5..]))])))]);
                    state.insert("console".into(), Value::Null);
                    one(Changes { state: Some(state), ..Changes::default() })
                }
                "sa:cct" => {
                    let (_, _, cct, green) = standalone_c0(f);
                    one(standalone_color(f, json!({ "kind": "hsi", "hue": 0, "sat": 0, "cct": cct, "green": green })))
                }
                "sa:hsi" => {
                    let (hue, sat, _, _) = standalone_c0(f);
                    one(standalone_color(f, json!({ "kind": "hsi", "hue": hue, "sat": if sat > 0.0 { sat } else { 100.0 }, "cct": 5600 })))
                }
                _ => return None,
            }
        }
    })
}

/// Modification que produit un geste dans un champ ; None : rien à faire.
pub fn apply(doc: &Document, selection: &[String], choices: &HashMap<String, Vec<Value>>, key: &str, input: Input) -> Option<Edit> {
    let first = selection.first()?;
    let value = match &input {
        Input::Text(t) => Value::String(t.clone()),
        Input::Value(v) => json!(*v as f64),
        Input::Index(i) => choices.get(key).and_then(|c| c.get(*i)).cloned().unwrap_or(Value::Null),
        Input::Toggle(b) => Value::Bool(*b),
    };
    let fixtures: Vec<&Fixture> = selection.iter().filter_map(|id| doc.fixture(id)).collect();
    let objects: Vec<&SceneObject> = selection.iter().filter_map(|id| doc.object(id)).collect();

    // nom, position, rotation : l'élément choisi en premier
    if key == "name" {
        let changes = Changes { name: Some(value.as_str()?.to_owned()), ..Changes::default() };
        return Some(if doc.fixture(first).is_some() { Edit::UpdateFixture { id: first.clone(), changes } } else { Edit::UpdateObject { id: first.clone(), changes } });
    }
    if let Some(axis) = key.strip_prefix("position.").or_else(|| key.strip_prefix("rotation.")) {
        let i: usize = axis.parse().ok()?;
        let v = parse(value.as_str()?)?;
        let rotation = key.starts_with("rotation");
        let (mut pos, mut rot) = match (doc.fixture(first), doc.object(first)) {
            (Some(f), _) => (f.position, f.rotation),
            (_, Some(o)) => (o.position, o.rotation),
            _ => return None,
        };
        if rotation {
            rot[i] = v.to_radians();
        } else {
            pos[i] = v;
        }
        let changes = Changes { position: Some(pos), rotation: Some(rot), ..Changes::default() };
        return Some(if doc.fixture(first).is_some() { Edit::UpdateFixture { id: first.clone(), changes } } else { Edit::UpdateObject { id: first.clone(), changes } });
    }

    // réglages d'objets : tous les objets choisis du même type
    if !objects.is_empty() && doc.fixture(first).is_none() {
        let o0 = objects[0];
        let prop_key = key.strip_prefix("p.").unwrap_or(key);
        let v = match (&input, value.clone()) {
            (Input::Text(t), _) if key.starts_with("p.") => parse(t).map(|n| json!(n)).unwrap_or_else(|| json!(t.trim())),
            (_, v) => v,
        };
        let edits = objects.iter().filter(|o| o.type_id == o0.type_id).map(|o| Edit::UpdateObject { id: o.id.clone(), changes: Changes { props: Some(map(&[(prop_key, v.clone())])), ..Changes::default() } }).collect();
        return Some(Edit::Batch(edits));
    }

    // sélection multiple : profil commun
    if key == "m.dmxMode" {
        let mode = value.as_str()?;
        return Some(Edit::UpdateFixtures(fixtures.iter().map(|f| (f.id.clone(), Changes { settings: Some(map(&[("dmxMode", json!(mode))])), ..Changes::default() })).collect()));
    }

    // réglages des projecteurs : tous les projecteurs choisis
    let mut edits = Vec::new();
    for f in fixtures {
        let st = &f.state;
        let changes = match key {
            "universe" | "dmxAddress" | "fixtureNumber" => {
                let t = value.as_str()?.trim();
                if key == "fixtureNumber" && t.is_empty() {
                    Changes { settings: Some(map(&[(key, Value::Null)])), ..Changes::default() }
                } else {
                    let n = parse(t)?.round();
                    let v = match key {
                        "dmxAddress" => json!(n.clamp(1.0, 512.0) as u64),
                        "universe" => json!(n.clamp(1.0, 63999.0) as u64),
                        _ if n >= 1.0 => json!(n.min(9999.0) as u64),
                        _ => Value::Null,
                    };
                    Changes { settings: Some(map(&[(key, v)])), ..Changes::default() }
                }
            }
            "dmxMode" | "source" | "support" | "modifier" | "grid" | "gel" | "zoom" | "wired" => {
                let mut s = map(&[(key, value.clone())]);
                // plus de boîte : plus de toile avant
                if key == "modifier" && value.is_null() {
                    s.insert("front".into(), Value::Null);
                }
                Changes { settings: Some(s), ..Changes::default() }
            }
            "front" => Changes { settings: Some(map(&[("front", value.clone())])), ..Changes::default() },
            "sa.intensity" => Changes { settings: Some(map(&[("standalone", Value::Object(map(&[("intensity", json!(value.as_f64()?.round()))])))])), ..Changes::default() },
            "sa.hue" | "sa.sat" => {
                let (mut hue, mut sat, _, _) = standalone_c0(f);
                if key == "sa.hue" {
                    hue = value.as_f64()?.round();
                } else {
                    sat = value.as_f64()?.round();
                }
                standalone_color(f, json!({ "kind": "hsi", "hue": hue, "sat": sat, "cct": 5600 }))
            }
            "sa.cct" | "sa.green" => {
                let (_, _, mut cct, mut green) = standalone_c0(f);
                if key == "sa.cct" {
                    cct = value.as_f64()?.round();
                } else {
                    green = (value.as_f64()? * 100.0).round() / 100.0;
                }
                standalone_color(f, json!({ "kind": "hsi", "hue": 0, "sat": 0, "cct": cct, "green": green }))
            }
            "power" | "charging" | "dmxCable" => Changes { state: Some(map(&[(key, value.clone())])), ..Changes::default() },
            "battery" => Changes { state: Some(map(&[("battery", json!(value.as_f64()?.round()))])), ..Changes::default() },
            "intensity" | "cct" | "tint" | "hue" | "sat" => Changes { state: Some(map(&[("manual", Value::Object(map(&[(key, value.clone())])))])), ..Changes::default() },
            "r" | "g" | "b" => {
                let mut rgb: Vec<f64> = (0..3).map(|i| get(st, &["manual", "rgb"]).get(i).and_then(Value::as_f64).unwrap_or(1.0)).collect();
                rgb["rgb".find(key)?] = value.as_f64()?;
                Changes { state: Some(map(&[("manual", Value::Object(map(&[("rgb", json!(rgb))])))])), ..Changes::default() }
            }
            _ => return None,
        };
        edits.push((f.id.clone(), release_on_look(f, changes)));
    }
    Some(Edit::UpdateFixtures(edits))
}
