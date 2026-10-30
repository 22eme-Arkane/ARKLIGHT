// Panneaux de commande simulés (ARRI, Aputure, Hive, DMG, Kino Flo, LiteGear, Dedolight).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { PanelController } from '../shared/onboard/panel.js';
import { PANEL_TYPES, panelFor } from '../shared/onboard/panels/index.js';
import { serialOf } from '../shared/onboard/panels/common.js';
import { OnboardController } from '../shared/onboard/engine.js';
import { defaultSettings, defaultState, getFixtureType } from '../shared/fixtures/index.js';

/** Projecteur simulé : les changements émis par le panneau s'appliquent comme dans l'interface. */
function rig(typeId, { live = false } = {}) {
  const type = getFixtureType(typeId);
  const f = { id: `f-${typeId}`, type: typeId, name: 'Test', position: [0, 2, 3], rotation: [0, 0, 0], settings: { ...defaultSettings(type), dmxAddress: 1 }, state: defaultState(type) };
  let t = 0;
  const c = new PanelController({
    type,
    def: panelFor(type),
    getFixture: () => f,
    emit: (ch) => {
      if (ch.settings) Object.assign(f.settings, ch.settings);
      if (ch.state) Object.assign(f.state, ch.state);
    },
    env: () => ({ dmxLive: () => live }),
  });
  const tick = (dt = 0.1) => (t += dt);
  const press = (id, hold = 0.1) => {
    c.press(id, tick());
    for (let k = 0; k < Math.ceil(hold / 0.25); k++) c.tick(tick(0.25));
    c.release(id, tick());
  };
  const turn = (id, steps) => c.turn(id, steps, tick(0.3));
  const draw = () => {
    const d = c.display(tick(0.01));
    assert.ok(Array.isArray(d.prims), `${typeId} : primitives`);
    for (const p of d.prims) {
      assert.ok(['text', 'rect', 'line', 'ring'].includes(p.t), `${typeId} : primitive ${p.t}`);
      if (p.t === 'text') assert.ok(Number.isFinite(p.x) && Number.isFinite(p.y), `${typeId} : texte placé`);
    }
    return d;
  };
  return { c, f, type, press, turn, draw };
}

const elements = (def) => {
  const out = [];
  const walk = (it) => {
    if (it.col) it.col.forEach(walk);
    else out.push(it);
  };
  def.face.rows.flat().forEach(walk);
  return out;
};

test('chaque modèle avec panneau : toutes les touches et molettes, écran dessiné sans erreur', () => {
  assert.ok(PANEL_TYPES.length >= 30);
  for (const id of PANEL_TYPES) {
    const { c, press, turn, draw, type } = rig(id);
    assert.ok(type, `${id} existe dans la bibliothèque`);
    const els = elements(c.def).filter((e) => !e.model || e.model === id);
    draw();
    for (let round = 0; round < 2; round++) {
      for (const e of els) {
        if (e.el === 'key' || (e.el === 'knob' && e.push)) {
          press(e.id);
          draw();
          press(e.id, 3.2); // appui long
          draw();
        }
        if (e.el === 'knob') {
          turn(e.id, 3);
          draw();
          turn(e.id, -5);
          draw();
        }
      }
    }
    c.leds();
    c.hint();
    assert.equal(c.overridePixels(), false);
  }
});

test('SkyPanel : MENU › DMX Settings › DMX Address, molette, validation', () => {
  const { c, f, press, turn } = rig('arri.skypanel-s60c');
  press('menu');
  assert.equal(c.screen, 'menu');
  press('is'); // DMX Settings
  press('is'); // DMX Address : édition
  turn('is', 4);
  press('is');
  assert.equal(f.settings.dmxAddress, 5);
  press('back');
  press('back');
  assert.equal(c.screen, 'home');
  // MODE : CCT ↔ HSI ; molettes de l'accueil
  press('mode');
  assert.equal(f.state.manual.mode, 'hsi');
  turn('center', 10);
  assert.ok(f.state.manual.hue !== 0);
  press('mode');
  turn('center', 3);
  assert.ok(f.state.manual.cct > 5600);
});

