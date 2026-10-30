//! Messages MIDI et correspondances (portage de `shared/midi.js`) : décodage des messages, clé
//! d'une commande du contrôleur, fonctions d'ARKLIGHT qu'une commande peut piloter, échelles des
//! faders, aiguillage d'un message vers les correspondances, lecture des correspondances.

use arklight_core::engine::js_round;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Sorte de message (et de commande du contrôleur).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MsgType {
    /// Control Change : fader, molette, touche
    Cc,
    Note,
    /// Program Change
    Program,
    /// Pitch bend
    Pitch,
}

impl MsgType {
    /// Nom JavaScript (« cc », « note », « program », « pitch »).
    pub fn name(self) -> &'static str {
        match self {
            MsgType::Cc => "cc",
            MsgType::Note => "note",
            MsgType::Program => "program",
            MsgType::Pitch => "pitch",
        }
    }

    pub fn parse(s: &str) -> Option<MsgType> {
        Some(match s {
            "cc" => MsgType::Cc,
            "note" => MsgType::Note,
            "program" => MsgType::Program,
            "pitch" => MsgType::Pitch,
            _ => return None,
        })
    }
}

/// Message MIDI décodé.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MidiMessage {
    pub kind: MsgType,
    /// canal 1..16
    pub channel: u8,
    pub number: u8,
    /// 0..1
    pub value: f64,
    /// note jouée, CC au-dessus de la moitié, programme, pitch bend
    pub on: bool,
}

/// Message MIDI → message décodé, ou None (horloge, sysex, aftertouch… ignorés).
pub fn parse_midi(data: &[u8]) -> Option<MidiMessage> {
    let status = *data.first()?;
    if !(0x80..0xf0).contains(&status) {
        return None;
    }
    let channel = (status & 0x0f) + 1;
    let d1 = data.get(1).copied().unwrap_or(0);
    let d2 = data.get(2).copied().unwrap_or(0);
    let m = |kind, number, value, on| Some(MidiMessage { kind, channel, number, value, on });
    match status & 0xf0 {
        0x80 => m(MsgType::Note, d1, 0.0, false),
        0x90 => m(MsgType::Note, d1, d2 as f64 / 127.0, d2 > 0),
        0xb0 => m(MsgType::Cc, d1, d2 as f64 / 127.0, d2 >= 64),
        0xc0 => m(MsgType::Program, d1, 1.0, true),
        0xe0 => m(MsgType::Pitch, 0, (((d2 as u32) << 7) | d1 as u32) as f64 / 16383.0, true),
        _ => None,
    }
}

/// Une commande du contrôleur (fader, touche…) : sorte, canal, numéro (0 pour le pitch bend).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Source {
    #[serde(rename = "type")]
    pub kind: MsgType,
    pub channel: u8,
    pub number: u8,
}

impl Source {
    /// Clé de la commande (une note et son relâchement ont la même) : « cc:1:7 ».
    pub fn key(&self) -> String {
        source_key(self.kind, self.channel, self.number)
    }
}

/// Clé d'une commande du contrôleur (comme `sourceKey`).
pub fn source_key(kind: MsgType, channel: u8, number: u8) -> String {
    format!("{}:{}:{}", kind.name(), channel, if kind == MsgType::Pitch { 0 } else { number })
}

impl MidiMessage {
    /// Commande qui a envoyé ce message.
    pub fn source(&self) -> Source {
        Source { kind: self.kind, channel: self.channel, number: if self.kind == MsgType::Pitch { 0 } else { self.number } }
    }

    pub fn key(&self) -> String {
        source_key(self.kind, self.channel, self.number)
    }

    /// Une touche appuyée (note jouée, programme, CC qui passe au-dessus de la moitié).
    pub fn is_press(&self) -> bool {
        (self.kind == MsgType::Note && self.on) || self.kind == MsgType::Program || (self.kind == MsgType::Cc && self.on)
    }
}

/// Fader, molette, pitch bend : une valeur continue (les notes et programmes sont des touches).
pub fn is_continuous(kind: MsgType) -> bool {
    matches!(kind, MsgType::Cc | MsgType::Pitch)
}

/// Nom d'une note : 60 → « C4 ».
pub fn note_name(n: u8) -> String {
    format!("{}{}", NOTE_NAMES[(n % 12) as usize], (n / 12) as i32 - 1)
}

/// « CC 7 · canal 1 », « Note 60 (C4) · canal 1 »… (en français).
pub fn describe_source(s: &Source) -> String {
    describe_source_with(s, |x| x.to_owned())
}

