//! RDM (ANSI E1.20) des projecteurs virtuels (portage de `shared/rdm.js`) : identifiant (UID),
//! informations, modes, adresse, description des canaux, identification. Transporté en Art-Net
//! (voir `artnet` et `receiver`) : Luminair, une console… découvrent les projecteurs de la scène
//! et peuvent les ré-adresser. Messages sans le code de départ 0xCC (comme dans ArtRdm).
//!
//! Le répondeur ne touche pas au projet : un changement demandé par une appli (adresse, mode, nom,
//! identification) revient à l'application sous forme d'[`RdmEvent`] à appliquer.

use arklight_core::library::{DmxMode, FixtureType};
use arklight_core::{Fixture, Project, library};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::net::Ipv4Addr;

/// Fabricant ESTA réservé aux prototypes : aucune appli ne confond un projecteur virtuel avec un vrai modèle.
pub const PROTOTYPE_MANUFACTURER: u16 = 0x7ff0;

/// Classes de commande.
pub mod cc {
    pub const DISCOVERY: u8 = 0x10;
    pub const DISCOVERY_RESPONSE: u8 = 0x11;
    pub const GET: u8 = 0x20;
    pub const GET_RESPONSE: u8 = 0x21;
    pub const SET: u8 = 0x30;
    pub const SET_RESPONSE: u8 = 0x31;
}

/// Type de réponse (octet 15 d'une réponse).
pub mod response {
    pub const ACK: u8 = 0x00;
    pub const NACK: u8 = 0x02;
}

/// Raisons d'un refus (NACK).
pub mod nack {
    pub const UNKNOWN_PID: u16 = 0x0000;
    pub const FORMAT_ERROR: u16 = 0x0001;
    pub const UNSUPPORTED_COMMAND_CLASS: u16 = 0x0005;
    pub const DATA_OUT_OF_RANGE: u16 = 0x0006;
    pub const SUB_DEVICE_OUT_OF_RANGE: u16 = 0x0009;
}

/// Paramètres (PID) connus.
pub mod pid {
    pub const SUPPORTED_PARAMETERS: u16 = 0x0050;
    pub const DEVICE_INFO: u16 = 0x0060;
    pub const DEVICE_MODEL_DESCRIPTION: u16 = 0x0080;
    pub const MANUFACTURER_LABEL: u16 = 0x0081;
    pub const DEVICE_LABEL: u16 = 0x0082;
    pub const SOFTWARE_VERSION_LABEL: u16 = 0x00c0;
    pub const DMX_PERSONALITY: u16 = 0x00e0;
    pub const DMX_PERSONALITY_DESCRIPTION: u16 = 0x00e1;
    pub const DMX_START_ADDRESS: u16 = 0x00f0;
    pub const SLOT_INFO: u16 = 0x0120;
    pub const SLOT_DESCRIPTION: u16 = 0x0121;
    pub const IDENTIFY_DEVICE: u16 = 0x1000;
}

const PRODUCT_FIXTURE_FIXED: u16 = 0x0101;
const SOFTWARE_VERSION: u32 = 0x0001_0000;

/// fonction d'un canal → identifiant de canal E1.20 (table C-2) et description lisible par les applis
const SLOTS: [(&str, u16, &str); 24] = [
    ("dimmer", 0x0001, "Dimmer"),
    ("red", 0x0205, "Red"),
    ("green", 0x0206, "Green"),
    ("blue", 0x0207, "Blue"),
    ("amber", 0x0211, "Amber"),
    ("white", 0x0212, "White"),
    ("ww", 0x0213, "Warm White"),
    ("cw", 0x0214, "Cool White"),
    ("uv", 0x0215, "UV"),
    ("hue", 0x0216, "Hue"),
    ("sat", 0x0217, "Saturation"),
    ("cct", 0x0208, "CCT"),
    ("strobe", 0x0404, "Strobe"),
    ("tint", 0xffff, "Green/Magenta"),
    ("lime", 0xffff, "Lime"),
    ("mint", 0xffff, "Mint"),
    ("cyan", 0xffff, "Cyan"),
    ("x", 0xffff, "CIE x"),
    ("y", 0xffff, "CIE y"),
    ("crossfade", 0xffff, "Crossfade"),
    ("index", 0xffff, "Color Index"),
    ("cctIndex", 0xffff, "CCT Index"),
    ("fan", 0xffff, "Fan"),
    ("none", 0xffff, "Reserved"),
];

