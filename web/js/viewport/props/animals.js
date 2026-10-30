// Animaux : quadrupède paramétré par espèce (corps, pattes, cou, tête, oreilles, queue et
// particularités : crinière, cornes, bois, laine, taches…) et oiseaux. Face vers +Z, pattes au sol.
import * as THREE from 'three';
import { Kit, rng, shade } from './kit.js';

// Dimensions en mètres. neck : [longueur, angle au-dessus de l'horizontale, rayon] ;
// head : demi-axes [x, y, z] et inclinaison ; tail : [longueur, rayon, angle, style].
const SPECIES = {
  dog: { L: 0.62, r: 0.15, w: 0.82, leg: 0.34, legR: 0.033, neck: [0.17, 0.95, 0.07], head: [0.085, 0.085, 0.1, 0.15], snout: [0.05, 0.045, 0.085], ears: ['floppy', 0.085], tail: [0.32, 0.022, 0.45], nose: '#1f1a17' },
  smalldog: { L: 0.34, r: 0.085, w: 0.85, leg: 0.17, legR: 0.019, neck: [0.09, 0.9, 0.042], head: [0.058, 0.055, 0.065, 0.1], snout: [0.032, 0.028, 0.05], ears: ['pointy', 0.05], tail: [0.15, 0.012, 1.0], nose: '#1f1a17' },
  cat: { L: 0.36, r: 0.08, w: 0.8, leg: 0.16, legR: 0.016, neck: [0.07, 0.75, 0.034], head: [0.055, 0.05, 0.055, 0.05], snout: [0.026, 0.02, 0.025], ears: ['pointy', 0.045], tail: [0.3, 0.012, 1.0, 'up'], nose: '#b9787a' },
  horse: { L: 1.45, r: 0.3, w: 0.78, leg: 0.95, legR: 0.045, neck: [0.62, 0.95, 0.15], head: [0.11, 0.12, 0.3, 0.75], snout: [0.09, 0.09, 0.13], ears: ['small', 0.1], tail: [0.75, 0.055, -0.35, 'hair'], mane: true, hooves: '#2a2420', nose: '#2a2420' },
  donkey: { L: 1.05, r: 0.25, w: 0.8, leg: 0.68, legR: 0.038, neck: [0.42, 0.8, 0.12], head: [0.1, 0.11, 0.24, 0.6], snout: [0.085, 0.085, 0.1], ears: ['long', 0.24], tail: [0.55, 0.02, -0.4], mane: true, hooves: '#2a2420', nose: '#d8d0c4' },
  cow: { L: 1.6, r: 0.36, w: 0.85, leg: 0.72, legR: 0.06, neck: [0.32, 0.35, 0.17], head: [0.14, 0.15, 0.25, 0.45], snout: [0.12, 0.1, 0.1], ears: ['side', 0.13], horns: 'cow', tail: [0.8, 0.022, -0.2], udder: true, spots: '#1e1c1a', hooves: '#2a2420', nose: '#c99a94' },
  sheep: { L: 0.85, r: 0.27, w: 0.9, leg: 0.4, legR: 0.026, neck: [0.14, 0.6, 0.08], head: [0.075, 0.085, 0.13, 0.35], snout: [0.06, 0.06, 0.06], ears: ['side', 0.08], tail: [0.12, 0.035, -0.6], wool: true, darkHead: '#2a2522', hooves: '#2a2420', nose: '#2a2522' },
  goat: { L: 0.82, r: 0.2, w: 0.75, leg: 0.5, legR: 0.026, neck: [0.25, 1.0, 0.07], head: [0.065, 0.075, 0.15, 0.55], snout: [0.05, 0.05, 0.06], ears: ['side', 0.09], horns: 'goat', beard: true, tail: [0.1, 0.018, 0.9], hooves: '#2a2420', nose: '#5a4a3c' },
  pig: { L: 1.0, r: 0.26, w: 0.92, leg: 0.27, legR: 0.045, neck: [0.06, 0.3, 0.2], head: [0.15, 0.15, 0.17, 0.12], snout: [0.075, 0.065, 0.07, 'disk'], ears: ['floppy', 0.1], tail: [0.12, 0.012, 0.3, 'curl'], hooves: '#6b4a3c', nose: '#d98f84' },
  deer: { L: 1.12, r: 0.24, w: 0.75, leg: 0.88, legR: 0.03, neck: [0.45, 1.05, 0.09], head: [0.085, 0.095, 0.19, 0.55], snout: [0.06, 0.06, 0.08], ears: ['side', 0.12], antlers: true, tail: [0.12, 0.03, 0.9], hooves: '#2a2420', nose: '#1f1a17' },
};

