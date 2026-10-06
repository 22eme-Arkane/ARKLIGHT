// Tests de la bibliothèque multi-marques (profils OFL), du tungstène et du HMI.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CATEGORIES, FIXTURE_TYPES, defaultSettings, defaultState, getFixtureType, modeFootprint } from '../shared/fixtures/index.js';
import { EMITTER_FUNCTIONS, readAttributes } from '../shared/engine/dmx.js';
import { HMI, computeOutput, tungstenCct } from '../shared/engine/output.js';

const KNOWN = new Set(['none', 'fan', 'dimmer', 'cct', 'cctIndex', 'tint', 'crossfade', 'hue', 'sat', 'x', 'y', 'strobe', 'index', ...EMITTER_FUNCTIONS]);

function fixture(typeId, settings = {}, state = {}) {
  const type = getFixtureType(typeId);
  return { type, f: { id: 'x', type: typeId, settings: { ...defaultSettings(type), ...settings }, state: { ...defaultState(type), ...state } } };
}

test('bibliothèque : identifiants uniques, familles connues, profils DMX valides', () => {
  const ids = FIXTURE_TYPES.map((t) => t.id);
  assert.equal(new Set(ids).size, ids.length);
  const cats = new Set(CATEGORIES.map((c) => c.id));
  for (const t of FIXTURE_TYPES) {
    assert.ok(cats.has(t.category), `${t.id} : famille ${t.category}`);
    for (const m of t.dmx?.modes || []) {
      assert.ok(modeFootprint(t, m.id) > 0, `${t.id} / ${m.name} : aucun canal`);
      for (const ch of m.channels.flatMap((c) => (c.pixels ? c.channels : [c]))) {
        const f = typeof ch === 'string' ? ch : ch.f;
        if (f?.startsWith('fx')) continue; // modes d'effet Astera
        assert.ok(KNOWN.has(f), `${t.id} / ${m.name} : fonction inconnue ${f}`);
      }
    }
  }
  assert.ok(FIXTURE_TYPES.some((t) => t.brand === 'ARRI') && FIXTURE_TYPES.some((t) => t.technology === 'tungsten') && FIXTURE_TYPES.some((t) => t.technology === 'hmi'));
});

test('SkyPanel S60-C (OFL) : mode CCT 8 bits, zone neutre du vert/magenta', () => {
  const t = getFixtureType('arri.skypanel-s60c');
  const mode = t.dmx.modes.find((m) => m.name.startsWith('P02'));
  assert.equal(modeFootprint(t, mode.id), 7);
  const data = new Uint8Array(512);
  data.set([255, 128, 5], 0); // dimmer, CCT, G/M dans la zone neutre (0–10)
  let a = readAttributes(mode, data, 1, 1).fixture;
  assert.equal(a.dimmer, 1);
  assert.ok(Math.abs(a.cct - (2800 + (128 / 255) * 7200)) < 1);
  assert.equal(a.tint, 0, '0–10 : pas de correction');
  data[2] = 255;
  a = readAttributes(mode, data, 1, 1).fixture;
  assert.equal(a.tint, 1, 'plein vert en haut de course');
  data[2] = 11;
  a = readAttributes(mode, data, 1, 1).fixture;
  assert.ok(a.tint < -0.98, 'plein magenta juste après la zone neutre');
});

test('LiteMat (OFL) : le mode « Double » pilote deux zones', () => {
  const t = getFixtureType('litegear.litemat-plus-2');
  assert.equal(t.pixels, 2);
  const mode = t.dmx.modes.find((m) => m.name === 'Double 8bit');
  assert.equal(mode.pixels, 2);
  const data = new Uint8Array(512);
  data.set([0, 0, 255, 255], 0); // zone 1 éteinte, zone 2 pleine
  const { pixels } = readAttributes(mode, data, 1, 2);
  assert.equal(pixels[0].dimmer, 0);
  assert.equal(pixels[1].dimmer, 1);
});

test('tungstène : la couleur se réchauffe quand on gradue, avec l’inertie du filament', () => {
  assert.ok(Math.abs(tungstenCct(3200, 0.5) - 2970) < 10, '3200 K → ~2970 K à 50 % du flux');
  const { type, f } = fixture('mole.baby-junior-2k', { source: 'dmx', universe: 1, dmxAddress: 1 });
  const data = new Uint8Array(512);
  const ctx = { dmx: new Map([[1, data]]), time: 0 };
  const out = new Float32Array(3);
  const rt = {};
  computeOutput(f, type, ctx, out, rt); // éteint
  data[0] = 255;
  ctx.time = 0.02;
  computeOutput(f, type, ctx, out, rt);
  assert.ok(rt.flux[0] < 0.9, 'le filament ne s’allume pas instantanément');
  for (let i = 0; i < 100; i++) {
    ctx.time += 0.02;
    computeOutput(f, type, ctx, out, rt);
  }
  assert.ok(rt.flux[0] > 0.99);
  const full = [...out];
  data[0] = 64; // 25 %
  for (let i = 0; i < 100; i++) {
    ctx.time += 0.02;
    computeOutput(f, type, ctx, out, rt);
  }
  assert.ok(Math.abs(rt.flux[0] - 64 / 255) < 0.01);
  assert.ok(out[2] / out[0] < full[2] / full[0], 'plus de rouge relatif (plus chaud) à 25 %');
});

test('HMI : gradation limitée à 50–100 %, montée en régime après l’allumage', () => {
  const { type, f } = fixture('arri.m18', { source: 'dmx', universe: 1, dmxAddress: 1 });
  const data = new Uint8Array(512);
  data[0] = 3; // à peine ouvert
  const ctx = { dmx: new Map([[1, data]]), time: 100 };
  const out = new Float32Array(3);
  const rt = {};
  computeOutput(f, type, ctx, out, rt);
  assert.ok(rt.flux[0] < 0.5, 'à l’allumage, la lampe n’est pas encore à son régime');
  ctx.time += 300;
  computeOutput(f, type, ctx, out, rt);
  assert.ok(rt.flux[0] >= HMI.minPower * 0.99, 'jamais en dessous de 50 % une fois chaude');
  data[0] = 0;
  computeOutput(f, type, ctx, out, rt);
  assert.equal(rt.flux[0], 0, 'niveau 0 : lampe éteinte');
});
