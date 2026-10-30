// Vue 3D : studio, placement des projecteurs, sélection et rendu.
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { TransformControls } from 'three/addons/controls/TransformControls.js';
import { EffectComposer } from 'three/addons/postprocessing/EffectComposer.js';
import { RenderPass } from 'three/addons/postprocessing/RenderPass.js';
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
import { PropModel } from './prop-models.js';
import { buildFixtureAccessories } from './fixture-accessories.js';
import { SourceBloomPass } from './source-bloom.js';
import { getObjectType, isProp } from '/shared/objects.js';
import { cameraFov } from '/shared/props.js';
import { patchLine } from '../ui/patch.js';

const DEFAULT_AIM = new THREE.Vector3(0, 1.6, 0); // centre du plateau, hauteur d'un visage
const SNAP_DEG = 5; // crans de rotation de l'aimant (Maj : rotation libre)
const r3 = (v) => Math.round(v * 1000) / 1000;

// Surbrillance : projecteurs, pieds, grip et décor redessinés par-dessus l'image, sans éclairage,
// pour les retrouver dans une scène très sombre. Les maillages concernés sont aussi sur le calque 1.
const HIGHLIGHT_LAYER = 1;
const HighlightShader = {
  uniforms: { color: { value: new THREE.Color(0xffc56b) } },
  vertexShader: `varying vec3 vN; varying vec3 vV;
    void main() {
      vec4 mv = modelViewMatrix * vec4(position, 1.0);
      vN = normalize(normalMatrix * normal);
      vV = normalize(-mv.xyz);
      gl_Position = projectionMatrix * mv;
    }`,
  fragmentShader: `uniform vec3 color; varying vec3 vN; varying vec3 vV;
    void main() {
      float rim = clamp(1.0 - abs(dot(normalize(vN), normalize(vV))), 0.0, 1.0);
      gl_FragColor = vec4(color * (0.2 + 0.7 * rim), 0.05 + 0.55 * pow(rim, 1.6));
    }`,
};

/** Réglages qui changent le modèle d'un projecteur (zoom, accessoires) : il est alors reconstruit. */
const modelKey = (f) => [f.settings.zoom, f.settings.modifier, f.settings.front, f.settings.grid, f.settings.gel].join('|');

/** Met les maillages d'un modèle sur le calque de surbrillance (pas les faisceaux). */
function markHighlight(root) {
  root.traverse((o) => {
    if (o.isMesh && !o.userData.isBeam) o.layers.enable(HIGHLIGHT_LAYER);
  });
}

/**
 * Passe de sécurité : un projecteur très puissant (HMI, 2K…) à quelques centimètres d'une surface
 * (son pied, par exemple) dépasse la précision des tampons d'image ; la valeur devient infinie ou
 * indéfinie et apparaît en taches noires qui clignotent quand la caméra bouge. On remplace les valeurs
 * indéfinies par du noir et on plafonne les autres, puis on applique l'exposition de la caméra
 * (le halo des sources, calculé à part, suit la même exposition : voir source-bloom.js).
 */
