// LiteGear (docs/recherche/menus/dedo_dmg_kino_litegear.md) : les LiteMat n'ont aucune commande,
// tout est sur le LiteDimmer relié par câble.
//  - LiteMat Plus 2 / 4 → LiteDimmer Plus DC200 / AC400 DUO : OLED, trois touches sous l'écran
//    (← · centre · →), molettes LEVEL1, KELVIN 1, LEVEL 2, KELVIN 2, sélecteur LOCAL / MENU / DMX.
//  - S2 LiteMat 2 / 4 → LiteDimmer Pro Hybrid : pas d'écran, molettes KELVIN et LEVEL, interrupteur
//    ON / OFF / ALT, touche PROG, un voyant multicolore.
import { pad } from '../panel.js';
import { LED, cctRange, clamp, footprint } from './common.js';

// ================================================================== LiteDimmer Plus DUO
/** Écrans du menu (sélecteur sur MENU) : chaque ligne a ses valeurs, ← / → les changent. */
const PAGES = [
  {
    title: 'DIMMER SETTINGS',
    rows: [
      ['CIRCUIT', 'circuit', ['SINGLE', 'DOUBLE'], 'SINGLE'],
      ['DIM CURVE', 'curve', ['STANDARD'], 'STANDARD'],
      ['SMOOTING', 'smoothing', ['ON', 'OFF'], 'ON'],
    ],
  },
  {
    title: 'HYBRID SETTINGS',
    rows: [
      ['TRIGGER', 'trigger', ['CUE', 'BUMP'], 'CUE'],
      ['FADE SPEED', 'fade', null, 24],
      ['MASTER DMX OUT', 'master', ['OFF', 'ON'], 'OFF'],
    ],
  },
  {
    title: 'DMX SETTINGS',
    rows: [
      ['BIT RATE', 'bits', ['8', '16'], '8'],
      ['ON DMX LOSS', 'loss', ['HOLD', 'DROP'], 'HOLD'],
    ],
  },
  { title: 'DMX ADDRESS', rows: [['', 'address', null, 1]] },
];

const position = (c) => c.panel('switch', 'LOCAL');

function setSwitch(c, pos) {
  c.setPanel({ switch: pos });
  c.setSettings({ source: pos === 'DMX' ? 'dmx' : 'manual' });
  c.st.page = 0;
  c.st.row = -1;
  c.st.welcome = pos === 'MENU';
}

function rowValue(c, row) {
  const [, key, , dflt] = row;
  if (key === 'address') return pad(c.s.dmxAddress);
  if (key === 'fade') return `${c.panel('fade', dflt)} Frms`;
  return String(c.panel(key, dflt));
}

function changeRow(c, row, dir) {
  const [, key, options, dflt] = row;
  if (key === 'address') return c.setSettings({ dmxAddress: clamp(c.s.dmxAddress + dir, 1, 512 - footprint(c) + 1) });
  if (key === 'fade') return c.setPanel({ fade: clamp(c.panel('fade', dflt) + dir * 2, 2, 72) });
  const i = options.indexOf(String(c.panel(key, dflt)));
  c.setPanel({ [key]: options[(i + dir + options.length) % options.length] });
}

