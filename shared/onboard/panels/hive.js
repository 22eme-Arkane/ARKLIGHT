// Panneaux Hive Lighting (docs/recherche/menus/aputure_hive.md) :
//  - Wasp 100-C / Hornet 200-C (firmware 3.0.8) : écran OLED S.H.O.T. (S saturation, H teinte,
//    D intensité, K température), molettes SELECT et ADJUST (cliquables), interrupteurs marche et
//    DMX / Local, voyants alimentation et DMX ;
//  - Super Hornet 575-C : ballast avec écran LCD, SELECT + MENU, ADJUST + BACK, voyant STATUS.
// Sur un Hive, saturation 0 = blanc à la température K ; au-dessus, couleur (teinte, saturation).
import { MenuState, act, drawEdit, drawList, info, num, pad, pick, stored, sub } from '../panel.js';
import { LED, cctRange, clamp } from './common.js';

const EFFECTS = ['Cop Car 1', 'Cop Car 2', 'Emergency 1', 'Emergency 2', 'Emergency 3', 'Club Lights', 'Papparazzi', 'Color Chase', 'Fire', 'FireWorks', 'Pulsing', 'TV', 'Lightning', 'Strobe'];
const SOURCES = ['VioletGloStk', 'Pink Glo Stk', 'YellowGloStk', 'Red Glo Stk', 'GreenGloStk', 'Blue Glo Stk', 'Red Traffic', 'YelloTraffic', 'GreenTraffic', 'AmberCaution', 'Road Flare', 'Blow Torch', 'Luminescence', 'Comp Monitor', 'Mobile Phone', 'SunBlueHour', 'Sun Overcast', 'Sun Direct', 'Gaslight', 'Candlelight', 'Antique Bulb', 'Warm Antique', 'Xmas Lights', 'Night Light', 'Heat Lamp', 'Grow Light', 'Tungsten', 'Incandescent', 'Halogen', 'HMI', 'HP Sodium', 'LP Sodium', 'MercuryVapor', 'MetalHallide', 'Ceramic', 'Carbon Arc', 'Xenon', 'Blacklight', 'CoolWhiteFlo', 'WarmWhiteFlo'];
const CORRECTIVE = ['+GRN DAY', '1/2 +GRN DAY', '1/4 +GRN DAY', '1/8 +GRN DAY', '+GRN TUN', '1/2 +GRN TUN', '1/4 +GRN TUN', '1/8 +GRN TUN', '-GRN DAY', '1/2 -GRN DAY', '1/4 -GRN DAY', '1/8 -GRN DAY', '-GRN TUN', '1/2 -GRN TUN', '1/4 -GRN TUN', '1/8 -GRN TUN', 'CTSTRAW DAY', '1/2 CTS DAY', '1/4 CTS DAY', '1/8 CTS DAY', 'CTSTRAW TUN', '1/2 CTS TUN', '1/4 CTS TUN', '1/8 CTS TUN'];
const PROFILES = ['Cinema', 'Broadcast', 'Photo'];
const NOT_SIMULATED = 'Mémorisé (non simulé dans ARKLIGHT)';

// ---------------------------------------------------------------- S.H.O.T. ↔ réglage manuel
const shot = (c) => {
  const m = c.m;
  const sat = m.mode === 'hsi' ? Math.round((m.sat ?? 1) * 100) : 0;
  return { S: sat, H: Math.round(m.hue || 0), D: Math.round((m.intensity ?? 0) * 100), K: Math.round(m.cct || 5600) };
};
function setShot(c, key, value) {
  const v = { ...shot(c), [key]: value };
  c.setManual({ mode: v.S > 0 ? 'hsi' : 'cct', sat: v.S / 100, hue: v.H, intensity: v.D / 100, cct: v.K });
}
const RANGES = { S: [0, 100], H: [0, 360], D: [0, 100], K: [1650, 8000] };

function adjustShot(c, key, steps, fine) {
  const [lo, hi] = key === 'K' ? cctRange(c, RANGES.K) : RANGES[key];
  const step = key === 'K' ? (fine ? 25 : 200) : fine ? 1 : 10;
  const cur = shot(c)[key];
  let v = cur + steps * step;
  if (key === 'H') v = ((v % 360) + 360) % 360;
  setShot(c, key, clamp(v, lo, hi));
}

// ---------------------------------------------------------------- menus
const listPage = (title, items, key, extra = {}) => pick(title, { options: items, get: (c) => c.panel(key, items[0]), set: (c, v) => (c.setPanel({ [key]: v }), extra.msg ?? NOT_SIMULATED) });

