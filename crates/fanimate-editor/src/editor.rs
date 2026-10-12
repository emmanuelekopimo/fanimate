//! The editor: an open document plus everything about editing it.

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use fanimate_core::anim::Animatable;
use fanimate_core::bones::Skeleton;
use fanimate_core::{
    Affine2, Channel, Document, Frame, Interpolation, Layer, LayerId, LayerKind, Transform, samples,
};
use fanimate_fnm::{FnmError, Report};
use serde::{Deserialize, Serialize};

use crate::action::{Action, Tool};
use crate::history::History;

mod draw;
mod points;
pub use draw::DrawStyle;
pub use points::PointRef;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode {
    /// Plain click: select only this.
    Replace,
    /// Ctrl-click: add or remove this.
    Toggle,
    /// Shift-click: everything between the last clicked layer and this.
    Range,
}

/// Where dropped layers go relative to the layer under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Above,
    Below,
    /// Inside a group or bone layer, on top of its children.
    Into,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Warning,
    Error,
}

/// What running an action asks of the app.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Handled by the editor.
    Done,
    /// Handled; put this text on the system clipboard.
    CopyText(String),
    /// Needs the UI: a dialog, the view, or quitting.
    App(Action),
}

/// Layers on the clipboard, as JSON. Tagged so pasting arbitrary text is
/// recognised and ignored.
#[derive(Serialize, Deserialize)]
struct ClipboardData {
    fanimate: String,
    version: u32,
    layers: Vec<Layer>,
}

const CLIPBOARD_TAG: &str = "layers";

pub struct Editor {
    pub doc: Document,
    /// Where the project is saved; `None` until the first save.
    pub path: Option<PathBuf>,
    /// Unsaved changes.
    pub dirty: bool,
    /// Counts edits, undos and redos, so the app can tell when the document
    /// changed (for the crash recovery journal).
    revision: u64,
    /// Selected layers in click order; the last is the primary selection.
    pub selection: Vec<LayerId>,
    /// Selected points, for the point tools.
    pub points: BTreeSet<PointRef>,
    /// Current time; fractional while playing.
    pub frame: f32,
    pub playing: bool,
    pub tool: Tool,
    pub history: History,
    /// Groups collapsed in the Layers panel.
    pub collapsed: HashSet<LayerId>,
    /// Set when the UI should start renaming a layer in place.
    pub rename_request: Option<LayerId>,
    pub status: Option<(StatusKind, String)>,
    /// Colours and brush settings for new artwork.
    pub draw: DrawStyle,
    /// The brush stroke being drawn.
    stroke: Option<draw::StrokeInProgress>,
    /// The eraser gesture in progress.
    erase: Option<draw::EraseInProgress>,
    clipboard: Option<String>,
    /// Last plainly clicked layer, the anchor for Shift-click ranges.
    anchor: Option<LayerId>,
}

impl Editor {
    pub fn new(doc: Document) -> Self {
        let frame = doc.start as f32;
        Self {
            doc,
            path: None,
            dirty: false,
            revision: 0,
            selection: Vec::new(),
            points: BTreeSet::new(),
            frame,
            playing: false,
            tool: Tool::default(),
            history: History::default(),
            collapsed: HashSet::new(),
            rename_request: None,
            status: None,
            draw: DrawStyle::default(),
            stroke: None,
            erase: None,
            clipboard: None,
            anchor: None,
        }
    }

    /// A new, empty 1280x720 project with one vector layer.
    pub fn untitled() -> Self {
        let mut doc = Document::new("Untitled", 1280.0, 720.0, 24.0);
        doc.end = 119;
        let layer = doc.new_layer("Layer 1", LayerKind::vector(vec![]));
        let id = layer.id;
        doc.layers.push(layer);
        let mut ed = Self::new(doc);
        ed.selection = vec![id];
        ed
    }

    pub fn sample() -> Self {
        Self::new(samples::hello())
    }

    /// Open a project. Damage the validator can fix is repaired (and marks
    /// the project as changed, so the repair is saved).
    pub fn open(path: &Path) -> Result<(Self, Report), FnmError> {
        let (doc, report) = fanimate_fnm::load_and_repair(path)?;
        let mut ed = Self::new(doc);
        ed.path = Some(path.to_path_buf());
        ed.dirty = report.repaired_count() > 0;
        Ok((ed, report))
    }

    /// Adopt work recovered after a crash. It is unsaved until saved
    /// again, to `path` if it had been saved before.
    pub fn recovered(doc: Document, path: Option<PathBuf>) -> Self {
        let mut ed = Self::new(doc);
        ed.path = path;
        ed.dirty = true;
        ed.set_status(StatusKind::Success, "Recovered unsaved work. Save to keep it.");
        ed
    }

