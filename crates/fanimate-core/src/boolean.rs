//! Boolean shape operations: union, subtract and intersect.
//!
//! Paths are flattened to polygons, combined with [`i_overlay`] (filled by
//! the non-zero rule, as the renderer fills), then turned back into a path
//! with few points: straight runs stay straight, sharp corners stay sharp,
//! and curved runs are fitted with smooth cubic curves, so the result can
//! be edited point by point like any other shape.

use glam::Vec2;
use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::single::SingleFloatOverlay;

use crate::path::Path;

/// How far flattening curves may stray from them, in document units.
const FLATTEN_TOLERANCE: f32 = 0.05;

/// A turn sharper than this (in radians) is kept as a corner.
const CORNER_ANGLE: f32 = 0.6;

type Contours = Vec<Vec<[f64; 2]>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoolOp {
    /// Everything covered by any of the shapes.
    Union,
    /// The first shape without the area of the others.
    Subtract,
    /// Only what all the shapes cover.
    Intersect,
}

/// Combine `paths` by `op`. Open subpaths are closed, as when filling.
/// `tolerance` is how far the result may stray from the exact outline, in
/// document units. Returns an empty path when nothing is left.
pub fn combine(paths: &[&Path], op: BoolOp, tolerance: f32) -> Path {
    let tolerance = if tolerance.is_finite() && tolerance > 0.0 {
        tolerance
    } else {
        0.25
    };
    let mut iter = paths.iter().map(|p| contours(p));
    let Some(first) = iter.next() else {
        return Path::new();
    };
    // the first operand is normalised on its own, so later steps see a
    // clean outline with holes wound against their outer contour
    let empty: Contours = Vec::new();
    let mut acc = flat(first.overlay(&empty, OverlayRule::Subject, FillRule::NonZero));
    for next in iter {
        let rule = match op {
            BoolOp::Union => OverlayRule::Union,
            BoolOp::Subtract => OverlayRule::Difference,
            BoolOp::Intersect => OverlayRule::Intersect,
        };
        acc = flat(acc.overlay(&next, rule, FillRule::NonZero));
    }
    to_path(&acc, tolerance)
}

/// The path's subpaths as closed polygons, dropping any too small to fill.
fn contours(path: &Path) -> Contours {
    path.polylines(FLATTEN_TOLERANCE)
        .into_iter()
        .filter(|(pts, _)| pts.len() >= 3)
        .filter(|(pts, _)| pts.iter().all(|p| p.is_finite()))
        .map(|(pts, _)| pts.iter().map(|p| [p.x as f64, p.y as f64]).collect())
        .collect()
}

fn flat(shapes: Vec<Vec<Vec<[f64; 2]>>>) -> Contours {
    shapes.into_iter().flatten().collect()
}

fn to_path(contours: &Contours, tolerance: f32) -> Path {
    let mut path = Path::new();
    for contour in contours {
        let pts: Vec<Vec2> = contour
            .iter()
            .map(|p| Vec2::new(p[0] as f32, p[1] as f32))
            .collect();
        // curves fitted through few points follow the outline closely, so
        // start coarse and keep more points only where that strays
        let mut coarse = tolerance * 16.0;
        let fitted = loop {
            let kept = simplify_closed(&pts, coarse);
            let fitted = fit_closed(Path::new(), &kept);
            if kept.len() < 3 || coarse <= tolerance || deviation(&pts, &fitted) <= tolerance {
                break (kept.len() >= 3).then_some(fitted);
            }
            coarse /= 2.0;
        };
        if let Some(fitted) = fitted {
            path.cmds.extend(fitted.cmds);
        }
    }
    path
}

/// How far the furthest of `pts` is from `path`'s outline.
fn deviation(pts: &[Vec2], path: &Path) -> f32 {
    let lines = path.polylines(FLATTEN_TOLERANCE);
    pts.iter()
        .map(|&p| {
            lines
                .iter()
                .flat_map(|(q, _)| (0..q.len()).map(move |i| (q[i], q[(i + 1) % q.len()])))
                .map(|(a, b)| segment_distance(p, a, b))
                .fold(f32::INFINITY, f32::min)
        })
        .fold(0.0, f32::max)
}

/// The points of a closed polygon worth keeping: every corner, and enough
/// of the rest that the outline stays within `tolerance` of the original.
fn simplify_closed(pts: &[Vec2], tolerance: f32) -> Vec<Vec2> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let corner = |i: usize| {
        let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
        turn(a, b, c) > CORNER_ANGLE
    };
    let mut anchors: Vec<usize> = (0..n).filter(|&i| corner(i)).collect();
    if anchors.is_empty() {
        // a smooth loop: start from two opposite points
        anchors = vec![0, n / 2];
    } else if anchors.len() == 1 {
        anchors.push((anchors[0] + n / 2) % n);
        anchors.sort_unstable();
    }
    let mut keep = vec![false; n];
    for (k, &a) in anchors.iter().enumerate() {
        let b = anchors[(k + 1) % anchors.len()];
        keep[a] = true;
        // the run from one anchor to the next, wrapping round
        let run: Vec<usize> = std::iter::successors(Some(a), |&i| Some((i + 1) % n))
            .take(((b + n - a) % n).max(1) + 1)
            .collect();
        douglas_peucker(pts, &run, tolerance, &mut keep);
    }
    (0..n).filter(|&i| keep[i]).map(|i| pts[i]).collect()
}

