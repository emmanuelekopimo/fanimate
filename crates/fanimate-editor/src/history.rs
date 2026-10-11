//! Undo and redo.
//!
//! The spike stores whole-document snapshots: simple and impossible to get
//! subtly wrong, and cheap at current document sizes (the 40-bone benchmark
//! clones in microseconds). Milestone M2 replaces this with per-object
//! history that survives a crash (product vision, feature 5).

use fanimate_core::{Document, LayerId};

#[derive(Clone)]
struct Snapshot {
    label: &'static str,
    doc: Document,
    selection: Vec<LayerId>,
}

pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(200)
    }
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Record the state *before* an edit called `label`.
    pub fn push(&mut self, label: &'static str, doc: &Document, selection: &[LayerId]) {
        self.redo.clear();
        self.undo.push(Snapshot {
            label,
            doc: doc.clone(),
            selection: selection.to_vec(),
        });
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    /// Step back. `doc` and `selection` are swapped with the saved state.
    pub fn undo(&mut self, doc: &mut Document, selection: &mut Vec<LayerId>) -> Option<&'static str> {
        let snap = self.undo.pop()?;
        let label = snap.label;
        self.redo.push(Snapshot {
            label,
            doc: std::mem::replace(doc, snap.doc),
            selection: std::mem::replace(selection, snap.selection),
        });
        Some(label)
    }

    pub fn redo(&mut self, doc: &mut Document, selection: &mut Vec<LayerId>) -> Option<&'static str> {
        let snap = self.redo.pop()?;
        let label = snap.label;
        self.undo.push(Snapshot {
            label,
            doc: std::mem::replace(doc, snap.doc),
            selection: std::mem::replace(selection, snap.selection),
        });
        Some(label)
    }

    pub fn undo_label(&self) -> Option<&'static str> {
        self.undo.last().map(|s| s.label)
    }

    pub fn redo_label(&self) -> Option<&'static str> {
        self.redo.last().map(|s| s.label)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
