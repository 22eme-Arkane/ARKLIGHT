// Préréglages MIDI tout prêts, par console ou contrôleur : on branche, on choisit le préréglage, tout
// répond. Un préréglage est une « couche » : les correspondances apprises à la main passent avant lui.
//
// Ajouter une console : une entrée dans MIDI_PRESETS, avec
//   id       identifiant stable (enregistré sur le poste)
//   brand, name, variant  (variant : version du logiciel de la console, mode…)
//   match    noms d'appareil MIDI (USB) qui font proposer ce préréglage, ou null
//   source   d'où viennent les numéros (manuel, page, version)
//   options  réglages proposés au chargement : { channel } = premier canal MIDI de la console
//   summary  ce que fait le préréglage, en quelques lignes (affiché dans la fenêtre MIDI)
//   build(options) → correspondances [{ source: { type, channel, number }, target, label? }]
// Les aides range()/cc()/note() ci-dessous évitent d'écrire chaque commande une à une.

/** Une suite de commandes : n commandes à partir de first, chacune vers target(i) (i = 0, 1, 2…). */
function range(type, channel, first, count, target, label) {
  return Array.from({ length: count }, (_, i) => ({ source: { type, channel, number: first + i }, target: target(i), ...(label ? { label: label(i) } : {}) }));
}
const cc = (channel, first, count, target, label) => range('cc', channel, first, count, target, label);
const note = (channel, first, count, target, label) => range('note', channel, first, count, target, label);

// ------------------------------------------------------------------ ADJ DMX Operator 384
// Mode MIDI : interrupteur DMX / MIDI à l'arrière, USB. Manuel ADJ, version 4 (16/08/2024), pages
// « MIDI Control » et « MIDI Implementation Chart ». Canal : Midi/Bkc maintenu + UP/DOWN.
// En V2.6, 3 canaux à la suite : banques 1 à 8 sur le 1er canal (faders), banques 1 à 4 pour les touches.
// Chaque banque a ses propres numéros : la touche BANK UP/DOWN de la console fait office de page.

function operator384v26({ channel = 1 } = {}) {
  const ch = channel;
  return [
    // faders 1 à 16 de chaque banque (8 faders, pages A/B) : banque 1 = CC 0–15 … banque 8 = CC 112–125
    ...cc(ch, 0, 126, (i) => ({ kind: 'fader', index: i + 1 }), (i) => `B${Math.floor(i / 16) + 1}·${(i % 16) + 1}`),
    // faders Speed et Fade (CC 126 / 127 d'après la table d'implémentation, CC 32 / 33 du 2e canal d'après la table de commande)
    { source: { type: 'cc', channel: ch, number: 126 }, target: { kind: 'selectionLevel' } },
    { source: { type: 'cc', channel: ch + 1, number: 32 }, target: { kind: 'selectionLevel' } },
    { source: { type: 'cc', channel: ch, number: 127 }, target: { kind: 'selCct' } },
    { source: { type: 'cc', channel: ch + 1, number: 33 }, target: { kind: 'selCct' } },
    // touches Fixture 1–12 (à bascule) : notes 80–127 (banques 1 à 4) → projecteurs 1 à 48 de la liste
    // Scène : les touches allumées forment la sélection (de quoi composer un groupe)
    ...note(ch, 80, 48, (i) => ({ kind: 'fixtureLatch', index: i + 1 })),
    // touches Scene 1–8 (poussoirs) : notes 48–79 (banques 1 à 4) → plans 1 à 32 du projet
    ...note(ch, 48, 32, (i) => ({ kind: 'planIndex', index: i + 1 })),
    // touches Chase 1–12 (à bascule) : notes 0–47 (banques 1 à 4) → groupes 1 à 48 ajoutés à la sélection
    ...note(ch, 0, 48, (i) => ({ kind: 'groupLatch', index: i + 1 })),
  ];
}

// Logiciels avant V2.5 : un seul canal, numéros différents ; on reprend la page 1 pour les touches.
function operator384old({ channel = 1 } = {}) {
  const ch = channel;
  return [
    // pages 1 à 8 : faders 1–16 = CC 0–127 → faders 1 à 128 de la console
    ...cc(ch, 0, 128, (i) => ({ kind: 'fader', index: i + 1 }), (i) => `P${Math.floor(i / 16) + 1}·${(i % 16) + 1}`),
    ...note(ch, 20, 12, (i) => ({ kind: 'fixtureLatch', index: i + 1 })),
    ...note(ch, 12, 8, (i) => ({ kind: 'planIndex', index: i + 1 })),
    ...note(ch, 0, 12, (i) => ({ kind: 'groupLatch', index: i + 1 })),
  ];
}

export const MIDI_PRESETS = [
  {
    id: 'adj-dmx-operator-384',
    brand: 'ADJ',
    name: 'DMX Operator 384',
    variant: 'logiciel V2.6 et suivants',
    match: /operator|dmx ?384|adj/i,
    source: 'Manuel ADJ DMX Operator 384, version 4 (16/08/2024) : MIDI Control et MIDI Implementation Chart (V2.6)',
    options: { channel: 1 },
    summary: [
      'Faders 1 à 16 de chaque banque → faders de la console : banque 1 = canaux 1 à 16, banque 2 = 17 à 32… (BANK UP/DOWN sert de page)',
      'Touches Fixture (à bascule) : chaque touche allumée met son projecteur dans la sélection — projecteurs 1 à 12 de la liste Scène (banque 2 : 13 à 24…). Ctrl+G : en faire un groupe',
      'Touches Chase (à bascule) : chaque touche allumée ajoute son groupe à la sélection — groupes 1 à 12 (banque 2 : 13 à 24…)',
      'Touches Scene 1 à 8 → plans 1 à 8 du projet (banque 2 : 9 à 16…)',
      'Fader Speed → intensité de la sélection · fader Fade → Kelvin de la sélection',
      'Touche Blackout : « Apprendre une commande », appuie dessus, puis choisis « Blackout »',
    ],
    build: operator384v26,
  },
  {
    id: 'adj-dmx-operator-384-old',
    brand: 'ADJ',
    name: 'DMX Operator 384',
    variant: 'logiciel avant V2.5',
    match: null,
    source: 'Manuel ADJ DMX Operator 384, version 4 : MIDI Control, « for software revisions prior to V2.5 »',
    options: { channel: 1 },
    summary: [
      'Faders 1 à 16 de chaque page → faders de la console : page 1 = canaux 1 à 16, page 2 = 17 à 32…',
      'Page 1 : touches Fixture (à bascule) → projecteurs 1 à 12 dans la sélection, Chase (à bascule) → groupes 1 à 12 dans la sélection, Scene → plans 1 à 8',
    ],
    build: operator384old,
  },
];

export const presetById = (id) => MIDI_PRESETS.find((p) => p.id === id) || null;

/** Libellé d'un préréglage : « ADJ DMX Operator 384 — logiciel V2.6 et suivants ». */
export const presetLabel = (p) => `${p.brand ? `${p.brand} ` : ''}${p.name}${p.variant ? ` — ${p.variant}` : ''}`;

/** Correspondances d'un préréglage (canal de base borné pour que tous ses canaux restent de 1 à 16). */
export function presetMappings(preset, options = {}) {
  if (!preset) return [];
  const list = preset.build ? preset.build({ ...preset.options, ...options }) : preset.mappings || [];
  return list
    .filter((m) => m.source.channel >= 1 && m.source.channel <= 16 && m.source.number >= 0 && m.source.number <= 127)
    .map((m, i) => ({ id: `${preset.id}:${i}`, preset: true, ...m }));
}
