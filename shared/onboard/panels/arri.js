// Panneaux ARRI (docs/recherche/menus/arri.md) :
//  - SkyPanel S30-C / S60-C / S120-C / S360-C, firmware 4.4 : écran LCD, MENU / BACK / MODE / PRESET,
//    codeur INTENSITY/SELECTOR (tourner, appuyer) et deux molettes dont la fonction s'affiche au-dessus.
//  - L-Series L5-C / L7-C / L10-C (firmware 2.x) : afficheur à segments, FUNCT / ENTER / + / −,
//    potentiomètres INTENSITY, CCT/HUE, ±GN/SAT, mémoires 1 et 2.
//  - BroadCaster 2 Plus : pas d'écran ; roues codeuses 000–999 (adresse DMX, préréglages 900–905).
import { getMode, modeFootprint } from '../../fixtures/index.js';
import { HOLD_LONG, MenuState, act, drawList, info, num, pad, pick, stored, sub } from '../panel.js';

const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const cctRange = (c) => c.type.cctRange || [2800, 10000];

/** Numéro court d'un mode DMX ARRI (« P01: CCT & RGBW 8bit » → P01). */
function modeShort(type, mode, i) {
  const m = /^P\s*0*(\d+)/i.exec(mode.name || mode.label || '');
  return `P${pad(m ? Number(m[1]) : i + 1, 2)}`;
}
const modeOptions = (c) => (c.type.dmx?.modes || []).map((m, i) => [m.id, modeShort(c.type, m, i)]);
const footprint = (c) => modeFootprint(c.type, c.s.dmxMode) || 1;
const addressItem = (label, o = {}) =>
  num(label, { min: 1, max: (c) => 513 - footprint(c), get: (c) => c.s.dmxAddress, set: (c, v) => c.setSettings({ dmxAddress: v }), fmt: (v) => pad(v), ...o });
const modeItem = (label, o = {}) => pick(label, { options: modeOptions, get: (c) => c.s.dmxMode, set: (c, v) => c.setSettings({ dmxMode: v }), ...o });

/** Courbes de gradation : libellé ARRI ↔ courbe du moteur. */
const CURVES = [
  ['square', 'Exponential'],
  ['linear', 'Linear'],
  ['invSquare', 'Logarithmic'],
  ['sCurve', 'S-Curve'],
];

/** Numéro de série fictif, stable pour un projecteur. */
function serial(c, prefix) {
  let h = 2166136261;
  for (const ch of c.f.id) h = Math.imul(h ^ ch.charCodeAt(0), 16777619) >>> 0;
  return `${prefix}${String(h % 9000000 + 1000000)}-${pad(h % 1000)}`;
}

/** Univers Art-Net (Net / Subnet / Universe) ↔ univers ARKLIGHT (port-address + 1). */
const artnet = (c) => {
  const pa = Math.max(0, (c.s.universe || 1) - 1);
  return { net: Math.floor(pa / 256), sub: Math.floor(pa / 16) % 16, uni: pa % 16 };
};
const setArtnet = (c, part, v) => {
  const a = { ...artnet(c), [part]: v };
  c.setSettings({ universe: a.net * 256 + a.sub * 16 + a.uni + 1 });
};

const LED_GREEN = '#3ddc84';
const LED_BLUE = '#3d8bff';
const LED_RED = '#ff4d4d';

// ================================================================== SkyPanel
const isS360 = (c) => c.type.id === 'arri.skypanel-s360c';
const FACTORY_PRESETS = [
  ['Preset 01', { mode: 'cct', cct: 2900, tint: 0 }, '2900K'],
  ['Preset 02', { mode: 'cct', cct: 3200, tint: 0 }, '3200K'],
  ['Preset 03', { mode: 'cct', cct: 5600, tint: 0 }, '5600K'],
  ['Preset 04', { mode: 'cct', cct: 6500, tint: 0 }, '6500K'],
  ['Preset 05', { mode: 'hsi', hue: 120, sat: 1 }, '120° 100%'],
  ['Preset 06', { mode: 'hsi', hue: 240, sat: 1 }, '240° 100%'],
  ['Preset 07', { mode: 'cct', cct: 5600, tint: 0.35 }, 'Rosco 3408'],
  ['Preset 08', { mode: 'cct', cct: 3200, tint: -0.15 }, 'Lee 187'],
  ['Preset 09', { mode: 'cct', cct: 3200, tint: 0.1 }, 'Rosco 3152'],
  ['Preset 10', { mode: 'cct', cct: 3200, tint: 0.05 }, 'Lee 162'],
];
const EFFECTS = ['Off', 'Party Effect', 'Candle', 'Clouds Passing', 'Club Lights', 'Color Chase', 'Cop Car', 'Explosion', 'Fire', 'Fireworks', 'Fluorescent Flicker', 'Light Strobe', 'Lightning', 'Paparazzi', 'Process', 'Pulsing', 'Television', 'Welding'];
const LIGHT_MODES = [
  ['cct', 'CCT'],
  ['hsi', 'H S I'],
  ['gel', 'Gel'],
  ['source', 'Source Matching'],
  ['rgbw', 'RGBW'],
  ['xy', 'x, y Coord.'],
];
const onOff = (dflt) => ({ options: ['On', 'Off'], ...stored(dflt.key, dflt.v) });
const special = (key, label, other, otherLabel) =>
  pick(label, {
    options: ['On', 'Off'],
    get: (c) => c.panel(key, 'Off'),
    set: (c, v) => {
      if (v === 'On' && other && c.panel(other, 'Off') === 'On') return `Not Available: ${otherLabel}`;
      c.setPanel({ [key]: v });
      return v === 'On' ? 'Mémorisé (non simulé dans ARKLIGHT)' : undefined;
    },
  });

