//! Décodage DMX générique piloté par les définitions de modes des projecteurs
//! (portage de `shared/engine/dmx.js`).
//!
//! Un mode décrit ses canaux dans l'ordre :
//! - `"red"` : raccourci pour `{ f: "red" }` ;
//! - `{ f: "dimmer", bytes: 2 }` : 16 bits (canal gros + canal fin) ;
//! - `{ f: "cct", range: [1750, 20000] }` : plage linéaire de la fonction ;
//! - `{ f: "cct", map: "asteraCct" }` : conversion spécifique (voir [`MapKind`]) ;
//! - `{ f: "red", px: 3 }` : canal d'un pixel précis (modes déjà « dépliés ») ;
//! - `{ pixels: 16, channels: [...] }` : bloc répété pour chaque pixel ;
//! - `{ f: "none" }` : canal réservé / sans effet.

use super::effects::{Program, direction_from_dmx, program_from_dmx, render_program};
use super::gels::IndexColors;
use super::js_round;
use crate::color::{Rgb, cct_to_rgb, emitter, hs_to_rgb, xy_to_rgb};
use crate::library::DmxMode;
use serde_json::Value;

/// Fonction d'un canal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Func {
    // émetteurs, dans l'ordre de EMITTER_FUNCTIONS (l'ordre du mélange compte)
    Red,
    Green,
    Blue,
    White,
    Amber,
    Mint,
    Lime,
    Cyan,
    Uv,
    Ww,
    Cw,
    Magenta,
    Yellow,
    Indigo,
    Dimmer,
    Strobe,
    Cct,
    Tint,
    Crossfade,
    Hue,
    Sat,
    X,
    Y,
    Index,
    CctIndex,
    FxProgram,
    FxSpeed,
    FxFade,
    FxDirection,
    FxSize,
    FxOffset,
    FxRestart,
    FxIndex,
    FxR,
    FxG,
    FxB,
    Fan,
    /// canal réservé, sans effet
    None,
    /// fonction que cette version ne connaît pas (sans effet sur la lumière)
    Other,
}

const FUNCS: [(Func, &str); 39] = [
    (Func::Red, "red"),
    (Func::Green, "green"),
    (Func::Blue, "blue"),
    (Func::White, "white"),
    (Func::Amber, "amber"),
    (Func::Mint, "mint"),
    (Func::Lime, "lime"),
    (Func::Cyan, "cyan"),
    (Func::Uv, "uv"),
    (Func::Ww, "ww"),
    (Func::Cw, "cw"),
    (Func::Magenta, "magenta"),
    (Func::Yellow, "yellow"),
    (Func::Indigo, "indigo"),
    (Func::Dimmer, "dimmer"),
    (Func::Strobe, "strobe"),
    (Func::Cct, "cct"),
    (Func::Tint, "tint"),
    (Func::Crossfade, "crossfade"),
    (Func::Hue, "hue"),
    (Func::Sat, "sat"),
    (Func::X, "x"),
    (Func::Y, "y"),
    (Func::Index, "index"),
    (Func::CctIndex, "cctIndex"),
    (Func::FxProgram, "fxProgram"),
    (Func::FxSpeed, "fxSpeed"),
    (Func::FxFade, "fxFade"),
    (Func::FxDirection, "fxDirection"),
    (Func::FxSize, "fxSize"),
    (Func::FxOffset, "fxOffset"),
    (Func::FxRestart, "fxRestart"),
    (Func::FxIndex, "fxIndex"),
    (Func::FxR, "fxR"),
    (Func::FxG, "fxG"),
    (Func::FxB, "fxB"),
    (Func::Fan, "fan"),
    (Func::None, "none"),
    (Func::Other, "?"),
];

const FUNC_COUNT: usize = FUNCS.len();

