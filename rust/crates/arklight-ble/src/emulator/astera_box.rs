//! Émulateur AsteraBox — EN ATTENTE DES CAPTURES (portage de `server/ble/emulators/astera-box.js`).
//!
//! Le protocole Bluetooth entre l'AsteraApp et l'AsteraBox n'est pas public. Ce module sera rempli
//! à partir des captures (journal HCI Android analysé par tools/btsnoop/analyze.mjs) :
//!   1. `profile` : la table GATT exacte de la vraie boîte (UUID, propriétés, valeurs lues)
//!   2. `on_write` : décodage des commandes de l'appli → modification des projecteurs virtuels
//!   3. réponses / notifications attendues par l'appli (liste des lampes, état, batterie...)

use super::{Api, Emulator};
use crate::hex;
use crate::peripheral::WriteEvent;
use crate::profile::Profile;

#[derive(Debug, Clone, Copy, Default)]
pub struct AsteraBox;

impl Emulator for AsteraBox {
    fn id(&self) -> &str {
        "astera"
    }

    fn name(&self) -> &str {
        "AsteraBox (à compléter)"
    }

    fn note(&self) -> &str {
        "Profil inconnu tant que les échanges avec une vraie AsteraBox n’ont pas été capturés (voir docs/CAPTURE_BLUETOOTH.md)."
    }

    fn ready(&self) -> bool {
        false
    }

    fn profile(&self) -> Profile {
        Profile { name: "AsteraBox".into(), ask_core_on_read: false, services: Vec::new() }
    }

    fn on_write(&mut self, event: &WriteEvent, api: &mut dyn Api) {
        api.log(&format!("commande non décodée sur {} : {}", event.char, hex::encode(&event.value)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emulator::FakeApi;

    #[test]
    fn en_attente_des_captures() {
        let mut box_ = AsteraBox;
        assert!(!box_.ready());
        assert!(box_.profile().services.is_empty());
        let mut api = FakeApi::default();
        let event = WriteEvent { service: "s".into(), char: "c".into(), value: vec![0xab, 0x01], offset: 0, with_response: false, device: None };
        box_.on_write(&event, &mut api);
        assert_eq!(api.logs, ["commande non décodée sur c : ab01"]);
        assert!(api.notified.is_empty());
    }
}
