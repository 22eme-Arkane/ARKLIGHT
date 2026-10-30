//! Liste Scène du panneau de gauche (comme `web/js/ui/scene-list.js`) : groupes, projecteurs
//! (pastille de leur couleur, groupes, patch et ses conflits), objets et décor (repliables) ;
//! recherche (nom, modèle, patch, Fix, groupes) et tri.

use crate::{SceneRow, model_name, swatch, tr};
use arklight_core::library;
use arklight_core::addressing::range_of;
use arklight_core::engine::console::console_held;
use arklight_core::Fixture;
use arklight_core::document::Document;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;

/// Choix gardés d'une session à l'autre : tri, objets repliés.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub sort: usize,
    #[serde(default)]
    pub objects_folded: bool,
}

/// Minuscules sans accents (recherche).
pub fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'ÿ' => 'y',
            c => c,
        })
        .collect()
}

fn by_name(a: &Fixture, b: &Fixture) -> Ordering {
    norm(&a.name).cmp(&norm(&b.name))
}

/// Problèmes de patch d'un projecteur (`patchIssues`) : Fix en double, dépassement, chevauchements.
pub fn issues(f: &Fixture, all: &[Fixture]) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(n) = f.settings.fixture_number.filter(|n| *n > 0) {
        for o in all.iter().filter(|o| o.id != f.id && o.settings.fixture_number == Some(n)) {
            out.push(tr_owned(format!("Fix {n} aussi donné à « {} »", o.name), format!("Fix {n} also given to “{}”", o.name)));
        }
    }
    let a = range_of(f);
    if a.footprint == 0 {
        return out;
    }
    if a.end > 512 {
        out.push(format!("{} {})", tr("dépasse le canal 512 (fin à", "goes past channel 512 (ends at"), a.end));
    }
    for o in all.iter().filter(|o| o.id != f.id) {
        let b = range_of(o);
        if b.footprint == 0 || b.universe != a.universe {
            continue;
        }
        if a.start <= b.end && b.start <= a.end {
            out.push(format!("{} « {} »", tr("chevauche", "overlaps"), o.name));
        }
    }
    out
}

fn tr_owned(fr: String, en: String) -> String {
    if tr("fr", "en") == "en" { en } else { fr }
}

