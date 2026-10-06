// Géométrie du plan de feu (vue de dessus) : empreintes des projecteurs et du grip, faisceaux.
// Repère du plan : x vers la droite, z vers le bas de l'écran (le fond / cyclo en haut, la caméra en bas).
import * as THREE from 'three';

/** Demi-dimensions locales du corps d'un projecteur (axes du modèle 3D). */
export function bodyHalfExtents(type) {
  const g = type.geometry || {};
  switch (g.kind) {
    case 'tube':
      return [g.diameter / 2, g.length / 2, g.diameter / 2];
    case 'bar':
      return [g.length / 2, g.height / 2, g.depth / 2];
    case 'par':
      return [g.diameter / 2, g.diameter / 2, g.depth / 2];
    case 'bulb':
      return [g.diameter / 2, (g.length || g.diameter) / 2, g.diameter / 2];
    default:
      return [(g.width || 0.3) / 2, (g.height || 0.3) / 2, (g.depth || 0.1) / 2];
  }
}

const tmpM = new THREE.Matrix4();
const tmpE = new THREE.Euler();

/** Matrice de rotation d'un élément de scène (rotation XYZ en radians). */
export function rotationMatrix(rot) {
  return tmpM.clone().makeRotationFromEuler(tmpE.set(rot[0] || 0, rot[1] || 0, rot[2] || 0));
}

/** Enveloppe convexe (algorithme du monotone chain) d'une liste de points [x, z]. */
export function convexHull(points) {
  const pts = [...points].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  if (pts.length < 3) return pts;
  const cross = (o, a, b) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const lower = [];
  for (const p of pts) {
    while (lower.length >= 2 && cross(lower.at(-2), lower.at(-1), p) <= 0) lower.pop();
    lower.push(p);
  }
  const upper = [];
  for (const p of [...pts].reverse()) {
    while (upper.length >= 2 && cross(upper.at(-2), upper.at(-1), p) <= 0) upper.pop();
    upper.push(p);
  }
  return [...lower.slice(0, -1), ...upper.slice(0, -1)];
}

/** Empreinte au sol d'une boîte locale (demi-dimensions) placée et orientée dans la scène. */
export function footprint(half, position, rotation) {
  const m = rotationMatrix(rotation);
  const corners = [];
  for (const sx of [-1, 1]) for (const sy of [-1, 1]) for (const sz of [-1, 1]) {
    const v = new THREE.Vector3(sx * half[0], sy * half[1], sz * half[2]).applyMatrix4(m);
    corners.push([position[0] + v.x, position[2] + v.z]);
  }
  return convexHull(corners);
}

/**
 * Projecteur vu de dessus : empreinte du corps, direction du faisceau projetée au sol
 * (null s'il pointe presque à la verticale) et point de départ du faisceau (face avant).
 */
export function fixturePlan(fixture, type) {
  const half = bodyHalfExtents(type);
  const poly = footprint(half, fixture.position, fixture.rotation);
  const m = rotationMatrix(fixture.rotation);
  const axis = new THREE.Vector3(0, 0, 1).applyMatrix4(m); // axe du faisceau (+Z local)
  const flat = Math.hypot(axis.x, axis.z);
  const kind = type.geometry?.kind;
  const omni = kind === 'bulb' || kind === 'tube' || kind === 'box-device' || !type.pixels; // les tubes éclairent tout autour
  const front = new THREE.Vector3(0, 0, half[2]).applyMatrix4(m);
  return {
    poly,
    center: [fixture.position[0], fixture.position[2]],
    dir: !omni && flat > 0.25 ? [axis.x / flat, axis.z / flat] : null,
    down: !omni && flat <= 0.25 ? axis.y < 0 : false,
    front: [fixture.position[0] + front.x, fixture.position[2] + front.z],
    height: fixture.position[1],
  };
}

/** Élément de grip vu de dessus : empreinte selon sa famille. */
export function objectPlan(obj, type) {
  const fam = type.family;
  let half;
  if (fam === 'frame' || fam === 'flag') half = [type.w / 2, type.h / 2, 0.02];
  else if (fam === 'bounce') half = [type.w / 2, (type.h || type.w) / 2, 0.03];
  else if (fam === 'box' || fam === 'bag') half = type.size.map((v) => v / 2);
  else half = [0.35, 0.05, 0.35]; // pied seul : emprise des jambes
  return { poly: footprint(half, obj.position, obj.rotation), center: [obj.position[0], obj.position[2]] };
}

/** Distance d'un point à un segment (pour cliquer sur les lignes et flèches). */
export function distToSegment([px, pz], [ax, az], [bx, bz]) {
  const dx = bx - ax;
  const dz = bz - az;
  const L = dx * dx + dz * dz || 1e-9;
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (pz - az) * dz) / L));
  return Math.hypot(px - (ax + t * dx), pz - (az + t * dz));
}

/** Point dans un polygone (règle pair-impair). */
export function pointInPolygon([x, z], poly) {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, zi] = poly[i];
    const [xj, zj] = poly[j];
    if (zi > z !== zj > z && x < ((xj - xi) * (z - zi)) / (zj - zi + 1e-12) + xi) inside = !inside;
  }
  return inside;
}

/** Tourne un élément autour de la verticale (lacet) en gardant son inclinaison. */
export function rotateYaw(rotation, delta) {
  const q = new THREE.Quaternion().setFromEuler(new THREE.Euler(rotation[0], rotation[1], rotation[2]));
  q.premultiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 1, 0), delta));
  const e = new THREE.Euler().setFromQuaternion(q);
  const r3 = (v) => Math.round(v * 1000) / 1000;
  return [r3(e.x), r3(e.y), r3(e.z)];
}
