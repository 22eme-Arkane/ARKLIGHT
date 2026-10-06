// Moniteur DMX : les 512 canaux d'un univers, avec le patch (qui occupe quel canal), les conflits
// et le ré-adressage automatique.
import { getFixtureType, getMode } from '/shared/fixtures/index.js';
import { compileMode } from '/shared/engine/dmx.js';
import { findPatchConflicts, rangeOf } from '/shared/engine/addressing.js';
import { h } from './dom.js';
import { channelLabel } from './test-console.js';

/** Couleur stable par projecteur (repère visuel sur la grille). */
function colorOf(id) {
  let x = 0;
  for (let i = 0; i < id.length; i++) x = (x * 31 + id.charCodeAt(i)) >>> 0;
  return `hsl(${x % 360} 65% 55%)`;
}

export function mountDmxMonitor({ store, isVisible, actions }) {
  const body = document.querySelector('[data-body="dmx"]');
  const select = h('select', { onchange: () => paint(true) });
  const sources = h('span', { class: 'note' });
  const conflictBtn = h('button', { class: 'conflict-pill hidden', title: 'Corriger automatiquement les chevauchements', onclick: () => actions.openReaddress({ scope: 'conflicts' }) });
  const grid = h('div', { class: 'dmx-grid' });
  const cells = [];
  for (let i = 0; i < 512; i++) {
    const bar = h('div', { class: 'bar' });
    const own = h('div', { class: 'own' });
    const v = h('span', { class: 'v' });
    const cell = h('div', { class: 'dmx-cell', title: `Canal ${i + 1}` }, bar, own, h('span', { class: 'n' }, String(i + 1)), v);
    cells.push({ cell, bar, own, v, last: -1, owners: null });
    grid.append(cell);
  }
  body.append(
    h(
      'div',
      { class: 'dmx-head' },
      h('span', {}, 'Univers'),
      select,
      sources,
      h('span', { class: 'spacer' }),
      conflictBtn,
      h(
        'button',
        {
          title: 'Ré-adresser automatiquement : tout le plan, la sélection, un groupe, un univers ou seulement les conflits',
          onclick: () => actions.openReaddress({}),
        },
        'Ré-adressage auto…',
      ),
    ),
    grid,
  );

  let userPicked = false;
  select.addEventListener('change', () => (userPicked = true));

  // Clic sur un canal : sélectionne le projecteur qui l'occupe (Ctrl : ajoute à la sélection)
  grid.addEventListener('click', (e) => {
    const cell = e.target.closest('.dmx-cell');
    if (!cell) return;
    const owners = cells[[...grid.children].indexOf(cell)]?.owners;
    if (!owners?.length) return;
    const id = owners[owners.length - 1].id;
    if (e.ctrlKey || e.metaKey) store.toggleSelect(id);
    else store.select(id);
  });

  let uniKey = '';
  function refreshUniverses() {
    const set = new Set([...store.dmx.keys(), ...(store.scene?.fixtures || []).map((f) => f.settings.universe)]);
    if (!set.size) set.add(1);
    const list = [...set].sort((a, b) => a - b);
    const cur = Number(select.value) || null;
    const sel = store.selected();
    const want = !userPicked && sel ? sel.settings.universe : cur ?? list[0];
    const key = `${list.map((u) => `${u}${store.liveUniverses.has(u) ? '*' : ''}`).join(',')}|${want}`;
    if (key === uniKey) return; // rien n'a changé : on ne reconstruit pas la liste (elle peut être ouverte)
    uniKey = key;
    select.replaceChildren(...list.map((u) => h('option', { value: u, selected: u === want }, `U${u}${store.liveUniverses.has(u) ? ' ●' : ''}`)));
  }

  // ---- carte du patch de l'univers affiché
  let patchU = null;
  let patchDirty = true;
  function buildPatch(u) {
    const fixtures = store.scene?.fixtures || [];
    const conflicts = findPatchConflicts(fixtures);
    const inU = conflicts.ids.size ? [...conflicts.ids].filter((id) => store.fixtures.get(id)?.settings.universe === u).length : 0;
    conflictBtn.classList.toggle('hidden', !conflicts.ids.size);
    conflictBtn.textContent = `⚠ ${conflicts.ids.size} projecteur${conflicts.ids.size > 1 ? 's' : ''} en conflit${conflicts.ids.size && inU < conflicts.ids.size ? ` (${inU} dans U${u})` : ''} — corriger…`;
    for (const c of cells) c.owners = null;
    for (const f of fixtures) {
      const r = rangeOf(f);
      if (!r.footprint || r.universe !== u) continue;
      const type = getFixtureType(f.type);
      const mode = getMode(type, f.settings.dmxMode);
      const slots = mode ? compileMode(mode).slots : [];
      for (let ch = r.start; ch <= Math.min(512, r.end); ch++) {
        const k = ch - r.start;
        const slot = slots.find((s) => k >= s.offset && k < s.offset + s.bytes);
        const label = slot ? `${channelLabel(slot)}${slot.bytes === 2 ? (k === slot.offset ? ' (gros)' : ' (fin)') : ''}` : '';
        const c = cells[ch - 1];
        (c.owners ||= []).push({ id: f.id, name: f.name, k: k + 1, fp: r.footprint, label, start: ch === r.start });
      }
    }
    for (let i = 0; i < 512; i++) {
      const c = cells[i];
      const o = c.owners;
      const conflict = !!o && o.length > 1;
      c.cell.classList.toggle('used', !!o);
      c.cell.classList.toggle('conflict', conflict);
      c.cell.classList.toggle('start', !!o?.some((x) => x.start));
      c.own.style.background = o ? (conflict ? '' : colorOf(o[0].id)) : '';
      c.cell.title = o
        ? `Canal ${i + 1}\n${o.map((x) => `${x.name} — canal ${x.k}/${x.fp}${x.label ? ` : ${x.label}` : ''}`).join('\n')}${conflict ? '\n⚠ chevauchement' : ''}`
        : `Canal ${i + 1} — libre`;
    }
    patchU = u;
    patchDirty = false;
  }

  let queued = false;
  function paint(force = false) {
    if (!isVisible()) return;
    const u = Number(select.value) || 1;
    if (patchDirty || patchU !== u) buildPatch(u);
    const data = store.dmx.get(u);
    const mine = new Set(store.selectedIds().filter((id) => store.fixtures.get(id)?.settings.universe === u));
    for (let i = 0; i < 512; i++) {
      const c = cells[i];
      const val = data ? data[i] : 0;
      const isMine = !!c.owners?.some((x) => mine.has(x.id));
      if (force || c.last !== val) {
        c.bar.style.height = `${(val / 255) * 100}%`;
        c.v.textContent = val ? String(val) : '';
        c.last = val;
      }
      if (c.cell.classList.contains('mine') !== isMine) c.cell.classList.toggle('mine', isMine);
    }
    const src = (store.status?.sources || []).filter((s) => s.universes.includes(u));
    sources.textContent = src.length ? `Sources : ${src.map((s) => `${s.name || s.address} (${s.protocol})`).join(', ')}` : 'Aucune source active';
  }

  const schedulePaint = (force) => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      paint(force);
    });
  };
  const patchChanged = () => {
    patchDirty = true;
    schedulePaint(true);
  };

  store.addEventListener('dmx', (e) => {
    if (Number(select.value) === e.detail) schedulePaint(false);
  });
  let lastSelected = null;
  store.addEventListener('selection', () => {
    // L'événement est aussi émis à chaque modification du projecteur sélectionné : on ne
    // repart sur son univers que lors d'un vrai changement de sélection.
    if (store.selectedId !== lastSelected) {
      lastSelected = store.selectedId;
      userPicked = false;
      refreshUniverses();
    }
    patchChanged();
  });
  store.addEventListener('status', () => {
    refreshUniverses();
    schedulePaint(false);
  });
  for (const ev of ['scene', 'fixture-added', 'fixture-removed', 'fixture-updated']) {
    store.addEventListener(ev, () => {
      refreshUniverses();
      patchChanged();
    });
  }
  refreshUniverses();
  return { paint: () => paint(true) };
}
