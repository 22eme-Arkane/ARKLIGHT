// Version Rust d'ARKLIGHT : données d'essai de parité de l'assistant IA, calculées par la version
// JavaScript (server/ai/tools.js).
//   node tools/rust/export-ai-data.mjs
// Écrit dans rust/crates/arklight-ai/tests/data/ :
//   tool-defs.json   définitions des outils (schémas JSON donnés au modèle)
//   catalog.txt      catalogue du prompt système
//   aim.json         orientations calculées par aimRotation (three.js)
//   tool-runs.json   une suite d'appels d'outils sur le projet d'exemple « demo-night » : résultat de
//                    chaque appel et état du plan après (projecteurs, objets, groupes, ambiance)
// Les identifiants créés pendant la suite (aléatoires) sont remplacés par « #new ».
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { SceneStore } from '../../server/scene-store.js';
import { TOOL_DEFS, aimRotation, catalogText, createToolRunner } from '../../server/ai/tools.js';

const OUT = fileURLToPath(new URL('../../rust/crates/arklight-ai/tests/data/', import.meta.url));
const DEMO = fileURLToPath(new URL('../../rust/crates/arklight-core/tests/data/demo-night.json', import.meta.url));
mkdirSync(OUT, { recursive: true });

writeFileSync(join(OUT, 'tool-defs.json'), JSON.stringify(TOOL_DEFS, null, 1));
writeFileSync(join(OUT, 'catalog.txt'), catalogText());

// orientations : face, contre, plongée, verticale, même point, dans tous les quadrants
const aims = [];
const points = [[0, 1.6, 0], [3, 3, 3], [-3.5, 2.8, 4], [2, 4, -3], [-1, 0.5, -2], [0, 5, 0], [0, 1.6, 0.0001], [5.2, 3.2, -3], [-0.6, 1.55, 0.3], [10, 0.2, 10]];
for (const from of points) for (const to of points) aims.push({ from, to, rotation: aimRotation(from, to) });
writeFileSync(join(OUT, 'aim.json'), JSON.stringify(aims));

