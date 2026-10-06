// Point d'entrée de l'interface ARKLIGHT.
import { Net } from './net.js';
import { Store } from './store.js';
import { Viewport } from './viewport/viewport.js';
import { h, toast } from './ui/dom.js';
import { mountLibrary } from './ui/library.js';
import { mountSceneList } from './ui/scene-list.js';
import { mountInspector } from './ui/inspector.js';
import { mountTestConsole } from './ui/test-console.js';
import { mountDmxMonitor } from './ui/dmx-monitor.js';
import { mountLog, mountNetwork } from './ui/network.js';
import { mountBluetooth } from './ui/bluetooth.js';
import { OnboardRegistry, createControlPanel } from './ui/control-panel.js';
import { mountRemote } from './ui/remote.js';
import { isContextMenuOpen, openContextMenu } from './ui/context-menu.js';
import { mountMenubar } from './ui/menubar.js';
import { askSaveChanges, askText } from './ui/dialogs.js';
import { createActions } from './actions.js';
import { mountPlot } from './plot/plot-view.js';
import { commandById } from './commands.js';
import { createShortcuts } from './shortcuts.js';
import { confirmQuit, mountUpdateBanner, openAbout, openExternal, openSupport } from './ui/support.js';

const desktop = !!window.arkDesktop;
const store = new Store();
const net = new Net();
const shortcuts = createShortcuts({ desktop });
const keyOf = (id) => shortcuts.label(id);
const viewport = new Viewport(document.getElementById('viewport'), store, net);
const drawer = document.getElementById('drawer');

// ------------------------------------------------------------------ messages serveur
let pendingSelectType = null;
let pendingSelectObject = null;
net.addEventListener('message', (e) => {
  const msg = e.detail;
  switch (msg.t) {
    case 'hello':
      store.clientId = msg.clientId;
      store.version = msg.version;
      store.repo = msg.repo;
      store.config = msg.config || {};
      store.setScene(msg.scene);
      if (msg.history) store.setHistory(msg.history);
      for (const l of msg.logs || []) store.addLog(l);
      applySceneMeta();
      break;
    case 'scene':
      store.setScene(msg.scene);
      applySceneMeta();
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
    case 'config':
      store.config = msg.config;
      store.emit('config', msg.config);
      break;
    case 'scene.list':
      renderOpenList(msg.items);
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
    net.send({ t: 'object.add', type: type.id, ...viewport.gripPlacementAt(type, 1.2, 2.2) });
  },
});
const onboard = new OnboardRegistry({ store, net });
viewport.onboard = onboard;
const actions = createActions({ store, net, viewport, onboard, keyOf });
mountSceneList({ store, viewport, actions });
mountInspector({ store, net, viewport, onboard, actions, controlPanel: createControlPanel({ registry: onboard }) });
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
document.getElementById('btn-remote').addEventListener('click', () => remote.toggle());
mountTestConsole({ store, net });
const drawerTabs = { current: 'console' };
const dmxMonitor = mountDmxMonitor({ store, actions, isVisible: () => drawerTabs.current === 'dmx' && !drawer.classList.contains('collapsed') });
mountLog({ store });
mountNetwork({ store, net });
mountBluetooth({ store, net });

// Panneaux de gauche et de droite masquables (menu Affichage), mémorisés dans le navigateur
const PANELS_KEY = 'arklight.panels';
const panels = (() => {
  try {
    return { left: true, right: true, ...JSON.parse(localStorage.getItem(PANELS_KEY) || '{}') };
  } catch {
    return { left: true, right: true };
  }
})();
function applyPanels() {
  document.body.classList.toggle('no-left', !panels.left);
  document.body.classList.toggle('no-right', !panels.right);
}
function togglePanel(which) {
  panels[which] = !panels[which];
  applyPanels();
  try {
    localStorage.setItem(PANELS_KEY, JSON.stringify(panels));
  } catch {
    /* stockage indisponible */
  }
}
applyPanels();

// ------------------------------------------------------------------ scène : nom, enregistrement, ouverture
const sceneName = document.getElementById('scene-name');
const dirtyDot = document.getElementById('scene-dirty');
function applySceneMeta() {
  sceneName.value = store.scene?.name || '';
  applyEnvControls();
  applyTitle();
}
function applyTitle() {
  const dirty = store.history.dirty && hasContent();
  dirtyDot.classList.toggle('hidden', !dirty);
  document.title = `${dirty ? '● ' : ''}${store.scene?.name || 'Nouvelle scène'} — ARKLIGHT`;
}
store.addEventListener('history', applyTitle);
sceneName.addEventListener('change', () => net.send({ t: 'scene.rename', name: sceneName.value }));
sceneName.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' || e.key === 'Escape') {
    if (e.key === 'Escape') sceneName.value = store.scene?.name || '';
    sceneName.blur();
  }
});

