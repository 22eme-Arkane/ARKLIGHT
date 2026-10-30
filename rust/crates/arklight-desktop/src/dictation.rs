//! Dictée par ORAKLE dans tous les champs de texte (micro de chaque champ, global Slint
//! `Dictation`) et commande vocale de l'assistant (touche F8 maintenue : la demande part
//! aussitôt). Un clic sur le micro = ORAKLE en mains libres ; un nouveau clic écrit le texte
//! dans le champ. Les échanges avec ORAKLE passent par un fil à part, dans l'ordre.

use crate::ai_panel::on_ui;
use crate::{AppWindow, Dictation, State, toast, tr};
use slint::ComponentHandle;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Cible spéciale : la demande dictée part tout de suite à l'assistant (F8).
pub const AI_SEND: &str = "ai-send";

/// État de la dictée en cours.
pub struct Dictate {
    /// champ qui écoute (ou `AI_SEND`)
    target: Option<String>,
    /// touche maintenue (F8) : relâcher envoie ; appui bref → mains libres
    held: bool,
    since: Instant,
    tx: Option<mpsc::Sender<Cmd>>,
}

impl Default for Dictate {
    fn default() -> Self {
        Dictate { target: None, held: false, since: Instant::now(), tx: None }
    }
}

enum Cmd {
    Start,
    Stop(String),
    Cancel,
}

/// Raccourci d'ORAKLE lu dans ses réglages (« <ctrl>+1 » → « Ctrl+1 »).
fn orakle_hotkey() -> String {
    let path = std::env::var_os("APPDATA").map(|d| std::path::PathBuf::from(d).join("orakle").join("settings.json"));
    let raw = path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["hotkey"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "<ctrl>+1".into());
    raw.split('+')
        .map(|p| {
            let p = p.trim().trim_start_matches('<').trim_end_matches('>');
            match p {
                "ctrl" | "ctrl_l" | "ctrl_r" => "Ctrl".to_owned(),
                "shift" => tr("Maj", "Shift").to_owned(),
                "alt" | "alt_l" | "alt_gr" => "Alt".to_owned(),
                "cmd" => "Win".to_owned(),
                other => other.to_uppercase(),
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Aide au survol des micros (raccourcis d'ORAKLE et d'ARKLIGHT).
pub fn show_tooltip(app: &AppWindow, f8: &str) {
    let hk = orakle_hotkey();
    let text = format!(
        "{}\n{}\n{}\n{}",
        tr("Dictée ORAKLE", "ORAKLE dictation"),
        tr("Clic : parler (mains libres) · nouveau clic : le texte s’écrit ici", "Click: speak (hands-free) · click again: the text is written here"),
        format!("{} {hk} {} · {} {hk} : {}", tr("Maintenir", "Hold"), tr("pour dicter au curseur", "to dictate at the cursor"), tr("double appui", "double press"), tr("mains libres", "hands-free")),
        format!("{f8} {}", tr("maintenu : parler à l’assistant (la demande part aussitôt)", "held: talk to the assistant (the request is sent at once)")),
    );
    app.global::<Dictation>().set_tooltip(text.into());
}

fn send(st: &mut State, cmd: Cmd) {
    let tx = st.dictate.tx.get_or_insert_with(|| {
        let (tx, rx) = mpsc::channel::<Cmd>();
        std::thread::spawn(move || {
            for cmd in rx {
                match cmd {
                    Cmd::Start => {
                        if let Err(e) = crate::voice::start() {
                            on_ui(move |app, st| {
                                st.dictate.target = None;
                                show(app, st);
                                toast(app, st, &e);
                            });
                        }
                    }
                    Cmd::Stop(target) => {
                        let r = crate::voice::stop();
                        on_ui(move |app, st| deliver(app, st, &target, r));
                    }
                    Cmd::Cancel => crate::voice::cancel(),
                }
            }
        });
        tx
    });
    let _ = tx.send(cmd);
}

fn show(app: &AppWindow, st: &State) {
    app.global::<Dictation>().set_target(st.dictate.target.clone().unwrap_or_default().into());
}

fn start(app: &AppWindow, st: &mut State, target: &str, held: bool) {
    st.dictate.target = Some(target.to_owned());
    st.dictate.held = held;
    st.dictate.since = Instant::now();
    show(app, st);
    toast(app, st, tr("ORAKLE écoute… parle.", "ORAKLE is listening… speak."));
    send(st, Cmd::Start);
}

fn finish(app: &AppWindow, st: &mut State) {
    let Some(target) = st.dictate.target.take() else { return };
    st.dictate.held = false;
    show(app, st);
    toast(app, st, tr("Transcription par ORAKLE…", "ORAKLE is transcribing…"));
    send(st, Cmd::Stop(target));
}

/// Texte dicté : écrit dans son champ, ou envoyé à l'assistant.
fn deliver(app: &AppWindow, st: &mut State, target: &str, r: Result<String, String>) {
    match r {
        Ok(text) if !text.is_empty() => {
            if target == AI_SEND {
                crate::ai_panel::send(app, st, text);
            } else {
                let g = app.global::<Dictation>();
                g.set_result(text.into());
                g.set_result_for(target.into());
                g.set_seq(g.get_seq() + 1);
            }
        }
        Ok(_) => toast(app, st, tr("ORAKLE n’a rien entendu.", "ORAKLE heard nothing.")),
        Err(e) => toast(app, st, &e),
    }
}

/// Clic sur le micro d'un champ : mains libres ; un nouveau clic (sur n'importe quel micro) termine.
pub fn toggle(app: &AppWindow, st: &mut State, field: &str) {
    if st.dictate.target.is_some() {
        finish(app, st);
    } else {
        start(app, st, field, false);
    }
}

/// Touche « Parler à l'assistant » enfoncée (F8) ; en mains libres, termine.
pub fn key_down(app: &AppWindow, st: &mut State) {
    match &st.dictate.target {
        // touche maintenue : répétitions ignorées
        Some(_) if st.dictate.held => {}
        Some(_) => finish(app, st),
        None => {
            if !st.ai.busy {
                start(app, st, AI_SEND, true);
            }
        }
    }
}

/// Touche relâchée : appui bref → mains libres (un nouvel appui envoie), sinon fin de la dictée.
pub fn key_up(app: &AppWindow, st: &mut State) {
    if st.dictate.target.is_none() || !st.dictate.held {
        return;
    }
    if st.dictate.since.elapsed() < Duration::from_millis(300) {
        st.dictate.held = false;
        toast(app, st, tr("Mains libres : parle, puis appuie à nouveau pour envoyer.", "Hands-free: speak, then press again to send."));
        return;
    }
    finish(app, st);
}

/// Dictée abandonnée (bouton « Arrêter » de l'assistant).
pub fn cancel(app: &AppWindow, st: &mut State) {
    if st.dictate.target.take().is_some() {
        show(app, st);
        send(st, Cmd::Cancel);
    }
}
