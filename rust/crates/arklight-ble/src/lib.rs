//! Bluetooth LE d'ARKLIGHT : le PC se fait passer pour un appareil Bluetooth (une AsteraBox, ou
//! l'appareil de test « écho ») auprès des applis des téléphones. Portage du pont Windows
//! `ble/bridge.py` (Python WinRT), des émulateurs `server/ble/emulators/` et de la logique de
//! `server/ble/bridge-hub.js` (journal des échanges, choix de l'émulateur, abonnements).
//!
//! - [`Peripheral`] : publie une table GATT ([`Profile`]) et signale chaque écriture, lecture,
//!   abonnement et changement d'annonce par un [`Event`] ; envoie les notifications demandées.
//!   Sous Windows : API WinRT `GattServiceProvider` ; ailleurs : [`Error::Unsupported`].
//! - [`adapter_info`] : état de l'adaptateur Bluetooth, en lecture seule (ne touche jamais aux
//!   réglages de Windows : si la radio est coupée, c'est à l'utilisateur de l'activer).
//! - [`Emulator`] : logique d'un appareil (profil + réponses) ; [`TestEcho`] et [`AsteraBox`]
//!   (en attente des captures d'une vraie boîte).
//! - [`Hub`] : ce qu'utilise l'application de bureau : liste et choix de l'émulateur, démarrage et
//!   arrêt de l'annonce, état ([`HubStatus`]), journal des 300 derniers échanges ([`TrafficEntry`]).
//!
//! Utilisation par l'application :
//! - au démarrage : `let hub = Hub::new();` puis `hub.set_listener(|ev| …)` (réveille l'interface ;
//!   appelé depuis un autre fil) ;
//! - liste : `hub.status().emulators` ; choix : `hub.set_emulator("test")` ;
//! - bouton « Démarrer » : `hub.start()` (bloquant, quelques centaines de millisecondes : à appeler
//!   hors du fil de l'interface) ; « Arrêter » : `hub.stop()` ;
//! - affichage : `hub.status()`, `hub.traffic()` / `hub.traffic_since(seq)`, `hub.messages()`.

mod error;
pub mod emulator;
pub mod hex;
pub mod hub;
pub mod peripheral;
pub mod profile;
mod uuid;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
use stub as imp;
#[cfg(windows)]
mod winrt;
#[cfg(windows)]
use winrt as imp;

pub use emulator::{Api, AsteraBox, Emulator, TestEcho, builtin};
pub use error::Error;
pub use hub::{Direction, EmulatorInfo, Hub, HubEvent, HubStatus, Level, LogLine, MAX_MESSAGES, MAX_TRAFFIC, Op, TrafficEntry};
pub use peripheral::{
    AdapterInfo, AdvertisingEvent, Client, Event, Peripheral, READ_TIMEOUT, ReadEvent, ReadReply, SUPPORTED, SubscribeEvent, WriteEvent, adapter_info,
    adv_status_text, bluetooth_error_text, radio_text,
};
pub use profile::{Characteristic, Prop, Profile, Service};
pub use uuid::{normalize_uuid, parse_uuid};
