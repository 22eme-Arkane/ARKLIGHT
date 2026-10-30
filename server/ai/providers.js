// Fournisseurs d'IA de l'assistant : Ollama (local et gratuit, par défaut), AIODOS (l'IA locale
// d'ARKANE, voir aiodos.js), serveurs locaux compatibles OpenAI (LM Studio, llama.cpp, Jan…), et
// API en ligne avec la clé de l'utilisateur (Anthropic, OpenAI, Mistral, Google Gemini, ou toute
// API compatible OpenAI).
//
// Messages dans un format neutre, convertis pour chaque fournisseur :
//   { role: 'user', content, images?: [data URL] }
//   { role: 'assistant', content, toolCalls?: [{ id, name, args }] }
//   { role: 'tool', toolCallId, name, content }

export const PROVIDERS = {
  ollama: { label: 'Ollama (local, gratuit)', kind: 'ollama', local: true, url: 'http://localhost:11434' },
  aiodos: { label: 'AIODOS (IA locale d’ARKANE)', kind: 'aiodos', local: true, url: '' },
  local: { label: 'Serveur local compatible OpenAI (LM Studio, llama.cpp, Jan…)', kind: 'openai', local: true, url: 'http://localhost:1234/v1' },
  anthropic: { label: 'Anthropic (Claude)', kind: 'anthropic', url: 'https://api.anthropic.com/v1', key: true, defaultModel: 'claude-sonnet-5-5' },
  openai: { label: 'OpenAI (GPT)', kind: 'openai', url: 'https://api.openai.com/v1', key: true },
  mistral: { label: 'Mistral', kind: 'openai', url: 'https://api.mistral.ai/v1', key: true, defaultModel: 'mistral-small-latest' },
  gemini: { label: 'Google Gemini', kind: 'openai', url: 'https://generativelanguage.googleapis.com/v1beta/openai', key: true },
  custom: { label: 'Autre API compatible OpenAI', kind: 'openai', url: '', key: true },
};

const TIMEOUT_MS = 240_000; // une réponse

// AIODOS : le client créé par le serveur (clé, langue), voir aiodos.js
let aiodos = null;
export const useAiodos = (a) => (aiodos = a);
const LOAD_TIMEOUT_MS = 900_000; // premier chargement d'un gros modèle local (plusieurs minutes sur un portable)

/** Adresse du fournisseur (réglage de l'utilisateur, sinon celle par défaut). */
export function providerUrl(id, cfg = {}) {
  const p = PROVIDERS[id];
  const custom = cfg.urls?.[id];
  return String((typeof custom === 'string' && custom.trim()) || p?.url || '').replace(/\/+$/, '');
}

async function request(url, { method = 'GET', headers = {}, body, signal, timeout = TIMEOUT_MS } = {}) {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(new Error('Délai dépassé')), timeout);
  const onAbort = () => ctrl.abort(signal.reason);
  signal?.addEventListener('abort', onAbort, { once: true });
  try {
    const res = await fetch(url, { method, headers: { ...(body ? { 'content-type': 'application/json' } : {}), ...headers }, body: body ? JSON.stringify(body) : undefined, signal: ctrl.signal });
    const text = await res.text();
    let data = null;
    try {
      data = text ? JSON.parse(text) : null;
    } catch {
      data = null;
    }
    if (!res.ok) {
      const msg = data?.error?.message || data?.error || data?.message || text.slice(0, 300) || res.statusText;
      const err = new Error(`${res.status} — ${typeof msg === 'string' ? msg : JSON.stringify(msg)}`);
      err.status = res.status;
      throw err;
    }
    return data;
  } catch (e) {
    if (e.name === 'AbortError' || ctrl.signal.aborted) throw signal?.aborted ? new Error('Arrêté') : new Error('Délai dépassé : le modèle ne répond pas');
    if (e.cause?.code === 'ECONNREFUSED') throw new Error(`Connexion refusée (${new URL(url).host}) : le serveur ne tourne pas`);
    throw e;
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener('abort', onAbort);
  }
}

const dataUrlParts = (u) => {
  const m = /^data:([^;]+);base64,(.*)$/s.exec(String(u || ''));
  return m ? { mime: m[1], data: m[2] } : null;
};

/** Arguments d'un appel d'outil : objet, ou texte JSON (parfois mal formé chez les petits modèles). */
function parseArgs(a) {
  if (a && typeof a === 'object') return a;
  try {
    return JSON.parse(a || '{}');
  } catch {
    return {};
  }
}

/**
 * Appels d'outils écrits dans le texte par certains modèles locaux (au lieu du champ prévu) :
 * {"name": "...", "arguments": {...}} ou <tool_call>{...}</tool_call>.
 */
