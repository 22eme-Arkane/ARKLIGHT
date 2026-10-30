// Panneaux de commande des projecteurs « non Astera » (ARRI, Aputure, Hive, DMG, Kino Flo, LiteGear,
// Dedolight) : touches, molettes (tourner, appuyer, maintenir), écran et voyants, décrits marque par
// marque dans shared/onboard/panels/. Logique pure, sans DOM : l'interface relaie les gestes et
// dessine l'écran à partir de primitives simples (texte, rectangle, trait, anneau).
//
// Primitives d'écran (coordonnées en pixels de l'écran simulé) :
//   { t: 'text', x, y, text, s: taille, align: 'l'|'c'|'r', inv, bold, color }
//   { t: 'rect', x, y, w, h, fill, stroke }      fill / stroke / color : 'fg' | 'bg' | 'dim' | 'accent' | '#rrggbb'
//   { t: 'line', x1, y1, x2, y2, w, color }
//   { t: 'ring', x, y, r, v (0..1), w, color, track }

export const HOLD_LONG = 1; // s : au-delà, appui long

const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
export const pad = (n, w = 3) => String(Math.round(n)).padStart(w, '0');

/** Accélération d'une molette : tourner vite fait de grands pas. */
function acceleration(dt) {
  if (dt < 0.035) return 10;
  if (dt < 0.08) return 4;
  if (dt < 0.16) return 2;
  return 1;
}

// ------------------------------------------------------------------ menus en arborescence
/**
 * Éléments de menu :
 *   sub(label, children)                         sous-menu
 *   pick(label, { get, set, options })           choix dans une liste (options : [[valeur, libellé]] ou fonction)
 *   num(label, { get, set, min, max, step, fmt }) valeur numérique
 *   info(label, value)                           lecture seule (value : texte ou fonction)
 *   act(label, run)                              action
 * Chaque élément peut avoir when(c) (visible si vrai) et short (libellé court pour les écrans étroits).
 */
export const sub = (label, children, o = {}) => ({ label, children, ...o });
export const pick = (label, o) => ({ label, kind: 'enum', ...o });
export const num = (label, o) => ({ label, kind: 'number', step: 1, ...o });
export const info = (label, value, o = {}) => ({ label, kind: 'info', value, ...o });
export const act = (label, run, o = {}) => ({ label, kind: 'action', run, ...o });

/** Réglage mémorisé dans settings.panel (menus sans effet sur la lumière simulée). */
export const stored = (key, dflt) => ({ get: (c) => c.panel(key, typeof dflt === 'function' ? dflt(c) : dflt), set: (c, v) => c.setPanel({ [key]: v }) });

export function optionsOf(item, c) {
  const o = typeof item.options === 'function' ? item.options(c) : item.options || [];
  return o.map((x) => (Array.isArray(x) ? { value: x[0], label: x[1] ?? String(x[0]) } : { value: x, label: String(x) }));
}

export function valueText(item, c, value = item.get?.(c)) {
  if (item.kind === 'info') return typeof item.value === 'function' ? item.value(c) : item.value ?? '';
  if (item.fmt) return item.fmt(value, c);
  if (item.kind === 'enum') return optionsOf(item, c).find((o) => o.value === value)?.label ?? String(value ?? '');
  if (item.kind === 'number') return String(value);
  return '';
}

export class MenuState {
  constructor(tree, title = 'MENU', { wrap = false } = {}) {
    this.stack = [{ items: tree, title, cursor: 0 }];
    this.edit = null; // { item, value, cursor }
    this.wrap = wrap;
  }

  get level() {
    return this.stack[this.stack.length - 1];
  }

  items(c) {
    return this.level.items.filter((it) => !it.when || it.when(c));
  }

  current(c) {
    const items = this.items(c);
    this.level.cursor = clamp(this.level.cursor, 0, Math.max(0, items.length - 1));
    return items[this.level.cursor];
  }