    /// See the `revision` field.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn save_to(&mut self, path: &Path) -> Result<(), FnmError> {
        fanimate_fnm::save(&self.doc, path)?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        self.set_status(StatusKind::Success, format!("Saved {}", path.display()));
        Ok(())
    }

    pub fn set_status(&mut self, kind: StatusKind, message: impl Into<String>) {
        self.status = Some((kind, message.into()));
    }

    /// Record the state before an edit. Call once per user action: for a
    /// drag, at drag start, not on every movement.
    pub fn checkpoint(&mut self, label: &'static str) {
        self.history.push(label, &self.doc, &self.selection);
        self.dirty = true;
        self.revision += 1;
    }

    // ------------------------------------------------------------------
    // Actions

    pub fn run(&mut self, action: Action) -> Outcome {
        use Action::*;
        match action {
            Undo => self.undo(),
            Redo => self.redo(),
            Copy => {
                return match self.copy() {
                    Some(text) => Outcome::CopyText(text),
                    None => Outcome::Done,
                };
            }
            Cut => {
                return match self.cut() {
                    Some(text) => Outcome::CopyText(text),
                    None => Outcome::Done,
                };
            }
            Paste => {
                self.paste(None);
            }
            Duplicate => self.duplicate(),
            // with a point tool, editing commands act on points
            Delete if self.tool.edits_points() && !self.points.is_empty() => {
                self.delete_points();
            }
            SelectAll if self.tool.edits_points() => self.select_all_points(),
            SelectNone if self.tool.edits_points() => self.points.clear(),
            Delete => self.delete_selected(),
            SelectAll => self.select_all(),
            SelectNone => self.selection.clear(),
            Rename => self.rename_request = self.primary(),
            NewVectorLayer => self.new_layer(NewKind::Vector),
            NewGroupLayer => self.new_layer(NewKind::Group),
            NewBoneLayer => self.new_layer(NewKind::Bone),
            GroupLayers => self.group_selected(),
            Ungroup => self.ungroup_selected(),
            MoveLayerUp => self.move_selected(true),
            MoveLayerDown => self.move_selected(false),
            ToggleVisibility => self.toggle_visibility(),
            PlayPause => self.playing = !self.playing,
            NextFrame => self.step(1),
            PrevFrame => self.step(-1),
            FirstFrame => self.go_to(self.doc.start),
            LastFrame => self.go_to(self.doc.end),
            NextKey => self.jump_key(true),
            PrevKey => self.jump_key(false),
            AddKey => self.add_key(),
            DeleteKey => self.delete_key(),
            Tool(t) => self.set_tool(t),
            AddPointOrBone => {
                let bone = self
                    .primary()
                    .and_then(|id| self.doc.find(id))
                    .is_some_and(|l| matches!(l.kind, LayerKind::Bone { .. }));
                // full path: `Tool` here is the Action variant
                use crate::action::Tool as T;
                self.set_tool(if bone { T::AddBone } else { T::AddPoint });
            }
            New | Open | OpenSample(_) | Save | SaveAs | Quit | Preferences | CommandPalette
            | LayerProperties | ZoomIn | ZoomOut | FitView | ActualSize => return Outcome::App(action),
        }
        Outcome::Done
    }

    /// Whether `action` can run now, for greying out menu items.
    pub fn can_run(&self, action: Action) -> bool {
        use Action::*;
        let has_sel = !self.selection.is_empty();
        match action {
            Undo => self.history.undo_label().is_some(),
            Redo => self.history.redo_label().is_some(),
            Cut | Copy | Duplicate | Delete | GroupLayers | MoveLayerUp | MoveLayerDown
            | ToggleVisibility | AddKey | DeleteKey => has_sel,
            Rename | LayerProperties => self.primary().is_some(),
            Paste => self.clipboard.is_some(),
            Ungroup => self.selected_roots().iter().any(|id| {
                self.doc
                    .find(*id)
                    .is_some_and(|l| matches!(l.kind, LayerKind::Group { .. }))
            }),
            _ => true,
        }
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        if !tool.is_available() {
            self.set_status(StatusKind::Info, format!("{}: {}", tool.label(), tool.hint()));
        }
    }

    // ------------------------------------------------------------------
    // Undo

    fn undo(&mut self) {
        match self.history.undo(&mut self.doc, &mut self.selection) {
            Some(label) => {
                self.dirty = true;
                self.revision += 1;
                self.prune_selection();
                self.set_status(StatusKind::Info, format!("Undid {label}"));
            }
            None => self.set_status(StatusKind::Info, "Nothing to undo"),
        }
    }

    fn redo(&mut self) {
        match self.history.redo(&mut self.doc, &mut self.selection) {
            Some(label) => {
                self.dirty = true;
                self.revision += 1;
                self.prune_selection();
                self.set_status(StatusKind::Info, format!("Redid {label}"));
            }
            None => self.set_status(StatusKind::Info, "Nothing to redo"),
        }
    }

    fn prune_selection(&mut self) {
        let doc = &self.doc;
        self.selection.retain(|id| doc.find(*id).is_some());
        self.prune_points();
    }

    // ------------------------------------------------------------------
    // Selection and layer order

    pub fn primary(&self) -> Option<LayerId> {
        self.selection.last().copied()
    }

    pub fn is_selected(&self, id: LayerId) -> bool {
        self.selection.contains(&id)
    }

    /// Layers as the Layers panel lists them: top of the stack first,
    /// children indented under their parent, collapsed groups' children
    /// hidden. Pairs of (layer, depth).
    pub fn display_rows(&self) -> Vec<(LayerId, usize)> {
        self.rows(true)
    }

    fn rows(&self, honour_collapsed: bool) -> Vec<(LayerId, usize)> {
        fn go(
            layers: &[Layer],
            depth: usize,
            collapsed: Option<&HashSet<LayerId>>,
            out: &mut Vec<(LayerId, usize)>,
        ) {
            // last in the list is drawn on top, so it is listed first
            for l in layers.iter().rev() {
                out.push((l.id, depth));
                if let Some(children) = l.kind.children()
                    && !collapsed.is_some_and(|c| c.contains(&l.id))
                {
                    go(children, depth + 1, collapsed, out);
                }
            }
        }
        let mut out = Vec::new();
        go(
            &self.doc.layers,
            0,
            honour_collapsed.then_some(&self.collapsed),
            &mut out,
        );
        out
    }

    pub fn select(&mut self, id: LayerId, mode: SelectMode) {
        match mode {
            SelectMode::Replace => {
                self.selection = vec![id];
                self.anchor = Some(id);
            }
            SelectMode::Toggle => {
                if let Some(i) = self.selection.iter().position(|s| *s == id) {
                    self.selection.remove(i);
                } else {
                    self.selection.push(id);
                }
                self.anchor = Some(id);
            }
            SelectMode::Range => {
                let rows = self.display_rows();
                let pos = |x: LayerId| rows.iter().position(|(r, _)| *r == x);
                match (self.anchor.and_then(pos), pos(id)) {
                    (Some(a), Some(b)) => {
                        let (lo, hi) = (a.min(b), a.max(b));
                        self.selection = rows[lo..=hi].iter().map(|(r, _)| *r).collect();
                        // keep the clicked layer primary
                        self.selection.retain(|r| *r != id);
                        self.selection.push(id);
                    }
                    _ => self.select(id, SelectMode::Replace),
                }
            }
        }
    }

    pub fn select_all(&mut self) {
        self.selection = self.rows(false).into_iter().map(|(id, _)| id).collect();
    }

    /// Selected layers that are not inside another selected layer, in
    /// display order (top first). Commands act on these, so a group and
    /// its child are never copied twice.
    pub fn selected_roots(&self) -> Vec<LayerId> {
        self.rows(false)
            .into_iter()
            .map(|(id, _)| id)
            .filter(|id| self.is_selected(*id))
            .filter(|id| {
                !self
                    .selection
                    .iter()
                    .any(|other| other != id && self.doc.is_within(*id, *other))
            })
            .collect()
    }

    /// Where new and pasted layers go: just above the primary selection,
    /// in the same parent; otherwise on top of the stack.
    fn insertion_point(&self) -> (Option<LayerId>, usize) {
        self.primary()
            .and_then(|id| self.doc.locate(id))
            .map(|(parent, index)| (parent, index + 1))
            .unwrap_or((None, self.doc.layers.len()))
    }

    // ------------------------------------------------------------------
    // Layer commands

    fn new_layer(&mut self, kind: NewKind) {
        self.checkpoint("New layer");
        let (word, kind) = match kind {
            NewKind::Vector => ("Layer", LayerKind::vector(vec![])),
            NewKind::Group => ("Group", LayerKind::Group { children: vec![] }),
            NewKind::Bone => (
                "Bones",
                LayerKind::Bone {
                    skeleton: Skeleton::default(),
                    children: vec![],
                },
            ),
        };
        let name = self.unused_name(word);
        let layer = self.doc.new_layer(name, kind);
        let id = layer.id;
        let (parent, index) = self.insertion_point();
        self.doc.insert(parent, index, layer);
        self.selection = vec![id];
        self.anchor = Some(id);
        self.rename_request = Some(id);
    }

    fn delete_selected(&mut self) {
        let roots = self.selected_roots();
        if roots.is_empty() {
            return;
        }
        self.checkpoint("Delete");
        for id in &roots {
            self.doc.remove(*id);
        }
        self.selection.clear();
        let n = roots.len();
        self.set_status(StatusKind::Info, format!("Deleted {n} layer{}", plural(n)));
    }

    fn duplicate(&mut self) {
        let roots = self.selected_roots();
        if roots.is_empty() {
            return;
        }
        self.checkpoint("Duplicate");
        let mut new_sel = Vec::new();
        for id in roots {
            let Some(original) = self.doc.find(id).cloned() else {
                continue;
            };
            let Some((parent, index)) = self.doc.locate(id) else {
                continue;
            };
            let mut copy = original;
            self.doc.reassign_ids(&mut copy);
            copy.name = format!("{} copy", copy.name);
            new_sel.push(copy.id);
            self.doc.insert(parent, index + 1, copy);
        }
        self.selection = new_sel;
    }

    /// Copy the selected layers; returns clipboard text.
    pub fn copy(&mut self) -> Option<String> {
        let layers: Vec<Layer> = self
            .selected_roots()
            .into_iter()
            .rev() // document order, bottom first
            .filter_map(|id| self.doc.find(id).cloned())
            .collect();
        if layers.is_empty() {
            return None;
        }
        let n = layers.len();
        let data = ClipboardData {
            fanimate: CLIPBOARD_TAG.into(),
            version: fanimate_fnm::FORMAT_VERSION,
            layers,
        };
        let text = serde_json::to_string(&data).ok()?;
        self.clipboard = Some(text.clone());
        self.set_status(StatusKind::Info, format!("Copied {n} layer{}", plural(n)));
        Some(text)
    }

    pub fn cut(&mut self) -> Option<String> {
        let text = self.copy()?;
        self.checkpoint("Cut");
        for id in self.selected_roots() {
            self.doc.remove(id);
        }
        self.selection.clear();
        Some(text)
    }

    /// Paste layers from `text` (the system clipboard) if it holds Fanimate
    /// layers, else from the internal clipboard. Returns whether anything
    /// was pasted.
    pub fn paste(&mut self, text: Option<&str>) -> bool {
        let parse = |t: &str| {
            serde_json::from_str::<ClipboardData>(t)
                .ok()
                .filter(|d| d.fanimate == CLIPBOARD_TAG && d.version <= fanimate_fnm::FORMAT_VERSION)
        };
        let data = text
            .and_then(parse)
            .or_else(|| self.clipboard.as_deref().and_then(parse));
        let Some(data) = data else {
            return false;
        };
        if data.layers.is_empty() {
            return false;
        }
        self.checkpoint("Paste");
        let (parent, mut index) = self.insertion_point();
        let mut pasted = Vec::new();
        for mut layer in data.layers {
            self.doc.reassign_ids(&mut layer);
            pasted.push(layer.id);
            if !self.doc.insert(parent, index, layer.clone()) {
                self.doc.insert(None, self.doc.layers.len(), layer);
            }
            index += 1;
        }
        self.selection = pasted;
        true
    }

    /// Move each selected layer one step up (towards the top of the stack)
    /// or down among its siblings.
    fn move_selected(&mut self, up: bool) {
        let mut roots = self.selected_roots();
        if roots.is_empty() {
            return;
        }
        // move the leading layer first so neighbours do not block each other
        if !up {
            roots.reverse();
        }
        let mut moved = false;
        let mut checkpointed = false;
        for id in roots {
            let Some((parent, index)) = self.doc.locate(id) else {
                continue;
            };
            let Some(len) = self.doc.siblings_mut(parent).map(|s| s.len()) else {
                continue;
            };
            let target = if up { index + 1 } else { index.wrapping_sub(1) };
            if target >= len {
                continue;
            }
            let neighbour = self.doc.siblings_mut(parent).map(|s| s[target].id);
            if neighbour.is_some_and(|n| self.is_selected(n)) {
                continue;
            }
            if !checkpointed {
                self.checkpoint(if up { "Move layer up" } else { "Move layer down" });
                checkpointed = true;
            }
            if let Some(list) = self.doc.siblings_mut(parent) {
                list.swap(index, target);
                moved = true;
            }
        }
        if !moved {
            self.set_status(StatusKind::Info, "Already at the end of its group");
        }
    }

    /// Put the selected layers into a new group where the primary
    /// selection was.
    fn group_selected(&mut self) {
        let roots = self.selected_roots();
        let Some(anchor) = self
            .primary()
            .filter(|p| roots.contains(p))
            .or(roots.first().copied())
        else {
            return;
        };
        let Some((parent, _)) = self.doc.locate(anchor) else {
            return;
        };
        self.checkpoint("Group layers");
        // the group takes the place of the topmost selected layer in `parent`
        let index = roots
            .iter()
            .filter_map(|id| self.doc.locate(*id))
            .filter(|(p, _)| *p == parent)
            .map(|(_, i)| i)
            .max()
            .unwrap_or(0);
        let removed_below = roots
            .iter()
            .filter_map(|id| self.doc.locate(*id))
            .filter(|(p, i)| *p == parent && *i < index)
            .count();
        let mut children: Vec<Layer> = Vec::new();
        for id in roots.iter().rev() {
            if let Some(layer) = self.doc.remove(*id) {
                children.push(layer);
            }
        }
        let name = self.unused_name("Group");
        let group = self.doc.new_layer(name, LayerKind::Group { children });
        let gid = group.id;
        self.doc.insert(parent, index - removed_below, group);
        self.selection = vec![gid];
    }

    /// Replace selected groups with their contents. Only groups with no
    /// transform or opacity of their own, so nothing visibly moves.
    fn ungroup_selected(&mut self) {
        let groups: Vec<LayerId> = self
            .selected_roots()
            .into_iter()
            .filter(|id| {
                self.doc
                    .find(*id)
                    .is_some_and(|l| matches!(l.kind, LayerKind::Group { .. }))
            })
            .collect();
        let (plain, transformed): (Vec<_>, Vec<_>) = groups.into_iter().partition(|id| {
            self.doc.find(*id).is_some_and(|l| {
                !l.transform.is_animated()
                    && l.transform.sample(0.0) == Transform::IDENTITY
                    && !l.opacity.is_animated()
                    && l.opacity.base == 1.0
            })
        });
        if !transformed.is_empty() {
            self.set_status(
                StatusKind::Warning,
                "A group with its own transform or opacity can't be ungrouped yet; reset it first",
            );
        }
        if plain.is_empty() {
            return;
        }
        self.checkpoint("Ungroup");
        let mut new_sel = Vec::new();
        for id in plain {
            let Some((parent, index)) = self.doc.locate(id) else {
                continue;
            };
            let Some(group) = self.doc.remove(id) else {
                continue;
            };
            if let LayerKind::Group { children } = group.kind {
                for (offset, child) in children.into_iter().enumerate() {
                    new_sel.push(child.id);
                    self.doc.insert(parent, index + offset, child);
                }
            }
        }
        self.selection = new_sel;
    }

    /// Move layers next to, or into, `target` (drag and drop in the Layers
    /// panel). Layers keep their relative order. Refuses to move a layer
    /// into itself or its own descendants.
    pub fn move_relative(&mut self, ids: &[LayerId], target: LayerId, placement: Placement) -> bool {
        let roots: Vec<LayerId> = self
            .rows(false)
            .into_iter()
            .map(|(id, _)| id)
            .filter(|id| ids.contains(id))
            .filter(|id| !ids.iter().any(|o| o != id && self.doc.is_within(*id, *o)))
            .collect();
        if roots.is_empty() || roots.iter().any(|r| self.doc.is_within(target, *r)) {
            return false;
        }
        if placement == Placement::Into && self.doc.find(target).is_none_or(|l| l.kind.children().is_none()) {
            return false;
        }
        self.checkpoint("Move layers");
        // remove bottom-most first; collect in document order (bottom first)
        let mut moving: Vec<Layer> = roots.iter().rev().filter_map(|id| self.doc.remove(*id)).collect();
        let (parent, index) = match placement {
            Placement::Into => {
                let len = self
                    .doc
                    .find(target)
                    .and_then(|l| l.kind.children())
                    .map_or(0, |c| c.len());
                (Some(target), len)
            }
            Placement::Above | Placement::Below => {
                let Some((parent, i)) = self.doc.locate(target) else {
                    return false;
                };
                (parent, if placement == Placement::Above { i + 1 } else { i })
            }
        };
        for (offset, layer) in moving.drain(..).enumerate() {
            self.doc.insert(parent, index + offset, layer);
        }
        self.selection = roots;
        true
    }

    /// Transform from a layer's parent space to document space at the
    /// current frame. Used to turn canvas drags into layer movement.
    pub fn parent_world(&self, id: LayerId) -> Affine2 {
        let mut chain = Vec::new();
        let mut current = self.doc.locate(id).and_then(|(p, _)| p);
        while let Some(c) = current {
            chain.push(c);
            current = self.doc.locate(c).and_then(|(p, _)| p);
        }
        chain
            .iter()
            .rev()
            .filter_map(|id| self.doc.find(*id))
            .fold(Affine2::IDENTITY, |acc, l| {
                acc * l.transform.sample(self.frame).to_affine()
            })
    }

    /// Transform from a layer's own space to document space.
    pub fn world(&self, id: LayerId) -> Affine2 {
        let own = self
            .doc
            .find(id)
            .map_or(Affine2::IDENTITY, |l| l.transform.sample(self.frame).to_affine());
        self.parent_world(id) * own
    }

    /// `"{word} N"` with the lowest N not already used by a layer.
    fn unused_name(&self, word: &str) -> String {
        let mut used = HashSet::new();
        self.doc.walk(|l| {
            if let Some(n) = l
                .name
                .strip_prefix(word)
                .and_then(|r| r.trim().parse::<usize>().ok())
            {
                used.insert(n);
            }
        });
        let n = (1..).find(|n| !used.contains(n)).unwrap_or(1);
        format!("{word} {n}")
    }

    pub fn rename(&mut self, id: LayerId, name: &str) {
        let name = name.trim();
        if name.is_empty() || self.doc.find(id).is_none_or(|l| l.name == name) {
            return;
        }
        self.checkpoint("Rename");
        if let Some(l) = self.doc.find_mut(id) {
            l.name = name.to_owned();
        }
    }

    pub fn set_visible(&mut self, id: LayerId, visible: bool) {
        if self.doc.find(id).is_none_or(|l| l.visible == visible) {
            return;
        }
        self.checkpoint(if visible { "Show layer" } else { "Hide layer" });
        if let Some(l) = self.doc.find_mut(id) {
            l.visible = visible;
        }
    }

    fn toggle_visibility(&mut self) {
        let roots = self.selected_roots();
        let Some(first) = roots.first().and_then(|id| self.doc.find(*id)) else {
            return;
        };
        // like most apps: if the first is visible, hide all; else show all
        let visible = !first.visible;
        self.checkpoint(if visible { "Show layers" } else { "Hide layers" });
        for id in roots {
            if let Some(l) = self.doc.find_mut(id) {
                l.visible = visible;
            }
        }
    }

    // ------------------------------------------------------------------
    // Animation

    pub fn current_frame(&self) -> Frame {
        self.frame.floor() as Frame
    }

    pub fn go_to(&mut self, frame: Frame) {
        self.playing = false;
        self.frame = frame.clamp(self.doc.start, self.doc.end) as f32;
    }

    fn step(&mut self, by: Frame) {
        self.go_to(self.current_frame() + by);
    }

    /// Advance playback by `seconds`, looping over the frame range.
    pub fn tick(&mut self, seconds: f32) {
        if !self.playing {
            return;
        }
        let n = self.doc.frame_count().max(1) as f32;
        let t = self.frame - self.doc.start as f32 + seconds * self.doc.fps;
        self.frame = self.doc.start as f32 + t.rem_euclid(n);
    }

    /// Frames with any keyframe on this layer (its transform and opacity,
    /// and for bone layers its bones), sorted.
    pub fn key_frames(&self, id: LayerId) -> Vec<Frame> {
        let Some(layer) = self.doc.find(id) else {
            return Vec::new();
        };
        let mut frames: Vec<Frame> = Vec::new();
        let t = &layer.transform;
        frames.extend(layer.opacity.keys.iter().map(|k| k.frame));
        frames.extend(t.position.keys.iter().map(|k| k.frame));
        frames.extend(t.rotation.keys.iter().map(|k| k.frame));
        frames.extend(t.scale.keys.iter().map(|k| k.frame));
        if let LayerKind::Bone { skeleton, .. } = &layer.kind {
            for b in &skeleton.bones {
                frames.extend(b.angle.keys.iter().map(|k| k.frame));
            }
        }
        frames.sort_unstable();
        frames.dedup();
        frames
    }

    fn jump_key(&mut self, forward: bool) {
        let ids: Vec<LayerId> = if self.selection.is_empty() {
            self.rows(false).into_iter().map(|(id, _)| id).collect()
        } else {
            self.selection.clone()
        };
        let now = self.current_frame();
        let frames = ids.iter().flat_map(|id| self.key_frames(*id));
        let target = if forward {
            frames.filter(|f| *f > now).min()
        } else {
            frames.filter(|f| *f < now).max()
        };
        match target {
            Some(f) => self.go_to(f),
            None => self.set_status(StatusKind::Info, "No more keyframes"),
        }
    }

    /// Key every transform channel and opacity of the selected layers at
    /// the current frame, holding their current values.
    fn add_key(&mut self) {
        let roots = self.selected_roots();
        if roots.is_empty() {
            return;
        }
        self.checkpoint("Add keyframe");
        let frame = self.current_frame();
        for id in roots {
            if let Some(l) = self.doc.find_mut(id) {
                key_now(&mut l.transform.position, frame);
                key_now(&mut l.transform.rotation, frame);
                key_now(&mut l.transform.scale, frame);
                key_now(&mut l.opacity, frame);
            }
        }
    }

    /// Remove keys on the current frame from the selected layers,
    /// including their bones.
    fn delete_key(&mut self) {
        let roots = self.selected_roots();
        let frame = self.current_frame();
        let any = roots.iter().any(|id| self.key_frames(*id).contains(&frame));
        if !any {
            self.set_status(StatusKind::Info, "No keyframe on this frame");
            return;
        }
        self.checkpoint("Delete keyframe");
        for id in roots {
            if let Some(l) = self.doc.find_mut(id) {
                l.transform.position.remove_key(frame);
                l.transform.rotation.remove_key(frame);
                l.transform.scale.remove_key(frame);
                l.opacity.remove_key(frame);
                if let LayerKind::Bone { skeleton, .. } = &mut l.kind {
                    for b in &mut skeleton.bones {
                        b.angle.remove_key(frame);
                    }
                }
            }
        }
    }

    /// A layer's transform at the current frame.
    pub fn layer_transform(&self, id: LayerId) -> Option<Transform> {
        self.doc.find(id).map(|l| l.transform.sample(self.frame))
    }

    /// Change a layer's transform at the current frame, with auto-keying:
    /// a channel that is already animated gets a keyframe here; a channel
    /// that is not animated has its value changed for the whole animation.
    /// Does not checkpoint; callers checkpoint once per gesture.
    pub fn set_layer_transform(&mut self, id: LayerId, t: Transform) {
        let frame = self.current_frame();
        let Some(l) = self.doc.find_mut(id) else { return };
        let old = l.transform.sample(frame as f32);
        if old.position != t.position {
            write_channel(&mut l.transform.position, frame, t.position);
        }
        if old.rotation != t.rotation {
            write_channel(&mut l.transform.rotation, frame, t.rotation);
        }
        if old.scale != t.scale {
            write_channel(&mut l.transform.scale, frame, t.scale);
        }
        self.dirty = true;
    }

    pub fn layer_opacity(&self, id: LayerId) -> Option<f32> {
        self.doc.find(id).map(|l| l.opacity.sample(self.frame))
    }

    /// Like [`set_layer_transform`](Self::set_layer_transform), for opacity.
    pub fn set_layer_opacity(&mut self, id: LayerId, opacity: f32) {
        let frame = self.current_frame();
        if let Some(l) = self.doc.find_mut(id) {
            write_channel(&mut l.opacity, frame, opacity.clamp(0.0, 1.0));
            self.dirty = true;
        }
    }
}

