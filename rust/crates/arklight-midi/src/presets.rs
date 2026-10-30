//! Préréglages MIDI tout prêts, par console ou contrôleur (portage de `shared/midi-presets.js`) :
//! on branche, on choisit le préréglage, tout répond. Un préréglage est une « couche » : les
//! correspondances apprises à la main passent avant lui.
//!
//! Ajouter une console : une entrée dans [`MIDI_PRESETS`] (identifiant stable, marque, nom,
//! variante, noms d'appareil USB qui font proposer le préréglage, source des numéros, premier canal
//! par défaut, résumé, fonction qui construit les correspondances).

use crate::message::{Mapping, MsgType, Source, Target};

/// Correspondance d'un préréglage avant bornage du canal (le canal peut dépasser 16).
#[derive(Debug, Clone, PartialEq)]
pub struct PresetEntry {
    pub kind: MsgType,
    pub channel: i32,
    pub number: i32,
    pub target: Target,
    pub label: Option<String>,
}

/// Un préréglage intégré.
pub struct Preset {
    /// identifiant stable (enregistré sur le poste)
    pub id: &'static str,
    pub brand: &'static str,
    pub name: &'static str,
    /// version du logiciel de la console, mode…
    pub variant: &'static str,
    /// d'où viennent les numéros (manuel, page, version)
    pub source: &'static str,
    /// premier canal MIDI de la console (réglable au chargement)
    pub channel: i32,
    /// ce que fait le préréglage, en quelques lignes (fenêtre MIDI)
    pub summary: &'static [&'static str],
    /// noms d'appareil MIDI (USB) qui font proposer ce préréglage
    matches: Option<fn(&str) -> bool>,
    build: fn(i32) -> Vec<PresetEntry>,
}

impl Preset {
    /// L'appareil (« nom fabricant ») fait-il proposer ce préréglage ?
    pub fn matches(&self, device: &str) -> bool {
        self.matches.is_some_and(|m| m(device))
    }

    /// Le préréglage est-il proposé pour certains appareils ?
    pub fn has_match(&self) -> bool {
        self.matches.is_some()
    }

    /// Correspondances construites pour ce premier canal (avant bornage).
    pub fn build(&self, channel: i32) -> Vec<PresetEntry> {
        (self.build)(channel)
    }
}

impl std::fmt::Debug for Preset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preset").field("id", &self.id).finish()
    }
}

/// Une suite de commandes : `count` commandes à partir de `first`, chacune vers `target(i)`.
fn range(kind: MsgType, channel: i32, first: i32, count: i32, target: impl Fn(i32) -> Target, label: Option<&dyn Fn(i32) -> String>) -> Vec<PresetEntry> {
    (0..count).map(|i| PresetEntry { kind, channel, number: first + i, target: target(i), label: label.map(|l| l(i)) }).collect()
}

fn one(kind: MsgType, channel: i32, number: i32, target: Target) -> PresetEntry {
    PresetEntry { kind, channel, number, target, label: None }
}

// ------------------------------------------------------------------ ADJ DMX Operator 384
// Mode MIDI : interrupteur DMX / MIDI à l'arrière, USB. Manuel ADJ, version 4 (16/08/2024), pages
// « MIDI Control » et « MIDI Implementation Chart ». Canal : Midi/Bkc maintenu + UP/DOWN.
// En V2.6, 3 canaux à la suite : banques 1 à 8 sur le 1er canal (faders), banques 1 à 4 pour les touches.
// Chaque banque a ses propres numéros : la touche BANK UP/DOWN de la console fait office de page.

fn operator384v26(ch: i32) -> Vec<PresetEntry> {
    use MsgType::{Cc, Note};
    let mut out = Vec::new();
    // faders 1 à 16 de chaque banque (8 faders, pages A/B) : banque 1 = CC 0–15 … banque 8 = CC 112–125
    out.extend(range(Cc, ch, 0, 126, |i| Target::with_index("fader", (i + 1) as u32), Some(&|i| format!("B{}·{}", i / 16 + 1, i % 16 + 1))));
    // faders Speed et Fade (CC 126 / 127 d'après la table d'implémentation, CC 32 / 33 du 2e canal d'après la table de commande)
    out.push(one(Cc, ch, 126, Target::new("selectionLevel")));
    out.push(one(Cc, ch + 1, 32, Target::new("selectionLevel")));
    out.push(one(Cc, ch, 127, Target::new("selCct")));
    out.push(one(Cc, ch + 1, 33, Target::new("selCct")));
    // touches Fixture 1–12 (à bascule) : notes 80–127 (banques 1 à 4) → projecteurs 1 à 48 de la liste Scène
    out.extend(range(Note, ch, 80, 48, |i| Target::with_index("fixtureLatch", (i + 1) as u32), None));
    // touches Scene 1–8 (poussoirs) : notes 48–79 (banques 1 à 4) → plans 1 à 32 du projet
    out.extend(range(Note, ch, 48, 32, |i| Target::with_index("planIndex", (i + 1) as u32), None));
    // touches Chase 1–12 (à bascule) : notes 0–47 (banques 1 à 4) → groupes 1 à 48 ajoutés à la sélection
    out.extend(range(Note, ch, 0, 48, |i| Target::with_index("groupLatch", (i + 1) as u32), None));
    out
}

