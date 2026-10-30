// Pieds de projecteur en 3D : pied de 1000, combo, junior roulant, wind-up, Super Wind-Up,
// Long John Silver, C-stand, low-boy.
// Les jambes reposent au sol (ou sur leurs roues) ; le mât télescopique s'allonge jusqu'au bas du projecteur.
import * as THREE from 'three';

const chrome = () => new THREE.MeshStandardMaterial({ color: 0xb8bdc4, metalness: 0.85, roughness: 0.32 });
const black = () => new THREE.MeshStandardMaterial({ color: 0x17191c, metalness: 0.55, roughness: 0.5 });
// acier et alu brossés du Long John Silver : clair même sans reflets d'environnement
const steel = () => new THREE.MeshStandardMaterial({ color: 0xc4c9d0, metalness: 0.45, roughness: 0.42 });
const rubber = () => new THREE.MeshStandardMaterial({ color: 0x0d0e10, metalness: 0.1, roughness: 0.9 });

// Allure de chaque famille : jambes (rayon au sol, hauteur du moyeu, épaisseur), mât (sections, diamètres),
// roues (rayon), entretoises entre le mât et les jambes, manivelle (échelle).
const LOOKS = {
  baby: { mat: chrome, legR: 0.42, hub: 0.55, leg: 0.009, risers: [0.016, 0.013, 0.01] },
  combo: { mat: black, legR: 0.58, hub: 0.6, leg: 0.013, risers: [0.024, 0.019], cup: true },
  // junior roulant (Avenger Roller 34) : trois sections, roues, entretoises
  junior: { mat: black, legR: 0.72, hub: 0.62, leg: 0.016, risers: [0.032, 0.027, 0.022], cup: true, wheels: 0.05, brace: true },
  windup: { mat: black, legR: 0.8, hub: 0.8, leg: 0.018, risers: [0.04, 0.032], cup: true, crank: 1 },
  // Super Wind-Up (Manfrotto / Avenger 387XU) : manivelle, base roulante large
  superwindup: { mat: black, legR: 0.85, hub: 0.78, leg: 0.024, risers: [0.055, 0.045, 0.036], cup: true, crank: 1.4, wheels: 0.065, brace: true },
  // Long John Silver (Avenger B7057FF) : acier et alu, cinq sections, grosses roues freinées
  longjohn: { mat: steel, legR: 0.95, hub: 0.9, leg: 0.026, risers: [0.052, 0.046, 0.04, 0.034, 0.028], cup: true, wheels: 0.08, brace: true },
  cstand: { mat: chrome, legR: 0.38, hub: 0.22, leg: 0.011, risers: [0.017, 0.014], turtle: true, head: true },
  lowboy: { mat: black, legR: 0.5, hub: 0.16, leg: 0.014, risers: [0.024], cup: true },
};

const Y = new THREE.Vector3(0, 1, 0);

/** Oriente un cylindre unitaire (axe Y) entre deux points. */
function placeBetween(mesh, a, b) {
  const d = new THREE.Vector3().subVectors(b, a);
  mesh.position.copy(a).addScaledVector(d, 0.5);
  mesh.scale.set(1, d.length(), 1);
  mesh.quaternion.setFromUnitVectors(Y, d.normalize());
}

