// Décor et extérieur : pans de mur (avec porte, avec fenêtre), escalier, praticable, colonne,
// borniol, arbres, buissons, rochers, mobilier urbain. Face avant vers +Z, posé au sol.
import * as THREE from 'three';
import { Kit, rng, shade } from './kit.js';

const frameMat = (k) => k.mat('doorframe', '#f1efe9', { rough: 0.6 });
const glassMat = (k) => k.mat('glass', '#9fb8c8', { rough: 0.05, metal: 0.2, opacity: 0.35 });

/** Mur de largeur W et hauteur H, avec ou sans baie (x0..x1, y0..y1), en panneaux simples. */
function wallWithOpening(k, mat, W, H, T, hole) {
  if (!hole) return k.box(W, H, T, mat, [0, H / 2, 0]);
  const [x0, x1, y0, y1] = hole;
  const part = (xa, xb, ya, yb) => xb - xa > 0.001 && yb - ya > 0.001 && k.box(xb - xa, yb - ya, T, mat, [(xa + xb) / 2, (ya + yb) / 2, 0]);
  part(-W / 2, x0, 0, H);
  part(x1, W / 2, 0, H);
  part(x0, x1, y1, H);
  part(x0, x1, 0, y0);
}

function wall(type, props) {
  const k = new Kit();
  const paint = k.mat('paint', props.color || '#d9d4c8', { rough: 0.9 });
  const W = props.width ?? type.size[0];
  const H = props.height ?? type.size[1];
  const T = 0.1;
  const trim = k.mat('trim', shade(props.color || '#d9d4c8', 0.75), { rough: 0.8 });
  if (type.model === 'walldoor') {
    const dw = 0.9;
    wallWithOpening(k, paint, W, H, T, [-dw / 2, dw / 2, 0, 2.05]);
    doorLeaf(k, dw, 2.05, T, 0.55, '#f1efe9');
  } else if (type.model === 'wallwindow') {
    const ww = Math.min(1.2, W - 0.4);
    wallWithOpening(k, paint, W, H, T, [-ww / 2, ww / 2, 0.95, 2.15]);
    windowFrame(k, ww, 1.2, T, 0.95, '#f1efe9');
  } else wallWithOpening(k, paint, W, H, T, null);
  k.box(W, 0.1, T + 0.02, trim, [0, 0.05, 0]); // plinthe
  // jambes de force à l'arrière (décor de studio)
  const strut = k.mat('strut', '#a88a62', { rough: 0.8 });
  for (const x of [-W / 2 + 0.3, W / 2 - 0.3]) k.rod([x, H * 0.7, -T / 2], [x, 0.02, -T / 2 - H * 0.45], 0.025, strut);
  return k.build();
}

/** Battant de porte entrouvert dans une baie de largeur dw (charnière à gauche). */
function doorLeaf(k, dw, dh, T, open, color) {
  const frame = frameMat(k);
  const leaf = k.mat('leaf', color, { rough: 0.55 });
  for (const s of [-1, 1]) k.box(0.05, dh, T + 0.03, frame, [s * (dw / 2 + 0.025), dh / 2, 0]);
  k.box(dw + 0.1, 0.05, T + 0.03, frame, [0, dh + 0.025, 0]);
  const w = dw - 0.02;
  const hinge = new THREE.Vector3(-dw / 2 + 0.01, 0, T / 2);
  const dir = new THREE.Vector3(Math.cos(open), 0, Math.sin(open));
  const c = hinge.clone().addScaledVector(dir, w / 2);
  k.box(w, dh - 0.02, 0.04, leaf, [c.x, dh / 2, c.z + 0.02], { r: [0, -open, 0] });
  const knob = hinge.clone().addScaledVector(dir, w - 0.08);
  k.ball(0.03, k.mat('knob', '#b9a46a', { rough: 0.3, metal: 0.8 }), [knob.x + 0.04 * Math.sin(open), 1.0, knob.z + 0.06]);
}

