// Accessoires (valise, carton, bouteille, verre, livres, ordinateur, guitare, micro) et matériel
// de plateau (caméra sur trépied, travelling, chaise de réalisateur, moniteur, perche son).
// Face avant vers +Z, posés au sol.
import * as THREE from 'three';
import { Kit, rng, shade } from './kit.js';

const V = (x, y, z) => new THREE.Vector3(x, y, z);
const metal = (k) => k.mat('metal', '#a9adb2', { rough: 0.35, metal: 0.8 });
const black = (k) => k.mat('black', '#18191c', { rough: 0.55 });

function suitcase(c) {
  const k = new Kit();
  const shell = k.mat('shell', c, { rough: 0.35, metal: 0.1 });
  k.rbox(0.45, 0.62, 0.25, 0.04, shell, [0, 0.36, 0]);
  for (const x of [-0.12, 0, 0.12]) k.box(0.01, 0.6, 0.255, k.mat('rib', shade(c, 0.8), { rough: 0.4 }), [x, 0.36, 0]);
  const m = black(k);
  k.rod([-0.08, 0.67, 0], [-0.08, 0.95, 0], 0.008, metal(k));
  k.rod([0.08, 0.67, 0], [0.08, 0.95, 0], 0.008, metal(k));
  k.box(0.2, 0.025, 0.03, m, [0, 0.96, 0]);
  for (const sx of [-1, 1]) for (const sz of [-1, 1]) k.cyl(0.025, 0.025, 0.02, m, [sx * 0.18, 0.025, sz * 0.09], { r: [0, 0, Math.PI / 2], seg: 10 });
  return k.build();
}

function cardboard(c) {
  const k = new Kit();
  k.box(0.5, 0.4, 0.4, k.mat('card', c, { rough: 0.95 }), [0, 0.2, 0]);
  k.box(0.06, 0.405, 0.405, k.mat('tape', '#d9c08a', { rough: 0.5 }), [0, 0.2, 0]);
  return k.build();
}

function crate(c) {
  const k = new Kit();
  const w = k.mat('wood', c, { rough: 0.85 });
  const w2 = k.mat('wood2', shade(c, 0.78), { rough: 0.85 });
  k.box(0.78, 0.58, 0.58, w2, [0, 0.3, 0]);
  for (let i = 0; i < 4; i++) for (const s of [-1, 1]) k.box(0.8, 0.12, 0.02, w, [0, 0.08 + i * 0.145, s * 0.3]);
  for (const s of [-1, 1]) for (const sz of [-1, 1]) k.box(0.05, 0.6, 0.05, w, [s * 0.375, 0.3, sz * 0.28]);
  return k.build();
}

function flightcase(c) {
  const k = new Kit();
  const panel = k.mat('panel', c, { rough: 0.6 });
  const alu = metal(k);
  const y0 = 0.08; // roulettes
  k.box(0.78, 0.56, 0.58, panel, [0, y0 + 0.29, 0]);
  for (const [w, h, d, p] of [
    [0.8, 0.03, 0.03, [0, y0 + 0.575, 0.29]],
    [0.8, 0.03, 0.03, [0, y0 + 0.575, -0.29]],
    [0.8, 0.03, 0.03, [0, y0 + 0.015, 0.29]],
    [0.8, 0.03, 0.03, [0, y0 + 0.015, -0.29]],
    [0.03, 0.59, 0.03, [0.39, y0 + 0.295, 0.29]],
    [0.03, 0.59, 0.03, [-0.39, y0 + 0.295, 0.29]],
    [0.03, 0.59, 0.03, [0.39, y0 + 0.295, -0.29]],
    [0.03, 0.59, 0.03, [-0.39, y0 + 0.295, -0.29]],
    [0.8, 0.02, 0.6, [0, y0 + 0.4, 0]],
  ])
    k.box(w, h, d, alu, p);
  for (const sx of [-1, 1]) for (const sz of [-1, 1]) k.cyl(0.04, 0.04, 0.05, black(k), [sx * 0.32, 0.04, sz * 0.22], { r: [0, 0, Math.PI / 2], seg: 12 });
  return k.build();
}

