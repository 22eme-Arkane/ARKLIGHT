//! Moteur des commandes embarquées génériques (boutons, voyants, écran, menus), portage de
//! `shared/onboard/engine.js`. Une définition `controls` (JSON de la bibliothèque) décrit :
//!   buttons : [{ id, label, title }]
//!   leds    : [{ id, label }]
//!   display : { width, height } | null
//!   menu    : arbre de menus (si écran) — voir [`MenuNavigator`]
//!   actions : { [bouton]: { short, long, hold3, hold5, releaseAfterHold } }
//!   holdTimes : { long: 1, hold3: 3, hold5: 5 } en secondes
//!
//! Les parties écrites en fonctions dans la version JavaScript (`when`, `format`, `options()`,
//! `homeLines`, `ledStates`, `customActions`) n'existent pas en JSON : elles ne sont pas reprises.

use crate::fixture::{Ctx, Fx, Outcome};
use crate::js;
use crate::screen::{Leds, Screen, TextLine, TextScreen};
use serde_json::{Value, json};

/// Animation de retour visuel jouée sur les pixels du projecteur.
#[derive(Debug, Clone)]
pub struct PixelAnimation {
    pub kind: AnimKind,
    pub duration: f64,
    pub color: [f64; 3],
    pub level: f64,
    pub hz: f64,
    start: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimKind {
    /// jauge : pixels allumés proportionnellement au niveau (batterie…)
    Gauge,
    Blink,
    Pulse,
}

impl PixelAnimation {
    pub fn new(kind: AnimKind) -> PixelAnimation {
        PixelAnimation { kind, duration: 3.0, color: [0.0, 0.2, 1.0], level: 1.0, hz: 2.0, start: None }
    }

