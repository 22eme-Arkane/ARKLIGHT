//! Panneaux de commande des projecteurs « non Astera » (ARRI, Aputure, Hive, DMG, Kino Flo,
//! LiteGear, Dedolight), portage de `shared/onboard/panel.js` : touches, molettes (tourner,
//! appuyer, maintenir), écran et voyants, décrits marque par marque dans `panels/`. L'interface
//! relaie les gestes et dessine l'écran à partir de primitives simples (texte, rectangle, trait,
//! anneau).

use crate::fixture::{Ctx, Fx, Outcome};
use crate::js::{self, clamp, jv};
use crate::screen::{DrawScreen, Face, Leds, Prim, Screen, line, rect_fill, text};
use arklight_core::library::FixtureType;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Au-delà (secondes) : appui long.
pub const HOLD_LONG: f64 = 1.0;

/// Accélération d'une molette : tourner vite fait de grands pas.
fn acceleration(dt: f64) -> f64 {
    if dt < 0.035 {
        10.0
    } else if dt < 0.08 {
        4.0
    } else if dt < 0.16 {
        2.0
    } else {
        1.0
    }
}

// ------------------------------------------------------------------ menus en arborescence
pub(crate) type Pred<S> = Rc<dyn Fn(&Pc<S>) -> bool>;
pub(crate) type Get<S> = Rc<dyn Fn(&Pc<S>) -> Value>;
/// Enregistre une valeur ; renvoie le message à afficher.
pub(crate) type Set<S> = Rc<dyn Fn(&mut Pc<S>, Value) -> Option<String>>;
pub(crate) type Fmt<S> = Rc<dyn Fn(&Value, &Pc<S>) -> String>;
pub(crate) type Run<S> = Rc<dyn Fn(&mut Pc<S>) -> Option<String>>;
pub(crate) type ItemRef<S> = Rc<Item<S>>;
pub(crate) type Items<S> = Rc<Vec<ItemRef<S>>>;
pub(crate) type OptsFn<S> = Rc<dyn Fn(&Pc<S>) -> Vec<Opt>>;
pub(crate) type NumFn<S> = Rc<dyn Fn(&Pc<S>) -> f64>;
pub(crate) type TextFn<S> = Rc<dyn Fn(&Pc<S>) -> String>;
/// Appuis maintenus d'une touche : (secondes, action).
pub(crate) type Holds<S> = Vec<(f64, fn(&mut Pc<S>, &str))>;

/// Choix d'une liste : (valeur, libellé).
pub(crate) type Opt = (Value, String);

pub(crate) enum Opts<S: 'static> {
    List(Vec<Opt>),
    Dyn(OptsFn<S>),
}

pub(crate) enum Bound<S: 'static> {
    Fixed(f64),
    Dyn(NumFn<S>),
}

impl<S: 'static> Bound<S> {
    fn get(&self, c: &Pc<S>) -> f64 {
        match self {
            Bound::Fixed(v) => *v,
            Bound::Dyn(f) => f(c),
        }
    }
}

pub(crate) enum InfoVal<S: 'static> {
    Text(String),
    Dyn(TextFn<S>),
}

pub(crate) enum Kind<S: 'static> {
    Sub(Items<S>),
    Pick { options: Opts<S> },
    Num { min: Bound<S>, max: Bound<S>, step: f64, accel: bool },
    Info(InfoVal<S>),
    Act(Run<S>),
}

/// Élément de menu : sous-menu, choix dans une liste, valeur numérique, information, action.
/// `when` : visible si vrai ; `short` : libellé court pour les écrans étroits.
pub(crate) struct Item<S: 'static> {
    pub label: String,
    pub short: Option<String>,
    pub when: Option<Pred<S>>,
    pub kind: Kind<S>,
    pub get: Option<Get<S>>,
    pub set: Option<Set<S>>,
    pub fmt: Option<Fmt<S>>,
    pub wrap: Option<bool>,
}

impl<S: Brand> Item<S> {
    fn new(label: &str, kind: Kind<S>) -> Item<S> {
        Item { label: label.into(), short: None, when: None, kind, get: None, set: None, fmt: None, wrap: None }
    }

    pub fn rc(self) -> ItemRef<S> {
        Rc::new(self)
    }

    pub fn get(mut self, f: impl Fn(&Pc<S>) -> Value + 'static) -> Self {
        self.get = Some(Rc::new(f));
        self
    }

