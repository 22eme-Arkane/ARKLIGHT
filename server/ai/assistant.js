// Assistant IA d'ARKLIGHT : comprend une demande (« ajoute 4 Titan en arc derrière la chanteuse »,
// « ré-adresse tout à partir de 1 »…), agit dans le plan ouvert avec ses outils, puis répond.
// Le modèle est au choix : local (Ollama, serveur compatible OpenAI) ou en ligne (clé de l'utilisateur).
import { chat } from './providers.js';
import { TOOL_DEFS, catalogText, createToolRunner } from './tools.js';
import { currentPlan } from '../../shared/plans.js';

const MAX_ROUNDS = 10; // allers-retours modèle ↔ outils pour une demande
const MAX_MESSAGES = 40;

let catalogCache = null;

/** Consignes et connaissances données au modèle (le projet ouvert y est résumé). */
export function systemPrompt(store, lang = 'fr') {
  catalogCache ||= catalogText();
  const cur = currentPlan(store.planInfo());
  return `Tu es l'assistant intégré d'ARKLIGHT, un simulateur d'éclairage pour le cinéma, la télévision et le clip (formation des électros, préparation des plans de feu). Tu connais le logiciel par cœur et tu agis directement dans le projet avec tes outils.

ARKLIGHT
- Un projet contient des séquences et des plans ; chaque plan a son propre plan de feu (projecteurs, grip, décor, annotations). Tu travailles dans le plan ouvert (manage_plans pour en changer ou en créer).
- Repère en mètres : x = gauche / droite vu de la caméra (x < 0 : jardin, à gauche ; x > 0 : cour, à droite) ; y = hauteur (0 = sol) ; z = profondeur (z > 0 : vers la caméra ; z < 0 : vers le fond et le cyclo). Le comédien est en général en (0, 0, 0), son visage vers 1,6 m.
- Placements habituels : face (key) à 3–4 m devant et sur le côté du comédien, 2,5–3,5 m de haut ; contres (backlights) derrière (z négatif), plus hauts ; débouchage (fill) du côté opposé à la face, plus bas et plus faible ; un tube debout au sol a son centre à la moitié de sa longueur.
- DMX : univers à partir de 1, adresses 1 à 512 ; chaque mode DMX occupe un nombre de canaux ; le numéro de fixture (Fix) sert aux consoles et aux applis. Deux projecteurs ne doivent pas se chevaucher (readdress s'en charge et passe à l'univers suivant si besoin).
- Qui commande un projecteur (champ control de get_scene) : d'abord la console ARKLIGHT (onglet Console, elle passe avant tout), sinon le DMX reçu d'une console ou d'une appli (s'il s'arrête, le dernier état reste), sinon ses propres réglages (réglage manuel ; mode autonome pour les Astera : tubes Titan, Helios, Hyperion, AX…). Tes réglages (update_fixtures) relâchent la console ; si des projecteurs restent noirs ou ne réagissent plus à cause d'un DMX conservé ou de la console, appelle release_control.
- Vocabulaire : face = key, contre = backlight, débouchage = fill, cadre 6×6 / 8×8 / 12×12 / 20×20 avec sa toile (diffusion, ultrabounce, noir), drapeau = flag, floppy, réflecteur, pied (stand), sac de sable.

RÈGLES
- Pour modifier des éléments existants, appelle d'abord get_scene pour connaître leurs ids.
- N'invente jamais un modèle : prends les ids du catalogue ci-dessous (ou search_catalog).
- Ne supprime rien sans que l'utilisateur l'ait demandé.
- Tu n'as pas l'historique : ne « restaure » jamais des valeurs de mémoire. Pour annuler, l'utilisateur fait Ctrl+Z (Édition › Annuler ; chacune de tes demandes est une seule étape) ; pour retrouver le projet tel qu'il a été enregistré : Fichier › Revenir à la version enregistrée.
- Un projet neuf contient déjà un « Comédien » (homme debout) au centre : s'il correspond au premier personnage demandé, déplace-le ou renomme-le ; sinon (une comédienne, un enfant…), supprime-le et ajoute le bon personnage.
- Personnages et caméras sont posés au sol (y = 0) : un personnage assis prend props.pose = "sitting" ; une caméra a sa focale dans props.focal (mm) et la hauteur de son objectif dans props.lens (m).
- Pour placer plusieurs projecteurs à des endroits différents, donne une position par projecteur (positions dans add_fixtures, ou un appel update_fixtures par projecteur).
- Agis plutôt que d'expliquer. Si la demande est vague, choisis des valeurs raisonnables et dis lesquelles.
- Ta réponse finale : une à trois phrases simples sur ce qui a été fait (nombres, adresses, positions principales), sans JSON ni bloc de code.

PHOTO D'UN PLAN DE FEU DESSINÉ À LA MAIN
- Appelle place_from_drawing UNE fois avec tous les éléments du dessin, chacun à sa place dans l'image (x, y de 0 à 1, mesurés sur la photo) : n'utilise pas add_fixtures ni add_objects pour un dessin.
- Lis tout : chaque projecteur (symbole, flèche : sa pointe donne aim_x, aim_y ; annotations : modèle, puissance, gélatine, température, hauteur), le grip (cadres, drapeaux), le décor (murs : donne aussi x2, y2 au bout du trait), les personnages, la caméra (focale).
- Échelle (width_m) : d'après les cotes, le quadrillage ou des tailles connues (un personnage vu de dessus fait environ 0,5 m, une voiture 4,5 m, un cadre 12×12 fait 3,6 m) ; sinon 10 m.
- Traduis les noms écrits vers la bibliothèque (« Sky », « S60 » → SkyPanel S60 ; « 600D » → Aputure LS 600D ; « M18 » ; « Titan » ; « Kino 4ft » → Kino Flo) ; un mot mal lu ressemble souvent à un modèle du catalogue (« GOOD » → « 600D ») : prends le plus proche et dis-le.
- N'oublie ni la caméra (avec sa focale) ni le grip dessiné.
- Un nombre écrit (« 4 x TITAN ») est le TOTAL : un seul élément avec count, ou un élément par symbole dessiné avec count 1 — jamais les deux.
- Ensuite seulement, corrige si besoin avec les autres outils, puis résume ce que tu as reconnu et ce qui reste incertain.
- Réponds dans la langue de l'utilisateur (interface en ${lang === 'en' ? 'anglais' : 'français'}).

PROJET OUVERT : « ${store.scene.name} » — ${cur ? `${cur.seq.name} › ${cur.plan.name}` : ''} — ${store.scene.fixtures.length} projecteur(s), ${store.scene.objects.length} objet(s).

CATALOGUE
${catalogCache}`;
}

