// Accessoires de projecteur : boîtes à lumière (Chimera, Snapbag), dômes et lanternes (monture
// Bowens : Aputure, Amaran, Nanlite…), louvers / grilles, porte-filtre (gélatine ou diffusion).
// Effet calculé : angle d'ouverture, éclairement dans l'axe, couleur. Valeurs approchées :
// t = part de lumière restituée par l'accessoire avec sa toile d'origine.
import { filterTint, getFilter } from './filters.js';

export const MODIFIERS = [
  // dômes et lanternes (monture Bowens)
  { id: 'dome-mini', kind: 'dome', group: 'Dômes (monture Bowens)', name: 'Light Dome Mini (55 cm)', short: 'Dome Mini', d: 0.55, depth: 0.36, angle: 95, t: 0.5 },
  { id: 'dome-90', kind: 'dome', group: 'Dômes (monture Bowens)', name: 'Light Dome (90 cm)', short: 'Dome 90', d: 0.89, depth: 0.6, angle: 100, t: 0.5 },
  { id: 'dome-150', kind: 'dome', group: 'Dômes (monture Bowens)', name: 'Light Dome 150 (150 cm)', short: 'Dome 150', d: 1.5, depth: 1.0, angle: 105, t: 0.45 },
  { id: 'lantern-66', kind: 'lantern', group: 'Lanternes (monture Bowens)', name: 'Lantern 66 cm', short: 'Lantern 66', d: 0.66, angle: 170, t: 0.42 },
  { id: 'lantern-90', kind: 'lantern', group: 'Lanternes (monture Bowens)', name: 'Lantern 90 cm', short: 'Lantern 90', d: 0.9, angle: 170, t: 0.4 },
  // boîtes à lumière rectangulaires (couronne adaptée au projecteur)
  { id: 'chimera-xs', kind: 'softbox', group: 'Boîtes à lumière (Chimera)', name: 'Chimera XS (40×53 cm)', short: 'Chimera XS', w: 0.4, h: 0.53, depth: 0.34, angle: 92, t: 0.55 },
  { id: 'chimera-s', kind: 'softbox', group: 'Boîtes à lumière (Chimera)', name: 'Chimera S (61×81 cm)', short: 'Chimera S', w: 0.61, h: 0.81, depth: 0.45, angle: 95, t: 0.52 },
  { id: 'chimera-m', kind: 'softbox', group: 'Boîtes à lumière (Chimera)', name: 'Chimera M (91×122 cm)', short: 'Chimera M', w: 0.91, h: 1.22, depth: 0.66, angle: 98, t: 0.5 },
  { id: 'chimera-l', kind: 'softbox', group: 'Boîtes à lumière (Chimera)', name: 'Chimera L (137×183 cm)', short: 'Chimera L', w: 1.37, h: 1.83, depth: 0.92, angle: 102, t: 0.48 },
  { id: 'chimera-panel', kind: 'softbox', group: 'Boîtes à lumière (Chimera)', name: 'Chimera / Snapbag à la taille du panneau', short: 'Chimera panneau', fit: true, depth: 0.3, angle: 92, t: 0.6, panelOnly: true },
];

export const GRIDS = [
  { id: 'grid-60', name: 'Louver / grille 60°', short: 'Grille 60°', angle: 60, t: 0.82, depth: 0.04 },
  { id: 'grid-50', name: 'Louver / grille 50°', short: 'Grille 50°', angle: 50, t: 0.78, depth: 0.05 },
  { id: 'grid-40', name: 'Louver / grille 40°', short: 'Grille 40°', angle: 40, t: 0.72, depth: 0.065 },
  { id: 'grid-30', name: 'Louver / grille 30°', short: 'Grille 30°', angle: 30, t: 0.65, depth: 0.085 },
  { id: 'grid-20', name: 'Louver / grille 20°', short: 'Grille 20°', angle: 20, t: 0.55, depth: 0.12 },
];

/** Toile avant d'une boîte à lumière ou d'un dôme : d'origine, sans toile, ou une diffusion du nuancier. */
export const FRONT_DEFAULT_T = 0.55;

const modById = new Map(MODIFIERS.map((m) => [m.id, m]));
const gridById = new Map(GRIDS.map((g) => [g.id, g]));
export const getModifier = (id) => modById.get(id) || null;
export const getGrid = (id) => gridById.get(id) || null;

