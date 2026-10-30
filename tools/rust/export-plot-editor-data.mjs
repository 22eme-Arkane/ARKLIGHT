// Version Rust d'ARKLIGHT : données d'essai de parité des outils du plan de feu, calculées par la
// version JavaScript (web/js/plot/plot-view.js) pilotée par des gestes (souris, clavier, réglages de
// l'élément sélectionné, menu contextuel) dans un DOM simulé.
//   node tools/rust/export-plot-editor-data.mjs
// Écrit dans rust/crates/arklight-plot/tests/data/ :
//   editor-scene.json   projet d'essai : projecteurs, décor, grip, un élément de chaque nature,
//                       image de fond
//   editor.json         scénarios : vue, réglages d'affichage, gestes (en pixels de la vue) et, après
//                       chaque geste, l'état de la vue du plan (outil, indication, sélection, tracé en
//                       cours, poignées, cadre de sélection ; éléments du plan, image de fond,
//                       projecteurs et objets quand ils changent)
// Les modifications envoyées au serveur sont appliquées par les fonctions du serveur
// (server/scene-store.js : addPlotItem, updatePlotItem, removePlotItem, updatePlotMeta) puis renvoyées
// à la vue comme le fait web/js/app.js. Le rendu (render.js) est intercepté pour lire l'état interne
// de la vue (sélection du plan, tracé, poignées) ; i18n.js, icons.js et context-menu.js sont simulés.
import { registerHooks } from 'node:module';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// heure et hasard fixes (identifiants des éléments créés)
const FIXED = Date.UTC(2026, 9, 27, 8, 30, 14);
const RealDate = Date;
globalThis.Date = class extends RealDate {
  constructor(...a) {
    if (a.length) super(...a);
    else super(FIXED);
  }
  static now() {
    return FIXED;
  }
};
let seed = 12345;
Math.random = () => {
  seed = (seed * 1103515245 + 12345) % 2147483648;
  return seed / 2147483648;
};

// ------------------------------------------------------------------ DOM simulé
class FakeNode {}
class El extends FakeNode {
  constructor(tag) {
    super();
    this.tagName = String(tag).toUpperCase();
    this.children = [];
    this.attrs = {};
    this.listeners = {};
    this.parent = null;
    this.style = { setProperty() {} };
    this._cls = new Set();
    this._html = '';
    this._text = '';
    this.value = '';
    this.checked = false;
    this.w = 0;
    this.h = 0;
    const cls = this._cls;
    this.classList = {
      add: (...c) => c.forEach((x) => cls.add(x)),
      remove: (...c) => c.forEach((x) => cls.delete(x)),
      toggle: (c, on) => {
        const want = on === undefined ? !cls.has(c) : !!on;
        if (want) cls.add(c);
        else cls.delete(c);
        return want;
      },
      contains: (c) => cls.has(c),
    };
  }
  get className() {
    return [...this._cls].join(' ');
  }
  set className(v) {
    this._cls.clear();
    for (const c of String(v).split(/\s+/).filter(Boolean)) this._cls.add(c);
  }
  get textContent() {
    return this._text || this.children.map((c) => c.textContent ?? '').join('');
  }
  set textContent(v) {
    this._text = String(v);
    this.children = [];
  }
  setAttribute(k, v) {
    if (k === 'class') this.className = v;
    this.attrs[k] = String(v);
  }
  getAttribute(k) {
    return this.attrs[k] ?? null;
  }
  append(...cs) {
    for (const c of cs) {
      if (c instanceof FakeNode) c.parent = this;
      this.children.push(c);
    }
  }
  replaceChildren(...cs) {
    this.children = [];
    this.append(...cs);
  }
  remove() {
    if (this.parent) this.parent.children = this.parent.children.filter((x) => x !== this);
  }
  contains(x) {
    for (let n = x; n; n = n.parent) if (n === this) return true;
    return false;
  }
  addEventListener(t, fn) {
    (this.listeners[t] ||= []).push(fn);
  }
  removeEventListener() {}
  dispatch(t, ev = {}) {
    for (const fn of this.listeners[t] || []) fn(ev);
  }
  set innerHTML(v) {
    this._html = String(v);
    this.children = [];
  }
  get innerHTML() {
    return this._html;
  }
  getBoundingClientRect() {
    return { left: 0, top: 0, width: this.w, height: this.h };
  }
  setPointerCapture() {}
  closest() {
    return null;
  }
  focus() {}
  select() {}
  get offsetLeft() {
    return 0;
  }
  querySelector() {
    return null;
  }
  querySelectorAll() {
    return [];
  }
}
class TextNode extends FakeNode {
  constructor(t) {
    super();
    this.textContent = String(t);
  }
}
let wrap = new El('div');
let winListeners = {};
const storage = {};
let raf = [];
const body = new El('body');
globalThis.__El = El;
globalThis.Node = FakeNode;
globalThis.document = {
  createElement: (t) => new El(t),
  createElementNS: (_, t) => new El(t),
  createTextNode: (t) => new TextNode(t),
  querySelector: (s) => (s === '.viewport-wrap' ? wrap : null),
  querySelectorAll: () => [],
  addEventListener() {},
  removeEventListener() {},
  getElementById: () => new El('div'),
  activeElement: body,
  body,
  documentElement: new El('html'),
};
globalThis.window = {
  addEventListener: (t, fn) => (winListeners[t] ||= []).push(fn),
  removeEventListener() {},
};
globalThis.ResizeObserver = class {
  observe() {}
};
globalThis.requestAnimationFrame = (fn) => raf.push(fn);
globalThis.localStorage = { getItem: (k) => storage[k] ?? null, setItem: (k, v) => (storage[k] = String(v)) };
globalThis.setInterval = () => 0;
globalThis.setTimeout = () => 0;
globalThis.clearTimeout = () => {};

