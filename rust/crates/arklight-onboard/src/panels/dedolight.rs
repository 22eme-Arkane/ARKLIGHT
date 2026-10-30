//! Dedolight DLED4-BI / DLED7-BI : tout se règle sur le ballast DT4-BI-DMX / DT7-BI-DMX
//! (docs/recherche/menus/dedo_dmg_kino_litegear.md), portage de
//! `shared/onboard/panels/dedolight.js`. Pas de menu : afficheur 3 chiffres ambre, boutons jaunes
//! ↓ et ↑, molettes DIM et COLOR (potentiomètres), voyants HEAT et ON.
//!  - Mode DMX : l'adresse (001–512) ; point allumé = signal DMX valide.
//!  - ↓ + ↑ ensemble : l'adresse clignote, ↑ / ↓ la changent ; sous 001 → mode manuel.
//!  - Mode manuel : les molettes agissent (d.00–d.99, c.27–c.67) ; ↓ + ↑ puis ↓ = adresse 512, ↑ = 001.

use super::common::{YELLOW, cct_range, pad};
use crate::fixture::obj;
use crate::js::{self, clamp, jv};
use crate::panel::{Brand, Drawn, Pc};
use crate::screen::{DisplayKind, DisplaySpec, Element, Face, Led, Leds, key, knob, label, led, palette, set_led, text};
use arklight_core::library::FixtureType;
use serde_json::json;

/// « Quelques secondes » sans appui : l'adresse est enregistrée.
const EDIT_S: f64 = 6.0;

pub(crate) struct Dedolight {
    edit: f64,
    exiting: f64,
    /// valeur affichée en mode manuel : intensité ('d') ou couleur ('c')
    show: char,
    both_at: f64,
    skip: bool,
    /// dernière touche appuyée (identifiant, heure)
    last_key: Option<(String, f64)>,
}

fn manual(c: &Pc<Dedolight>) -> bool {
    js::truthy(Some(&c.panel_or("manual", json!(false))))
}

fn set_manual_mode(c: &mut Pc<Dedolight>, on: bool) {
    c.set_panel(obj([("manual", json!(on))]));
    c.set_settings(obj([("source", json!(if on { "manual" } else { "auto" }))]));
}

impl Brand for Dedolight {
    const SIZE: (f64, f64) = (60.0, 26.0);
    const ID: &'static str = "dedolight-dt-dmx";

