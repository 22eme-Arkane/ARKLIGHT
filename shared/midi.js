// Contrôleur MIDI : décodage des messages et correspondances « commande du contrôleur → fonction
// d'ARKLIGHT » (fader de la console, intensité d'un groupe, sélection, commande du menu).
// Partagé : testé côté serveur, utilisé par l'interface (web/js/midi.js).

const NOTE_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];

/**
 * Message MIDI → { type: 'cc' | 'note' | 'program' | 'pitch', channel 1..16, number, value 0..1, on }
 * ou null (horloge, sysex, aftertouch… ignorés).
 */
export function parseMidi(data) {
  if (!data || data.length < 1) return null;
  const status = data[0];
  if (status < 0x80 || status >= 0xf0) return null;
  const kind = status & 0xf0;
  const channel = (status & 0x0f) + 1;
  const d1 = data[1] ?? 0;
  const d2 = data[2] ?? 0;
  switch (kind) {
    case 0x80:
      return { type: 'note', channel, number: d1, value: 0, on: false };
    case 0x90:
      return { type: 'note', channel, number: d1, value: d2 / 127, on: d2 > 0 };
    case 0xb0:
      return { type: 'cc', channel, number: d1, value: d2 / 127, on: d2 >= 64 };
    case 0xc0:
      return { type: 'program', channel, number: d1, value: 1, on: true };
    case 0xe0:
      return { type: 'pitch', channel, number: 0, value: ((d2 << 7) | d1) / 16383, on: true };
    default:
      return null;
  }
}

/** Clé d'une commande du contrôleur (une note et son relâchement ont la même). */
export const sourceKey = (s) => `${s.type}:${s.channel}:${s.type === 'pitch' ? 0 : s.number}`;

/** Fader, molette, pitch bend : une valeur continue (les notes et programmes sont des touches). */
export const isContinuous = (s) => s.type === 'cc' || s.type === 'pitch';

export const noteName = (n) => `${NOTE_NAMES[n % 12]}${Math.floor(n / 12) - 1}`;

/** « CC 7 · canal 1 », « Note 60 (C4) · canal 1 »… */
export function describeSource(s, t = (x, p) => x.replace(/\{(\w+)\}/g, (_, k) => p?.[k] ?? '')) {
  if (!s) return '';
  const ch = t('canal {0}', { 0: s.channel });
  if (s.type === 'cc') return `CC ${s.number} · ${ch}`;
  if (s.type === 'note') return `${t('Note')} ${s.number} (${noteName(s.number)}) · ${ch}`;
  if (s.type === 'program') return `${t('Programme')} ${s.number + 1} · ${ch}`;
  return `Pitch bend · ${ch}`;
}

/**
 * Fonctions qu'une correspondance peut commander, par rubrique.
 * mode : 'continuous' (suit un fader), 'button' (à l'appui), 'momentary' (tant qu'on appuie),
 * 'latch' (suit l'état d'une touche à bascule : allumée / éteinte).
 */
export const TARGET_KINDS = [
  { id: 'fader', group: 'Console', label: 'Fader de la console', mode: 'continuous' },
  { id: 'pageNext', group: 'Console', label: 'Faders suivants (page +1)', mode: 'button' },
  { id: 'pagePrev', group: 'Console', label: 'Faders précédents (page −1)', mode: 'button' },
  { id: 'familyNext', group: 'Console', label: 'Modèle suivant (groupe de plusieurs modèles)', mode: 'button' },
  { id: 'flash', group: 'Console', label: 'Flash : plein feu tant qu’on appuie', mode: 'momentary' },
  { id: 'blackout', group: 'Console', label: 'Blackout : tout dans le noir (touche allumée)', mode: 'latch' },
  { id: 'selectionLevel', group: 'Intensité', label: 'Intensité de la sélection', mode: 'continuous' },
  { id: 'groupLevel', group: 'Intensité', label: 'Intensité d’un groupe', mode: 'continuous' },
  { id: 'fixtureLevel', group: 'Intensité', label: 'Intensité d’un projecteur', mode: 'continuous' },
  { id: 'selCct', group: 'Couleur de la sélection', label: 'Kelvin (2000–10 000 K)', mode: 'continuous' },
  { id: 'selTint', group: 'Couleur de la sélection', label: 'Teinte ±vert', mode: 'continuous' },
  { id: 'selHue', group: 'Couleur de la sélection', label: 'Couleur (teinte 0–360°)', mode: 'continuous' },
  { id: 'selSat', group: 'Couleur de la sélection', label: 'Saturation', mode: 'continuous' },
  { id: 'group', group: 'Sélection', label: 'Sélectionner un groupe', mode: 'button' },
  { id: 'fixture', group: 'Sélection', label: 'Sélectionner un projecteur', mode: 'button' },
  { id: 'groupIndex', group: 'Sélection', label: 'Sélectionner le groupe n°', mode: 'button' },
  { id: 'fixtureIndex', group: 'Sélection', label: 'Sélectionner le projecteur n° (liste Scène)', mode: 'button' },
  { id: 'planIndex', group: 'Sélection', label: 'Ouvrir le plan n° du projet', mode: 'button' },
  { id: 'fixtureLatch', group: 'Sélection', label: 'Projecteur n° dans la sélection (touche à bascule)', mode: 'latch' },
  { id: 'groupLatch', group: 'Sélection', label: 'Groupe n° dans la sélection (touche à bascule)', mode: 'latch' },
  { id: 'groupNext', group: 'Sélection', label: 'Groupe suivant', mode: 'button' },
  { id: 'groupPrev', group: 'Sélection', label: 'Groupe précédent', mode: 'button' },
  { id: 'fixtureNext', group: 'Sélection', label: 'Projecteur suivant', mode: 'button' },
  { id: 'fixturePrev', group: 'Sélection', label: 'Projecteur précédent', mode: 'button' },
  { id: 'haze', group: 'Plateau', label: 'Brume', mode: 'continuous' },
  { id: 'exposure', group: 'Plateau', label: 'Exposition de la caméra (−10 à +3 IL)', mode: 'continuous' },
  { id: 'ambient', group: 'Plateau', label: 'Lumière ambiante', mode: 'continuous' },
  { id: 'command', group: 'Commandes', label: 'Commande d’ARKLIGHT', mode: 'button' },
];
export const TARGET_GROUPS = [...new Set(TARGET_KINDS.map((k) => k.group))];
export const kindOf = (id) => TARGET_KINDS.find((k) => k.id === id) || null;

