// Section GRIP : éléments plaçables dans la scène (cadres, drapeaux, réflecteurs, cubes, sacs…),
// dessinés en 3D et sur le plan de feu, et comptés dans la liste du matériel. Ils agissent sur la
// lumière (voir web/js/viewport/light-grip.js). Les cadres prennent aussi toutes les diffusions,
// corrections, neutres et couleurs du nuancier (shared/filters.js).
// Dimensions : docs/recherche/marques/notes-partielles.md (drapeaux, cubes, toiles) ; sinon tailles nominales.
import { FILTERS, getFilter } from './filters.js';

const ft = (n) => Math.round(n * 0.3048 * 1000) / 1000;
/** Pieds possibles sous un cadre (inspecteur, clic droit › Pieds). */
export const FRAME_STANDS = ['long-john', 'super-windup', 'windup', 'stand-junior', 'stand-baby-5k'];
const inch = (n) => Math.round(n * 0.0254 * 1000) / 1000;

const r2 = (v) => Math.round(v * 100) / 100;
/** Toile que la lumière traverse : perte en diaphragmes, part réémise en lumière douce, part renvoyée. */
const through = (name, short, group, color, opacity, stops, soft, reflect = 0, tint = null) => {
  const k = 2 ** -stops;
  return { name, short, group, color, opacity, stops, t: r2(k * (1 - soft)), diffuse: r2(k * soft), reflect, solid: false, tint };
};
/** Toile opaque : elle coupe la lumière et en renvoie une part. */
const opaque = (name, short, group, color, reflect, tint = null) => ({ name, short, group, color, opacity: 1, t: 0, diffuse: 0, reflect, solid: true, tint });
const WARM = [1, 0.78, 0.5];
const WARM_LIGHT = [1, 0.85, 0.65];
const CHECKER = [1, 0.92, 0.8];
const ECRU = [1, 0.93, 0.82];

/**
 * Toiles et garnitures des cadres et drapeaux. Pertes indicatives, d'après les fiches des loueurs et
 * des fabricants (soie full ≈ 1,6 diaph, soie 1/4 ≈ 0,6, china ≈ 1, grid 1/4 ≈ 1, 1/2 ≈ 2, full ≈ 2,6,
 * soft frost 1/2 ≈ 2, tulle simple ≈ 1/2) ; muslins, magic cloth et réflecteurs : valeurs d'usage.
 */
export const CLOTHS = {
  'silk-1-4': through('Soie 1/4 (1/4 Silk)', 'Soie 1/4', 'silk', 0xf4f4f0, 0.3, 0.6, 0.75),
  'silk-1-2': through('Soie 1/2 (1/2 Silk)', 'Soie 1/2', 'silk', 0xf3f3ef, 0.42, 1, 0.75),
  silk: through('Soie full (Full Silk)', 'Soie full', 'silk', 0xf2f2ee, 0.55, 1.6, 0.75),
  'china-silk': through('China Silk', 'China Silk', 'silk', 0xf4f1e8, 0.42, 1, 0.6),
  'grid-1-4': through('Grid 1/4 (1/4 Grid)', 'Grid 1/4', 'grid', 0xececec, 0.45, 1, 0.9),
  'grid-1-2': through('Grid 1/2 (Light Grid)', 'Grid 1/2', 'grid', 0xeaeaea, 0.6, 2, 0.9),
  grid: through('Grid full (Full Grid)', 'Grid full', 'grid', 0xe8e8e8, 0.7, 2.6, 0.9),
  'silent-grid-1-4': through('Silent Grid 1/4', 'Silent 1/4', 'grid', 0xececec, 0.45, 1, 0.9),
  'silent-grid-1-2': through('Silent Grid 1/2', 'Silent 1/2', 'grid', 0xeaeaea, 0.6, 2, 0.9),
  'silent-grid': through('Silent Grid full', 'Silent full', 'grid', 0xe8e8e8, 0.7, 2.6, 0.9),
  'soft-frost-1-2': through('Soft Frost 1/2', 'Frost 1/2', 'frost', 0xeeeeee, 0.6, 2, 0.85),
  'soft-frost': through('Soft Frost full', 'Frost full', 'frost', 0xeaeaea, 0.72, 2.5, 0.85),
  'muslin-bleached': through('Muslin blanchi (Bleached)', 'Muslin blanc', 'frost', 0xf5f3ec, 0.85, 2.5, 0.95, 0.5),
  'muslin-unbleached': through('Muslin écru (Unbleached)', 'Muslin écru', 'frost', 0xe6dcc4, 0.88, 3, 0.95, 0.4, ECRU),
  'magic-cloth': through('Magic Cloth', 'Magic Cloth', 'frost', 0xf0efe9, 0.75, 2, 0.9, 0.3),
  ultrabounce: opaque('Ultrabounce (réflecteur blanc)', 'Ultrabounce', 'bounce', 0xffffff, 0.85),
  griffolyn: opaque('Griffolyn (blanc / noir)', 'Griffolyn', 'bounce', 0xf6f6f2, 0.75),
  'silver-lame': opaque('Lamé argent (Silver Lamé)', 'Lamé argent', 'bounce', 0xc9cdd2, 0.9),
  'soft-silver': opaque('Soft Silver', 'Soft Silver', 'bounce', 0xd8dade, 0.8),
  'gold-lame': opaque('Lamé or (Gold Lamé)', 'Lamé or', 'bounce', 0xd8b060, 0.8, WARM),
  'soft-gold': opaque('Soft Gold', 'Soft Gold', 'bounce', 0xe2c88c, 0.7, WARM_LIGHT),
  checkerboard: opaque('Damier argent / or (Lisa Marie)', 'Damier', 'bounce', 0xd0c4a0, 0.8, CHECKER),
  solid: opaque('Noir (borniol / duvetyne)', 'Noir', 'cut', 0x111111, 0.03),
  'net-lavender': through('Tulle lavande', 'Tulle lavande', 'cut', 0x3a3048, 0.2, 1 / 3, 0),
  'net-single': through('Tulle simple', 'Tulle simple', 'cut', 0x222222, 0.35, 0.5, 0),
  'net-double': through('Tulle double', 'Tulle double', 'cut', 0x1a1a1a, 0.55, 1, 0),
  'net-triple': through('Tulle triple', 'Tulle triple', 'cut', 0x141414, 0.7, 1.5, 0),
  empty: { name: 'Vide (cadre seul)', short: 'Vide', color: 0x000000, opacity: 0, t: 1, diffuse: 0, reflect: 0, solid: false, tint: null },
};

