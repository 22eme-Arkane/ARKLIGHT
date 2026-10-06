// ARKLIGHT — serveur central du simulateur.
//  - sert l'interface web (http://localhost:8686)
//  - reçoit le DMX (Art-Net / sACN) des applis et consoles
//  - tient l'état de la scène et le diffuse aux interfaces par WebSocket
import { createServer } from 'node:http';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
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

const store = new SceneStore(join(DATA, 'scenes'));
const ASSETS = join(DATA, 'scenes', 'assets'); // images de fond des plans de feu
mkdirSync(ASSETS, { recursive: true });
const universes = new UniverseStore();
const history = new History({ limit: 100 }); // annuler / rétablir

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

/** État d'annulation et d'enregistrement : menus Édition et Fichier, point « non enregistré ». */
function historyPayload() {
  return { t: 'history', ...history.status(), dirty: store.dirty, savedName: store.savedName };
}
let lastHistory = '';
function broadcastHistory() {
  const p = historyPayload();
  const s = JSON.stringify(p);
  if (s === lastHistory) return;
  lastHistory = s;
  broadcast(p);
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
      broadcast({ t: 'plot.meta', underlay: p.underlay, title: p.title, origin: ws.id });
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
    case 'scene.rename':
      store.rename(msg.name);
      broadcast({ t: 'scene.meta', name: store.scene.name });
      break;
    case 'scene.new':
      history.clear();
      broadcast({ t: 'scene', scene: store.newScene() });
      syncSacnUniverses();
      break;
    case 'scene.save': {
      const name = store.save(msg.name);
      broadcast({ t: 'scene.meta', name });
      send(ws, { t: 'scene.list', items: store.list() });
      log('info', `Scène enregistrée : ${name}`);
      break;
    }
    case 'scene.load':
      broadcast({ t: 'scene', scene: store.load(msg.name) });
      history.clear();
      syncSacnUniverses();
      log('info', `Scène ouverte : ${store.scene.name}`);
      break;
    case 'scene.exists':
      send(ws, { t: 'scene.exists', name: msg.name, exists: store.exists(msg.name) });
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
      const allowed = ['artnetUniverseOffset'];
      for (const k of allowed) if (msg.changes?.[k] !== undefined) config[k] = msg.changes[k];
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

const server = createServer((req, res) => {
  if (req.method === 'POST' && new URL(req.url, 'http://localhost').pathname === '/upload') return handleUpload(req, res);
  if (!serveStatic(req, res)) res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' }).end('Introuvable');
});

const wss = new WebSocketServer({ server });
wss.on('error', () => {}); // ws relaie les erreurs du serveur HTTP (port occupé) : elles sont traitées plus bas
wss.on('connection', (ws, req) => {
  const path = new URL(req.url, 'http://localhost').pathname;
  if (path === '/bridge') return ble.attach(ws);
  if (path !== '/ws') return ws.close(1008, 'Chemin inconnu');
  ws.id = nextClientId++;
  uiClients.add(ws);
  send(ws, { t: 'hello', clientId: ws.id, scene: store.scene, config, logs, version: APP_VERSION, repo: UPDATE_REPO, desktop: !!process.env.ARKLIGHT_DESKTOP, history: historyPayload() });
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