export const litedimmerPlus = {
  id: 'litegear-plus-duo',
  display: { w: 128, h: 64, palette: { bg: '#020406', fg: '#e8f2ff', dim: '#4a5664', accent: '#e8f2ff', off: '#020406' } },
  face: {
    style: 'litegear',
    rows: [
      [{ el: 'led', id: 'pwr', label: 'PWR' }, { el: 'led', id: 'dmx', label: 'DMX' }, { el: 'key', id: 'link', label: 'STATUS/LINK', shape: 'small blue' }, { el: 'led', id: 'mode', label: '' }],
      [{ col: [{ el: 'key', id: 'local', label: 'LOCAL', shape: 'rect' }, { el: 'key', id: 'menu', label: 'MENU', shape: 'rect' }, { el: 'key', id: 'dmxsw', label: 'DMX', shape: 'rect' }] }, { col: [{ el: 'display' }, { row: true, col: [{ el: 'key', id: 'left', label: '←', shape: 'small' }, { el: 'key', id: 'center', label: '', shape: 'small' }, { el: 'key', id: 'right', label: '→', shape: 'small' }] }] }],
      [{ el: 'knob', id: 'k2', label: 'KELVIN 2', pointer: true, size: 's' }, { el: 'knob', id: 'l2', label: 'LEVEL 2', pointer: true, size: 's' }, { el: 'knob', id: 'k1', label: 'KELVIN 1', pointer: true, size: 's' }, { el: 'knob', id: 'l1', label: 'LEVEL1', pointer: true, size: 's' }],
    ],
    note: 'Sélecteur à 3 positions : LOCAL (molettes), MENU (réglages), DMX (console). En MENU : ← / → changent d’écran, le centre choisit une ligne, puis ← / → changent la valeur.',
  },
  init: () => ({ page: 0, row: -1, welcome: false }),
  key(c, id) {
    if (id === 'local') return setSwitch(c, 'LOCAL');
    if (id === 'menu') return setSwitch(c, 'MENU');
    if (id === 'dmxsw') return setSwitch(c, 'DMX');
    if (id === 'link') return c.say('STATUS/LINK : pas de liaison sans fil (interrupteur TX / OFF / RX sur OFF)');
    const pos = position(c);
    if (pos === 'LOCAL') {
      if (id === 'center') {
        // TRIGGER : CUE = fondu vers le noir / retour ; BUMP = flash
        const off = !c.panel('cueOff', false);
        c.setPanel({ cueOff: off });
        c.setState({ power: !off });
        return c.say(off ? `CUE : fondu au noir (${c.panel('fade', 24)} images)` : 'CUE : retour');
      }
      return;
    }
    if (pos === 'MENU') {
      if (c.st.welcome) {
        c.st.welcome = false;
        if (id === 'center') return c.say('INFO : LiteDimmer+ Plus · RESET avec ←');
        return;
      }
      const page = PAGES[c.st.page];
      if (id === 'center') {
        c.st.row = c.st.row + 1 >= page.rows.length ? -1 : c.st.row + 1; // après la dernière ligne : « Done »
        return;
      }
      const dir = id === 'right' ? 1 : -1;
      if (c.st.row >= 0) return changeRow(c, page.rows[c.st.row], dir);
      c.st.page = (c.st.page + dir + PAGES.length) % PAGES.length;
    }
  },
  turn(c, id, steps) {
    if (position(c) !== 'LOCAL') return c.say(position(c) === 'DMX' ? 'Molettes inactives en DMX' : 'Molettes inactives en MENU');
    const dbl = c.panel('circuit', 'SINGLE') === 'DOUBLE';
    if ((id === 'k2' || id === 'l2') && !dbl) return c.say('Circuit 2 : seulement en CIRCUIT DOUBLE');
    if (id === 'l1' || id === 'l2') return c.setManual({ intensity: clamp(Math.round((c.m.intensity ?? 0) * 100 + steps) / 100, 0, 1) });
    const [lo, hi] = cctRange(c, [2600, 6000]);
    c.setManual({ mode: 'cct', cct: clamp(Math.round(((c.m.cct || 4500) + steps * 50) / 50) * 50, lo, hi) });
  },
  draw(c) {
    const W = 128;
    const m = c.m;
    const pos = position(c);
    const prims = [];
    const softkeys = (center) => prims.push({ t: 'line', x1: 0, y1: 52, x2: W, y2: 52 }, { t: 'text', x: 3, y: 54, text: '←', s: 8 }, { t: 'text', x: W / 2, y: 54, text: center, s: 8, align: 'c', bold: true }, { t: 'text', x: W - 3, y: 54, text: '→', s: 8, align: 'r' });
    if (pos === 'MENU') {
      if (c.st.welcome) {
        prims.push({ t: 'text', x: W / 2, y: 2, text: 'LITEGEAR', s: 9, align: 'c', bold: true }, { t: 'text', x: W / 2, y: 13, text: 'LITEDIMMER+ PLUS', s: 8, align: 'c' }, { t: 'text', x: W / 2, y: 26, text: 'SWITCH  POSITION', s: 8, align: 'c' }, { t: 'text', x: W / 2, y: 37, text: 'Up: Local    Down: DMX', s: 7, align: 'c' });
        softkeys('INFO');
        return { prims };
      }
      const page = PAGES[c.st.page];
      prims.push({ t: 'text', x: W / 2, y: 2, text: page.title, s: 9, align: 'c', bold: true }, { t: 'line', x1: 0, y1: 13, x2: W, y2: 13 });
      page.rows.forEach((row, i) => {
        const y = 16 + i * 11;
        const sel = c.st.row === i;
        if (sel) prims.push({ t: 'rect', x: 0, y: y - 1, w: W, h: 10, fill: 'fg' });
        if (row[0]) prims.push({ t: 'text', x: 3, y, text: row[0], s: 7.5, inv: sel });
        prims.push({ t: 'text', x: row[0] ? W - 3 : W / 2, y: row[0] ? y : y + 4, text: rowValue(c, row), s: row[0] ? 7.5 : 16, align: row[0] ? 'r' : 'c', inv: sel, bold: !row[0] });
      });
      softkeys(c.st.row >= 0 ? 'DONE' : 'EDIT');
      return { prims };
    }
    const dbl = c.panel('circuit', 'SINGLE') === 'DOUBLE';
    prims.push({ t: 'text', x: W / 2, y: 2, text: 'HYBRID', s: 8, align: 'c', bold: true });
    prims.push({ t: 'text', x: 30, y: 13, text: 'KELVIN TEMP', s: 7, align: 'c' }, { t: 'text', x: 98, y: 13, text: 'INTENSITY', s: 7, align: 'c' });
    prims.push({ t: 'text', x: 30, y: 24, text: `${Math.round(m.cct || 4500)}K`, s: 12, align: 'c', bold: true }, { t: 'text', x: 98, y: 24, text: `${Math.round((m.intensity ?? 0) * 100)}%`, s: 12, align: 'c', bold: true });
    if (pos === 'LOCAL') {
      // quelle molette commande chaque valeur (KELVIN 2, LEVEL 2, KELVIN 1, LEVEL1)
      prims.push({ t: 'text', x: 30, y: 40, text: '○○●○', s: 8, align: 'c' }, { t: 'text', x: 98, y: 40, text: '○○○●', s: 8, align: 'c' });
      softkeys(c.panel('cueOff', false) ? 'CUE ON' : 'CUE OFF');
    } else softkeys(dbl ? 'TWO CIRCUIT DMX' : 'ONE CIRCUIT DMX');
    return { prims };
  },
  leds(c) {
    const pos = position(c);
    return { pwr: { color: LED.green }, dmx: pos === 'DMX' && c.dmxLive() ? { color: LED.blue } : null, mode: { color: pos === 'LOCAL' ? LED.green : pos === 'DMX' ? LED.blue : LED.amber } };
  },
};