/// Émetteurs, dans l'ordre du mélange.
pub const EMITTER_FUNCTIONS: [Func; 14] = [
    Func::Red,
    Func::Green,
    Func::Blue,
    Func::White,
    Func::Amber,
    Func::Mint,
    Func::Lime,
    Func::Cyan,
    Func::Uv,
    Func::Ww,
    Func::Cw,
    Func::Magenta,
    Func::Yellow,
    Func::Indigo,
];

impl Func {
    pub fn parse(name: &str) -> Func {
        if name.is_empty() {
            return Func::None;
        }
        FUNCS.iter().find(|(_, n)| *n == name).map_or(Func::Other, |(f, _)| *f)
    }

    pub fn name(self) -> &'static str {
        FUNCS[self as usize].1
    }

    pub fn is_emitter(self) -> bool {
        (self as usize) <= Func::Indigo as usize
    }

    /// Couleur de l'émetteur (le blanc « W » est celui du projecteur).
    pub fn emitter_rgb(self, white: Rgb) -> Rgb {
        if self == Func::White { white } else { emitter(self.name()).unwrap_or([0.0; 3]) }
    }
}

/// Fonction et numéro du canal (0 sinon) : la clé des attributs décodés (« dimmer », « fxR1 »
/// pour les couleurs des modes d'effet, « cct2 » pour un second canal Kelvin…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    pub func: Func,
    pub slot: u8,
}

/// Numéros de canal possibles par fonction (0 : aucun, puis 1..4).
const SLOTS: usize = 5;

impl Key {
    pub const fn of(func: Func) -> Self {
        Self { func, slot: 0 }
    }

    fn index(self) -> Option<usize> {
        match self.func {
            Func::None | Func::Other => None,
            f if (self.slot as usize) < SLOTS => Some(f as usize * SLOTS + self.slot as usize),
            _ => None,
        }
    }

    /// Nom de la clé, comme en JavaScript (« fxR1 », « dimmer »).
    pub fn name(self) -> String {
        if self.slot == 0 { self.func.name().to_owned() } else { format!("{}{}", self.func.name(), self.slot) }
    }
}

const ATTR_COUNT: usize = FUNC_COUNT * SLOTS;

/// Attributs décodés d'un projecteur ou d'un pixel (valeur de chaque fonction présente ; NaN : absente).
#[derive(Debug, Clone, Copy)]
pub struct Attrs([f64; ATTR_COUNT]);

impl Default for Attrs {
    fn default() -> Self {
        Self([f64::NAN; ATTR_COUNT])
    }
}

impl Attrs {
    pub fn get(&self, f: Func) -> Option<f64> {
        self.key(Key::of(f))
    }

    pub fn key(&self, k: Key) -> Option<f64> {
        k.index().map(|i| self.0[i]).filter(|v| !v.is_nan())
    }

    pub fn set(&mut self, k: Key, v: f64) {
        if let Some(i) = k.index() {
            self.0[i] = v;
        }
    }

    /// Les attributs de `over` remplacent ceux-ci (`{ ...this, ...over }`).
    pub fn merged(&self, over: &Attrs) -> Attrs {
        let mut m = *self;
        for (a, b) in m.0.iter_mut().zip(over.0.iter()) {
            if !b.is_nan() {
                *a = *b;
            }
        }
        m
    }

    /// Attributs présents (clé, valeur).
    pub fn iter(&self) -> impl Iterator<Item = (Key, f64)> + '_ {
        self.0.iter().enumerate().filter(|(_, v)| !v.is_nan()).map(|(i, v)| (Key { func: FUNCS[i / SLOTS].0, slot: (i % SLOTS) as u8 }, *v))
    }
}

/// Conversions non linéaires (tables des documents « DMX Profiles » Astera).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapKind {
    AsteraStrobe,
    AsteraCct,
    AsteraCctInd,
    AsteraGm,
    AsteraIdx,
    /// canal « Fan Control » des profils FAN (QuikPunch, QuikBeam, Pluto, Leo)
    AsteraFan,
    Raw,
}

