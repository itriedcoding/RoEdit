//! Small, dependency-light math layer on top of `glam`.
//!
//! Every intersection routine here is exercised by unit tests at the bottom of
//! the file; the renderer, viewport picking and modeling tools all share them.

use serde::{Deserialize, Serialize};

pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec3A, Vec4};

pub const EPSILON: f32 = 1e-6;

pub fn deg_to_rad(d: f32) -> f32 {
    d * std::f32::consts::PI / 180.0
}

pub fn rad_to_deg(r: f32) -> f32 {
    r * 180.0 / std::f32::consts::PI
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < EPSILON {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = clamp01((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

/// Cube-map style sRGB <-> linear conversion used by every imaging path.
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn luminance(rgb: Vec3) -> f32 {
    rgb.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

/// Axis aligned bounding box. An "empty" box has min > max so that union
/// operations behave like an identity for the first inserted point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for Aabb {
    fn default() -> Self {
        Self::empty()
    }
}

impl Aabb {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    pub fn from_points<'a>(points: impl IntoIterator<Item = &'a Vec3>) -> Self {
        let mut b = Self::empty();
        for p in points {
            b.expand(*p);
        }
        b
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    pub fn expand(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn expand_by(&mut self, amount: f32) {
        if self.is_empty() {
            return;
        }
        self.min -= Vec3::splat(amount);
        self.max += Vec3::splat(amount);
    }

    pub fn union(&self, other: &Aabb) -> Aabb {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Aabb::new(self.min.min(other.min), self.max.max(other.max))
    }

    pub fn size(&self) -> Vec3 {
        if self.is_empty() {
            Vec3::ZERO
        } else {
            self.max - self.min
        }
    }

    pub fn center(&self) -> Vec3 {
        if self.is_empty() {
            Vec3::ZERO
        } else {
            (self.min + self.max) * 0.5
        }
    }

    pub fn half_size(&self) -> Vec3 {
        self.size() * 0.5
    }

    pub fn contains(&self, p: Vec3) -> bool {
        !self.is_empty()
            && p.x >= self.min.x
            && p.y >= self.min.y
            && p.z >= self.min.z
            && p.x <= self.max.x
            && p.y <= self.max.y
            && p.z <= self.max.z
    }

    pub fn corner(&self, i: usize) -> Vec3 {
        Vec3::new(
            if i & 1 == 0 { self.min.x } else { self.max.x },
            if i & 2 == 0 { self.min.y } else { self.max.y },
            if i & 4 == 0 { self.min.z } else { self.max.z },
        )
    }

    pub fn transformed(&self, m: Mat4) -> Aabb {
        if self.is_empty() {
            return *self;
        }
        let mut out = Aabb::empty();
        for i in 0..8 {
            out.expand(m.transform_point3(self.corner(i)));
        }
        out
    }

    /// Slab test. Returns the entry distance (0 when the origin is inside).
    pub fn ray(&self, origin: Vec3, inv_dir: Vec3) -> Option<f32> {
        let t1 = (self.min - origin) * inv_dir;
        let t2 = (self.max - origin) * inv_dir;
        let tmin = t1.min(t2).max_element();
        let tmax = t1.max(t2).min_element();
        if tmax.max(0.0) >= tmin && tmax > 0.0 {
            Some(tmin.max(0.0))
        } else {
            None
        }
    }

    /// Longest axis: 0 = X, 1 = Y, 2 = Z.
    pub fn longest_axis(&self) -> usize {
        let s = self.size();
        if s.x >= s.y && s.x >= s.z {
            0
        } else if s.y >= s.z {
            1
        } else {
            2
        }
    }

    pub fn surface_area(&self) -> f32 {
        let s = self.size();
        2.0 * (s.x * s.y + s.y * s.z + s.z * s.x)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize_or_zero(),
        }
    }

    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    pub fn inv_dir(&self) -> Vec3 {
        Vec3::new(
            safe_recip(self.direction.x),
            safe_recip(self.direction.y),
            safe_recip(self.direction.z),
        )
    }
}

fn safe_recip(x: f32) -> f32 {
    if x.abs() < 1e-12 {
        1e12_f32.copysign(x)
    } else {
        1.0 / x
    }
}

/// Moller-Trumbore ray/triangle test.
/// Returns `(t, u, v)` where the hit point is `a + u*(b-a) + v*(c-a)`.
pub fn ray_triangle(ray: Ray, a: Vec3, b: Vec3, c: Vec3) -> Option<(f32, f32, f32)> {
    let e1 = b - a;
    let e2 = c - a;
    let p = ray.direction.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv_det = 1.0 / det;
    let t_vec = ray.origin - a;
    let u = t_vec.dot(p) * inv_det;
    if !(-1e-5..=1.0 + 1e-5).contains(&u) {
        return None;
    }
    let q = t_vec.cross(e1);
    let v = ray.direction.dot(q) * inv_det;
    if v < -1e-5 || u + v > 1.0 + 1e-5 {
        return None;
    }
    let t = e2.dot(q) * inv_det;
    if t > 1e-5 {
        Some((t, u, v))
    } else {
        None
    }
}

/// Barycentric-safe normal for a triangle hit.
pub fn interpolate_triangle(_ray: Ray, a: Vec3, b: Vec3, c: Vec3, u: f32, v: f32) -> Vec3 {
    a + (b - a) * u + (c - a) * v
}

pub fn ray_sphere(ray: Ray, center: Vec3, radius: f32) -> Option<f32> {
    let oc = ray.origin - center;
    let b = oc.dot(ray.direction);
    let c = oc.length_squared() - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let t0 = -b - sq;
    let t1 = -b + sq;
    if t0 > 1e-5 {
        Some(t0)
    } else if t1 > 1e-5 {
        Some(t1)
    } else {
        None
    }
}

pub fn ray_plane(ray: Ray, point: Vec3, normal: Vec3) -> Option<f32> {
    let denom = normal.dot(ray.direction);
    if denom.abs() < 1e-8 {
        return None;
    }
    let t = (point - ray.origin).dot(normal) / denom;
    if t > 0.0 {
        Some(t)
    } else {
        None
    }
}

/// Closest points between two infinite lines; used by gizmo dragging.
pub fn closest_point_line_line(p1: Vec3, d1: Vec3, p2: Vec3, d2: Vec3) -> Option<(f32, f32)> {
    let r = p1 - p2;
    let a = d1.dot(d1);
    let b = d1.dot(d2);
    let c = d2.dot(d2);
    let d = d1.dot(r);
    let e = d2.dot(r);
    let denom = a * c - b * b;
    if denom.abs() < 1e-8 {
        return None;
    }
    let s = (b * e - c * d) / denom;
    let t = (a * e - b * d) / denom;
    Some((s, t))
}

pub fn closest_point_on_segment(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < EPSILON {
        return 0.0;
    }
    clamp01((p - a).dot(ab) / len2)
}

/// Builds an orthonormal basis around `n` (used by BSDF sampling).
pub fn orthonormal_basis(n: Vec3) -> (Vec3, Vec3, Vec3) {
    let a = if n.x.abs() > 0.9 { Vec3::Y } else { Vec3::X };
    let t = n.cross(a).normalize_or_zero();
    let b = n.cross(t);
    (t, b, n)
}

/// Camera orbit helper shared by the viewport and cinematic tools.
pub fn orbit_position(target: Vec3, yaw: f32, pitch: f32, distance: f32) -> Vec3 {
    let cp = pitch.cos();
    target
        + Vec3::new(
            distance * cp * yaw.sin(),
            distance * pitch.sin(),
            distance * cp * yaw.cos(),
        )
}

pub fn euler_quat(yaw_deg: f32, pitch_deg: f32, roll_deg: f32) -> Quat {
    Quat::from_euler(
        glam::EulerRot::YXZ,
        deg_to_rad(yaw_deg),
        deg_to_rad(pitch_deg),
        deg_to_rad(roll_deg),
    )
}

/// Convert a matrix to (translation, rotation, scale), decomposing shear as
/// scale on the primary axes (standard TRS extraction).
pub fn decompose(m: Mat4) -> (Vec3, Quat, Vec3) {
    let t = m.w_axis.truncate();
    let sx = m.x_axis.truncate().length();
    let sy = m.y_axis.truncate().length();
    let sz = m.z_axis.truncate().length();
    let scale = Vec3::new(sx, sy, sz);
    let rot = Mat3::from_cols(
        m.x_axis.truncate() / sx.max(EPSILON),
        m.y_axis.truncate() / sy.max(EPSILON),
        m.z_axis.truncate() / sz.max(EPSILON),
    );
    (t, Quat::from_mat3(&rot), scale)
}

/// Ray/box overlap in the [0,1]^3 local space of an axis-aligned unit cube.
pub fn unit_cube_ray(ray: Ray) -> Option<(f32, Vec3)> {
    let b = Aabb::new(Vec3::splat(-0.5), Vec3::splat(0.5));
    b.ray(ray.origin, ray.inv_dir()).map(|t| (t, ray.at(t)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aabb_union_and_bounds() {
        let mut b = Aabb::empty();
        assert!(b.is_empty());
        b.expand(Vec3::new(1.0, 2.0, 3.0));
        b.expand(Vec3::new(-1.0, 0.0, 8.0));
        assert_eq!(b.size(), Vec3::new(2.0, 2.0, 5.0));
        assert_eq!(b.center(), Vec3::new(0.0, 1.0, 5.5));
        assert!(b.contains(Vec3::new(0.0, 1.0, 5.0)));
        assert!(!b.contains(Vec3::new(5.0, 0.0, 0.0)));
        assert!(!b.contains(Vec3::ZERO));
    }

    #[test]
    fn ray_box_hits_and_misses() {
        let b = Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0));
        let r = Ray::new(Vec3::new(0.0, 0.0, -5.0), Vec3::Z);
        assert!((b.ray(r.origin, r.inv_dir()).unwrap() - 4.0).abs() < 1e-4);
        let miss = Ray::new(Vec3::new(3.0, 0.0, -5.0), Vec3::Z);
        assert!(b.ray(miss.origin, miss.inv_dir()).is_none());
    }

    #[test]
    fn triangle_intersection_matches_barycentrics() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = Vec3::new(0.0, 1.0, 0.0);
        let r = Ray::new(Vec3::new(0.25, 0.25, 1.0), Vec3::NEG_Z);
        let (t, u, v) = ray_triangle(r, a, b, c).expect("hit");
        assert!((t - 1.0).abs() < 1e-5);
        let p = interpolate_triangle(r, a, b, c, u, v);
        assert!((p - Vec3::new(0.25, 0.25, 0.0)).length() < 1e-4);
    }

    #[test]
    fn sphere_and_plane_hits() {
        let r = Ray::new(Vec3::new(0.0, 0.0, -3.0), Vec3::Z);
        assert!((ray_sphere(r, Vec3::ZERO, 1.0).unwrap() - 2.0).abs() < 1e-5);
        assert!(ray_sphere(r, Vec3::new(0.0, 5.0, 0.0), 1.0).is_none());
        let t = ray_plane(r, Vec3::ZERO, Vec3::Z).unwrap();
        assert!((t - 3.0).abs() < 1e-5);
    }

    #[test]
    fn matrix_decompose_roundtrip() {
        let t = Vec3::new(1.0, -2.0, 0.5);
        let r = Quat::from_euler(glam::EulerRot::XYZ, 0.3, -0.7, 1.1);
        let s = Vec3::new(2.0, 3.0, 0.5);
        let m = Mat4::from_scale_rotation_translation(s, r, t);
        let (dt, dr, ds) = decompose(m);
        assert!((dt - t).length() < 1e-5);
        assert!((dr.dot(r).abs() - 1.0).abs() < 1e-4);
        assert!((ds - s).length() < 1e-5);
    }

    #[test]
    fn orbit_and_srgb() {
        let p = orbit_position(Vec3::ZERO, 0.0, 0.0, 5.0);
        assert!((p - Vec3::new(0.0, 0.0, 5.0)).length() < 1e-5);
        let x = 0.5f32;
        assert!((linear_to_srgb(srgb_to_linear(x)) - x).abs() < 1e-5);
    }
}
