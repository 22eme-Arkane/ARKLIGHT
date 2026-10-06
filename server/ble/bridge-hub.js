// Gestion des ponts Bluetooth LE (processus Python WinRT aujourd'hui, ESP32 plus tard)
// et de l'émulateur actif (profil GATT + logique de protocole).
import { spawn } from 'node:child_process';
import { EventEmitter } from 'node:events';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

const MAX_TRAFFIC = 300;

export class BridgeHub extends EventEmitter {
  /**
   * @param {object} opts
   * @param {string} opts.root        racine du projet
   * @param {(level:string, text:string)=>void} opts.log
   * @param {Record<string, object>} opts.emulators  émulateurs disponibles par id
   */
  constructor({ root, log, emulators }) {
    super();
    this.root = root;
    this.log = log;
    this.emulators = emulators;
    this.emulatorId = Object.keys(emulators)[0];
    this.bridge = null; // { ws, kind, adapter, advertising, published }
    this.proc = null;
    this.subscriptions = new Map(); // char -> clients
    this.traffic = [];
  }

  get emulator() {
    return this.emulators[this.emulatorId];
  }

  // ------------------------------------------------------------------ processus du pont
  startProcess() {
    if (this.proc) return;
    const py = join(this.root, '.venv', 'Scripts', 'python.exe');
    if (!existsSync(py)) {
      this.log('error', 'Bluetooth : environnement Python absent (.venv). Voir README, section Bluetooth.');
      return;
    }
    const proc = spawn(py, ['-u', join(this.root, 'ble', 'bridge.py')], {
      cwd: this.root,
      env: { ...process.env, PYTHONIOENCODING: 'utf-8' },
      windowsHide: true,
    });
    this.proc = proc;
    const relay = (chunk, level) => {
      for (const line of chunk.toString('utf8').split(/\r?\n/)) if (line.trim()) this.log(level, line.replace(/^\[ble\]\s*/, 'Bluetooth : '));
    };
    proc.stdout.on('data', (c) => relay(c, 'info'));
    proc.stderr.on('data', (c) => relay(c, 'error'));
    proc.on('exit', (code) => {
      this.log(code ? 'error' : 'info', `Bluetooth : pont arrêté (code ${code ?? '—'})`);
      this.proc = null;
      this.emit('status');
    });
    this.log('info', 'Bluetooth : démarrage du pont Windows…');
    this.emit('status');
  }

  stopProcess() {
    if (this.bridge) this._send({ t: 'ble.stop' });
    this.proc?.kill();
    this.proc = null;
    this.emit('status');
  }

  setEmulator(id) {
    if (!this.emulators[id]) throw new Error(`Émulateur inconnu : ${id}`);
    this.emulatorId = id;
    this.subscriptions.clear();
    this._publish();
    this.emit('status');
  }

  // ------------------------------------------------------------------ connexion du pont
  attach(ws) {
    if (this.bridge) this.bridge.ws.close(1000, 'remplacé');
    const bridge = { ws, kind: '?', adapter: null, advertising: {}, published: null };
    this.bridge = bridge;
    ws.on('message', (raw) => {
      let msg;
      try {
        msg = JSON.parse(raw.toString());
      } catch {
        return;
      }
      this._onBridgeMessage(bridge, msg);
    });
    ws.on('close', () => {
      if (this.bridge === bridge) this.bridge = null;
      this.subscriptions.clear();
      this.emit('status');
    });
    this.emit('status');
  }

  _send(msg) {
    if (this.bridge?.ws.readyState === 1) this.bridge.ws.send(JSON.stringify(msg));
  }

  _publish() {
    if (!this.bridge?.adapter?.radioOn) return;
    this._send({ t: 'ble.profile', profile: this.emulator.profile });
  }

  _api() {
    return {
      notify: (char, bytes) => {
        this._record('out', 'NOTIFY', char, bytes);
        this._send({ t: 'ble.notify', char, value: Buffer.from(bytes).toString('hex') });
      },
      setValue: (char, bytes) => this._send({ t: 'ble.setValue', char, value: Buffer.from(bytes).toString('hex') }),
      log: (text) => this.log('info', `${this.emulator.name} : ${text}`),
    };
  }

  _record(dir, op, char, bytes) {
    const entry = { at: Date.now(), dir, op, char, hex: Buffer.from(bytes || []).toString('hex') };
    this.traffic.push(entry);
    if (this.traffic.length > MAX_TRAFFIC) this.traffic.shift();
    this.emit('traffic', entry);
  }

  _onBridgeMessage(bridge, msg) {
    switch (msg.t) {
      case 'ble.hello':
        bridge.kind = msg.kind;
        bridge.adapter = msg.adapter;
        if (msg.adapter?.radioOn) {
          this.log('info', `Bluetooth : pont ${msg.kind} connecté (adaptateur ${msg.adapter.address})`);
          this._publish();
        } else {
          this.log('warn', `Bluetooth ${msg.adapter?.radio || 'indisponible'} dans Windows : active-le (Paramètres > Bluetooth et appareils).`);
        }
        break;
      case 'ble.published':
        bridge.published = msg.name;
        bridge.services = msg.services ?? this.emulator.profile?.services?.length ?? 0;
        if (bridge.services) this.log('info', `Bluetooth : profil « ${msg.name} » publié, le PC est visible par les téléphones`);
        else
          this.log(
            'warn',
            `Bluetooth : le profil « ${msg.name} » est vide, rien n’est annoncé : aucun téléphone ne peut voir de box. ` +
              'Il faut d’abord capturer les échanges avec une vraie AsteraBox (docs/CAPTURE_BLUETOOTH.md).',
          );
        break;
      case 'ble.advertising':
        bridge.advertising[msg.service || '*'] = msg.status;
        break;
      case 'ble.subscribe': {
        this.subscriptions.set(msg.char, msg.clients);
        const n = msg.clients.length;
        this.log('info', `Bluetooth : ${n ? `${n} téléphone(s) abonné(s)` : 'désabonnement'} à ${msg.char}`);
        this.emulator.onSubscribe?.(msg, this._api());
        break;
      }
      case 'ble.write': {
        const bytes = Buffer.from(msg.value, 'hex');
        this._record('in', msg.withResponse ? 'WRITE_REQ' : 'WRITE_CMD', msg.char, bytes);
        this.emulator.onWrite?.({ ...msg, bytes }, this._api());
        break;
      }
      case 'ble.read': {
        this._record('in', 'READ', msg.char, []);
        const value = this.emulator.onRead?.(msg, this._api());
        if (msg.id !== undefined) this._send({ t: 'ble.readReply', id: msg.id, value: Buffer.from(value || []).toString('hex') });
        break;
      }
      case 'ble.error':
        this.log('error', `Bluetooth : ${msg.message}`);
        break;
      default:
        break;
    }
    this.emit('status');
  }

  status() {
    const b = this.bridge;
    const phones = new Set();
    for (const clients of this.subscriptions.values()) for (const c of clients) phones.add(c.device);
    return {
      emulator: this.emulatorId,
      emulators: Object.entries(this.emulators).map(([id, e]) => ({ id, name: e.name, ready: e.ready !== false, note: e.note || '' })),
      process: !!this.proc,
      bridge: b ? { kind: b.kind, adapter: b.adapter, advertising: b.advertising, published: b.published, services: b.services ?? null } : null,
      phones: [...phones],
    };
  }
}
