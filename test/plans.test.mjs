// Séquences et plans : chaque plan garde son plan de feu, l'enregistrement les conserve tous.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { SceneStore } from '../server/scene-store.js';
import { currentPlan, flatPlans, neighborPlan, nextNumber } from '../shared/plans.js';

function withStore(fn) {
  const dir = mkdtempSync(join(tmpdir(), 'ark-plans-'));
  const store = new SceneStore(dir);
  try {
    return fn(store, dir);
  } finally {
    clearTimeout(store._timer);
    rmSync(dir, { recursive: true, force: true });
  }
}

test('nouveau projet : une séquence, un plan', () => {
  withStore((store) => {
    const info = store.planInfo();
    assert.equal(info.sequences.length, 1);
    assert.equal(info.sequences[0].plans.length, 1);
    assert.equal(info.current.plan, info.sequences[0].plans[0].id);
    assert.equal(info.sequences[0].name, 'Séquence 1');
    assert.equal(info.sequences[0].plans[0].name, 'Plan 1');
  });
});

test('nouveau plan (copie) : plan de feu indépendant, retour au premier intact', () => {
  withStore((store) => {
    const f = store.addFixture({ type: 'arri.skypanel-s60c', position: [1, 2, 3] });
    const first = store.current.plan;
    const p2 = store.newPlan({ mode: 'copy' });
    assert.equal(p2.name, 'Plan 2');
    assert.equal(store.current.plan, p2.id);
    assert.equal(store.scene.fixtures.length, 1);
    store.updateFixture(f.id, { position: [5, 2, 3] });
    store.addFixture({ type: 'aputure.ls-600d-pro' });
    assert.equal(store.scene.fixtures.length, 2);
    store.switchPlan(first);
    assert.equal(store.scene.fixtures.length, 1);
    assert.deepEqual(store.scene.fixtures[0].position, [1, 2, 3]);
    store.switchPlan(p2.id);
    assert.deepEqual(store.getFixture(f.id).position, [5, 2, 3]);
  });
});

test('le nom du projet et le cartouche suivent d’un plan à l’autre', () => {
  withStore((store) => {
    const first = store.current.plan;
    store.newPlan({ mode: 'empty' });
    store.rename('Tournage');
    store.updatePlotMeta({ title: { production: 'Clip', author: 'Sam' } });
    store.switchPlan(first);
    assert.equal(store.scene.name, 'Tournage');
    assert.equal(store.scene.plot.title.production, 'Clip');
    assert.equal(store.scene.plot.title.author, 'Sam');
  });
});

test('nouvelle séquence « même décor » : le décor sans projecteurs ni grip', () => {
  withStore((store) => {
    store.addFixture({ type: 'arri.skypanel-s60c' });
    store.addObject({ type: 'sofa-3' });
    store.addObject({ type: 'frame-8x8' });
    const sq = store.newSequence({ mode: 'decor' });
    assert.equal(store.sequences.length, 2);
    assert.equal(store.current.seq, sq.id);
    assert.equal(sq.name, 'Séquence 2');
    assert.equal(store.scene.fixtures.length, 0);
    const types = store.scene.objects.map((o) => o.type);
    assert.ok(types.includes('sofa-3'));
    assert.ok(!types.includes('frame-8x8'));
  });
});

test('enregistrer puis rouvrir : séquences, plans et plan courant conservés', () => {
  withStore((store, dir) => {
    store.addFixture({ type: 'arri.skypanel-s60c' });
    store.newPlan({ mode: 'copy', name: 'Contrechamp' });
    store.addFixture({ type: 'aputure.ls-600d-pro' });
    store.newSequence({ mode: 'empty', name: 'Séq. 2 — rue' });
    store.addObject({ type: 'sofa-3' });
    const path = join(dir, 'Essai.json');
    store.saveTo(path);
    const data = JSON.parse(readFileSync(path, 'utf8'));
    assert.equal(data.format, 2);
    assert.equal(data.sequences.length, 2);
    // la scène du plan courant est au premier niveau (lisible par les versions précédentes)
    assert.equal(data.sequences[1].plans[0].scene, null);
    assert.ok(Array.isArray(data.fixtures));

    const other = new SceneStore(join(dir, 'autre'));
    try {
      other.loadFrom(path);
      const info = other.planInfo();
      assert.deepEqual(info.sequences.map((s) => s.name), ['Séquence 1', 'Séq. 2 — rue']);
      assert.deepEqual(info.sequences[0].plans.map((p) => p.name), ['Plan 1', 'Contrechamp']);
      assert.equal(currentPlan(info).seq.name, 'Séq. 2 — rue');
      assert.ok(other.scene.objects.some((o) => o.type === 'sofa-3'));
      other.switchPlan(info.sequences[0].plans[1].id);
      assert.equal(other.scene.fixtures.length, 2);
      assert.equal(other.scene.name, 'Essai');
    } finally {
      clearTimeout(other._timer);
    }
  });
});

test('supprimer le plan courant : on passe au voisin ; le dernier plan reste', () => {
  withStore((store) => {
    const first = store.current.plan;
    const p2 = store.newPlan({ mode: 'empty' });
    store.removePlan(p2.id);
    assert.equal(store.current.plan, first);
    assert.equal(flatPlans(store.planInfo()).length, 1);
    assert.throws(() => store.removePlan(first));
    const sq = store.newSequence({ mode: 'empty' });
    store.removeSequence(sq.id);
    assert.equal(store.sequences.length, 1);
    assert.equal(store.current.plan, first);
  });
});

test('projet d’avant les séquences : Séquence 1 › Plan 1, sans personnage ajouté', () => {
  withStore((store, dir) => {
    const path = join(dir, 'Ancien.json');
    writeFileSync(path, JSON.stringify({ app: 'ARKLIGHT', format: 1, name: 'Ancien', environment: {}, fixtures: [], objects: [], groups: [] }));
    store.loadFrom(path);
    const info = store.planInfo();
    assert.equal(info.sequences.length, 1);
    assert.equal(info.sequences[0].plans.length, 1);
    assert.equal(store.scene.objects.length, 0); // le comédien supprimé ne revient pas
  });
});

test('déplacer, renommer, dupliquer ; plan voisin et numérotation', () => {
  withStore((store) => {
    const first = store.current.plan;
    const p2 = store.newPlan({ mode: 'empty' });
    const p3 = store.newPlan({ mode: 'empty' });
    assert.equal(p3.name, 'Plan 3');
    store.movePlan(p3.id, { index: 0 });
    assert.deepEqual(store.sequences[0].plans.map((p) => p.id), [p3.id, first, p2.id]);
    store.renamePlan(first, 'Large');
    const copy = store.duplicatePlan(first);
    assert.equal(copy.name, 'Large (copie)');
    assert.equal(store.current.plan, p3.id); // dupliquer ne change pas de plan
    assert.equal(neighborPlan(store.planInfo(), 1).plan.id, first);
    assert.equal(neighborPlan(store.planInfo(), -1), null);
    assert.equal(nextNumber(['Plan 1', 'Plan 7 — gros plan', 'Large'], 3), 8);
  });
});
