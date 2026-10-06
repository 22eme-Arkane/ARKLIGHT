// Dessin du plan de feu en SVG : à l'écran (thème sombre) ou pour l'export et l'impression (fond blanc).
import { getFixtureType } from '/shared/fixtures/index.js';
import { CLOTHS, getGripType } from '/shared/grip.js';
import { SUPPORTS } from '/shared/fixtures/supports.js';
import { linearToSrgb } from '/shared/engine/color.js';
import { patchLabel } from '../ui/patch.js';
import { fixturePlan, objectPlan } from './geometry.js';

export const THEMES = {
  screen: { bg: '#0f1216', minor: '#1a1f26', major: '#29303a', text: '#d9dde3', muted: '#8a919c', body: '#e6e9ee', bodyFill: '#1d2229', sel: '#ffad3b', cyc: '#3a414c', actor: '#7d8896', beamAlpha: 0.22 },
  print: { bg: '#ffffff', minor: '#eef0f2', major: '#d5d9de', text: '#111418', muted: '#5d6470', body: '#111418', bodyFill: '#ffffff', sel: '#d97a00', cyc: '#9aa1ab', actor: '#5d6470', beamAlpha: 0.16 },
};

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const f1 = (v) => Math.round(v * 10) / 10;

/** Couleur moyenne émise par un projecteur (pour teinter son faisceau sur le plan). */
function outputCss(viewport, id, fallback) {
  const o = viewport?.outputOf(id);
  if (!o?.out || !o.type.pixels) return fallback;
  let r = 0;
  let g = 0;
  let b = 0;
  const n = o.type.pixels;
  for (let p = 0; p < n; p++) {
    r += o.out[p * 3];
    g += o.out[p * 3 + 1];
    b += o.out[p * 3 + 2];
  }
  const m = Math.max(r, g, b) / n;
  if (m < 0.02) return fallback;
  const c = (v) => Math.round(255 * linearToSrgb(Math.min(1, v / n / m)));
  return `rgb(${c(r)},${c(g)},${c(b)})`;
}

/**
 * @param {object} p
 * @param {object} p.scene
 * @param {{cx:number, cz:number, s:number, W:number, H:number}} p.view  centre (m), échelle (px/m), taille (px)
 * @param {'screen'|'print'} [p.theme]
 * @param {object} [p.options]  names, addresses, supports, beams, grid
 * @param {object} [p.sel]      fixtures (Set), objectId, plotId
 * @param {object} [p.preview]  élément en cours de tracé (même forme qu'un élément du plan)
 * @returns {string} contenu SVG (sans la balise <svg>)
 */