function skypanelTree() {
  return [
    sub('DMX Settings', [
      addressItem('DMX Address'),
      modeItem('DMX Mode'),
      pick('DMX Loss Behavior', { options: ['Hold Last Command', 'Black Out', 'Hold 2 Min Fade Out'], ...stored('dmxLoss', 'Hold Last Command') }),
      pick('DMX Protocol Version', { options: ['Version 3.4', 'Version 4.0', 'Version 4.1', 'Version 4.2', 'Version 4.3', 'Version 4.4'], ...stored('dmxProtocol', 'Version 4.4') }),
      pick('Ext. Color Control', onOff({ key: 'ecc', v: 'Off' })),
      pick('RDM State', onOff({ key: 'rdm', v: 'On' })),
      pick('DMX Frame Filter', onOff({ key: 'frameFilter', v: 'On' })),
      sub('CRMX Settings', [pick('CRMX Data State', onOff({ key: 'crmx', v: 'Off' })), act('CRMX Connection', () => 'Liaison CRMX : avec un émetteur, depuis l’onglet Réseau (non simulé ici)')], { when: isS360 }),
    ]),
    pick('Fan Mode', { options: (c) => [...(isS360(c) ? ['Normal'] : []), 'Quiet Mode', 'Variable', 'High Temp'], ...stored('fan', (c) => (isS360(c) ? 'Normal' : 'Variable')) }),
    pick('Light Mode', {
      options: LIGHT_MODES,
      get: (c) => c.panel('lightMode', c.m.mode === 'hsi' ? 'hsi' : 'cct'),
      set: (c, v) => {
        c.setPanel({ lightMode: v });
        if (v === 'cct' || v === 'hsi') return c.setManual({ mode: v });
        return 'Mode non simulé dans ARKLIGHT : la lumière reste en CCT / HSI';
      },
    }),
    sub('Light Control', [
      pick('Dimming Curve', { options: CURVES, get: (c) => c.s.dimmingCurve, set: (c, v) => c.setSettings({ dimmingCurve: v }) }),
      sub('Special Modes', [special('lowEnd', 'Low End Mode', 'stage', 'Stage Mode'), special('stage', 'Stage Mode', 'lowEnd', 'Low End Mode'), special('tungsten', 'Tungsten Mode'), special('highSpeed', 'High Speed Mode')]),
      pick('Host / Client Mode', onOff({ key: 'host', v: 'Off' })),
      pick('RGBW Color Space', { options: ['Calibrated Color', 'Direct Color'], ...stored('rgbwSpace', 'Calibrated Color') }),
      pick('Frequency Selection', { options: ['Default', ...Array.from({ length: 10 }, (_, i) => `Frequency ${i + 1}`)], ...stored('frequency', 'Default') }),
    ]),
    pick('Lighting Effects', {
      options: EFFECTS,
      get: (c) => c.panel('effect', 'Off'),
      set: (c, v) => {
        if (v !== 'Off' && c.panel('highSpeed', 'Off') === 'On') return 'Not Possible: High Speed Active';
        c.setPanel({ effect: v });
        return v === 'Off' ? undefined : 'Effet mémorisé (non simulé dans ARKLIGHT)';
      },
    }),
    sub('Display Setup', [
      pick('Display Illumination', { options: ['Always On', 'Off After 10 Sec.'], ...stored('illum', 'Always On') }),
      num('Display Brightness', { min: 0, max: 10, ...stored('brightness', 10) }),
      num('Display Contrast', { min: 1, max: 10, fmt: (v) => pad(v, 2), ...stored('contrast', 3) }),
      pick('Display Orientation', { options: ['Normal', 'Upside Down'], ...stored('orientation', 'Normal') }),
      pick('Display Error Mode', { options: ['Normal', 'Hidden'], ...stored('errorMode', 'Hidden') }),
    ]),
    sub('USB Functions', [
      act('Light Preset', () => 'USB Drive not Mountable : pas de clé USB'),
      act('DMX Preset', () => 'USB Drive not Mountable : pas de clé USB'),
      act('Fixture Setting', () => 'USB Drive not Mountable : pas de clé USB'),
      act('Save Error Log', () => 'USB Drive not Mountable : pas de clé USB'),
    ]),
    sub('Art-Net and sACN', [
      pick('Art-Net / sACN State', { options: ['Automatic', 'Art-Net only', 'sACN only', 'Off'], ...stored('netState', 'Automatic') }),
      num('Art-Net Net', { min: 0, max: 127, get: (c) => artnet(c).net, set: (c, v) => setArtnet(c, 'net', v) }),
      num('Art-Net Subnet', { min: 0, max: 15, get: (c) => artnet(c).sub, set: (c, v) => setArtnet(c, 'sub', v) }),
      num('Art-Net Universe', { min: 0, max: 15, get: (c) => artnet(c).uni, set: (c, v) => setArtnet(c, 'uni', v) }),
      pick('Merge Mode', { options: ['Latest Takes Precedence', 'Highest Takes Precedence'], ...stored('merge', 'Latest Takes Precedence') }),
      pick('Gateway', onOff({ key: 'gateway', v: 'On' })),
      num('sACN Universe', { min: 1, max: 63999, get: (c) => c.s.universe || 1, set: (c, v) => c.setSettings({ universe: v }) }),
    ]),
    sub('Network Settings', [
      info('Link', 'No Connection'),
      info('IP', '2.0.0.1'),
      pick('Mode', { options: ['DHCP', 'Art-Net 2.B.C.D', 'Art-Net 10.B.C.D', 'Manual'], ...stored('ipMode', 'DHCP') }),
      info('Mask', '255.255.255.0'),
      info('MAC', (c) => `00:1E:9D:${serial(c, '').slice(1, 3)}:${serial(c, '').slice(3, 5)}:${serial(c, '').slice(5, 7)}`),
      pick('BONJ', onOff({ key: 'bonjour', v: 'On' })),
    ]),
    sub('Fixture Info', [
      info('Fixture Status', 'System Ready'),
      info('Light Engine Temp.', (c) => `${(30 + (c.m.intensity ?? 0.6) * 18).toFixed(1)} °C`),
      info('Hour Counter', '512h - LE'),
      info('Battery Status', '— V', { when: (c) => !isS360(c) }),
      info('Fixture Serial No.', (c) => serial(c, 'L1.')),
      info('Firmware Versions', 'FW: 4.4.0.6'),
    ]),
    sub('Fixture Settings', [
      num('Low Battery Warning', { min: 22, max: 30, step: 0.5, fmt: (v) => `${v.toFixed(1)} V`, ...stored('lowBatt', 23), when: (c) => !isS360(c) }),
      pick('USB Mode', { options: ['Normal', 'Service'], ...stored('usbMode', 'Normal') }),
      sub('LE Compensation', [info('Light Engine Status', 'Gen 2 or above installed'), pick('LE Compensation State', onOff({ key: 'leComp', v: 'Off' }))]),
    ]),
    pick('Factory Reset', {
      options: ['No', 'Yes'],
      get: () => 'No',
      set: (c, v) => {
        if (v !== 'Yes') return;
        c.setSettings({ panel: {}, dmxAddress: 1, dmxMode: c.type.dmx?.modes?.[0]?.id, dimmingCurve: 'square' });
        c.setManual({ mode: 'cct', intensity: 1, cct: 5600, tint: 0 });
        return 'Factory Reset : réglages d’usine';
      },
    }),
  ];
}

