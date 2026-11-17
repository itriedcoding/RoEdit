//! Wavefront OBJ import and export.
//!
//! Import handles `v`, `vt`, `vn`, `f` (any polygon arity, `v/vt/vn` and
//! negative indices), `o`/`g` groups and `usemtl`. Export writes a companion
//! MTL file with a PBR-flavoured material block per slot.

use crate::ImportError;
use roedit_core::math::{Vec2, Vec3};
use roedit_core::mesh::{Face, Mesh};
use std::collections::HashMap;

fn resolve_index(token: &str, count: usize) -> Option<usize> {
    if token.is_empty() {
        return None;
    }
    let value: i64 = token.parse().ok()?;
    if value > 0 {
        let idx = value as usize - 1;
        if idx < count {
            Some(idx)
        } else {
            None
        }
    } else if value < 0 {
        let idx = count as i64 + value;
        if idx >= 0 && (idx as usize) < count {
            Some(idx as usize)
        } else {
            None
        }
    } else {
        None
    }
}

struct Builder {
    name: String,
    mesh: Mesh,
    /// global OBJ vertex index -> local mesh vertex index
    map: HashMap<usize, u32>,
}

impl Builder {
    fn new(name: String) -> Self {
        Self {
            name,
            mesh: Mesh::new("OBJ"),
            map: HashMap::new(),
        }
    }

    fn local_vertex(&mut self, global: usize, positions: &[Vec3]) -> u32 {
        if let Some(v) = self.map.get(&global) {
            return *v;
        }
        let local = self.mesh.add_vertex(positions[global]);
        self.map.insert(global, local);
        local
    }

    fn finish(mut self) -> (String, Mesh) {
        self.mesh.name = self.name.clone();
        self.mesh.recompute_normals();
        (self.name, self.mesh)
    }
}

/// Imports an OBJ file into one mesh per `o`/`g` group.
pub fn import_obj_multi(text: &str) -> Result<Vec<(String, Mesh)>, ImportError> {
    let mut positions: Vec<Vec3> = Vec::new();
    let mut texcoords: Vec<Vec2> = Vec::new();
    let mut builders: Vec<Builder> = Vec::new();
    let mut material_names: Vec<String> = Vec::new();
    let mut current_material: u16 = 0;
    let mut current = Builder::new("Imported Mesh".to_string());

    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(tag) = parts.next() else { continue };
        match tag {
            "v" => {
                let values: Vec<f32> = parts.filter_map(|p| p.parse().ok()).collect();
                if values.len() < 3 {
                    return Err(ImportError::Invalid(format!(
                        "line {}: vertex needs three coordinates",
                        line_no + 1
                    )));
                }
                positions.push(Vec3::new(values[0], values[1], values[2]));
            }
            "vt" => {
                let values: Vec<f32> = parts.filter_map(|p| p.parse().ok()).collect();
                if values.len() >= 2 {
                    texcoords.push(Vec2::new(values[0], 1.0 - values[1]));
                }
            }
            "vn" => { /* normals are recomputed */ }
            "o" | "g" => {
                let name = parts.collect::<Vec<_>>().join(" ");
                if !current.mesh.faces.is_empty() {
                    builders.push(std::mem::replace(
                        &mut current,
                        Builder::new(if name.is_empty() {
                            "Imported Mesh".to_string()
                        } else {
                            name
                        }),
                    ));
                } else if !name.is_empty() {
                    current.name = name;
                }
            }
            "usemtl" => {
                let name = parts.collect::<Vec<_>>().join(" ");
                let index = match material_names.iter().position(|m| *m == name) {
                    Some(i) => i as u16,
                    None => {
                        material_names.push(name);
                        (material_names.len() - 1) as u16
                    }
                };
                current_material = index;
            }
            "f" => {
                let mut verts = Vec::new();
                let mut uvs: Vec<Vec2> = Vec::new();
                for token in parts {
                    let fields: Vec<&str> = token.split('/').collect();
                    let Some(global) = resolve_index(fields[0], positions.len()) else {
                        return Err(ImportError::Invalid(format!(
                            "line {}: face references a missing vertex '{}'",
                            line_no + 1,
                            fields[0]
                        )));
                    };
                    let uv = fields
                        .get(1)
                        .and_then(|t| resolve_index(t, texcoords.len()))
                        .map(|i| texcoords[i])
                        .unwrap_or(Vec2::ZERO);
                    verts.push(current.local_vertex(global, &positions));
                    uvs.push(uv);
                }
                if verts.len() < 3 {
                    continue;
                }
                let mut face = Face::with_uvs(verts, uvs);
                face.material = current_material;
                current.mesh.add_face(face);
            }
            _ => {}
        }
    }
    if !current.mesh.faces.is_empty() {
        builders.push(current);
    }
    if builders.is_empty() {
        return Err(ImportError::Invalid(
            "the OBJ file contains no faces".to_string(),
        ));
    }
    Ok(builders.into_iter().map(Builder::finish).collect())
}

