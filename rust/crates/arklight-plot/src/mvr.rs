//! Export MVR (My Virtual Rig, DIN SPEC 15801, version 1.6 ; portage de `shared/mvr.js`) : le plan
//! d'éclairage avec son patch, lisible par Blackout, grandMA3, Vectorworks, Capture, ChamSys, Eos…
//! L'archive contient GeneralSceneDescription.xml et un fichier GDTF (DIN SPEC 15800) par modèle de
//! projecteur, décrit à partir des profils DMX de la bibliothèque (seulement les modes utilisés).
//!
//! Repères : ARKLIGHT a Y vers le haut et Z vers le public ; MVR a Z vers le haut, Y vers le
//! lointain, des millimètres. Un projecteur GDTF « au repos » éclaire vers le bas (−Z), un
//! projecteur ARKLIGHT non tourné éclaire vers le public : on passe de l'un à l'autre.

use crate::geometry::M3;
use crate::js::{min2, num, num_truthy, round, strip_accents, text_of, truthy};
use crate::zip::{Stamp, create_zip};
use arklight_core::library::{DmxMode, FixtureType, library};
use arklight_core::project::Project;
use serde_json::Value;

// ------------------------------------------------------------------ identifiants stables
/// UUID (texte RFC 4122) tiré d'un texte : le même projecteur garde le même UUID d'un export à l'autre.
pub fn uuid_from(text: &str) -> String {
    let (mut h1, mut h2, mut h3, mut h4): (u32, u32, u32, u32) = (0x811c9dc5, 0x01000193, 0x9e3779b9, 0x85ebca6b);
    for (i, c) in text.encode_utf16().enumerate() {
        let c = c as u32;
        h1 = (h1 ^ c).wrapping_mul(0x01000193);
        h2 = (h2 ^ c).wrapping_mul(0x5bd1e995);
        h3 = h3.wrapping_add(c).wrapping_mul(0x27d4eb2d) ^ (h3 >> 15);
        h4 = (h4 ^ c.wrapping_add(i as u32)).wrapping_mul(0xc2b2ae35) ^ (h4 >> 13);
    }
    let mut b: Vec<char> = format!("{h1:08x}{h2:08x}{h3:08x}{h4:08x}").chars().collect();
    b[12] = '5'; // version (identifiant dérivé d'un nom)
    let v = b[16].to_digit(16).unwrap_or(0);
    b[16] = char::from_digit((v & 0x3) | 0x8, 16).unwrap_or('8'); // variante RFC 4122
    let s: String = b.into_iter().collect::<String>().to_uppercase();
    format!("{}-{}-{}-{}-{}", &s[0..8], &s[8..12], &s[12..16], &s[16..20], &s[20..])
}

/// Échappement XML (& < > " ').
fn xml_esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Nombre arrondi à `d` décimales, écrit comme en JavaScript.
fn numd(v: f64, d: i32) -> String {
    let k = 10f64.powi(d);
    num(round(v * k) / k)
}

/// Nom de fichier sans caractères gênants.
fn file_safe(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in strip_accents(s).chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            if gap {
                out.push('_');
                gap = false;
            }
            out.push(c);
        } else {
            gap = true;
        }
    }
    if gap {
        out.push('_');
    }
    let t = out.trim_matches('_');
    if t.is_empty() { "Fixture".into() } else { t.to_string() }
}

