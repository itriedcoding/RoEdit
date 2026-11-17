//! Primitive builders. Every primitive has outward-facing winding and planar or
//! parametric UVs so it can be textured immediately.

use crate::uv;
use crate::Vec3;
use roedit_core::mesh::{Face, Mesh};

/// Axis-aligned box centred on the origin.
pub fn cube(size: Vec3) -> Mesh {
    let h = size * 0.5;
    let mut m = Mesh::new("Cube");
    let p = [
        Vec3::new(-h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, -h.z),
        Vec3::new(h.x, h.y, -h.z),
        Vec3::new(-h.x, h.y, -h.z),
        Vec3::new(-h.x, -h.y, h.z),
        Vec3::new(h.x, -h.y, h.z),
        Vec3::new(h.x, h.y, h.z),
        Vec3::new(-h.x, h.y, h.z),
    ];
    for v in p {
        m.add_vertex(v);
    }
    // Outward winding for each side.
    for f in [
        vec![0, 3, 2, 1], // -Z
        vec![4, 5, 6, 7], // +Z
        vec![0, 4, 7, 3], // -X
        vec![1, 2, 6, 5], // +X
        vec![0, 1, 5, 4], // -Y
        vec![3, 7, 6, 2], // +Y
    ] {
        m.add_face(Face::new(f));
    }
    uv::box_uvs(&mut m, 1.0);
    m.recompute_normals();
    m
}

/// Subdivided flat grid on the XZ plane (Y up).
pub fn grid_plane(size: f32, divisions: u32) -> Mesh {
    let div = divisions.max(1);
    let mut m = Mesh::new("Plane");
    let step = size / div as f32;
    let half = size * 0.5;
    for z in 0..=div {
        for x in 0..=div {
            let px = -half + x as f32 * step;
            let pz = -half + z as f32 * step;
            m.positions.push(Vec3::new(px, 0.0, pz));
        }
    }
    let idx = |x: u32, z: u32| z * (div + 1) + x;
    for z in 0..div {
        for x in 0..div {
            let a = idx(x, z);
            let b = idx(x + 1, z);
            let c = idx(x + 1, z + 1);
            let d = idx(x, z + 1);
            let mut f = Face::new(vec![a, d, c, b]);
            f.uvs = vec![
                roedit_core::math::Vec2::new(x as f32 / div as f32, z as f32 / div as f32),
                roedit_core::math::Vec2::new(x as f32 / div as f32, (z + 1) as f32 / div as f32),
                roedit_core::math::Vec2::new(
                    (x + 1) as f32 / div as f32,
                    (z + 1) as f32 / div as f32,
                ),
                roedit_core::math::Vec2::new((x + 1) as f32 / div as f32, z as f32 / div as f32),
            ];
            m.add_face(f);
        }
    }
    m.recompute_normals();
    m
}

/// UV sphere with quads and triangle caps.
pub fn uv_sphere(radius: f32, segments: u32, rings: u32) -> Mesh {
    let segments = segments.max(3);
    let rings = rings.max(2);
    let mut m = Mesh::new("Sphere");
    for r in 0..=rings {
        let v = r as f32 / rings as f32;
        let phi = v * std::f32::consts::PI;
        for s in 0..=segments {
            let u = s as f32 / segments as f32;
            let theta = u * std::f32::consts::TAU;
            let x = phi.sin() * theta.cos();
            let y = phi.cos();
            let z = phi.sin() * theta.sin();
            m.positions.push(Vec3::new(x, y, z) * radius);
        }
    }
    let idx = |s: u32, r: u32| r * (segments + 1) + s;
    for r in 0..rings {
        for s in 0..segments {
            let a = idx(s, r);
            let b = idx(s + 1, r);
            let c = idx(s + 1, r + 1);
            let d = idx(s, r + 1);
            let mut f = if r == 0 {
                Face::new(vec![a, c, d])
            } else if r == rings - 1 {
                Face::new(vec![a, b, c])
            } else {
                Face::new(vec![a, b, c, d])
            };
            let uv_at = |s: u32, r: u32| {
                roedit_core::math::Vec2::new(
                    s as f32 / segments as f32,
                    1.0 - r as f32 / rings as f32,
                )
            };
            f.uvs = if f.verts.len() == 3 {
                if r == 0 {
                    vec![uv_at(s, r), uv_at(s + 1, r + 1), uv_at(s, r + 1)]
                } else {
                    vec![uv_at(s, r), uv_at(s + 1, r), uv_at(s + 1, r + 1)]
                }
            } else {
                vec![
                    uv_at(s, r),
                    uv_at(s + 1, r),
                    uv_at(s + 1, r + 1),
                    uv_at(s, r + 1),
                ]
            };
            m.add_face(f);
        }
    }
    // Weld the poles so the sphere is watertight.
    m.weld(1e-5);
    m.recompute_normals();
    m
}

