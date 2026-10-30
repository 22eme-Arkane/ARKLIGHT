// Contrôleur MIDI : décodage des messages, aiguillage vers les fonctions d'ARKLIGHT.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { cleanMappings, describeSource, parseMidi, routeMidi, sourceKey } from '../shared/midi.js';

test('décodage : CC, notes (vélocité 0 = relâchement), programme, pitch bend ; le reste est ignoré', () => {
  assert.deepEqual(parseMidi([0xb0, 7, 127]), { type: 'cc', channel: 1, number: 7, value: 1, on: true });
  assert.deepEqual(parseMidi([0xb3, 1, 0]), { type: 'cc', channel: 4, number: 1, value: 0, on: false });
  assert.equal(parseMidi([0x91, 60, 100]).on, true);
  assert.equal(parseMidi([0x91, 60, 0]).on, false);
  assert.equal(parseMidi([0x81, 60, 64]).on, false);
  assert.deepEqual(parseMidi([0xc0, 4]), { type: 'program', channel: 1, number: 4, value: 1, on: true });
  assert.equal(parseMidi([0xe0, 0x7f, 0x7f]).value, 1);
  assert.equal(parseMidi([0xf8]), null); // horloge
  assert.equal(parseMidi([0xd0, 40]), null); // aftertouch
  assert.equal(parseMidi([]), null);
  // une note et son relâchement : même commande
  assert.equal(sourceKey(parseMidi([0x90, 60, 90])), sourceKey(parseMidi([0x80, 60, 0])));
  assert.equal(describeSource({ type: 'cc', channel: 1, number: 7 }), 'CC 7 · canal 1');
  assert.equal(describeSource({ type: 'note', channel: 2, number: 60 }), 'Note 60 (C4) · canal 2');
});

test('aiguillage : un fader suit la valeur, une touche ne déclenche qu’à l’appui', () => {
  const maps = cleanMappings([
    { id: 'a', source: { type: 'cc', channel: 1, number: 1 }, target: { kind: 'fader', index: 1 } },
    { id: 'b', source: { type: 'cc', channel: 1, number: 20 }, target: { kind: 'command', id: 'console.full' } },
    { id: 'c', source: { type: 'note', channel: 1, number: 36 }, target: { kind: 'group', name: 'Face' } },
    { id: 'd', source: { type: 'note', channel: 1, number: 37 }, target: { kind: 'groupLevel', name: 'Contres' } },
  ]);
  const held = new Map();
  const r = (bytes) => routeMidi(maps, parseMidi(bytes), held);
  assert.deepEqual(
    r([0xb0, 1, 64]).map((x) => [x.mapping.id, Math.round(x.value * 127)]),
    [['a', 64]],
  );
  // touche en CC : 0 → 127 déclenche une fois, rester appuyé ne répète pas, relâcher non plus
  assert.equal(r([0xb0, 20, 127]).length, 1);
  assert.equal(r([0xb0, 20, 127]).length, 0);
  assert.equal(r([0xb0, 20, 0]).length, 0);
  assert.equal(r([0xb0, 20, 127])[0].press, true);
  // note : appui / relâchement
  assert.equal(r([0x90, 36, 100])[0].mapping.id, 'c');
  assert.equal(r([0x80, 36, 0]).length, 0);
  // une touche peut aussi commander une intensité : plein à l'appui, zéro au relâchement
  assert.equal(r([0x90, 37, 100])[0].value, 1);
  assert.equal(r([0x90, 37, 0])[0].value, 0);
  // autre canal : rien
  assert.equal(r([0xb1, 1, 64]).length, 0);
});

test('correspondances lues : les mal formées sont écartées, les valeurs bornées', () => {
  const list = cleanMappings([null, { target: { kind: 'inconnu' } }, { id: 'x', source: { type: 'cc', channel: 40, number: 300 }, target: { kind: 'fader', index: 2 } }]);
  assert.equal(list.length, 1);
  assert.deepEqual(list[0].source, { type: 'cc', channel: 16, number: 127 });
  assert.deepEqual(cleanMappings('pas une liste'), []);
});

test('flash : appui puis relâchement ; échelles des faders (Kelvin, teinte ±vert, exposition)', async () => {
  const { scaleValue } = await import('../shared/midi.js');
  const maps = cleanMappings([{ id: 'f', source: { type: 'note', channel: 1, number: 40 }, target: { kind: 'flash' } }]);
  const held = new Map();
  const r = (bytes) => routeMidi(maps, parseMidi(bytes), held);
  assert.deepEqual(
    r([0x90, 40, 120]).map((x) => !!x.press),
    [true],
  );
  assert.deepEqual(
    r([0x80, 40, 0]).map((x) => !!x.release),
    [true],
  );
  assert.equal(r([0x80, 40, 0]).length, 0);
  assert.equal(scaleValue('selCct', 0), 2000);
  assert.equal(scaleValue('selCct', 1), 10000);
  assert.equal(scaleValue('selTint', 0.5), 0);
  assert.equal(scaleValue('exposure', 10 / 13), 0);
  assert.equal(scaleValue('selHue', 1), 360);
});

