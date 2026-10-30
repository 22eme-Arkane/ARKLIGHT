//! Périphérique Bluetooth LE : le PC publie une table GATT et relaie ce que font les téléphones
//! (portage de la classe `Peripheral` et de `adapter_info` de `ble/bridge.py`).
//!
//! Le périphérique est volontairement « bête » : il publie le [`Profile`] qu'on lui donne, signale
//! chaque écriture, lecture, abonnement et changement d'annonce par un [`Event`], et envoie les
//! notifications qu'on lui demande. Toute la logique du protocole reste dans les émulateurs (voir
//! [`crate::Hub`]).
//!
//! Sous Windows : API WinRT `GattServiceProvider` ; ailleurs, [`Error::Unsupported`].
//! Les événements arrivent sur des fils du système : le rappel doit rester bref (le [`crate::Hub`]
//! les passe à son propre fil).

use crate::profile::Profile;
use crate::uuid::normalize_uuid;
use crate::{Error, imp};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// Bluetooth LE périphérique disponible sur ce système (Windows seulement pour l'instant).
pub const SUPPORTED: bool = cfg!(windows);

/// Attente maximale de la réponse de l'émulateur à une lecture (profil `ask_core_on_read`),
/// comme le pont Python (0,8 s) ; au-delà, la valeur en cache est servie.
pub const READ_TIMEOUT: Duration = Duration::from_millis(800);

// ---------------------------------------------------------------------------------- événements

/// Téléphone abonné aux notifications d'une caractéristique.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    /// identifiant Windows de l'appareil (`BluetoothLE#BluetoothLE…`), si connu
    pub device: Option<String>,
    /// taille maximale d'une notification pour ce téléphone (octets)
    pub max_notification: u16,
}

/// Écriture d'un téléphone dans une caractéristique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteEvent {
    pub service: String,
    pub char: String,
    pub value: Vec<u8>,
    pub offset: u32,
    /// écriture avec accusé (« write request ») ; sinon « write command » (sans réponse)
    pub with_response: bool,
    pub device: Option<String>,
}

/// Lecture d'une caractéristique par un téléphone.
#[derive(Debug, Clone)]
pub struct ReadEvent {
    pub service: String,
    pub char: String,
    pub offset: u32,
    pub device: Option<String>,
    /// réponse à donner si le profil demande `ask_core_on_read` (ignorée sinon)
    pub reply: ReadReply,
}

/// Abonnements d'une caractéristique (liste complète des téléphones abonnés ; vide = plus personne).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeEvent {
    pub service: String,
    pub char: String,
    pub clients: Vec<Client>,
}

/// Changement d'état de l'annonce d'un service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvertisingEvent {
    pub service: String,
    /// état en français : voir [`adv_status_text`]
    pub status: String,
    /// code `BluetoothError` de Windows (0 = aucune erreur), voir [`bluetooth_error_text`]
    pub error: i32,
}

#[derive(Debug, Clone)]
pub enum Event {
    Write(WriteEvent),
    Read(ReadEvent),
    Subscribe(SubscribeEvent),
    Advertising(AdvertisingEvent),
    /// échec dans le traitement d'une demande d'un téléphone (message en français)
    Error(String),
}

/// Réponse à une lecture, donnée par le rappel (tout de suite ou depuis un autre fil, avant
/// [`READ_TIMEOUT`]). Sert seulement si le profil a `ask_core_on_read`.
#[derive(Clone, Default)]
pub struct ReadReply(Arc<(Mutex<ReplySlot>, Condvar)>);

/// Réponse d'une lecture : `None` en attente, `Some(None)` valeur en cache, `Some(Some(v))` valeur.
type ReplySlot = Option<Option<Vec<u8>>>;

impl ReadReply {
    /// Répond avec cette valeur (la lecture renvoie la suite à partir de son décalage).
    pub fn send(&self, value: Vec<u8>) {
        self.answer(Some(value));
    }

    /// Répond avec la valeur en cache (sans attendre la fin du délai).
    pub fn use_cached(&self) {
        self.answer(None);
    }

    fn answer(&self, value: Option<Vec<u8>>) {
        let (slot, ready) = &*self.0;
        let mut slot = lock(slot);
        if slot.is_none() {
            *slot = Some(value);
        }
        ready.notify_all();
    }

