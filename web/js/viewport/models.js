// Modèles 3D des projecteurs. Chaque modèle reçoit à chaque image la couleur de ses pixels
// (sRGB linéaire) et met à jour ses matériaux émissifs, ses lumières et ses faisceaux.
import * as THREE from 'three';

/**
 * Calibrage global : facteur appliqué aux candelas physiques. Avec 0.02, un éclairement de ~160 lux
 * donne un gris moyen correctement exposé (exposition 1).
 */
const LIGHT_SCALE = 0.02;
const EMISSIVE_GAIN = 1.6;
const BEAM_LENGTH = 7;

const bodyMat = () => new THREE.MeshStandardMaterial({ color: 0x1c1f23, roughness: 0.55, metalness: 0.35 });

function averageRgb(px, from, to) {
  let r = 0;
  let g = 0;
  let b = 0;
  const n = Math.max(1, to - from);
  for (let p = from; p < to; p++) {
    r += px[p * 3];
    g += px[p * 3 + 1];
    b += px[p * 3 + 2];
  }
  return [r / n, g / n, b / n];
}

/** Applique une couleur (avec intensité) à une lumière : couleur normalisée + intensité. */
function driveLight(light, rgb, candela) {
  const m = Math.max(rgb[0], rgb[1], rgb[2]);
  if (m <= 1e-4) {
    light.intensity = 0;
    return 0;
  }
  light.color.setRGB(rgb[0] / m, rgb[1] / m, rgb[2] / m, THREE.LinearSRGBColorSpace);
  light.intensity = candela * m;
  return m;
}

// ------------------------------------------------------------------ faisceau volumétrique
const beamVertex = /* glsl */ `
  uniform float uLength;
  varying float vAlong;
  varying vec3 vNormalV;
  varying vec3 vViewV;
  varying vec3 vWorld;
  void main() {
    vAlong = clamp(position.z / uLength, 0.0, 1.0);
    vec4 world = modelMatrix * vec4(position, 1.0);
    vWorld = world.xyz;
    vec4 mv = viewMatrix * world;
    vNormalV = normalize(normalMatrix * normal);
    vViewV = normalize(-mv.xyz);
    gl_Position = projectionMatrix * mv;
  }
`;
// Grip dans le faisceau (drapeau, cadre, réflecteur) : rectangles dans l'espace du monde.
// Derrière un rectangle (vu depuis la source), le faisceau est multiplié par sa transmission
// (0 pour un drapeau noir, 0,5 pour un tulle simple…).
export const MAX_BEAM_OCCLUDERS = 4;
const beamFragment = /* glsl */ `
  uniform vec3 uColor;
  uniform float uIntensity;
  uniform vec3 uApex;
  uniform int uOccCount;
  uniform vec3 uOccC[${4}];
  uniform vec3 uOccU[${4}];
  uniform vec3 uOccV[${4}];
  uniform float uOccT[${4}];
  varying float vAlong;
  varying vec3 vNormalV;
  varying vec3 vViewV;
  varying vec3 vWorld;
  float occlusion() {
    float k = 1.0;
    vec3 d = vWorld - uApex;
    for (int i = 0; i < ${4}; i++) {
      if (i >= uOccCount) break;
      vec3 n = cross(uOccU[i], uOccV[i]);
      float denom = dot(n, d);
      if (abs(denom) < 1e-7) continue;
      float t = dot(n, uOccC[i] - uApex) / denom;
      if (t <= 0.0 || t >= 1.0) continue;
      vec3 p = uApex + d * t - uOccC[i];
      float a = dot(p, uOccU[i]) / dot(uOccU[i], uOccU[i]);
      float b = dot(p, uOccV[i]) / dot(uOccV[i], uOccV[i]);
      if (abs(a) <= 1.0 && abs(b) <= 1.0) k *= uOccT[i];
    }
    return k;
  }
  void main() {
    float edge = pow(abs(dot(vNormalV, vViewV)), 1.6);
    float along = pow(1.0 - vAlong, 1.4) * smoothstep(0.0, 0.04, vAlong);
    float a = uIntensity * edge * along;
    if (uOccCount > 0) a *= occlusion();
    gl_FragColor = vec4(uColor * a, 1.0);
  }
`;

