//! The Properties panel: layer or project settings, edited in place.
//!
//! Moho edits these in modal dialogs (Layer Settings, Project Settings)
//! that block the canvas. Here they live in a docked panel and update the
//! canvas live. Animated values follow the auto-key rule: if a channel is
//! animated, editing it sets a keyframe on the current frame.

use eframe::egui::{self, RichText};
use fanimate_core::{Color, LayerKind};

use crate::app::Shell;

/// Call before applying a change from `r`: one undo step per drag or edit.
pub fn begin_edit(shell: &mut Shell, r: &egui::Response, label: &'static str) {
    if shell.ui.active_edit != Some(r.id) {
        shell.editor.checkpoint(label);
        shell.ui.active_edit = Some(r.id);
    }
}

/// Forget the active edit once its widget is no longer dragged or focused.
pub fn end_edits(ui: &egui::Ui, shell: &mut Shell) {
    if let Some(id) = shell.ui.active_edit
        && !ui.ctx().is_being_dragged(id)
        && !ui.ctx().memory(|m| m.has_focus(id))
    {
        shell.ui.active_edit = None;
    }
}

pub fn to_color32(c: Color) -> egui::Color32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    egui::Color32::from_rgba_unmultiplied(q(c.r), q(c.g), q(c.b), q(c.a))
}

pub fn from_color32(c: egui::Color32) -> Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color::rgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    )
}

pub fn show(ui: &mut egui::Ui, shell: &mut Shell) {
    ui.add_space(4.0);
    match shell.editor.primary() {
        Some(id) if shell.editor.doc.find(id).is_some() => layer(ui, shell, id),
        _ => project(ui, shell),
    }
    end_edits(ui, shell);
}

/// The keyframe diamond beside an animated value (drawn, not a font glyph).
fn key_mark(ui: &mut egui::Ui, on: bool, color: egui::Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    if on {
        let (c, r) = (rect.center(), 4.0);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(0.0, -r),
                c + egui::vec2(r, 0.0),
                c + egui::vec2(0.0, r),
                c + egui::vec2(-r, 0.0),
            ],
            color,
            egui::Stroke::NONE,
        ));
    }
}

fn heading(ui: &mut egui::Ui, shell: &Shell, text: &str) {
    ui.label(RichText::new(text).strong().color(shell.palette.text));
    ui.add_space(2.0);
}

