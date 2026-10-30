//! Ce qu'affiche un panneau : écran (lignes de texte des Astera, primitives de dessin des autres
//! marques), voyants, et la face avant (touches, molettes, voyants, écran) à dessiner.

use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

// ------------------------------------------------------------------ écran
/// Contenu de l'écran à un instant.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Screen {
    /// pas d'écran (AX1, AsteraBox, BroadCaster…)
    #[default]
    None,
    /// écran OLED / LCD des Astera V5 : lignes de texte
    Text(TextScreen),
    /// écran des autres marques : primitives de dessin en pixels de l'écran simulé
    Draw(DrawScreen),
}

impl Screen {
    /// Forme JSON de la version JavaScript (essais de parité, journal).
    pub fn to_json(&self) -> Value {
        match self {
            Screen::None => Value::Null,
            Screen::Text(t) => t.to_json(),
            Screen::Draw(d) => json!({ "off": d.off, "dim": d.dim, "prims": d.prims }),
        }
    }
}

/// Écran à lignes de texte (Astera V5 ; panneau générique).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextScreen {
    /// écran éteint
    pub off: bool,
    /// écran assombri (aucune touche depuis 30 s)
    pub dim: bool,
    /// icônes de la ligne du haut (batterie, prise, Bluetooth) ; None : pas de ligne d'icônes
    pub icons: Option<Icons>,
    /// deux grandes lignes (sinon petites lignes)
    pub big: bool,
    pub lines: Vec<TextLine>,
}

impl TextScreen {
    pub fn to_json(&self) -> Value {
        if self.off {
            return json!({ "off": true });
        }
        let lines: Vec<Value> = self
            .lines
            .iter()
            .map(|l| {
                let mut o = json!({ "text": l.text, "inverse": l.inverse });
                if let Some(v) = &l.value {
                    o["value"] = json!(v);
                }
                if l.selected {
                    o["selected"] = json!(true);
                }
                if l.emphasis {
                    o["big"] = json!(true);
                }
                o
            })
            .collect();
        let icons = self.icons.as_ref().map_or(Value::Null, |i| json!({ "battery": i.battery, "plug": i.plug, "bt": i.bt, "btWaves": i.bt_waves, "bars": i.bars }));
        json!({ "dim": self.dim, "icons": icons, "size": if self.big { "big" } else { "small" }, "lines": lines })
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextLine {
    pub text: String,
    /// ligne en inversé (sélection, valeur en cours de réglage)
    pub inverse: bool,
    /// valeur à droite (panneau générique)
    pub value: Option<String>,
    pub selected: bool,
    /// ligne en grand (panneau générique)
    pub emphasis: bool,
}

impl TextLine {
    pub fn new(text: impl Into<String>) -> TextLine {
        TextLine { text: text.into(), ..TextLine::default() }
    }

    pub fn inv(text: impl Into<String>, inverse: bool) -> TextLine {
        TextLine { text: text.into(), inverse, ..TextLine::default() }
    }
}

/// Icônes de l'écran Astera.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Icons {
    /// niveau de batterie (%)
    pub battery: f64,
    /// en charge (prise)
    pub plug: bool,
    /// Bluetooth (BTB lié)
    pub bt: bool,
    /// ondes : appli connectée
    pub bt_waves: bool,
    /// jauge à barres de l'afficheur LCD (AX5, AX7, AX10)
    pub bars: bool,
}

/// Écran dessiné (marques autres qu'Astera).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DrawScreen {
    pub off: bool,
    /// assombri (veille de l'écran, verrou)
    pub dim: bool,
    pub prims: Vec<Prim>,
}

/// Alignement d'un texte sur son point `x`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum Align {
    #[default]
    #[serde(rename = "l")]
    Left,
    #[serde(rename = "c")]
    Center,
    #[serde(rename = "r")]
    Right,
}

