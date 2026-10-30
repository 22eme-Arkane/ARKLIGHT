// Outils de l'assistant : ce que l'IA peut lire et faire dans la scène (ajouter, régler, adresser,
// grouper, décor, ambiance, séquences et plans). Chaque outil renvoie un résultat court (JSON) que
// le modèle lit pour continuer, avec un résumé lisible pour l'interface.
import { Object3D, Vector3 } from 'three';
import { FIXTURE_TYPES, getFixtureType, getMode, isTraditionalType, modeFootprint } from '../../shared/fixtures/index.js';
import { GRIP_TYPES } from '../../shared/grip.js';
import { PROP_TYPES } from '../../shared/props.js';
import { getObjectType, isProp } from '../../shared/objects.js';
import { planAddressing } from '../../shared/engine/addressing.js';
import { consoleHeld } from '../../shared/engine/console.js';
import { controlSource, isAsteraV5 } from '../../shared/engine/output.js';
import { currentPlan, flatPlans } from '../../shared/plans.js';

const r2 = (v) => Math.round(Number(v) * 100) / 100;
const r3 = (v) => Math.round(Number(v) * 1000) / 1000;
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const num = (v, d) => (Number.isFinite(Number(v)) ? Number(v) : d);
const vec3 = (v) => (Array.isArray(v) && v.length === 3 && v.every((x) => Number.isFinite(Number(x))) ? v.map(Number) : null);
const norm = (s) =>
  String(s ?? '')
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .trim();

export const ACTOR_AIM = [0, 1.6, 0];
const pivot = new Object3D();

/** Orientation d'un projecteur qui vise un point (son axe +Z vers la cible). */
export function aimRotation(from, to) {
  pivot.position.set(...from);
  pivot.rotation.set(0, 0, 0);
  pivot.lookAt(new Vector3(...to));
  return [pivot.rotation.x, pivot.rotation.y, pivot.rotation.z].map(r3);
}

/** Lacet pour qu'un objet (personnage, meuble) fasse face à un point. */
const faceYaw = ([x, , z], [tx, , tz]) => r3(Math.atan2(tx - x, tz - z));

const isTube = (type) => type?.geometry?.kind === 'tube';

/** Personnages et caméras sont posés au sol (la pose « assis » et la hauteur d'objectif font le reste). */
const grounded = (type) => ['person', 'camera', 'dolly'].includes(type?.model);
const isCamera = (type) => ['camera', 'dolly'].includes(type?.model);

/** Un projecteur ne s'enfonce pas dans le sol (un tube debout pose sur sa base). */
function liftFixture(type, p) {
  const min = isTube(type) ? r3((type.geometry.length || 1) / 2 + 0.02) : 0.15;
  return [p[0], Math.max(p[1], min), p[2]];
}

/** Visée : un tube pivote autour de la verticale (il reste debout), les autres visent le point. */
const fixtureRotation = (type, from, to) => (isTube(type) ? [0, faceYaw(from, to), 0] : aimRotation(from, to));

// ------------------------------------------------------------------ définitions (schémas JSON)
const POINT = { type: 'array', items: { type: 'number' }, minItems: 3, maxItems: 3, description: '[x, y, z] en mètres' };
const TARGET = {
  type: 'object',
  description: 'Projecteurs visés : ids (ou noms, ou numéros de fixture), et/ou un groupe, et/ou un modèle, ou tous',
  properties: {
    ids: { type: 'array', items: { type: 'string' }, description: 'ids donnés par get_scene (les noms et numéros de fixture marchent aussi)' },
    group: { type: 'string', description: 'nom d’un groupe' },
    type: { type: 'string', description: 'id de modèle (ex. astera.titan) : tous les projecteurs de ce modèle' },
    all: { type: 'boolean', description: 'tous les projecteurs du plan' },
  },
};
const LAYOUT = {
  type: 'object',
  description: 'Répartition automatique quand positions n’est pas donné',
  properties: {
    kind: { type: 'string', enum: ['line', 'arc', 'grid'] },
    from: POINT,
    to: POINT,
    center: POINT,
    radius: { type: 'number', description: 'rayon de l’arc (m)' },
    start_angle: { type: 'number', description: 'degrés ; 0 = vers la caméra (+z), négatif = jardin (x<0)' },
    end_angle: { type: 'number' },
    cols: { type: 'number' },
    spacing: { type: 'number', description: 'écart (m) pour la grille' },
  },
};
const LOOK = {
  intensity: { type: 'number', description: 'intensité en % (0–100)' },
  cct: { type: 'number', description: 'température de couleur en kelvins (blanc)' },
  hue: { type: 'number', description: 'teinte en degrés (0–360) pour une couleur saturée' },
  saturation: { type: 'number', description: 'saturation en % (0–100), avec hue' },
  power: { type: 'boolean', description: 'allumé / éteint' },
};

