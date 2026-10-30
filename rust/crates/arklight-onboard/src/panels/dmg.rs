//! DMG Lumière (Rosco) SL1 MIX, MINI MIX, MAXI MIX : le contrôleur MIX (firmware 2.1.x)
//! (docs/recherche/menus/dedo_dmg_kino_litegear.md), portage de `shared/onboard/panels/dmg.js`.
//! Écran couleur 400×240 (fond noir, champ choisi en rouge), touches POWER, BACK (lock), GEL, WHITE,
//! COLOR, SETTINGS, NAVIGATION, LIBRARY, et la grosse molette rouge (tourner = valeur, appuyer =
//! préréglages rapides du champ).

use super::common::{cct_range, footprint, pad, serial_of};
use crate::fixture::obj;
use crate::js::{self, clamp, jv};
use crate::panel::{Brand, Holds, Drawn, ItemRef, ListOpts, Pc, act, draw_edit, draw_list, info, info_fn, num, opts, opts2, pick, sub};
use crate::screen::{DisplayKind, DisplaySpec, Element, Face, Prim, col, key, knob, palette, rect_fill, rect_stroke, text};
use arklight_core::library::FixtureType;
use serde_json::{Map, Value, json};

const RED: &str = "#e82028";
const GELS: [(&str, &str, f64); 8] = [
    ("R2002", "Storaro Orange", 30.0),
    ("R02", "Bastard Amber", 35.0),
    ("E201", "Full CT Blue", 210.0),
    ("R80", "Primary Blue", 228.0),
    ("R26", "Light Red", 355.0),
    ("E128", "Bright Pink", 320.0),
    ("E121", "Leaf Green", 110.0),
    ("R2005", "Storaro Cyan", 188.0),
];
const EFFECTS: [&str; 8] = ["Emergency Vehicle (B)", "Emergency Vehicle (B+R)", "Emergency Vehicle (B+W)", "Emergency Vehicle (R+W)", "Emergency Vehicle (R)", "Firelight", "TV", "Paparazzi"];
const SOURCES: [&str; 10] = ["Green Screen", "Blue Screen", "Xenon Headlamp", "Candle Flame", "Frosty Moonlight", "HP Sodium", "LP Sodium", "Tungsten Domestic", "Mercury Vapor", "Sodium Vapor"];
const DATA_IN: [&str; 5] = ["XLR", "CRMX", "LAN", "WiFi 5.8G", "WiFi 2.4G"];

pub(crate) struct DmgMix {
    field: i64,
    tab: i64,
    lib: i64,
    dmx_field: i64,
    quick: i64,
    locked: bool,
    blackout: bool,
    last_int: Option<f64>,
}

fn is_maxi(c: &Pc<DmgMix>) -> bool {
    c.ty.id == "dmg-lumiere.maxi-mix"
}

/// Champs de chaque mode, dans l'ordre de la touche NAVIGATION (coins de l'écran).
fn fields(mode: &str) -> &'static [&'static str] {
    match mode {
        "WHITE" => &["INT", "CCT", "G/M"],
        "COLOR" | "GEL" => &["INT", "CCT", "HUE", "SAT"],
        _ => &[],
    }
}

fn quick(f: &str) -> &'static [f64] {
    match f {
        "INT" => &[0.25, 0.5, 0.75, 1.0],
        "CCT" => &[1700.0, 3200.0, 4300.0, 5600.0, 10000.0],
        "HUE" => &[0.0, 60.0, 120.0, 240.0, 300.0],
        "SAT" => &[0.0, 0.5, 1.0],
        "G/M" => &[0.0],
        _ => &[],
    }
}

/// Champ choisi (`FIELDS[mode][field % n]`).
fn field_of(c: &Pc<DmgMix>) -> Option<&'static str> {
    let f = fields(&mode(c));
    (!f.is_empty()).then(|| f[(c.st.field.rem_euclid(f.len() as i64)) as usize])
}

fn universe(c: &Pc<DmgMix>) -> f64 {
    js::number(c.s("universe"))
}

