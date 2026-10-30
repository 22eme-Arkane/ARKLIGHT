//! Raccourcis clavier personnalisables (comme `web/js/shortcuts.js`) : touches par défaut des
//! commandes, celles changées par l'utilisateur (seules mémorisées : les commandes d'une version
//! future gardent leurs touches par défaut), noms des touches.

use crate::tr;
use std::collections::BTreeMap;

/// Touches par défaut (« Ctrl+Shift+Z ») des commandes (`commands::COMMANDS`).
pub const DEFAULTS: &[(&str, &[&str])] = &[
    ("scene.new", &["Ctrl+N"]),
    ("scene.open", &["Ctrl+O"]),
    ("scene.save", &["Ctrl+S"]),
    ("scene.saveAs", &["Ctrl+Shift+S"]),
    ("scene.rename", &["F2"]),
    ("plot.print", &["Ctrl+P"]),
    ("plan.next", &["Ctrl+PageDown"]),
    ("plan.prev", &["Ctrl+PageUp"]),
    ("edit.undo", &["Ctrl+Z"]),
    ("edit.redo", &["Ctrl+Y", "Ctrl+Shift+Z"]),
    ("edit.cut", &["Ctrl+X"]),
    ("edit.copy", &["Ctrl+C"]),
    ("edit.paste", &["Ctrl+V"]),
    ("edit.duplicate", &["Ctrl+D"]),
    ("edit.delete", &["Delete"]),
    ("select.all", &["Ctrl+A"]),
    ("select.none", &["Escape"]),
    ("group.create", &["Ctrl+G"]),
    ("tool.translate", &["W"]),
    ("tool.rotate", &["E"]),
    ("snap.toggle", &["M"]),
    ("view.persp", &["1"]),
    ("view.front", &["2"]),
    ("view.top", &["3"]),
    ("view.side", &["4"]),
    ("view.frame", &["F"]),
    ("view.camera", &["C"]),
    ("remote.toggle", &["R"]),
    ("view.highlight", &["H"]),
    ("view.fullscreen", &["F11"]),
    ("menu.focus", &["F10"]),
    ("view.fullview", &["Shift+F11"]),
    ("library.search", &["Ctrl+F"]),
    ("drawer.assistant", &["Ctrl+Shift+A"]),
    ("panel.left", &["Ctrl+B"]),
    ("panel.right", &["Ctrl+Alt+B"]),
    ("panel.bottom", &["Ctrl+J"]),
    ("view.plot", &["P"]),
    ("labels.toggle", &["N"]),
    ("ai.voice", &["F8"]),
    ("shortcuts.open", &["Ctrl+Alt+K", "F1"]),
    ("app.quit", &["Ctrl+Q"]),
    ("fixture.drop", &["End"]),
    ("plans.open", &["Ctrl+Shift+P"]),
    ("scene.openFile", &["Ctrl+Shift+O"]),
    ("select.invert", &["Ctrl+I"]),
];

/// Nom d'une touche frappée (« Ctrl+Shift+Z », « Delete », « F5 ») ; None : touche de modification seule.
pub fn key_name(text: &str, ctrl: bool, shift: bool, alt: bool) -> Option<String> {
    let c = text.chars().next()?;
    let base: String = match c {
        '\u{0008}' => "Backspace".into(),
        '\u{0009}' => "Tab".into(),
        '\u{000a}' | '\u{000d}' => "Enter".into(),
        '\u{001b}' => "Escape".into(),
        '\u{007f}' => "Delete".into(),
        '\u{0020}' => "Space".into(),
        '\u{0010}'..='\u{0018}' => return None,
        '\u{F700}' => "Up".into(),
        '\u{F701}' => "Down".into(),
        '\u{F702}' => "Left".into(),
        '\u{F703}' => "Right".into(),
        '\u{F704}'..='\u{F71B}' => format!("F{}", c as u32 - 0xF704 + 1),
        '\u{F727}' => "Insert".into(),
        '\u{F729}' => "Home".into(),
        '\u{F72B}' => "End".into(),
        '\u{F72C}' => "PageUp".into(),
        '\u{F72D}' => "PageDown".into(),
        c if (c as u32) < 0x20 => {
            // Ctrl+lettre arrive parfois comme caractère de contrôle (Ctrl+A = 0x01)
            char::from_u32(c as u32 + 0x40)?.to_string()
        }
        c if ('\u{F700}'..='\u{F8FF}').contains(&c) => return None,
        // clavier AZERTY : la rangée des chiffres sans Maj (comme la touche physique de la version
        // JavaScript) ; « - » (6) reste le signe moins
        '&' => "1".into(),
        'é' => "2".into(),
        '"' => "3".into(),
        '\'' => "4".into(),
        '(' => "5".into(),
        'è' => "7".into(),
        '_' => "8".into(),
        'ç' => "9".into(),
        'à' => "0".into(),
        c => c.to_uppercase().collect(),
    };
    let mut name = String::new();
    if ctrl {
        name.push_str("Ctrl+");
    }
    if alt {
        name.push_str("Alt+");
    }
    if shift {
        name.push_str("Shift+");
    }
    name.push_str(&base);
    Some(name)
}

