//! Réception en tâche de fond : un fil pour Art-Net (port 6454), un pour sACN (port 5568). Les
//! trames reçues sont fusionnées dans un `UniverseStore` partagé ; l'application en recopie les
//! univers à chaque image (`snapshot`).
//!
//! RDM sur Art-Net : une fois les projecteurs de la scène donnés (`set_rdm_fixtures`), le fil
//! Art-Net répond aux ArtTodRequest / ArtTodControl / ArtRdm. Un changement demandé par une appli
//! est appliqué tout de suite à sa copie des projecteurs (les questions suivantes voient la
//! nouvelle adresse) et mis en attente pour l'application (`take_rdm_events`).

use crate::rdm::{RdmEvent, RdmResponder};
use crate::universes::{Input, SourceInfo, UniverseStore};
use crate::{artnet, sacn};
use arklight_core::Fixture;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Où écouter (adresse et ports) ; l'univers 1 d'ARKLIGHT reçoit le port-address Art-Net `1 − offset`.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind: Ipv4Addr,
    pub artnet_port: u16,
    pub sacn_port: u16,
    /// Art-Net port-address 0 → univers ARKLIGHT `offset` (1 par défaut, comme la version JavaScript)
    pub artnet_offset: u32,
    pub short_name: String,
    pub long_name: String,
}

impl Default for Config {
    fn default() -> Self {
        Self { bind: Ipv4Addr::UNSPECIFIED, artnet_port: artnet::PORT, sacn_port: sacn::PORT, artnet_offset: 1, short_name: "ARKLIGHT".into(), long_name: "ARKLIGHT simulator".into() }
    }
}

/// État de la réception (barre d'état).
#[derive(Debug, Clone, Default)]
pub struct Status {
    /// Art-Net : à l'écoute, ou le message d'erreur
    pub artnet: Option<Result<u16, String>>,
    pub sacn: Option<Result<u16, String>>,
    pub artnet_packets: u64,
    pub sacn_packets: u64,
    pub polls: u64,
    pub sources: Vec<SourceInfo>,
    /// autres nodes Art-Net vus sur le réseau (adresse, nom)
    pub nodes: Vec<(Ipv4Addr, String)>,
    /// RDM actif (projecteurs annoncés aux applis)
    pub rdm: bool,
    /// messages ArtRdm reçus
    pub rdm_requests: u64,
}

/// changements RDM gardés au plus en attendant l'application
const MAX_RDM_EVENTS: usize = 1000;

/// RDM : copie des projecteurs de la scène, répondeur, changements en attente.
struct Rdm {
    fixtures: Vec<Fixture>,
    responder: RdmResponder,
    events: Vec<RdmEvent>,
}

impl Rdm {
    fn push(&mut self, events: impl IntoIterator<Item = RdmEvent>) {
        self.events.extend(events);
        if self.events.len() > MAX_RDM_EVENTS {
            let extra = self.events.len() - MAX_RDM_EVENTS;
            self.events.drain(..extra);
        }
    }
}

struct Shared {
    store: Mutex<UniverseStore>,
    /// univers utilisés par le projet (annoncés aux ArtPoll, groupes sACN rejoints)
    universes: Mutex<Vec<u32>>,
    artnet: Mutex<Option<Result<u16, String>>>,
    sacn: Mutex<Option<Result<u16, String>>>,
    nodes: Mutex<Vec<(Ipv4Addr, String)>>,
    /// RDM (None : désactivé)
    rdm: Mutex<Option<Rdm>>,
    rdm_requests: AtomicU64,
    artnet_packets: AtomicU64,
    sacn_packets: AtomicU64,
    polls: AtomicU64,
    running: AtomicBool,
    /// numérotation Art-Net : univers ARKLIGHT = port-address + décalage (0 ou 1), modifiable en marche
    artnet_offset: AtomicU32,
}

/// Réception Art-Net et sACN ; s'arrête quand on la lâche.
pub struct Receiver {
    shared: Arc<Shared>,
    config: Config,
}

fn udp_socket(bind: Ipv4Addr, port: u16) -> std::io::Result<UdpSocket> {
    let s = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    // une console ou un autre logiciel écoute souvent déjà le même port sur cet ordinateur
    s.set_reuse_address(true)?;
    s.set_broadcast(true)?;
    s.bind(&SockAddr::from(SocketAddrV4::new(bind, port)))?;
    s.set_read_timeout(Some(Duration::from_millis(250)))?;
    Ok(s.into())
}

