//! Hub Bluetooth de l'application de bureau (portage de `server/ble/bridge-hub.js`) : émulateur
//! actif, périphérique publié, abonnements des téléphones, journal des 300 derniers échanges.
//!
//! Les événements du périphérique arrivent sur des fils du système ; le hub les passe à son propre
//! fil (« arklight-ble »), qui appelle l'émulateur une demande à la fois, dans l'ordre d'arrivée.
//! Le rappel de [`Hub::set_listener`] est appelé depuis ce fil ou depuis celui qui a appelé
//! `start` / `stop` / `set_emulator`, toujours hors des verrous : il peut relire `status()`.

use crate::emulator::{Api, Emulator, builtin};
use crate::peripheral::{AdapterInfo, Client, Event, Peripheral, adapter_info, bluetooth_error_text, lock};
use crate::profile::Profile;
use crate::{Error, hex};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::panic::AssertUnwindSafe;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

/// Taille du journal des échanges (comme `MAX_TRAFFIC` en JS).
pub const MAX_TRAFFIC: usize = 300;
/// Messages gardés pour l'affichage (`Hub::messages`).
pub const MAX_MESSAGES: usize = 200;

/// Sens d'un échange : du téléphone vers le PC (`in`) ou l'inverse (`out`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    In,
    Out,
}

/// Nature d'un échange (noms du journal JS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Op {
    /// écriture avec accusé
    WriteReq,
    /// écriture sans réponse
    WriteCmd,
    Read,
    Notify,
}

impl Op {
    pub fn as_str(self) -> &'static str {
        match self {
            Op::WriteReq => "WRITE_REQ",
            Op::WriteCmd => "WRITE_CMD",
            Op::Read => "READ",
            Op::Notify => "NOTIFY",
        }
    }
}

/// Échange noté au journal (forme JSON du journal JS : `at`, `dir`, `op`, `char`, `hex`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrafficEntry {
    /// numéro d'ordre croissant (pour `Hub::traffic_since`)
    pub seq: u64,
    /// heure en millisecondes depuis 1970 (comme `Date.now()`)
    pub at: u64,
    pub dir: Direction,
    pub op: Op,
    pub char: String,
    #[serde(rename = "hex", serialize_with = "crate::hex::serialize_bytes")]
    pub bytes: Vec<u8>,
}

impl TrafficEntry {
    pub fn hex(&self) -> String {
        hex::encode(&self.bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Info,
    Warn,
    Error,
}

/// Message pour le journal de l'application (en français, déjà préfixé « Bluetooth : » ou du nom
/// de l'émulateur).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LogLine {
    pub at: u64,
    pub level: Level,
    pub text: String,
}

/// Ce que reçoit le rappel de [`Hub::set_listener`].
#[derive(Debug, Clone)]
pub enum HubEvent {
    /// l'état a changé : relire `Hub::status()`
    Status,
    /// nouvel échange au journal
    Traffic(TrafficEntry),
    /// message pour le journal de l'application
    Log(LogLine),
}

/// Émulateur proposé dans l'interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmulatorInfo {
    pub id: String,
    pub name: String,
    /// faux : émulateur incomplet (protocole pas encore capturé)
    pub ready: bool,
    pub note: String,
}

/// État affiché par l'interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubStatus {
    /// identifiant de l'émulateur actif
    pub emulator: String,
    pub emulators: Vec<EmulatorInfo>,
    /// table GATT publiée (annonce démarrée)
    pub running: bool,
    /// dernier état lu de l'adaptateur (au démarrage ou par `refresh_adapter`)
    pub adapter: Option<AdapterInfo>,
    /// nom du profil publié
    pub published: Option<String>,
    /// nombre de services publiés (0 : profil vide, rien n'est annoncé)
    pub services: usize,
    /// UUID de service → état de son annonce (« en cours », « arrêté »…)
    pub advertising: BTreeMap<String, String>,
    /// téléphones abonnés à au moins une caractéristique
    pub phones: Vec<String>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ---------------------------------------------------------------------------------- journaux

