//! Exports du plan (portage de `exportSvg` et des noms de fichiers de `web/js/plot/plot-view.js`) :
//! document SVG complet (fond blanc ou couleur choisie, cadré sur la scène, avec cartouche),
//! cadrage « Tout voir », séquence et plan du projet, nom de fichier.

use crate::js::{max2, max_of, min2, min_of, strip_accents, string_of};
use crate::render::{BY_HEIGHT, PlotData, PlotOptions, RenderOptions, ThemeName, TitleInfo, Tr, View, Viewport, render_with, title_block};
use arklight_core::project::Project;
use serde_json::Value;

/// `computeFit` : centre et échelle qui montrent toute la scène dans un cadre `w` × `h` (px).
pub fn compute_fit(scene: &Project, w: f64, h: f64) -> View {
    let mut xs: Vec<f64> = Vec::new();
    let mut zs: Vec<f64> = Vec::new();
    for f in &scene.fixtures {
        xs.push(f.position[0]);
        zs.push(f.position[2]);
    }
    for o in &scene.objects {
        xs.push(o.position[0]);
        zs.push(o.position[2]);
    }
    for it in PlotData::of(scene).items {
        for [x, z] in it.points {
            xs.push(x);
            zs.push(z);
        }
    }
    if xs.is_empty() {
        xs = vec![-5.0, 5.0];
        zs = vec![-7.0, 4.0];
    }
    let min_x = min_of(xs.iter().copied()) - 1.5;
    let max_x = max_of(xs.iter().copied()) + 1.5;
    let min_z = min_of(zs.iter().copied()) - 1.5;
    let max_z = max_of(zs.iter().copied()) + 1.5;
    let s = max2(8.0, min2(200.0, min2(w / (max_x - min_x), (h - 60.0) / (max_z - min_z))));
    View { cx: (min_x + max_x) / 2.0, cz: (min_z + max_z) / 2.0, s, w, h }
}

/// Séquence et plan en cours (« Séquence 1 › Plan 2 ») d'un projet à plusieurs plans ; vide sinon.
pub fn plan_text(project: &Project) -> String {
    let seqs = project.extra.get("sequences").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    let flat: Vec<(&Value, &Value)> = seqs.iter().flat_map(|s| s.get("plans").and_then(Value::as_array).into_iter().flatten().map(move |p| (s, p))).collect();
    if flat.len() <= 1 {
        return String::new();
    }
    let current = project.extra.get("current").and_then(|c| c.get("plan"));
    match flat.iter().find(|(_, p)| current.is_some() && p.get("id") == current) {
        Some((s, p)) => format!("{} › {}", s.get("name").map(string_of).unwrap_or_default(), p.get("name").map(string_of).unwrap_or_default()),
        None => String::new(),
    }
}

/// Nom de fichier tiré du projet et du plan (« demo-clip-couplet-salon-plan-1-large ») : à mettre
/// dans « plan-de-feu-{0}.svg », « liste-materiel-{0}.csv », « patch-{0}.csv », « {0}.mvr ».
pub fn slug(project: &Project) -> String {
    let name = if project.name.is_empty() { "plan" } else { &project.name };
    let text = strip_accents(&format!("{name} {}", plan_text(project))).to_lowercase();
    let mut out = String::new();
    let mut gap = false;
    for c in text.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap {
                out.push('-');
                gap = false;
            }
            out.push(c);
        } else {
            gap = true;
        }
    }
    if gap {
        out.push('-');
    }
    let s = out.strip_prefix('-').unwrap_or(&out);
    let s = s.strip_suffix('-').unwrap_or(s);
    if s.is_empty() { "plan".into() } else { s.to_string() }
}

/// Document SVG autour d'un contenu (`renderPlot`) de `w` × `h` px.
pub fn svg_document(body: &str, w: f64, h: f64) -> String {
    let (w, h) = (crate::js::num(w), crate::js::num(h));
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="Segoe UI, Arial, sans-serif">{body}</svg>"#)
}

/// Réglages de l'export SVG (« Plan en SVG », aussi la base des exports PNG et impression).
pub struct PlotSvgOptions<'a> {
    /// ce qui s'affiche (« Affichage »)
    pub options: PlotOptions,
    /// date du cartouche, déjà écrite (« 27/10/2026 »)
    pub date: String,
    /// taille de la page (px) : A4 paysage à 150 ppp par défaut (1754 × 1240)
    pub width: f64,
    pub height: f64,
    /// pieds et teinte des faisceaux (par défaut : pieds recalculés d'après la hauteur)
    pub viewport: Option<&'a dyn Viewport>,
    /// image de fond intégrée au fichier (data URL, voir [`data_url`]) à la place de son adresse
    pub underlay_href: Option<String>,
    pub tr: Option<Tr<'a>>,
}

impl Default for PlotSvgOptions<'_> {
    fn default() -> Self {
        Self { options: PlotOptions::default(), date: String::new(), width: 1754.0, height: 1240.0, viewport: Some(&BY_HEIGHT), underlay_href: None, tr: None }
    }
}

/// `exportSvg` : le plan d'un projet en document SVG complet (fond blanc ou couleur choisie, cadré
/// sur la scène, avec cartouche).
pub fn plot_svg(project: &Project, opts: &PlotSvgOptions) -> String {
    let (w, h) = (opts.width, opts.height);
    let fitted = compute_fit(project, w, h);
    let view = View { s: fitted.s * 0.92, ..fitted };
    let mut plot = PlotData::of(project);
    if let (Some(u), Some(href)) = (plot.underlay.as_mut(), opts.underlay_href.as_ref()) {
        u.url = href.clone();
    }
    let ro = RenderOptions { theme: ThemeName::Print, options: opts.options.clone(), viewport: opts.viewport, tr: opts.tr, ..RenderOptions::default() };
    let body = render_with(project, &plot, &view, &ro);
    let plan = plan_text(project);
    let title = TitleInfo { scene: Some(project.name.clone()), plan: (!plan.is_empty()).then_some(plan), ..plot.title.clone() };
    let tb = title_block(w, h, &title, ThemeName::Print, &opts.date, plot.background.as_deref(), opts.tr);
    svg_document(&format!("{body}{tb}"), w, h)
}

/// Fichier en data URL (« data:image/png;base64,… »), pour intégrer l'image de fond à l'export.
pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len() * 4 / 3 + 32);
    s.push_str("data:");
    s.push_str(mime);
    s.push_str(";base64,");
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                s.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_url_en_base64() {
        assert_eq!(data_url("text/plain", b"Man"), "data:text/plain;base64,TWFu");
        assert_eq!(data_url("text/plain", b"Ma"), "data:text/plain;base64,TWE=");
        assert_eq!(data_url("text/plain", b"M"), "data:text/plain;base64,TQ==");
    }
}
