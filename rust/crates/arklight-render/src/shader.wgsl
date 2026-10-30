// Moteur 3D d'ARKLIGHT : surfaces éclairées comme le MeshStandardMaterial de three.js (diffus de
// Lambert, reflets GGX, rugosité, métal), spots et sources ponctuelles en candelas (atténuation
// 1/d²), ombres des spots coupés par le grip, rebonds (sources rectangulaires, partie diffuse des
// RectAreaLight de three.js), lumière d'hémisphère, faces émettrices. L'image reste en HDR :
// exposition, halo et tone mapping viennent après (post.wgsl).

struct Globals {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    camera: vec4<f32>,
    // x : lumière ambiante, y : exposition (multiplicateur), z : nombre de lumières, w : de rebonds
    params: vec4<f32>,
};

struct Light {
    // xyz : position, w : 0 spot, 1 source ponctuelle
    pos: vec4<f32>,
    // xyz : direction du faisceau, w : cosinus du bord du cône
    dir: vec4<f32>,
    // rgb : couleur × candelas, w : cosinus du début de la pénombre
    color: vec4<f32>,
    // x : couche de la carte d'ombre (−1 : sans ombre)
    extra: vec4<f32>,
};

struct Area {
    corners: array<vec4<f32>, 4>,
    color: vec4<f32>,
};

struct Lighting {
    areas: array<Area, 6>,
    shadows: array<mat4x4<f32>, 6>,
};

struct BeamData {
    // xyz : sommet du faisceau, w : nombre d'éléments de grip qui le coupent
    apex: vec4<f32>,
    c: array<vec4<f32>, 4>,
    u: array<vec4<f32>, 4>,
    v: array<vec4<f32>, 4>,
    t: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var<storage, read> lights: array<Light>;
@group(0) @binding(2) var<uniform> lighting: Lighting;
@group(0) @binding(3) var shadow_maps: texture_depth_2d_array;
@group(0) @binding(4) var shadow_sampler: sampler_comparison;
@group(0) @binding(5) var<storage, read> beam_data: array<BeamData>;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) m0: vec4<f32>,
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
    // rgb : couleur, w : surface (0 éclairée, 1 sans éclairage, 2 sol quadrillé, 3 facettes)
    @location(6) albedo: vec4<f32>,
    // rgb : lumière émise, w : 1 si les deux faces sont visibles
    @location(7) emissive: vec4<f32>,
    // x : rugosité, y : métal, z : opacité, w : reçoit les ombres
    @location(8) pbr: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) albedo: vec4<f32>,
    @location(3) emissive: vec4<f32>,
    @location(4) pbr: vec4<f32>,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    let world = model * vec4<f32>(v.pos, 1.0);
    var out: VsOut;
    out.clip = globals.view_proj * world;
    out.world = world.xyz;
    // échelles non uniformes : normale par l'inverse transposée (axes divisés par leur échelle²)
    let sx = dot(v.m0.xyz, v.m0.xyz);
    let sy = dot(v.m1.xyz, v.m1.xyz);
    let sz = dot(v.m2.xyz, v.m2.xyz);
    let nl = (v.m0.xyz * v.normal.x / max(sx, 1e-12)) + (v.m1.xyz * v.normal.y / max(sy, 1e-12)) + (v.m2.xyz * v.normal.z / max(sz, 1e-12));
    out.normal = normalize(nl);
    out.albedo = v.albedo;
    out.emissive = v.emissive;
    out.pbr = v.pbr;
    return out;
}

const PI: f32 = 3.14159265;
const RECIPROCAL_PI: f32 = 0.31830988;
const SHADOW_TEXEL: f32 = 1.0 / 1024.0;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
}