function createBeam(lensRadius, halfAngle) {
  const far = Math.tan(halfAngle) * BEAM_LENGTH + lensRadius;
  const geo = new THREE.CylinderGeometry(lensRadius, far, BEAM_LENGTH, 40, 1, true);
  geo.translate(0, -BEAM_LENGTH / 2, 0);
  geo.rotateX(-Math.PI / 2); // axe du faisceau = +Z local
  const v3 = () => Array.from({ length: 4 }, () => new THREE.Vector3());
  const mat = new THREE.ShaderMaterial({
    uniforms: {
      uColor: { value: new THREE.Color(0, 0, 0) },
      uIntensity: { value: 0 },
      uLength: { value: BEAM_LENGTH },
      uApex: { value: new THREE.Vector3() },
      uOccCount: { value: 0 },
      uOccC: { value: v3() },
      uOccU: { value: v3() },
      uOccV: { value: v3() },
      uOccT: { value: [1, 1, 1, 1] },
    },
    vertexShader: beamVertex,
    fragmentShader: beamFragment,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.DoubleSide,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.renderOrder = 10;
  mesh.frustumCulled = false;
  mesh.userData.isBeam = true;
  mesh.userData.halfAngle = halfAngle;
  return mesh;
}

// ------------------------------------------------------------------ base
class BaseModel {
  constructor(type) {
    this.type = type;
    this.root = new THREE.Group();
    this.pickables = [];
    this.selection = null;
    this.haze = 0.25;
  }

  _finish() {
    for (const m of this.pickables) m.userData.pickRoot = this.root;
    const box = new THREE.Box3();
    for (const m of this.pickables) {
      m.geometry.computeBoundingBox();
      const b = m.geometry.boundingBox.clone().applyMatrix4(m.matrix);
      box.union(b);
    }
    const size = box.getSize(new THREE.Vector3()).addScalar(0.04);
    const center = box.getCenter(new THREE.Vector3());
    const edges = new THREE.EdgesGeometry(new THREE.BoxGeometry(size.x, size.y, size.z));
    this.selection = new THREE.LineSegments(edges, new THREE.LineBasicMaterial({ color: 0x4d8dff, transparent: true, opacity: 0.9 }));
    this.selection.position.copy(center);
    this.selection.visible = false;
    this.root.add(this.selection);
  }

  setSelected(on) {
    if (this.selection) this.selection.visible = on;
  }

  setHaze(h) {
    this.haze = h;
  }

  /** Candelas d'une source omnidirectionnelle (lux à 1 m si connu, sinon lumens / 4π). */
  _omniCandela(share = 1) {
    const o = this.type.output || {};
    const cd = o.luxAt1m ?? (o.lumens || 500) / (4 * Math.PI);
    return cd * share * LIGHT_SCALE;
  }

  /** Candelas sur l'axe d'une source à faisceau (lux à 1 m si connu, sinon lumens / angle solide). */
  _spotCandela(halfAngle) {
    const o = this.type.output || {};
    const cd = o.luxAt1m ?? (o.lumens || 500) / (2 * Math.PI * (1 - Math.cos(halfAngle)));
    return cd * LIGHT_SCALE;
  }

  dispose() {
    this.root.traverse((o) => {
      o.geometry?.dispose?.();
      if (Array.isArray(o.material)) o.material.forEach((m) => m.dispose());
      else o.material?.dispose?.();
    });
  }
}

// ------------------------------------------------------------------ tube (axe = Y local, pixel 1 en bas)
class TubeModel extends BaseModel {
  constructor(type, quality = 2) {
    super(type);
    const { length: L, diameter: D } = type.geometry;
    const r = D / 2;
    const cap = Math.min(0.035, L * 0.04);
    const zone = L - 2 * cap;
    const n = type.pixels;
    const seg = zone / n;
    const segGeo = new THREE.CylinderGeometry(r, r, seg * 0.985, 20, 1, true);
    this.pixelMats = [];
    for (let i = 0; i < n; i++) {
      const mat = new THREE.MeshStandardMaterial({ color: 0x8e939b, roughness: 0.35, metalness: 0, emissive: 0x000000, emissiveIntensity: EMISSIVE_GAIN });
      const mesh = new THREE.Mesh(segGeo, mat);
      mesh.position.y = -zone / 2 + seg * (i + 0.5);
      this.root.add(mesh);
      this.pixelMats.push(mat);
      this.pickables.push(mesh);
    }
    const capGeo = new THREE.CylinderGeometry(r * 1.1, r * 1.1, cap, 20);
    for (const sgn of [-1, 1]) {
      const c = new THREE.Mesh(capGeo, bodyMat());
      c.position.y = sgn * (zone / 2 + cap / 2);
      this.root.add(c);
      this.pickables.push(c);
    }
    // Lumières réparties le long du tube
    const count = Math.max(1, Math.min(4, quality, n));
    this.lights = [];
    for (let i = 0; i < count; i++) {
      const light = new THREE.PointLight(0xffffff, 0, 0, 2);
      light.position.y = -zone / 2 + (zone / count) * (i + 0.5);
      this.root.add(light);
      this.lights.push({ light, from: Math.floor((n * i) / count), to: Math.floor((n * (i + 1)) / count) });
    }
    this.candela = this._omniCandela(1 / count);
    this.root.updateMatrixWorld(true);
    this._finish();
  }

  update(px) {
    for (let i = 0; i < this.pixelMats.length; i++) {
      this.pixelMats[i].emissive.setRGB(px[i * 3], px[i * 3 + 1], px[i * 3 + 2], THREE.LinearSRGBColorSpace);
    }
    for (const l of this.lights) driveLight(l.light, averageRgb(px, l.from, l.to), this.candela);
  }
}

// ------------------------------------------------------------------ projecteurs à faisceau (axe = +Z local)
class BeamModel extends BaseModel {
  _addSpot(lensRadius, lensZ, beamAngleDeg) {
    const half = THREE.MathUtils.degToRad(Math.min(170, beamAngleDeg || 40) / 2);
    const spot = new THREE.SpotLight(0xffffff, 0, 0, Math.min(Math.PI / 2 - 0.01, half * 1.25), 0.55, 2);
    spot.position.set(0, 0, lensZ);
    const target = new THREE.Object3D();
    target.position.set(0, 0, lensZ + 1);
    this.root.add(spot, target);
    spot.target = target;
    this.spot = spot;
    this.candela = this._spotCandela(half);
    if (beamAngleDeg <= 90) {
      this.beam = createBeam(lensRadius * 0.9, half);
      this.beam.position.z = lensZ;
      this.root.add(this.beam);
    }
  }

  _driveBeam(rgb, m) {
    if (!this.beam) return;
    const u = this.beam.material.uniforms;
    if (m <= 1e-4) {
      u.uIntensity.value = 0;
      return;
    }
    u.uColor.value.setRGB(rgb[0] / m, rgb[1] / m, rgb[2] / m, THREE.LinearSRGBColorSpace);
    u.uIntensity.value = Math.min(1.5, m) * this.haze * 0.35;
  }

  update(px) {
    const rgb = averageRgb(px, 0, this.type.pixels);
    for (const mat of this.lensMats) mat.emissive.setRGB(rgb[0], rgb[1], rgb[2], THREE.LinearSRGBColorSpace);
    const m = driveLight(this.spot, rgb, this.candela);
    this._driveBeam(rgb, m);
  }
}

class ParModel extends BeamModel {
  constructor(type) {
    super(type);
    const { diameter: D, depth } = type.geometry;
    const r = D / 2;
    const bodyGeo = new THREE.CylinderGeometry(r, r * 0.85, depth, 28);
    bodyGeo.rotateX(Math.PI / 2);
    const body = new THREE.Mesh(bodyGeo, bodyMat());
    this.root.add(body);
    const lensMat = new THREE.MeshStandardMaterial({ color: 0x30343a, roughness: 0.2, emissive: 0x000000, emissiveIntensity: EMISSIVE_GAIN });
    const lenses = type.geometry.lenses || 1;
    const lens = new THREE.Group();
    if (lenses === 1) lens.add(new THREE.Mesh(new THREE.CircleGeometry(r * 0.86, 32), lensMat));
    else if (lenses <= 4) {
      // couronne sans optique centrale : 3 en triangle (AX5), 4 en carré (AX7)
      const lr = r * (lenses === 3 ? 0.34 : 0.3);
      for (let i = 0; i < lenses; i++) {
        const a = (i / lenses) * Math.PI * 2 + (lenses === 4 ? Math.PI / 4 : Math.PI / 2);
        const m = new THREE.Mesh(new THREE.CircleGeometry(lr, 24), lensMat);
        m.position.set(Math.cos(a) * r * 0.42, Math.sin(a) * r * 0.42, 0);
        lens.add(m);
      }
    } else {
      // optique centrale + couronne (AX9 : 7, AX10 : 9)
      const lr = r * 0.27;
      const ring = lenses - 1;
      lens.add(new THREE.Mesh(new THREE.CircleGeometry(lr, 24), lensMat));
      for (let i = 0; i < ring; i++) {
        const a = (i / ring) * Math.PI * 2;
        const m = new THREE.Mesh(new THREE.CircleGeometry(lr, 24), lensMat);
        m.position.set(Math.cos(a) * r * 0.58, Math.sin(a) * r * 0.58, 0);
        lens.add(m);
      }
    }
    lens.position.z = depth / 2 + 0.001;
    this.root.add(lens);
    // Lyre
    const armGeo = new THREE.BoxGeometry(0.012, r * 2.3, 0.03);
    const yokeMat = bodyMat();
    for (const sgn of [-1, 1]) {
      const arm = new THREE.Mesh(armGeo, yokeMat);
      arm.position.set(sgn * (r + 0.015), -r * 0.25, 0);
      this.root.add(arm);
      this.pickables.push(arm);
    }
    const bar = new THREE.Mesh(new THREE.BoxGeometry(r * 2 + 0.042, 0.012, 0.03), yokeMat);
    bar.position.set(0, -r * 1.4, 0);
    this.root.add(bar);
    this.pickables.push(body, bar, ...lens.children);
    this.lensMats = [lensMat];
    this._addSpot(r * 0.86, depth / 2 + 0.002, type.beamAngle);
    this.root.updateMatrixWorld(true);
    this._finish();
  }
}

class BoxModel extends BeamModel {
  constructor(type) {
    super(type);
    const { width: w, height: h, depth: d } = type.geometry;
    const body = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), bodyMat());
    this.root.add(body);
    this.pickables.push(body);
    // Face émettrice : une zone par pixel (grille 2 rangées si plusieurs pixels)
    const n = type.pixels;
    const layout = type.pixelLayout || {};
    const cols = layout.cols || (n > 1 ? Math.ceil(n / 2) : 1);
    const rows = layout.rows || (n > 1 ? 2 : 1);
    const byColumn = layout.order === 'column';
    const cw = (w * 0.9) / cols;
    const ch = (h * 0.86) / rows;
    this.lensMats = [];
    for (let p = 0; p < n; p++) {
      const mat = new THREE.MeshStandardMaterial({ color: 0x30343a, roughness: 0.25, emissive: 0x000000, emissiveIntensity: EMISSIVE_GAIN });
      const cell = new THREE.Mesh(new THREE.PlaneGeometry(cw * 0.94, ch * 0.92), mat);
      const col = byColumn ? Math.floor(p / rows) : p % cols;
      const row = byColumn ? p % rows : Math.floor(p / cols);
      cell.position.set(-w * 0.45 + cw * (col + 0.5), h * 0.43 - ch * (row + 0.5), d / 2 + 0.001);
      this.root.add(cell);
      this.pickables.push(cell);
      this.lensMats.push(mat);
    }
    this._addSpot(Math.min(w, h) * 0.43, d / 2 + 0.002, type.beamAngle);
    this.root.updateMatrixWorld(true);
    this._finish();
  }

  update(px) {
    for (let p = 0; p < this.lensMats.length; p++) this.lensMats[p].emissive.setRGB(px[p * 3], px[p * 3 + 1], px[p * 3 + 2], THREE.LinearSRGBColorSpace);
    const rgb = averageRgb(px, 0, this.type.pixels);
    const m = driveLight(this.spot, rgb, this.candela);
    this._driveBeam(rgb, m);
  }
}

