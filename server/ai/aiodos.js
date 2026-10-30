// AIODOS, l'IA locale d'ARKANE : un moteur de l'assistant, à côté d'Ollama et des API en ligne.
// Le client officiel (aiodos-client, Apache-2.0, copie épinglée dans vendor/) vit ici, dans le
// serveur, jamais dans l'interface. ARKLIGHT n'a accès à AIODOS qu'après l'accord de
// l'utilisateur : bouton « Se connecter à AIODOS », code à 6 chiffres à comparer dans AIODOS ;
// la clé d'ARKLIGHT est alors rangée avec les autres clés (chiffrée par Windows dans
// l'application de bureau). Rien n'est demandé au lancement, rien n'est téléchargé.
import { createClient, pickModel } from 'aiodos-client';

/** Droits demandés : la conversation (outils et photos compris). */
export const AIODOS_SCOPES = ['chat'];
/** Où calculer : ce PC (les outils et les images ne se calculent qu'ici de toute façon). */
export const AIODOS_WHERE = ['here'];
const PURPOSE = {
  fr: 'Assistant lumière : comprendre les demandes, lire les plans dessinés et agir sur le plan de feu',
  en: 'Lighting assistant: understand requests, read drawn plots and act on the lighting plot',
};

/** Arguments d'un appel d'outil illisible pour le client : on tente le texte brut. */
function argsOf(call) {
  if (call.args && typeof call.args === 'object') return call.args;
  try {
    return JSON.parse(call.raw || '{}');
  } catch {
    return {};
  }
}

/**
 * @param {object} o
 * @param {{ get(id: string): string, set(id: string, v: string): void }} o.keys  coffre des clés d'ARKLIGHT
 * @param {string} o.version  version d'ARKLIGHT
 * @param {() => string} o.lang  langue de l'interface (« fr » ou « en »)
 * @param {object} [o.test]  options du client pour les essais (fetch, env, platform)
 */
export function createAiodos({ keys, version, lang, test = {} }) {
  const client = createClient({
    app: 'ARKLIGHT',
    appId: 'arkane.arklight',
    version,
    key: { get: () => keys.get('aiodos') || null, set: (k) => keys.set('aiodos', k || '') },
    lang: () => (lang() === 'en' ? 'en' : 'fr'),
    ...test,
  });
  let pending = null; // demande d'accès en cours : { ctrl, code }

  return {
    client,

    /** État pour les réglages de l'assistant (jamais la clé). Ne rejette jamais. */
    async status() {
      const st = await client.status();
      return {
        state: pending ? 'pending' : st.state,
        message: st.message,
        version: st.version,
        installed: st.installed,
        running: st.running,
        scopes: st.scopes || [],
        where: st.where || [],
        pending: !!pending,
        code: pending?.code || null,
      };
    },

    /**
     * Demande l'accès (sur un clic seulement) : `onCode(code)` reçoit le code à 6 chiffres à
     * afficher en grand ; la promesse se résout quand AIODOS a accordé l'accès.
     */
    async connect({ onCode }) {
      if (pending) return;
      const ctrl = new AbortController();
      pending = { ctrl, code: null };
      try {
        await client.connect({
          scopes: AIODOS_SCOPES,
          where: AIODOS_WHERE,
          purpose: PURPOSE[lang() === 'en' ? 'en' : 'fr'],
          signal: ctrl.signal,
          onCode: (code) => {
            if (pending) pending.code = code;
            onCode?.(code);
          },
        });
      } finally {
        pending = null;
      }
    },

    /** Abandonne la demande en cours. */
    cancel() {
      pending?.ctrl.abort();
    },

    /** Retire l'accès dans AIODOS et efface la clé. */
    disconnect: () => client.disconnect(),

    /** Lance AIODOS (installé, arrêté) et attend qu'il réponde. */
    async launch() {
      await client.launch();
      return client.waitReady({ timeout: 60_000 }).catch(() => null);
    },

    /** Modèles de conversation, au format de la liste des réglages. */
    async models() {
      const list = await client.models({ kind: 'chat' });
      return list.map((m) => ({ id: m.id, size: m.size || 0, caps: m.caps || [], params: m.params || '', machine: m.machine || '', here: m.here !== false, loaded: !!m.loaded, default: !!m.default, context: m.context ?? null }));
    },

    /** Le modèle conseillé : de ce PC, qui sait utiliser les outils, et qui voit les photos si possible. */
    async pickModel() {
      const m = pickModel(await this.models(), { needs: ['tools'], vision: true });
      return m?.id || '';
    },

    /** Un tour de conversation (format neutre de providers.js) ; `by` : qui a calculé. */
    async chat({ model, system, messages, tools, signal, onLoading }) {
      const r = await client.chat({ model, system, messages, tools, think: false, temperature: 0.2, where: 'here', signal, onLoading: () => onLoading?.() });
      return {
        text: r.text || '',
        toolCalls: (r.toolCalls || []).filter((c) => c.name).map((c, i) => ({ id: c.id || `t${i + 1}`, name: c.name, args: argsOf(c) })),
        by: { model: r.answeredBy || model, machine: r.machine || '', place: r.place || '' },
      };
    },
  };
}
