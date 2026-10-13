//! Images and their media files.
//!
//! An image layer shows a picture, such as one layer of an imported
//! Photoshop file. The picture's file (a PNG) lives in the document's media
//! store under a name made from its content, so the same picture is stored
//! once however many layers use it, and a name never changes meaning.
//! The engine keeps the encoded file; decoding is the renderer's job.

use std::sync::Arc;

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::bones::Skin;

/// An encoded image file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaFile {
    pub bytes: Vec<u8>,
}

impl MediaFile {
    /// The content-derived name to store `bytes` under, with `ext` (such as
    /// "png"): stable across sessions and machines.
    pub fn name_for(bytes: &[u8], ext: &str) -> String {
        // FNV-1a, 64-bit: no dependency, and plenty to tell files apart
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("media/{h:016x}-{}.{ext}", bytes.len())
    }
}

/// The document's media files by name.
pub type Media = std::collections::BTreeMap<String, Arc<MediaFile>>;

/// An image layer's picture, and how bones bend it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Image {
    /// The media file's name in the document's media store.
    pub media: String,
    /// Size in document units; the image spans (0, 0) to (width, height) in
    /// the layer's space.
    pub width: f32,
    pub height: f32,
    /// The mesh it bends with: columns and rows of cells. 1 by 1 is a plain
    /// rectangle.
    #[serde(default = "one_cell", skip_serializing_if = "is_one_cell")]
    pub grid: [u32; 2],
    /// Bone it follows rigidly, as for shapes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bone: Option<usize>,
    /// Weights per mesh vertex, row by row, as for shapes' `skin`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<Skin>,
}

fn one_cell() -> [u32; 2] {
    [1, 1]
}

fn is_one_cell(g: &[u32; 2]) -> bool {
    *g == [1, 1]
}

/// The most cells a mesh may have each way.
pub const MAX_GRID: u32 = 64;

impl Image {
    pub fn new(media: impl Into<String>, width: f32, height: f32) -> Self {
        Self {
            media: media.into(),
            width,
            height,
            grid: one_cell(),
            bone: None,
            skin: None,
        }
    }

    /// Columns and rows of cells, kept within sensible bounds.
    pub fn cells(&self) -> [u32; 2] {
        [self.grid[0].clamp(1, MAX_GRID), self.grid[1].clamp(1, MAX_GRID)]
    }

    /// The mesh vertices in the layer's space, row by row, with their
    /// texture coordinates.
    pub fn vertices(&self) -> Vec<(Vec2, Vec2)> {
        let [cols, rows] = self.cells();
        let mut out = Vec::with_capacity(((cols + 1) * (rows + 1)) as usize);
        for r in 0..=rows {
            for c in 0..=cols {
                let uv = Vec2::new(c as f32 / cols as f32, r as f32 / rows as f32);
                out.push((uv * Vec2::new(self.width, self.height), uv));
            }
        }
        out
    }

    /// The smooth binding, if it has one weight list per vertex.
    pub fn valid_skin(&self) -> Option<&Skin> {
        let [c, r] = self.cells();
        self.skin
            .as_ref()
            .filter(|s| s.len() == ((c + 1) * (r + 1)) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_content() {
        let a = MediaFile::name_for(b"one", "png");
        assert_eq!(a, MediaFile::name_for(b"one", "png"));
        assert_ne!(a, MediaFile::name_for(b"two", "png"));
        assert!(a.starts_with("media/") && a.ends_with(".png"));
    }

    #[test]
    fn mesh_vertices_cover_the_image() {
        let mut img = Image::new("m", 100.0, 50.0);
        img.grid = [2, 1];
        let v = img.vertices();
        assert_eq!(v.len(), 6);
        assert_eq!(v[5], (Vec2::new(100.0, 50.0), Vec2::ONE));
        img.grid = [0, 999];
        assert_eq!(img.cells(), [1, MAX_GRID]);
    }
}
