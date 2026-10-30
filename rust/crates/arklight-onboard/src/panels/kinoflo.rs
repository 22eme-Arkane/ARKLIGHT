//! Kino Flo (docs/recherche/menus/dedo_dmg_kino_litegear.md), portage de
//! `shared/onboard/panels/kinoflo.js` :
//!  - Celeb 250 / 450 LED DMX, panneau True Match (firmware 5.0) : OLED ambre, ⏻ (voyant vert / rouge),
//!    MENU, MODE, LOCK, préréglages 1–4, molette cliquable (fin / grossier) ;
//!  - Celeb LED 201 DMX (2015) : afficheur rouge 4 caractères, 5 touches Kelvin, DMX ADDRESS,
//!    KELVIN / DIM, LOCK / RESET, molette. Pas de menu.

use super::common::{AMBER, GREEN, RED, cct_range, pad, profile_item, serial_of};
use crate::fixture::obj;
use crate::js::{self, clamp, jv};
use crate::panel::{Brand, Holds, Drawn, ItemRef, ListOpts, Pc, act, draw_edit, draw_list, info_fn, num, opts, opts2, pick, sub};
use crate::screen::{DisplayKind, DisplaySpec, Element, Face, Led, Leds, Prim, col, key, knob, led, line, palette, rect_fill, set_led, text};
use arklight_core::library::FixtureType;
use serde_json::{Value, json};

// ================================================================== Celeb 250 / 450 (True Match)
const DEFAULT_PRESETS: [f64; 4] = [2700.0, 3200.0, 5000.0, 6500.0];
const MODES_TM: [&str; 5] = ["WHITE", "GELS/HUE", "RGB", "CIE xy", "FX"];

fn firmware(c: &Pc<TrueMatch>) -> &'static str {
    if c.ty.id.contains("250") { "CEL2" } else { "CEL4" }
}

fn yes_no_action(label: &str, f: fn(&mut Pc<TrueMatch>) -> Option<String>) -> ItemRef<TrueMatch> {
    pick(label, opts(&["NO", "YES"])).get(|_| json!("NO")).set(move |c, v| if v.as_str() == Some("YES") { f(c) } else { None }).rc()
}

