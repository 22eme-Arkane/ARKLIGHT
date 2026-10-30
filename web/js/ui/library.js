// Panneau "Bibliothèque" : quatre onglets sous la recherche — Lumières (par marque, plus la section
// Tungstène & HMI), Pieds, Grip, Décor (personnages, animaux, véhicules, mobilier…). On glisse un
// élément dans la vue 3D pour le placer ; le bouton « + » l'ajoute à une place proposée.
import { CATEGORIES, FIXTURE_TYPES, isTraditionalType } from '/shared/fixtures/index.js';
import { GRIP_FAMILIES, GRIP_TYPES } from '/shared/grip.js';
import { PROP_CATEGORIES, PROP_TYPES } from '/shared/props.js';
import { lang } from '../i18n.js';
import { h } from './dom.js';
import { icon } from './icons.js';
import { openContextMenu } from './context-menu.js';

export const DRAG_TYPE = 'application/x-arklight-fixture';
export const GRIP_DRAG_TYPE = 'application/x-arklight-grip';

const ICONS = {
  tube: '<svg viewBox="0 0 26 18"><rect x="1" y="7" width="24" height="4" rx="2" fill="currentColor"/></svg>',
  par: '<svg viewBox="0 0 26 18"><rect x="6" y="3" width="11" height="12" rx="2" fill="currentColor"/><path d="M17 5 L25 1 V17 L17 13Z" fill="currentColor" opacity=".35"/></svg>',
  bulb: '<svg viewBox="0 0 26 18"><circle cx="13" cy="10" r="6" fill="currentColor"/><rect x="10.5" y="1" width="5" height="4" fill="currentColor" opacity=".6"/></svg>',
  panel: '<svg viewBox="0 0 26 18"><rect x="5" y="3" width="16" height="12" rx="2" fill="currentColor"/></svg>',
  fresnel: '<svg viewBox="0 0 26 18"><circle cx="13" cy="9" r="7" fill="currentColor"/><circle cx="13" cy="9" r="4.5" fill="none" stroke="#14171c" stroke-width="1"/><circle cx="13" cy="9" r="2" fill="none" stroke="#14171c" stroke-width="1"/></svg>',
  spot: '<svg viewBox="0 0 26 18"><rect x="3" y="5" width="9" height="8" rx="1.5" fill="currentColor"/><path d="M12 6 L24 2 V16 L12 12Z" fill="currentColor" opacity=".35"/></svg>',
  'open-face': '<svg viewBox="0 0 26 18"><path d="M6 4 H20 L17 14 H9Z" fill="currentColor"/></svg>',
  multi: '<svg viewBox="0 0 26 18"><g fill="currentColor"><circle cx="8" cy="4.5" r="2.3"/><circle cx="13" cy="4.5" r="2.3"/><circle cx="18" cy="4.5" r="2.3"/><circle cx="8" cy="9" r="2.3"/><circle cx="13" cy="9" r="2.3"/><circle cx="18" cy="9" r="2.3"/><circle cx="8" cy="13.5" r="2.3"/><circle cx="13" cy="13.5" r="2.3"/><circle cx="18" cy="13.5" r="2.3"/></g></svg>',
};
export const GRIP_ICON = {
  frame: '<svg viewBox="0 0 26 18"><rect x="6" y="2" width="14" height="14" fill="currentColor" opacity=".25" stroke="currentColor" stroke-width="1.6"/></svg>',
  flag: '<svg viewBox="0 0 26 18"><rect x="8" y="2" width="11" height="13" fill="currentColor"/><path d="M8 15 L4 17" stroke="currentColor" stroke-width="1.4"/></svg>',
  bounce: '<svg viewBox="0 0 26 18"><rect x="7" y="1" width="12" height="16" fill="#e8e8e8"/></svg>',
  stand: '<svg viewBox="0 0 26 18"><path d="M13 1 V12 M13 12 L6 17 M13 12 L20 17 M13 12 V17" stroke="currentColor" stroke-width="1.6" fill="none"/></svg>',
  box: '<svg viewBox="0 0 26 18"><rect x="4" y="6" width="18" height="9" rx="1" fill="#8a6a45"/></svg>',
  bag: '<svg viewBox="0 0 26 18"><rect x="5" y="7" width="16" height="7" rx="3.5" fill="#6b4a2a"/></svg>',
};

