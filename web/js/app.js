// Point d'entrée de l'interface ARKLIGHT.
import { LANGS, lang, langChoice, setLang, systemLanguage, t, takeLangReload, translatePage } from './i18n.js';
import { Net } from './net.js';
import { Store } from './store.js';
import { Viewport } from './viewport/viewport.js';
import { h, toast } from './ui/dom.js';
import { mountLibrary } from './ui/library.js';
import { mountSceneList } from './ui/scene-list.js';
import { mountInspector } from './ui/inspector.js';
import { mountTestConsole } from './ui/test-console.js';
import { createConsoleControl } from './ui/console-control.js';
import { createMidi } from './midi.js';
import { createMidiPanel } from './ui/midi-panel.js';
import { mountDmxMonitor } from './ui/dmx-monitor.js';
import { mountLog, mountNetwork } from './ui/network.js';
import { mountBluetooth } from './ui/bluetooth.js';
import { OnboardRegistry, createControlPanel } from './ui/control-panel.js';
import { mountRemote } from './ui/remote.js';
import { isContextMenuOpen, openContextMenu } from './ui/context-menu.js';
import { mountMenubar } from './ui/menubar.js';
import { mountPanels } from './ui/panels.js';
import { askConfirm, askSaveChanges, askText } from './ui/dialogs.js';
import { isProp } from '/shared/objects.js';
import { createActions } from './actions.js';
import { mountPlot } from './plot/plot-view.js';
import { COMMANDS, MENUS, commandById } from './commands.js';
import { createShortcuts, keyLabel, setKeyNames, setTranslator } from './shortcuts.js';
import { hydrateIcons } from './ui/icons.js';
import { mountProjects, projectsAtStartup } from './ui/projects.js';
import { mountPlans } from './ui/plans.js';
import { mountAssistant } from './ui/assistant.js';
import { currentPlan, flatPlans } from '/shared/plans.js';
import { confirmQuit, mountUpdateBanner, openAbout, openExternal, openSupport } from './ui/support.js';

// Langue de l'interface : menus et commandes traduits, puis tout ce qui s'affiche (voir i18n.js)
if (lang !== 'fr') {
  for (const m of MENUS) m.label = t(m.label);
  for (const c of COMMANDS) {
    c.label = c[lang] || t(c.label);
    c.section = t(c.section);
  }
  if (lang === 'en') setKeyNames({ Shift: 'Shift', Delete: 'Del', Escape: 'Esc', Enter: 'Enter', Space: 'Space', Backspace: 'Backspace', Home: 'Home', End: 'End', PageUp: 'PgUp', PageDown: 'PgDn', Insert: 'Ins' });
}
setTranslator(t);
translatePage();
const langReload = takeLangReload(); // rechargement après un changement de langue
window.arkDesktop?.setLanguage?.(lang);

const desktop = !!window.arkDesktop;
const store = new Store();
const net = new Net();
const shortcuts = createShortcuts({ desktop });
const keyOf = (id) => shortcuts.label(id);
hydrateIcons(); // icônes des barres d'outils
const viewport = new Viewport(document.getElementById('viewport'), store, net);
viewport.fpsUnit = t('i/s');
const drawer = document.getElementById('drawer');
const smoke = new URLSearchParams(location.search).has('smoke'); // test automatique : pas d'écran des projets

// ------------------------------------------------------------------ messages serveur
let pendingSelectType = null;
let pendingSelectObject = null;
net.addEventListener('message', (e) => {
  const msg = e.detail;
  switch (msg.t) {
    case 'hello': {
      const first = !store.clientId;
      store.clientId = msg.clientId;
      store.version = msg.version;
      store.repo = msg.repo;
      store.config = msg.config || {};
      store.local = msg.local !== false; // interface ouverte sur l'ordinateur d'ARKLIGHT (assistant IA)
      assistantUi.setLocal(store.local);
      store.setScene(msg.scene);
      store.setPlans(msg.plans);
      if (msg.history) store.setHistory(msg.history);
      for (const l of msg.logs || []) store.addLog(l);
      applySceneMeta();
      // le serveur nomme les nouveaux objets et le projet d'exemple dans la langue de l'interface
      if (first && (store.config.lang || 'fr') !== lang) net.send({ t: 'config.update', changes: { lang } });
      // au lancement : l'écran des projets (déjà ouvert, voir plus bas) — sa liste se met à jour ici
      if (first && hubAtStartup) projects.open();
      document.documentElement.classList.remove('boot-hub');
      break;
    }
    case 'scene':
      store.setScene(msg.scene);
      store.setPlans(msg.plans);
      applySceneMeta();
      projects.close();
      break;
    case 'plans':
      store.setPlans(msg.plans);
      break;
    case 'ai.state':
    case 'ai.models':
    case 'ai.event':
    case 'ai.done':
    case 'ai.error':
    case 'ai.reset':
    case 'ai.progress':
    case 'ai.aiodos.code':
      assistantUi.onMessage(msg);
      break;
    case 'scene.restore':
      // annuler / rétablir : seuls les éléments modifiés sont mis à jour
      store.reconcileScene(msg.scene);
      if (msg.origin === store.clientId) toast(`${msg.undo ? 'Annulé' : 'Rétabli'} : ${msg.label}`);
      break;
    case 'scene.meta':
      store.scene.name = msg.name;
      applySceneMeta();
      break;
    case 'history':
      store.setHistory(msg);
      break;
    case 'fixture.added':
      store.upsertFixture(msg.fixture, msg.origin);
      if (msg.origin === store.clientId && pendingSelectType === msg.fixture.type) {
        pendingSelectType = null;
        store.select(msg.fixture.id);
      }
      break;
    case 'fixture.updated':
      if (msg.origin !== store.clientId) store.upsertFixture(msg.fixture, msg.origin);
      break;
    case 'fixture.identify': {
      // une appli (Luminair…) demande au projecteur de se signaler (RDM)
      viewport.identify(msg.id, msg.on);
      const f = store.fixtures.get(msg.id);
      if (msg.on && f) toast(t('Identification demandée par l’appli : {0}', { 0: f.name }), 'info', 3500);
      break;
    }
    case 'fixture.removed':
      store.removeFixture(msg.id);
      break;
    case 'fixtures.added':
      for (const f of msg.fixtures) store.upsertFixture(f, msg.origin);
      if (msg.origin === store.clientId && msg.select) store.setSelection(msg.fixtures.map((f) => f.id));
      break;
    case 'fixtures.updated':
      // l'interface d'origine a déjà appliqué ces changements localement
      if (msg.origin !== store.clientId) for (const f of msg.fixtures) store.upsertFixture(f, msg.origin);
      break;
    case 'fixtures.removed':
      for (const id of msg.ids) store.removeFixture(id);
      break;
    case 'object.added':
      store.upsertObject(msg.object, msg.origin);
      if (msg.origin === store.clientId && (msg.select || pendingSelectObject === msg.object.type)) {
        pendingSelectObject = null;
        store.selectObject(msg.object.id);
      }
      break;
    case 'object.updated':
      if (msg.origin !== store.clientId) store.upsertObject(msg.object, msg.origin);
      break;
    case 'object.removed':
      store.removeObject(msg.id);
      break;
    case 'plot.item':
      store.setPlotItem(msg.item);
      if (msg.added && msg.origin === store.clientId) store.emit('plot-item-created', msg.item);
      break;
    case 'plot.removed':
      store.removePlotItem(msg.id);
      break;
    case 'plot.meta':
      store.setPlotMeta(msg);
      break;
    case 'groups':
      store.setGroups(msg.groups);
      if (msg.created && msg.origin === store.clientId) store.emit('group-created', msg.created);
      break;
    case 'env':
      if (msg.origin !== store.clientId) {
        store.setEnvironment(msg.environment);
        applyEnvControls();
      }
      break;
    case 'status':
      store.setStatus(msg);
      break;
    case 'dmx.forget':
      // DMX conservé oublié (« Reprendre la main ») : les Astera quittent l'entrée CRMX / filaire verrouillée
      for (const f of store.scene?.fixtures || []) if (msg.universes.includes(f.settings.universe)) onboard.runtime(f.id).latch = null;
      store.forgetDmx(msg.universes);
      break;
    case 'config':
      store.config = msg.config;
      store.emit('config', msg.config);
      break;
    case 'projects':
    case 'projects.changed':
      projects.onMessage(msg);
      break;
    case 'log':
      store.addLog(msg);
      break;
    case 'update':
      updateBanner.show(msg);
      break;
    case 'error':
      toast(msg.message, 'error');
      break;
  }
});
net.addEventListener('dmx', (e) => store.setDmx(e.detail.universe, e.detail.data));
net.connect();

