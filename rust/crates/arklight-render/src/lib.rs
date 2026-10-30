//! Moteur 3D d'ARKLIGHT (wgpu). Il dessine la scène dans une texture que l'interface affiche
//! (Slint la montre sans copie : même carte graphique, même file de commandes).
//!
//! Même rendu que la version three.js : image HDR multi-échantillonnée, surfaces comme le
//! MeshStandardMaterial (rugosité, métal), spots et sources ponctuelles en candelas, lumière
//! d'hémisphère, ombres nettes des spots coupés par le grip, rebonds (sources rectangulaires),
//! faces émettrices, toiles transparentes, faisceaux dans la brume ; puis halo des seules sources
//! (UnrealBloomPass sur une image des faces émettrices), exposition, ACES et sRGB.

pub mod grip;
pub mod grip_light;
pub mod mesh;
pub mod models;
pub mod props;
pub mod stage;

use bytemuck::{Pod, Zeroable};
use glam::{EulerRot, Mat4, Quat, Vec3};
use mesh::MeshData;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

pub use glam;
pub use wgpu;

/// Lumières prises en compte par image (les plus fortes d'abord au-delà).
pub const MAX_LIGHTS: usize = 256;
/// Spots à ombre portée (chaque ombre coûte un rendu de la scène : les plus utiles).
pub const MAX_SHADOWS: usize = 6;
/// Rebonds (sources rectangulaires).
pub const MAX_AREA_LIGHTS: usize = 6;
/// Éléments de grip qui coupent un faisceau visible.
pub const MAX_BEAM_OCCLUDERS: usize = 4;
const SHADOW_SIZE: u32 = 1024;
const SAMPLES: u32 = 4;
const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Halo des sources (réglages de la version three.js).
const BLOOM_STRENGTH: f32 = 0.28;
const BLOOM_RADIUS: f32 = 0.12;
const BLOOM_THRESHOLD: f32 = 0.8;
const BLOOM_KNEE: f32 = 0.6;
const BLOOM_KERNELS: [usize; 5] = [6, 10, 14, 18, 22];
/// Plafond des valeurs HDR (projecteur collé à une surface).
const HDR_MAX: f32 = 4000.0;

/// Façon d'éclairer une surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Surface {
    #[default]
    Lit,
    /// couleur telle quelle (MeshBasicMaterial)
    Unlit,
    /// sol avec son quadrillage
    Grid,
    /// facettes visibles (flatShading)
    Flat,
}

/// Matériau (MeshStandardMaterial de three.js).
#[derive(Clone, Copy, Debug)]
pub struct Material {
    /// couleur (sRGB linéaire)
    pub color: [f32; 3],
    pub roughness: f32,
    pub metalness: f32,
    /// gain des faces émettrices (0 : n'émet pas)
    pub emissive_gain: f32,
    /// lumière émise (sRGB linéaire, gain compris)
    pub emissive: [f32; 3],
    pub surface: Surface,
    pub double_sided: bool,
    /// 1 : opaque ; en dessous, transparent (dessiné après, du plus loin au plus proche)
    pub opacity: f32,
    /// écrit la profondeur (un transparent qui ne masque pas ce qui est derrière : non)
    pub depth_write: bool,
    /// fait de l'ombre (spots à ombre portée)
    pub cast_shadow: bool,
    /// reçoit les ombres
    pub receive_shadow: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self { color: [1.0; 3], roughness: 1.0, metalness: 0.0, emissive_gain: 0.0, emissive: [0.0; 3], surface: Surface::Lit, double_sided: false, opacity: 1.0, depth_write: true, cast_shadow: false, receive_shadow: false }
    }
}

impl Material {
    pub fn transparent(&self) -> bool {
        self.opacity < 1.0
    }
}