/** Accessoires possibles pour un type de projecteur. */
export function accessoriesFor(type) {
  const kind = type.geometry?.kind;
  const none = { modifiers: [], grids: false, gel: false };
  if (!type.pixels || ['tube', 'bulb', 'box-device'].includes(kind) || ['device', 'bulb', 'tube'].includes(type.category)) return none;
  if (kind === 'bar' || type.category === 'bar') return { modifiers: [], grids: false, gel: true };
  const panel = ['panel', 'brick'].includes(kind) || type.category === 'panel';
  const bowens = /bowens/i.test(type.mount || '') || (type.category === 'spot' && type.technology !== 'tungsten' && type.technology !== 'hmi');
  let modifiers;
  if (panel) modifiers = MODIFIERS.filter((m) => m.kind === 'softbox' && (m.panelOnly || ['chimera-s', 'chimera-m'].includes(m.id)));
  else if (bowens) modifiers = MODIFIERS.filter((m) => !m.panelOnly);
  else modifiers = MODIFIERS.filter((m) => m.kind === 'softbox' && !m.panelOnly); // fresnels, PAR, open face (couronne)
  return { modifiers, grids: true, gel: true };
}

const tanHalf = (deg) => Math.tan((Math.min(170, deg) * Math.PI) / 360);

/**
 * Effet des accessoires réglés sur un projecteur (settings : modifier, front, grid, gel).
 * Renvoie l'angle d'ouverture, le facteur d'éclairement dans l'axe, la teinte et les accessoires.
 */
export function accessoryEffect(type, settings = {}, baseAngle = type.beamAngle || 40) {
  const allowed = accessoriesFor(type);
  const mod = allowed.modifiers.find((m) => m.id === settings.modifier) || null;
  const grid = allowed.grids ? getGrid(settings.grid) : null;
  const gel = allowed.gel ? getFilter(settings.gel) : null;
  const front = mod && mod.kind !== 'lantern' && settings.front ? (settings.front === 'none' ? 'none' : getFilter(settings.front)) : null;
  let angle = baseAngle;
  let lux = 1;
  let transmission = 1; // part de lumière restituée (sans l'effet de l'ouverture du faisceau)
  if (mod) {
    angle = mod.angle;
    transmission *= mod.t;
    // lanterne : lumière dans toutes les directions (environ 4 diaph de moins dans l'axe)
    lux *= mod.t * (mod.kind === 'lantern' ? 0.06 : Math.min(1, (tanHalf(baseAngle) / tanHalf(mod.angle)) ** 2));
    const k = front === 'none' ? 1 / FRONT_DEFAULT_T : front ? front.t / FRONT_DEFAULT_T : 1;
    lux *= k;
    transmission *= k;
  }
  if (gel) {
    lux *= gel.t;
    transmission *= gel.t;
    if (gel.scatter > 0 && !mod) {
      // diffusion devant un projecteur nu : le faisceau s'ouvre
      const wider = Math.min(120, Math.max(angle, angle + gel.scatter * 60));
      lux *= Math.min(1, (tanHalf(angle) / tanHalf(wider)) ** 2);
      angle = wider;
    }
  }
  if (grid && grid.angle < angle) {
    angle = grid.angle;
    lux *= grid.t;
    transmission *= grid.t;
  } else if (grid) {
    lux *= Math.min(1, grid.t + 0.15);
    transmission *= Math.min(1, grid.t + 0.15);
  }
  return { angle, lux, transmission, tint: gel ? filterTint(gel) : null, modifier: mod, grid, gel, front };
}

/** Accessoires montés, en clair (« Chimera M + 216 · Grille 40° · CTO ») ; '' s'il n'y en a pas. */
export function accessoryLabel(type, settings = {}, tr = (s) => s) {
  const e = accessoryEffect(type, settings);
  const parts = [];
  if (e.modifier) parts.push(`${tr(e.modifier.short)}${e.front ? ` + ${e.front === 'none' ? tr('sans toile') : e.front.name.split(' — ')[0]}` : ''}`);
  if (e.grid) parts.push(tr(e.grid.short));
  if (e.gel) parts.push(e.gel.name.split(' — ')[1] ? `${e.gel.name.split(' — ')[0]} ${e.gel.name.split(' — ')[1].replace(/\s*\(.*\)/, '')}` : e.gel.name);
  return parts.join(' · ');
}
