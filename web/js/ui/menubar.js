// Barre de menus (Fichier, Édition, Affichage, Outils, Aide), comme dans la plupart des logiciels :
// un clic ouvre le menu, le survol passe d'un menu à l'autre, les flèches et Entrée marchent au clavier.
// Le contenu de chaque menu est construit à l'ouverture (cases cochées, commandes grisées à jour).
import { h } from './dom.js';
import { closeContextMenu, openContextMenu } from './context-menu.js';

/** menus : [{ id, label, items: () => [éléments de menu contextuel] }] */
export function mountMenubar(host, menus) {
  let openIdx = -1;
  const buttons = menus.map((m, i) =>
    h(
      'button',
      {
        class: 'mb-item',
        role: 'menuitem',
        'aria-haspopup': 'true',
        onpointerdown: (e) => {
          if (e.button !== 0) return;
          e.preventDefault();
          if (openIdx === i) closeContextMenu();
          else open(i);
        },
        onpointerenter: () => {
          if (openIdx >= 0 && openIdx !== i) open(i);
        },
        onkeydown: (e) => {
          if (['Enter', ' ', 'ArrowDown'].includes(e.key)) {
            e.preventDefault();
            open(i, true);
          } else if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
            e.preventDefault();
            buttons[(i + (e.key === 'ArrowRight' ? 1 : -1) + menus.length) % menus.length].focus();
          }
        },
      },
      m.label,
    ),
  );
  host.replaceChildren(...buttons);

  function open(i, keyboard = false) {
    const r = buttons[i].getBoundingClientRect();
    openIdx = i;
    buttons.forEach((b, j) => b.classList.toggle('open', j === i));
    openContextMenu(r.left, r.bottom + 3, menus[i].items(), {
      className: 'mb-menu',
      ignore: host,
      focusFirst: keyboard,
      onSide: (dir) => open((i + dir + menus.length) % menus.length, true),
      onClose: () => {
        if (openIdx !== i) return;
        openIdx = -1;
        buttons[i].classList.remove('open');
      },
    });
  }

  return {
    /** Ouvre le premier menu au clavier (F10, comme sous Windows). */
    focus: () => open(0, true),
    get isOpen() {
      return openIdx >= 0;
    },
  };
}
