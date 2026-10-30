//! Petits outils communs : nombres et textes lus avec les règles de JavaScript (les arguments des
//! outils viennent d'un modèle, en JSON plus ou moins bien typé), base64.

use arklight_core::engine::js_round;
use serde_json::{Map, Value};

/// `Math.round(v * 100) / 100`
pub(crate) fn r2(v: f64) -> f64 {
    js_round(v * 100.0) / 100.0
}

/// `Math.round(v * 1000) / 1000`
pub(crate) fn r3(v: f64) -> f64 {
    js_round(v * 1000.0) / 1000.0
}

/// `Math.max` de JavaScript (NaN l'emporte).
pub(crate) fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) }
}

/// `Math.min` de JavaScript (NaN l'emporte).
pub(crate) fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.min(b) }
}

/// `Math.min(b, Math.max(a, v))`
pub(crate) fn clamp(v: f64, a: f64, b: f64) -> f64 {
    js_min(b, js_max(a, v))
}

/// `Number(texte)`
fn parse_number(s: &str) -> f64 {
    let t = s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    if t.is_empty() {
        return 0.0;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(digits) = t.strip_prefix(prefix) {
            return u64::from_str_radix(digits, radix).map_or(f64::NAN, |n| n as f64);
        }
    }
    // Rust lit aussi « inf » et « nan », que JavaScript refuse
    if t.chars().any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E') {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

/// `Number(v)` (absent : NaN, null : 0).
pub(crate) fn js_number(v: Option<&Value>) -> f64 {
    match v {
        None => f64::NAN,
        Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => parse_number(s),
        Some(Value::Array(a)) => match a.as_slice() {
            [] => 0.0,
            [one] => parse_number(&js_string(one)),
            _ => f64::NAN,
        },
        Some(Value::Object(_)) => f64::NAN,
    }
}

/// `Number.isFinite(Number(v)) ? Number(v) : d`
pub(crate) fn num(v: Option<&Value>, d: f64) -> f64 {
    let n = js_number(v);
    if n.is_finite() { n } else { d }
}

/// Le nombre est-il fini (`Number.isFinite(Number(v))`) ?
pub(crate) fn finite(v: Option<&Value>) -> bool {
    js_number(v).is_finite()
}

/// Valeur « vraie » au sens de JavaScript.
pub(crate) fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `String(v)`
pub(crate) fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), fmt_num),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        Value::Array(a) => a.iter().map(|x| if x.is_null() { String::new() } else { js_string(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// Un nombre écrit comme JavaScript (`${n}`) : 5600, 0.45, -1.5.
pub(crate) fn fmt_num(x: f64) -> String {
    if x.is_nan() {
        "NaN".into()
    } else if x.is_infinite() {
        if x > 0.0 { "Infinity".into() } else { "-Infinity".into() }
    } else if x == 0.0 {
        "0".into()
    } else {
        format!("{x}")
    }
}

/// Une valeur JSON écrite comme JavaScript dans un texte (`${v}`, absent : « undefined »).
pub(crate) fn fmt_value(v: Option<&Value>) -> String {
    v.map_or_else(|| "undefined".into(), js_string)
}

/// Nombre JSON comme l'écrirait `JSON.stringify` : entier sans décimale, NaN → null.
pub(crate) fn jnum(x: f64) -> Value {
    if x.is_finite() && x.fract() == 0.0 && x.abs() < 9_007_199_254_740_992.0 {
        if x == 0.0 { Value::from(0) } else { Value::from(x as i64) }
    } else {
        serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
    }
}

/// `[x, y, z]` en JSON.
pub(crate) fn jvec(v: [f64; 3]) -> Value {
    Value::Array(v.iter().map(|x| jnum(*x)).collect())
}

/// `vec3(v)` : trois nombres finis, sinon None.
pub(crate) fn vec3(v: Option<&Value>) -> Option<[f64; 3]> {
    let a = v?.as_array()?;
    if a.len() != 3 || !a.iter().all(|x| finite(Some(x))) {
        return None;
    }
    Some([js_number(Some(&a[0])), js_number(Some(&a[1])), js_number(Some(&a[2]))])
}

/// Objet JSON (`{ k: v, … }`) à partir de paires.
pub(crate) fn obj<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_owned(), v);
    }
    Value::Object(m)
}

