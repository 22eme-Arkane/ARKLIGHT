//! Panneaux Hive Lighting (docs/recherche/menus/aputure_hive.md), portage de
//! `shared/onboard/panels/hive.js` :
//!  - Wasp 100-C / Hornet 200-C (firmware 3.0.8) : écran OLED S.H.O.T. (S saturation, H teinte,
//!    D intensité, K température), molettes SELECT et ADJUST (cliquables), interrupteurs marche et
//!    DMX / Local, voyants alimentation et DMX ;
//!  - Super Hornet 575-C : ballast avec écran LCD, SELECT + MENU, ADJUST + BACK, voyant STATUS.
//!
//! Sur un Hive, saturation 0 = blanc à la température K ; au-dessus, couleur (teinte, saturation).

use super::common::{AMBER, BLUE, GREEN, cct_range, pad};
use crate::fixture::obj;
use crate::js::{self, clamp, jv};
use crate::panel::{Brand, Holds, Drawn, ItemRef, Kind, ListOpts, Pc, act, draw_edit, draw_list, info, num, opts, pick, pick_dyn, sub};
use crate::screen::{DisplayKind, DisplaySpec, Element, Face, Led, Leds, Prim, col, key, knob, led, line, palette, rect_fill, rect_stroke, set_led, text};
use arklight_core::library::FixtureType;
use serde_json::{Value, json};

const EFFECTS: [&str; 14] = ["Cop Car 1", "Cop Car 2", "Emergency 1", "Emergency 2", "Emergency 3", "Club Lights", "Papparazzi", "Color Chase", "Fire", "FireWorks", "Pulsing", "TV", "Lightning", "Strobe"];
const SOURCES: [&str; 40] = [
    "VioletGloStk", "Pink Glo Stk", "YellowGloStk", "Red Glo Stk", "GreenGloStk", "Blue Glo Stk", "Red Traffic", "YelloTraffic", "GreenTraffic", "AmberCaution", "Road Flare", "Blow Torch", "Luminescence", "Comp Monitor", "Mobile Phone", "SunBlueHour", "Sun Overcast", "Sun Direct", "Gaslight", "Candlelight",
    "Antique Bulb", "Warm Antique", "Xmas Lights", "Night Light", "Heat Lamp", "Grow Light", "Tungsten", "Incandescent", "Halogen", "HMI", "HP Sodium", "LP Sodium", "MercuryVapor", "MetalHallide", "Ceramic", "Carbon Arc", "Xenon", "Blacklight", "CoolWhiteFlo", "WarmWhiteFlo",
];
const CORRECTIVE: [&str; 24] = [
    "+GRN DAY", "1/2 +GRN DAY", "1/4 +GRN DAY", "1/8 +GRN DAY", "+GRN TUN", "1/2 +GRN TUN", "1/4 +GRN TUN", "1/8 +GRN TUN", "-GRN DAY", "1/2 -GRN DAY", "1/4 -GRN DAY", "1/8 -GRN DAY", "-GRN TUN", "1/2 -GRN TUN", "1/4 -GRN TUN", "1/8 -GRN TUN", "CTSTRAW DAY", "1/2 CTS DAY", "1/4 CTS DAY", "1/8 CTS DAY", "CTSTRAW TUN",
    "1/2 CTS TUN", "1/4 CTS TUN", "1/8 CTS TUN",
];
const PROFILES: [&str; 3] = ["Cinema", "Broadcast", "Photo"];
const NOT_SIMULATED: &str = "Mémorisé (non simulé dans ARKLIGHT)";

// ---------------------------------------------------------------- S.H.O.T. ↔ réglage manuel
#[derive(Clone, Copy)]
struct Shot {
    s: f64,
    h: f64,
    d: f64,
    k: f64,
}

fn shot<S: Brand>(c: &Pc<S>) -> Shot {
    let sat = if c.hsi() { js::round(c.m_nn("sat", 1.0) * 100.0) } else { 0.0 };
    Shot { s: sat, h: js::round(c.m_or("hue", 0.0)), d: js::round(c.m_nn("intensity", 0.0) * 100.0), k: js::round(c.m_or("cct", 5600.0)) }
}

