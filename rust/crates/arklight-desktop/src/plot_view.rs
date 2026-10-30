//! Plan de feu 2D (« Plan 2D » à la place de la vue 3D, touche P) : dessin du module
//! `arklight-plot` (SVG rendu en image par resvg), choix et déplacement à la souris, zoom, et les
//! exports du plan (SVG, PNG, liste du matériel, patch, MVR), comme `web/js/plot/plot-view.js`.

use crate::{State, i18n, tr};
use arklight_core::document::Edit;
use arklight_core::engine::Lights;
use arklight_plot::{
    Ctx as EdCtx, EditOp, Editor, ItemChange, Key as EdKey, Modifiers, Tool, apply_op, edit_item, kind_name, plot_item, plot_pdf,
};
use arklight_plot::{
    MvrOptions, PlotOptions, PlotSvgOptions, RenderOptions, Selection, Stamp, ThemeName, View, Viewport, build_equipment_list, build_mvr, build_patch, compute_fit, equipment_csv, patch_csv, plot_svg, rasterize, render_plot, slug, svg_to_png, svg_document,
};
use std::time::Instant;

/// Réglages d'affichage du plan, dans l'ordre de `PLOT_FLAGS` (menu « Plan »).
pub const PLOT_FLAGS: [&str; 20] = [
    "names", "fixtures", "addresses", "supports", "beams", "grid", "underlay", "stands", "grip", "sandbags", "studio", "annotations", "people", "animals", "vehicles", "furniture", "set", "outdoor", "props", "filmset",
];

fn flag<'a>(o: &'a mut PlotOptions, key: &str) -> Option<&'a mut bool> {
    Some(match key {
        "names" => &mut o.names,
        "fixtures" => &mut o.fixtures,
        "addresses" => &mut o.addresses,
        "supports" => &mut o.supports,
        "beams" => &mut o.beams,
        "grid" => &mut o.grid,
        "underlay" => &mut o.underlay,
        "stands" => &mut o.stands,
        "grip" => &mut o.grip,
        "sandbags" => &mut o.sandbags,
        "studio" => &mut o.studio,
        "annotations" => &mut o.annotations,
        "people" => &mut o.people,
        "animals" => &mut o.animals,
        "vehicles" => &mut o.vehicles,
        "furniture" => &mut o.furniture,
        "set" => &mut o.set,
        "outdoor" => &mut o.outdoor,
        "props" => &mut o.props,
        "filmset" => &mut o.filmset,
        _ => return None,
    })
}

/// État de la vue du plan.
pub struct Plot {
    pub on: bool,
    pub view: View,
    pub options: PlotOptions,
    /// cadré une première fois sur la scène
    pub fitted: bool,
    pub drag: Option<PlotDrag>,
    /// image à refaire
    pub dirty: bool,
    /// dernière image (la lumière change souvent : 5 images par seconde au plus pour les faisceaux)
    pub drawn: Instant,
    /// taille de la dernière image (pixels de l'écran)
    pub size: (u32, u32),
    /// outils de dessin, poignées, cadre de sélection (`arklight_plot::Editor`)
    pub editor: Editor,
    /// élément du plan choisi (forme, texte, flèche…)
    pub plot_id: Option<String>,
    /// bouton gauche enfoncé sur le plan (geste de l'éditeur), et ses touches de modification
    pub down: Option<Modifiers>,
    /// image de fond lue sur le disque : (adresse « /assets/… », image en data URL)
    underlay: Option<(String, String)>,
}

impl Default for Plot {
    fn default() -> Self {
        Plot {
            on: false,
            view: View { cx: 0.0, cz: 0.0, s: 40.0, w: 800.0, h: 600.0 },
            options: PlotOptions::default(),
            fitted: false,
            drag: None,
            dirty: true,
            drawn: Instant::now(),
            size: (0, 0),
            editor: Editor::new(),
            plot_id: None,
            down: None,
            underlay: None,
        }
    }
}

