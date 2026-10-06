// Moteur des commandes embarquées d'un projecteur (boutons, voyants, écran, menus).
// Logique pure, sans DOM : utilisée par le panneau de commande de l'interface et par la vue 3D
// (retours visuels sur les pixels : jauge de batterie, clignotement d'identification, BlueMode…).
//
// Une définition `controls` (dans la bibliothèque de projecteurs) décrit :
//   buttons : [{ id, label, title }]
//   leds    : [{ id, label }]
//   display : { width, height } | null       (écran OLED/LCD simulé en pixels)
//   menu    : arbre de menus (si écran)       — voir MenuNavigator
//   actions : { [bouton]: { short, long, hold3, ... } } — gestes -> actions
//   holdTimes : { long: 1, hold3: 3, hold5: 5 } en secondes

export const DEFAULT_HOLD_TIMES = { long: 1, hold3: 3, hold5: 5 };

/** Animation de retour visuel jouée sur les pixels du projecteur. */
export class PixelAnimation {
  constructor(kind, { duration = 3, color = [0, 0.2, 1], level = 1, hz = 2 } = {}) {
    this.kind = kind;
    this.duration = duration;
    this.color = color;
    this.level = level;
    this.hz = hz;
    this.start = null;
  }

  /** @returns {boolean} true si l'animation est terminée */
  apply(out, pixels, t) {
    if (this.start === null) this.start = t;
    const e = t - this.start;
    if (e > this.duration) return true;
    const [r, g, b] = this.color;
    if (this.kind === 'gauge') {
      // Jauge : n pixels allumés proportionnellement au niveau (batterie...)
      const lit = Math.max(1, Math.round(this.level * pixels));
      const col = this.level > 0.5 ? [0, 1, 0] : this.level > 0.2 ? [1, 0.6, 0] : [1, 0, 0];
      for (let p = 0; p < pixels; p++) {
        const on = p < lit ? 0.6 : 0;
        out[p * 3] = col[0] * on;
        out[p * 3 + 1] = col[1] * on;
        out[p * 3 + 2] = col[2] * on;
      }
    } else if (this.kind === 'blink') {
      const on = (e * this.hz) % 1 < 0.5 ? 1 : 0;
      for (let p = 0; p < pixels; p++) {
        out[p * 3] = r * on;
        out[p * 3 + 1] = g * on;
        out[p * 3 + 2] = b * on;
      }
    } else if (this.kind === 'pulse') {
      const k = 0.5 - 0.5 * Math.cos(e * this.hz * 2 * Math.PI);
      for (let p = 0; p < pixels; p++) {
        out[p * 3] = r * k;
        out[p * 3 + 1] = g * k;
        out[p * 3 + 2] = b * k;
      }
    }
    return false;
  }
}

/** Navigation dans un arbre de menus affiché sur un petit écran. */
export class MenuNavigator {
  constructor(tree, ctx) {
    this.tree = tree;
    this.ctx = ctx; // { type, getFixture(), setSetting(key, value), runAction(name) }
    this.stack = [];
    this.editing = null; // { item, value }
    this.cursor = 0;
    this.open = false;
  }

  items() {
    const node = this.stack.length ? this.stack[this.stack.length - 1].item.children : this.tree;
    return node.filter((it) => !it.when || it.when(this.ctx.getFixture(), this.ctx.type));
  }

  current() {
    return this.items()[this.cursor];
  }

  enter() {
    if (!this.open) {
      this.open = true;
      this.cursor = 0;
      return;
    }
    if (this.editing) {
      this.ctx.setSetting(this.editing.item.setting, this.editing.value);
      this.editing = null;
      return;
    }
    const it = this.current();
    if (!it) return;
    if (it.children) {
      this.stack.push({ item: it, cursor: this.cursor });
      this.cursor = 0;
    } else if (it.action) {
      this.ctx.runAction(it.action, it);
    } else if (it.setting) {
      this.editing = { item: it, value: this.ctx.getFixture().settings[it.setting] };
    }
  }

  back() {
    if (this.editing) this.editing = null;
    else if (this.stack.length) this.cursor = this.stack.pop().cursor;
    else this.open = false;
  }