/** La scène contient quelque chose qui mérite d'être enregistré. */
const hasContent = () => !!(store.scene && (store.scene.fixtures.length || store.scene.objects.length || store.scene.plot.items.length));

/** Enregistre la scène. Demande un nom la première fois (ou avec « Enregistrer sous »). */
async function saveScene({ as = false } = {}) {
  let name = (sceneName.value || store.scene.name || '').trim();
  if (as || !store.history.savedName) {
    name = await askText({
      title: as ? 'Enregistrer sous' : 'Enregistrer la scène',
      label: 'Nom de la scène',
      value: name === 'Nouvelle scène' && !as ? '' : name,
      ok: 'Enregistrer',
      note: 'Les scènes sont rangées dans le dossier des scènes d’ARKLIGHT.',
    });
    if (!name) return false;
    if (name !== store.history.savedName) {
      const reply = waitFor('scene.exists', { filter: (m) => m.name === name });
      net.send({ t: 'scene.exists', name });
      if ((await reply)?.exists && !confirm(`Une scène « ${name} » existe déjà. La remplacer ?`)) return false;
    }
  }
  const done = waitFor('scene.list');
  net.send({ t: 'scene.save', name });
  if (!(await done)) return false;
  toast(`Scène enregistrée : ${name}`);
  return true;
}

/** Avant de quitter la scène : proposer d'enregistrer. false : l'utilisateur annule. */
async function confirmDiscard(action) {
  if (!store.history.dirty || !hasContent()) return true;
  const r = await askSaveChanges(store.scene.name, action);
  if (r === 'cancel') return false;
  return r === 'save' ? saveScene() : true;
}

async function newScene() {
  if (await confirmDiscard('créer une nouvelle scène')) net.send({ t: 'scene.new' });
}

const dlgOpen = document.getElementById('dlg-open');
function openSceneDialog() {
  net.send({ t: 'scene.list' });
  dlgOpen.showModal();
}
document.getElementById('open-cancel').addEventListener('click', () => dlgOpen.close());
function renderOpenList(items) {
  const list = document.getElementById('open-list');
  list.replaceChildren(
    ...(items.length
      ? items.map((name) =>
          h(
            'div',
            { class: `open-item${name === store.history.savedName ? ' current' : ''}` },
            h(
              'button',
              {
                onclick: async () => {
                  dlgOpen.close();
                  if (await confirmDiscard('ouvrir une autre scène')) net.send({ t: 'scene.load', name });
                },
              },
              name,
            ),
            h(
              'button',
              {
                title: 'Supprimer',
                onclick: () => confirm(`Supprimer la scène « ${name} » ?`) && net.send({ t: 'scene.delete', name }),
              },
              '🗑',
            ),
          ),
        )
      : [h('div', { class: 'empty' }, 'Aucune scène enregistrée.')]),
  );
}

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

const envInputs = {
  haze: document.getElementById('env-haze'),
  exposure: document.getElementById('env-exposure'),
  ambient: document.getElementById('env-ambient'),
};
function sendEnv(changes) {
  Object.assign(store.scene.environment, changes);
  viewport.applyEnvironment(store.scene.environment);
  clearTimeout(sendEnv.t);
  sendEnv.t = setTimeout(() => net.send({ t: 'env.update', changes }), 120);
}
// L'exposition se règle en diaphragmes (IL) : 0 IL = exposition 1, chaque cran double ou divise la lumière
const toEnv = { exposure: (stops) => Math.pow(2, stops) };
const fromEnv = { exposure: (x) => Math.log2(x || 1) };
const expoVal = document.getElementById('env-exposure-val');
function showExposure() {
  const stops = Number(envInputs.exposure.value);
  expoVal.textContent = `${stops > 0 ? '+' : stops < 0 ? '−' : ''}${Math.abs(stops).toFixed(1).replace('.', ',').replace(',0', '')} IL`;
}
for (const [k, input] of Object.entries(envInputs)) {
  input.addEventListener('input', () => {
    sendEnv({ [k]: toEnv[k] ? toEnv[k](Number(input.value)) : Number(input.value) });
    if (k === 'exposure') showExposure();
  });
}
const envMenu = document.getElementById('env-menu');
document.getElementById('btn-env').addEventListener('click', () => envMenu.classList.toggle('hidden'));
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

