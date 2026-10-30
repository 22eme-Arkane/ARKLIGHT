//! Entrées MIDI de l'ordinateur (remplace Web MIDI) : sous Windows, l'API WinMM (`midiInOpen`
//! avec une fonction de rappel) ; ailleurs, aucune entrée pour l'instant.
//!
//! Les messages arrivent sur un fil de WinMM : ils sont mis en file, l'application les relève à
//! chaque image (`poll`) et les passe à [`crate::Midi::receive`]. WinMM ne signale pas les
//! branchements : `refresh` relit la liste des appareils (à appeler de temps en temps).

use crate::controller::DeviceInfo;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

/// Entrées MIDI disponibles sur ce système.
pub const SUPPORTED: bool = cfg!(windows);

/// Message reçu d'un appareil (octets d'un message court : statut et données).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawMessage {
    pub device: String,
    pub data: Vec<u8>,
}

/// Message court de WinMM (statut, données 1 et 2 dans les octets de poids faible) → octets, à la
/// longueur du message.
pub fn unpack_short_message(dw: u32) -> Vec<u8> {
    let b = dw.to_le_bytes();
    let len = match b[0] {
        0x80..=0xbf | 0xe0..=0xef | 0xf2 => 3,
        0xc0..=0xdf | 0xf1 | 0xf3 => 2,
        _ => 1,
    };
    b[..len].to_vec()
}

/// Rappel après chaque message reçu (réveille l'interface) ; appelé depuis le fil de WinMM.
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// Entrées MIDI ouvertes ; fermées quand on les lâche.
pub struct MidiInputs {
    tx: Sender<RawMessage>,
    rx: Receiver<RawMessage>,
    notify: Option<Notify>,
    #[cfg(windows)]
    open: Vec<win::Input>,
    /// appareils ouverts
    devices: Vec<DeviceInfo>,
    /// noms de tous les appareils vus à la dernière lecture (ouverts ou non)
    names: Vec<String>,
    /// appareils qui n'ont pas pu être ouverts (déjà pris par un autre logiciel…) : nom, erreur
    pub errors: Vec<(String, String)>,
    started: bool,
}

impl Default for MidiInputs {
    fn default() -> Self {
        Self::new()
    }
}

impl MidiInputs {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        MidiInputs {
            tx,
            rx,
            notify: None,
            #[cfg(windows)]
            open: Vec::new(),
            devices: Vec::new(),
            names: Vec::new(),
            errors: Vec::new(),
            started: false,
        }
    }

    /// Rappel après chaque message reçu (pour réveiller la boucle de l'interface) ; à donner avant `start`.
    pub fn set_notify(&mut self, notify: impl Fn() + Send + Sync + 'static) {
        self.notify = Some(Arc::new(notify));
    }

    /// Noms des appareils d'entrée MIDI du système (sans les ouvrir).
    pub fn list() -> Vec<String> {
        #[cfg(windows)]
        {
            win::device_names()
        }
        #[cfg(not(windows))]
        {
            Vec::new()
        }
    }

    /// Ouvre tous les appareils d'entrée ; renvoie ceux qui écoutent.
    pub fn start(&mut self) -> Vec<DeviceInfo> {
        self.started = true;
        self.reopen(Self::list());
        self.devices.clone()
    }

    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Relit la liste des appareils (branchement, débranchement) et rouvre les entrées si elle a
    /// changé ; vrai si la liste a changé.
    pub fn refresh(&mut self) -> bool {
        if !self.started {
            return false;
        }
        let names = Self::list();
        if names == self.names {
            return false;
        }
        self.reopen(names);
        true
    }

    fn reopen(&mut self, names: Vec<String>) {
        self.close();
        self.errors.clear();
        #[cfg(windows)]
        for (i, name) in names.iter().enumerate() {
            match win::Input::open(i as u32, name.clone(), self.tx.clone(), self.notify.clone()) {
                Ok(input) => {
                    self.open.push(input);
                    self.devices.push(DeviceInfo { id: format!("winmm:{i}:{name}"), name: name.clone(), manufacturer: String::new() });
                }
                Err(e) => self.errors.push((name.clone(), e)),
            }
        }
        self.names = names;
    }

    fn close(&mut self) {
        #[cfg(windows)]
        self.open.clear();
        self.devices.clear();
    }

    /// Ferme toutes les entrées.
    pub fn stop(&mut self) {
        self.close();
        self.names.clear();
        self.started = false;
    }

    /// Appareils qui écoutent.
    pub fn devices(&self) -> &[DeviceInfo] {
        &self.devices
    }

    /// Messages reçus depuis le dernier appel, dans l'ordre.
    pub fn poll(&self) -> Vec<RawMessage> {
        self.rx.try_iter().collect()
    }

    /// Ajoute un message comme s'il venait d'un appareil (essais, clavier virtuel).
    pub fn inject(&self, device: &str, data: &[u8]) {
        let _ = self.tx.send(RawMessage { device: device.to_owned(), data: data.to_vec() });
        if let Some(n) = &self.notify {
            n();
        }
    }
}