fn set_shot<S: Brand>(c: &mut Pc<S>, key: char, value: f64) {
    let mut v = shot(c);
    match key {
        'S' => v.s = value,
        'H' => v.h = value,
        'D' => v.d = value,
        _ => v.k = value,
    }
    c.set_manual(obj([("mode", json!(if v.s > 0.0 { "hsi" } else { "cct" })), ("sat", jv(v.s / 100.0)), ("hue", jv(v.h)), ("intensity", jv(v.d / 100.0)), ("cct", jv(v.k))]));
}

fn adjust_shot<S: Brand>(c: &mut Pc<S>, key: char, steps: f64, fine: bool) {
    // C, A : lettres du mode DMX, rien à régler en S.H.O.T.
    let (lo, hi) = match key {
        'S' | 'D' => (0.0, 100.0),
        'H' => (0.0, 360.0),
        'K' => cct_range(c, (1650.0, 8000.0)),
        _ => return,
    };
    let step = if key == 'K' {
        if fine { 25.0 } else { 200.0 }
    } else if fine {
        1.0
    } else {
        10.0
    };
    let v0 = shot(c);
    let cur = match key {
        'S' => v0.s,
        'H' => v0.h,
        'D' => v0.d,
        _ => v0.k,
    };
    let mut v = cur + steps * step;
    if key == 'H' {
        v = (v % 360.0 + 360.0) % 360.0;
    }
    set_shot(c, key, clamp(v, lo, hi));
}

// ---------------------------------------------------------------- menus
fn list_page<S: Brand>(title: &str, items: &'static [&'static str], key: &'static str) -> ItemRef<S> {
    pick(title, opts(items))
        .get(move |c| c.panel_or(key, json!(items[0])))
        .set(move |c, v| {
            c.set_panel(obj([(key, v)]));
            Some(NOT_SIMULATED.into())
        })
        .rc()
}

fn profiles<S: Brand>() -> ItemRef<S> {
    pick("Profiles", opts(&PROFILES)).stored("profile", json!("Photo")).rc()
}

/// Modes DMX Hive : SHOT - HSIC, RGB - CCT, SHOT+EFX+GEL ↔ modes du profil (HSI, RGB+CCT, FX+).
fn dmx_mode_item<S: Brand>() -> ItemRef<S> {
    pick_dyn("DMX Mode", |c: &Pc<S>| {
        c.ty.dmx
            .modes
            .iter()
            .map(|m| {
                let n = m.name.to_lowercase();
                (json!(m.id), (if n.contains("fx") { "SHOT+EFX+GEL" } else if n.contains("rgb") { "RGB - CCT" } else { "SHOT - HSIC" }).to_string())
            })
            .collect()
    })
    .get(|c| c.s("dmxMode").cloned().unwrap_or(Value::Null))
    .set(|c, v| {
        c.set_settings(obj([("dmxMode", v)]));
        None
    })
    .rc()
}

fn local_menu() -> Vec<ItemRef<HiveC>> {
    vec![
        list_page("Effects", &EFFECTS, "effect"),
        list_page("Sources", &SOURCES, "source"),
        profiles(),
        list_page("Corrective", &CORRECTIVE, "corrective"),
        sub("System Info", vec![info("HiveLighting", "").rc(), info("FW Ver 3.0.8", "").rc()]).rc(),
    ]
}

fn dmx_menu() -> Vec<ItemRef<HiveC>> {
    vec![
        pick("DMX Input", opts(&["Lumen Radio", "Local Wired"])).stored("dmxInput", json!("Local Wired")).rc(),
        act("CRMX Linking", |c| {
            Some(if c.panel_or("dmxInput", json!("Local Wired")).as_str() == Some("Lumen Radio") { "Unlinking… (pas d’émetteur)" } else { "Choisis d’abord DMX Input : Lumen Radio" }.into())
        })
        .rc(),
        dmx_mode_item(),
        profiles(),
        sub("System Info", vec![info("HiveLighting", "").rc(), info("FW Ver 3.0.8", "").rc()]).rc(),
    ]
}

fn dmx_on<S: Brand>(c: &Pc<S>) -> bool {
    c.panel_or("dmxSwitch", json!("LOCAL")).as_str() == Some("DMX")
}

// ================================================================== Wasp 100-C / Hornet 200-C
pub(crate) struct HiveC {
    cur: char,
    fine: bool,
    last_select: f64,
    locked: bool,
}

