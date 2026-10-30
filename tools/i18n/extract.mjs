// Recense les textes français de l'interface (chaînes et gabarits ${…}) pour le dictionnaire anglais.
//   node tools/i18n/extract.mjs > textes.json          liste complète
//   node tools/i18n/extract.mjs --missing              textes pas encore traduits (à ajouter dans shared/i18n/en.js)
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../../', import.meta.url));
const DIRS = ['web/js', 'shared', 'server'];
const SKIP = /node_modules|i18n[\\/]en\.js|data[\\/]ofl-fixtures|[\\/]ble[\\/]/;

function* files(dir) {
  for (const f of readdirSync(dir)) {
    const p = join(dir, f);
    if (SKIP.test(p)) continue;
    if (statSync(p).isDirectory()) yield* files(p);
    else if (/\.(m?js)$/.test(f)) yield p;
  }
}

/** Chaînes d'un source JS : '…', "…", `…` (les ${…} deviennent {0}, {1}…). Commentaires ignorés. */
export function strings(src) {
  const out = [];
  let i = 0;
  const n = src.length;
  let prev = '';
  while (i < n) {
    const c = src[i];
    if (c === '/' && src[i + 1] === '/') {
      i = src.indexOf('\n', i);
      if (i < 0) break;
      continue;
    }
    if (c === '/' && src[i + 1] === '*') {
      i = src.indexOf('*/', i + 2);
      if (i < 0) break;
      i += 2;
      continue;
    }
    // expression régulière littérale (après un opérateur ou une parenthèse) : on la saute
    if (c === '/' && /[(,=:[!&|?{};]/.test(prev)) {
      let j = i + 1;
      let cls = false;
      while (j < n && (src[j] !== '/' || cls)) {
        if (src[j] === '\\') j++;
        else if (src[j] === '[') cls = true;
        else if (src[j] === ']') cls = false;
        else if (src[j] === '\n') break;
        j++;
      }
      i = j + 1;
      prev = '/';
      continue;
    }
    if (c === "'" || c === '"') {
      let j = i + 1;
      let s = '';
      while (j < n && src[j] !== c) {
        if (src[j] === '\\') {
          s += src[j + 1] === 'n' ? '\n' : src[j + 1];
          j += 2;
        } else s += src[j++];
      }
      out.push(s);
      i = j + 1;
      prev = 'a';
      continue;
    }
    if (c === '`') {
      let j = i + 1;
      let s = '';
      let k = 0;
      while (j < n && src[j] !== '`') {
        if (src[j] === '\\') {
          s += src[j + 1] === 'n' ? '\n' : src[j + 1];
          j += 2;
        } else if (src[j] === '$' && src[j + 1] === '{') {
          let depth = 1;
          j += 2;
          while (j < n && depth) {
            if (src[j] === '{') depth++;
            else if (src[j] === '}') depth--;
            else if (src[j] === '`') {
              j++;
              while (j < n && src[j] !== '`') j++;
            } else if (src[j] === "'" || src[j] === '"') {
              const q = src[j++];
              while (j < n && src[j] !== q) j += src[j] === '\\' ? 2 : 1;
            }
            j++;
          }
          s += `{${k++}}`;
        } else s += src[j++];
      }
      out.push(s);
      i = j + 1;
      prev = 'a';
      continue;
    }
    if (!/\s/.test(c)) prev = c;
    i++;
  }
  return out;
}

const FRENCH = /[àâäçéèêëîïôöùûüÿœæÀÂÇÉÈÊÎÔÙÛŒ’«»]|\b(le|la|les|un|une|des|du|de|et|ou|pour|avec|sans|dans|sur|par|au|aux|ce|cette|est|pas|plus|tout|tous|son|sa|ses|ton|ta|tes|vue|pied|pieds|projecteur|projecteurs|scène|projet|nom|fichier|ouvrir|enregistrer|supprimer|ajouter|couleur|hauteur|largeur|aucun|aucune)\b/i;

export function collect() {
  const items = new Map();
  for (const f of DIRS.flatMap((d) => [...files(join(ROOT, d))])) {
    const rel = relative(ROOT, f).replace(/\\/g, '/');
    for (const s of strings(readFileSync(f, 'utf8'))) {
      const t = s.trim();
      if (t.length < 2 || !/[a-zA-Zà-ÿ]/.test(t)) continue;
      if (/^[\w.\-/:#@%?=&]+$/.test(t) && !/^[A-ZÀ-Ý][a-zà-ÿ]+$/.test(t)) continue; // identifiants, chemins, classes
      if (/^(https?:|\/|\.\/|\.\.\/|#[0-9a-f]{3,8}$|data:|<|M\d)/i.test(t)) continue;
      if (!FRENCH.test(t)) continue;
      const cur = items.get(t) || { text: t, files: new Set() };
      cur.files.add(rel);
      items.set(t, cur);
    }
  }
  const html = readFileSync(join(ROOT, 'web/index.html'), 'utf8');
  const add = (t) => {
    t = t.replace(/\s+/g, ' ').trim();
    if (t && /[a-zA-Zà-ÿ]/.test(t) && !items.has(t)) items.set(t, { text: t, files: new Set(['web/index.html']) });
  };
  for (const m of html.matchAll(/>([^<>{}]*)</g)) add(m[1]);
  for (const m of html.matchAll(/(?:title|placeholder|aria-label|data-tip)="([^"]+)"/g)) add(m[1]);
  return [...items.values()].map((x) => ({ text: x.text, files: [...x.files] }));
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const list = collect();
  if (process.argv.includes('--missing')) {
    const { EN } = await import('../../shared/i18n/en.js');
    const missing = list.filter((x) => !(x.text in EN));
    process.stdout.write(JSON.stringify(missing, null, 1));
    console.error(`${missing.length} textes sans traduction (sur ${list.length})`);
  } else {
    process.stdout.write(JSON.stringify(list, null, 1));
    console.error(`${list.length} textes`);
  }
}
