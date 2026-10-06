// Serveur de fichiers statiques : interface web, modules partagés et three.js.
import { createReadStream, statSync } from 'node:fs';
import { extname, join, normalize, resolve, sep } from 'node:path';

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.webp': 'image/webp',
  '.woff2': 'font/woff2',
  '.glb': 'model/gltf-binary',
  '.hdr': 'application/octet-stream',
  '.wasm': 'application/wasm',
};

/**
 * @param {{prefix: string, dir: string}[]} mounts  préfixes d'URL -> dossiers (le plus spécifique d'abord)
 */
export function createStaticHandler(mounts) {
  const roots = mounts.map((m) => ({ prefix: m.prefix, dir: resolve(m.dir) }));

  return function handle(req, res) {
    if (req.method !== 'GET' && req.method !== 'HEAD') return false;
    let pathname;
    try {
      pathname = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    } catch {
      return false;
    }
    const mount = roots.find((m) => pathname === m.prefix.replace(/\/$/, '') || pathname.startsWith(m.prefix));
    if (!mount) return false;

    let rel = pathname.slice(mount.prefix.length);
    if (rel === '' || rel.endsWith('/')) rel += 'index.html';
    const file = normalize(join(mount.dir, rel));
    if (file !== mount.dir && !file.startsWith(mount.dir + sep)) {
      res.writeHead(403).end();
      return true;
    }
    let st;
    try {
      st = statSync(file);
    } catch {
      return false;
    }
    if (!st.isFile()) return false;

    res.writeHead(200, {
      'Content-Type': MIME[extname(file).toLowerCase()] || 'application/octet-stream',
      'Content-Length': st.size,
      'Cache-Control': 'no-cache',
    });
    if (req.method === 'HEAD') return res.end(), true;
    createReadStream(file).pipe(res);
    return true;
  };
}
