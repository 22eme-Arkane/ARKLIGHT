//! Outils communs aux panneaux des marques (adresse DMX, profils, courbes, réglages de la lumière),
//! portage de `shared/onboard/panels/common.js`.

use crate::fixture::obj;
use crate::js::{self, clamp, jv};
use crate::panel::{Brand, Item, Pc, num, pick_dyn};
use arklight_core::library::DmxMode;
use serde_json::{Value, json};

pub(crate) const GREEN: &str = "#3ddc84";
pub(crate) const BLUE: &str = "#3d8bff";
pub(crate) const RED: &str = "#ff4d4d";
pub(crate) const AMBER: &str = "#ffb238";
pub(crate) const WHITE: &str = "#f4f6f8";
pub(crate) const PURPLE: &str = "#b56cff";
pub(crate) const ORANGE: &str = "#ff8a3d";
pub(crate) const YELLOW: &str = "#ffe14d";

/// Nombre de canaux du mode DMX (au moins 1).
pub(crate) fn footprint<S: Brand>(c: &Pc<S>) -> f64 {
    let n = c.ty.mode(c.s("dmxMode").and_then(Value::as_str)).map_or(0, DmxMode::footprint);
    if n == 0 { 1.0 } else { n as f64 }
}

/// Plage de températures du modèle (`type.cctRange`), sinon `dflt`.
pub(crate) fn cct_range<S: Brand>(c: &Pc<S>, dflt: (f64, f64)) -> (f64, f64) {
    match c.ty.extra.get("cctRange").and_then(Value::as_array) {
        Some(a) if a.len() >= 2 => (js::number(a.first()), js::number(a.get(1))),
        _ => dflt,
    }
}

/// `pad(n, w)`
pub(crate) fn pad(n: f64, w: usize) -> String {
    js::pad(n, w)
}

/// Adresse DMX (1 … 513 − nombre de canaux du mode).
pub(crate) fn address_item<S: Brand>(label: &str) -> Item<S> {
    num(label, 1.0, 512.0)
        .max_fn(|c| 513.0 - footprint(c))
        .get(|c| c.s("dmxAddress").cloned().unwrap_or(Value::Null))
        .set(|c, v| {
            c.set_settings(obj([("dmxAddress", v)]));
            None
        })
        .fmt(|v, _| pad(js::number(Some(v)), 3))
}

/// Profil (mode) DMX parmi ceux du modèle.
pub(crate) fn profile_item<S: Brand>(label: &str, fmt: fn(&DmxMode, usize) -> String) -> Item<S> {
    pick_dyn(label, move |c: &Pc<S>| c.ty.dmx.modes.iter().enumerate().map(|(i, m)| (json!(m.id), fmt(m, i))).collect())
        .get(|c| c.s("dmxMode").cloned().unwrap_or(Value::Null))
        .set(|c, v| {
            c.set_settings(obj([("dmxMode", v)]));
            None
        })
}

/// Courbe de gradation : libellés de la marque (exponentielle, logarithmique, S, linéaire) →
/// courbes du moteur (absentes : None).
pub(crate) fn curve_item<S: Brand>(label: &str, names: [Option<&'static str>; 4]) -> Item<S> {
    let list: Vec<(Value, String)> = ["square", "invSquare", "sCurve", "linear"].iter().zip(names).filter_map(|(v, n)| n.map(|n| (json!(v), n.to_string()))).collect();
    crate::panel::pick(label, list).get(|c| c.s("dimmingCurve").cloned().unwrap_or(Value::Null)).set(|c, v| {
        c.set_settings(obj([("dimmingCurve", v)]));
        None
    })
}

/// Empreinte FNV-1a de l'identifiant du projecteur (numéros de série fictifs).
pub(crate) fn id_hash<S: Brand>(c: &Pc<S>) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for code in js::char_codes(&c.fx.id) {
        h = (h ^ code).wrapping_mul(16_777_619);
    }
    h
}

/// Numéro de série fictif, stable pour un projecteur (lettres et chiffres).
pub(crate) fn serial_of<S: Brand>(c: &Pc<S>, len: usize, alphabet: &str) -> String {
    let chars: Vec<char> = alphabet.chars().collect();
    let n = chars.len() as f64;
    let mut h = f64::from(id_hash(c));
    let mut s = String::new();
    for _ in 0..len {
        // le mélange peut rendre h négatif : indice ramené dans l'alphabet
        s.push(chars[((h % n + n) % n) as usize]);
        h = (h / n).floor() + js::imul(h, 2_654_435_761.0) % 997.0;
    }
    s
}

pub(crate) const SERIAL_ALPHABET: &str = "0123456789ABCDEFGHJKLMNPRSTUVWXYZ";

/// Réglage manuel : intensité, température, teinte ±vert, teinte, saturation (utilisé sans DMX).
pub(crate) struct Nudge {
    pub step: f64,
    pub min: f64,
    pub max: f64,
    pub wrap: bool,
    pub round: f64,
}

impl Nudge {
    pub fn new(step: f64, min: f64, max: f64) -> Nudge {
        Nudge { step, min, max, wrap: false, round: 1000.0 }
    }
    pub fn round(mut self, r: f64) -> Nudge {
        self.round = r;
        self
    }
    pub fn wrap(mut self) -> Nudge {
        self.wrap = true;
        self
    }
}

pub(crate) fn nudge<S: Brand>(c: &mut Pc<S>, key: &str, steps: f64, o: Nudge) -> f64 {
    let cur = match key {
        "sat" => js::nn(c.m("sat"), 1.0),
        "intensity" => js::nn(c.m("intensity"), 0.0),
        _ => js::nn(c.m(key), 0.0),
    };
    let mut v = cur + steps * o.step;
    if o.wrap {
        let span = o.max - o.min;
        v = ((v - o.min) % span + span) % span + o.min;
    }
    v = clamp(js::round(v * o.round) / o.round, o.min, o.max);
    c.set_manual(obj([(key, jv(v))]));
    v
}

/// `pct(v, d)` : « 60.0% »
pub(crate) fn pct(v: f64, d: usize) -> String {
    format!("{}%", js::to_fixed(v * 100.0, d))
}
