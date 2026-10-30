// Version Rust d'ARKLIGHT : données d'essai de parité du RDM (shared/rdm.js) et de son transport
// en Art-Net (server/dmx/artnet.js), calculées par la version JavaScript. Le répondeur Rust
// (rust/crates/arklight-net/src/rdm.rs) doit donner exactement les mêmes octets.
//   node tools/rust/export-rdm-data.mjs        (depuis la racine du projet)
// Écrit :
//   rust/crates/arklight-net/tests/data/rdm.json
//     fixtures   projecteurs de départ (ceux de demo-night, plus quelques cas particuliers)
//     tod        table des appareils (UID) de chaque univers
//     requests   suite de questions RDM (hexadécimal) → réponse exacte, et changements demandés
//     slots      canaux RDM (type, identifiant, texte) de chaque mode distinct de la bibliothèque
//     ascii      textes → texte ASCII des applis RDM ; deviceIds : identifiant → numéro d'appareil
//     artnet     paquets ArtTodData, ArtRdm, ArtPollReply (RDM annoncé)
//   rust/crates/arklight-net/src/rdm_ascii.rs
//     table de conversion en ASCII (décomposition Unicode, accents retirés) de la fonction ascii()
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { FIXTURE_TYPES } from '../../shared/fixtures/index.js';
import { CC, PID, RdmResponder, ascii, buildRdm, deviceIdOf, slotsOf, uidText } from '../../shared/rdm.js';
import { ArtNetInput } from '../../server/dmx/artnet.js';

const root = (p) => fileURLToPath(new URL(`../../${p}`, import.meta.url));
const OUT = root('rust/crates/arklight-net/tests/data/rdm.json');
const OUT_ASCII = root('rust/crates/arklight-net/src/rdm_ascii.rs');
const hex = (d) => (d ? Buffer.from(d).toString('hex') : null);

// ------------------------------------------------------------------ projecteurs
const demo = JSON.parse(readFileSync(root('rust/crates/arklight-core/tests/data/demo-night.json'), 'utf8'));
const fixtures = structuredClone(demo.fixtures);
const byName = (n) => fixtures.find((f) => f.name === n);
byName('Titan enseigne 2').settings.universe = 2;
const base = (id, type, name, settings) => ({ id, type, name, position: [0, 0, 0], rotation: [0, 0, 0], settings, state: {} });
fixtures.push(
  // nom accentué, trop long, avec un emoji : texte ASCII de 32 caractères
  base('fxlong', 'astera.titan', 'Projecteur « côté » jardin — très très long nom 💡', { universe: 2, dmxAddress: 200, dmxMode: '44' }),
  // sans mode : premier mode du modèle
  base('fxnomode', 'arri.skypanel-s60c', 'Sans mode', { universe: 2, dmxAddress: 100 }),
  // sans adresse
  base('fxnoaddr', 'aputure.ls-600d', 'Sans adresse', { universe: 2, dmxMode: 'm1' }),
  // modèle sans DMX : absent de la table des appareils
  base('fxart7', 'astera.art7', 'ART7', { universe: 1, dmxAddress: 300 }),
  // deux identifiants de même empreinte (FNV-1a) : le second prend le numéro suivant
  base('f55zx', 'astera.nyx', 'Collision A', { universe: 3, dmxAddress: 1, dmxMode: '41' }),
  base('fgpcd', 'astera.helios', 'Collision B', { universe: 3, dmxAddress: 400, dmxMode: '100' }),
  // univers sans nombre : ignoré
  base('fxnouniv', 'astera.nyx', 'Sans univers', { dmxAddress: 1, dmxMode: '1' }),
);
const initial = structuredClone(fixtures);

// ------------------------------------------------------------------ répondeur
let events = [];
const rdm = new RdmResponder({
  getFixtures: () => fixtures,
  onSet(f, changes, what) {
    // comme SceneStore.updateFixture : nom tronqué à 60 caractères, réglages fusionnés
    events.push({ type: 'set', fixture: f.id, what, changes: structuredClone(changes) });
    if (typeof changes.name === 'string') f.name = changes.name.slice(0, 60);
    if (changes.settings) Object.assign(f.settings, changes.settings);
  },
  onIdentify(f, on) {
    events.push({ type: 'identify', fixture: f.id, on });
  },
});