export function renderPlot({ scene, view, viewport, theme = 'screen', options = {}, sel = {}, preview = null, handles = [] }) {
  const T = THEMES[theme];
  const { cx, cz, s, W, H } = view;
  const X = (x) => f1((x - cx) * s + W / 2);
  const Z = (z) => f1((z - cz) * s + H / 2);
  const pts = (list) => list.map(([x, z]) => `${X(x)},${Z(z)}`).join(' ');
  const out = [];
  out.push(`<rect x="0" y="0" width="${W}" height="${H}" fill="${T.bg}"/>`);

  // ---- grille (1 m, traits forts tous les 5 m)
  if (options.grid !== false) {
    const x0 = Math.floor(cx - W / 2 / s);
    const x1 = Math.ceil(cx + W / 2 / s);
    const z0 = Math.floor(cz - H / 2 / s);
    const z1 = Math.ceil(cz + H / 2 / s);
    if (s >= 6) {
      for (let x = x0; x <= x1; x++) out.push(`<line x1="${X(x)}" y1="0" x2="${X(x)}" y2="${H}" stroke="${x % 5 ? T.minor : T.major}" stroke-width="1"/>`);
      for (let z = z0; z <= z1; z++) out.push(`<line x1="0" y1="${Z(z)}" x2="${W}" y2="${Z(z)}" stroke="${z % 5 ? T.minor : T.major}" stroke-width="1"/>`);
    }
  }

  // ---- image de fond (photo, plan d'architecte) calée à l'échelle
  const u = scene.plot?.underlay;
  if (u?.url) {
    const w = u.width * s;
    const h = (u.width / (u.aspect || 1)) * s;
    out.push(`<image href="${esc(u.url)}" x="${f1(X(u.x) - w / 2)}" y="${f1(Z(u.z) - h / 2)}" width="${f1(w)}" height="${f1(h)}" opacity="${u.opacity ?? 0.6}" transform="rotate(${u.rotation || 0} ${X(u.x)} ${Z(u.z)})" preserveAspectRatio="none"/>`);
  }

  // ---- décor du studio : cyclo et personnage
  const env = scene.environment || {};
  if (env.cyc !== false) {
    out.push(`<rect x="${X(-8)}" y="${Z(-7)}" width="${f1(16 * s)}" height="${f1(1.2 * s)}" fill="${T.cyc}" opacity="0.25"/>`);
    out.push(`<line x1="${X(-8)}" y1="${Z(-7)}" x2="${X(8)}" y2="${Z(-7)}" stroke="${T.cyc}" stroke-width="3"/>`);
    out.push(`<text x="${X(0)}" y="${Z(-7) - 6}" fill="${T.muted}" font-size="11" text-anchor="middle">Cyclo (fond)</text>`);
  }
  if (env.actor !== false) {
    out.push(`<circle cx="${X(0)}" cy="${Z(0)}" r="${Math.max(4, 0.22 * s)}" fill="none" stroke="${T.actor}" stroke-width="2"/>`);
    out.push(`<text x="${X(0)}" y="${f1(Z(0) + Math.max(4, 0.22 * s) + 12)}" fill="${T.muted}" font-size="10" text-anchor="middle">Comédien</text>`);
  }

  // ---- formes, textes, flèches, mesures
  const items = [...(scene.plot?.items || []), ...(preview ? [{ ...preview, preview: true }] : [])];
  for (const it of items) out.push(drawItem(it, { X, Z, pts, s, T, selected: it.id && it.id === sel.plotId }));

  // ---- grip
  for (const o of scene.objects || []) {
    const type = getGripType(o.type);
    if (!type) continue;
    const p = objectPlan(o, type);
    const cloth = CLOTHS[o.props?.cloth];
    const fill = type.family === 'box' ? '#8a6a45' : type.family === 'bag' ? '#6b4a2a' : type.family === 'bounce' ? '#f4f4f0' : cloth && cloth.opacity >= 1 ? (cloth.color === 0x111111 ? '#222' : '#f4f4f0') : 'none';
    const isSel = o.id === sel.objectId;
    out.push(`<polygon points="${pts(p.poly)}" fill="${fill}" stroke="${isSel ? T.sel : T.body}" stroke-width="${isSel ? 2.5 : 1.5}" ${cloth && cloth.opacity < 1 && type.family !== 'bounce' ? 'stroke-dasharray="5 3"' : ''}/>`);
    if (type.family === 'stand') out.push(standGlyph(X(o.position[0]), Z(o.position[2]), T.body));
    if (options.names !== false) out.push(`<text x="${X(p.center[0])}" y="${f1(Z(p.center[1]) - 7)}" fill="${T.muted}" font-size="10" text-anchor="middle">${esc(type.short)}${cloth && type.cloths ? ` · ${esc(cloth.name.split(' ')[0])}` : ''}</text>`);
  }

  // ---- projecteurs : faisceau, corps, étiquettes
  const labels = [];
  for (const f of scene.fixtures) {
    const type = getFixtureType(f.type);
    if (!type) continue;
    const p = fixturePlan(f, type);
    const isSel = sel.fixtures?.has(f.id);
    const color = outputCss(viewport, f.id, theme === 'print' ? '#f0b040' : '#ffd27a');
    if (options.beams !== false && p.dir) {
      const half = (Math.min(120, type.beamAngle || 40) / 2) * (Math.PI / 180);
      const L = 1.6;
      const [dx, dz] = p.dir;
      const rot = (a) => [dx * Math.cos(a) - dz * Math.sin(a), dx * Math.sin(a) + dz * Math.cos(a)];
      const [ax, az] = rot(half);
      const [bx, bz] = rot(-half);
      const [fx, fz] = p.front;
      out.push(`<polygon points="${pts([[fx, fz], [fx + ax * L, fz + az * L], [fx + bx * L, fz + bz * L]])}" fill="${color}" opacity="${T.beamAlpha}" stroke="none"/>`);
      out.push(`<line x1="${X(fx)}" y1="${Z(fz)}" x2="${X(fx + dx * L)}" y2="${Z(fz + dz * L)}" stroke="${color}" stroke-width="1" stroke-dasharray="4 3" opacity="0.8"/>`);
    }
    out.push(`<polygon points="${pts(p.poly)}" fill="${T.bodyFill}" stroke="${isSel ? T.sel : T.body}" stroke-width="${isSel ? 2.5 : 1.4}"/>`);
    if (p.down) out.push(`<circle cx="${X(p.center[0])}" cy="${Z(p.center[1])}" r="3" fill="${color}"/>`); // pointe vers le sol
    if (f.state?.power === false) out.push(`<text x="${X(p.center[0])}" y="${Z(p.center[1]) + 3}" fill="${T.muted}" font-size="9" text-anchor="middle">éteint</text>`);
    const lines = [];
    if (options.names !== false) lines.push({ t: f.name, cls: 'n' });
    if (options.addresses !== false && type.dmx?.modes?.length) lines.push({ t: patchLabel(f), cls: 'a' });
    if (options.supports) {
      const sup = viewport?.outputOf(f.id)?.support;
      if (sup?.def) lines.push({ t: `${sup.def.short} · h ${f1(p.height)} m`, cls: 's' });
    }
    if (lines.length) labels.push({ x: X(p.center[0]), z: Z(p.center[1]), dir: p.dir, lines, isSel });
  }
  for (const l of labels) {
    // étiquette du côté opposé au faisceau
    const ox = l.dir ? -l.dir[0] * 22 : 0;
    const oz = l.dir ? -l.dir[1] * 22 : 16;
    let y = l.z + oz + (oz < 0 ? -6 * l.lines.length : 4);
    for (const line of l.lines) {
      const style = line.cls === 'n' ? `font-weight="700" font-size="11" fill="${l.isSel ? T.sel : T.text}"` : line.cls === 'a' ? `font-family="ui-monospace, Consolas, monospace" font-size="10" fill="${theme === 'print' ? '#9a5b00' : '#ffad3b'}"` : `font-size="9" fill="${T.muted}"`;
      out.push(`<text x="${f1(l.x + ox)}" y="${f1(y)}" ${style} text-anchor="middle">${esc(line.t)}</text>`);
      y += 12;
    }
  }

  // ---- poignées (rotation, sommets)
  for (const hd of handles) out.push(`<circle cx="${X(hd.x)}" cy="${Z(hd.z)}" r="6" fill="${T.bg}" stroke="${T.sel}" stroke-width="2"/>${hd.kind === 'rotate' ? `<path d="M${X(hd.x) - 3},${Z(hd.z) - 1} a3,3 0 1,1 3,4" fill="none" stroke="${T.sel}" stroke-width="1.4"/>` : ''}`);

  // ---- échelle
  const nice = [0.5, 1, 2, 5, 10, 20, 50].find((m) => m * s >= 70) || 100;
  const bx = 16;
  const by = H - 18;
  out.push(`<line x1="${bx}" y1="${by}" x2="${f1(bx + nice * s)}" y2="${by}" stroke="${T.text}" stroke-width="2"/><line x1="${bx}" y1="${by - 4}" x2="${bx}" y2="${by + 4}" stroke="${T.text}" stroke-width="2"/><line x1="${f1(bx + nice * s)}" y1="${by - 4}" x2="${f1(bx + nice * s)}" y2="${by + 4}" stroke="${T.text}" stroke-width="2"/>`);
  out.push(`<text x="${f1(bx + (nice * s) / 2)}" y="${by - 7}" fill="${T.text}" font-size="11" text-anchor="middle">${String(nice).replace('.', ',')} m</text>`);
  return out.join('\n');
}

