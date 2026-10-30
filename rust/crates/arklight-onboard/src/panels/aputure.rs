//! Panneaux Aputure (docs/recherche/menus/aputure_hive.md), portage de
//! `shared/onboard/panels/aputure.js` :
//!  - « Pro » (LS 600d Pro, 600x Pro, 1200d Pro) : écran LCD couleur, touches Menu / Lighting / Effects /
//!    Back, molette SELECT (et CCT sur le 600x Pro), menu en carrousel d'icônes ;
//!  - OLED (LS 600d, LS 300x, LS C300d) : 4 touches à double légende, molette MENU / BRIGHTNESS (et
//!    PRESET / CCT sur le 300x), menu système en liste ;
//!  - Nova P300c : boîtier OLED, molettes INT, CCT/HUE, G/M/SAT, touches LIGHT MODE, MENU, préréglages 1–4.
//!
//! Les effets se choisissent comme sur le projecteur mais ne sont pas simulés sur la lumière.

use super::common::{Nudge, SERIAL_ALPHABET, cct_range, curve_item, nudge, pad, pct, profile_item, serial_of};
use crate::fixture::obj;
use crate::js::{self, clamp, jv};
use crate::panel::{Brand, Holds, Drawn, HOLD_LONG, ItemRef, Kind, ListOpts, MenuState, Pc, act, draw_edit, draw_list, info, info_fn, num, opts, pick, sub};
use crate::screen::{DisplayKind, DisplaySpec, Element, Face, Prim, Ring, col, key, knob, label, line, line_col, palette, rect_fill, rect_stroke, text};
use arklight_core::library::FixtureType;
use serde_json::{Value, json};

const NOT_SIMULATED: &str = "Mémorisé (non simulé dans ARKLIGHT)";
const FX_LS: [&str; 8] = ["Paparazzi", "Fireworks", "Faulty Bulb", "Lightning", "TV", "Pulsing", "Strobe", "Explosion"];
const CURVE_NAMES: [Option<&str>; 4] = [Some("Exp"), Some("Log"), Some("S-curve"), Some("Linear")];

/// État des panneaux Aputure (K : 0 = Pro, 1 = OLED, 2 = Nova).
pub(crate) struct Ap<const K: u8> {
    car: i64,
    fx_field: &'static str,
    dmx_lock: bool,
    dmx_edit: bool,
    single: bool,
    fx: bool,
    mode_cur: i64,
}

impl<const K: u8> Ap<K> {
    fn new() -> Ap<K> {
        Ap { car: 0, fx_field: "int", dmx_lock: false, dmx_edit: false, single: false, fx: false, mode_cur: 0 }
    }
}

pub(crate) type AputurePro = Ap<0>;
pub(crate) type AputureOled = Ap<1>;
pub(crate) type Nova = Ap<2>;

fn is(v: &Value, s: &str) -> bool {
    v.as_str() == Some(s)
}

