// DMG Lumière (Rosco) SL1 MIX, MINI MIX, MAXI MIX : le contrôleur MIX (firmware 2.1.x)
// (docs/recherche/menus/dedo_dmg_kino_litegear.md). Écran couleur 400×240 (fond noir, champ choisi
// en rouge), touches POWER, BACK (lock), GEL, WHITE, COLOR, SETTINGS, NAVIGATION, LIBRARY, et la
// grosse molette rouge (tourner = valeur, appuyer = préréglages rapides du champ).
import { act, drawEdit, drawList, info, num, pad, pick, stored, sub } from '../panel.js';
import { cctRange, clamp, footprint, serialOf } from './common.js';

const RED = '#e82028';
const GELS = [
  ['R2002', 'Storaro Orange', 30],
  ['R02', 'Bastard Amber', 35],
  ['E201', 'Full CT Blue', 210],
  ['R80', 'Primary Blue', 228],
  ['R26', 'Light Red', 355],
  ['E128', 'Bright Pink', 320],
  ['E121', 'Leaf Green', 110],
  ['R2005', 'Storaro Cyan', 188],
];
const EFFECTS = ['Emergency Vehicle (B)', 'Emergency Vehicle (B+R)', 'Emergency Vehicle (B+W)', 'Emergency Vehicle (R+W)', 'Emergency Vehicle (R)', 'Firelight', 'TV', 'Paparazzi'];
const SOURCES = ['Green Screen', 'Blue Screen', 'Xenon Headlamp', 'Candle Flame', 'Frosty Moonlight', 'HP Sodium', 'LP Sodium', 'Tungsten Domestic', 'Mercury Vapor', 'Sodium Vapor'];
const isMaxi = (c) => c.type.id === 'dmg-lumiere.maxi-mix';

/** Champs de chaque mode, dans l'ordre de la touche NAVIGATION (coins de l'écran). */
const FIELDS = { WHITE: ['INT', 'CCT', 'G/M'], COLOR: ['INT', 'CCT', 'HUE', 'SAT'], GEL: ['INT', 'CCT', 'HUE', 'SAT'] };
const QUICK = { INT: [0.25, 0.5, 0.75, 1], CCT: [1700, 3200, 4300, 5600, 10000], HUE: [0, 60, 120, 240, 300], SAT: [0, 0.5, 1], 'G/M': [0] };

