// Couleurs prédéfinies des projecteurs Astera (menu STATIC COLOR, BlueMode de l'AX1, IR).
// Valeurs « AsteraRGB » R/G/B 0..255 (Titan TRM 2020 §7.13.1, AX1 manuel 2018 §7.8.1).
import { buildIndexColors, gelColor } from '../engine/gels.js';
import { srgbToLinear } from '../engine/color.js';
import INDEX_LIST from './data/astera-index-colors.js';

export const STATIC_COLORS = [
  ['RED', [255, 0, 0]],
  ['ORANGE', [255, 107, 0]],
  ['YELLOW', [255, 160, 18]],
  ['GREEN', [0, 255, 0]],
  ['CYAN', [0, 255, 224]],
  ['BLUE', [0, 0, 255]],
  ['VIOLET', [127, 84, 255]],
  ['PINK', [255, 53, 119]],
  ['BLACK', [0, 0, 0]],
  ['2700K', [255, 166, 70]],
  ['3200K', [255, 178, 89]],
  ['4000K', [255, 193, 115]],
  ['5500K', [255, 211, 150]],
  ['6500K', [255, 219, 167]],
];

/** Les 13 couleurs du BlueMode de l'AX1 (sans BLACK). */
export const AX1_BLUEMODE_COLORS = STATIC_COLORS.filter(([n]) => n !== 'BLACK');

export const asteraRgbToLinear = (rgb) => rgb.map((v) => srgbToLinear(v / 255));

export function staticColorRgb(name) {
  const hit = STATIC_COLORS.find(([n]) => n === name);
  return hit ? asteraRgbToLinear(hit[1]) : [1, 0, 0];
}

const INDEX = buildIndexColors(INDEX_LIST);

/** Gels disponibles sur la page « Color Gel » : liste LEE des couleurs index Astera. */
export const LEE_GELS = INDEX_LIST.filter(([, name]) => !/no effect/i.test(name)).map(([num, name]) => ({ number: Number(String(num).split('..')[0]), name }));

export const GEL_FAMILIES = ['Lee Daylight', 'Lee Tungsten'];

/**
 * Couleur d'une « spécification de couleur » du mode autonome :
 *   { kind: 'named', name } | { kind: 'custom', rgb: [0..255 x3] } | { kind: 'index', value }
 *   { kind: 'hsi', cct, green, hue, sat } | { kind: 'gel', family, number }
 * @param {(k:number, tint:number)=>number[]} cct  conversion Kelvin -> RGB
 * @param {(h:number, s:number, white:number[])=>number[]} hs
 */
export function colorSpecRgb(spec, { cct, hs }) {
  if (!spec) return [0, 0, 0];
  switch (spec.kind) {
    case 'named':
      return staticColorRgb(spec.name);
    case 'custom':
      return asteraRgbToLinear(spec.rgb || [255, 0, 0]);
    case 'index':
      return INDEX.color(spec.value) || [1, 1, 1];
    case 'hsi': {
      const white = cct(spec.cct ?? 5600, spec.green ?? 0);
      return hs(spec.hue ?? 0, (spec.sat ?? 0) / 100, white);
    }
    case 'gel': {
      const gel = LEE_GELS.find((g) => g.number === spec.number);
      const base = cct(spec.family === 'Lee Tungsten' ? 3200 : 5600, 0);
      const filt = gel ? gelColor(gel.name) : [1, 1, 1];
      const c = [base[0] * filt[0], base[1] * filt[1], base[2] * filt[2]];
      const m = Math.max(...c) || 1;
      return c.map((v) => v / m);
    }
    default:
      return [1, 1, 1];
  }
}

export function colorSpecLabel(spec) {
  if (!spec) return '—';
  switch (spec.kind) {
    case 'named':
      return spec.name;
    case 'custom':
      return 'CUSTOM COLOR';
    case 'index':
      return `INDEX ${spec.value}`;
    case 'hsi':
      return 'HSI';
    case 'gel':
      return `GEL ${String(spec.number).padStart(3, '0')}`;
    default:
      return '—';
  }
}