  move(delta) {
    if (!this.open) return;
    if (this.editing) {
      this.editing.value = stepValue(this.editing.item, this.editing.value, delta, this.ctx);
      return;
    }
    const n = this.items().length;
    if (n) this.cursor = (this.cursor + delta + n) % n;
  }

  /** Lignes de texte à afficher (le panneau ou la vue 3D les dessinent). */
  lines(maxLines = 6) {
    const f = this.ctx.getFixture();
    if (!this.open) return this.ctx.homeLines ? this.ctx.homeLines(f) : [];
    const title = this.stack.length ? this.stack[this.stack.length - 1].item.label : 'MENU';
    if (this.editing) {
      const it = this.editing.item;
      return [{ text: it.label, inverse: true }, { text: '' }, { text: `> ${formatValue(it, this.editing.value, this.ctx)} <`, big: true }, { text: '' }, { text: 'OK = valider' }];
    }
    const items = this.items();
    const out = [{ text: title, inverse: true }];
    const visible = maxLines - 1;
    const first = Math.max(0, Math.min(this.cursor - Math.floor(visible / 2), items.length - visible));
    for (let i = first; i < Math.min(items.length, first + visible); i++) {
      const it = items[i];
      const val = it.setting ? formatValue(it, f.settings[it.setting], this.ctx) : it.children ? '>' : '';
      out.push({ text: `${i === this.cursor ? '>' : ' '}${it.label}`, value: val, selected: i === this.cursor });
    }
    return out;
  }
}

function optionsOf(item, ctx) {
  const opts = typeof item.options === 'function' ? item.options(ctx.type, ctx.getFixture()) : item.options || [];
  return opts.map((o) => (Array.isArray(o) ? { value: o[0], label: o[1] } : o));
}

function stepValue(item, value, delta, ctx) {
  if (item.type === 'number') {
    const step = item.step || 1;
    let v = Number(value) + delta * step;
    if (v > item.max) v = item.wrap ? item.min : item.max;
    if (v < item.min) v = item.wrap ? item.max : item.min;
    return v;
  }
  if (item.type === 'enum' || item.type === 'bool') {
    const opts = item.type === 'bool' ? [{ value: true }, { value: false }] : optionsOf(item, ctx);
    const i = opts.findIndex((o) => o.value === value);
    return opts[(i + delta + opts.length) % opts.length].value;
  }
  return value;
}

export function formatValue(item, value, ctx) {
  if (item.format) return item.format(value, ctx.type);
  if (item.type === 'bool') return value ? 'ON' : 'OFF';
  if (item.type === 'enum') return optionsOf(item, ctx).find((o) => o.value === value)?.label ?? String(value ?? '—');
  if (item.type === 'number') return `${value}${item.unit || ''}`;
  return value === undefined ? '' : String(value);
}

/**
 * Contrôleur d'un projecteur : interprète les gestes sur les boutons selon la définition.
 */
export class OnboardController {
  /**
   * @param {object} p
   * @param {object} p.type     type de projecteur (avec p.type.controls)
   * @param {() => object} p.getFixture
   * @param {(changes: {settings?: object, state?: object}) => void} p.emit
   */
  constructor({ type, getFixture, emit }) {
    this.type = type;
    this.def = type.controls || {};
    this.getFixture = getFixture;
    this.emit = emit;
    this.holdTimes = { ...DEFAULT_HOLD_TIMES, ...(this.def.holdTimes || {}) };
    this.pressed = new Map(); // bouton -> { t0, fired: Set }
    this.anim = null;
    this.message = null; // { text, until }
    this.mode = null; // ex. 'bluemode'
    this.lastT = 0;
    this.menu = this.def.menu
      ? new MenuNavigator(this.def.menu, {
          type,
          getFixture,
          setSetting: (k, v) => this.emit({ settings: { [k]: v } }),
          runAction: (name, item) => this.runAction(name, { item }),
          homeLines: this.def.homeLines,
        })
      : null;
  }

  // ------------------------------------------------------------------ gestes
  press(id, t = this.lastT) {
    this.pressed.set(id, { t0: t, fired: new Set() });
  }

