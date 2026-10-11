//! Frame evaluation: turn a document at a frame into a flat draw list.

use glam::Affine2;

use crate::path::Shape;
use crate::scene::{Document, Layer, LayerId, LayerKind};

/// One shape to draw, with everything resolved for the frame.
#[derive(Clone, Copy, Debug)]
pub struct DrawItem<'a> {
    pub layer: LayerId,
    pub shape: &'a Shape,
    /// Shape space to document space.
    pub transform: Affine2,
    /// Combined opacity of the shape's layer and all its ancestors.
    pub opacity: f32,
}

/// Evaluate `doc` at `frame` (which may be fractional, for motion blur or
/// smooth playback) into draw items, back to front.
pub fn evaluate(doc: &Document, frame: f32) -> Vec<DrawItem<'_>> {
    let mut out = Vec::new();
    Evaluator::default().evaluate_into(doc, frame, &mut out);
    out
}

/// Reusable evaluator. Keeps its scratch buffers between frames so playback
/// does not allocate.
#[derive(Default)]
pub struct Evaluator {
    /// Per bone-layer nesting depth: skinning matrix for each bone
    /// (current pose times inverse rest pose).
    skins: Vec<Vec<Affine2>>,
    pose: Vec<Affine2>,
    rest: Vec<Affine2>,
}

/// The bone layer a subtree is inside.
#[derive(Clone, Copy)]
struct Rig {
    /// Bone layer space to document space.
    world: Affine2,
    /// Index into `Evaluator::skins`.
    depth: usize,
}

impl Evaluator {
    pub fn evaluate_into<'a>(&mut self, doc: &'a Document, frame: f32, out: &mut Vec<DrawItem<'a>>) {
        out.clear();
        self.visit(&doc.layers, frame, Affine2::IDENTITY, 1.0, None, out);
    }

    /// `world` is the parent's space to document space. Inside a bone layer,
    /// `rig` is set and `world` is still kept up to date; `rel` (parent
    /// space to bone layer space) is recovered from both when needed.
    fn visit<'a>(
        &mut self,
        layers: &'a [Layer],
        frame: f32,
        world: Affine2,
        opacity: f32,
        rig: Option<(Rig, Affine2)>,
        out: &mut Vec<DrawItem<'a>>,
    ) {
        for layer in layers {
            if !layer.visible {
                continue;
            }
            let opacity = opacity * layer.opacity.sample(frame).clamp(0.0, 1.0);
            if opacity <= 0.0 {
                continue;
            }
            let local = layer.transform.sample(frame).to_affine();
            let world = world * local;
            // rel: this layer's space to the enclosing bone layer's space
            let rig = rig.map(|(r, rel)| (r, rel * local));

            match &layer.kind {
                LayerKind::Vector { shapes } => {
                    for shape in shapes {
                        let transform = match (rig, shape.bone) {
                            (Some((r, rel)), Some(b)) => match self.skins[r.depth].get(b) {
                                Some(skin) => r.world * *skin * rel,
                                None => world,
                            },
                            _ => world,
                        };
                        out.push(DrawItem {
                            layer: layer.id,
                            shape,
                            transform,
                            opacity,
                        });
                    }
                }
                LayerKind::Group { children } => {
                    self.visit(children, frame, world, opacity, rig, out);
                }
                LayerKind::Bone { skeleton, children } => {
                    let depth = rig.map_or(0, |(r, _)| r.depth + 1);
                    if self.skins.len() <= depth {
                        self.skins.resize_with(depth + 1, Vec::new);
                    }
                    skeleton.pose(frame, &mut self.pose);
                    skeleton.rest_pose(&mut self.rest);
                    let skins = &mut self.skins[depth];
                    skins.clear();
                    skins.extend(self.pose.iter().zip(&self.rest).map(|(p, r)| *p * r.inverse()));
                    let inner = Rig { world, depth };
                    self.visit(
                        children,
                        frame,
                        world,
                        opacity,
                        Some((inner, Affine2::IDENTITY)),
                        out,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::Interpolation;
    use crate::bones::Skeleton;
    use crate::path::{Path, Style};
    use crate::transform::AnimTransform;
    use crate::{Color, Vec2};
    use std::f32::consts::FRAC_PI_2;

    fn square() -> Shape {
        Shape::new(
            "sq",
            Path::rect(Vec2::ZERO, Vec2::splat(1.0), 0.0),
            Style::fill(Color::BLACK),
        )
    }

    #[test]
    fn nested_transforms_compose() {
        let mut doc = Document::new("t", 100.0, 100.0, 24.0);
        let inner = doc
            .new_layer("v", LayerKind::vector(vec![square()]))
            .with_transform(AnimTransform::at(Vec2::new(5.0, 0.0)));
        let group = doc
            .new_layer(
                "g",
                LayerKind::Group {
                    children: vec![inner],
                },
            )
            .with_transform(AnimTransform::at(Vec2::new(10.0, 0.0)));
        doc.layers.push(group);

        let items = evaluate(&doc, 0.0);
        assert_eq!(items.len(), 1);
        let p = items[0].transform.transform_point2(Vec2::ZERO);
        assert!(p.abs_diff_eq(Vec2::new(15.0, 0.0), 1e-5));
    }

    #[test]
    fn hidden_and_transparent_layers_are_skipped() {
        let mut doc = Document::new("t", 100.0, 100.0, 24.0);
        let mut hidden = doc.new_layer("h", LayerKind::vector(vec![square()]));
        hidden.visible = false;
        let mut clear = doc.new_layer("c", LayerKind::vector(vec![square()]));
        clear.opacity.base = 0.0;
        doc.layers.extend([hidden, clear]);
        assert!(evaluate(&doc, 0.0).is_empty());
    }

    #[test]
    fn bound_shape_follows_its_bone() {
        let mut doc = Document::new("t", 100.0, 100.0, 24.0);
        let mut skel = Skeleton::default();
        let b = skel.add("b", None, Vec2::new(50.0, 50.0), 10.0, 0.0);
        skel.key_angle(b, 0, 0.0, Interpolation::Linear);
        skel.key_angle(b, 10, FRAC_PI_2, Interpolation::Linear);

        // a point on the bone, 10 units along it, in rest pose
        let dot = Shape::new(
            "dot",
            Path::new().move_to(Vec2::new(60.0, 50.0)),
            Style::default(),
        )
        .bound_to(b);
        let shapes = doc.new_layer("art", LayerKind::vector(vec![dot]));
        let rig = doc.new_layer(
            "rig",
            LayerKind::Bone {
                skeleton: skel,
                children: vec![shapes],
            },
        );
        doc.layers.push(rig);

        let at = |frame: f32| {
            let items = evaluate(&doc, frame);
            items[0].transform.transform_point2(Vec2::new(60.0, 50.0))
        };
        assert!(at(0.0).abs_diff_eq(Vec2::new(60.0, 50.0), 1e-4));
        // quarter turn about the joint at (50, 50) moves the point to (50, 60)
        assert!(at(10.0).abs_diff_eq(Vec2::new(50.0, 60.0), 1e-4));
    }

    #[test]
    fn missing_bone_falls_back_to_layer_transform() {
        let mut doc = Document::new("t", 100.0, 100.0, 24.0);
        let shapes = doc.new_layer("art", LayerKind::vector(vec![square().bound_to(7)]));
        let rig = doc.new_layer(
            "rig",
            LayerKind::Bone {
                skeleton: Skeleton::default(),
                children: vec![shapes],
            },
        );
        doc.layers.push(rig);
        let items = evaluate(&doc, 0.0);
        assert_eq!(items[0].transform, Affine2::IDENTITY);
    }
}