fn tm_tree() -> Vec<ItemRef<TrueMatch>> {
    let stored = |label: &str, list: &[&str], key: &'static str, d: &str| pick(label, opts(list)).stored(key, json!(d)).rc();
    vec![
        sub(
            "GENERAL",
            vec![
                pick("MODE", opts(&MODES_TM))
                    .get(|c| json!(tm_mode(c)))
                    .set(|c, v| {
                        c.set_panel(obj([("mode", v.clone())]));
                        c.st.field = 0;
                        match v.as_str() {
                            Some("WHITE") => {
                                c.set_manual(obj([("mode", json!("cct"))]));
                                None
                            }
                            Some("GELS/HUE") => {
                                c.set_manual(obj([("mode", json!("hsi"))]));
                                None
                            }
                            _ => Some(format!("{} : mode non simulé dans ARKLIGHT", js::string(Some(&v)))),
                        }
                    })
                    .rc(),
                pick("TIMEOUT (Sec)", crate::panel::opts_num(&[0.0, 5.0, 10.0, 15.0, 20.0, 30.0])).stored("timeout", json!(15)).rc(),
                stored("SMOOTH", &["ON", "OFF"], "smooth", "ON"),
                stored("SCREEN", &["NORMAL", "ROTATED"], "screen", "NORMAL"),
                num("FREQ", 30.0, 300.0)
                    .fmt(|v, _| {
                        let n = js::number(Some(v));
                        format!("{} kHz{}", pad(n, 3), if n == 30.0 { " (Std)" } else { "" })
                    })
                    .stored("freq", json!(30))
                    .rc(),
            ],
        )
        .rc(),
        sub(
            "RESET",
            vec![
                yes_no_action("FACTORY RESET", |c| {
                    let first = c.ty.dmx.modes.first().map_or(Value::Null, |m| json!(m.id));
                    c.set_settings(obj([("panel", json!({})), ("dmxAddress", json!(1)), ("dmxMode", first), ("dimmingCurve", json!("square"))]));
                    c.set_manual(obj([("mode", json!("cct")), ("intensity", json!(0.1)), ("cct", json!(2700)), ("tint", json!(0))]));
                    Some("Dim = 10%, Kelvin = 2700K, G/M = 000, DMX = 001".into())
                }),
                yes_no_action("CLEAR SETTINGS", |c| {
                    c.set_manual(obj([("intensity", json!(0.1)), ("cct", json!(2700)), ("tint", json!(0))]));
                    Some("Réglages effacés".into())
                }),
                yes_no_action("CLEAR PRESETS", |c| {
                    c.set_panel(obj([("presets", Value::Null)]));
                    Some("Préréglages 2700 / 3200 / 5000 / 6500 K".into())
                }),
                stored("DEMO", &["OFF", "SCROLL"], "demo", "OFF"),
                info_fn("RDM", |c| format!("0x4B46{}", serial_of(c, 8, "0123456789ABCDEF"))).rc(),
            ],
        )
        .rc(),
        sub(
            "DMX",
            vec![
                num("DMX ADDR", 0.0, 512.0)
                    .get(|c| if js::truthy(Some(&c.panel_or("addr0", json!(false)))) { json!(0) } else { c.s("dmxAddress").cloned().unwrap_or(Value::Null) })
                    .set(|c, v| {
                        if js::number(Some(&v)) == 0.0 {
                            c.set_panel(obj([("addr0", json!(true))]));
                            c.set_settings(obj([("source", json!("manual"))]));
                            return Some("Adresse 0 : commande manuelle, câble branché".into());
                        }
                        c.set_panel(obj([("addr0", json!(false))]));
                        c.set_settings(obj([("dmxAddress", v), ("source", json!("auto"))]));
                        None
                    })
                    .fmt(|v, _| pad(js::number(Some(v)), 3))
                    .rc(),
                profile_item::<TrueMatch>("PERSONALITY", |m, _| m.name.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(14).collect()).rc(),
                stored("RDM", &["ENABLED", "DISABLED"], "rdm", "ENABLED"),
                pick("DIM CURVE", opts2(&[("square", "SQUARE"), ("linear", "LINEAR")]))
                    .get(|c| c.s("dimmingCurve").cloned().unwrap_or(Value::Null))
                    .set(|c, v| {
                        c.set_settings(obj([("dimmingCurve", v)]));
                        None
                    })
                    .rc(),
            ],
        )
        .rc(),
        sub(
            "DMX WIRELESS",
            vec![
                pick("ENABLE", opts(&["OFF", "ON"]))
                    .get(|c| c.panel_or("wireless", json!("OFF")))
                    .set(|c, v| {
                        let on = v.as_str() == Some("ON");
                        c.set_panel(obj([("wireless", v)]));
                        on.then(|| "Commandes manuelles désactivées tant que le sans-fil est actif".into())
                    })
                    .rc(),
                act("UNLINK - PRESS 3 SEC", |_| Some("STATUS: UNLINKED".into())).rc(),
                info_fn("STATUS", |c| (if c.panel_or("wireless", json!("OFF")).as_str() == Some("ON") { "LINKING" } else { "UNLINKED" }).into()).rc(),
                crate::panel::info("SIGNAL", "000%").rc(),
                stored("ANTENNA", &["#2 EXT", "#1 PCB"], "antenna", "#2 EXT"),
            ],
        )
        .rc(),
        stored("CAMERA LUT", &["KF Default", "C1  Arri Alexa", "C2  Sony Venice", "C3  Panavision DXL", "C4  Panasonic Varicam"], "lut", "KF Default"),
        stored("COLOR SPACE", &["KF Default", "rec 709 / sRGB", "P3 D65", "rec 2020"], "colorSpace", "KF Default"),
    ]
}

