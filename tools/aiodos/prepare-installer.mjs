// Installeur d'ARKLIGHT : met l'installeur d'AIODOS dans build/aiodos/, pour la page facultative
// « AIODOS » (build/installer.nsh). Lancé par « npm run dist ».
//   node tools/aiodos/prepare-installer.mjs                 le plus récent AIODOS-Setup-*.exe de ../AIODOS/dist
//   AIODOS_SETUP=C:\…\AIODOS-Setup-1.2.0.exe node …         un installeur précis
//   node tools/aiodos/prepare-installer.mjs --without       installeur d'ARKLIGHT sans AIODOS
// Sans installeur d'AIODOS trouvé, ARKLIGHT est construit sans la page (rien n'échoue).
import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const OUT = join(ROOT, 'build', 'aiodos');
const TARGET = join(OUT, 'AIODOS-Setup.exe');

/** « 1.10.0 » après « 1.9.2 ». */
const version = (name) => (/AIODOS-Setup-([\d.]+)\.exe$/i.exec(name)?.[1] || '0').split('.').map(Number);
const newer = (a, b) => {
  const x = version(a);
  const y = version(b);
  for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) - (y[i] || 0);
  return 0;
};

function find() {
  if (process.env.AIODOS_SETUP) return existsSync(process.env.AIODOS_SETUP) ? process.env.AIODOS_SETUP : null;
  const dist = join(ROOT, '..', 'AIODOS', 'dist');
  if (!existsSync(dist)) return null;
  const list = readdirSync(dist).filter((f) => /^AIODOS-Setup-[\d.]+\.exe$/i.test(f));
  list.sort(newer);
  return list.length ? join(dist, list.at(-1)) : null;
}

const clear = () => rmSync(OUT, { recursive: true, force: true });

if (process.argv.includes('--without')) {
  clear();
  console.log('AIODOS : installeur d’ARKLIGHT construit sans la page AIODOS (--without)');
} else {
  const src = find();
  if (!src) {
    clear();
    console.log('AIODOS : aucun AIODOS-Setup-*.exe trouvé (../AIODOS/dist ou AIODOS_SETUP) : installeur sans la page AIODOS');
  } else {
    mkdirSync(OUT, { recursive: true });
    copyFileSync(src, TARGET);
    writeFileSync(join(OUT, 'SOURCE.txt'), `${basename(src)}\n${statSync(src).size} octets\n`);
    console.log(`AIODOS : ${basename(src)} sera proposé à l’installation (${Math.round(statSync(src).size / 1e6)} Mo)`);
  }
}