fn douglas_peucker(pts: &[Vec2], run: &[usize], tolerance: f32, keep: &mut [bool]) {
    if run.len() < 3 {
        return;
    }
    let (a, b) = (pts[run[0]], pts[run[run.len() - 1]]);
    let (far, d) = run[1..run.len() - 1]
        .iter()
        .enumerate()
        .map(|(k, &i)| (k + 1, segment_distance(pts[i], a, b)))
        .fold((0, 0.0), |best, x| if x.1 > best.1 { x } else { best });
    if d > tolerance {
        keep[run[far]] = true;
        douglas_peucker(pts, &run[..=far], tolerance, keep);
        douglas_peucker(pts, &run[far..], tolerance, keep);
    }
}

/// A closed subpath through `pts`: straight where the outline turns
/// sharply or runs straight, smooth cubic curves elsewhere.
fn fit_closed(mut path: Path, pts: &[Vec2]) -> Path {
    let n = pts.len();
    let corner: Vec<bool> = (0..n)
        .map(|i| turn(pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]) > CORNER_ANGLE)
        .collect();
    // the tangent at each smooth point (Catmull-Rom); none at corners
    let tangent = |i: usize| -> Vec2 {
        if corner[i] {
            Vec2::ZERO
        } else {
            (pts[(i + 1) % n] - pts[(i + n - 1) % n]) / 6.0
        }
    };
    path = path.move_to(pts[0]);
    for i in 0..n {
        let j = (i + 1) % n;
        let (t0, t1) = (tangent(i), tangent(j));
        if t0 == Vec2::ZERO && t1 == Vec2::ZERO {
            if j != 0 {
                path = path.line_to(pts[j]);
            }
        } else {
            path = path.cubic_to(pts[i] + t0, pts[j] - t1, pts[j]);
        }
    }
    path.close()
}

/// The angle the outline turns through at `b`, from 0 (straight on) to π.
fn turn(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    let (u, v) = (b - a, c - b);
    if u.length_squared() == 0.0 || v.length_squared() == 0.0 {
        return 0.0;
    }
    u.angle_to(v).abs()
}

fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::{Shape, Style};
    use crate::{Color, PathCmd};

    fn area(path: &Path) -> f32 {
        // the shoelace formula over the flattened outline, non-zero filled
        path.polylines(0.01)
            .iter()
            .map(|(pts, _)| {
                (0..pts.len())
                    .map(|i| pts[i].perp_dot(pts[(i + 1) % pts.len()]))
                    .sum::<f32>()
                    / 2.0
            })
            .sum::<f32>()
            .abs()
    }

    fn square(x: f32, y: f32, s: f32) -> Path {
        Path::rect(Vec2::new(x, y), Vec2::splat(s), 0.0)
    }

    #[test]
    fn union_of_overlapping_squares() {
        let (a, b) = (square(0.0, 0.0, 10.0), square(5.0, 5.0, 10.0));
        let u = combine(&[&a, &b], BoolOp::Union, 0.1);
        assert!((area(&u) - 175.0).abs() < 0.5, "{}", area(&u));
        // straight edges and sharp corners: lines only, eight corners
        assert!(u.cmds.iter().all(|c| !matches!(c, PathCmd::CubicTo(..))));
        assert_eq!(u.on_curve_count(), 8);
    }

    #[test]
    fn subtract_and_intersect() {
        let (a, b) = (square(0.0, 0.0, 10.0), square(5.0, 5.0, 10.0));
        let s = combine(&[&a, &b], BoolOp::Subtract, 0.1);
        assert!((area(&s) - 75.0).abs() < 0.5);
        let i = combine(&[&a, &b], BoolOp::Intersect, 0.1);
        assert!((area(&i) - 25.0).abs() < 0.5);
        let apart = square(50.0, 50.0, 5.0);
        assert!(combine(&[&a, &apart], BoolOp::Intersect, 0.1).cmds.is_empty());
    }

    #[test]
    fn a_hole_punched_through_stays_a_hole() {
        let ring = combine(
            &[&Path::circle(Vec2::ZERO, 20.0), &Path::circle(Vec2::ZERO, 10.0)],
            BoolOp::Subtract,
            0.1,
        );
        let shape = Shape::new("ring", ring.clone(), Style::fill(Color::BLACK));
        assert!(shape.contains(Vec2::new(15.0, 0.0), 0.0));
        assert!(!shape.contains(Vec2::ZERO, 0.0));
        let expected = std::f32::consts::PI * (400.0 - 100.0);
        assert!((area(&ring) - expected).abs() / expected < 0.01);
    }

    #[test]
    fn curves_come_back_as_few_smooth_points() {
        let a = Path::circle(Vec2::ZERO, 40.0);
        let b = Path::circle(Vec2::new(50.0, 0.0), 40.0);
        let u = combine(&[&a, &b], BoolOp::Union, 0.25);
        assert!(u.on_curve_count() < 40, "{} points", u.on_curve_count());
        assert!(u.cmds.iter().any(|c| matches!(c, PathCmd::CubicTo(..))));
        // still close to the true outline
        let r = 40.0_f32;
        for (pts, _) in u.polylines(0.05) {
            for p in pts {
                let d = (p.length() - r)
                    .abs()
                    .min((p.distance(Vec2::new(50.0, 0.0)) - r).abs());
                assert!(d < 1.0, "{p:?} is {d} off");
            }
        }
    }

    #[test]
    fn nothing_in_nothing_out() {
        assert!(combine(&[], BoolOp::Union, 0.1).cmds.is_empty());
        let line = Path::new().move_to(Vec2::ZERO).line_to(Vec2::X);
        assert!(combine(&[&line], BoolOp::Union, 0.1).cmds.is_empty());
    }
}
