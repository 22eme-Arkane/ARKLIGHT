// Liste du matériel (projecteurs, pieds, grip, sacs de sable) et patch DMX, tirés de la scène.
import { getFixtureType, getMode, modeFootprint } from '/shared/fixtures/index.js';
import { SUPPORTS, resolveSupport } from '/shared/fixtures/supports.js';
import { clothName, getGripType, gripStands } from '/shared/grip.js';
import { accessoryEffect } from '/shared/modifiers.js';
import { bodyHalfExtents } from './geometry.js';

// Sacs de sable conseillés par pied (règle de sécurité usuelle, à adapter au plateau)
const SANDBAGS_PER_STAND = { 'stand-1000': 1, 'stand-baby-5k': 1, 'stand-junior': 2, windup: 2, 'super-windup': 2, 'long-john': 2, cstand: 1, lowboy: 1 };

/** Pied d'un projecteur : celui calculé par la vue 3D, sinon recalculé d'après sa hauteur. */
function supportOf(f, type, viewport) {
  const fromView = viewport?.outputOf(f.id)?.support;
  if (fromView) return fromView;
  const half = bodyHalfExtents(type);
  return resolveSupport(type, f.settings.support, f.position[1] - Math.max(...half));
}

/**
 * @returns {{ sections: Array<{ title: string, rows: Array<{ name: string, qty: number, note?: string }> }> }}
 */
export function buildEquipmentList(scene, viewport) {
  const fixtures = new Map();
  const stands = new Map();
  const addStand = (id, qty, label) => {
    const key = label || SUPPORTS[id]?.name || id;
    const cur = stands.get(key) || { name: key, qty: 0, id };
    cur.qty += qty;
    stands.set(key, cur);
  };
  let hanging = 0;
  const accessories = new Map();
  for (const f of scene.fixtures) {
    const type = getFixtureType(f.type);
    if (!type) continue;
    const key = type.id;
    const cur = fixtures.get(key) || { name: type.name.startsWith(`${type.brand} `) ? type.name : `${type.brand} ${type.name}`, qty: 0, watts: type.watts || type.lamp?.watts, modes: new Set() };
    cur.qty++;
    const mode = getMode(type, f.settings.dmxMode);
    if (mode) cur.modes.add(`${mode.name} (${modeFootprint(type, mode.id)} c.)`);
    fixtures.set(key, cur);
    const sup = supportOf(f, type, viewport);
    if (sup.def) addStand(sup.id, 1);
    else if (sup.id === 'hanging') hanging++;
    // accessoires montés : boîte à lumière, dôme, louver, gélatine (feuille)
    const e = accessoryEffect(type, f.settings);
    const addAcc = (name, note = '') => {
      const cur = accessories.get(name) || { name, qty: 0, note };
      cur.qty++;
      accessories.set(name, cur);
    };
    if (e.modifier) addAcc(e.modifier.name, e.modifier.kind === 'softbox' ? 'avec couronne adaptée' : 'monture Bowens');
    if (e.front && e.front !== 'none') addAcc(`Toile avant ${e.front.name}`);
    if (e.grid) addAcc(e.grid.name);
    if (e.gel) addAcc(`Gélatine ${e.gel.name}`, 'feuille');
  }
  const grip = new Map();
  for (const o of scene.objects || []) {
    const type = getGripType(o.type);
    if (!type) continue;
    const cloth = o.props?.cloth && type.cloths ? ` — ${clothName(o.props.cloth)}` : '';
    const key = `${type.id}|${cloth}`;
    const cur = grip.get(key) || { name: `${type.name}${cloth}`, qty: 0 };
    cur.qty++;
    grip.set(key, cur);
    const st = gripStands(type, o.props);
    if (type.family === 'stand') addStand(type.id, 1);
    else if (st) addStand(st.id, st.count, st.label);
  }
  let sandbags = 0;
  for (const s of stands.values()) sandbags += (SANDBAGS_PER_STAND[s.id] ?? 1) * s.qty;

  const byName = (a, b) => a.name.localeCompare(b.name, 'fr');
  return {
    sections: [
      {
        title: 'Projecteurs',
        rows: [...fixtures.values()].sort(byName).map((r) => ({ name: r.name, qty: r.qty, note: [r.watts ? `${r.watts} W` : '', [...r.modes].join(' / ')].filter(Boolean).join(' · ') })),
      },
      {
        title: 'Pieds et supports',
        rows: [
          ...[...stands.values()].sort(byName).map((r) => ({ name: r.name, qty: r.qty })),
          ...(hanging ? [{ name: 'Accroches (pinces, perches) pour projecteurs accrochés', qty: hanging }] : []),
        ],
      },
      { title: 'Accessoires des projecteurs', rows: [...accessories.values()].sort(byName) },
      { title: 'Grip', rows: [...grip.values()].sort(byName) },
      { title: 'Consommables conseillés', rows: sandbags ? [{ name: 'Sacs de sable (1 par pied, 2 par junior, wind-up ou Long John)', qty: sandbags, note: 'conseil' }] : [] },
    ].filter((s) => s.rows.length),
  };
}

/** Patch DMX trié par univers et adresse. */
export function buildPatch(scene) {
  return scene.fixtures
    .map((f) => {
      const type = getFixtureType(f.type);
      const mode = type && getMode(type, f.settings.dmxMode);
      const fp = type ? modeFootprint(type, f.settings.dmxMode) : 0;
      return { name: f.name, model: type ? `${type.brand} ${type.name}` : f.type, universe: f.settings.universe, address: f.settings.dmxAddress, footprint: fp, mode: mode?.name || '—' };
    })
    .filter((r) => r.footprint)
    .sort((a, b) => a.universe - b.universe || a.address - b.address);
}

const csvCell = (v) => {
  const s = String(v ?? '');
  return /[;"\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
};
export const toCsv = (rows) => '﻿' + rows.map((r) => r.map(csvCell).join(';')).join('\r\n'); // BOM + « ; » : s'ouvre tel quel dans Excel (français)

/** tr : traduction des textes (langue de l'interface), sinon le français. */
export function equipmentCsv(list, tr = (s) => s) {
  const rows = [['Rubrique', 'Désignation', 'Quantité', 'Remarques'].map((x) => tr(x))];
  for (const s of list.sections) for (const r of s.rows) rows.push([tr(s.title), tr(r.name), r.qty, r.note ? tr(r.note) : '']);
  return toCsv(rows);
}

export function patchCsv(patch, tr = (s) => s) {
  return toCsv([['Univers', 'Adresse', 'Fin', 'Projecteur', 'Modèle', 'Mode', 'Canaux'].map((x) => tr(x)), ...patch.map((p) => [p.universe, p.address, p.address + p.footprint - 1, p.name, p.model, p.mode, p.footprint])]);
}