impl Brand for HiveC {
    const SIZE: (f64, f64) = (128.0, 64.0);
    const ID: &'static str = "hive-c";

    fn init(_: &'static FixtureType) -> Self {
        HiveC { cur: 'D', fine: false, last_select: -9.0, locked: false }
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "hive".into(),
            rows: vec![
                vec![led("pwr", "POWER"), led("dmx", "DMX")],
                vec![col(vec![key("power", "ON / OFF", "rect"), key("dmxsw", "DMX / LOCAL", "rect")]), Element::Display],
                vec![knob("select", "SELECT", "push"), knob("adjust", "ADJUST", "push")],
            ],
            display: Some(DisplaySpec { kind: DisplayKind::Draw, w: 128.0, h: 64.0, zoom: 2.0, palette: palette("#050505", "#e9f3ff", "#4c5866", "#e9f3ff", "#050505") }),
            note: "SELECT : choisir S, H, D ou K ; double-clic = menus. ADJUST : régler ; clic = fin / grossier ; maintenu 4 s = verrou. Interrupteur DMX : le projecteur n’écoute que le DMX.".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        if id != "adjust" {
            return Vec::new();
        }
        vec![(4.0, |c, _| {
            c.st.locked = !c.st.locked;
            let l = c.st.locked;
            c.say(if l { "Lock" } else { "Unlock" });
        })]
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if id == "power" {
            let on = c.on();
            return c.set_state(obj([("power", json!(!on))]));
        }
        if id == "dmxsw" {
            let next = if dmx_on(c) { "LOCAL" } else { "DMX" };
            c.set_panel(obj([("dmxSwitch", json!(next))]));
            c.set_settings(obj([("source", json!(if next == "DMX" { "dmx" } else { "manual" }))]));
            // de retour en Local, les lettres propres au DMX reviennent sur S.H.O.T. : C (température) → K, A (adresse) → D
            if next == "LOCAL" && matches!(c.st.cur, 'C' | 'A') {
                c.st.cur = if c.st.cur == 'C' { 'K' } else { 'D' };
            }
            c.set_menu(None);
            c.screen = "home";
            return c.say(if next == "DMX" { "Mode DMX : seules les consoles et applis commandent" } else { "Mode Local : réglages du panneau" });
        }
        if !c.on() {
            return;
        }
        if c.st.locked {
            return c.say("Lock : ADJUST maintenu 4 s pour déverrouiller");
        }
        if id == "select" {
            let double = c.t - c.st.last_select < 0.45;
            c.st.last_select = c.t;
            if c.screen == "menu" {
                c.menu_enter_say();
                return;
            }
            if double {
                let dmx = dmx_on(c);
                c.open_menu(if dmx { dmx_menu() } else { local_menu() }, if dmx { "DMX MENU" } else { "MENU" }, true);
            }
            return;
        }
        if id == "adjust" {
            if c.screen == "menu" {
                if !c.menu_back() {
                    c.close_menu();
                }
                return;
            }
            c.st.fine = !c.st.fine;
            let f = c.st.fine;
            c.say(if f { "Réglage fin" } else { "Réglage grossier" });
        }
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, _mult: f64) {
        if !c.on() {
            return;
        }
        if c.st.locked {
            return c.say("Lock");
        }
        if c.screen == "menu" {
            if id == "select" {
                c.menu_move(steps, 1.0);
            }
            return;
        }
        let keys: &[char] = if dmx_on(c) { &['S', 'H', 'D', 'C', 'A'] } else { &['S', 'H', 'D', 'K'] };
        if id == "select" {
            let i = keys.iter().position(|k| *k == c.st.cur).map_or(0, |i| i as i64);
            c.st.cur = keys[clamp((i as f64) + steps, 0.0, (keys.len() - 1) as f64) as usize];
            return;
        }
        if dmx_on(c) {
            // en DMX, ADJUST règle l'adresse de départ (10 par cran ; fin en tenant la molette enfoncée)
            let fine = c.held_for("adjust") > 0.0;
            let a = clamp(c.address() + steps * if fine { 1.0 } else { 10.0 }, 1.0, 512.0);
            return c.set_settings(obj([("dmxAddress", jv(a))]));
        }
        let (cur, fine) = (c.st.cur, c.st.fine);
        adjust_shot(c, cur, steps, fine);
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let w = 128.0;
        if c.screen == "menu" {
            let Some(menu) = &c.menu else { return Drawn::default() };
            if menu.edit.is_some() {
                return Drawn::of(draw_edit(c, menu, &ListOpts { rows: 2, row_h: 20.0, size: 12.0, ..ListOpts::default() }));
            }
            let items = menu.items(c);
            let idx = menu.level().cursor;
            let top = menu.stack.len() == 1;
            let it = usize::try_from(idx).ok().and_then(|i| items.get(i));
            return Drawn::of(vec![
                text(2.0, 4.0, menu.level().title.clone(), 11.0).b().into(),
                text(w - 2.0, 4.0, if top { format!("{}/{}", idx + 1, items.len()) } else { String::new() }, 11.0).r().b().into(),
                line(0.0, 20.0, w, 20.0, Some(1.0)),
                text(w / 2.0, 30.0, it.map_or(String::new(), |i| i.label.clone()), 14.0).c().b().into(),
                text(w / 2.0, 50.0, if it.is_some_and(|i| matches!(i.kind, Kind::Info(_))) { "" } else { "click SELECT" }, 7.0).c().col("dim").into(),
            ]);
        }
        let v = shot(c);
        let dmx = dmx_on(c);
        let n = js::num_str;
        let mut rows = vec![('S', format!("{}%", n(v.s))), ('H', format!("{}°", n(v.h))), ('D', format!("{}%", n(v.d)))];
        if dmx {
            rows.push(('C', n(v.k)));
            rows.push(('A', pad(c.address(), 3)));
        } else {
            rows.push(('K', n(v.k)));
        }
        let profile: String = c.panel_str("profile", "Photo").encode_utf16().take(1).map(|u| char::from_u32(u32::from(u)).unwrap_or('?')).collect();
        let mut prims: Vec<Prim> = vec![text(w - 3.0, 2.0, profile, 11.0).r().b().into()];
        if dmx {
            let input = if c.panel_or("dmxInput", json!("Local Wired")).as_str() == Some("Lumen Radio") { "CRMX" } else { "WIRE" };
            prims.push(text(w - 3.0, 16.0, input, 8.0).r().into());
            prims.push(text(w - 3.0, 28.0, "SHOT", 8.0).r().col("dim").into());
        }
        let row_h = if dmx { 12.5 } else { 15.5 };
        let size = if dmx { 10.0 } else { 12.0 };
        for (i, (k, val)) in rows.into_iter().enumerate() {
            let y = 2.0 + i as f64 * row_h;
            let sel = c.st.cur == k;
            prims.push(text(4.0, y, format!("{}{k}", if sel { '>' } else { ' ' }), size).b().font("mono").into());
            prims.push(text(30.0, y, val, size).bold_if(sel).font("mono").into());
        }
        if !dmx {
            prims.push(text(w - 3.0, 40.0, if c.st.fine { "FINE" } else { "x10" }, 7.0).r().col("dim").into());
        }
        if c.st.locked {
            prims.push(text(w - 3.0, 52.0, "🔒", 9.0).r().into());
        }
        Drawn::of(prims)
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        let mut l = Leds::new();
        let on = c.on();
        set_led(&mut l, "pwr", on.then(|| Led::on(GREEN)));
        set_led(&mut l, "dmx", (on && dmx_on(c)).then(|| Led::on(if c.dmx_live() { BLUE } else { AMBER })));
        l
    }
}

