// Onglet « Assistant IA » du panneau du bas : conversation avec l'assistant (il agit dans le plan
// ouvert), et ses réglages — moteur local (Ollama) ou en ligne (clé API), modèle, installation.
import { t } from '../i18n.js';
import { h, toast } from './dom.js';
import { iconSvg } from './icons.js';

/** Nom lisible des outils (lignes d'action dans la conversation). */
const TOOL_LABELS = {
  get_scene: 'Lecture du plan',
  search_catalog: 'Bibliothèque',
  add_fixtures: 'Ajout de projecteurs',
  update_fixtures: 'Réglage des projecteurs',
  readdress: 'Adressage DMX',
  add_objects: 'Ajout au décor',
  update_objects: 'Décor',
  remove_items: 'Suppression',
  create_group: 'Groupe',
  set_environment: 'Ambiance',
  manage_plans: 'Séquences et plans',
  place_from_drawing: 'Plan dessiné',
  release_control: 'Reprise de la main',
};
const QUIET_TOOLS = new Set(['get_scene', 'search_catalog']);

const EXAMPLES = [
  'Reconstruis le plan de feu de cette photo (joins la photo d’un plan dessiné)',
  'Ajoute une face et un contre sur le comédien',
  'Ré-adresse tous les projecteurs à partir de 1, de gauche à droite',
  'Mets les tubes en bleu à 50 %',
  'Crée un nouveau plan pour le contrechamp',
];

const gb = (n) => `${(n / 1e9).toFixed(1).replace('.', ',')} Go`;

/** Réponse du modèle en Markdown simple (gras, listes, titres) → éléments, sans HTML injecté. */
function richText(text) {
  const out = [];
  for (const raw of String(text || '').replace(/\r/g, '').split('\n')) {
    const bullet = /^\s*([*\-•]|\d+[.)])\s+/.exec(raw);
    const line = raw.replace(/^\s*#{1,6}\s*/, '').replace(/^\s*([*\-•])\s+/, '');
    const el = h('div', { class: bullet ? 'ai-li' : 'ai-p' });
    if (bullet && !/^\d/.test(bullet[1])) el.append('• ');
    line.split(/(\*\*[^*]+\*\*|`[^`]+`)/).forEach((part) => {
      if (/^\*\*[^*]+\*\*$/.test(part)) el.append(h('b', {}, part.slice(2, -2)));
      else if (/^`[^`]+`$/.test(part)) el.append(h('code', {}, part.slice(1, -1)));
      else if (part) el.append(part.replace(/(^|\s)\*([^*]+)\*(?=\s|$)/g, '$1$2'));
    });
    if (raw.trim() || out.length) out.push(el);
  }
  return out;
}
const MAX_VOICE_S = 30;
const PHOTO_MAX = 1600; // côté le plus long envoyé au modèle (px)

/** Photo réduite pour le modèle (JPEG en data URL). */
async function photoForModel(file) {
  const bmp = await createImageBitmap(file);
  const k = Math.min(1, PHOTO_MAX / Math.max(bmp.width, bmp.height));
  const c = document.createElement('canvas');
  c.width = Math.round(bmp.width * k);
  c.height = Math.round(bmp.height * k);
  c.getContext('2d').drawImage(bmp, 0, 0, c.width, c.height);
  bmp.close?.();
  return { dataUrl: c.toDataURL('image/jpeg', 0.85), size: [c.width, c.height] };
}
const HOLD_MS = 350; // touche ou bouton tenu plus longtemps : « appuyer pour parler » ; sinon mains libres

/** WAV 16 bits mono. */
function encodeWav(samples, rate) {
  const buf = new ArrayBuffer(44 + samples.length * 2);
  const v = new DataView(buf);
  const w = (o, str) => [...str].forEach((c, i) => v.setUint8(o + i, c.charCodeAt(0)));
  w(0, 'RIFF');
  v.setUint32(4, 36 + samples.length * 2, true);
  w(8, 'WAVE');
  w(12, 'fmt ');
  v.setUint32(16, 16, true);
  v.setUint16(20, 1, true);
  v.setUint16(22, 1, true);
  v.setUint32(24, rate, true);
  v.setUint32(28, rate * 2, true);
  v.setUint16(32, 2, true);
  v.setUint16(34, 16, true);
  w(36, 'data');
  v.setUint32(40, samples.length * 2, true);
  for (let i = 0; i < samples.length; i++) {
    const x = Math.max(-1, Math.min(1, samples[i]));
    v.setInt16(44 + i * 2, x < 0 ? x * 0x8000 : x * 0x7fff, true);
  }
  return new Blob([buf], { type: 'audio/wav' });
}