// ------------------------------------------------------------------ modules
const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const ROOT_URL = pathToFileURL(ROOT).href.replace(/\/?$/, '/');
const url = (p) => new URL(p, ROOT_URL).href;
const stub = (code) => `data:text/javascript,${encodeURIComponent(code)}`;
const STUBS = {
  [url('web/js/i18n.js')]: stub(`import { LANGS, translate, translateMarkup } from '${url('shared/i18n.js')}';
export { LANGS };
export const lang = 'fr';
export const t = (fr, params) => translate(fr, 'fr', params);
export const tMarkup = (markup) => translateMarkup(markup, 'fr');`),
  [url('web/js/ui/icons.js')]: stub(`export const icon = () => new globalThis.__El('svg');
export const iconSvg = () => '';
export function hydrateIcons() {}`),
  [url('web/js/ui/context-menu.js')]: stub(`export function openContextMenu(x, y, items) { globalThis.__menu = items; }
export function closeContextMenu() {}
export const isContextMenuOpen = () => false;`),
  // rendu : l'état de la vue (sélection, tracé, poignées) est noté à chaque dessin
  [url('web/js/plot/render.js')]: stub(`import * as real from '${url('web/js/plot/render.js')}?real';
export * from '${url('web/js/plot/render.js')}?real';
export function renderPlot(args) {
  globalThis.__render = { sel: args.sel, preview: args.preview, handles: args.handles, view: { ...args.view } };
  return real.renderPlot(args);
}`),
};
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (/^\/(shared|web)\//.test(specifier)) specifier = url(specifier.slice(1));
    const r = nextResolve(specifier, context);
    // le rendu n'est intercepté que pour la vue du plan
    if (r.url === url('web/js/plot/render.js') && !context.parentURL?.endsWith('/plot/plot-view.js')) return r;
    if (STUBS[r.url]) return { url: STUBS[r.url], shortCircuit: true };
    return r;
  },
});
const mod = (p) => import(url(p));
const { mountPlot } = await mod('web/js/plot/plot-view.js');
const { Store } = await mod('web/js/store.js');
const { SceneStore } = await mod('server/scene-store.js');
const { getFixtureType } = await mod('shared/fixtures/index.js');
const { resolveSupport } = await mod('shared/fixtures/supports.js');
const { bodyHalfExtents } = await mod('web/js/plot/geometry.js');
const { PROP_CATEGORIES, PROP_TYPES, defaultPropProps, propSize } = await mod('shared/props.js');
// Bogue de la version JavaScript : les personnages n'ont pas de `size` (leur taille dépend de la pose)
// et la poignée d'orientation d'un personnage sélectionné arrête le dessin du plan (TypeError dans
// handles()). On leur donne la taille qu'a la version Rust (bibliothèque : propSize avec les réglages
// par défaut, voir tools/rust/export-library.mjs).
for (const t of PROP_TYPES) if (!t.size) t.size = propSize(t, defaultPropProps(t));