/// Journal des échanges (300 derniers) et des messages, et événements à passer au rappel.
#[derive(Default)]
struct Journal {
    traffic: VecDeque<TrafficEntry>,
    next_seq: u64,
    messages: VecDeque<LogLine>,
    outbox: Vec<HubEvent>,
}

impl Journal {
    fn record(&mut self, dir: Direction, op: Op, char: &str, bytes: &[u8]) {
        let entry = TrafficEntry { seq: self.next_seq, at: now_ms(), dir, op, char: char.to_string(), bytes: bytes.to_vec() };
        self.next_seq += 1;
        self.traffic.push_back(entry.clone());
        while self.traffic.len() > MAX_TRAFFIC {
            self.traffic.pop_front();
        }
        self.outbox.push(HubEvent::Traffic(entry));
    }

    fn log(&mut self, level: Level, text: String) {
        let line = LogLine { at: now_ms(), level, text };
        self.messages.push_back(line.clone());
        while self.messages.len() > MAX_MESSAGES {
            self.messages.pop_front();
        }
        self.outbox.push(HubEvent::Log(line));
    }

    fn status_changed(&mut self) {
        self.outbox.push(HubEvent::Status);
    }
}

/// [`Api`] donnée à l'émulateur pendant le traitement d'une demande.
struct HubApi<'a> {
    name: String,
    peripheral: Option<&'a Peripheral>,
    journal: &'a mut Journal,
}

impl Api for HubApi<'_> {
    fn notify(&mut self, char_uuid: &str, bytes: &[u8]) {
        self.journal.record(Direction::Out, Op::Notify, char_uuid, bytes);
        if let Some(p) = self.peripheral
            && let Err(e) = p.notify(char_uuid, bytes)
        {
            self.journal.log(Level::Error, format!("Bluetooth : notification {char_uuid} : {e}"));
        }
    }

    fn set_value(&mut self, char_uuid: &str, bytes: &[u8]) {
        if let Some(p) = self.peripheral
            && let Err(e) = p.set_value(char_uuid, bytes)
        {
            self.journal.log(Level::Error, format!("Bluetooth : valeur {char_uuid} : {e}"));
        }
    }

    fn log(&mut self, text: &str) {
        self.journal.log(Level::Info, format!("{} : {text}", self.name));
    }
}

// ---------------------------------------------------------------------------------- état

struct State {
    emulators: Vec<Box<dyn Emulator>>,
    active: usize,
    /// change à chaque démarrage / arrêt : les événements d'un ancien périphérique sont ignorés
    generation: u64,
    peripheral: Option<Peripheral>,
    adapter: Option<AdapterInfo>,
    published: Option<String>,
    services: usize,
    advertising: BTreeMap<String, String>,
    /// UUID de caractéristique → téléphones abonnés
    subscriptions: BTreeMap<String, Vec<Client>>,
    journal: Journal,
}

impl State {
    fn emulator(&self) -> &dyn Emulator {
        self.emulators[self.active].as_ref()
    }

    fn clear_session(&mut self) {
        self.published = None;
        self.services = 0;
        self.advertising.clear();
        self.subscriptions.clear();
    }