/// Geste en cours sur le plan.
pub enum PlotDrag {
    /// se déplacer dans le plan (clic droit ou molette enfoncée)
    Pan { x: f32, y: f32 },
}

/// Sortie des projecteurs (teinte des faisceaux sur le plan).
struct Live<'a>(&'a Lights);

impl Viewport for Live<'_> {
    fn output(&self, id: &str) -> Option<Vec<f64>> {
        self.0.get(id).map(|l| l.pixels.clone())
    }
}

fn translate(s: &str) -> String {
    i18n::t(s)
}

/// Valeurs des réglages d'affichage (menu « Plan »).
pub fn flags(o: &PlotOptions) -> Vec<bool> {
    let mut o = o.clone();
    PLOT_FLAGS.iter().map(|k| flag(&mut o, k).is_some_and(|v| *v)).collect()
}

/// Inverse un réglage d'affichage (« all » / « equipment » : préréglages).
pub fn toggle_flag(p: &mut Plot, key: &str) {
    match key {
        "all" => p.options = PlotOptions::show_all(),
        "equipment" => p.options = PlotOptions::equipment_only(),
        // le plan sans aucune inscription (noms, numéros, adresses DMX, pieds et hauteurs)
        "no-text" => {
            let o = &mut p.options;
            (o.names, o.fixtures, o.addresses, o.supports) = (false, false, false, false);
        }
        // seulement les projecteurs : ni texte, ni grille, ni faisceaux, ni décor
        "none" => {
            for k in PLOT_FLAGS {
                if let Some(v) = flag(&mut p.options, k) {
                    *v = false;
                }
            }
        }
        k => {
            if let Some(v) = flag(&mut p.options, k) {
                *v = !*v;
            }
        }
    }
    p.dirty = true;
}

/// Point du plan (mètres) sous la souris (pixels de la vue).
pub fn to_world(v: &View, x: f32, y: f32) -> [f64; 2] {
    [v.cx + (x as f64 - v.w / 2.0) / v.s, v.cz + (y as f64 - v.h / 2.0) / v.s]
}

/// Cadre le plan sur la scène (« Tout voir »).
pub fn fit(st: &mut State) {
    let (w, h) = (st.view.0 as f64, st.view.1 as f64);
    st.plot.view = compute_fit(&st.doc.project, w.max(1.0), h.max(1.0));
    st.plot.fitted = true;
    st.plot.dirty = true;
}

/// Image du plan à la taille de la vue (`w` × `h` pixels logiques, `scale` : pixels de l'écran).
pub fn image(st: &mut State, w: f32, h: f32, scale: f32) -> Option<slint::Image> {
    let p = &mut st.plot;
    p.view.w = w as f64;
    p.view.h = h as f64;
    let sel = Selection {
        fixtures: st.selection.iter().filter(|id| st.doc.fixture(id).is_some()).cloned().collect(),
        object_id: st.selection.iter().find(|id| st.doc.object(id).is_some()).cloned(),
        plot_id: p.plot_id.clone(),
    };
    let live = Live(&st.lights);
    let handles = p.editor.handles(&EdCtx { project: &st.doc.project, view: p.view, options: &p.options, sel: &sel });
    let underlay_href = underlay_href(&mut p.underlay, &st.doc.project);
    let opts = RenderOptions { theme: ThemeName::Screen, options: p.options.clone(), sel, preview: p.editor.preview(), handles, viewport: Some(&live), tr: Some(&translate), underlay_href, ..Default::default() };
    let mut body = render_plot(&st.doc.project, &p.view, &opts);
    if let Some(m) = p.editor.marquee_svg(&p.view) {
        body.push_str(&m);
    }
    let svg = svg_document(&body, w as f64, h as f64);
    let (pw, ph) = ((w * scale).round().max(1.0) as u32, (h * scale).round().max(1.0) as u32);
    let rgba = rasterize(&svg, pw, ph);
    if rgba.len() != (pw * ph * 4) as usize {
        return None;
    }
    p.size = (pw, ph);
    p.drawn = Instant::now();
    p.dirty = false;
    let buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&rgba, pw, ph);
    Some(slint::Image::from_rgba8(buf))
}

