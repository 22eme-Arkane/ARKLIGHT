//! Panneaux ARRI (docs/recherche/menus/arri.md), portage de `shared/onboard/panels/arri.js` :
//!  - SkyPanel S30-C / S60-C / S120-C / S360-C, firmware 4.4 : écran LCD, MENU / BACK / MODE / PRESET,
//!    codeur INTENSITY/SELECTOR (tourner, appuyer) et deux molettes dont la fonction s'affiche au-dessus ;
//!  - L-Series L5-C / L7-C / L10-C (firmware 2.x) : afficheur à segments, FUNCT / ENTER / + / −,
//!    potentiomètres INTENSITY, CCT/HUE, ±GN/SAT, mémoires 1 et 2 ;
//!  - BroadCaster 2 Plus : pas d'écran ; roues codeuses 000–999 (adresse DMX, préréglages 900–905).

use super::common::{BLUE, GREEN, RED, footprint, id_hash, pad};
use crate::fixture::obj;
use crate::js::{self, clamp, jv};
use crate::panel::{Brand, Holds, Drawn, Flash, HOLD_LONG, ItemRef, Kind, ListOpts, MenuState, Opts, Pc, act, draw_list, info, info_fn, num, opts, opts2, pick, pick_dyn, sub};
use crate::screen::{Element, Face, Led, Leds, Prim, col, col_row, key, knob, led, line, palette, rect_fill, set_led, text};
use crate::screen::{DisplayKind, DisplaySpec};
use arklight_core::library::{DmxMode, FixtureType};
use serde_json::{Value, json};

fn cct_range<S: Brand>(c: &Pc<S>) -> (f64, f64) {
    super::common::cct_range(c, (2800.0, 10000.0))
}

/// Numéro court d'un mode DMX ARRI (« P01: CCT & RGBW 8bit » → P01).
fn mode_short(mode: &DmxMode, i: usize) -> String {
    let name = if mode.name.is_empty() { mode.extra.get("label").and_then(Value::as_str).unwrap_or("") } else { mode.name.as_str() };
    let n = name
        .strip_prefix(['P', 'p'])
        .map(|r| r.trim_start().chars().take_while(char::is_ascii_digit).collect::<String>())
        .filter(|d| !d.is_empty())
        .and_then(|d| d.parse::<f64>().ok());
    format!("P{}", pad(n.unwrap_or(i as f64 + 1.0), 2))
}

fn address_item<S: Brand>(label: &str) -> crate::panel::Item<S> {
    super::common::address_item(label)
}

fn mode_item<S: Brand>(label: &str) -> crate::panel::Item<S> {
    pick_dyn(label, |c: &Pc<S>| c.ty.dmx.modes.iter().enumerate().map(|(i, m)| (json!(m.id), mode_short(m, i))).collect())
        .get(|c| c.s("dmxMode").cloned().unwrap_or(Value::Null))
        .set(|c, v| {
            c.set_settings(obj([("dmxMode", v)]));
            None
        })
}

/// Courbes de gradation : libellé ARRI ↔ courbe du moteur.
const CURVES: [(&str, &str); 4] = [("square", "Exponential"), ("linear", "Linear"), ("invSquare", "Logarithmic"), ("sCurve", "S-Curve")];

/// Numéro de série fictif, stable pour un projecteur.
fn serial<S: Brand>(c: &Pc<S>, prefix: &str) -> String {
    let h = f64::from(id_hash(c));
    format!("{prefix}{}-{}", js::num_str(h % 9_000_000.0 + 1_000_000.0), pad(h % 1000.0, 3))
}

/// Univers Art-Net (Net / Subnet / Universe) ↔ univers ARKLIGHT (port-address + 1).
fn artnet<S: Brand>(c: &Pc<S>) -> (f64, f64, f64) {
    let pa = (js::or(c.s("universe"), 1.0) - 1.0).max(0.0);
    ((pa / 256.0).floor(), (pa / 16.0).floor() % 16.0, pa % 16.0)
}

fn set_artnet<S: Brand>(c: &mut Pc<S>, part: usize, v: f64) {
    let (mut net, mut sub, mut uni) = artnet(c);
    match part {
        0 => net = v,
        1 => sub = v,
        _ => uni = v,
    }
    c.set_settings(obj([("universe", jv(net * 256.0 + sub * 16.0 + uni + 1.0))]));
}

fn first_mode(ty: &FixtureType) -> Value {
    ty.dmx.modes.first().map_or(Value::Null, |m| json!(m.id))
}

fn is(v: &Value, s: &str) -> bool {
    v.as_str() == Some(s)
}

// ================================================================== SkyPanel
pub(crate) struct SkyPanel {
    cursor: i64,
    locked: bool,
}

fn is_s360(c: &Pc<SkyPanel>) -> bool {
    c.ty.id == "arri.skypanel-s360c"
}

/// Préréglages d'usine : (nom, mode, cct ou teinte, teinte ±vert ou saturation, description).
const FACTORY_PRESETS: [(&str, &str, f64, f64, &str); 10] = [
    ("Preset 01", "cct", 2900.0, 0.0, "2900K"),
    ("Preset 02", "cct", 3200.0, 0.0, "3200K"),
    ("Preset 03", "cct", 5600.0, 0.0, "5600K"),
    ("Preset 04", "cct", 6500.0, 0.0, "6500K"),
    ("Preset 05", "hsi", 120.0, 1.0, "120° 100%"),
    ("Preset 06", "hsi", 240.0, 1.0, "240° 100%"),
    ("Preset 07", "cct", 5600.0, 0.35, "Rosco 3408"),
    ("Preset 08", "cct", 3200.0, -0.15, "Lee 187"),
    ("Preset 09", "cct", 3200.0, 0.1, "Rosco 3152"),
    ("Preset 10", "cct", 3200.0, 0.05, "Lee 162"),
];
const EFFECTS: [&str; 18] = ["Off", "Party Effect", "Candle", "Clouds Passing", "Club Lights", "Color Chase", "Cop Car", "Explosion", "Fire", "Fireworks", "Fluorescent Flicker", "Light Strobe", "Lightning", "Paparazzi", "Process", "Pulsing", "Television", "Welding"];
const LIGHT_MODES: [(&str, &str); 6] = [("cct", "CCT"), ("hsi", "H S I"), ("gel", "Gel"), ("source", "Source Matching"), ("rgbw", "RGBW"), ("xy", "x, y Coord.")];
const USB: &str = "USB Drive not Mountable : pas de clé USB";