const BIRDS = {
  chicken: { body: [0.11, 0.12, 0.16], y: 0.25, head: 0.05, neck: 0.1, beak: '#e3a531', legs: '#e3b43a', comb: true, tail: 'fan' },
  duck: { body: [0.11, 0.09, 0.2], y: 0.17, head: 0.052, neck: 0.12, beak: '#f0a330', legs: '#f0a330', flatBeak: true, tail: 'short' },
  pigeon: { body: [0.055, 0.058, 0.1], y: 0.115, head: 0.032, neck: 0.045, beak: '#2a2a2a', legs: '#b5524f', tail: 'long' },
};

const V = (x, y, z) => new THREE.Vector3(x, y, z);

function quadruped(sp, coat) {
  const k = new Kit();
  const fur = k.mat('fur', coat, { rough: 0.95, flat: !!sp.wool });
  const dark = k.mat('dark', shade(coat, 0.55), { rough: 0.9 });
  const hoof = k.mat('hoof', sp.hooves || shade(coat, 0.45), { rough: 0.6 });
  const nose = k.mat('nose', sp.nose || '#2a2420', { rough: 0.5 });
  const eye = k.mat('eye', '#0b0b0c', { rough: 0.2 });
  const headMat = sp.darkHead ? k.mat('head', sp.darkHead, { rough: 0.9 }) : fur;
  const { L, r, w, leg, legR } = sp;
  const bodyY = leg + r;
  // corps
  if (sp.wool) {
    const rand = rng(7);
    for (let i = 0; i < 26; i++) {
      const a = rand() * Math.PI * 2;
      const z = (rand() - 0.5) * (L - r * 0.6);
      k.ico(r * (0.42 + rand() * 0.18), fur, [Math.cos(a) * r * w * 0.62, bodyY + Math.sin(a) * r * 0.62, z]);
    }
    k.capsule([0, bodyY, -L / 2 + r], [0, bodyY, L / 2 - r], r * 0.86, fur, { s: [w, 1, 1] });
  } else k.capsule([0, bodyY, -L / 2 + r], [0, bodyY, L / 2 - r], r, fur, { s: [w, 1, 1], seg: 16 });
  // pattes : cuisse épaisse, canon fin, sabot ou patte
  const legX = r * w * 0.55;
  const legZ = L / 2 - r * 0.75;
  for (const sx of [-1, 1]) {
    for (const sz of [-1, 1]) {
      const top = V(sx * legX, bodyY - r * 0.2, sz * legZ);
      const mid = V(sx * legX, leg * 0.5, sz * legZ + (sz < 0 ? -legR * 0.6 : legR * 0.3));
      const bot = V(sx * legX, legR * 0.6, sz * legZ);
      k.capsule(top, mid, legR * 1.6, fur);
      k.capsule(mid, bot, legR, sp.wool ? headMat : fur);
      k.cyl(legR * 1.15, legR * 1.3, legR * 1.2, hoof, [bot.x, legR * 0.6, bot.z + legR * 0.2]);
    }
  }
  // cou et tête
  const [nLen, nAng, nR] = sp.neck;
  const nDir = V(0, Math.sin(nAng), Math.cos(nAng));
  const nBase = V(0, bodyY + r * 0.35, L / 2 - r * 0.45);
  const nEnd = nBase.clone().addScaledVector(nDir, nLen);
  k.capsule(nBase, nEnd, nR, sp.wool ? fur : headMat);
  const [hx, hy, hz, pitch] = sp.head;
  const hq = new THREE.Quaternion().setFromEuler(new THREE.Euler(pitch, 0, 0));
  const head = nEnd.clone().add(V(0, hy * 0.3, hz * 0.45).applyQuaternion(hq));
  const at = (x, y, z) => head.clone().add(V(x, y, z).applyQuaternion(hq));
  k.ball(1, headMat, head, { q: hq, s: [hx, hy, hz], seg: 18 });
  const [sx, sy, sz, snoutStyle] = sp.snout;
  const snout = at(0, -hy * 0.3, hz * 0.85);
  k.ball(1, headMat, snout, { q: hq, s: [sx, sy, sz], seg: 14 });
  if (snoutStyle === 'disk') k.cyl(sx * 0.9, sx * 0.9, sz * 0.5, nose, at(0, -hy * 0.3, hz * 0.85 + sz * 0.95), { q: new THREE.Quaternion().setFromEuler(new THREE.Euler(Math.PI / 2 + pitch, 0, 0)), seg: 14 });
  else k.ball(sx * 0.45, nose, at(0, -hy * 0.3 + sy * 0.35, hz * 0.85 + sz * 0.85));
  for (const s of [-1, 1]) k.ball(Math.max(0.006, hx * 0.14), eye, at(s * hx * 0.72, hy * 0.28, hz * 0.42));
  // oreilles
  const [earStyle, ear] = sp.ears;
  for (const s of [-1, 1]) {
    if (earStyle === 'pointy' || earStyle === 'small' || earStyle === 'long') {
      const base = at(s * hx * 0.5, hy * 0.82, -hz * (earStyle === 'small' ? 0.35 : 0.15));
      k.cone(ear * (earStyle === 'long' ? 0.22 : 0.42), ear, headMat, base.clone().add(V(0, ear * 0.45, 0)), { r: [-0.15, 0, -s * 0.25], seg: 8 });
    } else if (earStyle === 'floppy') {
      k.ball(1, dark, at(s * hx * 1.0, hy * 0.15, -hz * 0.05), { q: hq, s: [ear * 0.22, ear * 0.9, ear * 0.55], seg: 10 });
    } else if (earStyle === 'side') {
      k.ball(1, headMat, at(s * (hx + ear * 0.4), hy * 0.45, -hz * 0.25), { s: [ear * 0.6, ear * 0.2, ear * 0.32], seg: 10 });
    }
  }
  // queue
  const [tLen, tR, tAng, tStyle] = sp.tail;
  const tBase = V(0, bodyY + r * 0.55, -L / 2 + r * 0.25);
  if (tStyle === 'up') {
    const mid = tBase.clone().add(V(0, tLen * 0.25, -tLen * 0.45));
    k.capsule(tBase, mid, tR, fur);
    k.capsule(mid, mid.clone().add(V(0, tLen * 0.55, -tLen * 0.05)), tR, fur);
  } else if (tStyle === 'hair') {
    const end = tBase.clone().add(V(0, -tLen * 0.85, -tLen * 0.35));
    k.capsule(tBase, end, tR, dark, { s: [0.7, 1, 1.2] });
  } else if (tStyle === 'curl') {
    k.torus(tLen * 0.3, tR, fur, tBase.clone().add(V(0, 0, -tLen * 0.2)), { r: [0, Math.PI / 2, 0], tube: 6, seg: 14 });
  } else {
    k.rod(tBase, tBase.clone().add(V(0, Math.sin(tAng) * tLen, -Math.cos(tAng) * tLen)), tR, fur, { r2: tR * 0.6 });
  }
  // particularités
  if (sp.mane) k.capsule(nBase.clone().add(V(0, nR * 0.9, -0.02)), at(0, hy * 0.9, -hz * 0.6), nR * 0.35, dark, { s: [0.5, 1, 1] });
  if (sp.horns === 'cow') for (const s of [-1, 1]) k.cone(0.03, 0.16, k.mat('horn', '#e8dcc0', { rough: 0.5 }), at(s * hx * 1.05, hy * 0.85, -hz * 0.25), { r: [0, 0, -s * 1.1], seg: 8 });
  if (sp.horns === 'goat') for (const s of [-1, 1]) k.cone(0.022, 0.2, k.mat('horn', '#6b5a48', { rough: 0.6 }), at(s * hx * 0.45, hy * 1.05, -hz * 0.45), { r: [-0.9, 0, -s * 0.15], seg: 8 });
  if (sp.beard) k.cone(0.025, 0.09, dark, at(0, -hy * 1.05, hz * 0.45), { r: [Math.PI, 0, 0], seg: 8 });
  if (sp.antlers) {
    const bone = k.mat('antler', '#d8c8a8', { rough: 0.7 });
    for (const s of [-1, 1]) {
      const base = at(s * hx * 0.45, hy * 0.85, -hz * 0.3);
      const top = base.clone().add(V(s * 0.22, 0.42, -0.22));
      k.rod(base, top, 0.022, bone, { r2: 0.012 });
      for (const t of [0.35, 0.65, 0.9]) {
        const p = base.clone().lerp(top, t);
        k.rod(p, p.clone().add(V(s * 0.05, 0.14, 0.1)), 0.012, bone, { r2: 0.006 });
      }
    }
  }
  if (sp.udder) k.ball(0.13, k.mat('udder', '#e2a8a2', { rough: 0.6 }), [0, leg - 0.02, -L / 2 + r * 1.3], { s: [1, 0.7, 1] });
  if (sp.spots) {
    const spot = k.mat('spot', sp.spots, { rough: 0.95 });
    const rand = rng(3);
    for (let i = 0; i < 9; i++) {
      const a = rand() * Math.PI * 2;
      const z = (rand() - 0.5) * (L - r * 0.8);
      k.ball(r * (0.35 + rand() * 0.25), spot, [Math.cos(a) * r * w * 0.72, bodyY + Math.sin(a) * r * 0.72, z], { s: [1, 0.8, 1.3] });
    }
  }
  return { ...k.build(), aim: head };
}

