//! Colorimétrie (espace sRGB linéaire) : portage à l'identique de `shared/engine/color.js`.
//! Les essais `tests/parity.rs` comparent les résultats à ceux de la version JavaScript.

pub type Rgb = [f64; 3];

pub fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

pub fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

pub fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// Couleurs approximatives des émetteurs LED, en sRGB linéaire.
pub fn emitter(name: &str) -> Option<Rgb> {
    Some(match name {
        "red" => [1.0, 0.0, 0.0],
        "green" => [0.0, 1.0, 0.0],
        "blue" => [0.0, 0.02, 1.0],
        "amber" => [1.0, 0.42, 0.0],
        "mint" => [0.52, 1.0, 0.42],
        "lime" => [0.62, 1.0, 0.08],
        "cyan" => [0.0, 0.85, 1.0],
        "uv" => [0.22, 0.0, 0.55],
        "ww" => cct_to_rgb(3000.0, 0.0),
        "cw" => cct_to_rgb(6000.0, 0.0),
        "magenta" => [1.0, 0.0, 1.0],
        "yellow" => [1.0, 0.85, 0.0],
        "indigo" => [0.3, 0.0, 1.0],
        _ => return None,
    })
}

/// Point du lieu de Planck (approximation de Kim et al., 1667–25000 K).
pub fn cct_to_xy(kelvin: f64) -> (f64, f64) {
    let t_k = kelvin.clamp(1667.0, 25000.0);
    let t = 1e3 / t_k;
    let x = if t_k <= 4000.0 {
        -0.266_123_9 * t.powi(3) - 0.234_358 * t.powi(2) + 0.877_695_6 * t + 0.179_91
    } else {
        -3.025_846_9 * t.powi(3) + 2.107_037_9 * t.powi(2) + 0.222_634_7 * t + 0.240_39
    };
    let y = if t_k <= 2222.0 {
        -1.106_381_4 * x.powi(3) - 1.348_110_2 * x.powi(2) + 2.185_558_32 * x - 0.202_196_83
    } else if t_k <= 4000.0 {
        -0.954_947_6 * x.powi(3) - 1.374_185_93 * x.powi(2) + 2.091_370_15 * x - 0.167_488_67
    } else {
        3.081_758 * x.powi(3) - 5.873_386_7 * x.powi(2) + 3.751_129_97 * x - 0.370_014_83
    };
    (x, y)
}

fn xy_to_uv(x: f64, y: f64) -> (f64, f64) {
    let d = -2.0 * x + 12.0 * y + 3.0;
    (4.0 * x / d, 6.0 * y / d)
}

fn uv_to_xy(u: f64, v: f64) -> (f64, f64) {
    let d = 2.0 * u - 8.0 * v + 4.0;
    (3.0 * u / d, 2.0 * v / d)
}

/// Chromaticité xy → sRGB linéaire normalisé (composante max = 1).
pub fn xy_to_rgb(x: f64, y: f64) -> Rgb {
    if y <= 1e-6 {
        return [0.0, 0.0, 0.0];
    }
    let big_y = 1.0;
    let big_x = x / y * big_y;
    let big_z = (1.0 - x - y) / y * big_y;
    let mut r = 3.240_454_2 * big_x - 1.537_138_5 * big_y - 0.498_531_4 * big_z;
    let mut g = -0.969_266 * big_x + 1.876_010_8 * big_y + 0.041_556 * big_z;
    let mut b = 0.055_643_4 * big_x - 0.204_025_9 * big_y + 1.057_225_2 * big_z;
    // hors gamut : on désature vers le blanc plutôt que de couper brutalement
    let m = r.min(g).min(b);
    if m < 0.0 {
        r -= m;
        g -= m;
        b -= m;
    }
    let mx = r.max(g).max(b);
    let mx = if mx == 0.0 { 1.0 } else { mx };
    [r / mx, g / mx, b / mx]
}

/// Température de couleur + teinte vert / magenta → sRGB linéaire normalisé.
/// `tint` : −1 (plein magenta) … 0 … +1 (plein vert) ; ±1 ≈ ±0,02 Duv.
pub fn cct_to_rgb(kelvin: f64, tint: f64) -> Rgb {
    let (mut x, mut y) = cct_to_xy(kelvin);
    if tint != 0.0 {
        let (u0, v0) = xy_to_uv(x, y);
        let (x2, y2) = cct_to_xy(kelvin * 1.01);
        let (u1, v1) = xy_to_uv(x2, y2);
        let (mut du, mut dv) = (u1 - u0, v1 - v0);
        let len = du.hypot(dv);
        let len = if len == 0.0 { 1.0 } else { len };
        du /= len;
        dv /= len;
        // normale au lieu orientée vers le vert (v croissant)
        let (mut nu, mut nv) = (-dv, du);
        if nv < 0.0 {
            nu = -nu;
            nv = -nv;
        }
        let duv = tint * 0.02;
        (x, y) = uv_to_xy(u0 + nu * duv, v0 + nv * duv);
    }
    xy_to_rgb(x, y)
}

/// Teinte / saturation → sRGB linéaire normalisé ; à saturation 0, `white`.
pub fn hs_to_rgb(hue: f64, sat: f64, white: Rgb) -> Rgb {
    let h = ((hue % 360.0) + 360.0) % 360.0 / 60.0;
    let c = 1.0;
    let x = c * (1.0 - ((h % 2.0) - 1.0).abs());
    let rgb = if h < 1.0 {
        [c, x, 0.0]
    } else if h < 2.0 {
        [x, c, 0.0]
    } else if h < 3.0 {
        [0.0, c, x]
    } else if h < 4.0 {
        [0.0, x, c]
    } else if h < 5.0 {
        [x, 0.0, c]
    } else {
        [c, 0.0, x]
    };
    let s = clamp01(sat);
    let out: Vec<f64> = (0..3).map(|i| white[i] * (1.0 - s) + srgb_to_linear(rgb[i]) * s).collect();
    let mx = out.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mx = if mx == 0.0 { 1.0 } else { mx };
    [out[0] / mx, out[1] / mx, out[2] / mx]
}

pub fn luminance([r, g, b]: Rgb) -> f64 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}