function localMenu() {
  return [
    listPage('Effects', EFFECTS, 'effect'),
    listPage('Sources', SOURCES, 'source'),
    pick('Profiles', { options: PROFILES, ...stored('profile', 'Photo') }),
    listPage('Corrective', CORRECTIVE, 'corrective'),
    sub('System Info', [info('HiveLighting', ''), info('FW Ver 3.0.8', '')]),
  ];
}

/** Modes DMX Hive : SHOT - HSIC, RGB - CCT, SHOT+EFX+GEL ↔ modes du profil (HSI, RGB+CCT, FX+). */
function dmxModeOptions(c) {
  const modes = c.type.dmx?.modes || [];
  const label = (m) => (/fx/i.test(m.name) ? 'SHOT+EFX+GEL' : /rgb/i.test(m.name) ? 'RGB - CCT' : 'SHOT - HSIC');
  return modes.map((m) => [m.id, label(m)]);
}

function dmxMenu() {
  return [
    pick('DMX Input', { options: ['Lumen Radio', 'Local Wired'], ...stored('dmxInput', 'Local Wired') }),
    act('CRMX Linking', (c) => (c.panel('dmxInput', 'Local Wired') === 'Lumen Radio' ? 'Unlinking… (pas d’émetteur)' : 'Choisis d’abord DMX Input : Lumen Radio')),
    pick('DMX Mode', { options: dmxModeOptions, get: (c) => c.s.dmxMode, set: (c, v) => c.setSettings({ dmxMode: v }) }),
    pick('Profiles', { options: PROFILES, ...stored('profile', 'Photo') }),
    sub('System Info', [info('HiveLighting', ''), info('FW Ver 3.0.8', '')]),
  ];
}

const dmxOn = (c) => c.panel('dmxSwitch', 'LOCAL') === 'DMX';

