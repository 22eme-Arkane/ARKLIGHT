//! Console ARKLIGHT (panneau du bas) : pilote les projecteurs choisis du même modèle et du même
//! profil, canal par canal, en partant de ce qu'ils font déjà ; « Relâcher » leur rend la main
//! (comme `web/js/ui/test-console.js`, moteur : `arklight_core::engine::console`).

use crate::tr;
use arklight_core::document::{Changes, Document, Edit};
use arklight_core::engine::console::{ColorChange, Look, active_console_values, adjust_color, console_held, console_options, look_to_values, mode_pixels, seed_console};
use arklight_core::engine::dmx::{Func, MapKind, Slot};
use arklight_core::engine::{Lights, Runtime, Universes, js_round};
use arklight_core::library::{DmxMode, FixtureType, library};
use arklight_core::project::Fixture;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

/// Un fader (voir `ConsoleFader` dans app.slint).
pub struct Fader {
    pub label: String,
    pub channel: String,
    pub value: f32,
    pub max: f32,
    pub text: String,
}

/// Libellé court d'une fonction (« Rouge », « Kelvin »).
fn function_label(f: Func) -> &'static str {
    match f {
        Func::Dimmer => "Dimmer",
        Func::Red | Func::FxR => tr("Rouge", "Red"),
        Func::Green | Func::FxG => tr("Vert", "Green"),
        Func::Blue | Func::FxB => tr("Bleu", "Blue"),
        Func::White => tr("Blanc", "White"),
        Func::Amber => tr("Ambre", "Amber"),
        Func::Mint => tr("Menthe", "Mint"),
        Func::Lime => "Lime",
        Func::Cyan => "Cyan",
        Func::Uv => "UV",
        Func::Ww => tr("Blanc chaud", "Warm white"),
        Func::Cw => tr("Blanc froid", "Cool white"),
        Func::Magenta => "Magenta",
        Func::Yellow => tr("Jaune", "Yellow"),
        Func::Indigo => "Indigo",
        Func::Cct => "Kelvin",
        Func::Tint => tr("Teinte ±V", "Tint ±G"),
        Func::Crossfade => tr("Fondu couleur", "Colour fade"),
        Func::Hue => tr("Teinte", "Hue"),
        Func::Sat => tr("Satur.", "Sat."),
        Func::Strobe => "Strobe",
        Func::X => "x",
        Func::Y => "y",
        Func::Index => tr("Gélatine", "Gel"),
        Func::CctIndex => tr("Kelvin index", "Kelvin index"),
        Func::Fan => tr("Ventilation", "Fan"),
        Func::FxProgram => tr("Programme", "Program"),
        Func::FxSpeed => tr("Vitesse", "Speed"),
        Func::FxFade => tr("Fondu", "Fade"),
        Func::FxDirection => tr("Sens", "Direction"),
        Func::FxSize => tr("Taille", "Size"),
        Func::FxOffset => tr("Décalage", "Offset"),
        Func::FxRestart => tr("Relance", "Restart"),
        Func::FxIndex => tr("Couleur", "Colour"),
        Func::None | Func::Other => "—",
    }
}

/// Libellé court d'un canal, avec le numéro de couleur d'effet et le pixel (« Rouge P3 »).
pub fn short_label(s: &Slot) -> String {
    let known = !matches!(s.func(), Func::None | Func::Other);
    let mut base = if known { function_label(s.func()).to_owned() } else { s.def.label.clone().unwrap_or_else(|| "—".into()) };
    if known && s.def.slot > 0 {
        base = format!("{base} {}", s.def.slot);
    }
    if s.px >= 0 { format!("{base} P{}", s.px + 1) } else { base }
}