function skypanelHome(c) {
  const W = 128;
  const m = c.m;
  const prims = [];
  const hsi = m.mode === 'hsi';
  const live = c.dmxLive();
  // coin haut gauche : verrou, effet, mode spécial
  const special = c.st.locked ? 'LOCKED' : c.panel('effect', 'Off') !== 'Off' ? c.panel('effect') : c.panel('lowEnd') === 'On' ? 'Low End Mode' : c.panel('tungsten') === 'On' ? 'Tungsten' : '';
  if (special) prims.push({ t: 'text', x: 2, y: 1, text: special, s: 9, w: 76 });
  prims.push({ t: 'text', x: W - 2, y: 1, text: live ? 'DMX' : c.panel('ecc') === 'On' ? (hsi ? '★HSI' : '★CCT') : hsi ? 'HSI' : 'CCT', s: 10, align: 'r', bold: true });
  if (live) {
    // page « DMX Live » : mode, adresse, nombre de canaux
    const mode = getMode(c.type, c.s.dmxMode);
    const i = (c.type.dmx?.modes || []).indexOf(mode);
    prims.push({ t: 'text', x: W / 2, y: 15, text: mode ? modeShort(c.type, mode, i) : '—', s: 18, align: 'c', bold: true });
    prims.push({ t: 'line', x1: 2, y1: 40, x2: W - 2, y2: 40, w: 1 });
    prims.push({ t: 'text', x: 32, y: 46, text: `${pad(c.s.dmxAddress)}-${pad(c.s.dmxAddress + footprint(c) - 1)}`, s: 10, align: 'c' });
    prims.push({ t: 'line', x1: 64, y1: 42, x2: 64, y2: 62, w: 1 });
    prims.push({ t: 'text', x: 96, y: 46, text: `${footprint(c)} Ch`, s: 10, align: 'c' });
    return prims;
  }
  const left = hsi ? `${Math.round(m.hue || 0)}°` : `${Math.round(m.cct || 5600)}K`;
  const tint = Number(m.tint || 0);
  const right = hsi ? `${Math.round((m.sat ?? 1) * 100)}% SAT` : `${tint >= 0 ? '+' : ''}${tint.toFixed(2)} GN`;
  const big = c.flash?.text || `${((m.intensity ?? 0) * 100).toFixed(1)}%`;
  prims.push({ t: 'text', x: W / 2, y: 14, text: big, s: 20, align: 'c', bold: true });
  // trait : une courte barre épaisse à gauche figure l'intensité
  prims.push({ t: 'line', x1: 2, y1: 40, x2: W - 2, y2: 40, w: 1 });
  prims.push({ t: 'rect', x: 2, y: 39, w: Math.max(2, (W - 4) * (m.intensity ?? 0)), h: 2.5, fill: 'fg' });
  if (c.flash?.cell !== 'L') prims.push({ t: 'text', x: 32, y: 46, text: left, s: 11, align: 'c' });
  prims.push({ t: 'line', x1: 64, y1: 43, x2: 64, y2: 62, w: 1 });
  if (c.flash?.cell !== 'R') prims.push({ t: 'text', x: 96, y: 46, text: right, s: 11, align: 'c' });
  return prims;
}

