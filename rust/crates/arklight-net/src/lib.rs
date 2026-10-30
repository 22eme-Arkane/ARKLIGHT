//! Réseau d'ARKLIGHT, partagé par l'application PC et l'app mobile (portage de `server/dmx/`) :
//! réception Art-Net 4 (ArtDmx, réponse aux ArtPoll pour être découvert comme un vrai node) et
//! sACN (E1.31, multicast et unicast), fusion des sources par univers (priorité sACN, puis la plus
//! haute valeur entre sources de même priorité ; dernier état conservé quand tout se tait).
//! RDM (portage de `shared/rdm.js`) : les projecteurs de la scène répondent aux applis (Luminair,
//! consoles) par Art-Net, qui peuvent les découvrir, lire leurs modes et canaux, les ré-adresser.

pub mod artnet;
pub mod rdm;
mod rdm_ascii;
pub mod receiver;
pub mod sacn;
pub mod universes;

pub use rdm::{RdmEvent, RdmResponder, RdmScene};
pub use receiver::{Receiver, Status};
pub use universes::UniverseStore;
