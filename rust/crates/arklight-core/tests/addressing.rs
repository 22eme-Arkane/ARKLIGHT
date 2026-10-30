//! Adressage automatique : mêmes résultats que `shared/engine/addressing.js` (données :
//! node tools/rust/export-addressing-data.mjs).

use arklight_core::Fixture;
use arklight_core::addressing::{Options, Order, Scope, conflicts, natural_cmp, plan_addressing};
use serde_json::Value;
use std::cmp::Ordering;

#[test]
fn ordre_naturel() {
    assert_eq!(natural_cmp("Titan 2", "Titan 10"), Ordering::Less);
    assert_eq!(natural_cmp("élise", "Elise"), Ordering::Equal);
    assert_eq!(natural_cmp("Face", "fond"), Ordering::Less);
    assert_eq!(natural_cmp("Titan", "Titan 1"), Ordering::Less);
}

#[test]
fn comme_la_version_javascript() {
    let data: Vec<Value> = serde_json::from_str(include_str!("data/addressing.json")).unwrap();
    let mut checked = 0;
    for (n, case) in data.iter().enumerate() {
        let fixtures: Vec<Fixture> = serde_json::from_value(case["fixtures"].clone()).unwrap();
        let c = conflicts(&fixtures);
        let overlaps: Vec<(String, String)> = serde_json::from_value(case["conflicts"]["overlaps"].clone()).unwrap();
        let overflow: Vec<String> = serde_json::from_value(case["conflicts"]["overflow"].clone()).unwrap();
        assert_eq!(c.overlaps, overlaps, "scène {n} : chevauchements");
        assert_eq!(c.overflow, overflow, "scène {n} : dépassements");
        let ids: Vec<String> = serde_json::from_value(case["ids"].clone()).unwrap();
        for p in case["plans"].as_array().unwrap() {
            let o = &p["opts"];
            let opts = Options {
                scope: match o["scope"].as_str().unwrap() {
                    "conflicts" => Scope::Conflicts,
                    "all" => Scope::All,
                    _ => Scope::Ids(ids.clone()),
                },
                order: Order::from_id(o["order"].as_str().unwrap()),
                universe: o["universe"].as_u64().unwrap() as u32,
                address: o["address"].as_u64().unwrap() as u32,
                pack: o["universes"] == "pack",
                align: o["align"].as_u64().unwrap() as u32,
                overflow: o["overflow"].as_bool().unwrap(),
            };
            let plan = plan_addressing(&fixtures, &opts);
            let want = &p["plan"];
            let moves: Vec<(String, u64, u64)> = want["moves"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| (m[0].as_str().unwrap().to_owned(), m[1].as_u64().unwrap(), m[2].as_u64().unwrap()))
                .collect();
            let got: Vec<(String, u64, u64)> = plan.moves.iter().map(|m| (m.id.clone(), m.to.universe as u64, m.to.address as u64)).collect();
            assert_eq!(got, moves, "scène {n}, options {o}");
            let unplaced: Vec<String> = serde_json::from_value(want["unplaced"].clone()).unwrap();
            assert_eq!(plan.unplaced, unplaced, "scène {n}, options {o} : sans place");
            assert_eq!(plan.considered as u64, want["considered"].as_u64().unwrap());
            assert_eq!(plan.conflicts_before as u64, want["conflictsBefore"].as_u64().unwrap());
            assert_eq!(plan.conflicts_after as u64, want["conflictsAfter"].as_u64().unwrap(), "scène {n}, options {o} : conflits après");
            checked += 1;
        }
    }
    assert!(checked > 1000, "{checked} plans comparés");
}
