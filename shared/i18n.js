// Langues de l'interface. Les textes sont écrits en français dans le code (langue d'origine) ; le
// dictionnaire anglais (shared/i18n/en.js) les traduit. Les parties variables s'écrivent {0}, {1}…
// ou {nom} : « Projet enregistré : {0} » → « Project saved: {0} ». Un texte déjà composé (« Projet
// enregistré : Pub ») est reconnu grâce à ces gabarits, et ses parties variables connues du
// dictionnaire sont traduites aussi (« Pied 1000 · h 2 m » → « Baby stand · h 2 m »).
// À chaque nouvelle fonction : ajouter ses textes au dictionnaire (node tools/i18n/extract.mjs --missing).
import { EN } from './i18n/en.js';

export const LANGS = [
  { id: 'fr', label: 'Français' },
  { id: 'en', label: 'English' },
];

const DICTS = { en: EN };
const SLOT = /\{(\w+)\}/g;
const compiled = new Map(); // langue → gabarits rangés par début de texte

function escapeRe(s) {
  return s.replace(/[.*+?^$()|[\]\\]/g, '\\$&');
}

function templatesOf(lang) {
  if (compiled.has(lang)) return compiled.get(lang);
  const byStart = new Map(); // 3 premiers caractères fixes → gabarits
  const loose = []; // gabarits qui commencent par une partie variable
  for (const [fr, out] of Object.entries(DICTS[lang] || {})) {
    if (!fr.includes('{')) continue;
    // assez de texte fixe pour ne pas reconnaître n'importe quoi (« le {0} » ne sert qu'avec t())
    if ((fr.replace(SLOT, '').match(/[A-Za-zÀ-ÿ]/g) || []).length < 5) continue;
    const names = [];
    const parts = fr.split(SLOT);
    let re = '^';
    for (let i = 0; i < parts.length; i++) {
      if (i % 2) {
        names.push(parts[i]);
        re += '(.+?)';
      } else re += escapeRe(parts[i]);
    }
    const tpl = { re: new RegExp(`${re}$`, 's'), names, out, head: parts[0], weight: fr.replace(SLOT, '').length };
    if (tpl.head.length >= 3) {
      const k = tpl.head.slice(0, 3);
      if (!byStart.has(k)) byStart.set(k, []);
      byStart.get(k).push(tpl);
    } else loose.push(tpl);
  }
  const heavier = (a, b) => b.weight - a.weight;
  for (const list of byStart.values()) list.sort(heavier);
  loose.sort(heavier);
  const res = { byStart, loose };
  compiled.set(lang, res);
  return res;
}

/** Traduction d'un texte, ou null s'il n'est pas connu du dictionnaire. */
export function lookup(text, lang, depth = 0) {
  const dict = DICTS[lang];
  if (!dict || typeof text !== 'string' || !text) return null;
  if (Object.prototype.hasOwnProperty.call(dict, text)) return dict[text];
  if (depth > 2 || !/[A-Za-zÀ-ÿ]/.test(text)) return null;
  const { byStart, loose } = templatesOf(lang);
  // d'abord les gabarits qui commencent comme le texte, puis les autres (les plus précis d'abord)
  const candidates = [...(byStart.get(text.slice(0, 3)) || []), ...loose];
  for (const tpl of candidates) {
    const m = tpl.re.exec(text);
    if (!m) continue;
    const values = Object.fromEntries(tpl.names.map((n, i) => [n, m[i + 1]]));
    // les parties variables connues du dictionnaire sont traduites elles aussi
    return tpl.out.replace(SLOT, (all, n) => {
      if (!(n in values)) return all;
      const v = values[n];
      return lookup(v, lang, depth + 1) ?? v;
    });
  }
  return null;
}

/** Texte dans la langue voulue ; params remplace les {nom} / {0}. */
export function translate(text, lang, params) {
  let s = lang && lang !== 'fr' ? (lookup(text, lang) ?? text) : text;
  if (params) s = s.replace(SLOT, (all, k) => (k in params ? String(params[k]) : all));
  return s;
}

/** Traduit les textes d'un document HTML ou SVG déjà écrit (exports du plan de feu). */
export function translateMarkup(markup, lang) {
  if (!lang || lang === 'fr') return markup;
  return markup
    .replace(/>([^<>]+)</g, (all, txt) => {
      const core = txt.trim();
      const tr = core && lookup(decode(core), lang);
      return tr == null ? all : `>${txt.replace(core, encode(tr))}<`;
    })
    .replace(/\b(title|aria-label)="([^"]+)"/g, (all, a, v) => {
      const tr = lookup(decode(v), lang);
      return tr == null ? all : `${a}="${encode(tr)}"`;
    });
}

const decode = (s) => s.replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, '&');
const encode = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