const OUT = join(ROOT, 'rust/crates/arklight-plot/tests/data');
mkdirSync(OUT, { recursive: true });

// ------------------------------------------------------------------ projet d'essai
const DECOR = PROP_CATEGORIES.map((c) => c.id);
const EQUIPMENT_ONLY = { stands: true, grip: true, sandbags: false, studio: false, annotations: false, ...Object.fromEntries(DECOR.map((c) => [c, false])) };
const SHOW_ALL = Object.fromEntries(Object.keys(EQUIPMENT_ONLY).map((k) => [k, true]));
const DEFAULTS = { names: true, fixtures: true, addresses: true, supports: true, beams: true, grid: true, underlay: true, ...EQUIPMENT_ONLY };

function editorScene() {
  const fx = (id, type, name, position, rotation) => {
    const t = getFixtureType(type);
    if (!t) throw new Error(`projecteur inconnu : ${type}`);
    return {
      id,
      type,
      name,
      position,
      rotation,
      settings: { universe: 1, dmxAddress: 1, dmxMode: t.dmx?.modes?.[0]?.id ?? null, whitePoint: 5600, dimmingCurve: 'linear', support: 'auto', ...(t.defaults?.settings || {}) },
      state: { power: true, battery: 100, charging: false, manual: { mode: 'cct', intensity: 0.6, cct: 5600, tint: 0, hue: 0, sat: 1 } },
    };
  };
  const scene = {
    app: 'ARKLIGHT',
    format: 2,
    name: 'Outils du plan',
    environment: { floor: true, cyc: true, stands: true, labels: false, labelPatch: false, haze: 0.2, ambient: 0.04, exposure: 1 },
    fixtures: [
      fx('f-panel', 'arri.skypanel-s60c', 'Panneau', [-3, 2, 2], [0.3, -0.6, 0]),
      fx('f-down', 'arri.m18', 'Plongée', [3, 4.8, 2], [Math.PI / 2, 0, 0]),
      fx('f-tube', 'astera.titan', 'Tube', [0, 1.4, -2], [0, 0.4, Math.PI / 2]),
      fx('f-bar', 'astera.ax2-100', 'Barre', [-1, 0.3, 3.5], [-0.2, 0.8, 0]),
    ],
    groups: [],
    objects: [
      { id: 'o-actor', type: 'person-man', name: 'Comédien', position: [0, 0, 0.5], rotation: [0, 0.5, 0], props: {} },
      { id: 'g-frame', type: 'frame-6x6', name: 'Cadre 6×6', position: [4, 0, -2], rotation: [0, 0.3, 0], props: { cloth: 'silk' } },
      { id: 'g-flag', type: 'flag-18x24', name: 'Drapeau', position: [-4.5, 0, -1], rotation: [0, -0.7, 0], props: { cloth: 'solid' } },
      { id: 'g-stand', type: 'stand-1000', name: 'Pied 1000', position: [2, 0, 3.2], rotation: [0, 0, 0], props: { height: 2.2 } },
      { id: 'g-bag', type: 'sandbag', name: 'Sac', position: [-2, 0, -4], rotation: [0, 0.2, 0], props: {} },
    ],
    plot: { items: [], underlay: { url: '/assets/plan.png', x: 0, z: 0, width: 12, aspect: 1.5, rotation: 0, opacity: 0.6, floor3d: false }, background: null, title: { production: '', author: '', notes: '' } },
  };
  const style = (stroke, extra = {}) => ({ stroke, fill: 'none', width: 2, dash: false, size: 14, ...extra });
  const raw = [
    { id: 'p-rect', kind: 'rect', points: [[-6, -5], [-4, -3.5]], style: style('#5aa9ff') },
    { id: 'p-fill', kind: 'rect', points: [[5, 3], [7, 4.5]], text: 'Loges', style: style('#3ddc84', { fill: '#3ddc84' }), height: 2.5 },
    { id: 'p-ell', kind: 'ellipse', points: [[-6.5, 4], [-4.5, 5.5]], style: style('#c792ea', { dash: true }) },
    { id: 'p-poly', kind: 'polygon', points: [[1, 4.5], [2.5, 5], [2, 6.5]], style: style('#ffad3b', { fill: '#ffad3b' }) },
    { id: 'p-free', kind: 'freehand', points: [[4, -5], [4.5, -4.6], [5, -4.8], [5.6, -4.4]], style: style('#ff5c5c', { width: 5 }) },
    { id: 'p-line', kind: 'line', points: [[-2, -5.5], [1, -5.5]], text: 'Câble', style: style('#ff5c5c') },
    { id: 'p-arrow', kind: 'arrow', points: [[6.5, -1.5], [7.5, 1]], style: style('#3ddc84', { width: 3 }) },
    { id: 'p-measure', kind: 'measure', points: [[-7, -1], [-7, 1.5]], style: style('#5aa9ff') },
    { id: 'p-text', kind: 'text', points: [[-3, 6.8]], text: 'Régie', style: { stroke: '#ffffff', size: 20 } },
  ];
  // éléments nettoyés par le serveur (mêmes identifiants)
  const server = { scene, _changed() {} };
  for (const it of raw) {
    scene.plot.items.push(it);
    SceneStore.prototype.updatePlotItem.call(server, it.id, {});
  }
  return scene;
}
const SCENE = editorScene();
writeFileSync(join(OUT, 'editor-scene.json'), `${JSON.stringify(SCENE, null, 1)}\n`);