// ------------------------------------------------------------------ clavier AZERTY dessiné
/// Touche du clavier dessiné : nom de la touche (None : non attribuable ; « mod:Ctrl »… :
/// modificateur), inscription, inscription du haut, largeur (en touches), espace vide.
pub struct Cap {
    pub token: Option<&'static str>,
    pub cap: &'static str,
    pub sub: &'static str,
    pub w: f32,
    pub gap: bool,
}

const fn k(token: &'static str, cap: &'static str, w: f32, sub: &'static str) -> Cap {
    Cap { token: Some(token), cap, sub, w, gap: false }
}

const fn dead(cap: &'static str, w: f32, sub: &'static str) -> Cap {
    Cap { token: None, cap, sub, w, gap: false }
}

const fn gap(w: f32) -> Cap {
    Cap { token: None, cap: "", sub: "", w, gap: true }
}

/// Rangées du clavier (`MAIN_ROWS` puis `NAV_ROWS` à droite), 18,5 touches de large.
pub fn keyboard() -> Vec<Vec<Cap>> {
    let letters = |s: &'static str| -> Vec<Cap> { s.split("").filter(|c| !c.is_empty()).map(|c| k(c, c, 1.0, "")).collect() };
    let mut main: Vec<Vec<Cap>> = vec![
        {
            let mut r = vec![k("Escape", "Échap", 1.0, ""), gap(1.0)];
            for (i, f) in ["F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12"].into_iter().enumerate() {
                if i == 4 || i == 8 {
                    r.push(gap(0.5));
                }
                r.push(k(f, f, 1.0, ""));
            }
            r
        },
        {
            let mut r = vec![k("²", "²", 1.0, "")];
            for (d, s) in [("1", "&"), ("2", "é"), ("3", "\""), ("4", "'"), ("5", "("), ("6", "-"), ("7", "è"), ("8", "_"), ("9", "ç"), ("0", "à")] {
                r.push(k(d, d, 1.0, s));
            }
            r.extend([k(")", ")", 1.0, "°"), k("=", "=", 1.0, "+"), k("Backspace", "Retour", 2.0, "")]);
            r
        },
        {
            let mut r = vec![k("Tab", "Tab", 1.5, "")];
            r.extend(letters("AZERTYUIOP"));
            r.extend([dead("^", 1.0, "¨"), k("$", "$", 1.0, "£"), k("Enter", "Entrée", 1.5, "")]);
            r
        },
        {
            let mut r = vec![dead("Verr. Maj", 1.75, "")];
            r.extend(letters("QSDFGHJKLM"));
            r.extend([k("Ù", "ù", 1.0, "%"), k("*", "*", 1.0, "µ"), k("Enter", "Entrée", 1.25, "")]);
            r
        },
        {
            let mut r = vec![k("mod:Shift", "Maj", 1.25, ""), k("<", "<", 1.0, ">")];
            r.extend(letters("WXCVBN"));
            r.extend([k(",", ",", 1.0, "?"), k(";", ";", 1.0, "."), k(":", ":", 1.0, "/"), k("!", "!", 1.0, "§"), k("mod:Shift", "Maj", 2.75, "")]);
            r
        },
        vec![k("mod:Ctrl", "Ctrl", 1.5, ""), dead("Win", 1.25, ""), k("mod:Alt", "Alt", 1.25, ""), k("Space", "Espace", 6.5, ""), dead("Alt Gr", 1.25, ""), dead("Menu", 1.25, ""), k("mod:Ctrl", "Ctrl", 2.0, "")],
    ];
    let nav: Vec<Vec<Cap>> = vec![
        vec![gap(3.0)],
        vec![k("Insert", "Inser", 1.0, ""), k("Home", "Début", 1.0, ""), k("PageUp", "Pg ↑", 1.0, "")],
        vec![k("Delete", "Suppr", 1.0, ""), k("End", "Fin", 1.0, ""), k("PageDown", "Pg ↓", 1.0, "")],
        vec![gap(3.0)],
        vec![gap(1.0), k("Up", "↑", 1.0, ""), gap(1.0)],
        vec![k("Left", "←", 1.0, ""), k("Down", "↓", 1.0, ""), k("Right", "→", 1.0, "")],
    ];
    for (row, n) in main.iter_mut().zip(nav) {
        row.push(gap(0.5));
        row.extend(n);
    }
    main
}

