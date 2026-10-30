// Tests des profils DMX Astera, du moteur lumineux V5 et des menus embarqués.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { defaultSettings, defaultState, getFixtureType, getMode, modeFootprint } from '../shared/fixtures/index.js';
import { readAttributes } from '../shared/engine/dmx.js';
import { computeOutput } from '../shared/engine/output.js';
import { AsteraV5Controller, profileBaseName } from '../shared/onboard/astera-v5.js';

const titan = getFixtureType('astera.titan');
const fixture = (type = titan) => ({ id: 'ftest', settings: defaultSettings(type), state: defaultState(type) });

test('réglages par défaut : chaque projecteur a sa copie (standalone non partagé)', () => {
  const a = defaultSettings(titan);
  const b = defaultSettings(titan);
  assert.notEqual(a.standalone, b.standalone);
  a.standalone.colors[0].hue = 123;
  assert.notEqual(b.standalone.colors[0].hue, 123);
  assert.notEqual(defaultSettings(titan).standalone.colors[0].hue, 123, 'la bibliothèque reste intacte');
});

test('profils Titan : empreintes et canaux 16 bits', () => {
  assert.equal(titan.dmx.modes.length, 143);
  assert.equal(modeFootprint(titan, '42'), 48); // RGB RGB, 16 pixels
  assert.equal(modeFootprint(titan, '91'), 8); // D16 CCT GM C RGB
  assert.equal(modeFootprint(titan, '25'), 15); // RGB.RGBS, 4 pixels
  const m25 = getMode(titan, '25');
  assert.equal(m25.channels[3].f, 'strobe');
  assert.equal(m25.channels[3].px, -1, 'le canal 4 est le strobe de tout le tube');
});

test('sémantique Astera : Kelvin, vert/magenta, Kelvin prioritaire, couleur index', () => {
  const data = new Uint8Array(512);
  data.set([255, 45, 128, 0, 10, 20, 30]); // mode 89 : Dim, CCT, GM, CRO, R, G, B
  const a = readAttributes(getMode(titan, '89'), data, 1, 1).fixture;
  assert.equal(a.cct, 3190);
  assert.ok(Math.abs(a.tint) < 1e-9);
  assert.equal(a.crossfade, 0);
  data.fill(0);
  data.set([0, 0, 0, 3, 255, 106]); // mode 7 : RGB, CCT (sans effet), Dim, Index 106
  const b = readAttributes(getMode(titan, '7'), data, 1, 1).fixture;
  assert.equal(b.cctIndex, undefined, 'valeur ≤ 4 : Kelvin sans effet');
  assert.equal(b.index, 106);
});

test('moteur : verrouillage AUTO sur CRMX, DMX FAIL, bridage par l’autonomie', () => {
  const f = fixture();
  const out = new Float32Array(16 * 3);
  const rt = {};
  const data = new Uint8Array(512);
  data.set([255, 255, 255]); // mode 1 : RGB blanc
  computeOutput(f, titan, { dmx: new Map([[1, data]]), live: new Set([1]), time: 0 }, out, rt);
  assert.equal(rt.input, 'CRMX');
  const full = Math.max(out[0], out[1], out[2]);
  assert.ok(Math.abs(full - 0.3) < 0.01, 'autonomie 5 h : sortie bridée à 30 %');
  // perte du signal avec DMX FAIL = EMERGENCY LIGHT
  f.settings.dmxFail = 'EMERGENCY LIGHT';
  const src = computeOutput(f, titan, { dmx: new Map([[1, data]]), live: new Set(), time: 1 }, out, rt);
  assert.equal(src, 'emergency');
  // extinction : le verrou est relâché
  f.state.power = false;
  computeOutput(f, titan, { dmx: new Map(), live: new Set(), time: 2 }, out, rt);
  assert.equal(rt.latch, null);
});

test('nom de base des profils pour changer PIXELS / STROBE', () => {
  assert.equal(profileBaseName('DIM RGBWS DIM RGBWS'), 'DIM RGBW');
  assert.equal(profileBaseName('RGB RGB .. SS'), 'RGB');
  assert.equal(profileBaseName('RGB.RGBS'), profileBaseName('RGB.RGB.'));
});

