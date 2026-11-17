//! Executes extension-pack macros against a project using the host's own
//! operator implementations.

use crate::manifest::{Capability, MacroStep, PluginManifest};
use crate::PluginError;
use roedit_core::ids::VideoTrackId;
use roedit_core::material::Material;
use roedit_core::math::Vec3;
use roedit_core::mesh::Mesh;
use roedit_core::project::Project;
use roedit_core::render_settings::{RenderMode, RenderQuality};
use roedit_core::scene::LightKind;
use roedit_core::timeline::{ClipSource, Timeline, TrackKind, VideoTrack};
use roedit_mesh::primitives as prim;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct MacroReport {
    pub pack: String,
    pub command: String,
    pub label: String,
    pub steps: usize,
    pub log: Vec<String>,
    pub entities_added: usize,
    pub materials_added: usize,
    pub lights_added: usize,
    pub tracks_added: usize,
}

impl MacroReport {
    pub fn summary(&self) -> String {
        format!(
            "{}: {} steps, +{} objects, +{} materials, +{} lights, +{} tracks",
            self.label,
            self.steps,
            self.entities_added,
            self.materials_added,
            self.lights_added,
            self.tracks_added
        )
    }
}

fn primitive_mesh(name: &str, size: Vec3) -> Option<Mesh> {
    let s = size;
    Some(match name {
        "cube" => prim::cube(s),
        "sphere" => prim::uv_sphere((s.x.min(s.y).min(s.z) * 0.5).max(0.01), 32, 16),
        "cylinder" => prim::cylinder(s.x * 0.5, s.y, 32, true),
        "cone" => prim::cone(s.x * 0.5, s.y, 32),
        "torus" => prim::torus(s.x * 0.5, (s.y * 0.18).max(0.01), 32, 16),
        "capsule" => prim::capsule(s.x * 0.5, s.y, 24, 12),
        "disc" => prim::disc(s.x * 0.5, 32),
        "grid" => prim::grid_plane(s.x, 8),
        "wedge" => prim::wedge(s),
        "corner_wedge" => prim::corner_wedge(s),
        "roblox_part" => prim::roblox_part(prim::PartShape::Block, s, 8),
        _ => return None,
    })
}

fn parse_track_kind(name: &str) -> Option<TrackKind> {
    Some(match name.to_lowercase().as_str() {
        "video" => TrackKind::Video,
        "image" => TrackKind::Image,
        "text" => TrackKind::Text,
        "audio" => TrackKind::Audio,
        "adjustment" => TrackKind::Adjustment,
        _ => return None,
    })
}

fn parse_quality(name: &str) -> Option<RenderQuality> {
    RenderQuality::ALL.iter().copied().find(|q| {
        q.label().eq_ignore_ascii_case(name) || format!("{q:?}").eq_ignore_ascii_case(name)
    })
}

fn parse_mode(name: &str) -> Option<RenderMode> {
    RenderMode::ALL.iter().copied().find(|m| {
        m.label().eq_ignore_ascii_case(name) || format!("{m:?}").eq_ignore_ascii_case(name)
    })
}

fn material_id_by_name(project: &Project, name: &str) -> Option<roedit_core::ids::MaterialId> {
    project
        .scene
        .materials
        .iter()
        .find(|(_, m)| m.name.eq_ignore_ascii_case(name))
        .map(|(id, _)| *id)
}

