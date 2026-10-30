// Tests du RDM des projecteurs virtuels (E1.20) et de son transport en Art-Net (ArtTodRequest, ArtRdm).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { SceneStore } from '../server/scene-store.js';
import { ArtNetInput } from '../server/dmx/artnet.js';
import { CC, NACK, PID, PROTOTYPE_MANUFACTURER, RdmResponder, buildRdm, parseRdm } from '../shared/rdm.js';

const CONTROLLER = Uint8Array.of(0x4c, 0x55, 0x00, 0x00, 0x00, 0x01); // UID de l'appli qui interroge

function setup() {
  const dir = mkdtempSync(join(tmpdir(), 'arklight-rdm-'));
  const store = new SceneStore(dir);
  const titan = store.addFixture({ type: 'astera.titan', settings: { universe: 1 } });
  const s60 = store.addFixture({ type: 'arri.skypanel-s60c', settings: { universe: 1 } });
  store.addFixture({ type: 'arri.m18', settings: { universe: 2 } });
  const events = [];
  const rdm = new RdmResponder({
    getFixtures: () => store.scene.fixtures,
    onSet: (f, changes, what) => {
      store.updateFixture(f.id, changes);
      events.push(what);
    },
    onIdentify: (f, on) => events.push(`identify:${on}`),
  });
  const done = () => {
    clearTimeout(store._timer);
    rmSync(dir, { recursive: true, force: true });
  };
  return { store, rdm, titan, s60, events, done };
}

const ask = (rdm, uid, cc, pid, pd = new Uint8Array(0)) => parseRdm(rdm.handle(1, buildRdm({ dest: uid, src: CONTROLLER, tn: 7, cc, pid, pd })));

test('RDM : table des appareils, infos, modes, canaux', () => {
  const { rdm, s60, done } = setup();
  try {
    const uids = rdm.tod(1);
    assert.equal(uids.length, 2, 'deux projecteurs sur l’univers 1');
    assert.equal(rdm.tod(2).length, 1);
    assert.equal((uids[0][0] << 8) | uids[0][1], PROTOTYPE_MANUFACTURER);
    const uid = rdm.devices(1).find((d) => d.f.id === s60.id).uid;
    const info = ask(rdm, uid, CC.GET, PID.DEVICE_INFO);
    assert.equal(info.cc, CC.GET_RESPONSE);
    assert.equal(info.tn, 7);
    assert.deepEqual([...info.src], [...uid]);
    assert.equal(info.pd.length, 19);
    const footprint = (info.pd[10] << 8) | info.pd[11];
    assert.ok(footprint > 0);
    assert.equal((info.pd[14] << 8) | info.pd[15], s60.settings.dmxAddress, 'adresse de départ');
    assert.equal(info.pd[13], 15, 'nombre de modes');
    const label = ask(rdm, uid, CC.GET, PID.MANUFACTURER_LABEL);
    assert.equal(String.fromCharCode(...label.pd), 'ARRI');
    const pers = ask(rdm, uid, CC.GET, PID.DMX_PERSONALITY_DESCRIPTION, Uint8Array.of(1));
    assert.equal(pers.pd[0], 1);
    assert.match(String.fromCharCode(...pers.pd.slice(3)), /CCT/);
    const slot = ask(rdm, uid, CC.GET, PID.SLOT_DESCRIPTION, Uint8Array.of(0, 0));
    assert.equal(String.fromCharCode(...slot.pd.slice(2)), 'Dimmer');
    const unknown = ask(rdm, uid, CC.GET, 0x0400);
    assert.equal(unknown.port, 0x02, 'NACK');
    assert.equal((unknown.pd[0] << 8) | unknown.pd[1], NACK.UNKNOWN_PID);
  } finally {
    done();
  }
});

