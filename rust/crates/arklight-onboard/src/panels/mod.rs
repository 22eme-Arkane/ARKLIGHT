//! Panneau de commande simulé de chaque projecteur « non Astera » (voir `panel.rs` et
//! docs/recherche/menus/ : arri.md, aputure_hive.md, dedo_dmg_kino_litegear.md), portage de
//! `shared/onboard/panels/`.

mod aputure;
mod arri;
mod common;
mod dedolight;
mod dmg;
mod hive;
mod kinoflo;
mod litegear;

use crate::panel::{BrandPanel, Pc};
use arklight_core::library::FixtureType;

/// Modèles qui ont un panneau simulé.
pub const PANEL_TYPES: [&str; 30] = [
    "arri.skypanel-s30c",
    "arri.skypanel-s60c",
    "arri.skypanel-s120c",
    "arri.skypanel-s360c",
    "arri.l5-c",
    "arri.l7-c",
    "arri.l10-c",
    "arri.broadcaster-2-plus",
    "aputure.ls-600d-pro",
    "aputure.ls-600x-pro",
    "aputure.ls-1200d-pro",
    "aputure.ls-600d",
    "aputure.ls-300x",
    "aputure.c300d",
    "aputure.nova-p300c",
    "hive.wasp-100-c",
    "hive.hornet-200-c",
    "hive.super-hornet-575-c",
    "dmg-lumiere.sl1-mix",
    "dmg-lumiere.mini-mix",
    "dmg-lumiere.maxi-mix",
    "kino-flo.celeb-250-led-dmx",
    "kino-flo.celeb-450-led-dmx",
    "kino-flo.celeb-led-201-dmx",
    "litegear.litemat-plus-2",
    "litegear.litemat-plus-4",
    "litegear.s2-litemat-2",
    "litegear.s2-litemat-4",
    "dedolight.dled4-bi",
    "dedolight.dled7-bi",
];

/// Panneau d'un modèle, ou None.
pub(crate) fn panel_for(ty: &'static FixtureType) -> Option<Box<dyn BrandPanel>> {
    Some(match ty.id.as_str() {
        "arri.skypanel-s30c" | "arri.skypanel-s60c" | "arri.skypanel-s120c" | "arri.skypanel-s360c" => Box::new(Pc::<arri::SkyPanel>::new(ty)),
        "arri.l5-c" | "arri.l7-c" | "arri.l10-c" => Box::new(Pc::<arri::LSeries>::new(ty)),
        "arri.broadcaster-2-plus" => Box::new(Pc::<arri::BroadCaster>::new(ty)),
        "aputure.ls-600d-pro" | "aputure.ls-600x-pro" | "aputure.ls-1200d-pro" => Box::new(Pc::<aputure::AputurePro>::new(ty)),
        "aputure.ls-600d" | "aputure.ls-300x" | "aputure.c300d" => Box::new(Pc::<aputure::AputureOled>::new(ty)),
        "aputure.nova-p300c" => Box::new(Pc::<aputure::Nova>::new(ty)),
        "hive.wasp-100-c" | "hive.hornet-200-c" => Box::new(Pc::<hive::HiveC>::new(ty)),
        "hive.super-hornet-575-c" => Box::new(Pc::<hive::SuperHornet>::new(ty)),
        "dmg-lumiere.sl1-mix" | "dmg-lumiere.mini-mix" | "dmg-lumiere.maxi-mix" => Box::new(Pc::<dmg::DmgMix>::new(ty)),
        "kino-flo.celeb-250-led-dmx" | "kino-flo.celeb-450-led-dmx" => Box::new(Pc::<kinoflo::TrueMatch>::new(ty)),
        "kino-flo.celeb-led-201-dmx" => Box::new(Pc::<kinoflo::Celeb201>::new(ty)),
        "litegear.litemat-plus-2" | "litegear.litemat-plus-4" => Box::new(Pc::<litegear::PlusDuo>::new(ty)),
        "litegear.s2-litemat-2" | "litegear.s2-litemat-4" => Box::new(Pc::<litegear::ProHybrid>::new(ty)),
        "dedolight.dled4-bi" | "dedolight.dled7-bi" => Box::new(Pc::<dedolight::Dedolight>::new(ty)),
        _ => return None,
    })
}
