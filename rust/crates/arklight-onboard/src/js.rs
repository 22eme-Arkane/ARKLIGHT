//! Règles de JavaScript dont dépendent les menus (arrondis, `toFixed`, `String(n)`, valeurs
//! « vraies », `===`…) : les écrans affichent exactement les mêmes textes que la version
//! JavaScript.

use serde_json::{Map, Value};

/// `Math.round` (moitiés arrondies vers le haut).
pub fn round(x: f64) -> f64 {
    arklight_core::engine::js_round(x)
}

/// `Math.min(b, Math.max(a, v))`
pub fn clamp(v: f64, a: f64, b: f64) -> f64 {
    // NaN : Math.max(a, NaN) = NaN, Math.min(b, NaN) = NaN
    if v.is_nan() {
        return f64::NAN;
    }
    v.max(a).min(b)
}

/// `String(n)` pour un nombre.
pub fn num_str(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let a = x.abs();
    if !(1e-6..1e21).contains(&a) {
        // notation exponentielle : 1e+21, 1.5e-7
        let s = format!("{x:e}");
        return match s.split_once('e') {
            Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
            _ => s,
        };
    }
    format!("{x}")
}

/// `x.toFixed(d)` : arrondi décimal exact, les moitiés vers le haut (en valeur absolue).
pub fn to_fixed(x: f64, d: usize) -> String {
    if !x.is_finite() || x.abs() >= 1e21 {
        return num_str(x);
    }
    let neg = x < 0.0;
    // développement décimal exact du nombre binaire (assez de chiffres pour lire la décimale d+1)
    let long = format!("{:.*}", d + 100, x.abs());
    let (int_part, frac) = long.split_once('.').unwrap_or((&long, ""));
    let mut digits: Vec<u8> = int_part.bytes().chain(frac.bytes().take(d)).map(|b| b - b'0').collect();
    let up = frac.as_bytes().get(d).is_some_and(|b| *b >= b'5');
    if up {
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
    let n_int = digits.len() - d;
    let mut s: String = digits[..n_int].iter().map(|b| (b + b'0') as char).collect();
    if d > 0 {
        s.push('.');
        s.extend(digits[n_int..].iter().map(|b| (b + b'0') as char));
    }
    if neg { format!("-{s}") } else { s }
}

/// `str.padStart(w, ch)` (longueur en unités UTF-16, comme JavaScript).
pub fn pad_start(s: &str, w: usize, ch: char) -> String {
    let n = s.encode_utf16().count();
    if n >= w {
        return s.to_string();
    }
    let mut out: String = std::iter::repeat_n(ch, w - n).collect();
    out.push_str(s);
    out
}

/// `str.padEnd(w, ch)`
pub fn pad_end(s: &str, w: usize, ch: char) -> String {
    let n = s.encode_utf16().count();
    let mut out = s.to_string();
    out.extend(std::iter::repeat_n(ch, w.saturating_sub(n)));
    out
}

/// `pad(n, w)` des panneaux : `String(Math.round(n)).padStart(w, '0')`.
pub fn pad(n: f64, w: usize) -> String {
    pad_start(&num_str(round(n)), w, '0')
}

/// Longueur d'une chaîne pour JavaScript (unités UTF-16).
pub fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Nombre → valeur JSON (entier quand il l'est, comme `JSON.stringify`).
pub fn jv(x: f64) -> Value {
    if !x.is_finite() {
        return Value::Null;
    }
    if x.fract() == 0.0 && x.abs() < 9.007_199_254_740_992e15 {
        return Value::from(x as i64);
    }
    Value::from(x)
}

/// Valeur « vraie » (absente : fausse).
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `Number(v)` (absente : NaN).
pub fn number(v: Option<&Value>) -> f64 {
    match v {
        None => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() { 0.0 } else { t.parse().unwrap_or(f64::NAN) }
        }
        Some(Value::Array(a)) => match a.as_slice() {
            [] => 0.0,
            [one] => number(Some(one)),
            _ => f64::NAN,
        },
        Some(Value::Object(_)) => f64::NAN,
    }
}

/// `v ?? d` pour un nombre.
pub fn nn(v: Option<&Value>, d: f64) -> f64 {
    match v {
        None | Some(Value::Null) => d,
        Some(x) => number(Some(x)),
    }
}

/// `v || d` pour un nombre.
pub fn or(v: Option<&Value>, d: f64) -> f64 {
    if truthy(v) { number(v) } else { d }
}

/// `String(v)` (absente : « undefined »).
pub fn string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::Null) => "null".into(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => num_str(n.as_f64().unwrap_or(f64::NAN)),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a.iter().map(|x| if x.is_null() { String::new() } else { string(Some(x)) }).collect::<Vec<_>>().join(","),
        Some(Value::Object(_)) => "[object Object]".into(),
    }
}

