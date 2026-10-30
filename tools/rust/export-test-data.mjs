// Version Rust d'ARKLIGHT : données d'essai de parité, calculées par la version JavaScript.
//   node tools/rust/export-test-data.mjs
// Écrit dans rust/crates/arklight-core/tests/data/ :
//   color.json        couleurs (Kelvin, teinte, couleur saturée) → résultats de shared/engine/color.js
//   demo-night.json   projets d'exemple, tels qu'ARKLIGHT les enregistre
//   demo-clip.json
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { cctToRgb, cctToXy, hsToRgb, linearToSrgb, srgbToLinear } from '../../shared/engine/color.js';
import { SceneStore } from '../../server/scene-store.js';
import { buildDemo } from '../../server/demo-project.js';

const OUT = fileURLToPath(new URL('../../rust/crates/arklight-core/tests/data/', import.meta.url));
mkdirSync(OUT, { recursive: true });

const kelvins = [1000, 1667, 1800, 2222, 2700, 3200, 4000, 4300, 5600, 6500, 10000, 20000, 25000, 30000];
const tints = [-1, -0.5, 0, 0.3, 1];
const color = {
  cct: kelvins.flatMap((k) => tints.map((t) => ({ k, t, rgb: cctToRgb(k, t) }))),
  xy: kelvins.map((k) => ({ k, xy: cctToXy(k) })),
  hs: [0, 30, 59.9, 60, 120, 180, 240, 300, 359, 400, -20].flatMap((h) => [0, 0.4, 1, 1.5].map((s) => ({ h, s, rgb: hsToRgb(h, s, cctToRgb(5600)) }))),
  srgb: [0, 0.002, 0.04045, 0.1, 0.5, 1].map((c) => ({ c, lin: srgbToLinear(c), back: linearToSrgb(c) })),
};
writeFileSync(join(OUT, 'color.json'), JSON.stringify(color));

const tmp = mkdtempSync(join(tmpdir(), 'arklight-demo-'));
try {
  for (const id of ['night', 'clip']) {
    const store = new SceneStore(join(tmp, id));
    store.lang = 'fr';
    buildDemo(store, 'fr', id);
    store.saveTo(join(OUT, `demo-${id}.json`), {});
  }
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
console.log(`données d'essai écrites dans ${OUT}`);
