// Écran « Projets », comme dans PANDORA : au démarrage et avec « Ouvrir un projet ». Grandes
// vignettes (la vue 3D au moment de l'enregistrement), nom et date ; nouveau projet, projet
// d'exemple, ouverture d'un fichier ; dossier des projets au choix (disque externe, NAS…).
import { h, toast } from './dom.js';
import { icon } from './icons.js';
import { openContextMenu } from './context-menu.js';
import { askText } from './dialogs.js';
import { LANGS, lang, langChoice, setLang, systemLanguage, t } from '../i18n.js';
import { DEMOS } from '/shared/demos.js';
import { planCountEl } from './plans.js';

const STARTUP_KEY = 'arklight.projects.startup';

const norm = (s) =>
  String(s)
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase();
/** « 3 projecteurs », dans la langue de l'interface. */
const plural = (n, one, many) => t(n > 1 ? `{0} ${many}` : `{0} ${one}`, { 0: n });
const samePath = (a, b) => !!a && !!b && a.replace(/\//g, '\\').toLowerCase() === b.replace(/\//g, '\\').toLowerCase();

/** « il y a 5 min », « aujourd’hui à 14:32 », « hier à 9:05 », « le 06/10/2026 ». */
function when(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return '';
  const now = new Date();
  const mins = Math.round((now - d) / 60000);
  const loc = lang === 'en' ? 'en-GB' : 'fr-FR';
  if (mins < 1) return t('à l’instant');
  if (mins < 60) return t('il y a {0} min', { 0: mins });
  const hm = d.toLocaleTimeString(loc, { hour: '2-digit', minute: '2-digit' });
  if (d.toDateString() === now.toDateString()) return t('aujourd’hui à {0}', { 0: hm });
  const y = new Date(now);
  y.setDate(now.getDate() - 1);
  if (d.toDateString() === y.toDateString()) return t('hier à {0}', { 0: hm });
  return t('le {0}', { 0: d.toLocaleDateString(loc) });
}

/** Préférence « afficher les projets au démarrage » (oui par défaut). */
export function projectsAtStartup() {
  try {
    return localStorage.getItem(STARTUP_KEY) !== '0';
  } catch {
    return true;
  }
}

/**
 * @param {object} o
 * @param {Function} o.confirmDiscard  (action) => Promise<boolean> : proposer d'enregistrer avant de changer de projet
 */
export function mountProjects({ store, net, desktop, confirmDiscard }) {
  let data = null; // dernière liste reçue du serveur
  let query = '';
  let isOpen = false;

  const search = h('input', { type: 'search', placeholder: 'Rechercher un projet', spellcheck: 'false', oninput: () => ((query = search.value), renderGrid()) });
  const grid = h('div', { class: 'hub-grid' });
  const count = h('span', { class: 'hub-count' });
  const empty = h('div', { class: 'hub-empty hidden' });
  const folderPath = h('code', { class: 'hub-folder-path' });
  const folderState = h('span', { class: 'hub-folder-state' });
  const startup = h('input', {
    type: 'checkbox',
    checked: projectsAtStartup(),
    onchange: () => {
      try {
        localStorage.setItem(STARTUP_KEY, startup.checked ? '1' : '0');
      } catch {
        /* stockage indisponible : on garde le choix pour cette session */
      }
    },
  });

  const root = h(
    'div',
    { class: 'hub hidden', role: 'dialog', 'aria-modal': 'true', 'aria-label': 'Projets ARKLIGHT' },
    h(
      'header',
      { class: 'hub-head' },
      h('div', { class: 'hub-brand' }, h('img', { src: '/img/ark-light-logo.png', alt: '' }), h('span', { class: 'hub-title' }, 'ARK', h('b', {}, 'LIGHT')), h('span', { class: 'hub-sub-title' }, 'Projets')),
      h(
        'div',
        { class: 'hub-actions' },
        h('button', { class: 'primary', onclick: () => newProject() }, icon('plus'), 'Nouveau projet'),
        h(
          'button',
          {
            title: 'Des projets tout prêts : projecteurs, grip, décor…',
            onclick: (e) => {
              const r = e.currentTarget.getBoundingClientRect();
              openContextMenu(r.left, r.bottom + 4, demoMenu());
            },
          },
          icon('sparkle'),
          'Projets d’exemple ▾',
        ),
        desktop ? h('button', { onclick: () => openFile(), title: 'Ouvrir un projet rangé ailleurs (clé USB, disque externe…)' }, icon('folder-open'), 'Ouvrir un fichier…') : null,
      ),
      h('label', { class: 'hub-search' }, icon('search'), search),
      h('button', { class: 'hub-close', title: 'Revenir au projet en cours (Échap)', onclick: () => close() }, icon('close')),
    ),
    h('main', { class: 'hub-body' }, h('div', { class: 'hub-caption' }, 'PROJETS', count), grid, empty),
    h(
      'footer',
      { class: 'hub-foot' },
      h(
        'div',
        { class: 'hub-folder' },
        icon('folder'),
        h('span', { class: 'hub-folder-label' }, 'Dossier des projets'),
        folderPath,
        desktop ? h('button', { onclick: () => chooseFolder(), title: 'Les nouveaux projets seront enregistrés dans ce dossier' }, 'Changer…') : null,
        desktop ? h('button', { onclick: () => window.arkDesktop.showProjectsFolder() }, 'Ouvrir le dossier') : null,
        folderState,
      ),
      h('label', { class: 'hub-startup' }, startup, 'Afficher les projets au démarrage'),
      h(
        'div',
        { class: 'hub-lang', role: 'group', title: 'Langue' },
        // Auto : la langue de l'ordinateur ; le bouton allumé suit le choix (sans rechargement si la langue ne change pas)
        [['auto', t('Auto'), t('Automatique — langue de l’ordinateur ({0})', { 0: LANGS.find((l) => l.id === systemLanguage)?.label })], ...LANGS.map((l) => [l.id, l.id.toUpperCase(), l.label])].map(([id, text, tip]) =>
          h('button', {
            class: langChoice() === id ? 'active' : '',
            title: tip,
            translate: id === 'auto' ? null : 'no',
            onclick: (e) => {
              for (const b of e.currentTarget.parentElement.children) b.classList.toggle('active', b === e.currentTarget);
              setLang(id, { reopenProjects: true });
            },
          }, text),
        ),
      ),
    ),
  );
  document.body.append(root);

  const isCurrent = (it) => samePath(it.path, store.history.savedPath);

  // ------------------------------------------------------------------ affichage
  function thumbEl(src) {
    const t = h('div', { class: 'hub-thumb' }, src ? null : icon('cube', 44));
    if (src) t.style.backgroundImage = `url("${src}")`;
    return t;
  }

  function card(it) {
    const open = isCurrent(it);
    const more = h(
      'button',
      {
        class: 'hub-more',
        title: 'Plus d’actions',
        onclick: (e) => {
          e.stopPropagation();
          const r = e.currentTarget.getBoundingClientRect();
          openContextMenu(r.left, r.bottom + 4, menuFor(it));
        },
      },
      icon('dots'),
    );
    return h(
      'div',
      {
        class: `hub-card${open ? ' current' : ''}`,
        tabindex: 0,
        title: it.path,
        onclick: () => openItem(it),
        onkeydown: (e) => e.key === 'Enter' && openItem(it),
        oncontextmenu: (e) => {
          e.preventDefault();
          openContextMenu(e.clientX, e.clientY, menuFor(it));
        },
      },
      thumbEl(it.thumbnail),
      open ? h('span', { class: 'hub-badge' }, 'ouvert') : null,
      h(
        'div',
        { class: 'hub-meta' },
        h('div', { class: 'hub-name' }, it.name),
        h('div', { class: 'hub-sub' }, t('Modifié {0} · {1}', { 0: when(it.modifiedAt), 1: plural(it.fixtures, 'projecteur', 'projecteurs') })),
        it.plans > 1 ? h('div', { class: 'hub-sub' }, plural(it.sequences || 1, 'séquence', 'séquences'), ' · ', planCountEl(it.plans)) : null,
        it.inRoot ? null : h('div', { class: 'hub-path' }, it.folder),
      ),
      more,
    );
  }

  function renderGrid() {
    const items = (data?.items || []).filter((it) => !query || norm(it.name).includes(norm(query)));
    count.textContent = data ? ` · ${plural(data.items.length, 'projet', 'projets')}` : '';
    count.setAttribute('translate', 'no'); // déjà dans la langue de l'interface
    grid.replaceChildren(...items.map(card));
    empty.classList.toggle('hidden', !!items.length || !data);
    empty.textContent = query
      ? `Aucun projet ne contient « ${query} ».`
      : data && !data.rootAvailable
        ? 'Le dossier des projets est introuvable (disque débranché ?). Ses projets réapparaîtront quand il sera rebranché.'
        : 'Aucun projet enregistré pour l’instant. Crée ton premier projet, ou ouvre le projet d’exemple pour voir une scène complète.';
  }

  function renderFoot() {
    folderPath.textContent = data?.root || '…';
    folderPath.title = data?.root || '';
    folderState.className = `hub-folder-state${data && !data.rootAvailable ? ' warn' : ''}`;
    folderState.textContent = !data
      ? ''
      : !data.rootAvailable
        ? '⚠ Dossier introuvable pour l’instant (disque débranché ?)'
        : desktop
          ? 'Changer de dossier ne déplace aucun projet : les anciens restent dans la liste.'
          : 'Le dossier se choisit dans l’application installée.';
  }

  function render() {
    renderGrid();
    renderFoot();
  }

  // ------------------------------------------------------------------ actions
  function menuFor(it) {
    const open = isCurrent(it);
    return [
      { header: it.name },
      { label: open ? 'Revenir à ce projet' : 'Ouvrir', onClick: () => openItem(it) },
      { label: 'Renommer…', onClick: () => renameItem(it) },
      { label: 'Dupliquer…', onClick: () => duplicateItem(it) },
      desktop ? { label: 'Afficher dans l’Explorateur', onClick: () => window.arkDesktop.revealProject(it.path) } : null,
      '-',
      it.inRoot ? null : { label: 'Retirer de la liste', onClick: () => net.send({ t: 'project.forget', path: it.path }) },
      { label: 'Mettre à la corbeille…', danger: true, disabled: open, onClick: () => trashItem(it) },
    ];
  }

  async function openItem(it) {
    if (isCurrent(it)) return close();
    if (!(await confirmDiscard('ouvrir un autre projet'))) return;
    net.send({ t: 'project.open', path: it.path });
    close();
  }

  async function renameItem(it) {
    const name = await askText({ title: 'Renommer le projet', label: 'Nouveau nom', value: it.name, ok: 'Renommer' });
    if (name && name !== it.name) net.send({ t: 'project.rename', path: it.path, name });
  }

  async function duplicateItem(it) {
    const name = await askText({ title: 'Dupliquer le projet', label: 'Nom de la copie', value: `${it.name} (copie)`, ok: 'Dupliquer' });
    if (name) net.send({ t: 'project.duplicate', path: it.path, name });
  }

  function trashItem(it) {
    const where = desktop ? 'Tu pourras le récupérer dans la corbeille de Windows.' : 'Il sera rangé dans le dossier « _corbeille » du dossier des projets.';
    if (confirm(t('Mettre « {name} » à la corbeille ?\n{where}', { name: it.name, where: t(where) }))) net.send({ t: 'project.trash', path: it.path });
  }

  async function newProject() {
    if (!(await confirmDiscard('créer un nouveau projet'))) return;
    net.send({ t: 'project.new' });
    close();
  }

  /** Menu des projets d'exemple (écran des projets, menu Fichier). */
  function demoMenu() {
    return DEMOS.map((d) => ({ label: d.name, title: d.note, onClick: () => openDemo(d.id) }));
  }

  async function openDemo(id) {
    if (!(await confirmDiscard('ouvrir le projet d’exemple'))) return;
    net.send({ t: 'project.demo', id });
    close();
    toast('Projet d’exemple ouvert : enregistre-le (Ctrl+S) pour le garder', 'info', 4500);
  }

  async function openFile() {
    if (!desktop || !(await confirmDiscard('ouvrir un autre projet'))) return;
    const r = await window.arkDesktop.openProjectFile();
    if (r?.error) toast(r.error, 'error', 4000);
    else if (r) close();
  }

  async function chooseFolder() {
    const r = await window.arkDesktop.chooseProjectsFolder();
    if (r?.error) toast(r.error, 'error');
    else if (r) toast(`Dossier des projets : ${r.root}`);
  }

  // ------------------------------------------------------------------ ouverture / fermeture
  function refresh() {
    net.send({ t: 'projects.list' });
    render();
  }

  function open() {
    if (!isOpen) {
      isOpen = true;
      root.classList.remove('hidden');
      document.body.classList.add('hub-open');
    }
    query = '';
    search.value = '';
    refresh();
    setTimeout(() => (root.querySelector('.hub-card') || search).focus(), 50);
  }

  function close() {
    if (!isOpen) return;
    isOpen = false;
    root.classList.add('hidden');
    document.body.classList.remove('hub-open');
  }

  // Échap : revenir au projet (sauf si une petite fenêtre ou un menu est ouvert au-dessus)
  window.addEventListener(
    'keydown',
    (e) => {
      if (!isOpen || e.key !== 'Escape' || document.querySelector('dialog[open]') || document.querySelector('.ctx-root')) return;
      e.preventDefault();
      e.stopPropagation();
      close();
    },
    true,
  );

  return {
    open,
    close,
    get isOpen() {
      return isOpen;
    },
    openFile,
    chooseFolder,
    openDemo,
    demoMenu,
    /** Réponses du serveur : liste des projets, dossier changé. */
    onMessage(msg) {
      if (msg.t === 'projects') {
        data = msg;
        if (isOpen) render();
      } else if (msg.t === 'projects.changed' && isOpen) refresh();
    },
  };
}
