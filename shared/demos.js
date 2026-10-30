// Projets d'exemple proposés sur l'écran « Projets » et dans le menu Fichier (construits par le serveur).
export const DEMOS = [
  { id: 'night', name: 'Exemple — tournage de nuit', note: 'Rue et terrasse de café, la nuit : rebond d’un M18 dans un 20×20, contres, lune, practicals, véhicules, équipe caméra.' },
  { id: 'clip', name: 'Exemple — clip en studio', note: 'Salon rétro posé sur un cyclo blanc : tubes en arc, couleurs saturées, brume, contres en faisceaux, voiture, danseurs. Deux séquences, trois plans.' },
];

export const getDemo = (id) => DEMOS.find((d) => d.id === id) || DEMOS[0];
