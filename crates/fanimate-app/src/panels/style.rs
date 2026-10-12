//! The Style panel: the brush and the style of new artwork, then the fill
//! and stroke of the shapes on the selected layer.

use eframe::egui::{self, RichText};
use fanimate_core::{LayerKind, Stroke};
use fanimate_editor::{Action, Tool};

use crate::app::Shell;
use crate::panels::properties::{begin_edit, end_edits, from_color32, to_color32};

pub fn show(ui: &mut egui::Ui, shell: &mut Shell) {
    ui.add_space(4.0);
    new_artwork(ui, shell);
    ui.separator();
    selected_shapes(ui, shell);
}

/// Settings for what is drawn next. These are not part of the document, so
/// changing them is not an undo step.
fn new_artwork(ui: &mut egui::Ui, shell: &mut Shell) {
    let muted = shell.palette.text_muted;
    ui.label(
        RichText::new("Brush and new shapes")
            .strong()
            .color(shell.palette.text),
    );
    ui.add_space(2.0);
    let d = &mut shell.editor.draw;
    egui::Grid::new("draw-style")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Line").color(muted));
            let mut line = to_color32(d.line);
            if ui.color_edit_button_srgba(&mut line).changed() {
                d.line = from_color32(line);
            }
            ui.end_row();

            ui.label(RichText::new("Width").color(muted));
            ui.add(egui::DragValue::new(&mut d.width).range(0.1..=500.0).speed(0.1))
                .on_hover_text("Brush size at full pressure, and outline width, in canvas pixels");
            ui.end_row();

            ui.label(RichText::new("Outline").color(muted));
            ui.checkbox(&mut d.outline, "").on_hover_text(
                "Give new shapes an outline in the line colour. Shapes with no fill always get one.",
            );
            ui.end_row();

            ui.label(RichText::new("Fill").color(muted));
            ui.horizontal(|ui| {
                let mut on = d.fill.is_some();
                if ui
                    .checkbox(&mut on, "")
                    .on_hover_text("Fill new shapes")
                    .changed()
                {
                    d.fill = on.then_some(d.fill.unwrap_or(fanimate_core::Color::WHITE));
                }
                if let Some(fill) = &mut d.fill {
                    let mut c = to_color32(*fill);
                    if ui.color_edit_button_srgba(&mut c).changed() {
                        *fill = from_color32(c);
                    }
                }
            });
            ui.end_row();

            ui.label(RichText::new("Smoothing").color(muted));
            let mut pct = d.smoothing * 100.0;
            if ui
                .add(
                    egui::Slider::new(&mut pct, 0.0..=100.0)
                        .suffix("%")
                        .max_decimals(0),
                )
                .on_hover_text("Smooths shaky brush strokes")
                .changed()
            {
                d.smoothing = pct / 100.0;
            }
            ui.end_row();

            ui.label(RichText::new("Taper").color(muted));
            ui.add(egui::DragValue::new(&mut d.taper).range(0.0..=500.0).speed(0.5))
                .on_hover_text("Length of the thin ends of a brush stroke, in canvas pixels");
            ui.end_row();

            ui.label(RichText::new("Eraser").color(muted));
            ui.add(egui::DragValue::new(&mut d.eraser).range(1.0..=1000.0).speed(0.5))
                .on_hover_text("Eraser size, in canvas pixels");
            ui.end_row();
        });
    ui.add_space(4.0);
}

fn selected_shapes(ui: &mut egui::Ui, shell: &mut Shell) {
    let muted = shell.palette.text_muted;
    let Some(id) = shell.editor.primary() else {
        ui.label(RichText::new("Select a vector layer to edit its colours.").color(muted));
        return;
    };
    let Some(LayerKind::Vector { shapes }) = shell.editor.doc.find(id).map(|l| l.kind.clone()) else {
        ui.label(RichText::new("Style applies to vector layers.").color(muted));
        return;
    };
    if shapes.is_empty() {
        let key = shell.keymap.shortcut_text(Action::Tool(Tool::Freehand));
        let tool = if key.is_empty() {
            String::new()
        } else {
            format!(" ({key})")
        };
        ui.label(
            RichText::new(format!(
                "This layer has no shapes yet. Draw on it with Freehand{tool}."
            ))
            .color(muted),
        );
        return;
    }

    let mut edited = None;
    egui::Grid::new("style-grid")
        .num_columns(4)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Shape").color(muted));
            ui.label(RichText::new("Fill").color(muted));
            ui.label(RichText::new("Stroke").color(muted));
            ui.label(RichText::new("Width").color(muted));
            ui.end_row();
            for (i, shape) in shapes.iter().enumerate() {
                let mut style = shape.style;
                ui.label(&shape.name);

                let mut fill = style.fill.map(to_color32).unwrap_or(egui::Color32::TRANSPARENT);
                let r = ui.color_edit_button_srgba(&mut fill);
                if r.changed() {
                    style.fill = Some(from_color32(fill));
                    edited = Some((i, style, r));
                }

                let mut stroke = style.stroke.unwrap_or(Stroke {
                    color: fanimate_core::Color::BLACK,
                    width: 0.0,
                });
                let mut c = to_color32(stroke.color);
                let r = ui.color_edit_button_srgba(&mut c);
                if r.changed() {
                    stroke.color = from_color32(c);
                    style.stroke = Some(stroke);
                    edited = Some((i, style, r));
                }
                let r = ui.add(
                    egui::DragValue::new(&mut stroke.width)
                        .range(0.0..=200.0)
                        .speed(0.1),
                );
                if r.changed() {
                    style.stroke = (stroke.width > 0.0).then_some(stroke);
                    edited = Some((i, style, r));
                }
                ui.end_row();
            }
        });

    if let Some((i, style, r)) = edited {
        begin_edit(shell, &r, "Change style");
        if let Some(l) = shell.editor.doc.find_mut(id)
            && let LayerKind::Vector { shapes } = &mut l.kind
            && let Some(s) = shapes.get_mut(i)
        {
            s.style = style;
        }
    }
    end_edits(ui, shell);
}
