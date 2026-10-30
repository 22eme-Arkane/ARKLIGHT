//! LiteGear (docs/recherche/menus/dedo_dmg_kino_litegear.md), portage de
//! `shared/onboard/panels/litegear.js` : les LiteMat n'ont aucune commande, tout est sur le
//! LiteDimmer relié par câble.
//!  - LiteMat Plus 2 / 4 → LiteDimmer Plus DC200 / AC400 DUO : OLED, trois touches sous l'écran
//!    (← · centre · →), molettes LEVEL1, KELVIN 1, LEVEL 2, KELVIN 2, sélecteur LOCAL / MENU / DMX.
//!  - S2 LiteMat 2 / 4 → LiteDimmer Pro Hybrid : pas d'écran, molettes KELVIN et LEVEL, interrupteur
//!    ON / OFF / ALT, touche PROG, un voyant multicolore.

use super::common::{AMBER, BLUE, GREEN, ORANGE, PURPLE, WHITE, YELLOW, cct_range, footprint, pad};
use crate::fixture::obj;
use crate::js::{self, clamp, jv};
use crate::panel::{Brand, Holds, Drawn, Pc};
use crate::screen::{DisplayKind, DisplaySpec, Element, Face, Led, Leds, Prim, col, col_row, key, knob, led, line, palette, rect_fill, set_led, text};
use arklight_core::library::FixtureType;
use serde_json::json;

// ================================================================== LiteDimmer Plus DUO
/// Ligne d'un écran du menu : libellé, réglage, valeurs possibles (None : valeur numérique),
/// valeur par défaut.
type Row = (&'static str, &'static str, Option<&'static [&'static str]>, &'static str);

struct Page {
    title: &'static str,
    rows: &'static [Row],
}

/// Écrans du menu (sélecteur sur MENU) : chaque ligne a ses valeurs, ← / → les changent.
const PAGES: [Page; 4] = [
    Page { title: "DIMMER SETTINGS", rows: &[("CIRCUIT", "circuit", Some(&["SINGLE", "DOUBLE"]), "SINGLE"), ("DIM CURVE", "curve", Some(&["STANDARD"]), "STANDARD"), ("SMOOTING", "smoothing", Some(&["ON", "OFF"]), "ON")] },
    Page { title: "HYBRID SETTINGS", rows: &[("TRIGGER", "trigger", Some(&["CUE", "BUMP"]), "CUE"), ("FADE SPEED", "fade", None, "24"), ("MASTER DMX OUT", "master", Some(&["OFF", "ON"]), "OFF")] },
    Page { title: "DMX SETTINGS", rows: &[("BIT RATE", "bits", Some(&["8", "16"]), "8"), ("ON DMX LOSS", "loss", Some(&["HOLD", "DROP"]), "HOLD")] },
    Page { title: "DMX ADDRESS", rows: &[("", "address", None, "1")] },
];

pub(crate) struct PlusDuo {
    page: i64,
    row: i64,
    welcome: bool,
}

fn position(c: &Pc<PlusDuo>) -> String {
    c.panel_str("switch", "LOCAL")
}

fn set_switch(c: &mut Pc<PlusDuo>, pos: &str) {
    c.set_panel(obj([("switch", json!(pos))]));
    c.set_settings(obj([("source", json!(if pos == "DMX" { "dmx" } else { "manual" }))]));
    c.st.page = 0;
    c.st.row = -1;
    c.st.welcome = pos == "MENU";
}

fn row_value(c: &Pc<PlusDuo>, row: &Row) -> String {
    let (_, key, _, dflt) = *row;
    match key {
        "address" => pad(c.address(), 3),
        "fade" => format!("{} Frms", c.panel_str("fade", dflt)),
        _ => c.panel_str(key, dflt),
    }
}

fn change_row(c: &mut Pc<PlusDuo>, row: &Row, dir: f64) {
    let (_, key, options, dflt) = *row;
    match key {
        "address" => {
            let a = clamp(c.address() + dir, 1.0, 512.0 - footprint(c) + 1.0);
            c.set_settings(obj([("dmxAddress", jv(a))]));
        }
        "fade" => {
            let v = clamp(c.panel_num("fade", 24.0) + dir * 2.0, 2.0, 72.0);
            c.set_panel(obj([("fade", jv(v))]));
        }
        _ => {
            let options = options.unwrap_or(&[]);
            let cur = c.panel_str(key, dflt);
            let n = options.len() as i64;
            let i = options.iter().position(|o| *o == cur).map_or(-1, |i| i as i64);
            let k = (i + dir as i64 + n) % n;
            let v = if k >= 0 { json!(options[k as usize]) } else { serde_json::Value::Null };
            c.set_panel(obj([(key, v)]));
        }
    }
}

