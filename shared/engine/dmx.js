// Décodage DMX générique piloté par les définitions de modes des projecteurs.
//
// Un mode décrit ses canaux dans l'ordre :
//   'red'                                -> raccourci pour { f: 'red' }
//   { f: 'dimmer', bytes: 2 }            -> 16 bits (canal gros + canal fin)
//   { f: 'cct', range: [1750, 20000] }   -> plage linéaire de la fonction
//   { f: 'cct', map: 'asteraCct' }       -> conversion spécifique (voir MAPS)
//   { f: 'red', px: 3 }                  -> canal d'un pixel précis (modes déjà « dépliés »)
//   { pixels: 16, channels: [...] }      -> bloc répété pour chaque pixel
//   { f: 'none' }                        -> canal réservé / sans effet

import { EMITTERS, cctToRgb, hsToRgb, xyToRgb } from './color.js';
import { directionFromDmx, programFromDmx, renderProgram } from './effects.js';

export const EMITTER_FUNCTIONS = ['red', 'green', 'blue', 'white', 'amber', 'mint', 'lime', 'cyan', 'uv', 'ww', 'cw', 'magenta', 'yellow', 'indigo'];

export const DEFAULT_RANGES = {
  cct: [2000, 10000],
  hue: [0, 360],
  tint: [-1, 1],
};

/**
 * Conversions non linéaires. Elles reçoivent la valeur brute (0..255, ou 0..65535 en 16 bits)
 * et renvoient la valeur de l'attribut, ou undefined pour « sans effet ».
 */
export const MAPS = {
  // Astera — tables des documents « DMX Profiles »
  asteraStrobe: (v) => (v <= 3 ? 0 : v === 4 ? -1 : v === 5 ? -2 : v === 6 ? -3 : 0.4 + ((v - 7) / 248) * 24.6),
  asteraCct: (v) => 1750 + 32 * v,
  asteraCctInd: (v) => (v <= 4 ? undefined : 2000 + 20 * v),
  asteraGM: (v) => (v <= 4 ? 0 : v / 128 - 1),
  asteraIdx: (v) => (v <= 1 ? undefined : v),
  // canal « Fan Control » des profils FAN (QuikPunch, QuikBeam, Pluto, Leo)
  asteraFan: (v) => (v <= 4 ? 'AUTO' : v <= 35 ? 'LOW' : v <= 70 ? 'MED' : v <= 100 ? 'HIGH' : v <= 128 ? 'OFF' : undefined),
  raw: (v) => v,
};

const cache = new WeakMap();

/** Aplatit un mode : liste de "slots" { f, px, offset, bytes, def, key }. */
export function compileMode(mode) {
  let compiled = cache.get(mode);
  if (compiled) return compiled;
  const slots = [];
  let offset = 0;
  const push = (ch, px) => {
    const def = typeof ch === 'string' ? { f: ch } : ch;
    const bytes = def.bytes || 1;
    const p = def.px !== undefined ? def.px : px;
    slots.push({ f: def.f, px: p, offset, bytes, def, key: def.slot ? `${def.f}${def.slot}` : def.f });
    offset += bytes;
  };
  for (const ch of mode.channels) {
    if (ch && typeof ch === 'object' && ch.pixels) {
      const first = ch.firstPixel || 0;
      for (let p = 0; p < ch.pixels; p++) for (const c of ch.channels) push(c, first + p);
    } else {
      push(ch, -1);
    }
  }
  compiled = { slots, footprint: offset };
  cache.set(mode, compiled);
  return compiled;
}

export function footprintOf(mode) {
  return compileMode(mode).footprint;
}

function slotValue(data, start, slot) {
  const i = start + slot.offset;
  const raw = slot.bytes === 2 ? (data[i] || 0) * 256 + (data[i + 1] || 0) : data[i] || 0;
  const def = slot.def;
  if (def.map) {
    // Les tables Astera sont exprimées en 8 bits
    return MAPS[def.map](slot.bytes === 2 ? raw / 257 : raw);
  }
  const n = slot.bytes === 2 ? raw / 65535 : raw / 255;
  if (slot.f === 'strobe') return decodeStrobe(slot, n);
  const range = def.range || DEFAULT_RANGES[slot.f];
  if (def.neutral !== undefined) {
    // zone neutre en bas de course (ex. ARRI : 0–10 = pas de correction), puis toute la plage au-dessus
    // seuil arrondi dans la bibliothèque (10/255 noté 0,0392) : un demi-pas de tolérance
    if (n <= def.neutral + 0.5 / 255) return def.neutralValue ?? 0;
    const k = (n - def.neutral) / (1 - def.neutral);
    const r = range || [0, 1];
    return r[0] + k * (r[1] - r[0]);
  }
  return range ? range[0] + n * (range[1] - range[0]) : n;
}

