// Version Rust d'ARKLIGHT : données d'essai de parité du MIDI, calculées par la version JavaScript
// (shared/midi.js, shared/midi-presets.js, web/js/midi.js et la logique d'apprentissage de
// web/js/ui/midi-panel.js). Le contrôleur Rust (rust/crates/arklight-midi) doit donner les mêmes
// résultats sur les mêmes entrées.
//   node tools/rust/export-midi-data.mjs        (depuis la racine du projet)
// Écrit rust/crates/arklight-midi/tests/data/midi.json :
//   kinds      fonctions commandables (id, rubrique, libellé, mode) et rubriques
//   parse      messages MIDI → message décodé, clé, description ; noms des notes
//   scale      échelles des faders (Kelvin, teinte, couleur, exposition, ambiance)
//   presets    préréglages : libellé, appareils reconnus, correspondances selon le premier canal
//   clean      correspondances lues (stockage, fichier) → correspondances gardées
//   route      aiguillage d'une suite de messages (état des touches gardé entre deux messages)
//   scenario   contrôleur complet (web/js/midi.js) sur le projet demo-night : préréglages,
//              correspondances, apprentissage, assistants ; appels faits à l'application et état
//   retarget   fonction proposée quand on change la sorte de fonction d'une correspondance
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { TARGET_GROUPS, TARGET_KINDS, cleanMappings, describeSource, isContinuous, noteName, parseMidi, routeMidi, scaleValue, sourceKey } from '../../shared/midi.js';
import { MIDI_PRESETS, presetLabel, presetMappings } from '../../shared/midi-presets.js';
import { Store } from '../../web/js/store.js';

const root = (p) => fileURLToPath(new URL(`../../${p}`, import.meta.url));
const OUT = root('rust/crates/arklight-midi/tests/data/midi.json');

// ------------------------------------------------------------------ fonctions, messages, échelles
const kinds = { kinds: TARGET_KINDS.map((k) => [k.id, k.group, k.label, k.mode]), groups: TARGET_GROUPS };

const messages = [
  [0xb0, 7, 127], [0xb3, 1, 0], [0xbf, 127, 64], [0xb0, 5, 63], [0x91, 60, 100], [0x91, 60, 0], [0x81, 60, 64], [0x9f, 0, 1],
  [0xc0, 4], [0xcf, 127, 9], [0xe0, 0x7f, 0x7f], [0xe0, 0, 0x40], [0xe5, 0x01, 0x00], [0xe0], [0xf8], [0xd0, 40], [0xa0, 60, 3],
  [], [0x7f, 1, 2], [0x90], [0xb0, 200, 255], [0x90, 61], [0xf0, 1, 2, 0xf7],
];
const parse = messages.map((data) => {
  const msg = parseMidi(data);
  return { data, msg, key: msg ? sourceKey(msg) : null, text: msg ? describeSource(msg) : null };
});
const notes = Array.from({ length: 128 }, (_, n) => noteName(n));

const scaleKinds = ['selCct', 'selTint', 'selHue', 'exposure', 'ambient', 'selSat', 'fader'];
const values = [...Array.from({ length: 128 }, (_, i) => i / 127), 0.5, 10 / 13, 0.123456789, 1 / 3];
const scale = scaleKinds.map((kind) => ({ kind, values: values.map((v) => [v, scaleValue(kind, v)]) }));

// ------------------------------------------------------------------ préréglages
const devices = ['DMX Operator 384 MIDI', 'ADJ', 'USB MIDI Interface', 'dmx384', 'DMX 384', 'Operator', 'Akai APC mini', 'adjust', ''];
const presets = MIDI_PRESETS.map((p) => ({
  id: p.id,
  label: presetLabel(p),
  summary: p.summary,
  source: p.source,
  channel: p.options.channel,
  matches: devices.map((d) => [d, !!p.match && p.match.test(d)]),
  mappings: [null, 3, 16, 0].map((channel) => ({ channel, mappings: presetMappings(p, channel === null ? {} : { channel }) })),
}));

