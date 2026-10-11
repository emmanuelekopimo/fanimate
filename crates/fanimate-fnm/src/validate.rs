//! Project validator and self-repair.
//!
//! Catches the kinds of damage that silently corrupt or slow down projects
//! in other tools (pain point P13 in the product vision): keyframes on
//! negative frames, duplicate or unsorted keys, broken bone references,
//! duplicate layer ids and non-finite numbers.
//!
//! [`validate`] only reports. [`repair`] fixes everything it safely can and
//! marks each fixed issue as repaired; anything left is for the user.

use std::collections::HashSet;
use std::fmt;

use fanimate_core::anim::Animatable;
use fanimate_core::{Channel, Document, Layer, LayerId, LayerKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Worth knowing; the project still behaves correctly.
    Warning,
    /// The project is damaged and may misbehave until fixed.
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub severity: Severity,
    /// Where the problem is, e.g. `layer 3 "arm" > transform.rotation`.
    pub location: String,
    pub message: String,
    pub repaired: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub issues: Vec<Issue>,
}

impl Report {
    /// No issues at all.
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    /// Errors that were not repaired.
    pub fn unresolved_errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error && !i.repaired)
    }

    pub fn repaired_count(&self) -> usize {
        self.issues.iter().filter(|i| i.repaired).count()
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.issues.is_empty() {
            return f.write_str("no issues");
        }
        for i in &self.issues {
            let sev = match i.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            let fixed = if i.repaired { " (repaired)" } else { "" };
            writeln!(f, "{sev}: {}: {}{fixed}", i.location, i.message)?;
        }
        Ok(())
    }
}

/// Check a document without changing it.
pub fn validate(doc: &Document) -> Report {
    // The checks share one code path with repair; run it on a scratch copy.
    let mut scratch = doc.clone();
    Checker::new(false).run(&mut scratch)
}

/// Check a document and fix everything that can be fixed safely.
pub fn repair(doc: &mut Document) -> Report {
    Checker::new(true).run(doc)
}

struct Checker {
    repair: bool,
    report: Report,
}

impl Checker {
    fn new(repair: bool) -> Self {
        Self {
            repair,
            report: Report::default(),
        }
    }

    fn issue(&mut self, severity: Severity, location: &str, message: String, fixable: bool) {
        self.report.issues.push(Issue {
            severity,
            location: location.to_owned(),
            message,
            repaired: fixable && self.repair,
        });
    }

    fn run(mut self, doc: &mut Document) -> Report {
        self.document(doc);
        self.layer_ids(doc);
        let end = doc.end;
        let mut layers = std::mem::take(&mut doc.layers);
        for layer in &mut layers {
            self.layer(layer, end, None);
        }
        doc.layers = layers;
        self.report
    }

    fn document(&mut self, doc: &mut Document) {
        let loc = "document";
        if !(doc.fps.is_finite() && doc.fps > 0.0) {
            self.issue(
                Severity::Error,
                loc,
                format!("frame rate {} is not positive", doc.fps),
                true,
            );
            if self.repair {
                doc.fps = 24.0;
            }
        }
        if !(doc.width.is_finite() && doc.width > 0.0 && doc.height.is_finite() && doc.height > 0.0) {
            self.issue(
                Severity::Error,
                loc,
                format!("canvas size {}x{} is not positive", doc.width, doc.height),
                true,
            );
            if self.repair {
                doc.width = 1920.0;
                doc.height = 1080.0;
            }
        }
        if doc.start < 0 {
            self.issue(
                Severity::Error,
                loc,
                format!("start frame {} is negative", doc.start),
                true,
            );
            if self.repair {
                doc.start = 0;
            }
        }
        if doc.end < doc.start {
            self.issue(
                Severity::Error,
                loc,
                format!("end frame {} is before start frame {}", doc.end, doc.start),
                true,
            );
            if self.repair {
                doc.end = doc.start;
            }
        }
    }

    /// Layer ids must be unique, and `next_layer_id` must be above all of them.
    fn layer_ids(&mut self, doc: &mut Document) {
        let mut seen = HashSet::new();
        let mut dupes = Vec::new();
        let mut max = 0;
        doc.walk(|l| {
            max = max.max(l.id.0);
            if !seen.insert(l.id) {
                dupes.push(l.id);
            }
        });
        if doc.next_layer_id <= max {
            self.issue(
                Severity::Error,
                "document",
                format!(
                    "next layer id {} is not above the highest id {max}",
                    doc.next_layer_id
                ),
                true,
            );
            if self.repair {
                doc.next_layer_id = max + 1;
            }
        }
        for id in &dupes {
            self.issue(
                Severity::Error,
                &format!("layer {}", id.0),
                "layer id is used more than once".into(),
                true,
            );
        }
        if self.repair && !dupes.is_empty() {
            // keep the first use of each id, renumber the rest
            let mut seen = HashSet::new();
            let mut next = doc.next_layer_id.max(max + 1);
            doc.walk_mut(|l| {
                if !seen.insert(l.id) {
                    l.id = LayerId(next);
                    next += 1;
                }
            });
            doc.next_layer_id = next;
        }
    }