export const GRIP_TYPES = [
  // cadres (butterfly) : posés sur deux pieds
  ...[
    [6, 'Cadre 6×6'],
    [8, 'Cadre 8×8'],
    [12, 'Cadre 12×12'],
    [20, 'Cadre 20×20'],
  ].map(([n, name]) => ({
    id: `frame-${n}x${n}`,
    name: `${name} (${(ft(n)).toFixed(1).replace('.', ',')} m)`,
    short: name,
    family: 'frame',
    w: ft(n),
    h: ft(n),
    cloths: [...Object.keys(CLOTHS), ...FILTERS.map((f) => f.id)],
    defaultCloth: 'silk',
    // deux pieds aux oreilles du cadre ; un 20×20 demande de gros pieds roulants (Long John Silver)
    stands: { id: n >= 20 ? 'long-john' : n >= 12 ? 'super-windup' : n >= 8 ? 'stand-junior' : 'stand-baby-5k', count: 2 },
    standChoices: FRAME_STANDS,
    sandbags: n >= 12 ? 4 : 2,
  })),
  // drapeaux : sur C-stand (bras + rotule)
  { id: 'flag-18x24', name: 'Drapeau 18×24" (45×60 cm)', short: 'Drapeau 18×24', family: 'flag', w: inch(18), h: inch(24), cloths: ['solid', 'silk-1-4', 'silk-1-2', 'silk', 'net-lavender', 'net-single', 'net-double'], defaultCloth: 'solid', stands: { id: 'cstand', count: 1, label: 'C-stand' }, sandbags: 1 },
  { id: 'flag-24x36', name: 'Drapeau 24×36" (60×90 cm)', short: 'Drapeau 24×36', family: 'flag', w: inch(24), h: inch(36), cloths: ['solid', 'silk-1-4', 'silk-1-2', 'silk', 'net-lavender', 'net-single', 'net-double'], defaultCloth: 'solid', stands: { id: 'cstand', count: 1, label: 'C-stand' }, sandbags: 1 },
  { id: 'floppy', name: 'Floppy 4×4 (120×120 cm)', short: 'Floppy', family: 'flag', w: 1.22, h: 1.22, cloths: ['solid'], defaultCloth: 'solid', stands: { id: 'cstand', count: 1, label: 'C-stand' }, sandbags: 1 },
  // réflecteurs
  { id: 'poly-4x8', name: 'Polystyrène 4×8 (1,2×2,4 m)', short: 'Polystyrène', family: 'bounce', w: 1.22, h: 2.44, color: 0xf4f4f0, stands: { id: 'stand-1000', count: 1, label: 'Pied de 1000 + pince' }, sandbags: 1 },
  { id: 'reflector-1m', name: 'Réflecteur pliable Ø 1 m', short: 'Réflecteur', family: 'bounce', w: 1.0, h: 1.0, round: true, color: 0xd9dde2, stands: { id: 'cstand', count: 1, label: 'C-stand' }, sandbags: 1 },
  // accessoires au sol
  { id: 'applebox-full', name: 'Cube plein (51×30×20 cm)', short: 'Cube plein', family: 'box', size: [0.51, 0.2, 0.3] },
  { id: 'applebox-half', name: 'Demi-cube (51×30×10 cm)', short: 'Demi-cube', family: 'box', size: [0.51, 0.1, 0.3] },
  { id: 'applebox-quarter', name: 'Quart de cube (51×30×5 cm)', short: 'Quart de cube', family: 'box', size: [0.51, 0.05, 0.3] },
  { id: 'applebox-pancake', name: 'Galette (51×30×2,5 cm)', short: 'Galette', family: 'box', size: [0.51, 0.025, 0.3] },
  { id: 'sandbag', name: 'Sac de sable', short: 'Sac de sable', family: 'bag', size: [0.45, 0.12, 0.25] },
  // pieds seuls (sans projecteur) : mêmes familles que les supports des projecteurs (supports.js)
  { id: 'stand-1000', name: 'Pied de 1000 (baby)', short: 'Pied 1000', family: 'stand', standLook: 'baby', height: 2.2, sandbags: 1 },
  { id: 'stand-baby-5k', name: 'Pied Baby 5K (combo)', short: 'Baby 5K', family: 'stand', standLook: 'combo', height: 1.8, sandbags: 1 },
  { id: 'stand-junior', name: 'Pied junior roulant (28 mm)', short: 'Pied junior', family: 'stand', standLook: 'junior', height: 2.6, sandbags: 2 },
  { id: 'windup', name: 'Wind-up (pied à manivelle)', short: 'Wind-up', family: 'stand', standLook: 'windup', height: 3.5, sandbags: 2 },
  { id: 'super-windup', name: 'Super Wind-Up (manivelle, roulant)', short: 'Super Wind-Up', family: 'stand', standLook: 'superwindup', height: 3.0, sandbags: 2 },
  { id: 'long-john', name: 'Long John Silver (roulant)', short: 'Long John', family: 'stand', standLook: 'longjohn', height: 4.0, sandbags: 2 },
  { id: 'cstand', name: 'C-stand (bras et rotule)', short: 'C-stand', family: 'stand', standLook: 'cstand', height: 2.0, sandbags: 1 },
  { id: 'lowboy', name: 'Low-boy (pied bas)', short: 'Low-boy', family: 'stand', standLook: 'lowboy', height: 1.0, sandbags: 1 },
];

