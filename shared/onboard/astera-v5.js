// Panneau de commande Astera « V5 » avec écran OLED (Titan / Helios / Hyperion, BTB, AX2).
// Clavier : ⏻, MENU, −, +, ENTER, palette, soleil (+ touche ASTERA sur l'AX2).
// Comportement d'après le Titan TRM 2020 et les guides 2022-2023 : docs/recherche/menus/tubes.md.
import { GEL_FAMILIES, LEE_GELS, STATIC_COLORS } from '../fixtures/astera-colors.js';
import { getMode } from '../fixtures/index.js';
import { resetSettings, standaloneSpeedSeconds } from '../engine/astera-light.js';

export const INPUT_SELECT = ['AUTO', 'STANDALONE', 'APP CONTROL', 'Wired DMX', 'ART3 DMX', 'CRMX wirel. DMX'];
const DIMMER_CURVES = ['FAST', 'STANDARD', 'HALOGEN', 'THEATER', 'TV'];
const DMX_FAIL = ['HOLD', 'EMERGENCY LIGHT', 'BLACKOUT', 'STANDALONE'];
const AC_FAILURE = ['NO ACTION', 'BLACKOUT', 'EMERGENCY LIGHT', 'POWEROFF'];
const RF_REGIONS = ['EU', 'USA', 'AUS', 'SGP', 'KOR', 'RUS', 'JPN'];
export const STANDALONE_PROGRAMS = [
  'ONE COLOR STATIC',
  'TWO COLOR STATIC',
  'THREE COLOR STATIC',
  'FOUR COLOR STATIC',
  'ONE COLOR FADE',
  'TWO COLOR FADE',
  'THREE COLOR FADE',
  'FOUR COLOR FADE',
  'SIMPLE RUNNING',
  'DOUBLE RUNNING',
  'TWO COL RUNNING',
  'FLAG RUNNING',
  'DOUBLE FLAG RUNNING',
  'SPIRAL FOUR COLORS',
  'SPIRAL TWO COLORS',
  'RAINBOW',
  'FIRE',
];
const COLOR_NAMES = STATIC_COLORS.map(([n]) => n);
const LIST_TITLES = { 'DMX SETTINGS': 'DMX Settings:', STANDALONE: 'Standalone:', 'CUSTOM COLOR': 'Custom Color:' };
const pad3 = (n) => String(n).padStart(3, '0');
const titleCase = (s) => s.charAt(0) + s.slice(1).toLowerCase();
const IDLE_TO_STATUS = 180; // s
const IDLE_DIM = 30; // s

/** Profils DMX disponibles pour un nombre de pixels et un mode stroboscope (les modes d'effet avec 1 pixel). */
function profilesFor(type, pixels, strobe) {
  return type.dmx.modes.filter((m) => (m.pixels === pixels && (m.strobe || 'OFF') === strobe) || (pixels === 1 && strobe === 'OFF' && m.pixels === 'effet'));
}

/** Nom « de base » d'un profil, sans marque de stroboscope ni répétition par pixel (« DIM RGBS DIM RGBS » -> « DIM RGB »). */
export function profileBaseName(name) {
  const words = String(name || '')
    .replace(/\s*\.\.\s*S+$/, '')
    .replace(/\s+S$/, '')
    .split(/\s+/)
    .map((w) => (/^[A-Z0-9.]+S$/.test(w) && w !== 'IND' ? w.slice(0, -1) : w));
  const n = words.length;
  const out = n % 2 === 0 && words.slice(0, n / 2).join(' ') === words.slice(n / 2).join(' ') ? words.slice(0, n / 2) : words;
  return out.join(' ').replace(/\.$/, '');
}

/** Recherche le profil équivalent quand PIXELS ou STROBE changent. */
function resolveProfile(type, pixels, strobe, preferName) {
  const list = profilesFor(type, pixels, strobe).filter((m) => m.pixels !== 'effet');
  if (!list.length) return null;
  const want = profileBaseName(preferName);
  return list.find((m) => profileBaseName(m.name) === want) || list[0];
}

export class AsteraV5Controller {
  constructor({ type, getFixture, emit, rt }) {
    this.type = type;
    this.getFixture = getFixture;
    this.emit = emit;
    this.rt = rt;
    this.ui = { screen: 'status' };
    this.pressed = new Map();
    this.lastKey = 0;
    this.lastT = 0;
    this.blueSince = null;
    this.message = null;
    this.cfg = type.controls || {};
    this.dual = this.cfg.keyfoil === 'dual'; // touches MENU/COLOR, −/SPEED, +/BRIGHT, ENTER/PROG (AX5, AX7, AX10)
  }

