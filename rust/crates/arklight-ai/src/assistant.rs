//! Assistant IA d'ARKLIGHT (portage de `server/ai/assistant.js`) : comprend une demande (« ajoute 4
//! Titan en arc derrière la chanteuse », « ré-adresse tout à partir de 1 »…), agit dans le plan
//! ouvert avec ses outils, puis répond. Le modèle est au choix : local (Ollama, AIODOS, serveur
//! compatible OpenAI) ou en ligne (clé de l'utilisateur).
//!
//! Fils : une demande est bloquante (réseau) et se lance sur un fil de travail ; le projet n'est
//! touché que le temps de chaque outil, par [`Workspace::with_document`] (l'application de bureau y
//! passe sur le fil de l'interface, ou verrouille son projet). « Arrêter » : [`Assistant::stopper`].

use crate::error::AiError;
use crate::http::Cancel;
use crate::providers::{AnsweredBy, Backend, ChatOptions, Message};
use crate::tools::{Changed, ToolHooks, ToolRunner, catalog_text, is_error, result_summary, tool_defs};
use arklight_core::document::Document;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// allers-retours modèle ↔ outils pour une demande
pub const MAX_ROUNDS: usize = 10;
const MAX_MESSAGES: usize = 40;
/// longueur d'une demande (caractères)
const MAX_TEXT: usize = 8000;
/// ce que le modèle relit d'un résultat d'outil
const MAX_RESULT: usize = 14000;