// ================================================================== Wasp 100-C / Hornet 200-C
export const hiveC = {
  id: 'hive-c',
  display: { w: 128, h: 64, palette: { bg: '#050505', fg: '#e9f3ff', dim: '#4c5866', accent: '#e9f3ff', off: '#050505' } },
  face: {
    style: 'hive',
    rows: [
      [{ el: 'led', id: 'pwr', label: 'POWER' }, { el: 'led', id: 'dmx', label: 'DMX' }],
      [{ col: [{ el: 'key', id: 'power', label: 'ON / OFF', shape: 'rect' }, { el: 'key', id: 'dmxsw', label: 'DMX / LOCAL', shape: 'rect' }] }, { el: 'display' }],
      [{ el: 'knob', id: 'select', label: 'SELECT', push: true }, { el: 'knob', id: 'adjust', label: 'ADJUST', push: true }],
    ],
    note: 'SELECT : choisir S, H, D ou K ; double-clic = menus. ADJUST : régler ; clic = fin / grossier ; maintenu 4 s = verrou. Interrupteur DMX : le projecteur n’écoute que le DMX.',
  },
  init: () => ({ cur: 'D', fine: false, lastSelect: -9, locked: false }),
  holds: { adjust: [[4, (c) => ((c.st.locked = !c.st.locked), c.say(c.st.locked ? 'Lock' : 'Unlock'))]] },
  key(c, id) {
    if (id === 'power') return c.setState({ power: !c.on });
    if (id === 'dmxsw') {
      const next = dmxOn(c) ? 'LOCAL' : 'DMX';
      c.setPanel({ dmxSwitch: next });
      c.setSettings({ source: next === 'DMX' ? 'dmx' : 'manual' });
      // de retour en Local, les lettres propres au DMX reviennent sur S.H.O.T. : C (température) → K, A (adresse) → D
      if (next === 'LOCAL' && (c.st.cur === 'C' || c.st.cur === 'A')) c.st.cur = c.st.cur === 'C' ? 'K' : 'D';
      c.menu = null;
      c.screen = 'home';
      return c.say(next === 'DMX' ? 'Mode DMX : seules les consoles et applis commandent' : 'Mode Local : réglages du panneau');
    }
    if (!c.on) return;
    if (c.st.locked) return c.say('Lock : ADJUST maintenu 4 s pour déverrouiller');
    if (id === 'select') {
      const double = c.t - c.st.lastSelect < 0.45;
      c.st.lastSelect = c.t;
      if (c.screen === 'menu') {
        const r = c.menu.enter(c);
        if (r?.msg) c.say(r.msg);
        return;
      }
      if (double) c.openMenu(dmxOn(c) ? dmxMenu() : localMenu(), dmxOn(c) ? 'DMX MENU' : 'MENU', { wrap: true });
      return;
    }
    if (id === 'adjust') {
      if (c.screen === 'menu') {
        if (!c.menu.back()) c.closeMenu();
        return;
      }
      c.st.fine = !c.st.fine;
      return c.say(c.st.fine ? 'Réglage fin' : 'Réglage grossier');
    }
  },
  turn(c, id, steps) {
    if (!c.on) return;
    if (c.st.locked) return c.say('Lock');
    if (c.screen === 'menu') return id === 'select' && c.menu.move(c, steps);
    const keys = dmxOn(c) ? ['S', 'H', 'D', 'C', 'A'] : ['S', 'H', 'D', 'K'];
    if (id === 'select') {
      const i = keys.indexOf(c.st.cur);
      c.st.cur = keys[clamp((i < 0 ? 0 : i) + steps, 0, keys.length - 1)];
      return;
    }
    if (dmxOn(c)) {
      // en DMX, ADJUST règle l'adresse de départ (10 par cran ; fin en tenant la molette enfoncée)
      const fine = c.heldFor('adjust') > 0;
      return c.setSettings({ dmxAddress: clamp(c.s.dmxAddress + steps * (fine ? 1 : 10), 1, 512) });
    }
    adjustShot(c, c.st.cur, steps, c.st.fine);
  },
  draw(c) {
    const W = 128;
    if (c.screen === 'menu') {
      const items = c.menu.items(c);
      const idx = c.menu.level.cursor;
      if (c.menu.edit) return { prims: drawEdit(c, c.menu, { rows: 2, rowH: 20, size: 12 }) };
      const top = c.menu.stack.length === 1;
      return {
        prims: [
          { t: 'text', x: 2, y: 4, text: top ? `${c.menu.level.title}` : c.menu.level.title, s: 11, bold: true },
          { t: 'text', x: W - 2, y: 4, text: top ? `${idx + 1}/${items.length}` : '', s: 11, align: 'r', bold: true },
          { t: 'line', x1: 0, y1: 20, x2: W, y2: 20, w: 1 },
          { t: 'text', x: W / 2, y: 30, text: items[idx]?.label || '', s: 14, align: 'c', bold: true },
          { t: 'text', x: W / 2, y: 50, text: items[idx]?.kind === 'info' ? '' : 'click SELECT', s: 7, align: 'c', color: 'dim' },
        ],
      };
    }
    const v = shot(c);
    const dmx = dmxOn(c);
    const rows = dmx ? [['S', `${v.S}%`], ['H', `${v.H}°`], ['D', `${v.D}%`], ['C', `${v.K}`], ['A', pad(c.s.dmxAddress)]] : [['S', `${v.S}%`], ['H', `${v.H}°`], ['D', `${v.D}%`], ['K', `${v.K}`]];
    const prims = [{ t: 'text', x: W - 3, y: 2, text: c.panel('profile', 'Photo')[0], s: 11, align: 'r', bold: true }];
    if (dmx) prims.push({ t: 'text', x: W - 3, y: 16, text: c.panel('dmxInput', 'Local Wired') === 'Lumen Radio' ? 'CRMX' : 'WIRE', s: 8, align: 'r' }, { t: 'text', x: W - 3, y: 28, text: 'SHOT', s: 8, align: 'r', color: 'dim' });
    const rowH = dmx ? 12.5 : 15.5;
    rows.forEach(([k, val], i) => {
      const y = 2 + i * rowH;
      const sel = c.st.cur === k;
      prims.push({ t: 'text', x: 4, y, text: `${sel ? '>' : ' '}${k}`, s: dmx ? 10 : 12, bold: true, font: 'mono' });
      prims.push({ t: 'text', x: 30, y, text: val, s: dmx ? 10 : 12, bold: sel, font: 'mono' });
    });
    if (!dmx) prims.push({ t: 'text', x: W - 3, y: 40, text: c.st.fine ? 'FINE' : 'x10', s: 7, align: 'r', color: 'dim' });
    if (c.st.locked) prims.push({ t: 'text', x: W - 3, y: 52, text: '🔒', s: 9, align: 'r' });
    return { prims };
  },
  leds(c) {
    return { pwr: c.on ? { color: LED.green } : null, dmx: c.on && dmxOn(c) ? { color: c.dmxLive() ? LED.blue : LED.amber } : null };
  },
};