export function toolCallsInText(text, toolNames) {
  const out = [];
  const src = String(text || '');
  const candidates = [...src.matchAll(/<tool_call>\s*([\s\S]*?)\s*<\/tool_call>/g)].map((m) => m[1]);
  if (!candidates.length) {
    const trimmed = src.trim().replace(/^```(?:json)?\s*|\s*```$/g, '');
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) candidates.push(trimmed);
  }
  for (const c of candidates) {
    let v;
    try {
      v = JSON.parse(c);
    } catch {
      continue;
    }
    for (const it of Array.isArray(v) ? v : [v]) {
      const name = it?.name || it?.function?.name;
      if (name && toolNames.has(name)) out.push({ id: `t${out.length + 1}`, name, args: parseArgs(it.arguments ?? it.parameters ?? it.function?.arguments) });
    }
  }
  return out;
}

// ------------------------------------------------------------------ Ollama
/** Le modèle est-il déjà en mémoire dans Ollama ? (null : on ne sait pas) */
async function ollamaLoaded(url, model) {
  try {
    const ps = await request(`${url}/api/ps`, { timeout: 4000 });
    const names = (ps?.models || []).flatMap((m) => [m.name, m.model]);
    return names.includes(model) || names.includes(`${model}:latest`);
  } catch {
    return null;
  }
}

async function chatOllama({ url, model, system, messages, tools, signal, think, onLoading }) {
  // pas encore chargé : on prévient (« chargement du modèle… ») et on lui laisse le temps
  const loaded = await ollamaLoaded(url, model);
  if (loaded === false) onLoading?.();
  const msgs = [{ role: 'system', content: system }];
  for (const m of messages) {
    if (m.role === 'tool') msgs.push({ role: 'tool', content: m.content, tool_name: m.name });
    else if (m.role === 'assistant') msgs.push({ role: 'assistant', content: m.content || '', ...(m.toolCalls?.length ? { tool_calls: m.toolCalls.map((c) => ({ function: { name: c.name, arguments: c.args } })) } : {}) });
    else msgs.push({ role: 'user', content: m.content, ...(m.images?.length ? { images: m.images.map((u) => dataUrlParts(u)?.data).filter(Boolean) } : {}) });
  }
  const body = {
    model,
    messages: msgs,
    stream: false,
    keep_alive: '15m',
    options: { temperature: 0.2, num_ctx: 16384 },
    ...(tools?.length ? { tools: tools.map((t) => ({ type: 'function', function: t })) } : {}),
    ...(think === false ? { think: false } : {}),
  };
  const data = await request(`${url}/api/chat`, { method: 'POST', body, signal, timeout: loaded === false ? LOAD_TIMEOUT_MS : TIMEOUT_MS });
  const msg = data?.message || {};
  const toolCalls = (msg.tool_calls || []).map((c, i) => ({ id: `t${i + 1}`, name: c.function?.name, args: parseArgs(c.function?.arguments) })).filter((c) => c.name);
  return { text: msg.content || '', toolCalls };
}

// ------------------------------------------------------------------ compatible OpenAI (OpenAI, Mistral, Gemini, LM Studio…)
async function chatOpenAI({ id, url, key, model, system, messages, tools, signal }) {
  const msgs = [{ role: 'system', content: system }];
  for (const m of messages) {
    if (m.role === 'tool') msgs.push({ role: 'tool', tool_call_id: m.toolCallId, content: m.content });
    else if (m.role === 'assistant')
      msgs.push({
        role: 'assistant',
        content: m.content || (m.toolCalls?.length ? null : ''),
        ...(m.toolCalls?.length ? { tool_calls: m.toolCalls.map((c) => ({ id: c.id, type: 'function', function: { name: c.name, arguments: JSON.stringify(c.args || {}) } })) } : {}),
      });
    else msgs.push({ role: 'user', content: m.images?.length ? [{ type: 'text', text: m.content }, ...m.images.map((u) => ({ type: 'image_url', image_url: { url: u } }))] : m.content });
  }
  const body = {
    model,
    messages: msgs,
    ...(tools?.length ? { tools: tools.map((t) => ({ type: 'function', function: t })), tool_choice: 'auto' } : {}),
    // les modèles « à raisonnement » d'OpenAI n'acceptent que la température par défaut
    ...(id === 'openai' || id === 'gemini' ? {} : { temperature: 0.2 }),
  };
  const data = await request(`${url}/chat/completions`, { method: 'POST', headers: key ? { authorization: `Bearer ${key}` } : {}, body, signal });
  const msg = data?.choices?.[0]?.message || {};
  const text = Array.isArray(msg.content) ? msg.content.map((c) => c.text || '').join('') : msg.content || '';
  const toolCalls = (msg.tool_calls || []).map((c, i) => ({ id: c.id || `t${i + 1}`, name: c.function?.name, args: parseArgs(c.function?.arguments) })).filter((c) => c.name);
  return { text, toolCalls };
}