/** Attend une réponse du serveur (null après le délai). */
function waitFor(type, { timeout = 2500, filter = () => true } = {}) {
  return new Promise((resolve) => {
    const onMsg = (e) => {
      if (e.detail.t !== type || !filter(e.detail)) return;
      clearTimeout(timer);
      net.removeEventListener('message', onMsg);
      resolve(e.detail);
    };
    const timer = setTimeout(() => {
      net.removeEventListener('message', onMsg);
      resolve(null);
    }, timeout);
    net.addEventListener('message', onMsg);
  });
}

// ------------------------------------------------------------------ panneaux
const library = mountLibrary({
  onAdd: (type) => {
    pendingSelectType = type.id;
    net.send({ t: 'fixture.add', type: type.id, ...viewport.suggestPlacement(type) });
  },
  onAddGrip: (type) => {
    pendingSelectObject = type.id;
    const place = isProp(type) ? viewport.suggestObjectPlacement(type) : viewport.gripPlacementAt(type, 1.2, 2.2);
    net.send({ t: 'object.add', type: type.id, ...place });
  },
});
const onboard = new OnboardRegistry({ store, net });
viewport.onboard = onboard;
const consoleControl = createConsoleControl({ store, net, viewport }); // console ARKLIGHT (onglet Console, « Canaux »)
const actions = createActions({ store, net, viewport, onboard, keyOf });
const sceneList = mountSceneList({ store, viewport, actions });
mountInspector({ store, net, viewport, onboard, actions, control: consoleControl, controlPanel: createControlPanel({ registry: onboard }) });
viewport.addEventListener('context', (e) => openContextMenu(e.detail.x, e.detail.y, e.detail.objectId ? actions.objectMenu(e.detail.objectId) : actions.selectionMenu()));
const plot = mountPlot({ store, net, viewport, actions });
document.getElementById('btn-plot').addEventListener('click', () => plot.toggle());
viewport.addEventListener('drop-object', (e) => {
  const { type, position, rotation } = e.detail;
  pendingSelectObject = type.id;
  net.send({ t: 'object.add', type: type.id, position, rotation });
});
viewport.addEventListener('drop-fixture', (e) => {
  const { type, position, rotation } = e.detail;
  pendingSelectType = type.id;
  net.send({ t: 'fixture.add', type: type.id, position, rotation });
});
const remote = mountRemote({ store, viewport, onboard });
const remoteBtn = document.getElementById('btn-remote');
remoteBtn.addEventListener('click', () => remote.toggle());
// le bouton suit l'ouverture du panneau (bouton, raccourci ou fermeture depuis le panneau)
const remotePanel = document.querySelector('.remote-panel');
if (remotePanel) new MutationObserver(() => remoteBtn.classList.toggle('active', remote.open)).observe(remotePanel, { attributes: true, attributeFilter: ['class'] });
const consoleApi = mountTestConsole({ store, net, control: consoleControl });
// contrôleur MIDI (faders de la console, intensité des groupes, commandes) : voir midi.js
// Blackout général (touche d'une console MIDI, menu Outils) : un bandeau le rappelle tant qu'il est actif
const blackoutBadge = h('button', { class: 'blackout-badge hidden', title: 'Clic : fin du blackout', onclick: () => setBlackout(false) }, 'BLACKOUT');
document.querySelector('.viewport-area').append(blackoutBadge);
function setBlackout(on) {
  viewport.setBlackout(on);
  blackoutBadge.classList.toggle('hidden', !on);
}
const midi = createMidi({
  store,
  setBlackout,
  control: consoleControl,
  consoleApi,
  actions,
  run: (id) => run(id),
  setEnv: (changes) => {
    sendEnv(changes);
    applyEnvControls();
  },
  fixtureOrder: () => sceneList.order(),
  openPlan: (id) => plans.switchTo(id),
});
const midiPanel = createMidiPanel({ midi, store, fixtureOrder: () => sceneList.order() });
const midiPill = document.getElementById('st-midi');
midiPill.addEventListener('click', () => midiPanel.open());
midi.onChange((st) => {
  midiPill.classList.toggle('hidden', !st.inputs.length && !st.mappings.length);
  midiPill.classList.toggle('ok', st.inputs.length > 0);
  midiPill.title = st.inputs.length ? `${t('Contrôleur MIDI')} : ${st.inputs.map((d) => d.name).join(', ')}` : t('Contrôleur MIDI : aucun appareil branché');
});
// l'application de bureau écoute le MIDI dès le lancement ; un navigateur demanderait l'autorisation
if (desktop || midi.state.mappings.length) midi.start();
const drawerTabs = { current: 'console' };
const dmxMonitor = mountDmxMonitor({ store, actions, isVisible: () => drawerTabs.current === 'dmx' && !drawer.classList.contains('collapsed') });
mountLog({ store });
mountNetwork({ store, net });
mountBluetooth({ store, net });

