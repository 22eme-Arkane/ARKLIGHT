// Modèles 3D du grip (cadres, drapeaux, réflecteurs, cubes, sacs, pieds seuls) et du plan de feu
// dans la vue 3D (volumes extrudés, image de fond posée au sol).
// Effet sur la lumière (voir light-grip.js) : chaque surface décrit son comportement optique.
import * as THREE from 'three';
import { CLOTHS, gripStands } from '/shared/grip.js';
import { SUPPORTS } from '/shared/fixtures/supports.js';
import { filterOptics, getFilter } from '/shared/filters.js';
import { StandModel } from './stands.js';

/**
 * Comportement optique des toiles et surfaces (valeurs approchées) :
 *   t        part du faisceau qui passe en ligne droite (0 : coupé net)
 *   reflect  part renvoyée côté source (réflecteurs)
 *   diffuse  part réémise en lumière douce de l'autre côté (soie, grid cloth)
 *   solid    surface opaque : elle fait une ombre nette
 */
function clothOptics(id) {
  const c = CLOTHS[id];
  return c && c.opacity > 0 ? { t: c.t, reflect: c.reflect, diffuse: c.diffuse, solid: c.solid } : null;
}

const frameMat = () => new THREE.MeshStandardMaterial({ color: 0x9ea3a9, metalness: 0.8, roughness: 0.35 });

function clothMat(id) {
  const f = !CLOTHS[id] && getFilter(id);
  if (f) {
    // filtre du nuancier tendu sur le cadre : diffusion blanche translucide ou gélatine colorée
    const diffusion = f.group === 'diffusion';
    return new THREE.MeshStandardMaterial({ color: f.color, roughness: diffusion ? 0.9 : 0.25, transparent: true, opacity: diffusion ? 0.55 + f.scatter * 0.35 : 0.45, side: THREE.DoubleSide, depthWrite: false });
  }
  const c = CLOTHS[id] || CLOTHS.silk;
  return new THREE.MeshStandardMaterial({
    color: c.color,
    roughness: 0.9,
    transparent: c.opacity < 1,
    opacity: c.opacity,
    side: THREE.DoubleSide,
    depthWrite: c.opacity >= 1,
  });
}

/** Cadre rectangulaire en tube (plan local XY, face vers +Z). */
function tubeFrame(w, h, t, mat) {
  const g = new THREE.Group();
  const parts = [
    [w + t, t, 0, h / 2],
    [w + t, t, 0, -h / 2],
    [t, h, -w / 2, 0],
    [t, h, w / 2, 0],
  ];
  for (const [sx, sy, x, y] of parts) {
    const m = new THREE.Mesh(new THREE.BoxGeometry(sx, sy, t), mat);
    m.position.set(x, y, 0);
    g.add(m);
  }
  return g;
}

export class GripModel {
  constructor(type, props) {
    this.type = type;
    this.root = new THREE.Group();
    this.body = new THREE.Group(); // suit la position / l'orientation de l'objet
    this.supports = new THREE.Group(); // pieds : toujours verticaux, recalés sous l'objet
    this.root.add(this.body);
    this.pickables = [];
    this.stands = [];
    this.props = {};
    this._build(props);
  }

