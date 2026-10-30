// Plan de feu 2D : vue de dessus de la scène, outils de dessin, image de fond à l'échelle, exports
// (PNG, SVG, impression / PDF avec la liste du matériel et le patch, CSV).
import { h, toast } from '../ui/dom.js';
import { icon } from '../ui/icons.js';
import { lang, t, tMarkup } from '../i18n.js';
import { openContextMenu } from '../ui/context-menu.js';
import { getFixtureType } from '/shared/fixtures/index.js';
import { getObjectType, isProp } from '/shared/objects.js';
import { objectShown, renderPlot, titleBlock } from './render.js';
import { PROP_CATEGORIES } from '/shared/props.js';
import { distToSegment, fixturePlan, objectPlan, pointInPolygon, rotateYaw } from './geometry.js';
import { buildEquipmentList, buildPatch, equipmentCsv, patchCsv } from './equipment.js';
import { buildMvr } from '/shared/mvr.js';
import { currentPlan, flatPlans } from '/shared/plans.js';

const SVG_NS = 'http://www.w3.org/2000/svg';
// [outil, icône, infobulle]
const TOOLS = [
  ['select', 'select', 'Sélection : clic, Ctrl+clic, cadre de sélection ; glisser pour déplacer'],
  ['rect', 'rect', 'Rectangle (maison, décor…)'],
  ['ellipse', 'ellipse', 'Ellipse ou cercle'],
  ['polygon', 'polygon', 'Forme point par point : double-clic ou Entrée pour terminer'],
  ['freehand', 'freehand', 'Dessin à main levée'],
  ['line', 'line', 'Ligne'],
  ['arrow', 'arrow', 'Flèche (désigner un lieu, un kiosque…)'],
  ['text', 'text', 'Texte'],
  ['measure', 'measure', 'Mesure d’une distance'],
];
const KIND_NAMES = { rect: 'Rectangle', ellipse: 'Ellipse', polygon: 'Forme', freehand: 'Main levée', line: 'Ligne', arrow: 'Flèche', text: 'Texte', measure: 'Mesure' };
const OPT_KEY = 'arklight.plot.options';
const r3 = (v) => Math.round(v * 1000) / 1000;

// Ce qui s'affiche : par défaut le matériel seulement (projecteurs, pieds, grip), comme le plan de feu
// qu'on envoie ; le décor (par catégorie), le studio et les annotations se cochent dans « Affichage ».
const DECOR = PROP_CATEGORIES.map((c) => c.id); // people, animals, vehicles, furniture, set, outdoor, props, filmset
// les sacs de sable restent dans la liste du matériel, pas sur le plan qu'on envoie
const EQUIPMENT_ONLY = { stands: true, grip: true, sandbags: false, studio: false, annotations: false, ...Object.fromEntries(DECOR.map((c) => [c, false])) };
const SHOW_ALL = Object.fromEntries(Object.keys(EQUIPMENT_ONLY).map((k) => [k, true]));

function loadOptions() {
  const defaults = { names: true, fixtures: true, addresses: true, supports: true, beams: true, grid: true, underlay: true, ...EQUIPMENT_ONLY, v: 3 };
  try {
    const saved = JSON.parse(localStorage.getItem(OPT_KEY) || '{}');
    if (!saved.v) delete saved.supports; // pieds affichés par défaut depuis la version 2
    // version 3 : « matériel seulement » par défaut ; les choix d'étiquettes déjà faits sont gardés
    if ((saved.v || 0) < 3) for (const k of [...Object.keys(EQUIPMENT_ONLY), 'underlay']) delete saved[k];
    return { ...defaults, ...saved, v: 3 };
  } catch {
    return defaults;
  }
}

/** Préréglage en cours : « equipment », « all » ou null (personnalisé). */
function presetOf(options) {
  const same = (preset) => Object.entries(preset).every(([k, v]) => !!options[k] === v);
  return same(EQUIPMENT_ONLY) ? 'equipment' : same(SHOW_ALL) ? 'all' : null;
}

function download(name, data, type) {
  const url = URL.createObjectURL(data instanceof Blob ? data : new Blob([data], { type }));
  const a = h('a', { href: url, download: name });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 2000);
}

/** Image de fond en data URL (indispensable pour l'export PNG / impression). */
async function inlineUrl(url) {
  const blob = await (await fetch(url)).blob();
  return new Promise((resolve) => {
    const fr = new FileReader();
    fr.onload = () => resolve(fr.result);
    fr.readAsDataURL(blob);
  });
}

