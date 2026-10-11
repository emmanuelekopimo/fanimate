//! The app: window chrome, docking layout, dialogs and command dispatch.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, CursorIcon, Key, Modifiers};
use egui_dock::{DockArea, DockState, NodeIndex};
use fanimate_core::LayerId;
use fanimate_editor::{Action, Chord, Editor, Keymap, KeymapKind, Outcome, StatusKind};
use serde::{Deserialize, Serialize};

use crate::canvas_gpu::CanvasGpu;
use crate::panels::{self, Tab, TabViewer};
use crate::theme::{self, Palette, Theme};
use crate::{dialogs, titlebar};

const SETTINGS_KEY: &str = "fanimate.settings";
const DOCK_KEY: &str = "fanimate.dock";
const BENCH_DURATION: Duration = Duration::from_secs(10);

/// Remembered between sessions.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    keymap: String,
    theme: Theme,
    welcome_done: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keymap: KeymapKind::Fanimate.id().into(),
            theme: Theme::Night,
            welcome_done: false,
        }
    }
}

/// Canvas camera: which document point is centred, and how zoomed.
pub struct CanvasView {
    pub center: fanimate_core::Vec2,
    /// Screen points per document unit.
    pub zoom: f32,
    /// Fit the canvas to the panel on the next frame.
    pub fit_pending: bool,
}

impl CanvasView {
    pub fn fit(&mut self) {
        self.fit_pending = true;
    }
}

pub struct Bench {
    started: Instant,
    frames: u32,
    cpu: Duration,
}

/// Transient UI state, not part of the document.
#[derive(Default)]
pub struct UiState {
    /// Layer being renamed in place, and the text so far.
    pub renaming: Option<(LayerId, String)>,
    /// Give the rename field focus (and select its text) on the next frame.
    pub rename_focus: bool,
    pub layer_filter: String,
    pub show_preferences: bool,
    pub show_shortcuts: bool,
    pub show_about: bool,
    pub show_welcome: bool,
    pub palette: Option<dialogs::PaletteState>,
    /// An action waiting for "save changes?" to be answered.
    pub confirm_discard: Option<Action>,
    /// Bring the Properties tab forward on the next frame.
    pub focus_properties: bool,
    /// Layer transform when a canvas drag started.
    pub drag_origin: Option<(LayerId, fanimate_core::Transform)>,
    /// The status message on screen, and when it appeared.
    pub status_since: Option<(String, Instant)>,
    /// Timeline zoom (pixels per frame) and horizontal scroll (frames).
    pub timeline_frame_width: f32,
    pub timeline_scroll: f32,
    /// The widget being edited in the Properties or Style panel, so one
    /// drag or one typed value makes one undo step.
    pub active_edit: Option<egui::Id>,
    /// Show (or reopen) a panel on the next frame.
    pub open_tab: Option<Tab>,
    pub reset_layout: bool,
    pub window: titlebar::WindowState,
}

/// Everything except the dock layout (kept apart so the dock and its tab
/// viewer can be borrowed separately).
pub struct Shell {
    pub editor: Editor,
    pub keymap: Keymap,
    pub theme: Theme,
    pub palette: Palette,
    pub canvas: Option<CanvasGpu>,
    /// The UI's GPU device, shared with the canvas renderer.
    pub render_state: Option<eframe::egui_wgpu::RenderState>,
    pub view: CanvasView,
    pub ui: UiState,
    /// Actions queued by widgets this frame, run after drawing.
    pub pending: Vec<Action>,
    pub gpu_name: String,
    pub bench: Option<Bench>,
    /// The app icon, for the title bar.
    pub icon: egui::TextureHandle,
    last_tick: Instant,
    allow_close: bool,
}

pub struct FanimateApp {
    dock: DockState<Tab>,
    shell: Shell,
}

pub struct Options {
    pub file: Option<PathBuf>,
    pub bench_bones: Option<usize>,
}

