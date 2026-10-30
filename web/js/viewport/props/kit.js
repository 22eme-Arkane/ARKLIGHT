// Petit atelier de modélisation pour le décor : pièces simples (boîtes, cylindres, capsules,
// tours, extrusions…) posées dans le repère de l'objet (sol en y = 0, face avant vers +Z),
// regroupées par matière puis fusionnées : un seul maillage par matière, donc peu d'appels de
// dessin même avec beaucoup d'objets dans la scène.
import * as THREE from 'three';
import { mergeGeometries } from 'three/addons/utils/BufferGeometryUtils.js';
import { RoundedBoxGeometry } from 'three/addons/geometries/RoundedBoxGeometry.js';

const UP = new THREE.Vector3(0, 1, 0);
const vec = (p) => (p instanceof THREE.Vector3 ? p : new THREE.Vector3(p[0], p[1], p[2]));

export class Kit {
  constructor() {
    this.mats = new Map(); // nom -> { color, rough, metal, opacity, emissive, flat, double }
    this.parts = new Map(); // nom -> [géométries déjà placées]
  }

  /** Déclare une matière (couleur et finition) et renvoie son nom. */
  mat(name, color, { rough = 0.8, metal = 0, opacity = 1, emissive = null, flat = false, double = false } = {}) {
    this.mats.set(name, { color, rough, metal, opacity, emissive, flat, double });
    return name;
  }

  /** Ajoute une géométrie placée : p (position), r (angles XYZ) ou q (quaternion), s (échelle). */
  add(geo, mat, { p = [0, 0, 0], r = null, q = null, s = null } = {}) {
    const quat = q || (r ? new THREE.Quaternion().setFromEuler(new THREE.Euler(r[0], r[1], r[2])) : new THREE.Quaternion());
    const m = new THREE.Matrix4().compose(vec(p), quat, s ? new THREE.Vector3(s[0], s[1], s[2]) : new THREE.Vector3(1, 1, 1));
    const g = geo.index ? geo.toNonIndexed() : geo;
    if (g !== geo) geo.dispose();
    g.applyMatrix4(m);
    // mêmes attributs partout (position, normale, uv) : les pièces d'une matière se fusionnent
    for (const k of Object.keys(g.attributes)) if (!['position', 'normal', 'uv'].includes(k)) g.deleteAttribute(k);
    if (!g.attributes.normal) g.computeVertexNormals();
    if (!g.attributes.uv) g.setAttribute('uv', new THREE.Float32BufferAttribute(new Float32Array(g.attributes.position.count * 2), 2));
    g.clearGroups();
    if (!this.parts.has(mat)) this.parts.set(mat, []);
    this.parts.get(mat).push(g);
    return this;
  }

  box(w, h, d, mat, p, o = {}) {
    return this.add(new THREE.BoxGeometry(w, h, d), mat, { ...o, p });
  }

  /** Boîte aux arêtes arrondies (coussins, carrosseries, meubles). */
  rbox(w, h, d, radius, mat, p, o = {}) {
    const rad = Math.max(0.001, Math.min(radius, w / 2 - 0.001, h / 2 - 0.001, d / 2 - 0.001));
    return this.add(new RoundedBoxGeometry(w, h, d, 2, rad), mat, { ...o, p });
  }

  /** Cylindre vertical centré en p (rTop en haut, rBot en bas). */
  cyl(rTop, rBot, h, mat, p, o = {}) {
    return this.add(new THREE.CylinderGeometry(rTop, rBot, h, o.seg || 16, 1, !!o.open, o.start || 0, o.arc ?? Math.PI * 2), mat, { ...o, p });
  }

  ball(r, mat, p, o = {}) {
    const seg = o.seg || 16;
    return this.add(new THREE.SphereGeometry(r, seg, Math.max(6, Math.round(seg * 0.7)), 0, Math.PI * 2, o.theta0 || 0, o.theta ?? Math.PI), mat, { ...o, p });
  }

  cone(r, h, mat, p, o = {}) {
    return this.add(new THREE.ConeGeometry(r, h, o.seg || 16, 1, !!o.open), mat, { ...o, p });
  }

  torus(R, r, mat, p, o = {}) {
    return this.add(new THREE.TorusGeometry(R, r, o.tube || 8, o.seg || 24, o.arc ?? Math.PI * 2), mat, { ...o, p });
  }

  /** Pierre, feuillage : polyèdre facetté. */
  ico(r, mat, p, o = {}) {
    return this.add(new THREE.IcosahedronGeometry(r, o.detail ?? 1), mat, { ...o, p });
  }

