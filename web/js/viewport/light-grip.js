// Interaction de la lumière avec le grip, recalculée à chaque image :
//  - drapeaux et toiles noires coupent le faisceau visible et font une ombre nette (ombres des spots) ;
//  - tulles et soies laissent passer une partie de la lumière (faisceau et éclairement atténués) ;
//  - réflecteurs (poly, ultrabounce, réflecteur pliable) renvoient la lumière reçue en lumière douce ;
//  - soies et grid cloth réémettent de l'autre côté une lumière douce (source large).
// Les rebonds sont des sources rectangulaires dont la luminance vaut ρ·E/π (E : éclairement reçu).
// Limites : les tubes et ampoules (sources omnidirectionnelles) ne font pas encore d'ombre.
import * as THREE from 'three';
import { RectAreaLightUniformsLib } from 'three/addons/lights/RectAreaLightUniformsLib.js';
import { MAX_BEAM_OCCLUDERS } from './models.js';

const MAX_SHADOW_SPOTS = 6; // chaque ombre coûte un rendu de la scène : on garde les plus utiles
const MAX_AREA_LIGHTS = 6;
const AREA_MIN = 0.002; // luminance en dessous de laquelle un rebond est ignoré

const _tint = new THREE.Color();
const _tc = new THREE.Color();
const _p = new THREE.Vector3();
const _q = new THREE.Vector3();
const _d = new THREE.Vector3();
const _axis = new THREE.Vector3();

/** Lumières et faisceaux d'un modèle de projecteur (mis en cache sur le modèle). */
function lightsOf(model) {
  if (!model._optLights) {
    model._optLights = [];
    model._optBeams = [];
    model.root.traverse((o) => {
      if (o.isSpotLight || o.isPointLight) model._optLights.push(o);
      if (o.userData?.isBeam) model._optBeams.push(o);
    });
  }
  return model._optLights;
}

/** Intersection du segment [a, b] avec le rectangle d'un élément (centre c, demi-axes u, v). */
function segmentHitsRect(a, b, occ) {
  _d.subVectors(b, a);
  const denom = occ.n.dot(_d);
  if (Math.abs(denom) < 1e-9) return false;
  const t = occ.n.dot(_q.subVectors(occ.center, a)) / denom;
  if (t <= 0 || t >= 1) return false;
  _p.copy(a).addScaledVector(_d, t).sub(occ.center);
  return Math.abs(_p.dot(occ.u) / occ.u.lengthSq()) <= 1 && Math.abs(_p.dot(occ.v) / occ.v.lengthSq()) <= 1;
}

export class GripLighting {
  constructor(scene, renderer) {
    RectAreaLightUniformsLib.init();
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    this.scene = scene;
    this.area = [];
    for (let i = 0; i < MAX_AREA_LIGHTS; i++) {
      const l = new THREE.RectAreaLight(0xffffff, 0, 1, 1);
      l.visible = false;
      scene.add(l);
      this.area.push(l);
    }
  }

  /** Surfaces du grip qui agissent sur la lumière, dans le repère du monde. */
  _occluders(objects) {
    const list = [];
    for (const { model } of objects.values()) {
      const o = model.optics;
      if (!o) continue;
      model.body.updateMatrixWorld(true);
      const m = model.body.matrixWorld;
      const center = new THREE.Vector3().setFromMatrixPosition(m);
      const u = new THREE.Vector3().setFromMatrixColumn(m, 0).normalize().multiplyScalar(o.w / 2);
      const v = new THREE.Vector3().setFromMatrixColumn(m, 1).normalize().multiplyScalar(o.h / 2);
      const n = new THREE.Vector3().crossVectors(u, v).normalize();
      list.push({ ...o, center, u, v, n, radius: Math.hypot(o.w, o.h) / 2 });
    }
    return list;
  }