function presetsScreen(c) {
  const save = c.screen === 'save';
  const rows = save ? userSlots(c).map((p, i) => `${i + 1}  ${p ? p.label : 'Empty'}`) : [...FACTORY_PRESETS.map(([n, , d]) => `${n.replace('Preset ', '')} ${d}`), ...userSlots(c).map((p, i) => `U${i + 1} ${p ? p.label : 'Empty'}`)];
  const fake = { level: { title: save ? 'Save Preset' : 'Presets', cursor: c.st.cursor }, items: () => rows.map((label) => ({ label })) };
  return drawList(c, fake, { values: false });
}

const userSlots = (c) => {
  const u = c.panel('presets', []);
  return Array.from({ length: 10 }, (_, i) => u[i] || null);
};

function applyPreset(c, i) {
  if (i < FACTORY_PRESETS.length) {
    const [name, look] = FACTORY_PRESETS[i];
    c.setManual({ ...look, intensity: c.m.intensity ?? 1 });
    c.setPanel({ lightMode: look.mode });
    return c.say(`${name} activé`);
  }
  const p = userSlots(c)[i - FACTORY_PRESETS.length];
  if (!p) return c.say('Empty');
  c.setManual(p.look);
  c.say(`Preset U${i - FACTORY_PRESETS.length + 1} activé`);
}

