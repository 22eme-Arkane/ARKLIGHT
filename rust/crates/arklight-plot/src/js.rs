//! Calculs et écritures « à la JavaScript » : mêmes arrondis, mêmes nombres dans le SVG et le XML
//! que la version JavaScript (`Number#toString`, `Math.round`, `Math.hypot` de V8, `toFixed`,
//! `localeCompare` en français), pour des fichiers identiques d'une version à l'autre.

use serde_json::Value;
use std::cmp::Ordering;

/// `Math.round` : moitiés arrondies vers le haut (-2,5 → -2).
pub fn round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// Arrondi au dixième (`f1` du plan : pixels).
pub fn f1(v: f64) -> f64 {
    round(v * 10.0) / 10.0
}

/// Arrondi au millième (`f3` des symboles : mètres).
pub fn f3(v: f64) -> f64 {
    round(v * 1000.0) / 1000.0
}

/// Un nombre écrit comme `String(n)` en JavaScript (« 0.1 », « 12 », « 1e-7 », « NaN »).
pub fn num(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return "0".into(); // -0 compris
    }
    let a = x.abs();
    if (1e-6..1e21).contains(&a) {
        return format!("{x}");
    }
    // notation scientifique : « 1e+21 », « 1.5e-7 »
    let s = format!("{x:e}");
    match s.split_once('e') {
        Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
        _ => s,
    }
}

/// `Math.hypot` de V8 (à deux nombres) : somme de Kahan des carrés ramenés au plus grand. Même
/// résultat au bit près que la version JavaScript (la fonction de la bibliothèque C diffère parfois).
pub fn hypot(a: f64, b: f64) -> f64 {
    let (a, b) = (a.abs(), b.abs());
    if a == f64::INFINITY || b == f64::INFINITY {
        return f64::INFINITY;
    }
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    let max = if b > a { b } else { a };
    if max == 0.0 {
        return 0.0;
    }
    let (mut sum, mut comp) = (0.0_f64, 0.0_f64);
    for v in [a, b] {
        let n = v / max;
        let summand = n * n - comp;
        let preliminary = sum + summand;
        comp = (preliminary - sum) - summand;
        sum = preliminary;
    }
    sum.sqrt() * max
}

/// `Math.max(...v)` : NaN l'emporte (contrairement à `f64::max`).
pub fn max_of(v: impl IntoIterator<Item = f64>) -> f64 {
    let mut m = f64::NEG_INFINITY;
    for x in v {
        if x.is_nan() {
            return f64::NAN;
        }
        if x > m {
            m = x;
        }
    }
    m
}

/// `Math.min(...v)`
pub fn min_of(v: impl IntoIterator<Item = f64>) -> f64 {
    let mut m = f64::INFINITY;
    for x in v {
        if x.is_nan() {
            return f64::NAN;
        }
        if x < m {
            m = x;
        }
    }
    m
}

/// `Math.max(a, b)`
pub fn max2(a: f64, b: f64) -> f64 {
    max_of([a, b])
}

/// `Math.min(a, b)`
pub fn min2(a: f64, b: f64) -> f64 {
    min_of([a, b])
}

/// `x.toFixed(d)` : arrondi de la valeur décimale exacte, moitiés vers le haut (Rust arrondit les
/// moitiés exactes au pair).
pub fn to_fixed(x: f64, d: usize) -> String {
    if !x.is_finite() {
        return num(x);
    }
    if x.abs() >= 1e21 {
        return num(x);
    }
    // 40 chiffres après la virgule suffisent à départager une moitié exacte (écart minimal ~1e-21)
    let s = format!("{:.40}", x.abs());
    let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
    let mut digits: Vec<u8> = int.bytes().chain(frac.bytes().take(d)).map(|c| c - b'0').collect();
    if frac.as_bytes().get(d).is_some_and(|&c| c >= b'5') {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, 1);
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let n = digits.len() - d;
    let mut out = String::new();
    // le signe reste même si l'arrondi donne zéro (« -0.00 »), sauf pour -0
    if x < 0.0 {
        out.push('-');
    }
    out.extend(digits[..n].iter().map(|&c| (b'0' + c) as char));
    if d > 0 {
        out.push('.');
        out.extend(digits[n..].iter().map(|&c| (b'0' + c) as char));
    }
    out
}