test('L-Series : ENTER puis 1 = 3200 K ; SETUP › DMX › CHAN', () => {
  const { c, f, press, turn } = rig('arri.l10-c');
  press('enter');
  press('mem1');
  assert.equal(f.state.manual.cct, 3200);
  press('minus'); // STATUS
  press('minus'); // SETUP
  press('enter'); // menu SETUP
  press('enter'); // DMX
  press('minus'); // LSIG
  press('minus'); // CHAN
  press('enter');
  press('plus');
  press('plus'); // −2 dans une liste ? non : édition numérique, + = −1 (mouvement vers le haut)
  turn('intensity', 0);
  press('enter');
  assert.ok(f.settings.dmxAddress >= 1);
  assert.equal(c.hint(), '', 'pas de message (la valeur clignote)');
});

test('BroadCaster : roue codeuse 9 0 4 = mode secours 5600 K', () => {
  const { f, press } = rig('arri.broadcaster-2-plus');
  for (let i = 0; i < 9; i++) press('d2+');
  // centaines à 9, unités à 1 (adresse 001) → 901, puis 3 de plus aux unités → 904
  for (let i = 0; i < 3; i++) press('d0+');
  assert.equal(f.settings.panel.bcd, 904);
  assert.equal(f.state.manual.cct, 5600);
});

test('Dedolight : sous 001 → mode manuel, les molettes agissent', () => {
  const { f, press, turn } = rig('dedolight.dled4-bi');
  turn('dim', 5);
  assert.equal(f.settings.panel?.manual ?? false, false); // en DMX, molettes inactives
  press('both');
  press('down');
  assert.equal(f.settings.panel.manual, true);
  assert.equal(f.settings.source, 'manual');
  const before = f.state.manual.intensity;
  turn('dim', 5);
  assert.ok(f.state.manual.intensity !== before);
});

test('Hive : l’interrupteur DMX passe la commande en DMX seul', () => {
  const { f, press } = rig('hive.hornet-200-c');
  press('dmxsw');
  assert.equal(f.settings.source, 'dmx');
  press('dmxsw');
  assert.equal(f.settings.source, 'manual');
});

test('Hive : retour en Local avec C ou A choisi, ADJUST règle K ou D', () => {
  const { c, f, press, turn } = rig('hive.hornet-200-c');
  press('dmxsw');
  turn('select', 4); // A (adresse)
  assert.equal(c.st.cur, 'A');
  press('dmxsw');
  assert.equal(c.st.cur, 'D');
  const i = f.state.manual.intensity;
  turn('adjust', -2);
  assert.ok(f.state.manual.intensity < i);
  press('dmxsw');
  turn('select', 1); // D → C (température)
  assert.equal(c.st.cur, 'C');
  press('dmxsw');
  assert.equal(c.st.cur, 'K');
  turn('adjust', 2);
  assert.equal(f.state.manual.cct, 6000);
});

test('Kino True Match : préréglage 2 = 3200 K ; LOCK bloque la molette', () => {
  const { c, f, press, turn } = rig('kino-flo.celeb-450-led-dmx');
  press('p2');
  assert.equal(f.state.manual.cct, 3200);
  press('lock');
  assert.match(c.hint(), /^Verrouillé/);
  const i = f.state.manual.intensity;
  turn('knob', 5);
  assert.equal(f.state.manual.intensity, i);
  press('lock');
  assert.equal(c.hint(), 'Déverrouillé');
});

test('DMG : appui court sur ⏻ = noir, puis retour à l’intensité d’avant', () => {
  const { f, press } = rig('dmg-lumiere.sl1-mix');
  f.state.manual = { ...f.state.manual, intensity: 0.7 };
  press('power');
  assert.equal(f.state.manual.intensity, 0);
  press('power');
  assert.equal(f.state.manual.intensity, 0.7);
});

test('numéros de série fictifs : toujours des caractères de l’alphabet', () => {
  for (let k = 0; k < 200; k++) {
    assert.match(serialOf({ f: { id: `f-${k}` } }, 8), /^[0-9A-HJ-NPR-Z]{8}$/);
    assert.match(serialOf({ f: { id: `fx${k}` } }, 4, '0123456789'), /^\d{4}$/);
  }
});

test('commande générique togglePower : message selon le nouvel état', () => {
  const f = { id: 'f-gen', state: { power: true }, settings: {} };
  const c = new OnboardController({ type: { id: 'gen', controls: {} }, getFixture: () => f, emit: (ch) => Object.assign(f.state, ch.state || {}) });
  c.runAction('togglePower');
  assert.equal(f.state.power, false);
  assert.equal(c.message.text, 'Extinction');
  c.runAction('togglePower');
  assert.equal(c.message.text, 'Allumage');
});
