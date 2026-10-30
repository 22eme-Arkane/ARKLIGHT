//! Profil GATT publié par le PC : services, caractéristiques et valeurs initiales. Même forme JSON
//! que les profils des émulateurs JS (`server/ble/emulators/`), octets en hexadécimal :
//!
//! ```json
//! { "name": "ARKLIGHT Test", "askCoreOnRead": false,
//!   "services": [{ "uuid": "a7c1a000-…", "advertise": true, "serviceData": "0102",
//!     "characteristics": [{ "uuid": "a7c1a002-…", "props": ["read", "notify"],
//!                           "value": "41524b4c49474854", "description": "Réponse" }] }] }
//! ```

use crate::Error;
use crate::uuid::normalize_uuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// nom affiché (journal, état)
    #[serde(default)]
    pub name: String,
    /// lectures : demander la valeur à l'émulateur (`Emulator::on_read`) au lieu de servir la valeur
    /// en cache ; sans réponse sous `READ_TIMEOUT`, la valeur en cache est servie
    #[serde(default, skip_serializing_if = "is_false")]
    pub ask_core_on_read: bool,
    #[serde(default)]
    pub services: Vec<Service>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    pub uuid: String,
    /// service annoncé (visible des téléphones qui cherchent des appareils) ; vrai par défaut
    #[serde(default = "yes")]
    pub advertise: bool,
    /// données de service jointes à l'annonce
    #[serde(default, with = "crate::hex::option", skip_serializing_if = "Option::is_none")]
    pub service_data: Option<Vec<u8>>,
    #[serde(default)]
    pub characteristics: Vec<Characteristic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Characteristic {
    pub uuid: String,
    #[serde(default)]
    pub props: Vec<Prop>,
    /// valeur renvoyée aux lectures (jusqu'à `Peripheral::set_value`)
    #[serde(default, with = "crate::hex::option", skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<u8>>,
    /// description lisible (descripteur « User Description »)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Propriété d'une caractéristique (noms des profils JS : `read`, `write`, `writeNoResp`,
/// `notify`, `indicate`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Prop {
    Read,
    Write,
    WriteNoResp,
    Notify,
    Indicate,
}

impl Prop {
    /// Bit de la propriété dans la déclaration GATT (Bluetooth Core, vol. 3 partie G § 3.3.1.1 ;
    /// mêmes valeurs que `GattCharacteristicProperties` de Windows).
    pub const fn flag(self) -> u32 {
        match self {
            Prop::Read => 0x02,
            Prop::WriteNoResp => 0x04,
            Prop::Write => 0x08,
            Prop::Notify => 0x10,
            Prop::Indicate => 0x20,
        }
    }

    /// Bits de toutes les propriétés.
    pub fn flags(props: &[Prop]) -> u32 {
        props.iter().fold(0, |acc, p| acc | p.flag())
    }
}