// ------------------------------------------------------------------ pilotage de la vue du plan
const TIPS = {
  'Sélection : clic, Ctrl+clic, cadre de sélection ; glisser pour déplacer': 'select',
  'Rectangle (maison, décor…)': 'rect',
  'Ellipse ou cercle': 'ellipse',
  'Forme point par point : double-clic ou Entrée pour terminer': 'polygon',
  'Dessin à main levée': 'freehand',
  Ligne: 'line',
  'Flèche (désigner un lieu, un kiosque…)': 'arrow',
  Texte: 'text',
  'Mesure d’une distance': 'measure',
};
const walk = (el, f, out = []) => {
  if (f(el)) out.push(el);
  for (const c of el.children || []) if (c instanceof El) walk(c, f, out);
  return out;
};
const hasText = (el, text) => el.children.some((c) => c instanceof TextNode && c.textContent === text);
const clone = (v) => (v === undefined ? null : JSON.parse(JSON.stringify(v)));

function session(W, H, options) {
  storage['arklight.plot.options'] = JSON.stringify({ ...options, v: 3 });
  wrap = new El('div');
  winListeners = {};
  raf = [];
  const scene = structuredClone(SCENE);
  const server = { scene: structuredClone(SCENE), _changed() {} };
  const store = new Store();
  store.clientId = 'me';
  store.setScene(scene);
  const queue = [];
  const net = { send: (msg) => queue.push(structuredClone(msg)) };
  const viewport = {
    outputOf(id) {
      const f = store.scene.fixtures.find((x) => x.id === id);
      const type = f && getFixtureType(f.type);
      if (!type) return null;
      return { type, out: null, support: resolveSupport(type, f.settings.support, f.position[1] - Math.max(...bodyHalfExtents(type))) };
    },
    dispatchEvent() {},
  };
  const plot = mountPlot({ store, net, viewport, actions: { selectionMenu: () => [], objectMenu: () => [] } });
  const host = wrap.children.at(-1);
  const [toolbar, svg, hint, props, pop] = host.children;
  svg.w = W;
  svg.h = H;

  // web/js/app.js : messages du serveur
  const app = (msg) => {
    if (msg.t === 'plot.item') {
      store.setPlotItem(msg.item);
      if (msg.added && msg.origin === store.clientId) store.emit('plot-item-created', msg.item);
    } else if (msg.t === 'plot.removed') store.removePlotItem(msg.id);
    else if (msg.t === 'plot.meta') store.setPlotMeta(msg);
  };
  // server/index.js : projecteurs et objets sont déjà changés dans le store (déplacement local)
  const serve = () => {
    while (queue.length) {
      const msg = queue.shift();
      const S = SceneStore.prototype;
      if (msg.t === 'plot.add') app({ t: 'plot.item', item: clone(S.addPlotItem.call(server, msg.item)), origin: 'me', added: true });
      else if (msg.t === 'plot.update') {
        const it = S.updatePlotItem.call(server, msg.id, msg.changes || {});
        if (it) app({ t: 'plot.item', item: clone(it), origin: 'me' });
      } else if (msg.t === 'plot.remove') {
        for (const id of Array.isArray(msg.ids) ? msg.ids : [msg.id]) if (S.removePlotItem.call(server, id)) app({ t: 'plot.removed', id, origin: 'me' });
      } else if (msg.t === 'plot.meta') {
        const p = S.updatePlotMeta.call(server, msg);
        app({ t: 'plot.meta', underlay: clone(p.underlay), title: clone(p.title), background: p.background, origin: 'me' });
      }
    }
  };
  const flush = () => {
    for (let i = 0; i < 20 && (queue.length || raf.length); i++) {
      serve();
      const fns = raf;
      raf = [];
      for (const fn of fns) fn();
    }
  };
  plot.show();
  flush();

  const px = ([x, z]) => plot.toScreen(x, z).map(Math.round);
  const ev = ([x, y], o = {}) => ({ clientX: x, clientY: y, button: o.button ?? 0, ctrlKey: !!o.ctrl, metaKey: false, shiftKey: !!o.shift, pointerId: 1, preventDefault() {}, stopPropagation() {} });
  const click = (el) => el.dispatch('click', { currentTarget: el, target: el });
  const toolButton = (id) => walk(toolbar, (e) => e.tagName === 'BUTTON' && TIPS[e.attrs.title] === id)[0];
  const propsLabel = (text) => walk(props, (e) => e.tagName === 'LABEL' && hasText(e, text))[0];
  const field = (label, tag) => walk(propsLabel(label), (e) => e.tagName === tag)[0];
  const steps = [];

  const state = () => {
    const r = globalThis.__render;
    const opts = JSON.parse(storage['arklight.plot.options']);
    const marquee = svg.children.find((c) => c instanceof El && c.className === 'plot-marquee');
    const active = walk(toolbar, (e) => e.tagName === 'BUTTON' && e.classList.contains('plot-tool') && e.classList.contains('active'))[0];
    return {
      tool: active ? TIPS[active.attrs.title] : null,
      hint: hint.textContent,
      annotations: opts.annotations !== false,
      sel: { fixtures: store.selectedIds(), objectId: store.selectedObjectId ?? null, plotId: r.sel.plotId ?? null },
      preview: r.preview ? { kind: r.preview.kind, points: clone(r.preview.points) } : null,
      handles: r.handles.map((h) => ({ kind: h.kind, x: h.x, z: h.z, target: h.target })),
      marquee: marquee ? ['x', 'y', 'width', 'height'].map((k) => Number(marquee.attrs[k])) : null,
      calibrating: !!walk(props, (e) => e.tagName === 'INPUT' && e.classList.contains('plot-calib')).length,
      items: clone(store.scene.plot.items),
      underlay: clone(store.scene.plot.underlay),
      fixtures: store.scene.fixtures.map((f) => ({ id: f.id, position: f.position, rotation: f.rotation })),
      objects: store.scene.objects.map((o) => ({ id: o.id, position: o.position, rotation: o.rotation })),
    };
  };
  // éléments du plan, image de fond, projecteurs et objets : notés seulement quand ils changent
  const HEAVY = ['items', 'underlay', 'fixtures', 'objects'];
  const last = {};
  const record = (step) => {
    flush();
    const after = state();
    for (const k of HEAVY) {
      const j = JSON.stringify(after[k]);
      if (j === last[k]) delete after[k];
      else last[k] = j;
    }
    steps.push({ ...step, after });
  };
  for (const [k, v] of Object.entries(state())) if (HEAVY.includes(k)) last[k] = JSON.stringify(v);
  const api = {
    view: () => clone(globalThis.__render.view),
    state,
    down(p, o = {}) {
      const at = px(p);
      svg.dispatch('pointerdown', ev(at, o));
      record({ do: 'down', x: at[0], y: at[1], ctrl: !!o.ctrl, shift: !!o.shift });
    },
    move(p, o = {}) {
      const at = px(p);
      svg.dispatch('pointermove', ev(at, o));
      record({ do: 'move', x: at[0], y: at[1], ctrl: !!o.ctrl, shift: !!o.shift });
    },
    up(p) {
      const at = px(p);
      svg.dispatch('pointerup', ev(at));
      record({ do: 'up', x: at[0], y: at[1] });
    },
    click(p, o = {}) {
      api.down(p, o);
      api.up(p);
    },
    drag(a, b, o = {}, n = 3) {
      api.down(a, o);
      for (let i = 1; i <= n; i++) api.move([a[0] + ((b[0] - a[0]) * i) / n, a[1] + ((b[1] - a[1]) * i) / n], o);
      api.up(b);
    },
    dblclick() {
      svg.dispatch('dblclick', {});
      record({ do: 'dblclick' });
    },
    key(key) {
      for (const fn of winListeners.keydown || []) fn({ key, stopPropagation() {}, preventDefault() {} });
      record({ do: 'key', key });
    },
    tool(id) {
      click(toolButton(id));
      record({ do: 'tool', tool: id });
    },
    calibrate() {
      click(walk(toolbar, (e) => e.attrs['data-pop'] === 'underlay')[0]);
      click(walk(pop, (e) => e.tagName === 'BUTTON' && hasText(e, 'Caler l’échelle (2 points)'))[0]);
      record({ do: 'calibrate' });
    },
    distance(value) {
      walk(props, (e) => e.tagName === 'INPUT' && e.classList.contains('plot-calib'))[0].value = String(value);
      click(walk(props, (e) => e.tagName === 'BUTTON' && hasText(e, 'Appliquer'))[0]);
      record({ do: 'distance', value });
    },
    // réglages de l'élément sélectionné (barre des propriétés)
    prop(change, value) {
      const target = (el, v) => ({ target: Object.assign(el, v), currentTarget: el });
      if (change === 'text') {
        const el = walk(props, (e) => e.tagName === 'INPUT' && e.classList.contains('plot-text'))[0];
        el.dispatch('input', target(el, { value }));
      } else if (change === 'color') {
        const el = walk(props, (e) => e.tagName === 'INPUT' && e.attrs.type === 'color')[0];
        el.dispatch('input', target(el, { value }));
      } else if (change === 'fill' || change === 'dash') {
        const el = field(change === 'fill' ? ' remplie' : ' pointillés', 'INPUT');
        el.dispatch('change', target(el, { checked: value }));
      } else if (change === 'width' || change === 'size') {
        const el = field(change === 'width' ? 'trait ' : 'taille ', 'SELECT');
        el.dispatch('change', target(el, { value: String(value) }));
      } else if (change === 'height') {
        const el = field('hauteur 3D ', 'INPUT');
        el.dispatch('change', target(el, { value: String(value) }));
      } else if (change === 'delete') click(walk(props, (e) => e.tagName === 'BUTTON' && hasText(e, '🗑'))[0]);
      record({ do: 'prop', change, value: value ?? null });
    },
    // clic droit sans glisser, puis une ligne du menu (null : menu fermé)
    menu(p, action = null) {
      const at = px(p);
      globalThis.__menu = null;
      svg.dispatch('pointerdown', ev(at, { button: 2 }));
      svg.dispatch('pointerup', ev(at, { button: 2 }));
      const items = globalThis.__menu || [];
      if (action) items.find((i) => i.label === action)?.onClick();
      record({ do: 'menu', x: at[0], y: at[1], action, labels: items.map((i) => i.label ?? i.header ?? null) });
    },
  };
  return { api, steps, view: api.view };
}