/// Cylinder aligned to Y with optional caps.
pub fn cylinder(radius: f32, height: f32, segments: u32, caps: bool) -> Mesh {
    let segments = segments.max(3);
    let mut m = Mesh::new("Cylinder");
    let half = height * 0.5;
    for s in 0..segments {
        let a = s as f32 / segments as f32 * std::f32::consts::TAU;
        m.positions
            .push(Vec3::new(a.cos() * radius, -half, a.sin() * radius));
    }
    for s in 0..segments {
        let a = s as f32 / segments as f32 * std::f32::consts::TAU;
        m.positions
            .push(Vec3::new(a.cos() * radius, half, a.sin() * radius));
    }
    for s in 0..segments {
        let n = (s + 1) % segments;
        let u0 = s as f32 / segments as f32;
        let u1 = (s + 1) as f32 / segments as f32;
        let mut f = Face::new(vec![s, n, n + segments, s + segments]);
        f.uvs = vec![
            roedit_core::math::Vec2::new(u0, 0.0),
            roedit_core::math::Vec2::new(u1, 0.0),
            roedit_core::math::Vec2::new(u1, 1.0),
            roedit_core::math::Vec2::new(u0, 1.0),
        ];
        m.add_face(f);
    }
    if caps {
        let bottom: Vec<u32> = (0..segments).rev().collect();
        m.add_face(Face::new(bottom));
        let top: Vec<u32> = (0..segments).map(|s| s + segments).collect();
        m.add_face(Face::new(top));
    }
    m.recompute_normals();
    m
}

/// Cone aligned to Y (tip up).
pub fn cone(radius: f32, height: f32, segments: u32) -> Mesh {
    let segments = segments.max(3);
    let mut m = Mesh::new("Cone");
    let half = height * 0.5;
    for s in 0..segments {
        let a = s as f32 / segments as f32 * std::f32::consts::TAU;
        m.positions
            .push(Vec3::new(a.cos() * radius, -half, a.sin() * radius));
    }
    let tip = m.add_vertex(Vec3::new(0.0, half, 0.0));
    for s in 0..segments {
        let n = (s + 1) % segments;
        let mut f = Face::new(vec![s, n, tip]);
        f.uvs = vec![
            roedit_core::math::Vec2::new(s as f32 / segments as f32, 0.0),
            roedit_core::math::Vec2::new((s + 1) as f32 / segments as f32, 0.0),
            roedit_core::math::Vec2::new((s as f32 + 0.5) / segments as f32, 1.0),
        ];
        m.add_face(f);
    }
    let base: Vec<u32> = (0..segments).rev().collect();
    m.add_face(Face::new(base));
    m.recompute_normals();
    m
}

pub fn torus(major_radius: f32, minor_radius: f32, segments: u32, sides: u32) -> Mesh {
    let segments = segments.max(3);
    let sides = sides.max(3);
    let mut m = Mesh::new("Torus");
    for i in 0..segments {
        let u = i as f32 / segments as f32 * std::f32::consts::TAU;
        for j in 0..sides {
            let v = j as f32 / sides as f32 * std::f32::consts::TAU;
            let r = major_radius + minor_radius * v.cos();
            m.positions
                .push(Vec3::new(r * u.cos(), minor_radius * v.sin(), r * u.sin()));
        }
    }
    for i in 0..segments {
        for j in 0..sides {
            let i2 = (i + 1) % segments;
            let j2 = (j + 1) % sides;
            let a = i * sides + j;
            let b = i2 * sides + j;
            let c = i2 * sides + j2;
            let d = i * sides + j2;
            // Outward winding: the parameterisation sweeps u around Y and v
            // around the tube, so the quad has to be reversed.
            let mut f = Face::new(vec![a, d, c, b]);
            let uv_at = |i: u32, j: u32| {
                roedit_core::math::Vec2::new(i as f32 / segments as f32, j as f32 / sides as f32)
            };
            f.uvs = vec![uv_at(i, j), uv_at(i, j2), uv_at(i2, j2), uv_at(i2, j)];
            m.add_face(f);
        }
    }
    m.recompute_normals();
    m
}

