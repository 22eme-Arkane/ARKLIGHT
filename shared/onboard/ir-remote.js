// Télécommandes infrarouges Astera : ARC1 (« FX Remote », 28 touches) et ARC3 (« White Remote », 26 touches).
// Règles d'acceptation et fonctions : docs/recherche/menus/box_remotes.md.
import { isAsteraV5 } from '../engine/output.js';

/** Plan des touches (4 colonnes × 7 rangées), libellés tels qu'imprimés. */
export const ARC1_KEYS = [
  ['RAINBOW', 'PASTEL', 'GARDEN', 'POWER ON'],
  ['DISCO', 'FIRE', 'ICE', 'POWER OFF'],
  ['RED', 'WARM WHITE', 'SPEED4', 'FADE ON'],
  ['ORANGE', 'COLD WHITE', 'SPEED3', 'FADE OFF'],
  ['YELLOW', 'PINK', 'SPEED2', 'RUN5'],
  ['GREEN', 'MAGENTA', 'SPEED1', 'RUN8'],
  ['CYAN', 'BLUE', 'STOP', 'RUN12'],
];

export const ARC3_KEYS = [
  ['2400 K', '5000 K', '10%', 'POWER ON'],
  ['2700 K', '5500 K', '20%', 'POWER OFF'],
  ['3000 K', '6000 K', '40%', null],
  ['3200 K', '6500 K', '60%', 'BLUE MODE 1/2'],
  ['3500 K', '10000 K', '100%', 'BLUE MODE 2/2'],
  ['4000 K', 'CCT+', 'DIM+', null],
  ['4500 K', 'CCT-', 'DIM-', 'MAX RUNTIME'],
];

/** Effets de l'ARC1 : interprétation (le rendu exact n'est pas documenté). */
const ARC1_EFFECTS = {
  RAINBOW: { program: 15, colors: [] },
  PASTEL: { program: 7, colors: [[255, 170, 200], [170, 210, 255], [190, 255, 190], [255, 245, 170]] },
  GARDEN: { program: 7, colors: [[40, 200, 40], [180, 255, 60], [255, 220, 40], [0, 160, 90]] },
  DISCO: { program: 13, colors: [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]] },
  FIRE: { program: 16, colors: [] },
  ICE: { program: 6, colors: [[0, 80, 255], [0, 255, 255], [220, 240, 255]] },
};

const ARC1_COLORS = {
  RED: { kind: 'named', name: 'RED' },
  ORANGE: { kind: 'named', name: 'ORANGE' },
  YELLOW: { kind: 'named', name: 'YELLOW' },
  GREEN: { kind: 'named', name: 'GREEN' },
  CYAN: { kind: 'named', name: 'CYAN' },
  BLUE: { kind: 'named', name: 'BLUE' },
  PINK: { kind: 'named', name: 'PINK' },
  MAGENTA: { kind: 'custom', rgb: [255, 0, 255] },
  'WARM WHITE': { kind: 'named', name: '3200K' },
  'COLD WHITE': { kind: 'named', name: '6500K' },
};

const SPEEDS = { SPEED4: 2, SPEED3: 4, SPEED2: 6, SPEED1: 8, STOP: 9 };
const RUNTIMES = { RUN5: 5, RUN8: 8, RUN12: 12 };

/**
 * La lampe accepte-t-elle cette commande infrarouge ?
 * AUTO (sans DMX verrouillé) ou APP CONTROL ; si le DMX est verrouillé en AUTO, seuls ON/OFF passent.
 */
export function irAccepted(fixture, rt, key) {
  const sel = fixture.settings.inputSelect || 'AUTO';
  const onOff = key === 'POWER ON' || key === 'POWER OFF';
  if (sel === 'APP CONTROL') return true;
  if (sel !== 'AUTO') return false;
  if (rt.latch === 'CRMX' || rt.latch === 'WIRED') return onOff;
  return true;
}

/**
 * Applique une touche de télécommande à un projecteur.
 * @returns {{changes?: object, accepted: boolean, note?: string}}
 */
