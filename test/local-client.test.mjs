// Droits « de cet ordinateur » (assistant IA : clés d'API, AIODOS) : l'interface d'ARKLIGHT
// ouverte sur ce PC les a ; une page web d'un autre site, ouverte dans un navigateur de ce PC, ne
// les a pas, même si elle se connecte depuis 127.0.0.1.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer, request } from 'node:http';
import { WebSocket, WebSocketServer } from 'ws';
import { isLocalClient, isLoopback, uiOrigins } from '../server/local-client.js';

const req = (remoteAddress, origin) => ({ socket: { remoteAddress }, headers: origin === undefined ? {} : { origin } });

test('droits locaux : adresse de ce PC et origine de l’interface d’ARKLIGHT (ou pas de navigateur)', () => {
  assert.ok(isLoopback('127.0.0.1') && isLoopback('::1') && isLoopback('::ffff:127.0.0.1'));
  assert.ok(!isLoopback('192.168.1.20') && !isLoopback('::ffff:192.168.1.20') && !isLoopback(undefined));
  assert.deepEqual(uiOrigins(8686), ['http://localhost:8686', 'http://127.0.0.1:8686', 'http://[::1]:8686']);
  // l'interface (navigateur ou application de bureau, qui charge http://localhost:<port>)
  for (const o of uiOrigins(8686)) assert.equal(isLocalClient(req('127.0.0.1', o), 8686), true, o);
  assert.equal(isLocalClient(req('::1', 'http://[::1]:8686'), 8686), true);
  assert.equal(isLocalClient(req('::ffff:127.0.0.1', 'HTTP://LOCALHOST:8686'), 8686), true);
  // programme qui n'est pas un navigateur (pas d'en-tête Origin)
  assert.equal(isLocalClient(req('127.0.0.1'), 8686), true);
  // une page web d'un autre site, ouverte sur ce PC
  for (const o of ['https://evil.example', 'http://evil.example:8686', 'null', '', 'http://localhost:3000', 'https://localhost:8686', 'file://'])
    assert.equal(isLocalClient(req('127.0.0.1', o), 8686), false, `origine « ${o} »`);
  // une tablette du réseau, même avec la bonne origine
  assert.equal(isLocalClient(req('192.168.1.20', 'http://localhost:8686'), 8686), false);
  assert.equal(isLocalClient(req('192.168.1.20'), 8686), false);
});

/** Serveur d'essai câblé comme celui d'ARKLIGHT : `local` dans le message d'accueil, POST refusé sinon. */
async function startServer() {
  const server = createServer((rq, res) => {
    const ok = isLocalClient(rq, server.address().port);
    res.writeHead(ok ? 200 : 403).end();
  });
  const wss = new WebSocketServer({ server });
  wss.on('connection', (ws, rq) => ws.send(JSON.stringify({ t: 'hello', local: isLocalClient(rq, server.address().port) })));
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  return { server, wss, port: server.address().port };
}

function hello(port, origin) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}/ws`, origin ? { origin } : {});
    ws.on('message', (data) => {
      ws.close();
      resolve(JSON.parse(String(data)));
    });
    ws.on('error', reject);
  });
}

function post(port, origin) {
  return new Promise((resolve, reject) => {
    const r = request({ host: '127.0.0.1', port, method: 'POST', path: '/ai/voice', headers: { 'content-type': 'audio/wav', ...(origin ? { origin } : {}) } }, (res) => {
      res.resume();
      resolve(res.statusCode);
    });
    r.on('error', reject);
    r.end('RIFF');
  });
}

test('WebSocket et POST : une origine étrangère n’a pas les droits locaux, l’interface les a', async () => {
  const { server, wss, port } = await startServer();
  try {
    assert.equal((await hello(port, 'https://evil.example')).local, false);
    assert.equal((await hello(port, `http://evil.example:${port}`)).local, false, 'DNS rebinding');
    assert.equal((await hello(port, 'null')).local, false);
    assert.equal((await hello(port, `http://localhost:${port}`)).local, true, 'interface d’ARKLIGHT');
    assert.equal((await hello(port, `http://127.0.0.1:${port}`)).local, true);
    assert.equal((await hello(port)).local, true, 'client qui n’est pas un navigateur');
    assert.equal(await post(port, 'https://evil.example'), 403);
    assert.equal(await post(port, `http://localhost:${port}`), 200);
  } finally {
    wss.close();
    server.close();
  }
});
