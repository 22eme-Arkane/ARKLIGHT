// Version Rust d'ARKLIGHT : données d'essai de parité du plan de feu, calculées par la version
// JavaScript (web/js/plot/, shared/mvr.js, shared/zip.js).
//   node tools/rust/export-plot-data.mjs
// Écrit dans rust/crates/arklight-plot/tests/data/ :
//   catalogue.json      projet d'essai : un exemplaire de chaque décor et de chaque grip, des
//                       projecteurs avec accessoires, pieds choisis, éteints, pointés vers le sol…
//   cases.json          cas d'essai (projet, vue, thème, options, sélection…) et résultats attendus
//   *.svg               plans (export « Plan en SVG » ou rendu de la vue), *.csv listes et patch
//   mvr.json            export MVR : GeneralSceneDescription.xml, fichiers GDTF, archive (CRC, taille)
//   collation.json      ordre alphabétique français (localeCompare) de noms de la bibliothèque
// Les modules du navigateur importent « three » et des chemins « /shared/… » : un crochet de
// résolution (node:module) les redirige vers les fichiers du projet ; web/js/i18n.js (qui touche
// au document) est remplacé par la traduction de shared/i18n.js en français.
import { registerHooks } from 'node:module';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// heure fixe (révision des GDTF, date des fichiers de l'archive), en temps universel
process.env.TZ = 'UTC';
const FIXED = Date.UTC(2026, 9, 27, 8, 30, 14);
const RealDate = Date;
globalThis.Date = class extends RealDate {
  constructor(...a) {
    if (a.length) super(...a);
    else super(FIXED);
  }
  static now() {
    return FIXED;
  }
};

const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const ROOT_URL = pathToFileURL(ROOT).href.replace(/\/?$/, '/');
const I18N_URL = new URL('web/js/i18n.js', ROOT_URL).href;
const I18N_STUB = `data:text/javascript,${encodeURIComponent(
  `import { LANGS, translate, translateMarkup } from '${new URL('shared/i18n.js', ROOT_URL).href}';
export { LANGS };
export const lang = 'fr';
export const t = (fr, params) => translate(fr, 'fr', params);
export const tMarkup = (markup) => translateMarkup(markup, 'fr');`,
)}`;
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (/^\/(shared|web)\//.test(specifier)) return { url: new URL(specifier.slice(1), ROOT_URL).href, shortCircuit: true };
    const r = nextResolve(specifier, context);
    if (r.url === I18N_URL) return { url: I18N_STUB, shortCircuit: true };
    return r;
  },
});

const mod = (p) => import(new URL(p, ROOT_URL).href);
const { renderPlot, titleBlock } = await mod('web/js/plot/render.js');
const { buildEquipmentList, buildPatch, equipmentCsv, patchCsv } = await mod('web/js/plot/equipment.js');
const { bodyHalfExtents, distToSegment, fixturePlan, rotateYaw } = await mod('web/js/plot/geometry.js');
const { getFixtureType, FIXTURE_TYPES } = await mod('shared/fixtures/index.js');
const { resolveSupport, SUPPORTS } = await mod('shared/fixtures/supports.js');
const { buildMvrFiles, buildMvr, buildGdtf, uuidFrom, mvrMatrix } = await mod('shared/mvr.js');
const { crc32, readZip } = await mod('shared/zip.js');
const { flatPlans, currentPlan } = await mod('shared/plans.js');
const { PROP_CATEGORIES, PROP_TYPES } = await mod('shared/props.js');
const { GRIP_TYPES, CLOTHS } = await mod('shared/grip.js');
const { FILTERS, getFilter } = await mod('shared/filters.js');
const { MODIFIERS, GRIDS } = await mod('shared/modifiers.js');

const DATA = join(ROOT, 'rust/crates/arklight-core/tests/data');
const OUT = join(ROOT, 'rust/crates/arklight-plot/tests/data');
mkdirSync(OUT, { recursive: true });
const write = (name, text) => writeFileSync(join(OUT, name), text);
const readProject = (name) => JSON.parse(readFileSync(name === 'catalogue.json' ? join(OUT, name) : join(DATA, name), 'utf8'));

