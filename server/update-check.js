// Vérification des mises à jour, comme dans PANDORA : la dernière version publiée sur GitHub
// (Releases du dépôt indiqué dans package.json > arklight.updateRepo). Les versions alpha et bêta
// (« pré-versions » GitHub) comptent : le logiciel est lui-même en alpha.
import { readFileSync } from 'node:fs';

const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
export const APP_VERSION = pkg.version;
export const UPDATE_REPO = pkg.arklight?.updateRepo || '';

/** Compare deux versions semver (« 0.2.0 », « v0.1.0-alpha.3 ») : −1, 0 ou 1. */
export function compareVersions(a, b) {
  const parse = (v) => {
    const [core, pre = ''] = String(v).trim().replace(/^v/i, '').split('+')[0].split(/-(.*)/s);
    return { nums: core.split('.').map((n) => Number(n) || 0), pre: pre ? pre.split('.') : [] };
  };
  const A = parse(a);
  const B = parse(b);
  for (let i = 0; i < 3; i++) if ((A.nums[i] || 0) !== (B.nums[i] || 0)) return (A.nums[i] || 0) > (B.nums[i] || 0) ? 1 : -1;
  // une version finale passe avant ses pré-versions (1.0.0 > 1.0.0-alpha.9)
  if (!A.pre.length || !B.pre.length) return A.pre.length === B.pre.length ? 0 : A.pre.length ? -1 : 1;
  for (let i = 0; i < Math.max(A.pre.length, B.pre.length); i++) {
    const x = A.pre[i];
    const y = B.pre[i];
    if (x === undefined) return -1;
    if (y === undefined) return 1;
    const nx = /^\d+$/.test(x);
    const ny = /^\d+$/.test(y);
    if (nx && ny && Number(x) !== Number(y)) return Number(x) > Number(y) ? 1 : -1;
    if (nx !== ny) return nx ? -1 : 1;
    if (x !== y) return x > y ? 1 : -1;
  }
  return 0;
}

/**
 * @returns {Promise<{status: 'available'|'up-to-date'|'none'|'no-repo'|'error', current: string, latest?: string,
 *   url?: string, download?: string, notes?: string, prerelease?: boolean, date?: string, message?: string}>}
 */
export async function checkForUpdate(repo = UPDATE_REPO) {
  const current = APP_VERSION;
  if (!repo) return { status: 'no-repo', current };
  try {
    const res = await fetch(`https://api.github.com/repos/${repo}/releases?per_page=15`, {
      headers: { Accept: 'application/vnd.github+json', 'User-Agent': 'ARKLIGHT' },
      signal: AbortSignal.timeout(10_000),
    });
    if (res.status === 404) return { status: 'no-repo', current };
    if (!res.ok) return { status: 'error', current, message: `GitHub : ${res.status}` };
    const releases = (await res.json()).filter((r) => !r.draft);
    releases.sort((x, y) => compareVersions(y.tag_name, x.tag_name));
    const rel = releases[0];
    if (!rel) return { status: 'none', current };
    const latest = rel.tag_name.replace(/^v/i, '');
    const asset = (rel.assets || []).find((a) => /\.exe$/i.test(a.name));
    return {
      status: compareVersions(latest, current) > 0 ? 'available' : 'up-to-date',
      current,
      latest,
      url: rel.html_url,
      download: asset?.browser_download_url,
      notes: (rel.body || '').slice(0, 3000),
      prerelease: !!rel.prerelease,
      date: rel.published_at,
    };
  } catch (err) {
    return { status: 'error', current, message: err.message };
  }
}