  _build(props) {
    this.props = { ...props };
    const t = this.type;
    const mats = [];
    const add = (mesh) => {
      this.body.add(mesh);
      mesh.traverse((o) => o.isMesh && this.pickables.push(o));
      return mesh;
    };
    if (t.family === 'frame' || t.family === 'flag') {
      const fm = t.family === 'frame' ? frameMat() : new THREE.MeshStandardMaterial({ color: 0x2a2c30, roughness: 0.6, metalness: 0.4 });
      mats.push(fm);
      add(tubeFrame(t.w, t.h, t.family === 'frame' ? 0.03 : 0.012, fm));
      if (props.cloth !== 'empty') {
        const cm = clothMat(props.cloth);
        mats.push(cm);
        add(new THREE.Mesh(new THREE.PlaneGeometry(t.w, t.h), cm));
      }
    } else if (t.family === 'bounce') {
      const m = new THREE.MeshStandardMaterial({ color: t.color, roughness: 0.85, side: THREE.DoubleSide });
      mats.push(m);
      add(t.round ? new THREE.Mesh(new THREE.CircleGeometry(t.w / 2, 40), m) : new THREE.Mesh(new THREE.BoxGeometry(t.w, t.h, 0.05), m));
    } else if (t.family === 'box') {
      const m = new THREE.MeshStandardMaterial({ color: 0x8a6a45, roughness: 0.8 });
      mats.push(m);
      const [w, h, d] = t.size;
      add(new THREE.Mesh(new THREE.BoxGeometry(w, h, d), m));
    } else if (t.family === 'bag') {
      const m = new THREE.MeshStandardMaterial({ color: 0x6b4a2a, roughness: 0.95 });
      mats.push(m);
      const [w, h, d] = t.size;
      const bag = new THREE.Mesh(new THREE.CapsuleGeometry(h / 2, w - h, 4, 10), m);
      bag.rotation.z = Math.PI / 2;
      bag.scale.z = d / h;
      add(bag);
    } else if (t.family === 'stand') {
      const s = new StandModel(t.standLook);
      s.setTop(props.height || t.height || 2);
      this.body.add(s.group);
      s.group.traverse((o) => o.isMesh && this.pickables.push(o));
      this.stands.push(s);
    }
    this.mats = mats;
    // Effet sur la lumière : surfaces planes (cadres, drapeaux, réflecteurs)
    const filter = !CLOTHS[props.cloth] && getFilter(props.cloth);
    const optics = t.family === 'bounce' ? { t: 0, reflect: t.round ? 0.85 : 0.8, solid: true } : t.family === 'frame' || t.family === 'flag' ? (filter ? filterOptics(filter) : clothOptics(props.cloth)) : null;
    this.optics = optics ? { w: t.w, h: t.h, ...optics } : null;
    for (const m of this.pickables) {
      m.receiveShadow = true;
      // ombre nette : toile opaque, réflecteurs, cubes et sacs (pas les tubes du cadre)
      const plane = m.geometry?.type === 'PlaneGeometry' || m.geometry?.type === 'CircleGeometry';
      m.castShadow = ((this.optics?.solid && (plane || t.family === 'bounce')) || t.family === 'box' || t.family === 'bag') && t.family !== 'stand';
    }
    // Pieds sous les cadres, drapeaux et réflecteurs (recalés dans place()) : ceux choisis, sinon ceux du type
    const holder = gripStands(t, props);
    if (holder) {
      const look = SUPPORTS[holder.id]?.look || 'combo';
      for (let i = 0; i < holder.count; i++) {
        const s = new StandModel(look);
        this.supports.add(s.group);
        this.stands.push(s);
      }
      if (look === 'cstand') {
        this.arm = new THREE.Mesh(new THREE.CylinderGeometry(0.008, 0.008, 1, 8), frameMat());
        this.supports.add(this.arm);
      }
    }
    for (const m of this.pickables) m.userData.pickRoot = this.root;
    this.root.add(this.supports);
    // boîte de sélection
    const box = new THREE.Box3();
    for (const m of this.pickables) {
      m.updateMatrixWorld(true);
      box.expandByObject(m);
    }
    const size = box.getSize(new THREE.Vector3()).addScalar(0.04);
    this.selection = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(size.x, size.y, size.z)), new THREE.LineBasicMaterial({ color: 0xffad3b }));
    this.selection.position.copy(box.getCenter(new THREE.Vector3()));
    this.selection.visible = false;
    this.body.add(this.selection);
  }

  /** Recale les pieds (toujours verticaux) sous l'objet. */
  place() {
    const t = this.type;
    if (!this.stands.length || t.family === 'stand') return;
    this.body.updateMatrixWorld(true);
    const pts =
      t.family === 'frame'
        ? [new THREE.Vector3(-t.w / 2, 0, 0), new THREE.Vector3(t.w / 2, 0, 0)] // oreilles au milieu des côtés
        : [new THREE.Vector3(-(t.w || 1) / 2, 0, 0)];
    const inv = new THREE.Matrix4().copy(this.root.matrixWorld).invert();
    pts.forEach((p, i) => {
      const s = this.stands[i];
      if (!s) return;
      const world = p.clone().applyMatrix4(this.body.matrixWorld);
      const local = world.clone().applyMatrix4(inv);
      const away = t.family === 'frame' ? 0 : 0.55; // C-stand décalé, relié par le bras
      const dir = new THREE.Vector3(-1, 0, 0).applyQuaternion(this.body.quaternion).setY(0).normalize();
      s.group.position.set(local.x + dir.x * away, 0, local.z + dir.z * away);
      s.setTop(Math.max(0.3, world.y));
      if (this.arm && i === 0) {
        const a = new THREE.Vector3(s.group.position.x, world.y, s.group.position.z);
        const b = local.clone().setY(world.y);
        const d = new THREE.Vector3().subVectors(b, a);
        this.arm.position.copy(a).addScaledVector(d, 0.5);
        this.arm.scale.set(1, Math.max(0.01, d.length()), 1);
        this.arm.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), d.normalize());
      }
    });
  }

  setTransform(position, rotation) {
    this.body.position.fromArray(position);
    this.body.rotation.set(rotation[0], rotation[1], rotation[2]);
    this.place();
  }

  /** Change de toile ou de hauteur : reconstruit le modèle. */
  update(props) {
    if (props.cloth === this.props.cloth && props.height === this.props.height && props.stand === this.props.stand) return;
    const pos = this.body.position.toArray();
    const rot = [this.body.rotation.x, this.body.rotation.y, this.body.rotation.z];
    const selected = this.selection.visible;
    this.dispose();
    this.body.clear();
    this.supports.clear();
    this.pickables = [];
    this.stands = [];
    this.arm = null;
    this._build(props);
    this.setTransform(pos, rot);
    this.setSelected(selected);
  }

  setSelected(on) {
    this.selection.visible = on;
  }

  dispose() {
    for (const s of this.stands) s.dispose();
    this.body.traverse((o) => o.geometry?.dispose());
    this.supports.traverse((o) => o.geometry?.dispose());
    for (const m of this.mats || []) m.dispose();
    this.arm?.material.dispose();
    this.selection?.material.dispose();
  }
}

