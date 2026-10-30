// Panneau "Scène" : groupes et projecteurs placés, avec aperçu de couleur, patch et sélection
// multiple ; puis les objets (grip et décor : personnages, véhicules, mobilier…).
import { rgbToCss } from '/shared/engine/color.js';
import { consoleHeld } from '/shared/engine/console.js';
import { getFixtureType } from '/shared/fixtures/index.js';
import { getObjectType, isProp } from '/shared/objects.js';
import { lang, t } from '../i18n.js';
import { h } from './dom.js';
import { icon } from './icons.js';
import { patchIssues, patchLabel } from './patch.js';
import { openContextMenu } from './context-menu.js';
import { GRIP_ICON, PROP_ICON } from './library.js';

const OBJ_FOLD_KEY = 'arklight.scene.objectsFolded';
const SORT_KEY = 'arklight.scene.sort';

/** Façons de trier les projecteurs de la liste. */
const SORTS = [
  { id: 'scene', label: 'Ordre d’ajout', short: 'Ajout' },
  { id: 'name', label: 'Nom (A → Z)', short: 'Nom' },
  { id: 'dmx', label: 'Adresse DMX (univers, adresse)', short: 'DMX' },
  { id: 'model', label: 'Modèle', short: 'Modèle' },
  { id: 'fix', label: 'Numéro de fixture (Fix)', short: 'Fix' },
];
const collator = new Intl.Collator(lang, { numeric: true, sensitivity: 'base' }); // « Titan 2 » avant « Titan 10 »
const byName = (a, b) => collator.compare(a.name, b.name);
const modelOf = (f) => {
  const type = getFixtureType(f.type);
  return type ? `${type.brand} ${type.name}` : f.type;
};
const COMPARE = {
  name: byName,
  dmx: (a, b) => a.settings.universe - b.settings.universe || a.settings.dmxAddress - b.settings.dmxAddress || byName(a, b),
  model: (a, b) => collator.compare(modelOf(a), modelOf(b)) || byName(a, b),
  fix: (a, b) => (a.settings.fixtureNumber ?? Infinity) - (b.settings.fixtureNumber ?? Infinity) || byName(a, b),
};
const norm = (v) =>
  String(v ?? '')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase();

