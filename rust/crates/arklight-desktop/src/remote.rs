//! Télécommandes infrarouges Astera ARC1 (« FX Remote ») et ARC3 (« White Remote ») : panneau
//! flottant qui vise les projecteurs choisis ou tous les Astera (comme `web/js/ui/remote.js`) ;
//! la logique des touches est celle d'`arklight_onboard::ir_remote`.

use crate::{RemoteKey, State, onboard_panel, tr};
use arklight_core::document::Edit;
use arklight_core::library::library;
use arklight_onboard::ir_remote::{ARC1_KEYS, ARC3_KEYS, apply_ir_key};

fn rgb(hex: u32) -> slint::Color {
    slint::Color::from_rgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Couleur de l'anneau d'une touche de l'ARC1 (photos officielles).
fn arc1_ring(key: &str) -> u32 {
    match key {
        "RED" => 0xe53935,
        "ORANGE" => 0xfb8c00,
        "YELLOW" => 0xfdd835,
        "GREEN" => 0x43a047,
        "CYAN" => 0x00acc1,
        "BLUE" => 0x3949ab,
        "PINK" => 0xec407a,
        "MAGENTA" => 0x8e24aa,
        "WARM WHITE" => 0xfbc02d,
        _ => 0xd8dce0,
    }
}

/// Fond d'une touche de l'ARC3.
fn arc3_fill(key: &str) -> u32 {
    match key {
        "2400 K" => 0xd98a2b,
        "2700 K" => 0xe3a24a,
        "3000 K" => 0xe8b765,
        "3200 K" => 0xeec986,
        "3500 K" => 0xf2d9a6,
        "4000 K" => 0xf5e6c6,
        "4500 K" => 0xf6eedc,
        "5000 K" => 0xecedee,
        "5500 K" => 0xe3e6e9,
        "6000 K" => 0xd7dde3,
        "6500 K" => 0xc7d3df,
        "10000 K" => 0xa9c8ea,
        "BLUE MODE 1/2" | "BLUE MODE 2/2" => 0x2b2f7a,
        "MAX RUNTIME" => 0xd32f2f,
        _ => 0xf4f5f6,
    }
}

/// Touches de la télécommande (`arc3` : ARC3, sinon ARC1), 4 × 7 dans l'ordre de lecture.
pub fn keys(arc3: bool) -> Vec<RemoteKey> {
    let plan = if arc3 { ARC3_KEYS } else { ARC1_KEYS };
    plan.iter()
        .flatten()
        .map(|k| match k {
            None => RemoteKey { empty: true, ..RemoteKey::default() },
            Some(id) => {
                let label = if arc3 {
                    id.to_string()
                } else {
                    match *id {
                        "SPEED4" => "↑ 4".into(),
                        "SPEED3" => "↑ 3".into(),
                        "SPEED2" => "↑ 2".into(),
                        "SPEED1" => "↑ 1".into(),
                        "STOP" => "■".into(),
                        "RUN5" => "5h".into(),
                        "RUN8" => "8h".into(),
                        "RUN12" => "12h".into(),
                        other => other.to_string(),
                    }
                };
                let dark_text = arc3 && !(id.contains("BLUE") || id.contains("MAX"));
                RemoteKey {
                    id: (*id).into(),
                    label: label.into(),
                    ring: rgb(if arc3 { 0xb7bcc2 } else { arc1_ring(id) }),
                    fill: rgb(if arc3 { arc3_fill(id) } else { 0x08090a }),
                    ink: rgb(if dark_text { 0x222222 } else { 0xffffff }),
                    empty: false,
                }
            }
        })
        .collect()
}

/// Une touche pressée : appliquée à chaque projecteur visé (choisis, ou tous les Astera) ;
/// renvoie les modifications et le compte rendu (« POWER ON → Titan 1 : OK · … »).
pub fn press(st: &mut State, key: &str, all: bool) -> (Option<Edit>, String) {
    let lib = library();
    let ids: Vec<String> = if all {
        st.doc.project.fixtures.iter().filter(|f| lib.fixture(&f.type_id).is_some_and(|t| t.is_astera_v5())).map(|f| f.id.clone()).collect()
    } else {
        st.selection.iter().filter(|id| st.doc.fixture(id).is_some()).cloned().collect()
    };
    if ids.is_empty() {
        return (None, tr("Vise un projecteur : sélectionne-le dans la vue 3D", "Aim at a fixture: select it in the 3D view").into());
    }
    let now = st.clock.elapsed().as_secs_f64();
    let mut outcomes = Vec::new();
    let mut results = Vec::new();
    for id in &ids {
        let Some(f) = st.doc.fixture(id) else { continue };
        let Some(ty) = lib.fixture(&f.type_id) else { continue };
        let rt = st.lights.runtime_mut(id);
        let ext = st.onboard.astera(id);
        let r = apply_ir_key(f, ty, rt, ext, key, now);
        let what = if r.accepted { "OK".to_owned() } else { tr("ignoré", "ignored").to_owned() };
        results.push(format!("{} : {what}{}", f.name, r.note.map(|n| format!(" ({n})")).unwrap_or_default()));
        if let Some(out) = r.changes {
            outcomes.push((id.clone(), out));
        }
    }
    let edits: Vec<Edit> = outcomes.into_iter().filter_map(|(id, out)| onboard_panel::outcome_edit(st, &id, out)).collect();
    ((!edits.is_empty()).then_some(Edit::Batch(edits)), format!("{key} → {}", results.join(" · ")))
}
