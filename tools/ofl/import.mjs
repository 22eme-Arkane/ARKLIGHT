// Importe des projecteurs de l'Open Fixture Library (https://open-fixture-library.org, licence MIT)
// et les convertit au format de la bibliothèque ARKLIGHT.
//
//   node tools/ofl/import.mjs           → réécrit shared/fixtures/data/ofl-fixtures.js
//
// Les profils DMX (canaux, modes, plages) viennent tels quels de l'OFL. La sortie lumineuse n'y figure
// presque jamais : elle est estimée (puissance × rendement typique) et marquée « estimated ».
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const OUT = join(ROOT, 'shared', 'fixtures', 'data', 'ofl-fixtures.js');
const RAW = 'https://raw.githubusercontent.com/OpenLightingProject/open-fixture-library/master/fixtures/';

// Sélection (cinéma) et ce que l'OFL ne dit pas : famille, forme 3D, angle par défaut.
//   cat : panel | spot | fresnel     beam : angle de faisceau (°) si l'OFL n'en donne pas
const SELECTION = [
  { mfr: 'arri', key: 'skypanel-s30c', cat: 'panel' },
  { mfr: 'arri', key: 'skypanel-s60c', cat: 'panel' },
  { mfr: 'arri', key: 'skypanel-s120c', cat: 'panel' },
  { mfr: 'arri', key: 'skypanel-s360c', cat: 'panel' },
  { mfr: 'arri', key: 'l5-c', cat: 'fresnel' },
  { mfr: 'arri', key: 'l7-c', cat: 'fresnel' },
  { mfr: 'arri', key: 'l10-c', cat: 'fresnel' },
  { mfr: 'arri', key: 'broadcaster-2-plus', cat: 'panel' },
  { mfr: 'aputure', key: 'ls-600d-pro', cat: 'spot', beam: 55 },
  { mfr: 'aputure', key: 'ls-600d', cat: 'spot', beam: 55 },
  { mfr: 'aputure', key: 'ls-600x-pro', cat: 'spot', beam: 55 },
  { mfr: 'aputure', key: 'ls-1200d-pro', cat: 'spot', beam: 55 },
  { mfr: 'aputure', key: 'ls-300x', cat: 'spot', beam: 55 },
  { mfr: 'aputure', key: 'c300d', cat: 'spot', beam: 55 },
  { mfr: 'aputure', key: 'nova-p300c', cat: 'panel', beam: 110 },
  { mfr: 'dedolight', key: 'dled4-bi', cat: 'spot' },
  { mfr: 'dedolight', key: 'dled7-bi', cat: 'spot' },
  { mfr: 'dmg-lumiere', key: 'sl1-mix', cat: 'panel', beam: 110 },
  { mfr: 'dmg-lumiere', key: 'maxi-mix', cat: 'panel', beam: 110 },
  { mfr: 'dmg-lumiere', key: 'mini-mix', cat: 'panel', beam: 110 },
  { mfr: 'kino-flo', key: 'celeb-250-led-dmx', cat: 'panel', beam: 100 },
  { mfr: 'kino-flo', key: 'celeb-450-led-dmx', cat: 'panel', beam: 100 },
  { mfr: 'kino-flo', key: 'celeb-led-201-dmx', cat: 'panel', beam: 100 },
  { mfr: 'litegear', key: 'litemat-plus-2', cat: 'panel', beam: 120 },
  { mfr: 'litegear', key: 'litemat-plus-4', cat: 'panel', beam: 120 },
  { mfr: 'litegear', key: 's2-litemat-2', cat: 'panel', beam: 120 },
  { mfr: 'litegear', key: 's2-litemat-4', cat: 'panel', beam: 120 },
  { mfr: 'hive', key: 'wasp-100-c', cat: 'spot', beam: 40 },
  { mfr: 'hive', key: 'hornet-200-c', cat: 'spot', beam: 40 },
  { mfr: 'hive', key: 'super-hornet-575-c', cat: 'spot', beam: 40 },
];

const BRAND_NAMES = { arri: 'ARRI', aputure: 'Aputure', dedolight: 'Dedolight', 'dmg-lumiere': 'DMG Lumière', 'kino-flo': 'Kino Flo', litegear: 'LiteGear', hive: 'Hive Lighting' };

// Rendement lumineux typique d'un projecteur LED complet (lm/W) pour estimer la sortie
const EFFICACY = { panel: 45, spot: 70, fresnel: 40 };
// Part du flux dans le faisceau
const BEAM_SHARE = { panel: 0.85, spot: 0.6, fresnel: 0.5 };