  move(c, d, mult = 1) {
    if (this.edit) return this._editMove(c, d, mult);
    const n = this.items(c).length;
    if (!n) return;
    const i = this.level.cursor + d;
    this.level.cursor = this.wrap ? (i + n * 10) % n : clamp(i, 0, n - 1);
  }

  _editMove(c, d, mult) {
    const { item } = this.edit;
    if (item.kind === 'number') {
      const step = (item.step || 1) * (item.accel === false ? 1 : mult);
      const min = typeof item.min === 'function' ? item.min(c) : item.min;
      const max = typeof item.max === 'function' ? item.max(c) : item.max;
      let v = Math.round((Number(this.edit.value) + d * step) / (item.step || 1)) * (item.step || 1);
      if (item.wrap) v = v > max ? min : v < min ? max : v;
      this.edit.value = clamp(Number(v.toFixed(4)), min, max);
    } else {
      const opts = optionsOf(item, c);
      const n = opts.length;
      this.edit.cursor = item.wrap === false ? clamp(this.edit.cursor + d, 0, n - 1) : (this.edit.cursor + d + n) % n;
      this.edit.value = opts[this.edit.cursor]?.value;
    }
  }

  /** Appui (molette, ENTER…) : ouvre, édite, valide. Renvoie ce qui s'est passé. */
  enter(c) {
    if (this.edit) {
      const { item, value } = this.edit;
      this.edit = null;
      const msg = item.set?.(c, value);
      return { committed: item, value, msg };
    }
    const it = this.current(c);
    if (!it) return null;
    if (it.children) {
      this.stack.push({ items: it.children, title: it.label, cursor: 0, item: it });
      return { opened: it };
    }
    if (it.kind === 'enum') {
      const opts = optionsOf(it, c);
      const cur = it.get?.(c);
      const i = Math.max(0, opts.findIndex((o) => o.value === cur));
      this.edit = { item: it, value: opts[i]?.value, cursor: i };
      return { editing: it };
    }
    if (it.kind === 'number') {
      this.edit = { item: it, value: Number(it.get?.(c) ?? 0), cursor: 0 };
      return { editing: it };
    }
    if (it.kind === 'action') return { action: it, msg: it.run(c) };
    return null;
  }

  /** Retour d'un niveau. Renvoie false s'il n'y a plus rien à fermer (le menu se ferme). */
  back() {
    if (this.edit) {
      this.edit = null;
      return true;
    }
    if (this.stack.length > 1) {
      this.stack.pop();
      return true;
    }
    return false;
  }
}

// ------------------------------------------------------------------ contrôleur
export class PanelController {
  /**
   * @param {object} p
   * @param {object} p.type  type du projecteur
   * @param {object} p.def   définition de la marque (panels/*.js)
   * @param {() => object} p.getFixture
   * @param {(changes: object) => void} p.emit   { settings?, state? } (fusion superficielle)
   * @param {() => object} [p.env]  { dmxLive(univers) }
   */
  constructor({ type, def, getFixture, emit, env }) {
    this.type = type;
    this.def = def;
    this.getFixture = getFixture;
    this.emit = emit;
    this.env = env || (() => ({}));
    this.t = 0;
    this.pressed = new Map(); // id → { t0, fired }
    this.lastTurn = new Map();
    this.message = null;
    this.flash = null; // valeur affichée en grand pendant qu'on tourne une molette : { text, until }
    this.menu = null;
    this.screen = 'home';
    this.st = def.init ? def.init(this) : {}; // état local de la marque (curseurs, pages…)
  }