function settingsTree() {
  return [
    pick('Bluetooth', { options: ['BT', 'OFF'], ...stored('bt', 'BT'), fmt: (v) => v }),
    act('DMX Mode', (c) => ((c.screen = 'dmx'), (c.menu = null), (c.st.dmxField = 1), undefined)),
    pick('DMX profile revision', { options: ['2.0', '2.1'], ...stored('dmxRev', '2.1') }),
    sub('Art-Net / Wi-Fi / RDM', [
      pick('RDM', { options: ['ON', 'OFF'], ...stored('rdm', 'ON') }),
      pick('Wi-Fi', { options: ['2.4 GHz', '5.8 GHz'], ...stored('wifi', '5.8 GHz') }),
      num('Art-Net Universe', { min: 0, max: 15, get: (c) => (c.s.universe - 1) % 16, set: (c, v) => c.setSettings({ universe: Math.floor((c.s.universe - 1) / 16) * 16 + v + 1 }) }),
      num('Art-Net Sub-Net', { min: 0, max: 15, get: (c) => Math.floor((c.s.universe - 1) / 16) % 16, set: (c, v) => c.setSettings({ universe: v * 16 + ((c.s.universe - 1) % 16) + 1 }) }),
      info('IP', '2 . 0 . 0 . 1'),
      info('Mask', '255 . 0 . 0 . 0'),
    ]),
    pick('Select Zone', { options: ['Zone 1+2', 'Zone 1', 'Zone 2'], ...stored('zone', 'Zone 1+2'), when: isMaxi }),
    sub('Information', [
      info('Serial #', (c) => `MIX-1809-${serialOf(c, 4, '0123456789')}`),
      info('Firmware Version', 'V2.13'),
      info('Light Hours', '6.6'),
      info('Light Version', (c) => `1${isMaxi(c) ? 3 : c.type.id.includes('mini') ? 1 : 2}`),
      info('Light Temperature', (c) => `${(35 + (c.m.intensity ?? 0.5) * 20).toFixed(1)}°C`),
      info('Error(s)', 'None'),
    ]),
    sub('Display settings', [
      pick('Display auto OFF on lock', { options: ['YES', 'NO'], ...stored('autoOff', 'YES') }),
      pick('Display rotation', { options: ['NO', 'YES'], ...stored('rotation', 'NO') }),
      pick('Screen brightness', { options: ['LOW', 'MED', 'HIGH'], ...stored('brightness', 'HIGH') }),
    ]),
    pick('Dimming Curve', {
      options: [
        ['linear', 'Linear'],
        ['square', 'Square Law'],
        ['sCurve', 'S Curve'],
        ['tungsten', 'Tungsten Emulate'],
      ],
      get: (c) => (c.panel('tungstenCurve') ? 'tungsten' : c.s.dimmingCurve),
      set: (c, v) => {
        c.setPanel({ tungstenCurve: v === 'tungsten' });
        c.setSettings({ dimmingCurve: v === 'tungsten' ? 'square' : v });
        return v === 'tungsten' ? 'Tungsten Emulate : la teinte n’est pas simulée (courbe seulement)' : undefined;
      },
    }),
    pick('Gel Sorting', { options: ['Name', 'Color', 'Number'], ...stored('gelSort', 'Number') }),
    pick('Factory Settings', {
      options: ['Keep myMIX Library', 'Erase myMIX Library'],
      get: () => '',
      set: (c, v) => {
        const keep = v === 'Keep myMIX Library' ? { mymix: c.panel('mymix', []) } : {};
        c.setSettings({ panel: keep, dmxAddress: 1, dimmingCurve: 'linear', dmxMode: c.type.dmx?.modes?.[0]?.id });
        c.setManual({ mode: 'cct', intensity: 0.5, cct: 5600, tint: 0 });
        return 'Restore Factory Settings';
      },
    }),
  ];
}

const mode = (c) => c.panel('mode', c.m.mode === 'hsi' ? 'COLOR' : 'WHITE');

function fieldValue(c, f) {
  const m = c.m;
  if (f === 'INT') return `${((m.intensity ?? 0) * 100).toFixed(1)}%`;
  if (f === 'CCT') return `${Math.round(m.cct || 5600)}°K`;
  if (f === 'G/M') {
    const g = Math.round((m.tint || 0) * 100);
    return g === 0 ? '0' : g > 0 ? `${g}G` : `${-g}M`;
  }
  if (f === 'HUE') return mode(c) === 'GEL' ? `${c.panel('gelHue', 0)}°` : `${Math.round(m.hue || 0)}°`;
  if (f === 'SAT') return mode(c) === 'GEL' ? `${c.panel('gelSat', 0)}%` : `${Math.round((m.sat ?? 1) * 100)}%`;
  return '';
}

function changeField(c, f, steps, mult) {
  const m = c.m;
  const md = mode(c);
  if (f === 'INT') return c.setManual({ intensity: clamp(Math.round(((m.intensity ?? 0) + steps * 0.001 * mult) * 1000) / 1000, 0, 1) });
  if (f === 'CCT') {
    const [lo, hi] = cctRange(c, [1700, 10000]);
    if (md === 'GEL') return c.setManual({ cct: (m.cct || 3200) >= 4400 ? (steps < 0 ? 3200 : 5600) : steps > 0 ? 5600 : 3200 });
    return c.setManual({ cct: clamp(Math.round(((m.cct || 5600) + steps * 50 * Math.min(mult, 4)) / 50) * 50, lo, hi) });
  }
  if (f === 'G/M') return c.setManual({ tint: clamp(Math.round((m.tint || 0) * 100 + steps) / 100, -1, 1) });
  if (md === 'GEL') {
    if (f === 'HUE') return c.setPanel({ gelHue: clamp(c.panel('gelHue', 0) + steps, -10, 10) });
    return c.setPanel({ gelSat: clamp(c.panel('gelSat', 0) + steps, -20, 20) });
  }
  if (f === 'HUE') return c.setManual({ hue: (((Math.round(m.hue || 0) + steps * Math.min(mult, 4)) % 360) + 360) % 360 });
  if (f === 'SAT') return c.setManual({ sat: clamp(Math.round((m.sat ?? 1) * 100 + steps) / 100, 0, 1) });
}

