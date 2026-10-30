//! Bluetooth LE de Windows (API WinRT `GattServiceProvider`), portage de `ble/bridge.py`.
//!
//! Un `GattServiceProvider` par service du profil, une `GattLocalCharacteristic` par
//! caractéristique, avec trois gestionnaires (écriture, lecture, abonnements) ; l'annonce démarre
//! service par service. Les gestionnaires sont appelés sur des fils du système (MTA) : ils
//! attendent les opérations asynchrones de WinRT (`join`), ce qui y est permis.
//!
//! Les appels de démarrage et de lecture de l'adaptateur se font sur un fil de travail : le fil de
//! l'interface (STA) n'attend jamais lui-même une opération WinRT.

use crate::Error;
use crate::peripheral::{
    AdapterInfo, AdvertisingEvent, Client, Event, ReadEvent, ReadReply, Shared, SubscribeEvent, WriteEvent, adv_status_text, bluetooth_error_text, radio_text,
};
use crate::profile::{Profile, Prop, Service};
use crate::uuid::{normalize_uuid, parse_uuid};
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristicProperties, GattLocalCharacteristic, GattLocalCharacteristicParameters, GattProtectionLevel, GattReadRequestedEventArgs, GattServiceProvider,
    GattServiceProviderAdvertisementStatusChangedEventArgs, GattServiceProviderAdvertisingParameters, GattSession, GattWriteOption, GattWriteRequestedEventArgs,
};
use windows::Devices::Bluetooth::{BluetoothAdapter, BluetoothError};
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::{DataReader, DataWriter, IBuffer};
use windows::core::{GUID, HSTRING, IInspectable, Ref};

type WinResult<T> = windows::core::Result<T>;

impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Error::Bluetooth(format!("erreur Windows : {e}"))
    }
}

/// Exécute `f` sur un fil de travail (MTA implicite) et attend son résultat.
fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| match s.spawn(f).join() {
        Ok(v) => v,
        Err(panic) => std::panic::resume_unwind(panic),
    })
}

/// Opération WinRT qui a renvoyé un objet nul (pas d'adaptateur, pas de radio, tampon vide…).
fn is_null_result(e: &windows::core::Error) -> bool {
    e.code().is_ok()
}

// ---------------------------------------------------------------------------------- adaptateur

pub(crate) fn adapter_info() -> Result<AdapterInfo, Error> {
    on_worker(read_adapter)
}

fn read_adapter() -> Result<AdapterInfo, Error> {
    let adapter = match BluetoothAdapter::GetDefaultAsync()?.join() {
        Ok(a) => a,
        Err(e) if is_null_result(&e) => return Ok(AdapterInfo::absent()),
        Err(e) => return Err(e.into()),
    };
    // radio illisible (accès refusé…) → état inconnu, comme le pont Python
    let state = adapter.GetRadioAsync().and_then(|op| op.join()).and_then(|radio| radio.State()).map(|s| s.0).unwrap_or(0);
    Ok(AdapterInfo {
        present: true,
        address: Some(format!("{:012X}", adapter.BluetoothAddress()?)),
        peripheral_role: adapter.IsPeripheralRoleSupported().unwrap_or(false),
        // propriété absente avant Windows 10 2004
        extended_advertising: adapter.IsExtendedAdvertisingSupported().ok(),
        radio: radio_text(state),
        radio_on: state == 1,
    })
}

// ---------------------------------------------------------------------------------- tampons

fn to_buffer(data: &[u8]) -> WinResult<IBuffer> {
    let writer = DataWriter::new()?;
    writer.WriteBytes(data)?;
    writer.DetachBuffer()
}

fn from_buffer(buffer: WinResult<IBuffer>) -> WinResult<Vec<u8>> {
    let buffer = match buffer {
        Ok(b) => b,
        Err(e) if is_null_result(&e) => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let len = buffer.Length()? as usize;
    if len == 0 {
        return Ok(Vec::new());
    }
    let reader = DataReader::FromBuffer(&buffer)?;
    let mut out = vec![0; len];
    reader.ReadBytes(&mut out)?;
    Ok(out)
}

fn guid(uuid: &str) -> Result<GUID, Error> {
    parse_uuid(uuid).map(GUID::from_u128).ok_or_else(|| Error::InvalidUuid(uuid.to_string()))
}

fn session_device(session: WinResult<GattSession>) -> Option<String> {
    session.and_then(|s| s.DeviceId()).and_then(|id| id.Id()).ok().map(|id| id.to_string())
}

/// Corps d'un gestionnaire WinRT : une erreur devient un [`Event::Error`], une panique est rattrapée
/// (elle ne doit pas traverser l'appel du système) ; le système reçoit toujours « réussi ».
fn guarded(shared: &Shared, context: &str, body: impl FnOnce() -> WinResult<()>) -> WinResult<()> {
    match std::panic::catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => shared.emit(Event::Error(format!("{context} : {e}"))),
        Err(_) => shared.emit(Event::Error(format!("{context} : erreur interne"))),
    }
    Ok(())
}

