// Colorimétrie (espace sRGB linéaire). Module partagé serveur / navigateur.

export const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);

export function srgbToLinear(c) {
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

export function linearToSrgb(c) {
  return c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055;
}

/** Couleurs approximatives des émetteurs LED, en sRGB linéaire. */
export const EMITTERS = {
  red: [1, 0, 0],
  green: [0, 1, 0],
  blue: [0, 0.02, 1],
  amber: [1, 0.42, 0],
  mint: [0.52, 1, 0.42],
  lime: [0.62, 1, 0.08],
  cyan: [0, 0.85, 1],
  uv: [0.22, 0, 0.55],
};

/** Point du lieu de Planck (approximation de Kim et al., 1667–25000 K). */
export function cctToXy(kelvin) {
  const T = Math.min(25000, Math.max(1667, kelvin));
  const t = 1e3 / T;
  let x;
  if (T <= 4000) x = -0.2661239 * t ** 3 - 0.234358 * t ** 2 + 0.8776956 * t + 0.17991;
  else x = -3.0258469 * t ** 3 + 2.1070379 * t ** 2 + 0.2226347 * t + 0.24039;
  let y;
  if (T <= 2222) y = -1.1063814 * x ** 3 - 1.3481102 * x ** 2 + 2.18555832 * x - 0.20219683;
  else if (T <= 4000) y = -0.9549476 * x ** 3 - 1.37418593 * x ** 2 + 2.09137015 * x - 0.16748867;
  else y = 3.081758 * x ** 3 - 5.8733867 * x ** 2 + 3.75112997 * x - 0.37001483;
  return [x, y];
}

const xyToUv = (x, y) => {
  const d = -2 * x + 12 * y + 3;
  return [(4 * x) / d, (6 * y) / d];
};
const uvToXy = (u, v) => {
  const d = 2 * u - 8 * v + 4;
  return [(3 * u) / d, (2 * v) / d];
};

/** Chromaticité xy -> sRGB linéaire normalisé (composante max = 1). */
export function xyToRgb(x, y) {
  if (y <= 1e-6) return [0, 0, 0];
  const Y = 1;
  const X = (x / y) * Y;
  const Z = ((1 - x - y) / y) * Y;
  let r = 3.2404542 * X - 1.5371385 * Y - 0.4985314 * Z;
  let g = -0.969266 * X + 1.8760108 * Y + 0.041556 * Z;
  let b = 0.0556434 * X - 0.2040259 * Y + 1.0572252 * Z;
  // Hors gamut : on désature vers le blanc plutôt que de couper brutalement.
  const m = Math.min(r, g, b);
  if (m < 0) {
    r -= m;
    g -= m;
    b -= m;
  }
  const M = Math.max(r, g, b) || 1;
  return [r / M, g / M, b / M];
}

/**
 * Température de couleur + teinte vert/magenta -> sRGB linéaire normalisé.
 * @param {number} kelvin
 * @param {number} tint  -1 (plein magenta) .. 0 .. +1 (plein vert) ; ±1 ≈ ±0.02 Duv
 */
export function cctToRgb(kelvin, tint = 0) {
  let [x, y] = cctToXy(kelvin);
  if (tint) {
    const [u0, v0] = xyToUv(x, y);
    const [x2, y2] = cctToXy(kelvin * 1.01);
    const [u1, v1] = xyToUv(x2, y2);
    let du = u1 - u0;
    let dv = v1 - v0;
    const len = Math.hypot(du, dv) || 1;
    du /= len;
    dv /= len;
    // Normale au lieu orientée vers le vert (v croissant).
    let nu = -dv;
    let nv = du;
    if (nv < 0) {
      nu = -nu;
      nv = -nv;
    }
    const duv = tint * 0.02;
    [x, y] = uvToXy(u0 + nu * duv, v0 + nv * duv);
  }
  return xyToRgb(x, y);
}

/**
 * Teinte / saturation -> sRGB linéaire normalisé. À saturation 0 on obtient `white`.
 * @param {number} hue 0..360
 * @param {number} sat 0..1
 * @param {number[]} [white]
 */
export function hsToRgb(hue, sat, white = [1, 1, 1]) {
  const h = (((hue % 360) + 360) % 360) / 60;
  const c = 1;
  const x = c * (1 - Math.abs((h % 2) - 1));
  let rgb;
  if (h < 1) rgb = [c, x, 0];
  else if (h < 2) rgb = [x, c, 0];
  else if (h < 3) rgb = [0, c, x];
  else if (h < 4) rgb = [0, x, c];
  else if (h < 5) rgb = [x, 0, c];
  else rgb = [c, 0, x];
  rgb = rgb.map(srgbToLinear);
  const s = clamp01(sat);
  const out = rgb.map((v, i) => white[i] * (1 - s) + v * s);
  const M = Math.max(...out) || 1;
  return out.map((v) => v / M);
}

// Émetteurs blancs et couleurs secondaires (profils d'autres marques) : blancs chaud et froid
// sur le lieu de Planck, normalisés comme les autres émetteurs.
Object.assign(EMITTERS, {
  ww: cctToRgb(3000),
  cw: cctToRgb(6000),
  magenta: [1, 0, 1],
  yellow: [1, 0.85, 0],
  indigo: [0.3, 0, 1],
});

export function luminance([r, g, b]) {
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** sRGB linéaire -> chaîne CSS (pour l'interface). */
export function rgbToCss([r, g, b], gain = 1) {
  const f = (v) => Math.round(255 * linearToSrgb(clamp01(v * gain)));
  return `rgb(${f(r)}, ${f(g)}, ${f(b)})`;
}
