// Projets ARKLIGHT : un projet = un fichier .json (la scène, sa vignette et la date d'enregistrement).
// Format 2 : la scène du plan courant au premier niveau (lisible par les versions précédentes),
// les autres plans dans « sequences » (voir shared/plans.js).
// Le dossier des projets se choisit (disque externe, NAS…) ; les projets ouverts ou enregistrés
// ailleurs restent dans la liste des récents. Changer de dossier ne déplace aucun projet.
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, renameSync, statSync, unlinkSync, writeFileSync } from 'node:fs';
import { basename, dirname, extname, join, resolve } from 'node:path';

export const PROJECT_EXT = '.json';
const MAX_BYTES = 40 * 1024 * 1024;
const MAX_RECENTS = 30;
const MAX_THUMB = 1.5 * 1024 * 1024;

/** Nom de fichier sûr (Windows) : sans caractères interdits, ni point en tête. */
export const safeName = (name) =>
  String(name || '')
    .replace(/[<>:"/\\|?*\x00-\x1f]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/^\.+/, '')
    .slice(0, 80);

/** Nom d'un projet : celui de son fichier. */
export const projectName = (path) => basename(path, extname(path));

/** Vignette acceptée : image JPEG, PNG ou WebP en data URL, pas trop lourde. */
export function cleanThumbnail(t) {
  return typeof t === 'string' && /^data:image\/(jpeg|png|webp);base64,[A-Za-z0-9+/=]+$/.test(t) && t.length <= MAX_THUMB ? t : null;
}

/** Contenu reconnu comme une scène ARKLIGHT. */
export function isProjectData(data) {
  return !!data && typeof data === 'object' && !Array.isArray(data) && Array.isArray(data.fixtures) && (data.app === 'ARKLIGHT' || (!!data.environment && typeof data.environment === 'object'));
}

/**
 * Lit un fichier de projet : { scene (brute, celle du plan courant), thumbnail, savedAt, session,
 * sequences, current }, ou null si ce n'en est pas un.
 */
export function readProjectFile(path) {
  const st = statSync(path);
  if (!st.isFile() || st.size > MAX_BYTES) return null;
  const data = JSON.parse(readFileSync(path, 'utf8'));
  if (!isProjectData(data)) return null;
  const { thumbnail, app, format, savedAt, _session, sequences, current, ...scene } = data;
  void app;
  void format;
  return { scene, thumbnail: cleanThumbnail(thumbnail), savedAt: typeof savedAt === 'string' ? savedAt : null, session: _session, sequences: Array.isArray(sequences) ? sequences : null, current: current && typeof current === 'object' ? current : null };
}

/** Écrit un fichier de projet (d'abord à côté, puis remplacement : pas de fichier à moitié écrit). */
export function writeProjectFile(path, scene, { thumbnail = null, extra = {} } = {}) {
  mkdirSync(dirname(path), { recursive: true });
  const data = { app: 'ARKLIGHT', format: 2, savedAt: new Date().toISOString(), ...scene, ...extra, ...(thumbnail ? { thumbnail } : {}) };
  const json = JSON.stringify(data, null, 2);
  const tmp = `${path}.tmp`;
  writeFileSync(tmp, json);
  try {
    renameSync(tmp, path);
  } catch {
    // fichier ouvert ailleurs (antivirus, synchronisation) : écriture directe
    writeFileSync(path, json);
    try {
      unlinkSync(tmp);
    } catch {
      /* rien */
    }
  }
}

export class ProjectLibrary {
  /**
   * @param {object} o
   * @param {string} o.defaultDir  dossier des projets par défaut (Documents\ARKLIGHT\scenes)
   * @param {object} o.config      réglages (config.json) : projectsDir, recentProjects
   * @param {Function} [o.onConfig] enregistre les réglages
   */
  constructor({ defaultDir, config = {}, onConfig = () => {} }) {
    this.defaultDir = defaultDir;
    this.config = config;
    this.onConfig = onConfig;
    this.cache = new Map(); // chemin → { mtimeMs, size, meta }
    this.known = new Set(); // fichiers montrés à l'interface ou choisis dans une fenêtre du système
    this.trash = null; // corbeille du système (application de bureau) : (chemin) => Promise
  }

  /** Dossier où naissent les nouveaux projets. */
  get root() {
    const p = this.config.projectsDir;
    return typeof p === 'string' && p.trim() ? p : this.defaultDir;
  }

  get isDefault() {
    return resolve(this.root) === resolve(this.defaultDir);
  }

  /** Le dossier existe vraiment (un disque externe peut être débranché). */
  get available() {
    try {
      return statSync(this.root).isDirectory();
    } catch {
      return false;
    }
  }

  setRoot(dir) {
    const p = resolve(String(dir || ''));
    let ok = false;
    try {
      ok = !!dir && statSync(p).isDirectory();
    } catch {
      ok = false;
    }
    if (!ok) throw new Error('Dossier introuvable');
    if (p === resolve(this.defaultDir)) delete this.config.projectsDir;
    else this.config.projectsDir = p;
    this.onConfig();
    return this.root;
  }

  get recents() {
    return Array.isArray(this.config.recentProjects) ? this.config.recentProjects.filter((p) => typeof p === 'string') : [];
  }

  addRecent(path) {
    const p = resolve(path);
    this.known.add(p);
    this.config.recentProjects = [p, ...this.recents.filter((x) => resolve(x) !== p)].slice(0, MAX_RECENTS);
    this.onConfig();
  }

  /** Retire de la liste des récents (le fichier reste sur le disque). */
  forget(path) {
    const p = resolve(path);
    this.config.recentProjects = this.recents.filter((x) => resolve(x) !== p);
    this.onConfig();
  }

  /** Fichier montré à l'interface (seuls ceux-là s'ouvrent, se renomment ou se suppriment par leur chemin). */
  isKnown(path) {
    return typeof path === 'string' && this.known.has(resolve(path));
  }

  /** Chemin choisi par l'utilisateur dans une fenêtre du système (application de bureau). */
  allow(path) {
    this.known.add(resolve(path));
  }

  /** Résumé d'un fichier de projet (mis en cache tant qu'il ne change pas), ou null. */
  meta(path) {
    let st;
    try {
      st = statSync(path);
    } catch {
      return null;
    }
    if (!st.isFile() || st.size > MAX_BYTES) return null;
    const c = this.cache.get(path);
    if (c && c.mtimeMs === st.mtimeMs && c.size === st.size) return c.meta;
    let meta = null;
    try {
      const r = readProjectFile(path);
      if (r) {
        meta = {
          path,
          name: projectName(path),
          folder: dirname(path),
          modifiedMs: st.mtimeMs,
          modifiedAt: new Date(st.mtimeMs).toISOString(),
          fixtures: r.scene.fixtures.length,
          objects: Array.isArray(r.scene.objects) ? r.scene.objects.length : 0,
          sequences: r.sequences?.length || 1,
          plans: r.sequences?.reduce((n, sq) => n + (Array.isArray(sq?.plans) ? sq.plans.length : 0), 0) || 1,
          thumbnail: r.thumbnail,
        };
      }
    } catch {
      meta = null;
    }
    this.cache.set(path, { mtimeMs: st.mtimeMs, size: st.size, meta });
    return meta;
  }

  /** Projets du dossier, puis récents ouverts d'ailleurs ; les plus récemment modifiés d'abord. */
  list() {
    const seen = new Set();
    const items = [];
    const add = (p, inRoot) => {
      const r = resolve(p);
      if (seen.has(r)) return;
      seen.add(r);
      const m = this.meta(r);
      if (!m) return;
      this.known.add(r);
      items.push({ ...m, inRoot });
    };
    if (this.available) {
      for (const f of readdirSync(this.root)) {
        if (f.toLowerCase().endsWith(PROJECT_EXT) && !f.startsWith('_')) add(join(this.root, f), true);
      }
    }
    for (const p of this.recents) add(p, resolve(dirname(p)) === resolve(this.root));
    return items.sort((a, b) => b.modifiedMs - a.modifiedMs);
  }

  /** Chemin d'un projet nommé dans un dossier. */
  target(folder, name) {
    const n = safeName(name);
    if (!n || n.startsWith('_')) throw new Error('Nom de projet invalide');
    return join(folder, `${n}${PROJECT_EXT}`);
  }

  _check(path) {
    if (!this.isKnown(path)) throw new Error('Projet inconnu : rouvre la liste des projets');
    return resolve(path);
  }

  rename(path, name) {
    const from = this._check(path);
    const to = this.target(dirname(from), name);
    if (resolve(to) === from) return from;
    // Windows ne distingue pas les majuscules : « essai » → « Essai » est le même fichier
    if (existsSync(to) && resolve(to).toLowerCase() !== from.toLowerCase()) throw new Error(`Un projet « ${projectName(to)} » existe déjà dans ce dossier`);
    renameSync(from, to);
    this._moved(from, to);
    return to;
  }

  duplicate(path, name) {
    const from = this._check(path);
    const to = this.target(dirname(from), name);
    if (existsSync(to)) throw new Error(`Un projet « ${projectName(to)} » existe déjà dans ce dossier`);
    copyFileSync(from, to);
    this.allow(to);
    if (resolve(dirname(to)) !== resolve(this.root)) this.addRecent(to);
    return to;
  }

  /** Met le fichier à la corbeille du système (application de bureau), sinon dans un dossier « _corbeille ». */
  async trashFile(path) {
    const p = this._check(path);
    if (this.trash) await this.trash(p);
    else {
      const bin = join(dirname(p), '_corbeille');
      mkdirSync(bin, { recursive: true });
      renameSync(p, join(bin, `${projectName(p)} (${new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')})${PROJECT_EXT}`));
    }
    this.known.delete(p);
    this.cache.delete(p);
    this.forget(p);
  }

  _moved(from, to) {
    this.known.delete(from);
    this.cache.delete(from);
    this.known.add(resolve(to));
    if (this.recents.some((x) => resolve(x) === from)) {
      this.config.recentProjects = this.recents.map((x) => (resolve(x) === from ? resolve(to) : x));
      this.onConfig();
    }
  }
}