  update(models, objects) {
    const occ = this._occluders(objects);
    const lights = [];
    for (const { model } of models.values()) for (const l of lightsOf(model)) lights.push({ l, model });

    // ---- 1. ombres nettes : les spots qui éclairent une surface opaque
    const solids = occ.filter((o) => o.solid);
    const candidates = [];
    for (const { l } of lights) {
      if (!l.isSpotLight) continue;
      let score = 0;
      if (l.intensity > 0 && solids.length) {
        l.getWorldPosition(_p);
        l.target.getWorldPosition(_axis).sub(_p).normalize();
        for (const o of solids) {
          _d.subVectors(o.center, _p);
          const dist = _d.length();
          if (dist > 20) continue;
          const ang = Math.acos(Math.min(1, _axis.dot(_d) / dist));
          if (ang <= l.angle + Math.atan(o.radius / dist)) score = Math.max(score, l.intensity / (dist * dist));
        }
      }
      if (score > 0) candidates.push({ l, score });
      else if (l.castShadow) l.castShadow = false;
    }
    candidates.sort((a, b) => b.score - a.score);
    candidates.forEach(({ l }, i) => {
      const on = i < MAX_SHADOW_SPOTS;
      if (on && !l.castShadow) {
        l.shadow.mapSize.set(1024, 1024);
        l.shadow.camera.near = 0.05;
        l.shadow.camera.far = 40;
        l.shadow.bias = -0.0004;
        l.shadow.normalBias = 0.02;
      }
      l.castShadow = on;
    });

    // ---- 2. tulles, soies (et surfaces opaques sans ombre calculée) : éclairement atténué
    for (const { l } of lights) l.userData.i0 = l.intensity; // intensité avant le grip (pour les rebonds)
    if (occ.length) {
      for (const { l } of lights) {
        if (!l.isSpotLight || l.intensity <= 0) continue;
        l.getWorldPosition(_p);
        l.target.getWorldPosition(_axis);
        const end = _axis.sub(_p).normalize().multiplyScalar(30).add(_p);
        let k = 1;
        _tint.setRGB(1, 1, 1);
        for (const o of occ) {
          if ((o.solid && l.castShadow) || !segmentHitsRect(_p, end, o)) continue;
          k *= o.t;
          if (o.tint) _tint.multiply(_tc.setRGB(o.tint[0], o.tint[1], o.tint[2])); // gélatine : la lumière prend sa couleur
        }
        l.intensity *= k;
        if (_tint.r < 1 || _tint.g < 1 || _tint.b < 1) l.color.multiply(_tint);
      }
    }

    // ---- 3. faisceaux visibles coupés derrière le grip
    for (const { model } of models.values()) {
      for (const beam of model._optBeams || []) {
        const u = beam.material.uniforms;
        if (!u.uOccCount) continue;
        if (!occ.length || u.uIntensity.value <= 0) {
          u.uOccCount.value = 0;
          continue;
        }
        beam.getWorldPosition(u.uApex.value);
        _axis.set(0, 0, 1).transformDirection(beam.matrixWorld);
        let count = 0;
        for (const o of occ) {
          if (count >= MAX_BEAM_OCCLUDERS) break;
          _d.subVectors(o.center, u.uApex.value);
          const along = _d.dot(_axis);
          if (along <= 0 || along > 8) continue;
          const dist = _d.length();
          if (Math.acos(Math.min(1, along / dist)) > beam.userData.halfAngle + Math.atan(o.radius / dist) + 0.05) continue;
          u.uOccC.value[count].copy(o.center);
          u.uOccU.value[count].copy(o.u);
          u.uOccV.value[count].copy(o.v);
          u.uOccT.value[count] = o.t;
          count++;
        }
        u.uOccCount.value = count;
      }
    }

    // ---- 4. rebonds (réflecteurs) et diffusion (soies) : sources douces rectangulaires
    const emitters = [];
    for (const o of occ) {
      if (!o.reflect && !o.diffuse) continue;
      // éclairement reçu sur chaque face, avec sa couleur
      const faces = [
        { e: 0, c: new THREE.Color(0, 0, 0) },
        { e: 0, c: new THREE.Color(0, 0, 0) },
      ];
      for (const { l } of lights) {
        const intensity = l.userData.i0 ?? l.intensity;
        if (intensity <= 0) continue;
        l.getWorldPosition(_p);
        _d.subVectors(o.center, _p);
        const dist = Math.max(0.3, _d.length());
        _d.divideScalar(dist);
        let f = 1;
        if (l.isSpotLight) {
          l.target.getWorldPosition(_axis).sub(_p).normalize();
          const c = _axis.dot(_d);
          const outer = Math.cos(l.angle);
          const inner = Math.cos(l.angle * (1 - l.penumbra));
          f = THREE.MathUtils.smoothstep(c, outer, inner);
        }
        const cosInc = Math.abs(o.n.dot(_d));
        const e = (intensity * f * cosInc) / (dist * dist);
        if (e <= 0) continue;
        const face = o.n.dot(_d) < 0 ? 0 : 1; // 0 : la source est du côté de la normale
        faces[face].e += e;
        faces[face].c.r += l.color.r * e;
        faces[face].c.g += l.color.g * e;
        faces[face].c.b += l.color.b * e;
      }
      faces.forEach((face, i) => {
        if (face.e <= 0) return;
        const side = i === 0 ? 1 : -1; // côté de la source
        const color = face.c.multiplyScalar(1 / face.e);
        if (o.reflect) emitters.push({ o, dir: side, lum: (o.reflect * face.e) / Math.PI, color });
        if (o.diffuse) emitters.push({ o, dir: -side, lum: (o.diffuse * face.e) / Math.PI, color: o.tint ? color.clone().multiply(new THREE.Color(...o.tint)) : color });
      });
    }
    emitters.sort((a, b) => b.lum - a.lum);
    this.area.forEach((a, i) => {
      const em = emitters[i];
      if (!em || em.lum < AREA_MIN) {
        a.visible = false;
        a.intensity = 0;
        return;
      }
      const { o } = em;
      a.visible = true;
      a.width = o.w;
      a.height = o.h;
      a.color.copy(em.color);
      a.intensity = em.lum;
      _d.copy(o.n).multiplyScalar(em.dir);
      a.position.copy(o.center).addScaledVector(_d, 0.03);
      // la source rectangulaire éclaire vers son −Z : on la tourne vers l'extérieur de la surface
      a.up.copy(o.v).normalize();
      a.lookAt(_q.copy(a.position).add(_d));
    });
  }
}
