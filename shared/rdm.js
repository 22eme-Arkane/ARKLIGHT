// RDM (ANSI E1.20) des projecteurs virtuels : identifiant (UID), informations, modes, adresse,
// description des canaux, identification. Transporté en Art-Net (voir server/dmx/artnet.js) :
// Luminair, une console… découvrent les projecteurs de la scène et peuvent les ré-adresser.
// Messages sans le code de départ 0xCC (comme dans ArtRdm) ; octets en Uint8Array.
import { compileMode } from './engine/dmx.js';
import { getFixtureType, getMode, modeFootprint } from './fixtures/index.js';

/** Fabricant ESTA réservé aux prototypes : aucune appli ne confond un projecteur virtuel avec un vrai modèle. */
export const PROTOTYPE_MANUFACTURER = 0x7ff0;

export const CC = { DISCOVERY: 0x10, DISCOVERY_RESPONSE: 0x11, GET: 0x20, GET_RESPONSE: 0x21, SET: 0x30, SET_RESPONSE: 0x31 };
const RESPONSE = { ACK: 0x00, NACK: 0x02 };
export const NACK = { UNKNOWN_PID: 0x0000, FORMAT_ERROR: 0x0001, UNSUPPORTED_COMMAND_CLASS: 0x0005, DATA_OUT_OF_RANGE: 0x0006, SUB_DEVICE_OUT_OF_RANGE: 0x0009 };
export const PID = {
  SUPPORTED_PARAMETERS: 0x0050,
  DEVICE_INFO: 0x0060,
  DEVICE_MODEL_DESCRIPTION: 0x0080,
  MANUFACTURER_LABEL: 0x0081,
  DEVICE_LABEL: 0x0082,
  SOFTWARE_VERSION_LABEL: 0x00c0,
  DMX_PERSONALITY: 0x00e0,
  DMX_PERSONALITY_DESCRIPTION: 0x00e1,
  DMX_START_ADDRESS: 0x00f0,
  SLOT_INFO: 0x0120,
  SLOT_DESCRIPTION: 0x0121,
  IDENTIFY_DEVICE: 0x1000,
};
const PRODUCT_FIXTURE_FIXED = 0x0101;
const SOFTWARE_VERSION = 0x00010000;

// fonction d'un canal → identifiant de canal E1.20 (table C-2) et description lisible par les applis
const SLOTS = {
  dimmer: [0x0001, 'Dimmer'],
  red: [0x0205, 'Red'],
  green: [0x0206, 'Green'],
  blue: [0x0207, 'Blue'],
  amber: [0x0211, 'Amber'],
  white: [0x0212, 'White'],
  ww: [0x0213, 'Warm White'],
  cw: [0x0214, 'Cool White'],
  uv: [0x0215, 'UV'],
  hue: [0x0216, 'Hue'],
  sat: [0x0217, 'Saturation'],
  cct: [0x0208, 'CCT'],
  strobe: [0x0404, 'Strobe'],
  tint: [0xffff, 'Green/Magenta'],
  lime: [0xffff, 'Lime'],
  mint: [0xffff, 'Mint'],
  cyan: [0xffff, 'Cyan'],
  x: [0xffff, 'CIE x'],
  y: [0xffff, 'CIE y'],
  crossfade: [0xffff, 'Crossfade'],
  index: [0xffff, 'Color Index'],
  cctIndex: [0xffff, 'CCT Index'],
  fan: [0xffff, 'Fan'],
  none: [0xffff, 'Reserved'],
};

/** Texte ASCII (32 caractères au plus) : les applis RDM n'acceptent pas les accents. */
export function ascii(s, max = 32) {
  return String(s ?? '')
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\x20-\x7e]/g, '?')
    .slice(0, max);
}
const textBytes = (s, max = 32) => Uint8Array.from(ascii(s, max), (c) => c.charCodeAt(0));
const fromAscii = (bytes) => String.fromCharCode(...bytes).replace(/\0+$/, '');

