//! Face avant du projecteur choisi (en tête de l'inspecteur) : écran, touches,
//! molettes et voyants simulés par `arklight-onboard`, comme `web/js/ui/panel-face.js` et
//! `control-panel.js`. L'écran est dessiné en SVG puis rendu en image (resvg).

use crate::{FaceCell, FaceItem, FaceRow, State, i18n};
use arklight_core::document::{Changes, Edit};
use arklight_onboard::screen::{Align, Palette};
use arklight_onboard::{DisplaySpec, Element, Env, Leds, Outcome, Prim, Screen};
use slint::{ModelRc, SharedString, VecModel};
use std::fmt::Write;

/// Projecteur dont la face avant est montrée : le premier choisi qui a des commandes embarquées.
pub fn target(st: &mut State) -> Option<String> {
    let ids: Vec<String> = st.selection.clone();
    for id in ids {
        let Some(f) = st.doc.fixture(&id) else { continue };
        if st.onboard.panel(f).is_some() {
            return Some(id);
        }
    }
    None
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Couleur d'un nom de la palette (« fg », « dim », « accent »…) ou couleur CSS.
fn color(name: Option<&str>, pal: &Palette, default: &str) -> String {
    match name.unwrap_or(default) {
        "fg" => pal.fg.clone(),
        "bg" => pal.bg.clone(),
        "dim" => pal.dim.clone(),
        "accent" => pal.accent.clone(),
        "off" => pal.off.clone(),
        other => other.to_owned(),
    }
}

fn default_palette(spec: &DisplaySpec) -> Palette {
    spec.palette.clone().unwrap_or_else(|| match spec.kind {
        arklight_onboard::screen::DisplayKind::Lcd => Palette { bg: "#9fc23a".into(), fg: "#1b2410".into(), dim: "#5d7330".into(), accent: "#1b2410".into(), off: "#2a3020".into() },
        _ => Palette { bg: "#000000".into(), fg: "#e8f4ff".into(), dim: "#5b6b78".into(), accent: "#ffffff".into(), off: "#050607".into() },
    })
}

/// Écran en SVG, à la taille de l'écran simulé (pixels de l'écran).
fn screen_svg(screen: &Screen, spec: &DisplaySpec) -> String {
    let (w, h) = (spec.w, spec.h);
    let pal = default_palette(spec);
    let mut s = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#);
    match screen {
        Screen::None => {
            let _ = write!(s, r#"<rect width="{w}" height="{h}" fill="{}"/>"#, pal.off);
        }
        Screen::Text(t) => {
            if t.off {
                let _ = write!(s, r#"<rect width="{w}" height="{h}" fill="{}"/>"#, pal.off);
            } else {
                let fg = if t.dim { pal.dim.clone() } else { pal.fg.clone() };
                let _ = write!(s, r#"<rect width="{w}" height="{h}" fill="{}"/>"#, pal.bg);
                let mut y = 1.0;
                if let Some(i) = &t.icons {
                    // batterie (en haut à droite), prise, Bluetooth
                    let bw = 14.0;
                    let x = w - bw - 3.0;
                    let _ = write!(s, r#"<rect x="{x}" y="1.5" width="{bw}" height="6" fill="none" stroke="{fg}" stroke-width="1"/><rect x="{}" y="3" width="1.5" height="3" fill="{fg}"/>"#, x + bw);
                    let fill = (bw - 2.0) * (i.battery / 100.0).clamp(0.0, 1.0);
                    let _ = write!(s, r#"<rect x="{}" y="2.5" width="{fill}" height="4" fill="{fg}"/>"#, x + 1.0);
                    if i.plug {
                        let _ = write!(s, r#"<text x="{}" y="7.5" font-family="Consolas" font-size="8" fill="{fg}">⚡</text>"#, x - 9.0);
                    }
                    if i.bt {
                        let _ = write!(s, r#"<text x="2" y="7.5" font-family="Consolas" font-size="8" fill="{fg}">ʙ{}</text>"#, if i.bt_waves { " )))" } else { "" });
                    }
                    y = 10.0;
                }
                let n = t.lines.len().max(1) as f64;
                let size = if t.big { ((h - y) / n * 0.62).min(18.0) } else { ((h - y) / n * 0.72).min(10.0) };
                let step = (h - y) / n;
                for (k, l) in t.lines.iter().enumerate() {
                    let top = y + step * k as f64;
                    let base = top + step / 2.0 + size * 0.36;
                    let size = if l.emphasis { size * 1.3 } else { size };
                    // la ligne tient dans la largeur de l'écran (police à chasse fixe : 0,55 em par lettre)
                    let chars = (l.text.chars().count() + l.value.as_ref().map_or(0, |v| v.chars().count() + 1)).max(1) as f64;
                    let size = size.min((w - 6.0) / (chars * 0.55));
                    let (bgc, txt) = if l.inverse || l.selected { (Some(fg.clone()), pal.bg.clone()) } else { (None, fg.clone()) };
                    if let Some(b) = bgc {
                        let _ = write!(s, r#"<rect x="0" y="{top}" width="{w}" height="{step}" fill="{b}"/>"#);
                    }
                    let anchor = if t.big { "middle" } else { "start" };
                    let x = if t.big { w / 2.0 } else { 3.0 };
                    let _ = write!(s, r#"<text x="{x}" y="{base}" font-family="Consolas" font-size="{size}" text-anchor="{anchor}" fill="{txt}">{}</text>"#, esc(&l.text));
                    if let Some(v) = &l.value {
                        let _ = write!(s, r#"<text x="{}" y="{base}" font-family="Consolas" font-size="{size}" text-anchor="end" fill="{txt}">{}</text>"#, w - 3.0, esc(v));
                    }
                }
            }
        }
        Screen::Draw(d) => {
            let _ = write!(s, r#"<rect width="{w}" height="{h}" fill="{}"/>"#, if d.off { &pal.off } else { &pal.bg });
            if !d.off {
                let _ = write!(s, r#"<g opacity="{}">"#, if d.dim { 0.45 } else { 1.0 });
                for p in &d.prims {
                    match p {
                        Prim::Rect(r) => {
                            let fill = r.fill.as_deref().map_or("none".to_owned(), |c| color(Some(c), &pal, "fg"));
                            let stroke = r.stroke.as_deref().map_or("none".to_owned(), |c| color(Some(c), &pal, "fg"));
                            let _ = write!(s, r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{fill}" stroke="{stroke}" stroke-width="1"/>"#, r.x, r.y, r.w, r.h);
                        }
                        Prim::Line(l) => {
                            let _ = write!(s, r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#, l.x1, l.y1, l.x2, l.y2, color(l.color.as_deref(), &pal, "fg"), l.w.unwrap_or(1.0));
                        }
                        Prim::Ring(r) => {
                            let width = r.w.unwrap_or(2.0);
                            let track = color(r.track.as_deref(), &pal, "dim");
                            let _ = write!(s, r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{track}" stroke-width="{width}"/>"#, r.x, r.y, r.r);
                            let v = r.v.clamp(0.0, 1.0);
                            if v > 0.0 {
                                let c = std::f64::consts::TAU * r.r;
                                let _ = write!(
                                    s,
                                    r#"<circle cx="{}" cy="{}" r="{}" fill="none" stroke="{}" stroke-width="{width}" stroke-dasharray="{} {}" transform="rotate(-90 {} {})"/>"#,
                                    r.x,
                                    r.y,
                                    r.r,
                                    color(r.color.as_deref(), &pal, "accent"),
                                    c * v,
                                    c,
                                    r.x,
                                    r.y
                                );
                            }
                        }
                        Prim::Text(t) => {
                            let fg = color(t.color.as_deref(), &pal, "fg");
                            let anchor = match t.align {
                                Align::Left => "start",
                                Align::Center => "middle",
                                Align::Right => "end",
                            };
                            let family = if t.font.as_deref() == Some("seg") || t.font.as_deref() == Some("mono") { "Consolas" } else { "Segoe UI" };
                            let weight = if t.bold { "700" } else { "400" };
                            let base = t.y + t.s * 0.82;
                            if t.inv {
                                let tw = t.w.unwrap_or(t.text.chars().count() as f64 * t.s * 0.6);
                                let x0 = match t.align {
                                    Align::Left => t.x - 1.0,
                                    Align::Center => t.x - tw / 2.0 - 1.0,
                                    Align::Right => t.x - tw - 1.0,
                                };
                                let _ = write!(s, r#"<rect x="{x0}" y="{}" width="{}" height="{}" fill="{fg}"/>"#, t.y - 1.0, tw + 2.0, t.s + 2.0);
                            }
                            let fill = if t.inv { pal.bg.clone() } else { fg };
                            // trop long pour sa place : le texte est resserré
                            let est = t.text.chars().count() as f64 * t.s * 0.6;
                            let fit = match t.w {
                                Some(w) if est > w => format!(r#" textLength="{w}" lengthAdjust="spacingAndGlyphs""#),
                                _ => String::new(),
                            };
                            let spacing = t.spacing.map_or(String::new(), |sp| format!(r#" letter-spacing="{sp}""#));
                            let _ = write!(s, r#"<text x="{}" y="{base}" font-family="{family}" font-weight="{weight}" font-size="{}" text-anchor="{anchor}" fill="{fill}"{fit}{spacing}>{}</text>"#, t.x, t.s, esc(&t.text));
                        }
                    }
                }
                s.push_str("</g>");
            }
        }
    }
    s.push_str("</svg>");
    s
}

/// Image de l'écran (agrandie de `zoom` × `scale`).
pub fn screen_image(screen: &Screen, spec: &DisplaySpec, scale: f32) -> Option<slint::Image> {
    let svg = screen_svg(screen, spec);
    let z = spec.zoom.max(1.0) * scale as f64;
    let (pw, ph) = ((spec.w * z).round() as u32, (spec.h * z).round() as u32);
    let rgba = arklight_plot::rasterize(&svg, pw, ph);
    if rgba.len() != (pw * ph * 4) as usize {
        return None;
    }
    Some(slint::Image::from_rgba8(slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&rgba, pw, ph)))
}

fn led_color(name: &str) -> slint::Color {
    let (r, g, b) = match name {
        "red" => (255, 59, 48),
        "green" => (52, 211, 89),
        "blue" => (64, 140, 255),
        "yellow" | "amber" | "orange" => (255, 184, 48),
        "white" => (240, 240, 240),
        "cyan" => (60, 220, 230),
        "magenta" | "purple" => (220, 80, 230),
        _ => {
            let hex = name.trim_start_matches('#');
            let v = u32::from_str_radix(hex, 16).unwrap_or(0xffffff);
            ((v >> 16) as u8, (v >> 8) as u8, v as u8)
        }
    };
    slint::Color::from_rgb_u8(r, g, b)
}

fn item(kind: &str, id: &str, label: &str) -> FaceItem {
    FaceItem { kind: kind.into(), id: id.into(), label: i18n::t(label).into(), ..FaceItem::default() }
}

fn rgb(hex: u32) -> slint::Color {
    slint::Color::from_rgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Capuchon et bague d'une touche d'après sa forme (« round small », « yellow », « ring-red »…),
/// couleurs de `.pk-key` (web/css/app.css).
fn key_look(it: &mut FaceItem, shape: &str) {
    let has = |w: &str| shape.split_whitespace().any(|x| x == w);
    it.round = has("round");
    it.small = has("small");
    it.rect = has("rect");
    it.icon = has("icon");
    it.logo = has("logo");
    it.dual = has("dual-key");
    it.link = has("link-key");
    let (light, dark) = if has("yellow") {
        (0xffe680, 0xd9a400)
    } else if has("green") {
        (0x7be38f, 0x1f8a3a)
    } else if has("red") {
        (0xff8a80, 0xb3261e)
    } else if has("blue") {
        (0x8cc4ff, 0x1d5fc4)
    } else {
        (0x44484f, 0x121417)
    };
    it.cap_light = rgb(light);
    it.cap_dark = rgb(dark);
    let ring = shape.split_whitespace().find_map(|w| w.strip_prefix("ring-")).and_then(|c| match c {
        "red" => Some(0xd9342b),
        "yellow" => Some(0xf2c230),
        "orange" => Some(0xf28a30),
        "lightblue" => Some(0x8cc8ff),
        "blue" => Some(0x3d7bff),
        "darkblue" => Some(0x2340a8),
        _ => None,
    });
    it.ring = ring.map_or(slint::Color::from_argb_u8(255, 0, 0, 0), rgb);
}

/// Rangées de la face avant : cases (un élément ou une colonne), voyants allumés, angle des
/// molettes, durée des appuis. `wrap` : une rangée trop large pour l'inspecteur passe à la ligne
/// (faces des marques ; les claviers Astera gardent leur disposition).
pub fn rows(rows: &[Vec<Element>], leds: &Leds, time: f64, held: &dyn Fn(&str) -> f64, angles: &std::collections::HashMap<String, f32>, wrap: bool) -> Vec<FaceRow> {
    let one = |e: &Element| -> Option<FaceItem> {
        let mut it = match e {
            Element::Led { id, label, .. } => {
                let mut it = item("led", id, label);
                if let Some(l) = leds.get(id) {
                    // clignotement : allumé la moitié de chaque période
                    it.lit = l.blink_hz <= 0.0 || (time * l.blink_hz) % 1.0 < 0.5;
                    it.color = led_color(&l.color);
                }
                it
            }
            Element::Key { id, label, shape, alt, .. } => {
                let mut it = item("key", id, label);
                it.shape = shape.as_str().into();
                it.sub = alt.as_deref().map(i18n::t).unwrap_or_default().into();
                key_look(&mut it, shape);
                it.held = held(id) as f32;
                it
            }
            Element::Knob { id, label, push, pointer, cap, size } => {
                let mut it = item("knob", id, label);
                it.push = *push;
                it.pointer = *pointer || cap != "blue";
                it.size = size.as_str().into();
                it.angle = angles.get(id).copied().unwrap_or(0.0);
                // capuchon : dégradé de `.pk-knob.blue`, `.red` ; bague jaune de `.yellow`
                let (light, dark) = match cap.as_str() {
                    "blue" => (0x6aa8ff, 0x1c4ea8),
                    "red" => (0xff7a6a, 0xa8261c),
                    _ => (0x3a3d43, 0x16181b),
                };
                it.cap_light = rgb(light);
                it.cap_dark = rgb(dark);
                it.ring = rgb(if cap == "yellow" { 0xe3b21c } else { 0x0b0c0e });
                it
            }
            Element::Text { text } => item("text", "", text),
            Element::Shake { label } => item("shake", "", label),
            Element::Display => item("display", "", ""),
            Element::IrWindow => item("ir", "", ""),
            Element::Slider => item("slider", "", ""),
            Element::Col { .. } => return None,
        };
        // faces des marques : légendes en capitales (`text-transform: uppercase`)
        if wrap {
            it.label = it.label.to_uppercase().into();
        }
        Some(it)
    };
    let cell = |e: &Element| -> Option<FaceCell> {
        match e {
            Element::Col { items, row } => {
                let list: Vec<FaceItem> = items.iter().filter_map(one).collect();
                (!list.is_empty()).then(|| FaceCell { col: true, row: *row, items: ModelRc::new(VecModel::from(list)) })
            }
            e => one(e).map(|it| FaceCell { col: false, row: false, items: ModelRc::new(VecModel::from(vec![it])) }),
        }
    };
    // largeur à l'écran d'une case (pixels, à peu près, tailles de `.pf`)
    fn width(c: &FaceCell) -> f32 {
        let w = |it: &FaceItem| -> f32 {
            let chars = |t: &SharedString| t.chars().count() as f32;
            let text = chars(&it.label).max(chars(&it.sub)) * 5.5;
            match it.kind.as_str() {
                "key" => (if it.rect { 30f32 } else if it.round { 30.0 } else { 24.0 }).max(text.min(76.0)),
                "knob" => (match it.size.as_str() {
                    "l" => 58f32,
                    "s" => 34.0,
                    _ => 46.0,
                })
                .max(text.min(76.0)),
                "led" => 8f32.max(text),
                "display" => 200.0,
                _ => text + 4.0,
            }
        };
        let items: Vec<FaceItem> = slint::Model::iter(&c.items).collect();
        if c.col && !c.row { items.iter().map(w).fold(0.0, f32::max) } else { items.iter().map(w).sum::<f32>() + 18.0 * items.len().saturating_sub(1) as f32 }
    }
    const BUDGET: f32 = 262.0;
    let mut out = Vec::new();
    for r in rows {
        let cells: Vec<FaceCell> = r.iter().filter_map(cell).collect();
        if !wrap {
            if !cells.is_empty() {
                out.push(FaceRow { cells: ModelRc::new(VecModel::from(cells)) });
            }
            continue;
        }
        let mut line: Vec<FaceCell> = Vec::new();
        let mut used = 0.0;
        for c in cells {
            let w = width(&c);
            if !line.is_empty() && used + 12.0 + w > BUDGET {
                out.push(FaceRow { cells: ModelRc::new(VecModel::from(std::mem::take(&mut line))) });
                used = 0.0;
            }
            used += if line.is_empty() { w } else { 12.0 + w };
            line.push(c);
        }
        if !line.is_empty() {
            out.push(FaceRow { cells: ModelRc::new(VecModel::from(line)) });
        }
    }
    out
}

/// Geste sur la face avant du projecteur `id`.
pub enum Gesture {
    Press(String),
    Release(String),
    Turn(String, i32),
    Shake,
}

/// Applique un geste ; renvoie les modifications du projet (une étape d'annulation).
pub fn gesture(st: &mut State, id: &str, g: Gesture) -> Option<Edit> {
    let time = st.clock.elapsed().as_secs_f64();
    let s = &mut *st;
    let f = s.doc.fixture(id)?;
    let env = Env { time, fixtures: &s.doc.project.fixtures, live: &s.live, app_connected: false };
    let rt = s.lights.runtime_mut(id);
    let out = match g {
        Gesture::Press(k) => s.onboard.press(f, rt, &env, &k),
        Gesture::Release(k) => s.onboard.release(f, rt, &env, &k),
        Gesture::Turn(k, n) => s.onboard.turn(f, rt, &env, &k, n),
        Gesture::Shake => {
            s.onboard.shake(f, rt, &env);
            Outcome::default()
        }
    };
    outcome_edit(s, id, out)
}

/// Modifications d'un résultat de geste (AsteraBox : projecteurs liés ou déliés, entrée à redétecter).
pub fn outcome_edit(st: &mut State, id: &str, out: Outcome) -> Option<Edit> {
    let mut edits = Vec::new();
    if !out.settings.is_empty() || !out.state.is_empty() {
        edits.push(Edit::UpdateFixture { id: id.to_owned(), changes: out.changes() });
    }
    for r in out.others {
        if !r.settings.is_empty() {
            edits.push(Edit::UpdateFixture { id: r.id.clone(), changes: Changes { settings: Some(r.settings.clone()), ..Changes::default() } });
        }
        if r.relatch {
            let rt = st.lights.runtime_mut(&r.id);
            st.onboard.relatch(&r.id, rt, r.unlinked);
        }
    }
    (!edits.is_empty()).then_some(Edit::Batch(edits))
}
