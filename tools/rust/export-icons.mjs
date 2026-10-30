// Version Rust d'ARKLIGHT : icônes des barres d'outils de la version JavaScript (web/js/ui/icons.js)
// en fichiers SVG pour l'interface Slint (rust/crates/arklight-desktop/ui/icons/<nom>.svg). Traits
// blancs : l'interface les teinte (propriété `colorize`).
//   node tools/rust/export-icons.mjs
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { iconSvg } from '../../web/js/ui/icons.js';

const SRC = fileURLToPath(new URL('../../web/js/ui/icons.js', import.meta.url));
const OUT = fileURLToPath(new URL('../../rust/crates/arklight-desktop/ui/icons/', import.meta.url));
mkdirSync(OUT, { recursive: true });
const names = [...readFileSync(SRC, 'utf8').matchAll(/^\s{2}'?([a-z][a-z-]*)'?:/gm)].map((m) => m[1]);
for (const name of names) {
  const svg = iconSvg(name, 24)
    .replace(' class="ico"', ' xmlns="http://www.w3.org/2000/svg"')
    .replace(' aria-hidden="true"', '')
    .replaceAll('currentColor', '#ffffff');
  writeFileSync(`${OUT}${name}.svg`, `${svg}\n`);
}
console.log(`${names.length} icônes → ${OUT}`);
