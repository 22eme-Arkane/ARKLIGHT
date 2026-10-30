// Halo des sources lumineuses : seules les surfaces qui émettent de la lumière (faces des projecteurs,
// toiles de boîte à lumière, écrans…) rayonnent. Une surface éclairée — un visage, une main, le cyclo —
// ne « diffuse » jamais, même surexposée. Pour cela le halo se calcule sur une seconde image où chaque
// source n'a que sa couleur émise et où tout le reste est noir : un personnage devant un projecteur
// masque toujours son halo.
import * as THREE from 'three';
import { FullScreenQuad, Pass } from 'three/addons/postprocessing/Pass.js';
import { UnrealBloomPass } from 'three/addons/postprocessing/UnrealBloomPass.js';

const SIDES = [THREE.FrontSide, THREE.BackSide, THREE.DoubleSide];
const OCCLUDERS = new Map(SIDES.map((side) => [side, new THREE.MeshBasicMaterial({ color: 0x000000, side })]));
const HIDDEN = new THREE.MeshBasicMaterial({ visible: false });

const glowing = (m) => m.emissive?.isColor === true && m.emissiveIntensity > 0 && m.emissive.r + m.emissive.g + m.emissive.b > 0;

export class SourceBloomPass extends Pass {
  /** Mêmes réglages qu'UnrealBloomPass ; `exposure` suit l'exposition de la caméra. */
  constructor(scene, camera, { strength, radius, threshold, knee = 0.01 }) {
    super();
    this.scene = scene;
    this.camera = camera;
    this.exposure = 1;
    this.needsSwap = false;
    this.sources = new THREE.WebGLRenderTarget(1, 1, { type: THREE.HalfFloatType });
    this.bloom = new UnrealBloomPass(new THREE.Vector2(256, 256), strength, radius, threshold);
    this.bloom.highPassUniforms.smoothWidth.value = knee;
    // seuil sur la composante la plus forte et non sur la luminance : une LED rouge ou bleue saturée
    // rayonne comme une blanche de même niveau
    const hp = this.bloom.materialHighPassFilter;
    hp.fragmentShader = hp.fragmentShader.replace('float v = luminance( texel.xyz );', 'float v = max( texel.r, max( texel.g, texel.b ) );');
    this.blend = new FullScreenQuad(this.bloom.blendMaterial); // halo ajouté à l'image (mélange additif)
    this._glow = new WeakMap(); // matériau émissif -> matériau de la source seule
    this._swap = []; // objet, matériau d'origine, objet, matériau d'origine…
    this._peak = 0; // composante la plus forte émise par une source (image en cours)
    this._clear = new THREE.Color();
  }

  setSize(width, height) {
    this.sources.setSize(width, height);
    this.bloom.setSize(width, height);
  }

  render(renderer, writeBuffer, readBuffer) {
    if (!this._renderSources(renderer)) return; // aucune source assez forte pour rayonner : rien à ajouter
    this.bloom.render(renderer, null, this.sources, 0, false); // halo seul dans bloom.renderTargetsHorizontal[0]
    const autoClear = renderer.autoClear;
    renderer.autoClear = false;
    this.bloom.copyUniforms.tDiffuse.value = this.bloom.renderTargetsHorizontal[0].texture;
    renderer.setRenderTarget(this.renderToScreen ? null : readBuffer);
    this.blend.render(renderer);
    renderer.autoClear = autoClear;
  }

  /** Matériau d'un objet pour l'image des sources : couleur émise, cache noir ou rien. */
  _sourceMaterial(o, m) {
    if (Array.isArray(m)) {
      let list = this._glow.get(m);
      if (!list) this._glow.set(m, (list = []));
      for (let i = 0; i < m.length; i++) list[i] = this._sourceMaterial(o, m[i]);
      return list;
    }
    if (!o.isMesh || o.userData.isBeam || !m.visible) return HIDDEN; // traits, faisceaux, aides
    if (glowing(m)) {
      let g = this._glow.get(m);
      if (!g) this._glow.set(m, (g = new THREE.MeshBasicMaterial()));
      g.side = m.side;
      g.color.copy(m.emissive).multiplyScalar(m.emissiveIntensity * this.exposure);
      this._peak = Math.max(this._peak, g.color.r, g.color.g, g.color.b);
      return g;
    }
    // transparents, fils de fer, poignées dessinées par-dessus : ne cachent rien
    if (m.transparent || m.wireframe || !m.depthWrite || !m.depthTest || m.alphaTest > 0) return HIDDEN;
    return OCCLUDERS.get(m.side) || OCCLUDERS.get(THREE.DoubleSide);
  }

  /** Dessine l'image des sources ; false (sans rien dessiner) si aucune n'atteint le seuil du halo. */
  _renderSources(renderer) {
    const { scene, _swap: swap } = this;
    const prev = {
      background: scene.background,
      autoUpdate: scene.matrixWorldAutoUpdate,
      shadows: renderer.shadowMap.autoUpdate,
      autoClear: renderer.autoClear,
      alpha: renderer.getClearAlpha(),
    };
    renderer.getClearColor(this._clear);
    try {
      this._peak = 0;
      scene.traverseVisible((o) => {
        if (!o.material) return;
        swap.push(o, o.material);
        o.material = this._sourceMaterial(o, o.material);
      });
      if (this._peak <= this.bloom.threshold) return false;
      scene.background = null;
      scene.matrixWorldAutoUpdate = false; // positions déjà à jour (image principale de la même trame)
      renderer.shadowMap.autoUpdate = false;
      renderer.autoClear = true;
      renderer.setClearColor(0x000000, 1);
      renderer.setRenderTarget(this.sources);
      renderer.render(scene, this.camera);
      return true;
    } finally {
      renderer.setClearColor(this._clear, prev.alpha);
      renderer.autoClear = prev.autoClear;
      renderer.shadowMap.autoUpdate = prev.shadows;
      scene.matrixWorldAutoUpdate = prev.autoUpdate;
      scene.background = prev.background;
      for (let i = 0; i < swap.length; i += 2) swap[i].material = swap[i + 1];
      swap.length = 0;
    }
  }

  dispose() {
    this.sources.dispose();
    this.bloom.dispose();
    this.blend.dispose();
  }
}