// ------------------------------------------------------------------ textes

/// Texte ASCII (`max` caractères au plus) : les applis RDM n'acceptent pas les accents. Comme en
/// JavaScript : décomposition (NFD), accents retirés, « ? » pour le reste (deux au-delà de U+FFFF).
pub fn ascii(s: &str, max: usize) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let cp = c as u32;
        if (0x20..=0x7e).contains(&cp) {
            out.push(c);
        } else if cp < 0x80 {
            out.push('?');
        } else if (0xac00..=0xd7a3).contains(&cp) {
            // syllabe coréenne : deux ou trois lettres (jamos), chacune « ? »
            out.push_str(if (cp - 0xac00).is_multiple_of(28) { "??" } else { "???" });
        } else {
            let t = crate::rdm_ascii::RANGES;
            let i = t.partition_point(|r| r.1 < cp);
            match t.get(i) {
                Some(&(first, _, r)) if first <= cp => out.push_str(r),
                _ => out.push_str(if cp > 0xffff { "??" } else { "?" }),
            }
        }
        if out.len() >= max {
            break;
        }
    }
    out.truncate(max);
    out
}

fn text_bytes(s: &str) -> Vec<u8> {
    ascii(s, 32).into_bytes()
}

/// Octets reçus → texte (Latin-1), sans les zéros de fin.
fn from_ascii(bytes: &[u8]) -> String {
    let end = bytes.iter().rposition(|b| *b != 0).map_or(0, |i| i + 1);
    bytes[..end].iter().map(|b| *b as char).collect()
}

/// Espaces retirés au début et à la fin (ceux de `String.trim` en JavaScript, en Latin-1).
fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| matches!(c, '\t' | '\n' | '\u{0b}' | '\u{0c}' | '\r' | ' ' | '\u{a0}'))
}

// ------------------------------------------------------------------ identifiants

/// UID RDM : fabricant ESTA et numéro d'appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Uid {
    pub mfr: u16,
    pub dev: u32,
}

impl Uid {
    pub const fn new(mfr: u16, dev: u32) -> Self {
        Self { mfr, dev }
    }

    /// Tous les appareils (diffusion).
    pub const ALL: Uid = Uid::new(0xffff, 0xffff_ffff);

    pub fn bytes(self) -> [u8; 6] {
        let [a, b] = self.mfr.to_be_bytes();
        let [c, d, e, f] = self.dev.to_be_bytes();
        [a, b, c, d, e, f]
    }

    pub fn from_bytes(b: &[u8]) -> Option<Uid> {
        let b: &[u8; 6] = b.get(..6)?.try_into().ok()?;
        Some(Uid { mfr: u16::from_be_bytes([b[0], b[1]]), dev: u32::from_be_bytes([b[2], b[3], b[4], b[5]]) })
    }

    /// UID en texte « 7FF0:1A2B3C4D ».
    pub fn text(self) -> String {
        format!("{:04X}:{:08X}", self.mfr, self.dev)
    }
}

/// Identifiant RDM d'un projecteur (stable : tiré de son identifiant dans la scène, FNV-1a).
pub fn device_id_of(fixture_id: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for c in fixture_id.encode_utf16() {
        h = (h ^ c as u32).wrapping_mul(0x0100_0193);
    }
    if h == 0xffff_ffff || h == 0 { 1 } else { h }
}

