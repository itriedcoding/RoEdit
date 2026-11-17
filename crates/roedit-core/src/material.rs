//! PBR material description and the scene environment (sky, sun, fog, tonemap).

use crate::ids::ImageId;
use crate::math::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tonemap {
    None,
    Reinhard,
    Aces,
    Filmic,
}

impl Tonemap {
    pub const ALL: [Tonemap; 4] = [
        Tonemap::None,
        Tonemap::Reinhard,
        Tonemap::Aces,
        Tonemap::Filmic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tonemap::None => "Linear",
            Tonemap::Reinhard => "Reinhard",
            Tonemap::Aces => "ACES",
            Tonemap::Filmic => "Filmic",
        }
    }

    pub fn apply(self, x: f32) -> f32 {
        match self {
            Tonemap::None => x,
            Tonemap::Reinhard => x / (1.0 + x),
            Tonemap::Aces => {
                let a = 2.51;
                let b = 0.03;
                let c = 2.43;
                let d = 0.59;
                let e = 0.14;
                ((x * (a * x + b)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
            }
            Tonemap::Filmic => {
                let x = (x - 0.004).max(0.0);
                (x * (6.2 * x + 0.5)) / (x * (6.2 * x + 1.7) + 0.06)
            }
        }
    }
}

/// Roblox/texture-friendly PBR inputs. Texture slots reference images stored in
/// the scene's image library.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub normal_strength: f32,
    pub emission: [f32; 3],
    pub emission_strength: f32,
    pub opacity: f32,
    pub ior: f32,
    pub uv_scale: [f32; 2],
    pub uv_offset: [f32; 2],
    pub albedo_map: Option<ImageId>,
    pub orm_map: Option<ImageId>,
    pub normal_map: Option<ImageId>,
    pub emission_map: Option<ImageId>,
    pub opacity_map: Option<ImageId>,
    /// Procedural texture generated from the material node graph, if any.
    pub procedural_albedo: Option<String>,
    pub double_sided: bool,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name: "Material".into(),
            base_color: [0.8, 0.8, 0.82],
            metallic: 0.0,
            roughness: 0.45,
            normal_strength: 1.0,
            emission: [0.0, 0.0, 0.0],
            emission_strength: 0.0,
            opacity: 1.0,
            ior: 1.5,
            uv_scale: [1.0, 1.0],
            uv_offset: [0.0, 0.0],
            albedo_map: None,
            orm_map: None,
            normal_map: None,
            emission_map: None,
            opacity_map: None,
            procedural_albedo: None,
            double_sided: false,
        }
    }
}

impl Material {
    pub fn new(name: impl Into<String>, color: [f32; 3], metallic: f32, roughness: f32) -> Self {
        Self {
            name: name.into(),
            base_color: color,
            metallic,
            roughness,
            ..Default::default()
        }
    }

    pub fn base_color_vec(&self) -> Vec3 {
        Vec3::from_array(self.base_color)
    }

    pub fn emissive(&self) -> Vec3 {
        Vec3::from_array(self.emission) * self.emission_strength
    }

    pub fn is_transparent(&self) -> bool {
        self.opacity < 0.999
    }

