//! The Style panel: fill and stroke of the shapes on the selected layer.

use eframe::egui::{self, RichText};
use fanimate_core::{LayerKind, Stroke};

use crate::app::Shell;
use crate::panels::properties::{begin_edit, end_edits, from_color32, to_color32};

pub fn show(ui: &mut egui::Ui, shell: &mut Shell) {
    ui.add_space(4.0);
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
        ui.label(
            RichText::new("This layer has no shapes yet. Drawing tools arrive in milestone M2.").color(muted),
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
