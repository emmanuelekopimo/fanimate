//! The app: window chrome, docking layout, dialogs and command dispatch.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, CursorIcon, Key, Modifiers};
use egui_dock::{DockArea, DockState, NodeIndex};
use fanimate_core::LayerId;
use fanimate_editor::{Action, Chord, DrawStyle, Editor, Keymap, KeymapKind, Outcome, StatusKind};
use serde::{Deserialize, Serialize};

use crate::canvas_gpu::CanvasGpu;
use crate::panels::{self, Tab, TabViewer};
use crate::recovery::{self, Recovery};
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
    draw: DrawStyle,
    onion: OnionSkin,
    /// The format Export Video chose last.
    export_format: fanimate_export::Format,
    /// After a crash, offer to report it.
    crash_reports: bool,
    updates: crate::update::UpdateSettings,
}

/// Onion skin settings, remembered between sessions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OnionSkin {
    pub on: bool,
    /// How many earlier and later frames or drawings show.
    pub before: usize,
    pub after: usize,
    /// Only frames with drawings or keys, rather than every frame.
    pub keys: bool,
    /// Every layer, rather than the selected ones.
    pub all_layers: bool,
}

impl Default for OnionSkin {
    fn default() -> Self {
        Self {
            on: false,
            before: 2,
            after: 1,
            keys: true,
            all_layers: false,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            keymap: KeymapKind::Fanimate.id().into(),
            theme: Theme::default(),
            welcome_done: false,
            draw: DrawStyle::default(),
            onion: OnionSkin::default(),
            export_format: Default::default(),
            crash_reports: true,
            updates: Default::default(),
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
    /// Where a Rectangle or Ellipse drag started, in document space.
    pub shape_drag: Option<fanimate_core::Vec2>,
    /// A point tool drag in progress.
    pub point_gesture: Option<crate::panels::canvas::PointGesture>,
    /// Whether the pointer went down with the Pen on the canvas.
    pub pen_down: bool,
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
    /// The Export Video window, while open.
    pub export: Option<crate::export::ExportForm>,
    /// The Update Fanimate window.
    pub show_update: bool,
    /// Arrange the panels this way on the next frame.
    pub workspace: Option<fanimate_editor::Workspace>,
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
    /// The crash recovery journal, and work recovered from a crash.
    pub recovery: Recovery,
    pub onion: OnionSkin,
    /// Fade every layer but the selected ones.
    pub light_table: bool,
    /// Plays audio layers along with playback.
    pub audio: crate::audio::Audio,
    /// A lip sync waiting on Rhubarb.
    pub lipsync: Option<crate::lipsync::Job>,
    pub export_format: fanimate_export::Format,
    /// Renders queued from here, for the status bar.
    pub renders: crate::export::Renders,
    /// A crash from an earlier session, waiting to be reported or not.
    pub crash: Option<crate::crash::Pending>,
    /// After a crash, offer to report it (Preferences).
    pub crash_reports: bool,
    /// This session's marker; a normal exit removes it.
    _crash_session: Option<crate::crash::Session>,
    pub updater: crate::update::Updater,
    /// How this copy was installed, which decides how it updates.
    pub install: crate::update::Install,
    pub updates: crate::update::UpdateSettings,
    /// Commands recently run from the palette, most recent first.
    pub recent_actions: Vec<Action>,
    /// The user asked for this check, so its result is shown.
    pub update_manual: bool,
    /// When the automatic check is due.
    update_due: Option<Instant>,
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
            (Some(path), None) if fanimate_import::can_import(path) => match Editor::import_project(path) {
                Ok(ed) => ed,
                Err(e) => {
                    status = Some((StatusKind::Error, e));
                    Editor::sample()
                }
            },
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
        editor.draw = settings.draw;

        let canvas = cc.wgpu_render_state.as_ref().map(CanvasGpu::new);
        let gpu_name = cc
            .wgpu_render_state
            .as_ref()
            .map(|rs| {
                let info = rs.adapter.get_info();
                format!("{} ({:?})", info.name, info.backend)
            })
            .unwrap_or_else(|| "no GPU".into());

        crate::crash::set_gpu(&gpu_name);
        let bench = opts.bench_bones.is_some();
        let (crash_session, mut crash) = match crate::crash::dir() {
            Some(dir) if !bench => crate::crash::begin(&dir),
            _ => (None, None),
        };
        if !settings.crash_reports
            && let Some(p) = crash.take()
        {
            crate::crash::dismiss(&p);
        }

        let icon = load_icon(&cc.egui_ctx);
        // the benchmark leaves no trace
        let recovery_root = if opts.bench_bones.is_some() {
            None
        } else {
            recovery::root()
        };
        let mut app = Self {
            dock,
            shell: Shell {
                icon,
                recovery: Recovery::new(recovery_root),
                onion: settings.onion,
                light_table: false,
                audio: Default::default(),
                lipsync: None,
                export_format: settings.export_format,
                renders: Default::default(),
                crash,
                crash_reports: settings.crash_reports,
                _crash_session: crash_session,
                updater: Default::default(),
                install: crate::update::Install::detect(),
                updates: settings.updates.clone(),
                recent_actions: Vec::new(),
                update_manual: false,
                update_due: (!bench).then(|| Instant::now() + Duration::from_secs(5)),
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
        };
        recovery::start(&mut app.shell);
        app
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
        vec![Tab::Properties, Tab::Style, Tab::Actions, Tab::History],
    );
    s.split_below(right, 0.42, vec![Tab::Layers]);
    let [canvas, _timeline] = s.split_below(left, 0.70, vec![Tab::Timeline, Tab::XSheet]);
    s.split_left(canvas, 0.09, vec![Tab::Tools]);
    dock
}

/// The panel arrangement for a kind of work.
pub fn workspace_layout(w: fanimate_editor::Workspace) -> DockState<Tab> {
    use fanimate_editor::Workspace as W;
    if w == W::Animate {
        return default_layout();
    }
    let mut dock = DockState::new(vec![Tab::Canvas]);
    let s = dock.main_surface_mut();
    match w {
        W::Animate => {}
        W::Draw => {
            let [left, right] = s.split_right(
                NodeIndex::root(),
                0.78,
                vec![Tab::Style, Tab::Properties, Tab::Actions, Tab::History],
            );
            s.split_below(right, 0.55, vec![Tab::Layers]);
            let [canvas, _] = s.split_below(left, 0.84, vec![Tab::Timeline, Tab::XSheet]);
            s.split_left(canvas, 0.08, vec![Tab::Tools]);
        }
        W::Rig => {
            let [left, right] = s.split_right(
                NodeIndex::root(),
                0.72,
                vec![Tab::Properties, Tab::Actions, Tab::Style],
            );
            s.split_below(right, 0.5, vec![Tab::Layers, Tab::History]);
            let [canvas, _] = s.split_below(left, 0.72, vec![Tab::Timeline, Tab::XSheet]);
            s.split_left(canvas, 0.09, vec![Tab::Tools]);
        }
        W::FrameByFrame => {
            let [left, right] = s.split_right(NodeIndex::root(), 0.70, vec![Tab::XSheet]);
            s.split_below(
                right,
                0.6,
                vec![
                    Tab::Layers,
                    Tab::Style,
                    Tab::Properties,
                    Tab::Actions,
                    Tab::History,
                ],
            );
            let [canvas, _] = s.split_below(left, 0.70, vec![Tab::Timeline]);
            s.split_left(canvas, 0.09, vec![Tab::Tools]);
        }
        // only the canvas and timeline: the next start opens in Animate
        W::Review => {
            s.split_below(NodeIndex::root(), 0.78, vec![Tab::Timeline]);
        }
    }
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
        shell
            .audio
            .follow(&shell.editor.doc, shell.editor.playing, shell.editor.frame);
        crate::lipsync::poll(&ctx, shell);

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
        crate::export::window(&ctx, shell);
        {
            let Shell { renders, editor, .. } = shell;
            renders.poll(&ctx, &mut |kind, message| editor.set_status(kind, message));
        }
        titlebar::resize_edges(&ctx, &shell.ui.window);
        native_cursors(&ctx);

        // run queued actions now that no widget borrows the editor
        for action in std::mem::take(&mut shell.pending) {
            dispatch(&ctx, shell, action);
        }
        recovery::tick(&ctx, shell);
        update_tick(&ctx, shell);
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
        if let Some(w) = shell.ui.workspace.take() {
            self.dock = workspace_layout(w);
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
                draw: s.editor.draw,
                onion: s.onion,
                export_format: s.export_format,
                crash_reports: s.crash_reports,
                updates: s.updates.clone(),
            },
        );
        eframe::set_value(storage, DOCK_KEY, &self.dock);
    }