export function applyIrKey(fixture, type, rt, key, now) {
  if (!isAsteraV5(type)) return { accepted: false, note: 'pas de récepteur IR simulé sur ce modèle' };
  if (!irAccepted(fixture, rt, key)) return { accepted: false, note: `refusé (INPUT SELECT = ${fixture.settings.inputSelect}${rt.latch ? `, entrée ${rt.latch} verrouillée` : ''})` };
  const s = fixture.settings;
  const st = fixture.state;
  const sa = s.standalone || {};
  const latchRemote = () => {
    if ((s.inputSelect || 'AUTO') === 'AUTO' && !rt.latch) rt.latch = 'REMOTE';
  };
  const custom = (rgb) => ({ kind: 'custom', rgb });

  if (key === 'POWER ON') {
    if (st.acceptConfig) return { accepted: true, changes: { state: { acceptConfig: false } }, note: 'configuration acceptée' };
    return { accepted: true, changes: { state: { power: true } } };
  }
  if (key === 'POWER OFF') return { accepted: true, changes: { state: { power: false, blueMode: false } } };
  if (!st.power) return { accepted: false, note: 'projecteur éteint' };

  // ---- ARC1
  if (ARC1_EFFECTS[key]) {
    latchRemote();
    const fx = ARC1_EFFECTS[key];
    const colors = fx.colors.length ? fx.colors.map(custom) : sa.colors;
    return { accepted: true, changes: { settings: { standalone: { ...sa, program: fx.program, colors } } } };
  }
  if (ARC1_COLORS[key]) {
    latchRemote();
    return { accepted: true, changes: { settings: { standalone: { ...sa, program: 0, colors: [ARC1_COLORS[key], ...(sa.colors || []).slice(1)] } } } };
  }
  if (key === 'FADE ON' || key === 'FADE OFF') {
    latchRemote();
    return { accepted: true, changes: { settings: { standalone: { ...sa, fade: key === 'FADE ON' ? 100 : 0 } } } };
  }
  if (SPEEDS[key] !== undefined) return { accepted: true, changes: { settings: { standalone: { ...sa, speed: SPEEDS[key] } } } };
  if (RUNTIMES[key]) return { accepted: true, changes: { settings: { runtime: RUNTIMES[key] } } };

  // ---- ARC3
  const cct = key.match(/^(\d+) K$/);
  if (cct) {
    latchRemote();
    return { accepted: true, changes: { settings: { standalone: { ...sa, program: 0, colors: [{ kind: 'hsi', cct: Number(cct[1]), green: 0, hue: 0, sat: 0 }, ...(sa.colors || []).slice(1)] } } } };
  }
  if (key === 'CCT+' || key === 'CCT-') {
    latchRemote();
    const c0 = sa.colors?.[0]?.kind === 'hsi' ? sa.colors[0] : { kind: 'hsi', cct: 5600, green: 0, hue: 0, sat: 0 };
    const next = Math.max(1750, Math.min(20000, c0.cct + (key === 'CCT+' ? 100 : -100)));
    return { accepted: true, changes: { settings: { standalone: { ...sa, program: 0, colors: [{ ...c0, cct: next }, ...(sa.colors || []).slice(1)] } } }, note: 'pas de 100 K (non publié)' };
  }
  const pct = key.match(/^(\d+)%$/);
  if (pct) {
    latchRemote();
    return { accepted: true, changes: { settings: { standalone: { ...sa, intensity: Number(pct[1]) } } } };
  }
  if (key === 'DIM+' || key === 'DIM-') {
    latchRemote();
    const v = Math.max(10, Math.min(100, (sa.intensity ?? 100) + (key === 'DIM+' ? 5 : -5)));
    return { accepted: true, changes: { settings: { standalone: { ...sa, intensity: v } } } };
  }
  if (key === 'BLUE MODE 1/2') {
    rt.irBlue1 = now;
    return { accepted: true, note: 'en attente de BLUE MODE 2/2' };
  }
  if (key === 'BLUE MODE 2/2') {
    if (rt.irBlue1 && now - rt.irBlue1 < 5) {
      rt.irBlue1 = null;
      return { accepted: true, changes: { state: { blueMode: true } } };
    }
    return { accepted: false, note: 'appuyer d’abord sur BLUE MODE 1/2' };
  }
  if (key === 'MAX RUNTIME') return { accepted: true, changes: { settings: { runtimeLimit: s.runtimeLimit === false } }, note: s.runtimeLimit === false ? 'limite d’autonomie réactivée' : 'limite d’autonomie désactivée' };
  return { accepted: false };
}
