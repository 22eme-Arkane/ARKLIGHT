// Récepteur Art-Net 4 : ArtDmx + réponse aux ArtPoll pour que les applis (Luminair, Blackout,
// consoles...) découvrent le simulateur comme un vrai node. RDM sur Art-Net (ArtTodRequest,
// ArtTodControl, ArtRdm) : les projecteurs virtuels se laissent découvrir et ré-adresser.
import dgram from 'node:dgram';
import { EventEmitter } from 'node:events';
import { localInterfaceFor } from './net-utils.js';

export const ARTNET_PORT = 6454;
const HEADER = Buffer.from('Art-Net\0', 'latin1');
const OP_POLL = 0x2000;
const OP_POLL_REPLY = 0x2100;
const OP_DMX = 0x5000;
const OP_SYNC = 0x5200;
const OP_TOD_REQUEST = 0x8000;
const OP_TOD_DATA = 0x8100;
const OP_TOD_CONTROL = 0x8200;
const OP_RDM = 0x8300;
const PROT_VER = 14;
const MAX_UIDS = 200; // par paquet ArtTodData

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
    this.rdmRequests = 0;
    this.error = null;
    /** RDM : { tod(portAddress) → UID[], handle(portAddress, message) → réponse | null } (branché par le serveur). */
    this.rdm = null;
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
    } else if (op === OP_TOD_REQUEST || op === OP_TOD_CONTROL) {
      // table des appareils RDM d'un ou plusieurs univers
      if (!this.rdm || msg.length < 24) return;
      const net = msg[21] & 0x7f;
      const subs = op === OP_TOD_CONTROL ? [msg[23]] : [...msg.subarray(24, 24 + Math.min(32, msg[23]))];
      let total = 0;
      for (const sub of subs) total += this._sendTod((net << 8) | sub, rinfo);
      this.emit('rdm-discovery', { source: rinfo.address, universes: subs.length, devices: total });
    } else if (op === OP_RDM) {
      if (!this.rdm || msg.length < 24 + 25) return;
      const portAddress = ((msg[21] & 0x7f) << 8) | msg[23];
      this.rdmRequests++;
      const reply = this.rdm.handle(portAddress, msg.subarray(24));
      if (reply) this._send(this._buildRdm(portAddress, reply), rinfo);
    } else if (op === OP_POLL_REPLY) {
      // Autres nodes présents sur le réseau : utile pour le journal.
      const ip = `${msg[10]}.${msg[11]}.${msg[12]}.${msg[13]}`;
      const name = msg.toString('latin1', 26, 44).replace(/\0.*$/s, '');
      this.emit('node', { ip, name });
    }
  }

  _send(pkt, rinfo) {
    if (!this.socket) return;
    this.socket.send(pkt, ARTNET_PORT, rinfo.address);
    if (rinfo.port !== ARTNET_PORT) this.socket.send(pkt, rinfo.port, rinfo.address);
  }

  /** Envoie la table des appareils d'un univers (en paquets de 200 UID) ; renvoie leur nombre. */
  _sendTod(portAddress, rinfo) {
    const uids = this.rdm.tod(portAddress) || [];
    const blocks = Math.max(1, Math.ceil(uids.length / MAX_UIDS));
    for (let i = 0; i < blocks; i++) this._send(this._buildTodData(portAddress, uids.slice(i * MAX_UIDS, (i + 1) * MAX_UIDS), uids.length, i), rinfo);
    return uids.length;
  }

  _buildTodData(portAddress, uids, total, block) {
    const b = Buffer.alloc(28 + uids.length * 6);
    HEADER.copy(b, 0);
    b.writeUInt16LE(OP_TOD_DATA, 8);
    b[11] = PROT_VER;
    b[12] = 0x01; // RdmVer : RDM standard
    b[13] = 1; // port physique
    b[20] = 1; // BindIndex
    b[21] = (portAddress >> 8) & 0x7f; // Net
    b[22] = 0x00; // TodFull
    b[23] = portAddress & 0xff; // Sub-Net + Univers
    b.writeUInt16BE(total, 24);
    b[26] = block;
    b[27] = uids.length;
    uids.forEach((u, i) => b.set(u, 28 + i * 6));
    return b;
  }

  _buildRdm(portAddress, data) {
    const b = Buffer.alloc(24 + data.length);
    HEADER.copy(b, 0);
    b.writeUInt16LE(OP_RDM, 8);
    b[11] = PROT_VER;
    b[12] = 0x01;
    b[21] = (portAddress >> 8) & 0x7f;
    b[22] = 0x00; // ArProcess
    b[23] = portAddress & 0xff;
    b.set(data, 24);
    return b;
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
    b[23] = this.rdm ? 0xd2 : 0xd0; // Status1 : indicateurs normaux, programmation réseau, RDM
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