/// Valeur lisible d'un canal (unités quand on les connaît : %, K, °) ; `bytes` : valeur DMX brute.
fn fader_text(s: &Slot, raw: u16, bytes: bool) -> String {
    if bytes {
        return js_round(if s.bytes() == 2 { raw as f64 / 257.0 } else { raw as f64 }).to_string();
    }
    let max = s.max() as f64;
    let n = raw as f64 / max;
    let def = &s.def;
    if let Some(kind) = def.map {
        // valeur réelle des canaux Astera (stroboscope en Hz, Kelvin, teinte) plutôt que la valeur brute
        let v8 = js_round(if s.bytes() == 2 { raw as f64 / 257.0 } else { raw as f64 });
        return match (kind, kind.apply(v8)) {
            (MapKind::AsteraStrobe, Some(hz)) if hz > 0.0 => format!("{hz:.1} Hz"),
            (MapKind::AsteraStrobe, Some(hz)) if hz < 0.0 => tr("effet", "effect").into(),
            (MapKind::AsteraStrobe, _) => tr("fixe", "steady").into(),
            (MapKind::AsteraCct | MapKind::AsteraCctInd, Some(k)) => format!("{} K", js_round(k / 10.0) * 10.0),
            (MapKind::AsteraCct | MapKind::AsteraCctInd, None) => "—".into(),
            (MapKind::AsteraGm, Some(t)) => format!("{:+.0}", t * 100.0),
            _ => format!("{v8}"),
        };
    }
    let neutral = def.neutral.is_some_and(|nt| n <= nt + 0.5 / 255.0);
    match s.func() {
        Func::Cct => {
            if neutral {
                return "—".into();
            }
            let r = def.range.unwrap_or([2000.0, 10000.0]);
            format!("{} K", js_round((r[0] + n * (r[1] - r[0])) / 10.0) * 10.0)
        }
        Func::Hue => format!("{}°", js_round(n * 360.0)),
        Func::Tint => {
            if neutral {
                "0".into()
            } else {
                format!("{}", js_round((n * 2.0 - 1.0) * 100.0))
            }
        }
        Func::None => raw.to_string(),
        _ => format!("{} %", js_round(n * 100.0)),
    }
}

/// Projecteurs pilotés : ceux de la sélection du même modèle et du même profil que le premier.
fn family<'a>(doc: &'a Document, selection: &[String]) -> Option<(&'static FixtureType, &'static DmxMode, Vec<&'a Fixture>)> {
    let lib = library();
    let first = selection.iter().find_map(|id| doc.fixture(id))?;
    let ty = lib.fixture(&first.type_id)?;
    let mode = ty.mode(first.settings.dmx_mode.as_deref())?;
    let list = selection.iter().filter_map(|id| doc.fixture(id)).filter(|f| f.type_id == first.type_id && ty.mode(f.settings.dmx_mode.as_deref()).map(|m| &m.id) == Some(&mode.id)).collect();
    Some((ty, mode, list))
}

/// Valeurs actuelles de la console pour un projecteur : les siennes s'il est déjà piloté, sinon au
/// plus près de ce qu'il fait (réglage, mode autonome, DMX reçu).
fn current_values(f: &Fixture, ty: &FixtureType, mode: &DmxMode, lights: &Lights, dmx: &Universes) -> Vec<u16> {
    if console_held(f) {
        return active_console_values(f, ty, Some(mode), &mut Runtime::default()).unwrap_or_default();
    }
    let source = lights.get(&f.id).map(|l| l.source.as_str());
    seed_console(f, ty, mode, Some(dmx), source)
}

/// Familles de la sélection (`families`) : projecteurs du même modèle et du même profil, dans
/// l'ordre de la sélection ; la console en pilote une à la fois.
pub fn families(doc: &Document, selection: &[String]) -> Vec<Vec<String>> {
    let lib = library();
    let mut out: Vec<(String, String, Vec<String>)> = Vec::new();
    for f in selection.iter().filter_map(|id| doc.fixture(id)) {
        let Some(ty) = lib.fixture(&f.type_id) else { continue };
        let Some(mode) = ty.mode(f.settings.dmx_mode.as_deref()) else { continue };
        match out.iter_mut().find(|e| e.0 == f.type_id && e.1 == mode.id) {
            Some(e) => e.2.push(f.id.clone()),
            None => out.push((f.type_id.clone(), mode.id.clone(), vec![f.id.clone()])),
        }
    }
    out.into_iter().map(|e| e.2).collect()
}

/// En-tête de la console : qui est piloté, familles, profil, place dans le patch, état.
#[derive(Default)]
pub struct Head {
    pub title: String,
    pub sub: String,
    /// plusieurs modèles ou profils : une pastille par famille (« Titan Tube ×3 »)
    pub chips: Vec<String>,
    pub profiles: Vec<String>,
    pub profile: usize,
    /// « Univers 1 · canaux 1–12 » (un seul projecteur)
    pub place: String,
    pub pixels: usize,
    pub note: String,
    pub warn: String,
}

fn tr_owned(fr: String, en: String) -> String {
    if tr("fr", "en") == "en" { en } else { fr }
}