/** Valeur d'un fader (0..1) dans les unités d'une fonction continue. */
export function scaleValue(kind, v) {
  switch (kind) {
    case 'selCct':
      return Math.round((2000 + v * 8000) / 50) * 50;
    case 'selTint':
      return Math.round((v * 2 - 1) * 100) / 100;
    case 'selHue':
      return Math.round(v * 360);
    case 'exposure':
      return Math.round((-10 + v * 13) * 10) / 10; // en IL
    case 'ambient':
      return Math.round(v * 0.5 * 1000) / 1000;
    default:
      return v;
  }
}

/**
 * Ce que déclenche un message reçu :
 * - fonctions continues (faders) : [{ mapping, value }] ;
 * - touches : [{ mapping, press: true }] à l'appui (note jouée, programme, CC qui passe au-dessus de la
 *   moitié), et [{ mapping, release: true }] au relâchement pour une fonction « tant qu'on appuie » ;
 * - touches à bascule (console qui garde ses touches allumées) : [{ mapping, latch: true | false }] quand
 *   la touche s'allume ou s'éteint. Une note rejouée sans avoir été relâchée éteint la touche (consoles
 *   qui n'envoient qu'une note à chaque appui).
 * `held` garde l'état des touches entre deux messages ({ down, latched } par commande).
 */
export function routeMidi(mappings, msg, held = new Map()) {
  const key = sourceKey(msg);
  const out = [];
  const list = mappings.filter((m) => m.source && sourceKey(m.source) === key);
  if (!list.length) return out;
  const st = held.get(key) || { down: false, latched: false };
  const now = msg.type === 'program' ? true : msg.on;
  // appui : une note jouée (même rejouée), un programme, un CC qui monte
  const pressed = msg.type === 'program' || (now && (!st.down || msg.type === 'note'));
  const released = !now && st.down;
  const latched = msg.type === 'program' || (msg.type === 'note' && now && st.down) ? !st.latched : now;
  held.set(key, { down: msg.type === 'program' ? false : now, latched });
  for (const m of list) {
    const kind = kindOf(m.target?.kind);
    if (!kind) continue;
    if (kind.mode === 'continuous') out.push({ mapping: m, value: msg.type === 'note' ? (msg.on ? 1 : 0) : msg.value });
    else if (kind.mode === 'latch') {
      if (latched !== st.latched) out.push({ mapping: m, latch: latched });
    } else if (pressed) out.push({ mapping: m, press: true });
    else if (kind.mode === 'momentary' && released) out.push({ mapping: m, release: true });
  }
  return out;
}

/** Correspondances lues (stockage) : on ne garde que ce qui est bien formé. */
export function cleanMappings(list) {
  if (!Array.isArray(list)) return [];
  return list
    .filter((m) => m && typeof m === 'object' && m.target && TARGET_KINDS.some((k) => k.id === m.target.kind))
    .map((m, i) => ({
      id: typeof m.id === 'string' ? m.id : `m${Date.now().toString(36)}${i}`,
      source: m.source && ['cc', 'note', 'program', 'pitch'].includes(m.source.type) ? { type: m.source.type, channel: Math.min(16, Math.max(1, m.source.channel | 0)), number: Math.min(127, Math.max(0, m.source.number | 0)) } : null,
      target: { ...m.target },
      ...(typeof m.label === 'string' ? { label: m.label.slice(0, 12) } : {}),
    }));
}
