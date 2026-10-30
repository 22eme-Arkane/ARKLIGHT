//! Export PDF du plan : structure du fichier (en-tête, objets, table des renvois, fin) et image
//! (taille, pixels) relues octet par octet.

use arklight_core::Project;
use arklight_plot::*;

fn project(name: &str) -> Project {
    Project::from_json(&std::fs::read_to_string(format!("{}/../arklight-core/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|i| i + from)
}

fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

/// Nombre écrit après `key` dans `dict` (« /Width 2481 »).
fn entry(dict: &str, key: &str) -> usize {
    let at = dict.find(key).unwrap_or_else(|| panic!("{key} absent de {dict}")) + key.len();
    dict[at..].split_whitespace().next().unwrap().trim_end_matches(">>").parse().unwrap()
}

/// Dictionnaire et flux d'un objet.
fn object(pdf: &[u8], offset: usize, n: usize) -> (String, Option<Vec<u8>>) {
    let head = format!("{n} 0 obj\n");
    assert_eq!(&pdf[offset..offset + head.len()], head.as_bytes(), "objet {n} à l'adresse de la table");
    let start = offset + head.len();
    let end = find(pdf, b"endobj\n", start).unwrap();
    match find(pdf, b"stream\n", start).filter(|s| *s < end) {
        Some(s) => {
            let dict = String::from_utf8(pdf[start..s].to_vec()).unwrap();
            let len = entry(&dict, "/Length ");
            let data = pdf[s + 7..s + 7 + len].to_vec();
            assert_eq!(&pdf[s + 7 + len..s + 7 + len + 11], b"\nendstream\n", "objet {n} : longueur du flux");
            (dict, Some(data))
        }
        None => (String::from_utf8(pdf[start..end].to_vec()).unwrap(), None),
    }
}

/// Vérifie la structure du PDF (page `media_box`) ; renvoie la largeur, la hauteur et les pixels
/// RVB de l'image, et les informations du document.
fn read_pdf(pdf: &[u8], media_box: &str) -> (usize, usize, Vec<u8>, String) {
    assert!(pdf.starts_with(b"%PDF-1.4\n%"), "en-tête");
    assert!(pdf.ends_with(b"%%EOF\n"), "fin");
    let sx = rfind(pdf, b"startxref\n").unwrap();
    let xref: usize = std::str::from_utf8(&pdf[sx + 10..]).unwrap().lines().next().unwrap().parse().unwrap();
    assert!(pdf[xref..].starts_with(b"xref\n0 7\n"), "table des renvois à l'adresse annoncée");
    let table = &pdf[xref + 9..];
    assert_eq!(&table[..20], b"0000000000 65535 f \n");
    let offsets: Vec<usize> = (1..7)
        .map(|i| {
            let e = &table[i * 20..i * 20 + 20];
            assert_eq!(&e[10..], b" 00000 n \n", "entrée {i} de 20 octets");
            std::str::from_utf8(&e[..10]).unwrap().parse().unwrap()
        })
        .collect();
    let trailer = std::str::from_utf8(&table[7 * 20..]).unwrap();
    assert!(trailer.starts_with("trailer\n<< /Size 7 /Root 1 0 R /Info 6 0 R >>\nstartxref\n"), "{trailer}");
    let objs: Vec<(String, Option<Vec<u8>>)> = offsets.iter().enumerate().map(|(i, o)| object(pdf, *o, i + 1)).collect();
    assert!(objs[0].0.contains("/Type /Catalog /Pages 2 0 R"));
    assert!(objs[1].0.contains("/Kids [3 0 R] /Count 1"));
    assert!(objs[2].0.contains(&format!("/MediaBox [{media_box}]")) && objs[2].0.contains("/XObject << /Im1 4 0 R >>") && objs[2].0.contains("/Contents 5 0 R"));
    let (dict, data) = &objs[3];
    assert!(dict.contains("/Subtype /Image") && dict.contains("/ColorSpace /DeviceRGB") && dict.contains("/BitsPerComponent 8") && dict.contains("/Filter /FlateDecode"));
    let (w, h) = (entry(dict, "/Width "), entry(dict, "/Height "));
    let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(data.as_ref().unwrap()).unwrap();
    assert_eq!(rgb.len(), w * h * 3, "image décompressée : {w} × {h} pixels RVB");
    let content = String::from_utf8(objs[4].1.clone().unwrap()).unwrap();
    assert!(content.contains("/Im1 Do"), "{content}");
    (w, h, rgb, objs[5].0.clone())
}

#[test]
fn plan_en_pdf() {
    let p = project("demo-night.json");
    let opts = PlotSvgOptions { options: PlotOptions::show_all(), date: "27/10/2026".into(), ..PlotSvgOptions::default() };
    let t = std::time::Instant::now();
    let pdf = plot_pdf(&p, &opts);
    let elapsed = t.elapsed();
    std::fs::write(format!("{}/night.pdf", env!("CARGO_TARGET_TMPDIR")), &pdf).unwrap();
    let (w, h, rgb, info) = read_pdf(&pdf, "0 0 841.89 595.28");
    // A4 à 300 ppp, aux proportions de l'export SVG (1754 × 1240)
    assert_eq!((w, h), (3508, 2480));
    let px = rgb.chunks(3);
    let white = px.clone().filter(|c| *c == [255, 255, 255]).count();
    let dark = px.filter(|c| c.iter().map(|v| u32::from(*v)).sum::<u32>() < 384).count();
    assert!(white > w * h / 2, "fond blanc : {white} pixels blancs");
    assert!(dark > 5000, "traits et textes : {dark} pixels sombres");
    // titre en UTF-16 : « Plan de feu — <production du cartouche> »
    let title: String = "Plan de feu — Projet d’exemple ARKLIGHT".encode_utf16().map(|u| format!("{u:04X}")).collect();
    assert!(info.contains(&format!("/Title <FEFF{title}>")), "{info}");
    assert!(pdf.len() < 4_000_000, "image compressée : {} octets ({elapsed:?})", pdf.len());
    eprintln!("PDF : {} octets en {elapsed:?}", pdf.len());
}

#[test]
fn pdf_d_une_image() {
    // 3 × 2 pixels sur une page de 300 × 100 points : agrandie à 150 × 100, centrée
    let rgb: Vec<u8> = (0..18).collect();
    let pdf = image_pdf(&rgb, 3, 2, (300.0, 100.0), "Essai « é »");
    let (w, h, back, info) = read_pdf(&pdf, "0 0 300 100");
    assert_eq!((w, h), (3, 2));
    assert_eq!(back, rgb);
    assert!(info.contains("/Title <FEFF00450073007300610069002000AB002000E9002000BB>"), "{info}");
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("q\n150 0 0 100 75 0 cm\n/Im1 Do\nQ\n"), "{text}");
}
