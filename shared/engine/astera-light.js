// Comportement lumineux des projecteurs Astera (plateforme firmware « V5 ») :
// sélection d'entrée et verrouillage AUTO, mode autonome, DMX (CRMX / filaire), DMX FAIL,
// limitation par l'autonomie choisie, BlueMode, demande « ACCEPT CONFIG ? ».
// Références : docs/recherche/menus/common.md, tubes.md, ax1_ax2.md.
import { getMode } from '../fixtures/index.js';
import { ASTERA_INDEX_COLORS } from '../fixtures/astera-dmx.js';
import { colorSpecRgb } from '../fixtures/astera-colors.js';
import { cctToRgb, hsToRgb } from './color.js';
import { activeConsoleValues, faderBuffer } from './console.js';
import { DIMMING_CURVES, decodeDmxOutput } from './dmx.js';
import { renderProgram } from './effects.js';

const INPUT_OF_SELECT = {
  STANDALONE: 'STANDALONE',
  'APP CONTROL': 'APP',
  'Wired DMX': 'WIRED',
  'ART3 DMX': 'ART3',
  'CRMX wirel. DMX': 'CRMX',
};

/** Durée de cycle de la vitesse autonome (0 = 0,09 s … 9 = 10 s). */
export const standaloneSpeedSeconds = (i) => 0.09 * Math.pow(10 / 0.09, i / 9);

/** Facteur de sortie maximal imposé par l'autonomie choisie (la lampe se bride pour tenir la durée). */
export function runtimeFactor(type, runtimeHours) {
  const full = type.maxBrightnessRuntime || 1.5;
  return Math.min(1, full / Math.max(1, runtimeHours || 5));
}

const fill = (out, pixels, rgb, k = 1) => {
  for (let p = 0; p < pixels; p++) {
    out[p * 3] = rgb[0] * k;
    out[p * 3 + 1] = rgb[1] * k;
    out[p * 3 + 2] = rgb[2] * k;
  }
};

/** Rendu du « look » autonome (programme interne + palette de 4 couleurs). */
export function renderStandalone(fixture, type, time, rt, out) {
  const sa = fixture.settings.standalone || {};
  const colors = (sa.colors || []).map((spec) => colorSpecRgb(spec, { cct: cctToRgb, hs: hsToRgb }));
  if (rt.saProgram !== sa.program) {
    rt.saProgram = sa.program;
    rt.saStart = time;
  }
  const seconds = standaloneSpeedSeconds(sa.speed ?? 5);
  // vitesse 0..1 du moteur d'effets telle que le cycle dure `seconds`
  const speed = Math.log(seconds / 10) / Math.log(0.09 / 10);
  renderProgram({ program: sa.program || 0, colors, pixels: type.pixels, time: time - (rt.saStart ?? 0), speed, fade: (sa.fade ?? 50) / 100 }, out);
  const k = ((sa.intensity ?? 100) / 100) * ((fixture.settings.brightness ?? 100) / 100);
  for (let i = 0; i < type.pixels * 3; i++) out[i] *= k;
}

/**
 * Détermine l'entrée active (verrouillage AUTO : la première source active est retenue
 * jusqu'à l'extinction ou un changement manuel).
 * @returns {'DETECTING'|'CRMX'|'WIRED'|'APP'|'STANDALONE'|'ART3'}
 */
export function resolveInput(fixture, ctx, rt) {
  const s = fixture.settings;
  const live = ctx.live?.has(s.universe);
  if ((s.inputSelect || 'AUTO') !== 'AUTO') {
    rt.latch = null;
    return INPUT_OF_SELECT[s.inputSelect] || 'STANDALONE';
  }
  if (!rt.latch) {
    if (s.crmxLinked && live) rt.latch = 'CRMX';
    else if (s.wired && live) rt.latch = 'WIRED';
    else if (fixture.state.app) rt.latch = 'APP';
  }
  return rt.latch || 'DETECTING';
}

/**
 * Sortie d'un projecteur Astera V5.
 * @param {object} ctx { dmx: Map, live: Set, time }
 * @param {object} rt  état d'exécution local (verrou d'entrée, effets…)
 */
