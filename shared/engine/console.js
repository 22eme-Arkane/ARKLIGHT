// Console ARKLIGHT : la console intégrée pilote les projecteurs canal par canal, comme une console
// branchée sur leur entrée DMX, mais sans passer par un univers : seuls les projecteurs qu'elle a pris
// en main changent, les autres gardent leurs réglages. Ses valeurs sont rangées dans l'état du
// projecteur (state.console = { mode, values }) : elles s'enregistrent avec le projet et passent avant
// tout le reste (réglages, DMX reçu). « Relâcher » les efface et rend la main au projecteur.
//
// Quand la console prend un projecteur, elle part de ce qu'il fait déjà (réglage manuel, mode autonome
// Astera, DMX reçu) : rien ne saute à l'image.
import { colorSpecRgb } from '../fixtures/astera-colors.js';
import { ASTERA_INDEX_COLORS } from '../fixtures/astera-dmx.js';
import { getMode } from '../fixtures/index.js';
import { EMITTERS, cctToRgb, clamp01, hsToRgb } from './color.js';
import { DEFAULT_RANGES, DIMMING_CURVES, EMITTER_FUNCTIONS, compileMode, decodeDmxOutput, readAttributes } from './dmx.js';

const slotMax = (slot) => (slot.bytes === 2 ? 65535 : 255);
const clamp = (v, lo, hi) => (v < lo ? lo : v > hi ? hi : v);
const isAstera = (type) => !!type.platform?.startsWith('astera-v5');
const isLamp = (type) => type.technology === 'tungsten' || type.technology === 'hmi';

/** Nombre de pixels pilotés par un mode sur un projecteur à `pixelCount` pixels. */
export const modePixels = (mode, pixelCount) => (typeof mode.pixels === 'number' ? Math.max(1, Math.min(mode.pixels, pixelCount)) : pixelCount);

/** Valeurs de départ d'une console vide : plein feu, teinte neutre, température au milieu. */
export function defaultFaders(mode) {
  const { slots } = compileMode(mode);
  const hasCct = slots.some((x) => x.f === 'cct');
  return slots.map((slot) => {
    const max = slotMax(slot);
    if (slot.f === 'dimmer') return max;
    if (slot.f === 'cct' || slot.f === 'tint') return slot.def?.neutral !== undefined ? Math.round(slot.def.neutral * max) : Math.round(max / 2);
    if (['red', 'green', 'blue', 'white'].includes(slot.f) && !hasCct) return max;
    return 0;
  });
}

/**
 * Valeurs brutes par canal (0..255, ou 0..65535 en 16 bits) → trame DMX virtuelle du mode (adresse 1).
 * faders : { mode, values } ; valeurs d'un autre mode : valeurs de départ.
 */
export function faderBuffer(mode, faders, rt = {}) {
  const buf = rt.faderBuf || (rt.faderBuf = new Uint8Array(512));
  buf.fill(0);
  const { slots } = compileMode(mode);
  const values = faders?.mode === mode.id && Array.isArray(faders.values) ? faders.values : defaultFaders(mode);
  slots.forEach((slot, i) => {
    if (slot.offset >= 512) return;
    const v = clamp(Math.round(Number(values[i]) || 0), 0, slotMax(slot));
    if (slot.bytes === 2) {
      buf[slot.offset] = v >> 8;
      if (slot.offset + 1 < 512) buf[slot.offset + 1] = v & 255;
    } else buf[slot.offset] = v;
  });
  return buf;
}

/** Blanc et courbe de gradation du projecteur (conversions de valeurs). */
export function consoleOptions(fixture, type) {
  const s = fixture.settings || {};
  const curve = isAstera(type) ? DIMMING_CURVES[s.dimmerCurve] || DIMMING_CURVES.STANDARD : DIMMING_CURVES[s.dimmingCurve] || DIMMING_CURVES.linear;
  return { whiteK: s.whitePoint || 5600, curve };
}

/** Le projecteur est-il piloté par la console ? */
export const consoleHeld = (fixture) => Array.isArray(fixture?.state?.console?.values);

/**
 * Valeurs de la console pour le mode actuel du projecteur, ou null s'il n'est pas piloté par elle.
 * Profil changé depuis : les valeurs suivent (fonction par fonction), calculées une fois (rt).
 */
