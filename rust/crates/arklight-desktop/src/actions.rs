//! Actions sur la sélection et les groupes (portage de `web/js/actions.js`) : menu « Sélection »,
//! menu contextuel de la vue 3D, liste des groupes, ré-adressage automatique.

use crate::{State, tr};
use arklight_core::Fixture;
use arklight_core::addressing::{Options, Order, Plan, Scope, conflicts, plan_addressing};
use arklight_core::document::{Changes, Edit};
use arklight_core::library;
use arklight_render::glam::Vec3;
use serde_json::{Value, json};

/// Centre du plateau à hauteur de visage : cible quand la scène n'a personne à viser.
const DEFAULT_AIM: Vec3 = Vec3::new(0.0, 1.6, 0.0);

/// Arrondi au millimètre / milliradian (comme la version JavaScript).
fn r3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Projecteurs choisis, dans l'ordre de la scène.
pub fn selected_fixtures(st: &State) -> Vec<&Fixture> {
    st.doc.project.fixtures.iter().filter(|f| st.selection.contains(&f.id)).collect()
}

fn selected_ids(st: &State) -> Vec<String> {
    selected_fixtures(st).iter().map(|f| f.id.clone()).collect()
}

/// Projecteurs qu'on oriente vers une cible (pas les tubes ni les boîtiers).
fn is_aimable(f: &Fixture) -> bool {
    library().fixture(&f.type_id).is_some_and(|t| !matches!(t.geometry.kind.as_str(), "tube" | "box-device"))
}

fn is_v5(f: &Fixture) -> bool {
    library().fixture(&f.type_id).is_some_and(|t| t.is_astera_v5())
}

fn power(f: &Fixture) -> bool {
    f.state.get("power").and_then(Value::as_bool).unwrap_or(false)
}

fn settings(map: Value) -> Changes {
    Changes { settings: map.as_object().cloned(), ..Changes::default() }
}

/// Angles d'Euler (ordre XYZ, comme three.js) d'un élément placé en `from` dont l'axe +Z vise `to`
/// (`Object3D.lookAt` de three.js).
pub fn look_at(from: [f64; 3], to: Vec3) -> [f64; 3] {
    let up = [0.0, 1.0, 0.0];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let norm = |a: [f64; 3]| {
        let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        if l > 0.0 { [a[0] / l, a[1] / l, a[2] / l] } else { a }
    };
    let len2 = |a: [f64; 3]| a[0] * a[0] + a[1] * a[1] + a[2] * a[2];
    let mut z = [to.x as f64 - from[0], to.y as f64 - from[1], to.z as f64 - from[2]];
    if len2(z) == 0.0 {
        z[2] = 1.0;
    }
    z = norm(z);
    let mut x = cross(up, z);
    if len2(x) == 0.0 {
        z[2] += 0.0001;
        z = norm(z);
        x = cross(up, z);
    }
    let x = norm(x);
    let y = cross(z, x);
    // matrice (colonnes x, y, z) → angles XYZ
    let (m11, m12, m13) = (x[0], y[0], z[0]);
    let (m22, m23) = (y[1], z[1]);
    let (m32, m33) = (y[2], z[2]);
    let ry = m13.clamp(-1.0, 1.0).asin();
    let (rx, rz) = if m13.abs() < 0.9999999 { ((-m23).atan2(m33), (-m12).atan2(m11)) } else { (m32.atan2(m22), 0.0) };
    [r3(rx), r3(ry), r3(rz)]
}

/// Cibles de « Viser » : (nom, point) des personnages, animaux, véhicules de la scène.
pub fn aim_targets(st: &State) -> Vec<(String, Vec3)> {
    let lib = library();
    st.scene
        .aims
        .iter()
        .filter_map(|a| {
            let o = st.doc.object(&a.id)?;
            let name = if o.name.is_empty() { lib.prop(&o.type_id).and_then(|t| t.short.clone()).unwrap_or_default() } else { o.name.clone() };
            Some((name, a.point))
        })
        .collect()
}