pub fn capsule(radius: f32, height: f32, segments: u32, rings: u32) -> Mesh {
    let mut m = uv_sphere(radius, segments, rings.max(2));
    m.name = "Capsule".into();
    // Stretch the upper and lower hemispheres apart.
    let half = (height * 0.5).max(0.0);
    for p in &mut m.positions {
        let s = if p.y > 0.0 { 1.0 } else { 0.0 };
        p.y += (s - 0.5) * 2.0 * half;
    }
    m.recompute_normals();
    m
}

/// Disc in the XZ plane.
pub fn disc(radius: f32, segments: u32) -> Mesh {
    let segments = segments.max(3);
    let mut m = Mesh::new("Disc");
    m.add_vertex(Vec3::ZERO);
    for s in 0..segments {
        let a = s as f32 / segments as f32 * std::f32::consts::TAU;
        m.positions
            .push(Vec3::new(a.cos() * radius, 0.0, a.sin() * radius));
    }
    for s in 0..segments {
        let n = (s + 1) % segments;
        let mut f = Face::new(vec![0, n + 1, s + 1]);
        f.uvs = vec![
            roedit_core::math::Vec2::new(0.5, 0.5),
            roedit_core::math::Vec2::new(
                0.5 + 0.5 * ((s + 1) as f32 / segments as f32 * std::f32::consts::TAU).cos(),
                0.5 + 0.5 * ((s + 1) as f32 / segments as f32 * std::f32::consts::TAU).sin(),
            ),
            roedit_core::math::Vec2::new(
                0.5 + 0.5 * (s as f32 / segments as f32 * std::f32::consts::TAU).cos(),
                0.5 + 0.5 * (s as f32 / segments as f32 * std::f32::consts::TAU).sin(),
            ),
        ];
        m.add_face(f);
    }
    m.recompute_normals();
    m
}

/// Roblox Part shapes. Sizes are in studs (converted by the caller).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PartShape {
    Block,
    Ball,
    Cylinder,
    Wedge,
    CornerWedge,
}

impl PartShape {
    pub const ALL: [PartShape; 5] = [
        PartShape::Block,
        PartShape::Ball,
        PartShape::Cylinder,
        PartShape::Wedge,
        PartShape::CornerWedge,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PartShape::Block => "Block",
            PartShape::Ball => "Ball",
            PartShape::Cylinder => "Cylinder",
            PartShape::Wedge => "Wedge",
            PartShape::CornerWedge => "Corner Wedge",
        }
    }
}

/// Builds a Roblox-style part. `size` is in scene units; `detail` controls the
/// tessellation of round shapes.
pub fn roblox_part(shape: PartShape, size: Vec3, detail: u32) -> Mesh {
    match shape {
        PartShape::Block => cube(size),
        PartShape::Ball => uv_sphere(
            size.x.min(size.y).min(size.z) * 0.5,
            detail.max(8) * 2,
            detail.max(6),
        ),
        PartShape::Cylinder => cylinder(size.x * 0.5, size.y, detail.max(8) * 2, true),
        PartShape::Wedge => wedge(size),
        PartShape::CornerWedge => corner_wedge(size),
    }
}

/// Right triangular prism: flat bottom, sloped top going up towards +Z.
pub fn wedge(size: Vec3) -> Mesh {
    let h = size * 0.5;
    let mut m = Mesh::new("Wedge");
    let p = [
        Vec3::new(-h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, -h.z),
        Vec3::new(-h.x, -h.y, h.z),
        Vec3::new(h.x, -h.y, h.z),
        Vec3::new(-h.x, h.y, h.z),
        Vec3::new(h.x, h.y, h.z),
    ];
    for v in p {
        m.add_vertex(v);
    }
    // bottom, back, slope, left, right
    m.add_face(Face::new(vec![0, 1, 3, 2]));
    m.add_face(Face::new(vec![2, 3, 5, 4]));
    m.add_face(Face::new(vec![0, 4, 5, 1]));
    m.add_face(Face::new(vec![0, 2, 4]));
    m.add_face(Face::new(vec![1, 5, 3]));
    uv::box_uvs(&mut m, 1.0);
    m.recompute_normals();
    m
}

