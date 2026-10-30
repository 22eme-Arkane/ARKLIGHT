// Symboles du décor vus de dessus pour le plan de feu : arbres (feuillage découpé, pin en étoile,
// palmier), véhicules, mobilier, murs et portes, mobilier urbain, accessoires, matériel de plateau.
// Dessinés en mètres dans le repère de l'objet (x : côté, y : avant), puis placés et tournés ;
// couleurs de l'objet en aplat léger, traits fins, pour encombrer le moins possible le plan.
import { propSize } from '/shared/props.js';

const f3 = (v) => Math.round(v * 1000) / 1000;
const NS = 'vector-effect="non-scaling-stroke"';

/** Couleur « #rrggbb » éclaircie (k > 0) ou assombrie (k < 0). */
function shade(hex, k) {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex || '');
  if (!m) return hex;
  const n = parseInt(m[1], 16);
  const ch = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => Math.round(k >= 0 ? c + (255 - c) * k : c * (1 + k)));
  return `#${ch.map((c) => c.toString(16).padStart(2, '0')).join('')}`;
}

/** Petit nombre pseudo-aléatoire stable (formes des rochers…). */
function rand(seed) {
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) h = Math.imul(h ^ seed.charCodeAt(i), 16777619);
  return () => {
    h = Math.imul(h ^ (h >>> 15), 2246822507);
    h = Math.imul(h ^ (h >>> 13), 3266489909);
    return ((h ^= h >>> 16) >>> 0) / 4294967296;
  };
}

/** Outils de dessin dans le repère de l'objet. */
function pen({ stroke, sw, fillOpacity = 0.55 }) {
  const st = (s = stroke, w = sw) => `stroke="${s}" stroke-width="${w}" ${NS}`;
  const fo = (o) => (o === undefined ? fillOpacity : o);
  // un `stroke` dans `extra` remplace le trait par défaut : l'attribut n'est écrit qu'une fois (XML valide)
  const ownStroke = (extra) => /(^|\s)stroke="/.test(extra);
  return {
    rect: (x0, y0, x1, y1, fill, o, extra = '') => `<rect x="${f3(Math.min(x0, x1))}" y="${f3(Math.min(y0, y1))}" width="${f3(Math.abs(x1 - x0))}" height="${f3(Math.abs(y1 - y0))}" fill="${fill}" fill-opacity="${fo(o)}" ${ownStroke(extra) ? '' : `${st()} `}${extra}/>`,
    rrect: (x0, y0, x1, y1, r, fill, o) => `<rect x="${f3(Math.min(x0, x1))}" y="${f3(Math.min(y0, y1))}" width="${f3(Math.abs(x1 - x0))}" height="${f3(Math.abs(y1 - y0))}" rx="${f3(r)}" fill="${fill}" fill-opacity="${fo(o)}" ${st()}/>`,
    circle: (x, y, r, fill, o, s2) => `<circle cx="${f3(x)}" cy="${f3(y)}" r="${f3(r)}" fill="${fill}" fill-opacity="${fo(o)}" ${st(s2)}/>`,
    ellipse: (x, y, rx, ry, rotDeg, fill, o) => `<ellipse cx="${f3(x)}" cy="${f3(y)}" rx="${f3(rx)}" ry="${f3(ry)}" transform="rotate(${f3(rotDeg)} ${f3(x)} ${f3(y)})" fill="${fill}" fill-opacity="${fo(o)}" ${st()}/>`,
    line: (x0, y0, x1, y1, s2, w2) => `<line x1="${f3(x0)}" y1="${f3(y0)}" x2="${f3(x1)}" y2="${f3(y1)}" ${st(s2, w2)} stroke-linecap="round"/>`,
    poly: (pts, fill, o) => `<polygon points="${pts.map(([x, y]) => `${f3(x)},${f3(y)}`).join(' ')}" fill="${fill}" fill-opacity="${fo(o)}" ${st()} stroke-linejoin="round"/>`,
    path: (d, fill, o, s2, w2) => `<path d="${d}" fill="${fill}" fill-opacity="${fo(o)}" ${st(s2, w2)} stroke-linejoin="round" stroke-linecap="round"/>`,
  };
}