fn settings_tree() -> Vec<ItemRef<DmgMix>> {
    let stored = |label: &str, list: &[&str], key: &'static str, d: &str| pick(label, opts(list)).stored(key, json!(d)).rc();
    vec![
        pick("Bluetooth", opts(&["BT", "OFF"])).stored("bt", json!("BT")).fmt(|v, _| js::string(Some(v))).rc(),
        act("DMX Mode", |c: &mut Pc<DmgMix>| {
            c.screen = "dmx";
            c.set_menu(None);
            c.st.dmx_field = 1;
            None
        })
        .rc(),
        stored("DMX profile revision", &["2.0", "2.1"], "dmxRev", "2.1"),
        sub(
            "Art-Net / Wi-Fi / RDM",
            vec![
                stored("RDM", &["ON", "OFF"], "rdm", "ON"),
                stored("Wi-Fi", &["2.4 GHz", "5.8 GHz"], "wifi", "5.8 GHz"),
                num("Art-Net Universe", 0.0, 15.0)
                    .get(|c| jv((universe(c) - 1.0) % 16.0))
                    .set(|c, v| {
                        let u = universe(c);
                        c.set_settings(obj([("universe", jv(((u - 1.0) / 16.0).floor() * 16.0 + js::number(Some(&v)) + 1.0))]));
                        None
                    })
                    .rc(),
                num("Art-Net Sub-Net", 0.0, 15.0)
                    .get(|c| jv(((universe(c) - 1.0) / 16.0).floor() % 16.0))
                    .set(|c, v| {
                        let u = universe(c);
                        c.set_settings(obj([("universe", jv(js::number(Some(&v)) * 16.0 + ((u - 1.0) % 16.0) + 1.0))]));
                        None
                    })
                    .rc(),
                info("IP", "2 . 0 . 0 . 1").rc(),
                info("Mask", "255 . 0 . 0 . 0").rc(),
            ],
        )
        .rc(),
        pick("Select Zone", opts(&["Zone 1+2", "Zone 1", "Zone 2"])).stored("zone", json!("Zone 1+2")).when(is_maxi).rc(),
        sub(
            "Information",
            vec![
                info_fn("Serial #", |c| format!("MIX-1809-{}", serial_of(c, 4, "0123456789"))).rc(),
                info("Firmware Version", "V2.13").rc(),
                info("Light Hours", "6.6").rc(),
                info_fn("Light Version", |c| format!("1{}", if is_maxi(c) { 3 } else if c.ty.id.contains("mini") { 1 } else { 2 })).rc(),
                info_fn("Light Temperature", |c| format!("{}°C", js::to_fixed(35.0 + c.m_nn("intensity", 0.5) * 20.0, 1))).rc(),
                info("Error(s)", "None").rc(),
            ],
        )
        .rc(),
        sub(
            "Display settings",
            vec![stored("Display auto OFF on lock", &["YES", "NO"], "autoOff", "YES"), stored("Display rotation", &["NO", "YES"], "rotation", "NO"), stored("Screen brightness", &["LOW", "MED", "HIGH"], "brightness", "HIGH")],
        )
        .rc(),
        pick("Dimming Curve", opts2(&[("linear", "Linear"), ("square", "Square Law"), ("sCurve", "S Curve"), ("tungsten", "Tungsten Emulate")]))
            .get(|c| if js::truthy(c.panel("tungstenCurve")) { json!("tungsten") } else { c.s("dimmingCurve").cloned().unwrap_or(Value::Null) })
            .set(|c, v| {
                let t = v.as_str() == Some("tungsten");
                c.set_panel(obj([("tungstenCurve", json!(t))]));
                c.set_settings(obj([("dimmingCurve", if t { json!("square") } else { v })]));
                t.then(|| "Tungsten Emulate : la teinte n’est pas simulée (courbe seulement)".into())
            })
            .rc(),
        stored("Gel Sorting", &["Name", "Color", "Number"], "gelSort", "Number"),
        pick("Factory Settings", opts(&["Keep myMIX Library", "Erase myMIX Library"]))
            .get(|_| json!(""))
            .set(|c, v| {
                let mut keep = Map::new();
                if v.as_str() == Some("Keep myMIX Library") {
                    keep.insert("mymix".into(), c.panel_or("mymix", json!([])));
                }
                let first = c.ty.dmx.modes.first().map_or(Value::Null, |m| json!(m.id));
                c.set_settings(obj([("panel", Value::Object(keep)), ("dmxAddress", json!(1)), ("dimmingCurve", json!("linear")), ("dmxMode", first)]));
                c.set_manual(obj([("mode", json!("cct")), ("intensity", json!(0.5)), ("cct", json!(5600)), ("tint", json!(0))]));
                Some("Restore Factory Settings".into())
            })
            .rc(),
    ]
}

