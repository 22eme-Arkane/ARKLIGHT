//! Assistant IA (onglet « Assistant IA » du panneau du bas) : `arklight-ai` sur un fil de travail ;
//! chaque outil agit sur le projet ouvert depuis le fil de l'interface (une étape d'annulation par
//! outil, vue et listes à jour), comme `server/ai/assistant.js`.

use crate::{AiLine, AppWindow, State, refresh, tr};
use arklight_ai::{AiError, AiSettings, Aiodos, AiodosOptions, AskRequest, Assistant, AssistantEvent, Backend, Cancel, Changed, DmxState, DocumentTask, KeyStore, ModelInfo, PROVIDERS, ToolHooks, Workspace};
use arklight_core::document::Document;
use slint::{ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use slint::ComponentHandle as _;

/// Ce que l'interface garde de l'assistant.
pub struct Ai {
    pub settings: AiSettings,
    keys: Arc<KeyStore>,
    aiodos: Arc<Aiodos>,
    assistant: Arc<Mutex<Assistant>>,
    stopper: Option<Cancel>,
    pub busy: bool,
    /// conversation affichée : (rôle, texte) ; rôles : user, assistant, tool, note, error
    log: Vec<(String, String)>,
    models: Vec<String>,
    status: String,
    /// demande d'accès à AIODOS en cours (son drapeau « annulée ») : une seule à la fois
    aiodos_request: Option<Arc<AtomicBool>>,
    /// le code à 6 chiffres de cette demande, à comparer avec celui qu'affiche AIODOS
    aiodos_code: String,
    /// photo jointe (plan de feu dessiné, repérage…), envoyée avec la prochaine demande
    pub photo: Option<Photo>,
    /// état d'AIODOS (« ready », « not_installed », « not_running », « needs_access »…) et son message
    pub aiodos_state: String,
    pub aiodos_message: String,
}

/// Installeur d'AIODOS gardé dans le dossier d'ARKLIGHT (case « Installer aussi AIODOS »).
fn aiodos_setup() -> Option<PathBuf> {
    let p = std::env::current_exe().ok()?.parent()?.join("aiodos").join("AIODOS-Setup.exe");
    p.is_file().then_some(p)
}

/// Relit l'état d'AIODOS (en arrière-plan) puis l'affiche ; prêt : liste de ses modèles.
pub fn refresh_aiodos(app: &AppWindow, st: &mut State) {
    // essais automatiques : l'état est donné par ARKLIGHT_SMOKE_AIODOS, AIODOS n'est pas consulté
    if st.smoke {
        st.ai.aiodos_state = std::env::var("ARKLIGHT_SMOKE_AIODOS").unwrap_or_else(|_| "needs_access".into());
        st.ai.aiodos_message = String::new();
        show(app, st);
        return;
    }
    let aiodos = st.ai.aiodos.clone();
    aiodos.set_lang(tr("fr", "en"));
    std::thread::spawn(move || {
        let s = aiodos.status();
        on_ui(move |app, st| {
            let became_ready = s.state == "ready" && st.ai.aiodos_state != "ready";
            st.ai.aiodos_state = s.state.clone();
            st.ai.aiodos_message = s.message.clone();
            show(app, st);
            if became_ready && st.ai.settings.provider == "aiodos" {
                list_models(app, st);
            }
        });
    });
}

/// « Lancer AIODOS » (installé, arrêté) ; son état est relu une fois qu'il a démarré.
pub fn launch_aiodos(app: &AppWindow, st: &mut State) {
    let aiodos = st.ai.aiodos.clone();
    st.ai.status = tr("Lancement d’AIODOS…", "Starting AIODOS…").into();
    show(app, st);
    std::thread::spawn(move || {
        let r = aiodos.launch();
        // le temps qu'il démarre (au plus 30 s)
        for _ in 0..30 {
            if r.is_err() || aiodos.status().state != "not_running" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        on_ui(move |app, st| {
            st.ai.status = match r {
                Ok(_) => String::new(),
                Err(e) => e.message,
            };
            refresh_aiodos(app, st);
        });
    });
}

/// « Installer AIODOS » : son installeur, gardé avec ARKLIGHT, s'ouvre (sur un clic de l'utilisateur).
pub fn install_aiodos(app: &AppWindow, st: &mut State) {
    st.ai.status = match aiodos_setup() {
        Some(exe) => match std::process::Command::new(&exe).arg(format!("/LANG={}", tr("fr", "en"))).spawn() {
            Ok(_) => tr("L’installeur d’AIODOS est ouvert : une fois AIODOS installé, clique sur « Vérifier à nouveau ».", "The AIODOS installer is open: once AIODOS is installed, click “Check again”.").into(),
            Err(e) => e.to_string(),
        },
        None => tr("Installe AIODOS avec l’installeur d’ARKLIGHT (case « Installer aussi AIODOS »), puis reviens ici.", "Install AIODOS with the ARKLIGHT installer (“Also install AIODOS” box), then come back here.").into(),
    };
    show(app, st);
}

/// Photo jointe à la demande : réduite à 1600 pixels de côté au plus (PNG ou JPEG en data URL).
pub struct Photo {
    pub name: String,
    pub data_url: String,
    pub size: (u32, u32),
    /// aussi en image de fond du plan 2D
    pub underlay: bool,
    /// la photo en plus grand (4096 pixels de côté au plus), pour l'image de fond
    full: crate::picture::Picture,
}

/// Côté le plus long d'une photo envoyée au modèle (pixels), comme `PHOTO_MAX`.
const PHOTO_MAX: u32 = 1600;

/// Joint une photo à la prochaine demande, de n'importe quel format (HEIC, WebP… convertis sur un
/// fil de travail : une grande photo prend quelques secondes) ; `send_now` (« Plan dessiné… ») :
/// envoyée dès qu'elle est prête, et le plan 2D s'ouvre.
pub fn attach(app: &AppWindow, st: &mut State, path: PathBuf, underlay: bool, send_now: bool) {
    st.ai.status = tr("Lecture de la photo…", "Reading the photo…").into();
    show(app, st);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    std::thread::spawn(move || {
        let r = crate::picture::load(&path, crate::plot_view::UNDERLAY_MAX).and_then(|full| Ok((crate::picture::shrink(&full, PHOTO_MAX)?, full)));
        on_ui(move |app, st| {
            st.ai.status.clear();
            match r {
                Ok((small, full)) => {
                    if st.smoke {
                        eprintln!("photo : {}×{} ({}), fond {}×{}", small.width, small.height, small.mime, full.width, full.height);
                    }
                    st.ai.photo = Some(Photo { name, data_url: arklight_plot::data_url(small.mime, &small.bytes), size: (small.width, small.height), underlay, full });
                    if send_now {
                        send(app, st, String::new());
                        st.plot.on = true;
                        refresh(app, st);
                    } else {
                        show(app, st);
                    }
                }
                Err(e) => push(app, st, "error", &format!("{name} : {e}")),
            }
        });
    });
}

thread_local! {
    /// l'état de l'application, pour les tâches que le fil de l'assistant envoie à l'interface
    static UI: RefCell<Option<(Rc<RefCell<State>>, slint::Weak<AppWindow>)>> = const { RefCell::new(None) };
}

/// À appeler une fois au démarrage (fil de l'interface).
pub fn install(state: Rc<RefCell<State>>, app: slint::Weak<AppWindow>) {
    UI.with(|u| *u.borrow_mut() = Some((state, app)));
}

/// À la fermeture, avant la fin de `main` : l'état n'est plus gardé par le fil de l'interface (sinon
/// il serait détruit pendant l'arrêt du processus, quand les autres fils n'existent plus).
pub fn uninstall() {
    UI.with(|u| u.borrow_mut().take());
}

/// Exécute `f` sur le fil de l'interface avec l'état et la fenêtre.
pub(crate) fn on_ui(f: impl FnOnce(&AppWindow, &mut State) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || {
        UI.with(|u| {
            if let Some((state, app)) = &*u.borrow()
                && let Some(app) = app.upgrade()
            {
                f(&app, &mut state.borrow_mut());
            }
        });
    });
}

fn data_dir() -> PathBuf {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("ARKLIGHT-Rust")).unwrap_or_else(|| PathBuf::from("."))
}