// Panneaux repliables et redimensionnables (gauche, droite, bas)
const panels = mountPanels({
  keyOf,
  onChange: (which) => which === 'bottom' && !panels.folded('bottom') && drawerTabs.current === 'dmx' && dmxMonitor.paint(),
});

// ------------------------------------------------------------------ scène : nom, enregistrement, ouverture
const sceneName = document.getElementById('scene-name');
function applySceneMeta() {
  sceneName.value = store.scene?.name || '';
  applyEnvControls();
  applyTitle();
}
/** Titre de la fenêtre : « Mon projet * — ARKLIGHT » tant que des modifications ne sont pas enregistrées. */
function applyTitle() {
  const dirty = store.history.dirty && hasContent();
  const cur = flatPlans(store.plans).length > 1 ? currentPlan(store.plans) : null;
  document.title = `${store.scene?.name || t('Nouveau projet')}${cur ? ` · ${cur.seq.name} › ${cur.plan.name}` : ''}${dirty ? ' *' : ''} — ARKLIGHT`;
  sceneName.title = t('Nom du projet ({0} pour renommer){1}', { 0: keyOf('scene.rename') || 'F2', 1: dirty ? t(' — modifications non enregistrées') : '' });
}
store.addEventListener('history', applyTitle);
store.addEventListener('plans', applyTitle);
sceneName.addEventListener('change', () => net.send({ t: 'scene.rename', name: sceneName.value }));
sceneName.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' || e.key === 'Escape') {
    if (e.key === 'Escape') sceneName.value = store.scene?.name || '';
    sceneName.blur();
  }
});

/** La scène contient quelque chose qui mérite d'être enregistré. */
const hasContent = () => !!(store.scene && (store.scene.fixtures.length || store.scene.objects.length || store.scene.plot.items.length));

/** Attend la première réponse parmi ces types (null après le délai). */
function waitForAny(types, timeout = 5000) {
  return new Promise((resolve) => {
    const onMsg = (e) => {
      if (!types.includes(e.detail.t)) return;
      clearTimeout(timer);
      net.removeEventListener('message', onMsg);
      resolve(e.detail);
    };
    const timer = setTimeout(() => {
      net.removeEventListener('message', onMsg);
      resolve(null);
    }, timeout);
    net.addEventListener('message', onMsg);
  });
}

/**
 * Enregistre le projet avec une vignette de la vue 3D. La première fois (ou « Enregistrer sous ») :
 * fenêtre de Windows pour choisir le nom et l'emplacement (application installée), sinon un nom
 * dans le dossier des projets.
 */
/** Fichier › Revenir à la version enregistrée : oublie les modifications depuis le dernier enregistrement. */
async function revertScene() {
  const path = store.history.savedPath;
  if (!path) return toast(t('Ce projet n’a pas encore été enregistré'), 'error');
  const ok = await askConfirm({ title: 'Revenir à la version enregistrée ?', note: 'Les modifications faites depuis le dernier enregistrement seront perdues.', ok: 'Revenir', danger: true });
  if (ok) net.send({ t: 'project.open', path });
}

async function saveScene({ as = false } = {}) {
  let thumbnail = null;
  try {
    thumbnail = viewport.snapshot();
  } catch {
    thumbnail = null;
  }
  if (!as && store.history.savedPath) {
    const reply = waitForAny(['project.saved', 'error']);
    net.send({ t: 'project.save', thumbnail });
    const r = await reply;
    if (r?.t !== 'project.saved') return false;
    toast(`Projet enregistré : ${r.name}`);
    return true;
  }
  if (desktop) {
    const r = await window.arkDesktop.saveProjectAs(thumbnail);
    if (!r) return false;
    if (r.error) {
      toast(r.error, 'error', 4000);
      return false;
    }
    toast(`Projet enregistré : ${r.name}`);
    return true;
  }
  const current = (sceneName.value || store.scene.name || '').trim();
  const name = await askText({
    title: as ? 'Enregistrer sous' : 'Enregistrer le projet',
    label: 'Nom du projet',
    value: /^(Nouveau projet|Nouvelle scène)$/.test(current) && !as ? '' : current,
    ok: 'Enregistrer',
    note: 'Le projet est rangé dans le dossier des projets d’ARKLIGHT.',
  });
  if (!name) return false;
  for (let overwrite = false; ; overwrite = true) {
    const reply = waitForAny(['project.saved', 'project.exists', 'error']);
    net.send({ t: 'project.save', name, thumbnail, overwrite });
    const r = await reply;
    if (r?.t === 'project.exists') {
      if (!overwrite && confirm(t('Un projet « {0} » existe déjà. Le remplacer ?', { 0: r.name }))) continue;
      return false;
    }
    if (r?.t !== 'project.saved') return false;
    toast(`Projet enregistré : ${r.name}`);
    return true;
  }
}

/** Avant de quitter le projet : proposer d'enregistrer. false : l'utilisateur annule. */
async function confirmDiscard(action) {
  if (!store.history.dirty || !hasContent()) return true;
  const r = await askSaveChanges(store.scene.name, action);
  if (r === 'cancel') return false;
  return r === 'save' ? saveScene() : true;
}

async function newScene() {
  if (await confirmDiscard('créer un nouveau projet')) net.send({ t: 'project.new' });
}