fn on_off(label: &str, key: &'static str, v: &str) -> ItemRef<SkyPanel> {
    pick(label, opts(&["On", "Off"])).stored(key, json!(v)).rc()
}

fn special(key: &'static str, label: &str, other: Option<(&'static str, &'static str)>) -> ItemRef<SkyPanel> {
    pick(label, opts(&["On", "Off"]))
        .get(move |c| c.panel_or(key, json!("Off")))
        .set(move |c, v| {
            if let Some((o, olabel)) = other
                && is(&v, "On")
                && is(&c.panel_or(o, json!("Off")), "On")
            {
                return Some(format!("Not Available: {olabel}"));
            }
            let on = is(&v, "On");
            c.set_panel(obj([(key, v)]));
            on.then(|| "Mémorisé (non simulé dans ARKLIGHT)".into())
        })
        .rc()
}

fn skypanel_tree() -> Vec<ItemRef<SkyPanel>> {
    let stored = |label: &str, list: &[&str], key: &'static str, d: &str| pick(label, opts(list)).stored(key, json!(d)).rc();
    vec![
        sub(
            "DMX Settings",
            vec![
                address_item("DMX Address").rc(),
                mode_item("DMX Mode").rc(),
                stored("DMX Loss Behavior", &["Hold Last Command", "Black Out", "Hold 2 Min Fade Out"], "dmxLoss", "Hold Last Command"),
                stored("DMX Protocol Version", &["Version 3.4", "Version 4.0", "Version 4.1", "Version 4.2", "Version 4.3", "Version 4.4"], "dmxProtocol", "Version 4.4"),
                on_off("Ext. Color Control", "ecc", "Off"),
                on_off("RDM State", "rdm", "On"),
                on_off("DMX Frame Filter", "frameFilter", "On"),
                sub("CRMX Settings", vec![on_off("CRMX Data State", "crmx", "Off"), act("CRMX Connection", |_| Some("Liaison CRMX : avec un émetteur, depuis l’onglet Réseau (non simulé ici)".into())).rc()]).when(is_s360).rc(),
            ],
        )
        .rc(),
        pick_dyn("Fan Mode", |c: &Pc<SkyPanel>| {
            let mut l = if is_s360(c) { vec!["Normal"] } else { vec![] };
            l.extend(["Quiet Mode", "Variable", "High Temp"]);
            opts(&l)
        })
        .stored_fn("fan", |c| json!(if is_s360(c) { "Normal" } else { "Variable" }))
        .rc(),
        pick("Light Mode", opts2(&LIGHT_MODES))
            .get(|c| c.panel_or("lightMode", json!(if c.hsi() { "hsi" } else { "cct" })))
            .set(|c, v| {
                c.set_panel(obj([("lightMode", v.clone())]));
                if is(&v, "cct") || is(&v, "hsi") {
                    c.set_manual(obj([("mode", v)]));
                    return None;
                }
                Some("Mode non simulé dans ARKLIGHT : la lumière reste en CCT / HSI".into())
            })
            .rc(),
        sub(
            "Light Control",
            vec![
                pick("Dimming Curve", opts2(&CURVES))
                    .get(|c| c.s("dimmingCurve").cloned().unwrap_or(Value::Null))
                    .set(|c, v| {
                        c.set_settings(obj([("dimmingCurve", v)]));
                        None
                    })
                    .rc(),
                sub("Special Modes", vec![special("lowEnd", "Low End Mode", Some(("stage", "Stage Mode"))), special("stage", "Stage Mode", Some(("lowEnd", "Low End Mode"))), special("tungsten", "Tungsten Mode", None), special("highSpeed", "High Speed Mode", None)]).rc(),
                on_off("Host / Client Mode", "host", "Off"),
                stored("RGBW Color Space", &["Calibrated Color", "Direct Color"], "rgbwSpace", "Calibrated Color"),
                pick("Frequency Selection", std::iter::once("Default".to_string()).chain((1..=10).map(|i| format!("Frequency {i}"))).map(|s| (json!(s), s)).collect()).stored("frequency", json!("Default")).rc(),
            ],
        )
        .rc(),
        pick("Lighting Effects", opts(&EFFECTS))
            .get(|c| c.panel_or("effect", json!("Off")))
            .set(|c, v| {
                if !is(&v, "Off") && is(&c.panel_or("highSpeed", json!("Off")), "On") {
                    return Some("Not Possible: High Speed Active".into());
                }
                let off = is(&v, "Off");
                c.set_panel(obj([("effect", v)]));
                (!off).then(|| "Effet mémorisé (non simulé dans ARKLIGHT)".into())
            })
            .rc(),
        sub(
            "Display Setup",
            vec![
                stored("Display Illumination", &["Always On", "Off After 10 Sec."], "illum", "Always On"),
                num("Display Brightness", 0.0, 10.0).stored("brightness", json!(10)).rc(),
                num("Display Contrast", 1.0, 10.0).fmt(|v, _| pad(js::number(Some(v)), 2)).stored("contrast", json!(3)).rc(),
                stored("Display Orientation", &["Normal", "Upside Down"], "orientation", "Normal"),
                stored("Display Error Mode", &["Normal", "Hidden"], "errorMode", "Hidden"),
            ],
        )
        .rc(),
        sub("USB Functions", ["Light Preset", "DMX Preset", "Fixture Setting", "Save Error Log"].iter().map(|l| act(l, |_| Some(USB.into())).rc()).collect()).rc(),
        sub(
            "Art-Net and sACN",
            vec![
                stored("Art-Net / sACN State", &["Automatic", "Art-Net only", "sACN only", "Off"], "netState", "Automatic"),
                num("Art-Net Net", 0.0, 127.0)
                    .get(|c| jv(artnet(c).0))
                    .set(|c, v| {
                        set_artnet(c, 0, js::number(Some(&v)));
                        None
                    })
                    .rc(),
                num("Art-Net Subnet", 0.0, 15.0)
                    .get(|c| jv(artnet(c).1))
                    .set(|c, v| {
                        set_artnet(c, 1, js::number(Some(&v)));
                        None
                    })
                    .rc(),
                num("Art-Net Universe", 0.0, 15.0)
                    .get(|c| jv(artnet(c).2))
                    .set(|c, v| {
                        set_artnet(c, 2, js::number(Some(&v)));
                        None
                    })
                    .rc(),
                stored("Merge Mode", &["Latest Takes Precedence", "Highest Takes Precedence"], "merge", "Latest Takes Precedence"),
                on_off("Gateway", "gateway", "On"),
                num("sACN Universe", 1.0, 63999.0)
                    .get(|c| jv(js::or(c.s("universe"), 1.0)))
                    .set(|c, v| {
                        c.set_settings(obj([("universe", v)]));
                        None
                    })
                    .rc(),
            ],
        )
        .rc(),
        sub(
            "Network Settings",
            vec![
                info("Link", "No Connection").rc(),
                info("IP", "2.0.0.1").rc(),
                stored("Mode", &["DHCP", "Art-Net 2.B.C.D", "Art-Net 10.B.C.D", "Manual"], "ipMode", "DHCP"),
                info("Mask", "255.255.255.0").rc(),
                info_fn("MAC", |c| {
                    let s: Vec<char> = serial(c, "").chars().collect();
                    let part = |a: usize, b: usize| s.get(a..b.min(s.len())).map_or(String::new(), |x| x.iter().collect());
                    format!("00:1E:9D:{}:{}:{}", part(1, 3), part(3, 5), part(5, 7))
                })
                .rc(),
                on_off("BONJ", "bonjour", "On"),
            ],
        )
        .rc(),
        sub(
            "Fixture Info",
            vec![
                info("Fixture Status", "System Ready").rc(),
                info_fn("Light Engine Temp.", |c| format!("{} °C", js::to_fixed(30.0 + c.m_nn("intensity", 0.6) * 18.0, 1))).rc(),
                info("Hour Counter", "512h - LE").rc(),
                info("Battery Status", "— V").when(|c| !is_s360(c)).rc(),
                info_fn("Fixture Serial No.", |c| serial(c, "L1.")).rc(),
                info("Firmware Versions", "FW: 4.4.0.6").rc(),
            ],
        )
        .rc(),
        sub(
            "Fixture Settings",
            vec![
                num("Low Battery Warning", 22.0, 30.0).step(0.5).fmt(|v, _| format!("{} V", js::to_fixed(js::number(Some(v)), 1))).stored("lowBatt", json!(23)).when(|c| !is_s360(c)).rc(),
                stored("USB Mode", &["Normal", "Service"], "usbMode", "Normal"),
                sub("LE Compensation", vec![info("Light Engine Status", "Gen 2 or above installed").rc(), on_off("LE Compensation State", "leComp", "Off")]).rc(),
            ],
        )
        .rc(),
        pick("Factory Reset", opts(&["No", "Yes"]))
            .get(|_| json!("No"))
            .set(|c, v| {
                if !is(&v, "Yes") {
                    return None;
                }
                let first = first_mode(c.ty);
                c.set_settings(obj([("panel", json!({})), ("dmxAddress", json!(1)), ("dmxMode", first), ("dimmingCurve", json!("square"))]));
                c.set_manual(obj([("mode", json!("cct")), ("intensity", json!(1)), ("cct", json!(5600)), ("tint", json!(0))]));
                Some("Factory Reset : réglages d’usine".into())
            })
            .rc(),
    ]
}