// reflets : BRDF_GGX de three.js (Fresnel de Schlick, visibilité de Smith corrélée, GGX)
fn brdf_ggx(l: vec3<f32>, v: vec3<f32>, n: vec3<f32>, f0: vec3<f32>, alpha: f32) -> vec3<f32> {
    let h = normalize(l + v);
    let dot_nl = clamp(dot(n, l), 0.0, 1.0);
    let dot_nv = clamp(dot(n, v), 0.0, 1.0);
    let dot_nh = clamp(dot(n, h), 0.0, 1.0);
    let dot_vh = clamp(dot(v, h), 0.0, 1.0);
    let fresnel = exp2((-5.55473 * dot_vh - 6.98316) * dot_vh);
    let f = f0 * (1.0 - fresnel) + vec3<f32>(fresnel);
    let a2 = alpha * alpha;
    let gv = dot_nl * sqrt(a2 + (1.0 - a2) * dot_nv * dot_nv);
    let gl = dot_nv * sqrt(a2 + (1.0 - a2) * dot_nl * dot_nl);
    let vis = 0.5 / max(gv + gl, 1e-6);
    let denom = dot_nh * dot_nh * (a2 - 1.0) + 1.0;
    let d = RECIPROCAL_PI * a2 / (denom * denom);
    return f * (vis * d);
}

// ombre d'un spot (PCF 3 × 3) : décalage le long de la normale 2 cm, biais −0,0004 (three.js)
fn shadow(layer: i32, world: vec3<f32>, n: vec3<f32>) -> f32 {
    let p = lighting.shadows[layer] * vec4<f32>(world + n * 0.02, 1.0);
    if (p.w <= 0.0) {
        return 1.0;
    }
    let ndc = p.xyz / p.w;
    let uv = ndc.xy * vec2<f32>(0.5, -0.5) + 0.5;
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || ndc.z > 1.0) {
        return 1.0;
    }
    let reference = ndc.z - 0.0004;
    var lit = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            lit += textureSampleCompareLevel(shadow_maps, shadow_sampler, uv + vec2<f32>(f32(x), f32(y)) * SHADOW_TEXEL, layer, reference);
        }
    }
    return lit / 9.0;
}

// rebonds : facteur de forme d'un rectangle (LTC_Evaluate de three.js, partie diffuse)
fn edge_form_factor(v1: vec3<f32>, v2: vec3<f32>) -> vec3<f32> {
    let x = dot(v1, v2);
    let y = abs(x);
    let a = 0.8543985 + (0.4965155 + 0.0145206 * y) * y;
    let b = 3.4175940 + (4.1616724 + y) * y;
    let v = a / b;
    let theta_sintheta = select(0.5 * inverseSqrt(max(1.0 - x * x, 1e-7)) - v, v, x > 0.0);
    return cross(v1, v2) * theta_sintheta;
}

fn area_diffuse(a: Area, n: vec3<f32>, v: vec3<f32>, p: vec3<f32>) -> f32 {
    let c0 = a.corners[0].xyz;
    let light_normal = cross(a.corners[1].xyz - c0, a.corners[3].xyz - c0);
    if (dot(light_normal, p - c0) < 0.0) {
        return 0.0; // derrière la source
    }
    var t1 = v - n * dot(v, n);
    if (dot(t1, t1) < 1e-8) {
        t1 = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.x) > 0.9) - n * select(n.x, n.y, abs(n.x) > 0.9);
    }
    t1 = normalize(t1);
    let t2 = -cross(n, t1);
    let m = transpose(mat3x3<f32>(t1, t2, n));
    var c: array<vec3<f32>, 4>;
    for (var i = 0; i < 4; i++) {
        c[i] = normalize(m * (a.corners[i].xyz - p));
    }
    let f = edge_form_factor(c[0], c[1]) + edge_form_factor(c[1], c[2]) + edge_form_factor(c[2], c[3]) + edge_form_factor(c[3], c[0]);
    let l = length(f);
    return max((l * l + f.z) / (l + 1.0), 0.0);
}