export const skypanel = {
  id: 'arri-skypanel',
  display: { w: 128, h: 64, palette: { bg: '#a9d9ee', fg: '#0d2533', dim: '#6c93a4', accent: '#0d2533', off: '#1d2a31' } },
  face: {
    style: 'arri',
    rows: [
      [
        { col: [{ el: 'led', id: 'power', label: 'POWER' }, { el: 'led', id: 'data', label: 'DATA' }, { el: 'led', id: 'status', label: 'STATUS' }] },
        { col: [{ el: 'key', id: 'menu', label: 'MENU' }, { el: 'key', id: 'back', label: 'BACK' }] },
        { el: 'display' },
        { col: [{ el: 'key', id: 'mode', label: 'MODE' }, { el: 'key', id: 'preset', label: 'PRESET' }] },
      ],
      [
        { el: 'knob', id: 'is', label: 'INTENSITY/SELECTOR', cap: 'blue', push: true, size: 'l' },
        { el: 'knob', id: 'center', label: '', pointer: true },
        { el: 'knob', id: 'right', label: '', pointer: true },
      ],
    ],
    note: 'Molettes : tourner avec la molette de la souris ou en glissant ; cliquer le codeur bleu pour valider. Le codeur maintenu 5 s verrouille le panneau.',
  },
  init: () => ({ cursor: 0, locked: false }),
  holds: {
    is: [
      [
        5,
        (c) => {
          if (c.screen !== 'home') return;
          c.st.locked = !c.st.locked;
          c.say(c.st.locked ? 'LOCKED' : 'Déverrouillé');
        },
      ],
    ],
    preset: [
      [
        HOLD_LONG,
        (c) => {
          if (c.st.locked) return;
          c.screen = 'save';
          c.st.cursor = 0;
        },
      ],
    ],
  },
  key(c, id, { long }) {
    if (c.st.locked) return c.say('LOCKED');
    if (id === 'menu') return c.screen === 'menu' ? c.closeMenu() : c.openMenu(skypanelTree(), 'Menu');
    if (id === 'back') {
      if (c.screen === 'menu') return c.menu.back() || c.closeMenu();
      c.screen = 'home';
      return;
    }
    if (id === 'mode') {
      if (c.screen !== 'home') return;
      if (long) return c.say('Extended Color Control : non simulé');
      const next = c.m.mode === 'hsi' ? 'cct' : 'hsi';
      c.setManual({ mode: next });
      c.setPanel({ lightMode: next });
      return;
    }
    if (id === 'preset') {
      c.screen = c.screen === 'presets' ? 'home' : 'presets';
      c.st.cursor = 0;
      return;
    }
    if (id === 'is') {
      if (c.screen === 'menu') {
        const r = c.menu.enter(c);
        if (r?.msg) c.say(r.msg);
      } else if (c.screen === 'presets') {
        applyPreset(c, c.st.cursor);
        c.screen = 'home';
      } else if (c.screen === 'save') {
        const u = userSlots(c);
        const look = { mode: c.m.mode, intensity: c.m.intensity, cct: c.m.cct, tint: c.m.tint, hue: c.m.hue, sat: c.m.sat };
        u[c.st.cursor] = { look, label: c.m.mode === 'hsi' ? `${Math.round(c.m.hue || 0)}°` : `${Math.round(c.m.cct || 5600)}K` };
        c.setPanel({ presets: u });
        c.say(`Preset U${c.st.cursor + 1} enregistré`);
        c.screen = 'home';
      }
    }
  },
  turn(c, id, steps, mult) {
    if (c.st.locked) return c.say('LOCKED');
    if (c.screen === 'menu') {
      if (id === 'is') c.menu.move(c, steps, mult);
      return;
    }
    if (c.screen === 'presets' || c.screen === 'save') {
      if (id === 'is') c.st.cursor = clamp(c.st.cursor + steps, 0, c.screen === 'save' ? 9 : 19);
      return;
    }
    const m = c.m;
    if (id === 'is') return c.setManual({ intensity: clamp(Math.round(((m.intensity ?? 0) + steps * 0.001 * mult) * 1000) / 1000, 0, 1) });
    const hsi = m.mode === 'hsi';
    if (id === 'center') {
      if (hsi) {
        const hue = ((Math.round(m.hue || 0) + steps * mult) % 360 + 360) % 360;
        c.setManual({ hue });
        c.flash = { text: `${hue}°`, until: c.t + 1.5, cell: 'L' };
      } else {
        const [lo, hi] = cctRange(c);
        const cct = clamp(Math.round(((m.cct || 5600) + steps * 10 * mult) / 10) * 10, lo, hi);
        c.setManual({ cct });
        c.flash = { text: `${cct}K`, until: c.t + 1.5, cell: 'L' };
      }
    } else if (id === 'right') {
      if (hsi) {
        const sat = clamp(Math.round((m.sat ?? 1) * 100 + steps * mult), 0, 100) / 100;
        c.setManual({ sat });
        c.flash = { text: `${Math.round(sat * 100)}% SAT`, until: c.t + 1.5, cell: 'R' };
      } else {
        const tint = clamp(Math.round(((m.tint || 0) + steps * 0.01 * mult) * 100) / 100, -1, 1);
        c.setManual({ tint });
        c.flash = { text: `${tint >= 0 ? '+' : ''}${tint.toFixed(2)} GN`, until: c.t + 1.5, cell: 'R' };
      }
    }
  },
  draw(c) {
    if (c.screen === 'menu') return { prims: drawList(c, c.menu, { rows: 3 }) };
    if (c.screen === 'presets' || c.screen === 'save') return { prims: presetsScreen(c) };
    return { prims: skypanelHome(c), dim: c.panel('illum') === 'Off After 10 Sec.' && c.t - (c.lastTouch || 0) > 10 };
  },
  leds(c, t) {
    return {
      power: c.on ? { color: LED_GREEN } : null,
      data: c.on && c.dmxLive() ? { color: LED_BLUE } : null,
      status: c.on ? (c.st.locked ? { color: LED_RED, blinkHz: 2 } : { color: LED_GREEN }) : null,
    };
  },
};

// ================================================================== L-Series (L5-C, L7-C, L10-C)
const LEVELS = ['INTENSITY', 'CCT', 'GRN', 'STATUS', 'SETUP'];

