//! Built-in sample documents: a hello-world rig and the benchmark scene.

use std::f32::consts::TAU;

use glam::Vec2;

use crate::Color;
use crate::anim::{Frame, Interpolation};
use crate::bones::Skeleton;
use crate::path::{Path, Shape, Style};
use crate::scene::{Document, LayerKind};

/// Brand colours used by the samples (from `brand/tokens/tokens.json`).
pub mod palette {
    use crate::Color;

    pub const NIGHT_BG: Color = rgb(0x15, 0x11, 0x1F);
    pub const VIOLET_300: Color = rgb(0xBC, 0x9B, 0xFF);
    pub const VIOLET_400: Color = rgb(0x9F, 0x73, 0xFF);
    pub const VIOLET_500: Color = rgb(0x84, 0x50, 0xF5);
    pub const VIOLET_600: Color = rgb(0x6D, 0x35, 0xDE);
    pub const VIOLET_950: Color = rgb(0x1C, 0x0B, 0x3D);
    pub const KEYFRAME: Color = rgb(0xFF, 0xB8, 0x4D);
    pub const BONE: Color = rgb(0x5E, 0xEA, 0xD4);

    const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }
}

/// The Bone-F mark as a tiny rig: the middle arm is a bone that waves.
pub fn hello() -> Document {
    let mut doc = Document::new("Hello Fanimate", 1280.0, 720.0, 24.0);
    doc.end = 47;
    doc.background = palette::VIOLET_950;

    let (cx, cy) = (640.0, 360.0);
    let w = 56.0; // stroke width of the F
    let stem_x = cx - 70.0;
    let joint = Vec2::new(stem_x, cy - 10.0);

    let mut skel = Skeleton::default();
    let arm = skel.add("arm", None, joint, 110.0, 0.0);
    for (frame, angle) in [(0, 0.0), (12, -0.35), (24, 0.0), (36, 0.25), (47, 0.0)] {
        skel.key_angle(arm, frame, angle, Interpolation::Smooth);
    }

    let white = Style::fill(Color::WHITE);
    let shapes = vec![
        Shape::new(
            "stem",
            Path::rect(
                Vec2::new(stem_x - w / 2.0, cy - 220.0),
                Vec2::new(w, 440.0),
                w / 2.0,
            ),
            white,
        ),
        Shape::new(
            "top arm",
            Path::rect(
                Vec2::new(stem_x - w / 2.0, cy - 220.0),
                Vec2::new(210.0, w),
                w / 2.0,
            ),
            white,
        ),
        Shape::new(
            "middle arm",
            Path::rect(
                Vec2::new(stem_x - w / 2.0, joint.y - w / 2.0),
                Vec2::new(170.0, w),
                w / 2.0,
            ),
            white,
        )
        .bound_to(arm),
        Shape::new("joint", Path::circle(joint, w * 0.78), white).bound_to(arm),
        // the pivot dot is see-through in the brand mark, so it matches the background
        Shape::new(
            "pivot",
            Path::circle(joint, w * 0.3),
            Style::fill(palette::VIOLET_950),
        )
        .bound_to(arm),
    ];

    let art = doc.new_layer("F", LayerKind::vector(shapes));
    let rig = doc.new_layer(
        "Bone-F rig",
        LayerKind::Bone {
            skeleton: skel,
            children: vec![art],
        },
    );
    doc.layers.push(rig);
    doc
}

