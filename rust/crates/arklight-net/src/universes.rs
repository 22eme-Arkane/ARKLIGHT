//! Tampons des univers DMX et fusion des sources (portage de `server/dmx/universes.js`) :
//! priorité sACN, puis la plus haute valeur (HTP) entre sources de même priorité. Quand toutes les
//! sources se taisent, le dernier état est conservé (« hold last look »).

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Délai de perte d'une source (norme E1.31).
pub const SOURCE_TIMEOUT: Duration = Duration::from_millis(2500);

/// Une source DMX d'un univers.
#[derive(Debug, Clone)]
pub struct Source {
    pub data: [u8; 512],
    pub packets: u64,
    pub protocol: &'static str,
    pub address: String,
    pub name: String,
    pub priority: u8,
    pub last: Instant,
}

#[derive(Debug, Clone)]
struct Universe {
    data: [u8; 512],
    sources: HashMap<String, Source>,
    dirty: bool,
}

/// Univers reçus : numéro interne (1..) → données fusionnées.
#[derive(Debug, Default)]
pub struct UniverseStore {
    universes: HashMap<u32, Universe>,
}

/// Résumé d'une source active (barre d'état, journal).
#[derive(Debug, Clone, PartialEq)]
pub struct SourceInfo {
    pub protocol: &'static str,
    pub address: String,
    pub name: String,
    pub universes: Vec<u32>,
    pub priority: u8,
}

/// Données reçues d'une source.
pub struct Input<'a> {
    pub universe: u32,
    pub data: &'a [u8],
    /// identifiant unique de la source (protocole, adresse ou CID)
    pub key: String,
    /// priorité sACN (Art-Net : 100)
    pub priority: u8,
    pub protocol: &'static str,
    pub address: String,
    pub name: String,
}

impl UniverseStore {
    pub fn input(&mut self, i: Input, now: Instant) {
        let uni = self.universes.entry(i.universe).or_insert_with(|| Universe { data: [0; 512], sources: HashMap::new(), dirty: true });
        let src = uni.sources.entry(i.key).or_insert_with(|| Source { data: [0; 512], packets: 0, protocol: i.protocol, address: i.address.clone(), name: i.name.clone(), priority: i.priority, last: now });
        let n = i.data.len().min(512);
        src.data[..n].copy_from_slice(&i.data[..n]);
        src.data[n..].fill(0);
        src.priority = i.priority;
        src.last = now;
        src.packets += 1;
        if !i.name.is_empty() {
            src.name = i.name;
        }
        merge(uni, now);
    }

    /// Une source sACN s'arrête.
    pub fn terminate(&mut self, universe: u32, key: &str, now: Instant) {
        if let Some(uni) = self.universes.get_mut(&universe)
            && uni.sources.remove(key).is_some()
        {
            merge(uni, now);
        }
    }

    /// Oublie le dernier état d'un univers que plus aucune source n'envoie (« reprendre la main ») ;
    /// faux si une source émet encore.
    pub fn forget(&mut self, universe: u32, now: Instant) -> bool {
        let Some(uni) = self.universes.get(&universe) else { return true };
        if uni.sources.values().any(|s| now.duration_since(s.last) <= SOURCE_TIMEOUT) {
            return false;
        }
        self.universes.remove(&universe);
        true
    }

    /// Supprime les sources muettes.
    pub fn sweep(&mut self, now: Instant) {
        for uni in self.universes.values_mut() {
            uni.sources.retain(|_, s| now.duration_since(s.last) <= SOURCE_TIMEOUT * 3);
        }
    }

    /// Données de chaque univers reçu.
    pub fn data(&self) -> impl Iterator<Item = (u32, &[u8; 512])> {
        self.universes.iter().map(|(u, x)| (*u, &x.data))
    }

    /// Univers modifiés depuis le dernier appel.
    pub fn take_dirty(&mut self) -> Vec<u32> {
        let mut out = Vec::new();
        for (u, x) in &mut self.universes {
            if x.dirty {
                x.dirty = false;
                out.push(*u);
            }
        }
        out
    }

    /// Univers dont une source émet en ce moment.
    pub fn live(&self, now: Instant) -> Vec<u32> {
        self.universes.iter().filter(|(_, x)| x.sources.values().any(|s| now.duration_since(s.last) <= SOURCE_TIMEOUT)).map(|(u, _)| *u).collect()
    }

    /// Sources actives, regroupées par protocole et adresse.
    pub fn sources(&self, now: Instant) -> Vec<SourceInfo> {
        let mut out: Vec<SourceInfo> = Vec::new();
        let mut list: Vec<(&u32, &Universe)> = self.universes.iter().collect();
        list.sort_by_key(|(u, _)| **u);
        for (u, uni) in list {
            for s in uni.sources.values() {
                if now.duration_since(s.last) > SOURCE_TIMEOUT {
                    continue;
                }
                match out.iter_mut().find(|e| e.protocol == s.protocol && e.address == s.address) {
                    Some(e) => {
                        e.universes.push(*u);
                        if !s.name.is_empty() {
                            e.name = s.name.clone();
                        }
                    }
                    None => out.push(SourceInfo { protocol: s.protocol, address: s.address.clone(), name: s.name.clone(), universes: vec![*u], priority: s.priority }),
                }
            }
        }
        out
    }
}

fn merge(uni: &mut Universe, now: Instant) {
    let live: Vec<&Source> = uni.sources.values().filter(|s| now.duration_since(s.last) <= SOURCE_TIMEOUT).collect();
    let Some(best) = live.iter().map(|s| s.priority).max() else { return }; // dernier état conservé
    let top: Vec<&&Source> = live.iter().filter(|s| s.priority == best).collect();
    let mut changed = false;
    for i in 0..512 {
        let v = top.iter().map(|s| s.data[i]).max().unwrap_or(0);
        if uni.data[i] != v {
            uni.data[i] = v;
            changed = true;
        }
    }
    if changed {
        uni.dirty = true;
    }
}
