// Cartes d'ombre : profondeur des objets qui font de l'ombre, vus depuis un spot.

@group(0) @binding(0) var<uniform> light_view_proj: mat4x4<f32>;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) m0: vec4<f32>,
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
};

@vertex
fn vs_shadow(v: VsIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    return light_view_proj * model * vec4<f32>(v.pos, 1.0);
}