/// Modèle RDM : numéro tiré de l'identifiant du modèle dans la bibliothèque.
fn model_id_of(type_id: &str) -> u16 {
    let mut h: u32 = 0;
    for c in type_id.encode_utf16() {
        h = (h.wrapping_mul(31) + c as u32) & 0xffff;
    }
    if h == 0 { 1 } else { h as u16 }
}

// ------------------------------------------------------------------ canaux

/// Un canal au sens RDM (SLOT_INFO, SLOT_DESCRIPTION).
#[derive(Debug, Clone, PartialEq)]
pub struct SlotInfo {
    /// 0x00 : canal principal, 0x01 : canal « fin » (ST_SEC_FINE)
    pub kind: u8,
    /// identifiant E1.20 du canal, ou numéro du canal principal pour un canal fin
    pub label: u16,
    pub text: String,
}

/// Texte d'une valeur JSON comme `String(v)` en JavaScript (`None` : `undefined`).
fn js_text(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => "null".into(),
        Some(v) => v.to_string(),
    }
}

/// Valeur « vraie » au sens de JavaScript.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// Canaux d'un mode, au sens RDM (16 bits : un canal principal puis un canal « fin »).
pub fn slots_of(mode: &DmxMode) -> Vec<SlotInfo> {
    let mut out = Vec::new();
    let mut push = |ch: &Value, px: i64| {
        let o = ch.as_object();
        let f = match ch {
            Value::String(s) => s.clone(),
            _ => js_text(o.and_then(|o| o.get("f"))),
        };
        let px = o.and_then(|o| o.get("px")).and_then(Value::as_f64).map_or(px, |p| p as i64);
        let bytes2 = o.and_then(|o| o.get("bytes")).and_then(Value::as_f64) == Some(2.0);
        let (id, name) = SLOTS.iter().find(|(k, _, _)| *k == f).map_or((0xffff, f.as_str()), |(_, id, n)| (*id, *n));
        let label = o.and_then(|o| o.get("label")).filter(|l| truthy(Some(l)));
        let mut base = label.map_or(name.to_owned(), |l| js_text(Some(l)));
        if px >= 0 {
            base.push_str(&format!(" P{}", px + 1));
        }
        let at = out.len() as u16;
        if bytes2 {
            let fine = format!("{base} fine");
            out.push(SlotInfo { kind: 0x00, label: id, text: base });
            out.push(SlotInfo { kind: 0x01, label: at, text: fine });
        } else {
            out.push(SlotInfo { kind: 0x00, label: id, text: base });
        }
    };
    for ch in &mode.channels {
        let block = ch.as_object().and_then(|o| o.get("pixels")).and_then(Value::as_f64).filter(|n| *n != 0.0);
        match block {
            Some(n) => {
                let first = ch.get("firstPixel").and_then(Value::as_f64).unwrap_or(0.0) as i64;
                let inner = ch.get("channels").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
                for p in 0..n.max(0.0).ceil() as i64 {
                    for c in inner {
                        push(c, first + p);
                    }
                }
            }
            None => push(ch, -1),
        }
    }
    out
}

// ------------------------------------------------------------------ messages

/// Message RDM (sans le code de départ 0xCC).
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub dest: Uid,
    pub src: Uid,
    /// numéro de transaction
    pub tn: u8,
    /// numéro de port (question) ou type de réponse ([`response`])
    pub port: u8,
    pub sub_device: u16,
    pub cc: u8,
    pub pid: u16,
    pub pd: Vec<u8>,
}

/// Lit un message RDM (sans 0xCC) ; None s'il est invalide.
pub fn parse(data: &[u8]) -> Option<Message> {
    if data.len() < 25 || data[0] != 0x01 {
        return None;
    }
    let pdl = data[22] as usize;
    if data.len() < 25 + pdl || data[1] as usize != 24 + pdl {
        return None;
    }
    let sum = data[..23 + pdl].iter().fold(0xccu32, |s, b| s + *b as u32);
    if ((sum & 0xffff) >> 8) as u8 != data[23 + pdl] || (sum & 0xff) as u8 != data[24 + pdl] {
        return None;
    }
    Some(Message {
        dest: Uid::from_bytes(&data[2..8])?,
        src: Uid::from_bytes(&data[8..14])?,
        tn: data[14],
        port: data[15],
        sub_device: u16::from_be_bytes([data[17], data[18]]),
        cc: data[19],
        pid: u16::from_be_bytes([data[20], data[21]]),
        pd: data[23..23 + pdl].to_vec(),
    })
}

