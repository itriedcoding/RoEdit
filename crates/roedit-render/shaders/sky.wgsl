// Fullscreen sky gradient reconstructed from the inverse view-projection.

struct Camera {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    ground: vec4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    let p = positions[index];
    out.clip = vec4<f32>(p, 1.0, 1.0);
    out.uv = p * 0.5 + vec2<f32>(0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let ndc = vec4<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0, 1.0, 1.0);
    let world = camera.inv_view_proj * ndc;
    let dir = normalize(world.xyz / world.w - camera.camera_pos.xyz);
    let up = clamp(dir.y, -1.0, 1.0);

    var color: vec3<f32>;
    if (up >= 0.0) {
        color = mix(camera.sky_horizon.rgb, camera.sky_top.rgb, pow(up, 0.55));
    } else {
        color = mix(camera.sky_horizon.rgb, camera.ground.rgb, pow(-up, 0.4));
    }

    // Soft sun glow near the light direction.
    let cos_sun = clamp(dot(dir, normalize(camera.sun_dir.xyz)), 0.0, 1.0);
    let glow = pow(cos_sun, 256.0) * 0.8 + pow(cos_sun, 16.0) * 0.15;
    color = color * camera.sun_color.w + camera.sun_color.rgb * glow * camera.sun_dir.w * 0.1;
    color = color * camera.camera_pos.w;
    color = color / (color + vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(color, 1.0);
}