/** Feuillage découpé (festons) : contour d'arbre vu de dessus. */
function scallops(r, n, seed = 0) {
  const pts = Array.from({ length: n }, (_, i) => {
    const a = (i / n) * Math.PI * 2 + seed;
    return [Math.cos(a) * r * 0.86, Math.sin(a) * r * 0.86];
  });
  let d = `M${f3(pts[0][0])},${f3(pts[0][1])}`;
  for (let i = 1; i <= n; i++) {
    const [x, y] = pts[i % n];
    const [px, py] = pts[i - 1];
    const rr = Math.hypot(x - px, y - py) * 0.62;
    d += ` A${f3(rr)},${f3(rr)} 0 0 1 ${f3(x)},${f3(y)}`;
  }
  return `${d} Z`;
}

const star = (r, n, inner) => Array.from({ length: n * 2 }, (_, i) => {
  const a = (i / (n * 2)) * Math.PI * 2;
  const rr = i % 2 ? r * inner : r;
  return [Math.cos(a) * rr, Math.sin(a) * rr];
});

// ------------------------------------------------------------------ symboles par modèle
// c : { w, d, color, stroke, light, p (stylo), props, id, T }
const SYMBOLS = {
  person(c) {
    const { p, props } = c;
    const k = (props.height || 175) / 175;
    const top = props.top || '#3d4f6b';
    if (props.pose === 'lying') return p.rrect(-0.22 * k, -0.85 * k, 0.22 * k, 0.75 * k, 0.08, top) + p.circle(0, -0.82 * k, 0.11 * k, props.skin || '#e0aa83', 0.8);
    const back = props.pose === 'sitting' ? -0.08 : 0;
    const legs = props.pose === 'sitting' ? p.rrect(-0.18 * k, 0.05, 0.18 * k, 0.48 * k, 0.05, props.pants || '#2a2e35') : '';
    const arm = props.pose === 'pointing' ? p.line(-0.2 * k, back, -0.2 * k, back + 0.6 * k) : '';
    return legs + p.ellipse(0, back, 0.24 * k, 0.12 * k, 0, top, 0.75) + arm + p.circle(0, back, 0.1 * k, props.hairColor || '#2b1d14', 0.85) + p.line(0, back + 0.1 * k, 0, back + 0.19 * k);
  },
  animal(c) {
    const { p, w, d, color } = c;
    return p.ellipse(0, -d * 0.08, w / 2, d * 0.36, 0, color, 0.65) + p.circle(0, d * 0.38, Math.max(0.05, w * 0.32), color, 0.75) + p.line(0, -d * 0.44, w * 0.15, -d * 0.55);
  },
  vehicle(c) {
    const { p, w, d, color, props, stroke } = c;
    const type = c.typeId;
    if (/bicycle/.test(type)) return p.line(0, -d / 2, 0, d / 2, stroke, 2) + p.line(-w * 0.4, d * 0.32, w * 0.4, d * 0.32) + p.circle(0, -d * 0.05, 0.08, color, 0.8);
    if (/motorbike|scooter/.test(type)) return p.rrect(-w * 0.32, -d / 2, w * 0.32, d / 2, w * 0.3, color) + p.line(-w / 2, d * 0.3, w / 2, d * 0.3) + p.ellipse(0, -d * 0.05, w * 0.18, d * 0.18, 0, shade(color, -0.3), 0.7);
    if (/truck|van|caravan|generator/.test(type)) {
      const cab = /truck/.test(type) ? d * 0.26 : /van/.test(type) ? d * 0.18 : 0;
      let s = p.rrect(-w / 2, -d / 2, w / 2, d / 2, 0.12, color);
      if (cab) s += p.line(-w / 2, d / 2 - cab, w / 2, d / 2 - cab) + p.poly([[-w * 0.42, d / 2 - cab * 0.35], [w * 0.42, d / 2 - cab * 0.35], [w * 0.38, d / 2 - 0.08], [-w * 0.38, d / 2 - 0.08]], '#9fc6e8', 0.6);
      if (/generator/.test(type)) for (let i = 1; i < 6; i++) s += p.line(-w * 0.35, -d / 2 + (d * i) / 6, w * 0.35, -d / 2 + (d * i) / 6, shade(color, -0.4), 1);
      if (/caravan/.test(type)) s += p.line(w / 2, -d * 0.1, w / 2 - 0.05, d * 0.15, stroke, 2);
      return s;
    }
    // voiture : carrosserie arrondie, pare-brise, lunette arrière, toit
    return (
      p.rrect(-w / 2, -d / 2, w / 2, d / 2, Math.min(w, d) * 0.22, color) +
      p.poly([[-w * 0.42, d * 0.12], [w * 0.42, d * 0.12], [w * 0.36, d * 0.27], [-w * 0.36, d * 0.27]], '#9fc6e8', 0.65) +
      p.poly([[-w * 0.38, -d * 0.3], [w * 0.38, -d * 0.3], [w * 0.4, -d * 0.2], [-w * 0.4, -d * 0.2]], '#9fc6e8', 0.5) +
      p.rect(-w * 0.4, -d * 0.2, w * 0.4, d * 0.12, shade(color, -0.25), 0.55) +
      (props.wheels === false ? '' : '')
    );
  },
  tree(c) {
    const { p, w, color, id } = c;
    const r = w / 2;
    const seed = rand(id)() * Math.PI;
    return p.path(scallops(r, 9, seed), color, 0.42, shade(color, -0.35), 1.2) + p.path(scallops(r * 0.55, 6, seed + 0.4), shade(color, 0.25), 0.35, 'none') + p.circle(0, 0, Math.max(0.08, r * 0.08), '#6b4a2a', 0.9);
  },
  pine(c) {
    const { p, w, color } = c;
    const r = w / 2;
    return p.poly(star(r, 12, 0.72), color, 0.5) + p.poly(star(r * 0.55, 9, 0.7), shade(color, 0.2), 0.45) + p.circle(0, 0, Math.max(0.06, r * 0.07), '#5a3d22', 0.9);
  },
  palm(c) {
    const { p, w, color } = c;
    const r = w / 2;
    let s = '';
    for (let i = 0; i < 7; i++) {
      const a = (i / 7) * 360;
      const rad = (a * Math.PI) / 180;
      s += p.ellipse(Math.cos(rad) * r * 0.5, Math.sin(rad) * r * 0.5, r * 0.5, r * 0.11, a, color, 0.55);
    }
    return s + p.circle(0, 0, Math.max(0.08, r * 0.1), '#7a5a32', 0.9);
  },
  bush(c) {
    const { p, w, d, color, id } = c;
    return `<g transform="scale(1 ${f3(d / w)})">${p.path(scallops(w / 2, 7, rand(id)() * 3), color, 0.45, shade(color, -0.35), 1)}</g>`;
  },
  plant(c) {
    const { p, w, color } = c;
    const r = w / 2;
    let s = p.circle(0, 0, r * 0.45, color, 0.85);
    for (let i = 0; i < 6; i++) {
      const a = (i / 6) * 360 + 15;
      const rad = (a * Math.PI) / 180;
      s += p.ellipse(Math.cos(rad) * r * 0.55, Math.sin(rad) * r * 0.55, r * 0.42, r * 0.16, a, '#4f8a3a', 0.6);
    }
    return s;
  },
  rock(c) {
    const { p, w, d, color, id } = c;
    const rnd = rand(id);
    const pts = Array.from({ length: 9 }, (_, i) => {
      const a = (i / 9) * Math.PI * 2;
      const k = 0.75 + rnd() * 0.25;
      return [(Math.cos(a) * w * k) / 2, (Math.sin(a) * d * k) / 2];
    });
    return p.poly(pts, color, 0.6);
  },
  // ---- mobilier
  chair(c) {
    const { p, w, d, color } = c;
    return p.rrect(-w / 2, -d / 2, w / 2, d / 2, 0.04, color) + p.rect(-w / 2, -d / 2, w / 2, -d / 2 + 0.07, shade(color, -0.3), 0.8);
  },
  directorchair(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.5) + p.line(-w / 2, -d / 2, w / 2, d / 2) + p.line(w / 2, -d / 2, -w / 2, d / 2) + p.rect(-w / 2, -d / 2, w / 2, -d / 2 + 0.06, shade(color, -0.3), 0.9);
  },
  stool(c) {
    return c.p.circle(0, 0, c.w / 2, c.color);
  },
  barstool(c) {
    return c.p.circle(0, 0, c.w / 2, c.color) + c.p.circle(0, 0, c.w / 4, shade(c.color, -0.3), 0.6);
  },
  armchair(c) {
    const { p, w, d, color } = c;
    const t = 0.14;
    return p.rrect(-w / 2, -d / 2, w / 2, d / 2, 0.08, color, 0.45) + p.rect(-w / 2, -d / 2, w / 2, -d / 2 + 0.18, shade(color, -0.25)) + p.rect(-w / 2, -d / 2, -w / 2 + t, d / 2, shade(color, -0.25)) + p.rect(w / 2 - t, -d / 2, w / 2, d / 2, shade(color, -0.25));
  },
  sofa(c) {
    const { p, w, d, color } = c;
    const t = 0.18;
    const seats = Math.max(2, Math.round((w - 2 * t) / 0.6));
    let s = p.rrect(-w / 2, -d / 2, w / 2, d / 2, 0.08, color, 0.45) + p.rect(-w / 2, -d / 2, w / 2, -d / 2 + 0.22, shade(color, -0.25)) + p.rect(-w / 2, -d / 2, -w / 2 + t, d / 2, shade(color, -0.25)) + p.rect(w / 2 - t, -d / 2, w / 2, d / 2, shade(color, -0.25));
    for (let i = 1; i < seats; i++) {
      const x = -w / 2 + t + ((w - 2 * t) * i) / seats;
      s += p.line(x, -d / 2 + 0.22, x, d / 2, shade(color, -0.4), 1);
    }
    return s;
  },
  table(c) {
    return c.p.rrect(-c.w / 2, -c.d / 2, c.w / 2, c.d / 2, 0.03, c.color, 0.6);
  },
  roundtable(c) {
    return c.p.circle(0, 0, c.w / 2, c.color, 0.6) + c.p.circle(0, 0, c.w / 2 - 0.05, 'none', 0, shade(c.color, -0.4));
  },
  coffeetable(c) {
    return c.p.rrect(-c.w / 2, -c.d / 2, c.w / 2, c.d / 2, 0.05, c.color, 0.55);
  },
  desk(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.55) + p.rect(-w * 0.2, -d / 2 + 0.05, w * 0.2, -d / 2 + 0.12, '#30343a', 0.8);
  },
  bed(c) {
    const { p, w, d, color } = c;
    const pillows = w > 1.2 ? 2 : 1;
    let s = p.rect(-w / 2, -d / 2, w / 2, d / 2, '#8a6a4a', 0.5) + p.rect(-w / 2 + 0.04, -d / 2 + 0.45, w / 2 - 0.04, d / 2 - 0.04, color, 0.6);
    for (let i = 0; i < pillows; i++) {
      const x0 = -w / 2 + 0.08 + (i * (w - 0.16)) / pillows;
      s += p.rrect(x0, -d / 2 + 0.08, x0 + (w - 0.16) / pillows - 0.06, -d / 2 + 0.38, 0.06, '#f2f0ea', 0.85);
    }
    return s;
  },
  nightstand(c) {
    return c.p.rect(-c.w / 2, -c.d / 2, c.w / 2, c.d / 2, c.color, 0.6);
  },
  wardrobe(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.6) + p.line(0, -d / 2, 0, d / 2, shade(color, -0.4), 1) + p.line(-w / 2, d / 2, w / 2, d / 2, shade(color, -0.4), 2);
  },
  dresser(c) {
    return SYMBOLS.wardrobe(c);
  },
  bookcase(c) {
    const { p, w, d, color } = c;
    let s = p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.55);
    const books = ['#9b3b32', '#355f8c', '#d8b04a', '#4c7a4a', '#7a4e8c'];
    const n = Math.max(4, Math.round(w / 0.12));
    for (let i = 0; i < n; i++) {
      const x0 = -w / 2 + 0.03 + (i * (w - 0.06)) / n;
      s += p.rect(x0, -d / 2 + 0.04, x0 + (w - 0.06) / n - 0.01, d / 2 - 0.05, books[i % books.length], 0.7, 'stroke="none"');
    }
    return s;
  },
  counter(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.6) + p.rect(-w / 2, d / 2 - 0.08, w / 2, d / 2, shade(color, -0.35), 0.8);
  },
  kitchen(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.55) + p.rrect(-w * 0.3, -d * 0.3, -w * 0.05, d * 0.25, 0.04, '#b9c2cc', 0.8) + [w * 0.12, w * 0.3].map((x) => p.circle(x, -d * 0.12, d * 0.13, '#2a2d31', 0.8) + p.circle(x, d * 0.18, d * 0.11, '#2a2d31', 0.8)).join('');
  },
  fridge(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.7) + p.line(-w / 2, d * 0.15, w / 2, d * 0.15, shade(color, -0.4), 1);
  },
  tv(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.5) + p.rect(-w * 0.45, d * 0.1, w * 0.45, d * 0.35, '#111317', 0.9);
  },
  piano(c) {
    const { p, w, d, color } = c;
    let s = p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.75) + p.rect(-w / 2 + 0.05, d / 2 - 0.16, w / 2 - 0.05, d / 2, '#f4f2ec', 0.95);
    for (let i = 1; i < 12; i++) s += p.line(-w / 2 + 0.05 + ((w - 0.1) * i) / 12, d / 2 - 0.16, -w / 2 + 0.05 + ((w - 0.1) * i) / 12, d / 2 - 0.06, '#222', 1);
    return s;
  },
  floorlamp(c) {
    return c.p.circle(0, 0, c.w / 2, '#ffe4a3', 0.55) + c.p.circle(0, 0, c.w / 8, c.color, 0.9);
  },
  rug(c) {
    const { p, w, d, color } = c;
    let s = p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.3) + p.rect(-w / 2 + 0.1, -d / 2 + 0.1, w / 2 - 0.1, d / 2 - 0.1, 'none', 0, `stroke-dasharray="4 3"`);
    for (let i = 0; i <= 8; i++) {
      const y = -d / 2 + (d * i) / 8;
      s += p.line(-w / 2 - 0.06, y, -w / 2, y, undefined, 1) + p.line(w / 2, y, w / 2 + 0.06, y, undefined, 1);
    }
    return s;
  },
  coatrack(c) {
    let s = c.p.circle(0, 0, c.w / 2, c.color, 0.35);
    for (let i = 0; i < 4; i++) {
      const a = (i / 4) * Math.PI * 2;
      s += c.p.line(0, 0, (Math.cos(a) * c.w) / 2, (Math.sin(a) * c.w) / 2);
    }
    return s;
  },
  divider(c) {
    const { p, w, d, color } = c;
    const n = 3;
    const pts = Array.from({ length: n + 1 }, (_, i) => [-w / 2 + (w * i) / n, i % 2 ? d / 2 : -d / 2]);
    return p.path(`M${pts.map(([x, y]) => `${f3(x)},${f3(y)}`).join(' L')}`, 'none', 0, shade(color, -0.2), 3);
  },
  // ---- murs, portes, fenêtres, structures
  wall(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.85);
  },
  walldoor(c) {
    const { p, w, d, color } = c;
    const door = Math.min(0.9, w * 0.4);
    return p.rect(-w / 2, -d / 2, -door / 2, d / 2, color, 0.85) + p.rect(door / 2, -d / 2, w / 2, d / 2, color, 0.85) + p.line(-door / 2, d / 2, -door / 2, d / 2 + door, undefined, 1.2) + p.path(`M${f3(-door / 2)},${f3(d / 2 + door)} A${f3(door)},${f3(door)} 0 0 0 ${f3(door / 2)},${f3(d / 2)}`, 'none', 0, undefined, 1);
  },
  wallwindow(c) {
    const { p, w, d, color } = c;
    const win = Math.min(1.4, w * 0.5);
    return p.rect(-w / 2, -d / 2, -win / 2, d / 2, color, 0.85) + p.rect(win / 2, -d / 2, w / 2, d / 2, color, 0.85) + p.rect(-win / 2, -d / 2, win / 2, d / 2, '#bfe0f5', 0.5) + p.line(-win / 2, 0, win / 2, 0, undefined, 1);
  },
  door(c) {
    const { p, w, d } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, c.color, 0.7) + p.path(`M${f3(-w / 2)},${f3(d / 2)} L${f3(-w / 2)},${f3(d / 2 + w)} A${f3(w)},${f3(w)} 0 0 0 ${f3(w / 2)},${f3(d / 2)}`, 'none', 0, undefined, 1);
  },
  window(c) {
    const { p, w, d } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, '#bfe0f5', 0.5) + p.line(-w / 2, 0, w / 2, 0, undefined, 1);
  },
  stairs(c) {
    const { p, w, d, color } = c;
    let s = p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.45);
    for (let i = 1; i < 6; i++) s += p.line(-w / 2, -d / 2 + (d * i) / 6, w / 2, -d / 2 + (d * i) / 6, undefined, 1);
    return s + p.line(0, d * 0.4, 0, -d * 0.4, undefined, 1.5) + p.path(`M${f3(-0.12)},${f3(-d * 0.3)} L0,${f3(-d * 0.4)} L${f3(0.12)},${f3(-d * 0.3)}`, 'none', 0, undefined, 1.5);
  },
  riser(c) {
    const { p, w, d, color } = c;
    let s = p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.45);
    for (let x = -w / 2 + 0.25; x < w / 2 + d; x += 0.3) s += p.line(Math.max(-w / 2, x - d), Math.min(d / 2, -d / 2 + (x - Math.max(-w / 2, x - d)) - (x - d < -w / 2 ? 0 : 0)), Math.min(x, w / 2), -d / 2, undefined, 0.6);
    return s;
  },
  column(c) {
    return c.p.circle(0, 0, c.w / 2, c.color, 0.75);
  },
  drape(c) {
    const { p, w, color } = c;
    const n = Math.max(4, Math.round(w / 0.3));
    let d = `M${f3(-w / 2)},0`;
    for (let i = 0; i < n; i++) d += ` q${f3(w / n / 2)},${i % 2 ? -0.1 : 0.1} ${f3(w / n)},0`;
    return p.path(d, 'none', 0, shade(color, 0.3), 3);
  },
  // ---- rue
  streetlamp(c) {
    const { p, d } = c;
    return p.circle(0, -d / 2 + 0.12, 0.12, '#40454c', 0.9) + p.line(0, -d / 2 + 0.12, 0, d / 2 - 0.15, undefined, 2) + p.ellipse(0, d / 2 - 0.12, 0.18, 0.12, 0, '#ffe08a', 0.75);
  },
  bench(c) {
    const { p, w, d, color } = c;
    let s = '';
    for (let i = 0; i < 4; i++) {
      const y0 = -d / 2 + (d * i) / 4;
      s += p.rect(-w / 2, y0 + 0.02, w / 2, y0 + d / 4 - 0.02, color, 0.65);
    }
    return s;
  },
  bin(c) {
    return c.p.circle(0, 0, c.w / 2, c.color, 0.7) + c.p.circle(0, 0, c.w / 2 - 0.06, '#1a1c1f', 0.6);
  },
  barrier(c) {
    const { p, w, d, color } = c;
    let s = p.rect(-w / 2, -0.03, w / 2, 0.03, color, 0.8);
    for (const x of [-w / 2 + 0.1, w / 2 - 0.1]) s += p.line(x, -d / 2, x, d / 2, undefined, 2);
    return s;
  },
  cone(c) {
    return c.p.rect(-c.w / 2, -c.w / 2, c.w / 2, c.w / 2, '#30343a', 0.6) + c.p.circle(0, 0, c.w * 0.35, c.color, 0.9) + c.p.circle(0, 0, c.w * 0.15, '#f4f4f0', 0.9);
  },
  sign(c) {
    return c.p.circle(0, 0, 0.04, '#9aa3ae', 0.9) + c.p.rect(-c.w / 2, -0.03, c.w / 2, 0.03, c.color, 0.85);
  },
  trafficlight(c) {
    const { p, w, d } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, '#25282c', 0.85) + p.circle(0, 0, Math.min(w, d) * 0.25, '#ff4b3a', 0.9);
  },
  // ---- accessoires
  suitcase(c) {
    const { p, w, d, color } = c;
    return p.rrect(-w / 2, -d / 2, w / 2, d / 2, 0.04, color, 0.7) + p.line(-w * 0.15, -d / 2, w * 0.15, -d / 2, undefined, 2);
  },
  cardboard(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.7) + p.line(-w / 2, 0, w / 2, 0, '#d9c49a', 2);
  },
  crate(c) {
    const { p, w, d, color } = c;
    let s = p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.6);
    for (let i = 1; i < 4; i++) s += p.line(-w / 2, -d / 2 + (d * i) / 4, w / 2, -d / 2 + (d * i) / 4, shade(color, -0.4), 1);
    return s;
  },
  flightcase(c) {
    const { p, w, d, color } = c;
    const k = Math.min(w, d) * 0.18;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.85) + [[-1, -1], [1, -1], [1, 1], [-1, 1]].map(([sx, sy]) => p.rect((sx * w) / 2, (sy * d) / 2, (sx * w) / 2 - sx * k, (sy * d) / 2 - sy * k, '#b8bdc4', 0.9)).join('');
  },
  bottle(c) {
    return c.p.circle(0, 0, Math.max(0.03, c.w / 2), c.color, 0.85);
  },
  wineglass(c) {
    return c.p.circle(0, 0, Math.max(0.03, c.w / 2), c.color, 0.5);
  },
  mug(c) {
    return c.p.circle(0, 0, Math.max(0.03, c.w / 2), c.color, 0.8) + c.p.line(c.w / 2, 0, c.w / 2 + 0.03, 0, undefined, 2);
  },
  books(c) {
    return c.p.rect(-c.w / 2, -c.d / 2, c.w / 2, c.d / 2, c.color, 0.75) + c.p.rect(-c.w / 2 + 0.02, -c.d / 2 + 0.03, c.w / 2 - 0.03, c.d / 2 - 0.02, '#f4f2ec', 0.6);
  },
  laptop(c) {
    const { p, w, d, color } = c;
    return p.rect(-w / 2, -d / 2, w / 2, d / 2, color, 0.7) + p.line(-w / 2, -d / 2 + 0.01, w / 2, -d / 2 + 0.01, '#9fc6e8', 2);
  },
  guitar(c) {
    const { p, w, d, color } = c;
    return p.ellipse(0, d * 0.22, w * 0.5, d * 0.22, 0, color, 0.75) + p.ellipse(0, -d * 0.02, w * 0.38, d * 0.15, 0, color, 0.75) + p.rect(-0.025, -d / 2, 0.025, -d * 0.12, '#3b2a1a', 0.9);
  },
  micstand(c) {
    return c.p.circle(0, 0, c.w / 2, '#30343a', 0.5) + c.p.line(0, 0, 0, c.d / 2, undefined, 2) + c.p.circle(0, c.d / 2, 0.04, '#111', 0.9);
  },
  // ---- plateau
  camera(c) {
    const { p, w, color } = c;
    let s = '';
    for (let i = 0; i < 3; i++) {
      const a = (i / 3) * Math.PI * 2 + Math.PI / 2;
      s += p.line(0, 0, (Math.cos(a) * w) / 2, (Math.sin(a) * w) / 2, undefined, 1.5);
    }
    return s + p.rrect(-0.12, -0.2, 0.12, 0.22, 0.03, color, 0.9) + p.rect(-0.06, 0.22, 0.06, 0.38, '#111317', 0.95);
  },
  dolly(c) {
    const { p, w, d, color } = c;
    return p.line(-w * 0.35, -d / 2, -w * 0.35, d / 2, '#9aa3ae', 2) + p.line(w * 0.35, -d / 2, w * 0.35, d / 2, '#9aa3ae', 2) + p.rect(-w * 0.42, -0.45, w * 0.42, 0.45, color, 0.75) + p.rrect(-0.12, -0.15, 0.12, 0.25, 0.03, '#2b2d31', 0.95) + p.rect(-0.06, 0.25, 0.06, 0.4, '#111317', 0.95);
  },
  monitor(c) {
    const { p, w, color } = c;
    return p.circle(0, 0, w * 0.35, color, 0.35) + p.rect(-w * 0.35, 0.05, w * 0.35, 0.12, '#111317', 0.95);
  },
  boom(c) {
    const { p, d } = c;
    return p.circle(0, -d / 2 + 0.2, 0.25, '#30343a', 0.35) + p.line(0, -d / 2 + 0.2, 0, d / 2, '#9aa3ae', 2) + p.ellipse(0, d / 2 - 0.08, 0.04, 0.12, 0, '#4b4f55', 0.95);
  },
};

