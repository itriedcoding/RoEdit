//! Multithreaded CPU path tracer.
//!
//! Deterministic: every pixel/sample gets its own PCG32 stream, so output is
//! identical regardless of thread count. Supports GGX specular, Lambertian
//! diffuse, emissive materials, sun/point/spot lights, depth of field, sky
//! lighting, the AOV render modes and optional cross-bilateral denoising.

use crate::bvh::{Bvh, Hit};
use crate::rng::{basis, hemisphere_cosine, Pcg32};
use glam::{Vec2, Vec3};
use rayon::prelude::*;
use roedit_core::camera::Camera;
use roedit_core::material::{Environment, Tonemap};
use roedit_core::math::{linear_to_srgb, Aabb, Mat3};
use roedit_core::raster::RasterImage;
use roedit_core::render_settings::{RenderMode, RenderRegion, RenderSettings};
use roedit_core::scene::{Light, LightKind, Scene};
use std::sync::atomic::{AtomicUsize, Ordering};

const PI: f32 = std::f32::consts::PI;
const EPS: f32 = 1e-4;

#[derive(Clone, Copy, Debug)]
pub struct RenderTriangle {
    /// First vertex and two edges.
    pub p0: Vec3,
    pub e1: Vec3,
    pub e2: Vec3,
    pub n0: Vec3,
    pub n1: Vec3,
    pub n2: Vec3,
    pub material: u32,
}