/// Longueur d'un texte en JavaScript (unités UTF-16).
pub fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Échappement des textes et attributs du SVG (`esc` du plan : & < > ").
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Valeur « vraie » au sens de JavaScript.
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `String(v)` d'une valeur JSON simple (texte, nombre, booléen).
pub fn string_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), num),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        _ => String::new(),
    }
}

/// Texte non vide (`v || défaut` sur un texte).
pub fn text_of(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).filter(|s| !s.is_empty())
}

/// Nombre non nul (`v || défaut` sur un nombre).
pub fn num_truthy(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|x| *x != 0.0 && !x.is_nan())
}

// ------------------------------------------------------------------ lettres accentuées
// Décomposition canonique (NFD) des lettres latines accentuées : lettre de base et accent combinant.
const DECOMPOSED: [(u32, &str, &str); 13] = [
    (0x300, "ÀÈÌÒÙàèìòùǸǹẀẁỲỳ", "AEIOUaeiouNnWwYy"),
    (0x301, "ÁÉÍÓÚÝáéíóúýĆćĹĺŃńŔŕŚśŹźǴǵẂẃ", "AEIOUYaeiouyCcLlNnRrSsZzGgWw"),
    (0x302, "ÂÊÎÔÛâêîôûĈĉĜĝĤĥĴĵŜŝŴŵŶŷ", "AEIOUaeiouCcGgHhJjSsWwYy"),
    (0x303, "ÃÑÕãñõĨĩŨũẼẽỸỹ", "ANOanoIiUuEeYy"),
    (0x304, "ĀāĒēĪīŌōŪūȲȳ", "AaEeIiOoUuYy"),
    (0x306, "ĂăĔĕĞğĬĭŎŏŬŭ", "AaEeGgIiOoUu"),
    (0x307, "ĊċĖėĠġİŻżṠṡ", "CcEeGgIZzSs"),
    (0x308, "ÄËÏÖÜäëïöüÿŸẄẅ", "AEIOUaeiouyYWw"),
    (0x30A, "ÅåŮů", "AaUu"),
    (0x30B, "ŐőŰű", "OoUu"),
    (0x30C, "ČčĎďĚěŇňŘřŠšŤťŽžĽľǍǎǏǐǑǒǓǔ", "CcDdEeNnRrSsTtZzLlAaIiOoUu"),
    (0x327, "ÇçĢģĶķĻļŅņŖŗŞşŢţ", "CcGgKkLlNnRrSsTt"),
    (0x328, "ĄąĘęĮįŲų", "AaEeIiUu"),
];

/// Lettre accentuée → (lettre de base, accent combinant).
pub fn decompose(c: char) -> Option<(char, u32)> {
    if c.is_ascii() {
        return None;
    }
    for (mark, from, to) in DECOMPOSED {
        if let Some(i) = from.chars().position(|x| x == c) {
            return to.chars().nth(i).map(|b| (b, mark));
        }
    }
    None
}

/// Accent combinant (U+0300 à U+036F).
pub fn is_mark(c: char) -> bool {
    ('\u{300}'..='\u{36f}').contains(&c)
}

/// `s.normalize('NFD').replace(/[̀-ͯ]/g, '')` : le texte sans ses accents.
pub fn strip_accents(s: &str) -> String {
    s.chars().filter(|c| !is_mark(*c)).map(|c| decompose(c).map_or(c, |(b, _)| b)).collect()
}

// ------------------------------------------------------------------ ordre alphabétique
// `a.localeCompare(b, 'fr')` (ICU, ordre de base du français) : d'abord les lettres sans accent ni
// casse (espaces, puis ponctuation, symboles, chiffres et lettres), puis les accents, puis la casse
// (minuscule avant majuscule).
const PUNCT: &str = "_-‐‑‒–—―,;:!¡?¿.…·'‘’‚‹›\"“”„«»()[]{}§¶@*/\\&#%‰†‡•`´˜^¯˘˙¨˚˝¸˛°©®+±÷×<=>¬|¦~¤¢$£¥€";
// rang des accents (poids secondaire) ; 0 : sans accent
const MARKS: [u32; 14] = [0x301, 0x300, 0x306, 0x302, 0x30C, 0x30A, 0x308, 0x30B, 0x303, 0x307, 0x327, 0x328, 0x304, 0x335];

