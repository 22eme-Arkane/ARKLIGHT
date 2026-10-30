// Version Rust d'ARKLIGHT : cas de test de l'adressage automatique (shared/engine/addressing.js)
// pour rust/crates/arklight-core/tests/addressing.rs. Patchs tirés au hasard (chevauchements,
// dépassements, plusieurs univers), puis chaque combinaison d'options.
//   node tools/rust/export-addressing-data.mjs
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { FIXTURE_TYPES, defaultSettings, getFixtureType, modeFootprint } from '../../shared/fixtures/index.js';
import { findPatchConflicts, planAddressing } from '../../shared/engine/addressing.js';

const OUT = fileURLToPath(new URL('../../rust/crates/arklight-core/tests/data/addressing.json', import.meta.url));
const list = Array.isArray(FIXTURE_TYPES) ? FIXTURE_TYPES : Object.values(FIXTURE_TYPES);

let seed = 12345;
const rand = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const pick = (a) => a[Math.floor(rand() * a.length)];

const patchable = list.filter((t) => (t.dmx?.modes || []).some((m) => modeFootprint(t, m.id) > 0));
const NAMES = ['Titan', 'Helios', 'Hyperion', 'Face', 'Contre', 'Douche', 'Élise', 'éclairage', 'Ambiance', 'Fond'];

function scene(n) {
  const fixtures = [];
  for (let i = 0; i < n; i++) {
    const t = pick(patchable);
    const mode = pick(t.dmx.modes.filter((m) => modeFootprint(t, m.id) > 0));
    const settings = { ...defaultSettings(t), dmxMode: mode.id, universe: 1 + Math.floor(rand() * 3), dmxAddress: 1 + Math.floor(rand() * 520) };
    fixtures.push({
      id: `f${i}`,
      type: t.id,
      name: `${pick(NAMES)} ${1 + Math.floor(rand() * 12)}`,
      position: [Math.round((rand() * 10 - 5) * 4) / 4, 2, Math.round((rand() * 6 - 3) * 2) / 2],
      rotation: [0, 0, 0],
      settings,
      state: {},
    });
  }
  // un émetteur sans canal (AsteraBox), ignoré par l'adressage
  const box = list.find((t) => t.id === 'astera.asterabox');
  if (box) fixtures.push({ id: 'box', type: box.id, name: 'AsteraBox', position: [0, 1, 0], rotation: [0, 0, 0], settings: { ...defaultSettings(box) }, state: {} });
  return fixtures;
}

const cases = [];
for (let s = 0; s < 40; s++) {
  const fixtures = scene(3 + Math.floor(rand() * 30));
  const conflicts = findPatchConflicts(fixtures.filter((f) => modeFootprint(getFixtureType(f.type), f.settings.dmxMode) > 0));
  const plans = [];
  const ids = fixtures.filter(() => rand() < 0.5).map((f) => f.id);
  for (const scope of ['conflicts', 'all', 'ids']) {
    for (const order of ['scene', 'name', 'position', 'type', 'address']) {
      if (scope === 'conflicts' && order !== 'scene') continue;
      for (const universes of ['keep', 'pack']) {
        for (const align of [0, 16]) {
          for (const overflow of [true, false]) {
            const opts = { scope, order, universes, align, overflow, universe: 1 + Math.floor(rand() * 2), address: 1 + Math.floor(rand() * 400) };
            const plan = planAddressing(fixtures, { ...opts, ids });
            // déplacements : [id, univers, adresse]
            plans.push({ opts, plan: { ...plan, moves: plan.moves.map((m) => [m.id, m.to.universe, m.to.address]) } });
          }
        }
      }
    }
  }
  cases.push({ fixtures, ids, conflicts: { overlaps: conflicts.overlaps, overflow: conflicts.overflow }, plans });
}

writeFileSync(OUT, JSON.stringify(cases));
console.log(`${cases.length} scènes, ${cases.reduce((n, c) => n + c.plans.length, 0)} plans → ${OUT}`);
