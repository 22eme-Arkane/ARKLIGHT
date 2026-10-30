// Sortie lumineuse d'un projecteur à un instant donné (couleur de chaque pixel, sRGB linéaire).
import { getMode } from '../fixtures/index.js';
import { DIMMING_CURVES, decodeDmxOutput, readAttributes } from './dmx.js';
import { cctToRgb, hsToRgb } from './color.js';
import { computeAsteraOutput } from './astera-light.js';
import { activeConsoleValues, faderBuffer } from './console.js';
import { ASTERA_INDEX_COLORS } from '../fixtures/astera-dmx.js';

export { defaultFaders, faderBuffer } from './console.js';

const whiteCache = new Map();
function whiteFor(kelvin) {
  let w = whiteCache.get(kelvin);
  if (!w) {
    w = cctToRgb(kelvin);
    whiteCache.set(kelvin, w);
  }
  return w;
}

/**
 * Source de commande effective : 'dmx' | 'manual'.
 * En mode auto, le DMX prend la main dès qu'un univers a été reçu (et le garde : "hold last look").
 */
export function controlSource(fixture, hasDmx) {
  const pref = fixture.settings.source || 'auto';
  if (pref === 'dmx' || pref === 'manual') return pref;
  return hasDmx ? 'dmx' : 'manual';
}

export const isAsteraV5 = (type) => !!type.platform?.startsWith('astera-v5');
export const isTraditional = (type) => type.technology === 'tungsten' || type.technology === 'hmi';

// ---------------------------------------------------------------- sources traditionnelles
/**
 * Température de couleur d'une halogène gradée : CCT ≈ CCT nominale × flux^0,107
 * (3200 K → environ 2970 K à 50 % du flux ; brevet US 2004/0169477, Marumo / NHK).
 */
export const tungstenCct = (nominal, flux) => nominal * Math.pow(Math.max(1e-4, flux), 0.107);

/** Inertie du filament (approximation, non mesurée) : environ 0,1 s pour 1 kW, plus lente pour les grosses lampes. */
const filamentTau = (watts) => 0.04 + 0.06 * Math.sqrt((watts || 1000) / 1000);

/**
 * HMI (approximation, à vérifier sur le vrai matériel) : ballast gradable de 50 à 100 % de puissance,
 * lumière plus froide quand on gradue, montée en régime d'environ 1 minute après l'allumage.
 */
export const HMI = { minPower: 0.5, dimShiftK: 800, warmupTau: 15, warmupStart: 0.3 };

function computeTraditional(fixture, type, ctx, out, rt) {
  const s = fixture.settings;
  const n = type.pixels;
  const nominal = type.lamp?.cct || (type.technology === 'hmi' ? 5600 : 3200);
  const curve = DIMMING_CURVES[s.dimmingCurve] || DIMMING_CURVES.linear;
  const data = ctx.dmx.get(s.universe);
  const mode = getMode(type, s.dmxMode);
  const conValues = activeConsoleValues(fixture, type, mode, rt);
  const src = conValues ? 'console' : controlSource(fixture, !!data);

  // Niveau demandé par circuit (pixel) : canal Dimmer en DMX (ou console), intensité manuelle sinon
  const levels = new Array(n).fill(0);
  if (fixture.state.power) {
    if (src === 'dmx' || src === 'console') {
      const buf = src === 'console' ? faderBuffer(mode, { mode: mode.id, values: conValues }, rt) : data;
      if (mode && buf) {
        const modePx = typeof mode.pixels === 'number' ? Math.max(1, Math.min(mode.pixels, n)) : 1;
        const { fixture: fa, pixels } = readAttributes(mode, buf, src === 'console' ? 1 : s.dmxAddress, modePx);
        for (let p = 0; p < n; p++) {
          const v = pixels[Math.min(modePx - 1, Math.floor((p * modePx) / n))]?.dimmer ?? fa.dimmer ?? 0;
          levels[p] = curve(v);
        }
      }
    } else levels.fill(curve(Math.max(0, Math.min(1, fixture.state.manual?.intensity ?? 0))));
  }

  const dt = rt.lastTime === undefined ? 1 : Math.max(0, Math.min(1, ctx.time - rt.lastTime));
  rt.lastTime = ctx.time;
  if (!rt.flux || rt.flux.length !== n) rt.flux = new Array(n).fill(0);

  if (type.technology === 'hmi') {
    // Allumage : la lampe monte en régime ; extinction immédiate
    if (!fixture.state.power) rt.hmiOn = null;
    else if (rt.hmiOn === null || rt.hmiOn === undefined) rt.hmiOn = ctx.time;
    const warm = fixture.state.power ? 1 - (1 - HMI.warmupStart) * Math.exp(-(ctx.time - rt.hmiOn) / HMI.warmupTau) : 0;
    for (let p = 0; p < n; p++) {
      // niveau 0 : lampe éteinte ; au-dessus, puissance de 50 à 100 %
      const power = levels[p] > 0 ? HMI.minPower + (1 - HMI.minPower) * levels[p] : 0;
      rt.flux[p] = power * warm;
      const cct = power > 0 ? nominal + (HMI.dimShiftK * (1 - power)) / (1 - HMI.minPower) : nominal;
      const rgb = cctToRgb(cct);
      out[p * 3] = rgb[0] * rt.flux[p];
      out[p * 3 + 1] = rgb[1] * rt.flux[p];
      out[p * 3 + 2] = rgb[2] * rt.flux[p];
    }
    return src !== 'manual' ? src : fixture.state.power ? 'manual' : 'off';
  }

  // Tungstène : le filament suit le gradateur avec un léger retard, la couleur suit le flux
  const k = 1 - Math.exp(-dt / filamentTau(type.lamp?.watts));
  for (let p = 0; p < n; p++) {
    rt.flux[p] += (levels[p] - rt.flux[p]) * k;
    const f = rt.flux[p];
    const rgb = whiteFor(Math.round(tungstenCct(nominal, f) / 10) * 10);
    out[p * 3] = rgb[0] * f;
    out[p * 3 + 1] = rgb[1] * f;
    out[p * 3 + 2] = rgb[2] * f;
  }
  return src !== 'manual' ? src : fixture.state.power ? 'manual' : 'off';
}

