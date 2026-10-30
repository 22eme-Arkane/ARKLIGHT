// Version Rust d'ARKLIGHT : données d'essai de parité des menus embarqués (shared/onboard), calculées
// par la version JavaScript. Le portage Rust (rust/crates/arklight-onboard) rejoue les mêmes gestes et
// doit afficher les mêmes écrans et faire les mêmes changements.
//   node tools/rust/export-onboard-data.mjs
// Écrit rust/crates/arklight-onboard/tests/data/onboard.json : une liste de scénarios, chacun avec le
// projecteur de départ (réglages et état par défaut) et des pas (appui, relâchement, molette, image…).
// Après chaque pas : sortie du moteur lumière (verrou d'entrée des Astera), tick, puis relevé de l'écran,
// des voyants, du message, des pixels remplacés et des réglages / état. Pour rester petit, un relevé
// n'est écrit que s'il a changé depuis le pas précédent (réglages et état : les seuls champs changés).
// Aucun projet de l'utilisateur n'est lu ni écrit.
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defaultSettings, defaultState, getFixtureType } from '../../shared/fixtures/index.js';
import { AsteraV5Controller } from '../../shared/onboard/astera-v5.js';
import { AsteraAx1Controller } from '../../shared/onboard/astera-ax1.js';
import { AsteraBoxController } from '../../shared/onboard/astera-box.js';
import { OnboardController } from '../../shared/onboard/engine.js';
import { PanelController } from '../../shared/onboard/panel.js';
import { PANEL_TYPES, panelFor } from '../../shared/onboard/panels/index.js';
import { applyIrKey } from '../../shared/onboard/ir-remote.js';
import { computeOutput } from '../../shared/engine/output.js';

const DIR = fileURLToPath(new URL('../../rust/crates/arklight-onboard/tests/data/', import.meta.url));

const clone = (x) => JSON.parse(JSON.stringify(x ?? null));
const round6 = (v) => Math.round(v * 1e6) / 1e6;

/** Projecteur neuf, comme l'ajoute l'interface. */
function makeFixture(typeId, { id, rotation = [0, 0, 0], settings = {}, state = {} } = {}) {
  const type = getFixtureType(typeId);
  if (!type) throw new Error(`modèle inconnu : ${typeId}`);
  return {
    id: id || `f-${typeId.replace(/[^a-z0-9]/gi, '')}`,
    type: typeId,
    name: type.name,
    position: [0, 1, 0],
    rotation,
    settings: { ...defaultSettings(type), ...clone(settings) },
    state: { ...defaultState(type), ...clone(state) },
  };
}

/** Sans les champs faux, vides ou par défaut (alignement à gauche) : relevés plus petits. */
function compact(v) {
  if (Array.isArray(v)) return v.map(compact);
  if (v && typeof v === 'object') {
    const out = {};
    for (const [k, x] of Object.entries(v)) {
      if (x === false || x === null || x === undefined || (k === 'align' && x === 'l')) continue;
      out[k] = compact(x);
    }
    return out;
  }
  return v;
}

/** Écran, en JSON (les textes des primitives sont des chaînes, comme à l'affichage). */
function screenJson(d) {
  if (d === null || d === undefined) return null;
  const out = clone(d);
  if (Array.isArray(out.prims)) for (const p of out.prims) if (p.t === 'text') p.text = String(p.text ?? '');
  return compact(out);
}

/** Écran dessiné : seulement les primitives changées ({ n: nombre, p: { indice: primitive } }). */
function diffScreen(prev, cur) {
  if (!prev || !cur || !Array.isArray(prev.prims) || !Array.isArray(cur.prims)) return null;
  const out = { n: cur.prims.length, p: {} };
  cur.prims.forEach((p, i) => {
    if (JSON.stringify(p) !== JSON.stringify(prev.prims[i])) out.p[i] = p;
  });
  for (const k of ['off', 'dim']) if (!!prev[k] !== !!cur[k]) out[k] = !!cur[k];
  return out;
}

/** Champs changés d'un objet depuis le relevé précédent (supprimés : null). */
function diffObj(prev, cur) {
  const out = {};
  for (const k of new Set([...Object.keys(prev), ...Object.keys(cur)])) {
    if (JSON.stringify(prev[k]) !== JSON.stringify(cur[k])) out[k] = cur[k] === undefined ? null : clone(cur[k]);
  }
  return out;
}

/** Voyants allumés seulement (null = éteint). */
function ledsJson(l) {
  const out = {};
  for (const [k, v] of Object.entries(l || {})) if (v) out[k] = { color: v.color, blinkHz: v.blinkHz || 0 };
  return out;
}

/** Développe la liste compacte des gestes en pas élémentaires (heure croissante). */
function expand(ops) {
  const steps = [];
  let t = 1;
  const at = (dt) => (t = Math.round((t + dt) * 1e6) / 1e6);
  for (const op of ops) {
    const [kind, ...a] = op.split(' ');
    switch (kind) {
      case 'tap': // appui court
        steps.push({ op: 'press', id: a[0], t: at(0.1) });
        steps.push({ op: 'release', id: a[0], t: at(0.05) });
        break;
      case 'hold': {
        // appui maintenu `a[1]` secondes, images toutes les 0,25 s
        const sec = Number(a[1]);
        steps.push({ op: 'press', id: a[0], t: at(0.1) });
        let held = 0;
        while (held + 0.25 < sec) {
          held += 0.25;
          steps.push({ op: 'tick', t: at(0.25) });
        }
        steps.push({ op: 'release', id: a[0], t: at(sec - held) });
        break;
      }
      case 'press':
        steps.push({ op: 'press', id: a[0], t: at(0.1) });
        break;
      case 'release':
        steps.push({ op: 'release', id: a[0], t: at(0.1) });
        break;
      case 'cancel':
        steps.push({ op: 'cancel', id: a[0], t: at(0.05) });
        break;
      case 'turn': // molette : crans, intervalle (accélération)
        steps.push({ op: 'turn', id: a[0], steps: Number(a[1]), t: at(a[2] !== undefined ? Number(a[2]) : 0.3) });
        break;
      case 'wait': {
        // images toutes les `a[1]` secondes (0,25 par défaut) pendant `a[0]` secondes
        const total = Number(a[0]);
        const dt = a[1] !== undefined ? Number(a[1]) : 0.25;
        let w = 0;
        while (w + 1e-9 < total) {
          const d = Math.min(dt, total - w);
          w += d;
          steps.push({ op: 'tick', t: at(d) });
        }
        break;
      }
      case 'jump': // une seule image après un long délai
        steps.push({ op: 'tick', t: at(Number(a[0])) });
        break;
      case 'shake':
        steps.push({ op: 'shake', t: at(0.1) });
        break;
      case 'slider':
        steps.push({ op: 'slider', zone: a[0], steps: Number(a[1] ?? 1), t: at(0.1) });
        break;
      case 'ir':
        steps.push({ op: 'ir', key: a.join(' '), t: at(0.1) });
        break;
      case 'set': {
        // set state.battery 40 : modification directe (comme l'inspecteur)
        steps.push({ op: 'set', path: a[0], value: JSON.parse(a.slice(1).join(' ')), t: at(0.1) });
        break;
      }
      case 'live': // univers reçus (DMX en direct)
        steps.push({ op: 'live', value: JSON.parse(a.join(' ')), t: at(0.1) });
        break;
      default:
        throw new Error(`geste inconnu : ${op}`);
    }
  }
  return steps;
}