    /// Attend la réponse : `Some(Some(v))` valeur donnée, `Some(None)` valeur en cache demandée,
    /// `None` pas de réponse dans le délai.
    #[cfg_attr(not(windows), allow(dead_code))] // sert au pont du système (Windows)
    pub(crate) fn wait(&self, timeout: Duration) -> Option<Option<Vec<u8>>> {
        let (slot, ready) = &*self.0;
        let slot = lock(slot);
        let (mut slot, _) = ready.wait_timeout_while(slot, timeout, |s| s.is_none()).unwrap_or_else(PoisonError::into_inner);
        slot.take()
    }
}

impl fmt::Debug for ReadReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadReply")
    }
}

// ---------------------------------------------------------------------------------- textes

/// État de l'annonce (`GattServiceProviderAdvertisementStatus`) en français, comme `ADV_STATUS`.
pub fn adv_status_text(status: i32) -> String {
    match status {
        0 => "créé".into(),
        1 => "arrêté".into(),
        2 => "en cours".into(),
        3 => "abandonné".into(),
        4 => "démarré sans tous les services".into(),
        other => other.to_string(),
    }
}

/// État de la radio (`RadioState`) en français, comme `RADIO_STATES`.
pub fn radio_text(state: i32) -> String {
    match state {
        0 => "inconnu".into(),
        1 => "activé".into(),
        2 => "désactivé".into(),
        3 => "désactivé (matériel)".into(),
        other => other.to_string(),
    }
}

/// Code `BluetoothError` de Windows → texte (« erreur Bluetooth 6 : non pris en charge »).
pub fn bluetooth_error_text(code: i32) -> String {
    let why = match code {
        0 => "aucune",
        1 => "radio indisponible",
        2 => "ressource déjà utilisée",
        3 => "appareil non connecté",
        4 => "autre erreur",
        5 => "désactivé par une stratégie",
        6 => "non pris en charge",
        7 => "désactivé par l'utilisateur",
        8 => "consentement requis",
        9 => "transport non pris en charge",
        _ => "inconnue",
    };
    format!("erreur Bluetooth {code} : {why}")
}

// ---------------------------------------------------------------------------------- adaptateur

/// État de l'adaptateur Bluetooth (lecture seule : ne modifie jamais les réglages de Windows).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterInfo {
    pub present: bool,
    /// adresse de l'adaptateur, 12 chiffres hexadécimaux (`"A1B2C3D4E5F6"`, comme le pont Python)
    pub address: Option<String>,
    /// l'adaptateur sait jouer le rôle de périphérique (indispensable pour imiter un appareil)
    pub peripheral_role: bool,
    /// annonces étendues (Bluetooth 5) ; `None` si Windows ne sait pas le dire
    pub extended_advertising: Option<bool>,
    /// état de la radio dans Windows : « activé », « désactivé », « désactivé (matériel) », « inconnu »
    pub radio: String,
    pub radio_on: bool,
}

impl AdapterInfo {
    /// Pas d'adaptateur Bluetooth.
    pub fn absent() -> Self {
        AdapterInfo { present: false, address: None, peripheral_role: false, extended_advertising: None, radio: radio_text(0), radio_on: false }
    }
}

/// Lit l'état de l'adaptateur Bluetooth par défaut (bloquant, quelques dizaines de millisecondes).
/// `Ok(AdapterInfo { present: false, .. })` sans adaptateur ; [`Error::Unsupported`] hors Windows.
pub fn adapter_info() -> Result<AdapterInfo, Error> {
    imp::adapter_info()
}

// ---------------------------------------------------------------------------------- périphérique