    /// `bones`: bone count of the enclosing bone layer, if any.
    fn layer(&mut self, layer: &mut Layer, end: i32, bones: Option<usize>) {
        let loc = format!("layer {} \"{}\"", layer.id.0, layer.name);
        self.channel(&mut layer.opacity, &loc, "opacity", end);
        if layer.opacity.base < 0.0 || layer.opacity.base > 1.0 {
            self.issue(
                Severity::Warning,
                &loc,
                format!(
                    "opacity {} is outside 0..1 and will be clamped",
                    layer.opacity.base
                ),
                false,
            );
        }
        self.channel(&mut layer.transform.position, &loc, "transform.position", end);
        self.channel(&mut layer.transform.rotation, &loc, "transform.rotation", end);
        self.channel(&mut layer.transform.scale, &loc, "transform.scale", end);

        match &mut layer.kind {
            LayerKind::Vector { shapes } => {
                for (i, shape) in shapes.iter_mut().enumerate() {
                    let Some(b) = shape.bone else { continue };
                    let problem = match bones {
                        None => Some("is bound to a bone but is not inside a bone layer".to_owned()),
                        Some(n) if b >= n => Some(format!("is bound to bone {b}, but the skeleton has {n}")),
                        Some(_) => None,
                    };
                    if let Some(p) = problem {
                        let sloc = format!("{loc} > shape {i} \"{}\"", shape.name);
                        self.issue(
                            Severity::Warning,
                            &sloc,
                            format!("{p}; it will not move with the rig"),
                            true,
                        );
                        if self.repair {
                            shape.bone = None;
                        }
                    }
                    if shape.path.points().any(|p| !p.is_finite()) {
                        let sloc = format!("{loc} > shape {i} \"{}\"", shape.name);
                        self.issue(Severity::Error, &sloc, "path has non-finite points".into(), false);
                    }
                }
            }
            LayerKind::Group { children } => {
                for child in children {
                    self.layer(child, end, bones);
                }
            }
            LayerKind::Bone { skeleton, children } => {
                for i in 0..skeleton.bones.len() {
                    let bone = &mut skeleton.bones[i];
                    let bloc = format!("{loc} > bone {i} \"{}\"", bone.name);
                    if let Some(p) = bone.parent.filter(|&p| p >= i) {
                        self.issue(
                            Severity::Error,
                            &bloc,
                            format!("parent {p} does not come before this bone; it is treated as a root"),
                            true,
                        );
                        if self.repair {
                            bone.parent = None;
                        }
                    }
                    if !(bone.origin.is_finite() && bone.length.is_finite()) {
                        self.issue(
                            Severity::Error,
                            &bloc,
                            "origin or length is not a finite number".into(),
                            true,
                        );
                        if self.repair {
                            if !bone.origin.is_finite() {
                                bone.origin = fanimate_core::Vec2::ZERO;
                            }
                            if !bone.length.is_finite() {
                                bone.length = 0.0;
                            }
                        }
                    }
                    let mut angle = std::mem::replace(&mut bone.angle, Channel::new(0.0));
                    self.channel(&mut angle, &bloc, "angle", end);
                    skeleton.bones[i].angle = angle;
                }
                let n = skeleton.bones.len();
                for child in children {
                    self.layer(child, end, Some(n));
                }
            }
        }
    }