function bottle(c) {
  const k = new Kit();
  k.lathe([[0.001, 0], [0.038, 0], [0.04, 0.02], [0.04, 0.2], [0.03, 0.24], [0.014, 0.27], [0.014, 0.3], [0.001, 0.31]], k.mat('glass', c, { rough: 0.1, metal: 0.1, opacity: 0.8 }), [0, 0, 0], { seg: 20 });
  k.cyl(0.0405, 0.0405, 0.08, k.mat('label', '#efe6d0', { rough: 0.6 }), [0, 0.12, 0], { seg: 20 });
  return k.build();
}

function wineglass(c) {
  const k = new Kit();
  const glass = k.mat('glass', '#dfe8ee', { rough: 0.05, opacity: 0.35, double: true });
  k.lathe([[0.001, 0], [0.035, 0.002], [0.004, 0.006], [0.004, 0.1], [0.025, 0.12], [0.04, 0.16], [0.038, 0.2], [0.036, 0.2]], glass, [0, 0, 0], { seg: 20 });
  k.lathe([[0.001, 0.115], [0.026, 0.125], [0.033, 0.145], [0.001, 0.145]], k.mat('wine', c, { rough: 0.2 }), [0, 0, 0], { seg: 20 });
  return k.build();
}

function mug(c) {
  const k = new Kit();
  const m = k.mat('mug', c, { rough: 0.35 });
  k.cyl(0.042, 0.04, 0.095, m, [0, 0.0475, 0], { seg: 20 });
  k.torus(0.03, 0.008, m, [0.05, 0.05, 0], { r: [0, 0, 0], tube: 8, seg: 14 });
  k.cyl(0.036, 0.036, 0.004, k.mat('coffee', '#3a2416', { rough: 0.3 }), [0, 0.09, 0], { seg: 20 });
  return k.build();
}

function books(c) {
  const k = new Kit();
  const rand = rng(23);
  const covers = [c, '#2e4a7a', '#3e6a3a', '#c9a24a', '#e8e2d6'];
  let y = 0;
  for (let i = 0; i < 5; i++) {
    const h = 0.025 + rand() * 0.02;
    k.box(0.22 + rand() * 0.06, h, 0.16 + rand() * 0.04, k.mat(`b${i}`, covers[i], { rough: 0.8 }), [(rand() - 0.5) * 0.03, y + h / 2, (rand() - 0.5) * 0.03], { r: [0, (rand() - 0.5) * 0.3, 0] });
    y += h;
  }
  return k.build();
}

function laptop(c) {
  const k = new Kit();
  const shell = k.mat('shell', c, { rough: 0.35, metal: 0.6 });
  k.rbox(0.33, 0.018, 0.23, 0.006, shell, [0, 0.009, 0.02]);
  k.box(0.28, 0.002, 0.1, black(k), [0, 0.019, 0.04]);
  const open = 1.85; // ~105°
  k.rbox(0.33, 0.22, 0.008, 0.005, shell, [0, 0.018 + Math.sin(open) * 0.11, -0.095 + Math.cos(open) * 0.11], { r: [-(Math.PI - open), 0, 0] });
  k.box(0.3, 0.19, 0.002, k.mat('screen', '#20324a', { rough: 0.1, emissive: '#0d1622' }), [0, 0.018 + Math.sin(open) * 0.11, -0.09 + Math.cos(open) * 0.11], { r: [-(Math.PI - open), 0, 0] });
  return k.build();
}

function guitar(c) {
  const k = new Kit();
  const body = k.mat('body', c, { rough: 0.4 });
  const neck = k.mat('neck', '#3a2416', { rough: 0.6 });
  const tilt = new THREE.Quaternion().setFromEuler(new THREE.Euler(-0.25, 0, 0));
  const at = (x, y, z) => V(x, y, z).applyQuaternion(tilt).add(V(0, 0.08, 0.05));
  k.ball(1, body, at(0, 0.28, 0), { q: tilt, s: [0.19, 0.2, 0.05], seg: 24 });
  k.ball(1, body, at(0, 0.5, 0), { q: tilt, s: [0.145, 0.15, 0.05], seg: 24 });
  k.cyl(0.045, 0.045, 0.01, black(k), at(0, 0.4, 0.05), { q: new THREE.Quaternion().setFromEuler(new THREE.Euler(Math.PI / 2 - 0.25, 0, 0)), seg: 18 });
  k.box(0.05, 0.5, 0.025, neck, at(0, 0.88, 0.0), { q: tilt });
  k.box(0.08, 0.17, 0.02, neck, at(0, 1.2, -0.01), { q: tilt });
  // support
  const m = black(k);
  k.rod([0, 0.0, -0.22], [0, 0.85, -0.1], 0.01, m);
  for (const s of [-1, 1]) k.rod([0, 0.15, -0.18], [s * 0.18, 0, 0.12], 0.01, m);
  return k.build();
}