/// Imports an OBJ file, merging all groups into a single mesh.
pub fn import_obj(text: &str, name: &str) -> Result<Mesh, ImportError> {
    let parts = import_obj_multi(text)?;
    if parts.len() == 1 {
        let mut mesh = parts.into_iter().next().unwrap().1;
        mesh.name = name.to_string();
        return Ok(mesh);
    }
    let mut merged = Mesh::new(name);
    for (_, part) in parts {
        let base = merged.positions.len() as u32;
        merged.positions.extend(part.positions.iter().copied());
        for f in &part.faces {
            let verts = f.verts.iter().map(|v| base + *v).collect();
            let mut nf = Face::with_uvs(verts, f.uvs.clone());
            nf.material = f.material;
            nf.smooth = f.smooth;
            merged.add_face(nf);
        }
    }
    merged.recompute_normals();
    Ok(merged)
}

fn key_for(v: Vec2) -> (u32, u32) {
    (v.x.to_bits(), v.y.to_bits())
}

fn key_for3(v: Vec3) -> (u32, u32, u32) {
    (v.x.to_bits(), v.y.to_bits(), v.z.to_bits())
}

/// Exports a mesh to OBJ text plus a matching MTL document.
pub fn export_obj(mesh: &Mesh, object_name: &str, material_names: &[String]) -> (String, String) {
    let mut out = String::new();
    let mtl_name = format!("{object_name}.mtl");
    out.push_str("# RoEdit Studio OBJ export\n");
    out.push_str(&format!("mtllib {mtl_name}\n"));
    out.push_str(&format!("o {object_name}\n"));
    for p in &mesh.positions {
        out.push_str(&format!("v {:.6} {:.6} {:.6}\n", p.x, p.y, p.z));
    }
    // Deduplicate UVs and normals.
    let mut uvs: Vec<Vec2> = Vec::new();
    let mut uv_map: HashMap<(u32, u32), usize> = HashMap::new();
    let mut normals: Vec<Vec3> = Vec::new();
    let mut normal_map: HashMap<(u32, u32, u32), usize> = HashMap::new();
    let face_normals = mesh.face_normals();
    for (fi, f) in mesh.faces.iter().enumerate() {
        for i in 0..f.verts.len() {
            let uv = f.uvs.get(i).copied().unwrap_or(Vec2::ZERO);
            uv_map.entry(key_for(uv)).or_insert_with(|| {
                uvs.push(uv);
                uvs.len() - 1
            });
            let n = if f.smooth {
                mesh.normals
                    .get(f.verts[i] as usize)
                    .copied()
                    .unwrap_or(face_normals[fi])
            } else {
                face_normals[fi]
            };
            normal_map.entry(key_for3(n)).or_insert_with(|| {
                normals.push(n);
                normals.len() - 1
            });
        }
    }
    for uv in &uvs {
        out.push_str(&format!("vt {:.6} {:.6}\n", uv.x, 1.0 - uv.y));
    }
    for n in &normals {
        out.push_str(&format!("vn {:.6} {:.6} {:.6}\n", n.x, n.y, n.z));
    }
    let mut last_material: Option<u16> = None;
    for f in &mesh.faces {
        if last_material != Some(f.material) {
            let name = material_names
                .get(f.material as usize)
                .cloned()
                .unwrap_or_else(|| format!("material_{}", f.material));
            out.push_str(&format!("usemtl {name}\n"));
            last_material = Some(f.material);
        }
        out.push('f');
        for i in 0..f.verts.len() {
            let uv = f.uvs.get(i).copied().unwrap_or(Vec2::ZERO);
            let n = if f.smooth {
                mesh.normals
                    .get(f.verts[i] as usize)
                    .copied()
                    .unwrap_or(Vec3::Y)
            } else {
                f.normal(&mesh.positions)
            };
            let uv_index = uv_map[&key_for(uv)] + 1;
            let n_index = normal_map.get(&key_for3(n)).copied().unwrap_or(0) + 1;
            out.push_str(&format!(" {}/{}/{}", f.verts[i] + 1, uv_index, n_index));
        }
        out.push('\n');
    }
    // Material document.
    let mut mtl = String::from("# RoEdit Studio material export\n");
    let slots = mesh.materials_used().max(material_names.len()).max(1);
    for i in 0..slots {
        let name = material_names
            .get(i)
            .cloned()
            .unwrap_or_else(|| format!("material_{i}"));
        mtl.push_str(&format!("newmtl {name}\n"));
        mtl.push_str("Kd 0.800 0.800 0.820\n");
        mtl.push_str("Ka 0.000 0.000 0.000\n");
        mtl.push_str("Ks 0.500 0.500 0.500\n");
        mtl.push_str("Ns 96.0\n");
        mtl.push_str("d 1.0\n");
        mtl.push_str("Pr 0.450\n");
        mtl.push_str("Pm 0.000\n");
        mtl.push('\n');
    }
    (out, mtl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_mesh::primitives;

    #[test]
    fn roundtrip_preserves_geometry() {
        let cube = primitives::cube(Vec3::new(2.0, 2.0, 2.0));
        let (obj, mtl) = export_obj(&cube, "Cube", &["Plastic".to_string()]);
        assert!(mtl.contains("newmtl Plastic"));
        let back = import_obj(&obj, "Cube").unwrap();
        assert_eq!(back.faces.len(), cube.faces.len());
        assert_eq!(back.positions.len(), cube.positions.len());
        let b = back.bounds();
        assert!((b.size() - Vec3::splat(2.0)).length() < 1e-4);
        assert!(back.faces.iter().all(|f| f.uvs.len() == f.verts.len()));
    }

    #[test]
    fn parses_negative_indices_and_groups() {
        let text = "\
o A
v 0 0 0
v 1 0 0
v 1 1 0
vt 0 0
vt 1 0
vt 1 1
f 1/1 2/2 3/3
o B
v 0 0 5
v 1 0 5
v 1 1 5
f -3 -2 -1
";
        let parts = import_obj_multi(text).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, "A");
        assert_eq!(parts[0].1.faces.len(), 1);
        assert_eq!(parts[1].0, "B");
        assert_eq!(parts[1].1.faces.len(), 1);
        assert_eq!(parts[1].1.positions[0].z, 5.0);
    }

    #[test]
    fn vertex_index_out_of_range_is_an_error() {
        let text = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 9\n";
        let err = import_obj(text, "bad").unwrap_err();
        assert!(matches!(err, ImportError::Invalid(_)));
    }

    #[test]
    fn empty_file_is_rejected() {
        assert!(import_obj("# nothing\n", "empty").is_err());
    }

    #[test]
    fn materials_are_tracked() {
        let text = "\
v 0 0 0
v 1 0 0
v 0 1 0
usemtl Red
f 1 2 3
usemtl Blue
f 1 2 3
";
        let mesh = import_obj(text, "mats").unwrap();
        assert_eq!(mesh.faces.len(), 2);
        assert_eq!(mesh.materials_used(), 2);
        assert_ne!(mesh.faces[0].material, mesh.faces[1].material);
    }
}
