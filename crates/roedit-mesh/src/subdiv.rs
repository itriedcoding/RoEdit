//! Catmull-Clark subdivision surfaces.
//!
//! One level turns every n-gon into quads, smooths interior vertices with the
//! standard (F + 2R + (n-3)P) / n rule and keeps open boundaries crisp by using
//! the 6/8, 1/8, 1/8 boundary rule.

use crate::{edge_key, Mesh, Vec2, Vec3};
use roedit_core::mesh::Face;
use std::collections::HashMap;

/// Applies `levels` Catmull-Clark subdivisions.
pub fn catmull_clark(mesh: &Mesh, levels: u32) -> Mesh {
    let mut current = mesh.clone();
    for _ in 0..levels.clamp(0, 5) {
        current = catmull_clark_level(&current);
    }
    current
}

pub fn catmull_clark_level(mesh: &Mesh) -> Mesh {
    if mesh.faces.is_empty() {
        return mesh.clone();
    }
    let source = mesh;
    let n_verts = source.positions.len();
    let n_faces = source.faces.len();

    let face_points: Vec<Vec3> = source
        .faces
        .iter()
        .map(|f| f.centroid(&source.positions))
        .collect();

    // Deterministic edge ordering.
    let mut edge_keys: Vec<(u32, u32)> = source
        .faces
        .iter()
        .flat_map(|f| {
            (0..f.verts.len()).map(move |i| edge_key(f.verts[i], f.verts[(i + 1) % f.verts.len()]))
        })
        .collect();
    edge_keys.sort_unstable();
    edge_keys.dedup();

    let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (fi, f) in source.faces.iter().enumerate() {
        for i in 0..f.verts.len() {
            let k = edge_key(f.verts[i], f.verts[(i + 1) % f.verts.len()]);
            edge_faces.entry(k).or_default().push(fi);
        }
    }

    // Incidence maps for the vertex rule.
    let mut vertex_edges: Vec<Vec<(u32, u32)>> = vec![Vec::new(); n_verts];
    for (a, b) in &edge_keys {
        vertex_edges[*a as usize].push((*a, *b));
        vertex_edges[*b as usize].push((*a, *b));
    }
    let mut vertex_faces: Vec<Vec<usize>> = vec![Vec::new(); n_verts];
    for (fi, f) in source.faces.iter().enumerate() {
        for v in &f.verts {
            if (*v as usize) < n_verts {
                vertex_faces[*v as usize].push(fi);
            }
        }
    }

    let mut positions: Vec<Vec3> = Vec::with_capacity(n_verts + n_faces + edge_keys.len());
    for v in 0..n_verts {
        let p = source.positions[v];
        let boundary_neighbours: Vec<u32> = vertex_edges[v]
            .iter()
            .filter(|(a, b)| {
                edge_faces
                    .get(&edge_key(*a, *b))
                    .map(|f| f.len())
                    .unwrap_or(0)
                    == 1
            })
            .map(|(a, b)| if *a as usize == v { *b } else { *a })
            .collect();
        if !boundary_neighbours.is_empty() {
            // Crease rule: 6/8 P + 1/8 (n1 + n2)
            let mut sum = Vec3::ZERO;
            for n in &boundary_neighbours {
                sum += source.positions[*n as usize];
            }
            let count = boundary_neighbours.len() as f32;
            positions.push(p * 0.75 + (sum / count) * 0.25);
            continue;
        }
        let incident = &vertex_edges[v];
        let valence = incident.len() as f32;
        if valence < 2.0 {
            positions.push(p);
            continue;
        }
        let mut edge_mid_sum = Vec3::ZERO;
        for (a, b) in incident {
            edge_mid_sum += (source.positions[*a as usize] + source.positions[*b as usize]) * 0.5;
        }
        let r = edge_mid_sum / valence;
        let faces = &vertex_faces[v];
        let f_avg = if faces.is_empty() {
            p
        } else {
            let mut s = Vec3::ZERO;
            for fi in faces {
                s += face_points[*fi];
            }
            s / faces.len() as f32
        };
        positions.push((f_avg + r * 2.0 + p * (valence - 3.0)) / valence);
    }

    // Edge points.
    let mut edge_uv_base: HashMap<(u32, u32), u32> = HashMap::new();
    for (a, b) in &edge_keys {
        let pa = source.positions[*a as usize];
        let pb = source.positions[*b as usize];
        let adjacent = &edge_faces[&edge_key(*a, *b)];
        let point = if adjacent.len() == 2 {
            (pa + pb + face_points[adjacent[0]] + face_points[adjacent[1]]) * 0.25
        } else {
            (pa + pb) * 0.5
        };
        edge_uv_base.insert((*a, *b), positions.len() as u32);
        positions.push(point);
    }
    // Face points.
    for p in &face_points {
        positions.push(*p);
    }

    let mut out = Mesh::new(format!("{} Subdivided", source.name));
    out.positions = positions;

    for (fi, f) in source.faces.iter().enumerate() {
        let n = f.verts.len();
        if n < 3 {
            continue;
        }
        let face_uv = if f.uvs.len() == n {
            f.uvs.iter().copied().sum::<Vec2>() / n as f32
        } else {
            Vec2::ZERO
        };
        for i in 0..n {
            let prev = (i + n - 1) % n;
            let current = f.verts[i];
            let next = f.verts[(i + 1) % n];
            let e_next = *edge_uv_base.get(&edge_key(current, next)).unwrap();
            let e_prev = *edge_uv_base.get(&edge_key(f.verts[prev], current)).unwrap();
            let face_point_index = n_verts + edge_keys.len() + fi;
            let uvs = if f.uvs.len() == n {
                vec![
                    f.uvs[i],
                    (f.uvs[i] + f.uvs[(i + 1) % n]) * 0.5,
                    face_uv,
                    (f.uvs[prev] + f.uvs[i]) * 0.5,
                ]
            } else {
                vec![Vec2::ZERO; 4]
            };
            let mut q = Face::with_uvs(vec![current, e_next, face_point_index as u32, e_prev], uvs);
            q.material = f.material;
            q.smooth = f.smooth;
            out.add_face(q);
        }
    }
    out.recompute_normals();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;
    use crate::signed_volume;

    #[test]
    fn one_level_quadruples_faces() {
        let cube = primitives::cube(Vec3::ONE);
        let once = catmull_clark(&cube, 1);
        assert_eq!(once.faces.len(), 24);
        assert_eq!(once.positions.len(), 8 + 12 + 6);
        assert!(once.faces.iter().all(|f| f.verts.len() == 4));
        assert!(once.faces.iter().all(|f| f.uvs.len() == 4));
    }

    #[test]
    fn subdivision_stays_inside_bounds_and_positive() {
        let cube = primitives::cube(Vec3::ONE);
        let once = catmull_clark(&cube, 1);
        let b = once.bounds();
        assert!(b.min.x > -0.5 - 1e-4 && b.max.x < 0.5 + 1e-4);
        assert!(signed_volume(&once) > 0.0);
        assert!(once.validate().boundary_edges == 0);
        assert!(once.validate().out_of_range_indices == 0);
    }

    #[test]
    fn two_levels_are_finer() {
        let cube = primitives::cube(Vec3::ONE);
        let twice = catmull_clark(&cube, 2);
        assert_eq!(twice.faces.len(), 96);
        assert!(twice.positions.iter().all(|p| p.is_finite()));
    }

    #[test]
    fn boundary_surface_keeps_its_border() {
        let plane = primitives::grid_plane(1.0, 1);
        let sub = catmull_clark(&plane, 1);
        assert_eq!(sub.faces.len(), 4);
        let b = sub.bounds();
        // Corners stay put on the crease rule, so the border keeps its extent.
        assert!((b.min.x + 0.5).abs() < 1e-5);
        assert!((b.max.x - 0.5).abs() < 1e-5);
    }

    #[test]
    fn empty_mesh_is_handled() {
        let m = Mesh::new("empty");
        let out = catmull_clark(&m, 2);
        assert!(out.positions.is_empty());
    }
}