export function activeConsoleValues(fixture, type, mode, rt = {}) {
  const con = fixture.state?.console;
  if (!mode || !con || !Array.isArray(con.values)) return null;
  if (con.mode === mode.id) return con.values;
  if (rt.conFrom !== con.values || rt.conTo !== mode.id) {
    const from = getMode(type, con.mode);
    rt.conFrom = con.values;
    rt.conTo = mode.id;
    rt.conValues = from ? remapValues(from, mode, con.values, type.pixels, consoleOptions(fixture, type)) : defaultFaders(mode);
  }
  return rt.conValues;
}

// ------------------------------------------------------------------ valeur d'une fonction → canal
const INVERSE_MAPS = {
  asteraStrobe: (hz) => (!hz ? 0 : hz === -1 ? 4 : hz === -2 ? 5 : hz === -3 ? 6 : clamp(Math.round(7 + ((hz - 0.4) / 24.6) * 248), 7, 255)),
  asteraCct: (k) => clamp(Math.round((k - 1750) / 32), 0, 255),
  asteraCctInd: (k) => (k === undefined ? 0 : clamp(Math.round((k - 2000) / 20), 5, 255)),
  asteraGM: (t) => (!t ? 0 : clamp(Math.round((t + 1) * 128), 5, 255)),
  asteraIdx: (v) => (v === undefined ? 0 : clamp(Math.round(v), 2, 255)),
  asteraFan: (v) => ({ AUTO: 0, LOW: 20, MED: 50, HIGH: 90, OFF: 120 })[v] ?? 0,
  raw: (v) => clamp(Math.round(Number(v) || 0), 0, 255),
};

/** Stroboscope générique : fréquence (Hz, 0 = ouvert) → canal. */
function encodeStrobe(slot, hz) {
  if (!(hz > 0)) return 0;
  const s = slot.def.strobe || { open: 9, min: 1, max: 25 };
  const v = s.open + 1 + ((clamp(hz, s.min, s.max) - s.min) / (s.max - s.min || 1)) * (255 - s.open - 1);
  return Math.round(clamp(v, 0, 255) * (slotMax(slot) / 255));
}

/** Valeur brute d'un canal pour une valeur de sa fonction (inverse du décodage DMX). */
export function encodeSlot(slot, value) {
  const max = slotMax(slot);
  const def = slot.def || {};
  if (def.map) {
    const v8 = INVERSE_MAPS[def.map]?.(value) ?? 0;
    return slot.bytes === 2 ? Math.round(v8 * 257) : v8;
  }
  if (typeof value !== 'number' || !Number.isFinite(value)) return 0;
  if (slot.f === 'strobe') return encodeStrobe(slot, value);
  const range = def.range || DEFAULT_RANGES[slot.f];
  let n;
  if (def.neutral !== undefined) {
    // zone neutre en bas de course : la valeur « neutre » y reste, les autres au-dessus du seuil
    if (Math.abs(value - (def.neutralValue ?? 0)) < 1e-6) n = 0;
    else {
      const r = range || [0, 1];
      const k = clamp01((value - r[0]) / (r[1] - r[0]));
      n = Math.max(def.neutral + 1 / 255, def.neutral + k * (1 - def.neutral));
    }
  } else n = range ? (value - range[0]) / (range[1] - range[0]) : value;
  return Math.round(clamp01(n) * max);
}

/** Valeurs brutes des canaux d'un projecteur dans une trame DMX reçue. */
export function readRawValues(mode, data, address) {
  const start = address - 1;
  return compileMode(mode).slots.map((slot) => {
    const i = start + slot.offset;
    if (i >= 512) return 0;
    return slot.bytes === 2 ? (data[i] || 0) * 256 + (data[i + 1] || 0) : data[i] || 0;
  });
}