/** Ce que le modèle relit d'un résultat d'outil (le résumé sert à l'interface). */
const forModel = (result) => {
  const { summary, ...rest } = result || {};
  const json = JSON.stringify(Object.keys(rest).length ? rest : { ok: true, summary });
  return json.length > 14000 ? `${json.slice(0, 14000)}…` : json;
};

export class Assistant {
  /**
   * @param {object} o
   * @param {import('../scene-store.js').SceneStore} o.store
   * @param {() => object} o.settings   réglages (provider, model, urls)
   * @param {(provider: string) => string} o.keyFor  clé API du fournisseur
   */
  constructor({ store, settings, keyFor }) {
    this.store = store;
    this.settings = settings;
    this.keyFor = keyFor;
    this.messages = [];
    this.ctrl = null;
  }

  get busy() {
    return !!this.ctrl;
  }

  reset() {
    this.messages = [];
  }

  stop() {
    this.ctrl?.abort(new Error('Arrêté'));
  }

  /** Garde les derniers échanges (coupés au début d'une demande, jamais au milieu d'un appel d'outil). */
  _trim() {
    if (this.messages.length <= MAX_MESSAGES) return;
    let i = this.messages.length - MAX_MESSAGES;
    while (i < this.messages.length && this.messages[i].role !== 'user') i++;
    this.messages = this.messages.slice(i);
  }

  /**
   * Une demande de l'utilisateur.
   * @param {object} o
   * @param {string} o.text
   * @param {string[]} [o.images]  photos (data URL) pour un modèle qui voit les images
   * @param {string} [o.lang]
   * @param {(e: object) => void} o.onEvent  { kind: 'thinking' | 'tool' | 'note' | 'reply', … }
   * @param {object} [o.hooks]  { afterTool(runner), planChanging(), planChanged() }
   */
  async ask({ text, images = [], imageAspect = null, lang = 'fr', onEvent = () => {}, hooks = {} }) {
    if (this.ctrl) throw new Error('L’assistant est déjà au travail : attends sa réponse ou arrête-le');
    const ctrl = new AbortController();
    this.ctrl = ctrl;
    const runner = createToolRunner({ store: this.store, hooks, context: { imageAspect } });
    try {
      // une photo n'est envoyée qu'avec sa demande (les échanges suivants gardent le texte)
      for (const m of this.messages) delete m.images;
      this.messages.push({ role: 'user', content: String(text || '').slice(0, 8000), ...(images.length ? { images } : {}) });
      this._trim();
      const cfg = this.settings();
      let think = false;
      for (let round = 0; round < MAX_ROUNDS; round++) {
        onEvent({ kind: 'thinking', round });
        let res;
        try {
          res = await chat({ provider: cfg.provider, model: cfg.model, key: this.keyFor(cfg.provider), cfg, system: systemPrompt(this.store, lang), messages: this.messages, tools: TOOL_DEFS, signal: ctrl.signal, think, onLoading: () => onEvent({ kind: 'thinking', round, loading: true }) });
        } catch (e) {
          // modèle sans mode « réflexion » : on réessaie sans le préciser
          if (think === false && /think/i.test(e.message)) {
            think = undefined;
            round--;
            continue;
          }
          throw e;
        }
        this.messages.push({ role: 'assistant', content: res.text, toolCalls: res.toolCalls });
        if (!res.toolCalls.length) {
          // by : qui a calculé (AIODOS le dit : modèle, machine)
          onEvent({ kind: 'reply', text: res.text.trim() || '…', ...(res.by ? { by: res.by } : {}) });
          return runner;
        }
        if (res.text.trim()) onEvent({ kind: 'note', text: res.text.trim() });
        for (const call of res.toolCalls) {
          if (ctrl.signal.aborted) throw new Error('Arrêté');
          const result = runner.run(call.name, call.args);
          onEvent({ kind: 'tool', name: call.name, args: call.args, ok: !result.error, summary: result.error || result.summary || '' });
          hooks.afterTool?.(runner);
          this.messages.push({ role: 'tool', toolCallId: call.id, name: call.name, content: forModel(result) });
        }
      }
      onEvent({ kind: 'reply', text: lang === 'en' ? 'I went through many steps without finishing: tell me if I should go on.' : 'J’ai enchaîné beaucoup d’étapes sans terminer : dis-moi si je continue.' });
      return runner;
    } catch (e) {
      // la demande reste dans l'historique, sans réponse : on la retire pour ne pas embrouiller la suite
      while (this.messages.length && this.messages.at(-1).role !== 'user') this.messages.pop();
      this.messages.pop();
      throw e;
    } finally {
      this.ctrl = null;
    }
  }
}
