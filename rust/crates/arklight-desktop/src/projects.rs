//! Écran des projets (comme `web/js/ui/projects.js`, à la manière de PANDORA) : les projets du
//! dossier des projets (le même que la version JavaScript, même format de fichier) avec leur
//! vignette, les exemples, « Nouveau » et « Ouvrir ». Vignettes enregistrées avec le projet
//! (`thumbnail` : image en data URL), faites d'après la vue 3D.

use crate::{ProjectCard, ProjectRow};
use serde_json::Value;
use slint::{ModelRc, VecModel};
use std::path::{Path, PathBuf};

/// Taille des vignettes (pixels).
pub const THUMB_W: u32 = 320;
pub const THUMB_H: u32 = 180;
/// Cartes par rangée.
const PER_ROW: usize = 4;

thread_local! {
    /// dossier choisi (Fichier › Dossier des projets…), gardé dans les réglages
    static CHOSEN: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Dossier des projets choisi par l'utilisateur (aucun : celui par défaut).
pub fn set_projects_dir(dir: Option<PathBuf>) {
    CHOSEN.with(|c| *c.borrow_mut() = dir);
}

/// Dossier des projets : `ARKLIGHT_PROJECTS_DIR` (essais), celui choisi, sinon
/// Documents\ARKLIGHT\scenes.
pub fn projects_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("ARKLIGHT_PROJECTS_DIR") {
        return PathBuf::from(d);
    }
    if let Some(d) = CHOSEN.with(|c| c.borrow().clone()).filter(|d| d.is_dir()) {
        return d;
    }
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    let candidates = [home.join("Documents").join("ARKLIGHT").join("scenes"), home.join("OneDrive").join("Documents").join("ARKLIGHT").join("scenes")];
    candidates.iter().find(|p| p.is_dir()).cloned().unwrap_or_else(|| candidates[0].clone())
}

/// Un projet du dossier.
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub saved: String,
    pub fixtures: usize,
    pub thumb: Option<String>,
}

/// Projets du dossier, les plus récents d'abord (copie de travail « _… » exclue).
pub fn list(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<(std::time::SystemTime, Entry)> = Vec::new();
    for e in read.flatten() {
        let path = e.path();
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if path.extension().and_then(|x| x.to_str()) != Some("json") || name.starts_with('_') {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        let Some(fixtures) = v.get("fixtures").and_then(Value::as_array) else { continue };
        let mtime = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        let saved = v.get("savedAt").and_then(Value::as_str).map(|s| s.chars().take(16).collect::<String>().replace('T', " ")).unwrap_or_default();
        let thumb = v.get("thumbnail").and_then(Value::as_str).filter(|t| t.starts_with("data:image/")).map(str::to_owned);
        out.push((mtime, Entry { path, name, saved, fixtures: fixtures.len(), thumb }));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|e| e.1).collect()
}

/// Image d'une vignette (JPEG ou PNG en data URL) : décodée par resvg.
pub fn thumb_image(data_url: &str) -> Option<slint::Image> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{THUMB_W}" height="{THUMB_H}"><image width="{THUMB_W}" height="{THUMB_H}" preserveAspectRatio="xMidYMid slice" xlink:href="{data_url}"/></svg>"#
    );
    let rgba = arklight_plot::rasterize(&svg, THUMB_W, THUMB_H);
    (rgba.len() == (THUMB_W * THUMB_H * 4) as usize).then(|| slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(&rgba, THUMB_W, THUMB_H)))
}

/// Rangées de cartes pour l'écran des projets.
pub fn rows(list: &[Entry]) -> Vec<ProjectRow> {
    list.chunks(PER_ROW)
        .enumerate()
        .map(|(r, chunk)| {
            let cards: Vec<ProjectCard> = chunk
                .iter()
                .enumerate()
                .map(|(i, e)| ProjectCard {
                    index: (r * PER_ROW + i) as i32,
                    name: e.name.as_str().into(),
                    sub: format!("{} · {} {}", e.saved, e.fixtures, crate::tr("projecteurs", "fixtures")).into(),
                    has_thumb: e.thumb.is_some(),
                    thumb: e.thumb.as_deref().and_then(thumb_image).unwrap_or_default(),
                })
                .collect();
            ProjectRow { cards: ModelRc::new(VecModel::from(cards)) }
        })
        .collect()
}

/// Vignette d'une image de la vue 3D (pixels RGBA `w` × `h`) : réduite, recadrée en 16/9, PNG en data URL.
pub fn thumbnail(rgba: &[u8], w: u32, h: u32) -> Option<String> {
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    // recadrage au centre en 16/9, puis moyenne des pixels de chaque case
    let (cw, ch) = if w * 9 > h * 16 { (h * 16 / 9, h) } else { (w, w * 9 / 16) };
    let (x0, y0) = ((w - cw) / 2, (h - ch) / 2);
    let mut out = vec![0u8; (THUMB_W * THUMB_H * 4) as usize];
    for ty in 0..THUMB_H {
        for tx in 0..THUMB_W {
            let (sx0, sx1) = (x0 + tx * cw / THUMB_W, x0 + ((tx + 1) * cw / THUMB_W).max(tx * cw / THUMB_W + 1));
            let (sy0, sy1) = (y0 + ty * ch / THUMB_H, y0 + ((ty + 1) * ch / THUMB_H).max(ty * ch / THUMB_H + 1));
            let mut acc = [0u32; 3];
            let mut n = 0u32;
            for y in sy0..sy1.min(h) {
                for x in sx0..sx1.min(w) {
                    let i = ((y * w + x) * 4) as usize;
                    for k in 0..3 {
                        acc[k] += rgba[i + k] as u32;
                    }
                    n += 1;
                }
            }
            let o = ((ty * THUMB_W + tx) * 4) as usize;
            for k in 0..3 {
                out[o + k] = (acc[k] / n.max(1)) as u8;
            }
            out[o + 3] = 255;
        }
    }
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, THUMB_W, THUMB_H);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(&out).ok()?;
    }
    Some(format!("data:image/png;base64,{}", arklight_ai::base64_encode(&png)))
}
