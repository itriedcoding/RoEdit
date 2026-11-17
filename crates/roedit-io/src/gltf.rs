//! glTF 2.0 binary (`.glb`) import and export.
//!
//! Import supports positions, normals, UV0, indices and node transforms.
//! Export writes a single-buffer GLB with one node per mesh. Both directions
//! triangulate n-gons with a fan, which is exact for convex faces.

use crate::ImportError;
use roedit_core::math::{Quat, Vec2, Vec3};
use roedit_core::mesh::{Face, Mesh};
use serde_json::{json, Value};

const MAGIC: &[u8; 4] = b"glTF";
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;

fn align4(bytes: &mut Vec<u8>, pad: u8) {
    while bytes.len() % 4 != 0 {
        bytes.push(pad);
    }
}

/// Exports meshes as a single GLB document.
pub fn export_glb(meshes: &[(String, Mesh)], base_color: [f32; 4]) -> Result<Vec<u8>, ImportError> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views: Vec<Value> = Vec::new();
    let mut accessors: Vec<Value> = Vec::new();
    let mut json_meshes: Vec<Value> = Vec::new();
    let mut nodes: Vec<Value> = Vec::new();

    fn add_view(bin: &mut Vec<u8>, views: &mut Vec<Value>, data: &[u8], target: u32) -> usize {
        align4(bin, 0);
        let offset = bin.len();
        bin.extend_from_slice(data);
        views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": data.len(),
            "target": target
        }));
        views.len() - 1
    }

    for (name, mesh) in meshes {
        if mesh.positions.is_empty() || mesh.faces.is_empty() {
            continue;
        }
        let mut positions: Vec<u8> = Vec::new();
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        let mut normals: Vec<u8> = Vec::new();
        let mut uvs: Vec<u8> = Vec::new();
        for (i, p) in mesh.positions.iter().enumerate() {
            positions.extend_from_slice(&p.x.to_le_bytes());
            positions.extend_from_slice(&p.y.to_le_bytes());
            positions.extend_from_slice(&p.z.to_le_bytes());
            min[0] = min[0].min(p.x);
            min[1] = min[1].min(p.y);
            min[2] = min[2].min(p.z);
            max[0] = max[0].max(p.x);
            max[1] = max[1].max(p.y);
            max[2] = max[2].max(p.z);
            let n = mesh.normals.get(i).copied().unwrap_or(Vec3::Y);
            normals.extend_from_slice(&n.x.to_le_bytes());
            normals.extend_from_slice(&n.y.to_le_bytes());
            normals.extend_from_slice(&n.z.to_le_bytes());
            let mut uv = Vec2::ZERO;
            for f in &mesh.faces {
                if let Some(pos) = f.verts.iter().position(|v| *v as usize == i) {
                    uv = f.uvs.get(pos).copied().unwrap_or(Vec2::ZERO);
                    break;
                }
            }
            uvs.extend_from_slice(&uv.x.to_le_bytes());
            uvs.extend_from_slice(&uv.y.to_le_bytes());
        }
        let mut indices: Vec<u32> = Vec::new();
        for f in &mesh.faces {
            if f.verts.len() < 3 {
                continue;
            }
            for i in 1..f.verts.len() - 1 {
                indices.push(f.verts[0]);
                indices.push(f.verts[i]);
                indices.push(f.verts[i + 1]);
            }
        }
        if indices.is_empty() {
            continue;
        }
        let count = mesh.positions.len();
        let pos_view = add_view(&mut bin, &mut views, &positions, 34962);
        let nrm_view = add_view(&mut bin, &mut views, &normals, 34962);
        let uv_view = add_view(&mut bin, &mut views, &uvs, 34962);
        let idx_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let idx_view = add_view(&mut bin, &mut views, &idx_bytes, 34963);

        accessors.push(json!({
            "bufferView": pos_view, "componentType": 5126, "count": count, "type": "VEC3",
            "min": min, "max": max
        }));
        let pos_acc = accessors.len() - 1;
        accessors.push(json!({
            "bufferView": nrm_view, "componentType": 5126, "count": count, "type": "VEC3"
        }));
        let nrm_acc = accessors.len() - 1;
        accessors.push(json!({
            "bufferView": uv_view, "componentType": 5126, "count": count, "type": "VEC2"
        }));
        let uv_acc = accessors.len() - 1;
        accessors.push(json!({
            "bufferView": idx_view, "componentType": 5125, "count": indices.len(), "type": "SCALAR"
        }));
        let idx_acc = accessors.len() - 1;

        json_meshes.push(json!({
            "name": name,
            "primitives": [{
                "attributes": { "POSITION": pos_acc, "NORMAL": nrm_acc, "TEXCOORD_0": uv_acc },
                "indices": idx_acc,
                "material": 0,
                "mode": 4
            }]
        }));
        nodes.push(json!({
            "name": name,
            "mesh": json_meshes.len() - 1
        }));
    }

    if json_meshes.is_empty() {
        return Err(ImportError::Invalid(
            "nothing to export: the scene has no mesh geometry".into(),
        ));
    }

    align4(&mut bin, 0);
    let scene_nodes: Vec<usize> = (0..nodes.len()).collect();
    let doc = json!({
        "asset": { "version": "2.0", "generator": "RoEdit Studio" },
        "scene": 0,
        "scenes": [{ "nodes": scene_nodes }],
        "nodes": nodes,
        "meshes": json_meshes,
        "materials": [{
            "name": "RoEdit Material",
            "pbrMetallicRoughness": {
                "baseColorFactor": base_color,
                "metallicFactor": 0.0,
                "roughnessFactor": 0.55
            }
        }],
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": views,
        "accessors": accessors
    });
    let mut json_bytes = serde_json::to_vec(&doc)?;
    align4(&mut json_bytes, b' ');

    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
    out.extend_from_slice(&bin);
    Ok(out)
}

