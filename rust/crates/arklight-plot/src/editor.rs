//! Outils du plan de feu, sans interface (portage de la partie « édition » de
//! `web/js/plot/plot-view.js`) : sélection (clic, Ctrl+clic, cadre de sélection), déplacement des
//! projecteurs, des objets et des éléments du plan, poignée d'orientation, tracé des formes
//! (rectangle, ellipse, forme point par point, main levée, ligne, flèche, texte, mesure), calage
//! de l'image de fond sur deux points, menu contextuel (dupliquer, supprimer). En plus de la
//! version JavaScript : des poignées sur les sommets de l'élément sélectionné, pour le retoucher
//! ([`Editor::vertex_handles`]).
//!
//! L'application envoie les gestes en coordonnées du plan (mètres) : bouton gauche seulement (le
//! déplacement de la vue au clic droit, à la molette enfoncée ou avec Espace reste à
//! l'application) ; un double-clic envoie deux `pointer_down` / `pointer_up` puis
//! [`Editor::double_click`], comme le navigateur. Chaque geste renvoie des [`EditOp`] à appliquer
//! dans l'ordre : sur le plan avec [`crate::apply_op`], au document ([`EditOp::Doc`]), à la
//! sélection ([`EditOp::Select`]). Pendant un glissement, les opérations de `pointer_move` sont à
//! appliquer tout de suite (le document fusionne un geste continu en une seule étape
//! d'annulation) ; `pointer_up` renvoie l'état final.

use crate::catalog::{ObjectType, object_type};
use crate::geometry::{P2, dist_to_segment, fixture_plan, object_plan, point_in_polygon, rotate_yaw};
use crate::items::{ACCENT, EditOp, calibrate, clean_plot_item, jpoint, new_id, plot_item, r3};
use crate::js::{hypot, len16, max_of, min_of, num, round};
use crate::render::{Handle, ItemStyle, PlotData, PlotItem, PlotOptions, Selection, View, object_shown};
use arklight_core::document::{Changes, Edit};
use arklight_core::library::library;
use arklight_core::project::{Project, Vec3};
use serde_json::{Value, json};
use std::f64::consts::PI;

/// Outil du plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    #[default]
    Select,
    Rect,
    Ellipse,
    Polygon,
    Freehand,
    Line,
    Arrow,
    Text,
    Measure,
}

impl Tool {
    /// Outils dans l'ordre de la barre (`TOOLS`).
    pub const ALL: [Tool; 9] = [Tool::Select, Tool::Rect, Tool::Ellipse, Tool::Polygon, Tool::Freehand, Tool::Line, Tool::Arrow, Tool::Text, Tool::Measure];

    /// Identifiant (aussi le nom de l'icône et la nature des éléments tracés).
    pub fn id(self) -> &'static str {
        match self {
            Tool::Select => "select",
            Tool::Rect => "rect",
            Tool::Ellipse => "ellipse",
            Tool::Polygon => "polygon",
            Tool::Freehand => "freehand",
            Tool::Line => "line",
            Tool::Arrow => "arrow",
            Tool::Text => "text",
            Tool::Measure => "measure",
        }
    }

    pub fn from_id(id: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.id() == id)
    }

    /// Infobulle du bouton (et indication sous le plan quand l'outil est choisi).
    pub fn tip(self) -> &'static str {
        match self {
            Tool::Select => "Sélection : clic, Ctrl+clic, cadre de sélection ; glisser pour déplacer",
            Tool::Rect => "Rectangle (maison, décor…)",
            Tool::Ellipse => "Ellipse ou cercle",
            Tool::Polygon => "Forme point par point : double-clic ou Entrée pour terminer",
            Tool::Freehand => "Dessin à main levée",
            Tool::Line => "Ligne",
            Tool::Arrow => "Flèche (désigner un lieu, un kiosque…)",
            Tool::Text => "Texte",
            Tool::Measure => "Mesure d’une distance",
        }
    }
}

/// Indication sous le plan avec l'outil de sélection.
pub const SELECT_HINT: &str = "Clic : sélectionner · glisser : déplacer · poignée ⟳ : orienter · glisser dans le vide : cadre de sélection · clic droit glissé ou molette enfoncée : se déplacer · molette : zoom";
/// Calage de l'image de fond : premier point.
pub const CALIB_FIRST: &str = "Calage : clique le premier point de l’image dont tu connais la distance…";
/// Calage de l'image de fond : second point.
pub const CALIB_SECOND: &str = "Calage : clique le second point…";
/// Calage de l'image de fond : titre de la saisie de la distance.
pub const CALIB_DISTANCE: &str = "Distance réelle entre les deux points";

/// Ce que les outils lisent du projet et de l'application.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    pub project: &'a Project,
    /// vue du plan (l'échelle règle la tolérance des clics : 7 px)
    pub view: View,
    /// ce qui s'affiche (objets cliquables, annotations)
    pub options: &'a PlotOptions,
    /// sélection de l'application
    pub sel: &'a Selection,
}

/// Touches de modification du geste.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Ctrl ou Cmd : ajouter à la sélection
    pub ctrl: bool,
    /// Maj : ajouter à la sélection ; orientation libre (sans crans de 5°)
    pub shift: bool,
}

/// Élément de la scène orienté par une poignée.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Fixture(String),
    Object(String),
}

/// Rôle d'une poignée.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleKind {
    /// orientation d'un projecteur ou d'un objet
    Rotate(Target),
    /// sommet de l'élément du plan sélectionné (rectangle et ellipse : coin 0 à 3 dans le sens
    /// des aiguilles d'une montre depuis le premier point)
    Vertex { id: String, index: usize },
}

/// Poignée et ce qu'elle déplace.
#[derive(Debug, Clone, PartialEq)]
pub struct EditHandle {
    pub kind: HandleKind,
    pub x: f64,
    pub z: f64,
}

impl EditHandle {
    /// Poignée à dessiner (`RenderOptions::handles`).
    pub fn to_render(&self) -> Handle {
        let kind = match self.kind {
            HandleKind::Rotate(_) => "rotate",
            HandleKind::Vertex { .. } => "vertex",
        };
        Handle { kind: kind.into(), x: self.x, z: self.z }
    }
}

