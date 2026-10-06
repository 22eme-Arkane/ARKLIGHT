// Scène courante (projecteurs placés + environnement), sauvegarde automatique et fichiers de scène.
import { EventEmitter } from 'node:events';
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync, unlinkSync } from 'node:fs';
import { join } from 'node:path';
import { defaultSettings, defaultState, getFixtureType, modeFootprint } from '../shared/fixtures/index.js';
import { defaultGripProps, getGripType } from '../shared/grip.js';

const AUTOSAVE = '_autosave';
let revisionCounter = 0;
const nextRevision = () => ++revisionCounter;

export function emptyScene() {
  return {
    version: 1,
    name: 'Nouvelle scène',
    environment: { floor: true, cyc: true, actor: true, stands: true, labels: false, labelPatch: false, haze: 0.25, ambient: 0.04, exposure: 1 },
    fixtures: [],
    groups: [], // sélections nommées : { id, name, ids } (un projecteur peut être dans plusieurs groupes)
    objects: [], // grip et accessoires : { id, type, name, position, rotation, props }
    plot: emptyPlot(), // plan de feu 2D : formes, textes, flèches, image de fond
  };
}

export function emptyPlot() {
  return { items: [], underlay: null, title: { production: '', author: '', notes: '' } };
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

const safeName = (name) =>
  String(name || '')
    .replace(/[<>:"/\\|?*\x00-\x1f]/g, '')
    .trim()
    .slice(0, 80);

export class SceneStore extends EventEmitter {
  constructor(dir) {
    super();
    this.dir = dir;
    mkdirSync(dir, { recursive: true });
    // Révisions : chaque modification change le numéro ; la scène est « enregistrée » tant que le
    // numéro est celui du dernier enregistrement (une annulation peut y ramener).
    this.revision = nextRevision();
    this.savedRevision = this.revision;
    this.savedName = null; // fichier de scène correspondant (null : jamais enregistrée)
    this._timer = null;
    const auto = this._read(AUTOSAVE, true);
    this.scene = auto?.scene || emptyScene();
    if (auto?.session) {
      this.savedName = typeof auto.session.savedName === 'string' ? auto.session.savedName : null;
      if (auto.session.dirty) this.savedRevision = -1;
    }
  }

  /** Modifications non enregistrées. */
  get dirty() {
    return this.revision !== this.savedRevision;
  }

  _path(name) {
    return join(this.dir, `${name}.json`);
  }

  _read(name, withSession = false) {
    try {
      const scene = JSON.parse(readFileSync(this._path(name), 'utf8'));
      const session = scene._session; // sauvegarde automatique : état « enregistrée ou non »
      delete scene._session;
      const clean = this._sanitize(scene);
      return withSession ? { scene: clean, session } : clean;
    } catch {
      return null;
    }
  }

  /** Ajoute les valeurs par défaut manquantes (scènes créées par une version antérieure). */
  _sanitize(scene) {
    const base = emptyScene();
    scene.environment = { ...base.environment, ...(scene.environment || {}) };
    scene.fixtures = (scene.fixtures || []).filter((f) => getFixtureType(f.type));
    for (const f of scene.fixtures) {
      const type = getFixtureType(f.type);
      const defaults = defaultSettings(type);
      f.settings = { ...defaults, ...(f.settings || {}) };
      // Mode DMX disparu (bibliothèque mise à jour) : retour au mode par défaut du modèle
      if (type.dmx?.modes?.length && !type.dmx.modes.some((m) => m.id === f.settings.dmxMode)) f.settings.dmxMode = defaults.dmxMode;
      f.state = deepMerge(defaultState(type), f.state || {});
    }
    scene.objects = (Array.isArray(scene.objects) ? scene.objects : []).filter((o) => o && o.id && getGripType(o.type));
    for (const o of scene.objects) {
      o.position = Array.isArray(o.position) ? o.position.map(Number) : [0, 0, 0];
      o.rotation = Array.isArray(o.rotation) ? o.rotation.map(Number) : [0, 0, 0];
      o.props = { ...defaultGripProps(getGripType(o.type)), ...(o.props || {}) };
    }
    const plot = scene.plot && typeof scene.plot === 'object' ? scene.plot : {};
    scene.plot = { ...emptyPlot(), ...plot, items: (plot.items || []).map((it) => cleanPlotItem(it)).filter(Boolean), title: { ...emptyPlot().title, ...(plot.title || {}) } };
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
    const data = name === AUTOSAVE ? { ...this.scene, _session: { savedName: this.savedName, dirty: this.dirty } } : this.scene;
    writeFileSync(this._path(name), JSON.stringify(data, null, 2));
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
    const { position, rotation, name, settings, state } = changes;
    if (Array.isArray(position) && position.length === 3) f.position = position.map(Number);
    if (Array.isArray(rotation) && rotation.length === 3) f.rotation = rotation.map(Number);
    if (typeof name === 'string') f.name = name.slice(0, 60);
    if (isPlainObject(settings)) deepMerge(f.settings, settings);
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
    const type = getGripType(typeId);
    if (!type) throw new Error(`Élément de grip inconnu : ${typeId}`);
    const count = this.scene.objects.filter((o) => o.type === typeId).length;
    const obj = {
      id: newId('o'),
      type: typeId,
      name: typeof name === 'string' && name.trim() ? name.trim().slice(0, 60) : `${type.short} ${count + 1}`,
      position: position.map(Number),
      rotation: rotation.map(Number),
      props: { ...defaultGripProps(type), ...(isPlainObject(props) ? props : {}) },
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
    if (isPlainObject(props)) deepMerge(o.props, props);
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

  /** Image de fond (calée à l'échelle) et cartouche du plan. */
  updatePlotMeta({ underlay, title } = {}) {
    const p = this.scene.plot;
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

  rename(name) {
    this.scene.name = safeName(name) || this.scene.name;
    this._changed();
  }

  newScene() {
    this.scene = emptyScene();
    this._changed();
    this.savedRevision = this.revision; // rien à enregistrer dans une scène vide
    this.savedName = null;
    return this.scene;
  }

  save(name) {
    const n = safeName(name || this.scene.name);
    if (!n || n.startsWith('_')) throw new Error('Nom de scène invalide');
    this.scene.name = n;
    this._write(n);
    this.savedRevision = this.revision;
    this.savedName = n;
    this._touch();
    return n;
  }

  /** Une scène de ce nom existe déjà dans le dossier des scènes. */
  exists(name) {
    const n = safeName(name);
    return !!n && existsSync(this._path(n));
  }

  load(name) {
    const scene = this._read(safeName(name));
    if (!scene) throw new Error(`Scène introuvable : ${name}`);
    this.scene = scene;
    this._changed();
    this.savedRevision = this.revision;
    this.savedName = scene.name = safeName(name);
    return scene;
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
    const p = this._path(safeName(name));
    if (existsSync(p)) unlinkSync(p);
  }

  list() {
    return readdirSync(this.dir)
      .filter((f) => f.endsWith('.json') && !f.startsWith('_'))
      .map((f) => f.slice(0, -5))
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
}
