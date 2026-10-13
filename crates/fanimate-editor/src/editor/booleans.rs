//! Union, Subtract and Intersect on the shapes that have selected points.
//!
//! Like Illustrator's Pathfinder: Union and Intersect make one shape with
//! the topmost shape's style, where the topmost was; Subtract cuts the
//! topmost shape out of each of the others.

use std::collections::BTreeSet;

use fanimate_core::boolean::{self, BoolOp};
use fanimate_core::Shape;

use super::{Editor, StatusKind};

/// How far a combined outline may stray from the exact one, in document
/// units: well below a pixel at 100%.
const TOLERANCE: f32 = 0.25;

impl Editor {
    /// Shapes the boolean commands would act on: those with selected
    /// points, on the point layer, bottom first.
    fn boolean_operands(&self) -> Vec<usize> {
        let layer = self.point_layer();
        let shapes: BTreeSet<usize> = self
            .points
            .iter()
            .filter(|r| Some(r.layer) == layer)
            .map(|r| r.shape)
            .collect();
        shapes.into_iter().collect()
    }

    pub(super) fn can_combine(&self) -> bool {
        self.boolean_operands().len() >= 2
    }

    /// Combine the shapes with selected points. One undo step. Returns
    /// whether anything changed.
    pub(super) fn combine_shapes(&mut self, op: BoolOp) -> bool {
        let operands = self.boolean_operands();
        let Some(layer) = self.point_layer() else {
            return false;
        };
        if operands.len() < 2 {
            self.set_status(
                StatusKind::Info,
                "Select points on two or more shapes, then combine them",
            );
            return false;
        }
        let Some(shapes) = self.layer_shapes(layer) else {
            return false;
        };
        let picked: Vec<&Shape> = operands.iter().filter_map(|&i| shapes.get(i)).collect();
        if picked.iter().any(|s| s.is_animated()) {
            self.set_status(
                StatusKind::Warning,
                "Shapes with point keyframes can't be combined; they would lose their animation",
            );
            return false;
        }
        if picked
            .iter()
            .any(|s| s.widths.is_some() && s.style.fill.is_none())
        {
            self.set_status(
                StatusKind::Warning,
                "Brush strokes can't be combined, only shapes",
            );
            return false;
        }
        if picked.iter().any(|s| s.bone != picked[0].bone) {
            self.set_status(
                StatusKind::Warning,
                "These shapes follow different bones, so they can't be combined",
            );
            return false;
        }
        let top = picked[picked.len() - 1];
        // (index to replace, shapes to put there), bottom first
        let mut results: Vec<(usize, Option<Shape>)> = Vec::new();
        match op {
            BoolOp::Union | BoolOp::Intersect => {
                let paths: Vec<_> = picked.iter().map(|s| &s.path).collect();
                let path = boolean::combine(&paths, op, TOLERANCE);
                if path.cmds.is_empty() {
                    self.set_status(StatusKind::Info, "The shapes don't overlap, so nothing is left");
                    return false;
                }
                let mut shape = top.clone();
                shape.path = path;
                shape.widths = None;
                for &i in &operands[..operands.len() - 1] {
                    results.push((i, None));
                }
                results.push((operands[operands.len() - 1], Some(shape)));
            }
            BoolOp::Subtract => {
                for (k, s) in picked[..picked.len() - 1].iter().enumerate() {
                    let path = boolean::combine(&[&s.path, &top.path], op, TOLERANCE);
                    let shape = (!path.cmds.is_empty()).then(|| {
                        let mut shape = (*s).clone();
                        shape.path = path;
                        shape.widths = None;
                        shape
                    });
                    results.push((operands[k], shape));
                }
                results.push((operands[operands.len() - 1], None));
            }
        }
        let n = operands.len();
        self.checkpoint(match op {
            BoolOp::Union => "Union shapes",
            BoolOp::Subtract => "Subtract shape",
            BoolOp::Intersect => "Intersect shapes",
        });
        if let Some(shapes) = self.layer_shapes_mut(layer) {
            // back to front, so earlier indices stay valid
            for (i, shape) in results.into_iter().rev() {
                match shape {
                    Some(shape) => shapes[i] = shape,
                    None => {
                        shapes.remove(i);
                    }
                }
            }
        }
        self.points.clear();
        self.set_status(StatusKind::Info, format!("Combined {n} shapes"));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fanimate_core::LayerKind;
    use crate::{Action, Tool};
    use fanimate_core::{LayerId, Vec2};

    /// Two overlapping 100x100 squares, all their points selected.
    fn editor() -> (Editor, LayerId) {
        let mut ed = Editor::untitled();
        let id = ed.primary().unwrap();
        for at in [Vec2::ZERO, Vec2::splat(50.0)] {
            ed.add_drag_shape(Tool::Rectangle, at, at + Vec2::splat(100.0), false, false, 1.0);
        }
        ed.tool = Tool::SelectPoints;
        ed.select_all_points();
        (ed, id)
    }

    fn shapes(ed: &Editor, id: LayerId) -> Vec<Shape> {
        match &ed.doc.find(id).unwrap().kind {
            LayerKind::Vector { shapes } => shapes.clone(),
            _ => panic!(),
        }
    }

    #[test]
    fn union_makes_one_shape_in_one_undo_step() {
        let (mut ed, id) = editor();
        assert!(ed.can_run(Action::Union));
        ed.run(Action::Union);
        let s = shapes(&ed, id);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "Rectangle 2");
        assert_eq!(s[0].path.on_curve_count(), 8);
        assert!(s[0].contains(Vec2::splat(10.0), 0.0) && s[0].contains(Vec2::splat(140.0), 0.0));
        ed.run(Action::Undo);
        assert_eq!(shapes(&ed, id).len(), 2);
    }

    #[test]
    fn subtract_cuts_the_top_shape_out() {
        let (mut ed, id) = editor();
        ed.run(Action::Subtract);
        let s = shapes(&ed, id);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "Rectangle 1");
        assert!(s[0].contains(Vec2::splat(10.0), 0.0));
        assert!(!s[0].contains(Vec2::splat(75.0), 0.0));
    }

    #[test]
    fn intersect_keeps_the_overlap_and_refuses_when_there_is_none() {
        let (mut ed, id) = editor();
        ed.run(Action::Intersect);
        let s = shapes(&ed, id);
        assert_eq!(s.len(), 1);
        assert!(s[0].contains(Vec2::splat(75.0), 0.0));
        assert!(!s[0].contains(Vec2::splat(10.0), 0.0));

        let (mut ed, id) = editor();
        ed.add_drag_shape(
            Tool::Rectangle,
            Vec2::splat(500.0),
            Vec2::splat(510.0),
            false,
            false,
            1.0,
        );
        ed.select_all_points();
        let steps = ed.history.undo_label().map(str::to_owned);
        ed.run(Action::Intersect);
        assert_eq!(shapes(&ed, id).len(), 3);
        assert_eq!(ed.history.undo_label().map(str::to_owned), steps);
    }

    #[test]
    fn needs_two_shapes_and_leaves_animated_shapes_alone() {
        let (mut ed, id) = editor();
        let first: BTreeSet<_> = ed.points.iter().filter(|r| r.shape == 0).copied().collect();
        ed.points = first;
        assert!(!ed.can_run(Action::Union));
        ed.select_all_points();
        ed.run(Action::AddKey);
        ed.run(Action::Union);
        assert_eq!(shapes(&ed, id).len(), 2);
        assert!(matches!(ed.status, Some((StatusKind::Warning, _))));
    }
}
