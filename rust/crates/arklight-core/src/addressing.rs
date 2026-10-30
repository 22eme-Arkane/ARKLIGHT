//! Adressage DMX automatique (portage de `shared/engine/addressing.js`) : conflits de patch
//! (chevauchements, dépassement du canal 512) et calcul d'un nouveau patch, sans rien modifier.

use crate::library;
use crate::project::Fixture;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

pub const UNIVERSE_SIZE: u32 = 512;
/// univers suivants explorés quand un univers est plein
const MAX_SPILL: u32 = 64;

/// Nombre de canaux occupés par un projecteur placé (0 pour un émetteur comme l'AsteraBox).
pub fn footprint_of(f: &Fixture) -> u32 {
    library().fixture(&f.type_id).and_then(|t| t.mode(f.settings.dmx_mode.as_deref())).map_or(0, |m| m.footprint() as u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub universe: u32,
    pub start: u32,
    pub end: u32,
    pub footprint: u32,
}

pub fn range_of(f: &Fixture) -> Range {
    let footprint = footprint_of(f);
    let start = f.settings.dmx_address.unwrap_or(1);
    Range { universe: f.settings.universe.unwrap_or(1), start, end: (start + footprint).saturating_sub(1), footprint }
}

/// Conflits : paires qui se chevauchent dans un même univers, projecteurs qui dépassent le canal 512.
#[derive(Debug, Clone, Default)]
pub struct Conflicts {
    pub overlaps: Vec<(String, String)>,
    pub overflow: Vec<String>,
    pub ids: HashSet<String>,
}

pub fn find_patch_conflicts<'a>(fixtures: impl IntoIterator<Item = (&'a str, Range)>) -> Conflicts {
    let mut c = Conflicts::default();
    // univers dans l'ordre d'apparition (comme une Map JavaScript)
    let mut by_universe: Vec<(u32, Vec<(&str, Range)>)> = Vec::new();
    for (id, r) in fixtures {
        if r.footprint == 0 {
            continue;
        }
        if r.end > UNIVERSE_SIZE {
            c.overflow.push(id.to_owned());
        }
        match by_universe.iter_mut().find(|(u, _)| *u == r.universe) {
            Some((_, list)) => list.push((id, r)),
            None => by_universe.push((r.universe, vec![(id, r)])),
        }
    }
    for (_, list) in &mut by_universe {
        list.sort_by_key(|(_, r)| r.start); // tri stable, comme Array.sort
        for i in 0..list.len() {
            let mut j = i + 1;
            while j < list.len() && list[j].1.start <= list[i].1.end {
                c.overlaps.push((list[i].0.to_owned(), list[j].0.to_owned()));
                j += 1;
            }
        }
    }
    c.ids = c.overflow.iter().cloned().chain(c.overlaps.iter().flat_map(|(a, b)| [a.clone(), b.clone()])).collect();
    c
}

/// Conflits des projecteurs d'une scène.
pub fn conflicts(fixtures: &[Fixture]) -> Conflicts {
    find_patch_conflicts(fixtures.iter().map(|f| (f.id.as_str(), range_of(f))))
}

/// Première adresse ≥ a de la forme 1 + k × step (step 0 ou 1 : a elle-même).
pub fn align_up(a: u32, step: u32) -> u32 {
    if step <= 1 { a } else { (a - 1).div_ceil(step) * step + 1 }
}

/// Canaux déjà pris, par univers.
#[derive(Default)]
struct Occupancy(HashMap<u32, Vec<(u32, u32)>>);

impl Occupancy {
    fn add(&mut self, u: u32, start: u32, end: u32) {
        let l = self.0.entry(u).or_default();
        l.push((start, end));
        l.sort_by_key(|r| r.0);
    }

    fn list(&self, u: u32) -> &[(u32, u32)] {
        self.0.get(&u).map_or(&[], |l| l.as_slice())
    }

    fn fits(&self, u: u32, a: u32, fp: u32) -> bool {
        a >= 1 && a + fp - 1 <= UNIVERSE_SIZE && !self.list(u).iter().any(|&(s, e)| a <= e && s <= a + fp - 1)
    }

    /// Première adresse ≥ from (alignée) où fp canaux tiennent sans chevauchement.
    fn find(&self, u: u32, from: u32, fp: u32, align: u32) -> Option<u32> {
        let l = self.list(u);
        let mut a = align_up(from.max(1), align);
        while a + fp - 1 <= UNIVERSE_SIZE {
            match l.iter().find(|&&(s, e)| a <= e && s <= a + fp - 1) {
                None => return Some(a),
                Some(&(_, e)) => a = align_up(e + 1, align),
            }
        }
        None
    }
}

