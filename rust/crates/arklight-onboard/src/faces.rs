//! Faces avant des Astera (claviers « tube », « ax2 », « v5square », « quik », « dual », dessus de
//! l'AsteraBox, culots des ampoules, embouts des AX1 / AX3), d'après les dispositions de
//! `web/js/ui/control-panel.js`.

use crate::screen::{DisplayKind, DisplaySpec, Element, Face, col, label, led, palette};
use arklight_core::library::FixtureType;
use serde_json::Value;

/// Touche décrite dans `controls.keys` (libellé, infobulle, seconde légende).
fn key_of(controls: &Value, id: &str, shape: &str) -> Option<Element> {
    let keys = controls.get("keys").or_else(|| controls.get("buttons"))?.as_array()?;
    let k = keys.iter().find(|k| k.get("id").and_then(Value::as_str) == Some(id))?;
    let s = |f: &str| k.get(f).and_then(Value::as_str).map(str::to_string);
    let shape = if shape.is_empty() { s("shape").unwrap_or_default() } else { shape.to_string() };
    Some(Element::Key { id: id.into(), label: s("label").unwrap_or_default(), shape, title: s("title"), alt: s("alt") })
}

/// Écran OLED / LCD d'un Astera V5.
fn oled(controls: &Value) -> Option<DisplaySpec> {
    let d = controls.get("display")?;
    let lcd = d.get("kind").and_then(Value::as_str) == Some("lcd");
    let n = |k: &str, v: f64| d.get(k).and_then(Value::as_f64).unwrap_or(v);
    Some(DisplaySpec {
        kind: if lcd { DisplayKind::Lcd } else { DisplayKind::Oled },
        w: n("width", 128.0),
        h: n("height", 64.0),
        zoom: 2.0,
        palette: if lcd { palette("#8fc9f5", "#0d2236", "#0d2236", "#0d2236", "#2b3a46") } else { palette("#000000", "#d6f2ff", "#557482", "#d6f2ff", "#000000") },
    })
}

/// Face avant d'un Astera (ou d'un projecteur à commandes génériques `controls.buttons`).
pub fn astera_face(ty: &FixtureType) -> Face {
    generic_face(ty.extra.get("controls").unwrap_or(&Value::Null))
}

