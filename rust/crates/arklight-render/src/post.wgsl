// Après l'image HDR : halo des sources (seuil, flous en 5 niveaux, assemblage : UnrealBloomPass de
// three.js), puis image finale (valeurs aberrantes retirées, exposition, halo ajouté, ACES, sRGB).

struct FullOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// triangle qui couvre l'écran
@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> FullOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: FullOut;
    out.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2<f32>(p.x, 1.0 - p.y);
    return out;
}

struct Blur {
    inv_size: vec2<f32>,
    direction: vec2<f32>,
    // x : poids central, y : nombre de paires, z : seuil, w : douceur du seuil
    params: vec4<f32>,
    offsets: array<vec4<f32>, 3>,
    weights: array<vec4<f32>, 3>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> blur: Blur;

// zones lumineuses : seuil sur la composante la plus forte (une LED rouge saturée rayonne comme
// une blanche de même niveau)
@fragment
fn fs_highpass(in: FullOut) -> @location(0) vec4<f32> {
    let texel = textureSample(src, samp, in.uv);
    let v = max(texel.r, max(texel.g, texel.b));
    let a = smoothstep(blur.params.z, blur.params.z + blur.params.w, v);
    return mix(vec4<f32>(0.0), texel, a);
}

// flou gaussien séparable (paires de prélèvements fusionnées en une lecture bilinéaire)
@fragment
fn fs_blur(in: FullOut) -> @location(0) vec4<f32> {
    var sum = textureSample(src, samp, in.uv).rgb * blur.params.x;
    let pairs = u32(blur.params.y);
    for (var i = 0u; i < pairs; i++) {
        let off = blur.direction * blur.inv_size * blur.offsets[i / 4u][i % 4u];
        sum += (textureSample(src, samp, in.uv + off).rgb + textureSample(src, samp, in.uv - off).rgb) * blur.weights[i / 4u][i % 4u];
    }
    return vec4<f32>(sum, 1.0);
}

struct Composite {
    // force × facteur de chaque niveau (5 niveaux)
    factors: array<vec4<f32>, 2>,
};

@group(0) @binding(0) var level0: texture_2d<f32>;
@group(0) @binding(1) var level1: texture_2d<f32>;
@group(0) @binding(2) var level2: texture_2d<f32>;
@group(0) @binding(3) var level3: texture_2d<f32>;
@group(0) @binding(4) var level4: texture_2d<f32>;
@group(0) @binding(5) var csamp: sampler;
@group(0) @binding(6) var<uniform> composite: Composite;

@fragment
fn fs_composite(in: FullOut) -> @location(0) vec4<f32> {
    let f0 = composite.factors[0];
    let f1 = composite.factors[1];
    let c = textureSample(level0, csamp, in.uv) * f0.x + textureSample(level1, csamp, in.uv) * f0.y + textureSample(level2, csamp, in.uv) * f0.z + textureSample(level3, csamp, in.uv) * f0.w + textureSample(level4, csamp, in.uv) * f1.x;
    return vec4<f32>(c.rgb, 1.0);
}

struct Final {
    // x : exposition, y : halo (0 ou 1), z : plafond des valeurs
    params: vec4<f32>,
};

@group(0) @binding(0) var hdr: texture_2d<f32>;
@group(0) @binding(1) var bloom: texture_2d<f32>;
@group(0) @binding(2) var fsamp: sampler;
@group(0) @binding(3) var<uniform> fin: Final;

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, 12.92 * c, c <= vec3<f32>(0.0031308));
}

// ACES filmique, comme three.js (ACESFilmicToneMapping)
fn rrt_odt_fit(v: vec3<f32>) -> vec3<f32> {
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return a / b;
}

fn aces(color_in: vec3<f32>) -> vec3<f32> {
    let aces_in = mat3x3<f32>(
        vec3<f32>(0.59719, 0.07600, 0.02840),
        vec3<f32>(0.35458, 0.90834, 0.13383),
        vec3<f32>(0.04823, 0.01566, 0.83777),
    );
    let aces_out = mat3x3<f32>(
        vec3<f32>(1.60475, -0.10208, -0.00327),
        vec3<f32>(-0.53108, 1.10813, -0.07276),
        vec3<f32>(-0.07367, -0.00605, 1.07602),
    );
    var c = color_in / 0.6;
    c = aces_in * c;
    c = rrt_odt_fit(c);
    c = aces_out * c;
    return clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_final(in: FullOut) -> @location(0) vec4<f32> {
    var c = textureLoad(hdr, vec2<i32>(in.pos.xy), 0).rgb;
    // valeurs indéfinies (source collée à une surface) : du noir plutôt que des taches qui clignotent
    if (any(c != c)) {
        c = vec3<f32>(0.0);
    }
    c = clamp(c, vec3<f32>(0.0), vec3<f32>(fin.params.z)) * fin.params.x;
    c += textureSample(bloom, fsamp, in.uv).rgb * fin.params.y;
    return vec4<f32>(linear_to_srgb(aces(c)), 1.0);
}
