//! Mesh *data*. Editing operators live in `roedit-mesh`; this module defines
//! the representation shared by the scene graph, the renderer and serialisation.
//!
//! Faces are n-gons with per-corner UVs so that seams are representable without
//! duplicating positions. Rendering and ray tracing fan-triangulate faces.

use crate::math::{Aabb, Vec2, Vec3, EPSILON};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Face {
    pub verts: Vec<u32>,
    /// UV per corner, parallel to `verts`. Always kept at the same length.
    pub uvs: Vec<Vec2>,
    pub material: u16,
    pub smooth: bool,
}

impl Face {
    pub fn new(verts: Vec<u32>) -> Self {
        let n = verts.len();
        Self {
            verts,
            uvs: vec![Vec2::ZERO; n],
            material: 0,
            smooth: true,
        }
    }

    pub fn with_uvs(verts: Vec<u32>, uvs: Vec<Vec2>) -> Self {
        debug_assert_eq!(verts.len(), uvs.len());
        Self {
            verts,
            uvs,
            material: 0,
            smooth: true,
        }
    }

    pub fn len(&self) -> usize {
        self.verts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.verts.is_empty()
    }

    pub fn normal(&self, positions: &[Vec3]) -> Vec3 {
        newell_normal(&self.verts, positions)
    }

    pub fn centroid(&self, positions: &[Vec3]) -> Vec3 {
        if self.verts.is_empty() {
            return Vec3::ZERO;
        }
        let mut sum = Vec3::ZERO;
        for &v in &self.verts {
            sum += positions[v as usize];
        }
        sum / self.verts.len() as f32
    }

    pub fn area(&self, positions: &[Vec3]) -> f32 {
        if self.verts.len() < 3 {
            return 0.0;
        }
        let mut area = 0.0;
        let a = positions[self.verts[0] as usize];
        for i in 1..self.verts.len() - 1 {
            let b = positions[self.verts[i] as usize];
            let c = positions[self.verts[i + 1] as usize];
            area += (b - a).cross(c - a).length() * 0.5;
        }
        area
    }

    /// Triangle indices (local vertex indices into `verts`), fan triangulated.
    pub fn triangles(&self) -> Vec<[u32; 3]> {
        let mut out = Vec::new();
        if self.verts.len() < 3 {
            return out;
        }
        for i in 1..self.verts.len() - 1 {
            out.push([self.verts[0], self.verts[i], self.verts[i + 1]]);
        }
        out
    }

    /// Ensures `uvs` stays parallel to `verts` (defensive against loaded files).
    pub fn normalize(&mut self) {
        while self.uvs.len() < self.verts.len() {
            self.uvs.push(Vec2::ZERO);
        }
        self.uvs.truncate(self.verts.len());
    }
}