// ------------------------------------------------------------------ options d'affichage (plot-view.js)
const DECOR = PROP_CATEGORIES.map((c) => c.id);
const EQUIPMENT_ONLY = { stands: true, grip: true, sandbags: false, studio: false, annotations: false, ...Object.fromEntries(DECOR.map((c) => [c, false])) };
const SHOW_ALL = Object.fromEntries(Object.keys(EQUIPMENT_ONLY).map((k) => [k, true]));
const DEFAULTS = { names: true, fixtures: true, addresses: true, supports: true, beams: true, grid: true, underlay: true, ...EQUIPMENT_ONLY };
const DATE = '27/10/2026';

// ------------------------------------------------------------------ projet d'essai « catalogue »
const filter = (id) => {
  if (!getFilter(id)) throw new Error(`filtre inconnu : ${id}`);
  return id;
};
function catalogue() {
  const fx = (id, type, name, position, rotation, settings = {}, state = {}) => {
    const t = getFixtureType(type);
    if (!t) throw new Error(`projecteur inconnu : ${type}`);
    return {
      id,
      type,
      name,
      position,
      rotation,
      settings: { universe: 1, dmxAddress: 1, dmxMode: t.dmx?.modes?.[0]?.id ?? null, whitePoint: 5600, dimmingCurve: 'linear', support: 'auto', ...(t.defaults?.settings || {}), ...settings },
      state: { power: true, battery: 100, charging: false, manual: { mode: 'cct', intensity: 0.6, cct: 5600, tint: 0, hue: 0, sat: 1 }, ...state },
    };
  };
  const fixtures = [
    fx('f-dome', 'aputure.ls-600d-pro', 'Dôme & grille', [0, 2.2, 4], [0, Math.PI, 0], { modifier: 'dome-90', front: filter('lee-216'), grid: 'grid-40', gel: filter('lee-204'), fixtureNumber: 101, dmxAddress: 40 }),
    fx('f-chimera', 'arri.skypanel-s60c', 'Chimera <M> "face"', [3, 2.5, 2], [0.3, -0.6, 0], { modifier: 'chimera-m', front: 'none', gel: filter('lee-216'), support: 'windup', fixtureNumber: 102, universe: 2, dmxAddress: 100 }),
    fx('f-down', 'arri.m18', 'M18 plongée', [-3, 4.8, 2], [Math.PI / 2, 0, 0], { gel: filter('lee-201'), grid: 'grid-20', support: 'long-john', fixtureNumber: 103, dmxAddress: 200 }),
    fx('f-off', 'astera.titan', 'Titan éteint', [-2, 1.4, -1], [0, 0.4, Math.PI / 2], { support: 'hanging', dmxAddress: 300 }, { power: false }),
    fx('f-bar', 'astera.ax2-100', 'Barre AX2', [1.5, 0.3, -2], [-0.2, 0.8, 0], { gel: filter('lee-106'), fixtureNumber: 104, universe: 2, dmxAddress: 1 }),
    fx('f-box', 'astera.art7', 'AsteraBox', [4, 1, -3], [0, 0, 0], { support: 'none' }),
    fx('f-bulb', 'astera.nyx', 'NYX sur pied', [-4.5, 1.8, 0.5], [0, 0, 0], { support: 'stand-1000', fixtureNumber: 105, dmxAddress: 400 }),
    fx('f-hydra', 'astera.hydrapanel', 'Hydra « chimera »', [2.2, 1.6, 0.4], [0, -2.4, 0], { modifier: 'chimera-panel', fixtureNumber: 106, dmxAddress: 450 }),
    fx('f-baby', 'mole.baby-1k', 'Baby 1K', [5.5, 2.9, 3.5], [0.2, 0.9, 0], { grid: 'grid-60', fixtureNumber: 107, dmxAddress: 500 }),
    fx('f-sky', 'arri.skypanel-s360c', 'S360 haut', [-6, 5.2, 4.5], [0.4, -2.2, 0], { fixtureNumber: 108, universe: 3, dmxAddress: 1 }),
  ];
  const objects = [];
  // décor : une grille, un exemplaire de chaque modèle (quelques variantes de réglages)
  const variants = {
    'person-man': { pose: 'pointing' },
    'person-woman': { pose: 'lying' },
    'person-child': { pose: 'sitting', height: 110 },
    'wall-flat': { width: 3.5, color: '#c0b8a8' },
    'camera-tripod': { focal: 24, sensor: 'ff' },
    'camera-dolly': { focal: 85, sensor: 'm43' },
    tree: { height: 8 },
  };
  PROP_TYPES.forEach((t, i) => {
    const col = i % 10;
    const row = Math.floor(i / 10);
    const defaults = Object.fromEntries((t.params || []).map((p) => [p.key, p.default]));
    objects.push({ id: `o-${t.id}`, type: t.id, name: t.short || t.name, position: [-14 + col * 3.1, 0, -16 - row * 3.4], rotation: [0, ((i * 37) % 360) * (Math.PI / 180), 0], props: { ...defaults, ...(variants[t.id] || {}) } });
  });
  // grip : chaque modèle, des toiles et des pieds variés
  const cloths = { 'frame-6x6': 'silk', 'frame-8x8': filter('lee-216'), 'frame-12x12': 'solid', 'frame-20x20': 'empty', 'flag-18x24': 'net-single', 'flag-24x36': 'solid', floppy: 'solid' };
  GRIP_TYPES.forEach((t, i) => {
    const props = {};
    if (t.cloths) props.cloth = cloths[t.id] || t.defaultCloth;
    if (t.family === 'stand') props.height = t.height;
    if (t.id === 'frame-12x12') props.stand = 'long-john';
    objects.push({ id: `g-${t.id}`, type: t.id, name: t.short, position: [-12 + (i % 6) * 5, 0, 7 + Math.floor(i / 6) * 5], rotation: [0, i * 0.35, 0], props });
  });
  objects.push({ id: 'g-frame-8x8-b', type: 'frame-8x8', name: 'Cadre 8×8', position: [9, 0, 2], rotation: [0, 1.2, 0], props: { cloth: 'ultrabounce' } });
  objects.push({ id: 'g-frame-6x6-b', type: 'frame-6x6', name: 'Cadre 6×6', position: [10, 0, -3], rotation: [0, 0, 0], props: { cloth: filter('lee-204'), stand: 'windup' } });
  const style = (stroke, extra = {}) => ({ stroke, fill: 'none', width: 2, dash: false, size: 14, ...extra });
  const items = [
    { id: 'p-rect', kind: 'rect', points: [[-9, -6], [-5, -3]], text: 'Maison <A> & "B"', style: style('#3ddc84', { fill: '#3ddc84' }), height: 2.5 },
    { id: 'p-ell', kind: 'ellipse', points: [[6, -6], [9, -4]], text: 'Kiosque', style: style('#5aa9ff', { dash: true, width: 3 }) },
    { id: 'p-poly', kind: 'polygon', points: [[-2, 8], [1, 9.5], [2.5, 11], [-1.5, 12]], text: 'Zone', style: style('#ffad3b', { fill: '#ffad3b' }), height: 1.25 },
    { id: 'p-free', kind: 'freehand', points: [[3, 8], [3.4, 8.3], [3.9, 8.2], [4.4, 8.9], [5.1, 9.4]], style: style('#c792ea', { width: 5 }) },
    { id: 'p-line', kind: 'line', points: [[-8, 2], [-4, 5]], text: 'Câble', style: style('#ff5c5c', { width: 1 }) },
    { id: 'p-arrow', kind: 'arrow', points: [[8, 6], [5, 3]], style: style('#3ddc84', { width: 8 }) },
    { id: 'p-measure', kind: 'measure', points: [[-6, -1], [-1.25, -1]], text: 'recul', style: style('#5aa9ff') },
    { id: 'p-text', kind: 'text', points: [[-10, 13]], text: 'Régie & loges', style: { stroke: '#fdfdfd', size: 20 } },
    { id: 'p-text2', kind: 'text', points: [[4, 13]], style: {} },
    { id: 'p-unknown', kind: 'cloud', points: [[0, 0]], style: {} },
  ];
  return {
    app: 'ARKLIGHT',
    format: 2,
    name: 'Catalogue « essai » & <tests>',
    environment: { floor: true, cyc: true, stands: true, labels: false, labelPatch: false, haze: 0.2, ambient: 0.04, exposure: 1 },
    fixtures,
    groups: [],
    objects,
    plot: {
      items,
      underlay: { url: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', x: 2, z: -3, width: 14, aspect: 1.6, rotation: 12, opacity: 0.45, floor3d: false },
      background: null,
      title: { production: '', author: 'Camille Électro', notes: '' },
    },
    current: { seq: 's1', plan: 'p2' },
    sequences: [
      { id: 's1', name: 'Séq. 1 « nuit »', plans: [{ id: 'p1', name: 'Plan 1', scene: null }, { id: 'p2', name: 'Plan 2 & fin', scene: null }] },
    ],
  };
}
const cat = catalogue();
write('catalogue.json', `${JSON.stringify(cat, null, 1)}\n`);

// ------------------------------------------------------------------ vue 3D simulée (outputOf)
// pied : celui que calcule la liste du matériel sans vue 3D ; sortie : couleurs données par le cas
function fakeViewport(scene, outputs) {
  return {
    outputOf(id) {
      const f = scene.fixtures.find((x) => x.id === id);
      const type = f && getFixtureType(f.type);
      if (!type) return null;
      return { type, out: outputs?.[id] || null, support: resolveSupport(type, f.settings.support, f.position[1] - Math.max(...bodyHalfExtents(type))) };
    },
  };
}

// ------------------------------------------------------------------ export SVG (plot-view.js, exportSvg)
function computeFit(scene, W, H) {
  const xs = [];
  const zs = [];
  for (const f of scene.fixtures || []) {
    xs.push(f.position[0]);
    zs.push(f.position[2]);
  }
  for (const o of scene.objects || []) {
    xs.push(o.position[0]);
    zs.push(o.position[2]);
  }
  for (const it of scene.plot?.items || []) for (const [x, z] of it.points) xs.push(x), zs.push(z);
  if (!xs.length) xs.push(-5, 5), zs.push(-7, 4);
  const minX = Math.min(...xs) - 1.5;
  const maxX = Math.max(...xs) + 1.5;
  const minZ = Math.min(...zs) - 1.5;
  const maxZ = Math.max(...zs) + 1.5;
  return { cx: (minX + maxX) / 2, cz: (minZ + maxZ) / 2, s: Math.max(8, Math.min(200, Math.min(W / (maxX - minX), (H - 60) / (maxZ - minZ)))) };
}
function planText(scene) {
  const plans = { sequences: scene.sequences, current: scene.current };
  const cur = flatPlans(plans).length > 1 ? currentPlan(plans) : null;
  return cur ? `${cur.seq.name} › ${cur.plan.name}` : '';
}
function exportSvg(scene, options, viewport, W = 1754, H = 1240) {
  const fitted = computeFit(scene, W, H);
  const exportView = { ...fitted, s: fitted.s * 0.92, W, H };
  const body = renderPlot({ scene, view: exportView, viewport, theme: 'print', options, sel: {} });
  const tb = titleBlock({ W, H, title: { ...(scene.plot.title || {}), scene: scene.name, plan: planText(scene) }, date: DATE, background: scene.plot.background });
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" font-family="Segoe UI, Arial, sans-serif">${body}${tb}</svg>`;
}

// ------------------------------------------------------------------ cas d'essai du plan
const night = readProject('demo-night.json');
const outputsOf = (scene) => {
  const out = {};
  scene.fixtures.forEach((f, i) => {
    const type = getFixtureType(f.type);
    if (!type?.pixels || i % 3 === 1) return;
    const px = [];
    for (let p = 0; p < type.pixels; p++) {
      if (i % 5 === 4) px.push(0, 0, 0.001); // trop sombre : couleur par défaut
      else px.push(((i * 37 + p * 11) % 100) / 100, ((i * 53 + p * 7) % 100) / 100, ((i * 71 + p * 3) % 100) / 100);
    }
    out[f.id] = px;
  });
  return out;
};
const extraItems = [
  { id: 'x-ell', kind: 'ellipse', points: [[1, -6], [4, -4.5]], text: 'Kiosque', style: { stroke: '#0f1216', fill: '#5aa9ff', width: 3, dash: false, size: 18 }, height: 3 },
  { id: 'x-poly', kind: 'polygon', points: [[-8, 5], [-6, 6], [-7, 7.5]], text: 'Zone', style: { stroke: '#ffad3b', fill: 'none', width: 2, dash: true }, height: 0 },
  { id: 'x-free', kind: 'freehand', points: [[2, 5], [2.5, 5.2], [3, 5.1]], text: 'trace', style: { stroke: '#c792ea', fill: '#c792ea', width: 1 } },
  { id: 'x-line', kind: 'line', points: [[6, -2], [8, 1]], text: 'Câble 32 A', style: { stroke: 'red' } },
];
const cases = [
  { name: 'night-export', project: 'demo-night.json', kind: 'export', options: DEFAULTS },
  { name: 'clip-export-dark', project: 'demo-clip.json', kind: 'export', options: { ...DEFAULTS, ...SHOW_ALL }, plot: { background: '#2b2f36' } },
  { name: 'catalogue-export', project: 'catalogue.json', kind: 'export', options: { ...DEFAULTS, ...SHOW_ALL } },
  {
    name: 'night-screen',
    project: 'demo-night.json',
    kind: 'render',
    theme: 'screen',
    view: { cx: -1, cz: 0.5, s: 38, W: 1280, H: 760 },
    options: { ...DEFAULTS, ...SHOW_ALL },
    plot: {
      items: [...night.plot.items, ...extraItems],
      underlay: { url: 'uploads/plan.png', x: 1, z: -2, width: 12, aspect: 1.5, rotation: 15, opacity: 0.5 },
    },
    sel: { fixtures: [night.fixtures[0].id, night.fixtures[7].id], objectId: night.objects.find((o) => o.type === 'car-sedan')?.id, plotId: night.plot.items[0].id },
    preview: { kind: 'polygon', points: [[0, 0], [1.5, 0.5], [1, 2]], style: { stroke: '#ffad3b', fill: 'none', width: 2, dash: false, size: 14 } },
    handles: [
      { kind: 'rotate', x: 0.5, z: 4.2 },
      { kind: 'vertex', x: -2, z: 1 },
    ],
    outputs: outputsOf(night),
  },
  {
    name: 'clip-light',
    project: 'demo-clip.json',
    kind: 'render',
    theme: 'print',
    view: { cx: 0, cz: -2, s: 22, W: 900, H: 700 },
    options: { ...DEFAULTS, names: false, fixtures: false, supports: false, beams: false, grid: false, studio: true, annotations: true, people: true, vehicles: true },
    plot: { background: '#c9ccd1' },
  },
  {
    name: 'catalogue-small',
    project: 'catalogue.json',
    kind: 'render',
    theme: 'screen',
    view: { cx: 0, cz: 0, s: 5, W: 640, H: 480 },
    options: { ...DEFAULTS, addresses: false, sandbags: true },
    plot: { background: '#1d2b3a' },
  },
];
for (const c of cases) {
  const raw = readProject(c.project);
  const scene = { ...raw, plot: { ...raw.plot, ...(c.plot || {}) } };
  const viewport = fakeViewport(scene, c.outputs);
  let svg;
  if (c.kind === 'export') svg = exportSvg(scene, c.options, viewport);
  else {
    svg = renderPlot({
      scene,
      view: c.view,
      viewport,
      theme: c.theme,
      options: c.options,
      sel: { fixtures: new Set(c.sel?.fixtures || []), objectId: c.sel?.objectId, plotId: c.sel?.plotId },
      preview: c.preview || null,
      handles: c.handles || [],
    });
  }
  c.file = `${c.name}.svg`;
  write(c.file, svg);
}

// cartouches seuls (thèmes, plan, auteur)
const titleBlocks = [
  { W: 1754, H: 1240, title: { production: 'Pub « Été »', scene: 'Cuisine & salon', author: 'Lou', plan: 'Séq. 2 › Plan 3' }, theme: 'print', background: null },
  { W: 800, H: 600, title: { scene: '' }, theme: 'screen', background: '#000000' },
  { W: 1000, H: 700, title: { production: '', scene: 'Nuit', author: '' }, theme: 'print', background: '#c9ccd1' },
].map((tb) => ({ ...tb, date: DATE, svg: titleBlock({ ...tb, date: DATE }) }));

// ------------------------------------------------------------------ liste du matériel et patch (CSV)
const csv = [];
for (const [project, short] of [
  ['demo-night.json', 'night'],
  ['demo-clip.json', 'clip'],
  ['catalogue.json', 'catalogue'],
]) {
  const scene = readProject(project);
  const viewport = fakeViewport(scene, null);
  write(`${short}-equipment.csv`, equipmentCsv(buildEquipmentList(scene, viewport)));
  write(`${short}-patch.csv`, patchCsv(buildPatch(scene)));
  // sans vue 3D : pieds recalculés d'après la hauteur (même résultat ici)
  if (equipmentCsv(buildEquipmentList(scene, null)) !== equipmentCsv(buildEquipmentList(scene, viewport))) throw new Error('liste du matériel : vue 3D simulée incohérente');
  csv.push({ project, equipment: `${short}-equipment.csv`, patch: `${short}-patch.csv` });
}

// géométrie (rotation sur le plan, clic sur les lignes, empreintes) et noms de fichiers des exports
const slug = (scene) =>
  [scene.name || 'plan', planText(scene)]
    .join(' ')
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '') || 'plan';
const geometry = {
  rotateYaw: [
    [[0, 0, 0], 0.5],
    [[0.3, -0.6, 0.9], -1.2],
    [[-2.756, -1.181, -2.782], Math.PI / 36],
    [[Math.PI / 2, 0, 0], 2],
    [[0, Math.PI / 2, 0], 0.1],
  ].map(([rotation, delta]) => ({ rotation, delta, out: rotateYaw(rotation, delta) })),
  distToSegment: [
    [[0.5, 1], [0, 0], [1, 0]],
    [[-2, 0.3], [0, 0], [1, 0]],
    [[3, 4], [1, 1], [1, 1]],
    [[0.25, -0.75], [-1.5, 2], [2.5, -3]],
  ].map(([p, a, b]) => ({ p, a, b, d: distToSegment(p, a, b) })),
  fixturePlans: ['demo-night.json', 'catalogue.json'].flatMap((project) =>
    readProject(project).fixtures.map((f) => ({ project, id: f.id, plan: fixturePlan(f, getFixtureType(f.type)) })),
  ),
  slugs: ['demo-night.json', 'demo-clip.json', 'catalogue.json'].map((project) => ({ project, slug: slug(readProject(project)) })),
};