/// Comme [`describe_source`], textes traduits par `tr` (« canal {0} », « Note », « Programme »).
pub fn describe_source_with(s: &Source, tr: impl Fn(&str) -> String) -> String {
    let ch = tr("canal {0}").replace("{0}", &s.channel.to_string());
    match s.kind {
        MsgType::Cc => format!("CC {} · {ch}", s.number),
        MsgType::Note => format!("{} {} ({}) · {ch}", tr("Note"), s.number, note_name(s.number)),
        MsgType::Program => format!("{} {} · {ch}", tr("Programme"), s.number as u32 + 1),
        MsgType::Pitch => format!("Pitch bend · {ch}"),
    }
}

// ------------------------------------------------------------------ fonctions commandées

/// Comment une fonction suit la commande du contrôleur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetMode {
    /// suit un fader
    Continuous,
    /// à l'appui
    Button,
    /// tant qu'on appuie
    Momentary,
    /// suit l'état d'une touche à bascule (allumée / éteinte)
    Latch,
}

impl TargetMode {
    pub fn name(self) -> &'static str {
        match self {
            TargetMode::Continuous => "continuous",
            TargetMode::Button => "button",
            TargetMode::Momentary => "momentary",
            TargetMode::Latch => "latch",
        }
    }

    /// Bulle de la flèche d'une correspondance (fenêtre MIDI).
    pub fn hint(self) -> &'static str {
        match self {
            TargetMode::Continuous => "suit le fader",
            TargetMode::Latch => "suit la touche (allumée / éteinte)",
            TargetMode::Momentary => "tant qu’on appuie",
            TargetMode::Button => "à l’appui",
        }
    }
}

/// Une fonction qu'une correspondance peut commander.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetKind {
    pub id: &'static str,
    /// rubrique (liste de choix)
    pub group: &'static str,
    pub label: &'static str,
    pub mode: TargetMode,
}

const fn k(id: &'static str, group: &'static str, label: &'static str, mode: TargetMode) -> TargetKind {
    TargetKind { id, group, label, mode }
}

use TargetMode::{Button, Continuous, Latch, Momentary};

/// Fonctions qu'une correspondance peut commander, par rubrique.
pub const TARGET_KINDS: [TargetKind; 28] = [
    k("fader", "Console", "Fader de la console", Continuous),
    k("pageNext", "Console", "Faders suivants (page +1)", Button),
    k("pagePrev", "Console", "Faders précédents (page −1)", Button),
    k("familyNext", "Console", "Modèle suivant (groupe de plusieurs modèles)", Button),
    k("flash", "Console", "Flash : plein feu tant qu’on appuie", Momentary),
    k("blackout", "Console", "Blackout : tout dans le noir (touche allumée)", Latch),
    k("selectionLevel", "Intensité", "Intensité de la sélection", Continuous),
    k("groupLevel", "Intensité", "Intensité d’un groupe", Continuous),
    k("fixtureLevel", "Intensité", "Intensité d’un projecteur", Continuous),
    k("selCct", "Couleur de la sélection", "Kelvin (2000–10 000 K)", Continuous),
    k("selTint", "Couleur de la sélection", "Teinte ±vert", Continuous),
    k("selHue", "Couleur de la sélection", "Couleur (teinte 0–360°)", Continuous),
    k("selSat", "Couleur de la sélection", "Saturation", Continuous),
    k("group", "Sélection", "Sélectionner un groupe", Button),
    k("fixture", "Sélection", "Sélectionner un projecteur", Button),
    k("groupIndex", "Sélection", "Sélectionner le groupe n°", Button),
    k("fixtureIndex", "Sélection", "Sélectionner le projecteur n° (liste Scène)", Button),
    k("planIndex", "Sélection", "Ouvrir le plan n° du projet", Button),
    k("fixtureLatch", "Sélection", "Projecteur n° dans la sélection (touche à bascule)", Latch),
    k("groupLatch", "Sélection", "Groupe n° dans la sélection (touche à bascule)", Latch),
    k("groupNext", "Sélection", "Groupe suivant", Button),
    k("groupPrev", "Sélection", "Groupe précédent", Button),
    k("fixtureNext", "Sélection", "Projecteur suivant", Button),
    k("fixturePrev", "Sélection", "Projecteur précédent", Button),
    k("haze", "Plateau", "Brume", Continuous),
    k("exposure", "Plateau", "Exposition de la caméra (−10 à +3 IL)", Continuous),
    k("ambient", "Plateau", "Lumière ambiante", Continuous),
    k("command", "Commandes", "Commande d’ARKLIGHT", Button),
];

/// Rubriques des fonctions, dans l'ordre.
pub fn target_groups() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for k in &TARGET_KINDS {
        if !out.contains(&k.group) {
            out.push(k.group);
        }
    }
    out
}