/** Gel choisi (approché : teinte de la gélatine sur une base 3200 / 5600 K, non calibrée). */
function applyGel(c, i) {
  const [num_, name, hue] = GELS[(i + GELS.length) % GELS.length];
  c.setPanel({ gel: (i + GELS.length) % GELS.length, mode: 'GEL' });
  c.setManual({ mode: 'hsi', hue, sat: 0.6 });
  return `${name} ${num_.replace(/^\D+/, '')}`;
}

function modeScreen(c) {
  const W = 160;
  const H = 96;
  const md = mode(c);
  const fields = FIELDS[md];
  const cur = fields[c.st.field % fields.length];
  const prims = [{ t: 'text', x: W / 2, y: 4, text: md, s: 10, align: 'c', bold: true }];
  const corner = (f, x, y, align) => {
    if (!f) return;
    const sel = f === cur;
    const w = 46;
    const bx = align === 'r' ? x - w : x;
    if (sel) prims.push({ t: 'rect', x: bx, y, w, h: 24, fill: RED });
    prims.push({ t: 'text', x: bx + w / 2, y: y + 2, text: f, s: 8, align: 'c', bold: true });
    prims.push({ t: 'text', x: bx + w / 2, y: y + 12, text: fieldValue(c, f), s: 9, align: 'c', bold: true });
  };
  corner('INT', 3, 3, 'l');
  corner('CCT', W - 3, 3, 'r');
  if (md === 'WHITE') corner('G/M', W - 3, H - 27, 'r');
  else {
    corner('HUE', 3, H - 27, 'l');
    corner('SAT', W - 3, H - 27, 'r');
  }
  // centre : grande valeur, spectre (COLOR) ou gélatine (GEL)
  if (md === 'COLOR') {
    const x0 = 30;
    const w = 100;
    const cols = ['#ff7a1a', '#ffd21a', '#ffffff', '#8cc8ff', '#2f6bff', '#8a3dff', '#c4123c'];
    cols.forEach((col, k) => prims.push({ t: 'rect', x: x0 + (k * w) / cols.length, y: 40, w: w / cols.length + 0.5, h: 8, fill: col }));
    prims.push({ t: 'rect', x: x0 + (w * (c.m.hue || 0)) / 360 - 1, y: 37, w: 2, h: 14, fill: 'fg' });
  } else if (md === 'GEL') {
    const g = GELS[c.panel('gel', 0)];
    prims.push({ t: 'rect', x: 50, y: 32, w: 60, h: 6, fill: '#f8b828' });
    prims.push({ t: 'text', x: W / 2, y: 42, text: g[1], s: 9, align: 'c', bold: true }, { t: 'text', x: W / 2, y: 54, text: g[0].replace(/^\D+/, ''), s: 8, align: 'c', color: 'dim' });
  } else prims.push({ t: 'text', x: W / 2, y: 38, text: fieldValue(c, 'INT'), s: 16, align: 'c', bold: true });
  if (c.panel('bt', 'BT') === 'BT') prims.push({ t: 'text', x: W / 2, y: H - 14, text: 'ᛒ', s: 10, align: 'c', color: '#5aa9ff' });
  if (c.st.locked) prims.push({ t: 'text', x: W / 2, y: 18, text: 'LOCKED', s: 8, align: 'c', color: RED });
  return prims;
}