/// Adresse IPv4 du PC sur le réseau local (interface de la route par défaut ; aucun paquet n'est
/// envoyé) : à saisir dans les applications (Luminair, Blackout…).
pub fn primary_ipv4() -> Option<Ipv4Addr> {
    let ip = local_ip_for(Ipv4Addr::new(192, 168, 255, 255));
    let ip = if ip.is_unspecified() || ip.is_loopback() { local_ip_for(Ipv4Addr::new(10, 255, 255, 255)) } else { ip };
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

/// Adresse locale qui sert à joindre `remote` (interface du même réseau).
fn local_ip_for(remote: Ipv4Addr) -> Ipv4Addr {
    if remote.is_loopback() {
        return Ipv4Addr::LOCALHOST;
    }
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect(SocketAddrV4::new(remote, artnet::PORT))?;
            s.local_addr()
        })
        .ok()
        .and_then(|a| match a {
            SocketAddr::V4(v) => Some(*v.ip()),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::UNSPECIFIED)
}

impl Receiver {
    /// Numérotation Art-Net : « Art-Net 0 = Univers 1 » (1) ou « Art-Net 0 = Univers 0 » (0).
    pub fn set_artnet_offset(&self, offset: u32) {
        self.shared.artnet_offset.store(offset, Ordering::Relaxed);
    }

    /// Ouvre les ports et commence à écouter.
    pub fn start(config: Config) -> Receiver {
        let shared = Arc::new(Shared {
            store: Mutex::new(UniverseStore::default()),
            universes: Mutex::new(vec![1]),
            artnet: Mutex::new(None),
            sacn: Mutex::new(None),
            nodes: Mutex::new(Vec::new()),
            rdm: Mutex::new(None),
            rdm_requests: AtomicU64::new(0),
            artnet_packets: AtomicU64::new(0),
            sacn_packets: AtomicU64::new(0),
            polls: AtomicU64::new(0),
            running: AtomicBool::new(true),
            artnet_offset: AtomicU32::new(config.artnet_offset),
        });
        let r = Receiver { shared, config };
        r.spawn_artnet();
        r.spawn_sacn();
        r
    }

