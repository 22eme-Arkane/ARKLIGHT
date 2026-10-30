// Séquences et plans du projet : sélecteur dans la barre du haut (« Séquence 1 › Plan 2 ▾ ») et
// panneau des vignettes pour passer d'un plan de feu à l'autre, en créer, renommer, réordonner.
import { currentPlan, flatPlans, neighborPlan } from '/shared/plans.js';
import { t } from '../i18n.js';
import { askConfirm, askText } from './dialogs.js';
import { h, toast } from './dom.js';
import { iconSvg } from './icons.js';
import { closeContextMenu, openContextMenu } from './context-menu.js';

const THUMB_W = 320;
/** « 1 plan », « 3 plans » (le mot seul passe par la traduction de l'interface). */
export const planCountEl = (n) => [`${n} `, h('span', {}, n > 1 ? t('plans') : t('plan'))];
const THUMB_H = 180;

export function mountPlans({ store, net, viewport, keyOf = () => '' }) {
  const picker = h(
    'button',
    { id: 'plan-picker', class: 'plan-picker', onclick: () => toggle() },
    h('span', { class: 'pp-seq' }),
    h('span', { class: 'pp-sep' }, '›'),
    h('span', { class: 'pp-plan' }),
    h('span', { class: 'pp-caret' }, '▾'),
  );
  document.querySelector('.scene-title')?.append(picker);
  let panel = null;

  /** Image de la vue 3D (vignette du plan qu'on quitte). */
  const snapshot = () => {
    try {
      return viewport.snapshot(THUMB_W, THUMB_H);
    } catch {
      return null;
    }
  };

  function applyPicker() {
    const cur = currentPlan(store.plans);
    picker.querySelector('.pp-seq').textContent = cur?.seq.name || '';
    picker.querySelector('.pp-plan').textContent = cur?.plan.name || '';
    picker.title = t('Séquences et plans du projet — {0} / {1} : plan précédent / suivant', { 0: keyOf('plan.prev') || 'Ctrl+PgUp', 1: keyOf('plan.next') || 'Ctrl+PgDn' });
    picker.classList.toggle('hidden', !cur);
  }

  // ------------------------------------------------------------------ actions
  function switchTo(id) {
    if (id === store.plans.current?.plan) return;
    net.send({ t: 'plan.switch', id, thumbnail: snapshot() });
  }

  function step(dir) {
    const n = neighborPlan(store.plans, dir);
    if (n) switchTo(n.plan.id);
    else toast(dir > 0 ? t('Dernier plan du projet') : t('Premier plan du projet'));
  }

  function newPlan(mode = 'copy', seq = null) {
    net.send({ t: 'plan.new', mode, seq, thumbnail: snapshot() });
  }

  function newSequence(mode = 'empty') {
    net.send({ t: 'sequence.new', mode, thumbnail: snapshot() });
  }

  async function renamePlan(p) {
    const name = await askText({ title: t('Renommer le plan'), value: p.name, ok: t('Renommer'), note: t('Par exemple « Plan 3 — contrechamp » ou « 12/4 gros plan ».') });
    if (name) net.send({ t: 'plan.rename', id: p.id, name });
  }

  async function renameSequence(sq) {
    const name = await askText({ title: t('Renommer la séquence'), value: sq.name, ok: t('Renommer'), note: t('Par exemple « Séq. 12 — INT. CAFÉ — NUIT ».') });
    if (name) net.send({ t: 'sequence.rename', id: sq.id, name });
  }

  async function deletePlan(p) {
    if (flatPlans(store.plans).length <= 1) return toast(t('Le projet garde au moins un plan'), 'error');
    const ok = await askConfirm({ title: t('Supprimer le plan « {0} » ?', { 0: p.name }), note: t('Son plan de feu (projecteurs, grip, décor, annotations) sera perdu. On ne peut pas l’annuler.'), ok: t('Supprimer'), danger: true });
    if (ok) net.send({ t: 'plan.delete', id: p.id });
  }

  async function deleteSequence(sq) {
    if (store.plans.sequences.length <= 1) return toast(t('Le projet garde au moins une séquence'), 'error');
    const ok = await askConfirm({ title: t('Supprimer la séquence « {0} » ?', { 0: sq.name }), note: t('Ses {0} plan(s) seront perdus. On ne peut pas l’annuler.', { 0: sq.plans.length }), ok: t('Supprimer'), danger: true });
    if (ok) net.send({ t: 'sequence.delete', id: sq.id });
  }

  const movePlan = (p, seq, index) => net.send({ t: 'plan.move', id: p.id, seq: seq.id, index });

  /** Choix de départ d'un nouveau plan ou d'une nouvelle séquence. */
  const startMenu = (fn) => [
    { label: t('Copie du plan actuel'), title: t('Mêmes projecteurs, grip et décor : on ajuste pour le nouveau plan'), onClick: () => fn('copy') },
    { label: t('Même décor, sans lumière'), title: t('Le décor et les personnages, sans projecteurs ni grip'), onClick: () => fn('decor') },
    { label: t('Plan vide'), onClick: () => fn('empty') },
  ];

  function planMenu(sq, p) {
    const i = sq.plans.indexOf(p);
    const others = store.plans.sequences.filter((s) => s !== sq);
    return [
      { label: t('Ouvrir ce plan'), disabled: p.id === store.plans.current?.plan, onClick: () => switchTo(p.id) },
      { label: t('Renommer…'), onClick: () => renamePlan(p) },
      { label: t('Dupliquer'), onClick: () => net.send({ t: 'plan.duplicate', id: p.id }) },
      '-',
      { label: t('Déplacer vers la gauche'), disabled: i <= 0, onClick: () => movePlan(p, sq, i - 1) },
      { label: t('Déplacer vers la droite'), disabled: i >= sq.plans.length - 1, onClick: () => movePlan(p, sq, i + 1) },
      others.length ? { label: t('Déplacer dans la séquence'), items: others.map((o) => ({ label: o.name, onClick: () => movePlan(p, o, o.plans.length) })) } : null,
      '-',
      { label: t('Supprimer le plan…'), danger: true, onClick: () => deletePlan(p) },
    ];
  }

  function sequenceMenu(sq) {
    const i = store.plans.sequences.indexOf(sq);
    return [
      { label: t('Renommer…'), onClick: () => renameSequence(sq) },
      { label: t('Nouveau plan'), items: startMenu((mode) => newPlan(mode, sq.id)) },
      '-',
      { label: t('Monter'), disabled: i <= 0, onClick: () => net.send({ t: 'sequence.move', id: sq.id, index: i - 1 }) },
      { label: t('Descendre'), disabled: i >= store.plans.sequences.length - 1, onClick: () => net.send({ t: 'sequence.move', id: sq.id, index: i + 1 }) },
      '-',
      { label: t('Supprimer la séquence…'), danger: true, onClick: () => deleteSequence(sq) },
    ];
  }

  const menuAt = (e, items) => {
    e.preventDefault();
    e.stopPropagation();
    const r = e.currentTarget.getBoundingClientRect();
    openContextMenu(e.type === 'contextmenu' ? e.clientX : r.left, e.type === 'contextmenu' ? e.clientY : r.bottom + 2, items, { ignore: panel });
  };

  // ------------------------------------------------------------------ panneau
  function render() {
    if (!panel) return;
    const info = store.plans;
    const curId = info.current?.plan;
    const liveThumb = panel.dataset.live || null; // vignette du plan courant, prise à l'ouverture
    const seqs = info.sequences.map((sq) =>
      h(
        'section',
        { class: 'pl-seq' },
        h(
          'header',
          { class: 'pl-seq-head', oncontextmenu: (e) => menuAt(e, sequenceMenu(sq)) },
          h('strong', { class: 'pl-seq-name', title: t('Double-clic : renommer'), ondblclick: () => renameSequence(sq) }, sq.name),
          h('span', { class: 'pl-count' }, planCountEl(sq.plans.length)),
          h('span', { class: 'spacer' }),
          h('button', { class: 'ib pl-more', title: t('Options de la séquence'), html: iconSvg('dots'), onclick: (e) => menuAt(e, sequenceMenu(sq)) }),
        ),
        h(
          'div',
          { class: 'pl-cards' },
          sq.plans.map((p) => {
            const active = p.id === curId;
            const thumb = (active && liveThumb) || p.thumbnail;
            return h(
              'div',
              {
                class: `pl-card${active ? ' active' : ''}`,
                tabindex: 0,
                title: active ? t('Plan ouvert') : t('Ouvrir ce plan'),
                onclick: () => {
                  switchTo(p.id);
                  close();
                },
                onkeydown: (e) => {
                  if (e.key === 'Enter') {
                    switchTo(p.id);
                    close();
                  }
                },
                oncontextmenu: (e) => menuAt(e, planMenu(sq, p)),
              },
              h('div', { class: 'pl-thumb' }, thumb ? h('img', { src: thumb, alt: '' }) : h('span', { class: 'pl-empty', html: iconSvg('camera') })),
              h(
                'div',
                { class: 'pl-meta' },
                h(
                  'span',
                  {
                    class: 'pl-name',
                    title: t('Double-clic : renommer'),
                    ondblclick: (e) => {
                      e.stopPropagation();
                      renamePlan(p);
                    },
                  },
                  p.name,
                ),
                h('button', { class: 'ib pl-more', title: t('Options du plan'), html: iconSvg('dots'), onclick: (e) => menuAt(e, planMenu(sq, p)) }),
              ),
              h('div', { class: 'pl-sub' }, t('{0} proj. · {1} obj.', { 0: p.fixtures, 1: p.objects })),
            );
          }),
          h(
            'button',
            { class: 'pl-card pl-add', title: t('Nouveau plan dans cette séquence'), onclick: (e) => menuAt(e, startMenu((mode) => newPlan(mode, sq.id))) },
            h('span', { class: 'pl-plus', html: iconSvg('plus') }),
            t('Nouveau plan'),
          ),
        ),
      ),
    );
    panel.replaceChildren(
      h(
        'div',
        { class: 'pl-head' },
        h('h3', {}, t('Séquences et plans')),
        h('span', { class: 'spacer' }),
        h('button', { onclick: (e) => menuAt(e, startMenu(newSequence)) }, h('span', { class: 'ico', html: iconSvg('plus') }), t('Nouvelle séquence ▾')),
        h('button', { class: 'ib', title: t('Fermer'), html: iconSvg('close'), onclick: () => close() }),
      ),
      h('div', { class: 'pl-body' }, seqs),
      h('div', { class: 'pl-foot note' }, t('Chaque plan a son propre plan de feu. Double-clic sur un nom pour le renommer, clic droit pour les autres options. {0} / {1} : plan précédent / suivant.', { 0: keyOf('plan.prev') || 'Ctrl+PgUp', 1: keyOf('plan.next') || 'Ctrl+PgDn' })),
    );
  }

  function onOutside(e) {
    if (!panel || panel.contains(e.target) || picker.contains(e.target) || e.target.closest?.('.ctx-menu') || e.target.closest?.('dialog')) return;
    close();
  }
  function onKey(e) {
    if (e.key === 'Escape' && panel && !document.querySelector('dialog[open]')) {
      e.stopPropagation();
      close();
    }
  }

  function open() {
    if (panel) return;
    panel = h('div', { class: 'plans-panel', role: 'dialog', 'aria-label': t('Séquences et plans') });
    // vignette à jour du plan ouvert (montrée tout de suite, gardée par le serveur)
    const live = snapshot();
    if (live) {
      panel.dataset.live = live;
      net.send({ t: 'plan.thumbnail', id: store.plans.current?.plan, thumbnail: live });
    }
    document.body.append(panel);
    const r = picker.getBoundingClientRect();
    panel.style.left = `${Math.max(8, Math.min(r.left, window.innerWidth - Math.min(820, window.innerWidth - 16) - 8))}px`;
    panel.style.top = `${r.bottom + 6}px`;
    picker.classList.add('open');
    render();
    document.addEventListener('pointerdown', onOutside, true);
    window.addEventListener('keydown', onKey, true);
  }

  function close() {
    if (!panel) return;
    closeContextMenu();
    panel.remove();
    panel = null;
    picker.classList.remove('open');
    document.removeEventListener('pointerdown', onOutside, true);
    window.removeEventListener('keydown', onKey, true);
  }

  const toggle = () => (panel ? close() : open());

  let lastPlan = store.plans?.current?.plan || null;
  store.addEventListener('plans', () => {
    applyPicker();
    render();
    const cur = currentPlan(store.plans);
    // changement de plan (ici ou depuis une autre interface) : on le dit
    if (cur && lastPlan && cur.plan.id !== lastPlan) toast(t('Plan : {0} › {1}', { 0: cur.seq.name, 1: cur.plan.name }));
    lastPlan = cur?.plan.id || null;
  });
  applyPicker();

  return { open, close, toggle, next: () => step(1), prev: () => step(-1), switchTo, newPlan, newSequence, isOpen: () => !!panel };
}