/** La suite d'appels (les ids viennent du fichier ou passent par les noms). */
const CALLS = [
  ['get_scene', {}],
  ['search_catalog', { query: 'skypanel s60' }],
  ['search_catalog', { query: 'cadre 8x8', kind: 'object' }],
  ['add_fixtures', { type: 'astera.titan', count: 4, name: 'Contre Titan', layout: { kind: 'arc', center: [0, 0.6, -2], radius: 3, start_angle: 150, end_angle: 210 }, aim: 'Comédienne', intensity: 80, hue: 220, saturation: 70, group: 'Contres Titan' }],
  ['add_fixtures', { type: 's60', positions: [[3, 2.8, 3], [-3, 2.8, 3]], aim: 'actor', cct: 3200, intensity: 45, universe: 2, start_address: 101, mode: 'P02' }],
  ['add_fixtures', { type: 'arri.m18', aim: [0, 1.5, 0] }],
  ['add_fixtures', { type: 'inconnu-xyz' }],
  ['update_fixtures', { target: { ids: ['S360 face'] }, position: [5, 3, 4], aim: 'Comédien', intensity: 70, cct: 5000 }],
  ['update_fixtures', { target: { group: 'Face' }, move: [0, 0.5, 0], power: false }],
  ['update_fixtures', { target: { type: 'astera.nyx' }, cct: 2700, intensity: 40 }],
  ['update_fixtures', { target: 'Fix 5', name: 'Contre principal', aim: 'Voiture des comédiens', support: 'windup' }],
  ['update_fixtures', { target: { ids: ['Contre Titan'] }, position: [0, 2, -4], aim: [0, 1.6, 0] }],
  ['update_fixtures', { target: { ids: ['personne'] }, intensity: 10 }],
  ['update_fixtures', { target: { all: true }, aim: 'nulle part' }],
  ['readdress', { target: { all: true }, universe: 1, start_address: 1, order: 'position', fixture_start: 101 }],
  ['add_objects', { type: 'frame-12x12', count: 2, name: 'Cadre', layout: { kind: 'line', from: [-2, 2, 2], to: [2, 2, 2] }, face: 'actor', props: { cloth: 'silk' } }],
  ['add_objects', { type: 'camera-tripod', positions: [[0, 0, 6]], props: { focal: 50, lens: 1.6 } }],
  ['add_objects', { type: 'applebox-full', name: 'Caisse', positions: [[-1, 0, 2], [1, 0, 2], [3, 0, 2]] }],
  ['add_objects', { type: 'person-woman', name: 'Chanteuse', positions: [[1, 0, -1]], face: [0, 1.6, 6], props: { pose: 'sitting', height: 500, top: '#AABBCC', bogus: 1 } }],
  ['add_objects', { type: 'sandbag', count: 3, layout: { kind: 'grid', center: [4, 0, 4], cols: 2, spacing: 0.5 } }],
  ['update_objects', { ids: ['Passant', 'Chien'], position: [0, 0, 3], face: 'Caméra A' }],
  ['update_objects', { ids: ['Réverbère'], move: [0.5, 0, 0], name: 'Lampadaire', props: { color: '#AABBCC' } }],
  ['remove_items', { ids: ['Pigeon 1', 'Pigeon 2', 'Nova débouchage', 'introuvable'] }],
  ['create_group', { name: '  Practicals   café ', target: { ids: ['Baby 1K porte du café', 'Wasp lampadaire'] } }],
  ['set_environment', { haze: 35, exposure: 0.7, ambient: 3, floor: true, cyc: true }],
  [
    'place_from_drawing',
    {
      width_m: 12,
      items: [
        { kind: 'fixture', label: 'SKY S60 face 3200K', x: 0.7, y: 0.75, aim_x: 0.5, aim_y: 0.5 },
        { kind: 'fixture', label: '4 x HELIOS bleu', count: 4, x: 0.2, y: 0.1, x2: 0.8, y2: 0.1 },
        { kind: 'fixture', label: '2 x TITAN rouge', count: 2, x: 0.1, y: 0.3, x2: 0.1, y2: 0.7 },
        { kind: 'fixture', label: 'GOOD 600 contre', x: 0.3, y: 0.2, height: 3.5, intensity: 60 },
        { kind: 'grip', label: '12x12 ultra', x: 0.75, y: 0.6 },
        { kind: 'grip', label: 'drapeau noir', x: 0.4, y: 0.6 },
        { kind: 'camera', label: 'Caméra 35mm', x: 0.5, y: 0.95 },
        { kind: 'set', label: 'mur du fond', x: 0.05, y: 0.02, x2: 0.95, y2: 0.02 },
        { kind: 'set', label: 'canapé', x: 0.5, y: 0.4 },
        { kind: 'set', label: 'qsdfgh', x: 0.1, y: 0.9 },
        { kind: 'person', label: 'COMÉDIENNE assise', x: 0.5, y: 0.5, aim_x: 0.5, aim_y: 0.9 },
        { kind: 'person', label: 'enfant', x: 0.6, y: 0.5 },
        { kind: 'fixture', label: 'sans position' },
      ],
    },
  ],
  ['manage_plans', { action: 'list' }],
  ['release_control', {}],
  ['inconnu', {}],
  ['get_scene', { include_objects: false }],
];

const tmp = mkdtempSync(join(tmpdir(), 'arklight-ai-'));
let store = null;
try {
  store = new SceneStore(tmp);
  store.lang = 'fr';
  store.loadFrom(DEMO);
  const original = new Set([...store.scene.fixtures.map((f) => f.id), ...store.scene.objects.map((o) => o.id), ...store.scene.groups.map((g) => g.id)]);
  const runner = createToolRunner({ store, hooks: {}, context: { imageAspect: 1.5 } });
  /** Les ids créés pendant la suite deviennent « #new » (ils sont aléatoires). */
  const created = () => new Set([...store.scene.fixtures.map((f) => f.id), ...store.scene.objects.map((o) => o.id), ...store.scene.groups.map((g) => g.id)].filter((id) => !original.has(id)));
  const normalize = (v, ids) => JSON.parse(JSON.stringify(v ?? null), (k, x) => (typeof x === 'string' && ids.has(x) ? '#new' : x));
  const snapshot = () => ({
    fixtures: store.scene.fixtures.map(({ id, type, name, position, rotation, settings, state }) => ({ id, type, name, position, rotation, settings, state })),
    objects: store.scene.objects.map(({ id, type, name, position, rotation, props }) => ({ id, type, name, position, rotation, props })),
    groups: store.scene.groups.map(({ id, name, ids }) => ({ id, name, ids })),
    environment: store.scene.environment,
  });
  const steps = [];
  for (const [name, args] of CALLS) {
    const result = runner.run(name, structuredClone(args));
    const ids = created();
    steps.push({ name, args, result: normalize(result, ids), after: normalize(snapshot(), ids) });
  }
  writeFileSync(join(OUT, 'tool-runs.json'), JSON.stringify({ project: 'demo-night', imageAspect: 1.5, steps }));
  console.log(`données de l'assistant écrites dans ${OUT} (${steps.length} appels d'outils, ${aims.length} orientations)`);
} finally {
  if (store) clearTimeout(store._timer);
  rmSync(tmp, { recursive: true, force: true });
}