impl Ai {
    pub fn new(settings: AiSettings, version: &str) -> Ai {
        let keys = Arc::new(KeyStore::open(data_dir().join("ai-keys.json")));
        let aiodos = Arc::new(Aiodos::new(AiodosOptions::arklight(version), keys.clone()));
        Ai {
            settings,
            keys,
            aiodos,
            assistant: Arc::new(Mutex::new(Assistant::new())),
            stopper: None,
            busy: false,
            log: Vec::new(),
            models: Vec::new(),
            status: String::new(),
            aiodos_request: None,
            aiodos_code: String::new(),
            photo: None,
            aiodos_state: String::new(),
            aiodos_message: String::new(),
        }
    }

    fn backend(&self) -> Backend {
        Backend::new(self.settings.clone(), self.keys.clone(), Some(self.aiodos.clone()))
    }
}

/// Crochets des outils : DMX reçu (l'assistant voit ce que pilote une console).
struct UiHooks<'a> {
    dmx: DmxState,
    forget: &'a mut Vec<u32>,
}

impl ToolHooks for UiHooks<'_> {
    fn dmx_state(&self) -> DmxState {
        self.dmx.clone()
    }

    fn forget_dmx(&mut self, universes: &[u32]) -> Vec<u32> {
        self.forget.extend_from_slice(universes);
        universes.to_vec()
    }
}

