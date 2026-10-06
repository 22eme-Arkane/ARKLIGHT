// Tests de l'adressage automatique (conflits, ré-adressage) et des groupes de la scène.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { alignUp, findPatchConflicts, footprintOf, planAddressing } from '../shared/engine/addressing.js';
import { defaultSettings, getFixtureType } from '../shared/fixtures/index.js';
import { SceneStore } from '../server/scene-store.js';

let seq = 0;
function fx(typeId, universe, address, extra = {}) {
  seq++;
  return {
    id: `t${seq}`,
    type: typeId,
    name: extra.name || `${typeId} ${seq}`,
    position: extra.position || [0, 1, 0],
    rotation: [0, 0, 0],
    settings: { ...defaultSettings(getFixtureType(typeId)), universe, dmxAddress: address, ...(extra.settings || {}) },
  };
}

/** Applique un plan à une copie des projecteurs. */
function apply(fixtures, plan) {
  const byId = new Map(plan.moves.map((m) => [m.id, m.to]));
  return fixtures.map((f) => (byId.has(f.id) ? { ...f, settings: { ...f.settings, universe: byId.get(f.id).universe, dmxAddress: byId.get(f.id).address } } : f));
}

test('détection des chevauchements et des dépassements', () => {
  const a = fx('astera.titan', 1, 1);
  const fp = footprintOf(a);
  const b = fx('astera.titan', 1, fp); // chevauche le dernier canal de a
  const c = fx('astera.titan', 1, fp + 1 + fp); // libre
  const d = fx('astera.titan', 1, 512); // dépasse 512 (fp > 1)
  const e = fx('astera.titan', 2, 1); // autre univers
  const box = fx('astera.art7', 1, 1); // AsteraBox : pas de canaux
  const r = findPatchConflicts([a, b, c, d, e, box]);
  assert.deepEqual(r.overlaps, [[a.id, b.id]]);
  assert.deepEqual(r.overflow, [d.id]);
  assert.deepEqual([...r.ids].sort(), [a.id, b.id, d.id].sort());
});

test('correction des conflits : le premier garde sa place, les autres vont à la première place libre', () => {
  const a = fx('astera.titan', 1, 1);
  const b = fx('astera.titan', 1, 1);
  const c = fx('astera.titan', 1, 1);
  const free = fx('astera.titan', 1, 100);
  const fp = footprintOf(a);
  const plan = planAddressing([a, b, c, free], { scope: 'conflicts' });
  assert.equal(plan.conflictsBefore, 3);
  assert.equal(plan.conflictsAfter, 0);
  assert.equal(plan.moves.length, 2, 'seuls b et c bougent');
  assert.deepEqual(plan.moves.map((m) => m.to), [
    { universe: 1, address: 1 + fp },
    { universe: 1, address: 1 + 2 * fp },
  ]);
  assert.ok(!plan.moves.some((m) => m.id === free.id), 'un projecteur sans conflit ne bouge pas');
});

test('correction d’un dépassement : retour dans le premier trou de l’univers', () => {
  const a = fx('astera.titan', 1, 1);
  const fp = footprintOf(a);
  const over = fx('astera.titan', 1, 511);
  const plan = planAddressing([a, over], { scope: 'conflicts' });
  assert.deepEqual(plan.moves, [{ id: over.id, name: over.name, footprint: fp, from: { universe: 1, address: 511 }, to: { universe: 1, address: 1 + fp } }]);
});

test('ré-adressage complet : ordre par nom, adresses alignées sur les dizaines', () => {
  const list = [fx('astera.titan', 1, 300, { name: 'Tube 10' }), fx('astera.titan', 1, 7, { name: 'Tube 2' }), fx('astera.titan', 1, 50, { name: 'Tube 1' })];
  const plan = planAddressing(list, { scope: 'all', order: 'name', align: 10 });
  const after = apply(list, plan);
  const addr = Object.fromEntries(after.map((f) => [f.name, f.settings.dmxAddress]));
  assert.deepEqual(addr, { 'Tube 1': 1, 'Tube 2': 11, 'Tube 10': 21 });
  assert.equal(alignUp(12, 10), 21);
  assert.equal(alignUp(11, 10), 11);
  assert.equal(alignUp(5, 0), 5);
});

