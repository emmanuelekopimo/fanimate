//! The scene graph: a document holds a tree of layers.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::Color;
use crate::anim::{Channel, Frame};
use crate::bones::Skeleton;
use crate::path::Shape;
use crate::transform::AnimTransform;

/// Stable identifier for a layer, unique within a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerId(pub u32);

/// A Fanimate project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub name: String,
    /// Canvas size in document units (pixels at 100% zoom).
    pub width: f32,
    pub height: f32,
    pub fps: f32,
    /// Inclusive frame range that plays and renders.
    pub start: Frame,
    pub end: Frame,
    pub background: Color,
    /// Top-level layers, drawn first to last (last is on top).
    pub layers: Vec<Layer>,
    /// Next unused layer id.
    pub next_layer_id: u32,
}

impl Document {
    pub fn new(name: impl Into<String>, width: f32, height: f32, fps: f32) -> Self {
        Self {
            name: name.into(),
            width,
            height,
            fps,
            start: 0,
            end: (fps * 4.0) as Frame - 1,
            background: Color::WHITE,
            layers: Vec::new(),
            next_layer_id: 1,
        }
    }

    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width, self.height)
    }

    pub fn frame_count(&self) -> u32 {
        (self.end - self.start + 1).max(0) as u32
    }

    /// Allocate a fresh layer id.
    pub fn new_layer_id(&mut self) -> LayerId {
        let id = LayerId(self.next_layer_id);
        self.next_layer_id += 1;
        id
    }

    /// Create a layer with a fresh id (not yet inserted anywhere).
    pub fn new_layer(&mut self, name: impl Into<String>, kind: LayerKind) -> Layer {
        Layer::new(self.new_layer_id(), name, kind)
    }

    /// Visit every layer in the tree, depth first, parents before children.
    pub fn walk(&self, mut f: impl FnMut(&Layer)) {
        fn go(layers: &[Layer], f: &mut impl FnMut(&Layer)) {
            for l in layers {
                f(l);
                if let Some(children) = l.kind.children() {
                    go(children, f);
                }
            }
        }
        go(&self.layers, &mut f);
    }

    /// Mutable version of [`walk`](Self::walk).
    pub fn walk_mut(&mut self, mut f: impl FnMut(&mut Layer)) {
        fn go(layers: &mut [Layer], f: &mut impl FnMut(&mut Layer)) {
            for l in layers {
                f(l);
                if let Some(children) = l.kind.children_mut() {
                    go(children, f);
                }
            }
        }
        go(&mut self.layers, &mut f);
    }

    pub fn layer_count(&self) -> usize {
        let mut n = 0;
        self.walk(|_| n += 1);
        n
    }

    pub fn find(&self, id: LayerId) -> Option<&Layer> {
        fn go(layers: &[Layer], id: LayerId) -> Option<&Layer> {
            for l in layers {
                if l.id == id {
                    return Some(l);
                }
                if let Some(found) = l.kind.children().and_then(|c| go(c, id)) {
                    return Some(found);
                }
            }
            None
        }
        go(&self.layers, id)
    }

    pub fn find_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        fn go(layers: &mut [Layer], id: LayerId) -> Option<&mut Layer> {
            for l in layers {
                if l.id == id {
                    return Some(l);
                }
                if let Some(found) = l.kind.children_mut().and_then(|c| go(c, id)) {
                    return Some(found);
                }
            }
            None
        }
        go(&mut self.layers, id)
    }

    /// Where a layer sits: its parent (`None` for top level) and its index
    /// in that parent's list.
    pub fn locate(&self, id: LayerId) -> Option<(Option<LayerId>, usize)> {
        fn go(layers: &[Layer], parent: Option<LayerId>, id: LayerId) -> Option<(Option<LayerId>, usize)> {
            for (i, l) in layers.iter().enumerate() {
                if l.id == id {
                    return Some((parent, i));
                }
                if let Some(found) = l.kind.children().and_then(|c| go(c, Some(l.id), id)) {
                    return Some(found);
                }
            }
            None
        }
        go(&self.layers, None, id)
    }

    /// The list that holds the children of `parent` (`None`: top level).
    pub fn siblings_mut(&mut self, parent: Option<LayerId>) -> Option<&mut Vec<Layer>> {
        match parent {
            None => Some(&mut self.layers),
            Some(p) => self.find_mut(p)?.kind.children_mut(),
        }
    }

    /// Remove a layer (with its children) from the tree.
    pub fn remove(&mut self, id: LayerId) -> Option<Layer> {
        let (parent, index) = self.locate(id)?;
        Some(self.siblings_mut(parent)?.remove(index))
    }

    /// Insert a layer into `parent` at `index` (clamped to the end).
    /// Returns false if `parent` does not exist or cannot hold layers.
    pub fn insert(&mut self, parent: Option<LayerId>, index: usize, layer: Layer) -> bool {
        match self.siblings_mut(parent) {
            Some(list) => {
                let index = index.min(list.len());
                list.insert(index, layer);
                true
            }
            None => false,
        }
    }

    /// Whether `id` is `ancestor` or inside it.
    pub fn is_within(&self, id: LayerId, ancestor: LayerId) -> bool {
        let mut current = Some(id);
        while let Some(c) = current {
            if c == ancestor {
                return true;
            }
            current = self.locate(c).and_then(|(p, _)| p);
        }
        false
    }

    /// Give `layer` and everything inside it fresh ids from this document,
    /// for pasting or duplicating.
    pub fn reassign_ids(&mut self, layer: &mut Layer) {
        layer.id = self.new_layer_id();
        if let Some(children) = layer.kind.children_mut() {
            for child in children {
                self.reassign_ids(child);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub visible: bool,
    pub opacity: Channel<f32>,
    pub transform: AnimTransform,
    #[serde(flatten)]
    pub kind: LayerKind,
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

impl Layer {
    pub fn new(id: LayerId, name: impl Into<String>, kind: LayerKind) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            opacity: Channel::new(1.0),
            transform: AnimTransform::default(),
            kind,
        }
    }

    pub fn with_transform(mut self, transform: AnimTransform) -> Self {
        self.transform = transform;
        self
    }
}

/// What a layer contains. Matches Moho's layer types where they overlap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LayerKind {
    /// Vector shapes.
    Vector { shapes: Vec<Shape> },
    /// A folder of layers that move together.
    Group { children: Vec<Layer> },
    /// A skeleton; shapes in child vector layers can bind to its bones.
    Bone {
        skeleton: Skeleton,
        children: Vec<Layer>,
    },
}

