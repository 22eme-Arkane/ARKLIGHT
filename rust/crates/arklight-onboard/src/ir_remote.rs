//! Télécommandes infrarouges Astera : ARC1 (« FX Remote », 28 touches) et ARC3 (« White Remote »,
//! 26 touches), portage de `shared/onboard/ir-remote.js`.
//! Règles d'acceptation et fonctions : docs/recherche/menus/box_remotes.md.

use crate::fixture::{AsteraRuntime, Outcome, obj};
use crate::js::{self, jv};
use arklight_core::engine::Runtime;
use arklight_core::engine::astera::Input;
use arklight_core::library::FixtureType;
use arklight_core::project::Fixture;
use serde_json::{Map, Value, json};

/// Plan des touches de l'ARC1 (4 colonnes × 7 rangées), libellés tels qu'imprimés.
pub const ARC1_KEYS: [[Option<&str>; 4]; 7] = [
    [Some("RAINBOW"), Some("PASTEL"), Some("GARDEN"), Some("POWER ON")],
    [Some("DISCO"), Some("FIRE"), Some("ICE"), Some("POWER OFF")],
    [Some("RED"), Some("WARM WHITE"), Some("SPEED4"), Some("FADE ON")],
    [Some("ORANGE"), Some("COLD WHITE"), Some("SPEED3"), Some("FADE OFF")],
    [Some("YELLOW"), Some("PINK"), Some("SPEED2"), Some("RUN5")],
    [Some("GREEN"), Some("MAGENTA"), Some("SPEED1"), Some("RUN8")],
    [Some("CYAN"), Some("BLUE"), Some("STOP"), Some("RUN12")],
];

/// Plan des touches de l'ARC3 (None : emplacement vide).
pub const ARC3_KEYS: [[Option<&str>; 4]; 7] = [
    [Some("2400 K"), Some("5000 K"), Some("10%"), Some("POWER ON")],
    [Some("2700 K"), Some("5500 K"), Some("20%"), Some("POWER OFF")],
    [Some("3000 K"), Some("6000 K"), Some("40%"), None],
    [Some("3200 K"), Some("6500 K"), Some("60%"), Some("BLUE MODE 1/2")],
    [Some("3500 K"), Some("10000 K"), Some("100%"), Some("BLUE MODE 2/2")],
    [Some("4000 K"), Some("CCT+"), Some("DIM+"), None],
    [Some("4500 K"), Some("CCT-"), Some("DIM-"), Some("MAX RUNTIME")],
];

/// Effets de l'ARC1 : programme et couleurs (interprétation : le rendu exact n'est pas documenté).
fn arc1_effect(key: &str) -> Option<(u32, Vec<[u8; 3]>)> {
    Some(match key {
        "RAINBOW" => (15, vec![]),
        "PASTEL" => (7, vec![[255, 170, 200], [170, 210, 255], [190, 255, 190], [255, 245, 170]]),
        "GARDEN" => (7, vec![[40, 200, 40], [180, 255, 60], [255, 220, 40], [0, 160, 90]]),
        "DISCO" => (13, vec![[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]]),
        "FIRE" => (16, vec![]),
        "ICE" => (6, vec![[0, 80, 255], [0, 255, 255], [220, 240, 255]]),
        _ => return None,
    })
}

fn arc1_color(key: &str) -> Option<Value> {
    let named = |n: &str| json!({ "kind": "named", "name": n });
    Some(match key {
        "RED" | "ORANGE" | "YELLOW" | "GREEN" | "CYAN" | "BLUE" | "PINK" => named(key),
        "MAGENTA" => json!({ "kind": "custom", "rgb": [255, 0, 255] }),
        "WARM WHITE" => named("3200K"),
        "COLD WHITE" => named("6500K"),
        _ => return None,
    })
}

fn speed(key: &str) -> Option<u32> {
    Some(match key {
        "SPEED4" => 2,
        "SPEED3" => 4,
        "SPEED2" => 6,
        "SPEED1" => 8,
        "STOP" => 9,
        _ => return None,
    })
}

fn run_hours(key: &str) -> Option<u32> {
    Some(match key {
        "RUN5" => 5,
        "RUN8" => 8,
        "RUN12" => 12,
        _ => return None,
    })
}

/// Résultat d'une touche de télécommande.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IrResult {
    pub accepted: bool,
    /// changements à appliquer au projecteur
    pub changes: Option<Outcome>,
    /// explication (« refusé (INPUT SELECT = …) »)
    pub note: Option<String>,
}

fn select(f: &Fixture) -> String {
    js::text_or(f.settings.extra.get("inputSelect"), "AUTO").to_string()
}

/// La lampe accepte-t-elle cette commande infrarouge ?
/// AUTO (sans DMX verrouillé) ou APP CONTROL ; si le DMX est verrouillé en AUTO, seuls ON/OFF passent.
pub fn ir_accepted(f: &Fixture, rt: &Runtime, key: &str) -> bool {
    let sel = select(f);
    let on_off = key == "POWER ON" || key == "POWER OFF";
    if sel == "APP CONTROL" {
        return true;
    }
    if sel != "AUTO" {
        return false;
    }
    if matches!(rt.latch, Some(Input::Crmx | Input::Wired)) {
        return on_off;
    }
    true
}

fn ok(settings: Map<String, Value>, state: Map<String, Value>, note: Option<&str>) -> IrResult {
    IrResult { accepted: true, changes: Some(Outcome { settings, state, others: Vec::new() }), note: note.map(str::to_string) }
}

fn refused(note: Option<String>) -> IrResult {
    IrResult { accepted: false, changes: None, note }
}