function lseriesTree(c) {
  const modes = (c.type.dmx?.modes || []).map((m, i) => [m.id, `P.${pad(i + 1)}`]);
  return [
    sub('STATUS', [
      info('OK', ''),
      info('SW', '2.5'),
      info('DMXV', '3.1'),
      info('HOUR', '501t'),
      info('TEMP', (c2) => `${(30 + (c2.m.intensity ?? 0.6) * 14).toFixed(1)}°`),
      info('VBAT', '—', { when: (c2) => c2.type.id === 'arri.l5-c' }),
    ]),
    sub('SETUP', [
      sub('DMX', [
        pick('PROT', { options: modes, get: (c2) => c2.s.dmxMode, set: (c2, v) => (c2.setSettings({ dmxMode: v }), c2.blink()) }),
        pick('LSIG', { options: ['HOLD', 'BOUT', 'H2MF'], get: (c2) => c2.panel('dmxLoss', 'HOLD'), set: (c2, v) => (c2.setPanel({ dmxLoss: v }), c2.blink()) }),
        num('CHAN', { min: 1, max: (c2) => 513 - footprint(c2), get: (c2) => c2.s.dmxAddress, set: (c2, v) => (c2.setSettings({ dmxAddress: v }), c2.blink()) }),
      ]),
      pick('FACT', {
        options: ['NO', 'YES'],
        get: () => 'NO',
        set: (c2, v) => {
          if (v !== 'YES') return;
          c2.setSettings({ panel: {}, dmxAddress: 1, dmxMode: c2.type.dmx?.modes?.[0]?.id });
          c2.setManual({ mode: 'cct', intensity: 0, cct: 5600, tint: 0 });
          return 'Réglages d’usine';
        },
      }),
      pick('DEMO', { options: ['ON', 'OFF'], ...stored('demo', 'OFF') }),
      pick('INDC', { options: ['ON', 'OFF'], ...stored('indc', 'ON') }),
      pick('FAN', { options: ['LOW', 'VARI', 'HI45'], ...stored('fan', 'VARI') }),
    ]),
  ];
}

function lseriesText(c) {
  const m = c.m;
  const hsi = m.mode === 'hsi';
  if (c.menu) {
    const menu = c.menu;
    if (menu.edit) {
      const it = menu.edit.item;
      const v = menu.edit.value;
      return { text: it.kind === 'number' ? String(v) : it.options && typeof it.options === 'function' ? it.options(c).find((o) => o[0] === v)?.[1] || '' : String(v), legend: c.st.legend, arrow: false };
    }
    const it = menu.current(c);
    const val = it?.kind === 'info' ? (typeof it.value === 'function' ? it.value(c) : it.value) : '';
    return { text: it ? (it.kind === 'info' && val ? `${val}` : it.label) : '', legend: c.st.legend, arrow: !!(it && (it.children || it.kind === 'enum' || it.kind === 'number')), label: it?.kind === 'info' ? it.label : '' };
  }
  const lv = LEVELS[c.st.level];
  if (lv === 'INTENSITY') return { text: `${Math.round((m.intensity ?? 0) * 100)}`, unit: '%', legend: 'INTENSITY' };
  if (lv === 'CCT') return hsi ? { text: `${Math.round(m.hue || 0)}°`, legend: 'CCT' } : { text: `${Math.round(m.cct || 5600)}`, unit: 'K', legend: 'CCT' };
  if (lv === 'GRN') return hsi ? { text: `${Math.round((m.sat ?? 1) * 100)}`, legend: 'GRN' } : { text: `${(m.tint || 0) >= 0 ? '+' : '-'}${Math.abs(m.tint || 0).toFixed(2)}`, legend: 'GRN' };
  if (lv === 'STATUS') return { text: 'OK', legend: 'STATUS', arrow: true };
  return { text: '', legend: 'SETUP', arrow: true };
}

