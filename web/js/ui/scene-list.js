// Panneau "Scène" : groupes et projecteurs placés, avec aperçu de couleur, patch et sélection multiple.
import { rgbToCss } from '/shared/engine/color.js';
import { h } from './dom.js';
import { patchIssues, patchLabel } from './patch.js';
import { openContextMenu } from './context-menu.js';

export function mountSceneList({ store, viewport, actions }) {
  const root = document.getElementById('scene-list');
  const count = document.getElementById('fixture-count');
  const swatches = new Map();
  let renaming = null; // { id, value } : groupe en cours de renommage
  let queued = false;

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
    const sw = h('span', { class: 'sw' });
    swatches.set(f.id, sw);
    const issues = patchIssues(f, list);
    const tags = groups.filter((g) => g.ids.includes(f.id)).map((g) => g.name);
    const multi = store.selection.size > 1;
    return h(
      'div',
      {
        class: `sl-item${store.isSelected(f.id) ? ' selected' : ''}${multi && f.id === store.selectedId ? ' primary' : ''}${f.state?.power === false ? ' off' : ''}`,
        'data-id': f.id,
        onclick: (e) => {
          if (e.shiftKey) store.selectRange(f.id);
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

  function render() {
    const keepScroll = root.scrollTop;
    root.replaceChildren();
    swatches.clear();
    const list = store.scene?.fixtures || [];
    const groups = store.scene?.groups || [];
    const nSel = store.selection.size;
    count.textContent = list.length ? `(${list.length}${nSel > 1 ? ` · ${nSel} sélectionnés` : ''})` : '';
    if (groups.length) {
      root.append(h('div', { class: 'sl-section' }, 'Groupes'));
      for (const g of groups) root.append(groupRow(g));
      root.append(h('div', { class: 'sl-section' }, 'Projecteurs'));
    }
    if (!list.length) {
      root.append(h('div', { class: 'empty' }, 'Glisse un projecteur de la bibliothèque dans la vue 3D, ou clique sur son « + ».'));
    } else for (const f of list) root.append(fixtureRow(f, list, groups));
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

  for (const ev of ['scene', 'groups', 'fixture-added', 'fixture-removed', 'selection']) store.addEventListener(ev, schedule);
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
}