function micstand(c) {
  const k = new Kit();
  const m = k.mat('stand', c, { rough: 0.5, metal: 0.4 });
  for (let i = 0; i < 3; i++) {
    const a = (i / 3) * Math.PI * 2;
    k.rod([0, 0.35, 0], [Math.cos(a) * 0.3, 0, Math.sin(a) * 0.3], 0.01, m);
  }
  k.cyl(0.012, 0.012, 1.1, m, [0, 0.9, 0]);
  k.rod([0, 1.42, 0], [0, 1.5, 0.35], 0.009, m);
  k.capsule([0, 1.5, 0.37], [0, 1.53, 0.5], 0.02, black(k));
  k.ball(0.028, k.mat('grill', '#8a8d92', { rough: 0.4, metal: 0.7 }), [0, 1.535, 0.53]);
  return k.build();
}

// ------------------------------------------------------------------ plateau
/** Trépied à trois branches (hauteur de la tête h) avec araignée au sol. */
function tripod(k, h, m) {
  for (let i = 0; i < 3; i++) {
    const a = (i / 3) * Math.PI * 2 + Math.PI / 6;
    k.rod([0, h, 0], [Math.cos(a) * 0.42, 0.02, Math.sin(a) * 0.42], 0.016, m);
    k.rod([Math.cos(a) * 0.12, 0.35, Math.sin(a) * 0.12], [0, 0.3, 0], 0.008, m);
  }
}

/** Caméra cinéma : corps, objectif, pare-soleil, viseur, poignée. Visée vers +Z. */
function cameraBody(k, y, c) {
  const body = k.mat('camera', c, { rough: 0.5, metal: 0.2 });
  const glass = k.mat('lens', '#0a0c10', { rough: 0.05, metal: 0.5 });
  const m = black(k);
  k.rbox(0.16, 0.18, 0.3, 0.02, body, [0, y, -0.02]);
  k.cyl(0.055, 0.06, 0.2, m, [0, y, 0.22], { r: [Math.PI / 2, 0, 0], seg: 20 });
  k.cyl(0.05, 0.05, 0.01, glass, [0, y, 0.325], { r: [Math.PI / 2, 0, 0], seg: 20 });
  k.box(0.24, 0.18, 0.12, m, [0, y, 0.4]); // pare-soleil
  k.box(0.2, 0.14, 0.02, glass, [0, y, 0.45]);
  k.capsule([0.1, y + 0.07, -0.08], [0.1, y + 0.07, -0.22], 0.025, m); // viseur
  k.rod([-0.05, y + 0.1, 0.04], [-0.05, y + 0.14, -0.1], 0.012, m); // poignée
  k.box(0.07, 0.05, 0.1, k.mat('monitor', '#0f1114', { rough: 0.2 }), [-0.12, y + 0.06, 0.02]);
}

function camera(c, lens = 1.45) {
  const k = new Kit();
  const m = black(k);
  const head = lens - 0.21;
  tripod(k, head, m);
  k.cyl(0.07, 0.08, 0.12, m, [0, head + 0.06, 0], { seg: 16 }); // tête fluide
  k.rod([0, head + 0.08, -0.05], [0, head - 0.07, -0.45], 0.012, m); // manche
  cameraBody(k, lens, c);
  return { ...k.build(), aim: V(0, lens, 0.3), lens: V(0, lens, 0.47) };
}

