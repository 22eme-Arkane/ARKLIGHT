// Projets d'exemple (liste : shared/demos.js).
//  - « night » : une nuit en extérieur, une rue et la terrasse d'un café ; projecteurs de plusieurs
//    marques (rebond d'un M18 dans un 20×20, face à travers un 12×12, contres, lune, practicals),
//    grip, décor (comédiens, passants, véhicules, mobilier, animaux), équipe caméra, plan annoté.
//  - « clip » : un clip en studio, un salon rétro posé sur un cyclo blanc ; tubes en arc, couleurs
//    saturées, contres dans la brume, voiture, danseurs, musiciens — sans équipe technique. Deux
//    séquences et trois plans : le large et le gros plan du couplet, le refrain en rouge.
import { Object3D, Vector3 } from 'three';
import { getGripType } from '../shared/grip.js';
import { translate } from '../shared/i18n.js';
import { getDemo } from '../shared/demos.js';

const r3 = (v) => Math.round(v * 1000) / 1000;
const pivot = new Object3D();

/** Orientation d'un projecteur qui vise un point (son axe +Z pointe vers la cible). */
function aim(from, to) {
  pivot.position.set(...from);
  pivot.rotation.set(0, 0, 0);
  pivot.lookAt(new Vector3(...to));
  return [pivot.rotation.x, pivot.rotation.y, pivot.rotation.z].map(r3);
}

/** Lacet (rotation autour de la verticale) pour faire face à un point du sol. */
const face = ([x, , z], [tx, , tz]) => r3(Math.atan2(tx - x, tz - z));

const ACTORS = [0, 1.45, 0.2];

/** Couleur autonome d'un Astera (sans DMX, il joue son « look » interne) tirée d'un réglage manuel. */
function asteraLook(manual) {
  const spec = manual.mode === 'hsi' ? { kind: 'hsi', hue: manual.hue || 0, sat: Math.round((manual.sat ?? 1) * 100), cct: 5600 } : { kind: 'hsi', hue: 0, sat: 0, cct: manual.cct || 5600 };
  return { program: 0, intensity: Math.round((manual.intensity ?? 1) * 100), speed: 5, fade: 50, colors: [spec, spec, spec, spec] };
}

/** Outils de construction : projecteurs (réglés, orientés), objets, grip, groupes. */
function kit(store, lang) {
  const tr = (s) => (s ? translate(s, lang) : s);
  const ids = {};
  const fx = (key, type, name, position, target, { settings = {}, manual = null, power = true } = {}) => {
    // les Astera sans DMX suivent leur réglage autonome ; les autres, le réglage manuel
    const astera = type.startsWith('astera.');
    const f = store.addFixture({ type, name: tr(name), position, rotation: Array.isArray(target) ? aim(position, target) : target?.rot || [0, 0, 0], settings: { ...settings, ...(astera && manual ? { standalone: asteraLook(manual) } : {}) } });
    if ((manual && !astera) || !power) store.updateFixture(f.id, { state: { power, ...(manual && !astera ? { manual } : {}) } });
    (ids[key] ||= []).push(f.id);
    return f;
  };
  const obj = (type, name, position, yaw = 0, props = {}, rotation = null) => store.addObject({ type, name: tr(name), position, rotation: rotation || [0, r3(yaw), 0], props });
  const grip = (type, position, yaw, props = {}, rotation = null) => {
    const t = getGripType(type);
    const y = position[1] ?? (t.size ? t.size[1] / 2 : 0);
    return store.addObject({ type, position: [position[0], y, position[2]], rotation: rotation || [0, r3(yaw), 0], props });
  };
  return { tr, ids, fx, obj, grip };
}

/**
 * Remplit la scène du magasin (projet vide) avec un exemple.
 * @param {import('./scene-store.js').SceneStore} store
 * @param {string} [lang]  langue des noms (comédiens, projecteurs, groupes, annotations)
 * @param {string} [id]    exemple (shared/demos.js)
 */
