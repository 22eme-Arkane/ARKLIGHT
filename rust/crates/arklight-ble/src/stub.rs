//! Systèmes autres que Windows : pas de Bluetooth LE périphérique pour l'instant (sur téléphone,
//! l'app mobile n'en a pas besoin) ; tout renvoie [`Error::Unsupported`].

use crate::Error;
use crate::peripheral::{AdapterInfo, Shared};
use crate::profile::Profile;
use std::sync::Arc;

pub(crate) fn adapter_info() -> Result<AdapterInfo, Error> {
    Err(Error::Unsupported)
}

/// Jamais construit : `start` échoue toujours.
pub(crate) enum Backend {}

impl Backend {
    pub(crate) fn start(_profile: &Profile, _shared: &Arc<Shared>) -> Result<Backend, Error> {
        Err(Error::Unsupported)
    }

    pub(crate) fn services(&self) -> usize {
        match *self {}
    }

    pub(crate) fn notify(&self, _char: &str, _data: &[u8]) -> Result<usize, Error> {
        match *self {}
    }
}
