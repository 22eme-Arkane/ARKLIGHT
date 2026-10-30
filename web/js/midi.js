// Contrôleur MIDI (Web MIDI) : un contrôleur branché en USB — boîte de faders, DMX Operator 384 en mode
// MIDI… — pilote la console ARKLIGHT, l'intensité des groupes, la sélection et les commandes du menu.
// Les correspondances sont gardées sur ce poste (le contrôleur est celui de ce PC, pas du projet) ; les
// groupes et projecteurs y sont désignés par leur nom (ou leur rang), pour servir dans tous les projets.
// Un préréglage (console connue, ou préréglage enregistré par l'utilisateur) forme une couche : les
// correspondances apprises à la main passent avant lui.
import { cleanMappings, isContinuous, parseMidi, routeMidi, scaleValue, sourceKey } from '/shared/midi.js';
import { MIDI_PRESETS, presetById, presetMappings } from '/shared/midi-presets.js';
import { flatPlans } from '/shared/plans.js';

const KEY = 'arklight.midi.mappings';
const PRESET_KEY = 'arklight.midi.preset'; // préréglage actif : { id, channel }
const USER_PRESETS_KEY = 'arklight.midi.userPresets'; // préréglages enregistrés : [{ id, name, mappings }]
export const PRESET_FORMAT = 'arklight-midi-preset';

function read(key, fallback) {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : JSON.parse(v);
  } catch {
    return fallback;
  }
}
function write(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* stockage indisponible : gardé pour cette session */
  }
}
const load = () => cleanMappings(read(KEY, []));
const loadUserPresets = () =>
  (Array.isArray(read(USER_PRESETS_KEY, [])) ? read(USER_PRESETS_KEY, []) : [])
    .filter((p) => p && typeof p.name === 'string')
    .map((p) => ({ id: String(p.id || `user:${p.name}`), name: p.name.slice(0, 60), mappings: cleanMappings(p.mappings) }));

/**
 * @param {object} o
 * @param {object} o.store
 * @param {object} o.control     console ARKLIGHT (web/js/ui/console-control.js)
 * @param {object} o.consoleApi  onglet Console : setFader(index, 0..1)
 * @param {object} o.actions     selectGroup…
 * @param {(id: string) => void} o.run  commandes d'ARKLIGHT
 * @param {(changes: object) => void} o.setEnv  ambiance (brume, exposition, lumière ambiante)
 * @param {() => string[]} o.fixtureOrder  projecteurs dans l'ordre de la liste Scène (tri choisi)
 * @param {(id: string) => void} o.openPlan  ouvre un plan du projet
 * @param {(on: boolean) => void} o.setBlackout  blackout général (tout dans le noir)
 */