    pub fn set(mut self, f: impl Fn(&mut Pc<S>, Value) -> Option<String> + 'static) -> Self {
        self.set = Some(Rc::new(f));
        self
    }

    pub fn fmt(mut self, f: impl Fn(&Value, &Pc<S>) -> String + 'static) -> Self {
        self.fmt = Some(Rc::new(f));
        self
    }

    pub fn when(mut self, f: impl Fn(&Pc<S>) -> bool + 'static) -> Self {
        self.when = Some(Rc::new(f));
        self
    }

    pub fn no_accel(mut self) -> Self {
        if let Kind::Num { accel, .. } = &mut self.kind {
            *accel = false;
        }
        self
    }

    pub fn step(mut self, s: f64) -> Self {
        if let Kind::Num { step, .. } = &mut self.kind {
            *step = s;
        }
        self
    }

    pub fn max_fn(mut self, f: impl Fn(&Pc<S>) -> f64 + 'static) -> Self {
        if let Kind::Num { max, .. } = &mut self.kind {
            *max = Bound::Dyn(Rc::new(f));
        }
        self
    }

    /// Réglage mémorisé dans `settings.panel` (menus sans effet sur la lumière simulée).
    pub fn stored(self, key: &'static str, dflt: Value) -> Self {
        let d = dflt.clone();
        self.get(move |c| c.panel_or(key, d.clone())).set(move |c, v| {
            c.set_panel(crate::fixture::obj([(key, v)]));
            None
        })
    }

    /// Réglage mémorisé, valeur par défaut calculée.
    pub fn stored_fn(self, key: &'static str, dflt: impl Fn(&Pc<S>) -> Value + 'static) -> Self {
        self.get(move |c| c.panel_or(key, dflt(c))).set(move |c, v| {
            c.set_panel(crate::fixture::obj([(key, v)]));
            None
        })
    }
}

pub(crate) fn sub<S: Brand>(label: &str, children: Vec<ItemRef<S>>) -> Item<S> {
    Item::new(label, Kind::Sub(Rc::new(children)))
}

pub(crate) fn pick<S: Brand>(label: &str, options: Vec<Opt>) -> Item<S> {
    Item::new(label, Kind::Pick { options: Opts::List(options) })
}

pub(crate) fn pick_dyn<S: Brand>(label: &str, options: impl Fn(&Pc<S>) -> Vec<Opt> + 'static) -> Item<S> {
    Item::new(label, Kind::Pick { options: Opts::Dyn(Rc::new(options)) })
}

pub(crate) fn num<S: Brand>(label: &str, min: f64, max: f64) -> Item<S> {
    Item::new(label, Kind::Num { min: Bound::Fixed(min), max: Bound::Fixed(max), step: 1.0, accel: true })
}

pub(crate) fn info<S: Brand>(label: &str, value: &str) -> Item<S> {
    Item::new(label, Kind::Info(InfoVal::Text(value.into())))
}

pub(crate) fn info_fn<S: Brand>(label: &str, f: impl Fn(&Pc<S>) -> String + 'static) -> Item<S> {
    Item::new(label, Kind::Info(InfoVal::Dyn(Rc::new(f))))
}

pub(crate) fn act<S: Brand>(label: &str, run: impl Fn(&mut Pc<S>) -> Option<String> + 'static) -> Item<S> {
    Item::new(label, Kind::Act(Rc::new(run)))
}

/// Options identiques à leur libellé (`['On', 'Off']`).
pub(crate) fn opts(list: &[&str]) -> Vec<Opt> {
    list.iter().map(|s| (Value::from(*s), s.to_string())).collect()
}

/// Options (valeur, libellé).
pub(crate) fn opts2(list: &[(&str, &str)]) -> Vec<Opt> {
    list.iter().map(|(v, l)| (Value::from(*v), l.to_string())).collect()
}

/// Options numériques (`[0, 5, 10]`).
pub(crate) fn opts_num(list: &[f64]) -> Vec<Opt> {
    list.iter().map(|v| (jv(*v), js::num_str(*v))).collect()
}

pub(crate) fn options_of<S: Brand>(item: &Item<S>, c: &Pc<S>) -> Vec<Opt> {
    match &item.kind {
        Kind::Pick { options: Opts::List(l) } => l.clone(),
        Kind::Pick { options: Opts::Dyn(f) } => f(c),
        _ => Vec::new(),
    }
}

