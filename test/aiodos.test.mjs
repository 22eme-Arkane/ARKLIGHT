// AIODOS (IA locale d'ARKANE) comme moteur de l'assistant : essais contre le faux AIODOS du client
// officiel (aiodos-client/fake), sans réseau ni vraie installation.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { FAKE_KEY, createFakeFetch } from 'aiodos-client/fake';
import { createAiodos } from '../server/ai/aiodos.js';
import { PROVIDERS, chat, listModels, useAiodos } from '../server/ai/providers.js';

/** Coffre des clés d'ARKLIGHT, en mémoire (même interface que KeyStore). */
function memoryKeys(initial = {}) {
  const keys = { ...initial };
  return { get: (id) => keys[id] || '', set: (id, v) => (v ? (keys[id] = v) : delete keys[id]), keys };
}

function setup({ key = FAKE_KEY, fake = {} } = {}) {
  const fetch = createFakeFetch(fake);
  const keys = memoryKeys(key ? { aiodos: key } : {});
  const aiodos = createAiodos({
    keys,
    version: '0.0.0-essai',
    lang: () => 'fr',
    test: { fetch, url: fetch.fake.url, env: { AIODOS_DISCOVERY_FILE: '-', AIODOS_HOME: join(tmpdir(), 'arklight-sans-aiodos') } },
  });
  return { aiodos, keys, fake: fetch.fake };
}

test('AIODOS : un moteur local de l’assistant, sans adresse ni clé API à saisir', () => {
  assert.equal(PROVIDERS.aiodos.kind, 'aiodos');
  assert.equal(PROVIDERS.aiodos.local, true);
  assert.ok(!PROVIDERS.aiodos.key);
});

test('AIODOS : prêt avec la clé d’ARKLIGHT ; modèles au format des réglages ; conversation par le fournisseur', async () => {
  const { aiodos } = setup();
  const st = await aiodos.status();
  assert.equal(st.state, 'ready');
  assert.equal(st.code, null);
  useAiodos(aiodos);
  const models = await listModels({ provider: 'aiodos' });
  assert.ok(models.length > 0);
  assert.ok(models.every((m) => typeof m.id === 'string' && Array.isArray(m.caps)));
  const r = await chat({ provider: 'aiodos', model: models[0].id, system: 'Tu es un assistant.', messages: [{ role: 'user', content: 'Bonjour' }] });
  assert.equal(typeof r.text, 'string');
  assert.ok(r.text.length > 0);
  assert.deepEqual(r.toolCalls, []);
  assert.ok(r.by?.model, 'qui a calculé');
});

test('AIODOS : un appel d’outil arrive au format de l’assistant ({ id, name, args })', async () => {
  const { aiodos, fake } = setup();
  useAiodos(aiodos);
  fake.nextChat('conversation en flux : appel d\'outil');
  const tools = [{ name: 'set_light', description: 'Règle un projecteur.', parameters: { type: 'object', properties: { fixture: { type: 'number' }, level: { type: 'number' } } } }];
  const r = await chat({ provider: 'aiodos', model: 'qwen3.5:9b', system: 'Tu règles des projecteurs.', messages: [{ role: 'user', content: 'Mets le 3 à 80 %' }], tools });
  assert.equal(r.toolCalls.length, 1);
  assert.equal(r.toolCalls[0].name, 'set_light');
  assert.deepEqual(r.toolCalls[0].args, { fixture: 3, level: 80 });
  assert.equal(r.by.machine, 'PC-BUREAU');
  // le calcul est demandé sur ce PC
  assert.equal(fake.received('/v1/chat/completions').at(-1).headers['x-aiodos-where'], 'here');
});

test('AIODOS : connexion sur un clic — le code à 6 chiffres, l’accord, la clé rangée dans le coffre d’ARKLIGHT', async () => {
  const { aiodos, keys, fake } = setup({ key: null });
  assert.equal((await aiodos.status()).state, 'needs_access');
  let shown = null;
  const done = aiodos.connect({
    onCode: (code) => {
      shown = code;
      fake.approve();
    },
  });
  await done;
  assert.match(shown, /^\d{3} ?\d{3}$/);
  assert.ok(keys.get('aiodos'), 'clé rangée');
  assert.equal((await aiodos.status()).state, 'ready');
  // la demande ne porte que la conversation, calculée sur ce PC
  const req = fake.received('/aiodos/v1/access/requests').at(-1).json;
  assert.equal(req.scope, 'chat');
  assert.deepEqual(req.where, ['here']);
  assert.equal(req.client_name, 'ARKLIGHT');
  // se déconnecter efface la clé
  await aiodos.disconnect();
  assert.equal(keys.get('aiodos'), '');
});

test('AIODOS : une demande abandonnée ne laisse ni clé ni demande en cours', async () => {
  const { aiodos, keys } = setup({ key: null });
  let pending;
  const done = aiodos.connect({ onCode: () => (pending = aiodos.status()) });
  await new Promise((r) => setTimeout(r, 20));
  assert.equal((await pending).state, 'pending');
  aiodos.cancel();
  await assert.rejects(done, (e) => e.code === 'cancelled');
  assert.equal(keys.get('aiodos'), '');
  assert.notEqual((await aiodos.status()).state, 'pending');
});
