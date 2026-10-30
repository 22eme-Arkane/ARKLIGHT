// Véhicules : voitures (profil latéral extrudé + habitacle vitré), utilitaires, camion, remorques,
// deux-roues. Avant vers +Z, roues au sol. Dimensions des modèles courants.
import * as THREE from 'three';
import { Kit } from './kit.js';

const V = (x, y, z) => new THREE.Vector3(x, y, z);
const TURN = new THREE.Quaternion().setFromEuler(new THREE.Euler(0, -Math.PI / 2, 0)); // profil (x = longueur) → avant vers +Z

/** Matières communes : peinture, vitres, pneus, jantes, phares, feux. */
function materials(k, paint) {
  return {
    paint: k.mat('paint', paint, { rough: 0.35, metal: 0.35 }),
    trim: k.mat('trim', '#1b1c1f', { rough: 0.6 }),
    glass: k.mat('glass', '#1d2a33', { rough: 0.08, metal: 0.4 }),
    tire: k.mat('tire', '#141416', { rough: 0.9 }),
    rim: k.mat('rim', '#b9bdc2', { rough: 0.3, metal: 0.8 }),
    head: k.mat('headlight', '#f4f1e6', { rough: 0.2, emissive: '#2a2a22' }),
    tail: k.mat('taillight', '#a3121a', { rough: 0.3, emissive: '#3a0306' }),
    plate: k.mat('plate', '#eef0f2', { rough: 0.5 }),
  };
}

function wheel(k, m, x, z, R, w) {
  const q = new THREE.Quaternion().setFromEuler(new THREE.Euler(0, 0, Math.PI / 2));
  k.cyl(R, R, w, m.tire, [x, R, z], { q, seg: 20 });
  k.cyl(R * 0.62, R * 0.62, w + 0.012, m.rim, [x, R, z], { q, seg: 16 });
}

/**
 * Silhouette latérale d'une voiture (x de l'arrière 0 à l'avant L) avec passages de roues.
 * body : points du haut de caisse de l'arrière vers l'avant ; axles : positions des essieux.
 */
function bodyShape(L, clr, R, axles, top) {
  const s = new THREE.Shape();
  const ar = R * 1.12;
  s.moveTo(L * 0.985, clr + 0.05);
  // dessous et passages de roues, de l'avant vers l'arrière
  for (const ax of [...axles].sort((a, b) => b - a)) {
    s.lineTo(ax + ar, clr);
    s.lineTo(ax + ar, R);
    s.absarc(ax, R, ar, 0, Math.PI, false);
    s.lineTo(ax - ar, clr);
  }
  s.lineTo(L * 0.015, clr + 0.05);
  // ligne du haut : de l'arrière vers l'avant
  for (const [x, y] of top) s.lineTo(x, y);
  s.lineTo(L * 0.985, clr + 0.05);
  return s;
}

