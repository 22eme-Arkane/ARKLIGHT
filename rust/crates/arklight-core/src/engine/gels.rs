//! Couleurs des gélatines LEE d'après leur nom, couleurs index et couleurs prédéfinies des Astera
//! (portage de `shared/engine/gels.js` et `shared/fixtures/astera-colors.js`).
//!
//! Les « couleurs index » Astera donnent seulement le numéro et le nom du gel (valeur DMX = numéro
//! LEE) : on en déduit une teinte plausible pour l'affichage (approximation visuelle, pas une mesure).

use crate::color::{Rgb, cct_to_rgb, hs_to_rgb, srgb_to_linear};
use crate::engine::{field, num, num_or, text};
use crate::library::library;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

const HUES: [(&str, f64); 39] = [
    ("primary red", 0.0),
    ("scarlet", 355.0),
    ("crimson", 350.0),
    ("red", 0.0),
    ("fire", 12.0),
    ("sunset", 14.0),
    ("flame", 18.0),
    ("salmon", 10.0),
    ("peach", 20.0),
    ("apricot", 24.0),
    ("chocolate", 24.0),
    ("bronze", 30.0),
    ("orange", 28.0),
    ("amber", 38.0),
    ("gold", 45.0),
    ("straw", 48.0),
    ("yellow", 56.0),
    ("lime", 88.0),
    ("moss", 95.0),
    ("fern", 130.0),
    ("jade", 150.0),
    ("green", 120.0),
    ("turquoise", 178.0),
    ("teal", 180.0),
    ("peacock", 185.0),
    ("cyan", 188.0),
    ("aqua", 190.0),
    ("sky", 205.0),
    ("steel", 212.0),
    ("tokyo", 230.0),
    ("navy", 232.0),
    ("blue", 225.0),
    ("lavender", 262.0),
    ("violet", 270.0),
    ("purple", 280.0),
    ("mauve", 300.0),
    ("magenta", 305.0),
    ("pink", 330.0),
    ("rose", 340.0),
];

struct Patterns {
    neutral: Regex,
    ct: Regex,
    density: Regex,
    pale: Regex,
    medium: Regex,
    deep: Regex,
    dark: Regex,
    half: Regex,
    quarter: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| {
        let r = |s: &str| Regex::new(s).expect("motif de gel");
        Patterns {
            neutral: r("frost|diffus|clear|opal|silk|grid|cloth|hampshire|soft"),
            ct: r(r"(full|half|quarter|1/2|1/4|1/8|eighth|double)?\s*c\.?\s*t\.?\s*(blue|orange|straw|eight)"),
            density: r(r"neutral density|\bnd\b|grey|gray"),
            pale: r("pale|light|tint|paler|lighter|surprise|bastard"),
            medium: r("medium|middle|mist|soft"),
            deep: r("dark|deep|deeper|primary|just|congo|bright|vivid|rich"),
            dark: r("dark|deep"),
            half: r("half|1/2"),
            quarter: r("quarter|1/4"),
        }
    })
}

/// Couleur (sRGB linéaire, max = 1) d'un gel d'après son nom.
pub fn gel_color(name: &str) -> Rgb {
    let p = patterns();
    let n = name.to_lowercase();
    if p.neutral.is_match(&n) {
        return [1.0; 3];
    }
    if let Some(ct) = p.ct.captures(&n) {
        let k = match ct.get(1).map_or("full", |m| m.as_str()) {
            "double" => 2.0,
            "half" | "1/2" => 0.5,
            "quarter" | "1/4" => 0.25,
            "eighth" | "1/8" => 0.125,
            _ => 1.0,
        };
        let kind = ct.get(2).map_or("", |m| m.as_str());
        if kind == "blue" {
            return cct_to_rgb(3200.0 + 2400.0 * k, 0.0);
        }
        return cct_to_rgb(5600.0 - 2400.0 * k, if kind == "straw" { 0.15 } else { 0.0 });
    }
    let amount = |n: &str| {
        if p.half.is_match(n) {
            0.5
        } else if p.quarter.is_match(n) {
            0.25
        } else {
            1.0
        }
    };
    if n.contains("plus green") {
        return cct_to_rgb(5600.0, amount(&n));
    }
    if n.contains("minus green") {
        return cct_to_rgb(5600.0, -amount(&n));
    }
    if p.density.is_match(&n) {
        return [0.5; 3];
    }
    let Some((_, hue)) = HUES.iter().find(|(w, _)| n.contains(w)) else {
        return [1.0, 0.92, 0.85];
    };
    let mut sat = 0.8;
    let mut val = 1.0;
    if p.pale.is_match(&n) {
        sat = 0.38;
    } else if p.medium.is_match(&n) {
        sat = 0.62;
    }
    if p.deep.is_match(&n) {
        sat = 1.0;
        val = if p.dark.is_match(&n) { 0.75 } else { 1.0 };
    }
    hs_to_rgb(*hue, sat, [1.0; 3]).map(|v| v * val)
}

struct IndexEntry {
    name: String,
    rgb: Option<Rgb>,
}

/// Table d'index : valeur DMX (0..255) → gel. Une valeur absente de la liste reprend l'entrée
/// précédente.
pub struct IndexColors {
    entries: Vec<IndexEntry>,
    table: [Option<usize>; 256],
}