impl LayerKind {
    pub fn vector(shapes: Vec<Shape>) -> Self {
        Self::Vector { shapes }
    }

    pub fn children(&self) -> Option<&Vec<Layer>> {
        match self {
            Self::Vector { .. } => None,
            Self::Group { children } | Self::Bone { children, .. } => Some(children),
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<Layer>> {
        match self {
            Self::Vector { .. } => None,
            Self::Group { children } | Self::Bone { children, .. } => Some(children),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// top: [a, g: [b, c]]
    fn tree() -> (Document, [LayerId; 4]) {
        let mut doc = Document::new("t", 10.0, 10.0, 24.0);
        let a = doc.new_layer("a", LayerKind::vector(vec![]));
        let b = doc.new_layer("b", LayerKind::vector(vec![]));
        let c = doc.new_layer("c", LayerKind::vector(vec![]));
        let ids = [a.id, b.id, c.id];
        let g = doc.new_layer("g", LayerKind::Group { children: vec![b, c] });
        let gid = g.id;
        doc.layers = vec![a, g];
        (doc, [ids[0], gid, ids[1], ids[2]])
    }

    #[test]
    fn find_and_locate_nested_layers() {
        let (doc, [a, g, b, c]) = tree();
        assert_eq!(doc.find(c).unwrap().name, "c");
        assert_eq!(doc.locate(a), Some((None, 0)));
        assert_eq!(doc.locate(c), Some((Some(g), 1)));
        assert!(doc.is_within(b, g));
        assert!(!doc.is_within(a, g));
        assert!(doc.find(LayerId(999)).is_none());
    }

    #[test]
    fn remove_and_insert_move_layers() {
        let (mut doc, [a, g, _, c]) = tree();
        let layer = doc.remove(c).unwrap();
        assert!(doc.insert(None, 0, layer));
        assert_eq!(doc.locate(c), Some((None, 0)));
        assert_eq!(doc.locate(a), Some((None, 1)));
        // vector layers cannot hold children
        let layer = doc.remove(c).unwrap();
        assert!(!doc.insert(Some(a), 0, layer.clone()));
        assert!(doc.insert(Some(g), 99, layer));
        assert_eq!(doc.locate(c), Some((Some(g), 1)));
    }

    #[test]
    fn reassign_ids_renumbers_subtree() {
        let (mut doc, [_, g, ..]) = tree();
        let mut copy = doc.find(g).unwrap().clone();
        doc.reassign_ids(&mut copy);
        assert_ne!(copy.id, g);
        let mut ids = Vec::new();
        doc.layers.push(copy);
        doc.walk(|l| ids.push(l.id));
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
    }
}
