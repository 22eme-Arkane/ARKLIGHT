//! Symboles du décor vus de dessus pour le plan de feu (portage de `web/js/plot/symbols.js`) :
//! arbres (feuillage découpé, pin en étoile, palmier), véhicules, mobilier, murs et portes, mobilier
//! urbain, accessoires, matériel de plateau. Dessinés en mètres dans le repère de l'objet (x : côté,
//! y : avant), puis placés et tournés ; couleurs de l'objet en aplat léger, traits fins.

use crate::catalog::{prop_size, props_of};
use crate::geometry::P2;
use crate::js::{f3, hypot, max2, min2, num, num_truthy, round, string_of, truthy};
use arklight_core::library::PropType;
use arklight_core::project::SceneObject;
use serde_json::Value;
use std::f64::consts::PI;

const NS: &str = r#"vector-effect="non-scaling-stroke""#;

/// Nombre arrondi au millième, écrit comme en JavaScript.
fn n3(v: f64) -> String {
    num(f3(v))
}

/// Couleur « #rrggbb » éclaircie (k > 0) ou assombrie (k < 0) ; autre texte : inchangé.
pub fn shade(hex: &str, k: f64) -> String {
    let h = hex.strip_prefix('#').unwrap_or(hex);
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return hex.to_string();
    }
    let n = u32::from_str_radix(h, 16).unwrap_or(0);
    let ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(|c| {
        let c = c as f64;
        round(if k >= 0.0 { c + (255.0 - c) * k } else { c * (1.0 + k) }) as i64
    });
    format!("#{:02x}{:02x}{:02x}", ch[0], ch[1], ch[2])
}

/// Petit nombre pseudo-aléatoire stable (formes des rochers…), tiré de l'identifiant de l'objet.
fn rand(seed: &str) -> impl FnMut() -> f64 {
    let mut h: u32 = 2166136261;
    for c in seed.encode_utf16() {
        h = (h ^ c as u32).wrapping_mul(16777619);
    }
    move || {
        h = (h ^ (h >> 15)).wrapping_mul(2246822507);
        h = (h ^ (h >> 13)).wrapping_mul(3266489909);
        h ^= h >> 16;
        h as f64 / 4294967296.0
    }
}

/// Outils de dessin dans le repère de l'objet.
struct Pen {
    stroke: String,
    sw: f64,
    fill_opacity: f64,
}