fn tm_mode(c: &Pc<TrueMatch>) -> String {
    c.panel_str("mode", if c.hsi() { "GELS/HUE" } else { "WHITE" })
}

fn tm_fields(c: &Pc<TrueMatch>) -> &'static [&'static str] {
    if tm_mode(c) == "GELS/HUE" { &["HUE", "SAT", "DIM", "KELVIN", "G/M"] } else { &["DIM", "KELVIN", "G/M"] }
}

fn tm_field(c: &Pc<TrueMatch>) -> &'static str {
    let f = tm_fields(c);
    f[c.st.field.rem_euclid(f.len() as i64) as usize]
}

fn gm(t: f64) -> String {
    let v = js::round(t.abs() * 100.0);
    if t == 0.0 || v == 0.0 { "000".into() } else { format!("{}{}", pad(v, 3), if t > 0.0 { "G" } else { "M" }) }
}

fn locked(c: &Pc<TrueMatch>) -> bool {
    js::truthy(Some(&c.panel_or("locked", json!(false))))
}

/// Préréglages (null : ceux d'usine).
fn tm_presets(c: &Pc<TrueMatch>) -> Vec<Value> {
    match c.panel("presets") {
        Some(p) if js::truthy(Some(p)) => p.as_array().cloned().unwrap_or_default(),
        _ => DEFAULT_PRESETS.iter().map(|k| json!({ "cct": jv(*k), "tint": 0 })).collect(),
    }
}

pub(crate) struct TrueMatch {
    field: i64,
    fine: bool,
    flash_k: f64,
}

fn save_preset(c: &mut Pc<TrueMatch>, id: &str) {
    let i = id[1..].parse::<usize>().unwrap_or(1) - 1;
    let mut p = tm_presets(c);
    let mut look = serde_json::Map::new();
    for k in ["cct", "tint", "intensity"] {
        if let Some(v) = c.m(k) {
            look.insert(k.into(), v.clone());
        }
    }
    if p.len() <= i {
        p.resize(i + 1, Value::Null);
    }
    p[i] = Value::Object(look);
    c.set_panel(obj([("presets", Value::Array(p))]));
    c.st.flash_k = c.t + 1.0;
    c.say(format!("Préréglage {} enregistré", i + 1));
}

impl Brand for TrueMatch {
    const SIZE: (f64, f64) = (128.0, 64.0);
    /// ⏻ ne commande que la source : l'écran et les menus restent actifs
    const DISPLAY_WHEN_OFF: bool = true;
    const ID: &'static str = "kino-truematch";