/** Châssis de fenêtre (2 vantaux, croisillon) dans une baie ww × wh posée à la hauteur y0. */
function windowFrame(k, ww, wh, T, y0, color) {
  const frame = k.mat('frame', color, { rough: 0.6 });
  const glass = glassMat(k);
  const t = 0.05;
  k.box(ww, t, T + 0.02, frame, [0, y0 + t / 2, 0]);
  k.box(ww, t, T + 0.02, frame, [0, y0 + wh - t / 2, 0]);
  for (const x of [-ww / 2 + t / 2, 0, ww / 2 - t / 2]) k.box(t, wh, T + 0.02, frame, [x, y0 + wh / 2, 0]);
  k.box(ww - 0.04, 0.03, 0.03, frame, [0, y0 + wh * 0.55, 0]);
  k.box(ww - 0.04, wh - 0.04, 0.008, glass, [0, y0 + wh / 2, 0]);
}

function door(props) {
  const k = new Kit();
  doorLeaf(k, 0.83, 2.04, 0.12, 0.6, props.color || '#f1efe9');
  return k.build();
}

function windowAlone(props) {
  const k = new Kit();
  windowFrame(k, 1.2, 1.4, 0.1, 0, props.color || '#f1efe9');
  return k.build();
}

function stairs(props) {
  const k = new Kit();
  const w = k.mat('wood', props.color || '#9a7650', { rough: 0.7 });
  const side = k.mat('side', shade(props.color || '#9a7650', 0.7), { rough: 0.75 });
  const n = 6;
  const rise = 0.18;
  const run = 0.28;
  for (let i = 0; i < n; i++) k.box(1.0, rise * (i + 1), run, w, [0, (rise * (i + 1)) / 2, (run * n) / 2 - run * (i + 0.5)]);
  // rampes : poteau sur la première et la dernière marche, main courante entre les deux
  const zFront = (run * n) / 2 - run / 2;
  const zBack = -(run * n) / 2 + run / 2;
  for (const s of [-1, 1]) {
    const x = s * 0.47;
    k.rod([x, rise, zFront], [x, rise + 0.9, zFront], 0.018, side);
    k.rod([x, rise * n, zBack], [x, rise * n + 0.9, zBack], 0.018, side);
    k.rod([x, rise + 0.9, zFront], [x, rise * n + 0.9, zBack], 0.022, side);
  }
  return k.build();
}

function riser(props) {
  const k = new Kit();
  const H = props.height ?? 0.4;
  const top = k.mat('top', props.color || '#2a2a2c', { rough: 0.8 });
  const alu = k.mat('alu', '#a9adb2', { rough: 0.35, metal: 0.8 });
  k.box(2.0, 0.03, 1.0, top, [0, H - 0.015, 0]);
  k.box(2.0, 0.05, 1.0, alu, [0, H - 0.055, 0]);
  for (const sx of [-1, 1]) for (const sz of [-1, 1]) k.box(0.05, H - 0.06, 0.05, alu, [sx * 0.95, (H - 0.06) / 2, sz * 0.45]);
  k.box(2.0, H - 0.08, 0.005, k.mat('skirt', '#111112', { rough: 1 }), [0, (H - 0.08) / 2, 0.5]); // jupe noire
  return k.build();
}

function column(props) {
  const k = new Kit();
  const H = props.height ?? 3.0;
  const m = k.mat('stone', props.color || '#e8e4da', { rough: 0.85 });
  k.box(0.5, 0.12, 0.5, m, [0, 0.06, 0]);
  k.cyl(0.19, 0.21, H - 0.3, m, [0, H / 2, 0], { seg: 24 });
  k.cyl(0.24, 0.2, 0.08, m, [0, H - 0.14, 0], { seg: 24 });
  k.box(0.5, 0.1, 0.5, m, [0, H - 0.05, 0]);
  return k.build();
}

