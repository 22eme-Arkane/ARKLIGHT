//! Bibliothèque du panneau de gauche (comme `web/js/ui/library.js`) : onglets Lumières, Pieds,
//! Grip, Décor ; projecteurs par marque (tungstène et HMI à part), par type, par nom ou par
//! puissance ; marques et catégories repliables (une recherche déplie tout) ; icône du type et
//! badges (HMI, données provisoires ou estimées).

use crate::{tr, LibRow};
use arklight_core::library::{FixtureType, Library};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;

/// Choix gardés d'une session à l'autre : onglet, tri des projecteurs, groupes repliés.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default)]
    pub tab: usize,
    #[serde(default)]
    pub sort: usize,
    #[serde(default)]
    pub folded: Vec<String>,
}

/// Types de projecteurs (ordre et libellés de `CATEGORIES`, shared/fixtures/index.js).
const CATEGORIES: [(&str, &str, &str); 10] = [
    ("tube", "Tubes", "Tubes"),
    ("bar", "Barres", "Bars"),
    ("panel", "Panneaux / Briques", "Panels / Bricks"),
    ("fresnel", "Fresnels", "Fresnels"),
    ("spot", "Spots / COB", "Spots / COB"),
    ("open-face", "Open face", "Open face"),
    ("par", "PAR / Spots", "PAR / Spots"),
    ("multi", "Multi-lampes", "Multi-lamp"),
    ("bulb", "Ampoules", "Bulbs"),
    ("device", "Boîtiers (AsteraBox…)", "Boxes (AsteraBox…)"),
];
/// Icônes de projecteurs (ui/icons/lib/fixture-*.svg) ; les autres types : « panel ».
const FIXTURE_ICONS: [&str; 8] = ["tube", "par", "bulb", "panel", "fresnel", "spot", "open-face", "multi"];
const GRIP_FAMILIES: [&str; 6] = ["frame", "flag", "bounce", "stand", "box", "bag"];
const PROP_CATEGORIES: [&str; 8] = ["people", "animals", "vehicles", "furniture", "set", "outdoor", "props", "filmset"];

/// Libellé d'une catégorie de décor ou d'une famille de grip.
pub fn category_label(cat: &str) -> &'static str {
    match cat {
        "people" => tr("Personnages", "People"),
        "animals" => tr("Animaux", "Animals"),
        "vehicles" => tr("Véhicules", "Vehicles"),
        "furniture" => tr("Mobilier", "Furniture"),
        "set" => tr("Décor et architecture", "Set and architecture"),
        "outdoor" => tr("Extérieur et nature", "Outdoor and nature"),
        "props" => tr("Accessoires", "Props"),
        "filmset" => tr("Plateau (caméra, son…)", "Film set (camera, sound…)"),
        "frame" => tr("Cadres", "Frames"),
        "flag" => tr("Drapeaux", "Flags"),
        "bounce" => tr("Réflecteurs", "Reflectors"),
        "stand" => tr("Pieds seuls", "Stands"),
        "box" => tr("Cubes", "Apple boxes"),
        "bag" => tr("Sacs", "Sandbags"),
        _ => "",
    }
}

fn fixture_category(id: &str) -> &'static str {
    CATEGORIES.iter().find(|c| c.0 == id).map_or("", |c| tr(c.1, c.2))
}

fn traditional_label() -> &'static str {
    tr("Tungstène & HMI", "Tungsten & HMI")
}

/// Marque d'un projecteur dans la bibliothèque ; tungstène et HMI ont leur propre section.
fn group_of(t: &FixtureType) -> &str {
    if matches!(t.technology.as_deref(), Some("tungsten" | "hmi")) { traditional_label() } else { &t.brand }
}

fn lux_of(t: &FixtureType) -> f64 {
    t.output.as_ref().and_then(|o| o.lux_at1m).unwrap_or(0.0)
}

fn extra_str<'a>(t: &'a FixtureType, key: &str) -> Option<&'a str> {
    t.extra.get(key).and_then(Value::as_str)
}

