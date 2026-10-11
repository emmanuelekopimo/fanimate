//! A custom title bar that holds the menus, like modern editors do, so no
//! vertical space is spent on a separate menu row.
//!
//! On Windows and Linux the window has no system decorations: this bar
//! draws the minimise, maximise and close buttons, drags the window, and
//! [`resize_edges`] makes the window edges resizable. On macOS the native
//! traffic-light buttons stay, inside a full-size content view.

use eframe::egui::{
    self, Align, Color32, CursorIcon, Id, Layout, Rect, ResizeDirection, Sense, Stroke, ViewportCommand,
};

use crate::app::{Shell, window_title};
use crate::menus;

pub const HEIGHT: f32 = 34.0;
const BUTTON_WIDTH: f32 = 46.0;
const RESIZE_MARGIN: f32 = 5.0;

pub fn uses_custom_frame() -> bool {
    !cfg!(target_os = "macos")
}

pub fn show(ui: &mut egui::Ui, shell: &mut Shell) {
    let bg = shell.palette.bg;
    egui::Panel::top("titlebar")
        .exact_size(HEIGHT)
        .frame(egui::Frame::NONE.fill(bg))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            // background first, so the menus drawn on top get their clicks
            let drag = ui.interact(rect, Id::new("titlebar-drag"), Sense::click_and_drag());
            if drag.drag_started_by(egui::PointerButton::Primary) {
                ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if drag.double_clicked() {
                toggle_maximized(ui.ctx());
            }

            // centred document title
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                window_title(&shell.editor),
                egui::FontId::proportional(13.0),
                shell.palette.text_muted,
            );

            ui.horizontal_centered(|ui| {
                if cfg!(target_os = "macos") {
                    ui.add_space(72.0); // traffic lights
                } else {
                    ui.add_space(10.0);
                }
                ui.add(egui::Image::new((shell.icon.id(), egui::vec2(18.0, 18.0))));
                ui.add_space(4.0);
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| menus::file(ui, shell));
                    ui.menu_button("Edit", |ui| menus::edit(ui, shell));
                    ui.menu_button("Layer", |ui| menus::layer(ui, shell));
                    ui.menu_button("Animation", |ui| menus::animation(ui, shell));
                    ui.menu_button("View", |ui| menus::view(ui, shell));
                    ui.menu_button("Window", |ui| menus::window(ui, shell));
                    ui.menu_button("Help", |ui| menus::help(ui, shell));
                });

                if uses_custom_frame() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        window_buttons(ui, shell);
                    });
                }
            });
        });
}

fn toggle_maximized(ctx: &egui::Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
}

#[derive(Clone, Copy)]
enum Caption {
    Minimize,
    Maximize,
    Restore,
    Close,
}

fn window_buttons(ui: &mut egui::Ui, shell: &Shell) {
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    let p = shell.palette;
    if caption_button(
        ui,
        Caption::Close,
        p.text,
        Some(Color32::from_rgb(0xC4, 0x2B, 0x1C)),
        p.border,
    )
    .clicked()
    {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    }
    let max = if maximized {
        Caption::Restore
    } else {
        Caption::Maximize
    };
    if caption_button(ui, max, p.text, None, p.border).clicked() {
        toggle_maximized(ui.ctx());
    }
    if caption_button(ui, Caption::Minimize, p.text, None, p.border).clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
}

/// A Windows-style caption button with a drawn glyph (no font needed).
fn caption_button(
    ui: &mut egui::Ui,
    kind: Caption,
    fg: Color32,
    hover_bg: Option<Color32>,
    default_hover: Color32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(BUTTON_WIDTH, HEIGHT), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter()
            .rect_filled(rect, 0.0, hover_bg.unwrap_or(default_hover));
    }
    let fg = if hovered && hover_bg.is_some() {
        Color32::WHITE
    } else {
        fg
    };
    let stroke = Stroke::new(1.0, fg);
    let c = rect.center();
    let s = 5.0;
    let painter = ui.painter();
    match kind {
        Caption::Minimize => {
            painter.line_segment([c + egui::vec2(-s, 0.0), c + egui::vec2(s, 0.0)], stroke);
        }
        Caption::Maximize => {
            painter.rect_stroke(
                Rect::from_center_size(c, egui::vec2(2.0 * s, 2.0 * s)),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Caption::Restore => {
            let back = Rect::from_center_size(c + egui::vec2(2.0, -2.0), egui::vec2(1.6 * s, 1.6 * s));
            let front = Rect::from_center_size(c + egui::vec2(-1.0, 1.0), egui::vec2(1.6 * s, 1.6 * s));
            painter.rect_stroke(back, 1.0, stroke, egui::StrokeKind::Inside);
            painter.rect_filled(front, 1.0, ui.visuals().panel_fill);
            painter.rect_stroke(front, 1.0, stroke, egui::StrokeKind::Inside);
        }
        Caption::Close => {
            painter.line_segment([c + egui::vec2(-s, -s), c + egui::vec2(s, s)], stroke);
            painter.line_segment([c + egui::vec2(-s, s), c + egui::vec2(s, -s)], stroke);
        }
    }
    response
}

/// Make the edges of an undecorated window resizable.
pub fn resize_edges(ctx: &egui::Context) {
    if !uses_custom_frame() {
        return;
    }
    let (maximized, fullscreen) = ctx.input(|i| {
        (
            i.viewport().maximized.unwrap_or(false),
            i.viewport().fullscreen.unwrap_or(false),
        )
    });
    if maximized || fullscreen {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let r = ctx.content_rect();
    let m = RESIZE_MARGIN;
    let (l, rt, t, b) = (
        pos.x <= r.left() + m,
        pos.x >= r.right() - m,
        pos.y <= r.top() + m,
        pos.y >= r.bottom() - m,
    );
    let dir = match (l, rt, t, b) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (.., true) => ResizeDirection::South,
        _ => return,
    };
    ctx.set_cursor_icon(match dir {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
    });
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}