// ================================================================== Super Hornet 575-C (ballast)
pub(crate) struct SuperHornet {
    cur: i64,
}

fn super_menu() -> Vec<ItemRef<SuperHornet>> {
    vec![
        list_page("Effects", &EFFECTS, "effect"),
        list_page("Sources", &SOURCES, "source"),
        profiles(),
        list_page("Corrective", &CORRECTIVE, "corrective"),
        sub(
            "DMX",
            vec![
                pick("DMX Input", opts(&["Lumen Radio", "Wired", "Ethernet"])).stored("dmxInput", json!("Wired")).rc(),
                act("CRMX Linking", |_| Some("Pas d’émetteur CRMX".into())).rc(),
                dmx_mode_item(),
                num("DMX Address", 1.0, 512.0)
                    .get(|c| c.s("dmxAddress").cloned().unwrap_or(Value::Null))
                    .set(|c, v| {
                        c.set_settings(obj([("dmxAddress", v)]));
                        None
                    })
                    .fmt(|v, _| pad(js::number(Some(v)), 3))
                    .rc(),
            ],
        )
        .rc(),
        sub("System Info", vec![info("HiveLighting", "").rc(), info("Super Hornet 575-C", "").rc()]).rc(),
    ]
}

impl Brand for SuperHornet {
    const SIZE: (f64, f64) = (128.0, 64.0);
    const ID: &'static str = "hive-super";

