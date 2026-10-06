// Vue 3D : studio, placement des projecteurs, sélection et rendu.
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { TransformControls } from 'three/addons/controls/TransformControls.js';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js';
import { OutputPass } from 'three/addons/postprocessing/OutputPass.js';
import { ShaderPass } from 'three/addons/postprocessing/ShaderPass.js';
import { CSS2DObject, CSS2DRenderer } from 'three/addons/renderers/CSS2DRenderer.js';
import { effectiveType, getFixtureType } from '/shared/fixtures/index.js';
import { computeOutput } from '/shared/engine/output.js';
import { createFixtureModel } from './models.js';
import { StandModel } from './stands.js';
import { SUPPORTS, resolveSupport } from '/shared/fixtures/supports.js';
import { DRAG_TYPE, GRIP_DRAG_TYPE } from '../ui/library.js';
import { GripModel, buildPlotVolumes, buildUnderlayFloor } from './grip-models.js';
import { GripLighting } from './light-grip.js';
import { getGripType } from '/shared/grip.js';
import { patchLabel } from '../ui/patch.js';

const ACTOR_HEAD = new THREE.Vector3(0, 1.6, 0);
const r3 = (v) => Math.round(v * 1000) / 1000;

/**
 * Passe de sécurité avant le halo : un projecteur très puissant (HMI, 2K…) à quelques centimètres
 * d'une surface (son pied, par exemple) dépasse la précision des tampons d'image ; la valeur devient
 * infinie ou indéfinie et le halo l'étale en taches noires qui clignotent quand la caméra bouge.
 * On remplace les valeurs indéfinies par du noir et on plafonne les autres.
 */
const SanitizeShader = {
  uniforms: { tDiffuse: { value: null }, uMax: { value: 4000 } },
  vertexShader: 'varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }',
  fragmentShader: `uniform sampler2D tDiffuse; uniform float uMax; varying vec2 vUv;
    void main() {
      vec4 c = texture2D(tDiffuse, vUv);
      if (any(isnan(c))) c = vec4(0.0, 0.0, 0.0, 1.0);
      c.rgb = clamp(c.rgb, 0.0, uMax);
      gl_FragColor = c;
    }`,
};
/** Projecteurs qu'on peut orienter vers le personnage (pas les tubes ni les boîtiers). */
const aimable = (type) => !['tube', 'box-device'].includes(type.geometry.kind);

export class Viewport extends EventTarget {
  constructor(container, store, net) {
    super();
    this.container = container;
    this.store = store;
    this.net = net;
    this.models = new Map(); // id -> { model, out: Float32Array, source }
    this.draggingIds = new Set(); // projecteurs déplacés par la poignée pendant un glisser
    this.objects = new Map(); // grip : id -> { model, type }
    this.draggingObjectId = null;
    this.plot3d = new THREE.Group(); // volumes du plan de feu et image au sol
    this.pivot = new THREE.Object3D(); // poignée commune d'une sélection multiple (au centre des projecteurs)
    this._groupOffsets = new Map();
    this.quality = 2;
    this._lastSend = 0;
    this._initRenderer();
    this._initScene();
    this.gripLighting = new GripLighting(this.scene, this.renderer); // ombres, rebonds, diffusion
    this._initControls();
    this._initDrop();
    this._bindStore();
    this._fps = { frames: 0, last: performance.now(), el: document.getElementById('fps') };
    this.onboard = null; // registre des commandes embarquées (branché par app.js)
    this.renderer.setAnimationLoop(() => this._frame());
  }

  // ---------------------------------------------------------------- initialisation
  _initRenderer() {
    const r = new THREE.WebGLRenderer({ antialias: true, powerPreference: 'high-performance' });
    r.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    r.toneMapping = THREE.ACESFilmicToneMapping;
    r.toneMappingExposure = 1;
    r.outputColorSpace = THREE.SRGBColorSpace;
    this.container.appendChild(r.domElement);
    this.renderer = r;

    // Étiquettes (nom, adresse DMX) au-dessus des projecteurs
    this.labelRenderer = new CSS2DRenderer();
    this.labelRenderer.domElement.className = 'fx-labels';
    this.labelRenderer.domElement.style.display = 'none';
    this.container.appendChild(this.labelRenderer.domElement);
    this._labelsOn = false;

    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x040506);
    this.camera = new THREE.PerspectiveCamera(50, 1, 0.05, 200);
    this.camera.position.set(5.5, 3.2, 7.5);