    fn channel<T: Animatable + Default>(&mut self, ch: &mut Channel<T>, loc: &str, name: &str, end: i32) {
        let loc = format!("{loc} > {name}");

        if !ch.base.is_finite() {
            self.issue(Severity::Error, &loc, "value is not a finite number".into(), true);
            if self.repair {
                ch.base = T::default();
            }
        }

        let negative = ch.keys.iter().filter(|k| k.frame < 0).count();
        if negative > 0 {
            self.issue(
                Severity::Error,
                &loc,
                format!("{negative} keyframe(s) on negative frames"),
                true,
            );
            if self.repair {
                ch.keys.retain(|k| k.frame >= 0);
            }
        }

        let bad = ch.keys.iter().filter(|k| !k.value.is_finite()).count();
        if bad > 0 {
            self.issue(
                Severity::Error,
                &loc,
                format!("{bad} keyframe(s) with non-finite values"),
                true,
            );
            if self.repair {
                ch.keys.retain(|k| k.value.is_finite());
            }
        }

        if !ch.keys.is_sorted_by_key(|k| k.frame) {
            self.issue(Severity::Error, &loc, "keyframes are out of order".into(), true);
            if self.repair {
                // stable sort keeps the original order of same-frame keys,
                // so the later key wins in the dedupe below
                ch.keys.sort_by_key(|k| k.frame);
            }
        }

        let mut frames: Vec<i32> = ch.keys.iter().map(|k| k.frame).collect();
        frames.sort_unstable();
        let dupes = frames.windows(2).filter(|w| w[0] == w[1]).count();
        if dupes > 0 {
            self.issue(
                Severity::Error,
                &loc,
                format!("{dupes} duplicate keyframe(s)"),
                true,
            );
            if self.repair {
                // keep the last key on each frame
                let keys = std::mem::take(&mut ch.keys);
                for k in keys {
                    match ch.keys.last_mut() {
                        Some(last) if last.frame == k.frame => *last = k,
                        _ => ch.keys.push(k),
                    }
                }
            }
        }

        let late = ch.keys.iter().filter(|k| k.frame > end).count();
        if late > 0 {
            self.issue(
                Severity::Warning,
                &loc,
                format!("{late} keyframe(s) after the last frame ({end})"),
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fanimate_core::{Interpolation, Key, samples};

    #[test]
    fn samples_are_clean() {
        assert!(validate(&samples::hello()).is_clean());
        assert!(validate(&samples::benchmark(40)).is_clean());
    }

    fn rigged() -> Document {
        samples::hello()
    }

    fn arm_angle(doc: &mut Document) -> &mut Channel<f32> {
        let LayerKind::Bone { skeleton, .. } = &mut doc.layers[0].kind else {
            unreachable!()
        };
        &mut skeleton.bones[0].angle
    }

    #[test]
    fn negative_keyframes_are_reported_then_removed() {
        let mut doc = rigged();
        arm_angle(&mut doc).keys.insert(
            0,
            Key {
                frame: -40,
                value: 1.0,
                interp: Interpolation::Linear,
            },
        );
        let report = validate(&doc);
        assert_eq!(report.unresolved_errors().count(), 1);
        assert!(report.issues[0].message.contains("negative"));

        let fixed = repair(&mut doc);
        assert_eq!(fixed.repaired_count(), 1);
        assert!(arm_angle(&mut doc).keys.iter().all(|k| k.frame >= 0));
        assert!(validate(&doc).is_clean());
    }

    #[test]
    fn unsorted_and_duplicate_keys_are_repaired_keeping_last() {
        let mut doc = rigged();
        let ch = arm_angle(&mut doc);
        ch.keys = vec![
            Key {
                frame: 10,
                value: 1.0,
                interp: Interpolation::Linear,
            },
            Key {
                frame: 0,
                value: 0.0,
                interp: Interpolation::Linear,
            },
            Key {
                frame: 10,
                value: 2.0,
                interp: Interpolation::Linear,
            },
        ];
        let report = repair(&mut doc);
        assert_eq!(report.unresolved_errors().count(), 0);
        let ch = arm_angle(&mut doc);
        let frames: Vec<_> = ch.keys.iter().map(|k| (k.frame, k.value)).collect();
        assert_eq!(frames, [(0, 0.0), (10, 2.0)]);
        assert!(validate(&doc).is_clean());
    }

    #[test]
    fn broken_bone_binding_is_unbound() {
        let mut doc = rigged();
        let LayerKind::Bone { children, .. } = &mut doc.layers[0].kind else {
            unreachable!()
        };
        let LayerKind::Vector { shapes } = &mut children[0].kind else {
            unreachable!()
        };
        shapes[2].bone = Some(42);
        assert_eq!(validate(&doc).issues.len(), 1);
        repair(&mut doc);
        assert!(validate(&doc).is_clean());
    }

    #[test]
    fn duplicate_layer_ids_are_renumbered() {
        let mut doc = samples::benchmark(8);
        let mut copy = doc.layers[0].clone();
        copy.name = "copy".into();
        doc.layers.push(copy);
        let report = validate(&doc);
        assert!(report.unresolved_errors().count() > 0);
        repair(&mut doc);
        assert!(validate(&doc).is_clean(), "{}", validate(&doc));
    }

    #[test]
    fn bad_document_settings_are_repaired() {
        let mut doc = rigged();
        doc.fps = 0.0;
        doc.end = -5;
        repair(&mut doc);
        // keys past the shortened range remain as warnings, not errors
        assert_eq!(
            validate(&doc).unresolved_errors().count(),
            0,
            "{}",
            validate(&doc)
        );
        assert_eq!(doc.fps, 24.0);
    }
}