export const TOOL_DEFS = [
  {
    name: 'get_scene',
    description: 'État du plan en cours : projecteurs (id, nom, modèle, fixture, univers, adresse, canaux, position, réglage), décor et grip, groupes, ambiance, séquence et plan. À appeler avant de modifier des éléments existants.',
    parameters: { type: 'object', properties: { include_objects: { type: 'boolean', description: 'inclure le décor et le grip (par défaut oui)' } } },
  },
  {
    name: 'release_control',
    description:
      'Rend la main aux projecteurs : relâche la console ARKLIGHT (qui les pilote canal par canal et passe avant tout) et oublie le dernier DMX conservé quand plus aucune console ni appli n’envoie de signal. À utiliser quand des projecteurs restent noirs ou ne réagissent plus à leurs réglages (champ control de get_scene).',
    parameters: { type: 'object', properties: { target: TARGET } },
  },
  {
    name: 'search_catalog',
    description: 'Cherche dans la bibliothèque : projecteurs (avec leurs modes DMX et nombres de canaux) ou objets (décor, personnages, grip, pieds).',
    parameters: { type: 'object', properties: { query: { type: 'string' }, kind: { type: 'string', enum: ['fixture', 'object'] } }, required: ['query'] },
  },
  {
    name: 'add_fixtures',
    description: 'Ajoute des projecteurs d’un modèle de la bibliothèque, placés (positions ou répartition), orientés et adressés à la suite.',
    parameters: {
      type: 'object',
      properties: {
        type: { type: 'string', description: 'id du modèle (voir le catalogue), ex. arri.skypanel-s60c' },
        count: { type: 'number', description: 'nombre (1 par défaut)' },
        name: { type: 'string', description: 'nom de base (numéroté s’il y en a plusieurs)' },
        positions: { type: 'array', items: POINT },
        layout: LAYOUT,
        aim: { description: '"actor" (le comédien), le nom d’un objet de la scène, ou un point [x, y, z]', anyOf: [{ type: 'string' }, POINT] },
        universe: { type: 'number' },
        start_address: { type: 'number', description: 'première adresse DMX (sinon : à la suite du patch)' },
        mode: { type: 'string', description: 'id ou nom du mode DMX' },
        group: { type: 'string', description: 'crée un groupe avec ces projecteurs' },
        ...LOOK,
      },
      required: ['type'],
    },
  },
  {
    name: 'update_fixtures',
    description: 'Règle des projecteurs existants : nom, position, visée, intensité, couleur, allumage, mode DMX, pied.',
    parameters: {
      type: 'object',
      properties: {
        target: TARGET,
        name: { type: 'string' },
        position: { ...POINT, description: 'nouvelle position [x, y, z] ; pour plusieurs projecteurs : leur centre (ils gardent leurs écarts)' },
        move: { ...POINT, description: 'déplacement relatif [dx, dy, dz] en mètres' },
        aim: { description: '"actor", le nom d’un objet, ou un point [x, y, z]', anyOf: [{ type: 'string' }, POINT] },
        mode: { type: 'string', description: 'id ou nom du mode DMX' },
        support: { type: 'string', description: 'auto, hanging (accroché), none (au sol), ou id de pied' },
        ...LOOK,
      },
      required: ['target'],
    },
  },
  {
    name: 'readdress',
    description: 'Adresse des projecteurs à la suite (sans chevauchement, en passant à l’univers suivant si besoin), et/ou renumérote leurs fixtures.',
    parameters: {
      type: 'object',
      properties: {
        target: TARGET,
        universe: { type: 'number', description: 'univers de départ (1 par défaut)' },
        start_address: { type: 'number', description: 'adresse de départ (1 par défaut)' },
        order: { type: 'string', enum: ['scene', 'name', 'position', 'type', 'address'], description: 'ordre : scène, nom, de gauche à droite, modèle, adresse actuelle' },
        fixture_start: { type: 'number', description: 'renumérote aussi les fixtures à partir de ce numéro' },
      },
      required: ['target'],
    },
  },
  {
    name: 'add_objects',
    description: 'Ajoute du décor, des personnages, des véhicules, du grip (cadres, drapeaux, réflecteurs) ou des pieds.',
    parameters: {
      type: 'object',
      properties: {
        type: { type: 'string', description: 'id de l’objet (voir le catalogue)' },
        count: { type: 'number' },
        name: { type: 'string' },
        positions: { type: 'array', items: POINT },
        layout: LAYOUT,
        face: { description: '"actor", le nom d’un objet, ou un point : l’objet se tourne vers lui', anyOf: [{ type: 'string' }, POINT] },
        props: { type: 'object', description: 'réglages propres à l’objet (couleur, pose, toile d’un cadre…)' },
      },
      required: ['type'],
    },
  },
  {
    name: 'update_objects',
    description: 'Déplace, tourne ou renomme des objets existants (décor, personnages, grip).',
    parameters: {
      type: 'object',
      properties: {
        ids: { type: 'array', items: { type: 'string' }, description: 'ids ou noms des objets' },
        position: POINT,
        move: { ...POINT, description: 'déplacement relatif [dx, dy, dz]' },
        face: { anyOf: [{ type: 'string' }, POINT] },
        name: { type: 'string' },
        props: { type: 'object' },
      },
      required: ['ids'],
    },
  },
  {
    name: 'remove_items',
    description: 'Supprime des projecteurs ou des objets (ids ou noms). Seulement si l’utilisateur le demande.',
    parameters: { type: 'object', properties: { ids: { type: 'array', items: { type: 'string' } } }, required: ['ids'] },
  },
  {
    name: 'create_group',
    description: 'Crée un groupe (sélection nommée) de projecteurs.',
    parameters: { type: 'object', properties: { name: { type: 'string' }, target: TARGET }, required: ['name', 'target'] },
  },
  {
    name: 'set_environment',
    description: 'Ambiance de la vue : brume, exposition de la caméra, lumière ambiante, sol, cyclo.',
    parameters: {
      type: 'object',
      properties: {
        haze: { type: 'number', description: 'brume en % (0–100)' },
        exposure: { type: 'number', description: 'exposition en IL (−10 à +3)' },
        ambient: { type: 'number', description: 'lumière ambiante en % (0–100)' },
        floor: { type: 'boolean' },
        cyc: { type: 'boolean' },
      },
    },
  },
  {
    name: 'place_from_drawing',
    description:
      'Reconstruit un plan de feu DESSINÉ (photo jointe) en un seul appel : liste chaque élément vu sur le dessin avec sa place dans l’image, x et y de 0 à 1 (0,0 = coin en haut à gauche ; le haut du dessin = le fond du plateau, le bas = côté caméra). ARKLIGHT convertit en mètres, choisit le modèle de la bibliothèque, oriente les projecteurs selon leurs flèches.',
    parameters: {
      type: 'object',
      properties: {
        width_m: { type: 'number', description: 'largeur réelle couverte par l’image, en mètres (d’après les cotes ou l’échelle écrite ; sinon 10)' },
        items: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              kind: { type: 'string', enum: ['fixture', 'grip', 'person', 'camera', 'set'] },
              label: { type: 'string', description: 'ce qui est écrit à côté, ou ce qui est dessiné (« SKY S60 face 3200K », « 8x8 diff », « COMÉDIENNE », « canapé »)' },
              type: { type: 'string', description: 'id de la bibliothèque si tu le connais' },
              x: { type: 'number' },
              y: { type: 'number' },
              x2: { type: 'number', description: 'fin d’une rangée d’éléments, ou d’un mur' },
              y2: { type: 'number' },
              count: { type: 'number', description: 'nombre, si l’élément est multiple (« 4 x TITAN »)' },
              aim_x: { type: 'number', description: 'pointe de la flèche du projecteur, ou point regardé (caméra, personnage)' },
              aim_y: { type: 'number' },
              height: { type: 'number', description: 'hauteur en mètres, si elle est écrite' },
              cct: { type: 'number' },
              hue: { type: 'number' },
              saturation: { type: 'number' },
              intensity: { type: 'number' },
              focal: { type: 'number', description: 'focale de la caméra (mm)' },
            },
            required: ['kind', 'label', 'x', 'y'],
          },
        },
      },
      required: ['items'],
    },
  },
  {
    name: 'manage_plans',
    description: 'Séquences et plans du projet (un plan de feu par plan) : lister, ouvrir, créer, renommer.',
    parameters: {
      type: 'object',
      properties: {
        action: { type: 'string', enum: ['list', 'switch', 'new_plan', 'new_sequence', 'rename_plan', 'rename_sequence'] },
        id: { type: 'string', description: 'id ou nom du plan / de la séquence' },
        name: { type: 'string' },
        mode: { type: 'string', enum: ['copy', 'decor', 'empty'], description: 'départ d’un nouveau plan : copie du plan actuel, même décor sans lumière, vide' },
      },
      required: ['action'],
    },
  },
];