  // ---------------------------------------------------------------- accès
  get f() {
    return this.getFixture();
  }
  get s() {
    return this.f.settings;
  }
  get m() {
    return this.f.state.manual || {};
  }
  get on() {
    return this.f.state.power !== false;
  }
  panel(key, dflt) {
    const v = this.s.panel?.[key];
    return v === undefined ? dflt : v;
  }
  setPanel(changes) {
    this.emit({ settings: { panel: { ...(this.s.panel || {}), ...changes } } });
  }
  setSettings(changes) {
    this.emit({ settings: changes });
  }
  setManual(changes) {
    this.emit({ state: { manual: { ...this.m, ...changes } } });
  }
  setState(changes) {
    this.emit({ state: changes });
  }
  /** Le projecteur reçoit du DMX (console, appli) sur son univers. */
  dmxLive() {
    return !!this.env().dmxLive?.(this.s.universe);
  }
  say(text, seconds = 2.5) {
    this.message = { text, until: this.t + seconds };
  }
  showBig(text, seconds = 1.5) {
    this.flash = { text, until: this.t + seconds };
  }
  openMenu(tree, title, opts) {
    this.menu = new MenuState(tree, title, opts);
    this.screen = 'menu';
  }
  closeMenu() {
    this.menu = null;
    this.screen = 'home';
  }

  // ---------------------------------------------------------------- gestes
  press(id, t = this.t) {
    this.t = t;
    this.pressed.set(id, { t0: t, fired: new Set() });
    this.def.down?.(this, id);
  }

  release(id, t = this.t) {
    this.t = t;
    const p = this.pressed.get(id);
    if (!p) return;
    this.pressed.delete(id);
    if (p.fired.size) return; // un appui maintenu a déjà agi
    const held = t - p.t0;
    this.def.key?.(this, id, { long: held >= (this.def.holdLong ?? HOLD_LONG), held });
  }

  /** Appui annulé (la molette a tourné pendant l'appui). */
  cancel(id) {
    this.pressed.delete(id);
  }

  turn(id, steps, t = this.t) {
    this.t = t;
    const last = this.lastTurn.get(id) ?? -1;
    this.lastTurn.set(id, t);
    if (!steps) return;
    this.def.turn?.(this, id, steps, acceleration(t - last));
  }

  /** Chaque image : appuis maintenus (holds de la marque : { id: [[secondes, action]] }), messages. */
  tick(t) {
    this.t = t;
    for (const [id, p] of this.pressed) {
      for (const [sec, fn] of this.def.holds?.[id] || []) {
        const k = `h${sec}`;
        if (!p.fired.has(k) && t - p.t0 >= sec) {
          p.fired.add(k);
          fn(this, id);
        }
      }
    }
    if (this.message && t > this.message.until) this.message = null;
    if (this.flash && t > this.flash.until) this.flash = null;
    this.def.tick?.(this, t);
  }

  heldFor(id, t = this.t) {
    const p = this.pressed.get(id);
    return p ? t - p.t0 : 0;
  }

  // ---------------------------------------------------------------- sorties
  display(t = this.t) {
    this.tick(t);
    if (!this.on && !this.def.displayWhenOff) return { off: true, prims: [] };
    return this.def.draw(this);
  }

  leds(t = this.t) {
    return this.def.leds?.(this, t) || {};
  }

  hint() {
    return this.message?.text || '';
  }

  /** Vue 3D : pas d'animation propre (les effets des marques ne sont pas simulés sur la lumière). */
  overridePixels() {
    return false;
  }
}

// ------------------------------------------------------------------ dessins communs
/**
 * Liste de menu : titre, lignes (valeur à droite), ligne choisie en inversé, ascenseur.
 * o : { rows, rowH, titleH, size, titleSize, underline, values, x0, w }
 */