export const PROP_ICON = {
  people: '<svg viewBox="0 0 26 18"><circle cx="13" cy="4" r="2.6" fill="currentColor"/><path d="M8.5 17 L10 8.5 Q13 7 16 8.5 L17.5 17 Z" fill="currentColor"/></svg>',
  animals: '<svg viewBox="0 0 26 18"><ellipse cx="12" cy="9" rx="7" ry="3.6" fill="currentColor"/><circle cx="20.5" cy="6" r="2.6" fill="currentColor"/><path d="M7 11 V16 M10 12 V16 M14 12 V16 M17 11 V16" stroke="currentColor" stroke-width="1.6"/></svg>',
  vehicles: '<svg viewBox="0 0 26 18"><path d="M2 12 V9 L6 8 L9 4.5 H17 L20.5 8 L24 9 V12 Z" fill="currentColor"/><circle cx="7.5" cy="13" r="2.4" fill="currentColor"/><circle cx="18.5" cy="13" r="2.4" fill="currentColor"/></svg>',
  furniture: '<svg viewBox="0 0 26 18"><rect x="5" y="2" width="3" height="14" fill="currentColor"/><rect x="5" y="9" width="12" height="2.5" fill="currentColor"/><path d="M8 11.5 V16 M16 11.5 V16" stroke="currentColor" stroke-width="2"/></svg>',
  set: '<svg viewBox="0 0 26 18"><rect x="3" y="2" width="20" height="14" fill="currentColor" opacity=".35"/><rect x="10" y="7" width="6" height="9" fill="currentColor"/></svg>',
  outdoor: '<svg viewBox="0 0 26 18"><circle cx="13" cy="7" r="5.5" fill="currentColor"/><rect x="12" y="11" width="2" height="6" fill="currentColor"/></svg>',
  props: '<svg viewBox="0 0 26 18"><rect x="6" y="5" width="14" height="11" rx="1.5" fill="currentColor"/><rect x="10" y="2" width="6" height="3" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>',
  filmset: '<svg viewBox="0 0 26 18"><rect x="6" y="3" width="10" height="6" rx="1" fill="currentColor"/><path d="M16 4.5 L21 2.5 V9.5 L16 7.5Z" fill="currentColor"/><path d="M11 9 L6 17 M11 9 L16 17 M11 9 V17" stroke="currentColor" stroke-width="1.4"/></svg>',
};

// Les projecteurs LED sont classés par marque ; tungstène et HMI ont leur propre section.
const TRADITIONAL = 'Tungstène & HMI';
const groupOf = (t) => (isTraditionalType(t) ? TRADITIONAL : t.brand);

const TABS = [
  { id: 'lights', label: 'Lumières', placeholder: 'Rechercher un projecteur…' },
  { id: 'stands', label: 'Pieds', placeholder: 'Rechercher un pied…' },
  { id: 'grip', label: 'Grip', placeholder: 'Rechercher dans le grip…' },
  { id: 'decor', label: 'Décor', placeholder: 'Personnage, chien, voiture, canapé…' },
];

// Préférences de l'utilisateur, conservées dans le navigateur : onglet actif, marques repliées, tri
const FOLD_KEY = 'arklight.library.folded';
const TAB_KEY = 'arklight.library.tab';
const SORT_KEY = 'arklight.library.sort';

/** Façons de ranger les projecteurs (onglet Lumières). */
const LIGHT_SORTS = [
  { id: 'brand', label: 'Par marque', short: 'Marque' },
  { id: 'type', label: 'Par type (panneaux, fresnels, tubes…)', short: 'Type' },
  { id: 'name', label: 'Par nom (A → Z)', short: 'Nom' },
  { id: 'power', label: 'Par puissance (lux à 1 m)', short: 'Puissance' },
];
const collator = new Intl.Collator(lang, { numeric: true, sensitivity: 'base' });
const luxOf = (t) => t.output?.luxAt1m || 0;
function load(key, fallback) {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : JSON.parse(v);
  } catch {
    return fallback;
  }
}
function save(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* stockage indisponible : tant pis */
  }
}