/**
 * Rejoue un scénario dans la version JavaScript et relève l'état après chaque pas.
 * family : 'v5' | 'ax1' | 'box' | 'panel' | 'generic'
 */
function run({ name, family, typeId, fixture: fopts = {}, others: otherDefs = [], live: live0 = [], appConnected = false, ops, controls }) {
  const type = family === 'generic' ? { id: typeId, pixels: 4, controls } : getFixtureType(typeId);
  const f = family === 'generic' ? { id: 'f-generic', type: typeId, name: 'Générique', position: [0, 1, 0], rotation: [0, 0, 0], settings: { universe: 1, dmxAddress: 1, ...clone(fopts.settings) }, state: { power: true, battery: 100, ...clone(fopts.state) } } : makeFixture(typeId, fopts);
  const initial = clone(f);
  const others = otherDefs.map((o) => makeFixture(o.type, o));
  const othersInitial = clone(others);
  const all = [f, ...others];
  const rts = new Map(all.map((x) => [x.id, {}]));
  const rt = rts.get(f.id);
  let live = new Set(live0);
  const env = () => ({ appConnected, dmxLive: (u) => live.has(u) });
  const emit = (ch) => {
    if (ch.settings) Object.assign(f.settings, ch.settings);
    if (ch.state) Object.assign(f.state, ch.state);
  };
  const opts = { type, getFixture: () => f, emit, rt };
  let ctrl;
  if (family === 'v5') ctrl = new AsteraV5Controller(opts);
  else if (family === 'ax1') ctrl = new AsteraAx1Controller(opts);
  else if (family === 'box')
    ctrl = new AsteraBoxController({
      ...opts,
      getAll: () => all,
      emitTo: (id, ch, o = {}) => {
        const other = all.find((x) => x.id === id);
        if (ch.settings) Object.assign(other.settings, ch.settings);
        if (ch.state) Object.assign(other.state, ch.state);
        if (o.relatch) {
          const r = rts.get(id);
          r.latch = null;
          r.justUnlinked = !!o.unlinked;
        }
      },
      env,
    });
  else if (family === 'panel') ctrl = new PanelController({ type, def: panelFor(type), getFixture: () => f, emit, env });
  else ctrl = new OnboardController(opts);

  const steps = expand(ops);
  const prev = {};
  let prevS = {};
  let prevD = null;
  let prevSt = {};
  const changed = (key, v) => {
    const s = JSON.stringify(v);
    if (prev[key] === s) return false;
    prev[key] = s;
    return true;
  };
  for (const step of steps) {
    const t = step.t;
    const e = {};
    switch (step.op) {
      case 'press':
        ctrl.press(step.id, t);
        break;
      case 'release':
        ctrl.release(step.id, t);
        break;
      case 'cancel':
        ctrl.cancel(step.id);
        break;
      case 'turn':
        ctrl.turn(step.id, step.steps, t);
        break;
      case 'shake':
        ctrl.shake();
        break;
      case 'slider':
        e.ok = ctrl.slider(step.zone, t, step.steps);
        break;
      case 'ir': {
        const r = applyIrKey(f, type, rt, step.key, t);
        if (r.changes) emit(r.changes);
        e.ir = { accepted: r.accepted, note: r.note ?? null };
        break;
      }
      case 'set': {
        const [root, key] = step.path.split('.');
        if (root === 'rotation') f.rotation = step.value;
        else f[root][key] = clone(step.value);
        break;
      }
      case 'live':
        live = new Set(step.value);
        break;
      default:
        break;
    }
    // image : moteur lumière (verrou d'entrée), tick, pixels remplacés
    const ctx = { dmx: new Map(), live, time: t };
    for (const x of all) {
      const ty = getFixtureType(x.type);
      if (ty && ty.pixels) computeOutput(x, ty, ctx, new Float32Array(ty.pixels * 3), rts.get(x.id));
    }
    ctrl.tick(t);
    const pixels = type.pixels || 0;
    if (pixels) {
      const out = new Float64Array(pixels * 3);
      const on = ctrl.overridePixels(out, pixels, t);
      const px = on ? Array.from(out, round6) : null;
      if (changed('px', px)) e.px = px;
    }
    const d = family === 'generic' ? { lines: clone(ctrl.displayLines(6)) } : screenJson(ctrl.display ? ctrl.display(t) : null);
    if (changed('d', d)) {
      const dp = diffScreen(prevD, d);
      if (dp && JSON.stringify(dp).length < JSON.stringify(d).length) e.dp = dp;
      else e.d = d;
    }
    prevD = d;
    const l = ledsJson(ctrl.leds ? ctrl.leds(t) : {});
    if (changed('l', l)) e.l = l;
    const h = String((ctrl.hint ? ctrl.hint() : '') || ctrl.message?.text || '');
    if (changed('h', h)) e.h = h;
    // réglages et état : seulement les champs changés
    const ds = diffObj(prevS, f.settings);
    if (Object.keys(ds).length) e.s = ds;
    prevS = clone(f.settings);
    const dst = diffObj(prevSt, f.state);
    if (Object.keys(dst).length) e.st = dst;
    prevSt = clone(f.state);
    const latch = rt.latch ?? null;
    if (changed('latch', latch)) e.latch = latch;
    if (changed('ju', !!rt.justUnlinked)) e.ju = !!rt.justUnlinked;
    if (others.length) {
      const o = Object.fromEntries(others.map((x) => [x.id, { crmxLinked: x.settings.crmxLinked, universe: x.settings.universe, latch: rts.get(x.id).latch ?? null, ju: !!rts.get(x.id).justUnlinked }]));
      if (changed('o', o)) e.o = o;
    }
    if (ctrl.ui && changed('screen', ctrl.ui.screen)) e.screen = ctrl.ui.screen;
    else if (!ctrl.ui && ctrl.screen !== undefined && changed('screen', ctrl.screen)) e.screen = ctrl.screen;
    step.e = e;
  }
  return { name, family, type: typeId, controls: controls ?? null, live: live0, appConnected, fixture: initial, others: othersInitial, steps };
}

