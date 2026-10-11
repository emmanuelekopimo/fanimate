//! The dockable panels and the status bar.

mod canvas;
mod layers;
mod properties;
mod style;
mod timeline;
mod tools;

use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};
use fanimate_editor::StatusKind;
use serde::{Deserialize, Serialize};

use crate::app::Shell;

const STATUS_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tab {
    Tools,
    Canvas,
    Timeline,
    Layers,
    Properties,
    Style,
    Actions,
}

impl Tab {
    pub const ALL: [Tab; 7] = [
        Self::Canvas,
        Self::Tools,
        Self::Layers,
        Self::Timeline,
        Self::Properties,
        Self::Style,
        Self::Actions,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Tools => "Tools",
            Self::Canvas => "Canvas",
            Self::Timeline => "Timeline",
            Self::Layers => "Layers",
            Self::Properties => "Properties",
            Self::Style => "Style",
            Self::Actions => "Actions",
        }
    }
}

pub struct TabViewer<'a> {
    pub shell: &'a mut Shell,
}

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = Tab;

    fn title(&mut self, tab: &mut Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn id(&mut self, tab: &mut Tab) -> egui::Id {
        egui::Id::new(("fanimate-tab", *tab))
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Tab) {
        let shell = &mut *self.shell;
        match tab {
            Tab::Tools => tools::show(ui, shell),
            Tab::Canvas => canvas::show(ui, shell),
            Tab::Timeline => timeline::show(ui, shell),
            Tab::Layers => layers::show(ui, shell),
            Tab::Properties => properties::show(ui, shell),
            Tab::Style => style::show(ui, shell),
            Tab::Actions => {
                ui.add_space(8.0);
                ui.label(RichText::new("Actions and Smart Bones").strong());
                ui.label(
                    RichText::new(
                        "Smart Bone actions, morphs and on-canvas controls arrive in milestone M3, \
                         with the rigging system.",
                    )
                    .color(shell.palette.text_muted),
                );
            }
        }
    }

    fn is_closeable(&self, tab: &Tab) -> bool {
        *tab != Tab::Canvas
    }

    fn scroll_bars(&self, tab: &Tab) -> [bool; 2] {
        match tab {
            Tab::Canvas | Tab::Timeline | Tab::Layers => [false, false],
            _ => [false, true],
        }
    }
}

/// Bottom bar: messages or the tool hint on the left, state on the right.
pub fn status_bar(ui: &mut egui::Ui, shell: &mut Shell) {
    // expire messages after a few seconds
    let current = shell.editor.status.as_ref().map(|(_, m)| m.clone());
    match (&current, &shell.ui.status_since) {
        (Some(m), Some((seen, at))) if m == seen => {
            if at.elapsed() > STATUS_TIMEOUT {
                shell.editor.status = None;
                shell.ui.status_since = None;
            }
        }
        (Some(m), _) => shell.ui.status_since = Some((m.clone(), Instant::now())),
        (None, _) => shell.ui.status_since = None,
    }
    if shell.editor.status.is_some() {
        ui.ctx().request_repaint_after(Duration::from_secs(1));
    }

    let p = shell.palette;
    egui::Panel::bottom("status")
        .exact_size(24.0)
        .frame(
            egui::Frame::NONE
                .fill(p.bg)
                .inner_margin(egui::Margin::symmetric(10, 0)),
        )
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let (text, color) = match &shell.editor.status {
                    Some((StatusKind::Error, m)) => (m.clone(), p.error),
                    Some((StatusKind::Warning, m)) => (m.clone(), p.keyframe),
                    Some((StatusKind::Success, m)) => (m.clone(), p.success),
                    Some((StatusKind::Info, m)) => (m.clone(), p.text),
                    None => (
                        format!("{}: {}", shell.editor.tool.label(), shell.editor.tool.hint()),
                        p.text_muted,
                    ),
                };
                ui.label(RichText::new(text).size(12.0).color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let muted = |s: String| RichText::new(s).size(12.0).color(p.text_muted);
                    ui.label(muted(shell.gpu_name.clone()));
                    ui.separator();
                    if ui
                        .add(
                            egui::Label::new(muted(format!("⌨ {}", shell.keymap.kind.label())))
                                .sense(egui::Sense::click()),
                        )
                        .on_hover_text("Keyboard shortcuts. Click to change.")
                        .clicked()
                    {
                        shell.ui.show_preferences = true;
                    }
                    ui.separator();
                    ui.label(muted(format!(
                        "{:.0}%",
                        shell.view.zoom * ui.ctx().pixels_per_point() * 100.0
                    )));
                    ui.separator();
                    let doc = &shell.editor.doc;
                    ui.label(muted(format!(
                        "Frame {} of {}",
                        shell.editor.current_frame(),
                        doc.end
                    )));
                });
            });
        });
}