export function mountPlot({ store, net, viewport, actions }) {
  const wrap = document.querySelector('.viewport-wrap');
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.setAttribute('class', 'plot-svg');
  const view = { cx: 0, cz: -1, s: 40, W: 800, H: 600 };
  const options = loadOptions();
  let active = false;
  let tool = 'select';
  let selPlot = null;
  let preview = null;
  let drag = null;
  let fitted = false;
  let dirty = true;
  let calib = null; // calage de l'image : premier point cliqué

  // ------------------------------------------------------------------ barre d'outils
  const toolBtns = new Map();
  // grille du plan : un bouton pour l'afficher ou la masquer (aussi dans Affichage ▾)
  const gridBtn = h('button', { class: `ib toggle${options.grid ? ' active' : ''}`, title: 'Grille (1 m) : afficher / masquer', onclick: () => (setOption('grid', !options.grid), popKind === 'options' && renderPop()) }, icon('grid'));
  // même barre que la vue 3D : sélecteur 3D / plan 2D, puis les outils séparés par de petits traits
  const toolbar = h(
    'div',
    { class: 'plot-toolbar viewbar', role: 'toolbar', 'aria-label': 'Outils du plan de feu' },
    h(
      'div',
      { class: 'mode-switch', role: 'group' },
      h('button', { class: 'mode', 'data-cmd': 'view.plot', 'data-tip': 'Vue 3D', 'aria-pressed': 'false', onclick: () => hide() }, '3D'),
      h('button', { class: 'mode active', 'data-cmd': 'view.plot', 'aria-pressed': 'true' }, icon('plan'), 'Plan 2D'),
    ),
    h('span', { class: 'sep' }),
    TOOLS.map(([id, ico, title]) => {
      const b = h('button', { class: 'plot-tool ib', title, onclick: () => setTool(id) }, icon(ico));
      toolBtns.set(id, b);
      return b;
    }),
    h('span', { class: 'sep' }),
    h('button', { class: 'ib', title: 'Tout voir', onclick: () => fit() }, icon('fit')),
    gridBtn,
    h('button', { class: 'ib', 'data-pop': 'underlay', title: 'Image de fond : photo ou plan à l’échelle', onclick: (e) => togglePop('underlay', e.currentTarget) }, icon('image'), 'Fond'),
    h('button', { class: 'ib', 'data-pop': 'options', title: 'Ce qui s’affiche sur le plan', onclick: (e) => togglePop('options', e.currentTarget) }, icon('layers'), 'Affichage ▾'),
    h('button', { class: 'ib', 'data-pop': 'export', title: 'Exporter le plan, la liste du matériel et le patch', onclick: (e) => togglePop('export', e.currentTarget) }, icon('download'), 'Exporter ▾'),
  );
  const hint = h('div', { class: 'plot-hint' });
  const props = h('div', { class: 'plot-props hidden' });
  const pop = h('div', { class: 'plot-pop hidden' });
  const host = h('div', { class: 'plot hidden' }, toolbar, svg, hint, props, pop);
  wrap.append(host);
  // une fenêtre ouverte (fond, affichage, export) se ferme quand on clique à côté
  document.addEventListener(
    'pointerdown',
    (e) => {
      if (!popKind || pop.contains(e.target) || e.target.closest?.('[data-pop]') || e.target.closest?.('.ctx-root, dialog')) return;
      closePop();
    },
    true,
  );

  function setTool(id) {
    tool = id;
    preview = null;
    calib = null;
    // dessiner une annotation : les annotations doivent être visibles
    if (id !== 'select' && options.annotations === false) {
      setOption('annotations', true);
      toast('Annotations affichées (Affichage ▾ pour les masquer)');
    }
    for (const [k, b] of toolBtns) b.classList.toggle('active', k === id);
    hint.textContent =
      id === 'select'
        ? 'Clic : sélectionner · glisser : déplacer · poignée ⟳ : orienter · glisser dans le vide : cadre de sélection · clic droit glissé ou molette enfoncée : se déplacer · molette : zoom'
        : TOOLS.find((t) => t[0] === id)?.[2] || '';
    svg.style.cursor = id === 'select' ? 'default' : 'crosshair';
    markDirty();
  }

  // ------------------------------------------------------------------ repère écran <-> sol
  const toWorld = (e) => {
    const r = svg.getBoundingClientRect();
    return [view.cx + (e.clientX - r.left - view.W / 2) / view.s, view.cz + (e.clientY - r.top - view.H / 2) / view.s];
  };
  const tol = () => 7 / view.s; // tolérance de clic : 7 px

  function resize() {
    const r = svg.getBoundingClientRect();
    view.W = Math.max(100, Math.round(r.width));
    view.H = Math.max(100, Math.round(r.height));
    svg.setAttribute('viewBox', `0 0 ${view.W} ${view.H}`);
    markDirty();
  }
  new ResizeObserver(resize).observe(svg);

  /** Cadre la vue sur tout ce qui est placé. */
  function fit() {
    Object.assign(view, computeFit(view.W, view.H));
    markDirty();
  }

  /** Centre et échelle qui montrent toute la scène dans un cadre W × H (px). */
  function computeFit(W, H) {
    const xs = [];
    const zs = [];
    for (const f of store.scene?.fixtures || []) {
      xs.push(f.position[0]);
      zs.push(f.position[2]);
    }
    for (const o of store.scene?.objects || []) {
      xs.push(o.position[0]);
      zs.push(o.position[2]);
    }
    for (const it of store.scene?.plot?.items || []) for (const [x, z] of it.points) xs.push(x), zs.push(z);
    if (!xs.length) xs.push(-5, 5), zs.push(-7, 4);
    const minX = Math.min(...xs) - 1.5;
    const maxX = Math.max(...xs) + 1.5;
    const minZ = Math.min(...zs) - 1.5;
    const maxZ = Math.max(...zs) + 1.5;
    return { cx: (minX + maxX) / 2, cz: (minZ + maxZ) / 2, s: Math.max(8, Math.min(200, Math.min(W / (maxX - minX), (H - 60) / (maxZ - minZ)))) };
  }

  // ------------------------------------------------------------------ dessin
  function handles() {
    const ids = store.selectedIds();
    const one = ids.length === 1 ? store.fixtures.get(ids[0]) : null;
    if (one) {
      const p = fixturePlan(one, getFixtureType(one.type));
      if (p.dir) return [{ kind: 'rotate', x: p.center[0] + p.dir[0] * 1.9, z: p.center[1] + p.dir[1] * 1.9, target: { kind: 'fixture', id: one.id } }];
    }
    const o = store.selectedObject();
    if (o && !ids.length) {
      const type = getObjectType(o.type);
      if (isProp(type) || ['frame', 'flag', 'bounce'].includes(type.family)) {
        const a = o.rotation[1];
        const d = Math.max(0.8, (isProp(type) ? Math.max(...type.size.filter((_, i) => i !== 1)) : type.w || 1) / 2 + 0.4);
        return [{ kind: 'rotate', x: o.position[0] + Math.sin(a) * d, z: o.position[2] + Math.cos(a) * d, target: { kind: 'object', id: o.id } }];
      }
    }
    return [];
  }

  function draw() {
    dirty = false;
    if (!active || !store.scene) return;
    svg.innerHTML = renderPlot({
      scene: store.scene,
      view,
      viewport,
      options,
      sel: { fixtures: store.selection, objectId: store.selectedObjectId, plotId: selPlot },
      preview,
      handles: handles(),
    });
    if (drag?.kind === 'marquee') {
      const [a, b] = drag.rect;
      const X = (x) => (x - view.cx) * view.s + view.W / 2;
      const Z = (z) => (z - view.cz) * view.s + view.H / 2;
      const r = document.createElementNS(SVG_NS, 'rect');
      r.setAttribute('x', Math.min(X(a[0]), X(b[0])));
      r.setAttribute('y', Math.min(Z(a[1]), Z(b[1])));
      r.setAttribute('width', Math.abs(X(b[0]) - X(a[0])));
      r.setAttribute('height', Math.abs(Z(b[1]) - Z(a[1])));
      r.setAttribute('class', 'plot-marquee');
      svg.append(r);
    }
  }
  function markDirty() {
    if (dirty || !active) return;
    dirty = true;
    requestAnimationFrame(draw);
  }

  // ------------------------------------------------------------------ ce qui est sous le curseur
  function hitItem(it, p) {
    const P = it.points;
    const t = tol();
    if (it.kind === 'rect' || it.kind === 'ellipse') {
      const [[x0, z0], [x1, z1]] = P;
      const inside = p[0] >= Math.min(x0, x1) - t && p[0] <= Math.max(x0, x1) + t && p[1] >= Math.min(z0, z1) - t && p[1] <= Math.max(z0, z1) + t;
      if (!inside) return false;
      if (it.style?.fill && it.style.fill !== 'none') return true;
      const nearEdge = Math.min(Math.abs(p[0] - x0), Math.abs(p[0] - x1), Math.abs(p[1] - z0), Math.abs(p[1] - z1)) <= t * 1.5;
      return it.kind === 'rect' ? nearEdge : true;
    }
    if (it.kind === 'text') {
      // boîte approximative du texte (ancré en bas à gauche)
      const size = it.style?.size || 16;
      const w = ((it.text || 'Texte').length * size * 0.6) / view.s;
      return p[0] >= P[0][0] - t && p[0] <= P[0][0] + w + t && p[1] <= P[0][1] + t && p[1] >= P[0][1] - size / view.s - t;
    }
    if (it.kind === 'polygon' && pointInPolygon(p, P)) return true;
    for (let i = 0; i < P.length - 1; i++) if (distToSegment(p, P[i], P[i + 1]) <= t) return true;
    if (it.kind === 'polygon' && P.length > 2 && distToSegment(p, P.at(-1), P[0]) <= t) return true;
    return false;
  }

  function hitTest(p) {
    for (const hd of handles()) if (Math.hypot(p[0] - hd.x, p[1] - hd.z) <= tol() * 1.4) return { kind: 'handle', handle: hd };
    const fixtures = store.scene.fixtures;
    for (let i = fixtures.length - 1; i >= 0; i--) {
      const f = fixtures[i];
      const pl = fixturePlan(f, getFixtureType(f.type));
      if (pointInPolygon(p, pl.poly) || Math.hypot(p[0] - pl.center[0], p[1] - pl.center[1]) <= tol() * 1.5) return { kind: 'fixture', id: f.id };
    }
    // objets, puis cadres : un cadre est transparent sur le plan, on le prend par ses tubes ou son centre
    const objs = store.scene.objects || [];
    for (const frames of [false, true]) {
      for (let i = objs.length - 1; i >= 0; i--) {
        const o = objs[i];
        const type = getObjectType(o.type);
        if (!type || !objectShown(type, options) || (type.family === 'frame') !== frames) continue;
        const pl = objectPlan(o, type);
        const near = Math.hypot(p[0] - pl.center[0], p[1] - pl.center[1]) <= tol() * 1.5;
        if (near || (frames ? pl.poly.some((a, k) => distToSegment(p, a, pl.poly[(k + 1) % pl.poly.length]) <= tol() * 1.5) : pointInPolygon(p, pl.poly))) return { kind: 'object', id: o.id };
      }
    }
    const items = options.annotations !== false ? store.scene.plot.items : [];
    for (let i = items.length - 1; i >= 0; i--) if (hitItem(items[i], p)) return { kind: 'plot', id: items[i].id };
    return null;
  }

  // ------------------------------------------------------------------ envoi des modifications
  let lastSend = 0;
  function sendFixtureMoves(final, ids) {
    const now = performance.now();
    if (!final && now - lastSend < 90) return;
    lastSend = now;
    const items = ids.map((id) => {
      const f = store.fixtures.get(id);
      return { id, changes: { position: f.position, rotation: f.rotation } };
    });
    if (items.length) net.send({ t: 'fixtures.update', items });
  }

  function addItem(item) {
    net.send({ t: 'plot.add', item });
  }
  function updateItem(id, changes, local = true) {
    const it = store.scene.plot.items.find((x) => x.id === id);
    if (!it) return;
    if (local) store.setPlotItem({ ...it, ...changes, style: { ...it.style, ...(changes.style || {}) } });
    net.send({ t: 'plot.update', id, changes });
  }
  function removeItem(id) {
    net.send({ t: 'plot.remove', id });
    if (selPlot === id) selectPlot(null);
  }

  // ------------------------------------------------------------------ souris
  let space = false;
  svg.addEventListener('wheel', (e) => {
    e.preventDefault();
    const before = toWorld(e);
    view.s = Math.max(4, Math.min(400, view.s * (e.deltaY < 0 ? 1.15 : 1 / 1.15)));
    const after = toWorld(e);
    view.cx += before[0] - after[0];
    view.cz += before[1] - after[1];
    markDirty();
  });
  svg.addEventListener('contextmenu', (e) => e.preventDefault());

  svg.addEventListener('pointerdown', (e) => {
    svg.setPointerCapture(e.pointerId);
    const p = toWorld(e);
    if (e.button === 1 || e.button === 2 || (e.button === 0 && space)) {
      drag = { kind: 'pan', x: e.clientX, y: e.clientY, cx: view.cx, cz: view.cz, button: e.button, moved: false };
      return;
    }
    if (e.button !== 0) return;
    if (calib) return calibrationClick(p);
    if (tool === 'select') return selectDown(e, p);
    if (tool === 'polygon') {
      if (!preview) preview = { kind: 'polygon', points: [p, p], style: { ...defaultStyle() } };
      else preview.points.splice(preview.points.length - 1, 0, p);
      markDirty();
      return;
    }
    if (tool === 'text') {
      addItem({ kind: 'text', points: [p], text: 'Texte', style: { stroke: '#ffffff', size: 16 } });
      setTool('select');
      return;
    }
    preview = { kind: tool, points: tool === 'freehand' ? [p] : [p, p], style: { ...defaultStyle() } };
    drag = { kind: 'draw' };
    markDirty();
  });

  function defaultStyle() {
    return { stroke: '#ffad3b', fill: 'none', width: 2, dash: false, size: 14 };
  }

  function selectDown(e, p) {
    const hit = hitTest(p);
    const add = e.ctrlKey || e.metaKey || e.shiftKey;
    if (hit?.kind === 'handle') {
      const t = hit.handle.target;
      const el = t.kind === 'fixture' ? store.fixtures.get(t.id) : store.object(t.id);
      const c = [el.position[0], el.position[2]];
      drag = { kind: 'rotate', target: t, center: c, start: Math.atan2(p[1] - c[1], p[0] - c[0]), rotation: [...el.rotation] };
      return;
    }
    if (hit?.kind === 'fixture') {
      selectPlot(null);
      if (add) store.toggleSelect(hit.id);
      else if (!store.isSelected(hit.id)) store.select(hit.id);
      const ids = store.selectedIds();
      drag = { kind: 'move-fixtures', start: p, ids, orig: new Map(ids.map((id) => [id, [...store.fixtures.get(id).position]])), moved: false };
      return;
    }
    if (hit?.kind === 'object') {
      selectPlot(null);
      store.selectObject(hit.id);
      drag = { kind: 'move-object', start: p, id: hit.id, orig: [...store.object(hit.id).position], moved: false };
      return;
    }
    if (hit?.kind === 'plot') {
      selectPlot(hit.id);
      const it = store.scene.plot.items.find((x) => x.id === hit.id);
      drag = { kind: 'move-plot', start: p, id: hit.id, orig: it.points.map((q) => [...q]), moved: false };
      return;
    }
    if (!add) {
      store.select(null);
      store.selectObject(null);
      selectPlot(null);
    }
    drag = { kind: 'marquee', rect: [p, p], add };
  }

  svg.addEventListener('pointermove', (e) => {
    const p = toWorld(e);
    if (tool === 'polygon' && preview && !drag) {
      preview.points[preview.points.length - 1] = p;
      markDirty();
      return;
    }
    if (!drag) return;
    switch (drag.kind) {
      case 'pan':
        view.cx = drag.cx - (e.clientX - drag.x) / view.s;
        view.cz = drag.cz - (e.clientY - drag.y) / view.s;
        drag.moved = drag.moved || Math.hypot(e.clientX - drag.x, e.clientY - drag.y) > 4;
        break;
      case 'draw':
        if (preview.kind === 'freehand') {
          const last = preview.points.at(-1);
          if (Math.hypot(p[0] - last[0], p[1] - last[1]) > 3 / view.s) preview.points.push(p);
        } else preview.points[1] = p;
        break;
      case 'marquee':
        drag.rect[1] = p;
        break;
      case 'move-fixtures': {
        const dx = p[0] - drag.start[0];
        const dz = p[1] - drag.start[1];
        drag.moved = drag.moved || Math.hypot(dx, dz) * view.s > 3;
        if (!drag.moved) break;
        for (const id of drag.ids) {
          const f = store.fixtures.get(id);
          const o = drag.orig.get(id);
          f.position = [r3(o[0] + dx), o[1], r3(o[2] + dz)];
          store.upsertFixture(f, store.clientId);
        }
        sendFixtureMoves(false, drag.ids);
        break;
      }
      case 'move-object': {
        const dx = p[0] - drag.start[0];
        const dz = p[1] - drag.start[1];
        drag.moved = drag.moved || Math.hypot(dx, dz) * view.s > 3;
        if (!drag.moved) break;
        const o = store.object(drag.id);
        o.position = [r3(drag.orig[0] + dx), drag.orig[1], r3(drag.orig[2] + dz)];
        store.upsertObject(o, store.clientId);
        break;
      }
      case 'move-plot': {
        const dx = p[0] - drag.start[0];
        const dz = p[1] - drag.start[1];
        drag.moved = drag.moved || Math.hypot(dx, dz) * view.s > 3;
        if (!drag.moved) break;
        const it = store.scene.plot.items.find((x) => x.id === drag.id);
        if (it) store.setPlotItem({ ...it, points: drag.orig.map(([x, z]) => [r3(x + dx), r3(z + dz)]) });
        break;
      }
      case 'rotate': {
        let a = Math.atan2(p[1] - drag.center[1], p[0] - drag.center[0]) - drag.start;
        if (!e.shiftKey) a = Math.round(a / (Math.PI / 36)) * (Math.PI / 36); // crans de 5° (Maj : libre)
        const rotation = rotateYaw(drag.rotation, -a);
        if (drag.target.kind === 'fixture') {
          const f = store.fixtures.get(drag.target.id);
          f.rotation = rotation;
          store.upsertFixture(f, store.clientId);
        } else {
          const o = store.object(drag.target.id);
          o.rotation = rotation;
          store.upsertObject(o, store.clientId);
        }
        drag.rotationNow = rotation;
        break;
      }
      default:
        break;
    }
    markDirty();
  });

  svg.addEventListener('pointerup', (e) => {
    const d = drag;
    drag = null;
    if (!d) return;
    switch (d.kind) {
      case 'pan':
        if (!d.moved && d.button === 2) contextMenu(e);
        break;
      case 'draw': {
        const pv = preview;
        preview = null;
        const [a, b] = [pv.points[0], pv.points.at(-1)];
        const size = Math.hypot(b[0] - a[0], b[1] - a[1]) * view.s;
        if (pv.kind === 'freehand' ? pv.points.length > 2 : size > 6) {
          addItem({ kind: pv.kind, points: pv.points, style: pv.style });
          if (pv.kind !== 'freehand') setTool('select');
        }
        break;
      }
      case 'marquee': {
        const [a, b] = d.rect;
        const [x0, x1] = [Math.min(a[0], b[0]), Math.max(a[0], b[0])];
        const [z0, z1] = [Math.min(a[1], b[1]), Math.max(a[1], b[1])];
        if ((x1 - x0) * view.s > 4 || (z1 - z0) * view.s > 4) {
          const ids = store.scene.fixtures.filter((f) => f.position[0] >= x0 && f.position[0] <= x1 && f.position[2] >= z0 && f.position[2] <= z1).map((f) => f.id);
          store.setSelection(d.add ? [...store.selection, ...ids] : ids);
        }
        break;
      }
      case 'move-fixtures':
        if (d.moved) {
          sendFixtureMoves(true, d.ids);
          viewport.dispatchEvent(new CustomEvent('transform', { detail: { ids: d.ids } }));
        }
        break;
      case 'move-object':
        if (d.moved) net.send({ t: 'object.update', id: d.id, changes: { position: store.object(d.id).position } });
        break;
      case 'move-plot':
        if (d.moved) {
          const it = store.scene.plot.items.find((x) => x.id === d.id);
          if (it) updateItem(d.id, { points: it.points }, false);
        }
        break;
      case 'rotate':
        if (d.rotationNow) {
          if (d.target.kind === 'fixture') net.send({ t: 'fixture.update', id: d.target.id, changes: { rotation: d.rotationNow } });
          else net.send({ t: 'object.update', id: d.target.id, changes: { rotation: d.rotationNow } });
        }
        break;
      default:
        break;
    }
    markDirty();
  });

  svg.addEventListener('dblclick', () => {
    if (tool === 'polygon') finishPolygon();
  });

  function finishPolygon() {
    if (!preview || preview.kind !== 'polygon') return;
    const points = preview.points.slice(0, -1);
    // le double-clic ajoute deux fois le dernier point
    while (points.length > 1 && Math.hypot(points.at(-1)[0] - points.at(-2)[0], points.at(-1)[1] - points.at(-2)[1]) < 1e-3) points.pop();
    preview = null;
    if (points.length >= 3) addItem({ kind: 'polygon', points, style: { ...defaultStyle(), fill: '#ffad3b' } });
    setTool('select');
  }

  function contextMenu(e) {
    const p = toWorld(e);
    const hit = hitTest(p);
    if (hit?.kind === 'fixture') {
      if (!store.isSelected(hit.id)) store.select(hit.id);
      return openContextMenu(e.clientX, e.clientY, actions.selectionMenu());
    }
    if (hit?.kind === 'object') {
      store.selectObject(hit.id);
      return openContextMenu(e.clientX, e.clientY, actions.objectMenu(hit.id));
    }
    if (hit?.kind === 'plot') {
      selectPlot(hit.id);
      return openContextMenu(e.clientX, e.clientY, [
        { header: KIND_NAMES[store.scene.plot.items.find((x) => x.id === hit.id)?.kind] || 'Élément' },
        { label: 'Dupliquer', onClick: () => duplicateItem(hit.id) },
        { label: 'Supprimer', shortcut: 'Suppr', danger: true, onClick: () => removeItem(hit.id) },
      ]);
    }
    openContextMenu(e.clientX, e.clientY, actions.selectionMenu());
  }

  function duplicateItem(id) {
    const it = store.scene.plot.items.find((x) => x.id === id);
    if (it) addItem({ ...it, id: undefined, points: it.points.map(([x, z]) => [x + 0.5, z + 0.5]) });
  }

  // ------------------------------------------------------------------ propriétés de l'élément sélectionné
  function selectPlot(id) {
    selPlot = id;
    if (id) {
      store.select(null);
      store.selectObject(null);
    }
    renderProps();
    markDirty();
  }

  function renderProps() {
    const it = selPlot && store.scene.plot.items.find((x) => x.id === selPlot);
    props.classList.toggle('hidden', !it);
    if (!it) return props.replaceChildren();
    const st = it.style || {};
    const closed = ['rect', 'ellipse', 'polygon', 'freehand'].includes(it.kind);
    const textInput = h('input', {
      class: 'plot-text',
      value: it.text || '',
      placeholder: it.kind === 'text' ? 'Texte' : 'Étiquette (facultatif)',
      oninput: (e) => updateItem(it.id, { text: e.target.value }),
    });
    props.replaceChildren(
      ...[
      h('strong', {}, KIND_NAMES[it.kind] || it.kind),
      textInput,
      h('label', { title: 'Couleur' }, h('input', { type: 'color', value: /^#[0-9a-f]{6}$/i.test(st.stroke) ? st.stroke : '#ffad3b', oninput: (e) => updateItem(it.id, { style: { stroke: e.target.value, ...(st.fill && st.fill !== 'none' ? { fill: e.target.value } : {}) } }) })),
      closed
        ? h('label', {}, h('input', { type: 'checkbox', checked: st.fill && st.fill !== 'none', onchange: (e) => updateItem(it.id, { style: { fill: e.target.checked ? st.stroke || '#ffad3b' : 'none' } }) }), ' remplie')
        : null,
      it.kind !== 'text' ? h('label', {}, h('input', { type: 'checkbox', checked: !!st.dash, onchange: (e) => updateItem(it.id, { style: { dash: e.target.checked } }) }), ' pointillés') : null,
      it.kind !== 'text'
        ? h('label', {}, 'trait ', h('select', { onchange: (e) => updateItem(it.id, { style: { width: Number(e.target.value) } }) }, [1, 2, 3, 5, 8].map((w) => h('option', { value: w, selected: (st.width || 2) === w }, `${w}`))))
        : h('label', {}, 'taille ', h('select', { onchange: (e) => updateItem(it.id, { style: { size: Number(e.target.value) } }) }, [12, 16, 20, 28, 40, 60].map((w) => h('option', { value: w, selected: (st.size || 16) === w }, `${w}`)))),
      closed
        ? h('label', { title: 'Hauteur du volume dans la vue 3D (0 : forme plate, seulement sur le plan)' }, 'hauteur 3D ', h('input', { type: 'number', min: 0, max: 100, step: 0.5, value: it.height || 0, onchange: (e) => updateItem(it.id, { height: Math.max(0, Number(e.target.value) || 0) }) }), ' m')
        : null,
      h('button', { title: 'Supprimer (Suppr)', onclick: () => removeItem(it.id) }, '🗑'),
      ].filter(Boolean),
    );
    if (it.kind === 'text' && it.text === 'Texte') requestAnimationFrame(() => textInput.select());
  }

  // ------------------------------------------------------------------ fenêtres : fond, affichage, export
  let popKind = null;
  function closePop() {
    popKind = null;
    pop.classList.add('hidden');
  }
  function togglePop(kind, anchor) {
    if (popKind === kind) return closePop();
    popKind = kind;
    pop.classList.remove('hidden');
    pop.style.left = `${anchor.offsetLeft}px`;
    renderPop();
  }

  function renderPop() {
    if (popKind === 'options') {
      const box = (key, label) => h('label', {}, h('input', { type: 'checkbox', checked: !!options[key], onchange: (e) => setOption(key, e.target.checked) }), ` ${label}`);
      const preset = presetOf(options);
      const apply = (values) => {
        for (const [k, v] of Object.entries(values)) options[k] = v;
        setOption('v', 3);
        renderPop();
      };
      const presetBtn = (id, label, values, title) => h('button', { class: preset === id ? 'active' : '', title, onclick: () => apply(values) }, label);
      pop.replaceChildren(
        h(
          'div',
          { class: 'plot-presets' },
          presetBtn('equipment', 'Matériel seulement', EQUIPMENT_ONLY, 'Projecteurs, pieds et grip : le plan de feu qu’on envoie'),
          presetBtn('all', 'Tout afficher', SHOW_ALL, 'Avec le décor, le studio et les annotations'),
        ),
        h('div', { class: 'plot-pop-group' }, h('strong', {}, 'Matériel'), box('names', 'Noms'), box('fixtures', 'Numéros de fixture (Fix)'), box('addresses', 'Adresses DMX'), box('supports', 'Pieds et hauteurs'), box('beams', 'Faisceaux'), box('stands', 'Pieds seuls'), box('grip', 'Grip (cadres, drapeaux, réflecteurs…)'), box('sandbags', 'Sacs de sable')),
        h('div', { class: 'plot-pop-group' }, h('strong', {}, 'Décor'), PROP_CATEGORIES.map((c) => box(c.id, c.label)), box('studio', 'Studio (cyclo)')),
        h('div', { class: 'plot-pop-group' }, h('strong', {}, 'Plan'), box('annotations', 'Annotations (formes, textes, flèches, mesures)'), box('underlay', 'Image de fond'), box('grid', 'Grille (1 m)')),
      );
    } else if (popKind === 'underlay') {
      const u = store.scene.plot.underlay;
      const file = h('input', { type: 'file', accept: 'image/png,image/jpeg,image/webp', class: 'hidden', onchange: (e) => uploadUnderlay(e.target.files[0]) });
      const setU = (changes) => net.send({ t: 'plot.meta', underlay: { ...u, ...changes } });
      // couleur unie du fond : les projecteurs et les traits s'adaptent pour rester visibles
      const bg = store.scene.plot.background || null;
      const setBg = (v) => net.send({ t: 'plot.meta', background: v });
      const swatch = (value, label, css) => h('button', { class: `plot-bg${bg === value ? ' active' : ''}`, title: label, onclick: () => setBg(value) }, h('i', { style: { background: css } }), label);
      pop.replaceChildren(
        ...[
        h(
          'div',
          { class: 'plot-bgs' },
          h('strong', {}, 'Couleur du fond'),
          h(
            'div',
            { class: 'plot-bg-row' },
            swatch(null, 'Auto', 'linear-gradient(135deg, #0f1216 50%, #ffffff 50%)'),
            swatch('#000000', 'Noir', '#000000'),
            swatch('#2b2f36', 'Gris foncé', '#2b2f36'),
            swatch('#c9ccd1', 'Gris clair', '#c9ccd1'),
            swatch('#ffffff', 'Blanc', '#ffffff'),
            h('label', { class: 'plot-bg', title: 'Autre couleur' }, h('input', { type: 'color', value: bg || '#1d2b3a', onchange: (e) => setBg(e.target.value) }), 'Autre…'),
          ),
          h('div', { class: 'note' }, 'Auto : sombre à l’écran, blanc à l’export. Une couleur choisie sert aussi à l’export.'),
        ),
        h('div', { class: 'note' }, 'Photo ou plan vu de dessus. Cale-le à l’échelle avec deux points dont tu connais la distance.'),
        file,
        h('button', { onclick: () => file.click() }, u ? 'Changer d’image…' : 'Importer une image…'),
        u
          ? h(
              'div',
              { class: 'plot-pop-rows' },
              h('button', { class: 'primary', onclick: () => startCalibration() }, 'Caler l’échelle (2 points)'),
              h('label', {}, 'Largeur ', h('input', { type: 'number', step: 0.5, min: 0.5, value: u.width, onchange: (e) => setU({ width: Number(e.target.value) || u.width }) }), ' m'),
              h('label', {}, 'Rotation ', h('input', { type: 'number', step: 1, value: u.rotation || 0, onchange: (e) => setU({ rotation: Number(e.target.value) || 0 }) }), ' °'),
              h('label', {}, 'Opacité ', h('input', { type: 'range', min: 0.05, max: 1, step: 0.05, value: u.opacity ?? 0.6, oninput: (e) => setU({ opacity: Number(e.target.value) }) })),
              h('label', {}, h('input', { type: 'checkbox', checked: !!u.floor3d, onchange: (e) => setU({ floor3d: e.target.checked }) }), ' Afficher au sol dans la vue 3D'),
              h('button', { onclick: () => setU({ x: view.cx, z: view.cz }) }, 'Centrer sur la vue'),
              h('button', { onclick: () => net.send({ t: 'plot.meta', underlay: null }) }, 'Retirer l’image'),
            )
          : null,
        ].filter(Boolean),
      );
    } else if (popKind === 'export') {
      const title = store.scene.plot.title || {};
      const setT = (k, v) => net.send({ t: 'plot.meta', title: { ...title, [k]: v } });
      pop.replaceChildren(
        h('label', {}, 'Production ', h('input', { value: title.production || '', onchange: (e) => setT('production', e.target.value) })),
        h('label', {}, 'Chef électro ', h('input', { value: title.author || '', onchange: (e) => setT('author', e.target.value) })),
        h('div', { class: 'plot-pop-rows' },
          h('button', { class: 'primary', onclick: () => printSheet() }, 'Imprimer / PDF (plan + liste + patch)'),
          h('button', { onclick: () => exportImage('png') }, 'Plan en image PNG'),
          h('button', { onclick: () => exportImage('svg') }, 'Plan en SVG (vectoriel)'),
          h('button', { onclick: () => exportEquipmentCsv() }, 'Liste du matériel (CSV)'),
          h('button', { onclick: () => exportPatchCsv() }, 'Patch DMX (CSV)'),
        ),
        h(
          'div',
          { class: 'plot-pop-group' },
          h('strong', {}, 'Vers une console ou une appli'),
          h('button', { class: 'primary', onclick: () => exportMvr() }, 'MVR (Blackout, grandMA3, Capture…)'),
          h('div', { class: 'note' }, 'Patch, numéros de fixture, positions et orientations. Chaque modèle est décrit en GDTF.'),
        ),
      );
    }
  }

  function setOption(key, value) {
    options[key] = value;
    gridBtn.classList.toggle('active', !!options.grid);
    try {
      localStorage.setItem(OPT_KEY, JSON.stringify(options));
    } catch {
      /* préférence non mémorisée */
    }
    markDirty();
  }

  async function uploadUnderlay(fileObj) {
    if (!fileObj) return;
    try {
      const res = await fetch('/upload', { method: 'POST', headers: { 'Content-Type': fileObj.type }, body: fileObj });
      const json = await res.json();
      if (!res.ok) throw new Error(json.error || res.statusText);
      const img = new Image();
      img.src = json.url;
      await img.decode();
      net.send({ t: 'plot.meta', underlay: { url: json.url, x: view.cx, z: view.cz, width: 20, aspect: img.naturalWidth / img.naturalHeight, rotation: 0, opacity: 0.6, floor3d: false } });
      toast('Image importée : cale-la à l’échelle avec deux points connus');
    } catch (err) {
      toast(`Import impossible : ${err.message}`, 'error');
    }
  }

  function startCalibration() {
    calib = { points: [] };
    pop.classList.add('hidden');
    popKind = null;
    hint.textContent = 'Calage : clique le premier point de l’image dont tu connais la distance…';
    svg.style.cursor = 'crosshair';
  }

  function calibrationClick(p) {
    calib.points.push(p);
    if (calib.points.length === 1) {
      hint.textContent = 'Calage : clique le second point…';
      return;
    }
    const [a, b] = calib.points;
    const measured = Math.hypot(b[0] - a[0], b[1] - a[1]);
    const input = h('input', { type: 'number', step: 0.1, min: 0.1, placeholder: 'mètres', class: 'plot-calib' });
    const ok = h('button', { class: 'primary' }, 'Appliquer');
    const apply = () => {
      const real = Number(input.value);
      if (!(real > 0) || !(measured > 0)) return;
      const u = store.scene.plot.underlay;
      const k = real / measured;
      // le premier point reste en place, l'image grandit ou rétrécit autour de lui
      net.send({ t: 'plot.meta', underlay: { ...u, width: u.width * k, x: a[0] + (u.x - a[0]) * k, z: a[1] + (u.z - a[1]) * k } });
      calib = null;
      setTool('select');
      renderProps();
      toast('Image mise à l’échelle');
    };
    ok.addEventListener('click', apply);
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') apply();
    });
    props.classList.remove('hidden');
    props.replaceChildren(h('strong', {}, 'Distance réelle entre les deux points'), input, h('span', {}, 'm'), ok, h('button', { onclick: () => ((calib = null), setTool('select'), renderProps()) }, 'Annuler'));
    requestAnimationFrame(() => input.focus());
  }

  // ------------------------------------------------------------------ export
  // séquence et plan (projet à plusieurs plans) : dans le cartouche et le nom des fichiers
  const planText = () => {
    const cur = flatPlans(store.plans).length > 1 ? currentPlan(store.plans) : null;
    return cur ? `${cur.seq.name} › ${cur.plan.name}` : '';
  };
  const docTitle = (title) => [title.production || store.scene.name, planText()].filter(Boolean).join(' — ');
  const slug = () =>
    [store.scene.name || 'plan', planText()]
      .join(' ')
      .normalize('NFD')
      .replace(/[\u0300-\u036f]/g, '')
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '') || 'plan';
  const exportEquipmentCsv = () => download(t('liste-materiel-{0}.csv', { 0: slug() }), equipmentCsv(buildEquipmentList(store.scene, viewport), t), 'text/csv');
  const exportPatchCsv = () => download(t('patch-{0}.csv', { 0: slug() }), patchCsv(buildPatch(store.scene), t), 'text/csv');

  /** MVR (My Virtual Rig) : le patch et les positions pour Blackout, grandMA3, Capture, Vectorworks… */
  function exportMvr() {
    const { bytes, fixtures, skipped } = buildMvr(store.scene, { providerVersion: store.version || '', layerName: t('Projecteurs') });
    if (!fixtures) return toast(t('Aucun projecteur à exporter'), 'error');
    download(`${slug()}.mvr`, new Blob([bytes], { type: 'application/octet-stream' }));
    toast(t('MVR exporté : {0} projecteurs{1}', { 0: fixtures, 1: skipped.length ? t(' ({0} sans DMX ignorés)', { 0: skipped.length }) : '' }), 'info', 4000);
  }

  /** SVG du plan pour l'export : fond blanc, cadré sur la scène, avec cartouche. */
  async function exportSvg(W = 1754, H = 1240) {
    const fitted = computeFit(W, H);
    const exportView = { ...fitted, s: fitted.s * 0.92, W, H };
    const scene = { ...store.scene, plot: { ...store.scene.plot, underlay: store.scene.plot.underlay ? { ...store.scene.plot.underlay, url: await inlineUrl(store.scene.plot.underlay.url) } : null } };
    const body = renderPlot({ scene, view: exportView, viewport, theme: 'print', options, sel: {} });
    const date = new Date().toLocaleDateString(lang === 'en' ? 'en-GB' : 'fr-FR');
    const tb = titleBlock({ W, H, title: { ...(store.scene.plot.title || {}), scene: store.scene.name, plan: planText() }, date, background: store.scene.plot.background });
    // textes du plan et du cartouche dans la langue de l'interface
    return tMarkup(`<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" font-family="Segoe UI, Arial, sans-serif">${body}${tb}</svg>`);
  }

  async function exportImage(kind) {
    const svgText = await exportSvg();
    if (kind === 'svg') return download(t('plan-de-feu-{0}.svg', { 0: slug() }), svgText, 'image/svg+xml');
    const img = new Image();
    img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svgText)}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width;
    c.height = img.height;
    c.getContext('2d').drawImage(img, 0, 0);
    c.toBlob((blob) => download(t('plan-de-feu-{0}.png', { 0: slug() }), blob), 'image/png');
  }

  async function printSheet() {
    const w = window.open('', '_blank');
    if (!w) return toast(t('Autorise les fenêtres pop-up pour imprimer'), 'error');
    const svgText = await exportSvg();
    const list = buildEquipmentList(store.scene, viewport);
    const patch = buildPatch(store.scene);
    const esc = (s) => String(s ?? '').replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' })[c]);
    const title = store.scene.plot.title || {};
    w.document.write(tMarkup(`<!doctype html><html lang="${lang}"><head><meta charset="utf-8"><title>Plan de feu — ${esc(docTitle(title))}</title>
<style>
  body { font: 12px "Segoe UI", Arial, sans-serif; color: #111; margin: 0; }
  .page { page-break-after: always; padding: 10mm; }
  .page:last-child { page-break-after: auto; }
  svg { width: 100%; height: auto; border: 1px solid #ccc; }
  h1 { font-size: 18px; margin: 0 0 4mm; } h2 { font-size: 14px; margin: 6mm 0 2mm; }
  table { border-collapse: collapse; width: 100%; } th, td { border: 1px solid #bbb; padding: 3px 6px; text-align: left; }
  th { background: #f0f0f0; } td.q { text-align: right; width: 18mm; font-weight: 700; }
  @page { size: A4 landscape; margin: 8mm; }
</style></head><body>
<div class="page">${svgText}</div>
<div class="page"><h1>Liste du matériel — ${esc(docTitle(title))}</h1>
${list.sections.map((s) => `<h2>${esc(s.title)}</h2><table><tr><th>Désignation</th><th>Qté</th><th>Remarques</th></tr>${s.rows.map((r) => `<tr><td>${esc(r.name)}</td><td class="q">${r.qty}</td><td>${esc(r.note || '')}</td></tr>`).join('')}</table>`).join('')}
<h2>Patch DMX</h2><table><tr><th>Univers</th><th>Adresses</th><th>Projecteur</th><th>Modèle</th><th>Mode</th></tr>${patch.map((p) => `<tr><td>${p.universe}</td><td>${p.address}–${p.address + p.footprint - 1}</td><td>${esc(p.name)}</td><td>${esc(p.model)}</td><td>${esc(p.mode)}</td></tr>`).join('')}</table>
</div></body></html>`));
    w.document.close();
    setTimeout(() => w.print(), 400);
  }

  // ------------------------------------------------------------------ clavier
  window.addEventListener(
    'keydown',
    (e) => {
      if (!active || document.querySelector('dialog[open]')) return;
      if (['INPUT', 'SELECT', 'TEXTAREA'].includes(document.activeElement?.tagName)) return;
      if (e.key === ' ') space = true;
      if (e.key === 'Escape') {
        if (preview || calib || tool !== 'select') {
          e.stopPropagation();
          setTool('select');
          renderProps();
          return;
        }
        if (selPlot) {
          e.stopPropagation();
          selectPlot(null);
        }
      } else if (e.key === 'Enter' && tool === 'polygon') {
        e.stopPropagation();
        finishPolygon();
      } else if ((e.key === 'Delete' || e.key === 'Backspace') && selPlot) {
        e.stopPropagation();
        removeItem(selPlot);
      }
    },
    true,
  );
  window.addEventListener('keyup', (e) => {
    if (e.key === ' ') space = false;
  });

  // ------------------------------------------------------------------ mises à jour
  for (const ev of ['scene', 'fixture-added', 'fixture-updated', 'fixture-removed', 'selection', 'object-added', 'object-updated', 'object-removed', 'object-selection', 'env', 'groups']) store.addEventListener(ev, markDirty);
  store.addEventListener('plot', () => {
    if (selPlot && !store.scene.plot.items.some((x) => x.id === selPlot)) selPlot = null;
    // pas de nouveau rendu des réglages pendant une saisie : le champ perdrait le curseur à chaque lettre
    const editing = (el) => el.contains(document.activeElement) && ['INPUT', 'SELECT', 'TEXTAREA'].includes(document.activeElement.tagName);
    if (!drag && !(selPlot && editing(props))) renderProps();
    if ((popKind === 'underlay' || popKind === 'export') && !editing(pop)) renderPop();
    markDirty();
  });
  store.addEventListener('plot-item-created', (e) => selectPlot(e.detail.id));
  setInterval(() => active && markDirty(), 500); // couleurs des faisceaux

  function show() {
    active = true;
    host.classList.remove('hidden');
    wrap.classList.add('plot-mode');
    viewport.paused = true;
    resize();
    if (!fitted) {
      fit();
      fitted = true;
    }
    setTool(tool);
    draw();
  }
  function hide() {
    active = false;
    host.classList.add('hidden');
    wrap.classList.remove('plot-mode');
    viewport.paused = false;
    pop.classList.add('hidden');
    popKind = null;
  }
  return {
    show,
    hide,
    toggle: () => (active ? hide() : show()),
    exportSvg,
    // exports (menu Fichier › Plan de feu), aussi quand le plan n'est pas affiché
    print: () => printSheet(),
    exportPng: () => exportImage('png'),
    exportMvr: () => exportMvr(),
    exportSvgFile: () => exportImage('svg'),
    equipmentCsv: () => exportEquipmentCsv(),
    patchCsv: () => exportPatchCsv(),
    /** Position à l'écran (px client) d'un point du sol : utile pour les tests. */
    toScreen(x, z) {
      const r = svg.getBoundingClientRect();
      return [r.left + (x - view.cx) * view.s + view.W / 2, r.top + (z - view.cz) * view.s + view.H / 2];
    },
    get active() {
      return active;
    },
  };
}
