//! ARKLIGHT pour PC, version Rust (migration en cours, voir docs/rust/PLAN.md) : interface Slint,
//! vue 3D dessinée par le moteur wgpu dans une texture que Slint affiche sur la même carte
//! graphique. Le projet ouvert est un `Document` (modifications, annuler / rétablir) ; le moteur
//! lumière calcule la sortie des projecteurs 60 fois par seconde ; la vue n'est redessinée que si
//! quelque chose change (lumière, caméra, projet, taille).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

mod actions;
mod ai_panel;
mod ble_panel;
mod commands;
mod console_panel;
mod dictation;
mod i18n;
mod inspector;
mod library_panel;
mod scene_panel;
mod onboard_panel;
mod plot_view;
mod projects;
mod remote;
mod shortcuts;
mod news;
mod picture;
mod single;
mod update;
mod voice;

use arklight_core::color::linear_to_srgb;
use arklight_core::document::{Changes, Document, Edit};
use arklight_core::engine::{Ctx, Lights, Universes};
use arklight_core::{Project, library};
use arklight_midi::message::TARGET_KINDS;
use arklight_midi::{Action as MidiAction, ColorChange as MidiColor, EnvChange, Midi, MidiContext, MidiInputs, WizardKind, plan_ids};
use arklight_net::{RdmEvent, Receiver};
use arklight_render::glam::Vec3;
use arklight_render::stage::scene_from_project;
use arklight_render::{Camera, Renderer, Scene, outline, pick};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

const DEMO_NIGHT: &str = include_str!("../../arklight-core/tests/data/demo-night.json");
const DEMO_CLIP: &str = include_str!("../../arklight-core/tests/data/demo-clip.json");

/// Geste en cours dans la vue 3D.
enum Drag {
    /// tourner (bouton 0) ou décaler (autres boutons) la caméra
    Camera { button: i32, x: f32, y: f32 },
    /// déplacer la sélection au sol (ou en hauteur avec Maj) depuis ses positions de départ ;
    /// `basis` : déplacement au sol d'un pixel de souris en x et en y ; `snap` : pas de 10 cm
    Move { x: f32, y: f32, basis: (Vec3, Vec3), start: Vec<(String, [f64; 3])>, vertical: bool, snap: bool },
    /// poignée de la barre d'axes : le long d'un axe (`pixels` : déplacement à l'écran d'un mètre
    /// sur cet axe) ou au sol (axe nul, avec `basis`)
    Axis { axis: Vec3, pixels: (f32, f32), x: f32, y: f32, basis: (Vec3, Vec3), start: Vec<(String, [f64; 3])>, snap: bool },
    /// tourner la sélection : pan (horizontal) et tilt (vertical) depuis l'orientation de départ
    Rotate { x: f32, y: f32, start: Vec<(String, [f64; 3], bool)>, free: bool },
    /// anneau de rotation (outil Pivoter) : autour d'un axe du monde et du centre de la sélection ;
    /// `turn` : angle selon la souris (voir `RingTurn`), `last` / `acc` : angle à l'écran suivi pas à
    /// pas (plusieurs tours possibles)
    Ring { axis: Vec3, center: Vec3, turn: RingTurn, x: f32, y: f32, last: f32, acc: f32, start: Vec<(String, [f64; 3], [f64; 3])>, snap: bool, angle: f64 },
}

/// Lecture de l'angle d'un anneau : autour du centre à l'écran (anneau vu ouvert ; `sign` selon
/// que l'axe vient vers l'œil ou s'en éloigne) ou le long de sa tangente au point saisi (anneau vu
/// par la tranche ; pixels par radian).
#[derive(Clone, Copy)]
enum RingTurn {
    Around { cx: f32, cy: f32, sign: f32 },
    Along { tx: f32, ty: f32 },
}

/// Nombre de points de saisie par anneau de rotation.
const RING_DOTS: usize = 32;

/// Orientation après un pan (autour de la verticale du monde) et un tilt (autour de l'axe X du
/// projecteur), en angles d'Euler XYZ.
fn pan_tilt(rot: [f64; 3], pan: f64, tilt: f64) -> [f64; 3] {
    use arklight_render::glam::{DQuat, EulerRot};
    let q = DQuat::from_euler(EulerRot::XYZ, rot[0], rot[1], rot[2]);
    let q = DQuat::from_rotation_y(pan) * q * DQuat::from_rotation_x(tilt);
    let (x, y, z) = q.to_euler(EulerRot::XYZ);
    let r3 = |v: f64| (v * 1000.0).round() / 1000.0;
    [r3(x), r3(y), r3(z)]
}

/// Ce que la fenêtre montre et ce que la vue 3D dessine.
struct State {
    doc: Document,
    /// sortie de chaque projecteur (moteur lumière)
    lights: Lights,
    /// univers DMX reçus (réseau : étape 6) et ceux dont le signal arrive
    dmx: Universes,
    live: HashSet<u32>,
    /// heure du moteur lumière (départ des effets)
    clock: Instant,
    scene: Scene,
    camera: Camera,
    /// éléments choisis (projecteurs et objets)
    selection: Vec<String>,
    /// halo des sources (réglage de la vue, comme la version JavaScript)
    bloom: bool,
    /// valeurs des listes de choix de l'inspecteur
    choices: HashMap<String, Vec<Value>>,
    /// valeurs des choix des menus du clic droit (`context_menus`)
    ctx_choices: HashMap<String, Vec<Value>>,
    ctx_key: String,
    /// paquets Art-Net / sACN comptés et quand leur nombre a changé (voyants de la barre du haut)
    artnet_seen: (u64, Instant),
    sacn_seen: (u64, Instant),
    /// image de fond du plan décodée pour le sol de la vue 3D (adresse, pixels, taille)
    floor_cache: Option<(String, std::sync::Arc<Vec<u8>>, u32, u32)>,
    search: String,
    /// univers du moniteur choisi à la main (sinon il suit la sélection)
    monitor_picked: bool,
    /// journal (heure, texte, niveau), 300 lignes au plus ; sources DMX déjà annoncées
    log: std::collections::VecDeque<(String, String, &'static str)>,
    log_changed: bool,
    known_sources: HashSet<String>,
    /// surbrillance du matériel (H)
    highlight: bool,
    /// voir par la caméra (C) : caméra de la scène suivie et vue libre à retrouver
    look: Option<(String, Camera)>,
    /// console : famille pilotée (plusieurs modèles choisis), valeurs brutes
    console_family: usize,
    console_raw: bool,
    /// presse-papiers : projecteurs et objets copiés (positions d'origine), collages successifs
    clipboard: Vec<Edit>,
    pastes: u32,
    /// angle de chaque molette de la face avant (degrés : 12 par cran, comme la version JavaScript)
    knob_angles: HashMap<String, f32>,
    /// action en attente de la réponse à « Enregistrer ? » / « Quitter ? » ; reprise sans redemander
    pending: Option<Pending>,
    skip_guard: bool,
    /// inspecteur affiché (sélection et champs) : même clé, les champs sont mis à jour sur place
    inspector_key: String,
    inspector_drawn: Instant,
    /// sélection multiple : univers et adresse de départ saisis (« Adresser à la suite »)
    multi_address: Option<(String, u32, u32)>,
    /// liste Scène : recherche, tri, objets repliés
    scene_search: String,
    scene_view: scene_panel::Prefs,
    /// la vue est à redessiner
    dirty: bool,
    size: (u32, u32),
    /// taille de la vue en pixels logiques (rayons de la souris)
    view: (f32, f32),
    drag: Option<Drag>,
    /// réception Art-Net et sACN (absente pendant les essais automatiques)
    net: Option<Receiver>,
    /// version du projet déjà écrite dans la copie de travail
    autosaved: u64,
    /// essai automatique : ni copie de travail, ni réseau
    smoke: bool,
    /// univers affiché par le moniteur DMX
    monitor_universe: u32,
    /// dernier affichage de l'état du réseau
    net_shown: Instant,
    /// appui du bouton droit dans la vue (menu contextuel s'il n'y a pas eu de glisser)
    press: Option<(f32, f32)>,
    /// clic gauche en attente : choisi au relâchement s'il n'a pas glissé (sinon la caméra a tourné) ;
    /// position, élément sous la souris, Ctrl
    click: Option<(f32, f32, Option<String>, bool)>,
    /// fin d'affichage du message bref
    toast_until: Option<Instant>,
    /// ce qu'attend la fenêtre de saisie
    prompt: Option<Prompt>,
    /// fenêtre « Ré-adressage automatique »
    readdress: actions::Readdress,
    /// projecteurs qu'une appli fait clignoter (RDM IDENTIFY_DEVICE)
    identifying: HashSet<String>,
    /// blackout général : rien ne sort, les réglages ne bougent pas
    blackout: bool,
    /// console pilotée par le MIDI (gestes en cours, flash)
    console_mem: console_panel::Memory,
    /// contrôleurs MIDI : correspondances et appareils branchés (absents pendant les essais automatiques)
    midi: Midi,
    midi_inputs: Option<MidiInputs>,
    /// derniers repères des faders commandés par le contrôleur
    fader_marks: Vec<arklight_midi::FaderMark>,
    /// nombre de faders de la console affichée
    fader_count: usize,
    /// sélection de la dernière mise à jour, et si elle vient du contrôleur MIDI
    prev_selection: Vec<String>,
    midi_select: bool,
    /// dernière recherche des contrôleurs MIDI branchés
    midi_scan: Instant,
    /// plan de feu 2D
    plot: plot_view::Plot,
    /// menus embarqués des projecteurs (panneaux ouverts, état des Astera)
    onboard: arklight_onboard::Onboard,
    /// un menu embarqué a changé le projet (appui long, fin d'un message…)
    onboard_dirty: bool,
    /// dernier dessin de la face avant
    panel_drawn: Instant,
    /// assistant IA
    ai: ai_panel::Ai,
    /// dictée ORAKLE en cours (champs de texte, F8 de l'assistant)
    dictate: dictation::Dictate,
    /// pont Bluetooth LE (fausse AsteraBox, profil de test)
    ble: std::sync::Arc<arklight_ble::Hub>,
    /// projets de l'écran des projets
    projects: Vec<projects::Entry>,
    /// vignette de la vue 3D (enregistrée avec le projet), et la version du projet qu'elle montre
    thumb: Option<String>,
    thumb_rev: u64,
    thumb_at: Instant,
}

/// Ce qu'attend la fenêtre de saisie.
enum Prompt {
    /// nouveau nom d'un plan ou d'une séquence
    RenamePlan(String),
    /// nom d'un préréglage MIDI enregistré
    MidiPreset,
    /// premier numéro de fixture de la sélection
    Number,
    /// nouveau nom d'un groupe
    RenameGroup(String),
    /// distance réelle entre les deux points de calage de l'image de fond (m)
    Calibrate,
}

thread_local! {
    /// erreurs à montrer (fichiers illisibles…) : affichées par la fenêtre, jamais en boîte Windows
    static ERRORS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Une ligne du journal (onglet « Journal »).
fn log(st: &mut State, level: &'static str, text: &str) {
    st.log.push_back((chrono::Local::now().format("%H:%M:%S").to_string(), text.to_owned(), level));
    while st.log.len() > 300 {
        st.log.pop_front();
    }
    st.log_changed = true;
}

/// Message bref en bas de la vue 3D (3,5 s), noté aussi dans le journal.
fn toast(app: &AppWindow, st: &mut State, text: &str) {
    log(st, "", text);
    app.global::<App>().set_toast(text.into());
    st.toast_until = Some(Instant::now() + Duration::from_millis(3500));
}

/// Fenêtre « Ré-adressage automatique » : réglages et résultat du calcul.
fn show_readdress(app: &AppWindow, st: &State) {
    let r = &st.readdress;
    let scope = r.scopes.get(r.scope).map(String::as_str).unwrap_or("");
    let conflicts_only = scope == "conflicts";
    app.global::<App>().set_ra_scope(r.scope as i32);
    app.global::<App>().set_ra_order(arklight_core::addressing::Order::ALL.iter().position(|o| *o == r.order).unwrap_or(0) as i32);
    app.global::<App>().set_ra_pack(r.pack as i32);
    app.global::<App>().set_ra_align(actions::ALIGNS.iter().position(|a| *a == r.align).unwrap_or(0) as i32);
    app.global::<App>().set_ra_overflow(r.overflow);
    app.global::<App>().set_ra_universe(r.universe.to_string().into());
    app.global::<App>().set_ra_address(r.address.to_string().into());
    app.global::<App>().set_ra_conflicts_only(conflicts_only);
    app.global::<App>().set_ra_universe_scope(scope.starts_with("universe:"));
    let plan = &r.plan;
    let n = plan.moves.len();
    let (summary, status, warn) = if conflicts_only && plan.conflicts_before == 0 {
        (String::new(), tr("✓ Aucun chevauchement ni dépassement : rien à corriger.", "✓ No overlap or overflow: nothing to fix.").to_owned(), false)
    } else {
        let summary = if n > 0 {
            format!("{n} {} {}.", tr("projecteur(s) à ré-adresser sur", "fixture(s) to readdress out of"), plan.considered)
        } else {
            tr("Rien à changer : les adresses correspondent déjà.", "Nothing to change: the addresses already match.").to_owned()
        };
        let extra = if plan.conflicts_after > 0 && !conflicts_only {
            tr(" (conflits avec des projecteurs hors de la portée choisie)", " (conflicts with fixtures outside the chosen scope)")
        } else {
            ""
        };
        let status = format!("{} {} → {}{extra}", tr("Projecteurs en conflit :", "Fixtures in conflict:"), plan.conflicts_before, plan.conflicts_after);
        (summary, status, plan.conflicts_after > 0)
    };
    app.global::<App>().set_ra_summary(summary.into());
    app.global::<App>().set_ra_status(status.into());
    app.global::<App>().set_ra_warn(warn);
    let unplaced = if plan.unplaced.is_empty() {
        String::new()
    } else {
        format!(
            "⚠ {} {}{}",
            plan.unplaced.len(),
            tr("projecteur(s) sans place libre : ils gardent leur adresse.", "fixture(s) with no free space: they keep their address."),
            if r.overflow { "" } else { tr(" (coche « passer à l’univers suivant »)", " (tick “go to the next universe”)") }
        )
    };
    app.global::<App>().set_ra_unplaced(unplaced.into());
    let moves: Vec<MoveRow> = plan
        .moves
        .iter()
        .take(300)
        .map(|m| MoveRow { name: m.name.as_str().into(), before: actions::fmt_range(m.from.universe, m.from.address, m.footprint).into(), after: actions::fmt_range(m.to.universe, m.to.address, m.footprint).into() })
        .collect();
    app.global::<App>().set_ra_moves(ModelRc::new(VecModel::from(moves)));
    app.global::<App>().set_ra_count(n as i32);
}

/// Ouvre la fenêtre de saisie.
fn ask(app: &AppWindow, st: &mut State, prompt: Prompt, title: String, label: &str, value: String, note: &str) {
    st.prompt = Some(prompt);
    app.global::<App>().set_prompt_title(title.into());
    app.global::<App>().set_prompt_label(label.into());
    app.global::<App>().set_prompt_note(note.into());
    app.global::<App>().set_prompt_value(value.into());
    app.global::<App>().set_prompt_open(true);
}

// ------------------------------------------------------------------ réglages (langue)
#[derive(Default, Serialize, Deserialize)]
struct Settings {
    #[serde(default)]
    language: String,
    /// projets ouverts ou enregistrés récemment (le plus récent d'abord)
    #[serde(default)]
    recent: Vec<String>,
    /// correspondances MIDI, préréglage actif, préréglages enregistrés
    #[serde(default)]
    midi: Value,
    /// assistant IA : fournisseur, modèle, adresses
    #[serde(default)]
    ai: arklight_ai::AiSettings,
    /// raccourcis clavier changés (identifiant de commande → touches)
    #[serde(default)]
    shortcuts: shortcuts::Overrides,
    /// bibliothèque : onglet, tri, groupes repliés
    #[serde(default)]
    library: library_panel::Prefs,
    /// liste Scène : tri, objets repliés
    #[serde(default)]
    scene: scene_panel::Prefs,
    /// numérotation Art-Net : 1 (Art-Net 0 = Univers 1, par défaut) ou 0
    #[serde(default = "one")]
    artnet_offset: u32,
    /// dossier des projets choisi (vide : Documents\ARKLIGHT\scenes)
    #[serde(default)]
    projects_dir: String,
    /// panneaux : tailles et replis (gardés d'une fois sur l'autre)
    #[serde(default)]
    panels: Panels,
    /// l'assistant est passé une fois sur AIODOS (moteur par défaut depuis la 1.0)
    #[serde(default)]
    ai_aiodos_default: bool,
    /// version pour laquelle « Ne plus afficher » est cochée dans la fenêtre des nouveautés
    #[serde(default)]
    news_hidden: String,
}

/// Tailles (pixels) et replis des panneaux ; aide, images par seconde et halo de la vue.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Panels {
    left: f32,
    right: f32,
    bottom: f32,
    show_left: bool,
    show_right: bool,
    show_bottom: bool,
    help: bool,
    fps: bool,
    bloom: bool,
}

impl Default for Panels {
    fn default() -> Self {
        Panels { left: 260.0, right: 330.0, bottom: 230.0, show_left: true, show_right: true, show_bottom: true, help: true, fps: false, bloom: true }
    }
}

fn panels_of(app: &AppWindow) -> Panels {
    let g = app.global::<App>();
    Panels {
        left: g.get_left_w(),
        right: g.get_right_w(),
        bottom: g.get_bottom_h(),
        show_left: g.get_show_left(),
        show_right: g.get_show_right(),
        show_bottom: g.get_show_console(),
        help: g.get_show_help(),
        fps: g.get_show_fps(),
        bloom: g.get_show_bloom(),
    }
}

fn one() -> u32 {
    1
}

/// Note un projet dans la liste des récents (10 au plus).
fn remember(settings: &RefCell<Settings>, path: &std::path::Path) {
    let p = path.to_string_lossy().to_string();
    let mut s = settings.borrow_mut();
    s.recent.retain(|x| *x != p);
    s.recent.insert(0, p);
    s.recent.truncate(10);
    save_settings(&s);
}

/// Copie de travail du projet ouvert (rouverte au lancement, comme la version JavaScript).
fn autosave_path() -> Option<PathBuf> {
    settings_path().map(|p| p.with_file_name("autosave.json"))
}

fn write_autosave(doc: &Document) {
    let Some(path) = autosave_path() else { return };
    let Ok(text) = doc.project.to_json() else { return };
    let Ok(mut v) = serde_json::from_str::<Value>(&text) else { return };
    v["_session"] = json!({ "savedPath": doc.path.as_ref().map(|p| p.to_string_lossy().to_string()), "dirty": doc.dirty() });
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_string(&v).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// Session précédente : projet, son fichier et s'il restait des modifications à enregistrer.
fn read_autosave() -> Option<(Project, Option<PathBuf>, bool)> {
    let text = std::fs::read_to_string(autosave_path()?).ok()?;
    let mut v: Value = serde_json::from_str(&text).ok()?;
    let session = v.as_object_mut()?.remove("_session").unwrap_or(Value::Null);
    let path = session["savedPath"].as_str().map(PathBuf::from);
    let dirty = session["dirty"].as_bool().unwrap_or(false);
    let p = Project::from_json(&v.to_string()).ok()?;
    Some((p, path, dirty))
}

fn settings_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("ARKLIGHT-Rust").join("settings.json"))
}

fn load_settings() -> Settings {
    settings_path().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_settings(s: &Settings) {
    if let Some(p) = settings_path() {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, serde_json::to_string_pretty(s).unwrap_or_default());
    }
}

/// Langue de l'ordinateur : le français si c'est la première langue connue, sinon l'anglais.
fn system_language() -> &'static str {
    for l in sys_locale::get_locales() {
        match l.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase().as_str() {
            "fr" => return "fr",
            "en" => return "en",
            _ => {}
        }
    }
    "en"
}

thread_local! {
    /// langue affichée (« fr » ou « en »), pour les textes composés côté Rust
    static LANG: std::cell::Cell<&'static str> = const { std::cell::Cell::new("fr") };
}

/// Texte côté Rust dans la langue de l'interface (ceux de l'interface passent par @tr).
pub fn tr(fr: &'static str, en: &'static str) -> &'static str {
    if LANG.with(|l| l.get()) == "en" { en } else { fr }
}

/// Comme `tr`, pour des textes composés.
fn tr_owned(fr: String, en: String) -> String {
    if LANG.with(|l| l.get()) == "en" { en } else { fr }
}

/// Applique le choix (« auto », « fr », « en ») : les textes de l'interface changent aussitôt.
fn apply_language(choice: &str) {
    let lang = match choice {
        "fr" => "fr",
        "en" => "en",
        _ => system_language(),
    };
    LANG.with(|l| l.set(lang));
    // textes d'origine en français (chaîne vide : les textes d'origine) ; seul l'anglais a un catalogue
    let _ = slint::select_bundled_translation(if lang == "en" { "en" } else { "" });
}

/// Ce qu'annule ou refait une étape (« le déplacement »), dans la langue de l'interface.
fn step_label(fr: Option<&str>) -> SharedString {
    let Some(fr) = fr else { return SharedString::new() };
    if LANG.with(|l| l.get()) != "en" {
        return fr.into();
    }
    let en = match fr {
        "le déplacement" => "the move".to_owned(),
        "le changement de nom" => "the rename".to_owned(),
        "l’adressage DMX" => "the DMX addressing".to_owned(),
        "le réglage du projecteur" => "the fixture setting".to_owned(),
        "l’ajout du projecteur" => "adding the fixture".to_owned(),
        "la suppression du projecteur" => "deleting the fixture".to_owned(),
        "l’ajout de l’objet" => "adding the object".to_owned(),
        "l’ajout du grip" => "adding the grip".to_owned(),
        "le réglage de l’objet" => "the object setting".to_owned(),
        "la suppression de l’objet" => "deleting the object".to_owned(),
        "la création du groupe" => "creating the group".to_owned(),
        "la modification du groupe" => "the group change".to_owned(),
        "la suppression du groupe" => "deleting the group".to_owned(),
        "le réglage de l’ambiance" => "the ambience setting".to_owned(),
        "la suppression de la sélection" => "deleting the selection".to_owned(),
        other => {
            let n: String = other.chars().filter(char::is_ascii_digit).collect();
            if other.starts_with("la suppression de") {
                format!("deleting {n} fixtures")
            } else if other.starts_with("l’ajout de") {
                format!("adding {n} items")
            } else {
                "the change".to_owned()
            }
        }
    };
    en.into()
}

// ------------------------------------------------------------------ projet → fenêtre
/// Couleur d'aperçu (pastille de la liste Scène) d'une couleur sRGB linéaire.
fn swatch(rgb: [f64; 3]) -> slint::Color {
    let m = rgb.iter().cloned().fold(1e-6, f64::max);
    let c = |v: f64| (linear_to_srgb((v / m).clamp(0.0, 1.0)) * 255.0).round() as u8;
    slint::Color::from_rgb_u8(c(rgb[0]), c(rgb[1]), c(rgb[2]))
}

fn model_name(type_id: &str) -> String {
    let lib = library();
    if let Some(t) = lib.fixture(type_id) {
        return if t.name.starts_with(&t.brand) { t.name.clone() } else { format!("{} {}", t.brand, t.name) };
    }
    lib.prop(type_id).map(|t| t.name.clone()).or_else(|| lib.grip(type_id).map(|g| g.name.clone())).unwrap_or_else(|| type_id.to_owned())
}

/// Lignes de la liste Scène (recherche, tri), compteur et tri choisi.
fn show_scene(app: &AppWindow, st: &State) {
    let color = |id: &str| st.lights.get(id).map_or([0.0; 3], |l| l.average());
    let (rows, count) = scene_panel::rows(&st.doc, &st.selection, &color, &st.scene_search, &st.scene_view);
    let g = app.global::<App>();
    g.set_scene_rows(ModelRc::new(VecModel::from(rows)));
    g.set_scene_count(count.into());
    g.set_scene_sort(st.scene_view.sort as i32);
}

/// Ajout d'un élément de la bibliothèque à un point du sol.
fn add_edit(st: &State, kind: &str, id: &str, at: Option<Vec3>) -> Edit {
    match kind {
        "fixture" => {
            // comme la version JavaScript : « + » répartit les projecteurs en arc autour du plateau,
            // à une hauteur selon leur forme, tournés vers le personnage
            let ty = library().fixture(id);
            let shape = ty.map_or("", |t| t.geometry.kind.as_str());
            let (x, z) = match at {
                Some(p) => (p.x as f64, p.z as f64),
                None if shape == "box-device" => (0.7, 2.6),
                None => {
                    let n = st.doc.project.fixtures.len() as f64;
                    let angle = (-60.0 + (n * 37.0) % 120.0).to_radians();
                    let r = if shape == "tube" { 2.0 } else { 3.0 };
                    (angle.sin() * r, angle.cos() * r)
                }
            };
            let y = match shape {
                "box-device" => 1.0,
                "tube" => ty.and_then(|t| t.dim("length")).unwrap_or(1.0) / 2.0 + 0.5,
                "bulb" => 2.6,
                _ => 2.2,
            };
            let r3 = |v: f64| (v * 1000.0).round() / 1000.0;
            let position = [r3(x), r3(y), r3(z)];
            let rotation = if matches!(shape, "tube" | "bulb" | "box-device") { [0.0; 3] } else { actions::look_at(position, actions::main_aim_point(st)).map(r3) };
            Edit::AddFixture { type_id: id.to_owned(), position, rotation, name: None, settings: Map::new() }
        }
        _ => {
            let at = at.unwrap_or_else(|| st.drop_point());
            let yaw = library().prop(id).and_then(|t| t.extra.get("yaw")).and_then(Value::as_f64).unwrap_or(0.0);
            let y = if library().grip(id).is_some_and(|g| matches!(g.family.as_deref(), Some("frame" | "flag" | "bounce"))) { 1.5 } else { 0.0 };
            Edit::AddObject { type_id: id.to_owned(), position: [at.x as f64, y, at.z as f64], rotation: [0.0, yaw, 0.0], name: None, props: Map::new() }
        }
    }
}

/// Montre la bibliothèque (onglet, tri, groupes repliés, recherche).
fn show_library(app: &AppWindow, search: &str, prefs: &library_panel::Prefs) {
    let (rows, counts) = library_panel::rows(library(), search, prefs);
    let g = app.global::<App>();
    g.set_library(ModelRc::new(VecModel::from(rows)));
    g.set_lib_counts(ModelRc::new(VecModel::from(counts)));
    g.set_lib_tab(prefs.tab as i32);
    g.set_lib_sort(prefs.sort as i32);
}

/// Univers et adresse de départ proposés pour la sélection (`renderMulti`) : l'univers du premier
/// projecteur, la plus petite adresse dans cet univers.
fn multi_start(st: &State) -> (u32, u32) {
    let list: Vec<&arklight_core::Fixture> = st.selection.iter().filter_map(|id| st.doc.fixture(id)).collect();
    let u = list.first().and_then(|f| f.settings.universe).unwrap_or(1);
    let a = list.iter().filter(|f| f.settings.universe.unwrap_or(1) == u).filter_map(|f| f.settings.dmx_address).min().unwrap_or(1);
    (u, a)
}

/// Ce que l'inspecteur montre d'un projecteur seul hors du projet : signal, entrée des Astera,
/// lampe, pied, face avant, tirettes de la console.
fn inspector_live(st: &mut State) -> inspector::Live {
    let Some(f) = st.selection.first().and_then(|id| st.doc.fixture(id)).filter(|_| st.selection.len() == 1) else { return inspector::Live::default() };
    let (id, u) = (f.id.clone(), f.settings.universe.unwrap_or(1));
    let sources = st
        .net
        .as_ref()
        .map(|n| n.status().sources.iter().filter(|x| x.universes.contains(&u)).map(|x| format!("{} ({})", if x.name.is_empty() { &x.address } else { &x.name }, x.protocol)).collect())
        .unwrap_or_default();
    let bottom = st.scene.picks.iter().find(|p| p.id == id).map(|p| p.lo.y as f64);
    let (_, faders, _) = console_panel::panel(&st.doc, std::slice::from_ref(&id), &st.lights, &st.dmx);
    let mut live = inspector::Live {
        has_data: st.dmx.contains_key(&u),
        live: st.live.contains(&u),
        sources,
        runtime: st.lights.runtime(&id).cloned(),
        bottom,
        front_panel: false,
        faders: faders.into_iter().map(|f| (f.label, f.value, f.max, f.text)).collect(),
    };
    live.front_panel = onboard_panel::target(st).as_deref() == Some(id.as_str());
    live
}