const SanitizeShader = {
  uniforms: { tDiffuse: { value: null }, uMax: { value: 4000 }, uExposure: { value: 1 } },
  vertexShader: 'varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }',
  fragmentShader: `uniform sampler2D tDiffuse; uniform float uMax; uniform float uExposure; varying vec2 vUv;
    void main() {
      vec4 c = texture2D(tDiffuse, vUv);
      if (any(isnan(c))) c = vec4(0.0, 0.0, 0.0, 1.0);
      c.rgb = clamp(c.rgb, 0.0, uMax) * uExposure;
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
    this.identifying = new Set(); // projecteurs identifiés par une appli (RDM) : ils clignotent
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

    // image en HDR avec anticrénelage (4 échantillons) : les contours restent nets malgré les effets
    const target = new THREE.WebGLRenderTarget(1, 1, { type: THREE.HalfFloatType, samples: 4 });
    this.composer = new EffectComposer(r, target);
    this.composer.addPass(new RenderPass(this.scene, this.camera));
    this.sanitize = new ShaderPass(SanitizeShader);
    this.composer.addPass(this.sanitize);
    // halo des sources lumineuses, discret et serré : seules les faces émettrices rayonnent (une peau
    // ou un décor surexposés ne « diffusent » pas) ; il apparaît en douceur quand la source approche du blanc
    this.bloom = new SourceBloomPass(this.scene, this.camera, { strength: 0.28, radius: 0.12, threshold: 0.8, knee: 0.6 });
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

    // Surbrillance (désactivée au départ)
    this.highlight = false;
    this.blackout = false; // blackout général (voir setBlackout)
    this.hlMat = new THREE.ShaderMaterial({ ...HighlightShader, transparent: true, depthTest: false, depthWrite: false, blending: THREE.AdditiveBlending });
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
    t.setRotationSnap(THREE.MathUtils.degToRad(SNAP_DEG));
    // Maj enfoncée : déplacement et rotation libres (sans aimant), pour les réglages fins
    this.snapOn = true;
    const free = (on) => {
      this.transform.setTranslationSnap(this.snapOn && !on ? 0.1 : null);
      this.transform.setRotationSnap(this.snapOn && !on ? THREE.MathUtils.degToRad(SNAP_DEG) : null);
    };
    window.addEventListener('keydown', (e) => e.key === 'Shift' && free(true));
    window.addEventListener('keyup', (e) => e.key === 'Shift' && free(false));
    window.addEventListener('blur', () => free(false));
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
      const obj = getObjectType(e.dataTransfer.getData(GRIP_DRAG_TYPE));
      if (obj && p) return this.dispatchEvent(new CustomEvent('drop-object', { detail: { type: obj, ...this.gripPlacementAt(obj, p.x, p.z) } }));
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
    for (const { model } of this.models.values()) targets.push(...model.pickables, ...(model.accPickables || []));
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
    const type = getObjectType(o.type);
    if (!type || this.objects.has(o.id)) return;
    const model = isProp(type) ? new PropModel(type, o.props || {}) : new GripModel(type, o.props || {});
    markHighlight(model.root);
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
    markHighlight(entry.model.root);
    if (this.draggingObjectId !== o.id) entry.model.setTransform(o.position, o.rotation);
  }

  _removeObject(id) {
    const entry = this.objects.get(id);
    if (!entry) return;
    if (this.lookId === id) this.lookThrough(null);
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

  /** Objet posé au point (x, z) du sol : décor dans son orientation habituelle, grip tourné vers le centre. */
  gripPlacementAt(type, x, z) {
    if (isProp(type)) return { position: [r3(x), 0, r3(z)], rotation: [0, r3(type.yaw || 0), 0] };
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

  /** Place libre proposée pour un nouvel objet de décor (bouton « + ») : autour du centre, sans chevauchement. */
  suggestObjectPlacement(type) {
    const taken = [...(this.store.scene?.objects || []), ...(this.store.scene?.fixtures || [])].map((o) => o.position);
    for (let i = 0; i < 80; i++) {
      const a = i * 2.39996; // angle d'or : points bien répartis
      const r = 0.9 * Math.sqrt(i);
      const x = Math.sin(a) * r;
      const z = 1.2 + Math.cos(a) * r * 0.7;
      if (taken.every((p) => Math.hypot(p[0] - x, p[2] - z) > 0.9)) return this.gripPlacementAt(type, x, z);
    }
    return this.gripPlacementAt(type, 0, 2);
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
    const eff = effectiveType(type, f);
    const model = createFixtureModel(eff, { quality: this.quality });
    model.root.userData.fixtureId = f.id;
    model.setHaze(this.store.scene?.environment?.haze ?? 0.25);
    // accessoires (boîte à lumière, dôme, louver, gélatine) : la source passe sur la toile avant
    const acc = buildFixtureAccessories(eff);
    if (acc) {
      model.root.add(acc.group);
      model.accPickables = [];
      acc.group.traverse((o) => {
        if (!o.isMesh) return;
        o.userData.pickRoot = model.root;
        model.accPickables.push(o);
      });
      if (model.spot) {
        model.spot.position.z = acc.lightZ;
        model.spot.target.position.z = acc.lightZ + 1;
        if (acc.soft) model.spot.penumbra = 1;
        model.spot.userData.soft = acc.soft;
      }
    }
    markHighlight(model.root);
    this.scene.add(model.root);
    const stand = new THREE.Group(); // pied : construit selon le support retenu (_updateStand)
    this.scene.add(stand);
    const label = new CSS2DObject(document.createElement('div'));
    label.element.className = 'fx-label';
    label.visible = this._labelsOn;
    this.scene.add(label);
    const entry = { model, type, eff, acc, key: modelKey(f), stand, label, out: new Float32Array(type.pixels * 3), source: 'manual' };
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
    patch.textContent = patchLine(f);
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
      if (entry.standModel) {
        stand.add(entry.standModel.group);
        markHighlight(entry.standModel.group);
      }
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
    if (entry && entry.key !== modelKey(f)) {
      // zoom ou accessoire modifié : le modèle et son faisceau sont reconstruits
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
    this.hemi.intensity = (env.ambient ?? 0.04) * 5;
    this.sanitize.uniforms.uExposure.value = env.exposure ?? 1; // exposition avant le halo (voir SanitizeShader)
    this.bloom.exposure = env.exposure ?? 1;
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
    this.snapOn = on;
    this.transform.setTranslationSnap(on ? 0.1 : null);
    this.transform.setRotationSnap(on ? THREE.MathUtils.degToRad(SNAP_DEG) : null);
  }

  setView(name) {
    if (this.lookId) this.lookThrough(null);
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
    if (!entries.length) return this.store.selectedObjectId ? this.frameObject(this.store.selectedObjectId) : undefined;
    const box = new THREE.Box3();
    for (const [, entry] of entries) box.union(this._bodyBox(entry.model));
    const center = box.getCenter(new THREE.Vector3());
    const size = Math.max(0.5, box.getSize(new THREE.Vector3()).length());
    const dir = this.camera.position.clone().sub(this.orbit.target).normalize();
    this.orbit.target.copy(center);
    this.camera.position.copy(center).addScaledVector(dir, size * 1.8 + 0.6);
    this.orbit.update();
  }

  /** Cadre la vue sur un objet (grip ou décor). */
  frameObject(id) {
    const entry = this.objects.get(id);
    if (!entry) return;
    entry.model.body.updateMatrixWorld(true);
    const box = new THREE.Box3().setFromObject(entry.model.body);
    const center = box.getCenter(new THREE.Vector3());
    const size = Math.max(0.5, box.getSize(new THREE.Vector3()).length());
    const dir = this.camera.position.clone().sub(this.orbit.target).normalize();
    this.orbit.target.copy(center);
    this.camera.position.copy(center).addScaledVector(dir, size * 1.6 + 0.6);
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
    if (!['tube', 'bulb', 'box-device'].includes(kind)) obj.lookAt(this.mainAimPoint());
    return {
      position: [r3(pos.x), r3(pos.y), r3(pos.z)],
      rotation: [r3(obj.rotation.x), r3(obj.rotation.y), r3(obj.rotation.z)],
    };
  }

  /** Personnages, animaux, véhicules… vers lesquels on peut orienter les projecteurs. */
  aimTargets() {
    const list = [];
    for (const [id, { model, type }] of this.objects) {
      if (!type.aimable || !model.aimWorld) continue;
      list.push({ id, name: this.store.object(id)?.name || type.short, point: model.aimWorld(), person: type.model === 'person' });
    }
    return list;
  }

  /** Cible par défaut : le premier personnage, sinon le centre du plateau à hauteur de visage. */
  mainAimPoint() {
    const targets = this.aimTargets();
    return (targets.find((t) => t.person) || targets[0])?.point || DEFAULT_AIM.clone();
  }

  /** Oriente les projecteurs sélectionnés vers une cible (sauf tubes et boîtiers en sélection multiple). */
  aimSelectedAt(targetId = null) {
    const point = (targetId && this.aimTargets().find((t) => t.id === targetId)?.point) || this.mainAimPoint();
    const single = this.store.selection.size === 1;
    const entries = this._selectedEntries().filter(([, e]) => single || aimable(e.type));
    for (const [, entry] of entries) entry.model.root.lookAt(point);
    this._sendTransform(true, entries.map(([id]) => id));
    this._onSelection();
  }

  aimSelectedAtActor() {
    this.aimSelectedAt(null);
  }

  /** Caméras de la scène (objets de décor avec un objectif). */
  cameras() {
    return [...this.objects].filter(([, e]) => e.model.lensWorld?.()).map(([id]) => ({ id, name: this.store.object(id)?.name || 'Caméra' }));
  }

  /**
   * Voir par une caméra de la scène (null : revenir à la vue libre). La vue suit la caméra si on
   * la déplace ; le cadre (16/9) correspond à sa focale et à son capteur.
   */
  lookThrough(id) {
    if (!id) {
      if (!this.lookId) return;
      this.lookId = null;
      const v = this._savedView;
      if (v) {
        this.camera.position.copy(v.position);
        this.orbit.target.copy(v.target);
        this.camera.fov = v.fov;
        this.camera.updateProjectionMatrix();
      }
      this.orbit.enabled = true;
      this.orbit.update();
      this._frameBox?.classList.add('hidden');
      this.dispatchEvent(new CustomEvent('look', { detail: null }));
      return;
    }
    if (!this.objects.get(id)?.model.lensWorld?.()) return;
    if (!this.lookId) this._savedView = { position: this.camera.position.clone(), target: this.orbit.target.clone(), fov: this.camera.fov };
    this.lookId = id;
    this.orbit.enabled = false;
    this._followLens();
    this.dispatchEvent(new CustomEvent('look', { detail: id }));
  }

  _followLens() {
    const entry = this.objects.get(this.lookId);
    const o = this.store.object(this.lookId);
    const lens = entry?.model.lensWorld?.();
    if (!lens || !o) return this.lookThrough(null);
    const fov = cameraFov(o.props);
    const cam = this.camera;
    cam.position.copy(lens.position);
    cam.lookAt(lens.position.clone().add(lens.forward));
    // cadre de la caméra dans la vue : bandes noires selon le rapport largeur / hauteur
    const view = cam.aspect;
    const wide = view >= fov.aspect;
    const vfov = wide ? fov.v : 2 * Math.atan(Math.tan(fov.h / 2) / view);
    const deg = THREE.MathUtils.radToDeg(vfov);
    if (Math.abs(cam.fov - deg) > 1e-3) {
      cam.fov = deg;
      cam.updateProjectionMatrix();
    }
    if (!this._frameBox) {
      this._frameBox = document.createElement('div');
      this._frameBox.className = 'camera-frame hidden';
      this._frameLabel = document.createElement('span');
      this._frameBox.append(this._frameLabel);
      this.container.append(this._frameBox);
    }
    const box = this._frameBox;
    box.classList.remove('hidden');
    box.style.width = wide ? `${(fov.aspect / view) * 100}%` : '100%';
    box.style.height = wide ? '100%' : `${(view / fov.aspect) * 100}%`;
    this._frameLabel.textContent = `${o.name} · ${Math.round(o.props.focal || 35)} mm · ${{ s35: 'Super 35', ff: 'Plein format', m43: 'Micro 4/3' }[o.props.sensor] || 'Super 35'} — Échap pour sortir`;
  }

  /** Surbrillance de tout le matériel et du décor (lisible dans le noir). */
  setHighlight(on) {
    this.highlight = !!on;
  }

  _renderHighlight() {
    const r = this.renderer;
    const scene = this.scene;
    const prev = { autoClear: r.autoClear, shadows: r.shadowMap.autoUpdate, bg: scene.background, override: scene.overrideMaterial };
    r.autoClear = false;
    r.shadowMap.autoUpdate = false;
    scene.background = null;
    scene.overrideMaterial = this.hlMat;
    this.camera.layers.set(HIGHLIGHT_LAYER);
    r.render(scene, this.camera);
    this.camera.layers.set(0);
    scene.overrideMaterial = prev.override;
    scene.background = prev.bg;
    r.shadowMap.autoUpdate = prev.shadows;
    r.autoClear = prev.autoClear;
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
    if (this.lookId) this._followLens();
    else this.orbit.update();
    const ctx = { dmx: this.store.dmx, live: this.store.liveUniverses, time };
    for (const [id, entry] of this.models) {
      const f = this.store.fixtures.get(id);
      if (!f) continue;
      entry.source = computeOutput(f, entry.type, ctx, entry.out, this.onboard ? this.onboard.runtime(id) : (entry.rt ||= {}));
      const tint = entry.eff?.accessories?.tint; // gélatine de couleur sur le projecteur
      if (tint) for (let i = 0; i < entry.out.length; i += 3) (entry.out[i] *= tint[0]), (entry.out[i + 1] *= tint[1]), (entry.out[i + 2] *= tint[2]);
      const ctrl = this.onboard?.peek(id);
      if (ctrl) {
        ctrl.tick(time);
        if (ctrl.overridePixels(entry.out, entry.type.pixels, time)) entry.source = 'onboard';
      }
      if (this.identifying.has(id)) {
        // identification RDM : clignote en blanc, comme un vrai projecteur
        entry.out.fill((time * 2.5) % 1 < 0.5 ? 1 : 0.02);
        entry.source = 'identify';
      }
      if (this.blackout) entry.out.fill(0); // blackout général : rien ne sort, les réglages ne bougent pas
      entry.model.update(entry.out, time);
      entry.acc?.glow(entry.out, entry.type.pixels);
      this._updateStand(entry, f);
    }
    this.gripLighting.update(this.models, this.objects);
    if (!this.paused) {
      // plan 2D affiché : les sorties restent calculées (couleurs du plan) mais la 3D n'est pas dessinée
      this.composer.render();
      if (this.highlight) this._renderHighlight();
      if (this._labelsOn) this.labelRenderer.render(this.scene, this.camera);
    }
    const fps = this._fps;
    fps.frames++;
    const now = performance.now();
    if (now - fps.last > 1000) {
      if (fps.el) fps.el.textContent = `${Math.round((fps.frames * 1000) / (now - fps.last))} ${this.fpsUnit || 'i/s'}`;
      fps.frames = 0;
      fps.last = now;
    }
  }

  /** Blackout général, comme la touche d'une console : tout est noir sans toucher aux réglages. */
  setBlackout(on) {
    this.blackout = !!on;
  }

  /** Identification demandée par une appli (RDM IDENTIFY_DEVICE). */
  identify(id, on) {
    if (on) this.identifying.add(id);
    else this.identifying.delete(id);
  }

  /** Halo des sources lumineuses (bloom) : utile pour l'ambiance, à couper pour une image nette. */
  setBloom(on) {
    this.bloom.enabled = !!on;
  }

  /** Dernière sortie calculée (pour les aperçus de l'interface). */
  outputOf(id) {
    return this.models.get(id) || null;
  }

  /** Vignette de la vue (écran des projets) : image JPEG 16/9 en data URL, ou null. */
  snapshot(width = 480, height = 270) {
    const src = this.renderer.domElement;
    if (!src.width || !src.height) return null;
    const paused = this.paused;
    this.paused = false;
    try {
      this._frame(); // dessinée à l'instant : l'image WebGL se lit juste après
    } finally {
      this.paused = paused;
    }
    const c = document.createElement('canvas');
    c.width = width;
    c.height = height;
    const g = c.getContext('2d');
    g.fillStyle = '#050607';
    g.fillRect(0, 0, width, height);
    // recadrage au centre, sans déformer
    const k = Math.max(width / src.width, height / src.height);
    const w = width / k;
    const hh = height / k;
    g.drawImage(src, (src.width - w) / 2, (src.height - hh) / 2, w, hh, 0, 0, width, height);
    return c.toDataURL('image/jpeg', 0.82);
  }
}
