// Version Rust d'ARKLIGHT : données d'essai de parité du moteur lumière, calculées par la version
// JavaScript (shared/engine). Le moteur Rust (rust/crates/arklight-core/src/engine) doit donner les
// mêmes résultats sur les mêmes entrées.
//   node tools/rust/export-engine-data.mjs
// Écrit rust/crates/arklight-core/tests/data/engine.json :
//   gels       couleurs index Astera (0..255), couleurs de gels d'après leur nom, spécifications de couleur
//   modes      chaque mode DMX distinct de la bibliothèque : canaux à plat
//   decode     décodage DMX (trames pseudo-aléatoires) : attributs et couleur de chaque pixel
//   console    valeurs de départ, réglage → valeurs, changement de profil, réglage des valeurs, couleur
//   scenarios  sortie de projecteurs image après image (manuel, DMX, console, Astera, tungstène, HMI)
//   demos      sortie de chaque projecteur des projets d'exemple (lancer d'abord export-test-data.mjs)
//   effective  type effectif (zoom, boîte à lumière, dôme, grille, gélatine) de chaque modèle
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { FIXTURE_TYPES, defaultSettings, defaultState, effectiveType, getMode } from '../../shared/fixtures/index.js';
import { FILTERS } from '../../shared/filters.js';
import { GRIDS, accessoriesFor } from '../../shared/modifiers.js';
import { ASTERA_INDEX_COLORS } from '../../shared/fixtures/astera-dmx.js';
import { LEE_GELS, STATIC_COLORS, colorSpecRgb } from '../../shared/fixtures/astera-colors.js';
import { cctToRgb, hsToRgb } from '../../shared/engine/color.js';
import { gelColor } from '../../shared/engine/gels.js';
import { DIMMING_CURVES, compileMode, decodeDmxOutput, readAttributes } from '../../shared/engine/dmx.js';
import { adjustColor, defaultFaders, lookOfValues, lookToValues, remapValues, seedConsole } from '../../shared/engine/console.js';
import { computeOutput } from '../../shared/engine/output.js';

const OUT = fileURLToPath(new URL('../../rust/crates/arklight-core/tests/data/engine.json', import.meta.url));

// générateur pseudo-aléatoire reproductible (mulberry32)
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const rand = rng(20261027);
const pick = (list) => list[Math.floor(rand() * list.length)];
const int = (lo, hi) => lo + Math.floor(rand() * (hi - lo + 1));

/** Trame variée : beaucoup de 0 et de 255, des petites valeurs (seuils des tables Astera), le reste au hasard. */
function frame() {
  const d = new Uint8Array(512);
  for (let i = 0; i < 512; i++) {
    const r = rand();
    d[i] = r < 0.35 ? 0 : r < 0.5 ? 255 : r < 0.6 ? int(1, 6) : int(0, 255);
  }
  return d;
}
/** Trame en hexadécimal (512 octets). */
const hex = (d) => Buffer.from(d).toString('hex');
const arr = (f64) => Array.from(f64);
const attrs = (o) => Object.fromEntries(Object.entries(o).sort());

// ------------------------------------------------------------------ couleurs
const gelNames = [...LEE_GELS.map((g) => g.name), 'Full CT Blue', 'Half C.T. Orange', '1/4 CT Straw', 'Eighth CT Blue', 'Double CT Orange', 'Plus Green', 'Half Minus Green', 'Quarter Plus Green', 'Neutral Density 0.3', 'ND', 'Grey', 'Frost', 'Thing'];
const specs = [
  null,
  ...STATIC_COLORS.map(([name]) => ({ kind: 'named', name })),
  { kind: 'named', name: 'NOPE' },
  { kind: 'custom', rgb: [12, 200, 90] },
  { kind: 'custom' },
  { kind: 'index', value: 2 },
  { kind: 'index', value: 106 },
  { kind: 'index', value: 0 },
  { kind: 'index' },
  { kind: 'hsi', cct: 3200, green: 0.3, hue: 200, sat: 70 },
  { kind: 'hsi', hue: 20 },
  { kind: 'gel', family: 'Lee Tungsten', number: LEE_GELS[5].number },
  { kind: 'gel', family: 'Lee Daylight', number: LEE_GELS[40].number },
  { kind: 'gel', number: 9999 },
  { kind: 'weird' },
];
const gels = {
  index: Array.from({ length: 256 }, (_, v) => ({ rgb: ASTERA_INDEX_COLORS.color(v), name: ASTERA_INDEX_COLORS.name(v) })),
  names: gelNames.map((name) => ({ name, rgb: gelColor(name) })),
  specs: specs.map((spec) => ({ spec, rgb: colorSpecRgb(spec, { cct: cctToRgb, hs: hsToRgb }) })),
};