function drape(props) {
  const k = new Kit();
  const W = props.width ?? 3.0;
  const H = props.height ?? 3.0;
  const cloth = k.mat('cloth', props.color || '#141414', { rough: 1, double: true });
  // tissu plissé : plan ondulé
  const geo = new THREE.PlaneGeometry(W, H - 0.05, Math.max(8, Math.round(W * 12)), 1);
  const pos = geo.attributes.position;
  for (let i = 0; i < pos.count; i++) pos.setZ(i, Math.sin((pos.getX(i) / W) * Math.PI * W * 4) * 0.05);
  geo.computeVertexNormals();
  k.add(geo, cloth, { p: [0, H / 2, 0] });
  k.rod([-W / 2 - 0.1, H, 0], [W / 2 + 0.1, H, 0], 0.02, k.mat('pipe', '#2a2a2c', { rough: 0.5, metal: 0.6 }));
  return k.build();
}

// ------------------------------------------------------------------ extérieur
function tree(props) {
  const k = new Kit();
  const H = props.height ?? 5;
  const bark = k.mat('bark', '#5a4330', { rough: 0.95 });
  const leaf = k.mat('leaf', props.color || '#3f6b2f', { rough: 0.85, flat: true });
  const leaf2 = k.mat('leaf2', shade(props.color || '#3f6b2f', 0.8), { rough: 0.85, flat: true });
  k.cyl(0.1 * (H / 5), 0.16 * (H / 5), H * 0.5, bark, [0, H * 0.25, 0], { seg: 10 });
  const rand = rng(13);
  const cr = H * 0.3;
  for (let i = 0; i < 14; i++) {
    const a = rand() * Math.PI * 2;
    const r = rand() * cr * 0.6;
    k.ico(cr * (0.45 + rand() * 0.25), i % 2 ? leaf : leaf2, [Math.cos(a) * r, H * 0.62 + (rand() - 0.3) * cr * 0.8, Math.sin(a) * r], { detail: 1 });
  }
  return k.build();
}

function pine(props) {
  const k = new Kit();
  const H = props.height ?? 4.5;
  const needles = k.mat('needles', props.color || '#2c4a2e', { rough: 0.9, flat: true });
  k.cyl(0.07 * (H / 4.5), 0.11 * (H / 4.5), H * 0.25, k.mat('bark', '#4a3626'), [0, H * 0.125, 0], { seg: 8 });
  for (let i = 0; i < 4; i++) {
    const t = i / 3;
    const h = H * 0.38 * (1 - t * 0.2);
    k.cone(H * 0.24 * (1 - t * 0.55), h, needles, [0, H * (0.25 + t * 0.55) + (H * 0.38 - h) / 2, 0], { seg: 10 });
  }
  return k.build();
}

function palm(props) {
  const k = new Kit();
  const bark = k.mat('bark', '#7a6248', { rough: 0.95 });
  const frond = k.mat('frond', props.color || '#4f7a34', { rough: 0.8, double: true, flat: true });
  let p = new THREE.Vector3(0, 0, 0);
  for (let i = 0; i < 6; i++) {
    const n = p.clone().add(new THREE.Vector3(0.06 * i * 0.3, 0.7, 0.04 * i));
    k.rod(p, n, 0.16 - i * 0.013, bark, { r2: 0.15 - i * 0.013 });
    p = n;
  }
  for (let i = 0; i < 9; i++) {
    const a = (i / 9) * Math.PI * 2;
    const dir = new THREE.Vector3(Math.cos(a), 0, Math.sin(a));
    const mid = p.clone().addScaledVector(dir, 0.75).add(new THREE.Vector3(0, 0.25, 0));
    const tip = p.clone().addScaledVector(dir, 1.5).add(new THREE.Vector3(0, -0.35, 0));
    for (const [a0, a1] of [[p, mid], [mid, tip]]) {
      // palme à plat : largeur horizontale, perpendiculaire à la nervure
      const d = a1.clone().sub(a0);
      const len = d.length();
      d.normalize();
      const x = new THREE.Vector3().crossVectors(d, new THREE.Vector3(0, 1, 0)).normalize();
      const z = new THREE.Vector3().crossVectors(x, d).normalize();
      const q = new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(x, d, z));
      k.add(new THREE.PlaneGeometry(0.32, len), frond, { q, p: a0.clone().add(a1).multiplyScalar(0.5) });
    }
  }
  return k.build();
}

