// Tests de la comparaison de versions (détection des mises à jour, versions alpha comprises).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compareVersions } from '../server/update-check.js';

test('versions : ordre semver, pré-versions alpha / bêta', () => {
  assert.equal(compareVersions('0.1.0-alpha.2', '0.1.0-alpha.1'), 1);
  assert.equal(compareVersions('v0.1.0-alpha.10', '0.1.0-alpha.9'), 1, 'numéros comparés comme des nombres');
  assert.equal(compareVersions('0.1.0-beta.1', '0.1.0-alpha.7'), 1);
  assert.equal(compareVersions('0.1.0', '0.1.0-rc.1'), 1, 'la version finale passe avant ses pré-versions');
  assert.equal(compareVersions('0.2.0-alpha.1', '0.1.9'), 1);
  assert.equal(compareVersions('1.0.0', 'v1.0.0'), 0);
  assert.equal(compareVersions('0.1.0-alpha.1', '0.1.0-alpha.1'), 0);
  assert.equal(compareVersions('0.1.0-alpha.1', '0.1.0-alpha.2'), -1);
});
