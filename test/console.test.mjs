// Console ARKLIGHT : reprise d'un projecteur sans saut, priorité, relâche, changement de profil.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { FIXTURE_TYPES, defaultSettings, defaultState, getFixtureType, getMode } from '../shared/fixtures/index.js';
import { computeOutput, isAsteraV5 } from '../shared/engine/output.js';
import { encodeSlot, releaseOnLookChange, remapValues, seedConsole } from '../shared/engine/console.js';
import { compileMode, readAttributes } from '../shared/engine/dmx.js';

const LOOKS = {
  cct3200: { manual: { mode: 'cct', intensity: 0.6, cct: 3200, tint: 0 }, sa: { intensity: 60, colors: [{ kind: 'hsi', hue: 0, sat: 0, cct: 3200 }] } },
  green: { manual: { mode: 'cct', intensity: 1, cct: 5600, tint: 0.3 }, sa: { intensity: 100, colors: [{ kind: 'hsi', hue: 0, sat: 0, cct: 5600, green: 0.3 }] } },
  red: { manual: { mode: 'hsi', intensity: 0.8, hue: 350, sat: 0.9 }, sa: { intensity: 80, colors: [{ kind: 'hsi', hue: 350, sat: 90, cct: 5600 }] } },
  cyan: { manual: { mode: 'rgb', intensity: 0.5, rgb: [0, 0.7, 1] }, sa: { intensity: 50, colors: [{ kind: 'named', name: 'CYAN' }] } },
};
const ctx = { dmx: new Map(), live: new Set(), time: 1 };

function fixture(typeId, modeId, look) {
  const type = getFixtureType(typeId);
  const f = { id: `f-${typeId}`, type: typeId, position: [0, 2, 0], rotation: [0, 0, 0], settings: { ...defaultSettings(type), dmxMode: modeId ?? defaultSettings(type).dmxMode }, state: defaultState(type) };
  if (look) {
    if (isAsteraV5(type)) f.settings.standalone = { ...(f.settings.standalone || {}), program: 0, ...look.sa };
    else f.state.manual = { ...f.state.manual, ...look.manual };
  }
  return { type, f };
}
const output = (f, type) => {
  const out = new Float32Array(type.pixels * 3);
  const source = computeOutput(f, type, ctx, out, {});
  return { out, source };
};
const maxDiff = (a, b) => a.reduce((m, v, i) => Math.max(m, Math.abs(v - b[i])), 0);

/** La console prend le projecteur : même lumière qu'avant. */
function takeOver(typeId, modeId, look) {
  const { type, f } = fixture(typeId, modeId, look);
  const before = output(f, type);
  const mode = getMode(type, f.settings.dmxMode);
  const values = seedConsole(f, type, mode, { dmx: ctx.dmx, source: before.source });
  f.state.console = { mode: mode.id, values };
  const after = output(f, type);
  assert.equal(after.source, 'console');
  return { err: maxDiff(before.out, after.out), f, type, mode, values };
}

test('la console reprend le réglage en cours sans saut (L10, Astera, tubes, tungstène)', () => {
  for (const look of Object.keys(LOOKS)) {
    assert.ok(takeOver('arri.l10-c', 'm1', LOOKS[look]).err < 0.03, `L10 P01 ${look}`);
    assert.ok(takeOver('astera.ax3', '2', LOOKS[look]).err < 0.03, `AX3 RGBW ${look}`);
    assert.ok(takeOver('astera.titan', '16', LOOKS[look]).err < 0.03, `Titan effet RGB ${look}`);
  }
  assert.ok(takeOver('arri.skypanel-s60c', 'm5', LOOKS.cct3200).err < 0.03, 'SkyPanel HSI sans Kelvin : blanc chaud approché');
  const lamp = FIXTURE_TYPES.find((t) => t.technology === 'tungsten' && t.dmx?.modes?.length);
  assert.ok(takeOver(lamp.id, null, LOOKS.cct3200).err < 0.02, 'tungstène : même niveau');
});

