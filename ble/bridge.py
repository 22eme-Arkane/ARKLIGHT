#!/usr/bin/env python
"""
Pont Bluetooth LE d'ARKLIGHT (Windows).

Fait du PC un périphérique Bluetooth LE (par exemple une fausse AsteraBox) grâce à l'API Windows
GattServiceProvider. Le pont est volontairement "bête" : il publie la table GATT que lui décrit
le serveur ARKLIGHT, lui transmet chaque écriture / lecture / abonnement venant du téléphone, et
envoie les notifications que le serveur lui demande. Toute la logique du protocole reste dans le
serveur Node (server/ble/).

Usage :
    .venv\\Scripts\\python.exe ble\\bridge.py                 # se connecte à ws://127.0.0.1:8686/bridge
    .venv\\Scripts\\python.exe ble\\bridge.py --autonome      # profil de test, sans serveur (écho)
"""
import argparse
import asyncio
import json
import logging
import sys
import time
import uuid

from winrt.windows.devices.bluetooth import BluetoothAdapter
from winrt.windows.devices.bluetooth.genericattributeprofile import (
    GattCharacteristicProperties as Props,
    GattLocalCharacteristicParameters,
    GattProtectionLevel,
    GattServiceProvider,
    GattServiceProviderAdvertisingParameters,
    GattWriteOption,
)
from winrt.windows.storage.streams import DataReader, DataWriter

log = logging.getLogger("arklight.ble")

PROP_FLAGS = {
    "read": Props.READ,
    "write": Props.WRITE,
    "writeNoResp": Props.WRITE_WITHOUT_RESPONSE,
    "notify": Props.NOTIFY,
    "indicate": Props.INDICATE,
}

ADV_STATUS = {0: "créé", 1: "arrêté", 2: "en cours", 3: "abandonné", 4: "démarré sans tous les services"}

# Profil de test : permet de vérifier toute la chaîne avec une appli générique (nRF Connect, LightBlue).
TEST_PROFILE = {
    "name": "ARKLIGHT Test",
    "services": [
        {
            "uuid": "a7c1a000-0000-4c1e-8000-41524b4c4954",
            "advertise": True,
            "characteristics": [
                {"uuid": "a7c1a001-0000-4c1e-8000-41524b4c4954", "props": ["write", "writeNoResp"]},
                {"uuid": "a7c1a002-0000-4c1e-8000-41524b4c4954", "props": ["read", "notify"], "value": "41524b4c49474854"},
            ],
        }
    ],
}


def to_buffer(data: bytes):
    w = DataWriter()
    w.write_bytes(list(data))
    return w.detach_buffer()


def from_buffer(buf) -> bytes:
    if buf is None or buf.length == 0:
        return b""
    try:
        return bytes(memoryview(buf))
    except TypeError:
        reader = DataReader.from_buffer(buf)
        out = bytearray(buf.length)
        reader.read_bytes(out)
        return bytes(out)


