// Panneau "Bibliothèque" : trois onglets sous la recherche — Lumières (par marque, plus la section
// Tungstène & HMI), Pieds, Grip. On glisse un élément dans la vue 3D pour le placer ; le bouton « + »
// l'ajoute à une place proposée.
import { CATEGORIES, FIXTURE_TYPES, isTraditionalType } from '/shared/fixtures/index.js';
import { GRIP_FAMILIES, GRIP_TYPES } from '/shared/grip.js';
import { h } from './dom.js';

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
const GRIP_ICON = {
  frame: '<svg viewBox="0 0 26 18"><rect x="6" y="2" width="14" height="14" fill="currentColor" opacity=".25" stroke="currentColor" stroke-width="1.6"/></svg>',
  flag: '<svg viewBox="0 0 26 18"><rect x="8" y="2" width="11" height="13" fill="currentColor"/><path d="M8 15 L4 17" stroke="currentColor" stroke-width="1.4"/></svg>',
  bounce: '<svg viewBox="0 0 26 18"><rect x="7" y="1" width="12" height="16" fill="#e8e8e8"/></svg>',
  stand: '<svg viewBox="0 0 26 18"><path d="M13 1 V12 M13 12 L6 17 M13 12 L20 17 M13 12 V17" stroke="currentColor" stroke-width="1.6" fill="none"/></svg>',
  box: '<svg viewBox="0 0 26 18"><rect x="4" y="6" width="18" height="9" rx="1" fill="#8a6a45"/></svg>',
  bag: '<svg viewBox="0 0 26 18"><rect x="5" y="7" width="16" height="7" rx="3.5" fill="#6b4a2a"/></svg>',
};

// Les projecteurs LED sont classés par marque ; tungstène et HMI ont leur propre section.
const TRADITIONAL = 'Tungstène & HMI';
const groupOf = (t) => (isTraditionalType(t) ? TRADITIONAL : t.brand);

const TABS = [
  { id: 'lights', label: 'Lumières', placeholder: 'Rechercher un projecteur…' },
  { id: 'stands', label: 'Pieds', placeholder: 'Rechercher un pied…' },
  { id: 'grip', label: 'Grip', placeholder: 'Rechercher dans le grip…' },
];

// Préférences de l'utilisateur, conservées dans le navigateur : onglet actif, marques repliées
const FOLD_KEY = 'arklight.library.folded';
const TAB_KEY = 'arklight.library.tab';
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
  search.after(tabBar);

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

  function fixtureItem(t) {
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
      h('span', { class: 'nm' }, t.name, ' ', h('span', { class: 'md' }, t.model !== t.name ? t.model : '')),
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

  const q = () => search.value.trim().toLowerCase();
  const lights = () => FIXTURE_TYPES.filter((t) => !q() || `${t.brand} ${t.name} ${t.model} ${t.technology || ''}`.toLowerCase().includes(q()));
  const stands = () => GRIP_TYPES.filter((t) => t.family === 'stand' && (!q() || `pied ${t.name} ${t.short}`.toLowerCase().includes(q())));
  const grip = () => GRIP_TYPES.filter((t) => t.family !== 'stand' && (!q() || `grip ${t.name} ${t.short}`.toLowerCase().includes(q())));

  function renderLights(list) {
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

  function render() {
    const counts = { lights: lights().length, stands: stands().length, grip: grip().length };
    for (const t of TABS) {
      const b = tabBtns.get(t.id);
      b.classList.toggle('active', t.id === tab);
      b.querySelector('.count').textContent = ` ${counts[t.id]}`;
    }
    search.placeholder = TABS.find((t) => t.id === tab).placeholder;
    root.replaceChildren();
    if (tab === 'lights') renderLights(lights());
    else if (tab === 'stands') {
      root.append(h('div', { class: 'note lib-note' }, 'Pieds seuls (pour un drapeau, une pince…). Les projecteurs ont déjà leur pied, réglable dans l’inspecteur.'));
      for (const t of stands()) root.append(gripItem(t));
    } else renderGrip(grip());
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