/// Ordre d'adressage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Order {
    #[default]
    Scene,
    Name,
    /// de gauche à droite (vue de face)
    Position,
    /// par modèle
    Type,
    /// adresse actuelle
    Address,
}

impl Order {
    pub const ALL: [Order; 5] = [Order::Scene, Order::Name, Order::Position, Order::Type, Order::Address];

    pub fn id(self) -> &'static str {
        match self {
            Order::Scene => "scene",
            Order::Name => "name",
            Order::Position => "position",
            Order::Type => "type",
            Order::Address => "address",
        }
    }

    pub fn from_id(id: &str) -> Order {
        Order::ALL.into_iter().find(|o| o.id() == id).unwrap_or_default()
    }
}

/// Lettre de base (sans accent, minuscule) pour la comparaison « à la française ».
fn base_char(c: char) -> char {
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'œ' => 'o',
        'æ' => 'a',
        other => other,
    }
}

/// Comparaison naturelle (« Titan 2 » avant « Titan 10 »), sans tenir compte de la casse ni des
/// accents : comme `Intl.Collator('fr', { numeric: true, sensitivity: 'base' })` pour les noms usuels.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(p), Some(q)) if p.is_ascii_digit() && q.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(d) = x.peek().copied().filter(char::is_ascii_digit) {
                    n.push(d);
                    x.next();
                }
                let mut m = String::new();
                while let Some(d) = y.peek().copied().filter(char::is_ascii_digit) {
                    m.push(d);
                    y.next();
                }
                let (n, m) = (n.trim_start_matches('0'), m.trim_start_matches('0'));
                let o = n.len().cmp(&m.len()).then_with(|| n.cmp(m));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(p), Some(q)) => {
                let o = base_char(p).cmp(&base_char(q));
                if o != Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

fn sort_fixtures<'a>(list: &[&'a Fixture], order: Order, all: &[Fixture]) -> Vec<&'a Fixture> {
    let idx: HashMap<&str, usize> = all.iter().enumerate().map(|(i, f)| (f.id.as_str(), i)).collect();
    let scene = |a: &Fixture, b: &Fixture| idx[a.id.as_str()].cmp(&idx[b.id.as_str()]);
    let lib = library();
    let type_name = |f: &Fixture| lib.fixture(&f.type_id).map_or(f.type_id.clone(), |t| t.name.clone());
    let mut v = list.to_vec();
    v.sort_by(|a, b| match order {
        Order::Scene => scene(a, b),
        Order::Name => natural_cmp(&a.name, &b.name).then_with(|| scene(a, b)),
        Order::Position => a.position[0]
            .partial_cmp(&b.position[0])
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.position[2].partial_cmp(&b.position[2]).unwrap_or(Ordering::Equal))
            .then_with(|| scene(a, b)),
        Order::Type => natural_cmp(&type_name(a), &type_name(b)).then_with(|| natural_cmp(&a.name, &b.name)).then_with(|| scene(a, b)),
        Order::Address => {
            let (ra, rb) = (range_of(a), range_of(b));
            ra.universe.cmp(&rb.universe).then(ra.start.cmp(&rb.start)).then_with(|| scene(a, b))
        }
    });
    v
}

