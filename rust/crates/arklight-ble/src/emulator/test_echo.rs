//! Émulateur de test : vérifie toute la chaîne téléphone → Bluetooth → PC avec une appli générique
//! (nRF Connect, LightBlue). Chaque écriture est renvoyée en notification préfixée par « ECHO: ».
//! Portage de `server/ble/emulators/test-echo.js`.

use super::{Api, Emulator};
use crate::hex;
use crate::peripheral::WriteEvent;
use crate::profile::{Characteristic, Prop, Profile, Service};

pub const SERVICE: &str = "a7c1a000-0000-4c1e-8000-41524b4c4954";
/// caractéristique où le téléphone écrit (« Commande »)
pub const RX: &str = "a7c1a001-0000-4c1e-8000-41524b4c4954";
/// caractéristique des réponses, lue ou notifiée (« Réponse »)
pub const TX: &str = "a7c1a002-0000-4c1e-8000-41524b4c4954";

#[derive(Debug, Clone, Copy, Default)]
pub struct TestEcho;

impl Emulator for TestEcho {
    fn id(&self) -> &str {
        "test"
    }

    fn name(&self) -> &str {
        "Test (écho)"
    }

    fn note(&self) -> &str {
        "Pour vérifier la connexion avec nRF Connect ou LightBlue : écris dans la caractéristique …a001, la réponse arrive sur …a002."
    }

    fn profile(&self) -> Profile {
        Profile {
            name: "ARKLIGHT Test".into(),
            ask_core_on_read: false,
            services: vec![Service {
                uuid: SERVICE.into(),
                advertise: true,
                service_data: None,
                characteristics: vec![
                    Characteristic { uuid: RX.into(), props: vec![Prop::Write, Prop::WriteNoResp], value: None, description: Some("Commande".into()) },
                    Characteristic {
                        uuid: TX.into(),
                        props: vec![Prop::Read, Prop::Notify],
                        value: Some(b"ARKLIGHT".to_vec()),
                        description: Some("Réponse".into()),
                    },
                ],
            }],
        }
    }

    fn on_write(&mut self, event: &WriteEvent, api: &mut dyn Api) {
        // texte lisible : caractères hors ASCII imprimable remplacés par « . »
        let text: String = String::from_utf8_lossy(&event.value).chars().map(|c| if (' '..='~').contains(&c) { c } else { '.' }).collect();
        api.log(&format!("reçu {} (« {text} »)", hex::encode(&event.value)));
        let mut answer = b"ECHO:".to_vec();
        answer.extend_from_slice(&event.value);
        api.notify(TX, &answer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulator::FakeApi;

    fn write(value: &[u8]) -> WriteEvent {
        WriteEvent { service: SERVICE.into(), char: RX.into(), value: value.to_vec(), offset: 0, with_response: true, device: None }
    }

    #[test]
    fn echo() {
        let mut api = FakeApi::default();
        TestEcho.on_write(&write(b"hi"), &mut api);
        assert_eq!(api.notified, [(TX.to_string(), b"ECHO:hi".to_vec())]);
        assert_eq!(api.logs, ["reçu 6869 (« hi »)"]);
        assert!(api.values.is_empty());
    }

    #[test]
    fn echo_octets_non_imprimables() {
        let mut api = FakeApi::default();
        TestEcho.on_write(&write(&[0x01, b'A', 0xff, b'~', 0x7f]), &mut api);
        assert_eq!(api.notified[0].1, [b'E', b'C', b'H', b'O', b':', 0x01, b'A', 0xff, b'~', 0x7f]);
        assert_eq!(api.logs, ["reçu 0141ff7e7f (« .A.~. »)"]);
        // UTF-8 : un caractère accentué → un seul « . » (comme le remplacement JS)
        let mut api = FakeApi::default();
        TestEcho.on_write(&write("é!".as_bytes()), &mut api);
        assert_eq!(api.logs, ["reçu c3a921 (« .! »)"]);
    }

    #[test]
    fn echo_vide() {
        let mut api = FakeApi::default();
        TestEcho.on_write(&write(b""), &mut api);
        assert_eq!(api.notified, [(TX.to_string(), b"ECHO:".to_vec())]);
        assert_eq!(api.logs, ["reçu  («  »)"]);
    }
}