/// En-tête de la console pour la famille `ids` (parmi `families`) de la sélection.
pub fn head(doc: &Document, selection: &[String], families: &[Vec<String>], ids: &[String]) -> Head {
    let lib = library();
    let Some((ty, mode, list)) = family(doc, ids) else { return Head::default() };
    let selected: Vec<&Fixture> = selection.iter().filter_map(|id| doc.fixture(id)).collect();
    let single = (list.len() == 1).then(|| list[0]);
    // un groupe entier choisi : son nom
    let group = doc.project.groups.iter().find(|g| !g.ids.is_empty() && g.ids.len() == selected.len() && g.ids.iter().all(|id| selection.contains(id))).map(|g| g.name.clone());
    let title = match (single, selected.len()) {
        (Some(f), 1) => f.name.clone(),
        _ => group.unwrap_or_else(|| tr_owned(format!("{} projecteurs", selected.len()), format!("{} fixtures", selected.len()))),
    };
    let chips = if families.len() > 1 {
        families
            .iter()
            .filter_map(|fam| {
                let f = doc.fixture(fam.first()?)?;
                Some(format!("{} ×{}", lib.fixture(&f.type_id)?.name, fam.len()))
            })
            .collect()
    } else {
        Vec::new()
    };
    let profiles = ty.dmx.modes.iter().map(|m| format!("{} — {} c.", if m.name.is_empty() { &m.id } else { &m.name }, m.footprint())).collect();
    let profile = ty.dmx.modes.iter().position(|m| m.id == mode.id).unwrap_or(0);
    let place = single.map_or(String::new(), |f| {
        let a = f.settings.dmx_address.unwrap_or(1);
        tr_owned(format!("Univers {} · canaux {}–{}", f.settings.universe.unwrap_or(1), a, a + mode.footprint() as u32 - 1), format!("Universe {} · channels {}–{}", f.settings.universe.unwrap_or(1), a, a + mode.footprint() as u32 - 1))
    });
    let held = list.iter().filter(|f| console_held(f)).count();
    let note = if list.len() == 1 {
        if held > 0 { tr("Piloté par la console.", "Driven by the console.").to_owned() } else { tr("Pas encore piloté : la console part de son réglage actuel. Bouge un fader pour en prendre la main.", "Not driven yet: the console starts from its current setting. Move a fader to take control.").to_owned() }
    } else if held > 0 {
        tr_owned(format!("Pilotés par la console : {held} sur {}.", list.len()), format!("Driven by the console: {held} of {}.", list.len()))
    } else {
        tr("Pas encore pilotés : la console part de leur réglage actuel. Bouge un fader pour en prendre la main (les autres projecteurs ne changent pas).", "Not driven yet: the console starts from their current setting. Move a fader to take control (the other fixtures don't change).").to_owned()
    };
    let issues: Vec<String> = list.iter().flat_map(|f| crate::scene_panel::issues(f, &doc.project.fixtures)).collect();
    let warn = issues.first().map(|i| format!("⚠ {i}{}", if issues.len() > 1 { " …" } else { "" })).unwrap_or_default();
    Head {
        title,
        sub: if chips.is_empty() { format!("{} × {} {}", list.len(), ty.brand, ty.name) } else { String::new() },
        chips,
        profiles,
        profile,
        place,
        pixels: mode_pixels(mode, ty.pixels),
        note,
        warn,
    }
}

/// « Arc-en-ciel » : une couleur par pixel, à pleine intensité.
pub fn rainbow(doc: &Document, ids: &[String], lights: &Lights, dmx: &Universes) -> Option<Edit> {
    let items = write(doc, ids, lights, dmx, |values, f, mode, ty| {
        let n = mode_pixels(mode, ty.pixels).max(1);
        let looks: Vec<Look> = (0..n).map(|p| Look { level: Some(1.0), hue: Some(360.0 * p as f64 / n as f64), sat: Some(1.0), ..Look::default() }).collect();
        *values = look_to_values(mode, &looks, console_options(f, ty));
        true
    });
    edit_of(items)
}

/// Titre, faders, et si la console pilote déjà ces projecteurs.
pub fn panel(doc: &Document, selection: &[String], lights: &Lights, dmx: &Universes) -> (String, Vec<Fader>, bool) {
    panel_raw(doc, selection, lights, dmx, false)
}