impl Align {
    fn is_left(&self) -> bool {
        *self == Align::Left
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Primitive de dessin. Couleurs : `fg`, `bg`, `dim`, `accent` (palette de l'écran) ou `#rrggbb`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum Prim {
    Text(Text),
    Rect(Rect),
    Line(Line),
    Ring(Ring),
}

/// Texte : `y` est le haut des lettres, `s` la taille en pixels ; `w` : largeur maximale (le texte
/// rapetisse pour y tenir) ; `inv` : couleur du fond (texte sur un rectangle plein).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Text {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub s: f64,
    #[serde(skip_serializing_if = "Align::is_left")]
    pub align: Align,
    #[serde(skip_serializing_if = "is_false")]
    pub inv: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    /// « seg » (afficheur à segments) ou « mono »
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    /// espacement des lettres (pixels)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Line {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    /// épaisseur (défaut 1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Anneau de progression (`v` de 0 à 1, départ en haut, sens horaire) sur une piste.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Ring {
    pub x: f64,
    pub y: f64,
    pub r: f64,
    pub v: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
}

// constructeurs courts (code des marques)
impl Text {
    pub fn c(mut self) -> Self {
        self.align = Align::Center;
        self
    }
    pub fn r(mut self) -> Self {
        self.align = Align::Right;
        self
    }
    pub fn b(mut self) -> Self {
        self.bold = true;
        self
    }
    pub fn bold_if(mut self, b: bool) -> Self {
        self.bold = b;
        self
    }
    pub fn inv(mut self, inv: bool) -> Self {
        self.inv = inv;
        self
    }
    pub fn col(mut self, c: &str) -> Self {
        self.color = Some(c.to_string());
        self
    }
    pub fn col_opt(mut self, c: Option<&str>) -> Self {
        self.color = c.map(str::to_string);
        self
    }
    pub fn w(mut self, w: f64) -> Self {
        self.w = Some(w);
        self
    }
    pub fn font(mut self, f: &str) -> Self {
        self.font = Some(f.to_string());
        self
    }
    pub fn sp(mut self, s: f64) -> Self {
        self.spacing = Some(s);
        self
    }
}

/// Texte à `(x, y)`, taille `s`.
pub(crate) fn text(x: f64, y: f64, txt: impl Into<String>, s: f64) -> Text {
    Text { x, y, text: txt.into(), s, ..Text::default() }
}

pub(crate) fn rect_fill(x: f64, y: f64, w: f64, h: f64, fill: &str) -> Prim {
    Prim::Rect(Rect { x, y, w, h, fill: Some(fill.into()), stroke: None })
}

pub(crate) fn rect_stroke(x: f64, y: f64, w: f64, h: f64, stroke: &str) -> Prim {
    Prim::Rect(Rect { x, y, w, h, fill: None, stroke: Some(stroke.into()) })
}

pub(crate) fn line(x1: f64, y1: f64, x2: f64, y2: f64, w: Option<f64>) -> Prim {
    Prim::Line(Line { x1, y1, x2, y2, w, color: None })
}

pub(crate) fn line_col(x1: f64, y1: f64, x2: f64, y2: f64, w: f64, color: &str) -> Prim {
    Prim::Line(Line { x1, y1, x2, y2, w: Some(w), color: Some(color.into()) })
}

impl From<Text> for Prim {
    fn from(t: Text) -> Prim {
        Prim::Text(t)
    }
}

// ------------------------------------------------------------------ voyants
/// Voyant allumé : couleur, clignotement (Hz, 0 = fixe).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Led {
    pub color: String,
    #[serde(rename = "blinkHz")]
    pub blink_hz: f64,
}

impl Led {
    pub fn on(color: &str) -> Led {
        Led { color: color.into(), blink_hz: 0.0 }
    }
    pub fn blink(color: &str, hz: f64) -> Led {
        Led { color: color.into(), blink_hz: hz }
    }
}

/// Voyants allumés, par identifiant (absent : éteint).
pub type Leds = BTreeMap<String, Led>;

pub(crate) fn set_led(leds: &mut Leds, id: &str, led: Option<Led>) {
    match led {
        Some(l) => {
            leds.insert(id.to_string(), l);
        }
        None => {
            leds.remove(id);
        }
    }
}

// ------------------------------------------------------------------ face avant
/// Palette de l'écran simulé.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Palette {
    pub bg: String,
    pub fg: String,
    pub dim: String,
    pub accent: String,
    /// fond de l'écran éteint
    pub off: String,
}

/// Nature de l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DisplayKind {
    /// OLED Astera : lignes de texte (`Screen::Text`)
    Oled,
    /// afficheur LCD 2 lignes rétroéclairé (AX5, AX7, AX10) : `Screen::Text`
    Lcd,
    /// écran dessiné (`Screen::Draw`)
    Draw,
}

