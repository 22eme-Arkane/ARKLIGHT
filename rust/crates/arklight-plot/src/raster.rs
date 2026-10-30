//! Image du plan (affichage à l'écran, export PNG) : le SVG du plan rendu par resvg. Les polices sont
//! lues dans leurs fichiers (Segoe UI, Arial et Consolas sous Windows ; Helvetica, Menlo sous
//! macOS ; DejaVu sous Linux ; Roboto sous Android), une fois pour toutes.

use resvg::usvg::fontdb::Database;
use resvg::{tiny_skia, usvg};
use std::borrow::Cow;
use std::sync::{Arc, OnceLock};

/// Fichiers de polices essayés (les absents sont ignorés).
fn font_files() -> Vec<String> {
    let mut v = Vec::new();
    let win = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    for f in ["segoeui.ttf", "segoeuib.ttf", "seguisb.ttf", "arial.ttf", "arialbd.ttf", "consola.ttf", "consolab.ttf"] {
        v.push(format!("{win}\\Fonts\\{f}"));
    }
    for f in [
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/Menlo.ttc",
        "/Library/Fonts/Arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        "/system/fonts/Roboto-Regular.ttf",
        "/system/fonts/Roboto-Bold.ttf",
        "/system/fonts/RobotoMono-Regular.ttf",
        "/system/fonts/DroidSansMono.ttf",
    ] {
        v.push(f.to_string());
    }
    v
}

/// Polices du plan (chargées au premier dessin).
fn fonts() -> Arc<Database> {
    static DB: OnceLock<Arc<Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = Database::new();
        for path in font_files() {
            if let Ok(data) = std::fs::read(&path) {
                db.load_font_data(data);
            }
        }
        let has = |db: &Database, name: &str| db.faces().any(|f| f.families.iter().any(|(n, _)| n == name));
        if let Some(sans) = ["Segoe UI", "Arial", "Helvetica", "DejaVu Sans", "Roboto"].into_iter().find(|n| has(&db, n)) {
            db.set_sans_serif_family(sans);
        }
        if let Some(mono) = ["Consolas", "Menlo", "DejaVu Sans Mono", "Roboto Mono", "Droid Sans Mono"].into_iter().find(|n| has(&db, n)) {
            db.set_monospace_family(mono);
        }
        Arc::new(db)
    })
    .clone()
}

/// resvg ignore `vector-effect="non-scaling-stroke"` : dans les symboles du décor (dessinés en
/// mètres dans un groupe `translate(…) rotate(…) scale(s)`), l'épaisseur des traits est divisée par
/// l'échelle du groupe pour garder des traits fins.
fn unscale_strokes(svg: &str) -> Cow<'_, str> {
    const NS: &str = r#"vector-effect="non-scaling-stroke""#;
    const SW: &str = r#"stroke-width=""#;
    if !svg.contains(NS) {
        return Cow::Borrowed(svg);
    }
    // groupes des symboles : position et échelle
    let mut groups: Vec<(usize, f64)> = Vec::new();
    let mut from = 0;
    while let Some(i) = svg[from..].find(r#"<g transform="translate("#) {
        let at = from + i;
        let end = svg[at..].find('>').map_or(svg.len(), |e| at + e);
        let tag = &svg[at..end];
        let scale = tag.find("scale(").and_then(|k| {
            let v = &tag[k + 6..];
            v[..v.find(')')?].split([' ', ',']).next()?.parse::<f64>().ok()
        });
        groups.push((at, scale.filter(|s| *s > 0.0).unwrap_or(1.0)));
        from = end;
    }
    let mut out = String::with_capacity(svg.len());
    let (mut copied, mut from) = (0, 0);
    while let Some(i) = svg[from..].find(NS) {
        let at = from + i;
        from = at + NS.len();
        let k = groups.partition_point(|g| g.0 < at);
        let scale = if k > 0 { groups[k - 1].1 } else { 1.0 };
        // « stroke-width="w" » juste avant
        let head = &svg[copied..at];
        let Some(start) = head.rfind(SW).map(|p| p + SW.len()) else { continue };
        let Some(len) = head[start..].find('"') else { continue };
        let Ok(w) = head[start..start + len].parse::<f64>() else { continue };
        if !head[start + len + 1..].trim().is_empty() {
            continue;
        }
        out.push_str(&head[..start]);
        out.push_str(&format!("{}\"", w / scale));
        copied = from;
    }
    out.push_str(&svg[copied..]);
    Cow::Owned(out)
}

/// Lit le SVG.
fn tree(svg: &str) -> Option<usvg::Tree> {
    let opt = usvg::Options { fontdb: fonts(), ..usvg::Options::default() };
    usvg::Tree::from_str(&unscale_strokes(svg), &opt).ok()
}

fn render(svg: &str, width: u32, height: u32) -> Option<tiny_skia::Pixmap> {
    let tree = tree(svg)?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    let size = tree.size();
    let ts = tiny_skia::Transform::from_scale(width as f32 / size.width(), height as f32 / size.height());
    resvg::render(&tree, ts, &mut pixmap.as_mut());
    Some(pixmap)
}

/// Image RGBA (8 bits, alpha non prémultiplié, ligne par ligne) d'un document SVG, mis à l'échelle
/// de `width` × `height` pixels ; vide si le SVG est illisible ou la taille nulle.
pub fn rasterize(svg: &str, width: u32, height: u32) -> Vec<u8> {
    let Some(pixmap) = render(svg, width, height) else { return Vec::new() };
    let mut out = Vec::with_capacity(pixmap.data().len());
    for p in pixmap.pixels() {
        let c = p.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    out
}

/// Taille propre d'un document SVG (en pixels CSS) ; None s'il est illisible.
pub fn svg_size(svg: &str) -> Option<(f64, f64)> {
    let size = tree(svg)?.size();
    Some((f64::from(size.width()), f64::from(size.height()))).filter(|(w, h)| *w > 0.0 && *h > 0.0)
}

/// Fichier PNG d'un document SVG (`width` × `height` pixels) : export « Plan en image PNG ».
pub fn svg_to_png(svg: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    render(svg, width, height)?.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_mal_forme_sans_image() {
        // attribut en double : XML refusé, pas d'image (le plan doit donc être bien formé)
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" stroke="red" stroke="none"/></svg>"##;
        assert!(rasterize(svg, 2, 2).is_empty());
    }

    #[test]
    fn traits_fins_des_symboles() {
        let s = r##"<g transform="translate(1,2) rotate(0) scale(40)"><line x1="0" stroke="#000" stroke-width="2" vector-effect="non-scaling-stroke" stroke-linecap="round"/><rect stroke-width="1" vector-effect="non-scaling-stroke" /></g>"##;
        assert_eq!(unscale_strokes(s), r##"<g transform="translate(1,2) rotate(0) scale(40)"><line x1="0" stroke="#000" stroke-width="0.05" stroke-linecap="round"/><rect stroke-width="0.025" /></g>"##);
    }

    #[test]
    fn image_rgba() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="0 0 20 10"><rect x="0" y="0" width="20" height="10" fill="#ff0000"/></svg>"##;
        let img = rasterize(svg, 40, 20);
        assert_eq!(img.len(), 40 * 20 * 4);
        assert_eq!(&img[..4], &[255, 0, 0, 255]);
        let png = svg_to_png(svg, 40, 20).unwrap();
        assert_eq!(&png[1..4], b"PNG");
    }
}
