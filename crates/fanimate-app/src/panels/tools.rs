//! The Tools panel, grouped as in Moho Pro.

use eframe::egui::{self, RichText};
use fanimate_editor::{Action, Tool, ToolGroup};

use crate::app::Shell;
use crate::icons::{self, Icon};
use crate::menus::tool_shortcut;

pub fn show(ui: &mut egui::Ui, shell: &mut Shell) {
    let p = shell.palette;
    ui.add_space(4.0);
    for group in ToolGroup::ALL {
        ui.label(RichText::new(group.label()).size(11.0).color(p.text_muted));
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
            for tool in Tool::ALL.into_iter().filter(|t| t.group() == group) {
                let selected = shell.editor.tool == tool;
                let tip = format!("{}{}\n{}", tool.label(), tool_shortcut(shell, tool), tool.hint());
                if icons::button(ui, Icon::Tool(tool), 32.0, selected, !tool.is_available())
                    .on_hover_text(tip)
                    .clicked()
                {
                    shell.pending.push(Action::Tool(tool));
                }
            }
        });
        ui.add_space(6.0);
    }
}
