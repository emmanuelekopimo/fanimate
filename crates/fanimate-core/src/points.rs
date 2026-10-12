//! Editing a path point by point: the Select Points and Transform Points
//! tools.
//!
//! Points are numbered as in a width profile: one per on-curve point (the
//! end of each move, line and cubic command), in order. A point's curve
//! handles (the control points either side of it) move with it, so moving
//! a point bends only the segments it touches.

use std::collections::BTreeSet;

use glam::Vec2;

use crate::path::{Path, PathCmd, Shape};

impl Path {
    /// The on-curve points, in order.
    pub fn on_curve_points(&self) -> Vec<Vec2> {
        self.cmds
            .iter()
            .filter_map(|c| match *c {
                PathCmd::MoveTo(p) | PathCmd::LineTo(p) | PathCmd::CubicTo(_, _, p) => Some(p),
                PathCmd::Close => None,
            })
            .collect()
    }

    /// Index into `cmds` of each on-curve point.
    fn point_cmds(&self) -> Vec<usize> {
        (0..self.cmds.len())
            .filter(|&i| !matches!(self.cmds[i], PathCmd::Close))
            .collect()
    }

    /// Each subpath as a range of on-curve point numbers, and whether it is
    /// closed.
    fn subpaths(&self) -> Vec<(std::ops::Range<usize>, bool)> {
        let mut out: Vec<(std::ops::Range<usize>, bool)> = Vec::new();
        let mut k = 0;
        for cmd in &self.cmds {
            match cmd {
                PathCmd::MoveTo(_) => {
                    out.push((k..k + 1, false));
                    k += 1;
                }
                PathCmd::LineTo(_) | PathCmd::CubicTo(..) => {
                    if let Some(last) = out.last_mut() {
                        last.0.end = k + 1;
                    }
                    k += 1;
                }
                PathCmd::Close => {
                    if let Some(last) = out.last_mut() {
                        last.1 = true;
                    }
                }
            }
        }
        out
    }

    /// `points` plus their twins: a closed subpath whose last point lies on
    /// its first (as circles do) has two numbers for one visible point.
    fn with_twins(&self, points: &BTreeSet<usize>) -> BTreeSet<usize> {
        let pts = self.on_curve_points();
        let mut out = points.clone();
        for (range, closed) in self.subpaths() {
            let (first, last) = (range.start, range.end - 1);
            if closed && last > first && pts[first] == pts[last] {
                if points.contains(&first) {
                    out.insert(last);
                }
                if points.contains(&last) {
                    out.insert(first);
                }
            }
        }
        out
    }

    /// Move the given points, and their handles, by `delta`.
    pub fn move_points(&mut self, points: &BTreeSet<usize>, delta: Vec2) {
        let points = self.with_twins(points);
        let at = self.point_cmds();
        for &k in &points {
            let Some(&i) = at.get(k) else { continue };
            match &mut self.cmds[i] {
                PathCmd::MoveTo(p) | PathCmd::LineTo(p) => *p += delta,
                PathCmd::CubicTo(_, c2, p) => {
                    *c2 += delta;
                    *p += delta;
                }
                PathCmd::Close => {}
            }
            // the handle leaving this point belongs to it too
            if let Some(PathCmd::CubicTo(c1, _, _)) = self.cmds.get_mut(i + 1) {
                *c1 += delta;
            }
        }
    }
}

