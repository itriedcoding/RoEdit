// RoEdit Studio viewport mesh shader: shadowed GGX direct light + sky ambient.

struct Camera {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,   // xyz = position, w = exposure
    sun_dir: vec4<f32>,      // xyz = direction, w = sun intensity
    sun_color: vec4<f32>,    // rgb = colour, w = sky intensity
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    ground: vec4<f32>,
    params: vec4<f32>,       // x = fog density, y = time, z = grid alpha, w = unused
};

struct Object {
    model: mat4x4<f32>,
    color: vec4<f32>,   // rgb = base colour, a = opacity
    misc: vec4<f32>,    // x = roughness, y = metallic, z = emissive strength, w = selected
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var<uniform> object: Object;
@group(1) @binding(1) var shadow_map: texture_depth_2d;
@group(1) @binding(2) var shadow_sampler: sampler_comparison;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) light_clip: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    let world = object.model * vec4<f32>(in.position, 1.0);
    out.world = world.xyz;
    out.clip = camera.view_proj * world;
    out.normal = normalize((object.model * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv = in.uv;
    out.light_clip = camera.light_view_proj * world;
    return out;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let v = normalize(camera.camera_pos.xyz - in.world);
    let l = normalize(camera.sun_dir.xyz);

    // Shadow lookup (uniform control flow: the sample happens before any branch).
    let light_ndc = in.light_clip.xyz / max(in.light_clip.w, 1e-5);
    let shadow_uv = vec2<f32>(light_ndc.x * 0.5 + 0.5, 0.5 - light_ndc.y * 0.5);
    let in_range = step(0.0, shadow_uv.x) * step(shadow_uv.x, 1.0) * step(0.0, shadow_uv.y) * step(shadow_uv.y, 1.0);
    let probed = textureSampleCompare(shadow_map, shadow_sampler, shadow_uv, light_ndc.z - 0.0015);
    let shadow = mix(1.0, probed, in_range);

    let base = object.color.rgb;
    var roughness = clamp(object.misc.x, 0.03, 1.0);
    let metallic = clamp(object.misc.y, 0.0, 1.0);

    let ndotl = max(dot(n, l), 0.0);
    let ndotv = max(dot(n, v), 1e-4);
    let h = normalize(l + v);
    let ndoth = max(dot(n, h), 0.0);
    let vdoth = max(dot(v, h), 0.0);

    let f0 = mix(vec3<f32>(0.04), base, metallic);
    let f = fresnel_schlick(vdoth, f0);
    let shininess = mix(512.0, 4.0, roughness);
    let spec_amount = pow(ndoth, shininess) * (1.0 - roughness) * 0.6;

    let diffuse = base * (1.0 - metallic) / 3.14159265;
    let sun_radiance = camera.sun_color.rgb * camera.sun_dir.w;
    let direct = (diffuse + f * spec_amount) * ndotl * shadow * sun_radiance;

    // Hemisphere ambient from the sky colours.
    let sky = mix(camera.ground.rgb, camera.sky_top.rgb, n.y * 0.5 + 0.5);
    let horizon = mix(camera.sky_horizon.rgb, sky, 0.5);
    let ambient = horizon * camera.sun_color.w * (0.35 + 0.65 * max(n.y, 0.0));

    var color = direct + base * ambient * (1.0 - metallic * 0.6) + base * object.misc.z;
    color = color * camera.camera_pos.w;
    color = color / (color + vec3<f32>(1.0));
    color = pow(color, vec3<f32>(1.0 / 2.2));

    // Selection tint keeps the highlighted object readable.
    let selected = object.misc.w;
    let tint = vec3<f32>(0.25, 0.62, 1.0);
    color = mix(color, color * 0.6 + tint * 0.4, selected * 0.35);

    return vec4<f32>(color, object.color.a);
}