// ------------------------------------------------------------------ modes distincts
const distinct = [];
const seen = new Set();
for (const t of FIXTURE_TYPES) {
  for (const m of t.dmx?.modes || []) {
    const k = JSON.stringify([m.channels, m.pixels, t.pixels]);
    if (seen.has(k)) continue;
    seen.add(k);
    distinct.push({ type: t, mode: m });
  }
}
const modes = distinct.map(({ type, mode }) => ({
  type: type.id,
  mode: mode.id,
  footprint: compileMode(mode).footprint,
  // « clé/pixel/octets » de chaque canal (la position découle des octets)
  slots: compileMode(mode)
    .slots.map((s) => `${s.key ?? ''}/${s.px}/${s.bytes}`)
    .join(' '),
}));

// ------------------------------------------------------------------ décodage DMX
const curves = ['linear', 'square', 'invSquare', 'sCurve', 'FAST', 'STANDARD', 'HALOGEN', 'THEATER', 'TV'];
const decode = [];
for (const { type, mode } of distinct) {
  const pixels = type.pixels || 1;
  const fp = compileMode(mode).footprint;
  for (let c = 0; c < 2; c++) {
    const data = frame();
    // adresse 1, ou au hasard (parfois si loin que le mode déborde de l'univers)
    const address = c === 0 ? 1 : rand() < 0.15 ? int(500, 512) : int(1, Math.max(1, 513 - fp));
    const white = pick([3200, 5600, 6500]);
    const curve = pick(curves);
    const time = Math.round(rand() * 20000) / 1000;
    const out = new Float64Array(pixels * 3);
    const runtime = {};
    const p = { mode, data, address, pixelCount: pixels, whiteRgb: cctToRgb(white), curve: DIMMING_CURVES[curve], indexColor: ASTERA_INDEX_COLORS.color, runtime };
    // une première image (départ des effets), puis celle qui est comparée
    decodeDmxOutput({ ...p, time: 0.5 }, new Float64Array(pixels * 3));
    decodeDmxOutput({ ...p, time }, out);
    const modePx = typeof mode.pixels === 'number' ? Math.max(1, Math.min(mode.pixels, pixels)) : pixels;
    const read = c === 0 ? readAttributes(mode, data, address, modePx) : null;
    decode.push({
      type: type.id,
      mode: mode.id,
      pixels,
      address,
      white,
      curve,
      time,
      // seuls les octets lus par le mode comptent : [premier indice, octets en hexadécimal]
      data: [Math.min(512, address - 1), hex(data.subarray(Math.min(512, address - 1), address - 1 + fp))],
      out: arr(out),
      ...(read ? { fixture: attrs(read.fixture), attrs: read.pixels.map(attrs) } : {}),
    });
  }
}