// ------------------------------------------------------------------ correspondances lues
const cleanInputs = [
  [{ id: 'a', source: { type: 'cc', channel: 1, number: 7 }, target: { kind: 'fader', index: 2 }, label: 'Fader deux très long' }],
  [null, 5, 'x', [], { target: { kind: 'inconnu' } }, { target: 'fader' }, { source: { type: 'cc' } }],
  [{ id: 'x', source: { type: 'cc', channel: 40, number: 300 }, target: { kind: 'fader', index: 2 } }],
  [{ id: 'y', source: { type: 'note', channel: '3', number: '64.9' }, target: { kind: 'group', name: 'Face', extra: [1, 2] } }],
  [{ id: 'z', source: { type: 'pitch', channel: true, number: null }, target: { kind: 'command', id: 'console.full' }, label: 5 }],
  [{ id: 'w', source: { type: 'sysex', channel: 1, number: 1 }, target: { kind: 'haze' } }],
  [{ id: 'v', source: { type: 'program', channel: -4, number: 1e12 }, target: { kind: 'planIndex', index: 3 }, label: 'émoji 💡💡💡💡' }],
  [{ id: 'u', source: { type: 'cc', channel: '0x0a', number: [12] }, target: { kind: 'selCct' }, preset: true }],
  'pas une liste',
  { mappings: [] },
];
// identifiant absent : un identifiant neuf (« m » + date), noté null
const clean = cleanInputs.map((input) => {
  const known = new Set((Array.isArray(input) ? input : []).map((x) => x?.id).filter((x) => typeof x === 'string'));
  return { input, output: cleanMappings(input).map((m) => ({ ...m, id: known.has(m.id) ? m.id : null })) };
});

// ------------------------------------------------------------------ aiguillage
const routeMaps = [
  ...cleanMappings([
    { id: 'a', source: { type: 'cc', channel: 1, number: 1 }, target: { kind: 'fader', index: 1 } },
    { id: 'b', source: { type: 'cc', channel: 1, number: 20 }, target: { kind: 'command', id: 'console.full' } },
    { id: 'c', source: { type: 'note', channel: 1, number: 36 }, target: { kind: 'group', name: 'Face' } },
    { id: 'd', source: { type: 'note', channel: 1, number: 37 }, target: { kind: 'groupLevel', name: 'Contres' } },
    { id: 'f', source: { type: 'note', channel: 1, number: 40 }, target: { kind: 'flash' } },
    { id: 'l', source: { type: 'note', channel: 1, number: 80 }, target: { kind: 'fixtureLatch', index: 1 } },
    { id: 'k', source: { type: 'cc', channel: 1, number: 90 }, target: { kind: 'blackout' } },
    { id: 'p', source: { type: 'program', channel: 1, number: 2 }, target: { kind: 'groupLatch', index: 2 } },
    { id: 'q', source: { type: 'program', channel: 1, number: 2 }, target: { kind: 'planIndex', index: 1 } },
    { id: 'r', source: { type: 'pitch', channel: 2, number: 99 }, target: { kind: 'selHue' } },
    { id: 's', source: { type: 'cc', channel: 1, number: 41 }, target: { kind: 'flash' } },
    { id: 't', source: null, target: { kind: 'flash' } },
  ]),
  { id: 'n', source: { type: 'note', channel: 1, number: 50 }, target: { kind: 'nouveau' } },
];
const routeStream = [
  [0xb0, 1, 64], [0xb0, 20, 127], [0xb0, 20, 127], [0xb0, 20, 0], [0xb0, 20, 127], [0x90, 36, 100], [0x80, 36, 0], [0x90, 37, 100], [0x90, 37, 0],
  [0xb1, 1, 64], [0x90, 40, 120], [0x80, 40, 0], [0x80, 40, 0], [0x90, 40, 1], [0x90, 40, 2], [0x90, 40, 0],
  [0x90, 80, 127], [0x80, 80, 0], [0x90, 80, 127], [0x90, 80, 127], [0x90, 80, 127], [0x80, 80, 0], [0x80, 80, 0],
  [0xb0, 90, 127], [0xb0, 90, 127], [0xb0, 90, 0], [0xb0, 90, 64], [0xb0, 90, 63],
  [0xc0, 2], [0xc0, 2], [0xc0, 3], [0xe1, 0x10, 0x20], [0xe1, 0, 0], [0xb0, 41, 127], [0xb0, 41, 30], [0x90, 50, 100], [0xf8],
];
const held = new Map();
const route = { mappings: routeMaps, steps: routeStream.map((data) => ({ data, routes: (parseMidi(data) ? routeMidi(routeMaps, parseMidi(data), held) : []).map((r) => ({ id: r.mapping.id, value: r.value ?? null, press: !!r.press, release: !!r.release, latch: r.latch ?? null })) })) };