/// Comparaison sans tenir compte de la casse (à la manière d'Intl.Collator).
fn coll(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Nombre lisible (« 12 400 » en français, « 12,400 » en anglais).
fn thousands(n: f64) -> String {
    let digits = format!("{}", n.round() as i64);
    let sep = tr("\u{202f}", ",");
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push_str(sep);
        }
        out.push(c);
    }
    out
}

fn line(kind: &str, id: &str, label: &str) -> LibRow {
    LibRow { kind: kind.into(), id: id.into(), label: label.into(), ..Default::default() }
}

fn fixture_row(t: &FixtureType, brand: bool, lux: bool) -> LibRow {
    let model = extra_str(t, "model").filter(|m| *m != t.name);
    let lx = (lux && lux_of(t) > 0.0).then(|| format!("{} lx", thousands(lux_of(t))));
    let md: Vec<String> = [brand.then(|| t.brand.clone()), model.map(str::to_owned), lx].into_iter().flatten().collect();
    let mut badges = Vec::new();
    if t.extra.get("provisional").and_then(Value::as_bool).unwrap_or(false) {
        badges.push("prov.");
    }
    if t.technology.as_deref() == Some("hmi") {
        badges.push("HMI");
    }
    match extra_str(t, "confidence") {
        Some("vérifié") | None => {}
        Some("estimé") => badges.push("est."),
        Some(_) => badges.push("?"),
    }
    let icon = if FIXTURE_ICONS.contains(&t.category.as_str()) { t.category.as_str() } else { "panel" };
    LibRow {
        kind: "fixture".into(),
        id: t.id.as_str().into(),
        label: t.name.as_str().into(),
        sub: md.join(" · ").into(),
        icon: format!("fixture-{icon}").into(),
        badge: badges.join(" ").into(),
        ..Default::default()
    }
}