/// Début d'une entrée de la liste (« 0..1 » → 0).
fn entry_start(key: &Value) -> f64 {
    match key {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => s.split("..").next().and_then(|a| a.trim().parse().ok()).unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

fn is_no_effect(name: &str) -> bool {
    name.to_lowercase().contains("no effect")
}

impl IndexColors {
    pub fn build(list: &[(Value, String)]) -> IndexColors {
        let mut entries: Vec<(f64, IndexEntry)> = list.iter().map(|(k, name)| (entry_start(k), IndexEntry { name: name.clone(), rgb: if is_no_effect(name) { None } else { Some(gel_color(name)) } })).collect();
        entries.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut table = [None; 256];
        let mut cur = None;
        let mut i = 0;
        for (v, slot) in table.iter_mut().enumerate() {
            while i < entries.len() && entries[i].0 <= v as f64 {
                cur = Some(i);
                i += 1;
            }
            *slot = cur;
        }
        IndexColors { entries: entries.into_iter().map(|(_, e)| e).collect(), table }
    }

    fn entry(&self, v: f64) -> Option<&IndexEntry> {
        if v.is_nan() {
            return None;
        }
        let i = super::js_round(v).clamp(0.0, 255.0) as usize;
        self.table[i].map(|e| &self.entries[e])
    }

    /// Couleur du gel de la valeur DMX `v` (« No effect » : None).
    pub fn color(&self, v: f64) -> Option<Rgb> {
        self.entry(v).and_then(|e| e.rgb)
    }

    pub fn name(&self, v: f64) -> &str {
        self.entry(v).map_or("", |e| e.name.as_str())
    }
}

/// Couleurs index des Astera (valeur DMX = numéro du gel LEE).
pub fn astera_index_colors() -> &'static IndexColors {
    static T: OnceLock<IndexColors> = OnceLock::new();
    T.get_or_init(|| IndexColors::build(&library().index_colors))
}

/// Un gel de la page « Color Gel » (liste LEE des couleurs index Astera).
pub struct LeeGel {
    pub number: f64,
    pub name: String,
}

pub fn lee_gels() -> &'static [LeeGel] {
    static G: OnceLock<Vec<LeeGel>> = OnceLock::new();
    G.get_or_init(|| library().index_colors.iter().filter(|(_, name)| !is_no_effect(name)).map(|(k, name)| LeeGel { number: entry_start(k), name: name.clone() }).collect())
}

/// Couleur « AsteraRGB » (0..255) → sRGB linéaire.
pub fn astera_rgb_to_linear(rgb: [f64; 3]) -> Rgb {
    rgb.map(|v| srgb_to_linear(v / 255.0))
}

/// Couleur du menu STATIC COLOR (« RED », « 5500K »…) ; inconnue : rouge.
pub fn static_color_rgb(name: &str) -> Rgb {
    library().static_colors.iter().find(|(n, _)| n == name).map_or([1.0, 0.0, 0.0], |(_, c)| astera_rgb_to_linear(*c))
}

/// Couleur d'une « spécification de couleur » du mode autonome :
/// `{ kind: "named", name }`, `{ kind: "custom", rgb: [0..255 ×3] }`, `{ kind: "index", value }`,
/// `{ kind: "hsi", cct, green, hue, sat }`, `{ kind: "gel", family, number }`.
pub fn color_spec_rgb(spec: Option<&Value>) -> Rgb {
    let Some(spec) = spec.filter(|s| !s.is_null()) else { return [0.0; 3] };
    let s = Some(spec);
    match text(field(s, "kind")) {
        Some("named") => static_color_rgb(text(field(s, "name")).unwrap_or("")),
        Some("custom") => {
            let rgb = field(s, "rgb").and_then(Value::as_array).map(|a| std::array::from_fn(|i| a.get(i).and_then(Value::as_f64).unwrap_or(0.0)));
            astera_rgb_to_linear(rgb.unwrap_or([255.0, 0.0, 0.0]))
        }
        Some("index") => num(field(s, "value")).and_then(|v| astera_index_colors().color(v)).unwrap_or([1.0; 3]),
        Some("hsi") => {
            let white = cct_to_rgb(num_or(field(s, "cct"), 5600.0), num_or(field(s, "green"), 0.0));
            hs_to_rgb(num_or(field(s, "hue"), 0.0), num_or(field(s, "sat"), 0.0) / 100.0, white)
        }
        Some("gel") => {
            let number = num(field(s, "number"));
            let gel = number.and_then(|n| lee_gels().iter().find(|g| g.number == n));
            let base = cct_to_rgb(if text(field(s, "family")) == Some("Lee Tungsten") { 3200.0 } else { 5600.0 }, 0.0);
            let filt = gel.map_or([1.0; 3], |g| gel_color(&g.name));
            let c = [base[0] * filt[0], base[1] * filt[1], base[2] * filt[2]];
            let m = c[0].max(c[1]).max(c[2]);
            let m = if m == 0.0 || m.is_nan() { 1.0 } else { m };
            c.map(|v| v / m)
        }
        _ => [1.0; 3],
    }
}
