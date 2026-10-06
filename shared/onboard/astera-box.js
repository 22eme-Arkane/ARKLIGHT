// AsteraBox ART7 / ART7-WIFI : face supérieure (bouton marche, touche verte CRMX LINK, voyants
// APP / LINK / DMX / RX / TX, jauge de batterie 5 segments). Pas d'écran.
// D'après le manuel ART7 v3.0 (2022) : docs/recherche/menus/box_remotes.md.
import { isAsteraV5 } from '../engine/output.js';
import { getFixtureType } from '../fixtures/index.js';

const LINK_SECONDS = 10;

export class AsteraBoxController {
  /**
   * @param {object} opts
   * @param {() => object[]} opts.getAll        tous les projecteurs de la scène
   * @param {(id:string, changes:object) => void} opts.emitTo
   * @param {() => {appConnected:boolean, dmxLive:(u:number)=>boolean}} opts.env
   */
  constructor({ type, getFixture, emit, rt, getAll, emitTo, env }) {
    this.type = type;
    this.getFixture = getFixture;
    this.emit = emit;
    this.rt = rt;
    this.getAll = getAll;
    this.emitTo = emitTo;
    this.env = env;
    this.pressed = new Map();
    this.lastT = 0;
    this.gaugeUntil = 0;
    this.linking = null; // { until }
    this.unlinking = null;
    this.modeSelect = null; // { pending: 'TX'|'RX' }
    this.linkPresses = []; // horodatage des appuis (5 appuis + maintien)
  }

  get f() {
    return this.getFixture();
  }

  press(id, t) {
    this.lastT = t;
    this.pressed.set(id, { t0: t, fired: false });
    if (id === 'link') {
      this.linkPresses = this.linkPresses.filter((x) => t - x < 3).concat(t);
    }
  }

  release(id, t) {
    this.lastT = t;
    const p = this.pressed.get(id);
    this.pressed.delete(id);
    if (!p) return;
    const held = t - p.t0;
    const f = this.f;
    if (id === 'power') {
      if (p.fired) return;
      if (held >= 1) return this.emit({ state: { power: !f.state.power } });
      if (f.state.power) this.gaugeUntil = t + 2;
      else if (f.state.charging) this.gaugeUntil = t + 2;
      return;
    }
    if (id === 'link' && f.state.power && !p.fired && held < 3) {
      if (this.modeSelect) {
        this.modeSelect.pending = this.modeSelect.pending === 'TX' ? 'RX' : 'TX';
        return;
      }
      if ((f.settings.crmxMode || 'TX') === 'TX') this.startLinking(t);
    }
  }

  tick(t) {
    this.lastT = t;
    const f = this.f;
    for (const [id, p] of this.pressed) {
      const held = t - p.t0;
      if (id === 'power' && held >= 1 && !p.fired) {
        p.fired = true;
        this.emit({ state: { power: !f.state.power } });
      }
      if (id === 'link' && held >= 3 && !p.fired && f.state.power) {
        p.fired = true;
        if (this.modeSelect) {
          // validation du mode choisi
          this.emit({ settings: { crmxMode: this.modeSelect.pending } });
          this.modeSelect = null;
        } else if (this.linkPresses.length >= 5) {
          this.modeSelect = { pending: f.settings.crmxMode || 'TX' };
        } else if ((f.settings.crmxMode || 'TX') === 'TX') {
          this.unlinkAll(t);
        } else {
          this.emit({ settings: { rxLinked: false } });
        }
        this.linkPresses = [];
      }
    }
    if (this.linking) {
      // Les projecteurs déliés, en AUTO (détection) ou en CRMX, se lient pendant la fenêtre de liaison
      this.linkCandidates();
      if (t > this.linking.until) this.linking = null;
    }
    if (this.unlinking && t > this.unlinking.until) this.unlinking = null;
  }

  startLinking(t) {
    this.linking = { until: t + LINK_SECONDS };
  }