/// État partagé avec les gestionnaires d'événements du système (inutilisé hors Windows).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct Shared {
    events: Box<dyn Fn(Event) + Send + Sync>,
    /// UUID de caractéristique → valeur servie aux lectures
    values: Mutex<HashMap<String, Vec<u8>>>,
    ask_core_on_read: bool,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl Shared {
    /// Passe un événement au rappel ; une panique du rappel est rattrapée (elle ne doit pas
    /// traverser l'appel du système, ce qui arrêterait l'application).
    pub(crate) fn emit(&self, event: Event) {
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| (self.events)(event)));
    }

    /// Valeur à renvoyer à une lecture déjà signalée par `emit` : la réponse de l'émulateur si le
    /// profil la demande (attendue au plus [`READ_TIMEOUT`]), sinon la valeur en cache ; à partir
    /// de `offset` (vide au-delà de la fin).
    pub(crate) fn read_value(&self, char: &str, reply: &ReadReply, offset: u32) -> Vec<u8> {
        self.read_value_within(char, reply, offset, READ_TIMEOUT)
    }

    fn read_value_within(&self, char: &str, reply: &ReadReply, offset: u32, timeout: Duration) -> Vec<u8> {
        let answered = if self.ask_core_on_read {
            match reply.wait(timeout) {
                Some(answer) => answer,
                None => {
                    self.emit(Event::Error(format!("lecture {char} : pas de réponse de l'émulateur, valeur en cache")));
                    None
                }
            }
        } else {
            None
        };
        let value = answered.unwrap_or_else(|| lock(&self.values).get(char).cloned().unwrap_or_default());
        value.get(offset as usize..).map(<[u8]>::to_vec).unwrap_or_default()
    }
}

/// Table GATT publiée ; l'annonce s'arrête avec [`Peripheral::stop`] ou quand on le lâche.
pub struct Peripheral {
    shared: Arc<Shared>,
    backend: Mutex<Option<imp::Backend>>,
    name: String,
    services: usize,
}

impl Peripheral {
    /// Publie le profil et démarre l'annonce (le PC devient visible des téléphones, sauf services
    /// avec `advertise: false`). Bloquant (quelques centaines de millisecondes) ; les appels au
    /// système se font sur un fil de travail, l'appel est donc possible depuis n'importe quel fil.
    ///
    /// `events` reçoit chaque [`Event`], depuis des fils du système : il doit rester bref.
    pub fn start(profile: &Profile, events: impl Fn(Event) + Send + Sync + 'static) -> Result<Peripheral, Error> {
        profile.validate()?;
        let mut values = HashMap::new();
        for ch in profile.services.iter().flat_map(|s| &s.characteristics) {
            if let Some(v) = &ch.value {
                values.insert(normalize_uuid(&ch.uuid)?, v.clone());
            }
        }
        let shared = Arc::new(Shared { events: Box::new(events), values: Mutex::new(values), ask_core_on_read: profile.ask_core_on_read });
        let backend = imp::Backend::start(profile, &shared)?;
        let services = backend.services();
        Ok(Peripheral { shared, backend: Mutex::new(Some(backend)), name: profile.name.clone(), services })
    }

    /// Nom du profil publié.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Nombre de services publiés (0 pour un profil vide : rien n'est annoncé).
    pub fn services(&self) -> usize {
        self.services
    }

    pub fn is_running(&self) -> bool {
        lock(&self.backend).is_some()
    }

    /// Envoie une notification aux téléphones abonnés ; renvoie leur nombre. Bloquant jusqu'à
    /// l'envoi (quelques millisecondes).
    pub fn notify(&self, char_uuid: &str, data: &[u8]) -> Result<usize, Error> {
        let uuid = normalize_uuid(char_uuid)?;
        let backend = lock(&self.backend);
        backend.as_ref().ok_or(Error::NotRunning)?.notify(&uuid, data)
    }

    /// Valeur renvoyée aux prochaines lectures de la caractéristique.
    pub fn set_value(&self, char_uuid: &str, data: &[u8]) -> Result<(), Error> {
        let uuid = normalize_uuid(char_uuid)?;
        lock(&self.shared.values).insert(uuid, data.to_vec());
        Ok(())
    }

    /// Arrête l'annonce et retire la table GATT (sans effet si déjà arrêté).
    pub fn stop(&self) {
        // retiré sous verrou, arrêté hors verrou (à la fin de la fonction)
        let _stopped = lock(&self.backend).take();
    }
}

