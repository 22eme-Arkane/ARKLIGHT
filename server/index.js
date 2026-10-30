// ARKLIGHT — serveur central du simulateur.
//  - sert l'interface web (http://localhost:8686)
//  - reçoit le DMX (Art-Net / sACN) des applis et consoles
//  - tient l'état de la scène et le diffuse aux interfaces par WebSocket
import { createServer } from 'node:http';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { WebSocketServer } from 'ws';
import { createStaticHandler } from './static.js';
import { SceneStore } from './scene-store.js';
import { ArtNetInput } from './dmx/artnet.js';
import { SacnInput } from './dmx/sacn.js';
import { UniverseStore } from './dmx/universes.js';
import { ipv4Interfaces } from './dmx/net-utils.js';
import { BridgeHub } from './ble/bridge-hub.js';
import { testEcho } from './ble/emulators/test-echo.js';
import { asteraBox } from './ble/emulators/astera-box.js';
import { APP_VERSION, UPDATE_REPO, checkForUpdate } from './update-check.js';
import { History, UNDOABLE } from './history.js';
import { ProjectLibrary, cleanThumbnail, projectName, safeName } from './projects.js';
import { buildDemo } from './demo-project.js';
import { getDemo } from '../shared/demos.js';
import { translate } from '../shared/i18n.js';
import { RdmResponder } from '../shared/rdm.js';
import { getFixtureType, getMode } from '../shared/fixtures/index.js';
import { Assistant } from './ai/assistant.js';
import { KeyStore } from './ai/keys.js';
import { PROVIDERS, listModels, providerUrl, useAiodos } from './ai/providers.js';
import { createAiodos } from './ai/aiodos.js';
import { SUGGESTED_MODELS, installOllama, ollamaStatus, pickModel, pullModel, startOllama } from './ai/ollama.js';
import { VOCABULARY, installVoice, transcribe, voiceStatus } from './ai/voice.js';
import { isLocalClient } from './local-client.js';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
// Données de l'utilisateur (scènes, réglages, images de fond) : à côté du code en développement,
// dans « Documents\ARKLIGHT » pour l'application installée (variable ARKLIGHT_DATA).
const DATA = process.env.ARKLIGHT_DATA || ROOT;
mkdirSync(DATA, { recursive: true });
const CONFIG_FILE = join(DATA, 'config.json');

const DEFAULT_CONFIG = {
  port: 8686,
  artnetUniverseOffset: 1, // Art-Net port-address 0 -> univers 1
  nodeShortName: 'ARKLIGHT',
  nodeLongName: 'ARKLIGHT - simulateur lumiere',
};

function loadConfig() {
  try {
    return { ...DEFAULT_CONFIG, ...JSON.parse(readFileSync(CONFIG_FILE, 'utf8')) };
  } catch {
    return { ...DEFAULT_CONFIG };
  }
}

const config = loadConfig();
const PORT = Number(process.env.ARKLIGHT_PORT || config.port);

const saveConfig = () => writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2));
const store = new SceneStore(join(DATA, 'scenes'));
// projets : dossier au choix (disque externe…), récents ouverts d'ailleurs
const library = new ProjectLibrary({ defaultDir: join(DATA, 'scenes'), config, onConfig: saveConfig });
store.projectsDir = library.root;
store.lang = config.lang === 'en' ? 'en' : 'fr'; // noms des nouveaux objets, projet d'exemple
if (store.savedPath) library.allow(store.savedPath);
const ASSETS = join(DATA, 'scenes', 'assets'); // images de fond des plans de feu
mkdirSync(ASSETS, { recursive: true });
const universes = new UniverseStore();
// annuler / rétablir : une pile par plan (changer de plan ne mélange pas les étapes)
const histories = new Map();
let history = null;
function useHistory() {
  const id = store.current.plan;
  if (!histories.has(id)) histories.set(id, new History({ limit: 100 }));
  history = histories.get(id);
}
function resetHistories() {
  histories.clear();
  useHistory();
}
useHistory();

// ---------------------------------------------------------------- assistant IA
// Réglages (config.json) : fournisseur, modèle, adresses ; clés API à part (ai-keys.json) : dans le
// profil de l'application de bureau (ARKLIGHT_KEYS, voir desktop/keys-file.js), jamais dans
// Documents ; à côté du code en développement (fichier ignoré par git).
const AI_DEFAULTS = { provider: 'ollama', model: '', urls: {} };
const VOICE_DEFAULTS = { enabled: false, model: 'base', lang: 'auto', autoSend: true };
config.ai = { ...AI_DEFAULTS, ...(config.ai && typeof config.ai === 'object' ? config.ai : {}) };
config.ai.voice = { ...VOICE_DEFAULTS, ...(config.ai.voice && typeof config.ai.voice === 'object' ? config.ai.voice : {}) };
const aiKeys = new KeyStore(process.env.ARKLIGHT_KEYS || join(DATA, 'ai-keys.json'));
const assistant = new Assistant({ store, settings: () => config.ai, keyFor: (p) => aiKeys.get(p) });
let aiTask = null; // téléchargement en cours (Ollama, modèle) : { kind, ctrl }
// AIODOS (IA locale d'ARKANE) : accès accordé dans AIODOS, clé gardée avec les autres
const aiodos = createAiodos({ keys: aiKeys, version: APP_VERSION, lang: () => config.lang });
useAiodos(aiodos);