// ------------------------------------------------------------------ GDTF : attributs
/// Fonction d'un canal ARKLIGHT → attribut GDTF, groupe de fonctions, unité physique, plage.
fn attr_of(f: Option<&str>) -> (&'static str, &'static str, &'static str, &'static str, f64, f64) {
    let rgb = |a: &'static str| (a, "Color", "RGB", "ColorComponent", 0.0, 1.0);
    let ctl = |a: &'static str| (a, "Control", "Control", "None", 0.0, 1.0);
    match f.unwrap_or("") {
        "dimmer" => ("Dimmer", "Dimmer", "Dimmer", "LuminousIntensity", 0.0, 1.0),
        "red" => rgb("ColorAdd_R"),
        "green" => rgb("ColorAdd_G"),
        "blue" => rgb("ColorAdd_B"),
        "white" => rgb("ColorAdd_W"),
        "amber" => rgb("ColorAdd_RY"),
        "lime" => rgb("ColorAdd_GY"),
        "mint" => rgb("ColorAdd_GC"),
        "cyan" => rgb("ColorAdd_C"),
        "magenta" => rgb("ColorAdd_M"),
        "yellow" => rgb("ColorAdd_Y"),
        "indigo" => rgb("ColorAdd_BM"),
        "uv" => rgb("ColorAdd_UV"),
        "ww" => rgb("ColorAdd_WW"),
        "cw" => rgb("ColorAdd_CW"),
        "cct" => ("CTC", "Color", "Color", "Temperature", 2000.0, 10000.0),
        "tint" => ("Tint", "Color", "Color", "None", -1.0, 1.0),
        "hue" => ("HSB_Hue", "Color", "HSB", "Angle", 0.0, 360.0),
        "sat" => ("HSB_Saturation", "Color", "HSB", "Percent", 0.0, 100.0),
        "x" => ("CIE_X", "Color", "CIE", "None", 0.0, 1.0),
        "y" => ("CIE_Y", "Color", "CIE", "None", 0.0, 1.0),
        "crossfade" => ("ColorMixMode", "Color", "Color", "None", 0.0, 1.0),
        "index" => ("ColorMacro1", "Color", "Color", "None", 0.0, 1.0),
        "cctIndex" => ("ColorMacro2", "Color", "Color", "None", 0.0, 1.0),
        "strobe" => ("Shutter1Strobe", "Beam", "Beam", "Frequency", 0.0, 25.0),
        "fan" => ctl("Fans"),
        "fxProgram" => ctl("Effects1"),
        "fxIndex" => ctl("Effects2"),
        "fxSpeed" => ctl("Effects1Rate"),
        "fxFade" => ctl("Effects1Fade"),
        "fxDirection" => ctl("Effects1Adjust1"),
        "fxSize" => ctl("Effects1Adjust2"),
        "fxOffset" => ctl("Effects1Adjust3"),
        "fxRestart" => ctl("Effects1Adjust4"),
        "fxR" => ("EffectsColor_R", "Control", "Control", "ColorComponent", 0.0, 1.0),
        "fxG" => ("EffectsColor_G", "Control", "Control", "ColorComponent", 0.0, 1.0),
        "fxB" => ("EffectsColor_B", "Control", "Control", "ColorComponent", 0.0, 1.0),
        _ => ctl("NoFeature"),
    }
}

/// Nom du modèle sans la marque devant (« ARRI M18 » → « M18 »).
fn model_name(ty: &FixtureType) -> String {
    if ty.name.to_lowercase().starts_with(&format!("{} ", ty.brand.to_lowercase())) { ty.name.chars().skip(ty.brand.chars().count() + 1).collect() } else { ty.name.clone() }
}

/// Nom du mode tel qu'il apparaît dans la console (unique pour le modèle).
pub fn mode_name(mode: &DmxMode) -> String {
    text_of(mode.extra.get("label")).map(str::to_string).or_else(|| (!mode.name.is_empty()).then(|| mode.name.clone())).unwrap_or_else(|| mode.id.clone())
}

/// Canal à plat (`compileMode`, d'après les définitions JSON : noms des fonctions gardés).
struct Slot<'a> {
    f: Option<&'a str>,
    px: f64,
    bytes: f64,
    label: Option<&'a str>,
}

fn compile(mode: &DmxMode) -> Vec<Slot<'_>> {
    fn push<'a>(ch: &'a Value, px: f64, out: &mut Vec<Slot<'a>>) {
        let (f, def) = match ch {
            Value::String(s) => (Some(s.as_str()), None),
            o => (o.get("f").and_then(Value::as_str), Some(o)),
        };
        let d = |k: &str| def.and_then(|d| d.get(k));
        let px = match d("px") {
            Some(Value::Null) => 0.0,
            Some(v) => v.as_f64().unwrap_or(f64::NAN),
            None => px,
        };
        out.push(Slot { f, px, bytes: num_truthy(d("bytes")).unwrap_or(1.0), label: text_of(d("label")) });
    }
    let mut out = Vec::new();
    for ch in &mode.channels {
        let block = ch.as_object().and_then(|o| o.get("pixels")).filter(|v| truthy(Some(v)));
        match block.and_then(Value::as_f64) {
            Some(n) => {
                let first = num_truthy(ch.get("firstPixel")).unwrap_or(0.0);
                let inner = ch.get("channels").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
                let mut p = 0.0;
                while p < n {
                    for c in inner {
                        push(c, first + p, &mut out);
                    }
                    p += 1.0;
                }
            }
            None => push(ch, -1.0, &mut out),
        }
    }
    out
}

