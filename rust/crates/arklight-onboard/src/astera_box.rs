//! AsteraBox ART7 / ART7-WIFI : face supérieure (bouton marche, touche verte CRMX LINK, voyants
//! APP / LINK / DMX / RX / TX, jauge de batterie 5 segments), sans écran ; portage de
//! `shared/onboard/astera-box.js`.
//! D'après le manuel ART7 v3.0 (2022) : docs/recherche/menus/box_remotes.md.

use crate::fixture::{Ctx, Fx, Outcome, Relayed, obj};
use crate::js;
use crate::screen::{Led, Leds};
use arklight_core::library::library;
use serde_json::{Value, json};

const LINK_SECONDS: f64 = 10.0;

#[derive(Debug, Clone)]
struct Press {
    t0: f64,
    fired: bool,
}

pub(crate) struct AsteraBox {
    pressed: Vec<(String, Press)>,
    last_t: f64,
    gauge_until: f64,
    /// fenêtre de liaison (fin)
    linking: Option<f64>,
    unlinking: Option<f64>,
    /// choix du mode TX / RX en cours
    mode_select: Option<String>,
    /// horodatage des appuis LINK (5 appuis + maintien)
    link_presses: Vec<f64>,
    fx: Fx,
}

impl AsteraBox {
    pub fn new() -> AsteraBox {
        AsteraBox { pressed: Vec::new(), last_t: 0.0, gauge_until: 0.0, linking: None, unlinking: None, mode_select: None, link_presses: Vec::new(), fx: Fx::default() }
    }

    fn crmx_mode(&self) -> String {
        js::text_or(self.fx.s("crmxMode"), "TX").to_string()
    }

    fn universe(&self) -> Option<&Value> {
        self.fx.s("universe")
    }

    pub fn press(&mut self, cx: &mut Ctx, id: &str) {
        let t = cx.env.time;
        self.last_t = t;
        let p = Press { t0: t, fired: false };
        match self.pressed.iter_mut().find(|(k, _)| k == id) {
            Some(old) => old.1 = p,
            None => self.pressed.push((id.to_string(), p)),
        }
        if id == "link" {
            self.link_presses.retain(|x| t - x < 3.0);
            self.link_presses.push(t);
        }
    }

    pub fn release(&mut self, cx: &mut Ctx, id: &str) {
        let t = cx.env.time;
        self.last_t = t;
        let Some(p) = self.pressed.iter().position(|(k, _)| k == id).map(|i| self.pressed.remove(i).1) else { return };
        let held = t - p.t0;
        let power = self.fx.power();
        if id == "power" {
            if p.fired {
                return;
            }
            if held >= 1.0 {
                return self.fx.set_state1("power", json!(!power));
            }
            if power || js::truthy(self.fx.st("charging")) {
                self.gauge_until = t + 2.0;
            }
            return;
        }
        if id == "link" && power && !p.fired && held < 3.0 {
            if let Some(pending) = &mut self.mode_select {
                *pending = if pending == "TX" { "RX".into() } else { "TX".into() };
                return;
            }
            if self.crmx_mode() == "TX" {
                self.linking = Some(t + LINK_SECONDS);
            }
        }
    }

    pub fn tick(&mut self, cx: &mut Ctx) {
        let t = cx.env.time;
        self.last_t = t;
        let ids: Vec<String> = self.pressed.iter().map(|(k, _)| k.clone()).collect();
        for id in ids {
            let Some(p) = self.pressed.iter().find(|(k, _)| *k == id).map(|(_, p)| p.clone()) else { continue };
            let held = t - p.t0;
            let fire = |me: &mut Self| {
                if let Some((_, p)) = me.pressed.iter_mut().find(|(k, _)| *k == id) {
                    p.fired = true;
                }
            };
            if id == "power" && held >= 1.0 && !p.fired {
                fire(self);
                let power = self.fx.power();
                self.fx.set_state1("power", json!(!power));
            }
            if id == "link" && held >= 3.0 && !p.fired && self.fx.power() {
                fire(self);
                if let Some(pending) = self.mode_select.take() {
                    // validation du mode choisi
                    self.fx.set1("crmxMode", json!(pending));
                } else if self.link_presses.len() >= 5 {
                    self.mode_select = Some(self.crmx_mode());
                } else if self.crmx_mode() == "TX" {
                    self.unlink_all(cx, t);
                } else {
                    self.fx.set1("rxLinked", json!(false));
                }
                self.link_presses.clear();
            }
        }
        if let Some(until) = self.linking {
            // Les projecteurs déliés, en AUTO (détection) ou en CRMX, se lient pendant la fenêtre de liaison
            self.link_candidates(cx);
            if t > until {
                self.linking = None;
            }
        }
        if self.unlinking.is_some_and(|u| t > u) {
            self.unlinking = None;
        }
    }

