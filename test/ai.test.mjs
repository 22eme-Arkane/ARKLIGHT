// Assistant IA : outils (sans modèle) et conversion des messages pour chaque fournisseur.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { SceneStore } from '../server/scene-store.js';
import { createToolRunner, TOOL_DEFS } from '../server/ai/tools.js';
import { chat, toolCallsInText } from '../server/ai/providers.js';
import { pickModel } from '../server/ai/ollama.js';
import { readZip, createZip } from '../shared/zip.js';
import { deflateRawSync, inflateRawSync } from 'node:zlib';

function withStore(fn) {
  const dir = mkdtempSync(join(tmpdir(), 'ark-ai-'));
  const store = new SceneStore(dir);
  try {
    return fn(store, createToolRunner({ store }));
  } finally {
    clearTimeout(store._timer);
    rmSync(dir, { recursive: true, force: true });
  }
}

test('outils : définitions valides (nom, description, schéma objet)', () => {
  for (const t of TOOL_DEFS) {
    assert.match(t.name, /^[a-z_]+$/);
    assert.ok(t.description.length > 10);
    assert.equal(t.parameters.type, 'object');
  }
});

test('add_fixtures : arc devant le comédien, visés, adressés à la suite, groupés', () => {
  withStore((store, run) => {
    const r = run.run('add_fixtures', { type: 'arri.skypanel-s60c', count: 3, universe: 2, start_address: 101, group: 'Face', cct: 3200, intensity: 40 });
    assert.ok(!r.error, r.error);
    const fx = store.scene.fixtures;
    assert.equal(fx.length, 3);
    for (const f of fx) {
      assert.ok(f.position[2] > 0, 'devant le comédien (z > 0)');
      assert.notDeepEqual(f.rotation, [0, 0, 0]);
      assert.equal(f.settings.universe, 2);
      assert.equal(f.state.manual.cct, 3200);
      assert.equal(f.state.manual.intensity, 0.4);
    }
    assert.deepEqual(fx.map((f) => f.settings.dmxAddress), [101, 113, 125]);
    assert.equal(store.scene.groups[0].name, 'Face');
    assert.equal(store.scene.groups[0].ids.length, 3);
  });
});

test('add_fixtures : nombre déduit des positions, rien sous le sol, tube debout qui pivote', () => {
  withStore((store, run) => {
    run.run('add_fixtures', { type: 'astera.titan', positions: [[-3, 0, -3], [-1, -2, -3], [1, 0, -3]], aim: 'actor', hue: 300 });
    const fx = store.scene.fixtures;
    assert.equal(fx.length, 3);
    for (const f of fx) {
      assert.ok(f.position[1] >= 0.5, 'base du tube au sol');
      assert.equal(f.rotation[0], 0);
      assert.equal(f.rotation[2], 0);
      assert.equal(f.settings.standalone.colors[0].hue, 300); // Astera : mode autonome
    }
  });
});

test('update_fixtures et readdress : par groupe, de gauche à droite, fixtures renumérotées', () => {
  withStore((store, run) => {
    run.run('add_fixtures', { type: 'arri.skypanel-s60c', positions: [[2, 2, 2], [-2, 2, 2]], group: 'Face' });
    run.run('add_fixtures', { type: 'aputure.ls-600d-pro', positions: [[0, 3, -3]] });
    const u = run.run('update_fixtures', { target: { group: 'Face' }, power: false });
    assert.ok(!u.error);
    assert.equal(store.scene.fixtures.filter((f) => f.state.power === false).length, 2);
    const r = run.run('readdress', { target: { all: true }, universe: 1, start_address: 1, order: 'position', fixture_start: 1 });
    assert.ok(!r.error, r.error);
    const byX = [...store.scene.fixtures].sort((a, b) => a.position[0] - b.position[0]);
    assert.equal(byX[0].settings.dmxAddress, 1);
    assert.deepEqual(byX.map((f) => f.settings.fixtureNumber), [1, 2, 3]);
    const addrs = byX.map((f) => f.settings.dmxAddress);
    assert.deepEqual([...addrs].sort((a, b) => a - b), addrs);
  });
});

