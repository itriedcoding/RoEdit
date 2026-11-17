//! Scene graph: entity hierarchy, meshes, materials, lights, camera and sky.
//!
//! Meshes and images are stored behind `Arc` so that undo snapshots and
//! duplicated documents share their buffers instead of copying them.

use crate::camera::Camera;
use crate::ids::{Counter, EntityId, ImageId, LightId, MaterialId, MeshId};
use crate::material::{Environment, Material};
use crate::math::{Aabb, Mat4, Quat, Ray, Vec3, EPSILON};
use crate::mesh::Mesh;
use crate::raster::RasterImage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self::identity()
    }
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    pub fn from_translation(t: Vec3) -> Self {
        Self {
            translation: t,
            ..Self::identity()
        }
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn from_matrix(m: Mat4) -> Self {
        let (translation, rotation, scale) = crate::math::decompose(m);
        Self {
            translation,
            rotation,
            scale,
        }
    }

    pub fn rotation_euler_deg(&self) -> Vec3 {
        let (x, y, z) = self.rotation.to_euler(glam::EulerRot::YXZ);
        Vec3::new(
            crate::math::rad_to_deg(x),
            crate::math::rad_to_deg(y),
            crate::math::rad_to_deg(z),
        )
    }

    pub fn set_rotation_euler_deg(&mut self, e: Vec3) {
        self.rotation = Quat::from_euler(
            glam::EulerRot::YXZ,
            crate::math::deg_to_rad(e.x),
            crate::math::deg_to_rad(e.y),
            crate::math::deg_to_rad(e.z),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightKind {
    Directional,
    Point,
    Spot,
}

impl LightKind {
    pub fn label(self) -> &'static str {
        match self {
            LightKind::Directional => "Directional",
            LightKind::Point => "Point",
            LightKind::Spot => "Spot",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Light {
    pub id: LightId,
    pub name: String,
    pub kind: LightKind,
    pub color: [f32; 3],
    pub intensity: f32,
    pub position: Vec3,
    /// Direction the light points towards (spot/directional).
    pub direction: Vec3,
    pub range: f32,
    pub spot_angle_deg: f32,
    pub spot_softness: f32,
    pub cast_shadows: bool,
    pub enabled: bool,
}

impl Light {
    pub fn new(id: LightId, kind: LightKind) -> Self {
        Self {
            id,
            name: format!("{} Light", kind.label()),
            kind,
            color: [1.0, 0.98, 0.94],
            intensity: 12.0,
            position: Vec3::new(2.0, 3.0, 2.0),
            direction: Vec3::new(-0.5, -1.0, -0.5).normalize(),
            range: 12.0,
            spot_angle_deg: 35.0,
            spot_softness: 0.35,
            cast_shadows: true,
            enabled: true,
        }
    }

    pub fn radiance(&self) -> Vec3 {
        Vec3::from_array(self.color) * self.intensity
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub parent: Option<EntityId>,
    pub children: Vec<EntityId>,
    pub transform: Transform,
    pub mesh: Option<MeshId>,
    pub material: Option<MaterialId>,
    pub light: Option<LightId>,
    pub visible: bool,
    pub locked: bool,
    pub cast_shadows: bool,
    pub receive_shadows: bool,
    pub tags: Vec<String>,
}

impl Entity {
    fn new(id: EntityId, name: impl Into<String>, parent: Option<EntityId>) -> Self {
        Self {
            id,
            name: name.into(),
            parent,
            children: Vec::new(),
            transform: Transform::identity(),
            mesh: None,
            material: None,
            light: None,
            visible: true,
            locked: false,
            cast_shadows: true,
            receive_shadows: true,
            tags: Vec::new(),
        }
    }

    pub fn has_geometry(&self) -> bool {
        self.mesh.is_some()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneHit {
    pub entity: EntityId,
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub face: usize,
    pub triangle: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SceneStats {
    pub entities: usize,
    pub visible_entities: usize,
    pub lights: usize,
    pub unique_meshes: usize,
    pub mesh_instances: usize,
    pub triangles: usize,
    pub vertices: usize,
    pub materials: usize,
    pub images: usize,
    pub image_bytes: usize,
    pub materials_referencing_images: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub entities: HashMap<EntityId, Entity>,
    pub roots: Vec<EntityId>,
    pub meshes: HashMap<MeshId, Arc<Mesh>>,
    pub materials: HashMap<MaterialId, Material>,
    pub images: HashMap<ImageId, Arc<RasterImage>>,
    pub lights: HashMap<LightId, Light>,
    pub camera: Camera,
    pub environment: Environment,
    next_entity: Counter,
    next_mesh: Counter,
    next_material: Counter,
    next_image: Counter,
    next_light: Counter,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new("Untitled Scene")
    }
}

impl Scene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entities: HashMap::new(),
            roots: Vec::new(),
            meshes: HashMap::new(),
            materials: HashMap::new(),
            images: HashMap::new(),
            lights: HashMap::new(),
            camera: Camera::default(),
            environment: Environment::default(),
            next_entity: Counter::default(),
            next_mesh: Counter::default(),
            next_material: Counter::default(),
            next_image: Counter::default(),
            next_light: Counter::default(),
        }
    }

    // ---------------------------------------------------------------- entities

    pub fn add_entity(&mut self, name: impl Into<String>, parent: Option<EntityId>) -> EntityId {
        let id = EntityId(self.next_entity.next());
        let entity = Entity::new(id, name, parent);
        self.entities.insert(id, entity);
        match parent {
            Some(p) if self.entities.contains_key(&p) => {
                self.entities.get_mut(&p).unwrap().children.push(id);
            }
            _ => self.roots.push(id),
        }
        id
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    pub fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    pub fn rename(&mut self, id: EntityId, name: impl Into<String>) {
        if let Some(e) = self.entities.get_mut(&id) {
            e.name = name.into();
        }
    }

    /// Deletes an entity and all descendants. Returns the removed ids.
    pub fn delete_entity(&mut self, id: EntityId) -> Vec<EntityId> {
        let mut removed = Vec::new();
        if !self.entities.contains_key(&id) {
            return removed;
        }
        let parent = self.entities[&id].parent;
        self.collect_subtree(id, &mut removed);
        for rid in &removed {
            self.entities.remove(rid);
        }
        match parent {
            Some(p) => {
                if let Some(e) = self.entities.get_mut(&p) {
                    e.children.retain(|c| *c != id);
                }
            }
            None => self.roots.retain(|c| *c != id),
        }
        removed
    }

    fn collect_subtree(&self, id: EntityId, out: &mut Vec<EntityId>) {
        out.push(id);
        if let Some(e) = self.entities.get(&id) {
            let children = e.children.clone();
            for c in children {
                self.collect_subtree(c, out);
            }
        }
    }

    /// Reparents `id` under `new_parent`, preserving its world transform.
    pub fn reparent(&mut self, id: EntityId, new_parent: Option<EntityId>) -> Result<(), String> {
        if Some(id) == new_parent {
            return Err("An object cannot be its own parent".into());
        }
        if let Some(np) = new_parent {
            if !self.entities.contains_key(&np) {
                return Err("Target parent does not exist".into());
            }
            if self.is_ancestor_of(id, np) {
                return Err("Cannot reparent an object under its own descendant".into());
            }
        }
        if !self.entities.contains_key(&id) {
            return Err("Entity does not exist".into());
        }
        let world = self.world_matrix(id);
        if let Some(old) = self.entities[&id].parent {
            if let Some(e) = self.entities.get_mut(&old) {
                e.children.retain(|c| *c != id);
            }
        } else {
            self.roots.retain(|c| *c != id);
        }
        if let Some(e) = self.entities.get_mut(&id) {
            e.parent = new_parent;
        }
        match new_parent {
            Some(np) => {
                self.entities.get_mut(&np).unwrap().children.push(id);
            }
            None => self.roots.push(id),
        }
        let parent_world = new_parent
            .map(|p| self.world_matrix(p))
            .unwrap_or(Mat4::IDENTITY);
        let local = parent_world.inverse() * world;
        if let Some(e) = self.entities.get_mut(&id) {
            e.transform = Transform::from_matrix(local);
        }
        Ok(())
    }

    /// True when `ancestor` is above `node` in the hierarchy.
    pub fn is_ancestor_of(&self, ancestor: EntityId, node: EntityId) -> bool {
        let mut cur = self.entities.get(&node).and_then(|e| e.parent);
        let mut guard = 0;
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            guard += 1;
            if guard > 4096 {
                break;
            }
            cur = self.entities.get(&c).and_then(|e| e.parent);
        }
        false
    }

    pub fn world_matrix(&self, id: EntityId) -> Mat4 {
        let mut chain: Vec<Mat4> = Vec::new();
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(c) = cur {
            match self.entities.get(&c) {
                Some(e) => chain.push(e.transform.matrix()),
                None => break,
            }
            guard += 1;
            if guard > 4096 {
                break;
            }
            cur = self.entities.get(&c).and_then(|e| e.parent);
        }
        let mut m = Mat4::IDENTITY;
        for t in chain.iter().rev() {
            m *= *t;
        }
        m
    }

    pub fn world_position(&self, id: EntityId) -> Vec3 {
        self.world_matrix(id).w_axis.truncate()
    }

    pub fn set_world_position(&mut self, id: EntityId, world: Vec3) {
        let parent_world = self
            .entities
            .get(&id)
            .and_then(|e| e.parent)
            .map(|p| self.world_matrix(p))
            .unwrap_or(Mat4::IDENTITY);
        let local = parent_world.inverse().transform_point3(world);
        if let Some(e) = self.entities.get_mut(&id) {
            e.transform.translation = local;
        }
    }

    pub fn entity_path(&self, id: EntityId) -> String {
        let mut parts = Vec::new();
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(c) = cur {
            if let Some(e) = self.entities.get(&c) {
                parts.push(e.name.clone());
                cur = e.parent;
            } else {
                break;
            }
            guard += 1;
            if guard > 4096 {
                break;
            }
        }
        parts.reverse();
        parts.join(" / ")
    }

    /// Duplicates an entity subtree. Meshes are shared (instanced).
    pub fn duplicate_entity(&mut self, id: EntityId) -> Option<EntityId> {
        let src = self.entities.get(&id)?.clone();
        let parent = src.parent;
        let new_root = self.add_entity(format!("{} Copy", src.name), parent);
        let mut map: HashMap<EntityId, EntityId> = HashMap::new();
        map.insert(id, new_root);
        self.copy_subtree(id, new_root, &mut map);
        // Lift the root's local transform.
        if let Some(e) = self.entities.get_mut(&new_root) {
            e.transform = src.transform.clone();
            e.transform.translation += Vec3::new(0.35, 0.0, 0.35);
            e.mesh = src.mesh;
            e.material = src.material;
            e.light = None;
            e.tags = src.tags.clone();
            e.cast_shadows = src.cast_shadows;
            e.receive_shadows = src.receive_shadows;
            e.visible = src.visible;
        }
        Some(new_root)
    }

    fn copy_subtree(
        &mut self,
        src: EntityId,
        dst: EntityId,
        map: &mut HashMap<EntityId, EntityId>,
    ) {
        let children = match self.entities.get(&src) {
            Some(e) => e.children.clone(),
            None => return,
        };
        for child in children {
            let child_data = match self.entities.get(&child) {
                Some(c) => c.clone(),
                None => continue,
            };
            let new_id = self.add_entity(child_data.name.clone(), Some(dst));
            map.insert(child, new_id);
            if let Some(e) = self.entities.get_mut(&new_id) {
                e.transform = child_data.transform.clone();
                e.mesh = child_data.mesh;
                e.material = child_data.material;
                e.visible = child_data.visible;
                e.cast_shadows = child_data.cast_shadows;
                e.receive_shadows = child_data.receive_shadows;
                e.tags = child_data.tags.clone();
            }
            self.copy_subtree(child, new_id, map);
        }
    }

    /// Depth-first list of (entity, depth) in hierarchy order.
    pub fn hierarchy(&self) -> Vec<(EntityId, usize)> {
        let mut out = Vec::new();
        for root in &self.roots {
            self.push_hierarchy(*root, 0, &mut out);
        }
        out
    }

    fn push_hierarchy(&self, id: EntityId, depth: usize, out: &mut Vec<(EntityId, usize)>) {
        let Some(e) = self.entities.get(&id) else {
            return;
        };
        out.push((id, depth));
        for c in &e.children {
            self.push_hierarchy(*c, depth + 1, out);
        }
    }

    pub fn selected_material_default(&self) -> MaterialId {
        self.materials
            .keys()
            .copied()
            .next()
            .unwrap_or(MaterialId(0))
    }

    // ------------------------------------------------------------ meshes/mats

    pub fn add_mesh(&mut self, mesh: Mesh) -> MeshId {
        let id = MeshId(self.next_mesh.next() as u32);
        self.meshes.insert(id, Arc::new(mesh));
        id
    }

    pub fn mesh(&self, id: MeshId) -> Option<&Mesh> {
        self.meshes.get(&id).map(|m| m.as_ref())
    }

    /// Copy-on-write access: the mesh is only cloned when it is shared.
    pub fn mesh_mut(&mut self, id: MeshId) -> Option<&mut Mesh> {
        self.meshes.get_mut(&id).map(Arc::make_mut)
    }

    pub fn add_material(&mut self, material: Material) -> MaterialId {
        let id = MaterialId(self.next_material.next() as u32);
        self.materials.insert(id, material);
        id
    }

    pub fn material(&self, id: MaterialId) -> Option<&Material> {
        self.materials.get(&id)
    }

    pub fn material_mut(&mut self, id: MaterialId) -> Option<&mut Material> {
        self.materials.get_mut(&id)
    }

    pub fn add_image(&mut self, image: RasterImage) -> ImageId {
        let id = ImageId(self.next_image.next() as u32);
        self.images.insert(id, Arc::new(image));
        id
    }

    pub fn image(&self, id: ImageId) -> Option<&RasterImage> {
        self.images.get(&id).map(|i| i.as_ref())
    }

    pub fn image_mut(&mut self, id: ImageId) -> Option<&mut RasterImage> {
        self.images.get_mut(&id).map(Arc::make_mut)
    }

    pub fn add_light(&mut self, kind: LightKind) -> LightId {
        let id = LightId(self.next_light.next() as u32);
        self.lights.insert(id, Light::new(id, kind));
        id
    }

    pub fn light(&self, id: LightId) -> Option<&Light> {
        self.lights.get(&id)
    }

    pub fn light_mut(&mut self, id: LightId) -> Option<&mut Light> {
        self.lights.get_mut(&id)
    }

    /// Bumps all id counters past the highest id in the document. Used after
    /// loading external files so new ids never collide.
    pub fn repair_counters(&mut self) {
        let e = self.entities.keys().map(|k| k.0 + 1).max().unwrap_or(0);
        self.next_entity.bump_to(e.saturating_sub(1));
        let m = self
            .meshes
            .keys()
            .map(|k| k.0 as u64 + 1)
            .max()
            .unwrap_or(0);
        self.next_mesh.bump_to(m.saturating_sub(1));
        let mat = self
            .materials
            .keys()
            .map(|k| k.0 as u64 + 1)
            .max()
            .unwrap_or(0);
        self.next_material.bump_to(mat.saturating_sub(1));
        let img = self
            .images
            .keys()
            .map(|k| k.0 as u64 + 1)
            .max()
            .unwrap_or(0);
        self.next_image.bump_to(img.saturating_sub(1));
        let lit = self
            .lights
            .keys()
            .map(|k| k.0 as u64 + 1)
            .max()
            .unwrap_or(0);
        self.next_light.bump_to(lit.saturating_sub(1));
    }

    // ---------------------------------------------------------------- queries

    pub fn visible_mesh_instances(&self) -> Vec<(EntityId, Mat4, MeshId, Option<MaterialId>)> {
        let mut out = Vec::new();
        for (id, _depth) in self.hierarchy() {
            if let Some(e) = self.entities.get(&id) {
                if !e.visible {
                    continue;
                }
                if let (Some(mesh), true) = (e.mesh, self.is_effectively_visible(id)) {
                    out.push((id, self.world_matrix(id), mesh, e.material));
                }
            }
        }
        out
    }

    pub fn is_effectively_visible(&self, id: EntityId) -> bool {
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(c) = cur {
            match self.entities.get(&c) {
                Some(e) => {
                    if !e.visible {
                        return false;
                    }
                    cur = e.parent;
                }
                None => break,
            }
            guard += 1;
            if guard > 4096 {
                break;
            }
        }
        true
    }

    pub fn bounds(&self) -> Aabb {
        let mut b = Aabb::empty();
        for (_id, m, mesh_id, _mat) in self.visible_mesh_instances() {
            if let Some(mesh) = self.mesh(mesh_id) {
                b = b.union(&mesh.bounds().transformed(m));
            }
        }
        b
    }

    /// Closest triangle hit across all visible meshes.
    pub fn raycast(&self, ray: Ray) -> Option<SceneHit> {
        let mut best: Option<SceneHit> = None;
        for (entity, world, mesh_id, _mat) in self.visible_mesh_instances() {
            let Some(mesh) = self.mesh(mesh_id) else {
                continue;
            };
            if !mesh
                .bounds()
                .transformed(world)
                .ray(ray.origin, ray.inv_dir())
                .is_some()
            {
                continue;
            }
            let inv = world.inverse();
            let local_ray = Ray::new(
                inv.transform_point3(ray.origin),
                inv.transform_vector3(ray.direction).normalize_or_zero(),
            );
            let normal_mat = inv.transpose();
            let mut tri_index = 0usize;
            for (fi, face) in mesh.faces.iter().enumerate() {
                let fnrm = face.normal(&mesh.positions);
                for tri in face.triangles() {
                    let a = mesh.positions[tri[0] as usize];
                    let b = mesh.positions[tri[1] as usize];
                    let c = mesh.positions[tri[2] as usize];
                    if let Some((t, _u, _v)) = crate::math::ray_triangle(local_ray, a, b, c) {
                        let world_t = t * (world.transform_vector3(local_ray.direction)).length();
                        if best.as_ref().map(|h| world_t < h.distance).unwrap_or(true) {
                            let local_point = local_ray.at(t);
                            let world_point = world.transform_point3(local_point);
                            let normal = normal_mat.transform_vector3(fnrm).normalize_or_zero();
                            best = Some(SceneHit {
                                entity,
                                distance: world_t,
                                point: world_point,
                                normal,
                                face: fi,
                                triangle: tri_index,
                            });
                        }
                    }
                    tri_index += 1;
                }
            }
        }
        best
    }

    pub fn statistics(&self) -> SceneStats {
        let mut stats = SceneStats {
            entities: self.entities.len(),
            lights: self.lights.len(),
            unique_meshes: self.meshes.len(),
            materials: self.materials.len(),
            images: self.images.len(),
            image_bytes: self.images.values().map(|i| i.byte_len()).sum(),
            ..Default::default()
        };
        for (id, _) in self.hierarchy() {
            if self.is_effectively_visible(id) {
                stats.visible_entities += 1;
            }
        }
        for (_id, _m, mesh_id, _mat) in self.visible_mesh_instances() {
            if let Some(mesh) = self.mesh(mesh_id) {
                stats.mesh_instances += 1;
                stats.triangles += mesh.triangle_count();
                stats.vertices += mesh.vertex_count();
            }
        }
        for m in self.materials.values() {
            if m.albedo_map.is_some()
                || m.orm_map.is_some()
                || m.normal_map.is_some()
                || m.emission_map.is_some()
                || m.opacity_map.is_some()
            {
                stats.materials_referencing_images += 1;
            }
        }
        stats
    }

    /// Structural problems that would break rendering or export.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (id, e) in &self.entities {
            if let Some(parent) = e.parent {
                if !self.entities.contains_key(&parent) {
                    problems.push(format!("{} ({}) has a missing parent", e.name, id));
                }
            }
            if let Some(mesh) = e.mesh {
                if !self.meshes.contains_key(&mesh) {
                    problems.push(format!(
                        "{} ({}) references missing mesh {}",
                        e.name, id, mesh
                    ));
                }
            }
            if let Some(mat) = e.material {
                if !self.materials.contains_key(&mat) {
                    problems.push(format!(
                        "{} ({}) references missing material {}",
                        e.name, id, mat
                    ));
                }
            }
        }
        for root in &self.roots {
            if !self.entities.contains_key(root) {
                problems.push(format!("Root {} is missing from the entity table", root));
            }
        }
        for (mid, m) in &self.materials {
            for (label, img) in [
                ("albedo", m.albedo_map),
                ("orm", m.orm_map),
                ("normal", m.normal_map),
                ("emission", m.emission_map),
                ("opacity", m.opacity_map),
            ] {
                if let Some(i) = img {
                    if !self.images.contains_key(&i) {
                        problems.push(format!(
                            "Material {} ({}) references missing {} image {}",
                            m.name, mid, label, i
                        ));
                    }
                }
            }
            for (label, c) in [("base colour", m.base_color), ("emission", m.emission)] {
                if c.iter().any(|v| !v.is_finite()) {
                    problems.push(format!(
                        "Material {} ({}) has a non-finite {}",
                        m.name, mid, label
                    ));
                }
            }
            if !(0.0..=1.0).contains(&m.opacity) {
                problems.push(format!(
                    "Material {} ({}) opacity is outside 0..1",
                    m.name, mid
                ));
            }
        }
        for (id, mesh) in &self.meshes {
            let v = mesh.validate();
            if v.out_of_range_indices > 0 {
                problems.push(format!(
                    "Mesh {} ({}) has out-of-range indices",
                    mesh.name, id
                ));
            }
            if mesh.positions.is_empty() && !mesh.faces.is_empty() {
                problems.push(format!(
                    "Mesh {} ({}) has faces but no vertices",
                    mesh.name, id
                ));
            }
        }
        let cam = &self.camera;
        if !cam.position.is_finite() || !cam.target.is_finite() {
            problems.push("Camera position/target is not finite".into());
        }
        if cam.near <= 0.0 || cam.far <= cam.near {
            problems.push("Camera near/far planes are invalid".into());
        }
        if cam.forward().length() < EPSILON {
            problems.push("Camera position and target coincide".into());
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::Face;

    fn cube_mesh() -> Mesh {
        let mut m = Mesh::new("Cube");
        let pts = [
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ];
        for p in pts {
            m.add_vertex(p);
        }
        // Outward winding for every face (the renderer relies on it).
        for f in [
            vec![0, 3, 2, 1],
            vec![4, 5, 6, 7],
            vec![0, 4, 7, 3],
            vec![1, 2, 6, 5],
            vec![0, 1, 5, 4],
            vec![3, 7, 6, 2],
        ] {
            m.add_face(Face::new(f));
        }
        m.recompute_normals();
        m
    }

    fn assert_outward_normals(m: &Mesh) {
        for (i, f) in m.faces.iter().enumerate() {
            let c = f.centroid(&m.positions);
            assert!(
                f.normal(&m.positions).dot(c.normalize()) > 0.9,
                "face {i} points inward"
            );
        }
    }

    #[test]
    fn cube_faces_point_outward() {
        assert_outward_normals(&cube_mesh());
    }

    #[test]
    fn hierarchy_parenting_and_world_transforms() {
        let mut s = Scene::new("t");
        let parent = s.add_entity("Parent", None);
        let child = s.add_entity("Child", Some(parent));
        s.entity_mut(parent).unwrap().transform.translation = Vec3::new(2.0, 0.0, 0.0);
        s.entity_mut(child).unwrap().transform.translation = Vec3::new(0.0, 1.0, 0.0);
        assert!((s.world_position(child) - Vec3::new(2.0, 1.0, 0.0)).length() < 1e-5);
        assert_eq!(s.hierarchy().len(), 2);
        assert!(s.is_ancestor_of(parent, child));
        assert!(!s.is_ancestor_of(child, parent));
    }

    #[test]
    fn reparent_preserves_world_position() {
        let mut s = Scene::new("t");
        let a = s.add_entity("A", None);
        s.entity_mut(a).unwrap().transform.translation = Vec3::new(5.0, 0.0, 0.0);
        let b = s.add_entity("B", None);
        s.entity_mut(b).unwrap().transform.translation = Vec3::new(1.0, 2.0, 3.0);
        let before = s.world_position(b);
        s.reparent(b, Some(a)).unwrap();
        let after = s.world_position(b);
        assert!((before - after).length() < 1e-4);
        assert!(s.reparent(a, Some(b)).is_err());
    }

    #[test]
    fn delete_removes_subtree() {
        let mut s = Scene::new("t");
        let a = s.add_entity("A", None);
        let b = s.add_entity("B", Some(a));
        let _c = s.add_entity("C", Some(b));
        let removed = s.delete_entity(a);
        assert_eq!(removed.len(), 3);
        assert!(s.entities.is_empty());
        assert!(s.roots.is_empty());
    }

    #[test]
    fn duplicate_makes_instances() {
        let mut s = Scene::new("t");
        let mesh = s.add_mesh(cube_mesh());
        let a = s.add_entity("Cube", None);
        s.entity_mut(a).unwrap().mesh = Some(mesh);
        let dup = s.duplicate_entity(a).unwrap();
        assert_ne!(a, dup);
        assert_eq!(s.meshes.len(), 1);
        assert_eq!(s.statistics().mesh_instances, 2);
    }

    #[test]
    fn raycast_hits_nearest_surface() {
        let mut s = Scene::new("t");
        let mesh = s.add_mesh(cube_mesh());
        let e = s.add_entity("Cube", None);
        s.entity_mut(e).unwrap().mesh = Some(mesh);
        let hit = s
            .raycast(Ray::new(Vec3::new(0.0, 0.0, -10.0), Vec3::Z))
            .expect("hit");
        assert!((hit.distance - 9.0).abs() < 1e-3);
        assert!((hit.point.z + 1.0).abs() < 1e-3);
        assert!((hit.normal.z + 1.0).abs() < 1e-2);
        assert!(s
            .raycast(Ray::new(Vec3::new(10.0, 10.0, -10.0), Vec3::Z))
            .is_none());
    }

    #[test]
    fn raycast_respects_visibility() {
        let mut s = Scene::new("t");
        let mesh = s.add_mesh(cube_mesh());
        let e = s.add_entity("Cube", None);
        s.entity_mut(e).unwrap().mesh = Some(mesh);
        s.entity_mut(e).unwrap().visible = false;
        assert!(s
            .raycast(Ray::new(Vec3::new(0.0, 0.0, -10.0), Vec3::Z))
            .is_none());
    }

    #[test]
    fn validation_reports_broken_refs() {
        let mut s = Scene::new("t");
        let e = s.add_entity("Broken", None);
        s.entity_mut(e).unwrap().mesh = Some(MeshId(99));
        let problems = s.validate();
        assert!(problems.iter().any(|p| p.contains("missing mesh")));
    }

    #[test]
    fn counters_repair_after_load() {
        let mut s = Scene::new("t");
        s.add_entity("A", None);
        s.repair_counters();
        let b = s.add_entity("B", None);
        assert_ne!(b, EntityId(0));
    }
}
