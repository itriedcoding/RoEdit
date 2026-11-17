//! UV projection, per-face unwrapping with shelf packing, and UV statistics.
//!
//! Packing is a deterministic shelf algorithm: islands are sorted by height and
//! placed left-to-right into shelves, retrying at smaller scales until they fit
//! the unit square. Relative texel density is preserved because islands are
//! normalised by a single global dimension before packing.

use crate::{Axis, Vec2, Vec3};
use roedit_core::mesh::Mesh;

/// Orthonormal tangent basis for a normal.
pub fn basis_for_normal(n: Vec3) -> (Vec3, Vec3) {
    let n = n.normalize_or_zero();
    let reference = if n.y.abs() > 0.9 { Vec3::Z } else { Vec3::Y };
    let t = n.cross(reference).normalize_or_zero();
    let b = n.cross(t).normalize_or_zero();
    (t, b)
}

/// Projects every face onto its own plane (box unwrap).
pub fn box_uvs(mesh: &mut Mesh, scale: f32) {
    let positions = mesh.positions.clone();
    for f in &mut mesh.faces {
        let n = f.normal(&positions);
        let (t, b) = basis_for_normal(n);
        let origin = positions[f.verts[0] as usize];
        f.uvs = f
            .verts
            .iter()
            .map(|v| {
                let rel = positions[*v as usize] - origin;
                Vec2::new(rel.dot(t) * scale, rel.dot(b) * scale)
            })
            .collect();
    }
}

/// Flat projection along an axis.
pub fn planar_uvs(mesh: &mut Mesh, axis: Axis, scale: f32, offset: Vec2) {
    let (u_axis, v_axis) = match axis {
        Axis::X => (Axis::Z.index(), Axis::Y.index()),
        Axis::Y => (Axis::X.index(), Axis::Z.index()),
        Axis::Z => (Axis::X.index(), Axis::Y.index()),
    };
    for f in &mut mesh.faces {
        f.uvs = f
            .verts
            .iter()
            .map(|v| {
                let p = mesh.positions[*v as usize];
                Vec2::new(p[u_axis] * scale, p[v_axis] * scale) + offset
            })
            .collect();
    }
}

/// Wraps UVs around the Y axis of the mesh bounds.
pub fn cylindrical_uvs(mesh: &mut Mesh) {
    let b = mesh.bounds();
    let center = b.center();
    let height = b.size().y.max(1e-5);
    for f in &mut mesh.faces {
        f.uvs = f
            .verts
            .iter()
            .map(|v| {
                let p = mesh.positions[*v as usize];
                let angle = (p.z - center.z).atan2(p.x - center.x);
                Vec2::new(
                    angle / std::f32::consts::TAU + 0.5,
                    (p.y - (center.y - height * 0.5)) / height,
                )
            })
            .collect();
    }
}

