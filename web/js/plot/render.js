// Dessin du plan de feu en SVG : à l'écran (thème sombre) ou pour l'export et l'impression (fond blanc),
// ou sur un fond de couleur choisi (les traits s'adaptent pour rester lisibles).
// Les étiquettes (noms, numéros de fixture, adresses DMX, pieds) sont placées autour des éléments
// sans les recouvrir, avec un trait de rappel quand elles s'en éloignent.
import { getFixtureType } from '/shared/fixtures/index.js';
import { CLOTHS, clothShort, gripStands } from '/shared/grip.js';
import { getFilter } from '/shared/filters.js';
import { accessoryLabel } from '/shared/modifiers.js';
import { getObjectType, isProp } from '/shared/objects.js';
import { SUPPORTS } from '/shared/fixtures/supports.js';
import { cameraFov, propSize } from '/shared/props.js';
import { linearToSrgb } from '/shared/engine/color.js';
import { patchLine } from '../ui/patch.js';
import { t as tr } from '../i18n.js';
import { fixturePlan, frameEars, objectPlan, pointInPolygon } from './geometry.js';
import { propSymbol } from './symbols.js';

export const THEMES = {
  screen: { bg: '#0f1216', minor: '#1a1f26', major: '#29303a', text: '#d9dde3', muted: '#8a919c', body: '#e6e9ee', bodyFill: '#1d2229', sel: '#ffad3b', cyc: '#3a414c', prop: '#2a3038', addr: '#ffad3b', beam: '#ffd27a', beamAlpha: 0.22, light: false },
  print: { bg: '#ffffff', minor: '#eef0f2', major: '#d5d9de', text: '#111418', muted: '#5d6470', body: '#111418', bodyFill: '#ffffff', sel: '#d97a00', cyc: '#9aa1ab', prop: '#e9ebee', addr: '#9a5b00', beam: '#f0b040', beamAlpha: 0.16, light: true },
};

const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
const f1 = (v) => Math.round(v * 10) / 10;

// ------------------------------------------------------------------ couleurs et fond
function rgbOf(hex) {
  const m = /^#?([0-9a-f]{6})$/i.exec(String(hex || '').trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
const lum = (rgb) => (0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]) / 255;
function mix(a, b, k) {
  const A = rgbOf(a);
  const B = rgbOf(b);
  if (!A || !B) return a;
  return `#${A.map((v, i) => Math.round(v + (B[i] - v) * k).toString(16).padStart(2, '0')).join('')}`;
}

/**
 * Thème du dessin. bg : couleur de fond choisie (null : automatique — sombre à l'écran, blanc à
 * l'export). Sur un fond clair, projecteurs, traits et textes passent en foncé, et inversement.
 */
export function themeFor(theme, bg) {
  const base = THEMES[theme] || THEMES.screen;
  const rgb = rgbOf(bg);
  if (!rgb) return base;
  const light = lum(rgb) > 0.5;
  const t = { ...(light ? THEMES.print : THEMES.screen), bg };
  t.minor = mix(bg, t.text, 0.08);
  t.major = mix(bg, t.text, 0.2);
  t.bodyFill = mix(bg, t.text, 0.07);
  t.prop = mix(bg, t.text, 0.12);
  t.cyc = mix(bg, t.text, 0.4);
  return t;
}

/** Couleur choisie par l'utilisateur, remplacée par celle du texte si elle se confond avec le fond. */
function readable(color, T) {
  const c = rgbOf(color);
  const b = rgbOf(T.bg);
  if (!c || !b) return color;
  return Math.abs(lum(c) - lum(b)) < 0.22 ? T.text : color;
}

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

// ------------------------------------------------------------------ placement des étiquettes
const LINE_H = 12;
/** Largeur approximative d'un texte (px) selon sa police. */
const textW = (t, cls) => String(t).length * (cls === 'n' ? 6.4 : cls === 'a' ? 6.0 : cls === 'o' ? 5.6 : 5.1);