/// Valeur affichée d'un élément (valeur actuelle si `value` est None).
pub(crate) fn value_text<S: Brand>(item: &Item<S>, c: &Pc<S>, value: Option<&Value>) -> String {
    if let Kind::Info(v) = &item.kind {
        return match v {
            InfoVal::Text(t) => t.clone(),
            InfoVal::Dyn(f) => f(c),
        };
    }
    let cur;
    let value = match value {
        Some(v) => Some(v),
        None => {
            cur = item.get.as_ref().map(|g| g(c));
            cur.as_ref()
        }
    };
    if let Some(f) = &item.fmt {
        return f(value.unwrap_or(&Value::Null), c);
    }
    match &item.kind {
        Kind::Pick { .. } => options_of(item, c).into_iter().find(|o| js::strict_eq(Some(&o.0), value)).map_or_else(|| js::string_or_empty(value), |o| o.1),
        Kind::Num { .. } => js::string(value),
        _ => String::new(),
    }
}

pub(crate) struct Level<S: 'static> {
    pub items: Items<S>,
    pub title: String,
    pub cursor: i64,
}

pub(crate) struct Editing<S: 'static> {
    pub item: ItemRef<S>,
    pub value: Value,
    pub cursor: i64,
}

pub(crate) struct MenuState<S: 'static> {
    pub stack: Vec<Level<S>>,
    pub edit: Option<Editing<S>>,
    wrap: bool,
}

/// Ce qu'a fait un appui dans le menu.
pub(crate) struct Entered {
    /// une valeur a été validée
    pub committed: bool,
    /// message à afficher
    pub msg: Option<String>,
}

impl<S: Brand> MenuState<S> {
    pub fn new(tree: Vec<ItemRef<S>>, title: &str, wrap: bool) -> MenuState<S> {
        MenuState { stack: vec![Level { items: Rc::new(tree), title: title.into(), cursor: 0 }], edit: None, wrap }
    }

    pub fn level(&self) -> &Level<S> {
        self.stack.last().expect("niveau de menu")
    }

    fn level_mut(&mut self) -> &mut Level<S> {
        self.stack.last_mut().expect("niveau de menu")
    }

    pub fn items(&self, c: &Pc<S>) -> Vec<ItemRef<S>> {
        self.level().items.iter().filter(|it| it.when.as_ref().is_none_or(|w| w(c))).cloned().collect()
    }

    pub fn current(&mut self, c: &Pc<S>) -> Option<ItemRef<S>> {
        let items = self.items(c);
        let n = items.len() as f64;
        let cur = clamp(self.level().cursor as f64, 0.0, (n - 1.0).max(0.0)) as i64;
        self.level_mut().cursor = cur;
        items.get(cur as usize).cloned()
    }

    pub fn move_by(&mut self, c: &Pc<S>, d: f64, mult: f64) {
        if self.edit.is_some() {
            return self.edit_move(c, d, mult);
        }
        let n = self.items(c).len() as i64;
        if n == 0 {
            return;
        }
        let i = self.level().cursor + d as i64;
        let wrap = self.wrap;
        self.level_mut().cursor = if wrap { (i + n * 10) % n } else { i.clamp(0, n - 1) };
    }

    fn edit_move(&mut self, c: &Pc<S>, d: f64, mult: f64) {
        let Some(edit) = &mut self.edit else { return };
        let item = edit.item.clone();
        if let Kind::Num { min, max, step, accel } = &item.kind {
            let st = step * if *accel { mult } else { 1.0 };
            let (lo, hi) = (min.get(c), max.get(c));
            let mut v = js::round((js::number(Some(&edit.value)) + d * st) / step) * step;
            if item.wrap == Some(true) {
                v = if v > hi {
                    lo
                } else if v < lo {
                    hi
                } else {
                    v
                };
            }
            let fixed: f64 = js::to_fixed(v, 4).parse().unwrap_or(f64::NAN);
            edit.value = jv(clamp(fixed, lo, hi));
        } else {
            let opts = options_of(&item, c);
            let n = opts.len() as i64;
            edit.cursor = if item.wrap == Some(false) { (edit.cursor + d as i64).clamp(0, (n - 1).max(0)) } else if n > 0 { (edit.cursor + d as i64 + n) % n } else { 0 };
            edit.value = if edit.cursor >= 0 { opts.get(edit.cursor as usize).map_or(Value::Null, |o| o.0.clone()) } else { Value::Null };
        }
    }