/// Zoom à la molette, le point sous la souris reste en place.
pub fn wheel(st: &mut State, dy: f32, x: f32, y: f32) {
    let v = &mut st.plot.view;
    let before = to_world(v, x, y);
    v.s = (v.s * if dy > 0.0 { 1.15 } else { 1.0 / 1.15 }).clamp(4.0, 400.0);
    let after = to_world(v, x, y);
    v.cx += before[0] - after[0];
    v.cz += before[1] - after[1];
    st.plot.dirty = true;
}

// ------------------------------------------------------------------ exports

fn save_dialog(name: &str, filter: &str, ext: &str) -> Option<std::path::PathBuf> {
    rfd::FileDialog::new().set_title("ARKLIGHT").add_filter(filter, &[ext]).set_file_name(name).save_file()
}

/// « Imprimer / PDF (plan + liste + patch) » (`printSheet`) : une page HTML (plan en SVG, liste du
/// matériel, patch DMX, A4 paysage) ouverte dans le navigateur, qui lance l'impression ; « Microsoft
/// Print to PDF » en fait un PDF. Message pour la barre d'état.
pub fn print_sheet(st: &State) -> String {
    let project = &st.doc.project;
    let live = Live(&st.lights);
    let href = underlay_href(&mut None, project);
    // l'image de fond est lue par le navigateur depuis le dossier des images
    let href = href.map(|h| match h.strip_prefix(arklight_plot::ASSET_PREFIX) {
        Some(name) => format!("file:///{}", assets_dir().join(name).display().to_string().replace('\\', "/")),
        None => h,
    });
    let opts = PlotSvgOptions { options: st.plot.options.clone(), date: Stamp::now_utc().date(), viewport: Some(&live), underlay_href: href, tr: Some(&translate), ..Default::default() };
    let svg = plot_svg(project, &opts);
    let list = build_equipment_list(project, Some(&live));
    let patch = build_patch(project);
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let production = project.extra.get("plot").and_then(|p| p.get("title")).and_then(|t| t.get("production")).and_then(serde_json::Value::as_str).unwrap_or("");
    let title = [if production.is_empty() { project.name.as_str() } else { production }, &st.doc.current_plan_label()].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(" — ");
    let t = |s: &str| esc(&translate(s));
    let mut html = format!(
        "<!doctype html><html lang=\"{lang}\"><head><meta charset=\"utf-8\"><title>{plot} — {title}</title>\n<style>\n  body {{ font: 12px \"Segoe UI\", Arial, sans-serif; color: #111; margin: 0; }}\n  .page {{ page-break-after: always; padding: 10mm; }}\n  .page:last-child {{ page-break-after: auto; }}\n  svg {{ width: 100%; height: auto; border: 1px solid #ccc; }}\n  h1 {{ font-size: 18px; margin: 0 0 4mm; }} h2 {{ font-size: 14px; margin: 6mm 0 2mm; }}\n  table {{ border-collapse: collapse; width: 100%; }} th, td {{ border: 1px solid #bbb; padding: 3px 6px; text-align: left; }}\n  th {{ background: #f0f0f0; }} td.q {{ text-align: right; width: 18mm; font-weight: 700; }}\n  @page {{ size: A4 landscape; margin: 8mm; }}\n</style></head><body>\n<div class=\"page\">{svg}</div>\n<div class=\"page\"><h1>{list_title} — {title}</h1>\n",
        lang = tr("fr", "en"),
        plot = t("Plan de feu"),
        title = esc(&title),
        list_title = t("Liste du matériel"),
    );
    for s in &list.sections {
        html += &format!("<h2>{}</h2><table><tr><th>{}</th><th>{}</th><th>{}</th></tr>", t(&s.title), t("Désignation"), t("Qté"), t("Remarques"));
        for r in &s.rows {
            html += &format!("<tr><td>{}</td><td class=\"q\">{}</td><td>{}</td></tr>", t(&r.name), r.qty, if r.note.is_empty() { String::new() } else { t(&r.note) });
        }
        html += "</table>";
    }
    html += &format!("\n<h2>{}</h2><table><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr>", t("Patch DMX"), t("Univers"), t("Adresses"), t("Projecteur"), t("Modèle"), t("Mode"));
    for p in &patch {
        html += &format!("<tr><td>{}</td><td>{}–{}</td><td>{}</td><td>{}</td><td>{}</td></tr>", p.universe, p.address, p.address as i64 + p.footprint as i64 - 1, esc(&p.name), esc(&p.model), esc(&p.mode));
    }
    html += "</table>\n</div>\n<script>window.addEventListener('load', () => setTimeout(() => window.print(), 400));</script>\n</body></html>";
    let dir = std::env::temp_dir().join("ARKLIGHT");
    let path = dir.join(format!("{}-impression.html", slug(project)));
    match std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, html)) {
        Ok(()) => {
            // (essais automatiques : la feuille est écrite, pas ouverte)
            if std::env::var_os("ARKLIGHT_SMOKE").is_some() {
                eprintln!("feuille : {}", path.display());
            } else {
                let _ = std::process::Command::new("explorer").arg(&path).spawn();
            }
            tr("Feuille ouverte dans le navigateur : impression, ou « Microsoft Print to PDF » pour un PDF", "Sheet opened in the browser: print it, or use “Microsoft Print to PDF” for a PDF").into()
        }
        Err(e) => format!("{} {e}", tr("Échec de l’enregistrement :", "Could not save:")),
    }
}

