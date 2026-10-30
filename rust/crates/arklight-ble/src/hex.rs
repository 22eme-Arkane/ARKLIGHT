//! Octets ↔ texte hexadécimal : forme des octets dans les profils JSON et le journal des échanges
//! (comme `Buffer.toString('hex')` en JS et `bytes.fromhex` en Python).

use serde::{Deserialize, Deserializer, Serializer};

/// Octets → texte hexadécimal en minuscules (`b"AR"` → `"4152"`).
pub fn encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0x0f) as usize] as char);
    }
    out
}

/// Texte hexadécimal → octets ; les espaces sont ignorés (comme `bytes.fromhex`), majuscules acceptées.
pub fn decode(text: &str) -> Result<Vec<u8>, String> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return Err(format!("texte hexadécimal de longueur impaire : « {text} »"));
    }
    let nibble = |c: u8| match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("caractère non hexadécimal « {} » dans « {text} »", c as char)),
    };
    digits.chunks(2).map(|pair| Ok((nibble(pair[0])? << 4) | nibble(pair[1])?)).collect()
}

/// Champ facultatif d'octets écrit en hexadécimal (`"value": "41524b4c49474854"`) ; absent ou
/// `null` → `None`.
pub(crate) mod option {
    use super::*;

    pub fn serialize<S: Serializer>(value: &Option<Vec<u8>>, s: S) -> Result<S::Ok, S::Error> {
        match value {
            Some(bytes) => s.serialize_str(&encode(bytes)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Vec<u8>>, D::Error> {
        let text: Option<String> = Option::deserialize(d)?;
        text.map(|t| decode(&t).map_err(serde::de::Error::custom)).transpose()
    }
}

/// Octets écrits en hexadécimal (journal des échanges, champ `hex` comme en JS).
pub(crate) fn serialize_bytes<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour() {
        assert_eq!(encode(b"ARKLIGHT"), "41524b4c49474854");
        assert_eq!(encode(&[]), "");
        assert_eq!(encode(&[0x00, 0x0f, 0xf0, 0xff]), "000ff0ff");
        assert_eq!(decode("41524b4c49474854").unwrap(), b"ARKLIGHT");
        assert_eq!(decode("DE AD be ef").unwrap(), [0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(decode("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn erreurs() {
        assert!(decode("abc").is_err());
        assert!(decode("zz").is_err());
        assert!(decode("0x12").is_err());
    }
}
