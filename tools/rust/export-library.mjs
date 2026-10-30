// Version Rust d'ARKLIGHT : exporte la bibliothèque (projecteurs, décor, grip, pieds) en JSON pour
// rust/crates/arklight-core/data/library.json. Tant que la version JavaScript reste la référence,
// la bibliothèque n'est écrite qu'à un endroit (shared/) : relancer ce script après chaque ajout.
//   node tools/rust/export-library.mjs
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { FIXTURE_TYPES } from '../../shared/fixtures/index.js';
import { STATIC_COLORS } from '../../shared/fixtures/astera-colors.js';
import INDEX_COLORS from '../../shared/fixtures/data/astera-index-colors.js';
import { SUPPORTS } from '../../shared/fixtures/supports.js';
import { CLOTHS, GRIP_TYPES } from '../../shared/grip.js';
import { FILTERS } from '../../shared/filters.js';
import { GRIDS, MODIFIERS } from '../../shared/modifiers.js';
import { PROP_TYPES, defaultPropProps, propSize } from '../../shared/props.js';

const OUT = fileURLToPath(new URL('../../rust/crates/arklight-core/data/library.json', import.meta.url));
const list = (x) => (Array.isArray(x) ? x : Object.values(x));

const props = list(PROP_TYPES).map((t) => ({
  ...t,
  short: t.short || null,
  category: t.cat || null,
  model: t.model || null,
  // dimensions du catalogue, et encombrement avec les réglages par défaut (largeur, hauteur, profondeur en mètres)
  baseSize: t.size || null,
  size: propSize(t, defaultPropProps(t)),
  defaults: defaultPropProps(t),
}));
const grip = list(GRIP_TYPES).map((t) => ({ ...t }));
const supports = Object.entries(SUPPORTS).map(([id, s]) => ({ id, ...s }));

// couleurs index (gels LEE) et couleurs du menu STATIC COLOR des Astera, pour le moteur lumière
// toiles du grip, nuancier de filtres, accessoires des projecteurs (boîtes à lumière, dômes, grilles)
const data = { source: 'ARKLIGHT (JavaScript) shared/', fixtures: list(FIXTURE_TYPES), props, grip, supports, indexColors: INDEX_COLORS, staticColors: STATIC_COLORS, cloths: CLOTHS, filters: FILTERS, modifiers: MODIFIERS, grids: GRIDS };
writeFileSync(OUT, JSON.stringify(data));
console.log(`library.json : ${data.fixtures.length} projecteurs, ${props.length} objets de décor, ${grip.length} grip, ${supports.length} pieds`);