const byId = new Map(GRIP_TYPES.map((t) => [t.id, t]));
export const getGripType = (id) => byId.get(id) || null;

/**
 * Pieds sous un cadre, un drapeau ou un réflecteur : ceux choisis par l'utilisateur (props.stand),
 * sinon ceux du type. Null pour les éléments posés (cubes, sacs) et les pieds seuls.
 */
export function gripStands(type, props = {}) {
  if (!type?.stands || type.family === 'stand') return null;
  const id = type.standChoices?.includes(props?.stand) ? props.stand : type.stands.id;
  return { id, count: type.stands.count || 1, label: id === type.stands.id ? type.stands.label : undefined, chosen: id !== type.stands.id };
}

/** Réglages d'un élément de grip : on retire un pied qui n'est pas proposé pour ce type. */
export function cleanGripProps(type, props) {
  const out = { ...props };
  if ('stand' in out && out.stand !== null && !type?.standChoices?.includes(out.stand)) delete out.stand;
  return out;
}

/** Nom d'une toile ou d'un filtre du nuancier (« Soie 1/4 (1/4 Silk) », « 216 — Full White Diffusion »). */
export const clothName = (id) => CLOTHS[id]?.name || getFilter(id)?.name || id;
/** Nom court pour le plan de feu (« Soie 1/4 », « 216 », « CTO »). */
export function clothShort(id) {
  if (CLOTHS[id]) return CLOTHS[id].short;
  const f = getFilter(id);
  return f ? f.name.split(' — ')[0].replace(/^Rosco /, 'R') : id;
}

export const GRIP_FAMILIES = [
  { id: 'frame', label: 'Cadres' },
  { id: 'flag', label: 'Drapeaux' },
  { id: 'bounce', label: 'Réflecteurs' },
  { id: 'stand', label: 'Pieds seuls' },
  { id: 'box', label: 'Cubes' },
  { id: 'bag', label: 'Sacs' },
];

/** Nouvel objet de grip, prêt à placer. */
export function defaultGripProps(type) {
  return {
    ...(type.cloths ? { cloth: type.defaultCloth } : {}),
    ...(type.family === 'stand' ? { height: type.height } : {}),
  };
}