@fragment
fn fs_main(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // dérivées d'abord (hors de toute condition) : quadrillage du sol, rugosité due à la courbure
    let n0 = normalize(in.normal);
    let dxy = max(abs(dpdx(n0)), abs(dpdy(n0)));
    let geometry_roughness = max(max(dxy.x, dxy.y), dxy.z);
    let p = in.world.xz / 0.5;
    let g = abs(fract(p - 0.5) - 0.5) / max(fwidth(p), vec2<f32>(1e-4));
    let line = 1.0 - min(min(g.x, g.y), 1.0);
    // axes centraux plus clairs (GridHelper de three.js)
    let axis = abs(in.world.xz) / max(fwidth(in.world.xz), vec2<f32>(1e-4));
    let center = 1.0 - min(min(axis.x, axis.y), 1.0);
    // facettes (flatShading) : normale de la face, d'après les dérivées de la position
    let facet = cross(dpdx(in.world), dpdy(in.world));

    // face arrière d'un matériau à une seule face : invisible (FrontSide)
    if (!front && in.emissive.w < 0.5) {
        discard;
    }
    let opacity = in.pbr.z;
    if (in.albedo.w > 0.5 && in.albedo.w < 1.5) {
        return vec4<f32>(in.albedo.rgb, opacity); // sans éclairage (MeshBasicMaterial)
    }
    var n = n0;
    if (!front) {
        n = -n;
    }
    let v = normalize(globals.camera.xyz - in.world);
    if (in.albedo.w > 2.5) {
        n = normalize(facet);
        if (dot(n, v) < 0.0) {
            n = -n;
        }
    }
    let metal = in.pbr.y;
    let diffuse_color = in.albedo.rgb * (1.0 - metal);
    let f0 = mix(vec3<f32>(0.04), in.albedo.rgb, metal);
    let roughness = min(max(in.pbr.x, 0.0525) + geometry_roughness, 1.0);
    let alpha = roughness * roughness;
    let receive = in.pbr.w > 0.5;

    // lumière ambiante : hémisphère ciel bleuté / sol sombre (intensité = ambiance × 5)
    let sky = srgb_to_linear(vec3<f32>(0.749, 0.831, 1.0));
    let ground = srgb_to_linear(vec3<f32>(0.102, 0.102, 0.102));
    let hemi = mix(ground, sky, 0.5 * n.y + 0.5) * globals.params.x * 5.0;
    var diffuse = hemi * diffuse_color * RECIPROCAL_PI;
    var specular = vec3<f32>(0.0);

    // projecteurs : E = I cos θ / d² (au moins 10 cm), dans le cône pour les spots
    let count = u32(globals.params.z);
    for (var i = 0u; i < count; i++) {
        let li = lights[i];
        let to_light = li.pos.xyz - in.world;
        let d2 = dot(to_light, to_light);
        let l = to_light * inverseSqrt(max(d2, 1e-8));
        var cone = 1.0;
        if (li.pos.w < 0.5) {
            cone = smoothstep(li.dir.w, li.color.w, dot(-l, li.dir.xyz));
        }
        let ndl = max(dot(n, l), 0.0);
        if (cone * ndl <= 0.0) {
            continue;
        }
        var lit = 1.0;
        if (receive && li.extra.x >= 0.0) {
            lit = shadow(i32(li.extra.x), in.world, n);
        }
        let irradiance = li.color.rgb * (cone * ndl * lit / max(d2, 0.01));
        diffuse += irradiance * diffuse_color * RECIPROCAL_PI;
        specular += irradiance * brdf_ggx(l, v, n, f0, alpha);
    }

    // rebonds (réflecteurs, soies) : luminance × facteur de forme
    let areas = u32(globals.params.w);
    for (var i = 0u; i < areas; i++) {
        let a = lighting.areas[i];
        diffuse += a.color.rgb * diffuse_color * area_diffuse(a, n, v, in.world);
    }

    var color = diffuse + specular + in.emissive.rgb;
    if (in.albedo.w > 1.5 && in.albedo.w < 2.5) {
        // quadrillage du sol tous les 50 cm (traits de 1 pixel, opacité 0,55)
        let lines = mix(srgb_to_linear(vec3<f32>(0.137, 0.153, 0.176)), srgb_to_linear(vec3<f32>(0.235, 0.263, 0.302)), center);
        color = mix(color, lines, 0.55 * max(line, center));
    }
    return vec4<f32>(color, opacity);
}