// ================================================================== LiteDimmer Pro Hybrid (S2 LiteMat)
const ALT_MODES = [
  ['OVERDRIVE', LED.white],
  ['LOW', LED.blue],
  ['2-CHANNEL', LED.yellow],
  ['BUMP', LED.purple],
  ['PAPARAZZI', LED.purple],
  ['FIRE', LED.orange],
];

export const litedimmerHybrid = {
  id: 'litegear-pro-hybrid',
  face: {
    style: 'litegear',
    rows: [
      [{ el: 'led', id: 'status', label: 'INDICATOR' }],
      [{ el: 'knob', id: 'kelvin', label: 'KELVIN (TUN ←)', pointer: true }, { el: 'knob', id: 'level', label: 'LEVEL', pointer: true }],
      [{ el: 'key', id: 'on', label: 'ON', shape: 'rect' }, { el: 'key', id: 'off', label: 'OFF', shape: 'rect' }, { el: 'key', id: 'alt', label: 'ALT', shape: 'rect' }, { el: 'key', id: 'prog', label: 'PROG', shape: 'small' }],
    ],
    note: 'Pas d’écran ni de DMX : interrupteur ON (hybride) / OFF / ALT, molettes potentiomètres. Interrupteur sur OFF puis PROG maintenu 2 s : choix du mode ALT avec LEVEL (couleur du voyant), PROG 2 s pour valider.',
  },
  init: () => ({ choosing: false, alt: 0 }),
  holds: {
    prog: [
      [
        2,
        (c) => {
          if (c.panel('switch', 'ON') !== 'OFF') return c.say('Mettre l’interrupteur sur OFF pour programmer');
          if (c.st.choosing) {
            c.st.choosing = false;
            c.setPanel({ alt: c.st.alt });
            return c.say(`Mode ALT : ${ALT_MODES[c.st.alt][0]}`);
          }
          c.st.choosing = true;
          c.st.alt = c.panel('alt', 0);
          c.say('Tourner LEVEL pour choisir le mode ALT');
        },
      ],
    ],
  },
  key(c, id) {
    if (id === 'on' || id === 'off' || id === 'alt') {
      const pos = id.toUpperCase();
      c.setPanel({ switch: pos });
      c.setState({ power: pos !== 'OFF' });
      if (pos === 'ALT') {
        const [name] = ALT_MODES[c.panel('alt', 0)];
        return c.say(`ALT : ${name}${['BUMP', 'PAPARAZZI', 'FIRE'].includes(name) ? ' (effet non simulé)' : ''}`);
      }
      return;
    }
    if (id === 'prog') c.say('PROG : maintenir 2 s (interrupteur sur OFF)');
  },
  turn(c, id, steps) {
    if (c.st.choosing && id === 'level') return (c.st.alt = (c.st.alt + (steps > 0 ? 1 : ALT_MODES.length - 1)) % ALT_MODES.length);
    if (id === 'level') {
      const max = c.panel('switch', 'ON') === 'ALT' && ALT_MODES[c.panel('alt', 0)][0] === 'LOW' ? 0.25 : 1;
      return c.setManual({ intensity: clamp(Math.round((c.m.intensity ?? 0) * 100 + steps) / 100, 0, max) });
    }
    const [lo, hi] = cctRange(c, [2600, 6000]);
    c.setManual({ mode: 'cct', cct: clamp(Math.round(((c.m.cct || 4500) + steps * 50) / 50) * 50, lo, hi) });
  },
  draw: () => ({ prims: [] }),
  leds(c) {
    if (c.st.choosing) return { status: { color: ALT_MODES[c.st.alt][1], blinkHz: 2 } };
    const pos = c.panel('switch', 'ON');
    if (pos === 'OFF') return { status: null };
    if (pos === 'ALT') return { status: { color: ALT_MODES[c.panel('alt', 0)][1], blinkHz: ALT_MODES[c.panel('alt', 0)][0] === 'PAPARAZZI' ? 3 : 0 } };
    return { status: { color: LED.green } };
  },
};