// ------------------------------------------------------------------ textes
/// Lettres latines accentuées U+00C0..U+00FF → lettre de base (« * » : pas de décomposition).
const LATIN1: &str = "AAAAAA*CEEEEIIII*NOOOOO**UUUUY**aaaaaa*ceeeeiiii*nooooo**uuuuy*y";
/// U+0100..U+017F (latin étendu A).
const LATIN_EXT_A: &str = "AaAaAaCcCcCcCcDd**EeEeEeEeEeGgGgGgGgHh**IiIiIiIiI***JjKk*LlLlLl****NnNnNn***OoOoOo**RrRrRrSsSsSsSsTtTt**UuUuUuUuUuUuWwYyYZzZzZz*";

/// Lettre sans son accent (ce que garde la décomposition NFD sans ses signes diacritiques).
fn base_letter(c: char) -> char {
    let pick = |table: &str, i: usize| table.as_bytes().get(i).copied().filter(|b| *b != b'*').map_or(c, char::from);
    match c as u32 {
        0xC0..=0xFF => pick(LATIN1, c as usize - 0xC0),
        0x100..=0x17F => pick(LATIN_EXT_A, c as usize - 0x100),
        _ => c,
    }
}

/// `norm(s)` : sans accents (NFD sans les diacritiques U+0300–U+036F), en minuscules, sans espaces
/// autour.
pub(crate) fn norm(s: &str) -> String {
    let stripped: String = s.chars().filter(|c| !('\u{300}'..='\u{36f}').contains(c)).map(base_letter).collect();
    stripped.to_lowercase().trim().to_owned()
}

/// Mots d'une recherche : `norm(q).split(/[^a-z0-9.]+/).filter(Boolean)`.
pub(crate) fn words(q: &str) -> Vec<String> {
    norm(q).split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.')).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}

/// Les `n` premiers caractères.
pub(crate) fn take_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ------------------------------------------------------------------ base64
const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const B64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn encode_with(data: &[u8], table: &[u8; 64], pad: bool) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(table[((n >> (18 - 6 * i)) & 63) as usize]));
            } else if pad {
                out.push('=');
            }
        }
    }
    out
}

/// Base64 (avec « = »).
pub fn base64_encode(data: &[u8]) -> String {
    encode_with(data, B64, true)
}

/// Base64url sans « = » (RFC 4648 § 5).
pub fn base64url_encode(data: &[u8]) -> String {
    encode_with(data, B64URL, false)
}

/// Base64 ou base64url, avec ou sans « = » ; None si le texte n'en est pas.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Base64url de 43 caractères exactement (32 octets : défi, preuve d'AIODOS), sinon None.
pub(crate) fn base64url_decode_43(text: &str) -> Option<Vec<u8>> {
    let ok = text.len() == 43 && text.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-');
    if ok { base64_decode(text) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textes_et_nombres() {
        assert_eq!(norm("  Comédienne À Côté "), "comedienne a cote");
        assert_eq!(norm("Électro ÇA œuvre"), "electro ca œuvre");
        assert_eq!(words("SkyPanel S60-C, 3200K"), vec!["skypanel", "s60", "c", "3200k"]);
        assert_eq!(js_number(Some(&Value::from(" 12 "))), 12.0);
        assert!(js_number(Some(&Value::from("inf"))).is_nan());
        assert_eq!(js_number(Some(&Value::Null)), 0.0);
        assert_eq!(fmt_num(5600.0), "5600");
        assert_eq!(fmt_num(0.45), "0.45");
        assert_eq!(jnum(60.0), Value::from(60));
        assert_eq!(r3(1.23456), 1.235);
        assert_eq!(r2(-2.345), -2.35); // comme JavaScript (-2,345 × 100 = -234,50000000000003)
    }

    #[test]
    fn base64() {
        assert_eq!(base64_encode(b"ARKLIGHT"), "QVJLTElHSFQ=");
        assert_eq!(base64url_encode(&[0xfb, 0xff]), "-_8");
        assert_eq!(base64_decode("QVJLTElHSFQ=").as_deref(), Some(&b"ARKLIGHT"[..]));
        assert_eq!(base64_decode("-_8").as_deref(), Some(&[0xfb, 0xff][..]));
    }
}