function bird(b, coat) {
  const k = new Kit();
  const fur = k.mat('plume', coat, { rough: 0.9 });
  const dark = k.mat('dark', shade(coat, 0.6), { rough: 0.9 });
  const beak = k.mat('beak', b.beak, { rough: 0.5 });
  const legs = k.mat('legs', b.legs, { rough: 0.6 });
  const eye = k.mat('eye', '#0b0b0c', { rough: 0.2 });
  const [bx, by, bz] = b.body;
  const c = V(0, b.y, 0);
  k.ball(1, fur, c, { s: [bx, by, bz], seg: 18 });
  for (const s of [-1, 1]) k.ball(1, dark, [s * bx * 0.85, b.y + by * 0.1, -bz * 0.1], { s: [bx * 0.25, by * 0.6, bz * 0.85], seg: 12 });
  // cou et tête
  const neckBase = V(0, b.y + by * 0.5, bz * 0.6);
  const head = neckBase.clone().add(V(0, b.neck, b.head * 0.6));
  k.capsule(neckBase, head, b.head * 0.55, fur);
  k.ball(b.head, fur, head);
  for (const s of [-1, 1]) k.ball(b.head * 0.16, eye, head.clone().add(V(s * b.head * 0.75, b.head * 0.2, b.head * 0.4)));
  if (b.flatBeak) k.rbox(b.head * 0.8, b.head * 0.25, b.head * 1.3, 0.01, beak, head.clone().add(V(0, -b.head * 0.25, b.head * 1.2)));
  else k.cone(b.head * 0.32, b.head * 0.9, beak, head.clone().add(V(0, -b.head * 0.1, b.head * 1.25)), { r: [Math.PI / 2, 0, 0], seg: 8 });
  if (b.comb) {
    const red = k.mat('comb', '#c8202a', { rough: 0.6 });
    for (const z of [-0.4, 0, 0.4]) k.ball(b.head * 0.32, red, head.clone().add(V(0, b.head * 1.0, z * b.head)));
    k.ball(b.head * 0.25, red, head.clone().add(V(0, -b.head * 0.85, b.head * 0.75)), { s: [0.6, 1.3, 0.8] });
  }
  // queue
  const tail = V(0, b.y + by * 0.35, -bz * 0.95);
  if (b.tail === 'fan') for (const a of [-0.35, 0, 0.35]) k.ball(1, dark, tail.clone().add(V(Math.sin(a) * 0.05, 0.08, -0.03)), { r: [-0.7, 0, a], s: [0.025, 0.12, 0.06], seg: 10 });
  else k.cone(bx * 0.6, bz * (b.tail === 'long' ? 0.8 : 0.4), dark, tail.clone().add(V(0, 0, -bz * 0.2)), { r: [-Math.PI / 2 - 0.25, 0, 0], s: [1, 1, 0.35], seg: 8 });
  // pattes
  const legTop = b.y - by * 0.7;
  for (const s of [-1, 1]) {
    const x = s * bx * 0.4;
    k.rod([x, legTop, 0], [x, 0.01, bz * 0.05], Math.max(0.004, bx * 0.06), legs);
    if (b.flatBeak) k.box(bx * 0.45, 0.006, bz * 0.35, legs, [x, 0.004, bz * 0.2]);
    else for (const a of [-0.5, 0, 0.5]) k.rod([x, 0.006, bz * 0.05], [x + Math.sin(a) * bz * 0.3, 0.004, bz * 0.05 + Math.cos(a) * bz * 0.3], Math.max(0.003, bx * 0.04), legs);
  }
  return { ...k.build(), aim: head };
}

/** Modèle 3D d'un animal (espèce du catalogue, couleur du pelage). */
export function buildAnimal(species, props = {}) {
  if (BIRDS[species]) return bird(BIRDS[species], props.color || '#9a9a9a');
  return quadruped(SPECIES[species] || SPECIES.dog, props.color || '#8a6a4a');
}