write('cases.json', `${JSON.stringify({ date: DATE, svg: cases, titleBlocks, csv, geometry }, null, 1)}\n`);

// ------------------------------------------------------------------ MVR
const mvr = [];
for (const project of ['demo-night.json', 'demo-clip.json', 'catalogue.json']) {
  const scene = readProject(project);
  const opts = { providerVersion: '0.1.0-alpha.5', layerName: 'Projecteurs' };
  const { files, fixtures, skipped } = buildMvrFiles(scene, opts);
  const { bytes } = buildMvr(scene, opts);
  const entries = readZip(bytes);
  mvr.push({
    project,
    providerVersion: opts.providerVersion,
    layerName: opts.layerName,
    fixtures,
    skipped,
    size: bytes.length,
    crc: crc32(bytes),
    files: files.map((f, i) => {
      if (typeof f.data === 'string') return { name: f.name, text: f.data, crc: entries[i].crc };
      const inner = readZip(f.data);
      return { name: f.name, size: f.data.length, crc: crc32(f.data), description: new TextDecoder().decode(inner[0].data) };
    }),
  });
}
const uuids = ['', 'fixture:abc', 'layer:Nuit — rue', 'gdtf:astera.titan', 'é😀'].map((text) => ({ text, uuid: uuidFrom(text) }));
const crcs = ['', '123456789', 'ARKLIGHT — plan de feu'].map((text) => ({ text, crc: crc32(new TextEncoder().encode(text)) }));
const matrices = [
  [[0, 0, 0], [0, 0, 0]],
  [[1.25, 2.5, -3.75], [0.3, -0.6, 0.9]],
  [[-1, 3, 5], [-2.756, -1.181, -2.782]],
].flatMap(([p, r]) => [true, false].map((fixture) => ({ position: p, rotation: r, fixture, matrix: mvrMatrix(p, r, { fixture }) })));
// un GDTF avec plusieurs modes (attributs répétés, 16 bits, pixels) : les 3 premiers modes et le
// plus long (les Astera en ont plus de 140 : fichier trop gros pour les données d'essai)
const multi = ['astera.titan', 'astera.ax2-100', 'arri.skypanel-s60c'].map((id) => {
  const type = getFixtureType(id);
  const all = type.dmx.modes;
  const longest = all.reduce((a, m) => (m.channels.length > a.channels.length ? m : a), all[0]);
  const modes = [...new Set([...all.slice(0, 3), longest])];
  return { type: id, modes: modes.map((m) => m.id), xml: buildGdtf(type, modes) };
});
write('mvr.json', `${JSON.stringify({ revision: new Date().toISOString().slice(0, 19), mvr, uuids, crcs, matrices, multi })}\n`);