export function drawList(c, menu, o = {}) {
  const W = o.w ?? c.def.display.w;
  const x0 = o.x0 ?? 0;
  const prims = [];
  const titleH = o.titleH ?? 14;
  const rowH = o.rowH ?? 16;
  const rows = o.rows ?? 3;
  const size = o.size ?? 11;
  if (menu.edit) return drawEdit(c, menu, o);
  const items = menu.items(c);
  const cur = menu.level.cursor;
  prims.push({ t: 'text', x: x0 + W / 2, y: 1, text: menu.level.title, s: o.titleSize ?? 10, align: 'c', bold: true });
  if (o.underline !== false) prims.push({ t: 'line', x1: x0 + 2, y1: titleH - 1.5, x2: x0 + W - 2, y2: titleH - 1.5, w: 1 });
  const first = clamp(cur - Math.floor(rows / 2), 0, Math.max(0, items.length - rows));
  for (let i = first; i < Math.min(items.length, first + rows); i++) {
    const it = items[i];
    const y = titleH + (i - first) * rowH;
    const sel = i === cur;
    const inv = sel && !o.selFill;
    const color = sel && o.selFill ? o.selText || 'fg' : undefined;
    if (sel) prims.push({ t: 'rect', x: x0, y: y + 1, w: W - 5, h: rowH - 1, fill: o.selFill || 'fg' });
    const label = it.short && o.short ? it.short : it.label;
    prims.push({ t: 'text', x: x0 + 3, y: y + (rowH - size) / 2 + 1, text: label, s: size, inv, color, w: W - 8 });
    // valeur courte à droite de la ligne (« 050 », « P01 », « On ») ; les longues s'affichent en ouvrant
    const val = o.values === false || it.children ? '' : String(valueText(it, c));
    if (val && val.length <= (o.maxValue ?? 8)) prims.push({ t: 'text', x: x0 + W - 8, y: y + (rowH - size) / 2 + 1, text: val, s: size, align: 'r', inv, color, w: (W - 10) * 0.4 });
  }
  // ascenseur
  if (items.length > rows) {
    const trackH = rows * rowH;
    const h = Math.max(6, (trackH * rows) / items.length);
    const y = titleH + ((trackH - h) * first) / Math.max(1, items.length - rows);
    prims.push({ t: 'rect', x: x0 + W - 3, y: titleH + 1, w: 2, h: trackH - 1, fill: 'dim' });
    prims.push({ t: 'rect', x: x0 + W - 4, y, w: 4, h, fill: 'fg' });
  }
  return prims;
}

/** Choix d'une valeur : liste d'options (◇ / ▶) ou grande valeur numérique. */
export function drawEdit(c, menu, o = {}) {
  const W = o.w ?? c.def.display.w;
  const H = o.h ?? c.def.display.h;
  const x0 = o.x0 ?? 0;
  const { item } = menu.edit;
  const titleH = o.titleH ?? 14;
  const prims = [{ t: 'text', x: x0 + W / 2, y: 1, text: item.label, s: o.titleSize ?? 10, align: 'c', bold: true }, { t: 'line', x1: x0 + 2, y1: titleH - 1, x2: x0 + W - 2, y2: titleH - 1, w: 1 }];
  if (item.kind === 'number') {
    prims.push({ t: 'text', x: x0 + W / 2, y: H / 2 - 8, text: valueText(item, c, menu.edit.value), s: o.bigSize ?? 20, align: 'c', bold: true });
    prims.push({ t: 'text', x: x0 + W / 2, y: H - 12, text: o.confirmHint ?? 'Push = OK', s: 9, align: 'c', color: 'dim' });
    return prims;
  }
  const opts = optionsOf(item, c);
  const rows = o.rows ?? 3;
  const rowH = o.rowH ?? 16;
  const cur = menu.edit.cursor;
  const first = clamp(cur - Math.floor(rows / 2), 0, Math.max(0, opts.length - rows));
  // repères : ▶ / ◇ (ARRI), ◉ / ○ (cases radio), ■ / □…
  const [on, off] = o.markers || ['▶', '◇'];
  const current = item.get?.(c);
  for (let i = first; i < Math.min(opts.length, first + rows); i++) {
    const y = titleH + (i - first) * rowH;
    const sel = i === cur;
    if (sel) prims.push({ t: 'rect', x: x0, y: y + 1, w: W - 5, h: rowH - 1, fill: o.selFill || 'fg' });
    const mark = o.radio ? (opts[i].value === current ? on : off) : sel ? on : off;
    prims.push({ t: 'text', x: x0 + 3, y: y + (rowH - (o.size ?? 11)) / 2 + 1, text: `${mark}${opts[i].label}`, s: o.size ?? 11, inv: sel && !o.selFill, color: sel && o.selFill ? o.selText || 'fg' : undefined, w: W - 8 });
  }
  return prims;
}