impl Shape {
    /// Remove the given points. Segments either side of a removed point join
    /// up; a subpath left with fewer than two points goes, and a closed one
    /// left with two opens. The width profile keeps step. Returns false when
    /// nothing is left of the shape.
    pub fn remove_points(&mut self, points: &BTreeSet<usize>) -> bool {
        let remove = self.path.with_twins(points);
        let at = self.path.point_cmds();
        let widths = self.widths.take().filter(|w| w.len() == at.len());
        let mut cmds = Vec::new();
        let mut kept_widths = Vec::new();
        for (range, closed) in self.path.subpaths() {
            let kept: Vec<usize> = range.filter(|k| !remove.contains(k)).collect();
            if kept.len() < 2 {
                continue;
            }
            for (n, &k) in kept.iter().enumerate() {
                let cmd = self.path.cmds[at[k]];
                cmds.push(if n == 0 {
                    // a subpath starts with a move to its first kept point
                    match cmd {
                        PathCmd::MoveTo(p) | PathCmd::LineTo(p) | PathCmd::CubicTo(_, _, p) => {
                            PathCmd::MoveTo(p)
                        }
                        PathCmd::Close => continue,
                    }
                } else if let PathCmd::MoveTo(p) = cmd {
                    PathCmd::LineTo(p)
                } else {
                    cmd
                });
                if let Some(w) = &widths {
                    kept_widths.push(w[k]);
                }
            }
            if closed && kept.len() > 2 {
                cmds.push(PathCmd::Close);
            }
        }
        self.path = Path { cmds };
        self.widths = widths.map(|_| kept_widths);
        !self.path.cmds.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    use crate::path::Style;

    fn set(points: &[usize]) -> BTreeSet<usize> {
        points.iter().copied().collect()
    }

    #[test]
    fn moving_a_point_moves_its_handles() {
        let mut path = Path::new()
            .move_to(Vec2::ZERO)
            .cubic_to(Vec2::new(1.0, 1.0), Vec2::new(2.0, 1.0), Vec2::new(3.0, 0.0))
            .cubic_to(Vec2::new(4.0, -1.0), Vec2::new(5.0, -1.0), Vec2::new(6.0, 0.0));
        path.move_points(&set(&[1]), Vec2::new(0.0, 10.0));
        assert_eq!(
            path.cmds[1],
            PathCmd::CubicTo(Vec2::new(1.0, 1.0), Vec2::new(2.0, 11.0), Vec2::new(3.0, 10.0))
        );
        assert_eq!(
            path.cmds[2],
            PathCmd::CubicTo(Vec2::new(4.0, 9.0), Vec2::new(5.0, -1.0), Vec2::new(6.0, 0.0))
        );
    }

    #[test]
    fn a_circle_moves_its_start_and_end_together() {
        let mut path = Path::circle(Vec2::ZERO, 10.0);
        assert_eq!(path.on_curve_points().len(), 5);
        path.move_points(&set(&[0]), Vec2::new(5.0, 0.0));
        let pts = path.on_curve_points();
        assert_eq!(pts[0], Vec2::new(15.0, 0.0));
        assert_eq!(pts[4], Vec2::new(15.0, 0.0));
    }

    #[test]
    fn removing_points_joins_segments_and_keeps_widths_in_step() {
        let mut shape = Shape::new(
            "s",
            Path::new()
                .move_to(Vec2::ZERO)
                .line_to(Vec2::new(1.0, 0.0))
                .line_to(Vec2::new(2.0, 0.0))
                .line_to(Vec2::new(3.0, 0.0)),
            Style::default().with_stroke(Color::BLACK, 1.0),
        );
        shape.widths = Some(vec![0.1, 0.2, 0.3, 0.4]);
        assert!(shape.remove_points(&set(&[0, 2])));
        assert_eq!(
            shape.path.on_curve_points(),
            [Vec2::new(1.0, 0.0), Vec2::new(3.0, 0.0)]
        );
        assert!(matches!(shape.path.cmds[0], PathCmd::MoveTo(_)));
        assert_eq!(shape.widths, Some(vec![0.2, 0.4]));
        assert!(!shape.remove_points(&set(&[0])));
    }

    #[test]
    fn a_closed_shape_cut_to_two_points_opens() {
        let mut shape = Shape::new(
            "r",
            Path::rect(Vec2::ZERO, Vec2::ONE, 0.0),
            Style::fill(Color::BLACK),
        );
        assert!(shape.remove_points(&set(&[1])));
        assert_eq!(shape.path.on_curve_points().len(), 3);
        assert_eq!(shape.path.cmds.last(), Some(&PathCmd::Close));
        assert!(shape.remove_points(&set(&[1])));
        assert_ne!(shape.path.cmds.last(), Some(&PathCmd::Close));
    }

    #[test]
    fn removing_a_circles_start_removes_its_twin() {
        let mut shape = Shape::new("c", Path::circle(Vec2::ZERO, 1.0), Style::fill(Color::BLACK));
        assert!(shape.remove_points(&set(&[0])));
        assert_eq!(shape.path.on_curve_points().len(), 3);
    }
}
