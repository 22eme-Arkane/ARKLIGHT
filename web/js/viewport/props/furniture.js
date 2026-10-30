// Mobilier : assises, tables, lits, rangements, cuisine, électroménager, piano, plantes…
// Face avant vers +Z, posé au sol. Dimensions des meubles courants.
import { Kit, rng, shade } from './kit.js';

const wood = (k, c) => k.mat('wood', c, { rough: 0.7 });
const fabric = (k, c) => k.mat('fabric', c, { rough: 0.95 });
const metal = (k) => k.mat('metal', '#a9adb2', { rough: 0.35, metal: 0.8 });
const dark = (k) => k.mat('dark', '#1d1e21', { rough: 0.6 });

/** Quatre pieds droits sous un plateau (w × d), de hauteur h. */
function legs4(k, w, d, h, r, mat, inset = 0.05, square = false) {
  for (const sx of [-1, 1]) {
    for (const sz of [-1, 1]) {
      const p = [sx * (w / 2 - inset), h / 2, sz * (d / 2 - inset)];
      if (square) k.box(r * 2, h, r * 2, mat, p);
      else k.cyl(r, r * 0.85, h, mat, p, { seg: 10 });
    }
  }
}

function chair(c) {
  const k = new Kit();
  const w = wood(k, c);
  legs4(k, 0.42, 0.42, 0.45, 0.018, w, 0.03, true);
  k.box(0.45, 0.035, 0.44, w, [0, 0.46, 0]);
  for (const s of [-1, 1]) k.box(0.035, 0.45, 0.035, w, [s * 0.195, 0.69, -0.195]);
  k.box(0.42, 0.22, 0.025, w, [0, 0.77, -0.205], { r: [-0.08, 0, 0] });
  return k.build();
}

function stool(c) {
  const k = new Kit();
  const w = wood(k, c);
  k.cyl(0.18, 0.18, 0.035, w, [0, 0.445, 0], { seg: 20 });
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2 + Math.PI / 4;
    k.rod([Math.cos(a) * 0.1, 0.43, Math.sin(a) * 0.1], [Math.cos(a) * 0.17, 0, Math.sin(a) * 0.17], 0.016, w);
  }
  return k.build();
}

function barstool(c) {
  const k = new Kit();
  const m = metal(k);
  k.cyl(0.21, 0.19, 0.08, fabric(k, c), [0, 0.76, 0], { seg: 22 });
  k.cyl(0.025, 0.025, 0.72, m, [0, 0.36, 0]);
  k.torus(0.16, 0.012, m, [0, 0.3, 0], { r: [Math.PI / 2, 0, 0] });
  k.cyl(0.2, 0.22, 0.02, m, [0, 0.01, 0], { seg: 22 });
  return k.build();
}

function armchair(c) {
  const k = new Kit();
  const f = fabric(k, c);
  const fd = k.mat('fabric2', shade(c, 0.82), { rough: 0.95 });
  k.rbox(0.85, 0.3, 0.8, 0.06, fd, [0, 0.25, 0]);
  k.rbox(0.62, 0.15, 0.64, 0.06, f, [0, 0.47, 0.05]);
  k.rbox(0.85, 0.5, 0.2, 0.08, f, [0, 0.62, -0.32], { r: [-0.12, 0, 0] });
  for (const s of [-1, 1]) k.rbox(0.13, 0.32, 0.78, 0.06, f, [s * 0.36, 0.55, 0]);
  legs4(k, 0.75, 0.7, 0.1, 0.02, dark(k), 0.04);
  return k.build();
}