/// Export demandé (« svg », « png », « equipment », « patch », « mvr ») ; message pour la barre d'état.
pub fn export(st: &State, what: &str, version: &str) -> Option<String> {
    let project = &st.doc.project;
    let live = Live(&st.lights);
    let base = slug(project);
    let href = underlay_href(&mut None, project);
    let svg = || {
        let opts = PlotSvgOptions { options: st.plot.options.clone(), date: Stamp::now_utc().date(), viewport: Some(&live), underlay_href: href.clone(), tr: Some(&translate), ..Default::default() };
        plot_svg(project, &opts)
    };
    let (name, filter, ext, bytes): (String, &str, &str, Vec<u8>) = match what {
        "svg" => (format!("{base}-plan.svg"), "SVG", "svg", svg().into_bytes()),
        "png" => {
            let png = svg_to_png(&svg(), 3508, 2480)?;
            (format!("{base}-plan.png"), "PNG", "png", png)
        }
        "equipment" => {
            let list = build_equipment_list(project, Some(&live));
            (format!("{base}-materiel.csv"), "CSV", "csv", equipment_csv(&list, Some(&translate)).into_bytes())
        }
        "patch" => (format!("{base}-patch.csv"), "CSV", "csv", patch_csv(&build_patch(project), Some(&translate)).into_bytes()),
        "pdf" => {
            let opts = PlotSvgOptions { options: st.plot.options.clone(), date: Stamp::now_utc().date(), viewport: Some(&live), underlay_href: href.clone(), tr: Some(&translate), ..Default::default() };
            (format!("{base}-plan.pdf"), "PDF", "pdf", plot_pdf(project, &opts))
        }
        "mvr" => {
            let opts = MvrOptions { provider: "ARKLIGHT".into(), provider_version: version.into(), layer_name: i18n::t("Projecteurs"), time: Stamp::now_utc() };
            let mvr = build_mvr(project, &opts);
            if mvr.fixtures == 0 {
                return Some(tr("Aucun projecteur à exporter", "No fixture to export").into());
            }
            (format!("{base}.mvr"), "MVR", "mvr", mvr.bytes)
        }
        _ => return None,
    };
    let path = save_dialog(&name, filter, ext)?;
    Some(match std::fs::write(&path, bytes) {
        Ok(()) => format!("{} {}", tr("Enregistré :", "Saved:"), path.display()),
        Err(e) => format!("{} {e}", tr("Échec de l’enregistrement :", "Could not save:")),
    })
}