const COLOR_FN = { Red: 'red', Green: 'green', Blue: 'blue', White: 'white', Amber: 'amber', Lime: 'lime', Cyan: 'cyan', UV: 'uv', 'Warm White': 'ww', 'Cold White': 'cw', Magenta: 'magenta', Yellow: 'yellow', Indigo: 'indigo' };
// Fonctions qu'un projecteur à plusieurs zones peut répéter par zone
const ZONE_FNS = new Set(['dimmer', 'cct', 'tint', 'red', 'green', 'blue', 'white', 'amber', 'lime', 'cyan', 'uv', 'ww', 'cw', 'hue', 'sat']);

const kelvin = (s) => Number(String(s).replace(/[^\d.]/g, '')) || null;
const hz = (s) => Number(String(s).replace(/[^\d.]/g, '')) || null;

/** Fonction ARKLIGHT d'un canal OFL (+ options : plage, zone neutre, stroboscope). */
function channelDef(name, ch, maxValue) {
  const caps = ch.capability ? [ch.capability] : ch.capabilities || [];
  const types = new Set(caps.map((c) => c.type));
  const n = name.toLowerCase();
  const def = { label: name };
  if (caps.length && caps.every((c) => c.type === 'Intensity')) return { ...def, f: 'dimmer' };
  const color = caps.find((c) => c.type === 'ColorIntensity');
  if (color && caps.every((c) => c.type === 'ColorIntensity' || c.type === 'NoFunction')) return { ...def, f: COLOR_FN[color.color] || 'none' };
  if (types.has('ColorTemperature')) {
    const ks = caps.filter((c) => c.type === 'ColorTemperature').flatMap((c) => [kelvin(c.colorTemperatureStart ?? c.colorTemperature), kelvin(c.colorTemperatureEnd ?? c.colorTemperature)]).filter(Boolean);
    if (ks.length) return { ...def, f: 'cct', range: [Math.min(...ks), Math.max(...ks)] };
  }
  if (types.has('ShutterStrobe')) {
    const strobes = caps.filter((c) => c.type === 'ShutterStrobe' && /strobe/i.test(c.shutterEffect || ''));
    if (strobes.length) {
      const first = strobes[0];
      const open = Math.max(0, (first.dmxRange?.[0] ?? 10) - 1);
      return { ...def, f: 'strobe', strobe: { open: Math.round((open * 255) / maxValue), min: hz(first.speedStart ?? first.speed) || 1, max: hz(strobes.at(-1).speedEnd ?? strobes.at(-1).speed) || 25 } };
    }
  }
  if (/green.?magenta|magenta.?green|plus\/minus green|\+\/- ?green|^g\/?m\b|^tint$/.test(n)) {
    // « 0–10 : neutre » (ARRI, Kino…) : en dessous du seuil, pas de correction
    const neutral = caps.find((c) => c.dmxRange?.[0] === 0 && /neutral|no correction|off|default/i.test(c.comment || ''));
    return { ...def, f: 'tint', ...(neutral ? { neutral: +(neutral.dmxRange[1] / maxValue).toFixed(4) } : {}) };
  }
  if (/cross ?fade|crossfade|cct ?<-> ?(rgb|hsi)/.test(n)) return { ...def, f: 'crossfade' };
  if (/^hue$|^hue \d$/.test(n)) return { ...def, f: 'hue' };
  if (/^saturation$|^saturation \d$/.test(n)) return { ...def, f: 'sat' };
  if (/^(cie )?x$|colorspace x/.test(n)) return { ...def, f: 'x', range: [0, 1] };
  if (/^(cie )?y$|colorspace y/.test(n)) return { ...def, f: 'y', range: [0, 1] };
  return { ...def, f: 'none' };
}

/** Convertit un mode OFL en mode ARKLIGHT (canaux 16 bits fusionnés, zones numérotées). */
function convertMode(j, mode, index) {
  const all = j.availableChannels || {};
  const fineOf = new Map(); // nom du canal fin -> canal gros
  for (const [name, ch] of Object.entries(all)) for (const alias of ch.fineChannelAliases || []) fineOf.set(alias, name);
  const zoneOf = (name) => {
    const m = / (\d+)$/.exec(name);
    return m && Number(m[1]) >= 2 ? Number(m[1]) - 1 : -1;
  };
  // Zones (ex. LiteMat « Double ») seulement s'il existe un « Dimmer 2 » ; sinon « Color Temperature 2 » est un 2e réglage
  const numbered = mode.channels.some((c) => typeof c === 'string' && zoneOf(c) > 0 && /^(dimmer|intensity)/i.test(c));
  const channels = [];
  for (const c of mode.channels) {
    if (c === null || typeof c !== 'string') {
      channels.push({ f: 'none', label: '—' });
      continue;
    }
    const coarse = fineOf.get(c);
    if (coarse) {
      const prev = channels.at(-1);
      if (prev && prev.ofl === coarse && !prev.bytes) prev.bytes = 2;
      else channels.push({ f: 'none', label: c });
      continue;
    }
    const ch = all[c];
    if (!ch) {
      channels.push({ f: 'none', label: c });
      continue;
    }
    const max = ch.fineChannelAliases?.length && mode.channels.includes(ch.fineChannelAliases[0]) ? 65535 : 255;
    const def = channelDef(c.replace(/ \d+$/, (s) => (zoneOf(c) > 0 ? '' : s)), ch, max);
    def.label = c;
    if (numbered && ZONE_FNS.has(def.f)) def.px = Math.max(0, zoneOf(c));
    else if (zoneOf(c) > 0 && def.f !== 'none') def.slot = String(zoneOf(c) + 1); // clé distincte (cct2…), ignorée par le rendu
    channels.push({ ...def, ofl: c });
  }
  const zones = Math.max(1, ...channels.map((ch) => (ch.px ?? -1) + 1));
  return {
    id: `m${index + 1}`,
    name: mode.name,
    ...(zones > 1 ? { pixels: zones } : {}),
    channels: channels.map(({ ofl, ...rest }) => rest),
  };
}

