//! Menu contents, shared by the title-bar menus and right-click menus so
//! the same command always looks and behaves the same.

use eframe::egui;
use fanimate_editor::{Action, KeymapKind, Tool};

use crate::app::Shell;
use crate::panels::Tab;
use crate::theme::{self, Theme};

/// Menus are at least this wide, so short ones don't look like tooltips.
pub const MENU_MIN_WIDTH: f32 = 200.0;

/// The right-click menu for `response`, styled like the menu bar's menus.
pub fn context_menu(response: &egui::Response, contents: impl FnOnce(&mut egui::Ui)) {
    egui::Popup::context_menu(response)
        .style(theme::menu_style)
        .show(|ui| {
            ui.set_min_width(MENU_MIN_WIDTH);
            contents(ui);
        });
}

/// A button that opens a menu, styled like the menu bar's menus.
pub fn menu_button(
    ui: &mut egui::Ui,
    button: egui::Button<'_>,
    contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let config = egui::containers::menu::MenuConfig::new().style(theme::menu_style);
    egui::containers::menu::MenuButton::from_button(button)
        .config(config)
        .ui(ui, |ui| {
            ui.set_min_width(MENU_MIN_WIDTH);
            contents(ui);
        })
        .0
}

/// A menu item that queues `action`, shows its shortcut from the active
/// keymap, and is greyed out when it cannot run.
pub fn item(ui: &mut egui::Ui, shell: &mut Shell, action: Action) {
    let label = match action {
        Action::Undo => shell
            .editor
            .history
            .undo_label()
            .map_or("Undo".to_owned(), |l| format!("Undo {l}")),
        Action::Redo => shell
            .editor
            .history
            .redo_label()
            .map_or("Redo".to_owned(), |l| format!("Redo {l}")),
        a => a.label().to_owned(),
    };
    // the gap applies on both sides of the stretch before the shortcut, so
    // a long label keeps a clear 24 px from its keys
    let button = egui::Button::new(label)
        .shortcut_text(shell.keymap.shortcut_text(action))
        .gap(12.0);
    if ui.add_enabled(shell.editor.can_run(action), button).clicked() {
        shell.pending.push(action);
        ui.close();
    }
}

fn items(ui: &mut egui::Ui, shell: &mut Shell, actions: &[Action]) {
    for a in actions {
        item(ui, shell, *a);
    }
}

pub fn file(ui: &mut egui::Ui, shell: &mut Shell) {
    items(ui, shell, &[Action::New, Action::Open]);
    ui.menu_button("Open Sample", |ui| {
        if ui.button("Hello Fanimate").clicked() {
            shell.editor = fanimate_editor::Editor::sample();
            shell.view.fit();
            ui.close();
        }
        if ui.button("Benchmark rig (40 bones)").clicked() {
            shell.editor = fanimate_editor::Editor::new(fanimate_core::samples::benchmark(40));
            shell.view.fit();
            ui.close();
        }
        if ui.button("Stress test (400 bones)").clicked() {
            shell.editor = fanimate_editor::Editor::new(fanimate_core::samples::benchmark(400));
            shell.view.fit();
            ui.close();
        }
    });
    ui.separator();
    items(ui, shell, &[Action::Save, Action::SaveAs]);
    ui.separator();
    item(ui, shell, Action::Quit);
}

pub fn edit(ui: &mut egui::Ui, shell: &mut Shell) {
    items(ui, shell, &[Action::Undo, Action::Redo]);
    ui.separator();
    items(
        ui,
        shell,
        &[
            Action::Cut,
            Action::Copy,
            Action::Paste,
            Action::Duplicate,
            Action::Delete,
        ],
    );
    ui.separator();
    items(ui, shell, &[Action::SelectAll, Action::SelectNone]);
    ui.separator();
    keymap_menu(ui, shell);
    item(ui, shell, Action::Preferences);
}

