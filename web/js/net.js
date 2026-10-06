// Connexion WebSocket au serveur ARKLIGHT (reconnexion automatique).
export class Net extends EventTarget {
  constructor(url = `ws://${location.host}/ws`) {
    super();
    this.url = url;
    this.ws = null;
    this.connected = false;
    this._retry = 500;
    this._queue = [];
  }

  connect() {
    const ws = new WebSocket(this.url);
    ws.binaryType = 'arraybuffer';
    this.ws = ws;
    ws.onopen = () => {
      this.connected = true;
      this._retry = 500;
      for (const m of this._queue.splice(0)) ws.send(m);
      this.dispatchEvent(new CustomEvent('open'));
    };
    ws.onclose = () => {
      const was = this.connected;
      this.connected = false;
      if (was) this.dispatchEvent(new CustomEvent('close'));
      setTimeout(() => this.connect(), this._retry);
      this._retry = Math.min(this._retry * 2, 5000);
    };
    ws.onmessage = (ev) => {
      if (ev.data instanceof ArrayBuffer) {
        const view = new DataView(ev.data);
        if (view.getUint8(0) === 0x01) {
          const universe = view.getUint16(1);
          const data = new Uint8Array(ev.data, 3, 512);
          this.dispatchEvent(new CustomEvent('dmx', { detail: { universe, data } }));
        }
        return;
      }
      let msg;
      try {
        msg = JSON.parse(ev.data);
      } catch {
        return;
      }
      this.dispatchEvent(new CustomEvent('message', { detail: msg }));
    };
  }

  send(obj) {
    const s = JSON.stringify(obj);
    if (this.connected) this.ws.send(s);
    else this._queue.push(s);
  }
}