    this.composer = new EffectComposer(r);
    this.composer.addPass(new RenderPass(this.scene, this.camera));
    this.composer.addPass(new ShaderPass(SanitizeShader));
    this.bloom = new UnrealBloomPass(new THREE.Vector2(256, 256), 0.42, 0.3, 0.95);
    this.composer.addPass(this.bloom);
    this.composer.addPass(new OutputPass());

    const resize = () => {
      const w = this.container.clientWidth || 1;
      const h = this.container.clientHeight || 1;
      r.setSize(w, h);
      this.composer.setSize(w, h);
      this.labelRenderer.setSize(w, h);
      this.camera.aspect = w / h;
      this.camera.updateProjectionMatrix();
    };
    new ResizeObserver(resize).observe(this.container);
    resize();
  }

  _initScene() {
    const s = this.scene;
    this.hemi = new THREE.HemisphereLight(0xbfd4ff, 0x1a1a1a, 0.2);
    s.add(this.hemi);

    // Sol
    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(40, 40),
      new THREE.MeshStandardMaterial({ color: 0x45484e, roughness: 0.88, metalness: 0 }),
    );
    floor.rotation.x = -Math.PI / 2;
    floor.userData.ground = true;
    floor.receiveShadow = true;
    const grid = new THREE.GridHelper(40, 80, 0x3c434d, 0x23272d);
    grid.position.y = 0.002;
    grid.material.transparent = true;
    grid.material.opacity = 0.55;
    this.floor = new THREE.Group();
    this.floor.add(floor, grid);
    s.add(this.floor);

    // Cyclo : fond gris avec gorge arrondie
    const cyc = new THREE.Group();
    const cycMat = new THREE.MeshStandardMaterial({ color: 0x9a9da3, roughness: 0.95, side: THREE.DoubleSide });
    const R = 1.2;
    const wall = new THREE.Mesh(new THREE.PlaneGeometry(16, 6), cycMat);
    wall.position.set(0, R + 3, -7);
    const cove = new THREE.Mesh(new THREE.CylinderGeometry(R, R, 16, 24, 1, true, Math.PI, Math.PI / 2), cycMat);
    cove.rotation.z = Math.PI / 2;
    cove.position.set(0, R, -7 + R);
    const cycFloor = new THREE.Mesh(new THREE.PlaneGeometry(16, 3), cycMat);
    cycFloor.rotation.x = -Math.PI / 2;
    cycFloor.position.set(0, 0.003, -7 + R + 1.5);
    cyc.add(wall, cove, cycFloor);
    for (const m of [wall, cove, cycFloor]) m.receiveShadow = true;
    this.cyc = cyc;
    s.add(cyc);

    // Personnage (silhouette simple pour juger l'éclairage d'un sujet)
    this.actor = this._buildActor();
    this.actor.traverse((o) => {
      if (o.isMesh) o.castShadow = o.receiveShadow = true;
    });
    s.add(this.actor);
  }

  _buildActor() {
    const g = new THREE.Group();
    const skin = new THREE.MeshStandardMaterial({ color: 0xb59a86, roughness: 0.6 });
    const cloth = new THREE.MeshStandardMaterial({ color: 0x4a5566, roughness: 0.85 });
    const add = (geo, mat, x, y, z, rx = 0, rz = 0) => {
      const m = new THREE.Mesh(geo, mat);
      m.position.set(x, y, z);
      m.rotation.set(rx, 0, rz);
      g.add(m);
      return m;
    };
    add(new THREE.SphereGeometry(0.11, 24, 18), skin, 0, 1.62, 0);
    add(new THREE.CylinderGeometry(0.05, 0.055, 0.1, 12), skin, 0, 1.48, 0);
    add(new THREE.CapsuleGeometry(0.17, 0.42, 6, 14), cloth, 0, 1.15, 0);
    add(new THREE.CapsuleGeometry(0.075, 0.68, 6, 10), cloth, -0.1, 0.48, 0);
    add(new THREE.CapsuleGeometry(0.075, 0.68, 6, 10), cloth, 0.1, 0.48, 0);
    add(new THREE.CapsuleGeometry(0.05, 0.5, 6, 10), cloth, -0.24, 1.12, 0, 0, 0.12);
    add(new THREE.CapsuleGeometry(0.05, 0.5, 6, 10), cloth, 0.24, 1.12, 0, 0, -0.12);
    return g;
  }

  _initControls() {
    const dom = this.renderer.domElement;
    this.orbit = new OrbitControls(this.camera, dom);
    this.orbit.target.set(0, 1.2, 0);
    this.orbit.enableDamping = true;
    this.orbit.dampingFactor = 0.12;
    this.orbit.update();

    const t = new TransformControls(this.camera, dom);
    t.setSize(0.8);
    t.setTranslationSnap(0.1);
    t.setRotationSnap(THREE.MathUtils.degToRad(15));
    this.transform = t;
    this.scene.add(t.getHelper(), this.pivot, this.plot3d);
    t.addEventListener('dragging-changed', (e) => {
      this.orbit.enabled = !e.value;
      const objId = t.object?.userData.objectId;
      if (objId) {
        // élément de grip
        this.draggingObjectId = e.value ? objId : null;
        if (!e.value) this._sendObjectTransform(objId, true);
        return;
      }
      if (e.value) {
        this.draggingIds = new Set(this.store.selectedIds());
        if (t.object === this.pivot) this._captureGroupOffsets();
      } else {
        this._sendTransform(true);
        this.draggingIds = new Set();
      }
    });
    t.addEventListener('objectChange', () => {
      const objId = t.object?.userData.objectId;
      if (objId) {
        this.objects.get(objId)?.model.place();
        this._sendObjectTransform(objId, false);
        return;
      }
      if (t.object === this.pivot) this._followPivot();
      this._sendTransform(false);
    });

    // Clic (sans glisser) : sélection ; Ctrl / Maj + clic : ajout ou retrait ; clic droit : menu contextuel
    let down = null;
    dom.addEventListener('pointerdown', (e) => {
      down = { x: e.clientX, y: e.clientY, button: e.button };
    });
    dom.addEventListener('pointerup', (e) => {
      if (!down || down.button !== e.button) return;
      const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y);
      const button = down.button;
      down = null;
      if (moved > 4) return;
      if (button === 0) {
        if (this.transform.dragging || this.transform.axis) return;
        const hit = this._pickAny(e);
        if (hit?.kind === 'object') return this.store.selectObject(hit.id);
        const id = hit?.id || null;
        if (e.ctrlKey || e.metaKey || e.shiftKey) {
          if (id) this.store.toggleSelect(id);
        } else {
          this.store.selectObject(null);
          this.store.select(id);
        }
      } else if (button === 2) {
        const hit = this._pickAny(e);
        if (hit?.kind === 'object') {
          this.store.selectObject(hit.id);
          this.dispatchEvent(new CustomEvent('context', { detail: { x: e.clientX, y: e.clientY, objectId: hit.id } }));
          return;
        }
        const id = hit?.id || null;
        if (id && !this.store.isSelected(id)) this.store.select(id);
        this.dispatchEvent(new CustomEvent('context', { detail: { x: e.clientX, y: e.clientY, id } }));
      }
    });
    dom.addEventListener('contextmenu', (e) => e.preventDefault());
    dom.addEventListener('dblclick', (e) => {
      if (!e.ctrlKey && !e.shiftKey && !e.metaKey) this.frameSelected();
    });
  }

  get dragging() {
    return this.draggingIds.size > 0;
  }

  /** Dépôt d'un projecteur glissé depuis la bibliothèque : il est posé là où pointe la souris. */
  _initDrop() {
    const c = this.container;
    const marker = new THREE.Mesh(
      new THREE.RingGeometry(0.16, 0.24, 40),
      new THREE.MeshBasicMaterial({ color: 0xffad3b, transparent: true, opacity: 0.9, depthTest: false }),
    );
    marker.rotation.x = -Math.PI / 2;
    marker.renderOrder = 10;
    marker.visible = false;
    this.scene.add(marker);
    const accepts = (e) => [...(e.dataTransfer?.types || [])].some((t) => t === DRAG_TYPE || t === GRIP_DRAG_TYPE);
    c.addEventListener('dragover', (e) => {
      if (!accepts(e)) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = 'copy';
      const p = this._floorPoint(e);
      marker.visible = !!p;
      if (p) marker.position.set(p.x, 0.012, p.z);
    });
    c.addEventListener('dragleave', (e) => {
      if (!c.contains(e.relatedTarget)) marker.visible = false;
    });
    c.addEventListener('drop', (e) => {
      if (!accepts(e)) return;
      e.preventDefault();
      marker.visible = false;
      const p = this._floorPoint(e);
      const grip = getGripType(e.dataTransfer.getData(GRIP_DRAG_TYPE));
      if (grip && p) return this.dispatchEvent(new CustomEvent('drop-object', { detail: { type: grip, ...this.gripPlacementAt(grip, p.x, p.z) } }));
      const type = getFixtureType(e.dataTransfer.getData(DRAG_TYPE));
      if (type && p) this.dispatchEvent(new CustomEvent('drop-fixture', { detail: { type, ...this.placementAt(type, p.x, p.z) } }));
    });
  }

  /** Point du sol (y = 0) sous le curseur, limité à 15 m du centre du plateau. */
  _floorPoint(e) {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, this.camera);
    const hit = new THREE.Vector3();
    if (!ray.ray.intersectPlane(new THREE.Plane(new THREE.Vector3(0, 1, 0), 0), hit)) return null;
    if (hit.length() > 15) hit.setLength(15);
    return hit;
  }

  /** Mémorise la place de chaque projecteur par rapport à la poignée commune. */
  _captureGroupOffsets() {
    this.pivot.updateMatrixWorld(true);
    const inv = this.pivot.matrixWorld.clone().invert();
    this._groupOffsets = new Map();
    for (const id of this.draggingIds) {
      const entry = this.models.get(id);
      if (!entry) continue;
      entry.model.root.updateMatrixWorld(true);
      this._groupOffsets.set(id, inv.clone().multiply(entry.model.root.matrixWorld));
    }
  }

  /** Déplace / fait pivoter les projecteurs sélectionnés avec la poignée commune. */
  _followPivot() {
    this.pivot.updateMatrixWorld(true);
    const m = new THREE.Matrix4();
    const scale = new THREE.Vector3();
    for (const [id, offset] of this._groupOffsets) {
      const entry = this.models.get(id);
      if (!entry) continue;
      m.multiplyMatrices(this.pivot.matrixWorld, offset);
      m.decompose(entry.model.root.position, entry.model.root.quaternion, scale);
    }
  }

  /** Place la poignée commune au centre des projecteurs sélectionnés. */
  _placePivot(ids) {
    const c = new THREE.Vector3();
    let n = 0;
    for (const id of ids) {
      const entry = this.models.get(id);
      if (!entry) continue;
      c.add(entry.model.root.position);
      n++;
    }
    if (!n) return false;
    this.pivot.position.copy(c.divideScalar(n));
    this.pivot.rotation.set(0, 0, 0);
    this.pivot.updateMatrixWorld(true);
    return true;
  }

  /** Ce qui est sous le curseur : { kind: 'fixture' | 'object', id } ou null. */
  _pickAny(e) {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, this.camera);
    const targets = [];
    for (const { model } of this.models.values()) targets.push(...model.pickables);
    for (const { model } of this.objects.values()) targets.push(...model.pickables);
    const hit = ray.intersectObjects(targets, false)[0];
    if (!hit) return null;
    const root = hit.object.userData.pickRoot;
    if (root.userData.objectId) return { kind: 'object', id: root.userData.objectId };
    return { kind: 'fixture', id: root.userData.fixtureId };
  }

  _pick(e) {
    const rect = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, this.camera);
    const targets = [];
    for (const { model } of this.models.values()) targets.push(...model.pickables);
    const hit = ray.intersectObjects(targets, false)[0];
    return hit ? hit.object.userData.pickRoot.userData.fixtureId : null;
  }

  /** Envoie la position / l'orientation des projecteurs déplacés (par défaut : la sélection). */
  _sendTransform(final, only) {
    const ids = only || (this.dragging ? [...this.draggingIds] : this.store.selectedIds());
    const now = performance.now();
    if (!ids.length || (!final && now - this._lastSend < 70)) return;
    this._lastSend = now;
    const items = [];
    for (const id of ids) {
      const entry = this.models.get(id);
      if (!entry) continue;
      const o = entry.model.root;
      const position = [r3(o.position.x), r3(o.position.y), r3(o.position.z)];
      const rotation = [r3(o.rotation.x), r3(o.rotation.y), r3(o.rotation.z)];
      const f = this.store.fixtures.get(id);
      if (f) {
        f.position = position;
        f.rotation = rotation;
      }
      items.push({ id, changes: { position, rotation } });
    }
    if (items.length === 1) this.net.send({ t: 'fixture.update', id: items[0].id, changes: items[0].changes });
    else if (items.length) this.net.send({ t: 'fixtures.update', items });
    this.dispatchEvent(new CustomEvent('transform', { detail: { ids: items.map((i) => i.id) } }));
  }

  /** Envoie la position / l'orientation d'un élément de grip déplacé à la poignée. */
  _sendObjectTransform(id, final) {
    const entry = this.objects.get(id);
    const now = performance.now();
    if (!entry || (!final && now - this._lastSend < 70)) return;
    this._lastSend = now;
    const b = entry.model.body;
    const position = [r3(b.position.x), r3(b.position.y), r3(b.position.z)];
    const rotation = [r3(b.rotation.x), r3(b.rotation.y), r3(b.rotation.z)];
    const o = this.store.object(id);
    if (o) {
      o.position = position;
      o.rotation = rotation;
    }
    this.net.send({ t: 'object.update', id, changes: { position, rotation } });
    this.dispatchEvent(new CustomEvent('object-transform', { detail: { id } }));
  }

  // ---------------------------------------------------------------- grip et plan de feu en 3D
  _addObject(o) {
    const type = getGripType(o.type);
    if (!type || this.objects.has(o.id)) return;
    const model = new GripModel(type, o.props || {});
    model.root.userData.objectId = o.id;
    model.body.userData.objectId = o.id; // la poignée s'accroche au corps
    this.scene.add(model.root);
    model.setTransform(o.position, o.rotation);
    this.objects.set(o.id, { model, type });
    if (o.id === this.store.selectedObjectId) this._onObjectSelection();
  }

  _updateObject(o) {
    const entry = this.objects.get(o.id);
    if (!entry) return this._addObject(o);
    entry.model.update(o.props || {});
    if (this.draggingObjectId !== o.id) entry.model.setTransform(o.position, o.rotation);
  }

  _removeObject(id) {
    const entry = this.objects.get(id);
    if (!entry) return;
    if (this.transform.object === entry.model.body) this.transform.detach();
    this.scene.remove(entry.model.root);
    entry.model.dispose();
    this.objects.delete(id);
  }

  _onObjectSelection() {
    const sel = this.store.selectedObjectId;
    for (const [id, { model }] of this.objects) model.setSelected(id === sel);
    const entry = sel && this.objects.get(sel);
    if (entry) this.transform.attach(entry.model.body);
    else if (!this.store.selection.size) this.transform.detach();
  }

  /** Volumes des formes du plan qui ont une hauteur, et image de fond posée au sol. */
  _rebuildPlot3d(plot) {
    for (const c of [...this.plot3d.children]) {
      this.plot3d.remove(c);
      c.traverse((o) => {
        o.geometry?.dispose();
        if (o.material?.map) o.material.map.dispose();
        if (o.material && o.material !== this._sharedVolumeMat) o.material.dispose?.();
      });
    }
    if (!plot) return;
    this.plot3d.add(buildPlotVolumes(plot.items || []));
    const floor = buildUnderlayFloor(plot.underlay);
    if (floor) this.plot3d.add(floor);
  }

  /** Élément de grip posé au point (x, z) du sol, tourné vers le personnage. */
  gripPlacementAt(type, x, z) {
    const y = { frame: type.h / 2 + 0.6, flag: 1.6, bounce: (type.h || 1) / 2 + 0.3, box: (type.size?.[1] || 0.1) / 2, bag: (type.size?.[1] || 0.1) / 2, stand: 0 }[type.family] ?? 0;
    const faces = ['frame', 'flag', 'bounce'].includes(type.family);
    const yaw = faces ? Math.atan2(-x, -z) : 0; // face +Z vers le centre du plateau
    return { position: [r3(x), r3(y), r3(z)], rotation: [0, r3(yaw), 0] };
  }

  // ---------------------------------------------------------------- synchronisation avec le store
  _bindStore() {
    const st = this.store;
    st.addEventListener('object-added', (e) => this._addObject(e.detail.object));
    st.addEventListener('object-updated', (e) => this._updateObject(e.detail.object));
    st.addEventListener('object-removed', (e) => this._removeObject(e.detail));
    st.addEventListener('object-selection', () => this._onObjectSelection());
    st.addEventListener('plot', (e) => this._rebuildPlot3d(e.detail));
    st.addEventListener('scene', (e) => this._rebuild(e.detail));
    st.addEventListener('fixture-added', (e) => this._addModel(e.detail.fixture));
    st.addEventListener('fixture-updated', (e) => this._updateModel(e.detail.fixture));
    st.addEventListener('fixture-removed', (e) => this._removeModel(e.detail));
    st.addEventListener('selection', () => this._onSelection());
    st.addEventListener('env', (e) => this.applyEnvironment(e.detail));
  }

  _rebuild(scene) {
    for (const id of [...this.models.keys()]) this._removeModel(id);
    for (const f of scene.fixtures) this._addModel(f);
    for (const id of [...this.objects.keys()]) this._removeObject(id);
    for (const o of scene.objects || []) this._addObject(o);
    this.applyEnvironment(scene.environment);
    this._onSelection();
  }

  _addModel(f) {
    const type = getFixtureType(f.type);
    if (!type || this.models.has(f.id)) return;
    const model = createFixtureModel(effectiveType(type, f), { quality: this.quality });
    model.root.userData.fixtureId = f.id;
    model.setHaze(this.store.scene?.environment?.haze ?? 0.25);
    this.scene.add(model.root);
    const stand = new THREE.Group(); // pied : construit selon le support retenu (_updateStand)
    this.scene.add(stand);
    const label = new CSS2DObject(document.createElement('div'));
    label.element.className = 'fx-label';
    label.visible = this._labelsOn;
    this.scene.add(label);
    const entry = { model, type, stand, label, zoom: f.settings.zoom, out: new Float32Array(type.pixels * 3), source: 'manual' };
    this.models.set(f.id, entry);
    this._updateLabel(f, entry);
    this._applyTransform(f);
  }

  _updateLabel(f, entry) {
    const el = entry.label.element;
    const name = document.createElement('div');
    name.className = 'n';
    name.textContent = f.name;
    const patch = document.createElement('div');
    patch.className = 'p';
    patch.textContent = patchLabel(f);
    el.replaceChildren(name, patch);
    el.classList.toggle('off', f.state?.power === false);
    el.classList.toggle('sel', this.store.isSelected(f.id));
  }

  /**
   * Pied du projecteur, recalé à chaque image sous le projecteur. Son type (pied de 1000, junior,
   * wind-up…) suit le réglage « Support » du projecteur, ou se choisit selon son poids et sa hauteur.
   */
  _updateStand(entry, f) {
    const { stand, model, label } = entry;
    const env = this.store.scene?.environment;
    const box = this._bodyBox(model);
    if (this._labelsOn) label.position.set((box.min.x + box.max.x) / 2, box.max.y + 0.08, (box.min.z + box.max.z) / 2);
    const top = box.min.y;
    const support = resolveSupport(entry.type, f.settings.support, top);
    entry.support = support;
    const look = SUPPORTS[support.id]?.look || null;
    if (look !== entry.standLook) {
      entry.standModel?.dispose();
      stand.clear();
      entry.standModel = look ? new StandModel(look) : null;
      if (entry.standModel) stand.add(entry.standModel.group);
      entry.standLook = look;
    }
    const visible = env?.stands !== false && !!entry.standModel;
    stand.visible = visible;
    if (!visible) return;
    const c = box.getCenter(new THREE.Vector3());
    stand.position.set(c.x, 0, c.z);
    entry.standModel.setTop(top);
  }

  _applyTransform(f) {
    const entry = this.models.get(f.id);
    if (!entry || this.draggingIds.has(f.id)) return;
    entry.model.root.position.fromArray(f.position);
    entry.model.root.rotation.set(f.rotation[0], f.rotation[1], f.rotation[2]);
  }

  _updateModel(f) {
    const entry = this.models.get(f.id);
    if (entry?.type.zoom && entry.zoom !== f.settings.zoom) {
      // zoom mécanique modifié : le faisceau est reconstruit
      this._removeModel(f.id);
      this._addModel(f);
      this._onSelection();
      return;
    }
    if (entry) this._updateLabel(f, entry);
    this._applyTransform(f);
  }

  _removeModel(id) {
    const entry = this.models.get(id);
    if (!entry) return;
    if (this.transform.object === entry.model.root) this.transform.detach();
    this.scene.remove(entry.model.root, entry.stand, entry.label); // retire aussi l'élément HTML de l'étiquette
    entry.model.dispose();
    entry.standModel?.dispose();
    this.models.delete(id);
  }

  _onSelection() {
    for (const [id, { model, label }] of this.models) {
      const on = this.store.isSelected(id);
      model.setSelected(on);
      label.element.classList.toggle('sel', on);
    }
    if (this.dragging) return;
    const ids = this.store.selectedIds();
    const entry = ids.length === 1 && this.models.get(ids[0]);
    if (entry) this.transform.attach(entry.model.root);
    else if (ids.length > 1 && this._placePivot(ids)) this.transform.attach(this.pivot);
    else if (this.store.selectedObjectId) this._onObjectSelection();
    else this.transform.detach();
  }

  /** [id, entrée 3D] des projecteurs sélectionnés. */
  _selectedEntries() {
    return this.store
      .selectedIds()
      .map((id) => [id, this.models.get(id)])
      .filter(([, e]) => e);
  }

  // ---------------------------------------------------------------- commandes
  applyEnvironment(env) {
    if (!env) return;
    this.floor.visible = env.floor !== false;
    this.cyc.visible = env.cyc !== false;
    this.actor.visible = env.actor !== false;
    this.hemi.intensity = (env.ambient ?? 0.04) * 5;
    this.renderer.toneMappingExposure = env.exposure ?? 1;
    for (const { model } of this.models.values()) model.setHaze(env.haze ?? 0.25);
    // étiquettes : noms et / ou adresses DMX
    this._labelsOn = !!(env.labels || env.labelPatch);
    const el = this.labelRenderer.domElement;
    el.style.display = this._labelsOn ? '' : 'none';
    el.classList.toggle('show-names', !!env.labels);
    el.classList.toggle('show-patch', !!env.labelPatch);
    for (const { label } of this.models.values()) label.visible = this._labelsOn;
  }

  setTool(mode) {
    this.transform.setMode(mode);
  }

  setSnap(on) {
    this.transform.setTranslationSnap(on ? 0.1 : null);
    this.transform.setRotationSnap(on ? THREE.MathUtils.degToRad(15) : null);
  }

  setView(name) {
    const target = new THREE.Vector3(0, 1.2, 0);
    const views = {
      persp: [5.5, 3.2, 7.5],
      front: [0, 1.6, 9],
      top: [0, 13, 0.01],
      side: [9, 1.6, 0],
    };
    this.camera.position.fromArray(views[name] || views.persp);
    this.orbit.target.copy(target);
    this.orbit.update();
  }

  /** Boîte englobante du corps du projecteur (sans faisceau ni lumières). */
  _bodyBox(model) {
    model.root.updateMatrixWorld(true);
    const box = new THREE.Box3();
    for (const m of model.pickables) box.expandByObject(m);
    return box;
  }

  dropSelectedToFloor() {
    const entries = this._selectedEntries();
    for (const [, entry] of entries) entry.model.root.position.y -= this._bodyBox(entry.model).min.y;
    this._sendTransform(true, entries.map(([id]) => id));
    this._onSelection();
  }

  frameSelected() {
    const entries = this._selectedEntries();
    if (!entries.length) return;
    const box = new THREE.Box3();
    for (const [, entry] of entries) box.union(this._bodyBox(entry.model));
    const center = box.getCenter(new THREE.Vector3());
    const size = Math.max(0.5, box.getSize(new THREE.Vector3()).length());
    const dir = this.camera.position.clone().sub(this.orbit.target).normalize();
    this.orbit.target.copy(center);
    this.camera.position.copy(center).addScaledVector(dir, size * 1.8 + 0.6);
    this.orbit.update();
  }

  /** Position / orientation proposées pour un nouveau projecteur (bouton « + »). */
  suggestPlacement(type) {
    if (type.geometry.kind === 'box-device') return this.placementAt(type, 0.7, 2.6);
    const n = this.models.size;
    const angle = THREE.MathUtils.degToRad(-60 + ((n * 37) % 120));
    const radius = type.geometry.kind === 'tube' ? 2 : 3;
    return this.placementAt(type, Math.sin(angle) * radius, Math.cos(angle) * radius);
  }

  /** Projecteur posé au point (x, z) du sol : hauteur selon le type, orienté vers le personnage. */
  placementAt(type, x, z) {
    const kind = type.geometry.kind;
    const pos = new THREE.Vector3(x, 0, z);
    if (kind === 'box-device') pos.y = 1.0;
    else if (kind === 'tube') pos.y = type.geometry.length / 2 + 0.5;
    else if (kind === 'bulb') pos.y = 2.6;
    else pos.y = 2.2;
    const obj = new THREE.Object3D();
    obj.position.copy(pos);
    if (!['tube', 'bulb', 'box-device'].includes(kind)) obj.lookAt(ACTOR_HEAD);
    return {
      position: [r3(pos.x), r3(pos.y), r3(pos.z)],
      rotation: [r3(obj.rotation.x), r3(obj.rotation.y), r3(obj.rotation.z)],
    };
  }

  /** Oriente les projecteurs sélectionnés vers la tête du personnage (sauf tubes et boîtiers en sélection multiple). */
  aimSelectedAtActor() {
    const single = this.store.selection.size === 1;
    const entries = this._selectedEntries().filter(([, e]) => single || aimable(e.type));
    for (const [, entry] of entries) entry.model.root.lookAt(ACTOR_HEAD);
    this._sendTransform(true, entries.map(([id]) => id));
    this._onSelection();
  }

  /** Remplace la rotation du projecteur sélectionné. */
  setSelectedRotation(rot) {
    const entry = this.store.selectedId && this.models.get(this.store.selectedId);
    if (!entry) return;
    entry.model.root.rotation.set(rot[0], rot[1], rot[2]);
    this._sendTransform(true, [this.store.selectedId]);
  }

  // ---------------------------------------------------------------- rendu
  _frame() {
    const time = performance.now() / 1000; // même base de temps que les commandes embarquées
    this.orbit.update();
    const ctx = { dmx: this.store.dmx, live: this.store.liveUniverses, time };
    for (const [id, entry] of this.models) {
      const f = this.store.fixtures.get(id);
      if (!f) continue;
      entry.source = computeOutput(f, entry.type, ctx, entry.out, this.onboard ? this.onboard.runtime(id) : (entry.rt ||= {}));
      const ctrl = this.onboard?.peek(id);
      if (ctrl) {
        ctrl.tick(time);
        if (ctrl.overridePixels(entry.out, entry.type.pixels, time)) entry.source = 'onboard';
      }
      entry.model.update(entry.out, time);
      this._updateStand(entry, f);
    }
    this.gripLighting.update(this.models, this.objects);
    if (!this.paused) {
      // plan 2D affiché : les sorties restent calculées (couleurs du plan) mais la 3D n'est pas dessinée
      this.composer.render();
      if (this._labelsOn) this.labelRenderer.render(this.scene, this.camera);
    }
    const fps = this._fps;
    fps.frames++;
    const now = performance.now();
    if (now - fps.last > 1000) {
      if (fps.el) fps.el.textContent = `${Math.round((fps.frames * 1000) / (now - fps.last))} i/s`;
      fps.frames = 0;
      fps.last = now;
    }
  }

  /** Dernière sortie calculée (pour les aperçus de l'interface). */
  outputOf(id) {
    return this.models.get(id) || null;
  }
}
