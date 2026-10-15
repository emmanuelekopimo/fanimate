//! Export Video: a small floating window (not modal: the animator can keep
//! working) that adds a render to the queue, and the render progress shown
//! in the status bar.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};
use fanimate_core::Frame;
use fanimate_editor::StatusKind;
use fanimate_export::{Format, Queue, Settings, State};

use crate::app::Shell;
use crate::formats::{self, VideoTools};
use crate::render_queue;

/// The Export Video window's choices.
pub struct ExportForm {
    format: Format,
    width: u32,
    height: u32,
    /// Change the height with the width, keeping the canvas's shape.
    keep_aspect: bool,
    start: Frame,
    end: Frame,
    /// Without its extension; the format adds it.
    output: PathBuf,
}

/// Why a render cannot be queued yet.
struct Problem {
    text: String,
    /// Setting up FFmpeg in Preferences would solve it.
    needs_ffmpeg: bool,
}

impl Problem {
    fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            needs_ffmpeg: false,
        }
    }
}

impl ExportForm {
    pub fn new(shell: &Shell, format: Format) -> Self {
        let doc = &shell.editor.doc;
        let s = Settings::for_doc(doc, format);
        // next to the project, or in the Videos folder
        let dir = shell
            .editor
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(videos_folder)
            .unwrap_or_default();
        let name = shell
            .editor
            .path
            .as_deref()
            .and_then(Path::file_stem)
            .map_or_else(|| doc.name.clone(), |s| s.to_string_lossy().into_owned());
        Self {
            format,
            width: s.width,
            height: s.height,
            keep_aspect: true,
            start: s.start,
            end: s.end,
            output: dir.join(safe_name(&name)),
        }
    }

    /// The render's settings, with the FFmpeg chosen in Preferences, if any.
    fn settings(&self, ffmpeg: Option<&Path>) -> Settings {
        Settings {
            format: self.format,
            width: self.width,
            height: self.height,
            start: self.start,
            end: self.end,
            ffmpeg: ffmpeg.map(Path::to_path_buf),
        }
    }

    fn output(&self) -> PathBuf {
        self.format.output(&self.output)
    }

    /// Why the render cannot be queued yet.
    fn problem(&self, tools: &VideoTools) -> Option<Problem> {
        if tools.looking() {
            return Some(Problem::new("Checking which formats this computer can make…"));
        }
        if let Err(why) = tools.maker(self.format) {
            return Some(Problem {
                text: why,
                needs_ffmpeg: true,
            });
        }
        let (min_w, min_h) = self.format.min_size();
        if self.width < min_w || self.height < min_h {
            return Some(Problem::new(format!(
                "{} needs a size of at least {min_w}x{min_h}.",
                self.format.label()
            )));
        }
        let out = self.output();
        match out.parent() {
            Some(dir) if dir.as_os_str().is_empty() || !dir.is_dir() => {
                Some(Problem::new("Choose a folder that exists to save into."))
            }
            None => Some(Problem::new("Choose where to save.")),
            _ if out.file_stem().is_none_or(|s| s.is_empty()) => Some(Problem::new("Give the file a name.")),
            _ => None,
        }
    }
}

/// The user's Videos folder, if there is one.
fn videos_folder() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)?;
    let videos = home.join(if cfg!(target_os = "macos") {
        "Movies"
    } else {
        "Videos"
    });
    Some(if videos.is_dir() { videos } else { home })
}

/// A project name usable as a file name.
fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        "Animation".into()
    } else {
        trimmed.into()
    }
}