/// Cible par défaut : le premier personnage, sinon la première cible, sinon le centre du plateau.
pub fn main_aim_point(st: &State) -> Vec3 {
    st.scene.aims.iter().find(|a| a.person).or(st.scene.aims.first()).map_or(DEFAULT_AIM, |a| a.point)
}

/// Oriente les projecteurs choisis vers une cible (index de `aim_targets`, ou la cible par défaut) ;
/// en sélection multiple, les tubes et boîtiers ne bougent pas.
pub fn aim(st: &State, target: Option<usize>) -> Option<Edit> {
    let point = target.and_then(|i| aim_targets(st).get(i).map(|t| t.1)).unwrap_or_else(|| main_aim_point(st));
    let list = selected_fixtures(st);
    let single = list.len() == 1;
    let items: Vec<(String, Changes)> = list
        .into_iter()
        .filter(|f| single || is_aimable(f))
        .map(|f| (f.id.clone(), Changes { rotation: Some(look_at(f.position, point)), ..Changes::default() }))
        .collect();
    (!items.is_empty()).then_some(Edit::UpdateFixtures(items))
}

/// Pose la sélection au sol : le bas de chaque projecteur à 0, les objets à 0.
pub fn drop_to_floor(st: &State) -> Option<Edit> {
    let mut edits = Vec::new();
    for id in &st.selection {
        if let Some(f) = st.doc.fixture(id) {
            let Some(pick) = st.scene.picks.iter().find(|p| p.id == *id) else { continue };
            let y = r3(f.position[1] - pick.lo.y as f64);
            edits.push(Edit::UpdateFixture { id: id.clone(), changes: Changes::moved([f.position[0], y, f.position[2]], None) });
        } else if let Some(o) = st.doc.object(id) {
            edits.push(Edit::UpdateObject { id: id.clone(), changes: Changes::moved([o.position[0], 0.0, o.position[2]], None) });
        }
    }
    (!edits.is_empty()).then_some(Edit::Batch(edits))
}

/// Cadre la vue sur la sélection (la caméra garde son orientation).
pub fn frame(st: &mut State) -> bool {
    let picks: Vec<_> = st.scene.picks.iter().filter(|p| st.selection.contains(&p.id)).collect();
    let Some(first) = picks.first() else { return false };
    let (mut lo, mut hi) = (first.lo, first.hi);
    for p in &picks {
        lo = lo.min(p.lo);
        hi = hi.max(p.hi);
    }
    let size = (hi - lo).length().max(0.5);
    st.camera.target = (lo + hi) / 2.0;
    st.camera.distance = size * 1.8 + 0.6;
    st.dirty = true;
    true
}

/// Allume si un projecteur choisi est éteint, sinon éteint tout.
pub fn toggle_power(st: &State, ids: &[String]) -> Option<Edit> {
    let list: Vec<&Fixture> = ids.iter().filter_map(|id| st.doc.fixture(id)).collect();
    let on = list.iter().any(|f| !power(f));
    let items: Vec<(String, Changes)> = list.iter().map(|f| (f.id.clone(), Changes { state: json!({ "power": on }).as_object().cloned(), ..Changes::default() })).collect();
    (!items.is_empty()).then_some(Edit::UpdateFixtures(items))
}

/// Liaison CRMX des Astera (comme la touche LINK d'un émetteur).
pub fn set_crmx(st: &State, ids: &[String], linked: bool) -> Option<Edit> {
    let items: Vec<(String, Changes)> = ids.iter().filter_map(|id| st.doc.fixture(id)).filter(|f| is_v5(f)).map(|f| (f.id.clone(), settings(json!({ "crmxLinked": linked })))).collect();
    (!items.is_empty()).then_some(Edit::UpdateFixtures(items))
}

