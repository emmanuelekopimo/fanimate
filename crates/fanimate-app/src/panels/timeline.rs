//! The Timeline panel: transport controls, a scrubbable ruler, and a row of
//! keyframes per layer.

use eframe::egui::{self, Id, Rect, RichText, Sense, Shape, Stroke, pos2, vec2};
use fanimate_core::{Frame, LayerId};
use fanimate_editor::{Action, SelectMode};

use crate::app::Shell;
use crate::menus;

const NAMES_WIDTH: f32 = 170.0;
const RULER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 22.0;
const DEFAULT_FRAME_WIDTH: f32 = 12.0;

pub fn show(ui: &mut egui::Ui, shell: &mut Shell) {
    transport(ui, shell);
    ui.add_space(2.0);

    let full = ui.available_rect_before_wrap();
    ui.allocate_rect(full, Sense::hover());
    if shell.ui.timeline_frame_width <= 0.0 {
        shell.ui.timeline_frame_width = DEFAULT_FRAME_WIDTH;
    }
    let p = shell.palette;
    let (start, end) = (shell.editor.doc.start, shell.editor.doc.end);
    let fps = shell.editor.doc.fps.max(1.0);

    let frames_rect = Rect::from_min_max(pos2(full.left() + NAMES_WIDTH, full.top()), full.max);
    let ruler = Rect::from_min_max(
        frames_rect.min,
        pos2(frames_rect.right(), frames_rect.top() + RULER_HEIGHT),
    );

    zoom_and_scroll(ui, shell, frames_rect, start, end);
    let fw = shell.ui.timeline_frame_width;
    let scroll = shell.ui.timeline_scroll;
    let x_of = |f: f32| ruler.left() + (f - start as f32 - scroll) * fw + fw / 2.0;
    let frame_at = |x: f32| -> Frame { (start as f32 + scroll + (x - ruler.left()) / fw).floor() as Frame };

    // keep the playhead in view while playing
    if shell.editor.playing {
        let x = x_of(shell.editor.frame);
        if x > frames_rect.right() - 20.0 || x < frames_rect.left() {
            shell.ui.timeline_scroll = (shell.editor.frame - start as f32 - 2.0).max(0.0);
        }
    }

    // ruler: click or drag to scrub
    let painter = ui.painter_at(full);
    painter.rect_filled(ruler, 0.0, p.bg);
    let ruler_resp = ui.interact(ruler, Id::new("timeline-ruler"), Sense::click_and_drag());
    if (ruler_resp.clicked() || ruler_resp.dragged())
        && let Some(pos) = ruler_resp.interact_pointer_pos()
    {
        shell.editor.go_to(frame_at(pos.x));
    }
    let label_every = nice_step(48.0 / fw);
    let first = frame_at(ruler.left()).max(start);
    let last = frame_at(ruler.right()).min(end);
    for f in first..=last {
        let x = x_of(f as f32);
        let second = (f - start) as f32 % fps == 0.0;
        let h = if second {
            10.0
        } else if f % label_every == 0 {
            6.0
        } else {
            3.0
        };
        painter.line_segment(
            [pos2(x, ruler.bottom() - h), pos2(x, ruler.bottom())],
            Stroke::new(1.0, p.text_muted),
        );
        if f % label_every == 0 {
            painter.text(
                pos2(x + 2.0, ruler.top() + 2.0),
                egui::Align2::LEFT_TOP,
                f.to_string(),
                egui::FontId::proportional(10.0),
                p.text_muted,
            );
        }
    }

    // rows
    let rows_rect = Rect::from_min_max(pos2(full.left(), ruler.bottom()), full.max);
    let keys_clip = Rect::from_min_max(pos2(frames_rect.left(), rows_rect.top()), rows_rect.max);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rows_rect));
    egui::ScrollArea::vertical()
        .id_salt("timeline-rows")
        .auto_shrink([false, false])
        .show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (id, depth) in shell.editor.display_rows() {
                row(ui, shell, id, depth, keys_clip, &x_of, &frame_at);
            }
        });

    // frame range end marker and playhead on top
    let painter = ui.painter_at(Rect::from_min_max(frames_rect.min, full.max));
    let end_x = x_of(end as f32) + fw / 2.0;
    painter.line_segment(
        [pos2(end_x, ruler.top()), pos2(end_x, full.bottom())],
        Stroke::new(1.0, p.border),
    );
    let x = x_of(shell.editor.frame.floor());
    painter.line_segment(
        [pos2(x, ruler.top()), pos2(x, full.bottom())],
        Stroke::new(1.5, p.keyframe),
    );
    painter.add(Shape::convex_polygon(
        vec![
            pos2(x - 5.0, ruler.top()),
            pos2(x + 5.0, ruler.top()),
            pos2(x, ruler.top() + 7.0),
        ],
        p.keyframe,
        Stroke::NONE,
    ));
}

/// Label spacing in frames that is at least `min` and a "round" number.
fn nice_step(min: f32) -> Frame {
    [1, 2, 4, 6, 12, 24, 48, 96, 240, 480, 960]
        .into_iter()
        .find(|s| *s as f32 >= min)
        .unwrap_or(1920)
}