enum NewKind {
    Vector,
    Group,
    Bone,
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn write_channel<T: Animatable>(ch: &mut Channel<T>, frame: Frame, value: T) {
    if ch.is_animated() {
        ch.set_key(frame, value, Interpolation::Smooth);
    } else {
        ch.base = value;
    }
}

fn key_now<T: Animatable>(ch: &mut Channel<T>, frame: Frame) {
    let v = ch.sample(frame as f32);
    ch.set_key(frame, v, Interpolation::Smooth);
}

#[cfg(test)]
mod tests {
    use super::*;
    use fanimate_core::Vec2;

    /// Editor with layers (bottom to top) A, B, C, all selected none.
    fn three() -> (Editor, [LayerId; 3]) {
        let mut doc = Document::new("t", 100.0, 100.0, 24.0);
        let ids = ["A", "B", "C"].map(|n| {
            let l = doc.new_layer(n, LayerKind::vector(vec![]));
            let id = l.id;
            doc.layers.push(l);
            id
        });
        (Editor::new(doc), ids)
    }

    fn names(ed: &Editor) -> Vec<String> {
        ed.doc.layers.iter().map(|l| l.name.clone()).collect()
    }

    #[test]
    fn new_layer_goes_above_selection_and_undoes() {
        let (mut ed, [a, ..]) = three();
        ed.select(a, SelectMode::Replace);
        ed.run(Action::NewVectorLayer);
        assert_eq!(names(&ed), ["A", "Layer 1", "B", "C"]);
        assert_eq!(ed.rename_request, ed.primary());
        assert!(ed.dirty);
        ed.run(Action::Undo);
        assert_eq!(names(&ed), ["A", "B", "C"]);
        assert_eq!(ed.selection, vec![a]);
        ed.run(Action::Redo);
        assert_eq!(names(&ed), ["A", "Layer 1", "B", "C"]);
    }