fn skypanel_home(c: &Pc<SkyPanel>) -> Vec<Prim> {
    let w = 128.0;
    let mut prims: Vec<Prim> = Vec::new();
    let hsi = c.hsi();
    let live = c.dmx_live();
    // coin haut gauche : verrou, effet, mode spécial
    let special = if c.st.locked {
        "LOCKED".to_string()
    } else if !is(&c.panel_or("effect", json!("Off")), "Off") {
        js::string(c.panel("effect"))
    } else if c.panel("lowEnd").is_some_and(|v| is(v, "On")) {
        "Low End Mode".into()
    } else if c.panel("tungsten").is_some_and(|v| is(v, "On")) {
        "Tungsten".into()
    } else {
        String::new()
    };
    if !special.is_empty() {
        prims.push(text(2.0, 1.0, special, 9.0).w(76.0).into());
    }
    let corner = if live {
        "DMX"
    } else if c.panel("ecc").is_some_and(|v| is(v, "On")) {
        if hsi { "★HSI" } else { "★CCT" }
    } else if hsi {
        "HSI"
    } else {
        "CCT"
    };
    prims.push(text(w - 2.0, 1.0, corner, 10.0).r().b().into());
    if live {
        // page « DMX Live » : mode, adresse, nombre de canaux
        let mode = c.ty.mode(c.s("dmxMode").and_then(Value::as_str));
        let i = mode.and_then(|m| c.ty.dmx.modes.iter().position(|x| std::ptr::eq(x, m)));
        prims.push(text(w / 2.0, 15.0, mode.map_or("—".into(), |m| mode_short(m, i.unwrap_or(0))), 18.0).c().b().into());
        prims.push(line(2.0, 40.0, w - 2.0, 40.0, Some(1.0)));
        let a = c.address();
        prims.push(text(32.0, 46.0, format!("{}-{}", pad(a, 3), pad(a + footprint(c) - 1.0, 3)), 10.0).c().into());
        prims.push(line(64.0, 42.0, 64.0, 62.0, Some(1.0)));
        prims.push(text(96.0, 46.0, format!("{} Ch", js::num_str(footprint(c))), 10.0).c().into());
        return prims;
    }
    let left = if hsi { format!("{}°", js::num_str(js::round(c.m_or("hue", 0.0)))) } else { format!("{}K", js::num_str(js::round(c.m_or("cct", 5600.0)))) };
    let tint = c.m_or("tint", 0.0);
    let right = if hsi { format!("{}% SAT", js::num_str(js::round(c.m_nn("sat", 1.0) * 100.0))) } else { format!("{}{} GN", if tint >= 0.0 { "+" } else { "" }, js::to_fixed(tint, 2)) };
    let intensity = c.m_nn("intensity", 0.0);
    let big = c.flash.as_ref().map(|f| f.text.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| format!("{}%", js::to_fixed(intensity * 100.0, 1)));
    prims.push(text(w / 2.0, 14.0, big, 20.0).c().b().into());
    // trait : une courte barre épaisse à gauche figure l'intensité
    prims.push(line(2.0, 40.0, w - 2.0, 40.0, Some(1.0)));
    prims.push(rect_fill(2.0, 39.0, ((w - 4.0) * intensity).max(2.0), 2.5, "fg"));
    let cell = c.flash.as_ref().and_then(|f| f.cell);
    if cell != Some('L') {
        prims.push(text(32.0, 46.0, left, 11.0).c().into());
    }
    prims.push(line(64.0, 43.0, 64.0, 62.0, Some(1.0)));
    if cell != Some('R') {
        prims.push(text(96.0, 46.0, right, 11.0).c().into());
    }
    prims
}

