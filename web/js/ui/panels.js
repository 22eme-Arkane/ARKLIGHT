// Panneaux repliables et redimensionnables : bibliothèque (gauche), inspecteur (droite), panneau
// du bas (console, moniteur DMX…). On tire leur bord pour les agrandir ; un double-clic sur le bord
// ou la poignée au milieu du bord (comme dans PANDORA) les replie et les déplie. Tailles et état
// mémorisés dans le navigateur.
import { h } from './dom.js';
import { icon } from './icons.js';
import { t } from '../i18n.js';

const KEY = 'arklight.layout';
const LEGACY_KEY = 'arklight.panels'; // version précédente : panneaux affichés ou masqués
const DEFAULTS = { left: 260, right: 330, bottom: 230, leftFolded: false, rightFolded: false, bottomFolded: false };
const FOLDED = { left: 30, right: 30, bottom: 36 };
const TOPBAR = 44;

function load() {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) || 'null');
    if (saved) return { ...DEFAULTS, ...saved };
    const old = JSON.parse(localStorage.getItem(LEGACY_KEY) || 'null');
    if (old) return { ...DEFAULTS, leftFolded: old.left === false, rightFolded: old.right === false };
  } catch {
    /* préférences illisibles : tailles par défaut */
  }
  return { ...DEFAULTS };
}

/** keyOf(id) : touche de la commande (infobulles). onChange(which) : après un repli / dépli. */
export function mountPanels({ keyOf = () => '', onChange = () => {} } = {}) {
  const st = load();
  const root = document.documentElement;
  const layout = document.querySelector('.layout');
  const left = document.querySelector('.panel.left');
  const right = document.querySelector('.panel.right');
  const drawer = document.getElementById('drawer');
  const NAMES = { left: 'la bibliothèque', right: 'l’inspecteur', bottom: 'le panneau du bas' };
  const CMD = { left: 'panel.left', right: 'panel.right', bottom: 'panel.bottom' };
  const tip = (which, verb) => `${t(`${verb} ${NAMES[which]}`)}${keyOf(CMD[which]) ? ` (${keyOf(CMD[which])})` : ''}`;

  const clamp = (v, a, b) => Math.round(Math.min(b, Math.max(a, v)));
  const maxSide = () => Math.max(260, window.innerWidth * 0.42);
  const maxBottom = () => Math.max(140, window.innerHeight - TOPBAR - 160);

  // poignées au milieu des bords, visibles panneau ouvert ou replié ; bandes verticales des panneaux repliés
  const handle = (which) => h('button', { class: `panel-handle ${which}`, onclick: () => toggle(which) }, icon(`chevron-${{ left: 'left', right: 'right', bottom: 'down' }[which]}`, 24));
  const strip = (which, text) => h('button', { class: `panel-strip ${which}`, onclick: () => toggle(which) }, h('span', { class: 'vt' }, text));
  const handles = { left: handle('left'), right: handle('right'), bottom: handle('bottom') };
  left.append(strip('left', 'Bibliothèque'));
  right.append(strip('right', 'Inspecteur'));
  layout.append(handles.left, handles.right);
  drawer.querySelector('.tabs').append(handles.bottom);

  // bords à tirer
  const splitter = (which, cls) => {
    const el = h('div', { class: `splitter ${cls} ${which}`, title: 'Glisser pour redimensionner · double-clic : replier / déplier' });
    el.addEventListener('pointerdown', (e) => startDrag(which, e));
    el.addEventListener('dblclick', () => toggle(which));
    return el;
  };
  layout.append(splitter('left', 'v'), splitter('right', 'v'));
  drawer.append(splitter('bottom', 'h'));

  function apply() {
    root.style.setProperty('--left-w', `${st.leftFolded ? FOLDED.left : clamp(st.left, 180, maxSide())}px`);
    root.style.setProperty('--right-w', `${st.rightFolded ? FOLDED.right : clamp(st.right, 240, maxSide())}px`);
    root.style.setProperty('--drawer-h', `${st.bottomFolded ? FOLDED.bottom : clamp(st.bottom, 120, maxBottom())}px`);
    document.body.classList.toggle('fold-left', st.leftFolded);
    document.body.classList.toggle('fold-right', st.rightFolded);
    drawer.classList.toggle('collapsed', st.bottomFolded);
    for (const which of ['left', 'right', 'bottom']) {
      const folded = st[`${which}Folded`];
      handles[which].classList.toggle('folded', folded); // la flèche se retourne
      handles[which].title = tip(which, folded ? 'Déplier' : 'Replier');
    }
    for (const el of document.querySelectorAll('.panel-strip')) el.title = tip(el.classList.contains('left') ? 'left' : 'right', 'Déplier');
  }

  function save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(st));
    } catch {
      /* stockage indisponible : la disposition reviendra par défaut */
    }
  }

  function toggle(which) {
    st[`${which}Folded`] = !st[`${which}Folded`];
    apply();
    save();
    onChange(which);
  }

  function startDrag(which, e) {
    if (e.button !== 0) return;
    e.preventDefault();
    const move = (ev) => {
      if (which === 'left') st.left = clamp(ev.clientX, 180, maxSide());
      else if (which === 'right') st.right = clamp(window.innerWidth - ev.clientX, 240, maxSide());
      else st.bottom = clamp(window.innerHeight - ev.clientY, 120, maxBottom());
      if (st[`${which}Folded`]) {
        st[`${which}Folded`] = false;
        onChange(which);
      }
      apply();
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      document.body.classList.remove('resizing', 'resizing-h', 'resizing-v');
      save();
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    document.body.classList.add('resizing', which === 'bottom' ? 'resizing-v' : 'resizing-h');
  }

  window.addEventListener('resize', apply);
  apply();

  return {
    toggle,
    /** Panneau replié ? */
    folded: (which) => !!st[`${which}Folded`],
    open(which) {
      if (st[`${which}Folded`]) toggle(which);
    },
    refreshTitles: apply,
  };
}
