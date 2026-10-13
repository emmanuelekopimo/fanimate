//! The Fanimate editing model.
//!
//! Everything the app does to a document lives here, independent of the UI
//! toolkit: selection, editing commands, undo and redo, the layer clipboard,
//! tools, and keyboard shortcut maps. The app crate only draws widgets and
//! turns clicks and key presses into calls on [`Editor`]. Keeping this
//! separate means every command is unit-tested without opening a window.

mod action;
mod editor;
mod history;
mod keymap;

pub use action::{Action, Sample, Tool, ToolGroup};
pub use editor::{BoneRef, DrawStyle, Editor, Outcome, Placement, PointRef, SelectMode, StatusKind};
pub use history::History;
pub use keymap::{Chord, Keymap, KeymapKind};
