//! Bones and forward kinematics.
//!
//! A bone layer owns a [`Skeleton`]. Shapes on vector layers inside the bone
//! layer can bind to a bone and then follow it. This spike implements
//! forward kinematics and rigid binding; IK, Smart Bones and weighted mesh
//! deformation come in milestone M3.

use glam::{Affine2, Vec2};
use serde::{Deserialize, Serialize};

use crate::anim::{Channel, Frame, Interpolation};

/// One bone. Its local space has the origin at the joint and +x running
/// along the bone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bone {
    pub name: String,
    /// Index of the parent bone. Parents always come before children.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<usize>,
    /// Joint position in the parent's bone space, or in the bone layer's
    /// space for a root bone.
    pub origin: Vec2,
    pub length: f32,
    /// Rotation relative to the parent, in radians. The channel's `base`
    /// value is the rest pose.
    pub angle: Channel<f32>,
}

impl Bone {
    pub fn local(&self, angle: f32) -> Affine2 {
        Affine2::from_angle_translation(angle, self.origin)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    /// Add a bone and return its index.
    ///
    /// # Panics
    /// If `parent` does not refer to an existing bone.
    pub fn add(
        &mut self,
        name: impl Into<String>,
        parent: Option<usize>,
        origin: Vec2,
        length: f32,
        rest_angle: f32,
    ) -> usize {
        if let Some(p) = parent {
            assert!(p < self.bones.len(), "parent bone {p} does not exist");
        }
        self.bones.push(Bone {
            name: name.into(),
            parent,
            origin,
            length,
            angle: Channel::new(rest_angle),
        });
        self.bones.len() - 1
    }

    pub fn key_angle(&mut self, bone: usize, frame: Frame, angle: f32, interp: Interpolation) {
        self.bones[bone].angle.set_key(frame, angle, interp);
    }

    /// World (bone-layer space) transform of every bone at `frame`.
    pub fn pose(&self, frame: f32, out: &mut Vec<Affine2>) {
        self.solve(out, |b| b.angle.sample(frame));
    }

    /// World transform of every bone in the rest pose.
    pub fn rest_pose(&self, out: &mut Vec<Affine2>) {
        self.solve(out, |b| b.angle.base);
    }

    fn solve(&self, out: &mut Vec<Affine2>, angle: impl Fn(&Bone) -> f32) {
        out.clear();
        out.reserve(self.bones.len());
        for bone in &self.bones {
            let local = bone.local(angle(bone));
            // Parents precede children (enforced by `add`, checked by the
            // validator), so the parent's world transform is already known.
            // A bad parent index is treated as a root rather than panicking.
            let world = match bone.parent.and_then(|p| out.get(p)) {
                Some(parent) => *parent * local,
                None => local,
            };
            out.push(world);
        }
    }

    /// Where a bone's tip is, given world transforms from [`pose`](Self::pose).
    pub fn tip(&self, poses: &[Affine2], bone: usize) -> Vec2 {
        poses[bone].transform_point2(Vec2::new(self.bones[bone].length, 0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_2;

    fn arm() -> Skeleton {
        let mut s = Skeleton::default();
        let upper = s.add("upper", None, Vec2::ZERO, 10.0, 0.0);
        s.add("lower", Some(upper), Vec2::new(10.0, 0.0), 10.0, 0.0);
        s
    }

    #[test]
    fn rest_pose_chains_along_x() {
        let s = arm();
        let mut poses = Vec::new();
        s.rest_pose(&mut poses);
        assert!(s.tip(&poses, 1).abs_diff_eq(Vec2::new(20.0, 0.0), 1e-5));
    }

    #[test]
    fn rotating_parent_carries_child() {
        let mut s = arm();
        s.key_angle(0, 0, FRAC_PI_2, Interpolation::Linear);
        let mut poses = Vec::new();
        s.pose(0.0, &mut poses);
        // upper arm points down (+y), lower arm continues it
        assert!(s.tip(&poses, 1).abs_diff_eq(Vec2::new(0.0, 20.0), 1e-4));
    }

    #[test]
    fn bad_parent_index_is_treated_as_root() {
        let mut s = arm();
        s.bones[1].parent = Some(99);
        let mut poses = Vec::new();
        s.rest_pose(&mut poses);
        assert!(s.tip(&poses, 1).abs_diff_eq(Vec2::new(20.0, 0.0), 1e-5));
    }
}
