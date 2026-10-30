//! Textes produits en français par les modules communs (MIDI, plan 2D, menus des projecteurs…),
//! traduits avec le dictionnaire de la version JavaScript (`shared/i18n/en.js`, exporté par
//! `tools/rust/export-i18n.mjs`). Les textes à trous (« fader {0} de la console ») sont reconnus
//! même une fois remplis.

use regex::Regex;
use serde_json::{Map, Value};
use std::sync::OnceLock;

struct Dict {
    exact: Map<String, Value>,
    /// textes à trous : motif, nom des trous dans l'ordre, traduction
    templates: Vec<(Regex, Vec<String>, String)>,
}

fn dict() -> &'static Dict {
    static D: OnceLock<Dict> = OnceLock::new();
    D.get_or_init(|| {
        let exact: Map<String, Value> = serde_json::from_str(include_str!("../data/en.json")).unwrap_or_default();
        let hole = Regex::new(r"\{(\w+)\}").expect("motif");
        let mut templates = Vec::new();
        for (fr, en) in &exact {
            let Some(en) = en.as_str() else { continue };
            if !hole.is_match(fr) {
                continue;
            }
            let mut pattern = String::from("^");
            let mut names = Vec::new();
            let mut last = 0;
            for m in hole.captures_iter(fr) {
                let all = m.get(0).expect("trou");
                pattern.push_str(&regex::escape(&fr[last..all.start()]));
                pattern.push_str("(.+?)");
                names.push(m[1].to_owned());
                last = all.end();
            }
            pattern.push_str(&regex::escape(&fr[last..]));
            pattern.push('$');
            if let Ok(re) = Regex::new(&pattern) {
                templates.push((re, names, en.to_owned()));
            }
        }
        // les motifs les plus longs d'abord (les plus précis)
        templates.sort_by_key(|t| std::cmp::Reverse(t.0.as_str().len()));
        Dict { exact, templates }
    })
}

/// Traduction anglaise d'un texte français (le texte lui-même s'il est inconnu).
pub fn english(fr: &str) -> String {
    let d = dict();
    if let Some(en) = d.exact.get(fr).and_then(Value::as_str) {
        return en.to_owned();
    }
    for (re, names, en) in &d.templates {
        if let Some(c) = re.captures(fr) {
            let mut out = en.clone();
            for (i, name) in names.iter().enumerate() {
                let part = english(&c[i + 1]);
                out = out.replace(&format!("{{{name}}}"), &part);
            }
            return out;
        }
    }
    fr.to_owned()
}

/// Texte dans la langue de l'interface.
pub fn t(fr: &str) -> String {
    if crate::tr("fr", "en") == "en" { english(fr) } else { fr.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::english;

    #[test]
    fn exact_et_a_trous() {
        assert_eq!(english("Fader de la console"), "Console fader");
        assert_eq!(english("canal 3"), "channel 3");
        assert_eq!(english("texte inconnu"), "texte inconnu");
    }
}