// Écran « Projets » : au lancement et avec « Ouvrir un projet »
const projects = mountProjects({ store, net, desktop, confirmDiscard });
// l'écran des projets s'ouvre tout de suite au lancement, avant même la connexion au serveur
const hubAtStartup = !smoke && (langReload === 'projects' || (!langReload && projectsAtStartup()));
if (hubAtStartup) projects.open();
document.documentElement.classList.remove('boot-hub');
const plans = mountPlans({ store, net, viewport, keyOf });
const assistantUi = mountAssistant({
  store,
  net,
  desktop,
  keyOf,
  // plan dessiné réinterprété : le plan 2D redessiné, prêt à exporter
  onDrawingDone: () => {
    if (!plot.active) plot.toggle();
    toast(t('Plan redessiné d’après le dessin : Fichier › Plan de feu › Exporter le plan en image PNG (ou SVG)'), 'info', 6000);
  },
});

// ------------------------------------------------------------------ barre d'outils 3D
const toolButtons = document.querySelectorAll('[data-tool]');
let currentTool = 'translate';
function setTool(mode) {
  currentTool = mode;
  viewport.setTool(mode);
  toolButtons.forEach((b) => b.classList.toggle('active', b.dataset.tool === mode));
}
toolButtons.forEach((b) => b.addEventListener('click', () => setTool(b.dataset.tool)));
document.querySelectorAll('[data-view]').forEach((b) => b.addEventListener('click', () => viewport.setView(b.dataset.view)));
const snapBtn = document.getElementById('btn-snap');
snapBtn.addEventListener('click', () => {
  snapBtn.classList.toggle('active');
  viewport.setSnap(snapBtn.classList.contains('active'));
});
// sélecteur 3D / plan 2D (le même en tête du plan), caméra, cadrage
document.querySelector('#viewbar .mode-switch .mode:not(#btn-plot)').addEventListener('click', () => plot.active && plot.toggle());
const camBtn = document.getElementById('btn-camview');
camBtn.addEventListener('click', () => toggleCameraView());
document.getElementById('btn-frame').addEventListener('click', () => viewport.frameSelected());

const envInputs = {
  haze: document.getElementById('env-haze'),
  exposure: document.getElementById('env-exposure'),
  ambient: document.getElementById('env-ambient'),
};
function sendEnv(changes) {
  Object.assign(store.scene.environment, changes);
  viewport.applyEnvironment(store.scene.environment);
  sendEnv.pending = { ...sendEnv.pending, ...changes }; // plusieurs réglages bougés ensemble : tous envoyés
  clearTimeout(sendEnv.t);
  sendEnv.t = setTimeout(() => {
    net.send({ t: 'env.update', changes: sendEnv.pending });
    sendEnv.pending = null;
  }, 120);
}
// L'exposition se règle en diaphragmes (IL) : 0 IL = exposition 1, chaque cran double ou divise la lumière
const toEnv = { exposure: (stops) => Math.pow(2, stops) };
const fromEnv = { exposure: (x) => Math.log2(x || 1) };
const expoVal = document.getElementById('env-exposure-val');
function showExposure() {
  const stops = Number(envInputs.exposure.value);
  const v = Math.abs(stops).toFixed(1).replace('.0', '');
  expoVal.textContent = t('{0} IL', { 0: `${stops > 0 ? '+' : stops < 0 ? '−' : ''}${lang === 'fr' ? v.replace('.', ',') : v}` });
}
for (const [k, input] of Object.entries(envInputs)) {
  input.addEventListener('input', () => {
    sendEnv({ [k]: toEnv[k] ? toEnv[k](Number(input.value)) : Number(input.value) });
    if (k === 'exposure') showExposure();
  });
}
const envMenu = document.getElementById('env-menu');
const envBtn = document.getElementById('btn-env');
envBtn.addEventListener('click', () => {
  const show = envMenu.classList.contains('hidden');
  envMenu.classList.toggle('hidden', !show);
  if (show) envMenu.style.left = `${Math.max(6, envBtn.offsetLeft)}px`; // sous le bouton
});
document.addEventListener(
  'pointerdown',
  (e) => {
    if (!envMenu.classList.contains('hidden') && !envMenu.contains(e.target) && !envBtn.contains(e.target)) envMenu.classList.add('hidden');
  },
  true,
);
envMenu.querySelectorAll('[data-env]').forEach((cb) => cb.addEventListener('change', () => sendEnv({ [cb.dataset.env]: cb.checked })));
function applyEnvControls() {
  const env = store.scene?.environment;
  if (!env) return;
  for (const [k, input] of Object.entries(envInputs)) input.value = fromEnv[k] ? fromEnv[k](env[k]) : env[k];
  showExposure();
  envMenu.querySelectorAll('[data-env]').forEach((cb) => (cb.checked = env[cb.dataset.env] !== false));
}
/** Bascule d'une case du menu Décor (noms des projecteurs, sol…). */
function toggleEnv(key) {
  const cb = envMenu.querySelector(`[data-env="${key}"]`);
  cb.checked = !cb.checked;
  cb.dispatchEvent(new Event('change'));
}
const envOn = (key) => store.scene?.environment?.[key] !== false;

// Aide en bas de la vue et images par seconde : chacune peut être masquée (Affichage)
const VIEW_PREFS_KEY = 'arklight.view';
const viewPrefs = (() => {
  try {
    return { help: true, fps: true, bloom: true, ...JSON.parse(localStorage.getItem(VIEW_PREFS_KEY) || '{}') };
  } catch {
    return { help: true, fps: true, bloom: true };
  }
})();
function applyViewPrefs() {
  document.body.classList.toggle('hide-help', !viewPrefs.help);
  document.body.classList.toggle('hide-fps', !viewPrefs.fps);
  viewport.setBloom(viewPrefs.bloom);
  envMenu.querySelectorAll('[data-pref]').forEach((cb) => (cb.checked = !!viewPrefs[cb.dataset.pref]));
}
function setViewPref(key, on) {
  viewPrefs[key] = on;
  try {
    localStorage.setItem(VIEW_PREFS_KEY, JSON.stringify(viewPrefs));
  } catch {
    /* préférence gardée pour cette session */
  }
  applyViewPrefs();
}
envMenu.querySelectorAll('[data-pref]').forEach((cb) => cb.addEventListener('change', () => setViewPref(cb.dataset.pref, cb.checked)));
document.getElementById('hint-close').addEventListener('click', () => {
  setViewPref('help', false);
  toast('Aide masquée — Affichage › Aide en bas de la vue pour la remettre', 'info', 4000);
});
applyViewPrefs();