const CONTROLLER = Uint8Array.of(0x4c, 0x55, 0x00, 0x00, 0x00, 0x01);
const UNIVERSES = [1, 2, 3, 4];
const tod = Object.fromEntries(UNIVERSES.map((u) => [u, rdm.tod(u).map(hex)]));
const requests = [];
let tn = 0;
function ask(universe, msg) {
  events = [];
  const reply = rdm.handle(universe, msg);
  requests.push({ universe, request: hex(msg), reply: hex(reply), events });
}
const uidOf = (u) => Uint8Array.from(Buffer.from(u, 'hex'));
const q = (universe, dest, cc, pid, pd = [], opts = {}) => ask(universe, buildRdm({ dest, src: CONTROLLER, tn: tn++ & 0xff, cc, pid, pd: Uint8Array.from(pd), ...opts }));
const GETS = [PID.SUPPORTED_PARAMETERS, PID.DEVICE_INFO, PID.DEVICE_MODEL_DESCRIPTION, PID.MANUFACTURER_LABEL, PID.DEVICE_LABEL, PID.SOFTWARE_VERSION_LABEL, PID.DMX_PERSONALITY, PID.DMX_START_ADDRESS, PID.SLOT_INFO, PID.IDENTIFY_DEVICE];

for (const u of UNIVERSES) {
  for (const uid of tod[u].map(uidOf)) {
    for (const pid of GETS) q(u, uid, CC.GET, pid);
    const f = rdm.devices(u).find((d) => hex(d.uid) === hex(uid));
    const modes = f.type.dmx.modes.length;
    for (let n = 0; n <= Math.min(modes + 1, 255); n += n < 3 || n > modes - 2 ? 1 : 7) q(u, uid, CC.GET, PID.DMX_PERSONALITY_DESCRIPTION, [n]);
    const slots = slotsOf(f.type.dmx.modes.find((m) => m.id === f.f.settings.dmxMode) || f.type.dmx.modes[0]).length;
    for (let n = 0; n <= slots + 1; n += n < 4 || n > slots - 3 ? 1 : 5) q(u, uid, CC.GET, PID.SLOT_DESCRIPTION, [n >> 8, n & 0xff]);
    // erreurs de format, classes de commande refusées
    q(u, uid, CC.GET, PID.DMX_PERSONALITY_DESCRIPTION, []);
    q(u, uid, CC.GET, PID.SLOT_DESCRIPTION, [0]);
    q(u, uid, CC.SET, PID.DEVICE_INFO, []);
    q(u, uid, CC.SET, PID.SUPPORTED_PARAMETERS, []);
    q(u, uid, CC.SET, PID.DEVICE_MODEL_DESCRIPTION, []);
    q(u, uid, CC.SET, PID.MANUFACTURER_LABEL, []);
    q(u, uid, CC.SET, PID.SOFTWARE_VERSION_LABEL, []);
    q(u, uid, CC.SET, PID.DMX_PERSONALITY_DESCRIPTION, [1]);
    q(u, uid, CC.SET, PID.SLOT_INFO, []);
    q(u, uid, CC.SET, PID.SLOT_DESCRIPTION, [0, 0]);
    q(u, uid, CC.GET, 0x0400); // paramètre inconnu
    q(u, uid, 0x40, PID.DEVICE_INFO); // classe de commande inconnue
    q(u, uid, CC.DISCOVERY, 0x0001); // découverte : par la ToD d'Art-Net
    q(u, uid, CC.GET, PID.DEVICE_INFO, [], { subDevice: 1 });
    q(u, uid, CC.GET, PID.DEVICE_INFO, [], { subDevice: 0xffff });
  }
}

