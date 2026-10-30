// Objets de décor dans la vue 3D (personnages, animaux, véhicules, mobilier…) : même interface
// que les modèles du grip (corps déplaçable, sélection, mise à jour des réglages), sans pied ni
// effet optique. Les maillages sont aussi sur le calque 1 (surbrillance).
import * as THREE from 'three';
import { propSize } from '../../../shared/props.js';
import { buildPerson } from './props/person.js';
import { buildAnimal } from './props/animals.js';
import { buildVehicle } from './props/vehicles.js';
import { buildFurniture } from './props/furniture.js';
import { buildSet } from './props/set.js';
import { buildAccessory } from './props/accessories.js';
import { Kit } from './props/kit.js';

/** Construit le modèle d'un objet de décor : { group, materials, aim } (aim : point visé, repère local). */
export function buildProp(type, props = {}) {
  let built = null;
  switch (type.cat) {
    case 'people':
      built = buildPerson(props);
      break;
    case 'animals':
      built = buildAnimal(type.species, props);
      break;
    case 'vehicles':
      built = buildVehicle(type.kind, props);
      break;
    case 'furniture':
      built = buildFurniture(type, props);
      break;
    case 'set':
    case 'outdoor':
      built = buildSet(type, props);
      break;
    case 'props':
    case 'filmset':
      built = buildAccessory(type, props);
      break;
    default:
      break;
  }
  if (!built) {
    // type inconnu de cette version : simple volume aux dimensions du catalogue
    const [w, h, d] = propSize(type, props);
    const k = new Kit();
    k.box(w, h, d, k.mat('box', props.color || '#8a8f96'), [0, h / 2, 0]);
    built = k.build();
  }
  if (!built.aim) {
    const [, h] = propSize(type, props);
    built.aim = new THREE.Vector3(0, h * 0.6, 0);
  }
  return built;
}

export class PropModel {
  constructor(type, props) {
    this.type = type;
    this.root = new THREE.Group();
    this.body = new THREE.Group(); // suit la position / l'orientation de l'objet
    this.root.add(this.body);
    this.optics = null; // le décor ne modifie pas la lumière (il fait seulement de l'ombre)
    this.stands = [];
    this._build(props);
  }

  _build(props) {
    this.props = structuredClone(props || {});
    const built = buildProp(this.type, this.props);
    this.content = built.group;
    this.mats = built.materials;
    this.aimLocal = built.aim.clone();
    this.lensLocal = built.lens ? built.lens.clone() : null; // caméras : centre de l'objectif
    this.body.add(this.content);
    this.pickables = [];
    this.content.traverse((o) => {
      if (!o.isMesh) return;
      o.userData.pickRoot = this.root;
      o.layers.enable(1);
      this.pickables.push(o);
    });
    // boîte de sélection
    this.content.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(this.content);
    const size = box.getSize(new THREE.Vector3()).addScalar(0.04);
    this.selection = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(size.x, size.y, size.z)), new THREE.LineBasicMaterial({ color: 0xffad3b }));
    this.selection.position.copy(box.getCenter(new THREE.Vector3()));
    this.selection.visible = false;
    this.body.add(this.selection);
  }

  place() {}

  setTransform(position, rotation) {
    this.body.position.fromArray(position);
    this.body.rotation.set(rotation[0], rotation[1], rotation[2]);
  }

  /** Nouveaux réglages (pose, couleur, taille…) : reconstruit le modèle. */
  update(props) {
    if (JSON.stringify(props || {}) === JSON.stringify(this.props)) return;
    const selected = this.selection.visible;
    this.dispose();
    this.body.clear();
    this._build(props);
    this.setSelected(selected);
  }

  setSelected(on) {
    this.selection.visible = on;
  }

  /** Caméra : position de l'objectif et direction de visée (monde), ou null. */
  lensWorld() {
    if (!this.lensLocal) return null;
    this.body.updateMatrixWorld(true);
    return {
      position: this.lensLocal.clone().applyMatrix4(this.body.matrixWorld),
      forward: new THREE.Vector3(0, 0, 1).transformDirection(this.body.matrixWorld),
    };
  }

  /** Point visé (tête d'un personnage, centre d'un véhicule…) dans le repère du monde. */
  aimWorld() {
    this.body.updateMatrixWorld(true);
    return this.aimLocal.clone().applyMatrix4(this.body.matrixWorld);
  }

  dispose() {
    this.content?.traverse((o) => o.geometry?.dispose());
    for (const m of this.mats || []) m.dispose();
    this.selection?.geometry.dispose();
    this.selection?.material.dispose();
  }
}