// ------------------------------------------------------------------ console
const looks = [
  { level: 0.7, cct: 3200, tint: 0.2 },
  { level: 1, hue: 200, sat: 0.8, cct: 5600 },
  { level: 0.4, rgb: [1, 0.3, 0.1] },
  { level: 0.55, cct: 6500 },
  { level: 0, cct: 5600, tint: 0 },
];
const randomValues = (mode) => compileMode(mode).slots.map((s) => (rand() < 0.3 ? 0 : int(0, s.bytes === 2 ? 65535 : 255)));
const consoleCases = { defaults: [], toValues: [], remap: [], lookOf: [], adjust: [] };
// un mode sur cinq (le calcul des réglages est long), plus tous ceux des Astera à effets
const sample = distinct.filter((d, i) => i % 5 === 0 || d.mode.pixels === 'effet');
for (const { type, mode } of sample) {
  const pixels = type.pixels || 1;
  const opts = { whiteK: pick([5600, 4300]), curve: pick(['linear', 'square']) };
  const o = { whiteK: opts.whiteK, curve: DIMMING_CURVES[opts.curve] };
  consoleCases.defaults.push({ type: type.id, mode: mode.id, values: defaultFaders(mode) });
  const look = pick(looks);
  const modePx = typeof mode.pixels === 'number' ? Math.max(1, Math.min(mode.pixels, pixels)) : pixels;
  const perPixel = modePx > 1 && rand() < 0.5 ? Array.from({ length: modePx }, () => pick(looks)) : null;
  consoleCases.toValues.push({ type: type.id, mode: mode.id, looks: perPixel || [look], opts, values: lookToValues(mode, perPixel || look, o) });
  const values = randomValues(mode);
  const other = pick(type.dmx.modes);
  consoleCases.remap.push({ type: type.id, from: mode.id, to: other.id, values, pixels, opts, result: remapValues(mode, other, values, pixels, o) });
  consoleCases.lookOf.push({ type: type.id, mode: mode.id, values, pixels, opts, look: lookOfValues(mode, values, { pixelCount: pixels, ...o }) });
  const change = pick([{ cct: 3200 }, { tint: -0.5 }, { hue: 120 }, { sat: 0.3 }, { cct: 4300, tint: 0.4 }]);
  const res = adjustColor(mode, values, change, { pixelCount: pixels, ...o });
  consoleCases.adjust.push({ type: type.id, mode: mode.id, values, pixels, opts, change, result: res.values, look: res.look });
}

// ------------------------------------------------------------------ scénarios (image après image)
const type = (id) => FIXTURE_TYPES.find((t) => t.id === id);
function fixture(typeId, settings = {}, state = {}) {
  const t = type(typeId);
  const s = { ...defaultSettings(t), ...settings };
  const st = { ...defaultState(t), ...state };
  return { id: 'f', type: typeId, settings: s, state: st };
}
const universe = (fill) => {
  const d = frame();
  if (fill) for (const [i, v] of Object.entries(fill)) d[Number(i)] = v;
  return d;
};

const scenarios = [];
function scenario(name, f, steps) {
  const t = type(f.type);
  const rt = {};
  const out = new Float64Array((t.pixels || 0) * 3);
  const recorded = [];
  for (const step of steps) {
    const dmx = new Map(Object.entries(step.dmx || {}).map(([u, d]) => [Number(u), d]));
    const ctx = { dmx, live: new Set(step.live || []), time: step.time };
    const source = computeOutput(f, t, ctx, out, rt);
    recorded.push({ time: step.time, dmx: Object.fromEntries([...dmx].map(([u, d]) => [u, hex(d)])), live: step.live || [], out: arr(out), source });
  }
  scenarios.push({ name, fixture: JSON.parse(JSON.stringify(f)), steps: recorded });
}
const times = (list, extra = {}) => list.map((time) => ({ time, ...extra }));

// LED d'une autre marque (premier type « generic » à pixels)
const led = FIXTURE_TYPES.find((t) => t.platform === 'generic' && t.pixels > 1 && t.dmx?.modes?.length > 1);
const ledSingle = FIXTURE_TYPES.find((t) => t.platform === 'generic' && t.pixels === 1 && !t.technology?.match(/tungsten|hmi/) && t.dmx?.modes?.length > 1);
for (const t of [led, ledSingle]) {
  scenario(`${t.id} manuel Kelvin`, fixture(t.id, {}, { manual: { mode: 'cct', intensity: 0.8, cct: 3200, tint: 0.3 } }), times([0, 1]));
  scenario(`${t.id} manuel HSI`, fixture(t.id, { whitePoint: 4300, dimmingCurve: 'square' }, { manual: { mode: 'hsi', intensity: 0.5, hue: 300, sat: 0.6 } }), times([0]));
  scenario(`${t.id} manuel RVB`, fixture(t.id, {}, { manual: { mode: 'rgb', intensity: 1, rgb: [0.2, 0.9, 0.4] } }), times([0]));
  scenario(`${t.id} éteint`, fixture(t.id, {}, { power: false }), times([0]));
  const m2 = t.dmx.modes[1];
  scenario(`${t.id} anciens « Canaux »`, fixture(t.id, { dmxMode: m2.id }, { manual: { mode: 'faders', intensity: 1, faders: { mode: m2.id, values: randomValues(m2) } } }), times([0, 0.3]));
  const u = universe();
  scenario(`${t.id} DMX reçu`, fixture(t.id, { universe: 2, dmxAddress: 17, dmxMode: m2.id }), [{ time: 0 }, { time: 0.4, dmx: { 2: u }, live: [2] }, { time: 0.8, dmx: { 2: u } }]);
  scenario(`${t.id} DMX imposé sans trame`, fixture(t.id, { source: 'dmx' }), times([0]));
  scenario(`${t.id} manuel imposé malgré le DMX`, fixture(t.id, { source: 'manual' }), [{ time: 0, dmx: { 1: u }, live: [1] }]);
  const m0 = t.dmx.modes[0];
  scenario(`${t.id} console même mode`, fixture(t.id, { dmxMode: m0.id }, { console: { mode: m0.id, values: randomValues(m0) } }), [{ time: 0, dmx: { 1: u }, live: [1] }]);
  scenario(`${t.id} console autre profil`, fixture(t.id, { dmxMode: m0.id }, { console: { mode: m2.id, values: randomValues(m2) } }), times([0, 0.5]));
}