/**
 * Micro → WAV 16 kHz mono pour Whisper. Rien n'est jamais joué : l'enregistrement est décodé et
 * rééchantillonné hors ligne (OfflineAudioContext), sans sortie vers les haut-parleurs.
 */
function createRecorder() {
  let stream = null;
  let rec = null;
  let chunks = [];
  const release = () => {
    stream?.getTracks().forEach((tr) => tr.stop());
    stream = null;
  };
  return {
    get active() {
      return !!rec;
    },
    async start() {
      stream = await navigator.mediaDevices.getUserMedia({ audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true } });
      chunks = [];
      rec = new MediaRecorder(stream);
      rec.ondataavailable = (e) => e.data.size && chunks.push(e.data);
      rec.start(250);
    },
    async stop() {
      if (!rec) return null;
      const r = rec;
      rec = null;
      await new Promise((res) => {
        r.onstop = res;
        r.stop();
      });
      release();
      const blob = new Blob(chunks, { type: r.mimeType });
      const audio = await new OfflineAudioContext(1, 1, 16000).decodeAudioData(await blob.arrayBuffer());
      if (audio.duration < 0.3) return null;
      const n = audio.length;
      const mono = new Float32Array(n);
      for (let c = 0; c < audio.numberOfChannels; c++) {
        const d = audio.getChannelData(c);
        for (let i = 0; i < n; i++) mono[i] += d[i] / audio.numberOfChannels;
      }
      return encodeWav(mono, audio.sampleRate);
    },
    cancel() {
      if (rec) {
        rec.onstop = null;
        try {
          rec.stop();
        } catch {
          /* déjà arrêté */
        }
      }
      rec = null;
      release();
    },
  };
}

