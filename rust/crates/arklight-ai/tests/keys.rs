//! Coffre des clés : chiffrées par Windows (DPAPI), relues, effacées ; valeurs de la version
//! JavaScript (Electron `safeStorage`, « raw: », texte brut). Dossier temporaire de l'essai
//! seulement : jamais les fichiers de l'utilisateur.

mod common;

use arklight_ai::KeyStore;
use arklight_ai::keys::{dpapi_protect_plain_for_tests, electron_encrypt_for_tests};
use arklight_ai::{base64_decode, base64_encode};
use common::TempDir;
use serde_json::{Value, json};

#[test]
fn ranger_relire_effacer() {
    let dir = TempDir::new("cles");
    let file = dir.0.join("ai-keys.json");
    let keys = KeyStore::open(&file);
    assert!(keys.list().is_empty());
    keys.set("openai", "  sk-essai-123  ").unwrap();
    keys.set("aiodos", "aiodos_app_k3j7m2qa_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
    assert_eq!(keys.get("openai"), "sk-essai-123");

    let text = std::fs::read_to_string(&file).unwrap();
    assert!(!text.contains("sk-essai-123"), "jamais en clair dans le fichier : {text}");
    let v: Value = serde_json::from_str(&text).unwrap();
    if cfg!(windows) {
        assert!(keys.encrypted());
        assert!(v["openai"].as_str().unwrap().starts_with("dpapi:"));
    }
    assert!(!format!("{keys:?}").contains("sk-essai"), "pas de clé dans les journaux");

    let again = KeyStore::open(&file);
    assert_eq!(again.get("openai"), "sk-essai-123");
    assert_eq!(again.list(), vec!["aiodos".to_owned(), "openai".to_owned()]);
    again.set("openai", "").unwrap();
    assert_eq!(KeyStore::open(&file).list(), vec!["aiodos".to_owned()]);
}

#[test]
#[cfg(windows)]
fn valeurs_de_la_version_javascript() {
    let dir = TempDir::new("electron");
    // profil Electron simulé : clé AES-256 protégée par DPAPI dans « Local State »
    let master: Vec<u8> = (0u8..32).map(|i| i.wrapping_mul(37).wrapping_add(11)).collect();
    let mut wrapped = b"DPAPI".to_vec();
    wrapped.extend(dpapi_protect_plain_for_tests(&master).unwrap());
    let local_state = dir.0.join("Local State");
    std::fs::write(&local_state, json!({ "os_crypt": { "encrypted_key": base64_encode(&wrapped) } }).to_string()).unwrap();
    let enc = electron_encrypt_for_tests("sk-ant-ancienne", &master, [7; 12]);
    assert_eq!(&enc[..3], b"v10");
    let file = dir.0.join("ai-keys.json");
    std::fs::write(&file, json!({ "anthropic": format!("enc:{}", base64_encode(&enc)), "mistral": "raw:cle-mistral", "openai": "cle-brute" }).to_string()).unwrap();

    let keys = KeyStore::open_with_electron(&file, Some(&local_state));
    assert_eq!(keys.get("anthropic"), "sk-ant-ancienne");
    assert_eq!(keys.get("mistral"), "cle-mistral");
    assert_eq!(keys.get("openai"), "cle-brute");

    // sans le profil Electron : la valeur chiffrée est illisible, mais gardée telle quelle
    let keys = KeyStore::open(&file);
    assert_eq!(keys.get("anthropic"), "");
    keys.set("gemini", "cle-gemini").unwrap();
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert!(v["anthropic"].as_str().unwrap().starts_with("enc:"));
    assert!(v["mistral"].as_str().unwrap().starts_with("dpapi:"), "réécrite chiffrée");
    assert!(base64_decode(v["gemini"].as_str().unwrap().trim_start_matches("dpapi:")).is_some());
    // et relue avec le profil : tout y est
    let keys = KeyStore::open_with_electron(&file, Some(&local_state));
    assert_eq!((keys.get("anthropic").as_str(), keys.get("gemini").as_str()), ("sk-ant-ancienne", "cle-gemini"));
}