/** Identifiant RDM d'un projecteur (stable : tiré de son identifiant dans la scène). */
export function deviceIdOf(fixtureId) {
  let h = 0x811c9dc5;
  for (let i = 0; i < fixtureId.length; i++) h = Math.imul(h ^ fixtureId.charCodeAt(i), 0x01000193);
  h >>>= 0;
  return h === 0xffffffff || h === 0 ? 0x00000001 : h;
}

/** UID en texte « 7FF0:1A2B3C4D ». */
export const uidText = (mfr, dev) => `${mfr.toString(16).toUpperCase().padStart(4, '0')}:${(dev >>> 0).toString(16).toUpperCase().padStart(8, '0')}`;

function uidBytes(mfr, dev) {
  return Uint8Array.of((mfr >> 8) & 0xff, mfr & 0xff, (dev >>> 24) & 0xff, (dev >>> 16) & 0xff, (dev >>> 8) & 0xff, dev & 0xff);
}

/** Modèle RDM : numéro tiré de l'identifiant du modèle dans la bibliothèque. */
function modelIdOf(typeId) {
  let h = 0;
  for (let i = 0; i < typeId.length; i++) h = (Math.imul(h, 31) + typeId.charCodeAt(i)) & 0xffff;
  return h || 1;
}

/** Canaux d'un mode, au sens RDM (16 bits : un canal principal puis un canal « fin »). */
export function slotsOf(mode) {
  const out = [];
  for (const s of compileMode(mode).slots) {
    const [id, name] = SLOTS[s.f] || [0xffff, s.f];
    const base = `${s.def?.label || name}${s.px >= 0 ? ` P${s.px + 1}` : ''}`;
    const at = out.length;
    out.push({ type: 0x00, label: id, text: base });
    if ((s.bytes || 1) === 2) out.push({ type: 0x01, label: at, text: `${base} fine` }); // ST_SEC_FINE : renvoie au canal principal
  }
  return out;
}

const u16 = (v) => Uint8Array.of((v >> 8) & 0xff, v & 0xff);
const u32 = (v) => Uint8Array.of((v >>> 24) & 0xff, (v >>> 16) & 0xff, (v >>> 8) & 0xff, v & 0xff);
const concat = (...parts) => {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
};

/** Lit un message RDM (sans 0xCC) ; null s'il est invalide. */
export function parseRdm(data) {
  if (!data || data.length < 25 || data[0] !== 0x01) return null;
  const pdl = data[22];
  if (data.length < 25 + pdl || data[1] !== 24 + pdl) return null;
  let sum = 0xcc;
  for (let i = 0; i < 23 + pdl; i++) sum += data[i];
  if (((sum & 0xffff) >> 8) !== data[23 + pdl] || (sum & 0xff) !== data[24 + pdl]) return null;
  const mfr = (data[2] << 8) | data[3];
  const dev = ((data[4] << 24) | (data[5] << 16) | (data[6] << 8) | data[7]) >>> 0;
  return {
    dest: { mfr, dev },
    src: data.slice(8, 14),
    tn: data[14],
    port: data[15],
    subDevice: (data[17] << 8) | data[18],
    cc: data[19],
    pid: (data[20] << 8) | data[21],
    pd: data.slice(23, 23 + pdl),
  };
}

/** Construit un message RDM (sans 0xCC). */
export function buildRdm({ dest, src, tn, response = RESPONSE.ACK, subDevice = 0, cc, pid, pd = new Uint8Array(0) }) {
  const pdl = pd.length;
  const b = new Uint8Array(25 + pdl);
  b[0] = 0x01;
  b[1] = 24 + pdl;
  b.set(dest, 2);
  b.set(src, 8);
  b[14] = tn;
  b[15] = response;
  b[16] = 0;
  b[17] = (subDevice >> 8) & 0xff;
  b[18] = subDevice & 0xff;
  b[19] = cc;
  b[20] = (pid >> 8) & 0xff;
  b[21] = pid & 0xff;
  b[22] = pdl;
  b.set(pd, 23);
  let sum = 0xcc;
  for (let i = 0; i < 23 + pdl; i++) sum += b[i];
  b[23 + pdl] = (sum >> 8) & 0xff;
  b[24 + pdl] = sum & 0xff;
  return b;
}