// ------------------------------------------------------------------ catalogue (prompt système, recherche)
export function catalogText() {
  const fx = FIXTURE_TYPES.map((t) => `${t.id} — ${t.brand} ${t.name}${t.dmx?.modes?.length ? '' : ' (sans DMX)'}`).join('\n');
  const props = PROP_TYPES.map((t) => `${t.id} — ${t.short || t.name}`).join('; ');
  const grip = GRIP_TYPES.map((t) => `${t.id} — ${t.short || t.name}`).join('; ');
  return `PROJECTEURS (id — marque modèle) :\n${fx}\n\nDÉCOR, PERSONNAGES, VÉHICULES (id — nom) : ${props}\n\nGRIP ET PIEDS (id — nom) : ${grip}`;
}

function score(text, words) {
  const t = norm(text);
  return words.reduce((s, w) => s + (t.includes(w) ? (t.split(/[^a-z0-9]+/).includes(w) ? 3 : 1) : 0), 0);
}

function searchCatalog({ query, kind = 'fixture' }) {
  const words = norm(query).split(/[^a-z0-9.]+/).filter(Boolean);
  if (kind === 'object') {
    const all = [...PROP_TYPES.map((t) => ({ t, k: 'décor' })), ...GRIP_TYPES.map((t) => ({ t, k: 'grip' }))];
    const found = all
      .map(({ t, k }) => ({ id: t.id, name: t.name, kind: k, s: score(`${t.id} ${t.name} ${t.short || ''} ${t.cat || t.family || ''}`, words) }))
      .filter((x) => x.s > 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, 12);
    return { results: found.map(({ s, ...x }) => (void s, x)), summary: `${found.length} objet(s) trouvé(s)` };
  }
  const found = FIXTURE_TYPES.map((t) => ({ t, s: score(`${t.id} ${t.brand} ${t.name} ${t.model || ''} ${t.category}`, words) }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s)
    .slice(0, 8)
    .map(({ t }) => ({ id: t.id, name: `${t.brand} ${t.name}`, category: t.category, modes: (t.dmx?.modes || []).map((m) => ({ id: m.id, name: m.label || m.name, channels: modeFootprint(t, m.id) })) }));
  return { results: found, summary: `${found.length} projecteur(s) trouvé(s)` };
}

/** Modèle de projecteur : id exact, ou meilleure correspondance du nom (« s60 » → arri.skypanel-s60c). */
function findFixtureType(q) {
  const exact = getFixtureType(String(q || '').trim());
  if (exact) return exact;
  const words = norm(q).split(/[^a-z0-9.]+/).filter(Boolean);
  const best = FIXTURE_TYPES.map((t) => ({ t, s: score(`${t.id} ${t.brand} ${t.name} ${t.model || ''}`, words) })).sort((a, b) => b.s - a.s)[0];
  return best?.s > 0 ? best.t : null;
}

function findObjectType(q) {
  const exact = getObjectType(String(q || '').trim());
  if (exact) return exact;
  const words = norm(q).split(/[^a-z0-9.]+/).filter(Boolean);
  const best = [...PROP_TYPES, ...GRIP_TYPES].map((t) => ({ t, s: score(`${t.id} ${t.name} ${t.short || ''}`, words) })).sort((a, b) => b.s - a.s)[0];
  return best?.s > 0 ? best.t : null;
}

// ------------------------------------------------------------------ plan dessiné
const COLOR_WORDS = [
  [/bleu|blue/, 220],
  [/cyan|turquoise/, 190],
  [/vert|green/, 120],
  [/jaune|yellow/, 55],
  [/ambre|amber|orange/, 30],
  [/rouge|red/, 0],
  [/magenta|fuchsia/, 300],
  [/rose|pink/, 330],
  [/violet|purple|mauve/, 275],
];

/** Couleur et température écrites sur le dessin (« 3200K », « bleu »). */
function lookFromLabel(label) {
  const t = norm(label);
  const out = {};
  const k = /(\d{4,5})\s*k\b/.exec(t);
  if (k) out.cct = Number(k[1]);
  for (const [re, hue] of COLOR_WORDS)
    if (re.test(t)) {
      out.hue = hue;
      out.saturation = 100;
      break;
    }
  const pct = /(\d{1,3})\s*%/.exec(t);
  if (pct) out.intensity = Number(pct[1]);
  return out;
}

/** Grip d'après ce qui est écrit : taille de cadre, drapeau, réflecteur… et sa toile. */
function gripFromLabel(label) {
  const t = norm(label).replace(/\s*[x×]\s*/g, 'x');
  const frame = /\b(6x6|8x8|12x12|20x20)\b/.exec(t);
  let id = frame ? `frame-${frame[1]}` : /floppy/.test(t) ? 'floppy' : /drapeau|flag|cutter/.test(t) ? 'flag-24x36' : /poly|4x8/.test(t) ? 'poly-4x8' : /reflec|reflet|bounce/.test(t) ? 'reflector-1m' : /sac|sandbag/.test(t) ? 'sandbag' : /cadre|frame|butterfly/.test(t) ? 'frame-8x8' : 'frame-6x6';
  const cloth = /ultra/.test(t) ? 'ultrabounce' : /grid/.test(t) ? 'grid' : /noir|black|solid|duv/.test(t) ? 'solid' : /\b216\b/.test(t) ? 'lee-216' : /\b250\b/.test(t) ? 'lee-250' : /\b251\b/.test(t) ? 'lee-251' : /\b252\b/.test(t) ? 'lee-252' : /silk|soie|diff/.test(t) ? 'silk' : /net|filet/.test(t) ? 'net-single' : null;
  const type = getObjectType(id);
  return { type, props: cloth && type?.cloths?.includes(cloth) ? { cloth } : {} };
}

function personFromLabel(label) {
  const t = norm(label);
  if (/enfant|child|kid|bebe/.test(t)) return 'person-child';
  if (/assis|sitting|seated/.test(t)) return 'person-sitting';
  if (/femme|comedienne|actrice|chanteuse|danseuse|woman|girl|mere|fille|she\b/.test(t)) return 'person-woman';
  return 'person-man';
}

// ------------------------------------------------------------------ exécution
/**
 * @param {object} o
 * @param {import('../scene-store.js').SceneStore} o.store
 * @param {object} o.hooks  { planChanging(), planChanged() } (séquences et plans)
 */
export function createToolRunner({ store, hooks = {}, context = {} }) {
  const changed = new Set(); // 'fixtures' | 'objects' | 'groups' | 'env' | 'plans'
  let lastAdded = [];

  const fixtures = () => store.scene.fixtures;
  const fixtureLabel = (f) => `${f.name}${f.settings.fixtureNumber ? ` (Fix ${f.settings.fixtureNumber})` : ''}`;

  /** Projecteurs visés (ids, noms, numéros de fixture, groupe, modèle, tous). */
  function resolveTarget(target = {}) {
    if (typeof target === 'string') target = { ids: [target] };
    if (Array.isArray(target)) target = { ids: target };
    const out = new Map();
    const all = fixtures();
    if (target.all) for (const f of all) out.set(f.id, f);
    for (const raw of target.ids || []) {
      const key = String(raw);
      const f = all.find((x) => x.id === key) || all.find((x) => norm(x.name) === norm(key)) || all.find((x) => String(x.settings.fixtureNumber) === key.replace(/^fix\s*/i, '')) || all.find((x) => norm(x.name).includes(norm(key)));
      if (f) out.set(f.id, f);
    }
    if (target.group) {
      const g = store.scene.groups.find((x) => norm(x.name) === norm(target.group)) || store.scene.groups.find((x) => norm(x.name).includes(norm(target.group)));
      for (const id of g?.ids || []) {
        const f = store.getFixture(id);
        if (f) out.set(id, f);
      }
    }
    if (target.type) {
      const t = findFixtureType(target.type);
      for (const f of all) if (t && f.type === t.id) out.set(f.id, f);
    }
    return [...out.values()];
  }

  /** Point visé : le comédien, un objet de la scène (par son nom), ou des coordonnées. */
  function resolvePoint(v) {
    const p = vec3(v);
    if (p) return p;
    if (v === undefined || v === null || v === '') return null;
    const key = norm(v);
    if (['actor', 'acteur', 'comedien', 'comedienne', 'personnage'].includes(key)) {
      const person = store.scene.objects.find((o) => getObjectType(o.type)?.model === 'person');
      return person ? [person.position[0], 1.55, person.position[2]] : ACTOR_AIM;
    }
    const o = store.scene.objects.find((x) => norm(x.name) === key) || store.scene.objects.find((x) => norm(x.name).includes(key));
    if (o) {
      const person = getObjectType(o.type)?.model === 'person';
      return [o.position[0], person ? 1.55 : o.position[1] + 0.5, o.position[2]];
    }
    const f = fixtures().find((x) => norm(x.name).includes(key));
    return f ? [...f.position] : null;
  }

  /** Positions : explicites, ou réparties (ligne, arc, grille) ; par défaut un arc devant le comédien. */
  function layoutPositions(count, positions, layout, defaultY = 2.4) {
    const list = (Array.isArray(positions) ? positions.map(vec3).filter(Boolean) : []).slice(0, count);
    if (list.length >= count) return list;
    const n = count - list.length;
    const L = layout || {};
    const pts = [];
    if (L.kind === 'line' && vec3(L.from) && vec3(L.to)) {
      const [a, b] = [vec3(L.from), vec3(L.to)];
      for (let i = 0; i < n; i++) {
        const k = n === 1 ? 0.5 : i / (n - 1);
        pts.push(a.map((v, j) => r3(v + (b[j] - v) * k)));
      }
    } else if (L.kind === 'grid') {
      const c = vec3(L.center) || [0, defaultY, 2];
      const cols = Math.max(1, Math.round(num(L.cols, Math.ceil(Math.sqrt(n)))));
      const rows = Math.ceil(n / cols);
      const sp = num(L.spacing, 1);
      for (let i = 0; i < n; i++) {
        const col = i % cols;
        const row = Math.floor(i / cols);
        pts.push([r3(c[0] + (col - (cols - 1) / 2) * sp), c[1], r3(c[2] + (row - (rows - 1) / 2) * sp)]);
      }
    } else {
      // arc autour du centre (le comédien) ; 0° = vers la caméra
      const c = vec3(L.center) || [0, defaultY, 0];
      const r = num(L.radius, 3.5);
      const a0 = num(L.start_angle, n === 1 ? -35 : -60);
      const a1 = num(L.end_angle, n === 1 ? -35 : 60);
      for (let i = 0; i < n; i++) {
        const a = ((n === 1 ? a0 : a0 + ((a1 - a0) * i) / (n - 1)) * Math.PI) / 180;
        pts.push([r3(c[0] + r * Math.sin(a)), c[1], r3(c[2] + r * Math.cos(a))]);
      }
    }
    return [...list, ...pts];
  }

  /** Réglage de couleur et d'intensité, selon le projecteur (Astera autonome, tungstène, LED). */
  function lookChanges(f, a) {
    const type = getFixtureType(f.type);
    const has = (k) => a[k] !== undefined && a[k] !== null;
    const changes = {};
    if (has('power')) changes.state = { power: !!a.power };
    if (!['intensity', 'cct', 'hue', 'saturation'].some(has)) return changes;
    const intensity = has('intensity') ? clamp(num(a.intensity, 60), 0, 100) : null;
    const colorful = has('hue') || (has('saturation') && num(a.saturation, 0) > 0);
    if (f.type.startsWith('astera.') && type.category !== 'device') {
      // Astera sans DMX : couleur du mode autonome (programme 0, couleur fixe)
      const sa = f.settings.standalone || {};
      const c0 = sa.colors?.[0] || { kind: 'hsi', hue: 0, sat: 0, cct: 5600 };
      const spec = colorful
        ? { kind: 'hsi', hue: clamp(num(a.hue, c0.hue || 0), 0, 360), sat: clamp(num(a.saturation, 100), 1, 100), cct: 5600 }
        : has('cct')
          ? { kind: 'hsi', hue: 0, sat: 0, cct: clamp(num(a.cct, 5600), 1750, 20000) }
          : c0;
      changes.settings = { standalone: { speed: 5, fade: 50, ...sa, program: 0, intensity: intensity ?? sa.intensity ?? 100, colors: [spec, spec, spec, spec] } };
      return changes;
    }
    const m = f.state.manual || {};
    const manual = { ...m };
    if (intensity !== null) manual.intensity = r3(intensity / 100);
    if (!isTraditionalType(type)) {
      if (colorful) Object.assign(manual, { mode: 'hsi', hue: clamp(num(a.hue, m.hue || 0), 0, 360), sat: r3(clamp(num(a.saturation, 100), 0, 100) / 100) });
      else if (has('cct')) Object.assign(manual, { mode: 'cct', cct: clamp(num(a.cct, 5600), 1700, 20000), tint: 0 });
    }
    changes.state = { ...(changes.state || {}), manual };
    return changes;
  }

  function modeOf(type, q) {
    if (!q || !type.dmx?.modes?.length) return null;
    const key = norm(q);
    return type.dmx.modes.find((m) => norm(m.id) === key) || type.dmx.modes.find((m) => norm(m.label || m.name) === key) || type.dmx.modes.find((m) => norm(m.label || m.name).includes(key)) || null;
  }

  /** Qui commande le projecteur : la console ARKLIGHT, le DMX reçu (ou conservé), ou ses propres réglages. */
  function controlOf(f, type) {
    if (consoleHeld(f)) return 'console ARKLIGHT';
    const dmx = hooks.dmxState?.() || { data: new Set(), live: new Set() };
    const u = f.settings.universe;
    const viaDmx = isAsteraV5(type) ? (f.settings.crmxLinked || f.settings.wired) && dmx.data.has(u) : type.pixels && controlSource(f, dmx.data.has(u)) === 'dmx';
    if (viaDmx) return dmx.live.has(u) ? `DMX reçu (univers ${u})` : `DMX conservé, plus de signal (univers ${u}) : release_control pour rendre la main`;
    return isAsteraV5(type) ? 'mode autonome' : 'réglage manuel';
  }

  function sceneSummary({ include_objects = true } = {}) {
    const cur = currentPlan(store.planInfo());
    const env = store.scene.environment;
    return {
      project: store.scene.name,
      sequence: cur?.seq.name,
      plan: cur?.plan.name,
      plans_total: flatPlans(store.planInfo()).length,
      environment: { haze_pct: Math.round(env.haze * 100), exposure_ev: env.exposure, ambient_pct: Math.round((env.ambient || 0) * 100), floor: env.floor, cyc: env.cyc },
      fixtures: fixtures().map((f) => {
        const type = getFixtureType(f.type);
        const mode = getMode(type, f.settings.dmxMode);
        const fp = mode ? modeFootprint(type, mode.id) : 0;
        const m = f.state.manual || {};
        const sa = f.settings.standalone;
        return {
          id: f.id,
          name: f.name,
          type: f.type,
          fix: f.settings.fixtureNumber ?? null,
          dmx: fp ? `U${f.settings.universe} ${f.settings.dmxAddress}-${f.settings.dmxAddress + fp - 1} (${fp} c., ${mode.label || mode.name})` : 'sans DMX',
          pos: f.position.map(r2),
          on: f.state.power !== false,
          control: controlOf(f, type),
          look: sa && f.type.startsWith('astera.') ? `autonome ${sa.intensity ?? 100} %` : `${m.mode || 'cct'} ${Math.round((m.intensity ?? 0.6) * 100)} %${m.mode === 'hsi' ? ` teinte ${Math.round(m.hue || 0)}°` : ` ${m.cct || 5600} K`}`,
        };
      }),
      groups: store.scene.groups.map((g) => ({ name: g.name, count: g.ids.length })),
      ...(include_objects !== false ? { objects: store.scene.objects.map((o) => ({ id: o.id, name: o.name, type: o.type, pos: o.position.map(r2) })) } : {}),
    };
  }

  // ---------------------------------------------------------------- les outils
  const RUN = {
    get_scene: (a) => {
      const s = sceneSummary(a);
      return { ...s, summary: `${s.fixtures.length} projecteur(s), ${store.scene.objects.length} objet(s)` };
    },

    search_catalog: (a) => searchCatalog(a),

    release_control(a) {
      const list = a.target ? resolveTarget(a.target) : fixtures();
      const held = list.filter(consoleHeld);
      for (const f of held) store.updateFixture(f.id, { state: { console: null } });
      if (held.length) changed.add('fixtures');
      const universes = [...new Set(list.map((f) => f.settings.universe))];
      const forgotten = hooks.forgetDmx?.(universes) || [];
      const parts = [];
      if (held.length) parts.push(`${held.length} projecteur(s) relâché(s) de la console`);
      if (forgotten.length) parts.push(`DMX conservé oublié (univers ${forgotten.join(', ')})`);
      return { released: held.map((f) => f.name), dmx_forgotten: forgotten, summary: parts.join(' ; ') || 'rien à relâcher : les projecteurs suivent déjà leurs réglages (ou un DMX encore envoyé)' };
    },

    add_fixtures(a) {
      const type = findFixtureType(a.type);
      if (!type) return { error: `Modèle inconnu : ${a.type}. Cherche avec search_catalog.` };
      // nombre oublié par le modèle : celui des positions données
      const given = Array.isArray(a.positions) ? a.positions.filter(vec3).length : 0;
      const count = clamp(Math.round(num(a.count, given > 1 ? given : 1)), 1, 64);
      const tube = isTube(type);
      const pts = layoutPositions(count, a.positions, a.layout, tube ? r3((type.geometry.length || 1) / 2 + 0.05) : 2.4).map((pt) => liftFixture(type, pt));
      const target = a.aim === undefined ? (tube ? null : ACTOR_AIM) : resolvePoint(a.aim);
      const mode = modeOf(type, a.mode);
      const universe = Math.max(1, Math.round(num(a.universe, 1)));
      let address = a.start_address !== undefined ? clamp(Math.round(num(a.start_address, 1)), 1, 512) : null;
      const added = [];
      for (let i = 0; i < count; i++) {
        const name = a.name ? (count > 1 ? `${a.name} ${i + 1}` : a.name) : undefined;
        const settings = { universe, ...(mode ? { dmxMode: mode.id } : {}), ...(address !== null ? { dmxAddress: address } : {}) };
        const f = store.addFixture({ type: type.id, name, position: pts[i], rotation: target ? fixtureRotation(type, pts[i], target) : [0, 0, 0], settings });
        if (address !== null) address += modeFootprint(type, f.settings.dmxMode);
        const look = lookChanges(f, a);
        if (look.state || look.settings) store.updateFixture(f.id, look);
        added.push(f);
      }
      changed.add('fixtures');
      if (a.group) {
        store.createGroup({ name: a.group, ids: added.map((f) => f.id) });
        changed.add('groups');
      }
      lastAdded = added.map((f) => f.id);
      return {
        added: added.map((f) => ({ id: f.id, name: f.name, fix: f.settings.fixtureNumber, universe: f.settings.universe, address: f.settings.dmxAddress, pos: f.position })),
        summary: `${added.length} × ${type.brand} ${type.name} ajouté(s)`,
      };
    },

    update_fixtures(a) {
      const list = resolveTarget(a.target);
      if (!list.length) return { error: 'Aucun projecteur trouvé : appelle get_scene pour connaître les ids.' };
      const target = a.aim !== undefined ? resolvePoint(a.aim) : null;
      if (a.aim !== undefined && !target) return { error: `Cible inconnue : ${JSON.stringify(a.aim)}` };
      // plusieurs projecteurs et une seule position : c'est le centre du groupe (ils gardent leurs écarts)
      const groupMove = list.length > 1 && vec3(a.position) ? vec3(a.position).map((v, i) => v - list.reduce((sum, f) => sum + f.position[i], 0) / list.length) : null;
      for (const f of list) {
        const type = getFixtureType(f.type);
        const changes = lookChanges(f, a);
        let pos = f.position;
        if (groupMove) pos = pos.map((v, i) => r3(v + groupMove[i]));
        else if (vec3(a.position)) pos = vec3(a.position);
        if (vec3(a.move)) pos = pos.map((v, i) => r3(v + vec3(a.move)[i]));
        if (pos !== f.position) changes.position = pos = liftFixture(type, pos);
        if (target) changes.rotation = fixtureRotation(type, pos, target);
        if (typeof a.name === 'string' && a.name.trim()) changes.name = list.length > 1 ? `${a.name.trim()} ${list.indexOf(f) + 1}` : a.name.trim();
        const mode = modeOf(type, a.mode);
        if (mode || a.support) changes.settings = { ...(changes.settings || {}), ...(mode ? { dmxMode: mode.id } : {}), ...(a.support ? { support: String(a.support) } : {}) };
        store.updateFixture(f.id, changes);
      }
      changed.add('fixtures');
      return { updated: list.map(fixtureLabel), summary: `${list.length} projecteur(s) réglé(s)` };
    },

    readdress(a) {
      const list = resolveTarget(a.target);
      if (!list.length) return { error: 'Aucun projecteur trouvé.' };
      const plan = planAddressing(fixtures(), { scope: 'ids', ids: list.map((f) => f.id), order: a.order || 'scene', universe: num(a.universe, 1), address: num(a.start_address, 1), universes: 'pack', overflow: true });
      for (const m of plan.moves) store.updateFixture(m.id, { settings: { universe: m.to.universe, dmxAddress: m.to.address } });
      let numbered = 0;
      if (a.fixture_start !== undefined) {
        let n = Math.max(1, Math.round(num(a.fixture_start, 1)));
        const order = new Map(plan.moves.map((m, i) => [m.id, i]));
        const sorted = [...list].filter((f) => getFixtureType(f.type)?.dmx?.modes?.length).sort((x, y) => x.settings.universe - y.settings.universe || x.settings.dmxAddress - y.settings.dmxAddress || (order.get(x.id) ?? 0) - (order.get(y.id) ?? 0));
        for (const f of sorted) {
          store.updateFixture(f.id, { settings: { fixtureNumber: n++ } });
          numbered++;
        }
      }
      changed.add('fixtures');
      const patch = list
        .filter((f) => modeFootprint(getFixtureType(f.type), f.settings.dmxMode) > 0)
        .map((f) => `${fixtureLabel(f)} : U${f.settings.universe}.${String(f.settings.dmxAddress).padStart(3, '0')}`);
      return { patch, unplaced: plan.unplaced.length, conflicts_after: plan.conflictsAfter, summary: `${plan.moves.length} projecteur(s) ré-adressé(s)${numbered ? `, ${numbered} renuméroté(s)` : ''}` };
    },

    add_objects(a) {
      const type = findObjectType(a.type);
      if (!type) return { error: `Objet inconnu : ${a.type}. Cherche avec search_catalog (kind: object).` };
      // nombre oublié par le modèle : celui des positions données
      const given = Array.isArray(a.positions) ? a.positions.filter(vec3).length : 0;
      const count = clamp(Math.round(num(a.count, a.count === undefined && given > 1 ? given : 1)), 1, 64);
      const prop = isProp(type);
      const minY = prop ? 0 : r3((type.h || type.size?.[1] || 0) / 2);
      const pts = layoutPositions(count, a.positions, a.layout ? { ...a.layout } : { kind: 'line', from: [-1.5, prop ? 0 : 1.6, -1], to: [1.5, prop ? 0 : 1.6, -1] }, prop ? 0 : 1.6).map((pt) => [pt[0], Math.max(minY, pt[1]), pt[2]]);
      // une caméra regarde le comédien si rien d'autre n'est demandé
      const face = a.face !== undefined ? resolvePoint(a.face) : isCamera(type) ? resolvePoint('actor') : null;
      const added = [];
      for (let i = 0; i < count; i++) {
        const pos = grounded(type) ? [pts[i][0], 0, pts[i][2]] : pts[i];
        const name = a.name ? (count > 1 ? `${a.name} ${i + 1}` : a.name) : undefined;
        const yaw = face ? faceYaw(pos, face) : 0;
        added.push(store.addObject({ type: type.id, name, position: pos, rotation: [0, yaw, 0], props: a.props && typeof a.props === 'object' ? a.props : {} }));
      }
      changed.add('objects');
      return { added: added.map((o) => ({ id: o.id, name: o.name, pos: o.position })), summary: `${added.length} × ${type.short || type.name} ajouté(s)` };
    },

    update_objects(a) {
      const ids = (a.ids || []).map(String);
      const objs = ids.map((k) => store.scene.objects.find((o) => o.id === k) || store.scene.objects.find((o) => norm(o.name) === norm(k)) || store.scene.objects.find((o) => norm(o.name).includes(norm(k)))).filter(Boolean);
      if (!objs.length) return { error: 'Aucun objet trouvé : appelle get_scene.' };
      const face = a.face !== undefined ? resolvePoint(a.face) : null;
      const groupMove = objs.length > 1 && vec3(a.position) ? vec3(a.position).map((v, i) => v - objs.reduce((sum, o) => sum + o.position[i], 0) / objs.length) : null;
      for (const o of objs) {
        let pos = o.position;
        if (groupMove) pos = pos.map((v, i) => r3(v + groupMove[i]));
        else if (vec3(a.position)) pos = vec3(a.position);
        if (vec3(a.move)) pos = pos.map((v, i) => r3(v + vec3(a.move)[i]));
        if (grounded(getObjectType(o.type))) pos = [pos[0], 0, pos[2]];
        store.updateObject(o.id, { position: pos, ...(face ? { rotation: [0, faceYaw(pos, face), 0] } : {}), ...(typeof a.name === 'string' ? { name: a.name } : {}), ...(a.props ? { props: a.props } : {}) });
      }
      changed.add('objects');
      return { updated: objs.map((o) => o.name), summary: `${objs.length} objet(s) modifié(s)` };
    },

    remove_items(a) {
      const removed = [];
      for (const k of (a.ids || []).map(String)) {
        const f = resolveTarget({ ids: [k] })[0];
        if (f && store.removeFixture(f.id)) {
          removed.push(f.name);
          changed.add('fixtures');
          changed.add('groups');
          continue;
        }
        const o = store.scene.objects.find((x) => x.id === k) || store.scene.objects.find((x) => norm(x.name) === norm(k));
        if (o && store.removeObject(o.id)) {
          removed.push(o.name);
          changed.add('objects');
        }
      }
      return { removed, summary: `${removed.length} élément(s) supprimé(s)` };
    },

    create_group(a) {
      const list = resolveTarget(a.target);
      if (!list.length) return { error: 'Aucun projecteur trouvé.' };
      const g = store.createGroup({ name: a.name, ids: list.map((f) => f.id) });
      changed.add('groups');
      return { group: g.name, count: g.ids.length, summary: `Groupe « ${g.name} » (${g.ids.length})` };
    },

    set_environment(a) {
      const c = {};
      if (a.haze !== undefined) c.haze = r3(clamp(num(a.haze, 25), 0, 100) / 100);
      if (a.exposure !== undefined) c.exposure = clamp(Math.round(num(a.exposure, 0) * 2) / 2, -10, 3);
      if (a.ambient !== undefined) c.ambient = r3(clamp(num(a.ambient, 4), 0, 50) / 100);
      if (typeof a.floor === 'boolean') c.floor = a.floor;
      if (typeof a.cyc === 'boolean') c.cyc = a.cyc;
      store.updateEnvironment(c);
      changed.add('env');
      return { environment: c, summary: 'Ambiance réglée' };
    },

    place_from_drawing(a) {
      const items = (Array.isArray(a.items) ? a.items : []).filter((it) => it && Number.isFinite(Number(it.x)) && Number.isFinite(Number(it.y)));
      if (!items.length) return { error: 'Aucun élément : donne items avec x et y entre 0 et 1.' };
      const W = clamp(num(a.width_m, 10), 2, 80);
      const H = W / (context.imageAspect || 4 / 3);
      // origine : le premier personnage dessiné devient le centre du plateau (sinon le centre de l'image)
      const who = items.find((it) => it.kind === 'person');
      const ox = who ? num(who.x, 0.5) : 0.5;
      const oy = who ? num(who.y, 0.5) : 0.5;
      const at = (x, y, h = 0) => [r3((clamp(num(x, 0.5), -0.25, 1.25) - ox) * W), h, r3((clamp(num(y, 0.5), -0.25, 1.25) - oy) * H)];
      /** Positions d'un élément multiple : le long de x,y → x2,y2, sinon une rangée de 1 m d'écart. */
      const spots = (it, h) => {
        const n = clamp(Math.round(num(it.count, 1)), 1, 32);
        const a0 = at(it.x, it.y, h);
        if (n === 1) return [a0];
        if (Number.isFinite(Number(it.x2)) && Number.isFinite(Number(it.y2))) {
          const b = at(it.x2, it.y2, h);
          return Array.from({ length: n }, (_, i) => a0.map((v, j) => r3(v + ((b[j] - v) * i) / (n - 1))));
        }
        return Array.from({ length: n }, (_, i) => [r3(a0[0] + i - (n - 1) / 2), h, a0[2]]);
      };
      const aimOf = (it) => (Number.isFinite(Number(it.aim_x)) && Number.isFinite(Number(it.aim_y)) ? at(it.aim_x, it.aim_y, 1.55) : null);
      const label = (it) => String(it.label || '').replace(/\s+/g, ' ').trim().slice(0, 40);
      const made = [];
      const guessed = [];

      // le comédien par défaut d'un projet neuf laisse la place aux personnages dessinés
      if (who) {
        const persons = store.scene.objects.filter((o) => getObjectType(o.type)?.model === 'person');
        const d = persons.length === 1 ? persons[0] : null;
        if (d && d.type === 'person-man' && d.position.every((v) => v === 0) && /^(comedien|actor)$/.test(norm(d.name))) {
          store.removeObject(d.id);
          changed.add('objects');
        }
      }
      const order = { person: 0, set: 1, camera: 2, grip: 3, fixture: 4 };
      for (const it of [...items].sort((p, q) => (order[p.kind] ?? 5) - (order[q.kind] ?? 5))) {
        const name = label(it);
        if (it.kind === 'fixture') {
          let type = findFixtureType(it.type) || findFixtureType(it.label);
          if (!type) {
            type = getFixtureType('arri.skypanel-s60c');
            guessed.push(`${name} → ${type.brand} ${type.name}`);
          }
          const tube = isTube(type);
          const h = num(it.height, tube ? r3((type.geometry.length || 1) / 2 + 0.05) : 2.6);
          const target = aimOf(it) || resolvePoint('actor');
          const look = { ...lookFromLabel(it.label), ...Object.fromEntries(['cct', 'hue', 'saturation', 'intensity'].filter((k) => Number.isFinite(Number(it[k]))).map((k) => [k, Number(it[k])])) };
          const pts = spots(it, h);
          pts.forEach((pt, i) => {
            const pos = liftFixture(type, pt);
            const f = store.addFixture({ type: type.id, name: pts.length > 1 ? `${name} ${i + 1}` : name, position: pos, rotation: fixtureRotation(type, pos, target) });
            const ch = lookChanges(f, look);
            if (ch.state || ch.settings) store.updateFixture(f.id, ch);
            made.push(`${f.name} (${type.brand} ${type.name}) ${pos.map(r2).join(' / ')}`);
            lastAdded.push(f.id);
          });
          changed.add('fixtures');
          continue;
        }
        let type;
        let props = {};
        if (it.kind === 'grip') ({ type, props } = gripFromLabel(it.label));
        else if (it.kind === 'person') type = getObjectType(personFromLabel(it.label));
        else if (it.kind === 'camera') {
          type = getObjectType(/travelling|dolly|rail/.test(norm(it.label)) ? 'camera-dolly' : 'camera-tripod');
          const mm = Number(it.focal) || Number((/(\d{2,3})\s*mm/.exec(norm(it.label)) || [])[1]);
          if (mm) props = { focal: clamp(mm, 8, 300) };
        } else {
          type = (it.type && getObjectType(it.type)) || (/mur|wall|cloison/.test(norm(it.label)) ? getObjectType('wall-flat') : findObjectType(it.label));
          if (!type) {
            guessed.push(`${name} : non reconnu (ignoré)`);
            continue;
          }
        }
        const isGrip = it.kind === 'grip';
        const h = isGrip ? r3((type.h || 0) / 2 + 0.6) : 0;
        let pts = spots(it, h);
        let rotation = null;
        if (type.id.startsWith('wall') && Number.isFinite(Number(it.x2)) && Number.isFinite(Number(it.y2))) {
          // mur dessiné comme un trait : longueur et direction du trait
          const a0 = at(it.x, it.y);
          const b = at(it.x2, it.y2);
          // un pan de mur fait 8 m au plus : un long trait devient plusieurs pans alignés
          const len = clamp(Math.hypot(b[0] - a0[0], b[2] - a0[2]), 0.5, 80);
          const n = Math.ceil(len / 8);
          pts = Array.from({ length: n }, (_, i) => [r3(a0[0] + ((b[0] - a0[0]) * (i + 0.5)) / n), 0, r3(a0[2] + ((b[2] - a0[2]) * (i + 0.5)) / n)]);
          rotation = [0, r3(-Math.atan2(b[2] - a0[2], b[0] - a0[0])), 0];
          props = { width: r2(len / n) };
        }
        const look = aimOf(it) || (it.kind === 'camera' || isGrip ? resolvePoint('actor') : it.kind === 'person' ? [0, 1.6, 50] : null);
        for (const [i, pt] of pts.entries()) {
          const o = store.addObject({ type: type.id, name: pts.length > 1 ? `${name} ${i + 1}` : name, position: pt, rotation: rotation || [0, look ? faceYaw(pt, look) : 0, 0], props });
          made.push(`${o.name} (${type.short || type.name}) ${o.position.map(r2).join(' / ')}`);
        }
        changed.add('objects');
      }
      return { created: made, guessed, scale: `${r2(W)} m × ${r2(H)} m`, summary: `${made.length} élément(s) placé(s) d’après le dessin${guessed.length ? ` (${guessed.length} à vérifier)` : ''}` };
    },

    manage_plans(a) {
      const info = store.planInfo();
      const findPlan = (k) => flatPlans(info).find((x) => x.plan.id === k) || flatPlans(info).find((x) => norm(x.plan.name) === norm(k)) || flatPlans(info).find((x) => norm(x.plan.name).includes(norm(k)));
      const findSeq = (k) => info.sequences.find((s) => s.id === k) || info.sequences.find((s) => norm(s.name).includes(norm(k)));
      // renommer sans id : le plan / la séquence en cours (un nom vide est contenu dans tous les noms)
      const hasId = a.id !== undefined && a.id !== null && String(a.id) !== '';
      const list = () => ({ sequences: store.planInfo().sequences.map((s) => ({ id: s.id, name: s.name, plans: s.plans.map((p) => ({ id: p.id, name: p.name, fixtures: p.fixtures })) })), current: store.planInfo().current });
      switch (a.action) {
        case 'switch': {
          const p = findPlan(a.id);
          if (!p) return { error: `Plan introuvable : ${a.id}` };
          hooks.planChanging?.();
          store.switchPlan(p.plan.id);
          hooks.planChanged?.();
          changed.add('plans');
          return { ...list(), summary: `Plan ouvert : ${p.seq.name} › ${p.plan.name}` };
        }
        case 'new_plan': {
          hooks.planChanging?.();
          const p = store.newPlan({ mode: a.mode || 'copy', name: a.name });
          hooks.planChanged?.();
          changed.add('plans');
          return { ...list(), summary: `Nouveau plan : ${p.name}` };
        }
        case 'new_sequence': {
          hooks.planChanging?.();
          const s = store.newSequence({ mode: a.mode || 'empty', name: a.name });
          hooks.planChanged?.();
          changed.add('plans');
          return { ...list(), summary: `Nouvelle séquence : ${s.name}` };
        }
        case 'rename_plan': {
          const p = (hasId ? findPlan(a.id) : null) || currentPlan(info);
          if (p) store.renamePlan(p.plan.id, a.name);
          changed.add('plans');
          return { ...list(), summary: 'Plan renommé' };
        }
        case 'rename_sequence': {
          const s = (hasId ? findSeq(a.id) : null) || currentPlan(info)?.seq;
          if (s) store.renameSequence(s.id, a.name);
          changed.add('plans');
          return { ...list(), summary: 'Séquence renommée' };
        }
        default:
          return { ...list(), summary: `${flatPlans(info).length} plan(s)` };
      }
    },
  };

  return {
    changed,
    get lastAdded() {
      return lastAdded;
    },
    /** Exécute un outil ; les erreurs deviennent un résultat que le modèle peut lire. */
    run(name, args) {
      const fn = RUN[name];
      if (!fn) return { error: `Outil inconnu : ${name}` };
      try {
        return fn(args && typeof args === 'object' ? args : {});
      } catch (e) {
        return { error: e.message };
      }
    },
  };
}