// ------------------------------------------------------------------ outils de dessin (éditeur du plan)

/// Sélection de l'application vue par l'éditeur.
fn selection(st: &State) -> Selection {
    Selection {
        fixtures: st.selection.iter().filter(|id| st.doc.fixture(id).is_some()).cloned().collect(),
        object_id: st.selection.iter().find(|id| st.doc.object(id).is_some()).cloned(),
        plot_id: st.plot.plot_id.clone(),
    }
}

/// Appelle l'éditeur avec le contexte du projet, puis applique ce qu'il demande.
fn with_editor(st: &mut State, f: impl FnOnce(&mut Editor, &EdCtx) -> Vec<EditOp>) -> bool {
    let sel = selection(st);
    let mut editor = std::mem::take(&mut st.plot.editor);
    let ops = {
        let ctx = EdCtx { project: &st.doc.project, view: st.plot.view, options: &st.plot.options, sel: &sel };
        f(&mut editor, &ctx)
    };
    st.plot.editor = editor;
    st.plot.dirty = true;
    apply_ops(st, ops)
}

/// Applique les opérations de l'éditeur (sélection, projecteurs et objets, éléments du plan) ; vrai
/// si le projet ou la sélection a changé.
pub fn apply_ops(st: &mut State, ops: Vec<EditOp>) -> bool {
    let mut changed = false;
    for op in ops {
        match op {
            EditOp::Select(s) => {
                let mut ids = s.fixtures;
                ids.extend(s.object_id);
                st.selection = ids;
                st.plot.plot_id = s.plot_id;
            }
            EditOp::Doc(e) => {
                st.doc.apply(e);
            }
            op => {
                if let Some(plot) = apply_op(st.doc.project.extra.get("plot"), &op) {
                    st.doc.apply(Edit::Plot { plot, key: op.history_key(), label: op.label() });
                }
            }
        }
        changed = true;
    }
    changed
}

/// Bouton gauche enfoncé, déplacé, relâché sur le plan (point en mètres).
pub fn down(st: &mut State, p: [f64; 2], m: Modifiers) -> bool {
    st.plot.down = Some(m);
    with_editor(st, |e, c| e.pointer_down(c, p, m))
}

pub fn moved(st: &mut State, p: [f64; 2]) -> bool {
    let Some(m) = st.plot.down else { return false };
    with_editor(st, |e, c| e.pointer_move(c, p, m))
}

pub fn up(st: &mut State, p: [f64; 2]) -> bool {
    if st.plot.down.take().is_none() {
        return false;
    }
    with_editor(st, |e, c| e.pointer_up(c, p))
}

pub fn double_click(st: &mut State) -> bool {
    with_editor(st, |e, c| e.double_click(c))
}

/// Clic droit : l'élément sous la souris est choisi (le menu de la sélection s'ouvre ensuite).
pub fn context(st: &mut State, p: [f64; 2]) -> bool {
    with_editor(st, |e, c| e.context_menu(c, p).1)
}

/// Échap, Entrée, Suppr pour l'éditeur ; vrai s'il s'en est servi.
pub fn key(st: &mut State, key: &str) -> bool {
    let k = match key {
        "Escape" => EdKey::Escape,
        "Enter" => EdKey::Enter,
        "Delete" | "Backspace" => EdKey::Delete,
        _ => return false,
    };
    let sel = selection(st);
    let mut editor = std::mem::take(&mut st.plot.editor);
    let ops = {
        let ctx = EdCtx { project: &st.doc.project, view: st.plot.view, options: &st.plot.options, sel: &sel };
        editor.key(&ctx, k)
    };
    st.plot.editor = editor;
    match ops {
        Some(ops) => {
            st.plot.dirty = true;
            apply_ops(st, ops);
            true
        }
        None => false,
    }
}

