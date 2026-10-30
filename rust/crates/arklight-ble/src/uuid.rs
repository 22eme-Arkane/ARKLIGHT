//! UUID Bluetooth : forme canonique en minuscules (`a7c1a000-0000-4c1e-8000-41524b4c4954`), comme
//! `str(uuid.UUID(...))` dans le pont Python. Les événements et le journal utilisent toujours cette
//! forme : un émulateur peut comparer les UUID reçus à ses constantes en minuscules.

use crate::Error;

/// Base des UUID courts de la norme Bluetooth (`0000xxxx-0000-1000-8000-00805f9b34fb`).
const BLUETOOTH_BASE: u128 = 0x0000_0000_0000_1000_8000_0080_5f9b_34fb;

/// Lit un UUID : 32 chiffres hexadécimaux avec ou sans tirets, entre accolades ou non ; accepte
/// aussi les UUID courts de 16 et 32 bits (`"fff0"`, `"0000fff0"`), complétés par la base Bluetooth.
pub fn parse_uuid(text: &str) -> Option<u128> {
    let t = text.trim();
    let t = t.strip_prefix('{').and_then(|t| t.strip_suffix('}')).unwrap_or(t);
    let digits: String = t.chars().filter(|&c| c != '-').collect();
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        4 | 8 => u32::from_str_radix(&digits, 16).ok().map(|short| BLUETOOTH_BASE | ((short as u128) << 96)),
        32 => u128::from_str_radix(&digits, 16).ok(),
        _ => None,
    }
}

/// UUID → forme canonique en minuscules.
pub(crate) fn format_uuid(v: u128) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        v >> 96,
        (v >> 80) & 0xffff,
        (v >> 64) & 0xffff,
        (v >> 48) & 0xffff,
        v & 0xffff_ffff_ffff
    )
}

/// UUID → forme canonique en minuscules, ou [`Error::InvalidUuid`].
pub fn normalize_uuid(text: &str) -> Result<String, Error> {
    parse_uuid(text).map(format_uuid).ok_or_else(|| Error::InvalidUuid(text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forme_canonique() {
        let canon = "a7c1a000-0000-4c1e-8000-41524b4c4954";
        assert_eq!(normalize_uuid(canon).unwrap(), canon);
        assert_eq!(normalize_uuid("A7C1A000-0000-4C1E-8000-41524B4C4954").unwrap(), canon);
        assert_eq!(normalize_uuid("{a7c1a000-0000-4c1e-8000-41524b4c4954}").unwrap(), canon);
        assert_eq!(normalize_uuid("a7c1a00000004c1e800041524b4c4954").unwrap(), canon);
        assert_eq!(parse_uuid(canon), Some(0xa7c1a000_0000_4c1e_8000_41524b4c4954));
    }

    #[test]
    fn uuid_courts() {
        assert_eq!(normalize_uuid("FFF0").unwrap(), "0000fff0-0000-1000-8000-00805f9b34fb");
        assert_eq!(normalize_uuid("0000180a").unwrap(), "0000180a-0000-1000-8000-00805f9b34fb");
    }

    #[test]
    fn uuid_invalides() {
        for bad in ["", "xyz", "123", "a7c1a000-0000-4c1e-8000-41524b4c495", "a7c1a000-0000-4c1e-8000-41524b4c4954ff", "+fff"] {
            assert_eq!(normalize_uuid(bad), Err(Error::InvalidUuid(bad.to_string())), "{bad}");
        }
    }
}
