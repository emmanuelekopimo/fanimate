//! Animation channels: a value that can change over time through keyframes.

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::Color;

/// A frame number on the timeline. Frame 0 is the first frame; keyframes on
/// negative frames are invalid (see the validator in `fanimate-fnm`).
pub type Frame = i32;

/// How a channel moves from one keyframe to the next.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interpolation {
    /// Hold the value until the next key.
    Step,
    /// Straight-line change.
    Linear,
    /// Ease in and out of each key.
    #[default]
    Smooth,
}

impl Interpolation {
    pub const ALL: [Interpolation; 3] = [Self::Smooth, Self::Linear, Self::Step];

    pub fn label(self) -> &'static str {
        match self {
            Self::Smooth => "Smooth",
            Self::Linear => "Linear",
            Self::Step => "Step",
        }
    }

    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Remap linear progress `t` (0..=1) through this interpolation.
    pub fn apply(self, t: f32) -> f32 {
        match self {
            Self::Step => 0.0,
            Self::Linear => t,
            Self::Smooth => t * t * (3.0 - 2.0 * t),
        }
    }
}

/// A value that can be interpolated between keyframes.
pub trait Animatable: Copy + PartialEq {
    fn lerp(a: Self, b: Self, t: f32) -> Self;

    /// Whether every component is a finite number.
    fn is_finite(&self) -> bool;
}

impl Animatable for f32 {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t
    }
    fn is_finite(&self) -> bool {
        f32::is_finite(*self)
    }
}

impl Animatable for Vec2 {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a.lerp(b, t)
    }
    fn is_finite(&self) -> bool {
        Vec2::is_finite(*self)
    }
}

impl Animatable for Color {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        Color::lerp(a, b, t)
    }
    fn is_finite(&self) -> bool {
        [self.r, self.g, self.b, self.a].iter().all(|c| c.is_finite())
    }
}

/// One keyframe. `interp` controls the motion from this key to the next.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Key<T> {
    pub frame: Frame,
    pub value: T,
    #[serde(default, skip_serializing_if = "Interpolation::is_default")]
    pub interp: Interpolation,
}

/// An animatable value.
///
/// `base` is the value when there are no keys. For bones it is also the rest
/// pose. Keys are kept sorted by frame with at most one key per frame; the
/// editing methods maintain that, and the file validator checks it on load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Channel<T> {
    pub base: T,
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<Key<T>>,
}

impl<T: Animatable> Channel<T> {
    pub fn new(base: T) -> Self {
        Self {
            base,
            keys: Vec::new(),
        }
    }

    pub fn is_animated(&self) -> bool {
        !self.keys.is_empty()
    }

    /// Insert a key, replacing any existing key on the same frame.
    pub fn set_key(&mut self, frame: Frame, value: T, interp: Interpolation) {
        let key = Key { frame, value, interp };
        match self.keys.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => self.keys[i] = key,
            Err(i) => self.keys.insert(i, key),
        }
    }

    /// Builder form of [`set_key`](Self::set_key).
    pub fn with_key(mut self, frame: Frame, value: T, interp: Interpolation) -> Self {
        self.set_key(frame, value, interp);
        self
    }

    pub fn remove_key(&mut self, frame: Frame) -> Option<Key<T>> {
        let i = self.keys.binary_search_by_key(&frame, |k| k.frame).ok()?;
        Some(self.keys.remove(i))
    }

    /// The value at `frame`, which may fall between whole frames.
    pub fn sample(&self, frame: f32) -> T {
        let keys = &self.keys;
        let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
            return self.base;
        };
        if frame <= first.frame as f32 {
            return first.value;
        }
        if frame >= last.frame as f32 {
            return last.value;
        }
        // index of the first key after `frame`; always 1..len here
        let next = keys.partition_point(|k| k.frame as f32 <= frame);
        let (a, b) = (&keys[next - 1], &keys[next]);
        let t = (frame - a.frame as f32) / (b.frame - a.frame) as f32;
        T::lerp(a.value, b.value, a.interp.apply(t))
    }
}

/// Keyframe editing that does not depend on the value type, so the timeline
/// can retime or change the interpolation of every channel of a layer
/// alike: numbers, vectors, colours and point keys.
pub trait KeyList {
    /// Push the frame of every key onto `out`.
    fn frames_into(&self, out: &mut Vec<Frame>);

    /// The interpolation of the key on `frame`, if there is one.
    fn interp_at(&self, frame: Frame) -> Option<Interpolation>;