export function createMidi({ store, control, consoleApi, actions, run, setEnv, fixtureOrder, openPlan, setBlackout }) {
  const state = {
    page: 0, // page de faders : les faders du contrôleur commandent les faders (page × taille) + n de la console
    supported: typeof navigator.requestMIDIAccess === 'function',
    started: false,
    error: '',
    inputs: [], // { id, name, manufacturer }
    last: null, // dernier message reçu : { msg, device }
    mappings: load(), // apprises à la main (passent avant le préréglage)
    preset: read(PRESET_KEY, null), // préréglage actif { id, channel } ou null
    userPresets: loadUserPresets(),
  };
  let layerCache = null; // correspondances du préréglage actif (calculées une fois)
  let allCache = null;

  /** Préréglage actif : intégré (MIDI_PRESETS) ou enregistré par l'utilisateur. */
  const activePreset = () => {
    const id = state.preset?.id;
    return id ? presetById(id) || state.userPresets.find((p) => p.id === id) || null : null;
  };
  function layer() {
    if (!layerCache) {
      const p = activePreset();
      layerCache = !p ? [] : p.build ? presetMappings(p, { channel: state.preset.channel }) : p.mappings.map((m, i) => ({ ...m, id: `${p.id}:${i}`, preset: true }));
    }
    return layerCache;
  }
  /** Toutes les correspondances : les siennes, puis celles du préréglage pour les autres commandes. */
  function all() {
    if (!allCache) {
      const own = new Set(state.mappings.filter((m) => m.source).map((m) => sourceKey(m.source)));
      allCache = [...state.mappings, ...layer().filter((m) => !own.has(sourceKey(m.source)))];
    }
    return allCache;
  }
  const held = new Map(); // état des touches (appui une seule fois)
  const latched = new Map(); // touches à bascule allumées (Fixture, Chase…) : clé → { kind, index }

  /** La sélection suit les touches allumées : leurs projecteurs, et les membres de leurs groupes. */
  function applyLatched() {
    const order = fixtureOrder?.() || store.scene.fixtures.map((f) => f.id);
    const groups = store.scene?.groups || [];
    const ids = new Set();
    for (const { kind, index } of latched.values()) {
      if (kind === 'fixtureLatch' && order[index - 1]) ids.add(order[index - 1]);
      if (kind === 'groupLatch') for (const id of groups[index - 1]?.ids || []) ids.add(id);
    }
    store.setSelection([...ids]);
  }
  const listeners = new Set();
  let access = null;
  let learner = null;
  let queued = false;

  const emit = () => listeners.forEach((fn) => fn(state));
  /** Au plus une mise à jour de l'affichage par image (un fader envoie beaucoup de messages). */
  const emitSoon = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      emit();
    });
  };
  const save = () => {
    write(KEY, state.mappings);
    write(PRESET_KEY, state.preset);
    write(USER_PRESETS_KEY, state.userPresets);
    layerCache = null;
    allCache = null;
    emit();
    requestAnimationFrame(() => showPage());
  };

  async function start() {
    if (!state.supported || state.started) return;
    state.started = true;
    try {
      access = await navigator.requestMIDIAccess({ sysex: false });
      access.onstatechange = () => refreshInputs();
      refreshInputs();
    } catch (e) {
      state.error = e?.message || String(e);
      emit();
    }
  }

  function refreshInputs() {
    const list = [];
    for (const input of access.inputs.values()) {
      if (input.state !== 'connected') continue;
      input.onmidimessage = (ev) => receive(ev.data, input.name);
      list.push({ id: input.id, name: input.name || 'MIDI', manufacturer: input.manufacturer || '' });
    }
    state.inputs = list;
    // un contrôleur connu est branché et rien n'est réglé : on propose son préréglage
    state.suggest = !state.preset && !state.mappings.length ? MIDI_PRESETS.find((p) => p.match && list.some((d) => p.match.test(`${d.name} ${d.manufacturer}`)))?.id || null : null;
    emit();
  }

  const groupByName = (name) => (store.scene?.groups || []).find((g) => g.name === name);
  const fixtureByName = (name) => (store.scene?.fixtures || []).find((f) => f.name === name);
  const groupFixtures = (g) => g.ids.map((id) => store.fixtures.get(id)).filter(Boolean);

  // ---- pages de faders : 8 faders sur le contrôleur, plus de canaux dans la console
  const faderMaps = () => all().filter((m) => m.target.kind === 'fader');
  const pageSize = () => Math.max(1, ...faderMaps().map((m) => m.target.index || 1));
  /** Faders de la console commandés en ce moment : [{ index, label }] (label : « B1·3 » d'un préréglage). */
  const faderMarks = () =>
    faderMaps()
      .map((m) => ({ index: (m.target.index || 1) + state.page * pageSize(), label: m.label || null }))
      .sort((a, b) => a.index - b.index);
  const faderIndexes = () => faderMarks().map((m) => m.index);
  function showPage() {
    consoleApi.setMidiFaders?.(faderMarks(), state.page);
    emitSoon();
  }
  function turnPage(step) {
    const count = consoleApi.faderCount?.() || 0;
    const last = Math.max(0, Math.ceil(count / pageSize()) - 1);
    state.page = Math.max(0, Math.min(last, state.page + step));
    showPage();
  }
  store.addEventListener('selection', () => {
    if (state.page) state.page = 0; // nouvelle sélection : on repart des premiers faders
    requestAnimationFrame(showPage);
  });

  /** Sélection suivante / précédente dans une liste (boucle). */
  function cycle(ids, current, step) {
    if (!ids.length) return null;
    const i = ids.indexOf(current);
    if (i < 0) return step > 0 ? ids[0] : ids[ids.length - 1];
    return ids[(i + step + ids.length) % ids.length];
  }
  const currentGroup = () => (store.scene?.groups || []).find((g) => g.ids.length && g.ids.length === store.selection.size && g.ids.every((id) => store.isSelected(id)));

  function apply({ mapping, value, press, release, latch }) {
    const t = mapping.target;
    switch (t.kind) {
      case 'fixtureLatch':
      case 'groupLatch': {
        const key = `${t.kind}:${t.index || 1}`;
        if (latch) latched.set(key, { kind: t.kind, index: t.index || 1 });
        else latched.delete(key);
        applyLatched();
        break;
      }
      case 'blackout':
        setBlackout?.(!!latch);
        break;
      case 'fader':
        consoleApi.setFader((t.index || 1) + state.page * pageSize(), value);
        break;
      case 'pageNext':
        turnPage(1);
        break;
      case 'pagePrev':
        turnPage(-1);
        break;
      case 'familyNext':
        consoleApi.nextFamily?.();
        break;
      case 'flash':
        control.flash(store.selectedFixtures(), !!press && !release);
        break;
      case 'groupLevel': {
        const g = groupByName(t.name);
        if (g) control.setIntensity(groupFixtures(g), value);
        break;
      }
      case 'fixtureLevel': {
        const f = fixtureByName(t.name);
        if (f) control.setIntensity([f], value);
        break;
      }
      case 'selectionLevel':
        control.setIntensity(store.selectedFixtures(), value);
        break;
      case 'selCct':
        control.setColor(store.selectedFixtures(), { cct: scaleValue('selCct', value) });
        break;
      case 'selTint':
        control.setColor(store.selectedFixtures(), { tint: scaleValue('selTint', value) });
        break;
      case 'selHue':
        control.setColor(store.selectedFixtures(), { hue: scaleValue('selHue', value) });
        break;
      case 'selSat':
        control.setColor(store.selectedFixtures(), { sat: value });
        break;
      case 'group': {
        const g = groupByName(t.name);
        if (g) actions.selectGroup(g.id);
        break;
      }
      case 'fixture': {
        const f = fixtureByName(t.name);
        if (f) store.select(f.id);
        break;
      }
      case 'fixtureIndex': {
        const id = (fixtureOrder?.() || store.scene.fixtures.map((f) => f.id))[(t.index || 1) - 1];
        if (id) store.select(id);
        break;
      }
      case 'groupIndex': {
        const g = (store.scene?.groups || [])[(t.index || 1) - 1];
        if (g) actions.selectGroup(g.id);
        break;
      }
      case 'planIndex': {
        const p = flatPlans(store.plans)[(t.index || 1) - 1];
        if (p) openPlan?.(p.plan.id);
        break;
      }
      case 'groupNext':
      case 'groupPrev': {
        const groups = (store.scene?.groups || []).filter((g) => g.ids.length);
        const next = cycle(
          groups.map((g) => g.id),
          currentGroup()?.id,
          t.kind === 'groupNext' ? 1 : -1,
        );
        if (next) actions.selectGroup(next);
        break;
      }
      case 'fixtureNext':
      case 'fixturePrev': {
        const next = cycle(fixtureOrder?.() || store.scene.fixtures.map((f) => f.id), store.selectedId, t.kind === 'fixtureNext' ? 1 : -1);
        if (next) store.select(next);
        break;
      }
      case 'haze':
        setEnv?.({ haze: Math.round(value * 100) / 100 });
        break;
      case 'exposure':
        setEnv?.({ exposure: Math.pow(2, scaleValue('exposure', value)) });
        break;
      case 'ambient':
        setEnv?.({ ambient: scaleValue('ambient', value) });
        break;
      case 'command':
        if (t.id) run(t.id);
        break;
    }
  }

  /** Un message reçu (d'un contrôleur, ou simulé par un test). */
  function receive(data, device = '') {
    const msg = parseMidi(data);
    if (!msg) return;
    state.last = { msg, device };
    if (learner && learner(msg, device)) return emitSoon();
    for (const r of routeMidi(all(), msg, held)) apply(r);
    emitSoon();
  }

  requestAnimationFrame(() => showPage());

  return {
    state,
    start,
    receive,
    /** Faders de la console commandés en ce moment par le contrôleur (repères dans l'onglet Console). */
    faderIndexes,
    onChange(fn) {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    /**
     * Attend la prochaine commande du contrôleur : fn(source) quand un fader bouge ou qu'une touche est
     * appuyée (les relâchements sont ignorés). accept(source) peut refuser (déjà apprise…). null : annule.
     */
    learn(fn, accept = () => true) {
      learner = fn
        ? (msg) => {
            if ((msg.type === 'note' && !msg.on) || !accept(msg)) return true;
            fn({ type: msg.type, channel: msg.channel, number: msg.type === 'pitch' ? 0 : msg.number }, msg);
            return true;
          }
        : null;
      emit();
    },
    get learning() {
      return !!learner;
    },
    add(mapping) {
      const m = { id: `m${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`, ...mapping };
      state.mappings.push(m);
      save();
      return m;
    },
    update(id, patch) {
      const m = state.mappings.find((x) => x.id === id);
      if (m) Object.assign(m, patch);
      save();
    },
    remove(id) {
      state.mappings = state.mappings.filter((x) => x.id !== id);
      save();
    },
    clear() {
      state.mappings = [];
      save();
    },
    // ---- préréglages
    /** Préréglage actif (intégré ou enregistré), et ses correspondances. */
    activePreset,
    layer,
    all,
    /** Active un préréglage (null : aucun). options : { channel } pour un préréglage intégré. */
    setPreset(id, options = {}) {
      const p = id ? presetById(id) || state.userPresets.find((x) => x.id === id) : null;
      state.preset = p ? { id: p.id, channel: Math.min(16, Math.max(1, options.channel ?? state.preset?.channel ?? p.options?.channel ?? 1)) } : null;
      state.suggest = null;
      save();
    },
    /** Enregistre l'ensemble en cours (préréglage + correspondances apprises) comme préréglage personnel. */
    saveUserPreset(name) {
      const id = `user:${Date.now().toString(36)}`;
      const mappings = all().map(({ source, target, label }) => ({ source, target, ...(label ? { label } : {}) }));
      state.userPresets.push({ id, name: String(name).slice(0, 60), mappings });
      state.preset = { id, channel: 1 };
      state.mappings = []; // tout est maintenant dans le préréglage
      save();
      return id;
    },
    deleteUserPreset(id) {
      state.userPresets = state.userPresets.filter((p) => p.id !== id);
      if (state.preset?.id === id) state.preset = null;
      save();
    },
    /** Fichier de préréglage à partager (autre poste, autre utilisateur de la même console). */
    exportPreset(name) {
      return { format: PRESET_FORMAT, version: 1, name, mappings: all().map(({ source, target, label }) => ({ source, target, ...(label ? { label } : {}) })) };
    },
    /** Ajoute un préréglage lu dans un fichier ; renvoie son nom, ou une erreur. */
    importPreset(json) {
      if (!json || json.format !== PRESET_FORMAT || !Array.isArray(json.mappings)) throw new Error('Ce fichier n’est pas un préréglage MIDI ARKLIGHT');
      const mappings = cleanMappings(json.mappings).filter((m) => m.source);
      if (!mappings.length) throw new Error('Ce préréglage ne contient aucune commande');
      const name = String(json.name || 'Préréglage importé').slice(0, 60);
      const id = `user:${Date.now().toString(36)}`;
      state.userPresets.push({ id, name, mappings });
      state.preset = { id, channel: 1 };
      save();
      return name;
    },
    sourceKey,
    isContinuous,
  };
}