/// Construit un message RDM (sans 0xCC) ; `port` : numéro de port ou type de réponse.
pub fn build(m: &Message) -> Vec<u8> {
    let pd = &m.pd[..m.pd.len().min(255)];
    let pdl = pd.len();
    let mut b = vec![0u8; 25 + pdl];
    b[0] = 0x01;
    b[1] = (24 + pdl) as u8;
    b[2..8].copy_from_slice(&m.dest.bytes());
    b[8..14].copy_from_slice(&m.src.bytes());
    b[14] = m.tn;
    b[15] = m.port;
    b[17..19].copy_from_slice(&m.sub_device.to_be_bytes());
    b[19] = m.cc;
    b[20..22].copy_from_slice(&m.pid.to_be_bytes());
    b[22] = pdl as u8;
    b[23..23 + pdl].copy_from_slice(pd);
    let sum = b[..23 + pdl].iter().fold(0xccu32, |s, x| s + *x as u32);
    b[23 + pdl] = (sum >> 8) as u8;
    b[24 + pdl] = sum as u8;
    b
}

// ------------------------------------------------------------------ scène et changements

/// Ce dont le répondeur a besoin : les projecteurs de la scène et leurs modèles (la bibliothèque
/// par défaut).
pub trait RdmScene {
    fn fixtures(&self) -> &[Fixture];
    fn fixture_type(&self, type_id: &str) -> Option<&FixtureType> {
        library().fixture(type_id)
    }
}

impl RdmScene for [Fixture] {
    fn fixtures(&self) -> &[Fixture] {
        self
    }
}

impl RdmScene for Vec<Fixture> {
    fn fixtures(&self) -> &[Fixture] {
        self
    }
}

impl RdmScene for Project {
    fn fixtures(&self) -> &[Fixture] {
        &self.fixtures
    }
}

/// Réglage changé par une appli.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetKind {
    /// adresse de départ (`settings.dmxAddress`)
    Address,
    /// mode DMX (`settings.dmxMode`)
    Mode,
    /// nom du projecteur
    Name,
}

impl SetKind {
    /// Nom JavaScript (« address », « mode », « name »).
    pub fn name(self) -> &'static str {
        match self {
            SetKind::Address => "address",
            SetKind::Mode => "mode",
            SetKind::Name => "name",
        }
    }
}

/// Ce que demande une appli RDM, à appliquer au projet par l'application.
#[derive(Debug, Clone, PartialEq)]
pub enum RdmEvent {
    /// une appli change l'adresse, le mode (`settings` : réglages à fusionner dans ceux du
    /// projecteur) ou le nom (`name`) d'un projecteur
    Set { fixture: String, what: SetKind, settings: Map<String, Value>, name: Option<String> },
    /// une appli demande au projecteur de se signaler (ou d'arrêter)
    Identify { fixture: String, on: bool },
    /// une appli a demandé la table des appareils (journal ; produit par le récepteur Art-Net)
    Discovery { source: Ipv4Addr, universes: usize, devices: usize },
}

impl RdmEvent {
    /// Changements au format JavaScript (`{ settings: {...} }` ou `{ name }`), pour `updateFixture`.
    pub fn changes(&self) -> Option<Value> {
        match self {
            RdmEvent::Set { settings, name, .. } => {
                let mut o = Map::new();
                if !settings.is_empty() {
                    o.insert("settings".into(), Value::Object(settings.clone()));
                }
                if let Some(n) = name {
                    o.insert("name".into(), Value::String(n.clone()));
                }
                Some(Value::Object(o))
            }
            _ => None,
        }
    }

