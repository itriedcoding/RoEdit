//! Vertex sculpting brushes. Each dab is applied to every vertex inside the
//! brush radius with a smooth falloff.

use crate::{Mesh, Vec3};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SculptBrush {
    Draw,
    Inflate,
    Smooth,
    Flatten,
    Pinch,
    Crease,
}

impl SculptBrush {
    pub const ALL: [SculptBrush; 6] = [
        SculptBrush::Draw,
        SculptBrush::Inflate,
        SculptBrush::Smooth,
        SculptBrush::Flatten,
        SculptBrush::Pinch,
        SculptBrush::Crease,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SculptBrush::Draw => "Draw",
            SculptBrush::Inflate => "Inflate",
            SculptBrush::Smooth => "Smooth",
            SculptBrush::Flatten => "Flatten",
            SculptBrush::Pinch => "Pinch",
            SculptBrush::Crease => "Crease",
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct BrushSettings {
    pub radius: f32,
    /// Displacement per dab as a fraction of the radius.
    pub strength: f32,
    /// 0 = hard edge, 1 = fully smooth falloff.
    pub softness: f32,
    pub invert: bool,
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self {
            radius: 0.5,
            strength: 0.15,
            softness: 0.8,
            invert: false,
        }
    }
}

fn falloff(distance: f32, radius: f32, softness: f32) -> f32 {
    if radius <= 1e-6 || distance > radius {
        return 0.0;
    }
    let t = (1.0 - distance / radius).clamp(0.0, 1.0);
    let hard = t;
    let smooth = t * t * (3.0 - 2.0 * t);
    let k = softness.clamp(0.0, 1.0);
    hard + (smooth - hard) * k
}

fn adjacency(mesh: &Mesh) -> HashMap<u32, Vec<u32>> {
    let mut map: HashMap<u32, Vec<u32>> = HashMap::new();
    for f in &mesh.faces {
        for i in 0..f.verts.len() {
            let a = f.verts[i];
            let b = f.verts[(i + 1) % f.verts.len()];
            map.entry(a).or_default().push(b);
            map.entry(b).or_default().push(a);
        }
    }
    map
}

/// Applies one brush dab. Returns how many vertices were affected.
pub fn sculpt(
    mesh: &mut Mesh,
    brush: SculptBrush,
    center: Vec3,
    settings: &BrushSettings,
) -> usize {
    if mesh.positions.is_empty() {
        return 0;
    }
    mesh.recompute_normals();
    let radius = settings.radius.max(1e-4);
    let sign = if settings.invert { -1.0 } else { 1.0 };
    let amount = settings.strength * radius * sign;

    let mut affected: Vec<(usize, f32)> = Vec::new();
    for (i, p) in mesh.positions.iter().enumerate() {
        let w = falloff((*p - center).length(), radius, settings.softness);
        if w > 0.0 {
            affected.push((i, w));
        }
    }
    if affected.is_empty() {
        return 0;
    }

    // Average normal of the affected region, used by Draw/Flatten/Crease.
    let mut region_normal = Vec3::ZERO;
    for (i, w) in &affected {
        region_normal += mesh.normals.get(*i).copied().unwrap_or(Vec3::Y) * *w;
    }
    let region_normal = region_normal.normalize_or_zero();
    let region_normal = if region_normal.length_squared() < 0.5 {
        Vec3::Y
    } else {
        region_normal
    };
    let plane_point = center;

    let neighbor_map = if brush == SculptBrush::Smooth {
        Some(adjacency(mesh))
    } else {
        None
    };

    let mut new_positions: Vec<(usize, Vec3)> = Vec::with_capacity(affected.len());
    for (i, w) in &affected {
        let p = mesh.positions[*i];
        let n = mesh.normals.get(*i).copied().unwrap_or(region_normal);
        let delta = match brush {
            SculptBrush::Draw => region_normal * amount * w,
            SculptBrush::Inflate => n * amount * w,
            SculptBrush::Smooth => {
                let map = neighbor_map.as_ref().unwrap();
                let Some(neighbors) = map.get(&(*i as u32)) else {
                    continue;
                };
                if neighbors.is_empty() {
                    continue;
                }
                let mut avg = Vec3::ZERO;
                for nb in neighbors {
                    avg += mesh.positions[*nb as usize];
                }
                avg /= neighbors.len() as f32;
                (avg - p) * (settings.strength * w * 3.0).min(0.9)
            }
            SculptBrush::Flatten => {
                let dist = (p - plane_point).dot(region_normal);
                -region_normal * dist * (settings.strength * w * 3.0).min(0.9)
            }
            SculptBrush::Pinch => {
                let mut lateral = center - p;
                lateral -= region_normal * lateral.dot(region_normal);
                lateral.normalize_or_zero() * amount * w
            }
            SculptBrush::Crease => {
                let mut lateral = center - p;
                lateral -= region_normal * lateral.dot(region_normal);
                let dist = (p - plane_point).dot(region_normal);
                (lateral.normalize_or_zero() * 0.5 - region_normal * dist.signum() * 0.5)
                    * amount
                    * w
            }
        };
        if delta.is_finite() {
            new_positions.push((*i, p + delta));
        }
    }
    let touched = new_positions.len();
    for (i, p) in new_positions {
        mesh.positions[i] = p;
    }
    mesh.recompute_normals();
    touched
}

/// Generates a sparse hair-cap style surface used by the hair modeling tool.
pub fn hair_cap(mesh: &Mesh, thickness: f32) -> Mesh {
    let mut cap = Mesh::new("Hair Cap");
    let positions = mesh.positions.clone();
    cap.positions = positions.clone();
    // Keep faces whose centroid is above the mesh centre.
    let center = mesh.bounds().center();
    for f in &mesh.faces {
        let c = f.centroid(&positions);
        if c.y >= center.y {
            cap.add_face(f.clone());
        }
    }
    cap.remove_unused_vertices();
    if thickness > 0.0 {
        let _ = crate::edit::solidify(&mut cap, thickness);
    }
    cap.recompute_normals();
    cap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn draw_moves_vertices_outward() {
        let mut mesh = primitives::uv_sphere(1.0, 16, 8);
        let before = mesh.bounds().size().y;
        let settings = BrushSettings {
            radius: 0.6,
            strength: 0.5,
            softness: 0.5,
            invert: false,
        };
        let touched = sculpt(&mut mesh, SculptBrush::Draw, Vec3::Y, &settings);
        assert!(touched > 0);
        assert!(mesh.bounds().size().y > before);
        assert_eq!(mesh.positions.len(), before_count(16, 8));
    }

    fn before_count(segments: u32, rings: u32) -> usize {
        // Pole welding merges the duplicated seam rows; just ensure the sphere
        // kept a plausible vertex count after sculpting.
        let _ = (segments, rings);
        // Build a fresh sphere to compare.
        primitives::uv_sphere(1.0, segments, rings).positions.len()
    }

    #[test]
    fn invert_moves_inward() {
        let mut mesh = primitives::uv_sphere(1.0, 16, 8);
        let outside = mesh.bounds().max.y;
        let settings = BrushSettings {
            radius: 0.5,
            strength: 0.4,
            softness: 0.5,
            invert: true,
        };
        sculpt(&mut mesh, SculptBrush::Inflate, Vec3::Y, &settings);
        assert!(mesh.bounds().max.y < outside);
    }

    #[test]
    fn smooth_reduces_perturbation() {
        let mut mesh = primitives::uv_sphere(1.0, 20, 10);
        for p in &mut mesh.positions {
            p.x += 0.2 * (p.y * 9.0).sin();
        }
        let rough = mesh.bounds().size().length();
        let settings = BrushSettings {
            radius: 5.0,
            strength: 0.5,
            softness: 1.0,
            invert: false,
        };
        sculpt(&mut mesh, SculptBrush::Smooth, Vec3::ZERO, &settings);
        let smooth = mesh.bounds().size().length();
        assert!(smooth <= rough + 1e-4);
        assert!(mesh.positions.iter().all(|p| p.is_finite()));
    }

    #[test]
    fn flatten_pushes_to_plane() {
        let mut mesh = primitives::uv_sphere(1.0, 16, 8);
        let settings = BrushSettings {
            radius: 2.0,
            strength: 0.8,
            softness: 0.2,
            invert: false,
        };
        sculpt(&mut mesh, SculptBrush::Flatten, Vec3::ZERO, &settings);
        let max_y = mesh.bounds().max.y;
        assert!(max_y < 1.0);
    }

    #[test]
    fn hair_cap_keeps_top_only() {
        let mesh = primitives::uv_sphere(1.0, 16, 8);
        let cap = hair_cap(&mesh, 0.0);
        assert!(!cap.faces.is_empty());
        assert!(cap.faces.len() < mesh.faces.len());
        let center = cap.bounds().center();
        assert!(center.y > 0.0);
    }

    #[test]
    fn falloff_is_monotonic() {
        let mut last = 1.0;
        for i in 0..=10 {
            let d = i as f32 * 0.1;
            let w = falloff(d, 1.0, 0.5);
            assert!(w <= last + 1e-6);
            last = w;
        }
        assert_eq!(falloff(2.0, 1.0, 0.5), 0.0);
    }
}