/// Préréglages utilisateur (10 emplacements, vides : None).
fn user_slots(c: &Pc<SkyPanel>) -> Vec<Value> {
    let u = c.panel_or("presets", json!([]));
    (0..10).map(|i| u.get(i).filter(|v| js::truthy(Some(v))).cloned().unwrap_or(Value::Null)).collect()
}

fn presets_screen(c: &Pc<SkyPanel>) -> Vec<Prim> {
    let save = c.screen == "save";
    let slot_label = |p: &Value| if p.is_null() { "Empty".to_string() } else { js::string(p.get("label")) };
    let rows: Vec<String> = if save {
        user_slots(c).iter().enumerate().map(|(i, p)| format!("{}  {}", i + 1, slot_label(p))).collect()
    } else {
        FACTORY_PRESETS.iter().map(|(n, .., d)| format!("{} {d}", n.replace("Preset ", ""))).chain(user_slots(c).iter().enumerate().map(|(i, p)| format!("U{} {}", i + 1, slot_label(p)))).collect()
    };
    let mut menu = MenuState::new(rows.iter().map(|l| info::<SkyPanel>(l, "").rc()).collect(), if save { "Save Preset" } else { "Presets" }, false);
    menu.stack[0].cursor = c.st.cursor;
    draw_list(c, &menu, &ListOpts { values: false, ..ListOpts::default() })
}

fn apply_preset(c: &mut Pc<SkyPanel>, i: i64) {
    if let Some((name, mode, a, b, _)) = usize::try_from(i).ok().and_then(|i| FACTORY_PRESETS.get(i)) {
        let mut look = if *mode == "cct" { obj([("mode", json!(mode)), ("cct", jv(*a)), ("tint", jv(*b))]) } else { obj([("mode", json!(mode)), ("hue", jv(*a)), ("sat", jv(*b))]) };
        look.insert("intensity".into(), jv(c.m_nn("intensity", 1.0)));
        c.set_manual(look);
        c.set_panel(obj([("lightMode", json!(mode))]));
        return c.say(format!("{name} activé"));
    }
    let k = i - FACTORY_PRESETS.len() as i64;
    let p = usize::try_from(k).ok().and_then(|k| user_slots(c).get(k).cloned()).unwrap_or(Value::Null);
    if p.is_null() {
        return c.say("Empty");
    }
    c.set_manual(p.get("look").and_then(Value::as_object).cloned().unwrap_or_default());
    c.say(format!("Preset U{} activé", k + 1));
}

fn arri_display(w: f64, h: f64, dim: &str) -> Option<DisplaySpec> {
    Some(DisplaySpec { kind: DisplayKind::Draw, w, h, zoom: 2.0, palette: palette("#a9d9ee", "#0d2533", dim, "#0d2533", "#1d2a31") })
}

impl Brand for SkyPanel {
    const SIZE: (f64, f64) = (128.0, 64.0);
    const ID: &'static str = "arri-skypanel";

    fn init(_: &'static FixtureType) -> Self {
        SkyPanel { cursor: 0, locked: false }
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "arri".into(),
            rows: vec![
                vec![col(vec![led("power", "POWER"), led("data", "DATA"), led("status", "STATUS")]), col(vec![key("menu", "MENU", ""), key("back", "BACK", "")]), Element::Display, col(vec![key("mode", "MODE", ""), key("preset", "PRESET", "")])],
                vec![knob("is", "INTENSITY/SELECTOR", "cap=blue push size=l"), knob("center", "", "pointer"), knob("right", "", "pointer")],
            ],
            display: arri_display(128.0, 64.0, "#6c93a4"),
            note: "Molettes : tourner avec la molette de la souris ou en glissant ; cliquer le codeur bleu pour valider. Le codeur maintenu 5 s verrouille le panneau.".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        match id {
            "is" => vec![(5.0, |c, _| {
                if c.screen != "home" {
                    return;
                }
                c.st.locked = !c.st.locked;
                let l = c.st.locked;
                c.say(if l { "LOCKED" } else { "Déverrouillé" });
            })],
            "preset" => vec![(HOLD_LONG, |c, _| {
                if c.st.locked {
                    return;
                }
                c.screen = "save";
                c.st.cursor = 0;
            })],
            _ => Vec::new(),
        }
    }