/** État de l'assistant pour les réglages de l'interface (jamais les clés elles-mêmes). */
async function aiState() {
  const [st, ai] = await Promise.all([ollamaStatus(providerUrl('ollama', config.ai)), aiodos.status()]);
  // pas encore de modèle choisi : le meilleur modèle local installé
  if (config.ai.provider === 'ollama' && !config.ai.model && st.models.length) {
    config.ai.model = pickModel(st.models);
    if (config.ai.model) saveConfig();
  }
  if (config.ai.provider === 'aiodos' && !config.ai.model && ai.state === 'ready') {
    config.ai.model = await aiodos.pickModel().catch(() => '');
    if (config.ai.model) saveConfig();
  }
  // le code d'une demande d'accès ne part qu'aux interfaces de cet ordinateur (ai.aiodos.code)
  const { code, ...aiodosState } = ai;
  void code;
  return {
    t: 'ai.state',
    config: config.ai,
    providers: Object.entries(PROVIDERS).map(([id, p]) => ({ id, label: p.label, key: !!p.key, local: !!p.local, url: providerUrl(id, config.ai), defaultUrl: p.url, defaultModel: p.defaultModel || '' })),
    keys: aiKeys.list(),
    encrypted: aiKeys.encrypted,
    ollama: st,
    aiodos: aiodosState,
    suggested: SUGGESTED_MODELS,
    voice: { ...voiceStatus(DATA), platform: process.platform },
    busy: assistant.busy,
    task: aiTask ? aiTask.kind : null,
  };
}

// ---------------------------------------------------------------- journal
const logs = [];
function log(level, text) {
  const entry = { t: 'log', level, text, at: Date.now() };
  logs.push(entry);
  if (logs.length > 200) logs.shift();
  broadcast(entry);
  console.log(`[${level}] ${text}`);
}

// ---------------------------------------------------------------- entrées DMX
const artnet = new ArtNetInput({
  shortName: config.nodeShortName,
  longName: config.nodeLongName,
  getPortAddresses: () => store.usedUniverses().map((u) => u - config.artnetUniverseOffset).filter((pa) => pa >= 0),
});
const sacn = new SacnInput();

const seenPollers = new Map();
artnet.on('listening', (p) => log('info', `Art-Net : écoute sur le port UDP ${p}`));
artnet.on('error', (e) => log('error', `Art-Net : ${e.message}`));
artnet.on('poll', ({ source }) => {
  const last = seenPollers.get(source) || 0;
  if (Date.now() - last > 60_000) log('info', `Art-Net : découverte (ArtPoll) depuis ${source} — le simulateur a répondu`);
  seenPollers.set(source, Date.now());
});
artnet.on('dmx', ({ portAddress, data, source }) => {
  universes.input({
    universe: portAddress + config.artnetUniverseOffset,
    data,
    key: `artnet|${source}`,
    protocol: 'artnet',
    address: source,
  });
});

// RDM : les applis (Luminair…) découvrent les projecteurs de la scène, lisent leur modèle et leurs
// canaux, et peuvent changer leur adresse, leur mode ou leur nom, ou les faire clignoter.
const rdm = new RdmResponder({
  getFixtures: () => store.scene.fixtures,
  onSet(f, changes, what) {
    const before = { address: f.settings.dmxAddress, name: f.name };
    const updated = store.updateFixture(f.id, changes);
    if (!updated) return;
    broadcast({ t: 'fixture.updated', fixture: updated, origin: 0 });
    if (what === 'address') log('info', `RDM : ${updated.name} → adresse ${updated.settings.dmxAddress} (avant ${before.address})`);
    else if (what === 'mode') log('info', `RDM : ${updated.name} → mode ${getMode(getFixtureType(updated.type), updated.settings.dmxMode)?.name || updated.settings.dmxMode}`);
    else log('info', `RDM : « ${before.name} » renommé « ${updated.name} »`);
    syncSacnUniverses();
    broadcastHistory();
  },
  onIdentify(f, on) {
    broadcast({ t: 'fixture.identify', id: f.id, on });
    log('info', on ? `RDM : identification de ${f.name}` : `RDM : fin de l’identification de ${f.name}`);
  },
});
artnet.rdm = {
  tod: (pa) => rdm.tod(pa + config.artnetUniverseOffset),
  handle: (pa, data) => rdm.handle(pa + config.artnetUniverseOffset, data),
};
let lastDiscovery = '';
artnet.on('rdm-discovery', ({ source, devices }) => {
  const key = `${source}|${devices}`;
  if (key !== lastDiscovery) log('info', `RDM : découverte demandée par ${source} — ${devices} projecteur(s) annoncé(s)`);
  lastDiscovery = key;
});

sacn.on('listening', (p) => log('info', `sACN : écoute sur le port UDP ${p}`));
sacn.on('error', (e) => log('error', `sACN : ${e.message}`));
sacn.on('dmx', ({ universe, data, priority, source, sourceName, cid }) => {
  universes.input({ universe, data, priority, key: `sacn|${cid}`, protocol: 'sacn', address: source, name: sourceName });
});
sacn.on('terminated', ({ universe, cid }) => universes.terminate(universe, `sacn|${cid}`));

function syncSacnUniverses() {
  sacn.setUniverses(store.usedUniverses());
}

// ---------------------------------------------------------------- Bluetooth (fausse AsteraBox)
const ble = new BridgeHub({ root: ROOT, log, emulators: { astera: asteraBox, test: testEcho } });
let bleStatusTimer = null;
ble.on('status', () => {
  clearTimeout(bleStatusTimer);
  bleStatusTimer = setTimeout(() => broadcast({ t: 'ble.status', ...ble.status() }), 50);
});
ble.on('traffic', (entry) => broadcast({ t: 'ble.traffic', entry }));

// ---------------------------------------------------------------- WebSocket
const uiClients = new Set();
let nextClientId = 1;

function send(ws, obj) {
  if (ws.readyState === 1) ws.send(JSON.stringify(obj));
}

function broadcast(obj) {
  const s = JSON.stringify(obj);
  for (const ws of uiClients) if (ws.readyState === 1) ws.send(s);
}

