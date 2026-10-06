// Émulateur AsteraBox — EN ATTENTE DES CAPTURES.
//
// Le protocole Bluetooth entre l'AsteraApp et l'AsteraBox n'est pas public. Ce module sera rempli
// à partir des captures (journal HCI Android analysé par tools/btsnoop/analyze.mjs) :
//   1. profile : la table GATT exacte de la vraie boîte (UUID, propriétés, valeurs lues)
//   2. onWrite : décodage des commandes de l'appli -> modification des projecteurs virtuels
//   3. réponses / notifications attendues par l'appli (liste des lampes, état, batterie...)

export const asteraBox = {
  name: 'AsteraBox (à compléter)',
  ready: false,
  note: 'Profil inconnu tant que les échanges avec une vraie AsteraBox n’ont pas été capturés (voir docs/CAPTURE_BLUETOOTH.md).',
  profile: { name: 'AsteraBox', services: [] },
  onWrite(evt, api) {
    api.log(`commande non décodée sur ${evt.char} : ${evt.value}`);
  },
};
