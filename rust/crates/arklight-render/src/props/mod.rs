//! Objets de décor dans la vue 3D (portage de `web/js/viewport/prop-models.js` et
//! `web/js/viewport/props/`) : personnages, animaux, véhicules, mobilier, décor et extérieur,
//! accessoires et matériel de plateau. Chaque famille est un module (fonctionnalité Cargo du même
//! nom, toutes par défaut) ; un type inconnu devient un simple volume aux dimensions du catalogue.

pub mod kit;

#[cfg(feature = "accessories")]
mod accessories;
#[cfg(feature = "animals")]
mod animals;
#[cfg(feature = "furniture")]
mod furniture;
#[cfg(feature = "person")]
mod person;
#[cfg(feature = "set")]
mod set;
#[cfg(feature = "vehicles")]
mod vehicles;

use arklight_core::library::PropType;
use glam::Vec3;
use kit::{Built, Kit, Props, mo};
use serde_json::Value;

#[allow(dead_code)]
fn text<'a>(t: &'a PropType, key: &str) -> &'a str {
    t.extra.get(key).and_then(Value::as_str).unwrap_or("")
}

#[allow(unused_variables)]
fn by_category(t: &PropType, p: Props) -> Option<Built> {
    match t.category.as_deref().unwrap_or("") {
        #[cfg(feature = "person")]
        "people" => Some(person::build_person(p)),
        #[cfg(feature = "animals")]
        "animals" => Some(animals::build_animal(text(t, "species"), p)),
        #[cfg(feature = "vehicles")]
        "vehicles" => Some(vehicles::build_vehicle(text(t, "kind"), p)),
        #[cfg(feature = "furniture")]
        "furniture" => furniture::build_furniture(t, p),
        #[cfg(feature = "set")]
        "set" | "outdoor" => set::build_set(t, p),
        #[cfg(feature = "accessories")]
        "props" | "filmset" => accessories::build_accessory(t, p),
        _ => None,
    }
}

/// Modèle d'un objet de décor ; `props` : ses réglages (valeurs par défaut comprises).
pub fn build_prop(t: &PropType, props: &Value) -> Built {
    let p = Props(props);
    let built = by_category(t, p).unwrap_or_else(|| {
        // type inconnu de cette version : simple volume aux dimensions du catalogue
        let [w, h, d] = size(t, props);
        let mut k = Kit::new();
        let m = k.mat("box", p.col("color", "#8a8f96"), mo());
        k.boxed(w, h, d, m, [0.0, h / 2.0, 0.0], kit::o());
        k.build()
    });
    if built.aim.is_some() {
        return built;
    }
    let h = size(t, props)[1];
    built.aim(Vec3::new(0.0, h * 0.6, 0.0))
}

/// Encombrement [largeur, hauteur, profondeur] d'un objet selon ses réglages (`propSize`).
pub fn size(t: &PropType, props: &Value) -> [f32; 3] {
    let n = |k: &str| props.get(k).and_then(Value::as_f64).map(|v| v as f32);
    if t.model.as_deref() == Some("person") {
        let h = n("height").filter(|v| *v != 0.0).unwrap_or(175.0) / 100.0;
        let k = h / 1.75;
        return match props.get("pose").and_then(Value::as_str) {
            Some("walking") => [0.58 * k, h, 1.0 * k],
            Some("sitting") => [0.52 * k, 0.76 * h, 0.82 * k],
            Some("lying") => [0.72 * k, 0.4 * k, h],
            Some("pointing") => [0.6 * k, h, 0.85 * k],
            Some("handsup") => [0.92 * k, 1.16 * h, 0.38 * k],
            _ => [0.6 * k, h, 0.4 * k],
        };
    }
    let base = t.extra.get("baseSize").and_then(Value::as_array).map(|a| [0, 1, 2].map(|i| a.get(i).and_then(Value::as_f64).unwrap_or(0.5) as f32)).unwrap_or(t.size.map(|v| v as f32));
    [n("width").unwrap_or(base[0]), n("height").unwrap_or(base[1]), base[2]]
}
