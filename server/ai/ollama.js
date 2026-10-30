// Ollama (IA locale et gratuite) : état, démarrage, installation et téléchargement des modèles,
// pour les utilisateurs qui ne l'ont pas encore. Tout est déclenché par l'utilisateur depuis
// l'assistant (bouton) : rien n'est téléchargé tout seul.
import { spawn } from 'node:child_process';
import { createWriteStream, existsSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

export const OLLAMA_SETUP_URL = 'https://ollama.com/download/OllamaSetup.exe';

/** Modèles proposés au téléchargement (taille approximative, Go). */
export const SUGGESTED_MODELS = [
  { id: 'qwen3.5:9b', size: 6.6, note: 'Conseillé : comprend les outils, rapide avec une carte graphique de 8 Go' },
  { id: 'qwen3:8b', size: 5.2, note: 'Comprend les outils' },
  { id: 'qwen3:4b', size: 2.6, note: 'Petit modèle pour un ordinateur modeste' },
  { id: 'qwen2.5vl:7b', size: 6.0, note: 'Voit les images (photo d’un plan de feu dessiné)' },
];

/** Programme Ollama installé (Windows : dossier de l'utilisateur), ou ''. */
export function ollamaExe() {
  const local = process.env.LOCALAPPDATA || '';
  for (const p of [join(local, 'Programs', 'Ollama', 'ollama app.exe'), join(local, 'Programs', 'Ollama', 'ollama.exe')]) if (local && existsSync(p)) return p;
  return '';
}

async function getJson(url, timeout = 4000) {
  const res = await fetch(url, { signal: AbortSignal.timeout(timeout) });
  if (!res.ok) throw new Error(`${res.status}`);
  return res.json();
}

/** État : installé ? en marche ? version, modèles (avec leurs capacités : outils, vision…). */
export async function ollamaStatus(url) {
  const st = { installed: !!ollamaExe(), running: false, version: '', models: [] };
  try {
    st.version = (await getJson(`${url}/api/version`)).version || '';
    st.running = true;
    st.installed = true;
    const tags = await getJson(`${url}/api/tags`, 8000);
    st.models = (tags.models || []).map((m) => ({ id: m.model || m.name, size: m.size || 0, caps: m.capabilities || [], params: m.details?.parameter_size || '' }));
  } catch {
    /* pas lancé */
  }
  return st;
}

/**
 * Meilleur modèle installé pour l'assistant (qui comprend les outils) : les familles récentes d'abord
 * (les petits modèles anciens se trompent souvent dans les placements), le plus léger de la famille.
 */
export function pickModel(models = []) {
  const withTools = models.filter((m) => !m.caps.length || m.caps.includes('tools'));
  // les modèles qui tiennent dans la mémoire d'une carte graphique de portable (8 Go) d'abord : qwen3.6
  // (36B) est meilleur mais met plusieurs minutes à se charger sur ce genre de machine
  const pref = [/qwen3\.5/, /qwen3(?!\.)/, /qwen3\.6/, /gemma4/, /llama3\.[1-3]/, /mistral-small/, /qwen2\.5(?!-coder|vl)/, /mistral/];
  for (const re of pref) {
    const m = withTools.filter((x) => re.test(x.id)).sort((a, b) => a.size - b.size)[0];
    if (m) return m.id;
  }
  return withTools[0]?.id || '';
}

/** Lance Ollama (installé mais arrêté). */
export function startOllama() {
  const exe = ollamaExe();
  if (!exe) throw new Error('Ollama n’est pas installé');
  spawn(exe, [], { detached: true, stdio: 'ignore', windowsHide: true }).unref();
}

/**
 * Télécharge un modèle (POST /api/pull, progression en continu).
 * @param {(p: { pct: number, text: string }) => void} onProgress
 */
export async function pullModel(url, model, onProgress, signal) {
  const res = await fetch(`${url}/api/pull`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ model, stream: true }), signal });
  if (!res.ok || !res.body) throw new Error(`Ollama : ${res.status} ${await res.text().catch(() => '')}`.trim());
  const decoder = new TextDecoder();
  let buf = '';
  let last = 0;
  for await (const chunk of res.body) {
    buf += decoder.decode(chunk, { stream: true });
    let i;
    while ((i = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, i).trim();
      buf = buf.slice(i + 1);
      if (!line) continue;
      let j;
      try {
        j = JSON.parse(line);
      } catch {
        continue;
      }
      if (j.error) throw new Error(j.error);
      const pct = j.total ? Math.round((100 * (j.completed || 0)) / j.total) : null;
      const now = Date.now();
      if (now - last > 400 || j.status === 'success') {
        last = now;
        onProgress({ pct: j.status === 'success' ? 100 : pct, text: j.total ? `${model} — ${gb(j.completed || 0)} / ${gb(j.total)}` : `${model} — ${j.status || ''}` });
      }
    }
  }
}

const gb = (n) => `${(n / 1e9).toFixed(n < 1e9 ? 2 : 1).replace('.', ',')} Go`;

/**
 * Télécharge l'installeur officiel d'Ollama puis le lance (l'utilisateur suit son assistant).
 * @returns {Promise<string>} chemin de l'installeur
 */
export async function installOllama(dir, onProgress, signal) {
  mkdirSync(dir, { recursive: true });
  const dest = join(dir, 'OllamaSetup.exe');
  const res = await fetch(OLLAMA_SETUP_URL, { signal, redirect: 'follow' });
  if (!res.ok || !res.body) throw new Error(`Téléchargement impossible (${res.status})`);
  const total = Number(res.headers.get('content-length')) || 0;
  const out = createWriteStream(dest);
  let done = 0;
  let last = 0;
  try {
    for await (const chunk of res.body) {
      if (!out.write(chunk)) await new Promise((r) => out.once('drain', r));
      done += chunk.length;
      if (Date.now() - last > 400) {
        last = Date.now();
        onProgress({ pct: total ? Math.round((100 * done) / total) : null, text: `OllamaSetup.exe — ${gb(done)}${total ? ` / ${gb(total)}` : ''}` });
      }
    }
  } finally {
    await new Promise((r) => out.end(r));
  }
  onProgress({ pct: 100, text: 'Installeur d’Ollama lancé : suis ses étapes, puis reviens ici.' });
  spawn(dest, [], { detached: true, stdio: 'ignore' }).unref();
  return dest;
}