    fn on_exit(&mut self) {
        recovery::end(&mut self.shell);
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
    u.palette.is_some() || u.confirm_discard.is_some() || u.show_welcome || !shell.recovery.offered.is_empty()
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
    if action.closes_document() && shell.editor.dirty {
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
                shell.replace_editor(Editor::untitled());
            }
            Action::Open => open_dialog(shell),
            Action::OpenOther => open_other_dialog(shell),
            Action::OpenSample(sample) => shell.replace_editor(Editor::new(sample.document())),
            Action::Save => {
                save(shell, false);
            }
            Action::SaveAs => {
                save(shell, true);
            }
            Action::Import => import_dialog(shell),
            Action::LipSync => crate::lipsync::start(shell),
            Action::ExportVideo => {
                shell.ui.export = Some(crate::export::ExportForm::new(shell, shell.export_format));
            }
            Action::RenderQueue => {
                if let Err(e) = crate::render_queue::open(shell.theme) {
                    shell.editor.set_status(StatusKind::Error, e);
                }
            }
            Action::ToggleOnionSkin => shell.onion.on = !shell.onion.on,
            Action::ToggleLightTable => shell.light_table = !shell.light_table,
            Action::Quit => {
                shell.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Action::Preferences => shell.ui.show_preferences = true,
            Action::CheckForUpdates => {
                shell.update_manual = true;
                shell.ui.show_update = true;
                shell.updater.check(true, &shell.install);
            }
            Action::InstallUpdate => install_update(ctx, shell),
            Action::Workspace(w) => shell.ui.workspace = Some(w),
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

/// Each frame: start the automatic update check when due, and report
/// what a check found.
fn update_tick(ctx: &egui::Context, shell: &mut Shell) {
    use crate::update::State;
    if let Some(due) = shell.update_due
        && Instant::now() >= due
    {
        shell.update_due = None;
        if shell.updates.check {
            shell.updater.check(false, &shell.install);
        }
    }
    match shell.updater.state() {
        State::Checking { .. } | State::Downloading(_) => {
            ctx.request_repaint_after(Duration::from_millis(300))
        }
        State::UpToDate => {
            if std::mem::take(&mut shell.update_manual) {
                let v = crate::update::Version::current();
                shell
                    .editor
                    .set_status(StatusKind::Success, format!("Fanimate {v} is the newest version"));
                shell.ui.show_update = false;
            }
            shell.updater.reset();
        }
        State::Failed(e) => {
            // an automatic check fails quietly: offline is normal
            if std::mem::take(&mut shell.update_manual) || shell.ui.show_update {
                shell.editor.set_status(StatusKind::Error, e);
            }
            shell.ui.show_update = false;
            shell.updater.reset();
        }
        State::Available(_) => shell.update_manual = false,
        State::Idle | State::Ready(..) => {}
    }
}

/// Install a downloaded update: wait for renders, then hand over to the
/// installer and quit. Unsaved changes were already asked about.
fn install_update(ctx: &egui::Context, shell: &mut Shell) {
    let crate::update::State::Ready(release, file) = shell.updater.state() else {
        shell.editor.set_status(
            StatusKind::Info,
            "No update is ready yet. Use Help > Check for Updates.",
        );
        return;
    };
    // jobs waiting with no queue window open resume in the new version
    let rendering = crate::render_queue::queue()
        .is_some_and(|q| q.has_runner() && q.jobs().iter().any(|e| !e.status.state.is_finished()));
    if rendering {
        shell.editor.set_status(
            StatusKind::Warning,
            "The render queue is still working. Install the update when it has finished.",
        );
        return;
    }
    match crate::update::install(&shell.install, &file) {
        Ok(()) => {
            log::info!("installing Fanimate {}", release.version);
            shell.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        Err(e) => shell
            .editor
            .set_status(StatusKind::Error, format!("Could not install the update: {e}")),
    }
}

fn zoom_by(shell: &mut Shell, factor: f32) {
    shell.view.zoom = (shell.view.zoom * factor).clamp(0.02, 64.0);
}

/// Pick pictures, Photoshop or SVG files, or sounds, and import them as
/// layers.
fn import_dialog(shell: &mut Shell) {
    let Some(paths) = rfd::FileDialog::new()
        .set_title("Import pictures, a Photoshop or SVG file, or sound")
        .add_filter(
            "Pictures, Photoshop and SVG files, and sound",
            &["png", "jpg", "jpeg", "psd", "svg", "wav"],
        )
        .pick_files()
    else {
        return;
    };
    for path in paths {
        let name = path.display().to_string();
        let result = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            match ext.as_str() {
                "psd" => shell.editor.import_psd(&name, &bytes),
                "svg" => shell.editor.import_svg(&name, &bytes),
                "wav" => shell.editor.import_audio(&name, bytes),
                _ => shell.editor.import_picture(&name, bytes),
            }
        });
        if let Err(e) = result {
            let file = path
                .file_name()
                .map_or(name.clone(), |f| f.to_string_lossy().into_owned());
            shell
                .editor
                .set_status(StatusKind::Error, format!("Could not import {file}: {e}"));
            return;
        }
    }
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
            shell.replace_editor(ed);
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

/// Pick a Moho or Animate project and open it as a new Fanimate project.
fn open_other_dialog(shell: &mut Shell) {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Open a Moho or Animate project")
        .add_filter("Moho and Animate projects", &fanimate_import::EXTENSIONS)
        .pick_file()
    else {
        return;
    };
    match Editor::import_project(&path) {
        Ok(ed) => shell.replace_editor(ed),
        Err(e) => {
            let file = path.file_name().map_or_else(
                || path.display().to_string(),
                |f| f.to_string_lossy().into_owned(),
            );
            shell
                .editor
                .set_status(StatusKind::Error, format!("Could not open {file}: {e}"));
        }
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
        Ok(()) => {
            // saved: a crash now would lose nothing
            recovery::record(shell);
            true
        }
        Err(e) => {
            shell
                .editor
                .set_status(StatusKind::Error, format!("Could not save: {e}"));
            false
        }
    }
}

impl Shell {
    /// Switch to another document, keeping the brush and colours.
    pub fn replace_editor(&mut self, mut editor: Editor) {
        editor.draw = self.editor.draw;
        self.editor = editor;
        self.view.fit();
        recovery::start(self);
    }

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