// changements demandés par une appli
const [a, b] = tod[1].map(uidOf);
q(1, a, CC.SET, PID.DMX_START_ADDRESS, [0, 101]);
q(1, a, CC.GET, PID.DMX_START_ADDRESS);
q(1, a, CC.GET, PID.DEVICE_INFO);
q(1, a, CC.SET, PID.DMX_START_ADDRESS, [0x02, 0x00]); // 512 : le mode ne tient plus
q(1, a, CC.SET, PID.DMX_START_ADDRESS, [0, 0]);
q(1, a, CC.SET, PID.DMX_START_ADDRESS, [0x02, 0x01]);
q(1, a, CC.SET, PID.DMX_START_ADDRESS, [1]);
q(1, a, CC.SET, PID.DMX_START_ADDRESS, [0, 1, 2]);
q(1, b, CC.SET, PID.DMX_PERSONALITY, [2]);
q(1, b, CC.GET, PID.DMX_PERSONALITY);
q(1, b, CC.GET, PID.DEVICE_INFO);
q(1, b, CC.GET, PID.SLOT_INFO);
q(1, b, CC.SET, PID.DMX_PERSONALITY, [0]);
q(1, b, CC.SET, PID.DMX_PERSONALITY, [200]);
q(1, b, CC.SET, PID.DMX_PERSONALITY, []);
q(1, b, CC.SET, PID.IDENTIFY_DEVICE, [1]);
q(1, b, CC.GET, PID.IDENTIFY_DEVICE);
q(1, b, CC.SET, PID.IDENTIFY_DEVICE, [2]);
q(1, b, CC.SET, PID.IDENTIFY_DEVICE, []);
q(1, b, CC.SET, PID.IDENTIFY_DEVICE, [0]);
q(1, b, CC.GET, PID.IDENTIFY_DEVICE);
// noms : zéros de fin, espaces, octets hors ASCII (é, espace insécable, U+0085), trop long
const label = (s) => [...Buffer.from(s, 'latin1')];
for (const pd of [label('Face jardin'), [...label('Contre  '), 0, 0], label('  '), [], label('Caf\xe9 \xa0'), label('\x85Nom\x85'), label('x'.repeat(32)), label('y'.repeat(33)), label('\tTab\n')]) {
  q(1, a, CC.SET, PID.DEVICE_LABEL, pd);
  q(1, a, CC.GET, PID.DEVICE_LABEL);
}
// diffusion à tous les appareils (FFFF:FFFFFFFF ou 7FF0:FFFFFFFF) : aucune réponse, chacun agit
const ALL = Uint8Array.of(0xff, 0xff, 0xff, 0xff, 0xff, 0xff);
const ALL_PROTO = Uint8Array.of(0x7f, 0xf0, 0xff, 0xff, 0xff, 0xff);
q(2, ALL, CC.SET, PID.IDENTIFY_DEVICE, [1]);
q(2, ALL_PROTO, CC.SET, PID.DMX_START_ADDRESS, [0, 7]);
q(2, ALL, CC.GET, PID.DEVICE_INFO);
q(2, Uint8Array.of(0x12, 0x34, 0xff, 0xff, 0xff, 0xff), CC.SET, PID.IDENTIFY_DEVICE, [1]); // autre fabricant
for (const uid of tod[2].map(uidOf)) q(2, uid, CC.GET, PID.IDENTIFY_DEVICE);
// appareil inconnu, mauvais univers
q(1, Uint8Array.of(0x7f, 0xf0, 0, 0, 0, 9), CC.GET, PID.DEVICE_INFO);
q(9, a, CC.GET, PID.DEVICE_INFO);
// messages abîmés : somme de contrôle, longueur, code de sous-départ
const good = buildRdm({ dest: a, src: CONTROLLER, tn: 1, cc: CC.GET, pid: PID.DEVICE_INFO });
const broken = (fn) => {
  const m = Uint8Array.from(good);
  fn(m);
  return m;
};
ask(1, broken((m) => (m[m.length - 1] ^= 0xff)));
ask(1, broken((m) => (m[m.length - 2] ^= 0x01)));
ask(1, broken((m) => (m[0] = 0x02)));
ask(1, broken((m) => (m[1] = 30)));
ask(1, good.slice(0, 24));
ask(1, Uint8Array.of(...good, 0xaa, 0xbb)); // octets en trop après le message : acceptés
ask(1, new Uint8Array(0));

// ------------------------------------------------------------------ canaux de chaque mode distinct
const slots = [];
const seen = new Set();
for (const t of FIXTURE_TYPES) {
  for (const m of t.dmx?.modes || []) {
    const s = slotsOf(m).map((x) => [x.type, x.label, x.text]);
    const key = JSON.stringify(s);
    if (seen.has(key)) continue;
    seen.add(key);
    slots.push({ type: t.id, mode: m.id, slots: s });
  }
}

