// Nuancier : toiles, diffusions, gélatines de correction (CTB, CTO, CTS, vert / magenta), neutres
// (ND) et couleurs. Pour les cadres, les boîtes à lumière (toile avant) et les porte-filtres des
// projecteurs. Repris du nuancier de l'appli ARK-LIGHT et complété. Valeurs indicatives :
// transmission (t, 0–1), part diffusée (scatter : 0 net, 1 très diffusant), décalage en mireds.
// Les références suivent la numérotation LEE ou Rosco la plus courante ; aucune donnée de
// catalogue recopiée hors numéro, couleur approchée et transmission.

export const FILTER_GROUPS = [
  { id: 'diffusion', label: 'Diffusions' },
  { id: 'ctb', label: 'CTB (vers la lumière du jour)' },
  { id: 'cto', label: 'CTO / CTS (vers le tungstène)' },
  { id: 'green', label: 'Vert / magenta' },
  { id: 'nd', label: 'Neutres (ND)' },
  { id: 'color', label: 'Couleurs' },
];

const D = (id, name, t, scatter, extra = {}) => ({ id, name, group: 'diffusion', color: '#f4f4ef', t, scatter, ...extra });
const G = (id, name, group, color, t, extra = {}) => ({ id, name, group, color, t, scatter: 0, ...extra });

