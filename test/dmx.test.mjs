// Tests du décodage DMX et de la réception Art-Net.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { compileMode, decodeDmxOutput, readAttributes, strobeGate } from '../shared/engine/dmx.js';
import { ArtNetInput } from '../server/dmx/artnet.js';

const WHITE = [1, 1, 1];

test('bloc de pixels RGB + dimmer 16 bits', () => {
  const mode = { channels: [{ f: 'dimmer', bytes: 2 }, { pixels: 3, channels: ['red', 'green', 'blue'] }] };
  assert.equal(compileMode(mode).footprint, 11);
  const data = new Uint8Array(512);
  data.set([128, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255], 9); // adresse 10
  const out = new Float32Array(9);
  decodeDmxOutput({ mode, data, address: 10, pixelCount: 3, time: 0, whiteRgb: WHITE }, out);
  const k = (128 * 256) / 65535;
  assert.ok(Math.abs(out[0] - k) < 1e-6, 'pixel 1 rouge au dimmer');
  assert.equal(out[1], 0);
  assert.ok(Math.abs(out[4] - k) < 1e-6, 'pixel 2 vert');
  assert.ok(Math.abs(out[8] - k) < 1e-6, 'pixel 3 bleu');
});

test('plage d’une fonction (Kelvin) et teinte centrée', () => {
  const mode = { channels: [{ f: 'cct', range: [2000, 10000] }, 'tint'] };
  const data = new Uint8Array(512);
  data[0] = 255;
  data[1] = 128;
  const { fixture } = readAttributes(mode, data, 1, 1);
  assert.equal(fixture.cct, 10000);
  assert.ok(Math.abs(fixture.tint) < 0.01);
});

test('stroboscope : 0–9 ouvert, ensuite fréquence croissante', () => {
  const mode = { channels: ['strobe'] };
  const data = new Uint8Array(512);
  data[0] = 5;
  assert.equal(readAttributes(mode, data, 1, 1).fixture.strobe, 0);
  data[0] = 255;
  const hz = readAttributes(mode, data, 1, 1).fixture.strobe;
  assert.equal(hz, 25);
  assert.equal(strobeGate(10, 0.001), 1);
  assert.equal(strobeGate(10, 0.05), 0);
});

test('Art-Net : réception ArtDmx et réponse ArtPoll', async () => {
  const input = new ArtNetInput({ shortName: 'ARKLIGHT', getPortAddresses: () => [0, 1, 2, 3, 16], port: 0 });
  await new Promise((resolve) => {
    input.once('listening', resolve);
    input.start();
  });
  const port = input.socket.address().port;
  const client = dgram.createSocket('udp4');
  await new Promise((r) => client.bind(0, '127.0.0.1', r));

  const header = Buffer.from('Art-Net\0', 'latin1');
  // ArtDmx sur le port-address 0x0123 (Net 1, Sub 2, Univers 3)
  const dmx = Buffer.alloc(18 + 4);
  header.copy(dmx);
  dmx.writeUInt16LE(0x5000, 8);
  dmx[11] = 14;
  dmx[14] = 0x23;
  dmx[15] = 0x01;
  dmx.writeUInt16BE(4, 16);
  dmx.set([10, 20, 30, 40], 18);
  const gotDmx = new Promise((r) => input.once('dmx', r));
  client.send(dmx, port, '127.0.0.1');
  const evt = await gotDmx;
  assert.equal(evt.portAddress, 0x0123);
  assert.deepEqual([...evt.data], [10, 20, 30, 40]);

  // ArtPoll -> deux ArtPollReply (ports 0..3 en Sub 0, puis port 16 en Sub 1)
  const poll = Buffer.alloc(14);
  header.copy(poll);
  poll.writeUInt16LE(0x2000, 8);
  poll[11] = 14;
  const replies = [];
  const done = new Promise((resolve) => {
    client.on('message', (m) => {
      replies.push(m);
      if (replies.length === 2) resolve();
    });
  });
  client.send(poll, port, '127.0.0.1');
  await done;
  replies.sort((a, b) => a[211] - b[211]);
  const [r1, r2] = replies;
  assert.equal(r1.length, 239);
  assert.equal(r1.readUInt16LE(8), 0x2100);
  assert.equal(r1.toString('latin1', 26, 34), 'ARKLIGHT');
  assert.equal(r1[173], 4);
  assert.deepEqual([...r1.subarray(190, 194)], [0, 1, 2, 3]);
  assert.equal(r2[19], 1, 'Sub-Net 1 pour le port-address 16');
  assert.equal(r2[173], 1);
  assert.equal(r2[190], 0);

  client.close();
  input.stop();
});