/// Comme `panel` ; `bytes` : valeurs DMX brutes (0–255) plutôt qu'en %, K ou °.
pub fn panel_raw(doc: &Document, selection: &[String], lights: &Lights, dmx: &Universes, bytes: bool) -> (String, Vec<Fader>, bool) {
    let Some((ty, mode, list)) = family(doc, selection) else {
        return (tr("Console — choisis un ou plusieurs projecteurs (même modèle, même profil)", "Console — select one or more fixtures (same model, same profile)").into(), Vec::new(), false);
    };
    let first = list[0];
    let values = current_values(first, ty, mode, lights, dmx);
    let single = list.len() == 1;
    let start = first.settings.dmx_address.unwrap_or(1) as usize;
    let faders = mode
        .compiled()
        .slots
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let v = values.get(i).copied().unwrap_or(0);
            Fader {
                label: short_label(s),
                channel: if single { (start + s.offset).to_string() } else { format!("c{}", s.offset + 1) },
                value: v as f32,
                max: s.max() as f32,
                text: fader_text(s, v, bytes),
            }
        })
        .collect();
    let names = if single { first.name.clone() } else { format!("{} {}", list.len(), tr("projecteurs", "fixtures")) };
    let profile = if mode.name.is_empty() { mode.id.clone() } else { mode.name.clone() };
    let held = list.iter().any(|f| console_held(f));
    (format!("{} — {names} · {profile}", tr("Console", "Console")), faders, held)
}

/// Un fader bougé : la même valeur pour tous les projecteurs pilotés (une étape par geste).
pub fn move_fader(doc: &Document, selection: &[String], lights: &Lights, dmx: &Universes, index: usize, value: f32) -> Option<Edit> {
    let (ty, mode, list) = family(doc, selection)?;
    let items = list
        .iter()
        .map(|f| {
            let mut values = current_values(f, ty, mode, lights, dmx);
            if let Some(v) = values.get_mut(index) {
                *v = value.round().clamp(0.0, 65535.0) as u16;
            }
            let mut state = Map::new();
            state.insert("console".into(), json!({ "mode": mode.id, "values": values }));
            (f.id.clone(), Changes { state: Some(state), ..Changes::default() })
        })
        .collect();
    Some(Edit::UpdateFixtures(items))
}

/// « Canaux » : la console prend les projecteurs en partant de leur réglage actuel.
pub fn take(doc: &Document, selection: &[String], lights: &Lights, dmx: &Universes) -> Option<Edit> {
    let (ty, mode, list) = family(doc, selection)?;
    let items = list
        .iter()
        .map(|f| {
            let values = current_values(f, ty, mode, lights, dmx);
            let mut state = Map::new();
            state.insert("console".into(), json!({ "mode": mode.id, "values": values }));
            (f.id.clone(), Changes { state: Some(state), ..Changes::default() })
        })
        .collect();
    Some(Edit::UpdateFixtures(items))
}

/// « Relâcher » : les projecteurs reprennent leurs propres réglages.
pub fn release(doc: &Document, selection: &[String]) -> Option<Edit> {
    let items: Vec<(String, Changes)> = selection
        .iter()
        .filter_map(|id| doc.fixture(id))
        .filter(|f| console_held(f))
        .map(|f| {
            let mut state = Map::new();
            state.insert("console".into(), Value::Null);
            (f.id.clone(), Changes { state: Some(state), ..Changes::default() })
        })
        .collect();
    (!items.is_empty()).then_some(Edit::UpdateFixtures(items))
}

// ------------------------------------------------------------------ pilotage (MIDI) : intensité, couleur, flash
// (portage de `web/js/ui/console-control.js`)

/// Ce que la console retient entre deux messages d'un même geste.
#[derive(Default)]
pub struct Memory {
    /// proportions des couleurs (profils sans gradateur ramenés à 0)
    colors: HashMap<String, Vec<f64>>,
    /// réglage de couleur du dernier geste (valeurs → réglage), pour ne pas tout recalculer
    looks: HashMap<String, (Vec<u16>, Look)>,
    /// valeurs d'avant le flash (None : le projecteur n'était pas piloté)
    flashes: HashMap<String, Option<Vec<u16>>>,
}

fn console_state(mode: &DmxMode, values: Vec<u16>) -> Changes {
    let mut state = Map::new();
    state.insert("console".into(), json!({ "mode": mode.id, "values": values }));
    Changes { state: Some(state), ..Changes::default() }
}

/// Valeurs de la console de chaque projecteur modifiées par `f` (le projecteur est pris en main) ;
/// `f` renvoie faux pour ne rien écrire.
fn write(doc: &Document, ids: &[String], lights: &Lights, dmx: &Universes, mut f: impl FnMut(&mut Vec<u16>, &Fixture, &'static DmxMode, &'static FixtureType) -> bool) -> Vec<(String, Changes)> {
    let lib = library();
    let mut items = Vec::new();
    for fx in ids.iter().filter_map(|id| doc.fixture(id)) {
        let Some(ty) = lib.fixture(&fx.type_id) else { continue };
        let Some(mode) = ty.mode(fx.settings.dmx_mode.as_deref()) else { continue };
        let mut values = current_values(fx, ty, mode, lights, dmx);
        if f(&mut values, fx, mode, ty) {
            items.push((fx.id.clone(), console_state(mode, values)));
        }
    }
    items
}