fn mode(c: &Pc<DmgMix>) -> String {
    c.panel_str("mode", if c.hsi() { "COLOR" } else { "WHITE" })
}

fn field_value(c: &Pc<DmgMix>, f: &str) -> String {
    let n = js::num_str;
    let gel = mode(c) == "GEL";
    match f {
        "INT" => format!("{}%", js::to_fixed(c.m_nn("intensity", 0.0) * 100.0, 1)),
        "CCT" => format!("{}°K", n(js::round(c.m_or("cct", 5600.0)))),
        "G/M" => {
            let g = js::round(c.m_or("tint", 0.0) * 100.0);
            if g == 0.0 {
                "0".into()
            } else if g > 0.0 {
                format!("{}G", n(g))
            } else {
                format!("{}M", n(-g))
            }
        }
        "HUE" => {
            if gel {
                format!("{}°", c.panel_str("gelHue", "0"))
            } else {
                format!("{}°", n(js::round(c.m_or("hue", 0.0))))
            }
        }
        "SAT" => {
            if gel {
                format!("{}%", c.panel_str("gelSat", "0"))
            } else {
                format!("{}%", n(js::round(c.m_nn("sat", 1.0) * 100.0)))
            }
        }
        _ => String::new(),
    }
}

fn change_field(c: &mut Pc<DmgMix>, f: &str, steps: f64, mult: f64) {
    let md = mode(c);
    let set = |c: &mut Pc<DmgMix>, k: &str, v: f64| c.set_manual(obj([(k, jv(v))]));
    match f {
        "INT" => return set(c, "intensity", clamp(js::round((c.m_nn("intensity", 0.0) + steps * 0.001 * mult) * 1000.0) / 1000.0, 0.0, 1.0)),
        "CCT" => {
            let (lo, hi) = cct_range(c, (1700.0, 10000.0));
            if md == "GEL" {
                let warm = c.m_or("cct", 3200.0) >= 4400.0;
                let v = if warm {
                    if steps < 0.0 { 3200.0 } else { 5600.0 }
                } else if steps > 0.0 {
                    5600.0
                } else {
                    3200.0
                };
                return set(c, "cct", v);
            }
            return set(c, "cct", clamp(js::round((c.m_or("cct", 5600.0) + steps * 50.0 * mult.min(4.0)) / 50.0) * 50.0, lo, hi));
        }
        "G/M" => return set(c, "tint", clamp(js::round(c.m_or("tint", 0.0) * 100.0 + steps) / 100.0, -1.0, 1.0)),
        _ => {}
    }
    if md == "GEL" {
        if f == "HUE" {
            let v = clamp(c.panel_num("gelHue", 0.0) + steps, -10.0, 10.0);
            return c.set_panel(obj([("gelHue", jv(v))]));
        }
        let v = clamp(c.panel_num("gelSat", 0.0) + steps, -20.0, 20.0);
        return c.set_panel(obj([("gelSat", jv(v))]));
    }
    if f == "HUE" {
        return set(c, "hue", ((js::round(c.m_or("hue", 0.0)) + steps * mult.min(4.0)) % 360.0 + 360.0) % 360.0);
    }
    if f == "SAT" {
        set(c, "sat", clamp(js::round(c.m_nn("sat", 1.0) * 100.0 + steps) / 100.0, 0.0, 1.0));
    }
}

