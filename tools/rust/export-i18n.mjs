// Version Rust d'ARKLIGHT : dictionnaire anglais de l'interface (shared/i18n/en.js) en JSON pour
// rust/crates/arklight-desktop/data/en.json. Les modules portés (MIDI, plan 2D, menus des
// projecteurs…) produisent leurs textes en français, traduits par ce même dictionnaire.
//   node tools/rust/export-i18n.mjs
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { EN } from '../../shared/i18n/en.js';

const OUT = fileURLToPath(new URL('../../rust/crates/arklight-desktop/data/en.json', import.meta.url));
const sorted = Object.fromEntries(Object.entries(EN).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
writeFileSync(OUT, `${JSON.stringify(sorted, null, 0)}\n`);
console.log(`${Object.keys(sorted).length} textes → ${OUT}`);