impl FanimateApp {
    pub fn new(cc: &eframe::CreationContext<'_>, opts: Options) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, SETTINGS_KEY))
            .unwrap_or_default();
        let dock = cc
            .storage
            .and_then(|s| eframe::get_value::<DockState<Tab>>(s, DOCK_KEY))
            .filter(|d| d.iter_all_tabs().count() == Tab::ALL.len())
            .unwrap_or_else(default_layout);
        let palette = theme::apply(&cc.egui_ctx, settings.theme);

        let mut status = None;
        let mut editor = match (&opts.file, opts.bench_bones) {
            (_, Some(bones)) => {
                let mut ed = Editor::new(fanimate_core::samples::benchmark(bones));
                ed.playing = true;
                ed
            }
            (Some(path), None) => match Editor::open(path) {
                Ok((ed, report)) => {
                    if !report.is_clean() {
                        status = Some((StatusKind::Warning, summarise(&report)));
                    }
                    ed
                }
                Err(e) => {
                    status = Some((StatusKind::Error, e.to_string()));
                    Editor::sample()
                }
            },
            (None, None) => Editor::sample(),
        };
        if let Some((kind, msg)) = status {
            editor.set_status(kind, msg);
        }

        let canvas = cc.wgpu_render_state.as_ref().map(CanvasGpu::new);
        let gpu_name = cc
            .wgpu_render_state
            .as_ref()
            .map(|rs| {
                let info = rs.adapter.get_info();
                format!("{} ({:?})", info.name, info.backend)
            })
            .unwrap_or_else(|| "no GPU".into());

        let icon = load_icon(&cc.egui_ctx);
        Self {
            dock,
            shell: Shell {
                icon,
                editor,
                keymap: Keymap::new(KeymapKind::from_id(&settings.keymap).unwrap_or_default()),
                theme: settings.theme,
                palette,
                canvas,
                render_state: cc.wgpu_render_state.clone(),
                view: CanvasView {
                    center: Default::default(),
                    zoom: 1.0,
                    fit_pending: true,
                },
                ui: UiState {
                    show_welcome: !settings.welcome_done && opts.bench_bones.is_none(),
                    ..Default::default()
                },
                pending: Vec::new(),
                gpu_name,
                bench: opts.bench_bones.map(|_| Bench {
                    started: Instant::now(),
                    frames: 0,
                    cpu: Duration::ZERO,
                }),
                last_tick: Instant::now(),
                allow_close: false,
            },
        }
    }
}

fn load_icon(ctx: &egui::Context) -> egui::TextureHandle {
    let png = include_bytes!("../../../brand/app-icon/fanimate-64.png");
    let img = image::load_from_memory(png)
        .map(|i| i.into_rgba8())
        .unwrap_or_default();
    let (w, h) = img.dimensions();
    let color = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    ctx.load_texture("fanimate-icon", color, egui::TextureOptions::LINEAR)
}

pub fn default_layout() -> DockState<Tab> {
    // Moho Pro's arrangement: tools left of the canvas, timeline along the
    // bottom, inspector tabs above the Layers panel on the right.
    let mut dock = DockState::new(vec![Tab::Canvas]);
    let s = dock.main_surface_mut();
    let [left, right] = s.split_right(
        NodeIndex::root(),
        0.74,
        vec![Tab::Properties, Tab::Style, Tab::Actions],
    );
    s.split_below(right, 0.42, vec![Tab::Layers]);
    let [canvas, _timeline] = s.split_below(left, 0.70, vec![Tab::Timeline]);
    s.split_left(canvas, 0.09, vec![Tab::Tools]);
    dock
}

fn summarise(report: &fanimate_fnm::Report) -> String {
    let n = report.issues.len();
    let fixed = report.repaired_count();
    format!("Opened with {n} issue(s); {fixed} repaired automatically. Save to keep the repairs.")
}