/// « R2002 » → « 2002 »
fn gel_number(n: &str) -> &str {
    n.trim_start_matches(|ch: char| !ch.is_ascii_digit())
}

/// Gel choisi (approché : teinte de la gélatine sur une base 3200 / 5600 K, non calibrée).
fn apply_gel(c: &mut Pc<DmgMix>, i: f64) -> String {
    let n = GELS.len() as f64;
    let k = ((i + n) % n) as usize;
    let (num_, name, hue) = GELS[k];
    c.set_panel(obj([("gel", json!(k)), ("mode", json!("GEL"))]));
    c.set_manual(obj([("mode", json!("hsi")), ("hue", jv(hue)), ("sat", json!(0.6))]));
    format!("{name} {}", gel_number(num_))
}

fn gel_index(c: &Pc<DmgMix>) -> f64 {
    c.panel_num("gel", 0.0)
}

fn mode_screen(c: &Pc<DmgMix>) -> Vec<Prim> {
    let (w, h) = (160.0, 96.0);
    let md = mode(c);
    let cur = field_of(c).unwrap_or("");
    let mut prims: Vec<Prim> = vec![text(w / 2.0, 4.0, md.clone(), 10.0).c().b().into()];
    let mut corner = |f: &str, x: f64, y: f64, right: bool| {
        let sel = f == cur;
        let cw = 46.0;
        let bx = if right { x - cw } else { x };
        if sel {
            prims.push(rect_fill(bx, y, cw, 24.0, RED));
        }
        prims.push(text(bx + cw / 2.0, y + 2.0, f, 8.0).c().b().into());
        prims.push(text(bx + cw / 2.0, y + 12.0, field_value(c, f), 9.0).c().b().into());
    };
    corner("INT", 3.0, 3.0, false);
    corner("CCT", w - 3.0, 3.0, true);
    if md == "WHITE" {
        corner("G/M", w - 3.0, h - 27.0, true);
    } else {
        corner("HUE", 3.0, h - 27.0, false);
        corner("SAT", w - 3.0, h - 27.0, true);
    }
    // centre : grande valeur, spectre (COLOR) ou gélatine (GEL)
    if md == "COLOR" {
        let (x0, sw) = (30.0, 100.0);
        let cols = ["#ff7a1a", "#ffd21a", "#ffffff", "#8cc8ff", "#2f6bff", "#8a3dff", "#c4123c"];
        let n = cols.len() as f64;
        for (k, col) in cols.iter().enumerate() {
            prims.push(rect_fill(x0 + (k as f64 * sw) / n, 40.0, sw / n + 0.5, 8.0, col));
        }
        prims.push(rect_fill(x0 + (sw * c.m_or("hue", 0.0)) / 360.0 - 1.0, 37.0, 2.0, 14.0, "fg"));
    } else if md == "GEL" {
        let g = usize::try_from(gel_index(c) as i64).ok().and_then(|i| GELS.get(i));
        prims.push(rect_fill(50.0, 32.0, 60.0, 6.0, "#f8b828"));
        prims.push(text(w / 2.0, 42.0, g.map_or("undefined", |g| g.1), 9.0).c().b().into());
        prims.push(text(w / 2.0, 54.0, g.map_or("", |g| gel_number(g.0)), 8.0).c().col("dim").into());
    } else {
        prims.push(text(w / 2.0, 38.0, field_value(c, "INT"), 16.0).c().b().into());
    }
    if c.panel_or("bt", json!("BT")).as_str() == Some("BT") {
        prims.push(text(w / 2.0, h - 14.0, "ᛒ", 10.0).c().col("#5aa9ff").into());
    }
    if c.st.locked {
        prims.push(text(w / 2.0, 18.0, "LOCKED", 8.0).c().col(RED).into());
    }
    prims
}