/// Un maillage placé dans la scène.
#[derive(Clone)]
pub struct Item {
    pub mesh: Arc<MeshData>,
    pub transform: Mat4,
    pub material: Material,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightKind {
    /// cône : bord à `angle` (radians), pénombre vers l'intérieur
    Spot { angle: f32, penumbra: f32 },
    /// source ponctuelle (tubes, ampoules)
    Point,
}

/// Une lumière.
#[derive(Clone, Copy)]
pub struct Light {
    pub position: Vec3,
    pub direction: Vec3,
    /// couleur normalisée (sRGB linéaire)
    pub color: [f32; 3],
    /// intensité en candelas (déjà calibrée)
    pub candela: f32,
    pub kind: LightKind,
    /// ombre portée (choisie par le grip)
    pub shadow: bool,
}

/// Un élément de grip qui coupe un faisceau visible (rectangle : centre, demi-axes, transmission).
#[derive(Clone, Copy, Debug)]
pub struct BeamOccluder {
    pub center: Vec3,
    pub u: Vec3,
    pub v: Vec3,
    pub t: f32,
}

/// Un faisceau visible dans la brume (cône ouvert, sommet à l'origine, axe +Z).
#[derive(Clone)]
pub struct Beam {
    pub mesh: Arc<MeshData>,
    pub transform: Mat4,
    pub color: [f32; 3],
    pub intensity: f32,
    /// demi-angle du cône (radians)
    pub half_angle: f32,
    pub occluders: Vec<BeamOccluder>,
}

/// Rebond : source rectangulaire (diffuse) qui éclaire du côté opposé à son axe −Z.
#[derive(Clone, Copy, Debug)]
pub struct AreaLight {
    pub position: Vec3,
    pub half_width: Vec3,
    pub half_height: Vec3,
    /// couleur × luminance
    pub color: [f32; 3],
}

/// La scène d'une image.
#[derive(Clone, Default)]
pub struct Scene {
    pub items: Vec<Item>,
    pub lights: Vec<Light>,
    pub beams: Vec<Beam>,
    pub areas: Vec<AreaLight>,
    /// encombrement de chaque projecteur et objet (sélection à la souris)
    pub picks: Vec<Pick>,
    /// cibles de « Viser » : personnages, animaux, véhicules (point visé dans le monde)
    pub aims: Vec<Aim>,
    /// lumière ambiante (réglage « ambiance » du projet)
    pub ambient: f32,
    /// exposition : multiplicateur
    pub exposure: f32,
    /// halo des sources
    pub bloom: bool,
    /// surbrillance : projecteurs, pieds et objets soulignés d'un liseré visible dans le noir
    pub highlight: bool,
    /// objectifs des caméras de la scène (voir par la caméra)
    pub lenses: Vec<Lens>,
    /// image de fond du plan au sol
    pub floor_image: Option<FloorImage>,
}

/// Cible de « Viser » (tête d'un personnage, centre d'un véhicule…).
#[derive(Clone, Debug)]
pub struct Aim {
    /// objet de la scène
    pub id: String,
    pub point: Vec3,
    /// un personnage (cible par défaut)
    pub person: bool,
}

/// Objectif d'une caméra de la scène : position et direction de visée (monde).
#[derive(Clone, Debug)]
pub struct Lens {
    pub id: String,
    pub position: Vec3,
    pub forward: Vec3,
}

/// Image de fond du plan posée au sol (`buildUnderlayFloor`) : dessinée sans éclairage, en
/// transparence, juste au-dessus du sol.
#[derive(Clone)]
pub struct FloorImage {
    /// clé de l'image (son adresse) : la texture est refaite quand elle change
    pub key: String,
    /// pixels RGBA (sRGB), `width` × `height`
    pub rgba: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    /// coins dans le monde : haut gauche, haut droit, bas droit, bas gauche de l'image
    pub corners: [Vec3; 4],
    pub opacity: f32,
}

/// Encombrement d'un projecteur ou d'un objet dans le repère du monde (boîte alignée sur les axes).
#[derive(Clone, Debug)]
pub struct Pick {
    pub id: String,
    /// objet (grip, décor) plutôt que projecteur
    pub object: bool,
    pub lo: Vec3,
    pub hi: Vec3,
    /// pièces de l'élément dans `Scene::items` (sélection au maillage près)
    pub items: std::ops::Range<usize>,
}

impl Pick {
    /// Distance le long du rayon (origine, direction) jusqu'à la boîte, s'il la touche.
    pub fn hit(&self, origin: Vec3, dir: Vec3) -> Option<f32> {
        let inv = dir.recip();
        let t1 = (self.lo - origin) * inv;
        let t2 = (self.hi - origin) * inv;
        let near = t1.min(t2).max_element();
        let far = t1.max(t2).min_element();
        (far >= near.max(0.0)).then_some(near.max(0.0))
    }
}

/// Distance le long du rayon jusqu'au premier triangle d'un maillage placé par `transform`.
fn hit_mesh(mesh: &MeshData, transform: &Mat4, origin: Vec3, dir: Vec3) -> Option<f32> {
    // rayon dans le repère du maillage : la distance le long du rayon ne change pas
    let inv = transform.inverse();
    let (o, d) = (inv.transform_point3(origin), inv.transform_vector3(dir));
    let (lo, hi) = mesh.bounds;
    let pad = Vec3::splat(1e-3);
    let probe = Pick { id: String::new(), object: false, lo: lo - pad, hi: hi + pad, items: 0..0 };
    probe.hit(o, d)?;
    let mut best: Option<f32> = None;
    for tri in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| Vec3::from(mesh.vertices[i as usize].pos));
        // Möller–Trumbore, les deux faces
        let (e1, e2) = (b - a, c - a);
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-12 {
            continue;
        }
        let inv_det = 1.0 / det;
        let s = o - a;
        let u = s.dot(p) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            continue;
        }
        let q = s.cross(e1);
        let v = d.dot(q) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            continue;
        }
        let t = e2.dot(q) * inv_det;
        if t > 1e-4 && best.is_none_or(|b| t < b) {
            best = Some(t);
        }
    }
    best
}

/// Le plus proche projecteur ou objet sous un rayon, touché par ses pièces elles-mêmes (comme le
/// `Raycaster` de three.js) et non par sa boîte englobante ; avec la distance le long du rayon.
pub fn pick<'a>(scene: &'a Scene, origin: Vec3, dir: Vec3) -> Option<(&'a Pick, f32)> {
    let mut best: Option<(&Pick, f32)> = None;
    for p in &scene.picks {
        let Some(t_box) = p.hit(origin, dir) else { continue };
        if best.is_some_and(|b| t_box > b.1) {
            continue;
        }
        let t = scene.items[p.items.clone()].iter().filter(|it| it.material.opacity > 0.05).filter_map(|it| hit_mesh(&it.mesh, &it.transform, origin, dir)).min_by(f32::total_cmp);
        if let Some(t) = t
            && best.is_none_or(|b| t < b.1)
        {
            best = Some((p, t));
        }
    }
    best
}

/// Contour d'une boîte (12 arêtes fines, sans éclairage) : la sélection, comme les traits de three.js.
pub fn outline(lo: Vec3, hi: Vec3, color: u32) -> Vec<Item> {
    let (lo, hi) = (lo - Vec3::splat(0.02), hi + Vec3::splat(0.02));
    let size = hi - lo;
    let t = 0.006 * (1.0 + size.max_element() * 0.15);
    let material = Material { color: models::hex(color), surface: Surface::Unlit, ..Material::default() };
    let unit = mesh::boxed(1.0, 1.0, 1.0);
    let mut out = Vec::new();
    let c = (lo + hi) / 2.0;
    for (axis, len) in [(0usize, size.x), (1, size.y), (2, size.z)] {
        for (a, b) in [(0.0f32, 0.0f32), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
            let mut p = c;
            let mut scale = Vec3::splat(t);
            scale[axis] = len + t;
            let (o1, o2) = ((axis + 1) % 3, (axis + 2) % 3);
            p[o1] = if a == 0.0 { lo[o1] } else { hi[o1] };
            p[o2] = if b == 0.0 { lo[o2] } else { hi[o2] };
            out.push(Item { mesh: unit.clone(), transform: Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, p), material });
        }
    }
    out
}

/// Caméra en orbite autour d'un point (comme OrbitControls).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// champ vertical (degrés)
    pub fov: f32,
}

impl Default for Camera {
    fn default() -> Self {
        // vue de la version JavaScript au lancement : œil en (5,5 ; 3,2 ; 7,5), vers le comédien
        Self::preset("persp")
    }
}

