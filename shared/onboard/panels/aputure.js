// Panneaux Aputure (docs/recherche/menus/aputure_hive.md) :
//  - « Pro » (LS 600d Pro, 600x Pro, 1200d Pro) : écran LCD couleur, touches Menu / Lighting / Effects /
//    Back, molette SELECT (et CCT sur le 600x Pro), menu en carrousel d'icônes ;
//  - OLED (LS 600d, LS 300x, LS C300d) : 4 touches à double légende, molette MENU / BRIGHTNESS (et
//    PRESET / CCT sur le 300x), menu système en liste ;
//  - Nova P300c : boîtier OLED, molettes INT, CCT/HUE, G/M/SAT, touches LIGHT MODE, MENU, préréglages 1–4.
// Les effets se choisissent comme sur le projecteur mais ne sont pas simulés sur la lumière.
import { HOLD_LONG, MenuState, act, drawEdit, drawList, info, num, pad, pick, stored, sub } from '../panel.js';
import { LED, addressItem, cctRange, clamp, curveItem, nudge, pct, profileItem, serialOf } from './common.js';

const onOff = (key, dflt = 'OFF', vals = ['ON', 'OFF']) => ({ options: vals, ...stored(key, dflt) });
const NOT_SIMULATED = 'Mémorisé (non simulé dans ARKLIGHT)';
const FX_LS = ['Paparazzi', 'Fireworks', 'Faulty Bulb', 'Lightning', 'TV', 'Pulsing', 'Strobe', 'Explosion'];
const exitItem = act('Exit', (c) => (c.closeMenu(), undefined));

/** Choix « radio » ouvert directement (Dimming Curve, Fan Mode…) : un seul élément, déjà en édition. */
function openPick(c, item, title) {
  c.menu = new MenuState([item], title || item.label);
  c.screen = 'menu';
  c.menu.enter(c);
  c.st.single = true;
}

// ================================================================== « Pro » : écran couleur
const MODELS_PRO = {
  'aputure.ls-600d-pro': { name: '600d Pro', cct: false, fw: 'Ver: 1.8' },
  'aputure.ls-600x-pro': { name: '600x Pro', cct: true, fw: 'Ver: 1.4' },
  'aputure.ls-1200d-pro': { name: '1200d Pro', cct: false, fw: 'Ver: 1.2' },
};
const proModel = (c) => MODELS_PRO[c.type.id] || MODELS_PRO['aputure.ls-600d-pro'];

const fanItem = (vals = ['Smart', 'High', 'Medium', 'Silent']) => pick('Fan Mode', { options: vals, ...stored('fan', vals[0]) });
const curvePro = () => curveItem('Dimming Curve', { exp: 'Exp', log: 'Log', s: 'S-curve', lin: 'Linear' });
const studio = (vals = ['OFF', 'ON'], dflt = 'OFF') => pick('Studio Mode', onOff('studio', dflt, vals));
const freqItem = (label = 'Frequency Selection') => num(label, { min: 0, max: 2000, step: 100, accel: false, fmt: (v) => `+${v}Hz`, ...stored('freq', 0) });
const lossItem4 = () => pick('DMX Loss Behavior', { options: ['Black Out', 'Fade to Black', 'Hold Last Settings', 'Hold 2 min.&Fade Out'], ...stored('dmxLoss', 'Fade to Black') });
const factory = (yes = 'YES', no = 'NO') =>
  pick('Factory Reset', {
    options: [no, yes],
    get: () => no,
    set: (c, v) => {
      if (v !== yes) return;
      c.setSettings({ panel: {}, dmxAddress: 1, dimmingCurve: 'linear', dmxMode: c.type.dmx?.modes?.[0]?.id });
      c.setManual({ mode: 'cct', intensity: 0.5, cct: 5600, tint: 0 });
      return 'Réglages d’usine';
    },
  });
const control = (with24 = true) =>
  sub('Control System', [
    sub('Bluetooth', [pick('Bluetooth', onOff('bt', 'ON')), act('Bluetooth Reset', () => 'Reset failed , please try again.'), info('Number', (c) => serialOf(c, 6))]),
    ...(with24 ? [sub('2.4G Control', [pick('2.4G', onOff('g24', 'ON')), num('Channel', { min: 1, max: 3, ...stored('ch', 1) }), pick('Group', { options: ['A', 'B', 'C', 'D'], ...stored('gr', 'A') })])] : []),
    sub('Lumenradio', [pick('Lumenradio', onOff('crmx', 'OFF')), act('Pair', () => 'Pairing failed , please try again.')]),
    sub('Ethernet', [act('Manual Set IP', () => 'IP Address 0.0.0.0'), act('Automatic Obtain IP', () => 'Pas de réseau : Local IP 0.0.0.0')]),
  ]);
const update = (fw) => sub('Update', [act('Update Firmware', () => 'No Firmware : Please connect to network or plug in U-disk with firmware.'), info('Firmware Version', fw)]);
const language = (vals = ['English', '简体中文']) => pick('Language', { options: vals, ...stored('lang', vals[0]) });