    #[test]
    fn copy_paste_through_text_gives_fresh_ids() {
        let (mut ed, [_, b, _]) = three();
        ed.select(b, SelectMode::Replace);
        let Outcome::CopyText(text) = ed.run(Action::Copy) else {
            panic!()
        };
        // paste into a different editor, as between two windows
        let (mut other, [.., c]) = three();
        other.select(c, SelectMode::Replace);
        assert!(other.paste(Some(&text)));
        assert_eq!(names(&other), ["A", "B", "C", "B"]);
        let mut ids = Vec::new();
        other.doc.walk(|l| ids.push(l.id));
        let unique: HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
        assert!(fanimate_fnm::validate(&other.doc).is_clean());
    }

    #[test]
    fn paste_ignores_foreign_text_and_falls_back_to_internal_clipboard() {
        let (mut ed, [a, ..]) = three();
        assert!(!ed.paste(Some("hello")));
        ed.select(a, SelectMode::Replace);
        ed.run(Action::Copy);
        assert!(ed.paste(Some("not layers")));
        assert_eq!(names(&ed), ["A", "A", "B", "C"]);
    }

    #[test]
    fn cut_removes_and_paste_restores() {
        let (mut ed, [a, b, _]) = three();
        ed.select(a, SelectMode::Replace);
        ed.select(b, SelectMode::Toggle);
        ed.run(Action::Cut);
        assert_eq!(names(&ed), ["C"]);
        ed.run(Action::Paste);
        assert_eq!(names(&ed), ["C", "A", "B"]);
    }

