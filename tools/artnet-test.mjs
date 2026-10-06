#!/usr/bin/env node
// Outil de test Art-Net : joue le rôle d'une application (Luminair, Blackout...) sans téléphone.
//
//   node tools/artnet-test.mjs poll                   -> envoie un ArtPoll et affiche les nodes qui répondent
//   node tools/artnet-test.mjs chase [univ] [canaux]  -> chenillard RGB sur un univers Art-Net (port-address, défaut 0)
//   node tools/artnet-test.mjs set <univ> <canal>=<valeur> ...  -> fixe des valeurs puis s'arrête
//
// Options : --ip <adresse>  (défaut 127.0.0.1 ; mettre l'IP du PC ou 255.255.255.255 pour du broadcast)
import dgram from 'node:dgram';

const args = process.argv.slice(2);
const ipIdx = args.indexOf('--ip');
const target = ipIdx >= 0 ? args.splice(ipIdx, 2)[1] : '127.0.0.1';
const [cmd = 'poll', ...rest] = args;
const PORT = 6454;
const HEADER = Buffer.from('Art-Net\0', 'latin1');

const sock = dgram.createSocket({ type: 'udp4', reuseAddr: true });

function artDmx(portAddress, data, seq) {
  const b = Buffer.alloc(18 + 512);
  HEADER.copy(b, 0);
  b.writeUInt16LE(0x5000, 8);
  b[10] = 0;
  b[11] = 14;
  b[12] = seq;
  b[13] = 0;
  b[14] = portAddress & 0xff;
  b[15] = (portAddress >> 8) & 0x7f;
  b.writeUInt16BE(512, 16);
  b.set(data, 18);
  return b;
}

function artPoll() {
  const b = Buffer.alloc(14);
  HEADER.copy(b, 0);
  b.writeUInt16LE(0x2000, 8);
  b[10] = 0;
  b[11] = 14;
  b[12] = 0x02; // envoyer ArtPollReply sur changement
  b[13] = 0;
  return b;
}

function hsv(h) {
  const k = (n) => (n + h / 60) % 6;
  const f = (n) => 1 - Math.max(0, Math.min(k(n), 4 - k(n), 1));
  return [f(5), f(3), f(1)].map((v) => Math.round(v * 255));
}

sock.bind(() => {
  sock.setBroadcast(true);
  if (cmd === 'poll') {
    sock.on('message', (msg, rinfo) => {
      if (msg.length < 200 || msg.readUInt16LE(8) !== 0x2100) return;
      const shortName = msg.toString('latin1', 26, 44).replace(/\0.*$/s, '');
      const longName = msg.toString('latin1', 44, 108).replace(/\0.*$/s, '');
      const ports = msg[173];
      const net = msg[18];
      const sub = msg[19];
      const outs = [...msg.subarray(190, 190 + ports)].map((u) => `${net}:${sub}:${u}`);
      console.log(`Node « ${shortName} » (${longName}) @ ${rinfo.address} — sorties ${outs.join(', ')} — bind ${msg[211]}`);
    });
    sock.send(artPoll(), PORT, target);
    console.log(`ArtPoll envoyé à ${target}:${PORT}, attente des réponses (2 s)…`);
    setTimeout(() => sock.close(), 2000);
  } else if (cmd === 'chase') {
    const pa = Number(rest[0] || 0);
    const channels = Number(rest[1] || 48);
    let seq = 1;
    let t = 0;
    console.log(`Chenillard RGB vers ${target}, port-address ${pa}, ${channels} canaux (Ctrl+C pour arrêter)`);
    setInterval(() => {
      const data = new Uint8Array(512);
      const pixels = Math.floor(channels / 3);
      for (let p = 0; p < pixels; p++) data.set(hsv((t * 4 + (p * 360) / pixels) % 360), p * 3);
      sock.send(artDmx(pa, data, seq), PORT, target);
      seq = (seq % 255) + 1;
      t++;
    }, 25);
  } else if (cmd === 'set') {
    const pa = Number(rest[0] || 0);
    const data = new Uint8Array(512);
    for (const kv of rest.slice(1)) {
      const [ch, v] = kv.split('=').map(Number);
      if (ch >= 1 && ch <= 512) data[ch - 1] = v;
    }
    sock.send(artDmx(pa, data, 1), PORT, target, () => sock.close());
    console.log(`ArtDmx envoyé (port-address ${pa}).`);
  } else {
    console.log('Commandes : poll | chase [univ] [canaux] | set <univ> <canal>=<valeur>…');
    sock.close();
  }
});
