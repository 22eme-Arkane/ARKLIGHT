// Fichier des clés de l'assistant IA : rangé dans le profil de l'application, déplacé de
// Documents\ARKLIGHT au premier lancement, sans rien perdre.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { placeKeyFile } from '../desktop/keys-file.js';

function dirs() {
  const root = mkdtempSync(join(tmpdir(), 'arklight-cles-'));
  const docs = join(root, 'Documents', 'ARKLIGHT');
  const profile = join(root, 'AppData', 'arklight');
  mkdirSync(docs, { recursive: true });
  writeFileSync(join(docs, 'config.json'), '{}');
  return { root, docs, profile, legacy: join(docs, 'ai-keys.json'), file: join(profile, 'ai-keys.json') };
}

const age = (path, seconds) => utimesSync(path, new Date(Date.now() - seconds * 1000), new Date(Date.now() - seconds * 1000));

test('clés : déplacées de Documents vers le profil de l’application (dossier créé), le reste ne bouge pas', () => {
  const d = dirs();
  try {
    writeFileSync(d.legacy, '{"aiodos":"enc:ancienne"}');
    assert.equal(placeKeyFile(d.file, d.legacy), d.file);
    assert.equal(readFileSync(d.file, 'utf8'), '{"aiodos":"enc:ancienne"}');
    assert.ok(!existsSync(d.legacy), 'plus de clé dans Documents');
    assert.deepEqual(readdirSync(d.docs), ['config.json']);
    // lancements suivants : rien à faire
    assert.equal(placeKeyFile(d.file, d.legacy), d.file);
    assert.equal(readFileSync(d.file, 'utf8'), '{"aiodos":"enc:ancienne"}');
  } finally {
    rmSync(d.root, { recursive: true, force: true });
  }
});

test('clés : aux deux endroits, la plus récente est gardée dans le profil', () => {
  const d = dirs();
  try {
    mkdirSync(d.profile, { recursive: true });
    // celle du profil est la plus récente : celle de Documents s'efface
    writeFileSync(d.legacy, 'documents');
    writeFileSync(d.file, 'profil');
    age(d.legacy, 60);
    assert.equal(placeKeyFile(d.file, d.legacy), d.file);
    assert.equal(readFileSync(d.file, 'utf8'), 'profil');
    assert.ok(!existsSync(d.legacy));
    // celle de Documents est la plus récente (ancienne version d'ARKLIGHT relancée) : elle remplace l'autre
    writeFileSync(d.legacy, 'documents, plus récente');
    age(d.file, 60);
    assert.equal(placeKeyFile(d.file, d.legacy), d.file);
    assert.equal(readFileSync(d.file, 'utf8'), 'documents, plus récente');
    assert.ok(!existsSync(d.legacy));
    assert.deepEqual(readdirSync(d.docs), ['config.json']);
    assert.deepEqual(readdirSync(d.profile), ['ai-keys.json']);
  } finally {
    rmSync(d.root, { recursive: true, force: true });
  }
});

test('clés : sans ancien fichier, ou même chemin, rien ne bouge', () => {
  const d = dirs();
  try {
    assert.equal(placeKeyFile(d.file, d.legacy), d.file);
    assert.equal(placeKeyFile(d.file), d.file);
    assert.ok(!existsSync(d.profile), 'rien de créé pour rien');
    writeFileSync(d.legacy, 'x');
    assert.equal(placeKeyFile(d.legacy, d.legacy), d.legacy);
    assert.equal(readFileSync(d.legacy, 'utf8'), 'x');
  } finally {
    rmSync(d.root, { recursive: true, force: true });
  }
});

test('clés : déplacement impossible, l’ancien fichier sert encore cette fois (rien n’est perdu)', () => {
  const d = dirs();
  try {
    writeFileSync(d.legacy, 'documents');
    mkdirSync(join(d.root, 'AppData'), { recursive: true });
    writeFileSync(d.profile, 'un fichier à la place du dossier du profil');
    assert.equal(placeKeyFile(d.file, d.legacy), d.legacy);
    assert.equal(readFileSync(d.legacy, 'utf8'), 'documents');
  } finally {
    rmSync(d.root, { recursive: true, force: true });
  }
});