test('ré-adressage : passage à l’univers suivant quand l’univers est plein', () => {
  const one = fx('astera.titan', 1, 1);
  const fp = footprintOf(one);
  const perUniverse = Math.floor(512 / fp);
  const list = Array.from({ length: perUniverse + 3 }, () => fx('astera.titan', 1, 1));
  const plan = planAddressing(list, { scope: 'all', universes: 'pack', universe: 1, address: 1 });
  const after = apply(list, plan);
  assert.equal(after.filter((f) => f.settings.universe === 1).length, perUniverse);
  assert.deepEqual(
    after.filter((f) => f.settings.universe === 2).map((f) => f.settings.dmxAddress),
    [1, 1 + fp, 1 + 2 * fp],
  );
  assert.equal(plan.conflictsAfter, 0);
  // sans débordement : les projecteurs en trop ne sont pas placés
  const strict = planAddressing(list, { scope: 'all', universes: 'pack', overflow: false });
  assert.equal(strict.unplaced.length, 3);
});

test('ré-adressage d’une sélection : les autres projecteurs sont contournés', () => {
  const fixed = fx('astera.titan', 1, 1);
  const fp = footprintOf(fixed);
  const s1 = fx('astera.titan', 3, 200, { position: [2, 1, 0] });
  const s2 = fx('astera.titan', 3, 100, { position: [-2, 1, 0] });
  const plan = planAddressing([fixed, s1, s2], { scope: 'ids', ids: [s1.id, s2.id], order: 'position', universes: 'pack', universe: 1, address: 1 });
  const after = apply([fixed, s1, s2], plan);
  assert.equal(after[0].settings.dmxAddress, 1, 'le projecteur hors sélection ne bouge pas');
  // de gauche à droite : s2 (x = −2) puis s1 (x = 2), après les canaux du projecteur fixe
  assert.deepEqual([after[2].settings.universe, after[2].settings.dmxAddress], [1, 1 + fp]);
  assert.deepEqual([after[1].settings.universe, after[1].settings.dmxAddress], [1, 1 + 2 * fp]);
});

test('ré-adressage « garder l’univers » : chaque univers repart de l’adresse de départ', () => {
  const a = fx('astera.titan', 1, 40);
  const b = fx('astera.titan', 2, 90);
  const plan = planAddressing([a, b], { scope: 'all', universes: 'keep', address: 1 });
  const after = apply([a, b], plan);
  assert.deepEqual(after.map((f) => [f.settings.universe, f.settings.dmxAddress]), [
    [1, 1],
    [2, 1],
  ]);
});

test('groupes de la scène : création, mise à jour, nettoyage à la suppression', () => {
  const dir = mkdtempSync(join(tmpdir(), 'arklight-'));
  try {
    const store = new SceneStore(dir);
    const f1 = store.addFixture({ type: 'astera.titan' });
    const f2 = store.addFixture({ type: 'astera.titan' });
    const f3 = store.addFixture({ type: 'astera.helios' });
    const g = store.createGroup({ ids: [f1.id, f2.id, 'inconnu'] });
    assert.equal(g.name, 'Groupe 1');
    assert.deepEqual(g.ids, [f1.id, f2.id]);
    const g2 = store.createGroup({ name: '  Contre  ', ids: [f3.id] });
    assert.equal(g2.name, 'Contre');
    store.updateGroup(g.id, { add: [f3.id, f1.id], remove: [f2.id], name: 'Face' });
    assert.deepEqual(store.scene.groups[0], { id: g.id, name: 'Face', ids: [f1.id, f3.id] });
    store.removeFixture(f3.id);
    assert.deepEqual(store.scene.groups[0].ids, [f1.id]);
    assert.deepEqual(store.scene.groups[1].ids, [], 'un groupe vidé est conservé');
    assert.ok(store.removeGroup(g2.id));
    assert.equal(store.scene.groups.length, 1);
    // nettoyage d'une scène enregistrée par une version antérieure (sans groupes) ou incohérente
    const clean = store._sanitize({ fixtures: [{ id: 'a', type: 'astera.titan' }], groups: [{ id: 'g', name: 'X', ids: ['a', 'b', 'a'] }, { name: 'sans id' }] });
    assert.deepEqual(clean.groups, [{ id: 'g', name: 'X', ids: ['a'] }]);
    assert.deepEqual(store._sanitize({ fixtures: [] }).groups, []);
    store.flush();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