fn dmx_page(c: &Pc<DmgMix>) -> Vec<Prim> {
    let fields = ["DATA IN", "ADDRESS", "PROFILE", "DATA OUT"];
    let modes = &c.ty.dmx.modes;
    let mode_id = c.s("dmxMode");
    let i = modes.iter().position(|m| js::strict_eq(Some(&json!(m.id)), mode_id)).unwrap_or(0);
    let name = modes.get(i).map_or("", |m| m.name.as_str());
    // « 1: White 8-bit » → « White 8-bit »
    let short = {
        let digits = name.trim_start_matches(|ch: char| ch.is_ascii_digit());
        match digits.strip_prefix(':') {
            Some(rest) if digits.len() < name.len() => rest.trim_start(),
            _ => name,
        }
    };
    let a = c.address();
    let values = [c.panel_str("dataIn", "XLR"), format!("{}-{}", js::num_str(a), js::num_str(a + footprint(c) - 1.0)), format!("{}:{short}", i + 1), c.panel_str("dataOut", "None")];
    let mut prims: Vec<Prim> = Vec::new();
    let mut bx = |k: usize, x: f64, y: f64, w: f64| {
        if c.st.dmx_field == k as i64 {
            prims.push(rect_fill(x, y, w, 22.0, RED));
        } else {
            prims.push(rect_stroke(x, y, w, 22.0, "dim"));
        }
        prims.push(text(x + w / 2.0, y + 2.0, fields[k], 7.0).c().b().into());
        prims.push(text(x + w / 2.0, y + 11.0, values[k].clone(), 8.0).c().w(w - 4.0).into());
    };
    bx(0, 2.0, 2.0, 46.0);
    bx(1, 52.0, 2.0, 46.0);
    bx(2, 102.0, 2.0, 56.0);
    let n = js::num_str;
    prims.push(text(6.0, 32.0, format!("INT:{}%", js::to_fixed(c.m_nn("intensity", 0.0) * 100.0, 1)), 8.0).into());
    prims.push(text(84.0, 32.0, format!("CCT:{} K", n(js::round(c.m_or("cct", 3200.0)))), 8.0).into());
    prims.push(text(6.0, 44.0, format!("HUE:{}°", n(js::round(c.m_or("hue", 0.0)))), 8.0).into());
    prims.push(text(84.0, 44.0, format!("SAT:{}%", n(js::round(c.m_nn("sat", 0.0) * 100.0))), 8.0).into());
    let mut bx3 = |x: f64, y: f64, w: f64| {
        if c.st.dmx_field == 3 {
            prims.push(rect_fill(x, y, w, 22.0, RED));
        } else {
            prims.push(rect_stroke(x, y, w, 22.0, "dim"));
        }
        prims.push(text(x + w / 2.0, y + 2.0, fields[3], 7.0).c().b().into());
        prims.push(text(x + w / 2.0, y + 11.0, values[3].clone(), 8.0).c().w(w - 4.0).into());
    };
    bx3(2.0, 70.0, 46.0);
    let live = c.dmx_live();
    prims.push(text(92.0, 72.0, if live { "DMX OK" } else { "NO DMX" }, 9.0).c().b().col(if live { "#3ddc84" } else { RED }).into());
    prims.push(text(156.0, 72.0, format!("Univ: {}", n(js::or(c.s("universe"), 1.0) - 1.0)), 7.0).r().into());
    prims.push(text(156.0, 82.0, "IP: 2.0.0.1", 7.0).r().col(RED).into());
    prims
}

fn library_items(c: &Pc<DmgMix>, tab: i64) -> Vec<String> {
    let p4 = |i: usize| pad(i as f64, 4);
    match tab {
        0 => c.panel_or("mymix", json!([])).as_array().map_or_else(Vec::new, |l| l.iter().enumerate().map(|(i, p)| format!("{}:{}:{}", p4(i + 1), i + 1, js::string(p.get("name")))).collect()),
        1 => GELS.iter().enumerate().map(|(i, g)| format!("{}:{}:{}", p4(i), i + 100, g.1)).collect(),
        2 => EFFECTS.iter().enumerate().map(|(i, e)| format!("{}:{}:{e}", p4(i), 801 + i)).collect(),
        _ => SOURCES.iter().enumerate().map(|(i, s)| format!("{}:{}:{s}", p4(i), 701 + i)).collect(),
    }
}