// ------------------------------------------------------------------ tiroir du bas
function showDrawerTab(tab) {
  drawerTabs.current = tab;
  document.querySelectorAll('.tabs [data-tab]').forEach((x) => x.classList.toggle('active', x.dataset.tab === tab));
  document.querySelectorAll('.tab-body').forEach((x) => x.classList.toggle('hidden', x.dataset.body !== tab));
  panels.open('bottom');
  if (tab === 'dmx') dmxMonitor.paint();
  if (tab === 'assistant') assistantUi.shown();
}
document.querySelectorAll('.tabs [data-tab]').forEach((b) => b.addEventListener('click', () => showDrawerTab(b.dataset.tab)));

// boutons de l'inspecteur
document.addEventListener('ark:delete', () => actions.deleteSelection());
document.addEventListener('ark:duplicate', () => actions.duplicateSelection());

// ------------------------------------------------------------------ copier / coller (projecteurs et grip)
let clip = null; // { kind: 'fixtures', items } | { kind: 'object', item } ; n : collages successifs (décalage)
const selectedObjectOnly = () => !!store.selectedObjectId && !store.selection.size;
function copySelection() {
  const fx = store.selectedFixtures();
  if (fx.length) {
    clip = { kind: 'fixtures', n: 0, items: fx.map((f) => structuredClone({ type: f.type, position: f.position, rotation: f.rotation, settings: f.settings })) };
    toast(fx.length > 1 ? `${fx.length} projecteurs copiés` : `${fx[0].name} copié`);
    return true;
  }
  const o = store.selectedObject();
  if (o) {
    clip = { kind: 'object', n: 0, item: structuredClone({ type: o.type, position: o.position, rotation: o.rotation, props: o.props }) };
    toast(`${o.name} copié`);
    return true;
  }
  toast('Rien à copier : sélectionne des projecteurs ou un élément de grip', 'error');
  return false;
}
function pasteClip() {
  if (!clip) return toast('Le presse-papiers est vide : copie d’abord des projecteurs (Ctrl+C)', 'error');
  clip.n += 1;
  const dx = 0.3 * clip.n;
  const shift = (p) => [Math.round((p[0] + dx) * 1000) / 1000, p[1], p[2]];
  if (clip.kind === 'fixtures') {
    net.send({
      t: 'fixtures.add',
      select: true,
      // nouvelle adresse libre pour chaque copie
      items: clip.items.map((f) => ({ type: f.type, position: shift(f.position), rotation: f.rotation, settings: { ...f.settings, dmxAddress: undefined } })),
    });
  } else {
    const o = clip.item;
    net.send({ t: 'object.add', type: o.type, position: shift(o.position), rotation: o.rotation, props: o.props, select: true });
  }
}

// ------------------------------------------------------------------ soutien, à propos, mises à jour, fermeture
const updateBanner = mountUpdateBanner();
document.getElementById('btn-support').addEventListener('click', () => openSupport());
document.querySelector('.brand').addEventListener('click', () => openAbout({ store, net, repo: store.repo }));

// Application de bureau : à la fermeture, proposer d'enregistrer (et de soutenir le projet)
let quitAsked = false;
async function requestQuit() {
  if (!desktop || quitAsked) return;
  quitAsked = true;
  const choice = await confirmQuit(store.scene?.name || 'Nouvelle scène', store.history.dirty && hasContent());
  quitAsked = false;
  if (choice === 0) return;
  if (choice === 1 && !(await saveScene())) return;
  window.arkDesktop.quit();
}
if (desktop) {
  window.arkDesktop.onRequestQuit(() => {
    window.arkDesktop.ackQuit();
    requestQuit();
  });
}

/** Plein écran : toute l'application, ou seulement la vue 3D / le plan (« vue seule »). */
function toggleFullscreen(viewOnly = false) {
  if (document.fullscreenElement) return document.exitFullscreen();
  const el = viewOnly ? document.querySelector('.viewport-wrap') : document.documentElement;
  el.requestFullscreen?.().catch(() => toast('Plein écran indisponible', 'error'));
  if (viewOnly) toast('Vue en plein écran — Échap pour revenir');
}
// les menus et messages restent visibles au-dessus de la vue en plein écran
document.addEventListener('fullscreenchange', () => {
  const host = document.fullscreenElement && document.fullscreenElement !== document.documentElement ? document.fullscreenElement : document.body;
  host.append(document.getElementById('toast'));
  fullviewBtn.classList.toggle('active', !!document.fullscreenElement);
});
const fullviewBtn = document.getElementById('btn-fullview');
fullviewBtn.addEventListener('click', () => toggleFullscreen(true));

// Voir par la caméra : la caméra sélectionnée, sinon la première de la scène (Échap pour sortir)
function toggleCameraView(id = null) {
  if (viewport.lookId && !id) return viewport.lookThrough(null);
  if (plot.active) plot.toggle();
  const cams = viewport.cameras();
  const target = id || cams.find((c) => c.id === store.selectedObjectId)?.id || cams[0]?.id;
  if (!target) return toast('Ajoute d’abord une caméra : bibliothèque › Décor › Plateau', 'error');
  viewport.lookThrough(target);
}
window.addEventListener(
  'keydown',
  (e) => {
    if (e.key === 'Escape' && viewport.lookId && !document.querySelector('dialog[open]')) {
      e.stopPropagation();
      viewport.lookThrough(null);
    }
  },
  true,
);
viewport.addEventListener('look', (e) => {
  document.body.classList.toggle('camera-view', !!e.detail);
  camBtn.classList.toggle('active', !!e.detail);
});

// Surbrillance : projecteurs, pieds et objets visibles même dans le noir
const highlightBtn = document.getElementById('btn-highlight');
function toggleHighlight() {
  viewport.setHighlight(!viewport.highlight);
  highlightBtn.classList.toggle('active', viewport.highlight);
}
highlightBtn.addEventListener('click', toggleHighlight);