function boxesOverlap(a, b) {
  return a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1;
}
function segIntersect(p, q, r, s) {
  const d = (a, b, c) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
  const d1 = d(r, s, p);
  const d2 = d(r, s, q);
  const d3 = d(p, q, r);
  const d4 = d(p, q, s);
  return d1 * d2 < 0 && d3 * d4 < 0;
}
/** Une boîte (étiquette) touche-t-elle un polygone (corps d'un projecteur, faisceau, objet) ? */
function boxHitsPoly(b, poly) {
  if (!poly.bbox) {
    const xs = poly.pts.map((p) => p[0]);
    const ys = poly.pts.map((p) => p[1]);
    poly.bbox = { x0: Math.min(...xs), x1: Math.max(...xs), y0: Math.min(...ys), y1: Math.max(...ys) };
  }
  if (!boxesOverlap(b, poly.bbox)) return false;
  const corners = [
    [b.x0, b.y0],
    [b.x1, b.y0],
    [b.x1, b.y1],
    [b.x0, b.y1],
  ];
  if (poly.pts.some(([x, y]) => x > b.x0 && x < b.x1 && y > b.y0 && y < b.y1)) return true;
  if (poly.pts.length >= 3 && corners.some((c) => pointInPolygon(c, poly.pts))) return true;
  for (let i = 0; i < poly.pts.length; i++) {
    const p = poly.pts[i];
    const q = poly.pts[(i + 1) % poly.pts.length];
    for (let j = 0; j < 4; j++) if (segIntersect(p, q, corners[j], corners[(j + 1) % 4])) return true;
  }
  return false;
}

const DIRS = [
  [0, 1],
  [0, -1],
  [1, 0],
  [-1, 0],
  [0.7071, 0.7071],
  [-0.7071, 0.7071],
  [0.7071, -0.7071],
  [-0.7071, -0.7071],
];

/**
 * Place les étiquettes : pour chacune, plusieurs positions autour de l'élément sont essayées et
 * la moins gênante est retenue (aucun chevauchement d'étiquette, puis aucun élément recouvert,
 * puis le moins de faisceaux traversés, au plus près).
 */
function placeLabels(reqs, solids, softs, W, H) {
  const placed = [];
  for (const r of reqs) {
    const w = Math.max(...r.lines.map((l) => textW(l.t, l.cls))) + 4;
    const h = r.lines.length * LINE_H + 2;
    const dirs = r.pref ? [r.pref, ...DIRS] : DIRS;
    let best = null;
    dirs.forEach((u, di) => {
      for (const extra of [4, 14, 28, 46]) {
        const d = r.radius + extra;
        const ext = Math.abs(u[0]) * (w / 2) + Math.abs(u[1]) * (h / 2);
        const cx = r.x + u[0] * (d + ext);
        const cy = r.y + u[1] * (d + ext);
        const b = { x0: cx - w / 2, x1: cx + w / 2, y0: cy - h / 2, y1: cy + h / 2 };
        let score = di * 0.6 + extra * 0.9;
        for (const p of placed) if (boxesOverlap(b, p.box)) score += 1000;
        for (const s of solids) if (s.owner !== r.owner && boxHitsPoly(b, s)) score += 380;
        for (const s of softs) if (s.owner !== r.owner && boxHitsPoly(b, s)) score += 18;
        if (b.x0 < 2 || b.y0 < 2 || b.x1 > W - 2 || b.y1 > H - 2) score += 250;
        if (!best || score < best.score) best = { score, box: b, u, d, ext };
      }
    });
    placed.push({ ...r, ...best });
  }
  return placed;
}

/** Rayon (px) d'un élément vu de son centre : les étiquettes se placent au-delà. */
function radiusOf(poly, x, y) {
  return Math.max(6, ...poly.map(([px, py]) => Math.hypot(px - x, py - y)));
}

/**
 * @param {object} p
 * @param {object} p.scene
 * @param {{cx:number, cz:number, s:number, W:number, H:number}} p.view  centre (m), échelle (px/m), taille (px)
 * @param {'screen'|'print'} [p.theme]
 * @param {object} [p.options]  names, fixtures, addresses, supports, beams, grid
 * @param {object} [p.sel]      fixtures (Set), objectId, plotId
 * @param {object} [p.preview]  élément en cours de tracé (même forme qu'un élément du plan)
 * @returns {string} contenu SVG (sans la balise <svg>)
 */