    fn init(_: &'static FixtureType) -> Self {
        SuperHornet { cur: 3 }
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "hive".into(),
            rows: vec![
                vec![led("status", "STATUS")],
                vec![col(vec![knob("select", "SELECT", ""), key("menu", "MENU", "")]), Element::Display, col(vec![knob("adjust", "ADJUST", ""), key("back", "BACK", "")])],
                vec![key("power", "⏻", "rect")],
            ],
            display: Some(DisplaySpec { kind: DisplayKind::Draw, w: 128.0, h: 64.0, zoom: 2.0, palette: palette("#eef3f8", "#1e3fa8", "#9aaccc", "#1e3fa8", "#2b3036") }),
            note: "Menus du 575-C non publiés (manuel introuvable) : arborescence reconstituée d’après les autres Hive.".into(),
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
        if id == "menu" {
            if c.screen == "menu" {
                c.menu_enter_say();
                return;
            }
            return c.open_menu(super_menu(), "MENU", false);
        }
        if id == "back" && c.screen == "menu" && !c.menu_back() {
            c.close_menu();
        }
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, mult: f64) {
        if !c.on() {
            return;
        }
        if c.screen == "menu" {
            if id == "select" {
                c.menu_move(steps, 1.0);
            } else if c.menu.as_ref().is_some_and(|m| m.edit.is_some()) {
                c.menu_move(steps, mult);
            }
            return;
        }
        if id == "select" {
            c.st.cur = clamp(c.st.cur as f64 + steps, 0.0, 3.0) as i64;
            return;
        }
        let k = ['S', 'H', 'D', 'K'][c.st.cur.clamp(0, 3) as usize];
        adjust_shot(c, k, steps, mult < 2.0);
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let w = 128.0;
        if c.screen == "menu" {
            let prims = match &c.menu {
                Some(m) if m.edit.is_some() => draw_edit(c, m, &ListOpts::default()),
                Some(m) => draw_list(c, m, &ListOpts::default()),
                None => Vec::new(),
            };
            return Drawn::of(prims);
        }
        let v = shot(c);
        let (k_lo, k_hi) = cct_range(c, (1650.0, 8000.0));
        let n = js::num_str;
        let lines = [
            ("SAT", format!("{}%", n(v.s)), v.s / 100.0),
            ("HUE", format!("{}°", n(v.h)), v.h / 360.0),
            ("DIM", format!("{}%", n(v.d)), v.d / 100.0),
            ("CCT", format!("{}K", n(v.k)), (v.k - k_lo) / (k_hi - k_lo)),
        ];
        let effect = c.panel("effect").filter(|e| js::truthy(Some(e)) && e.as_str() != Some("Off")).is_some();
        let mut prims: Vec<Prim> = vec![
            rect_fill(0.0, 0.0, w, 12.0, "fg"),
            text(3.0, 1.0, if effect { "EFX" } else { "SHOT" }, 9.0).inv(true).b().into(),
            text(w - 3.0, 1.0, c.panel_str("profile", "Photo"), 9.0).inv(true).r().b().into(),
        ];
        for (i, (k, val, f)) in lines.into_iter().enumerate() {
            let y = 15.0 + i as f64 * 12.0;
            let sel = c.st.cur == i as i64;
            prims.push(text(2.0, y, format!("{}{k}:{val}", if sel { '▶' } else { ' ' }), 9.0).bold_if(sel).into());
            prims.push(rect_stroke(68.0, y + 2.0, 56.0, 6.0, "dim"));
            prims.push(rect_fill(68.0, y + 2.0, (56.0 * clamp(f, 0.0, 1.0)).max(1.0), 6.0, "fg"));
        }
        Drawn::of(prims)
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        let mut l = Leds::new();
        set_led(&mut l, "status", c.on().then(|| Led::on(GREEN)));
        l
    }
}