/** Voiture particulière : caisse, habitacle vitré, toit, roues, phares et feux. */
function car(kind, paint) {
  const P = {
    city: { L: 3.95, W: 1.75, H: 1.48, clr: 0.15, R: 0.3, axles: [0.62, 3.15], belt: 0.86, hood: [3.2, 0.82], ws: [2.55, 1.42], roof: [0.55, 1.45], rear: [0.12, 0.88] },
    sedan: { L: 4.72, W: 1.84, H: 1.45, clr: 0.14, R: 0.32, axles: [0.95, 3.75], belt: 0.86, hood: [3.75, 0.8], ws: [3.05, 1.4], roof: [1.35, 1.42], rear: [0.55, 0.9] },
    suv: { L: 4.6, W: 1.9, H: 1.74, clr: 0.22, R: 0.37, axles: [0.85, 3.6], belt: 1.08, hood: [3.65, 1.02], ws: [3.0, 1.68], roof: [0.45, 1.7], rear: [0.12, 1.08] },
    pickup: { L: 5.3, W: 1.92, H: 1.84, clr: 0.24, R: 0.39, axles: [1.0, 4.25], belt: 1.1, hood: [4.3, 1.06], ws: [3.65, 1.78], roof: [2.55, 1.8], rear: [2.45, 1.1] },
  }[kind];
  const k = new Kit();
  const m = materials(k, paint);
  const { L, W, H, clr, R, axles } = P;
  // caisse jusqu'à la ligne de ceinture
  const top =
    kind === 'pickup'
      ? [
          [0.02, P.belt * 0.85],
          [0.05, P.belt],
          [P.rear[0], P.belt],
          [P.hood[0], P.hood[1]],
          [L * 0.985, P.hood[1] - 0.12],
          [L, 0.6],
        ]
      : [
          [0.0, 0.6 * H],
          [0.05, P.belt],
          [P.hood[0], P.hood[1]],
          [L * 0.975, P.hood[1] - 0.06],
          [L, P.hood[1] - 0.22],
        ];
  k.extrude(bodyShape(L, clr, R, axles, top), W, m.paint, { bevel: 0.05, q: TURN, p: [0, 0, -L / 2], curve: 14 });
  // habitacle vitré (plus étroit), toit et montants
  const cab = [
    [P.rear[0], P.rear[1]],
    [P.hood[0] - 0.05, P.belt],
    [P.ws[0], P.ws[1]],
    [P.roof[0], P.roof[1]],
  ];
  const cabW = W * 0.86;
  k.extrude([cab[0], cab[1], cab[2], cab[3]], cabW, m.glass, { q: TURN, p: [0, 0, -L / 2] });
  k.extrude([[P.roof[0] - 0.02, P.roof[1] - 0.03], [P.ws[0] + 0.02, P.ws[1] - 0.03], [P.ws[0] - 0.01, P.ws[1] + 0.025], [P.roof[0], P.roof[1] + 0.025]], cabW + 0.02, m.paint, { q: TURN, p: [0, 0, -L / 2] });
  const bx = ((P.roof[0] + P.ws[0]) / 2) - L / 2;
  k.box(cabW + 0.025, P.roof[1] - P.belt, 0.09, m.paint, [0, (P.roof[1] + P.belt) / 2, bx]);
  if (kind === 'pickup') {
    // benne ouverte
    const bedL = P.rear[0] - 0.08;
    const zc = -L / 2 + bedL / 2 + 0.05;
    for (const s of [-1, 1]) k.box(0.06, 0.42, bedL, m.paint, [s * (W / 2 - 0.03), P.belt - 0.21, zc]);
    k.box(W, 0.42, 0.06, m.paint, [0, P.belt - 0.21, -L / 2 + 0.06]);
    k.box(W - 0.1, 0.03, bedL, m.trim, [0, P.belt - 0.4, zc]);
  }
  // roues, phares, feux, plaques, rétroviseurs
  for (const ax of axles) for (const s of [-1, 1]) wheel(k, m, s * (W / 2 - 0.12), ax - L / 2, R, 0.22);
  for (const s of [-1, 1]) {
    k.rbox(0.28, 0.1, 0.06, 0.02, m.head, [s * (W / 2 - 0.22), P.hood[1] - 0.16, L / 2 - 0.04]);
    k.rbox(0.26, 0.09, 0.05, 0.02, m.tail, [s * (W / 2 - 0.2), (kind === 'pickup' ? P.belt : 0.62 * H) - 0.12, -L / 2 + 0.03]);
    k.box(0.04, 0.09, 0.16, m.trim, [s * (cabW / 2 + 0.1), P.belt + 0.08, P.hood[0] - L / 2 - 0.12]);
  }
  k.box(0.52, 0.11, 0.02, m.plate, [0, 0.42, L / 2 + 0.005]);
  k.box(0.52, 0.11, 0.02, m.plate, [0, 0.55, -L / 2 - 0.005]);
  k.box(W * 0.98, 0.12, 0.08, m.trim, [0, clr + 0.12, L / 2 - 0.02]);
  k.box(W * 0.98, 0.12, 0.08, m.trim, [0, clr + 0.12, -L / 2 + 0.02]);
  return { ...k.build(), aim: V(0, H * 0.55, 0) };
}