    #[test]
    fn delete_and_duplicate() {
        let (mut ed, [a, b, _]) = three();
        ed.select(b, SelectMode::Replace);
        ed.run(Action::Duplicate);
        assert_eq!(names(&ed), ["A", "B", "B copy", "C"]);
        ed.select(a, SelectMode::Replace);
        ed.run(Action::Delete);
        assert_eq!(names(&ed), ["B", "B copy", "C"]);
        assert!(ed.selection.is_empty());
    }

    #[test]
    fn move_up_and_down_stop_at_the_ends() {
        let (mut ed, [a, _, c]) = three();
        ed.select(a, SelectMode::Replace);
        ed.run(Action::MoveLayerUp);
        assert_eq!(names(&ed), ["B", "A", "C"]);
        ed.select(c, SelectMode::Replace);
        ed.run(Action::MoveLayerUp);
        assert_eq!(names(&ed), ["B", "A", "C"]);
        ed.run(Action::MoveLayerDown);
        assert_eq!(names(&ed), ["B", "C", "A"]);
    }

    #[test]
    fn group_then_ungroup_round_trips() {
        let (mut ed, [a, b, _]) = three();
        ed.select(a, SelectMode::Replace);
        ed.select(b, SelectMode::Toggle);
        ed.run(Action::GroupLayers);
        assert_eq!(names(&ed), ["Group 1", "C"]);
        let g = ed.primary().unwrap();
        let LayerKind::Group { children } = &ed.doc.find(g).unwrap().kind else {
            panic!()
        };
        assert_eq!(
            children.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
            ["A", "B"]
        );
        ed.run(Action::Ungroup);
        assert_eq!(names(&ed), ["A", "B", "C"]);
        assert!(fanimate_fnm::validate(&ed.doc).is_clean());
    }