impl Brand for PlusDuo {
    const SIZE: (f64, f64) = (128.0, 64.0);
    const ID: &'static str = "litegear-plus-duo";

    fn init(_: &'static FixtureType) -> Self {
        PlusDuo { page: 0, row: -1, welcome: false }
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "litegear".into(),
            rows: vec![
                vec![led("pwr", "PWR"), led("dmx", "DMX"), key("link", "STATUS/LINK", "small blue"), led("mode", "")],
                vec![
                    col(vec![key("local", "LOCAL", "rect"), key("menu", "MENU", "rect"), key("dmxsw", "DMX", "rect")]),
                    col(vec![Element::Display, col_row(vec![key("left", "←", "small"), key("center", "", "small"), key("right", "→", "small")])]),
                ],
                vec![knob("k2", "KELVIN 2", "pointer size=s"), knob("l2", "LEVEL 2", "pointer size=s"), knob("k1", "KELVIN 1", "pointer size=s"), knob("l1", "LEVEL1", "pointer size=s")],
            ],
            display: Some(DisplaySpec { kind: DisplayKind::Draw, w: 128.0, h: 64.0, zoom: 2.0, palette: palette("#020406", "#e8f2ff", "#4a5664", "#e8f2ff", "#020406") }),
            note: "Sélecteur à 3 positions : LOCAL (molettes), MENU (réglages), DMX (console). En MENU : ← / → changent d’écran, le centre choisit une ligne, puis ← / → changent la valeur.".into(),
        }
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        match id {
            "local" => return set_switch(c, "LOCAL"),
            "menu" => return set_switch(c, "MENU"),
            "dmxsw" => return set_switch(c, "DMX"),
            "link" => return c.say("STATUS/LINK : pas de liaison sans fil (interrupteur TX / OFF / RX sur OFF)"),
            _ => {}
        }
        let pos = position(c);
        if pos == "LOCAL" {
            if id == "center" {
                // TRIGGER : CUE = fondu vers le noir / retour ; BUMP = flash
                let off = !js::truthy(Some(&c.panel_or("cueOff", json!(false))));
                c.set_panel(obj([("cueOff", json!(off))]));
                c.set_state(obj([("power", json!(!off))]));
                let msg = if off { format!("CUE : fondu au noir ({} images)", c.panel_str("fade", "24")) } else { "CUE : retour".into() };
                c.say(msg);
            }
            return;
        }
        if pos == "MENU" {
            if c.st.welcome {
                c.st.welcome = false;
                if id == "center" {
                    c.say("INFO : LiteDimmer+ Plus · RESET avec ←");
                }
                return;
            }
            let page = &PAGES[c.st.page.rem_euclid(PAGES.len() as i64) as usize];
            if id == "center" {
                // après la dernière ligne : « Done »
                c.st.row = if c.st.row + 1 >= page.rows.len() as i64 { -1 } else { c.st.row + 1 };
                return;
            }
            let dir = if id == "right" { 1.0 } else { -1.0 };
            if c.st.row >= 0 {
                if let Some(row) = page.rows.get(c.st.row as usize) {
                    change_row(c, row, dir);
                }
                return;
            }
            c.st.page = (c.st.page + dir as i64 + PAGES.len() as i64) % PAGES.len() as i64;
        }
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, _mult: f64) {
        let pos = position(c);
        if pos != "LOCAL" {
            return c.say(if pos == "DMX" { "Molettes inactives en DMX" } else { "Molettes inactives en MENU" });
        }
        let dbl = c.panel_str("circuit", "SINGLE") == "DOUBLE";
        if (id == "k2" || id == "l2") && !dbl {
            return c.say("Circuit 2 : seulement en CIRCUIT DOUBLE");
        }
        if id == "l1" || id == "l2" {
            let v = clamp(js::round(c.m_nn("intensity", 0.0) * 100.0 + steps) / 100.0, 0.0, 1.0);
            return c.set_manual(obj([("intensity", jv(v))]));
        }
        let (lo, hi) = cct_range(c, (2600.0, 6000.0));
        let v = clamp(js::round((c.m_or("cct", 4500.0) + steps * 50.0) / 50.0) * 50.0, lo, hi);
        c.set_manual(obj([("mode", json!("cct")), ("cct", jv(v))]));
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let w = 128.0;
        let pos = position(c);
        let mut prims: Vec<Prim> = Vec::new();
        let softkeys = |prims: &mut Vec<Prim>, center: &str| {
            prims.push(line(0.0, 52.0, w, 52.0, None));
            prims.push(text(3.0, 54.0, "←", 8.0).into());
            prims.push(text(w / 2.0, 54.0, center, 8.0).c().b().into());
            prims.push(text(w - 3.0, 54.0, "→", 8.0).r().into());
        };
        if pos == "MENU" {
            if c.st.welcome {
                prims.push(text(w / 2.0, 2.0, "LITEGEAR", 9.0).c().b().into());
                prims.push(text(w / 2.0, 13.0, "LITEDIMMER+ PLUS", 8.0).c().into());
                prims.push(text(w / 2.0, 26.0, "SWITCH  POSITION", 8.0).c().into());
                prims.push(text(w / 2.0, 37.0, "Up: Local    Down: DMX", 7.0).c().into());
                softkeys(&mut prims, "INFO");
                return Drawn::of(prims);
            }
            let page = &PAGES[c.st.page.rem_euclid(PAGES.len() as i64) as usize];
            prims.push(text(w / 2.0, 2.0, page.title, 9.0).c().b().into());
            prims.push(line(0.0, 13.0, w, 13.0, None));
            for (i, row) in page.rows.iter().enumerate() {
                let y = 16.0 + i as f64 * 11.0;
                let sel = c.st.row == i as i64;
                if sel {
                    prims.push(rect_fill(0.0, y - 1.0, w, 10.0, "fg"));
                }
                let labeled = !row.0.is_empty();
                if labeled {
                    prims.push(text(3.0, y, row.0, 7.5).inv(sel).into());
                }
                let v = text(if labeled { w - 3.0 } else { w / 2.0 }, if labeled { y } else { y + 4.0 }, row_value(c, row), if labeled { 7.5 } else { 16.0 }).inv(sel).bold_if(!labeled);
                prims.push(if labeled { v.r().into() } else { v.c().into() });
            }
            softkeys(&mut prims, if c.st.row >= 0 { "DONE" } else { "EDIT" });
            return Drawn::of(prims);
        }
        let dbl = c.panel_str("circuit", "SINGLE") == "DOUBLE";
        prims.push(text(w / 2.0, 2.0, "HYBRID", 8.0).c().b().into());
        prims.push(text(30.0, 13.0, "KELVIN TEMP", 7.0).c().into());
        prims.push(text(98.0, 13.0, "INTENSITY", 7.0).c().into());
        prims.push(text(30.0, 24.0, format!("{}K", js::num_str(js::round(c.m_or("cct", 4500.0)))), 12.0).c().b().into());
        prims.push(text(98.0, 24.0, format!("{}%", js::num_str(js::round(c.m_nn("intensity", 0.0) * 100.0))), 12.0).c().b().into());
        if pos == "LOCAL" {
            // quelle molette commande chaque valeur (KELVIN 2, LEVEL 2, KELVIN 1, LEVEL1)
            prims.push(text(30.0, 40.0, "○○●○", 8.0).c().into());
            prims.push(text(98.0, 40.0, "○○○●", 8.0).c().into());
            let cue = js::truthy(Some(&c.panel_or("cueOff", json!(false))));
            softkeys(&mut prims, if cue { "CUE ON" } else { "CUE OFF" });
        } else {
            softkeys(&mut prims, if dbl { "TWO CIRCUIT DMX" } else { "ONE CIRCUIT DMX" });
        }
        Drawn::of(prims)
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        let pos = position(c);
        let mut l = Leds::new();
        set_led(&mut l, "pwr", Some(Led::on(GREEN)));
        set_led(&mut l, "dmx", (pos == "DMX" && c.dmx_live()).then(|| Led::on(BLUE)));
        set_led(&mut l, "mode", Some(Led::on(if pos == "LOCAL" { GREEN } else if pos == "DMX" { BLUE } else { AMBER })));
        l
    }
}

