// Export MVR (My Virtual Rig, DIN SPEC 15801, version 1.6) : le plan d'éclairage avec son patch,
// lisible par Blackout, grandMA3, Vectorworks, Capture, ChamSys, Eos… L'archive contient
// GeneralSceneDescription.xml et un fichier GDTF (DIN SPEC 15800) par modèle de projecteur, décrit
// à partir des profils DMX de la bibliothèque (seulement les modes utilisés dans la scène).
//
// Repères : ARKLIGHT (three.js) a Y vers le haut et Z vers le public ; MVR a Z vers le haut,
// Y vers le lointain, des millimètres. Un projecteur GDTF « au repos » éclaire vers le bas (−Z),
// un projecteur ARKLIGHT non tourné éclaire vers le public : on passe de l'un à l'autre.
import { compileMode } from './engine/dmx.js';
import { getFixtureType, getMode } from './fixtures/index.js';
import { createZip } from './zip.js';

// ------------------------------------------------------------------ identifiants stables
/** UUID (texte RFC 4122) tiré d'un texte : le même projecteur garde le même UUID d'un export à l'autre. */
export function uuidFrom(text) {
  let h1 = 0x811c9dc5;
  let h2 = 0x01000193;
  let h3 = 0x9e3779b9;
  let h4 = 0x85ebca6b;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    h1 = Math.imul(h1 ^ c, 0x01000193);
    h2 = Math.imul(h2 ^ c, 0x5bd1e995);
    h3 = Math.imul(h3 + c, 0x27d4eb2d) ^ (h3 >>> 15);
    h4 = Math.imul(h4 ^ (c + i), 0xc2b2ae35) ^ (h4 >>> 13);
  }
  const hex = [h1, h2, h3, h4].map((v) => (v >>> 0).toString(16).padStart(8, '0')).join('');
  const b = hex.split('');
  b[12] = '5'; // version (identifiant dérivé d'un nom)
  b[16] = ((parseInt(b[16], 16) & 0x3) | 0x8).toString(16); // variante RFC 4122
  const s = b.join('').toUpperCase();
  return `${s.slice(0, 8)}-${s.slice(8, 12)}-${s.slice(12, 16)}-${s.slice(16, 20)}-${s.slice(20)}`;
}

const xmlEsc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[c]);
const num = (v, d = 6) => String(Math.round(v * 10 ** d) / 10 ** d);
/** Nom de fichier sans caractères gênants. */
const fileSafe = (s) => String(s).normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/[^A-Za-z0-9._-]+/g, '_').replace(/^_+|_+$/g, '') || 'Fixture';