    fn key(c: &mut Pc<Self>, id: &str, long: bool) {
        if c.st.locked {
            return c.say("LOCKED");
        }
        match id {
            "menu" => {
                if c.screen == "menu" {
                    c.close_menu();
                } else {
                    c.open_menu(skypanel_tree(), "Menu", false);
                }
            }
            "back" => {
                if c.screen == "menu" {
                    if !c.menu_back() {
                        c.close_menu();
                    }
                    return;
                }
                c.screen = "home";
            }
            "mode" => {
                if c.screen != "home" {
                    return;
                }
                if long {
                    return c.say("Extended Color Control : non simulé");
                }
                let next = if c.hsi() { "cct" } else { "hsi" };
                c.set_manual(obj([("mode", json!(next))]));
                c.set_panel(obj([("lightMode", json!(next))]));
            }
            "preset" => {
                c.screen = if c.screen == "presets" { "home" } else { "presets" };
                c.st.cursor = 0;
            }
            "is" => {
                if c.screen == "menu" {
                    c.menu_enter_say();
                } else if c.screen == "presets" {
                    let i = c.st.cursor;
                    apply_preset(c, i);
                    c.screen = "home";
                } else if c.screen == "save" {
                    let mut u = user_slots(c);
                    let mut look = serde_json::Map::new();
                    for k in ["mode", "intensity", "cct", "tint", "hue", "sat"] {
                        if let Some(v) = c.m(k) {
                            look.insert(k.into(), v.clone());
                        }
                    }
                    let label = if c.hsi() { format!("{}°", js::num_str(js::round(c.m_or("hue", 0.0)))) } else { format!("{}K", js::num_str(js::round(c.m_or("cct", 5600.0)))) };
                    let cur = c.st.cursor;
                    if let Some(slot) = usize::try_from(cur).ok().and_then(|k| u.get_mut(k)) {
                        *slot = json!({ "look": look, "label": label });
                    }
                    c.set_panel(obj([("presets", Value::Array(u))]));
                    c.say(format!("Preset U{} enregistré", cur + 1));
                    c.screen = "home";
                }
            }
            _ => {}
        }
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, mult: f64) {
        if c.st.locked {
            return c.say("LOCKED");
        }
        if c.screen == "menu" {
            if id == "is" {
                c.menu_move(steps, mult);
            }
            return;
        }
        if c.screen == "presets" || c.screen == "save" {
            if id == "is" {
                let max = if c.screen == "save" { 9.0 } else { 19.0 };
                c.st.cursor = clamp(c.st.cursor as f64 + steps, 0.0, max) as i64;
            }
            return;
        }
        if id == "is" {
            let v = clamp(js::round((c.m_nn("intensity", 0.0) + steps * 0.001 * mult) * 1000.0) / 1000.0, 0.0, 1.0);
            return c.set_manual(obj([("intensity", jv(v))]));
        }
        let hsi = c.hsi();
        let flash = |c: &mut Pc<Self>, text: String, cell: char| c.flash = Some(Flash { text, until: c.t + 1.5, cell: Some(cell) });
        if id == "center" {
            if hsi {
                let hue = ((js::round(c.m_or("hue", 0.0)) + steps * mult) % 360.0 + 360.0) % 360.0;
                c.set_manual(obj([("hue", jv(hue))]));
                flash(c, format!("{}°", js::num_str(hue)), 'L');
            } else {
                let (lo, hi) = cct_range(c);
                let cct = clamp(js::round((c.m_or("cct", 5600.0) + steps * 10.0 * mult) / 10.0) * 10.0, lo, hi);
                c.set_manual(obj([("cct", jv(cct))]));
                flash(c, format!("{}K", js::num_str(cct)), 'L');
            }
        } else if id == "right" {
            if hsi {
                let sat = clamp(js::round(c.m_nn("sat", 1.0) * 100.0 + steps * mult), 0.0, 100.0) / 100.0;
                c.set_manual(obj([("sat", jv(sat))]));
                flash(c, format!("{}% SAT", js::num_str(js::round(sat * 100.0))), 'R');
            } else {
                let tint = clamp(js::round((c.m_or("tint", 0.0) + steps * 0.01 * mult) * 100.0) / 100.0, -1.0, 1.0);
                c.set_manual(obj([("tint", jv(tint))]));
                flash(c, format!("{}{} GN", if tint >= 0.0 { "+" } else { "" }, js::to_fixed(tint, 2)), 'R');
            }
        }
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        if c.screen == "menu" {
            let prims = c.menu.as_ref().map(|m| draw_list(c, m, &ListOpts::default())).unwrap_or_default();
            return Drawn::of(prims);
        }
        if c.screen == "presets" || c.screen == "save" {
            return Drawn::of(presets_screen(c));
        }
        // `lastTouch` n'est jamais renseigné dans la version JavaScript : 0
        let dim = c.panel("illum").is_some_and(|v| is(v, "Off After 10 Sec.")) && c.t > 10.0;
        Drawn { prims: skypanel_home(c), dim }
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        let mut l = Leds::new();
        let on = c.on();
        set_led(&mut l, "power", on.then(|| Led::on(GREEN)));
        set_led(&mut l, "data", (on && c.dmx_live()).then(|| Led::on(BLUE)));
        set_led(&mut l, "status", on.then(|| if c.st.locked { Led::blink(RED, 2.0) } else { Led::on(GREEN) }));
        l
    }
}

// ================================================================== L-Series (L5-C, L7-C, L10-C)
const LEVELS: [&str; 5] = ["INTENSITY", "CCT", "GRN", "STATUS", "SETUP"];

pub(crate) struct LSeries {
    level: i64,
    back: f64,
    shift: f64,
    jump: f64,
    legend: &'static str,
    blink_until: f64,
}

