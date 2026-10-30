//! Parité avec la version JavaScript : mêmes projets, mêmes réglages, mêmes fichiers (données
//! calculées par `node tools/rust/export-plot-data.mjs` dans `tests/data/`).

use arklight_core::Project;
use arklight_core::library::library;
use arklight_plot::*;
use serde_json::{Value, json};
use std::collections::HashMap;

fn data(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap_or_else(|e| panic!("{name} : {e}"))
}

/// Texte d'un fichier attendu (fins de ligne telles que la version JavaScript les écrit, même si
/// git les a converties).
fn text(name: &str) -> String {
    data(name).replace("\r\n", "\n")
}

fn project(name: &str) -> Project {
    let text = if name == "catalogue.json" { data(name) } else { std::fs::read_to_string(format!("{}/../arklight-core/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap() };
    Project::from_json(&text).unwrap()
}

fn cases() -> Value {
    serde_json::from_str(&data("cases.json")).unwrap()
}

/// Sorties simulées des projecteurs (teinte des faisceaux).
struct Outputs(HashMap<String, Vec<f64>>);

impl Viewport for Outputs {
    fn output(&self, id: &str) -> Option<Vec<f64>> {
        self.0.get(id).cloned()
    }
}

/// Premier écart entre deux textes, avec ce qui l'entoure.
fn first_diff(expected: &str, actual: &str) -> String {
    let (e, a): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), actual.lines().collect());
    for i in 0..e.len().max(a.len()) {
        if e.get(i) != a.get(i) {
            return format!("ligne {} :\n  JavaScript : {}\n  Rust       : {}", i + 1, e.get(i).unwrap_or(&"(fin)"), a.get(i).unwrap_or(&"(fin)"));
        }
    }
    "aucun".into()
}

fn same(name: &str, expected: &str, actual: &str) {
    assert!(expected == actual, "{name} : différent de la version JavaScript, {}", first_diff(expected, actual));
}

fn str_of(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

#[test]
fn plans_svg_identiques() {
    let c = cases();
    let date = c["date"].as_str().unwrap();
    for case in c["svg"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut p = project(case["project"].as_str().unwrap());
        if let Some(patch) = case["plot"].as_object() {
            let plot = p.extra.entry("plot").or_insert_with(|| json!({}));
            for (k, v) in patch {
                plot[k] = v.clone();
            }
        }
        let options: PlotOptions = serde_json::from_value(case["options"].clone()).unwrap();
        let outputs = Outputs(case["outputs"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect())).collect()).unwrap_or_default());
        let svg = if case["kind"] == "export" {
            plot_svg(&p, &PlotSvgOptions { options, date: date.into(), ..PlotSvgOptions::default() })
        } else {
            let v = &case["view"];
            let view = View { cx: v["cx"].as_f64().unwrap(), cz: v["cz"].as_f64().unwrap(), s: v["s"].as_f64().unwrap(), w: v["W"].as_f64().unwrap(), h: v["H"].as_f64().unwrap() };
            let sel = &case["sel"];
            let opts = RenderOptions {
                theme: if case["theme"] == "print" { ThemeName::Print } else { ThemeName::Screen },
                options,
                sel: Selection {
                    fixtures: sel["fixtures"].as_array().map(|a| a.iter().filter_map(str_of).collect()).unwrap_or_default(),
                    object_id: str_of(&sel["objectId"]),
                    plot_id: str_of(&sel["plotId"]),
                },
                preview: case.get("preview").map(PlotItem::from_json),
                handles: case["handles"].as_array().map(|a| a.iter().map(|h| Handle { kind: h["kind"].as_str().unwrap().into(), x: h["x"].as_f64().unwrap(), z: h["z"].as_f64().unwrap() }).collect()).unwrap_or_default(),
                viewport: Some(&outputs),
                tr: None,
                underlay_href: None,
            };
            render_plot(&p, &view, &opts)
        };
        same(name, &text(case["file"].as_str().unwrap()), &svg);
    }
}

#[test]
fn cartouches_identiques() {
    for tb in cases()["titleBlocks"].as_array().unwrap() {
        let t = &tb["title"];
        let title = TitleInfo { production: str_of(&t["production"]), scene: str_of(&t["scene"]), author: str_of(&t["author"]), plan: str_of(&t["plan"]) };
        let theme = if tb["theme"] == "print" { ThemeName::Print } else { ThemeName::Screen };
        let svg = title_block(tb["W"].as_f64().unwrap(), tb["H"].as_f64().unwrap(), &title, theme, tb["date"].as_str().unwrap(), tb["background"].as_str(), None);
        same("cartouche", tb["svg"].as_str().unwrap(), &svg);
    }
}

