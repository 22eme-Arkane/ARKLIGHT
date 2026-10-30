//! Liste du matériel (projecteurs, pieds, accessoires, grip, sacs de sable) et patch DMX, tirés de la
//! scène (portage de `web/js/plot/equipment.js`), et leurs exports CSV (Excel en français : BOM et
//! « ; »).

use crate::catalog::{cloth_name, grip_stands, props_of, support_def};
use crate::js::{collate, num, num_truthy, string_of, truthy};
use crate::render::{BY_HEIGHT, Tr, Viewport, tr};
use arklight_core::accessories::{Front, accessory_effect};
use arklight_core::library::library;
use arklight_core::project::Project;
use serde_json::Value;

/// Sacs de sable conseillés par pied (règle de sécurité usuelle, à adapter au plateau).
fn sandbags_per_stand(id: &str) -> u32 {
    match id {
        "stand-junior" | "windup" | "super-windup" | "long-john" => 2,
        _ => 1,
    }
}

/// Une ligne de la liste du matériel.
#[derive(Debug, Clone, PartialEq)]
pub struct EquipmentRow {
    pub name: String,
    pub qty: u32,
    /// remarque (puissance et modes, monture…) ; vide : aucune
    pub note: String,
}

/// Une rubrique (« Projecteurs », « Pieds et supports »…).
#[derive(Debug, Clone, PartialEq)]
pub struct EquipmentSection {
    pub title: String,
    pub rows: Vec<EquipmentRow>,
}

/// Liste du matériel : les rubriques non vides, dans l'ordre.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EquipmentList {
    pub sections: Vec<EquipmentSection>,
}

/// Table qui garde l'ordre d'arrivée (comme une `Map` JavaScript).
struct Ordered<T> {
    keys: Vec<String>,
    values: Vec<T>,
}

impl<T> Ordered<T> {
    fn new() -> Self {
        Self { keys: Vec::new(), values: Vec::new() }
    }

    fn entry(&mut self, key: &str, make: impl FnOnce() -> T) -> &mut T {
        let i = match self.keys.iter().position(|k| k == key) {
            Some(i) => i,
            None => {
                self.keys.push(key.to_string());
                self.values.push(make());
                self.values.len() - 1
            }
        };
        &mut self.values[i]
    }
}

struct FixtureEntry {
    name: String,
    qty: u32,
    watts: Option<f64>,
    modes: Vec<String>,
}

struct Stand {
    name: String,
    qty: u32,
    id: String,
}

fn by_name<T>(rows: &mut [T], name: impl Fn(&T) -> &str) {
    rows.sort_by(|a, b| collate(name(a), name(b)));
}

/// `buildEquipmentList` ; `viewport` : pieds retenus par la vue 3D (None : recalculés d'après la
/// hauteur des projecteurs).
pub fn build_equipment_list(scene: &Project, viewport: Option<&dyn Viewport>) -> EquipmentList {
    let lib = library();
    let view: &dyn Viewport = viewport.unwrap_or(&BY_HEIGHT);
    let mut fixtures: Ordered<FixtureEntry> = Ordered::new();
    let mut stands: Ordered<Stand> = Ordered::new();
    let mut add_stand = |id: &str, qty: u32, label: Option<&str>| {
        let key = label.map(str::to_string).or_else(|| support_def(id).and_then(|d| d.get("name")).and_then(Value::as_str).map(str::to_string)).unwrap_or_else(|| id.to_string());
        stands.entry(&key, || Stand { name: key.clone(), qty: 0, id: id.to_string() }).qty += qty;
    };
    let mut hanging = 0;
    let mut accessories: Ordered<EquipmentRow> = Ordered::new();
    for f in &scene.fixtures {
        let Some(ty) = lib.fixture(&f.type_id) else { continue };
        let cur = fixtures.entry(&ty.id, || FixtureEntry {
            name: if ty.name.starts_with(&format!("{} ", ty.brand)) { ty.name.clone() } else { format!("{} {}", ty.brand, ty.name) },
            qty: 0,
            watts: num_truthy(ty.extra.get("watts")).or_else(|| ty.lamp.as_ref().and_then(|l| l.watts).filter(|w| *w != 0.0)),
            modes: Vec::new(),
        });
        cur.qty += 1;
        if let Some(mode) = ty.mode(f.settings.dmx_mode.as_deref()) {
            let m = format!("{} ({} c.)", mode.name, mode.footprint());
            if !cur.modes.contains(&m) {
                cur.modes.push(m);
            }
        }
        let sup = view.support(f, ty);
        if sup.def.is_some() {
            add_stand(&sup.id, 1, None);
        } else if sup.id == "hanging" {
            hanging += 1;
        }
        // accessoires montés : boîte à lumière, dôme, louver, gélatine (feuille)
        let settings = Value::Object(f.settings.extra.clone());
        let e = accessory_effect(ty, Some(&settings), ty.beam_angle.filter(|a| *a != 0.0).unwrap_or(40.0));
        let mut add_acc = |name: String, note: &str| {
            accessories.entry(&name, || EquipmentRow { name: name.clone(), qty: 0, note: note.to_string() }).qty += 1;
        };
        if let Some(m) = e.modifier {
            add_acc(m.name.clone(), if m.kind == "softbox" { "avec couronne adaptée" } else { "monture Bowens" });
        }
        if let Some(Front::Filter(fl)) = e.front {
            add_acc(format!("Toile avant {}", fl.name), "");
        }
        if let Some(g) = e.grid {
            add_acc(g.name.clone(), "");
        }
        if let Some(g) = e.gel {
            add_acc(format!("Gélatine {}", g.name), "feuille");
        }
    }
    let mut grip: Ordered<EquipmentRow> = Ordered::new();
    for o in &scene.objects {
        let Some(g) = lib.grip(&o.type_id) else { continue };
        let props = props_of(o);
        let cloth = match props.get("cloth") {
            Some(c) if truthy(Some(c)) && g.extra.get("cloths").is_some_and(|v| truthy(Some(v))) => format!(" — {}", cloth_name(&string_of(c))),
            _ => String::new(),
        };
        let key = format!("{}|{cloth}", g.id);
        grip.entry(&key, || EquipmentRow { name: format!("{}{cloth}", g.name), qty: 0, note: String::new() }).qty += 1;
        if g.family.as_deref() == Some("stand") {
            add_stand(&g.id, 1, None);
        } else if let Some(st) = grip_stands(g, props) {
            add_stand(&st.id, st.count, st.label.as_deref());
        }
    }
    let sandbags: u32 = stands.values.iter().map(|s| sandbags_per_stand(&s.id) * s.qty).sum();

    let mut fx = fixtures.values;
    by_name(&mut fx, |r| &r.name);
    let mut st = stands.values;
    by_name(&mut st, |r| &r.name);
    let mut acc = accessories.values;
    by_name(&mut acc, |r| &r.name);
    let mut gr = grip.values;
    by_name(&mut gr, |r| &r.name);

    let mut stand_rows: Vec<EquipmentRow> = st.into_iter().map(|r| EquipmentRow { name: r.name, qty: r.qty, note: String::new() }).collect();
    if hanging > 0 {
        stand_rows.push(EquipmentRow { name: "Accroches (pinces, perches) pour projecteurs accrochés".into(), qty: hanging, note: String::new() });
    }
    let sections = vec![
        EquipmentSection {
            title: "Projecteurs".into(),
            rows: fx
                .into_iter()
                .map(|r| {
                    let note = [r.watts.map_or(String::new(), |w| format!("{} W", num(w))), r.modes.join(" / ")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ");
                    EquipmentRow { name: r.name, qty: r.qty, note }
                })
                .collect(),
        },
        EquipmentSection { title: "Pieds et supports".into(), rows: stand_rows },
        EquipmentSection { title: "Accessoires des projecteurs".into(), rows: acc },
        EquipmentSection { title: "Grip".into(), rows: gr },
        EquipmentSection {
            title: "Consommables conseillés".into(),
            rows: if sandbags > 0 { vec![EquipmentRow { name: "Sacs de sable (1 par pied, 2 par junior, wind-up ou Long John)".into(), qty: sandbags, note: "conseil".into() }] } else { Vec::new() },
        },
    ];
    EquipmentList { sections: sections.into_iter().filter(|s| !s.rows.is_empty()).collect() }
}

/// Une ligne du patch DMX.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchRow {
    pub name: String,
    pub model: String,
    pub universe: u32,
    pub address: u32,
    pub footprint: usize,
    pub mode: String,
}

