// Accessoires montés sur un projecteur dans la vue 3D : boîte à lumière (Chimera), dôme, lanterne,
// louver / grille, gélatine. Posés devant la face avant (+Z local du projecteur). La toile avant
// s'allume avec la couleur émise par le projecteur ; la source de lumière passe sur cette toile.
import * as THREE from 'three';
import { bodyHalfExtents } from '../plot/geometry.js';

const fabric = () => new THREE.MeshStandardMaterial({ color: 0x151517, roughness: 0.95, side: THREE.DoubleSide });
const diffuser = () => new THREE.MeshStandardMaterial({ color: 0xf2f2ee, roughness: 0.9, emissive: 0x000000, emissiveIntensity: 1, side: THREE.DoubleSide });

/** Frustum (boîte à lumière ou dôme) le long de +Z : ouverture arrière, face avant, profondeur. */
function frustum(back, front, depth, sides) {
  const g = new THREE.CylinderGeometry(front, back, depth, sides, 1, true);
  g.rotateX(Math.PI / 2); // haut du cylindre (face avant) vers +Z
  if (sides === 4) g.rotateZ(Math.PI / 4); // côtés alignés sur les axes
  g.translate(0, 0, depth / 2);
  return g;
}

/** Grille (nid d'abeille) de w × h et de profondeur d, cellules d'environ 5 cm. */
function eggcrate(w, h, d, mat) {
  const g = new THREE.Group();
  const nx = Math.max(3, Math.round(w / 0.06));
  const ny = Math.max(3, Math.round(h / 0.06));
  for (let i = 0; i <= nx; i++) {
    const m = new THREE.Mesh(new THREE.BoxGeometry(0.004, h, d), mat);
    m.position.set(-w / 2 + (w / nx) * i, 0, d / 2);
    g.add(m);
  }
  for (let j = 0; j <= ny; j++) {
    const m = new THREE.Mesh(new THREE.BoxGeometry(w, 0.004, d), mat);
    m.position.set(0, -h / 2 + (h / ny) * j, d / 2);
    g.add(m);
  }
  return g;
}

/**
 * Construit les accessoires d'un projecteur. type : type effectif (avec .accessories).
 * Renvoie { group, glow(out), lightZ, soft } ou null : lightZ = nouvelle position de la source
 * (devant la toile), soft = taille de la source (ombres plus douces).
 */
export function buildFixtureAccessories(type) {
  const acc = type.accessories;
  if (!acc) return null;
  const [hx, hy, hz] = bodyHalfExtents(type);
  const round = type.geometry?.kind === 'par';
  const group = new THREE.Group();
  group.position.z = hz;
  const mats = [];
  let front = null;
  let lightZ = hz;
  let face = { w: hx * 2, h: hy * 2, z: 0 }; // surface où se posent grille et gélatine
  let soft = 0;
  const mod = acc.modifier;
  if (mod) {
    const f = fabric();
    const d = diffuser();
    mats.push(f, d);
    front = d;
    if (mod.kind === 'softbox') {
      const w = mod.fit ? hx * 2 * 1.08 : mod.w;
      const h = mod.fit ? hy * 2 * 1.08 : mod.h;
      const backR = round ? Math.max(0.09, Math.min(hx, hy) * 0.9) : Math.max(hx, hy) * Math.SQRT2;
      const shell = new THREE.Mesh(frustum(backR, (Math.max(w, h) / 2) * Math.SQRT2, mod.depth, 4), f);
      shell.scale.set(w >= h ? 1 : w / h, w >= h ? h / w : 1, 1);
      const panel = new THREE.Mesh(new THREE.PlaneGeometry(w, h), d);
      panel.position.z = mod.depth;
      group.add(shell, panel);
      face = { w, h, z: mod.depth };
      lightZ = hz + mod.depth * 0.6;
      soft = Math.max(w, h);
    } else if (mod.kind === 'dome') {
      const shell = new THREE.Mesh(frustum(0.1, mod.d / 2, mod.depth, 16), f);
      const disc = new THREE.Mesh(new THREE.CircleGeometry(mod.d / 2, 40), d);
      disc.position.z = mod.depth;
      group.add(shell, disc);
      face = { w: mod.d * 0.72, h: mod.d * 0.72, z: mod.depth };
      lightZ = hz + mod.depth * 0.6;
      soft = mod.d;
    } else {
      // lanterne : boule de tissu, col noir vers le projecteur
      const ball = new THREE.Mesh(new THREE.SphereGeometry(mod.d / 2, 28, 18), d);
      ball.position.z = mod.d / 2 + 0.08;
      const neck = new THREE.Mesh(frustum(0.1, 0.16, 0.12, 16), f);
      group.add(ball, neck);
      face = null;
      lightZ = hz + mod.d / 2 + 0.08;
      soft = mod.d;
    }
  }
  if (acc.grid && face) {
    const black = new THREE.MeshStandardMaterial({ color: 0x0d0d0e, roughness: 0.8 });
    mats.push(black);
    const crate = eggcrate(face.w, face.h, acc.grid.depth, black);
    crate.position.z = face.z;
    group.add(crate);
  }
  if (acc.gel) {
    const color = new THREE.Color(acc.gel.color);
    const gelMat = new THREE.MeshStandardMaterial({ color, roughness: 0.3, transparent: true, opacity: acc.gel.group === 'diffusion' ? 0.75 : 0.5, side: THREE.DoubleSide, depthWrite: false });
    mats.push(gelMat);
    const fw = face ? face.w : hx * 2;
    const fh = face ? face.h : hy * 2;
    const sheet = round && !mod ? new THREE.Mesh(new THREE.CircleGeometry(Math.min(hx, hy) * 1.02, 32), gelMat) : new THREE.Mesh(new THREE.PlaneGeometry(fw * 1.02, fh * 1.02), gelMat);
    sheet.position.z = (face ? face.z : 0) + 0.012 + (acc.grid ? acc.grid.depth : 0);
    group.add(sheet);
  }
  group.traverse((o) => {
    if (!o.isMesh) return;
    o.castShadow = false; // l'accessoire ne coupe pas sa propre lumière
    o.receiveShadow = false;
  });
  return {
    group,
    lightZ,
    soft,
    /** La toile avant s'éclaire avec la couleur émise (moyenne des pixels). */
    glow(out, pixels) {
      if (!front) return;
      let r = 0;
      let g = 0;
      let b = 0;
      for (let p = 0; p < pixels; p++) {
        r += out[p * 3];
        g += out[p * 3 + 1];
        b += out[p * 3 + 2];
      }
      const k = 1.6 / Math.max(1, pixels);
      front.emissive.setRGB(r * k, g * k, b * k, THREE.LinearSRGBColorSpace);
    },
    dispose() {
      group.traverse((o) => o.geometry?.dispose());
      for (const m of mats) m.dispose();
    },
  };
}
