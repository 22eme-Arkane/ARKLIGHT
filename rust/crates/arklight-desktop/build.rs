// Interface Slint compilée avec le programme ; textes en français dans le code (@tr), traductions
// anglaises dans ui/translations/en/LC_MESSAGES/arklight-desktop.po.
// Sous Windows (MSVC), l'icône et la description du programme (explorateur, gestionnaire des
// tâches) sont ajoutées à l'exécutable : fichier de ressources .res écrit ici, sans outil externe.
fn main() {
    // traductions sans contexte : un texte français = une traduction, quel que soit le composant
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent-dark".into())
        .with_bundled_translations("ui/translations")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).expect("interface Slint");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        println!("cargo:rerun-if-changed=assets/arklight.ico");
        let ico = std::fs::read("assets/arklight.ico").expect("assets/arklight.ico");
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("arklight.res");
        std::fs::write(&out, res::file(&ico)).expect("arklight.res");
        // link.exe accepte directement les fichiers .res
        println!("cargo:rustc-link-arg-bin=arklight={}", out.display());
    }
}

/// Fichier de ressources Win32 (.res) : icône (RT_ICON + RT_GROUP_ICON) et version (RT_VERSION).
mod res {
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    const RT_VERSION: u16 = 16;
    const LANG: u16 = 0x0409;

    fn pad4(b: &mut Vec<u8>) {
        while b.len() % 4 != 0 {
            b.push(0);
        }
    }

    fn u16s(b: &mut Vec<u8>, v: &[u16]) {
        for x in v {
            b.extend_from_slice(&x.to_le_bytes());
        }
    }

    fn u32s(b: &mut Vec<u8>, v: &[u32]) {
        for x in v {
            b.extend_from_slice(&x.to_le_bytes());
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Une ressource : en-tête (type et nom numériques) puis données, alignées sur 4 octets.
    fn entry(b: &mut Vec<u8>, ty: u16, name: u16, flags: u16, lang: u16, data: &[u8]) {
        u32s(b, &[data.len() as u32, 32]);
        u16s(b, &[0xffff, ty, 0xffff, name]);
        u32s(b, &[0]); // DataVersion
        u16s(b, &[flags, lang]);
        u32s(b, &[0, 0]); // Version, Characteristics
        b.extend_from_slice(data);
        pad4(b);
    }

    /// Nœud de VS_VERSIONINFO : longueur, longueur de la valeur, type, clé, valeur, enfants.
    fn node(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
        let mut b = Vec::new();
        u16s(&mut b, &[0, value_len, text as u16]);
        u16s(&mut b, &wide(key));
        pad4(&mut b);
        b.extend_from_slice(value);
        for c in children {
            pad4(&mut b);
            b.extend_from_slice(c);
        }
        let len = b.len() as u16;
        b[..2].copy_from_slice(&len.to_le_bytes());
        b
    }

    fn string(key: &str, value: &str) -> Vec<u8> {
        let w = wide(value);
        let mut v = Vec::new();
        u16s(&mut v, &w);
        node(key, &v, w.len() as u16, true, &[])
    }

    fn version() -> Vec<u8> {
        let part = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
        let (major, minor, patch) = (part("CARGO_PKG_VERSION_MAJOR"), part("CARGO_PKG_VERSION_MINOR"), part("CARGO_PKG_VERSION_PATCH"));
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let mut fixed = Vec::new();
        u32s(
            &mut fixed,
            &[
                0xfeef04bd,                // signature
                0x0001_0000,               // version de la structure
                major << 16 | minor,       // version du fichier
                patch << 16,
                major << 16 | minor,       // version du produit
                patch << 16,
                0x3f,                      // masque des indicateurs
                if version.contains('-') { 0x02 } else { 0 }, // VS_FF_PRERELEASE
                0x0004_0004,               // VOS_NT_WINDOWS32
                1,                         // VFT_APP
                0,
                0,
                0,
            ],
        );
        let strings = node(
            "040904b0",
            &[],
            0,
            true,
            &[
                string("CompanyName", "22eme ARKANE"),
                string("FileDescription", "ARKLIGHT"),
                string("FileVersion", &version),
                string("InternalName", "arklight"),
                string("LegalCopyright", "Copyright © 2026 22eme ARKANE — Apache-2.0"),
                string("OriginalFilename", "arklight.exe"),
                string("ProductName", "ARKLIGHT"),
                string("ProductVersion", &version),
            ],
        );
        let mut translation = Vec::new();
        u16s(&mut translation, &[LANG, 0x04b0]);
        let var = node("Translation", &translation, 4, false, &[]);
        node(
            "VS_VERSION_INFO",
            &fixed,
            fixed.len() as u16,
            false,
            &[node("StringFileInfo", &[], 0, true, &[strings]), node("VarFileInfo", &[], 0, true, &[var])],
        )
    }

    /// Le .res complet à partir d'un fichier .ico (images PNG ou BMP).
    pub fn file(ico: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        entry(&mut b, 0, 0, 0, 0, &[]); // en-tête vide obligatoire
        let rd16 = |at: usize| u16::from_le_bytes([ico[at], ico[at + 1]]);
        let rd32 = |at: usize| u32::from_le_bytes([ico[at], ico[at + 1], ico[at + 2], ico[at + 3]]);
        let count = rd16(4) as usize;
        let mut group = Vec::new();
        u16s(&mut group, &[0, 1, count as u16]);
        for i in 0..count {
            let e = 6 + i * 16;
            let (size, offset) = (rd32(e + 8) as usize, rd32(e + 12) as usize);
            let id = (i + 1) as u16;
            entry(&mut b, RT_ICON, id, 0x1010, LANG, &ico[offset..offset + size]);
            group.extend_from_slice(&ico[e..e + 12]); // largeur, hauteur, couleurs, réservé, plans, bits, taille
            u16s(&mut group, &[id]);
        }
        entry(&mut b, RT_GROUP_ICON, 1, 0x1030, LANG, &group);
        entry(&mut b, RT_VERSION, 1, 0x0030, LANG, &version());
        b
    }
}
