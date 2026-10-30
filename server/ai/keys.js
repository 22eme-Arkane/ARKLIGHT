// Clés API des fournisseurs d'IA (Anthropic, OpenAI…) : gardées sur cet ordinateur seulement, à
// part des réglages, jamais renvoyées aux interfaces. Dans l'application de bureau, elles sont
// chiffrées par Windows (safeStorage d'Electron) ; sinon écrites telles quelles dans le fichier.
import { readFileSync, writeFileSync } from 'node:fs';

let safe = null;
if (process.versions.electron) {
  try {
    safe = (await import('electron')).safeStorage || null;
  } catch {
    safe = null;
  }
}
const canEncrypt = () => {
  try {
    return !!safe?.isEncryptionAvailable();
  } catch {
    return false;
  }
};

export class KeyStore {
  constructor(file) {
    this.file = file;
    this.keys = {};
    try {
      const raw = JSON.parse(readFileSync(file, 'utf8'));
      for (const [k, v] of Object.entries(raw || {})) {
        const key = this._decode(v);
        if (key) this.keys[k] = key;
      }
    } catch {
      /* pas encore de clés */
    }
  }

  _decode(v) {
    if (typeof v !== 'string') return '';
    if (v.startsWith('enc:')) {
      if (!canEncrypt()) return '';
      try {
        return safe.decryptString(Buffer.from(v.slice(4), 'base64'));
      } catch {
        return '';
      }
    }
    return v.startsWith('raw:') ? v.slice(4) : v;
  }

  _encode(v) {
    return canEncrypt() ? `enc:${safe.encryptString(v).toString('base64')}` : `raw:${v}`;
  }

  get(provider) {
    return this.keys[provider] || '';
  }

  /** Fournisseurs qui ont une clé (pour l'interface : « clé enregistrée »). */
  list() {
    return Object.keys(this.keys);
  }

  set(provider, value) {
    const key = String(value || '').trim();
    if (key) this.keys[provider] = key;
    else delete this.keys[provider];
    writeFileSync(this.file, JSON.stringify(Object.fromEntries(Object.entries(this.keys).map(([k, v]) => [k, this._encode(v)])), null, 2));
  }

  get encrypted() {
    return canEncrypt();
  }
}