export function mountLibrary({ onAdd, onAddGrip }) {
  const root = document.getElementById('library');
  const search = document.getElementById('lib-search');
  const folded = new Set(load(FOLD_KEY, []));
  const saved = load(TAB_KEY, 'lights');
  let tab = TABS.some((t) => t.id === saved) ? saved : 'lights';
  const savedSort = load(SORT_KEY, 'brand');
  let sort = LIGHT_SORTS.some((x) => x.id === savedSort) ? savedSort : 'brand';

  // ---- recherche et bouton de tri sur la même ligne
  const sortBtn = h('button', {
    class: 'ib lib-sort',
    title: 'Ranger les projecteurs',
    onclick: (e) => {
      const r = e.currentTarget.getBoundingClientRect();
      openContextMenu(r.left, r.bottom + 4, [
        { header: 'Ranger les projecteurs' },
        ...LIGHT_SORTS.map((x) => ({
          label: x.label,
          checked: x.id === sort,
          onClick: () => {
            sort = x.id;
            save(SORT_KEY, sort);
            render();
          },
        })),
      ]);
    },
  });
  const searchRow = h('div', { class: 'lib-search-row' });
  search.replaceWith(searchRow);
  searchRow.append(search, sortBtn);

  // ---- onglets sous la recherche
  const tabBtns = new Map();
  const tabBar = h(
    'div',
    { class: 'lib-tabs' },
    TABS.map((t) => {
      const b = h('button', { onclick: () => setTab(t.id) }, t.label, h('span', { class: 'count' }));
      tabBtns.set(t.id, b);
      return b;
    }),
  );
  searchRow.after(tabBar);

  function setTab(id) {
    tab = id;
    save(TAB_KEY, id);
    render();
  }

  /** En-tête de marque repliable ; renvoie true si la marque est repliée. */
  const brandHeader = (key, count) => {
    const isFolded = folded.has(key) && !search.value.trim(); // une recherche déplie tout
    root.append(
      h(
        'div',
        {
          class: `lib-brand${isFolded ? ' folded' : ''}`,
          onclick: () => {
            if (folded.has(key)) folded.delete(key);
            else folded.add(key);
            save(FOLD_KEY, [...folded]);
            render();
          },
        },
        h('span', { class: 'caret' }, isFolded ? '▸' : '▾'),
        key,
        h('span', { class: 'count' }, String(count)),
      ),
    );
    return isFolded;
  };

  const startDrag = (type, id, name) => (e) => {
    e.dataTransfer.setData(type, id);
    e.dataTransfer.setData('text/plain', name);
    e.dataTransfer.effectAllowed = 'copy';
    document.body.classList.add('dragging-fixture');
  };
  const endDrag = () => document.body.classList.remove('dragging-fixture');
  const addBtn = (title, fn) =>
    h(
      'button',
      {
        class: 'add',
        title,
        onclick: (e) => {
          e.stopPropagation();
          fn();
        },
      },
      '+',
    );

  function fixtureItem(t, { brand = false, lux = false } = {}) {
    const md = [brand ? t.brand : null, t.model !== t.name ? t.model : null, lux && luxOf(t) ? `${luxOf(t).toLocaleString(lang)} lx` : null].filter(Boolean).join(' · ');
    return h(
      'div',
      {
        class: 'lib-item',
        draggable: 'true',
        title: `Glisse ${t.name} dans la vue 3D pour le placer · « + » : l’ajouter à une place proposée`,
        ondragstart: startDrag(DRAG_TYPE, t.id, t.name),
        ondragend: endDrag,
      },
      h('span', { class: 'ico', html: ICONS[t.category] || ICONS.panel }),
      h('span', { class: 'nm' }, t.name, ' ', h('span', { class: 'md', translate: 'no' }, md)),
      t.provisional ? h('span', { class: 'badge', title: 'Données en cours de vérification' }, 'prov.') : null,
      t.technology === 'hmi' ? h('span', { class: 'badge hmi', title: 'Lampe à décharge (HMI)' }, 'HMI') : null,
      t.confidence && t.confidence !== 'vérifié' ? h('span', { class: 'badge', title: t.source?.notes || 'Données estimées' }, t.confidence === 'estimé' ? 'est.' : '?') : null,
      addBtn(`Ajouter ${t.name} à la scène`, () => onAdd(t)),
    );
  }

  function gripItem(t) {
    return h(
      'div',
      {
        class: 'lib-item',
        draggable: 'true',
        title: `Glisse ${t.name} dans la vue 3D · « + » : l’ajouter devant le personnage`,
        ondragstart: startDrag(GRIP_DRAG_TYPE, t.id, t.name),
        ondragend: endDrag,
      },
      h('span', { class: 'ico', html: GRIP_ICON[t.family] || GRIP_ICON.box }),
      h('span', { class: 'nm' }, t.name),
      addBtn(`Ajouter ${t.name}`, () => onAddGrip?.(t)),
    );
  }

  function propItem(t) {
    return h(
      'div',
      {
        class: 'lib-item',
        draggable: 'true',
        title: `Glisse ${t.name} dans la vue 3D · « + » : l’ajouter à une place libre`,
        ondragstart: startDrag(GRIP_DRAG_TYPE, t.id, t.name),
        ondragend: endDrag,
      },
      h('span', { class: 'ico', html: PROP_ICON[t.cat] || PROP_ICON.props }),
      h('span', { class: 'nm' }, t.name),
      addBtn(`Ajouter ${t.name}`, () => onAddGrip?.(t)),
    );
  }

  const q = () => search.value.trim().toLowerCase();
  const lights = () => FIXTURE_TYPES.filter((t) => !q() || `${t.brand} ${t.name} ${t.model} ${t.technology || ''}`.toLowerCase().includes(q()));
  const stands = () => GRIP_TYPES.filter((t) => t.family === 'stand' && (!q() || `pied ${t.name} ${t.short}`.toLowerCase().includes(q())));
  const grip = () => GRIP_TYPES.filter((t) => t.family !== 'stand' && (!q() || `grip ${t.name} ${t.short}`.toLowerCase().includes(q())));
  const catLabel = (t) => PROP_CATEGORIES.find((c) => c.id === t.cat)?.label || '';
  const decor = () => PROP_TYPES.filter((t) => !q() || `${catLabel(t)} ${t.name} ${t.short}`.toLowerCase().includes(q()));

  function renderLights(list) {
    if (sort === 'name' || sort === 'power') {
      // liste à plat : nom, ou du plus puissant au moins puissant
      const sorted = [...list].sort(sort === 'name' ? (a, b) => collator.compare(a.name, b.name) || collator.compare(a.brand, b.brand) : (a, b) => luxOf(b) - luxOf(a) || collator.compare(a.name, b.name));
      for (const t of sorted) root.append(fixtureItem(t, { brand: true, lux: sort === 'power' }));
      return;
    }
    if (sort === 'type') {
      // par type de projecteur, toutes marques confondues
      for (const cat of CATEGORIES) {
        const items = list.filter((t) => t.category === cat.id).sort((a, b) => collator.compare(a.brand, b.brand) || collator.compare(a.name, b.name));
        if (!items.length || brandHeader(cat.label, items.length)) continue;
        for (const t of items) root.append(fixtureItem(t, { brand: true }));
      }
      return;
    }
    const brands = [...new Set(list.map(groupOf))].sort((a, b) => (a === TRADITIONAL) - (b === TRADITIONAL) || a.localeCompare(b, 'fr'));
    for (const brand of brands) {
      const ofBrand = list.filter((t) => groupOf(t) === brand);
      if (brandHeader(brand, ofBrand.length)) continue;
      for (const cat of CATEGORIES) {
        const items = ofBrand.filter((t) => t.category === cat.id);
        if (!items.length) continue;
        root.append(h('div', { class: 'lib-cat' }, cat.label));
        for (const t of items) root.append(fixtureItem(t));
      }
    }
  }

  function renderGrip(list) {
    for (const fam of GRIP_FAMILIES) {
      const items = list.filter((t) => t.family === fam.id);
      if (!items.length) continue;
      root.append(h('div', { class: 'lib-cat' }, fam.label));
      for (const t of items) root.append(gripItem(t));
    }
  }

  function renderDecor(list) {
    for (const cat of PROP_CATEGORIES) {
      const items = list.filter((t) => t.cat === cat.id);
      if (!items.length || brandHeader(cat.label, items.length)) continue;
      for (const t of items) root.append(propItem(t));
    }
  }

  function render() {
    const counts = { lights: lights().length, stands: stands().length, grip: grip().length, decor: decor().length };
    for (const t of TABS) {
      const b = tabBtns.get(t.id);
      b.classList.toggle('active', t.id === tab);
      b.querySelector('.count').textContent = ` ${counts[t.id]}`;
      b.title = `${t.label} : ${counts[t.id]}`;
    }
    search.placeholder = TABS.find((t) => t.id === tab).placeholder;
    sortBtn.classList.toggle('hidden', tab !== 'lights');
    sortBtn.replaceChildren(icon('sort', 15), (LIGHT_SORTS.find((x) => x.id === sort) || LIGHT_SORTS[0]).short);
    root.replaceChildren();
    if (tab === 'lights') renderLights(lights());
    else if (tab === 'stands') {
      root.append(h('div', { class: 'note lib-note' }, 'Pieds seuls (pour un drapeau, une pince…). Les projecteurs ont déjà leur pied, réglable dans l’inspecteur.'));
      for (const t of stands()) root.append(gripItem(t));
    } else if (tab === 'decor') renderDecor(decor());
    else renderGrip(grip());
    if (!root.querySelector('.lib-item')) root.append(h('div', { class: 'empty' }, 'Rien ne correspond à la recherche'));
  }

  search.addEventListener('input', render);
  render();
  return {
    setTab,
    get tab() {
      return tab;
    },
    /** Place le curseur dans la recherche (Ctrl+F). */
    focusSearch() {
      search.focus();
      search.select();
    },
  };
}
