// Pieds de projecteur en 3D : pied de 1000, combo / junior, wind-up, C-stand, low-boy.
// Les jambes reposent au sol ; le mât télescopique s'allonge jusqu'au bas du projecteur.
import * as THREE from 'three';

const chrome = () => new THREE.MeshStandardMaterial({ color: 0xb8bdc4, metalness: 0.85, roughness: 0.32 });
const black = () => new THREE.MeshStandardMaterial({ color: 0x17191c, metalness: 0.55, roughness: 0.5 });

// Allure de chaque famille : jambes (rayon au sol, hauteur du moyeu, épaisseur), mât (sections, diamètres)
const LOOKS = {
  baby: { mat: chrome, legR: 0.42, hub: 0.55, leg: 0.009, risers: [0.016, 0.013, 0.01] },
  combo: { mat: black, legR: 0.58, hub: 0.6, leg: 0.013, risers: [0.024, 0.019], cup: true },
  windup: { mat: black, legR: 0.8, hub: 0.8, leg: 0.018, risers: [0.04, 0.032], cup: true, crank: true },
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
    this.legs = [];
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
    }
    this.risers = L.risers.map((r) => {
      const m = new THREE.Mesh(new THREE.CylinderGeometry(r, r, 1, 12), this.mat);
      this.group.add(m);
      return m;
    });
    this.hubMesh = new THREE.Mesh(new THREE.CylinderGeometry(L.risers[0] * 1.6, L.risers[0] * 1.6, 0.07, 12), this.mat);
    this.group.add(this.hubMesh);
    if (L.cup) {
      this.cup = new THREE.Mesh(new THREE.CylinderGeometry(0.026, 0.022, 0.06, 12), this.mat);
      this.group.add(this.cup);
    }
    if (L.crank) {
      this.crank = new THREE.Group();
      const box = new THREE.Mesh(new THREE.BoxGeometry(0.1, 0.14, 0.08), this.mat);
      const handle = new THREE.Mesh(new THREE.CylinderGeometry(0.006, 0.006, 0.12, 8), this.mat);
      handle.rotation.z = Math.PI / 2;
      handle.position.set(0.11, -0.03, 0);
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
    for (let i = 0; i < 3; i++) {
      const a = (i * Math.PI * 2) / 3 + Math.PI / 6;
      const footY = L.turtle ? 0.02 + i * 0.06 : 0;
      const foot = new THREE.Vector3(Math.cos(a) * L.legR, footY, Math.sin(a) * L.legR);
      const from = L.turtle ? new THREE.Vector3(0, footY + 0.01, 0) : new THREE.Vector3(0, hub, 0);
      placeBetween(this.legs[i], from, foot);
      if (L.turtle) this.legs[i].userData.foot.position.set(foot.x, footY / 2 + 0.01, foot.z);
    }
    const base = L.turtle ? 0 : hub - 0.05;
    this.hubMesh.position.y = hub;
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
    if (this.cup) this.cup.position.y = top - 0.03;
    if (this.crank) this.crank.position.set(0.05, hub + 0.3, 0);
    if (this.headMesh) this.headMesh.position.set(0, top - 0.04, 0.03);
  }

  dispose() {
    this.group.traverse((o) => o.geometry?.dispose());
    this.mat.dispose();
  }
}
