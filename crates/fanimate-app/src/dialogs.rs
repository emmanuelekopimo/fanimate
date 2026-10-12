//! Dialogs: first-run welcome, preferences, shortcut list, about, the
//! "save changes?" prompt, and the command palette.

use eframe::egui::{self, Id, Key, RichText};
use fanimate_editor::{Action, KeymapKind};

use crate::app::{Shell, run_unchecked, save};
use crate::theme::Theme;

#[derive(Default)]
pub struct PaletteState {
    query: String,
    selected: usize,
}

pub fn show(ctx: &egui::Context, shell: &mut Shell) {
    if shell.ui.show_welcome {
        welcome(ctx, shell);
    }
    if shell.ui.confirm_discard.is_some() {
        confirm_discard(ctx, shell);
    }
    if shell.ui.palette.is_some() {
        palette(ctx, shell);
    }
    if shell.ui.show_preferences {
        preferences(ctx, shell);
    }
    if shell.ui.show_shortcuts {
        shortcuts(ctx, shell);
    }
    if shell.ui.show_about {
        about(ctx, shell);
    }
}

fn keymap_choice(ui: &mut egui::Ui, shell: &mut Shell) {
    for kind in KeymapKind::ALL {
        let selected = shell.keymap.kind == kind;
        let frame = egui::Frame::group(ui.style())
            .fill(if selected {
                shell.palette.accent.gamma_multiply(0.25)
            } else {
                shell.palette.bg
            })
            .stroke(egui::Stroke::new(
                1.0,
                if selected {
                    shell.palette.accent
                } else {
                    shell.palette.border
                },
            ));
        let inner = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            let radio = ui.radio(selected, RichText::new(kind.label()).strong());
            ui.label(RichText::new(kind.description()).color(shell.palette.text_muted));
            radio.clicked()
        });
        // the whole card is clickable, not just the radio button
        let card = inner.response.interact(egui::Sense::click());
        if (inner.inner || card.clicked()) && !selected {
            shell.set_keymap(kind);
        }
        ui.add_space(4.0);
    }
}

fn theme_choice(ui: &mut egui::Ui, shell: &mut Shell) {
    ui.horizontal_wrapped(|ui| {
        for theme in Theme::ALL {
            if ui.selectable_label(shell.theme == theme, theme.label()).clicked() {
                shell.set_theme(ui.ctx(), theme);
            }
        }
    });
}

fn welcome(ctx: &egui::Context, shell: &mut Shell) {
    let modal = egui::Modal::new(Id::new("welcome")).show(ctx, |ui| {
        ui.set_width(460.0);
        ui.horizontal(|ui| {
            ui.add(egui::Image::new((shell.icon.id(), egui::vec2(40.0, 40.0))));
            ui.vertical(|ui| {
                ui.label(RichText::new("Welcome to Fanimate").size(20.0).strong());
                ui.label(RichText::new("Free to animate.").color(shell.palette.accent_strong));
            });
        });
        ui.add_space(10.0);
        ui.label(RichText::new("Keyboard shortcuts").strong());
        ui.label(
            RichText::new(
                "Coming from Moho? Keep its shortcuts. You can switch any time in Edit › Keyboard Shortcuts.",
            )
            .color(shell.palette.text_muted),
        );
        ui.add_space(4.0);
        keymap_choice(ui, shell);
        ui.add_space(6.0);
        ui.label(RichText::new("Theme").strong());
        theme_choice(ui, shell);
        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Start animating").color(shell.palette.on_button))
                        .fill(shell.palette.button),
                )
                .clicked()
            {
                shell.ui.show_welcome = false;
            }
        });
    });
    if modal.should_close() {
        shell.ui.show_welcome = false;
    }
}

fn confirm_discard(ctx: &egui::Context, shell: &mut Shell) {
    let Some(pending) = shell.ui.confirm_discard else {
        return;
    };
    let mut answer: Option<bool> = None; // Some(save?) ; None = undecided
    let mut cancel = false;
    let modal = egui::Modal::new(Id::new("confirm-discard")).show(ctx, |ui| {
        ui.set_width(380.0);
        ui.label(
            RichText::new(format!("Save changes to “{}”?", shell.editor.doc.name))
                .size(16.0)
                .strong(),
        );
        ui.label(
            RichText::new("Your changes will be lost if you don't save them.")
                .color(shell.palette.text_muted),
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new("Save").color(shell.palette.on_button))
                        .fill(shell.palette.button),
                )
                .clicked()
            {
                answer = Some(true);
            }
            if ui.button("Don't Save").clicked() {
                answer = Some(false);
            }
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
    });
    if cancel || modal.should_close() {
        shell.ui.confirm_discard = None;
        return;
    }
    if let Some(save_first) = answer {
        shell.ui.confirm_discard = None;
        if save_first && !save(shell, false) {
            return; // save cancelled or failed: keep the work
        }
        run_unchecked(ctx, shell, pending);
    }
}