/// Numéros de fixture à la suite à partir de `first` ; message pour la barre d'état.
pub fn number(st: &State, first: u32) -> (Option<Edit>, String) {
    let list = selected_fixtures(st);
    let n = list.len() as u32;
    let items: Vec<(String, Changes)> = list.iter().enumerate().map(|(i, f)| (f.id.clone(), settings(json!({ "fixtureNumber": (first + i as u32).min(9999) })))).collect();
    let ids = selected_ids(st);
    let taken: Vec<String> = st
        .doc
        .project
        .fixtures
        .iter()
        .filter(|o| !ids.contains(&o.id) && o.settings.fixture_number.is_some_and(|k| k >= first && k < first + n))
        .map(|o| format!("Fix {} ({})", o.settings.fixture_number.unwrap_or(0), o.name))
        .collect();
    let msg = if !taken.is_empty() {
        format!("{} {}", tr("Déjà utilisé :", "Already used:"), taken.join(", "))
    } else if n > 1 {
        format!("Fix {first} – {}", first + n - 1)
    } else {
        list.first().map_or_else(String::new, |f| format!("{}{} Fix {first}", f.name, tr(" :", ":")))
    };
    ((!items.is_empty()).then_some(Edit::UpdateFixtures(items)), msg)
}

// ------------------------------------------------------------------ sélection
pub fn select_same_type(st: &mut State) {
    let types: Vec<String> = selected_fixtures(st).iter().map(|f| f.type_id.clone()).collect();
    st.selection = st.doc.project.fixtures.iter().filter(|f| types.contains(&f.type_id)).map(|f| f.id.clone()).collect();
}

pub fn invert_selection(st: &mut State) {
    st.selection = st.doc.project.fixtures.iter().filter(|f| !st.selection.contains(&f.id)).map(|f| f.id.clone()).collect();
}

/// Les projecteurs d'un groupe (ajoutés à la sélection si `additive`).
pub fn select_group(st: &mut State, gid: &str, additive: bool) {
    let Some(g) = st.doc.project.groups.iter().find(|g| g.id == gid) else { return };
    let ids: Vec<String> = st.doc.project.fixtures.iter().filter(|f| g.ids.contains(&f.id)).map(|f| f.id.clone()).collect();
    if additive {
        for id in ids {
            if !st.selection.contains(&id) {
                st.selection.push(id);
            }
        }
    } else {
        st.selection = ids;
    }
}

// ------------------------------------------------------------------ groupes
pub fn group_selection(st: &State) -> Option<Edit> {
    let ids = selected_ids(st);
    (!ids.is_empty()).then_some(Edit::CreateGroup { name: None, ids })
}

/// Ajoute (`add`) ou retire la sélection d'un groupe.
pub fn change_group(st: &State, gid: &str, add: bool) -> Option<Edit> {
    let g = st.doc.project.groups.iter().find(|g| g.id == gid)?;
    let sel = selected_ids(st);
    let mut ids = g.ids.clone();
    if add {
        ids.extend(sel.into_iter().filter(|id| !g.ids.contains(id)));
    } else {
        ids.retain(|id| !sel.contains(id));
    }
    (ids != g.ids).then(|| Edit::UpdateGroup { id: gid.into(), name: None, ids: Some(ids) })
}

/// Ce que les menus affichent de la sélection.
pub struct MenuInfo {
    pub count: usize,
    pub fixtures: usize,
    /// un projecteur choisi est éteint : « Allumer »
    pub any_off: bool,
    pub v5: bool,
    pub aimable: bool,
    /// « Sélectionner tous les « Titan Tube » » (un seul modèle choisi)
    pub same_label: String,
    /// (id, nom, nombre de projecteurs, toute la sélection y est, une partie y est)
    pub groups: Vec<(String, String, usize, bool, bool)>,
    pub aims: Vec<String>,
}