/// The Export Video window, while open.
pub fn window(ctx: &egui::Context, shell: &mut Shell) {
    let Some(mut form) = shell.ui.export.take() else {
        return;
    };
    let p = shell.palette;
    let mut open = true;
    let mut queue_it = false;
    let doc_aspect = shell.editor.doc.width / shell.editor.doc.height.max(1.0);
    egui::Window::new("Export Video")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            egui::Grid::new("export-form")
                .num_columns(2)
                .spacing([10.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Format");
                    egui::ComboBox::from_id_salt("export-format")
                        .selected_text(form.format.label())
                        .width(240.0)
                        // every format at once, without scrolling
                        .height(560.0)
                        .show_ui(ui, |ui| {
                            for (i, (group, formats)) in formats::groups().into_iter().enumerate() {
                                if i > 0 {
                                    ui.separator();
                                }
                                ui.label(RichText::new(group).size(11.0).color(p.text_muted));
                                for f in formats.iter().copied() {
                                    // formats this computer cannot make yet stay listed,
                                    // so choosing one explains what they need
                                    let text = if shell.video.maker(f).is_ok() || shell.video.looking() {
                                        RichText::new(f.label())
                                    } else {
                                        RichText::new(format!("{} (needs FFmpeg)", f.label()))
                                            .color(p.text_muted)
                                    };
                                    ui.selectable_value(&mut form.format, f, text)
                                        .on_hover_text(f.hint());
                                }
                            }
                        });
                    ui.end_row();

                    ui.label("");
                    ui.vertical(|ui| {
                        ui.label(RichText::new(form.format.hint()).size(11.0).color(p.text_muted));
                        if let Ok(maker) = shell.video.maker(form.format) {
                            ui.label(RichText::new(maker).size(11.0).color(p.text_muted));
                        }
                    });
                    ui.end_row();

                    ui.label("Size");
                    ui.horizontal(|ui| {
                        let w = ui.add(
                            egui::DragValue::new(&mut form.width)
                                .range(2..=16384)
                                .suffix(" px"),
                        );
                        ui.label("x");
                        let h = ui.add(
                            egui::DragValue::new(&mut form.height)
                                .range(2..=16384)
                                .suffix(" px"),
                        );
                        if form.keep_aspect && w.changed() {
                            form.height = (form.width as f32 / doc_aspect).round().max(2.0) as u32;
                        }
                        if form.keep_aspect && h.changed() {
                            form.width = (form.height as f32 * doc_aspect).round().max(2.0) as u32;
                        }
                    });
                    ui.end_row();

                    ui.label("");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut form.keep_aspect, "Keep shape");
                        for (label, scale) in [("100%", 1.0), ("50%", 0.5)] {
                            if ui.small_button(label).clicked() {
                                form.width = (shell.editor.doc.width * scale).round().max(2.0) as u32;
                                form.height = (shell.editor.doc.height * scale).round().max(2.0) as u32;
                            }
                        }
                    });
                    ui.end_row();

                    ui.label("Frames");
                    ui.horizontal(|ui| {
                        let last = form.end;
                        ui.add(egui::DragValue::new(&mut form.start).range(0..=last));
                        ui.label("to");
                        let first = form.start;
                        ui.add(egui::DragValue::new(&mut form.end).range(first..=Frame::MAX));
                    });
                    ui.end_row();

                    ui.label("Save as");
                    ui.horizontal(|ui| {
                        let shown = form.output().display().to_string();
                        ui.add(
                            egui::Label::new(RichText::new(shown.clone()).color(p.text_muted))
                                .truncate()
                                .selectable(false),
                        )
                        .on_hover_text(shown);
                    });
                    ui.end_row();

                    ui.label("");
                    if ui.button("Browse…").clicked() {
                        browse(&mut form);
                    }
                    ui.end_row();
                });
            ui.add_space(4.0);
            let s = form.settings(None).sanitized();
            let seconds = s.frame_count() as f32 / shell.editor.doc.fps.max(1.0);
            let mut notes = vec![format!("{} frames, {seconds:.1} seconds", s.frame_count())];
            if form.format.has_sound() && !fanimate_export::sound::voices(&shell.editor.doc).is_empty() {
                notes.push("with sound".into());
            } else if !form.format.has_sound() {
                notes.push("no sound".into());
            }
            if (s.width, s.height) != (form.width, form.height) {
                notes.push(format!("made {}x{} for video", s.width, s.height));
            }
            ui.label(RichText::new(notes.join(", ")).size(11.0).color(p.text_muted));
            let problem = form.problem(&shell.video);
            if let Some(problem) = &problem {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&problem.text).size(11.0).color(p.error));
                    if problem.needs_ffmpeg && ui.small_button("Set up FFmpeg…").clicked() {
                        shell.ui.show_preferences = true;
                    }
                });
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let add =
                    egui::Button::new(RichText::new("Add to Render Queue").color(p.on_button)).fill(p.button);
                if ui.add_enabled(problem.is_none(), add).clicked() {
                    queue_it = true;
                }
                ui.label(
                    RichText::new("Renders in its own window; you can keep working.")
                        .size(11.0)
                        .color(p.text_muted),
                );
            });
        });
    shell.export_format = form.format;
    if queue_it {
        add_to_queue(shell, &form);
        open = false;
    }
    if open {
        shell.ui.export = Some(form);
    }
}