fn on_off<const K: u8>(label: &str, key: &'static str, dflt: &str, vals: &[&str]) -> ItemRef<Ap<K>>
where
    Ap<K>: Brand,
{
    pick(label, opts(vals)).stored(key, json!(dflt)).rc()
}

fn exit_item<const K: u8>() -> ItemRef<Ap<K>>
where
    Ap<K>: Brand,
{
    act("Exit", |c| {
        c.close_menu();
        None
    })
    .rc()
}

fn not_sim<const K: u8>(label: &str) -> ItemRef<Ap<K>>
where
    Ap<K>: Brand,
{
    act(label, |_| Some(NOT_SIMULATED.into())).rc()
}

fn msg<const K: u8>(label: &str, m: &'static str) -> ItemRef<Ap<K>>
where
    Ap<K>: Brand,
{
    act(label, move |_| Some(m.into())).rc()
}

/// Choix « radio » ouvert directement (Dimming Curve, Fan Mode…) : un seul élément, déjà en édition.
fn open_pick<const K: u8>(c: &mut Pc<Ap<K>>, item: ItemRef<Ap<K>>)
where
    Ap<K>: Brand,
{
    let title = item.label.clone();
    c.set_menu(Some(MenuState::new(vec![item], &title, false)));
    c.screen = "menu";
    c.menu_enter();
    c.st.single = true;
}

fn first_mode(ty: &FixtureType) -> Value {
    ty.dmx.modes.first().map_or(Value::Null, |m| json!(m.id))
}

fn factory<const K: u8>() -> ItemRef<Ap<K>>
where
    Ap<K>: Brand,
{
    pick("Factory Reset", opts(&["NO", "YES"]))
        .get(|_| json!("NO"))
        .set(|c, v| {
            if !is(&v, "YES") {
                return None;
            }
            let first = first_mode(c.ty);
            c.set_settings(obj([("panel", json!({})), ("dmxAddress", json!(1)), ("dimmingCurve", json!("linear")), ("dmxMode", first)]));
            c.set_manual(obj([("mode", json!("cct")), ("intensity", json!(0.5)), ("cct", json!(5600)), ("tint", json!(0))]));
            Some("Réglages d’usine".into())
        })
        .rc()
}

fn dmx_address_screen<const K: u8>(label: &str, close: bool) -> ItemRef<Ap<K>>
where
    Ap<K>: Brand,
{
    act(label, move |c| {
        c.screen = "dmx";
        if close {
            c.set_menu(None);
        }
        None
    })
    .rc()
}

// ================================================================== « Pro » : écran couleur
struct ProModel {
    cct: bool,
    fw: &'static str,
}

fn pro_model(id: &str) -> ProModel {
    match id {
        "aputure.ls-600x-pro" => ProModel { cct: true, fw: "Ver: 1.4" },
        "aputure.ls-1200d-pro" => ProModel { cct: false, fw: "Ver: 1.2" },
        _ => ProModel { cct: false, fw: "Ver: 1.8" },
    }
}

fn fan_item<const K: u8>(vals: &[&str]) -> ItemRef<Ap<K>>
where
    Ap<K>: Brand,
{
    pick("Fan Mode", opts(vals)).stored("fan", json!(vals[0])).rc()
}

fn freq_item<const K: u8>(label: &str) -> ItemRef<Ap<K>>
where
    Ap<K>: Brand,
{
    num(label, 0.0, 2000.0).step(100.0).no_accel().fmt(|v, _| format!("+{}Hz", js::string(Some(v)))).stored("freq", json!(0)).rc()
}

fn loss_item4() -> ItemRef<AputurePro> {
    pick("DMX Loss Behavior", opts(&["Black Out", "Fade to Black", "Hold Last Settings", "Hold 2 min.&Fade Out"])).stored("dmxLoss", json!("Fade to Black")).rc()
}

fn control(with24: bool) -> ItemRef<AputurePro> {
    let mut list = vec![sub(
        "Bluetooth",
        vec![on_off("Bluetooth", "bt", "ON", &["ON", "OFF"]), msg("Bluetooth Reset", "Reset failed , please try again."), info_fn("Number", |c| serial_of(c, 6, SERIAL_ALPHABET)).rc()],
    )
    .rc()];
    if with24 {
        list.push(
            sub("2.4G Control", vec![on_off("2.4G", "g24", "ON", &["ON", "OFF"]), num("Channel", 1.0, 3.0).stored("ch", json!(1)).rc(), pick("Group", opts(&["A", "B", "C", "D"])).stored("gr", json!("A")).rc()]).rc(),
        );
    }
    list.push(sub("Lumenradio", vec![on_off("Lumenradio", "crmx", "OFF", &["ON", "OFF"]), msg("Pair", "Pairing failed , please try again.")]).rc());
    list.push(sub("Ethernet", vec![msg("Manual Set IP", "IP Address 0.0.0.0"), msg("Automatic Obtain IP", "Pas de réseau : Local IP 0.0.0.0")]).rc());
    sub("Control System", list).rc()
}

fn update(fw: &str) -> ItemRef<AputurePro> {
    sub("Update", vec![msg("Update Firmware", "No Firmware : Please connect to network or plug in U-disk with firmware."), info("Firmware Version", fw).rc()]).rc()
}

fn pro_tree(c: &Pc<AputurePro>) -> Vec<ItemRef<AputurePro>> {
    let id = c.ty.id.as_str();
    let fw = pro_model(id).fw;
    let curve = || curve_item::<AputurePro>("Dimming Curve", CURVE_NAMES).rc();
    let fan = || fan_item(&["Smart", "High", "Medium", "Silent"]);
    let studio = || on_off("Studio Mode", "studio", "OFF", &["OFF", "ON"]);
    let language = || pick("Language", opts(&["English", "简体中文"])).stored("lang", json!("English")).rc();
    let profile = || profile_item::<AputurePro>("DMX Profile", |m, i| format!("{}.{}", i + 1, m.name)).rc();
    let custom_fx = || sub("Custom FX", vec![not_sim("Picker FX"), not_sim("Music FX"), not_sim("Touchbar  FX")]).rc();
    let dmx_addr = || dmx_address_screen("DMX Address", false);
    if id == "aputure.ls-600x-pro" {
        return vec![
            pick("Output Mode", opts(&["Constant Output", "Max Output"])).stored("output", json!("Max Output")).rc(),
            sub("DMX Mode", vec![dmx_addr(), loss_item4(), profile()]).rc(),
            curve(),
            fan(),
            studio(),
            freq_item("Frequency Selection"),
            control(true),
            language(),
            factory(),
            update(fw),
            custom_fx(),
            sub("Battery Power Options", vec![msg("Wake Up Batteries", "Activation Failed : pas de batterie"), msg("Full Battery Power", "Pas de batterie montée")]).rc(),
        ];
    }
    if id == "aputure.ls-1200d-pro" {
        return vec![
            sub("DMX Mode", vec![dmx_addr(), loss_item4(), profile(), on_off("DMX Termination", "term", "OFF", &["ON", "OFF"])]).rc(),
            on_off("Lead-Follow Mode", "lead", "NO", &["YES", "NO"]),
            curve(),
            fan(),
            studio(),
            freq_item("Frequency Selection"),
            control(false),
            language(),
            factory(),
            update(fw),
            sub("Product Information", vec![info("Hardware Version", "V1.0").rc(), info_fn("UID", |c| format!("6KM{}", serial_of(c, 8, SERIAL_ALPHABET))).rc()]).rc(),
            custom_fx(),
        ];
    }
    vec![sub("DMX Mode", vec![dmx_addr(), on_off("DMX Loss Behavior", "dmxLoss", "OFF", &["ON", "OFF"])]).rc(), curve(), fan(), studio(), control(true), language(), update(fw)]
}

/// Abréviation dans les vignettes du carrousel (les icônes du projecteur ne sont pas reproduites).
fn abbr(label: &str) -> String {
    let known = [
        ("DMX Mode", "DMX"),
        ("Dimming Curve", "CRV"),
        ("Fan Mode", "FAN"),
        ("Studio Mode", "STD"),
        ("Control System", "CTRL"),
        ("Language", "LANG"),
        ("Update", "UPD"),
        ("Output Mode", "OUT"),
        ("Frequency Selection", "Hz"),
        ("Factory Reset", "RST"),
        ("Custom FX", "CFX"),
        ("Battery Power Options", "BAT"),
        ("Lead-Follow Mode", "L-F"),
        ("Product Information", "INFO"),
    ];
    known.iter().find(|(k, _)| *k == label).map_or_else(|| label.chars().take(3).collect::<String>().to_uppercase(), |(_, v)| v.to_string())
}

fn curve_short<S: Brand>(c: &Pc<S>) -> &'static str {
    match c.s("dimmingCurve").and_then(Value::as_str) {
        Some("square") => "EXP",
        Some("invSquare") => "LOG",
        Some("sCurve") => "S-curve",
        _ => "Linear",
    }
}

fn ring(x: f64, y: f64, r: f64, v: f64, w: f64, color: &str, track: &str) -> Prim {
    Prim::Ring(Ring { x, y, r, v, w: Some(w), color: Some(color.into()), track: Some(track.into()) })
}

fn pro_home(c: &Pc<AputurePro>) -> Vec<Prim> {
    let w = 160.0;
    let mut prims: Vec<Prim> = Vec::new();
    let model = pro_model(&c.ty.id);
    let bt = if is(&c.panel_or("bt", json!("ON")), "ON") { "ᛒ" } else { "" };
    prims.push(text(w - 4.0, 2.0, format!("{}  {bt}", c.panel_str("fan", "Smart")), 8.0).r().col("dim").into());
    // tuile de gauche : anneau d'intensité
    let intensity = c.m_nn("intensity", 0.0);
    prims.push(rect_stroke(4.0, 14.0, 70.0, 82.0, "dim"));
    prims.push(ring(39.0, 52.0, 24.0, intensity, 5.0, "accent", "#28303c"));
    let big = c.flash.as_ref().map(|f| f.text.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| pct(intensity, if intensity < 1.0 { 1 } else { 0 }));
    prims.push(text(39.0, 46.0, big, 11.0).c().b().into());
    prims.push(text(39.0, 82.0, "Brightness", 7.0).c().col("dim").into());
    if model.cct {
        // 600x Pro : tuile de température (chaud → froid) puis trois petites tuiles
        let (lo, hi) = cct_range(c, (2700.0, 6500.0));
        let cct = c.m_or("cct", 5600.0);
        let k = (cct - lo) / (hi - lo);
        prims.push(rect_stroke(80.0, 14.0, 76.0, 46.0, "dim"));
        prims.push(line_col(88.0, 30.0, 148.0, 30.0, 4.0, "#ffae5a"));
        prims.push(line_col(118.0, 30.0, 148.0, 30.0, 4.0, "#8cc4ff"));
        prims.push(rect_fill(86.0 + 60.0 * k, 26.0, 4.0, 8.0, "fg"));
        prims.push(text(118.0, 40.0, format!("{}K", js::num_str(js::round(cct))), 12.0).c().b().into());
        prims.push(rect_stroke(80.0, 64.0, 22.0, 32.0, "dim"));
        prims.push(text(91.0, 76.0, curve_short(c).chars().take(3).collect::<String>(), 7.0).c().into());
        prims.push(rect_stroke(105.0, 64.0, 22.0, 32.0, "dim"));
        prims.push(text(116.0, 76.0, "AC", 8.0).c().into());
        prims.push(rect_stroke(130.0, 64.0, 26.0, 32.0, "dim"));
        prims.push(text(143.0, 72.0, format!("GR:{}", c.panel_str("gr", "A")), 7.0).c().into());
        prims.push(text(143.0, 82.0, format!("CH:{}", c.panel_str("ch", "1")), 7.0).c().into());
    } else {
        let tiles = [curve_short(c).to_string(), "AC  48V".into(), format!("SN: {}", serial_of(c, 6, SERIAL_ALPHABET))];
        for (i, txt) in tiles.into_iter().enumerate() {
            let i = i as f64;
            prims.push(rect_stroke(80.0, 14.0 + i * 28.0, 76.0, 25.0, "dim"));
            prims.push(text(118.0, 22.0 + i * 28.0, txt, 9.0).c().bold_if(i == 0.0).into());
        }
    }
    prims
}

fn car_index(c: &Pc<AputurePro>, n: usize) -> usize {
    let n = n as i64;
    (((c.st.car % n) + n) % n) as usize
}

fn pro_carousel(c: &Pc<AputurePro>) -> Vec<Prim> {
    let w = 160.0;
    let items = pro_tree(c);
    let n = items.len();
    let i = car_index(c, n);
    let mut prims: Vec<Prim> = vec![text(w / 2.0, 2.0, "MENU", 10.0).c().b().into()];
    let mut tile = |it: &ItemRef<AputurePro>, x: f64, tw: f64, big: bool| {
        prims.push(rect_fill(x, if big { 18.0 } else { 26.0 }, tw, if big { 44.0 } else { 30.0 }, if big { "accent" } else { "#1b2230" }));
        prims.push(text(x + tw / 2.0, if big { 33.0 } else { 36.0 }, abbr(&it.label), if big { 12.0 } else { 8.0 }).c().b().col(if big { "fg" } else { "dim" }).into());
    };
    tile(&items[(i + n - 1) % n], 8.0, 34.0, false);
    tile(&items[i], 52.0, 56.0, true);
    tile(&items[(i + 1) % n], 118.0, 34.0, false);
    prims.push(text(3.0, 34.0, "〈", 10.0).col("dim").into());
    prims.push(text(w - 3.0, 34.0, "〉", 10.0).r().col("dim").into());
    prims.push(text(w / 2.0, 68.0, items[i].label.clone(), 10.0).c().b().into());
    for k in 0..n {
        prims.push(rect_fill(w / 2.0 - n as f64 * 4.0 + k as f64 * 8.0 + 2.0, 88.0, 4.0, 4.0, if k == i { "accent" } else { "dim" }));
    }
    prims
}

fn dmx_screen<const K: u8>(c: &Pc<Ap<K>>, w: f64, h: f64, big: f64) -> Vec<Prim>
where
    Ap<K>: Brand,
{
    let (locked, editing) = (c.st.dmx_lock, c.st.dmx_edit);
    vec![
        text(w / 2.0, 2.0, "DMX Mode", 9.0).c().b().into(),
        text(w / 2.0, h / 2.0 - big / 2.0, format!("DMX:{}", pad(c.address(), 3)), big).c().b().col(if editing { "accent" } else { "fg" }).into(),
        text(w / 2.0, h - 14.0, if locked { "🔒 LOCK" } else if editing { "SELECT = OK" } else { "🔓" }, 9.0).c().col("dim").into(),
    ]
}

fn fx_screen<const K: u8>(c: &Pc<Ap<K>>, w: f64, rings: bool) -> Vec<Prim>
where
    Ap<K>: Brand,
{
    let fx = c.panel_str("fx", FX_LS[1]);
    let mut prims: Vec<Prim> = vec![text(w / 2.0, 2.0, fx, 10.0).c().b().into()];
    let field = c.st.fx_field;
    let int = c.panel_num("fxInt", 0.5);
    let frq = c.panel_num("fxFrq", 5.0);
    let frq_text = |two: bool| if frq == 11.0 { "R".to_string() } else if two { pad(frq, 2) } else { js::num_str(frq) };
    if rings {
        let x1 = w / 4.0 + 8.0;
        let x2 = (w * 3.0) / 4.0 - 8.0;
        prims.push(ring(x1, 50.0, 20.0, int, 4.0, if field == "int" { "accent" } else { "dim" }, "#28303c"));
        prims.push(text(x1, 45.0, pct(int, 1), 9.0).c().b().into());
        prims.push(text(x1, 78.0, "Intensity", 7.0).c().col(if field == "int" { "fg" } else { "dim" }).into());
        prims.push(ring(x2, 50.0, 20.0, frq / 10.0, 4.0, if field == "frq" { "accent" } else { "dim" }, "#28303c"));
        prims.push(text(x2, 45.0, frq_text(true), 10.0).c().b().into());
        prims.push(text(x2, 78.0, "Frequency", 7.0).c().col(if field == "frq" { "fg" } else { "dim" }).into());
    } else {
        let mut bx = |x: f64, lab: &str, val: String, on: bool| {
            if on {
                prims.push(rect_stroke(x, 18.0, 54.0, 40.0, "fg"));
            }
            prims.push(text(x + 27.0, 22.0, lab, 9.0).c().into());
            prims.push(text(x + 27.0, 36.0, val, 14.0).c().b().into());
        };
        bx(6.0, "INT", pct(int, 0), field == "int");
        bx(68.0, "FRQ", frq_text(false), field == "frq");
    }
    prims.push(text(w / 2.0, if rings { 88.0 } else { 54.0 }, "(effet non simulé)", 7.0).c().col("dim").into());
    prims
}

fn fx_turn<const K: u8>(c: &mut Pc<Ap<K>>, steps: f64, mult: f64)
where
    Ap<K>: Brand,
{
    if c.st.fx_field == "int" {
        let v = clamp(js::round((c.panel_num("fxInt", 0.5) + steps * 0.001 * mult) * 1000.0) / 1000.0, 0.0, 1.0);
        c.set_panel(obj([("fxInt", jv(v))]));
    } else {
        let v = clamp(c.panel_num("fxFrq", 5.0) + steps, 1.0, 11.0);
        c.set_panel(obj([("fxFrq", jv(v))]));
    }
}

fn draw_display(w: f64, h: f64, zoom: f64, pal: Option<crate::screen::Palette>) -> Option<DisplaySpec> {
    Some(DisplaySpec { kind: DisplayKind::Draw, w, h, zoom, palette: pal })
}

/// Élément propre à un modèle de la famille (absent pour les autres).
fn only(ty: &FixtureType, model: &str, e: Element) -> Option<Element> {
    (ty.id == model).then_some(e)
}

impl Brand for AputurePro {
    const SIZE: (f64, f64) = (160.0, 100.0);
    const ID: &'static str = "aputure-pro";

    fn init(_: &'static FixtureType) -> Self {
        Ap::new()
    }

    fn face(ty: &FixtureType) -> Face {
        Face {
            style: "aputure".into(),
            rows: vec![
                vec![Element::Display],
                vec![key("menu", "Menu", ""), key("lighting", "Lighting", ""), key("effects", "Effects", ""), key("back", "Back", "")],
                [Some(knob("select", "SELECT", "push size=l")), only(ty, "aputure.ls-600x-pro", knob("cct", "CCT", "")), Some(key("power", "⏻", "round"))].into_iter().flatten().collect(),
            ],
            display: draw_display(160.0, 100.0, 1.6, palette("#05070b", "#eef2f7", "#5d6776", "#2f8cff", "#020304")),
            note: "SELECT : tourner = intensité (ou défiler), appuyer = valider. Sur l’écran DMX, SELECT maintenu 2 s verrouille l’adresse.".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        match id {
            "select" => vec![(2.0, |c, _| {
                if c.screen == "dmx" {
                    c.st.dmx_lock = !c.st.dmx_lock;
                    let l = c.st.dmx_lock;
                    c.say(if l { "Adresse DMX verrouillée" } else { "Adresse DMX déverrouillée" });
                }
            })],
            "effects" => vec![(2.0, |c, _| {
                if c.screen == "fx" && c.panel("fx").is_some_and(|v| is(v, "Lightning")) {
                    c.say("Lightning : cycle (non simulé)");
                }
            })],
            _ => Vec::new(),
        }
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if id == "power" {
            let on = c.on();
            return c.set_state(obj([("power", json!(!on))]));
        }
        if !c.on() {
            return;
        }
        if c.st.dmx_lock && c.screen == "dmx" && id != "select" {
            return c.say("Adresse DMX verrouillée : SELECT 2 s pour déverrouiller");
        }
        match id {
            "lighting" => {
                c.set_menu(None);
                c.screen = "home";
                c.set_panel(obj([("fxOn", json!(false))]));
            }
            "menu" => {
                c.set_menu(None);
                c.screen = if c.screen == "carousel" { "home" } else { "carousel" };
            }
            "effects" => {
                if c.screen == "fx" && c.panel("fx").is_some_and(|v| is(v, "Lightning") || is(v, "Explosion")) {
                    return c.say("Trigger (effet non simulé)");
                }
                let item = pick::<AputurePro>("Effects", opts(&FX_LS))
                    .get(|c| c.panel_or("fx", json!(FX_LS[1])))
                    .set(|c, v| {
                        c.set_panel(obj([("fx", v), ("fxOn", json!(true))]));
                        c.screen = "fx";
                        None
                    })
                    .rc();
                c.set_menu(Some(MenuState::new(vec![item], "Effects", false)));
                c.screen = "menu";
                c.menu_enter();
                c.st.single = true;
            }
            "back" => {
                if c.screen == "menu" {
                    if !c.menu_back() || c.st.single {
                        c.set_menu(None);
                        c.st.single = false;
                        c.screen = "carousel";
                    }
                    return;
                }
                if c.screen == "dmx" {
                    c.st.dmx_edit = false;
                    c.screen = "carousel";
                    return;
                }
                c.screen = if c.screen == "carousel" { "home" } else { "carousel" };
            }
            "select" => {
                if c.screen == "carousel" {
                    let items = pro_tree(c);
                    let it = items[car_index(c, items.len())].clone();
                    match &it.kind {
                        Kind::Sub(children) => {
                            let list = children.iter().cloned().collect();
                            c.set_menu(Some(MenuState::new(list, &it.label, false)));
                            c.screen = "menu";
                            c.st.single = false;
                        }
                        Kind::Act(run) => {
                            if let Some(m) = run(c) {
                                c.say(m);
                            }
                        }
                        _ => open_pick(c, it),
                    }
                    return;
                }
                if c.screen == "menu" {
                    let r = c.menu_enter_say();
                    if r.is_some_and(|r| r.committed) && c.st.single {
                        c.set_menu(None);
                        c.st.single = false;
                        if c.screen == "menu" {
                            c.screen = "carousel";
                        }
                    }
                    return;
                }
                if c.screen == "dmx" {
                    if c.st.dmx_lock {
                        return c.say("Adresse DMX verrouillée");
                    }
                    c.st.dmx_edit = !c.st.dmx_edit;
                    return;
                }
                if c.screen == "fx" {
                    c.st.fx_field = if c.st.fx_field == "frq" { "int" } else { "frq" };
                }
            }
            _ => {}
        }
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, mult: f64) {
        if !c.on() {
            return;
        }
        if id == "cct" {
            let (lo, hi) = cct_range(c, (2700.0, 6500.0));
            let v = nudge(c, "cct", steps, Nudge::new(50.0 * mult, lo, hi).round(1.0));
            c.set_manual(obj([("mode", json!("cct")), ("cct", jv(v))]));
            return;
        }
        match c.screen {
            "carousel" => c.st.car += steps as i64,
            "menu" => c.menu_move(steps, mult),
            "dmx" => {
                if !c.st.dmx_edit {
                    return;
                }
                let a = clamp(c.address() + steps * if mult > 2.0 { 10.0 } else { 1.0 }, 1.0, 512.0);
                c.set_settings(obj([("dmxAddress", jv(a))]));
            }
            "fx" => fx_turn(c, steps, mult),
            _ => {
                nudge(c, "intensity", steps, Nudge::new(0.001 * mult, 0.0, 1.0));
            }
        }
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let (w, h) = (160.0, 100.0);
        match c.screen {
            "carousel" => Drawn::of(pro_carousel(c)),
            "menu" => {
                let o = ListOpts { w: Some(w), h: Some(h), rows: 4, row_h: 18.0, size: 10.0, sel_fill: Some("#1d3f73"), ..ListOpts::default() };
                let prims = match &c.menu {
                    Some(m) if m.edit.is_some() => draw_edit(c, m, &ListOpts { markers: ("◉ ", "○ "), radio: true, ..o }),
                    Some(m) => draw_list(c, m, &ListOpts { h: None, max_value: 10, ..o }),
                    None => Vec::new(),
                };
                Drawn::of(prims)
            }
            "dmx" => Drawn::of(dmx_screen(c, w, h, 24.0)),
            "fx" => Drawn::of(fx_screen(c, w, true)),
            _ => Drawn::of(pro_home(c)),
        }
    }
}

// ================================================================== OLED : LS 600d, LS 300x, LS C300d
struct OledModel {
    cct: bool,
    fan: &'static [&'static str],
}

fn oled_model(id: &str) -> OledModel {
    match id {
        "aputure.ls-300x" => OledModel { cct: true, fan: &["FORCE", "AUTO"] },
        "aputure.c300d" => OledModel { cct: false, fan: &["FORCE", "AUTO"] },
        _ => OledModel { cct: false, fan: &["Smart", "High", "Medium", "Silent"] },
    }
}

const WIRELESS: [&str; 3] = ["2.4G", "BT", "OFF"];

fn oled_tree(c: &Pc<AputureOled>) -> Vec<ItemRef<AputureOled>> {
    let id = c.ty.id.as_str();
    let yes_no = |label: &str, key: &'static str, d: &str| pick(label, opts(&["YES", "NO"])).stored(key, json!(d)).rc();
    let curve = curve_item::<AputureOled>(if id == "aputure.ls-600d" { "Dimming Curves" } else { "Dimming Curve" }, CURVE_NAMES).rc();
    let fan = fan_item(oled_model(id).fan);
    let lang = pick("Language", opts(&["English", "中文"])).stored("lang", json!("English")).rc();
    let bt_reset = msg(if id == "aputure.c300d" { "Bluetooth Reset" } else { "BT Reset" }, "Loading… [Reset Failed]");
    if id == "aputure.ls-300x" {
        return vec![
            pick("Output Mode", opts(&["Constant Output", "Max Output"])).stored("output", json!("Max Output")).rc(),
            dmx_address_screen("DMX Mode", false),
            curve,
            fan,
            yes_no("Studio Mode", "studio", "YES"),
            bt_reset,
            lang,
            msg("USB Mode", "Update : pas de clé USB"),
            exit_item(),
            info("Ver:1.0", "").rc(),
        ];
    }
    if id == "aputure.c300d" {
        return vec![
            dmx_address_screen("DMX Mode", false),
            curve,
            fan,
            yes_no("Studio Mode", "studio", "YES"),
            info_fn("Fixture Serial No.", |c| serial_of(c, 6, SERIAL_ALPHABET)).rc(),
            bt_reset,
            lang,
            msg("Update Firmware", "Pas de clé USB"),
            info("Firmware Version", "V1.9").rc(),
            pick("DMX Loss Behavior", opts(&["Black Out", "Fade To Black", "Hold Last Settings"])).stored("dmxLoss", json!("Hold Last Settings")).rc(),
            exit_item(),
        ];
    }
    vec![
        sub(
            "DMX Mode",
            vec![
                dmx_address_screen("DMX Address", true),
                profile_item::<AputureOled>("DMX Profile", |m, _| strip_8bit(&m.name)).rc(),
                pick("DMX Loss Behavior", opts(&["Black Out", "Fade To Black", "Hold Last Settings", "Hold 2 min & Fade Out"])).stored("dmxLoss", json!("Black Out")).rc(),
                exit_item(),
            ],
        )
        .rc(),
        curve,
        fan,
        yes_no("Studio Mode", "studio", "NO"),
        freq_item("Freq.Selection"),
        bt_reset,
        info_fn("BT Serial NO", |c| format!("R:{}", serial_of(c, 6, SERIAL_ALPHABET))).rc(),
        lang,
        msg("Update Firmware", "Pas de clé USB"),
        info("Firmware Version", "Control Box : 1.2").rc(),
        info_fn("UID", |c| format!("6MH{}", serial_of(c, 8, SERIAL_ALPHABET))).rc(),
        factory(),
        exit_item(),
    ]
}

/// `name.replace(/ 8-bit$/i, '')`
fn strip_8bit(name: &str) -> String {
    let n = name.len();
    if n >= 6 && name.is_char_boundary(n - 6) && name[n - 6..].eq_ignore_ascii_case(" 8-bit") { name[..n - 6].to_string() } else { name.to_string() }
}

fn curve_glyph<S: Brand>(c: &Pc<S>) -> &'static str {
    match c.s("dimmingCurve").and_then(Value::as_str) {
        Some("square") => "⌒",
        Some("invSquare") => "◜",
        Some("sCurve") => "S",
        _ => "/",
    }
}

