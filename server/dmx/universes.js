// Tampons des univers DMX + fusion des sources (priorité sACN, puis HTP entre sources de même priorité).
// Quand toutes les sources disparaissent, le dernier état est conservé (comportement "hold last look").

const SOURCE_TIMEOUT_MS = 2500; // délai de perte de source de la norme E1.31

export class UniverseStore {
  constructor() {
    /** @type {Map<number, {data: Uint8Array, sources: Map<string, any>, dirty: boolean}>} */
    this.universes = new Map();
  }

  _get(u) {
    let uni = this.universes.get(u);
    if (!uni) {
      uni = { data: new Uint8Array(512), sources: new Map(), dirty: true };
      this.universes.set(u, uni);
    }
    return uni;
  }

  /**
   * @param {object} p
   * @param {number} p.universe   univers interne (1..)
   * @param {Uint8Array} p.data
   * @param {string} p.key        identifiant unique de la source
   * @param {number} [p.priority] priorité sACN (Art-Net = 100)
   */
  input({ universe, data, key, priority = 100, protocol, address, name }) {
    const uni = this._get(universe);
    let src = uni.sources.get(key);
    if (!src) {
      src = { data: new Uint8Array(512), packets: 0, protocol, address, name, priority, last: 0 };
      uni.sources.set(key, src);
    }
    const n = Math.min(data.length, 512);
    src.data.set(data.subarray(0, n));
    if (n < 512) src.data.fill(0, n);
    src.priority = priority;
    src.last = Date.now();
    src.packets++;
    if (name) src.name = name;
    this._merge(uni);
  }

  terminate(universe, key) {
    const uni = this.universes.get(universe);
    if (uni && uni.sources.delete(key)) this._merge(uni);
  }

  _merge(uni) {
    const now = Date.now();
    let best = -1;
    const live = [];
    for (const s of uni.sources.values()) {
      if (now - s.last > SOURCE_TIMEOUT_MS) continue;
      live.push(s);
      if (s.priority > best) best = s.priority;
    }
    if (live.length === 0) return; // on garde le dernier état
    const top = live.filter((s) => s.priority === best);
    const out = uni.data;
    let changed = false;
    if (top.length === 1) {
      const d = top[0].data;
      for (let i = 0; i < 512; i++) {
        if (out[i] !== d[i]) {
          out[i] = d[i];
          changed = true;
        }
      }
    } else {
      for (let i = 0; i < 512; i++) {
        let v = 0;
        for (const s of top) if (s.data[i] > v) v = s.data[i];
        if (out[i] !== v) {
          out[i] = v;
          changed = true;
        }
      }
    }
    if (changed) uni.dirty = true;
  }

  /** Supprime les sources muettes. */
  sweep() {
    const now = Date.now();
    for (const uni of this.universes.values()) {
      for (const [k, s] of uni.sources) if (now - s.last > SOURCE_TIMEOUT_MS * 3) uni.sources.delete(k);
    }
  }

  /** Univers modifiés depuis le dernier appel. */
  takeDirty() {
    const out = [];
    for (const [u, uni] of this.universes) {
      if (uni.dirty) {
        uni.dirty = false;
        out.push([u, uni.data]);
      }
    }
    return out;
  }

  all() {
    return [...this.universes].map(([u, uni]) => [u, uni.data]);
  }

  /** Résumé des sources actives pour la barre d'état. */
  sources() {
    const now = Date.now();
    const bySource = new Map();
    for (const [u, uni] of this.universes) {
      for (const s of uni.sources.values()) {
        const age = now - s.last;
        if (age > SOURCE_TIMEOUT_MS) continue;
        const k = `${s.protocol}|${s.address}`;
        const e = bySource.get(k) || { protocol: s.protocol, address: s.address, name: s.name || '', universes: [], priority: s.priority };
        e.universes.push(u);
        if (s.name) e.name = s.name;
        bySource.set(k, e);
      }
    }
    return [...bySource.values()];
  }
}