/// Réglages du ventilateur (canal « Fan Control ») ; l'attribut vaut l'indice dans cette liste.
pub const FAN_MODES: [&str; 5] = ["AUTO", "LOW", "MED", "HIGH", "OFF"];

impl MapKind {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "asteraStrobe" => Self::AsteraStrobe,
            "asteraCct" => Self::AsteraCct,
            "asteraCctInd" => Self::AsteraCctInd,
            "asteraGM" => Self::AsteraGm,
            "asteraIdx" => Self::AsteraIdx,
            "asteraFan" => Self::AsteraFan,
            "raw" => Self::Raw,
            _ => return None,
        })
    }

    /// Valeur brute (0..255, ou 16 bits ramenés à 8 bits) → valeur de l'attribut ; None : sans effet.
    pub fn apply(self, v: f64) -> Option<f64> {
        Some(match self {
            Self::AsteraStrobe => {
                if v <= 3.0 {
                    0.0
                } else if v == 4.0 {
                    -1.0
                } else if v == 5.0 {
                    -2.0
                } else if v == 6.0 {
                    -3.0
                } else {
                    0.4 + ((v - 7.0) / 248.0) * 24.6
                }
            }
            Self::AsteraCct => 1750.0 + 32.0 * v,
            Self::AsteraCctInd => {
                if v <= 4.0 {
                    return None;
                }
                2000.0 + 20.0 * v
            }
            Self::AsteraGm => {
                if v <= 4.0 {
                    0.0
                } else {
                    v / 128.0 - 1.0
                }
            }
            Self::AsteraIdx => {
                if v <= 1.0 {
                    return None;
                }
                v
            }
            Self::AsteraFan => match v {
                v if v <= 4.0 => 0.0,
                v if v <= 35.0 => 1.0,
                v if v <= 70.0 => 2.0,
                v if v <= 100.0 => 3.0,
                v if v <= 128.0 => 4.0,
                _ => return None,
            },
            Self::Raw => v,
        })
    }
}

/// Stroboscope générique : 0..`open` ouvert, puis de `min` à `max` Hz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrobeDef {
    pub open: f64,
    pub min: f64,
    pub max: f64,
}

impl Default for StrobeDef {
    fn default() -> Self {
        Self { open: 9.0, min: 1.0, max: 25.0 }
    }
}

/// Description d'un canal.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelDef {
    pub func: Func,
    /// couleur 1..4 des modes d'effet (« fxR1 »…), 0 sinon
    pub slot: u8,
    /// 1, ou 2 (16 bits)
    pub bytes: u8,
    pub px: Option<i32>,
    pub map: Option<MapKind>,
    pub range: Option<[f64; 2]>,
    /// zone neutre en bas de course (ex. ARRI : 0–10 = pas de correction)
    pub neutral: Option<f64>,
    pub neutral_value: Option<f64>,
    pub strobe: Option<StrobeDef>,
    pub label: Option<String>,
}

impl ChannelDef {
    fn parse(v: &Value) -> ChannelDef {
        let mut d = ChannelDef { func: Func::None, slot: 0, bytes: 1, px: None, map: None, range: None, neutral: None, neutral_value: None, strobe: None, label: None };
        match v {
            Value::String(s) => d.func = Func::parse(s),
            Value::Object(o) => {
                let n = |k: &str| o.get(k).and_then(Value::as_f64);
                d.func = o.get("f").and_then(Value::as_str).map_or(Func::None, Func::parse);
                // numéro du canal : nombre, ou texte (« 2 ») dans certains profils importés
                let slot = o.get("slot").and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())));
                d.slot = slot.filter(|s| *s > 0.0).map_or(0, |s| s as u8);
                d.bytes = if n("bytes") == Some(2.0) { 2 } else { 1 };
                d.px = n("px").map(|p| p as i32);
                d.map = o.get("map").and_then(Value::as_str).and_then(MapKind::parse);
                d.range = o.get("range").and_then(Value::as_array).and_then(|r| Some([r.first()?.as_f64()?, r.get(1)?.as_f64()?]));
                d.neutral = n("neutral");
                d.neutral_value = n("neutralValue");
                d.strobe = o.get("strobe").and_then(Value::as_object).map(|s| {
                    let base = StrobeDef::default();
                    let g = |k: &str, v: f64| s.get(k).and_then(Value::as_f64).unwrap_or(v);
                    StrobeDef { open: g("open", base.open), min: g("min", base.min), max: g("max", base.max) }
                });
                d.label = o.get("label").and_then(Value::as_str).map(str::to_owned);
            }
            _ => {}
        }
        d
    }
}

