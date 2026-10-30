// Langue de l'interface (français ou anglais), choisie dans le menu « Langue » ou sur l'écran des
// projets. t('Nouveau projet') → « New project » en anglais. Les textes déjà écrits dans les
// panneaux sont traduits à l'affichage (observateur de la page) : seuls les textes composés hors
// de la page (fenêtres confirm, exports, titres de fenêtres) passent explicitement par t().
import { LANGS, lookup, translate, translateMarkup } from '/shared/i18n.js';

export { LANGS };
const KEY = 'arklight.lang';

/**
 * Langue de l'ordinateur : celles choisies dans Windows (application de bureau), sinon celles du
 * navigateur. Le français si c'est la première des langues connues, sinon l'anglais.
 */
function systemLang() {
  const prefs = [...(window.arkDesktop?.systemLanguages || []), ...(navigator.languages || []), navigator.language].filter(Boolean);
  for (const p of prefs) {
    const id = String(p).toLowerCase().split(/[-_]/)[0];
    if (LANGS.some((l) => l.id === id)) return id;
  }
  return 'en';
}
export const systemLanguage = systemLang();

/** Choix mémorisé : 'fr', 'en', ou 'auto' (suivre la langue de l'ordinateur, par défaut). */
function savedChoice() {
  try {
    const saved = localStorage.getItem(KEY);
    if (LANGS.some((l) => l.id === saved)) return saved;
  } catch {
    /* stockage indisponible */
  }
  return 'auto';
}
let choice = savedChoice();
export const langChoice = () => choice;

function detect() {
  // ?lang=en dans l'adresse (tests automatiques) l'emporte sur le choix mémorisé
  const forced = new URLSearchParams(location.search).get('lang');
  if (LANGS.some((l) => l.id === forced)) return forced;
  return choice === 'auto' ? systemLanguage : choice;
}

export const lang = detect();
document.documentElement.lang = lang;

/** Texte dans la langue de l'interface ; params remplace les {nom}. */
export const t = (fr, params) => translate(fr, lang, params);

/** Document HTML ou SVG composé (exports) traduit. */
export const tMarkup = (markup) => translateMarkup(markup, lang);

/**
 * Change de langue : mémorise le choix ('auto' : la langue de l'ordinateur) et recharge l'interface
 * si elle change (le projet reste ouvert sur le serveur).
 */
export function setLang(id, { reopenProjects = false } = {}) {
  if (id !== 'auto' && !LANGS.some((l) => l.id === id)) return;
  if (id === choice) return;
  choice = id;
  try {
    if (id === 'auto') localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, id);
    if ((id === 'auto' ? systemLanguage : id) === lang) return; // même langue : rien à recharger
    sessionStorage.setItem('arklight.langReload', reopenProjects ? 'projects' : '1');
  } catch {
    /* sans stockage, le choix ne tient que jusqu'au rechargement */
  }
  location.reload();
}

/** Rechargement dû à un changement de langue : « projects » (rouvrir l'écran des projets), « 1 », ou null. */
export function takeLangReload() {
  try {
    const v = sessionStorage.getItem('arklight.langReload');
    sessionStorage.removeItem('arklight.langReload');
    return v;
  } catch {
    return null;
  }
}

// ------------------------------------------------------------------ traduction de la page
const ATTRS = ['title', 'placeholder', 'aria-label', 'data-tip'];
// ni scripts, ni contenus marqués translate="no" (noms donnés par l'utilisateur…) ; les champs de saisie
// gardent leur valeur (seuls leurs attributs, comme placeholder, sont traduits)
const SKIP = 'script, style, [translate="no"]';

// ce que la traduction a écrit : on ne retraduit jamais son propre résultat (pas de boucle)
const written = new WeakMap(); // nœud texte → texte écrit
const writtenAttr = new WeakMap(); // élément → { attribut: valeur écrite }

function textNode(n) {
  if (n.parentElement?.tagName === 'TEXTAREA') return;
  const v = n.nodeValue;
  if (!v || v.length > 400 || written.get(n) === v || !/[A-Za-zÀ-ÿ]/.test(v)) return;
  const core = v.trim();
  const tr = lookup(core, lang);
  if (tr == null || tr === core) return;
  const out = v.replace(core, tr);
  written.set(n, out);
  n.nodeValue = out;
}

function attr(el, name) {
  const v = el.getAttribute(name);
  if (!v) return;
  const mine = writtenAttr.get(el);
  if (mine?.[name] === v) return;
  const tr = lookup(v.trim(), lang);
  if (tr == null || tr === v) return;
  writtenAttr.set(el, { ...mine, [name]: tr });
  el.setAttribute(name, tr);
}

function walk(root) {
  if (root.nodeType === Node.TEXT_NODE) {
    if (!root.parentElement?.closest(SKIP)) textNode(root);
    return;
  }
  if (root.nodeType !== Node.ELEMENT_NODE || root.closest(SKIP)) return;
  for (const a of ATTRS) attr(root, a);
  const tw = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
    acceptNode: (n) => (n.nodeType === Node.ELEMENT_NODE && n.matches(SKIP) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
  });
  for (let n = tw.nextNode(); n; n = tw.nextNode()) {
    if (n.nodeType === Node.TEXT_NODE) textNode(n);
    else for (const a of ATTRS) attr(n, a);
  }
}

/** Traduit la page puis tout ce qui s'y affiche ensuite (menus, panneaux, fenêtres, messages). */
export function translatePage() {
  if (lang === 'fr') return;
  walk(document.body);
  const titleEl = document.querySelector('title');
  if (titleEl) walk(titleEl);
  new MutationObserver((muts) => {
    for (const m of muts) {
      if (m.type === 'characterData') {
        if (!m.target.parentElement?.closest(SKIP)) textNode(m.target);
      } else if (m.type === 'attributes') {
        if (!m.target.closest(SKIP)) attr(m.target, m.attributeName);
      } else for (const n of m.addedNodes) walk(n);
    }
  }).observe(document.documentElement, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS });
}
