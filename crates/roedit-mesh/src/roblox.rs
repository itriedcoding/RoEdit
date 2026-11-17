//! Roblox-specific mesh workflows: R6/R15 body part templates, attachment
//! anchors for accessories, MeshPart optimisation and export validation.

use crate::primitives::{self, PartShape};
use crate::{Mesh, Vec3};

/// Roblox's modern avatar scale: one stud is 0.28 metres.
pub const STUD_TO_METER: f32 = 0.28;

/// Conservative MeshPart budgets used by the validator. Roblox enforces
/// per-asset limits that vary by upload type; these are the limits the exporter
/// warns about by default and they can be overridden in the UI.
pub const MAX_TRIANGLES: usize = 10_000;
pub const MAX_VERTICES: usize = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RigType {
    R6,
    R15,
}

impl RigType {
    pub const ALL: [RigType; 2] = [RigType::R6, RigType::R15];

    pub fn label(self) -> &'static str {
        match self {
            RigType::R6 => "R6",
            RigType::R15 => "R15",
        }
    }

    pub const fn part_count(self) -> usize {
        match self {
            RigType::R6 => 6,
            RigType::R15 => 15,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PartTemplate {
    pub name: &'static str,
    /// Size in studs.
    pub size: Vec3,
    /// Centre of the part in studs, relative to the rig root.
    pub center: Vec3,
    pub shape: PartShape,
}

/// Adjustable proportions for custom avatars and UGC bodies.
#[derive(Clone, Copy, Debug)]
pub struct Proportions {
    pub head_scale: f32,
    pub torso_scale: f32,
    pub arm_scale: f32,
    pub leg_scale: f32,
    pub body_width: f32,
}

impl Default for Proportions {
    fn default() -> Self {
        Self {
            head_scale: 1.0,
            torso_scale: 1.0,
            arm_scale: 1.0,
            leg_scale: 1.0,
            body_width: 1.0,
        }
    }
}

pub fn r6_parts() -> Vec<PartTemplate> {
    vec![
        PartTemplate {
            name: "Torso",
            size: Vec3::new(2.0, 2.0, 1.0),
            center: Vec3::new(0.0, 0.0, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "Head",
            size: Vec3::new(2.0, 1.0, 1.0),
            center: Vec3::new(0.0, 1.5, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "Left Arm",
            size: Vec3::new(1.0, 2.0, 1.0),
            center: Vec3::new(-1.5, 0.0, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "Right Arm",
            size: Vec3::new(1.0, 2.0, 1.0),
            center: Vec3::new(1.5, 0.0, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "Left Leg",
            size: Vec3::new(1.0, 2.0, 1.0),
            center: Vec3::new(-0.5, -2.0, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "Right Leg",
            size: Vec3::new(1.0, 2.0, 1.0),
            center: Vec3::new(0.5, -2.0, 0.0),
            shape: PartShape::Block,
        },
    ]
}

pub fn r15_parts() -> Vec<PartTemplate> {
    // 15 rendered body parts. The HumanoidRootPart is an invisible physics
    // part and is not exported as geometry.
    vec![
        PartTemplate {
            name: "LowerTorso",
            size: Vec3::new(2.0, 0.8, 1.0),
            center: Vec3::new(0.0, 0.0, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "UpperTorso",
            size: Vec3::new(2.0, 1.6, 1.0),
            center: Vec3::new(0.0, 1.2, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "Head",
            size: Vec3::new(1.2, 1.2, 1.2),
            center: Vec3::new(0.0, 2.7, 0.0),
            shape: PartShape::Ball,
        },
        PartTemplate {
            name: "LeftUpperArm",
            size: Vec3::new(0.8, 1.2, 0.8),
            center: Vec3::new(-1.4, 1.8, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "LeftLowerArm",
            size: Vec3::new(0.8, 1.2, 0.8),
            center: Vec3::new(-1.4, 0.55, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "LeftHand",
            size: Vec3::new(0.8, 0.5, 0.8),
            center: Vec3::new(-1.4, -0.35, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "RightUpperArm",
            size: Vec3::new(0.8, 1.2, 0.8),
            center: Vec3::new(1.4, 1.8, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "RightLowerArm",
            size: Vec3::new(0.8, 1.2, 0.8),
            center: Vec3::new(1.4, 0.55, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "RightHand",
            size: Vec3::new(0.8, 0.5, 0.8),
            center: Vec3::new(1.4, -0.35, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "LeftUpperLeg",
            size: Vec3::new(0.9, 1.4, 0.9),
            center: Vec3::new(-0.5, -1.1, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "LeftLowerLeg",
            size: Vec3::new(0.9, 1.4, 0.9),
            center: Vec3::new(-0.5, -2.6, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "LeftFoot",
            size: Vec3::new(0.9, 0.5, 1.2),
            center: Vec3::new(-0.5, -3.5, 0.15),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "RightUpperLeg",
            size: Vec3::new(0.9, 1.4, 0.9),
            center: Vec3::new(0.5, -1.1, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "RightLowerLeg",
            size: Vec3::new(0.9, 1.4, 0.9),
            center: Vec3::new(0.5, -2.6, 0.0),
            shape: PartShape::Block,
        },
        PartTemplate {
            name: "RightFoot",
            size: Vec3::new(0.9, 0.5, 1.2),
            center: Vec3::new(0.5, -3.5, 0.15),
            shape: PartShape::Block,
        },
    ]
}

/// Real Roblox attachment names with their host part and local position (in
/// studs). Used when preparing accessories and layered clothing.
pub fn attachment_defs(rig: RigType) -> Vec<(&'static str, &'static str, Vec3)> {
    match rig {
        RigType::R6 => vec![
            ("HatAttachment", "Head", Vec3::new(0.0, 0.6, 0.0)),
            ("HairAttachment", "Head", Vec3::new(0.0, 0.55, 0.05)),
            ("FaceFrontAttachment", "Head", Vec3::new(0.0, 0.0, -0.6)),
            ("FaceCenterAttachment", "Head", Vec3::new(0.0, 0.0, -0.5)),
            ("NeckAttachment", "Torso", Vec3::new(0.0, 1.0, 0.0)),
            ("LeftCollarAttachment", "Torso", Vec3::new(-1.0, 0.5, 0.0)),
            ("RightCollarAttachment", "Torso", Vec3::new(1.0, 0.5, 0.0)),
            ("BodyFrontAttachment", "Torso", Vec3::new(0.0, 0.0, -0.5)),
            ("BodyBackAttachment", "Torso", Vec3::new(0.0, 0.0, 0.5)),
            ("WaistCenterAttachment", "Torso", Vec3::new(0.0, -1.0, 0.0)),
            ("WaistBackAttachment", "Torso", Vec3::new(0.0, -1.0, 0.5)),
            ("LeftGripAttachment", "Left Arm", Vec3::new(0.0, -1.0, 0.0)),
            (
                "RightGripAttachment",
                "Right Arm",
                Vec3::new(0.0, -1.0, 0.0),
            ),
        ],
        RigType::R15 => vec![
            ("HatAttachment", "Head", Vec3::new(0.0, 0.6, 0.0)),
            ("HairAttachment", "Head", Vec3::new(0.0, 0.5, 0.1)),
            ("FaceFrontAttachment", "Head", Vec3::new(0.0, 0.0, -0.6)),
            ("NeckAttachment", "UpperTorso", Vec3::new(0.0, 0.8, 0.0)),
            (
                "LeftCollarAttachment",
                "UpperTorso",
                Vec3::new(-1.0, 0.4, 0.0),
            ),
            (
                "RightCollarAttachment",
                "UpperTorso",
                Vec3::new(1.0, 0.4, 0.0),
            ),
            (
                "BodyFrontAttachment",
                "LowerTorso",
                Vec3::new(0.0, 0.0, -0.5),
            ),
            ("BodyBackAttachment", "LowerTorso", Vec3::new(0.0, 0.0, 0.5)),
            (
                "WaistCenterAttachment",
                "LowerTorso",
                Vec3::new(0.0, -0.4, 0.0),
            ),
            ("LeftGripAttachment", "LeftHand", Vec3::new(0.0, -0.25, 0.0)),
            (
                "RightGripAttachment",
                "RightHand",
                Vec3::new(0.0, -0.25, 0.0),
            ),
            ("LeftFootAttachment", "LeftFoot", Vec3::new(0.0, 0.0, 0.1)),
            ("RightFootAttachment", "RightFoot", Vec3::new(0.0, 0.0, 0.1)),
        ],
    }
}

#[derive(Clone, Debug, Default)]
pub struct RigBuild {
    /// Part name and mesh, positioned in world space (metres).
    pub parts: Vec<(String, Mesh)>,
    /// (attachment name, part name, world position in metres)
    pub attachments: Vec<(String, String, Vec3)>,
    pub triangles: usize,
}

/// Builds a character as separate part meshes, matching Roblox's naming so the
/// exported FBX/glTF parts map onto a Roblox rig.
pub fn build_rig(rig: RigType, proportions: Proportions, detail: u32, unit_scale: f32) -> RigBuild {
    let scale = if unit_scale > 0.0 {
        unit_scale
    } else {
        STUD_TO_METER
    };
    let templates = match rig {
        RigType::R6 => r6_parts(),
        RigType::R15 => r15_parts(),
    };
    let mut build = RigBuild::default();
    for t in templates {
        let mut size = t.size;
        let mut center = t.center;
        match t.name {
            "Head" => {
                size *= proportions.head_scale;
            }
            n if n.contains("Torso") => {
                size *= proportions.torso_scale;
                size.x *= proportions.body_width;
            }
            n if n.contains("Arm") || n.contains("Hand") => {
                size *= proportions.arm_scale;
            }
            n if n.contains("Leg") || n.contains("Foot") => {
                size *= proportions.leg_scale;
            }
            _ => {}
        }
        size.x *= proportions.body_width.max(0.1);
        center.x *= proportions.body_width.max(0.1);
        let mesh_size = size * scale;
        let mut mesh = primitives::roblox_part(t.shape, mesh_size, detail);
        mesh.name = t.name.to_string();
        for p in &mut mesh.positions {
            *p += center * scale;
        }
        mesh.recompute_normals();
        build.triangles += mesh.triangle_count();
        build.parts.push((t.name.to_string(), mesh));
    }
    let part_centers: Vec<(String, Vec3)> = build
        .parts
        .iter()
        .map(|(n, m)| (n.clone(), m.bounds().center()))
        .collect();
    for (name, host, local_studs) in attachment_defs(rig) {
        if let Some((_, center)) = part_centers.iter().find(|(n, _)| n == host) {
            build.attachments.push((
                name.to_string(),
                host.to_string(),
                *center + local_studs * scale,
            ));
        }
    }
    build
}

#[derive(Clone, Debug, Default)]
pub struct OptimizationReport {
    pub welded: usize,
    pub triangles_before: usize,
    pub triangles_after: usize,
    pub decimated: bool,
    pub warnings: Vec<String>,
}

/// Welds and, when needed, decimates a mesh so it fits a Roblox MeshPart budget.
pub fn optimize_meshpart(mesh: &mut Mesh, max_triangles: usize) -> OptimizationReport {
    let mut report = OptimizationReport {
        triangles_before: mesh.triangle_count(),
        ..Default::default()
    };
    report.welded = crate::edit::merge_by_distance(mesh, 1e-4);
    if report.triangles_before > max_triangles.max(4) {
        // Triangle count scales roughly with the vertex ratio.
        let ratio = max_triangles as f32 / report.triangles_before as f32;
        let _ = crate::edit::decimate(mesh, ratio.clamp(0.05, 1.0));
        report.decimated = true;
        if mesh.triangle_count() > max_triangles {
            report.warnings.push(format!(
                "Decimation reached {} triangles, still above the {max_triangles} budget",
                mesh.triangle_count()
            ));
        }
    }
    report.triangles_after = mesh.triangle_count();
    if mesh.materials_used() > 1 {
        report
            .warnings
            .push("Roblox MeshParts use a single colour/texture; extra materials must be split into separate parts".into());
    }
    report
}

#[derive(Clone, Debug)]
pub struct Check {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Default)]
pub struct RobloxValidation {
    pub checks: Vec<Check>,
    pub passed: bool,
}

impl RobloxValidation {
    pub fn summary(&self) -> String {
        let failed: Vec<&Check> = self.checks.iter().filter(|c| !c.passed).collect();
        if failed.is_empty() {
            format!("All {} Roblox export checks passed", self.checks.len())
        } else {
            failed
                .iter()
                .map(|c| format!("{}: {}", c.name, c.detail))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

pub fn validate_for_roblox(mesh: &Mesh) -> RobloxValidation {
    let mut v = RobloxValidation::default();
    let mut push = |name: &str, passed: bool, detail: String| {
        v.checks.push(Check {
            name: name.to_string(),
            passed,
            detail,
        });
    };
    let stats = mesh.stats();
    let validation = mesh.validate();
    push(
        "Triangle budget",
        stats.triangles <= MAX_TRIANGLES,
        format!("{} / {} triangles", stats.triangles, MAX_TRIANGLES),
    );
    push(
        "Vertex budget",
        stats.vertices <= MAX_VERTICES,
        format!("{} / {} vertices", stats.vertices, MAX_VERTICES),
    );
    push(
        "Degenerate faces",
        validation.degenerate_faces == 0,
        format!("{} degenerate", validation.degenerate_faces),
    );
    push(
        "Non-manifold edges",
        validation.non_manifold_edges == 0,
        format!("{} non-manifold", validation.non_manifold_edges),
    );
    push(
        "Indices in range",
        validation.out_of_range_indices == 0,
        format!("{} invalid indices", validation.out_of_range_indices),
    );
    let has_uvs = mesh
        .faces
        .iter()
        .all(|f| f.uvs.len() == f.verts.len() && !f.uvs.is_empty());
    push(
        "UV coordinates",
        has_uvs,
        if has_uvs {
            "every face has UVs".into()
        } else {
            "some faces are missing UVs".into()
        },
    );
    push(
        "Normals",
        mesh.normals.len() == mesh.positions.len(),
        format!(
            "{} normals for {} vertices",
            mesh.normals.len(),
            mesh.positions.len()
        ),
    );
    push(
        "Material slots",
        stats.materials <= 1,
        format!("{} material slot(s)", stats.materials),
    );
    let size = mesh.bounds().size();
    let max_dim_studs = size.max_element() / STUD_TO_METER;
    push(
        "Scale",
        max_dim_studs <= 2048.0,
        format!("{:.1} studs across (limit 2048)", max_dim_studs),
    );
    v.passed = v.checks.iter().all(|c| c.passed);
    v
}

/// Rescales a mesh from arbitrary units into studs.
pub fn scale_to_studs(mesh: &mut Mesh, units_per_stud: f32) {
    if units_per_stud <= 0.0 {
        return;
    }
    let factor = 1.0 / units_per_stud;
    for p in &mut mesh.positions {
        *p *= factor;
    }
    mesh.recompute_normals();
}

/// Rescales a mesh so its largest dimension equals `studs`.
pub fn fit_to_studs(mesh: &mut Mesh, studs: f32) {
    let size = mesh.bounds().size().max_element();
    if size <= 1e-6 || studs <= 0.0 {
        return;
    }
    let factor = studs / size;
    for p in &mut mesh.positions {
        *p *= factor;
    }
    mesh.recompute_normals();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signed_volume;

    #[test]
    fn r6_and_r15_part_counts() {
        assert_eq!(r6_parts().len(), 6);
        assert_eq!(r15_parts().len(), 15);
        assert_eq!(RigType::R6.part_count(), 6);
        assert_eq!(RigType::R15.part_count(), 15);
    }

    #[test]
    fn rig_build_produces_valid_parts() {
        for rig in RigType::ALL {
            let build = build_rig(rig, Proportions::default(), 12, STUD_TO_METER);
            assert_eq!(build.parts.len(), rig.part_count(), "{rig:?}");
            assert!(build.triangles > 0);
            assert!(!build.attachments.is_empty());
            for (name, mesh) in &build.parts {
                assert!(!mesh.positions.is_empty(), "{name} has no vertices");
                assert_eq!(mesh.validate().out_of_range_indices, 0, "{name}");
                assert!(signed_volume(mesh) > 0.0, "{name} is inside out");
            }
        }
    }

    #[test]
    fn attachments_reference_existing_parts() {
        for rig in RigType::ALL {
            let build = build_rig(rig, Proportions::default(), 8, STUD_TO_METER);
            for (name, part, _pos) in &build.attachments {
                assert!(
                    build.parts.iter().any(|(p, _)| p == part),
                    "{name} references missing part {part}"
                );
            }
        }
    }

    #[test]
    fn proportions_scale_the_head() {
        let normal = build_rig(RigType::R6, Proportions::default(), 8, STUD_TO_METER);
        let big = build_rig(
            RigType::R6,
            Proportions {
                head_scale: 2.0,
                ..Default::default()
            },
            8,
            STUD_TO_METER,
        );
        let head_size = |b: &RigBuild| {
            b.parts
                .iter()
                .find(|(n, _)| n == "Head")
                .map(|(_, m)| m.bounds().size().x)
                .unwrap()
        };
        assert!((head_size(&big) - head_size(&normal) * 2.0).abs() < 1e-3);
    }

    #[test]
    fn validation_passes_for_clean_cube() {
        let m = primitives::cube(Vec3::new(2.0, 2.0, 2.0));
        let v = validate_for_roblox(&m);
        assert!(v.passed, "{}", v.summary());
        assert!(v.checks.len() >= 8);
    }

    #[test]
    fn validation_flags_missing_uvs() {
        let mut m = primitives::cube(Vec3::ONE);
        for f in &mut m.faces {
            f.uvs.clear();
        }
        let v = validate_for_roblox(&m);
        assert!(!v.passed);
        assert!(v.summary().contains("UV"));
    }

    #[test]
    fn optimize_decimates_heavy_meshes() {
        let mut m = primitives::uv_sphere(1.0, 64, 32);
        let before = m.triangle_count();
        let report = optimize_meshpart(&mut m, 2000);
        assert_eq!(report.triangles_before, before);
        assert!(report.decimated);
        assert!(report.triangles_after < before);
    }

    #[test]
    fn scale_helpers_work() {
        let mut m = primitives::cube(Vec3::ONE);
        fit_to_studs(&mut m, 4.0);
        assert!((m.bounds().size().max_element() - 4.0).abs() < 1e-4);
        scale_to_studs(&mut m, 0.5);
        assert!((m.bounds().size().max_element() - 8.0).abs() < 1e-4);
    }
}