impl Pen {
    fn st(&self, s: Option<&str>, w: Option<f64>) -> String {
        format!(r#"stroke="{}" stroke-width="{}" {NS}"#, s.unwrap_or(&self.stroke), num(w.unwrap_or(self.sw)))
    }

    fn fo(&self, o: Option<f64>) -> String {
        num(o.unwrap_or(self.fill_opacity))
    }

    /// Un `stroke` dans `extra` remplace le trait par défaut : l'attribut n'est écrit qu'une fois (XML valide).
    #[allow(clippy::too_many_arguments)]
    fn rect_x(&self, x0: f64, y0: f64, x1: f64, y1: f64, fill: &str, o: Option<f64>, extra: &str) -> String {
        let own_stroke = extra.split_whitespace().any(|a| a.starts_with(r#"stroke=""#));
        format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{fill}" fill-opacity="{}" {}{extra}/>"#,
            n3(min2(x0, x1)),
            n3(min2(y0, y1)),
            n3((x1 - x0).abs()),
            n3((y1 - y0).abs()),
            self.fo(o),
            if own_stroke { String::new() } else { self.st(None, None) + " " }
        )
    }

    fn rect(&self, x0: f64, y0: f64, x1: f64, y1: f64, fill: &str, o: Option<f64>) -> String {
        self.rect_x(x0, y0, x1, y1, fill, o, "")
    }

    #[allow(clippy::too_many_arguments)]
    fn rrect(&self, x0: f64, y0: f64, x1: f64, y1: f64, r: f64, fill: &str, o: Option<f64>) -> String {
        format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{fill}" fill-opacity="{}" {}/>"#,
            n3(min2(x0, x1)),
            n3(min2(y0, y1)),
            n3((x1 - x0).abs()),
            n3((y1 - y0).abs()),
            n3(r),
            self.fo(o),
            self.st(None, None)
        )
    }

    fn circle(&self, x: f64, y: f64, r: f64, fill: &str, o: Option<f64>, s2: Option<&str>) -> String {
        format!(r#"<circle cx="{}" cy="{}" r="{}" fill="{fill}" fill-opacity="{}" {}/>"#, n3(x), n3(y), n3(r), self.fo(o), self.st(s2, None))
    }

    #[allow(clippy::too_many_arguments)]
    fn ellipse(&self, x: f64, y: f64, rx: f64, ry: f64, rot_deg: f64, fill: &str, o: Option<f64>) -> String {
        format!(
            r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" transform="rotate({} {} {})" fill="{fill}" fill-opacity="{}" {}/>"#,
            n3(x),
            n3(y),
            n3(rx),
            n3(ry),
            n3(rot_deg),
            n3(x),
            n3(y),
            self.fo(o),
            self.st(None, None)
        )
    }

    fn line(&self, x0: f64, y0: f64, x1: f64, y1: f64, s2: Option<&str>, w2: Option<f64>) -> String {
        format!(r#"<line x1="{}" y1="{}" x2="{}" y2="{}" {} stroke-linecap="round"/>"#, n3(x0), n3(y0), n3(x1), n3(y1), self.st(s2, w2))
    }

    fn poly(&self, pts: &[P2], fill: &str, o: Option<f64>) -> String {
        let p: Vec<String> = pts.iter().map(|[x, y]| format!("{},{}", n3(*x), n3(*y))).collect();
        format!(r#"<polygon points="{}" fill="{fill}" fill-opacity="{}" {} stroke-linejoin="round"/>"#, p.join(" "), self.fo(o), self.st(None, None))
    }

    fn path(&self, d: &str, fill: &str, o: Option<f64>, s2: Option<&str>, w2: Option<f64>) -> String {
        format!(r#"<path d="{d}" fill="{fill}" fill-opacity="{}" {} stroke-linejoin="round" stroke-linecap="round"/>"#, self.fo(o), self.st(s2, w2))
    }
}

/// Feuillage découpé (festons) : contour d'arbre vu de dessus.
fn scallops(r: f64, n: usize, seed: f64) -> String {
    let pts: Vec<P2> = (0..n)
        .map(|i| {
            let a = (i as f64 / n as f64) * PI * 2.0 + seed;
            [a.cos() * r * 0.86, a.sin() * r * 0.86]
        })
        .collect();
    let mut d = format!("M{},{}", n3(pts[0][0]), n3(pts[0][1]));
    for i in 1..=n {
        let [x, y] = pts[i % n];
        let [px, py] = pts[i - 1];
        let rr = hypot(x - px, y - py) * 0.62;
        d += &format!(" A{},{} 0 0 1 {},{}", n3(rr), n3(rr), n3(x), n3(y));
    }
    format!("{d} Z")
}

fn star(r: f64, n: usize, inner: f64) -> Vec<P2> {
    (0..n * 2)
        .map(|i| {
            let a = (i as f64 / (n * 2) as f64) * PI * 2.0;
            let rr = if i % 2 == 1 { r * inner } else { r };
            [a.cos() * rr, a.sin() * rr]
        })
        .collect()
}

/// Ce que reçoit chaque symbole.
struct C<'a> {
    w: f64,
    d: f64,
    color: String,
    stroke: String,
    props: &'a Value,
    id: &'a str,
    type_id: &'a str,
    p: Pen,
}

/// `props.key || défaut` sur un texte (couleurs des vêtements…).
fn pstr(props: &Value, key: &str, d: &str) -> String {
    let v = props.get(key);
    if truthy(v) { string_of(v.unwrap_or(&Value::Null)) } else { d.to_string() }
}

const U: Option<f64> = None;
const S: Option<&str> = None;

fn o(v: f64) -> Option<f64> {
    Some(v)
}

/// Symbole d'un modèle ; None : emprise simple (rectangle arrondi).
fn draw(model: &str, c: &C) -> Option<String> {
    let (p, w, d) = (&c.p, c.w, c.d);
    let color = c.color.as_str();
    Some(match model {
        "person" => {
            let props = c.props;
            let k = num_truthy(props.get("height")).unwrap_or(175.0) / 175.0;
            let top = pstr(props, "top", "#3d4f6b");
            let pose = props.get("pose").and_then(Value::as_str);
            if pose == Some("lying") {
                return Some(p.rrect(-0.22 * k, -0.85 * k, 0.22 * k, 0.75 * k, 0.08, &top, U) + &p.circle(0.0, -0.82 * k, 0.11 * k, &pstr(props, "skin", "#e0aa83"), o(0.8), S));
            }
            let sitting = pose == Some("sitting");
            let back = if sitting { -0.08 } else { 0.0 };
            let legs = if sitting { p.rrect(-0.18 * k, 0.05, 0.18 * k, 0.48 * k, 0.05, &pstr(props, "pants", "#2a2e35"), U) } else { String::new() };
            let arm = if pose == Some("pointing") { p.line(-0.2 * k, back, -0.2 * k, back + 0.6 * k, S, U) } else { String::new() };
            legs + &p.ellipse(0.0, back, 0.24 * k, 0.12 * k, 0.0, &top, o(0.75)) + &arm + &p.circle(0.0, back, 0.1 * k, &pstr(props, "hairColor", "#2b1d14"), o(0.85), S) + &p.line(0.0, back + 0.1 * k, 0.0, back + 0.19 * k, S, U)
        }
        "animal" => p.ellipse(0.0, -d * 0.08, w / 2.0, d * 0.36, 0.0, color, o(0.65)) + &p.circle(0.0, d * 0.38, max2(0.05, w * 0.32), color, o(0.75), S) + &p.line(0.0, -d * 0.44, w * 0.15, -d * 0.55, S, U),
        "vehicle" => {
            let ty = c.type_id;
            let stroke = c.stroke.as_str();
            if ty.contains("bicycle") {
                return Some(p.line(0.0, -d / 2.0, 0.0, d / 2.0, Some(stroke), o(2.0)) + &p.line(-w * 0.4, d * 0.32, w * 0.4, d * 0.32, S, U) + &p.circle(0.0, -d * 0.05, 0.08, color, o(0.8), S));
            }
            if ty.contains("motorbike") || ty.contains("scooter") {
                return Some(p.rrect(-w * 0.32, -d / 2.0, w * 0.32, d / 2.0, w * 0.3, color, U) + &p.line(-w / 2.0, d * 0.3, w / 2.0, d * 0.3, S, U) + &p.ellipse(0.0, -d * 0.05, w * 0.18, d * 0.18, 0.0, &shade(color, -0.3), o(0.7)));
            }
            if ["truck", "van", "caravan", "generator"].iter().any(|k| ty.contains(k)) {
                let cab = if ty.contains("truck") {
                    d * 0.26
                } else if ty.contains("van") {
                    d * 0.18
                } else {
                    0.0
                };
                let mut s = p.rrect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, 0.12, color, U);
                if cab != 0.0 {
                    s += &p.line(-w / 2.0, d / 2.0 - cab, w / 2.0, d / 2.0 - cab, S, U);
                    s += &p.poly(&[[-w * 0.42, d / 2.0 - cab * 0.35], [w * 0.42, d / 2.0 - cab * 0.35], [w * 0.38, d / 2.0 - 0.08], [-w * 0.38, d / 2.0 - 0.08]], "#9fc6e8", o(0.6));
                }
                if ty.contains("generator") {
                    for i in 1..6 {
                        let y = -d / 2.0 + (d * i as f64) / 6.0;
                        s += &p.line(-w * 0.35, y, w * 0.35, y, Some(&shade(color, -0.4)), o(1.0));
                    }
                }
                if ty.contains("caravan") {
                    s += &p.line(w / 2.0, -d * 0.1, w / 2.0 - 0.05, d * 0.15, Some(stroke), o(2.0));
                }
                return Some(s);
            }
            // voiture : carrosserie arrondie, pare-brise, lunette arrière, toit
            p.rrect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, min2(w, d) * 0.22, color, U)
                + &p.poly(&[[-w * 0.42, d * 0.12], [w * 0.42, d * 0.12], [w * 0.36, d * 0.27], [-w * 0.36, d * 0.27]], "#9fc6e8", o(0.65))
                + &p.poly(&[[-w * 0.38, -d * 0.3], [w * 0.38, -d * 0.3], [w * 0.4, -d * 0.2], [-w * 0.4, -d * 0.2]], "#9fc6e8", o(0.5))
                + &p.rect(-w * 0.4, -d * 0.2, w * 0.4, d * 0.12, &shade(color, -0.25), o(0.55))
        }
        "tree" => {
            let r = w / 2.0;
            let seed = rand(c.id)() * PI;
            p.path(&scallops(r, 9, seed), color, o(0.42), Some(&shade(color, -0.35)), o(1.2)) + &p.path(&scallops(r * 0.55, 6, seed + 0.4), &shade(color, 0.25), o(0.35), Some("none"), U) + &p.circle(0.0, 0.0, max2(0.08, r * 0.08), "#6b4a2a", o(0.9), S)
        }
        "pine" => {
            let r = w / 2.0;
            p.poly(&star(r, 12, 0.72), color, o(0.5)) + &p.poly(&star(r * 0.55, 9, 0.7), &shade(color, 0.2), o(0.45)) + &p.circle(0.0, 0.0, max2(0.06, r * 0.07), "#5a3d22", o(0.9), S)
        }
        "palm" => {
            let r = w / 2.0;
            let mut s = String::new();
            for i in 0..7 {
                let a = (i as f64 / 7.0) * 360.0;
                let rad = (a * PI) / 180.0;
                s += &p.ellipse(rad.cos() * r * 0.5, rad.sin() * r * 0.5, r * 0.5, r * 0.11, a, color, o(0.55));
            }
            s + &p.circle(0.0, 0.0, max2(0.08, r * 0.1), "#7a5a32", o(0.9), S)
        }
        "bush" => format!(r#"<g transform="scale(1 {})">{}</g>"#, n3(d / w), p.path(&scallops(w / 2.0, 7, rand(c.id)() * 3.0), color, o(0.45), Some(&shade(color, -0.35)), o(1.0))),
        "plant" => {
            let r = w / 2.0;
            let mut s = p.circle(0.0, 0.0, r * 0.45, color, o(0.85), S);
            for i in 0..6 {
                let a = (i as f64 / 6.0) * 360.0 + 15.0;
                let rad = (a * PI) / 180.0;
                s += &p.ellipse(rad.cos() * r * 0.55, rad.sin() * r * 0.55, r * 0.42, r * 0.16, a, "#4f8a3a", o(0.6));
            }
            s
        }
        "rock" => {
            let mut rnd = rand(c.id);
            let pts: Vec<P2> = (0..9)
                .map(|i| {
                    let a = (i as f64 / 9.0) * PI * 2.0;
                    let k = 0.75 + rnd() * 0.25;
                    [(a.cos() * w * k) / 2.0, (a.sin() * d * k) / 2.0]
                })
                .collect();
            p.poly(&pts, color, o(0.6))
        }
        // ---- mobilier
        "chair" => p.rrect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, 0.04, color, U) + &p.rect(-w / 2.0, -d / 2.0, w / 2.0, -d / 2.0 + 0.07, &shade(color, -0.3), o(0.8)),
        "directorchair" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.5)) + &p.line(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, S, U) + &p.line(w / 2.0, -d / 2.0, -w / 2.0, d / 2.0, S, U) + &p.rect(-w / 2.0, -d / 2.0, w / 2.0, -d / 2.0 + 0.06, &shade(color, -0.3), o(0.9)),
        "stool" => p.circle(0.0, 0.0, w / 2.0, color, U, S),
        "barstool" => p.circle(0.0, 0.0, w / 2.0, color, U, S) + &p.circle(0.0, 0.0, w / 4.0, &shade(color, -0.3), o(0.6), S),
        "armchair" => {
            let t = 0.14;
            let sh = shade(color, -0.25);
            p.rrect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, 0.08, color, o(0.45)) + &p.rect(-w / 2.0, -d / 2.0, w / 2.0, -d / 2.0 + 0.18, &sh, U) + &p.rect(-w / 2.0, -d / 2.0, -w / 2.0 + t, d / 2.0, &sh, U) + &p.rect(w / 2.0 - t, -d / 2.0, w / 2.0, d / 2.0, &sh, U)
        }
        "sofa" => {
            let t = 0.18;
            let seats = max2(2.0, round((w - 2.0 * t) / 0.6));
            let sh = shade(color, -0.25);
            let mut s = p.rrect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, 0.08, color, o(0.45)) + &p.rect(-w / 2.0, -d / 2.0, w / 2.0, -d / 2.0 + 0.22, &sh, U) + &p.rect(-w / 2.0, -d / 2.0, -w / 2.0 + t, d / 2.0, &sh, U) + &p.rect(w / 2.0 - t, -d / 2.0, w / 2.0, d / 2.0, &sh, U);
            let mut i = 1.0;
            while i < seats {
                let x = -w / 2.0 + t + ((w - 2.0 * t) * i) / seats;
                s += &p.line(x, -d / 2.0 + 0.22, x, d / 2.0, Some(&shade(color, -0.4)), o(1.0));
                i += 1.0;
            }
            s
        }
        "table" => p.rrect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, 0.03, color, o(0.6)),
        "roundtable" => p.circle(0.0, 0.0, w / 2.0, color, o(0.6), S) + &p.circle(0.0, 0.0, w / 2.0 - 0.05, "none", o(0.0), Some(&shade(color, -0.4))),
        "coffeetable" => p.rrect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, 0.05, color, o(0.55)),
        "desk" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.55)) + &p.rect(-w * 0.2, -d / 2.0 + 0.05, w * 0.2, -d / 2.0 + 0.12, "#30343a", o(0.8)),
        "bed" => {
            let pillows = if w > 1.2 { 2.0 } else { 1.0 };
            let mut s = p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, "#8a6a4a", o(0.5)) + &p.rect(-w / 2.0 + 0.04, -d / 2.0 + 0.45, w / 2.0 - 0.04, d / 2.0 - 0.04, color, o(0.6));
            let mut i = 0.0;
            while i < pillows {
                let x0 = -w / 2.0 + 0.08 + (i * (w - 0.16)) / pillows;
                s += &p.rrect(x0, -d / 2.0 + 0.08, x0 + (w - 0.16) / pillows - 0.06, -d / 2.0 + 0.38, 0.06, "#f2f0ea", o(0.85));
                i += 1.0;
            }
            s
        }
        "nightstand" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.6)),
        "wardrobe" | "dresser" => {
            let sh = shade(color, -0.4);
            p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.6)) + &p.line(0.0, -d / 2.0, 0.0, d / 2.0, Some(&sh), o(1.0)) + &p.line(-w / 2.0, d / 2.0, w / 2.0, d / 2.0, Some(&sh), o(2.0))
        }
        "bookcase" => {
            let mut s = p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.55));
            let books = ["#9b3b32", "#355f8c", "#d8b04a", "#4c7a4a", "#7a4e8c"];
            let n = max2(4.0, round(w / 0.12));
            let mut i = 0.0;
            while i < n {
                let x0 = -w / 2.0 + 0.03 + (i * (w - 0.06)) / n;
                s += &p.rect_x(x0, -d / 2.0 + 0.04, x0 + (w - 0.06) / n - 0.01, d / 2.0 - 0.05, books[i as usize % books.len()], o(0.7), r#"stroke="none""#);
                i += 1.0;
            }
            s
        }
        "counter" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.6)) + &p.rect(-w / 2.0, d / 2.0 - 0.08, w / 2.0, d / 2.0, &shade(color, -0.35), o(0.8)),
        "kitchen" => {
            let mut s = p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.55)) + &p.rrect(-w * 0.3, -d * 0.3, -w * 0.05, d * 0.25, 0.04, "#b9c2cc", o(0.8));
            for x in [w * 0.12, w * 0.3] {
                s += &(p.circle(x, -d * 0.12, d * 0.13, "#2a2d31", o(0.8), S) + &p.circle(x, d * 0.18, d * 0.11, "#2a2d31", o(0.8), S));
            }
            s
        }
        "fridge" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.7)) + &p.line(-w / 2.0, d * 0.15, w / 2.0, d * 0.15, Some(&shade(color, -0.4)), o(1.0)),
        "tv" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.5)) + &p.rect(-w * 0.45, d * 0.1, w * 0.45, d * 0.35, "#111317", o(0.9)),
        "piano" => {
            let mut s = p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.75)) + &p.rect(-w / 2.0 + 0.05, d / 2.0 - 0.16, w / 2.0 - 0.05, d / 2.0, "#f4f2ec", o(0.95));
            for i in 1..12 {
                let x = -w / 2.0 + 0.05 + ((w - 0.1) * i as f64) / 12.0;
                s += &p.line(x, d / 2.0 - 0.16, x, d / 2.0 - 0.06, Some("#222"), o(1.0));
            }
            s
        }
        "floorlamp" => p.circle(0.0, 0.0, w / 2.0, "#ffe4a3", o(0.55), S) + &p.circle(0.0, 0.0, w / 8.0, color, o(0.9), S),
        "rug" => {
            let mut s = p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.3)) + &p.rect_x(-w / 2.0 + 0.1, -d / 2.0 + 0.1, w / 2.0 - 0.1, d / 2.0 - 0.1, "none", o(0.0), r#"stroke-dasharray="4 3""#);
            for i in 0..=8 {
                let y = -d / 2.0 + (d * i as f64) / 8.0;
                s += &(p.line(-w / 2.0 - 0.06, y, -w / 2.0, y, S, o(1.0)) + &p.line(w / 2.0, y, w / 2.0 + 0.06, y, S, o(1.0)));
            }
            s
        }
        "coatrack" => {
            let mut s = p.circle(0.0, 0.0, w / 2.0, color, o(0.35), S);
            for i in 0..4 {
                let a = (i as f64 / 4.0) * PI * 2.0;
                s += &p.line(0.0, 0.0, (a.cos() * w) / 2.0, (a.sin() * w) / 2.0, S, U);
            }
            s
        }
        "divider" => {
            let n = 3.0;
            let pts: Vec<String> = (0..=3).map(|i| format!("{},{}", n3(-w / 2.0 + (w * i as f64) / n), n3(if i % 2 == 1 { d / 2.0 } else { -d / 2.0 }))).collect();
            p.path(&format!("M{}", pts.join(" L")), "none", o(0.0), Some(&shade(color, -0.2)), o(3.0))
        }
        // ---- murs, portes, fenêtres, structures
        "wall" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.85)),
        "walldoor" => {
            let door = min2(0.9, w * 0.4);
            p.rect(-w / 2.0, -d / 2.0, -door / 2.0, d / 2.0, color, o(0.85))
                + &p.rect(door / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.85))
                + &p.line(-door / 2.0, d / 2.0, -door / 2.0, d / 2.0 + door, S, o(1.2))
                + &p.path(&format!("M{},{} A{},{} 0 0 0 {},{}", n3(-door / 2.0), n3(d / 2.0 + door), n3(door), n3(door), n3(door / 2.0), n3(d / 2.0)), "none", o(0.0), S, o(1.0))
        }
        "wallwindow" => {
            let win = min2(1.4, w * 0.5);
            p.rect(-w / 2.0, -d / 2.0, -win / 2.0, d / 2.0, color, o(0.85)) + &p.rect(win / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.85)) + &p.rect(-win / 2.0, -d / 2.0, win / 2.0, d / 2.0, "#bfe0f5", o(0.5)) + &p.line(-win / 2.0, 0.0, win / 2.0, 0.0, S, o(1.0))
        }
        "door" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.7)) + &p.path(&format!("M{},{} L{},{} A{},{} 0 0 0 {},{}", n3(-w / 2.0), n3(d / 2.0), n3(-w / 2.0), n3(d / 2.0 + w), n3(w), n3(w), n3(w / 2.0), n3(d / 2.0)), "none", o(0.0), S, o(1.0)),
        "window" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, "#bfe0f5", o(0.5)) + &p.line(-w / 2.0, 0.0, w / 2.0, 0.0, S, o(1.0)),
        "stairs" => {
            let mut s = p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.45));
            for i in 1..6 {
                let y = -d / 2.0 + (d * i as f64) / 6.0;
                s += &p.line(-w / 2.0, y, w / 2.0, y, S, o(1.0));
            }
            s + &p.line(0.0, d * 0.4, 0.0, -d * 0.4, S, o(1.5)) + &p.path(&format!("M{},{} L0,{} L{},{}", n3(-0.12), n3(-d * 0.3), n3(-d * 0.4), n3(0.12), n3(-d * 0.3)), "none", o(0.0), S, o(1.5))
        }
        "riser" => {
            let mut s = p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.45));
            let mut x = -w / 2.0 + 0.25;
            while x < w / 2.0 + d {
                let x0 = max2(-w / 2.0, x - d);
                s += &p.line(x0, min2(d / 2.0, -d / 2.0 + (x - x0) - 0.0), min2(x, w / 2.0), -d / 2.0, S, o(0.6));
                x += 0.3;
            }
            s
        }
        "column" => p.circle(0.0, 0.0, w / 2.0, color, o(0.75), S),
        "drape" => {
            let n = max2(4.0, round(w / 0.3));
            let mut path = format!("M{},0", n3(-w / 2.0));
            let mut i = 0.0;
            while i < n {
                path += &format!(" q{},{} {},0", n3(w / n / 2.0), if i % 2.0 == 1.0 { "-0.1" } else { "0.1" }, n3(w / n));
                i += 1.0;
            }
            p.path(&path, "none", o(0.0), Some(&shade(color, 0.3)), o(3.0))
        }
        // ---- rue
        "streetlamp" => p.circle(0.0, -d / 2.0 + 0.12, 0.12, "#40454c", o(0.9), S) + &p.line(0.0, -d / 2.0 + 0.12, 0.0, d / 2.0 - 0.15, S, o(2.0)) + &p.ellipse(0.0, d / 2.0 - 0.12, 0.18, 0.12, 0.0, "#ffe08a", o(0.75)),
        "bench" => {
            let mut s = String::new();
            for i in 0..4 {
                let y0 = -d / 2.0 + (d * i as f64) / 4.0;
                s += &p.rect(-w / 2.0, y0 + 0.02, w / 2.0, y0 + d / 4.0 - 0.02, color, o(0.65));
            }
            s
        }
        "bin" => p.circle(0.0, 0.0, w / 2.0, color, o(0.7), S) + &p.circle(0.0, 0.0, w / 2.0 - 0.06, "#1a1c1f", o(0.6), S),
        "barrier" => {
            let mut s = p.rect(-w / 2.0, -0.03, w / 2.0, 0.03, color, o(0.8));
            for x in [-w / 2.0 + 0.1, w / 2.0 - 0.1] {
                s += &p.line(x, -d / 2.0, x, d / 2.0, S, o(2.0));
            }
            s
        }
        "cone" => p.rect(-w / 2.0, -w / 2.0, w / 2.0, w / 2.0, "#30343a", o(0.6)) + &p.circle(0.0, 0.0, w * 0.35, color, o(0.9), S) + &p.circle(0.0, 0.0, w * 0.15, "#f4f4f0", o(0.9), S),
        "sign" => p.circle(0.0, 0.0, 0.04, "#9aa3ae", o(0.9), S) + &p.rect(-w / 2.0, -0.03, w / 2.0, 0.03, color, o(0.85)),
        "trafficlight" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, "#25282c", o(0.85)) + &p.circle(0.0, 0.0, min2(w, d) * 0.25, "#ff4b3a", o(0.9), S),
        // ---- accessoires
        "suitcase" => p.rrect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, 0.04, color, o(0.7)) + &p.line(-w * 0.15, -d / 2.0, w * 0.15, -d / 2.0, S, o(2.0)),
        "cardboard" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.7)) + &p.line(-w / 2.0, 0.0, w / 2.0, 0.0, Some("#d9c49a"), o(2.0)),
        "crate" => {
            let mut s = p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.6));
            for i in 1..4 {
                let y = -d / 2.0 + (d * i as f64) / 4.0;
                s += &p.line(-w / 2.0, y, w / 2.0, y, Some(&shade(color, -0.4)), o(1.0));
            }
            s
        }
        "flightcase" => {
            let k = min2(w, d) * 0.18;
            let mut s = p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.85));
            for [sx, sy] in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
                s += &p.rect((sx * w) / 2.0, (sy * d) / 2.0, (sx * w) / 2.0 - sx * k, (sy * d) / 2.0 - sy * k, "#b8bdc4", o(0.9));
            }
            s
        }
        "bottle" => p.circle(0.0, 0.0, max2(0.03, w / 2.0), color, o(0.85), S),
        "wineglass" => p.circle(0.0, 0.0, max2(0.03, w / 2.0), color, o(0.5), S),
        "mug" => p.circle(0.0, 0.0, max2(0.03, w / 2.0), color, o(0.8), S) + &p.line(w / 2.0, 0.0, w / 2.0 + 0.03, 0.0, S, o(2.0)),
        "books" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.75)) + &p.rect(-w / 2.0 + 0.02, -d / 2.0 + 0.03, w / 2.0 - 0.03, d / 2.0 - 0.02, "#f4f2ec", o(0.6)),
        "laptop" => p.rect(-w / 2.0, -d / 2.0, w / 2.0, d / 2.0, color, o(0.7)) + &p.line(-w / 2.0, -d / 2.0 + 0.01, w / 2.0, -d / 2.0 + 0.01, Some("#9fc6e8"), o(2.0)),
        "guitar" => p.ellipse(0.0, d * 0.22, w * 0.5, d * 0.22, 0.0, color, o(0.75)) + &p.ellipse(0.0, -d * 0.02, w * 0.38, d * 0.15, 0.0, color, o(0.75)) + &p.rect(-0.025, -d / 2.0, 0.025, -d * 0.12, "#3b2a1a", o(0.9)),
        "micstand" => p.circle(0.0, 0.0, w / 2.0, "#30343a", o(0.5), S) + &p.line(0.0, 0.0, 0.0, d / 2.0, S, o(2.0)) + &p.circle(0.0, d / 2.0, 0.04, "#111", o(0.9), S),
        // ---- plateau
        "camera" => {
            let mut s = String::new();
            for i in 0..3 {
                let a = (i as f64 / 3.0) * PI * 2.0 + PI / 2.0;
                s += &p.line(0.0, 0.0, (a.cos() * w) / 2.0, (a.sin() * w) / 2.0, S, o(1.5));
            }
            s + &p.rrect(-0.12, -0.2, 0.12, 0.22, 0.03, color, o(0.9)) + &p.rect(-0.06, 0.22, 0.06, 0.38, "#111317", o(0.95))
        }
        "dolly" => {
            p.line(-w * 0.35, -d / 2.0, -w * 0.35, d / 2.0, Some("#9aa3ae"), o(2.0))
                + &p.line(w * 0.35, -d / 2.0, w * 0.35, d / 2.0, Some("#9aa3ae"), o(2.0))
                + &p.rect(-w * 0.42, -0.45, w * 0.42, 0.45, color, o(0.75))
                + &p.rrect(-0.12, -0.15, 0.12, 0.25, 0.03, "#2b2d31", o(0.95))
                + &p.rect(-0.06, 0.25, 0.06, 0.4, "#111317", o(0.95))
        }
        "monitor" => p.circle(0.0, 0.0, w * 0.35, color, o(0.35), S) + &p.rect(-w * 0.35, 0.05, w * 0.35, 0.12, "#111317", o(0.95)),
        "boom" => p.circle(0.0, -d / 2.0 + 0.2, 0.25, "#30343a", o(0.35), S) + &p.line(0.0, -d / 2.0 + 0.2, 0.0, d / 2.0, Some("#9aa3ae"), o(2.0)) + &p.ellipse(0.0, d / 2.0 - 0.08, 0.04, 0.12, 0.0, "#4b4f55", o(0.95)),
        _ => return None,
    })
}