export function buildDemo(store, lang = 'fr', id = 'night') {
  store.newScene();
  store.scene.objects = []; // les personnages de l'exemple remplacent celui du projet vide
  const k = kit(store, lang);
  if (getDemo(id).id === 'clip') buildClip(store, k);
  else buildNight(store, k);
  return store.scene;
}

function buildNight(store, { tr, ids, fx, obj, grip }) {
  // ---------------------------------------------------------------- lumière
  // face : M18 en rebond dans un 20×20 Ultrabounce, SkyPanel S360 à travers un 12×12 grid cloth
  const bounce = [-5.2, 3.65, 3.4];
  fx('face', 'arri.m18', 'M18 rebond', [-1.0, 3.0, 5.0], bounce, { manual: { mode: 'cct', intensity: 0.6, cct: 5600, tint: 0, hue: 0, sat: 1 } });
  fx('face', 'arri.skypanel-s360c', 'S360 face', [5.8, 2.7, 4.6], [0, 1.4, 0.2], { manual: { mode: 'cct', intensity: 0.45, cct: 4300, tint: 0, hue: 0, sat: 1 } });
  fx('face', 'aputure.nova-p300c', 'Nova débouchage', [-2.8, 1.8, 3.6], ACTORS, { manual: { mode: 'cct', intensity: 0.18, cct: 4300, tint: 0, hue: 0, sat: 1 } });
  fx('face', 'dmg-lumiere.sl1-mix', 'SL1 regard', [0.9, 1.7, 2.4], [0, 1.6, 0.25], { manual: { mode: 'cct', intensity: 0.08, cct: 4300, tint: 0, hue: 0, sat: 1 } });
  // contres et lune
  fx('back', 'aputure.ls-600d-pro', '600D contre jardin', [-1.6, 3.4, -4.4], [-0.4, 1.6, 0.2], { manual: { mode: 'cct', intensity: 0.55, cct: 6500, tint: 0, hue: 0, sat: 1 } });
  fx('back', 'aputure.ls-600d', '600D contre cour', [3.0, 3.6, -4.2], [0.6, 1.6, 0.1], { manual: { mode: 'cct', intensity: 0.45, cct: 6500, tint: 0, hue: 0, sat: 1 } });
  fx('back', 'arri.skypanel-s60c', 'S60 lune', [5.2, 3.2, -3.0], [0.4, 1.4, 0.2], { manual: { mode: 'hsi', intensity: 0.5, cct: 5600, tint: 0, hue: 215, sat: 0.45 } });
  // practicals : enseigne du café, guinguette, voiture, lampadaire, porte du café
  fx('prac', 'astera.titan', 'Titan enseigne 1', [-3.9, 2.4, -3.25], { rot: [0, 0, 0] }, { settings: { support: 'hanging' }, manual: { mode: 'hsi', intensity: 0.45, cct: 5600, tint: 0, hue: 28, sat: 0.85 } });
  fx('prac', 'astera.titan', 'Titan enseigne 2', [-5.6, 2.4, -3.25], { rot: [0, 0, 0] }, { settings: { support: 'hanging' }, manual: { mode: 'hsi', intensity: 0.45, cct: 5600, tint: 0, hue: 28, sat: 0.85 } });
  for (const [i, p] of [
    [-3.9, 2.6, -1.6],
    [-4.9, 2.6, -1.0],
    [-5.9, 2.6, -0.4],
  ].entries()) {
    fx('prac', 'astera.nyx', `NYX guinguette ${i + 1}`, p, { rot: [0, 0, 0] }, { settings: { support: 'hanging' }, manual: { mode: 'cct', intensity: 0.35, cct: 2400, tint: 0, hue: 0, sat: 1 } });
  }
  fx('prac', 'astera.helios', 'Helios voiture', [0.3, 1.15, -1.9], { rot: [0, r3(Math.PI / 2), 0] }, { settings: { support: 'none' }, manual: { mode: 'cct', intensity: 0.25, cct: 3200, tint: 0, hue: 0, sat: 1 } });
  fx('prac', 'hive.wasp-100-c', 'Wasp lampadaire', [2.6, 4.0, -2.5], [2.6, 0, -2.45], { settings: { support: 'hanging' }, manual: { mode: 'hsi', intensity: 0.8, cct: 5600, tint: 0, hue: 32, sat: 0.7 } });
  fx('prac', 'mole.baby-1k', 'Baby 1K porte du café', [-6.8, 2.2, -1.4], [-4.8, 0.8, -1.0], { manual: { mode: 'cct', intensity: 0.7, cct: 3200, tint: 0, hue: 0, sat: 1 } });

  // ---------------------------------------------------------------- grip
  grip('frame-20x20', bounce, face(bounce, ACTORS), { cloth: 'ultrabounce' });
  grip('frame-12x12', [3.8, 2.43, 3.0], face([3.8, 0, 3.0], ACTORS), { cloth: 'grid' });
  grip('frame-8x8', [2.0, 2.4, -3.0], face([2.0, 0, -3.0], [0.6, 0, 0.1]), { cloth: 'net-single' });
  grip('frame-6x6', [-2.0, 1.6, 2.9], face([-2.0, 0, 2.9], ACTORS), { cloth: 'lee-216' });
  grip('flag-24x36', [2.6, 1.9, 1.7], -2.2, { cloth: 'solid' });
  grip('floppy', [-1.9, 1.4, -0.4], 1.2, { cloth: 'solid' });
  grip('poly-4x8', [1.4, 1.22, 1.9], face([1.4, 0, 1.9], ACTORS));
  grip('reflector-1m', [-1.2, 1.0, 1.4], face([-1.2, 0, 1.4], ACTORS));
  grip('applebox-full', [0.9, undefined, 4.4], 0.1);
  grip('applebox-half', [1.45, undefined, 4.45], -0.2);
  grip('applebox-pancake', [1.2, undefined, 4.0], 0.4);
  for (const p of [
    [-6.3, undefined, 4.9],
    [-4.2, undefined, 1.8],
    [2.9, undefined, 4.5],
    [6.5, undefined, 5.3],
  ])
    grip('sandbag', p, 0.3);
  grip('cstand', [6.3, 0, 1.0], 0.5);

  // ---------------------------------------------------------------- décor
  // comédiens, figurants, animaux
  obj('person-man', 'Comédien', [-0.6, 0, 0.3], 0.4);
  obj('person-woman', 'Comédienne', [0.6, 0, 0.1], -0.5);
  obj('person-phone', 'Passante au téléphone', [4.0, 0, -1.6], -2.4);
  obj('person-walking', 'Passant', [-2.9, 0, 2.2], 1.5);
  obj('person-child', 'Enfant', [3.4, 0, 0.6], -1.4);
  obj('dog', 'Chien', [4.0, 0, 0.75], -1.4);
  obj('person-sitting', 'Cliente du café', [-4.0, 0, -1.15], Math.PI);
  obj('cat', 'Chat', [-3.3, 0, -2.6], 0.7);
  obj('pigeon', 'Pigeon 1', [4.6, 0, -0.5], 1);
  obj('pigeon', 'Pigeon 2', [4.9, 0, -0.8], 2.4);
  // véhicules
  obj('car-sedan', 'Voiture des comédiens', [0.3, 0, -1.9], Math.PI / 2);
  obj('van-large', 'Camion régie', [9.0, 0, -3.8], 0);
  obj('generator', 'Groupe électrogène', [10.5, 0, 3.0], 0.4);
  obj('motorbike', 'Moto', [4.8, 0, -0.4], 0.5);
  obj('scooter', 'Scooter', [5.6, 0, -2.0], -0.6);
  obj('bicycle', 'Vélo', [-2.9, 0, -3.25], Math.PI / 2);
  // rue
  obj('streetlamp', 'Réverbère', [2.6, 0, -3.0], 0);
  obj('bench', 'Banc', [3.7, 0, -2.9], 0);
  obj('bin', 'Poubelle', [4.7, 0, -2.7], 0);
  for (const p of [
    [-2.3, 0, -0.6],
    [-2.3, 0, -3.0],
    [3.0, 0, -1.0],
  ])
    obj('traffic-cone', null, p, 0);
  obj('road-sign', 'Panneau', [-2.1, 0, -4.3], 0);
  obj('tree', 'Platane 1', [-1.0, 0, -5.4], 0.3);
  obj('tree', 'Platane 2', [1.9, 0, -5.9], 1.7);
  obj('pine', 'Pin', [5.4, 0, -5.6], 0);
  obj('bush', 'Massif 1', [-2.9, 0, -4.7], 0);
  obj('bush', 'Massif 2', [4.1, 0, -4.4], 0.8);
  // café : façade, terrasse
  obj('wall-window', 'Façade du café', [-4.7, 0, -3.5], 0);
  obj('wall-door', 'Porte du café', [-7.1, 0, -3.5], 0);
  obj('table-round', 'Table 1', [-4.0, 0, -1.8], 0);
  obj('chair', 'Chaise 1', [-4.0, 0, -1.15], Math.PI);
  obj('chair', 'Chaise 2', [-4.0, 0, -2.45], 0);
  obj('table-round', 'Table 2', [-5.6, 0, -0.5], 0);
  obj('chair', 'Chaise 3', [-5.6, 0, 0.15], Math.PI);
  obj('chair', 'Chaise 4', [-6.25, 0, -0.5], Math.PI / 2);
  obj('plant', 'Plante 1', [-3.3, 0, -3.1], 0);
  obj('plant', 'Plante 2', [-6.0, 0, -3.1], 0);
  obj('crate', 'Cagette', [-6.5, 0, -2.6], 0.3);
  obj('bottle', 'Bouteille', [-3.95, 0.75, -1.8], 0);
  obj('wine-glass', 'Verre 1', [-4.15, 0.75, -1.7], 0);
  obj('wine-glass', 'Verre 2', [-5.55, 0.75, -0.45], 0);
  // équipe caméra
  obj('camera-dolly', 'Caméra A', [0.2, 0, 5.2], Math.PI, { focal: 40 });
  obj('camera-tripod', 'Caméra B', [-3.3, 0, 4.0], face([-3.3, 0, 4.0], ACTORS), { focal: 85 });
  obj('director-chair', 'Fauteuil réalisatrice', [2.1, 0, 6.8], Math.PI);
  obj('monitor-stand', 'Moniteur', [2.9, 0, 5.7], 0);
  obj('boom-stand', 'Perche son', [-1.6, 0, 1.7], 2.6);
  obj('person-pointing', 'Réalisatrice', [2.7, 0, 6.3], Math.PI);
  obj('person-crossed', 'Cheffe opératrice', [0.9, 0, 6.2], Math.PI);
  obj('person-man', 'Électro', [6.0, 0, 3.6], -1.9, { top: '#1d1f22', pants: '#1d1f22' });
  obj('flightcase', 'Flight 1', [7.4, 0, -1.4], 0.2);
  obj('flightcase', 'Flight 2', [7.6, 0, -0.5], -0.1);

  // ---------------------------------------------------------------- groupes, plan de feu, ambiance
  store.createGroup({ name: tr('Face'), ids: ids.face });
  store.createGroup({ name: tr('Contres et lune'), ids: ids.back });
  store.createGroup({ name: tr('Practicals'), ids: ids.prac });
  store.addPlotItem({ kind: 'rect', points: [[-8.4, -3.7], [-2.9, 0.9]], text: tr('Terrasse du café'), style: { stroke: '#c792ea', dash: true } });
  store.addPlotItem({ kind: 'arrow', points: [[-7.5, 3.1], [5.5, 3.1]], text: tr('Passage des figurants'), style: { stroke: '#3ddc84' } });
  store.addPlotItem({ kind: 'measure', points: [[0.2, 4.4], [0, 0.3]], style: { stroke: '#5aa9ff' } });
  store.addPlotItem({ kind: 'text', points: [[7.6, -7.2]], text: tr('Régie'), style: { stroke: '#9aa3ae', size: 18 } });
  store.addPlotItem({ kind: 'text', points: [[9.2, 5.6]], text: tr('Groupe à 50 m'), style: { stroke: '#9aa3ae', size: 14 } });
  store.updatePlotMeta({ title: { production: tr('Projet d’exemple ARKLIGHT'), author: '', notes: tr('Nuit — rue et terrasse de café') } });
  store.updateEnvironment({ cyc: false, floor: true, stands: true, labels: false, labelPatch: false, haze: 0.1, ambient: 0.035, exposure: 0.35 });
}