#[test]
fn geometrie_identique() {
    let g = &cases()["geometry"];
    let f = |v: &Value| v.as_f64().unwrap();
    let p2 = |v: &Value| [f(&v[0]), f(&v[1])];
    for r in g["rotateYaw"].as_array().unwrap() {
        let out = rotate_yaw([f(&r["rotation"][0]), f(&r["rotation"][1]), f(&r["rotation"][2])], f(&r["delta"]));
        assert_eq!(out, [f(&r["out"][0]), f(&r["out"][1]), f(&r["out"][2])], "rotateYaw {r}");
    }
    for d in g["distToSegment"].as_array().unwrap() {
        assert_eq!(dist_to_segment(p2(&d["p"]), p2(&d["a"]), p2(&d["b"])), f(&d["d"]), "distToSegment {d}");
    }
    // empreintes : mêmes valeurs à l'arrondi près des fonctions trigonométriques (le plan arrondit au dixième de pixel)
    let close = |a: f64, b: f64, what: &str| assert!((a - b).abs() < 1e-12, "{what} : {a} ≠ {b} (JavaScript)");
    for fp in g["fixturePlans"].as_array().unwrap() {
        let p = project(fp["project"].as_str().unwrap());
        let fx = p.fixtures.iter().find(|x| x.id == fp["id"].as_str().unwrap()).unwrap();
        let plan = fixture_plan(fx, library().fixture(&fx.type_id).unwrap());
        let e = &fp["plan"];
        let poly = e["poly"].as_array().unwrap();
        assert_eq!(plan.poly.len(), poly.len(), "{} : empreinte", fx.id);
        for (a, b) in plan.poly.iter().zip(poly) {
            close(a[0], f(&b[0]), &fx.id);
            close(a[1], f(&b[1]), &fx.id);
        }
        match (plan.dir, e["dir"].as_array()) {
            (Some(d), Some(x)) => {
                close(d[0], f(&x[0]), &fx.id);
                close(d[1], f(&x[1]), &fx.id);
            }
            (None, None) => {}
            (a, b) => panic!("{} : direction {a:?} / {b:?}", fx.id),
        }
        assert_eq!(plan.down, e["down"].as_bool().unwrap(), "{} : vers le sol", fx.id);
        close(plan.front[0], f(&e["front"][0]), &fx.id);
        close(plan.front[1], f(&e["front"][1]), &fx.id);
    }
    for s in g["slugs"].as_array().unwrap() {
        assert_eq!(slug(&project(s["project"].as_str().unwrap())), s["slug"].as_str().unwrap());
    }
}

#[test]
fn listes_du_materiel_et_patch_identiques() {
    for c in cases()["csv"].as_array().unwrap() {
        let p = project(c["project"].as_str().unwrap());
        let eq = equipment_csv(&build_equipment_list(&p, None), None);
        same(c["equipment"].as_str().unwrap(), &data(c["equipment"].as_str().unwrap()), &eq);
        let patch = patch_csv(&build_patch(&p), None);
        same(c["patch"].as_str().unwrap(), &data(c["patch"].as_str().unwrap()), &patch);
    }
}

/// « 2026-10-27T08:30:14 »
fn stamp(iso: &str) -> Stamp {
    let n = |a: usize, b: usize| iso[a..b].parse::<u32>().unwrap();
    Stamp { year: n(0, 4) as i32, month: n(5, 7), day: n(8, 10), hour: n(11, 13), minute: n(14, 16), second: n(17, 19) }
}

