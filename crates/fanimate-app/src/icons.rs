//! Interface icons drawn with vector strokes, so they never depend on a
//! font having the right glyph and they match at every size and theme.
//! Style follows the brand icon spec: rounded strokes on a square grid.

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Sense, Shape, Stroke, pos2, vec2};
use fanimate_editor::Tool;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Tool(Tool),
    VectorLayer,
    GroupLayer,
    BoneLayer,
    Duplicate,
    Delete,
    Up,
    Down,
    Eye,
    EyeClosed,
}

/// Draw `icon` centred in `rect`.
pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height()) * 0.5 - 1.0; // half size
    let c = rect.center();
    let at = |x: f32, y: f32| pos2(c.x + x * s, c.y + y * s);
    let st = Stroke::new((s * 0.16).clamp(1.2, 2.0), color);
    let line = |a: Pos2, b: Pos2| p.line_segment([a, b], st);
    let poly = |pts: Vec<Pos2>, closed: bool| {
        if closed {
            p.add(Shape::closed_line(pts, st));
        } else {
            p.add(Shape::line(pts, st));
        }
    };
    let dot = |q: Pos2, r: f32| p.circle_filled(q, r * s, color);
    let ring = |q: Pos2, r: f32| p.circle_stroke(q, r * s, st);
    let bone = |a: Pos2, b: Pos2| {
        // a tapered bone: joint circle and two lines to the tip
        let d = (b - a).normalized();
        let n = vec2(-d.y, d.x) * 0.22 * s;
        ring(a, 0.22);
        poly(vec![a + n + d * 0.2 * s, b, a - n + d * 0.2 * s], false);
    };
    let arrow = |a: Pos2, b: Pos2| {
        line(a, b);
        let d = (b - a).normalized() * 0.3 * s;
        let n = vec2(-d.y, d.x);
        poly(vec![b - d + n, b, b - d - n], false);
    };

    match icon {
        Icon::Tool(Tool::SelectPoints) => {
            for (x, y) in [(-0.6, -0.6), (0.6, -0.6), (0.6, 0.6), (-0.6, 0.6)] {
                p.rect_filled(
                    Rect::from_center_size(at(x, y), vec2(0.3 * s, 0.3 * s)),
                    1.0,
                    color,
                );
            }
            poly(
                vec![at(-0.6, -0.6), at(0.6, -0.6), at(0.6, 0.6), at(-0.6, 0.6)],
                true,
            );
        }
        Icon::Tool(Tool::TranslatePoints) => {
            arrow(at(0.0, 0.0), at(0.0, -0.9));
            arrow(at(0.0, 0.0), at(0.0, 0.9));
            arrow(at(0.0, 0.0), at(-0.9, 0.0));
            arrow(at(0.0, 0.0), at(0.9, 0.0));
            dot(c, 0.15);
        }
        Icon::Tool(Tool::AddPoint) => {
            poly(vec![at(-0.8, 0.6), at(-0.2, -0.4), at(0.3, 0.3)], false);
            p.rect_filled(
                Rect::from_center_size(at(-0.2, -0.4), vec2(0.3 * s, 0.3 * s)),
                1.0,
                color,
            );
            line(at(0.6, 0.2), at(0.6, 0.9));
            line(at(0.25, 0.55), at(0.95, 0.55));
        }
        Icon::Tool(Tool::Freehand) => {
            // pencil
            poly(
                vec![
                    at(-0.8, 0.8),
                    at(-0.6, 0.3),
                    at(0.5, -0.8),
                    at(0.8, -0.5),
                    at(-0.3, 0.6),
                ],
                true,
            );
            line(at(-0.6, 0.3), at(-0.3, 0.6));
        }
        Icon::Tool(Tool::Eraser) => {
            poly(
                vec![at(-0.9, 0.3), at(-0.2, -0.4), at(0.4, 0.2), at(-0.3, 0.9)],
                true,
            );
            poly(
                vec![at(-0.2, -0.4), at(0.3, -0.9), at(0.9, -0.3), at(0.4, 0.2)],
                true,
            );
            line(at(-0.3, 0.9), at(0.9, 0.9));
        }
        Icon::Tool(Tool::PaintBucket) => {
            poly(
                vec![at(-0.7, -0.1), at(0.0, -0.8), at(0.7, -0.1), at(0.0, 0.6)],
                true,
            );
            line(at(-0.7, -0.1), at(0.7, -0.1));
            dot(at(0.8, 0.6), 0.18);
        }
        Icon::Tool(Tool::Eyedropper) => {
            line(at(-0.8, 0.8), at(0.3, -0.3));
            ring(at(0.5, -0.5), 0.3);
            dot(at(-0.8, 0.8), 0.12);
        }
        Icon::Tool(Tool::TransformLayer) => {
            poly(
                vec![at(-0.5, -0.5), at(0.5, -0.5), at(0.5, 0.5), at(-0.5, 0.5)],
                true,
            );
            arrow(at(0.5, 0.5), at(0.95, 0.95));
            arrow(at(-0.5, -0.5), at(-0.95, -0.95));
        }
        Icon::Tool(Tool::SelectBone) => {
            bone(at(-0.6, 0.6), at(0.7, -0.7));
            ring(at(-0.6, 0.6), 0.38);
        }
        Icon::Tool(Tool::AddBone) => {
            bone(at(-0.7, 0.7), at(0.4, -0.4));
            line(at(0.65, -0.05), at(0.65, 0.75));
            line(at(0.25, 0.35), at(1.0, 0.35));
        }
        Icon::Tool(Tool::ManipulateBones) => {
            bone(at(-0.6, 0.6), at(0.5, -0.5));
            let arc: Vec<Pos2> = (0..=8)
                .map(|i| {
                    let a = 0.3 + i as f32 * 0.15;
                    at(-0.6 + 1.2 * a.cos(), 0.6 - 1.2 * a.sin())
                })
                .collect();
            let end = *arc.last().unwrap_or(&c);
            poly(arc, false);
            dot(end, 0.12);
        }
        Icon::Tool(Tool::Pan) => {
            for (dx, dy) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
                arrow(at(dx * 0.25, dy * 0.25), at(dx * 0.95, dy * 0.95));
            }
            ring(c, 0.18);
        }
        Icon::Tool(Tool::Zoom) => {
            ring(at(-0.2, -0.2), 0.55);
            line(at(0.2, 0.2), at(0.85, 0.85));
        }
        Icon::VectorLayer => {
            // pen nib
            poly(
                vec![at(0.0, -0.9), at(0.6, 0.0), at(0.0, 0.9), at(-0.6, 0.0)],
                true,
            );
            line(at(0.0, -0.9), at(0.0, 0.2));
            dot(at(0.0, 0.2), 0.12);
        }
        Icon::GroupLayer => {
            poly(
                vec![
                    at(-0.9, -0.6),
                    at(-0.3, -0.6),
                    at(-0.1, -0.35),
                    at(0.9, -0.35),
                    at(0.9, 0.7),
                    at(-0.9, 0.7),
                ],
                true,
            );
        }
        Icon::BoneLayer => bone(at(-0.6, 0.6), at(0.8, -0.8)),
        Icon::Duplicate => {
            poly(
                vec![at(-0.8, -0.3), at(0.3, -0.3), at(0.3, 0.8), at(-0.8, 0.8)],
                true,
            );
            poly(
                vec![
                    at(-0.3, -0.3),
                    at(-0.3, -0.8),
                    at(0.8, -0.8),
                    at(0.8, 0.3),
                    at(0.3, 0.3),
                ],
                false,
            );
        }
        Icon::Delete => {
            line(at(-0.8, -0.55), at(0.8, -0.55));
            poly(
                vec![at(-0.3, -0.55), at(-0.3, -0.85), at(0.3, -0.85), at(0.3, -0.55)],
                false,
            );
            poly(
                vec![at(-0.6, -0.55), at(-0.45, 0.9), at(0.45, 0.9), at(0.6, -0.55)],
                false,
            );
        }
        Icon::Up => arrow(at(0.0, 0.8), at(0.0, -0.8)),
        Icon::Down => arrow(at(0.0, -0.8), at(0.0, 0.8)),
        Icon::Eye | Icon::EyeClosed => {
            let upper: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let t = i as f32 / 10.0 * std::f32::consts::PI;
                    at(-0.9 * t.cos(), -0.55 * t.sin())
                })
                .collect();
            if icon == Icon::Eye {
                let lower: Vec<Pos2> = upper.iter().map(|q| pos2(q.x, c.y + (c.y - q.y))).collect();
                poly(upper, false);
                poly(lower, false);
                dot(c, 0.25);
            } else {
                // closed lid with lashes
                let lid: Vec<Pos2> = upper.iter().map(|q| pos2(q.x, c.y + (c.y - q.y) * 0.6)).collect();
                for x in [-0.45, 0.0, 0.45] {
                    line(at(x, 0.35), at(x * 1.2, 0.7));
                }
                poly(lid, false);
            }
        }
    }
}

/// An icon button. `dim` draws it faded (unavailable or not yet built) but
/// it still reacts, so tooltips and hints keep working.
pub fn button(ui: &mut egui::Ui, icon: Icon, size: f32, selected: bool, dim: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let visuals = ui.visuals();
    let bg = if selected {
        Some(visuals.widgets.active.bg_fill)
    } else if response.hovered() {
        Some(visuals.widgets.hovered.bg_fill)
    } else {
        None
    };
    if let Some(bg) = bg {
        ui.painter().rect_filled(rect, 4.0, bg);
    }
    let color = if selected {
        visuals.widgets.active.fg_stroke.color
    } else if dim {
        visuals.weak_text_color().gamma_multiply(0.7)
    } else {
        visuals.text_color()
    };
    paint(ui.painter(), rect.shrink(size * 0.22), icon, color);
    response
}
