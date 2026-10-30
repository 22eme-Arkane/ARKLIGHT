//! Moteur lumière : ce que sort chaque projecteur à un instant donné (couleur de chaque pixel,
//! sRGB linéaire). Portage à l'identique de `shared/engine` (version JavaScript) :
//!
//! - `dmx` : décodage des canaux selon le mode du projecteur, stroboscope, courbes de gradation ;
//! - `effects` : programmes d'effets Astera (modes d'effet DMX, mode autonome) ;
//! - `gels` : couleurs index (gélatines LEE), couleurs du menu STATIC COLOR ;
//! - `console` : console d'ARKLIGHT (valeurs par canal, changement de profil, réglage → valeurs) ;
//! - `astera` : comportement des Astera V5 (entrées, DMX FAIL, autonomie, BlueMode) ;
//! - `output` : sortie d'un projecteur (manuel, DMX, console, tungstène et HMI).
//!
//! Les essais `tests/engine.rs` comparent les résultats à ceux de la version JavaScript.

pub mod astera;
pub mod console;
pub mod dmx;
pub mod effects;
pub mod gels;
pub mod lights;
pub mod output;

use crate::color::Rgb;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub use lights::{Light, Lights};
pub use output::{Source, compute_output};

/// Un univers DMX (512 canaux).
pub type Universe = [u8; 512];

/// Univers reçus, par numéro.
pub type Universes = HashMap<u32, Universe>;

/// Ce que voit le moteur à un instant : univers reçus, univers dont le signal arrive, heure.
pub struct Ctx<'a> {
    pub dmx: &'a Universes,
    /// univers dont les trames arrivent en ce moment (sinon : « DMX FAIL » des Astera)
    pub live: &'a HashSet<u32>,
    /// secondes
    pub time: f64,
}

/// État d'exécution d'un projecteur, gardé d'une image à l'autre (départ des effets, verrouillage
/// de l'entrée des Astera, filament des lampes…).
#[derive(Debug, Clone, Default)]
pub struct Runtime {
    pub fx: dmx::FxState,
    // Astera
    pub latch: Option<astera::Input>,
    pub input: Option<astera::Input>,
    pub signal: bool,
    pub src: Option<Source>,
    pub sa_program: Option<f64>,
    pub sa_start: Option<f64>,
    /// couleur du BlueMode choisie dans le menu (AX1), sinon bleu
    pub blue_color: Option<Rgb>,
    // tungstène et HMI
    pub last_time: Option<f64>,
    pub flux: Vec<f64>,
    pub hmi_on: Option<f64>,
    // console : valeurs suivies après un changement de profil
    pub(crate) console: Option<console::Remapped>,
}

/// `Math.round` de JavaScript (moitiés arrondies vers le haut, -2,5 → -2).
pub fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

// ------------------------------------------------------------------ lecture des réglages (JSON)
// Les réglages et l'état des projecteurs restent en JSON (champs gardés tels quels) ; ces fonctions
// les lisent avec les règles de JavaScript (`??`, `||`, valeurs « vraies »).

/// `obj.key` (absent ou null : None).
pub(crate) fn field<'a>(obj: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    obj?.get(key).filter(|v| !v.is_null())
}

/// Valeur « vraie » au sens de JavaScript.
pub(crate) fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0 && !x.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

pub(crate) fn num(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64)
}

/// `v ?? d`
pub(crate) fn num_or(v: Option<&Value>, d: f64) -> f64 {
    num(v).unwrap_or(d)
}

/// `v || d` (0 compris : valeur par défaut)
pub(crate) fn num_or_falsy(v: Option<&Value>, d: f64) -> f64 {
    match num(v) {
        Some(x) if x != 0.0 && !x.is_nan() => x,
        _ => d,
    }
}

pub(crate) fn text(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str)
}

/// Couleur `[r, g, b]` d'un champ JSON.
pub(crate) fn rgb_of(v: Option<&Value>) -> Option<Rgb> {
    let a = v?.as_array()?;
    let c = |i: usize| a.get(i).and_then(Value::as_f64).unwrap_or(0.0);
    Some([c(0), c(1), c(2)])
}
