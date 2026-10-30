//! Menus embarqués des projecteurs (portage de `shared/onboard/`) : écrans, voyants, touches et
//! molettes simulés, et ce qu'ils changent dans les réglages (`settings`) et l'état (`state`) du
//! projecteur. Logique pure, sans interface : l'application relaie les gestes, dessine l'écran et
//! la face avant, et applique les changements renvoyés.
//!
//! - [`Panel`] : le panneau d'un projecteur (Astera V5 à écran, Astera à bouton unique, AsteraBox,
//!   panneaux ARRI, Aputure, Hive, DMG, Kino Flo, LiteGear, Dedolight, commandes génériques) ;
//! - [`Onboard`] : registre des panneaux et de l'état d'exécution des Astera, par projecteur ;
//! - [`ir_remote`] : télécommandes infrarouges ARC1 / ARC3.
//!
//! Chaque geste reçoit l'heure (`Env::time`) : appuis longs, messages et clignotements ne lisent
//! jamais l'horloge. `tick` est à appeler à chaque image (avant `display` et `override_pixels`).
//!
//! Chaque geste renvoie un [`Outcome`] : réglages et état changés (clés de premier niveau, à
//! appliquer avec `Edit::UpdateFixture { changes: outcome.changes() }`), et pour l'AsteraBox les
//! projecteurs à lier / délier (`Outcome::others`, puis [`Onboard::relatch`]).
//!
//! ```
//! use arklight_core::engine::Runtime;
//! use arklight_core::{library, project::Fixture};
//! use arklight_onboard::{Env, Onboard, Screen, apply_shallow};
//!
//! let mut titan: Fixture = serde_json::from_value(serde_json::json!({ "id": "f1", "type": "astera.titan" })).unwrap();
//! titan.apply_defaults(library().fixture("astera.titan").unwrap());
//! let (mut onboard, mut rt) = (Onboard::new(), Runtime::default());
//! let live = std::collections::HashSet::new();
//! for (time, key, down) in [(1.0, "enter", true), (1.1, "enter", false)] {
//!     let env = Env { time, fixtures: std::slice::from_ref(&titan), live: &live, app_connected: false };
//!     let out = if down { onboard.press(&titan, &mut rt, &env, key) } else { onboard.release(&titan, &mut rt, &env, key) };
//!     apply_shallow(&mut titan, &out.settings, &out.state); // l'application : Document::apply
//! }
//! let env = Env { time: 1.2, fixtures: std::slice::from_ref(&titan), live: &live, app_connected: false };
//! let Screen::Text(screen) = onboard.display(&titan, &mut rt, &env) else { unreachable!() };
//! assert_eq!(screen.lines[1].text, "DMX ADDRESS");
//! ```

pub mod astera_ax1;
pub mod astera_box;
pub mod astera_v5;
pub mod engine;
mod faces;
pub mod fixture;
pub mod ir_remote;
mod js;
mod panel;
mod panels;
pub mod screen;

pub use astera_v5::SliderZone;
pub use fixture::{AsteraRuntime, Ctx, Env, Outcome, Relayed, apply_shallow};
pub use ir_remote::{IrResult, apply_ir_key};
pub use panels::PANEL_TYPES;
pub use screen::{DisplaySpec, Element, Face, Led, Leds, Prim, Screen};

use arklight_core::engine::Runtime;
use arklight_core::library::{FixtureType, library};
use arklight_core::project::Fixture;
use std::collections::HashMap;

/// Famille de panneau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelKind {
    /// Astera à écran OLED / LCD (Titan, Helios, Hyperion, AX2, AX5, Quik…)
    AsteraV5,
    /// Astera à bouton unique (AX1, AX3, NYX, SolaBulb) : la lumière sert d'affichage
    AsteraButton,
    /// AsteraBox ART7
    AsteraBox,
    /// panneau d'une autre marque (identifiant de la définition : « arri-skypanel »…)
    Brand(&'static str),
    /// commandes génériques décrites par `controls.buttons` (moteur de `engine.js`)
    Generic,
}

enum Inner {
    V5(Box<astera_v5::AsteraV5>),
    Ax1(Box<astera_ax1::AsteraAx1>),
    Box(Box<astera_box::AsteraBox>),
    Brand(Box<dyn panel::BrandPanel>),
    Generic(Box<engine::OnboardController>),
}

/// Panneau de commande d'un projecteur.
pub struct Panel {
    type_id: String,
    face: Face,
    inner: Inner,
}