export class StandModel {
  constructor(look) {
    this.look = LOOKS[look] || LOOKS.baby;
    const L = this.look;
    this.group = new THREE.Group();
    this.mat = L.mat();
    this.mats = [this.mat];
    this.legs = [];
    this.braces = [];
    this.wheels = [];
    const rub = L.wheels ? rubber() : null;
    if (rub) this.mats.push(rub);
    for (let i = 0; i < 3; i++) {
      const leg = new THREE.Mesh(new THREE.CylinderGeometry(L.leg, L.leg * 1.1, 1, 8), this.mat);
      this.group.add(leg);
      this.legs.push(leg);
      if (L.turtle) {
        // C-stand : pieds plats décalés en hauteur (base « tortue »)
        const foot = new THREE.Mesh(new THREE.CylinderGeometry(0.016, 0.016, 0.02, 10), this.mat);
        this.group.add(foot);
        leg.userData.foot = foot;
      }
      if (L.brace) {
        const brace = new THREE.Mesh(new THREE.CylinderGeometry(L.leg * 0.55, L.leg * 0.55, 1, 6), this.mat);
        this.group.add(brace);
        this.braces.push(brace);
      }
      if (L.wheels) {
        // roulette : roue en caoutchouc et chape (pivot vertical) au bout de la jambe
        const r = L.wheels;
        const caster = new THREE.Group();
        const wheel = new THREE.Mesh(new THREE.CylinderGeometry(r, r, r * 0.55, 18), rub);
        wheel.rotation.z = Math.PI / 2;
        wheel.position.y = r;
        const fork = new THREE.Mesh(new THREE.BoxGeometry(r * 0.75, r * 1.15, r * 0.35), this.mat);
        fork.position.y = r * 1.45;
        const pin = new THREE.Mesh(new THREE.CylinderGeometry(r * 0.16, r * 0.16, r * 0.6, 8), this.mat);
        pin.position.y = r * 2.2;
        caster.add(wheel, fork, pin);
        caster.rotation.y = -((i * Math.PI * 2) / 3 + Math.PI / 6);
        this.group.add(caster);
        this.wheels.push(caster);
      }
    }
    this.risers = L.risers.map((r) => {
      const m = new THREE.Mesh(new THREE.CylinderGeometry(r, r, 1, 12), this.mat);
      this.group.add(m);
      return m;
    });
    this.hubMesh = new THREE.Mesh(new THREE.CylinderGeometry(L.risers[0] * 1.6, L.risers[0] * 1.6, 0.07, 12), this.mat);
    this.group.add(this.hubMesh);
    if (L.brace) {
      this.collar = new THREE.Mesh(new THREE.CylinderGeometry(L.risers[0] * 1.45, L.risers[0] * 1.45, 0.05, 12), this.mat);
      this.group.add(this.collar);
    }
    if (L.cup) {
      // réception : 16 mm (combo) ou 28 mm (junior et plus gros)
      const big = L.risers[0] >= 0.03;
      this.cup = new THREE.Mesh(new THREE.CylinderGeometry(big ? 0.034 : 0.026, big ? 0.03 : 0.022, big ? 0.08 : 0.06, 12), this.mat);
      this.group.add(this.cup);
    }
    if (L.crank) {
      const k = L.crank;
      this.crank = new THREE.Group();
      const box = new THREE.Mesh(new THREE.BoxGeometry(0.1 * k, 0.14 * k, 0.08 * k), this.mat);
      const handle = new THREE.Mesh(new THREE.CylinderGeometry(0.006 * k, 0.006 * k, 0.12 * k, 8), this.mat);
      handle.rotation.z = Math.PI / 2;
      handle.position.set(0.11 * k, -0.03 * k, 0);
      this.crank.add(box, handle);
      this.group.add(this.crank);
    }
    if (L.head) {
      this.headMesh = new THREE.Mesh(new THREE.CylinderGeometry(0.03, 0.03, 0.025, 14), this.mat);
      this.headMesh.rotation.x = Math.PI / 2;
      this.group.add(this.headMesh);
    }
    this.top = -1;
  }

  /** Hauteur du haut du pied (bas du projecteur). */
  setTop(top) {
    if (Math.abs(top - this.top) < 1e-4) return;
    this.top = top;
    const L = this.look;
    const hub = Math.min(L.hub, top * 0.55);
    const footLift = L.wheels ? L.wheels * 2.5 : 0; // la jambe s'arrête au-dessus de la roulette
    const collarY = hub * 0.42;
    for (let i = 0; i < 3; i++) {
      const a = (i * Math.PI * 2) / 3 + Math.PI / 6;
      const footY = L.turtle ? 0.02 + i * 0.06 : footLift;
      const foot = new THREE.Vector3(Math.cos(a) * L.legR, footY, Math.sin(a) * L.legR);
      const from = L.turtle ? new THREE.Vector3(0, footY + 0.01, 0) : new THREE.Vector3(0, hub, 0);
      placeBetween(this.legs[i], from, foot);
      if (L.turtle) this.legs[i].userData.foot.position.set(foot.x, footY / 2 + 0.01, foot.z);
      if (this.braces[i]) placeBetween(this.braces[i], new THREE.Vector3(0, collarY, 0), from.clone().lerp(foot, 0.55));
      if (this.wheels[i]) this.wheels[i].position.set(foot.x, 0, foot.z);
    }
    const base = L.turtle ? 0 : Math.min(hub - 0.05, L.brace ? collarY - 0.03 : hub - 0.05);
    this.hubMesh.position.y = hub;
    if (this.collar) this.collar.position.y = collarY;
    // mât télescopique : sections qui se recouvrent, la plus fine en haut
    const n = this.risers.length;
    const span = Math.max(0.05, top - base);
    const seg = span / n + 0.08;
    for (let i = 0; i < n; i++) {
      const y0 = base + (span / n) * i;
      const len = Math.min(seg, top - y0);
      this.risers[i].scale.y = Math.max(0.01, len);
      this.risers[i].position.y = y0 + len / 2;
    }
    if (this.cup) this.cup.position.y = top - 0.04;
    if (this.crank) this.crank.position.set(0.05 * L.crank, hub + 0.3, 0);
    if (this.headMesh) this.headMesh.position.set(0, top - 0.04, 0.03);
  }

  dispose() {
    this.group.traverse((o) => o.geometry?.dispose());
    for (const m of this.mats) m.dispose();
  }
}