export const FILTERS = [
  // diffusions (du plus léger au plus dense)
  D('lee-452', '452 — 1/16 White Diffusion', 0.9, 0.15),
  D('lee-252', '252 — 1/8 White Diffusion', 0.84, 0.3),
  D('lee-256', '256 — 1/2 Hampshire Frost', 0.85, 0.25),
  D('lee-420', '420 — Light Opal Frost', 0.8, 0.35),
  D('rosco-3009', 'Rosco 3009 — Light Tough Frost', 0.8, 0.4),
  D('lee-434', '434 — Quarter Grid Cloth', 0.8, 0.45),
  D('lee-253', '253 — Hampshire Frost', 0.71, 0.4),
  D('lee-251', '251 — 1/4 White Diffusion', 0.7, 0.5),
  D('rosco-3010', 'Rosco 3010 — Opal Tough Frost', 0.7, 0.55),
  D('lee-410', '410 — Opal Frost', 0.66, 0.55),
  D('rosco-3011', 'Rosco 3011 — Tough Silk', 0.65, 0.6, { silk: true }),
  D('lee-450', '450 — 3/8 White Diffusion', 0.66, 0.6),
  D('lee-432', '432 — Light Grid Cloth', 0.66, 0.7),
  D('lee-250', '250 — 1/2 White Diffusion', 0.62, 0.7),
  D('rosco-3006', 'Rosco 3006 — Tough Spun', 0.58, 0.8),
  D('lee-430', '430 — Grid Cloth', 0.55, 0.85),
  D('lee-416', '416 — 3/4 White Diffusion', 0.5, 0.8),
  D('lee-129', '129 — Heavy Frost', 0.46, 0.95),
  D('lee-216', '216 — Full White Diffusion', 0.41, 0.9),
  D('lee-400', '400 — LeeLux', 0.4, 0.92),
  // CTB : du tungstène vers le jour (mireds négatifs)
  G('lee-218', '218 — 1/8 CTB', 'ctb', '#c7dcef', 0.78, { mired: -18 }),
  G('lee-203', '203 — 1/4 CTB', 'ctb', '#a2c6e6', 0.66, { mired: -35 }),
  G('lee-202', '202 — 1/2 CTB', 'ctb', '#6fa3d6', 0.52, { mired: -68 }),
  G('lee-201', '201 — Full CTB', 'ctb', '#3f7cc4', 0.36, { mired: -131 }),
  // CTO / CTS : du jour vers le tungstène (mireds positifs)
  G('lee-223', '223 — 1/8 CTO', 'cto', '#f7e0c4', 0.81, { mired: 20 }),
  G('lee-206', '206 — 1/4 CTO', 'cto', '#f3cda0', 0.73, { mired: 42 }),
  G('lee-205', '205 — 1/2 CTO', 'cto', '#eeb271', 0.63, { mired: 81 }),
  G('lee-204', '204 — Full CTO', 'cto', '#e8934a', 0.47, { mired: 167 }),
  G('lee-207', '207 — Full CTO + ND 0.3', 'cto', '#a8693a', 0.24, { mired: 167 }),
  G('lee-208', '208 — Full CTO + ND 0.6', 'cto', '#7a4c2b', 0.12, { mired: 167 }),
  G('rosco-3442', 'Rosco 3442 — 1/4 CTS', 'cto', '#f5dca8', 0.74, { mired: 40 }),
  G('rosco-3441', 'Rosco 3441 — 1/2 CTS', 'cto', '#f0c878', 0.65, { mired: 80 }),
  G('rosco-3407', 'Rosco 3407 — Full CTS', 'cto', '#e9b145', 0.5, { mired: 160 }),
  // vert / magenta
  G('lee-246', '246 — 1/4 Plus Green', 'green', '#d9ecb4', 0.86),
  G('lee-245', '245 — 1/2 Plus Green', 'green', '#bfdb7a', 0.78),
  G('lee-244', '244 — Plus Green', 'green', '#9ccb3b', 0.71),
  G('lee-249', '249 — 1/4 Minus Green', 'green', '#ecc8dc', 0.8),
  G('lee-248', '248 — 1/2 Minus Green', 'green', '#db9bc3', 0.71),
  G('lee-247', '247 — Minus Green', 'green', '#c95e9e', 0.6),
  // neutres
  G('lee-298', '298 — ND 0.15 (1/2 diaph)', 'nd', '#d0d0d0', 0.7),
  G('lee-209', '209 — ND 0.3 (1 diaph)', 'nd', '#b3b3b3', 0.5),
  G('lee-210', '210 — ND 0.6 (2 diaph)', 'nd', '#808080', 0.25),
  G('lee-211', '211 — ND 0.9 (3 diaph)', 'nd', '#4d4d4d', 0.125),
  G('lee-299', '299 — ND 1.2 (4 diaph)', 'nd', '#333333', 0.0625),
  // couleurs
  G('lee-103', '103 — Straw (paille)', 'color', '#f0d878', 0.72),
  G('lee-162', '162 — Bastard Amber', 'color', '#f5c08a', 0.7),
  G('lee-101', '101 — Yellow (jaune)', 'color', '#f1c40f', 0.75),
  G('lee-105', '105 — Orange', 'color', '#e8611a', 0.3),
  G('lee-020', '020 — Medium Amber (ambre)', 'color', '#e67e22', 0.4),
  G('lee-156', '156 — Chocolate (chocolat)', 'color', '#8b5a2b', 0.35),
  G('lee-652', '652 — Urban Sodium (sodium)', 'color', '#f2a541', 0.45),
  G('lee-019', '019 — Fire (rouge feu)', 'color', '#d8431c', 0.12),
  G('lee-106', '106 — Primary Red (rouge)', 'color', '#c0392b', 0.12),
  G('lee-111', '111 — Dark Pink (rose)', 'color', '#f48fb1', 0.48),
  G('lee-113', '113 — Magenta', 'color', '#c2185b', 0.14),
  G('lee-136', '136 — Pale Lavender (lavande)', 'color', '#9b8cc4', 0.35),
  G('lee-117', '117 — Steel Blue (bleu acier)', 'color', '#7ea6c8', 0.45),
  G('lee-183', '183 — Moonlight Blue (nuit)', 'color', '#4d7db8', 0.2),
  G('lee-181', '181 — Congo Blue', 'color', '#2a2e8f', 0.03),
  G('lee-079', '079 — Just Blue (bleu)', 'color', '#2e5fa3', 0.08),
  G('lee-116', '116 — Medium Blue-Green (bleu-vert)', 'color', '#1f8f8a', 0.2),
  G('lee-139', '139 — Primary Green (vert)', 'color', '#1e8449', 0.15),
];

const byId = new Map(FILTERS.map((f) => [f.id, f]));
export const getFilter = (id) => byId.get(id) || null;

/** Couleur [r, g, b] (0–1) normalisée : la teinte, sans la perte de lumière (portée par t). */
export function filterTint(f) {
  if (!f || f.group === 'diffusion' || f.group === 'nd') return [1, 1, 1];
  const n = parseInt(f.color.slice(1), 16);
  const rgb = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => (v / 255) ** 2.2); // sRGB → linéaire
  const m = Math.max(...rgb) || 1;
  return rgb.map((v) => v / m);
}

/**
 * Effet d'un filtre sur la lumière qui le traverse : t (part qui passe en ligne droite), diffuse
 * (part réémise en lumière douce), tint (couleur).
 */
export function filterOptics(f) {
  if (!f) return null;
  return { t: f.t * (1 - f.scatter), diffuse: f.t * f.scatter, tint: filterTint(f) };
}