// ------------------------------------------------------------------ GDTF : attributs
// fonction d'un canal ARKLIGHT → attribut GDTF, groupe de fonctions, unité physique
const ATTRS = {
  dimmer: ['Dimmer', 'Dimmer', 'Dimmer', 'LuminousIntensity', 0, 1],
  red: ['ColorAdd_R', 'Color', 'RGB', 'ColorComponent', 0, 1],
  green: ['ColorAdd_G', 'Color', 'RGB', 'ColorComponent', 0, 1],
  blue: ['ColorAdd_B', 'Color', 'RGB', 'ColorComponent', 0, 1],
  white: ['ColorAdd_W', 'Color', 'RGB', 'ColorComponent', 0, 1],
  amber: ['ColorAdd_RY', 'Color', 'RGB', 'ColorComponent', 0, 1],
  lime: ['ColorAdd_GY', 'Color', 'RGB', 'ColorComponent', 0, 1],
  mint: ['ColorAdd_GC', 'Color', 'RGB', 'ColorComponent', 0, 1],
  cyan: ['ColorAdd_C', 'Color', 'RGB', 'ColorComponent', 0, 1],
  magenta: ['ColorAdd_M', 'Color', 'RGB', 'ColorComponent', 0, 1],
  yellow: ['ColorAdd_Y', 'Color', 'RGB', 'ColorComponent', 0, 1],
  indigo: ['ColorAdd_BM', 'Color', 'RGB', 'ColorComponent', 0, 1],
  uv: ['ColorAdd_UV', 'Color', 'RGB', 'ColorComponent', 0, 1],
  ww: ['ColorAdd_WW', 'Color', 'RGB', 'ColorComponent', 0, 1],
  cw: ['ColorAdd_CW', 'Color', 'RGB', 'ColorComponent', 0, 1],
  cct: ['CTC', 'Color', 'Color', 'Temperature', 2000, 10000],
  tint: ['Tint', 'Color', 'Color', 'None', -1, 1],
  hue: ['HSB_Hue', 'Color', 'HSB', 'Angle', 0, 360],
  sat: ['HSB_Saturation', 'Color', 'HSB', 'Percent', 0, 100],
  x: ['CIE_X', 'Color', 'CIE', 'None', 0, 1],
  y: ['CIE_Y', 'Color', 'CIE', 'None', 0, 1],
  crossfade: ['ColorMixMode', 'Color', 'Color', 'None', 0, 1],
  index: ['ColorMacro1', 'Color', 'Color', 'None', 0, 1],
  cctIndex: ['ColorMacro2', 'Color', 'Color', 'None', 0, 1],
  strobe: ['Shutter1Strobe', 'Beam', 'Beam', 'Frequency', 0, 25],
  fan: ['Fans', 'Control', 'Control', 'None', 0, 1],
  fxProgram: ['Effects1', 'Control', 'Control', 'None', 0, 1],
  fxIndex: ['Effects2', 'Control', 'Control', 'None', 0, 1],
  fxSpeed: ['Effects1Rate', 'Control', 'Control', 'None', 0, 1],
  fxFade: ['Effects1Fade', 'Control', 'Control', 'None', 0, 1],
  fxDirection: ['Effects1Adjust1', 'Control', 'Control', 'None', 0, 1],
  fxSize: ['Effects1Adjust2', 'Control', 'Control', 'None', 0, 1],
  fxOffset: ['Effects1Adjust3', 'Control', 'Control', 'None', 0, 1],
  fxRestart: ['Effects1Adjust4', 'Control', 'Control', 'None', 0, 1],
  fxR: ['EffectsColor_R', 'Control', 'Control', 'ColorComponent', 0, 1],
  fxG: ['EffectsColor_G', 'Control', 'Control', 'ColorComponent', 0, 1],
  fxB: ['EffectsColor_B', 'Control', 'Control', 'ColorComponent', 0, 1],
  none: ['NoFeature', 'Control', 'Control', 'None', 0, 1],
};
const attrOf = (f) => ATTRS[f] || ['NoFeature', 'Control', 'Control', 'None', 0, 1];

/** Nom du modèle sans la marque devant (« ARRI M18 » → « M18 »). */
const modelName = (type) => (type.name.toLowerCase().startsWith(`${type.brand.toLowerCase()} `) ? type.name.slice(type.brand.length + 1) : type.name);
/** Nom du mode tel qu'il apparaît dans la console (unique pour le modèle). */
export const modeName = (mode) => String(mode.label || mode.name || mode.id);

