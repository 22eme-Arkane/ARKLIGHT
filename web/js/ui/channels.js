// Canaux DMX à l'écran : libellés courts par fonction et valeurs lisibles (console, inspecteur, moniteur).
import { t } from '../i18n.js';

export const CHANNEL_LABELS = {
  dimmer: 'Dimmer',
  red: 'Rouge',
  green: 'Vert',
  blue: 'Bleu',
  white: 'Blanc',
  amber: 'Ambre',
  mint: 'Menthe',
  lime: 'Lime',
  cyan: 'Cyan',
  uv: 'UV',
  ww: 'Blanc chaud',
  cw: 'Blanc froid',
  magenta: 'Magenta',
  yellow: 'Jaune',
  indigo: 'Indigo',
  cct: 'Kelvin',
  tint: 'Teinte ±V',
  crossfade: 'Fondu couleur',
  hue: 'Teinte',
  sat: 'Satur.',
  strobe: 'Strobe',
  x: 'x',
  y: 'y',
  index: 'Gélatine',
  cctIndex: 'Kelvin index',
  fan: 'Ventilation',
  fxProgram: 'Programme',
  fxSpeed: 'Vitesse',
  fxFade: 'Fondu',
  fxDirection: 'Sens',
  fxSize: 'Taille',
  fxOffset: 'Décalage',
  fxRestart: 'Relance',
  fxIndex: 'Couleur',
  fxR: 'Rouge',
  fxG: 'Vert',
  fxB: 'Bleu',
  effect: 'Effet',
  speed: 'Vitesse',
  none: '—',
};

/** Libellé du profil (nom d'origine du canal), sinon le libellé court. */
export function channelLabel(slot) {
  const base = slot.def.label || CHANNEL_LABELS[slot.f] || slot.f;
  return slot.px >= 0 ? `${base} P${slot.px + 1}` : base;
}

/** Libellé court d'une fonction (Rouge, Kelvin…), dans la langue de l'interface ; sinon celui du profil. */
export const functionLabel = (slot) => (slot.f !== 'none' && CHANNEL_LABELS[slot.f] ? t(CHANNEL_LABELS[slot.f]) : slot.def.label || slot.f);

/** Libellé court par fonction (Rouge, Kelvin…), avec le numéro de couleur d'effet et le pixel. */
export function shortLabel(slot) {
  const known = slot.f !== 'none' && CHANNEL_LABELS[slot.f];
  const base = functionLabel(slot) + (known && slot.def.slot ? ` ${slot.def.slot}` : '');
  return slot.px >= 0 ? `${base} P${slot.px + 1}` : base;
}

/** Valeur lisible d'un canal (unités quand on les connaît : %, K, °). */
export function faderText(slot, raw) {
  const max = slot.bytes === 2 ? 65535 : 255;
  const n = raw / max;
  const def = slot.def || {};
  if (def.map) return String(Math.round(slot.bytes === 2 ? raw / 257 : raw));
  if (slot.f === 'cct') {
    const r = def.range || [2000, 10000];
    if (def.neutral !== undefined && n <= def.neutral + 0.5 / 255) return '—';
    return `${Math.round((r[0] + n * (r[1] - r[0])) / 10) * 10} K`;
  }
  if (slot.f === 'hue') return `${Math.round(n * 360)}°`;
  if (slot.f === 'tint') return def.neutral !== undefined && n <= def.neutral + 0.5 / 255 ? '0' : `${Math.round((n * 2 - 1) * 100)}`;
  if (slot.f === 'none') return String(raw);
  return `${Math.round(n * 100)} %`;
}

/** Valeur DMX brute (0–255, ou le canal gros en 16 bits). */
export const rawText = (slot, raw) => String(slot.bytes === 2 ? Math.round(raw / 257) : raw);