fn apply_mesh_op(mesh: &mut Mesh, op: &str, amount: f32) -> Result<String, String> {
    let all_faces: Vec<usize> = (0..mesh.faces.len()).collect();
    match op {
        "subdivide" => {
            let faces = roedit_mesh::edit::subdivide_faces(mesh, &all_faces, amount.round() as u32)
                .map_err(|e| e.to_string())?;
            Ok(format!("subdivided to {} faces", faces.len()))
        }
        "bevel" => {
            let count = roedit_mesh::edit::bevel_faces(mesh, &all_faces, amount, 2)
                .map_err(|e| e.to_string())?;
            Ok(format!("beveled {count} edges"))
        }
        "extrude" => {
            let faces = roedit_mesh::edit::extrude_faces(mesh, &all_faces, amount)
                .map_err(|e| e.to_string())?;
            Ok(format!("extruded into {} faces", faces.len()))
        }
        "smooth" => {
            let moved =
                roedit_mesh::edit::smooth_laplacian(mesh, None, amount.max(1.0) as u32, 0.5);
            Ok(format!("smoothed {moved} vertices"))
        }
        "decimate" => {
            let ratio = roedit_mesh::edit::decimate(mesh, amount).map_err(|e| e.to_string())?;
            Ok(format!("decimated to {:.0}%", ratio * 100.0))
        }
        "solidify" => {
            let added = roedit_mesh::edit::solidify(mesh, amount).map_err(|e| e.to_string())?;
            Ok(format!("added {added} shell faces"))
        }
        "triangulate" => {
            let n = roedit_mesh::edit::triangulate(mesh);
            Ok(format!("triangulated {n} faces"))
        }
        "recalc_normals" => {
            let flipped = roedit_mesh::edit::recalc_normals_outside(mesh);
            Ok(if flipped {
                "flipped outward"
            } else {
                "normals already outward"
            }
            .to_string())
        }
        other => Err(format!("unsupported mesh op '{other}'")),
    }
}