/// Outil choisi (rang de `Tool::ALL`) ; les annotations s'affichent si elles étaient masquées.
pub fn set_tool(st: &mut State, index: usize) -> bool {
    let Some(tool) = Tool::ALL.get(index).copied() else { return false };
    let shown = st.plot.editor.set_tool(tool, &mut st.plot.options);
    st.plot.dirty = true;
    shown
}

pub fn tool_index(st: &State) -> usize {
    Tool::ALL.iter().position(|t| *t == st.plot.editor.tool()).unwrap_or(0)
}

/// Élément du plan choisi (barre de réglages, `renderProps`).
pub struct ItemInfo {
    /// nature affichée (« Rectangle », « Texte »…)
    pub name: String,
    pub text: String,
    /// texte libre (taille du texte au lieu du trait)
    pub is_text: bool,
    /// forme fermée : se remplit, peut devenir un volume 3D
    pub closed: bool,
    pub filled: bool,
    pub dash: bool,
    /// épaisseur du trait, taille du texte (px), hauteur du volume 3D (m)
    pub width: f64,
    pub size: f64,
    pub height: f64,
}

/// Épaisseurs de trait et tailles de texte proposées.
pub const WIDTHS: [f64; 5] = [1.0, 2.0, 3.0, 5.0, 8.0];
pub const SIZES: [f64; 6] = [12.0, 16.0, 20.0, 28.0, 40.0, 60.0];

pub fn item_info(st: &State) -> Option<ItemInfo> {
    use serde_json::Value;
    let id = st.plot.plot_id.as_deref()?;
    let it = plot_item(&st.doc.project, id)?;
    let kind = it.get("kind").and_then(Value::as_str).unwrap_or("");
    let style = it.get("style").cloned().unwrap_or(Value::Null);
    // remplie : une couleur (ou vrai), pas « none »
    let filled = match style.get("fill") {
        Some(Value::String(s)) => !s.is_empty() && s != "none",
        Some(Value::Bool(b)) => *b,
        _ => false,
    };
    Some(ItemInfo {
        name: i18n::t(kind_name(kind).unwrap_or(kind)),
        text: it.get("text").and_then(Value::as_str).unwrap_or("").to_owned(),
        is_text: kind == "text",
        closed: arklight_plot::is_closed(kind),
        filled,
        dash: style.get("dash").and_then(Value::as_bool).unwrap_or(false),
        width: style.get("width").and_then(Value::as_f64).unwrap_or(2.0),
        size: style.get("size").and_then(Value::as_f64).unwrap_or(16.0),
        height: it.get("height").and_then(Value::as_f64).unwrap_or(0.0),
    })
}

/// Fond et cartouche du plan (fenêtre « Fond et cartouche »).
pub struct MetaInfo {
    /// couleur du fond (vide : automatique)
    pub background: String,
    /// image de fond : (largeur m, rotation °, opacité)
    pub underlay: Option<(f64, f64, f64)>,
    /// image de fond aussi au sol de la vue 3D
    pub floor3d: bool,
    pub production: String,
    pub author: String,
}

pub fn meta_info(st: &State) -> MetaInfo {
    use serde_json::Value;
    let plot = st.doc.project.extra.get("plot").cloned().unwrap_or(Value::Null);
    let u = plot.get("underlay").filter(|u| u.is_object());
    let title = plot.get("title").cloned().unwrap_or(Value::Null);
    MetaInfo {
        background: plot.get("background").and_then(Value::as_str).unwrap_or("").to_owned(),
        underlay: u.map(|u| (u["width"].as_f64().unwrap_or(20.0), u["rotation"].as_f64().unwrap_or(0.0), u["opacity"].as_f64().unwrap_or(0.6))),
        floor3d: u.is_some_and(|u| u["floor3d"].as_bool() == Some(true)),
        production: title.get("production").and_then(Value::as_str).unwrap_or("").to_owned(),
        author: title.get("author").and_then(Value::as_str).unwrap_or("").to_owned(),
    }
}