    /// Événement au format des données d'essai (`{ type: "set", fixture, what, changes }`…).
    pub fn to_json(&self) -> Value {
        match self {
            RdmEvent::Set { fixture, what, .. } => json!({ "type": "set", "fixture": fixture, "what": what.name(), "changes": self.changes() }),
            RdmEvent::Identify { fixture, on } => json!({ "type": "identify", "fixture": fixture, "on": on }),
            RdmEvent::Discovery { source, universes, devices } => json!({ "type": "discovery", "source": source.to_string(), "universes": universes, "devices": devices }),
        }
    }

    /// Applique un changement à la liste des projecteurs (comme `SceneStore.updateFixture` : nom
    /// tronqué à 60 caractères, réglages fusionnés) ; vrai si un projecteur a changé.
    pub fn apply(&self, fixtures: &mut [Fixture]) -> bool {
        let RdmEvent::Set { fixture, settings, name, .. } = self else { return false };
        let Some(f) = fixtures.iter_mut().find(|f| f.id == *fixture) else { return false };
        if let Some(n) = name {
            let units: Vec<u16> = n.encode_utf16().take(60).collect();
            f.name = String::from_utf16_lossy(&units);
        }
        if !settings.is_empty()
            && let Ok(Value::Object(mut cur)) = serde_json::to_value(&f.settings)
        {
            for (k, v) in settings {
                cur.insert(k.clone(), v.clone());
            }
            if let Ok(s) = serde_json::from_value(Value::Object(cur)) {
                f.settings = s;
            }
        }
        true
    }
}

// ------------------------------------------------------------------ répondeur

/// Un projecteur pilotable en DMX d'un univers, avec son UID.
pub struct Device<'a> {
    pub fixture: &'a Fixture,
    pub fixture_type: &'a FixtureType,
    pub uid: Uid,
}

/// Répondeur RDM de la scène (garde seulement les projecteurs en identification).
#[derive(Debug, Default, Clone)]
pub struct RdmResponder {
    /// ids des projecteurs en identification
    pub identifying: HashSet<String>,
}

fn u16b(v: u32) -> [u8; 2] {
    [(v >> 8) as u8, v as u8]
}

