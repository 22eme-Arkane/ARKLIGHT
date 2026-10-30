//! Contrôleurs MIDI d'ARKLIGHT (portage de `shared/midi.js`, `shared/midi-presets.js`, et de la
//! logique de `web/js/midi.js` et `web/js/ui/midi-panel.js`) : décodage des messages,
//! correspondances « commande du contrôleur → fonction d'ARKLIGHT », préréglages de consoles
//! connues, apprentissage, et entrées MIDI de Windows (WinMM).
//!
//! Utilisation par l'application :
//! - au démarrage : `Midi::from_settings(&réglages)`, `MidiInputs::start()` puis `Midi::set_inputs` ;
//! - à chaque image : `for m in inputs.poll() { actions.extend(midi.receive(&m.data, &m.device, &ctx)) }`
//!   puis appliquer les [`Action`] au projet ;
//! - de temps en temps : `inputs.refresh()` (branchements) ;
//! - après un changement des correspondances : enregistrer `midi.settings()`.

pub mod controller;
pub mod input;
pub mod message;
pub mod presets;

pub use controller::{Action, ColorChange, DeviceInfo, EnvChange, FaderMark, Learning, Midi, MidiContext, WizardKind, plan_ids};
pub use input::{MidiInputs, RawMessage};
pub use message::{Mapping, MidiMessage, MsgType, Source, Target, parse_midi};
pub use presets::{MIDI_PRESETS, Preset, preset_by_id};
