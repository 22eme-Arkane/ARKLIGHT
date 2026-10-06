// Commandes de l'interface : une seule liste pour la barre de menus, les raccourcis clavier
// (personnalisables) et les menus contextuels. Rangées comme les menus : menu › rubrique › commande.

export const MENUS = [
  { id: 'file', label: 'Fichier', color: '#5aa9ff' },
  { id: 'edit', label: 'Édition', color: '#ffad3b' },
  { id: 'view', label: 'Affichage', color: '#3ddc84' },
  { id: 'tools', label: 'Outils', color: '#c792ea' },
  { id: 'help', label: 'Aide', color: '#9aa3ae' },
];

// keys : touches par défaut (« Ctrl+Shift+S », « 1 », « Delete »…). Plusieurs touches possibles par commande.
export const COMMANDS = [
  // ---- Fichier
  { id: 'scene.new', menu: 'file', section: 'Scène', label: 'Nouvelle scène', keys: ['Ctrl+N'] },
  { id: 'scene.open', menu: 'file', section: 'Scène', label: 'Ouvrir une scène…', keys: ['Ctrl+O'] },
  { id: 'scene.save', menu: 'file', section: 'Scène', label: 'Enregistrer', keys: ['Ctrl+S'] },
  { id: 'scene.saveAs', menu: 'file', section: 'Scène', label: 'Enregistrer sous…', keys: ['Ctrl+Shift+S'] },
  { id: 'scene.rename', menu: 'file', section: 'Scène', label: 'Renommer la scène', keys: ['F2'] },
  { id: 'plot.print', menu: 'file', section: 'Plan de feu', label: 'Imprimer / PDF (plan, matériel, patch)…', keys: ['Ctrl+P'] },
  { id: 'plot.png', menu: 'file', section: 'Plan de feu', label: 'Exporter le plan en image PNG', keys: [] },
  { id: 'plot.svg', menu: 'file', section: 'Plan de feu', label: 'Exporter le plan en SVG (vectoriel)', keys: [] },
  { id: 'plot.equipment', menu: 'file', section: 'Plan de feu', label: 'Liste du matériel (CSV)', keys: [] },
  { id: 'plot.patch', menu: 'file', section: 'Plan de feu', label: 'Patch DMX (CSV)', keys: [] },
  { id: 'app.quit', menu: 'file', section: 'Application', label: 'Quitter', keys: ['Ctrl+Q'], desktopOnly: true },

  // ---- Édition
  { id: 'edit.undo', menu: 'edit', section: 'Historique', label: 'Annuler', keys: ['Ctrl+Z'] },
  { id: 'edit.redo', menu: 'edit', section: 'Historique', label: 'Rétablir', keys: ['Ctrl+Y', 'Ctrl+Shift+Z'] },
  { id: 'edit.cut', menu: 'edit', section: 'Presse-papiers', label: 'Couper', keys: ['Ctrl+X'] },
  { id: 'edit.copy', menu: 'edit', section: 'Presse-papiers', label: 'Copier', keys: ['Ctrl+C'] },
  { id: 'edit.paste', menu: 'edit', section: 'Presse-papiers', label: 'Coller', keys: ['Ctrl+V'] },
  { id: 'edit.duplicate', menu: 'edit', section: 'Presse-papiers', label: 'Dupliquer', keys: ['Ctrl+D'] },
  { id: 'edit.delete', menu: 'edit', section: 'Presse-papiers', label: 'Supprimer', keys: ['Delete'] },
  { id: 'select.all', menu: 'edit', section: 'Sélection', label: 'Tout sélectionner', keys: ['Ctrl+A'] },
  { id: 'select.none', menu: 'edit', section: 'Sélection', label: 'Désélectionner', keys: ['Escape'] },
  { id: 'select.invert', menu: 'edit', section: 'Sélection', label: 'Inverser la sélection', keys: ['Ctrl+I'] },
  { id: 'select.same', menu: 'edit', section: 'Sélection', label: 'Sélectionner les mêmes modèles', keys: [] },
  { id: 'group.create', menu: 'edit', section: 'Projecteurs', label: 'Grouper la sélection', keys: ['Ctrl+G'] },
  { id: 'fixture.power', menu: 'edit', section: 'Projecteurs', label: 'Allumer / éteindre la sélection', keys: [] },
  { id: 'fixture.aim', menu: 'edit', section: 'Projecteurs', label: 'Viser le personnage', keys: [] },
  { id: 'fixture.drop', menu: 'edit', section: 'Projecteurs', label: 'Poser au sol', keys: ['End'] },
  { id: 'dmx.sequence', menu: 'edit', section: 'Adressage DMX', label: 'Adresser la sélection à la suite…', keys: [] },
  { id: 'dmx.readdress', menu: 'edit', section: 'Adressage DMX', label: 'Ré-adressage automatique…', keys: [] },
  { id: 'shortcuts.open', menu: 'edit', section: 'Préférences', label: 'Raccourcis clavier…', keys: ['Ctrl+Alt+K', 'F1'] },

  // ---- Affichage
  { id: 'view.persp', menu: 'view', section: 'Vues', label: 'Vue 3D (perspective)', keys: ['1'] },
  { id: 'view.front', menu: 'view', section: 'Vues', label: 'Vue de face', keys: ['2'] },
  { id: 'view.top', menu: 'view', section: 'Vues', label: 'Vue de dessus', keys: ['3'] },
  { id: 'view.side', menu: 'view', section: 'Vues', label: 'Vue de côté', keys: ['4'] },
  { id: 'view.plot', menu: 'view', section: 'Vues', label: 'Plan de feu 2D', keys: ['P'] },
  { id: 'view.frame', menu: 'view', section: 'Vues', label: 'Cadrer la sélection', keys: ['F'] },
  { id: 'labels.toggle', menu: 'view', section: 'Dans la vue 3D', label: 'Noms des projecteurs', keys: ['N'] },
  { id: 'labels.patch', menu: 'view', section: 'Dans la vue 3D', label: 'Adresses DMX', keys: [] },
  { id: 'env.floor', menu: 'view', section: 'Dans la vue 3D', label: 'Sol', keys: [] },
  { id: 'env.cyc', menu: 'view', section: 'Dans la vue 3D', label: 'Cyclo (fond)', keys: [] },
  { id: 'env.actor', menu: 'view', section: 'Dans la vue 3D', label: 'Personnage', keys: [] },
  { id: 'env.stands', menu: 'view', section: 'Dans la vue 3D', label: 'Pieds de projecteur', keys: [] },
  { id: 'panel.left', menu: 'view', section: 'Panneaux', label: 'Bibliothèque et scène (à gauche)', keys: ['Ctrl+B'] },
  { id: 'panel.right', menu: 'view', section: 'Panneaux', label: 'Inspecteur (à droite)', keys: ['Ctrl+Alt+B'] },
  { id: 'panel.bottom', menu: 'view', section: 'Panneaux', label: 'Panneau du bas', keys: ['Ctrl+J'] },
  { id: 'view.fullscreen', menu: 'view', section: 'Panneaux', label: 'Plein écran', keys: ['F11'] },
  { id: 'menu.focus', menu: 'view', section: 'Panneaux', label: 'Ouvrir la barre de menus', keys: ['F10'] },

  // ---- Outils
  { id: 'tool.translate', menu: 'tools', section: 'Manipulation', label: 'Déplacer (poignée)', keys: ['W'] },
  { id: 'tool.rotate', menu: 'tools', section: 'Manipulation', label: 'Pivoter (poignée)', keys: ['E'] },
  { id: 'snap.toggle', menu: 'tools', section: 'Manipulation', label: 'Aimant (10 cm / 15°)', keys: ['M'] },
  { id: 'library.search', menu: 'tools', section: 'Bibliothèque', label: 'Rechercher dans la bibliothèque', keys: ['Ctrl+F'] },
  { id: 'library.lights', menu: 'tools', section: 'Bibliothèque', label: 'Lumières', keys: [] },
  { id: 'library.stands', menu: 'tools', section: 'Bibliothèque', label: 'Pieds', keys: [] },
  { id: 'library.grip', menu: 'tools', section: 'Bibliothèque', label: 'Grip', keys: [] },
  { id: 'remote.toggle', menu: 'tools', section: 'Test et réseau', label: 'Télécommande infrarouge', keys: ['R'] },
  { id: 'drawer.console', menu: 'tools', section: 'Test et réseau', label: 'Console de test', keys: [] },
  { id: 'drawer.dmx', menu: 'tools', section: 'Test et réseau', label: 'Moniteur DMX', keys: [] },
  { id: 'drawer.log', menu: 'tools', section: 'Test et réseau', label: 'Journal', keys: [] },
  { id: 'drawer.network', menu: 'tools', section: 'Test et réseau', label: 'Réseau (Art-Net, sACN)', keys: [] },
  { id: 'drawer.bluetooth', menu: 'tools', section: 'Test et réseau', label: 'Bluetooth (fausse AsteraBox)', keys: [] },

  // ---- Aide
  { id: 'app.update', menu: 'help', section: 'Aide', label: 'Rechercher une mise à jour…', keys: [] },
  { id: 'app.source', menu: 'help', section: 'Aide', label: 'Code source (GitHub)', keys: [] },
  { id: 'app.issue', menu: 'help', section: 'Aide', label: 'Signaler un problème', keys: [] },
  { id: 'app.donate', menu: 'help', section: 'ARKLIGHT', label: 'Faire un don…', keys: [] },
  { id: 'app.about', menu: 'help', section: 'ARKLIGHT', label: 'À propos d’ARKLIGHT', keys: [] },
];

export const commandById = new Map(COMMANDS.map((c) => [c.id, c]));
export const menuById = new Map(MENUS.map((m) => [m.id, m]));