impl Camera {
    /// Vues prédéfinies de la version JavaScript (`setView`) : « persp », « front » (de face),
    /// « top » (de dessus), « side » (de côté), toutes vers le comédien.
    pub fn preset(name: &str) -> Self {
        let eye = match name {
            "front" => Vec3::new(0.0, 1.6, 9.0),
            "top" => Vec3::new(0.0, 13.0, 0.01),
            "side" => Vec3::new(9.0, 1.6, 0.0),
            _ => Vec3::new(5.5, 3.2, 7.5),
        };
        let target = Vec3::new(0.0, 1.2, 0.0);
        let d = eye - target;
        Self { target, yaw: d.x.atan2(d.z), pitch: (d.y / d.length()).asin(), distance: d.length(), fov: 50.0 }
    }

    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + Vec3::new(sy * cp, sp, cy * cp) * self.distance
    }

    pub fn view(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov.to_radians(), aspect.max(0.01), 0.05, 200.0)
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }

    /// Rayon (origine, direction) sous un point de la vue (x, y en pixels, taille w × h).
    pub fn ray(&self, x: f32, y: f32, w: f32, h: f32) -> (Vec3, Vec3) {
        let inv = self.view_proj(w / h.max(1.0)).inverse();
        let ndc = glam::Vec2::new(x / w.max(1.0) * 2.0 - 1.0, 1.0 - y / h.max(1.0) * 2.0);
        let near = inv.project_point3(ndc.extend(0.0));
        let far = inv.project_point3(ndc.extend(1.0));
        (near, (far - near).normalize_or(Vec3::NEG_Z))
    }

    /// Tourner autour du point visé (glisser) ; dx, dy en pixels.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.006;
        self.pitch = (self.pitch + dy * 0.006).clamp(-0.05, 1.5);
    }

    /// Décaler la vue (clic droit glissé) ; dx, dy en pixels pour une vue de `height` pixels.
    pub fn pan(&mut self, dx: f32, dy: f32, height: f32) {
        let k = 2.0 * self.distance * (self.fov.to_radians() / 2.0).tan() / height.max(1.0);
        let forward = (self.target - self.eye()).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward);
        self.target += (-right * dx + up * dy) * k;
    }

    /// Molette : rapprocher (> 0) ou éloigner.
    pub fn zoom(&mut self, steps: f32) {
        self.distance = (self.distance * 0.88f32.powf(steps)).clamp(0.5, 120.0);
    }
}

/// Position et angles d'Euler (radians, ordre XYZ comme three.js) → transformation, avec une échelle.
pub fn transform(position: [f64; 3], rotation: [f64; 3], scale: [f32; 3]) -> Mat4 {
    let q = Quat::from_euler(EulerRot::XYZ, rotation[0] as f32, rotation[1] as f32, rotation[2] as f32);
    Mat4::from_scale_rotation_translation(Vec3::from(scale), q, Vec3::new(position[0] as f32, position[1] as f32, position[2] as f32))
}

/// Caméra d'ombre d'un spot (SpotLightShadow de three.js : ouverture 2 × angle, 5 cm à 40 m).
fn shadow_matrix(l: &Light, angle: f32) -> Mat4 {
    let dir = l.direction.normalize_or(Vec3::Z);
    let up = if dir.y.abs() > 0.999 { Vec3::Z } else { Vec3::Y };
    Mat4::perspective_rh((2.0 * angle).min(3.1), 1.0, 0.05, 40.0) * Mat4::look_at_rh(l.position, l.position + dir, up)
}

