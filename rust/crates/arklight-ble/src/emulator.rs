//! Émulateurs : l'appareil que le PC fait croire aux applis des téléphones (portage de
//! `server/ble/emulators/`). Un émulateur donne le profil GATT à publier et réagit aux écritures,
//! lectures et abonnements des téléphones par l'[`Api`] du [`crate::Hub`] : notifications, valeurs
//! servies aux lectures, messages du journal.

pub mod astera_box;
pub mod test_echo;

pub use astera_box::AsteraBox;
pub use test_echo::TestEcho;

use crate::peripheral::{ReadEvent, SubscribeEvent, WriteEvent};
use crate::profile::Profile;

/// Ce qu'un émulateur peut faire en réponse aux téléphones (l'objet `api` des émulateurs JS).
pub trait Api {
    /// Envoie une notification aux téléphones abonnés à la caractéristique (notée dans le journal
    /// des échanges ; sans effet si le Bluetooth est arrêté).
    fn notify(&mut self, char_uuid: &str, bytes: &[u8]);
    /// Valeur renvoyée aux prochaines lectures de la caractéristique.
    fn set_value(&mut self, char_uuid: &str, bytes: &[u8]);
    /// Message pour le journal de l'application (préfixé du nom de l'émulateur).
    fn log(&mut self, text: &str);
}

/// Appareil imité. Les méthodes `on_*` sont appelées sur le fil du [`crate::Hub`], une à la fois.
pub trait Emulator: Send {
    /// identifiant stable (réglages, choix dans l'interface) : `"astera"`, `"test"`
    fn id(&self) -> &str;
    /// nom affiché
    fn name(&self) -> &str;
    /// explication affichée sous le choix de l'émulateur
    fn note(&self) -> &str {
        ""
    }
    /// faux tant que le protocole de l'appareil n'est pas connu (émulateur incomplet)
    fn ready(&self) -> bool {
        true
    }
    /// table GATT à publier
    fn profile(&self) -> Profile;
    /// écriture d'un téléphone
    fn on_write(&mut self, _event: &WriteEvent, _api: &mut dyn Api) {}
    /// lecture d'un téléphone ; la valeur rendue ne sert que si le profil a `ask_core_on_read`
    /// (`None` : valeur en cache)
    fn on_read(&mut self, _event: &ReadEvent, _api: &mut dyn Api) -> Option<Vec<u8>> {
        None
    }
    /// abonnements d'une caractéristique changés
    fn on_subscribe(&mut self, _event: &SubscribeEvent, _api: &mut dyn Api) {}
}

/// Émulateurs fournis, dans l'ordre de `server/index.js` (le premier est choisi par défaut).
pub fn builtin() -> Vec<Box<dyn Emulator>> {
    vec![Box::new(AsteraBox), Box::new(TestEcho)]
}

/// Api factice pour les tests des émulateurs : retient tout ce qui est demandé.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct FakeApi {
    pub notified: Vec<(String, Vec<u8>)>,
    pub values: Vec<(String, Vec<u8>)>,
    pub logs: Vec<String>,
}

#[cfg(test)]
impl Api for FakeApi {
    fn notify(&mut self, char_uuid: &str, bytes: &[u8]) {
        self.notified.push((char_uuid.to_string(), bytes.to_vec()));
    }
    fn set_value(&mut self, char_uuid: &str, bytes: &[u8]) {
        self.values.push((char_uuid.to_string(), bytes.to_vec()));
    }
    fn log(&mut self, text: &str) {
        self.logs.push(text.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emulateurs_fournis() {
        let all = builtin();
        let ids: Vec<&str> = all.iter().map(|e| e.id()).collect();
        assert_eq!(ids, ["astera", "test"]);
        assert!(!all[0].ready());
        assert!(all[1].ready());
        for e in &all {
            e.profile().validate().unwrap();
        }
    }
}