fn zoom_and_scroll(ui: &egui::Ui, shell: &mut Shell, area: Rect, start: Frame, end: Frame) {
    let hovered = ui.rect_contains_pointer(area);
    if !hovered {
        return;
    }
    let (zoom, scroll, mods) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.modifiers));
    if zoom != 1.0 {
        shell.ui.timeline_frame_width = (shell.ui.timeline_frame_width * zoom).clamp(2.0, 60.0);
    } else if scroll.x != 0.0 || (mods.shift && scroll.y != 0.0) {
        let dx = if scroll.x != 0.0 { scroll.x } else { scroll.y };
        let max = (end - start) as f32;
        shell.ui.timeline_scroll =
            (shell.ui.timeline_scroll - dx / shell.ui.timeline_frame_width).clamp(0.0, max);
    }
}

#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut egui::Ui,
    shell: &mut Shell,
    id: LayerId,
    depth: usize,
    keys_clip: Rect,
    x_of: &impl Fn(f32) -> f32,
    frame_at: &impl Fn(f32) -> Frame,
) {
    let Some(layer) = shell.editor.doc.find(id) else {
        return;
    };
    let name = layer.name.clone();
    let p = shell.palette;
    let selected = shell.editor.is_selected(id);

    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    let names = Rect::from_min_max(rect.min, pos2(rect.left() + NAMES_WIDTH, rect.bottom()));
    let keys = Rect::from_min_max(pos2(names.right(), rect.top()), rect.max);

    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, 0.0, p.accent.gamma_multiply(0.2));
    }
    painter.hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, p.border.gamma_multiply(0.5)),
    );
    painter.text(
        pos2(names.left() + 8.0 + depth as f32 * 12.0, names.center().y),
        egui::Align2::LEFT_CENTER,
        &name,
        egui::FontId::proportional(12.0),
        if layer.visible { p.text } else { p.text_muted },
    );

    // name column: select, right-click for the layer menu
    let name_resp = ui.interact(names, Id::new(("tl-name", id)), Sense::click());
    if name_resp.clicked() {
        let m = ui.input(|i| i.modifiers);
        let mode = if m.command {
            SelectMode::Toggle
        } else if m.shift {
            SelectMode::Range
        } else {
            SelectMode::Replace
        };
        shell.editor.select(id, mode);
    }
    if name_resp.secondary_clicked() && !selected {
        shell.editor.select(id, SelectMode::Replace);
    }
    menus::context_menu(&name_resp, |ui| menus::layer_context(ui, shell));

    // keyframes
    let now = shell.editor.current_frame();
    let key_painter = ui.painter_at(keys.intersect(keys_clip));
    for f in shell.editor.key_frames(id) {
        let c = pos2(x_of(f as f32), keys.center().y);
        let s = 5.0;
        let fill = if f == now { p.accent_strong } else { p.keyframe };
        key_painter.add(Shape::convex_polygon(
            vec![
                c + vec2(0.0, -s),
                c + vec2(s, 0.0),
                c + vec2(0.0, s),
                c + vec2(-s, 0.0),
            ],
            fill,
            Stroke::new(1.0, p.bg),
        ));
    }

    // key area: click to select the layer and jump to that frame;
    // right-click to add or delete a key there
    let key_resp = ui.interact(
        keys.intersect(keys_clip),
        Id::new(("tl-keys", id)),
        Sense::click(),
    );
    if key_resp.clicked() || key_resp.secondary_clicked() {
        if let Some(pos) = key_resp.interact_pointer_pos() {
            shell.editor.go_to(frame_at(pos.x));
        }
        if !selected || key_resp.clicked() {
            shell.editor.select(id, SelectMode::Replace);
        }
    }
    menus::context_menu(&key_resp, |ui| {
        let f = shell.editor.current_frame();
        ui.label(RichText::new(format!("Frame {f}")).color(p.text_muted));
        menus::item(ui, shell, Action::AddKey);
        menus::item(ui, shell, Action::DeleteKey);
        ui.separator();
        menus::item(ui, shell, Action::PrevKey);
        menus::item(ui, shell, Action::NextKey);
    });
}

fn transport(ui: &mut egui::Ui, shell: &mut Shell) {
    ui.horizontal(|ui| {
        let tip = |shell: &Shell, a: Action| {
            let s = shell.keymap.shortcut_text(a);
            if s.is_empty() {
                a.label().to_owned()
            } else {
                format!("{} ({s})", a.label())
            }
        };
        let play = if shell.editor.playing { "⏸" } else { "⏵" };
        for (glyph, action) in [
            ("⏮", Action::FirstFrame),
            ("|⏴", Action::PrevFrame),
            (play, Action::PlayPause),
            ("⏵|", Action::NextFrame),
            ("⏭", Action::LastFrame),
        ] {
            if ui.button(glyph).on_hover_text(tip(shell, action)).clicked() {
                shell.pending.push(action);
            }
        }
        ui.separator();
        ui.label("Frame");
        let mut f = shell.editor.current_frame();
        let (start, end) = (shell.editor.doc.start, shell.editor.doc.end);
        if ui
            .add(egui::DragValue::new(&mut f).range(start..=end).speed(0.2))
            .changed()
        {
            shell.editor.go_to(f);
        }
        ui.label(
            RichText::new(format!("of {end}  ·  {} fps", shell.editor.doc.fps))
                .color(shell.palette.text_muted),
        );
        ui.separator();
        for (glyph, action) in [("◆+", Action::AddKey), ("◆−", Action::DeleteKey)] {
            if ui
                .add_enabled(shell.editor.can_run(action), egui::Button::new(glyph))
                .on_hover_text(tip(shell, action))
                .clicked()
            {
                shell.pending.push(action);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new("Ctrl+scroll to zoom")
                    .size(11.0)
                    .color(shell.palette.text_muted),
            );
        });
    });
}