// ------------------------------------------------------------------ contrôleur complet (web/js/midi.js)
const storage = new Map();
globalThis.localStorage = { getItem: (k) => (storage.has(k) ? storage.get(k) : null), setItem: (k, v) => storage.set(k, String(v)), removeItem: (k) => storage.delete(k) };
let frames = [];
globalThis.requestAnimationFrame = (fn) => frames.push(fn);
let access = null;
Object.defineProperty(globalThis, 'navigator', { value: { requestMIDIAccess: async () => access }, configurable: true, writable: true });
const src = readFileSync(root('web/js/midi.js'), 'utf8').replace(/from '\/shared\//g, `from '${pathToFileURL(root('shared')).href}/`);
const { createMidi } = await import(`data:text/javascript,${encodeURIComponent(src)}`);

const demo = JSON.parse(readFileSync(root('rust/crates/arklight-core/tests/data/demo-night.json'), 'utf8'));
const scene = structuredClone({ fixtures: demo.fixtures, groups: demo.groups, objects: [], environment: demo.environment });
scene.groups.push({ id: 'gvide', name: 'Vide', ids: [] }, { id: 'gfantome', name: 'Fantôme', ids: ['fmvqe4s4f3flw', 'disparu', 'fmvqe4s4dkt0c'] });
const plans = { current: { seq: 's1', plan: 'p1' }, sequences: [{ id: 's1', name: 'Séquence 1', plans: [{ id: 'p1' }, { id: 'p2' }] }, { id: 's2', plans: [{ id: 'p3' }] }] };
const store = new Store();
store.setScene(scene);
store.setPlans(plans);
const fixtureOrder = () => [...store.scene.fixtures].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0)).map((f) => f.id);

let log = [];
let inFlush = false;
const ids = (fs) => fs.map((f) => f.id);
const firstInScene = (list) => store.scene.fixtures.find((f) => list.includes(f.id))?.id ?? null;
const midi = createMidi({
  store,
  control: {
    flash: (fs, on) => log.push(['flash', ids(fs), on]),
    setIntensity: (fs, v) => log.push(['setIntensity', ids(fs), v]),
    setColor: (fs, c) => log.push(['setColor', ids(fs), c]),
  },
  consoleApi: {
    setFader: (i, v) => log.push(['setFader', i, v]),
    setMidiFaders: (marks, page) => log.push([inFlush ? 'deferred' : 'setMidiFaders', marks, page]),
    faderCount: () => 48,
    nextFamily: () => log.push(['nextFamily']),
  },
  // comme web/js/actions.js (selectGroup)
  actions: {
    selectGroup(gid) {
      const g = store.group(gid);
      if (!g) return;
      const list = g.ids.filter((id) => store.fixtures.has(id));
      store.setSelection(list, firstInScene(list));
    },
  },
  run: (id) => log.push(['run', id]),
  setEnv: (changes) => log.push(['setEnv', changes]),
  fixtureOrder,
  openPlan: (id) => log.push(['openPlan', id]),
  setBlackout: (on) => log.push(['blackout', on]),
});
store.addEventListener('selection', () => log.push(['selection', [...store.selection], store.selectedId]));

// ---- copie de la logique d'apprentissage de web/js/ui/midi-panel.js (sans l'interface)
const MAX_FADERS = 32;
const MAX_KEYS = 12;
const isPress = (msg) => (msg.type === 'note' && msg.on) || msg.type === 'program' || (msg.type === 'cc' && msg.on);
let learning = null;
let wizard = null;
function stopLearning() {
  learning = null;
  wizard = null;
  midi.learn(null);
}
function defaultTarget(source) {
  const used = midi.state.mappings;
  if (isContinuous(source)) {
    const next = Math.max(0, ...used.filter((m) => m.target.kind === 'fader').map((m) => m.target.index || 0)) + 1;
    return next <= MAX_FADERS ? { kind: 'fader', index: next } : { kind: 'selectionLevel' };
  }
  const groups = store.scene?.groups || [];
  const free = groups.find((g) => !used.some((m) => m.target.kind === 'group' && m.target.name === g.name));
  return free ? { kind: 'group', name: free.name } : { kind: 'command', id: 'console.full' };
}
function startLearn(id) {
  learning = id;
  midi.learn((source) => {
    if (id === 'new') midi.add({ source, target: defaultTarget(source) });
    else midi.update(id, { source });
    stopLearning();
  });
}
function assign(source, target) {
  const key = sourceKey(source);
  const same = (m) => m.target.kind === target.kind && (target.kind === 'fader' ? m.target.index === target.index : m.target.name === target.name);
  for (const m of midi.state.mappings.filter((x) => (x.source && sourceKey(x.source) === key) || same(x))) midi.remove(m.id);
  midi.add({ source, target });
}
function toggleWizard(kind) {
  if (learning === 'wizard' && wizard?.kind === kind) return stopLearning();
  const items =
    kind === 'faders'
      ? Array.from({ length: 8 }, (_, i) => i + 1)
      : kind === 'groups'
        ? (store.scene?.groups || []).map((g) => g.name)
        : (fixtureOrder?.() || []).map((id) => store.fixtures.get(id)?.name).filter(Boolean).slice(0, MAX_KEYS);
  if (!items.length) return;
  learning = 'wizard';
  wizard = { kind, items, next: 0, keys: new Set() };
  const step = () => {
    midi.learn(
      (source) => {
        wizard.keys.add(sourceKey(source));
        const item = wizard.items[wizard.next];
        assign(source, kind === 'faders' ? { kind: 'fader', index: item } : { kind: kind === 'groups' ? 'group' : 'fixture', name: item });
        wizard.next++;
        if (wizard.next >= wizard.items.length) stopLearning();
        else step();
      },
      (msg) => !wizard.keys.has(sourceKey(msg)) && (kind === 'faders' ? isContinuous(msg) : isPress(msg)),
    );
  };
  step();
}
const tt = (x, p) => x.replace(/\{(\w+)\}/g, (_, k) => p?.[k] ?? '');
function learningStatus() {
  const item = wizard?.items[wizard.next];
  return learning === 'new'
    ? 'Bouge un fader ou appuie sur une touche du contrôleur…'
    : learning === 'wizard' && wizard.kind === 'faders'
      ? tt('Bouge le fader du contrôleur qui doit commander le fader {0} de la console…', { 0: item })
      : learning === 'wizard' && wizard.kind === 'groups'
        ? tt('Appuie sur la touche du contrôleur pour le groupe « {0} » ({1} sur {2})…', { 0: item, 1: wizard.next + 1, 2: wizard.items.length })
        : learning === 'wizard'
          ? tt('Appuie sur la touche du contrôleur pour « {0} » ({1} sur {2})…', { 0: item, 1: wizard.next + 1, 2: wizard.items.length })
          : '';
}
// choix d'une autre sorte de fonction (kindSelect)
function targetForKind(kind, m) {
  const groups = store.scene?.groups || [];
  const fixtureName = store.selected()?.name || store.scene?.fixtures[0]?.name || '';
  return kind === 'fader'
    ? { kind, index: m.target.index || 1 }
    : kind === 'group' || kind === 'groupLevel'
      ? { kind, name: m.target.name && groups.some((g) => g.name === m.target.name) ? m.target.name : groups[0]?.name || '' }
      : kind === 'fixture' || kind === 'fixtureLevel'
        ? { kind, name: fixtureName }
        : kind === 'command'
          ? { kind, id: 'console.full' }
          : ['fixtureIndex', 'groupIndex', 'planIndex', 'fixtureLatch', 'groupLatch'].includes(kind)
            ? { kind, index: 1 }
            : { kind };
}

// ---- étapes
const bare = (m) => ({ source: m.source, target: m.target, ...(m.label ? { label: m.label } : {}) });
const presetId = (id) => (id?.startsWith('user:') ? `user#${midi.state.userPresets.findIndex((p) => p.id === id)}` : id);
// listes de correspondances : notées seulement quand elles changent (null sinon)
const previousLists = {};
const changed = (key, value) => {
  const text = JSON.stringify(value);
  if (previousLists[key] === text) return null;
  previousLists[key] = text;
  return value;
};
function snapshot() {
  const s = midi.state;
  const idx = midi.faderIndexes();
  return {
    page: s.page,
    mappings: changed('mappings', s.mappings.map(bare)),
    preset: s.preset ? { id: presetId(s.preset.id), channel: s.preset.channel } : null,
    userPresets: changed('userPresets', s.userPresets.map((p) => ({ name: p.name, mappings: p.mappings.map(bare) }))),
    layer: midi.layer().length,
    all: changed('all', midi.all().map(bare)),
    suggest: s.suggest ?? null,
    faderIndexes: idx,
    last: s.last ? `${describeSource(s.last.msg)} = ${Math.round(s.last.msg.value * 127)}${s.last.device ? ` — ${s.last.device}` : ''}` : '—',
    learning: learningStatus(),
    learningOn: midi.learning,
    pageInfo: idx.length && idx.length <= MAX_FADERS && !midi.activePreset() ? tt('Faders du contrôleur → faders {0} à {1} de la console (page {2})', { 0: idx[0], 1: idx[idx.length - 1], 2: s.page + 1 }) : '',
    selection: [...store.selection],
    selectedId: store.selectedId,
  };
}
const steps = [];
async function step(op, args, fn) {
  // préréglages personnels : identifiant tiré de l'heure (ms) ; deux dans la même milliseconde auraient le même
  if (op === 'saveUserPreset' || op === 'importPreset') await new Promise((r) => setTimeout(r, 5));
  log = [];
  let result = null;
  try {
    result = (await fn()) ?? null;
  } catch (e) {
    result = { error: e.message };
  }
  inFlush = true;
  const fs = frames;
  frames = [];
  for (const f of fs) f();
  inFlush = false;
  const deferred = log.filter((e) => e[0] === 'deferred');
  const out = log.filter((e) => e[0] !== 'deferred');
  if (deferred.length) out.push(['setMidiFaders', ...deferred[deferred.length - 1].slice(1)]);
  steps.push({ op, args, result, log: out, state: snapshot() });
}
const recv = (data, device = 'DMX Operator 384') => step('receive', { data, device }, () => midi.receive(Uint8Array.from(data), device));
const setInputs = (list) =>
  step('inputs', { list }, async () => {
    const inputs = new Map(list.map((d, i) => [`in${i}`, { id: `in${i}`, name: d.name, manufacturer: d.manufacturer, state: 'connected' }]));
    if (!access) {
      access = { inputs };
      await midi.start();
    } else {
      access.inputs = inputs;
      access.onstatechange();
    }
  });
const own = (i) => midi.state.mappings[i].id;
const add = (source, target, label) => step('add', { source, target, label }, () => void midi.add({ source, target, ...(label ? { label } : {}) }));

await step('init', {}, () => {});
await setInputs([{ name: 'DMX Operator 384 MIDI', manufacturer: '' }]);
await setInputs([{ name: 'USB Keyboard', manufacturer: 'Generic' }]);
await setInputs([{ name: 'USB MIDI', manufacturer: 'ADJ' }, { name: 'Autre', manufacturer: '' }]);
await step('setPreset', { id: 'adj-dmx-operator-384', channel: null }, () => midi.setPreset('adj-dmx-operator-384'));
await setInputs([{ name: 'DMX Operator 384 MIDI', manufacturer: '' }]);
// préréglage DMX Operator 384 : faders par banque, Speed / Fade, touches à bascule, scènes
for (const d of [[0xb0, 0, 64], [0xb0, 15, 127], [0xb0, 16, 1], [0xb0, 125, 100], [0xb0, 126, 90], [0xb0, 127, 30], [0xb1, 32, 100], [0xb1, 33, 0]]) await recv(d);
for (const d of [[0x90, 80, 127], [0x90, 81, 127], [0x80, 80, 0], [0x90, 92, 127], [0x90, 0, 127], [0x90, 3, 127], [0x90, 4, 127], [0x90, 0, 127], [0x90, 81, 127], [0x80, 81, 0]]) await recv(d);
for (const d of [[0xb0, 126, 127], [0xb0, 127, 127], [0x90, 48, 100], [0x80, 48, 0], [0x90, 50, 100], [0x90, 51, 100], [0x90, 79, 100], [0xb0, 0, 0]]) await recv(d);
for (const d of [[0x80, 92, 0], [0x80, 4, 64], [0x90, 4, 127]]) await recv(d);
await step('setPreset', { id: 'adj-dmx-operator-384', channel: 3 }, () => midi.setPreset('adj-dmx-operator-384', { channel: 3 }));
await recv([0xb2, 0, 64]);
await recv([0xb0, 0, 64]);
await recv([0xb3, 33, 127]);
await step('setPreset', { id: 'adj-dmx-operator-384-old', channel: null }, () => midi.setPreset('adj-dmx-operator-384-old'));
await recv([0xb2, 127, 64]);
await recv([0x92, 20, 127]);
await step('setPreset', { id: 'adj-dmx-operator-384', channel: 40 }, () => midi.setPreset('adj-dmx-operator-384', { channel: 40 }));
await recv([0xbf, 3, 64]);
await step('setPreset', { id: 'inconnu', channel: null }, () => midi.setPreset('inconnu'));
await step('setPreset', { id: null, channel: null }, () => midi.setPreset(null));

// correspondances apprises à la main : chaque sorte de fonction
const cc = (n, ch = 1) => ({ type: 'cc', channel: ch, number: n });
const nt = (n, ch = 1) => ({ type: 'note', channel: ch, number: n });
await add(cc(1), { kind: 'fader', index: 1 });
await add(cc(2), { kind: 'fader', index: 2 }, 'F2');
await add(cc(3), { kind: 'fader', index: 8 });
await add(nt(36), { kind: 'pageNext' });
await add(nt(37), { kind: 'pagePrev' });
await add(nt(38), { kind: 'familyNext' });
await add(nt(39), { kind: 'flash' });
await add(cc(64), { kind: 'blackout' });
await add(cc(10), { kind: 'groupLevel', name: 'Contres et lune' });
await add(cc(11), { kind: 'fixtureLevel', name: 'S60 lune' });
await add(cc(12), { kind: 'fixtureLevel', name: 'Inconnu' });
await add(cc(13), { kind: 'selectionLevel' });
await add(cc(14), { kind: 'selCct' });
await add(cc(15), { kind: 'selTint' });
await add(cc(16), { kind: 'selHue' });
await add(cc(17), { kind: 'selSat' });
await add(nt(40), { kind: 'group', name: 'Practicals' });
await add(nt(41), { kind: 'fixture', name: 'Titan enseigne 1' });
await add(nt(42), { kind: 'groupIndex', index: 2 });
await add(nt(43), { kind: 'fixtureIndex', index: 3 });
await add(nt(44), { kind: 'planIndex', index: 2 });
await add(nt(45), { kind: 'planIndex', index: 9 });
await add(nt(46), { kind: 'fixtureLatch' });
await add(nt(47), { kind: 'groupLatch', index: 2 });
await add(nt(48), { kind: 'groupNext' });
await add(nt(49), { kind: 'groupPrev' });
await add(nt(50), { kind: 'fixtureNext' });
await add(nt(51), { kind: 'fixturePrev' });
await add(cc(20), { kind: 'haze' });
await add(cc(21), { kind: 'exposure' });
await add(cc(22), { kind: 'ambient' });
await add(nt(52), { kind: 'command', id: 'console.full' });
await add(nt(53), { kind: 'command' });
await add({ type: 'pitch', channel: 2, number: 0 }, { kind: 'selectionLevel' });
await add({ type: 'program', channel: 1, number: 5 }, { kind: 'command', id: 'view.top' });
await add(nt(54), { kind: 'group', name: 'Face' });
await add(nt(54), { kind: 'flash' });
await add(cc(30), { kind: 'fader', index: 0 });
await add(null, { kind: 'fader', index: 12 });
for (const d of [[0xb0, 1, 64], [0xb0, 2, 127], [0xb0, 3, 10], [0xb0, 30, 5], [0x90, 36, 100], [0xb0, 1, 64], [0xb0, 3, 64], [0x80, 36, 0], [0x90, 36, 100], [0x90, 36, 100], [0x90, 36, 100], [0x90, 36, 100], [0x90, 36, 100], [0x90, 36, 100], [0xb0, 2, 64], [0x90, 37, 100], [0xb0, 2, 64]]) await recv(d);
for (const d of [[0x90, 38, 100], [0x90, 39, 100], [0x80, 39, 0], [0xb0, 64, 127], [0xb0, 64, 127], [0xb0, 64, 0], [0xb0, 10, 99], [0xb0, 11, 98], [0xb0, 12, 97], [0xb0, 13, 50]]) await recv(d);
for (const d of [[0x90, 40, 100], [0xb0, 2, 64], [0xb0, 13, 50], [0xb0, 14, 33], [0xb0, 15, 64], [0xb0, 16, 127], [0xb0, 17, 12], [0x90, 39, 1], [0x90, 39, 0]]) await recv(d);
for (const d of [[0x90, 41, 100], [0x90, 42, 100], [0x90, 42, 100], [0x90, 43, 100], [0x90, 44, 100], [0x90, 45, 100], [0x90, 46, 100], [0x90, 47, 100], [0x80, 46, 0], [0x80, 47, 0]]) await recv(d);
for (const d of [[0x90, 48, 100], [0x90, 48, 100], [0x90, 48, 100], [0x90, 48, 100], [0x90, 48, 100], [0x90, 49, 100], [0x90, 49, 100], [0x90, 50, 100], [0x90, 50, 100], [0x90, 51, 100]]) await recv(d);
await step('select', { id: null }, () => store.select(null));
for (const d of [[0x90, 51, 100], [0x90, 49, 100], [0x90, 50, 100]]) await recv(d);
for (const d of [[0xb0, 20, 0], [0xb0, 20, 127], [0xb0, 20, 33], [0xb0, 21, 0], [0xb0, 21, 127], [0xb0, 21, 98], [0xb0, 21, 77], [0xb0, 22, 101], [0x90, 52, 100], [0x90, 53, 100], [0xc0, 5], [0xc0, 5], [0xe1, 0x22, 0x33], [0x90, 54, 100], [0x80, 54, 0], [0xf8]]) await recv(d);
// sélection changée dans l'application : page remise à zéro
for (const d of [[0x90, 36, 100], [0x90, 36, 100]]) await recv(d);
await step('select', { id: 'fmvqe4s4en9g6' }, () => store.select('fmvqe4s4en9g6'));
await step('select', { id: 'fmvqe4s4en9g6' }, () => store.select('fmvqe4s4en9g6'));
await recv([0xb0, 1, 64]);
// apprentissage : nouvelle commande, réapprendre, assistants
await step('learnNew', {}, () => startLearn('new'));
await recv([0xb0, 70, 50]);
await step('learnNew', {}, () => startLearn('new'));
await recv([0x80, 71, 0]);
await recv([0x90, 71, 100]);
await recv([0x90, 71, 100]);
await step('learnNew', {}, () => startLearn('new'));
await recv([0xc3, 9]);
await step('relearn', { index: 0 }, () => startLearn(own(0)));
await recv([0xb0, 75, 10]);
await step('wizard', { kind: 'faders' }, () => toggleWizard('faders'));
for (const d of [[0xb0, 80, 1], [0xb0, 80, 2], [0x90, 60, 100], [0xb0, 81, 1], [0xe4, 1, 1], [0xb0, 2, 1], [0xb0, 83, 0], [0xb0, 84, 0], [0xb0, 85, 0]]) await recv(d);
await step('wizard', { kind: 'faders' }, () => toggleWizard('faders'));
await step('wizard', { kind: 'faders' }, () => toggleWizard('faders'));
for (const d of [[0xb0, 86, 0], [0xb0, 87, 0], [0xb0, 88, 0], [0xb0, 89, 0], [0xb0, 90, 0], [0xb0, 91, 0], [0xb0, 92, 0], [0xb0, 93, 0], [0xb0, 94, 0]]) await recv(d);
await step('wizard', { kind: 'groups' }, () => toggleWizard('groups'));
for (const d of [[0x90, 100, 100], [0x80, 100, 0], [0x90, 100, 100], [0xb0, 101, 10], [0xb0, 101, 127], [0xc0, 7], [0x90, 40, 1], [0xe0, 1, 1], [0x90, 102, 9]]) await recv(d);
await step('wizard', { kind: 'fixtures' }, () => toggleWizard('fixtures'));
for (const d of [[0x90, 110, 100], [0x90, 111, 100], [0x90, 110, 100], [0x90, 112, 100]]) await recv(d);
await step('wizard', { kind: 'groups' }, () => toggleWizard('groups'));
await recv([0x90, 113, 100]);
await step('stopLearning', {}, () => stopLearning());
await recv([0x90, 113, 100]);
// modifications, préréglage personnel, fichiers
await step('update', { index: 1, target: { kind: 'selHue' } }, () => midi.update(own(1), { target: { kind: 'selHue' } }));
await step('update', { index: 2, label: 'Lune' }, () => midi.update(own(2), { label: 'Lune' }));
await step('remove', { index: 0 }, () => midi.remove(own(0)));
// une correspondance sans commande ferait échouer le préréglage personnel (sourceKey(null) en JavaScript)
const noSource = midi.state.mappings.findIndex((m) => !m.source);
await step('remove', { index: noSource }, () => midi.remove(own(noSource)));
await step('setPreset', { id: 'adj-dmx-operator-384', channel: null }, () => midi.setPreset('adj-dmx-operator-384'));
for (const d of [[0xb0, 1, 64], [0xb0, 0, 64], [0xb0, 2, 64]]) await recv(d);
await step('exportPreset', { name: 'Ma console' }, () => midi.exportPreset('Ma console'));
await step('saveUserPreset', { name: 'Mon préréglage de plateau, un nom vraiment très long pour voir la coupure' }, () => void midi.saveUserPreset('Mon préréglage de plateau, un nom vraiment très long pour voir la coupure'));
for (const d of [[0xb0, 1, 64], [0xb0, 0, 64], [0xb0, 21, 64]]) await recv(d);
await step('setPreset', { userIndex: 0, channel: 9 }, () => midi.setPreset(midi.state.userPresets[0].id, { channel: 9 }));
const file = { format: 'arklight-midi-preset', version: 1, name: 'Fichier', mappings: [{ source: cc(5), target: { kind: 'haze' } }, { source: null, target: { kind: 'flash' } }, { target: { kind: 'zzz' }, source: cc(6) }] };
await step('importPreset', { json: file }, () => midi.importPreset(file));
await recv([0xb0, 5, 127]);
await step('importPreset', { json: { format: 'autre', mappings: [] } }, () => midi.importPreset({ format: 'autre', mappings: [] }));
await step('importPreset', { json: { format: 'arklight-midi-preset', mappings: [{ source: null, target: { kind: 'flash' } }] } }, () => midi.importPreset({ format: 'arklight-midi-preset', mappings: [{ source: null, target: { kind: 'flash' } }] }));
await step('importPreset', { json: { format: 'arklight-midi-preset', name: 42, mappings: [{ source: cc(9), target: { kind: 'flash' } }] } }, () => midi.importPreset({ format: 'arklight-midi-preset', name: 42, mappings: [{ source: cc(9), target: { kind: 'flash' } }] }));
await step('deleteUserPreset', { userIndex: 2 }, () => midi.deleteUserPreset(midi.state.userPresets[2].id));
await step('deleteUserPreset', { userIndex: 0 }, () => midi.deleteUserPreset(midi.state.userPresets[0].id));
await step('clear', {}, () => midi.clear());
await setInputs([{ name: 'DMX Operator 384 MIDI', manufacturer: '' }]);

// réglages enregistrés sur le poste (relus par la version Rust)
const settings = { mappings: JSON.parse(storage.get('arklight.midi.mappings')), preset: JSON.parse(storage.get('arklight.midi.preset')), userPresets: JSON.parse(storage.get('arklight.midi.userPresets')) };

// ------------------------------------------------------------------ autre sorte de fonction
store.select('fmvqe4s4en9g6');
const previous = [{ target: { kind: 'fader', index: 5 } }, { target: { kind: 'group', name: 'Practicals' } }, { target: { kind: 'groupLevel', name: 'Disparu' } }, { target: { kind: 'flash' } }];
const retarget = [];
for (const m of previous) for (const k of TARGET_KINDS) retarget.push({ previous: m.target, kind: k.id, target: targetForKind(k.id, m), selectedId: store.selectedId });
store.select(null);
for (const k of ['fixture', 'fixtureLevel']) retarget.push({ previous: { kind: 'flash' }, kind: k, target: targetForKind(k, { target: { kind: 'flash' } }), selectedId: null });

mkdirSync(dirname(OUT), { recursive: true });
writeFileSync(
  OUT,
  JSON.stringify({
    kinds,
    parse,
    notes,
    scale,
    presets,
    clean,
    route,
    scenario: { scene: { fixtures: scene.fixtures, groups: scene.groups }, plans: ['p1', 'p2', 'p3'], fixtureOrder: fixtureOrder(), faderCount: 48, steps, settings },
    retarget,
  }),
);
console.log(`${OUT} : ${parse.length} messages, ${steps.length} étapes du contrôleur, ${retarget.length} changements de fonction`);