// ------------------------------------------------------------------ données envoyées à la carte
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    view: [[f32; 4]; 4],
    camera: [f32; 4],
    // ambiance, exposition, nombre de lumières, nombre de rebonds
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuLight {
    pos: [f32; 4],
    dir: [f32; 4],
    color: [f32; 4],
    // x : couche de la carte d'ombre (−1 : sans ombre)
    extra: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuArea {
    corners: [[f32; 4]; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Lighting {
    areas: [GpuArea; MAX_AREA_LIGHTS],
    shadows: [[[f32; 4]; 4]; MAX_SHADOWS],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    model: [[f32; 4]; 4],
    albedo: [f32; 4],
    emissive: [f32; 4],
    // rugosité, métal, opacité, reçoit les ombres
    pbr: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BeamInstance {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BeamData {
    // x : sommet, w : nombre d'éléments
    apex: [f32; 4],
    c: [[f32; 4]; MAX_BEAM_OCCLUDERS],
    u: [[f32; 4]; MAX_BEAM_OCCLUDERS],
    v: [[f32; 4]; MAX_BEAM_OCCLUDERS],
    t: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniform {
    inv_size: [f32; 2],
    direction: [f32; 2],
    params: [f32; 4],
    offsets: [[f32; 4]; 3],
    weights: [[f32; 4]; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CompositeUniform {
    factors: [[f32; 4]; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FinalUniform {
    params: [f32; 4],
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

/// Poids du flou gaussien d'un niveau (paires fusionnées en lectures bilinéaires), comme three.js.
fn blur_kernel(radius: usize) -> (f32, Vec<f32>, Vec<f32>) {
    let sigma = radius as f32 / 3.0;
    let coef: Vec<f32> = (0..radius).map(|i| 0.39894 * (-0.5 * (i * i) as f32 / (sigma * sigma)).exp() / sigma).collect();
    let (mut offsets, mut weights) = (Vec::new(), Vec::new());
    let mut i = 1;
    while i < radius {
        let wa = coef[i];
        let wb = if i + 1 < radius { coef[i + 1] } else { 0.0 };
        let w = wa + wb;
        offsets.push((i as f32 * wa + (i + 1) as f32 * wb) / w);
        weights.push(w);
        i += 2;
    }
    (coef[0], offsets, weights)
}

fn pack(values: &[f32]) -> [[f32; 4]; 3] {
    let mut out = [[0.0; 4]; 3];
    for (i, v) in values.iter().take(12).enumerate() {
        out[i / 4][i % 4] = *v;
    }
    out
}

/// `Math.round` (moitié vers le haut) d'une moitié de taille, au moins 1 pixel.
fn half(v: u32) -> u32 {
    (v as f32 / 2.0).round().max(1.0) as u32
}

fn v4(v: Vec3, w: f32) -> [f32; 4] {
    [v.x, v.y, v.z, w]
}

// ------------------------------------------------------------------ cibles à la taille de la vue
struct Targets {
    size: (u32, u32),
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    hdr: wgpu::TextureView,
    sources: wgpu::TextureView,
    sources_depth: wgpu::TextureView,
    bright: wgpu::TextureView,
    blur_h: Vec<wgpu::TextureView>,
    blur_v: Vec<wgpu::TextureView>,
    highpass_bg: wgpu::BindGroup,
    blur_bg: Vec<[wgpu::BindGroup; 2]>,
    composite_bg: wgpu::BindGroup,
    final_bg: wgpu::BindGroup,
    /// textures affichées tour à tour (l'interface montre l'une pendant qu'on dessine l'autre)
    shown: [wgpu::Texture; 2],
    next: usize,
}

/// Image au sol envoyée à la carte graphique (texture, sommets).
struct DecalGpu {
    key: String,
    bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DecalVertex {
    pos: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DecalUniform {
    view_proj: [[f32; 4]; 4],
    /// x : opacité
    params: [f32; 4],
}

pub struct Renderer {
    decal_pipeline: wgpu::RenderPipeline,
    decal_layout: wgpu::BindGroupLayout,
    decal_uniform: wgpu::Buffer,
    decal_vertices: wgpu::Buffer,
    decal: Option<DecalGpu>,
    scene_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    sources_pipeline: wgpu::RenderPipeline,
    beam_pipeline: wgpu::RenderPipeline,
    highlight_pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    highpass_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    final_pipeline: wgpu::RenderPipeline,
    scene_layout: wgpu::BindGroupLayout,
    post_layout: wgpu::BindGroupLayout,
    composite_layout: wgpu::BindGroupLayout,
    final_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    shadow_sampler: wgpu::Sampler,
    shadow_maps: wgpu::TextureView,
    shadow_layers: Vec<wgpu::TextureView>,
    shadow_globals: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    globals: wgpu::Buffer,
    lights: wgpu::Buffer,
    lighting: wgpu::Buffer,
    beam_data: wgpu::Buffer,
    beam_data_capacity: usize,
    bind_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instance_capacity: usize,
    beam_instances: wgpu::Buffer,
    beam_capacity: usize,
    composite_uniform: wgpu::Buffer,
    final_uniform: wgpu::Buffer,
    meshes: HashMap<u64, GpuMesh>,
    targets: Option<Targets>,
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }
}

fn storage_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
        count: None,
    }
}

fn tv(binding: u32, v: &wgpu::TextureView) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource: wgpu::BindingResource::TextureView(v) }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry { binding, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None }
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("arklight scène"), source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) });
        let post = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("arklight post"), source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()) });
        let shadow = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("arklight ombres"), source: wgpu::ShaderSource::Wgsl(include_str!("shadow.wgsl").into()) });
        let uniform = |label: &str, size: usize| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: size as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let globals = uniform("arklight globals", std::mem::size_of::<Globals>());
        let lighting = uniform("arklight rebonds et ombres", std::mem::size_of::<Lighting>());
        let lights = device.create_buffer(&wgpu::BufferDescriptor { label: Some("arklight lumières"), size: (std::mem::size_of::<GpuLight>() * MAX_LIGHTS) as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let beam_data_capacity = 64;
        let beam_data = Self::storage(device, std::mem::size_of::<BeamData>() * beam_data_capacity);

        // ---- cartes d'ombre (une couche par spot à ombre portée)
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("arklight ombres"),
            size: wgpu::Extent3d { width: SHADOW_SIZE, height: SHADOW_SIZE, depth_or_array_layers: MAX_SHADOWS as u32 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_maps = shadow_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let shadow_layers: Vec<wgpu::TextureView> = (0..MAX_SHADOWS as u32)
            .map(|i| shadow_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2), base_array_layer: i, array_layer_count: Some(1), ..Default::default() }))
            .collect();
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("arklight ombres"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("arklight globals"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                storage_entry(1),
                uniform_entry(2, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 4, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
                storage_entry(5),
            ],
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("arklight caméra d'ombre"), entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX)] });
        let shadow_globals = (0..MAX_SHADOWS)
            .map(|_| {
                let buf = uniform("arklight caméra d'ombre", 64);
                let bg = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("arklight caméra d'ombre"), layout: &shadow_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: buf.as_entire_binding() }] });
                (buf, bg)
            })
            .collect();

        let scene_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("arklight scène"), bind_group_layouts: &[Some(&scene_layout)], ..Default::default() });
        let shadow_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("arklight ombres"), bind_group_layouts: &[Some(&shadow_layout)], ..Default::default() });
        let vertex_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let instance_attrs = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4];
        let beam_attrs = wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4];
        let vertex_layout = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<mesh::Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &vertex_attrs };
        let instance_layout = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Instance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &instance_attrs };
        let beam_layout = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<BeamInstance>() as u64, step_mode: wgpu::VertexStepMode::Instance, attributes: &beam_attrs };
        let depth = |write: bool| Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: Some(write), depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: Default::default() });
        let alpha_blend = wgpu::BlendState::ALPHA_BLENDING;
        let mesh_pipeline = |label: &str, fs: &str, samples: u32, blend: Option<wgpu::BlendState>, write: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&scene_pl),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[Some(vertex_layout.clone()), Some(instance_layout.clone())] },
                fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: HDR_FORMAT, blend, write_mask: wgpu::ColorWrites::ALL })] }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: depth(write),
                multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
                multiview_mask: None,
                cache: None,
            })
        };
        let scene_pipeline = mesh_pipeline("arklight scène", "fs_main", SAMPLES, None, true);
        let transparent_pipeline = mesh_pipeline("arklight transparents", "fs_main", SAMPLES, Some(alpha_blend), false);
        let sources_pipeline = mesh_pipeline("arklight sources", "fs_sources", 1, None, true);
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
        };
        let beam_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("arklight faisceaux"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_beam"), compilation_options: Default::default(), buffers: &[Some(vertex_layout.clone()), Some(beam_layout)] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_beam"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: HDR_FORMAT, blend: Some(additive), write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: depth(false),
            multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
            multiview_mask: None,
            cache: None,
        });
        // surbrillance (`HighlightShader` de la version JavaScript) : par-dessus tout, en addition
        let highlight_blend = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Zero, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
        };
        let highlight_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("arklight surbrillance"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[Some(vertex_layout.clone()), Some(instance_layout.clone())] },
            fragment: Some(wgpu::FragmentState { module: &shader, entry_point: Some("fs_highlight"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: HDR_FORMAT, blend: Some(highlight_blend), write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: Some(false), depth_compare: Some(wgpu::CompareFunction::Always), stencil: Default::default(), bias: Default::default() }),
            multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
            multiview_mask: None,
            cache: None,
        });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("arklight ombres"),
            layout: Some(&shadow_pl),
            vertex: wgpu::VertexState { module: &shadow, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &[Some(vertex_layout.clone()), Some(instance_layout.clone())] },
            fragment: None,
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: depth(true),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        // ---- passes plein écran
        let post_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("arklight flou"), entries: &[texture_entry(0), sampler_entry(1), uniform_entry(2, wgpu::ShaderStages::FRAGMENT)] });
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("arklight halo"),
            entries: &[texture_entry(0), texture_entry(1), texture_entry(2), texture_entry(3), texture_entry(4), sampler_entry(5), uniform_entry(6, wgpu::ShaderStages::FRAGMENT)],
        });
        let final_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("arklight image"), entries: &[texture_entry(0), texture_entry(1), sampler_entry(2), uniform_entry(3, wgpu::ShaderStages::FRAGMENT)] });
        let full = |label: &str, fs: &str, layout: &wgpu::BindGroupLayout, format: wgpu::TextureFormat| {
            let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some(label), bind_group_layouts: &[Some(layout)], ..Default::default() });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                vertex: wgpu::VertexState { module: &post, entry_point: Some("vs_full"), compilation_options: Default::default(), buffers: &[] },
                fragment: Some(wgpu::FragmentState { module: &post, entry_point: Some(fs), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let highpass_pipeline = full("arklight seuil du halo", "fs_highpass", &post_layout, HDR_FORMAT);
        let blur_pipeline = full("arklight flou", "fs_blur", &post_layout, HDR_FORMAT);
        let composite_pipeline = full("arklight halo", "fs_composite", &composite_layout, HDR_FORMAT);
        let final_pipeline = full("arklight image finale", "fs_final", &final_layout, COLOR_FORMAT);
        // image de fond au sol : texture sans éclairage, en transparence, sans écrire la profondeur
        let decal_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("arklight image au sol"), source: wgpu::ShaderSource::Wgsl(include_str!("decal.wgsl").into()) });
        let decal_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("arklight image au sol"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                sampler_entry(2),
            ],
        });
        let decal_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("arklight image au sol"), bind_group_layouts: &[Some(&decal_layout)], ..Default::default() });
        let decal_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2];
        let decal_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("arklight image au sol"),
            layout: Some(&decal_pl),
            vertex: wgpu::VertexState {
                module: &decal_shader,
                entry_point: Some("vs_decal"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<DecalVertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &decal_attrs })],
            },
            fragment: Some(wgpu::FragmentState { module: &decal_shader, entry_point: Some("fs_decal"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: HDR_FORMAT, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: Some(false), depth_compare: Some(wgpu::CompareFunction::LessEqual), stencil: Default::default(), bias: Default::default() }),
            multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
            multiview_mask: None,
            cache: None,
        });
        let decal_uniform = uniform("arklight image au sol", std::mem::size_of::<DecalUniform>());
        let decal_vertices = Self::buffer(device, "arklight image au sol", std::mem::size_of::<DecalVertex>() * 6);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("arklight linéaire"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        // assemblage des niveaux du halo : force × facteur (rayon : facteurs rapprochés de 1,2 − f)
        let factors: Vec<f32> = [1.0f32, 0.8, 0.6, 0.4, 0.2].iter().map(|f| BLOOM_STRENGTH * (f + (1.2 - 2.0 * f) * BLOOM_RADIUS)).collect();
        let composite = CompositeUniform { factors: [[factors[0], factors[1], factors[2], factors[3]], [factors[4], 0.0, 0.0, 0.0]] };
        let composite_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("arklight halo"), contents: bytemuck::bytes_of(&composite), usage: wgpu::BufferUsages::UNIFORM });
        let final_uniform = uniform("arklight image", std::mem::size_of::<FinalUniform>());
        let bind_group = Self::scene_bind_group(device, &scene_layout, &globals, &lights, &lighting, &shadow_maps, &shadow_sampler, &beam_data);

        Self {
            decal_pipeline,
            decal_layout,
            decal_uniform,
            decal_vertices,
            decal: None,
            scene_pipeline,
            transparent_pipeline,
            sources_pipeline,
            beam_pipeline,
            highlight_pipeline,
            shadow_pipeline,
            highpass_pipeline,
            blur_pipeline,
            composite_pipeline,
            final_pipeline,
            scene_layout,
            post_layout,
            composite_layout,
            final_layout,
            sampler,
            shadow_sampler,
            shadow_maps,
            shadow_layers,
            shadow_globals,
            globals,
            lights,
            lighting,
            beam_data,
            beam_data_capacity,
            bind_group,
            instances: Self::buffer(device, "arklight instances", std::mem::size_of::<Instance>() * 1024),
            instance_capacity: 1024,
            beam_instances: Self::buffer(device, "arklight faisceaux", std::mem::size_of::<BeamInstance>() * 64),
            beam_capacity: 64,
            composite_uniform,
            final_uniform,
            meshes: HashMap::new(),
            targets: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn scene_bind_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, globals: &wgpu::Buffer, lights: &wgpu::Buffer, lighting: &wgpu::Buffer, shadows: &wgpu::TextureView, sampler: &wgpu::Sampler, beams: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arklight globals"),
            layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: lights.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: lighting.as_entire_binding() },
                tv(3, shadows),
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: beams.as_entire_binding() },
            ],
        })
    }

    fn buffer(device: &wgpu::Device, label: &str, size: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: size as u64, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }

    fn storage(device: &wgpu::Device, size: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor { label: Some("arklight faisceaux coupés"), size: size as u64, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }

    fn targets(&self, device: &wgpu::Device, (w, h): (u32, u32)) -> Targets {
        let tex = |label: &str, (w, h): (u32, u32), format, samples, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let sampled = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let view = |t: wgpu::Texture| t.create_view(&Default::default());
        let size = (w, h);
        let msaa = view(tex("arklight hdr msaa", size, HDR_FORMAT, SAMPLES, attach));
        let depth = view(tex("arklight profondeur", size, DEPTH_FORMAT, SAMPLES, attach));
        let hdr = view(tex("arklight hdr", size, HDR_FORMAT, 1, sampled));
        let sources = view(tex("arklight sources", size, HDR_FORMAT, 1, sampled));
        let sources_depth = view(tex("arklight profondeur des sources", size, DEPTH_FORMAT, 1, attach));
        // halo : demi-taille, puis chaque niveau moitié du précédent
        let mut level = (half(w), half(h));
        let bright = view(tex("arklight halo seuil", level, HDR_FORMAT, 1, sampled));
        let (mut blur_h, mut blur_v, mut sizes) = (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..BLOOM_KERNELS.len() {
            blur_h.push(view(tex("arklight halo h", level, HDR_FORMAT, 1, sampled)));
            blur_v.push(view(tex("arklight halo v", level, HDR_FORMAT, 1, sampled)));
            sizes.push(level);
            level = (half(level.0), half(level.1));
        }
        let post_bg = |src: &wgpu::TextureView, u: &BlurUniform| {
            let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("arklight flou"), contents: bytemuck::bytes_of(u), usage: wgpu::BufferUsages::UNIFORM });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("arklight flou"),
                layout: &self.post_layout,
                entries: &[tv(0, src), wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) }, wgpu::BindGroupEntry { binding: 2, resource: buf.as_entire_binding() }],
            })
        };
        let highpass = BlurUniform { inv_size: [0.0; 2], direction: [0.0; 2], params: [0.0, 0.0, BLOOM_THRESHOLD, BLOOM_KNEE], offsets: [[0.0; 4]; 3], weights: [[0.0; 4]; 3] };
        let highpass_bg = post_bg(&sources, &highpass);
        let blur_bg = (0..BLOOM_KERNELS.len())
            .map(|i| {
                let (center, offsets, weights) = blur_kernel(BLOOM_KERNELS[i]);
                let (lw, lh) = sizes[i];
                let u = |direction: [f32; 2]| BlurUniform { inv_size: [1.0 / lw as f32, 1.0 / lh as f32], direction, params: [center, offsets.len() as f32, 0.0, 0.0], offsets: pack(&offsets), weights: pack(&weights) };
                let input = if i == 0 { &bright } else { &blur_v[i - 1] };
                [post_bg(input, &u([1.0, 0.0])), post_bg(&blur_h[i], &u([0.0, 1.0]))]
            })
            .collect();
        let composite_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arklight halo"),
            layout: &self.composite_layout,
            entries: &[
                tv(0, &blur_v[0]),
                tv(1, &blur_v[1]),
                tv(2, &blur_v[2]),
                tv(3, &blur_v[3]),
                tv(4, &blur_v[4]),
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 6, resource: self.composite_uniform.as_entire_binding() },
            ],
        });
        let final_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("arklight image"),
            layout: &self.final_layout,
            entries: &[tv(0, &hdr), tv(1, &blur_h[0]), wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) }, wgpu::BindGroupEntry { binding: 3, resource: self.final_uniform.as_entire_binding() }],
        });
        // COPY_SRC : capture de la vue (exports, vignettes des projets)
        let shown_usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC;
        let shown = [tex("arklight vue A", size, COLOR_FORMAT, 1, shown_usage), tex("arklight vue B", size, COLOR_FORMAT, 1, shown_usage)];
        Targets { size, msaa, depth, hdr, sources, sources_depth, bright, blur_h, blur_v, highpass_bg, blur_bg, composite_bg, final_bg, shown, next: 0 }
    }

    fn upload(&mut self, device: &wgpu::Device, m: &MeshData) {
        self.meshes.entry(m.id).or_insert_with(|| GpuMesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("arklight maillage"), contents: bytemuck::cast_slice(&m.vertices), usage: wgpu::BufferUsages::VERTEX }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("arklight maillage"), contents: bytemuck::cast_slice(&m.indices), usage: wgpu::BufferUsages::INDEX }),
            count: m.indices.len() as u32,
        });
    }

    /// Dessine une image de `size` pixels et rend la texture à afficher.
    pub fn render(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, size: (u32, u32), scene: &Scene, camera: &Camera) -> wgpu::Texture {
        let size = (size.0.clamp(1, 8192), size.1.clamp(1, 8192));
        if self.targets.as_ref().map(|t| t.size) != Some(size) {
            self.targets = Some(self.targets(device, size));
        }

        // ---- lumières : les plus fortes d'abord si la scène en a trop ; ombres des spots choisis
        let strength = |l: &Light| l.candela * l.color.iter().cloned().fold(0.0, f32::max);
        let mut lights: Vec<&Light> = scene.lights.iter().filter(|l| strength(l) > 0.0).collect();
        lights.sort_by(|a, b| strength(b).total_cmp(&strength(a)));
        lights.truncate(MAX_LIGHTS);
        let mut lighting = Lighting::zeroed();
        let mut shadow_casters: Vec<Mat4> = Vec::new();
        let gpu_lights: Vec<GpuLight> = lights
            .iter()
            .map(|l| {
                let d = l.direction.normalize_or(Vec3::Z);
                let (kind, outer, inner) = match l.kind {
                    // cône comme le SpotLight de three.js : bord à `angle`, pénombre vers l'intérieur
                    LightKind::Spot { angle, penumbra } => (0.0, angle.cos(), (angle * (1.0 - penumbra.clamp(0.0, 1.0))).cos()),
                    LightKind::Point => (1.0, -1.0, -1.0),
                };
                let mut layer = -1.0;
                if let (true, LightKind::Spot { angle, .. }) = (l.shadow, l.kind)
                    && shadow_casters.len() < MAX_SHADOWS
                {
                    layer = shadow_casters.len() as f32;
                    let m = shadow_matrix(l, angle);
                    lighting.shadows[shadow_casters.len()] = m.to_cols_array_2d();
                    shadow_casters.push(m);
                }
                let c = l.color.map(|c| c * l.candela);
                GpuLight { pos: v4(l.position, kind), dir: v4(d, outer), color: [c[0], c[1], c[2], inner], extra: [layer, 0.0, 0.0, 0.0] }
            })
            .collect();
        if !gpu_lights.is_empty() {
            queue.write_buffer(&self.lights, 0, bytemuck::cast_slice(&gpu_lights));
        }
        // rebonds : coins du rectangle comme la RectAreaLight de three.js
        let areas = &scene.areas[..scene.areas.len().min(MAX_AREA_LIGHTS)];
        for (i, a) in areas.iter().enumerate() {
            let (p, hw, hh) = (a.position, a.half_width, a.half_height);
            lighting.areas[i] = GpuArea { corners: [v4(p + hw - hh, 0.0), v4(p - hw - hh, 0.0), v4(p - hw + hh, 0.0), v4(p + hw + hh, 0.0)], color: [a.color[0], a.color[1], a.color[2], 0.0] };
        }
        queue.write_buffer(&self.lighting, 0, bytemuck::bytes_of(&lighting));
        for (i, m) in shadow_casters.iter().enumerate() {
            queue.write_buffer(&self.shadow_globals[i].0, 0, bytemuck::bytes_of(&m.to_cols_array_2d()));
        }
        let eye = camera.eye();
        let aspect = size.0 as f32 / size.1 as f32;
        let globals = Globals {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            view: camera.view().to_cols_array_2d(),
            camera: [eye.x, eye.y, eye.z, 1.0],
            params: [scene.ambient, scene.exposure, gpu_lights.len() as f32, areas.len() as f32],
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        // ---- instances : opaques rangés par maillage, puis ceux qui font de l'ombre, puis les
        // transparents du plus loin au plus proche
        let instance = |it: &Item| {
            let m = &it.material;
            let surface = match m.surface {
                Surface::Lit => 0.0,
                Surface::Unlit => 1.0,
                Surface::Grid => 2.0,
                Surface::Flat => 3.0,
            };
            Instance {
                model: it.transform.to_cols_array_2d(),
                albedo: [m.color[0], m.color[1], m.color[2], surface],
                emissive: [m.emissive[0], m.emissive[1], m.emissive[2], if m.double_sided { 1.0 } else { 0.0 }],
                pbr: [m.roughness, m.metalness, m.opacity, if m.receive_shadow { 1.0 } else { 0.0 }],
            }
        };
        let mut instances = Vec::with_capacity(scene.items.len() + 16);
        let batch = |list: Vec<&Item>, instances: &mut Vec<Instance>| {
            let mut ranges: Vec<(u64, std::ops::Range<u32>)> = Vec::new();
            for it in list {
                let i = instances.len() as u32;
                instances.push(instance(it));
                match ranges.last_mut() {
                    Some((id, r)) if *id == it.mesh.id => r.end = i + 1,
                    _ => ranges.push((it.mesh.id, i..i + 1)),
                }
            }
            ranges
        };
        for it in &scene.items {
            self.upload(device, &it.mesh);
        }
        let mut opaque: Vec<&Item> = scene.items.iter().filter(|it| !it.material.transparent()).collect();
        opaque.sort_by_key(|it| it.mesh.id);
        let opaque_ranges = batch(opaque, &mut instances);
        let mut casters: Vec<&Item> = scene.items.iter().filter(|it| it.material.cast_shadow).collect();
        casters.sort_by_key(|it| it.mesh.id);
        let caster_ranges = if shadow_casters.is_empty() { Vec::new() } else { batch(casters, &mut instances) };
        let mut transparent: Vec<&Item> = scene.items.iter().filter(|it| it.material.transparent()).collect();
        let depth_of = |it: &Item| it.transform.transform_point3(Vec3::ZERO).distance_squared(eye);
        transparent.sort_by(|a, b| depth_of(b).total_cmp(&depth_of(a)));
        let transparent_draws: Vec<(u64, u32)> = transparent
            .iter()
            .map(|it| {
                instances.push(instance(it));
                (it.mesh.id, instances.len() as u32 - 1)
            })
            .collect();
        // surbrillance : les pièces des projecteurs et des objets (pas les faisceaux)
        let highlight_draws: Vec<(u64, u32)> = if scene.highlight {
            scene
                .picks
                .iter()
                .flat_map(|p| scene.items[p.items.clone()].iter())
                .map(|it| {
                    instances.push(instance(it));
                    (it.mesh.id, instances.len() as u32 - 1)
                })
                .collect()
        } else {
            Vec::new()
        };
        let peak = scene.items.iter().filter(|it| !it.material.transparent()).map(|it| it.material.emissive.iter().cloned().fold(0.0, f32::max) * scene.exposure).fold(0.0f32, f32::max);
        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instances = Self::buffer(device, "arklight instances", std::mem::size_of::<Instance>() * self.instance_capacity);
        }
        if !instances.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        }

        // ---- faisceaux et éléments de grip qui les coupent
        let beams: Vec<&Beam> = scene.beams.iter().filter(|b| b.intensity > 0.0).collect();
        let beam_instances: Vec<BeamInstance> = beams.iter().map(|b| BeamInstance { model: b.transform.to_cols_array_2d(), color: [b.color[0], b.color[1], b.color[2], b.intensity] }).collect();
        let beam_data: Vec<BeamData> = beams
            .iter()
            .map(|b| {
                let mut d = BeamData::zeroed();
                let n = b.occluders.len().min(MAX_BEAM_OCCLUDERS);
                d.apex = v4(b.transform.transform_point3(Vec3::ZERO), n as f32);
                for (i, o) in b.occluders.iter().take(n).enumerate() {
                    d.c[i] = v4(o.center, 0.0);
                    d.u[i] = v4(o.u, 0.0);
                    d.v[i] = v4(o.v, 0.0);
                    d.t[i] = o.t;
                }
                d
            })
            .collect();
        for b in &beams {
            self.upload(device, &b.mesh);
        }
        if beam_instances.len() > self.beam_capacity {
            self.beam_capacity = beam_instances.len().next_power_of_two();
            self.beam_instances = Self::buffer(device, "arklight faisceaux", std::mem::size_of::<BeamInstance>() * self.beam_capacity);
        }
        if beam_data.len() > self.beam_data_capacity {
            self.beam_data_capacity = beam_data.len().next_power_of_two();
            self.beam_data = Self::storage(device, std::mem::size_of::<BeamData>() * self.beam_data_capacity);
            self.bind_group = Self::scene_bind_group(device, &self.scene_layout, &self.globals, &self.lights, &self.lighting, &self.shadow_maps, &self.shadow_sampler, &self.beam_data);
        }
        if !beam_instances.is_empty() {
            queue.write_buffer(&self.beam_instances, 0, bytemuck::cast_slice(&beam_instances));
            queue.write_buffer(&self.beam_data, 0, bytemuck::cast_slice(&beam_data));
        }
        let bloom = scene.bloom && peak > BLOOM_THRESHOLD;
        queue.write_buffer(&self.final_uniform, 0, bytemuck::bytes_of(&FinalUniform { params: [scene.exposure, if bloom { 1.0 } else { 0.0 }, HDR_MAX, 0.0] }));

        let targets = self.targets.as_mut().expect("cibles créées plus haut");
        let shown = targets.shown[targets.next].clone();
        targets.next = 1 - targets.next;
        let targets = self.targets.as_ref().expect("cibles");
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("arklight image") });
        let draw_ranges = |pass: &mut wgpu::RenderPass, ranges: &[(u64, std::ops::Range<u32>)]| {
            pass.set_vertex_buffer(1, self.instances.slice(..));
            for (id, range) in ranges {
                let mesh = &self.meshes[id];
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.count, 0, range.clone());
            }
        };

        // ---- cartes d'ombre des spots choisis
        for (i, _) in shadow_casters.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arklight ombre"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.shadow_layers[i], depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                ..Default::default()
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, &self.shadow_globals[i].1, &[]);
            draw_ranges(&mut pass, &caster_ranges);
        }

        // ---- image de fond au sol : texture refaite quand l'image change ; coins et opacité à chaque image
        let decal_on = match &scene.floor_image {
            Some(img) if img.width > 0 && img.height > 0 && img.rgba.len() == (img.width * img.height * 4) as usize => {
                if self.decal.as_ref().map(|d| d.key.as_str()) != Some(img.key.as_str()) {
                    let extent = wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 };
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("arklight image au sol"),
                        size: extent,
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                        &img.rgba,
                        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * img.width), rows_per_image: Some(img.height) },
                        extent,
                    );
                    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("arklight image au sol"),
                        layout: &self.decal_layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: self.decal_uniform.as_entire_binding() },
                            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                        ],
                    });
                    self.decal = Some(DecalGpu { key: img.key.clone(), bind_group });
                }
                let c = img.corners;
                let v = |i: usize, uv: [f32; 2]| DecalVertex { pos: c[i].to_array(), uv };
                let quad = [v(0, [0.0, 0.0]), v(1, [1.0, 0.0]), v(2, [1.0, 1.0]), v(0, [0.0, 0.0]), v(2, [1.0, 1.0]), v(3, [0.0, 1.0])];
                queue.write_buffer(&self.decal_vertices, 0, bytemuck::cast_slice(&quad));
                let u = DecalUniform { view_proj: camera.view_proj(size.0 as f32 / size.1 as f32).to_cols_array_2d(), params: [img.opacity.clamp(0.0, 1.0), 0.0, 0.0, 0.0] };
                queue.write_buffer(&self.decal_uniform, 0, bytemuck::bytes_of(&u));
                true
            }
            _ => false,
        };

        // ---- scène HDR (4 échantillons) : opaques, transparents, faisceaux
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arklight scène"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.msaa,
                    depth_slice: None,
                    resolve_target: Some(&targets.hdr),
                    // fond 0x040506 (sRGB) en linéaire
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0012, g: 0.0015, b: 0.0018, a: 1.0 }), store: wgpu::StoreOp::Discard },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &targets.depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.scene_pipeline);
            draw_ranges(&mut pass, &opaque_ranges);
            if decal_on && let Some(d) = &self.decal {
                pass.set_pipeline(&self.decal_pipeline);
                pass.set_bind_group(0, &d.bind_group, &[]);
                pass.set_vertex_buffer(0, self.decal_vertices.slice(..));
                pass.draw(0..6, 0..1);
                pass.set_bind_group(0, &self.bind_group, &[]);
            }
            if !transparent_draws.is_empty() {
                pass.set_pipeline(&self.transparent_pipeline);
                for (id, i) in &transparent_draws {
                    let mesh = &self.meshes[id];
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.count, 0, *i..*i + 1);
                }
            }
            if !highlight_draws.is_empty() {
                pass.set_pipeline(&self.highlight_pipeline);
                for (id, i) in &highlight_draws {
                    let mesh = &self.meshes[id];
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.count, 0, *i..*i + 1);
                }
            }
            if !beams.is_empty() {
                pass.set_pipeline(&self.beam_pipeline);
                pass.set_vertex_buffer(1, self.beam_instances.slice(..));
                for (i, b) in beams.iter().enumerate() {
                    let mesh = &self.meshes[&b.mesh.id];
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.count, 0, i as u32..i as u32 + 1);
                }
            }
        }

        // ---- halo des sources (faces émettrices seules ; le reste opaque les masque)
        if bloom {
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("arklight sources"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &targets.sources, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &targets.sources_depth, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }), stencil_ops: None }),
                    ..Default::default()
                });
                pass.set_pipeline(&self.sources_pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                draw_ranges(&mut pass, &opaque_ranges);
            }
            let full = |encoder: &mut wgpu::CommandEncoder, label: &str, pipeline: &wgpu::RenderPipeline, bg: &wgpu::BindGroup, target: &wgpu::TextureView| {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some(label),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                    ..Default::default()
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, bg, &[]);
                pass.draw(0..3, 0..1);
            };
            full(&mut encoder, "arklight seuil du halo", &self.highpass_pipeline, &targets.highpass_bg, &targets.bright);
            for i in 0..BLOOM_KERNELS.len() {
                full(&mut encoder, "arklight flou h", &self.blur_pipeline, &targets.blur_bg[i][0], &targets.blur_h[i]);
                full(&mut encoder, "arklight flou v", &self.blur_pipeline, &targets.blur_bg[i][1], &targets.blur_v[i]);
            }
            full(&mut encoder, "arklight halo", &self.composite_pipeline, &targets.composite_bg, &targets.blur_h[0]);
        }

        // ---- image finale : exposition, halo, ACES, sRGB
        {
            let view = shown.create_view(&Default::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("arklight image finale"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                ..Default::default()
            });
            pass.set_pipeline(&self.final_pipeline);
            pass.set_bind_group(0, &targets.final_bg, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rayon_et_choix_a_la_souris() {
        let cam = Camera::default();
        // le centre de la vue vise le point visé par la caméra
        let (o, d) = cam.ray(400.0, 300.0, 800.0, 600.0);
        let to_target = (cam.target - o).normalize();
        assert!(d.dot(to_target) > 0.9999, "{d:?} / {to_target:?}");
        // le plus proche de deux éléments alignés est choisi, touché par ses pièces
        let mut scene = Scene::default();
        let cube = |c: Vec3, scene: &mut Scene, id: &str, object: bool| {
            let start = scene.items.len();
            scene.items.push(Item { mesh: mesh::boxed(0.4, 0.4, 0.4), transform: Mat4::from_translation(c), material: Material::default() });
            scene.picks.push(Pick { id: id.into(), object, lo: c - Vec3::splat(0.2), hi: c + Vec3::splat(0.2), items: start..scene.items.len() });
        };
        cube(cam.target, &mut scene, "loin", true);
        cube(cam.target + d * -2.0, &mut scene, "proche", false);
        assert_eq!(pick(&scene, o, d).map(|p| p.0.id.as_str()), Some("proche"));
        // un rayon à côté ne touche rien
        let (o2, d2) = cam.ray(5.0, 5.0, 800.0, 600.0);
        assert!(pick(&scene, o2, d2).is_none());
        // dans la boîte englobante mais à côté de la pièce (un pied fin dans une grande boîte) : rien
        let mut thin = Scene::default();
        thin.items.push(Item { mesh: mesh::boxed(0.04, 0.04, 0.04), transform: Mat4::from_translation(cam.target + Vec3::new(0.4, 0.0, 0.0)), material: Material::default() });
        thin.picks.push(Pick { id: "pied".into(), object: true, lo: cam.target - Vec3::splat(0.5), hi: cam.target + Vec3::splat(0.5), items: 0..1 });
        assert!(thin.picks[0].hit(o, d).is_some());
        assert!(pick(&thin, o, d).is_none());
        assert_eq!(outline(Vec3::ZERO, Vec3::ONE, 0x4d8dff).len(), 12);
    }
}

/// Pixels RGBA (8 bits, ligne par ligne) d'une image rendue (`Renderer::render`) : capture de la
/// vue, vignettes des projets. Attend la carte graphique.
pub fn read_rgba(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Option<Vec<u8>> {
    let (width, height) = (texture.width(), texture.height());
    // lignes alignées sur 256 octets
    let row = (width * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some("relecture"), size: (row * height) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(height) } },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit(Some(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r.is_ok());
    });
    device.poll(wgpu::PollType::wait_indefinitely()).ok()?;
    if !rx.recv().unwrap_or(false) {
        return None;
    }
    let data = buffer.slice(..).get_mapped_range().ok()?;
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let start = (y * row) as usize;
        pixels.extend_from_slice(&data[start..start + (width * 4) as usize]);
    }
    Some(pixels)
}