  // ---------------------------------------------------------------- utilitaires
  get f() {
    return this.getFixture();
  }
  get s() {
    return this.f.settings;
  }
  set(settings) {
    this.emit({ settings });
  }
  setState(state) {
    this.emit({ state });
  }
  ring() {
    if (this.cfg.ring) return this.cfg.ring;
    const r = ['INPUT SELECT', 'STATIC COLOR', 'DMX ADDRESS', 'DMX SETTINGS', 'UNLINK CRMX', 'RUNTIME', 'STANDALONE', 'AC FAILURE', 'INFO', 'RESET SETTINGS'];
    // UNBOND BLUETOOTH suit UNLINK CRMX (guides 2023) ; l'HydraPanel (guide 2022) montre l'ordre inverse
    if (this.type.btb) r.splice(this.type.ringOrder === 'unbondFirst' ? 4 : 5, 0, 'UNBOND BLUETOOTH');
    if (this.cfg.extraRing) r.splice(r.indexOf('DMX SETTINGS') + 1, 0, ...this.cfg.extraRing);
    return r;
  }
  mode() {
    return getMode(this.type, this.s.dmxMode);
  }
  latchStandalone() {
    if (['AUTO', 'STANDALONE'].includes(this.s.inputSelect)) this.rt.latch = 'STANDALONE';
  }
  setStandalone(patch) {
    this.set({ standalone: { ...this.s.standalone, ...patch } });
  }
  setColor0(spec) {
    const colors = [...(this.s.standalone.colors || [])];
    colors[0] = spec;
    this.setStandalone({ program: 0, colors });
  }
  show(lines, seconds, then) {
    this.message = { lines, until: this.lastT + seconds, then };
  }