/// Confirmation d'un réglage : la valeur clignote trois fois (pas de message).
fn blink(c: &mut Pc<LSeries>) {
    c.st.blink_until = c.t + 1.2;
}

fn lseries_tree(c: &Pc<LSeries>) -> Vec<ItemRef<LSeries>> {
    let modes: Vec<(Value, String)> = c.ty.dmx.modes.iter().enumerate().map(|(i, m)| (json!(m.id), format!("P.{}", pad(i as f64 + 1.0, 3)))).collect();
    vec![
        sub(
            "STATUS",
            vec![
                info("OK", "").rc(),
                info("SW", "2.5").rc(),
                info("DMXV", "3.1").rc(),
                info("HOUR", "501t").rc(),
                info_fn("TEMP", |c| format!("{}°", js::to_fixed(30.0 + c.m_nn("intensity", 0.6) * 14.0, 1))).rc(),
                info("VBAT", "—").when(|c| c.ty.id == "arri.l5-c").rc(),
            ],
        )
        .rc(),
        sub(
            "SETUP",
            vec![
                sub(
                    "DMX",
                    vec![
                        pick("PROT", modes)
                            .get(|c| c.s("dmxMode").cloned().unwrap_or(Value::Null))
                            .set(|c, v| {
                                c.set_settings(obj([("dmxMode", v)]));
                                blink(c);
                                None
                            })
                            .rc(),
                        pick("LSIG", opts(&["HOLD", "BOUT", "H2MF"]))
                            .get(|c| c.panel_or("dmxLoss", json!("HOLD")))
                            .set(|c, v| {
                                c.set_panel(obj([("dmxLoss", v)]));
                                blink(c);
                                None
                            })
                            .rc(),
                        num("CHAN", 1.0, 512.0)
                            .max_fn(|c| 513.0 - footprint(c))
                            .get(|c| c.s("dmxAddress").cloned().unwrap_or(Value::Null))
                            .set(|c, v| {
                                c.set_settings(obj([("dmxAddress", v)]));
                                blink(c);
                                None
                            })
                            .rc(),
                    ],
                )
                .rc(),
                pick("FACT", opts(&["NO", "YES"]))
                    .get(|_| json!("NO"))
                    .set(|c, v| {
                        if !is(&v, "YES") {
                            return None;
                        }
                        let first = first_mode(c.ty);
                        c.set_settings(obj([("panel", json!({})), ("dmxAddress", json!(1)), ("dmxMode", first)]));
                        c.set_manual(obj([("mode", json!("cct")), ("intensity", json!(0)), ("cct", json!(5600)), ("tint", json!(0))]));
                        Some("Réglages d’usine".into())
                    })
                    .rc(),
                pick("DEMO", opts(&["ON", "OFF"])).stored("demo", json!("OFF")).rc(),
                pick("INDC", opts(&["ON", "OFF"])).stored("indc", json!("ON")).rc(),
                pick("FAN", opts(&["LOW", "VARI", "HI45"])).stored("fan", json!("VARI")).rc(),
            ],
        )
        .rc(),
    ]
}

/// Enfants d'un sous-menu.
fn children<S: Brand>(it: &ItemRef<S>) -> Vec<ItemRef<S>> {
    match &it.kind {
        Kind::Sub(c) => c.iter().cloned().collect(),
        _ => Vec::new(),
    }
}

/// Ce qu'affiche l'afficheur à segments.
struct LText {
    text: String,
    unit: &'static str,
    legend: &'static str,
    arrow: bool,
    label: String,
}

fn lseries_text(c: &mut Pc<LSeries>) -> LText {
    let hsi = c.hsi();
    let legend = c.st.legend;
    if c.menu.is_some() {
        if let Some(edit) = c.menu.as_ref().and_then(|m| m.edit.as_ref()) {
            let it = edit.item.clone();
            let v = edit.value.clone();
            let text = match &it.kind {
                Kind::Num { .. } => js::string(Some(&v)),
                Kind::Pick { options: Opts::Dyn(f) } => f(c).into_iter().find(|o| js::strict_eq(Some(&o.0), Some(&v))).map(|o| o.1).unwrap_or_default(),
                _ => js::string(Some(&v)),
            };
            return LText { text, unit: "", legend, arrow: false, label: String::new() };
        }
        let it = c.with_menu(|m, c| m.current(c)).flatten();
        let (val, is_info) = match it.as_ref().map(|i| &i.kind) {
            Some(Kind::Info(crate::panel::InfoVal::Text(t))) => (t.clone(), true),
            Some(Kind::Info(crate::panel::InfoVal::Dyn(f))) => (f(c), true),
            _ => (String::new(), false),
        };
        let text = match &it {
            Some(_) if is_info && !val.is_empty() => val,
            Some(i) => i.label.clone(),
            None => String::new(),
        };
        let arrow = it.as_ref().is_some_and(|i| matches!(i.kind, Kind::Sub(_) | Kind::Pick { .. } | Kind::Num { .. }));
        let label = if is_info { it.as_ref().map(|i| i.label.clone()).unwrap_or_default() } else { String::new() };
        return LText { text, unit: "", legend, arrow, label };
    }
    let r = |x: f64| js::num_str(js::round(x));
    let t = |text: String, unit: &'static str, legend: &'static str| LText { text, unit, legend, arrow: false, label: String::new() };
    match LEVELS.get(c.st.level as usize).copied().unwrap_or("") {
        "INTENSITY" => t(r(c.m_nn("intensity", 0.0) * 100.0), "%", "INTENSITY"),
        "CCT" => {
            if hsi {
                t(format!("{}°", r(c.m_or("hue", 0.0))), "", "CCT")
            } else {
                t(r(c.m_or("cct", 5600.0)), "K", "CCT")
            }
        }
        "GRN" => {
            if hsi {
                t(r(c.m_nn("sat", 1.0) * 100.0), "", "GRN")
            } else {
                let tint = c.m_or("tint", 0.0);
                t(format!("{}{}", if tint >= 0.0 { "+" } else { "-" }, js::to_fixed(tint.abs(), 2)), "", "GRN")
            }
        }
        "STATUS" => LText { text: "OK".into(), unit: "", legend: "STATUS", arrow: true, label: String::new() },
        _ => LText { text: String::new(), unit: "", legend: "SETUP", arrow: true, label: String::new() },
    }
}

