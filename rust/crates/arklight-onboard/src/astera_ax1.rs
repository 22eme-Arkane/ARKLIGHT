//! Projecteurs Astera à bouton unique, sans écran : AX1 PixelTube, AX3 Lightdrop (deux boutons en
//! parallèle), NYX et SolaBulb ; portage de `shared/onboard/astera-ax1.js`. La lumière elle-même
//! sert d'affichage (jauge de batterie, charge, BlueMode, confirmation, adressage DMX par secousse
//! sur l'AX1).
//! D'après les Technical Reference Manuals AX1 (2020) et AX3 (2023) : docs/recherche/menus/ax1_ax2.md, ax3_ax5_ax7.md.

use crate::astera_v5::reset_settings;
use crate::fixture::{Ctx, Fx, Outcome, obj};
use crate::js;
use arklight_core::engine::gels::astera_rgb_to_linear;
use arklight_core::library::{FixtureType, library};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Adressage DMX par le bouton : couleur affichée -> profil et adresses possibles (AX1 manuel 2018 p.16).
pub struct Ax1Addressing {
    pub color: [f64; 3],
    pub label: &'static str,
    pub mode: &'static str,
    pub name: &'static str,
    pub step: f64,
    pub count: usize,
}

pub const AX1_ADDRESSING: [Ax1Addressing; 4] = [
    Ax1Addressing { color: [0.18, 0.02, 0.6], label: "violet", mode: "42", name: "42: RGB RGB (16 px)", step: 48.0, count: 10 },
    Ax1Addressing { color: [0.0, 1.0, 0.0], label: "vert", mode: "1", name: "1: RGB (1 px)", step: 3.0, count: 15 },
    Ax1Addressing { color: [1.0, 0.15, 0.45], label: "rose", mode: "48", name: "48: RGB CCT DIM IND (16 px)", step: 96.0, count: 5 },
    Ax1Addressing { color: [1.0, 0.0, 0.0], label: "rouge", mode: "7", name: "7: RGB CCT DIM IND (1 px)", step: 6.0, count: 15 },
];

/// Couleurs fixes du BlueMode de l'AX1 (couleurs STATIC COLOR sans le noir).
fn bluemode_colors() -> Vec<&'static (String, [f64; 3])> {
    library().static_colors.iter().filter(|(n, _)| n != "BLACK").collect()
}

fn fill(out: &mut [f64], pixels: usize, rgb: [f64; 3], from: f64, to: f64) {
    for (p, px) in out.chunks_mut(3).take(pixels).enumerate() {
        let on = (p as f64) >= from && (p as f64) < to;
        for (v, c) in px.iter_mut().zip(rgb) {
            *v = if on { c } else { 0.0 };
        }
    }
}

