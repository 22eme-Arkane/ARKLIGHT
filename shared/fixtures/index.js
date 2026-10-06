// Bibliothèque des types de projecteurs (module partagé serveur / navigateur).
import { ASTERA_FIXTURES } from './astera.js';
import { OFL_FIXTURES } from './data/ofl-fixtures.js';
import { TRADITIONAL_FIXTURES } from './traditional.js';
import { footprintOf } from '../engine/dmx.js';

export const FIXTURE_TYPES = [...ASTERA_FIXTURES, ...OFL_FIXTURES, ...TRADITIONAL_FIXTURES];

/** Projecteur non LED (tungstène, HMI) : rangé dans la section « Tungstène & HMI » de la bibliothèque. */
export const isTraditionalType = (type) => type.technology === 'tungsten' || type.technology === 'hmi';

const byId = new Map(FIXTURE_TYPES.map((t) => [t.id, t]));

export function getFixtureType(id) {
  return byId.get(id) || null;
}

export function getMode(type, modeId) {
  if (!type?.dmx?.modes?.length) return null;
  return type.dmx.modes.find((m) => m.id === modeId) || type.dmx.modes[0];
}

export function modeFootprint(type, modeId) {
  const mode = getMode(type, modeId);
  return mode ? footprintOf(mode) : 0;
}

/** Réglages internes d'un projecteur neuf (ce que modifient ses menus, l'appli ou le patch). */
export function defaultSettings(type) {
  return {
    universe: 1, // univers DMX reçu (lien CRMX / entrée filaire simulée)
    dmxAddress: 1,
    dmxMode: type.dmx?.modes?.[0]?.id ?? null,
    whitePoint: 5600,
    dimmingCurve: 'linear',
    support: 'auto', // pied : voir supports.js
    ...(type.defaults?.settings || {}),
  };
}

/** État "physique" simulé (alimentation, batterie, commande manuelle). */
export function defaultState(type) {
  return {
    power: true,
    battery: 100,
    charging: false,
    manual: { mode: 'cct', intensity: 0.6, cct: 5600, tint: 0, hue: 0, sat: 1 },
    ...(type.defaults?.state || {}),
  };
}

/**
 * Type « effectif » d'un projecteur selon ses réglages mécaniques (zoom) :
 * angle de faisceau et éclairement à 1 m interpolés entre les mesures du fabricant.
 */
export function effectiveType(type, fixture) {
  if (!type.zoom) return type;
  const z = Math.min(type.zoom.max, Math.max(type.zoom.min, Number(fixture.settings.zoom ?? type.beamAngle)));
  const pts = type.zoom.luxAt1m;
  let lux = pts[0][1];
  for (let i = 0; i < pts.length - 1; i++) {
    const [a, la] = pts[i];
    const [b, lb] = pts[i + 1];
    if (z >= a && z <= b) {
      // interpolation logarithmique (l'éclairement varie fortement avec l'angle)
      const k = (z - a) / (b - a);
      lux = Math.exp(Math.log(la) + k * (Math.log(lb) - Math.log(la)));
    }
  }
  if (z >= pts[pts.length - 1][0]) lux = pts[pts.length - 1][1];
  return { ...type, beamAngle: z, output: { ...type.output, luxAt1m: Math.round(lux) } };
}

export const CATEGORIES = [
  { id: 'tube', label: 'Tubes' },
  { id: 'bar', label: 'Barres' },
  { id: 'panel', label: 'Panneaux / Briques' },
  { id: 'fresnel', label: 'Fresnels' },
  { id: 'spot', label: 'Spots / COB' },
  { id: 'open-face', label: 'Open face' },
  { id: 'par', label: 'PAR / Spots' },
  { id: 'multi', label: 'Multi-lampes' },
  { id: 'bulb', label: 'Ampoules' },
  { id: 'device', label: 'Boîtiers (AsteraBox…)' },
];