function proTree(c) {
  const id = c.type.id;
  const dmxAddr = act('DMX Address', (cc) => ((cc.screen = 'dmx'), undefined));
  if (id === 'aputure.ls-600x-pro')
    return [
      pick('Output Mode', { options: ['Constant Output', 'Max Output'], ...stored('output', 'Max Output') }),
      sub('DMX Mode', [dmxAddr, lossItem4(), profileItem('DMX Profile', (m, i) => `${i + 1}.${m.name}`)]),
      curvePro(),
      fanItem(),
      studio(),
      freqItem(),
      control(true),
      language(),
      factory(),
      update(proModel(c).fw),
      sub('Custom FX', [act('Picker FX', () => NOT_SIMULATED), act('Music FX', () => NOT_SIMULATED), act('Touchbar  FX', () => NOT_SIMULATED)]),
      sub('Battery Power Options', [act('Wake Up Batteries', () => 'Activation Failed : pas de batterie'), act('Full Battery Power', () => 'Pas de batterie montée')]),
    ];
  if (id === 'aputure.ls-1200d-pro')
    return [
      sub('DMX Mode', [dmxAddr, lossItem4(), profileItem('DMX Profile', (m, i) => `${i + 1}.${m.name}`), pick('DMX Termination', onOff('term', 'OFF'))]),
      pick('Lead-Follow Mode', onOff('lead', 'NO', ['YES', 'NO'])),
      curvePro(),
      fanItem(),
      studio(),
      freqItem(),
      control(false),
      language(),
      factory(),
      update(proModel(c).fw),
      sub('Product Information', [info('Hardware Version', 'V1.0'), info('UID', (cc) => `6KM${serialOf(cc, 8)}`)]),
      sub('Custom FX', [act('Picker FX', () => NOT_SIMULATED), act('Music FX', () => NOT_SIMULATED), act('Touchbar  FX', () => NOT_SIMULATED)]),
    ];
  return [sub('DMX Mode', [dmxAddr, pick('DMX Loss Behavior', onOff('dmxLoss', 'OFF'))]), curvePro(), fanItem(), studio(), control(true), language(), update(proModel(c).fw)];
}

/** Abréviation dans les vignettes du carrousel (les icônes du projecteur ne sont pas reproduites). */
const abbr = (label) =>
  ({ 'DMX Mode': 'DMX', 'Dimming Curve': 'CRV', 'Fan Mode': 'FAN', 'Studio Mode': 'STD', 'Control System': 'CTRL', Language: 'LANG', Update: 'UPD', 'Output Mode': 'OUT', 'Frequency Selection': 'Hz', 'Factory Reset': 'RST', 'Custom FX': 'CFX', 'Battery Power Options': 'BAT', 'Lead-Follow Mode': 'L-F', 'Product Information': 'INFO' })[label] || label.slice(0, 3).toUpperCase();

const curveShort = (c) => ({ square: 'EXP', invSquare: 'LOG', sCurve: 'S-curve', linear: 'Linear' })[c.s.dimmingCurve] || 'Linear';

function proHome(c) {
  const W = 160;
  const m = c.m;
  const prims = [];
  const model = proModel(c);
  prims.push({ t: 'text', x: W - 4, y: 2, text: `${c.panel('fan', 'Smart')}  ${c.panel('bt', 'ON') === 'ON' ? 'ᛒ' : ''}`, s: 8, align: 'r', color: 'dim' });
  // tuile de gauche : anneau d'intensité
  prims.push({ t: 'rect', x: 4, y: 14, w: 70, h: 82, stroke: 'dim' });
  prims.push({ t: 'ring', x: 39, y: 52, r: 24, v: m.intensity ?? 0, w: 5, color: 'accent', track: '#28303c' });
  prims.push({ t: 'text', x: 39, y: 46, text: c.flash?.text || pct(m.intensity ?? 0, (m.intensity ?? 0) < 1 ? 1 : 0), s: 11, align: 'c', bold: true });
  prims.push({ t: 'text', x: 39, y: 82, text: 'Brightness', s: 7, align: 'c', color: 'dim' });
  if (model.cct) {
    // 600x Pro : tuile de température (chaud → froid) puis trois petites tuiles
    const [lo, hi] = cctRange(c, [2700, 6500]);
    const k = ((m.cct || 5600) - lo) / (hi - lo);
    prims.push({ t: 'rect', x: 80, y: 14, w: 76, h: 46, stroke: 'dim' });
    prims.push({ t: 'line', x1: 88, y1: 30, x2: 148, y2: 30, w: 4, color: '#ffae5a' });
    prims.push({ t: 'line', x1: 118, y1: 30, x2: 148, y2: 30, w: 4, color: '#8cc4ff' });
    prims.push({ t: 'rect', x: 86 + 60 * k, y: 26, w: 4, h: 8, fill: 'fg' });
    prims.push({ t: 'text', x: 118, y: 40, text: `${Math.round(m.cct || 5600)}K`, s: 12, align: 'c', bold: true });
    prims.push({ t: 'rect', x: 80, y: 64, w: 22, h: 32, stroke: 'dim' }, { t: 'text', x: 91, y: 76, text: curveShort(c).slice(0, 3), s: 7, align: 'c' });
    prims.push({ t: 'rect', x: 105, y: 64, w: 22, h: 32, stroke: 'dim' }, { t: 'text', x: 116, y: 76, text: 'AC', s: 8, align: 'c' });
    prims.push({ t: 'rect', x: 130, y: 64, w: 26, h: 32, stroke: 'dim' }, { t: 'text', x: 143, y: 72, text: `GR:${c.panel('gr', 'A')}`, s: 7, align: 'c' }, { t: 'text', x: 143, y: 82, text: `CH:${c.panel('ch', 1)}`, s: 7, align: 'c' });
  } else {
    const tiles = [curveShort(c), 'AC  48V', `SN: ${serialOf(c, 6)}`];
    tiles.forEach((txt, i) => {
      prims.push({ t: 'rect', x: 80, y: 14 + i * 28, w: 76, h: 25, stroke: 'dim' });
      prims.push({ t: 'text', x: 118, y: 22 + i * 28, text: txt, s: 9, align: 'c', bold: i === 0 });
    });
  }
  return prims;
}