export function computeAsteraOutput(fixture, type, ctx, out, rt) {
  const s = fixture.settings;
  const st = fixture.state;
  const time = ctx.time;
  const px = type.pixels;
  if (!st.power) {
    rt.latch = null;
    rt.input = 'OFF';
    out.fill(0);
    return 'off';
  }
  if (st.blueMode) {
    // clignotement bleu rapide, ou couleur choisie (BlueMode de l'AX1)
    const on = (time * 3) % 1 < 0.5 ? 1 : 0.08;
    fill(out, px, rt.blueColor || [0, 0.12, 1], on * 0.8);
    rt.input = 'BLUEMODE';
    return 'bluemode';
  }
  if (st.standby) {
    out.fill(0);
    rt.input = 'STANDBY';
    return 'standby';
  }

  const input = resolveInput(fixture, ctx, rt);
  rt.input = input;
  const curve = DIMMING_CURVES[s.dimmerCurve] || DIMMING_CURVES.STANDARD;
  let src = 'standalone';
  const mode = getMode(type, s.dmxMode);
  const conValues = activeConsoleValues(fixture, type, mode, rt);

  if (conValues) {
    // Console ARKLIGHT : elle passe avant l'entrée choisie (comme un DMX reçu, profil du projecteur)
    const buf = faderBuffer(mode, { mode: mode.id, values: conValues }, rt);
    decodeDmxOutput({ mode, data: buf, address: 1, pixelCount: px, time, whiteRgb: cctToRgb(s.whitePoint || 5600), curve, indexColor: ASTERA_INDEX_COLORS.color, runtime: rt }, out);
    src = 'console';
  } else if (input === 'CRMX' || input === 'WIRED') {
    const connected = input === 'CRMX' ? s.crmxLinked : s.wired;
    const data = ctx.dmx.get(s.universe);
    const signal = connected && ctx.live?.has(s.universe);
    rt.signal = !!signal;
    const decode = () => {
      const mode = getMode(type, s.dmxMode);
      decodeDmxOutput(
        { mode, data, address: s.dmxAddress, pixelCount: px, time, whiteRgb: cctToRgb(s.whitePoint || 5600), curve, indexColor: ASTERA_INDEX_COLORS.color, runtime: rt },
        out,
      );
    };
    if (signal && data) {
      decode();
      src = 'dmx';
    } else {
      switch (s.dmxFail) {
        case 'EMERGENCY LIGHT':
          fill(out, px, cctToRgb(4000));
          src = 'emergency';
          break;
        case 'BLACKOUT':
          out.fill(0);
          src = 'blackout';
          break;
        case 'STANDALONE':
          renderStandalone(fixture, type, time, rt, out);
          break;
        default:
          // HOLD : dernier état reçu
          if (connected && data) {
            decode();
            src = 'dmx';
          } else out.fill(0);
      }
    }
  } else if (input === 'APP' && st.app) {
    const a = st.app;
    fill(out, px, a.rgb || [1, 1, 1], a.intensity ?? 1);
    src = 'app';
  } else {
    renderStandalone(fixture, type, time, rt, out);
  }

  // Demande de l'appli « ACCEPT CONFIG ? » : flash blanc toutes les 2 s
  if (st.acceptConfig && time % 2 < 0.15) fill(out, px, [1, 1, 1]);

  // Bridage par l'autonomie choisie (toujours actif, même sur secteur)
  const gain = type.outputGain && s.outputGain === 'OFF' ? type.outputGain : 1;
  const k = (s.runtimeLimit === false || type.power === 'mains' ? 1 : runtimeFactor(type, s.runtime)) * gain;
  for (let i = 0; i < px * 3; i++) out[i] *= k;
  rt.src = src;
  return src;
}

/** Réglages appliqués par RESET SETTINGS (firmware actuel). */
export function resetSettings(type, settings) {
  return {
    inputSelect: 'AUTO',
    crmxLinked: false,
    runtime: 5,
    ...(type.outputGain ? { outputGain: 'ON' } : {}),
    rfRegion: null,
    ...(type.btb ? { btbBonded: false } : {}),
    standalone: {
      ...settings.standalone,
      program: 0,
      intensity: 100,
      colors: [{ kind: 'named', name: 'RED' }, ...(settings.standalone?.colors || []).slice(1)],
    },
  };
}