fn layer(ui: &mut egui::Ui, shell: &mut Shell, id: fanimate_core::LayerId) {
    let Some(l) = shell.editor.doc.find(id) else {
        return;
    };
    let kind = match l.kind {
        LayerKind::Vector { .. } => "Vector layer",
        LayerKind::Group { .. } => "Group",
        LayerKind::Bone { .. } => "Bone layer",
    };
    let mut name = l.name.clone();
    let mut visible = l.visible;
    let animated = (
        l.transform.position.is_animated(),
        l.transform.rotation.is_animated(),
        l.transform.scale.is_animated(),
        l.opacity.is_animated(),
    );
    let muted = shell.palette.text_muted;
    let key_color = shell.palette.keyframe;

    heading(ui, shell, kind);
    egui::Grid::new("layer-props")
        .num_columns(3)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Name");
            let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(160.0));
            if r.lost_focus() {
                shell.editor.rename(id, &name);
            }
            ui.label("");
            ui.end_row();

            ui.label("Visible");
            if ui.checkbox(&mut visible, "").changed() {
                shell.editor.set_visible(id, visible);
            }
            ui.label("");
            ui.end_row();

            let Some(mut t) = shell.editor.layer_transform(id) else {
                return;
            };
            let mut opacity = shell.editor.layer_opacity(id).unwrap_or(1.0) * 100.0;

            ui.label("Opacity");
            let r = ui.add(egui::Slider::new(&mut opacity, 0.0..=100.0).suffix("%"));
            if r.changed() {
                begin_edit(shell, &r, "Change opacity");
                shell.editor.set_layer_opacity(id, opacity / 100.0);
            }
            key_mark(ui, animated.3, key_color);
            ui.end_row();

            ui.label("Position");
            let mut changed = None;
            ui.horizontal(|ui| {
                for v in [&mut t.position.x, &mut t.position.y] {
                    let r = ui.add(egui::DragValue::new(v).speed(1.0).max_decimals(2));
                    if r.changed() {
                        changed = Some(r);
                    }
                }
            });
            key_mark(ui, animated.0, key_color);
            ui.end_row();

            ui.label("Rotation");
            let mut deg = t.rotation.to_degrees();
            let r = ui.add(
                egui::DragValue::new(&mut deg)
                    .speed(0.5)
                    .suffix("°")
                    .max_decimals(1),
            );
            if r.changed() {
                t.rotation = deg.to_radians();
                changed = Some(r);
            }
            key_mark(ui, animated.1, key_color);
            ui.end_row();

            ui.label("Scale");
            ui.horizontal(|ui| {
                let mut pct = t.scale * 100.0;
                for v in [&mut pct.x, &mut pct.y] {
                    let r = ui.add(egui::DragValue::new(v).speed(0.5).suffix("%").max_decimals(1));
                    if r.changed() {
                        changed = Some(r);
                    }
                }
                t.scale = pct / 100.0;
            });
            key_mark(ui, animated.2, key_color);
            ui.end_row();

            if let Some(r) = changed {
                begin_edit(shell, &r, "Transform layer");
                shell.editor.set_layer_transform(id, t);
            }
        });

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        key_mark(ui, true, key_color);
        ui.label(
            RichText::new(format!(
                "marks animated values: editing them sets a keyframe on frame {}. \
                 Other values change for the whole animation.",
                shell.editor.current_frame()
            ))
            .size(11.0)
            .color(muted),
        );
    });
    if let Some(l) = shell.editor.doc.find(id)
        && let LayerKind::Bone { skeleton, .. } = &l.kind
    {
        let names: Vec<String> = skeleton.bones.iter().map(|b| b.name.clone()).collect();
        ui.add_space(8.0);
        heading(ui, shell, "Bones");
        if names.is_empty() {
            ui.label(RichText::new("No bones yet. Draw them with the Add Bone tool.").color(muted));
        }
        let selected = shell
            .editor
            .selected_bone()
            .filter(|b| b.layer == id)
            .map(|b| b.bone);
        for (i, n) in names.iter().enumerate() {
            if ui.selectable_label(selected == Some(i), n).clicked() {
                shell
                    .editor
                    .select_bone(Some(fanimate_editor::BoneRef { layer: id, bone: i }));
            }
        }
        if let Some(b) = shell.editor.selected_bone().filter(|b| b.layer == id) {
            bone(ui, shell, b);
        }
    }
}