// ------------------------------------------------------------------ commandes (menus et raccourcis)
const selectionDo = (fixtures, object) => () => (selectedObjectOnly() ? object?.() : fixtures());
const RUN = {
  'scene.new': () => newScene(),
  'scene.open': () => projects.open(),
  'scene.openFile': () => projects.openFile(),
  'scene.demo': () => openContextMenu(Math.round(window.innerWidth / 2) - 120, 60, projects.demoMenu()),
  'plans.open': () => plans.toggle(),
  'plan.next': () => plans.next(),
  'plan.prev': () => plans.prev(),
  'plan.new': () => plans.newPlan('copy'),
  'sequence.new': () => plans.newSequence('empty'),
  'projects.folder': () => projects.chooseFolder(),
  'scene.save': () => saveScene(),
  'scene.saveAs': () => saveScene({ as: true }),
  'scene.revert': () => revertScene(),
  'scene.rename': () => {
    sceneName.focus();
    sceneName.select();
  },
  'plot.print': () => plot.print(),
  'plot.png': () => plot.exportPng(),
  'plot.svg': () => plot.exportSvgFile(),
  'plot.equipment': () => plot.equipmentCsv(),
  'plot.patch': () => plot.patchCsv(),
  'plot.mvr': () => plot.exportMvr(),
  'app.quit': () => requestQuit(),

  'edit.undo': () => net.send({ t: 'history.undo' }),
  'edit.redo': () => net.send({ t: 'history.redo' }),
  'edit.copy': () => copySelection(),
  'edit.cut': () => {
    if (!copySelection()) return;
    if (selectedObjectOnly()) actions.deleteObject();
    else net.send({ t: 'fixtures.remove', ids: store.selectedIds() });
  },
  'edit.paste': () => pasteClip(),
  'edit.duplicate': selectionDo(
    () => actions.duplicateSelection(),
    () => actions.duplicateObject(),
  ),
  'edit.delete': selectionDo(
    () => actions.deleteSelection(),
    () => actions.deleteObject(),
  ),
  'select.all': () => actions.selectAll(),
  'select.none': () => (selectedObjectOnly() ? store.selectObject(null) : store.select(null)),
  'select.invert': () => actions.invertSelection(),
  'select.same': () => actions.selectSameType(),
  'group.create': () => actions.groupSelection(),
  'fixture.power': () => {
    const list = store.selectedFixtures();
    if (list.length) actions.setPower(list.map((f) => f.id), list.some((f) => !f.state.power));
  },
  'fixture.aim': () => actions.aim(),
  'fixture.drop': () => actions.drop(),
  'fixture.number': () => (store.selection.size ? actions.askFixtureNumber(store.selectedIds()) : toast('Sélectionne d’abord des projecteurs', 'error')),
  'dmx.sequence': () => (store.selection.size ? actions.openReaddress({ scope: 'selection' }) : toast('Sélectionne d’abord des projecteurs', 'error')),
  'dmx.readdress': () => actions.openReaddress(),
  'shortcuts.open': () => shortcuts.open(),

  'view.persp': () => viewport.setView('persp'),
  'view.front': () => viewport.setView('front'),
  'view.top': () => viewport.setView('top'),
  'view.side': () => viewport.setView('side'),
  'view.plot': () => plot.toggle(),
  'view.frame': () => viewport.frameSelected(),
  'view.camera': () => toggleCameraView(),
  'labels.toggle': () => toggleEnv('labels'),
  'labels.patch': () => toggleEnv('labelPatch'),
  'env.floor': () => toggleEnv('floor'),
  'env.cyc': () => toggleEnv('cyc'),
  'env.stands': () => toggleEnv('stands'),
  'view.highlight': () => toggleHighlight(),
  'view.help': () => setViewPref('help', !viewPrefs.help),
  'view.bloom': () => setViewPref('bloom', !viewPrefs.bloom),
  'view.fps': () => setViewPref('fps', !viewPrefs.fps),
  'panel.left': () => panels.toggle('left'),
  'panel.right': () => panels.toggle('right'),
  'panel.bottom': () => panels.toggle('bottom'),
  'view.fullscreen': () => toggleFullscreen(),
  'view.fullview': () => toggleFullscreen(true),
  'menu.focus': () => menubar.focus(),

  'tool.translate': () => setTool('translate'),
  'tool.rotate': () => setTool('rotate'),
  'snap.toggle': () => snapBtn.click(),
  'library.search': () => {
    panels.open('left');
    library.focusSearch();
  },
  'library.lights': () => (panels.open('left'), library.setTab('lights')),
  'library.stands': () => (panels.open('left'), library.setTab('stands')),
  'library.grip': () => (panels.open('left'), library.setTab('grip')),
  'library.decor': () => (panels.open('left'), library.setTab('decor')),
  'remote.toggle': () => remote.toggle(),
  'midi.open': () => midiPanel.open(),
  'console.full': () => consoleControl.setIntensity(store.selectedFixtures(), 1, { immediate: true }),
  'console.black': () => consoleControl.setIntensity(store.selectedFixtures(), 0, { immediate: true }),
  'console.release': () => consoleControl.release(store.selectedFixtures()),
  'console.releaseAll': () => consoleControl.release(consoleControl.heldFixtures()),
  'view.blackout': () => setBlackout(!viewport.blackout),
  'drawer.console': () => showDrawerTab('console'),
  'drawer.dmx': () => showDrawerTab('dmx'),
  'drawer.log': () => showDrawerTab('log'),
  'drawer.network': () => showDrawerTab('network'),
  'drawer.bluetooth': () => showDrawerTab('bluetooth'),
  'drawer.assistant': () => showDrawerTab('assistant'),
  'ai.voice': () => assistantUi.voiceToggle(),
  'ai.drawing': () => {
    showDrawerTab('assistant');
    assistantUi.reinterpretDrawing();
  },

  'app.update': () => openAbout({ store, net, repo: store.repo, check: true }),
  'app.source': () => store.repo && openExternal(`https://github.com/${store.repo}`),
  'app.issue': () => store.repo && openExternal(`https://github.com/${store.repo}/issues/new`),
  'app.donate': () => openSupport(),
  'app.about': () => openAbout({ store, net, repo: store.repo }),
};
const run = (id) => RUN[id]?.();

// ------------------------------------------------------------------ barre de menus
/** Élément de menu d'une commande (libellé, touche choisie par l'utilisateur). */
const item = (id, extra = {}) => ({ label: commandById.get(id).label, shortcut: keyOf(id), onClick: () => run(id), ...extra });