impl Panel {
    /// Panneau d'un modèle ; None : pas de commandes embarquées simulées (tungstène, HMI,
    /// LunaBulb, modèles génériques…).
    pub fn for_type(ty: &'static FixtureType) -> Option<Panel> {
        let platform = ty.platform.as_deref().unwrap_or("");
        let controls = ty.extra.get("controls");
        let (inner, face) = if platform == "astera-v5-oled" {
            (Inner::V5(Box::new(astera_v5::AsteraV5::new(ty))), faces::astera_face(ty))
        } else if platform == "astera-v5-button" {
            (Inner::Ax1(Box::new(astera_ax1::AsteraAx1::new(ty))), faces::astera_face(ty))
        } else if platform == "astera-box" {
            (Inner::Box(Box::new(astera_box::AsteraBox::new())), faces::astera_face(ty))
        } else if let Some(p) = panels::panel_for(ty) {
            let face = p.face();
            (Inner::Brand(p), face)
        } else if controls.is_some_and(|c| c.get("buttons").is_some_and(|b| !b.is_null())) {
            (Inner::Generic(Box::new(engine::OnboardController::new(ty.pixels, controls.cloned().unwrap_or_default()))), faces::astera_face(ty))
        } else {
            return None;
        };
        Some(Panel { type_id: ty.id.clone(), face, inner })
    }

    /// Panneau du projecteur (modèle de la bibliothèque).
    pub fn new(fixture: &Fixture) -> Option<Panel> {
        Panel::for_type(library().fixture(&fixture.type_id)?)
    }

    /// Commandes génériques décrites en JSON (`controls` : `buttons`, `leds`, `display`, `menu`,
    /// `actions`, `holdTimes`), pour un modèle à `pixels` pixels.
    pub fn generic(type_id: &str, pixels: usize, controls: serde_json::Value) -> Panel {
        let face = faces::generic_face(&controls);
        Panel { type_id: type_id.into(), face, inner: Inner::Generic(Box::new(engine::OnboardController::new(pixels, controls))) }
    }

    pub fn type_id(&self) -> &str {
        &self.type_id
    }

    pub fn kind(&self) -> PanelKind {
        match &self.inner {
            Inner::V5(_) => PanelKind::AsteraV5,
            Inner::Ax1(_) => PanelKind::AsteraButton,
            Inner::Box(_) => PanelKind::AsteraBox,
            Inner::Brand(b) => PanelKind::Brand(b.id()),
            Inner::Generic(_) => PanelKind::Generic,
        }
    }

    /// Face avant à dessiner (touches, molettes, voyants, écran).
    pub fn face(&self) -> &Face {
        &self.face
    }

    fn refresh(&mut self, cx: &Ctx) {
        match &mut self.inner {
            Inner::V5(c) => c.refresh(cx),
            Inner::Ax1(c) => c.refresh(cx),
            Inner::Box(c) => c.refresh(cx),
            Inner::Generic(c) => c.refresh(cx),
            Inner::Brand(_) => {}
        }
    }

    fn take(&mut self) -> Outcome {
        match &mut self.inner {
            Inner::V5(c) => c.take(),
            Inner::Ax1(c) => c.take(),
            Inner::Box(c) => c.take(),
            Inner::Generic(c) => c.take(),
            Inner::Brand(_) => Outcome::default(),
        }
    }

    /// Appui sur une touche (ou une molette cliquable).
    pub fn press(&mut self, cx: &mut Ctx, id: &str) -> Outcome {
        self.refresh(cx);
        match &mut self.inner {
            Inner::V5(c) => c.press(cx, id),
            Inner::Ax1(c) => c.press(cx, id),
            Inner::Box(c) => c.press(cx, id),
            Inner::Generic(c) => c.press(cx, id),
            Inner::Brand(b) => return b.press(cx, id),
        }
        self.take()
    }

    /// Relâchement : appui court ou long selon la durée.
    pub fn release(&mut self, cx: &mut Ctx, id: &str) -> Outcome {
        self.refresh(cx);
        match &mut self.inner {
            Inner::V5(c) => c.release(cx, id),
            Inner::Ax1(c) => c.release(cx, id),
            Inner::Box(c) => c.release(cx, id),
            Inner::Generic(c) => c.release(cx, id),
            Inner::Brand(b) => return b.release(cx, id),
        }
        self.take()
    }

    /// Appui annulé : la molette a tourné pendant qu'on l'enfonçait.
    pub fn cancel(&mut self, id: &str) {
        if let Inner::Brand(b) = &mut self.inner {
            b.cancel(id);
        }
    }

    /// Molette tournée de `steps` crans (positif : sens horaire / vers le haut).
    pub fn turn(&mut self, cx: &mut Ctx, id: &str, steps: i32) -> Outcome {
        match &mut self.inner {
            Inner::Brand(b) => b.turn(cx, id, steps),
            _ => Outcome::default(),
        }
    }