  /** Définition éditable d'un élément de menu. */
  item(name) {
    const s = this.s;
    const sa = s.standalone || {};
    const m = this.mode();
    const num = (min, max, get, set, fmt = String) => ({ kind: 'num', min, max, get, set, fmt });
    const opt = (options, get, set, fmt = String) => ({ kind: 'opt', options, get, set, fmt });
    switch (name) {
      case 'INPUT SELECT':
        return opt(this.cfg.inputSelect || INPUT_SELECT, () => s.inputSelect, (v) => {
          this.rt.latch = null;
          this.set({ inputSelect: v });
        });
      case 'DMX ADDRESS':
        return num(1, 512, () => s.dmxAddress, (v) => this.set({ dmxAddress: v }), pad3);
      case 'RUNTIME':
        return num(1, 20, () => s.runtime, (v) => this.set({ runtime: v }), (v) => `${v}h`);
      case 'AC FAILURE':
        return opt(AC_FAILURE, () => s.acFailure, (v) => this.set({ acFailure: v }));
      case 'PIXELS':
        return opt(this.type.pixelOptions, () => (typeof m?.pixels === 'number' ? m.pixels : 1), (v) => {
          const cur = m?.strobe || 'OFF';
          const strobe = v === 1 ? (cur === 'OFF' ? 'OFF' : 'ON') : cur === 'ON' ? 'SINGLE' : cur;
          const r = resolveProfile(this.type, v, strobe, m?.name) || resolveProfile(this.type, v, 'OFF', m?.name);
          if (r) this.set({ dmxMode: r.id });
        });
      case 'DMX PROFILE': {
        const px = typeof m?.pixels === 'number' ? m.pixels : 1;
        const list = profilesFor(this.type, px, m?.strobe || 'OFF');
        return opt(list.map((p) => p.id), () => s.dmxMode, (v) => this.set({ dmxMode: v }), (id) => {
          const p = this.type.dmx.modes.find((x) => x.id === id);
          return p ? `${p.number}:${p.name}` : id;
        });
      }
      case 'STROBE': {
        const px = typeof m?.pixels === 'number' ? m.pixels : 1;
        const opts = px === 1 ? ['OFF', 'ON'] : ['OFF', 'SINGLE', 'MULTIPLE'];
        return opt(opts, () => m?.strobe || 'OFF', (v) => {
          const r = resolveProfile(this.type, px, v, m?.name);
          if (r) this.set({ dmxMode: r.id });
        });
      }
      case 'DIMMER CURVE':
        return opt(DIMMER_CURVES, () => s.dimmerCurve, (v) => this.set({ dimmerCurve: v }));
      case 'DMX FAIL':
        return opt(DMX_FAIL, () => s.dmxFail, (v) => this.set({ dmxFail: v }));
      case 'PROGRAM':
        return opt(STANDALONE_PROGRAMS.map((_, i) => i), () => sa.program, (v) => this.setStandalone({ program: v }), (i) => STANDALONE_PROGRAMS[i]);
      case 'INTENSITY':
        return num(0, 100, () => sa.intensity, (v) => this.setStandalone({ intensity: v }), (v) => `${v}%`);
      case 'SPEED':
        return num(0, 9, () => sa.speed, (v) => this.setStandalone({ speed: v }), (v) => `${v}:${standaloneSpeedSeconds(v).toFixed(2)}`);
      case 'FADE':
        return num(0, 100, () => sa.fade, (v) => this.setStandalone({ fade: v }), (v) => `${v}%`);
      case 'OUTPUT GAIN':
        return opt(['ON', 'OFF'], () => s.outputGain || 'ON', (v) => this.set({ outputGain: v }));
      case 'FAN': {
        // bascule entre un profil et sa variante « FAN » (canal ventilateur en plus)
        const cur = this.mode();
        return opt(['OFF', 'ON'], () => (cur?.fan ? 'ON' : 'OFF'), (v) => {
          if (!cur) return;
          const target = v === 'ON' ? this.type.dmx.modes.find((x) => x.baseId === cur.id) : this.type.dmx.modes.find((x) => x.id === cur.baseId);
          if (target) this.set({ dmxMode: target.id });
        });
      }
      case 'FAN CONTROL':
        return opt(['OFF', 'AUTO', 'LOW', 'MED', 'HIGH'], () => s.fan || 'AUTO', (v) => this.set({ fan: v }));
      case 'BRIGHTNESS':
        return num(0, 100, () => s.brightness ?? 100, (v) => this.set({ brightness: v }), (v) => `${v}%`);
      case 'COLOR 1':
      case 'COLOR 2':
      case 'COLOR 3':
      case 'COLOR 4': {
        const i = Number(name.slice(-1)) - 1;
        return opt(COLOR_NAMES, () => (sa.colors?.[i]?.kind === 'named' ? sa.colors[i].name : COLOR_NAMES[0]), (v) => {
          const colors = [...(sa.colors || [])];
          colors[i] = { kind: 'named', name: v };
          this.setStandalone({ colors });
        });
      }
      case 'SET ADDRESS':
        return num(0, 255, () => s.setAddress, (v) => this.set({ setAddress: v }));
      case 'FIXTURE NUMBER':
        return num(1, 255, () => s.fixtureNumber ?? 1, (v) => this.set({ fixtureNumber: v }));
      case 'SET SIZE':
        return num(1, 32, () => s.setSize, (v) => this.set({ setSize: v }));
      case 'POS IN SET':
        return num(1, 32, () => s.posInSet, (v) => this.set({ posInSet: v }));
      case 'GROUP ADDRESS':
        return num(1, 4, () => s.groupAddress, (v) => this.set({ groupAddress: v }));
      case 'INDEX COLOR':
        return num(0, 255, () => (sa.colors?.[0]?.kind === 'index' ? sa.colors[0].value : 0), (v) => this.setColor0({ kind: 'index', value: v }));
      case 'RED':
      case 'GREEN':
      case 'BLUE': {
        const ch = { RED: 0, GREEN: 1, BLUE: 2 }[name];
        const cur = () => (sa.colors?.[0]?.kind === 'custom' ? sa.colors[0].rgb : [0, 0, 0]);
        return num(0, 255, () => cur()[ch], (v) => {
          const rgb = [...cur()];
          rgb[ch] = v;
          this.setColor0({ kind: 'custom', rgb });
        });
      }
      case 'RF Region':
        return opt(RF_REGIONS, () => s.rfRegion || 'EU', (v) => this.set({ rfRegion: v }));
      case 'AX1 Compatibility':
        return opt([true, false], () => !!s.ax1Compat, (v) => this.set({ ax1Compat: v }), (v) => (v ? 'ENABLED' : 'DISABLED'));
      default:
        return null;
    }
  }

  /** Contenu des sous-menus. */
  subItems(entry) {
    switch (entry) {
      case 'DMX SETTINGS':
        if (this.cfg.dmxSettings) return this.cfg.dmxSettings;
        // FIXTURE NUMBER : vu sur l'HydraPanel (« DMX Settings: / FIXTURE NUMBER »), position et plage déduites
        return ['PIXELS', 'DMX PROFILE', 'STROBE', 'DIMMER CURVE', 'DMX FAIL', 'FIXTURE NUMBER'];
      case 'STANDALONE':
        return ['PROGRAM', 'INTENSITY', 'SPEED', 'FADE', 'COLOR 1', 'COLOR 2', 'COLOR 3', 'COLOR 4', 'SET ADDRESS', 'SET SIZE', 'POS IN SET', 'GROUP ADDRESS'];
      case 'STATIC COLOR':
        return [...COLOR_NAMES, 'INDEX COLOR', 'CUSTOM COLOR'];
      case 'CUSTOM COLOR':
        return ['RED', 'GREEN', 'BLUE'];
      case 'INFO':
        return this.cfg.infoItems || ['Radio PIN', 'RF Region', 'AX1 Compatibility', 'Serial number', 'Firmware version', 'Power-on hours', 'RF link', 'CRMX Version', 'Battery state', 'Battery Cycles', 'Calibration'];
      default:
        return [];
    }
  }

