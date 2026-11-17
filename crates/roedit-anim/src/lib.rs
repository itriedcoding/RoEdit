//! Rigging and animation utilities: skeletons for Roblox R6/R15, analytic
//! two-bone IK, CCD IK, pose libraries, pose blending and retargeting.

pub mod rig;

pub use rig::{
    apply_pose, retarget_pose, solve_ccd, solve_two_bone_ik, Bone, IkSolution, Pose, PoseLibrary,
    Skeleton,
};

/// Roblox avatar scale in metres per stud, matching `roedit_core`.
pub const STUD_TO_METER: f32 = roedit_core::STUD_TO_METER;