// ================================================================== LiteDimmer Pro Hybrid (S2 LiteMat)
const ALT_MODES: [(&str, &str); 6] = [("OVERDRIVE", WHITE), ("LOW", BLUE), ("2-CHANNEL", YELLOW), ("BUMP", PURPLE), ("PAPARAZZI", PURPLE), ("FIRE", ORANGE)];

pub(crate) struct ProHybrid {
    choosing: bool,
    alt: i64,
}

fn alt_of(c: &Pc<ProHybrid>) -> usize {
    (c.panel_num("alt", 0.0) as i64).rem_euclid(ALT_MODES.len() as i64) as usize
}

impl Brand for ProHybrid {
    const SIZE: (f64, f64) = (0.0, 0.0);
    const ID: &'static str = "litegear-pro-hybrid";

    fn init(_: &'static FixtureType) -> Self {
        ProHybrid { choosing: false, alt: 0 }
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "litegear".into(),
            rows: vec![
                vec![led("status", "INDICATOR")],
                vec![knob("kelvin", "KELVIN (TUN ←)", "pointer"), knob("level", "LEVEL", "pointer")],
                vec![key("on", "ON", "rect"), key("off", "OFF", "rect"), key("alt", "ALT", "rect"), key("prog", "PROG", "small")],
            ],
            display: None,
            note: "Pas d’écran ni de DMX : interrupteur ON (hybride) / OFF / ALT, molettes potentiomètres. Interrupteur sur OFF puis PROG maintenu 2 s : choix du mode ALT avec LEVEL (couleur du voyant), PROG 2 s pour valider.".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        if id != "prog" {
            return Vec::new();
        }
        vec![(2.0, |c, _| {
            if c.panel_str("switch", "ON") != "OFF" {
                return c.say("Mettre l’interrupteur sur OFF pour programmer");
            }
            if c.st.choosing {
                c.st.choosing = false;
                let alt = c.st.alt;
                c.set_panel(obj([("alt", json!(alt))]));
                return c.say(format!("Mode ALT : {}", ALT_MODES[alt as usize].0));
            }
            c.st.choosing = true;
            c.st.alt = alt_of(c) as i64;
            c.say("Tourner LEVEL pour choisir le mode ALT");
        })]
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if id == "on" || id == "off" || id == "alt" {
            let pos = id.to_uppercase();
            c.set_panel(obj([("switch", json!(pos))]));
            c.set_state(obj([("power", json!(pos != "OFF"))]));
            if pos == "ALT" {
                let name = ALT_MODES[alt_of(c)].0;
                let fx = if ["BUMP", "PAPARAZZI", "FIRE"].contains(&name) { " (effet non simulé)" } else { "" };
                return c.say(format!("ALT : {name}{fx}"));
            }
            return;
        }
        if id == "prog" {
            c.say("PROG : maintenir 2 s (interrupteur sur OFF)");
        }
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, _mult: f64) {
        if c.st.choosing && id == "level" {
            let n = ALT_MODES.len() as i64;
            c.st.alt = (c.st.alt + if steps > 0.0 { 1 } else { n - 1 }) % n;
            return;
        }
        if id == "level" {
            let max = if c.panel_str("switch", "ON") == "ALT" && ALT_MODES[alt_of(c)].0 == "LOW" { 0.25 } else { 1.0 };
            let v = clamp(js::round(c.m_nn("intensity", 0.0) * 100.0 + steps) / 100.0, 0.0, max);
            return c.set_manual(obj([("intensity", jv(v))]));
        }
        let (lo, hi) = cct_range(c, (2600.0, 6000.0));
        let v = clamp(js::round((c.m_or("cct", 4500.0) + steps * 50.0) / 50.0) * 50.0, lo, hi);
        c.set_manual(obj([("mode", json!("cct")), ("cct", jv(v))]));
    }

    fn draw(_c: &mut Pc<Self>) -> Drawn {
        Drawn::default()
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        let mut l = Leds::new();
        if c.st.choosing {
            set_led(&mut l, "status", Some(Led::blink(ALT_MODES[c.st.alt as usize].1, 2.0)));
            return l;
        }
        let pos = c.panel_str("switch", "ON");
        let led = match pos.as_str() {
            "OFF" => None,
            "ALT" => {
                let (name, color) = ALT_MODES[alt_of(c)];
                Some(Led::blink(color, if name == "PAPARAZZI" { 3.0 } else { 0.0 }))
            }
            _ => Some(Led::on(GREEN)),
        };
        set_led(&mut l, "status", led);
        l
    }
}