  infoValue(name) {
    const f = this.f;
    const s = f.settings;
    const serialTail = String(parseInt(f.id?.slice(-6) || '0', 36) % 10000).padStart(4, '0');
    switch (name) {
      case 'Radio PIN':
        return s.radioPin || '0000';
      case 'Serial number':
        return `000-00000 ${serialTail}`;
      case 'Firmware version':
        return '5.16.24.U  HW001';
      case 'Power-on hours':
        return '00042h';
      case 'RF link':
        return '-36.0dBm -0.0ppm';
      case 'CRMX Version':
        return 'V1.0.5.0';
      case 'Battery state':
        return `${Math.round(f.state.battery ?? 100)}%`;
      case 'Battery Cycles':
        return '012';
      case 'Calibration':
        return '2024-03-12-0001';
      default:
        return '';
    }
  }

  // ---------------------------------------------------------------- touches
  press(id, t) {
    this.lastT = t;
    this.lastKey = t;
    this.pressed.set(id, { t0: t, fired: new Set(), nextRepeat: t + 0.5 });
    if (id === 'power') return;
    if (!this.f.state.power) return;
    if (this.dual && this.ui.screen === 'status' && ['menu', 'minus', 'plus', 'enter'].includes(id)) {
      this.pressed.get(id).deferred = true; // décidé au relâchement (court) ou au bout de 2 s (maintien)
      return;
    }
    this.key(id);
  }

  release(id, t) {
    this.lastT = t;
    const p = this.pressed.get(id);
    this.pressed.delete(id);
    if (p?.deferred && !p.fired.size) {
      // le maintien est aussi vérifié au relâchement (si l'animation n'a pas tourné entre-temps)
      if (t - p.t0 >= 2) this.holdAction(id);
      else this.key(id);
      return;
    }
    if (!p || id !== 'power') return;
    const held = t - p.t0;
    const st = this.f.state;
    if (!p.fired.size) {
      if (!st.power && held >= 1) {
        this.setState({ power: true });
        this.ui = { screen: 'status' };
        this.rt.latch = null;
        return;
      }
      if (st.power && held >= 3) {
        this.setState({ blueMode: true });
        this.blueSince = t;
        return;
      }
    }
    if (p.fired.size) return;
    if (st.power && held < 3) {
      if (st.blueMode) {
        this.setState({ blueMode: false });
        this.blueSince = null;
      } else {
        this.setState({ power: false });
        this.ui = { screen: 'status' };
      }
    }
  }

  tick(t) {
    this.lastT = t;
    const st = this.f.state;
    for (const [id, p] of this.pressed) {
      const held = t - p.t0;
      if (id === 'power') {
        if (!st.power && held >= 1 && !p.fired.has('on')) {
          p.fired.add('on');
          this.setState({ power: true });
          this.ui = { screen: 'status' };
          this.rt.latch = null;
        } else if (st.power && held >= 3 && !p.fired.size) {
          p.fired.add('blue');
          this.setState({ blueMode: true });
          this.blueSince = t;
        }
      } else if (p.deferred) {
        if (held >= 2 && !p.fired.size) {
          p.fired.add('hold');
          this.holdAction(id);
        }
      } else if ((id === 'plus' || id === 'minus') && t >= p.nextRepeat && st.power) {
        // répétition automatique, accélérée après 2 s
        p.nextRepeat = t + 0.08;
        this.key(id, held > 2 ? 10 : 1);
      }
    }
    if (st.blueMode && this.blueSince && t - this.blueSince > 120) {
      this.setState({ blueMode: false });
      this.blueSince = null;
    }
    if (this.message && t > this.message.until) {
      const then = this.message.then;
      this.message = null;
      then?.();
    }
    if (this.ui.screen !== 'status' && t - this.lastKey > IDLE_TO_STATUS) this.ui = { screen: 'status' };
  }

  heldFor(id, t = this.lastT) {
    const p = this.pressed.get(id);
    return p ? t - p.t0 : 0;
  }