/// Deux champs identiques (les listes comparées par leur contenu).
fn same_field(a: &Field, b: &Field) -> bool {
    let list = |m: &ModelRc<SharedString>| m.iter().collect::<Vec<_>>();
    a.key == b.key && a.label == b.label && a.kind == b.kind && a.text == b.text && a.value == b.value && a.min == b.min && a.max == b.max && a.step == b.step && a.index == b.index && a.on == b.on && a.tip == b.tip && list(&a.options) == list(&b.options) && list(&a.keys) == list(&b.keys)
}

/// Inspecteur de la sélection. Même sélection et mêmes champs : mis à jour sur place (un curseur
/// qu'on glisse ou un champ en cours de saisie ne sont pas recréés).
fn show_inspector(app: &AppWindow, st: &mut State) {
    st.inspector_drawn = Instant::now();
    let live = inspector_live(st);
    let panel = inspector::panel(&st.doc, &st.selection, &live);
    app.global::<App>().set_inspector_title(panel.title.into());
    st.choices = panel.choices;
    let fields: Vec<Field> = panel
        .fields
        .into_iter()
        .map(|f| {
            // longue liste rangée par famille : menu à sous-menus
            let menu = (f.kind == "combo").then(|| combo_menu(&f.key, &f.options, f.index, st.choices.get(&f.key).map_or(&[][..], Vec::as_slice)));
            let mut row = field_row(f);
            if let Some((items, subs)) = menu.filter(|m| !m.1.is_empty()) {
                row.menu_items = ModelRc::new(VecModel::from(items));
                row.menu_subs = ModelRc::new(VecModel::from(subs));
            }
            row
        })
        .collect();
    let key = format!("{}|{}", st.selection.join(","), fields.iter().map(|f| format!("{}:{}", f.kind, f.key)).collect::<Vec<_>>().join(","));
    let groups = field_groups(&fields);
    let model = app.global::<App>().get_field_groups();
    if st.inspector_key == key && model.row_count() == groups.len() {
        for (i, g) in groups.into_iter().enumerate() {
            let Some(old) = model.row_data(i) else { continue };
            if old.title != g.title || old.fields.row_count() != g.fields.row_count() {
                model.set_row_data(i, g);
                continue;
            }
            for j in 0..g.fields.row_count() {
                let Some(new) = g.fields.row_data(j) else { continue };
                if !old.fields.row_data(j).is_some_and(|o| same_field(&o, &new)) {
                    old.fields.set_row_data(j, new);
                }
            }
        }
        return;
    }
    st.inspector_key = key;
    app.global::<App>().set_field_groups(ModelRc::new(VecModel::from(groups)));
}

/// Les choix d'une liste rangés par famille : toiles et gélatines par groupe, boîtes « Dômes › … » ;
/// ceux sans famille d'abord.
fn combo_menu(key: &str, options: &[String], index: i32, values: &[Value]) -> (Vec<CtxItem>, Vec<CtxSub>) {
    let lib = library();
    let filter_label = |id: &str| match id {
        "diffusion" => tr("Diffusions", "Diffusions"),
        "ctb" => tr("CTB (vers la lumière du jour)", "CTB (towards daylight)"),
        "cto" => tr("CTO / CTS (vers le tungstène)", "CTO / CTS (towards tungsten)"),
        "green" => tr("Vert / magenta", "Green / magenta"),
        "nd" => tr("Neutres (ND)", "Neutral density (ND)"),
        _ => tr("Couleurs", "Colours"),
    };
    let cloth_label = |id: &str| match id {
        "silk" => tr("Soies", "Silks"),
        "grid" => tr("Grids", "Grids"),
        "frost" => tr("Frosts et muslins", "Frosts and muslins"),
        "bounce" => tr("Réflecteurs", "Bounce"),
        _ => tr("Noirs et tulles", "Solids and nets"),
    };
    let mut items = Vec::new();
    let mut subs: Vec<(String, Vec<CtxItem>)> = Vec::new();
    for (i, label) in options.iter().enumerate() {
        let value = values.get(i).and_then(Value::as_str);
        let (family, title) = if let Some(g) = value.and_then(|id| lib.cloths.get(id)).filter(|_| key == "cloth").map(|c| c.group.clone()) {
            (g.as_deref().map(|g| cloth_label(g).to_owned()), label.clone())
        } else if let Some(filter) = value.and_then(|id| lib.filter(id)) {
            // sur un cadre, à côté des toiles : ce sont des gélatines
            let group = filter_label(&filter.group);
            (Some(if key == "cloth" { format!("{} : {group}", tr("Gélatines", "Gels")) } else { group.to_owned() }), label.clone())
        } else {
            match label.split_once(" › ") {
                Some((fam, name)) => (Some(fam.to_owned()), name.to_owned()),
                None => (None, label.clone()),
            }
        };
        let item = CtxItem { title: title.into(), action: format!("set:{key}:{i}").into(), checked: i as i32 == index, index: i as i32 };
        match family {
            Some(fam) => match subs.iter_mut().find(|s| s.0 == fam) {
                Some(s) => s.1.push(item),
                None => subs.push((fam, vec![item])),
            },
            None => items.push(item),
        }
    }
    (items, subs.into_iter().map(|(t, list)| CtxSub { title: t.into(), items: ModelRc::new(VecModel::from(list)) }).collect())
}

/// Menus du clic droit qui dépendent de la sélection (`fixtureMenu`, `objectMenu`) : pied et
/// accessoires des projecteurs ; toile, pieds et pose d'un objet. Faits d'après les listes de
/// l'inspecteur ; un choix passe par `inspector::apply` (action « set:clé:n° »).
fn context_menus(app: &AppWindow, st: &mut State) {
    let live = inspector_live(st);
    let panel = inspector::panel(&st.doc, &st.selection, &live);
    let g = app.global::<App>();
    let objects = !st.selection.is_empty() && st.selection.iter().all(|id| st.doc.object(id).is_some());
    g.set_ctx_object(objects);
    let camera = st.selection.first().filter(|id| objects && st.scene.lenses.iter().any(|l| l.id == **id)).cloned().unwrap_or_default();
    g.set_ctx_camera(camera.into());
    let keys: &[&str] = if objects { &["cloth", "stand", "p.pose"] } else { &["support", "modifier", "front", "grid", "gel"] };
    let mut menus = Vec::new();
    for key in keys {
        let Some(f) = panel.fields.iter().find(|f| f.kind == "combo" && f.key == *key) else { continue };
        let values = panel.choices.get(*key).cloned().unwrap_or_default();
        let (items, subs) = combo_menu(key, &f.options, f.index, &values);
        // titre : « Pied : Junior », « Gélatine : CTO 1/2 »… (choix en cours, s'il est court)
        let label = if *key == "support" { tr("Pied", "Stand").to_owned() } else { f.label.clone() };
        // (« 250 — 1/2 White Diffusion » : « 250 »)
        let current = f.options.get(f.index.max(0) as usize).map(|c| c.rsplit(" › ").next().unwrap_or(c).split(" — ").next().unwrap_or(c).to_owned()).unwrap_or_default();
        let title = if !current.is_empty() && current.chars().count() <= 28 { format!("{label} : {current}") } else { label };
        menus.push(CtxMenu { title: title.into(), items: ModelRc::new(VecModel::from(items)), subs: ModelRc::new(VecModel::from(subs)) });
    }
    st.ctx_choices = panel.choices;
    // remplacés seulement s'ils changent (un menu ouvert n'est pas refait sous la souris)
    let key = menus.iter().map(|m| format!("{}|{}|{}", m.title, m.items.iter().map(|i| format!("{}{}", i.title, i.checked)).collect::<String>(), m.subs.iter().map(|s| format!("{}{}", s.title, s.items.iter().map(|i| format!("{}{}", i.title, i.checked)).collect::<String>())).collect::<String>())).collect::<Vec<_>>().join("¦");
    if st.ctx_key != key {
        st.ctx_key = key;
        g.set_ctx_menus(ModelRc::new(VecModel::from(menus)));
    }
}

/// Sections de l'inspecteur : les champs coupés à chaque titre (« header »).
fn field_groups(fields: &[Field]) -> Vec<FieldGroup> {
    let mut out: Vec<(String, Vec<Field>)> = Vec::new();
    for f in fields {
        if f.kind == "header" {
            out.push((f.label.to_uppercase(), Vec::new()));
        } else {
            if out.is_empty() {
                out.push((String::new(), Vec::new()));
            }
            out.last_mut().unwrap().1.push(f.clone());
        }
    }
    out.into_iter().filter(|g| !g.1.is_empty()).map(|(title, list)| FieldGroup { title: title.into(), fields: ModelRc::new(VecModel::from(list)) }).collect()
}

/// Couleur « #rrggbb » (blanc si illisible).
fn hex_color(t: &str) -> slint::Color {
    let h = t.trim().trim_start_matches('#');
    match (h.len(), u32::from_str_radix(h, 16)) {
        (6, Ok(v)) => slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8),
        _ => slint::Color::from_rgb_u8(255, 255, 255),
    }
}

fn field_row(f: inspector::Field) -> Field {
    let swatch = if f.kind == "color" { hex_color(&f.text) } else { slint::Color::default() };
    Field {
        key: f.key.into(),
        label: f.label.into(),
        kind: f.kind.into(),
        text: f.text.into(),
        value: f.value,
        min: f.min,
        max: f.max,
        step: f.step,
        options: ModelRc::new(VecModel::from(f.options.into_iter().map(SharedString::from).collect::<Vec<_>>())),
        keys: ModelRc::new(VecModel::from(f.keys.into_iter().map(SharedString::from).collect::<Vec<_>>())),
        index: f.index,
        on: f.on,
        tip: f.tip.into(),
        swatch,
        ..Field::default()
    }
}

impl State {
    /// Calcule la sortie des projecteurs à l'heure actuelle ; vrai si la lumière a changé.
    fn update_lights(&mut self) -> bool {
        if let Some(net) = &self.net {
            net.snapshot(&mut self.dmx, &mut self.live);
        }
        let time = self.clock.elapsed().as_secs_f64();
        let ctx = Ctx { dmx: &self.dmx, live: &self.live, time };
        let (identifying, blackout) = (&self.identifying, self.blackout);
        let (onboard, fixtures, live) = (&mut self.onboard, &self.doc.project.fixtures, &self.live);
        let mut outcomes = Vec::new();
        let lit = self.lights.update_with(fixtures, &ctx, |f, rt, out| {
            let id = f.id.as_str();
            if onboard.peek(id).is_some() {
                // panneaux ouverts : appuis longs, fins de message, puis retour visuel sur la lumière
                let env = arklight_onboard::Env { time, fixtures, live, app_connected: false };
                let o = onboard.tick(f, rt, &env);
                if !o.is_empty() {
                    outcomes.push((f.id.clone(), o));
                }
                let pixels = out.len() / 3;
                onboard.override_pixels(f, rt, &env, out, pixels);
            }
            if identifying.contains(id) {
                // identification RDM : clignote en blanc, comme un vrai projecteur
                out.fill(if (time * 2.5) % 1.0 < 0.5 { 1.0 } else { 0.02 });
            }
            if blackout {
                out.fill(0.0);
            }
        });
        for (id, o) in outcomes {
            if let Some(e) = onboard_panel::outcome_edit(self, &id, o) {
                self.doc.apply(e);
                self.onboard_dirty = true;
            }
        }
        // diagnostic (ARKLIGHT_DEBUG_BLINK) : projecteurs qui s'allument ou s'éteignent, image par image
        if std::env::var_os("ARKLIGHT_DEBUG_BLINK").is_some() {
            thread_local! {
                static LIT: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
            }
            LIT.with(|m| {
                let mut m = m.borrow_mut();
                for f in &self.doc.project.fixtures {
                    let Some(l) = self.lights.get(&f.id) else { continue };
                    let on = l.average().iter().any(|v| *v > 1e-4);
                    if let Some(prev) = m.insert(f.id.clone(), on)
                        && prev != on
                    {
                        eprintln!("{:.3} s  {}  {}  ({})", time, f.name, if on { "allumé" } else { "éteint" }, l.source.as_str());
                    }
                }
            });
        }
        lit
    }

    /// Scène du moteur (et contour de la sélection).
    fn rebuild_scene(&mut self) {
        let mut scene = scene_from_project(&self.doc.project, &self.lights);
        scene.floor_image = plot_view::floor_image(&mut self.floor_cache, &self.doc.project);
        scene.bloom = self.bloom;
        scene.highlight = self.highlight;
        let picks: Vec<_> = scene.picks.iter().filter(|p| self.selection.contains(&p.id)).cloned().collect();
        for p in picks {
            scene.items.extend(outline(p.lo, p.hi, if p.object { 0xffad3b } else { 0x4d8dff }));
        }
        self.scene = scene;
        self.dirty = true;
    }

    /// Point visé par la caméra, au sol (où se posent les nouveaux éléments).
    fn drop_point(&self) -> Vec3 {
        Vec3::new(self.camera.target.x, 0.0, self.camera.target.z)
    }
}

/// Montre tout le projet dans la fenêtre : liste, inspecteur, ambiance, menus.
fn refresh(app: &AppWindow, st: &mut State) {
    let doc = &st.doc;
    st.selection.retain(|id| doc.fixture(id).is_some() || doc.object(id).is_some());
    if let Some(net) = &st.net {
        // univers du projet : annoncés aux consoles, groupes sACN rejoints
        let used: Vec<u32> = st.doc.project.fixtures.iter().map(|f| f.settings.universe.unwrap_or(1)).collect();
        net.set_universes(&used);
        // RDM : les applis (Luminair, consoles) découvrent et ré-adressent les projecteurs de la scène
        net.set_rdm_fixtures(&st.doc.project.fixtures);
    }
    // sélection changée dans l'application : le contrôleur MIDI repart des premiers faders
    if st.selection != st.prev_selection {
        if !st.midi_select && st.midi.selection_changed() {
            st.fader_marks = st.midi.fader_marks();
        }
        st.prev_selection = st.selection.clone();
    }
    st.midi_select = false;
    app.global::<App>().set_blackout(st.blackout);
    st.plot.dirty = true;
    app.global::<App>().set_plot_mode(st.plot.on);
    if st.plot.on {
        app.global::<App>().set_plot_hint(plot_view::hint(st).into());
        app.global::<App>().set_plot_tool(plot_view::tool_index(st) as i32);
        let g = app.global::<App>();
        match plot_view::item_info(st) {
            Some(it) => {
                g.set_plot_item_kind(it.name.into());
                g.set_plot_item_text(it.text.into());
                g.set_plot_item_closed(it.closed);
                g.set_plot_item_filled(it.filled);
                g.set_plot_item_is_text(it.is_text);
                g.set_plot_item_dash(it.dash);
                g.set_plot_item_width(plot_view::WIDTHS.iter().position(|w| *w == it.width).unwrap_or(1) as i32);
                g.set_plot_item_size(plot_view::SIZES.iter().position(|s| *s == it.size).unwrap_or(1) as i32);
                g.set_plot_item_height(format!("{}", (it.height * 10.0).round() / 10.0).into());
            }
            None => g.set_plot_item_kind(SharedString::new()),
        }
        let m = plot_view::meta_info(st);
        g.set_plot_bg(m.background.into());
        g.set_plot_underlay(m.underlay.is_some());
        let (uw, ur, uo) = m.underlay.unwrap_or((20.0, 0.0, 0.6));
        g.set_plot_u_width(format!("{}", (uw * 100.0).round() / 100.0).into());
        g.set_plot_u_rotation(format!("{}", ur.round()).into());
        g.set_plot_u_opacity(uo as f32);
        g.set_plot_u_floor(m.floor3d);
        g.set_plot_production(m.production.into());
        g.set_plot_author(m.author.into());
    }
    app.global::<App>().set_plot_flags(ModelRc::new(VecModel::from(plot_view::flags(&st.plot.options))));
    st.update_lights();
    st.rebuild_scene();
    let p = &st.doc.project;
    app.global::<App>().set_project_name(p.name.as_str().into());
    app.global::<App>().set_dirty(st.doc.dirty());
    app.global::<App>().set_fixture_count(p.fixtures.len() as i32);
    app.global::<App>().set_object_count(p.objects.len() as i32);
    show_scene(app, st);
    let env = &st.doc.project.environment;
    app.global::<App>().set_show_cyc(env.cyc);
    app.global::<App>().set_show_floor(env.floor);
    app.global::<App>().set_show_stands(env.stands());
    // étiquettes 3D : réglages gardés dans le projet (comme la version JavaScript)
    app.global::<App>().set_show_labels(env.extra.get("labels") == Some(&Value::Bool(true)));
    app.global::<App>().set_show_patch(env.extra.get("labelPatch") == Some(&Value::Bool(true)));
    app.global::<App>().set_show_bloom(st.bloom);
    app.global::<App>().set_haze(env.haze as f32);
    app.global::<App>().set_exposure_ev(env.exposure.max(1e-6).log2() as f32);
    app.global::<App>().set_ambient(env.ambient as f32);
    app.global::<App>().set_undo_label(step_label(st.doc.undo_label()));
    app.global::<App>().set_redo_label(step_label(st.doc.redo_label()));
    let (plans, current) = plan_list(&st.doc);
    app.global::<App>().set_plans(ModelRc::new(VecModel::from(plans.into_iter().map(|(_, l)| SharedString::from(l)).collect::<Vec<_>>())));
    app.global::<App>().set_plan_index(current as i32);
    show_monitor(app, st);
    let menu = actions::menu_info(st);
    app.global::<App>().set_selected_count(menu.count as i32);
    app.global::<App>().set_selected_fixtures(menu.fixtures as i32);
    app.global::<App>().set_selection_off(menu.any_off);
    app.global::<App>().set_selection_v5(menu.v5);
    app.global::<App>().set_selection_aimable(menu.aimable);
    app.global::<App>().set_same_label(menu.same_label.into());
    let groups: Vec<GroupRow> = menu.groups.into_iter().map(|(id, name, count, all, some)| GroupRow { id: id.into(), name: name.into(), count: count as i32, all, some }).collect();
    app.global::<App>().set_groups(ModelRc::new(VecModel::from(groups)));
    app.global::<App>().set_aim_targets(ModelRc::new(VecModel::from(menu.aims.into_iter().map(SharedString::from).collect::<Vec<_>>())));
    show_inspector(app, st);
    // panneaux repliés : leur nom, une lettre sous l'autre
    let letters = |w: &str| ModelRc::new(VecModel::from(w.chars().map(|c| SharedString::from(c.to_string())).collect::<Vec<_>>()));
    app.global::<App>().set_left_strip(letters(tr("BIBLIOTHÈQUE", "LIBRARY")));
    app.global::<App>().set_right_strip(letters(tr("INSPECTEUR", "INSPECTOR")));
    // une famille (même modèle, même profil) à la fois
    let families = console_panel::families(&st.doc, &st.selection);
    st.console_family = st.console_family.min(families.len().saturating_sub(1));
    let ids = families.get(st.console_family).cloned().unwrap_or_default();
    let (title, faders, held) = console_panel::panel_raw(&st.doc, &ids, &st.lights, &st.dmx, st.console_raw);
    let head = console_panel::head(&st.doc, &st.selection, &families, &ids);
    {
        let g = app.global::<App>();
        let strings = |v: Vec<String>| ModelRc::new(VecModel::from(v.into_iter().map(SharedString::from).collect::<Vec<_>>()));
        g.set_console_sub(head.sub.into());
        g.set_console_chips(strings(head.chips));
        g.set_console_chip(st.console_family as i32);
        g.set_console_profiles(strings(head.profiles));
        g.set_console_profile(head.profile as i32);
        g.set_console_place(head.place.into());
        g.set_console_pixels(head.pixels as i32);
        g.set_console_note(head.note.into());
        g.set_console_warn(head.warn.into());
        g.set_console_raw(st.console_raw);
    }
    let title = if ids.is_empty() { title } else { head.title };
    app.global::<App>().set_console_title(title.into());
    app.global::<App>().set_console_held(held);
    st.fader_count = faders.len();
    let marks = &st.fader_marks;
    let faders: Vec<ConsoleFader> = faders
        .into_iter()
        .enumerate()
        .map(|(i, f)| {
            let mark = marks.iter().find(|m| m.index as usize == i + 1).map(|m| m.label.clone().unwrap_or_else(|| "MIDI".into())).unwrap_or_default();
            ConsoleFader { label: f.label.into(), channel: f.channel.into(), value: f.value, max: f.max, text: f.text.into(), mark: mark.into() }
        })
        .collect();
    // sur place : un fader recréé pendant qu'on le tire lâcherait la souris après un cran
    let model = app.global::<App>().get_console_faders();
    if model.row_count() == faders.len() {
        for (i, f) in faders.into_iter().enumerate() {
            if model.row_data(i).as_ref() != Some(&f) {
                model.set_row_data(i, f);
            }
        }
    } else {
        app.global::<App>().set_console_faders(ModelRc::new(VecModel::from(faders)));
    }
    show_midi(app, st);
    context_menus(app, st);
    app.window().request_redraw();
}

/// Projecteurs dans l'ordre de la liste Scène (univers, adresse).
fn fixture_order(doc: &Document) -> Vec<String> {
    let mut list: Vec<&arklight_core::Fixture> = doc.project.fixtures.iter().collect();
    list.sort_by_key(|f| (f.settings.universe.unwrap_or(1), f.settings.dmx_address.unwrap_or(0)));
    list.into_iter().map(|f| f.id.clone()).collect()
}

/// Libellé d'une sorte de fonction MIDI (« Console — Fader de la console »).
fn midi_kind_label(i: usize) -> String {
    let k = &TARGET_KINDS[i];
    format!("{} — {}", i18n::t(k.group), i18n::t(k.label))
}

const MIDI_INDEX_KINDS: [&str; 6] = ["fader", "fixtureIndex", "groupIndex", "planIndex", "fixtureLatch", "groupLatch"];

/// Fenêtre MIDI : préréglages, appareils, correspondances.
fn show_midi(app: &AppWindow, st: &State) {
    let midi = &st.midi;
    let mut presets = vec![SharedString::from(tr("Aucun préréglage (correspondances apprises)", "No preset (learned mappings)"))];
    presets.extend(arklight_midi::MIDI_PRESETS.iter().map(|p| SharedString::from(i18n::t(&arklight_midi::presets::preset_label(p)))));
    presets.extend(midi.user_presets().iter().map(|p| SharedString::from(format!("{} — {}", tr("Enregistré", "Saved"), p.name))));
    let index = midi.preset().and_then(|a| {
        arklight_midi::MIDI_PRESETS.iter().position(|p| p.id == a.id).map(|i| i + 1).or_else(|| midi.user_presets().iter().position(|p| p.id == a.id).map(|i| i + 1 + arklight_midi::MIDI_PRESETS.len()))
    });
    app.global::<App>().set_midi_presets(ModelRc::new(VecModel::from(presets)));
    let active = midi.active_preset();
    app.global::<App>().set_midi_builtin(matches!(active, Some(arklight_midi::controller::PresetRef::Builtin(_))));
    app.global::<App>().set_midi_user(matches!(active, Some(arklight_midi::controller::PresetRef::User(_))));
    app.global::<App>().set_midi_channel(midi.preset().map_or(1, |p| p.channel));
    app.global::<App>().set_midi_preset_index(index.unwrap_or(0) as i32);
    let devices = st.midi_inputs.as_ref().map(|i| i.devices().iter().map(|d| d.name.clone()).collect::<Vec<_>>()).unwrap_or_default();
    app.global::<App>().set_midi_on(!devices.is_empty());
    app.global::<App>().set_midi_tip(format!("{} : {}", tr("Contrôleur MIDI", "MIDI controller"), devices.join(", ")).into());
    app.global::<App>().set_midi_devices(
        if !arklight_midi::input::SUPPORTED {
            tr("MIDI non disponible sur ce système", "MIDI not available on this system").into()
        } else if devices.is_empty() {
            tr("Aucun contrôleur MIDI branché", "No MIDI controller connected").into()
        } else {
            format!("{} {}", tr("Contrôleurs :", "Controllers:"), devices.join(", ")).into()
        },
    );
    app.global::<App>().set_midi_last(i18n::t(&midi.last_text()).into());
    app.global::<App>().set_midi_status(i18n::t(&midi.learning_status()).into());
    app.global::<App>().set_midi_page(i18n::t(&midi.page_info()).into());
    let summary = match midi.active_preset() {
        Some(arklight_midi::controller::PresetRef::Builtin(p)) => p.summary.iter().map(|l| i18n::t(l)).collect::<Vec<_>>().join(" · "),
        _ => String::new(),
    };
    app.global::<App>().set_midi_summary(summary.into());
    app.global::<App>().set_midi_learning(midi.learning().is_some());
    let learning_id = match midi.learning() {
        Some(arklight_midi::Learning::Relearn(id)) => Some(id.clone()),
        _ => None,
    };
    let lib_t = |s: &str| i18n::t(s);
    let groups: Vec<String> = st.doc.project.groups.iter().map(|g| g.name.clone()).collect();
    let fixtures: Vec<String> = st.doc.project.fixtures.iter().map(|f| f.name.clone()).collect();
    let rows: Vec<MidiRow> = midi
        .all()
        .iter()
        .map(|m| {
            let kind = m.target.kind.as_str();
            let kind_index = TARGET_KINDS.iter().position(|k| k.id == kind).unwrap_or(0);
            let (params, param_index): (Vec<String>, usize) = if MIDI_INDEX_KINDS.contains(&kind) {
                ((1..=32).map(|i| i.to_string()).collect(), m.target.index_or_1() as usize - 1)
            } else if matches!(kind, "group" | "groupLevel") {
                let i = groups.iter().position(|g| Some(g) == m.target.name.as_ref()).unwrap_or(0);
                (groups.clone(), i)
            } else if matches!(kind, "fixture" | "fixtureLevel") {
                let i = fixtures.iter().position(|f| Some(f) == m.target.name.as_ref()).unwrap_or(0);
                (fixtures.clone(), i)
            } else if kind == "command" {
                let i = commands::COMMANDS.iter().position(|c| Some(c.id) == m.target.id.as_deref()).unwrap_or(0);
                (commands::COMMANDS.iter().map(|c| tr(c.fr, c.en).to_owned()).collect(), i)
            } else {
                (Vec::new(), 0)
            };
            MidiRow {
                id: m.id.as_str().into(),
                source: m.source.map(|s| arklight_midi::message::describe_source_with(&s, lib_t)).unwrap_or_else(|| tr("(à apprendre)", "(to learn)").to_owned()).into(),
                kind_index: kind_index as i32,
                params: ModelRc::new(VecModel::from(params.into_iter().map(SharedString::from).collect::<Vec<_>>())),
                param_index: param_index as i32,
                note: arklight_midi::controller::param_note(kind).map(i18n::t).unwrap_or_default().into(),
                learning: learning_id.as_deref() == Some(m.id.as_str()),
                preset: m.preset,
            }
        })
        .collect();
    app.global::<App>().set_midi_rows(ModelRc::new(VecModel::from(rows)));
}

/// Enregistre les réglages MIDI sur le poste.
fn save_midi(settings: &RefCell<Settings>, st: &State) {
    let mut s = settings.borrow_mut();
    s.midi = st.midi.settings();
    save_settings(&s);
}