function standGlyph(x, z, color) {
  return `<g stroke="${color}" stroke-width="1.4"><line x1="${x}" y1="${z}" x2="${x - 7}" y2="${z + 5}"/><line x1="${x}" y1="${z}" x2="${x + 7}" y2="${z + 5}"/><line x1="${x}" y1="${z}" x2="${x}" y2="${z - 8}"/></g>`;
}

/** Un élément du plan (forme, texte, flèche, mesure). */
function drawItem(it, { X, Z, pts, s, T, selected }) {
  const st = it.style || {};
  const stroke = selected ? T.sel : st.stroke || '#ffad3b';
  const width = (st.width || 2) + (selected ? 1 : 0);
  const dash = st.dash || it.preview ? 'stroke-dasharray="6 4"' : '';
  const fill = st.fill && st.fill !== 'none' ? `fill="${st.fill}" fill-opacity="0.25"` : 'fill="none"';
  const P = it.points;
  const parts = [];
  const label = (x, z, text, anchor = 'middle') => (text ? `<text x="${f1(x)}" y="${f1(z)}" fill="${T.text}" font-size="${st.size || 12}" text-anchor="${anchor}">${esc(text)}</text>` : '');
  switch (it.kind) {
    case 'rect': {
      if (P.length < 2) break;
      const [[x0, z0], [x1, z1]] = P;
      const x = Math.min(X(x0), X(x1));
      const y = Math.min(Z(z0), Z(z1));
      parts.push(`<rect x="${x}" y="${y}" width="${f1(Math.abs(X(x1) - X(x0)))}" height="${f1(Math.abs(Z(z1) - Z(z0)))}" stroke="${stroke}" stroke-width="${width}" ${fill} ${dash}/>`);
      parts.push(label((X(x0) + X(x1)) / 2, (Z(z0) + Z(z1)) / 2 + 4, it.text));
      break;
    }
    case 'ellipse': {
      if (P.length < 2) break;
      const [[x0, z0], [x1, z1]] = P;
      parts.push(`<ellipse cx="${f1((X(x0) + X(x1)) / 2)}" cy="${f1((Z(z0) + Z(z1)) / 2)}" rx="${f1(Math.abs(X(x1) - X(x0)) / 2)}" ry="${f1(Math.abs(Z(z1) - Z(z0)) / 2)}" stroke="${stroke}" stroke-width="${width}" ${fill} ${dash}/>`);
      parts.push(label((X(x0) + X(x1)) / 2, (Z(z0) + Z(z1)) / 2 + 4, it.text));
      break;
    }
    case 'polygon':
    case 'freehand': {
      const closed = it.kind === 'polygon' && !it.preview;
      parts.push(`<${closed ? 'polygon' : 'polyline'} points="${pts(P)}" stroke="${stroke}" stroke-width="${width}" stroke-linejoin="round" stroke-linecap="round" ${closed || (st.fill && st.fill !== 'none') ? fill : 'fill="none"'} ${dash}/>`);
      if (it.text) {
        const cx = P.reduce((a, p) => a + p[0], 0) / P.length;
        const cz = P.reduce((a, p) => a + p[1], 0) / P.length;
        parts.push(label(X(cx), Z(cz) + 4, it.text));
      }
      break;
    }
    case 'line':
    case 'arrow':
    case 'measure': {
      if (P.length < 2) break;
      const [[ax, az], [bx, bz]] = P;
      parts.push(`<line x1="${X(ax)}" y1="${Z(az)}" x2="${X(bx)}" y2="${Z(bz)}" stroke="${stroke}" stroke-width="${width}" stroke-linecap="round" ${dash}/>`);
      const ang = Math.atan2(Z(bz) - Z(az), X(bx) - X(ax));
      if (it.kind === 'arrow') {
        const k = 8 + width * 2;
        const p1 = [X(bx) - k * Math.cos(ang - 0.45), Z(bz) - k * Math.sin(ang - 0.45)];
        const p2 = [X(bx) - k * Math.cos(ang + 0.45), Z(bz) - k * Math.sin(ang + 0.45)];
        parts.push(`<polygon points="${X(bx)},${Z(bz)} ${f1(p1[0])},${f1(p1[1])} ${f1(p2[0])},${f1(p2[1])}" fill="${stroke}"/>`);
        if (it.text) parts.push(label(X(ax), Z(az) - 6, it.text));
      } else if (it.kind === 'measure') {
        const len = Math.hypot(bx - ax, bz - az);
        const nx = -Math.sin(ang) * 6;
        const nz = Math.cos(ang) * 6;
        for (const [px, pz] of [[X(ax), Z(az)], [X(bx), Z(bz)]]) parts.push(`<line x1="${f1(px - nx)}" y1="${f1(pz - nz)}" x2="${f1(px + nx)}" y2="${f1(pz + nz)}" stroke="${stroke}" stroke-width="${width}"/>`);
        parts.push(label((X(ax) + X(bx)) / 2 - nx * 2, (Z(az) + Z(bz)) / 2 - nz * 2, `${len.toFixed(2).replace('.', ',')} m${it.text ? ` — ${it.text}` : ''}`));
      } else parts.push(label((X(ax) + X(bx)) / 2, (Z(az) + Z(bz)) / 2 - 6, it.text));
      break;
    }
    case 'text':
      parts.push(`<text x="${X(P[0][0])}" y="${Z(P[0][1])}" fill="${selected ? T.sel : st.stroke || T.text}" font-size="${st.size || 16}" font-weight="600">${esc(it.text || 'Texte')}</text>`);
      break;
    default:
      break;
  }
  if (it.height > 0 && P.length) parts.push(`<text x="${X(P[0][0]) + 4}" y="${Z(P[0][1]) - 4}" fill="${T.muted}" font-size="9">h ${String(it.height).replace('.', ',')} m</text>`);
  return parts.join('');
}

/** Cadre de titre (cartouche) pour l'export. */
export function titleBlock({ W, H, title, theme = 'print', date }) {
  const T = THEMES[theme];
  const w = 280;
  const h = 74;
  const x = W - w - 16;
  const y = H - h - 16;
  return `<g><rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${T.bg}" stroke="${T.text}" stroke-width="1.2"/>
<text x="${x + 10}" y="${y + 20}" font-size="14" font-weight="700" fill="${T.text}">${esc(title.production || 'Plan de feu')}</text>
<text x="${x + 10}" y="${y + 38}" font-size="11" fill="${T.text}">${esc(title.scene || '')}</text>
<text x="${x + 10}" y="${y + 54}" font-size="10" fill="${T.muted}">${esc(title.author ? `Chef électro : ${title.author}` : '')}</text>
<text x="${x + w - 10}" y="${y + 66}" font-size="9" fill="${T.muted}" text-anchor="end">${esc(date)} · ARKLIGHT</text></g>`;
}

export const SUPPORT_NAMES = Object.fromEntries(Object.entries(SUPPORTS).map(([k, v]) => [k, v.short]));
