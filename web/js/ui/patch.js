// Calculs de patch DMX partagés par l'inspecteur et la liste de scène.
import { getFixtureType, getMode, modeFootprint } from '/shared/fixtures/index.js';
import { pad3 } from './dom.js';

export function patchRange(f) {
  const type = getFixtureType(f.type);
  const fp = modeFootprint(type, f.settings.dmxMode);
  return { start: f.settings.dmxAddress, end: f.settings.dmxAddress + fp - 1, footprint: fp };
}

export function patchLabel(f) {
  const { start, end, footprint } = patchRange(f);
  if (!footprint) return `TX U${f.settings.universe}`;
  return `U${f.settings.universe} · ${pad3(start)}${end > start ? `–${pad3(end)}` : ''}`;
}

/** Problèmes de patch : dépassement de l'univers, chevauchements avec d'autres projecteurs. */
export function patchIssues(f, allFixtures) {
  const issues = [];
  const a = patchRange(f);
  if (!a.footprint) return issues;
  if (a.end > 512) issues.push(`dépasse le canal 512 (fin à ${a.end})`);
  for (const o of allFixtures) {
    if (o.id === f.id || o.settings.universe !== f.settings.universe) continue;
    const b = patchRange(o);
    if (!b.footprint) continue;
    if (a.start <= b.end && b.start <= a.end) issues.push(`chevauche « ${o.name} »`);
  }
  return issues;
}

export function modeLabel(type, modeId) {
  const m = getMode(type, modeId);
  return m ? `${m.name} (${modeFootprint(type, m.id)} c.)` : '—';
}