/// Applique ce que demande le contrôleur MIDI ; renvoie les commandes du menu à lancer (une fois
/// l'état rendu) et si les réglages MIDI sont à enregistrer.
fn apply_midi(app: &AppWindow, st: &mut State, list: Vec<MidiAction>) -> (Vec<String>, bool) {
    let mut commands = Vec::new();
    let mut save = false;
    let mut changed = false;
    for a in list {
        let e = match a {
            MidiAction::Select { ids, primary } => {
                let mut ids = ids;
                if let Some(p) = primary
                    && let Some(i) = ids.iter().position(|x| *x == p)
                {
                    let id = ids.remove(i);
                    ids.insert(0, id);
                }
                st.selection = ids;
                st.midi_select = true;
                changed = true;
                None
            }
            MidiAction::Blackout(on) => {
                st.blackout = on;
                changed = true;
                None
            }
            MidiAction::Fader { index, value } => {
                let i = index.max(1) as usize - 1;
                console_panel::fader_max(&st.doc, &st.selection, i).and_then(|max| console_panel::move_fader(&st.doc, &st.selection, &st.lights, &st.dmx, i, (value * max as f64) as f32))
            }
            MidiAction::FaderMarks { marks, .. } => {
                st.fader_marks = marks;
                changed = true;
                None
            }
            MidiAction::NextFamily => {
                // modèle suivant de la sélection en tête (la console montre le premier)
                let key = |st: &State, id: &str| st.doc.fixture(id).map(|f| (f.type_id.clone(), f.settings.dmx_mode.clone()));
                if let Some(first) = st.selection.first().and_then(|id| key(st, id))
                    && let Some(i) = st.selection.iter().position(|id| key(st, id).is_some_and(|k| k != first))
                {
                    st.selection.rotate_left(i);
                    st.midi_select = true;
                    changed = true;
                }
                None
            }
            MidiAction::Flash { fixtures, on } => console_panel::flash(&st.doc, &fixtures, &st.lights, &st.dmx, on, &mut st.console_mem),
            MidiAction::Intensity { fixtures, value } => console_panel::set_intensity(&st.doc, &fixtures, &st.lights, &st.dmx, value, &mut st.console_mem),
            MidiAction::Color { fixtures, change } => {
                let mut c = arklight_core::engine::console::ColorChange::default();
                match change {
                    MidiColor::Cct(v) => c.cct = Some(v),
                    MidiColor::Tint(v) => c.tint = Some(v),
                    MidiColor::Hue(v) => c.hue = Some(v),
                    MidiColor::Sat(v) => c.sat = Some(v),
                }
                console_panel::set_color(&st.doc, &fixtures, &st.lights, &st.dmx, c, &mut st.console_mem)
            }
            MidiAction::OpenPlan(id) => {
                if st.doc.switch_plan(&id) {
                    st.selection.clear();
                    st.lights.clear();
                    changed = true;
                }
                None
            }
            MidiAction::Env(env) => {
                let (k, v) = match env {
                    EnvChange::Haze(v) => ("haze", v),
                    EnvChange::Exposure(v) => ("exposure", v),
                    EnvChange::Ambient(v) => ("ambient", v),
                };
                let mut m = Map::new();
                m.insert(k.into(), json!(v));
                Some(Edit::Environment(m))
            }
            MidiAction::Command(id) => {
                commands.push(id);
                None
            }
            MidiAction::SettingsChanged => {
                save = true;
                changed = true;
                None
            }
        };
        if let Some(e) = e {
            st.doc.apply(e);
            changed = true;
        }
    }
    if changed {
        refresh(app, st);
    }
    (commands, save)
}

/// Étiquettes des projecteurs : nom, numéro et adresse au-dessus de chacun, à leur place à l'écran.
fn show_labels(app: &AppWindow, st: &State) {
    let (w, h) = st.view;
    if w < 2.0 || h < 2.0 {
        return;
    }
    let vp = st.camera.view_proj(w / h);
    let lib = library();
    let labels: Vec<Label3D> = st
        .scene
        .picks
        .iter()
        .filter(|p| !p.object)
        .filter_map(|p| {
            let f = st.doc.fixture(&p.id)?;
            let top = Vec3::new((p.lo.x + p.hi.x) / 2.0, p.hi.y + 0.05, (p.lo.z + p.hi.z) / 2.0);
            let c = vp * top.extend(1.0);
            if c.w <= 0.01 {
                return None;
            }
            let (x, y) = ((c.x / c.w * 0.5 + 0.5) * w, (0.5 - c.y / c.w * 0.5) * h);
            if !(0.0..=w).contains(&x) || !(0.0..=h).contains(&y) {
                return None;
            }
            // « Fix 3 · DMX U1 · 040–047 » (`patchLine`)
            let mut sub = Vec::new();
            if let Some(n) = f.settings.fixture_number {
                sub.push(format!("Fix {n}"));
            }
            if let Some(a) = f.settings.dmx_address {
                let footprint = lib.fixture(&f.type_id).and_then(|t| t.mode(f.settings.dmx_mode.as_deref())).map_or(1, |m| m.footprint()) as u32;
                sub.push(format!("DMX {}", actions::fmt_range(f.settings.universe.unwrap_or(1), a as u32, footprint)));
            }
            let off = f.state.get("power") == Some(&Value::Bool(false));
            Some(Label3D { x, y, name: f.name.as_str().into(), sub: sub.join(" · ").into(), selected: st.selection.contains(&p.id), off })
        })
        .collect();
    let model = app.global::<App>().get_labels();
    let same = model.row_count() == labels.len() && labels.iter().enumerate().all(|(i, l)| model.row_data(i).is_some_and(|m| m == *l));
    if !same {
        app.global::<App>().set_labels(ModelRc::new(VecModel::from(labels)));
    }
}

/// Touches et molettes de la face avant, mises à jour sur place quand la face garde sa forme (une
/// molette tournée à la souris n'est pas recréée en plein geste).
fn set_panel_rows(app: &AppWindow, rows: Vec<FaceRow>) {
    let model = app.global::<App>().get_panel_rows();
    let same = model.row_count() == rows.len()
        && rows.iter().enumerate().all(|(i, r)| {
            model.row_data(i).is_some_and(|o| {
                o.cells.row_count() == r.cells.row_count()
                    && (0..r.cells.row_count()).all(|j| match (o.cells.row_data(j), r.cells.row_data(j)) {
                        (Some(a), Some(b)) => a.col == b.col && a.row == b.row && a.items.row_count() == b.items.row_count(),
                        _ => false,
                    })
            })
        });
    if !same {
        app.global::<App>().set_panel_rows(ModelRc::new(VecModel::from(rows)));
        return;
    }
    for (i, r) in rows.iter().enumerate() {
        let Some(o) = model.row_data(i) else { continue };
        for j in 0..r.cells.row_count() {
            let (Some(oc), Some(nc)) = (o.cells.row_data(j), r.cells.row_data(j)) else { continue };
            for k in 0..nc.items.row_count() {
                let Some(item) = nc.items.row_data(k) else { continue };
                if oc.items.row_data(k).as_ref() != Some(&item) {
                    oc.items.set_row_data(k, item);
                }
            }
        }
    }
}

/// Face avant du projecteur choisi (écran, voyants, message).
fn show_panel(app: &AppWindow, st: &mut State) {
    st.panel_drawn = Instant::now();
    let Some(id) = onboard_panel::target(st) else {
        app.global::<App>().set_panel_title(SharedString::new());
        app.global::<App>().set_panel_rows(ModelRc::default());
        return;
    };
    let time = st.clock.elapsed().as_secs_f64();
    let scale = app.window().scale_factor();
    let s = &mut *st;
    let Some(f) = s.doc.fixture(&id) else { return };
    let env = arklight_onboard::Env { time, fixtures: &s.doc.project.fixtures, live: &s.live, app_connected: false };
    let rt = s.lights.runtime_mut(&id);
    let screen = s.onboard.display(f, rt, &env);
    let leds = s.onboard.leds(f, rt, &env);
    let hint = s.onboard.hint(f, rt, &env);
    let Some(panel) = s.onboard.peek(&id) else { return };
    let face = panel.face();
    app.global::<App>().set_panel_title(format!("{} — {}", f.name, model_name(&f.type_id)).into());
    match face.display.as_ref() {
        Some(spec) => {
            if let Some(img) = onboard_panel::screen_image(&screen, spec, scale) {
                app.global::<App>().set_panel_screen(img);
            }
            let z = spec.zoom.max(1.0) as f32;
            app.global::<App>().set_panel_screen_width(spec.w as f32 * z);
            app.global::<App>().set_panel_screen_height(spec.h as f32 * z);
        }
        None => {
            app.global::<App>().set_panel_screen_width(0.0);
            app.global::<App>().set_panel_screen_height(0.0);
        }
    }
    // claviers Astera : disposition gardée ; faces des marques : rangées coupées pour tenir
    let astera = matches!(face.style.as_str(), "tube" | "ax2" | "v5square" | "quik" | "dual" | "art7" | "bulb" | "ax3" | "ax1");
    let held = |key: &str| panel.held_for(key, time);
    let rows = onboard_panel::rows(&face.rows, &leds, time, &held, &s.knob_angles, !astera);
    app.global::<App>().set_panel_style(face.style.as_str().into());
    set_panel_rows(app, rows);
    app.global::<App>().set_panel_hint(i18n::t(&hint).into());
    app.global::<App>().set_panel_note(i18n::t(&face.note).into());
}

/// Clic sur le plan : choisir (Ctrl : ajouter), déplacer, se déplacer (clic droit / molette) ;
/// vrai : ouvrir le menu contextuel (clic droit sans glisser).
fn plot_pointer(app: &AppWindow, st: &mut State, kind: i32, button: i32, x: f32, y: f32, ctrl: bool, shift: bool) -> bool {
    let p = plot_view::to_world(&st.plot.view, x, y);
    // un clic sur le plan ferme la fenêtre « Fond et cartouche »
    if kind == 1 && app.global::<App>().get_plot_panel_open() {
        app.global::<App>().set_plot_panel_open(false);
    }
    if kind != 1 {
        st.plot.drag = None;
        if button == 0 {
            if plot_view::up(st, p) {
                refresh(app, st);
            }
            return false;
        }
        let Some((px, py)) = st.press.take().filter(|_| button == 2) else { return false };
        if (x - px).hypot(y - py) > 4.0 {
            return false;
        }
        if plot_view::context(st, p) {
            refresh(app, st);
        }
        return true;
    }
    if button == 0 {
        if plot_view::down(st, p, arklight_plot::Modifiers { ctrl, shift }) {
            refresh(app, st);
        }
        // calage de l'image de fond : après le second point, la distance réelle
        if plot_view::calibration_ready(st) {
            if matches!(st.prompt, Some(Prompt::Calibrate)) && !app.global::<App>().get_prompt_open() {
                // fenêtre de la distance fermée sans réponse : calage abandonné
                st.prompt = None;
                plot_view::cancel_calibration(st);
            } else {
                ask(app, st, Prompt::Calibrate, i18n::t(arklight_plot::CALIB_DISTANCE), tr("Distance (m)", "Distance (m)"), String::new(), "");
            }
        }
        refresh(app, st);
        return false;
    }
    // clic droit ou molette enfoncée : se déplacer dans le plan
    if button == 2 {
        st.press = Some((x, y));
    }
    st.plot.drag = Some(plot_view::PlotDrag::Pan { x, y });
    false
}

/// Plans dans l'ordre (id, « Séquence › Plan »), et le rang du plan ouvert.
fn plan_list(doc: &Document) -> (Vec<(String, String)>, usize) {
    let mut list = Vec::new();
    let mut current = 0;
    for s in doc.plan_info() {
        for p in s.plans {
            if p.current {
                current = list.len();
            }
            list.push((p.id, format!("{} › {}", s.name, p.name)));
        }
    }
    (list, current)
}

/// Univers proposés au moniteur : reçus et utilisés par le projet.
fn monitor_universes(st: &State) -> Vec<u32> {
    let mut u: Vec<u32> = st.dmx.keys().copied().chain(st.doc.project.fixtures.iter().map(|f| f.settings.universe.unwrap_or(1))).collect();
    u.sort_unstable();
    u.dedup();
    if u.is_empty() {
        u.push(1);
    }
    u
}

/// Couleur stable d'un projecteur (repère sur la grille du moniteur) : `hsl(h 65% 55%)`.
fn fixture_color(id: &str) -> slint::Color {
    let mut x: u32 = 0;
    for c in id.encode_utf16() {
        x = x.wrapping_mul(31).wrapping_add(c as u32);
    }
    let (h, s, l) = ((x % 360) as f32, 0.65f32, 0.55f32);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let xx = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, xx, 0.0),
        1 => (xx, c, 0.0),
        2 => (0.0, c, xx),
        3 => (0.0, xx, c),
        4 => (xx, 0.0, c),
        _ => (c, 0.0, xx),
    };
    let m = l - c / 2.0;
    let u8c = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    slint::Color::from_rgb_u8(u8c(r), u8c(g), u8c(b))
}

/// Moniteur DMX (`dmx-monitor.js`) : valeurs reçues, qui occupe quel canal (noms posés sur les
/// canaux, infobulles), conflits, canaux de la sélection ; l'univers suit la sélection.
fn show_monitor(app: &AppWindow, st: &mut State) {
    let lib = library();
    if !st.monitor_picked
        && let Some(u) = st.selection.iter().find_map(|id| st.doc.fixture(id)).map(|f| f.settings.universe.unwrap_or(1))
    {
        st.monitor_universe = u;
    }
    let u = st.monitor_universe;
    let data = st.dmx.get(&u);
    // propriétaires de chaque canal : (id, nom, canal du projecteur, nombre de canaux, fonction, début)
    let mut owners: Vec<Vec<(String, String, usize, usize, String, bool)>> = vec![Vec::new(); 512];
    let mut labels = Vec::new();
    for f in st.doc.project.fixtures.iter().filter(|f| f.settings.universe.unwrap_or(1) == u) {
        let Some(mode) = lib.fixture(&f.type_id).and_then(|t| t.mode(f.settings.dmx_mode.as_deref())) else { continue };
        let fp = mode.footprint();
        let a = f.settings.dmx_address.unwrap_or(1) as usize;
        if fp == 0 || a == 0 {
            continue;
        }
        let end = (a + fp - 1).min(512);
        // nom posé sur les canaux, une fois par rangée de la grille
        let mut ch = a;
        while ch <= end {
            let row_end = end.min(ch.div_ceil(32) * 32);
            labels.push(MonitorLabel { row: ((ch - 1) / 32) as i32, col: ((ch - 1) % 32) as i32, span: (row_end - ch + 1) as i32, text: f.name.as_str().into() });
            ch = row_end + 1;
        }
        let slots = &mode.compiled().slots;
        for ch in a..=end {
            let k = ch - a;
            let label = slots
                .iter()
                .find(|s| k >= s.offset && k < s.offset + s.bytes() as usize)
                .map(|s| {
                    let mut l = console_panel::short_label(s);
                    if s.bytes() == 2 {
                        l += if k == s.offset { tr(" (gros)", " (coarse)") } else { tr(" (fin)", " (fine)") };
                    }
                    l
                })
                .unwrap_or_default();
            owners[ch - 1].push((f.id.clone(), f.name.clone(), k + 1, fp, label, ch == a));
        }
    }
    let mine: HashSet<&String> = st.selection.iter().collect();
    let cells: Vec<MonitorCell> = (0..512)
        .map(|i| {
            let o = &owners[i];
            let conflict = o.len() > 1;
            let tip = if o.is_empty() {
                format!("{} {} — {}", tr("Canal", "Channel"), i + 1, tr("libre", "free"))
            } else {
                let lines: Vec<String> = o.iter().map(|x| format!("{} — {} {}/{}{}", x.1, tr("canal", "channel"), x.2, x.3, if x.4.is_empty() { String::new() } else { format!(" : {}", x.4) })).collect();
                format!("{} {}\n{}{}", tr("Canal", "Channel"), i + 1, lines.join("\n"), if conflict { tr("\n⚠ chevauchement", "\n⚠ overlap") } else { "" })
            };
            MonitorCell {
                value: data.map_or(0, |d| d[i] as i32),
                patched: !o.is_empty(),
                start: o.iter().any(|x| x.5),
                conflict,
                mine: o.iter().any(|x| mine.contains(&x.0)),
                color: o.first().map_or(slint::Color::default(), |x| fixture_color(&x.0)),
                owner: o.last().map_or(String::new(), |x| x.0.clone()).into(),
                tip: tip.into(),
            }
        })
        .collect();
    let g = app.global::<App>();
    // rangée du premier projecteur choisi dans cet univers
    let row = st.selection.iter().filter_map(|id| st.doc.fixture(id)).find(|f| f.settings.universe.unwrap_or(1) == u).map_or(0, |f| (f.settings.dmx_address.unwrap_or(1).max(1) as i32 - 1) / 32);
    g.set_monitor_row(row);
    g.set_monitor_cells(ModelRc::new(VecModel::from(cells)));
    g.set_monitor_labels(ModelRc::new(VecModel::from(labels)));
    // conflits de tout le patch, et combien dans cet univers
    let c = arklight_core::addressing::conflicts(&st.doc.project.fixtures);
    let n = c.ids.len();
    let in_u = c.ids.iter().filter(|id| st.doc.fixture(id).is_some_and(|f| f.settings.universe.unwrap_or(1) == u)).count();
    let pill = if n == 0 {
        String::new()
    } else {
        let here = if in_u < n { format!(" ({in_u} {} U{u})", tr("dans", "in")) } else { String::new() };
        format!("⚠ {n} {}{here} — {}", if n > 1 { tr("projecteurs en conflit", "fixtures in conflict") } else { tr("projecteur en conflit", "fixture in conflict") }, tr("corriger…", "fix…"))
    };
    g.set_monitor_conflicts(pill.into());
    let list = monitor_universes(st);
    g.set_monitor_universes(ModelRc::new(VecModel::from(list.iter().map(|x| SharedString::from(format!("U{x}{}", if st.live.contains(x) { " ●" } else { "" }))).collect::<Vec<_>>())));
    g.set_monitor_universe_index(list.iter().position(|x| *x == u).unwrap_or(0) as i32);
    // sources de cet univers
    let sources: Vec<String> = st
        .net
        .as_ref()
        .map(|n| n.status().sources.iter().filter(|x| x.universes.contains(&u)).map(|x| format!("{} ({})", if x.name.is_empty() { &x.address } else { &x.name }, if x.protocol == "artnet" { "Art-Net" } else { "sACN" })).collect())
        .unwrap_or_default();
    let title = if !sources.is_empty() {
        format!("{} {}", tr("Sources :", "Sources:"), sources.join(", "))
    } else if data.is_some() {
        tr("Dernier état reçu (plus de signal)", "Last received state (no signal)").to_owned()
    } else {
        tr("Aucune source active", "No active source").to_owned()
    };
    g.set_monitor_title(title.into());
}

/// État de la réception DMX dans la barre d'état.
fn show_network(app: &AppWindow, st: &mut State) {
    // journal : nouvelles erreurs, sources DMX qui arrivent ou s'en vont
    for e in ERRORS.with(|e| std::mem::take(&mut *e.borrow_mut())) {
        log(st, "error", &e);
        app.global::<App>().set_toast(e.as_str().into());
        st.toast_until = Some(Instant::now() + Duration::from_millis(6000));
    }
    if let Some(net) = &st.net {
        let s = net.status();
        let now: HashSet<String> = s.sources.iter().map(|x| format!("{}|{}", x.protocol, x.address)).collect();
        let mut lines = Vec::new();
        for x in s.sources.iter().filter(|x| !st.known_sources.contains(&format!("{}|{}", x.protocol, x.address))) {
            let us: Vec<String> = x.universes.iter().map(|u| u.to_string()).collect();
            lines.push((format!("{} : {} {}{} → U{}", if x.protocol == "artnet" { "Art-Net" } else { "sACN" }, tr("source", "source"), x.address, if x.name.is_empty() { String::new() } else { format!(" ({})", x.name) }, us.join(", ")), ""));
        }
        for k in st.known_sources.iter().filter(|k| !now.contains(*k)) {
            lines.push((format!("{} {}", tr("Source DMX perdue :", "DMX source lost:"), k.replace('|', " ")), "warn"));
        }
        st.known_sources = now;
        for (t, level) in lines {
            log(st, level, &t);
        }
        // onglet Réseau
        let g = app.global::<App>();
        let rows: Vec<NetSource> = s
            .sources
            .iter()
            .map(|x| NetSource {
                protocol: (if x.protocol == "artnet" { "Art-Net" } else { "sACN" }).into(),
                address: x.address.as_str().into(),
                name: x.name.as_str().into(),
                universes: x.universes.iter().map(|u| u.to_string()).collect::<Vec<_>>().join(", ").into(),
            })
            .collect();
        g.set_net_sources(ModelRc::new(VecModel::from(rows)));
        g.set_net_counters(format!("Art-Net : {} {}, {} {} · sACN : {} {}", s.artnet_packets, tr("paquets", "packets"), s.polls, tr("découvertes", "polls"), s.sacn_packets, tr("paquets", "packets")).into());
    }
    if std::mem::take(&mut st.log_changed) {
        let lines: Vec<LogLine> = st.log.iter().rev().map(|(t, x, l)| LogLine { time: t.as_str().into(), text: x.as_str().into(), level: (*l).into() }).collect();
        app.global::<App>().set_log_lines(ModelRc::new(VecModel::from(lines)));
    }
    let Some(net) = &st.net else {
        app.global::<App>().set_artnet_state(0);
        app.global::<App>().set_sacn_state(0);
        app.global::<App>().set_artnet_tip(tr("Réseau coupé", "Network off").into());
        app.global::<App>().set_sacn_tip(tr("Réseau coupé", "Network off").into());
        app.global::<App>().set_artnet_status(tr("Art-Net : coupé", "Art-Net: off").into());
        app.global::<App>().set_sacn_status(tr("sACN : coupé", "sACN: off").into());
        return;
    };
    let s = net.status();
    let show = |name: &str, r: &Option<Result<u16, String>>| match r {
        Some(Ok(port)) => (format!("{name} · {port}"), true),
        Some(Err(e)) => (format!("{name} · {}", e.chars().take(40).collect::<String>()), false),
        None => (format!("{name} …"), false),
    };
    let (a, a_on) = show("Art-Net", &s.artnet);
    let (c, c_on) = show("sACN", &s.sacn);
    // voyants : données reçues dans les 1,5 dernières secondes, à l'écoute, ou erreur
    let now = Instant::now();
    for (packets, seen) in [(s.artnet_packets, &mut st.artnet_seen), (s.sacn_packets, &mut st.sacn_seen)] {
        if packets != seen.0 {
            *seen = (packets, now);
        }
    }
    let pill = |r: &Option<Result<u16, String>>, seen: (u64, Instant)| -> (i32, String) {
        match r {
            Some(Err(e)) => (3, format!("{} {e}", tr("Erreur :", "Error:"))),
            Some(Ok(_)) if seen.0 > 0 && now.duration_since(seen.1) < Duration::from_millis(1500) => (2, tr("Données reçues", "Receiving data").into()),
            Some(Ok(_)) => (1, tr("En écoute, aucune donnée", "Listening, no data").into()),
            None => (0, String::new()),
        }
    };
    let g = app.global::<App>();
    let (sa, ta) = pill(&s.artnet, st.artnet_seen);
    let (sc, tc) = pill(&s.sacn, st.sacn_seen);
    if g.get_artnet_state() != sa || g.get_artnet_tip() != ta.as_str() {
        g.set_artnet_state(sa);
        g.set_artnet_tip(ta.into());
    }
    if g.get_sacn_state() != sc || g.get_sacn_tip() != tc.as_str() {
        g.set_sacn_state(sc);
        g.set_sacn_tip(tc.into());
    }
    app.global::<App>().set_artnet_status(a.into());
    app.global::<App>().set_artnet_on(a_on);
    app.global::<App>().set_sacn_status(c.into());
    app.global::<App>().set_sacn_on(c_on);
    let sources: Vec<String> = s
        .sources
        .iter()
        .map(|x| {
            let who = if x.name.is_empty() { x.address.clone() } else { format!("{} ({})", x.name, x.address) };
            let us: Vec<String> = x.universes.iter().map(|u| u.to_string()).collect();
            format!("{who} → U{}", us.join(", "))
        })
        .collect();
    app.global::<App>().set_dmx_sources(if sources.is_empty() { tr("aucune console ne pilote", "no console sending").into() } else { sources.join(" · ").into() });
}

/// Projet d'exemple (intégré au programme), nommé dans la langue de l'interface.
fn demo(id: &str) -> Option<Project> {
    let (text, name) = match id {
        "clip" => (DEMO_CLIP, tr("Exemple — clip", "Sample — music video")),
        _ => (DEMO_NIGHT, tr("Exemple — tournage de nuit", "Sample — night shoot")),
    };
    let mut p = Project::from_json(text).ok()?;
    p.name = name.into();
    Some(p)
}

/// Une erreur à montrer : message bref en bas de la vue et ligne du journal (pas de boîte Windows,
/// qui peut passer derrière la fenêtre et la bloquer).
fn error_box(_title: &str, message: &str) {
    ERRORS.with(|e| e.borrow_mut().push(message.to_owned()));
}

/// Enregistre le projet (dans son fichier, ou en demandant où) ; faux si annulé ou en échec.
fn save(st: &mut State, ask: bool) -> bool {
    let path = match (&st.doc.path, ask) {
        (Some(p), false) => p.clone(),
        _ => {
            let name = format!("{}.json", if st.doc.project.name.is_empty() { "Projet" } else { &st.doc.project.name });
            let dir = projects::projects_dir();
            let _ = std::fs::create_dir_all(&dir);
            let Some(p) = rfd::FileDialog::new().set_title("ARKLIGHT").add_filter("ARKLIGHT", &["json"]).set_directory(&dir).set_file_name(name).save_file() else { return false };
            p
        }
    };
    if let Some(t) = &st.thumb {
        st.doc.project.extra.insert("thumbnail".into(), json!(t));
    }
    st.doc.project.extra.insert("savedAt".into(), json!(format!("{}.000Z", arklight_plot::Stamp::now_utc().iso())));
    match st.doc.project.save(&path) {
        Ok(()) => {
            st.doc.mark_saved(path);
            true
        }
        Err(e) => {
            error_box("ARKLIGHT", &e.to_string());
            false
        }
    }
}

/// Avant de fermer le projet : propose d'enregistrer les modifications ; faux si l'on annule.
/// Ce qui attend la réponse à « Enregistrer les modifications ? » ou « Quitter ? ».
#[derive(Clone)]
enum Pending {
    /// installer la nouvelle version (installeur téléchargé) : ARKLIGHT se ferme
    Install(String),
    /// Fichier › Revenir à la version enregistrée
    Revert,
    /// supprimer un préréglage MIDI enregistré
    MidiDeletePreset(String),
    /// projet passé par un second lancement d'ARKLIGHT
    OpenPath(String),
    /// « Séquences et plans » : supprimer un plan, une séquence
    DeletePlan(String),
    DeleteSequence(String),
    /// MIDI : effacer toutes les correspondances apprises
    MidiClear,
    Quit,
    New,
    Open,
    Recent(i32),
    Demo(String),
    Card(i32),
}

/// Avant de quitter ou de remplacer le projet : vrai si on peut continuer tout de suite ; sinon la
/// question est posée dans la fenêtre d'ARKLIGHT (comme la version JavaScript) et l'action reprend
/// après la réponse. Quitter demande toujours (avec l'encart de soutien).
fn guard(app: &AppWindow, st: &mut State, what: Pending) -> bool {
    if std::mem::take(&mut st.skip_guard) {
        return true;
    }
    let quit = matches!(what, Pending::Quit);
    if !quit && !st.doc.dirty() {
        return true;
    }
    st.pending = Some(what);
    app.global::<App>().set_ask_confirm(false);
    app.global::<App>().set_ask_quit(quit);
    app.global::<App>().set_ask_dirty(st.doc.dirty());
    app.global::<App>().set_ask_open(true);
    false
}

/// Fenêtre « Séquences et plans » : séquences et vignettes des plans (le plan ouvert : sa vue
/// actuelle).
fn show_plans(app: &AppWindow, st: &State) {
    let seqs: Vec<SeqRow> = st
        .doc
        .plan_info()
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let plans: Vec<PlanCard> = s
                .plans
                .iter()
                .map(|p| {
                    let thumb = if p.current { st.thumb.clone().or_else(|| p.thumbnail.clone()) } else { p.thumbnail.clone() };
                    let image = thumb.as_deref().and_then(projects::thumb_image);
                    let sub = tr_owned(format!("{} proj. · {} obj.", p.fixtures, p.objects), format!("{} fixt. · {} obj.", p.fixtures, p.objects));
                    PlanCard { id: p.id.as_str().into(), name: p.name.as_str().into(), sub: sub.into(), has_thumb: image.is_some(), thumb: image.unwrap_or_default(), current: p.current }
                })
                .collect();
            SeqRow { id: s.id.into(), name: s.name.into(), index: i as i32, plans: ModelRc::new(VecModel::from(plans)) }
        })
        .collect();
    app.global::<App>().set_plan_seqs(ModelRc::new(VecModel::from(seqs)));
}

