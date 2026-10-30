//! Erreurs du Bluetooth (messages en français, prêts à afficher).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// pas de Bluetooth LE périphérique sur ce système (autre que Windows)
    Unsupported,
    /// aucun adaptateur Bluetooth dans l'ordinateur
    NoAdapter,
    /// radio Bluetooth coupée dans Windows (état : « désactivé », « désactivé (matériel) »…)
    RadioOff(String),
    /// UUID illisible dans un profil ou une commande
    InvalidUuid(String),
    /// profil JSON illisible
    InvalidProfile(String),
    /// notification vers une caractéristique absente du profil publié
    UnknownCharacteristic(String),
    /// identifiant d'émulateur inconnu
    UnknownEmulator(String),
    /// périphérique arrêté
    NotRunning,
    /// erreur de l'API Bluetooth de Windows
    Bluetooth(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unsupported => write!(f, "Bluetooth indisponible sur ce système"),
            Error::NoAdapter => write!(f, "aucun adaptateur Bluetooth trouvé"),
            Error::RadioOff(radio) => write!(f, "Bluetooth {radio} dans Windows : active-le (Paramètres > Bluetooth et appareils)"),
            Error::InvalidUuid(uuid) => write!(f, "UUID invalide : « {uuid} »"),
            Error::InvalidProfile(why) => write!(f, "profil Bluetooth invalide : {why}"),
            Error::UnknownCharacteristic(uuid) => write!(f, "caractéristique inconnue : {uuid}"),
            Error::UnknownEmulator(id) => write!(f, "Émulateur inconnu : {id}"),
            Error::NotRunning => write!(f, "le Bluetooth n'est pas démarré"),
            Error::Bluetooth(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}