/// Newell's method: robust for non-planar n-gons.
pub fn newell_normal(verts: &[u32], positions: &[Vec3]) -> Vec3 {
    if verts.len() < 3 {
        return Vec3::ZERO;
    }
    let mut n = Vec3::ZERO;
    for i in 0..verts.len() {
        let a = positions[verts[i] as usize];
        let b = positions[verts[(i + 1) % verts.len()] as usize];
        n.x += (a.y - b.y) * (a.z + b.z);
        n.y += (a.z - b.z) * (a.x + b.x);
        n.z += (a.x - b.x) * (a.y + b.y);
    }
    n.normalize_or_zero()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mesh {
    pub name: String,
    pub positions: Vec<Vec3>,
    pub faces: Vec<Face>,
    /// Smoothed per-vertex normals, maintained by `recompute_normals`.
    pub normals: Vec<Vec3>,
    /// Optional per-vertex RGB colour used by vertex-paint workflows.
    #[serde(default)]
    pub colors: Vec<Vec3>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeshStats {
    pub vertices: usize,
    pub faces: usize,
    pub triangles: usize,
    pub edges: usize,
    pub quads: usize,
    pub ngons: usize,
    pub materials: usize,
    pub surface_area: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshValidation {
    pub degenerate_faces: usize,
    pub out_of_range_indices: usize,
    pub duplicate_positions: usize,
    pub non_manifold_edges: usize,
    pub boundary_edges: usize,
    pub unused_vertices: usize,
}

impl MeshValidation {
    pub fn is_clean(&self) -> bool {
        self.degenerate_faces == 0 && self.out_of_range_indices == 0 && self.non_manifold_edges == 0
    }

    pub fn summary(&self) -> String {
        format!(
            "degenerate={} out-of-range={} duplicate-verts={} non-manifold={} boundary={} unused={}",
            self.degenerate_faces,
            self.out_of_range_indices,
            self.duplicate_positions,
            self.non_manifold_edges,
            self.boundary_edges,
            self.unused_vertices
        )
    }
}

impl Mesh {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    pub fn add_vertex(&mut self, p: Vec3) -> u32 {
        self.positions.push(p);
        (self.positions.len() - 1) as u32
    }

    pub fn add_face(&mut self, face: Face) -> usize {
        self.faces.push(face);
        self.faces.len() - 1
    }

    /// Convenience: add a polygon from positions with automatic planar UVs.
    pub fn add_polygon(&mut self, points: &[Vec3]) -> usize {
        let verts: Vec<u32> = points.iter().map(|p| self.add_vertex(*p)).collect();
        let face = Face::new(verts);
        self.add_face(face)
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.faces.iter().map(|f| f.len().saturating_sub(2)).sum()
    }

    pub fn tri_indices(&self) -> Vec<[u32; 3]> {
        let mut out = Vec::with_capacity(self.triangle_count());
        for f in &self.faces {
            out.extend(f.triangles());
        }
        out
    }

    /// Unique undirected edges sorted for determinism.
    pub fn edges(&self) -> Vec<(u32, u32)> {
        let mut set: Vec<(u32, u32)> = Vec::new();
        for f in &self.faces {
            for i in 0..f.verts.len() {
                let a = f.verts[i];
                let b = f.verts[(i + 1) % f.verts.len()];
                set.push(if a < b { (a, b) } else { (b, a) });
            }
        }
        set.sort_unstable();
        set.dedup();
        set
    }

    /// Map from undirected edge to the indices of faces using it.
    pub fn edge_faces(&self) -> HashMap<(u32, u32), Vec<usize>> {
        let mut map: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (fi, f) in self.faces.iter().enumerate() {
            for i in 0..f.verts.len() {
                let a = f.verts[i];
                let b = f.verts[(i + 1) % f.verts.len()];
                let key = if a < b { (a, b) } else { (b, a) };
                map.entry(key).or_default().push(fi);
            }
        }
        map
    }

    pub fn bounds(&self) -> Aabb {
        Aabb::from_points(self.positions.iter())
    }

    pub fn stats(&self) -> MeshStats {
        let mut s = MeshStats {
            vertices: self.positions.len(),
            faces: self.faces.len(),
            triangles: self.triangle_count(),
            edges: self.edges().len(),
            ..Default::default()
        };
        let mut mats = std::collections::BTreeSet::new();
        let mut area = 0.0;
        for f in &self.faces {
            match f.len() {
                4 => s.quads += 1,
                n if n > 4 => s.ngons += 1,
                _ => {}
            }
            mats.insert(f.material);
            area += f.area(&self.positions);
        }
        s.materials = mats.len();
        s.surface_area = area;
        s
    }

    /// Recompute smoothed vertex normals (area weighted, by corner angle).
    pub fn recompute_normals(&mut self) {
        self.normals = vec![Vec3::ZERO; self.positions.len()];
        for f in &self.faces {
            let n = f.normal(&self.positions);
            if n.length_squared() < EPSILON {
                continue;
            }
            let len = f.verts.len();
            for i in 0..len {
                let prev = self.positions[f.verts[(i + len - 1) % len] as usize];
                let cur = self.positions[f.verts[i] as usize];
                let next = self.positions[f.verts[(i + 1) % len] as usize];
                let a = (prev - cur).normalize_or_zero();
                let b = (next - cur).normalize_or_zero();
                let angle = a.dot(b).clamp(-1.0, 1.0).acos();
                self.normals[f.verts[i] as usize] += n * angle.max(1e-4);
            }
        }
        for n in &mut self.normals {
            *n = n.normalize_or_zero();
        }
    }

    pub fn face_normals(&self) -> Vec<Vec3> {
        self.faces
            .iter()
            .map(|f| f.normal(&self.positions))
            .collect()
    }

    pub fn transform(&mut self, m: crate::math::Mat4) {
        for p in &mut self.positions {
            *p = m.transform_point3(*p);
        }
        let n = m.inverse().transpose();
        for nrm in &mut self.normals {
            *nrm = n.transform_vector3(*nrm).normalize_or_zero();
        }
    }

    pub fn validate(&self) -> MeshValidation {
        let mut out = MeshValidation::default();
        let mut used = vec![false; self.positions.len()];
        for f in &self.faces {
            let mut in_range = true;
            for &v in &f.verts {
                if v as usize >= self.positions.len() {
                    out.out_of_range_indices += 1;
                    in_range = false;
                } else {
                    used[v as usize] = true;
                }
            }
            // Only measure faces whose indices resolve: a corrupt file must
            // produce a report, not a panic.
            if !in_range {
                continue;
            }
            if f.verts.len() < 3 || f.area(&self.positions) < 1e-10 {
                out.degenerate_faces += 1;
            }
        }
        out.unused_vertices = used.iter().filter(|u| !**u).count();

        // Duplicate positions within a 1e-5 tolerance.
        let mut seen: HashMap<(i64, i64, i64), usize> = HashMap::new();
        for p in &self.positions {
            let key = (
                (p.x * 1e5).round() as i64,
                (p.y * 1e5).round() as i64,
                (p.z * 1e5).round() as i64,
            );
            *seen.entry(key).or_insert(0) += 1;
        }
        out.duplicate_positions = seen.values().filter(|c| **c > 1).map(|c| c - 1).sum();

        for (_edge, faces) in self.edge_faces() {
            match faces.len() {
                1 => out.boundary_edges += 1,
                2 => {}
                _ => out.non_manifold_edges += 1,
            }
        }
        out
    }

    /// Number of distinct face material slots in use.
    pub fn materials_used(&self) -> usize {
        self.faces
            .iter()
            .map(|f| f.material)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }

    /// Remove faces/vertices that are not referenced, keeping the mesh compact.
    pub fn remove_unused_vertices(&mut self) -> usize {
        let mut used = vec![false; self.positions.len()];
        for f in &self.faces {
            for &v in &f.verts {
                if (v as usize) < used.len() {
                    used[v as usize] = true;
                }
            }
        }
        let mut remap = vec![u32::MAX; self.positions.len()];
        let mut new_positions = Vec::with_capacity(self.positions.len());
        for (i, u) in used.iter().enumerate() {
            if *u {
                remap[i] = new_positions.len() as u32;
                new_positions.push(self.positions[i]);
            }
        }
        let removed = self.positions.len() - new_positions.len();
        for f in &mut self.faces {
            f.verts.retain(|v| (*v as usize) < remap.len());
            f.uvs.truncate(f.verts.len());
            for v in &mut f.verts {
                *v = remap[*v as usize];
            }
        }
        self.positions = new_positions;
        self.recompute_normals();
        removed
    }

    /// Welds vertices closer than `tolerance`, remapping faces.
    pub fn weld(&mut self, tolerance: f32) -> usize {
        let cell = tolerance.max(1e-6);
        let mut buckets: HashMap<(i64, i64, i64), u32> = HashMap::new();
        let mut remap = vec![0u32; self.positions.len()];
        let mut new_positions: Vec<Vec3> = Vec::new();
        for (i, p) in self.positions.iter().enumerate() {
            let key = (
                (p.x / cell).round() as i64,
                (p.y / cell).round() as i64,
                (p.z / cell).round() as i64,
            );
            // Probe neighbours to catch points on cell borders.
            let mut found = None;
            'outer: for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(&idx) = buckets.get(&(key.0 + dx, key.1 + dy, key.2 + dz)) {
                            if (new_positions[idx as usize] - *p).length() <= tolerance {
                                found = Some(idx);
                                break 'outer;
                            }
                        }
                    }
                }
            }
            let idx = match found {
                Some(idx) => idx,
                None => {
                    let idx = new_positions.len() as u32;
                    new_positions.push(*p);
                    buckets.insert(key, idx);
                    idx
                }
            };
            remap[i] = idx;
        }
        let before = self.positions.len();
        for f in &mut self.faces {
            for v in &mut f.verts {
                *v = remap[*v as usize];
            }
            // Drop consecutive duplicates introduced by welding.
            let mut i = 0;
            while i < f.verts.len() && f.verts.len() > 3 {
                let next = (i + 1) % f.verts.len();
                if f.verts[i] == f.verts[next] {
                    f.verts.remove(next);
                    f.uvs.remove(next);
                } else {
                    i += 1;
                }
            }
        }
        self.positions = new_positions;
        self.faces.retain(|f| f.verts.len() >= 3);
        self.recompute_normals();
        before - self.positions.len()
    }

    /// Flip winding of every face and invert normals.
    pub fn flip(&mut self) {
        for f in &mut self.faces {
            f.verts.reverse();
            f.uvs.reverse();
        }
        for n in &mut self.normals {
            *n = -*n;
        }
    }

    pub fn ensure_normals(&mut self) {
        if self.normals.len() != self.positions.len() {
            self.recompute_normals();
        }
    }

    /// Interleaved triangle soup used by the GPU path and the ray tracer.
    pub fn triangle_soup(&self) -> TriangleSoup {
        let mut positions = Vec::with_capacity(self.triangle_count() * 3);
        let mut normals = Vec::with_capacity(self.triangle_count() * 3);
        let mut uvs = Vec::with_capacity(self.triangle_count() * 3);
        let mut materials = Vec::with_capacity(self.triangle_count());
        let face_normals = self.face_normals();
        for (fi, f) in self.faces.iter().enumerate() {
            let fnrm = face_normals[fi];
            for i in 1..f.verts.len().saturating_sub(1) {
                let corners = [0usize, i, i + 1];
                for c in corners {
                    let vi = f.verts[c] as usize;
                    positions.push(self.positions[vi]);
                    let n = if f.smooth {
                        self.normals.get(vi).copied().unwrap_or(fnrm)
                    } else {
                        fnrm
                    };
                    normals.push(n);
                    uvs.push(f.uvs.get(c).copied().unwrap_or(Vec2::ZERO));
                }
                materials.push(f.material);
            }
        }
        TriangleSoup {
            positions,
            normals,
            uvs,
            materials,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TriangleSoup {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub materials: Vec<u16>,
}

impl TriangleSoup {
    pub fn triangle_count(&self) -> usize {
        self.positions.len() / 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Mat4;

    fn quad() -> Mesh {
        let mut m = Mesh::new("quad");
        m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        m.add_vertex(Vec3::new(1.0, 0.0, 0.0));
        m.add_vertex(Vec3::new(1.0, 0.0, 1.0));
        m.add_vertex(Vec3::new(0.0, 0.0, 1.0));
        m.add_face(Face::new(vec![0, 1, 2, 3]));
        m.recompute_normals();
        m
    }

    #[test]
    fn quad_normals_and_stats() {
        let m = quad();
        assert_eq!(m.triangle_count(), 2);
        assert_eq!(m.stats().quads, 1);
        assert!((m.normals[0].dot(Vec3::Y)).abs() > 0.99);
        let s = m.stats();
        assert_eq!(s.edges, 4);
        assert!((s.surface_area - 1.0).abs() < 1e-5);
    }

    #[test]
    fn weld_and_cleanup() {
        let mut m = quad();
        // duplicate every vertex
        let base = m.positions.clone();
        for p in base {
            m.add_vertex(p);
        }
        assert_eq!(m.validate().duplicate_positions, 4);
        let merged = m.weld(1e-4);
        assert_eq!(merged, 4);
        let v = m.validate();
        assert_eq!(v.duplicate_positions, 0);
        assert_eq!(v.unused_vertices, 0);
    }

    #[test]
    fn transform_applies_scale() {
        let mut m = quad();
        m.transform(Mat4::from_scale(Vec3::splat(3.0)));
        let b = m.bounds();
        assert!((b.size() - Vec3::new(3.0, 0.0, 3.0)).length() < 1e-5);
    }

    #[test]
    fn validation_detects_bad_index() {
        let mut m = quad();
        m.faces[0].verts.push(99);
        m.faces[0].uvs.push(Vec2::ZERO);
        assert!(m.validate().out_of_range_indices > 0);
    }

    #[test]
    fn triangle_soup_matches_face_count() {
        let m = quad();
        let soup = m.triangle_soup();
        assert_eq!(soup.triangle_count(), m.triangle_count());
        assert_eq!(soup.materials.len(), 2);
    }
}
