// Programmes d'effets Astera (mode « EFFECT MODE » en DMX et programmes STANDALONE du menu).
// Liste et ordre des programmes : documents « DMX Profiles » Astera (canal Program).
// Le rendu exact des programmes n'est pas documenté : ce moteur en donne une interprétation visuelle.

export const PROGRAMS = [
  'One Color Static',
  'Two Color Static',
  'Three Color Static',
  'Four Color Static',
  'One Color Fade',
  'Two Color Fade',
  'Three Color Fade',
  'Four Color Fade',
  'Simple Running',
  'Double Running',
  'Two Col Running',
  'Flag Running',
  'Double Flag Running',
  'Spiral 4 Color',
  'Spiral 2 Color',
  'Rainbow',
  'Fire',
  'Rotor',
  'Rotor Split 2',
  'Rotor Split 4',
];

/** Plages DMX du canal Program (début de chaque plage). */
const PROGRAM_STARTS = [0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 102, 110, 118, 126, 134, 142, 150];

export function programFromDmx(v) {
  let p = 0;
  for (let i = 0; i < PROGRAM_STARTS.length; i++) if (v >= PROGRAM_STARTS[i]) p = i;
  return p;
}

/** Vitesse 0..1 (lent -> rapide) -> durée d'un cycle en secondes (10 s .. 0,09 s, échelle log). */
export function cycleSeconds(speed) {
  const s = Math.max(0, Math.min(1, speed));
  return 10 * Math.pow(0.09 / 10, s);
}

/** Direction (valeur DMX) -> { reverse, once } */
export function directionFromDmx(v) {
  if (v < 64) return { reverse: false, once: false };
  if (v < 128) return { reverse: false, once: true };
  if (v < 191) return { reverse: true, once: true };
  return { reverse: true, once: false };
}

const mix = (a, b, k) => [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k];
const BLACK = [0, 0, 0];

function hash(n) {
  const x = Math.sin(n * 127.1 + 311.7) * 43758.5453;
  return x - Math.floor(x);
}

function hueRgb(h) {
  const k = (n) => (n + h * 6) % 6;
  const f = (n) => 1 - Math.max(0, Math.min(k(n), 4 - k(n), 1));
  return [f(5), f(3), f(1)];
}

/**
 * Calcule les couleurs des pixels pour un programme.
 * @param {object} p
 * @param {number} p.program        index dans PROGRAMS
 * @param {number[][]} p.colors     jusqu'à 4 couleurs (sRGB linéaire)
 * @param {number} p.pixels
 * @param {number} p.time           secondes depuis le (re)départ du programme
 * @param {number} [p.speed]        0..1
 * @param {number} [p.fade]         0 (pas à pas) .. 1 (fondu)
 * @param {boolean} [p.reverse]
 * @param {boolean} [p.once]
 * @param {number} [p.size]         taille virtuelle (1..4 projecteurs)
 * @param {number} [p.offset]       0..1 : position dans le programme virtuel
 * @param {Float32Array} out
 */
export function renderProgram({ program, colors, pixels, time, speed = 0.5, fade = 0.5, reverse = false, once = false, size = 1, offset = 0 }, out) {
  const cols = [0, 1, 2, 3].map((i) => colors[i] || colors[0] || BLACK);
  let phase = time / cycleSeconds(speed);
  if (once) phase = Math.min(phase, 0.999);
  if (reverse) phase = -phase;
  const total = pixels * Math.max(1, size);
  const shift = Math.round(offset * (Math.max(1, size) - 1)) * pixels;
  const pos = (p) => (p + shift) / total; // position 0..1 dans le programme virtuel
  const step = (x) => {
    // avance par paliers avec un fondu réglable
    const i = Math.floor(x);
    const f = x - i;
    const k = fade <= 0 ? 0 : Math.min(1, Math.max(0, (f - (1 - fade)) / Math.max(fade, 1e-3)));
    return { i, k };
  };
  for (let p = 0; p < pixels; p++) {
    const u = pos(p);
    let c;
    switch (program) {
      case 0:
      case 1:
      case 2:
      case 3: {
        const n = program + 1;
        c = cols[Math.min(n - 1, Math.floor(u * n))];
        break;
      }
      case 4: {
        const k = 0.5 - 0.5 * Math.cos(phase * 2 * Math.PI);
        c = mix(BLACK, cols[0], fade > 0 ? k : k > 0.5 ? 1 : 0);
        break;
      }
      case 5:
      case 6:
      case 7: {
        const n = program - 3;
        const { i, k } = step((((phase * n) % n) + n) % n);
        c = mix(cols[i % n], cols[(i + 1) % n], k);
        break;
      }
      case 8:
      case 9: {
        const head = (((phase % 1) + 1) % 1) * total;
        const d = Math.abs(p + shift - head);
        const dots = program === 9 ? Math.min(d, Math.abs(total - 1 - (p + shift) - head)) : d;
        const width = 1 + fade * 2;
        c = mix(BLACK, cols[0], Math.max(0, 1 - dots / width));
        break;
      }
      case 10: {
        const x = u * 4 - phase * 2;
        const { i, k } = step(((x % 2) + 2) % 2);
        c = mix(cols[i % 2], cols[(i + 1) % 2], k);
        break;
      }
      case 11:
      case 12: {
        let x = u - phase;
        if (program === 12) x = Math.abs(u - 0.5) * 2 - phase;
        const { i, k } = step((((x * 4) % 4) + 4) % 4);
        c = mix(cols[i % 4], cols[(i + 1) % 4], k);
        break;
      }
      case 13:
      case 14: {
        const n = program === 13 ? 4 : 2;
        const x = p + shift + Math.floor(phase * total);
        c = cols[((x % n) + n) % n];
        break;
      }
      case 15:
        c = hueRgb((((u - phase) % 1) + 1) % 1);
        break;
      case 16: {
        const t = Math.floor(time * (6 + speed * 20));
        const r = hash(p * 13.7 + t);
        c = mix([1, 0.18, 0], [1, 0.55, 0.05], r).map((v) => v * (0.45 + 0.55 * hash(p + t * 3.1)));
        break;
      }
      default: {
        const n = program === 18 ? 2 : program === 19 ? 4 : 1;
        const w = 0.5 + 0.5 * Math.cos(2 * Math.PI * (u * n - phase));
        c = mix(cols[1] || BLACK, cols[0], w);
      }
    }
    out[p * 3] = c[0];
    out[p * 3 + 1] = c[1];
    out[p * 3 + 2] = c[2];
  }
}