    fn with_emulator<R>(&mut self, f: impl FnOnce(&mut dyn Emulator, &mut dyn Api) -> R) -> R {
        let State { emulators, active, peripheral, journal, .. } = self;
        let emulator = emulators[*active].as_mut();
        let mut api = HubApi { name: emulator.name().to_string(), peripheral: peripheral.as_ref(), journal };
        f(emulator, &mut api)
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Write(w) => {
                let op = if w.with_response { Op::WriteReq } else { Op::WriteCmd };
                self.journal.record(Direction::In, op, &w.char, &w.value);
                self.with_emulator(|emu, api| emu.on_write(&w, api));
            }
            Event::Read(r) => {
                self.journal.record(Direction::In, Op::Read, &r.char, &[]);
                match self.with_emulator(|emu, api| emu.on_read(&r, api)) {
                    Some(value) => r.reply.send(value),
                    None => r.reply.use_cached(),
                }
            }
            Event::Subscribe(sub) => {
                let n = sub.clients.len();
                let what = if n > 0 { format!("{n} téléphone(s) abonné(s)") } else { "désabonnement".to_string() };
                self.journal.log(Level::Info, format!("Bluetooth : {what} à {}", sub.char));
                if n > 0 {
                    self.subscriptions.insert(sub.char.clone(), sub.clients.clone());
                } else {
                    self.subscriptions.remove(&sub.char);
                }
                self.with_emulator(|emu, api| emu.on_subscribe(&sub, api));
            }
            Event::Advertising(a) => {
                if a.error != 0 {
                    self.journal.log(Level::Warn, format!("Bluetooth : annonce du service {} {} ({})", a.service, a.status, bluetooth_error_text(a.error)));
                }
                self.advertising.insert(a.service, a.status);
            }
            Event::Error(message) => self.journal.log(Level::Error, format!("Bluetooth : {message}")),
        }
        self.journal.status_changed();
    }
}

type Listener = Arc<dyn Fn(&HubEvent) + Send + Sync>;

struct HubShared {
    state: Mutex<State>,
    listener: Mutex<Option<Listener>>,
}

impl HubShared {
    fn lock(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }

    /// Traite un événement du périphérique de la génération `generation`.
    fn process(&self, generation: u64, event: Event) {
        {
            let mut s = self.lock();
            if s.generation != generation {
                // ancien périphérique : ne pas faire attendre une lecture
                if let Event::Read(r) = &event {
                    r.reply.use_cached();
                }
                return;
            }
            s.handle(event);
        }
        self.flush();
    }

    /// Passe les événements en attente au rappel, hors verrou.
    fn flush(&self) {
        let events = std::mem::take(&mut self.lock().journal.outbox);
        let listener = lock(&self.listener).clone();
        if let Some(listener) = listener {
            for event in &events {
                let _ = std::panic::catch_unwind(AssertUnwindSafe(|| listener(event)));
            }
        }
    }
}

enum Msg {
    Event(u64, Event),
    Quit,
}

// ---------------------------------------------------------------------------------- hub