/// Ce qui est sous le curseur (`hitTest`).
#[derive(Debug, Clone, PartialEq)]
pub enum Hit {
    Handle(EditHandle),
    Fixture(String),
    Object(String),
    /// élément du plan (identifiant)
    Item(String),
}

/// Touches traitées par les outils.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// annule le tracé, le calage ou l'outil ; sinon désélectionne l'élément du plan
    Escape,
    /// termine la forme point par point
    Enter,
    /// Suppr ou Retour arrière : supprime l'élément du plan sélectionné
    Delete,
}

/// Geste en cours.
#[derive(Debug, Clone, PartialEq)]
enum Drag {
    Draw,
    Marquee { a: P2, b: P2, add: bool },
    Fixtures { start: P2, orig: Vec<(String, Vec3)>, moved: bool, last: Option<Vec<(String, Vec3)>> },
    Object { start: P2, id: String, orig: Vec3, moved: bool, last: Option<Vec3> },
    Item { start: P2, id: String, orig: Vec<P2>, moved: bool, last: Option<Vec<P2>> },
    Rotate { target: Target, center: P2, start: f64, rotation: Vec3, now: Option<Vec3> },
    Vertex { start: P2, id: String, kind: String, index: usize, orig: Vec<P2>, moved: bool, last: Option<Vec<P2>> },
}

/// Outils du plan : outil choisi, tracé en cours, geste en cours, calage de l'image de fond.
#[derive(Debug, Clone)]
pub struct Editor {
    tool: Tool,
    preview: Option<PlotItem>,
    drag: Option<Drag>,
    /// calage de l'image de fond : points cliqués (deux au plus)
    calib: Option<Vec<P2>>,
    /// poignées sur les sommets de l'élément du plan sélectionné (absentes de la version
    /// JavaScript ; activées par défaut)
    pub vertex_handles: bool,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

/// Style d'une forme tracée (`defaultStyle`).
fn default_style() -> Value {
    json!({ "stroke": ACCENT, "fill": "none", "width": 2, "dash": false, "size": 14 })
}

/// Sélection modifiée comme le fait `store.js` (projecteurs, objet, élément du plan exclusifs).
struct Sel<'a> {
    s: Selection,
    project: &'a Project,
}

impl Sel<'_> {
    /// `setSelection` : projecteurs existants, sans doublon ; des projecteurs choisis
    /// désélectionnent l'objet.
    fn set(&mut self, ids: Vec<String>) {
        let mut next: Vec<String> = Vec::new();
        for id in ids {
            if !next.contains(&id) && self.project.fixtures.iter().any(|f| f.id == id) {
                next.push(id);
            }
        }
        if !next.is_empty() {
            self.s.object_id = None;
        }
        self.s.fixtures = next;
    }

    fn select(&mut self, id: Option<&str>) {
        self.set(id.map(|i| vec![i.to_string()]).unwrap_or_default());
    }

    /// Ctrl+clic : ajoute ou retire un projecteur.
    fn toggle(&mut self, id: &str) {
        if self.s.fixtures.iter().any(|x| x == id) {
            let rest = self.s.fixtures.iter().filter(|x| *x != id).cloned().collect();
            self.set(rest);
        } else {
            let mut all = self.s.fixtures.clone();
            all.push(id.to_string());
            self.set(all);
        }
    }

    fn select_object(&mut self, id: Option<&str>) {
        if id.is_some() {
            self.set(Vec::new());
        }
        self.s.object_id = id.map(str::to_string);
    }

    /// `selectPlot` : un élément du plan choisi désélectionne projecteurs et objet.
    fn select_plot(&mut self, id: Option<&str>) {
        self.s.plot_id = id.map(str::to_string);
        if id.is_some() {
            self.select(None);
            self.select_object(None);
        }
    }

    /// Opération si la sélection a changé.
    fn op(self, before: &Selection) -> Option<EditOp> {
        (self.s != *before).then_some(EditOp::Select(self.s))
    }
}

/// Projecteurs sélectionnés, dans l'ordre de la scène (`selectedIds`).
fn selected_ids(ctx: &Ctx) -> Vec<String> {
    ctx.project.fixtures.iter().filter(|f| ctx.sel.fixtures.contains(&f.id)).map(|f| f.id.clone()).collect()
}

/// Élément du plan nettoyé, avec un identifiant neuf, puis sélectionné (`plot-item-created`).
fn created(item: &Value) -> Vec<EditOp> {
    let id = new_id("p");
    match clean_plot_item(item, Some(&id)) {
        Some(it) => vec![EditOp::AddItem(it), EditOp::Select(Selection { plot_id: Some(id), ..Selection::default() })],
        None => Vec::new(),
    }
}

fn points_json(points: &[P2]) -> Value {
    Value::Array(points.iter().map(|p| jpoint(*p)).collect())
}

fn points_op(id: &str, points: &[P2]) -> EditOp {
    EditOp::UpdateItem { id: id.to_string(), changes: json!({ "points": points_json(points) }) }
}

fn fixtures_op(list: &[(String, Vec3)]) -> EditOp {
    EditOp::Doc(Edit::UpdateFixtures(list.iter().map(|(id, pos)| (id.clone(), Changes::moved(*pos, None))).collect()))
}

fn object_op(id: &str, pos: Vec3) -> EditOp {
    EditOp::Doc(Edit::UpdateObject { id: id.to_string(), changes: Changes::moved(pos, None) })
}

fn rotation_op(target: &Target, rotation: Vec3) -> EditOp {
    let changes = Changes { rotation: Some(rotation), ..Changes::default() };
    EditOp::Doc(match target {
        Target::Fixture(id) => Edit::UpdateFixture { id: id.clone(), changes },
        Target::Object(id) => Edit::UpdateObject { id: id.clone(), changes },
    })
}

/// Points d'un élément (JSON du projet).
fn item_points(it: &Value) -> Vec<P2> {
    PlotItem::from_json(it).points
}