    /// Move the key on `from` to `to`, replacing any key already there.
    /// Returns whether there was a key to move.
    fn move_key(&mut self, from: Frame, to: Frame) -> bool;

    /// Set the interpolation of the key on `frame`. Returns whether it
    /// changed.
    fn set_interp(&mut self, frame: Frame, interp: Interpolation) -> bool;

    /// Remove the key on `frame`. Returns whether there was one.
    fn delete_key(&mut self, frame: Frame) -> bool;
}

impl<T> KeyList for Vec<Key<T>> {
    fn frames_into(&self, out: &mut Vec<Frame>) {
        out.extend(self.iter().map(|k| k.frame));
    }

    fn interp_at(&self, frame: Frame) -> Option<Interpolation> {
        let i = self.binary_search_by_key(&frame, |k| k.frame).ok()?;
        Some(self[i].interp)
    }

    fn move_key(&mut self, from: Frame, to: Frame) -> bool {
        let Ok(i) = self.binary_search_by_key(&from, |k| k.frame) else {
            return false;
        };
        let mut key = self.remove(i);
        key.frame = to;
        match self.binary_search_by_key(&to, |k| k.frame) {
            Ok(j) => self[j] = key,
            Err(j) => self.insert(j, key),
        }
        true
    }

    fn set_interp(&mut self, frame: Frame, interp: Interpolation) -> bool {
        match self.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) if self[i].interp != interp => {
                self[i].interp = interp;
                true
            }
            _ => false,
        }
    }

    fn delete_key(&mut self, frame: Frame) -> bool {
        match self.binary_search_by_key(&frame, |k| k.frame) {
            Ok(i) => {
                self.remove(i);
                true
            }
            Err(_) => false,
        }
    }
}

impl<T: Animatable + Default> Default for Channel<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp() -> Channel<f32> {
        Channel::new(5.0)
            .with_key(10, 0.0, Interpolation::Linear)
            .with_key(20, 100.0, Interpolation::Linear)
    }

    #[test]
    fn unkeyed_channel_returns_base() {
        assert_eq!(Channel::new(5.0_f32).sample(42.0), 5.0);
    }

    #[test]
    fn holds_before_first_and_after_last_key() {
        let c = ramp();
        assert_eq!(c.sample(0.0), 0.0);
        assert_eq!(c.sample(99.0), 100.0);
    }

    #[test]
    fn linear_and_subframe_sampling() {
        let c = ramp();
        assert_eq!(c.sample(15.0), 50.0);
        assert_eq!(c.sample(12.5), 25.0);
    }

    #[test]
    fn step_holds_until_next_key() {
        let c =
            Channel::new(0.0)
                .with_key(0, 1.0, Interpolation::Step)
                .with_key(10, 2.0, Interpolation::Step);
        assert_eq!(c.sample(9.9), 1.0);
        assert_eq!(c.sample(10.0), 2.0);
    }

    #[test]
    fn smooth_eases_but_hits_midpoint() {
        let c = Channel::new(0.0)
            .with_key(0, 0.0, Interpolation::Smooth)
            .with_key(10, 1.0, Interpolation::Smooth);
        assert!(c.sample(1.0) < 0.1);
        assert!((c.sample(5.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn set_key_keeps_keys_sorted_and_unique() {
        let mut c = ramp();
        c.set_key(15, 1.0, Interpolation::Linear);
        c.set_key(15, 2.0, Interpolation::Linear);
        c.set_key(5, 3.0, Interpolation::Linear);
        let frames: Vec<_> = c.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, [5, 10, 15, 20]);
        assert_eq!(c.sample(15.0), 2.0);
        assert!(c.remove_key(15).is_some());
        assert!(c.remove_key(15).is_none());
    }

    #[test]
    fn key_list_moves_keys_replacing_any_on_the_target() {
        let mut c = ramp();
        assert!(c.keys.move_key(10, 25));
        let frames: Vec<_> = c.keys.iter().map(|k| (k.frame, k.value)).collect();
        assert_eq!(frames, [(20, 100.0), (25, 0.0)]);
        assert!(c.keys.move_key(25, 20));
        assert_eq!(c.keys.len(), 1);
        assert_eq!(c.keys[0].value, 0.0);
        assert!(!c.keys.move_key(3, 4));
        assert!(c.keys.set_interp(20, Interpolation::Step));
        assert!(!c.keys.set_interp(20, Interpolation::Step));
        assert_eq!(c.keys.interp_at(20), Some(Interpolation::Step));
        assert!(c.keys.delete_key(20));
        assert!(!c.is_animated());
    }
}