// ------------------------------------------------------------------ scénarios
const W = 1000;
const H = 700;
const scenarios = [];
function scenario(name, options, script) {
  const s = session(W, H, options);
  const view = s.view();
  const initial = s.steps.length;
  script(s.api);
  scenarios.push({ name, options, W, H, view, steps: s.steps.slice(initial) });
}

scenario('dessin', { ...DEFAULTS, ...SHOW_ALL }, (a) => {
  a.tool('rect');
  a.drag([-1, -3], [1.5, -1.5]);
  a.tool('ellipse');
  a.drag([2, -4], [3.5, -3]);
  a.tool('line');
  a.drag([-5, 2], [-3, 3]);
  a.tool('arrow');
  a.drag([5, -2], [6, -3], {}, 2);
  a.tool('measure');
  a.drag([-1, 5], [3, 5]);
  // trop petit : rien, l'outil reste
  a.tool('rect');
  a.drag([0, 0], [0.05, 0.05], {}, 1);
  a.key('Escape');
  a.tool('freehand');
  a.down([-6, 6]);
  for (const p of [[-5.98, 6.01], [-5.7, 6.2], [-5.4, 6.1], [-5.1, 6.4], [-4.8, 6.3]]) a.move(p);
  a.up([-4.8, 6.3]);
  // main levée de deux points seulement : rien
  a.down([6, 6]);
  a.move([6.5, 6]);
  a.up([6.5, 6]);
  a.tool('text');
  a.click([-6, 0]);
  // forme point par point terminée au double-clic
  a.tool('polygon');
  for (const p of [[3, 0.5], [4, 0.2], [4.5, 1.2]]) {
    a.click(p);
    a.move([p[0] + 0.3, p[1] + 0.2]);
  }
  a.click([3.6, 1.6]);
  a.click([3.6, 1.6]);
  a.dblclick();
  // terminée par Entrée
  a.tool('polygon');
  for (const p of [[-3, -6], [-2, -6.5], [-1.5, -5.8], [-2.6, -5.5]]) {
    a.click(p);
    a.move(p);
  }
  a.key('Enter');
  // deux points seulement : rien
  a.tool('polygon');
  a.click([0, 6]);
  a.move([1, 6]);
  a.click([1, 6]);
  a.key('Enter');
  // Échap : outil abandonné, puis élément désélectionné
  a.tool('ellipse');
  a.key('Escape');
  a.click([-6, -4.25], {}); // bord gauche du rectangle p-rect
  a.key('Escape');
  a.key('Escape');
});