    /// Presets offered by the material library.
    pub fn presets() -> Vec<(&'static str, Material)> {
        vec![
            (
                "Roblox Plastic",
                Material::new("Roblox Plastic", [0.72, 0.18, 0.16], 0.0, 0.55),
            ),
            (
                "Smooth Plastic",
                Material::new("Smooth Plastic", [0.9, 0.9, 0.92], 0.0, 0.18),
            ),
            ("Neon", {
                let mut m = Material::new("Neon", [1.0, 0.25, 0.85], 0.0, 0.3);
                m.emission = [1.0, 0.25, 0.85];
                m.emission_strength = 4.0;
                m
            }),
            (
                "Brushed Metal",
                Material::new("Brushed Metal", [0.82, 0.83, 0.86], 1.0, 0.32),
            ),
            (
                "Chrome",
                Material::new("Chrome", [0.95, 0.96, 0.98], 1.0, 0.05),
            ),
            ("Gold", Material::new("Gold", [1.0, 0.77, 0.34], 1.0, 0.18)),
            (
                "Rubber",
                Material::new("Rubber", [0.06, 0.06, 0.07], 0.0, 0.9),
            ),
            ("Glass", {
                let mut m = Material::new("Glass", [0.85, 0.93, 0.95], 0.0, 0.02);
                m.opacity = 0.22;
                m.ior = 1.52;
                m
            }),
            ("Wood", Material::new("Wood", [0.45, 0.28, 0.14], 0.0, 0.62)),
            (
                "Concrete",
                Material::new("Concrete", [0.55, 0.55, 0.53], 0.0, 0.85),
            ),
            (
                "Carpet",
                Material::new("Carpet", [0.25, 0.3, 0.42], 0.0, 0.95),
            ),
            (
                "Grass",
                Material::new("Grass", [0.24, 0.45, 0.2], 0.0, 0.88),
            ),
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkyMode {
    Solid,
    Gradient,
    /// Gradient plus a soft sun disc, evaluated analytically.
    Physical,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Environment {
    pub sky: SkyMode,
    pub sky_top: [f32; 3],
    pub sky_horizon: [f32; 3],
    pub ground_color: [f32; 3],
    pub sky_intensity: f32,
    pub sun_direction: Vec3,
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
    pub sun_angle_deg: f32,
    pub ambient_intensity: f32,
    pub fog_density: f32,
    pub fog_color: [f32; 3],
    pub exposure: f32,
    pub tonemap: Tonemap,
    pub show_sky: bool,
    pub show_ground: bool,
    pub ground_height: f32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            sky: SkyMode::Gradient,
            sky_top: [0.09, 0.16, 0.33],
            sky_horizon: [0.55, 0.66, 0.82],
            ground_color: [0.16, 0.17, 0.19],
            sky_intensity: 1.0,
            sun_direction: Vec3::new(-0.45, 0.78, -0.42).normalize(),
            sun_color: [1.0, 0.95, 0.86],
            sun_intensity: 3.2,
            sun_angle_deg: 1.5,
            ambient_intensity: 0.35,
            fog_density: 0.0,
            fog_color: [0.6, 0.68, 0.8],
            exposure: 1.0,
            tonemap: Tonemap::Aces,
            show_sky: true,
            show_ground: true,
            ground_height: 0.0,
        }
    }
}

impl Environment {
    /// Radiance arriving from a direction (sky dome + analytic ground).
    pub fn sample_sky(&self, dir: Vec3) -> Vec3 {
        let up = dir.y.clamp(-1.0, 1.0);
        let t = (up * 0.5 + 0.5).clamp(0.0, 1.0);
        let mut base = if up >= 0.0 {
            let k = up.powf(0.55);
            Vec3::from_array(self.sky_horizon).lerp(Vec3::from_array(self.sky_top), k)
        } else {
            let k = (-up).powf(0.4);
            Vec3::from_array(self.sky_horizon).lerp(Vec3::from_array(self.ground_color), k)
        };
        if self.sky == SkyMode::Solid {
            base = Vec3::from_array(self.sky_top);
        }
        if self.sky == SkyMode::Physical {
            let cos_sun = dir.dot(self.sun_direction).clamp(-1.0, 1.0);
            let disc = crate::math::smoothstep(
                (1.0 - self.sun_angle_deg.to_radians().tan()).max(0.9),
                1.0,
                cos_sun,
            );
            base += Vec3::from_array(self.sun_color) * disc * self.sun_intensity * 2.0;
            let glow = cos_sun.max(0.0).powf(64.0) * 0.35;
            base += Vec3::from_array(self.sun_color) * glow;
        }
        base * self.sky_intensity * (0.35 + 0.65 * t.sqrt())
    }

    pub fn sun_radiance(&self) -> Vec3 {
        Vec3::from_array(self.sun_color) * self.sun_intensity
    }

    pub fn apply_fog(&self, color: Vec3, distance: f32) -> Vec3 {
        if self.fog_density <= 0.0 {
            return color;
        }
        let f = 1.0 - (-distance * self.fog_density).exp();
        color.lerp(Vec3::from_array(self.fog_color), f.clamp(0.0, 1.0))
    }

    /// RGB curve for the light-probe preview thumbnail.
    pub fn sky_gradient_image(&self, width: u32, height: u32) -> crate::raster::RasterImage {
        let mut img = crate::raster::RasterImage::new(width, height);
        for y in 0..height {
            let v = 1.0 - (y as f32 + 0.5) / height as f32;
            let dir = Vec3::new(0.0, v * 2.0 - 1.0, -1.0).normalize();
            let c = self.sample_sky(dir);
            let sky = Vec3::from_array([c.x, c.y, c.z]);
            for x in 0..width {
                let _ = x;
                let tone = Vec3::new(
                    self.tonemap.apply(sky.x * self.exposure),
                    self.tonemap.apply(sky.y * self.exposure),
                    self.tonemap.apply(sky.z * self.exposure),
                );
                let srgb = Vec3::new(
                    crate::math::linear_to_srgb(tone.x),
                    crate::math::linear_to_srgb(tone.y),
                    crate::math::linear_to_srgb(tone.z),
                );
                img.set_f32(x, y, [srgb.x, srgb.y, srgb.z, 1.0]);
            }
        }
        img
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tonemaps_stay_bounded() {
        for tm in Tonemap::ALL {
            let v = tm.apply(10.0);
            if tm != Tonemap::None {
                // "None" is intentionally linear and must be clipped by the
                // image writer, so only the compressed curves are bounded.
                assert!((0.0..=1.0).contains(&v), "{:?} -> {}", tm, v);
            }
            assert!(tm.apply(0.0) >= 0.0);
        }
    }

    #[test]
    fn sky_is_brighter_upwards() {
        let env = Environment::default();
        let up = env.sample_sky(Vec3::Y);
        let down = env.sample_sky(Vec3::NEG_Y);
        assert!(up.y > down.y);
    }

    #[test]
    fn sun_direction_is_normalised() {
        let env = Environment::default();
        assert!((env.sun_direction.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn presets_are_named() {
        for (name, m) in Material::presets() {
            assert_eq!(name, m.name);
            assert!((0.0..=1.0).contains(&m.roughness));
        }
    }
}
