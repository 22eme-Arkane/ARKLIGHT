//! Cœur commun d'ARKLIGHT, partagé par l'application PC et l'app tablette / téléphone : projets
//! (format 2, les mêmes fichiers que la version JavaScript), bibliothèque (projecteurs, décor,
//! grip, pieds), colorimétrie, moteur lumière (DMX, console, Astera). Sans interface ni carte
//! graphique.

pub mod accessories;
pub mod addressing;
pub mod color;
pub mod document;
pub mod engine;
pub mod library;
pub mod plans;
pub mod project;

pub use library::{FixtureType, Library, library};
pub use project::{Fixture, Project, SceneObject};