function dmxPage(c) {
  const W = 160;
  const fields = ['DATA IN', 'ADDRESS', 'PROFILE', 'DATA OUT'];
  const modes = c.type.dmx?.modes || [];
  const i = Math.max(0, modes.findIndex((m) => m.id === c.s.dmxMode));
  const values = [c.panel('dataIn', 'XLR'), `${c.s.dmxAddress}-${c.s.dmxAddress + footprint(c) - 1}`, `${i + 1}:${(modes[i]?.name || '').replace(/^\d+:\s*/, '')}`, c.panel('dataOut', 'None')];
  const prims = [];
  const box = (k, x, y, w) => {
    const sel = c.st.dmxField === k;
    if (sel) prims.push({ t: 'rect', x, y, w, h: 22, fill: RED });
    else prims.push({ t: 'rect', x, y, w, h: 22, stroke: 'dim' });
    prims.push({ t: 'text', x: x + w / 2, y: y + 2, text: fields[k], s: 7, align: 'c', bold: true }, { t: 'text', x: x + w / 2, y: y + 11, text: values[k], s: 8, align: 'c', w: w - 4 });
  };
  box(0, 2, 2, 46);
  box(1, 52, 2, 46);
  box(2, 102, 2, 56);
  const m = c.m;
  prims.push({ t: 'text', x: 6, y: 32, text: `INT:${((m.intensity ?? 0) * 100).toFixed(1)}%`, s: 8 }, { t: 'text', x: 84, y: 32, text: `CCT:${Math.round(m.cct || 3200)} K`, s: 8 });
  prims.push({ t: 'text', x: 6, y: 44, text: `HUE:${Math.round(m.hue || 0)}°`, s: 8 }, { t: 'text', x: 84, y: 44, text: `SAT:${Math.round((m.sat ?? 0) * 100)}%`, s: 8 });
  box(3, 2, 70, 46);
  prims.push({ t: 'text', x: 92, y: 72, text: c.dmxLive() ? 'DMX OK' : 'NO DMX', s: 9, align: 'c', bold: true, color: c.dmxLive() ? '#3ddc84' : RED });
  prims.push({ t: 'text', x: 156, y: 72, text: `Univ: ${(c.s.universe || 1) - 1}`, s: 7, align: 'r' }, { t: 'text', x: 156, y: 82, text: 'IP: 2.0.0.1', s: 7, align: 'r', color: RED });
  return prims;
}

function libraryScreen(c) {
  const W = 160;
  const tabs = ['myMIX', 'Rosco Gels', 'Effects', 'Source Match'];
  const tab = c.st.tab || 0;
  const prims = [];
  tabs.forEach((t, k) => {
    const x = (k % 2) * 80;
    const y = Math.floor(k / 2) * 14;
    prims.push({ t: 'rect', x: x + 1, y: y + 1, w: 78, h: 12, fill: k === tab ? RED : '#1b1e24' });
    prims.push({ t: 'text', x: x + 40, y: y + 3, text: t, s: 7, align: 'c', bold: true });
  });
  const list = libraryItems(c, tab);
  const cur = clamp(c.st.lib || 0, 0, Math.max(0, list.length - 1));
  const first = clamp(cur - 2, 0, Math.max(0, list.length - 5));
  list.slice(first, first + 5).forEach((txt, k) => {
    const y = 32 + k * 12;
    if (first + k === cur) prims.push({ t: 'rect', x: 1, y, w: 3, h: 10, fill: 'fg' });
    prims.push({ t: 'text', x: 7, y: y + 1, text: txt, s: 7.5, w: W - 10 });
  });
  if (!list.length) prims.push({ t: 'text', x: W / 2, y: 50, text: '(vide)', s: 8, align: 'c', color: 'dim' });
  return prims;
}

function libraryItems(c, tab) {
  if (tab === 0) return c.panel('mymix', []).map((p, i) => `${pad(i + 1, 4)}:${i + 1}:${p.name}`);
  if (tab === 1) return GELS.map((g, i) => `${pad(i, 4)}:${i + 100}:${g[1]}`);
  if (tab === 2) return EFFECTS.map((e, i) => `${pad(i, 4)}:${801 + i}:${e}`);
  return SOURCES.map((s, i) => `${pad(i, 4)}:${701 + i}:${s}`);
}