/** Fichier GDTF (description.xml) d'un modèle, avec les modes demandés. */
export function buildGdtf(type, modes) {
  const attrs = new Map(); // nom → { group, feature, unit }
  const dmxModes = [];
  let pixelCount = 1;
  for (const mode of modes) {
    const { slots } = compileMode(mode);
    const used = new Set();
    const channels = [];
    let offset = 1;
    for (const slot of slots) {
      const [base, group, feature, unit, from, to] = attrOf(slot.f);
      const geometry = slot.px >= 0 ? `Pixel ${slot.px + 1}` : 'Body';
      if (slot.px >= 0) pixelCount = Math.max(pixelCount, slot.px + 1);
      // un même attribut deux fois sur la même géométrie : on le numérote (attribut propre au fichier)
      let attr = base;
      for (let k = 2; used.has(`${geometry}|${attr}`); k++) attr = `${base}${k}`;
      used.add(`${geometry}|${attr}`);
      attrs.set(attr, { group, feature, unit, pretty: slot.def?.label || attr });
      const bytes = slot.bytes || 1;
      const offsets = bytes === 2 ? `${offset},${offset + 1}` : `${offset}`;
      offset += bytes;
      const label = xmlEsc(slot.def?.label || attr);
      channels.push(
        `<DMXChannel DMXBreak="1" Offset="${offsets}" Highlight="None" Geometry="${xmlEsc(geometry)}">` +
          `<LogicalChannel Attribute="${xmlEsc(attr)}" Snap="No" Master="None" MibFade="0" DMXChangeTimeLimit="0">` +
          `<ChannelFunction Name="${label}" Attribute="${xmlEsc(attr)}" OriginalAttribute="" DMXFrom="0/${bytes}" Default="0/${bytes}" PhysicalFrom="${from}" PhysicalTo="${to}" RealFade="0" RealAcceleration="0"/>` +
          `</LogicalChannel></DMXChannel>`,
      );
    }
    dmxModes.push(`<DMXMode Name="${xmlEsc(modeName(mode))}" Description="${xmlEsc(mode.name || '')}" Geometry="Body"><DMXChannels>${channels.join('')}</DMXChannels><Relations/><FTMacros/></DMXMode>`);
  }
  const groups = new Map();
  for (const a of attrs.values()) {
    if (!groups.has(a.group)) groups.set(a.group, new Set());
    groups.get(a.group).add(a.feature);
  }
  const g = type.geometry || {};
  const L = g.length || g.depth || g.diameter || 0.3;
  const W = g.width || g.diameter || 0.3;
  const H = g.height || g.diameter || 0.2;
  const beamAngle = type.beamAngle || 40;
  const pixels =
    pixelCount > 1
      ? Array.from({ length: pixelCount }, (_, i) => {
          const x = g.kind === 'tube' ? -L / 2 + (L * (i + 0.5)) / pixelCount : 0;
          return `<Beam Name="Pixel ${i + 1}" Model="Pixel" Position="{1,0,0,${num(x)}}{0,1,0,0}{0,0,1,0}{0,0,0,1}" LampType="LED" BeamAngle="${beamAngle}" FieldAngle="${beamAngle}" BeamRadius="0.02" BeamType="Wash" ColorTemperature="5600" ColorRenderingIndex="90"/>`;
        }).join('')
      : `<Beam Name="Beam" Model="Pixel" Position="{1,0,0,0}{0,1,0,0}{0,0,1,${num(-H / 2)}}{0,0,0,1}" LampType="${type.technology === 'hmi' ? 'Discharge' : type.technology === 'tungsten' ? 'Tungsten' : 'LED'}" PowerConsumption="${type.watts || 100}" LuminousFlux="${type.output?.lumens || 1000}" BeamAngle="${beamAngle}" FieldAngle="${beamAngle}" BeamRadius="${num(Math.min(W, L) / 2, 3)}" BeamType="Wash" ColorTemperature="${type.lamp?.cct || 5600}" ColorRenderingIndex="90"/>`;
  const xml =
    `<?xml version="1.0" encoding="UTF-8" standalone="no" ?>\n` +
    `<GDTF DataVersion="1.2">` +
    `<FixtureType Name="${xmlEsc(modelName(type))}" ShortName="${xmlEsc(type.model || modelName(type))}" LongName="${xmlEsc(`${type.brand} ${modelName(type)}`)}" Manufacturer="${xmlEsc(type.brand)}" Description="${xmlEsc(`Exporté par ARKLIGHT — ${type.id}`)}" FixtureTypeID="${uuidFrom(`gdtf:${type.id}`)}" Thumbnail="" RefFT="" CanHaveChildren="Yes">` +
    `<AttributeDefinitions><ActivationGroups/><FeatureGroups>${[...groups].map(([grp, feats]) => `<FeatureGroup Name="${grp}" Pretty="${grp}">${[...feats].map((f) => `<Feature Name="${f}"/>`).join('')}</FeatureGroup>`).join('')}</FeatureGroups>` +
    `<Attributes>${[...attrs].map(([name, a]) => `<Attribute Name="${xmlEsc(name)}" Pretty="${xmlEsc(a.pretty).slice(0, 24)}" Feature="${a.group}.${a.feature}" PhysicalUnit="${a.unit}"/>`).join('')}</Attributes></AttributeDefinitions>` +
    `<Wheels/><PhysicalDescriptions/>` +
    `<Models><Model Name="Body" Length="${num(L, 3)}" Width="${num(W, 3)}" Height="${num(H, 3)}" PrimitiveType="Cube"/><Model Name="Pixel" Length="${num(Math.min(0.05, L), 3)}" Width="${num(Math.min(0.05, W), 3)}" Height="0.01" PrimitiveType="Cylinder"/></Models>` +
    `<Geometries><Geometry Name="Body" Model="Body" Position="{1,0,0,0}{0,1,0,0}{0,0,1,0}{0,0,0,1}">${pixels}</Geometry></Geometries>` +
    `<DMXModes>${dmxModes.join('')}</DMXModes>` +
    `<Revisions><Revision Text="ARKLIGHT" Date="${new Date().toISOString().slice(0, 19)}" UserID="0"/></Revisions><FTPresets/><Protocols/>` +
    `</FixtureType></GDTF>`;
  return xml;
}