    fn link_candidates(&mut self, cx: &mut Ctx) {
        let u = self.universe().cloned().unwrap_or(Value::Null);
        for other in cx.env.fixtures {
            let Some(ty) = library().fixture(&other.type_id) else { continue };
            let s = Fx::of(other);
            if !ty.is_astera_v5() || js::truthy(s.s("crmxLinked")) || !s.power() {
                continue;
            }
            let sel = js::text_or(s.s("inputSelect"), "AUTO");
            if sel != "AUTO" && sel != "CRMX wirel. DMX" {
                continue;
            }
            self.fx.out.others.push(Relayed { id: other.id.clone(), settings: obj([("crmxLinked", json!(true)), ("universe", u.clone())]), relatch: true, unlinked: false });
        }
    }

    fn unlink_all(&mut self, cx: &mut Ctx, t: f64) {
        let u = self.universe().cloned();
        for other in cx.env.fixtures {
            let Some(ty) = library().fixture(&other.type_id) else { continue };
            let s = Fx::of(other);
            if !ty.is_astera_v5() || !js::truthy(s.s("crmxLinked")) || !js::strict_eq(s.s("universe"), u.as_ref()) || !s.power() {
                continue;
            }
            self.fx.out.others.push(Relayed { id: other.id.clone(), settings: obj([("crmxLinked", json!(false))]), relatch: true, unlinked: true });
        }
        self.unlinking = Some(t + 4.0);
    }

    pub fn held_for(&self, id: &str, t: f64) -> f64 {
        self.pressed.iter().find(|(k, _)| k == id).map_or(0.0, |(_, p)| t - p.t0)
    }

    /// Voyants : jauge (bat0 … bat4), APP, LINK, DMX, RX, TX.
    pub fn leds(&self, cx: &Ctx) -> Leds {
        let t = cx.env.time;
        let mut out = Leds::new();
        let power = self.fx.power();
        let battery = js::nn(self.fx.st("battery"), 100.0);
        let show_gauge = (power && t < self.gauge_until) || (!power && js::truthy(self.fx.st("charging")) && battery < 100.0);
        for i in 0..5 {
            if show_gauge && battery > f64::from(i) * 20.0 {
                out.insert(format!("bat{i}"), Led::on("#f2f5f7"));
            }
        }
        if !power {
            return out;
        }
        out.insert("app".into(), Led::blink("#2f7bff", if cx.env.app_connected { 1.5 } else { 0.5 }));
        let tx = self.crmx_mode() == "TX";
        let cable = self.fx.st("dmxCable") != Some(&Value::Bool(false));
        if !cable {
            return out;
        }
        if let Some(pending) = &self.mode_select {
            let phase = (t * 2.0) % 1.0 < 0.5;
            if phase {
                out.insert("link".into(), Led::on("#33e07a"));
            } else {
                out.insert("dmx".into(), Led::on("#f2f5f7"));
            }
            let pend_tx = pending == "TX";
            out.insert("tx".into(), Led::on(if pend_tx { "#2f7bff" } else { "#f2f5f7" }));
            out.insert("rx".into(), Led::on(if pend_tx { "#f2f5f7" } else { "#ff9a2e" }));
            return out;
        }
        if tx {
            out.insert("tx".into(), Led::on("#2f7bff"));
            out.insert(
                "link".into(),
                if self.linking.is_some() {
                    Led::blink("#33e07a", 3.0)
                } else if self.unlinking.is_some() {
                    Led::blink("#33e07a", 1.0)
                } else {
                    Led::on("#33e07a")
                },
            );
            if cx.env.dmx_live(self.universe().and_then(Value::as_f64)) {
                out.insert("dmx".into(), Led::on("#33e07a"));
            }
        } else {
            out.insert("rx".into(), Led::on("#ff9a2e"));
            if js::truthy(self.fx.s("rxLinked")) {
                out.insert("link".into(), Led::on("#33e07a"));
            }
        }
        out
    }

    pub fn hint(&self, cx: &Ctx) -> String {
        if !self.fx.power() {
            return "Éteinte : maintenir ⏻ 1 s pour allumer (appui court : niveau de batterie).".into();
        }
        let u = js::string(self.universe());
        if let Some(p) = &self.mode_select {
            return format!("Choix du mode : {p} (appui court : changer · maintenir 3 s : valider).");
        }
        if let Some(until) = self.linking {
            let left = (until - self.last_t).ceil().max(0.0);
            return format!("Liaison CRMX en cours (≈ {} s) : les projecteurs déliés en AUTO se lient à l’univers {u}.", js::num_str(left));
        }
        if self.unlinking.is_some() {
            return "Déliaison : tous les projecteurs liés à cette box sont déliés.".into();
        }
        format!(
            "{} · entrée DMX = univers {u} · voyant APP : {}.",
            if self.crmx_mode() == "TX" { "Émetteur CRMX (TX)" } else { "Récepteur CRMX (RX)" },
            if cx.env.app_connected { "appli connectée" } else { "en attente de l’appli" }
        )
    }

    pub(crate) fn refresh(&mut self, cx: &Ctx) {
        self.fx = Fx::of(cx.fixture);
    }

    pub(crate) fn take(&mut self) -> Outcome {
        self.fx.take()
    }
}