impl Brand for LSeries {
    const SIZE: (f64, f64) = (128.0, 48.0);
    const ID: &'static str = "arri-lseries";

    fn init(_: &'static FixtureType) -> Self {
        LSeries { level: 1, back: 0.0, shift: 0.0, jump: 0.0, legend: "STATUS", blink_until: 0.0 }
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "arri".into(),
            rows: vec![
                vec![col(vec![led("status", "POWER/STATUS"), led("data", "DATA")]), col(vec![key("funct", "FUNCT", ""), key("enter", "ENTER", "")]), Element::Display, col(vec![key("plus", "+ ▲", ""), key("minus", "− ▼", "")])],
                vec![knob("intensity", "INTENSITY", "cap=blue size=l"), key("mem1", "1", "small"), knob("cct", "CCT / HUE", "pointer"), key("mem2", "2", "small"), knob("gn", "±GN / SAT", "pointer")],
            ],
            display: arri_display(128.0, 48.0, "#8fb8c9"),
            note: "Les molettes sont des potentiomètres (butées). ENTER puis 1 ou 2 : mémoires 3200 K / 5600 K. ENTER maintenu : JUMP (+ adresse, − mode DMX).".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        if id != "enter" {
            return Vec::new();
        }
        vec![(1.5, |c, _| {
            c.st.jump = c.t + 3.0;
            c.say("JUMP : + adresse DMX, − mode DMX, FUNCT démo");
        })]
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if c.st.jump > c.t && c.menu.is_none() && ["plus", "minus", "funct"].contains(&id) {
            // JUMP : + = adresse DMX (CHAN), − = mode DMX (PROT), FUNCT = démo
            c.st.jump = 0.0;
            if id == "funct" {
                let demo = if is(&c.panel_or("demo", json!("OFF")), "ON") { "OFF" } else { "ON" };
                return c.set_panel(obj([("demo", json!(demo))]));
            }
            let tree = lseries_tree(c);
            let dmx = children(&tree[1]).first().map(children).unwrap_or_default();
            let mut menu = MenuState::new(dmx, "DMX", false);
            menu.stack[0].cursor = if id == "plus" { 2 } else { 0 };
            c.set_menu(Some(menu));
            c.st.legend = "SETUP";
            c.menu_enter();
            return;
        }
        if c.menu.is_some() {
            match id {
                "plus" | "minus" => return c.menu_move(if id == "plus" { -1.0 } else { 1.0 }, 1.0),
                "enter" => {
                    c.menu_enter_say();
                    return;
                }
                "funct" => {
                    if !c.menu_back() {
                        c.set_menu(None);
                    }
                    return;
                }
                _ => {}
            }
        }
        match id {
            "plus" | "minus" => {
                c.st.level = (c.st.level + if id == "plus" { -1 } else { 1 } + LEVELS.len() as i64) % LEVELS.len() as i64;
                c.st.back = c.t + 2.0;
            }
            "funct" => {
                let next = if c.hsi() { "cct" } else { "hsi" };
                c.set_manual(obj([("mode", json!(next))]));
            }
            "enter" => {
                let lv = LEVELS[c.st.level as usize];
                if lv == "STATUS" || lv == "SETUP" {
                    let tree = lseries_tree(c);
                    let items = children(&tree[if lv == "STATUS" { 0 } else { 1 }]);
                    c.set_menu(Some(MenuState::new(items, lv, false)));
                    c.st.legend = lv;
                    return;
                }
                c.st.shift = c.t + 3.0; // flèche clignotante : touches mémoire armées
            }
            "mem1" | "mem2" => {
                if c.st.shift < c.t {
                    return c.say("Appuie d’abord sur ENTER (flèche clignotante)");
                }
                c.st.shift = 0.0;
                c.set_manual(obj([("mode", json!("cct")), ("cct", json!(if id == "mem1" { 3200 } else { 5600 })), ("tint", json!(0))]));
                c.st.level = 1;
                c.say(if id == "mem1" { "Mémoire 1 : 3200 K" } else { "Mémoire 2 : 5600 K" });
            }
            _ => {}
        }
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, mult: f64) {
        let hsi = c.hsi();
        match id {
            "intensity" => {
                let v = clamp(js::round(c.m_nn("intensity", 0.0) * 100.0 + steps * mult) / 100.0, 0.0, 1.0);
                c.set_manual(obj([("intensity", jv(v))]));
                c.st.level = 0;
            }
            "cct" => {
                if hsi {
                    let v = clamp(js::round(c.m_or("hue", 0.0) + steps * 2.0 * mult), 0.0, 360.0);
                    c.set_manual(obj([("hue", jv(v))]));
                } else {
                    let (lo, hi) = cct_range(c);
                    let v = clamp(js::round((c.m_or("cct", 5600.0) + steps * 50.0 * mult) / 50.0) * 50.0, lo, hi);
                    c.set_manual(obj([("cct", jv(v))]));
                }
                c.st.level = 1;
            }
            "gn" => {
                if hsi {
                    let v = clamp(js::round(c.m_nn("sat", 1.0) * 100.0 + steps * mult), 0.0, 100.0) / 100.0;
                    c.set_manual(obj([("sat", jv(v))]));
                } else {
                    let v = clamp(js::round((c.m_or("tint", 0.0) + steps * 0.02 * mult) * 100.0) / 100.0, -1.0, 1.0);
                    c.set_manual(obj([("tint", jv(v))]));
                }
                c.st.level = 2;
            }
            _ => {}
        }
        c.st.back = c.t + 2.0;
    }