// ---------------------------------------------------------------------------------- publication

/// Caractéristique publiée et ses gestionnaires (retirés quand on la lâche).
struct LocalChar {
    local: GattLocalCharacteristic,
    write: Option<i64>,
    read: Option<i64>,
    subscribe: Option<i64>,
}

impl Drop for LocalChar {
    fn drop(&mut self) {
        if let Some(t) = self.write.take() {
            let _ = self.local.RemoveWriteRequested(t);
        }
        if let Some(t) = self.read.take() {
            let _ = self.local.RemoveReadRequested(t);
        }
        if let Some(t) = self.subscribe.take() {
            let _ = self.local.RemoveSubscribedClientsChanged(t);
        }
    }
}

/// Service publié : l'annonce s'arrête quand on le lâche (puis ses caractéristiques sont retirées).
struct Provider {
    provider: GattServiceProvider,
    status: Option<i64>,
    chars: Vec<LocalChar>,
}

impl Drop for Provider {
    fn drop(&mut self) {
        let _ = self.provider.StopAdvertising();
        if let Some(t) = self.status.take() {
            let _ = self.provider.RemoveAdvertisementStatusChanged(t);
        }
    }
}

pub(crate) struct Backend {
    providers: Vec<Provider>,
    /// UUID de caractéristique → caractéristique publiée (pour les notifications)
    chars: HashMap<String, GattLocalCharacteristic>,
}

impl Backend {
    /// Publie chaque service et démarre son annonce ; en cas d'échec, ce qui a déjà été publié est
    /// retiré.
    pub(crate) fn start(profile: &Profile, shared: &Arc<Shared>) -> Result<Backend, Error> {
        on_worker(|| {
            let mut backend = Backend { providers: Vec::new(), chars: HashMap::new() };
            for svc in &profile.services {
                backend.publish(svc, shared)?;
            }
            Ok(backend)
        })
    }

    pub(crate) fn services(&self) -> usize {
        self.providers.len()
    }

    pub(crate) fn notify(&self, char: &str, data: &[u8]) -> Result<usize, Error> {
        let local = self.chars.get(char).ok_or_else(|| Error::UnknownCharacteristic(char.to_string()))?;
        let results = local.NotifyValueAsync(&to_buffer(data)?)?.join()?;
        Ok(results.Size()? as usize)
    }