function proCarousel(c) {
  const W = 160;
  const items = proTree(c);
  const i = ((c.st.car % items.length) + items.length) % items.length;
  const prims = [{ t: 'text', x: W / 2, y: 2, text: 'MENU', s: 10, align: 'c', bold: true }];
  const tile = (it, x, w, big) => {
    prims.push({ t: 'rect', x, y: big ? 18 : 26, w, h: big ? 44 : 30, fill: big ? 'accent' : '#1b2230' });
    prims.push({ t: 'text', x: x + w / 2, y: big ? 33 : 36, text: abbr(it.label), s: big ? 12 : 8, align: 'c', bold: true, color: big ? 'fg' : 'dim' });
  };
  tile(items[(i + items.length - 1) % items.length], 8, 34, false);
  tile(items[i], 52, 56, true);
  tile(items[(i + 1) % items.length], 118, 34, false);
  prims.push({ t: 'text', x: 3, y: 34, text: '〈', s: 10, color: 'dim' }, { t: 'text', x: W - 3, y: 34, text: '〉', s: 10, align: 'r', color: 'dim' });
  prims.push({ t: 'text', x: W / 2, y: 68, text: items[i].label, s: 10, align: 'c', bold: true });
  const n = items.length;
  for (let k = 0; k < n; k++) prims.push({ t: 'rect', x: W / 2 - n * 4 + k * 8 + 2, y: 88, w: 4, h: 4, fill: k === i ? 'accent' : 'dim' });
  return prims;
}

function dmxScreen(c, W, H, big = 22) {
  const locked = c.st.dmxLock;
  const editing = c.st.dmxEdit;
  return [
    { t: 'text', x: W / 2, y: 2, text: 'DMX Mode', s: 9, align: 'c', bold: true },
    { t: 'text', x: W / 2, y: H / 2 - big / 2, text: `DMX:${pad(c.s.dmxAddress)}`, s: big, align: 'c', bold: true, color: editing ? 'accent' : 'fg' },
    { t: 'text', x: W / 2, y: H - 14, text: locked ? '🔒 LOCK' : editing ? 'SELECT = OK' : '🔓', s: 9, align: 'c', color: 'dim' },
  ];
}

function fxScreen(c, W, opts = {}) {
  const fx = c.panel('fx', FX_LS[1]);
  const prims = [{ t: 'text', x: W / 2, y: 2, text: fx, s: 10, align: 'c', bold: true }];
  const field = c.st.fxField || 'int';
  const int = c.panel('fxInt', 0.5);
  const frq = c.panel('fxFrq', 5);
  if (opts.rings) {
    prims.push({ t: 'ring', x: W / 4 + 8, y: 50, r: 20, v: int, w: 4, color: field === 'int' ? 'accent' : 'dim', track: '#28303c' }, { t: 'text', x: W / 4 + 8, y: 45, text: pct(int, 1), s: 9, align: 'c', bold: true }, { t: 'text', x: W / 4 + 8, y: 78, text: 'Intensity', s: 7, align: 'c', color: field === 'int' ? 'fg' : 'dim' });
    prims.push({ t: 'ring', x: (W * 3) / 4 - 8, y: 50, r: 20, v: frq / 10, w: 4, color: field === 'frq' ? 'accent' : 'dim', track: '#28303c' }, { t: 'text', x: (W * 3) / 4 - 8, y: 45, text: frq === 11 ? 'R' : pad(frq, 2), s: 10, align: 'c', bold: true }, { t: 'text', x: (W * 3) / 4 - 8, y: 78, text: 'Frequency', s: 7, align: 'c', color: field === 'frq' ? 'fg' : 'dim' });
  } else {
    const box = (x, label, val, on) => {
      if (on) prims.push({ t: 'rect', x, y: 18, w: 54, h: 40, stroke: 'fg' });
      prims.push({ t: 'text', x: x + 27, y: 22, text: label, s: 9, align: 'c' }, { t: 'text', x: x + 27, y: 36, text: val, s: 14, align: 'c', bold: true });
    };
    box(6, 'INT', pct(int), field === 'int');
    box(68, 'FRQ', frq === 11 ? 'R' : String(frq), field === 'frq');
  }
  prims.push({ t: 'text', x: W / 2, y: opts.rings ? 88 : 54, text: '(effet non simulé)', s: 7, align: 'c', color: 'dim' });
  return prims;
}