/** Utilitaire, fourgon : nez court et grand caisson. */
function van(kind, paint) {
  const P = kind === 'bigvan' ? { L: 6.0, W: 2.02, H: 2.68, R: 0.36, axles: [1.2, 4.75], nose: 0.95 } : { L: 5.0, W: 1.96, H: 1.98, R: 0.34, axles: [0.9, 4.0], nose: 0.85 };
  const k = new Kit();
  const m = materials(k, paint);
  const { L, W, H, R, axles, nose } = P;
  const clr = 0.2;
  const top = [
    [0, H - 0.06],
    [0.05, H],
    [L - nose - 0.25, H],
    [L - nose * 0.45, H * 0.6],
    [L - 0.05, H * 0.47],
    [L, H * 0.3],
  ];
  k.extrude(bodyShape(L, clr, R, axles, top), W, m.paint, { bevel: 0.06, q: TURN, p: [0, 0, -L / 2], curve: 14 });
  // pare-brise (juste devant la tôle inclinée) et vitres de cabine
  const z0 = -L / 2;
  const a = [L - nose * 0.45, H * 0.6];
  const b = [L - nose - 0.25, H];
  const out = 0.025; // décalage vers l'avant, perpendiculaire au pare-brise
  const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
  const n = [(b[1] - a[1]) / len, -(b[0] - a[0]) / len];
  const on = (t) => [a[0] + (b[0] - a[0]) * t + n[0] * out, a[1] + (b[1] - a[1]) * t + n[1] * out];
  k.extrude([on(0.04), on(0.9), [L - nose - 0.75, on(0.9)[1]], [L - nose - 0.75, H * 0.62], [on(0.04)[0] - 0.05, H * 0.62]], W + 0.012, m.glass, { q: TURN, p: [0, 0, z0] });
  for (const ax of axles) for (const s of [-1, 1]) wheel(k, m, s * (W / 2 - 0.13), ax - L / 2, R, 0.24);
  for (const s of [-1, 1]) {
    k.rbox(0.3, 0.13, 0.05, 0.02, m.head, [s * (W / 2 - 0.25), H * 0.4, L / 2 - 0.02]);
    k.rbox(0.12, 0.3, 0.05, 0.02, m.tail, [s * (W / 2 - 0.1), H * 0.55, -L / 2 - 0.01]);
    k.box(0.05, 0.22, 0.18, m.trim, [s * (W / 2 + 0.08), H * 0.68, L / 2 - nose - 0.35]);
  }
  k.box(0.08, H * 0.82, 0.02, m.trim, [0, H * 0.5, -L / 2 - 0.012]); // portes arrière
  k.box(W * 0.98, 0.14, 0.1, m.trim, [0, clr + 0.12, L / 2 - 0.02]);
  k.box(0.52, 0.11, 0.02, m.plate, [0, 0.45, L / 2 + 0.01]);
  return { ...k.build(), aim: V(0, H * 0.55, 0) };
}

/** Camion porteur à caisse (type camion électro) : cabine, caisse, hayon, 6 roues. */
function truck(paint) {
  const k = new Kit();
  const m = materials(k, paint);
  const box = k.mat('box', '#f2f2f0', { rough: 0.7 });
  const L = 8.4;
  const W = 2.5;
  const R = 0.5;
  const cabL = 2.0;
  const zc = L / 2 - cabL / 2;
  // cabine
  k.rbox(W, 2.2, cabL, 0.12, m.paint, [0, 0.9 + 1.1, zc]);
  k.box(W - 0.2, 0.9, 0.05, m.glass, [0, 2.35, L / 2 + 0.005]);
  for (const s of [-1, 1]) k.box(0.05, 0.75, 1.0, m.glass, [s * (W / 2 + 0.004), 2.4, zc + 0.3]);
  k.box(W - 0.3, 0.45, 0.06, m.trim, [0, 1.15, L / 2 + 0.01]); // calandre
  for (const s of [-1, 1]) {
    k.rbox(0.32, 0.18, 0.06, 0.03, m.head, [s * (W / 2 - 0.3), 1.0, L / 2 + 0.01]);
    k.box(0.06, 0.4, 0.06, m.trim, [s * (W / 2 + 0.15), 2.3, L / 2 - 0.25]);
  }
  // caisse
  const boxL = L - cabL - 0.15;
  const bz = -L / 2 + boxL / 2;
  k.rbox(W + 0.05, 2.5, boxL, 0.05, box, [0, 1.1 + 1.25, bz]);
  k.box(W + 0.07, 0.08, boxL, m.trim, [0, 1.12, bz]);
  k.box(W - 0.2, 0.06, 0.04, m.trim, [0, 1.2, -L / 2 - 0.02]); // hayon
  for (const s of [-1, 1]) k.rbox(0.12, 0.3, 0.05, 0.02, m.tail, [s * (W / 2 - 0.12), 1.0, -L / 2 - 0.01]);
  // châssis et roues (jumelées à l'arrière)
  k.box(1.0, 0.3, L - 0.6, m.trim, [0, 0.75, 0]);
  wheel(k, m, W / 2 - 0.2, L / 2 - 1.2, R, 0.32);
  wheel(k, m, -(W / 2 - 0.2), L / 2 - 1.2, R, 0.32);
  for (const s of [-1, 1]) for (const dx of [0.2, 0.55]) wheel(k, m, s * (W / 2 - dx), -L / 2 + 2.0, R, 0.3);
  return { ...k.build(), aim: V(0, 1.8, 0) };
}