/**
 * Stroboscope générique : renvoie la fréquence en Hz (0 = ouvert).
 * Par défaut : 0–9 ouvert, 10–255 de 1 à 25 Hz.
 */
export function decodeStrobe(slot, n) {
  const s = slot.def.strobe || { open: 9, min: 1, max: 25 };
  const v = Math.round(n * 255);
  if (v <= s.open) return 0;
  const k = (v - s.open - 1) / (255 - s.open - 1);
  return s.min + k * (s.max - s.min);
}

const RANDOM_RATES = { '-1': 9, '-2': 4, '-3': 1.5 };

/**
 * Ouverture du stroboscope à l'instant t. hz > 0 : fréquence ; hz < 0 : aléatoire
 * (-1 rapide, -2 moyen, -3 lent) ; 0 : ouvert.
 */
export function strobeGate(hz, timeSec, duty = 0.2, seed = 0) {
  if (!hz) return 1;
  if (hz < 0) {
    const rate = RANDOM_RATES[hz] || 4;
    const slot = Math.floor(timeSec * rate * 2);
    const r = Math.sin((slot + seed * 17.3) * 12.9898) * 43758.5453;
    if (r - Math.floor(r) < 0.5) return 0;
    return (timeSec * rate * 2) % 1 < 0.35 ? 1 : 0;
  }
  return (timeSec * hz) % 1 < duty ? 1 : 0;
}

/** Courbes de gradation (appliquées aux canaux d'intensité). */
export const DIMMING_CURVES = {
  linear: (n) => n,
  square: (n) => n * n,
  invSquare: (n) => 1 - (1 - n) * (1 - n),
  sCurve: (n) => n * n * (3 - 2 * n),
  // Courbes du menu Astera DIMMER CURVE (formes approchées)
  FAST: (n) => 1 - (1 - n) * (1 - n),
  STANDARD: (n) => n,
  HALOGEN: (n) => n * n,
  THEATER: (n) => n * n * (3 - 2 * n),
  TV: (n) => Math.pow(n, 1.5),
};

/**
 * Décode les canaux DMX d'un projecteur.
 * @returns {{fixture: object, pixels: object[]}} attributs bruts par pixel
 */
export function readAttributes(mode, data, address, pixelCount) {
  const { slots } = compileMode(mode);
  const start = address - 1;
  const fixture = {};
  const pixels = Array.from({ length: pixelCount }, () => ({}));
  for (const slot of slots) {
    if (!slot.f || slot.f === 'none') continue;
    if (start + slot.offset >= 512) break;
    const value = slotValue(data, start, slot);
    if (value === undefined) continue;
    if (slot.px < 0) fixture[slot.key] = value;
    else if (slot.px < pixelCount) pixels[slot.px][slot.key] = value;
  }
  return { fixture, pixels };
}

/**
 * Couleur d'un pixel à partir de ses attributs (sRGB linéaire, avant gradateur).
 * Règles Astera : couleur index > Kelvin « IND » > RGB ; crossfade entre Kelvin et RGB.
 */
export function attributesToRgb(attrs, opts) {
  const whiteRgb = opts.whiteRgb;
  if (attrs.index !== undefined && opts.indexColor) {
    const c = opts.indexColor(attrs.index);
    if (c) return [...c];
  }
  if (attrs.cctIndex !== undefined) return cctToRgb(attrs.cctIndex, attrs.tint || 0);
  let rgb = null;
  for (const f of EMITTER_FUNCTIONS) {
    if (attrs[f] === undefined) continue;
    if (!rgb) rgb = [0, 0, 0];
    const e = f === 'white' ? whiteRgb : EMITTERS[f];
    rgb[0] += attrs[f] * e[0];
    rgb[1] += attrs[f] * e[1];
    rgb[2] += attrs[f] * e[2];
  }
  if (rgb) {
    if (attrs.cct !== undefined && attrs.crossfade !== undefined) {
      const c = cctToRgb(attrs.cct, attrs.tint || 0);
      const k = attrs.crossfade;
      return [c[0] * (1 - k) + rgb[0] * k, c[1] * (1 - k) + rgb[1] * k, c[2] * (1 - k) + rgb[2] * k];
    }
    return rgb;
  }
  if (attrs.hue !== undefined) {
    const white = attrs.cct !== undefined ? cctToRgb(attrs.cct, attrs.tint || 0) : whiteRgb;
    const hsi = hsToRgb(attrs.hue, attrs.sat ?? 1, white);
    // fondu Kelvin ↔ HSI (modes « CCT & HSI ») : 0 = blanc du canal Kelvin, 1 = couleur
    if (attrs.crossfade === undefined || attrs.cct === undefined) return hsi;
    const k = attrs.crossfade;
    return [white[0] * (1 - k) + hsi[0] * k, white[1] * (1 - k) + hsi[1] * k, white[2] * (1 - k) + hsi[2] * k];
  }
  if (attrs.cct !== undefined) return cctToRgb(attrs.cct, attrs.tint || 0);
  if (attrs.x !== undefined && attrs.y !== undefined) return xyToRgb(attrs.x, attrs.y);
  return [...whiteRgb];
}