/// Simple confirmation dans la fenêtre d'ARKLIGHT (titre, texte, bouton d'action) ; l'action
/// reprend après la réponse (`on_ask_answer`).
fn confirm(app: &AppWindow, st: &mut State, what: Pending, title: &str, note: &str, ok: &str) {
    st.pending = Some(what);
    let g = app.global::<App>();
    g.set_ask_quit(false);
    g.set_ask_dirty(false);
    g.set_ask_confirm(true);
    g.set_ask_title(title.into());
    g.set_ask_note(note.into());
    g.set_ask_ok(ok.into());
    g.set_ask_open(true);
}

fn open_doc(app: &AppWindow, state: &Rc<RefCell<State>>, project: Project, path: Option<PathBuf>) {
    let mut st = state.borrow_mut();
    let what = format!("{} « {} »", tr("Projet ouvert :", "Project opened:"), project.name);
    log(&mut st, "", &what);
    st.doc = Document::new(project, path);
    st.selection.clear();
    st.lights.clear(); // autre projet : effets et lampes repartent de zéro
    st.onboard.clear();
    refresh(app, &mut st);
}

/// Copie des éléments choisis, décalée de 50 cm (adresses DMX suivantes).
fn duplicate_edit(st: &State) -> Option<Edit> {
    let mut edits = Vec::new();
    for id in &st.selection {
        if let Some(f) = st.doc.fixture(id) {
            let mut settings = match serde_json::to_value(&f.settings) {
                Ok(Value::Object(m)) => m,
                _ => Map::new(),
            };
            settings.remove("dmxAddress");
            settings.remove("fixtureNumber");
            edits.push(Edit::AddFixture { type_id: f.type_id.clone(), position: [f.position[0] + 0.5, f.position[1], f.position[2]], rotation: f.rotation, name: None, settings });
        } else if let Some(o) = st.doc.object(id) {
            let props = o.props.as_object().cloned().unwrap_or_default();
            edits.push(Edit::AddObject { type_id: o.type_id.clone(), position: [o.position[0] + 0.5, o.position[1], o.position[2]], rotation: o.rotation, name: None, props });
        }
    }
    (!edits.is_empty()).then_some(Edit::Batch(edits))
}

fn delete_edit(st: &State) -> Option<Edit> {
    let fixtures: Vec<String> = st.selection.iter().filter(|id| st.doc.fixture(id).is_some()).cloned().collect();
    let mut edits: Vec<Edit> = st.selection.iter().filter(|id| st.doc.object(id).is_some()).map(|id| Edit::RemoveObject(id.clone())).collect();
    if !fixtures.is_empty() {
        edits.insert(0, Edit::RemoveFixtures(fixtures));
    }
    (!edits.is_empty()).then_some(Edit::Batch(edits))
}

/// Positions de départ de la sélection (projecteurs et objets).
fn start_positions(st: &State) -> Vec<(String, [f64; 3])> {
    st.selection.iter().filter_map(|id| st.doc.fixture(id).map(|f| (id.clone(), f.position)).or_else(|| st.doc.object(id).map(|o| (id.clone(), o.position)))).collect()
}

/// Sélection décalée de `offset` depuis ses positions de départ ; aimant : le premier élément
/// tombe sur la grille de 10 cm (les autres le suivent, l'écart entre eux ne change pas).
fn offset_edit(st: &State, start: &[(String, [f64; 3])], offset: Vec3, snap: bool) -> Option<Edit> {
    let (_, p0) = start.first()?;
    let grid = |v: f64| (v * 10.0).round() / 10.0;
    let o = [offset.x as f64, offset.y as f64, offset.z as f64];
    let d: [f64; 3] = std::array::from_fn(|i| if snap && o[i] != 0.0 { grid(p0[i] + o[i]) - p0[i] } else { o[i] });
    let edits: Vec<Edit> = start
        .iter()
        .map(|(id, p)| {
            // dans les limites du plateau (40 m autour du centre)
            let pos = [(p[0] + d[0]).clamp(-40.0, 40.0), (p[1] + d[1]).clamp(0.0, 30.0), (p[2] + d[2]).clamp(-40.0, 40.0)];
            if st.doc.fixture(id).is_some() {
                Edit::UpdateFixture { id: id.clone(), changes: Changes::moved(pos, None) }
            } else {
                Edit::UpdateObject { id: id.clone(), changes: Changes::moved(pos, None) }
            }
        })
        .collect();
    (!edits.is_empty()).then_some(Edit::Batch(edits))
}

/// Déplacement au sol d'un pixel de souris (en x, en y) pour un point de la scène : à l'échelle de
/// ce point à l'écran, le long de la droite de l'écran et de la direction du regard au sol. Linéaire :
/// l'élément suit la souris sans s'envoler vers l'horizon quand le regard est rasant.
fn ground_basis(st: &State, point: Vec3) -> (Vec3, Vec3) {
    let eye = st.camera.eye();
    let to = point - eye;
    let mpp = 2.0 * to.length().max(0.3) * (st.camera.fov.to_radians() / 2.0).tan() / st.view.1.max(1.0);
    let forward = (st.camera.target - eye).normalize_or(Vec3::NEG_Z);
    let right = forward.cross(Vec3::Y).normalize_or(Vec3::X);
    let ahead = Vec3::new(forward.x, 0.0, forward.z).normalize_or(Vec3::NEG_Z);
    // regard rasant : un pixel couvre plus de sol, sans dépasser 2 fois l'échelle de face
    let sin = to.normalize_or(forward).y.abs().max(0.5);
    (right * mpp, -ahead * (mpp / sin))
}

/// Mètres par pixel à la profondeur d'un point (déplacement en hauteur).
fn meters_per_pixel(st: &State, point: Vec3) -> f32 {
    2.0 * (point - st.camera.eye()).length().max(0.3) * (st.camera.fov.to_radians() / 2.0).tan() / st.view.1.max(1.0)
}

/// Vue libre retrouvée (sortie de « Voir par la caméra »).
fn stop_look(app: &AppWindow, st: &mut State) {
    if let Some((_, saved)) = st.look.take() {
        st.camera = saved;
        st.dirty = true;
    }
    app.global::<App>().set_look_label(SharedString::new());
    app.window().request_redraw();
}

/// La vue suit l'objectif de la caméra regardée (`_followLens`) : position, visée, champ selon la
/// focale et le capteur ; cadre 16/9 dans la vue.
fn follow_lens(app: &AppWindow, st: &mut State) {
    let Some((id, _)) = st.look.clone() else { return };
    let (Some(lens), Some(o)) = (st.scene.lenses.iter().find(|l| l.id == id).cloned(), st.doc.object(&id)) else {
        stop_look(app, st);
        return;
    };
    // `cameraFov` : capteurs en millimètres (16/9)
    let (sw, sh) = match o.props["sensor"].as_str() {
        Some("ff") => (36.0f32, 20.25f32),
        Some("m43") => (17.3, 9.73),
        _ => (24.89, 14.0),
    };
    let focal = o.props["focal"].as_f64().unwrap_or(35.0).max(4.0) as f32;
    let (hfov, vfov) = (2.0 * (sw / (2.0 * focal)).atan(), 2.0 * (sh / (2.0 * focal)).atan());
    let aspect = sw / sh;
    let view = st.view.0 / st.view.1.max(1.0);
    let wide = view >= aspect;
    let fov = if wide { vfov } else { 2.0 * ((hfov / 2.0).tan() / view).atan() };
    let f = lens.forward.normalize_or(Vec3::Z);
    let cam = Camera { target: lens.position + f, yaw: (-f.x).atan2(-f.z), pitch: (-f.y).clamp(-1.0, 1.0).asin(), distance: 1.0, fov: fov.to_degrees() };
    if (cam.target - st.camera.target).length() > 1e-5 || (cam.fov - st.camera.fov).abs() > 1e-4 || (cam.yaw - st.camera.yaw).abs() > 1e-5 || (cam.pitch - st.camera.pitch).abs() > 1e-5 || st.camera.distance != 1.0 {
        st.camera = cam;
        st.dirty = true;
        app.window().request_redraw();
    }
    let g = app.global::<App>();
    g.set_look_w(if wide { aspect / view } else { 1.0 });
    g.set_look_h(if wide { 1.0 } else { view / aspect });
    let sensor = match o.props["sensor"].as_str() {
        Some("ff") => tr("Plein format", "Full frame"),
        Some("m43") => "Micro 4/3",
        _ => "Super 35",
    };
    let label = format!("{} · {} mm · {sensor} — {}", o.name, focal.round(), tr("Échap pour sortir", "Esc to leave"));
    if g.get_look_label() != label.as_str() {
        g.set_look_label(label.into());
    }
}

/// Barre d'axes de la sélection (outil Déplacer, vue 3D) : centre et bout de chaque axe à l'écran.
fn show_gizmo(app: &AppWindow, st: &State) {
    let g = app.global::<App>();
    let off = Gizmo::default();
    let (w, h) = st.view;
    let start = start_positions(st);
    let tool = g.get_tool();
    if st.plot.on || st.look.is_some() || tool > 1 || start.is_empty() || w < 2.0 || h < 2.0 {
        if g.get_gizmo().on {
            g.set_gizmo(off);
        }
        return;
    }
    let n = start.len() as f64;
    let c: [f64; 3] = std::array::from_fn(|i| start.iter().map(|(_, p)| p[i]).sum::<f64>() / n);
    let center = Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32);
    let vp = st.camera.view_proj(w / h);
    let project = |p: Vec3| {
        let v = vp * p.extend(1.0);
        (v.w > 0.05).then(|| ((v.x / v.w * 0.5 + 0.5) * w, (0.5 - v.y / v.w * 0.5) * h))
    };
    // longueur des axes : environ 80 pixels à l'écran
    let depth = (center - st.camera.eye()).length().max(0.5);
    let len = 80.0 * 2.0 * depth * (st.camera.fov.to_radians() / 2.0).tan() / h;
    let (Some(o), Some(x), Some(y), Some(z)) = (project(center), project(center + Vec3::X * len), project(center + Vec3::Y * len), project(center + Vec3::Z * len)) else {
        g.set_gizmo(off);
        return;
    };
    let mut giz = Gizmo { on: true, cx: o.0, cy: o.1, xx: x.0, xy: x.1, yx: y.0, yy: y.1, zx: z.0, zy: z.1, len, ..Gizmo::default() };
    if tool == 1 {
        // anneaux : moitié tournée vers l'œil en plein, l'autre estompée ; points de saisie tout autour
        giz.rotate = true;
        let eye = st.camera.eye();
        let mut dots = Vec::with_capacity(3 * RING_DOTS);
        let mut paths = Vec::new();
        for (name, axis) in [("rx", Vec3::X), ("ry", Vec3::Y), ("rz", Vec3::Z)] {
            let samples = ring_points(center, axis, len * 0.95, 96);
            let (mut front, mut back) = (String::new(), String::new());
            let mut prev: Option<bool> = None;
            for (i, p) in samples.iter().enumerate() {
                let Some(s) = project(*p) else {
                    prev = None;
                    continue;
                };
                let near = (*p - center).dot(eye - center) >= 0.0;
                // chaque moitié : un trait continu, repris là où l'anneau passe de l'une à l'autre
                for (path, want) in [(&mut front, true), (&mut back, false)] {
                    if near == want || prev == Some(want) {
                        let cmd = if prev == Some(want) { "L" } else { "M" };
                        path.push_str(&format!("{cmd} {:.1} {:.1} ", s.0, s.1));
                    }
                }
                prev = Some(near);
                if i % (96 / RING_DOTS) == 0 {
                    dots.push(GizmoDot { axis: name.into(), x: s.0, y: s.1 });
                }
            }
            paths.push((front, back));
        }
        let mut path = paths.into_iter();
        (giz.xf, giz.xb) = path.next().map(|(f, b)| (f.into(), b.into())).unwrap_or_default();
        (giz.yf, giz.yb) = path.next().map(|(f, b)| (f.into(), b.into())).unwrap_or_default();
        (giz.zf, giz.zb) = path.next().map(|(f, b)| (f.into(), b.into())).unwrap_or_default();
        if let Some(Drag::Ring { angle, .. }) = st.drag {
            giz.angle = format!("{}{:.0}°", if angle >= 0.0 { "+" } else { "" }, angle.to_degrees()).into();
        }
        // points de saisie mis à jour sur place : celui qu'on tient n'est pas recréé
        dots.resize(3 * RING_DOTS, GizmoDot { axis: SharedString::new(), x: -100.0, y: -100.0 });
        let model = g.get_gizmo_dots();
        if model.row_count() != dots.len() {
            g.set_gizmo_dots(ModelRc::new(VecModel::from(dots)));
        } else {
            for (i, d) in dots.into_iter().enumerate() {
                if model.row_data(i).as_ref() != Some(&d) {
                    model.set_row_data(i, d);
                }
            }
        }
    }
    if g.get_gizmo() != giz {
        g.set_gizmo(giz);
    }
}

/// Points d'un anneau de rayon `radius` autour de `axis`, centré sur `center`.
fn ring_points(center: Vec3, axis: Vec3, radius: f32, n: usize) -> Vec<Vec3> {
    let u = axis.any_orthonormal_vector();
    let v = axis.cross(u);
    (0..=n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            center + (u * a.cos() + v * a.sin()) * radius
        })
        .collect()
}

