// Récepteur Art-Net 4 : ArtDmx + réponse aux ArtPoll pour que les applis (Luminair, Blackout,
// consoles...) découvrent le simulateur comme un vrai node.
import dgram from 'node:dgram';
import { EventEmitter } from 'node:events';
import { localInterfaceFor } from './net-utils.js';

export const ARTNET_PORT = 6454;
const HEADER = Buffer.from('Art-Net\0', 'latin1');
const OP_POLL = 0x2000;
const OP_POLL_REPLY = 0x2100;
const OP_DMX = 0x5000;
const OP_SYNC = 0x5200;

export class ArtNetInput extends EventEmitter {
  /**
   * @param {object} opts
   * @param {string} opts.shortName
   * @param {string} opts.longName
   * @param {() => number[]} opts.getPortAddresses  port-addresses Art-Net (0..32767) à annoncer
   */
  constructor({ shortName = 'ARKLIGHT', longName = 'ARKLIGHT simulator', getPortAddresses = () => [0], port = ARTNET_PORT } = {}) {
    super();
    this.port = port;
    this.shortName = shortName;
    this.longName = longName;
    this.getPortAddresses = getPortAddresses;
    this.socket = null;
    this.packets = 0;
    this.polls = 0;
    this.error = null;
  }

  start() {
    const sock = dgram.createSocket({ type: 'udp4', reuseAddr: true });
    this.socket = sock;
    sock.on('error', (err) => {
      this.error = err.message;
      this.emit('error', err);
    });
    sock.on('message', (msg, rinfo) => this._onMessage(msg, rinfo));
    sock.bind(this.port, () => {
      sock.setBroadcast(true);
      this.emit('listening', this.port);
    });
  }

  stop() {
    this.socket?.close();
    this.socket = null;
  }

  _onMessage(msg, rinfo) {
    if (msg.length < 12 || !msg.subarray(0, 8).equals(HEADER)) return;
    const op = msg.readUInt16LE(8);
    if (op === OP_DMX) {
      if (msg.length < 18) return;
      const portAddress = ((msg[15] & 0x7f) << 8) | msg[14];
      const length = Math.min((msg[16] << 8) | msg[17], 512, msg.length - 18);
      this.packets++;
      this.emit('dmx', {
        protocol: 'artnet',
        portAddress,
        sequence: msg[12],
        data: msg.subarray(18, 18 + length),
        source: rinfo.address,
      });
    } else if (op === OP_POLL) {
      this.polls++;
      this.emit('poll', { source: rinfo.address });
      this._replyToPoll(rinfo);
    } else if (op === OP_SYNC) {
      // Les données sont affichées dès réception : ArtSync n'a pas d'effet ici.
    } else if (op === OP_POLL_REPLY) {
      // Autres nodes présents sur le réseau : utile pour le journal.
      const ip = `${msg[10]}.${msg[11]}.${msg[12]}.${msg[13]}`;
      const name = msg.toString('latin1', 26, 44).replace(/\0.*$/s, '');
      this.emit('node', { ip, name });
    }
  }

  _replyToPoll(rinfo) {
    if (!this.socket) return;
    const iface = localInterfaceFor(rinfo.address);
    const addrs = [...new Set(this.getPortAddresses())].sort((a, b) => a - b);
    if (addrs.length === 0) addrs.push(0);

    // Un ArtPollReply décrit au plus 4 ports partageant le même Net/Sub-Net.
    const groups = [];
    for (const pa of addrs) {
      const last = groups[groups.length - 1];
      if (last && last.length < 4 && last[0] >> 4 === pa >> 4) last.push(pa);
      else groups.push([pa]);
    }
    groups.forEach((ports, i) => {
      const pkt = this._buildPollReply(iface, ports, i + 1);
      this.socket.send(pkt, ARTNET_PORT, rinfo.address);
      if (rinfo.port !== ARTNET_PORT) this.socket.send(pkt, rinfo.port, rinfo.address);
    });
  }

  _buildPollReply(iface, ports, bindIndex) {
    const b = Buffer.alloc(239);
    HEADER.copy(b, 0);
    b.writeUInt16LE(OP_POLL_REPLY, 8);
    const ip = iface.address.split('.').map(Number);
    b.set(ip, 10);
    b.writeUInt16LE(ARTNET_PORT, 14);
    b[16] = 0; // VersInfoH
    b[17] = 1; // VersInfoL
    b[18] = (ports[0] >> 8) & 0x7f; // NetSwitch
    b[19] = (ports[0] >> 4) & 0x0f; // SubSwitch
    b.writeUInt16BE(0x00ff, 20); // OEM inconnu
    b[22] = 0; // UBEA
    b[23] = 0xd0; // Status1 : indicateurs normaux, programmation réseau
    b.writeUInt16LE(0x7ff0, 24); // code ESTA "prototype"
    b.write(this.shortName.slice(0, 17), 26, 'latin1');
    b.write(this.longName.slice(0, 63), 44, 'latin1');
    b.write(`#0001 [${String(this.packets % 10000).padStart(4, '0')}] ARKLIGHT OK`.slice(0, 63), 108, 'latin1');
    b[172] = 0;
    b[173] = ports.length;
    ports.forEach((pa, i) => {
      b[174 + i] = 0x80; // PortTypes : sortie DMX512 depuis Art-Net
      b[182 + i] = 0x80; // GoodOutputA : données en sortie
      b[190 + i] = pa & 0x0f; // SwOut
    });
    b[200] = 0x00; // Style : StNode
    const mac = (iface.mac || '00:00:00:00:00:00').split(':').map((h) => parseInt(h, 16) || 0);
    b.set(mac.slice(0, 6), 201);
    b.set(ip, 207); // BindIp
    b[211] = bindIndex;
    b[212] = 0x0e; // Status2 : DHCP, Port-Address 15 bits
    return b;
  }
}