impl Brand for AputureOled {
    const SIZE: (f64, f64) = (128.0, 64.0);
    const ID: &'static str = "aputure-oled";

    fn init(_: &'static FixtureType) -> Self {
        Ap::new()
    }

    fn face(ty: &FixtureType) -> Face {
        Face {
            style: "aputure".into(),
            rows: vec![
                vec![Element::Display],
                vec![key("k1", "LIGHTING / EFFECTS", "small"), key("k2", "CHANNEL / INTENSITY", "small"), key("k3", "GROUP / FREQUENCY", "small"), key("k4", "WIRELESS / TRIGGER", "small")],
                [Some(knob("menu", "MENU · BRIGHTNESS", "push size=l")), only(ty, "aputure.ls-300x", knob("preset", "PRESET · CCT", "push")), Some(key("power", "⏻", "round"))].into_iter().flatten().collect(),
            ],
            display: draw_display(128.0, 64.0, 2.0, palette("#000000", "#f4f4f4", "#6a6a6a", "#f4f4f4", "#000000")),
            note: "Légende du haut : mode normal ; du bas : mode effets (EFFECTS maintenu 2 s). Molette : appuyer = MENU, tourner = intensité.".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        match id {
            "k1" => vec![(2.0, |c, _| {
                c.st.fx = !c.st.fx;
                let fx = c.st.fx;
                c.screen = if fx { "fx" } else { "home" };
                c.say(if fx { "Mode effets" } else { "Mode lumière" });
            })],
            "menu" => vec![(2.0, |c, _| {
                if c.ty.id == "aputure.ls-600d" && c.screen == "dmx" {
                    c.st.dmx_lock = !c.st.dmx_lock;
                    let l = c.st.dmx_lock;
                    c.say(if l { "DMX verrouillé" } else { "DMX déverrouillé" });
                }
            })],
            _ => Vec::new(),
        }
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if id == "power" {
            let on = c.on();
            return c.set_state(obj([("power", json!(!on))]));
        }
        if !c.on() {
            return;
        }
        if c.st.fx {
            match id {
                "k1" => {
                    let cur = c.panel_or("fx", json!(FX_LS[1]));
                    let i = FX_LS.iter().position(|f| is(&cur, f)).map_or(-1, |i| i as i64);
                    let next = FX_LS[((i + 1) % FX_LS.len() as i64) as usize];
                    return c.set_panel(obj([("fx", json!(next))]));
                }
                "k2" => return c.st.fx_field = "int",
                "k3" => return c.st.fx_field = "frq",
                "k4" => return c.say("Trigger (effet non simulé)"),
                "menu" => return, // pas de menu en mode effets
                _ => {}
            }
        }
        match id {
            "k1" => {
                c.set_menu(None);
                c.screen = "home";
            }
            "k2" => {
                let ch = c.panel_num("ch", 1.0) % 3.0 + 1.0;
                c.set_panel(obj([("ch", jv(ch))]));
                c.show_big(format!("CH:{}", js::num_str(ch)));
            }
            "k3" => {
                let cur = c.panel_str("gr", "A");
                let i = "ABCD".find(cur.as_str()).map_or(-1, |i| i as i64);
                let gr = "ABCD".chars().nth(((i + 1) % 4) as usize).unwrap_or('A').to_string();
                c.set_panel(obj([("gr", json!(gr))]));
                c.show_big(format!("GR:{gr}"));
            }
            "k4" => {
                let cur = c.panel_or("wireless", json!("2.4G"));
                let i = WIRELESS.iter().position(|w| is(&cur, w)).map_or(-1, |i| i as i64);
                let w = WIRELESS[((i + 1) % 3) as usize];
                c.set_panel(obj([("wireless", json!(w))]));
                c.say(if w == "OFF" { "Sans fil désactivé".to_string() } else { format!("Commande sans fil : {}", if w == "BT" { "Bluetooth" } else { "2.4G" }) });
            }
            "preset" => c.say("PRESET : fonction non documentée par Aputure"),
            "menu" => {
                if c.screen == "dmx" {
                    if c.st.dmx_lock {
                        return c.say("DMX verrouillé");
                    }
                    c.screen = if c.menu.is_some() { "menu" } else { "home" };
                    return;
                }
                if c.screen != "menu" {
                    let tree = oled_tree(c);
                    return c.open_menu(tree, "System Menu", true);
                }
                c.menu_enter_say();
            }
            _ => {}
        }
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, mult: f64) {
        if !c.on() {
            return;
        }
        if id == "preset" {
            let (lo, hi) = cct_range(c, (2700.0, 6500.0));
            if c.st.fx {
                return;
            }
            let v = nudge(c, "cct", steps, Nudge::new(50.0 * mult, lo, hi).round(1.0));
            c.set_manual(obj([("mode", json!("cct"))]));
            return c.show_big(format!("{}K", js::num_str(v)));
        }
        if c.st.fx {
            return fx_turn(c, steps, mult);
        }
        if c.screen == "menu" {
            return c.menu_move(steps, mult);
        }
        if c.screen == "dmx" {
            if c.st.dmx_lock {
                return c.say("DMX verrouillé");
            }
            let a = clamp(c.address() + steps * if mult > 2.0 { 10.0 } else { 1.0 }, 1.0, 512.0);
            return c.set_settings(obj([("dmxAddress", jv(a))]));
        }
        let v = nudge(c, "intensity", steps, Nudge::new(0.01 * mult.min(4.0), 0.0, 1.0).round(100.0));
        c.show_big(pct(v, 0));
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let w = 128.0;
        if c.st.fx {
            return Drawn::of(fx_screen(c, w, false));
        }
        if c.screen == "menu" {
            let prims = match &c.menu {
                Some(m) if m.edit.is_some() => draw_edit(c, m, &ListOpts { markers: ("■", "□"), radio: true, ..ListOpts::default() }),
                Some(m) => draw_list(c, m, &ListOpts::default()),
                None => Vec::new(),
            };
            return Drawn::of(prims);
        }
        if c.screen == "dmx" {
            return Drawn::of(dmx_screen(c, w, 64.0, 18.0));
        }
        if let Some(f) = &c.flash {
            return Drawn::of(vec![text(w / 2.0, 18.0, f.text.clone(), 26.0).c().b().into()]);
        }
        let wl = c.panel_or("wireless", json!("2.4G"));
        let model = oled_model(&c.ty.id);
        let mut prims: Vec<Prim> = vec![
            text(4.0, 4.0, pct(c.m_nn("intensity", 0.0), 0), 20.0).b().into(),
            line(74.0, 2.0, 74.0, 36.0, Some(1.0)),
            text(80.0, 4.0, format!("AC  {}", if is(&wl, "OFF") { "⊘" } else if is(&wl, "BT") { "ᛒ" } else { "2.4G" }), 9.0).into(),
            text(80.0, 18.0, ">2h", 9.0).into(),
            line(2.0, 38.0, w - 2.0, 38.0, Some(1.0)),
        ];
        if model.cct {
            prims.push(text(4.0, 44.0, format!("{}K", js::num_str(js::round(c.m_or("cct", 5600.0)))), 12.0).b().into());
        }
        prims.push(text(if model.cct { 60.0 } else { 6.0 }, 45.0, format!("CH:{} GR:{}", c.panel_str("ch", "1"), c.panel_str("gr", "A")), 9.0).into());
        prims.push(text(w - 2.0, 43.0, curve_glyph(c), 12.0).r().b().into());
        Drawn::of(prims)
    }
}

// ================================================================== Nova P300c
const LIGHT_MODES_NOVA: [&str; 7] = ["CCT", "HSI", "RGB", "xy", "SOURCE", "GEL", "FX"];

fn nova_tree() -> Vec<ItemRef<Nova>> {
    vec![
        pick("Output Mode", opts(&["Constant Output", "Max Output"])).stored("output", json!("Constant Output")).rc(),
        sub(
            "DMX Settings",
            vec![
                dmx_address_screen("DMX Address", true),
                profile_item::<Nova>("DMX Profile", |m, i| format!("{}:{}", i + 1, m.name)).rc(),
                pick("DMX Loss Behavior", opts(&["Black Out", "Fade to Black", "Hold Last Settings", "Hold 2 min & Fade Out"])).stored("dmxLoss", json!("Hold Last Settings")).rc(),
            ],
        )
        .rc(),
        fan_item(&["Smart", "High", "Medium", "Silent"]),
        curve_item::<Nova>("Dimming Curves", CURVE_NAMES).rc(),
        num("Freq. Selection", 1.0, 10.0).no_accel().stored("freq", json!(1)).rc(),
        sub("BT Settings", vec![on_off("BT Status", "bt", "ON", &["ON", "OFF"]), msg("BT Reset", "Reset Failed"), info_fn("Fixture Serial No.", |c| serial_of(c, 6, SERIAL_ALPHABET)).rc()]).rc(),
        pick("Studio Mode", opts(&["YES", "NO"])).stored("studio", json!("NO")).rc(),
        msg("Update Firmware", "Pas de clé USB"),
        info("Firmware Version", "Control Box : 1.9").rc(),
        pick("Language", opts(&["English", "中文"])).stored("lang", json!("English")).rc(),
        factory(),
        sub("Custom FX", vec![not_sim("Picker FX"), not_sim("Music FX")]).rc(),
        pick("Screensaver", opts(&["OFF", "ON"])).stored("screensaver", json!("OFF")).rc(),
        exit_item(),
    ]
}

/// Case étiquetée (étiquette inversée en haut à gauche).
#[allow(clippy::too_many_arguments)]
fn tag_cell(prims: &mut Vec<Prim>, x: f64, y: f64, w: f64, h: f64, tag: &str, value: String, big: bool) {
    prims.push(rect_stroke(x, y, w, h, "dim"));
    prims.push(rect_fill(x, y, js::len16(tag) as f64 * 5.0 + 6.0, 9.0, "fg"));
    prims.push(text(x + 3.0, y + 1.0, tag, 7.0).inv(true).b().into());
    prims.push(text(x + w / 2.0, y + if big { 11.0 } else { 12.0 }, value, if big { 15.0 } else { 11.0 }).c().b().into());
}

fn save_nova_preset(c: &mut Pc<Nova>, id: &str) {
    let mut presets = c.panel_or("presets", json!({})).as_object().cloned().unwrap_or_default();
    let mut look = serde_json::Map::new();
    for k in ["mode", "intensity", "cct", "tint", "hue", "sat"] {
        if let Some(v) = c.m(k) {
            look.insert(k.into(), v.clone());
        }
    }
    presets.insert(id.into(), Value::Object(look));
    c.set_panel(obj([("presets", Value::Object(presets))]));
    c.say(format!("Préréglage {} enregistré", &id[1..]));
}

impl Brand for Nova {
    const SIZE: (f64, f64) = (128.0, 64.0);
    const ID: &'static str = "aputure-nova";