scenario('selection', { ...DEFAULTS, ...SHOW_ALL }, (a) => {
  a.click([-3, 2]); // panneau
  a.drag([-3, 2], [-2.5, 2.6]);
  a.click([0, -2], { ctrl: true }); // tube ajouté
  a.drag([0, -2], [0.7, -1.2]);
  a.click([0.7, -1.2], { shift: true }); // tube retiré
  a.click([0.02, 0.5]); // comédien
  a.drag([0.02, 0.5], [0.5, 1.2]);
  // cadre : par son tube (pas par son centre ni au milieu de la toile)
  a.click([4 + Math.cos(0.3) * 0.9, -2 - Math.sin(0.3) * 0.9]);
  a.click([4.5, -2.6]);
  a.click([-5, -5]); // bord haut du rectangle p-rect
  a.drag([-5, -5], [-4.5, -4.2]);
  a.click([-4.25, -3.4]); // dedans : rectangle vide, rien ; cadre de sélection
  a.click([6, 3.7]); // rectangle rempli
  a.click([-5.5, 4.75]); // ellipse
  a.click([1.9, 5.3]); // forme
  a.click([4.5, -4.6]); // main levée
  a.click([-0.5, -5.52]); // ligne
  a.click([7, -0.25]); // flèche
  a.click([-6.98, 0]); // mesure
  a.click([-2.6, 6.6]); // texte
  a.key('Delete');
  // cadre de sélection
  a.drag([-4, 1], [1, 4.2], {}, 4);
  a.drag([-1, -3], [1, -1], { ctrl: true }, 2);
  a.drag([2.5, 1], [3.5, 3], { shift: true }, 2);
  a.drag([2.5, 1], [3.5, 3], {}, 2);
  a.click([5, 6.5]); // dans le vide : tout désélectionné
  a.click([0.5, 1.2], { ctrl: true }); // comédien (déplacé) avec Ctrl
  a.click([-1, 3.5]); // barre
  a.click([-1, 3.5], { ctrl: true }); // retirée : plus rien
  a.click([2, 3.2]); // pied seul
  a.drag([-2, -4], [-1.8, -3.9], {}, 1); // sac de sable (masqué par défaut, affiché ici)
});