/**
 * Répondeur RDM de la scène.
 *  - getFixtures() : projecteurs de la scène
 *  - onSet(fixture, changes, what) : une appli change l'adresse, le mode ou le nom d'un projecteur
 *  - onIdentify(fixture, on) : une appli demande au projecteur de se signaler
 */
export class RdmResponder {
  constructor({ getFixtures = () => [], onSet = () => {}, onIdentify = () => {} } = {}) {
    this.getFixtures = getFixtures;
    this.onSet = onSet;
    this.onIdentify = onIdentify;
    this.identifying = new Set(); // ids des projecteurs en identification
  }

  /** Projecteurs pilotables en DMX de cet univers, avec leur UID. */
  devices(universe) {
    const out = [];
    const used = new Set();
    for (const f of this.getFixtures()) {
      const type = getFixtureType(f.type);
      if (!type?.dmx?.modes?.length || Number(f.settings?.universe) !== universe) continue;
      let dev = deviceIdOf(f.id);
      while (used.has(dev)) dev = (dev + 1) >>> 0; // deux identifiants identiques : très improbable
      used.add(dev);
      out.push({ f, type, mfr: PROTOTYPE_MANUFACTURER, dev, uid: uidBytes(PROTOTYPE_MANUFACTURER, dev) });
    }
    return out;
  }

  /** Table des appareils (ToD) d'un univers : UID de 6 octets. */
  tod(universe) {
    return this.devices(universe).map((d) => d.uid);
  }

  /** Répond à un message RDM reçu sur cet univers ; null si aucune réponse n'est due. */
  handle(universe, data) {
    const req = parseRdm(data);
    if (!req) return null;
    const broadcast = req.dest.dev === 0xffffffff && (req.dest.mfr === 0xffff || req.dest.mfr === PROTOTYPE_MANUFACTURER);
    const targets = this.devices(universe).filter((d) => broadcast || (d.mfr === req.dest.mfr && d.dev === req.dest.dev));
    if (!targets.length || req.cc === CC.DISCOVERY) return null; // la découverte passe par la ToD d'Art-Net
    let reply = null;
    for (const d of targets) {
      const r = this._answer(d, req);
      if (!broadcast) reply = r;
    }
    return reply;
  }