pub fn kind_of(id: &str) -> Option<&'static TargetKind> {
    TARGET_KINDS.iter().find(|k| k.id == id)
}

/// Valeur d'un fader (0..1) dans les unités d'une fonction continue.
pub fn scale_value(kind: &str, v: f64) -> f64 {
    match kind {
        "selCct" => js_round((2000.0 + v * 8000.0) / 50.0) * 50.0,
        "selTint" => js_round((v * 2.0 - 1.0) * 100.0) / 100.0,
        "selHue" => js_round(v * 360.0),
        "exposure" => js_round((-10.0 + v * 13.0) * 10.0) / 10.0, // en IL
        "ambient" => js_round(v * 0.5 * 1000.0) / 1000.0,
        _ => v,
    }
}

// ------------------------------------------------------------------ correspondances

/// Fonction commandée par une correspondance : sorte (`kind`, voir [`TARGET_KINDS`]) et réglage
/// (fader ou n° `index`, groupe ou projecteur `name`, commande `id`). Les autres champs sont gardés.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Target {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Target {
    pub fn new(kind: &str) -> Self {
        Target { kind: kind.into(), ..Default::default() }
    }

    pub fn with_index(kind: &str, index: u32) -> Self {
        Target { index: Some(index), ..Target::new(kind) }
    }

    pub fn with_name(kind: &str, name: &str) -> Self {
        Target { name: Some(name.into()), ..Target::new(kind) }
    }

    pub fn command(id: &str) -> Self {
        Target { id: Some(id.into()), ..Target::new("command") }
    }

    /// N° du fader, du groupe, du projecteur ou du plan (`index || 1`).
    pub fn index_or_1(&self) -> u32 {
        self.index.filter(|i| *i != 0).unwrap_or(1)
    }

    /// Lit une fonction (objet JSON) ; un réglage d'un type inattendu reste dans `extra`.
    pub fn from_json(o: &Map<String, Value>) -> Target {
        let mut t = Target::default();
        for (key, v) in o {
            match (key.as_str(), v) {
                ("kind", Value::String(s)) => t.kind = s.clone(),
                ("index", Value::Number(n)) if n.as_u64().is_some_and(|i| i <= u32::MAX as u64) => t.index = n.as_u64().map(|i| i as u32),
                ("name", Value::String(s)) => t.name = Some(s.clone()),
                ("id", Value::String(s)) => t.id = Some(s.clone()),
                _ => {
                    t.extra.insert(key.clone(), v.clone());
                }
            }
        }
        t
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// Une correspondance « commande du contrôleur → fonction d'ARKLIGHT ».
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Mapping {
    /// vide : correspondance d'un préréglage enregistré ou d'un fichier (sans identifiant)
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// None : pas encore apprise
    pub source: Option<Source>,
    pub target: Target,
    /// repère court (« B1·3 ») affiché sur le fader de la console
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// vient d'un préréglage
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub preset: bool,
}

impl Mapping {
    pub fn new(source: Option<Source>, target: Target) -> Self {
        Mapping { id: String::new(), source, target, label: None, preset: false }
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Mode de la fonction commandée (None : fonction inconnue).
    pub fn mode(&self) -> Option<TargetMode> {
        kind_of(&self.target.kind).map(|k| k.mode)
    }
}

/// Texte tronqué à `n` unités UTF-16 (comme `slice(0, n)` en JavaScript).
pub(crate) fn js_slice(s: &str, n: usize) -> String {
    let mut used = 0;
    let mut out = String::new();
    for c in s.chars() {
        used += c.len_utf16();
        if used > n {
            break;
        }
        out.push(c);
    }
    out
}

/// Entier comme `x | 0` en JavaScript (nombre, texte, booléen ; 0 sinon).
pub(crate) fn js_int(v: Option<&Value>) -> i32 {
    fn wrap(x: f64) -> i32 {
        if !x.is_finite() {
            return 0;
        }
        (x.trunc().rem_euclid(4_294_967_296.0) as u32) as i32
    }
    match v {
        Some(Value::Number(n)) => n.as_f64().map_or(0, wrap),
        Some(Value::Bool(b)) => *b as i32,
        Some(Value::String(s)) => {
            let t = s.trim();
            let radix = |p: &str, r| t.strip_prefix(p).or_else(|| t.strip_prefix(&p.to_uppercase())).and_then(|d| u64::from_str_radix(d, r).ok()).map(|x| x as f64);
            let x = if t.is_empty() { Some(0.0) } else { radix("0x", 16).or_else(|| radix("0o", 8)).or_else(|| radix("0b", 2)).or_else(|| t.parse::<f64>().ok()) };
            x.map_or(0, wrap)
        }
        Some(Value::Array(a)) if a.len() <= 1 => a.first().map_or(0, |x| js_int(Some(x))),
        _ => 0,
    }
}

/// Identifiant neuf « m… » (temps en base 36 et compteur), comme `m${Date.now().toString(36)}…`.
pub(crate) fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
    format!("{prefix}{}{}", base36(ms), base36(COUNTER.fetch_add(1, Ordering::Relaxed)))
}

pub(crate) fn base36(mut n: u64) -> String {
    const D: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(D[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Correspondances lues (stockage, fichier) : on ne garde que ce qui est bien formé (fonction
/// connue), canal et numéro bornés, repère de 12 caractères au plus.
pub fn clean_mappings(list: &Value) -> Vec<Mapping> {
    let Some(list) = list.as_array() else { return Vec::new() };
    list.iter()
        .filter_map(|m| {
            let o = m.as_object()?;
            let target = o.get("target")?.as_object()?;
            let kind = target.get("kind")?.as_str()?;
            kind_of(kind)?;
            Some(o)
        })
        .enumerate()
        .map(|(i, o)| {
            let source = o.get("source").and_then(Value::as_object).and_then(|s| {
                let kind = MsgType::parse(s.get("type")?.as_str()?)?;
                let channel = js_int(s.get("channel")).clamp(1, 16) as u8;
                let number = js_int(s.get("number")).clamp(0, 127) as u8;
                Some(Source { kind, channel, number })
            });
            Mapping {
                id: o.get("id").and_then(Value::as_str).map_or_else(|| format!("{}{i}", new_id("m")), str::to_owned),
                source,
                target: Target::from_json(o["target"].as_object().unwrap_or(&Map::new())),
                label: o.get("label").and_then(Value::as_str).map(|l| js_slice(l, 12)),
                preset: false,
            }
        })
        .collect()
}

// ------------------------------------------------------------------ aiguillage

/// État d'une touche entre deux messages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyState {
    pub down: bool,
    pub latched: bool,
}

/// État des touches, par clé de commande.
pub type Held = HashMap<String, KeyState>;

/// Ce que déclenche un message pour une correspondance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteEvent {
    /// fonction continue : valeur 0..1 (une note : 1 jouée, 0 relâchée)
    Value(f64),
    /// touche appuyée
    Press,
    /// touche relâchée (fonction « tant qu'on appuie »)
    Release,
    /// touche à bascule allumée (vrai) ou éteinte
    Latch(bool),
}

/// Une correspondance déclenchée.
#[derive(Debug, Clone, PartialEq)]
pub struct Route<'a> {
    pub mapping: &'a Mapping,
    pub event: RouteEvent,
}

/// Ce que déclenche un message reçu :
/// - fonctions continues (faders) : une valeur ;
/// - touches : `Press` à l'appui (note jouée, programme, CC qui passe au-dessus de la moitié), et
///   `Release` au relâchement pour une fonction « tant qu'on appuie » ;
/// - touches à bascule : `Latch` quand la touche s'allume ou s'éteint. Une note rejouée sans avoir
///   été relâchée éteint la touche (consoles qui n'envoient qu'une note à chaque appui).
///
/// `held` garde l'état des touches entre deux messages.
pub fn route_midi<'a>(mappings: &'a [Mapping], msg: &MidiMessage, held: &mut Held) -> Vec<Route<'a>> {
    let key = msg.key();
    let list: Vec<&Mapping> = mappings.iter().filter(|m| m.source.is_some_and(|s| s.key() == key)).collect();
    let mut out = Vec::new();
    if list.is_empty() {
        return out;
    }
    let st = held.get(&key).copied().unwrap_or_default();
    let program = msg.kind == MsgType::Program;
    let note = msg.kind == MsgType::Note;
    let now = program || msg.on;
    // appui : une note jouée (même rejouée), un programme, un CC qui monte
    let pressed = program || (now && (!st.down || note));
    let released = !now && st.down;
    let latched = if program || (note && now && st.down) { !st.latched } else { now };
    held.insert(key, KeyState { down: if program { false } else { now }, latched });
    for m in list {
        let Some(mode) = m.mode() else { continue };
        let event = match mode {
            TargetMode::Continuous => Some(RouteEvent::Value(if note { if msg.on { 1.0 } else { 0.0 } } else { msg.value })),
            TargetMode::Latch => (latched != st.latched).then_some(RouteEvent::Latch(latched)),
            _ if pressed => Some(RouteEvent::Press),
            TargetMode::Momentary if released => Some(RouteEvent::Release),
            _ => None,
        };
        if let Some(event) = event {
            out.push(Route { mapping: m, event });
        }
    }
    out
}