impl RdmResponder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Projecteurs pilotables en DMX de cet univers, avec leur UID.
    pub fn devices<'a, S: RdmScene + ?Sized>(&self, scene: &'a S, universe: u32) -> Vec<Device<'a>> {
        let mut out = Vec::new();
        let mut used = HashSet::new();
        for f in scene.fixtures() {
            let Some(ty) = scene.fixture_type(&f.type_id) else { continue };
            if ty.dmx.modes.is_empty() || f.settings.universe != Some(universe) {
                continue;
            }
            let mut dev = device_id_of(&f.id);
            while used.contains(&dev) {
                dev = dev.wrapping_add(1); // deux identifiants identiques : très improbable
            }
            used.insert(dev);
            out.push(Device { fixture: f, fixture_type: ty, uid: Uid::new(PROTOTYPE_MANUFACTURER, dev) });
        }
        out
    }

    /// Table des appareils (ToD) d'un univers : UID de 6 octets.
    pub fn tod<S: RdmScene + ?Sized>(&self, scene: &S, universe: u32) -> Vec<[u8; 6]> {
        self.devices(scene, universe).iter().map(|d| d.uid.bytes()).collect()
    }

    /// Répond à un message RDM reçu sur cet univers ; None si aucune réponse n'est due. Les
    /// changements demandés sont ajoutés à `events` (à appliquer au projet).
    pub fn handle<S: RdmScene + ?Sized>(&mut self, scene: &S, universe: u32, data: &[u8], events: &mut Vec<RdmEvent>) -> Option<Vec<u8>> {
        let req = parse(data)?;
        let broadcast = req.dest.dev == 0xffff_ffff && (req.dest.mfr == 0xffff || req.dest.mfr == PROTOTYPE_MANUFACTURER);
        let targets: Vec<Device> = self.devices(scene, universe).into_iter().filter(|d| broadcast || d.uid == req.dest).collect();
        if targets.is_empty() || req.cc == cc::DISCOVERY {
            return None; // la découverte passe par la ToD d'Art-Net
        }
        let mut reply = None;
        for d in &targets {
            let r = self.answer(d, &req, events);
            if !broadcast {
                reply = Some(r);
            }
        }
        reply
    }

    fn answer(&mut self, d: &Device, req: &Message, events: &mut Vec<RdmEvent>) -> Vec<u8> {
        let reply = |port: u8, pd: Vec<u8>| build(&Message { dest: req.src, src: d.uid, tn: req.tn, port, sub_device: req.sub_device, cc: req.cc.wrapping_add(1), pid: req.pid, pd });
        let ack = |pd: Vec<u8>| reply(response::ACK, pd);
        let refuse = |reason: u16| reply(response::NACK, reason.to_be_bytes().to_vec());
        if req.sub_device != 0 && req.sub_device != 0xffff {
            return refuse(nack::SUB_DEVICE_OUT_OF_RANGE);
        }
        let (f, ty) = (d.fixture, d.fixture_type);
        let modes = &ty.dmx.modes;
        let dmx_mode = f.settings.dmx_mode.as_deref();
        let current = modes.iter().position(|m| Some(m.id.as_str()) == dmx_mode).unwrap_or(0);
        let mode = ty.mode(dmx_mode).unwrap_or(&modes[0]);
        let footprint = mode.footprint() as u32;
        let mode_count = modes.len().min(255) as u8;
        let address = if footprint > 0 { f.settings.dmx_address.unwrap_or(0) } else { 0xffff };
        let get = req.cc == cc::GET;
        let set = req.cc == cc::SET;
        if !get && !set {
            return refuse(nack::UNSUPPORTED_COMMAND_CLASS);
        }
        let pd = &req.pd;
        let set_event = |what: SetKind, settings: Map<String, Value>, name: Option<String>| RdmEvent::Set { fixture: f.id.clone(), what, settings, name };
        let get_only = |pd: Vec<u8>| if get { ack(pd) } else { refuse(nack::UNSUPPORTED_COMMAND_CLASS) };
        match req.pid {
            pid::SUPPORTED_PARAMETERS => {
                let list = [pid::DEVICE_MODEL_DESCRIPTION, pid::MANUFACTURER_LABEL, pid::DEVICE_LABEL, pid::DMX_PERSONALITY, pid::DMX_PERSONALITY_DESCRIPTION, pid::SLOT_INFO, pid::SLOT_DESCRIPTION];
                get_only(list.iter().flat_map(|p| p.to_be_bytes()).collect())
            }
            pid::DEVICE_INFO => {
                let mut b = Vec::with_capacity(19);
                b.extend(0x0100u16.to_be_bytes());
                b.extend(model_id_of(&ty.id).to_be_bytes());
                b.extend(PRODUCT_FIXTURE_FIXED.to_be_bytes());
                b.extend(SOFTWARE_VERSION.to_be_bytes());
                b.extend(u16b(footprint));
                b.extend([(current + 1) as u8, mode_count]);
                b.extend(u16b(address));
                b.extend([0, 0, 0]); // ni sous-appareils ni capteurs
                get_only(b)
            }
            pid::DEVICE_MODEL_DESCRIPTION => get_only(text_bytes(&ty.name)),
            pid::MANUFACTURER_LABEL => get_only(text_bytes(&ty.brand)),
            pid::SOFTWARE_VERSION_LABEL => get_only(text_bytes("ARKLIGHT 1.0")),
            pid::DEVICE_LABEL => {
                if get {
                    return ack(text_bytes(&f.name));
                }
                if pd.len() > 32 {
                    return refuse(nack::FORMAT_ERROR);
                }
                let text = from_ascii(pd);
                let name = Some(js_trim(&text)).filter(|n| !n.is_empty()).unwrap_or(&f.name).to_owned();
                events.push(set_event(SetKind::Name, Map::new(), Some(name)));
                ack(Vec::new())
            }
            pid::DMX_PERSONALITY => {
                if get {
                    return ack(vec![(current + 1) as u8, mode_count]);
                }
                if pd.len() != 1 {
                    return refuse(nack::FORMAT_ERROR);
                }
                let n = pd[0] as usize;
                if n < 1 || n > modes.len() {
                    return refuse(nack::DATA_OUT_OF_RANGE);
                }
                let mut s = Map::new();
                s.insert("dmxMode".into(), Value::String(modes[n - 1].id.clone()));
                events.push(set_event(SetKind::Mode, s, None));
                ack(Vec::new())
            }
            pid::DMX_PERSONALITY_DESCRIPTION => {
                if !get {
                    return refuse(nack::UNSUPPORTED_COMMAND_CLASS);
                }
                if pd.len() != 1 {
                    return refuse(nack::FORMAT_ERROR);
                }
                let n = pd[0] as usize;
                let Some(m) = n.checked_sub(1).and_then(|i| modes.get(i)) else { return refuse(nack::DATA_OUT_OF_RANGE) };
                let size = ty.mode(Some(m.id.as_str())).map_or(0, |x| x.footprint()) as u32;
                let label = m.extra.get("label").filter(|l| truthy(Some(l))).map_or(m.name.clone(), |l| js_text(Some(l)));
                let mut b = vec![n as u8];
                b.extend(u16b(size));
                b.extend(text_bytes(&label));
                ack(b)
            }
            pid::DMX_START_ADDRESS => {
                if get {
                    return ack(u16b(address).to_vec());
                }
                if pd.len() != 2 {
                    return refuse(nack::FORMAT_ERROR);
                }
                let a = u16::from_be_bytes([pd[0], pd[1]]) as u32;
                if !(1..=512).contains(&a) || a + footprint > 513 {
                    return refuse(nack::DATA_OUT_OF_RANGE);
                }
                let mut s = Map::new();
                s.insert("dmxAddress".into(), Value::from(a));
                events.push(set_event(SetKind::Address, s, None));
                ack(Vec::new())
            }
            pid::SLOT_INFO => {
                if !get {
                    return refuse(nack::UNSUPPORTED_COMMAND_CLASS);
                }
                // une réponse tient 231 octets de données (46 canaux) ; au-delà, les applis demandent SLOT_DESCRIPTION
                let mut b = Vec::new();
                for (i, s) in slots_of(mode).iter().take(46).enumerate() {
                    b.extend((i as u16).to_be_bytes());
                    b.push(s.kind);
                    b.extend(s.label.to_be_bytes());
                }
                ack(b)
            }
            pid::SLOT_DESCRIPTION => {
                if !get {
                    return refuse(nack::UNSUPPORTED_COMMAND_CLASS);
                }
                if pd.len() != 2 {
                    return refuse(nack::FORMAT_ERROR);
                }
                let n = u16::from_be_bytes([pd[0], pd[1]]);
                let Some(s) = slots_of(mode).into_iter().nth(n as usize) else { return refuse(nack::DATA_OUT_OF_RANGE) };
                let mut b = n.to_be_bytes().to_vec();
                b.extend(text_bytes(&s.text));
                ack(b)
            }
            pid::IDENTIFY_DEVICE => {
                if get {
                    return ack(vec![self.identifying.contains(&f.id) as u8]);
                }
                if pd.len() != 1 || pd[0] > 1 {
                    return refuse(nack::FORMAT_ERROR);
                }
                let on = pd[0] == 1;
                if on {
                    self.identifying.insert(f.id.clone());
                } else {
                    self.identifying.remove(&f.id);
                }
                events.push(RdmEvent::Identify { fixture: f.id.clone(), on });
                ack(Vec::new())
            }
            _ => refuse(nack::UNKNOWN_PID),
        }
    }
}
