// Personnage articulé (mannequin) : squelette simple calculé selon la pose, habillé de capsules.
// Face vers +Z, pieds au sol. Proportions d'après les canons anatomiques courants (en fraction de
// la taille H). La position de la tête sert de cible pour « Viser ».
import * as THREE from 'three';
import { Kit } from './kit.js';

const V = (x, y, z) => new THREE.Vector3(x, y, z);

// Silhouettes : longueurs et rayons en fraction de la taille H
const BUILD = {
  man: { ankle: 0.045, shin: 0.243, thigh: 0.245, hip: 0.058, torso: 0.3, neck: 0.04, head: 0.062, shoulder: 0.118, upper: 0.172, fore: 0.148, hand: 0.075, waist: 0.083, chest: 0.1, hips: 0.093, limb: 1 },
  woman: { ankle: 0.045, shin: 0.245, thigh: 0.248, hip: 0.064, torso: 0.295, neck: 0.042, head: 0.063, shoulder: 0.106, upper: 0.17, fore: 0.145, hand: 0.072, waist: 0.074, chest: 0.093, hips: 0.1, limb: 0.9 },
  child: { ankle: 0.04, shin: 0.225, thigh: 0.225, hip: 0.06, torso: 0.3, neck: 0.04, head: 0.078, shoulder: 0.11, upper: 0.165, fore: 0.14, hand: 0.075, waist: 0.09, chest: 0.098, hips: 0.095, limb: 1.05 },
};

// Poses : flexion de hanche, de genou, d'épaule (vers l'avant), abduction (bras écartés), coude,
// croisement des avant-bras (vers l'intérieur), inclinaison du buste. Indices [gauche, droite].
const POSES = {
  standing: { hip: [0, 0], knee: [0.03, 0.03], sh: [0.06, 0.06], abd: [0.1, 0.1], elbow: [0.18, 0.18] },
  walking: { hip: [0.36, -0.3], knee: [0.08, 0.5], sh: [-0.38, 0.38], abd: [0.08, 0.08], elbow: [0.25, 0.5], lean: 0.05 },
  sitting: { hip: [1.5, 1.5], knee: [1.5, 1.5], sh: [0.5, 0.5], abd: [0.05, 0.05], elbow: [0.85, 0.85], lean: 0.05, spread: 0.07 },
  lying: { hip: [0, 0], knee: [0.04, 0.04], sh: [0.04, 0.04], abd: [0.2, 0.2], elbow: [0.1, 0.1], lying: true },
  crossed: { hip: [0, 0], knee: [0.03, 0.03], sh: [0.3, 0.3], abd: [0.02, 0.02], elbow: [1.25, 1.25], cross: [1.6, 1.6] },
  phone: { hip: [0, 0], knee: [0.03, 0.03], sh: [0.06, 0.95], abd: [0.1, 0.35], elbow: [0.18, 2.35], cross: [0, 0.55], phone: true },
  pointing: { hip: [0, 0], knee: [0.03, 0.03], sh: [0.06, 1.45], abd: [0.1, 0.1], elbow: [0.18, 0.05] },
  handsup: { hip: [0, 0], knee: [0.03, 0.03], sh: [-0.1, -0.1], abd: [2.75, 2.75], elbow: [0.12, 0.12] },
};

/** Squelette (positions des articulations) dans le repère du personnage debout, face +Z. */
function skeleton(H, B, pose) {
  const P = POSES[pose] || POSES.standing;
  const L = (k) => B[k] * H;
  const spread = P.spread || 0.03;
  const legs = [1, -1].map((s, i) => {
    const a = P.hip[i];
    const th = a - P.knee[i];
    return {
      s,
      thighDir: V(s * Math.sin(spread), -Math.cos(a) * Math.cos(spread), Math.sin(a) * Math.cos(spread)),
      shinDir: V(s * Math.sin(spread) * 0.5, -Math.cos(th), Math.sin(th)),
      footDir: V(0, Math.sin(th), Math.cos(th)),
    };
  });
  const placeLegs = (pelvisY) => {
    for (const l of legs) {
      l.hip = V(l.s * L('hip'), pelvisY, 0);
      l.knee = l.hip.clone().addScaledVector(l.thighDir, L('thigh'));
      l.ankle = l.knee.clone().addScaledVector(l.shinDir, L('shin'));
      l.toe = l.ankle.clone().addScaledVector(l.footDir, 0.13 * H);
    }
  };
  // le point le plus bas des pieds (talon ou pointe) touche le sol
  placeLegs(0);
  const sole = Math.min(...legs.map((l) => Math.min(l.ankle.y, l.toe.y))) - 0.03 * H;
  const pelvis = V(0, -sole, 0);
  placeLegs(pelvis.y);
  const lean = P.lean || 0;
  const up = V(0, Math.cos(lean), Math.sin(lean));
  const neckBase = pelvis.clone().addScaledVector(up, L('torso'));
  const shoulderY = neckBase.clone().addScaledVector(up, -0.035 * H);
  const headC = neckBase.clone().addScaledVector(up, L('neck') + L('head') * 1.05);
  const arms = [1, -1].map((s, i) => {
    const al = P.abd[i];
    const f = P.sh[i];
    const e = P.elbow[i];
    const upper = V(s * Math.sin(al), -Math.cos(al) * Math.cos(f), Math.cos(al) * Math.sin(f)).normalize();
    const fore = V(s * Math.sin(al) - s * (P.cross?.[i] || 0), -Math.cos(al) * Math.cos(f + e), Math.cos(al) * Math.sin(f + e)).normalize();
    const shoulder = shoulderY.clone().add(V(s * L('shoulder'), 0, 0));
    const elbow = shoulder.clone().addScaledVector(upper, L('upper'));
    const wrist = elbow.clone().addScaledVector(fore, L('fore'));
    const hand = wrist.clone().addScaledVector(fore, L('hand'));
    return { s, shoulder, elbow, wrist, hand, fore };
  });
  return { pelvis, up, neckBase, headC, legs, arms, lying: !!P.lying, phone: !!P.phone };
}