/** Aux seules interfaces ouvertes sur cet ordinateur. */
function broadcastLocal(obj) {
  const s = JSON.stringify(obj);
  for (const ws of uiClients) if (ws.local && ws.readyState === 1) ws.send(s);
}

/**
 * Interface ouverte sur cet ordinateur (adresse locale ET origine de l'interface d'ARKLIGHT, ou
 * programme qui n'est pas un navigateur) : une page web d'un autre site, même ouverte sur ce PC,
 * n'en est pas une (voir local-client.js).
 */
const isLocal = (req) => isLocalClient(req, server.address()?.port ?? PORT);

/** Trame binaire : [0x01][univers u16 BE][512 octets] */
function dmxFrame(u, data) {
  const buf = Buffer.alloc(3 + 512);
  buf[0] = 0x01;
  buf.writeUInt16BE(u, 1);
  buf.set(data, 3);
  return buf;
}

function statusPayload() {
  return {
    t: 'status',
    artnet: { listening: !!artnet.socket && !artnet.error, packets: artnet.packets, polls: artnet.polls, error: artnet.error },
    sacn: { listening: sacn.ready, packets: sacn.packets, error: sacn.error, joined: [...sacn.joined] },
    sources: universes.sources(),
    interfaces: ipv4Interfaces().map((i) => ({ name: i.name, address: i.address })),
  };
}

/** État d'annulation et d'enregistrement : menus Édition et Fichier, titre de la fenêtre. */
function historyPayload() {
  return { t: 'history', ...history.status(), dirty: store.dirty, savedName: store.savedName, savedPath: store.savedPath };
}

// ---------------------------------------------------------------- projets
/** Liste des projets pour l'écran « Projets » (vignettes comprises). */
function projectsPayload() {
  return {
    t: 'projects',
    root: library.root,
    rootDefault: library.isDefault,
    rootAvailable: library.available,
    current: store.savedPath,
    items: library.list().map(({ modifiedMs, ...it }) => (void modifiedMs, it)),
  };
}
const samePath = (a, b) => !!a && !!b && resolve(a).toLowerCase() === resolve(b).toLowerCase();

/** Scène complète (nouveau projet, projet ouvert, changement de plan) avec ses séquences et plans. */
function broadcastScene() {
  broadcast({ t: 'scene', scene: store.scene, plans: store.planInfo() });
}
function broadcastPlans() {
  broadcast({ t: 'plans', plans: store.planInfo() });
}
/** Le plan courant a changé : sa scène, son historique, ses univers DMX. */
function planChanged() {
  useHistory();
  broadcastScene();
  syncSacnUniverses();
}

function openProject(path) {
  const scene = store.loadFrom(path);
  library.addRecent(path);
  resetHistories();
  broadcastScene();
  syncSacnUniverses();
  log('info', `Projet ouvert : ${scene.name}`);
  broadcastHistory();
  return scene;
}

function saveProject(path, thumbnail) {
  const name = store.saveTo(path, { thumbnail: cleanThumbnail(thumbnail) });
  library.addRecent(path);
  broadcast({ t: 'scene.meta', name });
  log('info', `Projet enregistré : ${path}`);
  broadcastHistory();
  return name;
}
let lastHistory = '';
function broadcastHistory() {
  const p = historyPayload();
  const s = JSON.stringify(p);
  if (s === lastHistory) return;
  lastHistory = s;
  broadcast(p);
}

/**
 * Une demande à l'assistant : il agit dans la scène (une seule étape « Annuler » par demande, par
 * plan), les interfaces voient les changements au fur et à mesure.
 */
async function askAssistant(ws, msg) {
  if (config.ai.provider === 'ollama' && !config.ai.model) await aiState(); // choisit le modèle local
  let step = history.begin(store.scene, store.revision, { t: 'ai.action' });
  const endStep = () => {
    if (step && history.end(step, store.scene, store.revision) === 'noop') store.revision = step.revision;
    step = null;
  };
  const flush = (runner) => {
    const c = runner.changed;
    if (c.has('fixtures') || c.has('objects') || c.has('groups')) broadcast({ t: 'scene.restore', scene: store.scene, origin: 0, undo: false, label: '' });
    if (c.has('env')) broadcast({ t: 'env', environment: store.scene.environment, origin: 0 });
    if (c.has('fixtures')) syncSacnUniverses();
    c.clear();
    broadcastHistory();
  };
  const hooks = {
    afterTool: flush,
    // DMX : univers reçus (dont le dernier état conservé) et ceux qui reçoivent encore un signal
    dmxState: () => ({ data: new Set(universes.universes.keys()), live: new Set(universes.sources().flatMap((s) => s.universes)) }),
    forgetDmx: (list) => {
      const done = list.filter((u) => universes.universes.has(u) && universes.forget(u));
      if (done.length) broadcast({ t: 'dmx.forget', universes: done });
      return done;
    },
    planChanging: () => endStep(),
    planChanged: () => {
      planChanged();
      step = history.begin(store.scene, store.revision, { t: 'ai.action' });
    },
  };
  send(ws, { t: 'ai.event', kind: 'start' });
  try {
    const size = Array.isArray(msg.imageSize) ? msg.imageSize.map(Number) : null;
    const imageAspect = size && size[0] > 0 && size[1] > 0 ? size[0] / size[1] : null;
    const runner = await assistant.ask({ text: msg.text, images: Array.isArray(msg.images) ? msg.images.slice(0, 4) : [], imageAspect, lang: config.lang, hooks, onEvent: (e) => send(ws, { t: 'ai.event', ...e }) });
    flush(runner);
    send(ws, { t: 'ai.done', select: runner.lastAdded });
  } catch (e) {
    send(ws, { t: 'ai.error', message: e.message, code: e.code || null });
  } finally {
    endStep();
    broadcastHistory();
  }
}