class Peripheral:
    """Publication d'une table GATT et relais des événements vers `emit`."""

    def __init__(self, loop: asyncio.AbstractEventLoop, emit):
        self.loop = loop
        self.emit = emit
        self.providers = []  # (uuid, provider)
        self.chars = {}  # uuid caractéristique -> (uuid service, GattLocalCharacteristic)
        self.values = {}  # uuid caractéristique -> bytes (réponses aux lectures)
        self.pending_reads = {}
        self.read_seq = 0
        self.ask_core_on_read = False

    # ------------------------------------------------------------ publication
    async def apply(self, profile: dict):
        self.stop()
        self.ask_core_on_read = bool(profile.get("askCoreOnRead"))
        for svc in profile.get("services", []):
            s_uuid = str(uuid.UUID(svc["uuid"]))
            res = await GattServiceProvider.create_async(uuid.UUID(s_uuid))
            if res.error != 0:
                raise RuntimeError(f"création du service {s_uuid} impossible (erreur Bluetooth {res.error})")
            provider = res.service_provider
            for ch in svc.get("characteristics", []):
                c_uuid = str(uuid.UUID(ch["uuid"]))
                params = GattLocalCharacteristicParameters()
                flags = 0
                for p in ch.get("props", []):
                    flags |= PROP_FLAGS[p]
                params.characteristic_properties = Props(flags)
                params.read_protection_level = GattProtectionLevel.PLAIN
                params.write_protection_level = GattProtectionLevel.PLAIN
                if ch.get("description"):
                    params.user_description = ch["description"]
                cres = await provider.service.create_characteristic_async(uuid.UUID(c_uuid), params)
                if cres.error != 0:
                    raise RuntimeError(f"création de la caractéristique {c_uuid} impossible (erreur {cres.error})")
                local = cres.characteristic
                local.add_write_requested(lambda sender, args, s=s_uuid, c=c_uuid: self._on_write(s, c, args))
                local.add_read_requested(lambda sender, args, s=s_uuid, c=c_uuid: self._on_read(s, c, args))
                local.add_subscribed_clients_changed(lambda sender, args, s=s_uuid, c=c_uuid: self._on_subscribe(s, c, sender))
                self.chars[c_uuid] = (s_uuid, local)
                if ch.get("value"):
                    self.values[c_uuid] = bytes.fromhex(ch["value"])
            provider.add_advertisement_status_changed(lambda sender, args, s=s_uuid: self._on_adv_status(s, args))
            adv = GattServiceProviderAdvertisingParameters()
            adv.is_connectable = True
            adv.is_discoverable = bool(svc.get("advertise", True))
            if svc.get("serviceData"):
                adv.service_data = to_buffer(bytes.fromhex(svc["serviceData"]))
            provider.start_advertising_with_parameters(adv)
            self.providers.append((s_uuid, provider))
        log.info("profil « %s » publié (%d service(s))", profile.get("name", "?"), len(self.providers))

    def stop(self):
        for _, provider in self.providers:
            try:
                provider.stop_advertising()
            except Exception:  # noqa: BLE001
                pass
        self.providers.clear()
        self.chars.clear()

    # ------------------------------------------------------------ événements WinRT (thread système)
    def _on_adv_status(self, s_uuid, args):
        status = ADV_STATUS.get(int(args.status), str(args.status))
        self._post({"t": "ble.advertising", "service": s_uuid, "status": status, "error": int(args.error)})

    def _on_write(self, s_uuid, c_uuid, args):
        deferral = args.get_deferral()
        device = args.session.device_id.id if args.session else None
        asyncio.run_coroutine_threadsafe(self._handle_write(s_uuid, c_uuid, args, deferral, device), self.loop)

    def _on_read(self, s_uuid, c_uuid, args):
        deferral = args.get_deferral()
        device = args.session.device_id.id if args.session else None
        asyncio.run_coroutine_threadsafe(self._handle_read(s_uuid, c_uuid, args, deferral, device), self.loop)

    def _on_subscribe(self, s_uuid, c_uuid, sender):
        clients = [
            {"device": c.session.device_id.id if c.session else None, "maxNotification": c.max_notification_size}
            for c in sender.subscribed_clients
        ]
        self._post({"t": "ble.subscribe", "service": s_uuid, "char": c_uuid, "clients": clients})

    def _post(self, msg):
        self.loop.call_soon_threadsafe(lambda: asyncio.ensure_future(self.emit(msg)))

    # ------------------------------------------------------------ traitement
    async def _handle_write(self, s_uuid, c_uuid, args, deferral, device):
        try:
            req = await args.get_request_async()
            data = from_buffer(req.value)
            with_response = req.option == GattWriteOption.WRITE_WITH_RESPONSE
            if with_response:
                req.respond()
            await self.emit(
                {
                    "t": "ble.write",
                    "service": s_uuid,
                    "char": c_uuid,
                    "value": data.hex(),
                    "offset": req.offset,
                    "withResponse": with_response,
                    "device": device,
                    "at": time.time(),
                }
            )
        except Exception as e:  # noqa: BLE001
            log.exception("écriture %s : %s", c_uuid, e)
        finally:
            deferral.complete()

    async def _handle_read(self, s_uuid, c_uuid, args, deferral, device):
        try:
            req = await args.get_request_async()
            value = self.values.get(c_uuid, b"")
            if self.ask_core_on_read:
                self.read_seq += 1
                rid = self.read_seq
                fut = self.loop.create_future()
                self.pending_reads[rid] = fut
                await self.emit({"t": "ble.read", "id": rid, "service": s_uuid, "char": c_uuid, "offset": req.offset, "device": device})
                try:
                    value = bytes.fromhex(await asyncio.wait_for(fut, 0.8))
                except asyncio.TimeoutError:
                    log.warning("lecture %s : pas de réponse du serveur, valeur en cache", c_uuid)
                finally:
                    self.pending_reads.pop(rid, None)
            else:
                await self.emit({"t": "ble.read", "service": s_uuid, "char": c_uuid, "offset": req.offset, "device": device})
            req.respond_with_value(to_buffer(value[req.offset :]))
        except Exception as e:  # noqa: BLE001
            log.exception("lecture %s : %s", c_uuid, e)
        finally:
            deferral.complete()

    def read_reply(self, rid, value_hex):
        fut = self.pending_reads.get(rid)
        if fut and not fut.done():
            fut.set_result(value_hex)

    async def notify(self, c_uuid, data: bytes):
        entry = self.chars.get(str(uuid.UUID(c_uuid)))
        if not entry:
            log.warning("notification vers une caractéristique inconnue %s", c_uuid)
            return 0
        _, local = entry
        results = await local.notify_value_async(to_buffer(data))
        return len(results)

    def set_value(self, c_uuid, data: bytes):
        self.values[str(uuid.UUID(c_uuid))] = data


