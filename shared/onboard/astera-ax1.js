// Projecteurs Astera à bouton unique, sans écran : AX1 PixelTube, AX3 Lightdrop (deux boutons en
// parallèle). La lumière elle-même sert d'affichage (jauge de batterie, charge, BlueMode, confirmation,
// adressage DMX par secousse sur l'AX1).
// D'après les Technical Reference Manuals AX1 (2020) et AX3 (2023) : docs/recherche/menus/ax1_ax2.md, ax3_ax5_ax7.md.
import { AX1_BLUEMODE_COLORS, asteraRgbToLinear } from '../fixtures/astera-colors.js';
import { resetSettings } from '../engine/astera-light.js';

/** Adressage DMX par le bouton : couleur affichée -> profil et adresses possibles (AX1 manuel 2018 p.16). */
export const AX1_ADDRESSING = [
  { color: [0.18, 0.02, 0.6], label: 'violet', mode: '42', name: '42: RGB RGB (16 px)', step: 48, count: 10 },
  { color: [0, 1, 0], label: 'vert', mode: '1', name: '1: RGB (1 px)', step: 3, count: 15 },
  { color: [1, 0.15, 0.45], label: 'rose', mode: '48', name: '48: RGB CCT DIM IND (16 px)', step: 96, count: 5 },
  { color: [1, 0, 0], label: 'rouge', mode: '7', name: '7: RGB CCT DIM IND (1 px)', step: 6, count: 15 },
];

const fill = (out, pixels, rgb, from = 0, to = pixels) => {
  for (let p = 0; p < pixels; p++) {
    const on = p >= from && p < to;
    out[p * 3] = on ? rgb[0] : 0;
    out[p * 3 + 1] = on ? rgb[1] : 0;
    out[p * 3 + 2] = on ? rgb[2] : 0;
  }
};

export class AsteraAx1Controller {
  constructor({ type, getFixture, emit, rt }) {
    this.type = type;
    this.getFixture = getFixture;
    this.emit = emit;
    this.rt = rt;
    this.pressed = new Map();
    this.lastT = 0;
    this.overlay = null; // { kind, until }
    this.colorIdx = -1; // couleur choisie en BlueMode
    this.addressing = null; // { profile, index }
    this.holdStage = null; // retour visuel pendant un appui long
    const cfg = type.controls || {};
    this.gaugeKind = cfg.gauge || 'bar'; // 'bar' (AX1 : longueur) ou 'color' (AX3 : couleur seule)
    this.canShake = cfg.shake !== false;
    this.offGesture = cfg.offGesture || 'hold'; // 'short' : un appui court éteint (SolaBulb)
    this.alias = Object.fromEntries((cfg.keys || []).filter((k) => k.alias).map((k) => [k.id, k.alias]));
  }

  get f() {
    return this.getFixture();
  }

  press(id, t) {
    this.lastT = t;
    this.pressed.set(this.alias[id] || id, { t0: t });
  }

  release(id, t) {
    id = this.alias[id] || id;
    this.lastT = t;
    const p = this.pressed.get(id);
    this.pressed.delete(id);
    this.holdStage = null;
    if (!p || id !== 'power') return;
    const held = t - p.t0;
    const st = this.f.state;

    if (!st.power) {
      if (held >= 1) this.powerOn(t);
      return;
    }
    // Adressage DMX par secousse
    if (this.addressing) {
      const a = this.addressing;
      const prof = AX1_ADDRESSING[a.profile];
      if (held < 1) a.index = (a.index + 1) % prof.count;
      else {
        this.emit({ settings: { dmxMode: prof.mode, dmxAddress: 1 + a.index * prof.step, dimmerCurve: 'STANDARD', dmxFail: 'HOLD' }, state: { blueMode: false } });
        this.addressing = null;
        this.overlay = { kind: 'confirm', until: t + 1.2 };
      }
      return;
    }
    // BlueMode : appui court = couleur suivante, maintien 1 s = valider la couleur et sortir
    if (st.blueMode) {
      if (held < 1) {
        this.colorIdx = (this.colorIdx + 1) % AX1_BLUEMODE_COLORS.length;
        this.rt.blueColor = asteraRgbToLinear(AX1_BLUEMODE_COLORS[this.colorIdx][1]);
      } else if (held < 2.5) {
        if (this.colorIdx >= 0) {
          const name = AX1_BLUEMODE_COLORS[this.colorIdx][0];
          const sa = this.f.settings.standalone;
          this.emit({ settings: { standalone: { ...sa, program: 0, colors: [{ kind: 'named', name }, ...(sa.colors || []).slice(1)] } }, state: { blueMode: false } });
          this.rt.latch = 'STANDALONE';
        } else this.emit({ state: { blueMode: false } });
        this.colorIdx = -1;
        this.rt.blueColor = null;
      }
      return;
    }
    // Lampe allumée
    if (held < 1) {
      if (st.acceptConfig) {
        this.emit({ state: { acceptConfig: false } });
        this.overlay = { kind: 'tap', until: t + 1.2 };
      } else if (this.offGesture === 'short') this.emit({ state: { power: false, blueMode: false } });
      else if (this.gaugeKind !== 'none') this.overlay = { kind: 'gauge', until: t + 2 };
    } else if (held < 2.5) {
      this.emit({ state: { power: false, blueMode: false } });
    } else if (held < 5) {
      this.enterBlueMode();
    } else {
      this.doReset(t);
    }
  }

  powerOn(t) {
    this.emit({ state: { power: true, blueMode: false } });
    this.rt.latch = null;
    if (this.gaugeKind !== 'none') this.overlay = { kind: 'gauge', until: t + 1.5 };
  }

