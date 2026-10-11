//! 2D transforms: position, rotation and scale, static or animated.

use glam::{Affine2, Vec2};
use serde::{Deserialize, Serialize};

use crate::anim::Channel;

/// A transform at one moment. Rotation is in radians; because document space
/// has y pointing down, a positive rotation turns clockwise on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub position: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
}

impl Transform {
    pub const IDENTITY: Self = Self {
        position: Vec2::ZERO,
        rotation: 0.0,
        scale: Vec2::ONE,
    };

    pub fn to_affine(self) -> Affine2 {
        Affine2::from_scale_angle_translation(self.scale, self.rotation, self.position)
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A layer's transform, with each part animatable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimTransform {
    pub position: Channel<Vec2>,
    pub rotation: Channel<f32>,
    pub scale: Channel<Vec2>,
}

impl AnimTransform {
    pub fn at(position: Vec2) -> Self {
        Self {
            position: Channel::new(position),
            ..Self::default()
        }
    }

    pub fn sample(&self, frame: f32) -> Transform {
        Transform {
            position: self.position.sample(frame),
            rotation: self.rotation.sample(frame),
            scale: self.scale.sample(frame),
        }
    }

    pub fn is_animated(&self) -> bool {
        self.position.is_animated() || self.rotation.is_animated() || self.scale.is_animated()
    }
}

impl Default for AnimTransform {
    fn default() -> Self {
        Self {
            position: Channel::new(Vec2::ZERO),
            rotation: Channel::new(0.0),
            scale: Channel::new(Vec2::ONE),
        }
    }
}