  _answer(d, req) {
    const ack = (pd) => buildRdm({ dest: req.src, src: d.uid, tn: req.tn, subDevice: req.subDevice, cc: req.cc + 1, pid: req.pid, pd });
    const nack = (reason) => buildRdm({ dest: req.src, src: d.uid, tn: req.tn, response: RESPONSE.NACK, subDevice: req.subDevice, cc: req.cc + 1, pid: req.pid, pd: u16(reason) });
    if (req.subDevice !== 0 && req.subDevice !== 0xffff) return nack(NACK.SUB_DEVICE_OUT_OF_RANGE);
    const { f, type } = d;
    const modes = type.dmx.modes;
    const current = Math.max(0, modes.findIndex((m) => m.id === f.settings.dmxMode));
    const mode = getMode(type, f.settings.dmxMode) || modes[0];
    const footprint = modeFootprint(type, mode.id);
    const get = req.cc === CC.GET;
    const set = req.cc === CC.SET;
    if (!get && !set) return nack(NACK.UNSUPPORTED_COMMAND_CLASS);
    const pd = req.pd;
    switch (req.pid) {
      case PID.SUPPORTED_PARAMETERS:
        if (!get) return nack(NACK.UNSUPPORTED_COMMAND_CLASS);
        return ack(concat(...[PID.DEVICE_MODEL_DESCRIPTION, PID.MANUFACTURER_LABEL, PID.DEVICE_LABEL, PID.DMX_PERSONALITY, PID.DMX_PERSONALITY_DESCRIPTION, PID.SLOT_INFO, PID.SLOT_DESCRIPTION].map(u16)));
      case PID.DEVICE_INFO:
        if (!get) return nack(NACK.UNSUPPORTED_COMMAND_CLASS);
        return ack(concat(u16(0x0100), u16(modelIdOf(type.id)), u16(PRODUCT_FIXTURE_FIXED), u32(SOFTWARE_VERSION), u16(footprint), Uint8Array.of(current + 1, Math.min(255, modes.length)), u16(footprint ? f.settings.dmxAddress : 0xffff), u16(0), Uint8Array.of(0)));
      case PID.DEVICE_MODEL_DESCRIPTION:
        return get ? ack(textBytes(type.name)) : nack(NACK.UNSUPPORTED_COMMAND_CLASS);
      case PID.MANUFACTURER_LABEL:
        return get ? ack(textBytes(type.brand)) : nack(NACK.UNSUPPORTED_COMMAND_CLASS);
      case PID.SOFTWARE_VERSION_LABEL:
        return get ? ack(textBytes('ARKLIGHT 1.0')) : nack(NACK.UNSUPPORTED_COMMAND_CLASS);
      case PID.DEVICE_LABEL:
        if (get) return ack(textBytes(f.name));
        if (pd.length > 32) return nack(NACK.FORMAT_ERROR);
        this.onSet(f, { name: fromAscii(pd).trim() || f.name }, 'name');
        return ack(new Uint8Array(0));
      case PID.DMX_PERSONALITY: {
        if (get) return ack(Uint8Array.of(current + 1, Math.min(255, modes.length)));
        if (pd.length !== 1) return nack(NACK.FORMAT_ERROR);
        const n = pd[0];
        if (n < 1 || n > modes.length) return nack(NACK.DATA_OUT_OF_RANGE);
        this.onSet(f, { settings: { dmxMode: modes[n - 1].id } }, 'mode');
        return ack(new Uint8Array(0));
      }
      case PID.DMX_PERSONALITY_DESCRIPTION: {
        if (!get) return nack(NACK.UNSUPPORTED_COMMAND_CLASS);
        if (pd.length !== 1) return nack(NACK.FORMAT_ERROR);
        const n = pd[0];
        const m = modes[n - 1];
        if (!m) return nack(NACK.DATA_OUT_OF_RANGE);
        return ack(concat(Uint8Array.of(n), u16(modeFootprint(type, m.id)), textBytes(m.label || m.name)));
      }
      case PID.DMX_START_ADDRESS: {
        if (get) return ack(u16(footprint ? f.settings.dmxAddress : 0xffff));
        if (pd.length !== 2) return nack(NACK.FORMAT_ERROR);
        const a = (pd[0] << 8) | pd[1];
        if (a < 1 || a > 512 || a + footprint - 1 > 512) return nack(NACK.DATA_OUT_OF_RANGE);
        this.onSet(f, { settings: { dmxAddress: a } }, 'address');
        return ack(new Uint8Array(0));
      }
      case PID.SLOT_INFO: {
        if (!get) return nack(NACK.UNSUPPORTED_COMMAND_CLASS);
        // une réponse tient 231 octets de données (46 canaux) ; au-delà, les applis demandent SLOT_DESCRIPTION
        const slots = slotsOf(mode).slice(0, 46);
        return ack(concat(...slots.map((s, i) => concat(u16(i), Uint8Array.of(s.type), u16(s.label)))));
      }
      case PID.SLOT_DESCRIPTION: {
        if (!get) return nack(NACK.UNSUPPORTED_COMMAND_CLASS);
        if (pd.length !== 2) return nack(NACK.FORMAT_ERROR);
        const n = (pd[0] << 8) | pd[1];
        const s = slotsOf(mode)[n];
        if (!s) return nack(NACK.DATA_OUT_OF_RANGE);
        return ack(concat(u16(n), textBytes(s.text)));
      }
      case PID.IDENTIFY_DEVICE: {
        if (get) return ack(Uint8Array.of(this.identifying.has(f.id) ? 1 : 0));
        if (pd.length !== 1 || pd[0] > 1) return nack(NACK.FORMAT_ERROR);
        if (pd[0]) this.identifying.add(f.id);
        else this.identifying.delete(f.id);
        this.onIdentify(f, !!pd[0]);
        return ack(new Uint8Array(0));
      }
      default:
        return nack(NACK.UNKNOWN_PID);
    }
  }
}