  /** Action d'une touche selon l'écran affiché. `step` = pas des touches +/− (répétition). */
  key(id, step = 1) {
    if (this.message) return;
    const st = this.f.state;
    const ui = this.ui;
    const dir = id === 'plus' ? 1 : id === 'minus' ? -1 : 0;

    // Touches directes
    if (id === 'palette') {
      this.latchStandalone();
      const order = ['hsi', 'static', 'gel'];
      const next = order[(order.indexOf(ui.screen) + 1) % order.length];
      this.openColorPage(next);
      return;
    }
    if (id === 'sun') {
      this.ui = { screen: 'bright', line: 0 };
      return;
    }
    if (id === 'astera') {
      if (this.cfg.asteraKey === 'tilt') {
        // Quik : la touche ASTERA bascule l'affichage de l'inclinaison (accéléromètre)
        this.ui = ui.screen === 'tilt' ? { screen: 'status' } : { screen: 'tilt' };
      } else this.show(['AUTO ADDRESS', 'non documenté'], 1.5);
      return;
    }
    if (ui.screen === 'tilt') return; // autres touches ignorées pendant l'affichage de l'inclinaison (non documenté)

    switch (ui.screen) {
      case 'status': {
        if (id === 'enter') {
          if (st.acceptConfig) return this.setState({ acceptConfig: false });
          if (st.standby) return this.setState({ standby: false });
          this.ui = { screen: 'ring', i: this.ring().indexOf('DMX ADDRESS') };
        } else if (id === 'menu' || id === 'plus') this.ui = { screen: 'ring', i: 0 };
        else if (id === 'minus') this.ui = { screen: 'ring', i: this.ring().length - 1 };
        return;
      }
      case 'ring': {
        const ring = this.ring();
        if (id === 'menu') this.ui = { screen: 'status' };
        else if (dir) {
          const i = ui.i + dir;
          this.ui = i < 0 || i >= ring.length ? { screen: 'status' } : { screen: 'ring', i };
        } else if (id === 'enter') this.openEntry(ring[ui.i]);
        return;
      }
      case 'value':
      case 'subvalue': {
        const it = ui.def;
        if (id === 'menu') return this.back();
        if (id === 'enter') {
          it.set(ui.value);
          this.back();
          return;
        }
        if (dir) {
          ui.value = stepValue(it, ui.value, dir * step);
          if (ui.live) it.set(ui.value);
        }
        return;
      }
      case 'list': {
        const items = this.subItems(ui.entry);
        if (id === 'menu') return this.back();
        if (dir) {
          ui.i = (ui.i + dir + items.length) % items.length;
          if (ui.entry === 'STATIC COLOR' && COLOR_NAMES.includes(items[ui.i])) this.setColor0({ kind: 'named', name: items[ui.i] });
          return;
        }
        if (id === 'enter') return this.openSubItem(ui.entry, items[ui.i]);
        return;
      }
      case 'pin': {
        if (id === 'menu') {
          ui.pos = (ui.pos + 1) % ui.digits.length;
          return;
        }
        if (id === 'enter') {
          this.set({ [ui.key || 'radioPin']: ui.digits.join('') });
          return this.back();
        }
        if (dir) ui.digits[ui.pos] = (ui.digits[ui.pos] + dir + 10) % 10;
        return;
      }
      case 'confirm': {
        if (this.cfg.resetChoice && dir) {
          ui.yes = !ui.yes; // firmware 2018-2020 : choix NO / YES avec + / −
          return;
        }
        if (id === 'enter' && this.cfg.resetChoice && !ui.yes) {
          this.ui = { screen: 'ring', i: this.ring().indexOf('RESET SETTINGS') };
          return;
        }
        if (id === 'enter') {
          this.set(resetSettings(this.type, this.s));
          this.rt.latch = null;
          this.rt.justUnlinked = false;
          this.ui = { screen: 'status' };
        } else if (id === 'menu') this.ui = { screen: 'ring', i: this.ring().indexOf('RESET SETTINGS') };
        return;
      }
      case 'hsi': {
        if (id === 'menu') return (this.ui = { screen: 'status' });
        if (id === 'enter') return (ui.line = (ui.line + 1) % 4);
        if (dir) {
          const c = { ...ui.color };
          if (ui.line === 0) c.cct = clamp(c.cct + dir * 50 * step, 1750, 20000);
          else if (ui.line === 1) c.green = clamp(Math.round((c.green + dir * 0.05 * step) * 100) / 100, -1, 1);
          else if (ui.line === 2) c.hue = (((c.hue + dir * step) % 361) + 361) % 361;
          else c.sat = clamp(c.sat + dir * step, 0, 100);
          ui.color = c;
          this.setColor0({ kind: 'hsi', ...c });
        }
        return;
      }
      case 'static': {
        if (id === 'menu') return (this.ui = { screen: 'status' });
        if (dir) {
          const names = COLOR_NAMES.filter((n) => n !== 'BLACK');
          ui.i = (ui.i + dir + names.length) % names.length;
          this.setColor0({ kind: 'named', name: names[ui.i] });
        }
        return;
      }
      case 'gel': {
        if (id === 'menu') return (this.ui = { screen: 'status' });
        if (id === 'enter') return (ui.line = ui.line ? 0 : 1);
        if (dir) {
          if (ui.line === 0) ui.family = (ui.family + dir + GEL_FAMILIES.length) % GEL_FAMILIES.length;
          else ui.gel = (ui.gel + dir * step + LEE_GELS.length * 10) % LEE_GELS.length;
          this.setColor0({ kind: 'gel', family: GEL_FAMILIES[ui.family], number: LEE_GELS[ui.gel].number });
        }
        return;
      }
      case 'bright': {
        if (id === 'menu') return (this.ui = { screen: 'status' });
        if (id === 'enter') return (ui.line = ui.line ? 0 : 1);
        if (dir) {
          if (ui.line === 0) this.set({ brightness: clamp((this.s.brightness ?? 100) + dir * step, 0, 100) });
          else this.set({ runtime: clamp((this.s.runtime ?? 5) + dir, 1, 20) });
        }
        return;
      }
      default:
        this.ui = { screen: 'status' };
    }
  }

