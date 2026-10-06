// Test de l'analyseur btsnoop sur une capture synthétique (publicité, connexion, découverte GATT, échanges).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { deflateRawSync } from 'node:zlib';
import { analyze, bursts } from '../tools/btsnoop/btsnoop.mjs';
import { findBtsnoopLogs, isZip } from '../tools/btsnoop/zip.mjs';

const EPOCH = 0x00dcddb30f2f8000n;
const BOX = [0x01, 0xee, 0xdd, 0xcc, 0xbb, 0xaa]; // AA:BB:CC:DD:EE:01 (petit-boutiste)
const SVC = Buffer.from('9ecadc240ee5a9e093f3a3b50100406e', 'hex'); // 6e400001-b5a3-f393-e0a9-e50e24dcca9e
const RX = Buffer.from(SVC);
RX[12] = 0x02;
const TX = Buffer.from(SVC);
TX[12] = 0x03;

function capture(records) {
  const head = Buffer.alloc(16);
  head.write('btsnoop\0', 0, 'latin1');
  head.writeUInt32BE(1, 8);
  head.writeUInt32BE(1002, 12);
  const parts = [head];
  let t = 1_700_000_000_000_000n;
  for (const { data, received } of records) {
    t += 10_000n;
    const rec = Buffer.alloc(24);
    rec.writeUInt32BE(data.length, 0);
    rec.writeUInt32BE(data.length, 4);
    rec.writeUInt32BE((received ? 1 : 0) | (data[0] === 0x01 || data[0] === 0x04 ? 2 : 0), 8);
    rec.writeBigUInt64BE(t + EPOCH, 16);
    parts.push(rec, data);
  }
  return Buffer.concat(parts);
}

const evt = (code, params) => ({ data: Buffer.from([0x04, code, params.length, ...params]), received: true });
const u16 = (v) => [v & 0xff, v >> 8];

/** Paquet ACL, éventuellement découpé en fragments de `frag` octets */
function acl(handle, pdu, received, frag = 0) {
  const l2 = Buffer.from([...u16(pdu.length), ...u16(0x0004), ...pdu]);
  const chunks = frag ? [] : [l2];
  if (frag) for (let i = 0; i < l2.length; i += frag) chunks.push(l2.subarray(i, i + frag));
  return chunks.map((c, i) => ({
    data: Buffer.from([0x02, ...u16(handle | ((i === 0 ? 0b10 : 0b01) << 12)), ...u16(c.length), ...c]),
    received,
  }));
}

test('analyse une session complète app <-> boîte', () => {
  const adv = Buffer.from([0x02, 0x01, 0x06, 0x0a, 0x09, ...Buffer.from('AsteraBox')]);
  const H = 0x0040;
  const records = [
    evt(0x3e, [0x02, 1, 0x00, 0x00, ...BOX, adv.length, ...adv, 0xc4]),
    evt(0x3e, [0x01, 0x00, ...u16(H), 0x00, 0x00, ...BOX, ...u16(24), 0, 0, ...u16(200), 0]),
    ...acl(H, [0x02, ...u16(247)], false),
    ...acl(H, [0x03, ...u16(185)], true),
    ...acl(H, [0x10, ...u16(1), ...u16(0xffff), ...u16(0x2800)], false),
    ...acl(H, [0x11, 20, ...u16(0x10), ...u16(0x20), ...SVC], true),
    ...acl(H, [0x08, ...u16(0x10), ...u16(0x20), ...u16(0x2803)], false),
    ...acl(H, [0x09, 21, ...u16(0x11), 0x0c, ...u16(0x12), ...RX, ...u16(0x13), 0x10, ...u16(0x14), ...TX], true),
    ...acl(H, [0x04, ...u16(0x15), ...u16(0x20)], false),
    ...acl(H, [0x05, 0x01, ...u16(0x15), ...u16(0x2902)], true),
    ...acl(H, [0x12, ...u16(0x15), 0x01, 0x00], false),
    ...acl(H, [0x13], true),
    ...acl(H, [0x52, ...u16(0x12), ...Buffer.from('bonjour la boite, ceci est un long message fragmente')], false, 20),
    ...acl(H, [0x1b, ...u16(0x14), 0xa5, 0x01, 0x02], true),
    evt(0x05, [0x00, ...u16(H), 0x13]),
  ];
  const r = analyze(capture(records));

  assert.equal(r.devices.length, 1);
  assert.equal(r.devices[0].name, 'AsteraBox');
  assert.equal(r.devices[0].address, 'AA:BB:CC:DD:EE:01');

  assert.equal(r.connections.length, 1);
  const c = r.connections[0];
  assert.equal(c.peer, 'AA:BB:CC:DD:EE:01');
  assert.equal(c.peerName, 'AsteraBox');
  assert.equal(c.mtu, 185);
  assert.ok(c.end);

  assert.equal(c.gatt.length, 1);
  assert.equal(c.gatt[0].uuid, '6e400001-b5a3-f393-e0a9-e50e24dcca9e');
  const [rx, tx] = c.gatt[0].characteristics;
  assert.equal(rx.uuid, '6e400002-b5a3-f393-e0a9-e50e24dcca9e');
  assert.deepEqual(rx.props, ['writeNoResp', 'write']);
  assert.deepEqual(tx.props, ['notify']);
  assert.equal(tx.descriptors[0].uuid, '0x2902');

  const write = c.ops.find((o) => o.op === 'WRITE_CMD');
  assert.equal(write.ascii, 'bonjour la boite, ceci est un long message fragmente');
  assert.equal(write.target.uuid, rx.uuid);
  const notif = c.ops.find((o) => o.op === 'NOTIFY');
  assert.equal(notif.value, 'a5 01 02');
  assert.equal(notif.dir, 'in');

  assert.equal(bursts(c.ops).length, 1);
});