export function renderPlot({ scene, view, viewport, theme = 'screen', options = {}, sel = {}, preview = null, handles = [] }) {
  const T = themeFor(theme, scene.plot?.background);
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
  if (u?.url && options.underlay !== false) {
    const w = u.width * s;
    const h = (u.width / (u.aspect || 1)) * s;
    out.push(`<image href="${esc(u.url)}" x="${f1(X(u.x) - w / 2)}" y="${f1(Z(u.z) - h / 2)}" width="${f1(w)}" height="${f1(h)}" opacity="${u.opacity ?? 0.6}" transform="rotate(${u.rotation || 0} ${X(u.x)} ${Z(u.z)})" preserveAspectRatio="none"/>`);
  }

  // ---- studio : cyclo
  const env = scene.environment || {};
  if (env.cyc !== false && options.studio !== false) {
    out.push(`<rect x="${X(-8)}" y="${Z(-7)}" width="${f1(16 * s)}" height="${f1(1.2 * s)}" fill="${T.cyc}" opacity="0.25"/>`);
    out.push(`<line x1="${X(-8)}" y1="${Z(-7)}" x2="${X(8)}" y2="${Z(-7)}" stroke="${T.cyc}" stroke-width="3"/>`);
    out.push(`<text x="${X(0)}" y="${Z(-7) - 6}" fill="${T.muted}" font-size="11" text-anchor="middle">${tr('Cyclo (fond)')}</text>`);
  }

  // ---- formes, textes, flèches, mesures
  // annotations (formes, textes, flèches, mesures) : masquables ; le tracé en cours reste visible
  const items = [...(options.annotations !== false ? scene.plot?.items || [] : []), ...(preview ? [{ ...preview, preview: true }] : [])];
  for (const it of items) out.push(drawItem(it, { X, Z, pts, s, T, selected: it.id && it.id === sel.plotId }));

  const solids = []; // corps des projecteurs et objets (écran) : les étiquettes les évitent
  const softs = []; // faisceaux : évités si possible
  const fixtureLabels = [];
  const objectLabels = [];

  // ---- objets : décor (personnages, véhicules, mobilier…) et grip
  for (const o of scene.objects || []) {
    const type = getObjectType(o.type);
    if (!type || !objectShown(type, options)) continue;
    const p = objectPlan(o, type);
    const isSel = o.id === sel.objectId;
    const poly = p.poly.map(([x, z]) => [X(x), Z(z)]);
    const frame = type.family === 'frame';
    const st = !isProp(type) && gripStands(type, o.props);
    if (isProp(type)) out.push(propGlyph(o, type, { X, Z, s, T, isSel, pts }));
    else if (frame) {
      // cadre transparent : on ne dessine que les tubes (la toile ne cache rien du plan) ;
      // une gélatine se devine à un liseré de sa couleur, les pieds sont aux oreilles du cadre
      const filter = !CLOTHS[o.props?.cloth] && getFilter(o.props?.cloth);
      out.push(`<polygon points="${pts(p.poly)}" fill="none" stroke="${isSel ? T.sel : T.body}" stroke-width="${isSel ? 3 : 2}" stroke-linejoin="round"/>`);
      if (filter) out.push(`<polygon points="${pts(insetPoly(p.poly, 0.06))}" fill="none" stroke="${filter.color}" stroke-width="1.5" stroke-dasharray="6 3"/>`);
      for (const [x, z] of frameEars(o, type)) out.push(standGlyph(X(x), Z(z), T.body));
    } else {
      const cloth = CLOTHS[o.props?.cloth];
      const filter = !cloth && getFilter(o.props?.cloth);
      const fill = type.family === 'box' ? '#8a6a45' : type.family === 'bag' ? '#6b4a2a' : type.family === 'bounce' ? '#f4f4f0' : filter ? filter.color : cloth && cloth.opacity >= 1 ? (cloth.color === 0x111111 ? '#222' : '#f4f4f0') : 'none';
      const dashed = filter || (cloth && cloth.opacity < 1 && type.family !== 'bounce');
      out.push(`<polygon points="${pts(p.poly)}" fill="${fill}" ${filter ? 'fill-opacity="0.45"' : ''} stroke="${isSel ? T.sel : T.body}" stroke-width="${isSel ? 2.5 : 1.5}" ${dashed ? 'stroke-dasharray="5 3"' : ''}/>`);
      if (type.family === 'stand') out.push(standGlyph(X(o.position[0]), Z(o.position[2]), T.body));
    }
    // le décor (voiture, canapé…) peut passer sous une étiquette ; le grip, non. Un cadre est vide :
    // les étiquettes peuvent se poser dedans, en évitant ses tubes (y compris la sienne)
    if (frame) for (let k = 0; k < poly.length; k++) softs.push({ pts: strip(poly[k], poly[(k + 1) % poly.length], 3), owner: `${o.id}#tube` });
    else (isProp(type) ? softs : solids).push({ pts: poly, owner: o.id });
    if (options.names !== false) {
      const text = isProp(type) ? o.name : `${tr(type.short)}${type.cloths && o.props?.cloth ? ` · ${tr(clothShort(o.props.cloth))}` : ''}`;
      const x = X(p.center[0]);
      const y = Z(p.center[1]);
      const lines = [{ t: text, cls: 'o' }];
      if (frame && st && options.supports !== false) lines.push({ t: `${st.count} × ${tr(SUPPORTS[st.id]?.short || st.id)}`, cls: 's' });
      objectLabels.push({ owner: o.id, x, y, radius: frame ? Math.min(radiusOf(poly, x, y) * 0.75, 20) : radiusOf(poly, x, y) * 0.75, lines, sel: isSel });
    }
  }

  // ---- projecteurs : faisceau et corps
  for (const f of scene.fixtures) {
    const type = getFixtureType(f.type);
    if (!type) continue;
    const p = fixturePlan(f, type);
    const isSel = sel.fixtures?.has(f.id);
    const color = outputCss(viewport, f.id, T.beam);
    if (options.beams !== false && p.dir) {
      const half = (Math.min(120, type.beamAngle || 40) / 2) * (Math.PI / 180);
      const L = 1.6;
      const [dx, dz] = p.dir;
      const rot = (a) => [dx * Math.cos(a) - dz * Math.sin(a), dx * Math.sin(a) + dz * Math.cos(a)];
      const [ax, az] = rot(half);
      const [bx, bz] = rot(-half);
      const [fx, fz] = p.front;
      const tri = [
        [fx, fz],
        [fx + ax * L, fz + az * L],
        [fx + bx * L, fz + bz * L],
      ];
      out.push(`<polygon points="${pts(tri)}" fill="${color}" opacity="${T.beamAlpha}" stroke="none"/>`);
      out.push(`<line x1="${X(fx)}" y1="${Z(fz)}" x2="${X(fx + dx * L)}" y2="${Z(fz + dz * L)}" stroke="${color}" stroke-width="1" stroke-dasharray="4 3" opacity="0.8"/>`);
      softs.push({ pts: tri.map(([x, z]) => [X(x), Z(z)]), owner: f.id });
    }
    out.push(`<polygon points="${pts(p.poly)}" fill="${T.bodyFill}" stroke="${isSel ? T.sel : T.body}" stroke-width="${isSel ? 2.5 : 1.4}"/>`);
    if (p.down) out.push(`<circle cx="${X(p.center[0])}" cy="${Z(p.center[1])}" r="3" fill="${color}"/>`); // pointe vers le sol
    if (f.state?.power === false) out.push(`<text x="${X(p.center[0])}" y="${Z(p.center[1]) + 3}" fill="${T.muted}" font-size="9" text-anchor="middle">${tr('éteint')}</text>`);
    const poly = p.poly.map(([x, z]) => [X(x), Z(z)]);
    solids.push({ pts: poly, owner: f.id });
    const lines = [];
    if (options.names !== false) lines.push({ t: f.name, cls: 'n' });
    if (type.dmx?.modes?.length && (options.fixtures !== false || options.addresses !== false)) {
      const t = patchLine(f, { fixture: options.fixtures !== false, dmx: options.addresses !== false });
      if (t) lines.push({ t, cls: 'a' });
    }
    if (options.supports !== false) {
      // accessoires montés (Chimera, dôme, louver, gélatine) puis pied et hauteur
      const sup = viewport?.outputOf(f.id)?.support;
      const t = [accessoryLabel(type, f.settings, tr), sup?.def ? `${tr(sup.def.short)} · h ${String(f1(p.height)).replace('.', ',')} m` : ''].filter(Boolean).join(' · ');
      if (t) lines.push({ t, cls: 's' });
    }
    if (lines.length) {
      const x = X(p.center[0]);
      const y = Z(p.center[1]);
      fixtureLabels.push({ owner: f.id, x, y, radius: radiusOf(poly, x, y), lines, sel: isSel, pref: p.dir ? [-p.dir[0], -p.dir[1]] : null });
    }
  }

  // ---- étiquettes : projecteurs d'abord, puis objets ; trait de rappel si elles s'éloignent
  const halo = `stroke="${T.bg}" stroke-width="3" stroke-linejoin="round" paint-order="stroke"`;
  for (const l of placeLabels([...fixtureLabels, ...objectLabels], solids, softs, W, H)) {
    const { box, u: dir } = l;
    const mx = (box.x0 + box.x1) / 2;
    if (l.d > l.radius + 12) {
      const ex = mx - dir[0] * l.ext;
      const ey = (box.y0 + box.y1) / 2 - dir[1] * l.ext;
      out.push(`<line x1="${f1(l.x + dir[0] * l.radius * 0.9)}" y1="${f1(l.y + dir[1] * l.radius * 0.9)}" x2="${f1(ex)}" y2="${f1(ey)}" stroke="${T.muted}" stroke-width="0.8" opacity="0.7"/>`);
    }
    l.lines.forEach((line, i) => {
      const y = box.y0 + 11 + i * LINE_H;
      const style =
        line.cls === 'n'
          ? `font-weight="700" font-size="11" fill="${l.sel ? T.sel : T.text}"`
          : line.cls === 'a'
            ? `font-family="ui-monospace, Consolas, monospace" font-size="10" fill="${T.addr}"`
            : line.cls === 'o'
              ? `font-size="10" fill="${l.sel ? T.sel : T.muted}"`
              : `font-size="9" fill="${T.muted}"`;
      out.push(`<text x="${f1(mx)}" y="${f1(y)}" ${style} ${halo} text-anchor="middle">${esc(line.t)}</text>`);
    });
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

/** Bande étroite autour d'un segment (tube d'un cadre, que les étiquettes évitent). */
function strip([ax, ay], [bx, by], r) {
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const nx = (-(by - ay) / L) * r;
  const ny = ((bx - ax) / L) * r;
  return [[ax + nx, ay + ny], [bx + nx, by + ny], [bx - nx, by - ny], [ax - nx, ay - ny]];
}

/**
 * Ce qui s'affiche sur le plan (« Affichage ») : le matériel (projecteurs, pieds, grip) par défaut, sans
 * les sacs de sable (comptés dans la liste du matériel) ;
 * le décor par catégorie (personnages, véhicules, mobilier, bâtiments…) seulement si on le coche.
 */
export function objectShown(type, options = {}) {
  if (!type) return false;
  if (type.family === 'bag') return options.sandbags === true; // sacs de sable : seulement si on les coche
  if (!isProp(type)) return type.family === 'stand' ? options.stands !== false : options.grip !== false;
  return options[type.cat] === true;
}

/** Polygone resserré vers son centre (liseré intérieur). */
function insetPoly(poly, k) {
  const cx = poly.reduce((a, p) => a + p[0], 0) / poly.length;
  const cz = poly.reduce((a, p) => a + p[1], 0) / poly.length;
  return poly.map(([x, z]) => [x + (cx - x) * k, z + (cz - z) * k]);
}

function standGlyph(x, z, color) {
  return `<g stroke="${color}" stroke-width="1.4"><line x1="${x}" y1="${z}" x2="${x - 7}" y2="${z + 5}"/><line x1="${x}" y1="${z}" x2="${x + 7}" y2="${z + 5}"/><line x1="${x}" y1="${z}" x2="${x}" y2="${z - 8}"/></g>`;
}

/**
 * Objet de décor vu de dessus : silhouette d'un personnage (épaules, tête, nez dans le sens du
 * regard), d'un animal, d'un véhicule (pare-brise), ou emprise simple ; champ des caméras.
 */
function propGlyph(o, type, { X, Z, s, T, isSel, pts }) {
  const stroke = isSel ? T.sel : T.muted;
  const sw = isSel ? 2.2 : 1.2;
  const yaw = o.rotation?.[1] || 0;
  const fwd = [Math.sin(yaw), Math.cos(yaw)]; // avant de l'objet (+Z local) vu de dessus
  const side = [Math.cos(yaw), -Math.sin(yaw)]; // +X local
  const at = (lx, lz) => [o.position[0] + side[0] * lx + fwd[0] * lz, o.position[2] + side[1] * lx + fwd[1] * lz];
  const P = (lx, lz) => {
    const [x, z] = at(lx, lz);
    return [X(x), Z(z)];
  };
  // symbole vu de dessus (arbres, véhicules, mobilier…) : voir symbols.js
  const parts = [propSymbol(o, type, { X, Z, s, T, isSel })];
  // champ de la caméra : deux bords du cadre horizontal, sur 6 m
  if (type.model === 'camera' || type.model === 'dolly') {
    const fov = cameraFov(o.props);
    const L = 6;
    const t = Math.tan(fov.h / 2) * L;
    const [lx, ly] = P(0, 0.35);
    const [ax, ay] = P(-t, L);
    const [bx, by] = P(t, L);
    parts.push(`<polygon points="${lx},${ly} ${ax},${ay} ${bx},${by}" fill="${T.sel}" opacity="0.06"/>`);
    parts.push(`<polyline points="${ax},${ay} ${lx},${ly} ${bx},${by}" fill="none" stroke="${T.sel}" stroke-width="1" stroke-dasharray="6 4" opacity="0.8"/>`);
  }
  return parts.join('');
}

/** Un élément du plan (forme, texte, flèche, mesure). */
function drawItem(it, { X, Z, pts, s, T, selected }) {
  const st = it.style || {};
  const stroke = selected ? T.sel : readable(st.stroke || '#ffad3b', T);
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
      parts.push(`<text x="${X(P[0][0])}" y="${Z(P[0][1])}" fill="${selected ? T.sel : readable(st.stroke || T.text, T)}" font-size="${st.size || 16}" font-weight="600">${esc(it.text || 'Texte')}</text>`);
      break;
    default:
      break;
  }
  if (it.height > 0 && P.length) parts.push(`<text x="${X(P[0][0]) + 4}" y="${Z(P[0][1]) - 4}" fill="${T.muted}" font-size="9">h ${String(it.height).replace('.', ',')} m</text>`);
  return parts.join('');
}

/** Cadre de titre (cartouche) pour l'export ; title.plan : « Séquence 1 › Plan 2 » (projet à plusieurs plans). */
export function titleBlock({ W, H, title, theme = 'print', date, background = null }) {
  const T = themeFor(theme, background);
  const w = 300;
  const extra = title.plan ? 16 : 0;
  const h = 74 + extra;
  const x = W - w - 16;
  const y = H - h - 16;
  const plan = title.plan ? `\n<text x="${x + 10}" y="${y + 54}" font-size="11" font-weight="600" fill="${T.text}">${esc(title.plan)}</text>` : '';
  return `<g><rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${T.bg}" stroke="${T.text}" stroke-width="1.2"/>
<text x="${x + 10}" y="${y + 20}" font-size="14" font-weight="700" fill="${T.text}">${esc(title.production || tr('Plan de feu'))}</text>
<text x="${x + 10}" y="${y + 38}" font-size="11" fill="${T.text}">${esc(title.scene || '')}</text>${plan}
<text x="${x + 10}" y="${y + 54 + extra}" font-size="10" fill="${T.muted}">${esc(title.author ? tr('Chef électro : {0}', { 0: title.author }) : '')}</text>
<text x="${x + w - 10}" y="${y + 66 + extra}" font-size="9" fill="${T.muted}" text-anchor="end">${esc(date)} · ARKLIGHT</text></g>`;
}

export const SUPPORT_NAMES = Object.fromEntries(Object.entries(SUPPORTS).map(([k, v]) => [k, v.short]));