    /// Appui (molette, ENTER…) : ouvre, édite, valide.
    pub fn enter(&mut self, c: &mut Pc<S>) -> Option<Entered> {
        if let Some(Editing { item, value, .. }) = self.edit.take() {
            let msg = item.set.as_ref().and_then(|s| s(c, value));
            return Some(Entered { committed: true, msg });
        }
        let it = self.current(c)?;
        match &it.kind {
            Kind::Sub(children) => {
                self.stack.push(Level { items: children.clone(), title: it.label.clone(), cursor: 0 });
                Some(Entered { committed: false, msg: None })
            }
            Kind::Pick { .. } => {
                let opts = options_of(&it, c);
                let cur = it.get.as_ref().map(|g| g(c));
                let i = opts.iter().position(|o| js::strict_eq(Some(&o.0), cur.as_ref())).unwrap_or(0);
                self.edit = Some(Editing { item: it.clone(), value: opts.get(i).map_or(Value::Null, |o| o.0.clone()), cursor: i as i64 });
                Some(Entered { committed: false, msg: None })
            }
            Kind::Num { .. } => {
                let v = js::nn(it.get.as_ref().map(|g| g(c)).as_ref(), 0.0);
                self.edit = Some(Editing { item: it.clone(), value: jv(v), cursor: 0 });
                Some(Entered { committed: false, msg: None })
            }
            Kind::Act(run) => {
                let msg = run(c);
                Some(Entered { committed: false, msg })
            }
            Kind::Info(_) => None,
        }
    }

    /// Retour d'un niveau. Faux s'il n'y a plus rien à fermer (le menu se ferme).
    pub fn back(&mut self) -> bool {
        if self.edit.is_some() {
            self.edit = None;
            return true;
        }
        if self.stack.len() > 1 {
            self.stack.pop();
            return true;
        }
        false
    }
}

// ------------------------------------------------------------------ contrôleur
/// Comportement d'un modèle (état propre à la marque : curseurs, pages…).
pub(crate) trait Brand: Sized + 'static {
    /// taille de l'écran simulé (largeur, hauteur en pixels)
    const SIZE: (f64, f64);
    /// l'écran reste allumé projecteur éteint
    const DISPLAY_WHEN_OFF: bool = false;
    /// durée d'un appui long
    const HOLD_LONG: f64 = HOLD_LONG;
    /// identifiant de la définition JavaScript (« arri-skypanel »…)
    const ID: &'static str;

    fn init(ty: &'static FixtureType) -> Self;
    fn face(ty: &FixtureType) -> Face;
    /// appui (avant le relâchement)
    fn down(_c: &mut Pc<Self>, _id: &str) {}
    /// relâchement d'une touche (ou d'une molette cliquable)
    fn key(c: &mut Pc<Self>, id: &str, long: bool);
    fn turn(_c: &mut Pc<Self>, _id: &str, _steps: f64, _mult: f64) {}
    /// appuis maintenus : (secondes, action)
    fn holds(_id: &str) -> Holds<Self> {
        Vec::new()
    }
    fn tick(_c: &mut Pc<Self>) {}
    fn draw(c: &mut Pc<Self>) -> Drawn;
    fn leds(_c: &mut Pc<Self>) -> Leds {
        Leds::new()
    }
}

/// Écran dessiné par une marque.
#[derive(Default)]
pub(crate) struct Drawn {
    pub prims: Vec<Prim>,
    pub dim: bool,
}

impl Drawn {
    pub fn of(prims: Vec<Prim>) -> Drawn {
        Drawn { prims, dim: false }
    }
}

#[derive(Debug, Clone)]
struct Pressed {
    t0: f64,
    fired: Vec<f64>,
}

pub(crate) struct Msg {
    pub text: String,
    pub until: f64,
}

/// Valeur affichée en grand pendant qu'on tourne une molette.
pub(crate) struct Flash {
    pub text: String,
    pub until: f64,
    /// case remplacée (« L », « R »)
    pub cell: Option<char>,
}

/// Contrôleur d'un panneau de marque (`PanelController`).
pub(crate) struct Pc<S: 'static> {
    pub ty: &'static FixtureType,
    pub fx: Fx,
    live: HashSet<u32>,
    pub t: f64,
    pressed: Vec<(String, Pressed)>,
    last_turn: HashMap<String, f64>,
    pub message: Option<Msg>,
    pub flash: Option<Flash>,
    pub menu: Option<MenuState<S>>,
    menu_gen: u64,
    pub screen: &'static str,
    pub st: S,
}