// Astera Titan (tube 16 pixels) : mode autonome, entrées, DMX FAIL…
const sa = (program, extra = {}) => ({ standalone: { program, intensity: 80, speed: 6, fade: 30, colors: [{ kind: 'named', name: 'RED' }, { kind: 'hsi', cct: 4300, green: 0, hue: 200, sat: 90 }, { kind: 'index', value: 106 }, { kind: 'custom', rgb: [10, 220, 120] }], ...extra } });
for (let program = 0; program < 21; program++) scenario(`titan autonome programme ${program}`, fixture('astera.titan', { ...sa(program), brightness: 70 }), times([0, 0.37, 1.9, 7.25]));
scenario('titan autonome sans programme', fixture('astera.titan', { standalone: { colors: [{ kind: 'named', name: 'BLUE' }] } }), times([2, 3]));
scenario('titan BlueMode', fixture('astera.titan', {}, { blueMode: true }), times([0, 0.2, 0.4]));
scenario('titan veille', fixture('astera.titan', {}, { standby: true }), times([0]));
scenario('titan éteint', fixture('astera.titan', {}, { power: false }), times([0]));
scenario('titan ACCEPT CONFIG', fixture('astera.titan', sa(3), { acceptConfig: true }), times([0.05, 1, 2.1]));
scenario('titan appli', fixture('astera.titan', { inputSelect: 'APP CONTROL' }, { app: { rgb: [1, 0.4, 0.1], intensity: 0.6 } }), times([0]));
scenario('titan appli (AUTO)', fixture('astera.titan', { crmxLinked: false }, { app: { rgb: [0.1, 0.4, 1] } }), times([0, 1]));
const tu = universe({ 0: 255 });
const titan = type('astera.titan');
for (const m of titan.dmx.modes.filter((_, i) => i % 7 === 0)) {
  scenario(`titan CRMX mode ${m.id}`, fixture('astera.titan', { dmxMode: m.id, universe: 3, dmxAddress: 5 }), [
    { time: 0 },
    { time: 0.5, dmx: { 3: tu }, live: [3] },
    { time: 1.3, dmx: { 3: tu }, live: [3] },
  ]);
}
for (const fail of ['HOLD', 'EMERGENCY LIGHT', 'BLACKOUT', 'STANDALONE']) {
  scenario(`titan DMX FAIL ${fail}`, fixture('astera.titan', { ...sa(5), dmxFail: fail }), [
    { time: 0, dmx: { 1: tu }, live: [1] },
    { time: 0.5, dmx: { 1: tu } },
    { time: 1, dmx: {} },
  ]);
}
scenario('titan filaire', fixture('astera.titan', { crmxLinked: false, wired: true }), [{ time: 0, dmx: { 1: tu }, live: [1] }]);
scenario('titan entrée imposée CRMX', fixture('astera.titan', { inputSelect: 'CRMX wirel. DMX', crmxLinked: false, dmxFail: 'EMERGENCY LIGHT' }), [{ time: 0, dmx: { 1: tu }, live: [1] }]);
scenario('titan ART3 (autonome)', fixture('astera.titan', { ...sa(9), inputSelect: 'ART3 DMX' }), times([0, 1]));
scenario('titan autonomie 10 h', fixture('astera.titan', { ...sa(0), runtime: 10 }), times([0]));
scenario('titan sans bridage', fixture('astera.titan', { ...sa(0), runtime: 10, runtimeLimit: false }), times([0]));
scenario('titan courbe HALOGEN', fixture('astera.titan', { dimmerCurve: 'HALOGEN', dmxMode: titan.dmx.modes[2].id }), [{ time: 0, dmx: { 1: tu }, live: [1] }]);
const tm = titan.dmx.modes;
scenario('titan console', fixture('astera.titan', { dmxMode: tm[1].id }, { console: { mode: tm[1].id, values: randomValues(tm[1]) } }), times([0, 0.6]));
scenario('titan console autre profil', fixture('astera.titan', { dmxMode: tm[0].id }, { console: { mode: tm[9].id, values: randomValues(tm[9]) } }), times([0]));
const fxMode = tm.find((m) => m.pixels === 'effet');
if (fxMode) {
  const v = defaultFaders(fxMode).map((x, i) => (compileMode(fxMode).slots[i].f === 'fxProgram' ? 70 : x));
  scenario(`titan console effet ${fxMode.id}`, fixture('astera.titan', { dmxMode: fxMode.id }, { console: { mode: fxMode.id, values: v } }), times([0, 0.5, 1.5, 4]));
}
scenario('quikspot OUTPUT GAIN OFF', fixture('astera.quikspot', { ...sa(0), outputGain: 'OFF' }), times([0]));
scenario('solabulb (secteur)', fixture('astera.solabulb', sa(15)), times([0, 2]));
const box = FIXTURE_TYPES.find((t) => !t.pixels);
if (box) scenario('boîtier sans lumière', fixture(box.id), times([0]));