function convert(sel, j) {
  const ph = j.physical || {};
  const [w = 300, h = 300, d = 150] = (ph.dimensions || []).map((v) => v / 1000);
  const modes = j.modes.map((m, i) => convertMode(j, m, i));
  const pixels = Math.max(1, ...modes.map((m) => m.pixels || 1));
  const lens = ph.lens?.degreesMinMax;
  const zoomable = lens && lens[1] > lens[0] && lens[1] <= 90;
  const beam = sel.beam ?? (lens ? (zoomable ? Math.round((lens[0] + lens[1]) / 2) : lens[0]) : sel.cat === 'panel' ? 110 : 50);
  const cctCh = modes.flatMap((m) => m.channels).find((c) => c.f === 'cct');
  const hasColor = modes.some((m) => m.channels.some((c) => ['red', 'hue', 'x'].includes(c.f)));
  const lumens = ph.bulb?.lumens || Math.round((ph.power || 100) * EFFICACY[sel.cat]);
  const luxAt = (deg) => Math.round((lumens * BEAM_SHARE[sel.cat]) / (2 * Math.PI * (1 - Math.cos(((deg / 2) * Math.PI) / 180))));
  const brand = BRAND_NAMES[sel.mfr] || sel.mfr;
  const geometry = sel.cat === 'panel' ? { kind: 'panel', width: w, height: h, depth: Math.max(0.02, d) } : { kind: 'par', diameter: Math.min(w, h) * 0.9, depth: d };
  return {
    id: `${sel.mfr}.${sel.key}`,
    brand,
    name: j.name,
    model: j.name,
    category: sel.cat,
    platform: 'generic',
    technology: 'led',
    geometry,
    pixels,
    ...(pixels > 1 ? { pixelLayout: { cols: pixels, rows: 1 } } : {}),
    weight: ph.weight,
    watts: ph.power,
    beamAngle: beam,
    ...(zoomable ? { zoom: { min: lens[0], max: lens[1], luxAt1m: [[lens[0], luxAt(lens[0])], [lens[1], luxAt(lens[1])]] } } : {}),
    emitters: hasColor ? 'couleur (RGB/RGBW…)' : cctCh ? `bicolore ${cctCh.range?.join('–')} K` : 'lumière du jour',
    cctRange: cctCh?.range,
    output: { lumens, luxAt1m: luxAt(beam), estimated: true },
    dmx: { modes },
    source: { kind: 'ofl', url: `https://open-fixture-library.org/${sel.mfr}/${sel.key}` },
    defaults: { settings: zoomable ? { zoom: beam } : {} },
  };
}

const fixtures = [];
for (const sel of SELECTION) {
  const r = await fetch(`${RAW}${sel.mfr}/${sel.key}.json`);
  if (!r.ok) {
    console.error(`${sel.mfr}/${sel.key} : HTTP ${r.status}`);
    continue;
  }
  const j = await r.json();
  if (!j.modes) {
    console.error(`${sel.mfr}/${sel.key} : pas de modes (${j.redirectTo || 'redirection'})`);
    continue;
  }
  const t = convert(sel, j);
  fixtures.push(t);
  console.log(`${t.id.padEnd(30)} ${String(t.dmx.modes.length).padStart(2)} modes  ${t.pixels} zone(s)  ${t.beamAngle}°  ~${t.output.lumens} lm`);
}

const header = `// FICHIER GÉNÉRÉ par tools/ofl/import.mjs — ne pas modifier à la main.
// Profils DMX : Open Fixture Library (https://open-fixture-library.org), licence MIT.
// Importé le ${new Date().toISOString().slice(0, 10)}. Sortie lumineuse estimée (output.estimated).
`;
writeFileSync(OUT, `${header}export const OFL_FIXTURES = ${JSON.stringify(fixtures, null, 1)};\n`);
console.log(`→ ${fixtures.length} projecteurs écrits dans ${OUT}`);