  /** Maintien 2 s sur l'écran d'état (clavier à double légende) : COLOR, SPEED, BRIGHT, PROG. */
  holdAction(id) {
    this.lastKey = this.lastT;
    this.latchStandalone();
    const open = (name) => {
      const def = this.item(name);
      this.ui = { screen: 'subvalue', entry: 'STANDALONE', name, def, value: def.get(), parent: { screen: 'status' }, live: true };
    };
    if (id === 'menu') this.ui = { screen: 'list', entry: 'STANDALONE', i: 0, parent: { screen: 'status' } };
    else if (id === 'minus') open('SPEED');
    else if (id === 'plus') open('BRIGHTNESS');
    else if (id === 'enter') open('PROGRAM');
  }

  /** TouchSlider (HydraPanel) : actif seulement sur les pages soleil / palette, DMX ADDRESS et FIXTURE NUMBER. */
  sliderActive() {
    const ui = this.ui;
    return (
      ['bright', 'hsi', 'static', 'gel'].includes(ui.screen) ||
      (ui.screen === 'value' && ui.entry === 'DMX ADDRESS') ||
      (ui.screen === 'subvalue' && ui.name === 'FIXTURE NUMBER')
    );
  }

  /** zone : 'top' (+1), 'middle' (ENTER), 'bottom' (−1) ; steps > 1 pour un glissé. */
  slider(zone, t, steps = 1) {
    this.lastT = t;
    this.lastKey = t;
    if (!this.f.state.power || !this.sliderActive()) return false;
    if (zone === 'middle') this.key('enter');
    else for (let i = 0; i < steps; i++) this.key(zone === 'top' ? 'plus' : 'minus');
    return true;
  }

  back() {
    const parent = this.ui.parent;
    this.ui = parent || { screen: 'status' };
  }

  openEntry(entry) {
    const parent = { ...this.ui };
    switch (entry) {
      case 'UNPAIR CRMX':
      case 'UNLINK CRMX':
        this.show([entry === 'UNPAIR CRMX' ? 'UNPAIRING...' : 'UNLINKING...', ''], 1.2, () => {
          this.set({ crmxLinked: false });
          this.rt.latch = null;
          this.rt.justUnlinked = true;
          this.ui = { screen: 'status' };
        });
        return;
      case 'UNBOND BLUETOOTH':
        this.show(['UNBONDING...', ''], 1.2, () => {
          this.set({ btbBonded: false });
          this.ui = { screen: 'status' };
        });
        return;
      case 'RESET SETTINGS':
        this.ui = { screen: 'confirm', parent };
        return;
      case 'STATIC COLOR':
        this.latchStandalone();
        this.setColor0({ kind: 'named', name: 'RED' });
        this.ui = { screen: 'list', entry, i: 0, parent };
        return;
      case 'DMX SETTINGS':
      case 'STANDALONE':
      case 'INFO':
        this.ui = { screen: 'list', entry, i: 0, parent };
        return;
      default: {
        const def = this.item(entry);
        if (def) this.ui = { screen: 'value', entry, def, value: def.get(), parent };
      }
    }
  }

