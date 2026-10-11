//! Vector paths, styles and shapes.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::Color;

/// One path command. Points are in the owning layer's space.
///
/// Serialised compactly as `{"M":[x,y]}`, `{"L":[x,y]}`,
/// `{"C":[[x1,y1],[x2,y2],[x,y]]}` and `"Z"`, after the SVG path letters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PathCmd {
    #[serde(rename = "M")]
    MoveTo(Vec2),
    #[serde(rename = "L")]
    LineTo(Vec2),
    /// Cubic Bézier: two control points, then the end point.
    #[serde(rename = "C")]
    CubicTo(Vec2, Vec2, Vec2),
    #[serde(rename = "Z")]
    Close,
}

/// A sequence of subpaths.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Path {
    pub cmds: Vec<PathCmd>,
}

/// Control-point distance for approximating a quarter circle with a cubic.
const KAPPA: f32 = 0.552_284_8;

impl Path {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(mut self, p: Vec2) -> Self {
        self.cmds.push(PathCmd::MoveTo(p));
        self
    }

    pub fn line_to(mut self, p: Vec2) -> Self {
        self.cmds.push(PathCmd::LineTo(p));
        self
    }

    pub fn cubic_to(mut self, c1: Vec2, c2: Vec2, p: Vec2) -> Self {
        self.cmds.push(PathCmd::CubicTo(c1, c2, p));
        self
    }

    pub fn close(mut self) -> Self {
        self.cmds.push(PathCmd::Close);
        self
    }

    /// Axis-aligned rectangle with optional rounded corners.
    pub fn rect(min: Vec2, size: Vec2, radius: f32) -> Self {
        let r = radius.clamp(0.0, size.x.min(size.y) / 2.0);
        let max = min + size;
        if r <= 0.0 {
            return Self::new()
                .move_to(min)
                .line_to(Vec2::new(max.x, min.y))
                .line_to(max)
                .line_to(Vec2::new(min.x, max.y))
                .close();
        }
        let k = r * (1.0 - KAPPA);
        Self::new()
            .move_to(Vec2::new(min.x + r, min.y))
            .line_to(Vec2::new(max.x - r, min.y))
            .cubic_to(
                Vec2::new(max.x - k, min.y),
                Vec2::new(max.x, min.y + k),
                Vec2::new(max.x, min.y + r),
            )
            .line_to(Vec2::new(max.x, max.y - r))
            .cubic_to(
                Vec2::new(max.x, max.y - k),
                Vec2::new(max.x - k, max.y),
                Vec2::new(max.x - r, max.y),
            )
            .line_to(Vec2::new(min.x + r, max.y))
            .cubic_to(
                Vec2::new(min.x + k, max.y),
                Vec2::new(min.x, max.y - k),
                Vec2::new(min.x, max.y - r),
            )
            .line_to(Vec2::new(min.x, min.y + r))
            .cubic_to(
                Vec2::new(min.x, min.y + k),
                Vec2::new(min.x + k, min.y),
                Vec2::new(min.x + r, min.y),
            )
            .close()
    }

    /// Circle made of four cubic arcs.
    pub fn circle(center: Vec2, r: f32) -> Self {
        let k = r * KAPPA;
        let (cx, cy) = (center.x, center.y);
        Self::new()
            .move_to(Vec2::new(cx + r, cy))
            .cubic_to(
                Vec2::new(cx + r, cy + k),
                Vec2::new(cx + k, cy + r),
                Vec2::new(cx, cy + r),
            )
            .cubic_to(
                Vec2::new(cx - k, cy + r),
                Vec2::new(cx - r, cy + k),
                Vec2::new(cx - r, cy),
            )
            .cubic_to(
                Vec2::new(cx - r, cy - k),
                Vec2::new(cx - k, cy - r),
                Vec2::new(cx, cy - r),
            )
            .cubic_to(
                Vec2::new(cx + k, cy - r),
                Vec2::new(cx + r, cy - k),
                Vec2::new(cx + r, cy),
            )
            .close()
    }

    /// Every point the path references, including control points.
    pub fn points(&self) -> impl Iterator<Item = Vec2> + '_ {
        self.cmds.iter().flat_map(|c| {
            let (n, pts) = match *c {
                PathCmd::MoveTo(p) | PathCmd::LineTo(p) => (1, [p; 3]),
                PathCmd::CubicTo(a, b, p) => (3, [a, b, p]),
                PathCmd::Close => (0, [Vec2::ZERO; 3]),
            };
            pts.into_iter().take(n)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Color,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Style {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<Stroke>,
}

impl Style {
    pub fn fill(color: Color) -> Self {
        Self {
            fill: Some(color),
            stroke: None,
        }
    }

    pub fn with_stroke(mut self, color: Color, width: f32) -> Self {
        self.stroke = Some(Stroke { color, width });
        self
    }
}

/// A styled path on a vector layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub name: String,
    pub path: Path,
    pub style: Style,
    /// Bone this shape is bound to, as an index into the skeleton of the
    /// nearest enclosing bone layer. The shape follows that bone rigidly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bone: Option<usize>,
}

impl Shape {
    pub fn new(name: impl Into<String>, path: Path, style: Style) -> Self {
        Self {
            name: name.into(),
            path,
            style,
            bone: None,
        }
    }

    pub fn bound_to(mut self, bone: usize) -> Self {
        self.bone = Some(bone);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_serialises_compactly() {
        let p = Path::new()
            .move_to(Vec2::new(0.0, 0.0))
            .line_to(Vec2::new(10.0, 0.0))
            .close();
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, r#"[{"M":[0.0,0.0]},{"L":[10.0,0.0]},"Z"]"#);
        assert_eq!(serde_json::from_str::<Path>(&json).unwrap(), p);
    }

    #[test]
    fn points_includes_control_points() {
        let p = Path::circle(Vec2::ZERO, 1.0);
        assert_eq!(p.points().count(), 1 + 4 * 3);
    }
}