    fn init(_: &'static FixtureType) -> Self {
        TrueMatch { field: 0, fine: true, flash_k: 0.0 }
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "kino".into(),
            rows: vec![
                vec![
                    col(vec![key("power", "⏻", ""), key("menu", "MENU", "green")]),
                    Element::Display,
                    col(vec![led("lock", ""), key("lock", "LOCK", "red"), key("mode", "MODE", "green")]),
                    knob("knob", "", "push size=l cap=yellow"),
                ],
                vec![key("p1", "1", "small ring-red"), key("p2", "2", "small ring-yellow"), key("p3", "3", "small ring-lightblue"), key("p4", "4", "small ring-blue"), led("power", "ON")],
            ],
            display: Some(DisplaySpec { kind: DisplayKind::Draw, w: 128.0, h: 64.0, zoom: 2.0, palette: palette("#000000", "#ffb20a", "#5e4300", "#ffb20a", "#000000") }),
            note: "MODE : DIM → KELVIN → G/M. Molette : clic = fin / grossier. Préréglages : appui court = rappel, 3 s = enregistrer. LOCK : bloque tout (3 s = préréglages d’usine).".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        match id {
            "lock" => vec![(3.0, |c, _| {
                c.set_panel(obj([("presets", Value::Null)]));
                c.say("Préréglages d’usine restaurés");
            })],
            "mode" => vec![(1.0, |c, _| {
                if c.menu.is_some() && !c.menu_back() {
                    c.close_menu();
                }
            })],
            "menu" => vec![(3.0, |c, _| c.say("Raccourci : changer de mode dans GENERAL › MODE"))],
            "p1" | "p2" | "p3" | "p4" => vec![(3.0, save_preset)],
            _ => Vec::new(),
        }
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if id == "power" {
            let on = c.on();
            return c.set_state(obj([("power", json!(!on))]));
        }
        if id == "lock" {
            let now = !locked(c);
            c.set_panel(obj([("locked", json!(now))]));
            return c.say(if now { "Verrouillé : commandes manuelles désactivées" } else { "Déverrouillé" });
        }
        if locked(c) {
            return c.say("Verrouillé (LOCK)");
        }
        if id.len() == 2 && id.starts_with('p') && id.as_bytes()[1].is_ascii_digit() {
            let i = usize::from(id.as_bytes()[1] - b'0').saturating_sub(1);
            let p = tm_presets(c).get(i).cloned().unwrap_or(Value::Null);
            let mut ch = obj([("mode", json!("cct")), ("cct", p.get("cct").cloned().unwrap_or(Value::Null)), ("tint", jv(js::nn(p.get("tint"), 0.0)))]);
            if let Some(v) = p.get("intensity") {
                ch.insert("intensity".into(), v.clone());
            }
            c.set_manual(ch);
            c.set_panel(obj([("mode", json!("WHITE"))]));
            return;
        }
        match id {
            "menu" => {
                if c.menu.is_some() {
                    c.close_menu();
                } else {
                    let title = format!("TrueMatch 5.00 {}", firmware(c));
                    c.open_menu(tm_tree(), &title, false);
                }
            }
            "mode" => {
                if c.menu.is_some() {
                    return c.close_menu();
                }
                c.st.field = (c.st.field + 1) % tm_fields(c).len() as i64;
            }
            "knob" => {
                if c.menu.is_some() {
                    c.menu_enter_say();
                    return;
                }
                c.st.fine = !c.st.fine;
                let f = c.st.fine;
                c.say(if f { "Fine" } else { "Coarse" });
            }
            _ => {}
        }
    }

