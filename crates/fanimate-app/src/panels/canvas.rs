//! The canvas: the GPU-rendered document, plus navigation, the Transform
//! Layer tool, click-to-select and overlays.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke};
use fanimate_core::{Color, LayerId, LayerKind, Vec2, evaluate};
use fanimate_editor::{Action, SelectMode, Tool};
use fanimate_render::View;

use crate::app::Shell;
use crate::menus;

const FIT_MARGIN: f32 = 32.0;

pub fn show(ui: &mut egui::Ui, shell: &mut Shell) {
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, Sense::click_and_drag());
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    let doc_size = shell.editor.doc.size();

    if shell.view.fit_pending && rect.width() > 1.0 {
        let avail = (Vec2::new(rect.width(), rect.height()) - Vec2::splat(2.0 * FIT_MARGIN)).max(Vec2::ONE);
        shell.view.zoom = (avail / doc_size.max(Vec2::ONE)).min_element();
        shell.view.center = doc_size / 2.0;
        shell.view.fit_pending = false;
    }

    let to_screen = |center: Vec2, zoom: f32, p: Vec2| -> Pos2 {
        let s = (p - center) * zoom;
        rect.center() + egui::vec2(s.x, s.y)
    };
    let to_doc = |center: Vec2, zoom: f32, p: Pos2| -> Vec2 {
        let d = p - rect.center();
        center + Vec2::new(d.x, d.y) / zoom
    };

    navigate(&ctx, &response, shell, rect, &to_doc);
    transform_tool(&ctx, &response, shell, &to_doc);

    // render the document into a texture and show it
    if let (Some(canvas), Some(rs)) = (&mut shell.canvas, &shell.render_state) {
        let size_px = [
            (rect.width() * ppp).round() as u32,
            (rect.height() * ppp).round() as u32,
        ];
        let view = View::looking_at(
            shell.view.center,
            Vec2::new(size_px[0] as f32, size_px[1] as f32),
            shell.view.zoom * ppp,
        );
        let p = shell.palette.pasteboard;
        let pasteboard = Color::rgb(p.r() as f32 / 255.0, p.g() as f32 / 255.0, p.b() as f32 / 255.0);
        let id = canvas.draw(
            rs,
            &shell.editor.doc,
            shell.editor.frame,
            size_px,
            &view,
            pasteboard,
        );
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        ui.painter().image(id, rect, uv, Color32::WHITE);
    } else {
        ui.painter().rect_filled(rect, 0.0, shell.palette.pasteboard);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No GPU available for the canvas",
            egui::FontId::proportional(14.0),
            shell.palette.error,
        );
    }

    overlays(ui, shell, rect, &to_screen);

    response.context_menu(|ui| {
        menus::item(ui, shell, Action::FitView);
        menus::item(ui, shell, Action::ActualSize);
        menus::item(ui, shell, Action::ZoomIn);
        menus::item(ui, shell, Action::ZoomOut);
        ui.separator();
        menus::layer_context(ui, shell);
    });
}

fn navigate(
    ctx: &egui::Context,
    response: &egui::Response,
    shell: &mut Shell,
    rect: Rect,
    to_doc: &impl Fn(Vec2, f32, Pos2) -> Vec2,
) {
    let view = &mut shell.view;
    // pan: middle-drag with any tool, or the Pan tool
    let panning = response.dragged_by(egui::PointerButton::Middle)
        || (shell.editor.tool == Tool::Pan && response.dragged_by(egui::PointerButton::Primary));
    if panning {
        let d = response.drag_delta();
        view.center -= Vec2::new(d.x, d.y) / view.zoom;
    }
    if !response.hovered() {
        return;
    }
    // Ctrl+scroll or pinch zooms about the pointer; plain scroll pans
    let (zoom_delta, scroll, pointer) =
        ctx.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
    let pointer = pointer.unwrap_or(rect.center());
    if zoom_delta != 1.0 {
        zoom_about(view, pointer, zoom_delta, to_doc);
    } else if scroll != egui::Vec2::ZERO {
        view.center -= Vec2::new(scroll.x, scroll.y) / view.zoom;
    }
    // Zoom tool: click in, Alt-click out
    if shell.editor.tool == Tool::Zoom && response.clicked() {
        let factor = if ctx.input(|i| i.modifiers.alt) { 0.8 } else { 1.25 };
        zoom_about(view, pointer, factor, to_doc);
    }
}

fn zoom_about(
    view: &mut crate::app::CanvasView,
    pointer: Pos2,
    factor: f32,
    to_doc: &impl Fn(Vec2, f32, Pos2) -> Vec2,
) {
    let before = to_doc(view.center, view.zoom, pointer);
    view.zoom = (view.zoom * factor).clamp(0.02, 64.0);
    let after = to_doc(view.center, view.zoom, pointer);
    // keep the document point under the pointer still
    view.center += before - after;
}

