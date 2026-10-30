// Noms des catalogues (décor, grip, pieds, filtres, accessoires, catégories) pas encore traduits.
import { EN } from '../../shared/i18n/en.js';
import { lookup } from '../../shared/i18n.js';
import { PROP_CATEGORIES, PROP_TYPES, POSES, BUILDS, HAIRS, SKINS } from '../../shared/props.js';
import { CLOTHS, GRIP_FAMILIES, GRIP_TYPES } from '../../shared/grip.js';
import { SUPPORTS, SUPPORT_CHOICES } from '../../shared/fixtures/supports.js';
import * as filters from '../../shared/filters.js';
import * as modifiers from '../../shared/modifiers.js';
import * as fixtures from '../../shared/fixtures/index.js';

const out = new Set();
const add = (s) => {
  if (typeof s === 'string' && /[A-Za-zÀ-ÿ]/.test(s) && !(s in EN) && lookup(s, 'en') == null) out.add(s);
};
const walk = (v, depth = 0) => {
  if (depth > 4 || v == null) return;
  if (typeof v === 'string') return add(v);
  if (Array.isArray(v)) return v.forEach((x) => walk(x, depth + 1));
  if (typeof v === 'object') for (const [k, x] of Object.entries(v)) if (['name', 'short', 'label', 'title', 'group', 'kind', 'note', 'cat', 'options'].includes(k) || Array.isArray(x) || depth < 1) walk(x, depth + 1);
};
for (const c of PROP_CATEGORIES) add(c.label);
for (const t of PROP_TYPES) {
  add(t.name);
  add(t.short);
  for (const p of t.params || []) {
    add(p.label);
    for (const o of p.options || []) add(o[1]);
  }
}
for (const x of [POSES, BUILDS, HAIRS, SKINS]) walk(x);
for (const t of GRIP_TYPES) {
  add(t.name);
  add(t.short);
  add(t.stands?.label);
}
for (const f of GRIP_FAMILIES) add(f.label);
for (const c of Object.values(CLOTHS)) add(c.name);
for (const s of Object.values(SUPPORTS)) {
  add(s.name);
  add(s.short);
}
for (const [, l] of SUPPORT_CHOICES) add(l);
for (const v of Object.values(filters)) walk(v);
for (const v of Object.values(modifiers)) walk(v);
for (const t of fixtures.FIXTURE_TYPES || []) {
  add(t.category);
  add(t.categoryLabel);
}
for (const v of Object.values(fixtures)) if (Array.isArray(v) && v.length < 60) walk(v);
console.log(`${out.size} noms à traduire`);
for (const s of out) console.log(JSON.stringify(s));
