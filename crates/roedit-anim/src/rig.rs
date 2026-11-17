//! Skeletons, IK solvers, poses and retargeting.
//!
//! Bone offsets follow Roblox's R6/R15 rigs (in studs, converted to metres).
//! The IK solvers are analytic/iterative geometry and are fully unit tested.

use crate::STUD_TO_METER;
use roedit_core::math::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    /// Rest translation relative to the parent bone, in metres.
    pub rest_offset: Vec3,
    pub rest_rotation: Quat,
    pub length: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skeleton {
    pub name: String,
    pub bones: Vec<Bone>,
}

impl Skeleton {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bones: Vec::new(),
        }
    }

    pub fn add_bone(
        &mut self,
        name: impl Into<String>,
        parent: Option<usize>,
        rest_offset: Vec3,
        length: f32,
    ) -> usize {
        self.bones.push(Bone {
            name: name.into(),
            parent,
            rest_offset,
            rest_rotation: Quat::IDENTITY,
            length,
        });
        self.bones.len() - 1
    }

    /// Roblox R6 rig: six blocky body parts, with Torso as the root.
    pub fn r6() -> Self {
        let s = STUD_TO_METER;
        let mut sk = Self::new("R6");
        let torso = sk.add_bone("Torso", None, Vec3::ZERO, 2.0 * s);
        let head = sk.add_bone("Head", Some(torso), Vec3::new(0.0, 1.5, 0.0) * s, 1.0 * s);
        sk.add_bone(
            "Left Arm",
            Some(torso),
            Vec3::new(-1.5, 0.0, 0.0) * s,
            2.0 * s,
        );
        sk.add_bone(
            "Right Arm",
            Some(torso),
            Vec3::new(1.5, 0.0, 0.0) * s,
            2.0 * s,
        );
        sk.add_bone(
            "Left Leg",
            Some(torso),
            Vec3::new(-0.5, -2.0, 0.0) * s,
            2.0 * s,
        );
        sk.add_bone(
            "Right Leg",
            Some(torso),
            Vec3::new(0.5, -2.0, 0.0) * s,
            2.0 * s,
        );
        sk.bones[head].length = 1.0 * s;
        sk
    }

    /// Roblox R15 rig: upper/lower body chains and hands/feet.
    pub fn r15() -> Self {
        let s = STUD_TO_METER;
        let mut sk = Self::new("R15");
        let lower = sk.add_bone("LowerTorso", None, Vec3::ZERO, 0.8 * s);
        let upper = sk.add_bone(
            "UpperTorso",
            Some(lower),
            Vec3::new(0.0, 0.8, 0.0) * s,
            1.6 * s,
        );
        sk.add_bone("Head", Some(upper), Vec3::new(0.0, 1.4, 0.0) * s, 1.2 * s);
        let luarm = sk.add_bone(
            "LeftUpperArm",
            Some(upper),
            Vec3::new(-1.0, 0.9, 0.0) * s,
            1.2 * s,
        );
        let llarm = sk.add_bone(
            "LeftLowerArm",
            Some(luarm),
            Vec3::new(0.0, -1.2, 0.0) * s,
            1.2 * s,
        );
        sk.add_bone(
            "LeftHand",
            Some(llarm),
            Vec3::new(0.0, -1.2, 0.0) * s,
            0.5 * s,
        );
        let ruarm = sk.add_bone(
            "RightUpperArm",
            Some(upper),
            Vec3::new(1.0, 0.9, 0.0) * s,
            1.2 * s,
        );
        let rlarm = sk.add_bone(
            "RightLowerArm",
            Some(ruarm),
            Vec3::new(0.0, -1.2, 0.0) * s,
            1.2 * s,
        );
        sk.add_bone(
            "RightHand",
            Some(rlarm),
            Vec3::new(0.0, -1.2, 0.0) * s,
            0.5 * s,
        );
        let luleg = sk.add_bone(
            "LeftUpperLeg",
            Some(lower),
            Vec3::new(-0.5, -0.4, 0.0) * s,
            1.4 * s,
        );
        let llleg = sk.add_bone(
            "LeftLowerLeg",
            Some(luleg),
            Vec3::new(0.0, -1.4, 0.0) * s,
            1.4 * s,
        );
        sk.add_bone(
            "LeftFoot",
            Some(llleg),
            Vec3::new(0.0, -1.4, 0.1) * s,
            0.5 * s,
        );
        let ruleg = sk.add_bone(
            "RightUpperLeg",
            Some(lower),
            Vec3::new(0.5, -0.4, 0.0) * s,
            1.4 * s,
        );
        let rlleg = sk.add_bone(
            "RightLowerLeg",
            Some(ruleg),
            Vec3::new(0.0, -1.4, 0.0) * s,
            1.4 * s,
        );
        sk.add_bone(
            "RightFoot",
            Some(rlleg),
            Vec3::new(0.0, -1.4, 0.1) * s,
            0.5 * s,
        );
        sk
    }

    pub fn find(&self, name: &str) -> Option<usize> {
        self.bones
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case(name))
    }

    pub fn bone_names(&self) -> Vec<String> {
        self.bones.iter().map(|b| b.name.clone()).collect()
    }

    pub fn depth(&self, index: usize) -> usize {
        let mut depth = 0;
        let mut cur = self.bones.get(index).and_then(|b| b.parent);
        while let Some(p) = cur {
            depth += 1;
            cur = self.bones.get(p).and_then(|b| b.parent);
            if depth > 256 {
                break;
            }
        }
        depth
    }

    /// World rest matrix for a bone.
    pub fn rest_matrix(&self, index: usize) -> Mat4 {
        let mut chain = Vec::new();
        let mut cur = Some(index);
        let mut guard = 0;
        while let Some(i) = cur {
            let Some(bone) = self.bones.get(i) else { break };
            chain.push(Mat4::from_rotation_translation(
                bone.rest_rotation,
                bone.rest_offset,
            ));
            cur = bone.parent;
            guard += 1;
            if guard > 256 {
                break;
            }
        }
        let mut m = Mat4::IDENTITY;
        for t in chain.iter().rev() {
            m *= *t;
        }
        m
    }

    pub fn joint_positions(&self) -> Vec<Vec3> {
        (0..self.bones.len())
            .map(|i| self.rest_matrix(i).w_axis.truncate())
            .collect()
    }

    pub fn total_length(&self) -> f32 {
        self.bones.iter().map(|b| b.length).sum()
    }

    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let mut names = std::collections::BTreeSet::new();
        for (i, bone) in self.bones.iter().enumerate() {
            if bone.name.trim().is_empty() {
                problems.push(format!("Bone {i} has an empty name"));
            }
            if !names.insert(bone.name.to_lowercase()) {
                problems.push(format!("Duplicate bone name '{}'", bone.name));
            }
            if let Some(p) = bone.parent {
                if p >= self.bones.len() {
                    problems.push(format!("Bone '{}' has an invalid parent", bone.name));
                }
                if p >= i {
                    problems.push(format!(
                        "Bone '{}' has a parent that comes later in the list",
                        bone.name
                    ));
                }
            }
            if !bone.rest_offset.is_finite() {
                problems.push(format!("Bone '{}' has a non-finite offset", bone.name));
            }
        }
        problems
    }
}

