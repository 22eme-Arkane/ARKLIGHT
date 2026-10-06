// Profils DMX Astera : conversion des tables extraites des documents officiels « DMX Profiles »
// (Titan/AX2-100, Helios/AX2-50, Hyperion, AX1, HydraPanel) vers le format de modes du simulateur.
import FAMILIES from './data/astera-dmx-families.js';
import INDEX_LIST from './data/astera-index-colors.js';
import { buildIndexColors } from '../engine/gels.js';

/** Couleurs index Astera : valeur DMX = numéro du gel LEE. */
export const ASTERA_INDEX_COLORS = buildIndexColors(INDEX_LIST);

/** Jeton du document -> descripteur de canal. `null` = octet de poids faible déjà consommé. */
function tokenDef(token, px) {
  const slotMatch = String(token).match(/^(R|G|B|Idx)([1-4])$/);
  if (slotMatch && px === -2) {
    // couleurs des modes d'effet (Idx1..4, R1/G1/B1..)
    const [, t, n] = slotMatch;
    if (t === 'Idx') return { f: 'fxIndex', slot: Number(n), map: 'asteraIdx', label: `Couleur index ${n}` };
    return { f: { R: 'fxR', G: 'fxG', B: 'fxB' }[t], slot: Number(n), label: `${{ R: 'Rouge', G: 'Vert', B: 'Bleu' }[t]} ${n}` };
  }
  switch (token) {
    case 'R':
      return { f: 'red' };
    case 'G':
      return { f: 'green' };
    case 'B':
      return { f: 'blue' };
    case 'A':
      return { f: 'amber' };
    case 'W':
    case 'W(emu)':
      return { f: 'white', label: 'Blanc émulé' };
    case 'Dim':
      return { f: 'dimmer' };
    case 'Dim16.hi':
      return { f: 'dimmer', bytes: 2 };
    case 'Strobe':
      return { f: 'strobe', map: 'asteraStrobe' };
    case 'Strobe(all)':
      return { f: 'strobe', map: 'asteraStrobe', px: -1, label: 'Strobe (tout)' };
    case 'CCT':
      return { f: 'cct', map: 'asteraCct' };
    case 'CCT_IND':
      return { f: 'cctIndex', map: 'asteraCctInd', label: 'Kelvin (prioritaire)' };
    case 'GM':
      return { f: 'tint', map: 'asteraGM', label: 'Vert/Magenta' };
    case 'CRO':
      return px === -2 ? { f: 'fxFade', label: 'Fondu effet' } : { f: 'crossfade', label: 'Crossfade K→RGB' };
    case 'Hue':
      return { f: 'hue', range: [0, 360] };
    case 'Hue16.hi':
      return { f: 'hue', bytes: 2, range: [0, 360] };
    case 'Sat':
      return { f: 'sat' };
    case 'X16.hi':
      return { f: 'x', bytes: 2, range: [0, 0.8] };
    case 'Y16.hi':
      return { f: 'y', bytes: 2, range: [0, 0.8] };
    case 'Idx':
      return { f: 'index', map: 'asteraIdx', label: 'Couleur index (gel)' };
    case 'Program':
      return { f: 'fxProgram', map: 'raw', label: 'Programme' };
    case 'Speed':
      return { f: 'fxSpeed', label: 'Vitesse' };
    case 'Direction':
      return { f: 'fxDirection', label: 'Sens' };
    case 'Size':
      return { f: 'fxSize', label: 'Taille' };
    case 'Offset':
      return { f: 'fxOffset', label: 'Décalage' };
    case 'Restart':
      return { f: 'fxRestart', map: 'raw', label: 'Redémarrer' };
    case 'Dim16.lo':
    case 'Hue16.lo':
    case 'X16.lo':
    case 'Y16.lo':
      return null;
    case '-':
    default:
      return { f: 'none' };
  }
}