/** Téléchargement (installeur d'Ollama, modèle) : progression envoyée à toutes les interfaces. */
function runAiTask(kind, fn) {
  if (aiTask) throw new Error('Un téléchargement est déjà en cours');
  const ctrl = new AbortController();
  aiTask = { kind, ctrl };
  const progress = (p) => broadcast({ t: 'ai.progress', kind, ...p });
  progress({ pct: 0, text: '…' });
  fn(progress, ctrl.signal)
    .then(() => broadcast({ t: 'ai.progress', kind, pct: 100, done: true }))
    .catch((e) => broadcast({ t: 'ai.progress', kind, error: ctrl.signal.aborted ? 'Annulé' : e.message, done: true }))
    .finally(async () => {
      aiTask = null;
      broadcast(await aiState());
    });
}

function broadcastGroups(ws, extra = {}) {
  broadcast({ t: 'groups', groups: store.scene.groups, origin: ws.id, ...extra });
}

function handleUi(ws, msg) {
  switch (msg.t) {
    case 'fixture.add': {
      const f = store.addFixture(msg);
      broadcast({ t: 'fixture.added', fixture: f, origin: ws.id });
      syncSacnUniverses();
      break;
    }
    case 'fixture.update': {
      const f = store.updateFixture(msg.id, msg.changes || {});
      if (f) broadcast({ t: 'fixture.updated', fixture: f, origin: ws.id });
      if (msg.changes?.settings?.universe !== undefined) syncSacnUniverses();
      break;
    }
    case 'fixture.remove': {
      if (store.removeFixture(msg.id)) {
        broadcast({ t: 'fixture.removed', id: msg.id, origin: ws.id });
        broadcastGroups(ws);
      }
      syncSacnUniverses();
      break;
    }
    // ---- opérations groupées (sélection multiple, ré-adressage automatique)
    case 'fixtures.add': {
      const added = (Array.isArray(msg.items) ? msg.items : []).slice(0, 256).map((item) => store.addFixture(item));
      if (added.length) broadcast({ t: 'fixtures.added', fixtures: added, origin: ws.id, select: !!msg.select });
      syncSacnUniverses();
      break;
    }
    case 'fixtures.update': {
      const updated = [];
      let universeChanged = false;
      for (const item of Array.isArray(msg.items) ? msg.items : []) {
        const f = store.updateFixture(item.id, item.changes || {});
        if (!f) continue;
        updated.push(f);
        if (item.changes?.settings?.universe !== undefined) universeChanged = true;
      }
      if (updated.length) broadcast({ t: 'fixtures.updated', fixtures: updated, origin: ws.id });
      if (universeChanged) syncSacnUniverses();
      break;
    }
    case 'fixtures.remove': {
      const ids = (Array.isArray(msg.ids) ? msg.ids : []).filter((id) => store.removeFixture(id));
      if (ids.length) {
        broadcast({ t: 'fixtures.removed', ids, origin: ws.id });
        broadcastGroups(ws);
      }
      syncSacnUniverses();
      break;
    }
    // ---- grip et accessoires
    case 'object.add': {
      const o = store.addObject(msg);
      broadcast({ t: 'object.added', object: o, origin: ws.id, select: !!msg.select });
      break;
    }
    case 'object.update': {
      const o = store.updateObject(msg.id, msg.changes || {});
      if (o) broadcast({ t: 'object.updated', object: o, origin: ws.id });
      break;
    }
    case 'object.remove':
      for (const id of Array.isArray(msg.ids) ? msg.ids : [msg.id]) if (store.removeObject(id)) broadcast({ t: 'object.removed', id, origin: ws.id });
      break;
    // ---- plan de feu 2D
    case 'plot.add': {
      const it = store.addPlotItem(msg.item);
      broadcast({ t: 'plot.item', item: it, origin: ws.id, added: true });
      break;
    }
    case 'plot.update': {
      const it = store.updatePlotItem(msg.id, msg.changes || {});
      if (it) broadcast({ t: 'plot.item', item: it, origin: ws.id });
      break;
    }
    case 'plot.remove':
      for (const id of Array.isArray(msg.ids) ? msg.ids : [msg.id]) if (store.removePlotItem(id)) broadcast({ t: 'plot.removed', id, origin: ws.id });
      break;
    case 'plot.meta': {
      const p = store.updatePlotMeta(msg);
      broadcast({ t: 'plot.meta', underlay: p.underlay, title: p.title, background: p.background, origin: ws.id });
      break;
    }
    // ---- groupes de projecteurs
    case 'group.create': {
      const g = store.createGroup(msg);
      broadcastGroups(ws, { created: g.id });
      break;
    }
    case 'group.update':
      if (store.updateGroup(msg.id, msg)) broadcastGroups(ws);
      break;
    case 'group.delete':
      if (store.removeGroup(msg.id)) broadcastGroups(ws);
      break;
    case 'env.update':
      broadcast({ t: 'env', environment: store.updateEnvironment(msg.changes), origin: ws.id });
      break;
    case 'scene.rename': {
      // projet déjà enregistré : son fichier change de nom avec lui
      const n = safeName(msg.name);
      if (!n) break;
      if (store.savedPath && n !== store.savedName) {
        library.allow(store.savedPath);
        store.savedPath = library.rename(store.savedPath, n);
        log('info', `Projet renommé : ${n}`);
      }
      store.rename(n);
      broadcast({ t: 'scene.meta', name: store.scene.name });
      break;
    }
    case 'scene.new':
    case 'project.new':
      store.newScene();
      resetHistories();
      broadcastScene();
      syncSacnUniverses();
      break;
    case 'project.demo': {
      const demo = getDemo(msg.id);
      buildDemo(store, store.lang, demo.id);
      store.rename(translate(demo.name, store.lang));
      resetHistories();
      broadcastScene();
      syncSacnUniverses();
      log('info', 'Projet d’exemple ouvert (à enregistrer pour le garder)');
      break;
    }
    case 'projects.list':
      send(ws, projectsPayload());
      break;
    case 'project.open':
      if (!library.isKnown(msg.path)) throw new Error('Projet inconnu : rouvre la liste des projets');
      openProject(msg.path);
      break;
    case 'project.save': {
      // nom donné : dans le dossier des projets (ou celui du projet ouvert) ; sinon dans son fichier
      let path = store.savedPath;
      if (typeof msg.name === 'string') {
        const folder = msg.where === 'current' && store.savedPath ? dirname(store.savedPath) : library.root;
        path = library.target(folder, msg.name);
        if (!msg.overwrite && existsSync(path) && !samePath(path, store.savedPath)) {
          send(ws, { t: 'project.exists', name: projectName(path), path });
          break;
        }
      }
      if (!path) throw new Error('Donne un nom au projet');
      const name = saveProject(path, msg.thumbnail);
      send(ws, { t: 'project.saved', name, path });
      break;
    }
    case 'project.rename': {
      const to = library.rename(msg.path, msg.name);
      if (samePath(msg.path, store.savedPath)) {
        store.savedPath = to;
        store.rename(projectName(to));
        broadcast({ t: 'scene.meta', name: store.scene.name });
      }
      send(ws, projectsPayload());
      break;
    }
    case 'project.duplicate':
      library.duplicate(msg.path, msg.name);
      send(ws, projectsPayload());
      break;
    case 'project.forget':
      library.forget(msg.path);
      send(ws, projectsPayload());
      break;
    case 'project.trash':
      if (samePath(msg.path, store.savedPath)) throw new Error('Ce projet est ouvert : ouvre-en un autre avant de le mettre à la corbeille');
      library
        .trashFile(msg.path)
        .then(() => {
          log('info', `Projet mis à la corbeille : ${projectName(msg.path)}`);
          send(ws, projectsPayload());
        })
        .catch((e) => send(ws, { t: 'error', message: `Corbeille : ${e.message}` }));
      break;
    case 'scene.save': {
      const name = store.save(msg.name);
      library.addRecent(store.savedPath);
      broadcast({ t: 'scene.meta', name });
      send(ws, { t: 'scene.list', items: store.list() });
      log('info', `Projet enregistré : ${name}`);
      break;
    }
    case 'scene.load':
      store.load(msg.name);
      resetHistories();
      broadcastScene();
      syncSacnUniverses();
      library.addRecent(store.savedPath);
      log('info', `Projet ouvert : ${store.scene.name}`);
      break;
    case 'scene.exists':
      send(ws, { t: 'scene.exists', name: msg.name, exists: store.exists(msg.name) });
      break;
    // ---- séquences et plans (un plan de feu par plan)
    case 'plan.switch':
      if (store.switchPlan(msg.id, { thumbnail: msg.thumbnail })) planChanged();
      break;
    case 'plan.thumbnail':
      if (store.setPlanThumbnail(msg.id, msg.thumbnail)) broadcastPlans();
      break;
    case 'plan.new': {
      const p = store.newPlan({ seq: msg.seq, mode: msg.mode, name: msg.name, thumbnail: msg.thumbnail });
      planChanged();
      log('info', `Nouveau plan : ${p.name}`);
      break;
    }
    case 'sequence.new': {
      const sq = store.newSequence({ mode: msg.mode, name: msg.name, thumbnail: msg.thumbnail });
      planChanged();
      log('info', `Nouvelle séquence : ${sq.name}`);
      break;
    }
    case 'plan.duplicate':
      if (store.duplicatePlan(msg.id)) broadcastPlans();
      break;
    case 'plan.rename':
      if (store.renamePlan(msg.id, msg.name)) broadcastPlans();
      break;
    case 'sequence.rename':
      if (store.renameSequence(msg.id, msg.name)) broadcastPlans();
      break;
    case 'plan.move':
      if (store.movePlan(msg.id, { seq: msg.seq, index: msg.index })) broadcastPlans();
      break;
    case 'sequence.move':
      if (store.moveSequence(msg.id, msg.index)) broadcastPlans();
      break;
    case 'plan.delete':
    case 'sequence.delete': {
      const before = store.current.plan;
      const gone = msg.t === 'plan.delete' ? [msg.id] : (store.sequences.find((sq) => sq.id === msg.id)?.plans || []).map((p) => p.id);
      const ok = msg.t === 'plan.delete' ? store.removePlan(msg.id) : store.removeSequence(msg.id);
      if (!ok) break;
      for (const id of gone) histories.delete(id);
      if (store.current.plan !== before) planChanged();
      else broadcastPlans();
      break;
    }
    // ---- assistant IA
    case 'ai.state':
      aiState().then((st) => send(ws, st));
      break;
    case 'ai.config': {
      const c = msg.changes || {};
      if (typeof c.provider === 'string' && PROVIDERS[c.provider]) {
        if (c.provider !== config.ai.provider) config.ai.model = PROVIDERS[c.provider].defaultModel || '';
        config.ai.provider = c.provider;
      }
      if (typeof c.model === 'string') config.ai.model = c.model.trim().slice(0, 120);
      if (c.voice && typeof c.voice === 'object') {
        const v = c.voice;
        if (typeof v.enabled === 'boolean') config.ai.voice.enabled = v.enabled;
        if (typeof v.autoSend === 'boolean') config.ai.voice.autoSend = v.autoSend;
        if (['base', 'small'].includes(v.model)) config.ai.voice.model = v.model;
        if (['auto', 'fr', 'en', 'es'].includes(v.lang)) config.ai.voice.lang = v.lang;
      }
      if (c.urls && typeof c.urls === 'object') {
        config.ai.urls = { ...config.ai.urls };
        for (const [k, v] of Object.entries(c.urls)) if (PROVIDERS[k] && typeof v === 'string') config.ai.urls[k] = v.trim().slice(0, 300);
      }
      saveConfig();
      aiState().then((st) => broadcast(st));
      break;
    }
    case 'ai.key':
      if (!PROVIDERS[msg.provider]?.key) break;
      aiKeys.set(msg.provider, msg.key);
      log('info', msg.key ? `Assistant : clé ${PROVIDERS[msg.provider].label} enregistrée sur cet ordinateur` : `Assistant : clé ${PROVIDERS[msg.provider].label} effacée`);
      aiState().then((st) => broadcast(st));
      break;
    case 'ai.models': {
      const provider = PROVIDERS[msg.provider] ? msg.provider : config.ai.provider;
      listModels({ provider, key: aiKeys.get(provider), cfg: config.ai })
        .then((models) => send(ws, { t: 'ai.models', provider, models }))
        .catch((e) => send(ws, { t: 'ai.models', provider, models: [], error: e.message }));
      break;
    }
    case 'ai.ask':
      if (typeof msg.text !== 'string' || !msg.text.trim()) break;
      askAssistant(ws, msg);
      break;
    case 'ai.stop':
      assistant.stop();
      break;
    case 'ai.reset':
      if (!assistant.busy) assistant.reset();
      send(ws, { t: 'ai.reset' });
      break;
    case 'ai.ollama.start':
      startOllama();
      log('info', 'Assistant : démarrage d’Ollama');
      setTimeout(() => aiState().then((st) => broadcast(st)), 4000);
      break;
    case 'ai.ollama.pull': {
      const model = String(msg.model || '').trim();
      if (!/^[\w.:/-]{2,80}$/.test(model)) throw new Error('Nom de modèle invalide');
      runAiTask('pull', async (progress, signal) => {
        log('info', `Assistant : téléchargement du modèle ${model}`);
        await pullModel(providerUrl('ollama', config.ai), model, progress, signal);
        if (config.ai.provider === 'ollama') {
          config.ai.model = model;
          saveConfig();
        }
        log('info', `Assistant : modèle ${model} prêt`);
      });
      break;
    }
    case 'ai.ollama.install':
      if (!process.env.ARKLIGHT_DESKTOP) throw new Error('Installation possible seulement dans l’application de bureau');
      runAiTask('install', async (progress, signal) => {
        log('info', 'Assistant : téléchargement de l’installeur officiel d’Ollama');
        await installOllama(join(DATA, 'downloads'), progress, signal);
      });
      break;
    case 'ai.voice.install':
      if (process.platform !== 'win32') throw new Error('Installation automatique prévue pour Windows');
      runAiTask('voice', async (progress, signal) => {
        log('info', 'Assistant : installation de la reconnaissance vocale (whisper.cpp)');
        await installVoice(DATA, config.ai.voice.model, progress, signal);
        config.ai.voice.enabled = true;
        saveConfig();
        log('info', 'Assistant : reconnaissance vocale prête');
      });
      break;
    case 'ai.task.cancel':
      aiTask?.ctrl.abort();
      break;
    // ---- AIODOS : se connecter (code à 6 chiffres comparé dans AIODOS), annuler, se déconnecter, lancer
    case 'ai.aiodos.connect':
      log('info', 'Assistant : demande d’accès à AIODOS');
      aiodos
        .connect({
          onCode: (code) => {
            broadcastLocal({ t: 'ai.aiodos.code', code });
            aiState().then((st) => broadcast(st));
          },
        })
        .then(async () => {
          log('info', 'Assistant : ARKLIGHT est connecté à AIODOS');
          broadcastLocal({ t: 'ai.aiodos.code', code: null, done: true });
        })
        .catch((e) => {
          broadcastLocal({ t: 'ai.aiodos.code', code: null, error: e.code === 'cancelled' ? '' : e.message });
          if (e.code !== 'cancelled') log('warn', `AIODOS : ${e.message}`);
        })
        .finally(async () => broadcast(await aiState()));
      break;
    case 'ai.aiodos.cancel':
      aiodos.cancel();
      break;
    case 'ai.aiodos.disconnect':
      aiodos
        .disconnect()
        .then(() => log('info', 'Assistant : ARKLIGHT est déconnecté d’AIODOS'))
        .catch((e) => log('warn', `AIODOS : ${e.message}`))
        .finally(async () => broadcast(await aiState()));
      break;
    case 'ai.aiodos.launch':
      log('info', 'Assistant : lancement d’AIODOS');
      aiodos
        .launch()
        .catch((e) => send(ws, { t: 'error', message: e.message }))
        .finally(async () => broadcast(await aiState()));
      break;
    // ---- annuler / rétablir
    case 'history.undo':
    case 'history.redo': {
      const undo = msg.t === 'history.undo';
      const step = undo ? history.undo(store.scene, store.revision) : history.redo(store.scene, store.revision);
      if (!step) break;
      const scene = store.restore(step.json, step.revision);
      broadcast({ t: 'scene.restore', scene, origin: ws.id, undo, label: step.label });
      syncSacnUniverses();
      break;
    }
    case 'scene.delete':
      store.remove(msg.name);
      send(ws, { t: 'scene.list', items: store.list() });
      break;
    case 'update.check':
      runUpdateCheck(ws);
      break;
    case 'scene.list':
      send(ws, { t: 'scene.list', items: store.list() });
      break;
    case 'config.update': {
      const allowed = ['artnetUniverseOffset', 'lang'];
      for (const k of allowed) if (msg.changes?.[k] !== undefined) config[k] = msg.changes[k];
      config.lang = config.lang === 'en' ? 'en' : 'fr';
      store.lang = config.lang;
      writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2));
      broadcast({ t: 'config', config });
      break;
    }
    case 'dmx.inject': {
      // Console de test intégrée : l'interface envoie directement des valeurs DMX.
      const data = Uint8Array.from(msg.data || []);
      universes.input({ universe: msg.universe, data, key: 'ui|test', priority: msg.priority ?? 100, protocol: 'test', address: 'interface' });
      break;
    }
    case 'dmx.release':
      universes.terminate(msg.universe, 'ui|test');
      break;
    case 'dmx.forget': {
      // « Reprendre la main » : on oublie le dernier état DMX des univers qui ne reçoivent plus rien
      const list = (Array.isArray(msg.universes) ? msg.universes : [msg.universe]).map(Number).filter((u) => Number.isInteger(u) && u > 0);
      const forgotten = list.filter((u) => universes.forget(u));
      if (forgotten.length) broadcast({ t: 'dmx.forget', universes: forgotten });
      const busy = list.filter((u) => !forgotten.includes(u));
      if (busy.length) send(ws, { t: 'error', message: `Univers ${busy.join(', ')} : une console ou une appli envoie encore du DMX` });
      break;
    }
    case 'ble.start':
      ble.startProcess();
      break;
    case 'ble.stop':
      ble.stopProcess();
      break;
    case 'ble.emulator':
      ble.setEmulator(msg.id);
      break;
    default:
      send(ws, { t: 'error', message: `Message inconnu : ${msg.t}` });
  }
}