/** Paquet ACL brut (L2CAP sur un CID quelconque) */
function aclCid(handle, cid, payload, received) {
  const l2 = Buffer.from([...u16(payload.length), ...u16(cid), ...payload]);
  return { data: Buffer.from([0x02, ...u16(handle | (0b10 << 12)), ...u16(l2.length), ...l2]), received };
}

test('décode une liaison série Bluetooth classique (RFCOMM / SPP)', () => {
  const H = 0x0011;
  const records = [
    evt(0x03, [0x00, ...u16(H), ...BOX, 0x01, 0x00]),
    evt(0x07, [0x00, ...BOX, ...Buffer.from('ART7-1234\0')]),
    // le téléphone ouvre le canal RFCOMM (PSM 3) : scid 0x0040, la boîte répond dcid 0x0041
    aclCid(H, 0x0001, [0x02, 0x01, ...u16(4), ...u16(0x0003), ...u16(0x0040)], false),
    aclCid(H, 0x0001, [0x03, 0x01, ...u16(8), ...u16(0x0041), ...u16(0x0040), ...u16(0), ...u16(0)], true),
    // SABM sur le DLCI 2 puis données UIH dans les deux sens
    aclCid(H, 0x0041, [0x0b, 0x3f, 0x01, 0x00], false),
    aclCid(H, 0x0041, [0x0b, 0xef, (5 << 1) | 1, ...Buffer.from('hello'), 0x00], false),
    aclCid(H, 0x0040, [0x09, 0xff, (3 << 1) | 1, 0x05, 0x01, 0x02, 0x03, 0x00], true),
  ];
  const r = analyze(capture(records));
  const c = r.connections[0];
  assert.equal(c.kind, 'classic');
  assert.equal(c.peerName, 'ART7-1234');
  const data = c.ops.filter((o) => o.op === 'SPP_DATA');
  assert.equal(data.length, 2);
  assert.equal(data[0].ascii, 'hello');
  assert.equal(data[0].dir, 'out');
  assert.equal(data[1].value, '01 02 03');
  assert.equal(data[1].dir, 'in');
});

test('extrait le journal btsnoop d’un rapport de bug Android (.zip)', () => {
  const log = capture([evt(0x05, [0x00, ...u16(1), 0x13])]);
  const name = Buffer.from('FS/data/misc/bluetooth/logs/btsnoop_hci.log');
  const comp = deflateRawSync(log);
  const local = Buffer.alloc(30);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(8, 8);
  local.writeUInt32LE(comp.length, 18);
  local.writeUInt32LE(log.length, 22);
  local.writeUInt16LE(name.length, 26);
  const central = Buffer.alloc(46);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(8, 10);
  central.writeUInt32LE(comp.length, 20);
  central.writeUInt32LE(log.length, 24);
  central.writeUInt16LE(name.length, 28);
  central.writeUInt32LE(0, 42);
  const cdOffset = local.length + name.length + comp.length;
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(1, 8);
  eocd.writeUInt16LE(1, 10);
  eocd.writeUInt32LE(central.length + name.length, 12);
  eocd.writeUInt32LE(cdOffset, 16);
  const zip = Buffer.concat([local, name, comp, central, name, eocd]);
  assert.ok(isZip(zip));
  const logs = findBtsnoopLogs(zip);
  assert.equal(logs.length, 1);
  assert.ok(logs[0].data.equals(log));
});