export const lseries = {
  id: 'arri-lseries',
  display: { w: 128, h: 48, palette: { bg: '#a9d9ee', fg: '#0d2533', dim: '#8fb8c9', accent: '#0d2533', off: '#1d2a31' } },
  face: {
    style: 'arri',
    rows: [
      [
        { col: [{ el: 'led', id: 'status', label: 'POWER/STATUS' }, { el: 'led', id: 'data', label: 'DATA' }] },
        { col: [{ el: 'key', id: 'funct', label: 'FUNCT' }, { el: 'key', id: 'enter', label: 'ENTER' }] },
        { el: 'display' },
        { col: [{ el: 'key', id: 'plus', label: '+ ▲' }, { el: 'key', id: 'minus', label: '− ▼' }] },
      ],
      [
        { el: 'knob', id: 'intensity', label: 'INTENSITY', cap: 'blue', size: 'l' },
        { el: 'key', id: 'mem1', label: '1', shape: 'small' },
        { el: 'knob', id: 'cct', label: 'CCT / HUE', pointer: true },
        { el: 'key', id: 'mem2', label: '2', shape: 'small' },
        { el: 'knob', id: 'gn', label: '±GN / SAT', pointer: true },
      ],
    ],
    note: 'Les molettes sont des potentiomètres (butées). ENTER puis 1 ou 2 : mémoires 3200 K / 5600 K. ENTER maintenu : JUMP (+ adresse, − mode DMX).',
  },
  init(c) {
    // confirmation d'un réglage : la valeur clignote trois fois
    // confirmation d'un réglage : la valeur clignote (rien à renvoyer, sinon l'heure de fin s'afficherait en message)
    c.blink = () => {
      c.st.blinkUntil = c.t + 1.2;
    };
    return { level: 1, back: 0, shift: 0, jump: 0, legend: 'STATUS', blinkUntil: 0 };
  },
  holds: {
    enter: [
      [
        1.5,
        (c) => {
          c.st.jump = c.t + 3;
          c.say('JUMP : + adresse DMX, − mode DMX, FUNCT démo');
        },
      ],
    ],
  },
  key(c, id) {
    const st = c.st;
    if (st.jump > c.t && !c.menu && ['plus', 'minus', 'funct'].includes(id)) {
      // JUMP : + = adresse DMX (CHAN), − = mode DMX (PROT), FUNCT = démo
      st.jump = 0;
      if (id === 'funct') return c.setPanel({ demo: c.panel('demo', 'OFF') === 'ON' ? 'OFF' : 'ON' });
      c.menu = new MenuState(lseriesTree(c)[1].children[0].children, 'DMX');
      c.menu.level.cursor = id === 'plus' ? 2 : 0;
      st.legend = 'SETUP';
      c.menu.enter(c);
      return;
    }
    if (c.menu) {
      if (id === 'plus' || id === 'minus') return c.menu.move(c, id === 'plus' ? -1 : 1);
      if (id === 'enter') {
        const r = c.menu.enter(c);
        if (r?.msg) c.say(r.msg);
        return;
      }
      if (id === 'funct') {
        if (!c.menu.back()) c.menu = null;
        return;
      }
    }
    if (id === 'plus' || id === 'minus') {
      st.level = (st.level + (id === 'plus' ? -1 : 1) + LEVELS.length) % LEVELS.length;
      st.back = c.t + 2;
      return;
    }
    if (id === 'funct') return c.setManual({ mode: c.m.mode === 'hsi' ? 'cct' : 'hsi' });
    if (id === 'enter') {
      const lv = LEVELS[st.level];
      if (lv === 'STATUS' || lv === 'SETUP') {
        const tree = lseriesTree(c);
        c.menu = new MenuState(tree[lv === 'STATUS' ? 0 : 1].children, lv);
        st.legend = lv;
        return;
      }
      st.shift = c.t + 3; // flèche clignotante : touches mémoire armées
      return;
    }
    if (id === 'mem1' || id === 'mem2') {
      if (st.shift < c.t) return c.say('Appuie d’abord sur ENTER (flèche clignotante)');
      st.shift = 0;
      c.setManual({ mode: 'cct', cct: id === 'mem1' ? 3200 : 5600, tint: 0 });
      st.level = 1;
      c.say(id === 'mem1' ? 'Mémoire 1 : 3200 K' : 'Mémoire 2 : 5600 K');
    }
  },
  turn(c, id, steps, mult) {
    const m = c.m;
    const st = c.st;
    const hsi = m.mode === 'hsi';
    if (id === 'intensity') {
      c.setManual({ intensity: clamp(Math.round((m.intensity ?? 0) * 100 + steps * mult) / 100, 0, 1) });
      st.level = 0;
    } else if (id === 'cct') {
      if (hsi) c.setManual({ hue: clamp(Math.round((m.hue || 0) + steps * 2 * mult), 0, 360) });
      else {
        const [lo, hi] = cctRange(c);
        c.setManual({ cct: clamp(Math.round(((m.cct || 5600) + steps * 50 * mult) / 50) * 50, lo, hi) });
      }
      st.level = 1;
    } else if (id === 'gn') {
      if (hsi) c.setManual({ sat: clamp(Math.round((m.sat ?? 1) * 100 + steps * mult), 0, 100) / 100 });
      else c.setManual({ tint: clamp(Math.round(((m.tint || 0) + steps * 0.02 * mult) * 100) / 100, -1, 1) });
      st.level = 2;
    }
    st.back = c.t + 2;
  },
  tick(c) {
    // retour à la valeur du mode après 2 s (sauf STATUS et SETUP)
    if (!c.menu && c.st.back && c.t > c.st.back && c.st.level !== 3 && c.st.level !== 4) {
      c.st.level = 1;
      c.st.back = 0;
    }
  },
  draw(c) {
    const d = lseriesText(c);
    const prims = [];
    const hsi = c.m.mode === 'hsi';
    // légendes imprimées à gauche ; celle du paramètre affiché est allumée
    const legends = [
      ['INTENSITY', 'INTENSITY'],
      ['CCT', hsi ? 'CCT  HUE' : 'CCT  HUE'],
      ['GRN', 'GRN  SAT'],
      ['STATUS', 'STATUS'],
      ['SETUP', 'SETUP'],
    ];
    legends.forEach(([k, txt], i) => prims.push({ t: 'text', x: 2, y: 2 + i * 9, text: txt, s: 7, color: (c.menu ? c.st.legend : d.legend) === k ? 'fg' : 'dim', bold: true }));
    const blinkOff = c.st.blinkUntil > c.t && Math.floor(c.t * 5) % 2 === 0;
    if (!blinkOff) prims.push({ t: 'text', x: 108, y: 12, text: String(d.text).slice(0, 5), s: 22, align: 'r', bold: true, font: 'seg' });
    if (d.unit) prims.push({ t: 'text', x: 124, y: 16, text: d.unit, s: 11, align: 'r', bold: true });
    if (d.label) prims.push({ t: 'text', x: 108, y: 38, text: d.label, s: 7, align: 'r', color: 'dim' });
    const arrowOn = d.arrow || (c.st.shift > c.t && Math.floor(c.t * 3) % 2 === 0);
    if (arrowOn) prims.push({ t: 'text', x: 54, y: 34, text: '➡', s: 10 });
    if (c.st.jump > c.t) prims.push({ t: 'text', x: 108, y: 12, text: 'JUMP', s: 22, align: 'r', bold: true, font: 'seg' });
    return { prims, dim: c.panel('indc', 'ON') === 'OFF' };
  },
  leds(c) {
    return {
      status: c.on && c.panel('indc', 'ON') === 'ON' ? { color: LED_GREEN } : null,
      data: c.on && c.dmxLive() && c.panel('indc', 'ON') === 'ON' ? { color: LED_BLUE } : null,
    };
  },
};

