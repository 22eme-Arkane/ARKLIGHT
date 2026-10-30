//! Clés API des fournisseurs d'IA (Anthropic, OpenAI…) et clé d'AIODOS (portage de
//! `server/ai/keys.js`) : gardées sur cet ordinateur seulement, à part des réglages, jamais
//! renvoyées aux interfaces.
//!
//! Chiffrement : sous Windows, DPAPI (`CryptProtectData` / `CryptUnprotectData`, compte de
//! l'utilisateur), valeurs « dpapi:… » ; ailleurs, écrites telles quelles (« raw:… »), comme la
//! version JavaScript quand le chiffrement manque.
//!
//! Version JavaScript (Electron) : ses valeurs « enc:… » viennent de `safeStorage`, qui ne chiffre
//! pas la clé par DPAPI directement : DPAPI protège une clé AES-256 rangée dans le fichier
//! « Local State » du profil Electron (`os_crypt.encrypted_key`), et chaque valeur est
//! « v10 » + nonce (12 octets) + AES-256-GCM. Elles sont relues si ce fichier est donné
//! ([`KeyStore::open_with_electron`]), puis réécrites en DPAPI au prochain enregistrement ; une
//! valeur illisible est gardée telle quelle (la version JavaScript la relit toujours).

use crate::util::{base64_decode, base64_encode};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Données propres à ARKLIGHT mêlées au chiffrement DPAPI (une autre appli du même compte ne
/// déchiffre pas ces valeurs par erreur).
#[cfg(windows)]
const ENTROPY: &[u8] = b"ARKLIGHT/ai-keys/1";

#[derive(Default)]
struct Inner {
    keys: BTreeMap<String, String>,
    /// valeurs chiffrées illisibles ici (autre compte, profil Electron absent) : réécrites telles quelles
    kept: BTreeMap<String, String>,
}

/// Le coffre des clés (fichier `ai-keys.json`). Partagé entre fils (`Arc<KeyStore>`).
pub struct KeyStore {
    file: Option<PathBuf>,
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for KeyStore {
    // jamais les clés
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "KeyStore({:?}, {:?})", self.file, self.list())
    }
}

impl KeyStore {
    /// Coffre en mémoire (essais) : rien n'est écrit.
    pub fn in_memory() -> KeyStore {
        KeyStore { file: None, inner: Mutex::new(Inner::default()) }
    }

    /// Lit le fichier (absent : pas encore de clés).
    pub fn open(file: impl Into<PathBuf>) -> KeyStore {
        KeyStore::open_with_electron(file, None)
    }

    /// Lit le fichier ; `local_state` : le fichier « Local State » du profil Electron de la version
    /// JavaScript (`%APPDATA%\arklight\Local State`), pour relire ses valeurs « enc:… ».
    pub fn open_with_electron(file: impl Into<PathBuf>, local_state: Option<&Path>) -> KeyStore {
        let file = file.into();
        let mut inner = Inner::default();
        let raw: Map<String, Value> = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let electron_key = local_state.and_then(electron_master_key);
        for (k, v) in raw {
            let Value::String(v) = v else { continue };
            match decode(&v, electron_key.as_deref()) {
                Some(key) if !key.is_empty() => {
                    inner.keys.insert(k, key);
                }
                Some(_) => {}
                None => {
                    inner.kept.insert(k, v);
                }
            }
        }
        KeyStore { file: Some(file), inner: Mutex::new(inner) }
    }

    /// Clé d'un fournisseur ('' : aucune).
    pub fn get(&self, provider: &str) -> String {
        self.inner.lock().map(|i| i.keys.get(provider).cloned().unwrap_or_default()).unwrap_or_default()
    }

    /// Fournisseurs qui ont une clé (pour l'interface : « clé enregistrée »).
    pub fn list(&self) -> Vec<String> {
        self.inner.lock().map(|i| i.keys.keys().cloned().collect()).unwrap_or_default()
    }

    /// Range (ou efface, valeur vide) la clé d'un fournisseur, puis réécrit le fichier.
    pub fn set(&self, provider: &str, value: &str) -> std::io::Result<()> {
        let key = value.trim();
        let mut inner = self.inner.lock().map_err(|_| std::io::Error::other("coffre des clés indisponible"))?;
        inner.kept.remove(provider);
        if key.is_empty() {
            inner.keys.remove(provider);
        } else {
            inner.keys.insert(provider.to_owned(), key.to_owned());
        }
        let Some(file) = &self.file else { return Ok(()) };
        let mut out = Map::new();
        for (k, v) in &inner.kept {
            out.insert(k.clone(), Value::String(v.clone()));
        }
        for (k, v) in &inner.keys {
            out.insert(k.clone(), Value::String(encode(v)?));
        }
        let text = serde_json::to_string_pretty(&Value::Object(out)).map_err(std::io::Error::other)?;
        let tmp = file.with_extension("json.tmp");
        std::fs::write(&tmp, text.as_bytes())?;
        if std::fs::rename(&tmp, file).is_err() {
            std::fs::write(file, text.as_bytes())?;
            let _ = std::fs::remove_file(&tmp);
        }
        Ok(())
    }

    /// Les clés sont-elles chiffrées sur ce système ?
    pub fn encrypted(&self) -> bool {
        cfg!(windows)
    }
}