test('cibles par nom ou numéro de fixture, objets face au comédien, plans', () => {
  withStore((store, run) => {
    run.run('add_fixtures', { type: 'arri.skypanel-s60c', name: 'Contre jardin', positions: [[-2, 3, -3]] });
    assert.ok(!run.run('update_fixtures', { target: { ids: ['contre jardin'] }, intensity: 10 }).error);
    assert.equal(store.scene.fixtures[0].state.manual.intensity, 0.1);
    assert.ok(!run.run('update_fixtures', { target: { ids: ['1'] }, intensity: 20 }).error);
    assert.equal(store.scene.fixtures[0].state.manual.intensity, 0.2);
    const o = run.run('add_objects', { type: 'sofa-3', positions: [[0, 0, -2]], face: 'actor' });
    assert.ok(!o.error, o.error);
    const p = run.run('manage_plans', { action: 'new_plan', mode: 'copy', name: 'Contrechamp' });
    assert.ok(!p.error);
    assert.equal(store.planInfo().sequences[0].plans.length, 2);
    assert.ok(run.changed.has('plans'));
  });
});

test('add_objects : nombre déduit des positions données sans count', () => {
  withStore((store, run) => {
    const before = store.scene.objects.length;
    const r = run.run('add_objects', { type: 'sandbag', positions: [[1, 0, 1], [2, 0, 1], [3, 0, 1]] });
    assert.ok(!r.error, r.error);
    assert.equal(r.added.length, 3);
    assert.equal(store.scene.objects.length, before + 3);
    assert.deepEqual(r.added.map((o) => o.pos[0]), [1, 2, 3]);
  });
});

test('manage_plans : renommer sans id = le plan et la séquence en cours', () => {
  withStore((store, run) => {
    run.run('manage_plans', { action: 'new_sequence', name: 'Extérieur' });
    run.run('manage_plans', { action: 'new_plan', name: 'Jour' });
    run.run('manage_plans', { action: 'rename_plan', name: 'Nuit' });
    run.run('manage_plans', { action: 'rename_sequence', name: 'Rue' });
    const [first, second] = store.planInfo().sequences;
    assert.equal(first.name, 'Séquence 1', 'la première séquence garde son nom');
    assert.equal(first.plans[0].name, 'Plan 1', 'le premier plan garde son nom');
    assert.equal(second.name, 'Rue');
    assert.deepEqual(second.plans.map((p) => p.name).slice(-1), ['Nuit']);
  });
});

test('appels d’outils écrits dans le texte par un petit modèle', () => {
  const names = new Set(['add_fixtures', 'get_scene']);
  assert.deepEqual(toolCallsInText('<tool_call>{"name": "get_scene", "arguments": {}}</tool_call>', names).map((c) => c.name), ['get_scene']);
  assert.equal(toolCallsInText('```json\n{"name":"add_fixtures","arguments":{"type":"x","count":2}}\n```', names)[0].args.count, 2);
  assert.equal(toolCallsInText('Bonjour', names).length, 0);
});

test('choix du modèle local : récent et compatible avec les outils', () => {
  const m = (id, caps, size) => ({ id, caps, size });
  assert.equal(pickModel([m('qwen2.5:7b', ['completion', 'tools'], 4), m('qwen3.6:latest', ['completion', 'tools'], 24), m('qwen2.5vl:7b', ['vision'], 6)]), 'qwen3.6:latest');
  assert.equal(pickModel([m('qwen2.5vl:7b', ['vision', 'completion'], 6)]), '');
  // un modèle qui tient dans une carte graphique de portable passe avant le gros qwen3.6 (36B)
  assert.equal(pickModel([m('qwen3.6:latest', ['completion', 'tools', 'vision'], 24), m('qwen3.5:9b', ['completion', 'tools', 'vision'], 7)]), 'qwen3.5:9b');
});

