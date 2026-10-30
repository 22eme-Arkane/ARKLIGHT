// Tests de la traduction : dictionnaire anglais, gabarits {0}, textes composés reconnus.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { EN } from '../shared/i18n/en.js';
import { lookup, translate, translateMarkup } from '../shared/i18n.js';
import { COMMANDS, MENUS } from '../web/js/commands.js';

const slots = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');

test('dictionnaire : pas de clé en double, mêmes parties variables en français et en anglais', () => {
  const src = readFileSync(new URL('../shared/i18n/en.js', import.meta.url), 'utf8');
  const keys = [...src.matchAll(/^ {2}('(?:[^'\\]|\\.)*'|[\p{L}\d_]+):/gmu)].map((m) => m[1].replace(/^'|'$/g, '').replace(/\\'/g, "'").replace(/\\n/g, '\n'));
  const seen = new Set();
  const dup = keys.filter((k) => (seen.has(k) ? true : (seen.add(k), false)));
  assert.deepEqual(dup, [], 'clés en double');
  // l'anglais peut laisser tomber une marque du pluriel français (« ré-adressé{2} »), pas en inventer
  for (const [fr, en] of Object.entries(EN)) {
    const known = new Set(slots(fr).split(','));
    for (const s of slots(en).split(',').filter(Boolean)) assert.ok(known.has(s), `partie variable {${s}} inconnue en français : « ${fr} »`);
  }
});

test('traduction : texte exact, paramètres, texte composé reconnu et ses parties traduites', () => {
  assert.equal(translate('Nouveau projet', 'en'), 'New project');
  assert.equal(translate('Nouveau projet', 'fr'), 'Nouveau projet');
  assert.equal(translate('Projet enregistré : {0}', 'en', { 0: 'Pub' }), 'Project saved: Pub');
  assert.equal(translate('Projet enregistré : {0}', 'fr', { 0: 'Pub' }), 'Projet enregistré : Pub');
  assert.equal(lookup('Projet enregistré : Pub voiture', 'en'), 'Project saved: Pub voiture');
  assert.equal(lookup('Annuler le déplacement', 'en'), 'Undo the move');
  assert.equal(lookup('Modifié il y a 5 min · 15 projecteurs', 'en'), 'Modified 5 min ago · 15 fixtures');
  assert.equal(lookup('texte inconnu du dictionnaire', 'en'), null);
  // un gabarit trop court (« le {0} ») ne sert pas à reconnaître n'importe quelle phrase
  assert.equal(lookup('le projecteur bouge', 'en'), null);
  assert.equal(translateMarkup('<text x="1">Cyclo (fond)</text><g title="Plan de feu"></g>', 'en'), '<text x="1">Cyc (backdrop)</text><g title="Lighting plot"></g>');
});

test('une traduction ne se retraduit pas elle-même (pas de « Flight case case … »)', () => {
  const bad = [];
  for (const fr of Object.keys(EN)) {
    if (!fr.includes('{')) continue;
    const sample = fr.replace(/\{(\w+)\}/g, 'Zq7');
    const once = lookup(sample, 'en');
    if (once == null) continue;
    const twice = lookup(once, 'en');
    if (twice != null && twice !== once) bad.push(`${fr} → ${once} → ${twice}`);
  }
  assert.deepEqual(bad, []);
});

test('menus et commandes : tous traduits en anglais', () => {
  const missing = [...MENUS.map((m) => m.label), ...COMMANDS.flatMap((c) => (c.en ? [c.section] : [c.label, c.section]))].filter((s) => lookup(s, 'en') == null);
  assert.deepEqual([...new Set(missing)], []);
});
