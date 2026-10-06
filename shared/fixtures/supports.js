// Supports des projecteurs (pieds) : modèles 3D, liste du matériel, avertissements de charge et de hauteur.
// Données : docs/recherche/marques/notes-partielles.md. « vérifié » = lu sur une fiche ; sinon valeur typique à confirmer.

export const SUPPORTS = {
  'stand-1000': {
    name: 'Pied de 1000 (baby)',
    short: 'Pied 1000',
    spigot: '16 mm',
    minH: 1.16,
    maxH: 3.76,
    load: 9,
    spread: 0.85,
    weight: 6.1,
    look: 'baby',
    ref: 'Matthews « 1000 », Avenger A0035 / A0040',
    confidence: 'vérifié',
  },
  'stand-baby-5k': {
    name: 'Pied Baby 5K (combo)',
    short: 'Baby 5K',
    spigot: '16 mm',
    minH: 1.0,
    maxH: 2.0,
    load: 35,
    spread: 1.1,
    look: 'combo',
    ref: 'Avenger A1020CS',
    confidence: 'vérifié (charge, hauteur maxi)',
  },
  'stand-junior': {
    name: 'Pied junior (réception 28 mm)',
    short: 'Pied junior',
    spigot: '28 mm',
    minH: 1.2,
    maxH: 3.4,
    load: 20,
    spread: 1.2,
    look: 'combo',
    ref: 'Avenger A5034 (Roller 34)',
    confidence: 'à vérifier',
  },
  windup: {
    name: 'Wind-up (pied à manivelle)',
    short: 'Wind-up',
    spigot: '28 mm',
    minH: 1.7,
    maxH: 4.5,
    load: 60,
    spread: 1.6,
    look: 'windup',
    ref: 'Manfrotto 087NW (valeurs typiques)',
    confidence: 'estimé',
  },
  cstand: {
    name: 'C-stand (bras et rotule)',
    short: 'C-stand',
    spigot: '16 mm',
    minH: 1.0,
    maxH: 3.2,
    load: 10,
    spread: 0.75,
    look: 'cstand',
    ref: 'valeurs typiques',
    confidence: 'estimé',
  },
  lowboy: {
    name: 'Low-boy (pied bas)',
    short: 'Low-boy',
    spigot: '28 mm',
    minH: 0.6,
    maxH: 1.4,
    load: 30,
    spread: 1.0,
    look: 'lowboy',
    ref: 'valeurs typiques',
    confidence: 'estimé',
  },
};

/** Choix de l'utilisateur dans l'inspecteur (en plus des pieds). */
export const SUPPORT_CHOICES = [
  ['auto', 'Automatique (selon poids, hauteur, montage)'],
  ...Object.entries(SUPPORTS).map(([id, s]) => [id, s.name]),
  ['hanging', 'Accroché (perche, pince, grill)'],
  ['none', 'Aucun (posé au sol ou tenu à la main)'],
];

const HEAVY_MOUNT = /28|junior/i;

/**
 * Support retenu pour un projecteur.
 * @param {object} type     type du projecteur (weight, mount)
 * @param {string} choice   settings.support ('auto' par défaut)
 * @param {number} top      hauteur du haut du pied (bas du projecteur), en mètres
 * @returns {{ id: string, def: object|null, auto: boolean, warnings: string[] }}
 */
export function resolveSupport(type, choice = 'auto', top = 0) {
  const w = type.weight || 0;
  let id = choice || 'auto';
  const auto = id === 'auto';
  if (auto) {
    // posé au sol (ou presque), ou boîtier comme l'AsteraBox : pas de pied
    if (top < 0.6 || type.category === 'device') id = 'none';
    else if (top < 1.0) id = 'lowboy';
    else if (w > 20 || top > 3.76) id = 'windup';
    else if (HEAVY_MOUNT.test(type.mount || '') || w > 9) id = top <= 2 && w <= 35 && !HEAVY_MOUNT.test(type.mount || '') ? 'stand-baby-5k' : 'stand-junior';
    else id = 'stand-1000';
  }
  const def = SUPPORTS[id] || null;
  const warnings = [];
  if (def) {
    if (w > def.load) warnings.push(`${w} kg : plus que la charge du ${def.short} (${def.load} kg)`);
    if (top > def.maxH + 0.01) warnings.push(`hauteur ${top.toFixed(2)} m : au-dessus du maximum du ${def.short} (${def.maxH} m)`);
    else if (top < def.minH - 0.01) warnings.push(`hauteur ${top.toFixed(2)} m : en dessous du minimum du ${def.short} (${def.minH} m)`);
  }
  return { id, def, auto, warnings };
}