test('fournisseurs : messages et appels d’outils convertis (Ollama, OpenAI, Anthropic)', async () => {
  const seen = [];
  const saved = globalThis.fetch;
  globalThis.fetch = async (url, init) => {
    const body = JSON.parse(init.body);
    seen.push({ url, body, headers: init.headers });
    let out;
    if (url.endsWith('/api/chat')) out = { message: { content: '', tool_calls: [{ function: { name: 'get_scene', arguments: { include_objects: false } } }] } };
    else if (url.endsWith('/chat/completions')) out = { choices: [{ message: { content: null, tool_calls: [{ id: 'c1', type: 'function', function: { name: 'get_scene', arguments: '{"include_objects":true}' } }] } }] };
    else out = { content: [{ type: 'text', text: 'Je regarde.' }, { type: 'tool_use', id: 'tu1', name: 'get_scene', input: {} }] };
    return new Response(JSON.stringify(out), { status: 200 });
  };
  try {
    const history = [
      { role: 'user', content: 'Ajoute un S60' },
      { role: 'assistant', content: '', toolCalls: [{ id: 'a1', name: 'get_scene', args: {} }] },
      { role: 'tool', toolCallId: 'a1', name: 'get_scene', content: '{"fixtures":[]}' },
    ];
    const tools = TOOL_DEFS.slice(0, 2);
    const o = await chat({ provider: 'ollama', model: 'm', system: 'S', messages: history, tools });
    assert.equal(o.toolCalls[0].name, 'get_scene');
    assert.equal(o.toolCalls[0].args.include_objects, false);
    assert.equal(seen[0].body.messages[0].role, 'system');
    assert.equal(seen[0].body.messages[3].role, 'tool');

    const g = await chat({ provider: 'openai', model: 'm', key: 'k', system: 'S', messages: history, tools });
    assert.equal(g.toolCalls[0].id, 'c1');
    assert.equal(g.toolCalls[0].args.include_objects, true);
    assert.equal(seen[1].headers.authorization, 'Bearer k');
    assert.equal(seen[1].body.messages[2].tool_calls[0].function.arguments, '{}');
    assert.equal(seen[1].body.messages[3].tool_call_id, 'a1');

    const a = await chat({ provider: 'anthropic', model: 'claude-sonnet-5-5', key: 'k', system: 'S', messages: history, tools });
    assert.equal(a.text, 'Je regarde.');
    assert.equal(a.toolCalls[0].id, 'tu1');
    assert.equal(seen[2].body.system, 'S');
    assert.equal(seen[2].body.messages[1].content[0].type, 'tool_use');
    assert.equal(seen[2].body.messages[2].content[0].type, 'tool_result');
    assert.equal(seen[2].body.tools[0].input_schema.type, 'object');

    await assert.rejects(chat({ provider: 'openai', model: 'm', system: 'S', messages: history }), /Clé API/);
  } finally {
    globalThis.fetch = saved;
  }
});

test('ZIP : lecture d’une archive compressée (Deflate) avec la fonction fournie', () => {
  const text = new TextEncoder().encode('bonjour '.repeat(50));
  const stored = readZip(createZip([{ name: 'a.txt', data: text }]));
  assert.equal(new TextDecoder().decode(stored[0].data), 'bonjour '.repeat(50));
  // archive Deflate fabriquée à la main à partir d'une archive STORE
  const zip = createZip([{ name: 'a.txt', data: text }]);
  const comp = deflateRawSync(text);
  const v = new DataView(zip.buffer, zip.byteOffset, zip.byteLength);
  const out = new Uint8Array(zip.length - text.length + comp.length);
  const dataAt = 30 + 5;
  out.set(zip.subarray(0, dataAt));
  out.set(comp, dataAt);
  out.set(zip.subarray(dataAt + text.length), dataAt + comp.length);
  const w = new DataView(out.buffer);
  w.setUint16(8, 8, true); // méthode locale
  w.setUint32(18, comp.length, true);
  const cd = v.getUint32(zip.length - 22 + 16, true) - text.length + comp.length;
  w.setUint16(cd + 10, 8, true);
  w.setUint32(cd + 20, comp.length, true);
  w.setUint32(out.length - 22 + 16, cd, true);
  assert.throws(() => readZip(out), /compression/);
  assert.equal(new TextDecoder().decode(readZip(out, { inflate: (d) => inflateRawSync(d) })[0].data), 'bonjour '.repeat(50));
});