fn edit_of(items: Vec<(String, Changes)>) -> Option<Edit> {
    (!items.is_empty()).then_some(Edit::UpdateFixtures(items))
}

/// Intensité (0..1) par le gradateur du profil ; sans canal Dimmer, l'intensité passe dans les
/// couleurs, dans les mêmes proportions (la teinte revient après un passage à zéro).
pub fn set_intensity(doc: &Document, ids: &[String], lights: &Lights, dmx: &Universes, level: f64, mem: &mut Memory) -> Option<Edit> {
    let items = write(doc, ids, lights, dmx, |values, f, mode, _| {
        let slots = &mode.compiled().slots;
        let max = |i: usize| slots[i].max() as f64;
        let dimmers: Vec<usize> = (0..slots.len()).filter(|i| slots[*i].func() == Func::Dimmer).collect();
        if !dimmers.is_empty() {
            for i in dimmers {
                values[i] = js_round(level * max(i)) as u16;
            }
            return true;
        }
        let emit: Vec<usize> = (0..slots.len()).filter(|i| slots[*i].func().is_emitter()).collect();
        if emit.is_empty() {
            return false;
        }
        let top = emit.iter().map(|i| values[*i] as f64 / max(*i)).fold(0.0, f64::max);
        let mut ratios = if top > 0.0 { Some(emit.iter().map(|i| values[*i] as f64 / max(*i) / top).collect::<Vec<_>>()) } else { mem.colors.get(&f.id).cloned() };
        if top > 0.0 {
            mem.colors.insert(f.id.clone(), ratios.clone().unwrap_or_default());
        }
        if ratios.as_ref().is_none_or(|r| r.len() != emit.len()) {
            ratios = Some(vec![1.0; emit.len()]);
        }
        let ratios = ratios.unwrap_or_default();
        for (k, i) in emit.iter().enumerate() {
            values[*i] = js_round(ratios[k] * level * max(*i)) as u16;
        }
        true
    });
    edit_of(items)
}

/// Couleur (Kelvin, teinte ±vert, teinte, saturation) à la même intensité ; pendant un geste, le
/// réglage de départ est retenu au lieu d'être recalculé à chaque message.
pub fn set_color(doc: &Document, ids: &[String], lights: &Lights, dmx: &Universes, change: ColorChange, mem: &mut Memory) -> Option<Edit> {
    let items = write(doc, ids, lights, dmx, |values, f, mode, ty| {
        let look = mem.looks.get(&f.id).filter(|(sig, _)| sig == values).map(|(_, l)| *l);
        let (next, look) = adjust_color(mode, values, change, ty.pixels, console_options(f, ty), look);
        *values = next;
        mem.looks.insert(f.id.clone(), (values.clone(), look));
        true
    });
    edit_of(items)
}

/// Flash : plein feu tant que la touche est enfoncée, puis retour à l'état d'avant.
pub fn flash(doc: &Document, ids: &[String], lights: &Lights, dmx: &Universes, on: bool, mem: &mut Memory) -> Option<Edit> {
    if on {
        for f in ids.iter().filter_map(|id| doc.fixture(id)) {
            if !mem.flashes.contains_key(&f.id) {
                let prev = console_held(f).then(|| f.state["console"]["values"].as_array().map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0) as u16).collect()).unwrap_or_default());
                mem.flashes.insert(f.id.clone(), prev);
            }
        }
        return set_intensity(doc, ids, lights, dmx, 1.0, mem);
    }
    let mut items = Vec::new();
    for (id, prev) in mem.flashes.drain() {
        let Some(f) = doc.fixture(&id) else { continue };
        match prev {
            None => {
                if console_held(f) {
                    let mut state = Map::new();
                    state.insert("console".into(), Value::Null);
                    items.push((id, Changes { state: Some(state), ..Changes::default() }));
                }
            }
            Some(values) => {
                let mode = f.state["console"]["mode"].as_str().map(str::to_owned).or_else(|| f.settings.dmx_mode.clone()).unwrap_or_default();
                let mut state = Map::new();
                state.insert("console".into(), json!({ "mode": mode, "values": values }));
                items.push((id, Changes { state: Some(state), ..Changes::default() }));
            }
        }
    }
    edit_of(items)
}

/// Valeur maximale du fader n° `index` (0..) de la console affichée.
pub fn fader_max(doc: &Document, selection: &[String], index: usize) -> Option<f32> {
    let (_, mode, _) = family(doc, selection)?;
    mode.compiled().slots.get(index).map(|s| s.max() as f32)
}