struct Attr {
    group: &'static str,
    feature: &'static str,
    unit: &'static str,
    pretty: String,
}

/// Fichier GDTF (description.xml) d'un modèle, avec les modes demandés ; `revision` : date ISO de la
/// révision (« 2026-10-27T08:30:14 »).
pub fn build_gdtf(ty: &FixtureType, modes: &[&DmxMode], revision: &str) -> String {
    let mut attrs: Vec<(String, Attr)> = Vec::new(); // nom → groupe, fonction, unité (ordre d'arrivée)
    let mut dmx_modes = Vec::new();
    let mut pixel_count = 1.0_f64;
    for mode in modes {
        let mut used: Vec<String> = Vec::new();
        let mut channels = String::new();
        let mut offset = 1.0;
        for slot in compile(mode) {
            let (base, group, feature, unit, from, to) = attr_of(slot.f);
            let geometry = if slot.px >= 0.0 { format!("Pixel {}", num(slot.px + 1.0)) } else { "Body".into() };
            if slot.px >= 0.0 {
                pixel_count = pixel_count.max(slot.px + 1.0);
            }
            // un même attribut deux fois sur la même géométrie : on le numérote (attribut propre au fichier)
            let mut attr = base.to_string();
            let mut k = 2;
            while used.contains(&format!("{geometry}|{attr}")) {
                attr = format!("{base}{k}");
                k += 1;
            }
            used.push(format!("{geometry}|{attr}"));
            let pretty = slot.label.map_or_else(|| attr.clone(), str::to_string);
            match attrs.iter_mut().find(|(n, _)| *n == attr) {
                Some((_, a)) => *a = Attr { group, feature, unit, pretty: pretty.clone() },
                None => attrs.push((attr.clone(), Attr { group, feature, unit, pretty: pretty.clone() })),
            }
            let bytes = slot.bytes;
            let offsets = if bytes == 2.0 { format!("{},{}", num(offset), num(offset + 1.0)) } else { num(offset) };
            offset += bytes;
            let label = xml_esc(&pretty);
            channels += &format!(
                r#"<DMXChannel DMXBreak="1" Offset="{offsets}" Highlight="None" Geometry="{}"><LogicalChannel Attribute="{a}" Snap="No" Master="None" MibFade="0" DMXChangeTimeLimit="0"><ChannelFunction Name="{label}" Attribute="{a}" OriginalAttribute="" DMXFrom="0/{b}" Default="0/{b}" PhysicalFrom="{}" PhysicalTo="{}" RealFade="0" RealAcceleration="0"/></LogicalChannel></DMXChannel>"#,
                xml_esc(&geometry),
                num(from),
                num(to),
                a = xml_esc(&attr),
                b = num(bytes)
            );
        }
        dmx_modes.push(format!(r#"<DMXMode Name="{}" Description="{}" Geometry="Body"><DMXChannels>{channels}</DMXChannels><Relations/><FTMacros/></DMXMode>"#, xml_esc(&mode_name(mode)), xml_esc(&mode.name)));
    }
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for (_, a) in &attrs {
        match groups.iter_mut().find(|(g, _)| *g == a.group) {
            Some((_, feats)) => {
                if !feats.contains(&a.feature) {
                    feats.push(a.feature);
                }
            }
            None => groups.push((a.group, vec![a.feature])),
        }
    }
    let g = |k: &str| ty.dim(k).filter(|v| *v != 0.0 && !v.is_nan());
    let l = g("length").or(g("depth")).or(g("diameter")).unwrap_or(0.3);
    let w = g("width").or(g("diameter")).unwrap_or(0.3);
    let h = g("height").or(g("diameter")).unwrap_or(0.2);
    let beam_angle = num(ty.beam_angle.filter(|a| *a != 0.0 && !a.is_nan()).unwrap_or(40.0));
    let pixels = if pixel_count > 1.0 {
        let mut s = String::new();
        let mut i = 0.0;
        while i < pixel_count {
            let x = if ty.geometry.kind == "tube" { -l / 2.0 + (l * (i + 0.5)) / pixel_count } else { 0.0 };
            s += &format!(
                r#"<Beam Name="Pixel {}" Model="Pixel" Position="{{1,0,0,{}}}{{0,1,0,0}}{{0,0,1,0}}{{0,0,0,1}}" LampType="LED" BeamAngle="{beam_angle}" FieldAngle="{beam_angle}" BeamRadius="0.02" BeamType="Wash" ColorTemperature="5600" ColorRenderingIndex="90"/>"#,
                num(i + 1.0),
                numd(x, 6)
            );
            i += 1.0;
        }
        s
    } else {
        let lamp = match ty.technology.as_deref() {
            Some("hmi") => "Discharge",
            Some("tungsten") => "Tungsten",
            _ => "LED",
        };
        format!(
            r#"<Beam Name="Beam" Model="Pixel" Position="{{1,0,0,0}}{{0,1,0,0}}{{0,0,1,{}}}{{0,0,0,1}}" LampType="{lamp}" PowerConsumption="{}" LuminousFlux="{}" BeamAngle="{beam_angle}" FieldAngle="{beam_angle}" BeamRadius="{}" BeamType="Wash" ColorTemperature="{}" ColorRenderingIndex="90"/>"#,
            numd(-h / 2.0, 6),
            num(num_truthy(ty.extra.get("watts")).unwrap_or(100.0)),
            num(ty.output.as_ref().and_then(|o| o.lumens).filter(|v| *v != 0.0).unwrap_or(1000.0)),
            numd(min2(w, l) / 2.0, 3),
            num(ty.lamp.as_ref().and_then(|lp| lp.cct).filter(|v| *v != 0.0).unwrap_or(5600.0))
        )
    };
    let model = model_name(ty);
    let short = text_of(ty.extra.get("model")).map_or_else(|| model.clone(), str::to_string);
    let feature_groups: String = groups.iter().map(|(grp, feats)| format!(r#"<FeatureGroup Name="{grp}" Pretty="{grp}">{}</FeatureGroup>"#, feats.iter().map(|f| format!(r#"<Feature Name="{f}"/>"#)).collect::<String>())).collect();
    let attributes: String = attrs
        .iter()
        .map(|(name, a)| {
            // `xmlEsc(a.pretty).slice(0, 24)` : 24 unités UTF-16 du texte échappé
            let pretty: Vec<u16> = xml_esc(&a.pretty).encode_utf16().take(24).collect();
            format!(r#"<Attribute Name="{}" Pretty="{}" Feature="{}.{}" PhysicalUnit="{}"/>"#, xml_esc(name), String::from_utf16_lossy(&pretty), a.group, a.feature, a.unit)
        })
        .collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n<GDTF DataVersion=\"1.2\">\
<FixtureType Name=\"{}\" ShortName=\"{}\" LongName=\"{}\" Manufacturer=\"{}\" Description=\"{}\" FixtureTypeID=\"{}\" Thumbnail=\"\" RefFT=\"\" CanHaveChildren=\"Yes\">\
<AttributeDefinitions><ActivationGroups/><FeatureGroups>{feature_groups}</FeatureGroups>\
<Attributes>{attributes}</Attributes></AttributeDefinitions>\
<Wheels/><PhysicalDescriptions/>\
<Models><Model Name=\"Body\" Length=\"{}\" Width=\"{}\" Height=\"{}\" PrimitiveType=\"Cube\"/><Model Name=\"Pixel\" Length=\"{}\" Width=\"{}\" Height=\"0.01\" PrimitiveType=\"Cylinder\"/></Models>\
<Geometries><Geometry Name=\"Body\" Model=\"Body\" Position=\"{{1,0,0,0}}{{0,1,0,0}}{{0,0,1,0}}{{0,0,0,1}}\">{pixels}</Geometry></Geometries>\
<DMXModes>{}</DMXModes>\
<Revisions><Revision Text=\"ARKLIGHT\" Date=\"{revision}\" UserID=\"0\"/></Revisions><FTPresets/><Protocols/>\
</FixtureType></GDTF>",
        xml_esc(&model),
        xml_esc(&short),
        xml_esc(&format!("{} {model}", ty.brand)),
        xml_esc(&ty.brand),
        xml_esc(&format!("Exporté par ARKLIGHT — {}", ty.id)),
        uuid_from(&format!("gdtf:{}", ty.id)),
        numd(l, 3),
        numd(w, 3),
        numd(h, 3),
        numd(min2(0.05, l), 3),
        numd(min2(0.05, w), 3),
        dmx_modes.concat()
    )
}

// ------------------------------------------------------------------ position et orientation
/// Rotation three.js (Euler XYZ, radians) → matrice 3×3 (lignes).
fn euler_matrix([a, b, c]: [f64; 3]) -> M3 {
    let (ca, sa, cb, sb, cc, sc) = (a.cos(), a.sin(), b.cos(), b.sin(), c.cos(), c.sin());
    [[cb * cc, -cb * sc, sb], [ca * sc + sa * sb * cc, ca * cc - sa * sb * sc, -sa * cb], [sa * sc - ca * sb * cc, sa * cc + ca * sb * sc, ca * cb]]
}

fn mul(a: &M3, b: &M3) -> M3 {
    a.map(|row| [0, 1, 2].map(|j| row[0] * b[0][j] + row[1] * b[1][j] + row[2] * b[2][j]))
}

fn transpose(a: &M3) -> M3 {
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| a[j][i]))
}

/// (x, y, z) three.js → (x, −z, y) MVR
const C: M3 = [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]];
/// projecteur GDTF (faisceau vers −Z) → projecteur ARKLIGHT non tourné (faisceau vers le public)
const M0: M3 = [[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]];

/// Matrice MVR « {u}{v}{w}{o} » d'un élément placé dans ARKLIGHT (mètres, rotation XYZ).
pub fn mvr_matrix(position: [f64; 3], rotation: [f64; 3], fixture: bool) -> String {
    let mut m = mul(&mul(&C, &euler_matrix(rotation)), &transpose(&C));
    if fixture {
        m = mul(&m, &M0);
    }
    let col = |j: usize| [m[0][j], m[1][j], m[2][j]];
    let [x, y, z] = position;
    let o = [x * 1000.0, -z * 1000.0, y * 1000.0];
    [col(0), col(1), col(2), o].iter().map(|v| format!("{{{}}}", v.iter().map(|n| numd(*n, 6)).collect::<Vec<_>>().join(","))).collect()
}

// ------------------------------------------------------------------ scène MVR
/// Réglages de l'export.
#[derive(Debug, Clone)]
pub struct MvrOptions {
    pub provider: String,
    /// version d'ARKLIGHT
    pub provider_version: String,
    /// nom du calque (« Projecteurs », traduit)
    pub layer_name: String,
    /// date des fichiers et de la révision des GDTF
    pub time: Stamp,
}

impl Default for MvrOptions {
    fn default() -> Self {
        Self { provider: "ARKLIGHT".into(), provider_version: "0".into(), layer_name: "Projecteurs".into(), time: Stamp::now_utc() }
    }
}

/// Un fichier de l'archive MVR.
#[derive(Debug, Clone, PartialEq)]
pub struct MvrFile {
    pub name: String,
    pub data: Vec<u8>,
}

/// Fichiers de l'export (`buildMvrFiles`).
#[derive(Debug, Clone, PartialEq)]
pub struct MvrFiles {
    /// GeneralSceneDescription.xml, puis un GDTF (archive) par modèle
    pub files: Vec<MvrFile>,
    /// projecteurs exportés
    pub fixtures: usize,
    /// projecteurs sans DMX ignorés (noms)
    pub skipped: Vec<String>,
}

/// Fichiers de l'export MVR d'une scène.
pub fn build_mvr_files(scene: &Project, opts: &MvrOptions) -> MvrFiles {
    let lib = library();
    let mut by_type: Vec<(&FixtureType, Vec<&DmxMode>, String)> = Vec::new(); // modèle, modes utilisés, fichier
    let mut skipped = Vec::new();
    let mut fixtures = Vec::new();
    for f in &scene.fixtures {
        let ty = lib.fixture(&f.type_id);
        let Some((ty, mode)) = ty.and_then(|t| t.mode(f.settings.dmx_mode.as_deref()).map(|m| (t, m))) else {
            skipped.push(f.name.clone()); // boîtiers sans DMX (AsteraBox…)
            continue;
        };
        match by_type.iter_mut().find(|(t, _, _)| t.id == ty.id) {
            Some((_, modes, _)) => {
                if !modes.iter().any(|m| m.id == mode.id) {
                    modes.push(mode);
                }
            }
            None => by_type.push((ty, vec![mode], String::new())),
        }
        fixtures.push((f, ty, mode));
    }
    // noms de fichiers GDTF uniques : « Marque@Modèle.gdtf »
    let mut names: Vec<String> = Vec::new();
    for (ty, _, file) in &mut by_type {
        let base = format!("{}@{}", file_safe(&ty.brand), file_safe(&model_name(ty)));
        let mut name = base.clone();
        let mut k = 2;
        while names.contains(&name.to_lowercase()) {
            name = format!("{base}_{k}");
            k += 1;
        }
        names.push(name.to_lowercase());
        *file = format!("{name}.gdtf");
    }
    let fixture_xml: String = fixtures
        .iter()
        .map(|(f, ty, mode)| {
            let s = &f.settings;
            let universe = s.universe.unwrap_or(1).max(1);
            let address = s.dmx_address.unwrap_or(1).clamp(1, 512);
            let fix = s.fixture_number.unwrap_or(0);
            let file = by_type.iter().find(|(t, _, _)| t.id == ty.id).map_or("", |e| e.2.as_str());
            format!(
                "<Fixture name=\"{}\" uuid=\"{}\"><Matrix>{}</Matrix><GDTFSpec>{}</GDTFSpec><GDTFMode>{}</GDTFMode><Addresses><Address break=\"0\">{}</Address></Addresses><FixtureID>{}</FixtureID><FixtureIDNumeric>{fix}</FixtureIDNumeric><UnitNumber>0</UnitNumber><CustomId>{fix}</CustomId><CastShadow>false</CastShadow></Fixture>",
                xml_esc(&f.name),
                uuid_from(&format!("fixture:{}", f.id)),
                mvr_matrix(f.position, f.rotation, true),
                xml_esc(file),
                xml_esc(&mode_name(mode)),
                (u64::from(universe) - 1) * 512 + u64::from(address),
                if fix != 0 { fix.to_string() } else { String::new() }
            )
        })
        .collect();
    let (p, v) = (xml_esc(&opts.provider), xml_esc(&opts.provider_version));
    let scene_xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\" ?>\n<GeneralSceneDescription verMajor=\"1\" verMinor=\"6\" provider=\"{p}\" providerVersion=\"{v}\"><UserData><Data provider=\"{p}\" ver=\"{v}\"/></UserData><Scene><AUXData/><Layers><Layer name=\"{}\" uuid=\"{}\"><ChildList>{fixture_xml}</ChildList></Layer></Layers></Scene></GeneralSceneDescription>",
        xml_esc(&opts.layer_name),
        uuid_from(&format!("layer:{}", scene.name))
    );
    let revision = opts.time.iso();
    let mut files = vec![MvrFile { name: "GeneralSceneDescription.xml".into(), data: scene_xml.into_bytes() }];
    for (ty, modes, file) in &by_type {
        let xml = build_gdtf(ty, modes, &revision);
        files.push(MvrFile { name: file.clone(), data: create_zip(&[("description.xml", xml.as_bytes())], &opts.time) });
    }
    MvrFiles { files, fixtures: fixtures.len(), skipped }
}

/// Archive MVR prête à enregistrer.
#[derive(Debug, Clone, PartialEq)]
pub struct Mvr {
    pub bytes: Vec<u8>,
    pub fixtures: usize,
    pub skipped: Vec<String>,
}

/// `buildMvr` : archive MVR d'une scène. Sans projecteur exporté (`fixtures` à 0), la version
/// JavaScript n'enregistre rien et affiche « Aucun projecteur à exporter ».
pub fn build_mvr(scene: &Project, opts: &MvrOptions) -> Mvr {
    let MvrFiles { files, fixtures, skipped } = build_mvr_files(scene, opts);
    let list: Vec<(&str, &[u8])> = files.iter().map(|f| (f.name.as_str(), f.data.as_slice())).collect();
    Mvr { bytes: create_zip(&list, &opts.time), fixtures, skipped }
}
