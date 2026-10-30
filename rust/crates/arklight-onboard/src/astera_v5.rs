//! Panneau de commande Astera « V5 » avec écran OLED (Titan / Helios / Hyperion, BTB, AX2, Quik…),
//! portage de `shared/onboard/astera-v5.js`.
//! Clavier : ⏻, MENU, −, +, ENTER, palette, soleil (+ touche ASTERA sur l'AX2 et les Quik).
//! Comportement d'après le Titan TRM 2020 et les guides 2022-2023 : docs/recherche/menus/tubes.md.

use crate::fixture::{Ctx, Fx, obj};
use crate::js::{self, jv};
use crate::screen::{Icons, Led, Leds, Screen, TextLine, TextScreen};
use arklight_core::engine::astera::{Input, standalone_speed_seconds};
use arklight_core::engine::gels::lee_gels;
use arklight_core::engine::output::Source;
use arklight_core::library::{DmxMode, FixtureType, library};
use serde_json::{Map, Value, json};

pub const INPUT_SELECT: [&str; 6] = ["AUTO", "STANDALONE", "APP CONTROL", "Wired DMX", "ART3 DMX", "CRMX wirel. DMX"];
const DIMMER_CURVES: [&str; 5] = ["FAST", "STANDARD", "HALOGEN", "THEATER", "TV"];
const DMX_FAIL: [&str; 4] = ["HOLD", "EMERGENCY LIGHT", "BLACKOUT", "STANDALONE"];
const AC_FAILURE: [&str; 4] = ["NO ACTION", "BLACKOUT", "EMERGENCY LIGHT", "POWEROFF"];
const RF_REGIONS: [&str; 7] = ["EU", "USA", "AUS", "SGP", "KOR", "RUS", "JPN"];
pub const STANDALONE_PROGRAMS: [&str; 17] = [
    "ONE COLOR STATIC",
    "TWO COLOR STATIC",
    "THREE COLOR STATIC",
    "FOUR COLOR STATIC",
    "ONE COLOR FADE",
    "TWO COLOR FADE",
    "THREE COLOR FADE",
    "FOUR COLOR FADE",
    "SIMPLE RUNNING",
    "DOUBLE RUNNING",
    "TWO COL RUNNING",
    "FLAG RUNNING",
    "DOUBLE FLAG RUNNING",
    "SPIRAL FOUR COLORS",
    "SPIRAL TWO COLORS",
    "RAINBOW",
    "FIRE",
];
pub const GEL_FAMILIES: [&str; 2] = ["Lee Daylight", "Lee Tungsten"];
const IDLE_TO_STATUS: f64 = 180.0;
const IDLE_DIM: f64 = 30.0;
const DEFAULT_RING: [&str; 10] = ["INPUT SELECT", "STATIC COLOR", "DMX ADDRESS", "DMX SETTINGS", "UNLINK CRMX", "RUNTIME", "STANDALONE", "AC FAILURE", "INFO", "RESET SETTINGS"];
const DEFAULT_DMX_SETTINGS: [&str; 6] = ["PIXELS", "DMX PROFILE", "STROBE", "DIMMER CURVE", "DMX FAIL", "FIXTURE NUMBER"];
const STANDALONE_ITEMS: [&str; 12] = ["PROGRAM", "INTENSITY", "SPEED", "FADE", "COLOR 1", "COLOR 2", "COLOR 3", "COLOR 4", "SET ADDRESS", "SET SIZE", "POS IN SET", "GROUP ADDRESS"];
const DEFAULT_INFO: [&str; 11] = ["Radio PIN", "RF Region", "AX1 Compatibility", "Serial number", "Firmware version", "Power-on hours", "RF link", "CRMX Version", "Battery state", "Battery Cycles", "Calibration"];

/// Couleurs du menu STATIC COLOR (« RED », « 5500K »…).
pub fn color_names() -> Vec<&'static str> {
    library().static_colors.iter().map(|(n, _)| n.as_str()).collect()
}

fn pad3(v: &Value) -> String {
    js::pad_start(&js::string(Some(v)), 3, '0')
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_string() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

fn strs(list: &[&str]) -> Vec<Value> {
    list.iter().map(|s| Value::from(*s)).collect()
}

// ------------------------------------------------------------------ profils DMX
fn mode_strobe(m: &DmxMode) -> &str {
    js::text_or(m.extra.get("strobe"), "OFF")
}

fn mode_pixels(m: &DmxMode) -> Option<f64> {
    m.pixels.as_ref().and_then(Value::as_f64)
}

fn is_effect(m: &DmxMode) -> bool {
    m.pixels.as_ref().and_then(Value::as_str) == Some("effet")
}

/// Profils DMX disponibles pour un nombre de pixels et un mode stroboscope (les modes d'effet avec 1 pixel).
fn profiles_for(ty: &'static FixtureType, pixels: f64, strobe: &str) -> Vec<&'static DmxMode> {
    ty.dmx.modes.iter().filter(|m| (mode_pixels(m) == Some(pixels) && mode_strobe(m) == strobe) || (pixels == 1.0 && strobe == "OFF" && is_effect(m))).collect()
}

/// Découpe `split(/\s+/)` (un séparateur en tête donne un mot vide).
fn split_ws(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_ws = false;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            if !in_ws {
                out.push(&s[start..i]);
                in_ws = true;
            }
        } else if in_ws {
            start = i;
            in_ws = false;
        }
    }
    out.push(if in_ws { "" } else { &s[start..] });
    out
}

/// Nom « de base » d'un profil, sans marque de stroboscope ni répétition par pixel (« DIM RGBS DIM RGBS » -> « DIM RGB »).
pub fn profile_base_name(name: &str) -> String {
    let mut s = name.to_string();
    // /\s*\.\.\s*S+$/
    let no_s = s.trim_end_matches('S');
    if no_s.len() < s.len() {
        let a = no_s.trim_end();
        if let Some(b) = a.strip_suffix("..") {
            s = b.trim_end().to_string();
        }
    }
    // /\s+S$/
    if let Some(a) = s.strip_suffix('S') {
        let b = a.trim_end();
        if b.len() < a.len() {
            s = b.to_string();
        }
    }
    let words: Vec<String> = split_ws(&s)
        .into_iter()
        .map(|w| {
            let ok = w.len() >= 2 && w.ends_with('S') && w.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'.') && w != "IND";
            if ok { w[..w.len() - 1].to_string() } else { w.to_string() }
        })
        .collect();
    let n = words.len();
    let out = if n.is_multiple_of(2) && words[..n / 2].join(" ") == words[n / 2..].join(" ") { &words[..n / 2] } else { &words[..] };
    let j = out.join(" ");
    j.strip_suffix('.').map_or(j.clone(), str::to_string)
}

