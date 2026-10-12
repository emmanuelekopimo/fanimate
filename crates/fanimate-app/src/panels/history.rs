//! The History panel: every undo step, the layers it changed, and the
//! current state. Click a step to go back (or forward) to it.

use eframe::egui::{self, Id, RichText, Sense, vec2};
use fanimate_editor::Action;

use crate::app::Shell;
use crate::icons::{self, Icon};
use crate::menus;

const ROW_HEIGHT: f32 = 34.0;

pub fn show(ui: &mut egui::Ui, shell: &mut Shell) {
    let p = shell.palette;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let action = Action::UndoLayer;
        let enabled = shell.editor.can_run(action);
        let shortcut = shell.keymap.shortcut_text(action);
        let shortcut = if shortcut.is_empty() {
            shortcut
        } else {
            format!(" ({shortcut})")
        };
        let tip = format!(
            "{}{shortcut}: take the selected layer back to before its last change",
            action.label()
        );
        if icons::button(ui, Icon::History, 24.0, false, !enabled)
            .on_hover_text(tip)
            .clicked()
            && enabled
        {
            shell.pending.push(action);
        }
        ui.label(
            RichText::new("Kept with your work if Fanimate closes unexpectedly")
                .size(11.0)
                .color(p.text_muted),
        );
    });
    ui.separator();

    let steps = shell.editor.history.steps(&shell.editor.doc);
    let cursor = shell.editor.history.cursor();
    // state 0 is where the history starts; state i + 1 follows step i
    let rows: Vec<(usize, String, String, bool)> =
        std::iter::once((0, "Start".to_owned(), String::new(), false))
            .chain(steps.into_iter().enumerate().map(|(i, s)| {
                let layers = s.layers.join(", ");
                (i + 1, s.label, layers, s.undone)
            }))
            .collect();

    egui::ScrollArea::vertical()
        .id_salt("history-rows")
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (state, label, layers, undone) in rows {
                let (rect, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
                let response = ui.interact(rect, Id::new(("history-row", state)), Sense::click());
                let current = state == cursor;
                let painter = ui.painter_at(rect);
                if current {
                    painter.rect_filled(rect, 2.0, p.accent.gamma_multiply(0.25));
                } else if response.hovered() {
                    painter.rect_filled(rect, 2.0, p.accent.gamma_multiply(0.1));
                }
                let color = if undone { p.text_muted } else { p.text };
                painter.text(
                    rect.left_top() + vec2(8.0, 4.0),
                    egui::Align2::LEFT_TOP,
                    &label,
                    egui::FontId::proportional(13.0),
                    color,
                );
                if !layers.is_empty() {
                    painter.text(
                        rect.left_top() + vec2(8.0, 19.0),
                        egui::Align2::LEFT_TOP,
                        &layers,
                        egui::FontId::proportional(11.0),
                        p.text_muted,
                    );
                }
                if response.clicked() && !current {
                    shell.pending.push(Action::GoToHistory(state));
                }
                menus::context_menu(&response, |ui| {
                    menus::item(ui, shell, Action::GoToHistory(state));
                    ui.separator();
                    menus::item(ui, shell, Action::Undo);
                    menus::item(ui, shell, Action::Redo);
                    menus::item(ui, shell, Action::UndoLayer);
                });
            }
        });
}