/// Lignes de la liste et texte du compteur (« (24 · 3 sélectionnés) »).
pub fn rows(doc: &Document, selection: &[String], color: &dyn Fn(&str) -> [f64; 3], search: &str, p: &Prefs) -> (Vec<SceneRow>, String) {
    let project = &doc.project;
    let all = &project.fixtures;
    let q = norm(search.trim());
    let groups_of = |id: &str| project.groups.iter().filter(|g| g.ids.iter().any(|x| x == id)).map(|g| g.name.clone()).collect::<Vec<_>>();
    let matches = |f: &Fixture| {
        let t = library().fixture(&f.type_id);
        let model = t.and_then(|t| t.extra.get("model")).and_then(Value::as_str).unwrap_or("");
        let fix = f.settings.fixture_number.map(|n| format!("fix {n}")).unwrap_or_default();
        let text = [
            f.name.clone(),
            t.map(|t| t.brand.clone()).unwrap_or_default(),
            t.map(|t| t.name.clone()).unwrap_or_default(),
            model.to_owned(),
            arklight_plot::catalog::patch_label(f),
            format!("u{}", f.settings.universe.unwrap_or(1)),
            fix,
            groups_of(&f.id).join(" "),
        ]
        .join(" ");
        norm(&text).contains(&q)
    };
    let mut list: Vec<&Fixture> = all.iter().filter(|f| q.is_empty() || matches(f)).collect();
    match p.sort {
        1 => list.sort_by(|a, b| by_name(a, b)),
        2 => list.sort_by(|a, b| {
            let (ra, rb) = (range_of(a), range_of(b));
            ra.universe.cmp(&rb.universe).then(ra.start.cmp(&rb.start)).then_with(|| by_name(a, b))
        }),
        3 => list.sort_by(|a, b| norm(&model_name(&a.type_id)).cmp(&norm(&model_name(&b.type_id))).then_with(|| by_name(a, b))),
        4 => list.sort_by(|a, b| {
            let n = |f: &Fixture| f.settings.fixture_number.filter(|n| *n > 0).unwrap_or(u32::MAX);
            n(a).cmp(&n(b)).then_with(|| by_name(a, b))
        }),
        _ => {}
    }
    let selected = |id: &str| selection.iter().any(|s| s == id);
    let multi = selection.len() > 1;
    let mut out = Vec::new();
    let section = |label: &str| SceneRow { kind: "section".into(), name: label.into(), ..Default::default() };

    let groups: Vec<_> = project.groups.iter().filter(|g| q.is_empty() || norm(&g.name).contains(&q)).collect();
    if !groups.is_empty() {
        out.push(section(tr("Groupes", "Groups")));
        for g in groups {
            let members: Vec<_> = g.ids.iter().filter(|id| all.iter().any(|f| f.id == **id)).collect();
            let n_sel = members.iter().filter(|id| selected(id)).count();
            out.push(SceneRow {
                kind: "group".into(),
                id: g.id.as_str().into(),
                name: g.name.as_str().into(),
                count: members.len() as i32,
                selected: !members.is_empty() && n_sel == members.len(),
                partial: n_sel > 0 && n_sel < members.len(),
                ..Default::default()
            });
        }
        out.push(section(tr("Projecteurs", "Fixtures")));
    }
    if all.is_empty() {
        out.push(SceneRow {
            kind: "empty".into(),
            name: tr("Glisse un projecteur de la bibliothèque dans la vue 3D, ou clique sur son « + ».", "Drag a fixture from the library into the 3D view, or click its “+”.").into(),
            ..Default::default()
        });
    } else if list.is_empty() {
        out.push(SceneRow { kind: "empty".into(), name: tr("Aucun projecteur ne correspond à la recherche.", "No fixture matches the search.").into(), ..Default::default() });
    }
    for f in list {
        let rgb = color(&f.id);
        let problems = issues(f, all);
        out.push(SceneRow {
            kind: "fixture".into(),
            id: f.id.as_str().into(),
            name: f.name.as_str().into(),
            sub: groups_of(&f.id).join(" · ").into(),
            patch: format!("{}{}", if problems.is_empty() { "" } else { "⚠ " }, arklight_plot::catalog::patch_label(f)).into(),
            warn: !problems.is_empty(),
            tip: problems.join("\n").into(),
            swatch: swatch(rgb),
            lit: rgb.iter().any(|v| *v > 1e-4),
            off: f.state.get("power").and_then(Value::as_bool) == Some(false),
            held: console_held(f),
            selected: selected(&f.id),
            primary: multi && selection.first().is_some_and(|s| *s == f.id),
            ..Default::default()
        });
    }

    let objects: Vec<_> = project
        .objects
        .iter()
        .filter(|o| {
            let lib = library();
            let (name, short) = match (lib.prop(&o.type_id), lib.grip(&o.type_id)) {
                (Some(t), _) => (t.name.clone(), t.short.clone().unwrap_or_default()),
                (None, Some(g)) => (g.name.clone(), g.short.clone().unwrap_or_default()),
                _ => (String::new(), String::new()),
            };
            q.is_empty() || norm(&format!("{} {name} {short}", o.name)).contains(&q)
        })
        .collect();
    if !objects.is_empty() {
        let folded = p.objects_folded && q.is_empty();
        out.push(SceneRow {
            kind: "fold".into(),
            name: tr_owned(format!("Objets et décor ({})", objects.len()), format!("Objects and set dressing ({})", objects.len())).into(),
            folded,
            ..Default::default()
        });
        if !folded {
            for o in objects {
                let lib = library();
                let (icon, short) = match (lib.prop(&o.type_id), lib.grip(&o.type_id)) {
                    (Some(t), _) => (format!("prop-{}", t.category.as_deref().unwrap_or("props")), t.short.clone().unwrap_or_default()),
                    (None, Some(g)) => (format!("grip-{}", g.family.as_deref().unwrap_or("box")), "grip".to_owned()),
                    _ => (String::new(), String::new()),
                };
                out.push(SceneRow {
                    kind: "object".into(),
                    id: o.id.as_str().into(),
                    name: o.name.as_str().into(),
                    sub: short.into(),
                    icon: icon.into(),
                    object: true,
                    selected: selected(&o.id),
                    ..Default::default()
                });
            }
        }
    }
    let count = if all.is_empty() {
        String::new()
    } else if selection.len() > 1 {
        tr_owned(format!("({} · {} sélectionnés)", all.len(), selection.len()), format!("({} · {} selected)", all.len(), selection.len()))
    } else {
        format!("({})", all.len())
    };
    (out, count)
}
