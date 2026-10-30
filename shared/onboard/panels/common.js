// Outils communs aux panneaux des marques (adresse DMX, profils, courbes, réglages de la lumière).
import { modeFootprint } from '../../fixtures/index.js';
import { num, pad, pick } from '../panel.js';

export const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
export const footprint = (c) => modeFootprint(c.type, c.s.dmxMode) || 1;
export const cctRange = (c, dflt = [2700, 6500]) => c.type.cctRange || dflt;

export const LED = { green: '#3ddc84', blue: '#3d8bff', red: '#ff4d4d', amber: '#ffb238', white: '#f4f6f8', purple: '#b56cff', orange: '#ff8a3d', yellow: '#ffe14d', cyan: '#3de0e8' };

/** Adresse DMX (1 … 513 − nombre de canaux du mode). */
export const addressItem = (label, o = {}) =>
  num(label, { min: 1, max: (c) => 513 - footprint(c), get: (c) => c.s.dmxAddress, set: (c, v) => c.setSettings({ dmxAddress: v }), fmt: (v) => pad(v), ...o });

/** Profil (mode) DMX parmi ceux du modèle. */
export const profileItem = (label, fmt = (m, i) => `${i + 1}:${m.name}`, o = {}) =>
  pick(label, { options: (c) => (c.type.dmx?.modes || []).map((m, i) => [m.id, fmt(m, i)]), get: (c) => c.s.dmxMode, set: (c, v) => c.setSettings({ dmxMode: v }), ...o });

/** Courbe de gradation : libellés de la marque → courbes du moteur (absentes : null). */
export const curveItem = (label, names, o = {}) =>
  pick(label, {
    options: [
      ['square', names.exp],
      ['invSquare', names.log],
      ['sCurve', names.s],
      ['linear', names.lin],
    ].filter((x) => x[1]),
    get: (c) => c.s.dimmingCurve,
    set: (c, v) => c.setSettings({ dimmingCurve: v }),
    ...o,
  });

/** Numéro de série fictif, stable pour un projecteur (lettres et chiffres). */
export function serialOf(c, len = 6, alphabet = '0123456789ABCDEFGHJKLMNPRSTUVWXYZ') {
  let h = 2166136261;
  for (const ch of c.f.id) h = Math.imul(h ^ ch.charCodeAt(0), 16777619) >>> 0;
  let s = '';
  const n = alphabet.length;
  for (let i = 0; i < len; i++) {
    s += alphabet[((h % n) + n) % n]; // le mélange peut rendre h négatif : indice ramené dans l'alphabet
    h = Math.floor(h / n) + Math.imul(h, 2654435761) % 997;
  }
  return s;
}

/** Intensité, température, teinte ±vert, teinte, saturation (réglage manuel, utilisé sans DMX). */
export function nudge(c, key, steps, { step, min, max, wrap = false, round = 1000 }) {
  const m = c.m;
  const cur = Number(key === 'sat' ? m.sat ?? 1 : key === 'intensity' ? m.intensity ?? 0 : m[key] ?? 0);
  let v = cur + steps * step;
  if (wrap) v = ((v - min) % (max - min) + (max - min)) % (max - min) + min;
  v = clamp(Math.round(v * round) / round, min, max);
  c.setManual({ [key]: v });
  return v;
}

export const pct = (v, d = 0) => `${(v * 100).toFixed(d)}%`;
export const tintText = (t, d = 2) => `${t >= 0 ? '+' : ''}${Number(t).toFixed(d)}`;
