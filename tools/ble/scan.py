#!/usr/bin/env python
"""
Scanner Bluetooth LE d'ARKLIGHT — à lancer sur le PC, à côté d'une vraie AsteraBox.

    .venv\\Scripts\\python.exe tools\\ble\\scan.py                       # liste les appareils BLE proches
    .venv\\Scripts\\python.exe tools\\ble\\scan.py --nom astera          # filtre sur le nom
    .venv\\Scripts\\python.exe tools\\ble\\scan.py --connecter AA:BB:CC:DD:EE:FF
        -> se connecte, relève toute la table GATT (services, caractéristiques, descripteurs,
           valeurs lisibles) et l'enregistre dans captures\\ (JSON + profil pour l'émulateur).

Important : l'AsteraApp doit être fermée pendant --connecter (la boîte n'accepte en général
qu'une connexion à la fois).
"""
import argparse
import asyncio
import json
import sys
import time
from pathlib import Path

from bleak import BleakClient, BleakScanner
from bleak.exc import BleakError

ROOT = Path(__file__).resolve().parents[2]
CAPTURES = ROOT / "captures"

PROP_MAP = {
    "read": "read",
    "write": "write",
    "write-without-response": "writeNoResp",
    "notify": "notify",
    "indicate": "indicate",
}


def printable(data: bytes) -> str:
    return "".join(chr(b) if 32 <= b < 127 else "." for b in data)


async def scan(seconds: float, name_filter: str):
    print(f"Scan Bluetooth LE pendant {seconds:.0f} s…")
    found = await BleakScanner.discover(timeout=seconds, return_adv=True)
    rows = []
    for address, (device, adv) in found.items():
        name = adv.local_name or device.name or ""
        if name_filter and name_filter.lower() not in name.lower():
            continue
        rows.append((adv.rssi, address, name, adv))
    rows.sort(key=lambda r: r[0], reverse=True)
    if not rows:
        print("Aucun appareil trouvé." + (" (filtre trop strict ?)" if name_filter else ""))
    for rssi, address, name, adv in rows:
        print(f"\n{name or '(sans nom)'}  {address}  RSSI {rssi} dBm")
        for uuid in adv.service_uuids:
            print(f"   service annoncé : {uuid}")
        for company, data in adv.manufacturer_data.items():
            print(f"   données fabricant 0x{company:04x} : {data.hex(' ')}  « {printable(data)} »")
        for uuid, data in adv.service_data.items():
            print(f"   données de service {uuid} : {data.hex(' ')}")
        if adv.tx_power is not None:
            print(f"   puissance d'émission : {adv.tx_power} dBm")
    return rows


async def dump(address: str, seconds: float):
    CAPTURES.mkdir(exist_ok=True)
    print(f"Recherche de {address}…")
    device = await BleakScanner.find_device_by_address(address, timeout=seconds)
    if device is None:
        print("Appareil introuvable : est-il allumé, à portée, et l'AsteraApp est-elle bien fermée ?")
        return
    adv_info = {}
    found = await BleakScanner.discover(timeout=4, return_adv=True)
    if address.upper() in {a.upper() for a in found}:
        for a, (d, adv) in found.items():
            if a.upper() == address.upper():
                adv_info = {
                    "localName": adv.local_name,
                    "serviceUuids": adv.service_uuids,
                    "manufacturerData": {f"0x{k:04x}": v.hex() for k, v in adv.manufacturer_data.items()},
                    "serviceData": {k: v.hex() for k, v in adv.service_data.items()},
                    "txPower": adv.tx_power,
                    "rssi": adv.rssi,
                }
    print(f"Connexion à {device.name or address}…")
    result = {"address": address, "name": device.name, "advertising": adv_info, "capturedAt": time.strftime("%Y-%m-%d %H:%M:%S"), "services": []}
    async with BleakClient(device, timeout=20) as client:
        print(f"Connecté. MTU : {client.mtu_size}")
        for service in client.services:
            s = {"uuid": service.uuid, "handle": service.handle, "description": service.description, "characteristics": []}
            print(f"\nService {service.uuid}  ({service.description})")
            for ch in service.characteristics:
                c = {"uuid": ch.uuid, "handle": ch.handle, "properties": ch.properties, "description": ch.description, "descriptors": []}
                line = f"  Caractéristique {ch.uuid}  [{', '.join(ch.properties)}]"
                if "read" in ch.properties:
                    try:
                        value = bytes(await client.read_gatt_char(ch))
                        c["value"] = value.hex()
                        line += f"\n      valeur : {value.hex(' ')}  « {printable(value)} »"
                    except Exception as e:  # noqa: BLE001
                        c["readError"] = str(e)
                        line += f"\n      lecture impossible : {e}"
                print(line)
                for d in ch.descriptors:
                    entry = {"uuid": d.uuid, "handle": d.handle, "description": d.description}
                    try:
                        dv = bytes(await client.read_gatt_descriptor(d.handle))
                        entry["value"] = dv.hex()
                    except Exception:  # noqa: BLE001
                        pass
                    c["descriptors"].append(entry)
                    print(f"      descripteur {d.uuid} ({d.description}) {entry.get('value', '')}")
                s["characteristics"].append(c)
            result["services"].append(s)

    stamp = time.strftime("%Y%m%d-%H%M%S")
    safe = (device.name or "appareil").replace(" ", "_")
    out = CAPTURES / f"gatt-{safe}-{stamp}.json"
    out.write_text(json.dumps(result, indent=2, ensure_ascii=False), encoding="utf-8")

    # Profil directement utilisable par l'émulateur (services hors GAP/GATT standard)
    profile = {"name": device.name or "Appareil", "services": []}
    for s in result["services"]:
        if s["uuid"].startswith(("00001800", "00001801")):
            continue
        profile["services"].append(
            {
                "uuid": s["uuid"],
                "advertise": s["uuid"] in (adv_info.get("serviceUuids") or []),
                "characteristics": [
                    {"uuid": c["uuid"], "props": [PROP_MAP[p] for p in c["properties"] if p in PROP_MAP], **({"value": c["value"]} if c.get("value") else {})}
                    for c in s["characteristics"]
                ],
            }
        )
    pout = CAPTURES / f"profil-{safe}-{stamp}.json"
    pout.write_text(json.dumps(profile, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"\nTable GATT enregistrée : {out}\nProfil pour l'émulateur : {pout}")


def main():
    ap = argparse.ArgumentParser(description="Scanner Bluetooth LE ARKLIGHT")
    ap.add_argument("--nom", default="", help="filtre sur le nom (ex. astera)")
    ap.add_argument("--duree", type=float, default=8.0, help="durée du scan en secondes")
    ap.add_argument("--connecter", metavar="ADRESSE", help="relever la table GATT de cet appareil")
    args = ap.parse_args()
    try:
        if args.connecter:
            asyncio.run(dump(args.connecter, args.duree))
        else:
            asyncio.run(scan(args.duree, args.nom))
    except KeyboardInterrupt:
        pass
    except (OSError, BleakError) as e:
        print(f"Erreur Bluetooth : {e}\nLe Bluetooth de Windows est-il activé (Paramètres > Bluetooth et appareils) ?")
        sys.exit(1)


if __name__ == "__main__":
    main()
