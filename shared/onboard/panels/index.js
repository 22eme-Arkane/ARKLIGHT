// Panneau de commande simulé de chaque projecteur « non Astera » (voir ../panel.js et
// docs/recherche/menus/ : arri.md, aputure_hive.md, dedo_dmg_kino_litegear.md).
import { broadcaster, lseries, skypanel } from './arri.js';
import { aputureOled, aputurePro, nova } from './aputure.js';
import { hiveC, superHornet } from './hive.js';
import { dmgMix } from './dmg.js';
import { kinoCeleb201, kinoTrueMatch } from './kinoflo.js';
import { litedimmerHybrid, litedimmerPlus } from './litegear.js';
import { dedolight } from './dedolight.js';

const BY_TYPE = {
  'arri.skypanel-s30c': skypanel,
  'arri.skypanel-s60c': skypanel,
  'arri.skypanel-s120c': skypanel,
  'arri.skypanel-s360c': skypanel,
  'arri.l5-c': lseries,
  'arri.l7-c': lseries,
  'arri.l10-c': lseries,
  'arri.broadcaster-2-plus': broadcaster,
  'aputure.ls-600d-pro': aputurePro,
  'aputure.ls-600x-pro': aputurePro,
  'aputure.ls-1200d-pro': aputurePro,
  'aputure.ls-600d': aputureOled,
  'aputure.ls-300x': aputureOled,
  'aputure.c300d': aputureOled,
  'aputure.nova-p300c': nova,
  'hive.wasp-100-c': hiveC,
  'hive.hornet-200-c': hiveC,
  'hive.super-hornet-575-c': superHornet,
  'dmg-lumiere.sl1-mix': dmgMix,
  'dmg-lumiere.mini-mix': dmgMix,
  'dmg-lumiere.maxi-mix': dmgMix,
  'kino-flo.celeb-250-led-dmx': kinoTrueMatch,
  'kino-flo.celeb-450-led-dmx': kinoTrueMatch,
  'kino-flo.celeb-led-201-dmx': kinoCeleb201,
  'litegear.litemat-plus-2': litedimmerPlus,
  'litegear.litemat-plus-4': litedimmerPlus,
  'litegear.s2-litemat-2': litedimmerHybrid,
  'litegear.s2-litemat-4': litedimmerHybrid,
  'dedolight.dled4-bi': dedolight,
  'dedolight.dled7-bi': dedolight,
};

/** Définition du panneau d'un modèle, ou null. */
export function panelFor(type) {
  return (type && BY_TYPE[type.id]) || null;
}

/** Modèles qui ont un panneau simulé (tests, documentation). */
export const PANEL_TYPES = Object.keys(BY_TYPE);