// ------------------------------------------------------------------ barre à pixels (AX2) : axe = X, LED sur la face +Z
class BarModel extends BaseModel {
  constructor(type) {
    super(type);
    const { length: L, height: H, depth: D } = type.geometry;
    const body = new THREE.Mesh(new THREE.BoxGeometry(L, H, D), bodyMat());
    this.root.add(body);
    this.pickables.push(body);
    const n = type.pixels;
    const pitch = (L * 0.96) / n;
    const lensGeo = new THREE.CircleGeometry(Math.min(pitch, H) * 0.38, 24);
    this.pixelMats = [];
    for (let p = 0; p < n; p++) {
      const mat = new THREE.MeshStandardMaterial({ color: 0x2a2e33, roughness: 0.2, emissive: 0x000000, emissiveIntensity: EMISSIVE_GAIN });
      const lens = new THREE.Mesh(lensGeo, mat);
      lens.position.set(-L * 0.48 + pitch * (p + 0.5), 0, D / 2 + 0.001);
      this.root.add(lens);
      this.pickables.push(lens);
      this.pixelMats.push(mat);
    }
    // Quelques projecteurs répartis le long de la barre (chacun moyenne un groupe de pixels)
    const half = THREE.MathUtils.degToRad((type.beamAngle || 21) / 2);
    const count = Math.min(4, n);
    const cd = (((type.output?.luxAt1m ?? 2000) * LIGHT_SCALE) / count) * 1;
    this.spots = [];
    for (let i = 0; i < count; i++) {
      const x = -L / 2 + (L / count) * (i + 0.5);
      const spot = new THREE.SpotLight(0xffffff, 0, 0, Math.min(Math.PI / 2 - 0.01, half * 1.4), 0.5, 2);
      spot.position.set(x, 0, D / 2 + 0.002);
      const target = new THREE.Object3D();
      target.position.set(x, 0, D / 2 + 1);
      this.root.add(spot, target);
      spot.target = target;
      const beam = createBeam(Math.min(L / count, H) * 0.35, half);
      beam.position.set(x, 0, D / 2 + 0.002);
      this.root.add(beam);
      this.spots.push({ spot, beam, from: Math.floor((n * i) / count), to: Math.floor((n * (i + 1)) / count) });
    }
    this.candela = cd;
    this.root.updateMatrixWorld(true);
    this._finish();
  }