    /// À chaque image : appuis maintenus, répétition des touches, fins de messages.
    pub fn tick(&mut self, cx: &mut Ctx) -> Outcome {
        self.refresh(cx);
        match &mut self.inner {
            Inner::V5(c) => c.tick(cx),
            Inner::Ax1(c) => c.tick(cx),
            Inner::Box(c) => c.tick(cx),
            Inner::Generic(c) => c.tick(cx),
            Inner::Brand(b) => return b.tick(cx),
        }
        self.take()
    }

    /// Secouer le projecteur (accéléromètre de l'AX1 : adressage DMX).
    pub fn shake(&mut self, cx: &mut Ctx) {
        self.refresh(cx);
        if let Inner::Ax1(c) = &mut self.inner {
            c.shake();
        }
    }

    /// TouchSlider (HydraPanel…) ; faux s'il est inactif sur l'écran affiché.
    pub fn slider(&mut self, cx: &mut Ctx, zone: SliderZone, steps: u32) -> (bool, Outcome) {
        self.refresh(cx);
        let ok = match &mut self.inner {
            Inner::V5(c) => c.slider(cx, zone, steps),
            _ => false,
        };
        (ok, self.take())
    }

    /// Contenu de l'écran.
    pub fn display(&mut self, cx: &mut Ctx) -> Screen {
        self.refresh(cx);
        match &mut self.inner {
            Inner::V5(c) => c.display(cx),
            Inner::Generic(c) => c.display(),
            Inner::Brand(b) => b.display(cx),
            Inner::Ax1(_) | Inner::Box(_) => Screen::None,
        }
    }

    /// Voyants allumés.
    pub fn leds(&mut self, cx: &mut Ctx) -> Leds {
        self.refresh(cx);
        match &mut self.inner {
            Inner::V5(c) => c.leds(),
            Inner::Box(c) => c.leds(cx),
            Inner::Generic(c) => c.leds(),
            Inner::Brand(b) => b.leds(cx),
            Inner::Ax1(_) => Leds::new(),
        }
    }

    /// Message d'aide sous le panneau (état, dernier message).
    pub fn hint(&mut self, cx: &mut Ctx) -> String {
        self.refresh(cx);
        match &self.inner {
            Inner::V5(c) => c.hint(),
            Inner::Ax1(c) => c.hint(),
            Inner::Box(c) => c.hint(cx),
            Inner::Generic(c) => c.hint(),
            Inner::Brand(b) => b.hint(),
        }
    }

    /// Durée d'appui en cours sur une touche (aide visuelle des appuis longs).
    pub fn held_for(&self, id: &str, time: f64) -> f64 {
        match &self.inner {
            Inner::V5(c) => c.held_for(id, time),
            Inner::Ax1(c) => c.held_for(id, time),
            Inner::Box(c) => c.held_for(id, time),
            Inner::Generic(c) => c.held_for(id, time),
            Inner::Brand(b) => b.held_for(id, time),
        }
    }

    /// Remplace la sortie des pixels (sRGB linéaire, pixels × 3) pendant un retour visuel (jauge,
    /// BlueMode, adressage…) ; faux : sortie normale. À appeler après `tick`, sur la sortie du
    /// moteur lumière.
    pub fn override_pixels(&mut self, cx: &mut Ctx, out: &mut [f64], pixels: usize) -> bool {
        self.refresh(cx);
        match &mut self.inner {
            Inner::Ax1(c) => c.override_pixels(out, pixels, cx.env.time),
            Inner::Generic(c) => c.override_pixels(out, pixels, cx.env.time),
            _ => false,
        }
    }

    /// Écran ou page affiché (« status », « menu », « home »…).
    pub fn screen_name(&self) -> &'static str {
        match &self.inner {
            Inner::V5(c) => c.screen_name(),
            Inner::Brand(b) => b.screen_name(),
            _ => "",
        }
    }
}

/// Registre des panneaux et de l'état d'exécution propre aux menus Astera, par projecteur
/// (comme `OnboardRegistry` de l'interface JavaScript). Non sauvegardé.
#[derive(Default)]
pub struct Onboard {
    panels: HashMap<String, Panel>,
    astera: HashMap<String, AsteraRuntime>,
}

impl Onboard {
    pub fn new() -> Onboard {
        Onboard::default()
    }

    /// Panneau d'un projecteur (créé au premier appel) ; None : pas de commandes embarquées.
    pub fn panel(&mut self, fixture: &Fixture) -> Option<&mut Panel> {
        if !self.panels.contains_key(&fixture.id) {
            let p = Panel::new(fixture)?;
            self.panels.insert(fixture.id.clone(), p);
        }
        self.panels.get_mut(&fixture.id)
    }

