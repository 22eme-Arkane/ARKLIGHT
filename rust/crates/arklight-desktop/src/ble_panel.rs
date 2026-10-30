//! Onglet « Bluetooth » : le PC se fait passer pour un appareil Bluetooth LE (fausse AsteraBox,
//! profil de test « écho ») grâce à `arklight-ble`. Rien n'est annoncé tant qu'on n'a pas cliqué
//! « Rendre le PC visible » ; les réglages Bluetooth de Windows ne sont jamais modifiés.

use crate::ai_panel::on_ui;
use crate::{AppWindow, BleLine, State, tr};
use arklight_ble::{Direction, Hub, HubEvent};
use slint::{ModelRc, SharedString, VecModel};
use std::sync::Arc;
use slint::ComponentHandle as _;

/// Le hub, et l'écoute de ses événements (fil de l'interface).
pub fn new_hub() -> Arc<Hub> {
    let hub = Arc::new(Hub::new());
    hub.set_listener(|ev: &HubEvent| {
        if matches!(ev, HubEvent::Status | HubEvent::Traffic(_) | HubEvent::Log(_)) {
            on_ui(|app, st| show(app, st));
        }
    });
    hub
}

/// Montre l'état du pont dans l'onglet.
pub fn show(app: &AppWindow, st: &State) {
    let s = st.ble.status();
    let names: Vec<SharedString> = s.emulators.iter().map(|e| SharedString::from(if e.ready { e.name.clone() } else { format!("{} — {}", e.name, tr("en attente des captures", "awaiting captures")) })).collect();
    app.global::<crate::App>().set_ble_emulators(ModelRc::new(VecModel::from(names)));
    app.global::<crate::App>().set_ble_emulator_index(s.emulators.iter().position(|e| e.id == s.emulator).unwrap_or(0) as i32);
    app.global::<crate::App>().set_ble_note(s.emulators.iter().find(|e| e.id == s.emulator).map(|e| crate::i18n::t(&e.note)).unwrap_or_default().into());
    app.global::<crate::App>().set_ble_running(s.running);
    let adapter = match &s.adapter {
        None => tr("Adaptateur pas encore lu (« Relire l’adaptateur »).", "Adapter not read yet (“Read the adapter”).").to_owned(),
        Some(a) if !a.present => tr("Aucun adaptateur Bluetooth dans cet ordinateur.", "No Bluetooth adapter in this computer.").to_owned(),
        Some(a) => {
            let role = if a.peripheral_role { tr("rôle périphérique : oui", "peripheral role: yes") } else { tr("rôle périphérique : non (il faudra un ESP32)", "peripheral role: no (an ESP32 will be needed)") };
            let radio = if a.radio_on {
                tr("Bluetooth activé", "Bluetooth on").to_owned()
            } else {
                format!("{} ({}) — {}", tr("Bluetooth", "Bluetooth"), crate::i18n::t(&a.radio), tr("active-le dans Paramètres › Bluetooth et appareils", "turn it on in Settings › Bluetooth & devices"))
            };
            format!("{} {} · {role} · {radio}", tr("Adaptateur", "Adapter"), a.address.clone().unwrap_or_default())
        }
    };
    let mut status = adapter;
    if s.running {
        let adv: Vec<String> = s.advertising.values().map(|v| crate::i18n::t(v)).collect();
        status.push_str(&format!(
            " · {} « {} » ({} {}) · {} · {} {}",
            tr("profil", "profile"),
            s.published.clone().unwrap_or_default(),
            s.services,
            tr("service(s)", "service(s)"),
            if adv.is_empty() { tr("annonce…", "advertising…").to_owned() } else { adv.join(", ") },
            s.phones.len(),
            tr("téléphone(s) abonné(s)", "phone(s) subscribed")
        ));
    }
    app.global::<crate::App>().set_ble_status(status.into());
    let mut lines: Vec<BleLine> = st
        .ble
        .traffic()
        .iter()
        .rev()
        .take(200)
        .map(|t| BleLine {
            text: format!("{} {:?} {} {}", if t.dir == Direction::In { "→" } else { "←" }, t.op, t.char.split('-').next().unwrap_or(""), t.hex()).into(),
            input: t.dir == Direction::In,
        })
        .collect();
    lines.extend(st.ble.messages().iter().rev().take(20).map(|m| BleLine { text: crate::i18n::t(&m.text).into(), input: false }));
    app.global::<crate::App>().set_ble_lines(ModelRc::new(VecModel::from(lines)));
}

/// Gestes de l'onglet : « start », « stop », « adapter », « emulator » (rang), « clear ».
pub fn action(app: &AppWindow, st: &mut State, what: &str, arg: &str) {
    let hub = st.ble.clone();
    match what {
        "start" => {
            // quelques centaines de millisecondes (Windows) : hors du fil de l'interface
            std::thread::spawn(move || {
                if let Err(e) = hub.start() {
                    let msg = e.to_string();
                    on_ui(move |app, st| {
                        crate::toast(app, st, &crate::i18n::t(&msg));
                        show(app, st);
                    });
                }
            });
        }
        "stop" => hub.stop(),
        "adapter" => {
            std::thread::spawn(move || {
                let _ = hub.refresh_adapter();
                on_ui(|app, st| show(app, st));
            });
        }
        "emulator" => {
            if let Some(e) = hub.emulators().get(arg.parse::<usize>().unwrap_or(0)) {
                let _ = hub.set_emulator(&e.id);
            }
        }
        "clear" => hub.clear_traffic(),
        _ => {}
    }
    show(app, st);
}
