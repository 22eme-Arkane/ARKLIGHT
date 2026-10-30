//! Sorties de tous les projecteurs d'un plan, recalculées à chaque image avec l'état d'exécution
//! de chacun (départ des effets, filament, verrouillage d'entrée…).

use super::{Ctx, Runtime, Source, compute_output};
use crate::color::Rgb;
use crate::library::library;
use crate::project::Fixture;
use std::collections::HashMap;

/// Sortie d'un projecteur.
#[derive(Debug, Clone)]
pub struct Light {
    /// couleur de chaque pixel (sRGB linéaire, niveau compris) : pixels × 3
    pub pixels: Vec<f64>,
    pub source: Source,
    pub runtime: Runtime,
}

impl Light {
    /// Couleur moyenne des pixels.
    pub fn average(&self) -> Rgb {
        let n = (self.pixels.len() / 3).max(1) as f64;
        let mut c = [0.0; 3];
        for px in self.pixels.chunks_exact(3) {
            for k in 0..3 {
                c[k] += px[k];
            }
        }
        c.map(|v| v / n)
    }
}

#[derive(Debug, Default)]
pub struct Lights {
    lights: HashMap<String, Light>,
}

impl Lights {
    /// Calcule la sortie de chaque projecteur ; vrai si une sortie a changé depuis l'image précédente.
    pub fn update(&mut self, fixtures: &[Fixture], ctx: &Ctx) -> bool {
        self.update_with(fixtures, ctx, |_, _, _| {})
    }

    /// Comme `update`, puis `post` peut changer la sortie de chaque projecteur (menus embarqués,
    /// identification RDM, blackout général…) et son état d'exécution, avant la comparaison avec
    /// l'image précédente.
    pub fn update_with(&mut self, fixtures: &[Fixture], ctx: &Ctx, mut post: impl FnMut(&Fixture, &mut Runtime, &mut [f64])) -> bool {
        let lib = library();
        let mut changed = self.lights.len() != fixtures.len();
        self.lights.retain(|id, _| fixtures.iter().any(|f| &f.id == id));
        for f in fixtures {
            let Some(ty) = lib.fixture(&f.type_id) else { continue };
            let light = self.lights.entry(f.id.clone()).or_insert_with(|| Light { pixels: Vec::new(), source: Source::Off, runtime: Runtime::default() });
            let mut out = vec![0.0; ty.pixels * 3];
            let source = compute_output(f, ty, ctx, &mut out, &mut light.runtime);
            post(f, &mut light.runtime, &mut out);
            if out != light.pixels || source != light.source {
                changed = true;
                light.pixels = out;
                light.source = source;
            }
        }
        changed
    }

    pub fn get(&self, id: &str) -> Option<&Light> {
        self.lights.get(id)
    }

    /// État d'exécution d'un projecteur (créé s'il n'a pas encore été calculé) : les menus embarqués
    /// changent le verrou d'entrée, la couleur BlueMode…
    /// État d'exécution d'un projecteur (entrée des Astera, lampe…), s'il a déjà été calculé.
    pub fn runtime(&self, id: &str) -> Option<&Runtime> {
        self.lights.get(id).map(|l| &l.runtime)
    }

    pub fn runtime_mut(&mut self, id: &str) -> &mut Runtime {
        &mut self.lights.entry(id.to_owned()).or_insert_with(|| Light { pixels: Vec::new(), source: Source::Off, runtime: Runtime::default() }).runtime
    }

    /// Oublie tout (autre projet ouvert) : effets et lampes repartent de zéro.
    pub fn clear(&mut self) {
        self.lights.clear();
    }
}
