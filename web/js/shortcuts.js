// Raccourcis clavier personnalisables, comme dans Premiere Pro ou DaVinci Resolve : recherche,
// arborescence des menus, clavier AZERTY qui montre les touches déjà prises, plusieurs touches
// par commande. Seules les commandes modifiées sont mémorisées (les nouvelles commandes d'une
// version future gardent leurs touches par défaut).
// Les chiffres sont reconnus par leur touche physique (rangée du haut ou pavé numérique) :
// sur un clavier AZERTY, « 1 » fonctionne sans Maj. Les lettres suivent la disposition du clavier.
import { COMMANDS, MENUS, commandById, menuById } from './commands.js';
import { h } from './ui/dom.js';

const STORE_KEY = 'arklight.shortcuts.v2'; // { id: [touches] } des commandes modifiées
const LEGACY_KEY = 'arklight.shortcuts'; // première version : une seule touche par action
const LEGACY_DEFAULTS = {
  'tool.translate': 'W',
  'tool.rotate': 'E',
  'view.frame': 'F',
  'snap.toggle': 'M',
  'labels.toggle': 'N',
  'remote.toggle': 'R',
  'view.persp': '1',
  'view.front': '2',
  'view.top': '3',
  'view.side': '4',
  'view.plot': 'P',
  'select.all': 'Ctrl+A',
  'select.none': 'Escape',
  'group.create': 'Ctrl+G',
  'edit.duplicate': 'Ctrl+D',
  'edit.delete': 'Delete',
  'scene.save': 'Ctrl+S',
  'shortcuts.open': 'K',
};
const KEYBOARD_KEY = 'arklight.shortcuts.keyboard';

const KEY_NAMES = {
  Shift: 'Maj',
  Delete: 'Suppr',
  Escape: 'Échap',
  Enter: 'Entrée',
  Space: 'Espace',
  Backspace: 'Retour arr.',
  ArrowUp: '↑',
  ArrowDown: '↓',
  ArrowLeft: '←',
  ArrowRight: '→',
  Home: 'Début',
  End: 'Fin',
  PageUp: 'Pg préc',
  PageDown: 'Pg suiv',
  Insert: 'Inser',
  Plus: '+',
  Ù: 'ù',
};
const MODS = ['Ctrl', 'Alt', 'Shift'];

/** Noms des touches dans une autre langue (« Maj » → « Shift »…), à appeler au démarrage. */
export function setKeyNames(names) {
  Object.assign(KEY_NAMES, names);
}

/** Combinaisons gardées par le navigateur : elles ne marchent que dans l'application installée. */
export const BROWSER_RESERVED = new Set(['Ctrl+N', 'Ctrl+T', 'Ctrl+W', 'Ctrl+Shift+N', 'Ctrl+Shift+T', 'Ctrl+Shift+W', 'Ctrl+Tab', 'Ctrl+Shift+Tab', 'F11']);