/// Bluetooth de l'application : émulateur actif, annonce, journal. Arrête l'annonce quand on le lâche.
pub struct Hub {
    shared: Arc<HubShared>,
    tx: Sender<Msg>,
    worker: Option<JoinHandle<()>>,
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl Hub {
    /// Hub avec les émulateurs fournis ([`builtin`]) ; le premier (AsteraBox) est choisi.
    pub fn new() -> Self {
        Self::with_emulators(builtin())
    }

    /// Hub avec ses propres émulateurs (liste vide → émulateurs fournis).
    pub fn with_emulators(emulators: Vec<Box<dyn Emulator>>) -> Self {
        let emulators = if emulators.is_empty() { builtin() } else { emulators };
        let state = State {
            emulators,
            active: 0,
            generation: 0,
            peripheral: None,
            adapter: None,
            published: None,
            services: 0,
            advertising: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            journal: Journal::default(),
        };
        let shared = Arc::new(HubShared { state: Mutex::new(state), listener: Mutex::new(None) });
        let (tx, rx) = channel::<Msg>();
        let worker_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name("arklight-ble".into())
            .spawn(move || {
                for msg in rx {
                    match msg {
                        Msg::Event(generation, event) => worker_shared.process(generation, event),
                        Msg::Quit => break,
                    }
                }
            })
            .ok();
        Hub { shared, tx, worker }
    }

    /// Rappel à chaque changement (état, échange, message) ; appelé depuis un autre fil, il doit
    /// rester bref (réveiller l'interface).
    pub fn set_listener(&self, listener: impl Fn(&HubEvent) + Send + Sync + 'static) {
        *lock(&self.shared.listener) = Some(Arc::new(listener));
    }

    /// Émulateurs proposés.
    pub fn emulators(&self) -> Vec<EmulatorInfo> {
        let s = self.shared.lock();
        s.emulators.iter().map(|e| EmulatorInfo { id: e.id().to_string(), name: e.name().to_string(), ready: e.ready(), note: e.note().to_string() }).collect()
    }

    /// Identifiant de l'émulateur actif.
    pub fn emulator(&self) -> String {
        self.shared.lock().emulator().id().to_string()
    }

    /// Profil de l'émulateur actif.
    pub fn profile(&self) -> Profile {
        self.shared.lock().emulator().profile()
    }

    /// Choisit l'émulateur ; s'il était publié, le nouveau profil le remplace (bloquant comme `start`).
    pub fn set_emulator(&self, id: &str) -> Result<(), Error> {
        let running = {
            let mut s = self.shared.lock();
            let index = s.emulators.iter().position(|e| e.id() == id).ok_or_else(|| Error::UnknownEmulator(id.to_string()))?;
            s.active = index;
            s.subscriptions.clear();
            s.journal.status_changed();
            s.peripheral.is_some()
        };
        self.shared.flush();
        if running { self.start() } else { Ok(()) }
    }

    /// Relit l'état de l'adaptateur (affiché dans `status().adapter`), sans rien publier.
    pub fn refresh_adapter(&self) -> Result<AdapterInfo, Error> {
        let info = adapter_info()?;
        {
            let mut s = self.shared.lock();
            s.adapter = Some(info.clone());
            s.journal.status_changed();
        }
        self.shared.flush();
        Ok(info)
    }

    /// Publie le profil de l'émulateur actif et démarre l'annonce (remplace une publication en
    /// cours). Vérifie d'abord l'adaptateur : sans adaptateur ou radio coupée dans Windows, renvoie
    /// une erreur (les réglages de Windows ne sont jamais modifiés). Bloquant (quelques centaines de
    /// millisecondes) : à appeler hors du fil de l'interface.
    pub fn start(&self) -> Result<(), Error> {
        let (old, profile, generation) = {
            let mut s = self.shared.lock();
            s.generation += 1;
            s.clear_session();
            (s.peripheral.take(), s.emulator().profile(), s.generation)
        };
        drop(old);
        let result = self.publish(&profile, generation);
        if let Err(e) = &result {
            let level = if matches!(e, Error::RadioOff(_)) { Level::Warn } else { Level::Error };
            let mut s = self.shared.lock();
            s.journal.log(level, format!("Bluetooth : {e}"));
            s.journal.status_changed();
        }
        self.shared.flush();
        result
    }

    fn publish(&self, profile: &Profile, generation: u64) -> Result<(), Error> {
        let info = adapter_info()?;
        {
            let mut s = self.shared.lock();
            s.adapter = Some(info.clone());
            if info.present && !info.peripheral_role {
                s.journal.log(Level::Warn, "Bluetooth : l'adaptateur ne gère pas le rôle périphérique, l'annonce risque d'échouer (utiliser un ESP32)".into());
            }
        }
        if !info.present {
            return Err(Error::NoAdapter);
        }
        if !info.radio_on {
            return Err(Error::RadioOff(info.radio));
        }
        let tx = self.tx.clone();
        let peripheral = Peripheral::start(profile, move |event| {
            let _ = tx.send(Msg::Event(generation, event));
        })?;
        let mut s = self.shared.lock();
        if s.generation != generation {
            // arrêté ou remplacé pendant la publication
            drop(s);
            drop(peripheral);
            return Ok(());
        }
        s.published = Some(profile.name.clone());
        s.services = peripheral.services();
        let address = info.address.unwrap_or_default();
        if s.services > 0 {
            s.journal.log(Level::Info, format!("Bluetooth : profil « {} » publié (adaptateur {address}), le PC est visible par les téléphones", profile.name));
        } else {
            s.journal.log(
                Level::Warn,
                format!(
                    "Bluetooth : le profil « {} » est vide, rien n’est annoncé : aucun téléphone ne peut voir de box. \
                     Il faut d’abord capturer les échanges avec une vraie AsteraBox (docs/CAPTURE_BLUETOOTH.md).",
                    profile.name
                ),
            );
        }
        s.peripheral = Some(peripheral);
        s.journal.status_changed();
        Ok(())
    }

    /// Arrête l'annonce et retire la table GATT (sans effet si déjà arrêté).
    pub fn stop(&self) {
        let old = {
            let mut s = self.shared.lock();
            s.generation += 1;
            let old = s.peripheral.take();
            if old.is_some() {
                s.journal.log(Level::Info, "Bluetooth : annonce arrêtée".into());
            }
            s.clear_session();
            s.journal.status_changed();
            old
        };
        drop(old);
        self.shared.flush();
    }

    pub fn is_running(&self) -> bool {
        self.shared.lock().peripheral.is_some()
    }

    pub fn status(&self) -> HubStatus {
        let s = self.shared.lock();
        let phones: BTreeSet<String> =
            s.subscriptions.values().flatten().map(|c| c.device.clone().unwrap_or_else(|| "inconnu".to_string())).collect();
        HubStatus {
            emulator: s.emulator().id().to_string(),
            emulators: s
                .emulators
                .iter()
                .map(|e| EmulatorInfo { id: e.id().to_string(), name: e.name().to_string(), ready: e.ready(), note: e.note().to_string() })
                .collect(),
            running: s.peripheral.is_some(),
            adapter: s.adapter.clone(),
            published: s.published.clone(),
            services: s.services,
            advertising: s.advertising.clone(),
            phones: phones.into_iter().collect(),
        }
    }

    /// Journal des échanges (300 derniers, du plus ancien au plus récent).
    pub fn traffic(&self) -> Vec<TrafficEntry> {
        self.shared.lock().journal.traffic.iter().cloned().collect()
    }

    /// Échanges de numéro `seq` ou plus (pour n'ajouter que les nouveaux : garder `dernier.seq + 1`).
    pub fn traffic_since(&self, seq: u64) -> Vec<TrafficEntry> {
        self.shared.lock().journal.traffic.iter().filter(|e| e.seq >= seq).cloned().collect()
    }

    pub fn clear_traffic(&self) {
        self.shared.lock().journal.traffic.clear();
    }

    /// Derniers messages (200), du plus ancien au plus récent.
    pub fn messages(&self) -> Vec<LogLine> {
        self.shared.lock().journal.messages.iter().cloned().collect()
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        self.stop();
        let _ = self.tx.send(Msg::Quit);
        if let Some(worker) = self.worker.take()
            && worker.thread().id() != std::thread::current().id()
        {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    //! Aucun de ces tests ne touche au Bluetooth : les événements sont injectés directement, sans
    //! appeler `start`.
    use super::*;
    use crate::emulator::test_echo::{RX, SERVICE, TX};
    use crate::peripheral::{AdvertisingEvent, ReadEvent, ReadReply, SubscribeEvent, WriteEvent};
    use std::time::Duration;

    fn inject(hub: &Hub, event: Event) {
        let generation = hub.shared.lock().generation;
        hub.shared.process(generation, event);
    }

    fn write(value: &[u8], with_response: bool) -> Event {
        Event::Write(WriteEvent { service: SERVICE.into(), char: RX.into(), value: value.to_vec(), offset: 0, with_response, device: Some("tel".into()) })
    }

    #[test]
    fn etat_initial() {
        let hub = Hub::new();
        let st = hub.status();
        assert_eq!(st.emulator, "astera");
        assert_eq!(hub.emulator(), "astera");
        assert!(!st.running && !hub.is_running());
        assert_eq!(st.emulators.len(), 2);
        assert_eq!(st.emulators[0].id, "astera");
        assert!(!st.emulators[0].ready);
        assert!(st.emulators[0].note.contains("CAPTURE_BLUETOOTH"));
        assert_eq!(st.emulators[1], EmulatorInfo { id: "test".into(), name: "Test (écho)".into(), ready: true, note: crate::TestEcho.note().into() });
        assert_eq!(hub.emulators(), st.emulators);
        assert!(hub.traffic().is_empty());
        hub.stop(); // sans effet
        assert!(!hub.is_running());
    }

    #[test]
    fn choix_de_l_emulateur() {
        let hub = Hub::new();
        assert_eq!(hub.set_emulator("inconnu"), Err(Error::UnknownEmulator("inconnu".into())));
        assert_eq!(hub.emulator(), "astera");
        hub.set_emulator("test").unwrap(); // arrêté : rien n'est publié
        assert_eq!(hub.status().emulator, "test");
        assert_eq!(hub.profile().name, "ARKLIGHT Test");
        assert!(!hub.is_running());
    }

    #[test]
    fn echo_par_le_hub() {
        let hub = Hub::new();
        hub.set_emulator("test").unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        hub.set_listener(move |e| sink.lock().unwrap().push(e.clone()));

        inject(&hub, write(b"hi", false));
        inject(&hub, write(b"ok", true));
        let traffic = hub.traffic();
        assert_eq!(traffic.len(), 4);
        assert_eq!((traffic[0].dir, traffic[0].op, traffic[0].char.as_str(), traffic[0].hex()), (Direction::In, Op::WriteCmd, RX, "6869".to_string()));
        assert_eq!((traffic[1].dir, traffic[1].op, traffic[1].char.as_str()), (Direction::Out, Op::Notify, TX));
        assert_eq!(traffic[1].bytes, b"ECHO:hi");
        assert_eq!(traffic[2].op, Op::WriteReq);
        assert_eq!(traffic.iter().map(|t| t.seq).collect::<Vec<_>>(), [0, 1, 2, 3]);
        assert_eq!(hub.traffic_since(2).len(), 2);

        // message de l'émulateur préfixé de son nom ; pas d'erreur (Bluetooth arrêté : notification seulement notée)
        let messages = hub.messages();
        assert_eq!(messages[0].text, "Test (écho) : reçu 6869 (« hi »)");
        assert!(messages.iter().all(|m| m.level == Level::Info));

        let seen = seen.lock().unwrap();
        assert!(seen.iter().any(|e| matches!(e, HubEvent::Traffic(t) if t.op == Op::Notify)));
        assert!(seen.iter().any(|e| matches!(e, HubEvent::Status)));
        assert!(seen.iter().any(|e| matches!(e, HubEvent::Log(l) if l.text.starts_with("Test (écho)"))));

        // JSON du journal : même forme que le JS
        let json = serde_json::to_value(&traffic[1]).unwrap();
        assert_eq!(json["dir"], "out");
        assert_eq!(json["op"], "NOTIFY");
        assert_eq!(json["char"], TX);
        assert_eq!(json["hex"], "4543484f3a6869");
    }

    #[test]
    fn journal_borne_a_300() {
        let hub = Hub::new();
        hub.set_emulator("test").unwrap();
        hub.set_listener(|_| {});
        for i in 0..200u32 {
            inject(&hub, write(&i.to_be_bytes(), false)); // 2 échanges chacun
        }
        let traffic = hub.traffic();
        assert_eq!(traffic.len(), MAX_TRAFFIC);
        assert_eq!(traffic.first().unwrap().seq, 100);
        assert_eq!(traffic.last().unwrap().seq, 399);
        assert!(hub.messages().len() <= MAX_MESSAGES);
        assert_eq!(hub.traffic_since(398).len(), 2);
        assert_eq!(hub.traffic_since(10).len(), MAX_TRAFFIC);
        hub.clear_traffic();
        assert!(hub.traffic().is_empty());
        inject(&hub, write(b"x", false));
        assert_eq!(hub.traffic()[0].seq, 400, "les numéros continuent après effacement");
    }

    #[test]
    fn abonnements_et_annonce() {
        let hub = Hub::new();
        let client = |d: &str| Client { device: Some(d.into()), max_notification: 20 };
        inject(&hub, Event::Subscribe(SubscribeEvent { service: SERVICE.into(), char: TX.into(), clients: vec![client("tel-a"), client("tel-b")] }));
        inject(&hub, Event::Subscribe(SubscribeEvent { service: SERVICE.into(), char: RX.into(), clients: vec![client("tel-a")] }));
        assert_eq!(hub.status().phones, ["tel-a", "tel-b"]);
        inject(&hub, Event::Subscribe(SubscribeEvent { service: SERVICE.into(), char: TX.into(), clients: vec![] }));
        assert_eq!(hub.status().phones, ["tel-a"]);
        assert!(hub.messages().iter().any(|m| m.text == format!("Bluetooth : désabonnement à {TX}")));

        inject(&hub, Event::Advertising(AdvertisingEvent { service: SERVICE.into(), status: "en cours".into(), error: 0 }));
        assert_eq!(hub.status().advertising.get(SERVICE).map(String::as_str), Some("en cours"));
        inject(&hub, Event::Advertising(AdvertisingEvent { service: SERVICE.into(), status: "abandonné".into(), error: 1 }));
        assert!(hub.messages().iter().any(|m| m.level == Level::Warn && m.text.contains("radio indisponible")));

        inject(&hub, Event::Error("lecture x : panne".into()));
        assert_eq!(hub.messages().last().unwrap().text, "Bluetooth : lecture x : panne");
        assert_eq!(hub.messages().last().unwrap().level, Level::Error);

        // changer d'émulateur oublie les abonnements
        hub.set_emulator("test").unwrap();
        assert!(hub.status().phones.is_empty());
        // arrêter efface l'état de la session
        hub.stop();
        assert!(hub.status().advertising.is_empty());
    }

    #[test]
    fn lecture_et_anciennes_generations() {
        let hub = Hub::new();
        hub.set_emulator("test").unwrap();
        let reply = ReadReply::default();
        inject(&hub, Event::Read(ReadEvent { service: SERVICE.into(), char: TX.into(), offset: 0, device: None, reply: reply.clone() }));
        assert_eq!(reply.wait(Duration::ZERO), Some(None), "l'écho ne répond pas : valeur en cache, sans attendre");
        assert_eq!(hub.traffic()[0].op, Op::Read);

        // événement d'un périphérique arrêté depuis : ignoré (mais une lecture est débloquée)
        let old = hub.shared.lock().generation;
        hub.stop();
        hub.shared.process(old, write(b"perdu", false));
        let reply = ReadReply::default();
        hub.shared.process(old, Event::Read(ReadEvent { service: SERVICE.into(), char: TX.into(), offset: 0, device: None, reply: reply.clone() }));
        assert_eq!(reply.wait(Duration::ZERO), Some(None));
        assert_eq!(hub.traffic().len(), 1);
    }

    #[test]
    fn par_le_fil_du_hub() {
        // chemin réel des événements (canal → fil « arklight-ble »), sans Bluetooth
        let hub = Hub::new();
        hub.set_emulator("test").unwrap();
        let generation = hub.shared.lock().generation;
        let (tx, rx) = channel();
        let tx = Mutex::new(tx);
        hub.set_listener(move |e| {
            if let HubEvent::Traffic(t) = e {
                let _ = lock(&tx).send(t.op);
            }
        });
        hub.tx.send(Msg::Event(generation, write(b"fil", false))).unwrap();
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Op::WriteCmd);
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), Op::Notify);
        drop(hub); // le fil s'arrête
    }

    #[test]
    fn rappel_qui_panique() {
        let hub = Hub::new();
        hub.set_listener(|_| panic!("interface en panne"));
        inject(&hub, write(b"x", false));
        assert_eq!(hub.traffic().len(), 1);
    }
}