// poignée d'orientation : on la prend et on la fait tourner autour de l'élément
const around = (c, p, deg) => {
  const r = (deg * Math.PI) / 180;
  const [dx, dz] = [p[0] - c[0], p[1] - c[1]];
  return [c[0] + dx * Math.cos(r) - dz * Math.sin(r), c[1] + dx * Math.sin(r) + dz * Math.cos(r)];
};
function turn(a, center, steps, o = {}) {
  const hd = a.state().handles[0];
  const h = [hd.x, hd.z];
  a.down(h);
  for (const deg of steps) a.move(around(center, h, deg), o);
  a.up(around(center, h, steps.at(-1)));
}
scenario('orientation', { ...DEFAULTS, ...SHOW_ALL }, (a) => {
  a.click([-3, 2]); // panneau : poignée devant le faisceau
  turn(a, [-3, 2], [20, 43]); // crans de 5°
  turn(a, [-3, 2], [-12, -33], { shift: true }); // Maj : libre
  a.click([0.02, 0.5]); // comédien
  turn(a, [0, 0.5], [30, 90, 181]);
  a.click([4 + Math.cos(0.3) * 0.9, -2 - Math.sin(0.3) * 0.9]); // cadre
  turn(a, [4, -2], [-45]);
  a.click([-4.5, -1]); // drapeau
  turn(a, [-4.5, -1], [62]);
  a.click([3, 2]); // projecteur vers le sol : pas de poignée
  a.click([-1, 3.5]); // barre
  turn(a, [-1, 3.5], [-100]);
  a.click([-3, 2], { ctrl: true }); // deux projecteurs : pas de poignée
  a.click([2, 3.2]); // pied seul : pas de poignée
});

