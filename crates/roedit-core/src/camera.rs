//! Camera model shared by the viewport, the cinematic tools and the offline
//! renderer.

use crate::math::{deg_to_rad, Mat4, Ray, Vec2, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    /// Vertical field of view in degrees (perspective only).
    pub fov_y_deg: f32,
    pub near: f32,
    pub far: f32,
    pub orthographic: bool,
    /// Half height of the orthographic frustum in world units.
    pub ortho_scale: f32,
    pub focal_length_mm: f32,
    pub sensor_width_mm: f32,
    /// f-number used for depth of field. 0 disables lens simulation.
    pub fstop: f32,
    pub focus_distance: f32,
    pub roll_deg: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(4.0, 3.2, 5.6),
            target: Vec3::new(0.0, 0.9, 0.0),
            up: Vec3::Y,
            fov_y_deg: 45.0,
            near: 0.05,
            far: 500.0,
            orthographic: false,
            ortho_scale: 5.0,
            focal_length_mm: 50.0,
            sensor_width_mm: 36.0,
            fstop: 0.0,
            focus_distance: 5.0,
            roll_deg: 0.0,
        }
    }
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        (self.target - self.position).normalize_or_zero()
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(self.up).normalize_or_zero()
    }

    pub fn up_vector(&self) -> Vec3 {
        self.right().cross(self.forward()).normalize_or_zero()
    }

    /// View matrix, including camera roll.
    pub fn view(&self) -> Mat4 {
        let f = self.forward();
        let r = f.cross(self.up).normalize_or_zero();
        let u = r.cross(f).normalize_or_zero();
        let roll = deg_to_rad(self.roll_deg);
        let (s, c) = (roll.sin(), roll.cos());
        let r2 = r * c + u * s;
        let u2 = u * c - r * s;
        let camera_to_world = Mat4::from_cols(
            r2.extend(0.0),
            u2.extend(0.0),
            (-f).extend(0.0),
            self.position.extend(1.0),
        );
        camera_to_world.inverse()
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        let aspect = aspect.max(1e-4);
        if self.orthographic {
            let h = self.ortho_scale.max(1e-4);
            Mat4::orthographic_rh(-h * aspect, h * aspect, -h, h, self.near, self.far)
        } else {
            Mat4::perspective_rh(
                deg_to_rad(self.fov_y_deg.clamp(1.0, 179.0)),
                aspect,
                self.near,
                self.far,
            )
        }
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }

    /// Ray through normalised device coordinates (-1..1, y up).
    pub fn ray(&self, ndc: Vec2, aspect: f32) -> Ray {
        if self.orthographic {
            let h = self.ortho_scale.max(1e-4);
            let w = h * aspect.max(1e-4);
            let origin =
                self.position + self.right() * (ndc.x * w) + self.up_vector() * (ndc.y * h);
            Ray::new(origin, self.forward())
        } else {
            let tan = (deg_to_rad(self.fov_y_deg) * 0.5).tan();
            let dir = (self.forward()
                + self.right() * (ndc.x * tan * aspect.max(1e-4))
                + self.up_vector() * (ndc.y * tan))
                .normalize_or_zero();
            Ray::new(self.position, dir)
        }
    }

    /// Focal length that matches the current vertical FOV.
    pub fn focal_from_fov(&self) -> f32 {
        let fov = deg_to_rad(self.fov_y_deg.clamp(1.0, 179.0));
        self.sensor_width_mm / (2.0 * (fov * 0.5).tan())
    }

    /// Update the vertical FOV from a focal length (36mm film back).
    pub fn set_focal(&mut self, focal_mm: f32) {
        let f = focal_mm.max(1.0);
        self.focal_length_mm = f;
        self.fov_y_deg = crate::math::rad_to_deg(2.0 * (self.sensor_width_mm / (2.0 * f)).atan());
    }

    /// World-space aperture radius used for thin-lens depth of field.
    pub fn lens_radius(&self) -> f32 {
        if self.fstop <= 0.01 {
            return 0.0;
        }
        (self.focal_length_mm / 1000.0) / (2.0 * self.fstop)
    }

    pub fn has_dof(&self) -> bool {
        self.fstop > 0.01 && !self.orthographic
    }

    pub fn orbit(&mut self, target: Vec3, yaw: f32, pitch: f32, distance: f32) {
        self.target = target;
        self.position = crate::math::orbit_position(target, yaw, pitch, distance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_rays_match_projection() {
        let cam = Camera::default();
        let aspect = 16.0 / 9.0;
        let ray = cam.ray(Vec2::ZERO, aspect);
        assert!(ray.direction.dot(cam.forward()) > 0.999);

        let vp = cam.view_projection(aspect);
        let p = vp.transform_point3(cam.position + cam.forward() * 3.0);
        // project a point straight ahead: ndc should be near zero
        assert!(p.x.abs() < 1e-3 && p.y.abs() < 1e-3);
    }

    #[test]
    fn focal_roundtrip() {
        let mut cam = Camera::default();
        cam.set_focal(85.0);
        let f = cam.focal_from_fov();
        assert!((f - 85.0).abs() < 0.5);
    }

    #[test]
    fn ortho_projection_is_finite() {
        let cam = Camera {
            orthographic: true,
            ..Default::default()
        };
        let m = cam.projection(1.5);
        assert!(m.to_cols_array().iter().all(|v| v.is_finite()));
    }

    #[test]
    fn dof_toggle() {
        let mut cam = Camera::default();
        assert!(!cam.has_dof());
        cam.fstop = 2.8;
        assert!(cam.has_dof());
        assert!(cam.lens_radius() > 0.0);
    }
}