// ================================================================== BroadCaster 2 Plus
const BACKUP = { 900: 2800, 901: 3200, 902: 4000, 903: 4800, 904: 5600, 905: 6500 };

function bcdApply(c, v) {
  c.setPanel({ bcd: v });
  if (v >= 1 && v <= 512) {
    c.setSettings({ dmxAddress: Math.min(v, 513 - footprint(c)) });
    c.say(`Adresse DMX ${pad(v)}`);
  } else if (BACKUP[v]) {
    c.setManual({ mode: 'cct', cct: BACKUP[v], intensity: 1, tint: 0 });
    c.say(`Mode secours ${v} : ${BACKUP[v]} K`);
  } else c.say(`${pad(v)} : sans fonction documentée`);
}

export const broadcaster = {
  id: 'arri-broadcaster',
  display: { w: 66, h: 26, palette: { bg: '#0b0c0e', fg: '#f4f4f0', dim: '#5d6066', accent: '#f4f4f0', off: '#0b0c0e' } },
  displayWhenOff: true,
  face: {
    style: 'arri',
    rows: [
      [{ el: 'led', id: 'red', label: '' }, { el: 'led', id: 'green', label: '' }, { el: 'led', id: 'blue', label: '' }],
      [{ col: [{ el: 'key', id: 'd2+', label: '+', shape: 'small' }, { el: 'key', id: 'd1+', label: '+', shape: 'small' }, { el: 'key', id: 'd0+', label: '+', shape: 'small' }], row: true }],
      [{ el: 'display' }],
      [{ col: [{ el: 'key', id: 'd2-', label: '−', shape: 'small' }, { el: 'key', id: 'd1-', label: '−', shape: 'small' }, { el: 'key', id: 'd0-', label: '−', shape: 'small' }], row: true }],
    ],
    note: 'Pas d’écran ni de menu : la roue codeuse donne l’adresse DMX (001–512) ou un mode secours (900–905 : 2800 à 6500 K).',
  },
  key(c, id) {
    const m = /^d(\d)([+-])$/.exec(id);
    if (!m) return;
    const digit = Number(m[1]);
    const cur = c.panel('bcd', c.s.dmxAddress || 1);
    const digits = [cur % 10, Math.floor(cur / 10) % 10, Math.floor(cur / 100) % 10];
    digits[digit] = (digits[digit] + (m[2] === '+' ? 1 : 9)) % 10;
    bcdApply(c, digits[0] + digits[1] * 10 + digits[2] * 100);
  },
  draw(c) {
    const v = c.panel('bcd', c.s.dmxAddress || 1);
    return { prims: [{ t: 'text', x: 33, y: 3, text: pad(v), s: 20, align: 'c', bold: true, font: 'mono', spacing: 6 }] };
  },
  leds(c) {
    return { green: c.on ? { color: LED_GREEN } : null, blue: c.on && c.dmxLive() ? { color: LED_BLUE } : null, red: null };
  },
};