  update(px) {
    for (let i = 0; i < this.pixelMats.length; i++) this.pixelMats[i].emissive.setRGB(px[i * 3], px[i * 3 + 1], px[i * 3 + 2], THREE.LinearSRGBColorSpace);
    for (const s of this.spots) {
      const rgb = averageRgb(px, s.from, s.to);
      const m = driveLight(s.spot, rgb, this.candela);
      const u = s.beam.material.uniforms;
      if (m <= 1e-4) u.uIntensity.value = 0;
      else {
        u.uColor.value.setRGB(rgb[0] / m, rgb[1] / m, rgb[2] / m, THREE.LinearSRGBColorSpace);
        u.uIntensity.value = Math.min(1.5, m) * this.haze * 0.3;
      }
    }
  }
}

// ------------------------------------------------------------------ ampoule (culot en haut)
class BulbModel extends BaseModel {
  constructor(type) {
    super(type);
    const { diameter: D, length: L } = type.geometry;
    const r = D / 2;
    const glassMat = new THREE.MeshStandardMaterial({ color: 0xbfc3c9, roughness: 0.3, emissive: 0x000000, emissiveIntensity: EMISSIVE_GAIN });
    const glass = new THREE.Mesh(new THREE.SphereGeometry(r, 28, 20), glassMat);
    this.root.add(glass);
    const neck = new THREE.Mesh(new THREE.CylinderGeometry(r * 0.55, r * 0.8, L - D, 24), bodyMat());
    neck.position.y = r + (L - D) / 2 - r * 0.3;
    this.root.add(neck);
    this.pickables.push(glass, neck);
    this.glassMat = glassMat;
    this.light = new THREE.PointLight(0xffffff, 0, 0, 2);
    this.root.add(this.light);
    this.candela = this._omniCandela();
    this.root.updateMatrixWorld(true);
    this._finish();
  }