/// Sélection tournée de `angle` autour de `axis` (axe du monde passant par `center`) depuis ses
/// orientations et positions de départ.
fn ring_edit(st: &State, start: &[(String, [f64; 3], [f64; 3])], axis: Vec3, center: Vec3, angle: f64) -> Option<Edit> {
    use arklight_render::glam::{DQuat, DVec3, EulerRot};
    let q = DQuat::from_axis_angle(axis.as_dvec3(), angle);
    let c = center.as_dvec3();
    let r3 = |v: f64| (v * 1000.0).round() / 1000.0;
    let edits: Vec<Edit> = start
        .iter()
        .map(|(id, rot, pos)| {
            let q0 = DQuat::from_euler(EulerRot::XYZ, rot[0], rot[1], rot[2]);
            let (x, y, z) = (q * q0).to_euler(EulerRot::XYZ);
            // plusieurs éléments : ils tournent aussi autour du centre (un seul : il reste en place)
            let p = if start.len() > 1 { c + q * (DVec3::from_array(*pos) - c) } else { DVec3::from_array(*pos) };
            let p = [p.x.clamp(-40.0, 40.0), p.y.clamp(0.0, 30.0), p.z.clamp(-40.0, 40.0)];
            let changes = Changes::moved(p, Some([r3(x), r3(y), r3(z)]));
            if st.doc.fixture(id).is_some() { Edit::UpdateFixture { id: id.clone(), changes } } else { Edit::UpdateObject { id: id.clone(), changes } }
        })
        .collect();
    (!edits.is_empty()).then_some(Edit::Batch(edits))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // une seule fenêtre : un second lancement lui passe son projet et s'arrête (pas en essai)
    let instance = if std::env::var_os("ARKLIGHT_SMOKE").is_some() {
        None
    } else {
        match single::claim(std::env::args_os().nth(1).as_deref().map(std::path::Path::new)) {
            single::Claim::Forwarded => return Ok(()),
            single::Claim::Primary(l) => Some(l),
            single::Claim::Alone => None,
        }
    };
    // Slint dessine avec wgpu : la vue 3D utilise la même carte graphique (aucune copie d'image)
    // ; limites normales de wgpu (le moteur lit ses lumières dans un tampon de stockage), carte la
    // plus puissante (portables à deux cartes graphiques)
    let mut wgpu_settings = slint::wgpu_30::WGPUSettings::default();
    wgpu_settings.device_required_limits = slint::wgpu_30::wgpu::Limits::default();
    wgpu_settings.power_preference = slint::wgpu_30::wgpu::PowerPreference::HighPerformance;
    slint::BackendSelector::new().require_wgpu_30(slint::wgpu_30::WGPUConfiguration::Automatic(wgpu_settings)).select()?;

    let settings = Rc::new(RefCell::new(load_settings()));
    // assistant : AIODOS par défaut (une fois ; un autre moteur choisi ensuite reste choisi)
    if !settings.borrow().ai_aiodos_default {
        let mut s = settings.borrow_mut();
        s.ai.set_provider("aiodos");
        s.ai_aiodos_default = true;
        if std::env::var_os("ARKLIGHT_SMOKE").is_none() {
            save_settings(&s);
        }
    }
    if !settings.borrow().projects_dir.is_empty() {
        projects::set_projects_dir(Some(PathBuf::from(&settings.borrow().projects_dir)));
    }
    if settings.borrow().language.is_empty() {
        settings.borrow_mut().language = "auto".into();
    }

    let app = AppWindow::new()?;
    // (Slint ne change de langue qu'une fois la première fenêtre créée)
    apply_language(&settings.borrow().language);
    app.global::<App>().set_language(settings.borrow().language.clone().into());
    // essai d'une langue sans toucher aux réglages (tests automatiques)
    if let Ok(lang) = std::env::var("ARKLIGHT_LANG") {
        apply_language(&lang);
    }
    app.global::<App>().set_version(env!("CARGO_PKG_VERSION").into());
    app.global::<App>().set_prerelease(env!("CARGO_PKG_VERSION").contains('-'));
    show_library(&app, "", &settings.borrow().library);
    // panneaux comme à la dernière fermeture ; enregistrés quand ils changent
    {
        let p = settings.borrow().panels;
        let g = app.global::<App>();
        g.set_left_w(p.left.clamp(200.0, 520.0));
        g.set_right_w(p.right.clamp(280.0, 620.0));
        g.set_bottom_h(p.bottom.clamp(140.0, 900.0));
        g.set_show_left(p.show_left);
        g.set_show_right(p.show_right);
        g.set_show_console(p.show_bottom);
        g.set_show_help(p.help);
        g.set_show_fps(p.fps);
    }
    let panels_timer = slint::Timer::default();
    {
        let app_weak = app.as_weak();
        let settings = settings.clone();
        panels_timer.start(slint::TimerMode::Repeated, Duration::from_millis(1500), move || {
            let Some(app) = app_weak.upgrade() else { return };
            let now = panels_of(&app);
            if settings.borrow().panels != now && std::env::var_os("ARKLIGHT_SMOKE").is_none() {
                settings.borrow_mut().panels = now;
                save_settings(&settings.borrow());
            }
        });
    }

    let state = Rc::new(RefCell::new(State {
        doc: Document::new(Project::empty("Nouveau projet"), None),
        lights: Lights::default(),
        dmx: Universes::new(),
        live: HashSet::new(),
        clock: Instant::now(),
        scene: Scene::default(),
        camera: Camera::default(),
        selection: Vec::new(),
        bloom: true,
        choices: HashMap::new(),
        ctx_choices: HashMap::new(),
        ctx_key: String::new(),
        artnet_seen: (0, Instant::now()),
        sacn_seen: (0, Instant::now()),
        floor_cache: None,
        search: String::new(),
        monitor_picked: false,
        log: std::collections::VecDeque::new(),
        log_changed: true,
        known_sources: HashSet::new(),
        highlight: false,
        look: None,
        console_family: 0,
        console_raw: false,
        clipboard: Vec::new(),
        pastes: 0,
        knob_angles: HashMap::new(),
        pending: None,
        skip_guard: false,
        inspector_key: String::new(),
        inspector_drawn: Instant::now(),
        multi_address: None,
        scene_search: String::new(),
        scene_view: settings.borrow().scene.clone(),
        dirty: true,
        size: (0, 0),
        view: (1.0, 1.0),
        drag: None,
        // réception DMX ; pas pendant les essais automatiques (aucun port ouvert, pas d'avertissement du pare-feu)
        net: (std::env::var_os("ARKLIGHT_SMOKE").is_none() && std::env::var_os("ARKLIGHT_NO_NET").is_none()).then(|| Receiver::start(arklight_net::receiver::Config { artnet_offset: settings.borrow().artnet_offset, ..Default::default() })),
        net_shown: Instant::now(),
        autosaved: 0,
        smoke: std::env::var_os("ARKLIGHT_SMOKE").is_some(),
        monitor_universe: 1,
        press: None,
        click: None,
        toast_until: None,
        prompt: None,
        readdress: actions::Readdress::default(),
        identifying: HashSet::new(),
        blackout: false,
        console_mem: console_panel::Memory::default(),
        midi: Midi::from_settings(&settings.borrow().midi),
        midi_inputs: None,
        fader_marks: Vec::new(),
        fader_count: 0,
        prev_selection: Vec::new(),
        midi_select: false,
        midi_scan: Instant::now(),
        plot: plot_view::Plot::default(),
        onboard: arklight_onboard::Onboard::new(),
        onboard_dirty: false,
        panel_drawn: Instant::now(),
        ai: ai_panel::Ai::new(settings.borrow().ai.clone(), env!("CARGO_PKG_VERSION")),
        dictate: dictation::Dictate::default(),
        ble: ble_panel::new_hub(),
        projects: Vec::new(),
        thumb: None,
        thumb_rev: u64::MAX,
        thumb_at: Instant::now() - Duration::from_secs(10),
    }));
    ai_panel::install(state.clone(), app.as_weak());
    ai_panel::show(&app, &state.borrow());
    // contrôleurs MIDI branchés (pas pendant les essais automatiques)
    if !state.borrow().smoke {
        let mut inputs = MidiInputs::new();
        let devices = inputs.start();
        let mut st = state.borrow_mut();
        st.midi.set_inputs(devices);
        st.fader_marks = st.midi.fader_marks();
        st.midi_inputs = Some(inputs);
    }
    app.global::<App>().set_midi_kinds(ModelRc::new(VecModel::from((0..TARGET_KINDS.len()).map(|i| SharedString::from(midi_kind_label(i))).collect::<Vec<_>>())));
    app.global::<App>().set_ra_orders(ModelRc::new(VecModel::from(actions::order_labels().into_iter().map(SharedString::from).collect::<Vec<_>>())));
    app.global::<App>().set_ra_aligns(ModelRc::new(VecModel::from(actions::align_labels().into_iter().map(SharedString::from).collect::<Vec<_>>())));
    let show_recent = {
        let settings = settings.clone();
        let app_weak = app.as_weak();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let list: Vec<SharedString> = settings.borrow().recent.iter().map(|p| SharedString::from(p.as_str())).collect();
                app.global::<App>().set_recent(ModelRc::new(VecModel::from(list)));
            }
        }
    };
    show_recent();
    show_network(&app, &mut state.borrow_mut());

    // projet donné en argument ; sinon la session précédente (copie de travail) ; sinon l'exemple de nuit
    let first = std::env::args().nth(1).map(PathBuf::from);
    let had_project = first.is_some();
    let smoke = state.borrow().smoke;
    match first.as_ref().map(|p| Project::open(p)) {
        Some(Ok(p)) => open_doc(&app, &state, p, first),
        Some(Err(e)) => {
            error_box("ARKLIGHT", &e.to_string());
            open_doc(&app, &state, demo("night").unwrap_or_else(|| Project::empty("ARKLIGHT")), None);
        }
        None => match (!smoke).then(read_autosave).flatten() {
            Some((p, path, dirty)) => {
                open_doc(&app, &state, p, path.clone());
                if !dirty && let Some(path) = path {
                    state.borrow_mut().doc.mark_saved(path);
                }
                let mut st = state.borrow_mut();
                st.autosaved = st.doc.revision;
                refresh(&app, &mut st);
            }
            None => open_doc(&app, &state, demo("night").unwrap_or_else(|| Project::empty("ARKLIGHT")), None),
        },
    }

    // ---- vue 3D : moteur créé avec la carte graphique de Slint, image redessinée à la demande
    {
        let app_weak = app.as_weak();
        let state = state.clone();
        let mut renderer: Option<Renderer> = None;
        app.window().set_rendering_notifier(move |rendering, api| {
            let slint::GraphicsAPI::WGPU30 { device, queue, .. } = api else { return };
            match rendering {
                slint::RenderingState::RenderingSetup => {
                    renderer = Some(Renderer::new(device));
                    if let Some(app) = app_weak.upgrade() {
                        app.global::<App>().set_gpu(device.adapter_info().name.into());
                    }
                }
                slint::RenderingState::BeforeRendering => {
                    let (Some(r), Some(app)) = (renderer.as_mut(), app_weak.upgrade()) else { return };
                    let scale = app.window().scale_factor();
                    let (vw, vh) = (app.get_view_width(), app.get_view_height());
                    let size = ((vw * scale).round().max(1.0) as u32, (vh * scale).round().max(1.0) as u32);
                    let mut st = state.borrow_mut();
                    st.view = (vw, vh);
                    if app.global::<App>().get_narrow_bar() != (vw < 1130.0) {
                        app.global::<App>().set_narrow_bar(vw < 1130.0);
                    }
                    let ww = app.window().size().width as f32 / scale;
                    let level = if ww >= 1640.0 { 0 } else if ww >= 1420.0 { 1 } else { 2 };
                    if app.global::<App>().get_head_level() != level {
                        app.global::<App>().set_head_level(level);
                    }
                    if st.plot.on || (!st.dirty && st.size == size) {
                        return;
                    }
                    st.dirty = false;
                    // taille changée : les textures de la vue sont recréées (image non comptée)
                    let resized = st.size != size;
                    st.size = size;
                    let start = Instant::now();
                    let texture = r.render(device, queue, size, &st.scene, &st.camera);
                    if !resized && st.thumb_rev != st.doc.revision && st.thumb_at.elapsed() > Duration::from_secs(2) {
                        st.thumb_at = Instant::now();
                        st.thumb_rev = st.doc.revision;
                        if let Some(px) = arklight_render::read_rgba(device, queue, &texture) {
                            st.thumb = projects::thumbnail(&px, texture.width(), texture.height());
                        }
                    }
                    drop(st);
                    if let Ok(image) = slint::Image::try_from(texture) {
                        app.global::<App>().set_view(image);
                    }
                    if !resized {
                        // moyenne glissante du temps de préparation d'une image
                        let ms = start.elapsed().as_secs_f32() * 1000.0;
                        let prev = app.global::<App>().get_frame_ms();
                        app.global::<App>().set_frame_ms(if prev > 0.0 { prev * 0.9 + ms * 0.1 } else { ms });
                    }
                }
                slint::RenderingState::RenderingTeardown => renderer = None,
                _ => {}
            }
        })?;
    }

    // ---- moteur lumière : 60 fois par seconde ; la vue n'est redessinée que si la lumière change
    // (effets, stroboscopes, filament des lampes, montée en régime des HMI…)
    let light_timer = slint::Timer::default();
    {
        let app_weak = app.as_weak();
        let state = state.clone();
        let settings = settings.clone();
        light_timer.start(slint::TimerMode::Repeated, Duration::from_millis(16), move || {
            let mut st = state.borrow_mut();
            if st.toast_until.is_some_and(|t| Instant::now() > t) {
                st.toast_until = None;
                if let Some(app) = app_weak.upgrade() {
                    app.global::<App>().set_toast(SharedString::new());
                }
            }
            if st.net_shown.elapsed() > Duration::from_millis(500) {
                st.net_shown = Instant::now();
                if let Some(app) = app_weak.upgrade() {
                    show_network(&app, &mut st);
                    if app.global::<App>().get_show_console() && app.global::<App>().get_bottom_tab() == 1 {
                        show_monitor(&app, &mut st);
                    }
                }
                // copie de travail du projet (toutes les demi-secondes au plus)
                if !st.smoke && st.autosaved != st.doc.revision {
                    write_autosave(&st.doc);
                    st.autosaved = st.doc.revision;
                }
            }
            let Some(app) = app_weak.upgrade() else { return };
            // RDM : réglages changés par une appli, identification
            let events = st.net.as_ref().map(|n| n.take_rdm_events()).unwrap_or_default();
            if !events.is_empty() {
                let mut changed = false;
                for ev in events {
                    match ev {
                        RdmEvent::Set { fixture, settings, name, .. } => {
                            let changes = Changes { settings: (!settings.is_empty()).then_some(settings), name, ..Changes::default() };
                            st.doc.apply(Edit::UpdateFixture { id: fixture, changes });
                            changed = true;
                        }
                        RdmEvent::Identify { fixture, on } => {
                            if on {
                                let name = st.doc.fixture(&fixture).map(|f| f.name.clone()).unwrap_or_default();
                                st.identifying.insert(fixture);
                                toast(&app, &mut st, &format!("{} {name}", tr("Identification demandée par l’appli :", "Identification requested by the app:")));
                            } else {
                                st.identifying.remove(&fixture);
                            }
                        }
                        RdmEvent::Discovery { .. } => {}
                    }
                }
                if changed {
                    refresh(&app, &mut st);
                }
            }
            // MIDI : messages des contrôleurs, appareils branchés ou débranchés
            let mut commands = Vec::new();
            let rescan = st.midi_scan.elapsed() > Duration::from_secs(2);
            let polled = st.midi_inputs.as_mut().map(|inputs| {
                let msgs = inputs.poll();
                let devices = (rescan && inputs.refresh()).then(|| inputs.devices().to_vec());
                (msgs, devices)
            });
            if let Some((msgs, devices)) = polled {
                if rescan {
                    st.midi_scan = Instant::now();
                }
                if let Some(d) = devices {
                    st.midi.set_inputs(d);
                    show_midi(&app, &st);
                }
                if !msgs.is_empty() {
                    let order = fixture_order(&st.doc);
                    let plans = plan_ids(&st.doc.project);
                    let selection = st.selection.clone();
                    let fader_count = st.fader_count;
                    let s = &mut *st;
                    let ctx = MidiContext { fixture_order: Some(&order), selection: &selection, selected_id: selection.first().map(String::as_str), plans: &plans, fader_count, ..MidiContext::new(&s.doc.project) };
                    let mut actions = Vec::new();
                    for m in msgs {
                        actions.extend(s.midi.receive(&m.data, &m.device, &ctx));
                    }
                    let (cmds, save) = apply_midi(&app, &mut st, actions);
                    commands = cmds;
                    if save {
                        save_midi(&settings, &st);
                    }
                    if app.global::<App>().get_bottom_tab() == 2 {
                        show_midi(&app, &st);
                    }
                }
            }
            if !commands.is_empty() {
                drop(st);
                for c in commands {
                    commands::run(&app, &c);
                }
                st = state.borrow_mut();
            }
            let lit = st.update_lights();
            if std::mem::take(&mut st.onboard_dirty) {
                refresh(&app, &mut st);
            }
            if (app.global::<App>().get_show_labels() || app.global::<App>().get_show_patch()) && !st.plot.on {
                show_labels(&app, &st);
            }
            if st.look.is_some() {
                follow_lens(&app, &mut st);
            }
            show_gizmo(&app, &st);
            if st.inspector_drawn.elapsed() > Duration::from_millis(500) && st.selection.len() == 1 {
                show_inspector(&app, &mut st);
            }
            if app.global::<App>().get_show_right() && !app.global::<App>().get_full_view() && st.panel_drawn.elapsed() > Duration::from_millis(80) {
                show_panel(&app, &mut st);
            }
            if st.plot.on {
                let scale = app.window().scale_factor();
                let (w, h) = (app.get_view_width(), app.get_view_height());
                let size = ((w * scale).round() as u32, (h * scale).round() as u32);
                // la lumière (teinte des faisceaux) : 5 images par seconde au plus
                if w > 1.0 && (st.plot.dirty || st.plot.size != size || (lit && st.plot.drawn.elapsed() > Duration::from_millis(200))) {
                    st.view = (w, h);
                    if !st.plot.fitted {
                        plot_view::fit(&mut st);
                    }
                    if let Some(img) = plot_view::image(&mut st, w, h, scale) {
                        app.global::<App>().set_plot_image(img);
                    }
                }
            }
            if !lit {
                return;
            }
            st.rebuild_scene();
            // pastilles de la liste Scène
            let rows = app.global::<App>().get_scene_rows();
            for i in 0..rows.row_count() {
                let Some(mut row) = rows.row_data(i) else { continue };
                if row.kind != "fixture" {
                    continue;
                }
                let rgb = st.lights.get(row.id.as_str()).map_or([0.0; 3], |l| l.average());
                let (sw, lit) = (swatch(rgb), rgb.iter().any(|v| *v > 1e-4));
                if row.swatch != sw || row.lit != lit {
                    row.swatch = sw;
                    row.lit = lit;
                    rows.set_row_data(i, row);
                }
            }
            app.window().request_redraw();
        });
    }

    // une modification du projet, puis tout est remontré
    let edit = {
        let app_weak = app.as_weak();
        let state = state.clone();
        move |e: Option<Edit>, select_created: bool| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if let Some(e) = e {
                let created = st.doc.apply(e);
                if select_created && let Some(id) = created {
                    st.selection = vec![id];
                }
            }
            refresh(&app, &mut st);
        }
    };

    // ---- vue 3D : choisir, déplacer, caméra
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_view_pointer(move |kind, button, x, y, ctrl, shift| {
            let Some(app) = app_weak.upgrade() else { return false };
            let mut st = state.borrow_mut();
            if st.plot.on {
                return plot_pointer(&app, &mut st, kind, button, x, y, ctrl, shift);
            }
            if kind != 1 {
                st.drag = None;
                // clic gauche sans glisser : choisir l'élément sous la souris (Ctrl : ajouter / retirer),
                // ou tout désélectionner dans le vide ; s'il a glissé, la caméra a tourné
                if button == 0 {
                    let Some((px, py, hit, add)) = st.click.take() else { return false };
                    if (x - px).hypot(y - py) > 4.0 {
                        return false;
                    }
                    match hit {
                        Some(id) if add => {
                            if let Some(i) = st.selection.iter().position(|s| *s == id) {
                                st.selection.remove(i);
                            } else {
                                st.selection.push(id);
                            }
                        }
                        Some(id) => st.selection = vec![id],
                        None if !add => st.selection.clear(),
                        None => return false,
                    }
                    refresh(&app, &mut st);
                    return false;
                }
                // clic droit sans glisser : le projecteur ou l'objet sous la souris est choisi, puis le menu
                let Some((px, py)) = st.press.take().filter(|_| button == 2) else { return false };
                if (x - px).hypot(y - py) > 4.0 {
                    return false;
                }
                let (w, h) = st.view;
                let (origin, dir) = st.camera.ray(x, y, w, h);
                if let Some((hit, _)) = pick(&st.scene, origin, dir)
                    && !st.selection.contains(&hit.id)
                {
                    st.selection = vec![hit.id.clone()];
                    refresh(&app, &mut st);
                }
                context_menus(&app, &mut st);
                return true;
            }
            if button == 2 {
                st.press = Some((x, y));
            }
            if button == 0 {
                let (w, h) = st.view;
                let (origin, dir) = st.camera.ray(x, y, w, h);
                let hit = pick(&st.scene, origin, dir).map(|(p, t)| (p.id.clone(), t));
                // glisser un élément déjà choisi : le déplacer ou le tourner ; partout ailleurs, glisser
                // tourne la caméra et un clic sans glisser choisit (comme la version JavaScript)
                if let Some((id, t)) = hit.clone()
                    && !ctrl
                    && st.selection.contains(&id)
                {
                    let point = origin + dir * t;
                    st.click = Some((x, y, Some(id), false));
                    st.drag = if app.global::<App>().get_tool() == 1 {
                        // départ de la rotation : orientation de chaque élément choisi (objets : pan seul)
                        let start = st
                            .selection
                            .iter()
                            .filter_map(|id| st.doc.fixture(id).map(|f| (id.clone(), f.rotation, true)).or_else(|| st.doc.object(id).map(|o| (id.clone(), o.rotation, false))))
                            .collect();
                        Some(Drag::Rotate { x, y, start, free: shift || !app.global::<App>().get_snap() })
                    } else {
                        Some(Drag::Move { x, y, basis: ground_basis(&st, point), start: start_positions(&st), vertical: shift, snap: app.global::<App>().get_snap() })
                    };
                    return false;
                }
                st.click = Some((x, y, hit.map(|h| h.0), ctrl));
            }
            if st.look.is_none() {
                st.drag = Some(Drag::Camera { button, x, y });
            }
            false
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let edit = edit.clone();
        app.global::<App>().on_view_moved(move |x, y| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if st.plot.on {
                match st.plot.drag {
                    Some(plot_view::PlotDrag::Pan { x: lx, y: ly }) => {
                        let s = st.plot.view.s;
                        st.plot.view.cx -= (x - lx) as f64 / s;
                        st.plot.view.cz -= (y - ly) as f64 / s;
                        st.plot.drag = Some(plot_view::PlotDrag::Pan { x, y });
                        st.plot.dirty = true;
                    }
                    None => {
                        let p = plot_view::to_world(&st.plot.view, x, y);
                        if plot_view::moved(&mut st, p) {
                            refresh(&app, &mut st);
                        }
                    }
                }
                return;
            }
            match st.drag {
                Some(Drag::Camera { button, x: lx, y: ly }) => {
                    st.drag = Some(Drag::Camera { button, x, y });
                    app.global::<App>().set_view_preset(SharedString::new());
                    if button == 0 {
                        st.camera.orbit(x - lx, y - ly);
                    } else {
                        let h = app.get_view_height();
                        st.camera.pan(x - lx, y - ly, h);
                    }
                    st.dirty = true;
                    app.window().request_redraw();
                }
                Some(Drag::Rotate { x: x0, y: y0, ref start, free }) => {
                    // 0,5° par pixel ; par pas de 5° sauf avec Maj
                    let step = |v: f64| if free { v } else { (v / 5f64.to_radians()).round() * 5f64.to_radians() };
                    let pan = step(((x - x0) as f64 * 0.5).to_radians());
                    let tilt = step(((y - y0) as f64 * 0.5).to_radians());
                    let edits: Vec<Edit> = start
                        .iter()
                        .map(|(id, rot, fixture)| {
                            let changes = Changes { rotation: Some(pan_tilt(*rot, pan, if *fixture { tilt } else { 0.0 })), ..Changes::default() };
                            if *fixture { Edit::UpdateFixture { id: id.clone(), changes } } else { Edit::UpdateObject { id: id.clone(), changes } }
                        })
                        .collect();
                    drop(st);
                    if !edits.is_empty() {
                        edit(Some(Edit::Batch(edits)), false);
                    }
                }
                Some(Drag::Move { x: x0, y: y0, basis, ref start, vertical, snap }) => {
                    let offset = if vertical {
                        // en hauteur : à l'échelle du premier élément à l'écran
                        let p = start.first().map_or(Vec3::ZERO, |s| Vec3::new(s.1[0] as f32, s.1[1] as f32, s.1[2] as f32));
                        Vec3::new(0.0, (y0 - y) * meters_per_pixel(&st, p), 0.0)
                    } else {
                        basis.0 * (x - x0) + basis.1 * (y - y0)
                    };
                    let e = offset_edit(&st, start, offset, snap);
                    drop(st);
                    edit(e, false);
                }
                Some(Drag::Axis { .. } | Drag::Ring { .. }) => {}
                None => {}
            }
        });
    }
    {
        // barre d'axes : 1 appui, 0 glisser, 2 relâcher ; axe « x », « y », « z » ou « xz » (au sol)
        let state = state.clone();
        let app_weak = app.as_weak();
        let edit = edit.clone();
        app.global::<App>().on_gizmo_drag(move |phase, axis, x, y| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if let Some(ring) = axis.strip_prefix('r') {
                match phase {
                    1 => {
                        let g = app.global::<App>().get_gizmo();
                        let dir = match ring {
                            "x" => Vec3::X,
                            "y" => Vec3::Y,
                            _ => Vec3::Z,
                        };
                        let start: Vec<(String, [f64; 3], [f64; 3])> = st
                            .selection
                            .iter()
                            .filter_map(|id| st.doc.fixture(id).map(|f| (id.clone(), f.rotation, f.position)).or_else(|| st.doc.object(id).map(|o| (id.clone(), o.rotation, o.position))))
                            .collect();
                        if start.is_empty() {
                            return;
                        }
                        let n = start.len() as f64;
                        let c: [f64; 3] = std::array::from_fn(|i| start.iter().map(|s| s.2[i]).sum::<f64>() / n);
                        let center = Vec3::new(c[0] as f32, c[1] as f32, c[2] as f32);
                        let eye = st.camera.eye();
                        let facing = dir.dot((eye - center).normalize_or(Vec3::Z));
                        let turn = if facing.abs() > 0.25 {
                            RingTurn::Around { cx: g.cx, cy: g.cy, sign: facing.signum() }
                        } else {
                            // vu par la tranche : tangente au point de l'anneau le plus proche de la souris
                            let (w, h) = st.view;
                            let vp = st.camera.view_proj(w / h.max(1.0));
                            let project = |p: Vec3| {
                                let v = vp * p.extend(1.0);
                                (v.w > 0.05).then(|| ((v.x / v.w * 0.5 + 0.5) * w, (0.5 - v.y / v.w * 0.5) * h))
                            };
                            let radius = g.len * 0.95;
                            let grab = ring_points(center, dir, radius, 96)
                                .into_iter()
                                .filter_map(|p| project(p).map(|s| (p, (s.0 - x).hypot(s.1 - y))))
                                .min_by(|a, b| a.1.total_cmp(&b.1))
                                .map_or(center + dir.any_orthonormal_vector() * radius, |g| g.0);
                            let tangent = dir.cross(grab - center);
                            let eps = 0.01;
                            match (project(grab), project(grab + tangent * eps)) {
                                (Some(a), Some(b)) => RingTurn::Along { tx: (b.0 - a.0) / eps, ty: (b.1 - a.1) / eps },
                                _ => return,
                            }
                        };
                        let a0 = match turn {
                            RingTurn::Around { cx, cy, .. } => (cy - y).atan2(x - cx),
                            RingTurn::Along { .. } => 0.0,
                        };
                        st.drag = Some(Drag::Ring { axis: dir, center, turn, x, y, last: a0, acc: 0.0, start, snap: app.global::<App>().get_snap(), angle: 0.0 });
                    }
                    0 => {
                        let Some(Drag::Ring { axis: dir, center, turn, x: x0, y: y0, last, acc, ref start, snap, .. }) = st.drag else { return };
                        let (a, acc, raw) = match turn {
                            RingTurn::Around { cx, cy, sign } => {
                                if (x - cx).hypot(y - cy) < 6.0 {
                                    return;
                                }
                                let a = (cy - y).atan2(x - cx);
                                // pas à pas, ramené entre -180° et 180° : les tours s'additionnent
                                let mut d = a - last;
                                if d > std::f32::consts::PI {
                                    d -= std::f32::consts::TAU;
                                } else if d < -std::f32::consts::PI {
                                    d += std::f32::consts::TAU;
                                }
                                (a, acc + d, (acc + d) * sign)
                            }
                            RingTurn::Along { tx, ty } => {
                                let k2 = tx * tx + ty * ty;
                                if k2 < 1.0 {
                                    return;
                                }
                                (last, acc, ((x - x0) * tx + (y - y0) * ty) / k2)
                            }
                        };
                        let step = 5f64.to_radians();
                        let angle = if snap { (raw as f64 / step).round() * step } else { raw as f64 };
                        let e = ring_edit(&st, start, dir, center, angle);
                        if let Some(Drag::Ring { last: l, acc: c, angle: shown, .. }) = st.drag.as_mut() {
                            *l = a;
                            *c = acc;
                            *shown = angle;
                        }
                        drop(st);
                        edit(e, false);
                    }
                    _ => st.drag = None,
                }
                return;
            }
            match phase {
                1 => {
                    let g = app.global::<App>().get_gizmo();
                    let (dir, end) = match axis.as_str() {
                        "x" => (Vec3::X, (g.xx, g.xy)),
                        "y" => (Vec3::Y, (g.yx, g.yy)),
                        "z" => (Vec3::Z, (g.zx, g.zy)),
                        _ => (Vec3::ZERO, (g.cx, g.cy)),
                    };
                    let len = g.len.max(1e-4);
                    let pixels = ((end.0 - g.cx) / len, (end.1 - g.cy) / len);
                    let start = start_positions(&st);
                    let first = start.first().map_or(Vec3::ZERO, |s| Vec3::new(s.1[0] as f32, s.1[1] as f32, s.1[2] as f32));
                    let basis = ground_basis(&st, first);
                    st.drag = Some(Drag::Axis { axis: dir, pixels, x, y, basis, start, snap: app.global::<App>().get_snap() });
                }
                0 => {
                    let Some(Drag::Axis { axis: dir, pixels, x: x0, y: y0, basis, ref start, snap }) = st.drag else { return };
                    let offset = if dir == Vec3::ZERO {
                        basis.0 * (x - x0) + basis.1 * (y - y0)
                    } else {
                        // projection du déplacement de la souris sur l'axe à l'écran
                        let k2 = pixels.0 * pixels.0 + pixels.1 * pixels.1;
                        if k2 < 4.0 {
                            return;
                        }
                        dir * (((x - x0) * pixels.0 + (y - y0) * pixels.1) / k2)
                    };
                    let e = offset_edit(&st, start, offset, snap);
                    drop(st);
                    edit(e, false);
                }
                _ => st.drag = None,
            }
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_view_wheel(move |dy, x, y| {
            let mut st = state.borrow_mut();
            if st.plot.on {
                plot_view::wheel(&mut st, dy, x, y);
                return;
            }
            if st.look.is_some() {
                return;
            }
            st.camera.zoom(dy / 60.0);
            st.dirty = true;
            if let Some(app) = app_weak.upgrade() {
                app.window().request_redraw();
            }
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_reset_view(move || {
            let mut st = state.borrow_mut();
            if let Some(app) = app_weak.upgrade() {
                stop_look(&app, &mut st);
            }
            st.camera = Camera::default();
            st.dirty = true;
            app_weak.upgrade().inspect(|a| a.global::<App>().set_view_preset("persp".into()));
            if let Some(app) = app_weak.upgrade() {
                app.window().request_redraw();
            }
        });
    }

    // ---- ambiance et affichage
    {
        let edit = edit.clone();
        app.global::<App>().on_set_env(move |key, v| {
            let value = if key.as_str() == "exposure" { 2f64.powf(v as f64) } else { v as f64 };
            let mut m = Map::new();
            m.insert(key.to_string(), json!(value));
            edit(Some(Edit::Environment(m)), false);
        });
    }
    {
        let edit = edit.clone();
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_set_flag(move |key, on| {
            if key.as_str() == "highlight" {
                let Some(app) = app_weak.upgrade() else { return };
                let mut st = state.borrow_mut();
                st.highlight = on;
                app.global::<App>().set_highlight(on);
                st.rebuild_scene();
                app.window().request_redraw();
                return;
            }
            if key.as_str() == "bloom" {
                let Some(app) = app_weak.upgrade() else { return };
                let mut st = state.borrow_mut();
                st.bloom = on;
                refresh(&app, &mut st);
                return;
            }
            let mut m = Map::new();
            m.insert(key.to_string(), json!(on));
            edit(Some(Edit::Environment(m)), false);
        });
    }

    // ---- bibliothèque, liste Scène, inspecteur
    {
        let app_weak = app.as_weak();
        let state = state.clone();
        let settings = settings.clone();
        app.global::<App>().on_search_changed(move |q| {
            state.borrow_mut().search = q.to_string();
            if let Some(app) = app_weak.upgrade() {
                show_library(&app, &q, &settings.borrow().library);
            }
        });
    }
    {
        // onglet, tri, groupe replié ou déplié
        let app_weak = app.as_weak();
        let state = state.clone();
        let settings = settings.clone();
        app.global::<App>().on_lib_action(move |what, arg| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut s = settings.borrow_mut();
            match what.as_str() {
                "tab" => s.library.tab = arg.parse().unwrap_or(0).min(3),
                "sort" => s.library.sort = arg.parse().unwrap_or(0).min(3),
                "fold" => library_panel::toggle_fold(&mut s.library, &arg),
                _ => return,
            }
            save_settings(&s);
            show_library(&app, &state.borrow().search, &s.library);
        });
    }
    {
        let edit = edit.clone();
        let state = state.clone();
        app.global::<App>().on_add_item(move |kind, id| {
            let e = add_edit(&state.borrow(), &kind, &id, None);
            edit(Some(e), true);
        });
    }
    {
        // glissé de la bibliothèque et déposé dans la vue 3D : posé au sol sous la souris
        let state = state.clone();
        let edit = edit.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_drop_item(move |kind, id, x, y| {
            let Some(app) = app_weak.upgrade() else { return };
            let (vx, vy) = (x - app.get_view_x(), y - app.get_view_y());
            let at = {
                let st = state.borrow();
                let (w, h) = st.view;
                if st.plot.on || vx < 0.0 || vy < 0.0 || vx > w || vy > h {
                    return;
                }
                let (origin, dir) = st.camera.ray(vx, vy, w, h);
                if dir.y >= -1e-4 {
                    return;
                }
                let t = -origin.y / dir.y;
                let p = origin + dir * t;
                Vec3::new(p.x.clamp(-30.0, 30.0), 0.0, p.z.clamp(-30.0, 30.0))
            };
            let e = add_edit(&state.borrow(), &kind, &id, Some(at));
            edit(Some(e), true);
        });
    }
    {
        // liste Scène : recherche, tri, objets repliés
        let state = state.clone();
        let settings = settings.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_scene_action(move |what, arg| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            match what.as_str() {
                "search" => st.scene_search = arg.to_string(),
                "sort" => st.scene_view.sort = arg.parse().unwrap_or(0).min(4),
                "fold" => st.scene_view.objects_folded = !st.scene_view.objects_folded,
                _ => return,
            }
            if what.as_str() != "search" {
                let mut s = settings.borrow_mut();
                s.scene = st.scene_view.clone();
                save_settings(&s);
            }
            show_scene(&app, &st);
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_select_row(move |id, add| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let id = id.to_string();
            if add {
                if let Some(i) = st.selection.iter().position(|s| *s == id) {
                    st.selection.remove(i);
                } else {
                    st.selection.push(id);
                }
            } else {
                st.selection = vec![id];
            }
            refresh(&app, &mut st);
        });
    }
    let field = {
        let state = state.clone();
        let edit = edit.clone();
        move |key: SharedString, input: inspector::Input| {
            // une tirette par canal : comme les faders de la console
            if let (Some(i), inspector::Input::Value(v)) = (key.strip_prefix("ch.").and_then(|i| i.parse::<usize>().ok()), &input) {
                let e = {
                    let st = state.borrow();
                    console_panel::move_fader(&st.doc, &st.selection, &st.lights, &st.dmx, i, *v)
                };
                edit(e, false);
                return;
            }
            // univers et adresse de départ de « Adresser à la suite » : gardés pour le bouton
            if let ("m.universe" | "m.address", inspector::Input::Text(t)) = (key.as_str(), &input) {
                let mut st = state.borrow_mut();
                let sel = st.selection.join(",");
                let (u, a) = multi_start(&st);
                let mut cur = st.multi_address.clone().filter(|m| m.0 == sel).map_or((u, a), |m| (m.1, m.2));
                let n = t.trim().parse::<f64>().ok().map(|n| n.round().max(1.0) as u32);
                if key.as_str() == "m.universe" {
                    cur.0 = n.unwrap_or(cur.0).min(63999);
                } else {
                    cur.1 = n.unwrap_or(cur.1).min(512);
                }
                st.multi_address = Some((sel, cur.0, cur.1));
                return;
            }
            let e = {
                let st = state.borrow();
                inspector::apply(&st.doc, &st.selection, &st.choices, &key, input)
            };
            edit(e, false);
        }
    };
    {
        let field = field.clone();
        app.global::<App>().on_field_text(move |k, t| field(k, inspector::Input::Text(t.to_string())));
    }
    {
        let field = field.clone();
        app.global::<App>().on_field_value(move |k, v| field(k, inspector::Input::Value(v)));
    }
    {
        let field = field.clone();
        app.global::<App>().on_field_index(move |k, i| field(k, inspector::Input::Index(i.max(0) as usize)));
    }
    app.global::<App>().on_field_toggle(move |k, on| field(k, inspector::Input::Toggle(on)));

    // ---- sélection et groupes (menus « Sélection », « Patch », clic droit, liste des groupes)
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let edit = edit.clone();
        app.global::<App>().on_action(move |what| {
            let Some(app) = app_weak.upgrade() else { return };
            let (name, arg) = what.split_once(':').map_or((what.as_str(), ""), |(a, b)| (a, b));
            let mut st = state.borrow_mut();
            let e = match name {
                // plan : fenêtre « Fond et cartouche » (passe sur le plan si besoin)
                "plot-panel" => {
                    let g = app.global::<App>();
                    if !st.plot.on {
                        st.plot.on = true;
                        g.set_plot_panel_open(true);
                        refresh(&app, &mut st);
                    } else {
                        g.set_plot_panel_open(!g.get_plot_panel_open());
                    }
                    None
                }
                // plein écran (F11) : toute la fenêtre ; Échap pour revenir
                "fullscreen" => {
                    let on = !app.window().is_fullscreen();
                    app.window().set_fullscreen(on);
                    app.global::<App>().set_fullscreen(on);
                    if on {
                        toast(&app, &mut st, tr("Plein écran — F11 ou Échap pour revenir", "Full screen — F11 or Esc to go back"));
                    }
                    None
                }
                // Fichier › Revenir à la version enregistrée (comme `revertScene`)
                "revert" => {
                    if st.doc.path.is_none() {
                        toast(&app, &mut st, tr("Ce projet n’a pas encore été enregistré", "This project hasn’t been saved yet"));
                    } else if !st.doc.dirty() {
                        toast(&app, &mut st, tr("Aucune modification depuis le dernier enregistrement", "No change since the last save"));
                    } else {
                        confirm(
                            &app,
                            &mut st,
                            Pending::Revert,
                            tr("Revenir à la version enregistrée ?", "Revert to the saved version?"),
                            tr("Les modifications faites depuis le dernier enregistrement seront perdues.", "Changes made since the last save will be lost."),
                            tr("Revenir", "Revert"),
                        );
                    }
                    None
                }
                // voir par la caméra : celle choisie, sinon la première de la scène ; encore : vue libre
                "camera" => {
                    if st.look.is_some() && arg.is_empty() {
                        stop_look(&app, &mut st);
                    } else {
                        let target = if !arg.is_empty() {
                            Some(arg.to_owned())
                        } else {
                            st.selection.iter().find(|id| st.scene.lenses.iter().any(|l| l.id == **id)).cloned().or_else(|| st.scene.lenses.first().map(|l| l.id.clone()))
                        };
                        match target {
                            Some(id) => {
                                if st.plot.on {
                                    st.plot.on = false;
                                    app.global::<App>().set_plot_mode(false);
                                }
                                let saved = st.look.take().map_or(st.camera, |l| l.1);
                                st.look = Some((id, saved));
                                follow_lens(&app, &mut st);
                            }
                            None => toast(&app, &mut st, tr("Ajoute d’abord une caméra : bibliothèque › Décor › Plateau", "Add a camera first: library › Set › Film set")),
                        }
                    }
                    None
                }
                "view" => {
                    // vue choisie : la vue par la caméra s'arrête d'elle-même
                    stop_look(&app, &mut st);
                    st.camera = Camera::preset(arg);
                    st.dirty = true;
                    app.global::<App>().set_view_preset(arg.into());
                    app.window().request_redraw();
                    None
                }
                "rename" => Some(Edit::Rename(arg.to_owned())),
                "frame" => {
                    if actions::frame(&mut st) {
                        app.window().request_redraw();
                    }
                    None
                }
                "aim" => actions::aim(&st, arg.parse().ok()),
                "drop" => actions::drop_to_floor(&st),
                // choix d'un menu du clic droit (pied, accessoires, toile, pieds, pose)
                "set" => {
                    let (key, i) = arg.rsplit_once(':').unwrap_or((arg, "0"));
                    inspector::apply(&st.doc, &st.selection, &st.ctx_choices, key, inspector::Input::Index(i.parse().unwrap_or(0)))
                }
                // objets : un quart de tour autour de la verticale
                "rot90" => {
                    let edits: Vec<Edit> = st
                        .selection
                        .iter()
                        .filter_map(|id| st.doc.object(id))
                        .map(|o| {
                            let y = ((o.rotation[1] + std::f64::consts::FRAC_PI_2) * 1000.0).round() / 1000.0;
                            Edit::UpdateObject { id: o.id.clone(), changes: Changes { rotation: Some([o.rotation[0], y, o.rotation[2]]), ..Changes::default() } }
                        })
                        .collect();
                    (!edits.is_empty()).then_some(Edit::Batch(edits))
                }
                "power" => {
                    let ids = st.selection.clone();
                    actions::toggle_power(&st, &ids)
                }
                "crmx-link" | "crmx-unlink" => {
                    let ids = st.selection.clone();
                    actions::set_crmx(&st, &ids, name == "crmx-link")
                }
                "group" => {
                    let e = actions::group_selection(&st);
                    if e.is_none() {
                        toast(&app, &mut st, tr("Sélectionne d’abord des projecteurs (Ctrl+clic pour en prendre plusieurs)", "Select fixtures first (Ctrl+click to pick several)"));
                    }
                    e
                }
                "blackout" => {
                    st.blackout = !st.blackout;
                    refresh(&app, &mut st);
                    None
                }
                "shortcuts" => {
                    drop(st);
                    app.global::<App>().invoke_shortcut("open".into(), SharedString::new());
                    return;
                }
                "toggle-plot" => {
                    st.plot.on = !st.plot.on;
                    st.dirty = true; // la vue 3D est redessinée au retour
                    refresh(&app, &mut st);
                    None
                }
                "plot-fit" => {
                    plot_view::fit(&mut st);
                    None
                }
                "group-add" => actions::change_group(&st, arg, true),
                "group-remove" => actions::change_group(&st, arg, false),
                "group-delete" => Some(Edit::DeleteGroup(arg.into())),
                "group-crmx-link" | "group-crmx-unlink" => {
                    let ids = st.doc.project.groups.iter().find(|g| g.id == arg).map(|g| g.ids.clone()).unwrap_or_default();
                    let e = actions::set_crmx(&st, &ids, name == "group-crmx-link");
                    if e.is_none() {
                        toast(&app, &mut st, tr("Aucun projecteur Astera V5 (CRMX) dans ce groupe", "No Astera V5 (CRMX) fixture in this group"));
                    }
                    e
                }
                "group-power" => {
                    let ids = st.doc.project.groups.iter().find(|g| g.id == arg).map(|g| g.ids.clone()).unwrap_or_default();
                    actions::toggle_power(&st, &ids)
                }
                "group-select" | "group-select-add" => {
                    actions::select_group(&mut st, arg, name == "group-select-add");
                    refresh(&app, &mut st);
                    None
                }
                "group-rename" => {
                    let name = st.doc.project.groups.iter().find(|g| g.id == arg).map(|g| g.name.clone()).unwrap_or_default();
                    ask(&app, &mut st, Prompt::RenameGroup(arg.into()), tr("Renommer le groupe", "Rename the group").into(), tr("Nom", "Name"), name, "");
                    None
                }
                "select-same" => {
                    actions::select_same_type(&mut st);
                    refresh(&app, &mut st);
                    None
                }
                // presse-papiers (comme la version JavaScript) : chaque collage décale de 30 cm et
                // reçoit une adresse DMX libre
                "copy" | "cut" => {
                    let copied = duplicate_edit(&st).map(|e| match e {
                        Edit::Batch(list) => list,
                        e => vec![e],
                    });
                    match copied {
                        Some(list) => {
                            let n = list.len();
                            // duplicate_edit décale de 50 cm : on garde les positions d'origine
                            st.clipboard = list
                                .into_iter()
                                .map(|e| match e {
                                    Edit::AddFixture { type_id, position, rotation, name, settings } => Edit::AddFixture { type_id, position: [position[0] - 0.5, position[1], position[2]], rotation, name, settings },
                                    Edit::AddObject { type_id, position, rotation, name, props } => Edit::AddObject { type_id, position: [position[0] - 0.5, position[1], position[2]], rotation, name, props },
                                    e => e,
                                })
                                .collect();
                            st.pastes = 0;
                            let msg = if name == "cut" { tr("coupé(s)", "cut") } else { tr("copié(s)", "copied") };
                            toast(&app, &mut st, &format!("{n} {} {msg}", tr("élément(s)", "item(s)")));
                            if name == "cut" { delete_edit(&st) } else { None }
                        }
                        None => {
                            toast(&app, &mut st, tr("Rien à copier : sélectionne des projecteurs ou des objets", "Nothing to copy: select fixtures or objects"));
                            None
                        }
                    }
                }
                "paste" => {
                    if st.clipboard.is_empty() {
                        toast(&app, &mut st, tr("Le presse-papiers est vide : copie d’abord des projecteurs (Ctrl+C)", "The clipboard is empty: copy fixtures first (Ctrl+C)"));
                        None
                    } else {
                        st.pastes += 1;
                        let dx = 0.3 * st.pastes as f64;
                        let shift = |p: [f64; 3]| [((p[0] + dx) * 1000.0).round() / 1000.0, p[1], p[2]];
                        let list = st
                            .clipboard
                            .iter()
                            .map(|e| match e.clone() {
                                Edit::AddFixture { type_id, position, rotation, name, settings } => Edit::AddFixture { type_id, position: shift(position), rotation, name, settings },
                                Edit::AddObject { type_id, position, rotation, name, props } => Edit::AddObject { type_id, position: shift(position), rotation, name, props },
                                e => e,
                            })
                            .collect();
                        Some(Edit::Batch(list))
                    }
                }
                "invert" => {
                    actions::invert_selection(&mut st);
                    refresh(&app, &mut st);
                    None
                }
                "number" => {
                    let list = actions::selected_fixtures(&st);
                    if let Some(first) = list.first() {
                        let many = list.len() > 1;
                        let value = first.settings.fixture_number.unwrap_or_else(|| st.doc.next_fixture_number()).to_string();
                        let title = if many { format!("{} {} {}", tr("Numéroter", "Number"), list.len(), tr("projecteurs à la suite", "fixtures in sequence")) } else { format!("{} — {}", tr("Numéro de fixture", "Fixture number"), first.name) };
                        let label = if many { tr("Premier numéro (Fix)", "First number (Fix)") } else { tr("Numéro (Fix)", "Number (Fix)") };
                        let note = tr(
                            "Le numéro de fixture identifie le projecteur dans la console ou l’appli (Luminair, Blackout…). Sur les Astera, c’est le réglage FIXTURE NUMBER.",
                            "The fixture number identifies the fixture in the console or app (Luminair, Blackout…). On Astera fixtures, it is the FIXTURE NUMBER setting.",
                        );
                        ask(&app, &mut st, Prompt::Number, title, label, value, note);
                    }
                    None
                }
                "readdress" | "readdress-selection" | "readdress-conflicts" | "group-readdress" => {
                    let preset = match name {
                        "readdress-selection" => Some("selection".to_owned()),
                        "readdress-conflicts" => Some("conflicts".to_owned()),
                        "group-readdress" => Some(format!("group:{arg}")),
                        _ => None,
                    };
                    let labels = actions::readdress_open(&mut st, preset.as_deref());
                    app.global::<App>().set_ra_scopes(ModelRc::new(VecModel::from(labels.into_iter().map(SharedString::from).collect::<Vec<_>>())));
                    show_readdress(&app, &st);
                    app.global::<App>().set_readdress_open(true);
                    None
                }
                _ => None,
            };
            drop(st);
            if e.is_some() {
                edit(e, false);
            }
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let edit = edit.clone();
        app.global::<App>().on_prompt_ok(move |value| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let e = match st.prompt.take() {
                Some(Prompt::Number) => match value.trim().parse::<f64>().ok().map(f64::round).filter(|n| (1.0..=9999.0).contains(n)) {
                    Some(n) => {
                        let (e, msg) = actions::number(&st, n as u32);
                        toast(&app, &mut st, &msg);
                        e
                    }
                    None => {
                        // la fenêtre reste ouverte
                        st.prompt = Some(Prompt::Number);
                        toast(&app, &mut st, tr("Numéro entre 1 et 9999", "Number between 1 and 9999"));
                        return;
                    }
                },
                Some(Prompt::Calibrate) => match value.trim().replace(',', ".").trim_end_matches('m').trim().parse::<f64>() {
                    Ok(m) if m > 0.0 => {
                        plot_view::apply_calibration(&mut st, m);
                        app.global::<App>().set_prompt_open(false);
                        refresh(&app, &mut st);
                        return;
                    }
                    _ => {
                        st.prompt = Some(Prompt::Calibrate);
                        toast(&app, &mut st, tr("Distance en mètres, par exemple 3,5", "Distance in metres, for example 3.5"));
                        return;
                    }
                },
                Some(Prompt::MidiPreset) => {
                    // enregistré par la fenêtre MIDI (après cette réponse)
                    let name = value.trim().to_owned();
                    if !name.is_empty() {
                        let w = app.as_weak();
                        slint::Timer::single_shot(Duration::ZERO, move || {
                            if let Some(a) = w.upgrade() {
                                a.global::<App>().invoke_midi("save-preset-now".into(), name.into());
                            }
                        });
                    }
                    None
                }
                Some(Prompt::RenamePlan(id)) => {
                    if st.doc.rename_plan(&id, value.trim()) {
                        refresh(&app, &mut st);
                        show_plans(&app, &st);
                    }
                    None
                }
                Some(Prompt::RenameGroup(id)) => {
                    let name = value.trim().to_owned();
                    (!name.is_empty()).then_some(Edit::UpdateGroup { id, name: Some(name), ids: None })
                }
                None => None,
            };
            app.global::<App>().set_prompt_open(false);
            drop(st);
            edit(e, false);
        });
    }
    let readdress = |scope_changed: bool| {
        let state = state.clone();
        let app_weak = app.as_weak();
        move || {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let r = &mut st.readdress;
            r.scope = app.global::<App>().get_ra_scope().max(0) as usize;
            r.order = arklight_core::addressing::Order::ALL.get(app.global::<App>().get_ra_order().max(0) as usize).copied().unwrap_or_default();
            r.pack = app.global::<App>().get_ra_pack() == 1;
            r.align = actions::ALIGNS.get(app.global::<App>().get_ra_align().max(0) as usize).copied().unwrap_or(0);
            r.overflow = app.global::<App>().get_ra_overflow();
            r.universe = app.global::<App>().get_ra_universe().trim().parse::<u32>().unwrap_or(1).clamp(1, 63999);
            r.address = app.global::<App>().get_ra_address().trim().parse::<u32>().unwrap_or(1).clamp(1, 512);
            if scope_changed {
                actions::readdress_scope_defaults(&mut st);
            }
            actions::readdress_update(&mut st);
            // les champs en cours de saisie gardent leur texte
            let (u, a) = (app.global::<App>().get_ra_universe(), app.global::<App>().get_ra_address());
            show_readdress(&app, &st);
            if !scope_changed {
                app.global::<App>().set_ra_universe(u);
                app.global::<App>().set_ra_address(a);
            }
        }
    };
    {
        let changed = readdress(true);
        app.global::<App>().on_readdress_scope(move |_| changed());
    }
    app.global::<App>().on_readdress_changed(readdress(false));
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let edit = edit.clone();
        app.global::<App>().on_readdress_apply(move || {
            let Some(app) = app_weak.upgrade() else { return };
            let (e, n) = {
                let st = state.borrow();
                (actions::readdress_edit(&st), st.readdress.plan.moves.len())
            };
            app.global::<App>().set_readdress_open(false);
            edit(e, false);
            if n > 0 {
                let msg = format!("{n} {}", tr("projecteur(s) ré-adressé(s)", "fixture(s) readdressed"));
                toast(&app, &mut state.borrow_mut(), &msg);
            }
        });
    }

    // ---- écran des projets
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let settings = settings.clone();
        let show_recent = show_recent.clone();
        app.global::<App>().on_projects(move |what, index| {
            let Some(app) = app_weak.upgrade() else { return };
            match what.as_str() {
                // Fichier › Dossier des projets… : les nouveaux projets y seront enregistrés
                "folder" => {
                    let Some(dir) = rfd::FileDialog::new().set_title(tr("Dossier des projets ARKLIGHT", "ARKLIGHT projects folder")).set_directory(projects::projects_dir()).pick_folder() else { return };
                    projects::set_projects_dir(Some(dir.clone()));
                    settings.borrow_mut().projects_dir = dir.to_string_lossy().to_string();
                    save_settings(&settings.borrow());
                    toast(&app, &mut state.borrow_mut(), &format!("{} {}", tr("Dossier des projets :", "Projects folder:"), dir.display()));
                    if app.global::<App>().get_projects_open() {
                        app.global::<App>().invoke_projects("open".into(), 0);
                    }
                }
                "open" => {
                    let dir = projects::projects_dir();
                    let list = projects::list(&dir);
                    app.global::<App>().set_project_rows(ModelRc::new(VecModel::from(projects::rows(&list))));
                    app.global::<App>().set_projects_folder(format!("{} {}", tr("Dossier des projets :", "Projects folder:"), dir.display()).into());
                    state.borrow_mut().projects = list;
                    app.global::<App>().set_projects_open(true);
                }
                "card" => {
                    let Some(path) = state.borrow().projects.get(index.max(0) as usize).map(|e| e.path.clone()) else { return };
                    if !guard(&app, &mut state.borrow_mut(), Pending::Card(index)) {
                        return;
                    }
                    match Project::open(&path) {
                        Ok(p) => {
                            open_doc(&app, &state, p, Some(path.clone()));
                            remember(&settings, &path);
                            show_recent();
                            app.global::<App>().set_projects_open(false);
                        }
                        Err(e) => error_box("ARKLIGHT", &format!("{}\n{}", path.display(), e)),
                    }
                }
                "new" => {
                    app.global::<App>().set_projects_open(false);
                    app.global::<App>().invoke_new_project();
                }
                "file" => {
                    app.global::<App>().set_projects_open(false);
                    app.global::<App>().invoke_open_project();
                }
                "demo-night" | "demo-clip" => {
                    app.global::<App>().set_projects_open(false);
                    app.global::<App>().invoke_open_demo(what.trim_start_matches("demo-").into());
                }
                _ => app.global::<App>().set_projects_open(false),
            }
        });
    }
    // au lancement sans projet donné : l'écran des projets, par-dessus la dernière session
    if !had_project && (!smoke || std::env::var("ARKLIGHT_SMOKE_ACTION").is_ok_and(|a| a.contains("projects"))) {
        app.global::<App>().invoke_projects("open".into(), 0);
    }

    // ---- raccourcis clavier
    {
        // bulles d'aide : « (W) » d'après les raccourcis choisis
        let settings = settings.clone();
        app.global::<App>().on_key_hint({
            let settings = settings.clone();
            move |id, _| shortcuts::keys_of(&settings.borrow().shortcuts, &id).first().map_or_else(SharedString::new, |k| format!(" ({})", shortcuts::display(k)).into())
        });
        app.global::<App>().on_menu_shortcut(move |id, _| shortcuts::keys_of(&settings.borrow().shortcuts, &id).first().map_or_else(SharedString::new, |k| format!("    {}", shortcuts::display(k)).into()));
    }
    let capture: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    // recherche (nom de la commande ou touche, sans tenir compte des accents ni des majuscules)
    let shortcut_filter: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    // clavier dessiné : modificateurs choisis (Ctrl, Alt, Maj) et touche choisie (filtre exact)
    let kb_state: Rc<RefCell<([bool; 3], Option<String>)>> = Rc::new(RefCell::new(([false; 3], None)));
    let show_shortcuts = {
        let settings = settings.clone();
        let capture = capture.clone();
        let filter = shortcut_filter.clone();
        let kb_state = kb_state.clone();
        let app_weak = app.as_weak();
        move || {
            let Some(app) = app_weak.upgrade() else { return };
            let s = settings.borrow();
            let f = scene_panel::norm(&filter.borrow());
            let ([ctrl, alt, shift], picked) = kb_state.borrow().clone();
            // clavier dessiné
            let mut keys = Vec::new();
            for (row, caps) in shortcuts::keyboard().into_iter().enumerate() {
                let mut x = 0.0f32;
                for c in caps {
                    if !c.gap {
                        let token = c.token.unwrap_or("");
                        let is_mod = token.starts_with("mod:");
                        let mod_on = match token {
                            "mod:Ctrl" => ctrl,
                            "mod:Alt" => alt,
                            "mod:Shift" => shift,
                            _ => false,
                        };
                        let combo = c.token.and_then(|t| shortcuts::visual_combo(t, ctrl, alt, shift));
                        let cmds: Vec<&commands::Command> = combo.as_deref().map_or_else(Vec::new, |k| commands::COMMANDS.iter().filter(|cm| shortcuts::keys_of(&s.shortcuts, cm.id).iter().any(|x| x == k)).collect());
                        let tip = if c.token.is_none() {
                            tr("Touche non attribuable", "Key not assignable").to_owned()
                        } else if is_mod {
                            tr_owned(format!("{} : clique pour voir les touches prises avec {}", c.cap, c.cap), format!("{0}: click to see the keys taken with {0}", shortcuts::cap_label(c.cap)))
                        } else if let Some(k) = &combo {
                            format!("{} : {}", shortcuts::display(k), if cmds.is_empty() { tr("libre", "free").to_owned() } else { cmds.iter().map(|cm| tr(cm.fr, cm.en)).collect::<Vec<_>>().join(", ") })
                        } else {
                            tr("Avec Maj, cette touche donne un autre caractère", "With Shift, this key gives another character").to_owned()
                        };
                        keys.push(KbKey {
                            row: row as i32,
                            x,
                            w: c.w,
                            cap: shortcuts::cap_label(c.cap).into(),
                            sub: c.sub.into(),
                            bound: !cmds.is_empty(),
                            action: cmds.first().map(|cm| tr(cm.fr, cm.en).trim_end_matches('…').to_owned()).unwrap_or_default().into(),
                            dead: c.token.is_none(),
                            is_mod,
                            mod_on,
                            picked: combo.is_some() && combo == picked,
                            token: (if is_mod { token.to_owned() } else { combo.clone().unwrap_or_default() }).into(),
                            tip: tip.into(),
                        });
                    }
                    x += c.w;
                }
            }
            app.global::<App>().set_kb_keys(ModelRc::new(VecModel::from(keys)));
            let rows: Vec<ShortcutRow> = commands::COMMANDS
                .iter()
                .filter(|c| match &picked {
                    Some(k) => shortcuts::keys_of(&s.shortcuts, c.id).iter().any(|x| x == k),
                    None => f.is_empty() || scene_panel::norm(tr(c.fr, c.en)).contains(&f) || shortcuts::keys_of(&s.shortcuts, c.id).iter().any(|k| scene_panel::norm(&shortcuts::display(k)).contains(&f)),
                })
                .map(|c| ShortcutRow {
                    id: c.id.into(),
                    label: tr(c.fr, c.en).into(),
                    keys: shortcuts::keys_of(&s.shortcuts, c.id).iter().map(|k| shortcuts::display(k)).collect::<Vec<_>>().join(", ").into(),
                    changed: s.shortcuts.contains_key(c.id),
                })
                .collect();
            app.global::<App>().set_shortcut_rows(ModelRc::new(VecModel::from(rows)));
            let cap = capture.borrow().as_deref().and_then(|id| commands::COMMANDS.iter().find(|c| c.id == id)).map(|c| tr(c.fr, c.en)).unwrap_or("");
            app.global::<App>().set_shortcut_capture(cap.into());
        }
    };
    {
        let settings = settings.clone();
        let capture = capture.clone();
        let show = show_shortcuts.clone();
        let app_weak = app.as_weak();
        let state = state.clone();
        app.global::<App>().on_key(move |text, ctrl, shift, alt| {
            let Some(app) = app_weak.upgrade() else { return false };
            let Some(key) = shortcuts::key_name(&text, ctrl, shift, alt) else { return false };
            let pending = capture.borrow_mut().take();
            if let Some(id) = pending {
                if key != "Escape" {
                    let lost = shortcuts::assign(&mut settings.borrow_mut().shortcuts, &id, &key);
                    save_settings(&settings.borrow());
                    app.global::<App>().set_keys_rev(app.global::<App>().get_keys_rev() + 1);
                    if let Some(lost) = lost {
                        let label = commands::COMMANDS.iter().find(|c| c.id == lost).map_or(lost, |c| tr(c.fr, c.en));
                        let msg = format!("{} {} : {label}", shortcuts::display(&key), tr("n’est plus la touche de", "is no longer the key of"));
                        toast(&app, &mut state.borrow_mut(), &msg);
                    }
                }
                show();
                return true;
            }
            if app.global::<App>().get_shortcuts_open() {
                if key == "Escape" {
                    app.global::<App>().set_shortcuts_open(false);
                    return true;
                }
                return false;
            }
            if key == "Escape" && state.borrow().look.is_some() {
                stop_look(&app, &mut state.borrow_mut());
                return true;
            }
            if key == "Escape" && app.window().is_fullscreen() {
                app.window().set_fullscreen(false);
                app.global::<App>().set_fullscreen(false);
                return true;
            }
            {
                let mut st = state.borrow_mut();
                if st.plot.on && plot_view::key(&mut st, &key) {
                    refresh(&app, &mut st);
                    return true;
                }
            }
            let cmd = shortcuts::command_for(&settings.borrow().shortcuts, &key);
            cmd.is_some_and(|c| commands::run(&app, c))
        });
    }
    {
        // relâchement : fin de la dictée maintenue (touche de « Parler à l'assistant »)
        let settings = settings.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_key_up(move |text, ctrl, shift, alt| {
            let Some(app) = app_weak.upgrade() else { return };
            let Some(key) = shortcuts::key_name(&text, ctrl, shift, alt) else { return };
            // (relâcher F8 après avoir lâché Ctrl… : la touche seule compte aussi)
            let base = key.rsplit('+').next().unwrap_or(&key).to_owned();
            let s = settings.borrow();
            if [key.as_str(), base.as_str()].iter().any(|k| shortcuts::command_for(&s.shortcuts, k) == Some("ai.voice")) {
                drop(s);
                app.global::<App>().invoke_ai("voice-up".into(), SharedString::new());
            }
        });
    }
    {
        let settings = settings.clone();
        let capture = capture.clone();
        let show = show_shortcuts.clone();
        let shortcut_filter = shortcut_filter.clone();
        let kb_state = kb_state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_shortcut(move |what, id| {
            let Some(app) = app_weak.upgrade() else { return };
            match what.as_str() {
                "open" => app.global::<App>().set_shortcuts_open(true),
                "capture" => *capture.borrow_mut() = Some(id.to_string()),
                "default" => shortcuts::reset(&mut settings.borrow_mut().shortcuts, &id),
                "clear" => {
                    settings.borrow_mut().shortcuts.insert(id.to_string(), Vec::new());
                }
                "close" => {
                    *capture.borrow_mut() = None;
                    shortcut_filter.borrow_mut().clear();
                    *kb_state.borrow_mut() = ([false; 3], None);
                    app.global::<App>().set_shortcuts_open(false);
                }
                "filter" => {
                    *shortcut_filter.borrow_mut() = id.to_string();
                    kb_state.borrow_mut().1 = None;
                    show();
                    return;
                }
                // clavier dessiné : modificateur (bascule) ou touche (filtre, encore : tout)
                "kb" => {
                    {
                        let mut k = kb_state.borrow_mut();
                        match id.as_str() {
                            "mod:Ctrl" => k.0[0] = !k.0[0],
                            "mod:Alt" => k.0[1] = !k.0[1],
                            "mod:Shift" => k.0[2] = !k.0[2],
                            "" => {}
                            combo => k.1 = if k.1.as_deref() == Some(combo) { None } else { Some(combo.to_owned()) },
                        }
                    }
                    show();
                    return;
                }
                _ => {}
            }
            save_settings(&settings.borrow());
            app.global::<App>().set_keys_rev(app.global::<App>().get_keys_rev() + 1);
            show();
        });
    }

    // ---- Bluetooth
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_ble(move |what, arg| {
            let Some(app) = app_weak.upgrade() else { return };
            ble_panel::action(&app, &mut state.borrow_mut(), &what, &arg);
        });
    }

    // ---- dictée ORAKLE dans les champs de texte (micro de chaque champ)
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<Dictation>().on_toggle(move |field| {
            let Some(app) = app_weak.upgrade() else { return };
            dictation::toggle(&app, &mut state.borrow_mut(), &field);
        });
    }
    {
        let keys = shortcuts::keys_of(&settings.borrow().shortcuts, "ai.voice");
        dictation::show_tooltip(&app, &keys.first().map(|k| shortcuts::display(k)).unwrap_or_else(|| "F8".into()));
    }

    // ---- projet passé par un second lancement (une seule fenêtre)
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let settings = settings.clone();
        let show_recent = show_recent.clone();
        app.global::<App>().on_open_path(move |path| {
            let Some(app) = app_weak.upgrade() else { return };
            // la fenêtre revient devant
            app.window().set_minimized(false);
            let _ = app.window().show();
            if path.is_empty() {
                return;
            }
            if !guard(&app, &mut state.borrow_mut(), Pending::OpenPath(path.to_string())) {
                return;
            }
            let path = PathBuf::from(path.as_str());
            match Project::open(&path) {
                Ok(p) => {
                    open_doc(&app, &state, p, Some(path.clone()));
                    remember(&settings, &path);
                    show_recent();
                }
                Err(e) => error_box("ARKLIGHT", &format!("{}\n{}", path.display(), e)),
            }
        });
    }
    if let Some(listener) = instance {
        let app_weak = app.as_weak();
        single::listen(listener, move |path| {
            let _ = app_weak.upgrade_in_event_loop(move |app| app.global::<App>().invoke_open_path(path.into()));
        });
    }

    // ---- liens (don, code source) et mises à jour
    app.global::<App>().on_open_url(|url| {
        // seulement nos propres liens (menu Aide, page de la nouvelle version)
        if url.starts_with("https://") {
            let _ = std::process::Command::new("explorer").arg(url.as_str()).spawn();
        }
    });
    // « Rechercher une mise à jour » (À propos), et une fois au lancement (bandeau seulement)
    let check_update = {
        let app_weak = app.as_weak();
        move |about: bool| {
            if about && let Some(app) = app_weak.upgrade() {
                app.global::<App>().set_about_status(tr("Vérification…", "Checking…").into());
            }
            let app_weak = app_weak.clone();
            std::thread::spawn(move || {
                let status = update::status(env!("CARGO_PKG_VERSION"));
                let _ = app_weak.upgrade_in_event_loop(move |app| {
                    let g = app.global::<App>();
                    let current = env!("CARGO_PKG_VERSION");
                    let text = match status {
                        update::Status::Available(version, url, exe) => {
                            let text = tr_owned(format!("Nouvelle version disponible : {version} (tu as {current})."), format!("New version available: {version} (you have {current})."));
                            g.set_update_url(url.into());
                            g.set_update_download(exe.into());
                            g.set_update_version(version.into());
                            text
                        }
                        update::Status::UpToDate => tr_owned(format!("Tu as la dernière version ({current})."), format!("You have the latest version ({current}).")),
                        update::Status::NoneYet => tr("Aucune version publiée pour l’instant.", "No version published yet.").to_owned(),
                        update::Status::Failed(e) => tr_owned(format!("Vérification impossible ({e})."), format!("Could not check ({e}).")),
                    };
                    if about {
                        g.set_about_status(text.into());
                    }
                });
            });
        }
    };
    {
        let check_update = check_update.clone();
        app.global::<App>().on_about_check(move || check_update(true));
    }
    {
        // bandeau de mise à jour : télécharger l'installeur (avancement), annuler, installer ;
        // « news » : la fenêtre des nouveautés
        let state = state.clone();
        let app_weak = app.as_weak();
        let cancel: Rc<RefCell<Option<arklight_ai::Cancel>>> = Rc::new(RefCell::new(None));
        app.global::<App>().on_update_action(move |what| {
            let Some(app) = app_weak.upgrade() else { return };
            let g = app.global::<App>();
            match what.as_str() {
                "news" => {
                    g.set_news_entries(news::entries());
                    g.set_news_index(0);
                    g.set_news_open(true);
                }
                "download" => {
                    let url = g.get_update_download().to_string();
                    if url.is_empty() {
                        // pas d'installeur joint à la version : sa page
                        g.invoke_open_url(g.get_update_url());
                        return;
                    }
                    let version = g.get_update_version().to_string();
                    let dest = std::env::temp_dir().join("ARKLIGHT").join(format!("ARKLIGHT-Setup-{version}.exe"));
                    let stop = arklight_ai::Cancel::new();
                    *cancel.borrow_mut() = Some(stop.clone());
                    g.set_update_progress(0.0);
                    let weak = app.as_weak();
                    std::thread::spawn(move || {
                        let last = std::cell::Cell::new(-1i32);
                        let weak2 = weak.clone();
                        let r = update::download(&url, &dest, &stop, |done, total| {
                            let k = total.map_or(0.0, |t| done as f32 / t.max(1) as f32);
                            let pct = (k * 100.0) as i32;
                            if pct != last.get() {
                                last.set(pct);
                                let _ = weak2.upgrade_in_event_loop(move |app| app.global::<App>().set_update_progress(k.min(1.0)));
                            }
                        });
                        let _ = weak.upgrade_in_event_loop(move |app| {
                            let g = app.global::<App>();
                            g.set_update_progress(-1.0);
                            match r {
                                Ok(path) => g.set_update_file(path.display().to_string().into()),
                                Err(e) if !e.is_empty() => error_box("ARKLIGHT", &format!("{} {e}", tr("Téléchargement impossible :", "Download failed:"))),
                                Err(_) => {}
                            }
                        });
                    });
                }
                "cancel" => {
                    if let Some(c) = cancel.borrow_mut().take() {
                        c.cancel();
                    }
                    g.set_update_progress(-1.0);
                }
                "install" => {
                    let file = g.get_update_file().to_string();
                    if file.is_empty() {
                        return;
                    }
                    // projet modifié : enregistrer d'abord (sinon ARKLIGHT se ferme et l'installeur s'ouvre)
                    let mut st = state.borrow_mut();
                    if guard(&app, &mut st, Pending::Install(file.clone())) {
                        drop(st);
                        if std::process::Command::new(&file).spawn().is_ok() {
                            let _ = slint::quit_event_loop();
                        }
                    }
                }
                _ => {}
            }
        });
    }
    {
        // « Ne plus afficher jusqu'à la prochaine version »
        let settings = settings.clone();
        app.global::<App>().on_news_hide(move |on| {
            let mut s = settings.borrow_mut();
            s.news_hidden = if on { env!("CARGO_PKG_VERSION").to_owned() } else { String::new() };
            save_settings(&s);
        });
    }
    // nouveautés au lancement (après une mise à jour), tant qu'elles ne sont pas masquées pour
    // cette version ; essais : seulement avec ARKLIGHT_SMOKE_NEWS
    {
        let hidden = settings.borrow().news_hidden == env!("CARGO_PKG_VERSION");
        app.global::<App>().set_news_hidden(hidden);
        let smoke = state.borrow().smoke;
        if (!hidden && !smoke) || std::env::var_os("ARKLIGHT_SMOKE_NEWS").is_some() {
            let w = app.as_weak();
            slint::Timer::single_shot(Duration::from_millis(if smoke { 100 } else { 900 }), move || {
                if let Some(app) = w.upgrade() {
                    app.global::<App>().invoke_update_action("news".into());
                }
            });
        }
    }
    if !state.borrow().smoke {
        check_update(false);
    }

    // ---- assistant IA
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let settings = settings.clone();
        app.global::<App>().on_ai(move |what, arg| {
            let Some(app) = app_weak.upgrade() else { return };
            // photo (fenêtre de Windows, sans tenir l'état) ; « Réinterpréter un plan dessiné… » :
            // envoyée tout de suite, puis le plan 2D s'ouvre
            if what.as_str() == "photo" || what.as_str() == "drawing" {
                let drawing = what.as_str() == "drawing";
                if drawing {
                    app.global::<App>().set_show_console(true);
                    app.global::<App>().set_bottom_tab(4);
                }
                let Some(path) = picture::pick() else { return };
                ai_panel::attach(&app, &mut state.borrow_mut(), path, drawing, drawing);
                return;
            }
            let mut st = state.borrow_mut();
            match what.as_str() {
                "photo-underlay" => {
                    if let Some(p) = st.ai.photo.as_mut() {
                        p.underlay = arg.as_str() == "1";
                    }
                    ai_panel::show(&app, &st);
                }
                "photo-remove" => {
                    st.ai.photo = None;
                    ai_panel::show(&app, &st);
                }
                "url" => {
                    let id = st.ai.settings.provider.clone();
                    st.ai.settings.set_url(&id, &arg);
                    ai_panel::list_models(&app, &mut st);
                }
                "send" => ai_panel::send(&app, &mut st, arg.to_string()),
                "stop" => {
                    ai_panel::stop(&mut st);
                    dictation::cancel(&app, &mut st);
                }
                "reset" => ai_panel::reset(&app, &mut st),
                "models" => ai_panel::list_models(&app, &mut st),
                "aiodos" => ai_panel::connect_aiodos(&app, &mut st),
                "aiodos-cancel" => ai_panel::cancel_aiodos(&app, &mut st),
                "aiodos-refresh" => ai_panel::refresh_aiodos(&app, &mut st),
                "aiodos-launch" => ai_panel::launch_aiodos(&app, &mut st),
                "aiodos-install" => ai_panel::install_aiodos(&app, &mut st),
                "key" => ai_panel::save_key(&app, &mut st, &arg),
                "provider" => {
                    if let Some(p) = arklight_ai::PROVIDERS.get(arg.parse::<usize>().unwrap_or(0)) {
                        st.ai.settings.set_provider(p.id);
                    }
                    ai_panel::list_models(&app, &mut st);
                    if st.ai.settings.provider == "aiodos" {
                        ai_panel::refresh_aiodos(&app, &mut st);
                    }
                }
                "model" => st.ai.settings.set_model(&arg),
                "voice-down" => dictation::key_down(&app, &mut st),
                "voice-up" => dictation::key_up(&app, &mut st),
                // onglet ouvert : l'état d'AIODOS relu
                _ => {
                    if st.ai.settings.provider == "aiodos" {
                        ai_panel::refresh_aiodos(&app, &mut st);
                    } else {
                        ai_panel::show(&app, &st);
                    }
                }
            }
            if matches!(what.as_str(), "provider" | "model" | "url") {
                let mut s = settings.borrow_mut();
                s.ai = st.ai.settings.clone();
                save_settings(&s);
                ai_panel::show(&app, &st);
            }
        });
    }

    // réglages de l'assistant changés par lui-même (modèle choisi tout seul) : gardés
    let ai_settings_timer = slint::Timer::default();
    {
        let settings = settings.clone();
        let state = state.clone();
        ai_settings_timer.start(slint::TimerMode::Repeated, Duration::from_secs(2), move || {
            let Ok(st) = state.try_borrow() else { return };
            if !st.smoke && settings.borrow().ai != st.ai.settings {
                settings.borrow_mut().ai = st.ai.settings.clone();
                save_settings(&settings.borrow());
            }
        });
    }

    // ---- face avant du projecteur (menus embarqués)
    let panel_gesture = {
        let state = state.clone();
        let app_weak = app.as_weak();
        let edit = edit.clone();
        move |g: onboard_panel::Gesture| {
            let Some(app) = app_weak.upgrade() else { return };
            let e = {
                let mut st = state.borrow_mut();
                if let onboard_panel::Gesture::Turn(knob, steps) = &g {
                    *st.knob_angles.entry(knob.clone()).or_default() += *steps as f32 * 12.0;
                }
                let Some(id) = onboard_panel::target(&mut st) else { return };
                let e = onboard_panel::gesture(&mut st, &id, g);
                st.panel_drawn = Instant::now() - Duration::from_secs(1); // redessin tout de suite
                e
            };
            if e.is_some() {
                edit(e, false);
            }
            show_panel(&app, &mut state.borrow_mut());
        }
    };
    {
        let go = panel_gesture.clone();
        app.global::<App>().on_panel_key(move |key, down| go(if down { onboard_panel::Gesture::Press(key.into()) } else { onboard_panel::Gesture::Release(key.into()) }));
    }
    {
        let go = panel_gesture.clone();
        app.global::<App>().on_panel_turn(move |knob, steps| go(onboard_panel::Gesture::Turn(knob.into(), steps)));
    }
    app.global::<App>().on_panel_shake(move || panel_gesture(onboard_panel::Gesture::Shake));

    // ---- plan 2D : outils de dessin, élément choisi, double-clic
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_plot_tool_set(move |i| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if plot_view::set_tool(&mut st, i.max(0) as usize) {
                toast(&app, &mut st, tr("Annotations affichées (menu Plan pour les masquer)", "Annotations shown (Plot menu to hide them)"));
            }
            refresh(&app, &mut st);
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_plot_item(move |what, arg| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if plot_view::item_change(&mut st, &what, &arg) {
                refresh(&app, &mut st);
            }
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_view_double(move || {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if st.plot.on && plot_view::double_click(&mut st) {
                refresh(&app, &mut st);
            }
        });
    }

    // ---- plan 2D : affichage et exports
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_plot_option(move |key| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            match key.as_str() {
                "underlay-import" => {
                    if !st.plot.on {
                        st.plot.on = true;
                    }
                    // tous formats (HEIC, WebP…), convertis sur un fil de travail
                    if let Some(path) = picture::pick() {
                        toast(&app, &mut st, tr("Lecture de l’image…", "Reading the image…"));
                        picture::load_async(path, plot_view::UNDERLAY_MAX, |app, st, pic| {
                            let msg = match pic.and_then(|pic| plot_view::place_underlay(st, &pic)) {
                                Ok(()) => tr("Image de fond posée : « Caler l’image » pour la mettre à l’échelle.", "Background image placed: “Calibrate the image” to scale it.").to_owned(),
                                Err(e) => e,
                            };
                            toast(app, st, &msg);
                            refresh(app, st);
                        });
                    }
                }
                "underlay-calibrate" => {
                    st.plot.on = true;
                    plot_view::start_calibration(&mut st);
                }
                "underlay-remove" => {
                    plot_view::remove_underlay(&mut st);
                }
                _ => plot_view::toggle_flag(&mut st.plot, &key),
            }
            refresh(&app, &mut st);
        });
    }
    {
        // fenêtre « Fond et cartouche » du plan
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_plot_meta(move |key, value| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if plot_view::meta_change(&mut st, &key, &value) {
                refresh(&app, &mut st);
            }
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_plot_export(move |what| {
            let Some(app) = app_weak.upgrade() else { return };
            let msg = if what.as_str() == "print" { Some(plot_view::print_sheet(&state.borrow())) } else { plot_view::export(&state.borrow(), &what, env!("CARGO_PKG_VERSION")) };
            if let Some(msg) = msg {
                toast(&app, &mut state.borrow_mut(), &msg);
            }
        });
    }

    // ---- fenêtre MIDI
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let settings = settings.clone();
        app.global::<App>().on_midi(move |what, arg| {
            let Some(app) = app_weak.upgrade() else { return };
            // fichiers (fenêtres de Windows) : sans tenir l'état pendant le choix
            if what.as_str() == "export" || what.as_str() == "import" {
                let msg = if what.as_str() == "export" {
                    let (empty, name) = {
                        let st = state.borrow();
                        let name = match st.midi.active_preset() {
                            Some(arklight_midi::controller::PresetRef::Builtin(p)) => i18n::t(&arklight_midi::presets::preset_label(p)),
                            Some(arklight_midi::controller::PresetRef::User(p)) => p.name.clone(),
                            None => tr("Mon contrôleur MIDI", "My MIDI controller").to_owned(),
                        };
                        (st.midi.all().is_empty(), name)
                    };
                    if empty {
                        tr("Rien à exporter : charge un préréglage ou apprends des commandes", "Nothing to export: load a preset or learn controls").to_owned()
                    } else {
                        let file = format!("{}.arkmidi.json", name.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "-"));
                        let Some(path) = rfd::FileDialog::new().set_title("ARKLIGHT").add_filter("ARKLIGHT MIDI", &["json"]).set_file_name(file).save_file() else { return };
                        let json = state.borrow().midi.export_preset(&name);
                        match std::fs::write(&path, serde_json::to_string_pretty(&json).unwrap_or_default()) {
                            Ok(()) => format!("{} {}", tr("Enregistré :", "Saved:"), path.display()),
                            Err(e) => format!("{} {e}", tr("Échec de l’enregistrement :", "Could not save:")),
                        }
                    }
                } else {
                    let Some(path) = rfd::FileDialog::new().set_title("ARKLIGHT").add_filter("ARKLIGHT MIDI", &["json"]).pick_file() else { return };
                    let json = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
                    let mut st = state.borrow_mut();
                    let r = match json {
                        Some(j) => st.midi.import_preset(&j),
                        None => Err("Fichier illisible".into()),
                    };
                    st.fader_marks = st.midi.fader_marks();
                    save_midi(&settings, &st);
                    refresh(&app, &mut st);
                    match r {
                        Ok(name) => tr_owned(format!("Préréglage importé : {name}"), format!("Preset imported: {name}")),
                        Err(e) => i18n::t(&e),
                    }
                };
                toast(&app, &mut state.borrow_mut(), &msg);
                return;
            }
            let mut st = state.borrow_mut();
            match what.as_str() {
                "channel" => {
                    let id = st.midi.preset().map(|p| p.id.clone());
                    st.midi.set_preset(id.as_deref(), arg.parse().ok());
                }
                "save-preset" => {
                    if st.midi.all().is_empty() {
                        toast(&app, &mut st, tr("Rien à enregistrer : charge un préréglage ou apprends des commandes", "Nothing to save: load a preset or learn controls"));
                        return;
                    }
                    let name = match st.midi.active_preset() {
                        Some(arklight_midi::controller::PresetRef::Builtin(p)) => i18n::t(&arklight_midi::presets::preset_label(p)),
                        Some(arklight_midi::controller::PresetRef::User(p)) => p.name.clone(),
                        None => String::new(),
                    };
                    ask(&app, &mut st, Prompt::MidiPreset, tr("Enregistrer comme préréglage", "Save as preset").into(), tr("Nom (la console ou le contrôleur, l’usage…)", "Name (console or controller, use…)"), name, "");
                    return;
                }
                "save-preset-now" => {
                    st.midi.save_user_preset(&arg);
                    toast(&app, &mut st, &tr_owned(format!("Préréglage enregistré : {arg}"), format!("Preset saved: {arg}")));
                }
                "delete-preset" => {
                    let Some(arklight_midi::controller::PresetRef::User(p)) = st.midi.active_preset() else { return };
                    let (id, name) = (p.id.clone(), p.name.clone());
                    confirm(&app, &mut st, Pending::MidiDeletePreset(id), &tr_owned(format!("Supprimer le préréglage « {name} » ?"), format!("Delete the preset “{name}”?")), tr("Les commandes qu’il contient seront oubliées.", "Its controls will be forgotten."), tr("Supprimer", "Delete"));
                    return;
                }
                "delete-preset-now" => st.midi.delete_user_preset(&arg),
                "learn" => st.midi.learn_new(),
                "stop" => st.midi.stop_learning(),
                "relearn" => st.midi.relearn(&arg),
                "remove" => st.midi.remove(&arg),
                "clear" => {
                    // question dans la fenêtre d'ARKLIGHT (une boîte de Windows pouvait rester cachée)
                    confirm(
                        &app,
                        &mut st,
                        Pending::MidiClear,
                        tr("Effacer toutes les correspondances apprises ?", "Clear all learned mappings?"),
                        tr("Le préréglage reste ; seules les commandes apprises sont oubliées.", "The preset stays; only the learned controls are forgotten."),
                        tr("Effacer", "Clear"),
                    );
                    return;
                }
                "clear-now" => st.midi.clear(),
                "wizard" => {
                    let kind = match arg.as_str() {
                        "groups" => WizardKind::Groups,
                        "fixtures" => WizardKind::Fixtures,
                        _ => WizardKind::Faders,
                    };
                    let order = fixture_order(&st.doc);
                    let plans = plan_ids(&st.doc.project);
                    let selection = st.selection.clone();
                    let fader_count = st.fader_count;
                    let s = &mut *st;
                    let ctx = MidiContext { fixture_order: Some(&order), selection: &selection, selected_id: selection.first().map(String::as_str), plans: &plans, fader_count, ..MidiContext::new(&s.doc.project) };
                    if !s.midi.toggle_wizard(kind, &ctx) {
                        let msg = match kind {
                            WizardKind::Groups => tr("Crée d’abord des groupes (Sélection › Grouper).", "Create groups first (Selection › Group)."),
                            _ => tr("Rien à apprendre : ajoute des projecteurs.", "Nothing to learn: add fixtures."),
                        };
                        toast(&app, &mut st, msg);
                    }
                }
                "preset" => {
                    let i: usize = arg.parse().unwrap_or(0);
                    let n = arklight_midi::MIDI_PRESETS.len();
                    let id = match i {
                        0 => None,
                        i if i <= n => Some(arklight_midi::MIDI_PRESETS[i - 1].id.to_owned()),
                        i => st.midi.user_presets().get(i - 1 - n).map(|p| p.id.clone()),
                    };
                    st.midi.set_preset(id.as_deref(), None);
                }
                _ => {}
            }
            st.fader_marks = st.midi.fader_marks();
            save_midi(&settings, &st);
            refresh(&app, &mut st);
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let settings = settings.clone();
        app.global::<App>().on_midi_kind(move |id, i| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let Some(kind) = TARGET_KINDS.get(i.max(0) as usize).map(|k| k.id) else { return };
            let previous = st.midi.all().iter().find(|m| m.id == id.as_str()).map(|m| m.target.clone()).unwrap_or_default();
            let selection = st.selection.clone();
            let target = {
                let ctx = MidiContext { selection: &selection, selected_id: selection.first().map(String::as_str), ..MidiContext::new(&st.doc.project) };
                Midi::target_for_kind(kind, &previous, &ctx)
            };
            st.midi.update(&id, |m| m.target = target);
            st.fader_marks = st.midi.fader_marks();
            save_midi(&settings, &st);
            refresh(&app, &mut st);
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let settings = settings.clone();
        app.global::<App>().on_midi_param(move |id, i| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let i = i.max(0) as usize;
            let groups: Vec<String> = st.doc.project.groups.iter().map(|g| g.name.clone()).collect();
            let fixtures: Vec<String> = st.doc.project.fixtures.iter().map(|f| f.name.clone()).collect();
            st.midi.update(&id, |m| {
                let kind = m.target.kind.clone();
                if MIDI_INDEX_KINDS.contains(&kind.as_str()) {
                    m.target.index = Some(i as u32 + 1);
                } else if matches!(kind.as_str(), "group" | "groupLevel") {
                    m.target.name = groups.get(i).cloned();
                } else if matches!(kind.as_str(), "fixture" | "fixtureLevel") {
                    m.target.name = fixtures.get(i).cloned();
                } else if kind == "command" {
                    m.target.id = commands::COMMANDS.get(i).map(|c| c.id.to_owned());
                }
            });
            st.fader_marks = st.midi.fader_marks();
            save_midi(&settings, &st);
            refresh(&app, &mut st);
        });
    }

    {
        // fenêtre « Séquences et plans… »
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_plans_action(move |what, id, arg| {
            use arklight_core::plans::Start;
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let start = |m: &str| match m {
                "decor" => Start::Decor,
                "empty" => Start::Empty,
                _ => Start::Copy,
            };
            let name_of = |st: &State, id: &str| {
                st.doc.plan_info().into_iter().find_map(|s| if s.id == id { Some(s.name) } else { s.plans.into_iter().find(|p| p.id == id).map(|p| p.name) }).unwrap_or_default()
            };
            // le plan qu'on quitte garde sa vignette
            let keep_thumb = |st: &mut State| {
                if let Some(t) = st.thumb.take() {
                    let cur = st.doc.current_plan_id();
                    st.doc.set_plan_thumbnail(&cur, &t);
                }
            };
            let switched = match what.as_str() {
                "show" => {
                    show_plans(&app, &st);
                    app.global::<App>().set_plans_open(true);
                    return;
                }
                "open" => {
                    if id != st.doc.current_plan_id() {
                        keep_thumb(&mut st);
                        st.doc.switch_plan(&id);
                    }
                    app.global::<App>().set_plans_open(false);
                    true
                }
                "rename" | "seq-rename" => {
                    let name = name_of(&st, &id);
                    let title = if what == "rename" { tr("Renommer le plan", "Rename the shot") } else { tr("Renommer la séquence", "Rename the sequence") };
                    let note = if what == "rename" { tr("Par exemple « Plan 3 — contrechamp » ou « 12/4 gros plan ».", "For example “Shot 3 — reverse” or “12/4 close-up”.") } else { tr("Par exemple « Séq. 12 — INT. CAFÉ — NUIT ».", "For example “Seq. 12 — INT. CAFÉ — NIGHT”.") };
                    ask(&app, &mut st, Prompt::RenamePlan(id.to_string()), title.into(), tr("Nom", "Name"), name, note);
                    return;
                }
                "duplicate" => {
                    st.doc.duplicate_plan(&id);
                    false
                }
                "move" => {
                    let (seq, at) = arg.split_once(':').unwrap_or((arg.as_str(), "end"));
                    st.doc.move_plan(&id, Some(seq), at.parse().ok());
                    false
                }
                "delete" => {
                    if st.doc.plan_info().iter().map(|s| s.plans.len()).sum::<usize>() <= 1 {
                        toast(&app, &mut st, tr("Le projet garde au moins un plan", "The project keeps at least one shot"));
                        return;
                    }
                    let name = name_of(&st, &id);
                    confirm(
                        &app,
                        &mut st,
                        Pending::DeletePlan(id.to_string()),
                        &tr_owned(format!("Supprimer le plan « {name} » ?"), format!("Delete the shot “{name}”?")),
                        tr("Son plan de feu (projecteurs, grip, décor, annotations) sera perdu. On ne peut pas l’annuler.", "Its lighting plot (fixtures, grip, set, notes) will be lost. This can’t be undone."),
                        tr("Supprimer", "Delete"),
                    );
                    return;
                }
                "seq-delete" => {
                    let name = name_of(&st, &id);
                    let n = st.doc.plan_info().iter().find(|s| s.id == id.as_str()).map_or(0, |s| s.plans.len());
                    confirm(
                        &app,
                        &mut st,
                        Pending::DeleteSequence(id.to_string()),
                        &tr_owned(format!("Supprimer la séquence « {name} » ?"), format!("Delete the sequence “{name}”?")),
                        &tr_owned(format!("Ses {n} plan(s) seront perdus. On ne peut pas l’annuler."), format!("Its {n} shot(s) will be lost. This can’t be undone.")),
                        tr("Supprimer", "Delete"),
                    );
                    return;
                }
                "delete-now" => {
                    let current = id == st.doc.current_plan_id();
                    st.doc.remove_plan(&id);
                    current
                }
                "seq-delete-now" => {
                    let current = st.doc.plan_info().iter().any(|s| s.id == id.as_str() && s.plans.iter().any(|p| p.current));
                    st.doc.remove_sequence(&id);
                    current
                }
                "new-plan" => {
                    keep_thumb(&mut st);
                    st.doc.new_plan_in(Some(&id), start(&arg));
                    true
                }
                "new-seq" => {
                    keep_thumb(&mut st);
                    st.doc.new_sequence(start(&arg));
                    true
                }
                "seq-move" => {
                    if let Ok(i) = arg.parse::<i32>() {
                        st.doc.move_sequence(&id, i.max(0) as usize);
                    }
                    false
                }
                _ => return,
            };
            if switched {
                st.selection.clear();
                st.lights.clear();
                st.thumb = None;
            }
            refresh(&app, &mut st);
            show_plans(&app, &st);
        });
    }

    // ---- séquences et plans
    let plan_action = |f: fn(&mut Document, &str) -> bool| {
        let state = state.clone();
        let app_weak = app.as_weak();
        move |arg: String| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let cur = st.doc.current_plan_id();
            let thumb = st.thumb.clone();
            if f(&mut st.doc, &arg) {
                if let Some(t) = thumb.filter(|_| st.doc.current_plan_id() != cur) {
                    st.doc.set_plan_thumbnail(&cur, &t);
                    st.thumb = None;
                }
                st.selection.clear();
                st.lights.clear();
                refresh(&app, &mut st);
            }
        }
    };
    {
        let go = plan_action(|d, id| d.switch_plan(id));
        let state = state.clone();
        app.global::<App>().on_switch_plan(move |i| {
            let id = plan_list(&state.borrow().doc).0.get(i.max(0) as usize).map(|(id, _)| id.clone());
            if let Some(id) = id {
                go(id);
            }
        });
    }
    {
        let go = plan_action(|d, mode| d.new_plan(if mode == "decor" { arklight_core::plans::Start::Decor } else { arklight_core::plans::Start::Copy }).is_some());
        app.global::<App>().on_new_plan(move |mode| go(mode.to_string()));
    }
    {
        let go = plan_action(|d, _| d.new_sequence(arklight_core::plans::Start::Empty).is_some());
        app.global::<App>().on_new_sequence(move || go(String::new()));
    }
    {
        let go = plan_action(|d, dir| d.step_plan(dir == "next"));
        app.global::<App>().on_step_plan(move |forward| go(if forward { "next".into() } else { "prev".into() }));
    }
    {
        let go = plan_action(|d, _| {
            let (list, cur) = plan_list(d);
            let id = list.get(cur).map(|(id, _)| id.clone()).unwrap_or_default();
            d.remove_plan(&id)
        });
        app.global::<App>().on_remove_plan(move || go(String::new()));
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_monitor_universe(move |i| {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if let Some(u) = monitor_universes(&st).get(i.max(0) as usize) {
                st.monitor_universe = *u;
                st.monitor_picked = true;
            }
            show_monitor(&app, &mut st);
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let settings = settings.clone();
        let show_recent = show_recent.clone();
        app.global::<App>().on_open_recent(move |i| {
            let Some(app) = app_weak.upgrade() else { return };
            let Some(path) = settings.borrow().recent.get(i.max(0) as usize).map(PathBuf::from) else { return };
            if !guard(&app, &mut state.borrow_mut(), Pending::Recent(i)) {
                return;
            }
            match Project::open(&path) {
                Ok(p) => {
                    open_doc(&app, &state, p, Some(path.clone()));
                    remember(&settings, &path);
                }
                Err(e) => error_box("ARKLIGHT", &format!("{}\n{}", path.display(), e)),
            }
            show_recent();
        });
    }

    // ---- console
    {
        let state = state.clone();
        let edit = edit.clone();
        app.global::<App>().on_console_fader(move |i, v| {
            let e = {
                let st = state.borrow();
                let families = console_panel::families(&st.doc, &st.selection);
                let ids = families.get(st.console_family).cloned().unwrap_or_default();
                console_panel::move_fader(&st.doc, &ids, &st.lights, &st.dmx, i.max(0) as usize, v)
            };
            edit(e, false);
        });
    }
    {
        let state = state.clone();
        let edit = edit.clone();
        app.global::<App>().on_console_release(move || {
            let e = console_panel::release(&state.borrow().doc, &state.borrow().selection);
            edit(e, false);
        });
    }

    // ---- télécommande infrarouge (ARC1 / ARC3)
    app.global::<App>().set_remote_keys(ModelRc::new(VecModel::from(remote::keys(false))));
    {
        let state = state.clone();
        let edit = edit.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_remote(move |what, arg| {
            let Some(app) = app_weak.upgrade() else { return };
            let g = app.global::<App>();
            match what.as_str() {
                "model" => {
                    let arc3 = arg.as_str() == "1";
                    g.set_remote_model(if arc3 { 1 } else { 0 });
                    g.set_remote_keys(ModelRc::new(VecModel::from(remote::keys(arc3))));
                }
                "key" => {
                    let (e, msg) = remote::press(&mut state.borrow_mut(), &arg, g.get_remote_target() == 1);
                    g.set_remote_feedback(msg.as_str().into());
                    log(&mut state.borrow_mut(), "", &msg);
                    if e.is_some() {
                        edit(e, false);
                    }
                }
                _ => {}
            }
        });
    }

    // ---- moniteur DMX : clic sur un canal = son projecteur
    {
        let app_weak = app.as_weak();
        app.global::<App>().on_monitor_pick(move |id, add| {
            if let Some(app) = app_weak.upgrade() {
                app.global::<App>().invoke_select_row(id, add);
            }
        });
    }

    // ---- réseau : numérotation Art-Net, adresse du PC
    app.global::<App>().set_net_offset(settings.borrow().artnet_offset.min(1) as i32);
    if !state.borrow().smoke {
        app.global::<App>().set_net_ip(arklight_net::receiver::primary_ipv4().map(|ip| ip.to_string()).unwrap_or_default().into());
    }
    {
        let state = state.clone();
        let settings = settings.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_net_offset_changed(move |i| {
            let offset = i.clamp(0, 1) as u32;
            let mut s = settings.borrow_mut();
            s.artnet_offset = offset;
            save_settings(&s);
            let mut st = state.borrow_mut();
            if let Some(net) = &st.net {
                net.set_artnet_offset(offset);
            }
            log(&mut st, "", if offset == 1 { tr("Numérotation Art-Net : Art-Net 0 = Univers 1", "Art-Net numbering: Art-Net 0 = Universe 1") } else { tr("Numérotation Art-Net : Art-Net 0 = Univers 0", "Art-Net numbering: Art-Net 0 = Universe 0") });
            if let Some(app) = app_weak.upgrade() {
                app.global::<App>().set_net_offset(offset as i32);
            }
        });
    }

    // ---- console : plein feu, noir, arc-en-ciel, relâcher, valeurs brutes, famille, profil
    {
        let state = state.clone();
        let edit = edit.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_console_action(move |what| {
            let Some(app) = app_weak.upgrade() else { return };
            let (name, arg) = what.split_once(':').map_or((what.as_str(), ""), |(a, b)| (a, b));
            let e = {
                let mut st = state.borrow_mut();
                let families = console_panel::families(&st.doc, &st.selection);
                let ids = families.get(st.console_family).cloned().unwrap_or_default();
                let s = &mut *st;
                match name {
                    "full" | "black" => console_panel::set_intensity(&s.doc, &ids, &s.lights, &s.dmx, if name == "full" { 1.0 } else { 0.0 }, &mut s.console_mem),
                    "rainbow" => console_panel::rainbow(&s.doc, &ids, &s.lights, &s.dmx),
                    "release" => console_panel::release(&s.doc, &s.selection),
                    "release-all" => {
                        let all: Vec<String> = s.doc.project.fixtures.iter().filter(|f| arklight_core::engine::console::console_held(f)).map(|f| f.id.clone()).collect();
                        console_panel::release(&s.doc, &all)
                    }
                    "raw" => {
                        s.console_raw = !s.console_raw;
                        refresh(&app, s);
                        None
                    }
                    "family" => {
                        s.console_family = arg.parse().unwrap_or(0);
                        refresh(&app, s);
                        None
                    }
                    "profile" => {
                        // nouveau profil DMX pour la famille pilotée (`setProfile`)
                        let i: usize = arg.parse().unwrap_or(0);
                        let mode = ids.first().and_then(|id| s.doc.fixture(id)).and_then(|f| library().fixture(&f.type_id)).and_then(|t| t.dmx.modes.get(i)).map(|m| m.id.clone());
                        mode.map(|m| Edit::UpdateFixtures(ids.iter().map(|id| (id.clone(), Changes { settings: json!({ "dmxMode": m }).as_object().cloned(), ..Changes::default() })).collect()))
                    }
                    _ => None,
                }
            };
            if e.is_some() {
                edit(e, false);
            }
        });
    }

    // ---- édition
    let duplicate = {
        let state = state.clone();
        let edit = edit.clone();
        move || {
            let e = duplicate_edit(&state.borrow());
            edit(e, false);
        }
    };
    let delete = {
        let state = state.clone();
        let edit = edit.clone();
        move || {
            let e = delete_edit(&state.borrow());
            edit(e, false);
        }
    };
    {
        let duplicate = duplicate.clone();
        let delete = delete.clone();
        let state = state.clone();
        let edit = edit.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_field_button(move |k| {
            let Some(app) = app_weak.upgrade() else { return };
            let press = {
                let st = state.borrow();
                inspector::press(&st.doc, &st.selection, &k)
            };
            match press {
                Some(inspector::Press::Edit(e)) => edit(Some(e), false),
                Some(inspector::Press::Crmx(e, id)) => {
                    // l'entrée des Astera se recalcule (plus de verrouillage)
                    state.borrow_mut().lights.runtime_mut(&id).latch = None;
                    edit(Some(e), false);
                }
                Some(inspector::Press::Action(a)) => app.global::<App>().invoke_action(a.into()),
                Some(inspector::Press::Duplicate) => duplicate(),
                Some(inspector::Press::Delete) => delete(),
                Some(inspector::Press::TakeConsole) => {
                    let e = {
                        let st = state.borrow();
                        console_panel::take(&st.doc, &st.selection, &st.lights, &st.dmx)
                    };
                    edit(e, false);
                }
                Some(inspector::Press::ReleaseConsole) => app.global::<App>().invoke_console_release(),
                Some(inspector::Press::Select(id)) => app.global::<App>().invoke_select_row(id.into(), false),
                Some(inspector::Press::AddressSequential) => {
                    let (e, message) = {
                        let st = state.borrow();
                        let sel = st.selection.join(",");
                        let (u, a) = st.multi_address.clone().filter(|m| m.0 == sel).map_or_else(|| multi_start(&st), |m| (m.1, m.2));
                        let ids: Vec<String> = st.selection.iter().filter(|id| st.doc.fixture(id).is_some()).cloned().collect();
                        let opts = arklight_core::addressing::Options { scope: arklight_core::addressing::Scope::Ids(ids), universe: u, address: a, pack: true, ..Default::default() };
                        let plan = arklight_core::addressing::plan_addressing(&st.doc.project.fixtures, &opts);
                        let placed = plan.considered - plan.unplaced.len();
                        let message = if !plan.unplaced.is_empty() {
                            format!("{placed} {} {} {}", tr("projecteur(s) adressé(s),", "fixture(s) addressed,"), plan.unplaced.len(), tr("sans place libre", "with no free room"))
                        } else if plan.moves.is_empty() {
                            tr("Les adresses étaient déjà à la suite", "The addresses were already one after another").to_owned()
                        } else {
                            format!("{placed} {}", tr("projecteur(s) adressé(s) à la suite", "fixture(s) addressed one after another"))
                        };
                        let items: Vec<(String, Changes)> = plan
                            .moves
                            .iter()
                            .map(|m| (m.id.clone(), Changes { settings: json!({ "universe": m.to.universe, "dmxAddress": m.to.address }).as_object().cloned(), ..Changes::default() }))
                            .collect();
                        ((!items.is_empty()).then_some(Edit::UpdateFixtures(items)), message)
                    };
                    edit(e, false);
                    toast(&app, &mut state.borrow_mut(), &message);
                }
                None => {}
            }
        });
    }
    app.global::<App>().on_duplicate(duplicate);
    app.global::<App>().on_delete_selection(delete);
    let history = |redo: bool| {
        let state = state.clone();
        let app_weak = app.as_weak();
        move || {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let label = step_label(if redo { st.doc.redo_label() } else { st.doc.undo_label() });
            if if redo { st.doc.redo() } else { st.doc.undo() } {
                refresh(&app, &mut st);
                let what = if redo { tr("Rétabli", "Redone") } else { tr("Annulé", "Undone") };
                toast(&app, &mut st, &format!("{what} : {label}"));
            }
        }
    };
    app.global::<App>().on_undo(history(false));
    app.global::<App>().on_redo(history(true));
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_select_all(move || {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.selection = st.doc.project.fixtures.iter().map(|f| f.id.clone()).chain(st.doc.project.objects.iter().map(|o| o.id.clone())).collect();
            refresh(&app, &mut st);
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_select_none(move || {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.selection.clear();
            refresh(&app, &mut st);
        });
    }

    // ---- projets
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_new_project(move || {
            let Some(app) = app_weak.upgrade() else { return };
            if !guard(&app, &mut state.borrow_mut(), Pending::New) {
                return;
            }
            open_doc(&app, &state, Project::empty(tr("Nouveau projet", "New project")), None);
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        let settings = settings.clone();
        let show_recent = show_recent.clone();
        app.global::<App>().on_open_project(move || {
            let Some(app) = app_weak.upgrade() else { return };
            if !guard(&app, &mut state.borrow_mut(), Pending::Open) {
                return;
            }
            let Some(path) = rfd::FileDialog::new().set_title("ARKLIGHT").add_filter("ARKLIGHT", &["json"]).pick_file() else { return };
            match Project::open(&path) {
                Ok(p) => {
                    open_doc(&app, &state, p, Some(path.clone()));
                    remember(&settings, &path);
                    show_recent();
                }
                Err(e) => error_box("ARKLIGHT", &format!("{}\n{}", path.display(), e)),
            }
        });
    }
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_open_demo(move |id| {
            let Some(app) = app_weak.upgrade() else { return };
            if !guard(&app, &mut state.borrow_mut(), Pending::Demo(id.to_string())) {
                return;
            }
            if let Some(p) = demo(&id) {
                open_doc(&app, &state, p, None);
            }
        });
    }
    let saver = |ask: bool| {
        let state = state.clone();
        let app_weak = app.as_weak();
        let settings = settings.clone();
        let show_recent = show_recent.clone();
        move || {
            let Some(app) = app_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if save(&mut st, ask) {
                if let Some(p) = st.doc.path.clone() {
                    remember(&settings, &p);
                    show_recent();
                }
                refresh(&app, &mut st);
            }
        }
    };
    app.global::<App>().on_save_project(saver(false));
    app.global::<App>().on_save_project_as(saver(true));
    {
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_quit(move || {
            let Some(app) = app_weak.upgrade() else { return };
            if guard(&app, &mut state.borrow_mut(), Pending::Quit) {
                let _ = slint::quit_event_loop();
            }
        });
    }
    {
        // fermeture de la fenêtre (croix) : même question que « Quitter », dans la fenêtre d'ARKLIGHT
        let state = state.clone();
        let app_weak = app.as_weak();
        app.window().on_close_requested(move || {
            let Some(app) = app_weak.upgrade() else { return slint::CloseRequestResponse::HideWindow };
            if std::env::var_os("ARKLIGHT_SMOKE").is_some() || guard(&app, &mut state.borrow_mut(), Pending::Quit) {
                let _ = slint::quit_event_loop();
                slint::CloseRequestResponse::HideWindow
            } else {
                slint::CloseRequestResponse::KeepWindowShown
            }
        });
    }
    {
        // réponse : 0 annuler, 1 enregistrer puis continuer, 2 continuer sans enregistrer
        let state = state.clone();
        let app_weak = app.as_weak();
        app.global::<App>().on_ask_answer(move |answer| {
            let Some(app) = app_weak.upgrade() else { return };
            app.global::<App>().set_ask_open(false);
            let pending = {
                let mut st = state.borrow_mut();
                let Some(pending) = st.pending.take() else { return };
                if answer == 0 || (answer == 1 && !save(&mut st, false)) {
                    return;
                }
                if answer == 1 {
                    refresh(&app, &mut st);
                }
                st.skip_guard = true;
                pending
            };
            let g = app.global::<App>();
            match pending {
                Pending::Revert => {
                    let path = state.borrow().doc.path.clone();
                    if let Some(path) = path {
                        match Project::open(&path) {
                            Ok(p) => {
                                open_doc(&app, &state, p, Some(path));
                                let mut st = state.borrow_mut();
                                toast(&app, &mut st, tr("Version enregistrée rouverte", "Saved version reopened"));
                            }
                            Err(e) => error_box("ARKLIGHT", &format!("{}\n{}", path.display(), e)),
                        }
                    }
                }
                Pending::MidiClear => g.invoke_midi("clear-now".into(), SharedString::new()),
                Pending::DeletePlan(id) => g.invoke_plans_action("delete-now".into(), id.into(), SharedString::new()),
                Pending::DeleteSequence(id) => g.invoke_plans_action("seq-delete-now".into(), id.into(), SharedString::new()),
                Pending::Quit => {
                    let _ = slint::quit_event_loop();
                }
                Pending::New => g.invoke_new_project(),
                Pending::Open => g.invoke_open_project(),
                Pending::Recent(i) => g.invoke_open_recent(i),
                Pending::Demo(id) => g.invoke_open_demo(id.into()),
                Pending::Card(i) => g.invoke_projects("card".into(), i),
                Pending::OpenPath(p) => g.invoke_open_path(p.into()),
                Pending::Install(p) => {
                    if std::process::Command::new(&p).spawn().is_ok() {
                        let _ = slint::quit_event_loop();
                    }
                }
                Pending::MidiDeletePreset(id) => g.invoke_midi("delete-preset-now".into(), id.into()),
            }
            state.borrow_mut().skip_guard = false;
        });
    }

    // ---- langue
    {
        let settings = settings.clone();
        let app_weak = app.as_weak();
        let state = state.clone();
        app.global::<App>().on_set_language(move |choice| {
            apply_language(&choice);
            settings.borrow_mut().language = choice.to_string();
            save_settings(&settings.borrow());
            if let Some(a) = app_weak.upgrade() {
                a.global::<App>().set_keys_rev(a.global::<App>().get_keys_rev() + 1);
                a.global::<App>().set_language(choice);
                show_library(&a, &state.borrow().search, &settings.borrow().library);
                refresh(&a, &mut state.borrow_mut());
            }
        });
    }

    // Test automatique (développement) : ARKLIGHT_SMOKE=capture.png → capture de la fenêtre puis
    // fermeture ; ARKLIGHT_SMOKE_SELECT=1 choisit d'abord le premier projecteur (inspecteur rempli)
    if let Ok(out) = std::env::var("ARKLIGHT_SMOKE") {
        if let Some(which) = std::env::var_os("ARKLIGHT_SMOKE_SELECT") {
            // « 1 » : le premier projecteur ; sinon le premier dont le nom contient ce texte
            let which = which.to_string_lossy().to_string();
            let mut st = state.borrow_mut();
            let f = st.doc.project.fixtures.iter().find(|f| which == "1" || f.name.contains(&which)).map(|f| f.id.clone());
            // sinon un objet (grip, décor) dont le nom contient ce texte
            let o = || st.doc.project.objects.iter().find(|o| o.name.contains(&which)).map(|o| o.id.clone());
            st.selection = f.or_else(o).map(|id| vec![id]).unwrap_or_default();
            refresh(&app, &mut st);
            // menus du clic droit préparés (ARKLIGHT_SMOKE_CTX=1) : écrits sur la sortie d'erreur
            if std::env::var_os("ARKLIGHT_SMOKE_CTX").is_some() {
                context_menus(&app, &mut st);
                let g = app.global::<App>();
                eprintln!("objet : {} · caméra : {:?}", g.get_ctx_object(), g.get_ctx_camera());
                for m in g.get_ctx_menus().iter() {
                    eprintln!("{} — {} choix, familles : {:?}", m.title, m.items.row_count(), m.subs.iter().map(|s| format!("{} ({})", s.title, s.items.row_count())).collect::<Vec<_>>());
                }
            }
            drop(st);
            // essai de la console : premier fader à fond (ARKLIGHT_SMOKE_FADER=valeur)
            if let Some(v) = std::env::var("ARKLIGHT_SMOKE_FADER").ok().and_then(|v| v.parse::<f32>().ok()) {
                app.global::<App>().invoke_console_fader(0, v);
            }
        }
        // onglet du bas (0 console, 1 moniteur DMX, 2 MIDI) et préréglage MIDI (n° de la liste)
        if let Some(tab) = std::env::var("ARKLIGHT_SMOKE_TAB").ok().and_then(|t| t.parse::<i32>().ok()) {
            app.global::<App>().set_bottom_tab(tab);
            if tab == 5 {
                app.global::<App>().invoke_ble("show".into(), SharedString::new());
            }
        }
        // inspecteur défilé jusqu'en bas (ARKLIGHT_SMOKE_INSPECTOR_END=1), après sa mise en page
        if std::env::var_os("ARKLIGHT_SMOKE_INSPECTOR_END").is_some() {
            let w = app.as_weak();
            slint::Timer::single_shot(Duration::from_millis(2000), move || {
                if let Some(a) = w.upgrade() {
                    a.global::<App>().set_inspector_end(a.global::<App>().get_inspector_end() + 1);
                }
            });
        }
        if std::env::var_os("ARKLIGHT_SMOKE_GEOM").is_some() {
            let w = app.as_weak();
            slint::Timer::single_shot(Duration::from_millis(2000), move || {
                if let Some(a) = w.upgrade() {
                    eprintln!("vue {}x{} à ({}, {}) · fenêtre {:?}", a.get_view_width(), a.get_view_height(), a.get_view_x(), a.get_view_y(), a.window().size());
                    use slint::winit_030::WinitWindowAccessor;
                    a.window().with_winit_window(|w| {
                        eprintln!("winit : agrandie {} · intérieur {:?} · cadre {:?} à {:?} · écran {:?}", w.is_maximized(), w.inner_size(), w.outer_size(), w.outer_position(), w.current_monitor().map(|m| m.size()));
                    });
                }
            });
        }
        // bandeau de mise à jour simulé (ARKLIGHT_SMOKE_UPDATE=1.1.0[:0.4] : avancement), rien n'est téléchargé
        if let Ok(spec) = std::env::var("ARKLIGHT_SMOKE_UPDATE") {
            let (v, k) = spec.split_once(':').unwrap_or((spec.as_str(), ""));
            let g = app.global::<App>();
            g.set_update_version(v.into());
            g.set_update_url("https://github.com/22eme-Arkane/ARKLIGHT/releases".into());
            g.set_update_download("https://github.com/22eme-Arkane/ARKLIGHT/releases/download/x/ARKLIGHT-Setup.exe".into());
            if let Ok(k) = k.parse::<f32>() {
                g.set_update_progress(k);
            }
        }
        // photo jointe à l'assistant (ARKLIGHT_SMOKE_PHOTO=chemin), sans fenêtre de choix
        if let Some(path) = std::env::var_os("ARKLIGHT_SMOKE_PHOTO").map(PathBuf::from) {
            ai_panel::attach(&app, &mut state.borrow_mut(), path, true, false);
        }
        if let Ok(preset) = std::env::var("ARKLIGHT_SMOKE_MIDI_PRESET") {
            app.global::<App>().invoke_midi("preset".into(), preset.into());
        }
        // commandes (ARKLIGHT_SMOKE_COMMAND=labels.toggle;view.plot…)
        for c in std::env::var("ARKLIGHT_SMOKE_COMMAND").unwrap_or_default().split(';').filter(|c| !c.is_empty()) {
            commands::run(&app, c);
        }
        // dessin simulé sur le plan (ARKLIGHT_SMOKE_PLOTDRAW) : un rectangle avec l'outil
        if std::env::var_os("ARKLIGHT_SMOKE_PLOTDRAW").is_some() {
            let mut st = state.borrow_mut();
            st.plot.on = true;
            plot_view::set_tool(&mut st, 1);
            plot_view::down(&mut st, [-2.0, 2.5], arklight_plot::Modifiers::default());
            plot_view::moved(&mut st, [1.0, 4.0]);
            plot_view::up(&mut st, [1.0, 4.0]);
            refresh(&app, &mut st);
        }
        // aide des micros (ARKLIGHT_SMOKE_TIP) et texte dicté simulé (ARKLIGHT_SMOKE_DICTATE=champ=texte)
        if std::env::var_os("ARKLIGHT_SMOKE_TIP").is_some() {
            let g = app.global::<Dictation>();
            g.set_tip_x(270.0);
            g.set_tip_y(150.0);
            g.set_tip_visible(true);
        }
        if let Some((field, text)) = std::env::var("ARKLIGHT_SMOKE_DICTATE").ok().and_then(|v| v.split_once('=').map(|(a, b)| (a.to_owned(), b.to_owned()))) {
            // une fois les champs affichés
            let app_weak = app.as_weak();
            slint::Timer::single_shot(Duration::from_millis(1500), move || {
                if let Some(app) = app_weak.upgrade() {
                    let g = app.global::<Dictation>();
                    g.set_result(text.into());
                    g.set_result_for(field.into());
                    g.set_seq(g.get_seq() + 1);
                }
            });
        }
        // glisser simulé (ARKLIGHT_SMOKE_DRAG) : « x:60 » (poignée X, 60 pixels), « body:0,-400 »
        // (le projecteur lui-même, vers l'horizon) ; positions écrites sur la sortie d'erreur
        if let Ok(spec) = std::env::var("ARKLIGHT_SMOKE_DRAG") {
            let app_weak = app.as_weak();
            let state = state.clone();
            slint::Timer::single_shot(Duration::from_millis(1500), move || {
                let Some(app) = app_weak.upgrade() else { return };
                let g = app.global::<App>().get_gizmo();
                let pos = |st: &State| st.selection.first().and_then(|id| st.doc.fixture(id).map(|f| (f.name.clone(), f.position, f.rotation)).or_else(|| st.doc.object(id).map(|o| (o.name.clone(), o.position, o.rotation))));
                eprintln!("avant : {:?} (axes : {})", pos(&state.borrow()), g.on);
                let (kind, arg) = spec.split_once(':').unwrap_or(("x", "60"));
                let n: Vec<f32> = arg.split(',').filter_map(|v| v.parse().ok()).collect();
                let (dx, dy) = (n.first().copied().unwrap_or(60.0), n.get(1).copied().unwrap_or(0.0));
                if kind == "body" {
                    app.global::<App>().invoke_view_pointer(1, 0, g.cx, g.cy, false, false);
                    for i in 1..=10 {
                        let k = i as f32 / 10.0;
                        app.global::<App>().invoke_view_moved(g.cx + dx * k, g.cy + dy * k);
                    }
                    app.global::<App>().invoke_view_pointer(2, 0, g.cx + dx, g.cy + dy, false, false);
                } else if kind.starts_with('r') {
                    // anneau (outil Pivoter) : depuis son premier point de saisie
                    let dots = app.global::<App>().get_gizmo_dots();
                    let Some(d) = dots.iter().find(|d| d.axis == kind) else {
                        eprintln!("pas d'anneau {kind}");
                        return;
                    };
                    app.global::<App>().invoke_gizmo_drag(1, kind.into(), d.x, d.y);
                    for i in 1..=10 {
                        let k = i as f32 / 10.0;
                        app.global::<App>().invoke_gizmo_drag(0, kind.into(), d.x + dx * k, d.y + dy * k);
                    }
                    eprintln!("angle affiché : {}", app.global::<App>().get_gizmo().angle);
                    app.global::<App>().invoke_gizmo_drag(2, kind.into(), d.x + dx, d.y + dy);
                } else {
                    let (ex, ey) = match kind {
                        "y" => (g.yx, g.yy),
                        "z" => (g.zx, g.zy),
                        "xz" => (g.cx, g.cy),
                        _ => (g.xx, g.xy),
                    };
                    app.global::<App>().invoke_gizmo_drag(1, kind.into(), ex, ey);
                    for i in 1..=10 {
                        let k = i as f32 / 10.0;
                        app.global::<App>().invoke_gizmo_drag(0, kind.into(), ex + dx * k, ey + dy * k);
                    }
                    app.global::<App>().invoke_gizmo_drag(2, kind.into(), ex + dx, ey + dy);
                }
                eprintln!("après : {:?}", pos(&state.borrow()));
            });
        }
        // actions du menu « Sélection » (ARKLIGHT_SMOKE_ACTION=group;readdress…)
        for a in std::env::var("ARKLIGHT_SMOKE_ACTION").unwrap_or_default().split(';').filter(|a| !a.is_empty()) {
            app.global::<App>().invoke_action(a.into());
        }
        let app_weak = app.as_weak();
        let state = state.clone();
        // capture au bout de 3 s (ARKLIGHT_SMOKE_WAIT=ms : plus tard, une photo à convertir par exemple)
        let wait = std::env::var("ARKLIGHT_SMOKE_WAIT").ok().and_then(|w| w.parse().ok()).unwrap_or(3000);
        slint::Timer::single_shot(Duration::from_millis(wait), move || {
            // essai de l'enregistrement (vignette comprise) : ARKLIGHT_SMOKE_SAVE=chemin.json
            if let Some(path) = std::env::var_os("ARKLIGHT_SMOKE_SAVE").map(PathBuf::from) {
                let mut st = state.borrow_mut();
                st.doc.path = Some(path);
                save(&mut st, false);
            }
            if let Some(app) = app_weak.upgrade() {
                match app.window().take_snapshot() {
                    Ok(buf) => {
                        if let Err(e) = save_png(&out, buf.width(), buf.height(), buf.as_bytes()) {
                            eprintln!("capture : {e}");
                        }
                    }
                    Err(e) => eprintln!("capture impossible : {e}"),
                }
            }
            let _ = slint::quit_event_loop();
        });
    }

    // halo comme à la dernière fermeture
    if !settings.borrow().panels.bloom {
        app.global::<App>().invoke_set_flag("bloom".into(), false);
    }
    // fenêtre agrandie à tout l'écran au lancement ; les essais automatiques gardent la taille par
    // défaut, pour des captures comparables (ARKLIGHT_SMOKE_MAXIMIZE=1 : essai de l'agrandissement)
    if !smoke || std::env::var_os("ARKLIGHT_SMOKE_MAXIMIZE").is_some() {
        ensure_maximized(app.as_weak(), Maximize::default());
    } else if let Some((w, h)) = std::env::var("ARKLIGHT_SMOKE_SIZE").ok().and_then(|s| s.split_once('x').and_then(|(w, h)| Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?)))) {
        // essai : taille de fenêtre choisie (ARKLIGHT_SMOKE_SIZE=1600x1800), pour voir tout l'inspecteur
        app.window().set_size(slint::LogicalSize::new(w, h));
    }
    app.run()?;
    // fermeture : pont Bluetooth arrêté et état libéré tant que ses fils existent encore
    state.borrow().ble.stop();
    ai_panel::uninstall();
    Ok(())
}