// ------------------------------------------------------------------ Anthropic
async function chatAnthropic({ url, key, model, system, messages, tools, signal }) {
  const msgs = [];
  const push = (role, blocks) => {
    const last = msgs.at(-1);
    if (last && last.role === role) last.content.push(...blocks);
    else msgs.push({ role, content: blocks });
  };
  for (const m of messages) {
    if (m.role === 'tool') push('user', [{ type: 'tool_result', tool_use_id: m.toolCallId, content: m.content }]);
    else if (m.role === 'assistant') {
      const blocks = [];
      if (m.content) blocks.push({ type: 'text', text: m.content });
      for (const c of m.toolCalls || []) blocks.push({ type: 'tool_use', id: c.id, name: c.name, input: c.args || {} });
      if (blocks.length) push('assistant', blocks);
    } else {
      const images = (m.images || []).map(dataUrlParts).filter(Boolean).map((p) => ({ type: 'image', source: { type: 'base64', media_type: p.mime, data: p.data } }));
      push('user', [...images, { type: 'text', text: m.content || '…' }]);
    }
  }
  const body = { model, max_tokens: 4096, system, messages: msgs, ...(tools?.length ? { tools: tools.map((t) => ({ name: t.name, description: t.description, input_schema: t.parameters })) } : {}) };
  const data = await request(`${url}/messages`, { method: 'POST', headers: { 'x-api-key': key, 'anthropic-version': '2023-06-01' }, body, signal });
  const blocks = data?.content || [];
  return {
    text: blocks.filter((b) => b.type === 'text').map((b) => b.text).join('\n'),
    toolCalls: blocks.filter((b) => b.type === 'tool_use').map((b) => ({ id: b.id, name: b.name, args: b.input || {} })),
  };
}

/**
 * Un tour de conversation avec le modèle choisi.
 * @returns {Promise<{ text: string, toolCalls: Array<{ id, name, args }> }>}
 */
export async function chat({ provider, model, key, cfg = {}, system, messages, tools = [], signal, think, onLoading }) {
  const p = PROVIDERS[provider];
  if (!p) throw new Error(`Fournisseur inconnu : ${provider}`);
  if (!model) throw new Error('Choisis un modèle dans les réglages de l’assistant');
  if (p.key && !key && !(provider === 'custom' && /localhost|127\.0\.0\.1/.test(providerUrl(provider, cfg)))) throw new Error(`Clé API ${p.label} manquante : ajoute-la dans les réglages de l’assistant`);
  if (p.kind === 'aiodos') {
    if (!aiodos) throw new Error('AIODOS indisponible');
    return aiodos.chat({ model, system, messages, tools, signal, onLoading });
  }
  const url = providerUrl(provider, cfg);
  if (!url) throw new Error('Adresse du serveur manquante');
  let res;
  if (p.kind === 'ollama') res = await chatOllama({ url, model, system, messages, tools, signal, think, onLoading });
  else if (p.kind === 'anthropic') res = await chatAnthropic({ url, key, model, system, messages, tools, signal });
  else res = await chatOpenAI({ id: provider, url, key, model, system, messages, tools, signal });
  // certains modèles locaux écrivent l'appel d'outil dans le texte
  if (!res.toolCalls.length && tools.length && /"name"\s*:|<tool_call>/.test(res.text)) {
    const found = toolCallsInText(res.text, new Set(tools.map((t) => t.name)));
    if (found.length) res = { text: '', toolCalls: found };
  }
  return res;
}

/** Modèles disponibles chez un fournisseur (pour la liste des réglages). */
export async function listModels({ provider, key, cfg = {}, timeout = 15_000 }) {
  const p = PROVIDERS[provider];
  if (!p) return [];
  if (p.kind === 'aiodos') return aiodos ? aiodos.models() : [];
  const url = providerUrl(provider, cfg);
  if (p.kind === 'ollama') {
    const data = await request(`${url}/api/tags`, { timeout });
    return (data?.models || [])
      .filter((m) => !m.capabilities || m.capabilities.includes('completion'))
      .map((m) => ({ id: m.model || m.name, size: m.size || 0, caps: m.capabilities || [], params: m.details?.parameter_size || '' }));
  }
  if (p.key && !key && provider !== 'custom') return [];
  if (p.kind === 'anthropic') {
    const data = await request(`${url}/models?limit=100`, { headers: { 'x-api-key': key, 'anthropic-version': '2023-06-01' }, timeout });
    return (data?.data || []).map((m) => ({ id: m.id, caps: ['tools', 'vision'] })).filter((m) => m.id.startsWith('claude-'));
  }
  const data = await request(`${url}/models`, { headers: key ? { authorization: `Bearer ${key}` } : {}, timeout });
  const skip = /embed|whisper|tts|audio|image|dall-e|moderation|realtime|transcribe|search|rerank|ocr/i;
  return (data?.data || [])
    .map((m) => String(m.id || '').replace(/^models\//, ''))
    .filter((id) => id && !skip.test(id))
    .sort((a, b) => a.localeCompare(b))
    .map((id) => ({ id, caps: [] }));
}