impl Drop for Peripheral {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Verrou qui survit à une panique d'un autre fil (l'état reste utilisable).
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn shared(ask: bool, events: impl Fn(Event) + Send + Sync + 'static) -> Shared {
        let values = HashMap::from([("c".to_string(), b"ARKLIGHT".to_vec())]);
        Shared { events: Box::new(events), values: Mutex::new(values), ask_core_on_read: ask }
    }

    #[test]
    fn textes() {
        assert_eq!(adv_status_text(2), "en cours");
        assert_eq!(adv_status_text(4), "démarré sans tous les services");
        assert_eq!(adv_status_text(9), "9");
        assert_eq!(radio_text(1), "activé");
        assert_eq!(radio_text(3), "désactivé (matériel)");
        assert_eq!(radio_text(0), "inconnu");
        assert_eq!(bluetooth_error_text(6), "erreur Bluetooth 6 : non pris en charge");
        assert_eq!(AdapterInfo::absent().radio, "inconnu");
    }

    #[test]
    fn reponse_de_lecture() {
        let reply = ReadReply::default();
        let other = reply.clone();
        let t = std::thread::spawn(move || other.send(b"abc".to_vec()));
        assert_eq!(reply.wait(Duration::from_secs(5)), Some(Some(b"abc".to_vec())));
        t.join().unwrap();

        let reply = ReadReply::default();
        reply.use_cached();
        reply.send(b"trop tard".to_vec()); // la première réponse compte
        assert_eq!(reply.wait(Duration::ZERO), Some(None));

        assert_eq!(ReadReply::default().wait(Duration::from_millis(10)), None);
    }

    #[test]
    fn valeur_lue() {
        // sans ask_core_on_read : valeur en cache, réponse ignorée
        let s = shared(false, |_| {});
        let reply = ReadReply::default();
        reply.send(b"ignoree".to_vec());
        assert_eq!(s.read_value("c", &reply, 0), b"ARKLIGHT");
        assert_eq!(s.read_value("c", &reply, 3), b"LIGHT");
        assert_eq!(s.read_value("c", &reply, 8), b"");
        assert_eq!(s.read_value("c", &reply, 50), b"", "décalage au-delà de la fin → vide (comme value[offset:])");
        assert_eq!(s.read_value("inconnue", &reply, 0), b"");

        // avec ask_core_on_read : réponse de l'émulateur, ou cache sur demande, ou cache après délai
        let errors = Arc::new(AtomicUsize::new(0));
        let counter = errors.clone();
        let s = shared(true, move |e| {
            if matches!(e, Event::Error(_)) {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        let reply = ReadReply::default();
        reply.send(b"0123456789".to_vec());
        assert_eq!(s.read_value("c", &reply, 4), b"456789");
        let reply = ReadReply::default();
        reply.use_cached();
        assert_eq!(s.read_value("c", &reply, 0), b"ARKLIGHT");
        assert_eq!(errors.load(Ordering::SeqCst), 0);
        assert_eq!(s.read_value_within("c", &ReadReply::default(), 0, Duration::from_millis(10)), b"ARKLIGHT");
        assert_eq!(errors.load(Ordering::SeqCst), 1, "délai dépassé : signalé");
    }

    #[test]
    fn rappel_qui_panique() {
        let s = shared(false, |_| panic!("rappel en panne"));
        s.emit(Event::Error("x".into())); // ne remonte pas
    }

    #[test]
    fn profil_invalide_refuse_avant_le_bluetooth() {
        // la vérification des UUID précède tout appel au système : aucun Bluetooth touché ici
        let profile = Profile::from_json(r#"{ "services": [] }"#).unwrap();
        let mut bad = profile.clone();
        bad.services.push(crate::Service { uuid: "pas-un-uuid".into(), advertise: true, service_data: None, characteristics: vec![] });
        assert_eq!(Peripheral::start(&bad, |_| {}).err(), Some(Error::InvalidUuid("pas-un-uuid".into())));
    }

    /// Lit l'adaptateur réel (lecture seule) : `cargo test -p arklight-ble -- --ignored --nocapture`.
    #[test]
    #[ignore = "lit l'adaptateur Bluetooth réel de l'ordinateur"]
    fn adaptateur_reel() {
        let info = adapter_info();
        println!("adapter_info() = {info:#?}");
        if SUPPORTED {
            assert!(info.is_ok(), "{info:?}");
        } else {
            assert_eq!(info, Err(Error::Unsupported));
        }
    }
}