RADIO_STATES = {0: "inconnu", 1: "activé", 2: "désactivé", 3: "désactivé (matériel)"}


async def adapter_info():
    a = await BluetoothAdapter.get_default_async()
    if a is None:
        return {"present": False}
    radio = await a.get_radio_async()
    state = int(radio.state) if radio else 0
    return {
        "present": True,
        "address": f"{a.bluetooth_address:012X}",
        "peripheralRole": a.is_peripheral_role_supported,
        "extendedAdvertising": getattr(a, "is_extended_advertising_supported", None),
        "radio": RADIO_STATES.get(state, str(state)),
        "radioOn": state == 1,
    }


async def wait_radio_on(emit=None):
    """Attend que le Bluetooth soit activé dans Windows (sans jamais le modifier soi-même)."""
    warned = False
    while True:
        info = await adapter_info()
        if info.get("radioOn"):
            return info
        if not warned:
            if emit:
                await emit({"t": "ble.hello", "kind": "winrt", "adapter": info})
            else:
                log.warning("Bluetooth %s dans Windows : active-le (Paramètres > Bluetooth et appareils)", info.get("radio"))
            warned = True
        await asyncio.sleep(2)


# ---------------------------------------------------------------- modes d'exécution
async def run_with_core(url: str):
    import websockets

    loop = asyncio.get_running_loop()
    info = await adapter_info()
    if not info.get("present"):
        log.error("aucun adaptateur Bluetooth trouvé")
        return
    if not info.get("peripheralRole"):
        log.error("l'adaptateur Bluetooth ne gère pas le rôle périphérique : utiliser un ESP32")
    while True:
        try:
            async with websockets.connect(url, max_size=2**20) as ws:
                log.info("connecté au serveur ARKLIGHT (%s)", url)

                async def emit(msg):
                    await ws.send(json.dumps(msg))

                periph = Peripheral(loop, emit)
                info = await wait_radio_on(emit)
                await emit({"t": "ble.hello", "kind": "winrt", "adapter": info})
                try:
                    async for raw in ws:
                        msg = json.loads(raw)
                        t = msg.get("t")
                        try:
                            if t == "ble.profile":
                                await periph.apply(msg["profile"])
                                await emit({"t": "ble.published", "name": msg["profile"].get("name"), "services": len(periph.providers)})
                            elif t == "ble.notify":
                                n = await periph.notify(msg["char"], bytes.fromhex(msg["value"]))
                                if msg.get("ack"):
                                    await emit({"t": "ble.notified", "char": msg["char"], "clients": n})
                            elif t == "ble.setValue":
                                periph.set_value(msg["char"], bytes.fromhex(msg["value"]))
                            elif t == "ble.readReply":
                                periph.read_reply(msg["id"], msg["value"])
                            elif t == "ble.stop":
                                periph.stop()
                                await emit({"t": "ble.advertising", "status": "arrêté"})
                        except Exception as e:  # noqa: BLE001
                            log.exception("commande %s", t)
                            await emit({"t": "ble.error", "message": f"{t} : {e}"})
                finally:
                    periph.stop()
        except (OSError, Exception) as e:  # noqa: BLE001
            log.warning("serveur injoignable (%s), nouvel essai dans 2 s", e)
            await asyncio.sleep(2)


async def run_standalone():
    """Profil de test sans serveur : chaque écriture est renvoyée en notification (écho)."""
    loop = asyncio.get_running_loop()
    info = await wait_radio_on()
    log.info("adaptateur : %s", info)
    periph = None

    async def emit(msg):
        log.info("%s", msg)
        if msg.get("t") == "ble.write":
            data = bytes.fromhex(msg["value"])
            await periph.notify(TEST_PROFILE["services"][0]["characteristics"][1]["uuid"], b"ECHO:" + data)

    periph = Peripheral(loop, emit)
    await periph.apply(TEST_PROFILE)
    log.info("profil de test publié : cherche le service %s avec nRF Connect", TEST_PROFILE["services"][0]["uuid"])
    while True:
        await asyncio.sleep(3600)


def main():
    ap = argparse.ArgumentParser(description="Pont Bluetooth LE ARKLIGHT")
    ap.add_argument("--serveur", default="ws://127.0.0.1:8686/bridge")
    ap.add_argument("--autonome", action="store_true", help="profil de test sans serveur ARKLIGHT")
    ap.add_argument("-v", "--verbeux", action="store_true")
    args = ap.parse_args()
    logging.basicConfig(level=logging.DEBUG if args.verbeux else logging.INFO, format="[ble] %(message)s", stream=sys.stdout)
    try:
        asyncio.run(run_standalone() if args.autonome else run_with_core(args.serveur))
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