fn yes() -> bool {
    true
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl Profile {
    /// Lit un profil JSON (forme des émulateurs JS) et vérifie ses UUID.
    pub fn from_json(text: &str) -> Result<Profile, Error> {
        let profile: Profile = serde_json::from_str(text).map_err(|e| Error::InvalidProfile(e.to_string()))?;
        profile.validate()?;
        Ok(profile)
    }

    /// Vérifie que tous les UUID sont lisibles (avant de toucher au Bluetooth).
    pub fn validate(&self) -> Result<(), Error> {
        for svc in &self.services {
            normalize_uuid(&svc.uuid)?;
            for ch in &svc.characteristics {
                normalize_uuid(&ch.uuid)?;
            }
        }
        Ok(())
    }

    /// Caractéristique du profil par UUID (toute forme acceptée par `normalize_uuid`).
    pub fn characteristic(&self, uuid: &str) -> Option<&Characteristic> {
        let wanted = normalize_uuid(uuid).ok()?;
        self.services.iter().flat_map(|s| &s.characteristics).find(|c| normalize_uuid(&c.uuid).is_ok_and(|u| u == wanted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulator::{AsteraBox, Emulator, TestEcho};

    /// Profil de `server/ble/emulators/test-echo.js` (valeur : `Buffer.from('ARKLIGHT').toString('hex')`).
    const JS_TEST_ECHO: &str = r#"{
        "name": "ARKLIGHT Test",
        "services": [
          {
            "uuid": "a7c1a000-0000-4c1e-8000-41524b4c4954",
            "advertise": true,
            "characteristics": [
              { "uuid": "a7c1a001-0000-4c1e-8000-41524b4c4954", "props": ["write", "writeNoResp"], "description": "Commande" },
              { "uuid": "a7c1a002-0000-4c1e-8000-41524b4c4954", "props": ["read", "notify"], "value": "41524b4c49474854", "description": "Réponse" }
            ]
          }
        ]
    }"#;

    /// Profil de `server/ble/emulators/astera-box.js` (vide en attendant les captures).
    const JS_ASTERA_BOX: &str = r#"{ "name": "AsteraBox", "services": [] }"#;

    #[test]
    fn profil_js_test_echo() {
        let p = Profile::from_json(JS_TEST_ECHO).unwrap();
        assert_eq!(p, TestEcho.profile());
        assert_eq!(p.name, "ARKLIGHT Test");
        assert!(!p.ask_core_on_read);
        let svc = &p.services[0];
        assert!(svc.advertise);
        assert_eq!(svc.service_data, None);
        assert_eq!(svc.characteristics[0].props, [Prop::Write, Prop::WriteNoResp]);
        assert_eq!(svc.characteristics[0].value, None);
        assert_eq!(svc.characteristics[1].value.as_deref(), Some(&b"ARKLIGHT"[..]));
        assert_eq!(svc.characteristics[1].description.as_deref(), Some("Réponse"));
        assert_eq!(p.characteristic("A7C1A002-0000-4C1E-8000-41524B4C4954").unwrap().props, [Prop::Read, Prop::Notify]);
    }

    #[test]
    fn profil_js_astera_box() {
        let p = Profile::from_json(JS_ASTERA_BOX).unwrap();
        assert_eq!(p, AsteraBox.profile());
        assert!(p.services.is_empty());
    }

    #[test]
    fn valeurs_par_defaut_et_champs_facultatifs() {
        // profil de test du pont Python (sans description) + champs facultatifs
        let p = Profile::from_json(
            r#"{ "askCoreOnRead": true, "services": [ { "uuid": "fff0", "serviceData": "0102",
                 "characteristics": [ { "uuid": "fff1", "props": ["indicate"], "value": null } ] } ] }"#,
        )
        .unwrap();
        assert_eq!(p.name, "");
        assert!(p.ask_core_on_read);
        assert!(p.services[0].advertise, "advertise vaut vrai par défaut (comme svc.get('advertise', True))");
        assert_eq!(p.services[0].service_data.as_deref(), Some(&[1u8, 2][..]));
        assert_eq!(p.services[0].characteristics[0].value, None);
        assert_eq!(p.services[0].characteristics[0].description, None);
    }

    #[test]
    fn ecriture_json_meme_forme_que_js() {
        let json = serde_json::to_value(TestEcho.profile()).unwrap();
        let expected: serde_json::Value = serde_json::from_str(JS_TEST_ECHO).unwrap();
        assert_eq!(json, expected);
        let json = serde_json::to_value(AsteraBox.profile()).unwrap();
        assert_eq!(json, serde_json::from_str::<serde_json::Value>(JS_ASTERA_BOX).unwrap());
    }

    #[test]
    fn profils_refuses() {
        let bad_prop = r#"{ "services": [ { "uuid": "fff0", "characteristics": [ { "uuid": "fff1", "props": ["broadcast"] } ] } ] }"#;
        assert!(matches!(Profile::from_json(bad_prop), Err(Error::InvalidProfile(_))));
        let bad_hex = r#"{ "services": [ { "uuid": "fff0", "characteristics": [ { "uuid": "fff1", "value": "abc" } ] } ] }"#;
        assert!(matches!(Profile::from_json(bad_hex), Err(Error::InvalidProfile(_))));
        let bad_uuid = r#"{ "services": [ { "uuid": "pas-un-uuid" } ] }"#;
        assert_eq!(Profile::from_json(bad_uuid), Err(Error::InvalidUuid("pas-un-uuid".into())));
        let bad_char_uuid = r#"{ "services": [ { "uuid": "fff0", "characteristics": [ { "uuid": "12345" } ] } ] }"#;
        assert_eq!(Profile::from_json(bad_char_uuid), Err(Error::InvalidUuid("12345".into())));
    }

    #[test]
    fn bits_des_proprietes() {
        assert_eq!(Prop::flags(&[]), 0);
        assert_eq!(Prop::flags(&[Prop::Read, Prop::Notify]), 0x12);
        assert_eq!(Prop::flags(&[Prop::Write, Prop::WriteNoResp]), 0x0c);
        assert_eq!(Prop::flags(&[Prop::Read, Prop::Write, Prop::WriteNoResp, Prop::Notify, Prop::Indicate]), 0x3e);
        assert_eq!(Prop::flags(&[Prop::Read, Prop::Read]), 0x02);
    }
}