/// Plages par défaut des fonctions.
pub fn default_range(f: Func) -> Option<[f64; 2]> {
    match f {
        Func::Cct => Some([2000.0, 10000.0]),
        Func::Hue => Some([0.0, 360.0]),
        Func::Tint => Some([-1.0, 1.0]),
        _ => None,
    }
}

/// Un canal à plat : fonction, pixel (-1 : tout le projecteur), position dans le mode.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub def: ChannelDef,
    pub px: i32,
    pub offset: usize,
}

impl Slot {
    pub fn func(&self) -> Func {
        self.def.func
    }

    pub fn bytes(&self) -> u8 {
        self.def.bytes
    }

    pub fn key(&self) -> Key {
        Key { func: self.def.func, slot: self.def.slot }
    }

    /// Valeur brute maximale (255, ou 65535 en 16 bits).
    pub fn max(&self) -> u16 {
        if self.def.bytes == 2 { 65535 } else { 255 }
    }

    /// Canal actif (pas réservé).
    pub fn active(&self) -> bool {
        !matches!(self.def.func, Func::None)
    }
}

/// Un mode aplati : la liste de ses canaux.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompiledMode {
    pub slots: Vec<Slot>,
    pub footprint: usize,
}

impl CompiledMode {
    pub fn compile(channels: &[Value]) -> CompiledMode {
        fn push(ch: &Value, px: i32, c: &mut CompiledMode) {
            let def = ChannelDef::parse(ch);
            let bytes = def.bytes as usize;
            let px = def.px.unwrap_or(px);
            c.slots.push(Slot { def, px, offset: c.footprint });
            c.footprint += bytes;
        }
        let mut c = CompiledMode::default();
        for ch in channels {
            let block = ch.as_object().and_then(|o| o.get("pixels")).and_then(Value::as_f64).filter(|n| *n != 0.0);
            match block {
                Some(n) => {
                    let first = ch.get("firstPixel").and_then(Value::as_f64).unwrap_or(0.0) as i32;
                    let inner = ch.get("channels").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
                    for p in 0..n as i32 {
                        for c2 in inner {
                            push(c2, first + p, &mut c);
                        }
                    }
                }
                None => push(ch, -1, &mut c),
            }
        }
        c
    }

    pub fn has(&self, f: Func) -> bool {
        self.slots.iter().any(|s| s.def.func == f)
    }
}

/// Octet `i` d'une trame (hors trame : 0).
fn byte(data: &[u8], i: i64) -> f64 {
    if i < 0 { 0.0 } else { data.get(i as usize).copied().unwrap_or(0) as f64 }
}

/// Valeur brute d'un canal (0..255, ou 0..65535 en 16 bits).
pub fn raw_value(data: &[u8], start: i64, slot: &Slot) -> f64 {
    let i = start + slot.offset as i64;
    if slot.def.bytes == 2 { byte(data, i) * 256.0 + byte(data, i + 1) } else { byte(data, i) }
}

