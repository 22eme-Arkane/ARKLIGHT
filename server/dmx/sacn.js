// Récepteur sACN (ANSI E1.31) : multicast 239.255.x.y et unicast sur le port 5568.
import dgram from 'node:dgram';
import { EventEmitter } from 'node:events';
import { ipv4Interfaces } from './net-utils.js';

export const SACN_PORT = 5568;
const ACN_PID = Buffer.from('ASC-E1.17\0\0\0', 'latin1');
const VECTOR_ROOT_E131_DATA = 0x00000004;
const VECTOR_E131_DATA_PACKET = 0x00000002;

export function sacnMulticastGroup(universe) {
  return `239.255.${(universe >> 8) & 0xff}.${universe & 0xff}`;
}

export class SacnInput extends EventEmitter {
  constructor() {
    super();
    this.socket = null;
    this.joined = new Set();
    this.wanted = new Set();
    this.packets = 0;
    this.error = null;
    this.ready = false;
  }

  start() {
    const sock = dgram.createSocket({ type: 'udp4', reuseAddr: true });
    this.socket = sock;
    sock.on('error', (err) => {
      this.error = err.message;
      this.emit('error', err);
    });
    sock.on('message', (msg, rinfo) => this._onMessage(msg, rinfo));
    sock.bind(SACN_PORT, () => {
      this.ready = true;
      this._syncMemberships();
      this.emit('listening', SACN_PORT);
    });
  }

  stop() {
    this.socket?.close();
    this.socket = null;
    this.ready = false;
    this.joined.clear();
  }

  /** Univers (1..63999) pour lesquels on rejoint le groupe multicast. */
  setUniverses(list) {
    this.wanted = new Set(list.filter((u) => u >= 1 && u <= 63999));
    this._syncMemberships();
  }

  _syncMemberships() {
    if (!this.ready || !this.socket) return;
    const ifaces = ipv4Interfaces();
    for (const u of this.wanted) {
      if (this.joined.has(u)) continue;
      for (const i of ifaces) {
        try {
          this.socket.addMembership(sacnMulticastGroup(u), i.address);
        } catch {
          /* interface sans multicast */
        }
      }
      this.joined.add(u);
    }
    for (const u of [...this.joined]) {
      if (this.wanted.has(u)) continue;
      for (const i of ifaces) {
        try {
          this.socket.dropMembership(sacnMulticastGroup(u), i.address);
        } catch {
          /* déjà quitté */
        }
      }
      this.joined.delete(u);
    }
  }

  _onMessage(msg, rinfo) {
    if (msg.length < 126) return;
    if (!msg.subarray(4, 16).equals(ACN_PID)) return;
    if (msg.readUInt32BE(18) !== VECTOR_ROOT_E131_DATA) return;
    if (msg.readUInt32BE(40) !== VECTOR_E131_DATA_PACKET) return;
    if (msg[117] !== 0x02) return;
    const options = msg[112];
    if (options & 0x80) return; // données de prévisualisation
    const universe = msg.readUInt16BE(113);
    const startCode = msg[125];
    const cid = msg.subarray(22, 38).toString('hex');
    if (options & 0x40) {
      this.emit('terminated', { universe, cid });
      return;
    }
    if (startCode !== 0x00) return; // seulement le start code DMX standard
    const count = Math.min(msg.readUInt16BE(123) - 1, 512, msg.length - 126);
    this.packets++;
    this.emit('dmx', {
      protocol: 'sacn',
      universe,
      priority: msg[108],
      sequence: msg[111],
      data: msg.subarray(126, 126 + Math.max(0, count)),
      source: rinfo.address,
      sourceName: msg.toString('utf8', 44, 108).replace(/\0.*$/s, ''),
      cid,
    });
  }
}
