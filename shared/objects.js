// Objets plaçables de la scène : grip (cadres, drapeaux, pieds…) et décor (personnages, animaux,
// véhicules, mobilier…). Ils partagent la liste « objects » de la scène et les mêmes outils
// (déplacer, pivoter, dupliquer, annuler).
import { cleanGripProps, defaultGripProps, getGripType } from './grip.js';
import { cleanPropProps, defaultPropProps, getPropType } from './props.js';

/** Type d'un objet (grip ou décor), ou null. */
export const getObjectType = (id) => getGripType(id) || getPropType(id);

/** Objet de décor (personnage, véhicule, meuble…) plutôt que du grip. */
export const isProp = (type) => !!type?.cat;

export function defaultObjectProps(type) {
  return isProp(type) ? defaultPropProps(type) : defaultGripProps(type);
}

/** Réglages valides d'un objet : ceux de son type (décor), ou ceux du grip (pied proposé pour ce cadre…). */
export function cleanObjectProps(type, props) {
  return isProp(type) ? cleanPropProps(type, props) : cleanGripProps(type, props);
}