// ---------------------------------------------------------------------- IK

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IkSolution {
    /// Middle joint position.
    pub joint: Vec3,
    /// Final end-effector position (clamped to reach when the target is far).
    pub end: Vec3,
    pub reached: bool,
}

/// Analytic two-bone IK. `pole` steers the elbow/knee plane.
pub fn solve_two_bone_ik(
    root: Vec3,
    target: Vec3,
    l1: f32,
    l2: f32,
    pole: Vec3,
) -> Option<IkSolution> {
    let l1 = l1.max(1e-5);
    let l2 = l2.max(1e-5);
    let to_target = target - root;
    let distance = to_target.length();
    if distance < 1e-6 {
        return None;
    }
    let dir = to_target / distance;
    let min_reach = (l1 - l2).abs() + 1e-4;
    let max_reach = l1 + l2 - 1e-4;
    let d = distance.clamp(min_reach, max_reach);
    let cos_angle = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let angle = cos_angle.acos();
    // Project the pole direction onto the plane perpendicular to the target axis.
    let mut pole_dir = pole - root;
    pole_dir -= dir * pole_dir.dot(dir);
    if pole_dir.length_squared() < 1e-8 {
        let fallback = if dir.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        pole_dir = fallback - dir * fallback.dot(dir);
    }
    let pole_dir = pole_dir.normalize_or_zero();
    let joint = root + (dir * angle.cos() + pole_dir * angle.sin()) * l1;
    let clamped_target = root + dir * d;
    let end = joint + (clamped_target - joint).normalize_or_zero() * l2;
    Some(IkSolution {
        joint,
        end,
        reached: (end - target).length() <= 1e-3 * (l1 + l2).max(1.0),
    })
}