/// Consignes et connaissances données au modèle (le projet ouvert y est résumé).
pub fn system_prompt(doc: &Document, lang: &str) -> String {
    let p = &doc.project;
    let interface = if lang == "en" { "anglais" } else { "français" };
    format!(
        "Tu es l'assistant intégré d'ARKLIGHT, un simulateur d'éclairage pour le cinéma, la télévision et le clip (formation des électros, préparation des plans de feu). Tu connais le logiciel par cœur et tu agis directement dans le projet avec tes outils.

ARKLIGHT
- Un projet contient des séquences et des plans ; chaque plan a son propre plan de feu (projecteurs, grip, décor, annotations). Tu travailles dans le plan ouvert (manage_plans pour en changer ou en créer).
- Repère en mètres : x = gauche / droite vu de la caméra (x < 0 : jardin, à gauche ; x > 0 : cour, à droite) ; y = hauteur (0 = sol) ; z = profondeur (z > 0 : vers la caméra ; z < 0 : vers le fond et le cyclo). Le comédien est en général en (0, 0, 0), son visage vers 1,6 m.
- Placements habituels : face (key) à 3–4 m devant et sur le côté du comédien, 2,5–3,5 m de haut ; contres (backlights) derrière (z négatif), plus hauts ; débouchage (fill) du côté opposé à la face, plus bas et plus faible ; un tube debout au sol a son centre à la moitié de sa longueur.
- DMX : univers à partir de 1, adresses 1 à 512 ; chaque mode DMX occupe un nombre de canaux ; le numéro de fixture (Fix) sert aux consoles et aux applis. Deux projecteurs ne doivent pas se chevaucher (readdress s'en charge et passe à l'univers suivant si besoin).
- Qui commande un projecteur (champ control de get_scene) : d'abord la console ARKLIGHT (onglet Console, elle passe avant tout), sinon le DMX reçu d'une console ou d'une appli (s'il s'arrête, le dernier état reste), sinon ses propres réglages (réglage manuel ; mode autonome pour les Astera : tubes Titan, Helios, Hyperion, AX…). Tes réglages (update_fixtures) relâchent la console ; si des projecteurs restent noirs ou ne réagissent plus à cause d'un DMX conservé ou de la console, appelle release_control.
- Vocabulaire : face = key, contre = backlight, débouchage = fill, cadre 6×6 / 8×8 / 12×12 / 20×20 avec sa toile (props.cloth : silk-1-4, silk-1-2, silk = soie full, china-silk, grid-1-4, grid-1-2, grid = full grid, silent-grid…, soft-frost-1-2, soft-frost, muslin-bleached, muslin-unbleached, magic-cloth, ultrabounce, griffolyn, silver-lame, soft-silver, gold-lame, soft-gold, checkerboard, solid = noir, net-lavender, net-single, net-double, net-triple, empty, ou un filtre du nuancier), drapeau = flag, floppy, réflecteur, pied (stand), sac de sable.

RÈGLES
- Pour modifier des éléments existants, appelle d'abord get_scene pour connaître leurs ids.
- N'invente jamais un modèle : prends les ids du catalogue ci-dessous (ou search_catalog).
- Ne supprime rien sans que l'utilisateur l'ait demandé.
- Tu n'as pas l'historique : ne « restaure » jamais des valeurs de mémoire. Pour annuler, l'utilisateur fait Ctrl+Z (Édition › Annuler ; chacune de tes actions s'annule à son tour) ; pour retrouver le projet tel qu'il a été enregistré : Fichier › Revenir à la version enregistrée.
- Un projet neuf contient déjà un « Comédien » (homme debout) au centre : s'il correspond au premier personnage demandé, déplace-le ou renomme-le ; sinon (une comédienne, un enfant…), supprime-le et ajoute le bon personnage.
- Personnages et caméras sont posés au sol (y = 0) : un personnage assis prend props.pose = \"sitting\" ; une caméra a sa focale dans props.focal (mm) et la hauteur de son objectif dans props.lens (m).
- Pour placer plusieurs projecteurs à des endroits différents, donne une position par projecteur (positions dans add_fixtures, ou un appel update_fixtures par projecteur).
- Agis plutôt que d'expliquer. Si la demande est vague, choisis des valeurs raisonnables et dis lesquelles.
- Ta réponse finale : une à trois phrases simples sur ce qui a été fait (nombres, adresses, positions principales), sans JSON ni bloc de code.

PHOTO D'UN PLAN DE FEU DESSINÉ À LA MAIN
- Appelle place_from_drawing UNE fois avec tous les éléments du dessin, chacun à sa place dans l'image (x, y de 0 à 1, mesurés sur la photo) : n'utilise pas add_fixtures ni add_objects pour un dessin.
- Lis tout : chaque projecteur (symbole, flèche : sa pointe donne aim_x, aim_y ; annotations : modèle, puissance, gélatine, température, hauteur), le grip (cadres, drapeaux), le décor (murs : donne aussi x2, y2 au bout du trait), les personnages, la caméra (focale).
- Une maison, un bâtiment, une estrade ou toute autre forme dessinée : kind « shape », en rectangle (x, y et x2, y2 aux coins opposés) ou en contour (points), avec sa hauteur si elle est écrite (une maison ≈ 3 m) ; elle apparaît sur le plan 2D et en volume dans la 3D.
- Échelle (width_m) : d'après les cotes, le quadrillage ou des tailles connues (un personnage vu de dessus fait environ 0,5 m, une voiture 4,5 m, un cadre 12×12 fait 3,6 m) ; sinon 10 m.
- Traduis les noms écrits vers la bibliothèque (« Sky », « S60 » → SkyPanel S60 ; « 600D » → Aputure LS 600D ; « M18 » ; « Titan » ; « Kino 4ft » → Kino Flo) ; un mot mal lu ressemble souvent à un modèle du catalogue (« GOOD » → « 600D ») : prends le plus proche et dis-le.
- N'oublie ni la caméra (avec sa focale) ni le grip dessiné.
- La photo est souvent prise de travers ou tournée d'un quart de tour : lis les mots dans leur sens, mais donne x, y mesurés sur la photo telle qu'elle est (elle sert aussi d'image de fond du plan).
- Un nombre écrit (« 4 x TITAN ») est le TOTAL : un seul élément avec count, ou un élément par symbole dessiné avec count 1 — jamais les deux.
- Ensuite seulement, corrige si besoin avec les autres outils, puis résume ce que tu as reconnu et ce qui reste incertain.
- Réponds dans la langue de l'utilisateur (interface en {interface}).

PROJET OUVERT : « {} » — {} — {} projecteur(s), {} objet(s).

CATALOGUE
{}",
        p.name,
        doc.current_plan_label(),
        p.fixtures.len(),
        p.objects.len(),
        catalog_text()
    )
}

/// Ce que le modèle relit d'un résultat d'outil (le résumé sert à l'interface).
pub fn for_model(result: &Value) -> String {
    let mut rest = result.as_object().cloned().unwrap_or_default();
    let summary = rest.remove("summary");
    let json = if rest.is_empty() {
        let mut m = serde_json::Map::new();
        m.insert("ok".into(), true.into());
        if let Some(s) = summary {
            m.insert("summary".into(), s);
        }
        Value::Object(m).to_string()
    } else {
        Value::Object(rest).to_string()
    };
    if json.chars().count() > MAX_RESULT { format!("{}…", json.chars().take(MAX_RESULT).collect::<String>()) } else { json }
}