fn library_screen(c: &Pc<DmgMix>) -> Vec<Prim> {
    let w = 160.0;
    let tabs = ["myMIX", "Rosco Gels", "Effects", "Source Match"];
    let tab = c.st.tab;
    let mut prims: Vec<Prim> = Vec::new();
    for (k, t) in tabs.iter().enumerate() {
        let x = (k % 2) as f64 * 80.0;
        let y = (k / 2) as f64 * 14.0;
        prims.push(rect_fill(x + 1.0, y + 1.0, 78.0, 12.0, if k as i64 == tab { RED } else { "#1b1e24" }));
        prims.push(text(x + 40.0, y + 3.0, *t, 7.0).c().b().into());
    }
    let list = library_items(c, tab);
    let n = list.len() as f64;
    let cur = clamp(c.st.lib as f64, 0.0, (n - 1.0).max(0.0)) as i64;
    let first = clamp((cur - 2) as f64, 0.0, (n - 5.0).max(0.0)) as i64;
    for (k, txt) in list.iter().skip(first as usize).take(5).enumerate() {
        let y = 32.0 + k as f64 * 12.0;
        if first + k as i64 == cur {
            prims.push(rect_fill(1.0, y, 3.0, 10.0, "fg"));
        }
        prims.push(text(7.0, y + 1.0, txt.clone(), 7.5).w(w - 10.0).into());
    }
    if list.is_empty() {
        prims.push(text(w / 2.0, 50.0, "(vide)", 8.0).c().col("dim").into());
    }
    prims
}

fn preset_name(c: &Pc<DmgMix>) -> String {
    let n = js::num_str;
    let md = mode(c);
    let int = n(js::round(c.m_nn("intensity", 0.0) * 1000.0));
    if md == "COLOR" {
        return format!("Color.{int}.{}.{}.{}", n(js::round(c.m_or("cct", 3200.0))), n(js::round(c.m_nn("sat", 1.0) * 100.0)), n(js::round(c.m_or("hue", 0.0))));
    }
    if md == "GEL" {
        return format!("Gel.{int}.{}.{}.{}.{}", n(js::round(c.m_or("cct", 3200.0))), c.panel_str("gelHue", "0"), c.panel_str("gelSat", "0"), c.panel_str("gel", "0"));
    }
    let g = js::round(c.m_or("tint", 0.0) * 100.0);
    format!("White.{int}.{}.{}", n(js::round(c.m_or("cct", 5600.0))), if g >= 0.0 { format!("{}G", n(g)) } else { format!("{}M", n(-g)) })
}

impl Brand for DmgMix {
    const SIZE: (f64, f64) = (160.0, 96.0);
    const ID: &'static str = "dmg-mix";