/** Remorque : groupe électrogène capoté ou loge (caravane). */
function trailer(kind, paint) {
  const k = new Kit();
  const m = materials(k, paint);
  const isGen = kind === 'generator';
  const L = isGen ? 3.2 : 5.6;
  const W = isGen ? 1.7 : 2.3;
  const H = isGen ? 1.55 : 2.25;
  const R = isGen ? 0.33 : 0.36;
  const zc = isGen ? -0.3 : -0.4;
  const y0 = R + 0.25;
  k.rbox(W, H, L, isGen ? 0.06 : 0.18, m.paint, [0, y0 + H / 2, zc]);
  if (isGen) {
    for (let i = 0; i < 6; i++) for (const s of [-1, 1]) k.box(0.02, 0.05, 1.2, m.trim, [s * (W / 2 + 0.005), y0 + 0.4 + i * 0.12, zc - 0.6]);
    k.box(0.5, 0.35, 0.03, m.trim, [W * 0.2, y0 + H * 0.6, zc + L / 2 + 0.01]); // tableau électrique
    k.cyl(0.07, 0.07, 0.35, m.trim, [-W * 0.3, y0 + H + 0.17, zc - L * 0.3]); // échappement
  } else {
    for (const s of [-1, 1]) for (const z of [-1.6, 0.4, 1.9]) k.box(0.02, 0.6, 0.9, m.glass, [s * (W / 2 + 0.006), y0 + 1.35, zc + z]);
    k.box(0.02, 1.85, 0.75, m.trim, [W / 2 + 0.008, y0 + 1.0, zc - 0.5]); // porte
    k.box(W * 0.6, 0.12, 0.6, k.mat('roof', '#d8d8d4', { rough: 0.6 }), [0, y0 + H + 0.06, zc]); // lanterneau
  }
  // timon, roues, béquille
  k.rod([0, y0 + 0.1, zc + L / 2], [0, y0 - 0.05, zc + L / 2 + 1.2], 0.05, m.trim);
  k.cyl(0.03, 0.03, y0, m.trim, [0, y0 / 2, zc + L / 2 + 1.1]);
  for (const s of [-1, 1]) wheel(k, m, s * (W / 2 - 0.12), zc, R, 0.22);
  return { ...k.build(), aim: V(0, y0 + H / 2, zc) };
}