// ------------------------------------------------------------------ position et orientation
/** Rotation three.js (Euler XYZ, radians) → matrice 3×3 (lignes). */
function eulerMatrix([a = 0, b = 0, c = 0]) {
  const [ca, sa, cb, sb, cc, sc] = [Math.cos(a), Math.sin(a), Math.cos(b), Math.sin(b), Math.cos(c), Math.sin(c)];
  return [
    [cb * cc, -cb * sc, sb],
    [ca * sc + sa * sb * cc, ca * cc - sa * sb * sc, -sa * cb],
    [sa * sc - ca * sb * cc, sa * cc + ca * sb * sc, ca * cb],
  ];
}
const mul = (A, B) => A.map((row) => [0, 1, 2].map((j) => row[0] * B[0][j] + row[1] * B[1][j] + row[2] * B[2][j]));
const T = (A) => [0, 1, 2].map((i) => [0, 1, 2].map((j) => A[j][i]));
const C = [
  [1, 0, 0],
  [0, 0, -1],
  [0, 1, 0],
]; // (x, y, z) three.js → (x, −z, y) MVR
const M0 = [
  [1, 0, 0],
  [0, 0, 1],
  [0, -1, 0],
]; // projecteur GDTF (faisceau vers −Z) → projecteur ARKLIGHT non tourné (faisceau vers le public)

/** Matrice MVR « {u}{v}{w}{o} » d'un élément placé dans ARKLIGHT (mètres, rotation XYZ). */
export function mvrMatrix(position, rotation, { fixture = true } = {}) {
  let M = mul(mul(C, eulerMatrix(rotation)), T(C));
  if (fixture) M = mul(M, M0);
  const col = (j) => [M[0][j], M[1][j], M[2][j]];
  const [x, y, z] = position;
  const o = [x * 1000, -z * 1000, y * 1000];
  return [col(0), col(1), col(2), o].map((v) => `{${v.map((n) => num(n, 6)).join(',')}}`).join('');
}

// ------------------------------------------------------------------ scène MVR
/**
 * Fichiers de l'export MVR d'une scène ARKLIGHT.
 * @returns {{ files: { name: string, data: string|Uint8Array }[], fixtures: number, skipped: string[] }}
 */