// ------------------------------------------------------------------ clip en studio : salon rétro sur cyclo blanc
const SINGER = [0, 1.55, -0.6];

function buildClip(store, { tr, ids, fx, obj, grip }) {
  const hsi = (hue, sat, intensity) => ({ mode: 'hsi', intensity, cct: 5600, tint: 0, hue, sat });
  const cct = (kelvin, intensity) => ({ mode: 'cct', intensity, cct: kelvin, tint: 0, hue: 0, sat: 1 });
  const up = { rot: [0, 0, 0] }; // tube debout (l'axe d'un tube est la verticale du modèle)

  // ---------------------------------------------------------------- lumière
  // arc de tubes devant le cyclo : magenta et cyan en alternance
  for (let i = 0; i < 8; i++) {
    const x = -5.25 + i * 1.5;
    const z = -5.4 - Math.cos((x / 5.25) * (Math.PI / 2)) * 0.6;
    fx('tubes', 'astera.titan', `Titan cyclo ${i + 1}`, [r3(x), 0.6, r3(z)], up, { settings: { support: 'none' }, manual: i % 2 ? hsi(190, 1, 0.85) : hsi(305, 1, 0.85) });
  }
  // colonnes de lumière de part et d'autre du décor
  fx('tubes', 'astera.hyperion', 'Hyperion jardin', [-5.4, 1.05, -2.6], up, { settings: { support: 'none' }, manual: hsi(205, 0.9, 0.8) });
  fx('tubes', 'astera.hyperion', 'Hyperion cour', [5.4, 1.05, -2.6], up, { settings: { support: 'none' }, manual: hsi(205, 0.9, 0.8) });
  // le cyclo éclairé par le bas : dégradé ambre → magenta → violet
  for (const [i, [x, hue]] of [
    [-3.2, 28],
    [0, 320],
    [3.2, 268],
  ].entries()) {
    fx('cyc', 'astera.ax9', `AX9 cyclo ${i + 1}`, [x, 0.25, -6.5], [x, 3.6, -7.2], { settings: { support: 'none' }, manual: hsi(hue, 1, 0.9) });
  }
  // face douce à travers un 8×8, débouchage, lumière d'ambiance zénithale
  fx('face', 'arri.l10-c', 'L10 face', [-3.3, 3.1, 2.0], SINGER, { manual: cct(3600, 0.75) });
  fx('face', 'kino-flo.celeb-450-led-dmx', 'Celeb débouchage', [3.0, 1.8, 2.6], [0, 1.4, -1.0], { manual: cct(4300, 0.2) });
  fx('top', 'arri.skypanel-s360c', 'S360 zénith', [0, 4.3, -2.8], [0, 0, -2.79], { settings: { support: 'hanging' }, manual: hsi(275, 0.35, 0.25) });
  // contres durs qui découpent la brume
  fx('back', 'astera.leo', 'Leo contre cour', [3.9, 3.6, -5.3], SINGER, { manual: cct(6500, 0.85) });
  fx('back', 'astera.pluto', 'Pluto contre jardin', [-3.9, 3.4, -5.1], [0, 1.4, -0.9], { manual: hsi(200, 0.55, 0.85) });
  // voiture : un projecteur coloré qui la caresse
  fx('car', 'hive.hornet-200-c', 'Hornet voiture', [6.3, 2.3, -0.4], [4.6, 0.7, -2.6], { manual: hsi(320, 0.9, 0.6) });
  // practicals : ampoules dans les lampadaires, gouttes de lumière sur la table basse
  fx('prac', 'astera.nyx', 'NYX lampadaire jardin', [-1.9, 1.45, -3.5], { rot: [0, 0, 0] }, { settings: { support: 'hanging' }, manual: cct(2400, 0.55) });
  fx('prac', 'astera.nyx', 'NYX lampadaire cour', [1.9, 1.45, -3.6], { rot: [0, 0, 0] }, { settings: { support: 'hanging' }, manual: cct(2400, 0.55) });
  for (const [i, x] of [-0.35, 0, 0.35].entries()) fx('prac', 'astera.ax3', `AX3 table basse ${i + 1}`, [x, 0.52, -2.3], { rot: [0, 0, 0] }, { settings: { support: 'none' }, manual: cct(2200, 0.35) });

  // ---------------------------------------------------------------- grip
  grip('frame-8x8', [-2.2, 2.4, 1.1], face([-2.2, 0, 1.1], SINGER), { cloth: 'lee-250' });
  grip('frame-6x6', [2.7, 1.5, 0.6], face([2.7, 0, 0.6], SINGER), { cloth: 'solid' });
  grip('floppy', [3.1, 2.5, -3.6], -0.6, { cloth: 'solid' });
  grip('flag-24x36', [-3.0, 2.6, -3.9], 0.7, { cloth: 'solid' });
  for (const p of [
    [-3.2, undefined, 1.6],
    [2.9, undefined, 0.2],
    [3.4, undefined, 2.2],
  ])
    grip('sandbag', p, 0.4);

  // ---------------------------------------------------------------- décor : salon rétro sur le cyclo
  obj('rug', 'Tapis du salon', [0, 0, -2.2], 0, { color: '#8c3b2e' });
  obj('sofa-3', 'Canapé', [0, 0, -3.4], 0, { color: '#2f6f73' });
  obj('coffee-table', 'Table basse', [0, 0, -2.3], 0, { color: '#6b4426' });
  obj('armchair', 'Fauteuil', [2.3, 0, -2.4], -0.8, { color: '#c98a2b' });
  obj('floor-lamp', 'Lampadaire jardin', [-1.9, 0, -3.5], 0);
  obj('floor-lamp', 'Lampadaire cour', [1.9, 0, -3.6], 0);
  obj('tv', 'Téléviseur vintage', [-2.6, 0, -2.0], 1.0);
  obj('plant', 'Plante', [2.8, 0, -3.6], 0);
  obj('column', 'Colonne jardin', [-4.1, 0, -4.4], 0);
  obj('column', 'Colonne cour', [4.1, 0, -4.4], 0);
  obj('bottle', 'Bouteille', [0.15, 0.42, -2.25], 0);
  obj('wine-glass', 'Verre 1', [-0.15, 0.42, -2.2], 0);
  obj('car-city', 'Voiture vintage', [4.6, 0, -2.6], -2.4, { color: '#b3262e' });
  // artistes
  obj('person-woman', 'Chanteuse', [0, 0, -0.6], 0, { top: '#c2185b', pants: '#1d1f22', hair: 'long', hairColor: '#e6c27a' });
  obj('mic-stand', 'Micro', [0, 0, -0.25], Math.PI);
  obj('person-sitting', 'Guitariste', [-0.6, 0, -3.25], 0, { top: '#f2f0ea' });
  obj('guitar', 'Guitare', [-1.4, 0, -2.9], 0.4);
  obj('person-crossed', 'Bassiste', [-2.0, 0, -1.3], 0.6, { top: '#2b2f36' });
  obj('person-handsup', 'Danseuse 1', [2.4, 0, -0.9], -0.4, { top: '#7c4dff' });
  obj('person-handsup', 'Danseur 2', [-2.9, 0, -0.5], 0.5, { top: '#00bcd4' });
  obj('dog-small', 'Chien', [2.9, 0, -1.8], -1.2);
  obj('cat', 'Chat', [2.3, 0.42, -2.4], -0.8);
  // caméras du clip (sans équipe)
  obj('camera-dolly', 'Caméra A', [0, 0, 5.6], Math.PI, { focal: 32 });
  obj('camera-tripod', 'Caméra B', [-4.6, 0, 2.8], face([-4.6, 0, 2.8], SINGER), { focal: 85 });

  // ---------------------------------------------------------------- groupes, plan de feu, ambiance
  store.createGroup({ name: tr('Tubes du cyclo'), ids: ids.tubes });
  store.createGroup({ name: tr('Cyclo par le bas'), ids: ids.cyc });
  store.createGroup({ name: tr('Face'), ids: ids.face });
  store.createGroup({ name: tr('Contres'), ids: ids.back });
  store.createGroup({ name: tr('Practicals'), ids: ids.prac });
  store.addPlotItem({ kind: 'rect', points: [[-3.4, -4.3], [3.4, -0.1]], text: tr('Décor du salon'), style: { stroke: '#c792ea', dash: true } });
  store.addPlotItem({ kind: 'arrow', points: [[0, 6.4], [0, 3.8]], text: tr('Travelling avant'), style: { stroke: '#3ddc84' } });
  store.addPlotItem({ kind: 'text', points: [[-1.2, -7.6]], text: tr('Cyclo blanc'), style: { stroke: '#9aa3ae', size: 16 } });
  store.addPlotItem({ kind: 'measure', points: [[0, 5.0], [0, -0.4]], style: { stroke: '#5aa9ff' } });
  store.updatePlotMeta({ title: { production: tr('Projet d’exemple ARKLIGHT'), author: '', notes: tr('Clip — salon rétro sur cyclo') } });
  store.updateEnvironment({ cyc: true, floor: true, stands: true, labels: false, labelPatch: false, haze: 0.45, ambient: 0.02, exposure: 0.6 });

  // ---------------------------------------------------------------- séquences et plans
  // Couplet : le large (ci-dessus), puis le gros plan de la chanteuse (copie retouchée)
  const seq1 = store.sequences[0];
  const large = seq1.plans[0].id;
  store.renameSequence(seq1.id, tr('Couplet — salon'));
  store.renamePlan(large, tr('Plan 1 — large'));
  store.newPlan({ mode: 'copy', name: tr('Plan 2 — gros plan chanteuse') });
  const [l10] = ids.face;
  store.updateFixture(l10, { position: [-2.4, 2.5, 1.3], rotation: aim([-2.4, 2.5, 1.3], SINGER), state: { manual: cct(3600, 0.6) } });
  for (const id of ids.tubes) store.updateFixture(id, { settings: { standalone: asteraLook(hsi(store.getFixture(id).settings.standalone.colors[0].hue, 1, 0.4)) } });
  for (const o of store.scene.objects.filter((x) => x.type === 'person-handsup')) store.removeObject(o.id);
  const camA = store.scene.objects.find((x) => x.type === 'camera-dolly');
  if (camA) store.updateObject(camA.id, { position: [0, 0, 1.7], props: { focal: 85 } });
  store.addPlotItem({ kind: 'text', points: [[0.6, 2.2]], text: tr('Gros plan 85 mm'), style: { stroke: '#3ddc84', size: 14 } });
  // Refrain : même salon, le cyclo et les tubes passent au rouge, plus de brume
  store.switchPlan(large);
  store.newSequence({ mode: 'copy', name: tr('Refrain — cyclo rouge') });
  store.renamePlan(store.current.plan, tr('Plan 1 — large rouge'));
  ids.tubes.forEach((id, i) => store.updateFixture(id, { settings: { standalone: asteraLook(hsi(i % 2 ? 18 : 352, 1, 0.95)) } }));
  for (const id of ids.cyc) store.updateFixture(id, { settings: { standalone: asteraLook(hsi(350, 1, 1)) } });
  store.updateEnvironment({ haze: 0.6 });
  store.switchPlan(large);
}