/// `buildPatch` : patch DMX trié par univers et adresse (projecteurs sans DMX exclus).
pub fn build_patch(scene: &Project) -> Vec<PatchRow> {
    let lib = library();
    let mut rows: Vec<PatchRow> = scene
        .fixtures
        .iter()
        .map(|f| {
            let ty = lib.fixture(&f.type_id);
            let mode = ty.and_then(|t| t.mode(f.settings.dmx_mode.as_deref()));
            PatchRow {
                name: f.name.clone(),
                model: ty.map_or_else(|| f.type_id.clone(), |t| format!("{} {}", t.brand, t.name)),
                universe: f.settings.universe.unwrap_or(1),
                address: f.settings.dmx_address.unwrap_or(1),
                footprint: mode.map_or(0, |m| m.footprint()),
                mode: mode.map(|m| m.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| "—".into()),
            }
        })
        .filter(|r| r.footprint != 0)
        .collect();
    rows.sort_by(|a, b| a.universe.cmp(&b.universe).then(a.address.cmp(&b.address)));
    rows
}

fn csv_cell(s: &str) -> String {
    if s.contains([';', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

/// `toCsv` : BOM + « ; » (s'ouvre tel quel dans Excel en français), lignes en CRLF.
pub fn to_csv(rows: &[Vec<String>]) -> String {
    let lines: Vec<String> = rows.iter().map(|r| r.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(";")).collect();
    format!("\u{feff}{}", lines.join("\r\n"))
}

/// Liste du matériel en CSV ; `t` : traduction des textes (langue de l'interface).
pub fn equipment_csv(list: &EquipmentList, t: Option<Tr>) -> String {
    let mut rows = vec![["Rubrique", "Désignation", "Quantité", "Remarques"].iter().map(|x| tr(t, x)).collect::<Vec<_>>()];
    for s in &list.sections {
        for r in &s.rows {
            rows.push(vec![tr(t, &s.title), tr(t, &r.name), r.qty.to_string(), if r.note.is_empty() { String::new() } else { tr(t, &r.note) }]);
        }
    }
    to_csv(&rows)
}

/// Patch DMX en CSV.
pub fn patch_csv(patch: &[PatchRow], t: Option<Tr>) -> String {
    let mut rows = vec![["Univers", "Adresse", "Fin", "Projecteur", "Modèle", "Mode", "Canaux"].iter().map(|x| tr(t, x)).collect::<Vec<_>>()];
    for p in patch {
        rows.push(vec![p.universe.to_string(), p.address.to_string(), (p.address as i64 + p.footprint as i64 - 1).to_string(), p.name.clone(), p.model.clone(), p.mode.clone(), p.footprint.to_string()]);
    }
    to_csv(&rows)
}