/// Ce qui se passe pendant une demande (pour l'interface).
#[derive(Debug, Clone, PartialEq)]
pub enum AssistantEvent {
    /// le modèle réfléchit (`loading` : il se charge d'abord en mémoire)
    Thinking { round: usize, loading: bool },
    /// un outil a été appelé
    Tool { name: String, args: Value, ok: bool, summary: String },
    /// texte du modèle entre deux outils
    Note { text: String },
    /// la réponse finale ; `by` : qui a calculé (AIODOS le dit : modèle, machine)
    Reply { text: String, by: Option<AnsweredBy> },
}

/// Une demande de l'utilisateur.
#[derive(Debug, Clone, Default)]
pub struct AskRequest {
    pub text: String,
    /// photos (data URL) pour un modèle qui voit les images ; 4 au plus
    pub images: Vec<String>,
    /// largeur / hauteur de la photo (plan dessiné)
    pub image_aspect: Option<f64>,
    /// langue de l'interface (« fr » ou « en »)
    pub lang: String,
}

/// Résultat d'une demande.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AskOutcome {
    /// la réponse finale (aussi envoyée par l'événement Reply)
    pub reply: String,
    pub by: Option<AnsweredBy>,
    /// ce qui a changé pendant toute la demande
    pub changed: Changed,
    /// projecteurs ajoutés (à sélectionner)
    pub last_added: Vec<String>,
}

/// Un passage sur le projet (calcul du prompt système, ou un outil) : court, sans réseau.
pub type DocumentTask<'a> = dyn FnMut(&mut Document, &mut dyn ToolHooks) + Send + 'a;

/// Où l'assistant agit : le projet ouvert.
pub trait Workspace {
    /// Donne le projet (et les crochets des outils) le temps d'un passage : sous le verrou du
    /// projet (`Arc<Mutex<Document>>`), ou sur le fil de l'interface (la tâche est `Send` : le fil
    /// de travail peut l'y envoyer et attendre qu'elle soit faite).
    fn with_document(&mut self, f: &mut DocumentTask<'_>);

    /// Après chaque outil : ce qui a changé (rafraîchir les vues, le patch, sACN…).
    fn after_tool(&mut self, _changed: Changed) {}
}

/// Le cas simple : un projet et ses crochets à portée de main (essais, outils en ligne de commande).
pub struct DocumentWorkspace<'a> {
    pub doc: &'a mut Document,
    pub hooks: &'a mut dyn ToolHooks,
}

impl Workspace for DocumentWorkspace<'_> {
    fn with_document(&mut self, f: &mut DocumentTask<'_>) {
        f(self.doc, self.hooks);
    }
}

/// Le projet partagé entre le fil de l'interface et celui de l'assistant (verrouillé le temps de
/// chaque passage) ; `on_change` : après chaque outil (rafraîchir l'interface).
pub struct SharedWorkspace<H: ToolHooks> {
    pub doc: std::sync::Arc<std::sync::Mutex<Document>>,
    pub hooks: H,
    pub on_change: Option<Box<dyn FnMut(Changed) + Send>>,
}

impl<H: ToolHooks> Workspace for SharedWorkspace<H> {
    fn with_document(&mut self, f: &mut DocumentTask<'_>) {
        let mut doc = self.doc.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut doc, &mut self.hooks);
    }

    fn after_tool(&mut self, changed: Changed) {
        if let Some(cb) = self.on_change.as_mut() {
            cb(changed);
        }
    }
}

/// L'assistant : la conversation en cours.
#[derive(Debug, Default)]
pub struct Assistant {
    messages: Vec<Message>,
    stop: Cancel,
    busy: Arc<AtomicBool>,
}

impl Assistant {
    pub fn new() -> Assistant {
        Assistant::default()
    }