fn browse(form: &mut ExportForm) {
    let ext = form.format.extension();
    let mut dialog = rfd::FileDialog::new()
        .set_title("Export Video")
        .add_filter(form.format.label(), &[ext])
        .set_file_name(format!(
            "{}.{ext}",
            form.output.file_name().unwrap_or_default().to_string_lossy()
        ));
    if let Some(dir) = form.output.parent().filter(|d| d.is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    if let Some(path) = dialog.save_file() {
        // a typed extension names the format, unless the chosen one
        // already writes it (.mp4 is H.264, H.265 or AV1)
        if let Some(e) = path.extension()
            && !e.eq_ignore_ascii_case(form.format.extension())
            && let Some(f) = Format::ALL
                .into_iter()
                .find(|f| e.eq_ignore_ascii_case(f.extension()))
        {
            form.format = f;
        }
        form.output = path.with_extension("");
    }
}

fn add_to_queue(shell: &mut Shell, form: &ExportForm) {
    let Some(queue) = render_queue::queue() else {
        shell.editor.set_status(
            StatusKind::Error,
            "There is no folder to keep the render queue in",
        );
        return;
    };
    let output = form.output();
    let doc = shell.editor.doc.mainline();
    match queue.add(&doc, &output, &form.settings(shell.video.chosen.as_deref())) {
        Ok(id) => {
            shell.renders.watch(id);
            match render_queue::open(shell.theme) {
                Ok(()) => shell.editor.set_status(
                    StatusKind::Info,
                    format!("Rendering {} in the render queue", file_name(&output)),
                ),
                Err(e) => shell.editor.set_status(StatusKind::Error, e),
            }
        }
        Err(e) => shell
            .editor
            .set_status(StatusKind::Error, format!("The render could not be queued: {e}")),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |f| f.to_string_lossy().into_owned(),
    )
}

/// Renders this editor queued, followed for the status bar.
#[derive(Default)]
pub struct Renders {
    watching: Vec<String>,
    polled: Option<Instant>,
    /// The render in progress: file name and how far along.
    pub active: Option<(String, f32)>,
}

impl Renders {
    pub fn watch(&mut self, id: String) {
        self.watching.push(id);
        self.polled = None;
    }

    /// Read the queue now and then; report renders that finished.
    pub fn poll(&mut self, ctx: &egui::Context, shell_status: &mut dyn FnMut(StatusKind, String)) {
        if self.watching.is_empty() {
            self.active = None;
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(500));
        if self
            .polled
            .is_some_and(|t| t.elapsed() < Duration::from_millis(500))
        {
            return;
        }
        self.polled = Some(Instant::now());
        let Some(queue) = render_queue::queue() else {
            return;
        };
        self.update(&queue, shell_status);
    }

    fn update(&mut self, queue: &Queue, status: &mut dyn FnMut(StatusKind, String)) {
        let jobs = queue.jobs();
        self.active = None;
        self.watching.retain(|id| {
            let Some(e) = jobs.iter().find(|e| &e.job.id == id) else {
                return false; // removed in the queue window
            };
            let name = file_name(&e.job.output);
            match e.status.state {
                State::Done => {
                    status(StatusKind::Success, format!("Exported {name}"));
                    false
                }
                State::Failed => {
                    let why = e.status.message.as_deref().unwrap_or("no reason given");
                    status(StatusKind::Error, format!("Could not export {name}: {why}"));
                    false
                }
                State::Cancelled => false,
                State::Rendering => {
                    self.active = Some((name, e.status.fraction()));
                    true
                }
                State::Waiting => true,
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_names_become_safe_file_names() {
        assert_eq!(safe_name("Walk: take 2?"), "Walk_ take 2_");
        assert_eq!(safe_name("  "), "Animation");
        assert_eq!(safe_name("end."), "end");
    }

    #[test]
    fn finished_renders_are_reported_once() {
        let dir = std::env::temp_dir().join(format!("fanimate-renders-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let queue = Queue::new(&dir);
        let doc = fanimate_core::samples::hello();
        let s = Settings::for_doc(&doc, Format::Gif);
        let id = queue.add(&doc, Path::new("/x/walk.gif"), &s).unwrap();
        let mut renders = Renders::default();
        renders.watch(id.clone());
        let mut said = Vec::new();
        let rendering = fanimate_export::Status {
            state: State::Rendering,
            done: 12,
            total: 48,
            message: None,
        };
        queue.set_status(&id, &rendering).unwrap();
        renders.update(&queue, &mut |_, m| said.push(m));
        assert_eq!(renders.active, Some(("walk.gif".into(), 0.25)));
        let done = fanimate_export::Status {
            state: State::Done,
            ..rendering
        };
        queue.set_status(&id, &done).unwrap();
        renders.update(&queue, &mut |_, m| said.push(m));
        renders.update(&queue, &mut |_, m| said.push(m));
        assert_eq!(said, ["Exported walk.gif"]);
        assert!(renders.active.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