test('menus du Titan : anneau, DMX SETTINGS et RESET SETTINGS', () => {
  const f = fixture();
  const rt = {};
  const ctrl = new AsteraV5Controller({
    type: titan,
    getFixture: () => f,
    emit: ({ settings, state }) => {
      if (settings) Object.assign(f.settings, settings);
      if (state) Object.assign(f.state, state);
    },
    rt,
  });
  let t = 10;
  const tap = (k) => {
    ctrl.press(k, (t += 0.1));
    ctrl.release(k, (t += 0.05));
  };
  const lines = () => ctrl.display(t).lines.map((l) => l.text);

  tap('minus');
  assert.deepEqual(lines(), ['Main menu:', 'RESET SETTINGS']);
  tap('plus'); // l'anneau repasse par l'écran d'état
  assert.equal(ctrl.ui.screen, 'status');
  tap('enter');
  assert.deepEqual(lines(), ['Main menu:', 'DMX ADDRESS']);
  tap('enter');
  tap('plus');
  tap('enter');
  assert.equal(f.settings.dmxAddress, 2);

  // DMX SETTINGS > PIXELS = 16 : le profil RGB devient 42 « RGB RGB »
  tap('plus');
  tap('enter');
  assert.deepEqual(lines(), ['DMX Settings:', 'PIXELS']);
  tap('enter');
  tap('plus');
  tap('plus');
  tap('plus');
  tap('enter');
  assert.equal(f.settings.dmxMode, '42');

  // RESET SETTINGS : − depuis l'état, ENTER, ENTER
  tap('menu');
  tap('menu');
  tap('minus');
  tap('enter');
  assert.deepEqual(lines(), ['ARE YOU SURE ?', 'ENTER FOR YES']);
  f.settings.runtime = 12;
  tap('enter');
  assert.equal(f.settings.crmxLinked, false);
  assert.equal(f.settings.runtime, 5);
  assert.equal(f.settings.inputSelect, 'AUTO');

  // ⏻ : maintien 3 s = BlueMode, appui court = sortie du BlueMode
  ctrl.press('power', (t += 0.1));
  ctrl.tick((t += 3.1));
  ctrl.release('power', (t += 0.1));
  assert.equal(f.state.blueMode, true);
  tap('power');
  assert.equal(f.state.blueMode, false);
  assert.equal(f.state.power, true);
  tap('power');
  assert.equal(f.state.power, false);
});

test('télécommande IR : acceptation selon INPUT SELECT et verrou DMX', async () => {
  const { applyIrKey } = await import('../shared/onboard/ir-remote.js');
  const f = fixture();
  const rt = {};
  // AUTO, rien de verrouillé : la commande est acceptée et verrouille « REMOTE CONTROL »
  let r = applyIrKey(f, titan, rt, 'FIRE', 0);
  assert.equal(r.accepted, true);
  assert.equal(rt.latch, 'REMOTE');
  // AUTO avec CRMX verrouillé : seuls ON / OFF passent
  rt.latch = 'CRMX';
  assert.equal(applyIrKey(f, titan, rt, 'RED', 0).accepted, false);
  assert.equal(applyIrKey(f, titan, rt, 'POWER OFF', 0).accepted, true);
  // INPUT SELECT = CRMX : refus total
  f.settings.inputSelect = 'CRMX wirel. DMX';
  assert.equal(applyIrKey(f, titan, rt, 'POWER ON', 0).accepted, false);
  // ARC3 : BlueMode en deux touches
  f.settings.inputSelect = 'AUTO';
  rt.latch = null;
  assert.equal(applyIrKey(f, titan, rt, 'BLUE MODE 2/2', 1).accepted, false);
  applyIrKey(f, titan, rt, 'BLUE MODE 1/2', 1);
  r = applyIrKey(f, titan, rt, 'BLUE MODE 2/2', 2);
  assert.deepEqual(r.changes, { state: { blueMode: true } });
});

