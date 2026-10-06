// Lecture des journaux Bluetooth HCI au format btsnoop (Android : "journal HCI snoop Bluetooth").
// Reconstitue : appareils vus en publicité (advertising), connexions LE, table GATT découverte
// par le téléphone, et toutes les opérations ATT (écritures / lectures / notifications).
//
// Rappel de sens : dans un journal Android, "sent" = du téléphone vers l'appareil distant,
// "received" = de l'appareil distant (ex. AsteraBox) vers le téléphone.

const BTSNOOP_MAGIC = 'btsnoop\0';
const EPOCH_DELTA_US = 0x00dcddb30f2f8000n; // microsecondes entre l'an 0 et 1970

export const GATT_NAMES = {
  0x1800: 'Generic Access',
  0x1801: 'Generic Attribute',
  0x180a: 'Device Information',
  0x180f: 'Battery Service',
  0x2800: 'Primary Service',
  0x2801: 'Secondary Service',
  0x2802: 'Include',
  0x2803: 'Characteristic',
  0x2900: 'Characteristic Extended Properties',
  0x2901: 'Characteristic User Description',
  0x2902: 'Client Characteristic Configuration (CCCD)',
  0x2a00: 'Device Name',
  0x2a01: 'Appearance',
  0x2a04: 'Peripheral Preferred Connection Parameters',
  0x2a05: 'Service Changed',
  0x2a19: 'Battery Level',
  0x2a23: 'System ID',
  0x2a24: 'Model Number String',
  0x2a25: 'Serial Number String',
  0x2a26: 'Firmware Revision String',
  0x2a27: 'Hardware Revision String',
  0x2a28: 'Software Revision String',
  0x2a29: 'Manufacturer Name String',
  0x2aa6: 'Central Address Resolution',
  0x2b29: 'Client Supported Features',
  0x2b2a: 'Database Hash',
  0xfe59: 'Nordic Secure DFU',
};

export const KNOWN_128 = {
  '6e400001-b5a3-f393-e0a9-e50e24dcca9e': 'Nordic UART Service (NUS)',
  '6e400002-b5a3-f393-e0a9-e50e24dcca9e': 'NUS RX (écriture)',
  '6e400003-b5a3-f393-e0a9-e50e24dcca9e': 'NUS TX (notification)',
  '8ec90001-f315-4f60-9fb8-838830daea50': 'Nordic Buttonless DFU',
};

const ATT_OPS = {
  0x01: 'ERROR_RSP',
  0x02: 'MTU_REQ',
  0x03: 'MTU_RSP',
  0x04: 'FIND_INFO_REQ',
  0x05: 'FIND_INFO_RSP',
  0x06: 'FIND_BY_TYPE_REQ',
  0x07: 'FIND_BY_TYPE_RSP',
  0x08: 'READ_BY_TYPE_REQ',
  0x09: 'READ_BY_TYPE_RSP',
  0x0a: 'READ_REQ',
  0x0b: 'READ_RSP',
  0x0c: 'READ_BLOB_REQ',
  0x0d: 'READ_BLOB_RSP',
  0x0e: 'READ_MULTI_REQ',
  0x0f: 'READ_MULTI_RSP',
  0x10: 'READ_BY_GROUP_REQ',
  0x11: 'READ_BY_GROUP_RSP',
  0x12: 'WRITE_REQ',
  0x13: 'WRITE_RSP',
  0x16: 'PREPARE_WRITE_REQ',
  0x17: 'PREPARE_WRITE_RSP',
  0x18: 'EXECUTE_WRITE_REQ',
  0x19: 'EXECUTE_WRITE_RSP',
  0x1b: 'NOTIFY',
  0x1d: 'INDICATE',
  0x1e: 'CONFIRM',
  0x20: 'READ_MULTI_VAR_REQ',
  0x21: 'READ_MULTI_VAR_RSP',
  0x23: 'MULTI_NOTIFY',
  0x52: 'WRITE_CMD',
  0xd2: 'SIGNED_WRITE_CMD',
};