function sofa(c, n, W) {
  const k = new Kit();
  const f = fabric(k, c);
  const fd = k.mat('fabric2', shade(c, 0.82), { rough: 0.95 });
  const D = 0.9;
  k.rbox(W, 0.28, D, 0.05, fd, [0, 0.24, 0]);
  const seatW = (W - 0.36) / n;
  for (let i = 0; i < n; i++) {
    const x = -W / 2 + 0.18 + seatW * (i + 0.5);
    k.rbox(seatW - 0.02, 0.14, 0.62, 0.06, f, [x, 0.45, 0.08]);
    k.rbox(seatW - 0.04, 0.42, 0.18, 0.07, f, [x, 0.68, -0.27], { r: [-0.15, 0, 0] });
  }
  k.rbox(W, 0.45, 0.18, 0.06, fd, [0, 0.6, -0.36]);
  for (const s of [-1, 1]) k.rbox(0.18, 0.36, D, 0.07, f, [s * (W / 2 - 0.09), 0.56, 0]);
  legs4(k, W - 0.1, D - 0.1, 0.1, 0.02, dark(k), 0.04);
  return k.build();
}

function table(c, W, D, H = 0.75) {
  const k = new Kit();
  const w = wood(k, c);
  k.box(W, 0.04, D, w, [0, H - 0.02, 0]);
  k.box(W - 0.12, 0.08, D - 0.12, k.mat('apron', shade(c, 0.85), { rough: 0.7 }), [0, H - 0.08, 0]);
  legs4(k, W, D, H - 0.04, 0.025, w, 0.07, true);
  return k.build();
}

function roundtable(c) {
  const k = new Kit();
  const w = wood(k, c);
  k.cyl(0.5, 0.5, 0.04, w, [0, 0.73, 0], { seg: 32 });
  k.cyl(0.05, 0.06, 0.69, w, [0, 0.36, 0], { seg: 14 });
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2;
    k.box(0.06, 0.04, 0.32, w, [Math.sin(a) * 0.16, 0.02, Math.cos(a) * 0.16], { r: [0, a, 0] });
  }
  return k.build();
}

function coffeetable(c) {
  const k = new Kit();
  const w = wood(k, c);
  k.rbox(1.1, 0.05, 0.6, 0.01, w, [0, 0.4, 0]);
  k.box(1.0, 0.025, 0.5, w, [0, 0.12, 0]);
  legs4(k, 1.06, 0.56, 0.38, 0.022, w, 0.04, true);
  return k.build();
}

function desk(c) {
  const k = new Kit();
  const w = wood(k, c);
  const m = metal(k);
  k.box(1.4, 0.03, 0.7, w, [0, 0.735, 0]);
  k.box(0.42, 0.68, 0.62, w, [0.47, 0.37, 0]); // caisson
  for (let i = 0; i < 3; i++) k.box(0.12, 0.015, 0.01, m, [0.47, 0.6 - i * 0.2, 0.315]);
  for (const sz of [-1, 1]) k.box(0.04, 0.72, 0.04, m, [-0.66, 0.36, sz * 0.31]);
  k.box(0.9, 0.35, 0.015, w, [-0.2, 0.52, -0.3]); // voile de fond
  return k.build();
}

function bed(c, bw) {
  const k = new Kit();
  const frame = wood(k, '#6e4a2c');
  const sheet = k.mat('sheet', '#f1efe9', { rough: 0.9 });
  const duvet = fabric(k, c);
  const L = 2.0;
  const W = bw + 0.08;
  k.box(W, 0.25, L + 0.05, frame, [0, 0.2, 0]);
  k.rbox(bw, 0.22, L - 0.02, 0.06, sheet, [0, 0.42, 0]);
  k.rbox(bw + 0.02, 0.12, L * 0.62, 0.05, duvet, [0, 0.55, 0.36]);
  const n = bw > 1.2 ? 2 : 1;
  for (let i = 0; i < n; i++) k.rbox(bw / n - 0.12, 0.12, 0.4, 0.05, sheet, [-bw / 2 + (bw / n) * (i + 0.5), 0.6, -L / 2 + 0.3]);
  k.box(W, 0.95, 0.06, frame, [0, 0.47, -L / 2 - 0.02]); // tête de lit
  legs4(k, W - 0.04, L, 0.08, 0.03, frame, 0.05, true);
  return k.build();
}