/** Couleur principale de l'objet (réglage « couleur » du modèle). */
function colorOf(o, type) {
  const props = o.props || {};
  if (props.color) return props.color;
  const prm = type.params?.find((x) => x.key === 'color');
  return prm?.default || '#8a919c';
}

/**
 * Symbole SVG d'un objet de décor vu de dessus, placé et orienté sur le plan.
 * @param {object} ctx  { X, Z, s, T, isSel }
 */
export function propSymbol(o, type, { X, Z, s, T, isSel }) {
  const yaw = o.rotation?.[1] || 0;
  const [w, , d] = propSize(type, o.props || {});
  const stroke = isSel ? T.sel : T.muted;
  const sw = isSel ? 2 : 1;
  const c = { w, d, color: colorOf(o, type), stroke, props: o.props || {}, id: o.id, T, typeId: type.id, p: pen({ stroke, sw }) };
  const draw = SYMBOLS[type.model] || ((x) => x.p.rrect(-x.w / 2, -x.d / 2, x.w / 2, x.d / 2, 0.03, x.color));
  return `<g transform="translate(${f3(X(o.position[0]))},${f3(Z(o.position[2]))}) rotate(${f3((-yaw * 180) / Math.PI)}) scale(${f3(s)})">${draw(c)}</g>`;
}