// tungstène et HMI
scenario('tungstène montée et descente', fixture('arri.650-plus', {}, { manual: { mode: 'cct', intensity: 1 } }), times([0, 0.02, 0.05, 0.1, 0.3, 1, 1.5]));
scenario('tungstène gradué (courbe square)', fixture('mole.baby-1k', { dimmingCurve: 'square' }, { manual: { intensity: 0.5 } }), times([0, 0.1, 2]));
scenario('tungstène éteint', fixture('ianiro.redhead-800', {}, { power: false }), times([0, 1]));
scenario('tungstène DMX', fixture('mole.baby-junior-2k', { universe: 4, dmxAddress: 100 }), [
  { time: 0, dmx: { 4: universe({ 99: 128 }) }, live: [4] },
  { time: 0.2, dmx: { 4: universe({ 99: 255 }) }, live: [4] },
  { time: 3, dmx: { 4: universe({ 99: 0 }) }, live: [4] },
]);
scenario('tungstène console', fixture('dedolight.dlh4', {}, { console: { mode: 'dim1', values: [200] } }), times([0, 0.5]));
for (const m of type('mole.molefay-9').dmx.modes) {
  scenario(`molefay ${m.id}`, fixture('mole.molefay-9', { dmxMode: m.id }), [{ time: 0, dmx: { 1: universe() }, live: [1] }, { time: 0.4, dmx: { 1: universe() }, live: [1] }]);
}
scenario('HMI allumage', fixture('arri.m18', {}, { manual: { intensity: 0.7 } }), times([0, 1, 5, 20, 60]));
scenario('HMI gradé à 0', fixture('k5600.joker2-800', {}, { manual: { intensity: 0 } }), times([0, 10]));
scenario('HMI éteint', fixture('arri.m18', {}, { power: false }), times([0, 10]));