/** Recopie fonction par fonction (même façon de faire la couleur dans les deux modes). */
function copyAttributes(from, to, values, pixelCount) {
  const buf = faderBuffer(from, { mode: from.id, values }, {});
  const fromPx = modePixels(from, pixelCount);
  const toPx = modePixels(to, pixelCount);
  const { fixture: fa, pixels } = readAttributes(from, buf, 1, fromPx);
  const target = compileMode(to).slots;
  const globalKeys = new Set(target.filter((s) => s.px < 0).map((s) => s.key));
  const defaults = defaultFaders(to);
  return target.map((slot, i) => {
    if (!slot.f || slot.f === 'none') return defaults[i];
    let v;
    if (slot.px >= 0) {
      const p = Math.min(fromPx - 1, Math.floor((slot.px * fromPx) / toPx));
      v = pixels[p]?.[slot.key];
      // gradateur par pixel quand le nouveau mode a aussi un gradateur général : pixel au maximum
      if (v === undefined && slot.f === 'dimmer' && globalKeys.has('dimmer')) return slotMax(slot);
      if (v === undefined) v = fa[slot.key];
    } else {
      v = fa[slot.key];
      if (v === undefined) v = pixels[0]?.[slot.key];
    }
    return v === undefined ? defaults[i] : encodeSlot(slot, v);
  });
}

// ------------------------------------------------------------------ réglage actuel → valeurs
/** Niveaux d'émetteurs (0..1) dont le mélange donne `target` (moindres carrés positifs bornés). */
export function solveEmitters(target, vectors) {
  const n = vectors.length;
  const e = new Array(n).fill(0);
  let L = 0;
  for (const v of vectors) L += v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
  if (!L) return e;
  const step = 1 / L;
  for (let it = 0; it < 2000; it++) {
    const r = [-target[0], -target[1], -target[2]];
    for (let i = 0; i < n; i++) {
      r[0] += e[i] * vectors[i][0];
      r[1] += e[i] * vectors[i][1];
      r[2] += e[i] * vectors[i][2];
    }
    let moved = 0;
    for (let i = 0; i < n; i++) {
      const g = r[0] * vectors[i][0] + r[1] * vectors[i][1] + r[2] * vectors[i][2];
      const next = clamp01(e[i] - step * g);
      moved += Math.abs(next - e[i]);
      e[i] = next;
    }
    if (moved < 1e-7) break;
  }
  return e;
}

/** Valeur du gradateur qui donne le niveau `level` (0..1) à travers la courbe de gradation. */
function invertCurve(curve, level) {
  if (level <= 0) return 0;
  if (level >= 1) return 1;
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 30; i++) {
    const mid = (lo + hi) / 2;
    if (curve(mid) < level) lo = mid;
    else hi = mid;
  }
  return (lo + hi) / 2;
}

/** Coordonnées xy (CIE 1931) d'une couleur sRGB linéaire. */
function rgbToXy([r, g, b]) {
  const X = 0.4124 * r + 0.3576 * g + 0.1805 * b;
  const Y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  const Z = 0.0193 * r + 0.1192 * g + 0.9505 * b;
  const s = X + Y + Z;
  return s > 0 ? [X / s, Y / s] : [0.3127, 0.329];
}