/** Touche d'un événement clavier, normalisée (« Ctrl+Shift+A », « 1 », « Delete »…). Null pour une touche de modification seule. */
export function comboOf(e) {
  if (['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'CapsLock', 'Dead', 'Unidentified', 'Process'].includes(e.key)) return null;
  let k = e.key;
  const digit = /^(Digit|Numpad)(\d)$/.exec(e.code || '');
  if (digit) k = digit[2];
  else if (k === ' ') k = 'Space';
  else if (k === '+') k = 'Plus';
  else if (k.length === 1) k = k.toUpperCase();
  const mods = [];
  if (e.ctrlKey || e.metaKey) mods.push('Ctrl');
  if (e.altKey) mods.push('Alt');
  // Maj compte seulement pour les touches spéciales et les lettres (pas pour « ? » ou « 1 » en AZERTY)
  if (e.shiftKey && (k.length > 1 || /[A-Z]/.test(k)) && !digit) mods.push('Shift');
  return [...mods, k].join('+');
}

/** Remet une combinaison dans l'ordre Ctrl, Alt, Maj (préférences écrites à la main ou anciennes). */
export function normalizeCombo(s) {
  if (typeof s !== 'string' || !s.trim()) return null;
  const parts = s.split('+').map((p) => p.trim());
  const alias = { ctrl: 'Ctrl', control: 'Ctrl', cmd: 'Ctrl', meta: 'Ctrl', alt: 'Alt', option: 'Alt', shift: 'Shift', maj: 'Shift' };
  const mods = new Set();
  let key = null;
  for (const p of parts) {
    if (!p) continue;
    const m = alias[p.toLowerCase()];
    if (m) mods.add(m);
    else key = p.length === 1 ? p.toUpperCase() : p;
  }
  if (!key) return null;
  return [...MODS.filter((m) => mods.has(m)), key].join('+');
}

/** Libellé lisible d'une touche (« Ctrl+D », « Suppr », « Ctrl+Maj+S »…). */
export const keyLabel = (combo) => (combo ? combo.split('+').map((p) => KEY_NAMES[p] || p).join('+') : '');

/**
 * Touches de chaque commande : celles modifiées par l'utilisateur d'abord (elles sont prioritaires),
 * puis les touches par défaut des autres commandes, sans doublon.
 */
export function resolveBindings(overrides = {}) {
  const map = new Map();
  const claimed = new Set();
  const take = (id, list) => {
    const ks = [...new Set(list.map(normalizeCombo).filter(Boolean))].filter((k) => !claimed.has(k));
    for (const k of ks) claimed.add(k);
    map.set(id, ks);
  };
  for (const c of COMMANDS) if (Array.isArray(overrides[c.id])) take(c.id, overrides[c.id]);
  for (const c of COMMANDS) if (!map.has(c.id)) take(c.id, c.keys);
  return new Map(COMMANDS.map((c) => [c.id, map.get(c.id)]));
}

/** Commandes dont les touches diffèrent des touches par défaut (ce qu'on mémorise). */
export function overridesOf(map) {
  const out = {};
  for (const c of COMMANDS) {
    const ks = map.get(c.id) || [];
    if (ks.join('|') !== c.keys.join('|')) out[c.id] = ks;
  }
  return out;
}

/** Préférences de la première version (toutes les actions, une touche chacune) → commandes modifiées. */
export function migrateLegacy(legacy) {
  const out = {};
  for (const [id, v] of Object.entries(legacy || {})) {
    if (!(id in LEGACY_DEFAULTS) || v === LEGACY_DEFAULTS[id] || !commandById.has(id)) continue;
    out[id] = v ? [v] : [];
  }
  return out;
}

function readJson(key) {
  try {
    const v = localStorage.getItem(key);
    return v === null ? null : JSON.parse(v);
  } catch {
    return null;
  }
}
function writeJson(key, value) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, JSON.stringify(value));
    return true;
  } catch {
    return false;
  }
}

function loadOverrides() {
  const saved = readJson(STORE_KEY);
  if (saved && typeof saved === 'object') return saved;
  const legacy = readJson(LEGACY_KEY);
  if (!legacy || typeof legacy !== 'object') return {};
  const migrated = migrateLegacy(legacy);
  writeJson(STORE_KEY, migrated);
  writeJson(LEGACY_KEY, null);
  return migrated;
}

const norm = (s) =>
  String(s)
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase();
const plural = (n, one, many) => `${n} ${n > 1 ? many : one}`;

// ------------------------------------------------------------------ clavier AZERTY (français)
// [touche, inscription, largeur] ; touche null : non attribuable ; 'gap' : espace ; 'mod:…' : modificateur
const K = (t, cap, w = 1, sub = '') => ({ t, cap, w, sub });
const GAP = (w) => ({ gap: true, w });
const MAIN_ROWS = [
  [K('Escape', 'Échap'), GAP(1), ...['F1', 'F2', 'F3', 'F4'].map((f) => K(f, f)), GAP(0.5), ...['F5', 'F6', 'F7', 'F8'].map((f) => K(f, f)), GAP(0.5), ...['F9', 'F10', 'F11', 'F12'].map((f) => K(f, f))],
  [K('²', '²'), ...[['1', '&'], ['2', 'é'], ['3', '"'], ['4', "'"], ['5', '('], ['6', '-'], ['7', 'è'], ['8', '_'], ['9', 'ç'], ['0', 'à']].map(([d, s]) => K(d, d, 1, s)), K(')', ')', 1, '°'), K('=', '=', 1, '+'), K('Backspace', '⌫', 2)],
  [K('Tab', 'Tab ↹', 1.5), ...'AZERTYUIOP'.split('').map((l) => K(l, l)), K(null, '^', 1, '¨'), K('$', '$', 1, '£'), K('Enter', 'Entrée', 1.5)],
  [K(null, 'Verr. Maj', 1.75), ...'QSDFGHJKLM'.split('').map((l) => K(l, l)), K('Ù', 'ù', 1, '%'), K('*', '*', 1, 'µ'), K('Enter', '↵', 1.25)],
  [K('mod:Shift', 'Maj ⇧', 1.25), K('<', '<', 1, '>'), ...'WXCVBN'.split('').map((l) => K(l, l)), K(',', ',', 1, '?'), K(';', ';', 1, '.'), K(':', ':', 1, '/'), K('!', '!', 1, '§'), K('mod:Shift', 'Maj ⇧', 2.75)],
  [K('mod:Ctrl', 'Ctrl', 1.5), K(null, 'Win', 1.25), K('mod:Alt', 'Alt', 1.25), K('Space', 'Espace', 6.5), K(null, 'Alt Gr', 1.25), K(null, 'Menu', 1.25), K('mod:Ctrl', 'Ctrl', 2)],
];
const NAV_ROWS = [[GAP(3)], [K('Insert', 'Inser'), K('Home', 'Début'), K('PageUp', 'Pg ↑')], [K('Delete', 'Suppr'), K('End', 'Fin'), K('PageDown', 'Pg ↓')], [GAP(3)], [GAP(1), K('ArrowUp', '↑'), GAP(1)], [K('ArrowLeft', '←'), K('ArrowDown', '↓'), K('ArrowRight', '→')]];