/// The selected bone's settings: name, length, limits, pin and strength.
fn bone(ui: &mut egui::Ui, shell: &mut Shell, b: fanimate_editor::BoneRef) {
    let Some(bone) = shell.editor.bone(b).cloned() else {
        return;
    };
    ui.add_space(6.0);
    heading(ui, shell, "Selected bone");
    let mut edited = bone.clone();
    let mut changed: Option<(egui::Response, &'static str)> = None;
    egui::Grid::new("bone-props")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Name");
            let r = ui.add(egui::TextEdit::singleline(&mut edited.name).desired_width(140.0));
            if r.changed() {
                changed = Some((r, "Rename bone"));
            }
            ui.end_row();
            ui.label("Length");
            let r = ui.add(
                egui::DragValue::new(&mut edited.length)
                    .range(0.5..=100_000.0)
                    .speed(0.5),
            );
            if r.changed() {
                changed = Some((r, "Edit bone"));
            }
            ui.end_row();
            ui.label("Pinned");
            let r = ui
                .checkbox(&mut edited.pinned, "")
                .on_hover_text("Stays put when bones further down its chain are dragged");
            if r.changed() {
                changed = Some((r, "Pin bone"));
            }
            ui.end_row();
            ui.label("Angle limits");
            ui.horizontal(|ui| {
                let mut on = edited.limits.is_some();
                let r = ui.checkbox(&mut on, "");
                if r.changed() {
                    edited.limits = on.then_some([-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2]);
                    changed = Some((r, "Change bone limits"));
                }
                if let Some([lo, hi]) = &mut edited.limits {
                    let (mut a, mut z) = (lo.to_degrees(), hi.to_degrees());
                    let r1 = ui.add(
                        egui::DragValue::new(&mut a)
                            .range(-360.0..=z)
                            .suffix("°")
                            .speed(0.5),
                    );
                    ui.label("to");
                    let r2 = ui.add(
                        egui::DragValue::new(&mut z)
                            .range(a..=360.0)
                            .suffix("°")
                            .speed(0.5),
                    );
                    *lo = a.to_radians();
                    *hi = z.to_radians();
                    for r in [r1, r2] {
                        if r.changed() {
                            changed = Some((r, "Change bone limits"));
                        }
                    }
                }
            });
            ui.end_row();
            ui.label("Strength");
            let mut s = edited.influence();
            let r = ui
                .add(egui::DragValue::new(&mut s).range(1.0..=100_000.0).speed(0.5))
                .on_hover_text("How far the bone reaches when artwork is bound smoothly");
            if r.changed() {
                edited.strength = Some(s);
                changed = Some((r, "Change bone strength"));
            }
            ui.end_row();
        });
    if let Some((r, label)) = changed {
        begin_edit(shell, &r, label);
        shell.editor.edit_bone(b, |bone| *bone = edited);
    }
}

fn project(ui: &mut egui::Ui, shell: &mut Shell) {
    heading(ui, shell, "Project");
    let doc = &shell.editor.doc;
    let (mut name, mut w, mut h, mut fps, mut start, mut end, mut bg) = (
        doc.name.clone(),
        doc.width,
        doc.height,
        doc.fps,
        doc.start,
        doc.end,
        to_color32(doc.background),
    );
    let mut edits: Vec<(egui::Response, &'static str)> = Vec::new();
    egui::Grid::new("project-props")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Name");
            let r = ui.add(egui::TextEdit::singleline(&mut name).desired_width(160.0));
            if r.changed() {
                edits.push((r, "Rename project"));
            }
            ui.end_row();
            ui.label("Size");
            ui.horizontal(|ui| {
                for v in [&mut w, &mut h] {
                    let r = ui.add(
                        egui::DragValue::new(v)
                            .range(16.0..=16384.0)
                            .speed(1.0)
                            .suffix(" px"),
                    );
                    if r.changed() {
                        edits.push((r, "Resize canvas"));
                    }
                }
            });
            ui.end_row();
            ui.label("Frame rate");
            let r = ui.add(
                egui::DragValue::new(&mut fps)
                    .range(1.0..=120.0)
                    .speed(0.1)
                    .suffix(" fps"),
            );
            if r.changed() {
                edits.push((r, "Change frame rate"));
            }
            ui.end_row();
            ui.label("Frames");
            ui.horizontal(|ui| {
                let r = ui.add(egui::DragValue::new(&mut start).range(0..=end).speed(0.2));
                if r.changed() {
                    edits.push((r, "Change frame range"));
                }
                ui.label("to");
                let r = ui.add(egui::DragValue::new(&mut end).range(start..=100_000).speed(0.2));
                if r.changed() {
                    edits.push((r, "Change frame range"));
                }
            });
            ui.end_row();
            ui.label("Background");
            let r = ui.color_edit_button_srgba(&mut bg);
            if r.changed() {
                edits.push((r, "Change background"));
            }
            ui.end_row();
        });
    if let Some((r, label)) = edits.into_iter().next() {
        begin_edit(shell, &r, label);
        let doc = &mut shell.editor.doc;
        doc.name = name;
        doc.width = w;
        doc.height = h;
        doc.fps = fps;
        doc.start = start;
        doc.end = end;
        doc.background = from_color32(bg);
        if label == "Resize canvas" {
            shell.view.fit();
        }
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new("Select a layer to edit its properties.")
            .size(11.0)
            .color(shell.palette.text_muted),
    );
}