function nightstand(c) {
  const k = new Kit();
  const w = wood(k, c);
  k.box(0.45, 0.45, 0.4, w, [0, 0.32, 0]);
  k.box(0.41, 0.015, 0.01, dark(k), [0, 0.42, 0.2]);
  k.cyl(0.012, 0.012, 0.02, metal(k), [0, 0.36, 0.21], { r: [Math.PI / 2, 0, 0] });
  legs4(k, 0.43, 0.38, 0.1, 0.015, w, 0.02, true);
  return k.build();
}

function cabinet(c, W, H, D, drawers) {
  const k = new Kit();
  const w = wood(k, c);
  const line = k.mat('line', shade(c, 0.6), { rough: 0.7 });
  const m = metal(k);
  k.box(W, H - 0.06, D, w, [0, (H + 0.06) / 2, 0]);
  k.box(W + 0.02, 0.025, D + 0.02, w, [0, H, 0]);
  k.box(W - 0.04, 0.05, D - 0.04, line, [0, 0.03, 0]); // socle
  if (drawers) {
    for (let i = 1; i < drawers; i++) k.box(W - 0.04, 0.008, 0.005, line, [0, 0.06 + ((H - 0.06) / drawers) * i, D / 2 + 0.002]);
    for (let i = 0; i < drawers; i++) k.box(0.14, 0.018, 0.02, m, [0, 0.06 + ((H - 0.06) / drawers) * (i + 0.5), D / 2 + 0.012]);
  } else {
    k.box(0.006, H - 0.12, 0.005, line, [0, H / 2 + 0.03, D / 2 + 0.002]);
    for (const s of [-1, 1]) k.box(0.018, 0.22, 0.02, m, [s * 0.05, H * 0.55, D / 2 + 0.012]);
  }
  return k.build();
}

function bookcase(c) {
  const k = new Kit();
  const w = wood(k, c);
  const W = 0.9;
  const H = 2.0;
  const D = 0.32;
  for (const s of [-1, 1]) k.box(0.025, H, D, w, [s * (W / 2 - 0.0125), H / 2, 0]);
  k.box(W, H - 0.02, 0.012, w, [0, H / 2, -D / 2 + 0.006]);
  const shelves = 5;
  const rand = rng(11);
  const colors = ['#7a2e2e', '#2e4a7a', '#3e6a3a', '#c9a24a', '#5a3a6a', '#d8d2c6', '#1f2a3a', '#8a5a2a'].map((col, i) => k.mat(`book${i}`, col, { rough: 0.8 }));
  for (let i = 0; i <= shelves; i++) {
    const y = 0.04 + (i * (H - 0.08)) / shelves;
    k.box(W - 0.05, 0.022, D - 0.02, w, [0, y, 0.005]);
    if (i === shelves) break;
    // livres sur l'étagère
    let x = -W / 2 + 0.05;
    while (x < W / 2 - 0.08) {
      const bw = 0.02 + rand() * 0.035;
      const bh = 0.2 + rand() * 0.12;
      if (rand() < 0.08) {
        x += 0.06;
        continue;
      }
      k.box(bw, bh, 0.18 + rand() * 0.06, colors[Math.floor(rand() * colors.length)], [x + bw / 2, y + 0.011 + bh / 2, 0.02]);
      x += bw + 0.003;
    }
  }
  return k.build();
}

function counter(c) {
  const k = new Kit();
  const face = wood(k, c);
  const top = k.mat('top', '#2b2b2e', { rough: 0.3, metal: 0.2 });
  const m = metal(k);
  k.box(2.4, 1.05, 0.55, face, [0, 0.525, 0]);
  k.box(2.5, 0.05, 0.7, top, [0, 1.08, 0.04]);
  k.rod([-1.2, 0.2, 0.36], [1.2, 0.2, 0.36], 0.02, m);
  return k.build();
}