fn slot_value(data: &[u8], start: i64, slot: &Slot) -> Option<f64> {
    let raw = raw_value(data, start, slot);
    let def = &slot.def;
    if let Some(map) = def.map {
        // les tables Astera sont exprimées en 8 bits
        return map.apply(if def.bytes == 2 { raw / 257.0 } else { raw });
    }
    let n = if def.bytes == 2 { raw / 65535.0 } else { raw / 255.0 };
    if def.func == Func::Strobe {
        return Some(decode_strobe(def, n));
    }
    let range = def.range.or(default_range(def.func));
    if let Some(neutral) = def.neutral {
        // zone neutre en bas de course, puis toute la plage au-dessus ; seuil arrondi dans la
        // bibliothèque (10/255 noté 0,0392) : un demi-pas de tolérance
        if n <= neutral + 0.5 / 255.0 {
            return Some(def.neutral_value.unwrap_or(0.0));
        }
        let k = (n - neutral) / (1.0 - neutral);
        let r = range.unwrap_or([0.0, 1.0]);
        return Some(r[0] + k * (r[1] - r[0]));
    }
    Some(match range {
        Some(r) => r[0] + n * (r[1] - r[0]),
        None => n,
    })
}

/// Stroboscope générique : fréquence en Hz (0 = ouvert). Par défaut 0–9 ouvert, 10–255 de 1 à 25 Hz.
pub fn decode_strobe(def: &ChannelDef, n: f64) -> f64 {
    let s = def.strobe.unwrap_or_default();
    let v = js_round(n * 255.0);
    if v <= s.open {
        return 0.0;
    }
    let k = (v - s.open - 1.0) / (255.0 - s.open - 1.0);
    s.min + k * (s.max - s.min)
}

/// Ouverture du stroboscope à l'instant `time` : `hz` > 0 fréquence ; < 0 aléatoire (-1 rapide,
/// -2 moyen, -3 lent) ; 0 ouvert.
pub fn strobe_gate(hz: f64, time: f64, duty: f64, seed: f64) -> f64 {
    if hz == 0.0 || hz.is_nan() {
        return 1.0;
    }
    if hz < 0.0 {
        let rate = match hz {
            -1.0 => 9.0,
            -2.0 => 4.0,
            -3.0 => 1.5,
            _ => 4.0,
        };
        let slot = (time * rate * 2.0).floor();
        let r = ((slot + seed * 17.3) * 12.9898).sin() * 43758.5453;
        if r - r.floor() < 0.5 {
            return 0.0;
        }
        return if (time * rate * 2.0) % 1.0 < 0.35 { 1.0 } else { 0.0 };
    }
    if (time * hz) % 1.0 < duty { 1.0 } else { 0.0 }
}

/// Courbes de gradation (appliquées aux canaux d'intensité).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    Linear,
    Square,
    InvSquare,
    SCurve,
    // menu DIMMER CURVE des Astera (formes approchées)
    Fast,
    Standard,
    Halogen,
    Theater,
    Tv,
}

impl Curve {
    pub fn parse(name: &str) -> Option<Curve> {
        Some(match name {
            "linear" => Self::Linear,
            "square" => Self::Square,
            "invSquare" => Self::InvSquare,
            "sCurve" => Self::SCurve,
            "FAST" => Self::Fast,
            "STANDARD" => Self::Standard,
            "HALOGEN" => Self::Halogen,
            "THEATER" => Self::Theater,
            "TV" => Self::Tv,
            _ => return None,
        })
    }

    /// Courbe nommée (`DIMMING_CURVES[name] || default`).
    pub fn named(name: Option<&str>, default: Curve) -> Curve {
        name.and_then(Curve::parse).unwrap_or(default)
    }

    pub fn apply(self, n: f64) -> f64 {
        match self {
            Self::Linear | Self::Standard => n,
            Self::Square | Self::Halogen => n * n,
            Self::InvSquare | Self::Fast => 1.0 - (1.0 - n) * (1.0 - n),
            Self::SCurve | Self::Theater => n * n * (3.0 - 2.0 * n),
            Self::Tv => n.powf(1.5),
        }
    }
}