const menubar = mountMenubar(document.getElementById('menubar'), [
  {
    label: 'Fichier',
    items: () => [
      item('scene.new'),
      item('scene.open'),
      desktop ? item('scene.openFile') : null,
      { label: commandById.get('scene.demo').label, items: projects.demoMenu() },
      '-',
      item('scene.save'),
      item('scene.saveAs'),
      item('scene.revert', { disabled: !store.history.savedPath || !store.history.dirty }),
      item('scene.rename'),
      desktop ? item('projects.folder') : null,
      '-',
      {
        label: 'Séquences et plans',
        items: [item('plans.open'), '-', item('plan.prev', { disabled: flatPlans(store.plans).length < 2 }), item('plan.next', { disabled: flatPlans(store.plans).length < 2 }), '-', item('plan.new'), item('sequence.new')],
      },
      {
        label: 'Plan de feu',
        items: [item('plot.print'), '-', item('plot.png'), item('plot.svg'), '-', item('plot.equipment'), item('plot.patch'), '-', item('plot.mvr')],
      },
      desktop ? '-' : null,
      desktop ? item('app.quit') : null,
    ],
  },
  {
    label: 'Édition',
    items: () => {
      const hist = store.history;
      const n = store.selection.size;
      const any = n > 0 || !!store.selectedObjectId;
      const all = store.scene?.fixtures.length || 0;
      return [
        item('edit.undo', { label: hist.canUndo ? t('Annuler {0}', { 0: t(hist.undoLabel) }) : commandById.get('edit.undo').label, disabled: !hist.canUndo }),
        item('edit.redo', { label: hist.canRedo ? t('Rétablir {0}', { 0: t(hist.redoLabel) }) : commandById.get('edit.redo').label, disabled: !hist.canRedo }),
        '-',
        item('edit.cut', { disabled: !any }),
        item('edit.copy', { disabled: !any }),
        item('edit.paste', { disabled: !clip }),
        item('edit.duplicate', { disabled: !any }),
        item('edit.delete', { disabled: !any }),
        '-',
        item('select.all', { disabled: !all || n === all }),
        item('select.invert', { disabled: !all }),
        item('select.same', { disabled: !n }),
        item('select.none', { disabled: !any }),
        '-',
        item('group.create', { disabled: !n }),
        item('fixture.power', { label: store.selectedFixtures().some((f) => !f.state.power) ? 'Allumer la sélection' : 'Éteindre la sélection', disabled: !n }),
        n ? { ...actions.aimMenu(), shortcut: keyOf('fixture.aim') } : item('fixture.aim', { disabled: true }),
        item('fixture.drop', { disabled: !n }),
        n ? actions.supportMenu(store.selectedIds()) : { label: 'Pied', disabled: true },
        '-',
        item('dmx.sequence', { disabled: !n }),
        item('fixture.number', { label: n > 1 ? 'Numéroter les fixtures à la suite…' : 'Numéro de fixture…', disabled: !n }),
        item('dmx.readdress', { disabled: !all }),
        '-',
        item('shortcuts.open'),
      ];
    },
  },
  {
    label: 'Affichage',
    items: () => [
      item('view.persp', { disabled: plot.active }),
      item('view.front', { disabled: plot.active }),
      item('view.top', { disabled: plot.active }),
      item('view.side', { disabled: plot.active }),
      item('view.camera', { checked: !!viewport.lookId, disabled: !viewport.cameras().length }),
      item('view.plot', { checked: plot.active }),
      item('view.frame', { disabled: !store.selection.size || plot.active }),
      '-',
      item('view.highlight', { checked: viewport.highlight, disabled: plot.active }),
      item('labels.toggle', { checked: envOn('labels') }),
      item('labels.patch', { checked: envOn('labelPatch') }),
      item('view.bloom', { checked: viewPrefs.bloom }),
      item('view.help', { checked: viewPrefs.help }),
      item('view.fps', { checked: viewPrefs.fps }),
      {
        label: 'Studio',
        items: [item('env.floor', { checked: envOn('floor') }), item('env.cyc', { checked: envOn('cyc') }), item('env.stands', { checked: envOn('stands') })],
      },
      '-',
      item('panel.left', { checked: !panels.folded('left') }),
      item('panel.right', { checked: !panels.folded('right') }),
      item('panel.bottom', { checked: !panels.folded('bottom') }),
      '-',
      item('view.fullscreen', { checked: document.fullscreenElement === document.documentElement }),
      item('view.fullview', { checked: !!document.fullscreenElement && document.fullscreenElement !== document.documentElement }),
    ],
  },
  {
    label: 'Outils',
    items: () => {
      const bottomOpen = !panels.folded('bottom');
      return [
        item('tool.translate', { radio: currentTool === 'translate' }),
        item('tool.rotate', { radio: currentTool === 'rotate' }),
        item('snap.toggle', { checked: snapBtn.classList.contains('active') }),
        '-',
        item('library.search'),
        { label: 'Bibliothèque', items: ['lights', 'stands', 'grip', 'decor'].map((t) => item(`library.${t}`, { radio: library.tab === t })) },
        '-',
        item('remote.toggle', { checked: remote.open }),
        item('midi.open'),
        item('view.blackout', { checked: !!viewport.blackout }),
        '-',
        ...['console', 'dmx', 'log', 'network', 'bluetooth'].map((t) => item(`drawer.${t}`, { radio: bottomOpen && drawerTabs.current === t })),
        '-',
        item('drawer.assistant', { radio: bottomOpen && drawerTabs.current === 'assistant' }),
        item('ai.drawing'),
      ];
    },
  },
  {
    label: 'Aide',
    items: () => [
      item('shortcuts.open'),
      '-',
      item('app.update'),
      store.repo ? item('app.source') : null,
      store.repo ? item('app.issue') : null,
      '-',
      item('app.donate', { label: '❤ Faire un don…' }),
      item('app.about'),
    ],
  },
  {
    // sélecteur de langue : l'interface se recharge, le projet reste ouvert
    // « Automatique » (par défaut) : la langue de l'ordinateur, relue à chaque lancement
    label: 'Langue',
    items: () => [
      { label: t('Automatique — langue de l’ordinateur ({0})', { 0: LANGS.find((l) => l.id === systemLanguage)?.label }), radio: langChoice() === 'auto', onClick: () => setLang('auto') },
      '-',
      ...LANGS.map((l) => ({ label: l.label, radio: langChoice() === l.id, onClick: () => setLang(l.id) })),
    ],
  },
]);