test('préréglages : DMX Operator 384 (V2.6) d’après la table MIDI du manuel ; aucun doublon, canal réglable', async () => {
  const { MIDI_PRESETS, presetById, presetMappings } = await import('../shared/midi-presets.js');
  for (const p of MIDI_PRESETS) {
    const maps = presetMappings(p);
    assert.ok(maps.length > 0, p.id);
    assert.equal(cleanMappings(maps).length, maps.length, `${p.id} : correspondances valides`);
    const keys = maps.map((m) => sourceKey(m.source));
    assert.equal(new Set(keys).size, keys.length, `${p.id} : une seule fonction par commande`);
  }
  const op = presetMappings(presetById('adj-dmx-operator-384'));
  const route = (bytes) => routeMidi(op, parseMidi(bytes), new Map()).map((r) => r.mapping.target);
  assert.deepEqual(route([0xb0, 0, 64]), [{ kind: 'fader', index: 1 }]); // banque 1, fader 1
  assert.deepEqual(route([0xb0, 16, 64]), [{ kind: 'fader', index: 17 }]); // banque 2, fader 1
  assert.deepEqual(route([0xb0, 125, 64]), [{ kind: 'fader', index: 126 }]);
  assert.deepEqual(route([0xb0, 126, 64]), [{ kind: 'selectionLevel' }]); // Speed
  assert.deepEqual(route([0xb1, 33, 64]), [{ kind: 'selCct' }]); // Fade (2e canal)
  assert.deepEqual(route([0x90, 80, 100]), [{ kind: 'fixtureLatch', index: 1 }]); // Fixture 1, banque 1 (à bascule)
  assert.deepEqual(route([0x90, 92, 100]), [{ kind: 'fixtureLatch', index: 13 }]); // Fixture 1, banque 2
  assert.deepEqual(route([0x90, 48, 100]), [{ kind: 'planIndex', index: 1 }]); // Scene 1 → plan 1
  assert.deepEqual(route([0x90, 11, 100]), [{ kind: 'groupLatch', index: 12 }]); // Chase 12 → groupe 12
  // console réglée sur le canal 3 : tout se décale
  const op3 = presetMappings(presetById('adj-dmx-operator-384'), { channel: 3 });
  assert.deepEqual(routeMidi(op3, parseMidi([0xb2, 0, 64]), new Map()).map((r) => r.mapping.target), [{ kind: 'fader', index: 1 }]);
  assert.equal(routeMidi(op3, parseMidi([0xb0, 0, 64]), new Map()).length, 0);
});

test('touches à bascule : allumée / éteinte (note puis relâchement), ou note rejouée qui éteint', () => {
  const maps = cleanMappings([
    { id: 'f', source: { type: 'note', channel: 1, number: 80 }, target: { kind: 'fixtureLatch', index: 1 } },
    { id: 'b', source: { type: 'cc', channel: 1, number: 90 }, target: { kind: 'blackout' } },
    { id: 'p', source: { type: 'note', channel: 1, number: 48 }, target: { kind: 'planIndex', index: 1 } },
  ]);
  const held = new Map();
  const r = (bytes) => routeMidi(maps, parseMidi(bytes), held).map((x) => x.latch ?? (x.press ? 'press' : x.release ? 'release' : x.value));
  // console qui garde la touche allumée : note à l'allumage, relâchement à l'extinction
  assert.deepEqual(r([0x90, 80, 127]), [true]);
  assert.deepEqual(r([0x80, 80, 0]), [false]);
  // console qui n'envoie qu'une note à chaque appui : la deuxième éteint
  assert.deepEqual(r([0x90, 80, 127]), [true]);
  assert.deepEqual(r([0x90, 80, 127]), [false]);
  assert.deepEqual(r([0x90, 80, 127]), [true]);
  // touche en CC : 127 allume, 0 éteint, un 127 répété ne change rien
  assert.deepEqual(r([0xb0, 90, 127]), [true]);
  assert.deepEqual(r([0xb0, 90, 127]), []);
  assert.deepEqual(r([0xb0, 90, 0]), [false]);
  // une touche poussoir déclenche à chaque appui
  assert.deepEqual(r([0x90, 48, 100]), ['press']);
  assert.deepEqual(r([0x80, 48, 0]), []);
  assert.deepEqual(r([0x90, 48, 100]), ['press']);
});
