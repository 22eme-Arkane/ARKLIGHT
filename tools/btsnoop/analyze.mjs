#!/usr/bin/env node
// Analyse d'une capture Bluetooth (btsnoop_hci.log) faite pendant une session AsteraApp <-> AsteraBox.
//
//   node tools/btsnoop/analyze.mjs <capture.log> [--filtre astera] [--appareil AA:BB:CC:DD:EE:FF] [--sortie dossier]
//
// Produit dans le dossier de sortie (par défaut à côté de la capture) :
//   - <nom>.rapport.md : appareils vus, table GATT, chronologie des échanges, rafales d'écritures
//   - <nom>.json       : les mêmes données, exploitables par l'émulateur
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { basename, dirname, join } from 'node:path';
import { analyze, bursts } from './btsnoop.mjs';
import { findBtsnoopLogs, isZip } from './zip.mjs';

const args = process.argv.slice(2);
const opt = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : null;
};
const filter = (opt('--filtre') || '').toLowerCase();
const onlyPeer = (opt('--appareil') || '').toUpperCase();
const outDirArg = opt('--sortie');
const file = args[0];
if (!file) {
  console.log('Usage : node tools/btsnoop/analyze.mjs <btsnoop_hci.log> [--filtre astera] [--appareil AA:BB:..] [--sortie dossier]');
  process.exit(1);
}

let raw = readFileSync(file);
if (isZip(raw)) {
  // Rapport de bug Android (adb bugreport) : on prend le journal btsnoop le plus gros.
  const logs = findBtsnoopLogs(raw);
  if (!logs.length) {
    console.error('Aucun journal btsnoop dans cette archive : le « journal HCI Bluetooth » était-il activé ?');
    process.exit(1);
  }
  console.log(`Archive Android : ${logs.map((l) => `${l.name} (${Math.round(l.data.length / 1024)} ko)`).join(', ')}`);
  raw = logs[0].data;
}
const result = analyze(raw);
const base = basename(file).replace(/\.[^.]+$/, '');
const outDir = outDirArg || dirname(file);
mkdirSync(outDir, { recursive: true });

const fmtTime = (ms) => {
  const d = new Date(ms);
  return `${d.toLocaleTimeString('fr-FR')}.${String(d.getMilliseconds()).padStart(3, '0')}`;
};

const interesting = (c) => {
  if (onlyPeer) return c.peer === onlyPeer;
  if (filter) return `${c.peerName || ''} ${c.peer || ''}`.toLowerCase().includes(filter) || c.gatt.length > 0;
  return true;
};

const conns = result.connections.filter(interesting);
const devices = result.devices
  .filter((d) => !filter || `${d.name || ''}`.toLowerCase().includes(filter) || conns.some((c) => c.peer === d.address))
  .sort((a, b) => b.count - a.count);

const L = [];
L.push(`# Analyse Bluetooth — ${basename(file)}`, '');
L.push(`${result.packetCount} paquets HCI · début ${fmtTime(result.start)}`, '');

L.push('## Appareils vus en publicité (advertising)', '');
if (!devices.length) L.push('_Aucun rapport de publicité dans la capture (le téléphone ne scannait pas pendant l’enregistrement)._', '');
for (const d of devices.slice(0, 40)) {
  L.push(`### ${d.name || '(sans nom)'} — \`${d.address}\``);
  L.push(`- vu ${d.count} fois, RSSI ${d.rssi} dBm`);
  if (d.services.length) L.push(`- services annoncés : ${d.services.map((s) => `\`${s}\``).join(', ')}`);
  for (const m of d.manufacturer) L.push(`- données fabricant : société \`${m.company}\` → \`${m.data}\``);
  if (d.adv) L.push(`- publicité brute : \`${d.adv.raw}\``);
  if (d.scanRsp) L.push(`- réponse au scan brute : \`${d.scanRsp.raw}\``);
  L.push('');
}

for (const c of conns) {
  L.push(`## Connexion ${c.peerName ? `« ${c.peerName} » ` : ''}\`${c.peer || '?'}\` (handle ${c.handle})`, '');
  L.push(`- de ${fmtTime(c.start)} à ${c.end ? fmtTime(c.end) : '(fin de capture)'} · MTU ${c.mtu} · appairage/chiffrement SMP : ${c.pairing ? 'oui' : 'non'}`, '');
  L.push('### Table GATT découverte', '');
  if (!c.gatt.length) L.push('_Pas de découverte de services dans cette capture (Android avait peut-être la table en cache : vider le cache Bluetooth ou réappairer)._', '');
  for (const s of c.gatt) {
    L.push(`- **Service \`${s.uuid}\`** ${s.name ? `(${s.name})` : ''} — handles ${s.start}–${s.end}`);
    for (const ch of s.characteristics) {
      L.push(`  - Caractéristique \`${ch.uuid}\` ${ch.name ? `(${ch.name})` : ''} — valeur @${ch.valueHandle} — [${ch.props.join(', ')}]`);
      for (const d of ch.descriptors) L.push(`    - descripteur \`${d.uuid}\` ${d.name ? `(${d.name})` : ''} @${d.handle}`);
    }
  }
  L.push('');

  const groups = bursts(c.ops);
  L.push(`### Rafales d’écritures du téléphone (${groups.length}) — une rafale ≈ une action dans l’appli`, '');
  groups.forEach((g, i) => {
    L.push(`**#${i + 1}** à ${fmtTime(g.start)} — ${g.writes.length} écriture(s)`);
    L.push('```');
    for (const w of g.writes.slice(0, 30)) L.push(`${w.op.padEnd(10)} @${w.handle} ${w.target?.uuid || ''}  ${w.value}`);
    if (g.writes.length > 30) L.push(`… ${g.writes.length - 30} de plus`);
    L.push('```');
  });
  L.push('');

  L.push('### Chronologie complète', '');
  L.push('| t (ms) | sens | opération | handle | cible | valeur (hex) | ASCII |');
  L.push('|---:|:-:|---|---:|---|---|---|');
  for (const o of c.ops) {
    if (['FIND_INFO_REQ', 'READ_BY_TYPE_REQ', 'READ_BY_GROUP_REQ'].includes(o.op)) continue;
    const dir = o.dir === 'out' ? 'tél → boîte' : 'boîte → tél';
    const target = o.target ? `${o.target.uuid}${o.target.name ? ` (${o.target.name})` : ''}` : '';
    const val = o.value ?? o.error ?? (o.mtu ? `MTU ${o.mtu}` : o.raw || '');
    L.push(`| ${o.rel} | ${dir} | ${o.op} | ${o.handle ?? ''} | ${target} | \`${val}\` | ${o.ascii ? `\`${o.ascii.replace(/\|/g, '¦')}\`` : ''} |`);
  }
  L.push('');
}

if (!conns.length) L.push('## Aucune connexion BLE trouvée', '', 'Vérifie que le journal HCI était activé avant de lancer l’AsteraApp, puis que Bluetooth a été coupé/rallumé.');

const mdPath = join(outDir, `${base}.rapport.md`);
const jsonPath = join(outDir, `${base}.json`);
writeFileSync(mdPath, L.join('\n'));
writeFileSync(jsonPath, JSON.stringify({ ...result, connections: conns, devices }, null, 2));
console.log(`Rapport : ${mdPath}`);
console.log(`Données : ${jsonPath}`);
console.log(`${devices.length} appareil(s) en publicité, ${conns.length} connexion(s) analysée(s).`);
for (const c of conns) console.log(`  - ${c.peerName || '?'} ${c.peer || ''} : ${c.gatt.length} service(s), ${c.ops.length} opération(s) ATT`);