/// Corner wedge: a block with one corner cut away (pyramid-like).
pub fn corner_wedge(size: Vec3) -> Mesh {
    let h = size * 0.5;
    let mut m = Mesh::new("CornerWedge");
    let p = [
        Vec3::new(-h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, -h.z),
        Vec3::new(h.x, -h.y, h.z),
        Vec3::new(-h.x, -h.y, h.z),
        Vec3::new(-h.x, h.y, -h.z),
        Vec3::new(h.x, h.y, -h.z),
        Vec3::new(-h.x, h.y, h.z),
    ];
    for v in p {
        m.add_vertex(v);
    }
    m.add_face(Face::new(vec![0, 3, 2, 1])); // bottom
    m.add_face(Face::new(vec![0, 4, 5, 1])); // back
    m.add_face(Face::new(vec![0, 3, 6, 4])); // left
    m.add_face(Face::new(vec![1, 5, 2])); // right
    m.add_face(Face::new(vec![3, 2, 5, 6])); // slope
    m.add_face(Face::new(vec![4, 6, 5])); // top-back triangle
    uv::box_uvs(&mut m, 1.0);
    m.recompute_normals();
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{signed_volume, surface_area};

    fn assert_outward(m: &Mesh) {
        let centroid = m.bounds().center();
        for (i, f) in m.faces.iter().enumerate() {
            let c = f.centroid(&m.positions) - centroid;
            if c.length() < 1e-6 {
                continue;
            }
            assert!(
                f.normal(&m.positions).dot(c.normalize()) > 0.0,
                "{}: face {i} points inward",
                m.name
            );
        }
    }

    #[test]
    fn cube_has_six_quads() {
        let m = cube(Vec3::new(2.0, 2.0, 2.0));
        assert_eq!(m.faces.len(), 6);
        assert!(surface_area(&m) - 24.0 < 1e-4);
        assert_outward(&m);
        assert_eq!(m.bounds().size(), Vec3::splat(2.0));
    }

    #[test]
    fn sphere_is_watertight_and_uvd() {
        let m = uv_sphere(1.0, 16, 8);
        let v = m.validate();
        assert_eq!(v.out_of_range_indices, 0);
        assert_eq!(v.non_manifold_edges, 0);
        assert_eq!(v.boundary_edges, 0, "sphere should be closed");
        assert!(m.faces.iter().all(|f| f.uvs.len() == f.verts.len()));
        assert!(signed_volume(&m) > 3.0);
        assert_outward(&m);
    }

    #[test]
    fn cylinder_caps_are_closed() {
        let m = cylinder(0.5, 2.0, 12, true);
        assert_eq!(m.validate().boundary_edges, 0);
        let open = cylinder(0.5, 2.0, 12, false);
        assert_eq!(open.validate().boundary_edges, 24);
    }

    #[test]
    fn torus_volume_is_positive() {
        let m = torus(1.0, 0.3, 24, 12);
        assert!(signed_volume(&m) > 0.0);
        assert_eq!(m.faces.len(), 24 * 12);
    }

    #[test]
    fn plane_is_a_grid() {
        let m = grid_plane(4.0, 4);
        assert_eq!(m.faces.len(), 16);
        assert!(m.positions.iter().all(|p| p.y.abs() < 1e-6));
        assert!((m.bounds().size().x - 4.0).abs() < 1e-5);
    }

    #[test]
    fn roblox_parts_build() {
        for shape in PartShape::ALL {
            let m = roblox_part(shape, Vec3::new(2.0, 2.0, 1.0), 8);
            assert!(!m.positions.is_empty(), "{shape:?}");
            assert!(!m.faces.is_empty(), "{shape:?}");
            assert_eq!(m.validate().out_of_range_indices, 0);
        }
    }

    #[test]
    fn cone_and_disc() {
        let c = cone(1.0, 2.0, 12);
        assert_eq!(c.faces.len(), 13);
        let d = disc(1.0, 16);
        assert_eq!(d.faces.len(), 16);
    }

    #[test]
    fn capsule_is_taller_than_width() {
        let m = capsule(0.5, 2.0, 12, 6);
        let s = m.bounds().size();
        assert!(s.y > s.x);
        assert!(signed_volume(&m) > 0.0);
    }
}