// ================================================================== scénarios Astera V5
const V5 = [
  {
    name: 'Titan : anneau, DMX ADDRESS, DMX SETTINGS (PIXELS, PROFIL, STROBE)',
    typeId: 'astera.titan',
    ops: ['tap minus', 'tap plus', 'tap enter', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap enter', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap minus', 'tap enter', 'tap plus', 'tap enter', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap minus', 'tap enter', 'tap plus', 'tap plus', 'tap enter', 'tap menu', 'tap menu', 'tap menu'],
  },
  {
    name: 'Titan : STATIC COLOR, INDEX COLOR, CUSTOM COLOR',
    typeId: 'astera.titan',
    ops: ['tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap plus', 'tap minus', 'tap enter', 'tap enter', 'tap plus', 'tap plus', 'tap plus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap enter', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap enter', 'tap minus', 'tap enter', 'tap menu', 'tap menu', 'tap menu'],
  },
  {
    name: 'Titan : palette (HSI, couleurs fixes, gélatines) et soleil',
    typeId: 'astera.titan',
    ops: ['tap palette', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap minus', 'tap enter', 'tap enter', 'tap palette', 'tap plus', 'tap plus', 'tap minus', 'tap minus', 'tap minus', 'tap palette', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap plus', 'tap minus', 'tap minus', 'tap minus', 'tap palette', 'tap palette', 'tap palette', 'tap menu', 'tap sun', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap plus', 'tap minus', 'tap enter', 'tap plus', 'tap menu'],
  },
  {
    name: 'Titan : répétition des touches + / − (accélérée après 2 s)',
    typeId: 'astera.titan',
    ops: ['tap enter', 'tap enter', 'press plus', 'wait 2.6 0.05', 'release plus', 'press minus', 'wait 0.9 0.05', 'release minus', 'tap enter', 'tap sun', 'press minus', 'wait 2.4 0.05', 'release minus', 'tap palette', 'press plus', 'wait 2.3 0.05', 'release plus', 'tap menu'],
  },
  {
    name: 'Titan : INFO, Radio PIN, RF Region, AX1 Compatibility',
    typeId: 'astera.titan',
    fixture: { id: 'f-titan-rig42' },
    ops: ['tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap menu', 'tap plus', 'tap plus', 'tap menu', 'tap menu', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap menu', 'tap menu'],
  },
  {
    name: 'Titan : marche / arrêt, BlueMode (3 s), fin du BlueMode après 2 min',
    typeId: 'astera.titan',
    ops: ['tap power', 'tap power', 'hold power 1.3', 'hold power 3.3', 'tap power', 'hold power 3.2', 'jump 121', 'tap enter', 'tap power', 'set state.charging true', 'set state.battery 47', 'tap menu', 'set state.battery 100', 'hold power 1.2'],
  },
  {
    name: 'Titan : RESET SETTINGS, UNLINK CRMX, veille de l’écran',
    typeId: 'astera.titan',
    ops: ['tap minus', 'tap enter', 'tap menu', 'tap enter', 'set settings.runtime 12', 'tap enter', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'wait 1.4', 'tap menu', 'jump 31', 'tap menu', 'jump 181', 'tap enter'],
  },
  {
    name: 'Titan : STANDALONE (programme, vitesse, couleurs, set)',
    typeId: 'astera.titan',
    ops: ['tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap enter', 'tap plus', 'tap plus', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap enter', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap enter', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap enter', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap plus', 'tap enter', 'tap menu', 'tap menu'],
  },
  {
    name: 'Titan : INPUT SELECT, demande ACCEPT CONFIG, STANDBY',
    typeId: 'astera.titan',
    ops: ['tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap palette', 'tap menu', 'tap plus', 'tap enter', 'tap minus', 'tap enter', 'set state.acceptConfig true', 'tap enter', 'set state.standby true', 'tap enter', 'tap palette', 'tap menu'],
  },
  {
    name: 'Titan : DMX reçu (CRMX), puis entrée filaire',
    typeId: 'astera.titan',
    live: [1],
    ops: ['tap menu', 'tap menu', 'live []', 'tap menu', 'tap menu', 'set settings.crmxLinked false', 'tap menu', 'tap menu', 'set settings.wired true', 'live [1]', 'tap power', 'hold power 1.2', 'live []', 'tap menu', 'tap menu', 'set settings.dmxFail "EMERGENCY LIGHT"', 'set settings.wired false', 'tap power', 'hold power 1.2', 'set settings.crmxLinked true', 'tap menu', 'tap menu'],
  },
  {
    name: 'Titan : télécommande IR (ARC1 / ARC3) et écran REMOTE CONTROL',
    typeId: 'astera.titan',
    ops: ['ir FIRE', 'ir RED', 'ir MAGENTA', 'ir WARM WHITE', 'ir FADE ON', 'ir SPEED3', 'ir RUN8', 'ir PASTEL', 'ir 3200 K', 'ir CCT+', 'ir CCT-', 'ir 40%', 'ir DIM+', 'ir DIM-', 'ir BLUE MODE 2/2', 'ir BLUE MODE 1/2', 'ir BLUE MODE 2/2', 'tap power', 'ir MAX RUNTIME', 'ir MAX RUNTIME', 'ir POWER OFF', 'ir RED', 'ir POWER ON', 'tap palette', 'tap menu', 'set settings.inputSelect "CRMX wirel. DMX"', 'ir RED', 'ir POWER OFF', 'set settings.inputSelect "APP CONTROL"', 'ir POWER ON', 'ir RAINBOW', 'set state.acceptConfig true', 'ir POWER ON'],
  },
  {
    name: 'Hyperion BTB : UNBOND BLUETOOTH, PIXELS 32',
    typeId: 'astera.hyperion-btb',
    ops: ['tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'wait 1.3', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap enter', 'tap minus', 'tap enter', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap menu', 'tap menu'],
  },
  {
    name: 'AX10 : clavier à double légende (appuis courts et maintenus), RESET NO / YES',
    typeId: 'astera.ax10',
    ops: ['tap menu', 'tap menu', 'hold menu 2.3', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap menu', 'hold minus 2.2', 'tap plus', 'tap plus', 'tap enter', 'hold plus 2.4', 'tap minus', 'tap minus', 'tap enter', 'hold enter 2.1', 'tap plus', 'tap enter', 'press menu', 'release menu', 'tap minus', 'tap enter', 'tap plus', 'tap enter', 'tap enter', 'tap minus', 'tap enter', 'tap plus', 'tap enter', 'set state.charging true', 'tap menu', 'set state.charging false', 'set state.battery 50', 'tap menu', 'set state.battery 10', 'set state.acceptConfig true', 'tap menu', 'tap enter'],
  },
  {
    name: 'AX7 : DMX TABLE, UNPAIR CRMX',
    typeId: 'astera.ax7',
    ops: ['tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap enter', 'tap plus', 'tap enter', 'tap menu', 'tap menu', 'tap plus', 'tap enter', 'wait 1.3', 'tap menu', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus'],
  },
  {
    name: 'QuikPunch : inclinaison (touche ASTERA), OUTPUT GAIN, FAN, FAN CONTROL',
    typeId: 'astera.quikpunch',
    fixture: { rotation: [-0.5235987755982988, 0, 0] },
    ops: ['tap astera', 'tap plus', 'tap astera', 'set rotation [0.3, 0.7, 0]', 'tap astera', 'tap astera', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap menu', 'tap plus', 'tap enter', 'tap plus', 'tap enter', 'tap menu'],
  },
  {
    name: 'HydraPanel : TouchSlider, ordre UNBOND / UNLINK',
    typeId: 'astera.hydrapanel',
    ops: ['slider top', 'tap enter', 'tap enter', 'slider top 4', 'slider bottom 2', 'slider middle', 'tap sun', 'slider bottom 3', 'slider middle', 'slider top 2', 'tap menu', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'wait 1.3'],
  },
  {
    name: 'Pluto : CRMX LINK BY KEY, FIXTURE NUMBER',
    typeId: 'astera.pluto',
    ops: ['tap plus', 'tap plus', 'tap plus', 'tap plus', 'tap enter', 'tap minus', 'tap enter', 'tap plus', 'tap menu', 'tap menu', 'tap plus', 'tap plus', 'tap enter', 'tap minus', 'tap minus', 'tap enter', 'tap plus', 'tap plus', 'tap enter', 'slider top 3', 'slider middle', 'tap menu', 'tap menu'],
  },
];

// ================================================================== bouton unique, AsteraBox, générique
const AX1 = [
  {
    name: 'AX1 : jauge, extinction, allumage, BlueMode, couleurs fixes, adressage par secousse, reset',
    typeId: 'astera.ax1',
    ops: ['tap power', 'wait 2.2', 'hold power 1.6', 'tap power', 'hold power 1.2', 'wait 1.6', 'hold power 3', 'tap power', 'tap power', 'tap power', 'hold power 1.3', 'hold power 3.1', 'tap power', 'hold power 1.2', 'hold power 2.8', 'shake', 'tap power', 'tap power', 'shake', 'tap power', 'hold power 1.2', 'wait 1.3', 'hold power 5.4', 'wait 0.6', 'set state.battery 50', 'tap power', 'set state.battery 10', 'wait 2.2', 'tap power', 'set state.acceptConfig true', 'tap power', 'wait 1.3', 'hold power 1.2', 'set state.charging true', 'wait 1', 'set state.battery 100', 'tap power'],
  },
  {
    name: 'AX3 : deux boutons, jauge couleur, pas de secousse',
    typeId: 'astera.ax3',
    ops: ['tap power-side', 'set state.battery 70', 'tap power', 'set state.battery 10', 'wait 2.2', 'tap power', 'hold power-side 2.7', 'shake', 'tap power', 'hold power 1.2', 'hold power-side 1.2', 'set state.charging true', 'wait 1.2', 'hold power 1.1'],
  },
  { name: 'SolaBulb : appui court = extinction', typeId: 'astera.solabulb', ops: ['tap power', 'tap power', 'hold power 1.2', 'hold power 2.7', 'tap power', 'tap power', 'hold power 1.1'] },
  { name: 'NYX : pas de jauge', typeId: 'astera.nyx', ops: ['tap power', 'hold power 1.4', 'hold power 1.2', 'hold power 2.9', 'hold power 1.3'] },
];

const BOX = [
  {
    name: 'ART7 : jauge, liaison CRMX des Astera de l’univers, déliaison, choix TX / RX',
    typeId: 'astera.art7',
    fixture: { settings: { universe: 3 } },
    others: [
      { type: 'astera.titan', id: 'f-titan-a', settings: { crmxLinked: false, universe: 1 } },
      { type: 'astera.helios', id: 'f-helios-b', settings: { crmxLinked: true, universe: 3 } },
      { type: 'astera.ax1', id: 'f-ax1-c', settings: { crmxLinked: false, inputSelect: 'STANDALONE' } },
      { type: 'astera.hyperion', id: 'f-hyp-d', settings: { crmxLinked: false }, state: { power: false } },
      { type: 'arri.skypanel-s60c', id: 'f-sky-e' },
    ],
    live: [3],
    ops: ['tap power', 'wait 2.2', 'tap link', 'wait 1', 'wait 10', 'hold link 3.2', 'wait 4.2', 'tap link', 'tap link', 'tap link', 'tap link', 'hold link 3.3', 'tap link', 'wait 0.7', 'hold link 3.1', 'tap link', 'hold link 3.2', 'tap power', 'set state.dmxCable false', 'tap power', 'hold power 1.3', 'set state.charging true', 'set state.battery 55', 'tap power', 'hold power 1.2'],
  },
];

const GENERIC = [
  {
    name: 'Commandes génériques : actions, maintiens, menu (nombre, liste, booléen, sous-menu)',
    typeId: 'test.generic',
    controls: {
      buttons: [
        { id: 'power', label: '⏻' },
        { id: 'menu', label: 'MENU' },
        { id: 'up', label: '▲' },
        { id: 'down', label: '▼' },
        { id: 'back', label: 'BACK' },
      ],
      leds: [{ id: 'status', label: 'STATUS' }],
      display: { width: 128, height: 64 },
      holdTimes: { long: 1, hold3: 2.5 },
      actions: {
        power: { short: 'batteryGauge', long: 'togglePower', hold3: 'blueMode', releaseAfterHold: 'identify' },
        menu: { short: 'menuEnter', long: 'menuBack', hold5: 'reboot' },
        up: { short: 'menuUp' },
        down: { short: 'menuDown' },
        back: { short: 'menuBack', long: 'powerOff' },
      },
      menu: [
        { label: 'DMX', children: [{ label: 'Address', setting: 'dmxAddress', type: 'number', min: 1, max: 512, wrap: true }, { label: 'Curve', setting: 'dimmingCurve', type: 'enum', options: [['linear', 'Linear'], ['square', 'Square'], { value: 'sCurve', label: 'S' }] }] },
        { label: 'Fan', setting: 'fanQuiet', type: 'bool' },
        { label: 'Level', setting: 'level', type: 'number', min: 0, max: 10, step: 5, unit: '%' },
        { label: 'Identify', action: 'identify' },
        { label: 'Reboot', action: 'reboot' },
      ],
    },
    fixture: { settings: { dimmingCurve: 'linear', fanQuiet: false, level: 5 } },
    ops: ['tap power', 'wait 3.2', 'hold power 1.2', 'hold power 1.3', 'hold power 2.8', 'wait 1', 'tap menu', 'tap down', 'tap menu', 'tap up', 'tap up', 'tap menu', 'tap menu', 'tap up', 'tap menu', 'tap menu', 'tap down', 'tap down', 'tap menu', 'tap menu', 'tap up', 'tap up', 'tap menu', 'tap down', 'tap menu', 'tap down', 'tap down', 'tap menu', 'tap down', 'tap menu', 'tap down', 'tap menu', 'wait 4.5', 'hold menu 1.2', 'tap back', 'tap back', 'hold back 1.2', 'hold menu 5.2'],
  },
];

// ================================================================== panneaux des marques
const rep = (n, op) => Array(n).fill(op);
const PANELS = [
  {
    name: 'SkyPanel S60-C : menu DMX, Light Mode, modes spéciaux, Art-Net, informations, usine',
    typeId: 'arri.skypanel-s60c',
    fixture: { id: 'f-sky-60' },
    ops: ['tap menu', 'tap is', 'tap is', 'turn is 4', 'tap is', 'turn is 1', 'tap is', 'turn is 2', 'tap is', 'turn is 1', 'tap is', 'turn is 2', 'tap is', 'turn is 1', 'tap is', 'turn is 1', 'tap is', 'turn is 3', 'turn is 1', 'tap back', 'tap back', 'turn is 1', 'tap is', 'turn is 1', 'tap is', 'turn is 1', 'tap is', 'turn is 2', 'tap is', 'tap is', 'turn is -2', 'tap is', 'turn is 1', 'tap is', 'tap is', 'turn is 1', 'tap is', 'turn is 1', 'tap is', 'tap is', 'turn is -1', 'tap is', 'turn is 1', 'tap is', 'turn is -1', 'tap is', 'turn is 1', 'tap is', 'turn is 1', 'tap is', 'tap back', 'turn is 1', 'tap is', 'turn is 5', 'tap is', 'tap back', 'tap back', 'turn is 1', 'tap is', 'turn is 1', 'tap is', 'turn is -1', 'tap is', 'tap is', 'turn is 1', 'tap is', 'turn is 2', 'tap is', 'tap back', 'turn is 1', 'tap is', 'tap is', 'tap back', 'turn is 1', 'tap is', 'tap is', 'turn is 1', 'tap is', 'tap is', 'turn is 3', 'tap is', 'turn is 1', 'tap is', 'turn is 2', 'tap is', 'turn is 1', 'tap is', 'turn is 3', 'tap is', 'tap back', 'turn is 1', 'tap is', 'turn is 4', 'turn is 1', 'tap back', 'turn is 1', 'tap is', 'turn is 5', 'tap back', 'turn is 1', 'tap is', 'tap is', 'turn is 1', 'tap is', 'tap back', 'turn is 1', 'tap is', 'turn is 1', 'tap is', 'tap menu', 'tap menu', 'turn is 12', 'tap is', 'turn is 1', 'tap is', 'tap menu'],
  },
  {
    name: 'SkyPanel S60-C : molettes (accélération), MODE, préréglages, verrou, DMX en direct',
    typeId: 'arri.skypanel-s60c',
    ops: ['turn is 5', 'turn is 5 0.03', 'turn is 5 0.05', 'turn is -3 0.1', 'turn center 3', 'turn center 2 0.03', 'turn right 4', 'turn right -6 0.06', 'wait 1.6', 'tap mode', 'turn center 7', 'turn center -400 0.02', 'turn right -5', 'wait 1.6', 'hold mode 1.2', 'tap preset', 'turn is 4', 'tap is', 'tap preset', 'turn is 12', 'tap is', 'hold preset 1.3', 'turn is 2', 'tap is', 'tap preset', 'turn is 12', 'tap is', 'tap preset', 'turn is 25', 'tap back', 'hold is 5.2', 'tap menu', 'turn center 2', 'hold is 5.1', 'live [1]', 'tap mode', 'live []', 'tap menu', 'turn is 4', 'tap is', 'tap is', 'turn is 1', 'tap is', 'tap menu', 'jump 11'],
  },
  {
    name: 'SkyPanel S360-C : ventilation, CRMX',
    typeId: 'arri.skypanel-s360c',
    ops: ['tap menu', 'tap is', 'turn is 7', 'tap is', 'tap is', 'turn is 1', 'tap is', 'turn is 1', 'tap is', 'tap back', 'tap back', 'turn is 1', 'tap is', 'turn is 2', 'tap is', 'turn is 8', 'tap is', 'turn is 3', 'tap is', 'tap back', 'turn is 1', 'tap is', 'tap back', 'tap back', 'tap power'],
  },
  {
    name: 'L10-C : mémoires, STATUS, SETUP › DMX, JUMP, usine, molettes',
    typeId: 'arri.l10-c',
    ops: ['tap enter', 'tap mem1', 'tap mem2', 'tap enter', 'wait 1', 'tap mem2', 'tap minus', 'tap enter', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap minus', 'tap funct', 'tap minus', 'tap enter', 'tap enter', 'tap enter', 'tap minus', 'tap enter', 'wait 0.6', 'tap minus', 'tap enter', 'tap minus', 'tap enter', 'tap minus', 'tap enter', 'tap plus', 'tap plus', 'tap enter', 'tap funct', 'tap minus', 'tap minus', 'tap enter', 'tap minus', 'tap minus', 'tap enter', 'tap minus', 'tap enter', 'tap funct', 'tap funct', 'tap plus', 'hold enter 1.7', 'tap plus', 'tap minus', 'tap enter', 'hold enter 1.6', 'tap minus', 'tap minus', 'tap enter', 'hold enter 1.6', 'tap funct', 'turn intensity 5', 'turn cct 3', 'turn gn -4', 'wait 2.4', 'tap funct', 'turn cct 4', 'turn gn -3 0.05', 'wait 2.2', 'tap plus', 'tap plus', 'tap enter', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap minus', 'tap enter', 'tap funct', 'tap funct', 'tap power'],
  },
  {
    name: 'L5-C : VBAT, indicateurs éteints',
    typeId: 'arri.l5-c',
    live: [1],
    ops: ['tap plus', 'tap plus', 'tap plus', 'tap enter', ...rep(6, 'tap minus'), 'tap funct', 'tap minus', 'tap enter', 'tap minus', 'tap minus', 'tap minus', 'tap enter', 'tap minus', 'tap enter', 'tap funct', 'tap funct'],
  },
  {
    name: 'BroadCaster 2 Plus : roue codeuse, modes secours',
    typeId: 'arri.broadcaster-2-plus',
    live: [1],
    ops: [...rep(9, 'tap d2+'), ...rep(3, 'tap d0+'), 'tap d0+', 'tap d0+', 'tap d0+', 'tap d1-', 'tap d2-', 'tap d2-', 'tap d0-', 'set state.power false', 'tap d1+', 'live []', 'set state.power true', 'tap d2-'],
  },
  {
    name: 'Aputure LS 600x Pro : carrousel, sous-menus, choix directs, écran DMX, verrou, effets, CCT',
    typeId: 'aputure.ls-600x-pro',
    fixture: { id: 'f-600x-07' },
    ops: ['turn select 20', 'turn select -4 0.03', 'tap menu', 'tap select', 'turn select 2', 'tap select', 'tap back', 'turn select 1', 'tap select', 'tap select', 'tap select', 'turn select 3', 'tap select', 'tap back', 'tap select', 'turn select 3', 'tap back', 'tap back', 'turn select 2', 'tap select', 'turn select 1', 'tap select', 'turn select 1', 'tap select', 'turn select 1', 'tap select', 'turn select 3', 'tap select', 'turn select 4', 'tap select', 'tap back', 'tap back', 'turn select -3', 'tap select', 'tap select', 'tap select', 'turn select 5', 'turn select 5 0.03', 'tap select', 'hold select 2.2', 'tap menu', 'turn select 3', 'hold select 2.1', 'tap back', 'turn select 1', 'tap select', 'turn select 1', 'tap select', 'tap select', 'tap select', 'tap back', 'tap back', 'turn select 5', 'tap select', 'tap select', 'tap select', 'tap select', 'tap effects', 'turn select 3', 'tap select', 'turn select 30', 'turn select 3 0.03', 'tap select', 'turn select 4', 'turn select 9', 'tap effects', 'turn select 2', 'tap select', 'tap effects', 'hold effects 2.1', 'tap lighting', 'turn cct 3', 'turn cct 9 0.03', 'turn select 50', 'tap power', 'tap menu', 'tap power'],
  },
  {
    name: 'Aputure LS 1200d Pro : informations, Lead-Follow, usine',
    typeId: 'aputure.ls-1200d-pro',
    fixture: { id: 'f-1200d-xy' },
    ops: ['tap menu', 'turn select -2', 'tap select', 'tap select', 'turn select 1', 'tap select', 'tap back', 'turn select -1', 'tap select', 'turn select 1', 'tap select', 'turn select -2', 'tap select', 'turn select 1', 'tap select', 'tap back', 'tap back', 'turn select 1', 'tap select', 'tap select', 'tap back', 'tap back', 'turn select 2', 'tap select', 'turn select 2', 'tap select', 'turn select -5', 'tap select', 'turn select 3', 'tap select', 'tap back', 'tap back', 'tap back'],
  },
  {
    name: 'Aputure LS 600d Pro : menu court',
    typeId: 'aputure.ls-600d-pro',
    ops: ['tap menu', 'tap select', 'turn select 1', 'tap select', 'turn select 1', 'tap select', 'tap back', 'turn select 4', 'tap select', 'tap select', 'tap back', 'tap back', 'tap back', 'tap back'],
  },
  {
    name: 'Aputure LS 600d : menu système, écran DMX verrouillé, CH / GR / sans fil, mode effets',
    typeId: 'aputure.ls-600d',
    fixture: { id: 'f-600d-77' },
    ops: ['turn menu 5', 'turn menu 3 0.03', 'tap k2', 'tap k2', 'tap k3', 'tap k4', 'tap k4', 'tap k4', 'wait 1.6', 'tap menu', 'tap menu', 'turn menu 1', 'tap menu', 'turn menu 2', 'tap menu', 'turn menu 1', 'tap menu', 'turn menu 1', 'tap menu', 'turn menu 3', 'tap menu', 'turn menu -1', 'tap menu', 'tap menu', 'hold menu 2.2', 'turn menu 3', 'tap menu', 'hold menu 2.1', 'turn menu 3', 'turn menu 3 0.03', 'tap menu', 'tap menu', 'turn menu -1', 'tap menu', 'turn menu 1', 'tap menu', 'turn menu 4', 'tap menu', 'turn menu 1', 'tap menu', 'turn menu 2', 'tap menu', 'turn menu 1', 'tap menu', 'turn menu 1', 'tap menu', 'turn menu 20', 'tap menu', 'tap k1', 'hold k1 2.2', 'tap k1', 'tap k1', 'turn menu 4', 'tap k3', 'turn menu 3', 'tap k4', 'tap menu', 'hold k1 2.1', 'tap power', 'turn menu 2'],
  },
  {
    name: 'Aputure LS 300x : molette CCT, menu court, sortie',
    typeId: 'aputure.ls-300x',
    ops: ['turn preset 3', 'turn preset -2 0.05', 'wait 1.6', 'tap preset', 'tap menu', 'turn menu 1', 'tap menu', 'tap menu', 'turn menu 3', 'tap menu', 'tap menu', 'turn menu 7', 'tap menu', 'tap menu', 'turn menu 8', 'tap menu', 'tap menu', 'turn menu 1', 'tap menu'],
  },
  {
    name: 'Aputure LS C300d : numéro de série, perte DMX',
    typeId: 'aputure.c300d',
    fixture: { id: 'f-c300-b2' },
    ops: ['tap menu', 'turn menu 4', 'turn menu 4', 'tap menu', 'turn menu 1', 'tap menu', 'turn menu 1', 'tap menu', 'tap menu'],
  },
  {
    name: 'Aputure Nova P300c : modes de lumière, préréglages, menu, molettes',
    typeId: 'aputure.nova-p300c',
    fixture: { id: 'f-nova-3' },
    ops: ['tap p1', 'hold p1 1.2', 'turn int 30', 'turn cct 4', 'turn gm 3', 'tap p1', 'tap mode', 'turn int 1', 'tap int', 'turn cct 30', 'turn cct -400 0.02', 'turn gm -20', 'hold p2 1.3', 'tap mode', 'turn int 3', 'tap int', 'tap mode', 'turn int -5', 'tap int', 'tap p2', 'tap menu', 'turn int 1', 'tap int', 'tap int', 'turn int 1', 'tap int', 'turn int 1', 'tap int', 'tap menu', 'tap menu', 'turn int 1', 'tap int', 'tap int', 'turn int 3', 'tap int', 'tap menu', 'tap menu', 'turn int 13', 'tap int', 'tap power', 'tap mode', 'tap power'],
  },
];

const PANELS2 = [
  {
    name: 'Hive Hornet 200-C : S.H.O.T., fin / grossier, menus local et DMX, verrou, interrupteur DMX',
    typeId: 'hive.hornet-200-c',
    ops: ['turn adjust 2', 'turn select -2', 'turn adjust 3', 'turn select 1', 'turn adjust 5', 'turn adjust 40', 'tap adjust', 'turn adjust -3', 'turn select 5', 'turn adjust 2', 'tap adjust', 'turn adjust -4', 'tap select', 'tap select', 'turn select 1', 'tap select', 'turn select 3', 'tap select', 'turn select 3', 'tap select', 'tap select', 'tap adjust', 'turn select 1', 'tap select', 'tap select', 'tap adjust', 'tap adjust', 'tap adjust', 'tap adjust', 'hold adjust 4.2', 'turn adjust 1', 'tap select', 'hold adjust 4.1', 'tap dmxsw', 'turn select 4', 'turn adjust 3', 'press adjust', 'turn adjust 2', 'cancel adjust', 'tap select', 'tap select', 'tap select', 'turn select 1', 'tap select', 'tap select', 'turn select -1', 'tap select', 'turn select 1', 'tap select', 'turn select 1', 'tap select', 'turn select 1', 'tap select', 'tap adjust', 'turn select 2', 'tap select', 'tap adjust', 'tap adjust', 'live [1]', 'turn select -1', 'live []', 'tap dmxsw', 'turn select -1', 'turn adjust 2', 'tap power', 'turn adjust 2', 'tap power'],
  },
  {
    name: 'Hive Wasp 100-C : retour en Local avec C ou A choisi (ADJUST règle alors K ou D)',
    typeId: 'hive.wasp-100-c',
    ops: ['tap dmxsw', 'turn select 4', 'tap dmxsw', 'turn adjust -2', 'tap dmxsw', 'turn select 1', 'tap dmxsw', 'turn adjust 2', 'tap adjust', 'turn adjust -3', 'turn select 1'],
  },
  {
    name: 'Hive Super Hornet 575-C : barres S.H.O.T., menu du ballast',
    typeId: 'hive.super-hornet-575-c',
    ops: ['turn select -1', 'turn adjust 3', 'turn adjust 2 0.03', 'turn select -2', 'turn adjust 2', 'turn select 1', 'turn adjust 7', 'tap menu', 'turn select 1', 'tap menu', 'turn adjust 3', 'tap menu', 'turn select 3', 'tap menu', 'turn select 3', 'tap menu', 'turn adjust 5', 'turn adjust 5 0.03', 'tap menu', 'turn select -1', 'tap menu', 'turn select 2', 'tap menu', 'tap back', 'tap back', 'turn select 1', 'tap menu', 'tap back', 'tap back', 'tap back', 'tap power', 'turn select 1', 'tap power'],
  },
  {
    name: 'DMG SL1 MIX : WHITE / COLOR / GEL, préréglages rapides, myMIX, bibliothèque, réglages, DMX, verrou',
    typeId: 'dmg-lumiere.sl1-mix',
    fixture: { id: 'f-sl1-mix9' },
    ops: ['turn knob 50', 'turn knob 5 0.03', 'tap nav', 'turn knob 4', 'turn knob 4 0.03', 'tap nav', 'turn knob -3', 'tap knob', 'tap nav', 'tap knob', 'tap knob', 'tap color', 'tap nav', 'tap nav', 'turn knob 30', 'turn knob 5 0.05', 'tap nav', 'turn knob -20', 'tap knob', 'tap gel', 'turn knob 3', 'tap nav', 'turn knob 1', 'turn knob 1', 'turn knob -1', 'tap nav', 'turn knob 3', 'tap nav', 'turn knob -25', 'hold library 1.2', 'tap library', 'tap nav', 'turn knob 3', 'tap knob', 'tap library', 'tap nav', 'tap nav', 'turn knob 2', 'tap knob', 'tap library', 'tap nav', 'tap nav', 'tap nav', 'turn knob 5', 'tap nav', 'tap nav', 'tap knob', 'hold library 1.1', 'tap library', 'hold library 1.1', 'tap knob', 'tap settings', 'turn knob 1', 'tap knob', 'turn knob 5', 'turn knob 3 0.03', 'tap nav', 'turn knob 2', 'tap nav', 'turn knob 1', 'tap nav', 'turn knob 2', 'tap nav', 'tap knob', 'live [1]', 'tap back', 'live []', 'tap settings', 'turn knob 3', 'tap knob', 'turn knob 2', 'tap knob', 'turn knob 3', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 1', 'tap knob', 'tap back', 'turn knob 1', 'tap knob', 'turn knob 5', 'tap back', 'turn knob 1', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 1', 'tap knob', 'tap back', 'turn knob 1', 'tap knob', 'turn knob 3', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob -1', 'tap knob', 'tap knob', 'tap settings', 'tap white', 'hold back 1.2', 'tap white', 'turn knob 2', 'hold back 1.2', 'tap power', 'tap power', 'hold power 1.2', 'tap power', 'tap settings', 'turn knob 12', 'tap knob', 'tap knob'],
  },
  {
    name: 'DMG MAXI MIX : zones, informations',
    typeId: 'dmg-lumiere.maxi-mix',
    fixture: { id: 'f-maxi-01' },
    ops: ['tap settings', 'turn knob 4', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 4', 'tap knob', 'tap back', 'tap back'],
  },
  {
    name: 'Kino Flo Celeb 450 : DIM / KELVIN / G/M, fin / grossier, préréglages, LOCK, menus TrueMatch',
    typeId: 'kino-flo.celeb-450-led-dmx',
    fixture: { id: 'f-celeb-450' },
    ops: ['turn knob 5', 'tap knob', 'turn knob 3', 'tap mode', 'turn knob 2', 'tap knob', 'turn knob -3', 'tap mode', 'turn knob 4', 'turn knob -9', 'tap p2', 'hold p3 3.2', 'wait 0.5', 'tap p3', 'tap lock', 'turn knob 1', 'tap menu', 'tap lock', 'hold lock 3.2', 'tap menu', 'tap knob', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 2', 'tap knob', 'turn knob 3', 'tap knob', 'turn knob 5', 'tap knob', 'tap mode', 'turn knob 3', 'tap mode', 'turn knob -2', 'tap mode', 'tap mode', 'tap mode', 'turn knob 4', 'tap mode', 'turn knob 1', 'tap menu', 'turn knob 1', 'tap knob', 'turn knob 2', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 2', 'hold mode 1.2', 'turn knob 1', 'tap knob', 'tap knob', 'turn knob -1', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 3', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 1', 'tap knob', 'hold mode 1.1', 'turn knob 1', 'tap knob', 'tap knob', 'turn knob 1', 'tap knob', 'turn knob 1', 'tap knob', 'hold mode 1.1', 'turn knob 1', 'tap knob', 'turn knob 1', 'tap knob', 'tap menu', 'tap p1', 'live [1]', 'tap power', 'hold menu 3.1', 'tap power'],
  },
  {
    name: 'Kino Flo Celeb 250 : titre du menu, adresse DMX',
    typeId: 'kino-flo.celeb-250-led-dmx',
    ops: ['tap menu', 'turn knob 2', 'tap knob', 'tap knob', 'turn knob 4', 'tap knob', 'tap mode', 'tap menu'],
  },
  {
    name: 'Kino Flo Celeb 201 : Kelvin / DIM, préréglages, adresse DMX, LOCK / RESET',
    typeId: 'kino-flo.celeb-led-201-dmx',
    ops: ['turn knob 3', 'tap knob', 'turn knob 5', 'tap kd', 'turn knob 10', 'tap knob', 'turn knob 2', 'tap k1', 'tap kd', 'hold k3 5.2', 'wait 0.6', 'tap k3', 'hold dmx 3.2', 'turn knob 3', 'tap knob', 'turn knob 2', 'wait 0.5', 'tap dmx', 'tap dmx', 'tap lock', 'turn knob 1', 'tap k0', 'tap lock', 'hold lock 5.2', 'tap power', 'turn knob 1', 'tap power', 'live [1]', 'tap k2'],
  },
  {
    name: 'LiteGear LiteDimmer Plus DUO : sélecteur LOCAL / MENU / DMX, écrans du menu, CUE',
    typeId: 'litegear.litemat-plus-2',
    ops: ['turn l1 5', 'turn k1 -4', 'turn l2 3', 'tap center', 'tap center', 'tap link', 'tap menu', 'tap center', 'tap right', 'tap center', 'tap right', 'tap center', 'tap right', 'tap right', 'tap left', 'tap center', 'tap right', 'tap center', 'tap right', 'tap center', 'tap right', 'tap center', 'tap left', 'tap center', 'tap right', 'tap center', 'tap right', 'tap right', 'tap right', 'tap left', 'tap center', 'tap left', 'tap left', 'tap left', 'tap center', 'tap right', 'tap center', 'tap center', 'tap center', 'turn l1 1', 'tap dmxsw', 'turn k1 1', 'live [1]', 'tap local', 'turn l2 3', 'turn k2 2', 'tap menu', 'tap left', 'tap left'],
  },
  {
    name: 'LiteGear LiteDimmer Pro Hybrid : interrupteur ON / OFF / ALT, programmation du mode ALT',
    typeId: 'litegear.s2-litemat-2',
    ops: ['turn level 20', 'turn kelvin -5', 'tap prog', 'hold prog 2.2', 'tap off', 'hold prog 2.2', 'turn level 1', 'turn level 1', 'turn level -1', 'hold prog 2.1', 'tap alt', 'turn level 40', 'tap on', 'turn level 5', 'tap off', 'hold prog 2.2', 'turn level 3', 'hold prog 2.2', 'tap alt'],
  },
  {
    name: 'Dedolight DLED4-BI : adresse (↓ + ↑), mode manuel, molettes, retour en DMX',
    typeId: 'dedolight.dled4-bi',
    live: [1],
    ops: ['turn dim 5', 'tap up', 'tap both', 'tap up', 'tap up', 'tap down', 'wait 6.5', 'tap both', 'tap down', 'tap down', 'tap down', 'turn dim 5', 'turn color 3', 'turn color -40', 'turn dim 120', 'tap both', 'tap down', 'tap both', 'tap up', 'tap down', 'press down', 'press up', 'release down', 'release up', 'tap up', 'tap power', 'tap up', 'tap power', 'live []', 'tap down', 'jump 7', 'tap down'],
  },
];

// ================================================================== faces avant des marques
/** Face avant d'un modèle, sous la forme de l'API Rust (éléments propres à d'autres modèles retirés). */
function faceJson(type) {
  const def = panelFor(type);
  const el = (it) => {
    if (it.model && it.model !== type.id) return null;
    if (it.col) return { el: 'col', items: it.col.map(el).filter(Boolean), row: !!it.row };
    if (it.el === 'display') return { el: 'display' };
    if (it.el === 'led') return { el: 'led', id: it.id, label: it.label || '', title: it.title ?? null };
    if (it.el === 'key') return { el: 'key', id: it.id, label: it.label || '', shape: it.shape || '', title: it.title ?? null, alt: it.alt ?? null };
    if (it.el === 'knob') return { el: 'knob', id: it.id, label: it.label || '', push: !!it.push, pointer: !!it.pointer, cap: it.cap || '', size: it.size || 'm' };
    if (it.el === 'text') return { el: 'text', text: it.text };
    return { el: 'gap' };
  };
  const d = def.display;
  return {
    type: type.id,
    id: def.id,
    face: { style: def.face.style || 'generic', rows: def.face.rows.map((r) => r.map(el).filter(Boolean)), note: def.face.note || '', display: d ? { w: d.w, h: d.h, zoom: d.zoom || 2, palette: d.palette } : null },
  };
}
const faces = PANEL_TYPES.map((id) => faceJson(getFixtureType(id)));

// ================================================================== export
const scenarios = [...V5.map((s) => run({ ...s, family: 'v5' })), ...AX1.map((s) => run({ ...s, family: 'ax1' })), ...BOX.map((s) => run({ ...s, family: 'box' })), ...GENERIC.map((s) => run({ ...s, family: 'generic' })), ...PANELS.map((s) => run({ ...s, family: 'panel' })), ...PANELS2.map((s) => run({ ...s, family: 'panel' }))];

mkdirSync(DIR, { recursive: true });
const json = JSON.stringify({ source: 'ARKLIGHT (JavaScript) shared/onboard', panelTypes: PANEL_TYPES, faces, scenarios });
writeFileSync(`${DIR}onboard.json`, json);
const steps = scenarios.reduce((n, s) => n + s.steps.length, 0);
console.log(`onboard.json : ${scenarios.length} scénarios, ${steps} pas, ${(json.length / 1024).toFixed(0)} Kio`);