function fxTurn(c, steps, mult) {
  if ((c.st.fxField || 'int') === 'int') c.setPanel({ fxInt: clamp(Math.round((c.panel('fxInt', 0.5) + steps * 0.001 * mult) * 1000) / 1000, 0, 1) });
  else c.setPanel({ fxFrq: clamp(c.panel('fxFrq', 5) + steps, 1, 11) });
}

export const aputurePro = {
  id: 'aputure-pro',
  display: { w: 160, h: 100, zoom: 1.6, palette: { bg: '#05070b', fg: '#eef2f7', dim: '#5d6776', accent: '#2f8cff', off: '#020304' } },
  face: {
    style: 'aputure',
    rows: [
      [{ el: 'display' }],
      [
        { el: 'key', id: 'menu', label: 'Menu' },
        { el: 'key', id: 'lighting', label: 'Lighting' },
        { el: 'key', id: 'effects', label: 'Effects' },
        { el: 'key', id: 'back', label: 'Back' },
      ],
      [{ el: 'knob', id: 'select', label: 'SELECT', push: true, size: 'l' }, { el: 'knob', id: 'cct', label: 'CCT', push: false, model: 'aputure.ls-600x-pro' }, { el: 'key', id: 'power', label: '⏻', shape: 'round' }],
    ],
    note: 'SELECT : tourner = intensité (ou défiler), appuyer = valider. Sur l’écran DMX, SELECT maintenu 2 s verrouille l’adresse.',
  },
  init: () => ({ car: 0, fxField: 'int', dmxLock: false, dmxEdit: false }),
  holds: {
    select: [[2, (c) => c.screen === 'dmx' && ((c.st.dmxLock = !c.st.dmxLock), c.say(c.st.dmxLock ? 'Adresse DMX verrouillée' : 'Adresse DMX déverrouillée'))]],
    effects: [[2, (c) => c.screen === 'fx' && ['Lightning'].includes(c.panel('fx')) && c.say('Lightning : cycle (non simulé)')]],
  },
  key(c, id) {
    if (id === 'power') return c.setState({ power: !c.on });
    if (!c.on) return;
    if (c.st.dmxLock && c.screen === 'dmx' && id !== 'select') return c.say('Adresse DMX verrouillée : SELECT 2 s pour déverrouiller');
    if (id === 'lighting') {
      c.menu = null;
      c.screen = 'home';
      c.setPanel({ fxOn: false });
      return;
    }
    if (id === 'menu') {
      c.menu = null;
      c.screen = c.screen === 'carousel' ? 'home' : 'carousel';
      return;
    }
    if (id === 'effects') {
      if (c.screen === 'fx' && ['Lightning', 'Explosion'].includes(c.panel('fx'))) return c.say('Trigger (effet non simulé)');
      c.menu = new MenuState([pick('Effects', { options: FX_LS, get: (cc) => cc.panel('fx', FX_LS[1]), set: (cc, v) => (cc.setPanel({ fx: v, fxOn: true }), (cc.screen = 'fx'), undefined) })], 'Effects');
      c.screen = 'menu';
      c.menu.enter(c);
      c.st.single = true;
      return;
    }
    if (id === 'back') {
      if (c.screen === 'menu') {
        if (!c.menu.back() || c.st.single) {
          c.menu = null;
          c.st.single = false;
          c.screen = 'carousel';
        }
        return;
      }
      if (c.screen === 'dmx') return (c.st.dmxEdit = false), (c.screen = 'carousel');
      c.screen = c.screen === 'carousel' ? 'home' : 'carousel';
      return;
    }
    if (id === 'select') {
      if (c.screen === 'carousel') {
        const items = proTree(c);
        const it = items[((c.st.car % items.length) + items.length) % items.length];
        if (it.children) {
          c.menu = new MenuState(it.children, it.label);
          c.screen = 'menu';
          c.st.single = false;
        } else if (it.kind === 'action') {
          const msg = it.run(c);
          if (msg) c.say(msg);
        } else openPick(c, it);
        return;
      }
      if (c.screen === 'menu') {
        const r = c.menu.enter(c);
        if (r?.msg) c.say(r.msg);
        if (r?.committed && c.st.single) {
          c.menu = null;
          c.st.single = false;
          if (c.screen === 'menu') c.screen = 'carousel';
        }
        return;
      }
      if (c.screen === 'dmx') {
        if (c.st.dmxLock) return c.say('Adresse DMX verrouillée');
        c.st.dmxEdit = !c.st.dmxEdit;
        return;
      }
      if (c.screen === 'fx') c.st.fxField = c.st.fxField === 'frq' ? 'int' : 'frq';
    }
  },
  turn(c, id, steps, mult) {
    if (!c.on) return;
    if (id === 'cct') {
      const [lo, hi] = cctRange(c, [2700, 6500]);
      const v = nudge(c, 'cct', steps, { step: 50 * mult, min: lo, max: hi, round: 1 });
      c.setManual({ mode: 'cct', cct: v });
      return;
    }
    if (c.screen === 'carousel') return (c.st.car += steps);
    if (c.screen === 'menu') return c.menu.move(c, steps, mult);
    if (c.screen === 'dmx') {
      if (!c.st.dmxEdit) return;
      const maxAddr = 512;
      return c.setSettings({ dmxAddress: clamp(c.s.dmxAddress + steps * (mult > 2 ? 10 : 1), 1, maxAddr) });
    }
    if (c.screen === 'fx') return fxTurn(c, steps, mult);
    nudge(c, 'intensity', steps, { step: 0.001 * mult, min: 0, max: 1 });
  },
  draw(c) {
    const W = 160;
    const H = 100;
    if (c.screen === 'carousel') return { prims: proCarousel(c) };
    if (c.screen === 'menu') return { prims: c.menu.edit ? drawEdit(c, c.menu, { w: W, h: H, rows: 4, rowH: 18, size: 10, markers: ['◉ ', '○ '], radio: true, selFill: '#1d3f73' }) : drawList(c, c.menu, { w: W, rows: 4, rowH: 18, size: 10, selFill: '#1d3f73', maxValue: 10 }) };
    if (c.screen === 'dmx') return { prims: dmxScreen(c, W, H, 24) };
    if (c.screen === 'fx') return { prims: fxScreen(c, W, { rings: true }) };
    return { prims: proHome(c) };
  },
  leds: () => ({}),
};