pub fn menu_info(st: &State) -> MenuInfo {
    let list = selected_fixtures(st);
    let ids = selected_ids(st);
    let mut types: Vec<&str> = list.iter().map(|f| f.type_id.as_str()).collect();
    types.dedup();
    let same_label = match (types.len(), list.first()) {
        (1, Some(f)) => {
            let name = library().fixture(&f.type_id).map_or(f.type_id.clone(), |t| t.name.clone());
            format!("{} « {name} »", tr("Sélectionner tous les", "Select all"))
        }
        _ => tr("Sélectionner les mêmes modèles", "Select the same models").into(),
    };
    let groups = st
        .doc
        .project
        .groups
        .iter()
        .map(|g| {
            let count = g.ids.iter().filter(|id| st.doc.fixture(id).is_some()).count();
            let all = !ids.is_empty() && ids.iter().all(|id| g.ids.contains(id));
            let some = ids.iter().any(|id| g.ids.contains(id));
            (g.id.clone(), g.name.clone(), count, all, some)
        })
        .collect();
    MenuInfo {
        count: st.selection.len(),
        fixtures: list.len(),
        any_off: list.iter().any(|f| !power(f)),
        v5: list.iter().any(|f| is_v5(f)),
        aimable: list.iter().any(|f| is_aimable(f)) || list.len() == 1,
        same_label,
        groups,
        aims: aim_targets(st).into_iter().map(|t| t.0).collect(),
    }
}

// ------------------------------------------------------------------ ré-adressage automatique
/// Réglages de la fenêtre « Ré-adressage automatique ».
pub struct Readdress {
    /// portées proposées : conflicts, all, selection, group:<id>, universe:<n>
    pub scopes: Vec<String>,
    pub scope: usize,
    pub order: Order,
    pub universe: u32,
    pub address: u32,
    pub pack: bool,
    pub align: u32,
    pub overflow: bool,
    pub plan: Plan,
}

impl Default for Readdress {
    fn default() -> Self {
        Readdress { scopes: Vec::new(), scope: 0, order: Order::Scene, universe: 1, address: 1, pack: false, align: 0, overflow: true, plan: Plan::default() }
    }
}

/// Pas d'alignement proposés, avec leur libellé.
pub const ALIGNS: [u32; 8] = [0, 5, 10, 16, 20, 32, 50, 100];

pub fn align_labels() -> Vec<String> {
    ALIGNS
        .iter()
        .map(|a| match a {
            0 => tr("À la suite (aucun trou)", "One after another (no gap)").into(),
            16 => format!("1, 17, 33, 49… {}", tr("(blocs de 16 : petites consoles)", "(blocks of 16: small consoles)")),
            32 => format!("1, 33, 65, 97… {}", tr("(blocs de 32 : DMX Operator 384)", "(blocks of 32: DMX Operator 384)")),
            n => format!("1, {}, {}, {}…", 1 + n, 1 + 2 * n, 1 + 3 * n),
        })
        .collect()
}

pub fn order_labels() -> Vec<String> {
    Order::ALL
        .iter()
        .map(|o| {
            match o {
                Order::Scene => tr("Ordre de la scène", "Scene order"),
                Order::Name => tr("Par nom", "By name"),
                Order::Position => tr("De gauche à droite (vue de face)", "Left to right (front view)"),
                Order::Type => tr("Par modèle", "By model"),
                Order::Address => tr("Adresse actuelle", "Current address"),
            }
            .to_owned()
        })
        .collect()
}

/// Projecteurs visés par une portée « sélection », « groupe » ou « univers » (None : conflits, tout).
fn scope_ids(st: &State, scope: &str) -> Option<Vec<String>> {
    let fixtures = &st.doc.project.fixtures;
    if scope == "selection" {
        return Some(selected_ids(st));
    }
    if let Some(gid) = scope.strip_prefix("group:") {
        let g = st.doc.project.groups.iter().find(|g| g.id == gid);
        return Some(g.map_or_else(Vec::new, |g| fixtures.iter().filter(|f| g.ids.contains(&f.id)).map(|f| f.id.clone()).collect()));
    }
    if let Some(u) = scope.strip_prefix("universe:") {
        let u: u32 = u.parse().unwrap_or(1);
        return Some(fixtures.iter().filter(|f| f.settings.universe.unwrap_or(1) == u).map(|f| f.id.clone()).collect());
    }
    None
}