/** Couleurs de programme d'un mode d'effet (Idx1..4 ou R1/G1/B1..R4/G4/B4). */
function effectColors(fx, opts) {
  const cols = [];
  for (let i = 1; i <= 4; i++) {
    if (fx[`fxIndex${i}`] !== undefined) cols.push(opts.indexColor?.(fx[`fxIndex${i}`]) || [0, 0, 0]);
    else if (fx[`fxR${i}`] !== undefined) cols.push([fx[`fxR${i}`] || 0, fx[`fxG${i}`] || 0, fx[`fxB${i}`] || 0]);
    else cols.push([0, 0, 0]);
  }
  return cols;
}

/**
 * Sortie complète d'un projecteur piloté en DMX.
 * @param {object} p
 * @param {object} p.mode         définition du mode DMX
 * @param {Uint8Array} p.data     univers (512 octets)
 * @param {number} p.address      adresse de départ (1..512)
 * @param {number} p.pixelCount
 * @param {number} p.time         secondes
 * @param {number[]} p.whiteRgb   couleur du blanc "W" / des modes sans couleur
 * @param {(n:number)=>number} [p.curve]
 * @param {(v:number)=>number[]|null} [p.indexColor]  couleurs index (gels)
 * @param {object} [p.runtime]    état persistant par projecteur (redémarrage des effets)
 * @param {Float32Array} out      pixelCount*3
 */
export function decodeDmxOutput({ mode, data, address, pixelCount, time, whiteRgb, curve = DIMMING_CURVES.linear, indexColor, runtime }, out) {
  // Un mode à M pixels sur un projecteur à N pixels physiques : chaque pixel DMX pilote N/M pixels.
  const modePx = typeof mode.pixels === 'number' ? Math.max(1, Math.min(mode.pixels, pixelCount)) : pixelCount;
  const { fixture, pixels } = readAttributes(mode, data, address, modePx);
  const master = fixture.dimmer !== undefined ? curve(fixture.dimmer) : 1;
  const gate = strobeGate(fixture.strobe || 0, time, 0.2, address);

  if (fixture.fxProgram !== undefined) {
    // Mode effet : programme interne du projecteur
    const program = programFromDmx(fixture.fxProgram);
    const rt = runtime || {};
    if (rt.fxProgram !== program || rt.fxRestart !== fixture.fxRestart) {
      rt.fxProgram = program;
      rt.fxRestart = fixture.fxRestart;
      rt.fxStart = time;
    }
    const dir = directionFromDmx(Math.round((fixture.fxDirection ?? 0) * 255));
    renderProgram(
      {
        program,
        colors: effectColors(fixture, { indexColor }),
        pixels: pixelCount,
        time: time - (rt.fxStart ?? 0),
        speed: fixture.fxSpeed ?? 0.5,
        fade: fixture.fxFade ?? 0.5,
        reverse: dir.reverse,
        once: dir.once,
        size: 1 + Math.min(3, Math.floor((fixture.fxSize ?? 0) * 4)),
        offset: fixture.fxOffset ?? 0,
      },
      out,
    );
    const k = master * gate;
    for (let i = 0; i < pixelCount * 3; i++) out[i] *= k;
    return { fixture, pixels };
  }

  const colors = [];
  for (let p = 0; p < modePx; p++) {
    const attrs = { ...fixture, ...pixels[p] };
    const pxDim = pixels[p].dimmer !== undefined ? curve(pixels[p].dimmer) : 1;
    const pxGate = pixels[p].strobe ? strobeGate(pixels[p].strobe, time, 0.2, address + p) : 1;
    const rgb = attributesToRgb(attrs, { whiteRgb, indexColor });
    const k = master * pxDim * gate * pxGate;
    colors.push([rgb[0] * k, rgb[1] * k, rgb[2] * k]);
  }
  for (let j = 0; j < pixelCount; j++) {
    const c = colors[Math.min(modePx - 1, Math.floor((j * modePx) / pixelCount))];
    out[j * 3] = c[0];
    out[j * 3 + 1] = c[1];
    out[j * 3 + 2] = c[2];
  }
  return { fixture, pixels };
}