/** Combinaison montrée par une touche du clavier dessiné, selon les modificateurs choisis. */
function visualCombo(token, mods) {
  if (!token || token.startsWith('mod:')) return null;
  const isDigit = /^\d$/.test(token);
  const isPunct = token.length === 1 && !/[A-Z0-9]/.test(token);
  if (mods.Shift && isPunct) return null; // Maj change le caractère (« ù » → « % »)
  return [...(mods.Ctrl ? ['Ctrl'] : []), ...(mods.Alt ? ['Alt'] : []), ...(mods.Shift && !isDigit ? ['Shift'] : []), token].join('+');
}

// ------------------------------------------------------------------ gestionnaire
/** Traduction des textes des fenêtres natives (confirm), fournie par l'interface. */
let translateText = (s) => s;
export function setTranslator(fn) {
  translateText = fn;
}

export function createShortcuts({ desktop = false } = {}) {
  let map = resolveBindings(loadOverrides());
  let index = new Map(); // touche → commande
  const listeners = new Set();
  const visible = (c) => desktop || !c.desktopOnly;

  // état de la fenêtre de réglage
  let dlg = null;
  let ui = null;
  let capturing = null; // { id, index, live } : commande qui attend sa touche
  let captureEvent = null;
  let note = null; // message du bas : { text, kind, undo }
  let node = { menu: null, section: null }; // nœud choisi dans l'arborescence
  const collapsed = new Set();
  let filter = 'all'; // all | bound | free | modified
  let keyFilter = null; // touche cliquée sur le clavier dessiné
  const pickedMods = { Ctrl: false, Alt: false, Shift: false };
  let heldMods = null; // modificateurs tenus au clavier (aperçu, comme dans Premiere)
  let showKeyboard = readJson(KEYBOARD_KEY) !== false;

  const reindex = () => {
    index = new Map();
    for (const [id, ks] of map) for (const k of ks) if (!index.has(k)) index.set(k, id);
  };
  reindex();

  function commit() {
    reindex();
    if (!writeJson(STORE_KEY, overridesOf(map))) note = { text: 'Raccourcis non mémorisés : le stockage du navigateur est indisponible.', kind: 'warn' };
    for (const fn of listeners) fn();
  }

  const api = {
    /** Commande déclenchée par cet événement clavier, ou null. */
    match(e) {
      const c = comboOf(e);
      return c ? index.get(c) || null : null;
    },
    combo: comboOf,
    keys: (id) => map.get(id) || [],
    /** Première touche de la commande, lisible (pour les menus et les infobulles). */
    label: (id) => keyLabel((map.get(id) || [])[0]),
    onChange: (fn) => listeners.add(fn),
    open: (query) => openDialog(query),
    /** Petite fenêtre « appuie sur la nouvelle touche » (clic droit sur un bouton de la barre d'outils). */
    quickEdit: (id) => openQuick(id),
    /** Retire toutes les touches d'une commande. */
    clearKeys(id) {
      map.set(id, []);
      commit();
    },
    isReserved: (combo) => !desktop && BROWSER_RESERVED.has(combo),
  };

  // ---------------------------------------------------------------- fenêtre de réglage
  const isModified = (c) => (map.get(c.id) || []).join('|') !== c.keys.join('|');
  const commandsOn = (combo) => COMMANDS.filter((c) => visible(c) && (map.get(c.id) || []).includes(combo));
  const snapshot = () => new Map([...map].map(([id, ks]) => [id, [...ks]]));

  function build() {
    dlg = h('dialog', { class: 'dialog kbd' });
    document.body.append(dlg);
    const search = h('input', { class: 'kbd-search', type: 'search', placeholder: 'Rechercher une commande ou une touche (ex. « dupliquer », « Ctrl+D »)…', spellcheck: 'false' });
    search.addEventListener('input', () => renderBody());
    ui = {
      search,
      keyChip: h('span', { class: 'kbd-keychip hidden' }),
      filters: h('div', { class: 'kbd-seg' }),
      keyboardWrap: h('div', { class: 'kbd-keyboard' }),
      kbToggle: h('button', { class: 'kbd-link', onclick: () => ((showKeyboard = !showKeyboard), writeJson(KEYBOARD_KEY, showKeyboard), renderKeyboard()) }),
      tree: h('nav', { class: 'kbd-tree' }),
      list: h('div', { class: 'kbd-list' }),
      status: h('div', { class: 'kbd-status' }),
    };
    dlg.append(
      h(
        'header',
        { class: 'kbd-head' },
        h('div', { class: 'kbd-title' }, h('h3', {}, 'Raccourcis clavier'), h('span', { class: 'note' }, 'Clique sur « + » pour ajouter une touche, sur une touche pour la changer, sur × pour la retirer.')),
        h('button', { class: 'kbd-close', title: 'Fermer (Échap)', onclick: () => dlg.close() }, '✕'),
      ),
      h('div', { class: 'kbd-bar' }, h('div', { class: 'kbd-searchbox' }, h('span', { class: 'kbd-lens' }, '⌕'), search, ui.keyChip), ui.filters),
      ui.keyboardWrap,
      h('div', { class: 'kbd-body' }, ui.tree, ui.list),
      h(
        'footer',
        { class: 'kbd-foot' },
        ui.status,
        h(
          'button',
          {
            onclick: () => {
              if (!confirm(translateText('Remettre toutes les touches par défaut ? Tes raccourcis personnalisés seront effacés.'))) return;
              const before = snapshot();
              map = resolveBindings({});
              capturing = null;
              note = { text: 'Toutes les touches sont revenues par défaut.', kind: 'ok', undo: before };
              commit();
              render();
            },
          },
          'Touches par défaut',
        ),
        h('button', { class: 'primary', onclick: () => dlg.close() }, 'Fermer'),
      ),
    );

    // Saisie d'une touche. Écoute au niveau de la fenêtre, en phase de capture : le bouton cliqué
    // disparaît au rendu (le focus repart sur la page) et rien ne doit atteindre la vue 3D.
    window.addEventListener(
      'keydown',
      (e) => {
        if (!dlg.open) return;
        if (!capturing) {
          if (['Control', 'Shift', 'Alt'].includes(e.key) && document.activeElement !== search) {
            heldMods = { Ctrl: e.ctrlKey, Alt: e.altKey, Shift: e.shiftKey };
            renderKeyboard();
          }
          return;
        }
        e.preventDefault();
        e.stopImmediatePropagation();
        if (e.key === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey) return cancelCapture();
        const combo = comboOf(e);
        if (!combo) {
          const live = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift'].filter(Boolean);
          capturing.live = live.length ? `${keyLabel(live.join('+'))}+…` : '';
          renderList();
          return;
        }
        assign(capturing.id, capturing.index, combo);
      },
      true,
    );
    window.addEventListener('keyup', (e) => {
      if (!dlg.open) return;
      if (capturing) {
        if (['Control', 'Shift', 'Alt'].includes(e.key)) {
          const live = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift'].filter(Boolean);
          capturing.live = live.length ? `${keyLabel(live.join('+'))}+…` : '';
          renderList();
        }
        return;
      }
      if (heldMods && ['Control', 'Shift', 'Alt'].includes(e.key)) {
        heldMods = e.ctrlKey || e.altKey || e.shiftKey ? { Ctrl: e.ctrlKey, Alt: e.altKey, Shift: e.shiftKey } : null;
        renderKeyboard();
      }
    });
    // Échap pendant une saisie : annule la saisie, pas la fenêtre
    dlg.addEventListener('cancel', (e) => {
      if (capturing) {
        e.preventDefault();
        cancelCapture();
      }
    });
    // Un clic ailleurs annule la saisie en cours
    dlg.addEventListener('click', (e) => {
      if (capturing && e !== captureEvent && !e.target.closest('.kbd-capture')) cancelCapture();
    });
    dlg.addEventListener('close', () => {
      capturing = null;
      heldMods = null;
    });
  }

  function openDialog(query) {
    if (!dlg) build();
    note = null;
    if (typeof query === 'string') ui.search.value = query;
    render();
    if (!dlg.open) dlg.showModal();
    ui.search.focus();
    ui.search.select();
  }

  // ---- modifications
  function startCapture(id, i, e) {
    capturing = { id, index: i, live: '' };
    captureEvent = e;
    note = { text: 'Appuie sur la combinaison voulue (Ctrl, Alt et Maj sont possibles). Échap : annuler.', kind: 'info' };
    renderList();
    renderStatus();
  }
  function cancelCapture() {
    capturing = null;
    note = null;
    renderList();
    renderStatus();
  }
  /** Donne la touche à la commande (au rang i) en la retirant des autres. À suivre de commit(). */
  function setKey(id, i, combo) {
    const before = snapshot();
    const takenBy = COMMANDS.filter((o) => o.id !== id && (map.get(o.id) || []).includes(combo));
    for (const o of takenBy) map.set(o.id, map.get(o.id).filter((k) => k !== combo));
    const ks = [...(map.get(id) || [])];
    if (!ks.includes(combo)) {
      if (i < ks.length) ks[i] = combo;
      else ks.push(combo);
    }
    map.set(id, ks);
    return { before, takenBy };
  }
  /** Touches par défaut d'une commande (retirées des commandes qui les avaient prises). À suivre de commit(). */
  function setDefaultKeys(id) {
    const c = commandById.get(id);
    const before = snapshot();
    const freed = [];
    for (const k of c.keys) {
      for (const o of COMMANDS) {
        if (o.id === id || !(map.get(o.id) || []).includes(k)) continue;
        map.set(o.id, map.get(o.id).filter((x) => x !== k));
        freed.push(`${keyLabel(k)} (« ${o.label} »)`);
      }
    }
    map.set(id, [...c.keys]);
    return { before, freed };
  }
  function assign(id, i, combo) {
    const c = commandById.get(id);
    const { before, takenBy } = setKey(id, i, combo);
    capturing = null;
    if (takenBy.length) note = { text: `${keyLabel(combo)} servait à « ${takenBy.map((o) => o.label).join(' », « ')} » : la touche passe à « ${c.label} ».`, kind: 'warn', undo: before };
    else if (api.isReserved(combo)) note = { text: `${keyLabel(combo)} est gardé par le navigateur : ce raccourci marchera dans l’application installée.`, kind: 'warn' };
    else note = { text: `« ${c.label} » : ${keyLabel(combo)}`, kind: 'ok', undo: before };
    commit();
    render();
  }
  function removeKey(id, i) {
    const before = snapshot();
    const ks = [...(map.get(id) || [])];
    const [k] = ks.splice(i, 1);
    map.set(id, ks);
    note = { text: `${keyLabel(k)} retiré de « ${commandById.get(id).label} ».`, kind: 'ok', undo: before };
    commit();
    render();
  }
  function resetCommand(id) {
    const c = commandById.get(id);
    const { before, freed } = setDefaultKeys(id);
    note = { text: `« ${c.label} » : touches par défaut${freed.length ? ` — retiré de ${freed.join(', ')}` : ''}.`, kind: freed.length ? 'warn' : 'ok', undo: before };
    commit();
    render();
  }

  // ---- changement rapide : clic droit sur un bouton › Changer le raccourci
  function openQuick(id) {
    const c = commandById.get(id);
    if (!c) return;
    document.querySelector('dialog.kbd-quick')?.close();
    let undo = null;
    const keysEl = h('div', { class: 'kbq-keys' });
    const zone = h('div', { class: 'kbq-zone' }, 'Appuie sur la nouvelle touche…');
    const msg = h('div', { class: 'kbq-msg' }, 'Ctrl, Alt et Maj sont possibles. Échap : fermer sans rien changer.');
    const say = (text, kind = '') => {
      msg.textContent = text;
      msg.className = `kbq-msg ${kind}`;
    };
    const showKeys = () => {
      const ks = map.get(id) || [];
      keysEl.replaceChildren(...(ks.length ? ks.map((k) => h('span', { class: `kbd-chip${api.isReserved(k) ? ' reserved' : ''}` }, h('span', { class: 'kbd-chip-key' }, keyLabel(k)))) : [h('span', { class: 'note' }, 'aucun')]));
    };
    const undoBtn = h(
      'button',
      {
        class: 'hidden',
        onclick: () => {
          if (!undo) return;
          map = undo;
          undo = null;
          commit();
          showKeys();
          zone.textContent = 'Appuie sur la nouvelle touche…';
          say('Modification annulée.', 'ok');
          undoBtn.classList.add('hidden');
        },
      },
      'Annuler la modification',
    );
    const changed = (before, text, kind) => {
      undo = before;
      commit();
      showKeys();
      say(text, kind);
      undoBtn.classList.remove('hidden');
    };
    const dlgQ = h(
      'dialog',
      { class: 'dialog kbd-quick' },
      h('h3', {}, 'Changer le raccourci'),
      h('div', { class: 'kbq-cmd' }, h('b', {}, c.label), h('span', { class: 'note' }, ` · ${menuById.get(c.menu).label} › ${c.section}`)),
      h('div', { class: 'kbq-row' }, h('span', { class: 'note' }, 'Touches actuelles'), keysEl),
      zone,
      msg,
      h(
        'div',
        { class: 'dialog-actions' },
        h(
          'button',
          {
            title: 'Plus aucune touche pour cette commande',
            onclick: () => {
              const before = snapshot();
              map.set(id, []);
              changed(before, `« ${c.label} » n’a plus de raccourci.`, 'ok');
            },
          },
          'Retirer',
        ),
        h(
          'button',
          {
            title: c.keys.length ? `Touche d’origine : ${c.keys.map(keyLabel).join(', ')}` : 'Pas de touche d’origine',
            onclick: () => {
              const { before, freed } = setDefaultKeys(id);
              changed(before, `Touches par défaut${freed.length ? ` — retiré de ${freed.join(', ')}` : ''}.`, freed.length ? 'warn' : 'ok');
            },
          },
          'Par défaut',
        ),
        undoBtn,
        h('span', { class: 'spacer' }),
        h(
          'button',
          {
            onclick: () => {
              dlgQ.close();
              openDialog(c.label);
            },
          },
          'Tous les raccourcis…',
        ),
        h('button', { class: 'primary', onclick: () => dlgQ.close() }, 'Fermer'),
      ),
    );
    // toute touche est prise pour la commande, sauf Échap seule (fermer)
    const onKey = (e) => {
      if (!dlgQ.open) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      if (e.key === 'Escape' && !e.ctrlKey && !e.altKey && !e.shiftKey) return dlgQ.close();
      const combo = comboOf(e);
      if (!combo) {
        const live = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift'].filter(Boolean);
        zone.textContent = live.length ? `${keyLabel(live.join('+'))}+…` : 'Appuie sur la nouvelle touche…';
        return;
      }
      zone.textContent = keyLabel(combo);
      const { before, takenBy } = setKey(id, 0, combo);
      if (takenBy.length) changed(before, `${keyLabel(combo)} servait à « ${takenBy.map((o) => o.label).join(' », « ')} » : la touche passe à « ${c.label} ».`, 'warn');
      else if (api.isReserved(combo)) changed(before, `${keyLabel(combo)} est gardé par le navigateur : ce raccourci marchera dans l’application installée.`, 'warn');
      else changed(before, `« ${c.label} » : ${keyLabel(combo)}`, 'ok');
    };
    window.addEventListener('keydown', onKey, true);
    dlgQ.addEventListener('close', () => {
      window.removeEventListener('keydown', onKey, true);
      dlgQ.remove();
    });
    showKeys();
    document.body.append(dlgQ);
    dlgQ.showModal();
  }

  // ---- affichage
  function render() {
    renderFilters();
    renderKeyboard();
    renderBody();
    renderStatus();
  }

  function renderBody() {
    renderTree();
    renderList();
  }

  function renderFilters() {
    const opts = [
      ['all', 'Toutes'],
      ['bound', 'Avec touche'],
      ['free', 'Sans touche'],
      ['modified', 'Modifiées'],
    ];
    ui.filters.replaceChildren(
      ...opts.map(([id, label]) =>
        h(
          'button',
          {
            class: filter === id ? 'active' : '',
            onclick: () => {
              filter = id;
              renderFilters();
              renderBody();
            },
          },
          label,
        ),
      ),
    );
    ui.keyChip.classList.toggle('hidden', !keyFilter);
    if (keyFilter)
      ui.keyChip.replaceChildren(
        `Touche ${keyLabel(keyFilter)}`,
        h(
          'button',
          {
            title: 'Retirer ce filtre',
            onclick: () => {
              keyFilter = null;
              renderFilters();
              renderKeyboard();
              renderBody();
            },
          },
          '×',
        ),
      );
  }

  function matches(c) {
    if (!visible(c)) return false;
    const ks = map.get(c.id) || [];
    if (filter === 'bound' && !ks.length) return false;
    if (filter === 'free' && ks.length) return false;
    if (filter === 'modified' && !isModified(c)) return false;
    if (keyFilter && !ks.some((k) => k.split('+').pop() === keyFilter)) return false;
    const q = norm(ui.search.value.trim());
    if (!q) return true;
    const hay = norm(`${c.label} ${menuById.get(c.menu).label} ${c.section} ${ks.map(keyLabel).join(' ')} ${ks.join(' ')}`);
    return q.split(/\s+/).every((w) => hay.includes(w));
  }

  function renderTree() {
    const shown = COMMANDS.filter(matches);
    const count = (menu, section) => shown.filter((c) => (!menu || c.menu === menu) && (!section || c.section === section)).length;
    const item = (label, n, sel, onclick, cls = '') => h('button', { class: `kbd-node ${cls}${sel ? ' active' : ''}${n ? '' : ' empty'}`, onclick }, h('span', { class: 'kbd-node-label' }, label), h('span', { class: 'count' }, String(n)));
    const rows = [item('Toutes les commandes', shown.length, !node.menu, () => ((node = { menu: null, section: null }), renderBody()), 'root')];
    for (const m of MENUS) {
      const sections = [...new Set(COMMANDS.filter((c) => c.menu === m.id && visible(c)).map((c) => c.section))];
      const open = !collapsed.has(m.id);
      rows.push(
        h(
          'div',
          { class: 'kbd-branch' },
          h(
            'button',
            {
              class: 'kbd-caret',
              title: open ? 'Replier' : 'Déplier',
              onclick: () => {
                if (open) collapsed.add(m.id);
                else collapsed.delete(m.id);
                renderTree();
              },
            },
            open ? '▾' : '▸',
          ),
          item(h('span', {}, h('i', { class: 'kbd-dot', style: { background: m.color } }), m.label), count(m.id), node.menu === m.id && !node.section, () => ((node = { menu: m.id, section: null }), renderBody()), 'menu'),
        ),
      );
      if (open) for (const s of sections) rows.push(item(s, count(m.id, s), node.menu === m.id && node.section === s, () => ((node = { menu: m.id, section: s }), renderBody()), 'section'));
    }
    ui.tree.replaceChildren(...rows);
  }

  function renderList() {
    const list = COMMANDS.filter((c) => matches(c) && (!node.menu || c.menu === node.menu) && (!node.section || c.section === node.section));
    const scroll = ui.list.scrollTop;
    const rows = [h('div', { class: 'kbd-row kbd-th' }, h('span', {}, 'Commande'), h('span', {}, 'Raccourcis'), h('span', {}))];
    let last = '';
    for (const c of list) {
      const m = menuById.get(c.menu);
      const head = `${m.label} › ${c.section}`;
      if (head !== last) {
        rows.push(h('div', { class: 'kbd-group' }, h('i', { class: 'kbd-dot', style: { background: m.color } }), head));
        last = head;
      }
      rows.push(row(c));
    }
    if (!list.length) rows.push(h('div', { class: 'empty' }, keyFilter ? `Aucune commande sur la touche ${keyLabel(keyFilter)} : elle est libre.` : 'Aucune commande ne correspond à la recherche.'));
    ui.list.replaceChildren(...rows);
    ui.list.scrollTop = scroll;
    ui.list.querySelector('.kbd-capture')?.scrollIntoView({ block: 'nearest' });
  }

  function row(c) {
    const ks = map.get(c.id) || [];
    const cap = capturing?.id === c.id ? capturing : null;
    const chips = ks.map((k, i) =>
      cap && cap.index === i
        ? captureChip(cap)
        : h(
            'span',
            { class: `kbd-chip${api.isReserved(k) ? ' reserved' : ''}`, title: api.isReserved(k) ? 'Gardé par le navigateur : marche dans l’application installée' : '' },
            h('button', { class: 'kbd-chip-key', title: 'Changer cette touche', onclick: (e) => startCapture(c.id, i, e) }, keyLabel(k)),
            h('button', { class: 'kbd-chip-x', title: 'Retirer ce raccourci', onclick: () => removeKey(c.id, i) }, '×'),
          ),
    );
    if (cap && cap.index >= ks.length) chips.push(captureChip(cap));
    else chips.push(h('button', { class: `kbd-add${ks.length ? '' : ' wide'}`, title: 'Ajouter un raccourci', onclick: (e) => startCapture(c.id, ks.length, e) }, ks.length ? '+' : '+ Ajouter'));
    return h(
      'div',
      { class: `kbd-row${cap ? ' capturing' : ''}${isModified(c) ? ' modified' : ''}` },
      h('span', { class: 'kbd-name' }, c.label, isModified(c) ? h('span', { class: 'kbd-modified', title: 'Modifié par toi' }, '•') : null),
      h('span', { class: 'kbd-keys' }, chips),
      h('span', { class: 'kbd-tools' }, isModified(c) ? h('button', { class: 'kbd-reset', title: 'Revenir aux touches par défaut', onclick: () => resetCommand(c.id) }, '↺') : null),
    );
  }

  const captureChip = (cap) => h('span', { class: 'kbd-capture' }, h('span', { class: 'kbd-capture-text' }, cap.live || 'Appuie sur les touches…'), h('button', { class: 'kbd-chip-x', title: 'Annuler (Échap)', onclick: () => cancelCapture() }, '×'));

  function renderKeyboard() {
    ui.kbToggle.textContent = showKeyboard ? 'Masquer le clavier' : 'Afficher le clavier';
    const mods = heldMods || pickedMods;
    const head = h(
      'div',
      { class: 'kbd-kbhead' },
      h('span', { class: 'kbd-kbtitle' }, 'Clavier'),
      h(
        'span',
        { class: 'kbd-seg small' },
        ['Ctrl', 'Alt', 'Shift'].map((m) =>
          h(
            'button',
            {
              class: mods[m] ? 'active' : '',
              title: 'Montre les touches prises avec ce modificateur (tu peux aussi le tenir enfoncé)',
              onclick: () => {
                pickedMods[m] = !pickedMods[m];
                renderKeyboard();
              },
            },
            keyLabel(m),
          ),
        ),
      ),
      h('span', { class: 'kbd-legend' }, MENUS.map((m) => h('span', {}, h('i', { class: 'kbd-dot', style: { background: m.color } }), m.label))),
      ui.kbToggle,
    );
    if (!showKeyboard) return ui.keyboardWrap.replaceChildren(head);
    const keyEl = (k) => {
      if (k.gap) return h('span', { class: 'kb-gap', style: { flex: `${k.w} ${k.w} 0` } });
      const isMod = k.t?.startsWith('mod:');
      const mod = isMod ? k.t.slice(4) : null;
      const combo = visualCombo(k.t, mods);
      const cmds = combo ? commandsOn(combo) : [];
      const first = cmds[0];
      const color = first ? menuById.get(first.menu).color : null;
      const title = !k.t ? 'Touche non attribuable' : isMod ? `${keyLabel(mod)} : clique pour voir les touches prises avec ${keyLabel(mod)}` : combo ? `${keyLabel(combo)} : ${cmds.length ? cmds.map((c) => c.label).join(', ') : 'libre'}` : 'Avec Maj, cette touche donne un autre caractère';
      return h(
        'button',
        {
          class: `kb-key${isMod && mods[mod] ? ' mod-on' : ''}${first ? ' bound' : ''}${!k.t ? ' dead' : ''}${keyFilter && k.t === keyFilter ? ' picked' : ''}`,
          style: { flex: `${k.w} ${k.w} 0`, ...(color ? { '--c': color } : {}) },
          title,
          tabindex: -1,
          onclick: () => {
            if (isMod) {
              pickedMods[mod] = !pickedMods[mod];
              return renderKeyboard();
            }
            if (!k.t) return;
            keyFilter = keyFilter === k.t ? null : k.t;
            renderFilters();
            renderKeyboard();
            renderBody();
          },
        },
        h('span', { class: 'kb-cap' }, k.sub ? h('small', {}, k.sub) : null, k.cap),
        first ? h('span', { class: 'kb-act' }, first.label.replace(/…$/, '')) : null,
      );
    };
    const rows = (defs) => h('div', { class: 'kb-block' }, defs.map((r) => h('div', { class: 'kb-row' }, r.map(keyEl))));
    ui.keyboardWrap.replaceChildren(head, h('div', { class: 'kb' }, rows(MAIN_ROWS), rows(NAV_ROWS)));
  }

  function renderStatus() {
    const shown = COMMANDS.filter(visible);
    const bound = shown.filter((c) => (map.get(c.id) || []).length).length;
    const n = note;
    ui.status.className = `kbd-status ${n?.kind || ''}`;
    ui.status.replaceChildren(
      n
        ? h(
            'span',
            {},
            n.text,
            n.undo
              ? h(
                  'button',
                  {
                    class: 'kbd-link',
                    onclick: () => {
                      map = n.undo;
                      note = { text: 'Modification annulée.', kind: 'ok' };
                      commit();
                      render();
                    },
                  },
                  'Annuler',
                )
              : null,
          )
        : h('span', { class: 'note' }, `${plural(shown.length, 'commande', 'commandes')} · ${plural(bound, 'avec raccourci', 'avec raccourci')}${desktop ? '' : ' · dans un navigateur, Ctrl+N, Ctrl+T et Ctrl+W restent au navigateur'}`),
    );
  }

  return api;
}