fn split_glb(bytes: &[u8]) -> Result<(Value, Vec<u8>), ImportError> {
    if bytes.len() < 20 || !bytes.starts_with(MAGIC) {
        return Err(ImportError::Invalid("not a GLB file".into()));
    }
    let length = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    if length > bytes.len() {
        return Err(ImportError::Invalid(
            "GLB header length exceeds file size".into(),
        ));
    }
    let mut offset = 12;
    let mut doc: Option<Value> = None;
    let mut bin: Vec<u8> = Vec::new();
    while offset + 8 <= bytes.len() {
        let chunk_len = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        let chunk_type = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        let start = offset + 8;
        let end = start.saturating_add(chunk_len).min(bytes.len());
        match chunk_type {
            CHUNK_JSON => {
                doc = Some(serde_json::from_slice(&bytes[start..end])?);
            }
            CHUNK_BIN => bin.extend_from_slice(&bytes[start..end]),
            _ => {}
        }
        offset = start + chunk_len;
        if chunk_len == 0 {
            break;
        }
    }
    let doc = doc.ok_or_else(|| ImportError::Invalid("GLB has no JSON chunk".into()))?;
    Ok((doc, bin))
}

fn buffer_view(
    doc: &Value,
    bin: &[u8],
    index: usize,
) -> Result<(usize, usize, usize), ImportError> {
    let view = doc
        .get("bufferViews")
        .and_then(|v| v.get(index))
        .ok_or_else(|| ImportError::Invalid(format!("missing bufferView {index}")))?;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or_else(|| ImportError::Invalid(format!("bufferView {index} has no byteLength")))?
        as usize;
    let stride = view.get("byteStride").and_then(Value::as_u64).unwrap_or(0) as usize;
    if offset + length > bin.len() {
        return Err(ImportError::Invalid(format!(
            "bufferView {index} points past the binary chunk"
        )));
    }
    Ok((offset, length, stride))
}

fn accessor_info(
    doc: &Value,
    index: usize,
) -> Result<(usize, usize, usize, usize, usize), ImportError> {
    // returns (view, byte_offset, count, component_type, components)
    let acc = doc
        .get("accessors")
        .and_then(|v| v.get(index))
        .ok_or_else(|| ImportError::Invalid(format!("missing accessor {index}")))?;
    let view = acc
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| ImportError::Invalid("accessor is not backed by a bufferView".into()))?
        as usize;
    let byte_offset = acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
    let count = acc.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
    let comp = acc
        .get("componentType")
        .and_then(Value::as_u64)
        .unwrap_or(5126) as usize;
    let ty = acc.get("type").and_then(Value::as_str).unwrap_or("VEC3");
    let comps = match ty {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        other => {
            return Err(ImportError::Invalid(format!(
                "unsupported accessor type {other}"
            )))
        }
    };
    Ok((view, byte_offset, count, comp, comps))
}