    fn init(_: &'static FixtureType) -> Self {
        Dedolight { edit: 0.0, exiting: 0.0, show: 'd', both_at: -9.0, skip: false, last_key: None }
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "dedo".into(),
            rows: vec![
                vec![led("heat", "HEAT"), led("on", "ON")],
                vec![knob("dim", "DIM", "pointer"), knob("color", "COLOR", "pointer")],
                vec![label("DMX 512 / 1990")],
                vec![key("down", "↓", "small yellow"), Element::Display, key("up", "↑", "small yellow")],
                vec![label("◄ SIGNAL"), key("both", "↓ + ↑ ensemble", "small"), key("power", "I / O", "rect")],
            ],
            display: Some(DisplaySpec { kind: DisplayKind::Draw, w: 60.0, h: 26.0, zoom: 2.4, palette: palette("#0b0604", "#ffa21f", "#3a2209", "#ffa21f", "#0b0604") }),
            note: "« ↓ + ↑ ensemble » remplace l’appui simultané sur les deux boutons jaunes : réglage de l’adresse. Sous 001 : mode manuel, les molettes agissent.".into(),
        }
    }

    fn down(c: &mut Pc<Self>, id: &str) {
        // les deux boutons ensemble (ou l'un juste après l'autre)
        let other = match id {
            "up" => Some("down"),
            "down" => Some("up"),
            _ => None,
        };
        if let Some(o) = other
            && (c.pressed_has(o) || c.t - c.st.last_key.as_ref().map_or(-9.0, |k| k.1) < 0.25)
        {
            c.st.both_at = c.t;
            c.st.skip = true;
        }
        c.st.last_key = Some((id.to_string(), c.t));
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if id == "power" {
            let on = c.on();
            return c.set_state(obj([("power", json!(!on))]));
        }
        if !c.on() {
            return;
        }
        if id == "both" {
            c.st.skip = true;
            c.st.both_at = c.t;
        }
        if c.st.skip && c.t - c.st.both_at < 0.6 {
            // ↓ + ↑ : réglage de l'adresse, ou sortie du mode manuel
            c.st.skip = false;
            if manual(c) {
                c.st.exiting = c.t + EDIT_S;
            } else {
                c.st.edit = c.t + EDIT_S;
            }
            return;
        }
        c.st.skip = false;
        if c.st.exiting > c.t {
            // sortie du mode manuel : ↓ = adresse 512, ↑ = adresse 001
            c.st.exiting = 0.0;
            set_manual_mode(c, false);
            c.set_settings(obj([("dmxAddress", json!(if id == "down" { 511 } else { 1 }))]));
            return c.say(if id == "down" { "Mode DMX, adresse 512 (511 avec 2 canaux)" } else { "Mode DMX, adresse 001" });
        }
        if c.st.edit > c.t {
            c.st.edit = c.t + EDIT_S;
            let a = c.address() + if id == "up" { 1.0 } else { -1.0 };
            if a < 1.0 {
                c.st.edit = 0.0;
                set_manual_mode(c, true);
                return c.say("Mode manuel : les molettes DIM et COLOR agissent");
            }
            return c.set_settings(obj([("dmxAddress", jv(clamp(a, 1.0, 511.0)))]));
        }
        c.say("↓ et ↑ ensemble pour régler l’adresse");
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, _mult: f64) {
        if !c.on() {
            return;
        }
        if !manual(c) {
            return c.say("Molettes inactives en mode DMX (adresse sous 001 pour le manuel)");
        }
        if id == "dim" {
            c.st.show = 'd';
            let v = clamp(js::round(c.m_nn("intensity", 0.0) * 100.0 + steps) / 100.0, 0.0, 0.99);
            return c.set_manual(obj([("intensity", jv(v))]));
        }
        let (lo, hi) = cct_range(c, (2700.0, 6500.0));
        c.st.show = 'c';
        let v = clamp(js::round((c.m_or("cct", 5600.0) + steps * 100.0) / 100.0) * 100.0, lo.max(2700.0), hi.min(6700.0));
        c.set_manual(obj([("mode", json!("cct")), ("cct", jv(v))]));
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let blink = (c.st.edit > c.t || c.st.exiting > c.t) && (c.t * 3.0).floor() % 2.0 == 0.0;
        let mut dot = false;
        let t = if manual(c) && c.st.exiting <= c.t {
            if c.st.show == 'c' {
                format!("c.{}", js::pad_start(&js::num_str(js::round(c.m_or("cct", 5600.0) / 100.0)), 2, '0'))
            } else {
                format!("d.{}", js::pad_start(&js::num_str(js::round(c.m_nn("intensity", 0.0) * 100.0)), 2, '0'))
            }
        } else {
            dot = c.dmx_live() && !manual(c);
            if manual(c) { "d.--".into() } else { pad(c.address(), 3) }
        };
        if blink {
            return Drawn::default();
        }
        Drawn::of(vec![text(30.0, 2.0, format!("{t}{}", if dot { "." } else { "" }), 19.0).c().b().font("seg").sp(2.0).into()])
    }

    fn leds(c: &mut Pc<Self>) -> Leds {
        // ON : jaune en DMX 8 bits (vert en 16 bits) ; HEAT : surchauffe (jamais simulée)
        let mut l = Leds::new();
        set_led(&mut l, "on", c.on().then(|| Led::on(YELLOW)));
        l
    }
}
