//! Dialogs: first-run welcome, preferences, shortcut list, about, the
//! "save changes?" prompt, the command palette, reporting a crash and
//! updating Fanimate.

use eframe::egui::{self, Id, Key, RichText};
use fanimate_editor::{Action, KeymapKind};

use crate::app::{Shell, run_unchecked, save};
use crate::theme::Theme;

#[derive(Default)]
pub struct PaletteState {
    query: String,
    selected: usize,
    /// The search field has had focus; it asks once, when the palette opens.
    focused: bool,
}

pub fn show(ctx: &egui::Context, shell: &mut Shell) {
    // recovering work after a crash comes before everything else
    if !shell.recovery.offered.is_empty() {
        crate::recovery::dialog(ctx, shell);
        return;
    }
    // then the crash that lost it, if the user wants to report it
    if shell.crash.is_some() {
        crash(ctx, shell);
        return;
    }
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
    if shell.ui.show_update {
        update(ctx, shell);
    }
}

fn primary(shell: &Shell, text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned()).color(shell.palette.on_button))
        .fill(shell.palette.button)
}

/// "Fanimate closed unexpectedly": report the crash, or not.
fn crash(ctx: &egui::Context, shell: &mut Shell) {
    let Some(pending) = &shell.crash else { return };
    let muted = shell.palette.text_muted;
    let mut report = false;
    let mut close = false;
    let mut ask = shell.crash_reports;
    egui::Modal::new(Id::new("crash-report")).show(ctx, |ui| {
        ui.set_width(460.0);
        ui.label(RichText::new("Fanimate closed unexpectedly").size(16.0).strong());
        ui.label(
            RichText::new(
                "Reporting the crash helps fix it. Fanimate opens a new issue on GitHub with the report \
                 filled in, for you to read and send. Nothing is sent unless you send it.",
            )
            .color(muted),
        );
        ui.add_space(6.0);
        egui::CollapsingHeader::new("Show the report").show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                let mut text = pending.text.as_str();
                ui.add(
                    egui::TextEdit::multiline(&mut text)
                        .code_editor()
                        .desired_width(f32::INFINITY),
                );
            });
        });
        ui.add_space(6.0);
        ui.checkbox(&mut ask, "Offer to report crashes");
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.add(primary(shell, "Report on GitHub…")).clicked() {
                report = true;
            }
            if ui.button("Don't report").clicked() {
                close = true;
            }
        });
    });
    shell.crash_reports = ask;
    if report {
        ctx.open_url(egui::OpenUrl::new_tab(crate::crash::issue_url(pending)));
    }
    if (report || close)
        && let Some(p) = shell.crash.take()
    {
        crate::crash::dismiss(&p);
    }
}

/// The Update Fanimate window.
fn update(ctx: &egui::Context, shell: &mut Shell) {
    use crate::update::{Install, State, Version};
    let state = shell.updater.state();
    let muted = shell.palette.text_muted;
    let mut open = true;
    let mut close = false;
    egui::Window::new("Update Fanimate")
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .show(ctx, |ui| match &state {
            State::Idle | State::Checking { .. } | State::UpToDate | State::Failed(_) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Checking for updates…");
                });
            }
            State::Available(r) => {
                ui.label(RichText::new(format!("Fanimate {} is available", r.version)).strong());
                ui.label(RichText::new(format!("You have version {}.", Version::current())).color(muted));
                ui.hyperlink_to("What's new", &r.page);
                ui.add_space(6.0);
                match (&r.file, &shell.install) {
                    (Some(_), _) => {
                        ui.horizontal(|ui| {
                            if ui.add(primary(shell, "Download")).clicked()
                                && let Some(dir) = eframe::storage_dir("Fanimate")
                            {
                                shell.updater.download(r.clone(), dir.join("updates"));
                            }
                            if ui.button("Skip this version").clicked() {
                                shell.updates.skipped = Some(r.version.to_string());
                                shell.updater.reset();
                                close = true;
                            }
                        });
                    }
                    (None, Install::Managed) => {
                        ui.label(
                            RichText::new("Update Fanimate through your system's software centre.")
                                .color(muted),
                        );
                    }
                    (None, _) => {
                        ui.label(
                            RichText::new(
                                "This copy of Fanimate cannot update itself. Download the new version:",
                            )
                            .color(muted),
                        );
                        ui.hyperlink_to("Fanimate releases", crate::update::DOWNLOAD_PAGE);
                    }
                }
            }
            State::Downloading(r) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Downloading and checking Fanimate {}…", r.version));
                });
            }
            State::Ready(r, _) => {
                ui.label(RichText::new(format!("Fanimate {} is ready to install", r.version)).strong());
                ui.label(
                    RichText::new(
                        "Fanimate closes to install it and opens again. You are asked to save changes first.",
                    )
                    .color(muted),
                );
                ui.add_space(6.0);
                if ui.add(primary(shell, "Restart and Update")).clicked() {
                    shell.pending.push(Action::InstallUpdate);
                    close = true;
                }
            }
        });
    shell.ui.show_update = open && !close;
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
        ui.add_space(6.0);
        ui.checkbox(&mut shell.updates.check, "Check for updates automatically")
            .on_hover_text("Fanimate asks before downloading or installing anything");
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
    let mut matches = fanimate_editor::search(&Action::all(), &state.query, &shell.recent_actions);
    // what can run now comes first, keeping the search order otherwise
    matches.sort_by_key(|a| !shell.editor.can_run(*a));
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
        if !state.focused {
            edit.request_focus();
            state.focused = true;
        }
        if edit.changed() {
            state.selected = 0;
        }
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
        // most recent first, a few kept
        shell.recent_actions.retain(|r| *r != a);
        shell.recent_actions.insert(0, a);
        shell.recent_actions.truncate(8);
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
            ui.add_space(8.0);
            ui.label(RichText::new("Updates and crashes").strong());
            ui.checkbox(&mut shell.updates.check, "Check for updates automatically")
                .on_hover_text("Turn off to stay on this version, for example during a production");
            if ui.button("Check now").clicked() {
                shell.pending.push(Action::CheckForUpdates);
            }
            ui.checkbox(&mut shell.crash_reports, "After a crash, offer to report it");
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
        .max_width(380.0)
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
            ui.add_space(6.0);
            // separate programs shipped with Fanimate, under their own licences
            let muted = shell.palette.text_muted;
            ui.label(RichText::new("Ships with").strong());
            ui.label(
                RichText::new(
                    "FFmpeg and x264 for video export (GPL 2.0 or later), run as a separate program; \
                     their sources are published with each release.",
                )
                .color(muted),
            );
            ui.label(RichText::new("Rhubarb Lip Sync for lip sync (MIT).").color(muted));
            ui.label(RichText::new("Inter font (OFL 1.1) and Lucide icons (ISC).").color(muted));
        });
    shell.ui.show_about = open;
}