    fn spawn_artnet(&self) {
        let shared = self.shared.clone();
        let cfg = self.config.clone();
        thread::spawn(move || {
            let sock = match udp_socket(cfg.bind, cfg.artnet_port) {
                Ok(s) => {
                    *shared.artnet.lock().unwrap() = Some(Ok(cfg.artnet_port));
                    s
                }
                Err(e) => {
                    *shared.artnet.lock().unwrap() = Some(Err(e.to_string()));
                    return;
                }
            };
            let mut buf = [0u8; 1024];
            while shared.running.load(Ordering::Relaxed) {
                let Ok((n, from)) = sock.recv_from(&mut buf) else { continue };
                let SocketAddr::V4(from) = from else { continue };
                // réponse : au port Art-Net de l'expéditeur, et au port d'où vient la demande
                let reply = |pkt: &[u8]| {
                    let _ = sock.send_to(pkt, SocketAddrV4::new(*from.ip(), cfg.artnet_port));
                    if from.port() != cfg.artnet_port {
                        let _ = sock.send_to(pkt, from);
                    }
                };
                match artnet::parse(&buf[..n]) {
                    Some(artnet::Packet::Dmx { port_address, data, .. }) => {
                        shared.artnet_packets.fetch_add(1, Ordering::Relaxed);
                        let input = Input { universe: port_address as u32 + shared.artnet_offset.load(Ordering::Relaxed), data, key: format!("artnet|{}", from.ip()), priority: 100, protocol: "artnet", address: from.ip().to_string(), name: String::new() };
                        shared.store.lock().unwrap().input(input, Instant::now());
                    }
                    Some(artnet::Packet::Poll) => {
                        shared.polls.fetch_add(1, Ordering::Relaxed);
                        let ports: Vec<u16> = shared.universes.lock().unwrap().iter().filter_map(|u| u.checked_sub(shared.artnet_offset.load(Ordering::Relaxed))).map(|pa| pa as u16).collect();
                        let node = artnet::Node { short_name: &cfg.short_name, long_name: &cfg.long_name, ip: local_ip_for(*from.ip()), mac: [0; 6], packets: shared.artnet_packets.load(Ordering::Relaxed) };
                        let rdm = shared.rdm.lock().unwrap().is_some();
                        for pkt in artnet::build_poll_replies_rdm(&node, &ports, rdm) {
                            reply(&pkt);
                        }
                    }
                    Some(artnet::Packet::TodRequest { port_addresses }) => send_tod(&shared, *from.ip(), &port_addresses, reply),
                    Some(artnet::Packet::TodControl { port_address, .. }) => send_tod(&shared, *from.ip(), &[port_address], reply),
                    Some(artnet::Packet::Rdm { port_address, data }) => {
                        let answer = {
                            let mut guard = shared.rdm.lock().unwrap();
                            let Some(rdm) = guard.as_mut() else { continue };
                            shared.rdm_requests.fetch_add(1, Ordering::Relaxed);
                            let mut events = Vec::new();
                            let answer = rdm.responder.handle(&rdm.fixtures, port_address as u32 + shared.artnet_offset.load(Ordering::Relaxed), data, &mut events);
                            for e in &events {
                                e.apply(&mut rdm.fixtures);
                            }
                            rdm.push(events);
                            answer
                        };
                        if let Some(a) = answer {
                            reply(&artnet::build_rdm(port_address, &a));
                        }
                    }
                    Some(artnet::Packet::PollReply { ip, name }) => {
                        let mut nodes = shared.nodes.lock().unwrap();
                        if !nodes.iter().any(|(i, _)| *i == ip) {
                            nodes.push((ip, name));
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn spawn_sacn(&self) {
        let shared = self.shared.clone();
        let cfg = self.config.clone();
        thread::spawn(move || {
            let sock = match udp_socket(cfg.bind, cfg.sacn_port) {
                Ok(s) => {
                    *shared.sacn.lock().unwrap() = Some(Ok(cfg.sacn_port));
                    s
                }
                Err(e) => {
                    *shared.sacn.lock().unwrap() = Some(Err(e.to_string()));
                    return;
                }
            };
            let iface = if cfg.bind.is_unspecified() { Ipv4Addr::UNSPECIFIED } else { cfg.bind };
            let mut joined: HashSet<u16> = HashSet::new();
            let mut buf = [0u8; 1144];
            while shared.running.load(Ordering::Relaxed) {
                // groupes multicast des univers du projet (1..63999)
                let wanted: HashSet<u16> = shared.universes.lock().unwrap().iter().filter(|u| (1..=63999).contains(*u)).map(|u| *u as u16).collect();
                for u in wanted.difference(&joined.clone()) {
                    if sock.join_multicast_v4(&sacn::multicast_group(*u), &iface).is_ok() || !cfg.bind.is_unspecified() {
                        joined.insert(*u);
                    }
                }
                for u in joined.clone().difference(&wanted) {
                    let _ = sock.leave_multicast_v4(&sacn::multicast_group(*u), &iface);
                    joined.remove(u);
                }
                let Ok((n, from)) = sock.recv_from(&mut buf) else { continue };
                let SocketAddr::V4(from) = from else { continue };
                let now = Instant::now();
                match sacn::parse(&buf[..n]) {
                    Some(sacn::Packet::Dmx { universe, priority, data, source_name, cid, .. }) => {
                        shared.sacn_packets.fetch_add(1, Ordering::Relaxed);
                        let key = format!("sacn|{}", cid.iter().map(|b| format!("{b:02x}")).collect::<String>());
                        let input = Input { universe: universe as u32, data, key, priority, protocol: "sacn", address: from.ip().to_string(), name: source_name };
                        shared.store.lock().unwrap().input(input, now);
                    }
                    Some(sacn::Packet::Terminated { universe, cid }) => {
                        let key = format!("sacn|{}", cid.iter().map(|b| format!("{b:02x}")).collect::<String>());
                        shared.store.lock().unwrap().terminate(universe as u32, &key, now);
                    }
                    None => {}
                }
            }
        });
    }

    /// RDM : projecteurs de la scène annoncés aux applis (à redonner quand le projet change) ;
    /// active le RDM au premier appel.
    pub fn set_rdm_fixtures(&self, fixtures: &[Fixture]) {
        let mut rdm = self.shared.rdm.lock().unwrap();
        match rdm.as_mut() {
            Some(r) => r.fixtures = fixtures.to_vec(),
            None => *rdm = Some(Rdm { fixtures: fixtures.to_vec(), responder: RdmResponder::new(), events: Vec::new() }),
        }
    }

    /// RDM : plus aucune réponse aux applis (projecteurs, identifications et changements oubliés).
    pub fn disable_rdm(&self) {
        *self.shared.rdm.lock().unwrap() = None;
    }

    /// RDM : changements demandés par les applis depuis le dernier appel, dans l'ordre (adresse,
    /// mode, nom à appliquer au projet ; identification ; découvertes pour le journal).
    pub fn take_rdm_events(&self) -> Vec<RdmEvent> {
        self.shared.rdm.lock().unwrap().as_mut().map(|r| std::mem::take(&mut r.events)).unwrap_or_default()
    }

    /// RDM : projecteurs qu'une appli a mis en identification (ids, triés).
    pub fn rdm_identifying(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.shared.rdm.lock().unwrap().as_ref().map(|r| r.responder.identifying.iter().cloned().collect()).unwrap_or_default();
        ids.sort();
        ids
    }

    /// Univers utilisés par le projet : annoncés aux consoles (ArtPoll), groupes sACN rejoints.
    pub fn set_universes(&self, used: &[u32]) {
        let mut list: Vec<u32> = used.to_vec();
        list.sort_unstable();
        list.dedup();
        *self.shared.universes.lock().unwrap() = list;
    }

    /// Recopie les univers reçus et ceux dont le signal arrive ; vrai si quelque chose a changé.
    pub fn snapshot(&self, dmx: &mut HashMap<u32, [u8; 512]>, live: &mut HashSet<u32>) -> bool {
        let now = Instant::now();
        let mut store = self.shared.store.lock().unwrap();
        let dirty = store.take_dirty();
        for u in &dirty {
            if let Some((_, d)) = store.data().find(|(x, _)| x == u) {
                dmx.insert(*u, *d);
            }
        }
        let now_live: HashSet<u32> = store.live(now).into_iter().collect();
        let changed = !dirty.is_empty() || now_live != *live;
        *live = now_live;
        store.sweep(now);
        changed
    }

    /// Oublie le dernier état reçu d'un univers que plus personne n'envoie (« reprendre la main »).
    pub fn forget(&self, universe: u32) -> bool {
        self.shared.store.lock().unwrap().forget(universe, Instant::now())
    }

    pub fn status(&self) -> Status {
        Status {
            artnet: self.shared.artnet.lock().unwrap().clone(),
            sacn: self.shared.sacn.lock().unwrap().clone(),
            artnet_packets: self.shared.artnet_packets.load(Ordering::Relaxed),
            sacn_packets: self.shared.sacn_packets.load(Ordering::Relaxed),
            polls: self.shared.polls.load(Ordering::Relaxed),
            sources: self.shared.store.lock().unwrap().sources(Instant::now()),
            nodes: self.shared.nodes.lock().unwrap().clone(),
            rdm: self.shared.rdm.lock().unwrap().is_some(),
            rdm_requests: self.shared.rdm_requests.load(Ordering::Relaxed),
        }
    }
}

/// RDM : envoie la table des appareils de chaque port-address demandé (ArtTodData).
fn send_tod(shared: &Shared, source: Ipv4Addr, port_addresses: &[u16], reply: impl Fn(&[u8])) {
    let mut guard = shared.rdm.lock().unwrap();
    let Some(rdm) = guard.as_mut() else { return };
    let mut total = 0;
    for pa in port_addresses {
        let uids = rdm.responder.tod(&rdm.fixtures, *pa as u32 + shared.artnet_offset.load(Ordering::Relaxed));
        total += uids.len();
        for pkt in artnet::build_tod_data(*pa, &uids) {
            reply(&pkt);
        }
    }
    rdm.push([RdmEvent::Discovery { source, universes: port_addresses.len(), devices: total }]);
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.shared.running.store(false, Ordering::Relaxed);
    }
}