fn mark_weight(m: u32) -> u8 {
    MARKS.iter().position(|&x| x == m).map_or(30, |i| i as u8 + 1)
}

#[derive(Default)]
struct SortKey {
    primary: Vec<u32>,
    secondary: Vec<u8>,
    tertiary: Vec<u8>,
}

impl SortKey {
    fn letter(&mut self, base: char, upper: bool) {
        self.primary.push(0x4_0000 + base.to_ascii_lowercase() as u32);
        self.secondary.push(0);
        self.tertiary.push(u8::from(upper));
    }
}

fn sort_key(s: &str) -> SortKey {
    let mut k = SortKey::default();
    for c in s.chars() {
        if is_mark(c) {
            k.secondary.push(mark_weight(c as u32));
            continue;
        }
        let upper = c.is_uppercase();
        let (base, mark) = match c {
            'Ø' | 'ø' => ('o', Some(0x335)),
            'Đ' | 'đ' => ('d', Some(0x335)),
            'Ł' | 'ł' => ('l', Some(0x335)),
            'Æ' | 'æ' | 'Œ' | 'œ' | 'ß' => {
                let pair = match c {
                    'Æ' | 'æ' => ['a', 'e'],
                    'Œ' | 'œ' => ['o', 'e'],
                    _ => ['s', 's'],
                };
                for b in pair {
                    k.letter(b, upper);
                }
                continue;
            }
            _ => match decompose(c) {
                Some((b, m)) => (b, Some(m)),
                None => (c, None),
            },
        };
        if base.is_ascii_alphabetic() {
            k.letter(base, upper);
            if let Some(m) = mark {
                k.secondary.push(mark_weight(m));
            }
            continue;
        }
        let (p, t) = if base.is_whitespace() {
            (0x1_0000 + base as u32, 0)
        } else if let Some(i) = PUNCT.chars().position(|x| x == base) {
            (0x2_0000 + i as u32, 0)
        } else if base.is_ascii_digit() {
            (0x3_0000 + base as u32 - '0' as u32, 0)
        } else if let Some(d) = "⁰¹²³⁴⁵⁶⁷⁸⁹".chars().position(|x| x == base) {
            (0x3_0000 + d as u32, 2)
        } else if base.is_alphabetic() {
            (0x5_0000 + base.to_lowercase().next().unwrap_or(base) as u32, u8::from(upper))
        } else if base.is_control() {
            continue;
        } else {
            (0x2_1000 + base as u32, 0)
        };
        k.primary.push(p);
        k.secondary.push(0);
        k.tertiary.push(t);
    }
    k
}

/// `a.localeCompare(b, 'fr')`
pub fn collate(a: &str, b: &str) -> Ordering {
    let (ka, kb) = (sort_key(a), sort_key(b));
    ka.primary.cmp(&kb.primary).then_with(|| ka.secondary.cmp(&kb.secondary)).then_with(|| ka.tertiary.cmp(&kb.tertiary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_comme_javascript() {
        assert_eq!(num(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(12.0), "12");
        assert_eq!(num(1e21), "1e+21");
        assert_eq!(num(1.5e-7), "1.5e-7");
        assert_eq!(num(0.000001), "0.000001");
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(round(2.5), 3.0);
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_fixed(9.995, 2), "9.99");
        assert_eq!(to_fixed(99.996, 2), "100.00");
        assert_eq!(to_fixed(-0.001, 2), "-0.00");
        assert_eq!(to_fixed(-0.0, 2), "0.00");
        assert_eq!(to_fixed(2.675, 2), "2.67");
        assert_eq!(to_fixed(-1.5, 0), "-2");
        assert_eq!(strip_accents("Électro Ça"), "Electro Ca");
    }
}