/// Écran simulé : taille en pixels, agrandissement conseillé, palette.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DisplaySpec {
    pub kind: DisplayKind,
    pub w: f64,
    pub h: f64,
    /// pixels d'interface par pixel d'écran (défaut 2)
    pub zoom: f64,
    pub palette: Option<Palette>,
}

/// Élément de la face avant.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "el", rename_all = "lowercase")]
pub enum Element {
    Led {
        id: String,
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    /// touche : appui (`press`) puis relâchement (`release`) ; les appuis longs sont mesurés par le panneau
    Key {
        id: String,
        label: String,
        /// forme : « round », « rect », « small », « small yellow »…
        shape: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// seconde légende (claviers à double légende : COLOR, SPEED…)
        #[serde(skip_serializing_if = "Option::is_none")]
        alt: Option<String>,
    },
    /// molette : `turn` (crans) ; `push` : cliquable (`press` / `release`, `cancel` si elle tourne pendant l'appui)
    Knob {
        id: String,
        label: String,
        push: bool,
        /// repère (potentiomètre)
        pointer: bool,
        /// couleur du capuchon : « blue », « red », « yellow »…
        cap: String,
        /// « s », « m », « l »
        size: String,
    },
    /// l'écran (voir `Panel::display`)
    Display,
    Text {
        text: String,
    },
    /// colonne d'éléments (`row` : disposés en ligne)
    Col {
        items: Vec<Element>,
        row: bool,
    },
    /// fenêtre du récepteur infrarouge (décor)
    IrWindow,
    /// TouchSlider de l'HydraPanel : `Panel::slider`
    Slider,
    /// bouton « secouer le tube » (accéléromètre de l'AX1) : `Panel::shake`
    Shake {
        label: String,
    },
}

/// Face avant d'un panneau.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Face {
    /// style (« arri », « aputure »… ; Astera : disposition du clavier « tube », « ax2 », « dual »…)
    pub style: String,
    /// rangées d'éléments, de haut en bas
    pub rows: Vec<Vec<Element>>,
    pub display: Option<DisplaySpec>,
    /// explication affichée sous la face
    pub note: String,
}

impl Face {
    /// Toutes les touches et molettes (à plat).
    pub fn elements(&self) -> Vec<&Element> {
        fn walk<'a>(e: &'a Element, out: &mut Vec<&'a Element>) {
            match e {
                Element::Col { items, .. } => items.iter().for_each(|i| walk(i, out)),
                _ => out.push(e),
            }
        }
        let mut out = Vec::new();
        for row in &self.rows {
            for e in row {
                walk(e, &mut out);
            }
        }
        out
    }
}

// constructeurs des faces
pub(crate) fn led(id: &str, label: &str) -> Element {
    Element::Led { id: id.into(), label: label.into(), title: None }
}

pub(crate) fn key(id: &str, label: &str, shape: &str) -> Element {
    Element::Key { id: id.into(), label: label.into(), shape: shape.into(), title: None, alt: None }
}

/// Molette : `opts` parmi « push », « pointer », « cap=… », « size=… ».
pub(crate) fn knob(id: &str, label: &str, opts: &str) -> Element {
    let mut push = false;
    let mut pointer = false;
    let mut cap = String::new();
    let mut size = "m".to_string();
    for o in opts.split_whitespace() {
        match o.split_once('=') {
            Some(("cap", c)) => cap = c.into(),
            Some(("size", s)) => size = s.into(),
            _ if o == "push" => push = true,
            _ if o == "pointer" => pointer = true,
            _ => {}
        }
    }
    Element::Knob { id: id.into(), label: label.into(), push, pointer, cap, size }
}

pub(crate) fn col(items: Vec<Element>) -> Element {
    Element::Col { items, row: false }
}

pub(crate) fn col_row(items: Vec<Element>) -> Element {
    Element::Col { items, row: true }
}

pub(crate) fn label(text: &str) -> Element {
    Element::Text { text: text.into() }
}

pub(crate) fn palette(bg: &str, fg: &str, dim: &str, accent: &str, off: &str) -> Option<Palette> {
    Some(Palette { bg: bg.into(), fg: fg.into(), dim: dim.into(), accent: accent.into(), off: off.into() })
}