// ================================================================== OLED : LS 600d, LS 300x, LS C300d
const MODELS_OLED = {
  'aputure.ls-600d': { cct: false, fan: ['Smart', 'High', 'Medium', 'Silent'] },
  'aputure.ls-300x': { cct: true, fan: ['FORCE', 'AUTO'] },
  'aputure.c300d': { cct: false, fan: ['FORCE', 'AUTO'] },
};
const oledModel = (c) => MODELS_OLED[c.type.id] || MODELS_OLED['aputure.ls-600d'];
const WIRELESS = ['2.4G', 'BT', 'OFF'];

function oledTree(c) {
  const id = c.type.id;
  const yesNo = (label, key, dflt) => pick(label, { options: ['YES', 'NO'], ...stored(key, dflt) });
  const curve = curveItem(id === 'aputure.ls-600d' ? 'Dimming Curves' : 'Dimming Curve', { exp: 'Exp', log: 'Log', s: 'S-curve', lin: 'Linear' });
  const fan = pick('Fan Mode', { options: oledModel(c).fan, ...stored('fan', oledModel(c).fan[0]) });
  const lang = pick('Language', { options: ['English', '中文'], ...stored('lang', 'English') });
  const btReset = act(id === 'aputure.c300d' ? 'Bluetooth Reset' : 'BT Reset', () => 'Loading… [Reset Failed]');
  if (id === 'aputure.ls-300x')
    return [
      pick('Output Mode', { options: ['Constant Output', 'Max Output'], ...stored('output', 'Max Output') }),
      act('DMX Mode', (cc) => ((cc.screen = 'dmx'), undefined)),
      curve,
      fan,
      yesNo('Studio Mode', 'studio', 'YES'),
      btReset,
      lang,
      act('USB Mode', () => 'Update : pas de clé USB'),
      exitItem,
      info('Ver:1.0', ''),
    ];
  if (id === 'aputure.c300d')
    return [act('DMX Mode', (cc) => ((cc.screen = 'dmx'), undefined)), curve, fan, yesNo('Studio Mode', 'studio', 'YES'), info('Fixture Serial No.', (cc) => serialOf(cc, 6)), btReset, lang, act('Update Firmware', () => 'Pas de clé USB'), info('Firmware Version', 'V1.9'), pick('DMX Loss Behavior', { options: ['Black Out', 'Fade To Black', 'Hold Last Settings'], ...stored('dmxLoss', 'Hold Last Settings') }), exitItem];
  return [
    sub('DMX Mode', [act('DMX Address', (cc) => ((cc.screen = 'dmx'), (cc.menu = null), undefined)), profileItem('DMX Profile', (m) => m.name.replace(/ 8-bit$/i, '')), pick('DMX Loss Behavior', { options: ['Black Out', 'Fade To Black', 'Hold Last Settings', 'Hold 2 min & Fade Out'], ...stored('dmxLoss', 'Black Out') }), exitItem]),
    curve,
    fan,
    yesNo('Studio Mode', 'studio', 'NO'),
    num('Freq.Selection', { min: 0, max: 2000, step: 100, accel: false, fmt: (v) => `+${v}Hz`, ...stored('freq', 0) }),
    btReset,
    info('BT Serial NO', (cc) => `R:${serialOf(cc, 6)}`),
    lang,
    act('Update Firmware', () => 'Pas de clé USB'),
    info('Firmware Version', 'Control Box : 1.2'),
    info('UID', (cc) => `6MH${serialOf(cc, 8)}`),
    factory(),
    exitItem,
  ];
}

const curveGlyph = (c) => ({ square: '⌒', invSquare: '◜', sCurve: 'S', linear: '/' })[c.s.dimmingCurve] || '/';