// ------------------------------------------------------------------ textes ASCII, identifiants
const texts = [
  '',
  'ARRI',
  'Nova débouchage',
  'Baby 1K porte du café',
  'Ça ira, Œuvre, ÆSIR, straße, ﬁn',
  'Ångström Å K ≠ ≮ ≯',
  'tab\tnew\nline\x7f\x00',
  'émoji 💡🎬 fin',
  '한국어 조명',
  'ガギグ ぱ カ',
  'é (combinant é) ñ (ñ)',
  'Ελληνικά ΐ',
  'Русский Й ё',
  'x'.repeat(40),
  'é'.repeat(40),
  '💡'.repeat(20),
  'Ǆ ǅ ǆ ŉ ẞ ỳ Ỹ ḁ',
  'ά ϓ ῭ ` ΅',
];
const asciiCases = [];
for (const s of texts) for (const max of [32, 12, 100]) asciiCases.push([s, max, ascii(s, max)]);
const ids = ['fmvqe4s4dkt0c', 'f55zx', 'fgpcd', '', 'é', '💡', 'a'.repeat(100)];
const deviceIds = ids.map((id) => [id, deviceIdOf(id), uidText(0x7ff0, deviceIdOf(id))]);

// ------------------------------------------------------------------ Art-Net
const node = new ArtNetInput({ port: 0 });
node.rdm = { tod: () => [], handle: () => null };
const todUids = rdm.tod(1);
const many = Array.from({ length: 450 }, (_, i) => Uint8Array.of(0x7f, 0xf0, 0, 0, i >> 8, i & 0xff));
const todPackets = (pa, uids) => {
  const blocks = Math.max(1, Math.ceil(uids.length / 200));
  return Array.from({ length: blocks }, (_, i) => hex(node._buildTodData(pa, uids.slice(i * 200, (i + 1) * 200), uids.length, i)));
};
const artnet = {
  todData: [
    { portAddress: 0, uids: todUids.map(hex), packets: todPackets(0, todUids) },
    { portAddress: 0x0123, uids: [], packets: todPackets(0x0123, []) },
    { portAddress: 0x7fff, uids: many.map(hex), packets: todPackets(0x7fff, many) },
  ],
  rdm: [
    { portAddress: 0, data: hex(good), packet: hex(node._buildRdm(0, good)) },
    { portAddress: 0x2345, data: hex(good), packet: hex(node._buildRdm(0x2345, good)) },
  ],
  pollReply: {
    ip: '10.0.0.5',
    mac: 'aa:bb:cc:dd:ee:ff',
    ports: [0, 1, 2, 3],
    packet: hex(node._buildPollReply({ address: '10.0.0.5', mac: 'aa:bb:cc:dd:ee:ff' }, [0, 1, 2, 3], 1)),
  },
};

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(OUT, JSON.stringify({ controller: hex(CONTROLLER), fixtures: initial, tod, requests, slots, ascii: asciiCases, deviceIds, artnet }));
console.log(`${OUT} : ${requests.length} questions, ${slots.length} modes distincts, ${asciiCases.length} textes`);

// ------------------------------------------------------------------ table ASCII (rdm_ascii.rs)
// Ce que ascii() fait de chaque caractère non ASCII, quand ce n'est pas « ? » (« ?? » au-delà du
// plan multilingue de base : JavaScript compte en unités UTF-16). Les syllabes coréennes (U+AC00 à
// U+D7A3) sont calculées (« ?? » ou « ??? » selon leur décomposition) : hors table.
const ranges = [];
for (let cp = 0x80; cp <= 0x10ffff; cp++) {
  if ((cp >= 0xd800 && cp <= 0xdfff) || (cp >= 0xac00 && cp <= 0xd7a3)) continue;
  const r = ascii(String.fromCodePoint(cp), 1000);
  if (r === (cp > 0xffff ? '??' : '?')) continue;
  const last = ranges[ranges.length - 1];
  if (last && last[1] === cp - 1 && last[2] === r) last[1] = cp;
  else ranges.push([cp, cp, r]);
}
const x = (n) => `0x${n.toString(16).padStart(4, '0')}`;
const lines = [];
for (let i = 0; i < ranges.length; i += 6) lines.push(`    ${ranges.slice(i, i + 6).map(([s, e, r]) => `(${x(s)}, ${x(e)}, ${JSON.stringify(r)})`).join(', ')},`);
writeFileSync(
  OUT_ASCII,
  `//! Table générée par \`node tools/rust/export-rdm-data.mjs\` (ne pas modifier à la main) : ce que la
//! fonction \`ascii\` de \`shared/rdm.js\` (décomposition NFD, accents retirés, « ? » pour le reste) fait
//! des caractères non ASCII dont le résultat n'est pas « ? » (« ?? » au-delà de U+FFFF).
//! Plages (premier, dernier, texte), triées.

pub(crate) const RANGES: &[(u32, u32, &str)] = &[
${lines.join('\n')}
];
`,
);
console.log(`${OUT_ASCII} : ${ranges.length} plages`);
