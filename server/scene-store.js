// Scène courante (projecteurs placés + environnement), sauvegarde automatique et fichiers de projet.
// Un projet contient des séquences et des plans (shared/plans.js) : this.scene est la scène du plan
// courant, les autres plans sont gardés à part (copie complète de leur scène).
import { EventEmitter } from 'node:events';
import { existsSync, mkdirSync, readdirSync, unlinkSync } from 'node:fs';
import { join } from 'node:path';
import { PROJECT_EXT, cleanThumbnail, projectName, readProjectFile, safeName, writeProjectFile } from './projects.js';
import { cleanPlanName, defaultPlanName, defaultSequenceName, nextNumber } from '../shared/plans.js';
import { translate } from '../shared/i18n.js';
import { defaultSettings, defaultState, getFixtureType, modeFootprint } from '../shared/fixtures/index.js';
import { releaseOnLookChange } from '../shared/engine/console.js';
import { cleanObjectProps, defaultObjectProps, getObjectType, isProp } from '../shared/objects.js';

const AUTOSAVE = '_autosave';
let revisionCounter = 0;
const nextRevision = () => ++revisionCounter;

export function emptyScene(lang = 'fr') {
  return {
    version: 1,
    name: translate('Nouveau projet', lang),
    environment: { floor: true, cyc: true, stands: true, labels: false, labelPatch: false, haze: 0.25, ambient: 0.04, exposure: 1 },
    fixtures: [],
    groups: [], // sélections nommées : { id, name, ids } (un projecteur peut être dans plusieurs groupes)
    objects: [defaultActor(lang)], // grip et décor : { id, type, name, position, rotation, props }
    plot: emptyPlot(), // plan de feu 2D : formes, textes, flèches, image de fond
  };
}

/** Comédien placé au centre du plateau (déplaçable, remplaçable, d'autres peuvent s'ajouter). */
export function defaultActor(lang = 'fr') {
  const type = getObjectType('person-man');
  return { id: newId('o'), type: type.id, name: translate('Comédien', lang), position: [0, 0, 0], rotation: [0, 0, 0], props: defaultObjectProps(type) };
}

export function emptyPlot() {
  return { items: [], underlay: null, background: null, title: { production: '', author: '', notes: '' } };
}