/// Décode les canaux DMX d'un projecteur : attributs du projecteur et de chaque pixel.
pub fn read_attributes(mode: &DmxMode, data: &[u8], address: i64, pixel_count: usize) -> (Attrs, Vec<Attrs>) {
    let start = address - 1;
    let mut fixture = Attrs::default();
    let mut pixels = vec![Attrs::default(); pixel_count];
    for slot in &mode.compiled().slots {
        if !slot.active() {
            continue;
        }
        if start + slot.offset as i64 >= 512 {
            break;
        }
        let Some(value) = slot_value(data, start, slot) else { continue };
        if slot.px < 0 {
            fixture.set(slot.key(), value);
        } else if (slot.px as usize) < pixel_count {
            pixels[slot.px as usize].set(slot.key(), value);
        }
    }
    (fixture, pixels)
}

fn blend(a: Rgb, b: Rgb, k: f64) -> Rgb {
    [a[0] * (1.0 - k) + b[0] * k, a[1] * (1.0 - k) + b[1] * k, a[2] * (1.0 - k) + b[2] * k]
}

/// Couleur d'un pixel d'après ses attributs (sRGB linéaire, avant gradateur).
/// Règles Astera : couleur index > Kelvin « IND » > RGB ; fondu entre Kelvin et RGB.
pub fn attributes_to_rgb(a: &Attrs, white_rgb: Rgb, index_colors: Option<&IndexColors>) -> Rgb {
    if let (Some(index), Some(ic)) = (a.get(Func::Index), index_colors)
        && let Some(c) = ic.color(index)
    {
        return c;
    }
    let tint = a.get(Func::Tint).unwrap_or(0.0);
    if let Some(k) = a.get(Func::CctIndex) {
        return cct_to_rgb(k, tint);
    }
    let mut rgb: Option<Rgb> = None;
    for f in EMITTER_FUNCTIONS {
        let Some(v) = a.get(f) else { continue };
        let e = f.emitter_rgb(white_rgb);
        let c = rgb.get_or_insert([0.0; 3]);
        c[0] += v * e[0];
        c[1] += v * e[1];
        c[2] += v * e[2];
    }
    if let Some(rgb) = rgb {
        if let (Some(cct), Some(k)) = (a.get(Func::Cct), a.get(Func::Crossfade)) {
            return blend(cct_to_rgb(cct, tint), rgb, k);
        }
        return rgb;
    }
    if let Some(hue) = a.get(Func::Hue) {
        let white = a.get(Func::Cct).map_or(white_rgb, |k| cct_to_rgb(k, tint));
        let hsi = hs_to_rgb(hue, a.get(Func::Sat).unwrap_or(1.0), white);
        // fondu Kelvin ↔ HSI (modes « CCT & HSI ») : 0 = blanc du canal Kelvin, 1 = couleur
        return match (a.get(Func::Crossfade), a.get(Func::Cct)) {
            (Some(k), Some(_)) => blend(white, hsi, k),
            _ => hsi,
        };
    }
    if let Some(cct) = a.get(Func::Cct) {
        return cct_to_rgb(cct, tint);
    }
    if let (Some(x), Some(y)) = (a.get(Func::X), a.get(Func::Y)) {
        return xy_to_rgb(x, y);
    }
    white_rgb
}

/// Couleurs de programme d'un mode d'effet (Idx1..4 ou R1/G1/B1..R4/G4/B4).
fn effect_colors(fx: &Attrs, index_colors: Option<&IndexColors>) -> [Rgb; 4] {
    let mut cols = [[0.0; 3]; 4];
    for (i, c) in cols.iter_mut().enumerate() {
        let slot = i as u8 + 1;
        let k = |func| fx.key(Key { func, slot });
        if let Some(v) = k(Func::FxIndex) {
            *c = index_colors.and_then(|ic| ic.color(v)).unwrap_or([0.0; 3]);
        } else if k(Func::FxR).is_some() {
            *c = [k(Func::FxR).unwrap_or(0.0), k(Func::FxG).unwrap_or(0.0), k(Func::FxB).unwrap_or(0.0)];
        }
    }
    cols
}

