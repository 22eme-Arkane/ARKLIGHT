// Kino Flo (docs/recherche/menus/dedo_dmg_kino_litegear.md) :
//  - Celeb 250 / 450 LED DMX, panneau True Match (firmware 5.0) : OLED ambre, ⏻ (voyant vert / rouge),
//    MENU, MODE, LOCK, préréglages 1–4, molette cliquable (fin / grossier) ;
//  - Celeb LED 201 DMX (2015) : afficheur rouge 4 caractères, 5 touches Kelvin, DMX ADDRESS,
//    KELVIN / DIM, LOCK / RESET, molette. Pas de menu.
import { act, drawEdit, drawList, info, num, pad, pick, stored, sub } from '../panel.js';
import { LED, cctRange, clamp, profileItem, serialOf } from './common.js';

// ================================================================== Celeb 250 / 450 (True Match)
const DEFAULT_PRESETS = [2700, 3200, 5000, 6500];
const MODES_TM = ['WHITE', 'GELS/HUE', 'RGB', 'CIE xy', 'FX'];
const firmware = (c) => (c.type.id.includes('250') ? 'CEL2' : 'CEL4');

function tmTree() {
  return [
    sub('GENERAL', [
      pick('MODE', {
        options: MODES_TM,
        get: (c) => c.panel('mode', c.m.mode === 'hsi' ? 'GELS/HUE' : 'WHITE'),
        set: (c, v) => {
          c.setPanel({ mode: v });
          c.st.field = 0;
          if (v === 'WHITE') return c.setManual({ mode: 'cct' });
          if (v === 'GELS/HUE') return c.setManual({ mode: 'hsi' });
          return `${v} : mode non simulé dans ARKLIGHT`;
        },
      }),
      pick('TIMEOUT (Sec)', { options: [0, 5, 10, 15, 20, 30], ...stored('timeout', 15) }),
      pick('SMOOTH', { options: ['ON', 'OFF'], ...stored('smooth', 'ON') }),
      pick('SCREEN', { options: ['NORMAL', 'ROTATED'], ...stored('screen', 'NORMAL') }),
      num('FREQ', { min: 30, max: 300, fmt: (v) => `${pad(v)} kHz${v === 30 ? ' (Std)' : ''}`, ...stored('freq', 30) }),
    ]),
    sub('RESET', [
      pick('FACTORY RESET', {
        options: ['NO', 'YES'],
        get: () => 'NO',
        set: (c, v) => {
          if (v !== 'YES') return;
          c.setSettings({ panel: {}, dmxAddress: 1, dmxMode: c.type.dmx?.modes?.[0]?.id, dimmingCurve: 'square' });
          c.setManual({ mode: 'cct', intensity: 0.1, cct: 2700, tint: 0 });
          return 'Dim = 10%, Kelvin = 2700K, G/M = 000, DMX = 001';
        },
      }),
      pick('CLEAR SETTINGS', { options: ['NO', 'YES'], get: () => 'NO', set: (c, v) => (v === 'YES' ? (c.setManual({ intensity: 0.1, cct: 2700, tint: 0 }), 'Réglages effacés') : undefined) }),
      pick('CLEAR PRESETS', { options: ['NO', 'YES'], get: () => 'NO', set: (c, v) => (v === 'YES' ? (c.setPanel({ presets: null }), 'Préréglages 2700 / 3200 / 5000 / 6500 K') : undefined) }),
      pick('DEMO', { options: ['OFF', 'SCROLL'], ...stored('demo', 'OFF') }),
      info('RDM', (c) => `0x4B46${serialOf(c, 8, '0123456789ABCDEF')}`),
    ]),
    sub('DMX', [
      num('DMX ADDR', { min: 0, max: 512, get: (c) => c.panel('addr0', false) ? 0 : c.s.dmxAddress, set: (c, v) => (v === 0 ? (c.setPanel({ addr0: true }), c.setSettings({ source: 'manual' }), 'Adresse 0 : commande manuelle, câble branché') : (c.setPanel({ addr0: false }), c.setSettings({ dmxAddress: v, source: 'auto' }))), fmt: (v) => pad(v) }),
      profileItem('PERSONALITY', (m) => m.name.replace(/\s+/g, ' ').slice(0, 14)),
      pick('RDM', { options: ['ENABLED', 'DISABLED'], ...stored('rdm', 'ENABLED') }),
      pick('DIM CURVE', {
        options: [
          ['square', 'SQUARE'],
          ['linear', 'LINEAR'],
        ],
        get: (c) => c.s.dimmingCurve,
        set: (c, v) => c.setSettings({ dimmingCurve: v }),
      }),
    ]),
    sub('DMX WIRELESS', [
      pick('ENABLE', { options: ['OFF', 'ON'], ...stored('wireless', 'OFF'), set: (c, v) => (c.setPanel({ wireless: v }), v === 'ON' ? 'Commandes manuelles désactivées tant que le sans-fil est actif' : undefined) }),
      act('UNLINK - PRESS 3 SEC', () => 'STATUS: UNLINKED'),
      info('STATUS', (c) => (c.panel('wireless', 'OFF') === 'ON' ? 'LINKING' : 'UNLINKED')),
      info('SIGNAL', '000%'),
      pick('ANTENNA', { options: ['#2 EXT', '#1 PCB'], ...stored('antenna', '#2 EXT') }),
    ]),
    pick('CAMERA LUT', { options: ['KF Default', 'C1  Arri Alexa', 'C2  Sony Venice', 'C3  Panavision DXL', 'C4  Panasonic Varicam'], ...stored('lut', 'KF Default') }),
    pick('COLOR SPACE', { options: ['KF Default', 'rec 709 / sRGB', 'P3 D65', 'rec 2020'], ...stored('colorSpace', 'KF Default') }),
  ];
}

