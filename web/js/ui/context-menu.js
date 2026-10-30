// Menu contextuel (clic droit) : actions, séparateurs '-', titres { header } et sous-menus { items }.
// Sert aussi aux menus déroulants de la barre de menus (Fichier, Édition…).
import { h } from './dom.js';

let current = null;
let currentOpts = {};

export function closeContextMenu() {
  if (!current) return;
  current.remove();
  current = null;
  document.removeEventListener('pointerdown', onOutside, true);
  window.removeEventListener('keydown', onKey, true);
  window.removeEventListener('blur', closeContextMenu);
  window.removeEventListener('resize', closeContextMenu);
  const { onClose } = currentOpts;
  currentOpts = {};
  onClose?.();
}

function onOutside(e) {
  if (!current || current.contains(e.target)) return;
  // la barre de menus gère elle-même un clic sur ses titres (ouvrir / refermer)
  if (currentOpts.ignore?.contains(e.target)) return;
  closeContextMenu();
}

function focusables(menu) {
  return [...menu.children].filter((el) => el.classList.contains('ctx-item') && !el.classList.contains('disabled'));
}

function onKey(e) {
  if (!current) return;
  const menu = document.activeElement?.closest('.ctx-menu') || current;
  const items = focusables(menu);
  const i = items.indexOf(document.activeElement);
  const stop = () => {
    e.preventDefault();
    e.stopPropagation();
  };
  if (e.key === 'Escape') {
    stop();
    closeContextMenu();
  } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    stop();
    const n = items.length;
    if (n) items[(i + (e.key === 'ArrowDown' ? 1 : n - 1) + (i < 0 && e.key === 'ArrowUp' ? 1 : 0)) % n].focus();
  } else if (e.key === 'ArrowRight' && document.activeElement?.classList.contains('has-sub')) {
    stop();
    openSub(document.activeElement);
    const sub = document.activeElement.querySelector(':scope > .ctx-menu');
    focusables(sub)[0]?.focus();
  } else if (e.key === 'ArrowLeft' && menu !== current) {
    stop();
    const owner = menu.parentElement;
    owner.classList.remove('open');
    owner.focus();
  } else if ((e.key === 'ArrowLeft' || e.key === 'ArrowRight') && currentOpts.onSide) {
    // barre de menus : menu voisin
    stop();
    currentOpts.onSide(e.key === 'ArrowLeft' ? -1 : 1);
  } else if (e.key === 'Enter' || e.key === ' ') {
    if (i >= 0) {
      stop();
      items[i].click();
    }
  } else stop(); // le menu garde le clavier (évite les raccourcis de la vue 3D)
}

function openSub(row) {
  for (const sib of row.parentElement.children) if (sib !== row) sib.classList.remove('open');
  row.classList.add('open');
}

function buildMenu(items) {
  const menu = h('div', { class: 'ctx-menu', role: 'menu' });
  let lastSep = true;
  for (const it of items) {
    if (!it) continue;
    if (it === '-') {
      if (!lastSep) menu.append(h('div', { class: 'ctx-sep' }));
      lastSep = true;
      continue;
    }
    lastSep = false;
    if (it.header) {
      menu.append(h('div', { class: 'ctx-header' }, it.header));
      continue;
    }
    const sub = it.items?.filter(Boolean);
    const disabled = it.disabled || (sub && !sub.length);
    const row = h(
      'div',
      {
        class: `ctx-item${it.danger ? ' danger' : ''}${sub ? ' has-sub' : ''}${disabled ? ' disabled' : ''}`,
        role: 'menuitem',
        tabindex: -1,
        title: it.title,
      },
      h('span', { class: 'ctx-check' }, it.checked ? '✓' : it.radio ? '•' : ''),
      h('span', { class: 'ctx-label' }, it.label),
      h('span', { class: 'ctx-key' }, sub ? '▸' : it.shortcut || ''),
    );
    if (!disabled) {
      if (sub) {
        row.append(buildMenu(sub));
        row.addEventListener('pointerenter', () => openSub(row));
        row.addEventListener('click', (e) => {
          if (e.target === row || e.target.parentElement === row) openSub(row);
        });
      } else {
        row.addEventListener('pointerenter', () => {
          for (const sib of row.parentElement.children) sib.classList.remove('open');
        });
        row.addEventListener('click', (e) => {
          e.stopPropagation();
          closeContextMenu();
          it.onClick?.();
        });
      }
    }
    menu.append(row);
  }
  if (menu.lastElementChild?.classList.contains('ctx-sep')) menu.lastElementChild.remove();
  return menu;
}

/**
 * Ouvre un menu aux coordonnées écran (x, y).
 * opts.ignore : élément où un clic ne referme pas le menu ; opts.onClose : appelé à la fermeture ;
 * opts.onSide(-1 | 1) : flèches gauche / droite (barre de menus) ; opts.focusFirst : clavier.
 */
export function openContextMenu(x, y, items, opts = {}) {
  closeContextMenu();
  const menu = buildMenu(items);
  if (!menu.children.length) return;
  menu.classList.add('ctx-root');
  if (opts.className) menu.classList.add(opts.className);
  menu.style.left = '0px';
  menu.style.top = '0px';
  (document.fullscreenElement || document.body).append(menu);
  const r = menu.getBoundingClientRect();
  const left = Math.max(4, Math.min(x, window.innerWidth - r.width - 4));
  const top = Math.max(4, Math.min(y, window.innerHeight - r.height - 4));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
  // sous-menus ouverts vers la gauche s'il n'y a pas la place à droite
  if (left + r.width * 2 + 8 > window.innerWidth) menu.classList.add('flip');
  current = menu;
  currentOpts = opts;
  document.addEventListener('pointerdown', onOutside, true);
  window.addEventListener('keydown', onKey, true);
  window.addEventListener('blur', closeContextMenu);
  window.addEventListener('resize', closeContextMenu);
  if (opts.focusFirst) focusables(menu)[0]?.focus();
}

export const isContextMenuOpen = () => !!current;
