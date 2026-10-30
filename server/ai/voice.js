// Commande vocale de l'assistant, sur le modèle d'ORAKLE : la parole est transcrite sur cet
// ordinateur par Whisper (whisper.cpp), sans internet ni abonnement, avec le vocabulaire du métier
// et les noms du projet en « amorce » pour bien reconnaître « SkyPanel », « Titan », « contre »…
// L'installation (programme officiel whisper.cpp + modèle) se fait sur demande, depuis l'assistant.
import { spawn } from 'node:child_process';
import { createWriteStream, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { cpus } from 'node:os';
import { dirname, join, normalize, sep } from 'node:path';
import { inflateRawSync } from 'node:zlib';
import { readZip } from '../../shared/zip.js';

/** Modèles Whisper proposés (taille approximative en Go). */
export const WHISPER_MODELS = {
  base: { file: 'ggml-base.bin', size: 0.15, note: 'Rapide : suffisant pour des commandes courtes' },
  small: { file: 'ggml-small.bin', size: 0.49, note: 'Plus précis (accents, bruit de plateau)' },
};
const MODEL_URL = (file) => `https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${file}`;
const RELEASES_URL = 'https://api.github.com/repos/ggml-org/whisper.cpp/releases/latest';
const BINARIES = ['whisper-cli.exe', 'main.exe', 'whisper-cli', 'main'];

/** Vocabulaire du métier (amorce de la transcription, comme le dictionnaire d'ORAKLE). */
export const VOCABULARY = 'ARKLIGHT, projecteur, SkyPanel, S30, S60, S120, S360, Orbiter, L10, M18, Titan, Helios, Hyperion, Astera, AX9, Aputure, Nova, Hive, Kino Flo, Celeb, DMG, LiteMat, Dedolight, DMX, univers, adresse, fixture, groupe, face, contre, débouchage, contre-jour, cadre, drapeau, floppy, réflecteur, Ultrabounce, pied, Kelvin, teinte, saturation, intensité, brume, cyclo, comédien, comédienne, jardin, cour, plan, séquence.';

const whisperDir = (dir) => join(dir, 'whisper');

/** Programme whisper.cpp installé (recherche dans le dossier extrait), ou ''. */
export function findBinary(dir) {
  const root = join(whisperDir(dir), 'bin');
  if (!existsSync(root)) return '';
  const stack = [root];
  while (stack.length) {
    const d = stack.pop();
    let entries = [];
    try {
      entries = readdirSync(d);
    } catch {
      continue;
    }
    for (const name of BINARIES) if (entries.includes(name)) return join(d, name);
    for (const e of entries) {
      const p = join(d, e);
      try {
        if (statSync(p).isDirectory()) stack.push(p);
      } catch {
        /* ignoré */
      }
    }
  }
  return '';
}

const modelPath = (dir, model) => join(whisperDir(dir), 'models', (WHISPER_MODELS[model] || WHISPER_MODELS.base).file);

/** État : programme installé ? modèles présents ? */
export function voiceStatus(dir) {
  return { installed: !!findBinary(dir), models: Object.keys(WHISPER_MODELS).filter((m) => existsSync(modelPath(dir, m))), available: WHISPER_MODELS };
}

const mb = (n) => `${Math.round(n / 1e6)} Mo`;

async function download(url, dest, label, onProgress, signal) {
  mkdirSync(dirname(dest), { recursive: true });
  const res = await fetch(url, { signal, redirect: 'follow', headers: { 'user-agent': 'ARKLIGHT' } });
  if (!res.ok || !res.body) throw new Error(`Téléchargement impossible (${res.status}) : ${label}`);
  const total = Number(res.headers.get('content-length')) || 0;
  const part = `${dest}.part`;
  const out = createWriteStream(part);
  let done = 0;
  let last = 0;
  try {
    for await (const chunk of res.body) {
      if (!out.write(chunk)) await new Promise((r) => out.once('drain', r));
      done += chunk.length;
      if (Date.now() - last > 400) {
        last = Date.now();
        onProgress({ pct: total ? Math.round((100 * done) / total) : null, text: `${label} — ${mb(done)}${total ? ` / ${mb(total)}` : ''}` });
      }
    }
  } finally {
    await new Promise((r) => out.end(r));
  }
  if (signal?.aborted) {
    rmSync(part, { force: true });
    throw new Error('Annulé');
  }
  renameSync(part, dest);
}

/**
 * Installe la reconnaissance vocale : whisper.cpp (version officielle pour Windows, GitHub
 * ggml-org/whisper.cpp) puis le modèle choisi (Hugging Face, ggerganov/whisper.cpp).
 */
export async function installVoice(dir, model, onProgress, signal) {
  const root = whisperDir(dir);
  mkdirSync(root, { recursive: true });
  if (!findBinary(dir)) {
    onProgress({ pct: null, text: 'whisper.cpp — recherche de la dernière version…' });
    const res = await fetch(RELEASES_URL, { signal, headers: { accept: 'application/vnd.github+json', 'user-agent': 'ARKLIGHT' } });
    if (!res.ok) throw new Error(`whisper.cpp : versions indisponibles (${res.status})`);
    const rel = await res.json();
    const assets = rel.assets || [];
    const asset = assets.find((a) => /^whisper-bin-x64\.zip$/i.test(a.name)) || assets.find((a) => /bin-x64\.zip$/i.test(a.name) && !/cublas|cuda|blas|vulkan/i.test(a.name));
    if (!asset) throw new Error('whisper.cpp : archive pour Windows introuvable');
    const zip = join(root, asset.name);
    await download(asset.browser_download_url, zip, `whisper.cpp ${rel.tag_name || ''}`.trim(), onProgress, signal);
    onProgress({ pct: null, text: 'whisper.cpp — extraction…' });
    const bin = join(root, 'bin');
    for (const f of readZip(readFileSync(zip), { inflate: (d) => inflateRawSync(d) })) {
      if (f.name.endsWith('/')) continue;
      const out = normalize(join(bin, ...f.name.split('/')));
      if (!out.startsWith(bin + sep)) continue; // chemin hors du dossier : ignoré
      mkdirSync(dirname(out), { recursive: true });
      writeFileSync(out, f.data);
    }
    rmSync(zip, { force: true });
    if (!findBinary(dir)) throw new Error('whisper.cpp : programme introuvable dans l’archive');
  }
  const m = WHISPER_MODELS[model] ? model : 'base';
  if (!existsSync(modelPath(dir, m))) await download(MODEL_URL(WHISPER_MODELS[m].file), modelPath(dir, m), `Whisper ${m}`, onProgress, signal);
}

/** Transcrit un fichier WAV (16 kHz mono). lang : 'auto', 'fr', 'en'… */
export function transcribe({ dir, model = 'base', wav, lang = 'auto', prompt = '' }) {
  const bin = findBinary(dir);
  if (!bin) return Promise.reject(new Error('Reconnaissance vocale non installée'));
  const m = existsSync(modelPath(dir, model)) ? model : Object.keys(WHISPER_MODELS).find((x) => existsSync(modelPath(dir, x)));
  if (!m) return Promise.reject(new Error('Modèle de reconnaissance vocale manquant'));
  const args = ['-m', modelPath(dir, m), '-f', wav, '-l', lang || 'auto', '-nt', '-np', '-t', String(Math.max(2, Math.min(8, cpus().length - 2)))];
  if (prompt) args.push('--prompt', prompt.slice(0, 800));
  return new Promise((resolve, reject) => {
    const p = spawn(bin, args, { windowsHide: true, cwd: dirname(bin) });
    let out = '';
    let err = '';
    const timer = setTimeout(() => p.kill(), 60_000);
    p.stdout.on('data', (d) => (out += d));
    p.stderr.on('data', (d) => (err += d));
    p.on('error', (e) => {
      clearTimeout(timer);
      reject(e);
    });
    p.on('close', (code) => {
      clearTimeout(timer);
      if (code !== 0) return reject(new Error(`Transcription impossible : ${err.trim().split('\n').slice(-2).join(' ').slice(0, 200)}`));
      // sans horodatage : une ligne par segment ; on retire les marques du type [BLANK_AUDIO]
      const text = out
        .replace(/\[[^\]]*\]|\([^)]*musique[^)]*\)/gi, ' ')
        .replace(/\s+/g, ' ')
        .trim();
      resolve(text);
    });
  });
}
