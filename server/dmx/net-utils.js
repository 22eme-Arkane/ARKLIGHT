import os from 'node:os';

function ipToInt(ip) {
  return ip.split('.').reduce((acc, oct) => ((acc << 8) | (Number(oct) & 0xff)) >>> 0, 0);
}

/** Interfaces IPv4 actives (hors loopback). */
export function ipv4Interfaces() {
  const out = [];
  for (const [name, addrs] of Object.entries(os.networkInterfaces())) {
    for (const a of addrs || []) {
      if (a.family === 'IPv4' && !a.internal) out.push({ name, address: a.address, netmask: a.netmask, mac: a.mac });
    }
  }
  return out;
}

/** Choisit l'interface locale qui se trouve sur le même sous-réseau que `remoteIp`. */
export function localInterfaceFor(remoteIp) {
  if (!remoteIp || remoteIp.startsWith('127.')) return { address: '127.0.0.1', netmask: '255.0.0.0', mac: '00:00:00:00:00:00' };
  const remote = ipToInt(remoteIp);
  const ifaces = ipv4Interfaces();
  for (const i of ifaces) {
    const mask = ipToInt(i.netmask);
    if ((ipToInt(i.address) & mask) === (remote & mask)) return i;
  }
  return ifaces[0] || { address: '0.0.0.0', netmask: '0.0.0.0', mac: '00:00:00:00:00:00' };
}