    fn init(_: &'static FixtureType) -> Self {
        Ap::new()
    }

    fn face(_: &FixtureType) -> Face {
        Face {
            style: "aputure".into(),
            rows: vec![
                vec![Element::Display, knob("int", "INT", "push size=l")],
                vec![knob("cct", "CCT / HUE", ""), knob("gm", "G/M / SAT", "push"), col(vec![key("mode", "LIGHT MODE", "small"), key("menu", "MENU", "small")])],
                vec![key("p1", "1", "small"), key("p2", "2", "small"), key("p3", "3", "small"), key("p4", "4", "small"), label("PRESET"), key("power", "⏻", "round")],
            ],
            display: draw_display(128.0, 64.0, 2.0, palette("#000000", "#f2f2f2", "#555555", "#f2f2f2", "#000000")),
            note: "Préréglages 1–4 : appui court = rappel, appui long = enregistrer. LIGHT MODE : CCT ou HSI (les autres modes ne sont pas simulés).".into(),
        }
    }

    fn holds(id: &str) -> Holds<Self> {
        if ["p1", "p2", "p3", "p4"].contains(&id) { vec![(HOLD_LONG, save_nova_preset)] } else { Vec::new() }
    }

    fn key(c: &mut Pc<Self>, id: &str, _long: bool) {
        if id == "power" {
            let on = c.on();
            return c.set_state(obj([("power", json!(!on))]));
        }
        if !c.on() {
            return;
        }
        if id.len() == 2 && id.starts_with('p') && id.as_bytes()[1].is_ascii_digit() {
            let p = c.panel_or("presets", json!({})).get(id).cloned().filter(|p| js::truthy(Some(p)));
            let Some(p) = p else { return c.say(format!("Préréglage {} vide (appui long pour l’enregistrer)", &id[1..])) };
            c.set_manual(p.as_object().cloned().unwrap_or_default());
            return c.say(format!("Préréglage {}", &id[1..]));
        }
        match id {
            "mode" => {
                c.set_menu(None);
                c.screen = if c.screen == "modes" { "home" } else { "modes" };
                let want = if c.hsi() { "HSI" } else { "CCT" };
                c.st.mode_cur = LIGHT_MODES_NOVA.iter().position(|m| *m == want).unwrap_or(0) as i64;
            }
            "menu" => {
                if c.screen == "menu" {
                    c.close_menu();
                } else {
                    c.open_menu(nova_tree(), "System Menu", false);
                }
            }
            "int" => {
                if c.screen == "modes" {
                    let m = LIGHT_MODES_NOVA[c.st.mode_cur.clamp(0, 6) as usize];
                    c.screen = "home";
                    if m == "CCT" || m == "HSI" {
                        return c.set_manual(obj([("mode", json!(m.to_lowercase()))]));
                    }
                    return c.say(format!("{m} : mode non simulé dans ARKLIGHT"));
                }
                if c.screen == "menu" {
                    c.menu_enter_say();
                    return;
                }
                if c.screen == "dmx" {
                    c.screen = if c.menu.is_some() { "menu" } else { "home" };
                }
            }
            _ => {}
        }
    }