/// Réglage du fond et du cartouche : « bg » (couleur, vide : auto), « u-width », « u-rotation »,
/// « u-opacity », « u-center » (image de fond), « production », « author ».
pub fn meta_change(st: &mut State, key: &str, value: &str) -> bool {
    use serde_json::json;
    let plot = st.doc.project.extra.get("plot");
    let num = || value.trim().replace(',', ".").parse::<f64>().ok();
    let op = match key {
        "bg" => Some(arklight_plot::background_change((!value.is_empty()).then_some(value))),
        "u-width" => num().filter(|w| *w >= 0.5).and_then(|w| arklight_plot::underlay_change(plot, &json!({ "width": w }))),
        "u-rotation" => num().and_then(|r| arklight_plot::underlay_change(plot, &json!({ "rotation": r }))),
        "u-opacity" => num().and_then(|o| arklight_plot::underlay_change(plot, &json!({ "opacity": (o.clamp(0.05, 1.0) * 100.0).round() / 100.0 }))),
        "u-center" => arklight_plot::underlay_change(plot, &json!({ "x": st.plot.view.cx, "z": st.plot.view.cz })),
        "u-floor" => arklight_plot::underlay_change(plot, &json!({ "floor3d": value == "1" })),
        "production" | "author" => Some(arklight_plot::title_change(plot, key, value.trim())),
        _ => None,
    };
    let Some(op) = op else { return false };
    st.plot.dirty = true;
    apply_ops(st, vec![op])
}

/// Réglage de l'élément choisi : « text », « color », « filled », « dash », « width », « size »,
/// « height », « delete », « duplicate ».
pub fn item_change(st: &mut State, what: &str, arg: &str) -> bool {
    let Some(id) = st.plot.plot_id.clone() else { return false };
    match what {
        "delete" => with_editor(st, |e, c| e.remove(c, &id)),
        "duplicate" => with_editor(st, |e, c| e.duplicate(c, &id)),
        _ => {
            let change = match what {
                "text" => ItemChange::Text(arg.to_owned()),
                "color" => ItemChange::Color(arg.to_owned()),
                "filled" => ItemChange::Filled(arg == "1"),
                "dash" => ItemChange::Dash(arg == "1"),
                "width" | "size" | "height" => {
                    let Ok(v) = arg.trim().replace(',', ".").parse::<f64>() else { return false };
                    match what {
                        "width" => ItemChange::Width(v),
                        "size" => ItemChange::Size(v),
                        _ => ItemChange::Height(v.clamp(0.0, 100.0)),
                    }
                }
                _ => return false,
            };
            let Some(op) = plot_item(&st.doc.project, &id).and_then(|it| edit_item(it, &change)) else { return false };
            st.plot.dirty = true;
            apply_ops(st, vec![op])
        }
    }
}

/// Message d'aide de l'outil (traduit).
pub fn hint(st: &State) -> String {
    // (le symbole ⟳ de la poignée manque aux polices de l'interface)
    i18n::t(st.plot.editor.hint()).replace(" ⟳", "")
}

// ------------------------------------------------------------------ image de fond (photo, plan scanné)

/// Dossier des images de fond : celui de la version JavaScript (`scenes\assets`), adresses « /assets/… ».
fn assets_dir() -> std::path::PathBuf {
    crate::projects::projects_dir().join("assets")
}

/// Image de fond du projet en data URL (relue seulement quand elle change).
fn underlay_href(cache: &mut Option<(String, String)>, project: &arklight_core::Project) -> Option<String> {
    let url = project.extra.get("plot")?.get("underlay")?.get("url")?.as_str()?.to_owned();
    if let Some((u, href)) = cache.as_ref()
        && *u == url
    {
        return Some(href.clone());
    }
    let name = url.strip_prefix(arklight_plot::ASSET_PREFIX)?;
    if name.contains(['/', '\\']) || name.contains("..") {
        return None;
    }
    let bytes = std::fs::read(assets_dir().join(name)).ok()?;
    let mime = match name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };
    let href = arklight_plot::data_url(mime, &bytes);
    *cache = Some((url, href.clone()));
    Some(href)
}