// ------------------------------------------------------------------ plan de feu dans la 3D
const volumeMat = new THREE.MeshStandardMaterial({ color: 0x8d9299, roughness: 0.9, transparent: true, opacity: 0.88 });

/** Volumes des formes du plan qui ont une hauteur (maison, kiosque, mur…). */
export function buildPlotVolumes(items) {
  const g = new THREE.Group();
  for (const it of items) {
    if (!(it.height > 0) || !['rect', 'ellipse', 'polygon', 'freehand'].includes(it.kind)) continue;
    let pts = it.points;
    if (it.kind === 'rect' && pts.length >= 2) {
      const [[x0, z0], [x1, z1]] = pts;
      pts = [[x0, z0], [x1, z0], [x1, z1], [x0, z1]];
    } else if (it.kind === 'ellipse' && pts.length >= 2) {
      const [[x0, z0], [x1, z1]] = pts;
      const cx = (x0 + x1) / 2;
      const cz = (z0 + z1) / 2;
      pts = Array.from({ length: 40 }, (_, i) => [cx + (Math.abs(x1 - x0) / 2) * Math.cos((i / 40) * Math.PI * 2), cz + (Math.abs(z1 - z0) / 2) * Math.sin((i / 40) * Math.PI * 2)]);
    }
    if (pts.length < 3) continue;
    // forme dans le plan (x, −z), extrudée puis couchée : l'extrusion monte selon +Y
    const shape = new THREE.Shape(pts.map(([x, z]) => new THREE.Vector2(x, -z)));
    const geo = new THREE.ExtrudeGeometry(shape, { depth: it.height, bevelEnabled: false });
    geo.rotateX(-Math.PI / 2);
    const mesh = new THREE.Mesh(geo, volumeMat);
    mesh.userData.plotId = it.id;
    g.add(mesh);
  }
  return g;
}

const loader = new THREE.TextureLoader();
/** Image de fond du plan posée au sol, à l'échelle. */
export function buildUnderlayFloor(underlay) {
  if (!underlay?.floor3d) return null;
  const w = underlay.width;
  const h = w / (underlay.aspect || 1);
  const tex = loader.load(underlay.url);
  tex.colorSpace = THREE.SRGBColorSpace;
  const mesh = new THREE.Mesh(new THREE.PlaneGeometry(w, h), new THREE.MeshBasicMaterial({ map: tex, transparent: true, opacity: underlay.opacity ?? 0.8, depthWrite: false }));
  mesh.rotation.set(-Math.PI / 2, 0, -((underlay.rotation || 0) * Math.PI) / 180, 'YXZ');
  mesh.position.set(underlay.x, 0.004, underlay.z);
  return mesh;
}