  linkCandidates() {
    const u = this.f.settings.universe;
    for (const other of this.getAll()) {
      const type = getFixtureType(other.type);
      if (!type || !isAsteraV5(type) || other.settings.crmxLinked || !other.state.power) continue;
      const sel = other.settings.inputSelect || 'AUTO';
      if (sel !== 'AUTO' && sel !== 'CRMX wirel. DMX') continue;
      this.emitTo(other.id, { settings: { crmxLinked: true, universe: u } }, { relatch: true });
    }
  }

  unlinkAll(t) {
    const u = this.f.settings.universe;
    for (const other of this.getAll()) {
      const type = getFixtureType(other.type);
      if (!type || !isAsteraV5(type) || !other.settings.crmxLinked || other.settings.universe !== u || !other.state.power) continue;
      this.emitTo(other.id, { settings: { crmxLinked: false } }, { relatch: true, unlinked: true });
    }
    this.unlinking = { until: t + 4 };
  }

  heldFor(id, t = this.lastT) {
    const p = this.pressed.get(id);
    return p ? t - p.t0 : 0;
  }

  /** État des voyants : { [id]: { color, blinkHz } } */
  leds(t = this.lastT) {
    const f = this.f;
    const st = f.state;
    const out = {};
    const battery = st.battery ?? 100;
    const showGauge = (st.power && t < this.gaugeUntil) || (!st.power && st.charging && battery < 100);
    for (let i = 0; i < 5; i++) out[`bat${i}`] = showGauge && battery > i * 20 ? { color: '#f2f5f7' } : null;
    if (!st.power) return out;
    const env = this.env();
    out.app = { color: '#2f7bff', blinkHz: env.appConnected ? 1.5 : 0.5 };
    const tx = (f.settings.crmxMode || 'TX') === 'TX';
    const cable = st.dmxCable !== false;
    if (!cable) return out;
    if (this.modeSelect) {
      const phase = (t * 2) % 1 < 0.5;
      out.link = phase ? { color: '#33e07a' } : null;
      out.dmx = phase ? null : { color: '#f2f5f7' };
      const pendTx = this.modeSelect.pending === 'TX';
      out.tx = pendTx ? { color: '#2f7bff' } : { color: '#f2f5f7' };
      out.rx = pendTx ? { color: '#f2f5f7' } : { color: '#ff9a2e' };
      return out;
    }
    if (tx) {
      out.tx = { color: '#2f7bff' };
      out.link = this.linking ? { color: '#33e07a', blinkHz: 3 } : this.unlinking ? { color: '#33e07a', blinkHz: 1 } : { color: '#33e07a' };
      out.dmx = env.dmxLive(f.settings.universe) ? { color: '#33e07a' } : null;
    } else {
      out.rx = { color: '#ff9a2e' };
      out.link = f.settings.rxLinked ? { color: '#33e07a' } : null;
    }
    return out;
  }

  display() {
    return null;
  }

  overridePixels() {
    return false;
  }

  hint() {
    const f = this.f;
    if (!f.state.power) return 'Éteinte : maintenir ⏻ 1 s pour allumer (appui court : niveau de batterie).';
    if (this.modeSelect) return `Choix du mode : ${this.modeSelect.pending} (appui court : changer · maintenir 3 s : valider).`;
    if (this.linking) return `Liaison CRMX en cours (≈ ${Math.max(0, Math.ceil(this.linking.until - this.lastT))} s) : les projecteurs déliés en AUTO se lient à l’univers ${f.settings.universe}.`;
    if (this.unlinking) return 'Déliaison : tous les projecteurs liés à cette box sont déliés.';
    return `${(f.settings.crmxMode || 'TX') === 'TX' ? 'Émetteur CRMX (TX)' : 'Récepteur CRMX (RX)'} · entrée DMX = univers ${f.settings.universe} · voyant APP : ${this.env().appConnected ? 'appli connectée' : 'en attente de l’appli'}.`;
  }
}