    /// Une demande est en cours.
    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }

    /// Oublie la conversation.
    pub fn reset(&mut self) {
        self.messages.clear();
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Bouton « Arrêter », utilisable depuis un autre fil.
    pub fn stopper(&self) -> Cancel {
        self.stop.clone()
    }

    /// Arrête la demande en cours.
    pub fn stop(&self) {
        self.stop.cancel();
    }

    /// Garde les derniers échanges (coupés au début d'une demande, jamais au milieu d'un appel d'outil).
    fn trim(&mut self) {
        if self.messages.len() <= MAX_MESSAGES {
            return;
        }
        let mut i = self.messages.len() - MAX_MESSAGES;
        while i < self.messages.len() && !matches!(self.messages[i], Message::User { .. }) {
            i += 1;
        }
        self.messages.drain(..i);
    }

    /// Une demande de l'utilisateur : le modèle répond, appelle des outils (appliqués au projet),
    /// jusqu'à sa réponse finale. Bloquant (réseau) : à lancer sur un fil de travail.
    pub fn ask(&mut self, ws: &mut dyn Workspace, backend: &Backend, req: AskRequest, on_event: &mut dyn FnMut(AssistantEvent)) -> Result<AskOutcome, AiError> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Err(AiError::with_code("busy", "L’assistant est déjà au travail : attends sa réponse ou arrête-le"));
        }
        self.stop.reset();
        let mut runner = ToolRunner::new(req.image_aspect);
        let r = self.ask_inner(ws, backend, &req, on_event, &mut runner);
        if r.is_err() {
            // la demande reste dans l'historique, sans réponse : on la retire pour ne pas embrouiller la suite
            while self.messages.last().is_some_and(|m| !matches!(m, Message::User { .. })) {
                self.messages.pop();
            }
            self.messages.pop();
        }
        self.busy.store(false, Ordering::SeqCst);
        r.map(|(reply, by, changed)| AskOutcome { reply, by, changed, last_added: runner.last_added })
    }

    fn ask_inner(&mut self, ws: &mut dyn Workspace, backend: &Backend, req: &AskRequest, on_event: &mut dyn FnMut(AssistantEvent), runner: &mut ToolRunner) -> Result<(String, Option<AnsweredBy>, Changed), AiError> {
        let lang = if req.lang == "en" { "en" } else { "fr" };
        let mut changed = Changed::default();
        // une photo n'est envoyée qu'avec sa demande (les échanges suivants gardent le texte)
        for m in &mut self.messages {
            if let Message::User { images, .. } = m {
                images.clear();
            }
        }
        self.messages.push(Message::User { content: req.text.chars().take(MAX_TEXT).collect(), images: req.images.iter().take(4).cloned().collect() });
        self.trim();
        let mut think = Some(false);
        let mut round = 0;
        while round < MAX_ROUNDS {
            on_event(AssistantEvent::Thinking { round, loading: false });
            let mut system = String::new();
            ws.with_document(&mut |doc, _| system = system_prompt(doc, lang));
            let res = {
                let mut on_loading = || on_event(AssistantEvent::Thinking { round, loading: true });
                let mut o = ChatOptions { system: &system, messages: &self.messages, tools: tool_defs(), think, cancel: Some(&self.stop), on_loading: &mut on_loading };
                backend.chat(&mut o)
            };
            let res = match res {
                Ok(r) => r,
                // modèle sans mode « réflexion » : on réessaie sans le préciser
                Err(e) if think == Some(false) && e.message.to_lowercase().contains("think") => {
                    think = None;
                    continue;
                }
                Err(e) => return Err(e),
            };
            self.messages.push(Message::Assistant { content: res.text.clone(), tool_calls: res.tool_calls.clone() });
            if res.tool_calls.is_empty() {
                let text = res.text.trim();
                let text = if text.is_empty() { "…".to_owned() } else { text.to_owned() };
                on_event(AssistantEvent::Reply { text: text.clone(), by: res.by.clone() });
                return Ok((text, res.by, changed));
            }
            if !res.text.trim().is_empty() {
                on_event(AssistantEvent::Note { text: res.text.trim().to_owned() });
            }
            for call in &res.tool_calls {
                if self.stop.is_cancelled() {
                    return Err(AiError::cancelled());
                }
                let mut result = Value::Null;
                ws.with_document(&mut |doc, hooks| result = runner.run(doc, hooks, &call.name, &call.args));
                on_event(AssistantEvent::Tool { name: call.name.clone(), args: call.args.clone(), ok: !is_error(&result), summary: result_summary(&result) });
                let c = runner.take_changed();
                changed.fixtures |= c.fixtures;
                changed.objects |= c.objects;
                changed.groups |= c.groups;
                changed.environment |= c.environment;
                changed.plans |= c.plans;
                ws.after_tool(c);
                self.messages.push(Message::Tool { tool_call_id: call.id.clone(), name: call.name.clone(), content: for_model(&result) });
            }
            round += 1;
        }
        let text = if lang == "en" { "I went through many steps without finishing: tell me if I should go on." } else { "J’ai enchaîné beaucoup d’étapes sans terminer : dis-moi si je continue." };
        on_event(AssistantEvent::Reply { text: text.into(), by: None });
        Ok((text.into(), None, changed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resultat_pour_le_modele() {
        assert_eq!(for_model(&json!({ "summary": "Ambiance réglée" })), r#"{"ok":true,"summary":"Ambiance réglée"}"#);
        assert_eq!(for_model(&json!({ "error": "x", "summary": "y" })), r#"{"error":"x"}"#);
        let long = for_model(&json!({ "a": "é".repeat(20000) }));
        assert!(long.ends_with('…') && long.chars().count() == MAX_RESULT + 1);
    }
}