impl<S: Brand> Pc<S> {
    pub fn new(ty: &'static FixtureType) -> Pc<S> {
        Pc { ty, fx: Fx::default(), live: HashSet::new(), t: 0.0, pressed: Vec::new(), last_turn: HashMap::new(), message: None, flash: None, menu: None, menu_gen: 0, screen: "home", st: S::init(ty) }
    }

    // ---------------------------------------------------------------- accès
    pub fn s(&self, key: &str) -> Option<&Value> {
        self.fx.s(key)
    }

    /// `settings.dmxAddress`
    pub fn address(&self) -> f64 {
        js::number(self.s("dmxAddress"))
    }

    /// `state.manual[key]`
    pub fn m(&self, key: &str) -> Option<&Value> {
        self.fx.st("manual").and_then(|m| m.get(key))
    }

    /// `m[key] ?? d`
    pub fn m_nn(&self, key: &str, d: f64) -> f64 {
        js::nn(self.m(key), d)
    }

    /// `m[key] || d`
    pub fn m_or(&self, key: &str, d: f64) -> f64 {
        js::or(self.m(key), d)
    }

    pub fn hsi(&self) -> bool {
        self.m("mode").and_then(Value::as_str) == Some("hsi")
    }

    /// Projecteur allumé (`state.power !== false`).
    pub fn on(&self) -> bool {
        self.fx.st("power") != Some(&Value::Bool(false))
    }

    /// `settings.panel[key]` (absent : None ; null est une valeur).
    pub fn panel(&self, key: &str) -> Option<&Value> {
        self.s("panel").and_then(|p| p.get(key))
    }

    pub fn panel_or(&self, key: &str, d: Value) -> Value {
        self.panel(key).cloned().unwrap_or(d)
    }

    /// Texte d'un réglage du panneau.
    pub fn panel_str(&self, key: &str, d: &str) -> String {
        match self.panel(key) {
            Some(v) => js::string(Some(v)),
            None => d.to_string(),
        }
    }

    pub fn panel_num(&self, key: &str, d: f64) -> f64 {
        match self.panel(key) {
            Some(v) => js::number(Some(v)),
            None => d,
        }
    }

    pub fn set_panel(&mut self, changes: Map<String, Value>) {
        let p = js::spread(self.s("panel"), &changes);
        self.fx.set1("panel", Value::Object(p));
    }

    pub fn set_settings(&mut self, changes: Map<String, Value>) {
        self.fx.set(changes);
    }

    pub fn set_manual(&mut self, changes: Map<String, Value>) {
        let m = js::spread(self.fx.st("manual"), &changes);
        self.fx.set_state1("manual", Value::Object(m));
    }

    pub fn set_state(&mut self, changes: Map<String, Value>) {
        self.fx.set_state(changes);
    }

    /// Le projecteur reçoit du DMX (console, appli) sur son univers.
    pub fn dmx_live(&self) -> bool {
        self.s("universe").and_then(Value::as_f64).is_some_and(|u| u >= 0.0 && self.live.contains(&(u as u32)))
    }

    pub fn say(&mut self, text: impl Into<String>) {
        self.say_for(text, 2.5);
    }

    pub fn say_for(&mut self, text: impl Into<String>, seconds: f64) {
        self.message = Some(Msg { text: text.into(), until: self.t + seconds });
    }

    pub fn show_big(&mut self, text: impl Into<String>) {
        self.flash = Some(Flash { text: text.into(), until: self.t + 1.5, cell: None });
    }

    pub fn open_menu(&mut self, tree: Vec<ItemRef<S>>, title: &str, wrap: bool) {
        self.set_menu(Some(MenuState::new(tree, title, wrap)));
        self.screen = "menu";
    }

    pub fn close_menu(&mut self) {
        self.set_menu(None);
        self.screen = "home";
    }

    /// `c.menu = …`
    pub fn set_menu(&mut self, menu: Option<MenuState<S>>) {
        self.menu = menu;
        self.menu_gen += 1;
    }

    pub fn pressed_has(&self, id: &str) -> bool {
        self.pressed.iter().any(|(k, _)| k == id)
    }

    pub fn held_for(&self, id: &str) -> f64 {
        self.pressed.iter().find(|(k, _)| k == id).map_or(0.0, |(_, p)| self.t - p.t0)
    }

