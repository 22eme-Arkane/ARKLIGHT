// Décor : chaque objet du catalogue se construit en 3D, à peu près aux dimensions annoncées.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from 'three';
import { PROP_CATEGORIES, PROP_TYPES, POSES, cleanPropProps, defaultPropProps, propSize } from '../shared/props.js';
import { getObjectType } from '../shared/objects.js';
import { buildProp } from '../web/js/viewport/prop-models.js';

test('catalogue : identifiants uniques, catégories connues, réglages par défaut valides', () => {
  const ids = PROP_TYPES.map((t) => t.id);
  assert.equal(new Set(ids).size, ids.length);
  const cats = new Set(PROP_CATEGORIES.map((c) => c.id));
  for (const t of PROP_TYPES) {
    assert.ok(cats.has(t.cat), `${t.id} : catégorie ${t.cat}`);
    assert.ok(getObjectType(t.id), t.id);
    const d = defaultPropProps(t);
    assert.deepEqual(cleanPropProps(t, d), d, `${t.id} : réglages par défaut hors bornes`);
  }
  assert.ok(PROP_TYPES.length >= 80, `${PROP_TYPES.length} objets`);
});

test('chaque objet se construit, posé au sol, à la taille du catalogue', () => {
  for (const t of PROP_TYPES) {
    const props = defaultPropProps(t);
    const { group, materials, aim } = buildProp(t, props);
    assert.ok(materials.length > 0, `${t.id} : aucune matière`);
    group.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(group);
    const size = box.getSize(new THREE.Vector3());
    const [w, h, d] = propSize(t, props);
    assert.ok(box.min.y > -0.05 && box.min.y < 0.12, `${t.id} : bas à ${box.min.y.toFixed(3)} m`);
    assert.ok(Math.abs(size.y - h) / h < 0.35, `${t.id} : hauteur ${size.y.toFixed(2)} m pour ${h} m`);
    assert.ok(Math.abs(Math.max(size.x, size.z) - Math.max(w, d)) / Math.max(w, d) < 0.45, `${t.id} : emprise ${size.x.toFixed(2)} × ${size.z.toFixed(2)} pour ${w} × ${d}`);
    assert.ok(aim instanceof THREE.Vector3, `${t.id} : point visé`);
    for (const m of materials) m.dispose();
  }
});

test('personnage : toutes les poses, pieds au sol', () => {
  const t = PROP_TYPES.find((x) => x.id === 'person-man');
  for (const [pose] of POSES) {
    const props = { ...defaultPropProps(t), pose };
    const { group } = buildProp(t, props);
    group.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(group);
    assert.ok(box.min.y > -0.03 && box.min.y < 0.05, `${pose} : bas à ${box.min.y.toFixed(3)}`);
    const [, h] = propSize(t, props);
    assert.ok(Math.abs(box.max.y - h) / h < 0.2, `${pose} : haut à ${box.max.y.toFixed(2)} pour ${h.toFixed(2)}`);
  }
});
