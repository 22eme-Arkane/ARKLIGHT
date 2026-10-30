//! Plan de feu 2D d'ARKLIGHT (PC et mobile) : portage de `web/js/plot/` (géométrie, symboles du
//! décor, rendu SVG, liste du matériel, exports) et de `shared/mvr.js` / `shared/zip.js` (export
//! MVR). Les fichiers produits sont identiques à ceux de la version JavaScript (essais
//! `tests/parity.rs`, données de `node tools/rust/export-plot-data.mjs`).
//!
//! - [`render_plot`] : contenu SVG de la vue du plan (thème écran ou impression, sélection, poignées,
//!   tracé en cours) ; [`svg_document`] l'entoure de la balise `<svg>` ;
//! - [`plot_svg`] : export « Plan en SVG » complet (cadré sur la scène, cartouche) ;
//! - [`build_equipment_list`], [`equipment_csv`], [`build_patch`], [`patch_csv`] : liste du
//!   matériel et patch DMX ;
//! - [`build_mvr`] : archive MVR (GeneralSceneDescription.xml et un GDTF par modèle) ;
//! - [`rasterize`], [`svg_to_png`] : image du plan (affichage, export PNG) ; [`plot_pdf`] : le
//!   plan en PDF (une page A4 paysage) ;
//! - [`Editor`] : outils du plan (sélection, déplacement, orientation, tracé des formes, calage de
//!   l'image de fond) ; ils rendent des [`EditOp`] que [`apply_op`] applique au `plot` du projet
//!   (fonctions de `server/scene-store.js` : [`clean_plot_item`], [`add_item`], [`update_item`],
//!   [`remove_item`], [`update_meta`]).

pub mod catalog;
pub mod editor;
pub mod equipment;
pub mod export;
pub mod geometry;
pub mod items;
pub mod js;
pub mod mvr;
pub mod pdf;
pub mod raster;
pub mod render;
pub mod symbols;
pub mod zip;

pub use catalog::{ObjectType, Support, object_type, patch_line, resolve_support, support_of};
pub use equipment::{EquipmentList, EquipmentRow, EquipmentSection, PatchRow, build_equipment_list, build_patch, equipment_csv, patch_csv, to_csv};
pub use editor::{CALIB_DISTANCE, CALIB_FIRST, CALIB_SECOND, Ctx, EditHandle, Editor, HandleKind, Hit, Key, Modifiers, SELECT_HINT, Target, Tool};
pub use export::{PlotSvgOptions, compute_fit, data_url, plan_text, plot_svg, slug, svg_document};
pub use geometry::{FixturePlan, ObjectPlan, dist_to_segment, fixture_plan, object_plan, point_in_polygon, rotate_yaw};
pub use items::{
    ACCENT, ASSET_PREFIX, EditOp, ItemChange, PLOT_KINDS, PlotVolume, SIZES, WIDTHS, add_item, apply_op, apply_to_project, background_change, calibrate, clean_color, clean_plot, clean_plot_item, color_value, edit_item, empty_plot, is_closed,
    item_changes, jnum, kind_name, new_id, plot_item, plot_items, plot_volumes, remove_item, title_change, underlay_change, underlay_import, underlay_remove, update_item, update_meta,
};
pub use mvr::{Mvr, MvrFile, MvrFiles, MvrOptions, build_gdtf, build_mvr, build_mvr_files, mvr_matrix, uuid_from};
pub use pdf::{A4_LANDSCAPE, PDF_DPI, image_pdf, plot_pdf, plot_pdf_at, rgba_to_rgb};
pub use raster::{rasterize, svg_size, svg_to_png};
pub use render::{BY_HEIGHT, ByHeight, Handle, ItemStyle, PlotData, PlotItem, PlotOptions, RenderOptions, Selection, Theme, ThemeName, TitleInfo, Tr, Underlay, View, Viewport, object_shown, render_plot, theme_for, title_block};
pub use zip::{Stamp, crc32, create_zip, read_zip};
