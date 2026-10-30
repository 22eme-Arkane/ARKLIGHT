//! sACN / ANSI E1.31 (portage de `server/dmx/sacn.js`) : lecture des paquets de données,
//! groupes multicast 239.255.x.y.

use std::net::Ipv4Addr;

pub const PORT: u16 = 5568;
const ACN_PID: &[u8; 12] = b"ASC-E1.17\0\0\0";
const VECTOR_ROOT_E131_DATA: u32 = 0x0000_0004;
const VECTOR_E131_DATA_PACKET: u32 = 0x0000_0002;

/// Groupe multicast d'un univers.
pub fn multicast_group(universe: u16) -> Ipv4Addr {
    Ipv4Addr::new(239, 255, (universe >> 8) as u8, (universe & 0xff) as u8)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Packet<'a> {
    Dmx { universe: u16, priority: u8, sequence: u8, data: &'a [u8], source_name: String, cid: [u8; 16] },
    /// la source s'arrête (option « Stream_Terminated »)
    Terminated { universe: u16, cid: [u8; 16] },
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Lit un paquet de données E1.31 ; None pour tout le reste (prévisualisation, autre start code…).
pub fn parse(msg: &[u8]) -> Option<Packet<'_>> {
    if msg.len() < 126 || &msg[4..16] != ACN_PID || be32(msg, 18) != VECTOR_ROOT_E131_DATA || be32(msg, 40) != VECTOR_E131_DATA_PACKET || msg[117] != 0x02 {
        return None;
    }
    let options = msg[112];
    if options & 0x80 != 0 {
        return None; // données de prévisualisation
    }
    let universe = u16::from_be_bytes([msg[113], msg[114]]);
    let mut cid = [0u8; 16];
    cid.copy_from_slice(&msg[22..38]);
    if options & 0x40 != 0 {
        return Some(Packet::Terminated { universe, cid });
    }
    if msg[125] != 0x00 {
        return None; // seulement le start code DMX standard
    }
    let count = (u16::from_be_bytes([msg[123], msg[124]]) as usize).saturating_sub(1).min(512).min(msg.len() - 126);
    let source_name = String::from_utf8_lossy(&msg[44..108]).trim_end_matches('\0').split('\0').next().unwrap_or("").to_owned();
    Some(Packet::Dmx { universe, priority: msg[108], sequence: msg[111], data: &msg[126..126 + count], source_name, cid })
}

/// Paquet de données E1.31 (pour les essais et, plus tard, la sortie sACN).
pub fn build(universe: u16, priority: u8, sequence: u8, source_name: &str, cid: [u8; 16], data: &[u8]) -> Vec<u8> {
    let n = data.len().min(512);
    let total = 126 + n;
    let mut b = vec![0u8; total];
    b[1] = 0x10; // taille du préambule
    b[4..16].copy_from_slice(ACN_PID);
    let flags = |len: usize| (0x7000u16 | len as u16).to_be_bytes();
    b[16..18].copy_from_slice(&flags(total - 16));
    b[18..22].copy_from_slice(&VECTOR_ROOT_E131_DATA.to_be_bytes());
    b[22..38].copy_from_slice(&cid);
    b[38..40].copy_from_slice(&flags(total - 38));
    b[40..44].copy_from_slice(&VECTOR_E131_DATA_PACKET.to_be_bytes());
    for (i, c) in source_name.bytes().take(63).enumerate() {
        b[44 + i] = c;
    }
    b[108] = priority;
    b[111] = sequence;
    b[113..115].copy_from_slice(&universe.to_be_bytes());
    b[115..117].copy_from_slice(&flags(total - 115));
    b[117] = 0x02;
    b[118] = 0xa1;
    b[121..123].copy_from_slice(&1u16.to_be_bytes());
    b[123..125].copy_from_slice(&((n + 1) as u16).to_be_bytes());
    b[126..].copy_from_slice(&data[..n]);
    b
}
