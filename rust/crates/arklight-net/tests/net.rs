//! Réception DMX : paquets Art-Net et sACN, fusion des sources, réception réelle en boucle locale.

use arklight_net::receiver::Config;
use arklight_net::universes::{Input, UniverseStore};
use arklight_net::{Receiver, artnet, sacn};
use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, UdpSocket};
use std::time::{Duration, Instant};

#[test]
fn artnet_paquets() {
    let data: Vec<u8> = (0..=255).collect();
    let pkt = artnet::build_dmx(0x0123, 7, &data);
    match artnet::parse(&pkt) {
        Some(artnet::Packet::Dmx { port_address, sequence, data: d }) => {
            assert_eq!((port_address, sequence), (0x0123, 7));
            assert_eq!(d, &data[..]);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(artnet::parse(&artnet::build_poll()), Some(artnet::Packet::Poll));
    assert_eq!(artnet::parse(b"pas de l'Art-Net"), None);
    // ports groupés par Net / Sub-Net, 4 au plus par réponse
    let node = artnet::Node { short_name: "ARKLIGHT", long_name: "ARKLIGHT simulator", ip: Ipv4Addr::new(10, 0, 0, 5), mac: [0; 6], packets: 3 };
    let replies = artnet::build_poll_replies(&node, &[0, 1, 2, 3, 4, 16]);
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[0].len(), 239);
    assert_eq!(&replies[0][10..14], &[10, 0, 0, 5]);
    assert_eq!(replies[0][173], 4);
    assert_eq!(replies[2][19], 1, "Sub-Net du port 16");
    match artnet::parse(&replies[0]) {
        Some(artnet::Packet::PollReply { ip, name }) => assert_eq!((ip, name.as_str()), (Ipv4Addr::new(10, 0, 0, 5), "ARKLIGHT")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn sacn_paquets() {
    let cid = [9u8; 16];
    let pkt = sacn::build(7, 150, 3, "Blackout", cid, &[255, 128, 0]);
    match sacn::parse(&pkt) {
        Some(sacn::Packet::Dmx { universe, priority, data, source_name, .. }) => {
            assert_eq!((universe, priority, data, source_name.as_str()), (7, 150, &[255u8, 128, 0][..], "Blackout"));
        }
        other => panic!("{other:?}"),
    }
    let mut stop = pkt.clone();
    stop[112] = 0x40;
    assert_eq!(sacn::parse(&stop), Some(sacn::Packet::Terminated { universe: 7, cid }));
    let mut preview = pkt.clone();
    preview[112] = 0x80;
    assert_eq!(sacn::parse(&preview), None, "prévisualisation ignorée");
    assert_eq!(sacn::multicast_group(258), Ipv4Addr::new(239, 255, 1, 2));
}

fn input(u: u32, key: &str, priority: u8, data: &[u8]) -> Input<'static> {
    Input { universe: u, data: Box::leak(data.to_vec().into_boxed_slice()), key: key.into(), priority, protocol: "sacn", address: key.into(), name: String::new() }
}

#[test]
fn fusion_des_sources() {
    let mut s = UniverseStore::default();
    let t0 = Instant::now();
    s.input(input(1, "a", 100, &[10, 200]), t0);
    s.input(input(1, "b", 100, &[50, 20]), t0);
    let d = |s: &UniverseStore| *s.data().find(|(u, _)| *u == 1).unwrap().1;
    assert_eq!(&d(&s)[..2], &[50, 200], "HTP entre sources de même priorité");
    s.input(input(1, "c", 150, &[1, 2]), t0);
    assert_eq!(&d(&s)[..2], &[1, 2], "la priorité sACN l'emporte");
    // toutes les sources se taisent : dernier état conservé, plus de signal
    let later = t0 + Duration::from_secs(5);
    assert!(s.live(later).is_empty());
    s.input(input(2, "x", 100, &[9]), later);
    assert_eq!(&d(&s)[..2], &[1, 2]);
    assert!(s.forget(1, later));
    assert!(s.data().all(|(u, _)| u != 1));
    assert_eq!(s.take_dirty(), vec![2]);
}

#[test]
fn reception_en_boucle_locale() {
    // ports à part et adresse locale : aucun accès au réseau de l'ordinateur
    let cfg = Config { bind: Ipv4Addr::LOCALHOST, artnet_port: 46454, sacn_port: 45568, ..Config::default() };
    let rx = Receiver::start(cfg);
    rx.set_universes(&[1, 3]);
    std::thread::sleep(Duration::from_millis(200));
    let client = UdpSocket::bind("127.0.0.1:0").unwrap();
    client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    client.send_to(&artnet::build_dmx(0, 0, &[255, 64]), "127.0.0.1:46454").unwrap();
    client.send_to(&sacn::build(3, 100, 0, "essai", [1; 16], &[7, 8, 9]), "127.0.0.1:45568").unwrap();
    // découverte : la réponse revient au port de la demande
    client.send_to(&artnet::build_poll(), "127.0.0.1:46454").unwrap();
    let mut buf = [0u8; 600];
    let (n, _) = client.recv_from(&mut buf).expect("ArtPollReply");
    assert!(matches!(artnet::parse(&buf[..n]), Some(artnet::Packet::PollReply { .. })));
    let (mut dmx, mut live) = (HashMap::new(), HashSet::new());
    let deadline = Instant::now() + Duration::from_secs(3);
    while (dmx.len() < 2) && Instant::now() < deadline {
        rx.snapshot(&mut dmx, &mut live);
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(&dmx[&1][..2], &[255, 64], "Art-Net port-address 0 → univers 1");
    assert_eq!(&dmx[&3][..3], &[7, 8, 9]);
    assert!(live.contains(&1) && live.contains(&3));
    let st = rx.status();
    assert_eq!((st.artnet_packets, st.sacn_packets, st.polls), (1, 1, 1));
}