impl eframe::App for FanimateApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let shell = &mut self.shell;

        // playback clock
        let now = Instant::now();
        let dt = (now - shell.last_tick).as_secs_f32().min(0.25);
        shell.last_tick = now;
        shell.editor.tick(dt);

        handle_close_request(&ctx, shell);
        titlebar::track_window(&ctx, &mut shell.ui.window);
        handle_input(&ctx, shell);

        titlebar::show(ui, shell);
        panels::status_bar(ui, shell);

        if shell.ui.focus_properties {
            if let Some(path) = self.dock.find_tab(&Tab::Properties) {
                let _ = self.dock.set_active_tab(path);
            }
            shell.ui.focus_properties = false;
        }

        let style = dock_style(ui.style(), &shell.palette);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(shell.palette.bg))
            .show(ui, |ui| {
                DockArea::new(&mut self.dock)
                    .style(style)
                    .show_close_buttons(true)
                    .show_add_buttons(false)
                    .show_leaf_collapse_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_inside(ui, &mut TabViewer { shell });
            });

        dialogs::show(&ctx, shell);
        titlebar::resize_edges(&ctx, &shell.ui.window);
        native_cursors(&ctx);

        // run queued actions now that no widget borrows the editor
        for action in std::mem::take(&mut shell.pending) {
            dispatch(&ctx, shell, action);
        }
        if let Some(tab) = shell.ui.open_tab.take() {
            match self.dock.find_tab(&tab) {
                Some(path) => {
                    let _ = self.dock.set_active_tab(path);
                }
                None => self.dock.push_to_focused_leaf(tab),
            }
        }
        if std::mem::take(&mut shell.ui.reset_layout) {
            self.dock = default_layout();
        }

        if let Some(b) = &mut shell.bench {
            b.frames += 1;
            if let Some(c) = &shell.canvas {
                b.cpu += c.cpu_time;
            }
            if b.started.elapsed() >= BENCH_DURATION {
                let secs = b.started.elapsed().as_secs_f64();
                crate::report(&format!(
                    "benchmark: {} ({} shapes) on {}\n  {:.1} fps average over {:.1} s (UI included)\n  {:.3} ms CPU per frame (evaluate + tessellate)",
                    shell.editor.doc.name,
                    shell.canvas.as_ref().map_or(0, |c| c.shapes),
                    shell.gpu_name,
                    b.frames as f64 / secs,
                    secs,
                    b.cpu.as_secs_f64() * 1000.0 / b.frames.max(1) as f64,
                ));
                shell.allow_close = true;
                shell.bench = None; // report once; a frame may still run before closing
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }

        if shell.editor.playing || shell.bench.is_some() {
            ctx.request_repaint();
        }
        let title = format!("{} - Fanimate", window_title(&shell.editor));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let s = &self.shell;
        eframe::set_value(
            storage,
            SETTINGS_KEY,
            &Settings {
                keymap: s.keymap.kind.id().into(),
                theme: s.theme,
                welcome_done: !s.ui.show_welcome,
            },
        );
        eframe::set_value(storage, DOCK_KEY, &self.dock);
    }
}

/// Dock colours from the theme tokens rather than egui_dock's defaults.
fn dock_style(egui_style: &egui::Style, p: &Palette) -> egui_dock::Style {
    let mut style = egui_dock::Style::from_egui(egui_style);
    style.main_surface_border_stroke = egui::Stroke::NONE;
    style.tab_bar.bg_fill = p.bg;
    style.tab_bar.height = 26.0;
    style.tab_bar.hline_color = p.border;
    style.tab.tab_body.bg_fill = p.panel;
    style.tab.tab_body.stroke = egui::Stroke::new(1.0, p.border);
    for (state, bg, text) in [
        (&mut style.tab.active, p.panel, p.text),
        (&mut style.tab.focused, p.panel, p.text),
        (&mut style.tab.inactive, p.bg, p.text_muted),
        (&mut style.tab.inactive_with_kb_focus, p.bg, p.text),
        (&mut style.tab.hovered, p.raised, p.text),
    ] {
        state.bg_fill = bg;
        state.text_color = text;
        state.outline_color = p.border;
    }
    style.separator.color_idle = p.bg;
    style.separator.color_hovered = p.accent;
    style
}

pub fn window_title(ed: &Editor) -> String {
    format!("{}{}", ed.doc.name, if ed.dirty { " •" } else { "" })
}

/// Intercept closing the window when there are unsaved changes.
fn handle_close_request(ctx: &egui::Context, shell: &mut Shell) {
    if ctx.input(|i| i.viewport().close_requested()) && !shell.allow_close && shell.editor.dirty {
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        shell.ui.confirm_discard = Some(Action::Quit);
    }
}

/// egui and egui_dock show a grab hand over anything draggable, such as
/// dock tabs and layer rows. Desktop apps keep the normal arrow there, and
/// Windows has no hand cursor at all: winit substitutes the four-way move
/// arrow, which reads as "this moves the window".
fn native_cursors(ctx: &egui::Context) {
    ctx.output_mut(|o| {
        let grabbing_shows_move = cfg!(windows) && o.cursor_icon == CursorIcon::Grabbing;
        if o.cursor_icon == CursorIcon::Grab || grabbing_shows_move {
            o.cursor_icon = CursorIcon::Default;
        }
    });
}

fn any_modal_open(shell: &Shell) -> bool {
    let u = &shell.ui;
    u.palette.is_some() || u.confirm_discard.is_some() || u.show_welcome
}