/// Sommets proposés à la retouche : coins d'un rectangle ou d'une ellipse, extrémités d'une
/// ligne, d'une flèche ou d'une mesure, sommets d'une forme (ni main levée, ni texte).
fn vertices(kind: &str, pts: &[P2]) -> Vec<P2> {
    match (kind, pts) {
        ("rect" | "ellipse", [[x0, z0], [x1, z1], ..]) => vec![[*x0, *z0], [*x1, *z0], [*x1, *z1], [*x0, *z1]],
        ("line" | "arrow" | "measure", [a, b, ..]) => vec![*a, *b],
        ("polygon", _) => pts.to_vec(),
        _ => Vec::new(),
    }
}

/// Points de l'élément après le déplacement du sommet `index` en `q`.
fn move_vertex(kind: &str, orig: &[P2], index: usize, q: P2) -> Vec<P2> {
    let mut pts = orig.to_vec();
    match (kind, index) {
        ("rect" | "ellipse", 0) => pts[0] = q,
        ("rect" | "ellipse", 1) => {
            pts[1][0] = q[0];
            pts[0][1] = q[1];
        }
        ("rect" | "ellipse", 2) => pts[1] = q,
        ("rect" | "ellipse", 3) => {
            pts[0][0] = q[0];
            pts[1][1] = q[1];
        }
        (_, i) if i < pts.len() => pts[i] = q,
        _ => {}
    }
    pts
}

/// `hitItem` : clic sur un élément du plan (`t` : tolérance en mètres, `s` : échelle en px/m).
fn hit_item(it: &PlotItem, p: P2, t: f64, s: f64) -> bool {
    let pts = &it.points;
    match it.kind.as_str() {
        "rect" | "ellipse" => {
            let (Some(&[x0, z0]), Some(&[x1, z1])) = (pts.first(), pts.get(1)) else { return false };
            let inside = p[0] >= x0.min(x1) - t && p[0] <= x0.max(x1) + t && p[1] >= z0.min(z1) - t && p[1] <= z0.max(z1) + t;
            if !inside {
                return false;
            }
            if it.style.fill.as_deref().is_some_and(|f| f != "none") {
                return true;
            }
            let near_edge = min_of([(p[0] - x0).abs(), (p[0] - x1).abs(), (p[1] - z0).abs(), (p[1] - z1).abs()]) <= t * 1.5;
            return it.kind == "ellipse" || near_edge;
        }
        "text" => {
            // boîte approximative du texte (ancré en bas à gauche)
            let Some(&[x, z]) = pts.first() else { return false };
            let size = it.style.size.unwrap_or(16.0);
            let w = (len16(it.text.as_deref().unwrap_or("Texte")) as f64 * size * 0.6) / s;
            return p[0] >= x - t && p[0] <= x + w + t && p[1] <= z + t && p[1] >= z - size / s - t;
        }
        _ => {}
    }
    if it.kind == "polygon" && point_in_polygon(p, pts) {
        return true;
    }
    if pts.windows(2).any(|w| dist_to_segment(p, w[0], w[1]) <= t) {
        return true;
    }
    it.kind == "polygon" && pts.len() > 2 && dist_to_segment(p, pts[pts.len() - 1], pts[0]) <= t
}

impl Editor {
    pub fn new() -> Self {
        Self { tool: Tool::Select, preview: None, drag: None, calib: None, vertex_handles: true }
    }

    /// Outil choisi.
    pub fn tool(&self) -> Tool {
        self.tool
    }

    /// Choisit un outil (`setTool`) : le tracé et le calage en cours sont abandonnés. Pour
    /// dessiner, les annotations doivent être visibles : renvoie true si elles viennent d'être
    /// affichées (l'application peut le signaler : « Annotations affichées (Affichage ▾ pour les
    /// masquer) »).
    pub fn set_tool(&mut self, tool: Tool, options: &mut PlotOptions) -> bool {
        self.reset(tool);
        if tool != Tool::Select && !options.annotations {
            options.annotations = true;
            return true;
        }
        false
    }

    fn reset(&mut self, tool: Tool) {
        self.tool = tool;
        self.preview = None;
        self.calib = None;
    }

    /// Élément en cours de tracé (`RenderOptions::preview`).
    pub fn preview(&self) -> Option<PlotItem> {
        self.preview.clone()
    }

    /// Un geste (glissement) est en cours.
    pub fn busy(&self) -> bool {
        self.drag.is_some()
    }