function kitchen(c) {
  const k = new Kit();
  const face = k.mat('face', c, { rough: 0.5 });
  const top = k.mat('top', '#4a4642', { rough: 0.4 });
  const steel = metal(k);
  const line = k.mat('line', shade(c, 0.7), { rough: 0.6 });
  const W = 2.4;
  k.box(W, 0.86, 0.6, face, [0, 0.47, 0]);
  k.box(W, 0.1, 0.55, k.mat('plinth', '#2a2a2c', { rough: 0.8 }), [0, 0.05, -0.02]);
  k.box(W + 0.02, 0.04, 0.64, top, [0, 0.92, 0.01]);
  for (let i = 1; i < 4; i++) k.box(0.006, 0.84, 0.004, line, [-W / 2 + (W / 4) * i, 0.47, 0.301]);
  k.box(0.5, 0.02, 0.42, steel, [-0.5, 0.945, 0.02]); // évier
  k.rod([-0.5, 0.94, -0.2], [-0.5, 1.15, -0.12], 0.012, steel);
  k.box(0.58, 0.012, 0.5, k.mat('hob', '#111214', { rough: 0.2 }), [0.55, 0.945, 0.02]); // plaque
  k.box(W, 0.7, 0.35, face, [0, 1.85, -0.125]); // meubles hauts
  for (let i = 1; i < 4; i++) k.box(0.006, 0.68, 0.004, line, [-W / 2 + (W / 4) * i, 1.85, 0.051]);
  return k.build();
}

function fridge(c) {
  const k = new Kit();
  const body = k.mat('body', c, { rough: 0.3, metal: 0.3 });
  const line = dark(k);
  k.rbox(0.6, 1.78, 0.64, 0.03, body, [0, 0.9, 0]);
  k.box(0.58, 0.008, 0.005, line, [0, 1.25, 0.322]);
  for (const y of [0.85, 1.55]) k.box(0.025, 0.3, 0.03, metal(k), [0.25, y, 0.335]);
  return k.build();
}

function tv(c) {
  const k = new Kit();
  const w = wood(k, c);
  const screen = k.mat('screen', '#0a0b0d', { rough: 0.15, metal: 0.3 });
  k.box(1.6, 0.45, 0.42, w, [0, 0.225, 0]);
  k.box(1.25, 0.72, 0.04, screen, [0, 0.45 + 0.08 + 0.36, 0]);
  k.box(0.3, 0.08, 0.2, dark(k), [0, 0.49, 0]);
  return k.build();
}

function piano(c) {
  const k = new Kit();
  const lac = k.mat('laque', c, { rough: 0.15, metal: 0.1 });
  const ivory = k.mat('ivory', '#f2efe6', { rough: 0.4 });
  const ebony = dark(k);
  k.box(1.5, 1.25, 0.38, lac, [0, 0.625, -0.12]);
  k.box(1.5, 0.12, 0.3, lac, [0, 0.68, 0.18]); // clavier
  k.box(1.36, 0.02, 0.15, ivory, [0, 0.75, 0.26]);
  for (let i = 0; i < 36; i++) if ([1, 3, 6, 8, 10].includes(i % 12)) k.box(0.02, 0.025, 0.09, ebony, [-0.66 + i * 0.038, 0.77, 0.23]);
  for (const s of [-1, 1]) k.box(0.06, 0.62, 0.06, lac, [s * 0.68, 0.31, 0.26]);
  return k.build();
}

function floorlamp(c) {
  const k = new Kit();
  const m = dark(k);
  k.cyl(0.16, 0.18, 0.03, m, [0, 0.015, 0], { seg: 24 });
  k.cyl(0.012, 0.012, 1.4, m, [0, 0.72, 0]);
  k.cyl(0.14, 0.22, 0.28, k.mat('shade', c, { rough: 0.9, double: true }), [0, 1.5, 0], { open: true, seg: 24 });
  return k.build();
}