// ------------------------------------------------------------------ tiroir du bas
function showDrawerTab(tab) {
  drawerTabs.current = tab;
  document.querySelectorAll('.tabs [data-tab]').forEach((x) => x.classList.toggle('active', x.dataset.tab === tab));
  document.querySelectorAll('.tab-body').forEach((x) => x.classList.toggle('hidden', x.dataset.body !== tab));
  if (drawer.classList.contains('collapsed')) toggleDrawer();
  if (tab === 'dmx') dmxMonitor.paint();
}
document.querySelectorAll('.tabs [data-tab]').forEach((b) => b.addEventListener('click', () => showDrawerTab(b.dataset.tab)));
const toggleBtn = document.getElementById('drawer-toggle');
function toggleDrawer() {
  drawer.classList.toggle('collapsed');
  document.documentElement.style.setProperty('--drawer-h', drawer.classList.contains('collapsed') ? '34px' : '230px');
  toggleBtn.textContent = drawer.classList.contains('collapsed') ? '▴' : '▾';
}
toggleBtn.addEventListener('click', toggleDrawer);

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

function toggleFullscreen() {
  if (document.fullscreenElement) document.exitFullscreen();
  else document.documentElement.requestFullscreen?.().catch(() => toast('Plein écran indisponible', 'error'));
}