  enterBlueMode() {
    this.colorIdx = -1;
    this.rt.blueColor = null;
    this.emit({ state: { blueMode: true } });
  }

  doReset(t) {
    this.emit({ settings: resetSettings(this.type, this.f.settings), state: { blueMode: false } });
    this.rt.latch = null;
    this.rt.blueColor = null;
    this.overlay = { kind: 'red', until: t + 0.5 };
  }

  /** Secouer le tube (accéléromètre) : passe en adressage DMX depuis le BlueMode, ou change de profil. */
  shake() {
    const st = this.f.state;
    if (!st.power || !this.canShake) return;
    if (this.addressing) {
      this.addressing.profile = (this.addressing.profile + 1) % AX1_ADDRESSING.length;
      this.addressing.index = 0;
    } else if (st.blueMode) {
      this.addressing = { profile: 0, index: 0 };
    }
  }

  tick(t) {
    this.lastT = t;
    const p = this.pressed.get('power');
    const st = this.f.state;
    if (p && st.power && !this.addressing && !st.blueMode) {
      const held = t - p.t0;
      this.holdStage = held >= 5 ? 'reset' : held >= 2.5 ? 'blue' : held >= 1 ? 'off' : null;
    } else if (p && !st.power) {
      this.holdStage = t - p.t0 >= 1 ? 'on' : null;
    }
    if (this.overlay && t > this.overlay.until) this.overlay = null;
  }

  heldFor(id, t = this.lastT) {
    const p = this.pressed.get(id);
    return p ? t - p.t0 : 0;
  }

  /** Le tube sert d'écran : remplace la sortie normale quand il affiche un état. */
  overridePixels(out, pixels, t) {
    const st = this.f.state;
    // Appui long en cours : extinction, puis BlueMode, puis rouge (reset)
    if (this.holdStage === 'off') return fill(out, pixels, [0, 0, 0]), true;
    if (this.holdStage === 'blue') return fill(out, pixels, (t * 3) % 1 < 0.5 ? [0, 0.12, 0.9] : [0, 0, 0]), true;
    if (this.holdStage === 'reset' || this.overlay?.kind === 'red') return fill(out, pixels, [0.8, 0, 0]), true;
    if (!st.power) {
      if (!st.charging) return false;
      if ((st.battery ?? 100) >= 100) return fill(out, pixels, [0, 0, 0]), true;
      if (this.gaugeKind === 'color') return fill(out, pixels, t % 1 < 0.5 ? [0, 0.5, 0] : [0, 0, 0]), true; // vert, clignotement lent
      const head = Math.floor((t * 6) % pixels);
      return fill(out, pixels, [0, 0.5, 0], head, head + 1), true;
    }
    if (this.addressing) {
      const prof = AX1_ADDRESSING[this.addressing.profile];
      const code = this.addressing.index + 1;
      const q = pixels / 4;
      for (let p = 0; p < pixels; p++) {
        const lit = (code >> Math.floor(p / q)) & 1;
        out[p * 3] = lit ? prof.color[0] * 0.7 : 0;
        out[p * 3 + 1] = lit ? prof.color[1] * 0.7 : 0;
        out[p * 3 + 2] = lit ? prof.color[2] * 0.7 : 0;
      }
      return true;
    }
    if (this.overlay?.kind === 'gauge') {
      const b = (st.battery ?? 100) / 100;
      if (this.gaugeKind === 'color') {
        // AX3 : rouge < 30 min restantes, orange au-delà de 30 min utilisées, vert sinon (seuils estimés)
        fill(out, pixels, b < 0.2 ? [0.7, 0, 0] : b < 0.85 ? [0.8, 0.35, 0] : [0, 0.6, 0]);
        return true;
      }
      if (b >= 0.9) fill(out, pixels, [0, 0.6, 0]);
      else if (b < 0.15) fill(out, pixels, [0.7, 0, 0], 0, 1);
      else fill(out, pixels, [0.8, 0.35, 0], 0, Math.max(1, Math.round(b * pixels)));
      return true;
    }
    if (this.overlay?.kind === 'tap' || this.overlay?.kind === 'confirm') {
      const e = this.overlay.until - t;
      return fill(out, pixels, (e * 2.5) % 1 < 0.5 ? [1, 1, 1] : [0, 0, 0]), true;
    }
    return false;
  }

  display() {
    return null;
  }

  hint() {
    const st = this.f.state;
    if (this.addressing) {
      const prof = AX1_ADDRESSING[this.addressing.profile];
      return `Adressage DMX : profil ${prof.name} (${prof.label}), adresse ${1 + this.addressing.index * prof.step}. Appui court : adresse suivante · secouer : profil suivant · maintenir : valider.`;
    }
    if (!st.power)
      return st.charging
        ? this.gaugeKind === 'color'
          ? 'Éteint sur chargeur : la LED clignote lentement en vert (éteinte = chargé).'
          : 'Éteint sur chargeur : un pixel vert parcourt le tube (sombre = chargé).'
        : 'Éteint : maintenir 1 s pour allumer.';
    if (st.blueMode)
      return this.colorIdx >= 0
        ? `BlueMode : couleur ${AX1_BLUEMODE_COLORS[this.colorIdx][0]}. Appui court : couleur suivante · maintenir 1 s : valider.`
        : `BlueMode (clignote en bleu) : prêt pour « Pair with Lights ». Appui court : couleurs fixes${this.canShake ? ' · secouer : adressage DMX' : ''}.`;
    if (this.holdStage === 'off') return 'Relâcher maintenant = extinction';
    if (this.holdStage === 'blue') return 'Relâcher maintenant = BlueMode';
    if (this.holdStage === 'reset') return 'Reset effectué (rouge)';
    return '';
  }
}