    fn init(_: &'static FixtureType) -> Self {
        DmgMix { field: 0, tab: 0, lib: 0, dmx_field: 1, quick: 0, locked: false, blackout: false, last_int: None }
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "dmg".into(),
            rows: vec![vec![
                col(vec![key("power", "⏻", ""), key("back", "‹  lock", "")]),
                col(vec![key("gel", "GEL", "small"), key("white", "WHITE", "small"), key("color", "COLOR", "small")]),
                Element::Display,
                col(vec![key("settings", "SETTINGS", "small"), key("nav", "⟲", "small"), key("library", "LIBRARY  save/delete", "small")]),
                knob("knob", "", "cap=red push size=l"),
            ]],
            display: Some(DisplaySpec { kind: DisplayKind::Draw, w: 160.0, h: 96.0, zoom: 1.6, palette: palette("#000000", "#ffffff", "#788898", RED, "#000000") }),
            note: "⟲ (NAVIGATION) : passer d’un coin à l’autre. Molette rouge : tourner = valeur, appuyer = préréglages rapides. LIBRARY maintenu = enregistrer dans myMIX. BACK maintenu = verrou.".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        match id {
            "back" => vec![(1.0, |c, _| {
                c.st.locked = !c.st.locked;
                let l = c.st.locked;
                c.say(if l { "Commandes verrouillées" } else { "Commandes déverrouillées" });
            })],
            "power" => vec![(1.0, |c, _| c.set_state(obj([("power", json!(false))])))],
            "library" => vec![(1.0, |c, _| {
                if c.screen == "library" {
                    return c.say("Delete Preset ? (non simulé)");
                }
                let mut list = c.panel_or("mymix", json!([])).as_array().cloned().unwrap_or_default();
                let name = preset_name(c);
                let look = c.fx.st("manual").cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
                list.push(json!({ "name": name, "look": look, "mode": mode(c) }));
                c.set_panel(obj([("mymix", Value::Array(list))]));
                c.say(format!("Saved in myMIX : {name}"));
            })],
            "nav" => vec![(1.0, |c, _| c.say("CRMX : Link / Unlink (pas d’émetteur)"))],
            _ => Vec::new(),
        }
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if id == "power" {
            if !c.on() {
                return c.set_state(obj([("power", json!(true))]));
            }
            c.st.blackout = !c.st.blackout; // appui court allumé : noir rapide
            if c.st.blackout {
                // intensité retenue avant le noir, rendue à l'appui suivant
                c.st.last_int = c.m("intensity").filter(|v| !v.is_null()).map(|v| js::number(Some(v)));
            }
            let v = if c.st.blackout { 0.0 } else { c.st.last_int.unwrap_or(0.5) };
            c.set_manual(obj([("intensity", jv(v))]));
            return;
        }
        if !c.on() {
            return;
        }
        if c.st.locked {
            return c.say("Verrouillé : BACK maintenu pour déverrouiller");
        }
        match id {
            "white" | "color" | "gel" => {
                let md = id.to_uppercase();
                c.set_panel(obj([("mode", json!(md))]));
                c.set_menu(None);
                c.screen = "home";
                c.st.field = 0;
                if md == "WHITE" {
                    c.set_manual(obj([("mode", json!("cct"))]));
                } else if md == "COLOR" {
                    c.set_manual(obj([("mode", json!("hsi"))]));
                } else {
                    let g = gel_index(c);
                    let m = apply_gel(c, g);
                    c.say(m);
                }
            }
            "settings" => {
                if c.screen == "menu" {
                    c.close_menu();
                } else {
                    c.open_menu(settings_tree(), "Settings", false);
                }
            }
            "library" => {
                c.set_menu(None);
                c.screen = if c.screen == "library" { "home" } else { "library" };
            }
            "nav" => {
                if c.screen == "library" {
                    c.st.tab = (c.st.tab + 1) % 4;
                    c.st.lib = 0;
                    return;
                }
                if c.screen == "dmx" {
                    c.st.dmx_field = (c.st.dmx_field + 1) % 4;
                    return;
                }
                let n = fields(&mode(c)).len().max(1) as i64;
                c.st.field = (c.st.field + 1) % n;
            }
            "back" => {
                if c.screen == "menu" {
                    if !c.menu_back() {
                        c.close_menu();
                    }
                    return;
                }
                c.set_menu(None);
                c.screen = "home";
            }
            "knob" => {
                if c.screen == "menu" {
                    c.menu_enter_say();
                    return;
                }
                if c.screen == "library" {
                    let (tab, i) = (c.st.tab, c.st.lib);
                    c.screen = "home";
                    if tab == 0 {
                        let p = c.panel_or("mymix", json!([])).get(i as usize).cloned().filter(|p| js::truthy(Some(p)));
                        if let Some(p) = p {
                            c.set_manual(p.get("look").and_then(Value::as_object).cloned().unwrap_or_default());
                            c.set_panel(obj([("mode", p.get("mode").cloned().unwrap_or(Value::Null))]));
                        }
                        return;
                    }
                    if tab == 1 {
                        let m = apply_gel(c, i as f64);
                        return c.say(m);
                    }
                    let item = library_items(c, tab).get(i as usize).cloned().unwrap_or_default();
                    c.set_panel(obj([("libItem", json!(item))]));
                    return c.say(format!("{} (non simulé dans ARKLIGHT)", item.rsplit(':').next().unwrap_or("")));
                }
                if c.screen == "dmx" {
                    return;
                }
                // préréglages rapides du champ choisi
                let Some(f) = field_of(c) else { return };
                let list = quick(f);
                if list.is_empty() {
                    return;
                }
                c.st.quick = (c.st.quick + 1) % list.len() as i64;
                let v = jv(list[c.st.quick as usize]);
                let k = match f {
                    "INT" => "intensity",
                    "CCT" => "cct",
                    "HUE" => "hue",
                    "SAT" => "sat",
                    _ => "tint",
                };
                c.set_manual(obj([(k, v)]));
            }
            _ => {}
        }
    }