// ---------------------------------------------------------------- HTTP
const serveStatic = createStaticHandler([
  { prefix: '/assets/', dir: ASSETS },
  { prefix: '/vendor/three/', dir: join(ROOT, 'node_modules', 'three') },
  { prefix: '/shared/', dir: join(ROOT, 'shared') },
  { prefix: '/', dir: join(ROOT, 'web') },
]);

// Envoi d'une image de fond pour le plan de feu (photo, plan d'architecte) : POST /upload, corps = l'image
const UPLOAD_TYPES = { 'image/png': '.png', 'image/jpeg': '.jpg', 'image/webp': '.webp' };
const UPLOAD_MAX = 25 * 1024 * 1024;
function handleUpload(req, res) {
  const ext = UPLOAD_TYPES[(req.headers['content-type'] || '').split(';')[0].trim()];
  const reply = (code, obj) => res.writeHead(code, { 'Content-Type': 'application/json; charset=utf-8' }).end(JSON.stringify(obj));
  if (!ext) return reply(415, { error: 'Image PNG, JPEG ou WebP attendue' });
  const chunks = [];
  let size = 0;
  req.on('data', (c) => {
    size += c.length;
    if (size > UPLOAD_MAX) {
      reply(413, { error: 'Image trop lourde (25 Mo maximum)' });
      req.destroy();
    } else chunks.push(c);
  });
  req.on('end', () => {
    if (size > UPLOAD_MAX) return;
    const name = `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}${ext}`;
    writeFileSync(join(ASSETS, name), Buffer.concat(chunks));
    log('info', `Plan de feu : image de fond enregistrée (${Math.round(size / 1024)} Ko)`);
    reply(200, { url: `/assets/${name}` });
  });
}