  release(id, t = this.lastT) {
    const p = this.pressed.get(id);
    if (!p) return;
    this.pressed.delete(id);
    const held = t - p.t0;
    const gestures = this.def.actions?.[id] || {};
    if (p.fired.size) {
      // Un geste "maintenu" a déjà été déclenché pendant l'appui.
      if (gestures.releaseAfterHold) this.runAction(gestures.releaseAfterHold, { button: id, held });
      return;
    }
    if (held < this.holdTimes.long) this.runAction(gestures.short, { button: id, held });
    else if (gestures.long) this.runAction(gestures.long, { button: id, held });
  }

  /** À appeler à chaque image : déclenche les gestes "maintenir N secondes". */
  tick(t) {
    this.lastT = t;
    for (const [id, p] of this.pressed) {
      const gestures = this.def.actions?.[id] || {};
      for (const key of ['hold3', 'hold5']) {
        if (gestures[key] && !p.fired.has(key) && t - p.t0 >= this.holdTimes[key]) {
          p.fired.add(key);
          this.runAction(gestures[key], { button: id, held: t - p.t0 });
        }
      }
    }
    if (this.message && t > this.message.until) this.message = null;
  }

  /** Durée d'appui en cours sur un bouton (pour l'aide visuelle). */
  heldFor(id, t = this.lastT) {
    const p = this.pressed.get(id);
    return p ? t - p.t0 : 0;
  }

  say(text, seconds = 2.5) {
    this.message = { text, until: this.lastT + seconds };
  }

  // ------------------------------------------------------------------ actions
  runAction(name, info = {}) {
    if (!name) return;
    const f = this.getFixture();
    const custom = this.def.customActions?.[name];
    if (custom) return custom(this, f, info);
    switch (name) {
      case 'togglePower':
        this.emit({ state: { power: !f.state.power } });
        this.say(f.state.power ? 'Extinction' : 'Allumage');
        break;
      case 'powerOn':
        if (!f.state.power) this.emit({ state: { power: true } }), this.say('Allumage');
        break;
      case 'powerOff':
        if (f.state.power) this.emit({ state: { power: false } }), this.say('Extinction');
        break;
      case 'batteryGauge':
        this.anim = new PixelAnimation('gauge', { level: (f.state.battery ?? 100) / 100, duration: 3 });
        this.say(`Batterie ${Math.round(f.state.battery ?? 100)} %`);
        break;
      case 'identify':
        this.anim = new PixelAnimation('blink', { color: [1, 1, 1], hz: 3, duration: 4 });
        this.say('Identification');
        break;
      case 'blueMode':
        this.mode = 'bluemode';
        this.anim = new PixelAnimation('blink', { color: [0, 0.15, 1], hz: 1.5, duration: 60 });
        this.say('BlueMode : prêt à recevoir le RadioPin de l’appli', 4);
        break;
      case 'menuEnter':
        this.menu?.enter();
        break;
      case 'menuBack':
        this.menu?.back();
        break;
      case 'menuUp':
        this.menu?.move(-1);
        break;
      case 'menuDown':
        this.menu?.move(1);
        break;
      default:
        this.say(`(${name} : non simulé)`);
    }
  }

  /** Fin du BlueMode (appairage reçu ou annulé). */
  endBlueMode() {
    this.mode = null;
    this.anim = null;
  }

  // ------------------------------------------------------------------ sorties
  /** Remplace la sortie normale des pixels pendant une animation. @returns {boolean} */
  overridePixels(out, pixels, t) {
    if (!this.anim) return false;
    if (this.anim.apply(out, pixels, t)) {
      this.anim = null;
      if (this.mode === 'bluemode') this.mode = null;
      return false;
    }
    return true;
  }

  /** État des voyants : { [id]: { color, blinkHz } } */
  leds() {
    const f = this.getFixture();
    if (this.def.ledStates) return this.def.ledStates(f, this);
    return {};
  }

  displayLines(maxLines = 6) {
    const lines = this.menu ? this.menu.lines(maxLines) : [];
    if (this.message && !this.menu?.open) return [...lines.slice(0, maxLines - 1), { text: this.message.text }];
    return lines;
  }
}