/** Modèle 3D d'un personnage. props : pose, height (cm), build, skin, hair, hairColor, top, pants. */
export function buildPerson(props = {}) {
  const H = Math.max(0.6, (props.height || 175) / 100);
  const B = BUILD[props.build] || BUILD.man;
  const sk = skeleton(H, B, props.pose);
  const k = new Kit();
  const skin = k.mat('skin', props.skin || '#e0aa83', { rough: 0.55 });
  const top = k.mat('top', props.top || '#3d4f6b', { rough: 0.85 });
  const pants = k.mat('pants', props.pants || '#2a2e35', { rough: 0.85 });
  const shoes = k.mat('shoes', '#1c1c1f', { rough: 0.5 });
  const hair = k.mat('hair', props.hairColor || '#2b1d14', { rough: 0.95 });
  const L = (key) => B[key] * H;
  const lr = B.limb * H;

  // buste : bassin (bas) et torse (haut), profils de révolution aplatis
  const q = new THREE.Quaternion().setFromUnitVectors(V(0, 1, 0), sk.up);
  const lower = [
    [0.001, -0.03 * H],
    [L('hips') * 0.75, -0.025 * H],
    [L('hips'), 0.02 * H],
    [L('hips') * 0.98, 0.07 * H],
    [L('waist') * 1.02, 0.12 * H],
  ];
  const upper = [
    [L('waist') * 1.02, 0.11 * H],
    [L('waist'), 0.15 * H],
    [L('chest') * 0.97, 0.21 * H],
    [L('chest'), 0.245 * H],
    [L('chest') * 0.95, L('torso') - 0.02 * H],
    [L('chest') * 0.55, L('torso') - 0.004 * H],
    [0.028 * H, L('torso')],
  ];
  k.lathe(lower, pants, sk.pelvis, { q, s: [1, 1, 0.66], seg: 18 });
  k.lathe(upper, top, sk.pelvis, { q, s: [1, 1, 0.62], seg: 18 });

  // cou, tête, nez (sens du regard), cheveux
  const headR = L('head');
  k.rod(sk.neckBase.clone().addScaledVector(sk.up, -0.01 * H), sk.headC.clone().addScaledVector(sk.up, -headR * 0.6), 0.026 * H, skin);
  k.ball(headR, skin, sk.headC, { q, s: [0.9, 1.12, 1.0], seg: 20 });
  k.ball(0.011 * H, skin, sk.headC.clone().add(V(0, -0.012 * H, headR * 0.96).applyQuaternion(q)));
  if (props.hair !== 'bald') {
    const cap = new THREE.Quaternion().setFromEuler(new THREE.Euler(-0.45, 0, 0)).premultiply(q);
    k.ball(headR * 1.06, hair, sk.headC.clone().add(V(0, headR * 0.06, -headR * 0.05).applyQuaternion(q)), { q: cap, s: [0.92, 1.1, 1.02], seg: 20, theta: Math.PI * 0.55 });
    if (props.hair === 'long') k.rbox(headR * 1.75, 0.2 * H, headR * 0.7, headR * 0.3, hair, sk.headC.clone().add(V(0, -0.075 * H, -headR * 0.62).applyQuaternion(q)), { q });
  }

  // bras : épaules, bras (manches), mains
  for (const a of sk.arms) {
    k.ball(0.034 * lr, top, a.shoulder);
    k.capsule(a.shoulder, a.elbow, 0.027 * lr, top);
    k.capsule(a.elbow, a.wrist, 0.022 * lr, top);
    k.capsule(a.wrist, a.hand, 0.018 * lr, skin, { s: [1.15, 1, 0.65] });
  }
  if (sk.phone) {
    const a = sk.arms[1];
    k.box(0.035 * H, 0.08 * H, 0.008 * H, shoes, a.hand.clone().addScaledVector(a.fore, -0.01 * H), { q: new THREE.Quaternion().setFromUnitVectors(V(0, 1, 0), a.fore) });
  }

  // jambes et chaussures
  for (const l of sk.legs) {
    k.capsule(l.hip, l.knee, 0.037 * lr, pants);
    k.capsule(l.knee, l.ankle, 0.028 * lr, pants);
    const heel = l.ankle.clone().addScaledVector(l.footDir, -0.025 * H).add(V(0, -0.012 * H, 0));
    k.capsule(heel, l.toe.clone().add(V(0, -0.012 * H, 0)), 0.024 * H, shoes, { s: [1.05, 1, 0.75] });
  }

  const { group, materials } = k.build();
  let aim = sk.headC.clone();
  if (sk.lying) {
    // allongé sur le dos, la tête vers -Z, centré sur l'objet
    const root = new THREE.Group();
    group.rotation.x = -Math.PI / 2;
    group.position.set(0, 0.072 * H, H * 0.47);
    root.add(group);
    root.updateMatrixWorld(true);
    aim = sk.headC.clone().applyMatrix4(group.matrixWorld);
    return { group: root, materials, aim };
  }
  return { group, materials, aim };
}