// Commande vocale : l'interface envoie la parole (WAV 16 kHz mono), le serveur la transcrit avec Whisper
const VOICE_MAX = 12 * 1024 * 1024;
function handleVoice(req, res) {
  const reply = (code, obj) => res.writeHead(code, { 'Content-Type': 'application/json; charset=utf-8' }).end(JSON.stringify(obj));
  if (!isLocal(req)) return reply(403, { error: 'L’assistant IA ne s’utilise que sur l’ordinateur où ARKLIGHT tourne' });
  const chunks = [];
  let size = 0;
  req.on('data', (c) => {
    size += c.length;
    if (size > VOICE_MAX) {
      reply(413, { error: 'Enregistrement trop long' });
      req.destroy();
    } else chunks.push(c);
  });
  req.on('end', async () => {
    if (size > VOICE_MAX) return;
    const dir = join(DATA, 'whisper', 'tmp');
    mkdirSync(dir, { recursive: true });
    const wav = join(dir, `voix-${Date.now().toString(36)}.wav`);
    writeFileSync(wav, Buffer.concat(chunks));
    // amorce : vocabulaire du métier + noms du projet (projecteurs, groupes, décor, plans)
    const names = [...new Set([...store.scene.fixtures.map((f) => f.name), ...store.scene.groups.map((g) => g.name), ...store.scene.objects.map((o) => o.name)])].slice(0, 60).join(', ');
    try {
      const text = await transcribe({ dir: DATA, model: config.ai.voice.model, wav, lang: config.ai.voice.lang, prompt: `${VOCABULARY} ${names}` });
      reply(200, { text });
    } catch (e) {
      reply(500, { error: e.message });
    } finally {
      rmSync(wav, { force: true });
    }
  });
}