// ------------------------------------------------------------------ console : départ d'après le projecteur
const seeds = [];
function seed(name, f, modeId, opts = {}) {
  const t = type(f.type);
  const mode = getMode(t, modeId);
  seeds.push({ name, fixture: JSON.parse(JSON.stringify(f)), mode: mode.id, source: opts.source ?? null, dmx: opts.dmx ? Object.fromEntries([...opts.dmx].map(([u, d]) => [u, hex(d)])) : null, values: seedConsole(f, t, mode, opts) });
}
seed('LED manuel → mode 2', fixture(led.id, {}, { manual: { mode: 'cct', intensity: 0.8, cct: 3200, tint: 0.3 } }), led.dmx.modes[1].id);
seed('LED HSI → mode 1', fixture(led.id, {}, { manual: { mode: 'hsi', intensity: 0.5, hue: 300, sat: 0.6 } }), led.dmx.modes[0].id);
const lu = universe();
seed('LED DMX → autre mode', fixture(led.id, { dmxAddress: 3 }), led.dmx.modes[1].id, { source: 'dmx', dmx: new Map([[1, lu]]) });
seed('LED DMX → même mode', fixture(led.id, { dmxAddress: 3 }), led.dmx.modes[0].id, { source: 'dmx', dmx: new Map([[1, lu]]) });
seed('Titan autonome → mode 1', fixture('astera.titan', sa(0)), tm[0].id);
seed('Titan autonome HSI → mode 5', fixture('astera.titan', { standalone: { intensity: 60, colors: [{ kind: 'hsi', cct: 3200, green: 0.2, hue: 30, sat: 40 }] } }), tm[4].id);
seed('Titan appli → mode 2', fixture('astera.titan', {}, { app: { rgb: [1, 0.2, 0.6], intensity: 0.9 } }), tm[1].id, { source: 'app' });
seed('Titan tenu par la console', fixture('astera.titan', {}, { console: { mode: tm[0].id, values: randomValues(tm[0]) } }), tm[3].id);
seed('tungstène manuel', fixture('arri.650-plus', {}, { manual: { intensity: 0.42 } }), 'dim1');

// ------------------------------------------------------------------ projets d'exemple (sans DMX, à 1 s)
const demos = {};
for (const id of ['night', 'clip']) {
  const project = JSON.parse(readFileSync(fileURLToPath(new URL(`../../rust/crates/arklight-core/tests/data/demo-${id}.json`, import.meta.url)), 'utf8'));
  demos[id] = project.fixtures.map((f) => {
    const t = type(f.type);
    const out = new Float64Array((t?.pixels || 0) * 3);
    const source = t ? computeOutput(f, t, { dmx: new Map(), live: new Set(), time: 1 }, out, {}) : null;
    return { id: f.id, source, out: arr(out) };
  });
}

// ------------------------------------------------------------------ type effectif (zoom, accessoires)
const effective = [];
const diffusions = FILTERS.filter((f) => f.group === 'diffusion');
for (const t of FIXTURE_TYPES) {
  const allowed = accessoriesFor(t);
  const cases = [{}];
  if (t.zoom) cases.push({ zoom: t.zoom.min }, { zoom: (t.zoom.min + t.zoom.max) / 2 }, { zoom: t.zoom.max + 10 }, { zoom: String(t.zoom.detent ?? t.zoom.min) }, { zoom: null });
  for (const m of allowed.modifiers) {
    cases.push({ modifier: m.id }, { modifier: m.id, front: 'none' }, { modifier: m.id, front: pick(diffusions).id, grid: pick(GRIDS).id, ...(t.zoom ? { zoom: t.zoom.max } : {}) });
  }
  if (allowed.grids) for (const g of GRIDS) cases.push({ grid: g.id });
  if (allowed.gel) for (let i = 0; i < 4; i++) cases.push({ gel: pick(FILTERS).id, ...(rand() < 0.5 ? { grid: pick(GRIDS).id } : {}) });
  cases.push({ modifier: 'nope', grid: 'nope', gel: 'nope' });
  for (const settings of cases) {
    const f = { id: 'f', type: t.id, settings: { ...defaultSettings(t), ...settings } };
    const e = effectiveType(t, f);
    const a = e.accessories;
    effective.push({
      fixture: f,
      beamAngle: e.beamAngle ?? null,
      luxAt1m: e.output?.luxAt1m ?? null,
      lumens: e.output?.lumens ?? null,
      acc: a ? { angle: a.angle, lux: a.lux, transmission: a.transmission, tint: a.tint, modifier: a.modifier?.id ?? null, grid: a.grid?.id ?? null, gel: a.gel?.id ?? null, front: a.front === 'none' ? 'none' : (a.front?.id ?? null) } : null,
    });
  }
}

const data = { gels, modes, decode, console: consoleCases, scenarios, seeds, demos, effective };
// 12 chiffres significatifs : largement assez pour la tolérance des essais (1e-9), deux fois moins lourd
writeFileSync(OUT, JSON.stringify(data, (k, v) => (typeof v === 'number' && !Number.isInteger(v) ? Number(v.toPrecision(12)) : v)));
console.log(`engine.json : ${modes.length} modes, ${decode.length} décodages, ${sample.length} modes pour la console, ${scenarios.length} scénarios, ${seeds.length} départs de console`);