test('tous les modes : valeurs dans les bornes, et les blancs reproduits quand le mode le permet', () => {
  let checked = 0;
  for (const type of FIXTURE_TYPES) {
    if (!type.pixels || !type.dmx?.modes?.length) continue;
    for (const mode of type.dmx.modes) {
      const fns = new Set(compileMode(mode).slots.map((s) => s.f));
      const { values, err } = takeOver(type.id, mode.id, LOOKS.cct3200);
      compileMode(mode).slots.forEach((s, i) => assert.ok(values[i] >= 0 && values[i] <= (s.bytes === 2 ? 65535 : 255), `${type.id} ${mode.id} canal ${i}`));
      // un blanc chaud se refait avec un canal Kelvin ou un mélange de couleurs
      if (fns.has('cct') || (fns.has('red') && fns.has('green') && fns.has('blue'))) {
        assert.ok(err < 0.05, `${type.id} ${mode.id} (${mode.name}) : écart ${err}`);
        checked++;
      }
    }
  }
  assert.ok(checked > 1500);
});

test('les autres projecteurs ne bougent pas ; relâcher rend le réglage', () => {
  const { type, f } = fixture('arri.l10-c', 'm1', LOOKS.red);
  const other = fixture('arri.l10-c', 'm1', LOOKS.cct3200);
  const otherBefore = output(other.f, other.type).out;
  const mode = getMode(type, 'm1');
  const values = seedConsole(f, type, mode, {});
  values[0] = 0; // dimmer à 0 : noir
  f.state.console = { mode: 'm1', values };
  assert.ok(output(f, type).out.every((v) => v === 0));
  assert.equal(maxDiff(output(other.f, other.type).out, otherBefore), 0);
  f.state.console = null;
  assert.equal(output(f, type).source, 'manual');
  assert.ok(Math.max(...output(f, type).out) > 0.5);
});

test('la console passe avant le DMX reçu', () => {
  const { type, f } = fixture('arri.l10-c', 'm1', LOOKS.cct3200);
  ctx.dmx.set(f.settings.universe, new Uint8Array(512)); // univers reçu, tout à 0
  try {
    assert.equal(output(f, type).source, 'dmx');
    const mode = getMode(type, 'm1');
    f.state.console = { mode: 'm1', values: seedConsole(f, type, mode, { dmx: ctx.dmx, source: 'manual' }) };
    const o = output(f, type);
    assert.equal(o.source, 'console');
    assert.ok(Math.max(...o.out) > 0.3);
  } finally {
    ctx.dmx.clear();
  }
});

test('changement de profil : les valeurs suivent, fonction par fonction', () => {
  const { type, f, values } = takeOver('astera.ax3', '2', LOOKS.cyan);
  const before = output(f, type).out;
  // le profil change (menu du projecteur, inspecteur…) : la console garde ses valeurs de l'ancien mode
  const rgbModes = type.dmx.modes.filter((m) => m.id !== '2' && /RGB/.test(m.name) && !/EFFECT/.test(m.name));
  assert.ok(rgbModes.length);
  for (const m of rgbModes) {
    f.settings.dmxMode = m.id;
    assert.ok(maxDiff(output(f, type).out, before) < 0.05, `AX3 → ${m.name}`);
  }
  // conversion directe
  const to = getMode(type, rgbModes[0].id);
  const next = remapValues(getMode(type, '2'), to, values, type.pixels);
  assert.equal(next.length, compileMode(to).slots.length);
});