const server = createServer((req, res) => {
  if (req.method === 'POST' && new URL(req.url, 'http://localhost').pathname === '/upload') return handleUpload(req, res);
  if (req.method === 'POST' && new URL(req.url, 'http://localhost').pathname === '/ai/voice') return handleVoice(req, res);
  if (!serveStatic(req, res)) res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' }).end('Introuvable');
});

const wss = new WebSocketServer({ server });
wss.on('error', () => {}); // ws relaie les erreurs du serveur HTTP (port occupé) : elles sont traitées plus bas
wss.on('connection', (ws, req) => {
  const path = new URL(req.url, 'http://localhost').pathname;
  if (path === '/bridge') return ble.attach(ws);
  if (path !== '/ws') return ws.close(1008, 'Chemin inconnu');
  ws.id = nextClientId++;
  // interface ouverte sur cet ordinateur (et non depuis une tablette du réseau, ni une page web
  // d'un autre site ouverte sur ce PC) : seule à pouvoir se servir de l'assistant IA (clés API,
  // AIODOS : ARKLIGHT ne doit pas en devenir le relais)
  ws.local = isLocal(req);
  uiClients.add(ws);
  send(ws, { t: 'hello', clientId: ws.id, scene: store.scene, plans: store.planInfo(), config, logs, version: APP_VERSION, repo: UPDATE_REPO, desktop: !!process.env.ARKLIGHT_DESKTOP, local: ws.local, history: historyPayload() });
  if (lastUpdate) send(ws, { t: 'update', ...lastUpdate });
  send(ws, statusPayload());
  send(ws, { t: 'ble.status', ...ble.status(), traffic: ble.traffic });
  for (const [u, data] of universes.all()) ws.send(dmxFrame(u, data));
  ws.on('message', (raw, isBinary) => {
    if (isBinary) return;
    let msg;
    try {
      msg = JSON.parse(raw.toString());
    } catch {
      return send(ws, { t: 'error', message: 'JSON invalide' });
    }
    try {
      if (typeof msg.t === 'string' && msg.t.startsWith('ai.') && !ws.local) throw new Error('L’assistant IA ne s’utilise que sur l’ordinateur où ARKLIGHT tourne');
      if (UNDOABLE.has(msg.t)) {
        // une étape d'historique, sauf si rien n'a changé (la scène reste alors « enregistrée »)
        const step = history.begin(store.scene, store.revision, msg);
        handleUi(ws, msg);
        if (history.end(step, store.scene, store.revision) === 'noop') store.revision = step.revision;
      } else handleUi(ws, msg);
    } catch (err) {
      send(ws, { t: 'error', message: err.message });
    }
    broadcastHistory();
  });
  ws.on('close', () => uiClients.delete(ws));
});