/// Lignes de la bibliothèque pour l'onglet et le tri choisis, et le nombre d'éléments de chaque onglet.
pub fn rows(lib: &Library, search: &str, p: &Prefs) -> (Vec<LibRow>, Vec<i32>) {
    let q = search.trim().to_lowercase();
    let has = |text: String| q.is_empty() || text.to_lowercase().contains(&q);
    let lights: Vec<&FixtureType> = lib
        .fixtures
        .iter()
        .filter(|t| has(format!("{} {} {} {}", t.brand, t.name, extra_str(t, "model").unwrap_or(""), t.technology.as_deref().unwrap_or(""))))
        .collect();
    let stands: Vec<_> = lib
        .grip
        .iter()
        .filter(|g| g.family.as_deref() == Some("stand") && has(format!("{} {} {}", tr("pied", "stand"), g.name, g.short.as_deref().unwrap_or(""))))
        .collect();
    let grip: Vec<_> = lib
        .grip
        .iter()
        .filter(|g| g.family.as_deref() != Some("stand") && has(format!("grip {} {}", g.name, g.short.as_deref().unwrap_or(""))))
        .collect();
    let decor: Vec<_> = lib
        .props
        .iter()
        .filter(|t| has(format!("{} {} {}", category_label(t.category.as_deref().unwrap_or("")), t.name, t.short.as_deref().unwrap_or(""))))
        .collect();
    let counts = vec![lights.len() as i32, stands.len() as i32, grip.len() as i32, decor.len() as i32];

    let mut out: Vec<LibRow> = Vec::new();
    // en-tête repliable ; vrai si le groupe est replié
    let header = |out: &mut Vec<LibRow>, key: &str, count: usize| {
        let folded = q.is_empty() && p.folded.iter().any(|f| f == key);
        out.push(LibRow { kind: "brand".into(), id: key.into(), label: key.into(), count: count as i32, folded, ..Default::default() });
        folded
    };
    match p.tab {
        0 => match p.sort {
            2 | 3 => {
                let mut list = lights.clone();
                if p.sort == 2 {
                    list.sort_by(|a, b| coll(&a.name, &b.name).then_with(|| coll(&a.brand, &b.brand)));
                } else {
                    list.sort_by(|a, b| lux_of(b).total_cmp(&lux_of(a)).then_with(|| coll(&a.name, &b.name)));
                }
                out.extend(list.into_iter().map(|t| fixture_row(t, true, p.sort == 3)));
            }
            1 => {
                for (cat, _, _) in CATEGORIES {
                    let mut items: Vec<_> = lights.iter().filter(|t| t.category == cat).collect();
                    if items.is_empty() || header(&mut out, fixture_category(cat), items.len()) {
                        continue;
                    }
                    items.sort_by(|a, b| coll(&a.brand, &b.brand).then_with(|| coll(&a.name, &b.name)));
                    out.extend(items.into_iter().map(|t| fixture_row(t, true, false)));
                }
            }
            _ => {
                let mut brands: Vec<&str> = Vec::new();
                for t in &lights {
                    if !brands.contains(&group_of(t)) {
                        brands.push(group_of(t));
                    }
                }
                let trad = traditional_label();
                brands.sort_by(|a, b| (*a == trad).cmp(&(*b == trad)).then_with(|| coll(a, b)));
                for brand in brands {
                    let of_brand: Vec<_> = lights.iter().filter(|t| group_of(t) == brand).collect();
                    if header(&mut out, brand, of_brand.len()) {
                        continue;
                    }
                    for (cat, _, _) in CATEGORIES {
                        let items: Vec<_> = of_brand.iter().filter(|t| t.category == cat).collect();
                        if items.is_empty() {
                            continue;
                        }
                        out.push(line("cat", "", fixture_category(cat)));
                        out.extend(items.into_iter().map(|t| fixture_row(t, false, false)));
                    }
                }
            }
        },
        1 => {
            out.push(line(
                "note",
                "",
                tr(
                    "Pieds seuls (pour un drapeau, une pince…). Les projecteurs ont déjà leur pied, réglable dans l’inspecteur.",
                    "Stands only (for a flag, a clamp…). Fixtures already have their own stand, adjustable in the inspector.",
                ),
            ));
            out.extend(stands.iter().map(|g| LibRow { kind: "grip".into(), id: g.id.as_str().into(), label: g.name.as_str().into(), icon: "grip-stand".into(), ..Default::default() }));
        }
        2 => {
            for fam in GRIP_FAMILIES {
                let items: Vec<_> = grip.iter().filter(|g| g.family.as_deref() == Some(fam)).collect();
                if items.is_empty() {
                    continue;
                }
                out.push(line("cat", "", category_label(fam)));
                out.extend(items.into_iter().map(|g| LibRow { kind: "grip".into(), id: g.id.as_str().into(), label: g.name.as_str().into(), icon: format!("grip-{fam}").into(), ..Default::default() }));
            }
        }
        _ => {
            for cat in PROP_CATEGORIES {
                let items: Vec<_> = decor.iter().filter(|t| t.category.as_deref() == Some(cat)).collect();
                if items.is_empty() || header(&mut out, category_label(cat), items.len()) {
                    continue;
                }
                out.extend(items.into_iter().map(|t| LibRow { kind: "prop".into(), id: t.id.as_str().into(), label: t.name.as_str().into(), icon: format!("prop-{cat}").into(), ..Default::default() }));
            }
        }
    }
    if !out.iter().any(|r| matches!(r.kind.as_str(), "fixture" | "grip" | "prop")) && !out.iter().any(|r| r.folded) {
        out.push(line("empty", "", tr("Rien ne correspond à la recherche", "Nothing matches the search")));
    }
    (out, counts)
}

/// Replie ou déplie un groupe.
pub fn toggle_fold(p: &mut Prefs, key: &str) {
    if let Some(i) = p.folded.iter().position(|f| f == key) {
        p.folded.remove(i);
    } else {
        p.folded.push(key.to_owned());
    }
}