    // ---------------------------------------------------------------- menu
    /// Opération sur le menu ouvert, qui reçoit aussi le contrôleur (le menu est sorti pendant
    /// l'opération ; s'il a été fermé ou remplacé entre-temps, il n'est pas remis).
    pub fn with_menu<R>(&mut self, f: impl FnOnce(&mut MenuState<S>, &mut Pc<S>) -> R) -> Option<R> {
        let mut menu = self.menu.take()?;
        let generation = self.menu_gen;
        let r = f(&mut menu, self);
        if self.menu_gen == generation {
            self.menu = Some(menu);
        }
        Some(r)
    }

    /// `c.menu.enter(c)`
    pub fn menu_enter(&mut self) -> Option<Entered> {
        self.with_menu(|m, c| m.enter(c)).flatten()
    }

    /// `c.menu.enter(c)` puis le message éventuel.
    pub fn menu_enter_say(&mut self) -> Option<Entered> {
        let r = self.menu_enter();
        if let Some(msg) = r.as_ref().and_then(|r| r.msg.clone()) {
            self.say(msg);
        }
        r
    }

    pub fn menu_move(&mut self, d: f64, mult: f64) {
        self.with_menu(|m, c| m.move_by(c, d, mult));
    }

    /// `c.menu.back()` (pas de menu : faux)
    pub fn menu_back(&mut self) -> bool {
        self.menu.as_mut().is_some_and(MenuState::back)
    }

    // ---------------------------------------------------------------- gestes
    fn begin(&mut self, cx: &Ctx) {
        self.fx = Fx::of(cx.fixture);
        self.live = cx.env.live.clone();
        self.t = cx.env.time;
    }

    fn press(&mut self, id: &str) {
        let p = Pressed { t0: self.t, fired: Vec::new() };
        match self.pressed.iter_mut().find(|(k, _)| k == id) {
            Some(old) => old.1 = p,
            None => self.pressed.push((id.to_string(), p)),
        }
        S::down(self, id);
    }

    fn release(&mut self, id: &str) {
        let Some(i) = self.pressed.iter().position(|(k, _)| k == id) else { return };
        let (_, p) = self.pressed.remove(i);
        if !p.fired.is_empty() {
            return; // un appui maintenu a déjà agi
        }
        let held = self.t - p.t0;
        S::key(self, id, held >= S::HOLD_LONG);
    }

    fn turn(&mut self, id: &str, steps: f64) {
        let last = self.last_turn.get(id).copied().unwrap_or(-1.0);
        self.last_turn.insert(id.to_string(), self.t);
        if steps == 0.0 {
            return;
        }
        S::turn(self, id, steps, acceleration(self.t - last));
    }

    /// Chaque image : appuis maintenus, messages.
    fn tick(&mut self, holds: bool) {
        let t = self.t;
        if holds {
            let ids: Vec<String> = self.pressed.iter().map(|(k, _)| k.clone()).collect();
            for id in ids {
                for (sec, f) in S::holds(&id) {
                    let Some((_, p)) = self.pressed.iter_mut().find(|(k, _)| *k == id) else { break };
                    if !p.fired.contains(&sec) && t - p.t0 >= sec {
                        p.fired.push(sec);
                        f(self, &id);
                    }
                }
            }
        }
        if self.message.as_ref().is_some_and(|m| t > m.until) {
            self.message = None;
        }
        if self.flash.as_ref().is_some_and(|f| t > f.until) {
            self.flash = None;
        }
        S::tick(self);
    }
}

/// Panneau d'une marque, quel que soit son état propre.
pub(crate) trait BrandPanel {
    fn id(&self) -> &'static str;
    fn face(&self) -> Face;
    fn press(&mut self, cx: &mut Ctx, id: &str) -> Outcome;
    fn release(&mut self, cx: &mut Ctx, id: &str) -> Outcome;
    fn cancel(&mut self, id: &str);
    fn turn(&mut self, cx: &mut Ctx, id: &str, steps: i32) -> Outcome;
    fn tick(&mut self, cx: &mut Ctx) -> Outcome;
    fn display(&mut self, cx: &mut Ctx) -> Screen;
    fn leds(&mut self, cx: &mut Ctx) -> Leds;
    fn hint(&self) -> String;
    fn held_for(&self, id: &str, t: f64) -> f64;
    fn screen_name(&self) -> &'static str;
}

