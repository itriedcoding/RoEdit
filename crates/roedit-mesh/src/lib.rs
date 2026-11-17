//! Modeling operators for RoEdit Studio.
//!
//! Everything here is deterministic, based on the n-gon mesh in
//! `roedit_core::mesh`, and unit tested: region extrude/inset/bevel, loop cuts,
//! simple and Catmull-Clark subdivision, decimation, mirror/array/solidify
//! modifiers, sculpt brushes, UV projection/packing and Roblox rig building.

pub mod edit;
pub mod primitives;
pub mod roblox;
pub mod sculpt;
pub mod subdiv;
pub mod uv;

pub use roedit_core::math::{Vec2, Vec3};
pub use roedit_core::mesh::{Face, Mesh};

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MeshError {
    #[error("nothing is selected")]
    EmptySelection,
    #[error("face index {0} is out of range")]
    FaceOutOfRange(usize),
    #[error("face {0} is not a quad")]
    NotAQuad(usize),
    #[error("invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("the mesh has no geometry")]
    NoGeometry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

    pub fn vector(self) -> Vec3 {
        match self {
            Axis::X => Vec3::X,
            Axis::Y => Vec3::Y,
            Axis::Z => Vec3::Z,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Axis::X => "X",
            Axis::Y => "Y",
            Axis::Z => "Z",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    Vertex,
    Edge,
    Face,
    Object,
}

impl SelectionMode {
    pub const ALL: [SelectionMode; 4] = [
        SelectionMode::Vertex,
        SelectionMode::Edge,
        SelectionMode::Face,
        SelectionMode::Object,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SelectionMode::Vertex => "Vertex",
            SelectionMode::Edge => "Edge",
            SelectionMode::Face => "Face",
            SelectionMode::Object => "Object",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    pub vertices: BTreeSet<u32>,
    pub edges: BTreeSet<(u32, u32)>,
    pub faces: BTreeSet<usize>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.edges.clear();
        self.faces.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() && self.edges.is_empty() && self.faces.is_empty()
    }

    pub fn face_list(&self) -> Vec<usize> {
        self.faces.iter().copied().collect()
    }

    pub fn select_all_faces(&mut self, mesh: &Mesh) {
        self.faces = (0..mesh.faces.len()).collect();
        self.vertices = mesh
            .positions
            .iter()
            .enumerate()
            .map(|(i, _)| i as u32)
            .collect();
    }

    pub fn toggle_face(&mut self, index: usize) {
        if !self.faces.remove(&index) {
            self.faces.insert(index);
        }
    }

    pub fn select_face(&mut self, index: usize) {
        self.faces.insert(index);
    }

    pub fn clear_faces(&mut self) {
        self.faces.clear();
    }

    pub fn invert_faces(&mut self, mesh: &Mesh) {
        let all: BTreeSet<usize> = (0..mesh.faces.len()).collect();
        self.faces = all.difference(&self.faces).copied().collect();
    }

    /// Expands the selection with every face touching a selected vertex/edge.
    pub fn grow(&mut self, mesh: &Mesh) {
        let mut extra: BTreeSet<usize> = BTreeSet::new();
        for (fi, f) in mesh.faces.iter().enumerate() {
            if f.verts.iter().any(|v| self.vertices.contains(v)) {
                extra.insert(fi);
            }
            for i in 0..f.verts.len() {
                let a = f.verts[i];
                let b = f.verts[(i + 1) % f.verts.len()];
                if self.edges.contains(&edge_key(a, b)) {
                    extra.insert(fi);
                }
            }
        }
        self.faces.extend(extra);
        self.sync_vertices(mesh);
    }

    pub fn shrink(&mut self, mesh: &Mesh) {
        // Keep only faces whose vertices are all selected.
        let verts = self.vertices.clone();
        let keep: BTreeSet<usize> = self
            .faces
            .iter()
            .copied()
            .filter(|fi| {
                mesh.faces
                    .get(*fi)
                    .map(|f| f.verts.iter().all(|v| verts.contains(v)))
                    .unwrap_or(false)
            })
            .collect();
        self.faces = keep;
        self.sync_vertices(mesh);
    }

    /// Rebuilds the vertex/edge sets from the selected faces.
    pub fn sync_vertices(&mut self, mesh: &Mesh) {
        self.vertices.clear();
        self.edges.clear();
        for &fi in &self.faces {
            if let Some(f) = mesh.faces.get(fi) {
                for i in 0..f.verts.len() {
                    self.vertices.insert(f.verts[i]);
                    let a = f.verts[i];
                    let b = f.verts[(i + 1) % f.verts.len()];
                    self.edges.insert(edge_key(a, b));
                }
            }
        }
    }

    pub fn selection_center(&self, mesh: &Mesh) -> Vec3 {
        if self.vertices.is_empty() {
            if self.faces.is_empty() {
                return Vec3::ZERO;
            }
            let mut sum = Vec3::ZERO;
            let mut n = 0;
            for &fi in &self.faces {
                if let Some(f) = mesh.faces.get(fi) {
                    sum += f.centroid(&mesh.positions);
                    n += 1;
                }
            }
            return if n == 0 { Vec3::ZERO } else { sum / n as f32 };
        }
        let mut sum = Vec3::ZERO;
        for v in &self.vertices {
            if let Some(p) = mesh.positions.get(*v as usize) {
                sum += *p;
            }
        }
        sum / self.vertices.len() as f32
    }
}

pub fn edge_key(a: u32, b: u32) -> (u32, u32) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Signed volume of a closed mesh; used to detect inverted winding.
pub fn signed_volume(mesh: &Mesh) -> f32 {
    let mut volume = 0.0;
    for f in &mesh.faces {
        for tri in f.triangles() {
            let a = mesh.positions[tri[0] as usize];
            let b = mesh.positions[tri[1] as usize];
            let c = mesh.positions[tri[2] as usize];
            volume += a.dot(b.cross(c)) / 6.0;
        }
    }
    volume
}

/// Total surface area.
pub fn surface_area(mesh: &Mesh) -> f32 {
    mesh.faces.iter().map(|f| f.area(&mesh.positions)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn cube_volume_is_positive() {
        let m = primitives::cube(Vec3::ONE);
        assert!((signed_volume(&m) - 1.0).abs() < 1e-4);
        assert!((surface_area(&m) - 6.0).abs() < 1e-4);
    }

    #[test]
    fn selection_grow_shrink() {
        let mut mesh = primitives::grid_plane(2.0, 2);
        let mut sel = Selection::default();
        sel.select_face(0);
        sel.sync_vertices(&mesh);
        let before = sel.faces.len();
        sel.grow(&mesh);
        assert!(sel.faces.len() > before);
        let grown = sel.faces.len();
        sel.shrink(&mesh);
        assert!(sel.faces.len() <= grown);
        sel.select_all_faces(&mesh);
        assert_eq!(sel.faces.len(), mesh.faces.len());
        sel.invert_faces(&mesh);
        assert!(sel.faces.is_empty());
        let _ = &mut mesh;
    }

    #[test]
    fn edge_key_is_ordered() {
        assert_eq!(edge_key(5, 2), (2, 5));
        assert_eq!(edge_key(2, 5), (2, 5));
    }

    #[test]
    fn selection_center() {
        let mesh = primitives::cube(Vec3::ONE);
        let mut sel = Selection::default();
        sel.select_all_faces(&mesh);
        sel.sync_vertices(&mesh);
        assert!(sel.selection_center(&mesh).length() < 1e-5);
    }
}