/**
 * @param {object} fixture   instance (settings, state)
 * @param {object} type      définition du type
 * @param {{dmx: Map<number, Uint8Array>, live?: Set<number>, time: number}} ctx
 * @param {Float32Array} out pixels*3
 * @param {object} [rt]      état d'exécution local du projecteur
 * @returns {string} source effective
 */
export function computeOutput(fixture, type, ctx, out, rt = {}) {
  if (!type.pixels) return 'device'; // boîtier sans lumière (AsteraBox…)
  if (isAsteraV5(type)) return computeAsteraOutput(fixture, type, ctx, out, rt);
  if (isTraditional(type)) return computeTraditional(fixture, type, ctx, out, rt);
  if (!fixture.state.power) {
    out.fill(0);
    return 'off';
  }
  const s = fixture.settings;
  const curve = DIMMING_CURVES[s.dimmingCurve] || DIMMING_CURVES.linear;
  const whiteRgb = whiteFor(s.whitePoint || 5600);
  const data = ctx.dmx.get(s.universe);
  const mode = getMode(type, s.dmxMode);

  // Console ARKLIGHT : elle passe avant tout le reste tant qu'elle pilote le projecteur
  const conValues = activeConsoleValues(fixture, type, mode, rt);
  if (conValues) {
    const buf = faderBuffer(mode, { mode: mode.id, values: conValues }, rt);
    decodeDmxOutput({ mode, data: buf, address: 1, pixelCount: type.pixels, time: ctx.time, whiteRgb, curve, indexColor: ASTERA_INDEX_COLORS.color, runtime: rt }, out);
    return 'console';
  }

  const src = controlSource(fixture, !!data);
  if (src === 'dmx') {
    if (!mode || !data) {
      out.fill(0);
      return 'dmx';
    }
    decodeDmxOutput(
      { mode, data, address: s.dmxAddress, pixelCount: type.pixels, time: ctx.time, whiteRgb, curve, indexColor: ASTERA_INDEX_COLORS.color, runtime: rt },
      out,
    );
    return 'dmx';
  }

  const m = fixture.state.manual || {};
  if (m.mode === 'faders') {
    // ancien réglage « Canaux » (projets des premières versions) : lu comme un DMX reçu
    if (mode) {
      const buf = faderBuffer(mode, m.faders, rt);
      decodeDmxOutput({ mode, data: buf, address: 1, pixelCount: type.pixels, time: ctx.time, whiteRgb, curve, indexColor: ASTERA_INDEX_COLORS.color, runtime: rt }, out);
      return 'manual';
    }
  }
  let rgb;
  if (m.mode === 'hsi') rgb = hsToRgb(m.hue || 0, m.sat ?? 1, whiteRgb);
  else if (m.mode === 'rgb') rgb = m.rgb || [1, 1, 1];
  else rgb = cctToRgb(m.cct || 5600, m.tint || 0);
  const k = curve(Math.max(0, Math.min(1, m.intensity || 0)));
  for (let p = 0; p < type.pixels; p++) {
    out[p * 3] = rgb[0] * k;
    out[p * 3 + 1] = rgb[1] * k;
    out[p * 3 + 2] = rgb[2] * k;
  }
  return 'manual';
}