// ------------------------------------------------------------------ raccourcis clavier (personnalisables)
const TYPING_TAGS = ['INPUT', 'SELECT', 'TEXTAREA'];
// Pendant une saisie, ces combinaisons restent au champ de texte (copier, coller, annuler…)
const TEXT_EDITING = /^Ctrl\+(Shift\+)?(A|C|V|X|Z|Y|ArrowLeft|ArrowRight|ArrowUp|ArrowDown|Home|End|Backspace|Delete)$/;

window.addEventListener('keydown', (e) => {
  const id = shortcuts.match(e);
  if (!id) return;
  const combo = shortcuts.combo(e);
  if (shortcuts.isReserved(combo)) return; // navigateur : la combinaison lui appartient
  if (document.querySelector('dialog[open]') || isContextMenuOpen() || projects.isOpen) return;
  const el = document.activeElement;
  const typing = TYPING_TAGS.includes(el?.tagName) || el?.isContentEditable;
  // dans un champ, seules les combinaisons avec Ctrl (hors édition de texte) et les touches F passent
  if (typing && !((combo.startsWith('Ctrl+') && !TEXT_EDITING.test(combo)) || /^F\d+$/.test(combo))) return;
  // dans le plan 2D, la poignée 3D n'a pas de sens
  if (plot.active && id.startsWith('tool.')) return;
  e.preventDefault();
  // parler à l'assistant : touche tenue (relâchée plus bas), sans répétition automatique
  if (id === 'ai.voice') {
    if (!e.repeat) assistantUi.voicePress(e.code);
    return;
  }
  run(id);
});
window.addEventListener('keyup', (e) => assistantUi.voiceRelease(e.code));

// Clic droit sur un bouton des barres d'outils : changer son raccourci directement
function shortcutMenu(id) {
  const c = commandById.get(id);
  if (!c) return [];
  const keys = shortcuts.keys(id);
  return [
    { header: c.label },
    { label: keys.length ? `Raccourci : ${keys.map(keyLabel).join(' ou ')}` : 'Pas encore de raccourci', disabled: true },
    { label: keys.length ? 'Changer le raccourci…' : 'Ajouter un raccourci…', onClick: () => shortcuts.quickEdit(id) },
    keys.length
      ? {
          label: 'Retirer le raccourci',
          onClick: () => {
            shortcuts.clearKeys(id);
            toast(`« ${c.label} » n’a plus de raccourci`);
          },
        }
      : null,
    '-',
    { label: 'Tous les raccourcis clavier…', shortcut: keyOf('shortcuts.open'), onClick: () => shortcuts.open(c.label) },
  ];
}
document.addEventListener('contextmenu', (e) => {
  const el = e.target.closest?.('.viewbar [data-cmd], .view-corner[data-cmd]');
  if (!el) return;
  e.preventDefault();
  openContextMenu(e.clientX, e.clientY, shortcutMenu(el.dataset.cmd));
});

// Infobulles des boutons : ce qu'ils font, leur touche, et le clic droit pour la changer
const TIPS = {
  'tool.translate': 'Déplacer',
  'tool.rotate': 'Pivoter',
  'snap.toggle': 'Aimant : pas de 10 cm et de 5° — Maj pendant le glisser : réglage libre, au degré près',
  'view.persp': 'Vue 3D (perspective)',
  'view.front': 'Vue de face',
  'view.top': 'Vue de dessus',
  'view.side': 'Vue de côté',
  'view.camera': 'Voir par la caméra de la scène',
  'view.frame': 'Cadrer la sélection',
  'view.highlight': 'Surbrillance : projecteurs, pieds et objets visibles même dans une scène très sombre',
  'remote.toggle': 'Télécommandes infrarouges Astera ARC1 / ARC3',
  'view.plot': 'Plan de feu 2D : vue de dessus, formes, annotations, export avec la liste du matériel',
  'view.fullview': 'Vue en plein écran — Échap pour revenir',
};

/** Les infobulles et l'aide de la vue 3D affichent les touches choisies. */
function applyShortcutLabels() {
  const k = (id) => keyOf(id);
  const withKey = (text, id) => (k(id) ? `${text} (${k(id)})` : text);
  for (const el of document.querySelectorAll('[data-cmd]')) {
    const id = el.dataset.cmd;
    el.title = t('{0}\nClic droit : changer le raccourci', { 0: withKey(t(el.dataset.tip || TIPS[id] || commandById.get(id)?.label || ''), id) });
  }
  const titles = {
    '#sl-haze': 'Brume : densité (faisceaux visibles)',
    '#sl-exposure': 'Exposition de la caméra, en diaphragmes (IL) : baisse-la face à un 2K tungstène ou un HMI',
    '#sl-ambient': 'Lumière d’ambiance du studio',
    '#btn-env': 'Affichage : sol, cyclo, pieds, noms et adresses des projecteurs, aide',
  };
  panels.refreshTitles();
  for (const [sel, title] of Object.entries(titles)) document.querySelector(sel)?.setAttribute('title', t(title));
  // aide en bas de la vue (dans la langue de l'interface, avec les touches choisies)
  const part = (ids, fr) => (ids.every((id) => k(id)) ? t(fr, { 0: ids.map(k).join('/') }) : null);
  document.getElementById('viewport-hint').textContent = [
    t('Clic : sélectionner'),
    t('Ctrl+clic : sélection multiple'),
    t('Clic droit : menu'),
    t('Glisser : orbiter'),
    t('Clic droit glissé : décaler'),
    t('Molette : zoom'),
    part(['tool.translate', 'tool.rotate'], '{0} : déplacer/pivoter'),
    k('view.persp') && k('view.side') ? t('{0}–{1} : vues', { 0: k('view.persp'), 1: k('view.side') }) : null,
    part(['view.plot'], '{0} : plan 2D'),
    part(['edit.undo'], '{0} : annuler'),
    part(['view.frame'], '{0} : cadrer'),
    part(['shortcuts.open'], '{0} : raccourcis'),
  ]
    .filter(Boolean)
    .join(' · ');
}
shortcuts.onChange(applyShortcutLabels);
applyShortcutLabels();

// Accès depuis la console du navigateur (débogage)
window.ark = { store, net, viewport, actions, plot, shortcuts, run, panels, projects, midi };