/// Runs one command from a pack. Every step is pre-validated, and each step's
/// capability is checked again here so a hand-written manifest cannot bypass
/// the declared permissions.
pub fn execute_command(
    manifest: &PluginManifest,
    command_id: &str,
    project: &mut Project,
) -> Result<MacroReport, PluginError> {
    let command = manifest
        .commands
        .iter()
        .find(|c| c.id == command_id)
        .ok_or_else(|| PluginError::NotFound(format!("command '{command_id}'")))?;
    let mut report = MacroReport {
        pack: manifest.id.clone(),
        command: command.id.clone(),
        label: command.label.clone(),
        ..Default::default()
    };
    let mut last_entity = None;
    let mut track_names: BTreeMap<String, VideoTrackId> = BTreeMap::new();
    if let Some(timeline) = &project.timeline {
        for track in &timeline.tracks {
            track_names.insert(track.name.clone(), track.id);
        }
    }

    for (i, step) in command.steps.iter().enumerate() {
        let needed: Capability = step.required_capability();
        if !manifest.capabilities.contains(&needed) {
            return Err(PluginError::Capability(format!(
                "step {} ({}) requires the '{}' capability",
                i + 1,
                step.label(),
                needed.label().to_lowercase()
            )));
        }
        match step {
            MacroStep::AddPrimitive {
                primitive,
                position,
                size,
                material,
                name,
            } => {
                let Some(mut mesh) = primitive_mesh(primitive, Vec3::from_array(*size)) else {
                    return Err(PluginError::StepFailed {
                        step: i + 1,
                        message: format!("unknown primitive '{primitive}'"),
                    });
                };
                let label = name.clone().unwrap_or_else(|| mesh.name.clone());
                mesh.name = label.clone();
                let mesh_id = project.scene.add_mesh(mesh);
                let entity_id = project.scene.add_entity(label.clone(), None);
                if let Some(entity) = project.scene.entity_mut(entity_id) {
                    entity.mesh = Some(mesh_id);
                    entity.transform.translation = Vec3::from_array(*position);
                }
                if let Some(mat_name) = material {
                    let mat_id = material_id_by_name(project, mat_name).unwrap_or_else(|| {
                        project.scene.add_material(Material::new(
                            mat_name.clone(),
                            [0.75, 0.75, 0.78],
                            0.0,
                            0.5,
                        ))
                    });
                    if let Some(entity) = project.scene.entity_mut(entity_id) {
                        entity.material = Some(mat_id);
                    }
                }
                last_entity = Some(entity_id);
                report.entities_added += 1;
                report.log.push(format!("added {primitive} '{label}'"));
            }
            MacroStep::AddMaterial {
                name,
                color,
                metallic,
                roughness,
                emission,
            } => {
                let mut material = Material::new(name.clone(), *color, *metallic, *roughness);
                if *emission > 0.0 {
                    material.emission = *color;
                    material.emission_strength = *emission;
                }
                let id = project.scene.add_material(material);
                if let Some(entity_id) = last_entity {
                    if let Some(entity) = project.scene.entity_mut(entity_id) {
                        entity.material = Some(id);
                    }
                }
                report.materials_added += 1;
                report.log.push(format!("added material '{name}'"));
            }
            MacroStep::AddLight {
                kind,
                position,
                intensity,
                color,
            } => {
                let kind = match kind.to_lowercase().as_str() {
                    "point" => LightKind::Point,
                    "directional" => LightKind::Directional,
                    "spot" => LightKind::Spot,
                    other => {
                        return Err(PluginError::StepFailed {
                            step: i + 1,
                            message: format!("unknown light kind '{other}'"),
                        })
                    }
                };
                let id = project.scene.add_light(kind);
                if let Some(light) = project.scene.light_mut(id) {
                    light.position = Vec3::from_array(*position);
                    light.intensity = *intensity;
                    light.color = *color;
                    light.direction = (Vec3::ZERO - light.position).normalize_or_zero();
                }
                report.lights_added += 1;
                report.log.push(format!("added {kind:?} light"));
            }
            MacroStep::ApplyMeshOp { operator, amount } => {
                let targets: Vec<roedit_core::ids::MeshId> = match last_entity {
                    Some(entity) => project
                        .scene
                        .entity(entity)
                        .and_then(|e| e.mesh)
                        .into_iter()
                        .collect(),
                    None => project.scene.meshes.keys().copied().collect(),
                };
                if targets.is_empty() {
                    return Err(PluginError::StepFailed {
                        step: i + 1,
                        message: "no mesh to apply the operator to".into(),
                    });
                }
                for id in targets {
                    if let Some(mesh) = project.scene.mesh_mut(id) {
                        let message =
                            apply_mesh_op(mesh, operator, *amount).map_err(|message| {
                                PluginError::StepFailed {
                                    step: i + 1,
                                    message,
                                }
                            })?;
                        report.log.push(format!("{}: {message}", mesh.name));
                    }
                }
            }
            MacroStep::SetEnvironment {
                sun_intensity,
                sun_angle_deg,
                sky_top,
                sky_horizon,
                exposure,
            } => {
                let env = &mut project.scene.environment;
                if let Some(v) = sun_intensity {
                    env.sun_intensity = v.max(0.0);
                }
                if let Some(v) = sun_angle_deg {
                    env.sun_angle_deg = v.clamp(0.05, 30.0);
                }
                if let Some(v) = sky_top {
                    env.sky_top = *v;
                }
                if let Some(v) = sky_horizon {
                    env.sky_horizon = *v;
                }
                if let Some(v) = exposure {
                    env.exposure = v.clamp(0.05, 16.0);
                }
                report.log.push("updated the environment".into());
            }
            MacroStep::SetRender {
                quality,
                samples,
                max_bounces,
                mode,
            } => {
                if let Some(q) = quality {
                    if let Some(q) = parse_quality(q) {
                        project.render.apply_quality_preset(q);
                    }
                }
                if let Some(s) = samples {
                    project.render.samples = (*s).clamp(1, 4096);
                }
                if let Some(b) = max_bounces {
                    project.render.max_bounces = (*b).clamp(1, 32);
                }
                if let Some(m) = mode {
                    if let Some(mode) = parse_mode(m) {
                        project.render.mode = mode;
                    }
                }
                report.log.push("updated render settings".into());
            }
            MacroStep::AddTimelineTrack { name, kind } => {
                let Some(kind) = parse_track_kind(kind) else {
                    return Err(PluginError::StepFailed {
                        step: i + 1,
                        message: "unknown track kind".into(),
                    });
                };
                if project.timeline.is_none() {
                    project.timeline = Some(Timeline::new(1920, 1080, 30.0, 10.0));
                }
                let timeline = project.timeline.as_mut().unwrap();
                let next_id = timeline
                    .tracks
                    .iter()
                    .map(|t| t.id.0 + 1)
                    .max()
                    .unwrap_or(0);
                let id = VideoTrackId(next_id);
                timeline
                    .tracks
                    .push(VideoTrack::new(id, name.clone(), kind));
                track_names.insert(name.clone(), id);
                report.tracks_added += 1;
                report.log.push(format!("added track '{name}'"));
            }
            MacroStep::AddTimelineClip {
                track,
                color,
                start,
                duration,
            } => {
                let Some(timeline) = project.timeline.as_mut() else {
                    return Err(PluginError::StepFailed {
                        step: i + 1,
                        message: "the project has no timeline yet".into(),
                    });
                };
                let track_id = match track_names.get(track).copied() {
                    Some(id) => id,
                    None => {
                        let next_id = timeline
                            .tracks
                            .iter()
                            .map(|t| t.id.0 + 1)
                            .max()
                            .unwrap_or(0);
                        let id = VideoTrackId(next_id);
                        timeline
                            .tracks
                            .push(VideoTrack::new(id, track.clone(), TrackKind::Image));
                        track_names.insert(track.clone(), id);
                        report.tracks_added += 1;
                        id
                    }
                };
                if timeline
                    .add_clip(track_id, ClipSource::Color(*color), *start, *duration)
                    .is_none()
                {
                    return Err(PluginError::StepFailed {
                        step: i + 1,
                        message: "could not add the clip".into(),
                    });
                }
                report.log.push(format!("added clip on '{track}'"));
            }
        }
        report.steps += 1;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::PluginManifest;

    fn manifest(json: &str) -> PluginManifest {
        let m: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(m.validate().is_empty(), "{:?}", m.validate());
        m
    }

    #[test]
    fn adds_a_primitive_with_material() {
        let m = manifest(
            r#"{
            "id": "test.pack", "name": "Test", "api_version": 1,
            "capabilities": ["mesh", "material"],
            "commands": [{
                "id": "test.pack.crate",
                "label": "Crate",
                "steps": [
                    {"op": "add_primitive", "primitive": "cube", "position": [1.0, 2.0, 3.0], "size": [2.0, 2.0, 2.0], "name": "Crate", "material": "Crate Wood"},
                    {"op": "add_material", "name": "Crate Wood", "color": [0.4, 0.25, 0.1], "roughness": 0.7}
                ]
            }]
        }"#,
        );
        let mut project = Project::new("Plugin Test");
        let before = project.scene.entities.len();
        let report = execute_command(&m, "test.pack.crate", &mut project).unwrap();
        assert_eq!(report.entities_added, 1);
        assert_eq!(report.materials_added, 1);
        assert_eq!(project.scene.entities.len(), before + 1);
        let entity = project
            .scene
            .entities
            .values()
            .find(|e| e.name == "Crate")
            .expect("crate entity");
        assert!(entity.mesh.is_some());
        assert!(entity.material.is_some());
        assert_eq!(entity.transform.translation, Vec3::new(1.0, 2.0, 3.0));
        let mesh = project.scene.mesh(entity.mesh.unwrap()).unwrap();
        assert_eq!(mesh.faces.len(), 6);
    }

    #[test]
    fn mesh_ops_run_on_the_last_primitive() {
        let m = manifest(
            r#"{
            "id": "test.subdiv", "name": "Subdiv", "api_version": 1,
            "capabilities": ["mesh"],
            "commands": [{
                "id": "test.subdiv.run",
                "label": "Subdivide",
                "steps": [
                    {"op": "add_primitive", "primitive": "cube", "size": [1.0, 1.0, 1.0]},
                    {"op": "apply_mesh_op", "operator": "subdivide", "amount": 1.0}
                ]
            }]
        }"#,
        );
        let mut project = Project::new("Subdiv Test");
        let report = execute_command(&m, "test.subdiv.run", &mut project).unwrap();
        assert_eq!(report.steps, 2);
        let mesh_id = *project.scene.meshes.keys().next().unwrap();
        let faces = project.scene.mesh(mesh_id).unwrap().faces.len();
        assert!(faces > 6, "expected subdivision, got {faces} faces");
    }

    #[test]
    fn capability_violation_is_rejected_at_execution() {
        let m = manifest(
            r#"{
            "id": "test.lights", "name": "Lights", "api_version": 1,
            "capabilities": ["light"],
            "commands": [{
                "id": "test.lights.add",
                "label": "Add light",
                "steps": [{"op": "add_light", "kind": "point"}]
            }]
        }"#,
        );
        let mut broken = m.clone();
        broken.capabilities.clear();
        let mut project = Project::new("Lights");
        let before = project.scene.lights.len();
        assert!(execute_command(&broken, "test.lights.add", &mut project).is_err());
        assert_eq!(
            project.scene.lights.len(),
            before,
            "the broken pack changed nothing"
        );
        assert!(execute_command(&m, "test.lights.add", &mut project).is_ok());
        assert_eq!(project.scene.lights.len(), before + 1);
    }

    #[test]
    fn environment_and_render_steps_apply() {
        let m = manifest(
            r#"{
            "id": "test.look", "name": "Look", "api_version": 1,
            "capabilities": ["environment", "render"],
            "commands": [{
                "id": "test.look.night",
                "label": "Night",
                "steps": [
                    {"op": "set_environment", "sun_intensity": 0.2, "sky_top": [0.01, 0.02, 0.05], "exposure": 2.0},
                    {"op": "set_render", "quality": "high", "mode": "path_trace"}
                ]
            }]
        }"#,
        );
        let mut project = Project::new("Look");
        execute_command(&m, "test.look.night", &mut project).unwrap();
        assert!((project.scene.environment.sun_intensity - 0.2).abs() < 1e-6);
        assert_eq!(project.scene.environment.sky_top, [0.01, 0.02, 0.05]);
        assert_eq!(project.render.mode, RenderMode::PathTrace);
        assert_eq!(project.render.quality, RenderQuality::High);
    }

    #[test]
    fn timeline_steps_create_tracks_and_clips() {
        let m = manifest(
            r#"{
            "id": "test.edit", "name": "Edit", "api_version": 1,
            "capabilities": ["timeline"],
            "commands": [{
                "id": "test.edit.slate",
                "label": "Slate",
                "steps": [
                    {"op": "add_timeline_track", "name": "Slate", "kind": "image"},
                    {"op": "add_timeline_clip", "track": "Slate", "color": [0.1, 0.1, 0.1], "start": 0.0, "duration": 2.0}
                ]
            }]
        }"#,
        );
        let mut project = Project::new("Slate");
        let report = execute_command(&m, "test.edit.slate", &mut project).unwrap();
        assert_eq!(report.tracks_added, 1);
        let timeline = project.timeline.as_ref().unwrap();
        assert_eq!(timeline.tracks.len(), 1);
        assert_eq!(timeline.clips.len(), 1);
        assert!((timeline.clips[0].duration - 2.0).abs() < 1e-6);
    }

    #[test]
    fn unknown_command_is_an_error() {
        let m = manifest(
            r#"{"id": "test.empty", "name": "Empty", "api_version": 1, "capabilities": ["mesh"],
                "commands": [{"id": "test.empty.a", "label": "A", "steps": [{"op": "add_primitive", "primitive": "cube"}]}]}"#,
        );
        let mut project = Project::new("x");
        assert!(execute_command(&m, "nope", &mut project).is_err());
    }
}
