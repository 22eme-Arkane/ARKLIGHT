// Tests de l'export MVR (et des fichiers GDTF qu'il contient) et de l'archive ZIP.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { SceneStore } from '../server/scene-store.js';
import { buildDemo } from '../server/demo-project.js';
import { buildMvr, mvrMatrix, uuidFrom } from '../shared/mvr.js';
import { createZip, readZip } from '../shared/zip.js';

const dec = new TextDecoder();

/** XML bien formé : balises équilibrées (contrôle simple, sans analyseur). */
function wellFormed(xml) {
  const stack = [];
  for (const m of xml.replace(/<\?xml[^>]*\?>/, '').matchAll(/<(\/?)([A-Za-z_][\w.-]*)([^>]*?)(\/?)>/g)) {
    const [, close, name, , self] = m;
    if (self) continue;
    if (close) {
      if (stack.pop() !== name) return false;
    } else stack.push(name);
  }
  return stack.length === 0;
}

function demoScene() {
  const dir = mkdtempSync(join(tmpdir(), 'arklight-mvr-'));
  try {
    const store = new SceneStore(dir);
    const scene = structuredClone(buildDemo(store));
    clearTimeout(store._timer);
    return scene;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('zip : écrit puis relu, avec le bon CRC', () => {
  const zip = createZip([
    { name: 'a.txt', data: 'bonjour' },
    { name: 'éè.xml', data: new Uint8Array([1, 2, 3]) },
  ]);
  const files = readZip(zip);
  assert.deepEqual(files.map((f) => f.name), ['a.txt', 'éè.xml']);
  assert.equal(dec.decode(files[0].data), 'bonjour');
  assert.deepEqual([...files[1].data], [1, 2, 3]);
});

test('MVR : description de la scène, un GDTF par modèle, adresses et numéros de fixture', () => {
  const scene = demoScene();
  const { bytes, fixtures, skipped } = buildMvr(scene, { providerVersion: 'test' });
  assert.equal(fixtures, scene.fixtures.length);
  assert.deepEqual(skipped, []);
  const files = readZip(bytes);
  const root = files.find((f) => f.name === 'GeneralSceneDescription.xml');
  assert.ok(root, 'fichier racine');
  const xml = dec.decode(root.data);
  assert.ok(wellFormed(xml), 'XML de la scène bien formé');
  assert.match(xml, /<GeneralSceneDescription verMajor="1" verMinor="6"/);
  assert.equal(xml.match(/<Fixture /g).length, scene.fixtures.length);
  // chaque fichier GDTF cité est présent, bien formé et contient le mode utilisé
  for (const spec of new Set([...xml.matchAll(/<GDTFSpec>([^<]+)<\/GDTFSpec>/g)].map((m) => m[1]))) {
    const gdtf = files.find((f) => f.name === spec);
    assert.ok(gdtf, `${spec} présent`);
    const desc = readZip(gdtf.data).find((f) => f.name === 'description.xml');
    const gx = dec.decode(desc.data);
    assert.ok(wellFormed(gx), `${spec} bien formé`);
    assert.match(gx, /<GDTF DataVersion="1.2">/);
  }
  // patch : adresse absolue (univers − 1) × 512 + adresse, numéro de fixture
  const s360 = scene.fixtures.find((f) => f.type === 'arri.skypanel-s360c');
  const block = xml.slice(xml.indexOf(`name="${s360.name}"`));
  assert.match(block, new RegExp(`<Address break="0">${(s360.settings.universe - 1) * 512 + s360.settings.dmxAddress}</Address>`));
  assert.match(block, new RegExp(`<FixtureIDNumeric>${s360.settings.fixtureNumber}</FixtureIDNumeric>`));
  // tous les fichiers à la racine de l'archive
  assert.ok(files.every((f) => !f.name.includes('/')));
});

test('MVR : repère Z en haut, millimètres, faisceau vers le public', () => {
  // projecteur non tourné, à 2 m de haut, 3 m vers le public : faisceau GDTF (−Z local) → −Y MVR
  assert.equal(mvrMatrix([1, 2, 3], [0, 0, 0]), '{1,0,0}{0,0,-1}{0,1,0}{1000,-3000,2000}');
  // tourné vers le bas (rotation X de 90°) : le faisceau va vers −Z (le sol)
  const m = mvrMatrix([0, 3, 0], [Math.PI / 2, 0, 0]).match(/\{([^}]+)\}/g).map((v) => v.slice(1, -1).split(',').map(Number));
  const w = m[2];
  assert.deepEqual(w.map((v) => Math.round(-v) + 0), [0, 0, -1], 'faisceau vers le sol');
  assert.match(uuidFrom('fixture:a'), /^[0-9A-F]{8}-[0-9A-F]{4}-5[0-9A-F]{3}-[89AB][0-9A-F]{3}-[0-9A-F]{12}$/);
  assert.equal(uuidFrom('x'), uuidFrom('x'));
  assert.notEqual(uuidFrom('x'), uuidFrom('y'));
});