/// Valeur du fichier → clé ; None : chiffrée et illisible ici.
fn decode(v: &str, electron_key: Option<&[u8]>) -> Option<String> {
    if let Some(b64) = v.strip_prefix("dpapi:") {
        let bytes = dpapi::unprotect(&base64_decode(b64)?, true)?;
        return String::from_utf8(bytes).ok();
    }
    if let Some(b64) = v.strip_prefix("enc:") {
        return electron_decrypt(&base64_decode(b64)?, electron_key?);
    }
    Some(v.strip_prefix("raw:").unwrap_or(v).to_owned())
}

fn encode(v: &str) -> std::io::Result<String> {
    if cfg!(windows) {
        let blob = dpapi::protect(v.as_bytes()).ok_or_else(|| std::io::Error::other("chiffrement Windows (DPAPI) impossible"))?;
        Ok(format!("dpapi:{}", base64_encode(&blob)))
    } else {
        Ok(format!("raw:{v}"))
    }
}

// ------------------------------------------------------------------ Electron (safeStorage)
/// Clé AES-256 du profil Electron : `os_crypt.encrypted_key` du fichier « Local State »
/// (« DPAPI » + données protégées par DPAPI, sans entropie).
fn electron_master_key(local_state: &Path) -> Option<Vec<u8>> {
    let text = std::fs::read_to_string(local_state).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let enc = base64_decode(v.get("os_crypt")?.get("encrypted_key")?.as_str()?)?;
    let blob = enc.strip_prefix(b"DPAPI")?;
    dpapi::unprotect(blob, false).filter(|k| k.len() == 32)
}

/// Une valeur de `safeStorage.encryptString` : « v10 » + nonce (12) + AES-256-GCM (texte + étiquette).
fn electron_decrypt(data: &[u8], key: &[u8]) -> Option<String> {
    use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
    let rest = data.strip_prefix(b"v10")?;
    if rest.len() < 12 + 16 {
        return None;
    }
    let (nonce, sealed) = rest.split_at(12);
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).ok()?);
    let mut buf = sealed.to_vec();
    let plain = key.open_in_place(Nonce::try_assume_unique_for_key(nonce).ok()?, Aad::empty(), &mut buf).ok()?;
    String::from_utf8(plain.to_vec()).ok()
}

/// Chiffre comme `safeStorage.encryptString` (essais de relecture des fichiers de la version
/// JavaScript).
#[doc(hidden)]
pub fn electron_encrypt_for_tests(plain: &str, key: &[u8], nonce: [u8; 12]) -> Vec<u8> {
    use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
    let Ok(unbound) = UnboundKey::new(&AES_256_GCM, key) else { return Vec::new() };
    let key = LessSafeKey::new(unbound);
    let mut buf = plain.as_bytes().to_vec();
    if key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut buf).is_err() {
        return Vec::new();
    }
    let mut out = b"v10".to_vec();
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&buf);
    out
}

/// Protège des octets par DPAPI sans entropie, comme Chromium pour `os_crypt.encrypted_key`
/// (essais).
#[doc(hidden)]
pub fn dpapi_protect_plain_for_tests(data: &[u8]) -> Option<Vec<u8>> {
    dpapi::protect_with(data, None)
}

// ------------------------------------------------------------------ DPAPI
#[cfg(windows)]
mod dpapi {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData};

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr().cast_mut() }
    }

    /// Copie le résultat de DPAPI puis libère sa mémoire.
    fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        if out.pbData.is_null() {
            return Vec::new();
        }
        // SAFETY : DPAPI a alloué `cbData` octets à `pbData` (LocalAlloc), libérés juste après la copie.
        let v = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        // SAFETY : mémoire rendue par CryptProtectData / CryptUnprotectData, à libérer par LocalFree.
        unsafe { LocalFree(out.pbData.cast()) };
        v
    }

    pub fn protect_with(data: &[u8], entropy: Option<&[u8]>) -> Option<Vec<u8>> {
        let input = blob(data);
        let ent = entropy.map(blob);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY : blobs valides le temps de l'appel ; `out` reçoit une mémoire libérée par `take`.
        let ok = unsafe { CryptProtectData(&input, std::ptr::null(), ent.as_ref().map_or(std::ptr::null(), |e| e as *const _), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) };
        if ok == 0 { None } else { Some(take(out)) }
    }

    pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
        protect_with(data, Some(super::ENTROPY))
    }

    /// `entropy` : avec celle d'ARKLIGHT (nos valeurs) ou sans (clé du profil Electron).
    pub fn unprotect(data: &[u8], entropy: bool) -> Option<Vec<u8>> {
        let input = blob(data);
        let ent = blob(super::ENTROPY);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        // SAFETY : blobs valides le temps de l'appel ; `out` reçoit une mémoire libérée par `take`.
        let ok = unsafe { CryptUnprotectData(&input, std::ptr::null_mut(), if entropy { &ent } else { std::ptr::null() }, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) };
        if ok == 0 { None } else { Some(take(out)) }
    }
}

#[cfg(not(windows))]
mod dpapi {
    pub fn protect_with(_: &[u8], _: Option<&[u8]>) -> Option<Vec<u8>> {
        None
    }
    pub fn protect(_: &[u8]) -> Option<Vec<u8>> {
        None
    }
    pub fn unprotect(_: &[u8], _: bool) -> Option<Vec<u8>> {
        None
    }
}
