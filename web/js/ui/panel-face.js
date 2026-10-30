// Face avant d'un panneau de commande (shared/onboard/panel.js) : voyants, touches, molettes et écran,
// disposés comme sur le projecteur ; l'écran est dessiné à partir des primitives de la marque.
import { h } from './dom.js';

const SCALE = 3; // pixels du canevas par pixel de l'écran simulé (net à toutes les tailles)
const now = () => performance.now() / 1000;

/** Couleur d'une primitive : nom de la palette ou couleur directe. */
const colorOf = (pal, c, dflt) => (!c ? pal[dflt] : pal[c] || c);

/** Dessine l'écran : { off, dim, prims }. */
export function drawPanelDisplay(canvas, model, display) {
  const pal = display.palette;
  const g = canvas.getContext('2d');
  const W = display.w;
  const H = display.h;
  g.setTransform(SCALE, 0, 0, SCALE, 0, 0);
  g.fillStyle = model?.off ? pal.off || '#111' : pal.bg;
  g.fillRect(0, 0, W, H);
  if (!model || model.off) return;
  g.textBaseline = 'top';
  for (const p of model.prims || []) {
    if (p.t === 'rect') {
      if (p.fill) {
        g.fillStyle = colorOf(pal, p.fill, 'fg');
        g.fillRect(p.x, p.y, p.w, p.h);
      }
      if (p.stroke) {
        g.strokeStyle = colorOf(pal, p.stroke, 'fg');
        g.lineWidth = p.lw || 1;
        g.strokeRect(p.x + 0.5, p.y + 0.5, p.w - 1, p.h - 1);
      }
    } else if (p.t === 'line') {
      g.strokeStyle = colorOf(pal, p.color, 'fg');
      g.lineWidth = p.w || 1;
      g.beginPath();
      g.moveTo(p.x1, p.y1);
      g.lineTo(p.x2, p.y2);
      g.stroke();
    } else if (p.t === 'ring') {
      g.lineWidth = p.w || 4;
      g.strokeStyle = colorOf(pal, p.track, 'dim');
      g.beginPath();
      g.arc(p.x, p.y, p.r, 0, Math.PI * 2);
      g.stroke();
      if (p.v > 0) {
        g.strokeStyle = colorOf(pal, p.color, 'accent');
        g.beginPath();
        g.arc(p.x, p.y, p.r, -Math.PI / 2, -Math.PI / 2 + Math.PI * 2 * Math.min(1, p.v));
        g.stroke();
      }
    } else if (p.t === 'text') {
      const family = p.font === 'seg' || p.font === 'mono' ? 'Consolas, "Cascadia Mono", monospace' : '"Segoe UI", Arial, sans-serif';
      let size = p.s || 10;
      const weight = p.bold ? 'bold ' : '';
      g.font = `${weight}${size}px ${family}`;
      if (p.spacing) g.letterSpacing = `${p.spacing}px`;
      else g.letterSpacing = '0px';
      if (p.w) {
        while (g.measureText(p.text).width > p.w && size > 5) g.font = `${weight}${--size}px ${family}`;
      }
      g.fillStyle = p.inv ? pal.bg : colorOf(pal, p.color, 'fg');
      g.textAlign = p.align === 'c' ? 'center' : p.align === 'r' ? 'right' : 'left';
      g.fillText(String(p.text ?? ''), p.x, p.y + (p.s - size) / 2);
    }
  }
  g.textAlign = 'left';
  g.letterSpacing = '0px';
  if (model.dim) {
    g.fillStyle = 'rgba(0, 0, 0, 0.55)';
    g.fillRect(0, 0, W, H);
  }
}

/** Touche : appui, relâchement (les appuis longs sont mesurés par le contrôleur). */
function keyEl(ctrl, k, holdBars) {
  const bar = h('i');
  holdBars.set(k.id, bar);
  const btn = h('button', { class: `pk-key ${k.shape || ''}`, title: k.title || k.label }, h('span', { class: 'pk-cap' }), k.label ? h('small', {}, k.label) : null, h('span', { class: 'hold' }, bar, h('em')));
  const up = () => {
    if (!btn.classList.contains('down')) return;
    btn.classList.remove('down');
    ctrl.release(k.id, now());
  };
  btn.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    btn.setPointerCapture?.(e.pointerId);
    btn.classList.add('down');
    ctrl.press(k.id, now());
  });
  btn.addEventListener('pointerup', up);
  btn.addEventListener('pointercancel', up);
  return btn;
}