/// Applique une touche de télécommande à un projecteur (`now` : heure en secondes).
pub fn apply_ir_key(f: &Fixture, ty: &FixtureType, rt: &mut Runtime, ext: &mut AsteraRuntime, key: &str, now: f64) -> IrResult {
    if !ty.is_astera_v5() {
        return refused(Some("pas de récepteur IR simulé sur ce modèle".into()));
    }
    if !ir_accepted(f, rt, key) {
        let latch = ext.latch_name(rt).map(|l| format!(", entrée {l} verrouillée")).unwrap_or_default();
        return refused(Some(format!("refusé (INPUT SELECT = {}{latch})", js::string(f.settings.extra.get("inputSelect")))));
    }
    let s = &f.settings.extra;
    let st = f.state.as_object().cloned().unwrap_or_default();
    let sa = s.get("standalone");
    let none = Map::new;
    let auto = select(f) == "AUTO";
    let mut latch_remote = |rt: &mut Runtime| {
        if auto && rt.latch.is_none() {
            ext.latch_remote(rt);
        }
    };
    let standalone = |patch: Map<String, Value>| obj([("standalone", Value::Object(js::spread(sa, &patch)))]);
    let rest = || sa.and_then(|s| s.get("colors")).and_then(Value::as_array).map(|c| c.iter().skip(1).cloned().collect::<Vec<_>>()).unwrap_or_default();
    let with_first = |first: Value| {
        let mut c = vec![first];
        c.extend(rest());
        Value::Array(c)
    };

    if key == "POWER ON" {
        if js::truthy(st.get("acceptConfig")) {
            return ok(none(), obj([("acceptConfig", json!(false))]), Some("configuration acceptée"));
        }
        return ok(none(), obj([("power", json!(true))]), None);
    }
    if key == "POWER OFF" {
        return ok(none(), obj([("power", json!(false)), ("blueMode", json!(false))]), None);
    }
    if !js::truthy(st.get("power")) {
        return refused(Some("projecteur éteint".into()));
    }

    // ---- ARC1
    if let Some((program, colors)) = arc1_effect(key) {
        latch_remote(rt);
        let colors = if colors.is_empty() { sa.and_then(|s| s.get("colors")).cloned().unwrap_or(Value::Null) } else { Value::Array(colors.iter().map(|c| json!({ "kind": "custom", "rgb": c })).collect()) };
        let mut patch = obj([("program", json!(program))]);
        if !colors.is_null() || sa.and_then(|s| s.get("colors")).is_some() {
            patch.insert("colors".into(), colors);
        }
        return ok(standalone(patch), none(), None);
    }
    if let Some(c) = arc1_color(key) {
        latch_remote(rt);
        return ok(standalone(obj([("program", json!(0)), ("colors", with_first(c))])), none(), None);
    }
    if key == "FADE ON" || key == "FADE OFF" {
        latch_remote(rt);
        return ok(standalone(obj([("fade", json!(if key == "FADE ON" { 100 } else { 0 }))])), none(), None);
    }
    if let Some(v) = speed(key) {
        return ok(standalone(obj([("speed", json!(v))])), none(), None);
    }
    if let Some(h) = run_hours(key) {
        return ok(obj([("runtime", json!(h))]), none(), None);
    }

    // ---- ARC3
    if let Some(k) = key.strip_suffix(" K").filter(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit())) {
        latch_remote(rt);
        let c = json!({ "kind": "hsi", "cct": jv(k.parse::<f64>().unwrap_or(0.0)), "green": 0, "hue": 0, "sat": 0 });
        return ok(standalone(obj([("program", json!(0)), ("colors", with_first(c))])), none(), None);
    }
    if key == "CCT+" || key == "CCT-" {
        latch_remote(rt);
        let c0 = sa.and_then(|s| s.get("colors")).and_then(|c| c.get(0)).filter(|c| c.get("kind").and_then(Value::as_str) == Some("hsi")).cloned().unwrap_or_else(|| json!({ "kind": "hsi", "cct": 5600, "green": 0, "hue": 0, "sat": 0 }));
        let next = js::clamp(js::number(c0.get("cct")) + if key == "CCT+" { 100.0 } else { -100.0 }, 1750.0, 20000.0);
        let first = Value::Object(js::spread(Some(&c0), &obj([("cct", jv(next))])));
        return ok(standalone(obj([("program", json!(0)), ("colors", with_first(first))])), none(), Some("pas de 100 K (non publié)"));
    }
    if let Some(p) = key.strip_suffix('%').filter(|k| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit())) {
        latch_remote(rt);
        return ok(standalone(obj([("intensity", jv(p.parse::<f64>().unwrap_or(0.0)))])), none(), None);
    }
    if key == "DIM+" || key == "DIM-" {
        latch_remote(rt);
        let v = js::clamp(js::nn(sa.and_then(|s| s.get("intensity")), 100.0) + if key == "DIM+" { 5.0 } else { -5.0 }, 10.0, 100.0);
        return ok(standalone(obj([("intensity", jv(v))])), none(), None);
    }
    if key == "BLUE MODE 1/2" {
        ext.ir_blue1 = Some(now);
        return IrResult { accepted: true, changes: None, note: Some("en attente de BLUE MODE 2/2".into()) };
    }
    if key == "BLUE MODE 2/2" {
        if ext.ir_blue1.is_some_and(|t| t != 0.0 && now - t < 5.0) {
            ext.ir_blue1 = None;
            return ok(none(), obj([("blueMode", json!(true))]), None);
        }
        return refused(Some("appuyer d’abord sur BLUE MODE 1/2".into()));
    }
    if key == "MAX RUNTIME" {
        let off = s.get("runtimeLimit") == Some(&Value::Bool(false));
        return ok(obj([("runtimeLimit", json!(off))]), none(), Some(if off { "limite d’autonomie réactivée" } else { "limite d’autonomie désactivée" }));
    }
    refused(None)
}