    fn turn(c: &mut Pc<Self>, _id: &str, steps: f64, mult: f64) {
        if !c.on() {
            return;
        }
        if c.st.locked {
            return c.say("Verrouillé");
        }
        if c.screen == "menu" {
            return c.menu_move(steps, mult);
        }
        if c.screen == "library" {
            let n = library_items(c, c.st.tab).len() as f64;
            c.st.lib = clamp(c.st.lib as f64 + steps, 0.0, (n - 1.0).max(0.0)) as i64;
            return;
        }
        if c.screen == "dmx" {
            match c.st.dmx_field {
                0 => {
                    let cur = c.panel_or("dataIn", json!("XLR"));
                    let i = DATA_IN.iter().position(|d| cur.as_str() == Some(d)).map_or(-1.0, |i| i as f64);
                    let k = ((i + steps) % 5.0 + 5.0) % 5.0;
                    c.set_panel(obj([("dataIn", json!(DATA_IN[k as usize]))]));
                }
                1 => {
                    let a = clamp(c.address() + steps * if mult > 2.0 { 10.0 } else { 1.0 }, 1.0, 513.0 - footprint(c));
                    c.set_settings(obj([("dmxAddress", jv(a))]));
                }
                2 => {
                    let modes = &c.ty.dmx.modes;
                    let i = modes.iter().position(|m| js::strict_eq(Some(&json!(m.id)), c.s("dmxMode"))).map_or(-1.0, |i| i as f64);
                    let k = clamp(i + steps, 0.0, (modes.len() as f64 - 1.0).max(0.0)) as usize;
                    let id = modes.get(k).map_or(Value::Null, |m| json!(m.id));
                    c.set_settings(obj([("dmxMode", id)]));
                }
                _ => {
                    let out = if c.panel_or("dataOut", json!("None")).as_str() == Some("None") { "CRMX" } else { "None" };
                    c.set_panel(obj([("dataOut", json!(out))]));
                }
            }
            return;
        }
        if let Some(f) = field_of(c) {
            change_field(c, f, steps, mult);
        }
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        match c.screen {
            "menu" => {
                let o = ListOpts { w: Some(160.0), h: Some(96.0), rows: 4, row_h: 18.0, size: 10.0, sel_fill: Some(RED), ..ListOpts::default() };
                let prims = match &c.menu {
                    Some(m) if m.edit.is_some() => draw_edit(c, m, &o),
                    Some(m) => draw_list(c, m, &ListOpts { max_value: 10, ..o }),
                    None => Vec::new(),
                };
                Drawn::of(prims)
            }
            "library" => Drawn::of(library_screen(c)),
            "dmx" => Drawn::of(dmx_page(c)),
            _ => Drawn { prims: mode_screen(c), dim: c.st.locked && c.panel_or("autoOff", json!("YES")).as_str() == Some("YES") },
        }
    }
}