fn transform_tool(
    ctx: &egui::Context,
    response: &egui::Response,
    shell: &mut Shell,
    to_doc: &impl Fn(Vec2, f32, Pos2) -> Vec2,
) {
    if shell.editor.tool != Tool::TransformLayer {
        return;
    }
    let mods = ctx.input(|i| i.modifiers);

    // click: select the topmost shape under the pointer
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let p = to_doc(shell.view.center, shell.view.zoom, pos);
        match hit_test(shell, p) {
            Some(id) => {
                let mode = if mods.command {
                    SelectMode::Toggle
                } else if mods.shift {
                    SelectMode::Range
                } else {
                    SelectMode::Replace
                };
                shell.editor.select(id, mode);
            }
            None if !mods.command && !mods.shift => shell.editor.selection.clear(),
            None => {}
        }
    }

    // drag: move the selected layer (pick one under the pointer if none)
    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some(pos) = response.interact_pointer_pos()
    {
        if shell.editor.primary().is_none()
            && let Some(id) = hit_test(shell, to_doc(shell.view.center, shell.view.zoom, pos))
        {
            shell.editor.select(id, SelectMode::Replace);
        }
        if let Some(id) = shell.editor.primary()
            && let Some(t) = shell.editor.layer_transform(id)
        {
            shell.editor.checkpoint("Move layer");
            shell.ui.drag_origin = Some((id, t));
        }
    }
    if response.dragged_by(egui::PointerButton::Primary)
        && let Some((id, origin)) = shell.ui.drag_origin
    {
        let total = response.total_drag_delta().unwrap_or_default();
        let doc_delta = Vec2::new(total.x, total.y) / shell.view.zoom;
        // drag in document space -> movement in the layer's parent space
        let parent = shell.editor.parent_world(id);
        let local = parent.matrix2.inverse() * doc_delta;
        let mut t = origin;
        t.position += if local.is_finite() { local } else { doc_delta };
        shell.editor.set_layer_transform(id, t);
    }
    if response.drag_stopped() {
        shell.ui.drag_origin = None;
    }
}

/// The deepest layer with a shape whose bounds contain `p`, topmost first.
fn hit_test(shell: &Shell, p: Vec2) -> Option<LayerId> {
    let items = evaluate(&shell.editor.doc, shell.editor.frame);
    items
        .iter()
        .rev()
        .find(|item| {
            bounds(
                item.shape
                    .path
                    .points()
                    .map(|q| item.transform.transform_point2(q)),
            )
            .is_some_and(|(lo, hi)| p.cmpge(lo).all() && p.cmple(hi).all())
        })
        .map(|item| item.layer)
}

fn bounds(points: impl Iterator<Item = Vec2>) -> Option<(Vec2, Vec2)> {
    points.fold(None, |acc, q| match acc {
        None => Some((q, q)),
        Some((lo, hi)) => Some((lo.min(q), hi.max(q))),
    })
}

/// Selection outlines and the bones of selected bone layers.
fn overlays(ui: &egui::Ui, shell: &Shell, rect: Rect, to_screen: &impl Fn(Vec2, f32, Vec2) -> Pos2) {
    let ed = &shell.editor;
    if ed.selection.is_empty() {
        return;
    }
    let painter = ui.painter_at(rect);
    let (c, z) = (shell.view.center, shell.view.zoom);
    let p = shell.palette;

    let items = evaluate(&ed.doc, ed.frame);
    for sel in &ed.selection {
        let pts = items
            .iter()
            .filter(|i| ed.doc.is_within(i.layer, *sel))
            .flat_map(|i| {
                i.shape
                    .path
                    .points()
                    .map(move |q| i.transform.transform_point2(q))
            });
        if let Some((lo, hi)) = bounds(pts) {
            let r = Rect::from_two_pos(to_screen(c, z, lo), to_screen(c, z, hi)).expand(3.0);
            painter.rect_stroke(r, 2.0, Stroke::new(1.5, p.accent), egui::StrokeKind::Outside);
        }

        // bones of a selected bone layer
        let Some(layer) = ed.doc.find(*sel) else { continue };
        let LayerKind::Bone { skeleton, .. } = &layer.kind else {
            continue;
        };
        let world = ed.world(*sel);
        let mut poses = Vec::new();
        skeleton.pose(ed.frame, &mut poses);
        for (i, bone) in skeleton.bones.iter().enumerate() {
            let a = world.transform_point2(poses[i].transform_point2(Vec2::ZERO));
            let b = world.transform_point2(poses[i].transform_point2(Vec2::new(bone.length, 0.0)));
            let (sa, sb) = (to_screen(c, z, a), to_screen(c, z, b));
            painter.line_segment([sa, sb], Stroke::new(3.0, p.bone));
            painter.circle_filled(sa, 4.5, p.bone);
            painter.circle_stroke(sa, 4.5, Stroke::new(1.0, p.bg));
        }
    }
}