    /// Panneau déjà créé (vue 3D : `tick` et `override_pixels` des seuls panneaux ouverts).
    pub fn peek(&mut self, id: &str) -> Option<&mut Panel> {
        self.panels.get_mut(id)
    }

    /// État d'exécution Astera d'un projecteur (télécommande IR, liaison CRMX par l'application).
    pub fn astera(&mut self, id: &str) -> &mut AsteraRuntime {
        self.astera.entry(id.to_string()).or_default()
    }

    /// Le projecteur a été supprimé.
    pub fn forget(&mut self, id: &str) {
        self.panels.remove(id);
        self.astera.remove(id);
    }

    /// Autre projet ouvert.
    pub fn clear(&mut self) {
        self.panels.clear();
        self.astera.clear();
    }

    /// Entrée à redétecter après une liaison / déliaison CRMX (touche LINK d'une AsteraBox,
    /// commande « Lier / Délier » de l'application) : `unlinked` affiche « CRMX: NOT LINKED ».
    pub fn relatch(&mut self, id: &str, runtime: &mut Runtime, unlinked: bool) {
        let a = self.astera(id);
        a.unlatch(runtime);
        a.just_unlinked = unlinked;
    }

    fn with<R>(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env, f: impl FnOnce(&mut Panel, &mut Ctx) -> R) -> Option<R> {
        let astera = self.astera.entry(fixture.id.clone()).or_default();
        if !self.panels.contains_key(&fixture.id) {
            let p = Panel::new(fixture)?;
            self.panels.insert(fixture.id.clone(), p);
        }
        let panel = self.panels.get_mut(&fixture.id)?;
        let mut cx = Ctx { fixture, runtime, astera, env };
        Some(f(panel, &mut cx))
    }

    pub fn press(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env, key: &str) -> Outcome {
        self.with(fixture, runtime, env, |p, cx| p.press(cx, key)).unwrap_or_default()
    }

    pub fn release(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env, key: &str) -> Outcome {
        self.with(fixture, runtime, env, |p, cx| p.release(cx, key)).unwrap_or_default()
    }

    pub fn cancel(&mut self, fixture_id: &str, key: &str) {
        if let Some(p) = self.panels.get_mut(fixture_id) {
            p.cancel(key);
        }
    }

    pub fn turn(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env, knob: &str, steps: i32) -> Outcome {
        self.with(fixture, runtime, env, |p, cx| p.turn(cx, knob, steps)).unwrap_or_default()
    }

    /// À chaque image, pour les panneaux déjà créés.
    pub fn tick(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env) -> Outcome {
        if !self.panels.contains_key(&fixture.id) {
            return Outcome::default();
        }
        self.with(fixture, runtime, env, |p, cx| p.tick(cx)).unwrap_or_default()
    }

    pub fn shake(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env) {
        self.with(fixture, runtime, env, |p, cx| p.shake(cx));
    }

    pub fn slider(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env, zone: SliderZone, steps: u32) -> (bool, Outcome) {
        self.with(fixture, runtime, env, |p, cx| p.slider(cx, zone, steps)).unwrap_or_default()
    }

    pub fn display(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env) -> Screen {
        self.with(fixture, runtime, env, |p, cx| p.display(cx)).unwrap_or_default()
    }

    pub fn leds(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env) -> Leds {
        self.with(fixture, runtime, env, |p, cx| p.leds(cx)).unwrap_or_default()
    }

    pub fn hint(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env) -> String {
        self.with(fixture, runtime, env, |p, cx| p.hint(cx)).unwrap_or_default()
    }

    /// Sortie des pixels remplacée par un retour visuel (seulement les panneaux déjà créés).
    pub fn override_pixels(&mut self, fixture: &Fixture, runtime: &mut Runtime, env: &Env, out: &mut [f64], pixels: usize) -> bool {
        if !self.panels.contains_key(&fixture.id) {
            return false;
        }
        self.with(fixture, runtime, env, |p, cx| p.override_pixels(cx, out, pixels)).unwrap_or(false)
    }

    /// Touche de télécommande infrarouge (ARC1 / ARC3) envoyée à un projecteur.
    pub fn ir_key(&mut self, fixture: &Fixture, runtime: &mut Runtime, key: &str, time: f64) -> IrResult {
        let Some(ty) = library().fixture(&fixture.type_id) else { return IrResult::default() };
        let ext = self.astera.entry(fixture.id.clone()).or_default();
        apply_ir_key(fixture, ty, runtime, ext, key, time)
    }
}
