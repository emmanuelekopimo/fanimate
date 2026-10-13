//! Frame-by-frame animation: a layer of drawings, each shown from its frame
//! until the next one (its exposure), as on paper or in an X-sheet.
//!
//! Each drawing has sub-layers, as in traditional ink and paint: a rough
//! sketch under everything, colour, line art over the colour, and an
//! overlay on top. Roughs are for the animator only and are left out of
//! renders.

use serde::{Deserialize, Serialize};

use crate::anim::{Frame, Interpolation, KeyList};
use crate::path::Shape;

/// A drawing's sub-layers, bottom to top.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Part {
    Rough,
    Colour,
    #[default]
    Line,
    Overlay,
}

impl Part {
    /// Drawn in this order.
    pub const ALL: [Part; 4] = [Self::Rough, Self::Colour, Self::Line, Self::Overlay];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rough => "Rough",
            Self::Colour => "Colour",
            Self::Line => "Line",
            Self::Overlay => "Overlay",
        }
    }
}

/// One drawing, exposed from `frame` until the next drawing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cel {
    pub frame: Frame,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rough: Vec<Shape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colour: Vec<Shape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line: Vec<Shape>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlay: Vec<Shape>,
}

impl Cel {
    pub fn new(frame: Frame) -> Self {
        Self {
            frame,
            ..Self::default()
        }
    }

    pub fn part(&self, part: Part) -> &Vec<Shape> {
        match part {
            Part::Rough => &self.rough,
            Part::Colour => &self.colour,
            Part::Line => &self.line,
            Part::Overlay => &self.overlay,
        }
    }

    pub fn part_mut(&mut self, part: Part) -> &mut Vec<Shape> {
        match part {
            Part::Rough => &mut self.rough,
            Part::Colour => &mut self.colour,
            Part::Line => &mut self.line,
            Part::Overlay => &mut self.overlay,
        }
    }

    /// Every shape with its sub-layer, bottom to top.
    pub fn shapes(&self) -> impl Iterator<Item = (Part, usize, &Shape)> {
        Part::ALL
            .into_iter()
            .flat_map(move |p| self.part(p).iter().enumerate().map(move |(i, s)| (p, i, s)))
    }

    /// Every shape, mutably.
    pub fn shapes_mut(&mut self) -> impl Iterator<Item = &mut Shape> {
        self.rough
            .iter_mut()
            .chain(&mut self.colour)
            .chain(&mut self.line)
            .chain(&mut self.overlay)
    }

    pub fn is_empty(&self) -> bool {
        Part::ALL.iter().all(|p| self.part(*p).is_empty())
    }
}

/// The index of the drawing shown on `frame`: the last one starting at or
/// before it. Nothing is shown before the first drawing.
pub fn cel_at(cels: &[Cel], frame: f32) -> Option<usize> {
    let n = cels.partition_point(|c| c.frame as f32 <= frame);
    n.checked_sub(1)
}

/// Drawings retime like keys: dragging one on the timeline changes when
/// it is shown. They hold until the next, so they have no interpolation.
impl KeyList for Vec<Cel> {
    fn frames_into(&self, out: &mut Vec<Frame>) {
        out.extend(self.iter().map(|c| c.frame));
    }

    fn interp_at(&self, _frame: Frame) -> Option<Interpolation> {
        None
    }

    fn move_key(&mut self, from: Frame, to: Frame) -> bool {
        let Ok(i) = self.binary_search_by_key(&from, |c| c.frame) else {
            return false;
        };
        let mut cel = self.remove(i);
        cel.frame = to;
        match self.binary_search_by_key(&to, |c| c.frame) {
            Ok(j) => self[j] = cel,
            Err(j) => self.insert(j, cel),
        }
        true
    }

    fn set_interp(&mut self, _frame: Frame, _interp: Interpolation) -> bool {
        false
    }

    fn delete_key(&mut self, frame: Frame) -> bool {
        match self.binary_search_by_key(&frame, |c| c.frame) {
            Ok(i) => {
                self.remove(i);
                true
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cels() -> Vec<Cel> {
        vec![Cel::new(0), Cel::new(4), Cel::new(10)]
    }

    #[test]
    fn each_drawing_holds_until_the_next() {
        let c = cels();
        assert_eq!(cel_at(&c, 0.0), Some(0));
        assert_eq!(cel_at(&c, 3.9), Some(0));
        assert_eq!(cel_at(&c, 4.0), Some(1));
        assert_eq!(cel_at(&c, 99.0), Some(2));
        assert_eq!(cel_at(&c[1..], 2.0), None);
    }

    #[test]
    fn drawings_retime_like_keys() {
        let mut c = cels();
        assert!(c.move_key(4, 12));
        let frames: Vec<_> = c.iter().map(|c| c.frame).collect();
        assert_eq!(frames, [0, 10, 12]);
        assert!(c.delete_key(10));
        assert_eq!(c.interp_at(0), None);
    }
}