/// Ouvre la fenêtre : portées proposées et leurs libellés (`preset` : portée voulue).
pub fn readdress_open(st: &mut State, preset: Option<&str>) -> Vec<String> {
    let fixtures = &st.doc.project.fixtures;
    let n_conflicts = conflicts(fixtures).ids.len();
    let sel = selected_fixtures(st).len();
    let mut universes: Vec<u32> = fixtures.iter().map(|f| f.settings.universe.unwrap_or(1)).collect();
    universes.sort_unstable();
    universes.dedup();
    let mut scopes = vec![
        ("conflicts".to_owned(), format!("{} ({n_conflicts})", tr("Corriger seulement les conflits", "Fix conflicts only"))),
        ("all".to_owned(), format!("{} ({})", tr("Tous les projecteurs", "All fixtures"), fixtures.len())),
    ];
    if sel > 0 {
        scopes.push(("selection".into(), format!("{} ({sel})", tr("La sélection", "The selection"))));
    }
    for g in &st.doc.project.groups {
        scopes.push((format!("group:{}", g.id), format!("{} « {} » ({})", tr("Groupe", "Group"), g.name, g.ids.len())));
    }
    if universes.len() > 1 {
        for u in universes {
            let n = fixtures.iter().filter(|f| f.settings.universe.unwrap_or(1) == u).count();
            scopes.push((format!("universe:{u}"), format!("{} {u} ({n})", tr("Univers", "Universe"))));
        }
    }
    let want = preset.map(str::to_owned).unwrap_or_else(|| if n_conflicts > 0 { "conflicts".into() } else { "all".into() });
    st.readdress.scope = scopes.iter().position(|s| s.0 == want).unwrap_or(if n_conflicts > 0 { 0 } else { 1 });
    st.readdress.scopes = scopes.iter().map(|s| s.0.clone()).collect();
    readdress_scope_defaults(st);
    readdress_update(st);
    scopes.into_iter().map(|s| s.1).collect()
}

/// Départ proposé pour la portée : là où commence déjà le premier projecteur concerné.
pub fn readdress_scope_defaults(st: &mut State) {
    let scope = st.readdress.scopes.get(st.readdress.scope).cloned().unwrap_or_default();
    let Some(ids) = scope_ids(st, &scope) else {
        st.readdress.universe = 1;
        st.readdress.address = 1;
        st.readdress.pack = false;
        return;
    };
    let list: Vec<&Fixture> = ids.iter().filter_map(|id| st.doc.fixture(id)).collect();
    let u = match scope.strip_prefix("universe:") {
        Some(u) => u.parse().unwrap_or(1),
        None => list.first().map_or(1, |f| f.settings.universe.unwrap_or(1)),
    };
    let first = list.iter().filter(|f| f.settings.universe.unwrap_or(1) == u).filter_map(|f| f.settings.dmx_address).min();
    st.readdress.universe = u;
    st.readdress.address = first.unwrap_or(1);
    st.readdress.pack = true;
}

/// Recalcule le plan avec les réglages de la fenêtre.
pub fn readdress_update(st: &mut State) {
    let scope = st.readdress.scopes.get(st.readdress.scope).cloned().unwrap_or_default();
    let ids = scope_ids(st, &scope);
    let r = &st.readdress;
    let opts = Options {
        scope: match (ids, scope.as_str()) {
            (Some(ids), _) => Scope::Ids(ids),
            (None, "all") => Scope::All,
            _ => Scope::Conflicts,
        },
        order: r.order,
        universe: r.universe,
        address: r.address,
        pack: r.pack,
        align: r.align,
        overflow: r.overflow,
    };
    st.readdress.plan = plan_addressing(&st.doc.project.fixtures, &opts);
}

/// Le plan à appliquer (une seule étape d'annulation).
pub fn readdress_edit(st: &State) -> Option<Edit> {
    let items: Vec<(String, Changes)> = st
        .readdress
        .plan
        .moves
        .iter()
        .map(|m| (m.id.clone(), settings(json!({ "universe": m.to.universe, "dmxAddress": m.to.address }))))
        .collect();
    (!items.is_empty()).then_some(Edit::UpdateFixtures(items))
}

/// « U1 · 001–012 »
pub fn fmt_range(universe: u32, start: u32, footprint: u32) -> String {
    if footprint > 1 { format!("U{universe} · {start:03}–{:03}", start + footprint - 1) } else { format!("U{universe} · {start:03}") }
}