/// Couleur principale de l'objet (réglage « couleur » du modèle).
fn color_of(o: &SceneObject, ty: &PropType) -> String {
    let props = props_of(o);
    if truthy(props.get("color")) {
        return string_of(&props["color"]);
    }
    let prm = ty.extra.get("params").and_then(Value::as_array).and_then(|a| a.iter().find(|x| x.get("key").and_then(Value::as_str) == Some("color")));
    match prm.and_then(|p| p.get("default")) {
        Some(v) if truthy(Some(v)) => string_of(v),
        _ => "#8a919c".into(),
    }
}

/// Symbole SVG d'un objet de décor vu de dessus, placé et orienté sur le plan : (`x`, `y`) position à
/// l'écran (px), `s` échelle (px/m), `stroke` couleur du trait (sélection ou thème).
pub fn prop_symbol(o: &SceneObject, ty: &PropType, x: f64, y: f64, s: f64, stroke: &str, is_sel: bool) -> String {
    let yaw = o.rotation[1];
    let yaw = if yaw.is_nan() { 0.0 } else { yaw };
    let props = props_of(o);
    let [w, _, d] = prop_size(ty, props);
    let sw = if is_sel { 2.0 } else { 1.0 };
    let c = C { w, d, color: color_of(o, ty), stroke: stroke.to_string(), props, id: &o.id, type_id: &ty.id, p: Pen { stroke: stroke.to_string(), sw, fill_opacity: 0.55 } };
    let body = ty.model.as_deref().and_then(|m| draw(m, &c)).unwrap_or_else(|| c.p.rrect(-c.w / 2.0, -c.d / 2.0, c.w / 2.0, c.d / 2.0, 0.03, &c.color, U));
    format!(r#"<g transform="translate({},{}) rotate({}) scale({})">{body}</g>"#, n3(x), n3(y), n3((-yaw * 180.0) / PI), n3(s))
}