/// Recherche le profil équivalent quand PIXELS ou STROBE changent.
fn resolve_profile(ty: &'static FixtureType, pixels: f64, strobe: &str, prefer: &str) -> Option<&'static DmxMode> {
    let list: Vec<_> = profiles_for(ty, pixels, strobe).into_iter().filter(|m| !is_effect(m)).collect();
    let want = profile_base_name(prefer);
    list.iter().find(|m| profile_base_name(&m.name) == want).or(list.first()).copied()
}

/// Réglages appliqués par RESET SETTINGS (firmware actuel), comme `resetSettings` de
/// `shared/engine/astera-light.js`.
pub fn reset_settings(ty: &FixtureType, settings: &Map<String, Value>) -> Map<String, Value> {
    let mut m = obj([("inputSelect", json!("AUTO")), ("crmxLinked", json!(false)), ("runtime", json!(5))]);
    if ty.output_gain.is_some_and(|g| g != 0.0 && !g.is_nan()) {
        m.insert("outputGain".into(), json!("ON"));
    }
    m.insert("rfRegion".into(), Value::Null);
    if js::truthy(ty.extra.get("btb")) {
        m.insert("btbBonded".into(), json!(false));
    }
    let sa = settings.get("standalone");
    let mut colors = vec![json!({ "kind": "named", "name": "RED" })];
    colors.extend(sa.and_then(|s| s.get("colors")).and_then(Value::as_array).map(|c| c.iter().skip(1).cloned().collect::<Vec<_>>()).unwrap_or_default());
    let standalone = js::spread(sa, &obj([("program", json!(0)), ("intensity", json!(100)), ("colors", Value::Array(colors))]));
    m.insert("standalone".into(), Value::Object(standalone));
    m
}

// ------------------------------------------------------------------ écrans
/// Élément de menu en cours de réglage, avec les réglages lus à son ouverture (comme les
/// fermetures de la version JavaScript).
#[derive(Debug, Clone)]
struct Def {
    name: String,
    kind: DefKind,
    /// `settings.standalone` à l'ouverture
    sa: Value,
    /// mode DMX à l'ouverture
    mode: Option<String>,
}

#[derive(Debug, Clone)]
enum DefKind {
    Num { min: f64, max: f64 },
    Opt(Vec<Value>),
}

#[derive(Debug, Clone, Copy)]
struct Hsi {
    cct: f64,
    green: f64,
    hue: f64,
    sat: f64,
}

#[derive(Debug, Clone)]
enum Ui {
    Status,
    Ring { i: i64 },
    Value { entry: String, def: Def, value: Value, parent: Box<Ui>, live: bool },
    SubValue { name: String, def: Def, value: Value, parent: Box<Ui>, live: bool },
    List { entry: String, i: i64, parent: Box<Ui> },
    Pin { key: String, title: String, digits: Vec<i64>, pos: usize, parent: Box<Ui> },
    Confirm { parent: Box<Ui>, yes: bool },
    Hsi { line: i64, color: Hsi },
    Static { i: i64 },
    Gel { line: i64, family: i64, gel: i64 },
    Bright { line: i64 },
    Tilt,
}

impl Ui {
    fn screen(&self) -> &'static str {
        match self {
            Ui::Status => "status",
            Ui::Ring { .. } => "ring",
            Ui::Value { .. } => "value",
            Ui::SubValue { .. } => "subvalue",
            Ui::List { .. } => "list",
            Ui::Pin { .. } => "pin",
            Ui::Confirm { .. } => "confirm",
            Ui::Hsi { .. } => "hsi",
            Ui::Static { .. } => "static",
            Ui::Gel { .. } => "gel",
            Ui::Bright { .. } => "bright",
            Ui::Tilt => "tilt",
        }
    }

    fn parent(&self) -> Option<&Ui> {
        match self {
            Ui::Value { parent, .. } | Ui::SubValue { parent, .. } | Ui::List { parent, .. } | Ui::Pin { parent, .. } | Ui::Confirm { parent, .. } => Some(parent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct Press {
    t0: f64,
    fired: Vec<&'static str>,
    next_repeat: f64,
    deferred: bool,
}

/// Action à la fin d'un message (« UNLINKING... »).
#[derive(Debug, Clone, Copy)]
enum Then {
    Unlink,
    Unbond,
}

#[derive(Debug, Clone)]
struct Message {
    lines: Vec<String>,
    until: f64,
    then: Option<Then>,
}

/// Zone du TouchSlider (HydraPanel) : haut = +1, milieu = ENTER, bas = −1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderZone {
    Top,
    Middle,
    Bottom,
}

pub(crate) struct AsteraV5 {
    ty: &'static FixtureType,
    cfg: Map<String, Value>,
    /// touches MENU/COLOR, −/SPEED, +/BRIGHT, ENTER/PROG (AX5, AX7, AX10)
    dual: bool,
    ui: Ui,
    pressed: Vec<(String, Press)>,
    last_key: f64,
    last_t: f64,
    blue_since: Option<f64>,
    message: Option<Message>,
    fx: Fx,
}

impl AsteraV5 {
    pub fn new(ty: &'static FixtureType) -> AsteraV5 {
        let cfg = ty.extra.get("controls").and_then(Value::as_object).cloned().unwrap_or_default();
        let dual = cfg.get("keyfoil").and_then(Value::as_str) == Some("dual");
        AsteraV5 { ty, cfg, dual, ui: Ui::Status, pressed: Vec::new(), last_key: 0.0, last_t: 0.0, blue_since: None, message: None, fx: Fx::default() }
    }

    fn cfg_list(&self, key: &str) -> Option<Vec<String>> {
        self.cfg.get(key).and_then(Value::as_array).map(|a| a.iter().map(|v| js::string(Some(v))).collect())
    }

    fn cfg_str(&self, key: &str) -> Option<&str> {
        self.cfg.get(key).and_then(Value::as_str)
    }

    fn sa(&self) -> Option<&Value> {
        self.fx.s("standalone")
    }

    // ---------------------------------------------------------------- utilitaires
    pub fn ring(&self) -> Vec<String> {
        if let Some(r) = self.cfg_list("ring") {
            return r;
        }
        let mut r: Vec<String> = DEFAULT_RING.iter().map(|s| s.to_string()).collect();
        // UNBOND BLUETOOTH suit UNLINK CRMX (guides 2023) ; l'HydraPanel (guide 2022) montre l'ordre inverse
        if js::truthy(self.ty.extra.get("btb")) {
            let at = if self.ty.extra.get("ringOrder").and_then(Value::as_str) == Some("unbondFirst") { 4 } else { 5 };
            r.insert(at, "UNBOND BLUETOOTH".into());
        }
        if let Some(extra) = self.cfg_list("extraRing") {
            let at = r.iter().position(|x| x == "DMX SETTINGS").map_or(0, |i| i + 1);
            for (k, e) in extra.into_iter().enumerate() {
                r.insert(at + k, e);
            }
        }
        r
    }

    fn ring_index(&self, name: &str) -> i64 {
        self.ring().iter().position(|x| x == name).map_or(-1, |i| i as i64)
    }

    fn mode(&self) -> Option<&'static DmxMode> {
        let ty: &'static FixtureType = self.ty;
        ty.mode(self.fx.s("dmxMode").and_then(Value::as_str))
    }

    fn latch_standalone(&mut self, cx: &mut Ctx) {
        let sel = self.fx.s("inputSelect");
        if js::strict_eq(sel, Some(&json!("AUTO"))) || js::strict_eq(sel, Some(&json!("STANDALONE"))) {
            cx.astera.latch_standalone(cx.runtime);
        }
    }

    fn set_standalone(&mut self, patch: Map<String, Value>) {
        let sa = js::spread(self.sa(), &patch);
        self.fx.set1("standalone", Value::Object(sa));
    }

    fn set_color0(&mut self, spec: Value) {
        let mut colors: Vec<Value> = self.sa().and_then(|s| s.get("colors")).and_then(Value::as_array).cloned().unwrap_or_default();
        if colors.is_empty() {
            colors.push(spec);
        } else {
            colors[0] = spec;
        }
        self.set_standalone(obj([("program", json!(0)), ("colors", Value::Array(colors))]));
    }

    fn show(&mut self, lines: &[&str], seconds: f64, then: Option<Then>) {
        self.message = Some(Message { lines: lines.iter().map(|s| s.to_string()).collect(), until: self.last_t + seconds, then });
    }

    // ---------------------------------------------------------------- éléments de menu
    /// Définition éditable d'un élément de menu.
    fn item(&self, name: &str) -> Option<Def> {
        let m = self.mode();
        let px = m.and_then(mode_pixels).unwrap_or(1.0);
        let num = |min, max| DefKind::Num { min, max };
        let kind = match name {
            "INPUT SELECT" => DefKind::Opt(self.cfg.get("inputSelect").and_then(Value::as_array).cloned().unwrap_or_else(|| strs(&INPUT_SELECT))),
            "DMX ADDRESS" => num(1.0, 512.0),
            "RUNTIME" => num(1.0, 20.0),
            "AC FAILURE" => DefKind::Opt(strs(&AC_FAILURE)),
            "PIXELS" => DefKind::Opt(self.ty.extra.get("pixelOptions").and_then(Value::as_array).cloned().unwrap_or_default()),
            "DMX PROFILE" => DefKind::Opt(profiles_for(self.ty, px, m.map_or("OFF", mode_strobe)).iter().map(|p| json!(p.id)).collect()),
            "STROBE" => DefKind::Opt(if px == 1.0 { strs(&["OFF", "ON"]) } else { strs(&["OFF", "SINGLE", "MULTIPLE"]) }),
            "DIMMER CURVE" => DefKind::Opt(strs(&DIMMER_CURVES)),
            "DMX FAIL" => DefKind::Opt(strs(&DMX_FAIL)),
            "PROGRAM" => DefKind::Opt((0..STANDALONE_PROGRAMS.len()).map(|i| json!(i)).collect()),
            "INTENSITY" | "FADE" | "BRIGHTNESS" => num(0.0, 100.0),
            "SPEED" => num(0.0, 9.0),
            "OUTPUT GAIN" => DefKind::Opt(strs(&["ON", "OFF"])),
            "FAN" => DefKind::Opt(strs(&["OFF", "ON"])),
            "FAN CONTROL" => DefKind::Opt(strs(&["OFF", "AUTO", "LOW", "MED", "HIGH"])),
            "COLOR 1" | "COLOR 2" | "COLOR 3" | "COLOR 4" => DefKind::Opt(strs(&color_names())),
            "SET ADDRESS" | "INDEX COLOR" | "RED" | "GREEN" | "BLUE" => num(0.0, 255.0),
            "FIXTURE NUMBER" => num(1.0, 255.0),
            "SET SIZE" | "POS IN SET" => num(1.0, 32.0),
            "GROUP ADDRESS" => num(1.0, 4.0),
            "RF Region" => DefKind::Opt(strs(&RF_REGIONS)),
            "AX1 Compatibility" => DefKind::Opt(vec![json!(true), json!(false)]),
            _ => return None,
        };
        Some(Def { name: name.into(), kind, sa: self.sa().cloned().unwrap_or(Value::Null), mode: m.map(|m| m.id.clone()) })
    }

    fn def_mode(&self, d: &Def) -> Option<&'static DmxMode> {
        let ty: &'static FixtureType = self.ty;
        d.mode.as_deref().and_then(|id| ty.dmx.modes.iter().find(|m| m.id == id))
    }

    /// Valeur actuelle de l'élément (lue à l'ouverture).
    fn def_get(&self, d: &Def) -> Value {
        let s = |k: &str| self.fx.s(k).cloned().unwrap_or(Value::Null);
        let sa_get = |k: &str| d.sa.get(k).cloned().unwrap_or(Value::Null);
        let color = |i: usize| d.sa.get("colors").and_then(|c| c.get(i)).filter(|c| c.get("kind").and_then(Value::as_str).is_some());
        let m = self.def_mode(d);
        match d.name.as_str() {
            "INPUT SELECT" => s("inputSelect"),
            "DMX ADDRESS" => s("dmxAddress"),
            "RUNTIME" => s("runtime"),
            "AC FAILURE" => s("acFailure"),
            "PIXELS" => jv(m.and_then(mode_pixels).unwrap_or(1.0)),
            "DMX PROFILE" => s("dmxMode"),
            "STROBE" => json!(m.map_or("OFF", mode_strobe)),
            "DIMMER CURVE" => s("dimmerCurve"),
            "DMX FAIL" => s("dmxFail"),
            "PROGRAM" => sa_get("program"),
            "INTENSITY" => sa_get("intensity"),
            "SPEED" => sa_get("speed"),
            "FADE" => sa_get("fade"),
            "OUTPUT GAIN" => json!(js::text_or(self.fx.s("outputGain"), "ON")),
            "FAN" => json!(if m.is_some_and(|m| js::truthy(m.extra.get("fan"))) { "ON" } else { "OFF" }),
            "FAN CONTROL" => json!(js::text_or(self.fx.s("fan"), "AUTO")),
            "BRIGHTNESS" => jv(js::nn(self.fx.s("brightness"), 100.0)),
            "COLOR 1" | "COLOR 2" | "COLOR 3" | "COLOR 4" => {
                let i = d.name[6..].parse::<usize>().unwrap_or(1) - 1;
                match color(i) {
                    Some(c) if c.get("kind").and_then(Value::as_str) == Some("named") => c.get("name").cloned().unwrap_or(Value::Null),
                    _ => json!(color_names().first().copied().unwrap_or("")),
                }
            }
            "SET ADDRESS" => s("setAddress"),
            "FIXTURE NUMBER" => jv(js::nn(self.fx.s("fixtureNumber"), 1.0)),
            "SET SIZE" => s("setSize"),
            "POS IN SET" => s("posInSet"),
            "GROUP ADDRESS" => s("groupAddress"),
            "INDEX COLOR" => match color(0) {
                Some(c) if c.get("kind").and_then(Value::as_str) == Some("index") => c.get("value").cloned().unwrap_or(Value::Null),
                _ => json!(0),
            },
            "RED" | "GREEN" | "BLUE" => {
                let ch = ["RED", "GREEN", "BLUE"].iter().position(|n| *n == d.name).unwrap_or(0);
                Self::custom_rgb(&d.sa).get(ch).cloned().unwrap_or(Value::Null)
            }
            "RF Region" => json!(js::text_or(self.fx.s("rfRegion"), "EU")),
            "AX1 Compatibility" => json!(js::truthy(self.fx.s("ax1Compat"))),
            _ => Value::Null,
        }
    }

    /// `sa.colors[0].rgb` d'une couleur personnalisée, sinon [0, 0, 0].
    fn custom_rgb(sa: &Value) -> Vec<Value> {
        match sa.get("colors").and_then(|c| c.get(0)) {
            Some(c) if c.get("kind").and_then(Value::as_str) == Some("custom") => c.get("rgb").and_then(Value::as_array).cloned().unwrap_or_default(),
            _ => vec![json!(0), json!(0), json!(0)],
        }
    }

    /// Enregistre la valeur choisie.
    fn def_set(&mut self, cx: &mut Ctx, d: &Def, v: Value) {
        let m = self.def_mode(d);
        match d.name.as_str() {
            "INPUT SELECT" => {
                cx.astera.unlatch(cx.runtime);
                self.fx.set1("inputSelect", v);
            }
            "DMX ADDRESS" => self.fx.set1("dmxAddress", v),
            "RUNTIME" => self.fx.set1("runtime", v),
            "AC FAILURE" => self.fx.set1("acFailure", v),
            "PIXELS" => {
                let n = js::number(Some(&v));
                let cur = m.map_or("OFF", mode_strobe);
                let strobe = if n == 1.0 {
                    if cur == "OFF" { "OFF" } else { "ON" }
                } else if cur == "ON" {
                    "SINGLE"
                } else {
                    cur
                };
                let name = m.map_or("", |m| m.name.as_str());
                if let Some(r) = resolve_profile(self.ty, n, strobe, name).or_else(|| resolve_profile(self.ty, n, "OFF", name)) {
                    self.fx.set1("dmxMode", json!(r.id));
                }
            }
            "DMX PROFILE" => self.fx.set1("dmxMode", v),
            "STROBE" => {
                let px = m.and_then(mode_pixels).unwrap_or(1.0);
                if let Some(r) = resolve_profile(self.ty, px, v.as_str().unwrap_or(""), m.map_or("", |m| m.name.as_str())) {
                    self.fx.set1("dmxMode", json!(r.id));
                }
            }
            "DIMMER CURVE" => self.fx.set1("dimmerCurve", v),
            "DMX FAIL" => self.fx.set1("dmxFail", v),
            "PROGRAM" => self.set_standalone(obj([("program", v)])),
            "INTENSITY" => self.set_standalone(obj([("intensity", v)])),
            "SPEED" => self.set_standalone(obj([("speed", v)])),
            "FADE" => self.set_standalone(obj([("fade", v)])),
            "OUTPUT GAIN" => self.fx.set1("outputGain", v),
            "FAN" => {
                // bascule entre un profil et sa variante « FAN » (canal ventilateur en plus)
                let Some(cur) = m else { return };
                let modes = &self.ty.dmx.modes;
                let target = if v.as_str() == Some("ON") {
                    modes.iter().find(|x| x.extra.get("baseId").and_then(Value::as_str) == Some(cur.id.as_str()))
                } else {
                    let base = cur.extra.get("baseId");
                    modes.iter().find(|x| js::strict_eq(Some(&json!(x.id)), base))
                };
                if let Some(t) = target {
                    self.fx.set1("dmxMode", json!(t.id));
                }
            }
            "FAN CONTROL" => self.fx.set1("fan", v),
            "BRIGHTNESS" => self.fx.set1("brightness", v),
            "COLOR 1" | "COLOR 2" | "COLOR 3" | "COLOR 4" => {
                let i = d.name[6..].parse::<usize>().unwrap_or(1) - 1;
                let mut colors: Vec<Value> = d.sa.get("colors").and_then(Value::as_array).cloned().unwrap_or_default();
                if colors.len() <= i {
                    colors.resize(i + 1, Value::Null);
                }
                colors[i] = json!({ "kind": "named", "name": v });
                self.set_standalone(obj([("colors", Value::Array(colors))]));
            }
            "SET ADDRESS" => self.fx.set1("setAddress", v),
            "FIXTURE NUMBER" => self.fx.set1("fixtureNumber", v),
            "SET SIZE" => self.fx.set1("setSize", v),
            "POS IN SET" => self.fx.set1("posInSet", v),
            "GROUP ADDRESS" => self.fx.set1("groupAddress", v),
            "INDEX COLOR" => self.set_color0(json!({ "kind": "index", "value": v })),
            "RED" | "GREEN" | "BLUE" => {
                let ch = ["RED", "GREEN", "BLUE"].iter().position(|n| *n == d.name).unwrap_or(0);
                let mut rgb = Self::custom_rgb(&d.sa);
                if rgb.len() <= ch {
                    rgb.resize(ch + 1, Value::Null);
                }
                rgb[ch] = v;
                self.set_color0(json!({ "kind": "custom", "rgb": rgb }));
            }
            "RF Region" => self.fx.set1("rfRegion", v),
            "AX1 Compatibility" => self.fx.set1("ax1Compat", v),
            _ => {}
        }
    }

    /// Texte d'une valeur.
    fn def_fmt(&self, d: &Def, v: &Value) -> String {
        let s = js::string(Some(v));
        match d.name.as_str() {
            "DMX ADDRESS" => pad3(v),
            "RUNTIME" => format!("{s}h"),
            "INTENSITY" | "FADE" | "BRIGHTNESS" => format!("{s}%"),
            "SPEED" => format!("{s}:{}", js::to_fixed(standalone_speed_seconds(js::number(Some(v))), 2)),
            "DMX PROFILE" => match self.ty.dmx.modes.iter().find(|m| js::strict_eq(Some(&json!(m.id)), Some(v))) {
                Some(p) => format!("{}:{}", js::string(p.extra.get("number")), p.name),
                None => s,
            },
            "PROGRAM" => v.as_f64().and_then(|i| STANDALONE_PROGRAMS.get(i as usize)).map_or_else(|| "undefined".into(), |p| p.to_string()),
            "AX1 Compatibility" => (if js::truthy(Some(v)) { "ENABLED" } else { "DISABLED" }).into(),
            _ => s,
        }
    }

    /// Contenu des sous-menus.
    pub fn sub_items(&self, entry: &str) -> Vec<String> {
        let own = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        match entry {
            // FIXTURE NUMBER : vu sur l'HydraPanel (« DMX Settings: / FIXTURE NUMBER »), position et plage déduites
            "DMX SETTINGS" => self.cfg_list("dmxSettings").unwrap_or_else(|| own(&DEFAULT_DMX_SETTINGS)),
            "STANDALONE" => own(&STANDALONE_ITEMS),
            "STATIC COLOR" => {
                let mut v = own(&color_names());
                v.push("INDEX COLOR".into());
                v.push("CUSTOM COLOR".into());
                v
            }
            "CUSTOM COLOR" => own(&["RED", "GREEN", "BLUE"]),
            "INFO" => self.cfg_list("infoItems").unwrap_or_else(|| own(&DEFAULT_INFO)),
            _ => Vec::new(),
        }
    }

    fn info_value(&self, name: &str) -> String {
        let id = &self.fx.id;
        let tail: String = {
            let c: Vec<char> = id.chars().collect();
            c[c.len().saturating_sub(6)..].iter().collect()
        };
        let n = js::parse_int36(if tail.is_empty() { "0" } else { &tail });
        let serial_tail = js::pad_start(&js::num_str(n % 10000.0), 4, '0');
        match name {
            "Radio PIN" => js::text_or(self.fx.s("radioPin"), "0000").to_string(),
            "Serial number" => format!("000-00000 {serial_tail}"),
            "Firmware version" => "5.16.24.U  HW001".into(),
            "Power-on hours" => "00042h".into(),
            "RF link" => "-36.0dBm -0.0ppm".into(),
            "CRMX Version" => "V1.0.5.0".into(),
            "Battery state" => format!("{}%", js::num_str(js::round(js::nn(self.fx.st("battery"), 100.0)))),
            "Battery Cycles" => "012".into(),
            "Calibration" => "2024-03-12-0001".into(),
            _ => String::new(),
        }
    }

    // ---------------------------------------------------------------- touches
    fn pressed_mut(&mut self, id: &str) -> Option<&mut Press> {
        self.pressed.iter_mut().find(|(k, _)| k == id).map(|(_, p)| p)
    }

    pub fn press(&mut self, cx: &mut Ctx, id: &str) {
        let t = cx.env.time;
        self.last_t = t;
        self.last_key = t;
        let p = Press { t0: t, fired: Vec::new(), next_repeat: t + 0.5, deferred: false };
        match self.pressed_mut(id) {
            Some(old) => *old = p,
            None => self.pressed.push((id.to_string(), p)),
        }
        if id == "power" || !self.fx.power() {
            return;
        }
        if self.dual && matches!(self.ui, Ui::Status) && ["menu", "minus", "plus", "enter"].contains(&id) {
            // décidé au relâchement (court) ou au bout de 2 s (maintien)
            if let Some(p) = self.pressed_mut(id) {
                p.deferred = true;
            }
            return;
        }
        self.key(cx, id, 1.0);
    }

    pub fn release(&mut self, cx: &mut Ctx, id: &str) {
        let t = cx.env.time;
        self.last_t = t;
        let p = self.pressed.iter().position(|(k, _)| k == id).map(|i| self.pressed.remove(i).1);
        if let Some(p) = &p
            && p.deferred
            && p.fired.is_empty()
        {
            // le maintien est aussi vérifié au relâchement (si l'animation n'a pas tourné entre-temps)
            if t - p.t0 >= 2.0 {
                self.hold_action(cx, id);
            } else {
                self.key(cx, id, 1.0);
            }
            return;
        }
        let Some(p) = p else { return };
        if id != "power" {
            return;
        }
        let held = t - p.t0;
        let power = self.fx.power();
        if p.fired.is_empty() {
            if !power && held >= 1.0 {
                self.fx.set_state1("power", json!(true));
                self.ui = Ui::Status;
                cx.astera.unlatch(cx.runtime);
                return;
            }
            if power && held >= 3.0 {
                self.fx.set_state1("blueMode", json!(true));
                self.blue_since = Some(t);
                return;
            }
        }
        if !p.fired.is_empty() {
            return;
        }
        if power && held < 3.0 {
            if js::truthy(self.fx.st("blueMode")) {
                self.fx.set_state1("blueMode", json!(false));
                self.blue_since = None;
            } else {
                self.fx.set_state1("power", json!(false));
                self.ui = Ui::Status;
            }
        }
    }

    pub fn tick(&mut self, cx: &mut Ctx) {
        let t = cx.env.time;
        self.last_t = t;
        let ids: Vec<String> = self.pressed.iter().map(|(k, _)| k.clone()).collect();
        for id in ids {
            let Some(p) = self.pressed_mut(&id).cloned() else { continue };
            let held = t - p.t0;
            let power = self.fx.power();
            if id == "power" {
                if !power && held >= 1.0 && !p.fired.contains(&"on") {
                    if let Some(p) = self.pressed_mut(&id) {
                        p.fired.push("on");
                    }
                    self.fx.set_state1("power", json!(true));
                    self.ui = Ui::Status;
                    cx.astera.unlatch(cx.runtime);
                } else if power && held >= 3.0 && p.fired.is_empty() {
                    if let Some(p) = self.pressed_mut(&id) {
                        p.fired.push("blue");
                    }
                    self.fx.set_state1("blueMode", json!(true));
                    self.blue_since = Some(t);
                }
            } else if p.deferred {
                if held >= 2.0 && p.fired.is_empty() {
                    if let Some(p) = self.pressed_mut(&id) {
                        p.fired.push("hold");
                    }
                    self.hold_action(cx, &id);
                }
            } else if (id == "plus" || id == "minus") && t >= p.next_repeat && power {
                // répétition automatique, accélérée après 2 s
                if let Some(p) = self.pressed_mut(&id) {
                    p.next_repeat = t + 0.08;
                }
                self.key(cx, &id, if held > 2.0 { 10.0 } else { 1.0 });
            }
        }
        if js::truthy(self.fx.st("blueMode")) && self.blue_since.is_some_and(|b| t - b > 120.0) {
            self.fx.set_state1("blueMode", json!(false));
            self.blue_since = None;
        }
        if let Some(m) = &self.message
            && t > m.until
        {
            let then = m.then;
            self.message = None;
            match then {
                Some(Then::Unlink) => {
                    self.fx.set1("crmxLinked", json!(false));
                    cx.astera.unlatch(cx.runtime);
                    cx.astera.just_unlinked = true;
                    self.ui = Ui::Status;
                }
                Some(Then::Unbond) => {
                    self.fx.set1("btbBonded", json!(false));
                    self.ui = Ui::Status;
                }
                None => {}
            }
        }
        if !matches!(self.ui, Ui::Status) && t - self.last_key > IDLE_TO_STATUS {
            self.ui = Ui::Status;
        }
    }

    pub fn held_for(&self, id: &str, t: f64) -> f64 {
        self.pressed.iter().find(|(k, _)| k == id).map_or(0.0, |(_, p)| t - p.t0)
    }

    /// Action d'une touche selon l'écran affiché. `step` = pas des touches +/− (répétition).
    fn key(&mut self, cx: &mut Ctx, id: &str, step: f64) {
        if self.message.is_some() {
            return;
        }
        let dir: f64 = match id {
            "plus" => 1.0,
            "minus" => -1.0,
            _ => 0.0,
        };

        // Touches directes
        if id == "palette" {
            self.latch_standalone(cx);
            let next = match self.ui {
                Ui::Hsi { .. } => "static",
                Ui::Static { .. } => "gel",
                _ => "hsi",
            };
            self.open_color_page(next);
            return;
        }
        if id == "sun" {
            self.ui = Ui::Bright { line: 0 };
            return;
        }
        if id == "astera" {
            if self.cfg_str("asteraKey") == Some("tilt") {
                // Quik : la touche ASTERA bascule l'affichage de l'inclinaison (accéléromètre)
                self.ui = if matches!(self.ui, Ui::Tilt) { Ui::Status } else { Ui::Tilt };
            } else {
                self.show(&["AUTO ADDRESS", "non documenté"], 1.5, None);
            }
            return;
        }
        if matches!(self.ui, Ui::Tilt) {
            return; // autres touches ignorées pendant l'affichage de l'inclinaison (non documenté)
        }

        let mut ui = std::mem::replace(&mut self.ui, Ui::Status);
        match &mut ui {
            Ui::Status => {
                self.ui = Ui::Status;
                if id == "enter" {
                    if js::truthy(self.fx.st("acceptConfig")) {
                        return self.fx.set_state1("acceptConfig", json!(false));
                    }
                    if js::truthy(self.fx.st("standby")) {
                        return self.fx.set_state1("standby", json!(false));
                    }
                    self.ui = Ui::Ring { i: self.ring_index("DMX ADDRESS") };
                } else if id == "menu" || id == "plus" {
                    self.ui = Ui::Ring { i: 0 };
                } else if id == "minus" {
                    self.ui = Ui::Ring { i: self.ring().len() as i64 - 1 };
                }
            }
            Ui::Ring { i } => {
                let ring = self.ring();
                let i = *i;
                self.ui = ui;
                if id == "menu" {
                    self.ui = Ui::Status;
                } else if dir != 0.0 {
                    let n = i + dir as i64;
                    self.ui = if n < 0 || n >= ring.len() as i64 { Ui::Status } else { Ui::Ring { i: n } };
                } else if id == "enter" {
                    let entry = ring.get(i.max(0) as usize).cloned().unwrap_or_default();
                    self.open_entry(cx, &entry);
                }
            }
            Ui::Value { def, value, live, .. } | Ui::SubValue { def, value, live, .. } => {
                if id == "menu" {
                    self.ui = ui;
                    return self.back();
                }
                if id == "enter" {
                    let (d, v) = (def.clone(), value.clone());
                    self.ui = ui;
                    self.def_set(cx, &d, v);
                    self.back();
                    return;
                }
                if dir != 0.0 {
                    *value = step_value(def, value, dir * step);
                    let (d, v, l) = (def.clone(), value.clone(), *live);
                    self.ui = ui;
                    if l {
                        self.def_set(cx, &d, v);
                    }
                    return;
                }
                self.ui = ui;
            }
            Ui::List { entry, i, .. } => {
                let items = self.sub_items(entry);
                if id == "menu" {
                    self.ui = ui;
                    return self.back();
                }
                let n = items.len() as i64;
                if dir != 0.0 {
                    if n > 0 {
                        *i = (*i + dir as i64 + n).rem_euclid(n);
                    }
                    let (e, k) = (entry.clone(), *i);
                    self.ui = ui;
                    if let Some(name) = items.get(k as usize)
                        && e == "STATIC COLOR"
                        && color_names().contains(&name.as_str())
                    {
                        self.set_color0(json!({ "kind": "named", "name": name }));
                    }
                    return;
                }
                let (e, k) = (entry.clone(), *i);
                self.ui = ui;
                if id == "enter" {
                    let name = items.get(k as usize).cloned().unwrap_or_default();
                    self.open_sub_item(&e, &name);
                }
            }
            Ui::Pin { key, digits, pos, .. } => {
                if id == "menu" {
                    *pos = (*pos + 1) % digits.len().max(1);
                    self.ui = ui;
                    return;
                }
                if id == "enter" {
                    let k = if key.is_empty() { "radioPin".to_string() } else { key.clone() };
                    let text: String = digits.iter().map(|d| d.to_string()).collect();
                    self.ui = ui;
                    self.fx.set1(&k, json!(text));
                    return self.back();
                }
                if dir != 0.0
                    && let Some(d) = digits.get_mut(*pos)
                {
                    *d = (*d + dir as i64 + 10).rem_euclid(10);
                }
                self.ui = ui;
            }
            Ui::Confirm { yes, .. } => {
                let choice = js::truthy(self.cfg.get("resetChoice"));
                if choice && dir != 0.0 {
                    *yes = !*yes; // firmware 2018-2020 : choix NO / YES avec + / −
                    self.ui = ui;
                    return;
                }
                let y = *yes;
                self.ui = ui;
                if id == "enter" && choice && !y {
                    self.ui = Ui::Ring { i: self.ring_index("RESET SETTINGS") };
                    return;
                }
                if id == "enter" {
                    let reset = reset_settings(self.ty, &self.fx.settings);
                    self.fx.set(reset);
                    cx.astera.unlatch(cx.runtime);
                    cx.astera.just_unlinked = false;
                    self.ui = Ui::Status;
                } else if id == "menu" {
                    self.ui = Ui::Ring { i: self.ring_index("RESET SETTINGS") };
                }
            }
            Ui::Hsi { line, color } => {
                if id == "menu" {
                    self.ui = Ui::Status;
                    return;
                }
                if id == "enter" {
                    *line = (*line + 1) % 4;
                    self.ui = ui;
                    return;
                }
                if dir != 0.0 {
                    let mut c = *color;
                    match *line {
                        0 => c.cct = js::clamp(c.cct + dir * 50.0 * step, 1750.0, 20000.0),
                        1 => c.green = js::clamp(js::round((c.green + dir * 0.05 * step) * 100.0) / 100.0, -1.0, 1.0),
                        2 => c.hue = ((c.hue + dir * step) % 361.0 + 361.0) % 361.0,
                        _ => c.sat = js::clamp(c.sat + dir * step, 0.0, 100.0),
                    }
                    *color = c;
                    self.ui = ui;
                    self.set_color0(hsi_spec(c));
                    return;
                }
                self.ui = ui;
            }
            Ui::Static { i } => {
                if id == "menu" {
                    self.ui = Ui::Status;
                    return;
                }
                if dir != 0.0 {
                    let names = static_names();
                    let n = names.len() as i64;
                    *i = (*i + dir as i64 + n).rem_euclid(n);
                    let name = names[*i as usize];
                    self.ui = ui;
                    self.set_color0(json!({ "kind": "named", "name": name }));
                    return;
                }
                self.ui = ui;
            }
            Ui::Gel { line, family, gel } => {
                if id == "menu" {
                    self.ui = Ui::Status;
                    return;
                }
                if id == "enter" {
                    *line = if *line != 0 { 0 } else { 1 };
                    self.ui = ui;
                    return;
                }
                if dir != 0.0 {
                    let gels = lee_gels();
                    let n = gels.len() as f64;
                    if *line == 0 {
                        *family = (*family + dir as i64 + GEL_FAMILIES.len() as i64).rem_euclid(GEL_FAMILIES.len() as i64);
                    } else {
                        *gel = ((*gel as f64 + dir * step + n * 10.0) % n) as i64;
                    }
                    let spec = json!({ "kind": "gel", "family": GEL_FAMILIES[*family as usize], "number": jv(gels[*gel as usize].number) });
                    self.ui = ui;
                    self.set_color0(spec);
                    return;
                }
                self.ui = ui;
            }
            Ui::Bright { line } => {
                if id == "menu" {
                    self.ui = Ui::Status;
                    return;
                }
                if id == "enter" {
                    *line = if *line != 0 { 0 } else { 1 };
                    self.ui = ui;
                    return;
                }
                let l = *line;
                self.ui = ui;
                if dir != 0.0 {
                    if l == 0 {
                        let b = js::clamp(js::nn(self.fx.s("brightness"), 100.0) + dir * step, 0.0, 100.0);
                        self.fx.set1("brightness", jv(b));
                    } else {
                        let r = js::clamp(js::nn(self.fx.s("runtime"), 5.0) + dir, 1.0, 20.0);
                        self.fx.set1("runtime", jv(r));
                    }
                }
            }
            Ui::Tilt => self.ui = Ui::Status,
        }
    }

    /// Maintien 2 s sur l'écran d'état (clavier à double légende) : COLOR, SPEED, BRIGHT, PROG.
    fn hold_action(&mut self, cx: &mut Ctx, id: &str) {
        self.last_key = self.last_t;
        self.latch_standalone(cx);
        let open = |me: &mut Self, name: &str| {
            if let Some(def) = me.item(name) {
                let value = me.def_get(&def);
                me.ui = Ui::SubValue { name: name.into(), def, value, parent: Box::new(Ui::Status), live: true };
            }
        };
        match id {
            "menu" => self.ui = Ui::List { entry: "STANDALONE".into(), i: 0, parent: Box::new(Ui::Status) },
            "minus" => open(self, "SPEED"),
            "plus" => open(self, "BRIGHTNESS"),
            "enter" => open(self, "PROGRAM"),
            _ => {}
        }
    }

    /// TouchSlider (HydraPanel) : actif seulement sur les pages soleil / palette, DMX ADDRESS et FIXTURE NUMBER.
    fn slider_active(&self) -> bool {
        match &self.ui {
            Ui::Bright { .. } | Ui::Hsi { .. } | Ui::Static { .. } | Ui::Gel { .. } => true,
            Ui::Value { entry, .. } => entry == "DMX ADDRESS",
            Ui::SubValue { name, .. } => name == "FIXTURE NUMBER",
            _ => false,
        }
    }

    /// `steps` > 1 pour un glissé.
    pub fn slider(&mut self, cx: &mut Ctx, zone: SliderZone, steps: u32) -> bool {
        let t = cx.env.time;
        self.last_t = t;
        self.last_key = t;
        if !self.fx.power() || !self.slider_active() {
            return false;
        }
        match zone {
            SliderZone::Middle => self.key(cx, "enter", 1.0),
            SliderZone::Top => (0..steps).for_each(|_| self.key(cx, "plus", 1.0)),
            SliderZone::Bottom => (0..steps).for_each(|_| self.key(cx, "minus", 1.0)),
        }
        true
    }

    fn back(&mut self) {
        self.ui = self.ui.parent().cloned().unwrap_or(Ui::Status);
    }

    fn open_entry(&mut self, cx: &mut Ctx, entry: &str) {
        let parent = Box::new(self.ui.clone());
        match entry {
            "UNPAIR CRMX" | "UNLINK CRMX" => {
                self.show(&[if entry == "UNPAIR CRMX" { "UNPAIRING..." } else { "UNLINKING..." }, ""], 1.2, Some(Then::Unlink));
            }
            "UNBOND BLUETOOTH" => self.show(&["UNBONDING...", ""], 1.2, Some(Then::Unbond)),
            "RESET SETTINGS" => self.ui = Ui::Confirm { parent, yes: false },
            "STATIC COLOR" => {
                self.latch_standalone(cx);
                self.set_color0(json!({ "kind": "named", "name": "RED" }));
                self.ui = Ui::List { entry: entry.into(), i: 0, parent };
            }
            "DMX SETTINGS" | "STANDALONE" | "INFO" => self.ui = Ui::List { entry: entry.into(), i: 0, parent },
            _ => {
                if let Some(def) = self.item(entry) {
                    let value = self.def_get(&def);
                    self.ui = Ui::Value { entry: entry.into(), def, value, parent, live: false };
                }
            }
        }
    }

    fn open_sub_item(&mut self, entry: &str, name: &str) {
        let parent = Box::new(self.ui.clone());
        let digits = |s: &str| s.chars().map(|c| c.to_digit(10).map_or(-1, i64::from)).collect::<Vec<_>>();
        if name == "CUSTOM COLOR" {
            self.ui = Ui::List { entry: "CUSTOM COLOR".into(), i: 0, parent };
            return;
        }
        if name == "Radio PIN" {
            let d = digits(js::text_or(self.fx.s("radioPin"), "0000"));
            self.ui = Ui::Pin { key: "radioPin".into(), title: "Radio PIN:".into(), digits: d, pos: 0, parent };
            return;
        }
        if name == "CRMX LINK BY KEY" {
            // clé de liaison CRMX à 8 chiffres (même édition chiffre par chiffre que le Radio PIN)
            let d = digits(js::text_or(self.fx.s("linkKey"), "00000000"));
            self.ui = Ui::Pin { key: "linkKey".into(), title: "Link Key:".into(), digits: d, pos: 0, parent };
            return;
        }
        if entry == "STATIC COLOR" && color_names().contains(&name) {
            self.back();
            return;
        }
        let Some(def) = self.item(name) else { return }; // information en lecture seule
        let live = entry == "STATIC COLOR" || entry == "CUSTOM COLOR";
        let value = self.def_get(&def);
        self.ui = Ui::SubValue { name: name.into(), def, value, parent, live };
    }

    fn open_color_page(&mut self, page: &str) {
        let c0 = self.sa().and_then(|s| s.get("colors")).and_then(|c| c.get(0)).cloned().unwrap_or(Value::Null);
        let kind = c0.get("kind").and_then(Value::as_str);
        let num = |k: &str| js::number(c0.get(k));
        match page {
            "hsi" => {
                let color = if kind == Some("hsi") { Hsi { cct: num("cct"), green: num("green"), hue: num("hue"), sat: num("sat") } } else { Hsi { cct: 5600.0, green: 0.0, hue: 0.0, sat: 0.0 } };
                self.ui = Ui::Hsi { line: 0, color };
                self.set_color0(hsi_spec(color));
            }
            "static" => {
                let names = static_names();
                let want = if kind == Some("named") { c0.get("name").and_then(Value::as_str).unwrap_or("") } else { "RED" };
                let i = names.iter().position(|n| *n == want).unwrap_or(0);
                self.ui = Ui::Static { i: i as i64 };
                self.set_color0(json!({ "kind": "named", "name": names[i] }));
            }
            _ => {
                let gels = lee_gels();
                let (gel, family) = if kind == Some("gel") {
                    let number = c0.get("number");
                    let g = gels.iter().position(|g| js::strict_eq(Some(&jv(g.number)), number)).unwrap_or(0);
                    let fam = c0.get("family").and_then(Value::as_str);
                    (g, GEL_FAMILIES.iter().position(|f| Some(*f) == fam).unwrap_or(0))
                } else {
                    (0, 0)
                };
                self.ui = Ui::Gel { line: 1, family: family as i64, gel: gel as i64 };
                self.set_color0(json!({ "kind": "gel", "family": GEL_FAMILIES[family], "number": jv(gels[gel].number) }));
            }
        }
    }

    // ---------------------------------------------------------------- affichage
    fn status_line(&self, cx: &Ctx) -> String {
        let rt = &cx.runtime;
        let crmx_word = self.cfg_str("crmxWord").unwrap_or("LINKED");
        if js::truthy(self.fx.st("acceptConfig")) {
            return self.cfg_str("acceptText").unwrap_or("ACCEPT CONFIG ?").into();
        }
        if js::truthy(self.fx.st("standby")) {
            return "STANDBY".into();
        }
        let linked = js::truthy(self.fx.s("crmxLinked"));
        match rt.input {
            Some(Input::Crmx) => {
                if !linked {
                    format!("CRMX: NOT {crmx_word}")
                } else if rt.signal {
                    "CRMX: SIGNAL 99%".into()
                } else {
                    "CRMX: NO DMX".into()
                }
            }
            Some(Input::Wired) => (if rt.signal { "XLR-DMX: LINK OK" } else { "XLR-DMX: NO DMX" }).into(),
            Some(Input::App) => "APP CONTROL".into(),
            Some(Input::Art3) => "ART3: NO LINK".into(),
            Some(Input::Standalone) => (if cx.astera.remote && rt.latch == Some(Input::Standalone) { "REMOTE CONTROL" } else { "STANDALONE" }).into(),
            _ => {
                if rt.src == Some(Source::Emergency) {
                    "EMERGENCY LIGHT".into()
                } else if linked {
                    "CRMX: NO DMX".into()
                } else if cx.astera.just_unlinked {
                    format!("CRMX: NOT {crmx_word}")
                } else {
                    "INPUT: DETECTING".into()
                }
            }
        }
    }

    /// Écran : lignes de texte et icônes.
    pub fn display(&self, cx: &Ctx) -> Screen {
        let t = cx.env.time;
        let battery = js::round(js::nn(self.fx.st("battery"), 100.0));
        let charging = js::truthy(self.fx.st("charging"));
        let icons = Icons {
            battery,
            plug: charging,
            bt: js::truthy(self.ty.extra.get("btb")) && self.fx.s("btbBonded") != Some(&Value::Bool(false)),
            // la version JavaScript lit `rt.appConnected`, jamais renseigné : ici l'appli connectée
            bt_waves: cx.env.app_connected,
            bars: self.cfg.get("display").and_then(|d| d.get("kind")).and_then(Value::as_str) == Some("lcd"),
        };
        if !self.fx.power() {
            if !charging {
                return Screen::Text(TextScreen { off: true, ..TextScreen::default() });
            }
            let b = battery;
            let bar = format!("[{}]{}%", js::pad_end(&"=".repeat(js::round(b / 10.0).max(0.0) as usize), 10, ' '), js::pad_start(&js::num_str(b), 3, ' '));
            return Screen::Text(TextScreen { off: false, dim: false, icons: None, big: true, lines: vec![TextLine::new(if b >= 100.0 { "Fully Charged" } else { "Charging..." }), TextLine::new(bar)] });
        }
        let dim = t - self.last_key > IDLE_DIM;
        if let Some(m) = &self.message {
            return Screen::Text(TextScreen { off: false, dim: false, icons: Some(icons), big: true, lines: m.lines.iter().map(TextLine::new).collect() });
        }
        let big = |a: String, b: String, inv2: bool| Screen::Text(TextScreen { off: false, dim, icons: Some(icons.clone()), big: true, lines: vec![TextLine::new(a), TextLine::inv(b, inv2)] });
        let small = |lines: Vec<TextLine>| Screen::Text(TextScreen { off: false, dim, icons: None, big: false, lines });
        match &self.ui {
            Ui::Ring { i } => big("Main menu:".into(), self.ring().get(*i as usize).cloned().unwrap_or_default(), false),
            Ui::Value { entry, def, value, .. } => big(format!("{entry}:"), self.def_fmt(def, value), true),
            Ui::SubValue { name, def, value, .. } => big(format!("{name}:"), self.def_fmt(def, value), true),
            Ui::List { entry, i, .. } => {
                let name = self.sub_items(entry).get(*i as usize).cloned().unwrap_or_default();
                if entry == "INFO" {
                    let v = match self.item(&name) {
                        Some(def) => self.def_fmt(&def, &self.def_get(&def)),
                        None => self.info_value(&name),
                    };
                    return big(format!("{name}:"), v, false);
                }
                if entry == "STATIC COLOR" {
                    return big("Static Color:".into(), name, false);
                }
                // Format vu sur l'HydraPanel : « DMX Settings: » / « FIXTURE NUMBER »
                let title = match entry.as_str() {
                    "DMX SETTINGS" => "DMX Settings:".to_string(),
                    "STANDALONE" => "Standalone:".to_string(),
                    "CUSTOM COLOR" => "Custom Color:".to_string(),
                    _ => format!("{entry}:"),
                };
                let label = self.cfg.get("itemLabels").and_then(|l| l.get(&name)).and_then(Value::as_str).map_or(name.clone(), str::to_string);
                big(title, label, false)
            }
            Ui::Pin { title, digits, pos, .. } => {
                let blink = (t * 2.0) % 1.0 < 0.5;
                let s: String = digits.iter().enumerate().map(|(i, d)| if i == *pos && blink { "_".to_string() } else { d.to_string() }).collect();
                big(if title.is_empty() { "Radio PIN:".into() } else { title.clone() }, s, false)
            }
            Ui::Confirm { yes, .. } => {
                if js::truthy(self.cfg.get("resetChoice")) {
                    return big("ARE YOU SURE?".into(), (if *yes { " NO  [YES]" } else { "[NO]  YES " }).into(), false);
                }
                let c = self.cfg_list("resetConfirm");
                let a = c.as_ref().and_then(|c| c.first().cloned()).unwrap_or_else(|| "ARE YOU SURE ?".into());
                let b = c.as_ref().and_then(|c| c.get(1).cloned()).unwrap_or_else(|| if c.is_some() { String::new() } else { "ENTER FOR YES".into() });
                big(a, b, false)
            }
            Ui::Hsi { line, color } => {
                let rows = [
                    format!("CCT    : {} K", js::num_str(color.cct)),
                    format!("Green  : {}", js::to_fixed(color.green, 2)),
                    format!("Hue    : {}", js::num_str(color.hue)),
                    format!("Sat    : {}%", js::num_str(color.sat)),
                ];
                small(rows.into_iter().enumerate().map(|(i, s)| TextLine::inv(s, i as i64 == *line)).collect())
            }
            Ui::Static { i } => big("Static Color:".into(), title_case(static_names().get(*i as usize).copied().unwrap_or("")), false),
            Ui::Gel { line, family, gel } => {
                let g = &lee_gels()[*gel as usize];
                small(vec![
                    TextLine::new("Color Gel:"),
                    TextLine::inv(GEL_FAMILIES[*family as usize], *line == 0),
                    TextLine::inv(js::pad_start(&js::num_str(g.number), 3, '0'), *line == 1),
                    TextLine::new(g.name.clone()),
                ])
            }
            Ui::Tilt => {
                // inclinaison de l'axe du faisceau par rapport à l'horizontale (format d'écran non publié)
                let [rx, ry, _] = self.fx.rotation;
                let tilt = js::clamp(-ry.cos() * rx.sin(), -1.0, 1.0).asin() * 180.0 / std::f64::consts::PI;
                big("TILT".into(), format!("{}{}°", if tilt >= 0.0 { "+" } else { "" }, js::to_fixed(tilt, 1)), false)
            }
            Ui::Bright { line } => small(vec![
                TextLine::inv(format!("Brigntness: {}%", js::num_str(js::nn(self.fx.s("brightness"), 100.0))), *line == 0),
                TextLine::inv(format!("Runtime:   {}h", js::num_str(js::nn(self.fx.s("runtime"), 5.0))), *line == 1),
            ]),
            Ui::Status => {
                let set = self.fx.s("setAddress");
                let set_txt = if js::truthy(set) { pad3(set.unwrap_or(&Value::Null)) } else { "---".into() };
                let dmx = pad3(self.fx.s("dmxAddress").unwrap_or(&Value::Null));
                big(self.status_line(cx), format!("SET:{set_txt}  DMX:{dmx}"), false)
            }
        }
    }

    /// Voyant d'état de la batterie (claviers à double légende) : bleu sur secteur, sinon vert / orange / rouge.
    pub fn leds(&self) -> Leds {
        let mut out = Leds::new();
        if !js::truthy(self.cfg.get("batteryLed")) {
            return out;
        }
        if js::truthy(self.fx.st("charging")) {
            out.insert("battery".into(), Led::on("#2f7bff"));
            return out;
        }
        if !self.fx.power() {
            return out;
        }
        let b = js::nn(self.fx.st("battery"), 100.0);
        out.insert("battery".into(), Led::on(if b < 15.0 { "#ff4d4d" } else if b < 90.0 { "#ff9a2e" } else { "#3ddc84" }));
        out
    }

    pub fn hint(&self) -> String {
        if !self.fx.power() {
            return (if js::truthy(self.fx.st("charging")) { "Éteint, en charge (l’écran affiche la charge)." } else { "Éteint : maintenir ⏻ 1 s pour allumer." }).into();
        }
        if js::truthy(self.fx.st("blueMode")) {
            return "BlueMode : le tube clignote en bleu, prêt à recevoir le Radio PIN (« Pair with Lights » dans l’appli).".into();
        }
        String::new()
    }

    /// Écran affiché (« status », « ring », « list »…), pour l'interface et les essais.
    pub fn screen_name(&self) -> &'static str {
        self.ui.screen()
    }

    pub(crate) fn refresh(&mut self, cx: &Ctx) {
        self.fx = Fx::of(cx.fixture);
    }

    pub(crate) fn take(&mut self) -> crate::fixture::Outcome {
        self.fx.take()
    }
}

fn static_names() -> Vec<&'static str> {
    color_names().into_iter().filter(|n| *n != "BLACK").collect()
}

fn hsi_spec(c: Hsi) -> Value {
    json!({ "kind": "hsi", "cct": jv(c.cct), "green": jv(c.green), "hue": jv(c.hue), "sat": jv(c.sat) })
}

fn step_value(def: &Def, value: &Value, delta: f64) -> Value {
    match &def.kind {
        DefKind::Num { min, max } => {
            let mut v = js::number(Some(value)) + delta;
            if v > *max {
                v = if delta.abs() > 1.0 { *max } else { *min };
            }
            if v < *min {
                v = if delta.abs() > 1.0 { *min } else { *max };
            }
            jv(v)
        }
        DefKind::Opt(opts) => {
            if opts.is_empty() {
                return Value::Null;
            }
            let i = opts.iter().position(|o| js::strict_eq(Some(o), Some(value))).map_or(0, |i| i as i64);
            let d = delta.signum() as i64;
            let n = opts.len() as i64;
            opts[((i + d + n) % n) as usize].clone()
        }
    }
}