    fn turn(c: &mut Pc<Self>, _id: &str, steps: f64, mult: f64) {
        if locked(c) {
            return c.say("Verrouillé (LOCK)");
        }
        if c.menu.is_some() {
            return c.menu_move(steps, mult);
        }
        let f = tm_field(c);
        let k = if c.st.fine { 1.0 } else { 10.0 };
        let set = |c: &mut Pc<Self>, key: &str, v: f64| c.set_manual(obj([(key, jv(v))]));
        match f {
            "DIM" => set(c, "intensity", clamp(js::round(c.m_nn("intensity", 0.0) * 100.0 + steps * k) / 100.0, 0.0, 1.0)),
            "KELVIN" => {
                let (lo, hi) = if tm_mode(c) == "WHITE" { (2700.0, 6500.0) } else { cct_range(c, (2500.0, 9900.0)) };
                let step = if c.st.fine { 25.0 } else { 250.0 };
                set(c, "cct", clamp(js::round((c.m_or("cct", 5600.0) + steps * step) / 25.0) * 25.0, lo, hi));
            }
            "G/M" => set(c, "tint", clamp(js::round(c.m_or("tint", 0.0) * 100.0 + steps * k) / 100.0, -1.0, 1.0)),
            "HUE" => set(c, "hue", ((js::round(c.m_or("hue", 0.0)) + steps * k) % 360.0 + 360.0) % 360.0),
            "SAT" => set(c, "sat", clamp(js::round(c.m_nn("sat", 1.0) * 100.0 + steps * k) / 100.0, 0.0, 1.0)),
            _ => {}
        }
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let w = 128.0;
        if let Some(m) = &c.menu {
            let prims = if m.edit.is_some() { draw_edit(c, m, &ListOpts::default()) } else { draw_list(c, m, &ListOpts { title_size: 9.0, ..ListOpts::default() }) };
            return Drawn::of(prims);
        }
        let f = tm_field(c);
        let mut prims: Vec<Prim> = Vec::new();
        let addr0 = js::truthy(Some(&c.panel_or("addr0", json!(false))));
        let live = c.dmx_live() && !addr0;
        let label = |prims: &mut Vec<Prim>, txt: &str, x: f64, y: f64, sel: bool, center: bool| {
            if sel {
                let n = js::len16(txt) as f64;
                prims.push(rect_fill(if center { x - n * 3.3 - 2.0 } else { x - 1.0 }, y - 1.0, n * 6.6 + 4.0, 10.0, "fg"));
            }
            let t = text(x, y, txt, 8.0).b().inv(sel);
            prims.push(if center { t.c().into() } else { t.into() });
        };
        let dmx_text = format!("DMX: {}{}", if addr0 { "000".into() } else { pad(c.address(), 3) }, if live { " X" } else { "" });
        let k_blink = c.st.flash_k > c.t && (c.t * 4.0).floor() % 2.0 == 0.0;
        let n = js::num_str;
        if tm_mode(c) == "GELS/HUE" {
            label(&mut prims, "H:", 2.0, 2.0, f == "HUE", false);
            prims.push(text(16.0, 2.0, format!("{}°", pad(js::round(c.m_or("hue", 0.0)), 3)), 8.0).into());
            label(&mut prims, "SAT:", 70.0, 2.0, f == "SAT", false);
            prims.push(text(96.0, 2.0, pad(js::round(c.m_nn("sat", 1.0) * 100.0), 3), 8.0).into());
            prims.push(line(0.0, 13.0, w, 13.0, None));
            prims.push(text(2.0, 16.0, "GEL:", 8.0).into());
            prims.push(line(0.0, 27.0, w, 27.0, None));
            label(&mut prims, "G/ M:", 2.0, 30.0, f == "G/M", false);
            prims.push(text(34.0, 30.0, gm(c.m_or("tint", 0.0)), 8.0).into());
            prims.push(text(w - 2.0, 30.0, dmx_text, 8.0).r().into());
            prims.push(line(0.0, 41.0, w, 41.0, None));
            label(&mut prims, "DIM:", 2.0, 46.0, f == "DIM", false);
            prims.push(text(28.0, 46.0, n(js::round(c.m_nn("intensity", 0.0) * 100.0)), 8.0).into());
            label(&mut prims, "KELVIN:", 62.0, 46.0, f == "KELVIN", false);
            if !k_blink {
                prims.push(text(104.0, 46.0, n(js::round(c.m_or("cct", 6500.0))), 8.0).into());
            }
            return Drawn::of(prims);
        }
        label(&mut prims, "G/ M:", 2.0, 2.0, f == "G/M", false);
        prims.push(text(34.0, 2.0, gm(c.m_or("tint", 0.0)), 8.0).into());
        prims.push(text(w - 2.0, 2.0, dmx_text, 8.0).r().into());
        prims.push(line(0.0, 14.0, w, 14.0, None));
        prims.push(line(64.0, 14.0, 64.0, 62.0, None));
        let dim_label = if c.panel_or("smooth", json!("ON")).as_str() == Some("OFF") { "*DIM" } else { "DIM" };
        label(&mut prims, dim_label, 32.0, 20.0, f == "DIM", true);
        label(&mut prims, "KELVIN", 96.0, 20.0, f == "KELVIN", true);
        prims.push(text(32.0, 36.0, n(js::round(c.m_nn("intensity", 0.0) * 100.0)), 16.0).c().b().into());
        if !k_blink {
            prims.push(text(96.0, 36.0, n(js::round(c.m_or("cct", 6500.0))), 16.0).c().b().into());
        }
        if !c.st.fine {
            prims.push(text(2.0, 54.0, "COARSE", 6.0).col("dim").into());
        }
        Drawn::of(prims)
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        let mut l = Leds::new();
        set_led(&mut l, "lock", locked(c).then(|| Led::on(RED)));
        set_led(&mut l, "power", Some(Led::on(if c.on() { GREEN } else { RED })));
        l
    }
}