  openSubItem(entry, name) {
    const parent = { ...this.ui };
    if (name === 'CUSTOM COLOR') {
      this.ui = { screen: 'list', entry: 'CUSTOM COLOR', i: 0, parent };
      return;
    }
    if (name === 'Radio PIN') {
      this.ui = { screen: 'pin', key: 'radioPin', title: 'Radio PIN:', digits: (this.s.radioPin || '0000').split('').map(Number), pos: 0, parent };
      return;
    }
    if (name === 'CRMX LINK BY KEY') {
      // clé de liaison CRMX à 8 chiffres (même édition chiffre par chiffre que le Radio PIN)
      this.ui = { screen: 'pin', key: 'linkKey', title: 'Link Key:', digits: (this.s.linkKey || '00000000').split('').map(Number), pos: 0, parent };
      return;
    }
    if (entry === 'STATIC COLOR' && COLOR_NAMES.includes(name)) {
      this.back();
      return;
    }
    const def = this.item(name);
    if (!def) return; // information en lecture seule
    const live = entry === 'STATIC COLOR' || entry === 'CUSTOM COLOR';
    this.ui = { screen: 'subvalue', entry, name, def, value: def.get(), parent, live };
  }

  openColorPage(page) {
    const c0 = this.s.standalone?.colors?.[0] || {};
    if (page === 'hsi') {
      const color = c0.kind === 'hsi' ? { cct: c0.cct, green: c0.green, hue: c0.hue, sat: c0.sat } : { cct: 5600, green: 0, hue: 0, sat: 0 };
      this.ui = { screen: 'hsi', line: 0, color };
      this.setColor0({ kind: 'hsi', ...color });
    } else if (page === 'static') {
      const names = COLOR_NAMES.filter((n) => n !== 'BLACK');
      const i = Math.max(0, names.indexOf(c0.kind === 'named' ? c0.name : 'RED'));
      this.ui = { screen: 'static', i };
      this.setColor0({ kind: 'named', name: names[i] });
    } else {
      const gel = c0.kind === 'gel' ? Math.max(0, LEE_GELS.findIndex((g) => g.number === c0.number)) : 0;
      const family = c0.kind === 'gel' ? Math.max(0, GEL_FAMILIES.indexOf(c0.family)) : 0;
      this.ui = { screen: 'gel', line: 1, family, gel };
      this.setColor0({ kind: 'gel', family: GEL_FAMILIES[family], number: LEE_GELS[gel].number });
    }
  }

  // ---------------------------------------------------------------- affichage
  statusLine() {
    const f = this.f;
    const st = f.state;
    const rt = this.rt;
    if (st.acceptConfig) return this.cfg.acceptText || 'ACCEPT CONFIG ?';
    if (st.standby) return 'STANDBY';
    switch (rt.input) {
      case 'CRMX':
        return !f.settings.crmxLinked ? `CRMX: NOT ${this.cfg.crmxWord || 'LINKED'}` : rt.signal ? 'CRMX: SIGNAL 99%' : 'CRMX: NO DMX';
      case 'WIRED':
        return rt.signal ? 'XLR-DMX: LINK OK' : 'XLR-DMX: NO DMX';
      case 'APP':
        return 'APP CONTROL';
      case 'REMOTE':
        return 'REMOTE CONTROL';
      case 'ART3':
        return 'ART3: NO LINK';
      case 'STANDALONE':
        return 'STANDALONE';
      default:
        if (rt.src === 'emergency') return 'EMERGENCY LIGHT';
        if (f.settings.crmxLinked) return 'CRMX: NO DMX';
        return rt.justUnlinked ? `CRMX: NOT ${this.cfg.crmxWord || 'LINKED'}` : 'INPUT: DETECTING';
    }
  }