/// Cyclic Coordinate Descent IK over a chain of joint positions (mutated in
/// place). Returns true when the end effector is within tolerance.
pub fn solve_ccd(points: &mut [Vec3], target: Vec3, iterations: u32) -> bool {
    if points.len() < 2 {
        return false;
    }
    let tolerance = 1e-3;
    let n = points.len();
    for _ in 0..iterations.max(1) {
        for i in (0..n - 1).rev() {
            let joint = points[i];
            let end = points[n - 1];
            let to_end = end - joint;
            let to_target = target - joint;
            if to_end.length_squared() < 1e-10 || to_target.length_squared() < 1e-10 {
                continue;
            }
            let rotation = Quat::from_rotation_arc(to_end.normalize(), to_target.normalize());
            for p in points.iter_mut().skip(i + 1) {
                *p = joint + rotation * (*p - joint);
            }
            if (points[n - 1] - target).length() < tolerance {
                return true;
            }
        }
    }
    (points[n - 1] - target).length() < tolerance
}

// -------------------------------------------------------------------- poses

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pose {
    pub name: String,
    pub rotations: HashMap<String, Quat>,
}

impl Pose {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rotations: HashMap::new(),
        }
    }

    pub fn set(&mut self, bone: impl Into<String>, rotation: Quat) {
        self.rotations.insert(bone.into(), rotation);
    }

    pub fn get(&self, bone: &str) -> Option<Quat> {
        self.rotations.get(bone).copied()
    }

    pub fn bones(&self) -> Vec<&String> {
        self.rotations.keys().collect()
    }

    /// Spherical blend between two poses.
    pub fn blend(&self, other: &Pose, t: f32) -> Pose {
        let t = t.clamp(0.0, 1.0);
        let mut out = self.clone();
        for (bone, b) in &other.rotations {
            let a = self.rotations.get(bone).copied().unwrap_or(Quat::IDENTITY);
            out.rotations.insert(bone.clone(), a.slerp(*b, t));
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.rotations.is_empty()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PoseLibrary {
    pub poses: Vec<Pose>,
}

impl PoseLibrary {
    pub fn add(&mut self, pose: Pose) {
        if let Some(existing) = self.poses.iter_mut().find(|p| p.name == pose.name) {
            *existing = pose;
        } else {
            self.poses.push(pose);
        }
    }

    pub fn find(&self, name: &str) -> Option<&Pose> {
        self.poses.iter().find(|p| p.name == name)
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.poses.len();
        self.poses.retain(|p| p.name != name);
        before != self.poses.len()
    }

    pub fn blend(&self, a: &str, b: &str, t: f32) -> Option<Pose> {
        let pa = self.find(a)?;
        let pb = self.find(b)?;
        Some(pa.blend(pb, t))
    }
}

/// Convenience aliases between Roblox R6 part names and R15 joint names.
fn alias_name(name: &str) -> &str {
    match name {
        "Torso" => "UpperTorso",
        "Left Arm" => "LeftUpperArm",
        "Right Arm" => "RightUpperArm",
        "Left Leg" => "LeftUpperLeg",
        "Right Leg" => "RightUpperLeg",
        other => other,
    }
}

fn resolve_bone(dst: &Skeleton, name: &str, mirror: bool) -> Option<usize> {
    let swapped = if mirror {
        mirror_name(name)
    } else {
        name.to_string()
    };
    dst.find(&swapped)
        .or_else(|| dst.find(alias_name(&swapped)))
        .or_else(|| dst.find(name))
        .or_else(|| dst.find(alias_name(name)))
}

/// Copies rotations from a pose onto another skeleton, matching by bone name.
/// `mirror` swaps Left/Right names, which is what R6 <-> R15 work usually needs.
/// Returns the new pose plus warnings for bones that could not be matched.
pub fn retarget_pose(
    src: &Skeleton,
    dst: &Skeleton,
    pose: &Pose,
    mirror: bool,
) -> (Pose, Vec<String>) {
    let mut out = Pose::new(format!("{} ({})", pose.name, dst.name));
    let mut warnings = Vec::new();
    let mut matched = std::collections::BTreeSet::new();
    for (bone, rotation) in &pose.rotations {
        if let Some(index) = resolve_bone(dst, bone, mirror) {
            out.set(dst.bones[index].name.clone(), *rotation);
            matched.insert(index);
        } else {
            warnings.push(format!(
                "'{bone}' from {} has no counterpart in {}",
                src.name, dst.name
            ));
        }
    }
    for (i, bone) in dst.bones.iter().enumerate() {
        if !matched.contains(&i) {
            warnings.push(format!("'{}' keeps its rest rotation", bone.name));
        }
    }
    (out, warnings)
}

fn mirror_name(name: &str) -> String {
    if name.contains("Left") {
        name.replace("Left", "Right")
    } else if name.contains("Right") {
        name.replace("Right", "Left")
    } else {
        name.to_string()
    }
}

/// World matrices for every bone after applying a pose (missing bones keep
/// their rest rotation).
pub fn apply_pose(skeleton: &Skeleton, pose: &Pose) -> Vec<Mat4> {
    let mut world: Vec<Mat4> = Vec::with_capacity(skeleton.bones.len());
    for bone in skeleton.bones.iter() {
        let local_rotation = pose.get(&bone.name).unwrap_or(bone.rest_rotation);
        let local = Mat4::from_rotation_translation(local_rotation, bone.rest_offset);
        let matrix = match bone.parent {
            Some(p) if p < world.len() => world[p] * local,
            _ => local,
        };
        world.push(matrix);
    }
    world
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rig_skeletons_are_well_formed() {
        for sk in [Skeleton::r6(), Skeleton::r15()] {
            assert!(sk.validate().is_empty(), "{:?}", sk.validate());
            assert!(!sk.bones.is_empty());
            assert!(sk.total_length() > 0.0);
            for (i, bone) in sk.bones.iter().enumerate() {
                if let Some(p) = bone.parent {
                    assert!(p < i, "parents must precede children");
                }
            }
        }
        assert_eq!(Skeleton::r6().bones.len(), 6);
        assert_eq!(Skeleton::r15().bones.len(), 15);
    }

    #[test]
    fn rest_matrices_are_finite() {
        let sk = Skeleton::r15();
        for i in 0..sk.bones.len() {
            let m = sk.rest_matrix(i);
            assert!(m.to_cols_array().iter().all(|v| v.is_finite()));
        }
    }

    #[test]
    fn two_bone_ik_reaches_target() {
        let root = Vec3::ZERO;
        let target = Vec3::new(0.6, 0.4, 0.0);
        let sol = solve_two_bone_ik(root, target, 0.5, 0.5, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        assert!(sol.reached, "end {:?}", sol.end);
        assert!((sol.end - target).length() < 1e-3);
        // Joint stays at the correct distance from the root.
        assert!((sol.joint.length() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn two_bone_ik_clamps_out_of_reach() {
        let sol =
            solve_two_bone_ik(Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0), 0.5, 0.5, Vec3::Y).unwrap();
        assert!(!sol.reached);
        assert!((sol.end.length() - 1.0).abs() < 1e-2);
    }

    #[test]
    fn ccd_chain_converges() {
        let mut chain = vec![
            Vec3::ZERO,
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.0, -1.5, 0.0),
        ];
        let target = Vec3::new(0.9, -0.9, 0.0);
        let reached = solve_ccd(&mut chain, target, 40);
        assert!(reached, "end {:?}", chain.last());
        // Segment lengths are preserved.
        for w in chain.windows(2) {
            assert!((w[0].distance(w[1]) - 0.5).abs() < 1e-3);
        }
    }

    #[test]
    fn poses_blend_and_apply() {
        let sk = Skeleton::r6();
        let mut a = Pose::new("A");
        a.set("Head", Quat::IDENTITY);
        let mut b = Pose::new("B");
        b.set("Head", Quat::from_rotation_y(1.0));
        let mid = a.blend(&b, 0.5);
        let rot = mid.get("Head").unwrap();
        assert!((rot.to_euler(glam::EulerRot::XYZ).1 - 0.5).abs() < 1e-3);

        let rest = apply_pose(&sk, &Pose::new("rest"));
        let rest_direct: Vec<Mat4> = (0..sk.bones.len()).map(|i| sk.rest_matrix(i)).collect();
        for (a, b) in rest.iter().zip(rest_direct.iter()) {
            assert!(
                (a.to_cols_array()
                    .iter()
                    .zip(b.to_cols_array().iter())
                    .map(|(x, y)| x - y)
                    .fold(0.0f32, |acc, d| acc.max(d.abs())))
                    < 1e-5
            );
        }
        let posed = apply_pose(&sk, &mid);
        assert_ne!(posed[1].to_cols_array(), rest[1].to_cols_array());
    }

    #[test]
    fn retarget_matches_names_and_warns() {
        let r6 = Skeleton::r6();
        let r15 = Skeleton::r15();
        let mut pose = Pose::new("Wave");
        pose.set("Right Arm", Quat::from_rotation_z(1.0));
        pose.set("Tail", Quat::IDENTITY);
        let (retargeted, warnings) = retarget_pose(&r6, &r15, &pose, true);
        // "Right Arm" mirrors to "Left Arm" via the R6 -> R15 alias table.
        assert!(warnings.iter().any(|w| w.contains("Tail")));
        assert!(retargeted.get("LeftUpperArm").is_some());

        pose.set("LeftUpperArm", Quat::from_rotation_z(1.0));
        let (retargeted, _) = retarget_pose(&r6, &r15, &pose, false);
        assert!(retargeted.get("LeftUpperArm").is_some());
        assert!(warnings.iter().all(|w| !w.contains("Right Arm")));
    }

    #[test]
    fn pose_library_operations() {
        let mut lib = PoseLibrary::default();
        lib.add(Pose::new("Rest"));
        let mut t = Pose::new("T");
        t.set("Head", Quat::from_rotation_x(0.4));
        lib.add(t);
        assert_eq!(lib.poses.len(), 2);
        lib.add(Pose::new("Rest"));
        assert_eq!(lib.poses.len(), 2);
        assert!(lib.blend("Rest", "T", 0.5).is_some());
        assert!(lib.remove("Rest"));
        assert!(!lib.remove("Nope"));
    }
}