const ATT_ERRORS = {
  0x01: 'Invalid Handle',
  0x02: 'Read Not Permitted',
  0x03: 'Write Not Permitted',
  0x05: 'Insufficient Authentication',
  0x06: 'Request Not Supported',
  0x07: 'Invalid Offset',
  0x08: 'Insufficient Authorization',
  0x0a: 'Attribute Not Found',
  0x0d: 'Invalid Attribute Value Length',
  0x0e: 'Unlikely Error',
  0x0f: 'Insufficient Encryption',
};

export function uuidToString(bytes) {
  if (bytes.length === 2) return `0x${bytes.readUInt16LE(0).toString(16).padStart(4, '0')}`;
  if (bytes.length === 4) return `0x${bytes.readUInt32LE(0).toString(16).padStart(8, '0')}`;
  const hex = Buffer.from(bytes).reverse().toString('hex');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export function uuidName(uuid) {
  if (uuid.startsWith('0x') && uuid.length === 6) return GATT_NAMES[parseInt(uuid, 16)] || '';
  return KNOWN_128[uuid] || '';
}

const addrToString = (buf) =>
  [...buf]
    .reverse()
    .map((b) => b.toString(16).padStart(2, '0'))
    .join(':')
    .toUpperCase();

export function hex(buf) {
  return Buffer.from(buf).toString('hex').replace(/(..)/g, '$1 ').trim();
}

export function ascii(buf) {
  return [...buf].map((b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : '.')).join('');
}

const PROPS = ['broadcast', 'read', 'writeNoResp', 'write', 'notify', 'indicate', 'signedWrite', 'extended'];
export function decodeProps(p) {
  return PROPS.filter((_, i) => p & (1 << i));
}

/** Décode les structures AD d'une publicité BLE. */
export function parseAdvertisingData(data) {
  const out = { name: null, flags: null, services: [], manufacturer: [], serviceData: [], txPower: null, raw: hex(data) };
  let i = 0;
  while (i < data.length) {
    const len = data[i];
    if (!len || i + 1 + len > data.length) break;
    const type = data[i + 1];
    const v = data.subarray(i + 2, i + 1 + len);
    switch (type) {
      case 0x01:
        out.flags = v[0];
        break;
      case 0x02:
      case 0x03:
        for (let k = 0; k + 2 <= v.length; k += 2) out.services.push(uuidToString(v.subarray(k, k + 2)));
        break;
      case 0x06:
      case 0x07:
        for (let k = 0; k + 16 <= v.length; k += 16) out.services.push(uuidToString(v.subarray(k, k + 16)));
        break;
      case 0x08:
      case 0x09:
        out.name = v.toString('utf8');
        break;
      case 0x0a:
        out.txPower = (v[0] << 24) >> 24;
        break;
      case 0x16:
        out.serviceData.push({ uuid: uuidToString(v.subarray(0, 2)), data: hex(v.subarray(2)) });
        break;
      case 0x21:
        out.serviceData.push({ uuid: uuidToString(v.subarray(0, 16)), data: hex(v.subarray(16)) });
        break;
      case 0xff:
        out.manufacturer.push({ company: v.length >= 2 ? `0x${v.readUInt16LE(0).toString(16).padStart(4, '0')}` : null, data: hex(v.subarray(2)) });
        break;
      default:
        break;
    }
    i += 1 + len;
  }
  return out;
}

/** Lit un fichier btsnoop : renvoie la liste des paquets HCI. */
export function readBtsnoop(buf) {
  if (buf.toString('latin1', 0, 8) !== BTSNOOP_MAGIC) throw new Error('Fichier non reconnu : ce n’est pas un journal btsnoop.');
  const datalink = buf.readUInt32BE(12);
  if (![1001, 1002].includes(datalink)) throw new Error(`Type de liaison btsnoop non géré : ${datalink}`);
  const packets = [];
  let off = 16;
  while (off + 24 <= buf.length) {
    const incl = buf.readUInt32BE(off + 4);
    const flags = buf.readUInt32BE(off + 8);
    const ts = buf.readBigUInt64BE(off + 16);
    const data = buf.subarray(off + 24, off + 24 + incl);
    off += 24 + incl;
    if (data.length < incl) break;
    const received = (flags & 1) === 1;
    let type;
    let payload;
    if (datalink === 1002) {
      type = data[0];
      payload = data.subarray(1);
    } else {
      type = flags & 2 ? (received ? 0x04 : 0x01) : 0x02;
      payload = data;
    }
    packets.push({ time: Number((ts - EPOCH_DELTA_US) / 1000n), received, type, payload });
  }
  return packets;
}

/**
 * Analyse complète.
 * @param {Buffer} buf  contenu du fichier
 */
export function analyze(buf) {
  const packets = readBtsnoop(buf);
  const devices = new Map(); // adresse -> infos de publicité
  const connections = new Map(); // handle HCI -> { peer, start, end, gatt, ops }
  const allConnections = [];
  const reassembly = new Map(); // `${handle}|${dir}` -> { buf, expected }
  const pendingCreate = []; // adresses visées par LE Create Connection
  const classicNames = new Map(); // adresse -> nom (Bluetooth classique)

  const t0 = packets.length ? packets[0].time : 0;

  const getConn = (handle, time) => {
    let c = connections.get(handle);
    if (!c) {
      c = newConnection(handle, null, time);
      connections.set(handle, c);
      allConnections.push(c);
    }
    return c;
  };

  for (const p of packets) {
    if (p.type === 0x01) {
      // Commande HCI
      const opcode = p.payload.readUInt16LE(0);
      const params = p.payload.subarray(3);
      if (opcode === 0x200d && params.length >= 12) pendingCreate.push(addrToString(params.subarray(6, 12)));
      if (opcode === 0x2043 && params.length >= 10) pendingCreate.push(addrToString(params.subarray(4, 10)));
    } else if (p.type === 0x04) {
      handleEvent(p);
    } else if (p.type === 0x02) {
      handleAcl(p);
    }
  }

  function handleEvent(p) {
    const code = p.payload[0];
    const params = p.payload.subarray(2);
    if (code === 0x03 && params.length >= 11 && params[0] === 0) {
      // Connection Complete (Bluetooth classique)
      const handle = params.readUInt16LE(1) & 0x0fff;
      const c = newConnection(handle, addrToString(params.subarray(3, 9)), p.time);
      c.kind = 'classic';
      connections.set(handle, c);
      allConnections.push(c);
      return;
    }
    if (code === 0x07 && params.length >= 7 && params[0] === 0) {
      // Remote Name Request Complete
      const name = params.subarray(7).toString('utf8').replace(/\0.*$/s, '');
      if (name) classicNames.set(addrToString(params.subarray(1, 7)), name);
      return;
    }
    if (code === 0x05 && params.length >= 4) {
      const handle = params.readUInt16LE(1) & 0x0fff;
      const c = connections.get(handle);
      if (c) {
        c.end = p.time;
        c.disconnectReason = params[3];
        connections.delete(handle);
      }
      return;
    }
    if (code !== 0x3e) return;
    const sub = params[0];
    if (sub === 0x01 || sub === 0x0a || sub === 0x29) {
      // LE (Enhanced) Connection Complete
      if (params[1] !== 0) return;
      const handle = params.readUInt16LE(2) & 0x0fff;
      const role = params[4];
      const peer = addrToString(params.subarray(6, 12));
      const c = newConnection(handle, peer, p.time);
      c.role = role === 0 ? 'téléphone central' : 'téléphone périphérique';
      connections.set(handle, c);
      allConnections.push(c);
    } else if (sub === 0x02) {
      // LE Advertising Report
      const n = params[1];
      let o = 2;
      for (let k = 0; k < n && o < params.length; k++) {
        const evtType = params[o];
        const addr = addrToString(params.subarray(o + 2, o + 8));
        const len = params[o + 8];
        const data = params.subarray(o + 9, o + 9 + len);
        const rssi = (params[o + 9 + len] << 24) >> 24;
        recordAdvert(addr, evtType === 0x04 ? 'scan_rsp' : 'adv', data, rssi, p.time);
        o += 10 + len;
      }
    } else if (sub === 0x0d) {
      // LE Extended Advertising Report
      const n = params[1];
      let o = 2;
      for (let k = 0; k < n && o < params.length; k++) {
        const evtType = params.readUInt16LE(o);
        const addr = addrToString(params.subarray(o + 3, o + 9));
        const rssi = (params[o + 13] << 24) >> 24;
        const len = params[o + 23];
        const data = params.subarray(o + 24, o + 24 + len);
        recordAdvert(addr, evtType & 0x08 ? 'scan_rsp' : 'adv', data, rssi, p.time);
        o += 24 + len;
      }
    }
  }

  function recordAdvert(addr, kind, data, rssi, time) {
    let d = devices.get(addr);
    if (!d) {
      d = { address: addr, name: null, count: 0, rssi, first: time, adv: null, scanRsp: null, services: new Set(), manufacturer: [] };
      devices.set(addr, d);
    }
    d.count++;
    d.rssi = rssi;
    const parsed = parseAdvertisingData(data);
    if (parsed.name) d.name = parsed.name;
    parsed.services.forEach((s) => d.services.add(s));
    if (parsed.manufacturer.length) d.manufacturer = parsed.manufacturer;
    if (kind === 'scan_rsp') d.scanRsp = parsed;
    else d.adv = parsed;
  }

  function handleAcl(p) {
    const hdr = p.payload.readUInt16LE(0);
    const handle = hdr & 0x0fff;
    const pb = (hdr >> 12) & 0x3;
    const data = p.payload.subarray(4, 4 + p.payload.readUInt16LE(2));
    const key = `${handle}|${p.received ? 'r' : 's'}`;
    let frag;
    if (pb === 0x01) {
      frag = reassembly.get(key);
      if (!frag) return;
      frag.buf = Buffer.concat([frag.buf, data]);
    } else {
      if (data.length < 4) return;
      frag = { buf: Buffer.from(data), expected: data.readUInt16LE(0) + 4 };
      reassembly.set(key, frag);
    }
    if (frag.buf.length < frag.expected) return;
    reassembly.delete(key);
    const cid = frag.buf.readUInt16LE(2);
    const pdu = frag.buf.subarray(4, frag.expected);
    const conn = getConn(handle, p.time);
    if (cid === 0x0004) handleAtt(conn, pdu, p.received, p.time);
    else if (cid === 0x0006 || cid === 0x0007) conn.smp.push({ time: p.time, dir: p.received ? 'in' : 'out', code: pdu[0] });
    else if (cid === 0x0001) handleSignaling(conn, pdu, p.received);
    else {
      const psm = conn.channels.get(`${p.received ? 'in' : 'out'}|${cid}`);
      if (psm === 0x0003) handleRfcomm(conn, pdu, p.received, p.time);
    }
  }

  /** Signalisation L2CAP (classique) : on retient quel canal correspond à quel protocole (PSM). */
  function handleSignaling(conn, pdu, received) {
    let o = 0;
    while (o + 4 <= pdu.length) {
      const code = pdu[o];
      const len = pdu.readUInt16LE(o + 2);
      const d = pdu.subarray(o + 4, o + 4 + len);
      const side = received ? 'in' : 'out';
      if (code === 0x02 && d.length >= 4) {
        conn.pendingL2cap.set(`${side}|${d.readUInt16LE(2)}`, d.readUInt16LE(0));
      } else if (code === 0x03 && d.length >= 8 && d.readUInt16LE(4) === 0) {
        const dcid = d.readUInt16LE(0);
        const scid = d.readUInt16LE(2);
        // La requête venait de l'autre côté que cette réponse
        const reqSide = received ? 'out' : 'in';
        const psm = conn.pendingL2cap.get(`${reqSide}|${scid}`);
        if (psm !== undefined) {
          // Le demandeur écrit vers dcid ; il reçoit sur scid.
          conn.channels.set(`${reqSide}|${dcid}`, psm);
          conn.channels.set(`${reqSide === 'out' ? 'in' : 'out'}|${scid}`, psm);
        }
      }
      o += 4 + len;
    }
  }

  /** RFCOMM (port série Bluetooth, SPP) : on ne garde que les données applicatives. */
  function handleRfcomm(conn, pdu, received, time) {
    if (pdu.length < 4) return;
    const dlci = pdu[0] >> 2;
    const control = pdu[1] & ~0x10;
    const pf = (pdu[1] & 0x10) !== 0;
    let o = 2;
    let len = pdu[o] >> 1;
    if (pdu[o] & 1) o += 1;
    else {
      len |= pdu[o + 1] << 7;
      o += 2;
    }
    if (control !== 0xef || dlci === 0) {
      if (control === 0x2f) conn.ops.push({ time, rel: time - t0, dir: received ? 'in' : 'out', op: 'RFCOMM_OPEN', handle: dlci });
      return;
    }
    if (pf) o += 1; // crédit de contrôle de flux
    const value = pdu.subarray(o, o + len);
    if (!value.length) return;
    conn.ops.push({ time, rel: time - t0, dir: received ? 'in' : 'out', op: 'SPP_DATA', handle: dlci, value });
  }

  function handleAtt(conn, pdu, received, time) {
    const op = pdu[0];
    const name = ATT_OPS[op] || `OP_0x${op.toString(16)}`;
    const dir = received ? 'in' : 'out'; // in = depuis l'appareil distant
    const gatt = conn.gatt;
    const entry = { time, rel: time - t0, dir, op: name };
    // Les réponses de découverte sont interprétées selon la dernière requête du même côté.
    const lastReq = conn.lastReq[received ? 'out' : 'in'];
    switch (op) {
      case 0x01:
        entry.reqOp = ATT_OPS[pdu[1]] || pdu[1];
        entry.handle = pdu.readUInt16LE(2);
        entry.error = ATT_ERRORS[pdu[4]] || `0x${pdu[4].toString(16)}`;
        break;
      case 0x02:
      case 0x03:
        entry.mtu = pdu.readUInt16LE(1);
        if (op === 0x03) conn.mtu = entry.mtu;
        break;
      case 0x04:
      case 0x08:
      case 0x10:
        entry.range = [pdu.readUInt16LE(1), pdu.readUInt16LE(3)];
        if (op !== 0x04) entry.uuid = uuidToString(pdu.subarray(5));
        conn.lastReq[dir] = entry;
        break;
      case 0x05: {
        const fmt = pdu[1];
        const step = fmt === 1 ? 4 : 18;
        for (let o = 2; o + step <= pdu.length; o += step) {
          const h = pdu.readUInt16LE(o);
          const uuid = uuidToString(pdu.subarray(o + 2, o + step));
          gatt.attrs.set(h, { ...(gatt.attrs.get(h) || {}), handle: h, uuid });
        }
        break;
      }
      case 0x09: {
        const len = pdu[1];
        for (let o = 2; o + len <= pdu.length; o += len) {
          const h = pdu.readUInt16LE(o);
          const v = pdu.subarray(o + 2, o + len);
          if (lastReq?.uuid === '0x2803') {
            const props = v[0];
            const valueHandle = v.readUInt16LE(1);
            const uuid = uuidToString(v.subarray(3));
            gatt.chars.set(valueHandle, { declHandle: h, valueHandle, uuid, props: decodeProps(props) });
          } else if (lastReq?.uuid) {
            gatt.attrs.set(h, { ...(gatt.attrs.get(h) || {}), handle: h, uuid: lastReq.uuid, value: hex(v) });
          }
        }
        break;
      }
      case 0x11: {
        const len = pdu[1];
        for (let o = 2; o + len <= pdu.length; o += len) {
          const start = pdu.readUInt16LE(o);
          const end = pdu.readUInt16LE(o + 2);
          const uuid = uuidToString(pdu.subarray(o + 4, o + len));
          gatt.services.set(start, { start, end, uuid, primary: lastReq?.uuid !== '0x2801' });
        }
        break;
      }
      case 0x0a:
      case 0x0c:
        entry.handle = pdu.readUInt16LE(1);
        if (op === 0x0c) entry.offset = pdu.readUInt16LE(3);
        conn.lastReq[dir] = entry;
        break;
      case 0x0b:
      case 0x0d:
        entry.handle = lastReq?.handle;
        entry.value = pdu.subarray(1);
        break;
      case 0x12:
      case 0x52:
      case 0x1b:
      case 0x1d:
        entry.handle = pdu.readUInt16LE(1);
        entry.value = pdu.subarray(3);
        break;
      case 0x16:
        entry.handle = pdu.readUInt16LE(1);
        entry.offset = pdu.readUInt16LE(3);
        entry.value = pdu.subarray(5);
        break;
      default:
        entry.raw = hex(pdu.subarray(1));
    }
    conn.ops.push(entry);
  }

  // Résolution des noms de caractéristiques pour chaque opération
  for (const c of allConnections) {
    if (!c.peer && pendingCreate.length) c.peer = pendingCreate[0];
    for (const op of c.ops) {
      if (op.handle === undefined) continue;
      const ch = describeHandle(c.gatt, op.handle);
      if (ch) op.target = ch;
    }
    const name = c.peer && (devices.get(c.peer)?.name || classicNames.get(c.peer));
    if (name) c.peerName = name;
  }

  return {
    start: t0,
    packetCount: packets.length,
    devices: [...devices.values()].map((d) => ({ ...d, services: [...d.services] })),
    connections: allConnections.map(serializeConnection),
  };
}

function newConnection(handle, peer, time) {
  return {
    handle,
    peer,
    kind: 'le',
    channels: new Map(), // `${sens}|${cid}` -> PSM (L2CAP classique)
    pendingL2cap: new Map(),
    start: time,
    end: null,
    role: null,
    mtu: 23,
    gatt: { services: new Map(), chars: new Map(), attrs: new Map() },
    ops: [],
    smp: [],
    lastReq: { in: null, out: null },
  };
}

/** Nom lisible d'un handle : caractéristique, descripteur (CCCD...) ou attribut. */
export function describeHandle(gatt, handle) {
  const ch = gatt.chars.get(handle);
  if (ch) return { kind: 'value', uuid: ch.uuid, name: uuidName(ch.uuid), props: ch.props };
  const attr = gatt.attrs.get(handle);
  // Descripteur : on retrouve la caractéristique qui précède
  let owner = null;
  for (const c of gatt.chars.values()) if (c.valueHandle < handle && (!owner || c.valueHandle > owner.valueHandle)) owner = c;
  if (attr) return { kind: 'attr', uuid: attr.uuid, name: uuidName(attr.uuid), of: owner?.uuid };
  if (owner) return { kind: 'descriptor?', of: owner.uuid };
  return null;
}

function serializeConnection(c) {
  const services = [...c.gatt.services.values()].sort((a, b) => a.start - b.start);
  const chars = [...c.gatt.chars.values()].sort((a, b) => a.valueHandle - b.valueHandle);
  return {
    handle: c.handle,
    peer: c.peer,
    peerName: c.peerName || null,
    kind: c.kind,
    role: c.role,
    start: c.start,
    end: c.end,
    mtu: c.mtu,
    pairing: c.smp.length > 0,
    gatt: services.map((s) => ({
      ...s,
      name: uuidName(s.uuid),
      characteristics: chars
        .filter((ch) => ch.valueHandle > s.start && ch.valueHandle <= s.end)
        .map((ch) => ({
          ...ch,
          name: uuidName(ch.uuid),
          descriptors: [...c.gatt.attrs.values()]
            .filter((a) => a.handle > ch.valueHandle && a.handle <= s.end && !c.gatt.chars.has(a.handle) && a.handle < (chars.find((n) => n.valueHandle > ch.valueHandle)?.declHandle ?? Infinity))
            .map((a) => ({ handle: a.handle, uuid: a.uuid, name: uuidName(a.uuid) })),
        })),
    })),
    ops: c.ops.map((o) => ({ ...o, value: o.value ? hex(o.value) : undefined, ascii: o.value ? ascii(o.value) : undefined })),
  };
}

/**
 * Regroupe les écritures du téléphone en "rafales" séparées par un silence (une rafale ≈ une action dans l'appli).
 */
export function bursts(ops, gapMs = 1500) {
  const writes = ops.filter((o) => o.dir === 'out' && ['WRITE_REQ', 'WRITE_CMD', 'PREPARE_WRITE_REQ', 'SPP_DATA'].includes(o.op));
  const groups = [];
  let cur = null;
  for (const w of writes) {
    if (!cur || w.time - cur.end > gapMs) {
      cur = { start: w.time, end: w.time, writes: [] };
      groups.push(cur);
    }
    cur.writes.push(w);
    cur.end = w.time;
  }
  return groups;
}