/// Départ du programme d'effet en cours (redémarré quand le programme ou le canal Restart change).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FxState {
    pub program: Option<usize>,
    pub restart: Option<f64>,
    pub start: Option<f64>,
}

/// Entrées du décodage d'un projecteur piloté en DMX.
pub struct Decode<'a> {
    pub mode: &'a DmxMode,
    /// univers (512 octets)
    pub data: &'a [u8],
    /// adresse de départ (1..512)
    pub address: i64,
    pub pixel_count: usize,
    /// secondes
    pub time: f64,
    /// couleur du blanc « W » et des modes sans couleur
    pub white_rgb: Rgb,
    pub curve: Curve,
    pub index_colors: Option<&'a IndexColors>,
}

/// Sortie complète d'un projecteur piloté en DMX : `out` reçoit `pixel_count` × 3 valeurs.
/// `runtime` garde le départ des effets d'une image à l'autre.
pub fn decode_dmx_output(p: &Decode, runtime: Option<&mut FxState>, out: &mut [f64]) -> (Attrs, Vec<Attrs>) {
    // un mode à M pixels sur un projecteur à N pixels physiques : chaque pixel DMX pilote N/M pixels
    let mode_px = p.mode.pixels_on(p.pixel_count);
    let (fixture, pixels) = read_attributes(p.mode, p.data, p.address, mode_px);
    let master = fixture.get(Func::Dimmer).map_or(1.0, |d| p.curve.apply(d));
    let gate = strobe_gate(fixture.get(Func::Strobe).unwrap_or(0.0), p.time, 0.2, p.address as f64);

    if let Some(fx_program) = fixture.get(Func::FxProgram) {
        // mode effet : programme interne du projecteur
        let program = program_from_dmx(fx_program);
        let mut local = FxState::default();
        let rt = runtime.unwrap_or(&mut local);
        let restart = fixture.get(Func::FxRestart);
        if rt.program != Some(program) || rt.restart != restart {
            rt.program = Some(program);
            rt.restart = restart;
            rt.start = Some(p.time);
        }
        let dir = direction_from_dmx(js_round(fixture.get(Func::FxDirection).unwrap_or(0.0) * 255.0));
        let colors = effect_colors(&fixture, p.index_colors);
        render_program(
            &Program {
                program,
                colors: &colors,
                pixels: p.pixel_count,
                time: p.time - rt.start.unwrap_or(0.0),
                speed: fixture.get(Func::FxSpeed).unwrap_or(0.5),
                fade: fixture.get(Func::FxFade).unwrap_or(0.5),
                reverse: dir.reverse,
                once: dir.once,
                size: 1.0 + (fixture.get(Func::FxSize).unwrap_or(0.0) * 4.0).floor().min(3.0),
                offset: fixture.get(Func::FxOffset).unwrap_or(0.0),
            },
            out,
        );
        let k = master * gate;
        for v in out.iter_mut().take(p.pixel_count * 3) {
            *v *= k;
        }
        return (fixture, pixels);
    }

    let mut colors = Vec::with_capacity(mode_px);
    for (i, px) in pixels.iter().enumerate() {
        let attrs = fixture.merged(px);
        let px_dim = px.get(Func::Dimmer).map_or(1.0, |d| p.curve.apply(d));
        let px_gate = match px.get(Func::Strobe) {
            Some(hz) if hz != 0.0 && !hz.is_nan() => strobe_gate(hz, p.time, 0.2, (p.address + i as i64) as f64),
            _ => 1.0,
        };
        let rgb = attributes_to_rgb(&attrs, p.white_rgb, p.index_colors);
        let k = master * px_dim * gate * px_gate;
        colors.push([rgb[0] * k, rgb[1] * k, rgb[2] * k]);
    }
    for j in 0..p.pixel_count {
        let c = colors[(mode_px - 1).min(j * mode_px / p.pixel_count)];
        out[j * 3..j * 3 + 3].copy_from_slice(&c);
    }
    (fixture, pixels)
}