/** Deux-roues : moto, scooter, vélo. */
function twoWheeler(kind, paint) {
  const k = new Kit();
  const m = materials(k, paint);
  const metal = k.mat('metal', '#9da2a8', { rough: 0.3, metal: 0.8 });
  const isBike = kind === 'bicycle';
  const R = isBike ? 0.34 : kind === 'scooter' ? 0.24 : 0.32;
  const wb = isBike ? 1.05 : kind === 'scooter' ? 1.3 : 1.45;
  const zf = wb / 2;
  const zr = -wb / 2;
  const q = new THREE.Quaternion().setFromEuler(new THREE.Euler(0, Math.PI / 2, 0));
  for (const z of [zf, zr]) {
    k.torus(R - (isBike ? 0.015 : 0.05), isBike ? 0.018 : 0.055, m.tire, [0, R, z], { q, tube: 10, seg: 28 });
    if (isBike) for (let i = 0; i < 8; i++) k.rod([0, R, z], [0, R + Math.sin((i / 8) * Math.PI * 2) * (R - 0.03), z + Math.cos((i / 8) * Math.PI * 2) * (R - 0.03)], 0.003, metal);
    else k.cyl(R * 0.55, R * 0.55, 0.06, m.rim, [0, R, z], { r: [0, 0, Math.PI / 2], seg: 16 });
  }
  if (isBike) {
    const frame = k.mat('frame', paint, { rough: 0.4, metal: 0.5 });
    const bb = V(0, R, 0.02);
    const seat = V(0, R + 0.52, -0.18);
    const head = V(0, R + 0.5, zf - 0.22);
    k.rod(bb, seat, 0.017, frame);
    k.rod(bb, head, 0.02, frame);
    k.rod(seat.clone().add(V(0, -0.05, 0.02)), head.clone().add(V(0, 0.02, 0)), 0.016, frame);
    for (const s of [-1, 1]) {
      k.rod(bb, V(s * 0.05, R, zr), 0.011, frame);
      k.rod(seat.clone().add(V(0, -0.04, 0)), V(s * 0.05, R, zr), 0.01, frame);
      k.rod(head, V(s * 0.04, R, zf), 0.013, frame);
    }
    k.rod(seat, seat.clone().add(V(0, 0.12, -0.03)), 0.012, metal);
    k.rbox(0.13, 0.05, 0.26, 0.02, m.trim, seat.clone().add(V(0, 0.15, -0.01)));
    k.rod(head, head.clone().add(V(0, 0.18, -0.05)), 0.014, metal);
    k.rod(head.clone().add(V(-0.27, 0.18, -0.06)), head.clone().add(V(0.27, 0.18, -0.06)), 0.012, metal);
    k.cyl(0.09, 0.09, 0.02, metal, [0.05, R, 0.02], { r: [0, 0, Math.PI / 2] }); // plateau
    return { ...k.build(), aim: V(0, 0.8, 0) };
  }
  if (kind === 'scooter') {
    k.rbox(0.36, 0.45, 0.85, 0.14, m.paint, [0, R + 0.32, -0.2]); // coque arrière
    k.rbox(0.34, 0.12, 0.6, 0.05, m.trim, [0, R + 0.6, -0.25]); // selle
    k.rbox(0.32, 0.08, 0.45, 0.03, m.trim, [0, R + 0.12, 0.25]); // plancher
    k.rbox(0.42, 0.9, 0.22, 0.1, m.paint, [0, R + 0.5, zf - 0.18], { r: [-0.25, 0, 0] }); // tablier
    k.rod([0, R, zf], [0, R + 0.95, zf - 0.25], 0.025, metal);
    k.rod([-0.33, R + 0.95, zf - 0.27], [0.33, R + 0.95, zf - 0.27], 0.016, m.trim);
    k.rbox(0.18, 0.1, 0.06, 0.02, m.head, [0, R + 0.82, zf - 0.08]);
    return { ...k.build(), aim: V(0, 0.8, 0) };
  }
  // moto
  k.rbox(0.3, 0.22, 0.55, 0.1, m.paint, [0, R + 0.62, 0.1]); // réservoir
  k.rbox(0.26, 0.09, 0.65, 0.04, m.trim, [0, R + 0.55, -0.42]); // selle
  k.rbox(0.32, 0.35, 0.42, 0.08, metal, [0, R + 0.2, 0.05]); // moteur
  k.rod([0, R + 0.5, 0.35], [0, R, zf], 0.03, metal); // fourche
  k.rod([0, R + 0.35, -0.1], [0, R, zr], 0.035, m.trim); // bras oscillant
  k.rod([-0.38, R + 0.78, 0.42], [0.38, R + 0.78, 0.42], 0.016, m.trim); // guidon
  k.rod([0, R + 0.5, 0.35], [0, R + 0.78, 0.42], 0.025, metal);
  k.rbox(0.2, 0.18, 0.12, 0.05, m.head, [0, R + 0.62, 0.55]);
  k.rod([0.14, R + 0.12, 0.0], [0.17, R + 0.25, -0.8], 0.045, metal); // échappement
  k.rbox(0.24, 0.1, 0.28, 0.04, m.paint, [0, R + 0.42, zr + 0.05]); // garde-boue arrière
  return { ...k.build(), aim: V(0, 0.9, 0) };
}

/** Modèle 3D d'un véhicule du catalogue. */
export function buildVehicle(kind, props = {}) {
  const paint = props.color || '#8a8f96';
  if (['city', 'sedan', 'suv', 'pickup'].includes(kind)) return car(kind, paint);
  if (kind === 'van' || kind === 'bigvan') return van(kind, paint);
  if (kind === 'truck') return truck(paint);
  if (kind === 'generator' || kind === 'caravan') return trailer(kind, paint);
  return twoWheeler(kind, paint);
}