    fn publish(&mut self, svc: &Service, shared: &Arc<Shared>) -> Result<(), Error> {
        let s_uuid = normalize_uuid(&svc.uuid)?;
        let res = GattServiceProvider::CreateAsync(guid(&s_uuid)?)?.join()?;
        let err = res.Error()?;
        if err != BluetoothError::Success {
            return Err(Error::Bluetooth(format!("création du service {s_uuid} impossible ({})", bluetooth_error_text(err.0))));
        }
        let provider = res.ServiceProvider()?;
        let mut entry = Provider { provider: provider.clone(), status: None, chars: Vec::new() };
        let service = provider.Service()?;
        for ch in &svc.characteristics {
            let c_uuid = normalize_uuid(&ch.uuid)?;
            let params = GattLocalCharacteristicParameters::new()?;
            params.SetCharacteristicProperties(GattCharacteristicProperties(Prop::flags(&ch.props)))?;
            params.SetReadProtectionLevel(GattProtectionLevel::Plain)?;
            params.SetWriteProtectionLevel(GattProtectionLevel::Plain)?;
            if let Some(description) = ch.description.as_deref().filter(|d| !d.is_empty()) {
                params.SetUserDescription(&HSTRING::from(description))?;
            }
            let cres = service.CreateCharacteristicAsync(guid(&c_uuid)?, &params)?.join()?;
            let err = cres.Error()?;
            if err != BluetoothError::Success {
                return Err(Error::Bluetooth(format!("création de la caractéristique {c_uuid} impossible ({})", bluetooth_error_text(err.0))));
            }
            let local = cres.Characteristic()?;
            // en cas d'échec au milieu, `registered` retire les gestionnaires déjà posés
            let mut registered = LocalChar { local: local.clone(), write: None, read: None, subscribe: None };
            registered.write = Some(local.WriteRequested(&write_handler(shared.clone(), &s_uuid, &c_uuid))?);
            registered.read = Some(local.ReadRequested(&read_handler(shared.clone(), &s_uuid, &c_uuid))?);
            registered.subscribe = Some(local.SubscribedClientsChanged(&subscribe_handler(shared.clone(), &s_uuid, &c_uuid))?);
            entry.chars.push(registered);
            self.chars.insert(c_uuid, local);
        }
        entry.status = Some(provider.AdvertisementStatusChanged(&status_handler(shared.clone(), &s_uuid))?);
        let adv = GattServiceProviderAdvertisingParameters::new()?;
        adv.SetIsConnectable(true)?;
        adv.SetIsDiscoverable(svc.advertise)?;
        if let Some(data) = svc.service_data.as_deref().filter(|d| !d.is_empty()) {
            adv.SetServiceData(&to_buffer(data)?)?;
        }
        // rangé avant de démarrer : un échec plus loin arrête aussi ce service
        self.providers.push(entry);
        provider.StartAdvertisingWithParameters(&adv)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------------- gestionnaires

fn write_handler(shared: Arc<Shared>, service: &str, char: &str) -> TypedEventHandler<GattLocalCharacteristic, GattWriteRequestedEventArgs> {
    let (service, char) = (service.to_string(), char.to_string());
    TypedEventHandler::new(move |_sender: Ref<GattLocalCharacteristic>, args: Ref<GattWriteRequestedEventArgs>| {
        guarded(&shared, &format!("écriture {char}"), || {
            let args = args.ok()?;
            let deferral = args.GetDeferral()?;
            let device = session_device(args.Session());
            let request = (|| {
                let req = args.GetRequestAsync()?.join()?;
                let value = from_buffer(req.Value())?;
                let offset = req.Offset()?;
                let with_response = req.Option()? == GattWriteOption::WriteWithResponse;
                // accusé tout de suite : le téléphone n'attend pas la logique de l'émulateur
                if with_response {
                    req.Respond()?;
                }
                WinResult::Ok((value, offset, with_response))
            })();
            let _ = deferral.Complete();
            let (value, offset, with_response) = request?;
            shared.emit(Event::Write(WriteEvent { service: service.clone(), char: char.clone(), value, offset, with_response, device }));
            Ok(())
        })
    })
}

fn read_handler(shared: Arc<Shared>, service: &str, char: &str) -> TypedEventHandler<GattLocalCharacteristic, GattReadRequestedEventArgs> {
    let (service, char) = (service.to_string(), char.to_string());
    TypedEventHandler::new(move |_sender: Ref<GattLocalCharacteristic>, args: Ref<GattReadRequestedEventArgs>| {
        guarded(&shared, &format!("lecture {char}"), || {
            let args = args.ok()?;
            let deferral = args.GetDeferral()?;
            let device = session_device(args.Session());
            let result = (|| {
                let req = args.GetRequestAsync()?.join()?;
                let offset = req.Offset()?;
                let reply = ReadReply::default();
                shared.emit(Event::Read(ReadEvent { service: service.clone(), char: char.clone(), offset, device, reply: reply.clone() }));
                let value = shared.read_value(&char, &reply, offset);
                req.RespondWithValue(&to_buffer(&value)?)
            })();
            let _ = deferral.Complete();
            result
        })
    })
}

fn subscribe_handler(shared: Arc<Shared>, service: &str, char: &str) -> TypedEventHandler<GattLocalCharacteristic, IInspectable> {
    let (service, char) = (service.to_string(), char.to_string());
    TypedEventHandler::new(move |sender: Ref<GattLocalCharacteristic>, _args: Ref<IInspectable>| {
        guarded(&shared, &format!("abonnement {char}"), || {
            let clients = sender
                .ok()?
                .SubscribedClients()?
                .into_iter()
                .map(|c| Client { device: session_device(c.Session()), max_notification: c.MaxNotificationSize().unwrap_or(0) })
                .collect();
            shared.emit(Event::Subscribe(SubscribeEvent { service: service.clone(), char: char.clone(), clients }));
            Ok(())
        })
    })
}

fn status_handler(shared: Arc<Shared>, service: &str) -> TypedEventHandler<GattServiceProvider, GattServiceProviderAdvertisementStatusChangedEventArgs> {
    let service = service.to_string();
    TypedEventHandler::new(move |_sender: Ref<GattServiceProvider>, args: Ref<GattServiceProviderAdvertisementStatusChangedEventArgs>| {
        guarded(&shared, &format!("annonce {service}"), || {
            let args = args.ok()?;
            let status = adv_status_text(args.Status()?.0);
            shared.emit(Event::Advertising(AdvertisingEvent { service: service.clone(), status, error: args.Error()?.0 }));
            Ok(())
        })
    })
}