/// Projecteurs à ré-adresser.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Scope {
    /// seulement ceux en conflit (le premier de chaque chevauchement garde sa place)
    #[default]
    Conflicts,
    /// tout le patch
    All,
    /// ces projecteurs, en contournant les autres
    Ids(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct Options {
    pub scope: Scope,
    pub order: Order,
    /// univers de départ (`pack`)
    pub universe: u32,
    /// adresse de départ
    pub address: u32,
    /// true : tout s'enchaîne à partir de l'univers de départ ; false : chacun reste dans son univers
    pub pack: bool,
    /// adresses de départ alignées sur 1, 1+n, 1+2n… (0 : à la suite)
    pub align: u32,
    /// passer à l'univers suivant quand un univers est plein
    pub overflow: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { scope: Scope::Conflicts, order: Order::Scene, universe: 1, address: 1, pack: false, align: 0, overflow: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    pub universe: u32,
    pub address: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Move {
    pub id: String,
    pub name: String,
    pub footprint: u32,
    pub from: Place,
    pub to: Place,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub moves: Vec<Move>,
    pub unplaced: Vec<String>,
    pub considered: usize,
    pub conflicts_before: usize,
    pub conflicts_after: usize,
}

/// Calcule un nouveau patch sans rien modifier (`fixtures` : ceux de la scène, dans son ordre).
pub fn plan_addressing(fixtures: &[Fixture], opts: &Options) -> Plan {
    let start_u = opts.universe.max(1);
    let start_a = opts.address.clamp(1, UNIVERSE_SIZE);
    let align = opts.align;
    let patched: Vec<&Fixture> = fixtures.iter().filter(|f| footprint_of(f) > 0).collect();
    let before = find_patch_conflicts(patched.iter().map(|f| (f.id.as_str(), range_of(f))));

    let movable: HashSet<String> = match &opts.scope {
        Scope::Conflicts => before.ids.clone(),
        Scope::Ids(ids) => ids.iter().cloned().collect(),
        Scope::All => patched.iter().map(|f| f.id.clone()).collect(),
    };

    let mut occ = Occupancy::default();
    for f in &patched {
        if !movable.contains(&f.id) {
            let r = range_of(f);
            occ.add(r.universe, r.start, r.end);
        }
    }

    let mut target: HashMap<String, Place> = HashMap::new();
    let mut unplaced = Vec::new();
    let list: Vec<&Fixture> = patched.iter().copied().filter(|f| movable.contains(&f.id)).collect();

    let place_from = |occ: &Occupancy, u: u32, from: u32, fp: u32, wrap: bool| -> Option<Place> {
        let mut a = occ.find(u, from, fp, align);
        if a.is_none() && wrap && from > 1 {
            a = occ.find(u, 1, fp, align); // trou plus haut dans l'univers
        }
        if let Some(address) = a {
            return Some(Place { universe: u, address });
        }
        if !opts.overflow {
            return None;
        }
        (u + 1..=u + MAX_SPILL).find_map(|v| occ.find(v, 1, fp, align).map(|address| Place { universe: v, address }))
    };
    let mut commit = |occ: &mut Occupancy, f: &Fixture, pos: Option<Place>, fp: u32| -> Option<Place> {
        let Some(pos) = pos else {
            unplaced.push(f.id.clone());
            return None;
        };
        occ.add(pos.universe, pos.address, pos.address + fp - 1);
        target.insert(f.id.clone(), pos);
        Some(pos)
    };

    if opts.scope == Scope::Conflicts {
        for f in sort_fixtures(&list, Order::Address, fixtures) {
            let r = range_of(f);
            let pos = if occ.fits(r.universe, r.start, r.footprint) {
                Some(Place { universe: r.universe, address: r.start })
            } else {
                place_from(&occ, r.universe, r.start, r.footprint, true)
            };
            commit(&mut occ, f, pos, r.footprint);
        }
    } else {
        let sorted = sort_fixtures(&list, opts.order, fixtures);
        let mut sequence = |occ: &mut Occupancy, members: &[&Fixture], mut cursor: Place| {
            for f in members {
                let fp = footprint_of(f);
                let pos = place_from(occ, cursor.universe, cursor.address, fp, false);
                if let Some(pos) = commit(occ, f, pos, fp) {
                    cursor = Place { universe: pos.universe, address: pos.address + fp };
                }
            }
        };
        if opts.pack {
            sequence(&mut occ, &sorted, Place { universe: start_u, address: start_a });
        } else {
            let mut by_u: BTreeMap<u32, Vec<&Fixture>> = BTreeMap::new();
            for f in &sorted {
                by_u.entry(range_of(f).universe).or_default().push(f);
            }
            for (u, members) in &by_u {
                sequence(&mut occ, members, Place { universe: *u, address: start_a });
            }
        }
    }

    let mut moves = Vec::new();
    for f in &list {
        let Some(&to) = target.get(&f.id) else { continue };
        let r = range_of(f);
        if to.universe == r.universe && to.address == r.start {
            continue;
        }
        moves.push(Move { id: f.id.clone(), name: f.name.clone(), footprint: r.footprint, from: Place { universe: r.universe, address: r.start }, to });
    }
    let after = find_patch_conflicts(patched.iter().map(|f| {
        let r = range_of(f);
        let r = match target.get(&f.id) {
            Some(to) => Range { universe: to.universe, start: to.address, end: to.address + r.footprint - 1, footprint: r.footprint },
            None => r,
        };
        (f.id.as_str(), r)
    }));
    Plan { moves, unplaced, considered: list.len(), conflicts_before: before.ids.len(), conflicts_after: after.ids.len() }
}