/**
 * Molette : la roulette de la souris tourne d'un cran, glisser (haut / droite) tourne plus vite ;
 * un clic sans glisser appuie (molettes cliquables), maintenu = appui long.
 */
function knobEl(ctrl, k) {
  let angle = 0;
  const dial = h('div', { class: 'pk-dial' }, k.pointer || k.cap !== 'blue' ? h('i', { class: 'pk-pointer' }) : null);
  const el = h('div', { class: `pk-knob ${k.size || 'm'} ${k.cap || ''}`, title: `${k.label || 'Molette'} : roulette ou glisser pour tourner${k.push ? ', clic pour appuyer' : ''}` }, dial, k.label ? h('small', {}, k.label) : null);
  const turn = (steps) => {
    ctrl.turn(k.id, steps, now());
    angle += steps * 12;
    dial.style.transform = `rotate(${angle}deg)`;
  };
  el.addEventListener(
    'wheel',
    (e) => {
      e.preventDefault();
      turn(e.deltaY < 0 ? 1 : -1);
    },
    { passive: false },
  );
  let drag = null;
  el.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    el.setPointerCapture?.(e.pointerId);
    drag = { x: e.clientX, y: e.clientY, moved: 0, acc: 0 };
    if (k.push) {
      el.classList.add('down');
      ctrl.press(k.id, now());
    }
  });
  el.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const d = e.clientX - drag.x - (e.clientY - drag.y);
    drag.x = e.clientX;
    drag.y = e.clientY;
    drag.acc += d;
    const steps = Math.trunc(drag.acc / 6);
    if (steps) {
      drag.acc -= steps * 6;
      if (!drag.moved && k.push) ctrl.cancel(k.id); // on tourne : pas d'appui
      drag.moved += Math.abs(steps);
      turn(steps);
    }
  });
  const up = () => {
    if (!drag) return;
    el.classList.remove('down');
    if (k.push && !drag.moved) ctrl.release(k.id, now());
    drag = null;
  };
  el.addEventListener('pointerup', up);
  el.addEventListener('pointercancel', up);
  return el;
}

/** La face garde sa disposition et se réduit pour tenir dans la largeur de l'inspecteur. */
export function fitPanel(face) {
  const parent = face.parentElement;
  if (!parent || face._fit) return;
  const fit = () => {
    face.style.zoom = '1';
    const natural = face.offsetWidth;
    const cs = getComputedStyle(parent);
    const avail = parent.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight) - 2;
    if (natural > 0 && avail > 0) face.style.zoom = String(Math.min(1, avail / natural));
  };
  face._fit = new ResizeObserver(fit);
  face._fit.observe(parent);
  fit();
}

/**
 * Construit la face avant : rangées d'éléments (voyant, touche, molette, écran, texte) et colonnes.
 * @returns {{ face: HTMLElement, canvas: HTMLCanvasElement|null, leds: Map, holdBars: Map }}
 */
export function buildPanelFace(ctrl) {
  const def = ctrl.def;
  const leds = new Map();
  const holdBars = new Map();
  let canvas = null;
  const elOf = (it) => {
    if (it.model && it.model !== ctrl.type.id) return null; // élément propre à un autre modèle de la famille
    if (it.col) return h('div', { class: `pf-col${it.row ? ' as-row' : ''}` }, it.col.map(elOf));
    if (it.el === 'display') {
      canvas = h('canvas', { class: 'pf-display', width: def.display.w * SCALE, height: def.display.h * SCALE, style: { width: `${def.display.w * (def.display.zoom || 2)}px` } });
      return canvas;
    }
    if (it.el === 'led') {
      const dot = h('span', { class: 'led' });
      leds.set(it.id, dot);
      return h('span', { class: 'pf-led', title: it.title || it.label }, dot, it.label ? h('small', {}, it.label) : null);
    }
    if (it.el === 'key') return keyEl(ctrl, it, holdBars);
    if (it.el === 'knob') return knobEl(ctrl, it);
    if (it.el === 'text') return h('span', { class: `pf-text ${it.cls || ''}` }, it.text);
    return h('span', { class: 'pf-gap' });
  };
  const face = h('div', { class: `pf pf-${def.face.style || 'generic'}` }, def.face.rows.map((row) => h('div', { class: 'pf-row' }, row.map(elOf))));
  return { face, canvas, leds, holdBars, note: def.face.note || '' };
}