function bush(props) {
  const k = new Kit();
  const leaf = k.mat('leaf', props.color || '#46703a', { rough: 0.85, flat: true });
  const leaf2 = k.mat('leaf2', shade(props.color || '#46703a', 0.82), { rough: 0.85, flat: true });
  const rand = rng(17);
  for (let i = 0; i < 9; i++) k.ico(0.28 + rand() * 0.14, i % 2 ? leaf : leaf2, [(rand() - 0.5) * 0.8, 0.26 + rand() * 0.22, (rand() - 0.5) * 0.6]);
  return k.build();
}

function rock(props) {
  const k = new Kit();
  const stone = k.mat('stone', props.color || '#7d7a74', { rough: 0.95, flat: true });
  const geo = new THREE.DodecahedronGeometry(0.5, 1);
  const pos = geo.attributes.position;
  const rand = rng(21);
  for (let i = 0; i < pos.count; i++) pos.setXYZ(i, pos.getX(i) * (0.9 + rand() * 0.25), pos.getY(i) * (0.85 + rand() * 0.2), pos.getZ(i) * (0.9 + rand() * 0.2));
  geo.computeVertexNormals();
  k.add(geo, stone, { p: [0, 0.42, 0], s: [1.2, 0.85, 1.0] });
  return k.build();
}

function streetlamp(props) {
  const k = new Kit();
  const m = k.mat('cast', props.color || '#2b2f33', { rough: 0.5, metal: 0.6 });
  k.cyl(0.14, 0.18, 0.5, m, [0, 0.25, 0], { seg: 12 });
  k.cyl(0.05, 0.07, 3.6, m, [0, 2.25, 0], { seg: 12 });
  k.rod([0, 3.9, 0], [0, 4.05, 0.55], 0.035, m);
  k.cone(0.22, 0.25, m, [0, 3.95, 0.62], { seg: 14 });
  k.cyl(0.12, 0.12, 0.05, k.mat('lamp', '#e9e2c8', { rough: 0.3 }), [0, 3.8, 0.62], { seg: 14 });
  return k.build();
}

function bench(props) {
  const k = new Kit();
  const slat = k.mat('slat', props.color || '#7a5232', { rough: 0.8 });
  const iron = k.mat('iron', '#2a2c30', { rough: 0.5, metal: 0.6 });
  for (let i = 0; i < 4; i++) k.box(1.8, 0.035, 0.09, slat, [0, 0.44, -0.2 + i * 0.12]);
  for (let i = 0; i < 3; i++) k.box(1.8, 0.09, 0.03, slat, [0, 0.58 + i * 0.11, -0.3], { r: [-0.15, 0, 0] });
  for (const s of [-1, 1]) {
    k.box(0.05, 0.44, 0.5, iron, [s * 0.75, 0.22, 0]);
    k.box(0.05, 0.4, 0.05, iron, [s * 0.75, 0.62, -0.31]);
  }
  return k.build();
}

function bin(props) {
  const k = new Kit();
  const m = k.mat('bin', props.color || '#3c5a3c', { rough: 0.6, metal: 0.3 });
  k.cyl(0.24, 0.22, 0.85, m, [0, 0.45, 0], { seg: 20 });
  k.cyl(0.26, 0.26, 0.06, k.mat('lid', '#2a2c30', { rough: 0.6 }), [0, 0.9, 0], { seg: 20 });
  return k.build();
}