impl<S: Brand> BrandPanel for Pc<S> {
    fn id(&self) -> &'static str {
        S::ID
    }

    fn face(&self) -> Face {
        S::face(self.ty)
    }

    fn press(&mut self, cx: &mut Ctx, id: &str) -> Outcome {
        self.begin(cx);
        Pc::press(self, id);
        self.fx.take()
    }

    fn release(&mut self, cx: &mut Ctx, id: &str) -> Outcome {
        self.begin(cx);
        Pc::release(self, id);
        self.fx.take()
    }

    /// Appui annulé (la molette a tourné pendant l'appui).
    fn cancel(&mut self, id: &str) {
        self.pressed.retain(|(k, _)| k != id);
    }

    fn turn(&mut self, cx: &mut Ctx, id: &str, steps: i32) -> Outcome {
        self.begin(cx);
        Pc::turn(self, id, f64::from(steps));
        self.fx.take()
    }

    fn tick(&mut self, cx: &mut Ctx) -> Outcome {
        self.begin(cx);
        Pc::tick(self, true);
        self.fx.take()
    }

    fn display(&mut self, cx: &mut Ctx) -> Screen {
        self.begin(cx);
        // messages et retours de page à jour (les appuis maintenus se déclenchent dans `tick`)
        Pc::tick(self, false);
        if !self.on() && !S::DISPLAY_WHEN_OFF {
            return Screen::Draw(DrawScreen { off: true, dim: false, prims: Vec::new() });
        }
        let d = S::draw(self);
        self.fx.out = Outcome::default();
        Screen::Draw(DrawScreen { off: false, dim: d.dim, prims: d.prims })
    }

    fn leds(&mut self, cx: &mut Ctx) -> Leds {
        self.begin(cx);
        S::leds(self)
    }

    fn hint(&self) -> String {
        self.message.as_ref().map_or_else(String::new, |m| m.text.clone())
    }

    fn held_for(&self, id: &str, t: f64) -> f64 {
        self.pressed.iter().find(|(k, _)| k == id).map_or(0.0, |(_, p)| t - p.t0)
    }

    fn screen_name(&self) -> &'static str {
        self.screen
    }
}

// ------------------------------------------------------------------ dessins communs
/// Options des listes et des écrans de réglage.
#[derive(Clone)]
pub(crate) struct ListOpts {
    pub w: Option<f64>,
    pub h: Option<f64>,
    pub x0: f64,
    pub rows: usize,
    pub row_h: f64,
    pub title_h: f64,
    pub size: f64,
    pub title_size: f64,
    pub underline: bool,
    pub values: bool,
    pub sel_fill: Option<&'static str>,
    pub sel_text: Option<&'static str>,
    pub short: bool,
    pub max_value: usize,
    pub big_size: f64,
    pub confirm_hint: &'static str,
    pub markers: (&'static str, &'static str),
    pub radio: bool,
}

impl Default for ListOpts {
    fn default() -> Self {
        ListOpts {
            w: None,
            h: None,
            x0: 0.0,
            rows: 3,
            row_h: 16.0,
            title_h: 14.0,
            size: 11.0,
            title_size: 10.0,
            underline: true,
            values: true,
            sel_fill: None,
            sel_text: None,
            short: false,
            max_value: 8,
            big_size: 20.0,
            confirm_hint: "Push = OK",
            markers: ("▶", "◇"),
            radio: false,
        }
    }
}

fn first_row(cur: i64, rows: usize, len: usize) -> i64 {
    clamp((cur - (rows / 2) as i64) as f64, 0.0, (len as f64 - rows as f64).max(0.0)) as i64
}