#[test]
fn export_mvr_identique() {
    let v: Value = serde_json::from_str(&data("mvr.json")).unwrap();
    let revision = v["revision"].as_str().unwrap();
    for m in v["mvr"].as_array().unwrap() {
        let name = m["project"].as_str().unwrap();
        let p = project(name);
        let opts = MvrOptions { provider: "ARKLIGHT".into(), provider_version: m["providerVersion"].as_str().unwrap().into(), layer_name: m["layerName"].as_str().unwrap().into(), time: stamp(revision) };
        let out = build_mvr_files(&p, &opts);
        assert_eq!(out.fixtures as u64, m["fixtures"].as_u64().unwrap(), "{name} : projecteurs exportés");
        assert_eq!(out.skipped, m["skipped"].as_array().unwrap().iter().filter_map(str_of).collect::<Vec<_>>(), "{name} : projecteurs ignorés");
        let files = m["files"].as_array().unwrap();
        assert_eq!(out.files.len(), files.len(), "{name} : nombre de fichiers");
        for (f, e) in out.files.iter().zip(files) {
            assert_eq!(f.name, e["name"].as_str().unwrap(), "{name} : nom de fichier");
            if let Some(xml) = e["text"].as_str() {
                same(&format!("{name} › {}", f.name), &xml.replace("\r\n", "\n"), std::str::from_utf8(&f.data).unwrap());
            } else {
                let inner = read_zip(&f.data).unwrap();
                same(&format!("{name} › {}", f.name), e["description"].as_str().unwrap(), std::str::from_utf8(&inner[0].data).unwrap());
                assert_eq!(f.data.len() as u64, e["size"].as_u64().unwrap(), "{name} › {} : taille", f.name);
                assert_eq!(u64::from(crc32(&f.data)), e["crc"].as_u64().unwrap(), "{name} › {} : CRC", f.name);
            }
        }
        let mvr = build_mvr(&p, &opts);
        assert_eq!(mvr.bytes.len() as u64, m["size"].as_u64().unwrap(), "{name} : taille de l'archive");
        assert_eq!(u64::from(crc32(&mvr.bytes)), m["crc"].as_u64().unwrap(), "{name} : CRC de l'archive");
    }
    for u in v["uuids"].as_array().unwrap() {
        assert_eq!(uuid_from(u["text"].as_str().unwrap()), u["uuid"].as_str().unwrap());
    }
    for c in v["crcs"].as_array().unwrap() {
        assert_eq!(u64::from(crc32(c["text"].as_str().unwrap().as_bytes())), c["crc"].as_u64().unwrap());
    }
    let arr3 = |v: &Value| [v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap()];
    for m in v["matrices"].as_array().unwrap() {
        assert_eq!(mvr_matrix(arr3(&m["position"]), arr3(&m["rotation"]), m["fixture"].as_bool().unwrap()), m["matrix"].as_str().unwrap());
    }
    for g in v["multi"].as_array().unwrap() {
        let ty = library().fixture(g["type"].as_str().unwrap()).unwrap();
        let modes: Vec<_> = g["modes"].as_array().unwrap().iter().map(|id| ty.dmx.modes.iter().find(|m| Some(m.id.as_str()) == id.as_str()).unwrap()).collect();
        same(&format!("GDTF {}", ty.id), g["xml"].as_str().unwrap(), &build_gdtf(ty, &modes, revision));
    }
}

#[test]
fn ordre_alphabetique_francais() {
    let v: Value = serde_json::from_str(&data("collation.json")).unwrap();
    let mut names: Vec<String> = v["input"].as_array().unwrap().iter().filter_map(str_of).collect();
    names.sort_by(|a, b| js::collate(a, b));
    let expected: Vec<String> = v["sorted"].as_array().unwrap().iter().filter_map(str_of).collect();
    for (i, (a, e)) in names.iter().zip(&expected).enumerate() {
        assert_eq!(a, e, "rang {i}");
    }
}

#[test]
fn image_png_du_plan() {
    // le catalogue contient tous les symboles du décor : le SVG doit être bien formé (sinon pas d'image)
    let p = project("catalogue.json");
    let svg = plot_svg(&p, &PlotSvgOptions { options: PlotOptions::show_all(), date: "27/10/2026".into(), ..PlotSvgOptions::default() });
    let png = svg_to_png(&svg, 1754, 1240).unwrap();
    std::fs::write(format!("{}/catalogue.png", env!("CARGO_TARGET_TMPDIR")), png).unwrap();
    let rgba = rasterize(&svg, 877, 620);
    assert_eq!(rgba.len(), 877 * 620 * 4);
    let px: Vec<&[u8]> = rgba.chunks(4).collect();
    assert!(px.iter().all(|p| p[3] == 255), "fond opaque");
    let white = px.iter().filter(|p| p[..3] == [255, 255, 255]).count();
    let dark = px.iter().filter(|p| (u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])) < 384).count();
    assert!(white > px.len() / 3, "fond blanc : {white} pixels blancs sur {}", px.len());
    assert!(dark > 500, "traits et textes dessinés : {dark} pixels sombres");
    // vue à l'écran (thème sombre), comme l'application : contenu de `render_plot` dans un document
    let view = View { cx: -1.0, cz: 0.5, s: 38.0, w: 1280.0, h: 760.0 };
    let night = project("demo-night.json");
    let body = render_plot(&night, &view, &RenderOptions { options: PlotOptions::show_all(), ..RenderOptions::default() });
    let png = svg_to_png(&svg_document(&body, view.w, view.h), 1280, 760).unwrap();
    std::fs::write(format!("{}/night-screen.png", env!("CARGO_TARGET_TMPDIR")), png).unwrap();
}