    fn turn(c: &mut Pc<Self>, id: &str, steps: f64, mult: f64) {
        if !c.on() {
            return;
        }
        match c.screen {
            "modes" => {
                if id == "int" {
                    c.st.mode_cur = clamp(c.st.mode_cur as f64 + steps, 0.0, (LIGHT_MODES_NOVA.len() - 1) as f64) as i64;
                }
                return;
            }
            "menu" => {
                if id == "int" {
                    c.menu_move(steps, mult);
                }
                return;
            }
            "dmx" => {
                if id == "int" {
                    let a = clamp(c.address() + steps * if mult > 2.0 { 10.0 } else { 1.0 }, 1.0, 512.0);
                    c.set_settings(obj([("dmxAddress", jv(a))]));
                }
                return;
            }
            _ => {}
        }
        let hsi = c.hsi();
        match id {
            "int" => {
                nudge(c, "intensity", steps, Nudge::new(0.001 * mult, 0.0, 1.0));
            }
            "cct" => {
                if hsi {
                    nudge(c, "hue", steps, Nudge::new(mult, 0.0, 360.0).wrap().round(1.0));
                } else {
                    let (lo, hi) = cct_range(c, (2000.0, 10000.0));
                    nudge(c, "cct", steps, Nudge::new(50.0 * mult, lo, hi).round(1.0));
                }
            }
            "gm" => {
                if hsi {
                    nudge(c, "sat", steps, Nudge::new(0.01 * mult, 0.0, 1.0).round(100.0));
                } else {
                    nudge(c, "tint", steps, Nudge::new(0.1, -1.0, 1.0).round(10.0));
                }
            }
            _ => {}
        }
    }