const tmMode = (c) => c.panel('mode', c.m.mode === 'hsi' ? 'GELS/HUE' : 'WHITE');
const tmFields = (c) => (tmMode(c) === 'GELS/HUE' ? ['HUE', 'SAT', 'DIM', 'KELVIN', 'G/M'] : ['DIM', 'KELVIN', 'G/M']);
const gm = (t) => {
  const v = Math.round(Math.abs(t) * 100);
  return t === 0 || v === 0 ? '000' : `${pad(v)}${t > 0 ? 'G' : 'M'}`;
};
const locked = (c) => c.panel('locked', false);

export const kinoTrueMatch = {
  id: 'kino-truematch',
  displayWhenOff: true, // ⏻ ne commande que la source : l'écran et les menus restent actifs
  display: { w: 128, h: 64, palette: { bg: '#000000', fg: '#ffb20a', dim: '#5e4300', accent: '#ffb20a', off: '#000000' } },
  face: {
    style: 'kino',
    rows: [
      [
        { col: [{ el: 'key', id: 'power', label: '⏻' }, { el: 'key', id: 'menu', label: 'MENU', shape: 'green' }] },
        { el: 'display' },
        { col: [{ el: 'led', id: 'lock', label: '' }, { el: 'key', id: 'lock', label: 'LOCK', shape: 'red' }, { el: 'key', id: 'mode', label: 'MODE', shape: 'green' }] },
        { el: 'knob', id: 'knob', label: '', push: true, size: 'l', cap: 'yellow' },
      ],
      [
        { el: 'key', id: 'p1', label: '1', shape: 'small ring-red' },
        { el: 'key', id: 'p2', label: '2', shape: 'small ring-yellow' },
        { el: 'key', id: 'p3', label: '3', shape: 'small ring-lightblue' },
        { el: 'key', id: 'p4', label: '4', shape: 'small ring-blue' },
        { el: 'led', id: 'power', label: 'ON' },
      ],
    ],
    note: 'MODE : DIM → KELVIN → G/M. Molette : clic = fin / grossier. Préréglages : appui court = rappel, 3 s = enregistrer. LOCK : bloque tout (3 s = préréglages d’usine).',
  },
  init: () => ({ field: 0, fine: true }),
  holds: Object.assign(
    {
      lock: [[3, (c) => (c.setPanel({ presets: null }), c.say('Préréglages d’usine restaurés'))]],
      mode: [[1, (c) => c.menu && (c.menu.back() || c.closeMenu())]],
      menu: [[3, (c) => c.say('Raccourci : changer de mode dans GENERAL › MODE')]],
    },
    Object.fromEntries(
      ['p1', 'p2', 'p3', 'p4'].map((id, i) => [
        id,
        [
          [
            3,
            (c) => {
              const p = [...(c.panel('presets', null) || DEFAULT_PRESETS.map((k) => ({ cct: k, tint: 0 })))];
              p[i] = { cct: c.m.cct, tint: c.m.tint, intensity: c.m.intensity };
              c.setPanel({ presets: p });
              c.st.flashK = c.t + 1;
              c.say(`Préréglage ${i + 1} enregistré`);
            },
          ],
        ],
      ]),
    ),
  ),
  key(c, id) {
    if (id === 'power') return c.setState({ power: !c.on });
    if (id === 'lock') {
      const now = !locked(c);
      c.setPanel({ locked: now });
      return c.say(now ? 'Verrouillé : commandes manuelles désactivées' : 'Déverrouillé');
    }
    if (locked(c)) return c.say('Verrouillé (LOCK)');
    if (/^p\d$/.test(id)) {
      const i = Number(id[1]) - 1;
      const p = (c.panel('presets', null) || DEFAULT_PRESETS.map((k) => ({ cct: k, tint: 0 })))[i];
      c.setManual({ mode: 'cct', cct: p.cct, tint: p.tint ?? 0, ...(p.intensity !== undefined ? { intensity: p.intensity } : {}) });
      c.setPanel({ mode: 'WHITE' });
      return;
    }
    if (id === 'menu') return c.menu ? c.closeMenu() : c.openMenu(tmTree(), `TrueMatch 5.00 ${firmware(c)}`);
    if (id === 'mode') {
      if (c.menu) return c.closeMenu();
      c.st.field = (c.st.field + 1) % tmFields(c).length;
      return;
    }
    if (id === 'knob') {
      if (c.menu) {
        const r = c.menu.enter(c);
        if (r?.msg) c.say(r.msg);
        return;
      }
      c.st.fine = !c.st.fine;
      c.say(c.st.fine ? 'Fine' : 'Coarse');
    }
  },
  turn(c, id, steps, mult) {
    if (locked(c)) return c.say('Verrouillé (LOCK)');
    if (c.menu) return c.menu.move(c, steps, mult);
    const f = tmFields(c)[c.st.field % tmFields(c).length];
    const k = c.st.fine ? 1 : 10;
    const m = c.m;
    if (f === 'DIM') return c.setManual({ intensity: clamp(Math.round((m.intensity ?? 0) * 100 + steps * k) / 100, 0, 1) });
    if (f === 'KELVIN') {
      const [lo, hi] = tmMode(c) === 'WHITE' ? [2700, 6500] : cctRange(c, [2500, 9900]);
      return c.setManual({ cct: clamp(Math.round(((m.cct || 5600) + steps * (c.st.fine ? 25 : 250)) / 25) * 25, lo, hi) });
    }
    if (f === 'G/M') return c.setManual({ tint: clamp(Math.round((m.tint || 0) * 100 + steps * k) / 100, -1, 1) });
    if (f === 'HUE') return c.setManual({ hue: (((Math.round(m.hue || 0) + steps * k) % 360) + 360) % 360 });
    if (f === 'SAT') return c.setManual({ sat: clamp(Math.round((m.sat ?? 1) * 100 + steps * k) / 100, 0, 1) });
  },
  draw(c) {
    const W = 128;
    if (c.menu) return { prims: c.menu.edit ? drawEdit(c, c.menu, { rows: 3 }) : drawList(c, c.menu, { rows: 3, titleSize: 9 }) };
    const m = c.m;
    const f = tmFields(c)[c.st.field % tmFields(c).length];
    const prims = [];
    const live = c.dmxLive() && !c.panel('addr0', false);
    const label = (txt, x, y, sel, align = 'c') => {
      if (sel) prims.push({ t: 'rect', x: align === 'c' ? x - txt.length * 3.3 - 2 : x - 1, y: y - 1, w: txt.length * 6.6 + 4, h: 10, fill: 'fg' });
      prims.push({ t: 'text', x, y, text: txt, s: 8, align, bold: true, inv: sel });
    };
    const dmxText = `DMX: ${c.panel('addr0', false) ? '000' : pad(c.s.dmxAddress)}${live ? ' X' : ''}`;
    const kBlink = c.st.flashK > c.t && Math.floor(c.t * 4) % 2 === 0;
    if (tmMode(c) === 'GELS/HUE') {
      label('H:', 2, 2, f === 'HUE', 'l');
      prims.push({ t: 'text', x: 16, y: 2, text: `${pad(Math.round(m.hue || 0))}°`, s: 8 });
      label('SAT:', 70, 2, f === 'SAT', 'l');
      prims.push({ t: 'text', x: 96, y: 2, text: pad(Math.round((m.sat ?? 1) * 100)), s: 8 });
      prims.push({ t: 'line', x1: 0, y1: 13, x2: W, y2: 13 }, { t: 'text', x: 2, y: 16, text: 'GEL:', s: 8 }, { t: 'line', x1: 0, y1: 27, x2: W, y2: 27 });
      label('G/ M:', 2, 30, f === 'G/M', 'l');
      prims.push({ t: 'text', x: 34, y: 30, text: gm(m.tint || 0), s: 8 }, { t: 'text', x: W - 2, y: 30, text: dmxText, s: 8, align: 'r' }, { t: 'line', x1: 0, y1: 41, x2: W, y2: 41 });
      label('DIM:', 2, 46, f === 'DIM', 'l');
      prims.push({ t: 'text', x: 28, y: 46, text: String(Math.round((m.intensity ?? 0) * 100)), s: 8 });
      label('KELVIN:', 62, 46, f === 'KELVIN', 'l');
      if (!kBlink) prims.push({ t: 'text', x: 104, y: 46, text: String(Math.round(m.cct || 6500)), s: 8 });
      return { prims };
    }
    label('G/ M:', 2, 2, f === 'G/M', 'l');
    prims.push({ t: 'text', x: 34, y: 2, text: gm(m.tint || 0), s: 8 });
    prims.push({ t: 'text', x: W - 2, y: 2, text: dmxText, s: 8, align: 'r' });
    prims.push({ t: 'line', x1: 0, y1: 14, x2: W, y2: 14 }, { t: 'line', x1: 64, y1: 14, x2: 64, y2: 62 });
    label(c.panel('smooth', 'ON') === 'OFF' ? '*DIM' : 'DIM', 32, 20, f === 'DIM');
    label('KELVIN', 96, 20, f === 'KELVIN');
    prims.push({ t: 'text', x: 32, y: 36, text: String(Math.round((m.intensity ?? 0) * 100)), s: 16, align: 'c', bold: true });
    if (!kBlink) prims.push({ t: 'text', x: 96, y: 36, text: String(Math.round(m.cct || 6500)), s: 16, align: 'c', bold: true });
    if (!c.st.fine) prims.push({ t: 'text', x: 2, y: 54, text: 'COARSE', s: 6, color: 'dim' });
    return { prims };
  },
  leds(c) {
    return { lock: locked(c) ? { color: LED.red } : null, power: { color: c.on ? LED.green : LED.red } };
  },
};