/// Liste de menu : titre, lignes (valeur à droite), ligne choisie en inversé, ascenseur.
pub(crate) fn draw_list<S: Brand>(c: &Pc<S>, menu: &MenuState<S>, o: &ListOpts) -> Vec<Prim> {
    let w = o.w.unwrap_or(S::SIZE.0);
    let x0 = o.x0;
    let mut prims = Vec::new();
    let (title_h, row_h, rows, size) = (o.title_h, o.row_h, o.rows, o.size);
    if menu.edit.is_some() {
        return draw_edit(c, menu, o);
    }
    let items = menu.items(c);
    let cur = menu.level().cursor;
    prims.push(text(x0 + w / 2.0, 1.0, menu.level().title.clone(), o.title_size).c().b().into());
    if o.underline {
        prims.push(line(x0 + 2.0, title_h - 1.5, x0 + w - 2.0, title_h - 1.5, Some(1.0)));
    }
    let first = first_row(cur, rows, items.len());
    let last = (items.len() as i64).min(first + rows as i64);
    for i in first..last {
        let it = &items[i as usize];
        let y = title_h + (i - first) as f64 * row_h;
        let sel = i == cur;
        let inv = sel && o.sel_fill.is_none();
        let color = if sel && o.sel_fill.is_some() { Some(o.sel_text.unwrap_or("fg")) } else { None };
        if sel {
            prims.push(rect_fill(x0, y + 1.0, w - 5.0, row_h - 1.0, o.sel_fill.unwrap_or("fg")));
        }
        let label = match (&it.short, o.short) {
            (Some(s), true) => s.clone(),
            _ => it.label.clone(),
        };
        let ty = y + (row_h - size) / 2.0 + 1.0;
        prims.push(text(x0 + 3.0, ty, label, size).inv(inv).col_opt(color).w(w - 8.0).into());
        // valeur courte à droite de la ligne (« 050 », « P01 », « On ») ; les longues s'affichent en ouvrant
        let val = if !o.values || matches!(it.kind, Kind::Sub(_)) { String::new() } else { value_text(it, c, None) };
        if !val.is_empty() && js::len16(&val) <= o.max_value {
            prims.push(text(x0 + w - 8.0, ty, val, size).r().inv(inv).col_opt(color).w((w - 10.0) * 0.4).into());
        }
    }
    // ascenseur
    if items.len() > rows {
        let n = items.len() as f64;
        let track_h = rows as f64 * row_h;
        let h = (track_h * rows as f64 / n).max(6.0);
        let y = title_h + ((track_h - h) * first as f64) / (n - rows as f64).max(1.0);
        prims.push(rect_fill(x0 + w - 3.0, title_h + 1.0, 2.0, track_h - 1.0, "dim"));
        prims.push(rect_fill(x0 + w - 4.0, y, 4.0, h, "fg"));
    }
    prims
}

/// Choix d'une valeur : liste d'options (◇ / ▶) ou grande valeur numérique.
pub(crate) fn draw_edit<S: Brand>(c: &Pc<S>, menu: &MenuState<S>, o: &ListOpts) -> Vec<Prim> {
    let w = o.w.unwrap_or(S::SIZE.0);
    let h = o.h.unwrap_or(S::SIZE.1);
    let x0 = o.x0;
    let Some(edit) = &menu.edit else { return Vec::new() };
    let item = &edit.item;
    let title_h = o.title_h;
    let mut prims: Vec<Prim> = vec![text(x0 + w / 2.0, 1.0, item.label.clone(), o.title_size).c().b().into(), line(x0 + 2.0, title_h - 1.0, x0 + w - 2.0, title_h - 1.0, Some(1.0))];
    if matches!(item.kind, Kind::Num { .. }) {
        prims.push(text(x0 + w / 2.0, h / 2.0 - 8.0, value_text(item, c, Some(&edit.value)), o.big_size).c().b().into());
        prims.push(text(x0 + w / 2.0, h - 12.0, o.confirm_hint, 9.0).c().col("dim").into());
        return prims;
    }
    let opts = options_of(item, c);
    let (rows, row_h) = (o.rows, o.row_h);
    let cur = edit.cursor;
    let first = first_row(cur, rows, opts.len());
    let (on, off) = o.markers;
    let current = item.get.as_ref().map(|g| g(c));
    let last = (opts.len() as i64).min(first + rows as i64);
    for i in first..last {
        let y = title_h + (i - first) as f64 * row_h;
        let sel = i == cur;
        if sel {
            prims.push(rect_fill(x0, y + 1.0, w - 5.0, row_h - 1.0, o.sel_fill.unwrap_or("fg")));
        }
        let opt = &opts[i as usize];
        let mark = if o.radio {
            if js::strict_eq(Some(&opt.0), current.as_ref()) { on } else { off }
        } else if sel {
            on
        } else {
            off
        };
        let color = if sel && o.sel_fill.is_some() { Some(o.sel_text.unwrap_or("fg")) } else { None };
        prims.push(text(x0 + 3.0, y + (row_h - o.size) / 2.0 + 1.0, format!("{mark}{}", opt.1), o.size).inv(sel && o.sel_fill.is_none()).col_opt(color).w(w - 8.0).into());
    }
    prims
}