function barrier(props) {
  const k = new Kit();
  const m = k.mat('steel', props.color || '#a9adb2', { rough: 0.35, metal: 0.8 });
  k.rod([-1, 0.12, 0], [-1, 1.1, 0], 0.02, m);
  k.rod([1, 0.12, 0], [1, 1.1, 0], 0.02, m);
  k.rod([-1, 1.1, 0], [1, 1.1, 0], 0.02, m);
  k.rod([-1, 0.25, 0], [1, 0.25, 0], 0.02, m);
  for (let i = 1; i < 14; i++) k.rod([-1 + i * (2 / 14), 0.25, 0], [-1 + i * (2 / 14), 1.1, 0], 0.008, m);
  for (const x of [-0.8, 0.8]) {
    k.rod([x, 0.12, 0], [x, 0.0, 0.3], 0.02, m);
    k.rod([x, 0.12, 0], [x, 0.0, -0.3], 0.02, m);
  }
  return k.build();
}

function cone(props) {
  const k = new Kit();
  const orange = k.mat('orange', props.color || '#f26a1b', { rough: 0.6 });
  const white = k.mat('white', '#f4f4f2', { rough: 0.5 });
  k.box(0.34, 0.03, 0.34, k.mat('base', '#1f1f22'), [0, 0.015, 0]);
  k.cone(0.14, 0.47, orange, [0, 0.265, 0], { seg: 16 });
  k.cyl(0.08, 0.1, 0.07, white, [0, 0.28, 0], { seg: 16 });
  return k.build();
}

function sign(props) {
  const k = new Kit();
  const pole = k.mat('pole', '#a9adb2', { rough: 0.4, metal: 0.7 });
  k.cyl(0.03, 0.03, 2.3, pole, [0, 1.15, 0], { seg: 10 });
  k.cyl(0.32, 0.32, 0.02, k.mat('border', props.color || '#c8202a', { rough: 0.5 }), [0, 2.25, 0.03], { r: [Math.PI / 2, 0, 0], seg: 28 });
  k.cyl(0.25, 0.25, 0.022, k.mat('face', '#f4f4f2', { rough: 0.5 }), [0, 2.25, 0.035], { r: [Math.PI / 2, 0, 0], seg: 28 });
  return k.build();
}

function trafficlight(props) {
  const k = new Kit();
  const m = k.mat('mast', props.color || '#3a3e44', { rough: 0.5, metal: 0.5 });
  k.cyl(0.06, 0.07, 2.4, m, [0, 1.2, 0], { seg: 12 });
  k.rbox(0.3, 0.85, 0.22, 0.04, k.mat('box', '#1c1d20', { rough: 0.5 }), [0, 2.75, 0]);
  [['#7a1010', 3.0], ['#7a5a10', 2.75], ['#106a2a', 2.5]].forEach(([c, y], i) => k.cyl(0.085, 0.085, 0.03, k.mat(`l${i}`, c, { rough: 0.3 }), [0, y, 0.115], { r: [Math.PI / 2, 0, 0], seg: 16 }));
  return k.build();
}

/** Modèle 3D d'un élément de décor ou d'extérieur. */
export function buildSet(type, props = {}) {
  switch (type.model) {
    case 'wall':
    case 'walldoor':
    case 'wallwindow':
      return wall(type, props);
    case 'door':
      return door(props);
    case 'window':
      return windowAlone(props);
    case 'stairs':
      return stairs(props);
    case 'riser':
      return riser(props);
    case 'column':
      return column(props);
    case 'drape':
      return drape(props);
    case 'tree':
      return tree(props);
    case 'pine':
      return pine(props);
    case 'palm':
      return palm(props);
    case 'bush':
      return bush(props);
    case 'rock':
      return rock(props);
    case 'streetlamp':
      return streetlamp(props);
    case 'bench':
      return bench(props);
    case 'bin':
      return bin(props);
    case 'barrier':
      return barrier(props);
    case 'cone':
      return cone(props);
    case 'sign':
      return sign(props);
    case 'trafficlight':
      return trafficlight(props);
    default:
      return null;
  }
}