    /// Vrai si l'animation est terminée.
    pub fn apply(&mut self, out: &mut [f64], pixels: usize, t: f64) -> bool {
        let start = *self.start.get_or_insert(t);
        let e = t - start;
        if e > self.duration {
            return true;
        }
        let [r, g, b] = self.color;
        let mut put = |p: usize, c: [f64; 3]| {
            if let Some(px) = out.get_mut(p * 3..p * 3 + 3) {
                px.copy_from_slice(&c);
            }
        };
        match self.kind {
            AnimKind::Gauge => {
                let lit = js::round(self.level * pixels as f64).max(1.0);
                let col = if self.level > 0.5 {
                    [0.0, 1.0, 0.0]
                } else if self.level > 0.2 {
                    [1.0, 0.6, 0.0]
                } else {
                    [1.0, 0.0, 0.0]
                };
                for p in 0..pixels {
                    let on = if (p as f64) < lit { 0.6 } else { 0.0 };
                    put(p, col.map(|c| c * on));
                }
            }
            AnimKind::Blink => {
                let on = if (e * self.hz) % 1.0 < 0.5 { 1.0 } else { 0.0 };
                for p in 0..pixels {
                    put(p, [r * on, g * on, b * on]);
                }
            }
            AnimKind::Pulse => {
                let k = 0.5 - 0.5 * (e * self.hz * 2.0 * std::f64::consts::PI).cos();
                for p in 0..pixels {
                    put(p, [r * k, g * k, b * k]);
                }
            }
        }
        false
    }
}

// ------------------------------------------------------------------ menus
/// Navigation dans un arbre de menus affiché sur un petit écran. Éléments :
/// `{ label, children }`, `{ label, action }`, `{ label, setting, type: number|enum|bool, options,
/// min, max, step, wrap, unit }`.
#[derive(Debug, Clone)]
pub struct MenuNavigator {
    tree: Vec<Value>,
    /// (sous-menu ouvert, curseur du niveau parent)
    stack: Vec<(Value, i64)>,
    /// (élément, valeur en cours)
    editing: Option<(Value, Value)>,
    cursor: i64,
    pub open: bool,
}

fn options_of(item: &Value) -> Vec<(Value, Option<String>)> {
    item.get("options")
        .and_then(Value::as_array)
        .map(|o| {
            o.iter()
                .map(|x| match x {
                    Value::Array(a) => (a.first().cloned().unwrap_or(Value::Null), a.get(1).map(|l| js::string(Some(l)))),
                    _ => (x.get("value").cloned().unwrap_or(Value::Null), x.get("label").map(|l| js::string(Some(l)))),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn step_value(item: &Value, value: &Value, delta: f64) -> Value {
    match item.get("type").and_then(Value::as_str) {
        Some("number") => {
            let step = js::or(item.get("step"), 1.0);
            let (min, max) = (js::number(item.get("min")), js::number(item.get("max")));
            let wrap = js::truthy(item.get("wrap"));
            let mut v = js::number(Some(value)) + delta * step;
            if v > max {
                v = if wrap { min } else { max };
            }
            if v < min {
                v = if wrap { max } else { min };
            }
            js::jv(v)
        }
        Some(t @ ("enum" | "bool")) => {
            let opts: Vec<Value> = if t == "bool" { vec![json!(true), json!(false)] } else { options_of(item).into_iter().map(|o| o.0).collect() };
            if opts.is_empty() {
                return Value::Null;
            }
            let n = opts.len() as i64;
            let i = opts.iter().position(|o| js::strict_eq(Some(o), Some(value))).map_or(-1, |i| i as i64);
            let k = (i + delta as i64 + n) % n;
            if k < 0 { Value::Null } else { opts[k as usize].clone() }
        }
        _ => value.clone(),
    }
}

/// Texte d'une valeur de réglage.
pub fn format_value(item: &Value, value: Option<&Value>) -> String {
    match item.get("type").and_then(Value::as_str) {
        Some("bool") => (if js::truthy(value) { "ON" } else { "OFF" }).into(),
        Some("enum") => options_of(item).into_iter().find(|o| js::strict_eq(Some(&o.0), value)).and_then(|o| o.1).unwrap_or_else(|| match value {
            None | Some(Value::Null) => "—".into(),
            v => js::string(v),
        }),
        Some("number") => format!("{}{}", js::string(value), item.get("unit").and_then(Value::as_str).unwrap_or("")),
        _ => match value {
            None => String::new(),
            v => js::string(v),
        },
    }
}

impl MenuNavigator {
    pub fn new(tree: Vec<Value>) -> MenuNavigator {
        MenuNavigator { tree, stack: Vec::new(), editing: None, cursor: 0, open: false }
    }

    fn items(&self) -> Vec<Value> {
        match self.stack.last() {
            Some((it, _)) => it.get("children").and_then(Value::as_array).cloned().unwrap_or_default(),
            None => self.tree.clone(),
        }
    }

    fn current(&self) -> Option<Value> {
        self.items().get(usize::try_from(self.cursor).ok()?).cloned()
    }

    /// Ce qu'a demandé l'appui : rien, un réglage à enregistrer, une action.
    fn enter(&mut self, fx: &Fx) -> Option<MenuEffect> {
        if !self.open {
            self.open = true;
            self.cursor = 0;
            return None;
        }
        if let Some((item, value)) = self.editing.take() {
            return Some(MenuEffect::Set(item.get("setting").map(|s| js::string(Some(s))).unwrap_or_default(), value));
        }
        let it = self.current()?;
        if it.get("children").is_some_and(|c| !c.is_null()) {
            self.stack.push((it, self.cursor));
            self.cursor = 0;
        } else if let Some(a) = it.get("action").filter(|a| js::truthy(Some(a))) {
            return Some(MenuEffect::Action(js::string(Some(a))));
        } else if let Some(s) = it.get("setting").filter(|s| js::truthy(Some(s))) {
            let value = fx.s(&js::string(Some(s))).cloned().unwrap_or(Value::Null);
            self.editing = Some((it, value));
        }
        None
    }

    fn back(&mut self) {
        if self.editing.is_some() {
            self.editing = None;
        } else if let Some((_, cursor)) = self.stack.pop() {
            self.cursor = cursor;
        } else {
            self.open = false;
        }
    }

    fn move_by(&mut self, delta: i64) {
        if !self.open {
            return;
        }
        if let Some((item, value)) = &mut self.editing {
            *value = step_value(item, value, delta as f64);
            return;
        }
        let n = self.items().len() as i64;
        if n > 0 {
            self.cursor = (self.cursor + delta + n) % n;
        }
    }

    /// Lignes de texte à afficher.
    fn lines(&self, fx: &Fx, max_lines: usize) -> Vec<TextLine> {
        if !self.open {
            return Vec::new();
        }
        let title = self.stack.last().map_or_else(|| "MENU".to_string(), |(it, _)| js::string(it.get("label")));
        if let Some((it, value)) = &self.editing {
            return vec![
                TextLine::inv(js::string(it.get("label")), true),
                TextLine::new(""),
                TextLine { text: format!("> {} <", format_value(it, Some(value))), emphasis: true, ..TextLine::default() },
                TextLine::new(""),
                TextLine::new("OK = valider"),
            ];
        }
        let items = self.items();
        let mut out = vec![TextLine::inv(title, true)];
        let visible = max_lines as i64 - 1;
        let first = (self.cursor - visible / 2).min(items.len() as i64 - visible).max(0);
        for i in first..(items.len() as i64).min(first + visible) {
            let it = &items[i as usize];
            let val = if let Some(s) = it.get("setting").filter(|s| js::truthy(Some(s))) {
                format_value(it, fx.s(&js::string(Some(s))))
            } else if it.get("children").is_some_and(|c| !c.is_null()) {
                ">".into()
            } else {
                String::new()
            };
            let sel = i == self.cursor;
            out.push(TextLine { text: format!("{}{}", if sel { ">" } else { " " }, js::string(it.get("label"))), value: Some(val), selected: sel, ..TextLine::default() });
        }
        out
    }
}

enum MenuEffect {
    Set(String, Value),
    Action(String),
}

// ------------------------------------------------------------------ contrôleur
#[derive(Debug, Clone)]
struct Pressed {
    t0: f64,
    fired: Vec<&'static str>,
}

/// Contrôleur générique : interprète les gestes sur les boutons selon la définition.
pub struct OnboardController {
    def: Value,
    pixels: usize,
    hold_long: f64,
    hold3: f64,
    hold5: f64,
    pressed: Vec<(String, Pressed)>,
    anim: Option<PixelAnimation>,
    message: Option<(String, f64)>,
    /// « bluemode »
    mode: Option<String>,
    last_t: f64,
    menu: Option<MenuNavigator>,
    fx: Fx,
}

impl OnboardController {
    pub fn new(pixels: usize, controls: Value) -> OnboardController {
        let ht = controls.get("holdTimes");
        let h = |k: &str, d: f64| ht.and_then(|h| h.get(k)).and_then(Value::as_f64).unwrap_or(d);
        let menu = controls.get("menu").and_then(Value::as_array).cloned().map(MenuNavigator::new);
        OnboardController { hold_long: h("long", 1.0), hold3: h("hold3", 3.0), hold5: h("hold5", 5.0), def: controls, pixels, pressed: Vec::new(), anim: None, message: None, mode: None, last_t: 0.0, menu, fx: Fx::default() }
    }

    fn gesture(&self, id: &str, g: &str) -> Option<String> {
        self.def.get("actions").and_then(|a| a.get(id)).and_then(|a| a.get(g)).filter(|v| js::truthy(Some(v))).map(|v| js::string(Some(v)))
    }

    pub fn press(&mut self, cx: &mut Ctx, id: &str) {
        let p = Pressed { t0: cx.env.time, fired: Vec::new() };
        match self.pressed.iter_mut().find(|(k, _)| k == id) {
            Some(old) => old.1 = p,
            None => self.pressed.push((id.to_string(), p)),
        }
    }

    pub fn release(&mut self, cx: &mut Ctx, id: &str) {
        let t = cx.env.time;
        let Some(i) = self.pressed.iter().position(|(k, _)| k == id) else { return };
        let (_, p) = self.pressed.remove(i);
        let held = t - p.t0;
        if !p.fired.is_empty() {
            // un geste « maintenu » a déjà été déclenché pendant l'appui
            if let Some(a) = self.gesture(id, "releaseAfterHold") {
                self.run_action(&a);
            }
            return;
        }
        if held < self.hold_long {
            if let Some(a) = self.gesture(id, "short") {
                self.run_action(&a);
            }
        } else if let Some(a) = self.gesture(id, "long") {
            self.run_action(&a);
        }
    }

    /// À chaque image : déclenche les gestes « maintenir N secondes ».
    pub fn tick(&mut self, cx: &mut Ctx) {
        let t = cx.env.time;
        self.last_t = t;
        let ids: Vec<String> = self.pressed.iter().map(|(k, _)| k.clone()).collect();
        for id in ids {
            for (key, secs) in [("hold3", self.hold3), ("hold5", self.hold5)] {
                let Some(action) = self.gesture(&id, key) else { continue };
                let Some((_, p)) = self.pressed.iter_mut().find(|(k, _)| *k == id) else { break };
                if !p.fired.contains(&key) && t - p.t0 >= secs {
                    p.fired.push(key);
                    self.run_action(&action);
                }
            }
        }
        if self.message.as_ref().is_some_and(|(_, until)| t > *until) {
            self.message = None;
        }
    }

    pub fn held_for(&self, id: &str, t: f64) -> f64 {
        self.pressed.iter().find(|(k, _)| k == id).map_or(0.0, |(_, p)| t - p.t0)
    }

    fn say(&mut self, text: &str, seconds: f64) {
        self.message = Some((text.into(), self.last_t + seconds));
    }

    fn run_action(&mut self, name: &str) {
        let power = self.fx.power();
        match name {
            "togglePower" => {
                self.fx.set_state1("power", json!(!power));
                self.say(if power { "Extinction" } else { "Allumage" }, 2.5);
            }
            "powerOn" => {
                if !power {
                    self.fx.set_state1("power", json!(true));
                    self.say("Allumage", 2.5);
                }
            }
            "powerOff" => {
                if power {
                    self.fx.set_state1("power", json!(false));
                    self.say("Extinction", 2.5);
                }
            }
            "batteryGauge" => {
                let b = js::nn(self.fx.st("battery"), 100.0);
                self.anim = Some(PixelAnimation { level: b / 100.0, duration: 3.0, ..PixelAnimation::new(AnimKind::Gauge) });
                self.say(&format!("Batterie {} %", js::num_str(js::round(b))), 2.5);
            }
            "identify" => {
                self.anim = Some(PixelAnimation { color: [1.0, 1.0, 1.0], hz: 3.0, duration: 4.0, ..PixelAnimation::new(AnimKind::Blink) });
                self.say("Identification", 2.5);
            }
            "blueMode" => {
                self.mode = Some("bluemode".into());
                self.anim = Some(PixelAnimation { color: [0.0, 0.15, 1.0], hz: 1.5, duration: 60.0, ..PixelAnimation::new(AnimKind::Blink) });
                self.say("BlueMode : prêt à recevoir le RadioPin de l’appli", 4.0);
            }
            "menuEnter" => {
                if let Some(m) = &mut self.menu
                    && let Some(effect) = m.enter(&self.fx)
                {
                    match effect {
                        MenuEffect::Set(k, v) => self.fx.set1(&k, v),
                        MenuEffect::Action(a) => self.run_action(&a),
                    }
                }
            }
            "menuBack" => {
                if let Some(m) = &mut self.menu {
                    m.back();
                }
            }
            "menuUp" | "menuDown" => {
                if let Some(m) = &mut self.menu {
                    m.move_by(if name == "menuUp" { -1 } else { 1 });
                }
            }
            _ => self.say(&format!("({name} : non simulé)"), 2.5),
        }
    }

    /// Fin du BlueMode (appairage reçu ou annulé).
    pub fn end_blue_mode(&mut self) {
        self.mode = None;
        self.anim = None;
    }

    /// Remplace la sortie normale des pixels pendant une animation.
    pub fn override_pixels(&mut self, out: &mut [f64], pixels: usize, t: f64) -> bool {
        let Some(anim) = &mut self.anim else { return false };
        if anim.apply(out, pixels, t) {
            self.anim = None;
            if self.mode.as_deref() == Some("bluemode") {
                self.mode = None;
            }
            return false;
        }
        true
    }

    /// Voyants (`ledStates` n'existe pas en JSON : aucun).
    pub fn leds(&self) -> Leds {
        Leds::new()
    }

    pub fn display_lines(&self, max_lines: usize) -> Vec<TextLine> {
        let mut lines = self.menu.as_ref().map(|m| m.lines(&self.fx, max_lines)).unwrap_or_default();
        if let Some((text, _)) = &self.message
            && !self.menu.as_ref().is_some_and(|m| m.open)
        {
            lines.truncate(max_lines.saturating_sub(1));
            lines.push(TextLine::new(text.clone()));
        }
        lines
    }

    pub fn display(&self) -> Screen {
        if self.def.get("display").is_none_or(Value::is_null) {
            return Screen::None;
        }
        Screen::Text(TextScreen { off: false, dim: false, icons: None, big: false, lines: self.display_lines(6) })
    }

    pub fn hint(&self) -> String {
        self.message.as_ref().map_or_else(String::new, |(t, _)| t.clone())
    }

    /// Nombre de pixels du modèle.
    pub fn pixels(&self) -> usize {
        self.pixels
    }

    pub(crate) fn refresh(&mut self, cx: &Ctx) {
        self.fx = Fx::of(cx.fixture);
    }

    pub(crate) fn take(&mut self) -> Outcome {
        self.fx.take()
    }
}
