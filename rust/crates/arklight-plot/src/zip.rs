//! Archive ZIP minimale (méthode STORE, sans compression ; portage de `shared/zip.js`) : fichiers
//! MVR et GDTF.

/// Date et heure d'un export (fichiers de l'archive, révision des GDTF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub year: i32,
    /// 1 à 12
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl Stamp {
    /// Instant donné en secondes depuis le 1er janvier 1970 (temps universel).
    pub fn from_unix(secs: i64) -> Self {
        let days = secs.div_euclid(86400);
        let rem = secs.rem_euclid(86400);
        // jours → date civile (algorithme de H. Hinnant)
        let z = days + 719468;
        let era = z.div_euclid(146097);
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
        Self { year, month, day, hour: (rem / 3600) as u32, minute: (rem % 3600 / 60) as u32, second: (rem % 60) as u32 }
    }

    /// Maintenant (temps universel).
    pub fn now_utc() -> Self {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        Self::from_unix(secs)
    }

    /// « 2026-10-27T08:30:14 » (`toISOString().slice(0, 19)`).
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", self.year, self.month, self.day, self.hour, self.minute, self.second)
    }

    /// Date du cartouche (« 27/10/2026 », comme `toLocaleDateString` en français et en anglais
    /// britannique).
    pub fn date(&self) -> String {
        format!("{:02}/{:02}/{:04}", self.day, self.month, self.year)
    }

    /// Heure et date au format MS-DOS.
    pub fn dos(&self) -> (u16, u16) {
        let time = (self.hour << 11) | (self.minute << 5) | (self.second / 2);
        let date = (((self.year.max(1980) - 1980) as u32) << 9) | (self.month << 5) | self.day;
        (time as u16, date as u16)
    }
}

fn crc_table() -> &'static [u32; 256] {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, slot) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb88320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    })
}

/// CRC-32 (ZIP, PNG).
pub fn crc32(bytes: &[u8]) -> u32 {
    let t = crc_table();
    let mut c = 0xffff_ffffu32;
    for &b in bytes {
        c = t[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xffff_ffff
}

/// Crée une archive ZIP (fichiers stockés tels quels, noms en UTF-8).
pub fn create_zip(files: &[(&str, &[u8])], time: &Stamp) -> Vec<u8> {
    let (t, d) = time.dos();
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let name = name.as_bytes();
        // en-tête local
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version nécessaire
        out.extend_from_slice(&0x0800u16.to_le_bytes()); // noms en UTF-8
        out.extend_from_slice(&0u16.to_le_bytes()); // STORE
        out.extend_from_slice(&t.to_le_bytes());
        out.extend_from_slice(&d.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        // répertoire central
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version créée par
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&t.to_le_bytes());
        central.extend_from_slice(&d.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 12]); // extra, commentaire, disque, attributs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Un fichier lu dans une archive.
#[derive(Debug, Clone, PartialEq)]
pub struct ZipFile {
    pub name: String,
    pub data: Vec<u8>,
    pub crc: u32,
}

/// Lit une archive ZIP non compressée (contrôle des exports).
pub fn read_zip(bytes: &[u8]) -> Result<Vec<ZipFile>, String> {
    let u16_at = |p: usize| bytes.get(p..p + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or("archive tronquée");
    let u32_at = |p: usize| bytes.get(p..p + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or("archive tronquée");
    let mut end = bytes.len().checked_sub(22).ok_or("Archive ZIP illisible")?;
    while u32_at(end)? != 0x0605_4b50 {
        end = end.checked_sub(1).ok_or("Archive ZIP illisible")?;
    }
    let count = u16_at(end + 10)?;
    let mut p = u32_at(end + 16)? as usize;
    let mut files = Vec::with_capacity(count);
    for _ in 0..count {
        let method = u16_at(p + 10)?;
        let size = u32_at(p + 20)? as usize;
        let name_len = u16_at(p + 28)?;
        let extra_len = u16_at(p + 30)?;
        let comment_len = u16_at(p + 32)?;
        let local = u32_at(p + 42)? as usize;
        let name = String::from_utf8_lossy(bytes.get(p + 46..p + 46 + name_len).ok_or("archive tronquée")?).into_owned();
        let at = local + 30 + u16_at(local + 26)? + u16_at(local + 28)?;
        if method != 0 {
            return Err(format!("{name} : compression non prise en charge"));
        }
        let data = bytes.get(at..at + size).ok_or("archive tronquée")?.to_vec();
        files.push(ZipFile { name, data, crc: u32_at(p + 16)? });
        p += 46 + name_len + extra_len + comment_len;
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_et_dates() {
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
        let s = Stamp::from_unix(1_793_089_814); // 2026-10-27T08:30:14Z
        assert_eq!(s.iso(), "2026-10-27T08:30:14");
        assert_eq!(s.date(), "27/10/2026");
        let zip = create_zip(&[("a.txt", b"bonjour"), ("é.xml", b"<x/>")], &s);
        let files = read_zip(&zip).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[1].name, "é.xml");
        assert_eq!(files[0].data, b"bonjour");
    }
}