// ------------------------------------------------------------------ commandes (menus et raccourcis)
const selectionDo = (fixtures, object) => () => (selectedObjectOnly() ? object?.() : fixtures());
const RUN = {
  'scene.new': () => newScene(),
  'scene.open': () => openSceneDialog(),
  'scene.save': () => saveScene(),
  'scene.saveAs': () => saveScene({ as: true }),
  'scene.rename': () => {
    sceneName.focus();
    sceneName.select();
  },
  'plot.print': () => plot.print(),
  'plot.png': () => plot.exportPng(),
  'plot.svg': () => plot.exportSvgFile(),
  'plot.equipment': () => plot.equipmentCsv(),
  'plot.patch': () => plot.patchCsv(),
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
  'dmx.sequence': () => (store.selection.size ? actions.openReaddress({ scope: 'selection' }) : toast('Sélectionne d’abord des projecteurs', 'error')),
  'dmx.readdress': () => actions.openReaddress(),
  'shortcuts.open': () => shortcuts.open(),

  'view.persp': () => viewport.setView('persp'),
  'view.front': () => viewport.setView('front'),
  'view.top': () => viewport.setView('top'),
  'view.side': () => viewport.setView('side'),
  'view.plot': () => plot.toggle(),
  'view.frame': () => viewport.frameSelected(),
  'labels.toggle': () => toggleEnv('labels'),
  'labels.patch': () => toggleEnv('labelPatch'),
  'env.floor': () => toggleEnv('floor'),
  'env.cyc': () => toggleEnv('cyc'),
  'env.actor': () => toggleEnv('actor'),
  'env.stands': () => toggleEnv('stands'),
  'panel.left': () => togglePanel('left'),
  'panel.right': () => togglePanel('right'),
  'panel.bottom': () => toggleDrawer(),
  'view.fullscreen': () => toggleFullscreen(),
  'menu.focus': () => menubar.focus(),

  'tool.translate': () => setTool('translate'),
  'tool.rotate': () => setTool('rotate'),
  'snap.toggle': () => snapBtn.click(),
  'library.search': () => {
    if (!panels.left) togglePanel('left');
    library.focusSearch();
  },
  'library.lights': () => (panels.left || togglePanel('left'), library.setTab('lights')),
  'library.stands': () => (panels.left || togglePanel('left'), library.setTab('stands')),
  'library.grip': () => (panels.left || togglePanel('left'), library.setTab('grip')),
  'remote.toggle': () => remote.toggle(),
  'drawer.console': () => showDrawerTab('console'),
  'drawer.dmx': () => showDrawerTab('dmx'),
  'drawer.log': () => showDrawerTab('log'),
  'drawer.network': () => showDrawerTab('network'),
  'drawer.bluetooth': () => showDrawerTab('bluetooth'),

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
      '-',
      item('scene.save'),
      item('scene.saveAs'),
      item('scene.rename'),
      '-',
      {
        label: 'Plan de feu',
        items: [item('plot.print'), '-', item('plot.png'), item('plot.svg'), '-', item('plot.equipment'), item('plot.patch')],
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
        item('edit.undo', { label: hist.canUndo ? `Annuler ${hist.undoLabel}` : 'Annuler', disabled: !hist.canUndo }),
        item('edit.redo', { label: hist.canRedo ? `Rétablir ${hist.redoLabel}` : 'Rétablir', disabled: !hist.canRedo }),
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
        item('fixture.aim', { disabled: !n }),
        item('fixture.drop', { disabled: !n }),
        '-',
        item('dmx.sequence', { disabled: !n }),
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
      item('view.plot', { checked: plot.active }),
      item('view.frame', { disabled: !store.selection.size || plot.active }),
      '-',
      item('labels.toggle', { checked: envOn('labels') }),
      item('labels.patch', { checked: envOn('labelPatch') }),
      {
        label: 'Décor',
        items: [item('env.floor', { checked: envOn('floor') }), item('env.cyc', { checked: envOn('cyc') }), item('env.actor', { checked: envOn('actor') }), item('env.stands', { checked: envOn('stands') })],
      },
      '-',
      item('panel.left', { checked: panels.left }),
      item('panel.right', { checked: panels.right }),
      item('panel.bottom', { checked: !drawer.classList.contains('collapsed') }),
      '-',
      item('view.fullscreen', { checked: !!document.fullscreenElement }),
    ],
  },
  {
    label: 'Outils',
    items: () => {
      const bottomOpen = !drawer.classList.contains('collapsed');
      return [
        item('tool.translate', { radio: currentTool === 'translate' }),
        item('tool.rotate', { radio: currentTool === 'rotate' }),
        item('snap.toggle', { checked: snapBtn.classList.contains('active') }),
        '-',
        item('library.search'),
        { label: 'Bibliothèque', items: ['lights', 'stands', 'grip'].map((t) => item(`library.${t}`, { radio: library.tab === t })) },
        '-',
        item('remote.toggle', { checked: remote.open }),
        '-',
        ...['console', 'dmx', 'log', 'network', 'bluetooth'].map((t) => item(`drawer.${t}`, { radio: bottomOpen && drawerTabs.current === t })),
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
  if (document.querySelector('dialog[open]') || isContextMenuOpen()) return;
  const el = document.activeElement;
  const typing = TYPING_TAGS.includes(el?.tagName) || el?.isContentEditable;
  // dans un champ, seules les combinaisons avec Ctrl (hors édition de texte) et les touches F passent
  if (typing && !((combo.startsWith('Ctrl+') && !TEXT_EDITING.test(combo)) || /^F\d+$/.test(combo))) return;
  // dans le plan 2D, la poignée 3D n'a pas de sens
  if (plot.active && id.startsWith('tool.')) return;
  e.preventDefault();
  run(id);
});

/** Les infobulles et l'aide de la vue 3D affichent les touches choisies. */
function applyShortcutLabels() {
  const k = (id) => keyOf(id);
  const withKey = (text, id) => (k(id) ? `${text} (${k(id)})` : text);
  const titles = {
    '[data-tool="translate"]': withKey('Déplacer', 'tool.translate'),
    '[data-tool="rotate"]': withKey('Pivoter', 'tool.rotate'),
    '[data-view="persp"]': withKey('Vue perspective', 'view.persp'),
    '[data-view="front"]': withKey('Vue de face', 'view.front'),
    '[data-view="top"]': withKey('Vue de dessus', 'view.top'),
    '[data-view="side"]': withKey('Vue de côté', 'view.side'),
    '#btn-plot': `${withKey('Plan de feu 2D', 'view.plot')} : vue de dessus, formes, annotations, export avec la liste du matériel`,
    '#btn-snap': withKey('Aimantation 10 cm / 15°', 'snap.toggle'),
    '#btn-remote': withKey('Télécommandes infrarouges Astera ARC1 / ARC3', 'remote.toggle'),
  };
  for (const [sel, title] of Object.entries(titles)) document.querySelector(sel)?.setAttribute('title', title);
  const part = (ids, text) => (ids.every((id) => k(id)) ? `${ids.map(k).join('/')} : ${text}` : null);
  document.getElementById('viewport-hint').textContent = [
    'Clic : sélectionner',
    'Ctrl+clic : sélection multiple',
    'Clic droit : menu',
    'Glisser : orbiter',
    'Clic droit glissé : décaler',
    'Molette : zoom',
    part(['tool.translate', 'tool.rotate'], 'déplacer/pivoter'),
    k('view.persp') && k('view.side') ? `${k('view.persp')}–${k('view.side')} : vues` : null,
    part(['view.plot'], 'plan 2D'),
    part(['edit.undo'], 'annuler'),
    part(['view.frame'], 'cadrer'),
    part(['shortcuts.open'], 'raccourcis'),
  ]
    .filter(Boolean)
    .join(' · ');
}
shortcuts.onChange(applyShortcutLabels);
applyShortcutLabels();

// Accès depuis la console du navigateur (débogage)
window.ark = { store, net, viewport, actions, plot, shortcuts, run };
