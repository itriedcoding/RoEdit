//! Region modeling operators.
//!
//! All operators work on the n-gon mesh in `roedit_core`, preserve per-corner
//! UVs where it makes sense, keep material assignments, and recompute normals.
//! Each one has unit tests at the bottom of the file.

use crate::{edge_key, signed_volume, Axis, Mesh, MeshError, Vec2, Vec3};
use roedit_core::math::Mat4;
use roedit_core::mesh::Face;
use std::collections::{BTreeSet, HashMap, HashSet};

fn validate_faces(mesh: &Mesh, faces: &[usize]) -> Result<(), MeshError> {
    if faces.is_empty() {
        return Err(MeshError::EmptySelection);
    }
    for &f in faces {
        if f >= mesh.faces.len() {
            return Err(MeshError::FaceOutOfRange(f));
        }
    }
    Ok(())
}

/// Directed boundary edges of a face region (each edge appears once, in the
/// winding of the face that owns it).
pub fn region_boundary(mesh: &Mesh, faces: &[usize]) -> Vec<(u32, u32)> {
    let mut counts: HashMap<(u32, u32), usize> = HashMap::new();
    for &fi in faces {
        let f = &mesh.faces[fi];
        for i in 0..f.verts.len() {
            let key = edge_key(f.verts[i], f.verts[(i + 1) % f.verts.len()]);
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    let mut out = Vec::new();
    for &fi in faces {
        let f = &mesh.faces[fi];
        for i in 0..f.verts.len() {
            let a = f.verts[i];
            let b = f.verts[(i + 1) % f.verts.len()];
            if counts.get(&edge_key(a, b)).copied().unwrap_or(0) == 1 {
                out.push((a, b));
            }
        }
    }
    out
}

/// Averaged (area weighted) normal of a face region.
pub fn selection_normal(mesh: &Mesh, faces: &[usize]) -> Vec3 {
    let mut n = Vec3::ZERO;
    for &fi in faces {
        if let Some(f) = mesh.faces.get(fi) {
            n += f.normal(&mesh.positions) * f.area(&mesh.positions).max(1e-6);
        }
    }
    n.normalize_or_zero()
}

fn average_uv(uvs: &[Vec2]) -> Vec2 {
    if uvs.is_empty() {
        return Vec2::ZERO;
    }
    uvs.iter().copied().sum::<Vec2>() / uvs.len() as f32
}

fn quad_uvs() -> Vec<Vec2> {
    vec![
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ]
}

// ---------------------------------------------------------------- transforms

pub fn translate_vertices(mesh: &mut Mesh, verts: &BTreeSet<u32>, delta: Vec3) {
    for v in verts {
        if let Some(p) = mesh.positions.get_mut(*v as usize) {
            *p += delta;
        }
    }
    mesh.recompute_normals();
}

pub fn transform_vertices(mesh: &mut Mesh, verts: &BTreeSet<u32>, matrix: Mat4) {
    for v in verts {
        if let Some(p) = mesh.positions.get_mut(*v as usize) {
            *p = matrix.transform_point3(*p);
        }
    }
    mesh.recompute_normals();
}

// ------------------------------------------------------------------ extrude

pub fn extrude_faces(
    mesh: &mut Mesh,
    faces: &[usize],
    distance: f32,
) -> Result<Vec<usize>, MeshError> {
    validate_faces(mesh, faces)?;
    let mut normal = selection_normal(mesh, faces);
    if normal.length_squared() < 1e-8 {
        normal = Vec3::Y;
    }
    extrude_faces_along(mesh, faces, normal * distance)
}

/// Extrudes a region by an explicit offset and stitches side walls along the
/// region boundary.
pub fn extrude_faces_along(
    mesh: &mut Mesh,
    faces: &[usize],
    offset: Vec3,
) -> Result<Vec<usize>, MeshError> {
    validate_faces(mesh, faces)?;
    let boundary = region_boundary(mesh, faces);
    let material = mesh.faces[faces[0]].material;

    let mut region_verts: BTreeSet<u32> = BTreeSet::new();
    for &fi in faces {
        region_verts.extend(mesh.faces[fi].verts.iter().copied());
    }
    let mut remap: HashMap<u32, u32> = HashMap::new();
    for v in region_verts {
        let p = mesh.positions[v as usize] + offset;
        let nv = mesh.add_vertex(p);
        remap.insert(v, nv);
    }
    for &fi in faces {
        let f = &mut mesh.faces[fi];
        for v in &mut f.verts {
            *v = remap[v];
        }
    }
    let mut created = Vec::new();
    for (a, b) in boundary {
        let a2 = remap[&a];
        let b2 = remap[&b];
        let mut q = Face::with_uvs(vec![a, b, b2, a2], quad_uvs());
        q.material = material;
        created.push(mesh.add_face(q));
    }
    mesh.recompute_normals();
    Ok(created)
}

// -------------------------------------------------------------------- inset

/// Insets each selected face independently: the original face becomes the inner
/// face and a ring of quads is added around it.
pub fn inset_faces(
    mesh: &mut Mesh,
    faces: &[usize],
    amount: f32,
    depth: f32,
) -> Result<Vec<usize>, MeshError> {
    validate_faces(mesh, faces)?;
    let amount = amount.max(0.0);
    let mut rings = Vec::new();
    for &fi in faces {
        let face = mesh.faces[fi].clone();
        let n = face.verts.len();
        if n < 3 {
            continue;
        }
        let centroid = face.centroid(&mesh.positions);
        let normal = face.normal(&mesh.positions);
        let mut inner = Vec::with_capacity(n);
        for &v in &face.verts {
            let p = mesh.positions[v as usize];
            let mut lateral = centroid - p;
            lateral -= normal * lateral.dot(normal);
            let dir = lateral.normalize_or_zero();
            inner.push(mesh.add_vertex(p + dir * amount + normal * depth));
        }
        for i in 0..n {
            let j = (i + 1) % n;
            let uvs = vec![face.uvs[i], face.uvs[j], face.uvs[j], face.uvs[i]];
            let mut q = Face::with_uvs(vec![face.verts[i], face.verts[j], inner[j], inner[i]], uvs);
            q.material = face.material;
            q.smooth = face.smooth;
            rings.push(mesh.add_face(q));
        }
        // Replace the original face with the inset face.
        let mut updated = Face::with_uvs(inner, face.uvs.clone());
        updated.material = face.material;
        updated.smooth = face.smooth;
        mesh.faces[fi] = updated;
    }
    mesh.recompute_normals();
    Ok(rings)
}

/// Chamfer-style bevel of a face border. Each segment is one inset pass, which
/// produces a rounded border band on the selected faces.
pub fn bevel_faces(
    mesh: &mut Mesh,
    faces: &[usize],
    width: f32,
    segments: u32,
) -> Result<usize, MeshError> {
    validate_faces(mesh, faces)?;
    let segments = segments.clamp(1, 8);
    // Each pass replaces the selected faces with their inner face and adds a
    // ring, so repeating on the same faces builds a rounded border band.
    let mut created = 0;
    for _ in 0..segments {
        let rings = inset_faces(mesh, faces, width / segments as f32, 0.0)?;
        created += rings.len();
    }
    Ok(created)
}

// ----------------------------------------------------------------- subdivide

/// Midpoint subdivision of the selected faces into quads (shared edges get a
/// single midpoint, so the result stays watertight).
pub fn subdivide_faces(
    mesh: &mut Mesh,
    faces: &[usize],
    cuts: u32,
) -> Result<Vec<usize>, MeshError> {
    validate_faces(mesh, faces)?;
    let levels = cuts.clamp(1, 6);
    let mut current: Vec<usize> = faces.to_vec();
    let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
    for _ in 0..levels {
        let mut next = Vec::new();
        for fi in current {
            let face = mesh.faces[fi].clone();
            let n = face.verts.len();
            if n < 3 {
                continue;
            }
            let center = mesh.add_vertex(face.centroid(&mesh.positions));
            let center_uv = average_uv(&face.uvs);
            let mut mids = Vec::with_capacity(n);
            for i in 0..n {
                let a = face.verts[i];
                let b = face.verts[(i + 1) % n];
                let key = edge_key(a, b);
                let m = match midpoints.get(&key) {
                    Some(m) => *m,
                    None => {
                        let pa = mesh.positions[a as usize];
                        let pb = mesh.positions[b as usize];
                        let m = mesh.add_vertex((pa + pb) * 0.5);
                        midpoints.insert(key, m);
                        m
                    }
                };
                mids.push(m);
            }
            let mut quads: Vec<Face> = Vec::with_capacity(n);
            for i in 0..n {
                let prev = (i + n - 1) % n;
                let uvs = vec![
                    face.uvs[i],
                    (face.uvs[i] + face.uvs[(i + 1) % n]) * 0.5,
                    center_uv,
                    (face.uvs[prev] + face.uvs[i]) * 0.5,
                ];
                let mut q = Face::with_uvs(vec![face.verts[i], mids[i], center, mids[prev]], uvs);
                q.material = face.material;
                q.smooth = face.smooth;
                quads.push(q);
            }
            let first = quads.remove(0);
            mesh.faces[fi] = first;
            next.push(fi);
            for q in quads {
                next.push(mesh.add_face(q));
            }
        }
        current = next;
    }
    mesh.recompute_normals();
    Ok(current)
}

fn find_edge_index(verts: &[u32], key: (u32, u32)) -> Option<usize> {
    (0..verts.len()).find(|i| edge_key(verts[*i], verts[(*i + 1) % verts.len()]) == key)
}

/// Cuts a new edge loop through the quad ring containing `face`/`edge`.
/// Returns the number of faces created.
pub fn loop_cut(mesh: &mut Mesh, face: usize, edge: usize, t: f32) -> Result<usize, MeshError> {
    if mesh.faces.get(face).is_none() {
        return Err(MeshError::FaceOutOfRange(face));
    }
    if mesh.faces[face].verts.len() != 4 {
        return Err(MeshError::NotAQuad(face));
    }
    let t = t.clamp(0.02, 0.98);
    let edge_faces = mesh.edge_faces();
    let mut visited: Vec<(usize, usize)> = Vec::new();
    let mut queue: Vec<(usize, usize)> = vec![(face, edge % 4)];
    while let Some((f, i)) = queue.pop() {
        if visited.iter().any(|(vf, _)| *vf == f) {
            continue;
        }
        visited.push((f, i));
        // The loop continues through the cut edges themselves: the neighbour
        // across a cut edge is cut at that same shared edge.
        for k in [i % 4, (i + 2) % 4] {
            let verts = &mesh.faces[f].verts;
            let key = edge_key(verts[k], verts[(k + 1) % 4]);
            let Some(fs) = edge_faces.get(&key) else {
                continue;
            };
            if fs.len() != 2 {
                continue;
            }
            let nf = if fs[0] == f { fs[1] } else { fs[0] };
            if nf == f || mesh.faces[nf].verts.len() != 4 {
                continue;
            }
            if visited.iter().any(|(vf, _)| *vf == nf) {
                continue;
            }
            if let Some(ni) = find_edge_index(&mesh.faces[nf].verts, key) {
                queue.push((nf, ni));
            }
        }
    }

    let mut splits: HashMap<(u32, u32), u32> = HashMap::new();
    let mut created = 0;
    for (f, i) in visited {
        let verts = mesh.faces[f].verts.clone();
        let uvs = mesh.faces[f].uvs.clone();
        if verts.len() != 4 || uvs.len() != 4 {
            continue;
        }
        let ia = i % 4;
        let ib = (ia + 2) % 4;
        let split =
            |mesh: &mut Mesh, splits: &mut HashMap<(u32, u32), u32>, a: u32, b: u32| -> u32 {
                let key = edge_key(a, b);
                if let Some(v) = splits.get(&key) {
                    return *v;
                }
                let pa = mesh.positions[key.0 as usize];
                let pb = mesh.positions[key.1 as usize];
                let v = mesh.add_vertex(pa.lerp(pb, t));
                splits.insert(key, v);
                v
            };
        let na = split(mesh, &mut splits, verts[ia], verts[(ia + 1) % 4]);
        let nb = split(mesh, &mut splits, verts[ib], verts[(ib + 1) % 4]);
        let ta = if verts[ia] < verts[(ia + 1) % 4] {
            t
        } else {
            1.0 - t
        };
        let tb = if verts[ib] < verts[(ib + 1) % 4] {
            t
        } else {
            1.0 - t
        };
        let uva = uvs[ia].lerp(uvs[(ia + 1) % 4], ta);
        let uvb = uvs[ib].lerp(uvs[(ib + 1) % 4], tb);
        let material = mesh.faces[f].material;
        let smooth = mesh.faces[f].smooth;

        let mut q1 = Face::with_uvs(
            vec![verts[ia], na, nb, verts[(ia + 3) % 4]],
            vec![uvs[ia], uva, uvb, uvs[(ia + 3) % 4]],
        );
        q1.material = material;
        q1.smooth = smooth;
        let mut q2 = Face::with_uvs(
            vec![na, verts[(ia + 1) % 4], verts[(ia + 2) % 4], nb],
            vec![uva, uvs[(ia + 1) % 4], uvs[(ia + 2) % 4], uvb],
        );
        q2.material = material;
        q2.smooth = smooth;
        mesh.faces[f] = q1;
        mesh.add_face(q2);
        created += 1;
    }
    mesh.recompute_normals();
    Ok(created)
}

// ----------------------------------------------------------------- modifiers

/// Thickens an open surface into a closed shell.
pub fn solidify(mesh: &mut Mesh, thickness: f32) -> Result<usize, MeshError> {
    if mesh.faces.is_empty() {
        return Err(MeshError::NoGeometry);
    }
    let thickness = thickness.abs();
    mesh.recompute_normals();
    let edge_faces = mesh.edge_faces();
    let mut boundary: Vec<(u32, u32)> = Vec::new();
    for f in &mesh.faces {
        for i in 0..f.verts.len() {
            let a = f.verts[i];
            let b = f.verts[(i + 1) % f.verts.len()];
            if edge_faces
                .get(&edge_key(a, b))
                .map(|v| v.len())
                .unwrap_or(0)
                == 1
            {
                boundary.push((a, b));
            }
        }
    }
    let original_count = mesh.positions.len();
    let normals = mesh.normals.clone();
    let mut inner = Vec::with_capacity(original_count);
    for i in 0..original_count {
        let p = mesh.positions[i] - normals.get(i).copied().unwrap_or(Vec3::Y) * thickness;
        inner.push(mesh.add_vertex(p));
    }
    let faces = mesh.faces.clone();
    for f in &faces {
        let mut verts: Vec<u32> = f.verts.iter().map(|v| inner[*v as usize]).collect();
        verts.reverse();
        let mut uvs = f.uvs.clone();
        uvs.reverse();
        let mut nf = Face::with_uvs(verts, uvs);
        nf.material = f.material;
        nf.smooth = f.smooth;
        mesh.add_face(nf);
    }
    let mut rim = 0;
    for (a, b) in boundary {
        let mut q = Face::with_uvs(vec![b, a, inner[a as usize], inner[b as usize]], quad_uvs());
        if let Some(f) = mesh.faces.first() {
            q.material = f.material;
        }
        mesh.add_face(q);
        rim += 1;
    }
    mesh.recompute_normals();
    Ok(rim)
}

/// Mirrors geometry across an axis, welding vertices that sit on the plane.
pub fn mirror_modifier(
    mesh: &mut Mesh,
    axis: Axis,
    merge_threshold: f32,
) -> Result<usize, MeshError> {
    if mesh.faces.is_empty() {
        return Err(MeshError::NoGeometry);
    }
    let ai = axis.index();
    let original_count = mesh.positions.len();
    let faces = mesh.faces.clone();
    let mut map = vec![0u32; original_count];
    let mut created = 0;
    for i in 0..original_count {
        let mut p = mesh.positions[i];
        if p[ai].abs() <= merge_threshold {
            p[ai] = 0.0;
            mesh.positions[i] = p;
            map[i] = i as u32;
        } else {
            p[ai] = -p[ai];
            map[i] = mesh.add_vertex(p);
            created += 1;
        }
    }
    for f in faces {
        let mut verts: Vec<u32> = f.verts.iter().map(|v| map[*v as usize]).collect();
        verts.reverse();
        let mut uvs = f.uvs.clone();
        uvs.reverse();
        let mut nf = Face::with_uvs(verts, uvs);
        nf.material = f.material;
        nf.smooth = f.smooth;
        mesh.add_face(nf);
    }
    mesh.recompute_normals();
    Ok(created)
}

/// Repeats the mesh `count` times with a per-step translation and Y rotation.
pub fn array_modifier(
    mesh: &mut Mesh,
    count: u32,
    offset: Vec3,
    rotation_deg: f32,
) -> Result<usize, MeshError> {
    if mesh.faces.is_empty() {
        return Err(MeshError::NoGeometry);
    }
    let count = count.clamp(1, 512);
    let original_positions = mesh.positions.clone();
    let faces = mesh.faces.clone();
    let mut created = 0;
    for step in 1..count {
        let rotation = Mat4::from_rotation_y(rotation_deg.to_radians() * step as f32);
        let matrix = Mat4::from_translation(offset * step as f32) * rotation;
        let base = mesh.positions.len() as u32;
        for p in &original_positions {
            mesh.positions.push(matrix.transform_point3(*p));
        }
        for f in &faces {
            let verts: Vec<u32> = f.verts.iter().map(|v| base + *v).collect();
            let mut nf = Face::with_uvs(verts, f.uvs.clone());
            nf.material = f.material;
            nf.smooth = f.smooth;
            mesh.add_face(nf);
            created += 1;
        }
    }
    mesh.recompute_normals();
    Ok(created)
}

fn cluster_vertices(mesh: &Mesh, cell: f32) -> (Vec<Vec3>, Vec<u32>) {
    let cell = cell.max(1e-6);
    let mut map: HashMap<(i64, i64, i64), usize> = HashMap::new();
    let mut sums: Vec<Vec3> = Vec::new();
    let mut counts: Vec<u32> = Vec::new();
    let mut remap = Vec::with_capacity(mesh.positions.len());
    for p in &mesh.positions {
        let key = (
            (p.x / cell).floor() as i64,
            (p.y / cell).floor() as i64,
            (p.z / cell).floor() as i64,
        );
        let idx = match map.get(&key) {
            Some(i) => *i,
            None => {
                sums.push(Vec3::ZERO);
                counts.push(0);
                let i = sums.len() - 1;
                map.insert(key, i);
                i
            }
        };
        sums[idx] += *p;
        counts[idx] += 1;
        remap.push(idx as u32);
    }
    let reps = sums
        .iter()
        .zip(counts.iter())
        .map(|(s, c)| *s / (*c).max(1) as f32)
        .collect();
    (reps, remap)
}

/// Vertex-clustering decimation. Returns the achieved vertex ratio.
pub fn decimate(mesh: &mut Mesh, target_ratio: f32) -> Result<f32, MeshError> {
    let original_vertices = mesh.positions.len();
    if original_vertices < 8 || mesh.faces.is_empty() {
        return Ok(1.0);
    }
    let ratio = target_ratio.clamp(0.02, 1.0);
    if ratio >= 0.999 {
        return Ok(1.0);
    }
    let target = ((mesh.positions.len() as f32 * ratio).round() as usize).max(4);
    let diagonal = mesh.bounds().size().length().max(1e-5);
    let mut cell = diagonal / (target as f32).cbrt().max(1.0);
    let mut chosen: Option<(Vec<Vec3>, Vec<u32>)> = None;
    for _ in 0..16 {
        let (reps, remap) = cluster_vertices(mesh, cell);
        let reached = reps.len();
        chosen = Some((reps, remap));
        if reached <= target {
            break;
        }
        cell *= 1.35;
    }
    let Some((reps, remap)) = chosen else {
        return Ok(1.0);
    };
    let original_faces = mesh.faces.clone();
    let mut seen: HashSet<Vec<u32>> = HashSet::new();
    let mut faces = Vec::new();
    for f in original_faces {
        let mut verts: Vec<u32> = f.verts.iter().map(|v| remap[*v as usize]).collect();
        // drop repeated neighbours
        let mut deduped: Vec<u32> = Vec::with_capacity(verts.len());
        for v in verts.drain(..) {
            if deduped.last().copied() != Some(v) && deduped.first().copied() != Some(v) {
                deduped.push(v);
            }
        }
        if deduped.len() < 3 {
            continue;
        }
        let mut key = deduped.clone();
        key.sort_unstable();
        if !seen.insert(key) {
            continue;
        }
        let mut uvs = f.uvs.clone();
        while uvs.len() < deduped.len() {
            uvs.push(Vec2::ZERO);
        }
        uvs.truncate(deduped.len());
        let mut nf = Face::with_uvs(deduped, uvs);
        nf.material = f.material;
        nf.smooth = f.smooth;
        faces.push(nf);
    }
    if faces.is_empty() {
        return Ok(1.0);
    }
    mesh.positions = reps;
    mesh.faces = faces;
    mesh.normals.clear();
    mesh.recompute_normals();
    Ok(mesh.positions.len() as f32 / original_vertices as f32)
}

pub fn triangulate(mesh: &mut Mesh) -> usize {
    let before = mesh.faces.len();
    let mut faces = Vec::with_capacity(mesh.faces.len());
    for f in &mesh.faces {
        if f.verts.len() <= 3 {
            faces.push(f.clone());
            continue;
        }
        for i in 1..f.verts.len() - 1 {
            let mut tri = Face::with_uvs(
                vec![f.verts[0], f.verts[i], f.verts[i + 1]],
                vec![f.uvs[0], f.uvs[i], f.uvs[i + 1]],
            );
            tri.material = f.material;
            tri.smooth = f.smooth;
            faces.push(tri);
        }
    }
    mesh.faces = faces;
    mesh.recompute_normals();
    mesh.faces.len().saturating_sub(before)
}

/// Welds vertices closer than `tolerance`. Returns the number removed.
pub fn merge_by_distance(mesh: &mut Mesh, tolerance: f32) -> usize {
    let removed = mesh.weld(tolerance.max(1e-6));
    mesh.recompute_normals();
    removed
}

pub fn delete_faces(mesh: &mut Mesh, faces: &[usize]) -> usize {
    let mut sorted: Vec<usize> = faces.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut removed = 0;
    for f in sorted.iter().rev() {
        if *f < mesh.faces.len() {
            mesh.faces.remove(*f);
            removed += 1;
        }
    }
    mesh.remove_unused_vertices();
    removed
}

pub fn delete_vertices(mesh: &mut Mesh, verts: &BTreeSet<u32>) -> usize {
    let before = mesh.faces.len();
    mesh.faces
        .retain(|f| !f.verts.iter().any(|v| verts.contains(v)));
    let removed = before - mesh.faces.len();
    mesh.remove_unused_vertices();
    removed
}

/// Laplacian smoothing. `None` smooths every vertex.
pub fn smooth_laplacian(
    mesh: &mut Mesh,
    verts: Option<&BTreeSet<u32>>,
    iterations: u32,
    factor: f32,
) -> usize {
    let factor = factor.clamp(0.0, 1.0);
    let iterations = iterations.clamp(1, 50);
    let mut adjacency: HashMap<u32, Vec<u32>> = HashMap::new();
    for f in &mesh.faces {
        for i in 0..f.verts.len() {
            let a = f.verts[i];
            let b = f.verts[(i + 1) % f.verts.len()];
            adjacency.entry(a).or_default().push(b);
            adjacency.entry(b).or_default().push(a);
        }
    }
    let targets: Vec<u32> = match verts {
        Some(v) => v.iter().copied().collect(),
        None => (0..mesh.positions.len() as u32).collect(),
    };
    let mut touched = 0;
    for _ in 0..iterations {
        let mut updates: Vec<(u32, Vec3)> = Vec::new();
        for v in &targets {
            let Some(neigh) = adjacency.get(v) else {
                continue;
            };
            if neigh.is_empty() {
                continue;
            }
            let mut avg = Vec3::ZERO;
            for n in neigh {
                avg += mesh.positions[*n as usize];
            }
            avg /= neigh.len() as f32;
            let current = mesh.positions[*v as usize];
            updates.push((*v, current.lerp(avg, factor)));
        }
        for (v, p) in updates {
            mesh.positions[v as usize] = p;
            touched += 1;
        }
    }
    mesh.recompute_normals();
    touched
}

/// Flips winding when the mesh is inside-out. Returns true when it flipped.
pub fn recalc_normals_outside(mesh: &mut Mesh) -> bool {
    let flipped = signed_volume(mesh) < 0.0;
    if flipped {
        mesh.flip();
    }
    mesh.recompute_normals();
    flipped
}

/// Deletes geometry on the discard side of an axis and mirrors the remainder,
/// producing a symmetric mesh. Expects a half model.
pub fn symmetrize(mesh: &mut Mesh, axis: Axis, keep_positive: bool) -> Result<usize, MeshError> {
    let ai = axis.index();
    let sign = if keep_positive { 1.0 } else { -1.0 };
    let remove: Vec<usize> = mesh
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.centroid(&mesh.positions)[ai] * sign < -1e-5)
        .map(|(i, _)| i)
        .collect();
    delete_faces(mesh, &remove);
    let created = mirror_modifier(mesh, axis, 1e-4)?;
    // Faces that cross the mirror plane map onto themselves; welding and
    // de-duplicating leaves a single clean surface.
    merge_by_distance(mesh, 1e-4);
    dedupe_faces(mesh);
    Ok(created)
}

/// Removes faces that reference the same set of vertices (coincident duplicates).
pub fn dedupe_faces(mesh: &mut Mesh) -> usize {
    let before = mesh.faces.len();
    let mut seen: HashSet<Vec<u32>> = HashSet::with_capacity(mesh.faces.len());
    mesh.faces.retain(|f| {
        let mut key = f.verts.clone();
        key.sort_unstable();
        seen.insert(key)
    });
    mesh.remove_unused_vertices();
    before - mesh.faces.len()
}

pub fn flip_faces(mesh: &mut Mesh, faces: &[usize]) {
    for &fi in faces {
        if let Some(f) = mesh.faces.get_mut(fi) {
            f.verts.reverse();
            f.uvs.reverse();
        }
    }
    mesh.recompute_normals();
}

pub fn set_face_material(mesh: &mut Mesh, faces: &[usize], material: u16) -> usize {
    let mut changed = 0;
    for &fi in faces {
        if let Some(f) = mesh.faces.get_mut(fi) {
            f.material = material;
            changed += 1;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives;

    #[test]
    fn extrude_closed_region_keeps_solid() {
        let mut m = primitives::cube(Vec3::ONE);
        let faces = vec![5]; // +Y
        let created = extrude_faces(&mut m, &faces, 1.0).unwrap();
        assert_eq!(created.len(), 4);
        assert_eq!(m.faces.len(), 10);
        assert_eq!(m.validate().boundary_edges, 0);
        assert!(
            (signed_volume(&m) - 2.0).abs() < 1e-3,
            "{}",
            signed_volume(&m)
        );
    }

    #[test]
    fn extrude_whole_mesh_has_no_sides() {
        let mut m = primitives::cube(Vec3::ONE);
        let faces: Vec<usize> = (0..m.faces.len()).collect();
        let created = extrude_faces(&mut m, &faces, 0.5).unwrap();
        assert!(created.is_empty());
        assert!((m.signed_volume_abs() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn inset_adds_ring() {
        let mut m = primitives::cube(Vec3::ONE);
        let rings = inset_faces(&mut m, &[5], 0.25, 0.0).unwrap();
        assert_eq!(rings.len(), 4);
        assert_eq!(m.faces.len(), 10);
        assert_eq!(m.validate().boundary_edges, 0);
        let area_before = 6.0;
        assert!((crate::surface_area(&m) - area_before).abs() < 1e-3);
    }

    #[test]
    fn bevel_creates_multiple_rings() {
        let mut m = primitives::cube(Vec3::ONE);
        let created = bevel_faces(&mut m, &[5], 0.2, 3).unwrap();
        assert_eq!(created, 12);
        assert_eq!(m.faces.len(), 6 + 12);
        assert_eq!(m.validate().boundary_edges, 0);
    }

    #[test]
    fn subdivide_quad_makes_four() {
        let mut m = primitives::grid_plane(1.0, 1);
        let new_faces = subdivide_faces(&mut m, &[0], 1).unwrap();
        assert_eq!(new_faces.len(), 4);
        assert_eq!(m.faces.len(), 4);
        assert_eq!(
            m.validate().boundary_edges,
            8,
            "each outer edge is split by a midpoint"
        );
        // Area is preserved by midpoint subdivision.
        assert!((crate::surface_area(&m) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn subdivide_rejects_bad_selection() {
        let mut m = primitives::grid_plane(1.0, 1);
        assert_eq!(
            subdivide_faces(&mut m, &[99], 1).unwrap_err(),
            MeshError::FaceOutOfRange(99)
        );
        assert_eq!(
            subdivide_faces(&mut m, &[], 1).unwrap_err(),
            MeshError::EmptySelection
        );
    }

    #[test]
    fn loop_cut_splits_the_ring() {
        let mut m = primitives::cylinder(0.5, 2.0, 8, true);
        let before = m.faces.len();
        // Cut across the vertical edges so the loop travels around the tube.
        let created = loop_cut(&mut m, 0, 1, 0.5).unwrap();
        assert_eq!(created, 8, "loop should split the 8 side quads");
        assert_eq!(m.faces.len(), before + 8);
        assert_eq!(m.validate().boundary_edges, 0);
    }

    #[test]
    fn loop_cut_rejects_triangles() {
        let mut m = primitives::cube(Vec3::ONE);
        m.faces.push(Face::new(vec![0, 1, 2]));
        let idx = m.faces.len() - 1;
        assert_eq!(
            loop_cut(&mut m, idx, 0, 0.5).unwrap_err(),
            MeshError::NotAQuad(idx)
        );
    }

    #[test]
    fn solidify_thickens_surface() {
        let mut m = primitives::grid_plane(1.0, 1);
        let area_before = crate::surface_area(&m);
        let rim = solidify(&mut m, 0.1).unwrap();
        assert_eq!(rim, 4);
        assert_eq!(m.positions.len(), 8);
        assert!(crate::surface_area(&m) > area_before);
        assert_eq!(m.validate().boundary_edges, 0, "shell should be closed");
    }

    #[test]
    fn mirror_produces_symmetric_geometry() {
        let mut m = Mesh::new("half");
        let a = m.add_vertex(Vec3::new(0.0, 0.0, 0.0));
        let b = m.add_vertex(Vec3::new(1.0, 0.0, 0.0));
        let c = m.add_vertex(Vec3::new(1.0, 1.0, 0.0));
        let d = m.add_vertex(Vec3::new(0.0, 1.0, 0.0));
        m.add_face(Face::new(vec![a, b, c, d]));
        let created = mirror_modifier(&mut m, Axis::X, 1e-4).unwrap();
        assert_eq!(created, 2, "two off-plane vertices duplicated");
        assert_eq!(m.faces.len(), 2);
        let bounds = m.bounds();
        assert!((bounds.min.x + 1.0).abs() < 1e-5);
        assert!((bounds.max.x - 1.0).abs() < 1e-5);
        // The seam vertices are shared, so the two quads form one strip.
        assert_eq!(m.positions.len(), 6);
    }

    #[test]
    fn array_copies_geometry() {
        let mut m = primitives::cube(Vec3::ONE);
        let created = array_modifier(&mut m, 3, Vec3::new(2.0, 0.0, 0.0), 0.0).unwrap();
        assert_eq!(created, 12);
        assert_eq!(m.faces.len(), 18);
        assert!((m.bounds().max.x - 4.5).abs() < 1e-4);
    }

    #[test]
    fn decimate_reduces_triangles() {
        let mut m = primitives::uv_sphere(1.0, 32, 16);
        let before = m.positions.len();
        let ratio = decimate(&mut m, 0.25).unwrap();
        assert!(ratio < 1.0);
        assert!(
            m.positions.len() < before / 2,
            "{} -> {}",
            before,
            m.positions.len()
        );
        assert_eq!(m.validate().out_of_range_indices, 0);
        assert!(!m.faces.is_empty());
    }

    #[test]
    fn triangulate_splits_ngons() {
        let mut m = primitives::cube(Vec3::ONE);
        triangulate(&mut m);
        assert_eq!(m.faces.len(), 12);
        assert!(m.faces.iter().all(|f| f.verts.len() == 3));
    }

    #[test]
    fn deleting_faces_cleans_vertices() {
        let mut m = primitives::cube(Vec3::ONE);
        let removed = delete_faces(&mut m, &[5]);
        assert_eq!(removed, 1);
        assert_eq!(m.faces.len(), 5);
        assert_eq!(m.validate().unused_vertices, 0);
    }

    #[test]
    fn smoothing_reduces_roughness() {
        let mut m = primitives::grid_plane(2.0, 4);
        // Perturb interior vertices.
        for (i, p) in m.positions.iter_mut().enumerate() {
            if i % 3 == 0 {
                p.y = 0.4;
            }
        }
        let rough = (0..m.positions.len())
            .map(|i| m.positions[i].y.abs())
            .sum::<f32>();
        smooth_laplacian(&mut m, None, 5, 0.5);
        let smooth = (0..m.positions.len())
            .map(|i| m.positions[i].y.abs())
            .sum::<f32>();
        assert!(smooth < rough);
    }

    #[test]
    fn recalc_flips_inside_out_mesh() {
        let mut m = primitives::cube(Vec3::ONE);
        m.flip();
        assert!(signed_volume(&m) < 0.0);
        assert!(recalc_normals_outside(&mut m));
        assert!(signed_volume(&m) > 0.0);
    }

    #[test]
    fn symmetrize_makes_symmetric_mesh() {
        let mut m = primitives::cube(Vec3::ONE);
        symmetrize(&mut m, Axis::X, true).unwrap();
        let b = m.bounds();
        assert!((b.min.x + b.max.x).abs() < 1e-4);
        assert_eq!(m.faces.len(), 6, "duplicate centred faces are removed");
        assert_eq!(m.validate().boundary_edges, 0);
        assert_eq!(m.validate().non_manifold_edges, 0);
        assert!(signed_volume(&m) > 0.0);
    }

    #[test]
    fn merge_and_material_helpers() {
        let mut m = primitives::cube(Vec3::ONE);
        assert_eq!(set_face_material(&mut m, &[0, 1], 3), 2);
        assert_eq!(m.faces[0].material, 3);
        let dup = m.positions[0];
        m.add_vertex(dup);
        assert_eq!(merge_by_distance(&mut m, 1e-4), 1);
    }

    #[test]
    fn region_boundary_of_two_faces_shares_edge() {
        let m = primitives::grid_plane(1.0, 2);
        let boundary = region_boundary(&m, &[0, 1]);
        assert_eq!(boundary.len(), 6, "two quads share one edge");
    }

    trait VolumeAbs {
        fn signed_volume_abs(&self) -> f32;
    }
    impl VolumeAbs for Mesh {
        fn signed_volume_abs(&self) -> f32 {
            signed_volume(self).abs()
        }
    }
}
