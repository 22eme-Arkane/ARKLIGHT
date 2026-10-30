//! RDM : parité avec la version JavaScript (données calculées par
//! `node tools/rust/export-rdm-data.mjs`), puis RDM sur Art-Net en boucle locale.

use arklight_core::{Fixture, library};
use arklight_net::rdm::{self, Message, RdmEvent, RdmResponder, SetKind, Uid, cc, pid};
use arklight_net::receiver::Config;
use arklight_net::{Receiver, artnet};
use serde_json::Value;
use std::net::{Ipv4Addr, UdpSocket};
use std::time::{Duration, Instant};

fn data() -> Value {
    let text = std::fs::read_to_string(format!("{}/tests/data/rdm.json", env!("CARGO_MANIFEST_DIR"))).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn reponses_identiques_a_la_version_javascript() {
    let v = data();
    let mut fixtures: Vec<Fixture> = serde_json::from_value(v["fixtures"].clone()).unwrap();
    let mut responder = RdmResponder::new();
    for (u, uids) in v["tod"].as_object().unwrap() {
        let got: Vec<String> = responder.tod(&fixtures, u.parse().unwrap()).iter().map(|x| hex(x)).collect();
        let want: Vec<String> = uids.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_owned()).collect();
        assert_eq!(got, want, "table des appareils de l'univers {u}");
    }
    let requests = v["requests"].as_array().unwrap();
    for (i, r) in requests.iter().enumerate() {
        let universe = r["universe"].as_u64().unwrap() as u32;
        let request = unhex(r["request"].as_str().unwrap());
        let mut events = Vec::new();
        let reply = responder.handle(&fixtures, universe, &request, &mut events);
        let want = r["reply"].as_str().map(str::to_owned);
        assert_eq!(reply.as_deref().map(hex), want, "question n° {i} ({}) sur l'univers {universe}", r["request"]);
        let got: Vec<Value> = events.iter().map(RdmEvent::to_json).collect();
        assert_eq!(Value::Array(got), r["events"], "changements demandés par la question n° {i}");
        for e in &events {
            e.apply(&mut fixtures);
        }
    }
    assert!(requests.len() > 800);
}

#[test]
fn canaux_identiques_a_la_version_javascript() {
    let v = data();
    for entry in v["slots"].as_array().unwrap() {
        let (ty, mode) = (entry["type"].as_str().unwrap(), entry["mode"].as_str().unwrap());
        let m = library().fixture(ty).unwrap().dmx.modes.iter().find(|m| m.id == mode).unwrap();
        let got: Vec<Value> = rdm::slots_of(m).into_iter().map(|s| serde_json::json!([s.kind, s.label, s.text])).collect();
        assert_eq!(Value::Array(got), entry["slots"], "canaux RDM de {ty} / mode {mode}");
    }
}

#[test]
fn textes_ascii_et_identifiants() {
    let v = data();
    for c in v["ascii"].as_array().unwrap() {
        let (s, max, want) = (c[0].as_str().unwrap(), c[1].as_u64().unwrap() as usize, c[2].as_str().unwrap());
        assert_eq!(rdm::ascii(s, max), want, "ascii({s:?}, {max})");
    }
    for c in v["deviceIds"].as_array().unwrap() {
        let dev = rdm::device_id_of(c[0].as_str().unwrap());
        assert_eq!(dev as u64, c[1].as_u64().unwrap(), "identifiant de {}", c[0]);
        assert_eq!(Uid::new(rdm::PROTOTYPE_MANUFACTURER, dev).text(), c[2].as_str().unwrap());
    }
}

#[test]
fn paquets_art_net_identiques() {
    let v = data();
    let a = &v["artnet"];
    for t in a["todData"].as_array().unwrap() {
        let pa = t["portAddress"].as_u64().unwrap() as u16;
        let uids: Vec<[u8; 6]> = t["uids"].as_array().unwrap().iter().map(|u| unhex(u.as_str().unwrap()).try_into().unwrap()).collect();
        let got: Vec<String> = artnet::build_tod_data(pa, &uids).iter().map(|p| hex(p)).collect();
        let want: Vec<String> = t["packets"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_owned()).collect();
        assert_eq!(got, want, "ArtTodData du port-address {pa}");
    }
    for r in a["rdm"].as_array().unwrap() {
        let pa = r["portAddress"].as_u64().unwrap() as u16;
        let data = unhex(r["data"].as_str().unwrap());
        let pkt = artnet::build_rdm(pa, &data);
        assert_eq!(hex(&pkt), r["packet"].as_str().unwrap(), "ArtRdm du port-address {pa}");
        assert_eq!(artnet::parse(&pkt), Some(artnet::Packet::Rdm { port_address: pa, data: &data }));
    }
    let p = &a["pollReply"];
    let mac: Vec<u8> = p["mac"].as_str().unwrap().split(':').map(|h| u8::from_str_radix(h, 16).unwrap()).collect();
    let node = artnet::Node { short_name: "ARKLIGHT", long_name: "ARKLIGHT simulator", ip: p["ip"].as_str().unwrap().parse().unwrap(), mac: mac.try_into().unwrap(), packets: 0 };
    let ports: Vec<u16> = p["ports"].as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u16).collect();
    let replies = artnet::build_poll_replies_rdm(&node, &ports, true);
    assert_eq!(replies.len(), 1);
    assert_eq!(hex(&replies[0]), p["packet"].as_str().unwrap(), "ArtPollReply avec RDM");
    assert_eq!(artnet::build_poll_replies(&node, &ports)[0][23], 0xd0, "sans RDM : bit éteint");
    // ArtTodRequest / ArtTodControl lus
    let req = artnet::build_tod_request(&[0x0105, 0x0107]);
    assert_eq!(artnet::parse(&req), Some(artnet::Packet::TodRequest { port_addresses: vec![0x0105, 0x0107] }));
    let mut ctl = req.clone();
    ctl[8..10].copy_from_slice(&0x8200u16.to_le_bytes());
    ctl[22] = 0x01; // AtcFlush
    ctl[23] = 0x09;
    assert_eq!(artnet::parse(&ctl), Some(artnet::Packet::TodControl { port_address: 0x0109, command: 1 }));
    assert_eq!(artnet::parse(&req[..20]), None, "ArtTodRequest trop court");
}