test('encodeSlot est l’inverse du décodage (Kelvin Astera, zone neutre, plages)', () => {
  const ax3 = getFixtureType('astera.ax3');
  const sky = getFixtureType('arri.skypanel-s60c');
  for (const [type, modeId] of [
    [ax3, '2'],
    [sky, 'm1'],
    [sky, 'm5'],
  ]) {
    const mode = getMode(type, modeId);
    for (const slot of compileMode(mode).slots) {
      if (!['cct', 'tint', 'hue', 'sat', 'dimmer'].includes(slot.f)) continue;
      const max = slot.bytes === 2 ? 65535 : 255;
      for (const raw of [0, Math.round(max * 0.3), Math.round(max * 0.77), max]) {
        const buf = new Uint8Array(512);
        if (slot.bytes === 2) {
          buf[slot.offset] = raw >> 8;
          buf[slot.offset + 1] = raw & 255;
        } else buf[slot.offset] = raw;
        const v = readAttributes(mode, buf, 1, 1).fixture[slot.key] ?? readAttributes(mode, buf, 1, 1).pixels[0]?.[slot.key];
        if (v === undefined) continue;
        const again = new Uint8Array(512);
        const enc = encodeSlot(slot, v);
        if (slot.bytes === 2) {
          again[slot.offset] = enc >> 8;
          again[slot.offset + 1] = enc & 255;
        } else again[slot.offset] = enc;
        const v2 = readAttributes(mode, again, 1, 1).fixture[slot.key] ?? readAttributes(mode, again, 1, 1).pixels[0]?.[slot.key];
        const tol = slot.f === 'cct' ? 40 : slot.f === 'hue' ? 2 : 0.01;
        assert.ok(Math.abs(v2 - v) <= tol, `${type.id} ${slot.f} ${raw} : ${v} → ${v2}`);
      }
    }
  }
});

test('un réglage fait à la main rend la main (le dernier geste l’emporte)', () => {
  const held = { state: { console: { mode: 'm1', values: [255] } } };
  assert.deepEqual(releaseOnLookChange(held, { state: { manual: { intensity: 0.4 } } }), { state: { manual: { intensity: 0.4 }, console: null } });
  assert.deepEqual(releaseOnLookChange(held, { settings: { standalone: { intensity: 50 } } }), { settings: { standalone: { intensity: 50 } }, state: { console: null } });
  // déplacer le projecteur ou changer son adresse ne le relâche pas
  const move = { position: [1, 2, 3] };
  assert.equal(releaseOnLookChange(held, move), move);
  // projecteur libre : rien à ajouter
  const free = { state: {} };
  const ch = { state: { manual: { intensity: 1 } } };
  assert.equal(releaseOnLookChange(free, ch), ch);
});

test('couleur de la sélection : Kelvin et teinte, même intensité, sur des profils différents', async () => {
  const { adjustColor, consoleOptions } = await import('../shared/engine/console.js');
  const cases = [
    ['arri.l10-c', 'm1'], // Kelvin + fondu vers RGBW
    ['arri.skypanel-s60c', 'm5'], // HSI sans canal Kelvin
    ['astera.ax3', '2'], // RGBW
  ];
  for (const [id, modeId] of cases) {
    const { type, f, mode, values } = takeOver(id, modeId, LOOKS.cct3200);
    const opts = { ...consoleOptions(f, type), pixelCount: type.pixels };
    const level = Math.max(...output(f, type).out);
    // plus chaud : moins de bleu, même niveau de sortie
    const warm = adjustColor(mode, values, { cct: 2400 }, opts);
    f.state.console = { mode: mode.id, values: warm.values };
    const w = output(f, type).out;
    assert.ok(Math.abs(Math.max(...w) - level) < 0.03, `${id} : intensité gardée`);
    assert.ok(w[2] < w[0] * 0.5, `${id} : 2400 K est chaud`);
    // teinte 240° (bleu) : le bleu domine
    const blue = adjustColor(mode, values, { hue: 240 }, opts);
    f.state.console = { mode: mode.id, values: blue.values };
    const b = output(f, type).out;
    assert.ok(b[2] > b[0] && b[2] > b[1], `${id} : bleu`);
  }
});