export const aputureOled = {
  id: 'aputure-oled',
  display: { w: 128, h: 64, palette: { bg: '#000000', fg: '#f4f4f4', dim: '#6a6a6a', accent: '#f4f4f4', off: '#000000' } },
  face: {
    style: 'aputure',
    rows: [
      [{ el: 'display' }],
      [
        { el: 'key', id: 'k1', label: 'LIGHTING / EFFECTS', shape: 'small' },
        { el: 'key', id: 'k2', label: 'CHANNEL / INTENSITY', shape: 'small' },
        { el: 'key', id: 'k3', label: 'GROUP / FREQUENCY', shape: 'small' },
        { el: 'key', id: 'k4', label: 'WIRELESS / TRIGGER', shape: 'small' },
      ],
      [{ el: 'knob', id: 'menu', label: 'MENU · BRIGHTNESS', push: true, size: 'l' }, { el: 'knob', id: 'preset', label: 'PRESET · CCT', push: true, model: 'aputure.ls-300x' }, { el: 'key', id: 'power', label: '⏻', shape: 'round' }],
    ],
    note: 'Légende du haut : mode normal ; du bas : mode effets (EFFECTS maintenu 2 s). Molette : appuyer = MENU, tourner = intensité.',
  },
  init: () => ({ fx: false, fxField: 'int', dmxLock: false }),
  holds: {
    k1: [[2, (c) => ((c.st.fx = !c.st.fx), (c.screen = c.st.fx ? 'fx' : 'home'), c.say(c.st.fx ? 'Mode effets' : 'Mode lumière'))]],
    menu: [[2, (c) => c.type.id === 'aputure.ls-600d' && c.screen === 'dmx' && ((c.st.dmxLock = !c.st.dmxLock), c.say(c.st.dmxLock ? 'DMX verrouillé' : 'DMX déverrouillé'))]],
  },
  key(c, id) {
    if (id === 'power') return c.setState({ power: !c.on });
    if (!c.on) return;
    if (c.st.fx) {
      if (id === 'k1') {
        const i = FX_LS.indexOf(c.panel('fx', FX_LS[1]));
        return c.setPanel({ fx: FX_LS[(i + 1) % FX_LS.length] });
      }
      if (id === 'k2') return (c.st.fxField = 'int');
      if (id === 'k3') return (c.st.fxField = 'frq');
      if (id === 'k4') return c.say('Trigger (effet non simulé)');
      if (id === 'menu') return; // pas de menu en mode effets
    }
    if (id === 'k1') {
      c.menu = null;
      c.screen = 'home';
      return;
    }
    if (id === 'k2') {
      const ch = (c.panel('ch', 1) % 3) + 1;
      c.setPanel({ ch });
      return c.showBig(`CH:${ch}`);
    }
    if (id === 'k3') {
      const gr = 'ABCD'[('ABCD'.indexOf(c.panel('gr', 'A')) + 1) % 4];
      c.setPanel({ gr });
      return c.showBig(`GR:${gr}`);
    }
    if (id === 'k4') {
      const w = WIRELESS[(WIRELESS.indexOf(c.panel('wireless', '2.4G')) + 1) % 3];
      c.setPanel({ wireless: w });
      return c.say(w === 'OFF' ? 'Sans fil désactivé' : `Commande sans fil : ${w === 'BT' ? 'Bluetooth' : '2.4G'}`);
    }
    if (id === 'preset') return c.say('PRESET : fonction non documentée par Aputure');
    if (id === 'menu') {
      if (c.screen === 'dmx') return c.st.dmxLock ? c.say('DMX verrouillé') : ((c.screen = c.menu ? 'menu' : 'home'), undefined);
      if (c.screen !== 'menu') return c.openMenu(oledTree(c), 'System Menu', { wrap: true });
      const r = c.menu.enter(c);
      if (r?.msg) c.say(r.msg);
    }
  },
  turn(c, id, steps, mult) {
    if (!c.on) return;
    if (id === 'preset') {
      const [lo, hi] = cctRange(c, [2700, 6500]);
      if (c.st.fx) return;
      const v = nudge(c, 'cct', steps, { step: 50 * mult, min: lo, max: hi, round: 1 });
      c.setManual({ mode: 'cct' });
      return c.showBig(`${v}K`);
    }
    if (c.st.fx) return fxTurn(c, steps, mult);
    if (c.screen === 'menu') return c.menu.move(c, steps, mult);
    if (c.screen === 'dmx') {
      if (c.st.dmxLock) return c.say('DMX verrouillé');
      return c.setSettings({ dmxAddress: clamp(c.s.dmxAddress + steps * (mult > 2 ? 10 : 1), 1, 512) });
    }
    const v = nudge(c, 'intensity', steps, { step: 0.01 * Math.min(mult, 4), min: 0, max: 1, round: 100 });
    c.showBig(pct(v));
  },
  draw(c) {
    const W = 128;
    const m = c.m;
    if (c.st.fx) return { prims: fxScreen(c, W) };
    if (c.screen === 'menu') return { prims: c.menu.edit ? drawEdit(c, c.menu, { rows: 3, markers: ['■', '□'], radio: true }) : drawList(c, c.menu, { rows: 3 }) };
    if (c.screen === 'dmx') return { prims: dmxScreen(c, W, 64, 18) };
    if (c.flash) return { prims: [{ t: 'text', x: W / 2, y: 18, text: c.flash.text, s: 26, align: 'c', bold: true }] };
    const w = c.panel('wireless', '2.4G');
    const prims = [
      { t: 'text', x: 4, y: 4, text: pct(m.intensity ?? 0), s: 20, bold: true },
      { t: 'line', x1: 74, y1: 2, x2: 74, y2: 36, w: 1 },
      { t: 'text', x: 80, y: 4, text: `AC  ${w === 'OFF' ? '⊘' : w === 'BT' ? 'ᛒ' : '2.4G'}`, s: 9 },
      { t: 'text', x: 80, y: 18, text: '>2h', s: 9 },
      { t: 'line', x1: 2, y1: 38, x2: W - 2, y2: 38, w: 1 },
    ];
    if (oledModel(c).cct) prims.push({ t: 'text', x: 4, y: 44, text: `${Math.round(m.cct || 5600)}K`, s: 12, bold: true });
    prims.push({ t: 'text', x: oledModel(c).cct ? 60 : 6, y: 45, text: `CH:${c.panel('ch', 1)} GR:${c.panel('gr', 'A')}`, s: 9 });
    prims.push({ t: 'text', x: W - 2, y: 43, text: curveGlyph(c), s: 12, align: 'r', bold: true });
    return { prims };
  },
  leds: () => ({}),
};