function presetName(c) {
  const m = c.m;
  const md = mode(c);
  const int = Math.round((m.intensity ?? 0) * 1000);
  if (md === 'COLOR') return `Color.${int}.${Math.round(m.cct || 3200)}.${Math.round((m.sat ?? 1) * 100)}.${Math.round(m.hue || 0)}`;
  if (md === 'GEL') return `Gel.${int}.${Math.round(m.cct || 3200)}.${c.panel('gelHue', 0)}.${c.panel('gelSat', 0)}.${c.panel('gel', 0)}`;
  const g = Math.round((m.tint || 0) * 100);
  return `White.${int}.${Math.round(m.cct || 5600)}.${g >= 0 ? `${g}G` : `${-g}M`}`;
}

export const dmgMix = {
  id: 'dmg-mix',
  display: { w: 160, h: 96, zoom: 1.6, palette: { bg: '#000000', fg: '#ffffff', dim: '#788898', accent: RED, off: '#000000' } },
  face: {
    style: 'dmg',
    rows: [
      [
        { col: [{ el: 'key', id: 'power', label: '⏻' }, { el: 'key', id: 'back', label: '‹  lock' }] },
        { col: [{ el: 'key', id: 'gel', label: 'GEL', shape: 'small' }, { el: 'key', id: 'white', label: 'WHITE', shape: 'small' }, { el: 'key', id: 'color', label: 'COLOR', shape: 'small' }] },
        { el: 'display' },
        { col: [{ el: 'key', id: 'settings', label: 'SETTINGS', shape: 'small' }, { el: 'key', id: 'nav', label: '⟲', shape: 'small' }, { el: 'key', id: 'library', label: 'LIBRARY  save/delete', shape: 'small' }] },
        { el: 'knob', id: 'knob', label: '', cap: 'red', push: true, size: 'l' },
      ],
    ],
    note: '⟲ (NAVIGATION) : passer d’un coin à l’autre. Molette rouge : tourner = valeur, appuyer = préréglages rapides. LIBRARY maintenu = enregistrer dans myMIX. BACK maintenu = verrou.',
  },
  init: () => ({ field: 0, tab: 0, lib: 0, dmxField: 1, quick: 0, locked: false, blackout: false }),
  holds: {
    back: [[1, (c) => ((c.st.locked = !c.st.locked), c.say(c.st.locked ? 'Commandes verrouillées' : 'Commandes déverrouillées'))]],
    power: [[1, (c) => c.setState({ power: false })]],
    library: [
      [
        1,
        (c) => {
          if (c.screen === 'library') return c.say('Delete Preset ? (non simulé)');
          const list = [...c.panel('mymix', [])];
          const name = presetName(c);
          list.push({ name, look: { ...c.m }, mode: mode(c) });
          c.setPanel({ mymix: list });
          c.say(`Saved in myMIX : ${name}`);
        },
      ],
    ],
    nav: [[1, (c) => c.say('CRMX : Link / Unlink (pas d’émetteur)')]],
  },
  key(c, id) {
    if (id === 'power') {
      if (!c.on) return c.setState({ power: true });
      c.st.blackout = !c.st.blackout; // appui court allumé : noir rapide
      if (c.st.blackout) c.st.lastInt = c.m.intensity; // intensité retenue avant le noir, rendue à l'appui suivant
      c.setManual({ intensity: c.st.blackout ? 0 : c.st.lastInt ?? 0.5 });
      return;
    }
    if (!c.on) return;
    if (c.st.locked) return c.say('Verrouillé : BACK maintenu pour déverrouiller');
    if (['white', 'color', 'gel'].includes(id)) {
      const md = id.toUpperCase();
      c.setPanel({ mode: md });
      c.menu = null;
      c.screen = 'home';
      c.st.field = 0;
      if (md === 'WHITE') c.setManual({ mode: 'cct' });
      else if (md === 'COLOR') c.setManual({ mode: 'hsi' });
      else c.say(applyGel(c, c.panel('gel', 0)));
      return;
    }
    if (id === 'settings') return c.screen === 'menu' ? c.closeMenu() : c.openMenu(settingsTree(), 'Settings');
    if (id === 'library') {
      c.menu = null;
      c.screen = c.screen === 'library' ? 'home' : 'library';
      return;
    }
    if (id === 'nav') {
      if (c.screen === 'library') return ((c.st.tab = ((c.st.tab || 0) + 1) % 4), (c.st.lib = 0));
      if (c.screen === 'dmx') return (c.st.dmxField = (c.st.dmxField + 1) % 4);
      return (c.st.field = (c.st.field + 1) % FIELDS[mode(c)].length);
    }
    if (id === 'back') {
      if (c.screen === 'menu') return c.menu.back() || c.closeMenu();
      c.menu = null;
      c.screen = 'home';
      return;
    }
    if (id === 'knob') {
      if (c.screen === 'menu') {
        const r = c.menu.enter(c);
        if (r?.msg) c.say(r.msg);
        return;
      }
      if (c.screen === 'library') {
        const tab = c.st.tab || 0;
        const i = c.st.lib || 0;
        c.screen = 'home';
        if (tab === 0) {
          const p = c.panel('mymix', [])[i];
          if (p) c.setManual(p.look), c.setPanel({ mode: p.mode });
          return;
        }
        if (tab === 1) return c.say(applyGel(c, i));
        c.setPanel({ libItem: libraryItems(c, tab)[i] });
        return c.say(`${libraryItems(c, tab)[i].split(':').pop()} (non simulé dans ARKLIGHT)`);
      }
      if (c.screen === 'dmx') return;
      // préréglages rapides du champ choisi
      const f = FIELDS[mode(c)][c.st.field % FIELDS[mode(c)].length];
      const list = QUICK[f] || [];
      if (!list.length) return;
      c.st.quick = (c.st.quick + 1) % list.length;
      const v = list[c.st.quick];
      if (f === 'INT') c.setManual({ intensity: v });
      else if (f === 'CCT') c.setManual({ cct: v });
      else if (f === 'HUE') c.setManual({ hue: v });
      else if (f === 'SAT') c.setManual({ sat: v });
      else c.setManual({ tint: v });
    }
  },
  turn(c, id, steps, mult) {
    if (!c.on) return;
    if (c.st.locked) return c.say('Verrouillé');
    if (c.screen === 'menu') return c.menu.move(c, steps, mult);
    if (c.screen === 'library') return (c.st.lib = clamp((c.st.lib || 0) + steps, 0, Math.max(0, libraryItems(c, c.st.tab || 0).length - 1)));
    if (c.screen === 'dmx') {
      const k = c.st.dmxField;
      if (k === 0) return c.setPanel({ dataIn: ['XLR', 'CRMX', 'LAN', 'WiFi 5.8G', 'WiFi 2.4G'][((['XLR', 'CRMX', 'LAN', 'WiFi 5.8G', 'WiFi 2.4G'].indexOf(c.panel('dataIn', 'XLR')) + steps) % 5 + 5) % 5] });
      if (k === 1) return c.setSettings({ dmxAddress: clamp(c.s.dmxAddress + steps * (mult > 2 ? 10 : 1), 1, 513 - footprint(c)) });
      if (k === 2) {
        const modes = c.type.dmx?.modes || [];
        const i = modes.findIndex((m) => m.id === c.s.dmxMode);
        return c.setSettings({ dmxMode: modes[clamp(i + steps, 0, modes.length - 1)]?.id });
      }
      return c.setPanel({ dataOut: c.panel('dataOut', 'None') === 'None' ? 'CRMX' : 'None' });
    }
    changeField(c, FIELDS[mode(c)][c.st.field % FIELDS[mode(c)].length], steps, mult);
  },
  draw(c) {
    if (c.screen === 'menu') return { prims: c.menu.edit ? drawEdit(c, c.menu, { w: 160, h: 96, rows: 4, rowH: 18, size: 10, selFill: RED }) : drawList(c, c.menu, { w: 160, rows: 4, rowH: 18, size: 10, selFill: RED, maxValue: 10 }) };
    if (c.screen === 'library') return { prims: libraryScreen(c) };
    if (c.screen === 'dmx') return { prims: dmxPage(c) };
    return { prims: modeScreen(c), dim: c.st.locked && c.panel('autoOff', 'YES') === 'YES' };
  },
  leds: () => ({}),
};