  /**
   * Modèle d'affichage : { off, dim, icons, lines: [{text, inverse}], size: 'big'|'small' }
   */
  display(t = this.lastT) {
    const f = this.f;
    const st = f.state;
    const s = f.settings;
    const icons = {
      battery: Math.round(st.battery ?? 100),
      plug: !!st.charging,
      bt: this.type.btb && s.btbBonded !== false,
      btWaves: !!this.rt.appConnected,
      bars: this.cfg.display?.kind === 'lcd',
    };
    if (!st.power) {
      if (!st.charging) return { off: true };
      const b = Math.round(st.battery ?? 100);
      const bar = `[${'='.repeat(Math.round(b / 10)).padEnd(10, ' ')}]${String(b).padStart(3, ' ')}%`;
      return { icons: null, size: 'big', lines: [{ text: b >= 100 ? 'Fully Charged' : 'Charging...' }, { text: bar }] };
    }
    const dim = t - this.lastKey > IDLE_DIM;
    if (this.message) return { dim: false, icons, size: 'big', lines: this.message.lines.map((text) => ({ text })) };
    const ui = this.ui;
    const big = (a, b, inv2 = false) => ({ dim, icons, size: 'big', lines: [{ text: a }, { text: b, inverse: inv2 }] });
    switch (ui.screen) {
      case 'ring':
        return big('Main menu:', this.ring()[ui.i]);
      case 'value':
        return big(`${ui.entry}:`, ui.def.fmt(ui.value), true);
      case 'subvalue':
        return big(`${ui.name}:`, ui.def.fmt(ui.value), true);
      case 'list': {
        const name = this.subItems(ui.entry)[ui.i];
        if (ui.entry === 'INFO') {
          const def = this.item(name);
          return big(`${name}:`, def ? def.fmt(def.get()) : this.infoValue(name));
        }
        if (ui.entry === 'STATIC COLOR') return big('Static Color:', name);
        // Format vu sur l'HydraPanel : « DMX Settings: » / « FIXTURE NUMBER »
        return big(LIST_TITLES[ui.entry] || `${ui.entry}:`, this.cfg.itemLabels?.[name] || name);
      }
      case 'pin':
        return big(ui.title || 'Radio PIN:', ui.digits.map((d, i) => (i === ui.pos && (t * 2) % 1 < 0.5 ? '_' : d)).join(''));
      case 'confirm':
        if (this.cfg.resetChoice) return big('ARE YOU SURE?', ui.yes ? ' NO  [YES]' : '[NO]  YES ');
        return big(...(this.cfg.resetConfirm || ['ARE YOU SURE ?', 'ENTER FOR YES']));
      case 'hsi': {
        const c = ui.color;
        const rows = [`CCT    : ${c.cct} K`, `Green  : ${c.green.toFixed(2)}`, `Hue    : ${c.hue}`, `Sat    : ${c.sat}%`];
        return { dim, icons: null, size: 'small', lines: rows.map((text, i) => ({ text, inverse: i === ui.line })) };
      }
      case 'static': {
        const names = COLOR_NAMES.filter((n) => n !== 'BLACK');
        return big('Static Color:', titleCase(names[ui.i]));
      }
      case 'gel': {
        const g = LEE_GELS[ui.gel];
        return {
          dim,
          icons: null,
          size: 'small',
          lines: [{ text: 'Color Gel:' }, { text: GEL_FAMILIES[ui.family], inverse: ui.line === 0 }, { text: pad3(g.number), inverse: ui.line === 1 }, { text: g.name }],
        };
      }
      case 'tilt': {
        // inclinaison de l'axe du faisceau par rapport à l'horizontale (format d'écran non publié)
        const [rx, ry] = f.rotation || [0, 0, 0];
        const tilt = (Math.asin(Math.max(-1, Math.min(1, -Math.cos(ry) * Math.sin(rx)))) * 180) / Math.PI;
        return big('TILT', `${tilt >= 0 ? '+' : ''}${tilt.toFixed(1)}°`);
      }
      case 'bright':
        return {
          dim,
          icons: null,
          size: 'small',
          lines: [
            { text: `Brigntness: ${s.brightness ?? 100}%`, inverse: ui.line === 0 },
            { text: `Runtime:   ${s.runtime ?? 5}h`, inverse: ui.line === 1 },
          ],
        };
      default:
        return big(this.statusLine(), `SET:${s.setAddress ? pad3(s.setAddress) : '---'}  DMX:${pad3(s.dmxAddress)}`);
    }
  }

  /** Voyant d'état de la batterie (claviers à double légende) : bleu sur secteur, sinon vert / orange / rouge. */
  leds() {
    if (!this.cfg.batteryLed) return {};
    const st = this.f.state;
    if (st.charging) return { battery: { color: '#2f7bff' } };
    if (!st.power) return {};
    const b = st.battery ?? 100;
    return { battery: { color: b < 15 ? '#ff4d4d' : b < 90 ? '#ff9a2e' : '#3ddc84' } };
  }

  /** Les tubes n'ont pas d'animation de pixels propre au panneau (le BlueMode est géré par le moteur). */
  overridePixels() {
    return false;
  }

  hint() {
    const st = this.f.state;
    if (!st.power) return st.charging ? 'Éteint, en charge (l’écran affiche la charge).' : 'Éteint : maintenir ⏻ 1 s pour allumer.';
    if (st.blueMode) return 'BlueMode : le tube clignote en bleu, prêt à recevoir le Radio PIN (« Pair with Lights » dans l’appli).';
    return '';
  }
}

function clamp(v, a, b) {
  return Math.max(a, Math.min(b, v));
}

function stepValue(def, value, delta) {
  if (def.kind === 'num') {
    let v = Number(value) + delta;
    if (v > def.max) v = Math.abs(delta) > 1 ? def.max : def.min;
    if (v < def.min) v = Math.abs(delta) > 1 ? def.min : def.max;
    return v;
  }
  const opts = def.options;
  const i = opts.findIndex((o) => o === value);
  const d = Math.sign(delta);
  return opts[(Math.max(0, i) + d + opts.length) % opts.length];
}