impl Drop for MidiInputs {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(windows)]
mod win {
    use super::{Notify, RawMessage, unpack_short_message};
    use std::sync::Mutex;
    use std::sync::mpsc::Sender;
    use windows_sys::Win32::Media::Audio::{CALLBACK_FUNCTION, HMIDIIN, MIDIINCAPSW, midiInClose, midiInGetDevCapsW, midiInGetNumDevs, midiInOpen, midiInReset, midiInStart, midiInStop};
    use windows_sys::Win32::Media::{MM_MIM_DATA, MMSYSERR_NOERROR};

    /// Noms des appareils d'entrée (« MIDI n » si le pilote ne répond pas).
    pub fn device_names() -> Vec<String> {
        let n = unsafe { midiInGetNumDevs() };
        (0..n)
            .map(|i| {
                let mut caps = MIDIINCAPSW::default();
                let r = unsafe { midiInGetDevCapsW(i as usize, &mut caps, size_of::<MIDIINCAPSW>() as u32) };
                if r != MMSYSERR_NOERROR {
                    return format!("MIDI {}", i + 1);
                }
                let raw = caps.szPname; // copie : la structure est « packed »
                let end = raw.iter().position(|c| *c == 0).unwrap_or(raw.len());
                String::from_utf16_lossy(&raw[..end]).trim().to_owned()
            })
            .collect()
    }

    /// Ce que reçoit la fonction de rappel (gardé en vie jusqu'à la fermeture de l'entrée).
    struct Instance {
        device: String,
        tx: Mutex<Sender<RawMessage>>,
        notify: Option<Notify>,
    }

    /// Rappel de WinMM (fil du pilote) : seuls les messages courts sont gardés (pas de sysex).
    unsafe extern "system" fn on_message(_h: HMIDIIN, msg: u32, instance: usize, param1: usize, _param2: usize) {
        if msg != MM_MIM_DATA || instance == 0 {
            return;
        }
        // SAFETY : `instance` pointe sur l'`Instance` de l'entrée, libérée seulement après midiInClose
        let inst = unsafe { &*(instance as *const Instance) };
        if let Ok(tx) = inst.tx.lock() {
            let _ = tx.send(RawMessage { device: inst.device.clone(), data: unpack_short_message(param1 as u32) });
        }
        if let Some(n) = &inst.notify {
            n();
        }
    }

    /// Une entrée ouverte.
    pub struct Input {
        handle: HMIDIIN,
        instance: *mut Instance,
    }

    // SAFETY : la poignée WinMM s'utilise depuis n'importe quel fil ; l'instance n'est lue que par le rappel
    unsafe impl Send for Input {}

    impl Input {
        pub fn open(index: u32, device: String, tx: Sender<RawMessage>, notify: Option<Notify>) -> Result<Input, String> {
            let instance = Box::into_raw(Box::new(Instance { device, tx: Mutex::new(tx), notify }));
            let callback: unsafe extern "system" fn(HMIDIIN, u32, usize, usize, usize) = on_message;
            let mut handle: HMIDIIN = std::ptr::null_mut();
            let r = unsafe { midiInOpen(&mut handle, index, callback as usize, instance as usize, CALLBACK_FUNCTION) };
            if r != MMSYSERR_NOERROR {
                // SAFETY : l'entrée n'est pas ouverte, personne d'autre ne tient l'instance
                drop(unsafe { Box::from_raw(instance) });
                return Err(format!("midiInOpen : erreur {r}"));
            }
            let input = Input { handle, instance };
            let r = unsafe { midiInStart(handle) };
            if r != MMSYSERR_NOERROR {
                return Err(format!("midiInStart : erreur {r}")); // `input` se ferme en partant
            }
            Ok(input)
        }
    }

    impl Drop for Input {
        fn drop(&mut self) {
            unsafe {
                midiInStop(self.handle);
                midiInReset(self.handle);
                midiInClose(self.handle);
                // SAFETY : après midiInClose, WinMM n'appelle plus le rappel
                drop(Box::from_raw(self.instance));
            }
        }
    }
}