/// Face avant décrite par une définition `controls` (clavier Astera ou boutons génériques).
pub fn generic_face(controls: &Value) -> Face {
    let controls = controls.clone();
    let keyfoil = controls.get("keyfoil").and_then(Value::as_str).unwrap_or("").to_string();
    let k = |id: &str, shape: &str| key_of(&controls, id, shape);
    let some = |v: Vec<Option<Element>>| v.into_iter().flatten().collect::<Vec<_>>();
    let url = || vec![label("www.astera-led.com")];
    let keys4 = || some(vec![k("menu", ""), k("minus", ""), k("plus", ""), k("enter", "")]);
    let mut note = String::new();
    let rows: Vec<Vec<Element>> = match keyfoil.as_str() {
        "tube" => {
            note = "Disposition schématique : sur le tube, le clavier est une bande au dos, près du pixel 1.".into();
            vec![
                some(vec![k("power", "round"), Some(Element::IrWindow), Some(Element::Display), Some(col(some(vec![k("palette", "icon"), k("sun", "icon")])))]),
                some(vec![Some(label("▲ ASTERA")), k("menu", ""), k("minus", ""), k("plus", ""), k("enter", "")]),
                url(),
            ]
        }
        "ax2" => vec![some(vec![k("power", "round"), k("astera", "logo"), k("sun", "icon")]), some(vec![Some(Element::IrWindow), Some(Element::Display), k("palette", "icon")]), keys4(), url()],
        "v5square" => {
            // AX9, PixelBrick, HydraPanel : même clavier que les tubes, disposition carrée (+ TouchSlider)
            let mut mid = some(vec![Some(Element::IrWindow), Some(Element::Display), k("palette", "icon")]);
            if controls.get("slider").and_then(Value::as_bool) == Some(true) {
                mid.push(Element::Slider);
            }
            vec![some(vec![k("power", "round"), Some(label("▲ ASTERA")), k("sun", "icon")]), mid, keys4(), url()]
        }
        "quik" => vec![
            some(vec![k("power", "round"), k("astera", "round"), Some(Element::Led { id: "status".into(), label: String::new(), title: Some("Voyant d’état (documenté seulement dans un ChargingPlate avec PrepBox)".into()) }), k("sun", "icon")]),
            some(vec![Some(Element::IrWindow), Some(Element::Display), k("palette", "icon")]),
            keys4(),
            url(),
        ],
        "dual" => vec![
            vec![label("▲ ASTERA")],
            some(vec![Some(Element::Display), Some(col(vec![Element::Led { id: "battery".into(), label: "BAT".into(), title: Some("Voyant de batterie".into()) }, Element::IrWindow])), k("power", "round")]),
            some(vec![k("menu", "dual-key"), k("minus", "dual-key"), k("plus", "dual-key"), k("enter", "dual-key")]),
        ],
        "art7" => vec![vec![
            col(some(vec![Some(label("BTB")), Some(led("app", "APP")), k("power", "round")])),
            col((0..5).rev().map(|i| Element::Led { id: format!("bat{i}"), label: String::new(), title: Some("Batterie (5 segments)".into()) }).collect()),
            col(vec![led("link", "LINK"), led("dmx", "DMX"), led("rx", "RX"), led("tx", "TX")]),
            col(some(vec![Some(label("CRMX")), k("link", "round link-key")])),
        ]],
        // NYX / SolaBulb : prise DC et touche ON/OFF dans un renfoncement du culot
        "bulb" => vec![some(vec![Some(label("DC")), k("power", "round small")])],
        "ax3" => vec![some(vec![Some(col(some(vec![Some(label("aimant")), k("power", "round small center")])))]), some(vec![Some(label("Bouton latéral :")), k("power-side", "round small")])],
        "ax1" => vec![some(vec![Some(col(some(vec![Some(label("prise de charge")), Some(label("filetage M5")), k("power", "round small")])))]), vec![Element::Shake { label: "↕ Secouer le tube".into() }]],
        _ => {
            // contrôleur générique : toutes les touches à la suite
            let keys = controls.get("keys").or_else(|| controls.get("buttons")).and_then(Value::as_array).cloned().unwrap_or_default();
            let mut row: Vec<Element> = keys.iter().filter_map(|x| x.get("id").and_then(Value::as_str)).filter_map(|id| k(id, "")).collect();
            for l in controls.get("leds").and_then(Value::as_array).into_iter().flatten() {
                if let Some(id) = l.get("id").and_then(Value::as_str) {
                    row.push(led(id, l.get("label").and_then(Value::as_str).unwrap_or("")));
                }
            }
            let mut rows = vec![row];
            if controls.get("display").is_some_and(|d| !d.is_null()) {
                rows.insert(0, vec![Element::Display]);
            }
            rows
        }
    };
    let display = if rows.iter().flatten().any(|e| matches!(e, Element::Display)) { oled(&controls).or_else(|| generic_display(&controls)) } else { None };
    Face { style: if keyfoil.is_empty() { "generic".into() } else { keyfoil }, rows, display, note }
}

fn generic_display(controls: &Value) -> Option<DisplaySpec> {
    let d = controls.get("display")?;
    Some(DisplaySpec {
        kind: DisplayKind::Oled,
        w: d.get("width").and_then(Value::as_f64).unwrap_or(128.0),
        h: d.get("height").and_then(Value::as_f64).unwrap_or(64.0),
        zoom: 2.0,
        palette: palette("#000000", "#d6f2ff", "#557482", "#d6f2ff", "#000000"),
    })
}
