// Version Rust d'ARKLIGHT : icônes de la bibliothèque de la version JavaScript (web/js/ui/library.js :
// ICONS, GRIP_ICON, PROP_ICON) en fichiers SVG pour l'interface Slint
// (rust/crates/arklight-desktop/ui/icons/lib/<groupe>-<nom>.svg), à la couleur d'accent.
//   node tools/rust/export-lib-icons.mjs
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const SRC = fileURLToPath(new URL('../../web/js/ui/library.js', import.meta.url));
const OUT = fileURLToPath(new URL('../../rust/crates/arklight-desktop/ui/icons/lib/', import.meta.url));
const ACCENT = '#ffad3b';
mkdirSync(OUT, { recursive: true });
const text = readFileSync(SRC, 'utf8');
let n = 0;
for (const [group, name] of [['fixture', 'ICONS'], ['grip', 'GRIP_ICON'], ['prop', 'PROP_ICON']]) {
  const body = text.slice(text.indexOf(`${name} = {`), text.indexOf('};', text.indexOf(`${name} = {`)));
  for (const m of body.matchAll(/^\s{2}'?([a-z][a-z-]*)'?: '(<svg[^']*)'/gm)) {
    const svg = m[2]
      .replace('<svg ', '<svg xmlns="http://www.w3.org/2000/svg" width="26" height="18" ')
      .replaceAll('currentColor', ACCENT);
    writeFileSync(`${OUT}${group}-${m[1]}.svg`, `${svg}\n`);
    n++;
  }
}
console.log(`${n} icônes → ${OUT}`);