export function mountSceneList({ store, viewport, actions }) {
  const root = document.getElementById('scene-list');
  const count = document.getElementById('fixture-count');
  const swatches = new Map();
  let sort = (() => {
    try {
      const v = localStorage.getItem(SORT_KEY);
      return SORTS.some((x) => x.id === v) ? v : 'scene';
    } catch {
      return 'scene';
    }
  })();
  let shownIds = []; // projecteurs affichés, dans l'ordre (Maj+clic : plage)

  // ---- recherche et tri, au-dessus de la liste
  const search = h('input', {
    type: 'search',
    class: 'search sl-search',
    placeholder: 'Rechercher dans la scène…',
    spellcheck: 'false',
    oninput: () => render(),
    onkeydown: (e) => {
      e.stopPropagation(); // pas de raccourcis pendant la saisie
      if (e.key === 'Escape') {
        search.value = '';
        render();
        search.blur();
      }
    },
  });
  const sortBtn = h('button', {
    class: 'ib sl-sort',
    title: 'Trier les projecteurs',
    onclick: (e) => {
      const r = e.currentTarget.getBoundingClientRect();
      openContextMenu(r.left, r.bottom + 4, [
        { header: 'Trier les projecteurs' },
        ...SORTS.map((x) => ({
          label: x.label,
          checked: x.id === sort,
          onClick: () => {
            sort = x.id;
            try {
              localStorage.setItem(SORT_KEY, sort);
            } catch {
              /* stockage indisponible : choix gardé pour cette session */
            }
            render();
          },
        })),
      ]);
    },
  });
  root.before(h('div', { class: 'sl-tools' }, search, sortBtn));
  const query = () => norm(search.value.trim());
  let renaming = null; // { id, value } : groupe en cours de renommage
  let queued = false;
  let objectsFolded = (() => {
    try {
      return localStorage.getItem(OBJ_FOLD_KEY) === '1';
    } catch {
      return false;
    }
  })();

  /** Redessine au plus une fois par image (les opérations groupées émettent beaucoup d'événements). */
  const schedule = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      render();
    });
  };

  const additive = (e) => e.ctrlKey || e.metaKey;

  function groupRow(g) {
    const members = g.ids.filter((id) => store.fixtures.has(id));
    const nSel = members.filter((id) => store.isSelected(id)).length;
    const state = members.length && nSel === members.length ? 'selected' : nSel ? 'partial' : '';
    if (renaming?.id === g.id) {
      const input = h('input', {
        class: 'sl-rename',
        value: renaming.value,
        oninput: (e) => (renaming.value = e.target.value),
        onkeydown: (e) => {
          e.stopPropagation();
          if (e.key === 'Enter') commitRename();
          else if (e.key === 'Escape') {
            renaming = null;
            render();
          }
        },
        onblur: (e) => {
          if (e.target.isConnected) commitRename(); // pas quand la liste est redessinée
        },
      });
      requestAnimationFrame(() => {
        if (!input.isConnected) return;
        input.focus();
        if (!renaming?.started) {
          input.select();
          renaming.started = true;
        }
      });
      return h('div', { class: `sl-group ${state}` }, h('span', { class: 'gi' }, '▦'), input, h('span', { class: 'gc' }, String(members.length)));
    }
    return h(
      'div',
      {
        class: `sl-group ${state}`,
        title: 'Clic : sélectionner le groupe · Ctrl+clic : l’ajouter à la sélection · double-clic : renommer · clic droit : actions',
        onclick: (e) => actions.selectGroup(g.id, additive(e) || e.shiftKey),
        ondblclick: () => startRename(g.id),
        oncontextmenu: (e) => {
          e.preventDefault();
          e.stopPropagation();
          openContextMenu(e.clientX, e.clientY, actions.groupMenu(g.id));
        },
      },
      h('span', { class: 'gi' }, '▦'),
      h('span', { class: 'gn' }, g.name),
      h('span', { class: 'gc', title: `${members.length} projecteur(s)` }, members.length ? String(members.length) : 'vide'),
    );
  }

  function fixtureRow(f, list, groups) {
    const sw = h('span', { class: 'sw', title: consoleHeld(f) ? 'Piloté par la console ARKLIGHT' : null });
    swatches.set(f.id, sw);
    const issues = patchIssues(f, list);
    const tags = groups.filter((g) => g.ids.includes(f.id)).map((g) => g.name);
    const multi = store.selection.size > 1;
    return h(
      'div',
      {
        class: `sl-item${store.isSelected(f.id) ? ' selected' : ''}${multi && f.id === store.selectedId ? ' primary' : ''}${f.state?.power === false ? ' off' : ''}${consoleHeld(f) ? ' console' : ''}`,
        'data-id': f.id,
        onclick: (e) => {
          if (e.shiftKey) store.selectRange(f.id, shownIds);
          else if (additive(e)) store.toggleSelect(f.id);
          else store.select(f.id);
        },
        ondblclick: (e) => {
          if (!e.shiftKey && !additive(e)) viewport.frameSelected();
        },
        oncontextmenu: (e) => {
          e.preventDefault();
          e.stopPropagation();
          if (!store.isSelected(f.id)) store.select(f.id);
          openContextMenu(e.clientX, e.clientY, actions.selectionMenu());
        },
      },
      sw,
      h('span', { class: 'nm' }, f.name, tags.length ? h('span', { class: 'tags', title: `Groupes : ${tags.join(', ')}` }, tags.join(' · ')) : null),
      h('span', { class: `patch ${issues.length ? 'warn' : ''}`, title: issues.join('\n') }, issues.length ? '⚠ ' : '', patchLabel(f)),
    );
  }

  function objectRow(o) {
    const type = getObjectType(o.type);
    if (!type) return null;
    const icon = isProp(type) ? PROP_ICON[type.cat] : GRIP_ICON[type.family];
    return h(
      'div',
      {
        class: `sl-obj${o.id === store.selectedObjectId ? ' selected' : ''}`,
        title: 'Clic : sélectionner · double-clic : cadrer · clic droit : actions',
        onclick: () => store.selectObject(o.id),
        ondblclick: () => viewport.frameObject(o.id),
        oncontextmenu: (e) => {
          e.preventDefault();
          e.stopPropagation();
          store.selectObject(o.id);
          openContextMenu(e.clientX, e.clientY, actions.objectMenu(o.id));
        },
      },
      h('span', { class: 'oi', html: icon || '' }),
      h('span', { class: 'nm' }, o.name),
      h('span', { class: 'ok' }, isProp(type) ? type.short : 'grip'),
    );
  }

  /** Le projecteur correspond-il à la recherche (nom, modèle, patch, Fix, groupes) ? */
  function matches(f, groups, q) {
    const type = getFixtureType(f.type);
    const tags = groups.filter((g) => g.ids.includes(f.id)).map((g) => g.name);
    return norm([f.name, type?.brand, type?.name, type?.model, patchLabel(f), `u${f.settings.universe}`, f.settings.fixtureNumber ? `fix ${f.settings.fixtureNumber}` : '', ...tags].join(' ')).includes(q);
  }

  /** Projecteurs affichés : filtrés par la recherche, dans l'ordre du tri choisi. */
  function shownList() {
    const all = store.scene?.fixtures || [];
    const q = query();
    const groups = store.scene?.groups || [];
    return (q ? all.filter((f) => matches(f, groups, q)) : [...all]).sort(COMPARE[sort] || (() => 0));
  }

  function render() {
    const keepScroll = root.scrollTop;
    root.replaceChildren();
    swatches.clear();
    const all = store.scene?.fixtures || [];
    const q = query();
    const groups = (store.scene?.groups || []).filter((g) => !q || norm(g.name).includes(q));
    const allGroups = store.scene?.groups || [];
    const list = shownList();
    shownIds = list.map((f) => f.id);
    const nSel = store.selection.size;
    count.textContent = all.length ? `(${all.length}${nSel > 1 ? ` · ${t('{0} sélectionnés', { 0: nSel })}` : ''})` : '';
    sortBtn.replaceChildren(icon('sort', 15), (SORTS.find((x) => x.id === sort) || SORTS[0]).short);
    if (groups.length) {
      root.append(h('div', { class: 'sl-section' }, 'Groupes'));
      for (const g of groups) root.append(groupRow(g));
      root.append(h('div', { class: 'sl-section' }, 'Projecteurs'));
    }
    if (!all.length) {
      root.append(h('div', { class: 'empty' }, 'Glisse un projecteur de la bibliothèque dans la vue 3D, ou clique sur son « + ».'));
    } else if (!list.length) root.append(h('div', { class: 'empty' }, 'Aucun projecteur ne correspond à la recherche.'));
    else for (const f of list) root.append(fixtureRow(f, all, allGroups));
    const objects = (store.scene?.objects || []).filter((o) => {
      if (!q) return true;
      const type = getObjectType(o.type);
      return norm(`${o.name} ${type?.name || ''} ${type?.short || ''}`).includes(q);
    });
    if (objects.length) {
      root.append(
        h(
          'div',
          {
            class: 'sl-section fold',
            title: 'Replier / déplier',
            onclick: () => {
              objectsFolded = !objectsFolded;
              try {
                localStorage.setItem(OBJ_FOLD_KEY, objectsFolded ? '1' : '0');
              } catch {
                /* stockage indisponible */
              }
              render();
            },
          },
          `${objectsFolded ? '▸' : '▾'} Objets et décor (${objects.length})`,
        ),
      );
      if (!objectsFolded || q) for (const o of objects) root.append(objectRow(o));
    }
    root.scrollTop = keepScroll;
  }

  function startRename(gid) {
    const g = store.group(gid);
    if (!g) return;
    renaming = { id: gid, value: g.name };
    render();
  }

  function commitRename() {
    if (!renaming) return;
    const { id, value } = renaming;
    renaming = null;
    const g = store.group(id);
    if (g && value.trim() && value.trim() !== g.name) {
      g.name = value.trim();
      actions.renameGroup(id, value.trim());
    }
    render();
  }

  // Clic droit dans le vide de la liste : menu général (tout sélectionner…)
  root.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    openContextMenu(e.clientX, e.clientY, actions.selectionMenu());
  });
  // Clic dans le vide : désélectionner
  root.addEventListener('click', (e) => {
    if (e.target === root) store.select(null);
  });

  for (const ev of ['scene', 'groups', 'fixture-added', 'fixture-removed', 'selection', 'object-added', 'object-removed', 'object-selection']) store.addEventListener(ev, schedule);
  store.addEventListener('object-updated', (e) => {
    if (e.detail.origin === store.clientId && viewport.draggingObjectId) return; // déplacement en cours
    schedule();
  });
  store.addEventListener('fixture-updated', (e) => {
    // Les déplacements ne changent pas la liste : on évite de tout redessiner pendant un glisser.
    if (e.detail.origin === store.clientId && viewport.dragging) return;
    schedule();
  });
  // Nouveau groupe créé depuis cette interface : on propose tout de suite de le nommer
  store.addEventListener('group-created', (e) => startRename(e.detail));
  document.addEventListener('ark:rename-group', (e) => startRename(e.detail));

  // Aperçu de la couleur émise
  setInterval(() => {
    for (const [id, sw] of swatches) {
      const o = viewport.outputOf(id);
      if (!o) continue;
      const n = o.type.pixels;
      if (!n) continue;
      let r = 0;
      let g = 0;
      let b = 0;
      for (let p = 0; p < n; p++) {
        r += o.out[p * 3];
        g += o.out[p * 3 + 1];
        b += o.out[p * 3 + 2];
      }
      sw.style.background = rgbToCss([r / n, g / n, b / n]);
    }
  }, 200);

  render();
  return {
    /** Projecteurs dans l'ordre affiché (tri et recherche de la liste). */
    order: () => shownList().map((f) => f.id), // calculé à la demande (même avant que la liste soit redessinée)
  };
}
