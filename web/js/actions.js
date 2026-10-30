// Actions sur la sélection et les groupes, partagées par le clavier, les menus contextuels et l'inspecteur.
import { getFixtureType } from '/shared/fixtures/index.js';
import { isAsteraV5 } from '/shared/engine/output.js';
import { planAddressing } from '/shared/engine/addressing.js';
import { toast } from './ui/dom.js';
import { askText } from './ui/dialogs.js';
import { createReaddressDialog } from './ui/readdress.js';
import { SUPPORT_CHOICES, SUPPORTS } from '/shared/fixtures/supports.js';
import { getObjectType, isProp } from '/shared/objects.js';
import { CLOTHS, clothName, gripStands } from '/shared/grip.js';
import { FILTER_GROUPS, FILTERS } from '/shared/filters.js';
import { GRIDS, accessoriesFor, getModifier } from '/shared/modifiers.js';

/** Sous-menus du nuancier (diffusions, CTB, CTO, ND, couleurs) ; ids : filtres autorisés. */
function filterMenu(current, onPick, ids = null) {
  const allowed = ids ? new Set(ids) : null;
  return FILTER_GROUPS.map((g) => {
    const list = FILTERS.filter((f) => f.group === g.id && (!allowed || allowed.has(f.id)));
    return list.length ? { label: g.label, items: list.map((f) => ({ label: f.name, checked: current === f.id, onClick: () => onPick(f.id) })) } : null;
  });
}

function deepAssign(target, patch) {
  for (const [k, v] of Object.entries(patch)) {
    if (v && typeof v === 'object' && !Array.isArray(v) && target[k] && typeof target[k] === 'object') deepAssign(target[k], v);
    else target[k] = v;
  }
}

const typeOf = (f) => getFixtureType(f.type);
/** Projecteurs qu'on oriente vers le personnage (pas les tubes ni les boîtiers). */
export const isAimable = (f) => !['tube', 'box-device'].includes(typeOf(f)?.geometry.kind);
const isV5 = (f) => isAsteraV5(typeOf(f));
const plural = (n, one, many) => (n > 1 ? many : one);