function plant(c) {
  const k = new Kit();
  const pot = k.mat('pot', c, { rough: 0.8 });
  const leaf = k.mat('leaf', '#3f6b34', { rough: 0.85, flat: true });
  const leaf2 = k.mat('leaf2', '#2f5527', { rough: 0.85, flat: true });
  k.lathe([[0.001, 0], [0.16, 0], [0.22, 0.42], [0.2, 0.42], [0.001, 0.4]], pot, [0, 0, 0], { seg: 20 });
  k.cyl(0.02, 0.025, 0.5, k.mat('stem', '#5a4a2a'), [0, 0.65, 0]);
  const rand = rng(5);
  for (let i = 0; i < 14; i++) {
    const a = rand() * Math.PI * 2;
    const r = 0.08 + rand() * 0.2;
    k.ico(0.12 + rand() * 0.1, i % 2 ? leaf : leaf2, [Math.cos(a) * r, 0.75 + rand() * 0.55, Math.sin(a) * r], { s: [1, 0.8, 1] });
  }
  return k.build();
}

function rug(c) {
  const k = new Kit();
  k.box(2.0, 0.012, 1.4, k.mat('rug', c, { rough: 1 }), [0, 0.006, 0]);
  k.box(1.7, 0.014, 1.1, k.mat('rug2', shade(c, 1.35), { rough: 1 }), [0, 0.007, 0]);
  k.box(1.5, 0.016, 0.9, k.mat('rug3', c, { rough: 1 }), [0, 0.008, 0]);
  return k.build();
}

function coatrack(c) {
  const k = new Kit();
  const w = wood(k, c);
  k.cyl(0.025, 0.03, 1.75, w, [0, 0.875, 0]);
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2;
    k.rod([0, 0.12, 0], [Math.cos(a) * 0.25, 0, Math.sin(a) * 0.25], 0.018, w);
    k.rod([0, 1.62, 0], [Math.cos(a + 0.6) * 0.14, 1.7, Math.sin(a + 0.6) * 0.14], 0.012, w);
  }
  k.capsule([0.09, 1.55, 0.02], [0.12, 1.05, 0.06], 0.07, fabric(k, '#3a3f4a'), { s: [1.4, 1, 0.6] }); // manteau
  return k.build();
}

function divider(c) {
  const k = new Kit();
  const frame = wood(k, '#4a3424');
  const cloth = k.mat('cloth', c, { rough: 0.95, double: true });
  const pw = 0.5;
  const angles = [0.35, 0, -0.35];
  for (let i = 0; i < 3; i++) {
    const cx = -0.5 + i * 0.5;
    const z = i === 1 ? -0.08 : 0.05;
    k.box(pw - 0.04, 1.6, 0.01, cloth, [cx, 0.92, z], { r: [0, angles[i], 0] });
    for (const s of [-1, 1]) k.box(0.03, 1.75, 0.03, frame, [cx + s * (pw / 2 - 0.015) * Math.cos(angles[i]), 0.875, z - s * (pw / 2 - 0.015) * Math.sin(angles[i])]);
  }
  return k.build();
}

/** Modèle 3D d'un meuble du catalogue. */
export function buildFurniture(type, props = {}) {
  const c = props.color || '#8a6a4a';
  switch (type.model) {
    case 'chair':
      return chair(c);
    case 'stool':
      return stool(c);
    case 'barstool':
      return barstool(c);
    case 'armchair':
      return armchair(c);
    case 'sofa':
      return sofa(c, type.seats || 2, type.size[0]);
    case 'table':
      return table(c, 1.6, 0.9);
    case 'roundtable':
      return roundtable(c);
    case 'coffeetable':
      return coffeetable(c);
    case 'desk':
      return desk(c);
    case 'bed':
      return bed(c, type.bedW || 0.9);
    case 'nightstand':
      return nightstand(c);
    case 'wardrobe':
      return cabinet(c, 1.2, 2.0, 0.6, 0);
    case 'dresser':
      return cabinet(c, 1.0, 0.85, 0.48, 4);
    case 'bookcase':
      return bookcase(c);
    case 'counter':
      return counter(c);
    case 'kitchen':
      return kitchen(c);
    case 'fridge':
      return fridge(c);
    case 'tv':
      return tv(c);
    case 'piano':
      return piano(c);
    case 'floorlamp':
      return floorlamp(c);
    case 'plant':
      return plant(c);
    case 'rug':
      return rug(c);
    case 'coatrack':
      return coatrack(c);
    case 'divider':
      return divider(c);
    default:
      return null;
  }
}