    #[test]
    fn range_select_follows_display_order() {
        let (mut ed, [a, _, c]) = three();
        ed.select(c, SelectMode::Replace);
        ed.select(a, SelectMode::Range);
        assert_eq!(ed.selection.len(), 3);
        assert_eq!(ed.primary(), Some(a));
        ed.select(a, SelectMode::Toggle);
        assert_eq!(ed.selection.len(), 2);
    }

    #[test]
    fn display_rows_list_top_first_and_hide_collapsed() {
        let (mut ed, [a, b, _]) = three();
        ed.select(a, SelectMode::Replace);
        ed.select(b, SelectMode::Toggle);
        ed.run(Action::GroupLayers);
        let g = ed.primary().unwrap();
        let rows: Vec<_> = ed
            .display_rows()
            .iter()
            .map(|(id, d)| (ed.doc.find(*id).unwrap().name.clone(), *d))
            .collect();
        assert_eq!(
            rows,
            [
                ("C".into(), 0),
                ("Group 1".into(), 0),
                ("B".into(), 1),
                ("A".into(), 1)
            ]
        );
        ed.collapsed.insert(g);
        assert_eq!(ed.display_rows().len(), 2);
    }

    #[test]
    fn auto_key_only_keys_animated_channels() {
        let (mut ed, [a, ..]) = three();
        ed.go_to(10);
        let mut t = ed.layer_transform(a).unwrap();
        t.position = Vec2::new(5.0, 0.0);
        ed.set_layer_transform(a, t);
        // not animated: the base value changes, no key
        let l = ed.doc.find(a).unwrap();
        assert!(!l.transform.position.is_animated());
        assert_eq!(l.transform.position.base, Vec2::new(5.0, 0.0));

        ed.select(a, SelectMode::Replace);
        ed.run(Action::AddKey);
        ed.go_to(20);
        t.position = Vec2::new(50.0, 0.0);
        ed.set_layer_transform(a, t);
        assert_eq!(ed.key_frames(a), [10, 20]);
        ed.go_to(15);
        assert_eq!(ed.layer_transform(a).unwrap().position.x, 27.5);
    }