export function mountAssistant({ store, net, desktop = false, keyOf = () => '', onShown, onDrawingDone }) {
  const body = document.querySelector('[data-body="assistant"]');
  let state = null; // réglages et état (serveur)
  const models = {}; // fournisseur → [{ id, caps, size }]
  const modelsError = {}; // fournisseur → liste indisponible (raison)
  let busy = false;
  let thinking = null;
  let progress = null; // téléchargement : { kind, pct, text, error, done }
  let sideOpen = false;
  let local = true; // interface ouverte sur l'ordinateur d'ARKLIGHT (sinon : pas d'assistant)
  const aiodosReq = { code: null, error: '' }; // demande d'accès à AIODOS en cours : code à comparer

  const log = h('div', { class: 'ai-log', 'aria-live': 'polite' });
  const input = h('textarea', { class: 'ai-input', rows: 2, spellcheck: 'false', placeholder: t('Demande à l’assistant… ex. « ajoute 4 Titan en arc derrière le comédien »') });
  const sendBtn = h('button', { class: 'primary ai-send', onclick: () => send() }, t('Envoyer'));
  const stopBtn = h('button', { class: 'ai-stop hidden', onclick: () => net.send({ t: 'ai.stop' }) }, t('Arrêter'));
  // micro : maintenir pour parler, ou appui bref pour parler librement (nouvel appui : terminé)
  const mic = h('button', { class: 'ib ai-mic hidden', html: iconSvg('mic', 18) });
  mic.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    voicePress('mic');
  });
  mic.addEventListener('pointerup', () => voiceRelease('mic'));
  mic.addEventListener('pointerleave', () => recorder.active && voice.code === 'mic' && Date.now() - voice.at > HOLD_MS && voiceRelease('mic'));
  const recorder = createRecorder();
  const voice = { code: null, at: 0, timer: null, busy: false };
  // photo jointe (plan de feu dessiné, repérage…) : { file, dataUrl, underlay }
  let photo = null;
  // « Réinterpréter un plan dessiné » : la photo choisie part tout de suite, puis le plan 2D s'ouvre
  let drawing = { waitFile: false, sent: false };
  const fileInput = h('input', { type: 'file', accept: 'image/png,image/jpeg,image/webp', class: 'hidden', onchange: (e) => attach(e.target.files[0]) });
  fileInput.addEventListener('cancel', () => (drawing.waitFile = false));
  const photoBtn = h('button', { class: 'ib ai-photo', title: t('Joindre une photo (plan de feu dessiné à la main, repérage…)'), html: iconSvg('photo', 18), onclick: () => fileInput.click() });
  const chip = h('div', { class: 'ai-chip hidden' });
  const engine = h('button', { class: 'ai-engine', title: t('Moteur et réglages de l’assistant'), onclick: () => toggleSide() });
  const side = h('aside', { class: 'ai-side hidden' });
  body.append(
    h(
      'div',
      { class: 'ai-wrap' },
      h(
        'div',
        { class: 'ai-chat' },
        h('div', { class: 'ai-bar' }, engine, h('span', { class: 'spacer' }), h('button', { class: 'ib', title: t('Nouvelle conversation'), onclick: () => net.send({ t: 'ai.reset' }) }, t('Nouvelle conversation'))),
        log,
        chip,
        h('div', { class: 'ai-input-row' }, mic, photoBtn, fileInput, input, h('div', { class: 'ai-buttons' }, sendBtn, stopBtn)),
      ),
      side,
    ),
  );

  input.addEventListener('keydown', (e) => {
    e.stopPropagation(); // les touches de la vue 3D ne réagissent pas pendant la saisie
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  });

  // ------------------------------------------------------------------ conversation
  const scrollDown = () => requestAnimationFrame(() => (log.scrollTop = log.scrollHeight));

  function addLine(cls, ...content) {
    const el = h('div', { class: `ai-msg ${cls}` }, ...content);
    log.append(el);
    scrollDown();
    return el;
  }

  function renderEmpty() {
    if (log.children.length) return;
    log.append(
      h(
        'div',
        { class: 'ai-empty' },
        h('p', {}, t('Décris ce que tu veux : l’assistant agit directement dans le plan ouvert (et Ctrl+Z annule son action).')),
        h('div', { class: 'ai-examples' }, EXAMPLES.map((ex) => h('button', { onclick: () => ((input.value = t(ex)), input.focus()) }, t(ex)))),
      ),
    );
  }

  function setBusy(on) {
    busy = on;
    sendBtn.classList.toggle('hidden', on);
    stopBtn.classList.toggle('hidden', !on);
    if (!on && thinking) {
      thinking.remove();
      thinking = null;
    }
  }

  async function attach(file) {
    fileInput.value = '';
    if (!file || !/^image\//.test(file.type)) return;
    try {
      const shot = await photoForModel(file);
      photo = { file, dataUrl: shot.dataUrl, size: shot.size, underlay: true };
    } catch (e) {
      photo = null;
      return toast(t('Image illisible : {0}', { 0: e.message }), 'error');
    }
    renderChip();
    input.focus();
    if (drawing.waitFile) {
      drawing.waitFile = false;
      input.value = '';
      drawing.sent = send();
    }
  }

  function renderChip() {
    chip.classList.toggle('hidden', !photo);
    if (!photo) return chip.replaceChildren();
    chip.replaceChildren(
      h('img', { src: photo.dataUrl, alt: '' }),
      h('span', { class: 'ai-chip-name' }, photo.file.name),
      h('label', { class: 'ai-check' }, h('input', { type: 'checkbox', checked: photo.underlay, onchange: (e) => (photo.underlay = e.target.checked) }), ` ${t('Aussi en fond du plan 2D')}`),
      h('button', { class: 'ib', title: t('Retirer la photo'), onclick: () => ((photo = null), renderChip()) }, '×'),
    );
  }

  /** Le modèle choisi voit-il les images ? (inconnu : on essaie) */
  function modelSeesImages() {
    const cfg = state?.config;
    const m = (models[cfg?.provider] || []).find((x) => x.id === cfg?.model);
    return !m?.caps?.length || m.caps.includes('vision');
  }

  async function underlayFrom(file) {
    try {
      const res = await fetch('/upload', { method: 'POST', headers: { 'Content-Type': file.type }, body: file });
      const json = await res.json();
      if (!res.ok) throw new Error(json.error || res.statusText);
      const img = new Image();
      img.src = json.url;
      await img.decode();
      net.send({ t: 'plot.meta', underlay: { url: json.url, x: 0, z: 0, width: 12, aspect: img.naturalWidth / img.naturalHeight, rotation: 0, opacity: 0.5, floor3d: false } });
    } catch (e) {
      toast(t('Fond du plan : {0}', { 0: e.message }), 'error');
    }
  }

  function send() {
    let text = input.value.trim();
    if ((!text && !photo) || busy || !local) return;
    if (!state) net.send({ t: 'ai.state' });
    if (photo && !modelSeesImages()) {
      toggleSide(true);
      return toast(t('Le modèle {0} ne voit pas les images : choisis-en un marqué « vision » (par exemple qwen3.6) dans les réglages', { 0: state?.config.model || '' }), 'error', 5000);
    }
    if (!text) text = t('Voici la photo d’un plan de feu dessiné à la main : reconstruis-le dans le plan ouvert (projecteurs, grip, décor, personnages, caméra).');
    log.querySelector('.ai-empty')?.remove();
    const line = addLine('user', text);
    const images = [];
    let imageSize = null;
    if (photo) {
      imageSize = photo.size;
      line.prepend(h('img', { class: 'ai-thumb', src: photo.dataUrl, alt: '' }));
      images.push(photo.dataUrl);
      if (photo.underlay) underlayFrom(photo.file);
      photo = null;
      renderChip();
    }
    input.value = '';
    setBusy(true);
    net.send({ t: 'ai.ask', text, images, imageSize });
    return true;
  }

  // coller ou glisser une image dans la conversation
  input.addEventListener('paste', (e) => {
    const item = [...(e.clipboardData?.items || [])].find((it) => it.type.startsWith('image/'));
    if (item) {
      e.preventDefault();
      attach(item.getAsFile());
    }
  });
  body.addEventListener('dragover', (e) => {
    if ([...(e.dataTransfer?.items || [])].some((it) => it.type.startsWith('image/'))) e.preventDefault();
  });
  body.addEventListener('drop', (e) => {
    const file = [...(e.dataTransfer?.files || [])].find((f) => f.type.startsWith('image/'));
    if (file) {
      e.preventDefault();
      attach(file);
    }
  });

  function onEvent(e) {
    if (e.kind === 'thinking') {
      thinking ||= addLine('thinking', h('span', { class: 'ai-dots' }), h('span', { class: 'ai-thinking-text' }, t('L’assistant réfléchit…')));
      const label = thinking.querySelector('.ai-thinking-text');
      if (label) label.textContent = e.loading ? t('Chargement du modèle en mémoire (la première fois, jusqu’à quelques minutes)…') : t('L’assistant réfléchit…');
      log.append(thinking); // toujours en bas
      scrollDown();
    } else if (e.kind === 'tool') {
      if (QUIET_TOOLS.has(e.name) && e.ok) return;
      addLine(`tool${e.ok ? '' : ' err'}`, `${e.ok ? '✓' : '✗'} ${t(TOOL_LABELS[e.name] || e.name)} — `, t(e.summary || ''));
    } else if (e.kind === 'note') {
      addLine('note', e.text);
    } else if (e.kind === 'reply') {
      thinking?.remove();
      thinking = null;
      const line = addLine('assistant', ...richText(e.text));
      // qui a calculé (AIODOS) : le modèle et la machine
      if (e.by?.model) line.append(h('div', { class: 'ai-by' }, t('Calculé par {0}', { 0: [e.by.model, e.by.machine].filter(Boolean).join(' · ') })));
    }
  }

  // ------------------------------------------------------------------ commande vocale
  const voiceReady = () => !!state?.voice?.installed && state.voice.models.length > 0;
  const placeholder = input.placeholder;

  function applyMic() {
    const on = !!state?.config.voice?.enabled;
    mic.classList.toggle('hidden', !on);
    mic.classList.toggle('rec', recorder.active);
    mic.classList.toggle('wait', voice.busy);
    mic.title = voiceReady() ? t('Parler à l’assistant : maintenir (ou {0}) pour parler, relâcher pour envoyer. Appui bref : parler librement, nouvel appui pour terminer.', { 0: keyOf('ai.voice') || 'F8' }) : t('Reconnaissance vocale à installer (réglages de l’assistant)');
    input.placeholder = recorder.active ? t('Je t’écoute… relâche pour envoyer') : voice.busy ? t('Transcription…') : placeholder;
  }

  async function voicePress(code) {
    if (!local) return;
    if (!state) net.send({ t: 'ai.state' });
    if (!state?.config.voice?.enabled || !voiceReady()) {
      toggleSide(true);
      return toast(t('Active et installe la commande vocale dans les réglages de l’assistant'), 'info', 3500);
    }
    if (voice.busy || busy) return;
    if (recorder.active) return finishVoice(); // mains libres : deuxième appui
    try {
      await recorder.start();
    } catch (e) {
      return toast(t('Micro indisponible : {0}', { 0: e.message }), 'error', 4000);
    }
    voice.code = code;
    voice.at = Date.now();
    clearTimeout(voice.timer);
    voice.timer = setTimeout(() => recorder.active && finishVoice(), MAX_VOICE_S * 1000);
    applyMic();
  }

  function voiceRelease(code) {
    if (!recorder.active || code !== voice.code) return;
    if (Date.now() - voice.at > HOLD_MS) finishVoice(); // appuyer pour parler
  }

  async function finishVoice() {
    clearTimeout(voice.timer);
    voice.busy = true;
    applyMic();
    try {
      const wav = await recorder.stop();
      if (!wav) return;
      const res = await fetch('/ai/voice', { method: 'POST', headers: { 'content-type': 'audio/wav' }, body: wav });
      const data = await res.json().catch(() => ({}));
      if (!res.ok) throw new Error(data.error || res.statusText);
      const text = String(data.text || '').trim();
      if (!text) return toast(t('Rien compris : parle un peu plus près du micro'), 'info', 3000);
      input.value = input.value.trim() ? `${input.value.trim()} ${text}` : text;
      if (state?.config.voice?.autoSend !== false) send();
      else input.focus();
    } catch (e) {
      toast(t('Commande vocale : {0}', { 0: e.message }), 'error', 4000);
    } finally {
      voice.busy = false;
      applyMic();
    }
  }

  function voiceBox() {
    const v = state.voice || {};
    const vc = state.config.voice || {};
    const els = [h('h4', {}, t('Commande vocale'))];
    els.push(h('label', { class: 'ai-check' }, h('input', { type: 'checkbox', checked: !!vc.enabled, onchange: (e) => setConfig({ voice: { enabled: e.target.checked } }) }), ` ${t('Activer ({0} : maintenir pour parler)', { 0: keyOf('ai.voice') || 'F8' })}`));
    if (!vc.enabled) return els;
    const modelSel = h(
      'select',
      { onchange: (e) => setConfig({ voice: { model: e.target.value } }) },
      Object.entries(v.available || {}).map(([id, m]) => h('option', { value: id, selected: id === vc.model }, `Whisper ${id} — ≈ ${Math.round(m.size * 1000)} Mo${(v.models || []).includes(id) ? ` ✓` : ''}`)),
    );
    if (!v.installed || !(v.models || []).includes(vc.model)) {
      els.push(h('p', { class: 'note' }, t('La parole est transcrite sur cet ordinateur par Whisper, comme dans ORAKLE : sans internet ni abonnement. À installer une fois (programme whisper.cpp et modèle).')));
      els.push(h('label', { class: 'ai-label' }, t('Modèle de reconnaissance')), modelSel);
      els.push(h('button', { class: 'primary', disabled: !!state.task || v.platform !== 'win32', onclick: () => net.send({ t: 'ai.voice.install' }) }, t('Installer la reconnaissance vocale')));
    } else {
      els.push(h('p', { class: 'note' }, t('Prête : Whisper {0}, sur cet ordinateur.', { 0: vc.model })));
      els.push(h('label', { class: 'ai-label' }, t('Modèle de reconnaissance')), modelSel);
      els.push(
        h('label', { class: 'ai-label' }, t('Langue parlée')),
        h('select', { onchange: (e) => setConfig({ voice: { lang: e.target.value } }) }, [['auto', 'Automatique'], ['fr', 'Français'], ['en', 'English'], ['es', 'Español']].map(([id, l]) => h('option', { value: id, selected: id === vc.lang }, t(l)))),
        h('label', { class: 'ai-check' }, h('input', { type: 'checkbox', checked: vc.autoSend !== false, onchange: (e) => setConfig({ voice: { autoSend: e.target.checked } }) }), ` ${t('Envoyer la demande dès que j’ai fini de parler')}`),
      );
    }
    return els;
  }

  // ------------------------------------------------------------------ réglages
  const providerOf = () => state?.providers.find((p) => p.id === state.config.provider);

  function applyEngine() {
    const p = providerOf();
    engine.replaceChildren(h('span', { class: 'ico', html: iconSvg('sparkle', 15) }), state ? `${t(shortLabel(p))} · ${state.config.model || t('aucun modèle')}` : t('Assistant IA'), h('span', { class: 'caret' }, sideOpen ? '▸' : '▾'));
  }
  const shortLabel = (p) => (p ? p.label.replace(/\s*\(.*\)$/, '') : '');

  function toggleSide(force) {
    sideOpen = force ?? !sideOpen;
    side.classList.toggle('hidden', !sideOpen);
    if (sideOpen) {
      net.send({ t: 'ai.state' });
      net.send({ t: 'ai.models', provider: state?.config.provider });
    }
    renderSide();
    applyEngine();
  }

  function setConfig(changes) {
    net.send({ t: 'ai.config', changes });
  }

  function renderSide() {
    if (!sideOpen) return;
    if (!state) return side.replaceChildren(h('p', { class: 'note' }, '…'));
    const cfg = state.config;
    const p = providerOf();
    const list = models[cfg.provider] || [];
    const els = [];
    els.push(h('h4', {}, t('Moteur')));
    els.push(h('select', { onchange: (e) => setConfig({ provider: e.target.value }) }, state.providers.map((x) => h('option', { value: x.id, selected: x.id === cfg.provider }, t(x.label)))));
    if (cfg.provider === 'aiodos') els.push(...aiodosBox()); // avant la liste des modèles : elle demande l'accès
    if (p?.key) {
      const has = state.keys.includes(p.id);
      const keyInput = h('input', { type: 'password', placeholder: has ? t('•••••• clé enregistrée') : t('Colle ta clé API ici'), autocomplete: 'off', spellcheck: 'false' });
      keyInput.addEventListener('keydown', (e) => e.stopPropagation());
      els.push(
        h('label', { class: 'ai-label' }, t('Clé API')),
        h('div', { class: 'ai-row' }, keyInput, h('button', { onclick: () => keyInput.value.trim() && net.send({ t: 'ai.key', provider: p.id, key: keyInput.value.trim() }) }, t('Enregistrer')), has ? h('button', { title: t('Effacer la clé'), onclick: () => net.send({ t: 'ai.key', provider: p.id, key: '' }) }, '×') : null),
        h('p', { class: 'note' }, state.encrypted ? t('Gardée sur cet ordinateur, chiffrée par Windows. Elle n’est jamais montrée ni envoyée ailleurs qu’au fournisseur choisi.') : t('Gardée sur cet ordinateur. Elle n’est jamais montrée ni envoyée ailleurs qu’au fournisseur choisi.')),
      );
    }
    if ((p?.local && p.id !== 'aiodos') || cfg.provider === 'custom') {
      const url = h('input', { value: p.url, spellcheck: 'false', onchange: (e) => setConfig({ urls: { [p.id]: e.target.value } }) });
      url.addEventListener('keydown', (e) => e.stopPropagation());
      els.push(h('label', { class: 'ai-label' }, t('Adresse du serveur')), url);
    }
    els.push(h('label', { class: 'ai-label' }, t('Modèle')));
    if (list.length) {
      const ids = list.map((m) => m.id);
      if (cfg.model && !ids.includes(cfg.model)) ids.unshift(cfg.model);
      els.push(
        h(
          'div',
          { class: 'ai-row' },
          h(
            'select',
            { onchange: (e) => setConfig({ model: e.target.value }) },
            ids.map((id) => {
              const m = list.find((x) => x.id === id);
              const caps = m?.caps?.length ? ` — ${[m.caps.includes('tools') ? t('outils') : null, m.caps.includes('vision') ? t('vision') : null].filter(Boolean).join(', ')}` : '';
              return h('option', { value: id, selected: id === cfg.model }, `${id}${m?.size ? ` (${gb(m.size)})` : ''}${caps}`);
            }),
          ),
          h('button', { class: 'ib', title: t('Actualiser la liste'), onclick: () => net.send({ t: 'ai.models', provider: cfg.provider }) }, '↻'),
        ),
      );
    } else {
      const mi = h('input', { value: cfg.model || '', placeholder: p?.defaultModel || t('nom du modèle'), spellcheck: 'false', onchange: (e) => setConfig({ model: e.target.value }) });
      mi.addEventListener('keydown', (e) => e.stopPropagation());
      els.push(h('div', { class: 'ai-row' }, mi, h('button', { class: 'ib', title: t('Actualiser la liste'), onclick: () => net.send({ t: 'ai.models', provider: cfg.provider }) }, '↻')));
      // AIODOS pas encore connecté : sa carte, plus haut, dit déjà quoi faire
      const quiet = cfg.provider === 'aiodos' && state.aiodos?.state !== 'ready';
      if (modelsError[cfg.provider] && !quiet) els.push(h('p', { class: 'note err' }, modelsError[cfg.provider]));
    }
    if (cfg.provider === 'ollama') els.push(...ollamaBox());
    if (cfg.provider === 'aiodos') els.push(...aiodosInfo());
    els.push(...voiceBox());
    const bar = progressBar(); // téléchargement en cours (modèle, Ollama, reconnaissance vocale)
    if (bar) els.push(bar);
    side.replaceChildren(...els);
  }

  function progressBar() {
    if (!progress || progress.done) return progress?.error ? h('p', { class: 'note err' }, t(progress.error)) : null;
    return h(
      'div',
      { class: 'ai-progress' },
      h('div', { class: 'ai-bar-track' }, h('i', { style: { width: `${progress.pct ?? 5}%` } })),
      h('div', { class: 'ai-row' }, h('span', { class: 'note' }, t(progress.text || '…')), h('button', { onclick: () => net.send({ t: 'ai.task.cancel' }) }, t('Annuler'))),
    );
  }

  function ollamaBox() {
    const o = state.ollama;
    const els = [h('h4', {}, 'Ollama')];
    if (!o.installed && !o.running) {
      els.push(h('p', { class: 'note' }, t('Ollama n’est pas installé : c’est le moteur gratuit qui fait tourner l’IA sur cet ordinateur, sans internet ni abonnement.')));
      els.push(h('button', { class: 'primary', disabled: !!state.task || !desktop, onclick: () => net.send({ t: 'ai.ollama.install' }) }, t('Installer Ollama (installeur officiel)')));
      if (!desktop) els.push(h('p', { class: 'note' }, t('Depuis l’application de bureau seulement.')));
    } else if (!o.running) {
      els.push(h('p', { class: 'note' }, t('Ollama est installé mais arrêté.')));
      els.push(h('button', { class: 'primary', onclick: () => net.send({ t: 'ai.ollama.start' }) }, t('Démarrer Ollama')));
    } else {
      els.push(h('p', { class: 'note' }, t('Ollama {0} en marche — {1} modèle(s) installé(s).', { 0: o.version, 1: o.models.length })));
      if (!o.models.some((m) => m.caps.includes('tools'))) els.push(h('p', { class: 'note warn' }, t('Aucun modèle installé ne sait utiliser les outils : télécharges-en un ci-dessous.')));
    }
    if (o.running) {
      const sel = h('select', {}, state.suggested.map((m) => h('option', { value: m.id }, `${m.id} — ≈ ${String(m.size).replace('.', ',')} Go`)), h('option', { value: '' }, t('Autre (nom ci-contre)…')));
      const other = h('input', { placeholder: t('ou un nom de modèle (ollama.com/library)'), spellcheck: 'false' });
      other.addEventListener('keydown', (e) => e.stopPropagation());
      const note = h('p', { class: 'note' }, t(state.suggested[0]?.note || ''));
      sel.addEventListener('change', () => (note.textContent = t(state.suggested.find((m) => m.id === sel.value)?.note || '')));
      els.push(
        h('label', { class: 'ai-label' }, t('Télécharger un modèle')),
        sel,
        note,
        other,
        h(
          'button',
          {
            disabled: !!state.task,
            onclick: () => {
              const model = other.value.trim() || sel.value;
              if (model) net.send({ t: 'ai.ollama.pull', model });
            },
          },
          t('Télécharger'),
        ),
      );
    }
    return els;
  }

  /** AIODOS : état, connexion par code à 6 chiffres, lancement, déconnexion. */
  function aiodosBox() {
    const a = state.aiodos || {};
    const els = [];
    const btn = (label, t2, primary = true) => h('button', { class: primary ? 'primary' : '', onclick: () => net.send({ t: t2 }) }, label);
    if (aiodosReq.code) {
      // demande en cours : le code en grand, à comparer avec celui de la fenêtre d'AIODOS
      els.push(
        h('div', { class: 'ai-aiodos' },
          h('p', { class: 'note' }, t('Dans la fenêtre d’AIODOS, vérifie que le code est le même, puis autorise ARKLIGHT :')),
          h('div', { class: 'ai-code', translate: 'no' }, String(aiodosReq.code).replace(/^(\d{3})(\d{3})$/, '$1 $2')),
          btn(t('Annuler'), 'ai.aiodos.cancel', false),
        ),
      );
      return els;
    }
    const box = h('div', { class: 'ai-aiodos' });
    box.append(h('p', { class: `note${a.state === 'ready' ? '' : ' warn'}` }, a.message || '…'));
    if (aiodosReq.error) box.append(h('p', { class: 'note err' }, aiodosReq.error));
    switch (a.state) {
      case 'ready':
        box.append(h('div', { class: 'ai-row' }, h('span', { class: 'note' }, t('Le calcul reste sur cet ordinateur.')), h('span', { class: 'spacer' }), btn(t('Se déconnecter d’AIODOS'), 'ai.aiodos.disconnect', false)));
        break;
      case 'not_running':
        box.append(btn(t('Lancer AIODOS'), 'ai.aiodos.launch'));
        break;
      case 'not_installed':
        box.append(h('p', { class: 'note' }, t('Installe AIODOS avec l’installeur d’ARKLIGHT (case « Installer aussi AIODOS »), puis reviens ici.')));
        break;
      case 'pending':
        box.append(btn(t('Annuler'), 'ai.aiodos.cancel', false));
        break;
      case 'gateway_closed':
      case 'apps_closed':
      case 'needs_access':
      case 'denied':
      case 'revoked':
        box.append(btn(t('Se connecter à AIODOS'), 'ai.aiodos.connect'));
        break;
      default:
        box.append(h('button', { onclick: () => net.send({ t: 'ai.state' }) }, t('Vérifier à nouveau')));
    }
    els.push(box);
    return els;
  }

  /** Ce qu'apporte AIODOS et ce qu'il reçoit d'ARKLIGHT. */
  function aiodosInfo() {
    return [
      h('h4', {}, t('À propos d’AIODOS')),
      h('p', { class: 'note' }, t('AIODOS relie par Internet les ordinateurs de personnes qui ne se connaissent pas, pour partager leur puissance de calcul. Une fois installé, il prête par défaut une part de ce PC au réseau public : 60 % de sa puissance au plus, jamais sur batterie ; tout se coupe dans ses Réglages.')),
      h('p', { class: 'note' }, t('Les demandes d’ARKLIGHT, elles, restent sur ce PC : elles sont calculées ici, par les modèles installés dans AIODOS (Réglages → Modèles).')),
      h('p', { class: 'note' }, t('ARKLIGHT lui envoie tes demandes, l’état du plan ouvert et les photos jointes (plan dessiné à la main) ; l’accès se retire d’un clic, ici ou dans AIODOS.')),
    ];
  }

  // ------------------------------------------------------------------ messages du serveur
  function onMessage(msg) {
    switch (msg.t) {
      case 'ai.state': {
        // autre moteur, ou AIODOS qui vient d'être connecté : sa liste de modèles
        const prev = state;
        state = msg;
        const p = msg.config.provider;
        if (sideOpen && (prev?.config.provider !== p || (p === 'aiodos' && prev?.aiodos?.state !== 'ready' && msg.aiodos?.state === 'ready'))) net.send({ t: 'ai.models', provider: p });
        if (state.task && !progress) progress = { kind: state.task, pct: null, text: '…' };
        applyEngine();
        applyMic();
        renderSide();
        break;
      }
      case 'ai.models':
        models[msg.provider] = msg.models || [];
        modelsError[msg.provider] = msg.error ? t('Liste indisponible : {0}', { 0: msg.error }) : '';
        renderSide();
        break;
      case 'ai.event':
        onEvent(msg);
        break;
      case 'ai.done':
        setBusy(false);
        if (msg.select?.length) store.setSelection(msg.select);
        if (drawing.sent) {
          drawing.sent = false;
          if (msg.select?.length) onDrawingDone?.();
        }
        break;
      case 'ai.error':
        setBusy(false);
        drawing.sent = false;
        addLine('error', t(msg.message));
        if (/Connexion refusée|ne tourne pas|Ollama/.test(msg.message) || (state?.config.provider === 'aiodos' && msg.code)) toggleSide(true);
        break;
      case 'ai.aiodos.code':
        // demande d'accès à AIODOS : le code à comparer, puis la fin (accordé, refusé, annulé)
        aiodosReq.code = msg.code || null;
        aiodosReq.error = msg.error || '';
        if (msg.done) toast(t('ARKLIGHT est connecté à AIODOS'), 'info', 3500);
        if (msg.code) toggleSide(true);
        renderSide();
        break;
      case 'ai.reset':
        log.replaceChildren();
        renderEmpty();
        break;
      case 'ai.progress':
        progress = msg;
        if (msg.done && msg.error) toast(t(msg.error), 'error', 4000);
        else if (msg.done) toast(msg.kind === 'pull' ? t('Modèle prêt') : msg.kind === 'voice' ? t('Commande vocale prête') : t('Installeur d’Ollama lancé'), 'info', 3500);
        renderSide();
        break;
    }
  }

  renderEmpty();
  applyEngine();

  /** Interface ouverte depuis un autre appareil (tablette…) : l'assistant reste sur l'ordinateur. */
  const chatWrap = body.firstElementChild;
  const remoteNote = h('div', { class: 'ai-empty' }, h('p', {}, t('L’assistant IA s’utilise sur l’ordinateur où ARKLIGHT tourne : il se sert de ses clés et de son IA locale, qu’un autre appareil du réseau ne doit pas emprunter.')));
  function setLocal(on) {
    if (on === local) return;
    local = on;
    body.replaceChildren(local ? chatWrap : remoteNote);
  }

  return {
    onMessage,
    setLocal,
    /** Choisir la photo d'un plan dessiné à la main : l'assistant le reconstruit (3D et plan 2D). */
    reinterpretDrawing() {
      if (!local || busy) return;
      drawing = { waitFile: true, sent: false };
      fileInput.click();
    },
    /** L'onglet s'ouvre : réglages à jour, saisie prête. */
    shown() {
      if (!local) return;
      if (!state) net.send({ t: 'ai.state' });
      onShown?.();
      setTimeout(() => input.focus(), 0);
    },
    focus: () => input.focus(),
    /** Touche « parler » (F8 par défaut) : enfoncée, relâchée ; menu : bascule. */
    voicePress: (code) => voicePress(code),
    voiceRelease: (code) => voiceRelease(code),
    voiceToggle: () => (recorder.active ? finishVoice() : voicePress('menu')),
  };
}