pub fn keymap_menu(ui: &mut egui::Ui, shell: &mut Shell) {
    ui.menu_button("Keyboard Shortcuts", |ui| {
        for kind in KeymapKind::ALL {
            if ui
                .radio(shell.keymap.kind == kind, kind.label())
                .on_hover_text(kind.description())
                .clicked()
            {
                shell.set_keymap(kind);
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Show All Shortcuts…").clicked() {
            shell.ui.show_shortcuts = true;
            ui.close();
        }
    });
}

pub fn layer(ui: &mut egui::Ui, shell: &mut Shell) {
    items(
        ui,
        shell,
        &[
            Action::NewVectorLayer,
            Action::NewGroupLayer,
            Action::NewBoneLayer,
        ],
    );
    ui.separator();
    items(
        ui,
        shell,
        &[
            Action::Rename,
            Action::ToggleVisibility,
            Action::GroupLayers,
            Action::Ungroup,
            Action::MoveLayerUp,
            Action::MoveLayerDown,
        ],
    );
    ui.separator();
    item(ui, shell, Action::LayerProperties);
}

pub fn animation(ui: &mut egui::Ui, shell: &mut Shell) {
    items(
        ui,
        shell,
        &[
            Action::PlayPause,
            Action::FirstFrame,
            Action::PrevFrame,
            Action::NextFrame,
            Action::LastFrame,
            Action::PrevKey,
            Action::NextKey,
        ],
    );
    ui.separator();
    items(ui, shell, &[Action::AddKey, Action::DeleteKey]);
}

pub fn view(ui: &mut egui::Ui, shell: &mut Shell) {
    items(
        ui,
        shell,
        &[
            Action::ZoomIn,
            Action::ZoomOut,
            Action::FitView,
            Action::ActualSize,
        ],
    );
    ui.separator();
    ui.menu_button("Theme", |ui| {
        for theme in Theme::ALL {
            if ui.radio(shell.theme == theme, theme.label()).clicked() {
                shell.set_theme(ui.ctx(), theme);
                ui.close();
            }
        }
    });
    ui.separator();
    item(ui, shell, Action::CommandPalette);
}

pub fn window(ui: &mut egui::Ui, shell: &mut Shell) {
    for tab in Tab::ALL {
        if ui.button(tab.title()).clicked() {
            shell.ui.open_tab = Some(tab);
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Reset Layout").clicked() {
        shell.ui.reset_layout = true;
        ui.close();
    }
}

pub fn help(ui: &mut egui::Ui, shell: &mut Shell) {
    if ui.button("Keyboard Shortcuts…").clicked() {
        shell.ui.show_shortcuts = true;
        ui.close();
    }
    if ui.button("Product Vision").clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(
            "https://github.com/toonittmedia/fanimate/blob/main/docs/product-vision.md",
        ));
        ui.close();
    }
    if ui.button("Report a Bug").clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(
            "https://github.com/toonittmedia/fanimate/issues/new/choose",
        ));
        ui.close();
    }
    ui.separator();
    if ui.button("About Fanimate").clicked() {
        shell.ui.show_about = true;
        ui.close();
    }
}

/// Right-click menu contents for layers (Layers panel rows and the canvas).
pub fn layer_context(ui: &mut egui::Ui, shell: &mut Shell) {
    ui.menu_button("New", |ui| {
        items(
            ui,
            shell,
            &[
                Action::NewVectorLayer,
                Action::NewGroupLayer,
                Action::NewBoneLayer,
            ],
        );
    });
    ui.separator();
    items(
        ui,
        shell,
        &[
            Action::Cut,
            Action::Copy,
            Action::Paste,
            Action::Duplicate,
            Action::Delete,
        ],
    );
    ui.separator();
    items(
        ui,
        shell,
        &[
            Action::Rename,
            Action::ToggleVisibility,
            Action::GroupLayers,
            Action::Ungroup,
            Action::MoveLayerUp,
            Action::MoveLayerDown,
        ],
    );
    ui.separator();
    items(ui, shell, &[Action::AddKey, Action::DeleteKey]);
    ui.separator();
    item(ui, shell, Action::LayerProperties);
}

/// Tool shortcut for tooltips, e.g. " (V)".
pub fn tool_shortcut(shell: &Shell, tool: Tool) -> String {
    let s = shell.keymap.shortcut_text(Action::Tool(tool));
    if s.is_empty() { s } else { format!(" ({s})") }
}
