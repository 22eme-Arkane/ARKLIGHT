// Raccourcis clavier : lecture des touches (AZERTY), conflits, préférences mémorisées, commandes.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { COMMANDS, MENUS } from '../web/js/commands.js';
import { comboOf, keyLabel, migrateLegacy, normalizeCombo, overridesOf, resolveBindings } from '../web/js/shortcuts.js';

test('touches lues comme sur un clavier AZERTY', () => {
  assert.equal(comboOf({ key: 'd', code: 'KeyD', ctrlKey: true }), 'Ctrl+D');
  assert.equal(comboOf({ key: '&', code: 'Digit1' }), '1'); // « 1 » sans Maj
  assert.equal(comboOf({ key: '1', code: 'Digit1', shiftKey: true }), '1');
  assert.equal(comboOf({ key: 'S', code: 'KeyS', ctrlKey: true, shiftKey: true }), 'Ctrl+Shift+S');
  assert.equal(comboOf({ key: 'K', code: 'KeyK', ctrlKey: true, altKey: true, shiftKey: false }), 'Ctrl+Alt+K');
  assert.equal(comboOf({ key: ' ', code: 'Space' }), 'Space');
  assert.equal(comboOf({ key: 'Delete', code: 'Delete' }), 'Delete');
  assert.equal(comboOf({ key: 'F2', code: 'F2', shiftKey: true }), 'Shift+F2');
  assert.equal(comboOf({ key: 'Control', code: 'ControlLeft', ctrlKey: true }), null);
  assert.equal(comboOf({ key: 'Dead', code: 'BracketLeft' }), null); // accent circonflexe
});

test('libellés et combinaisons écrites à la main', () => {
  assert.equal(keyLabel('Ctrl+Shift+S'), 'Ctrl+Maj+S');
  assert.equal(keyLabel('Delete'), 'Suppr');
  assert.equal(keyLabel('Escape'), 'Échap');
  assert.equal(normalizeCombo('Shift+Ctrl+s'), 'Ctrl+Shift+S');
  assert.equal(normalizeCombo('Maj+Ctrl+Z'), 'Ctrl+Shift+Z');
  assert.equal(normalizeCombo('Ctrl+'), null);
});

test('commandes : identifiants uniques, menus connus, touches par défaut sans doublon', () => {
  const ids = COMMANDS.map((c) => c.id);
  assert.equal(new Set(ids).size, ids.length);
  const menus = new Set(MENUS.map((m) => m.id));
  for (const c of COMMANDS) assert.ok(menus.has(c.menu), `${c.id} : menu ${c.menu}`);
  const seen = new Map();
  for (const c of COMMANDS)
    for (const k of c.keys) {
      assert.equal(normalizeCombo(k), k, `${c.id} : ${k} n'est pas écrit dans l'ordre Ctrl, Alt, Shift`);
      assert.ok(!seen.has(k), `${k} est donné à ${seen.get(k)} et à ${c.id}`);
      seen.set(k, c.id);
    }
});

test('chaque commande a son action dans l’interface', () => {
  const app = readFileSync(new URL('../web/js/app.js', import.meta.url), 'utf8');
  for (const c of COMMANDS) assert.ok(app.includes(`'${c.id}':`), `pas d'action pour ${c.id}`);
});

test('touches personnalisées prioritaires, sans doublon', () => {
  const def = resolveBindings({});
  assert.deepEqual(def.get('edit.duplicate'), ['Ctrl+D']);
  assert.deepEqual(overridesOf(def), {});
  const map = resolveBindings({ 'fixture.aim': ['Ctrl+D', 'ctrl+d'] });
  assert.deepEqual(map.get('fixture.aim'), ['Ctrl+D']);
  assert.deepEqual(map.get('edit.duplicate'), []); // la touche est partie à « Viser le personnage »
  assert.deepEqual(overridesOf(map), { 'fixture.aim': ['Ctrl+D'], 'edit.duplicate': [] });
  // plusieurs touches pour une commande
  assert.deepEqual(def.get('edit.redo'), ['Ctrl+Y', 'Ctrl+Shift+Z']);
});

test('préférences de la première version : seules les vraies modifications sont reprises', () => {
  const out = migrateLegacy({ 'tool.translate': 'W', 'shortcuts.open': 'K', 'edit.duplicate': 'Ctrl+B', 'view.top': '', inconnu: 'X' });
  assert.deepEqual(out, { 'edit.duplicate': ['Ctrl+B'], 'view.top': [] });
});