// Logiciels avant V2.5 : un seul canal, numéros différents ; on reprend la page 1 pour les touches.
fn operator384old(ch: i32) -> Vec<PresetEntry> {
    use MsgType::{Cc, Note};
    let mut out = Vec::new();
    // pages 1 à 8 : faders 1–16 = CC 0–127 → faders 1 à 128 de la console
    out.extend(range(Cc, ch, 0, 128, |i| Target::with_index("fader", (i + 1) as u32), Some(&|i| format!("P{}·{}", i / 16 + 1, i % 16 + 1))));
    out.extend(range(Note, ch, 20, 12, |i| Target::with_index("fixtureLatch", (i + 1) as u32), None));
    out.extend(range(Note, ch, 12, 8, |i| Target::with_index("planIndex", (i + 1) as u32), None));
    out.extend(range(Note, ch, 0, 12, |i| Target::with_index("groupLatch", (i + 1) as u32), None));
    out
}

/// `/operator|dmx ?384|adj/i`
fn operator384_match(device: &str) -> bool {
    let d = device.to_ascii_lowercase();
    d.contains("operator") || d.contains("dmx384") || d.contains("dmx 384") || d.contains("adj")
}

/// Préréglages intégrés.
pub static MIDI_PRESETS: [Preset; 2] = [
    Preset {
        id: "adj-dmx-operator-384",
        brand: "ADJ",
        name: "DMX Operator 384",
        variant: "logiciel V2.6 et suivants",
        source: "Manuel ADJ DMX Operator 384, version 4 (16/08/2024) : MIDI Control et MIDI Implementation Chart (V2.6)",
        channel: 1,
        summary: &[
            "Faders 1 à 16 de chaque banque → faders de la console : banque 1 = canaux 1 à 16, banque 2 = 17 à 32… (BANK UP/DOWN sert de page)",
            "Touches Fixture (à bascule) : chaque touche allumée met son projecteur dans la sélection — projecteurs 1 à 12 de la liste Scène (banque 2 : 13 à 24…). Ctrl+G : en faire un groupe",
            "Touches Chase (à bascule) : chaque touche allumée ajoute son groupe à la sélection — groupes 1 à 12 (banque 2 : 13 à 24…)",
            "Touches Scene 1 à 8 → plans 1 à 8 du projet (banque 2 : 9 à 16…)",
            "Fader Speed → intensité de la sélection · fader Fade → Kelvin de la sélection",
            "Touche Blackout : « Apprendre une commande », appuie dessus, puis choisis « Blackout »",
        ],
        matches: Some(operator384_match),
        build: operator384v26,
    },
    Preset {
        id: "adj-dmx-operator-384-old",
        brand: "ADJ",
        name: "DMX Operator 384",
        variant: "logiciel avant V2.5",
        source: "Manuel ADJ DMX Operator 384, version 4 : MIDI Control, « for software revisions prior to V2.5 »",
        channel: 1,
        summary: &[
            "Faders 1 à 16 de chaque page → faders de la console : page 1 = canaux 1 à 16, page 2 = 17 à 32…",
            "Page 1 : touches Fixture (à bascule) → projecteurs 1 à 12 dans la sélection, Chase (à bascule) → groupes 1 à 12 dans la sélection, Scene → plans 1 à 8",
        ],
        matches: None,
        build: operator384old,
    },
];

pub fn preset_by_id(id: &str) -> Option<&'static Preset> {
    MIDI_PRESETS.iter().find(|p| p.id == id)
}

/// Libellé d'un préréglage : « ADJ DMX Operator 384 — logiciel V2.6 et suivants ».
pub fn preset_label(p: &Preset) -> String {
    let mut s = String::new();
    if !p.brand.is_empty() {
        s.push_str(p.brand);
        s.push(' ');
    }
    s.push_str(p.name);
    if !p.variant.is_empty() {
        s.push_str(" — ");
        s.push_str(p.variant);
    }
    s
}

/// Correspondances d'un préréglage (`channel` : premier canal, celui du préréglage par défaut) ;
/// les commandes hors des canaux 1 à 16 ou des numéros 0 à 127 sont écartées.
pub fn preset_mappings(preset: &Preset, channel: Option<i32>) -> Vec<Mapping> {
    preset
        .build(channel.unwrap_or(preset.channel))
        .into_iter()
        .filter(|m| (1..=16).contains(&m.channel) && (0..=127).contains(&m.number))
        .enumerate()
        .map(|(i, m)| Mapping {
            id: format!("{}:{i}", preset.id),
            source: Some(Source { kind: m.kind, channel: m.channel as u8, number: m.number as u8 }),
            target: m.target,
            label: m.label,
            preset: true,
        })
        .collect()
}