// ================================================================== Celeb LED 201 DMX
const PRESETS_201: [f64; 5] = [2700.0, 3200.0, 4500.0, 5500.0, 6500.0];

pub(crate) struct Celeb201 {
    /// ce que règle la molette : Kelvin ('K') ou intensité ('D')
    kd: char,
    fine: bool,
    edit: bool,
    flash_until: f64,
}

fn locked201(c: &Pc<Celeb201>) -> bool {
    js::truthy(Some(&c.panel_or("locked", json!(false))))
}

fn presets201(c: &Pc<Celeb201>) -> Vec<Value> {
    match c.panel("presets") {
        Some(p) if js::truthy(Some(p)) => p.as_array().cloned().unwrap_or_default(),
        _ => PRESETS_201.iter().map(|k| jv(*k)).collect(),
    }
}

fn save201(c: &mut Pc<Celeb201>, id: &str) {
    let i = usize::from(id.as_bytes()[1] - b'0');
    let mut p = presets201(c);
    if p.len() <= i {
        p.resize(i + 1, Value::Null);
    }
    p[i] = jv(js::round(c.m_or("cct", 5500.0)));
    c.set_panel(obj([("presets", Value::Array(p))]));
    c.st.flash_until = c.t + 1.0;
    c.say("Préréglage enregistré");
}

impl Brand for Celeb201 {
    const SIZE: (f64, f64) = (70.0, 24.0);
    const DISPLAY_WHEN_OFF: bool = true;
    const ID: &'static str = "kino-celeb201";

    fn init(_: &'static FixtureType) -> Self {
        Celeb201 { kd: 'K', fine: false, edit: false, flash_until: 0.0 }
    }

