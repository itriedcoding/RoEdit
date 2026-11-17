//! STL import (binary and ASCII) and export (binary and ASCII).

use crate::ImportError;
use roedit_core::math::Vec3;
use roedit_core::mesh::{Face, Mesh};

/// Imports an STL file, detecting binary versus ASCII automatically.
pub fn import_stl(bytes: &[u8]) -> Result<Mesh, ImportError> {
    if bytes.len() >= 84 {
        let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        if count > 0 && 84 + count * 50 == bytes.len() {
            return import_stl_binary(bytes, count);
        }
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ImportError::Invalid("STL is neither valid binary nor UTF-8 ASCII".into()))?;
    import_stl_ascii(text)
}

fn import_stl_binary(bytes: &[u8], count: usize) -> Result<Mesh, ImportError> {
    let mut mesh = Mesh::new("STL");
    for t in 0..count {
        let base = 84 + t * 50 + 12; // skip the per-facet normal
        let mut tri = [Vec3::ZERO; 3];
        for (i, slot) in tri.iter_mut().enumerate() {
            let o = base + i * 12;
            let x = f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
            let y = f32::from_le_bytes([bytes[o + 4], bytes[o + 5], bytes[o + 6], bytes[o + 7]]);
            let z = f32::from_le_bytes([bytes[o + 8], bytes[o + 9], bytes[o + 10], bytes[o + 11]]);
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                return Err(ImportError::Invalid(format!(
                    "triangle {t} has non-finite vertices"
                )));
            }
            *slot = Vec3::new(x, y, z);
        }
        let verts: Vec<u32> = tri.iter().map(|p| mesh.add_vertex(*p)).collect();
        mesh.add_face(Face::new(verts));
    }
    mesh.weld(1e-5);
    mesh.recompute_normals();
    Ok(mesh)
}

fn import_stl_ascii(text: &str) -> Result<Mesh, ImportError> {
    let mut mesh = Mesh::new("STL");
    let mut tri: Vec<Vec3> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("vertex ") {
            let values: Vec<f32> = rest
                .split_whitespace()
                .filter_map(|v| v.parse().ok())
                .collect();
            if values.len() < 3 {
                return Err(ImportError::Invalid("malformed ASCII STL vertex".into()));
            }
            tri.push(Vec3::new(values[0], values[1], values[2]));
        } else if line.starts_with("endloop") || line.starts_with("endfacet") {
            if tri.len() >= 3 {
                let verts: Vec<u32> = tri.iter().map(|p| mesh.add_vertex(*p)).collect();
                mesh.add_face(Face::new(verts));
            }
            tri.clear();
        }
    }
    if mesh.faces.is_empty() {
        return Err(ImportError::Invalid(
            "the STL file contains no triangles".into(),
        ));
    }
    mesh.weld(1e-5);
    mesh.recompute_normals();
    Ok(mesh)
}

/// Writes a binary STL (80-byte header, facet count, 50 bytes per triangle).
pub fn export_stl_binary(mesh: &Mesh) -> Vec<u8> {
    let tris: Vec<[Vec3; 3]> = triangulate(mesh);
    let mut out = Vec::with_capacity(84 + tris.len() * 50);
    let mut header = [0u8; 80];
    let tag = b"RoEdit Studio binary STL";
    header[..tag.len()].copy_from_slice(tag);
    out.extend_from_slice(&header);
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for t in &tris {
        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        for v in [n, t[0], t[1], t[2]] {
            out.extend_from_slice(&v.x.to_le_bytes());
            out.extend_from_slice(&v.y.to_le_bytes());
            out.extend_from_slice(&v.z.to_le_bytes());
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    out
}

/// Writes ASCII STL, the format older tools expect.
pub fn export_stl_ascii(mesh: &Mesh) -> String {
    let tris = triangulate(mesh);
    let mut out = String::from("solid RoEdit\n");
    for t in &tris {
        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        out.push_str(&format!("  facet normal {} {} {}\n", n.x, n.y, n.z));
        out.push_str("    outer loop\n");
        for v in t {
            out.push_str(&format!("      vertex {} {} {}\n", v.x, v.y, v.z));
        }
        out.push_str("    endloop\n  endfacet\n");
    }
    out.push_str("endsolid RoEdit\n");
    out
}

fn triangulate(mesh: &Mesh) -> Vec<[Vec3; 3]> {
    let mut tris = Vec::new();
    for f in &mesh.faces {
        if f.verts.len() < 3 {
            continue;
        }
        let p: Vec<Vec3> = f
            .verts
            .iter()
            .filter_map(|i| mesh.positions.get(*i as usize).copied())
            .collect();
        for i in 1..p.len().saturating_sub(1) {
            tris.push([p[0], p[i], p[i + 1]]);
        }
    }
    tris
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_mesh::primitives;

    #[test]
    fn binary_roundtrip() {
        let mesh = primitives::cube(Vec3::splat(1.0));
        let bytes = export_stl_binary(&mesh);
        assert_eq!(bytes.len(), 84 + 12 * 50);
        let back = import_stl(&bytes).unwrap();
        assert_eq!(back.faces.len(), 12);
        assert_eq!(
            back.positions.len(),
            8,
            "welding should merge shared corners"
        );
        let b = back.bounds();
        assert!((b.size() - Vec3::splat(1.0)).length() < 1e-4);
    }

    #[test]
    fn ascii_roundtrip() {
        let mesh = primitives::cube(Vec3::splat(2.0));
        let text = export_stl_ascii(&mesh);
        assert!(text.contains("facet normal"));
        let back = import_stl(text.as_bytes()).unwrap();
        assert_eq!(back.faces.len(), 12);
    }

    #[test]
    fn rejects_garbage() {
        assert!(import_stl(b"not an stl file").is_err());
    }

    #[test]
    fn exact_binary_length_detection() {
        // A binary STL with exactly the right length must not be misread as ASCII.
        let bytes = export_stl_binary(&primitives::cube(Vec3::ONE));
        assert_eq!(bytes.len(), 84 + 12 * 50);
        assert!(import_stl(&bytes).is_ok());
    }
}
