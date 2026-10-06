// Adressage DMX automatique : détection des conflits de patch et calcul d'un nouveau patch.
// Module partagé (interface, serveur, tests).
import { getFixtureType, modeFootprint } from '../fixtures/index.js';

export const UNIVERSE_SIZE = 512;
const MAX_SPILL = 64; // univers suivants explorés quand un univers est plein

/** Nombre de canaux occupés par un projecteur placé (0 pour un émetteur comme l'AsteraBox). */
export function footprintOf(f) {
  const type = getFixtureType(f.type);
  return type ? modeFootprint(type, f.settings.dmxMode) : 0;
}

export function rangeOf(f) {
  const footprint = footprintOf(f);
  const start = f.settings.dmxAddress;
  return { universe: f.settings.universe, start, end: start + footprint - 1, footprint };
}

/**
 * Conflits de patch : chevauchements entre projecteurs d'un même univers, dépassement du canal 512.
 * @returns {{ overlaps: Array<[string, string]>, overflow: string[], ids: Set<string> }}
 */
export function findPatchConflicts(fixtures) {
  const overlaps = [];
  const overflow = [];
  const byUniverse = new Map();
  for (const f of fixtures) {
    const r = rangeOf(f);
    if (!r.footprint) continue;
    if (r.end > UNIVERSE_SIZE) overflow.push(f.id);
    if (!byUniverse.has(r.universe)) byUniverse.set(r.universe, []);
    byUniverse.get(r.universe).push({ id: f.id, ...r });
  }
  for (const list of byUniverse.values()) {
    list.sort((a, b) => a.start - b.start);
    for (let i = 0; i < list.length; i++) {
      for (let j = i + 1; j < list.length && list[j].start <= list[i].end; j++) overlaps.push([list[i].id, list[j].id]);
    }
  }
  return { overlaps, overflow, ids: new Set([...overflow, ...overlaps.flat()]) };
}

/** Première adresse ≥ a de la forme 1 + k × step (step 0 ou 1 : a elle-même). */
export function alignUp(a, step) {
  if (!step || step <= 1) return a;
  return Math.ceil((a - 1) / step) * step + 1;
}

/** Canaux déjà pris, par univers. */
class Occupancy {
  constructor() {
    this.map = new Map();
  }

  _list(u) {
    if (!this.map.has(u)) this.map.set(u, []);
    return this.map.get(u);
  }

  add(u, start, end) {
    const l = this._list(u);
    l.push([start, end]);
    l.sort((a, b) => a[0] - b[0]);
  }

  fits(u, a, fp) {
    return a >= 1 && a + fp - 1 <= UNIVERSE_SIZE && !this._list(u).some(([s, e]) => a <= e && s <= a + fp - 1);
  }

  /** Première adresse ≥ from (alignée) où fp canaux tiennent sans chevauchement, sinon null. */
  find(u, from, fp, align) {
    const l = this._list(u);
    let a = alignUp(Math.max(1, from), align);
    while (a + fp - 1 <= UNIVERSE_SIZE) {
      const hit = l.find(([s, e]) => a <= e && s <= a + fp - 1);
      if (!hit) return a;
      a = alignUp(hit[1] + 1, align);
    }
    return null;
  }
}

const collator = new Intl.Collator('fr', { numeric: true, sensitivity: 'base' });

export const ADDRESS_ORDERS = {
  scene: 'Ordre de la scène',
  name: 'Par nom',
  position: 'De gauche à droite (vue de face)',
  type: 'Par modèle',
  address: 'Adresse actuelle',
};

function sortFixtures(list, order, all) {
  const idx = new Map(all.map((f, i) => [f.id, i]));
  const scene = (a, b) => idx.get(a.id) - idx.get(b.id);
  const typeName = (f) => getFixtureType(f.type)?.name || f.type;
  const cmp = {
    scene,
    name: (a, b) => collator.compare(a.name, b.name) || scene(a, b),
    position: (a, b) => a.position[0] - b.position[0] || a.position[2] - b.position[2] || scene(a, b),
    type: (a, b) => collator.compare(typeName(a), typeName(b)) || collator.compare(a.name, b.name) || scene(a, b),
    address: (a, b) => a.settings.universe - b.settings.universe || a.settings.dmxAddress - b.settings.dmxAddress || scene(a, b),
  }[order] || scene;
  return [...list].sort(cmp);
}

