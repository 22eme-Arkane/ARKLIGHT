// Image de fond du plan posée au sol (comme `MeshBasicMaterial` : sans éclairage).

struct Decal {
    view_proj: mat4x4<f32>,
    // x : opacité
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Decal;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_decal(@location(0) pos: vec3<f32>, @location(1) uv: vec2<f32>) -> VOut {
    var out: VOut;
    out.pos = u.view_proj * vec4<f32>(pos, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_decal(in: VOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    return vec4<f32>(c.rgb, c.a * u.params.x);
}