#[test]
fn changements_appliques_au_projet() {
    let mut fixtures: Vec<Fixture> = serde_json::from_value(data()["fixtures"].clone()).unwrap();
    let ev = RdmEvent::Set { fixture: fixtures[0].id.clone(), what: SetKind::Address, settings: serde_json::from_str(r#"{"dmxAddress": 77}"#).unwrap(), name: None };
    assert!(ev.apply(&mut fixtures));
    assert_eq!(fixtures[0].settings.dmx_address, Some(77));
    assert_eq!(fixtures[0].settings.extra["whitePoint"], 5600, "les autres réglages restent");
    let ev = RdmEvent::Set { fixture: fixtures[1].id.clone(), what: SetKind::Name, settings: Default::default(), name: Some("é".repeat(70)) };
    assert!(ev.apply(&mut fixtures));
    assert_eq!(fixtures[1].name.chars().count(), 60);
    assert!(!RdmEvent::Identify { fixture: fixtures[1].id.clone(), on: true }.apply(&mut fixtures));
    assert_eq!(ev.changes().unwrap()["name"].as_str().unwrap().chars().count(), 70);
}

#[test]
fn rdm_par_art_net_en_boucle_locale() {
    // ports à part et adresse locale : aucun accès au réseau de l'ordinateur
    let cfg = Config { bind: Ipv4Addr::LOCALHOST, artnet_port: 46464, sacn_port: 45578, ..Config::default() };
    let rx = Receiver::start(cfg);
    let mut fixtures: Vec<Fixture> = serde_json::from_value(data()["fixtures"].clone()).unwrap();
    fixtures.retain(|f| f.settings.universe == Some(1));
    rx.set_rdm_fixtures(&fixtures);
    std::thread::sleep(Duration::from_millis(200));
    let client = UdpSocket::bind("127.0.0.1:0").unwrap();
    client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut buf = [0u8; 2048];
    let mut next = |want: u16| -> Vec<u8> {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            assert!(Instant::now() < deadline, "pas de réponse {want:#06x}");
            let (n, _) = client.recv_from(&mut buf).expect("réponse");
            if buf[8..10] == want.to_le_bytes() {
                return buf[..n].to_vec();
            }
        }
    };
    // découverte : port-address 0 → univers 1
    client.send_to(&artnet::build_tod_request(&[0]), "127.0.0.1:46464").unwrap();
    let tod = next(0x8100);
    let count = u16::from_be_bytes([tod[24], tod[25]]) as usize;
    assert_eq!(count, fixtures.iter().filter(|f| library().fixture(&f.type_id).is_some_and(|t| !t.dmx.modes.is_empty())).count());
    let uid = Uid::from_bytes(&tod[28..34]).unwrap();
    // une appli ré-adresse le premier projecteur, puis relit son adresse
    let ask = |cc: u8, pid: u16, pd: Vec<u8>, tn: u8| artnet::build_rdm(0, &rdm::build(&Message { dest: uid, src: Uid::new(0x4c55, 1), tn, port: 1, sub_device: 0, cc, pid, pd }));
    client.send_to(&ask(cc::SET, pid::DMX_START_ADDRESS, vec![0, 201], 1), "127.0.0.1:46464").unwrap();
    let ack = rdm::parse(&next(0x8300)[24..]).expect("réponse RDM");
    assert_eq!((ack.cc, ack.port, ack.tn), (cc::SET_RESPONSE, 0x00, 1));
    client.send_to(&ask(cc::GET, pid::DMX_START_ADDRESS, vec![], 2), "127.0.0.1:46464").unwrap();
    let got = rdm::parse(&next(0x8300)[24..]).unwrap();
    assert_eq!(got.pd, vec![0, 201], "la nouvelle adresse est vue tout de suite");
    client.send_to(&ask(cc::SET, pid::IDENTIFY_DEVICE, vec![1], 3), "127.0.0.1:46464").unwrap();
    next(0x8300);
    let events = rx.take_rdm_events();
    assert!(matches!(&events[0], RdmEvent::Discovery { universes: 1, .. }));
    match &events[1] {
        RdmEvent::Set { fixture, what: SetKind::Address, settings, .. } => {
            assert_eq!(fixture, &fixtures[0].id);
            assert_eq!(settings["dmxAddress"], 201);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(events[2], RdmEvent::Identify { fixture: fixtures[0].id.clone(), on: true });
    assert!(rx.take_rdm_events().is_empty());
    assert_eq!(rx.rdm_identifying(), vec![fixtures[0].id.clone()]);
    // ArtPollReply : RDM annoncé
    client.send_to(&artnet::build_poll(), "127.0.0.1:46464").unwrap();
    assert_eq!(next(0x2100)[23] & 0x02, 0x02);
    let st = rx.status();
    assert!(st.rdm);
    // ses propres réponses lui reviennent aussi (envoyées au port Art-Net de 127.0.0.1) : comptées, sans réponse
    assert!(st.rdm_requests >= 3);
    // RDM coupé : plus de réponse
    rx.disable_rdm();
    client.send_to(&artnet::build_poll(), "127.0.0.1:46464").unwrap();
    assert_eq!(next(0x2100)[23] & 0x02, 0x00);
}