/// Pointeur vers la tâche d'un passage, envoyé au fil de l'interface : le fil de l'assistant attend
/// qu'elle soit faite avant de continuer, la tâche reste donc valide pendant tout son usage.
struct TaskPtr(*mut DocumentTask<'static>);
// SAFETY : la tâche est `Send` ; son fil d'origine reste bloqué tant que l'interface s'en sert.
unsafe impl Send for TaskPtr {}

impl TaskPtr {
    /// (méthode : la fermeture prend la structure entière, pas seulement son champ non `Send`)
    fn run(self, doc: &mut Document, hooks: &mut dyn ToolHooks) {
        // SAFETY : voir `TaskPtr`
        unsafe { (*self.0)(doc, hooks) }
    }
}

/// Le projet ouvert dans l'application, vu depuis le fil de l'assistant.
struct UiWorkspace;

impl Workspace for UiWorkspace {
    fn with_document(&mut self, f: &mut DocumentTask<'_>) {
        // SAFETY : même objet, durée de vie allongée ; on attend la fin de la tâche ci-dessous
        let ptr = TaskPtr(unsafe { std::mem::transmute::<*mut DocumentTask<'_>, *mut DocumentTask<'static>>(f as *mut DocumentTask<'_>) });
        let (tx, rx) = mpsc::channel();
        let sent = slint::invoke_from_event_loop(move || {
            UI.with(|u| {
                if let Some((state, _)) = &*u.borrow() {
                    let mut st = state.borrow_mut();
                    let dmx = DmxState { data: st.dmx.keys().copied().collect(), live: st.live.iter().copied().collect() };
                    let mut forget = Vec::new();
                    let mut hooks = UiHooks { dmx, forget: &mut forget };
                    ptr.run(&mut st.doc, &mut hooks);
                    for u in &forget {
                        if let Some(net) = &st.net {
                            net.forget(*u);
                        }
                        st.dmx.remove(u);
                    }
                }
            });
            let _ = tx.send(());
        });
        if sent.is_ok() {
            let _ = rx.recv();
        }
    }

    fn after_tool(&mut self, changed: Changed) {
        if changed.any() {
            on_ui(|app, st| refresh(app, st));
        }
    }
}

/// Ajoute une ligne à la conversation affichée.
fn push(app: &AppWindow, st: &mut State, role: &str, text: &str) {
    st.ai.log.push((role.to_owned(), text.to_owned()));
    show(app, st);
}

/// Montre l'état de l'assistant dans la fenêtre.
pub fn show(app: &AppWindow, st: &State) {
    let ai = &st.ai;
    let providers: Vec<SharedString> = PROVIDERS.iter().map(|p| SharedString::from(crate::i18n::t(p.label))).collect();
    app.global::<crate::App>().set_ai_providers(ModelRc::new(VecModel::from(providers)));
    app.global::<crate::App>().set_ai_provider_index(PROVIDERS.iter().position(|p| p.id == ai.settings.provider).unwrap_or(0) as i32);
    let p = PROVIDERS.iter().find(|p| p.id == ai.settings.provider);
    app.global::<crate::App>().set_ai_key_needed(p.is_some_and(|p| p.key));
    app.global::<crate::App>().set_ai_key_saved(ai.keys.list().contains(&ai.settings.provider));
    app.global::<crate::App>().set_ai_aiodos(ai.settings.provider == "aiodos");
    app.global::<crate::App>().set_ai_aiodos_state(ai.aiodos_state.as_str().into());
    app.global::<crate::App>().set_ai_aiodos_message(ai.aiodos_message.as_str().into());
    app.global::<crate::App>().set_ai_aiodos_pending(ai.aiodos_request.is_some());
    app.global::<crate::App>().set_ai_aiodos_code(ai.aiodos_code.as_str().into());
    let mut models = ai.models.clone();
    if !ai.settings.model.is_empty() && !models.contains(&ai.settings.model) {
        models.insert(0, ai.settings.model.clone());
    }
    app.global::<crate::App>().set_ai_model_index(models.iter().position(|m| *m == ai.settings.model).map_or(-1, |i| i as i32));
    app.global::<crate::App>().set_ai_models(ModelRc::new(VecModel::from(models.into_iter().map(SharedString::from).collect::<Vec<_>>())));
    app.global::<crate::App>().set_ai_busy(ai.busy);
    // photo jointe ; adresse du serveur (Ollama, LM Studio…)
    let g = app.global::<crate::App>();
    match &ai.photo {
        Some(p) => {
            g.set_ai_photo_name(p.name.as_str().into());
            g.set_ai_photo(crate::projects::thumb_image(&p.data_url).unwrap_or_default());
            g.set_ai_photo_underlay(p.underlay);
        }
        None => g.set_ai_photo_name(SharedString::new()),
    }
    let url = p.map_or("", |p| p.url);
    g.set_ai_url_shown(!url.is_empty());
    g.set_ai_url_default(url.into());
    g.set_ai_url(ai.settings.urls.get(&ai.settings.provider).cloned().unwrap_or_default().into());
    app.global::<crate::App>().set_ai_status(ai.status.as_str().into());
    let lines: Vec<AiLine> = ai.log.iter().map(|(role, text)| AiLine { role: role.as_str().into(), text: text.as_str().into() }).collect();
    app.global::<crate::App>().set_ai_log(ModelRc::new(VecModel::from(lines)));
}

/// Envoie une demande à l'assistant.
pub fn send(app: &AppWindow, st: &mut State, text: String) {
    let mut text = text.trim().to_owned();
    if (text.is_empty() && st.ai.photo.is_none()) || st.ai.busy {
        return;
    }
    // photo jointe : avec la demande (et en fond du plan 2D si coché)
    let (mut images, mut image_aspect) = (Vec::new(), None);
    if let Some(photo) = st.ai.photo.take() {
        if text.is_empty() {
            text = tr(
                "Voici la photo d’un plan de feu dessiné à la main : reconstruis-le dans le plan ouvert (projecteurs, grip, décor, personnages, caméra).",
                "Here is a photo of a hand-drawn lighting plot: rebuild it in the open shot (fixtures, grip, set, people, camera).",
            )
            .to_owned();
        }
        images.push(photo.data_url);
        image_aspect = Some(photo.size.0 as f64 / photo.size.1.max(1) as f64);
        if photo.underlay
            && let Err(e) = crate::plot_view::place_underlay(st, &photo.full)
        {
            push(app, st, "error", &e);
        }
        push(app, st, "note", &format!("{} {}", tr("Photo jointe :", "Photo attached:"), photo.name));
    }
    push(app, st, "user", &text);
    st.ai.busy = true;
    st.ai.status = tr("L’assistant réfléchit…", "The assistant is thinking…").into();
    show(app, st);
    let mut backend = st.ai.backend();
    let aiodos = st.ai.aiodos.clone();
    let assistant = st.ai.assistant.clone();
    let lang = tr("fr", "en").to_owned();
    let stopper = assistant.lock().ok().map_or_else(Cancel::new, |a| a.stopper());
    stopper.reset(); // un ancien « Arrêter » ne compte plus
    st.ai.stopper = Some(stopper.clone());
    let photo = !images.is_empty();
    std::thread::spawn(move || {
        let result = (|| {
            // AIODOS lancé et connecté, modèle choisi : la demande part sans autre clic
            if let Some(p) = prepare(&mut backend, &aiodos, &stopper, photo)? {
                on_ui(move |app, st| {
                    st.ai.models = p.models;
                    if p.keep {
                        st.ai.settings.set_model(&p.model);
                    } else {
                        st.ai.log.push(("note".into(), format!("{} {}", tr("Photo lue par", "Photo read by"), p.model)));
                    }
                    show(app, st);
                });
            }
            let mut a = assistant.lock().map_err(|_| AiError::new(tr("Assistant indisponible", "Assistant unavailable")))?;
            let mut ws = UiWorkspace;
            let req = AskRequest { text, images, image_aspect, lang };
            a.ask(&mut ws, &backend, req, &mut |ev| {
            let line = match ev {
                AssistantEvent::Thinking { loading: true, .. } => Some(("status", tr("Chargement du modèle en mémoire (la première fois peut être longue)…", "Loading the model into memory (the first time can be long)…").to_owned())),
                AssistantEvent::Thinking { .. } => None,
                AssistantEvent::Tool { name, ok, summary, .. } => Some(("tool", format!("{} {name} — {summary}", if ok { "✓" } else { "✗" }))),
                AssistantEvent::Note { text } => Some(("note", text)),
                AssistantEvent::Reply { .. } => None,
            };
            if let Some((role, text)) = line {
                on_ui(move |app, st| {
                    if role == "status" {
                        st.ai.status = text;
                        show(app, st);
                    } else {
                        push(app, st, role, &text);
                    }
                });
            }
            })
        })();
        on_ui(move |app, st| {
            st.ai.busy = false;
            st.ai.stopper = None;
            st.ai.status = String::new();
            match result {
                Ok(out) => {
                    let by = out.by.map(|b| format!("\n— {} {} · {}", tr("calculé par", "computed by"), b.model, b.machine)).unwrap_or_default();
                    push(app, st, "assistant", &format!("{}{by}", out.reply));
                    if !out.last_added.is_empty() {
                        st.selection = out.last_added;
                    }
                    refresh(app, st);
                }
                Err(e) => push(app, st, "error", &e.message),
            }
        });
    });
}

/// Modèle choisi avant une demande, et la liste du fournisseur.
struct Picked {
    model: String,
    /// gardé dans les réglages (aucun modèle n'était choisi) ; sinon pour cette demande seulement
    /// (photo jointe : un modèle qui voit les images)
    keep: bool,
    models: Vec<String>,
}

/// Avant une demande (fil de travail) : AIODOS lancé et connecté s'il le faut, la demande partant
/// ensuite toute seule ; un modèle choisi s'il n'y en a pas (le conseillé : il sait agir dans
/// ARKLIGHT et voit les images), ou un modèle qui voit la photo jointe si le choisi ne la voit pas.
fn prepare(backend: &mut Backend, aiodos: &Aiodos, stop: &Cancel, photo: bool) -> Result<Option<Picked>, AiError> {
    let status = |text: String| {
        on_ui(move |app, st| {
            st.ai.status = text;
            show(app, st);
        })
    };
    let is_aiodos = backend.settings.provider == "aiodos";
    if is_aiodos {
        aiodos.set_lang(tr("fr", "en"));
        let mut s = aiodos.status();
        if s.state == "not_running" {
            status(tr("Lancement d’AIODOS…", "Starting AIODOS…").into());
            aiodos.launch()?;
            s = aiodos.wait_ready(Duration::from_secs(90), Some(stop))?;
        }
        // demande d'accès déjà ouverte (« Se connecter à AIODOS ») : on attend sa réponse
        while s.state == "pending" {
            stop.sleep(Duration::from_secs(1)).map_err(|_| AiError::cancelled())?;
            s = aiodos.status();
        }
        if matches!(s.state.as_str(), "needs_access" | "revoked") {
            status(tr("Demande d’accès envoyée à AIODOS…", "Access request sent to AIODOS…").into());
            // carte d'AIODOS : le code en grand et « Annuler »
            let flag = Arc::new(AtomicBool::new(false));
            on_ui(move |app, st| {
                st.ai.aiodos_request = Some(flag);
                show(app, st);
            });
            let r = aiodos.connect(&mut |code| {
                let code = code.to_owned();
                on_ui(move |app, st| {
                    st.ai.aiodos_code = code.clone();
                    st.ai.status = format!("{} {code} {}", tr("Accepte la demande dans AIODOS (code", "Accept the request in AIODOS (code"), tr("): la demande partira ensuite toute seule.", "): your request will then go on by itself."));
                    show(app, st);
                });
            });
            on_ui(|app, st| {
                st.ai.aiodos_request = None;
                st.ai.aiodos_code.clear();
                show(app, st);
            });
            r?;
            s = aiodos.status();
        }
        let (state, message) = (s.state.clone(), s.message.clone());
        on_ui(move |app, st| {
            st.ai.aiodos_state = state;
            st.ai.aiodos_message = message;
            show(app, st);
        });
        if s.state != "ready" {
            return Err(AiError::with_code(&s.state, s.message));
        }
        status(tr("L’assistant réfléchit…", "The assistant is thinking…").into());
    }
    let current = backend.settings.model.clone();
    let list = match backend.list_models(None) {
        Ok(l) => l,
        // liste indisponible mais modèle choisi : la conversation dira ce qui ne va pas
        Err(_) if !current.is_empty() => return Ok(None),
        Err(e) => return Err(e),
    };
    let picked = choose_model(&list, &current, is_aiodos, photo)?;
    if let Some(p) = &picked {
        backend.settings.model = p.model.clone();
    }
    Ok(picked)
}

/// Le modèle d'une demande : le choisi s'il convient (il sait agir dans ARKLIGHT pour AIODOS, il
/// voit les images s'il y a une photo), sinon le conseillé de la liste (None : le choisi).
fn choose_model(list: &[ModelInfo], current: &str, is_aiodos: bool, photo: bool) -> Result<Option<Picked>, AiError> {
    let needs: &[&str] = if is_aiodos { &["tools"] } else { &[] };
    // capacités inconnues (fournisseur qui ne les donne pas) : on suppose que oui
    let can = |m: &ModelInfo, cap: &str| m.caps.is_empty() || m.has(cap);
    match list.iter().find(|m| m.id == current) {
        Some(m) if m.installed && needs.iter().all(|c| can(m, c)) && (!photo || can(m, "vision")) => return Ok(None),
        // modèle saisi à la main, absent de la liste (fournisseur en ligne) : tel quel
        None if !current.is_empty() && !is_aiodos => return Ok(None),
        _ => {}
    }
    let Some(best) = arklight_ai::aiodos::pick_model(list, needs, photo) else {
        return Err(AiError::new(if is_aiodos {
            tr("Aucun modèle d’AIODOS ne sait agir dans ARKLIGHT : installes-en un dans AIODOS (avec « outils »).", "No AIODOS model can act in ARKLIGHT: install one in AIODOS (with “tools”).")
        } else {
            tr("Aucun modèle trouvé.", "No model found.")
        }));
    };
    if photo && !can(best, "vision") {
        return Err(AiError::new(tr(
            "Aucun modèle installé ne voit les images : installe un modèle « vision » pour lire un plan dessiné.",
            "No installed model can see images: install a “vision” model to read a drawn plan.",
        )));
    }
    // gardé si aucun modèle n'était choisi (ou plus installé) ; photo : pour cette demande
    let keep = current.is_empty() || !list.iter().any(|m| m.id == current && m.installed);
    Ok(Some(Picked { model: best.id.clone(), keep, models: list.iter().map(|m| m.id.clone()).collect() }))
}

/// Bouton « Arrêter » (y compris pendant le lancement d'AIODOS ou sa demande d'accès).
pub fn stop(st: &mut State) {
    if let Some(s) = &st.ai.stopper {
        s.cancel();
    }
    st.ai.aiodos.cancel();
}

/// « Nouvelle conversation ».
pub fn reset(app: &AppWindow, st: &mut State) {
    if st.ai.busy {
        return;
    }
    if let Ok(mut a) = st.ai.assistant.lock() {
        a.reset();
    }
    st.ai.log.clear();
    show(app, st);
}

/// Modèles du fournisseur choisi (sur un fil : le serveur peut être lent ou absent).
pub fn list_models(app: &AppWindow, st: &mut State) {
    st.ai.status = tr("Recherche des modèles…", "Looking for models…").into();
    show(app, st);
    let backend = st.ai.backend();
    std::thread::spawn(move || {
        let r = backend.list_models(None);
        on_ui(move |app, st| {
            match r {
                Ok(list) => {
                    // aucun modèle choisi : le conseillé (AIODOS : il sait agir dans ARKLIGHT, voit les images)
                    if st.ai.settings.model.is_empty() {
                        let needs: &[&str] = if st.ai.settings.provider == "aiodos" { &["tools"] } else { &[] };
                        if let Some(m) = arklight_ai::aiodos::pick_model(&list, needs, true).or(list.first()) {
                            st.ai.settings.set_model(&m.id.clone());
                        }
                    }
                    st.ai.models = list.into_iter().map(|m| m.id).collect();
                    st.ai.status = if st.ai.models.is_empty() { tr("Aucun modèle trouvé.", "No model found.").into() } else { String::new() };
                }
                Err(e) => st.ai.status = e.message,
            }
            show(app, st);
        });
    });
}

/// Connexion à AIODOS : code à 6 chiffres à comparer avec celui affiché par AIODOS, bouton
/// « Annuler » pendant l'attente. Déjà connecté : pas de nouvelle demande (AIODOS montrerait une
/// carte d'accord pour rien).
pub fn connect_aiodos(app: &AppWindow, st: &mut State) {
    if st.ai.aiodos_request.is_some() {
        return; // une demande attend déjà : pas une deuxième
    }
    let stop = Arc::new(AtomicBool::new(false));
    st.ai.aiodos_request = Some(stop.clone());
    st.ai.aiodos_code.clear();
    st.ai.status = tr("Demande d’accès envoyée à AIODOS…", "Access request sent to AIODOS…").into();
    show(app, st);
    let aiodos = st.ai.aiodos.clone();
    aiodos.set_lang(tr("fr", "en"));
    std::thread::spawn(move || {
        let ready = aiodos.status().state == "ready";
        let r = (!ready && !stop.load(Ordering::SeqCst)).then(|| {
            aiodos.connect(&mut |code| {
                if stop.load(Ordering::SeqCst) {
                    // « Annuler » cliqué avant que la demande existe : elle s'arrête ici
                    aiodos.cancel();
                    return;
                }
                let code = code.to_owned();
                on_ui(move |app, st| {
                    st.ai.aiodos_code = code.clone();
                    st.ai.status = format!("{} {code}", tr("Compare ce code avec celui d’AIODOS, puis accepte dans AIODOS :", "Compare this code with AIODOS’s, then accept in AIODOS:"));
                    show(app, st);
                });
            })
        });
        on_ui(move |app, st| {
            st.ai.aiodos_request = None;
            st.ai.aiodos_code.clear();
            st.ai.status = match r {
                None if ready => tr("AIODOS est déjà connecté.", "AIODOS is already connected.").into(),
                Some(Ok(_)) => tr("AIODOS est connecté.", "AIODOS is connected.").into(),
                Some(Err(e)) if !e.is_cancelled() => e.message,
                _ => tr("Demande d’accès annulée.", "Access request cancelled.").into(),
            };
            show(app, st);
            refresh_aiodos(app, st);
        });
    });
}

/// Bouton « Annuler » de la demande d'accès à AIODOS.
pub fn cancel_aiodos(app: &AppWindow, st: &mut State) {
    let Some(stop) = &st.ai.aiodos_request else { return };
    stop.store(true, Ordering::SeqCst);
    st.ai.aiodos.cancel();
    st.ai.status = tr("Annulation de la demande d’accès…", "Cancelling the access request…").into();
    show(app, st);
}

/// Enregistre la clé API du fournisseur choisi (chiffrée sur ce poste).
pub fn save_key(app: &AppWindow, st: &mut State, key: &str) {
    let provider = st.ai.settings.provider.clone();
    st.ai.status = match st.ai.keys.set(&provider, key.trim()) {
        Ok(()) => tr("Clé enregistrée (chiffrée sur ce poste).", "Key saved (encrypted on this computer).").into(),
        Err(e) => e.to_string(),
    };
    show(app, st);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, caps: &[&str]) -> ModelInfo {
        serde_json::from_value(serde_json::json!({ "id": id, "caps": caps })).unwrap()
    }

    fn list() -> Vec<ModelInfo> {
        vec![model("texte", &["completion", "tools"]), model("vision", &["completion", "tools", "vision"]), model("bavard", &["completion"])]
    }

    #[test]
    fn aiodos_sans_modele_choisit_le_conseille() {
        let p = choose_model(&list(), "", true, false).unwrap().unwrap();
        assert!(p.model == "texte" || p.model == "vision");
        assert!(p.keep);
        // avec une photo : celui qui voit les images
        let p = choose_model(&list(), "", true, true).unwrap().unwrap();
        assert_eq!((p.model.as_str(), p.keep), ("vision", true));
    }

    #[test]
    fn photo_avec_un_modele_aveugle() {
        // le modèle choisi reste dans les réglages ; la photo part avec un modèle qui la voit
        let p = choose_model(&list(), "texte", true, true).unwrap().unwrap();
        assert_eq!((p.model.as_str(), p.keep), ("vision", false));
        assert!(choose_model(&list(), "texte", true, false).unwrap().is_none());
    }

    #[test]
    fn modele_sans_outils_remplace() {
        let p = choose_model(&list(), "bavard", true, false).unwrap().unwrap();
        assert_ne!(p.model, "bavard");
    }

    #[test]
    fn aucun_modele_utilisable() {
        assert!(choose_model(&[model("bavard", &["completion"])], "", true, false).is_err());
        assert!(choose_model(&[model("texte", &["completion", "tools"])], "", true, true).is_err());
    }

    #[test]
    fn autres_fournisseurs() {
        // modèle saisi à la main, absent de la liste : gardé ; capacités inconnues : acceptées
        assert!(choose_model(&[model("a", &[])], "gpt-x", false, true).unwrap().is_none());
        let p = choose_model(&[model("a", &[])], "", false, true).unwrap().unwrap();
        assert_eq!((p.model.as_str(), p.keep), ("a", true));
        assert!(choose_model(&[], "", false, false).is_err());
    }
}