/// `String(v ?? '')`
pub fn string_or_empty(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        x => string(x),
    }
}

/// Texte d'une valeur (`v || d`).
pub fn text_or<'a>(v: Option<&'a Value>, d: &'a str) -> &'a str {
    match v {
        Some(Value::String(s)) if !s.is_empty() => s,
        _ => d,
    }
}

/// Égalité stricte `===` (objets et tableaux : jamais égaux, comme deux références distinctes).
pub fn strict_eq(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Value::Null), Some(Value::Null)) => true,
        (Some(Value::Bool(x)), Some(Value::Bool(y))) => x == y,
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        (Some(Value::String(x)), Some(Value::String(y))) => x == y,
        _ => false,
    }
}

/// `{ ...a, ...b }` pour deux objets JSON.
pub fn spread(a: Option<&Value>, b: &Map<String, Value>) -> Map<String, Value> {
    let mut m = a.and_then(Value::as_object).cloned().unwrap_or_default();
    for (k, v) in b {
        m.insert(k.clone(), v.clone());
    }
    m
}

/// `parseInt(s, 36)`
pub fn parse_int36(s: &str) -> f64 {
    let t = s.trim_start();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let mut v = 0.0;
    let mut any = false;
    for c in t.chars() {
        let Some(d) = c.to_digit(36) else { break };
        v = v * 36.0 + f64::from(d);
        any = true;
    }
    if !any {
        return f64::NAN;
    }
    if neg { -v } else { v }
}

/// `ToInt32` (opérateurs binaires, `Math.imul`).
pub fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    let t = x.trunc();
    let m = t.rem_euclid(4_294_967_296.0);
    (m as u64 as u32) as i32
}

/// `Math.imul(a, b)`
pub fn imul(a: f64, b: f64) -> f64 {
    f64::from(to_int32(a).wrapping_mul(to_int32(b)))
}

/// Unités UTF-16 d'un texte (`charCodeAt(0)` de chaque caractère d'un `for…of`).
pub fn char_codes(s: &str) -> impl Iterator<Item = u32> + '_ {
    s.chars().map(|c| {
        let u = c as u32;
        if u > 0xFFFF { 0xD800 + ((u - 0x10000) >> 10) } else { u }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_fixed_comme_javascript() {
        assert_eq!(to_fixed(0.5, 0), "1");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_fixed(-0.001, 2), "-0.00");
        assert_eq!(to_fixed(-0.0, 2), "0.00");
        assert_eq!(to_fixed(99.95, 1), "100.0");
        assert_eq!(to_fixed(9.995, 2), "9.99");
        assert_eq!(to_fixed(60.0, 1), "60.0");
    }

    #[test]
    fn nombres_en_texte() {
        assert_eq!(num_str(5.0), "5");
        assert_eq!(num_str(-0.0), "0");
        assert_eq!(num_str(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(num_str(1e21), "1e+21");
        assert_eq!(num_str(1.5e-7), "1.5e-7");
        assert_eq!(pad(5.0, 3), "005");
        assert_eq!(parse_int36("tan-01"), 29.0 * 1296.0 + 10.0 * 36.0 + 23.0);
        assert!(parse_int36("-").is_nan());
        assert_eq!(imul(4_294_967_295.0, 3.0), -3.0);
    }
}