/// Image de fond affichée au sol dans la vue 3D (« Afficher au sol dans la vue 3D ») : décodée une
/// fois (au plus 2048 pixels de large), coins selon sa place, sa largeur et sa rotation sur le plan.
pub fn floor_image(cache: &mut Option<(String, std::sync::Arc<Vec<u8>>, u32, u32)>, project: &arklight_core::Project) -> Option<arklight_render::FloorImage> {
    use arklight_render::glam::Vec3;
    let u = project.extra.get("plot")?.get("underlay")?;
    if u.get("floor3d").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    let url = u.get("url")?.as_str()?.to_owned();
    let num = |k: &str, d: f64| u.get(k).and_then(serde_json::Value::as_f64).unwrap_or(d);
    let aspect = num("aspect", 1.0).max(0.01);
    if cache.as_ref().map(|c| c.0.as_str()) != Some(url.as_str()) {
        let href = underlay_href(&mut None, project)?;
        let w: u32 = 2048;
        let h = ((w as f64 / aspect).round() as u32).clamp(1, 2048);
        let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}"><image width="{w}" height="{h}" preserveAspectRatio="none" xlink:href="{href}"/></svg>"#);
        let rgba = arklight_plot::rasterize(&svg, w, h);
        if rgba.len() != (w * h * 4) as usize {
            return None;
        }
        *cache = Some((url.clone(), std::sync::Arc::new(rgba), w, h));
    }
    let (_, rgba, w, h) = cache.as_ref()?;
    let (cx, cz, width) = (num("x", 0.0) as f32, num("z", 0.0) as f32, num("width", 20.0) as f32);
    let height = width / aspect as f32;
    let r = (num("rotation", 0.0) as f32).to_radians();
    // coin de l'image (u vers la droite, v vers le haut) → sol, tourné comme sur le plan
    let at = |u: f32, v: f32| Vec3::new(cx + u * r.cos() + v * r.sin(), 0.004, cz + u * r.sin() - v * r.cos());
    let (hw, hh) = (width / 2.0, height / 2.0);
    Some(arklight_render::FloorImage {
        key: url,
        rgba: rgba.clone(),
        width: *w,
        height: *h,
        corners: [at(-hw, hh), at(hw, hh), at(hw, -hh), at(-hw, -hh)],
        opacity: num("opacity", 0.8) as f32,
    })
}

/// Côté le plus long d'une image de fond (pixels) : assez pour un plan imprimé, sans alourdir le projet.
pub const UNDERLAY_MAX: u32 = 4096;

/// Image de fond déjà convertie (« Image de fond… », photo jointe à l'assistant avec « Aussi en fond
/// du plan 2D ») : copiée dans le dossier des images et posée au centre de la vue.
pub fn place_underlay(st: &mut State, pic: &crate::picture::Picture) -> Result<(), String> {
    let name = format!("{}.{}", arklight_plot::new_id("u"), pic.ext());
    let dir = assets_dir();
    std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(&name), &pic.bytes)).map_err(|e| e.to_string())?;
    let op = arklight_plot::underlay_import(&format!("{}{name}", arklight_plot::ASSET_PREFIX), pic.width as f64 / pic.height.max(1) as f64, &st.plot.view);
    st.plot.dirty = true;
    apply_ops(st, vec![op]);
    Ok(())
}

pub fn remove_underlay(st: &mut State) -> bool {
    st.plot.dirty = true;
    apply_ops(st, vec![arklight_plot::underlay_remove()])
}

/// Calage de l'image de fond : deux clics sur le plan, puis la distance réelle.
pub fn start_calibration(st: &mut State) {
    st.plot.editor.start_calibration();
    st.plot.dirty = true;
}

pub fn calibration_ready(st: &State) -> bool {
    st.plot.editor.calibration_ready()
}

pub fn apply_calibration(st: &mut State, metres: f64) -> bool {
    with_editor(st, |e, c| e.apply_calibration(c, metres))
}

pub fn cancel_calibration(st: &mut State) {
    st.plot.editor.cancel_calibration();
    st.plot.dirty = true;
}