    fn draw(c: &mut Pc<Self>) -> Drawn {
        let w = 128.0;
        match c.screen {
            "menu" => {
                let prims = match &c.menu {
                    Some(m) if m.edit.is_some() => draw_edit(c, m, &ListOpts { markers: ("▪", "▫"), radio: true, ..ListOpts::default() }),
                    Some(m) => draw_list(c, m, &ListOpts::default()),
                    None => Vec::new(),
                };
                Drawn::of(prims)
            }
            "dmx" => Drawn::of(dmx_screen(c, w, 64.0, 18.0)),
            "modes" => {
                let mut prims: Vec<Prim> = Vec::new();
                let i = c.st.mode_cur;
                for (k, d) in [-1i64, 0, 1].into_iter().enumerate() {
                    let Some(name) = usize::try_from(i + d).ok().and_then(|j| LIGHT_MODES_NOVA.get(j)) else { continue };
                    let y = 4.0 + k as f64 * 20.0;
                    if d == 0 {
                        prims.push(rect_fill(24.0, y, 80.0, 18.0, "fg"));
                    } else {
                        prims.push(rect_stroke(30.0, y + 2.0, 68.0, 14.0, "dim"));
                    }
                    let t = text(w / 2.0, y + 3.0, *name, if d == 0 { 12.0 } else { 9.0 }).c().b().inv(d == 0);
                    prims.push(if d == 0 { t.into() } else { t.col("dim").into() });
                }
                Drawn::of(prims)
            }
            _ => {
                let mut prims: Vec<Prim> = Vec::new();
                let hsi = c.hsi();
                tag_cell(&mut prims, 1.0, 1.0, w - 2.0, 30.0, "INT", format!("{} %", js::to_fixed(c.m_nn("intensity", 0.0) * 100.0, 1)), true);
                if hsi {
                    tag_cell(&mut prims, 1.0, 33.0, 62.0, 30.0, "HUE", format!("{} °", js::num_str(js::round(c.m_or("hue", 0.0)))), false);
                    tag_cell(&mut prims, 65.0, 33.0, 62.0, 30.0, "SAT", format!("{} %", js::num_str(js::round(c.m_nn("sat", 1.0) * 100.0))), false);
                } else {
                    let tint = c.m_or("tint", 0.0);
                    tag_cell(&mut prims, 1.0, 33.0, 62.0, 30.0, "CCT", format!("{} K", js::num_str(js::round(c.m_or("cct", 5500.0)))), false);
                    tag_cell(&mut prims, 65.0, 33.0, 62.0, 30.0, "G/M", format!("{}{}", if tint >= 0.0 { "+" } else { "" }, js::to_fixed(tint, 1)), false);
                }
                Drawn::of(prims)
            }
        }
    }
}