export function buildMvrFiles(scene, { provider = 'ARKLIGHT', providerVersion = '0', layerName = 'Projecteurs' } = {}) {
  const byType = new Map(); // id du modèle → { type, modes: Map(modeId → mode), file }
  const skipped = [];
  const fixtures = [];
  for (const f of scene.fixtures || []) {
    const type = getFixtureType(f.type);
    const mode = type && getMode(type, f.settings?.dmxMode);
    if (!type || !mode) {
      skipped.push(f.name); // boîtiers sans DMX (AsteraBox…)
      continue;
    }
    if (!byType.has(type.id)) byType.set(type.id, { type, modes: new Map() });
    byType.get(type.id).modes.set(mode.id, mode);
    fixtures.push({ f, type, mode });
  }
  // noms de fichiers GDTF uniques : « Marque@Modèle.gdtf »
  const names = new Set();
  for (const entry of byType.values()) {
    let name = `${fileSafe(entry.type.brand)}@${fileSafe(modelName(entry.type))}`;
    for (let k = 2; names.has(name.toLowerCase()); k++) name = `${fileSafe(entry.type.brand)}@${fileSafe(modelName(entry.type))}_${k}`;
    names.add(name.toLowerCase());
    entry.file = `${name}.gdtf`;
  }
  const fixtureXml = fixtures
    .map(({ f, type, mode }) => {
      const s = f.settings || {};
      const universe = Math.max(1, Number(s.universe) || 1);
      const address = Math.min(512, Math.max(1, Number(s.dmxAddress) || 1));
      const fix = Number(s.fixtureNumber) || 0;
      return (
        `<Fixture name="${xmlEsc(f.name)}" uuid="${uuidFrom(`fixture:${f.id}`)}">` +
        `<Matrix>${mvrMatrix(f.position || [0, 0, 0], f.rotation || [0, 0, 0])}</Matrix>` +
        `<GDTFSpec>${xmlEsc(byType.get(type.id).file)}</GDTFSpec>` +
        `<GDTFMode>${xmlEsc(modeName(mode))}</GDTFMode>` +
        `<Addresses><Address break="0">${(universe - 1) * 512 + address}</Address></Addresses>` +
        `<FixtureID>${fix || ''}</FixtureID><FixtureIDNumeric>${fix}</FixtureIDNumeric><UnitNumber>0</UnitNumber>` +
        `<CustomId>${fix}</CustomId><CastShadow>false</CastShadow>` +
        `</Fixture>`
      );
    })
    .join('');
  const scenexml =
    `<?xml version="1.0" encoding="UTF-8" standalone="no" ?>\n` +
    `<GeneralSceneDescription verMajor="1" verMinor="6" provider="${xmlEsc(provider)}" providerVersion="${xmlEsc(providerVersion)}">` +
    `<UserData><Data provider="${xmlEsc(provider)}" ver="${xmlEsc(providerVersion)}"/></UserData>` +
    `<Scene><AUXData/><Layers>` +
    `<Layer name="${xmlEsc(layerName)}" uuid="${uuidFrom(`layer:${scene.name || ''}`)}"><ChildList>${fixtureXml}</ChildList></Layer>` +
    `</Layers></Scene></GeneralSceneDescription>`;
  const files = [{ name: 'GeneralSceneDescription.xml', data: scenexml }];
  for (const entry of byType.values()) files.push({ name: entry.file, data: createZip([{ name: 'description.xml', data: buildGdtf(entry.type, [...entry.modes.values()]) }]) });
  return { files, fixtures: fixtures.length, skipped };
}

/** Archive MVR prête à enregistrer. */
export function buildMvr(scene, opts) {
  const { files, fixtures, skipped } = buildMvrFiles(scene, opts);
  return { bytes: createZip(files), fixtures, skipped };
}