  /** Orientation qui amène l'axe Y sur le segment a → b. */
  static along(a, b) {
    const d = vec(b).clone().sub(vec(a));
    const len = d.length();
    return { len, q: new THREE.Quaternion().setFromUnitVectors(UP, len > 1e-6 ? d.normalize() : UP), mid: vec(a).clone().add(vec(b)).multiplyScalar(0.5) };
  }

  /** Capsule (membre, accoudoir…) entre deux points ; s : aplatissement éventuel [x, y, z]. */
  capsule(a, b, r, mat, o = {}) {
    const { len, q, mid } = Kit.along(a, b);
    return this.add(new THREE.CapsuleGeometry(r, Math.max(0.0005, len), o.cap || 4, o.seg || 12), mat, { q, p: mid, s: o.s });
  }

  /** Tube droit entre deux points (rayon r en a, r2 en b). */
  rod(a, b, r, mat, o = {}) {
    const { len, q, mid } = Kit.along(a, b);
    return this.add(new THREE.CylinderGeometry(o.r2 ?? r, r, Math.max(0.0005, len), o.seg || 10, 1, !!o.open), mat, { q, p: mid, s: o.s });
  }

  /** Volume de révolution : profil [[rayon, hauteur], …] autour de l'axe Y. */
  lathe(points, mat, p = [0, 0, 0], o = {}) {
    return this.add(new THREE.LatheGeometry(points.map(([r, y]) => new THREE.Vector2(Math.max(0.0005, r), y)), o.seg || 20), mat, { ...o, p });
  }

  /**
   * Profil extrudé : shape (THREE.Shape ou points [[x, y], …]) dans le plan XY, épaisseur depth
   * centrée sur Z. o.holes : contours à évider ; o.bevel : arrondi des arêtes.
   */
  extrude(shape, depth, mat, o = {}) {
    const sh = shape instanceof THREE.Shape ? shape : new THREE.Shape(shape.map(([x, y]) => new THREE.Vector2(x, y)));
    for (const hole of o.holes || []) sh.holes.push(new THREE.Path(hole.map(([x, y]) => new THREE.Vector2(x, y))));
    const bevel = o.bevel || 0;
    const geo = new THREE.ExtrudeGeometry(sh, { depth: Math.max(0.001, depth - bevel * 2), bevelEnabled: bevel > 0, bevelSize: bevel, bevelThickness: bevel, bevelSegments: 2, curveSegments: o.curve || 10 });
    geo.translate(0, 0, -(depth - bevel * 2) / 2);
    return this.add(geo, mat, o);
  }

  /** Groupe final : un maillage par matière, qui porte et reçoit les ombres. */
  build() {
    const group = new THREE.Group();
    const materials = [];
    for (const [name, geos] of this.parts) {
      if (!geos.length) continue;
      const spec = this.mats.get(name) || { color: '#8a8f96', rough: 0.8, metal: 0, opacity: 1 };
      const geo = geos.length === 1 ? geos[0] : mergeGeometries(geos, false);
      if (geos.length > 1) for (const g of geos) g.dispose();
      const transparent = spec.opacity < 1;
      const mat = new THREE.MeshStandardMaterial({
        color: spec.color,
        roughness: spec.rough,
        metalness: spec.metal,
        transparent,
        opacity: spec.opacity,
        depthWrite: !transparent,
        flatShading: spec.flat,
        side: spec.double ? THREE.DoubleSide : THREE.FrontSide,
      });
      if (spec.emissive) {
        mat.emissive = new THREE.Color(spec.emissive);
        mat.emissiveIntensity = 0.5;
      }
      const mesh = new THREE.Mesh(geo, mat);
      mesh.castShadow = !transparent || spec.opacity >= 0.5;
      mesh.receiveShadow = true;
      mesh.name = name;
      group.add(mesh);
      materials.push(mat);
    }
    this.parts.clear();
    return { group, materials };
  }
}

/** Couleur un peu plus sombre ou plus claire (k < 1 : plus sombre). */
export function shade(hex, k) {
  const c = new THREE.Color(hex);
  if (k < 1) c.multiplyScalar(k);
  else c.lerp(new THREE.Color('#ffffff'), Math.min(1, k - 1));
  return `#${c.getHexString()}`;
}

/** Petit générateur pseudo-aléatoire stable (mêmes livres, mêmes feuilles à chaque construction). */
export function rng(seed = 1) {
  let s = seed >>> 0 || 1;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}