/**
 * Calcule un nouveau patch sans rien modifier.
 *
 * @param {object[]} fixtures  tous les projecteurs de la scène (dans l'ordre de la scène)
 * @param {object} [opts]
 * @param {'conflicts'|'all'|'ids'} [opts.scope]  conflicts : ne déplace que les projecteurs en conflit ;
 *        all : ré-adresse tout ; ids : ré-adresse opts.ids en contournant les autres projecteurs
 * @param {string[]} [opts.ids]
 * @param {'scene'|'name'|'position'|'type'|'address'} [opts.order]
 * @param {number} [opts.universe]  univers de départ (mode 'pack')
 * @param {number} [opts.address]   adresse de départ
 * @param {'keep'|'pack'} [opts.universes]  keep : chaque projecteur reste dans son univers ;
 *        pack : tout s'enchaîne à partir de l'univers de départ
 * @param {number} [opts.align]     adresses de départ alignées sur 1, 1+n, 1+2n… (0 : à la suite)
 * @param {boolean} [opts.overflow] passer à l'univers suivant quand un univers est plein
 */
export function planAddressing(fixtures, opts = {}) {
  const { scope = 'conflicts', order = 'scene', align = 0, overflow = true, universes = 'keep' } = opts;
  const startU = Math.max(1, Math.round(Number(opts.universe) || 1));
  const startA = Math.min(UNIVERSE_SIZE, Math.max(1, Math.round(Number(opts.address) || 1)));
  const patched = fixtures.filter((f) => footprintOf(f) > 0);
  const before = findPatchConflicts(patched);

  let movable;
  if (scope === 'conflicts') movable = new Set(before.ids);
  else if (scope === 'ids') movable = new Set(opts.ids || []);
  else movable = new Set(patched.map((f) => f.id));

  const occ = new Occupancy();
  for (const f of patched) {
    if (movable.has(f.id)) continue;
    const r = rangeOf(f);
    occ.add(r.universe, r.start, r.end);
  }

  const target = new Map(); // id -> { universe, address }
  const unplaced = [];
  const list = patched.filter((f) => movable.has(f.id));

  /** Place fp canaux à partir de (u, from), en débordant sur les univers suivants si permis. */
  const placeFrom = (u, from, fp, wrap) => {
    let a = occ.find(u, from, fp, align);
    if (a === null && wrap && from > 1) a = occ.find(u, 1, fp, align); // trou plus haut dans l'univers
    if (a !== null) return { universe: u, address: a };
    if (!overflow) return null;
    for (let v = u + 1; v <= u + MAX_SPILL; v++) {
      const b = occ.find(v, 1, fp, align);
      if (b !== null) return { universe: v, address: b };
    }
    return null;
  };
  const commit = (f, pos, fp) => {
    if (!pos) {
      unplaced.push(f.id);
      return null;
    }
    occ.add(pos.universe, pos.address, pos.address + fp - 1);
    target.set(f.id, pos);
    return pos;
  };

  if (scope === 'conflicts') {
    // Le premier projecteur (adresse la plus basse) garde sa place ; les suivants vont à la première place libre.
    for (const f of sortFixtures(list, 'address', fixtures)) {
      const r = rangeOf(f);
      const pos = occ.fits(r.universe, r.start, r.footprint) ? { universe: r.universe, address: r.start } : placeFrom(r.universe, r.start, r.footprint, true);
      commit(f, pos, r.footprint);
    }
  } else {
    const sequence = (members, cursor) => {
      for (const f of members) {
        const fp = footprintOf(f);
        const pos = commit(f, placeFrom(cursor.universe, cursor.address, fp, false), fp);
        if (pos) cursor = { universe: pos.universe, address: pos.address + fp };
      }
    };
    const sorted = sortFixtures(list, order, fixtures);
    if (universes === 'pack') sequence(sorted, { universe: startU, address: startA });
    else {
      const byU = new Map();
      for (const f of sorted) {
        const u = f.settings.universe;
        if (!byU.has(u)) byU.set(u, []);
        byU.get(u).push(f);
      }
      for (const u of [...byU.keys()].sort((a, b) => a - b)) sequence(byU.get(u), { universe: u, address: startA });
    }
  }

  const moves = [];
  for (const f of list) {
    const to = target.get(f.id);
    if (!to || (to.universe === f.settings.universe && to.address === f.settings.dmxAddress)) continue;
    moves.push({ id: f.id, name: f.name, footprint: footprintOf(f), from: { universe: f.settings.universe, address: f.settings.dmxAddress }, to });
  }
  const after = findPatchConflicts(
    patched.map((f) => {
      const to = target.get(f.id);
      return to ? { ...f, settings: { ...f.settings, universe: to.universe, dmxAddress: to.address } } : f;
    }),
  );
  return { moves, unplaced, considered: list.length, conflictsBefore: before.ids.size, conflictsAfter: after.ids.size };
}
