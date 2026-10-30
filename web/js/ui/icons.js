// Icônes des barres d'outils (vue 3D et plan 2D) : traits fins, couleur du texte, 24 × 24.
// Les faces pleines des cubes de vue montrent le côté regardé (face, dessus, côté).

// cube isométrique : sommet T, côtés droit R1/R2, bas B, côtés gauche L1/L2, centre C
const CUBE = '<path d="M12 3l8 4.5v9L12 21l-8-4.5v-9z"/><path d="M12 12l8-4.5M12 12v9M12 12L4 7.5"/>';
const face = (d) => `<path d="${d}" fill="currentColor" fill-opacity=".38" stroke="none"/>`;

const ICONS = {
  move: '<path d="M12 3v18M3 12h18"/><path d="M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3"/>',
  rotate: '<path d="M20 12a8 8 0 1 1-2.3-5.7"/><path d="M20 4v5h-5"/>',
  magnet: '<path d="M5 4h4v7a3 3 0 0 0 6 0V4h4v7a7 7 0 0 1-14 0z"/><path d="M5 8h4M15 8h4"/>',
  cube: CUBE,
  'view-front': face('M4 7.5L12 12v9l-8-4.5z') + CUBE,
  'view-top': face('M12 3l8 4.5-8 4.5-8-4.5z') + CUBE,
  'view-side': face('M12 12l8-4.5v9L12 21z') + CUBE,
  camera: '<rect x="3" y="7" width="12" height="10" rx="2"/><path d="M15 10.5l6-3v9l-6-3z"/>',
  focus: '<circle cx="12" cy="12" r="3"/><path d="M3 8V5a2 2 0 0 1 2-2h3M16 3h3a2 2 0 0 1 2 2v3M21 16v3a2 2 0 0 1-2 2h-3M8 21H5a2 2 0 0 1-2-2v-3"/>',
  eye: '<path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
  layers: '<path d="M12 3l9 5-9 5-9-5z"/><path d="M3 12.5l9 5 9-5"/><path d="M3 16.5l9 5 9-5"/>',
  remote: '<rect x="8" y="2.5" width="8" height="19" rx="2"/><circle cx="12" cy="7" r="1.4"/><path d="M10.5 11.5h3M10.5 14.5h3M10.5 17.5h3"/>',
  haze: '<path d="M3 8.5c2-1.5 4-1.5 6 0s4 1.5 6 0 4-1.5 6 0M3 13.5c2-1.5 4-1.5 6 0s4 1.5 6 0 4-1.5 6 0M5 18.5h14"/>',
  exposure: '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 21L21 3"/><path d="M6 8.5h4M8 6.5v4M14 15.5h4"/>',
  ambient: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2.5M12 19.5V22M4.9 4.9l1.8 1.8M17.3 17.3l1.8 1.8M2 12h2.5M19.5 12H22M4.9 19.1l1.8-1.8M17.3 6.7l1.8-1.8"/>',
  fullscreen: '<path d="M3 9V3h6M21 9V3h-6M3 15v6h6M21 15v6h-6"/>',
  plan: '<path d="M9 4L3 6v14l6-2 6 2 6-2V4l-6 2z"/><path d="M9 4v14M15 6v14"/>',
  grid: '<path d="M4 4h16v16H4z"/><path d="M4 9.33h16M4 14.67h16M9.33 4v16M14.67 4v16" stroke-opacity=".7"/>',
  'chevron-left': '<path d="M15 5l-7 7 7 7"/>',
  'chevron-right': '<path d="M9 5l7 7-7 7"/>',
  'chevron-up': '<path d="M5 15l7-7 7 7"/>',
  'chevron-down': '<path d="M5 9l7 7 7-7"/>',
  // plan de feu 2D
  select: '<path d="M6 3l12 9-5.5 1L10 19z"/>',
  rect: '<rect x="4" y="6" width="16" height="12" rx="1"/>',
  ellipse: '<ellipse cx="12" cy="12" rx="8.5" ry="6.5"/>',
  polygon: '<path d="M12 3.5l8 6-3 10H7l-3-10z"/>',
  freehand: '<path d="M4 20l4-1L19 8l-3-3L5 16z"/><path d="M14 7l3 3"/>',
  line: '<path d="M5 19L19 5"/>',
  arrow: '<path d="M4 12h15M14 7l5 5-5 5"/>',
  text: '<path d="M5 7V4h14v3M12 4v16M9 20h6"/>',
  measure: '<path d="M4 12h16M7 9l-3 3 3 3M17 9l3 3-3 3"/><path d="M4 6v12M20 6v12" stroke-opacity=".5"/>',
  fit: '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/><rect x="8.5" y="8.5" width="7" height="7" rx="1"/>',
  image: '<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="8.5" cy="10" r="1.5"/><path d="M21 16l-5-5-8 8"/>',
  download: '<path d="M12 3v12M7 10l5 5 5-5"/><path d="M4 17v3h16v-3"/>',
  // projets
  plus: '<path d="M12 5v14M5 12h14"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  'folder-open': '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v1"/><path d="M3 19l2.5-8h16L19 19z"/>',
  sort: '<path d="M7 4v16M4 7l3-3 3 3"/><path d="M17 20V4M14 17l3 3 3-3"/>',
  search: '<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4.5 4.5"/>',
  dots: '<circle cx="5" cy="12" r="1.3" fill="currentColor"/><circle cx="12" cy="12" r="1.3" fill="currentColor"/><circle cx="19" cy="12" r="1.3" fill="currentColor"/>',
  close: '<path d="M6 6l12 12M18 6L6 18"/>',
  photo: '<rect x="3" y="5" width="18" height="14" rx="2"/><circle cx="8.5" cy="10" r="1.6"/><path d="M21 16l-5-5-8 8"/>',
  mic: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21M8.5 21h7"/>',
  sparkle: '<path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z"/><path d="M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z"/>',
};

/** Code SVG d'une icône (pour les gabarits HTML). */
export function iconSvg(name, size = 18) {
  return `<svg class="ico" viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name] || ''}</svg>`;
}

/** Icône prête à insérer dans un bouton. */
export function icon(name, size = 18) {
  const t = document.createElement('template');
  t.innerHTML = iconSvg(name, size);
  return t.content.firstChild;
}

/** Remplace les <i data-icon="…"> de la page par leurs icônes. */
export function hydrateIcons(root = document) {
  for (const el of root.querySelectorAll('i[data-icon]')) el.replaceWith(icon(el.dataset.icon, Number(el.dataset.size) || 18));
}
