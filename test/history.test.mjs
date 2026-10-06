// Annuler / rétablir et état « enregistrée » de la scène.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { History, historyKey, historyLabel } from '../server/history.js';
import { SceneStore } from '../server/scene-store.js';
import { FIXTURE_TYPES } from '../shared/fixtures/index.js';

const TYPE = FIXTURE_TYPES[0].id;

/** Applique un message comme le serveur : étape d'historique autour de la modification. */
function apply(history, store, msg, change, now) {
  const step = history.begin(store.scene, store.revision, msg, now);
  change();
  const r = history.end(step, store.scene, store.revision);
  if (r === 'noop') store.revision = step.revision;
  return r;
}

/** Dossier de scènes temporaire ; open() ouvre un SceneStore dont la sauvegarde différée est coupée à la fin. */
function withStore(fn) {
  const dir = mkdtempSync(join(tmpdir(), 'arklight-'));
  const stores = [];
  const open = () => {
    const s = new SceneStore(dir);
    stores.push(s);
    return s;
  };
  try {
    return fn(open);
  } finally {
    for (const s of stores) clearTimeout(s._timer);
    rmSync(dir, { recursive: true, force: true });
  }
}

test('nature et libellé des étapes', () => {
  assert.equal(historyKey({ t: 'fixture.update', id: 'f1', changes: { position: [0, 0, 0], rotation: [0, 0, 0] } }), 'fixture.update|f1|position,rotation');
  assert.equal(historyKey({ t: 'fixture.add', type: 'x' }), null);
  assert.equal(historyLabel({ t: 'fixtures.update', items: [{ id: 'a', changes: { position: [1, 2, 3] } }] }), 'le déplacement');
  assert.equal(historyLabel({ t: 'fixture.update', id: 'a', changes: { settings: { dmxAddress: 5 } } }), 'l’adressage DMX');
  assert.equal(historyLabel({ t: 'fixtures.add', items: [{}, {}, {}] }), 'l’ajout de 3 projecteurs');
});

test('un geste continu ne fait qu’une étape, annuler puis rétablir', () =>
  withStore((open) => {
    const store = open();
    const history = new History();
    let f;
    apply(history, store, { t: 'fixture.add', type: TYPE }, () => (f = store.addFixture({ type: TYPE })), 1000);
    // glisser : 5 envois rapprochés → une seule étape
    for (let i = 1; i <= 5; i++) apply(history, store, { t: 'fixture.update', id: f.id, changes: { position: [i, 1.5, 0] } }, () => store.updateFixture(f.id, { position: [i, 1.5, 0] }), 2000 + i * 70);
    assert.equal(history.undoStack.length, 2);
    assert.equal(history.status().undoLabel, 'le déplacement');
    // annuler le déplacement : retour à la position d'origine
    const step = history.undo(store.scene, store.revision);
    store.restore(step.json, step.revision);
    assert.deepEqual(store.getFixture(f.id).position, [0, 1.5, 0]);
    assert.equal(history.status().redoLabel, 'le déplacement');
    const redo = history.redo(store.scene, store.revision);
    store.restore(redo.json, redo.revision);
    assert.deepEqual(store.getFixture(f.id).position, [5, 1.5, 0]);
    // une modification sans effet ne crée pas d'étape
    assert.equal(apply(history, store, { t: 'fixture.update', id: f.id, changes: { name: store.getFixture(f.id).name } }, () => store.updateFixture(f.id, { name: store.getFixture(f.id).name }), 9000), 'noop');
    assert.equal(history.undoStack.length, 2);
  }));

test('annuler garde l’environnement et le nom actuels', () =>
  withStore((open) => {
    const store = open();
    const history = new History();
    apply(history, store, { t: 'fixture.add', type: TYPE }, () => store.addFixture({ type: TYPE }), 1000);
    store.updateEnvironment({ haze: 0.9 });
    store.rename('Plateau B');
    const step = history.undo(store.scene, store.revision);
    store.restore(step.json, step.revision);
    assert.equal(store.scene.fixtures.length, 0);
    assert.equal(store.scene.environment.haze, 0.9);
    assert.equal(store.scene.name, 'Plateau B');
  }));

test('état « enregistrée » : modifications, enregistrement, annulation, relance', () =>
  withStore((open) => {
    const store = open();
    const history = new History();
    assert.equal(store.dirty, false);
    apply(history, store, { t: 'fixture.add', type: TYPE }, () => store.addFixture({ type: TYPE }), 1000);
    assert.equal(store.dirty, true);
    store.save('Essai');
    assert.equal(store.dirty, false);
    assert.equal(store.savedName, 'Essai');
    assert.ok(store.exists('Essai'));
    apply(history, store, { t: 'fixture.add', type: TYPE }, () => store.addFixture({ type: TYPE }), 5000);
    assert.equal(store.dirty, true);
    const step = history.undo(store.scene, store.revision);
    store.restore(step.json, step.revision);
    assert.equal(store.dirty, false); // retour à l'état enregistré
    // la sauvegarde automatique garde l'état « enregistrée ou non » d'une session à l'autre
    store.flush();
    const again = open();
    assert.equal(again.savedName, 'Essai');
    assert.equal(again.dirty, false);
    again.addFixture({ type: TYPE });
    again.flush();
    assert.equal(open().dirty, true);
    assert.ok(!again.list().includes('_autosave'));
  }));
