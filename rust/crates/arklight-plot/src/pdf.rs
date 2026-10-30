//! Export PDF du plan : une page A4 paysage avec l'image du plan (le même contenu que l'export
//! « Plan en SVG », rendu par resvg à 300 ppp), sans bibliothèque PDF. Le fichier est écrit à la
//! main : catalogue, page, image (RVB 8 bits compressée en FlateDecode), contenu qui la pose sur
//! la page, informations (titre, logiciel), table des renvois.

use crate::export::{PlotSvgOptions, plan_text, plot_svg};
use crate::raster::rasterize;
use crate::render::{PlotData, tr};
use arklight_core::project::Project;

/// Page A4 paysage, en points PDF (1/72 de pouce).
pub const A4_LANDSCAPE: (f64, f64) = (841.89, 595.28);

/// Résolution de l'image du plan dans le PDF (points par pouce).
pub const PDF_DPI: f64 = 300.0;

/// `plot_pdf` : le plan en PDF, une page A4 paysage (image à 300 ppp).
pub fn plot_pdf(project: &Project, opts: &PlotSvgOptions) -> Vec<u8> {
    plot_pdf_at(project, opts, PDF_DPI)
}

/// Le plan en PDF, image à `dpi` points par pouce (150 : fichier plus léger).
pub fn plot_pdf_at(project: &Project, opts: &PlotSvgOptions, dpi: f64) -> Vec<u8> {
    let svg = plot_svg(project, opts);
    let pw = A4_LANDSCAPE.0;
    // l'image couvre la largeur de la page, aux proportions du SVG
    let w = (pw / 72.0 * dpi).round().max(1.0) as u32;
    let h = (f64::from(w) * opts.height / opts.width).round().max(1.0) as u32;
    let rgba = rasterize(&svg, w, h);
    let (rgb, w, h) = if rgba.len() == (w as usize) * (h as usize) * 4 { (rgba_to_rgb(&rgba), w, h) } else { (vec![255, 255, 255], 1, 1) }; // SVG illisible : page blanche
    image_pdf(&rgb, w, h, A4_LANDSCAPE, &pdf_title(project, opts))
}

/// Titre du document (« Plan de feu — Production — Séquence › Plan »).
fn pdf_title(project: &Project, opts: &PlotSvgOptions) -> String {
    let production = PlotData::of(project).title.production.filter(|p| !p.is_empty()).unwrap_or_else(|| project.name.clone());
    let doc: Vec<String> = [production, plan_text(project)].into_iter().filter(|s| !s.is_empty()).collect();
    let head = tr(opts.tr, "Plan de feu");
    if doc.is_empty() { head } else { format!("{head} — {}", doc.join(" — ")) }
}

/// Pixels RVBA (alpha non prémultiplié) posés sur du blanc : RVB.
pub fn rgba_to_rgb(rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(rgba.len() / 4 * 3);
    for [r, g, b, a] in rgba.as_chunks::<4>().0 {
        let a = u32::from(*a);
        for c in [r, g, b] {
            out.push(((u32::from(*c) * a + 255 * (255 - a) + 127) / 255) as u8);
        }
    }
    out
}

/// Nombre écrit dans le PDF (au plus 3 décimales, sans zéros inutiles).
fn pdf_num(x: f64) -> String {
    let s = format!("{x:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// Texte PDF en UTF-16 (gros-boutiste, avec marque d'ordre) : `<FEFF…>`.
fn pdf_text(s: &str) -> String {
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        out.push_str(&format!("{u:04X}"));
    }
    out.push('>');
    out
}

/// PDF d'une page `page` (largeur, hauteur en points) avec une image RVB 8 bits de `width` ×
/// `height` pixels, centrée et agrandie au plus grand sans la déformer.
pub fn image_pdf(rgb: &[u8], width: u32, height: u32, page: (f64, f64), title: &str) -> Vec<u8> {
    let (pw, ph) = page;
    let (iw, ih) = (f64::from(width), f64::from(height));
    let k = (pw / iw).min(ph / ih);
    let (dw, dh) = (iw * k, ih * k);
    let content = format!("q\n{} 0 0 {} {} {} cm\n/Im1 Do\nQ\n", pdf_num(dw), pdf_num(dh), pdf_num((pw - dw) / 2.0), pdf_num((ph - dh) / 2.0));
    let image = miniz_oxide::deflate::compress_to_vec_zlib(rgb, 6);

    let mut out: Vec<u8> = Vec::with_capacity(image.len() + 2048);
    // en-tête, puis une ligne de commentaire en octets > 127 : fichier binaire
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets: Vec<usize> = Vec::new();
    let mut object = |out: &mut Vec<u8>, dict: &str, stream: Option<&[u8]>| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{dict}\n", offsets.len()).as_bytes());
        if let Some(data) = stream {
            out.extend_from_slice(b"stream\n");
            out.extend_from_slice(data);
            out.extend_from_slice(b"\nendstream\n");
        }
        out.extend_from_slice(b"endobj\n");
    };
    object(&mut out, "<< /Type /Catalog /Pages 2 0 R >>", None);
    object(&mut out, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>", None);
    object(&mut out, &format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /XObject << /Im1 4 0 R >> >> /Contents 5 0 R >>", pdf_num(pw), pdf_num(ph)), None);
    object(&mut out, &format!("<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>", image.len()), Some(&image));
    object(&mut out, &format!("<< /Length {} >>", content.len()), Some(content.as_bytes()));
    object(&mut out, &format!("<< /Title {} /Producer (ARKLIGHT) /Creator (ARKLIGHT) >>", pdf_text(title)), None);
    let xref = out.len();
    let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
    for o in &offsets {
        table.push_str(&format!("{o:010} 00000 n \n"));
    }
    table.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1, offsets.len()));
    out.extend_from_slice(table.as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_et_textes() {
        assert_eq!(pdf_num(841.89), "841.89");
        assert_eq!(pdf_num(12.0), "12");
        assert_eq!(pdf_num(-0.0001), "0");
        assert_eq!(pdf_num(0.12345), "0.123");
        assert_eq!(pdf_text("Aé"), "<FEFF004100E9>");
        assert_eq!(rgba_to_rgb(&[10, 20, 30, 255, 0, 0, 0, 0, 0, 0, 0, 128]), vec![10, 20, 30, 255, 255, 255, 127, 127, 127]);
    }
}
