// Fenêtre « Ré-adressage automatique » : corrige les chevauchements, ou ré-adresse tout le plan de feu,
// la sélection, un groupe ou un univers. Le résultat est affiché avant d'être appliqué.
import { ADDRESS_ORDERS, findPatchConflicts, planAddressing } from '/shared/engine/addressing.js';
import { h, pad3, toast } from './dom.js';

const ALIGNS = [
  [0, 'À la suite (aucun trou)'],
  [5, '1, 6, 11, 16…'],
  [10, '1, 11, 21, 31…'],
  [16, '1, 17, 33, 49… (blocs de 16 : petites consoles)'],
  [20, '1, 21, 41, 61…'],
  [32, '1, 33, 65, 97… (blocs de 32 : DMX Operator 384)'],
  [50, '1, 51, 101…'],
  [100, '1, 101, 201…'],
];

const fmt = (u, start, fp) => `U${u} · ${pad3(start)}${fp > 1 ? `–${pad3(start + fp - 1)}` : ''}`;

export function createReaddressDialog({ store, actions }) {
  const opts = { scope: 'conflicts', order: 'scene', universe: 1, address: 1, universes: 'keep', align: 0, overflow: true };
  let plan = null;

  const dlg = h('dialog', { class: 'dialog readdress' });
  document.body.append(dlg);

  const set = (k, v) => {
    opts[k] = v;
    update();
  };
  const scopeSel = h('select', {
    onchange: (e) => {
      opts.scope = e.target.value;
      applyScopeDefaults();
      update();
    },
  });
  const orderSel = h('select', { onchange: (e) => set('order', e.target.value) }, Object.entries(ADDRESS_ORDERS).map(([v, l]) => h('option', { value: v }, l)));
  const uniIn = h('input', { type: 'number', min: 1, max: 63999, oninput: (e) => set('universe', Number(e.target.value) || 1) });
  const addrIn = h('input', { type: 'number', min: 1, max: 512, oninput: (e) => set('address', Number(e.target.value) || 1) });
  const uniModeSel = h(
    'select',
    { onchange: (e) => set('universes', e.target.value) },
    h('option', { value: 'keep' }, 'Chaque projecteur garde son univers'),
    h('option', { value: 'pack' }, 'Tout à la suite, à partir de l’univers de départ'),
  );
  const alignSel = h('select', { onchange: (e) => set('align', Number(e.target.value)) }, ALIGNS.map(([v, l]) => h('option', { value: v }, l)));
  const overflowCb = h('input', { type: 'checkbox', onchange: (e) => set('overflow', e.target.checked) });

  const row = (label, input, hint) => h('div', { class: 'row' }, h('span', {}, label), h('div', {}, input, hint ? h('div', { class: 'note' }, hint) : null));
  const rows = {
    scope: row('Projecteurs', scopeSel),
    order: row('Ordre', orderSel),
    uniMode: row('Univers', uniModeSel),
    start: row('Départ', h('div', { class: 'ra-start' }, h('span', {}, 'univers'), uniIn, h('span', {}, 'adresse'), addrIn)),
    align: row('Adresses', alignSel),
    overflow: row('Univers plein', h('label', { class: 'ra-check' }, overflowCb, ' passer à l’univers suivant')),
  };
  const summary = h('div', { class: 'ra-summary' });
  const table = h('div', { class: 'ra-table' });
  const applyBtn = h('button', { class: 'primary', onclick: apply }, 'Appliquer');
  dlg.append(
    h('h3', {}, 'Ré-adressage automatique'),
    h('div', { class: 'note' }, 'Calcule un nouveau patch sans chevauchement. Rien n’est modifié avant « Appliquer ». Pense à reporter les nouvelles adresses dans l’appli ou la console.'),
    h('div', { class: 'ra-form' }, Object.values(rows)),
    summary,
    table,
    h('div', { class: 'dialog-actions' }, h('button', { onclick: () => dlg.close() }, 'Annuler'), applyBtn),
  );

  /** Projecteurs visés par une portée « ids » (sélection, groupe, univers). */
  function scopeIds(scope) {
    if (scope === 'selection') return store.selectedIds();
    if (scope.startsWith('group:')) {
      const g = store.group(scope.slice(6));
      return g ? store.scene.fixtures.filter((f) => g.ids.includes(f.id)).map((f) => f.id) : [];
    }
    if (scope.startsWith('universe:')) {
      const u = Number(scope.slice(9));
      return store.scene.fixtures.filter((f) => f.settings.universe === u).map((f) => f.id);
    }
    return null;
  }

  /** Départ proposé : là où commence déjà le premier projecteur concerné. */
  function applyScopeDefaults() {
    const ids = scopeIds(opts.scope);
    if (!ids) {
      Object.assign(opts, { universe: 1, address: 1, universes: 'keep' });
      return;
    }
    const list = ids.map((id) => store.fixtures.get(id)).filter(Boolean);
    const u = opts.scope.startsWith('universe:') ? Number(opts.scope.slice(9)) : list[0]?.settings.universe ?? 1;
    const inU = list.filter((f) => f.settings.universe === u).map((f) => f.settings.dmxAddress);
    Object.assign(opts, { universe: u, address: inU.length ? Math.min(...inU) : 1, universes: 'pack' });
  }

  function fillScopes() {
    const fixtures = store.scene.fixtures;
    const conflicts = findPatchConflicts(fixtures).ids.size;
    const sel = store.selectedIds().length;
    const universes = [...new Set(fixtures.map((f) => f.settings.universe))].sort((a, b) => a - b);
    const options = [
      ['conflicts', `Corriger seulement les conflits (${conflicts} projecteur${conflicts > 1 ? 's' : ''} en conflit)`],
      ['all', `Tous les projecteurs (${fixtures.length})`],
      ['selection', `La sélection (${sel})`, !sel],
      ...store.scene.groups.map((g) => [`group:${g.id}`, `Groupe « ${g.name} » (${g.ids.length})`, !g.ids.length]),
      ...(universes.length > 1 ? universes.map((u) => [`universe:${u}`, `Univers ${u} (${fixtures.filter((f) => f.settings.universe === u).length})`]) : []),
    ];
    scopeSel.replaceChildren(...options.map(([v, l, disabled]) => h('option', { value: v, disabled, selected: v === opts.scope }, l)));
  }

  function update() {
    const scope = opts.scope;
    const ids = scopeIds(scope);
    const conflictsOnly = scope === 'conflicts';
    rows.order.hidden = conflictsOnly;
    rows.uniMode.hidden = conflictsOnly || scope.startsWith('universe:');
    rows.start.hidden = conflictsOnly;
    uniIn.disabled = opts.universes === 'keep';
    uniIn.title = uniIn.disabled ? 'Chaque projecteur reste dans son univers' : '';
    orderSel.value = opts.order;
    uniModeSel.value = opts.universes;
    alignSel.value = String(opts.align);
    overflowCb.checked = opts.overflow;
    if (document.activeElement !== uniIn) uniIn.value = opts.universe;
    if (document.activeElement !== addrIn) addrIn.value = opts.address;

    plan = planAddressing(store.scene.fixtures, { ...opts, scope: ids ? 'ids' : scope, ids: ids || undefined });
    const n = plan.moves.length;
    const lines = [];
    if (conflictsOnly && !plan.conflictsBefore) lines.push(h('div', { class: 'ok' }, '✓ Aucun chevauchement ni dépassement : rien à corriger.'));
    else {
      lines.push(h('div', {}, n ? `${n} projecteur${n > 1 ? 's' : ''} à ré-adresser sur ${plan.considered}.` : 'Rien à changer : les adresses correspondent déjà.'));
      lines.push(
        h(
          'div',
          { class: plan.conflictsAfter ? 'warnbox' : 'ok' },
          `Projecteurs en conflit : ${plan.conflictsBefore} → ${plan.conflictsAfter}${plan.conflictsAfter && !conflictsOnly ? ' (conflits avec des projecteurs hors de la portée choisie)' : ''}`,
        ),
      );
    }
    if (plan.unplaced.length)
      lines.push(h('div', { class: 'warnbox' }, `⚠ ${plan.unplaced.length} projecteur(s) sans place libre${opts.overflow ? '' : ' (coche « passer à l’univers suivant »)'} : ils gardent leur adresse.`));
    summary.replaceChildren(...lines);

    table.replaceChildren(
      ...(n
        ? [
            h('div', { class: 'ra-row head' }, h('span', {}, 'Projecteur'), h('span', {}, 'Avant'), h('span', {}, ''), h('span', {}, 'Après')),
            ...plan.moves.slice(0, 300).map((m) =>
              h(
                'div',
                { class: 'ra-row' },
                h('span', { class: 'nm', title: m.name }, m.name),
                h('span', { class: 'mono from' }, fmt(m.from.universe, m.from.address, m.footprint)),
                h('span', { class: 'arrow' }, '→'),
                h('span', { class: 'mono to' }, fmt(m.to.universe, m.to.address, m.footprint)),
              ),
            ),
          ]
        : []),
    );
    applyBtn.disabled = !n;
    applyBtn.textContent = n ? `Appliquer (${n})` : 'Appliquer';
  }

  function apply() {
    if (!plan?.moves.length) return;
    actions.applyMoves(plan.moves);
    toast(`${plan.moves.length} projecteur${plan.moves.length > 1 ? 's' : ''} ré-adressé${plan.moves.length > 1 ? 's' : ''}`);
    dlg.close();
  }

  return {
    /** preset.scope : 'conflicts' | 'all' | 'selection' | 'group:<id>' | 'universe:<n>' */
    open(preset = {}) {
      const hasConflicts = findPatchConflicts(store.scene.fixtures).ids.size > 0;
      opts.scope = preset.scope || (hasConflicts ? 'conflicts' : 'all');
      if (opts.scope === 'selection' && !store.selectedIds().length) opts.scope = 'all';
      applyScopeDefaults();
      fillScopes();
      update();
      if (!dlg.open) dlg.showModal();
    },
  };
}
