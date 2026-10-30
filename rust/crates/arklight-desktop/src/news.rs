//! Nouveautés d'ARKLIGHT (comme `core/release_notes.py` de PANDORA) : une section
//! « ## X.Y.Z · AAAA-MM-JJ » par version dans `assets/notes/nouveautes_fr.md` et
//! `whats_new_en.md`, la plus récente en tête. À CHAQUE nouvelle version : écrire l'entrée FR et EN.
//! La fenêtre s'ouvre au lancement tant que « Ne plus afficher jusqu'à la prochaine version »
//! n'est pas cochée pour cette version.

use crate::{NewsEntry, NewsLine, tr};
use slint::{ModelRc, VecModel};

const FR: &str = include_str!("../assets/notes/nouveautes_fr.md");
const EN: &str = include_str!("../assets/notes/whats_new_en.md");

/// Une version : (numéro, date AAAA-MM-JJ, corps en Markdown).
pub fn parse(text: &str) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for line in text.lines() {
        if let Some(head) = line.strip_prefix("## ")
            && let Some((v, d)) = head.split_once(" · ")
            && v.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            out.push((v.trim().to_owned(), d.trim().to_owned(), String::new()));
        } else if let Some(last) = out.last_mut() {
            last.2.push_str(line);
            last.2.push('\n');
        }
    }
    out
}

/// Paragraphes et puces d'un corps Markdown simple (gras retiré : la fenêtre n'a qu'un style par
/// ligne) ; « lead » : un paragraphe tout en gras (le chapeau de la version).
pub fn lines(body: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut cur: Option<(String, String)> = None;
    let flush = |cur: &mut Option<(String, String)>, out: &mut Vec<(String, String)>| {
        if let Some((kind, text)) = cur.take() {
            let t = text.trim();
            let lead = kind == "p" && t.starts_with("**") && t.ends_with("**") && t.matches("**").count() == 2;
            out.push((if lead { "lead".into() } else { kind }, t.replace("**", "")));
        }
    };
    for line in body.lines() {
        let l = line.trim_end();
        if l.trim().is_empty() || l.trim_start().starts_with("<!--") {
            flush(&mut cur, &mut out);
        } else if let Some(item) = l.strip_prefix("- ") {
            flush(&mut cur, &mut out);
            cur = Some(("li".into(), item.to_owned()));
        } else if let Some((_, t)) = cur.as_mut() {
            t.push(' ');
            t.push_str(l.trim());
        } else {
            cur = Some(("p".into(), l.trim().to_owned()));
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// Date lisible (« 30 octobre 2026 », « October 30, 2026 »).
pub fn date_text(iso: &str) -> String {
    const FR_M: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
    const EN_M: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let p: Vec<u32> = iso.split('-').filter_map(|x| x.parse().ok()).collect();
    let [y, m, d] = p[..] else { return iso.to_owned() };
    let Some(i) = (m as usize).checked_sub(1).filter(|i| *i < 12) else { return iso.to_owned() };
    if tr("fr", "en") == "en" { format!("{} {d}, {y}", EN_M[i]) } else { format!("{} {} {y}", if d == 1 { "1er".to_owned() } else { d.to_string() }, FR_M[i]) }
}

/// Les nouveautés dans la langue de l'interface, pour la fenêtre.
pub fn entries() -> ModelRc<NewsEntry> {
    let list: Vec<NewsEntry> = parse(if tr("fr", "en") == "en" { EN } else { FR })
        .into_iter()
        .map(|(v, d, body)| NewsEntry {
            version: v.into(),
            date: date_text(&d).into(),
            lines: ModelRc::new(VecModel::from(lines(&body).into_iter().map(|(kind, text)| NewsLine { kind: kind.into(), text: text.into() }).collect::<Vec<_>>())),
        })
        .collect();
    ModelRc::new(VecModel::from(list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_et_lignes() {
        for text in [FR, EN] {
            let v = parse(text);
            assert_eq!(v.first().map(|e| e.0.as_str()), Some(env!("CARGO_PKG_VERSION")), "la version en cours en tête des nouveautés");
            assert!(v.len() >= 2);
            let l = lines(&v[0].2);
            assert_eq!(l[0].0, "lead");
            assert!(l.iter().filter(|x| x.0 == "li").count() >= 5);
            assert!(l.iter().all(|x| !x.1.contains("**")));
        }
        assert_eq!(date_text("2026-10-30"), "30 octobre 2026");
        assert_eq!(date_text("2026-10-01"), "1er octobre 2026");
    }
}