// ================================================================== Celeb LED 201 DMX
const PRESETS_201 = [2700, 3200, 4500, 5500, 6500];

export const kinoCeleb201 = {
  id: 'kino-celeb201',
  display: { w: 70, h: 24, zoom: 2.4, palette: { bg: '#100303', fg: '#ff2b20', dim: '#401010', accent: '#ff2b20', off: '#100303' } },
  displayWhenOff: true,
  face: {
    style: 'kino',
    rows: [
      [
        { col: [{ el: 'led', id: 'pl0', label: '' }, { el: 'key', id: 'k0', label: '', shape: 'small ring-red' }] },
        { col: [{ el: 'led', id: 'pl1', label: '' }, { el: 'key', id: 'k1', label: 'TUNGSTEN', shape: 'small ring-orange' }] },
        { col: [{ el: 'led', id: 'pl2', label: '' }, { el: 'key', id: 'k2', label: '', shape: 'small ring-lightblue' }] },
        { col: [{ el: 'led', id: 'pl3', label: '' }, { el: 'key', id: 'k3', label: '', shape: 'small ring-blue' }] },
        { col: [{ el: 'led', id: 'pl4', label: '' }, { el: 'key', id: 'k4', label: 'DAYLIGHT', shape: 'small ring-darkblue' }] },
      ],
      [
        { col: [{ el: 'led', id: 'dmxled', label: '' }, { el: 'key', id: 'dmx', label: 'DMX ADDRESS', shape: 'small' }] },
        { col: [{ el: 'led', id: 'kdled', label: '' }, { el: 'key', id: 'kd', label: 'KELVIN / DIM', shape: 'small' }] },
        { el: 'display' },
        { col: [{ el: 'led', id: 'lockled', label: '' }, { el: 'key', id: 'lock', label: 'LOCK / RESET', shape: 'small' }] },
        { col: [{ el: 'led', id: 'pwr', label: '' }, { el: 'key', id: 'power', label: '⏻', shape: 'small' }] },
        { el: 'knob', id: 'knob', label: '', push: true },
      ],
    ],
    note: 'Pas de menu. KELVIN / DIM bascule ce que règle la molette (clic = fin / grossier). DMX ADDRESS maintenu 3 s = adresse ; LOCK / RESET maintenu 5 s = réglages d’usine.',
  },
  init: () => ({ kd: 'K', fine: false, edit: false }),
  holds: Object.assign(
    {
      dmx: [[3, (c) => ((c.st.edit = true), c.say('Adresse DMX : molette, puis DMX ADDRESS pour enregistrer'))]],
      lock: [[5, (c) => (c.setPanel({ presets: null, locked: false }), c.setSettings({ dmxAddress: 1 }), c.setManual({ mode: 'cct', cct: 5500, intensity: 1 }), c.say('Réglages d’usine'))]],
    },
    Object.fromEntries(PRESETS_201.map((_, i) => [`k${i}`, [[5, (c) => (c.setPanel({ presets: Object.assign([...(c.panel('presets', null) || PRESETS_201)], { [i]: Math.round(c.m.cct || 5500) }) }), (c.st.flashUntil = c.t + 1), c.say('Préréglage enregistré'))]]])),
  ),
  key(c, id) {
    if (id === 'power') return c.setState({ power: !c.on });
    if (!c.on) return;
    if (id === 'lock') {
      c.setPanel({ locked: !c.panel('locked', false) });
      return;
    }
    if (id === 'kd') {
      c.st.kd = c.st.kd === 'K' ? 'D' : 'K';
      return;
    }
    if (c.panel('locked', false)) return c.say('LOCK');
    if (id === 'dmx') {
      if (c.st.edit) {
        c.st.edit = false;
        return c.say(`Adresse ${pad(c.s.dmxAddress)} enregistrée`);
      }
      return c.say('Maintenir DMX ADDRESS 3 s');
    }
    if (/^k\d$/.test(id)) {
      const i = Number(id[1]);
      const list = c.panel('presets', null) || PRESETS_201;
      c.setPanel({ active: i });
      return c.setManual({ mode: 'cct', cct: list[i] });
    }
    if (id === 'knob') c.st.fine = !c.st.fine;
  },
  turn(c, id, steps) {
    if (!c.on || c.panel('locked', false)) return c.panel('locked', false) && c.say('LOCK');
    if (c.st.edit) return c.setSettings({ dmxAddress: clamp(c.s.dmxAddress + steps * (c.st.fine ? 1 : 10), 1, 511) });
    const m = c.m;
    if (c.st.kd === 'K') {
      // grossier : toute la plage en un tour (24 crans) ; fin : 8 tours
      const step = c.st.fine ? 3800 / 192 : 3800 / 24;
      c.setPanel({ active: -1 });
      return c.setManual({ mode: 'cct', cct: clamp(Math.round(((m.cct || 5500) + steps * step) / 25) * 25, 2700, 6500) });
    }
    c.setManual({ intensity: clamp(Math.round(((m.intensity ?? 0) + steps * (c.st.fine ? 0.001 : 0.04)) * 1000) / 1000, 0, 1) });
  },
  draw(c) {
    if (!c.on) return { prims: [{ t: 'text', x: 35, y: 3, text: 'V0.02', s: 15, align: 'c', bold: true, font: 'seg' }] };
    let text;
    if (c.panel('locked', false)) text = 'LOCK';
    else if (c.st.edit) text = Math.floor(c.t * 3) % 2 ? String(c.s.dmxAddress).padStart(4, ' ') : '';
    else if (c.st.kd === 'K') text = c.st.flashUntil > c.t && Math.floor(c.t * 4) % 2 ? '' : String(Math.round(c.m.cct || 5500));
    else text = ((c.m.intensity ?? 0) * 100).toFixed(1);
    return { prims: [{ t: 'text', x: 66, y: 3, text, s: 16, align: 'r', bold: true, font: 'seg', spacing: 1 }] };
  },
  leds(c) {
    const active = c.panel('active', -1);
    const live = c.dmxLive();
    const out = {};
    for (let i = 0; i < 5; i++) out[`pl${i}`] = c.on && (live || active === i) ? { color: LED.green } : null;
    out.kdled = c.on ? { color: c.st.kd === 'K' ? LED.amber : LED.green } : null;
    out.lockled = c.panel('locked', false) ? { color: LED.amber } : null;
    out.dmxled = c.st.edit ? { color: LED.green, blinkHz: 2 } : null;
    out.pwr = { color: c.on ? LED.green : LED.amber };
    return out;
  },
};