// ================================================================== Nova P300c
const LIGHT_MODES_NOVA = ['CCT', 'HSI', 'RGB', 'xy', 'SOURCE', 'GEL', 'FX'];

function novaTree() {
  return [
    pick('Output Mode', { options: ['Constant Output', 'Max Output'], ...stored('output', 'Constant Output') }),
    sub('DMX Settings', [act('DMX Address', (c) => ((c.screen = 'dmx'), (c.menu = null), undefined)), profileItem('DMX Profile', (m, i) => `${i + 1}:${m.name}`), pick('DMX Loss Behavior', { options: ['Black Out', 'Fade to Black', 'Hold Last Settings', 'Hold 2 min & Fade Out'], ...stored('dmxLoss', 'Hold Last Settings') })]),
    pick('Fan Mode', { options: ['Smart', 'High', 'Medium', 'Silent'], ...stored('fan', 'Smart') }),
    curveItem('Dimming Curves', { exp: 'Exp', log: 'Log', s: 'S-curve', lin: 'Linear' }),
    num('Freq. Selection', { min: 1, max: 10, accel: false, ...stored('freq', 1) }),
    sub('BT Settings', [pick('BT Status', onOff('bt', 'ON')), act('BT Reset', () => 'Reset Failed'), info('Fixture Serial No.', (c) => serialOf(c, 6))]),
    pick('Studio Mode', { options: ['YES', 'NO'], ...stored('studio', 'NO') }),
    act('Update Firmware', () => 'Pas de clé USB'),
    info('Firmware Version', 'Control Box : 1.9'),
    pick('Language', { options: ['English', '中文'], ...stored('lang', 'English') }),
    factory(),
    sub('Custom FX', [act('Picker FX', () => NOT_SIMULATED), act('Music FX', () => NOT_SIMULATED)]),
    pick('Screensaver', { options: ['OFF', 'ON'], ...stored('screensaver', 'OFF') }),
    exitItem,
  ];
}

/** Case étiquetée (étiquette inversée en haut à gauche). */
function tagCell(prims, x, y, w, h, tag, value, big) {
  prims.push({ t: 'rect', x, y, w, h, stroke: 'dim' });
  prims.push({ t: 'rect', x, y, w: tag.length * 5 + 6, h: 9, fill: 'fg' });
  prims.push({ t: 'text', x: x + 3, y: y + 1, text: tag, s: 7, inv: true, bold: true });
  prims.push({ t: 'text', x: x + w / 2, y: y + (big ? 11 : 12), text: value, s: big ? 15 : 11, align: 'c', bold: true });
}

