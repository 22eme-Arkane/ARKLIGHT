// Tests du plan de feu et du grip côté serveur (scène : objets, éléments du plan, image de fond) et des pieds.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { SceneStore } from '../server/scene-store.js';
import { FIXTURE_TYPES, getFixtureType } from '../shared/fixtures/index.js';
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
    assert.deepEqual(store.scene.objects.map((o) => o.name), ['Comédien'], 'reste le comédien de la scène neuve');
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
    // scène d'une version antérieure : plan ajouté, personnage du décor fixe devenu un objet déplaçable
    const old = store._sanitize({ fixtures: [] });
    assert.deepEqual(old.objects.map((o) => [o.type, o.name, o.position]), [['person-man', 'Comédien', [0, 0, 0]]]);
    assert.equal(old.environment.actor, undefined);
    assert.deepEqual(old.plot.items, []);
    assert.equal(old.plot.background, null);
    assert.deepEqual(store._sanitize({ fixtures: [], environment: { actor: false } }).objects, [], 'personnage masqué : pas ajouté');
    store.updatePlotMeta({ background: '#FFFFFF' });
    assert.equal(store.scene.plot.background, '#ffffff');
    store.updatePlotMeta({ background: 'rouge' });
    assert.equal(store.scene.plot.background, null);
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

test('décor : objets de la bibliothèque, réglages nettoyés, numéros de fixture', () =>
  withStore((store) => {
    const dog = store.addObject({ type: 'dog', position: [1, 0, 1], props: { color: '#ABCDEF', inconnu: 1 } });
    assert.equal(dog.name, 'Chien 1');
    assert.deepEqual(dog.props, { color: '#abcdef' });
    const p = store.addObject({ type: 'person-woman' });
    store.updateObject(p.id, { props: { pose: 'sitting', height: 999 } });
    assert.equal(store.getObject(p.id).props.pose, 'sitting');
    assert.equal(store.getObject(p.id).props.height, 220, 'taille bornée');
    store.updateObject(p.id, { props: { pose: 'danse' } });
    assert.equal(store.getObject(p.id).props.pose, 'sitting', 'pose inconnue ignorée');
    // numéros de fixture : à la suite, réglables, effaçables
    const a = store.addFixture({ type: FIXTURE_TYPES.find((t) => t.dmx?.modes?.length).id });
    const b = store.addFixture({ type: a.type });
    assert.equal(b.settings.fixtureNumber, a.settings.fixtureNumber + 1);
    store.updateFixture(b.id, { settings: { fixtureNumber: '101.6' } });
    assert.equal(store.getFixture(b.id).settings.fixtureNumber, 102);
    store.updateFixture(b.id, { settings: { fixtureNumber: '' } });
    assert.equal(store.getFixture(b.id).settings.fixtureNumber, null);
    const c = store.addFixture({ type: a.type, settings: { fixtureNumber: 7 } });
    assert.equal(c.settings.fixtureNumber, 7);
  }));