    fn face(_: &FixtureType) -> Face {
        let preset = |i: usize, label: &str, ring: &str| col(vec![led(&format!("pl{i}"), ""), key(&format!("k{i}"), label, &format!("small ring-{ring}"))]);
        Face {
            style: "kino".into(),
            rows: vec![
                vec![preset(0, "", "red"), preset(1, "TUNGSTEN", "orange"), preset(2, "", "lightblue"), preset(3, "", "blue"), preset(4, "DAYLIGHT", "darkblue")],
                vec![
                    col(vec![led("dmxled", ""), key("dmx", "DMX ADDRESS", "small")]),
                    col(vec![led("kdled", ""), key("kd", "KELVIN / DIM", "small")]),
                    Element::Display,
                    col(vec![led("lockled", ""), key("lock", "LOCK / RESET", "small")]),
                    col(vec![led("pwr", ""), key("power", "⏻", "small")]),
                    knob("knob", "", "push"),
                ],
            ],
            display: Some(DisplaySpec { kind: DisplayKind::Draw, w: 70.0, h: 24.0, zoom: 2.4, palette: palette("#100303", "#ff2b20", "#401010", "#ff2b20", "#100303") }),
            note: "Pas de menu. KELVIN / DIM bascule ce que règle la molette (clic = fin / grossier). DMX ADDRESS maintenu 3 s = adresse ; LOCK / RESET maintenu 5 s = réglages d’usine.".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        match id {
            "dmx" => vec![(3.0, |c, _| {
                c.st.edit = true;
                c.say("Adresse DMX : molette, puis DMX ADDRESS pour enregistrer");
            })],
            "lock" => vec![(5.0, |c, _| {
                c.set_panel(obj([("presets", Value::Null), ("locked", json!(false))]));
                c.set_settings(obj([("dmxAddress", json!(1))]));
                c.set_manual(obj([("mode", json!("cct")), ("cct", json!(5500)), ("intensity", json!(1))]));
                c.say("Réglages d’usine");
            })],
            "k0" | "k1" | "k2" | "k3" | "k4" => vec![(5.0, save201)],
            _ => Vec::new(),
        }
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if id == "power" {
            let on = c.on();
            return c.set_state(obj([("power", json!(!on))]));
        }
        if !c.on() {
            return;
        }
        if id == "lock" {
            let l = locked201(c);
            return c.set_panel(obj([("locked", json!(!l))]));
        }
        if id == "kd" {
            c.st.kd = if c.st.kd == 'K' { 'D' } else { 'K' };
            return;
        }
        if locked201(c) {
            return c.say("LOCK");
        }
        if id == "dmx" {
            if c.st.edit {
                c.st.edit = false;
                let a = pad(c.address(), 3);
                return c.say(format!("Adresse {a} enregistrée"));
            }
            return c.say("Maintenir DMX ADDRESS 3 s");
        }
        if id.len() == 2 && id.starts_with('k') && id.as_bytes()[1].is_ascii_digit() {
            let i = usize::from(id.as_bytes()[1] - b'0');
            let cct = presets201(c).get(i).cloned().unwrap_or(Value::Null);
            c.set_panel(obj([("active", json!(i))]));
            return c.set_manual(obj([("mode", json!("cct")), ("cct", cct)]));
        }
        if id == "knob" {
            c.st.fine = !c.st.fine;
        }
    }

    fn turn(c: &mut Pc<Self>, _id: &str, steps: f64, _mult: f64) {
        if !c.on() || locked201(c) {
            if locked201(c) {
                c.say("LOCK");
            }
            return;
        }
        if c.st.edit {
            let a = clamp(c.address() + steps * if c.st.fine { 1.0 } else { 10.0 }, 1.0, 511.0);
            return c.set_settings(obj([("dmxAddress", jv(a))]));
        }
        if c.st.kd == 'K' {
            // grossier : toute la plage en un tour (24 crans) ; fin : 8 tours
            let step = if c.st.fine { 3800.0 / 192.0 } else { 3800.0 / 24.0 };
            c.set_panel(obj([("active", json!(-1))]));
            let v = clamp(js::round((c.m_or("cct", 5500.0) + steps * step) / 25.0) * 25.0, 2700.0, 6500.0);
            return c.set_manual(obj([("mode", json!("cct")), ("cct", jv(v))]));
        }
        let v = clamp(js::round((c.m_nn("intensity", 0.0) + steps * if c.st.fine { 0.001 } else { 0.04 }) * 1000.0) / 1000.0, 0.0, 1.0);
        c.set_manual(obj([("intensity", jv(v))]));
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        if !c.on() {
            return Drawn::of(vec![text(35.0, 3.0, "V0.02", 15.0).c().b().font("seg").into()]);
        }
        let t = if locked201(c) {
            "LOCK".to_string()
        } else if c.st.edit {
            if (c.t * 3.0).floor() % 2.0 != 0.0 { js::pad_start(&js::string(c.s("dmxAddress")), 4, ' ') } else { String::new() }
        } else if c.st.kd == 'K' {
            if c.st.flash_until > c.t && (c.t * 4.0).floor() % 2.0 != 0.0 { String::new() } else { js::num_str(js::round(c.m_or("cct", 5500.0))) }
        } else {
            js::to_fixed(c.m_nn("intensity", 0.0) * 100.0, 1)
        };
        Drawn::of(vec![text(66.0, 3.0, t, 16.0).r().b().font("seg").sp(1.0).into()])
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        let active = c.panel_num("active", -1.0);
        let live = c.dmx_live();
        let on = c.on();
        let mut l = Leds::new();
        for i in 0..5 {
            set_led(&mut l, &format!("pl{i}"), (on && (live || active == f64::from(i))).then(|| Led::on(GREEN)));
        }
        set_led(&mut l, "kdled", on.then(|| Led::on(if c.st.kd == 'K' { AMBER } else { GREEN })));
        set_led(&mut l, "lockled", locked201(c).then(|| Led::on(AMBER)));
        set_led(&mut l, "dmxled", c.st.edit.then(|| Led::blink(GREEN, 2.0)));
        set_led(&mut l, "pwr", Some(Led::on(if on { GREEN } else { AMBER })));
        l
    }
}