/// Agrandit la fenêtre à tout l'écran au lancement. Windows l'agrandit, mais Slint peut ensuite la
/// remettre à sa taille préférée (1280 × 760) quand sa propre demande de taille arrive après :
/// la fenêtre reste alors marquée « agrandie », en haut à gauche, sans couvrir l'écran. On regarde
/// donc toutes les 150 ms si elle couvre l'écran ; sinon on la rétablit puis on l'agrandit de
/// nouveau (3 fois au plus : jamais de clignotement). Fini quand elle le couvre deux fois de suite.
fn ensure_maximized(app_weak: slint::Weak<AppWindow>, mut m: Maximize) {
    use slint::winit_030::WinitWindowAccessor;
    let Some(app) = app_weak.upgrade() else { return };
    let full = app.window().with_winit_window(|w| {
        let size = w.inner_size();
        let full = w.is_maximized()
            && w.current_monitor().is_none_or(|s| {
                let screen = s.size();
                // la barre des tâches et la barre de titre prennent un peu de hauteur
                size.width as f32 >= screen.width as f32 * 0.85 && size.height as f32 >= screen.height as f32 * 0.75
            });
        if std::env::var_os("ARKLIGHT_SMOKE_GEOM").is_some() {
            eprintln!("agrandir ({}) : agrandie {} · {size:?} · écran {:?}", m.attempts, w.is_maximized(), w.current_monitor().map(|s| s.size()));
        }
        if !full && m.attempts < 3 {
            if w.is_maximized() {
                w.set_maximized(false);
            }
            w.set_maximized(true);
            m.attempts += 1;
        }
        full
    });
    let again = match full {
        // fenêtre pas encore créée par la boucle d'événements (lente au premier lancement) : 30 s au plus
        None => {
            m.waits += 1;
            m.waits < 200
        }
        Some(true) => {
            m.good += 1;
            m.good < 2
        }
        Some(false) => {
            m.good = 0;
            m.attempts <= 3
        }
    };
    if again {
        slint::Timer::single_shot(Duration::from_millis(150), move || ensure_maximized(app_weak, m));
    }
}

/// Où en est l'agrandissement du lancement ([`ensure_maximized`]).
#[derive(Clone, Copy, Default)]
struct Maximize {
    /// passages sans fenêtre
    waits: u32,
    /// agrandissements demandés
    attempts: u32,
    /// passages de suite où elle couvre l'écran
    good: u32,
}

fn save_png(path: &str, width: u32, height: u32, rgba: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    Ok(())
}