/// Spherical projection around the mesh bounds centre.
pub fn spherical_uvs(mesh: &mut Mesh) {
    let center = mesh.bounds().center();
    for f in &mut mesh.faces {
        f.uvs = f
            .verts
            .iter()
            .map(|v| {
                let d = (mesh.positions[*v as usize] - center).normalize_or_zero();
                let u = d.z.atan2(d.x) / std::f32::consts::TAU + 0.5;
                let v = d.y.clamp(-1.0, 1.0).acos() / std::f32::consts::PI;
                Vec2::new(u, 1.0 - v)
            })
            .collect();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UnwrapReport {
    pub islands: usize,
    pub packed_scale: f32,
    pub skipped_faces: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UvStats {
    pub faces_with_uvs: usize,
    pub coverage: f32,
    pub out_of_bounds: usize,
    pub degenerate_uv_faces: usize,
}

struct Island {
    face: usize,
    origin: Vec3,
    tangent: Vec3,
    bitangent: Vec3,
    min: Vec2,
    size: Vec2,
}

/// Unwraps the selected faces into islands and packs them into 0..1.
/// Tiny or degenerate faces are skipped and reported.
pub fn unwrap_faces(mesh: &mut Mesh, faces: &[usize], margin: f32) -> UnwrapReport {
    let margin = margin.clamp(0.0, 0.1);
    let mut islands: Vec<Island> = Vec::new();
    let mut skipped = 0;
    for &fi in faces {
        let Some(f) = mesh.faces.get(fi) else {
            skipped += 1;
            continue;
        };
        let n = f.normal(&mesh.positions);
        if n.length_squared() < 1e-8 || f.verts.len() < 3 {
            skipped += 1;
            continue;
        }
        let (t, b) = basis_for_normal(n);
        let origin = f
            .verts
            .iter()
            .map(|v| mesh.positions[*v as usize])
            .fold(Vec3::splat(f32::INFINITY), |a, p| a.min(p));
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for &v in &f.verts {
            let rel = mesh.positions[v as usize] - origin;
            let uv = Vec2::new(rel.dot(t), rel.dot(b));
            min = min.min(uv);
            max = max.max(uv);
        }
        let size = (max - min).max(Vec2::ZERO);
        if size.x.max(size.y) < 1e-6 {
            skipped += 1;
            continue;
        }
        islands.push(Island {
            face: fi,
            origin,
            tangent: t,
            bitangent: b,
            min,
            size,
        });
    }
    if islands.is_empty() {
        return UnwrapReport {
            islands: 0,
            packed_scale: 1.0,
            skipped_faces: skipped,
        };
    }
    let max_dim = islands
        .iter()
        .map(|i| i.size.x.max(i.size.y))
        .fold(0.0_f32, f32::max)
        .max(1e-5);
    let normalized: Vec<Vec2> = islands.iter().map(|i| i.size / max_dim).collect();
    let (offsets, scale) = pack_shelves(&normalized, margin).unwrap_or_else(|| {
        // Extremely fragmented input: fall back to a uniform grid so the result
        // is still usable even if it overlaps.
        (
            (0..normalized.len())
                .map(|i| Vec2::new((i % 8) as f32 * 0.125, (i / 8 % 8) as f32 * 0.125))
                .collect(),
            0.12,
        )
    });
    for (island, offset) in islands.iter().zip(offsets.iter()) {
        let f = &mut mesh.faces[island.face];
        let new_uvs: Vec<Vec2> = f
            .verts
            .iter()
            .map(|v| {
                let rel = mesh.positions[*v as usize] - island.origin;
                let local = Vec2::new(rel.dot(island.tangent), rel.dot(island.bitangent));
                (local - island.min) / max_dim * scale + *offset
            })
            .collect();
        f.uvs = new_uvs;
    }
    UnwrapReport {
        islands: islands.len(),
        packed_scale: scale,
        skipped_faces: skipped,
    }
}

/// Shelves-based packer. Returns per-island offsets and the scale used.
pub fn pack_shelves(sizes: &[Vec2], margin: f32) -> Option<(Vec<Vec2>, f32)> {
    if sizes.is_empty() {
        return Some((Vec::new(), 1.0));
    }
    for step in 0..=32 {
        let scale = 1.0 / (1.0 + step as f32 * 0.15);
        if let Some(offsets) = try_shelf(sizes, margin, scale) {
            return Some((offsets, scale));
        }
    }
    None
}

fn try_shelf(sizes: &[Vec2], margin: f32, scale: f32) -> Option<Vec<Vec2>> {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|a, b| {
        sizes[*b]
            .y
            .total_cmp(&sizes[*a].y)
            .then(sizes[*b].x.total_cmp(&sizes[*a].x))
    });
    let mut offsets = vec![Vec2::ZERO; sizes.len()];
    let mut x = margin;
    let mut y = margin;
    let mut shelf_height = 0.0f32;
    for i in order {
        let w = sizes[i].x * scale;
        let h = sizes[i].y * scale;
        if x + w + margin > 1.0 + 1e-6 {
            x = margin;
            y += shelf_height + margin;
            shelf_height = 0.0;
        }
        if y + h + margin > 1.0 + 1e-6 {
            return None;
        }
        offsets[i] = Vec2::new(x, y);
        x += w + margin;
        shelf_height = shelf_height.max(h);
    }
    Some(offsets)
}

pub fn uv_stats(mesh: &Mesh) -> UvStats {
    let mut stats = UvStats::default();
    let mut coverage = 0.0;
    for f in &mesh.faces {
        if f.uvs.len() != f.verts.len() || f.verts.len() < 3 {
            continue;
        }
        stats.faces_with_uvs += 1;
        let mut out = false;
        for uv in &f.uvs {
            if uv.x < -1e-4 || uv.x > 1.0 + 1e-4 || uv.y < -1e-4 || uv.y > 1.0 + 1e-4 {
                out = true;
            }
        }
        if out {
            stats.out_of_bounds += 1;
        }
        let mut area = 0.0;
        for i in 1..f.uvs.len() - 1 {
            let a = f.uvs[0];
            let b = f.uvs[i];
            let c = f.uvs[i + 1];
            area += ((b - a).perp_dot(c - a)).abs() * 0.5;
        }
        if area < 1e-10 {
            stats.degenerate_uv_faces += 1;
        }
        coverage += area;
    }
    stats.coverage = coverage;
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn basis_is_orthonormal() {
        for n in [Vec3::Y, Vec3::X, Vec3::new(0.3, 0.8, -0.5)] {
            let (t, b) = basis_for_normal(n);
            assert!(t.dot(n.normalize()).abs() < 1e-5);
            assert!(b.dot(n.normalize()).abs() < 1e-5);
            assert!(t.dot(b).abs() < 1e-5);
            assert!((t.length() - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn cube_box_uvs_are_finite() {
        let mut m = primitives::cube(Vec3::ONE);
        box_uvs(&mut m, 1.0);
        assert!(m.faces.iter().all(|f| f.uvs.iter().all(|u| u.is_finite())));
        let stats = uv_stats(&m);
        assert_eq!(stats.faces_with_uvs, 6);
        assert!(stats.coverage > 0.0);
    }

    #[test]
    fn unwrap_packs_inside_unit_square() {
        let mut m = primitives::cube(Vec3::new(2.0, 1.0, 3.0));
        let faces: Vec<usize> = (0..m.faces.len()).collect();
        let report = unwrap_faces(&mut m, &faces, 0.01);
        assert_eq!(report.islands, 6);
        assert!(report.packed_scale > 0.0 && report.packed_scale <= 1.0);
        let stats = uv_stats(&m);
        assert_eq!(stats.out_of_bounds, 0);
        assert!(stats.coverage > 0.0 && stats.coverage <= 1.0);
        assert_eq!(stats.degenerate_uv_faces, 0);
    }

    #[test]
    fn shelf_packer_fits_many_islands() {
        let sizes = vec![Vec2::new(1.0, 1.0); 9];
        let (offsets, scale) = pack_shelves(&sizes, 0.01).expect("packed");
        assert_eq!(offsets.len(), 9);
        assert!(scale < 1.0);
        for (o, s) in offsets.iter().zip(sizes.iter()) {
            assert!(o.x >= 0.0 && o.y >= 0.0);
            assert!(o.x + s.x * scale <= 1.001);
            assert!(o.y + s.y * scale <= 1.001);
        }
    }

    #[test]
    fn planar_projection_uses_requested_axis() {
        let mut m = primitives::grid_plane(2.0, 1);
        planar_uvs(&mut m, Axis::Y, 0.5, Vec2::ZERO);
        let f = &m.faces[0];
        assert!(f.uvs.iter().all(|uv| uv.is_finite()));
        let b = m.bounds();
        assert!(f.uvs.iter().all(|uv| uv.x >= b.min.x * 0.5 - 1e-4));
    }

    #[test]
    fn cylindrical_and_spherical_are_bounded() {
        let mut m = primitives::uv_sphere(1.0, 12, 6);
        cylindrical_uvs(&mut m);
        assert!(m.faces.iter().all(|f| f
            .uvs
            .iter()
            .all(|uv| uv.x >= -1e-4 && uv.x <= 1.0001 && uv.y >= -1e-4)));
        spherical_uvs(&mut m);
        assert!(m
            .faces
            .iter()
            .all(|f| f.uvs.iter().all(|uv| uv.is_finite())));
    }
}
