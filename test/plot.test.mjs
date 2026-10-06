// Tests du plan de feu et du grip côté serveur (scène : objets, éléments du plan, image de fond) et des pieds.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { SceneStore } from '../server/scene-store.js';
import { getFixtureType } from '../shared/fixtures/index.js';
import { resolveSupport } from '../shared/fixtures/supports.js';

function withStore(fn) {
  const dir = mkdtempSync(join(tmpdir(), 'arklight-plot-'));
  try {
    const store = new SceneStore(dir);
    fn(store);
    store.flush();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('grip : ajout avec garniture par défaut, mise à jour, suppression', () =>
  withStore((store) => {
    const frame = store.addObject({ type: 'frame-12x12', position: [1, 2.4, 2] });
    assert.equal(frame.name, 'Cadre 12×12 1');
    assert.deepEqual(frame.props, { cloth: 'silk' });
    store.updateObject(frame.id, { props: { cloth: 'solid' }, position: [0, 2, 0] });
    assert.equal(store.getObject(frame.id).props.cloth, 'solid');
    assert.deepEqual(store.getObject(frame.id).position, [0, 2, 0]);
    assert.throws(() => store.addObject({ type: 'inconnu' }));
    assert.ok(store.removeObject(frame.id));
    assert.equal(store.scene.objects.length, 0);
  }));

test('plan de feu : éléments nettoyés, image de fond limitée aux fichiers importés', () =>
  withStore((store) => {
    const it = store.addPlotItem({ kind: 'rect', points: [[0, 0], [2, 3]], text: 'Maison', height: 4, style: { width: 99 } });
    assert.equal(it.height, 4);
    assert.equal(it.style.width, 20, 'épaisseur bornée');
    assert.throws(() => store.addPlotItem({ kind: 'script', points: [[0, 0]] }));
    store.updatePlotItem(it.id, { text: 'Kiosque', style: { stroke: '#ff0000' } });
    assert.equal(store.scene.plot.items[0].text, 'Kiosque');
    assert.equal(store.scene.plot.items[0].style.stroke, '#ff0000');
    store.updatePlotMeta({ underlay: { url: 'https://exemple.com/x.png' } });
    assert.equal(store.scene.plot.underlay, null, 'seules les images importées (/assets/) sont acceptées');
    store.updatePlotMeta({ underlay: { url: '/assets/a.png', width: 20, aspect: 1.5 }, title: { production: 'Pub' } });
    assert.equal(store.scene.plot.underlay.width, 20);
    assert.equal(store.scene.plot.title.production, 'Pub');
    assert.ok(store.removePlotItem(it.id));
    // scène d'une version antérieure : plan et grip ajoutés au chargement
    const old = store._sanitize({ fixtures: [] });
    assert.deepEqual(old.objects, []);
    assert.deepEqual(old.plot.items, []);
  }));

test('pieds : choix automatique selon le poids, la hauteur et le montage', () => {
  const titan = getFixtureType('astera.titan');
  const m18 = getFixtureType('arri.m18');
  assert.equal(resolveSupport(titan, 'auto', 1.5).id, 'stand-1000');
  assert.equal(resolveSupport(titan, 'auto', 0.3).id, 'none', 'posé au sol');
  assert.equal(resolveSupport(m18, 'auto', 2.5).id, 'stand-junior', 'broche 28 mm');
  assert.equal(resolveSupport(m18, 'auto', 4.2).id, 'windup');
  const forced = resolveSupport(m18, 'stand-1000', 2.5);
  assert.ok(forced.warnings.some((w) => w.includes('charge')), 'M18 trop lourd pour un pied de 1000');
});