fn palette(ctx: &egui::Context, shell: &mut Shell) {
    let Some(state) = shell.ui.palette.as_mut() else {
        return;
    };
    let query = state.query.to_lowercase();
    let matches: Vec<Action> = Action::all()
        .into_iter()
        .filter(|a| {
            query
                .split_whitespace()
                .all(|w| a.label().to_lowercase().contains(w))
        })
        .collect();
    state.selected = state.selected.min(matches.len().saturating_sub(1));

    let (up, down, enter, escape) = ctx.input(|i| {
        (
            i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::Enter),
            i.key_pressed(Key::Escape),
        )
    });
    if down {
        state.selected = (state.selected + 1).min(matches.len().saturating_sub(1));
    }
    if up {
        state.selected = state.selected.saturating_sub(1);
    }

    let mut run: Option<Action> = None;
    let modal = egui::Modal::new(Id::new("command-palette")).show(ctx, |ui| {
        ui.set_width(440.0);
        let state = shell.ui.palette.as_mut().expect("checked above");
        let edit = ui.add(
            egui::TextEdit::singleline(&mut state.query)
                .hint_text("Type a command…")
                .desired_width(f32::INFINITY),
        );
        edit.request_focus();
        let selected = state.selected;
        ui.add_space(4.0);
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for (i, action) in matches.iter().enumerate() {
                let enabled = shell.editor.can_run(*action);
                let text = RichText::new(action.label()).color(if enabled {
                    shell.palette.text
                } else {
                    shell.palette.text_muted
                });
                let r = ui.add(
                    egui::Button::selectable(i == selected, text)
                        .shortcut_text(shell.keymap.shortcut_text(*action))
                        .min_size(egui::vec2(ui.available_width(), 0.0)),
                );
                if i == selected {
                    r.scroll_to_me(None);
                }
                if r.clicked() && enabled {
                    run = Some(*action);
                }
            }
        });
    });
    if enter
        && let Some(a) = matches.get(shell.ui.palette.as_ref().map_or(0, |s| s.selected))
        && shell.editor.can_run(*a)
    {
        run = Some(*a);
    }
    if escape || modal.should_close() || run.is_some() {
        shell.ui.palette = None;
    }
    if let Some(a) = run {
        shell.pending.push(a);
    }
}

fn preferences(ctx: &egui::Context, shell: &mut Shell) {
    let mut open = true;
    egui::Window::new("Preferences")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .show(ctx, |ui| {
            ui.label(RichText::new("Keyboard shortcuts").strong());
            keymap_choice(ui, shell);
            if ui.link("Show all shortcuts").clicked() {
                shell.ui.show_shortcuts = true;
            }
            ui.add_space(8.0);
            ui.label(RichText::new("Theme").strong());
            theme_choice(ui, shell);
        });
    shell.ui.show_preferences = open;
}

fn shortcuts(ctx: &egui::Context, shell: &mut Shell) {
    let mut open = true;
    egui::Window::new(format!("Keyboard Shortcuts: {}", shell.keymap.kind.label()))
        .open(&mut open)
        .collapsible(false)
        .default_size([420.0, 520.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for kind in KeymapKind::ALL {
                    if ui
                        .selectable_label(shell.keymap.kind == kind, kind.label())
                        .clicked()
                    {
                        shell.set_keymap(kind);
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::Grid::new("shortcut-grid")
                    .striped(true)
                    .num_columns(2)
                    .show(ui, |ui| {
                        for action in Action::all() {
                            let keys = shell.keymap.shortcut_text(action);
                            if keys.is_empty() {
                                continue;
                            }
                            ui.label(action.label());
                            ui.label(RichText::new(keys).monospace().color(shell.palette.accent_strong));
                            ui.end_row();
                        }
                    });
            });
        });
    shell.ui.show_shortcuts = open;
}

fn about(ctx: &egui::Context, shell: &mut Shell) {
    let mut open = true;
    egui::Window::new("About Fanimate")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new((shell.icon.id(), egui::vec2(48.0, 48.0))));
                ui.vertical(|ui| {
                    ui.label(RichText::new("Fanimate").size(20.0).strong());
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.label(RichText::new("Free to animate.").color(shell.palette.accent_strong));
                });
            });
            ui.add_space(6.0);
            ui.label("Free, open-source 2D character animation.");
            ui.label(RichText::new("Licensed under MIT or Apache 2.0.").color(shell.palette.text_muted));
            ui.label(RichText::new(format!("Graphics: {}", shell.gpu_name)).color(shell.palette.text_muted));
            ui.hyperlink_to(
                "github.com/toonittmedia/fanimate",
                "https://github.com/toonittmedia/fanimate",
            );
        });
    shell.ui.show_about = open;
}