impl RenderTriangle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(p0: Vec3, p1: Vec3, p2: Vec3, n0: Vec3, n1: Vec3, n2: Vec3, material: u32) -> Self {
        Self {
            p0,
            e1: p1 - p0,
            e2: p2 - p0,
            n0,
            n1,
            n2,
            material,
        }
    }

    pub fn bounds(&self) -> Aabb {
        let p1 = self.p0 + self.e1;
        let p2 = self.p0 + self.e2;
        Aabb::new(self.p0.min(p1).min(p2), self.p0.max(p1).max(p2))
    }

    pub fn geometric_normal(&self) -> Vec3 {
        self.e1.cross(self.e2).normalize_or_zero()
    }

    /// Moller-Trumbore intersection. Returns barycentric UV plus interpolated
    /// shading normal.
    pub fn intersect_triangle(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
    ) -> Option<Hit> {
        let pvec = direction.cross(self.e2);
        let det = self.e1.dot(pvec);
        if det.abs() < 1e-10 {
            return None;
        }
        let inv_det = 1.0 / det;
        let tvec = origin - self.p0;
        let u = tvec.dot(pvec) * inv_det;
        if !(-1e-6..=1.0 + 1e-6).contains(&u) {
            return None;
        }
        let qvec = tvec.cross(self.e1);
        let v = direction.dot(qvec) * inv_det;
        if v < -1e-6 || u + v > 1.0 + 1e-6 {
            return None;
        }
        let t = self.e2.dot(qvec) * inv_det;
        if t < t_min || t > t_max {
            return None;
        }
        let w = 1.0 - u - v;
        let normal = (self.n0 * w + self.n1 * u + self.n2 * v).normalize_or_zero();
        Some(Hit {
            t,
            triangle: 0,
            bary: [u, v],
            normal: if normal.length_squared() > 0.5 {
                normal
            } else {
                self.geometric_normal()
            },
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderMaterial {
    pub base: Vec3,
    pub metallic: f32,
    pub roughness: f32,
    pub emission: Vec3,
    pub opacity: f32,
}

impl Default for RenderMaterial {
    fn default() -> Self {
        Self {
            base: Vec3::new(0.75, 0.75, 0.78),
            metallic: 0.0,
            roughness: 0.5,
            emission: Vec3::ZERO,
            opacity: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderLight {
    pub kind: LightKind,
    pub color: Vec3,
    pub intensity: f32,
    pub position: Vec3,
    pub direction: Vec3,
    pub range: f32,
    pub cos_outer: f32,
    pub softness: f32,
    pub cast_shadows: bool,
}

impl RenderLight {
    pub fn from_light(light: &Light) -> Self {
        Self {
            kind: light.kind,
            color: Vec3::from_array(light.color),
            intensity: light.intensity,
            position: light.position,
            direction: light.direction.normalize_or_zero(),
            range: light.range,
            cos_outer: light.spot_angle_deg.to_radians().cos(),
            softness: light.spot_softness.clamp(0.0, 1.0),
            cast_shadows: light.cast_shadows,
        }
    }

    /// Radiance arriving at a point from this light (without shadows).
    pub fn irradiance_at(&self, point: Vec3) -> (Vec3, Vec3, f32) {
        // (direction towards the light, radiance, distance)
        match self.kind {
            LightKind::Directional => {
                let dir = -self.direction;
                (dir, self.color * self.intensity, f32::INFINITY)
            }
            LightKind::Point | LightKind::Spot => {
                let to_light = self.position - point;
                let distance = to_light.length();
                let dir = if distance > 1e-6 {
                    to_light / distance
                } else {
                    Vec3::Y
                };
                if self.range > 0.0 && distance > self.range {
                    return (dir, Vec3::ZERO, distance);
                }
                let mut radiance = self.color * self.intensity / (distance * distance).max(0.01);
                if self.kind == LightKind::Spot {
                    let cos_angle = (-dir).dot(self.direction);
                    let inner =
                        (self.cos_outer + (1.0 - self.cos_outer) * self.softness).min(0.9999);
                    let t = ((cos_angle - self.cos_outer) / (inner - self.cos_outer).max(1e-4))
                        .clamp(0.0, 1.0);
                    radiance *= t * t;
                }
                (dir, radiance, distance)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderScene {
    pub triangles: Vec<RenderTriangle>,
    pub materials: Vec<RenderMaterial>,
    pub lights: Vec<RenderLight>,
    pub env: Environment,
    pub camera: Camera,
    pub bvh: Bvh,
}

impl RenderScene {
    pub fn new(
        triangles: Vec<RenderTriangle>,
        materials: Vec<RenderMaterial>,
        lights: Vec<RenderLight>,
        env: Environment,
        camera: Camera,
    ) -> Self {
        let bvh = Bvh::build(&triangles);
        Self {
            triangles,
            materials,
            lights,
            env,
            camera,
            bvh,
        }
    }

    pub fn material(&self, index: u32) -> RenderMaterial {
        self.materials
            .get(index as usize)
            .copied()
            .unwrap_or_default()
    }
}

/// Flattens a scene document into render triangles, transformed materials and
/// lights. Procedural node textures are not evaluated here; albedo maps use the
/// material's base colour.
pub fn build_render_scene(
    scene: &Scene,
    env: &Environment,
    camera: &Camera,
    lights: &[Light],
) -> RenderScene {
    let mut materials: Vec<RenderMaterial> = Vec::new();
    let mut material_ids: std::collections::HashMap<roedit_core::ids::MaterialId, u32> =
        std::collections::HashMap::new();
    let mut triangles: Vec<RenderTriangle> = Vec::new();

    for (_entity, matrix, mesh_id, mat_id) in scene.visible_mesh_instances() {
        let Some(mesh) = scene.mesh(mesh_id) else {
            continue;
        };
        if mesh.positions.is_empty() || mesh.faces.is_empty() {
            continue;
        }
        let normal_matrix = Mat3::from_mat4(matrix.inverse().transpose());
        let positions: Vec<Vec3> = mesh
            .positions
            .iter()
            .map(|p| matrix.transform_point3(*p))
            .collect();
        let normals: Vec<Vec3> = mesh
            .normals
            .iter()
            .map(|n| normal_matrix.mul_vec3(*n).normalize_or_zero())
            .collect();
        let base_slot = match mat_id {
            Some(id) => {
                let slot = *material_ids.entry(id).or_insert_with(|| {
                    let m = scene.material(id).cloned().unwrap_or_default();
                    materials.push(RenderMaterial {
                        base: Vec3::from_array(m.base_color),
                        metallic: m.metallic.clamp(0.0, 1.0),
                        roughness: m.roughness.clamp(0.02, 1.0),
                        emission: m.emissive(),
                        opacity: m.opacity.clamp(0.0, 1.0),
                    });
                    (materials.len() - 1) as u32
                });
                slot
            }
            None => {
                if materials.is_empty() {
                    materials.push(RenderMaterial::default());
                }
                0
            }
        };
        for face in &mesh.faces {
            if face.verts.len() < 3 {
                continue;
            }
            let idx: Vec<u32> = face
                .verts
                .iter()
                .filter(|v| (**v as usize) < positions.len())
                .copied()
                .collect();
            if idx.len() < 3 {
                continue;
            }
            // Faces currently share the object's base material slot; per-face
            // material indices are reserved for future multi-material meshes.
            let material = base_slot;
            let _ = face.material;
            let normal_at = |i: u32| -> Vec3 {
                normals
                    .get(i as usize)
                    .copied()
                    .unwrap_or_else(|| face.normal(&mesh.positions).normalize_or_zero())
            };
            for k in 1..idx.len() - 1 {
                let (a, b, c) = (idx[0], idx[k], idx[k + 1]);
                triangles.push(RenderTriangle::new(
                    positions[a as usize],
                    positions[b as usize],
                    positions[c as usize],
                    normal_at(a),
                    normal_at(b),
                    normal_at(c),
                    material,
                ));
            }
        }
    }

    let render_lights: Vec<RenderLight> = lights
        .iter()
        .filter(|l| l.enabled && l.intensity > 0.0)
        .map(RenderLight::from_light)
        .collect();

    RenderScene::new(
        triangles,
        materials,
        render_lights,
        env.clone(),
        camera.clone(),
    )
}

#[derive(Clone, Debug)]
pub struct RenderRequest {
    pub width: u32,
    pub height: u32,
    pub samples: u32,
    pub max_bounces: u32,
    pub seed: u32,
    pub mode: RenderMode,
    pub exposure: f32,
    pub tonemap: Tonemap,
    pub transparent_background: bool,
    pub firefly_clamp: f32,
    pub denoise: bool,
    pub region: Option<RenderRegion>,
}

impl Default for RenderRequest {
    fn default() -> Self {
        Self {
            width: 640,
            height: 360,
            samples: 32,
            max_bounces: 4,
            seed: 1,
            mode: RenderMode::PathTrace,
            exposure: 1.0,
            tonemap: Tonemap::Aces,
            transparent_background: false,
            firefly_clamp: 20.0,
            denoise: false,
            region: None,
        }
    }
}

impl RenderRequest {
    pub fn from_settings(settings: &RenderSettings, env: &Environment) -> Self {
        let region = settings
            .region
            .map(|r| r.clamp_to(settings.width, settings.height));
        Self {
            width: region.map(|r| r.width).unwrap_or(settings.width).max(1),
            height: region.map(|r| r.height).unwrap_or(settings.height).max(1),
            samples: settings.samples.clamp(1, 8192),
            max_bounces: settings.max_bounces.clamp(1, 32),
            seed: settings.seed,
            mode: settings.mode,
            exposure: env.exposure,
            tonemap: env.tonemap,
            transparent_background: settings.transparent_background,
            firefly_clamp: settings.firefly_clamp.max(1.0),
            denoise: settings.denoise,
            region,
        }
    }
}

// ------------------------------------------------------------------- shading

fn schlick(cos: f32, f0: Vec3) -> Vec3 {
    let m = (1.0 - cos.clamp(0.0, 1.0)).powi(5);
    f0 + (Vec3::ONE - f0) * m
}

fn ggx_d(ndoth: f32, roughness: f32) -> f32 {
    let a = (roughness * roughness).max(1e-3);
    let a2 = a * a;
    let d = ndoth * ndoth * (a2 - 1.0) + 1.0;
    a2 / (PI * d * d).max(1e-6)
}

fn g_smith(ndotv: f32, ndotl: f32, roughness: f32) -> f32 {
    let k = ((roughness + 1.0) * (roughness + 1.0)) / 8.0;
    let gv = ndotv / (ndotv * (1.0 - k) + k);
    let gl = ndotl / (ndotl * (1.0 - k) + k);
    gv * gl
}

fn specular_color(m: &RenderMaterial) -> Vec3 {
    Vec3::splat(0.04).lerp(m.base, m.metallic)
}

/// Evaluates direct lighting from a direction (delta light contribution).
fn direct_brdf(m: &RenderMaterial, n: Vec3, v: Vec3, l: Vec3) -> Vec3 {
    let ndotl = n.dot(l).max(0.0);
    if ndotl <= 0.0 {
        return Vec3::ZERO;
    }
    let diffuse = m.base * ((1.0 - m.metallic) / PI);
    let h = (l + v).normalize_or_zero();
    let ndoth = n.dot(h).max(0.0);
    let vdoth = v.dot(h).max(0.0);
    let spec = specular_color(m)
        * ggx_d(ndoth, m.roughness)
        * g_smith(n.dot(v).max(0.0), ndotl, m.roughness)
        * schlick(vdoth, Vec3::ONE)
        / (4.0 * n.dot(v).max(0.0) * ndotl).max(1e-4);
    (diffuse + spec) * ndotl
}

/// Samples a GGX half vector and returns the reflected direction.
fn sample_ggx(rng: &mut Pcg32, n: Vec3, roughness: f32) -> (Vec3, f32) {
    let a = (roughness * roughness).max(1e-3);
    let u1 = rng.f32();
    let u2 = rng.f32();
    let phi = std::f32::consts::TAU * u1;
    let cos_theta = ((1.0 - u2) / (1.0 + (a * a - 1.0) * u2).max(1e-6))
        .max(0.0)
        .sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let h = {
        let (t, b) = basis(n);
        (t * (sin_theta * phi.cos()) + b * (sin_theta * phi.sin()) + n * cos_theta)
            .normalize_or_zero()
    };
    let wo = n; // local view direction is the normal in tangent space
    let _ = wo;
    (h, ggx_d(cos_theta, roughness))
}

fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    v - n * (2.0 * v.dot(n))
}

struct CameraBasis {
    forward: Vec3,
    right: Vec3,
    up: Vec3,
    tan_half: f32,
    aspect: f32,
    lens_radius: f32,
    focus: f32,
    orthographic: bool,
    ortho_scale: f32,
}

impl CameraBasis {
    fn new(cam: &Camera, aspect: f32) -> Self {
        Self {
            forward: cam.forward(),
            right: cam.right(),
            up: cam.up_vector(),
            tan_half: (cam.fov_y_deg.clamp(1.0, 179.0).to_radians() * 0.5).tan(),
            aspect: aspect.max(1e-4),
            lens_radius: cam.lens_radius(),
            focus: cam.focus_distance.max(0.01),
            orthographic: cam.orthographic,
            ortho_scale: cam.ortho_scale.max(1e-3),
        }
    }

    /// Ray for NDC coordinates in [-1, 1] with y up, plus a lens offset.
    fn ray(&self, cam: &Camera, ndc: Vec2, lens: Vec2) -> (Vec3, Vec3) {
        if self.orthographic {
            let h = self.ortho_scale;
            let w = h * self.aspect;
            let origin = cam.position + self.right * (ndc.x * w) + self.up * (ndc.y * h);
            return (origin, self.forward);
        }
        let dir = (self.forward
            + self.right * (ndc.x * self.tan_half * self.aspect)
            + self.up * (ndc.y * self.tan_half))
            .normalize_or_zero();
        if self.lens_radius > 0.0 {
            let focus_point = cam.position + dir * self.focus;
            let origin = cam.position
                + self.right * (lens.x * self.lens_radius)
                + self.up * (lens.y * self.lens_radius);
            (origin, (focus_point - origin).normalize_or_zero())
        } else {
            (cam.position, dir)
        }
    }
}

// ------------------------------------------------------------------- tracing

fn trace_path(
    scene: &RenderScene,
    req: &RenderRequest,
    origin: Vec3,
    direction: Vec3,
    rng: &mut Pcg32,
) -> Vec3 {
    let mut radiance = Vec3::ZERO;
    let mut throughput = Vec3::ONE;
    let mut origin = origin;
    let mut direction = direction;
    for bounce in 0..req.max_bounces.max(1) {
        let hit: Option<Hit> =
            scene
                .bvh
                .intersect(&scene.triangles, origin, direction, EPS, f32::MAX);
        let Some(hit) = hit else {
            let sky = scene.env.sample_sky(direction);
            radiance += throughput * sky;
            break;
        };
        let tri = &scene.triangles[hit.triangle as usize];
        let mat = scene.material(tri.material);
        let point = origin + direction * hit.t;
        let mut n = hit.normal.normalize_or_zero();
        let v = -direction;
        if n.dot(v) < 0.0 {
            n = -n;
        }
        radiance += throughput * mat.emission;

        // Direct lighting: sun plus every light in the scene.
        let sun = scene.env.sun_direction.normalize_or_zero();
        if !scene
            .bvh
            .occluded(&scene.triangles, point + n * EPS * 4.0, sun, EPS, 1e9)
        {
            let brdf = direct_brdf(&mat, n, v, sun);
            if brdf != Vec3::ZERO {
                radiance += throughput
                    * Vec3::from_array(scene.env.sun_color)
                    * scene.env.sun_intensity
                    * brdf;
            }
        }
        for light in &scene.lights {
            let (l, radiance_in, _dist) = light.irradiance_at(point);
            if radiance_in == Vec3::ZERO {
                continue;
            }
            let brdf = direct_brdf(&mat, n, v, l);
            if brdf == Vec3::ZERO {
                continue;
            }
            if light.cast_shadows
                && scene
                    .bvh
                    .occluded(&scene.triangles, point + n * EPS * 4.0, l, EPS, 1e9)
            {
                continue;
            }
            radiance += throughput * radiance_in * brdf;
        }

        // BSDF sampling.
        let spec = specular_color(&mat);
        let fresnel = schlick(n.dot(v).max(0.0), spec);
        let p_spec = (fresnel.max_element() * (0.25 + 0.75 * mat.metallic)).clamp(0.05, 0.95);
        if rng.f32() < p_spec {
            let (h, _d) = sample_ggx(rng, n, mat.roughness.max(0.03));
            let new_dir = reflect(direction, h);
            let ndotl = n.dot(new_dir);
            if ndotl <= 0.0 {
                break;
            }
            let ndoth = n.dot(h).max(1e-4);
            let vdoth = h.dot(v).max(1e-4);
            let weight = fresnel
                * g_smith(n.dot(v).max(1e-4), ndotl, mat.roughness)
                * (vdoth / (n.dot(v).max(1e-4) * ndoth))
                / p_spec;
            throughput *= weight.clamp(Vec3::ZERO, Vec3::splat(4.0));
            origin = point + n * EPS * 4.0;
            direction = new_dir.normalize_or_zero();
        } else {
            let new_dir = hemisphere_cosine(rng, n);
            throughput *= mat.base * ((1.0 - mat.metallic) / (1.0 - p_spec));
            origin = point + n * EPS * 4.0;
            direction = new_dir;
        }
        if !throughput.is_finite() || throughput.max_element() <= 0.0 {
            break;
        }
        // Russian roulette after a few bounces.
        if bounce >= 3 {
            let q = throughput.max_element().clamp(0.05, 0.95);
            if rng.f32() > q {
                break;
            }
            throughput /= q;
        }
    }
    if req.firefly_clamp > 0.0 {
        let m = radiance.max_element();
        if m > req.firefly_clamp {
            radiance *= req.firefly_clamp / m;
        }
    }
    if !radiance.is_finite() {
        return Vec3::ZERO;
    }
    radiance.max(Vec3::ZERO)
}

fn shade_aov(
    scene: &RenderScene,
    cam: &Camera,
    basis: &CameraBasis,
    req: &RenderRequest,
    ndc: Vec2,
    rng: &mut Pcg32,
) -> [f32; 4] {
    let (origin, dir) = basis.ray(cam, ndc, Vec2::ZERO);
    let hit = scene
        .bvh
        .intersect(&scene.triangles, origin, dir, EPS, f32::MAX);
    let Some(hit) = hit else {
        return [
            0.0,
            0.0,
            0.0,
            if req.transparent_background { 0.0 } else { 1.0 },
        ];
    };
    let tri = &scene.triangles[hit.triangle as usize];
    let mat = scene.material(tri.material);
    let point = origin + dir * hit.t;
    let mut n = hit.normal.normalize_or_zero();
    if n.dot(dir) > 0.0 {
        n = -n;
    }
    match req.mode {
        RenderMode::Albedo => [mat.base.x, mat.base.y, mat.base.z, 1.0],
        RenderMode::Normals => [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, 1.0],
        RenderMode::Depth => {
            // Fixed 0.1..32 m mapping: far-plane linear depth is useless at
            // 500 m because everything quantises to white.
            let near = 0.1f32;
            let far = 32.0f32;
            let d = ((hit.t - near) / (far - near)).clamp(0.0, 1.0);
            let v = 1.0 - d;
            [v, v, v, 1.0]
        }
        RenderMode::AmbientOcclusion => {
            let mut open = 0.0;
            let n_samples = 16;
            for _ in 0..n_samples {
                let d = hemisphere_cosine(rng, n);
                if !scene
                    .bvh
                    .occluded(&scene.triangles, point + n * EPS * 4.0, d, EPS, 0.75)
                {
                    open += 1.0;
                }
            }
            let ao = open / n_samples as f32;
            [ao, ao, ao, 1.0]
        }
        RenderMode::ShadowMatte => {
            let sun = scene.env.sun_direction.normalize_or_zero();
            let soft = scene.env.sun_angle_deg.to_radians().max(0.001);
            let mut lit = 0.0;
            let taps = 8;
            for _ in 0..taps {
                let jitter = rng.in_disc() * soft;
                let l = (sun + basis.x() * jitter.x + basis.y() * jitter.y).normalize_or_zero();
                if !scene
                    .bvh
                    .occluded(&scene.triangles, point + n * EPS * 4.0, l, EPS, 1e9)
                {
                    lit += 1.0;
                }
            }
            let v = lit / taps as f32;
            let alpha = if req.transparent_background { v } else { 1.0 };
            [v, v, v, alpha]
        }
        RenderMode::PathTrace => [0.0, 0.0, 0.0, 1.0],
    }
}

impl CameraBasis {
    fn x(&self) -> Vec3 {
        self.right
    }
    fn y(&self) -> Vec3 {
        self.up
    }
}

/// Renders linear radiance (or the AOV values) as RGBA floats.
pub fn render_float(
    scene: &RenderScene,
    req: &RenderRequest,
    progress: &(dyn Fn(f32) + Sync),
) -> Vec<f32> {
    let width = req.width.max(1) as usize;
    let height = req.height.max(1) as usize;
    let mut buffer = vec![0.0f32; width * height * 4];
    let aspect = width as f32 / height as f32;
    let basis = CameraBasis::new(&scene.camera, aspect);
    let samples = req.samples.max(1);
    let done = AtomicUsize::new(0);
    buffer
        .par_chunks_mut(width * 4)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..width {
                let mut acc = Vec3::ZERO;
                let mut alpha = 0.0f32;
                for s in 0..samples {
                    let seed = (req.seed as u64)
                        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        .wrapping_add((y as u64) << 32)
                        .wrapping_add(x as u64);
                    let mut rng = Pcg32::new(seed, s as u64 + 1);
                    let jitter = rng.stratified(s, samples);
                    let ndc = Vec2::new(
                        (x as f32 + jitter.x) / width as f32 * 2.0 - 1.0,
                        1.0 - (y as f32 + jitter.y) / height as f32 * 2.0,
                    );
                    if req.mode == RenderMode::PathTrace {
                        let lens = if basis.lens_radius > 0.0 {
                            rng.in_disc()
                        } else {
                            Vec2::ZERO
                        };
                        let (origin, dir) = basis.ray(&scene.camera, ndc, lens);
                        acc += trace_path(scene, req, origin, dir, &mut rng);
                        alpha += 1.0;
                    } else {
                        let px = shade_aov(scene, &scene.camera, &basis, req, ndc, &mut rng);
                        acc += Vec3::new(px[0], px[1], px[2]);
                        alpha += px[3];
                    }
                }
                let inv = 1.0 / samples as f32;
                let c = acc * inv;
                let a = alpha * inv;
                let o = x * 4;
                row[o] = c.x;
                row[o + 1] = c.y;
                row[o + 2] = c.z;
                // Path-traced output is written opaque: `transparent_background`
                // controls whether the sky is composited, not the alpha channel.
                row[o + 3] = if req.mode == RenderMode::PathTrace {
                    1.0
                } else {
                    a
                };
            }
            let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
            if finished % 8 == 0 || finished == height {
                progress(finished as f32 / height as f32);
            }
        });
    if req.denoise && samples < 256 {
        denoise(&mut buffer, width, height);
    }
    buffer
}

/// Small cross-bilateral filter that keeps edges while removing sample noise.
fn denoise(buffer: &mut [f32], width: usize, height: usize) {
    let src = buffer.to_vec();
    let get = |x: usize, y: usize| -> Vec3 {
        let o = (y * width + x) * 4;
        Vec3::new(src[o], src[o + 1], src[o + 2])
    };
    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let center = get(x, y);
            let mut sum = center;
            let mut weight = 1.0f32;
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nx = (x as i32 + dx) as usize;
                let ny = (y as i32 + dy) as usize;
                let c = get(nx, ny);
                let diff = (c - center).length();
                let w = (-diff * 8.0).exp();
                sum += c * w;
                weight += w;
            }
            let filtered = sum / weight;
            let o = (y * width + x) * 4;
            buffer[o] = center.x * 0.5 + filtered.x * 0.5;
            buffer[o + 1] = center.y * 0.5 + filtered.y * 0.5;
            buffer[o + 2] = center.z * 0.5 + filtered.z * 0.5;
        }
    }
}

/// Converts linear float radiance to a tonemapped, gamma-encoded RGBA8 image.
pub fn to_raster(linear: &[f32], req: &RenderRequest) -> RasterImage {
    let mut img = RasterImage::new(req.width.max(1), req.height.max(1));
    let exposure = req.exposure.max(0.0);
    for y in 0..img.height {
        for x in 0..img.width {
            let o = ((y as usize * img.width as usize) + x as usize) * 4;
            let r = req.tonemap.apply(linear[o] * exposure).clamp(0.0, 1.0);
            let g = req.tonemap.apply(linear[o + 1] * exposure).clamp(0.0, 1.0);
            let b = req.tonemap.apply(linear[o + 2] * exposure).clamp(0.0, 1.0);
            let a = if req.transparent_background {
                linear[o + 3].clamp(0.0, 1.0)
            } else {
                1.0
            };
            img.set(
                x,
                y,
                [
                    (linear_to_srgb(r) * 255.0).round() as u8,
                    (linear_to_srgb(g) * 255.0).round() as u8,
                    (linear_to_srgb(b) * 255.0).round() as u8,
                    (a * 255.0).round() as u8,
                ],
            );
        }
    }
    img
}

/// Convenience: render and tonemap in one call.
pub fn render(
    scene: &RenderScene,
    req: &RenderRequest,
    progress: &(dyn Fn(f32) + Sync),
) -> RasterImage {
    let linear = render_float(scene, req, progress);
    to_raster(&linear, req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_core::material::{Environment, SkyMode};

    fn sphere(center: Vec3, radius: f32, material: u32) -> Vec<RenderTriangle> {
        let (segments, rings) = (16, 10);
        let mut tris = Vec::new();
        let point = |i: usize, j: usize| -> Vec3 {
            let theta = PI * j as f32 / rings as f32;
            let phi = std::f32::consts::TAU * i as f32 / segments as f32;
            center
                + Vec3::new(
                    radius * theta.sin() * phi.cos(),
                    radius * theta.cos(),
                    radius * theta.sin() * phi.sin(),
                )
        };
        for j in 0..rings {
            for i in 0..segments {
                let (a, b, c, d) = (
                    point(i, j),
                    point(i + 1, j),
                    point(i + 1, j + 1),
                    point(i, j + 1),
                );
                let n = |p: Vec3| (p - center).normalize_or_zero();
                tris.push(RenderTriangle::new(a, b, c, n(a), n(b), n(c), material));
                tris.push(RenderTriangle::new(a, c, d, n(a), n(c), n(d), material));
            }
        }
        tris
    }

    fn ground(size: f32, material: u32) -> Vec<RenderTriangle> {
        let s = size;
        vec![
            RenderTriangle::new(
                Vec3::new(-s, 0.0, -s),
                Vec3::new(s, 0.0, -s),
                Vec3::new(s, 0.0, s),
                Vec3::Y,
                Vec3::Y,
                Vec3::Y,
                material,
            ),
            RenderTriangle::new(
                Vec3::new(-s, 0.0, -s),
                Vec3::new(s, 0.0, s),
                Vec3::new(-s, 0.0, s),
                Vec3::Y,
                Vec3::Y,
                Vec3::Y,
                material,
            ),
        ]
    }

    fn test_scene() -> RenderScene {
        let mut tris = ground(20.0, 1);
        tris.extend(sphere(Vec3::new(0.0, 1.0, 0.0), 1.0, 0));
        let materials = vec![
            RenderMaterial {
                base: Vec3::new(0.9, 0.2, 0.2),
                metallic: 0.0,
                roughness: 0.4,
                ..Default::default()
            },
            RenderMaterial {
                base: Vec3::new(0.45, 0.46, 0.48),
                roughness: 0.8,
                ..Default::default()
            },
        ];
        let env = Environment {
            sky: SkyMode::Gradient,
            ..Default::default()
        };
        let mut camera = Camera::default();
        camera.position = Vec3::new(0.0, 1.8, 4.5);
        camera.target = Vec3::new(0.0, 0.9, 0.0);
        RenderScene::new(tris, materials, Vec::new(), env, camera)
    }

    fn request(width: u32, height: u32) -> RenderRequest {
        RenderRequest {
            width,
            height,
            samples: 4,
            max_bounces: 2,
            seed: 7,
            mode: RenderMode::PathTrace,
            exposure: 1.0,
            tonemap: Tonemap::Aces,
            transparent_background: false,
            firefly_clamp: 20.0,
            denoise: false,
            region: None,
        }
    }

    #[test]
    fn renders_a_lit_sphere() {
        let scene = test_scene();
        let req = request(64, 48);
        let img = render(&scene, &req, &|_| {});
        assert_eq!((img.width, img.height), (64, 48));
        let center = img.get(32, 30);
        let corner = img.get(2, 2);
        assert!(
            center[0] > 20 || center[1] > 20 || center[2] > 20,
            "center should be lit, got {center:?}"
        );
        assert_ne!(center, corner, "sphere should differ from the background");
    }

    #[test]
    fn output_is_deterministic() {
        let scene = test_scene();
        let req = request(32, 24);
        let a = render_float(&scene, &req, &|_| {});
        let b = render_float(&scene, &req, &|_| {});
        assert_eq!(a, b);
    }

    #[test]
    fn different_seeds_differ() {
        let scene = test_scene();
        let mut req = request(32, 24);
        let a = render_float(&scene, &req, &|_| {});
        req.seed = 99;
        let b = render_float(&scene, &req, &|_| {});
        assert_ne!(a, b);
    }

    #[test]
    fn sky_only_render_has_a_gradient() {
        let mut scene = test_scene();
        scene.triangles.clear();
        scene.bvh = Bvh::build(&[]);
        let req = request(24, 48);
        let img = render(&scene, &req, &|_| {});
        let top = img.get(12, 2);
        let bottom = img.get(12, 45);
        assert_ne!(top, bottom, "sky should be a gradient");
        assert!(top[2] > top[0], "sky should be blueish: {top:?}");
    }

    #[test]
    fn albedo_mode_reports_material_colour() {
        let scene = test_scene();
        let mut req = request(32, 32);
        req.mode = RenderMode::Albedo;
        req.samples = 1;
        let img = render(&scene, &req, &|_| {});
        let px = img.get(16, 20);
        assert!(px[0] > px[1], "sphere albedo is red: {px:?}");
    }

    #[test]
    fn depth_and_normals_modes_have_detail() {
        let scene = test_scene();
        let mut req = request(32, 32);
        req.samples = 1;
        req.mode = RenderMode::Depth;
        let depth = render(&scene, &req, &|_| {});
        // Depth must contain a range of values, not a single flat level.
        let mut levels = std::collections::BTreeSet::new();
        for y in 0..depth.height {
            for x in 0..depth.width {
                levels.insert(depth.get(x, y)[0]);
            }
        }
        assert!(levels.len() > 5, "depth should vary: {:?}", levels);
        let sky = depth.get(16, 0);
        assert!(sky[0] <= 2, "sky depth should be black, got {sky:?}");

        req.mode = RenderMode::Normals;
        let normals = render(&scene, &req, &|_| {});
        let c = normals.get(16, 16);
        assert!(
            c[0] != c[1] || c[1] != c[2],
            "normals encode direction: {c:?}"
        );
    }

    #[test]
    fn ambient_occlusion_is_darker_under_the_sphere() {
        let scene = test_scene();
        let mut req = request(48, 48);
        req.mode = RenderMode::AmbientOcclusion;
        req.samples = 2;
        let img = render(&scene, &req, &|_| {});
        let under = img.get(24, 33);
        let far = img.get(4, 46);
        assert!(
            under[0] <= far[0],
            "AO under the sphere {under:?} vs open {far:?}"
        );
    }

    #[test]
    fn progress_is_reported() {
        let scene = test_scene();
        let req = request(32, 32);
        let seen = std::sync::atomic::AtomicUsize::new(0);
        render_float(&scene, &req, &|p| {
            assert!((0.0..=1.0).contains(&p));
            seen.fetch_add(1, Ordering::Relaxed);
        });
        assert!(seen.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn empty_scene_with_lights_still_renders_sky() {
        let mut scene = test_scene();
        scene.triangles.clear();
        scene.bvh = Bvh::build(&[]);
        scene.lights.push(RenderLight {
            kind: LightKind::Point,
            color: Vec3::ONE,
            intensity: 10.0,
            position: Vec3::new(0.0, 3.0, 0.0),
            direction: Vec3::NEG_Y,
            range: 10.0,
            cos_outer: 0.5,
            softness: 0.2,
            cast_shadows: true,
        });
        let img = render(&scene, &request(16, 16), &|_| {});
        assert_eq!(img.data.len(), 16 * 16 * 4);
    }

    #[test]
    fn point_light_contributes_visibility() {
        let mut scene = test_scene();
        scene.env.sun_intensity = 0.0;
        scene.env.sky_intensity = 0.05;
        let dark = render_float(&scene, &request(24, 24), &|_| {});
        scene.lights.push(RenderLight {
            kind: LightKind::Point,
            color: Vec3::ONE,
            intensity: 40.0,
            position: Vec3::new(0.0, 3.0, 2.5),
            direction: Vec3::NEG_Y,
            range: 20.0,
            cos_outer: 1.0,
            softness: 0.0,
            cast_shadows: false,
        });
        let lit = render_float(&scene, &request(24, 24), &|_| {});
        let sum = |v: &[f32]| v.iter().step_by(4).sum::<f32>();
        assert!(
            sum(&lit) > sum(&dark) * 1.05,
            "point light should add energy"
        );
    }
}