/// Keyboard shortcuts and clipboard events.
fn handle_input(ctx: &egui::Context, shell: &mut Shell) {
    // a text field has focus, or a dialog or menu is up: keys belong to it
    if ctx.egui_wants_keyboard_input() || any_modal_open(shell) || egui::Popup::is_any_open(ctx) {
        return;
    }
    let events = ctx.input(|i| i.events.clone());
    for event in events {
        match event {
            // egui turns Ctrl+C/X/V into these instead of key events
            egui::Event::Copy => shell.pending.push(Action::Copy),
            egui::Event::Cut => shell.pending.push(Action::Cut),
            egui::Event::Paste(text) if !shell.editor.paste(Some(&text)) => {
                shell.editor.set_status(StatusKind::Info, "Nothing to paste");
            }
            egui::Event::Key {
                key,
                pressed: true,
                repeat,
                modifiers,
                ..
            } => {
                let chord = chord_of(key, modifiers);
                if let Some(action) = shell.keymap.lookup(chord) {
                    // only frame stepping repeats while a key is held
                    let repeats = matches!(action, Action::NextFrame | Action::PrevFrame);
                    if !repeat || repeats {
                        shell.pending.push(action);
                    }
                }
            }
            _ => {}
        }
    }
}

fn chord_of(key: Key, m: Modifiers) -> Chord {
    Chord::new(key.name(), m.command, m.shift, m.alt)
}

/// Run an action from any source: menu, shortcut, context menu, palette.
pub fn dispatch(ctx: &egui::Context, shell: &mut Shell, action: Action) {
    // actions that would throw away unsaved work ask first
    if matches!(action, Action::New | Action::Open | Action::Quit) && shell.editor.dirty {
        shell.ui.confirm_discard = Some(action);
        return;
    }
    run_unchecked(ctx, shell, action);
}

/// Run an action without the unsaved-changes check (already answered).
pub fn run_unchecked(ctx: &egui::Context, shell: &mut Shell, action: Action) {
    match shell.editor.run(action) {
        Outcome::Done => {}
        Outcome::CopyText(text) => ctx.copy_text(text),
        Outcome::App(a) => match a {
            Action::New => {
                shell.editor = Editor::untitled();
                shell.view.fit();
            }
            Action::Open => open_dialog(shell),
            Action::Save => {
                save(shell, false);
            }
            Action::SaveAs => {
                save(shell, true);
            }
            Action::Quit => {
                shell.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Action::Preferences => shell.ui.show_preferences = true,
            Action::CommandPalette => shell.ui.palette = Some(Default::default()),
            Action::LayerProperties => shell.ui.focus_properties = true,
            Action::ZoomIn => zoom_by(shell, 1.25),
            Action::ZoomOut => zoom_by(shell, 0.8),
            Action::FitView => shell.view.fit(),
            Action::ActualSize => {
                shell.view.zoom = 1.0 / ctx.pixels_per_point();
                shell.view.center = shell.editor.doc.size() / 2.0;
            }
            other => log::warn!("unhandled app action {other:?}"),
        },
    }
}

fn zoom_by(shell: &mut Shell, factor: f32) {
    shell.view.zoom = (shell.view.zoom * factor).clamp(0.02, 64.0);
}

fn open_dialog(shell: &mut Shell) {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Open a Fanimate project (.fnm folder)")
        .pick_folder()
    else {
        return;
    };
    match Editor::open(&path) {
        Ok((ed, report)) => {
            shell.editor = ed;
            shell.view.fit();
            if report.is_clean() {
                shell
                    .editor
                    .set_status(StatusKind::Info, format!("Opened {}", path.display()));
            } else {
                shell.editor.set_status(StatusKind::Warning, summarise(&report));
            }
        }
        Err(e) => shell.editor.set_status(StatusKind::Error, e.to_string()),
    }
}

/// Save, asking for a location if needed. Returns whether it saved.
pub fn save(shell: &mut Shell, ask: bool) -> bool {
    let path = match (&shell.editor.path, ask) {
        (Some(p), false) => p.clone(),
        _ => {
            let Some(mut p) = rfd::FileDialog::new()
                .set_title("Save Fanimate project")
                .set_file_name(format!("{}.fnm", shell.editor.doc.name))
                .add_filter("Fanimate project", &["fnm"])
                .save_file()
            else {
                return false;
            };
            if p.extension().is_none_or(|e| e != "fnm") {
                p.set_extension("fnm");
            }
            p
        }
    };
    match shell.editor.save_to(&path) {
        Ok(()) => true,
        Err(e) => {
            shell
                .editor
                .set_status(StatusKind::Error, format!("Could not save: {e}"));
            false
        }
    }
}

impl Shell {
    pub fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.theme = theme;
        self.palette = theme::apply(ctx, theme);
    }

    pub fn set_keymap(&mut self, kind: KeymapKind) {
        self.keymap = Keymap::new(kind);
        self.editor
            .set_status(StatusKind::Info, format!("Keyboard shortcuts: {}", kind.label()));
    }
}