fn fill_all(out: &mut [f64], pixels: usize, rgb: [f64; 3]) {
    fill(out, pixels, rgb, 0.0, pixels as f64);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Overlay {
    Gauge,
    Tap,
    Confirm,
    Red,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoldStage {
    On,
    Off,
    Blue,
    Reset,
}

pub(crate) struct AsteraAx1 {
    ty: &'static FixtureType,
    pressed: Vec<(String, f64)>,
    last_t: f64,
    overlay: Option<(Overlay, f64)>,
    /// couleur choisie en BlueMode (-1 : aucune)
    color_idx: i64,
    /// adressage DMX par secousse : (profil, index)
    addressing: Option<(usize, usize)>,
    /// retour visuel pendant un appui long
    hold_stage: Option<HoldStage>,
    /// « bar » (AX1 : longueur), « color » (AX3 : couleur seule) ou « none »
    gauge_kind: String,
    can_shake: bool,
    /// « short » : un appui court éteint (SolaBulb)
    off_gesture: String,
    alias: HashMap<String, String>,
    fx: Fx,
}

impl AsteraAx1 {
    pub fn new(ty: &'static FixtureType) -> AsteraAx1 {
        let cfg = ty.extra.get("controls");
        let get = |k: &str| cfg.and_then(|c| c.get(k));
        let alias = get("keys")
            .and_then(Value::as_array)
            .map(|keys| {
                keys.iter()
                    .filter_map(|k| Some((k.get("id")?.as_str()?.to_string(), k.get("alias").filter(|a| js::truthy(Some(a)))?.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        AsteraAx1 {
            ty,
            pressed: Vec::new(),
            last_t: 0.0,
            overlay: None,
            color_idx: -1,
            addressing: None,
            hold_stage: None,
            gauge_kind: js::text_or(get("gauge"), "bar").to_string(),
            can_shake: get("shake") != Some(&Value::Bool(false)),
            off_gesture: js::text_or(get("offGesture"), "hold").to_string(),
            alias,
            fx: Fx::default(),
        }
    }

    fn resolve<'a>(&'a self, id: &'a str) -> &'a str {
        self.alias.get(id).map_or(id, String::as_str)
    }

    pub fn press(&mut self, cx: &mut Ctx, id: &str) {
        let t = cx.env.time;
        self.last_t = t;
        let id = self.resolve(id).to_string();
        match self.pressed.iter_mut().find(|(k, _)| *k == id) {
            Some(p) => p.1 = t,
            None => self.pressed.push((id, t)),
        }
    }

    pub fn release(&mut self, cx: &mut Ctx, id: &str) {
        let t = cx.env.time;
        let id = self.resolve(id).to_string();
        self.last_t = t;
        let p = self.pressed.iter().position(|(k, _)| *k == id).map(|i| self.pressed.remove(i).1);
        self.hold_stage = None;
        let Some(t0) = p else { return };
        if id != "power" {
            return;
        }
        let held = t - t0;
        if !self.fx.power() {
            if held >= 1.0 {
                self.power_on(cx, t);
            }
            return;
        }
        // Adressage DMX par secousse
        if let Some((profile, index)) = self.addressing {
            let prof = &AX1_ADDRESSING[profile];
            if held < 1.0 {
                self.addressing = Some((profile, (index + 1) % prof.count));
            } else {
                self.fx.set(obj([("dmxMode", json!(prof.mode)), ("dmxAddress", js::jv(1.0 + index as f64 * prof.step)), ("dimmerCurve", json!("STANDARD")), ("dmxFail", json!("HOLD"))]));
                self.fx.set_state1("blueMode", json!(false));
                self.addressing = None;
                self.overlay = Some((Overlay::Confirm, t + 1.2));
            }
            return;
        }
        // BlueMode : appui court = couleur suivante, maintien 1 s = valider la couleur et sortir
        if js::truthy(self.fx.st("blueMode")) {
            let colors = bluemode_colors();
            if held < 1.0 {
                self.color_idx = (self.color_idx + 1) % colors.len() as i64;
                cx.runtime.blue_color = Some(astera_rgb_to_linear(colors[self.color_idx as usize].1));
            } else if held < 2.5 {
                if self.color_idx >= 0 {
                    let name = colors[self.color_idx as usize].0.clone();
                    let sa = self.fx.s("standalone").cloned();
                    let mut list = vec![json!({ "kind": "named", "name": name })];
                    list.extend(sa.as_ref().and_then(|s| s.get("colors")).and_then(Value::as_array).map(|c| c.iter().skip(1).cloned().collect::<Vec<_>>()).unwrap_or_default());
                    let standalone = js::spread(sa.as_ref(), &obj([("program", json!(0)), ("colors", Value::Array(list))]));
                    self.fx.set1("standalone", Value::Object(standalone));
                    self.fx.set_state1("blueMode", json!(false));
                    cx.astera.latch_standalone(cx.runtime);
                } else {
                    self.fx.set_state1("blueMode", json!(false));
                }
                self.color_idx = -1;
                cx.runtime.blue_color = None;
            }
            return;
        }
        // Lampe allumée
        if held < 1.0 {
            if js::truthy(self.fx.st("acceptConfig")) {
                self.fx.set_state1("acceptConfig", json!(false));
                self.overlay = Some((Overlay::Tap, t + 1.2));
            } else if self.off_gesture == "short" {
                self.fx.set_state(obj([("power", json!(false)), ("blueMode", json!(false))]));
            } else if self.gauge_kind != "none" {
                self.overlay = Some((Overlay::Gauge, t + 2.0));
            }
        } else if held < 2.5 {
            self.fx.set_state(obj([("power", json!(false)), ("blueMode", json!(false))]));
        } else if held < 5.0 {
            self.enter_blue_mode(cx);
        } else {
            self.do_reset(cx, t);
        }
    }

    fn power_on(&mut self, cx: &mut Ctx, t: f64) {
        self.fx.set_state(obj([("power", json!(true)), ("blueMode", json!(false))]));
        cx.astera.unlatch(cx.runtime);
        if self.gauge_kind != "none" {
            self.overlay = Some((Overlay::Gauge, t + 1.5));
        }
    }

    fn enter_blue_mode(&mut self, cx: &mut Ctx) {
        self.color_idx = -1;
        cx.runtime.blue_color = None;
        self.fx.set_state1("blueMode", json!(true));
    }

    fn do_reset(&mut self, cx: &mut Ctx, t: f64) {
        let reset = reset_settings(self.ty, &self.fx.settings);
        self.fx.set(reset);
        self.fx.set_state1("blueMode", json!(false));
        cx.astera.unlatch(cx.runtime);
        cx.runtime.blue_color = None;
        self.overlay = Some((Overlay::Red, t + 0.5));
    }

    /// Secouer le tube (accéléromètre) : passe en adressage DMX depuis le BlueMode, ou change de profil.
    pub fn shake(&mut self) {
        if !self.fx.power() || !self.can_shake {
            return;
        }
        if let Some((profile, _)) = self.addressing {
            self.addressing = Some(((profile + 1) % AX1_ADDRESSING.len(), 0));
        } else if js::truthy(self.fx.st("blueMode")) {
            self.addressing = Some((0, 0));
        }
    }

    pub fn tick(&mut self, cx: &mut Ctx) {
        let t = cx.env.time;
        self.last_t = t;
        let p = self.pressed.iter().find(|(k, _)| k == "power").map(|(_, t0)| *t0);
        let power = self.fx.power();
        if let Some(t0) = p {
            if power && self.addressing.is_none() && !js::truthy(self.fx.st("blueMode")) {
                let held = t - t0;
                self.hold_stage = if held >= 5.0 {
                    Some(HoldStage::Reset)
                } else if held >= 2.5 {
                    Some(HoldStage::Blue)
                } else if held >= 1.0 {
                    Some(HoldStage::Off)
                } else {
                    None
                };
            } else if !power {
                self.hold_stage = if t - t0 >= 1.0 { Some(HoldStage::On) } else { None };
            }
        }
        if self.overlay.is_some_and(|(_, until)| t > until) {
            self.overlay = None;
        }
    }

    pub fn held_for(&self, id: &str, t: f64) -> f64 {
        self.pressed.iter().find(|(k, _)| k == id).map_or(0.0, |(_, t0)| t - t0)
    }

    /// Le tube sert d'écran : remplace la sortie normale quand il affiche un état.
    pub fn override_pixels(&self, out: &mut [f64], pixels: usize, t: f64) -> bool {
        let overlay = self.overlay.map(|(k, _)| k);
        // Appui long en cours : extinction, puis BlueMode, puis rouge (reset)
        if self.hold_stage == Some(HoldStage::Off) {
            fill_all(out, pixels, [0.0, 0.0, 0.0]);
            return true;
        }
        if self.hold_stage == Some(HoldStage::Blue) {
            fill_all(out, pixels, if (t * 3.0) % 1.0 < 0.5 { [0.0, 0.12, 0.9] } else { [0.0, 0.0, 0.0] });
            return true;
        }
        if self.hold_stage == Some(HoldStage::Reset) || overlay == Some(Overlay::Red) {
            fill_all(out, pixels, [0.8, 0.0, 0.0]);
            return true;
        }
        if !self.fx.power() {
            if !js::truthy(self.fx.st("charging")) {
                return false;
            }
            if js::nn(self.fx.st("battery"), 100.0) >= 100.0 {
                fill_all(out, pixels, [0.0, 0.0, 0.0]);
                return true;
            }
            if self.gauge_kind == "color" {
                // vert, clignotement lent
                fill_all(out, pixels, if t % 1.0 < 0.5 { [0.0, 0.5, 0.0] } else { [0.0, 0.0, 0.0] });
                return true;
            }
            let head = ((t * 6.0) % pixels as f64).floor();
            fill(out, pixels, [0.0, 0.5, 0.0], head, head + 1.0);
            return true;
        }
        if let Some((profile, index)) = self.addressing {
            let prof = &AX1_ADDRESSING[profile];
            let code = (index + 1) as i64;
            let q = pixels as f64 / 4.0;
            for (p, px) in out.chunks_mut(3).take(pixels).enumerate() {
                let lit = (code >> ((p as f64 / q).floor() as i64).clamp(0, 63)) & 1 == 1;
                for (v, c) in px.iter_mut().zip(prof.color) {
                    *v = if lit { c * 0.7 } else { 0.0 };
                }
            }
            return true;
        }
        if overlay == Some(Overlay::Gauge) {
            let b = js::nn(self.fx.st("battery"), 100.0) / 100.0;
            if self.gauge_kind == "color" {
                // AX3 : rouge < 30 min restantes, orange au-delà de 30 min utilisées, vert sinon (seuils estimés)
                fill_all(out, pixels, if b < 0.2 { [0.7, 0.0, 0.0] } else if b < 0.85 { [0.8, 0.35, 0.0] } else { [0.0, 0.6, 0.0] });
                return true;
            }
            if b >= 0.9 {
                fill_all(out, pixels, [0.0, 0.6, 0.0]);
            } else if b < 0.15 {
                fill(out, pixels, [0.7, 0.0, 0.0], 0.0, 1.0);
            } else {
                fill(out, pixels, [0.8, 0.35, 0.0], 0.0, js::round(b * pixels as f64).max(1.0));
            }
            return true;
        }
        if let Some((Overlay::Tap | Overlay::Confirm, until)) = self.overlay {
            let e = until - t;
            fill_all(out, pixels, if (e * 2.5) % 1.0 < 0.5 { [1.0, 1.0, 1.0] } else { [0.0, 0.0, 0.0] });
            return true;
        }
        false
    }

    pub fn hint(&self) -> String {
        if let Some((profile, index)) = self.addressing {
            let prof = &AX1_ADDRESSING[profile];
            return format!(
                "Adressage DMX : profil {} ({}), adresse {}. Appui court : adresse suivante · secouer : profil suivant · maintenir : valider.",
                prof.name,
                prof.label,
                js::num_str(1.0 + index as f64 * prof.step)
            );
        }
        if !self.fx.power() {
            return if js::truthy(self.fx.st("charging")) {
                if self.gauge_kind == "color" { "Éteint sur chargeur : la LED clignote lentement en vert (éteinte = chargé).".into() } else { "Éteint sur chargeur : un pixel vert parcourt le tube (sombre = chargé).".into() }
            } else {
                "Éteint : maintenir 1 s pour allumer.".into()
            };
        }
        if js::truthy(self.fx.st("blueMode")) {
            return if self.color_idx >= 0 {
                format!("BlueMode : couleur {}. Appui court : couleur suivante · maintenir 1 s : valider.", bluemode_colors()[self.color_idx as usize].0)
            } else {
                format!("BlueMode (clignote en bleu) : prêt pour « Pair with Lights ». Appui court : couleurs fixes{}.", if self.can_shake { " · secouer : adressage DMX" } else { "" })
            };
        }
        match self.hold_stage {
            Some(HoldStage::Off) => "Relâcher maintenant = extinction".into(),
            Some(HoldStage::Blue) => "Relâcher maintenant = BlueMode".into(),
            Some(HoldStage::Reset) => "Reset effectué (rouge)".into(),
            _ => String::new(),
        }
    }

    pub(crate) fn refresh(&mut self, cx: &Ctx) {
        self.fx = Fx::of(cx.fixture);
    }

    pub(crate) fn take(&mut self) -> Outcome {
        self.fx.take()
    }
}
