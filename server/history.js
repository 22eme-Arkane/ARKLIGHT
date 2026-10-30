// Annuler / rétablir : un instantané de la scène avant chaque modification (100 étapes au plus).
import { getObjectType, isProp } from '../shared/objects.js';

// Un geste continu (glisser une poignée, un curseur, une forme du plan) ne fait qu'une étape :
// les messages de même nature sur les mêmes éléments, à moins de MERGE_MS d'intervalle, sont fusionnés.

const MERGE_MS = 1200;

/** Nature d'un message : « fixture.update|f12|position,rotation ». Null : jamais fusionné. */
export function historyKey(msg) {
  const keys = (changes) =>
    Object.entries(changes && typeof changes === 'object' ? changes : {})
      .flatMap(([k, v]) => (v && typeof v === 'object' && !Array.isArray(v) ? Object.keys(v).map((s) => `${k}.${s}`) : [k]))
      .sort()
      .join(',');
  switch (msg.t) {
    case 'fixture.update':
    case 'object.update':
    case 'plot.update':
      return `${msg.t}|${msg.id}|${keys(msg.changes)}`;
    case 'fixtures.update': {
      const items = Array.isArray(msg.items) ? msg.items : [];
      return `${msg.t}|${items.map((i) => i.id).join(',')}|${[...new Set(items.map((i) => keys(i.changes)))].join(';')}`;
    }
    case 'plot.meta':
      return `${msg.t}|${keys({ underlay: msg.underlay ?? undefined, title: msg.title ?? undefined, background: msg.background === undefined ? undefined : 1 })}`;
    case 'group.update':
      return typeof msg.name === 'string' ? `${msg.t}|${msg.id}|name` : null;
    default:
      return null;
  }
}

/** Ce que l'étape annule, pour le menu : « Annuler le déplacement ». */
export function historyLabel(msg) {
  const n = msg.t === 'fixtures.add' ? (msg.items || []).length : msg.t === 'fixtures.remove' ? (msg.ids || []).length : 1;
  const changeKinds = (changes) => Object.keys(changes && typeof changes === 'object' ? changes : {});
  const fixtureChange = (list) => {
    const kinds = new Set(list.flatMap(changeKinds));
    const settings = list.flatMap((c) => changeKinds(c?.settings));
    if ([...kinds].every((k) => k === 'position' || k === 'rotation')) return 'le déplacement';
    if (kinds.size === 1 && kinds.has('name')) return 'le changement de nom';
    if (settings.length && settings.every((k) => k === 'dmxAddress' || k === 'universe')) return 'l’adressage DMX';
    return 'le réglage du projecteur';
  };
  switch (msg.t) {
    case 'fixture.add':
      return 'l’ajout du projecteur';
    case 'fixtures.add':
      return n > 1 ? `l’ajout de ${n} projecteurs` : 'l’ajout du projecteur';
    case 'fixture.remove':
      return 'la suppression du projecteur';
    case 'fixtures.remove':
      return n > 1 ? `la suppression de ${n} projecteurs` : 'la suppression du projecteur';
    case 'fixture.update':
      return fixtureChange([msg.changes]);
    case 'fixtures.update':
      return fixtureChange((msg.items || []).map((i) => i.changes));
    case 'object.add':
      return isProp(getObjectType(msg.type)) ? 'l’ajout de l’objet' : 'l’ajout du grip';
    case 'object.update': {
      const kinds = changeKinds(msg.changes);
      if (kinds.every((k) => k === 'position' || k === 'rotation')) return 'le déplacement';
      return kinds.length === 1 && kinds[0] === 'name' ? 'le changement de nom' : 'le réglage de l’objet';
    }
    case 'object.remove':
      return 'la suppression de l’objet';
    case 'plot.add':
      return 'l’ajout au plan de feu';
    case 'plot.update':
      return 'la modification du plan de feu';
    case 'plot.remove':
      return 'la suppression dans le plan de feu';
    case 'plot.meta':
      return msg.title ? 'le cartouche du plan' : msg.background !== undefined ? 'la couleur du fond du plan' : 'l’image de fond du plan';
    case 'group.create':
      return 'la création du groupe';
    case 'group.update':
      return 'la modification du groupe';
    case 'group.delete':
      return 'la suppression du groupe';
    case 'ai.action':
      return 'l’action de l’assistant';
    default:
      return 'la modification';
  }
}

/** Messages qui modifient la scène et qu'on peut annuler. */
export const UNDOABLE = new Set([
  'fixture.add',
  'fixture.update',
  'fixture.remove',
  'fixtures.add',
  'fixtures.update',
  'fixtures.remove',
  'object.add',
  'object.update',
  'object.remove',
  'plot.add',
  'plot.update',
  'plot.remove',
  'plot.meta',
  'group.create',
  'group.update',
  'group.delete',
]);

export class History {
  constructor({ limit = 100, mergeMs = MERGE_MS } = {}) {
    this.limit = limit;
    this.mergeMs = mergeMs;
    this.clear();
  }

  clear() {
    this.undoStack = [];
    this.redoStack = [];
    this.last = null; // dernière étape enregistrée : { key, at } (fusion des gestes continus)
  }

  /** Avant une modification : prépare l'étape (rien à copier si elle prolonge le geste en cours). */
  begin(scene, revision, msg, now = Date.now()) {
    const key = historyKey(msg);
    if (key && this.last?.key === key && now - this.last.at < this.mergeMs) return { merge: true, key, now };
    return { merge: false, key, now, revision, json: JSON.stringify(scene), label: historyLabel(msg) };
  }

  /**
   * Après la modification. Renvoie 'merged' (geste en cours), 'pushed' (nouvelle étape) ou 'noop'
   * (rien n'a changé : l'appelant remet le numéro de révision, la scène reste « enregistrée »).
   */
  end(step, scene, revision) {
    if (step.merge) {
      this.last.at = step.now;
      return 'merged';
    }
    if (revision === step.revision || JSON.stringify(scene) === step.json) return 'noop';
    this.undoStack.push({ json: step.json, label: step.label, revision: step.revision });
    if (this.undoStack.length > this.limit) this.undoStack.shift();
    this.redoStack.length = 0;
    this.last = { key: step.key, at: step.now };
    return 'pushed';
  }

  /** Renvoie l'état à restaurer ({ json, revision }) et garde l'état actuel pour « Rétablir ». */
  undo(scene, revision) {
    const e = this.undoStack.pop();
    if (!e) return null;
    this.redoStack.push({ json: JSON.stringify(scene), label: e.label, revision });
    this.last = null;
    return e;
  }

  redo(scene, revision) {
    const e = this.redoStack.pop();
    if (!e) return null;
    this.undoStack.push({ json: JSON.stringify(scene), label: e.label, revision });
    this.last = null;
    return e;
  }

  status() {
    return {
      canUndo: this.undoStack.length > 0,
      canRedo: this.redoStack.length > 0,
      undoLabel: this.undoStack.at(-1)?.label || '',
      redoLabel: this.redoStack.at(-1)?.label || '',
    };
  }
}