const PLOT_KINDS = new Set(['rect', 'ellipse', 'polygon', 'line', 'freehand', 'text', 'arrow', 'measure']);
const num = (v, d = 0) => (Number.isFinite(Number(v)) ? Number(v) : d);
const newId = (p) => `${p}${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;

/** Nettoie un élément du plan (coordonnées au sol en mètres : [x, z]). */
function cleanPlotItem(it, id) {
  if (!it || !PLOT_KINDS.has(it.kind)) return null;
  const points = (Array.isArray(it.points) ? it.points : []).slice(0, 5000).map((p) => [num(p?.[0]), num(p?.[1])].map((v) => Math.round(v * 1000) / 1000));
  if (!points.length) return null;
  const style = it.style && typeof it.style === 'object' ? it.style : {};
  return {
    id: id || it.id || newId('p'),
    kind: it.kind,
    points,
    text: typeof it.text === 'string' ? it.text.slice(0, 500) : '',
    style: {
      stroke: typeof style.stroke === 'string' ? style.stroke.slice(0, 20) : '#ffad3b',
      fill: typeof style.fill === 'string' ? style.fill.slice(0, 20) : 'none',
      width: Math.min(20, Math.max(0.5, num(style.width, 2))),
      dash: !!style.dash,
      size: Math.min(200, Math.max(6, num(style.size, 16))),
    },
    height: Math.min(100, Math.max(0, num(it.height))), // > 0 : volume extrudé dans la vue 3D
  };
}

function isPlainObject(v) {
  return v && typeof v === 'object' && !Array.isArray(v);
}

function deepMerge(target, patch) {
  for (const [k, v] of Object.entries(patch)) {
    if (isPlainObject(v) && isPlainObject(target[k])) deepMerge(target[k], v);
    else target[k] = v;
  }
  return target;
}

const groupName = (name) => String(name ?? '').replace(/\s+/g, ' ').trim().slice(0, 40);
/** Couleur « #rrggbb », ou null (automatique). */
const cleanColor = (c) => (typeof c === 'string' && /^#[0-9a-f]{6}$/i.test(c) ? c.toLowerCase() : null);

/** Numéro de fixture : entier de 1 à 9999, ou null (aucun). */
function cleanFixtureNumber(v) {
  if (v === null || v === undefined || v === '') return null;
  const n = Math.round(Number(v));
  return Number.isFinite(n) ? Math.min(9999, Math.max(1, n)) : null;
}

export class SceneStore extends EventEmitter {
  constructor(dir) {
    super();
    this.dir = dir;
    mkdirSync(dir, { recursive: true });
    // Révisions : chaque modification change le numéro ; la scène est « enregistrée » tant que le
    // numéro est celui du dernier enregistrement (une annulation peut y ramener).
    this.revision = nextRevision();
    this.savedRevision = this.revision;
    this.projectsDir = dir; // dossier des projets (réglable) ; la sauvegarde automatique reste dans « dir »
    this.savedPath = null; // fichier du projet (null : jamais enregistré)
    this.thumbnail = null; // vignette du dernier enregistrement
    this._timer = null;
    const auto = this._read(AUTOSAVE, true);
    this.scene = auto?.scene || emptyScene();
    this._loadPlans(auto?.sequences, auto?.current);
    if (auto?.session) {
      const ss = auto.session;
      // premières versions : nom de la scène dans le dossier des scènes
      this.savedPath = typeof ss.savedPath === 'string' ? ss.savedPath : typeof ss.savedName === 'string' ? this._path(ss.savedName) : null;
      if (ss.dirty) this.savedRevision = -1;
    }
  }

  /** Nom du projet enregistré (celui de son fichier), ou null. */
  get savedName() {
    return this.savedPath ? projectName(this.savedPath) : null;
  }

  /** Modifications non enregistrées. */
  get dirty() {
    return this.revision !== this.savedRevision;
  }

  _path(name) {
    return join(this.dir, `${name}${PROJECT_EXT}`);
  }

  _projectPath(name) {
    return join(this.projectsDir, `${name}${PROJECT_EXT}`);
  }

  _read(name, withSession = false) {
    try {
      const r = readProjectFile(this._path(name));
      if (!r) return null;
      const clean = this._sanitize(r.scene);
      // sauvegarde automatique : état « enregistrée ou non » (r.session), séquences et plans
      return withSession ? { scene: clean, session: r.session, sequences: r.sequences, current: r.current } : clean;
    } catch {
      return null;
    }
  }

  /** Ajoute les valeurs par défaut manquantes (scènes créées par une version antérieure). */
  _sanitize(scene) {
    const base = emptyScene();
    // premières versions (pas encore d'objets) : le personnage faisait partie du décor fixe
    // (environment.actor) ; ensuite, un projet sans personnage le reste
    const hadActor = !Array.isArray(scene.objects) && (scene.environment?.actor ?? true) !== false;
    scene.environment = { ...base.environment, ...(scene.environment || {}) };
    delete scene.environment.actor;
    scene.fixtures = (scene.fixtures || []).filter((f) => getFixtureType(f.type));
    for (const f of scene.fixtures) {
      const type = getFixtureType(f.type);
      const defaults = defaultSettings(type);
      f.settings = { ...defaults, ...(f.settings || {}) };
      // Mode DMX disparu (bibliothèque mise à jour) : retour au mode par défaut du modèle
      if (type.dmx?.modes?.length && !type.dmx.modes.some((m) => m.id === f.settings.dmxMode)) f.settings.dmxMode = defaults.dmxMode;
      f.state = deepMerge(defaultState(type), f.state || {});
    }
    // numéro de fixture (console, appli) : attribué à la suite aux projecteurs qui n'en ont pas
    let nextFix = Math.max(0, ...scene.fixtures.map((f) => Number(f.settings.fixtureNumber) || 0)) + 1;
    for (const f of scene.fixtures) if (f.settings.fixtureNumber === undefined && getFixtureType(f.type).dmx?.modes?.length) f.settings.fixtureNumber = nextFix++;
    scene.objects = (Array.isArray(scene.objects) ? scene.objects : []).filter((o) => o && o.id && getObjectType(o.type));
    for (const o of scene.objects) {
      const type = getObjectType(o.type);
      o.position = Array.isArray(o.position) ? o.position.map(Number) : [0, 0, 0];
      o.rotation = Array.isArray(o.rotation) ? o.rotation.map(Number) : [0, 0, 0];
      o.props = { ...defaultObjectProps(type), ...cleanObjectProps(type, o.props || {}) };
    }
    if (hadActor && !scene.objects.some((o) => getObjectType(o.type).model === 'person')) scene.objects.unshift(defaultActor());
    const plot = scene.plot && typeof scene.plot === 'object' ? scene.plot : {};
    scene.plot = { ...emptyPlot(), ...plot, items: (plot.items || []).map((it) => cleanPlotItem(it)).filter(Boolean), title: { ...emptyPlot().title, ...(plot.title || {}) } };
    scene.plot.background = cleanColor(scene.plot.background);
    const ids = new Set(scene.fixtures.map((f) => f.id));
    scene.groups = (Array.isArray(scene.groups) ? scene.groups : [])
      .filter((g) => g && g.id)
      .map((g) => ({ id: String(g.id), name: groupName(g.name) || 'Groupe', ids: [...new Set(g.ids || [])].filter((id) => ids.has(id)) }));
    return scene;
  }

  /** La scène a changé : nouvelle révision, sauvegarde automatique un peu plus tard. */
  _changed() {
    this.revision = nextRevision();
    this._touch();
  }

  _touch() {
    clearTimeout(this._timer);
    this._timer = setTimeout(() => this._write(AUTOSAVE), 500);
  }

  _write(name) {
    writeProjectFile(this._path(name), this.scene, { extra: { ...this._plansForFile(), _session: { savedPath: this.savedPath, dirty: this.dirty } } });
  }

  /** Prochaine adresse libre dans un univers, après le dernier projecteur patché. */
  nextFreeAddress(universe, footprint) {
    let end = 0;
    for (const f of this.scene.fixtures) {
      if (f.settings.universe !== universe) continue;
      const fp = modeFootprint(getFixtureType(f.type), f.settings.dmxMode);
      end = Math.max(end, f.settings.dmxAddress + fp - 1);
    }
    return end + footprint <= 512 ? end + 1 : 1;
  }

  /** Prochain numéro de fixture libre (après le plus grand). */
  nextFixtureNumber() {
    return Math.max(0, ...this.scene.fixtures.map((f) => Number(f.settings.fixtureNumber) || 0)) + 1;
  }

  addFixture({ type: typeId, position = [0, 1.5, 0], rotation = [0, 0, 0], name, settings = {}, autoAddress = true }) {
    const type = getFixtureType(typeId);
    if (!type) throw new Error(`Type de projecteur inconnu : ${typeId}`);
    const count = this.scene.fixtures.filter((f) => f.type === typeId).length;
    const fixture = {
      id: `f${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
      type: typeId,
      name: name || `${type.name} ${count + 1}`,
      position,
      rotation,
      settings: { ...defaultSettings(type), ...settings },
      state: defaultState(type),
    };
    if (autoAddress && settings.dmxAddress === undefined) {
      fixture.settings.dmxAddress = this.nextFreeAddress(fixture.settings.universe, modeFootprint(type, fixture.settings.dmxMode));
    }
    if (settings.fixtureNumber === undefined && type.dmx?.modes?.length) fixture.settings.fixtureNumber = this.nextFixtureNumber();
    else fixture.settings.fixtureNumber = cleanFixtureNumber(settings.fixtureNumber);
    this.scene.fixtures.push(fixture);
    this._changed();
    return fixture;
  }

  getFixture(id) {
    return this.scene.fixtures.find((f) => f.id === id) || null;
  }

  updateFixture(id, changes) {
    const f = this.getFixture(id);
    if (!f) return null;
    // un réglage de la lumière fait à la main (inspecteur, menu, assistant) rend la main à la console
    const { position, rotation, name, settings, state } = releaseOnLookChange(f, changes);
    if (Array.isArray(position) && position.length === 3) f.position = position.map(Number);
    if (Array.isArray(rotation) && rotation.length === 3) f.rotation = rotation.map(Number);
    if (typeof name === 'string') f.name = name.slice(0, 60);
    if (isPlainObject(settings)) {
      if ('fixtureNumber' in settings) settings.fixtureNumber = cleanFixtureNumber(settings.fixtureNumber);
      deepMerge(f.settings, settings);
    }
    if (isPlainObject(state)) deepMerge(f.state, state);
    this._changed();
    return f;
  }

  removeFixture(id) {
    const i = this.scene.fixtures.findIndex((f) => f.id === id);
    if (i < 0) return false;
    this.scene.fixtures.splice(i, 1);
    for (const g of this.scene.groups) g.ids = g.ids.filter((x) => x !== id);
    this._changed();
    return true;
  }

  // ---------------------------------------------------------------- grip et accessoires
  getObject(id) {
    return this.scene.objects.find((o) => o.id === id) || null;
  }

  addObject({ type: typeId, position = [0, 0, 0], rotation = [0, 0, 0], name, props = {} }) {
    const type = getObjectType(typeId);
    if (!type) throw new Error(`Objet inconnu : ${typeId}`);
    const count = this.scene.objects.filter((o) => o.type === typeId).length;
    const obj = {
      id: newId('o'),
      type: typeId,
      name: typeof name === 'string' && name.trim() ? name.trim().slice(0, 60) : `${translate(type.short, this.lang)} ${count + 1}`,
      position: position.map(Number),
      rotation: rotation.map(Number),
      props: { ...defaultObjectProps(type), ...cleanObjectProps(type, isPlainObject(props) ? props : {}) },
    };
    this.scene.objects.push(obj);
    this._changed();
    return obj;
  }

  updateObject(id, { position, rotation, name, props } = {}) {
    const o = this.getObject(id);
    if (!o) return null;
    if (Array.isArray(position) && position.length === 3) o.position = position.map(Number);
    if (Array.isArray(rotation) && rotation.length === 3) o.rotation = rotation.map(Number);
    if (typeof name === 'string' && name.trim()) o.name = name.trim().slice(0, 60);
    if (isPlainObject(props)) {
      // seuls les réglages valides de son type sont repris (pose connue, taille bornée, pied proposé…)
      deepMerge(o.props, cleanObjectProps(getObjectType(o.type), props));
    }
    this._changed();
    return o;
  }

  removeObject(id) {
    const i = this.scene.objects.findIndex((o) => o.id === id);
    if (i < 0) return false;
    this.scene.objects.splice(i, 1);
    this._changed();
    return true;
  }

  // ---------------------------------------------------------------- plan de feu 2D
  addPlotItem(item) {
    const it = cleanPlotItem(item, newId('p'));
    if (!it) throw new Error('Élément de plan invalide');
    this.scene.plot.items.push(it);
    this._changed();
    return it;
  }

  updatePlotItem(id, changes = {}) {
    const i = this.scene.plot.items.findIndex((x) => x.id === id);
    if (i < 0) return null;
    const cur = this.scene.plot.items[i];
    const it = cleanPlotItem({ ...cur, ...changes, style: { ...cur.style, ...(changes.style || {}) } }, id);
    if (!it) return null;
    this.scene.plot.items[i] = it;
    this._changed();
    return it;
  }

  removePlotItem(id) {
    const i = this.scene.plot.items.findIndex((x) => x.id === id);
    if (i < 0) return false;
    this.scene.plot.items.splice(i, 1);
    this._changed();
    return true;
  }

  /** Image de fond (calée à l'échelle), couleur du fond et cartouche du plan. */
  updatePlotMeta({ underlay, title, background } = {}) {
    const p = this.scene.plot;
    if (background !== undefined) p.background = cleanColor(background);
    if (underlay === null) p.underlay = null;
    else if (isPlainObject(underlay) && typeof underlay.url === 'string' && underlay.url.startsWith('/assets/')) {
      p.underlay = {
        url: underlay.url,
        x: num(underlay.x),
        z: num(underlay.z),
        width: Math.max(0.1, num(underlay.width, 10)),
        aspect: Math.max(0.01, num(underlay.aspect, 1)),
        rotation: num(underlay.rotation),
        opacity: Math.min(1, Math.max(0.05, num(underlay.opacity, 0.6))),
        floor3d: !!underlay.floor3d,
      };
    }
    if (isPlainObject(title)) for (const k of ['production', 'author', 'notes']) if (typeof title[k] === 'string') p.title[k] = title[k].slice(0, 300);
    this._changed();
    return p;
  }

  // ---------------------------------------------------------------- groupes
  getGroup(id) {
    return this.scene.groups.find((g) => g.id === id) || null;
  }

  _validIds(ids) {
    return [...new Set(Array.isArray(ids) ? ids : [])].filter((id) => this.getFixture(id));
  }

  createGroup({ name, ids } = {}) {
    const groups = this.scene.groups;
    let n = groups.length + 1;
    while (groups.some((g) => g.name === `Groupe ${n}`)) n++;
    const group = {
      id: `g${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`,
      name: groupName(name) || `Groupe ${n}`,
      ids: this._validIds(ids),
    };
    groups.push(group);
    this._changed();
    return group;
  }

  /** name : nouveau nom ; ids : remplace les membres ; add / remove : ajoute / retire des membres. */
  updateGroup(id, { name, ids, add, remove } = {}) {
    const g = this.getGroup(id);
    if (!g) return null;
    if (groupName(name)) g.name = groupName(name);
    if (Array.isArray(ids)) g.ids = this._validIds(ids);
    for (const x of this._validIds(add)) if (!g.ids.includes(x)) g.ids.push(x);
    if (Array.isArray(remove)) g.ids = g.ids.filter((x) => !remove.includes(x));
    this._changed();
    return g;
  }

  removeGroup(id) {
    const i = this.scene.groups.findIndex((g) => g.id === id);
    if (i < 0) return false;
    this.scene.groups.splice(i, 1);
    this._changed();
    return true;
  }

  updateEnvironment(changes) {
    if (isPlainObject(changes)) deepMerge(this.scene.environment, changes);
    this._changed();
    return this.scene.environment;
  }

  /** Renomme le projet. Déjà enregistré, son fichier est renommé à côté (rien de neuf à enregistrer). */
  rename(name) {
    this.scene.name = safeName(name) || this.scene.name;
    if (this.savedPath) this._touch();
    else this._changed();
  }

  newScene() {
    this.scene = emptyScene(this.lang);
    this._resetPlans();
    this._changed();
    this.savedRevision = this.revision; // rien à enregistrer dans un projet vide
    this.savedPath = null;
    this.thumbnail = null;
    return this.scene;
  }

  /** Nouveau projet tiré d'une scène toute faite (projet d'exemple) : à enregistrer pour le garder. */
  loadTemplate(scene, name) {
    this.scene = this._sanitize(structuredClone(scene));
    this.scene.name = safeName(name) || 'Nouveau projet';
    this._resetPlans();
    this.savedPath = null;
    this.thumbnail = null;
    this._changed();
    return this.scene;
  }

  /** Enregistre le projet dans ce fichier (son nom devient celui du projet), avec sa vignette. */
  saveTo(path, { thumbnail = null } = {}) {
    const name = projectName(path);
    if (!safeName(name) || name.startsWith('_')) throw new Error('Nom de projet invalide');
    this.scene.name = name;
    if (thumbnail) this.thumbnail = thumbnail;
    if (thumbnail) this._currentPlan().plan.thumbnail = thumbnail;
    writeProjectFile(path, this.scene, { thumbnail: this.thumbnail, extra: this._plansForFile() });
    this.savedRevision = this.revision;
    this.savedPath = path;
    this._touch();
    return name;
  }

  /** Enregistre sous ce nom dans le dossier des projets. */
  save(name, opts) {
    const n = safeName(name || this.scene.name);
    if (!n || n.startsWith('_')) throw new Error('Nom de projet invalide');
    return this.saveTo(this._projectPath(n), opts);
  }

  /** Un projet de ce nom existe déjà dans le dossier des projets. */
  exists(name) {
    const n = safeName(name);
    return !!n && existsSync(this._projectPath(n));
  }

  /** Ouvre un fichier de projet (le nom du projet est celui du fichier). */
  loadFrom(path) {
    let r = null;
    try {
      r = readProjectFile(path);
    } catch (e) {
      throw new Error(e.code === 'ENOENT' ? `Projet introuvable : ${projectName(path)}` : `Fichier illisible : ${projectName(path)}`);
    }
    if (!r) throw new Error(`${projectName(path)} n’est pas un projet ARKLIGHT`);
    this.scene = this._sanitize(r.scene);
    this.scene.name = projectName(path);
    this._loadPlans(r.sequences, r.current);
    this.thumbnail = r.thumbnail;
    this._changed();
    this.savedRevision = this.revision;
    this.savedPath = path;
    return this.scene;
  }

  load(name) {
    const n = safeName(name);
    if (!n || !existsSync(this._projectPath(n))) throw new Error(`Projet introuvable : ${name}`);
    return this.loadFrom(this._projectPath(n));
  }

  /**
   * Annuler / rétablir : remet un état de la scène. Le nom et l'environnement (brume, exposition,
   * décor) restent ceux d'aujourd'hui : ce sont des réglages de la vue, pas des étapes du travail.
   */
  restore(json, revision) {
    const scene = JSON.parse(json);
    scene.name = this.scene.name;
    scene.environment = this.scene.environment;
    this.scene = scene;
    this.revision = revision;
    this._touch();
    return scene;
  }

  remove(name) {
    const p = this._projectPath(safeName(name));
    if (existsSync(p)) unlinkSync(p);
  }

  list() {
    return readdirSync(this.projectsDir)
      .filter((f) => f.endsWith(PROJECT_EXT) && !f.startsWith('_'))
      .map((f) => f.slice(0, -PROJECT_EXT.length))
      .sort((a, b) => a.localeCompare(b, 'fr'));
  }

  /** Univers utilisés par les projecteurs (pour sACN multicast et ArtPollReply). */
  usedUniverses() {
    return [...new Set(this.scene.fixtures.map((f) => f.settings.universe))].sort((a, b) => a - b);
  }

  flush() {
    clearTimeout(this._timer);
    this._write(AUTOSAVE);
  }

  // ------------------------------------------------------------------ séquences et plans
  /** Nouveau projet : une séquence, un plan (le plan courant). */
  _resetPlans() {
    const plan = { id: newId('p'), name: defaultPlanName(1, this.lang), thumbnail: null, scene: null };
    const seq = { id: newId('s'), name: defaultSequenceName(1, this.lang), plans: [plan] };
    this.sequences = [seq];
    this.current = { seq: seq.id, plan: plan.id };
  }

  /**
   * Séquences et plans lus dans un fichier. Le plan courant est celui dont la scène est au premier
   * niveau du fichier (scene: null) ; un projet d'avant les séquences devient « Séquence 1 › Plan 1 ».
   */
  _loadPlans(sequences, current) {
    const ids = new Set();
    const uid = (v, prefix) => {
      const id = typeof v === 'string' && /^[\w-]{1,40}$/.test(v) && !ids.has(v) ? v : newId(prefix);
      ids.add(id);
      return id;
    };
    const seqs = [];
    for (const sq of Array.isArray(sequences) ? sequences : []) {
      if (!sq || typeof sq !== 'object') continue;
      const plans = [];
      for (const p of Array.isArray(sq.plans) ? sq.plans : []) {
        if (!p || typeof p !== 'object') continue;
        const scene = p.scene && typeof p.scene === 'object' && Array.isArray(p.scene.fixtures) ? p.scene : null;
        plans.push({ id: uid(p.id, 'p'), name: cleanPlanName(p.name) || defaultPlanName(plans.length + 1, this.lang), thumbnail: cleanThumbnail(p.thumbnail), scene });
      }
      if (plans.length) seqs.push({ id: uid(sq.id, 's'), name: cleanPlanName(sq.name) || defaultSequenceName(seqs.length + 1, this.lang), plans });
    }
    let cur = null;
    for (const sq of seqs) for (const p of sq.plans) if (!cur && p.id === current?.plan && !p.scene) cur = { sq, p };
    for (const sq of seqs) for (const p of sq.plans) if (!cur && !p.scene) cur = { sq, p };
    if (!cur) {
      // aucune place pour la scène du fichier (projet d'avant les séquences) : un plan pour elle
      if (!seqs.length) seqs.push({ id: newId('s'), name: defaultSequenceName(1, this.lang), plans: [] });
      const p = { id: newId('p'), name: defaultPlanName(seqs[0].plans.length + 1, this.lang), thumbnail: null, scene: null };
      seqs[0].plans.unshift(p);
      cur = { sq: seqs[0], p };
    }
    // fichier abîmé : un autre plan sans scène repart vide
    for (const sq of seqs) for (const p of sq.plans) if (p !== cur.p && !p.scene) p.scene = emptyScene(this.lang);
    this.sequences = seqs;
    this.current = { seq: cur.sq.id, plan: cur.p.id };
  }

  /** Pour le fichier : la scène du plan courant est au premier niveau (scene: null ici). */
  _plansForFile() {
    const withoutName = ({ name, ...scene }) => (void name, scene);
    return {
      current: { ...this.current },
      sequences: this.sequences.map((sq) => ({
        id: sq.id,
        name: sq.name,
        plans: sq.plans.map((p) => ({ id: p.id, name: p.name, ...(p.thumbnail ? { thumbnail: p.thumbnail } : {}), scene: p.scene ? withoutName(p.scene) : null })),
      })),
    };
  }

  /** Résumé pour les interfaces (sans les scènes des autres plans). */
  planInfo() {
    return {
      current: { ...this.current },
      sequences: this.sequences.map((sq) => ({
        id: sq.id,
        name: sq.name,
        plans: sq.plans.map((p) => {
          const sc = p.scene || this.scene;
          return { id: p.id, name: p.name, thumbnail: p.thumbnail, fixtures: sc.fixtures?.length || 0, objects: sc.objects?.length || 0 };
        }),
      })),
    };
  }

  _findPlan(id) {
    for (const seq of this.sequences) for (const plan of seq.plans) if (plan.id === id) return { seq, plan };
    return null;
  }

  _currentPlan() {
    return this._findPlan(this.current.plan);
  }

  _flatPlans() {
    return this.sequences.flatMap((seq) => seq.plans.map((plan) => ({ seq, plan })));
  }

  /** Vignette d'un plan (image de la vue 3D quand on le quitte). */
  setPlanThumbnail(id, thumbnail) {
    const r = this._findPlan(id);
    const t = cleanThumbnail(thumbnail);
    if (!r || !t) return false;
    r.plan.thumbnail = t;
    this._touch();
    return true;
  }

  /**
   * Passe à un autre plan : la scène vivante devient la sienne. Le nom du projet et le cartouche
   * (production, chef électro) suivent : ils appartiennent au projet, pas au plan.
   * Changer de plan n'est pas une modification (rien de neuf à enregistrer).
   */
  switchPlan(id, { thumbnail = null } = {}) {
    const to = this._findPlan(id);
    if (!to) throw new Error('Plan introuvable');
    const from = this._currentPlan();
    if (to.plan === from.plan) return false;
    const t = cleanThumbnail(thumbnail);
    if (t) from.plan.thumbnail = t;
    const prev = this.scene;
    from.plan.scene = prev;
    this.scene = this._sanitize(to.plan.scene || emptyScene(this.lang));
    to.plan.scene = null;
    this.scene.name = prev.name;
    const title = prev.plot?.title || {};
    this.scene.plot.title = { ...this.scene.plot.title, production: title.production || '', author: title.author || '' };
    this.current = { seq: to.seq.id, plan: to.plan.id };
    this._touch();
    return true;
  }

  /** Scène de départ d'un nouveau plan : copie du plan courant, même décor sans lumière, ou vide. */
  _startScene(mode) {
    if (mode === 'copy') return structuredClone(this.scene);
    const s = emptyScene(this.lang);
    if (mode === 'decor') {
      // le décor (meubles, véhicules, personnages…) sans projecteurs ni grip
      s.objects = structuredClone(this.scene.objects.filter((o) => isProp(getObjectType(o.type))));
      s.environment = structuredClone(this.scene.environment);
      s.plot.underlay = structuredClone(this.scene.plot.underlay);
      s.plot.background = this.scene.plot.background;
    }
    return s;
  }

  /** Nouveau plan juste après le plan courant (ou à la fin d'une autre séquence) ; il devient le plan courant. */
  newPlan({ seq = null, mode = 'copy', name = '', thumbnail = null } = {}) {
    const cur = this._currentPlan();
    const target = (seq && this.sequences.find((s) => s.id === seq)) || cur.seq;
    const at = target === cur.seq ? target.plans.indexOf(cur.plan) + 1 : target.plans.length;
    const n = nextNumber(target.plans.map((p) => p.name), target.plans.length);
    const plan = { id: newId('p'), name: cleanPlanName(name) || defaultPlanName(n, this.lang), thumbnail: null, scene: this._startScene(mode) };
    target.plans.splice(at, 0, plan);
    this.switchPlan(plan.id, { thumbnail });
    this._changed();
    return plan;
  }

  /** Nouvelle séquence (un premier plan) juste après la séquence courante ; elle devient courante. */
  newSequence({ mode = 'empty', name = '', thumbnail = null } = {}) {
    const cur = this._currentPlan();
    const plan = { id: newId('p'), name: defaultPlanName(1, this.lang), thumbnail: null, scene: this._startScene(mode) };
    const n = nextNumber(this.sequences.map((s) => s.name), this.sequences.length);
    const seq = { id: newId('s'), name: cleanPlanName(name) || defaultSequenceName(n, this.lang), plans: [plan] };
    this.sequences.splice(this.sequences.indexOf(cur.seq) + 1, 0, seq);
    this.switchPlan(plan.id, { thumbnail });
    this._changed();
    return seq;
  }

  /** Copie d'un plan, placée juste après lui (on reste sur le plan courant). */
  duplicatePlan(id) {
    const r = this._findPlan(id);
    if (!r) return null;
    const name = cleanPlanName(translate('{0} (copie)', this.lang, { 0: r.plan.name }));
    const copy = { id: newId('p'), name, thumbnail: r.plan.thumbnail, scene: structuredClone(r.plan.scene || this.scene) };
    r.seq.plans.splice(r.seq.plans.indexOf(r.plan) + 1, 0, copy);
    this._changed();
    return copy;
  }

  renamePlan(id, name) {
    const r = this._findPlan(id);
    const n = cleanPlanName(name);
    if (!r || !n || n === r.plan.name) return false;
    r.plan.name = n;
    this._changed();
    return true;
  }

  renameSequence(id, name) {
    const sq = this.sequences.find((s) => s.id === id);
    const n = cleanPlanName(name);
    if (!sq || !n || n === sq.name) return false;
    sq.name = n;
    this._changed();
    return true;
  }

  /** Supprime un plan (pas le dernier du projet) ; une séquence vidée disparaît avec lui. */
  removePlan(id) {
    const r = this._findPlan(id);
    if (!r) return false;
    const all = this._flatPlans();
    if (all.length <= 1) throw new Error('Le projet garde au moins un plan');
    if (r.plan.id === this.current.plan) {
      const i = all.findIndex((x) => x.plan === r.plan);
      this.switchPlan((all[i + 1] || all[i - 1]).plan.id);
    }
    r.seq.plans.splice(r.seq.plans.indexOf(r.plan), 1);
    if (!r.seq.plans.length) this.sequences.splice(this.sequences.indexOf(r.seq), 1);
    this._changed();
    return true;
  }

  /** Supprime une séquence et ses plans (pas la dernière du projet). */
  removeSequence(id) {
    const sq = this.sequences.find((s) => s.id === id);
    if (!sq) return false;
    if (this.sequences.length <= 1) throw new Error('Le projet garde au moins une séquence');
    if (sq.plans.some((p) => p.id === this.current.plan)) {
      const i = this.sequences.indexOf(sq);
      this.switchPlan((this.sequences[i + 1] || this.sequences[i - 1]).plans[0].id);
    }
    this.sequences.splice(this.sequences.indexOf(sq), 1);
    this._changed();
    return true;
  }

  /** Déplace un plan : à une autre place de sa séquence, ou dans une autre séquence. */
  movePlan(id, { seq = null, index = null } = {}) {
    const r = this._findPlan(id);
    const target = seq ? this.sequences.find((s) => s.id === seq) : r?.seq;
    if (!r || !target) return false;
    r.seq.plans.splice(r.seq.plans.indexOf(r.plan), 1);
    const i = Math.max(0, Math.min(target.plans.length, Number.isInteger(index) ? index : target.plans.length));
    target.plans.splice(i, 0, r.plan);
    if (!r.seq.plans.length) this.sequences.splice(this.sequences.indexOf(r.seq), 1);
    if (r.plan.id === this.current.plan) this.current.seq = target.id;
    this._changed();
    return true;
  }

  /** Déplace une séquence à une autre place. */
  moveSequence(id, index) {
    const sq = this.sequences.find((s) => s.id === id);
    if (!sq || !Number.isInteger(index)) return false;
    const from = this.sequences.indexOf(sq);
    const to = Math.max(0, Math.min(this.sequences.length - 1, index));
    if (from === to) return false;
    this.sequences.splice(from, 1);
    this.sequences.splice(to, 0, sq);
    this._changed();
    return true;
  }
}
