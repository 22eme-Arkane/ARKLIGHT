//! Art-Net 4 (portage de `server/dmx/artnet.js`) : lecture des paquets ArtDmx et ArtPoll,
//! fabrication des ArtPollReply qui annoncent le simulateur comme un node. RDM sur Art-Net
//! (ArtTodRequest, ArtTodControl → ArtTodData ; ArtRdm) : les projecteurs virtuels se laissent
//! découvrir et ré-adresser (voir `rdm`).

use std::net::Ipv4Addr;

pub const PORT: u16 = 6454;
const HEADER: &[u8; 8] = b"Art-Net\0";
const OP_POLL: u16 = 0x2000;
const OP_POLL_REPLY: u16 = 0x2100;
const OP_DMX: u16 = 0x5000;
const OP_TOD_REQUEST: u16 = 0x8000;
const OP_TOD_DATA: u16 = 0x8100;
const OP_TOD_CONTROL: u16 = 0x8200;
const OP_RDM: u16 = 0x8300;
const PROT_VER: u8 = 14;
/// UID par paquet ArtTodData
const MAX_UIDS: usize = 200;

/// Paquet Art-Net reçu.
#[derive(Debug, Clone, PartialEq)]
pub enum Packet<'a> {
    /// trame DMX d'un port-address (0..32767)
    Dmx { port_address: u16, sequence: u8, data: &'a [u8] },
    /// demande de découverte : répondre par un ArtPollReply
    Poll,
    /// un autre node se présente
    PollReply { ip: Ipv4Addr, name: String },
    /// RDM : table des appareils demandée pour ces port-address (ArtTodRequest)
    TodRequest { port_addresses: Vec<u16> },
    /// RDM : commande sur la table d'un port-address (ArtTodControl) ; on renvoie la table
    TodControl { port_address: u16, command: u8 },
    /// RDM : message pour un port-address (ArtRdm), sans le code de départ 0xCC
    Rdm { port_address: u16, data: &'a [u8] },
    /// autre paquet (ArtSync… : sans effet ici)
    Other(u16),
}

/// Lit un paquet ; None si ce n'est pas de l'Art-Net.
pub fn parse(msg: &[u8]) -> Option<Packet<'_>> {
    if msg.len() < 12 || &msg[..8] != HEADER {
        return None;
    }
    let op = u16::from_le_bytes([msg[8], msg[9]]);
    Some(match op {
        OP_DMX => {
            if msg.len() < 18 {
                return None;
            }
            let port_address = (((msg[15] & 0x7f) as u16) << 8) | msg[14] as u16;
            let length = (((msg[16] as usize) << 8) | msg[17] as usize).min(512).min(msg.len() - 18);
            Packet::Dmx { port_address, sequence: msg[12], data: &msg[18..18 + length] }
        }
        OP_POLL => Packet::Poll,
        OP_TOD_REQUEST | OP_TOD_CONTROL => {
            if msg.len() < 24 {
                return None;
            }
            let net = ((msg[21] & 0x7f) as u16) << 8;
            if op == OP_TOD_CONTROL {
                Packet::TodControl { port_address: net | msg[23] as u16, command: msg[22] }
            } else {
                let count = (msg[23] as usize).min(32);
                let subs = &msg[24..(24 + count).min(msg.len())];
                Packet::TodRequest { port_addresses: subs.iter().map(|s| net | *s as u16).collect() }
            }
        }
        OP_RDM => {
            if msg.len() < 24 + 25 {
                return None;
            }
            Packet::Rdm { port_address: (((msg[21] & 0x7f) as u16) << 8) | msg[23] as u16, data: &msg[24..] }
        }
        OP_POLL_REPLY if msg.len() >= 44 => {
            let name: String = msg[26..44].iter().take_while(|b| **b != 0).map(|b| *b as char).collect();
            Packet::PollReply { ip: Ipv4Addr::new(msg[10], msg[11], msg[12], msg[13]), name }
        }
        other => Packet::Other(other),
    })
}

/// Paquet ArtDmx (pour les essais et, plus tard, la sortie DMX).
pub fn build_dmx(port_address: u16, sequence: u8, data: &[u8]) -> Vec<u8> {
    let len = data.len().min(512);
    let even = len + (len & 1);
    let mut b = vec![0u8; 18 + even];
    b[..8].copy_from_slice(HEADER);
    b[8..10].copy_from_slice(&OP_DMX.to_le_bytes());
    b[11] = 14;
    b[12] = sequence;
    b[14] = (port_address & 0xff) as u8;
    b[15] = ((port_address >> 8) & 0x7f) as u8;
    b[16] = (even >> 8) as u8;
    b[17] = (even & 0xff) as u8;
    b[18..18 + len].copy_from_slice(&data[..len]);
    b
}

/// Paquet ArtPoll (découverte des nodes).
pub fn build_poll() -> Vec<u8> {
    let mut b = vec![0u8; 14];
    b[..8].copy_from_slice(HEADER);
    b[8..10].copy_from_slice(&OP_POLL.to_le_bytes());
    b[11] = 14;
    b
}

fn header(op: u16, len: usize) -> Vec<u8> {
    let mut b = vec![0u8; len];
    b[..8].copy_from_slice(HEADER);
    b[8..10].copy_from_slice(&op.to_le_bytes());
    b[11] = PROT_VER;
    b
}