test('RDM : l’appli ré-adresse, change le mode, identifie ; valeurs hors limites refusées', () => {
  const { store, rdm, titan, events, done } = setup();
  try {
    const uid = rdm.devices(1).find((d) => d.f.id === titan.id).uid;
    const set = ask(rdm, uid, CC.SET, PID.DMX_START_ADDRESS, Uint8Array.of(0, 101));
    assert.equal(set.cc, CC.SET_RESPONSE);
    assert.equal(set.port, 0x00, 'ACK');
    assert.equal(store.getFixture(titan.id).settings.dmxAddress, 101);
    const bad = ask(rdm, uid, CC.SET, PID.DMX_START_ADDRESS, Uint8Array.of(0x02, 0x00)); // 512 : le mode ne tient plus
    assert.equal(bad.port, 0x02);
    assert.equal((bad.pd[0] << 8) | bad.pd[1], NACK.DATA_OUT_OF_RANGE);
    ask(rdm, uid, CC.SET, PID.DMX_PERSONALITY, Uint8Array.of(3));
    assert.equal(store.getFixture(titan.id).settings.dmxMode, '3');
    ask(rdm, uid, CC.SET, PID.IDENTIFY_DEVICE, Uint8Array.of(1));
    assert.equal(ask(rdm, uid, CC.GET, PID.IDENTIFY_DEVICE).pd[0], 1);
    assert.deepEqual(events, ['address', 'mode', 'identify:true']);
    // un message abîmé (somme de contrôle fausse) est ignoré
    const msg = buildRdm({ dest: uid, src: CONTROLLER, tn: 1, cc: CC.GET, pid: PID.DEVICE_INFO });
    msg[msg.length - 1] ^= 0xff;
    assert.equal(rdm.handle(1, msg), null);
  } finally {
    done();
  }
});

test('Art-Net : ArtTodRequest → ArtTodData, ArtRdm → réponse, RDM annoncé dans ArtPollReply', async () => {
  const { rdm, done } = setup();
  const node = new ArtNetInput({ port: 0, getPortAddresses: () => [0] });
  node.rdm = { tod: (pa) => rdm.tod(pa + 1), handle: (pa, data) => rdm.handle(pa + 1, data) };
  const client = dgram.createSocket('udp4');
  try {
    await new Promise((r) => node.once('listening', r) && node.start());
    await new Promise((r) => client.bind(0, '127.0.0.1', r));
    const port = node.socket.address().port;
    const next = () => new Promise((r) => client.once('message', (m) => r(m)));
    const header = (op, len) => {
      const b = Buffer.alloc(len);
      b.write('Art-Net\0', 0, 'latin1');
      b.writeUInt16LE(op, 8);
      b[11] = 14;
      return b;
    };
    // découverte
    const req = header(0x8000, 25);
    req[21] = 0; // Net
    req[23] = 1; // un univers
    req[24] = 0; // Sub-Net 0 / Univers 0 → univers 1 d'ARKLIGHT
    let got = next();
    client.send(req, port, '127.0.0.1');
    const tod = await got;
    assert.equal(tod.readUInt16LE(8), 0x8100);
    assert.equal(tod.readUInt16BE(24), 2, 'deux UID');
    assert.equal(tod[27], 2);
    const uid = tod.subarray(28, 34);
    // question RDM
    const msg = buildRdm({ dest: uid, src: CONTROLLER, tn: 3, cc: CC.GET, pid: PID.DMX_START_ADDRESS });
    const art = header(0x8300, 24 + msg.length);
    art[12] = 1;
    art.set(msg, 24);
    got = next();
    client.send(art, port, '127.0.0.1');
    const reply = await got;
    assert.equal(reply.readUInt16LE(8), 0x8300);
    const res = parseRdm(reply.subarray(24));
    assert.ok(res, 'réponse RDM valide');
    assert.equal(res.pid, PID.DMX_START_ADDRESS);
    // ArtPollReply : bit « RDM »
    const poll = header(0x2000, 14);
    got = next();
    client.send(poll, port, '127.0.0.1');
    const pr = await got;
    assert.equal(pr.readUInt16LE(8), 0x2100);
    assert.equal(pr[23] & 0x02, 0x02);
  } finally {
    client.close();
    node.stop();
    done();
  }
});