/// Inscription d'une touche dans la langue de l'interface (le clavier reste AZERTY).
pub fn cap_label(cap: &'static str) -> &'static str {
    let en = match cap {
        "Échap" => "Esc",
        "Entrée" => "Enter",
        "Espace" => "Space",
        "Maj" => "Shift",
        "Inser" => "Ins",
        "Début" => "Home",
        "Suppr" => "Del",
        "Fin" => "End",
        "Retour" => "Backspace",
        "Verr. Maj" => "Caps Lock",
        _ => cap,
    };
    tr(cap, en)
}

/// Combinaison montrée par une touche selon les modificateurs choisis (`visualCombo`) ; None :
/// modificateur, touche non attribuable, ou caractère changé par Maj.
pub fn visual_combo(token: &str, ctrl: bool, alt: bool, shift: bool) -> Option<String> {
    if token.starts_with("mod:") {
        return None;
    }
    let digit = token.len() == 1 && token.chars().all(|c| c.is_ascii_digit());
    let punct = token.chars().count() == 1 && !token.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    if shift && punct {
        return None;
    }
    let mut s = String::new();
    if ctrl {
        s.push_str("Ctrl+");
    }
    if alt {
        s.push_str("Alt+");
    }
    if shift && !digit {
        s.push_str("Shift+");
    }
    s.push_str(token);
    Some(s)
}

/// Nom affiché d'une touche, dans la langue de l'interface (« Maj+Suppr »).
pub fn display(key: &str) -> String {
    key.split('+')
        .map(|p| match p {
            "Shift" => tr("Maj", "Shift"),
            "Delete" => tr("Suppr", "Del"),
            "Escape" => tr("Échap", "Esc"),
            "Enter" => tr("Entrée", "Enter"),
            "Space" => tr("Espace", "Space"),
            "Backspace" => tr("Retour arr.", "Backspace"),
            "PageUp" => tr("Pg préc", "PgUp"),
            "PageDown" => tr("Pg suiv", "PgDn"),
            "Home" => tr("Début", "Home"),
            "End" => tr("Fin", "End"),
            "Insert" => tr("Inser", "Ins"),
            "Up" => "↑",
            "Down" => "↓",
            "Left" => "←",
            "Right" => "→",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Touches des commandes changées par l'utilisateur (le reste : `DEFAULTS`).
pub type Overrides = BTreeMap<String, Vec<String>>;

/// Touches d'une commande.
pub fn keys_of(o: &Overrides, id: &str) -> Vec<String> {
    match o.get(id) {
        Some(k) => k.clone(),
        None => DEFAULTS.iter().find(|d| d.0 == id).map(|d| d.1.iter().map(|s| s.to_string()).collect()).unwrap_or_default(),
    }
}

/// Commande d'une touche.
pub fn command_for(o: &Overrides, key: &str) -> Option<&'static str> {
    crate::commands::COMMANDS.iter().map(|c| c.id).find(|id| keys_of(o, id).iter().any(|k| k == key))
}

/// Donne `key` à la commande `id` (en plus des siennes) ; la retire à celle qui l'avait.
/// Renvoie la commande qui la perd.
pub fn assign(o: &mut Overrides, id: &str, key: &str) -> Option<&'static str> {
    let previous = command_for(o, key).filter(|p| *p != id);
    if let Some(p) = previous {
        let keys: Vec<String> = keys_of(o, p).into_iter().filter(|k| k != key).collect();
        o.insert(p.to_owned(), keys);
    }
    let mut keys = keys_of(o, id);
    if !keys.iter().any(|k| k == key) {
        keys.push(key.to_owned());
    }
    o.insert(id.to_owned(), keys);
    previous
}

/// Revient aux touches par défaut (sans prendre celles réattribuées ailleurs).
pub fn reset(o: &mut Overrides, id: &str) {
    o.remove(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touches() {
        assert_eq!(key_name("z", true, false, false).as_deref(), Some("Ctrl+Z"));
        assert_eq!(key_name("Z", true, true, false).as_deref(), Some("Ctrl+Shift+Z"));
        assert_eq!(key_name("\u{F72D}", true, false, false).as_deref(), Some("Ctrl+PageDown"));
        assert_eq!(key_name("\u{0011}", true, false, false), None);
        let mut o = Overrides::new();
        assert_eq!(command_for(&o, "Ctrl+Shift+Z"), Some("edit.redo"));
        assert_eq!(assign(&mut o, "view.blackout", "F"), Some("view.frame"));
        assert_eq!(command_for(&o, "F"), Some("view.blackout"));
        assert!(keys_of(&o, "view.frame").is_empty());
        reset(&mut o, "view.frame");
        assert_eq!(keys_of(&o, "view.frame"), vec!["F".to_string()]);
    }
}
