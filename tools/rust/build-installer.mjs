// Version Rust d'ARKLIGHT : construit l’installeur Windows (rust/target/installer/ARKLIGHT-Setup-<version>.exe).
//   node tools/rust/build-installer.mjs              compile puis emballe
//   node tools/rust/build-installer.mjs --no-build   emballe le dernier programme compilé
// Installeur Inno Setup 6 dans le style sombre de celui d'AIODOS, aux couleurs d'ARKLIGHT
// (rust/installer/arklight.iss) : accueil avec le logo et la présentation, charte et conditions
// d'utilisation à accepter (rust/installer/legal, mises en page par tools/rust/installer_pages.py),
// AIODOS proposé si son installeur est prêt (build/aiodos/AIODOS-Setup.exe : node
// tools/aiodos/prepare-installer.mjs).
// Le programme est compilé à part (rust/target/dist) avec la bibliothèque C de Windows intégrée
// (crt-static) : rien à installer en plus sur l'ordinateur de l'utilisateur.
// Rien n'est publié : l'installeur reste sur ce PC.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../../', import.meta.url)).replace(/[\\/]$/, '');
const RUST = join(ROOT, 'rust');
const TARGET = join(RUST, 'target', 'dist');
const EXE = join(TARGET, 'release', 'arklight.exe');
const OUT_DIR = join(RUST, 'target', 'installer');
const STAGE = join(RUST, 'target', 'installer-stage');

const version = /\[workspace\.package\][^[]*?version\s*=\s*"([^"]+)"/s.exec(readFileSync(join(RUST, 'Cargo.toml'), 'utf8'))?.[1];
if (!version) throw new Error('version introuvable dans rust/Cargo.toml');

function run(cmd, args, env = {}, capture = false) {
  console.log(`> ${cmd} ${args.join(' ')}`);
  const r = spawnSync(cmd, args, { cwd: RUST, stdio: capture ? ['ignore', 'pipe', 'inherit'] : 'inherit', env: { ...process.env, ...env }, encoding: 'utf8' });
  if (r.status !== 0) process.exit(r.status ?? 1);
  return r.stdout;
}

/** ISCC.exe d'Inno Setup 6 (ARKLIGHT_ISCC=<chemin> pour un autre). */
function iscc() {
  const list = [process.env.ARKLIGHT_ISCC, 'C:\\Program Files (x86)\\Inno Setup 6\\ISCC.exe', 'C:\\Program Files\\Inno Setup 6\\ISCC.exe', join(process.env.LOCALAPPDATA || '', 'Programs', 'Inno Setup 6', 'ISCC.exe')];
  const found = list.find((p) => p && existsSync(p));
  if (!found) throw new Error('Inno Setup 6 introuvable (ISCC.exe) : https://jrsoftware.org/isdl.php, ou ARKLIGHT_ISCC');
  return found;
}

if (!process.argv.includes('--no-build')) {
  run('cargo', ['build', '--release', '--offline', '-p', 'arklight-desktop', '--target-dir', TARGET], {
    RUSTFLAGS: `${process.env.RUSTFLAGS || ''} -C target-feature=+crt-static`.trim(),
  });
}
if (!existsSync(EXE)) throw new Error(`programme absent : ${EXE}`);

// pages de l'assistant : charte et conditions (RTF), images tirées du logo
const pages = JSON.parse(run('python', ['-I', join(ROOT, 'tools', 'rust', 'installer_pages.py'), STAGE], {}, true));

// écrit dans un dossier à part puis déplacé : jamais d'installeur à moitié écrit (deux constructions en même temps)
const tmpDir = join(OUT_DIR, `.tmp-${process.pid}`);
mkdirSync(tmpDir, { recursive: true });
const aiodos = join(ROOT, 'build', 'aiodos', 'AIODOS-Setup.exe');
console.log(existsSync(aiodos) ? 'AIODOS : proposé à l’installation' : 'AIODOS : build/aiodos/AIODOS-Setup.exe absent, installeur sans AIODOS (node tools/aiodos/prepare-installer.mjs)');
const icon = join(RUST, 'crates', 'arklight-desktop', 'assets', 'arklight.ico');
run(iscc(), [
  '/Qp',
  `/DAppVersion=${version}`,
  `/DExe=${EXE}`,
  `/DRoot=${ROOT}`,
  `/DOutDir=${tmpDir}`,
  `/DIcon=${icon}`,
  `/DLegalFr=${pages.legal.fr}`,
  `/DLegalEn=${pages.legal.en}`,
  `/DWizardImages=${pages.large.join(',')}`,
  `/DWizardSmallImages=${pages.small.join(',')}`,
  ...(existsSync(aiodos) ? [`/DAiodosSetup=${aiodos}`] : []),
  join(RUST, 'installer', 'arklight.iss'),
]);
const name = `ARKLIGHT-Setup-${version}.exe`;
const out = join(OUT_DIR, name);
rmSync(out, { force: true });
renameSync(join(tmpDir, name), out);
rmSync(tmpDir, { recursive: true, force: true });
console.log(`Installeur : ${out} (${(statSync(out).size / 1048576).toFixed(1)} Mo)`);
