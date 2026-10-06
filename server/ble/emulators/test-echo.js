// Émulateur de test : vérifie toute la chaîne téléphone -> Bluetooth -> PC avec une appli générique
// (nRF Connect, LightBlue). Chaque écriture est renvoyée en notification préfixée par "ECHO:".

const SERVICE = 'a7c1a000-0000-4c1e-8000-41524b4c4954';
const RX = 'a7c1a001-0000-4c1e-8000-41524b4c4954';
const TX = 'a7c1a002-0000-4c1e-8000-41524b4c4954';

export const testEcho = {
  name: 'Test (écho)',
  note: 'Pour vérifier la connexion avec nRF Connect ou LightBlue : écris dans la caractéristique …a001, la réponse arrive sur …a002.',
  profile: {
    name: 'ARKLIGHT Test',
    services: [
      {
        uuid: SERVICE,
        advertise: true,
        characteristics: [
          { uuid: RX, props: ['write', 'writeNoResp'], description: 'Commande' },
          { uuid: TX, props: ['read', 'notify'], value: Buffer.from('ARKLIGHT').toString('hex'), description: 'Réponse' },
        ],
      },
    ],
  },
  onWrite(evt, api) {
    const text = evt.bytes.toString('utf8');
    api.log(`reçu ${evt.value} (« ${text.replace(/[^\x20-\x7e]/g, '.')} »)`);
    api.notify(TX, Buffer.concat([Buffer.from('ECHO:'), evt.bytes]));
  },
};