    fn tick(c: &mut Pc<Self>) {
        // retour à la valeur du mode après 2 s (sauf STATUS et SETUP)
        if c.menu.is_none() && c.st.back != 0.0 && c.t > c.st.back && c.st.level != 3 && c.st.level != 4 {
            c.st.level = 1;
            c.st.back = 0.0;
        }
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let d = lseries_text(c);
        let mut prims: Vec<Prim> = Vec::new();
        let legends = [("INTENSITY", "INTENSITY"), ("CCT", "CCT  HUE"), ("GRN", "GRN  SAT"), ("STATUS", "STATUS"), ("SETUP", "SETUP")];
        let lit = if c.menu.is_some() { c.st.legend } else { d.legend };
        for (i, (k, txt)) in legends.iter().enumerate() {
            prims.push(text(2.0, 2.0 + i as f64 * 9.0, *txt, 7.0).col(if lit == *k { "fg" } else { "dim" }).b().into());
        }
        let blink_off = c.st.blink_until > c.t && (c.t * 5.0).floor() % 2.0 == 0.0;
        if !blink_off {
            prims.push(text(108.0, 12.0, d.text.chars().take(5).collect::<String>(), 22.0).r().b().font("seg").into());
        }
        if !d.unit.is_empty() {
            prims.push(text(124.0, 16.0, d.unit, 11.0).r().b().into());
        }
        if !d.label.is_empty() {
            prims.push(text(108.0, 38.0, d.label, 7.0).r().col("dim").into());
        }
        let arrow_on = d.arrow || (c.st.shift > c.t && (c.t * 3.0).floor() % 2.0 == 0.0);
        if arrow_on {
            prims.push(text(54.0, 34.0, "➡", 10.0).into());
        }
        if c.st.jump > c.t {
            prims.push(text(108.0, 12.0, "JUMP", 22.0).r().b().font("seg").into());
        }
        Drawn { prims, dim: is(&c.panel_or("indc", json!("ON")), "OFF") }
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        let mut l = Leds::new();
        let ind = is(&c.panel_or("indc", json!("ON")), "ON");
        let on = c.on();
        set_led(&mut l, "status", (on && ind).then(|| Led::on(GREEN)));
        set_led(&mut l, "data", (on && c.dmx_live() && ind).then(|| Led::on(BLUE)));
        l
    }
}

// ================================================================== BroadCaster 2 Plus
pub(crate) struct BroadCaster;

/// Modes secours de la roue codeuse : 900 à 905 → température.
fn backup(v: f64) -> Option<f64> {
    [(900.0, 2800.0), (901.0, 3200.0), (902.0, 4000.0), (903.0, 4800.0), (904.0, 5600.0), (905.0, 6500.0)].iter().find(|(k, _)| *k == v).map(|x| x.1)
}

fn bcd_apply(c: &mut Pc<BroadCaster>, v: f64) {
    c.set_panel(obj([("bcd", jv(v))]));
    if (1.0..=512.0).contains(&v) {
        let a = v.min(513.0 - footprint(c));
        c.set_settings(obj([("dmxAddress", jv(a))]));
        c.say(format!("Adresse DMX {}", pad(v, 3)));
    } else if let Some(k) = backup(v) {
        c.set_manual(obj([("mode", json!("cct")), ("cct", jv(k)), ("intensity", json!(1)), ("tint", json!(0))]));
        c.say(format!("Mode secours {} : {} K", js::num_str(v), js::num_str(k)));
    } else {
        c.say(format!("{} : sans fonction documentée", pad(v, 3)));
    }
}

fn bcd_value(c: &Pc<BroadCaster>) -> f64 {
    match c.panel("bcd") {
        Some(v) => js::number(Some(v)),
        None => js::or(c.s("dmxAddress"), 1.0),
    }
}

impl Brand for BroadCaster {
    const SIZE: (f64, f64) = (66.0, 26.0);
    const DISPLAY_WHEN_OFF: bool = true;
    const ID: &'static str = "arri-broadcaster";

    fn init(_: &'static FixtureType) -> Self {
        BroadCaster
    }

    fn face(_: &FixtureType) -> Face {
        let digits = |sign: &str, label: &str| col_row(["d2", "d1", "d0"].iter().map(|d| key(&format!("{d}{sign}"), label, "small")).collect());
        Face {
            style: "arri".into(),
            rows: vec![vec![led("red", ""), led("green", ""), led("blue", "")], vec![digits("+", "+")], vec![Element::Display], vec![digits("-", "−")]],
            display: Some(DisplaySpec { kind: DisplayKind::Draw, w: 66.0, h: 26.0, zoom: 2.0, palette: palette("#0b0c0e", "#f4f4f0", "#5d6066", "#f4f4f0", "#0b0c0e") }),
            note: "Pas d’écran ni de menu : la roue codeuse donne l’adresse DMX (001–512) ou un mode secours (900–905 : 2800 à 6500 K).".into(),
        }
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        let b = id.as_bytes();
        if b.len() != 3 || b[0] != b'd' || !b[1].is_ascii_digit() || (b[2] != b'+' && b[2] != b'-') {
            return;
        }
        let digit = usize::from(b[1] - b'0');
        let cur = bcd_value(c);
        let mut digits = [cur % 10.0, (cur / 10.0).floor() % 10.0, (cur / 100.0).floor() % 10.0];
        if digit > 2 {
            return;
        }
        digits[digit] = (digits[digit] + if b[2] == b'+' { 1.0 } else { 9.0 }) % 10.0;
        bcd_apply(c, digits[0] + digits[1] * 10.0 + digits[2] * 100.0);
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let v = bcd_value(c);
        Drawn::of(vec![text(33.0, 3.0, pad(v, 3), 20.0).c().b().font("mono").sp(6.0).into()])
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        let mut l = Leds::new();
        let on = c.on();
        set_led(&mut l, "green", on.then(|| Led::on(GREEN)));
        set_led(&mut l, "blue", (on && c.dmx_live()).then(|| Led::on(BLUE)));
        l
    }
}