  update(px) {
    const rgb = averageRgb(px, 0, this.type.pixels);
    this.glassMat.emissive.setRGB(rgb[0], rgb[1], rgb[2], THREE.LinearSRGBColorSpace);
    driveLight(this.light, rgb, this.candela);
  }
}

// ------------------------------------------------------------------ boîtier sans lumière (AsteraBox)
class DeviceModel extends BaseModel {
  constructor(type) {
    super(type);
    const { width: w, height: h, depth: d } = type.geometry;
    const body = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), new THREE.MeshStandardMaterial({ color: 0x111316, roughness: 0.45, metalness: 0.6 }));
    this.root.add(body);
    const antMat = new THREE.MeshStandardMaterial({ color: 0x0b0c0e, roughness: 0.8 });
    for (const x of [-w / 2 + 0.011, -w / 2 + 0.066]) {
      const ant = new THREE.Mesh(new THREE.CylinderGeometry(0.0025, 0.0045, 0.054, 12), antMat);
      ant.position.set(x, h / 2 + 0.027, 0);
      this.root.add(ant);
      this.pickables.push(ant);
    }
    // Logo blanc (triangle) sur la face avant
    const logo = new THREE.Mesh(new THREE.CircleGeometry(0.009, 3), new THREE.MeshBasicMaterial({ color: 0xe8eaed }));
    logo.position.set(0, h * 0.22, d / 2 + 0.0005);
    logo.rotation.z = Math.PI / 2;
    this.root.add(logo);
    this.pickables.push(body);
    this.root.updateMatrixWorld(true);
    this._finish();
  }

  update() {}
}

export function createFixtureModel(type, { quality = 2 } = {}) {
  if (type.geometry?.kind === 'box-device') return new DeviceModel(type);
  switch (type.geometry?.kind) {
    case 'tube':
      return new TubeModel(type, quality);
    case 'par':
      return new ParModel(type);
    case 'bar':
      return new BarModel(type);
    case 'brick':
    case 'panel':
      return new BoxModel(type);
    case 'bulb':
    default:
      return new BulbModel(type);
  }
}