// Image des sources (halo) : chaque face émettrice n'a que sa lumière émise (× exposition) ; tout le
// reste est noir et masque les sources qu'il cache.
@fragment
fn fs_sources(in: VsOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    if (!front && in.emissive.w < 0.5) {
        discard;
    }
    return vec4<f32>(max(in.emissive.rgb, vec3<f32>(0.0)) * globals.params.y, 1.0);
}

// ------------------------------------------------------------------ faisceaux dans la brume
struct BeamIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) m0: vec4<f32>,
    @location(3) m1: vec4<f32>,
    @location(4) m2: vec4<f32>,
    @location(5) m3: vec4<f32>,
    // rgb : couleur normalisée, w : intensité
    @location(6) color: vec4<f32>,
};

struct BeamOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) along: f32,
    @location(1) normal_v: vec3<f32>,
    @location(2) view_v: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(4) world: vec3<f32>,
    @location(5) @interpolate(flat) index: u32,
};

const BEAM_LENGTH: f32 = 7.0;

@vertex
fn vs_beam(v: BeamIn, @builtin(instance_index) index: u32) -> BeamOut {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    let world = model * vec4<f32>(v.pos, 1.0);
    let mv = globals.view * world;
    var out: BeamOut;
    out.clip = globals.view_proj * world;
    out.along = clamp(v.pos.z / BEAM_LENGTH, 0.0, 1.0);
    out.normal_v = normalize((globals.view * model * vec4<f32>(v.normal, 0.0)).xyz);
    out.view_v = normalize(-mv.xyz);
    out.color = v.color;
    out.world = world.xyz;
    out.index = index;
    return out;
}

// part du faisceau qui passe les éléments de grip entre le sommet et ce point
fn beam_occlusion(b: BeamData, world: vec3<f32>) -> f32 {
    var k = 1.0;
    let apex = b.apex.xyz;
    let d = world - apex;
    let count = i32(b.apex.w);
    for (var i = 0; i < count; i++) {
        let u = b.u[i].xyz;
        let v = b.v[i].xyz;
        let c = b.c[i].xyz;
        let n = cross(u, v);
        let denom = dot(n, d);
        if (abs(denom) < 1e-7) {
            continue;
        }
        let t = dot(n, c - apex) / denom;
        if (t <= 0.0 || t >= 1.0) {
            continue;
        }
        let q = apex + d * t - c;
        if (abs(dot(q, u) / dot(u, u)) <= 1.0 && abs(dot(q, v) / dot(v, v)) <= 1.0) {
            k *= b.t[i];
        }
    }
    return k;
}

@fragment
fn fs_beam(in: BeamOut) -> @location(0) vec4<f32> {
    // bornes avant pow() : au bord du cône l'interpolation dépasse parfois 1 (pow d'un négatif : NaN)
    let edge = pow(clamp(abs(dot(in.normal_v, in.view_v)), 0.0, 1.0), 1.6);
    let along = pow(clamp(1.0 - in.along, 0.0, 1.0), 1.4) * smoothstep(0.0, 0.04, in.along);
    var a = in.color.w * edge * along;
    let b = beam_data[in.index];
    if (b.apex.w > 0.0) {
        a *= beam_occlusion(b, in.world);
    }
    return vec4<f32>(in.color.rgb * a, 1.0);
}

// surbrillance (`HighlightShader` de la version JavaScript) : liseré orangé, plus fort sur les bords
// (là où la surface fuit le regard) ; compensé de l'exposition pour rester visible dans le noir
@fragment
fn fs_highlight(in: VsOut) -> @location(0) vec4<f32> {
    let v = normalize(globals.camera.xyz - in.world);
    let rim = clamp(1.0 - abs(dot(normalize(in.normal), v)), 0.0, 1.0);
    let color = srgb_to_linear(vec3<f32>(1.0, 0.773, 0.420));
    let exposure = max(globals.params.y, 1e-4);
    return vec4<f32>(color * (0.2 + 0.7 * rim) / exposure, 0.05 + 0.55 * pow(rim, 1.6));
}