// ------------------------------------------------------------------ ordre alphabétique (localeCompare 'fr')
const names = [
  ...FIXTURE_TYPES.map((t) => (t.name.startsWith(`${t.brand} `) ? t.name : `${t.brand} ${t.name}`)),
  ...GRIP_TYPES.map((t) => t.name),
  ...GRIP_TYPES.filter((t) => t.cloths).flatMap((t) => t.cloths.slice(0, 9).map((c) => `${t.name} — ${CLOTHS[c]?.name || getFilter(c)?.name || c}`)),
  ...Object.values(SUPPORTS).map((s) => s.name),
  ...MODIFIERS.map((m) => m.name),
  ...GRIDS.map((g) => g.name),
  ...FILTERS.flatMap((f) => [`Toile avant ${f.name}`, `Gélatine ${f.name}`]),
  'Accroches (pinces, perches) pour projecteurs accrochés',
  'C-stand',
  'Pied de 1000 + pince',
  'éclair',
  'Eclair',
  'eclair',
  'Écran',
  'ecran',
  'a b',
  'ab',
  'a-b',
  'a_b',
  'a.b',
  'a(b',
  'a1',
  'a10',
  'a9',
  'Zèbre',
  'zebre',
  'Œuvre',
  'oeuvre',
];
// ordre de départ mélangé (le même pour Rust), puis tri
const shuffled = names.map((n, i) => [n, (i * 7919) % 104729]).sort((a, b) => a[1] - b[1]).map((x) => x[0]);
write('collation.json', `${JSON.stringify({ input: shuffled, sorted: [...shuffled].sort((a, b) => a.localeCompare(b, 'fr')) }, null, 1)}\n`);

console.log(`données d'essai du plan de feu écrites dans ${OUT}`);
