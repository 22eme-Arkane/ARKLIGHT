// Couleurs approximatives des gélatines LEE à partir de leur nom.
// Les « couleurs index » Astera donnent seulement le numéro / nom du gel (valeur DMX = numéro LEE) :
// on en déduit une teinte plausible pour l'affichage (approximation visuelle, pas une mesure).
import { cctToRgb, hsToRgb } from './color.js';

const HUES = [
  ['primary red', 0],
  ['scarlet', 355],
  ['crimson', 350],
  ['red', 0],
  ['fire', 12],
  ['sunset', 14],
  ['flame', 18],
  ['salmon', 10],
  ['peach', 20],
  ['apricot', 24],
  ['chocolate', 24],
  ['bronze', 30],
  ['orange', 28],
  ['amber', 38],
  ['gold', 45],
  ['straw', 48],
  ['yellow', 56],
  ['lime', 88],
  ['moss', 95],
  ['fern', 130],
  ['jade', 150],
  ['green', 120],
  ['turquoise', 178],
  ['teal', 180],
  ['peacock', 185],
  ['cyan', 188],
  ['aqua', 190],
  ['sky', 205],
  ['steel', 212],
  ['tokyo', 230],
  ['navy', 232],
  ['blue', 225],
  ['lavender', 262],
  ['violet', 270],
  ['purple', 280],
  ['mauve', 300],
  ['magenta', 305],
  ['pink', 330],
  ['rose', 340],
];

/** Couleur (sRGB linéaire, max = 1) d'un gel d'après son nom. */
export function gelColor(name) {
  const n = String(name || '').toLowerCase();
  if (/frost|diffus|clear|opal|silk|grid|cloth|hampshire|soft/.test(n)) return [1, 1, 1];
  const ct = n.match(/(full|half|quarter|1\/2|1\/4|1\/8|eighth|double)?\s*c\.?\s*t\.?\s*(blue|orange|straw|eight)/);
  if (ct) {
    const k = { double: 2, full: 1, half: 0.5, '1/2': 0.5, quarter: 0.25, '1/4': 0.25, eighth: 0.125, '1/8': 0.125 }[ct[1] || 'full'] ?? 1;
    if (ct[2] === 'blue') return cctToRgb(3200 + 2400 * k);
    return cctToRgb(5600 - 2400 * k, ct[2] === 'straw' ? 0.15 : 0);
  }
  if (/plus green/.test(n)) return cctToRgb(5600, /half|1\/2/.test(n) ? 0.5 : /quarter|1\/4/.test(n) ? 0.25 : 1);
  if (/minus green/.test(n)) return cctToRgb(5600, /half|1\/2/.test(n) ? -0.5 : /quarter|1\/4/.test(n) ? -0.25 : -1);
  if (/neutral density|\bnd\b|grey|gray/.test(n)) return [0.5, 0.5, 0.5];
  const hit = HUES.find(([w]) => n.includes(w));
  if (!hit) return [1, 0.92, 0.85];
  let sat = 0.8;
  let val = 1;
  if (/pale|light|tint|paler|lighter|surprise|bastard/.test(n)) sat = 0.38;
  else if (/medium|middle|mist|soft/.test(n)) sat = 0.62;
  if (/dark|deep|deeper|primary|just|congo|bright|vivid|rich/.test(n)) {
    sat = 1;
    val = /dark|deep/.test(n) ? 0.75 : 1;
  }
  return hsToRgb(hit[1], sat).map((v) => v * val);
}

/**
 * Table d'index : [["0..1","No effect"], ["2","Rose Pink"], ...] -> fonction valeur DMX -> couleur.
 * Une valeur absente de la liste reprend l'entrée précédente.
 */
export function buildIndexColors(list) {
  const entries = list
    .map(([key, name]) => {
      const [a] = String(key).split('..').map(Number);
      return { start: a, name, rgb: /no effect/i.test(name) ? null : gelColor(name) };
    })
    .sort((x, y) => x.start - y.start);
  const table = new Array(256).fill(null);
  let cur = null;
  for (let v = 0, i = 0; v < 256; v++) {
    while (i < entries.length && entries[i].start <= v) cur = entries[i++];
    table[v] = cur;
  }
  return {
    color: (v) => table[Math.max(0, Math.min(255, Math.round(v)))]?.rgb || null,
    name: (v) => table[Math.max(0, Math.min(255, Math.round(v)))]?.name || '',
  };
}
