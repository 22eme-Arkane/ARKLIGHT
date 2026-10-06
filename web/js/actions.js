// Actions sur la sélection et les groupes, partagées par le clavier, les menus contextuels et l'inspecteur.
import { getFixtureType } from '/shared/fixtures/index.js';
import { isAsteraV5 } from '/shared/engine/output.js';
import { planAddressing } from '/shared/engine/addressing.js';
import { toast } from './ui/dom.js';
import { createReaddressDialog } from './ui/readdress.js';

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
          settings: { ...f.settings, dmxAddress: undefined }, // nouvelle adresse libre
        })),
      });
    },
    frame: () => viewport.frameSelected(),
    aim: () => viewport.aimSelectedAtActor(),
    drop: () => viewport.dropSelectedToFloor(),

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
      return [
        { header: o.name },
        { label: 'Dupliquer', shortcut: keyOf('edit.duplicate'), onClick: () => actions.duplicateObject(id) },
        { label: 'Poser au sol', onClick: () => actions.updateObject(id, { position: [o.position[0], 0, o.position[2]] }) },
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
          list.some(isAimable) ? { label: 'Viser le personnage', onClick: actions.aim } : null,
          { label: 'Poser au sol', shortcut: keyOf('fixture.drop'), onClick: actions.drop },
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