// ---------------------------------------------------------------- boucles
setInterval(() => {
  const dirty = universes.takeDirty();
  if (!dirty.length || !uiClients.size) return;
  for (const [u, data] of dirty) {
    const frame = dmxFrame(u, data);
    for (const ws of uiClients) if (ws.readyState === 1) ws.send(frame);
  }
}, 25);

setInterval(() => broadcast(statusPayload()), 1000);
setInterval(() => universes.sweep(), 5000);

// ---------------------------------------------------------------- mises à jour (GitHub Releases)
let lastUpdate = null;
async function runUpdateCheck(requester) {
  const r = await checkForUpdate();
  if (r.status === 'available' && lastUpdate?.latest !== r.latest) log('info', `Mise à jour disponible : ARKLIGHT ${r.latest} (version installée : ${r.current})`);
  lastUpdate = r;
  if (r.status === 'available') broadcast({ t: 'update', ...r });
  else if (requester) send(requester, { t: 'update', ...r });
}

// ---------------------------------------------------------------- démarrage
/** Port HTTP effectif (l'application de bureau prend un port libre si 8686 est occupé). */
export const ready = new Promise((resolve, reject) => {
  let fallback = false;
  server.on('error', (err) => {
    if (err.code === 'EADDRINUSE') {
      if (process.env.ARKLIGHT_DESKTOP && !fallback) {
        fallback = true;
        console.warn(`Le port ${PORT} est déjà utilisé : l'interface prend un autre port.`);
        server.listen(0);
        return;
      }
      console.error(`Le port ${PORT} est déjà utilisé : ARKLIGHT tourne peut-être déjà (http://localhost:${PORT}).`);
      if (process.env.ARKLIGHT_DESKTOP) return reject(err);
      process.exit(1);
    }
    reject(err);
  });
  server.on('listening', () => {
    const port = server.address().port;
    console.log(`ARKLIGHT ${APP_VERSION} prêt : http://localhost:${port}`);
    for (const i of ipv4Interfaces()) console.log(`  réseau ${i.name} : ${i.address} (adresse à donner à Luminair / Blackout)`);
    artnet.start();
    sacn.start();
    syncSacnUniverses();
    if (UPDATE_REPO) {
      setTimeout(() => runUpdateCheck(), 4000);
      setInterval(() => runUpdateCheck(), 6 * 3600 * 1000).unref();
    }
    resolve(port);
  });
});
server.listen(PORT);

/**
 * Projets, pour l'application de bureau : ses fenêtres « Enregistrer sous », « Ouvrir » et « Choisir
 * un dossier » donnent des chemins choisis par l'utilisateur (jamais reçus par le réseau).
 */
export const projects = {
  root: () => library.root,
  setRoot(dir) {
    const root = library.setRoot(dir);
    store.projectsDir = root;
    log('info', `Dossier des projets : ${root}`);
    broadcast({ t: 'projects.changed' });
    return root;
  },
  open(path) {
    library.allow(path);
    return { name: openProject(path).name };
  },
  saveTo(path, thumbnail) {
    library.allow(path);
    return { name: saveProject(path, thumbnail), path };
  },
  /** Chemin connu de l'écran « Projets » (pour « Afficher dans le dossier »). */
  isKnown: (path) => library.isKnown(path) || samePath(path, store.savedPath),
  currentPath: () => store.savedPath,
  suggestedName: () => store.scene.name,
  setTrash(fn) {
    library.trash = fn;
  },
};

/** Enregistre la scène en cours et arrête le pont Bluetooth (fermeture du logiciel). */
export function shutdown() {
  store.flush();
  ble.stopProcess();
}
if (!process.env.ARKLIGHT_DESKTOP) {
  for (const sig of ['SIGINT', 'SIGTERM'])
    process.on(sig, () => {
      shutdown();
      process.exit(0);
    });
}