/// ArtTodRequest : demande la table des appareils RDM de ces port-address (32 au plus, même Net).
pub fn build_tod_request(port_addresses: &[u16]) -> Vec<u8> {
    let subs = &port_addresses[..port_addresses.len().min(32)];
    let mut b = header(OP_TOD_REQUEST, 24 + subs.len());
    b[21] = subs.first().map_or(0, |pa| ((pa >> 8) & 0x7f) as u8); // Net
    b[23] = subs.len() as u8; // AdCount
    for (i, pa) in subs.iter().enumerate() {
        b[24 + i] = (pa & 0xff) as u8;
    }
    b
}

/// ArtTodData : table des appareils d'un port-address, en paquets de 200 UID (au moins un paquet).
pub fn build_tod_data(port_address: u16, uids: &[[u8; 6]]) -> Vec<Vec<u8>> {
    let blocks = uids.len().div_ceil(MAX_UIDS).max(1);
    (0..blocks)
        .map(|block| {
            let part = &uids[(block * MAX_UIDS).min(uids.len())..((block + 1) * MAX_UIDS).min(uids.len())];
            let mut b = header(OP_TOD_DATA, 28 + part.len() * 6);
            b[12] = 0x01; // RdmVer : RDM standard
            b[13] = 1; // port physique
            b[20] = 1; // BindIndex
            b[21] = ((port_address >> 8) & 0x7f) as u8; // Net
            b[22] = 0x00; // TodFull
            b[23] = (port_address & 0xff) as u8; // Sub-Net + Univers
            b[24..26].copy_from_slice(&(uids.len() as u16).to_be_bytes());
            b[26] = block as u8;
            b[27] = part.len() as u8;
            for (i, u) in part.iter().enumerate() {
                b[28 + i * 6..34 + i * 6].copy_from_slice(u);
            }
            b
        })
        .collect()
}

/// ArtRdm : message RDM (sans 0xCC) pour un port-address.
pub fn build_rdm(port_address: u16, data: &[u8]) -> Vec<u8> {
    let mut b = header(OP_RDM, 24 + data.len());
    b[12] = 0x01; // RdmVer
    b[21] = ((port_address >> 8) & 0x7f) as u8; // Net
    b[22] = 0x00; // ArProcess
    b[23] = (port_address & 0xff) as u8;
    b[24..].copy_from_slice(data);
    b
}

/// Description du node annoncée aux consoles.
pub struct Node<'a> {
    pub short_name: &'a str,
    pub long_name: &'a str,
    pub ip: Ipv4Addr,
    pub mac: [u8; 6],
    /// trames reçues (compteur affiché dans le rapport du node)
    pub packets: u64,
}

fn put_str(b: &mut [u8], at: usize, max: usize, s: &str) {
    for (i, c) in s.chars().filter(|c| c.is_ascii()).take(max).enumerate() {
        b[at + i] = c as u8;
    }
}

/// ArtPollReply : un paquet par groupe d'au plus 4 ports partageant le même Net / Sub-Net.
pub fn build_poll_replies(node: &Node, port_addresses: &[u16]) -> Vec<Vec<u8>> {
    build_poll_replies_rdm(node, port_addresses, false)
}

/// ArtPollReply, en annonçant le RDM (`rdm` : bit 1 de Status1) quand les projecteurs y répondent.
pub fn build_poll_replies_rdm(node: &Node, port_addresses: &[u16], rdm: bool) -> Vec<Vec<u8>> {
    let mut addrs: Vec<u16> = port_addresses.to_vec();
    addrs.sort_unstable();
    addrs.dedup();
    if addrs.is_empty() {
        addrs.push(0);
    }
    let mut groups: Vec<Vec<u16>> = Vec::new();
    for pa in addrs {
        match groups.last_mut() {
            Some(last) if last.len() < 4 && last[0] >> 4 == pa >> 4 => last.push(pa),
            _ => groups.push(vec![pa]),
        }
    }
    groups
        .iter()
        .enumerate()
        .map(|(i, ports)| {
            let mut b = vec![0u8; 239];
            b[..8].copy_from_slice(HEADER);
            b[8..10].copy_from_slice(&OP_POLL_REPLY.to_le_bytes());
            b[10..14].copy_from_slice(&node.ip.octets());
            b[14..16].copy_from_slice(&PORT.to_le_bytes());
            b[17] = 1; // VersInfoL
            b[18] = ((ports[0] >> 8) & 0x7f) as u8; // NetSwitch
            b[19] = ((ports[0] >> 4) & 0x0f) as u8; // SubSwitch
            b[20..22].copy_from_slice(&0x00ffu16.to_be_bytes()); // OEM inconnu
            b[23] = if rdm { 0xd2 } else { 0xd0 }; // Status1 : indicateurs normaux, programmation réseau, RDM
            b[24..26].copy_from_slice(&0x7ff0u16.to_le_bytes()); // code ESTA « prototype »
            put_str(&mut b, 26, 17, node.short_name);
            put_str(&mut b, 44, 63, node.long_name);
            put_str(&mut b, 108, 63, &format!("#0001 [{:04}] ARKLIGHT OK", node.packets % 10000));
            b[173] = ports.len() as u8;
            for (k, pa) in ports.iter().enumerate() {
                b[174 + k] = 0x80; // PortTypes : sortie DMX512 depuis Art-Net
                b[182 + k] = 0x80; // GoodOutputA : données en sortie
                b[190 + k] = (pa & 0x0f) as u8; // SwOut
            }
            b[200] = 0x00; // Style : StNode
            b[201..207].copy_from_slice(&node.mac);
            b[207..211].copy_from_slice(&node.ip.octets()); // BindIp
            b[211] = (i + 1) as u8; // BindIndex
            b[212] = 0x0e; // Status2 : DHCP, Port-Address 15 bits
            b
        })
        .collect()
}
