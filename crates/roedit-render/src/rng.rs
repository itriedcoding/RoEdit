//! Deterministic PCG32 random numbers for the path tracer.
//!
//! Each pixel/sample pair gets its own stream, so the result does not depend on
//! thread count or scheduling.

use glam::{Vec2, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut r = Self {
            state: 0,
            inc: (stream << 1) | 1,
        };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(6364136223846793005).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Stratified sample in the unit square given a sample index.
    pub fn stratified(&mut self, index: u32, total: u32) -> Vec2 {
        let n = (total as f32).sqrt().ceil().max(1.0) as u32;
        let x = index % n;
        let y = index / n;
        Vec2::new(
            (x as f32 + self.f32()) / n as f32,
            (y as f32 + self.f32()) / n as f32,
        )
    }

    pub fn in_disc(&mut self) -> Vec2 {
        let r = self.f32().sqrt();
        let theta = self.f32() * std::f32::consts::TAU;
        Vec2::new(r * theta.cos(), r * theta.sin())
    }

    pub fn in_sphere(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let r = (1.0 - z * z).max(0.0).sqrt();
        let phi = self.f32() * std::f32::consts::TAU;
        Vec3::new(r * phi.cos(), r * phi.sin(), z)
    }

    pub fn on_hemisphere(&mut self) -> Vec3 {
        let v = self.in_sphere();
        Vec3::new(v.x, v.y.abs(), v.z)
    }
}

/// Builds an orthonormal basis around `n`.
pub fn basis(n: Vec3) -> (Vec3, Vec3) {
    let helper = if n.x.abs() > 0.9 { Vec3::Y } else { Vec3::X };
    let t = helper.cross(n).normalize_or_zero();
    let b = n.cross(t);
    (t, b)
}

pub fn hemisphere_cosine(rng: &mut Pcg32, n: Vec3) -> Vec3 {
    let d = rng.in_disc();
    let z = (1.0 - d.length_squared()).max(0.0).sqrt();
    let (t, b) = basis(n);
    (t * d.x + b * d.y + n * z).normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_the_same_seed() {
        let mut a = Pcg32::new(42, 7);
        let mut b = Pcg32::new(42, 7);
        for _ in 0..64 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_streams_differ() {
        let mut a = Pcg32::new(1, 1);
        let mut b = Pcg32::new(1, 2);
        assert_ne!(a.next_u32(), b.next_u32());
    }

    #[test]
    fn floats_stay_in_range() {
        let mut r = Pcg32::new(9, 3);
        for _ in 0..1000 {
            let v = r.f32();
            assert!((0.0..1.0).contains(&v));
        }
        let d = r.in_disc();
        assert!(d.length() <= 1.0001);
        let h = hemisphere_cosine(&mut r, Vec3::Y);
        assert!(h.y >= -1e-4 && (h.length() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn stratified_samples_cover_the_square() {
        let mut r = Pcg32::new(3, 4);
        let mut sum = Vec2::ZERO;
        for i in 0..16 {
            sum += r.stratified(i, 16);
        }
        let mean = sum / 16.0;
        assert!((mean.x - 0.5).abs() < 0.1);
        assert!((mean.y - 0.5).abs() < 0.1);
    }
}