export const nova = {
  id: 'aputure-nova',
  display: { w: 128, h: 64, palette: { bg: '#000000', fg: '#f2f2f2', dim: '#555555', accent: '#f2f2f2', off: '#000000' } },
  face: {
    style: 'aputure',
    rows: [
      [{ el: 'display' }, { el: 'knob', id: 'int', label: 'INT', push: true, size: 'l' }],
      [{ el: 'knob', id: 'cct', label: 'CCT / HUE' }, { el: 'knob', id: 'gm', label: 'G/M / SAT', push: true }, { col: [{ el: 'key', id: 'mode', label: 'LIGHT MODE', shape: 'small' }, { el: 'key', id: 'menu', label: 'MENU', shape: 'small' }] }],
      [{ el: 'key', id: 'p1', label: '1', shape: 'small' }, { el: 'key', id: 'p2', label: '2', shape: 'small' }, { el: 'key', id: 'p3', label: '3', shape: 'small' }, { el: 'key', id: 'p4', label: '4', shape: 'small' }, { el: 'text', text: 'PRESET' }, { el: 'key', id: 'power', label: '⏻', shape: 'round' }],
    ],
    note: 'Préréglages 1–4 : appui court = rappel, appui long = enregistrer. LIGHT MODE : CCT ou HSI (les autres modes ne sont pas simulés).',
  },
  init: () => ({ modeCur: 0 }),
  holds: Object.fromEntries(
    ['p1', 'p2', 'p3', 'p4'].map((id) => [
      id,
      [
        [
          HOLD_LONG,
          (c) => {
            const presets = { ...c.panel('presets', {}) };
            presets[id] = { mode: c.m.mode, intensity: c.m.intensity, cct: c.m.cct, tint: c.m.tint, hue: c.m.hue, sat: c.m.sat };
            c.setPanel({ presets });
            c.say(`Préréglage ${id.slice(1)} enregistré`);
          },
        ],
      ],
    ]),
  ),
  key(c, id) {
    if (id === 'power') return c.setState({ power: !c.on });
    if (!c.on) return;
    if (/^p\d$/.test(id)) {
      const p = c.panel('presets', {})[id];
      if (!p) return c.say(`Préréglage ${id.slice(1)} vide (appui long pour l’enregistrer)`);
      c.setManual(p);
      return c.say(`Préréglage ${id.slice(1)}`);
    }
    if (id === 'mode') {
      c.menu = null;
      c.screen = c.screen === 'modes' ? 'home' : 'modes';
      c.st.modeCur = Math.max(0, LIGHT_MODES_NOVA.indexOf(c.m.mode === 'hsi' ? 'HSI' : 'CCT'));
      return;
    }
    if (id === 'menu') return c.screen === 'menu' ? c.closeMenu() : c.openMenu(novaTree(), 'System Menu');
    if (id === 'int') {
      if (c.screen === 'modes') {
        const m = LIGHT_MODES_NOVA[c.st.modeCur];
        c.screen = 'home';
        if (m === 'CCT' || m === 'HSI') return c.setManual({ mode: m.toLowerCase() });
        return c.say(`${m} : mode non simulé dans ARKLIGHT`);
      }
      if (c.screen === 'menu') {
        const r = c.menu.enter(c);
        if (r?.msg) c.say(r.msg);
        return;
      }
      if (c.screen === 'dmx') c.screen = c.menu ? 'menu' : 'home';
    }
  },
  turn(c, id, steps, mult) {
    if (!c.on) return;
    if (c.screen === 'modes') return id === 'int' && (c.st.modeCur = clamp(c.st.modeCur + steps, 0, LIGHT_MODES_NOVA.length - 1));
    if (c.screen === 'menu') return id === 'int' && c.menu.move(c, steps, mult);
    if (c.screen === 'dmx') return id === 'int' && c.setSettings({ dmxAddress: clamp(c.s.dmxAddress + steps * (mult > 2 ? 10 : 1), 1, 512) });
    const hsi = c.m.mode === 'hsi';
    if (id === 'int') nudge(c, 'intensity', steps, { step: 0.001 * mult, min: 0, max: 1 });
    else if (id === 'cct') {
      if (hsi) nudge(c, 'hue', steps, { step: mult, min: 0, max: 360, wrap: true, round: 1 });
      else {
        const [lo, hi] = cctRange(c, [2000, 10000]);
        nudge(c, 'cct', steps, { step: 50 * mult, min: lo, max: hi, round: 1 });
      }
    } else if (id === 'gm') {
      if (hsi) nudge(c, 'sat', steps, { step: 0.01 * mult, min: 0, max: 1, round: 100 });
      else nudge(c, 'tint', steps, { step: 0.1, min: -1, max: 1, round: 10 });
    }
  },
  draw(c) {
    const W = 128;
    const m = c.m;
    if (c.screen === 'menu') return { prims: c.menu.edit ? drawEdit(c, c.menu, { rows: 3, markers: ['▪', '▫'], radio: true }) : drawList(c, c.menu, { rows: 3 }) };
    if (c.screen === 'dmx') return { prims: dmxScreen(c, W, 64, 18) };
    if (c.screen === 'modes') {
      const prims = [];
      const i = c.st.modeCur;
      [-1, 0, 1].forEach((d, k) => {
        const name = LIGHT_MODES_NOVA[i + d];
        if (!name) return;
        const y = 4 + k * 20;
        if (d === 0) prims.push({ t: 'rect', x: 24, y, w: 80, h: 18, fill: 'fg' });
        else prims.push({ t: 'rect', x: 30, y: y + 2, w: 68, h: 14, stroke: 'dim' });
        prims.push({ t: 'text', x: W / 2, y: y + 3, text: name, s: d === 0 ? 12 : 9, align: 'c', bold: true, inv: d === 0, color: d === 0 ? undefined : 'dim' });
      });
      return { prims };
    }
    const prims = [];
    const hsi = m.mode === 'hsi';
    tagCell(prims, 1, 1, W - 2, 30, 'INT', `${((m.intensity ?? 0) * 100).toFixed(1)} %`, true);
    if (hsi) {
      tagCell(prims, 1, 33, 62, 30, 'HUE', `${Math.round(m.hue || 0)} °`);
      tagCell(prims, 65, 33, 62, 30, 'SAT', `${Math.round((m.sat ?? 1) * 100)} %`);
    } else {
      tagCell(prims, 1, 33, 62, 30, 'CCT', `${Math.round(m.cct || 5500)} K`);
      tagCell(prims, 65, 33, 62, 30, 'G/M', `${(m.tint || 0) >= 0 ? '+' : ''}${(m.tint || 0).toFixed(1)}`);
    }
    return { prims };
  },
  leds: () => ({}),
};