function dolly(c) {
  const k = new Kit();
  const rail = metal(k);
  const m = black(k);
  const body = k.mat('dolly', c, { rough: 0.5, metal: 0.3 });
  for (const s of [-1, 1]) k.rod([s * 0.31, 0.04, -1.5], [s * 0.31, 0.04, 1.5], 0.024, rail);
  for (let z = -1.4; z <= 1.41; z += 0.4) k.box(0.8, 0.025, 0.06, k.mat('sleeper', '#5a4a3a', { rough: 0.9 }), [0, 0.012, z]);
  k.rbox(0.8, 0.12, 0.9, 0.03, body, [0, 0.2, 0]);
  for (const sx of [-1, 1]) for (const sz of [-1, 1]) k.cyl(0.05, 0.05, 0.05, m, [sx * 0.31, 0.11, sz * 0.32], { r: [0, 0, Math.PI / 2], seg: 12 });
  k.cyl(0.05, 0.06, 0.95, rail, [0, 0.75, 0.15], { seg: 14 }); // colonne
  k.box(0.38, 0.06, 0.32, m, [0, 0.04 + 0.27, -0.3]); // siège opérateur
  k.cyl(0.02, 0.02, 0.35, m, [0, 0.45, -0.3]);
  cameraBody(k, 1.36, '#25272b');
  return { ...k.build(), aim: V(0, 1.36, 0.3), lens: V(0, 1.36, 0.62) };
}

function directorchair(c) {
  const k = new Kit();
  const w = k.mat('wood', '#c9a46a', { rough: 0.6 });
  const canvas = k.mat('canvas', c, { rough: 0.95 });
  for (const s of [-1, 1]) {
    k.rod([s * 0.27, 0, -0.2], [s * 0.27, 0.72, 0.2], 0.016, w);
    k.rod([s * 0.27, 0, 0.2], [s * 0.27, 0.72, -0.2], 0.016, w);
    k.box(0.04, 0.03, 0.5, w, [s * 0.28, 0.72, 0]); // accoudoirs
    k.rod([s * 0.27, 0.72, -0.22], [s * 0.27, 1.12, -0.24], 0.016, w);
  }
  k.box(0.5, 0.01, 0.38, canvas, [0, 0.62, 0]);
  k.box(0.52, 0.2, 0.01, canvas, [0, 1.0, -0.24]);
  return k.build();
}

function monitor(c) {
  const k = new Kit();
  const m = k.mat('stand', c, { rough: 0.5, metal: 0.4 });
  tripod(k, 1.25, m);
  k.cyl(0.015, 0.015, 0.25, m, [0, 1.37, 0]);
  k.rbox(0.45, 0.3, 0.06, 0.015, black(k), [0, 1.62, 0]);
  k.box(0.4, 0.25, 0.004, k.mat('screen', '#20324a', { rough: 0.1, emissive: '#0d1622' }), [0, 1.62, 0.031]);
  k.box(0.46, 0.04, 0.2, black(k), [0, 1.8, 0.07]); // casquette
  return k.build();
}

function boom(c) {
  const k = new Kit();
  const m = black(k);
  tripod(k, 1.4, m);
  k.cyl(0.018, 0.018, 0.6, m, [0, 1.7, 0]);
  const base = V(0, 2.0, 0);
  const tip = V(0, 2.45, 2.2);
  k.rod(base.clone().add(V(0, -0.15, -0.8)), tip, 0.014, metal(k));
  k.capsule(tip.clone().add(V(0, -0.12, 0.05)), tip.clone().add(V(0, -0.12, 0.42)), 0.055, k.mat('zeppelin', c, { rough: 1 })); // bonnette
  k.rod(tip, tip.clone().add(V(0, -0.07, 0.2)), 0.006, m);
  return k.build();
}

/** Modèle 3D d'un accessoire ou d'un élément de plateau. */
export function buildAccessory(type, props = {}) {
  const c = props.color || '#8a8f96';
  switch (type.model) {
    case 'suitcase':
      return suitcase(c);
    case 'cardboard':
      return cardboard(c);
    case 'crate':
      return crate(c);
    case 'flightcase':
      return flightcase(c);
    case 'bottle':
      return bottle(c);
    case 'wineglass':
      return wineglass(c);
    case 'mug':
      return mug(c);
    case 'books':
      return books(c);
    case 'laptop':
      return laptop(c);
    case 'guitar':
      return guitar(c);
    case 'micstand':
      return micstand(c);
    case 'camera':
      return camera(c, props.lens ?? 1.45);
    case 'dolly':
      return dolly(c);
    case 'directorchair':
      return directorchair(c);
    case 'monitor':
      return monitor(c);
    case 'boom':
      return boom(c);
    default:
      return null;
  }
}