/** Déplie un mode du document en liste de canaux avec le pixel de chaque canal. */
function expandMode(m) {
  const perChannel = new Map(); // canal (1..) -> { token, px }
  if (m.chans) {
    const effect = m.px === null;
    m.chans.forEach((t, i) => perChannel.set(i + 1, { token: t, px: effect ? -2 : -1 }));
  } else {
    for (const [first, stride, fields, trail] of m.groups || []) {
      for (let p = 1; p <= m.px; p++) {
        fields.forEach((t, i) => {
          if (t === null || t === undefined) return;
          if (trail && p === m.px && i === fields.length - 1) return;
          perChannel.set(first + stride * (p - 1) + i, { token: t, px: m.px === 1 ? -1 : p - 1 });
        });
      }
    }
    for (const [ch, t] of Object.entries(m.fixed || {})) perChannel.set(Number(ch), { token: t, px: -1 });
  }
  const channels = [];
  for (let c = 1; c <= m.ch; c++) {
    const e = perChannel.get(c) || { token: '-', px: -1 };
    const def = tokenDef(e.token, e.px);
    if (def === null) continue; // octet fin d'un canal 16 bits
    const px = e.px === -2 ? -1 : def.px !== undefined ? def.px : e.px;
    channels.push({ ...def, px });
  }
  return channels;
}

const STROBE_LABEL = { off: 'OFF', on: 'ON', single: 'SINGLE', multiple: 'MULTIPLE' };

/**
 * Modes d'une famille Astera, au format du simulateur.
 * id = numéro de profil Astera (celui de « PROFILE BY NUM »).
 */
export function asteraModes(familyKey) {
  const fam = FAMILIES[familyKey];
  if (!fam) throw new Error(`Famille DMX Astera inconnue : ${familyKey}`);
  return fam.modes
    .map((m) => ({
      id: String(m.id),
      number: m.id,
      name: m.name,
      pixels: m.px ?? 'effet',
      strobe: m.strobe ? STROBE_LABEL[m.strobe] : null,
      label: m.px === null ? `${m.id}: ${m.name}` : `${m.id}: ${m.name} (PIXEL=${m.px}; STROBE=${STROBE_LABEL[m.strobe] || '—'})`,
      channels: expandMode(m),
    }))
    .sort((a, b) => a.number - b.number);
}

/**
 * Profils « FAN » (144–177) des projecteurs ventilés (QuikPunch, QuikBeam, Pluto, Leo) : le profil de
 * base suivi d'un dernier canal « Fan Control » (document « DMX Profiles for QuikBeam, QuikPunch,
 * PlutoFresnel and LeoFresnel », V4, 2025). Les profils XY 168–175 ne sont pas intégrés (structure inconnue).
 */
const FAN_OF = {
  1: 144, 2: 145, 3: 146, 4: 147, 5: 148, 6: 149, 7: 150, 89: 151, 90: 152, 91: 153, 92: 154, 93: 155,
  8: 156, 9: 157, 10: 158, 11: 159, 12: 160, 13: 161, 14: 162, 94: 163, 95: 164, 137: 165, 96: 166, 97: 167,
  15: 176, 16: 177,
};

export function withFanModes(baseModes) {
  const fan = [];
  for (const m of baseModes) {
    const n = FAN_OF[m.number];
    if (!n) continue;
    fan.push({
      ...m,
      id: String(n),
      number: n,
      name: `${m.name} FAN`,
      fan: true,
      baseId: m.id,
      label: m.label.replace(/^\d+: [^(]+/, `${n}: ${m.name} FAN `).replace(/\s+\(/, ' ('),
      channels: [...m.channels, { f: 'fan', map: 'asteraFan', px: -1, label: 'Ventilateur' }],
    });
  }
  return [...baseModes, ...fan].sort((a, b) => a.number - b.number);
}

export function asteraFamilyInfo(familyKey) {
  const f = FAMILIES[familyKey];
  return { title: f.title, date: f.date, url: f.url, pixelCounts: f.pixelCounts };
}