    #[test]
    fn key_navigation_and_deletion() {
        let (mut ed, [a, ..]) = three();
        ed.select(a, SelectMode::Replace);
        for f in [5, 12] {
            ed.go_to(f);
            ed.run(Action::AddKey);
        }
        ed.go_to(0);
        ed.run(Action::NextKey);
        assert_eq!(ed.current_frame(), 5);
        ed.run(Action::NextKey);
        assert_eq!(ed.current_frame(), 12);
        ed.run(Action::PrevKey);
        assert_eq!(ed.current_frame(), 5);
        ed.run(Action::DeleteKey);
        assert_eq!(ed.key_frames(a), [12]);
    }

    #[test]
    fn playback_loops_and_frames_clamp() {
        let (mut ed, _) = three();
        ed.doc.end = 23;
        ed.playing = true;
        ed.tick(1.5); // 36 frames at 24 fps over a 24-frame range
        assert_eq!(ed.current_frame(), 12);
        ed.go_to(500);
        assert_eq!(ed.current_frame(), 23);
        assert!(!ed.playing);
    }

    #[test]
    fn moho_a_picks_tool_by_layer_type() {
        let (mut ed, [a, ..]) = three();
        ed.select(a, SelectMode::Replace);
        ed.run(Action::AddPointOrBone);
        assert_eq!(ed.tool, Tool::AddPoint);
        ed.run(Action::NewBoneLayer);
        ed.run(Action::AddPointOrBone);
        assert_eq!(ed.tool, Tool::AddBone);
    }

