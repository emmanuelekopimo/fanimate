//! Fanimate core engine.
//!
//! The scene graph, animation curves, bones and frame evaluation. This crate
//! has no UI or GPU code, so the same engine runs in the desktop app, in the
//! browser and headless on a render server.
//!
//! The main entry points are [`Document`], which holds a project, and
//! [`eval::evaluate`], which turns a document at a given frame into a flat
//! list of shapes ready to draw.

pub mod anim;
pub mod bones;
pub mod color;
pub mod eval;
pub mod path;
pub mod samples;
pub mod scene;
pub mod transform;

pub use anim::{Channel, Frame, Interpolation, Key};
pub use bones::{Bone, Skeleton};
pub use color::Color;
pub use eval::{DrawItem, evaluate};
pub use glam::{Affine2, Vec2};
pub use path::{Path, PathCmd, Shape, Stroke, Style};
pub use scene::{Document, Layer, LayerId, LayerKind};
pub use transform::{AnimTransform, Transform};