test('place_from_drawing : un dessin (positions dans l’image) devient un plan en mètres', () => {
  withStore((store) => {
    const run = createToolRunner({ store, context: { imageAspect: 1.4 } });
    const r = run.run('place_from_drawing', {
      width_m: 14,
      items: [
        { kind: 'person', label: 'COMÉDIENNE', x: 0.5, y: 0.5 },
        { kind: 'fixture', label: 'SKY S60 - face 3200K', x: 0.27, y: 0.73, aim_x: 0.48, aim_y: 0.52 },
        { kind: 'fixture', label: '4 x TITAN bleu', count: 4, x: 0.21, y: 0.24, x2: 0.79, y2: 0.24 },
        { kind: 'grip', label: '8x8 diff', x: 0.36, y: 0.63 },
        { kind: 'camera', label: 'CAMERA 50mm', x: 0.5, y: 0.9 },
        { kind: 'set', label: 'MUR DU FOND', x: 0.1, y: 0.17, x2: 0.9, y2: 0.17 },
        { kind: 'set', label: 'canapé', x: 0.5, y: 0.35 },
      ],
    });
    assert.ok(!r.error, r.error);
    const objs = store.scene.objects;
    const fx = store.scene.fixtures;
    // le comédien par défaut est remplacé par la comédienne dessinée, au centre
    const woman = objs.find((o) => o.type === 'person-woman');
    assert.ok(woman);
    assert.deepEqual(woman.position, [0, 0, 0]);
    assert.ok(!objs.some((o) => o.type === 'person-man'));
    const sky = fx.find((f) => f.type === 'arri.skypanel-s60c');
    assert.ok(sky.position[0] < 0 && sky.position[2] > 0, 'face devant à jardin');
    assert.equal(sky.state.manual.cct, 3200);
    const titans = fx.filter((f) => f.type === 'astera.titan');
    assert.equal(titans.length, 4);
    for (const t of titans) assert.ok(t.position[2] < 0, 'tubes au fond');
    assert.equal(titans[0].settings.standalone.colors[0].hue, 220);
    assert.ok(objs.some((o) => o.type === 'frame-8x8'));
    const cam = objs.find((o) => o.type === 'camera-tripod');
    assert.equal(cam.props.focal, 50);
    assert.ok(cam.position[2] > 0);
    const walls = objs.filter((o) => o.type === 'wall-flat');
    const total = walls.reduce((sum, w) => sum + w.props.width, 0);
    assert.equal(walls.length, 2); // 11,2 m : deux pans de 5,6 m
    assert.ok(Math.abs(total - 11.2) < 0.05, `largeur ${total}`);
    for (const w of walls) assert.ok(w.position[2] < 0, 'mur au fond');
    assert.ok(objs.some((o) => o.type.startsWith('sofa')));
  });
});

test('get_scene dit qui commande chaque projecteur ; release_control rend la main (console, DMX conservé)', () => {
  const dir = mkdtempSync(join(tmpdir(), 'ark-ai-'));
  const store = new SceneStore(dir);
  try {
    const held = new Set([1]); // univers 1 : dernier état DMX conservé, plus de signal
    const forgotten = [];
    const run = createToolRunner({
      store,
      hooks: {
        dmxState: () => ({ data: new Set(held), live: new Set() }),
        forgetDmx: (list) => list.filter((u) => held.delete(u) && forgotten.push(u)),
      },
    });
    run.run('add_fixtures', { type: 'arri.skypanel-s60c', count: 2, universe: 1 });
    const [a, b] = store.scene.fixtures;
    store.updateFixture(a.id, { state: { console: { mode: a.settings.dmxMode, values: [255] } } });
    const scene = run.run('get_scene', {});
    const ctl = Object.fromEntries(scene.fixtures.map((f) => [f.id, f.control]));
    assert.equal(ctl[a.id], 'console ARKLIGHT');
    assert.match(ctl[b.id], /DMX conservé/);
    const r = run.run('release_control', {});
    assert.ok(!r.error, r.error);
    assert.equal(store.getFixture(a.id).state.console, null);
    assert.deepEqual(forgotten, [1]);
    assert.equal(run.run('get_scene', {}).fixtures.find((f) => f.id === b.id).control, 'réglage manuel');
    // un réglage de la lumière par l'assistant relâche aussi la console
    store.updateFixture(a.id, { state: { console: { mode: a.settings.dmxMode, values: [255] } } });
    run.run('update_fixtures', { target: { ids: [a.id] }, intensity: 30 });
    assert.equal(store.getFixture(a.id).state.console, null);
  } finally {
    clearTimeout(store._timer);
    rmSync(dir, { recursive: true, force: true });
  }
});