const dist2 = (a, b) => (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2;

/** Teinte / saturation dont la couleur (mélangée au blanc `white`) approche le mieux `target`. */
function fitHueSat(target, white) {
  let best = { hue: 0, sat: 0, d: dist2(hsToRgb(0, 0, white), target) };
  const scan = (h0, h1, hs, s0, s1, ss) => {
    for (let h = h0; h <= h1; h += hs)
      for (let sat = Math.max(0, s0); sat <= Math.min(1, s1) + 1e-9; sat += ss) {
        const d = dist2(hsToRgb(h, sat, white), target);
        if (d < best.d) best = { hue: (h + 360) % 360, sat, d };
      }
  };
  scan(0, 354, 6, 0, 1, 0.05);
  const { hue, sat } = best;
  scan(hue - 6, hue + 6, 0.5, sat - 0.05, sat + 0.05, 0.005);
  return best;
}

/** Couleur index (gélatine) la plus proche de `target` (modes d'effet FIX des Astera). */
function nearestIndex(target) {
  let best = { v: 0, d: Infinity };
  for (let v = 2; v < 256; v++) {
    const c = ASTERA_INDEX_COLORS.color(v);
    if (!c) continue;
    const M = Math.max(...c) || 1;
    const d = dist2(
      c.map((x) => x / M),
      target,
    );
    if (d < best.d) best = { v, d };
  }
  return best.v;
}

/** Réglage → ce qu'il faut demander au mode (couleur visée, teinte, xy…). */
function prepareLook(look, has, whiteK, whiteRgb) {
  const level = clamp01(look.level ?? 1);
  const isColor = !!look.rgb || look.sat > 0;
  const cct = look.cct ?? 5600;
  const tint = look.tint ?? 0;
  const white = cctToRgb(cct, tint);
  let target;
  if (look.rgb) {
    const M = Math.max(...look.rgb) || 1;
    target = look.rgb.map((c) => Math.max(0, c) / M);
  } else target = look.sat > 0 ? hsToRgb(look.hue ?? 0, look.sat, white) : white;
  // teinte / saturation : celles du réglage quand le mode mélange au même blanc, sinon les plus proches
  const exactWhite = has('cct') ? !tint || has('tint') : Math.abs(cct - whiteK) < 1 && !tint;
  let hs = { hue: 0, sat: 0 };
  if (has('hue')) {
    if (look.hue !== undefined && exactWhite) hs = { hue: look.hue, sat: look.sat ?? 0 };
    else if (isColor || !exactWhite) hs = fitHueSat(target, has('cct') ? cctToRgb(cct, has('tint') ? tint : 0) : whiteRgb);
  }
  // le blanc passe par le canal Kelvin quand le mode a un fondu Kelvin ↔ couleur
  return { level, isColor, cct, tint, target, hs, xy: rgbToXy(target), whiteByCct: !isColor && has('cct') && has('crossfade') };
}

/**
 * Valeurs d'un mode qui reproduisent un réglage.
 * looks : un réglage, ou un par pixel du mode — { level 0..1 (sortie), et cct/tint (blanc),
 * ou hue/sat (+ cct du blanc), ou rgb }.
 */
export function lookToValues(mode, looks, { whiteK = 5600, curve = (n) => n } = {}) {
  const { slots } = compileMode(mode);
  const values = new Array(slots.length).fill(0);
  const has = (f) => slots.some((s) => s.f === f);
  const globalDimmer = slots.some((s) => s.f === 'dimmer' && s.px < 0);
  const pixelDimmer = slots.some((s) => s.f === 'dimmer' && s.px >= 0);
  const whiteRgb = cctToRgb(whiteK); // blanc du projecteur (canal W, modes sans Kelvin)
  const prepared = (Array.isArray(looks) ? looks : [looks]).map((l) => prepareLook(l, has, whiteK, whiteRgb));
  const at = (px) => prepared[px >= 0 ? Math.min(prepared.length - 1, px) : 0];
  const top = Math.max(0, ...prepared.map((l) => l.level)); // le gradateur général donne le plus fort
  // part du niveau portée par les couleurs (le reste par les gradateurs)
  const colorScale = (L, perPixel) => (perPixel && pixelDimmer ? 1 : globalDimmer ? (top > 0 ? L.level / top : 0) : L.level);

  slots.forEach((slot, i) => {
    const L = at(slot.px);
    switch (slot.f) {
      case 'dimmer':
        if (slot.px < 0) values[i] = encodeSlot(slot, invertCurve(curve, top));
        else values[i] = encodeSlot(slot, invertCurve(curve, globalDimmer ? (top > 0 ? L.level / top : 0) : L.level));
        break;
      case 'cct':
        values[i] = encodeSlot(slot, L.cct);
        break;
      case 'tint':
        values[i] = encodeSlot(slot, L.tint);
        break;
      case 'crossfade':
        values[i] = encodeSlot(slot, L.whiteByCct ? 0 : 1);
        break;
      case 'hue':
        values[i] = encodeSlot(slot, L.hs.hue);
        break;
      case 'sat':
        values[i] = encodeSlot(slot, L.hs.sat);
        break;
      case 'x':
        values[i] = encodeSlot(slot, L.xy[0]);
        break;
      case 'y':
        values[i] = encodeSlot(slot, L.xy[1]);
        break;
      default:
        // modes d'effet : programme 0 (« One Color Static ») dans la couleur 1
        if (slot.key === 'fxR1' || slot.key === 'fxG1' || slot.key === 'fxB1') values[i] = encodeSlot(slot, L.target['RGB'.indexOf(slot.key[2])] * colorScale(L, false));
        else if (slot.key === 'fxIndex1') values[i] = nearestIndex(L.target) * (slot.bytes === 2 ? 257 : 1);
        else values[i] = 0; // stroboscope ouvert, programme 0, pas de couleur index…
    }
  });

  // émetteurs (rouge, vert, bleu, blanc, ambre…) : mélange qui donne la couleur, pixel par pixel
  const groups = new Map();
  slots.forEach((slot, i) => {
    if (!EMITTER_FUNCTIONS.includes(slot.f)) return;
    if (!groups.has(slot.px)) groups.set(slot.px, []);
    groups.get(slot.px).push(i);
  });
  for (const [px, list] of groups) {
    const L = at(px);
    if (L.whiteByCct) continue;
    const vectors = list.map((i) => (slots[i].f === 'white' ? whiteRgb : EMITTERS[slots[i].f] || [0, 0, 0]));
    const k = colorScale(L, px >= 0);
    const e = solveEmitters(
      L.target.map((c) => c * k),
      vectors,
    );
    list.forEach((i, j) => (values[i] = encodeSlot(slots[i], e[j])));
  }
  return values;
}

/** Blanc (Kelvin + teinte ±vert) le plus proche d'une couleur normalisée. */
function fitWhite(target) {
  let best = { cct: 5600, tint: 0, d: Infinity };
  const scan = (k0, k1, kStep, t0, t1, tStep) => {
    for (let k = k0; k <= k1; k *= kStep)
      for (let t = Math.max(-1, t0); t <= Math.min(1, t1) + 1e-9; t += tStep) {
        const d = dist2(cctToRgb(k, t), target);
        if (d < best.d) best = { cct: k, tint: t, d };
      }
  };
  scan(1750, 20000, 1.05, -1, 1, 0.1);
  scan(best.cct / 1.05, best.cct * 1.05, 1.004, best.tint - 0.1, best.tint + 0.1, 0.01);
  return best;
}

const COLOR_FUNCTIONS = new Set(['dimmer', 'cct', 'tint', 'crossfade', 'hue', 'sat', 'x', 'y', 'index', 'cctIndex', 'fxProgram', 'fxIndex', 'fxR', 'fxG', 'fxB', ...EMITTER_FUNCTIONS]);

/**
 * Valeurs d'un mode → un autre (changement de profil). Même façon de faire la couleur : on recopie
 * fonction par fonction. Sinon on repart de la lumière produite, pixel par pixel (blanc retrouvé en
 * Kelvin quand c'en est un), et les autres fonctions (stroboscope, vitesse…) suivent si elles existent.
 */
export function remapValues(from, to, values, pixelCount = 1, { whiteK = 5600, curve = (n) => n } = {}) {
  const copied = copyAttributes(from, to, values, pixelCount);
  const fromKeys = new Set(
    compileMode(from)
      .slots.filter((s) => COLOR_FUNCTIONS.has(s.f))
      .map((s) => `${s.key}|${s.px >= 0}`),
  );
  const toSlots = compileMode(to).slots;
  const sameColors = toSlots.filter((s) => COLOR_FUNCTIONS.has(s.f)).every((s) => fromKeys.has(`${s.key}|${s.px >= 0}`));
  if (sameColors && modePixels(from, pixelCount) === modePixels(to, pixelCount)) return copied;

  // lumière produite par l'ancien mode, moyenne sur chaque pixel du nouveau
  const out = new Float32Array(pixelCount * 3);
  const data = faderBuffer(from, { mode: from.id, values }, {});
  decodeDmxOutput({ mode: from, data, address: 1, pixelCount, time: 0, whiteRgb: cctToRgb(whiteK), curve, indexColor: ASTERA_INDEX_COLORS.color }, out);
  const toPx = modePixels(to, pixelCount);
  const looks = [];
  for (let p = 0; p < toPx; p++) {
    const a = Math.floor((p * pixelCount) / toPx);
    const b = Math.max(a + 1, Math.floor(((p + 1) * pixelCount) / toPx));
    const c = [0, 0, 0];
    for (let j = a; j < b; j++) for (let k = 0; k < 3; k++) c[k] += out[j * 3 + k] / (b - a);
    const M = Math.max(...c);
    const target = M > 0 ? c.map((x) => x / M) : [1, 1, 1];
    const w = fitWhite(target);
    looks.push(w.d < 2e-4 ? { level: Math.min(1, M), cct: w.cct, tint: w.tint } : { level: Math.min(1, M), rgb: target });
  }
  const colored = lookToValues(to, looks, { whiteK, curve });
  return toSlots.map((slot, i) => (COLOR_FUNCTIONS.has(slot.f) ? colored[i] : copied[i]));
}

/**
 * Réglage que donnent des valeurs de console : intensité de sortie et couleur (blanc en Kelvin et teinte
 * ±vert quand c'en est un, sinon teinte et saturation), d'après la lumière produite (moyenne des pixels).
 */
export function lookOfValues(mode, values, { pixelCount = 1, whiteK = 5600, curve = (n) => n } = {}) {
  const out = new Float32Array(pixelCount * 3);
  decodeDmxOutput({ mode, data: faderBuffer(mode, { mode: mode.id, values }, {}), address: 1, pixelCount, time: 0, whiteRgb: cctToRgb(whiteK), curve, indexColor: ASTERA_INDEX_COLORS.color }, out);
  const c = [0, 0, 0];
  for (let p = 0; p < pixelCount; p++) for (let k = 0; k < 3; k++) c[k] += out[p * 3 + k] / pixelCount;
  const M = Math.max(...c);
  if (!(M > 0)) return { level: 0, cct: whiteK, tint: 0 };
  const target = c.map((x) => x / M);
  const w = fitWhite(target);
  if (w.d < 2e-4) return { level: Math.min(1, M), cct: Math.round(w.cct), tint: Math.round(w.tint * 100) / 100 };
  const hs = fitHueSat(target, cctToRgb(whiteK));
  return { level: Math.min(1, M), hue: hs.hue, sat: hs.sat, cct: whiteK };
}

/**
 * Change la couleur de valeurs de console — Kelvin ou teinte ±vert (un blanc), teinte ou saturation
 * (une couleur) — sans toucher à l'intensité. Les canaux du mode servent quand ils existent (le reste ne
 * bouge pas) ; sinon la couleur est refaite avec ses émetteurs (rouge, vert, bleu, blanc…).
 * look : réglage de départ (sinon retrouvé d'après les valeurs). Renvoie { values, look }.
 */
export function adjustColor(mode, values, change, { pixelCount = 1, whiteK = 5600, curve = (n) => n, look = null } = {}) {
  const { slots } = compileMode(mode);
  const has = (f) => slots.some((s) => s.f === f);
  const base = look || lookOfValues(mode, values, { pixelCount, whiteK, curve });
  const next = { level: base.level, cct: base.cct ?? whiteK, tint: base.tint ?? 0, hue: base.hue ?? 0, sat: base.sat ?? 0 };
  const white = change.cct !== undefined || change.tint !== undefined;
  if (white) {
    Object.assign(next, change);
    next.sat = 0; // un réglage de blanc : la lumière redevient blanche
  } else {
    if (change.hue !== undefined) {
      next.hue = change.hue;
      if (!(next.sat > 0)) next.sat = 1; // on tourne la teinte d'un blanc : la couleur apparaît
    }
    if (change.sat !== undefined) next.sat = change.sat;
  }
  const emitters = slots.some((s) => EMITTER_FUNCTIONS.includes(s.f));
  const out = [...values];
  const setAll = (f, v) => slots.forEach((s, i) => s.f === f && (out[i] = encodeSlot(s, v)));
  // canaux du mode : Kelvin / teinte ±vert, fondu blanc ↔ couleur, teinte / saturation
  const whiteDirect = white && has('cct') && (!emitters || has('crossfade')) && (change.tint === undefined || has('tint'));
  const colorDirect = !white && has('hue') && !emitters;
  if (whiteDirect) {
    setAll('cct', next.cct);
    if (has('tint')) setAll('tint', next.tint);
    if (has('crossfade')) setAll('crossfade', 0);
    if (has('sat')) setAll('sat', 0);
    return { values: out, look: { level: next.level, cct: next.cct, tint: next.tint } };
  }
  if (colorDirect) {
    setAll('hue', next.hue);
    if (has('sat')) setAll('sat', next.sat);
    if (has('crossfade')) setAll('crossfade', 1);
    return { values: out, look: { level: next.level, hue: next.hue, sat: next.sat, cct: next.cct } };
  }
  // sinon : même intensité, couleur refaite ; les autres fonctions (stroboscope, effets…) ne bougent pas
  const target = next.sat > 0 ? { level: next.level, hue: next.hue, sat: next.sat, cct: next.cct } : { level: next.level, cct: next.cct, tint: next.tint };
  const colored = lookToValues(mode, target, { whiteK, curve });
  return { values: slots.map((s, i) => (COLOR_FUNCTIONS.has(s.f) ? colored[i] : values[i])), look: target };
}

/** Réglage actuel (hors DMX) : niveau de sortie et couleur. */
export function currentLook(fixture, type, source = null) {
  const s = fixture.settings || {};
  const st = fixture.state || {};
  if (isAstera(type)) {
    if (source === 'app' && st.app) return { level: st.app.intensity ?? 1, rgb: st.app.rgb || [1, 1, 1] };
    const sa = s.standalone || {};
    const spec = sa.colors?.[0] || { kind: 'hsi', hue: 0, sat: 0, cct: 5600 };
    const level = clamp01(((sa.intensity ?? 100) / 100) * ((s.brightness ?? 100) / 100));
    if (spec.kind === 'hsi') return { level, cct: spec.cct ?? 5600, tint: spec.green ?? 0, ...(spec.sat > 0 ? { hue: spec.hue ?? 0, sat: spec.sat / 100 } : {}) };
    return { level, rgb: colorSpecRgb(spec, { cct: cctToRgb, hs: hsToRgb }) };
  }
  const m = st.manual || {};
  const curve = DIMMING_CURVES[s.dimmingCurve] || DIMMING_CURVES.linear;
  const level = curve(clamp01(m.intensity ?? 0));
  if (isLamp(type)) return { level, cct: type.lamp?.cct || (type.technology === 'hmi' ? 5600 : 3200) };
  if (m.mode === 'hsi') return { level, hue: m.hue || 0, sat: m.sat ?? 1, cct: s.whitePoint || 5600 };
  if (m.mode === 'rgb') return { level, rgb: m.rgb || [1, 1, 1] };
  return { level, cct: m.cct || 5600, tint: m.tint || 0 };
}

/**
 * Valeurs de la console pour `mode` au plus près de ce que fait le projecteur.
 * @param {object} [o]
 * @param {Map<number, Uint8Array>} [o.dmx]  univers reçus (projecteur piloté en DMX)
 * @param {string} [o.source]                source actuelle (calcul de la vue : 'dmx', 'manual', 'app'…)
 */
export function seedConsole(fixture, type, mode, { dmx = null, source = null } = {}) {
  const s = fixture.settings || {};
  const cur = getMode(type, s.dmxMode);
  if (consoleHeld(fixture)) return [...activeConsoleValues(fixture, type, mode, {})];
  if (source === 'dmx' && cur) {
    const data = dmx?.get?.(s.universe);
    if (data) {
      const raw = readRawValues(cur, data, s.dmxAddress);
      return cur.id === mode.id ? raw : remapValues(cur, mode, raw, type.pixels, consoleOptions(fixture, type));
    }
  }
  const m = fixture.state?.manual || {};
  if (!isAstera(type) && m.mode === 'faders' && cur && m.faders?.mode === cur.id && Array.isArray(m.faders.values)) {
    // ancien mode « Canaux » des réglages : mêmes valeurs
    return cur.id === mode.id ? [...m.faders.values] : remapValues(cur, mode, m.faders.values, type.pixels, consoleOptions(fixture, type));
  }
  return lookToValues(mode, currentLook(fixture, type, source), consoleOptions(fixture, type));
}

/**
 * Changements qui rendent la main au projecteur : un réglage de sa lumière fait à la main (inspecteur,
 * menu du projecteur, assistant) libère la console, comme le dernier geste l'emporte.
 * Renvoie les changements, complétés si besoin de { state: { console: null } }.
 */
export function releaseOnLookChange(fixture, changes) {
  if (!consoleHeld(fixture) || !changes || changes.state?.console !== undefined) return changes;
  const look = changes.state?.manual !== undefined || changes.state?.app !== undefined || changes.settings?.standalone !== undefined;
  return look ? { ...changes, state: { ...(changes.state || {}), console: null } } : changes;
}