test('HydraPanel : ordre UNLINK / UNBOND du guide 2022 et TouchSlider', () => {
  const hydra = getFixtureType('astera.hydrapanel');
  const f = fixture(hydra);
  const ctrl = new AsteraV5Controller({
    type: hydra,
    getFixture: () => f,
    emit: ({ settings, state }) => {
      if (settings) Object.assign(f.settings, settings);
      if (state) Object.assign(f.state, state);
    },
    rt: {},
  });
  const ring = ctrl.ring();
  // depuis l'écran d'état : −×6 = UNLINK CRMX, −×7 = UNBOND BLUETOOTH (guide HydraPanel)
  assert.equal(ring[ring.length - 6], 'UNLINK CRMX');
  assert.equal(ring[ring.length - 7], 'UNBOND BLUETOOTH');
  let t = 0;
  // le TouchSlider est inactif sur l'écran d'état…
  assert.equal(ctrl.slider('top', (t += 1)), false);
  // …mais actif dans l'éditeur DMX ADDRESS
  ctrl.press('enter', (t += 1));
  ctrl.release('enter', (t += 0.1));
  ctrl.press('enter', (t += 1));
  ctrl.release('enter', (t += 0.1));
  assert.equal(ctrl.slider('top', (t += 1), 4), true);
  ctrl.slider('middle', (t += 1));
  assert.equal(f.settings.dmxAddress, 5);
});

test('Quik : TiltMeter, ordre UNLINK/UNBOND, OutputGain et profils FAN', () => {
  const punch = getFixtureType('astera.quikpunch');
  const f = fixture(punch);
  f.rotation = [-Math.PI / 6, 0, 0]; // faisceau incliné de 30° vers le haut
  const ctrl = new AsteraV5Controller({
    type: punch,
    getFixture: () => f,
    emit: ({ settings, state }) => {
      if (settings) Object.assign(f.settings, settings);
      if (state) Object.assign(f.state, state);
    },
    rt: {},
  });
  const ring = ctrl.ring();
  assert.equal(ring[ring.length - 6], 'UNLINK CRMX');
  assert.equal(ring[ring.length - 7], 'UNBOND BLUETOOTH');
  let t = 0;
  ctrl.press('astera', (t += 1));
  ctrl.release('astera', (t += 0.1));
  assert.deepEqual(ctrl.display(t).lines.map((l) => l.text), ['TILT', '+30.0°']);
  // OutputGain désactivé : −20 %
  const out = new Float32Array(3);
  const data = new Uint8Array(512);
  data.set([255, 0, 0]);
  computeOutput(f, punch, { dmx: new Map([[1, data]]), live: new Set([1]), time: 0 }, out, {});
  const on = out[0];
  f.settings.outputGain = 'OFF';
  computeOutput(f, punch, { dmx: new Map([[1, data]]), live: new Set([1]), time: 0 }, out, {});
  assert.ok(Math.abs(out[0] / on - 0.8) < 1e-6);
  // profil FAN : le dernier canal pilote le ventilateur
  const fan = getMode(punch, '144');
  assert.equal(modeFootprint(punch, '144'), 4);
  assert.equal(fan.channels.at(-1).f, 'fan');
});

test('AX5-BTB et AX7 : menu actuel vs firmware 2018 (UNPAIR, NO / YES)', () => {
  const make = (type) => {
    const f = fixture(type);
    const ctrl = new AsteraV5Controller({
      type,
      getFixture: () => f,
      emit: ({ settings, state }) => {
        if (settings) Object.assign(f.settings, settings);
        if (state) Object.assign(f.state, state);
      },
      rt: {},
    });
    return { f, ctrl };
  };
  const ax5 = make(getFixtureType('astera.ax5-btb'));
  const ring5 = ax5.ctrl.ring();
  assert.equal(ring5[4], 'UNLINK CRMX');
  assert.equal(ring5[5], 'UNBOND BLUETOOTH');
  assert.deepEqual(ax5.ctrl.subItems('DMX SETTINGS'), ['DMX PROFILE', 'STROBE', 'DIMMER CURVE', 'DMX FAIL']);

  const ax7 = make(getFixtureType('astera.ax7'));
  assert.equal(ax7.ctrl.ring()[4], 'UNPAIR CRMX');
  let t = 0;
  const tap = (k) => {
    ax7.ctrl.press(k, (t += 0.1));
    ax7.ctrl.release(k, (t += 0.05));
  };
  ax7.f.settings.runtime = 9;
  tap('minus'); // RESET SETTINGS (appui court : action au relâchement)
  tap('enter');
  assert.deepEqual(ax7.ctrl.display(t).lines.map((l) => l.text), ['ARE YOU SURE?', '[NO]  YES ']);
  tap('enter'); // NO : rien n'est réinitialisé
  assert.equal(ax7.f.settings.runtime, 9);
  tap('enter');
  tap('plus'); // sélectionne YES
  tap('enter');
  assert.equal(ax7.f.settings.runtime, 5);
});