    #[test]
    fn drag_and_drop_moves_layers() {
        let (mut ed, [a, b, c]) = three();
        assert!(ed.move_relative(&[a], c, Placement::Above));
        assert_eq!(names(&ed), ["B", "C", "A"]);
        ed.run(Action::NewGroupLayer);
        let g = ed.primary().unwrap();
        assert!(ed.move_relative(&[b, c], g, Placement::Into));
        let LayerKind::Group { children } = &ed.doc.find(g).unwrap().kind else {
            panic!()
        };
        assert_eq!(
            children.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(),
            ["B", "C"]
        );
        // a group cannot go inside itself, and a vector layer holds nothing
        assert!(!ed.move_relative(&[g], b, Placement::Above));
        assert!(!ed.move_relative(&[b], a, Placement::Into));
        assert!(fanimate_fnm::validate(&ed.doc).is_clean());
    }

    #[test]
    fn world_transforms_compose_parents() {
        let (mut ed, [a, b, _]) = three();
        ed.select(a, SelectMode::Replace);
        ed.select(b, SelectMode::Toggle);
        ed.run(Action::GroupLayers);
        let g = ed.primary().unwrap();
        ed.doc.find_mut(g).unwrap().transform.position.base = Vec2::new(10.0, 0.0);
        ed.doc.find_mut(a).unwrap().transform.position.base = Vec2::new(0.0, 5.0);
        let p = ed.world(a).transform_point2(Vec2::ZERO);
        assert!(p.abs_diff_eq(Vec2::new(10.0, 5.0), 1e-5));
        assert!(
            ed.parent_world(a)
                .transform_point2(Vec2::ZERO)
                .abs_diff_eq(Vec2::new(10.0, 0.0), 1e-5)
        );
    }

    #[test]
    fn opening_a_sample_is_left_to_the_app_and_closes_the_document() {
        let mut ed = Editor::untitled();
        let open = Action::OpenSample(crate::Sample::Hello);
        assert_eq!(ed.run(open), Outcome::App(open));
        // so the app asks about unsaved changes first, as for New and Open
        assert!(open.closes_document());
        assert!(Action::New.closes_document() && Action::Quit.closes_document());
        assert!(!Action::Save.closes_document());
    }

    #[test]
    fn revision_counts_edits_undos_and_redos() {
        let (mut ed, _) = three();
        let r0 = ed.revision();
        ed.run(Action::NewVectorLayer);
        assert_eq!(ed.revision(), r0 + 1);
        ed.run(Action::Undo);
        ed.run(Action::Redo);
        assert_eq!(ed.revision(), r0 + 3);
        // nothing to undo past the start: no change
        ed.run(Action::Undo);
        ed.run(Action::Undo);
        let r = ed.revision();
        ed.run(Action::Undo);
        assert_eq!(ed.revision(), r);
    }

    #[test]
    fn recovered_work_is_unsaved_and_keeps_its_path() {
        let ed = Editor::recovered(samples::hello(), Some(PathBuf::from("scene.fnm")));
        assert!(ed.dirty);
        assert_eq!(ed.path.as_deref(), Some(Path::new("scene.fnm")));
    }

    #[test]
    fn can_run_reflects_state() {
        let (mut ed, [a, ..]) = three();
        assert!(!ed.can_run(Action::Undo));
        assert!(!ed.can_run(Action::Copy));
        assert!(!ed.can_run(Action::Paste));
        ed.select(a, SelectMode::Replace);
        ed.run(Action::Copy);
        assert!(ed.can_run(Action::Paste));
        assert!(!ed.can_run(Action::Ungroup));
    }
}