    /// Indication sous le plan.
    pub fn hint(&self) -> &'static str {
        match &self.calib {
            Some(p) if p.is_empty() => CALIB_FIRST,
            Some(_) => CALIB_SECOND,
            None if self.tool == Tool::Select => SELECT_HINT,
            None => self.tool.tip(),
        }
    }

    /// Curseur : flèche avec l'outil de sélection, croix pour tracer ou caler.
    pub fn crosshair(&self) -> bool {
        self.tool != Tool::Select || self.calib.is_some()
    }

    // ------------------------------------------------------------------ poignées et clics
    /// Poignées et ce qu'elles déplacent : orientation du projecteur sélectionné (seul, s'il
    /// éclaire de côté) ou de l'objet sélectionné (décor, cadre, drapeau, réflecteur) ; sinon les
    /// sommets de l'élément du plan sélectionné.
    pub fn edit_handles(&self, ctx: &Ctx) -> Vec<EditHandle> {
        let lib = library();
        let ids = selected_ids(ctx);
        let one = if ids.len() == 1 { ctx.project.fixtures.iter().find(|f| f.id == ids[0]) } else { None };
        if let Some(f) = one
            && let Some(ty) = lib.fixture(&f.type_id)
        {
            let p = fixture_plan(f, ty);
            if let Some(d) = p.dir {
                return vec![EditHandle { kind: HandleKind::Rotate(Target::Fixture(f.id.clone())), x: p.center[0] + d[0] * 1.9, z: p.center[1] + d[1] * 1.9 }];
            }
        }
        let object = ctx.sel.object_id.as_deref().and_then(|id| ctx.project.objects.iter().find(|o| o.id == id));
        if let Some(o) = object
            && ids.is_empty()
            && let Some(ty) = object_type(&o.type_id)
            && (ty.is_prop() || matches!(ty.family(), Some("frame" | "flag" | "bounce")))
        {
            let a = o.rotation[1];
            let size = match ty {
                ObjectType::Prop(p) if ty.is_prop() => max_of([p.size[0], p.size[2]]),
                ObjectType::Grip(g) => g.w.filter(|w| *w != 0.0 && !w.is_nan()).unwrap_or(1.0),
                ObjectType::Prop(_) => 1.0,
            };
            let d = max_of([0.8, size / 2.0 + 0.4]);
            return vec![EditHandle { kind: HandleKind::Rotate(Target::Object(o.id.clone())), x: o.position[0] + a.sin() * d, z: o.position[2] + a.cos() * d }];
        }
        if !self.vertex_handles || !ctx.options.annotations {
            return Vec::new();
        }
        let Some(id) = ctx.sel.plot_id.as_deref() else { return Vec::new() };
        let Some(it) = plot_item(ctx.project, id) else { return Vec::new() };
        let kind = it.get("kind").and_then(Value::as_str).unwrap_or("");
        vertices(kind, &item_points(it)).into_iter().enumerate().map(|(index, [x, z])| EditHandle { kind: HandleKind::Vertex { id: id.to_string(), index }, x, z }).collect()
    }

    /// Poignées à dessiner (`RenderOptions::handles`).
    pub fn handles(&self, ctx: &Ctx) -> Vec<Handle> {
        self.edit_handles(ctx).iter().map(EditHandle::to_render).collect()
    }

    /// `hitTest` : poignée, projecteur, objet (le décor et le grip, puis les cadres par leurs
    /// tubes ou leur centre), élément du plan (si les annotations sont affichées) ; le plus haut
    /// d'abord.
    pub fn hit_test(&self, ctx: &Ctx, p: P2) -> Option<Hit> {
        let tol = 7.0 / ctx.view.s;
        if let Some(h) = self.edit_handles(ctx).into_iter().find(|h| hypot(p[0] - h.x, p[1] - h.z) <= tol * 1.4) {
            return Some(Hit::Handle(h));
        }
        let lib = library();
        for f in ctx.project.fixtures.iter().rev() {
            let Some(ty) = lib.fixture(&f.type_id) else { continue };
            let pl = fixture_plan(f, ty);
            if point_in_polygon(p, &pl.poly) || hypot(p[0] - pl.center[0], p[1] - pl.center[1]) <= tol * 1.5 {
                return Some(Hit::Fixture(f.id.clone()));
            }
        }
        // objets, puis cadres : un cadre est transparent sur le plan, on le prend par ses tubes ou son centre
        for frames in [false, true] {
            for o in ctx.project.objects.iter().rev() {
                let Some(ty) = object_type(&o.type_id) else { continue };
                if !object_shown(&ty, ctx.options) || (ty.family() == Some("frame")) != frames {
                    continue;
                }
                let pl = object_plan(o, ty);
                let near = hypot(p[0] - pl.center[0], p[1] - pl.center[1]) <= tol * 1.5;
                let n = pl.poly.len();
                let on = if frames { (0..n).any(|k| dist_to_segment(p, pl.poly[k], pl.poly[(k + 1) % n]) <= tol * 1.5) } else { point_in_polygon(p, &pl.poly) };
                if near || on {
                    return Some(Hit::Object(o.id.clone()));
                }
            }
        }
        if ctx.options.annotations {
            let plot = PlotData::of(ctx.project);
            for it in plot.items.iter().rev() {
                if let Some(id) = it.id.as_ref().filter(|_| hit_item(it, p, tol, ctx.view.s)) {
                    return Some(Hit::Item(id.clone()));
                }
            }
        }
        None
    }

    // ------------------------------------------------------------------ souris
    /// Bouton gauche enfoncé.
    pub fn pointer_down(&mut self, ctx: &Ctx, p: P2, m: Modifiers) -> Vec<EditOp> {
        if let Some(points) = &mut self.calib {
            if points.len() < 2 {
                points.push(p);
            }
            return Vec::new();
        }
        match self.tool {
            Tool::Select => self.select_down(ctx, p, m),
            Tool::Polygon => {
                match &mut self.preview {
                    Some(pv) => {
                        let at = pv.points.len().saturating_sub(1);
                        pv.points.insert(at, p);
                    }
                    None => self.preview = Some(Self::new_preview("polygon", vec![p, p])),
                }
                Vec::new()
            }
            Tool::Text => {
                self.reset(Tool::Select);
                created(&json!({ "kind": "text", "points": [jpoint(p)], "text": "Texte", "style": { "stroke": "#ffffff", "size": 16 } }))
            }
            tool => {
                let points = if tool == Tool::Freehand { vec![p] } else { vec![p, p] };
                self.preview = Some(Self::new_preview(tool.id(), points));
                self.drag = Some(Drag::Draw);
                Vec::new()
            }
        }
    }

    fn new_preview(kind: &str, points: Vec<P2>) -> PlotItem {
        let style = ItemStyle { stroke: Some(ACCENT.into()), fill: Some("none".into()), width: Some(2.0), dash: false, size: Some(14.0) };
        PlotItem { kind: kind.into(), points, style, ..PlotItem::default() }
    }

    fn select_down(&mut self, ctx: &Ctx, p: P2, m: Modifiers) -> Vec<EditOp> {
        let add = m.ctrl || m.shift;
        let mut sel = Sel { s: ctx.sel.clone(), project: ctx.project };
        match self.hit_test(ctx, p) {
            Some(Hit::Handle(h)) => match h.kind {
                HandleKind::Rotate(target) => {
                    let el = match &target {
                        Target::Fixture(id) => ctx.project.fixtures.iter().find(|f| &f.id == id).map(|f| (f.position, f.rotation)),
                        Target::Object(id) => ctx.project.objects.iter().find(|o| &o.id == id).map(|o| (o.position, o.rotation)),
                    };
                    if let Some((pos, rotation)) = el {
                        let center = [pos[0], pos[2]];
                        self.drag = Some(Drag::Rotate { target, center, start: (p[1] - center[1]).atan2(p[0] - center[0]), rotation, now: None });
                    }
                    return Vec::new();
                }
                HandleKind::Vertex { id, index } => {
                    if let Some(it) = plot_item(ctx.project, &id) {
                        let kind = it.get("kind").and_then(Value::as_str).unwrap_or("").to_string();
                        self.drag = Some(Drag::Vertex { start: p, id, kind, index, orig: item_points(it), moved: false, last: None });
                    }
                    return Vec::new();
                }
            },
            Some(Hit::Fixture(id)) => {
                sel.select_plot(None);
                if add {
                    sel.toggle(&id);
                } else if !sel.s.fixtures.contains(&id) {
                    sel.select(Some(&id));
                }
                let orig = ctx.project.fixtures.iter().filter(|f| sel.s.fixtures.contains(&f.id)).map(|f| (f.id.clone(), f.position)).collect();
                self.drag = Some(Drag::Fixtures { start: p, orig, moved: false, last: None });
            }
            Some(Hit::Object(id)) => {
                sel.select_plot(None);
                sel.select_object(Some(&id));
                let orig = ctx.project.objects.iter().find(|o| o.id == id).map_or([0.0; 3], |o| o.position);
                self.drag = Some(Drag::Object { start: p, id, orig, moved: false, last: None });
            }
            Some(Hit::Item(id)) => {
                sel.select_plot(Some(&id));
                let orig = plot_item(ctx.project, &id).map(item_points).unwrap_or_default();
                self.drag = Some(Drag::Item { start: p, id, orig, moved: false, last: None });
            }
            None => {
                if !add {
                    sel.select(None);
                    sel.select_object(None);
                    sel.select_plot(None);
                }
                self.drag = Some(Drag::Marquee { a: p, b: p, add });
            }
        }
        sel.op(ctx.sel).into_iter().collect()
    }

    /// Souris déplacée (bouton enfoncé ou non).
    pub fn pointer_move(&mut self, ctx: &Ctx, p: P2, m: Modifiers) -> Vec<EditOp> {
        let s = ctx.view.s;
        if self.tool == Tool::Polygon
            && self.drag.is_none()
            && let Some(pv) = &mut self.preview
        {
            if let Some(last) = pv.points.last_mut() {
                *last = p;
            }
            return Vec::new();
        }
        let Some(drag) = &mut self.drag else { return Vec::new() };
        // déplacement : seulement au-delà de 3 px
        let started = |start: &P2, moved: &mut bool| {
            let (dx, dz) = (p[0] - start[0], p[1] - start[1]);
            *moved = *moved || hypot(dx, dz) * s > 3.0;
            (*moved).then_some([dx, dz])
        };
        match drag {
            Drag::Draw => {
                if let Some(pv) = &mut self.preview {
                    if pv.kind == "freehand" {
                        let last = pv.points.last().copied().unwrap_or(p);
                        if hypot(p[0] - last[0], p[1] - last[1]) > 3.0 / s {
                            pv.points.push(p);
                        }
                    } else if pv.points.len() > 1 {
                        pv.points[1] = p;
                    }
                }
                Vec::new()
            }
            Drag::Marquee { b, .. } => {
                *b = p;
                Vec::new()
            }
            Drag::Fixtures { start, orig, moved, last } => {
                let Some([dx, dz]) = started(start, moved) else { return Vec::new() };
                let list: Vec<(String, Vec3)> = orig.iter().map(|(id, o)| (id.clone(), [r3(o[0] + dx), o[1], r3(o[2] + dz)])).collect();
                let op = fixtures_op(&list);
                *last = Some(list);
                vec![op]
            }
            Drag::Object { start, id, orig, moved, last } => {
                let Some([dx, dz]) = started(start, moved) else { return Vec::new() };
                let pos = [r3(orig[0] + dx), orig[1], r3(orig[2] + dz)];
                *last = Some(pos);
                vec![object_op(id, pos)]
            }
            Drag::Item { start, id, orig, moved, last } => {
                let Some([dx, dz]) = started(start, moved) else { return Vec::new() };
                let pts: Vec<P2> = orig.iter().map(|[x, z]| [r3(x + dx), r3(z + dz)]).collect();
                let op = points_op(id, &pts);
                *last = Some(pts);
                vec![op]
            }
            Drag::Vertex { start, id, kind, index, orig, moved, last } => {
                let Some([dx, dz]) = started(start, moved) else { return Vec::new() };
                let Some(v) = vertices(kind, orig).get(*index).copied() else { return Vec::new() };
                let pts = move_vertex(kind, orig, *index, [r3(v[0] + dx), r3(v[1] + dz)]);
                let op = points_op(id, &pts);
                *last = Some(pts);
                vec![op]
            }
            Drag::Rotate { target, center, start, rotation, now } => {
                let mut a = (p[1] - center[1]).atan2(p[0] - center[0]) - *start;
                if !m.shift {
                    a = round(a / (PI / 36.0)) * (PI / 36.0); // crans de 5° (Maj : libre)
                }
                let r = rotate_yaw(*rotation, -a);
                *now = Some(r);
                vec![rotation_op(target, r)]
            }
        }
    }

    /// Bouton gauche relâché.
    pub fn pointer_up(&mut self, ctx: &Ctx, _p: P2) -> Vec<EditOp> {
        let Some(drag) = self.drag.take() else { return Vec::new() };
        match drag {
            Drag::Draw => {
                // tracé abandonné (Échap) pendant le geste : rien à ajouter
                let Some(pv) = self.preview.take() else { return Vec::new() };
                let (Some(a), Some(b)) = (pv.points.first(), pv.points.last()) else { return Vec::new() };
                let size = hypot(b[0] - a[0], b[1] - a[1]) * ctx.view.s;
                let freehand = pv.kind == "freehand";
                if !(if freehand { pv.points.len() > 2 } else { size > 6.0 }) {
                    return Vec::new();
                }
                let ops = created(&json!({ "kind": pv.kind, "points": points_json(&pv.points), "style": default_style() }));
                if !freehand {
                    self.reset(Tool::Select);
                }
                ops
            }
            Drag::Marquee { a, b, add } => {
                let (x0, x1) = (a[0].min(b[0]), a[0].max(b[0]));
                let (z0, z1) = (a[1].min(b[1]), a[1].max(b[1]));
                if (x1 - x0) * ctx.view.s <= 4.0 && (z1 - z0) * ctx.view.s <= 4.0 {
                    return Vec::new();
                }
                let ids: Vec<String> = ctx.project.fixtures.iter().filter(|f| f.position[0] >= x0 && f.position[0] <= x1 && f.position[2] >= z0 && f.position[2] <= z1).map(|f| f.id.clone()).collect();
                let mut sel = Sel { s: ctx.sel.clone(), project: ctx.project };
                let all = if add { sel.s.fixtures.iter().cloned().chain(ids).collect() } else { ids };
                sel.set(all);
                sel.op(ctx.sel).into_iter().collect()
            }
            Drag::Fixtures { last: Some(list), .. } => vec![fixtures_op(&list)],
            Drag::Object { id, last: Some(pos), .. } => vec![object_op(&id, pos)],
            Drag::Item { id, last: Some(pts), .. } | Drag::Vertex { id, last: Some(pts), .. } => vec![points_op(&id, &pts)],
            Drag::Rotate { target, now: Some(r), .. } => vec![rotation_op(&target, r)],
            _ => Vec::new(),
        }
    }

    /// Double-clic : termine la forme point par point.
    pub fn double_click(&mut self, ctx: &Ctx) -> Vec<EditOp> {
        if self.tool == Tool::Polygon { self.finish_polygon(ctx) } else { Vec::new() }
    }

    /// Termine la forme point par point (double-clic, Entrée) : au moins 3 points, remplie.
    pub fn finish_polygon(&mut self, _ctx: &Ctx) -> Vec<EditOp> {
        let Some(pv) = self.preview.take_if(|pv| pv.kind == "polygon") else { return Vec::new() };
        let mut points = pv.points;
        points.pop();
        // le double-clic ajoute deux fois le dernier point
        while points.len() > 1 {
            let (a, b) = (points[points.len() - 1], points[points.len() - 2]);
            if hypot(a[0] - b[0], a[1] - b[1]) >= 1e-3 {
                break;
            }
            points.pop();
        }
        let ops = if points.len() >= 3 {
            let mut style = default_style();
            style["fill"] = ACCENT.into();
            created(&json!({ "kind": "polygon", "points": points_json(&points), "style": style }))
        } else {
            Vec::new()
        };
        self.reset(Tool::Select);
        ops
    }

    /// Touche du clavier ; None si les outils ne l'ont pas prise (elle peut servir ailleurs).
    pub fn key(&mut self, ctx: &Ctx, key: Key) -> Option<Vec<EditOp>> {
        match key {
            Key::Escape => {
                if self.preview.is_some() || self.calib.is_some() || self.tool != Tool::Select {
                    self.reset(Tool::Select);
                    Some(Vec::new())
                } else if ctx.sel.plot_id.is_some() {
                    Some(vec![EditOp::Select(Selection { plot_id: None, ..ctx.sel.clone() })])
                } else {
                    None
                }
            }
            Key::Enter if self.tool == Tool::Polygon => Some(self.finish_polygon(ctx)),
            Key::Delete => ctx.sel.plot_id.clone().map(|id| self.remove(ctx, &id)),
            Key::Enter => None,
        }
    }

    // ------------------------------------------------------------------ menu contextuel
    /// Clic droit sans glisser : ce qui est sous le curseur, et la sélection qui change (le
    /// projecteur, l'objet ou l'élément cliqué). Élément du plan : menu « Dupliquer » (voir
    /// [`Editor::duplicate`]), « Supprimer » ([`Editor::remove`]), titre [`crate::kind_name`] ;
    /// sinon le menu de la sélection de l'application.
    pub fn context_menu(&self, ctx: &Ctx, p: P2) -> (Option<Hit>, Vec<EditOp>) {
        let hit = self.hit_test(ctx, p);
        let mut sel = Sel { s: ctx.sel.clone(), project: ctx.project };
        match &hit {
            Some(Hit::Fixture(id)) if !sel.s.fixtures.contains(id) => sel.select(Some(id)),
            Some(Hit::Object(id)) => sel.select_object(Some(id)),
            Some(Hit::Item(id)) => sel.select_plot(Some(id)),
            _ => {}
        }
        let ops = sel.op(ctx.sel).into_iter().collect();
        (hit, ops)
    }

    /// Duplique un élément du plan (décalé de 50 cm) et le sélectionne.
    pub fn duplicate(&self, ctx: &Ctx, id: &str) -> Vec<EditOp> {
        let Some(Value::Object(it)) = plot_item(ctx.project, id) else { return Vec::new() };
        let mut copy = it.clone();
        copy.remove("id");
        let shifted: Vec<P2> = item_points(&Value::Object(it.clone())).iter().map(|[x, z]| [x + 0.5, z + 0.5]).collect();
        copy.insert("points".into(), points_json(&shifted));
        created(&Value::Object(copy))
    }

    /// Supprime un élément du plan (et le désélectionne).
    pub fn remove(&self, ctx: &Ctx, id: &str) -> Vec<EditOp> {
        let mut ops = vec![EditOp::RemoveItem(id.to_string())];
        if ctx.sel.plot_id.as_deref() == Some(id) {
            ops.push(EditOp::Select(Selection { plot_id: None, ..ctx.sel.clone() }));
        }
        ops
    }

    /// Après un changement du projet (annuler, autre plan…) : l'élément du plan sélectionné qui
    /// n'existe plus est désélectionné.
    pub fn sync(&self, ctx: &Ctx) -> Option<EditOp> {
        let id = ctx.sel.plot_id.as_deref()?;
        plot_item(ctx.project, id).is_none().then(|| EditOp::Select(Selection { plot_id: None, ..ctx.sel.clone() }))
    }

    // ------------------------------------------------------------------ cadre de sélection
    /// Cadre de sélection en cours (deux coins, en mètres).
    pub fn marquee(&self) -> Option<[P2; 2]> {
        match &self.drag {
            Some(Drag::Marquee { a, b, .. }) => Some([*a, *b]),
            _ => None,
        }
    }

    /// Cadre de sélection en pixels de la vue : x, y, largeur, hauteur.
    pub fn marquee_rect(&self, view: &View) -> Option<[f64; 4]> {
        let [a, b] = self.marquee()?;
        let x = |v: f64| (v - view.cx) * view.s + view.w / 2.0;
        let z = |v: f64| (v - view.cz) * view.s + view.h / 2.0;
        Some([x(a[0]).min(x(b[0])), z(a[1]).min(z(b[1])), (x(b[0]) - x(a[0])).abs(), (z(b[1]) - z(a[1])).abs()])
    }

    /// Cadre de sélection à ajouter au SVG de la vue (`.plot-marquee`).
    pub fn marquee_svg(&self, view: &View) -> Option<String> {
        let [x, y, w, h] = self.marquee_rect(view)?;
        Some(format!(r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{ACCENT}" fill-opacity="0.08" stroke="{ACCENT}" stroke-dasharray="4 3"/>"#, num(x), num(y), num(w), num(h)))
    }

    // ------------------------------------------------------------------ calage de l'image de fond
    /// « Caler l’échelle (2 points) » : les deux clics suivants désignent deux points de l'image.
    pub fn start_calibration(&mut self) {
        self.calib = Some(Vec::new());
    }

    /// Points du calage déjà cliqués (None : pas de calage en cours).
    pub fn calibration(&self) -> Option<&[P2]> {
        self.calib.as_deref()
    }

    /// Les deux points sont cliqués : demander la distance réelle ([`CALIB_DISTANCE`]).
    pub fn calibration_ready(&self) -> bool {
        self.calib.as_ref().is_some_and(|p| p.len() == 2)
    }

    /// Distance réelle (m) entre les deux points : l'image est mise à l'échelle autour du premier
    /// point (« Image mise à l’échelle »). Rien si la distance n'est pas valable (le calage
    /// attend une autre saisie).
    pub fn apply_calibration(&mut self, ctx: &Ctx, real: f64) -> Vec<EditOp> {
        let Some([a, b]) = self.calib.as_deref().and_then(|p| <[P2; 2]>::try_from(p).ok()) else { return Vec::new() };
        let plot = ctx.project.extra.get("plot");
        let Some(op) = calibrate(plot, a, b, real) else {
            if plot.and_then(|p| p.get("underlay")).is_none_or(Value::is_null) {
                self.reset(Tool::Select); // plus d'image à caler
            }
            return Vec::new();
        };
        self.reset(Tool::Select);
        vec![op]
    }

    /// Annule le calage (retour à l'outil de sélection).
    pub fn cancel_calibration(&mut self) {
        self.reset(Tool::Select);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::apply_to_project;

    fn project() -> Project {
        let mut p = Project::default();
        p.extra.insert("plot".into(), crate::items::empty_plot());
        p
    }

    /// Applique les opérations du plan et la sélection (les autres sont ignorées).
    fn run(p: &mut Project, sel: &mut Selection, ops: Vec<EditOp>) {
        for op in ops {
            match op {
                EditOp::Select(s) => *sel = s,
                op => {
                    apply_to_project(p, &op);
                }
            }
        }
    }

    const VIEW: View = View { cx: 0.0, cz: 0.0, s: 50.0, w: 800.0, h: 600.0 };

    #[test]
    fn rectangle_trace_puis_deplace() {
        let mut p = project();
        let mut sel = Selection::default();
        let mut options = PlotOptions::default();
        let mut ed = Editor::new();
        assert!(ed.set_tool(Tool::Rect, &mut options), "annotations affichées");
        assert!(!ed.set_tool(Tool::Rect, &mut options));
        assert_eq!(ed.hint(), "Rectangle (maison, décor…)");
        let m = Modifiers::default();
        let ctx = Ctx { project: &p, view: VIEW, options: &options, sel: &sel };
        assert!(ed.pointer_down(&ctx, [0.0, 0.0], m).is_empty());
        ed.pointer_move(&ctx, [2.0, 1.0], m);
        assert_eq!(ed.preview().unwrap().points, vec![[0.0, 0.0], [2.0, 1.0]]);
        let ops = ed.pointer_up(&ctx, [2.0, 1.0]);
        assert_eq!(ops.len(), 2);
        run(&mut p, &mut sel, ops);
        assert_eq!(ed.tool(), Tool::Select);
        let it = &crate::items::plot_items(&p)[0];
        assert_eq!(it["kind"], "rect");
        assert_eq!(it["style"], json!({ "stroke": ACCENT, "fill": "none", "width": 2, "dash": false, "size": 14 }));
        assert_eq!(sel.plot_id.as_deref(), it["id"].as_str());
        // poignées des 4 coins ; déplacer le rectangle par son bord
        let ctx = Ctx { project: &p, view: VIEW, options: &options, sel: &sel };
        assert_eq!(ed.handles(&ctx).len(), 4);
        let mut ed2 = Editor { vertex_handles: false, ..Editor::new() };
        assert!(ed2.handles(&ctx).is_empty());
        assert!(ed2.pointer_down(&ctx, [1.0, 0.02], m).is_empty(), "déjà sélectionné");
        let ops = ed2.pointer_move(&ctx, [1.5, 0.52], m);
        let ops2 = ed2.pointer_up(&ctx, [1.5, 0.52]);
        assert_eq!(ops, ops2);
        run(&mut p, &mut sel, ops2);
        assert_eq!(crate::items::plot_items(&p)[0]["points"], json!([[0.5, 0.5], [2.5, 1.5]]));
        // un coin retouché
        let ctx = Ctx { project: &p, view: VIEW, options: &options, sel: &sel };
        ed.pointer_down(&ctx, [2.5, 0.5], m);
        ed.pointer_move(&ctx, [3.0, 0.0], m);
        let ops = ed.pointer_up(&ctx, [3.0, 0.0]);
        run(&mut p, &mut sel, ops);
        assert_eq!(crate::items::plot_items(&p)[0]["points"], json!([[0.5, 0], [3, 1.5]]));
        // Suppr
        let ctx = Ctx { project: &p, view: VIEW, options: &options, sel: &sel };
        let ops = ed.key(&ctx, Key::Delete).unwrap();
        run(&mut p, &mut sel, ops);
        assert!(crate::items::plot_items(&p).is_empty());
        assert_eq!(sel, Selection::default());
    }

    #[test]
    fn trop_petit_ou_abandonne() {
        let p = project();
        let sel = Selection::default();
        let mut options = PlotOptions::show_all();
        let mut ed = Editor::new();
        ed.set_tool(Tool::Line, &mut options);
        let ctx = Ctx { project: &p, view: VIEW, options: &options, sel: &sel };
        let m = Modifiers::default();
        ed.pointer_down(&ctx, [0.0, 0.0], m);
        ed.pointer_move(&ctx, [0.1, 0.0], m); // 5 px
        assert!(ed.pointer_up(&ctx, [0.1, 0.0]).is_empty());
        assert_eq!(ed.tool(), Tool::Line);
        // Échap pendant le tracé, puis relâché (la version JavaScript s'arrête sur une erreur)
        ed.pointer_down(&ctx, [0.0, 0.0], m);
        ed.pointer_move(&ctx, [2.0, 0.0], m);
        assert_eq!(ed.key(&ctx, Key::Escape), Some(Vec::new()));
        assert!(ed.pointer_up(&ctx, [2.0, 0.0]).is_empty());
        assert_eq!(ed.key(&ctx, Key::Escape), None);
    }

    #[test]
    fn forme_point_par_point() {
        let mut p = project();
        let mut sel = Selection::default();
        let mut options = PlotOptions::show_all();
        let mut ed = Editor::new();
        ed.set_tool(Tool::Polygon, &mut options);
        let ctx = Ctx { project: &p, view: VIEW, options: &options, sel: &sel };
        let m = Modifiers::default();
        for q in [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [2.0, 2.0]] {
            ed.pointer_down(&ctx, q, m);
            ed.pointer_up(&ctx, q);
            ed.pointer_move(&ctx, q, m);
        }
        let ops = ed.double_click(&ctx);
        run(&mut p, &mut sel, ops);
        let it = &crate::items::plot_items(&p)[0];
        assert_eq!(it["points"], json!([[0, 0], [2, 0], [2, 2]]));
        assert_eq!(it["style"]["fill"], ACCENT);
        assert_eq!(ed.tool(), Tool::Select);
    }

    #[test]
    fn calage_de_l_image() {
        let mut p = project();
        p.extra.get_mut("plot").unwrap()["underlay"] = json!({ "url": "/assets/a.png", "x": 1, "z": 1, "width": 10, "aspect": 1.5, "rotation": 0, "opacity": 0.6, "floor3d": false });
        let sel = Selection::default();
        let options = PlotOptions::default();
        let mut ed = Editor::new();
        ed.start_calibration();
        assert_eq!(ed.hint(), CALIB_FIRST);
        let ctx = Ctx { project: &p, view: VIEW, options: &options, sel: &sel };
        ed.pointer_down(&ctx, [0.0, 0.0], Modifiers::default());
        assert_eq!(ed.hint(), CALIB_SECOND);
        ed.pointer_down(&ctx, [0.0, 2.0], Modifiers::default());
        assert!(ed.calibration_ready());
        assert!(ed.apply_calibration(&ctx, 0.0).is_empty());
        let ops = ed.apply_calibration(&ctx, 5.0);
        assert_eq!(ops, vec![EditOp::Meta(json!({ "underlay": { "url": "/assets/a.png", "x": 2.5, "z": 2.5, "width": 25, "aspect": 1.5, "rotation": 0, "opacity": 0.6, "floor3d": false } }))]);
        assert!(ed.calibration().is_none());
        assert_eq!(ed.hint(), SELECT_HINT);
    }

    #[test]
    fn element_touche() {
        let it = |v: Value| PlotItem::from_json(&v);
        let t = 0.1;
        let rect = it(json!({ "kind": "rect", "points": [[0, 0], [2, 2]] }));
        assert!(hit_item(&rect, [1.0, 0.05], t, 70.0));
        assert!(!hit_item(&rect, [1.0, 1.0], t, 70.0), "rectangle vide : par ses bords");
        let filled = it(json!({ "kind": "rect", "points": [[0, 0], [2, 2]], "style": { "fill": "#fff" } }));
        assert!(hit_item(&filled, [1.0, 1.0], t, 70.0));
        assert!(hit_item(&it(json!({ "kind": "ellipse", "points": [[0, 0], [2, 2]] })), [1.0, 1.0], t, 70.0));
        assert!(!hit_item(&it(json!({ "kind": "rect", "points": [[0, 0]] })), [0.0, 0.0], t, 70.0));
        let poly = it(json!({ "kind": "polygon", "points": [[0, 0], [2, 0], [2, 2]] }));
        assert!(hit_item(&poly, [1.5, 0.5], t, 70.0));
        assert!(hit_item(&poly, [1.0, 1.05], t, 70.0), "côté de fermeture");
        let line = it(json!({ "kind": "freehand", "points": [[0, 0], [2, 0], [2, 2]] }));
        assert!(!hit_item(&line, [1.0, 1.0], t, 70.0));
        assert!(hit_item(&line, [1.0, 0.05], t, 70.0));
        // texte de 16 px « Texte » à 70 px/m : 48 px de large, 16 de haut
        let text = it(json!({ "kind": "text", "points": [[0, 0]] }));
        assert!(hit_item(&text, [0.6, -0.1], t, 70.0));
        assert!(!hit_item(&text, [0.6, 0.2], t, 70.0));
    }
}