fn read_floats(doc: &Value, bin: &[u8], index: usize) -> Result<Vec<Vec<f32>>, ImportError> {
    let (view, byte_offset, count, comp, comps) = accessor_info(doc, index)?;
    if comp != 5126 {
        return Err(ImportError::Invalid(format!(
            "accessor {index} uses component type {comp}; only float accessors are supported for vertex data"
        )));
    }
    let (view_offset, view_len, _stride) = buffer_view(doc, bin, view)?;
    let element = comps * 4;
    if count * element + byte_offset > view_len {
        return Err(ImportError::Invalid(format!(
            "accessor {index} overruns its bufferView"
        )));
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let base = view_offset + byte_offset + i * element;
        let mut values = Vec::with_capacity(comps);
        for c in 0..comps {
            let o = base + c * 4;
            values.push(f32::from_le_bytes([
                bin[o],
                bin[o + 1],
                bin[o + 2],
                bin[o + 3],
            ]));
        }
        out.push(values);
    }
    Ok(out)
}

fn read_indices(doc: &Value, bin: &[u8], index: usize) -> Result<Vec<u32>, ImportError> {
    let (view, byte_offset, count, comp, comps) = accessor_info(doc, index)?;
    if comps != 1 {
        return Err(ImportError::Invalid("index accessor must be SCALAR".into()));
    }
    let size = match comp {
        5121 => 1,
        5123 => 2,
        5125 => 4,
        other => {
            return Err(ImportError::Invalid(format!(
                "unsupported index component type {other}"
            )))
        }
    };
    let (view_offset, view_len, _stride) = buffer_view(doc, bin, view)?;
    if count * size + byte_offset > view_len {
        return Err(ImportError::Invalid(format!(
            "accessor {index} overruns its bufferView"
        )));
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = view_offset + byte_offset + i * size;
        let v = match size {
            1 => bin[o] as u32,
            2 => u16::from_le_bytes([bin[o], bin[o + 1]]) as u32,
            _ => u32::from_le_bytes([bin[o], bin[o + 1], bin[o + 2], bin[o + 3]]),
        };
        out.push(v);
    }
    Ok(out)
}

fn node_matrix(node: &Value) -> roedit_core::math::Mat4 {
    if let Some(m) = node.get("matrix").and_then(Value::as_array) {
        if m.len() == 16 {
            let mut vals = [0.0f32; 16];
            for (i, v) in m.iter().enumerate() {
                vals[i] = v.as_f64().unwrap_or(0.0) as f32;
            }
            // glTF stores column-major, same as glam.
            return roedit_core::math::Mat4::from_cols_array(&vals);
        }
    }
    let t = node.get("translation").and_then(Value::as_array);
    let r = node.get("rotation").and_then(Value::as_array);
    let s = node.get("scale").and_then(Value::as_array);
    let translation = Vec3::new(
        t.and_then(|a| a.first())
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32,
        t.and_then(|a| a.get(1))
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32,
        t.and_then(|a| a.get(2))
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32,
    );
    let rotation = Quat::from_xyzw(
        r.and_then(|a| a.first())
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32,
        r.and_then(|a| a.get(1))
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32,
        r.and_then(|a| a.get(2))
            .and_then(Value::as_f64)
            .unwrap_or(0.0) as f32,
        r.and_then(|a| a.get(3))
            .and_then(Value::as_f64)
            .unwrap_or(1.0) as f32,
    );
    let scale = Vec3::new(
        s.and_then(|a| a.first())
            .and_then(Value::as_f64)
            .unwrap_or(1.0) as f32,
        s.and_then(|a| a.get(1))
            .and_then(Value::as_f64)
            .unwrap_or(1.0) as f32,
        s.and_then(|a| a.get(2))
            .and_then(Value::as_f64)
            .unwrap_or(1.0) as f32,
    );
    roedit_core::math::Mat4::from_scale_rotation_translation(scale, rotation, translation)
}

