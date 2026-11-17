// Depth-only pass for the sun shadow map.

struct Light {
    view_proj: mat4x4<f32>,
};

struct Object {
    model: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> light: Light;
@group(1) @binding(0) var<uniform> object: Object;

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return light.view_proj * object.model * vec4<f32>(position, 1.0);
}