export function createActions({ store, net, viewport, onboard, keyOf = () => '' }) {
  const sel = () => store.selectedFixtures();
  let readdressDialog = null;

  /** Modifie plusieurs projecteurs : application locale immédiate, puis un seul envoi au serveur. */
  function updateMany(items) {
    items = items.filter((it) => store.fixtures.has(it.id));
    if (!items.length) return;
    for (const { id, changes } of items) {
      const f = store.fixtures.get(id);
      deepAssign(f, structuredClone(changes));
      store.upsertFixture(f, store.clientId);
    }
    net.send({ t: 'fixtures.update', items });
  }

  const setPower = (ids, on) => updateMany(ids.map((id) => ({ id, changes: { state: { power: on } } })));

  /** Liaison CRMX des projecteurs Astera (comme la touche LINK d'un émetteur). */
  function setCrmx(ids, linked) {
    const v5 = ids.filter((id) => isV5(store.fixtures.get(id)));
    updateMany(v5.map((id) => ({ id, changes: { settings: { crmxLinked: linked } } })));
    for (const id of v5) {
      const rt = onboard?.runtime(id);
      if (rt) {
        rt.latch = null;
        rt.justUnlinked = !linked;
      }
    }
  }

  function applyMoves(moves) {
    updateMany(moves.map((m) => ({ id: m.id, changes: { settings: { universe: m.to.universe, dmxAddress: m.to.address } } })));
  }

  const firstInScene = (ids) => store.scene.fixtures.find((f) => ids.includes(f.id))?.id ?? null;

  const actions = {
    updateMany,
    applyMoves,
    setPower,
    setCrmx,

    // ---- sélection
    selectAll: () => store.setSelection(store.scene.fixtures.map((f) => f.id), store.selectedId),
    clearSelection: () => store.select(null),
    invertSelection: () => store.setSelection(store.scene.fixtures.filter((f) => !store.isSelected(f.id)).map((f) => f.id)),
    selectSameType() {
      const types = new Set(sel().map((f) => f.type));
      store.setSelection(store.scene.fixtures.filter((f) => types.has(f.type)).map((f) => f.id), store.selectedId);
    },
    selectGroup(gid, additive = false) {
      const g = store.group(gid);
      if (!g) return;
      const ids = g.ids.filter((id) => store.fixtures.has(id));
      if (additive) store.setSelection([...store.selection, ...ids], store.selectedId ?? firstInScene(ids));
      else store.setSelection(ids, firstInScene(ids));
    },

    // ---- projecteurs sélectionnés
    deleteSelection() {
      const ids = store.selectedIds();
      if (!ids.length) return;
      net.send({ t: 'fixtures.remove', ids });
      if (ids.length > 1) toast(`${ids.length} projecteurs supprimés${keyOf('edit.undo') ? ` (${keyOf('edit.undo')} pour annuler)` : ''}`);
    },
    duplicateSelection() {
      const list = sel();
      if (!list.length) return;
      net.send({
        t: 'fixtures.add',
        select: true,
        items: list.map((f) => ({
          type: f.type,
          position: [f.position[0] + 0.3, f.position[1], f.position[2]],
          rotation: f.rotation,
          settings: { ...f.settings, dmxAddress: undefined, fixtureNumber: undefined }, // nouvelle adresse, nouveau numéro
        })),
      });
    },
    frame: () => viewport.frameSelected(),
    aim: (targetId = null) => viewport.aimSelectedAt(targetId),
    drop: () => viewport.dropSelectedToFloor(),

    /** Pied des projecteurs (automatique, pied de 1000, junior, wind-up…). */
    setSupport: (ids, support) => updateMany(ids.map((id) => ({ id, changes: { settings: { support } } }))),
    supportMenu(ids) {
      const list = ids.map((id) => store.fixtures.get(id)).filter(Boolean);
      const cur = new Set(list.map((f) => f.settings.support || 'auto'));
      const shown = list.length === 1 ? viewport.outputOf(list[0].id)?.support : null;
      const what = shown ? (shown.def ? shown.def.short : shown.id === 'hanging' ? 'accroché' : 'aucun') : null;
      return {
        label: what ? `Pied : ${what}${shown.auto ? ' (auto)' : ''}` : 'Pied',
        items: SUPPORT_CHOICES.map(([id, label]) => ({ label: SUPPORTS[id]?.name || label, checked: cur.size === 1 && cur.has(id), onClick: () => actions.setSupport(ids, id) })),
      };
    },

    /** Numéro de fixture d'un projecteur, ou numéros à la suite pour une sélection. */
    async askFixtureNumber(ids) {
      const list = store.scene.fixtures.filter((f) => ids.includes(f.id));
      if (!list.length) return;
      const next = Math.max(0, ...store.scene.fixtures.map((f) => f.settings.fixtureNumber || 0)) + 1;
      const many = list.length > 1;
      const v = await askText({
        title: many ? `Numéroter ${list.length} projecteurs à la suite` : `Numéro de fixture — ${list[0].name}`,
        label: many ? 'Premier numéro (Fix)' : 'Numéro (Fix)',
        value: String(many ? list[0].settings.fixtureNumber || next : list[0].settings.fixtureNumber || next),
        ok: 'Valider',
        note: 'Le numéro de fixture identifie le projecteur dans la console ou l’appli (Luminair, Blackout…). Sur les Astera, c’est le réglage FIXTURE NUMBER.',
      });
      if (v === null) return;
      const n = Math.round(Number(v));
      if (!Number.isFinite(n) || n < 1 || n > 9999) return toast('Numéro entre 1 et 9999', 'error');
      updateMany(list.map((f, i) => ({ id: f.id, changes: { settings: { fixtureNumber: Math.min(9999, n + i) } } })));
      const taken = store.scene.fixtures.filter((o) => !ids.includes(o.id) && o.settings.fixtureNumber >= n && o.settings.fixtureNumber < n + list.length);
      if (taken.length) toast(`Déjà utilisé : ${taken.map((o) => `Fix ${o.settings.fixtureNumber} (${o.name})`).join(', ')}`, 'error');
      else toast(many ? `Fix ${n} à ${n + list.length - 1}` : `${list[0].name} : Fix ${n}`);
    },

    /** Accessoires des projecteurs : boîte à lumière ou dôme, toile avant, louver, gélatine. */
    accessoryMenu(ids) {
      const list = ids.map((id) => store.fixtures.get(id)).filter(Boolean);
      if (!list.length) return null;
      const opts = list.map((f) => accessoriesFor(typeOf(f)));
      const mods = opts[0].modifiers.filter((m) => opts.every((o) => o.modifiers.some((x) => x.id === m.id)));
      const one = list.length === 1 ? list[0].settings : null;
      const set = (changes) => updateMany(list.map((f) => ({ id: f.id, changes: { settings: changes } })));
      const items = [];
      if (mods.length) {
        const groups = [...new Set(mods.map((m) => m.group))];
        items.push({
          label: 'Boîte à lumière / dôme',
          items: [{ label: 'Aucun', checked: one && !one.modifier, onClick: () => set({ modifier: null, front: null }) }, '-', ...groups.map((g) => ({ label: g, items: mods.filter((m) => m.group === g).map((m) => ({ label: m.name, checked: one?.modifier === m.id, onClick: () => set({ modifier: m.id }) })) }))],
        });
      }
      const mod = one && getModifier(one.modifier);
      if (mod && mod.kind !== 'lantern') {
        const diffusions = FILTERS.filter((f) => f.group === 'diffusion').map((f) => f.id);
        items.push({ label: 'Toile avant', items: [{ label: 'Toile d’origine', checked: !one.front, onClick: () => set({ front: null }) }, { label: 'Sans toile avant', checked: one.front === 'none', onClick: () => set({ front: 'none' }) }, '-', ...filterMenu(one.front, (id) => set({ front: id }), diffusions)] });
      }
      if (opts.every((o) => o.grids)) items.push({ label: 'Louver / grille', items: [{ label: 'Aucun', checked: one && !one.grid, onClick: () => set({ grid: null }) }, '-', ...GRIDS.map((g) => ({ label: g.name, checked: one?.grid === g.id, onClick: () => set({ grid: g.id }) }))] });
      if (opts.every((o) => o.gel)) items.push({ label: 'Gélatine', items: [{ label: 'Aucune', checked: one && !one.gel, onClick: () => set({ gel: null }) }, '-', ...filterMenu(one?.gel, (id) => set({ gel: id }))] });
      return items.length ? { label: 'Accessoires', items } : null;
    },

    /** Cibles de « Viser » : un personnage, un animal, un véhicule… */
    aimMenu() {
      const targets = viewport.aimTargets();
      if (targets.length <= 1) return { label: 'Viser le personnage', onClick: () => actions.aim() };
      return { label: 'Viser', items: targets.map((t) => ({ label: t.name, onClick: () => actions.aim(t.id) })) };
    },

    /** Adresse les projecteurs ids à la suite depuis (universe, address), en contournant les autres. */
    addressSequential(ids, universe, address) {
      const plan = planAddressing(store.scene.fixtures, { scope: 'ids', ids, order: 'scene', universes: 'pack', universe, address });
      applyMoves(plan.moves);
      const placed = plan.considered - plan.unplaced.length;
      if (plan.unplaced.length) toast(`${placed} projecteur(s) adressé(s), ${plan.unplaced.length} sans place libre`, 'error');
      else toast(plan.moves.length ? `${placed} projecteur(s) adressé(s) à la suite` : 'Les adresses étaient déjà à la suite');
      return plan;
    },
    openReaddress(preset) {
      readdressDialog ||= createReaddressDialog({ store, actions });
      readdressDialog.open(preset);
    },

    // ---- groupes
    groupSelection() {
      const ids = store.selectedIds();
      if (!ids.length) return toast('Sélectionne d’abord des projecteurs (Ctrl+clic pour en prendre plusieurs)', 'error');
      net.send({ t: 'group.create', ids });
    },
    addSelectionToGroup: (gid) => net.send({ t: 'group.update', id: gid, add: store.selectedIds() }),
    removeSelectionFromGroup: (gid) => net.send({ t: 'group.update', id: gid, remove: store.selectedIds() }),
    renameGroup: (gid, name) => net.send({ t: 'group.update', id: gid, name }),
    startRenameGroup: (gid) => document.dispatchEvent(new CustomEvent('ark:rename-group', { detail: gid })),
    deleteGroup: (gid) => net.send({ t: 'group.delete', id: gid }),

    // ---- grip
    deleteObject(id = store.selectedObjectId) {
      if (id) net.send({ t: 'object.remove', id });
    },
    duplicateObject(id = store.selectedObjectId) {
      const o = store.object(id);
      if (!o) return;
      net.send({ t: 'object.add', type: o.type, position: [o.position[0] + 0.5, o.position[1], o.position[2]], rotation: o.rotation, props: o.props, select: true });
    },
    updateObject(id, changes) {
      const o = store.object(id);
      if (!o) return;
      deepAssign(o, structuredClone(changes));
      store.upsertObject(o, store.clientId);
      net.send({ t: 'object.update', id, changes });
    },
    objectMenu(id) {
      const o = store.object(id);
      if (!o) return [];
      const type = getObjectType(o.type);
      const pose = isProp(type) && type.params?.find((p) => p.key === 'pose');
      const cloths = !isProp(type) && type.cloths;
      const setCloth = (c) => actions.updateObject(id, { props: { cloth: c } });
      const st = !isProp(type) && type.standChoices && gripStands(type, o.props);
      return [
        { header: o.name },
        cloths
          ? {
              label: `Toile : ${clothName(o.props.cloth).split(' — ')[0]}`,
              items: [...cloths.filter((c) => CLOTHS[c]).map((c) => ({ label: clothName(c), checked: o.props.cloth === c, onClick: () => setCloth(c) })), '-', ...filterMenu(o.props.cloth, setCloth, cloths)],
            }
          : null,
        st
          ? {
              label: `Pieds : ${st.count} × ${SUPPORTS[st.id]?.short || st.id}`,
              items: type.standChoices.map((sid) => ({
                label: `${SUPPORTS[sid]?.name || sid}${sid === type.stands.id ? ' (conseillé)' : ''}`,
                checked: sid === st.id,
                onClick: () => actions.updateObject(id, { props: { stand: sid === type.stands.id ? null : sid } }),
              })),
            }
          : null,
        type.model === 'camera' || type.model === 'dolly' ? { label: 'Voir par cette caméra', shortcut: keyOf('view.camera'), onClick: () => viewport.lookThrough(id) } : null,
        { label: 'Cadrer', shortcut: keyOf('view.frame'), onClick: () => viewport.frameObject(id) },
        { label: 'Dupliquer', shortcut: keyOf('edit.duplicate'), onClick: () => actions.duplicateObject(id) },
        { label: 'Poser au sol', onClick: () => actions.updateObject(id, { position: [o.position[0], 0, o.position[2]] }) },
        { label: 'Tourner de 90°', onClick: () => actions.updateObject(id, { rotation: [o.rotation[0], Math.round((o.rotation[1] + Math.PI / 2) * 1000) / 1000, o.rotation[2]] }) },
        pose ? { label: 'Pose', items: pose.options.map(([v, l]) => ({ label: l, checked: (o.props.pose || pose.default) === v, onClick: () => actions.updateObject(id, { props: { pose: v } }) })) } : null,
        '-',
        { label: 'Supprimer', shortcut: keyOf('edit.delete'), danger: true, onClick: () => actions.deleteObject(id) },
      ];
    },

    // ---- menus contextuels
    /** Menu de la sélection (clic droit sur un projecteur, ou dans le vide). */
    selectionMenu() {
      const list = sel();
      const n = list.length;
      const ids = list.map((f) => f.id);
      const groups = store.scene.groups;
      const all = store.scene.fixtures.length;
      const items = [];
      if (n) {
        const types = new Set(list.map((f) => f.type));
        const anyOff = list.some((f) => !f.state.power);
        const v5 = list.filter(isV5);
        items.push(
          { header: n === 1 ? list[0].name : `${n} projecteurs sélectionnés` },
          { label: 'Cadrer', shortcut: keyOf('view.frame'), onClick: actions.frame },
          { label: n > 1 ? 'Dupliquer la sélection' : 'Dupliquer', shortcut: keyOf('edit.duplicate'), onClick: actions.duplicateSelection },
          { label: anyOff ? 'Allumer' : 'Éteindre', onClick: () => setPower(ids, anyOff) },
          list.some(isAimable) ? actions.aimMenu() : null,
          { label: 'Poser au sol', shortcut: keyOf('fixture.drop'), onClick: actions.drop },
          actions.supportMenu(ids),
          actions.accessoryMenu(ids),
          v5.length
            ? {
                label: 'Liaison CRMX',
                items: [
                  { label: `Lier ${plural(v5.length, 'le projecteur', `les ${v5.length} projecteurs`)} (touche LINK)`, onClick: () => setCrmx(ids, true) },
                  { label: 'Délier', onClick: () => setCrmx(ids, false) },
                ],
              }
            : null,
          '-',
          { label: n > 1 ? 'Grouper la sélection' : 'Créer un groupe', shortcut: keyOf('group.create'), onClick: actions.groupSelection },
          groups.length
            ? {
                label: 'Ajouter au groupe',
                items: groups.map((g) => ({ label: g.name, checked: ids.every((id) => g.ids.includes(id)), onClick: () => actions.addSelectionToGroup(g.id) })),
              }
            : null,
          groups.some((g) => ids.some((id) => g.ids.includes(id)))
            ? {
                label: 'Retirer du groupe',
                items: groups.filter((g) => ids.some((id) => g.ids.includes(id))).map((g) => ({ label: g.name, onClick: () => actions.removeSelectionFromGroup(g.id) })),
              }
            : null,
          '-',
          { label: n > 1 ? 'Adresser la sélection à la suite…' : 'Ré-adresser…', onClick: () => actions.openReaddress({ scope: 'selection' }) },
          { label: n > 1 ? 'Numéroter les fixtures à la suite…' : `Numéro de fixture${list[0].settings.fixtureNumber ? ` (Fix ${list[0].settings.fixtureNumber})` : ''}…`, onClick: () => actions.askFixtureNumber(ids) },
          '-',
          types.size === 1 ? { label: `Sélectionner tous les « ${typeOf(list[0]).name} »`, onClick: actions.selectSameType } : { label: 'Sélectionner les mêmes modèles', onClick: actions.selectSameType },
        );
      }
      items.push(
        { label: 'Tout sélectionner', shortcut: keyOf('select.all'), disabled: !all || n === all, onClick: actions.selectAll },
        groups.length ? { label: 'Sélectionner un groupe', items: groups.map((g) => ({ label: `${g.name} (${g.ids.length})`, disabled: !g.ids.length, onClick: () => actions.selectGroup(g.id) })) } : null,
        n && n < all ? { label: 'Inverser la sélection', shortcut: keyOf('select.invert'), onClick: actions.invertSelection } : null,
        n ? { label: 'Désélectionner', shortcut: keyOf('select.none'), onClick: actions.clearSelection } : null,
      );
      if (n) items.push('-', { label: n > 1 ? `Supprimer les ${n} projecteurs` : 'Supprimer', shortcut: keyOf('edit.delete'), danger: true, onClick: actions.deleteSelection });
      if (!all) items.push({ header: 'Ajoute des projecteurs depuis la bibliothèque' });
      return items;
    },

    /** Menu d'un groupe (clic droit sur un groupe de la liste). */
    groupMenu(gid) {
      const g = store.group(gid);
      if (!g) return [];
      const ids = g.ids.filter((id) => store.fixtures.has(id));
      const selIds = store.selectedIds();
      const anyOff = ids.some((id) => !store.fixtures.get(id).state.power);
      const v5 = ids.filter((id) => isV5(store.fixtures.get(id)));
      return [
        { header: `${g.name} · ${ids.length} projecteur${ids.length > 1 ? 's' : ''}` },
        { label: 'Sélectionner le groupe', disabled: !ids.length, onClick: () => actions.selectGroup(gid) },
        { label: 'Ajouter à la sélection', shortcut: 'Ctrl+clic', disabled: !ids.length, onClick: () => actions.selectGroup(gid, true) },
        '-',
        { label: 'Ajouter la sélection au groupe', disabled: !selIds.length || selIds.every((id) => ids.includes(id)), onClick: () => actions.addSelectionToGroup(gid) },
        { label: 'Retirer la sélection du groupe', disabled: !selIds.some((id) => ids.includes(id)), onClick: () => actions.removeSelectionFromGroup(gid) },
        '-',
        { label: anyOff ? 'Allumer le groupe' : 'Éteindre le groupe', disabled: !ids.length, onClick: () => setPower(ids, anyOff) },
        v5.length ? { label: 'Liaison CRMX', items: [{ label: 'Lier le groupe (touche LINK)', onClick: () => setCrmx(ids, true) }, { label: 'Délier le groupe', onClick: () => setCrmx(ids, false) }] } : null,
        { label: 'Adresser le groupe à la suite…', disabled: !ids.length, onClick: () => actions.openReaddress({ scope: `group:${gid}` }) },
        '-',
        { label: 'Renommer', shortcut: 'Double-clic', onClick: () => actions.startRenameGroup(gid) },
        { label: 'Supprimer le groupe', title: 'Les projecteurs restent dans la scène', danger: true, onClick: () => actions.deleteGroup(gid) },
      ];
    },
  };
  return actions;
}