/// Imports every mesh referenced by a node in a GLB.
pub fn import_glb(bytes: &[u8]) -> Result<Vec<(String, Mesh)>, ImportError> {
    let (doc, bin) = split_glb(bytes)?;
    let nodes = doc
        .get("nodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<(String, Mesh)> = Vec::new();
    for (node_index, node) in nodes.iter().enumerate() {
        let Some(mesh_index) = node.get("mesh").and_then(Value::as_u64) else {
            continue;
        };
        let Some(mesh_doc) = doc.get("meshes").and_then(|m| m.get(mesh_index as usize)) else {
            continue;
        };
        let name = mesh_doc
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("Mesh {}", node_index + 1));
        let transform = node_matrix(node);
        let normal_matrix = roedit_core::math::Mat4::from_mat3(
            roedit_core::math::Mat3::from_mat4(transform)
                .inverse()
                .transpose(),
        );
        let primitives = mesh_doc
            .get("primitives")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut mesh = Mesh::new(&name);
        let mut has_any = false;
        for prim in primitives {
            let Some(attributes) = prim.get("attributes") else {
                continue;
            };
            let Some(pos_acc) = attributes.get("POSITION").and_then(Value::as_u64) else {
                continue;
            };
            let positions = read_floats(&doc, &bin, pos_acc as usize)?;
            let normals = attributes
                .get("NORMAL")
                .and_then(Value::as_u64)
                .map(|i| read_floats(&doc, &bin, i as usize))
                .transpose()?;
            let uvs = attributes
                .get("TEXCOORD_0")
                .and_then(Value::as_u64)
                .map(|i| read_floats(&doc, &bin, i as usize))
                .transpose()?;
            let base = mesh.positions.len() as u32;
            for p in &positions {
                if p.len() < 3 {
                    continue;
                }
                let local = transform.transform_point3(Vec3::new(p[0], p[1], p[2]));
                mesh.add_vertex(local);
            }
            if let Some(normals) = &normals {
                for n in normals {
                    if n.len() >= 3 {
                        let local = normal_matrix
                            .transform_vector3(Vec3::new(n[0], n[1], n[2]))
                            .normalize_or_zero();
                        mesh.normals.push(local);
                    }
                }
            }
            let uvs = uvs.unwrap_or_default();
            let indices = prim
                .get("indices")
                .and_then(Value::as_u64)
                .map(|i| read_indices(&doc, &bin, i as usize))
                .transpose()?
                .unwrap_or_else(|| (0..positions.len() as u32).collect());
            let material = prim.get("material").and_then(Value::as_u64).unwrap_or(0) as u16;
            for tri in indices.chunks(3) {
                if tri.len() < 3 {
                    continue;
                }
                let verts: Vec<u32> = tri.iter().map(|i| base + *i).collect();
                let uv: Vec<Vec2> = tri
                    .iter()
                    .map(|i| {
                        uvs.get(*i as usize)
                            .filter(|v| v.len() >= 2)
                            .map(|v| Vec2::new(v[0], 1.0 - v[1]))
                            .unwrap_or(Vec2::ZERO)
                    })
                    .collect();
                let mut face = Face::with_uvs(verts, uv);
                face.material = material;
                mesh.add_face(face);
            }
            has_any = true;
        }
        if has_any && !mesh.faces.is_empty() {
            if mesh.normals.len() != mesh.positions.len() {
                mesh.recompute_normals();
            }
            out.push((name, mesh));
        }
    }
    if out.is_empty() {
        return Err(ImportError::Invalid(
            "the GLB contains no triangle meshes".into(),
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use roedit_mesh::primitives;

    #[test]
    fn glb_roundtrip() {
        let cube = primitives::cube(Vec3::splat(1.0));
        let bytes = export_glb(&[("Cube".to_string(), cube)], [1.0, 0.0, 0.0, 1.0]).unwrap();
        assert!(bytes.starts_with(b"glTF"));
        let back = import_glb(&bytes).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].0, "Cube");
        // Fan triangulation turns 6 quads into 12 triangles.
        assert_eq!(back[0].1.faces.len(), 12);
        let b = back[0].1.bounds();
        assert!((b.size() - Vec3::splat(1.0)).length() < 1e-4);
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(import_glb(b"not a glb").is_err());
    }

    #[test]
    fn rejects_empty_scene() {
        let empty = Mesh::new("empty");
        assert!(export_glb(&[("empty".to_string(), empty)], [1.0; 4]).is_err());
    }

    #[test]
    fn node_transform_is_applied() {
        let mut mesh = Mesh::new("tri");
        mesh.add_vertex(Vec3::ZERO);
        mesh.add_vertex(Vec3::X);
        mesh.add_vertex(Vec3::Y);
        mesh.add_face(Face::new(vec![0, 1, 2]));
        let bytes = export_glb(&[("t".to_string(), mesh)], [1.0; 4]).unwrap();
        let back = import_glb(&bytes).unwrap();
        assert_eq!(back[0].1.positions.len(), 3);
        assert_eq!(back[0].1.faces.len(), 1);
    }
}
