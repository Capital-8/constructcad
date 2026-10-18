struct Uniforms {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

// The target is a plain (non-sRGB) texture shown by egui, which expects
// gamma-encoded values: shade in linear space, then encode.
fn gamma_from_linear(rgb: vec3<f32>) -> vec3<f32> {
    let cutoff = rgb < vec3<f32>(0.0031308);
    let lower = rgb * vec3<f32>(12.92);
    let higher = vec3<f32>(1.055) * pow(rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(higher, lower, cutoff);
}

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = u.view_proj * vec4<f32>(v.pos, 1.0);
    o.normal = v.normal;
    o.color = v.color;
    return o;
}

@fragment
fn fs_face(i: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    let diffuse = max(dot(n, normalize(u.light_dir.xyz)), 0.0);
    // Hemispheric ambient keeps faces turned away from the sun readable.
    let sky = 0.5 + 0.5 * n.z;
    let k = 0.30 + 0.60 * diffuse + 0.25 * sky;
    return vec4<f32>(gamma_from_linear(i.color.rgb * k), 1.0);
}

@fragment
fn fs_edge(i: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(gamma_from_linear(i.color.rgb), 1.0);
}