/// The Gate 1 benchmark scene: a creature with `bones` bones split into
/// eight limbs, every bone carrying a segment and a joint shape, every bone
/// keyed through the whole range. The Phase 1 budget is 60 fps with 40 bones
/// (Smart Bones arrive in milestone M3).
pub fn benchmark(bones: usize) -> Document {
    let mut doc = Document::new(format!("Benchmark ({bones} bones)"), 1920.0, 1080.0, 24.0);
    doc.end = 119;
    doc.background = palette::NIGHT_BG;

    let center = Vec2::new(960.0, 540.0);
    let limbs = 8.min(bones.max(1));
    let mut skel = Skeleton::default();
    let mut limb_shapes = Vec::new();

    let mut made = 0;
    for limb in 0..limbs {
        // spread the bones as evenly as possible across limbs
        let count = bones / limbs + usize::from(limb < bones % limbs);
        let dir = limb as f32 / limbs as f32 * TAU;
        let mut parent = None;
        let mut length = 70.0;
        for seg in 0..count {
            let origin = match parent {
                None => center + Vec2::from_angle(dir) * 90.0,
                Some(_) => Vec2::new(length, 0.0),
            };
            let rest = if parent.is_none() { dir } else { 0.0 };
            length = 70.0 * 0.88_f32.powi(seg as i32);
            let bone = skel.add(format!("limb{limb}.{seg}"), parent, origin, length, rest);

            // wave: each segment lags the one before it
            let phase = limb as f32 * 0.7 + seg as f32 * 0.6;
            let amp = 0.35 + 0.05 * seg as f32;
            let mut frame: Frame = 0;
            while frame <= doc.end {
                let t = frame as f32 / (doc.end + 1) as f32 * TAU * 2.0;
                skel.key_angle(bone, frame, rest + amp * (t + phase).sin(), Interpolation::Smooth);
                frame += 6;
            }
            parent = Some(bone);
            made += 1;
        }
    }
    debug_assert_eq!(made, bones);

    // Shapes are drawn in rest pose; compute it to place them.
    let mut rest = Vec::new();
    skel.rest_pose(&mut rest);
    for (i, bone) in skel.bones.iter().enumerate() {
        let depth = std::iter::successors(bone.parent, |&p| skel.bones[p].parent).count();
        let t = (depth as f32 / 6.0).min(1.0);
        let color = Color::lerp(palette::VIOLET_300, palette::VIOLET_600, t);
        let width = 34.0 * 0.85_f32.powi(depth as i32);
        let to_world = |p: Vec2| rest[i].transform_point2(p);

        // segment as a polygon in rest pose world space
        let seg = Path::rect(
            Vec2::new(0.0, -width / 2.0),
            Vec2::new(bone.length, width),
            width / 2.0,
        );
        let seg = Path {
            cmds: seg.cmds.iter().map(|c| map_cmd(*c, to_world)).collect(),
        };
        limb_shapes.push(Shape::new(format!("seg {i}"), seg, Style::fill(color)).bound_to(i));
        limb_shapes.push(
            Shape::new(
                format!("joint {i}"),
                Path::circle(to_world(Vec2::ZERO), width * 0.42),
                Style::fill(palette::KEYFRAME),
            )
            .bound_to(i),
        );
    }

    let body = Shape::new(
        "body",
        Path::circle(center, 110.0),
        Style::fill(palette::VIOLET_500).with_stroke(palette::BONE, 6.0),
    );
    let limbs_layer = doc.new_layer("limbs", LayerKind::vector(limb_shapes));
    let body_layer = doc.new_layer("body", LayerKind::vector(vec![body]));
    let rig = doc.new_layer(
        "creature",
        LayerKind::Bone {
            skeleton: skel,
            children: vec![limbs_layer, body_layer],
        },
    );
    doc.layers.push(rig);
    doc
}

fn map_cmd(c: crate::PathCmd, f: impl Fn(Vec2) -> Vec2) -> crate::PathCmd {
    use crate::PathCmd::*;
    match c {
        MoveTo(p) => MoveTo(f(p)),
        LineTo(p) => LineTo(f(p)),
        CubicTo(a, b, p) => CubicTo(f(a), f(b), f(p)),
        Close => Close,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluate;

    #[test]
    fn benchmark_has_requested_bones_and_shapes() {
        for n in [1, 7, 40, 100] {
            let doc = benchmark(n);
            let LayerKind::Bone { skeleton, .. } = &doc.layers[0].kind else {
                panic!("expected a bone layer");
            };
            assert_eq!(skeleton.bones.len(), n);
            assert_eq!(evaluate(&doc, 10.0).len(), n * 2 + 1);
        }
    }

    #[test]
    fn hello_evaluates() {
        assert_eq!(evaluate(&hello(), 6.0).len(), 5);
    }
}