// ================================================================== Super Hornet 575-C (ballast)
function superMenu() {
  return [
    listPage('Effects', EFFECTS, 'effect'),
    listPage('Sources', SOURCES, 'source'),
    pick('Profiles', { options: PROFILES, ...stored('profile', 'Photo') }),
    listPage('Corrective', CORRECTIVE, 'corrective'),
    sub('DMX', [
      pick('DMX Input', { options: ['Lumen Radio', 'Wired', 'Ethernet'], ...stored('dmxInput', 'Wired') }),
      act('CRMX Linking', () => 'Pas d’émetteur CRMX'),
      pick('DMX Mode', { options: dmxModeOptions, get: (c) => c.s.dmxMode, set: (c, v) => c.setSettings({ dmxMode: v }) }),
      num('DMX Address', { min: 1, max: 512, get: (c) => c.s.dmxAddress, set: (c, v) => c.setSettings({ dmxAddress: v }), fmt: (v) => pad(v) }),
    ]),
    sub('System Info', [info('HiveLighting', ''), info('Super Hornet 575-C', '')]),
  ];
}

export const superHornet = {
  id: 'hive-super',
  display: { w: 128, h: 64, palette: { bg: '#eef3f8', fg: '#1e3fa8', dim: '#9aaccc', accent: '#1e3fa8', off: '#2b3036' } },
  face: {
    style: 'hive',
    rows: [
      [{ el: 'led', id: 'status', label: 'STATUS' }],
      [{ col: [{ el: 'knob', id: 'select', label: 'SELECT' }, { el: 'key', id: 'menu', label: 'MENU' }] }, { el: 'display' }, { col: [{ el: 'knob', id: 'adjust', label: 'ADJUST' }, { el: 'key', id: 'back', label: 'BACK' }] }],
      [{ el: 'key', id: 'power', label: '⏻', shape: 'rect' }],
    ],
    note: 'Menus du 575-C non publiés (manuel introuvable) : arborescence reconstituée d’après les autres Hive.',
  },
  init: () => ({ cur: 3 }),
  key(c, id) {
    if (id === 'power') return c.setState({ power: !c.on });
    if (!c.on) return;
    if (id === 'menu') {
      if (c.screen === 'menu') {
        const r = c.menu.enter(c);
        if (r?.msg) c.say(r.msg);
        return;
      }
      return c.openMenu(superMenu(), 'MENU');
    }
    if (id === 'back' && c.screen === 'menu' && !c.menu.back()) c.closeMenu();
  },
  turn(c, id, steps, mult) {
    if (!c.on) return;
    if (c.screen === 'menu') return id === 'select' ? c.menu.move(c, steps) : c.menu.edit && c.menu.move(c, steps, mult);
    const keys = ['S', 'H', 'D', 'K'];
    if (id === 'select') return (c.st.cur = clamp(c.st.cur + steps, 0, 3));
    adjustShot(c, keys[c.st.cur], steps, mult < 2);
  },
  draw(c) {
    const W = 128;
    if (c.screen === 'menu') return { prims: c.menu.edit ? drawEdit(c, c.menu, { rows: 3 }) : drawList(c, c.menu, { rows: 3 }) };
    const v = shot(c);
    const [kLo, kHi] = cctRange(c, RANGES.K);
    const lines = [
      ['SAT', `${v.S}%`, v.S / 100],
      ['HUE', `${v.H}°`, v.H / 360],
      ['DIM', `${v.D}%`, v.D / 100],
      ['CCT', `${v.K}K`, (v.K - kLo) / (kHi - kLo)],
    ];
    const prims = [
      { t: 'rect', x: 0, y: 0, w: W, h: 12, fill: 'fg' },
      { t: 'text', x: 3, y: 1, text: c.panel('effect') && c.panel('effect') !== 'Off' ? 'EFX' : 'SHOT', s: 9, inv: true, bold: true },
      { t: 'text', x: W - 3, y: 1, text: c.panel('profile', 'Photo'), s: 9, inv: true, align: 'r', bold: true },
    ];
    lines.forEach(([k, val, f], i) => {
      const y = 15 + i * 12;
      const sel = c.st.cur === i;
      prims.push({ t: 'text', x: 2, y, text: `${sel ? '▶' : ' '}${k}:${val}`, s: 9, bold: sel });
      prims.push({ t: 'rect', x: 68, y: y + 2, w: 56, h: 6, stroke: 'dim' });
      prims.push({ t: 'rect', x: 68, y: y + 2, w: Math.max(1, 56 * clamp(f, 0, 1)), h: 6, fill: 'fg' });
    });
    return { prims };
  },
  leds(c) {
    return { status: c.on ? { color: LED.green } : null };
  },
};