scenario('proprietes', { ...DEFAULTS, ...SHOW_ALL }, (a) => {
  a.click([-5, -5]); // rectangle p-rect
  a.prop('color', '#ff0000');
  a.prop('fill', true);
  a.prop('color', '#00ff00'); // rempli : le remplissage suit
  a.prop('fill', false);
  a.prop('dash', true);
  a.prop('width', 5);
  a.prop('height', 2.5);
  a.prop('height', -1);
  a.prop('text', 'Maison');
  a.click([-2.6, 6.6]); // texte
  a.prop('size', 40);
  a.prop('text', '');
  a.click([-2.6, 6.6]);
  a.prop('color', '#123456');
  a.prop('delete');
  a.click([-0.5, -5.52]); // ligne
  a.prop('width', 8);
  a.menu([6, 3.7], 'Dupliquer'); // rectangle rempli
  a.menu([7.3, 4.8], 'Supprimer'); // sa copie (décalée de 50 cm)
  a.menu([-3, 2]); // projecteur
  a.menu([0.02, 0.5]); // comédien
  a.menu([5, 6.5]); // dans le vide
  a.menu([-6.98, 0], 'Dupliquer'); // mesure
  a.key('Delete');
});

scenario('materiel', DEFAULTS, (a) => {
  a.click([-5, -5]); // annotations masquées : rien
  a.click([0.02, 0.5]); // décor masqué : rien
  a.click([-2, -4]); // sacs de sable masqués : rien
  a.click([2, 3.2]); // pied seul
  a.click([4 + Math.cos(0.3) * 0.9, -2 - Math.sin(0.3) * 0.9]); // cadre
  a.tool('rect'); // les annotations s'affichent
  a.drag([-1, -3], [1, -2]);
  a.click([-5, -5]);
});

scenario('calage', { ...DEFAULTS, ...SHOW_ALL }, (a) => {
  a.tool('rect');
  a.calibrate();
  a.click([-2, -1]);
  a.click([3, 2]); // sur un projecteur : c'est un point du calage
  a.distance(0); // distance non valable : on attend
  a.distance(7.5);
  a.click([-3, 2]);
  a.calibrate();
  a.click([0, 0]);
  a.key('Escape');
  a.click([-3, 2]);
});

writeFileSync(join(OUT, 'editor.json'), `${JSON.stringify({ scenarios }, null, 1)}\n`);
console.log(`données d'essai des outils du plan écrites dans ${OUT} (${scenarios.length} scénarios, ${scenarios.reduce((n, s) => n + s.steps.length, 0)} gestes)`);
