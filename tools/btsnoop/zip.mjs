// Lecture minimale d'une archive ZIP (rapport de bug Android) pour en extraire les journaux btsnoop.
import { inflateRawSync } from 'node:zlib';

const EOCD_SIG = 0x06054b50;
const CEN_SIG = 0x02014b50;
const LOC_SIG = 0x04034b50;

export function isZip(buf) {
  return buf.length > 4 && buf.readUInt32LE(0) === LOC_SIG;
}

/** Liste des fichiers de l'archive : [{ name, method, compressedSize, size, offset }] */
export function listEntries(buf) {
  let eocd = -1;
  for (let i = buf.length - 22; i >= Math.max(0, buf.length - 65557); i--) {
    if (buf.readUInt32LE(i) === EOCD_SIG) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) throw new Error('Archive ZIP illisible (fin de répertoire introuvable)');
  const count = buf.readUInt16LE(eocd + 10);
  let p = buf.readUInt32LE(eocd + 16);
  const entries = [];
  for (let k = 0; k < count; k++) {
    if (buf.readUInt32LE(p) !== CEN_SIG) break;
    const method = buf.readUInt16LE(p + 10);
    const compressedSize = buf.readUInt32LE(p + 20);
    const size = buf.readUInt32LE(p + 24);
    const nameLen = buf.readUInt16LE(p + 28);
    const extraLen = buf.readUInt16LE(p + 30);
    const commentLen = buf.readUInt16LE(p + 32);
    const offset = buf.readUInt32LE(p + 42);
    const name = buf.toString('utf8', p + 46, p + 46 + nameLen);
    entries.push({ name, method, compressedSize, size, offset });
    p += 46 + nameLen + extraLen + commentLen;
  }
  return entries;
}

export function readEntry(buf, entry) {
  const p = entry.offset;
  if (buf.readUInt32LE(p) !== LOC_SIG) throw new Error(`Entrée ZIP corrompue : ${entry.name}`);
  const start = p + 30 + buf.readUInt16LE(p + 26) + buf.readUInt16LE(p + 28);
  const data = buf.subarray(start, start + entry.compressedSize);
  if (entry.method === 0) return Buffer.from(data);
  if (entry.method === 8) return inflateRawSync(data);
  throw new Error(`Compression ZIP non gérée (${entry.method}) pour ${entry.name}`);
}

/** Journaux btsnoop contenus dans un rapport de bug Android (le plus gros en premier). */
export function findBtsnoopLogs(buf) {
  return listEntries(buf)
    .filter((e) => /btsnoop/i.test(e.name) && !e.name.endsWith('/'))
    .sort((a, b) => b.size - a.size)
    .map((e) => ({ name: e.name, data: readEntry(buf, e) }))
    .filter((e) => e.data.toString('latin1', 0, 8) === 'btsnoop\0');
}
