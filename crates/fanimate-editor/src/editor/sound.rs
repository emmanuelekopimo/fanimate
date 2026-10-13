//! Sound and lip sync: WAV audio layers, switch layers, and turning a
//! line of dialogue into mouth shapes.
//!
//! Lip sync uses Rhubarb Lip Sync's mouth shapes. The app runs Rhubarb on
//! the sound; this module reads its result and keys a switch layer whose
//! children are the mouths, named after Rhubarb's letters (A to H, X) or
//! the Preston Blair names many mouth sets use (MBP, etc, E, AI, O, U, FV,
//! L, rest).

use std::sync::Arc;

use fanimate_core::{Audio, Frame, Interpolation, Key, LayerId, LayerKind, MediaFile};
use serde::Deserialize;

use super::{Editor, StatusKind, labelled};

/// A sound file's length and layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundInfo {
    pub seconds: f32,
    pub channels: u16,
    pub sample_rate: u32,
}

/// Read a WAV file's header. Errors are for showing the user.
pub fn sound_info(bytes: &[u8]) -> Result<SoundInfo, String> {
    let reader = hound::WavReader::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("not a WAV file Fanimate can read: {e}"))?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err("the WAV file has no sound in it".into());
    }
    let frames = reader.duration();
    Ok(SoundInfo {
        seconds: frames as f32 / spec.sample_rate as f32,
        channels: spec.channels,
        sample_rate: spec.sample_rate,
    })
}

/// One mouth shape from Rhubarb, starting `start` seconds into the sound.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct MouthCue {
    pub start: f32,
    pub value: String,
}

#[derive(Deserialize)]
struct RhubarbOutput {
    #[serde(rename = "mouthCues")]
    mouth_cues: Vec<MouthCue>,
}

/// Read Rhubarb Lip Sync's JSON output (`rhubarb -f json`).
pub fn parse_rhubarb(json: &str) -> Result<Vec<MouthCue>, String> {
    serde_json::from_str::<RhubarbOutput>(json)
        .map(|o| o.mouth_cues)
        .map_err(|e| format!("Rhubarb's result could not be read: {e}"))
}

/// The layer names that can stand for one of Rhubarb's mouth shapes.
fn mouth_names(shape: &str) -> &'static [&'static str] {
    match shape.to_ascii_uppercase().as_str() {
        "A" => &["A", "MBP"],
        "B" => &["B", "etc"],
        "C" => &["C", "E"],
        "D" => &["D", "AI"],
        "E" => &["E", "O"],
        "F" => &["F", "U", "WQ"],
        "G" => &["G", "FV"],
        "H" => &["H", "L"],
        "X" => &["X", "rest"],
        _ => &[],
    }
}

impl Editor {
    /// Import a WAV file as an audio layer starting on the current frame.
    /// One undo step.
    pub fn import_audio(&mut self, name: &str, bytes: Vec<u8>) -> Result<LayerId, String> {
        let info = sound_info(&bytes)?;
        self.checkpoint("Import sound");
        let media = MediaFile::name_for(&bytes, "wav");
        self.doc
            .media
            .entry(media.clone())
            .or_insert_with(|| Arc::new(MediaFile { bytes }));
        let mut audio = Audio::new(media);
        audio.start = self.current_frame();
        let layer = labelled(
            self.doc
                .new_layer(super::import::stem(name), LayerKind::Audio(audio)),
        );
        let id = layer.id;
        self.place_imported(layer);
        self.set_status(
            StatusKind::Success,
            format!("Imported {name} ({:.1} seconds)", info.seconds),
        );
        Ok(id)
    }

    /// Set audio layer `id`'s start frame and volume (0 is silent, 1 as
    /// recorded, up to 2). The caller checkpoints, once per gesture.
    pub fn set_audio(&mut self, id: LayerId, start: Frame, volume: f32) {
        if let Some(LayerKind::Audio(a)) = self.doc.find_mut(id).map(|l| &mut l.kind) {
            a.start = start;
            a.volume = if volume.is_finite() {
                volume.clamp(0.0, 2.0)
            } else {
                1.0
            };
            self.dirty = true;
        }
    }

    /// The selected switch layer.
    pub fn switch_layer(&self) -> Option<LayerId> {
        let id = self.primary()?;
        matches!(self.doc.find(id)?.kind, LayerKind::Switch { .. }).then_some(id)
    }

    /// Show child `index` of the selected switch layer from the current
    /// frame on: a key naming it.
    pub(super) fn set_switch(&mut self, index: usize) -> bool {
        let Some(id) = self.switch_layer() else {
            return false;
        };
        let Some(name) = self
            .doc
            .find(id)
            .and_then(|l| l.kind.children())
            .and_then(|c| c.get(index))
            .map(|c| c.name.clone())
        else {
            return false;
        };
        let frame = self.current_frame();
        self.checkpoint("Switch layer");
        if let Some(LayerKind::Switch { switch, .. }) = self.doc.find_mut(id).map(|l| &mut l.kind) {
            let key = Key {
                frame,
                value: name,
                interp: Interpolation::Step,
            };
            match switch.binary_search_by_key(&frame, |k| k.frame) {
                Ok(i) => switch[i] = key,
                Err(i) => switch.insert(i, key),
            }
        }
        true
    }

    /// Key switch layer `id` from mouth cues of a sound starting on frame
    /// `start`, replacing its keys over the sound's span. Returns how many
    /// cues had no matching child layer. One undo step.
    pub fn apply_lip_sync(&mut self, id: LayerId, cues: &[MouthCue], start: Frame) -> Result<usize, String> {
        let Some(layer) = self.doc.find(id) else {
            return Err("the switch layer is gone".into());
        };
        let LayerKind::Switch { children, .. } = &layer.kind else {
            return Err("lip sync needs a switch layer with a mouth shape in each child".into());
        };
        let fps = self.doc.fps;
        let mut keys: Vec<Key<String>> = Vec::new();
        let mut unmatched = 0;
        for cue in cues {
            let child = mouth_names(&cue.value)
                .iter()
                .find_map(|n| children.iter().find(|c| c.name.eq_ignore_ascii_case(n)));
            let Some(child) = child else {
                unmatched += 1;
                continue;
            };
            let frame = start + (cue.start * fps).round() as Frame;
            if frame < 0 {
                continue;
            }
            let key = Key {
                frame,
                value: child.name.clone(),
                interp: Interpolation::Step,
            };
            // two cues on one frame: the later wins
            match keys.last_mut() {
                Some(last) if last.frame == frame => *last = key,
                _ => keys.push(key),
            }
        }
        if keys.is_empty() {
            return Err(
                "no mouth shape matched a layer: name the switch layer's children A to H and X, or MBP, etc, E, AI, O, U, FV, L and rest"
                    .into(),
            );
        }
        let (first, last) = (keys[0].frame, keys[keys.len() - 1].frame);
        self.checkpoint("Lip sync");
        if let Some(LayerKind::Switch { switch, .. }) = self.doc.find_mut(id).map(|l| &mut l.kind) {
            switch.retain(|k| k.frame < first || k.frame > last);
            switch.extend(keys);
            switch.sort_by_key(|k| k.frame);
        }
        Ok(unmatched)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Action;

    pub(crate) fn wav(seconds: f32) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut bytes = Vec::new();
        {
            let mut w = hound::WavWriter::new(std::io::Cursor::new(&mut bytes), spec).unwrap();
            for i in 0..(8000.0 * seconds) as i32 {
                w.write_sample(((i as f32 * 0.05).sin() * 8000.0) as i16).unwrap();
            }
            w.finalize().unwrap();
        }
        bytes
    }

    fn mouths() -> (Editor, LayerId) {
        let mut ed = Editor::untitled();
        ed.run(Action::NewSwitchLayer);
        let sw = ed.primary().unwrap();
        for name in ["rest", "MBP", "AI", "E"] {
            let child = ed.doc.new_layer(name, LayerKind::vector(vec![]));
            ed.doc.insert(Some(sw), 0, child);
        }
        ed.selection = vec![sw];
        (ed, sw)
    }

    #[test]
    fn sound_files_become_audio_layers() {
        let mut ed = Editor::untitled();
        ed.go_to(12);
        let id = ed.import_audio("line.wav", wav(0.5)).unwrap();
        let LayerKind::Audio(a) = &ed.doc.find(id).unwrap().kind else {
            panic!()
        };
        assert_eq!(a.start, 12);
        assert!(ed.doc.media.contains_key(&a.media));
        assert!(fanimate_fnm::validate(&ed.doc).is_clean());
        assert!((sound_info(&wav(0.5)).unwrap().seconds - 0.5).abs() < 1e-3);
        assert!(ed.import_audio("x.wav", b"RIFF nonsense".to_vec()).is_err());
    }

    #[test]
    fn rhubarb_cues_key_the_switch_layer() {
        let (mut ed, sw) = mouths();
        let cues = parse_rhubarb(
            r#"{"metadata":{},"mouthCues":[
                {"start":0.00,"end":0.10,"value":"X"},
                {"start":0.10,"end":0.30,"value":"D"},
                {"start":0.30,"end":0.35,"value":"A"},
                {"start":0.35,"end":0.50,"value":"G"}
            ]}"#,
        )
        .unwrap();
        let unmatched = ed.apply_lip_sync(sw, &cues, 24).unwrap();
        assert_eq!(unmatched, 1, "G has no FV mouth");
        let LayerKind::Switch { switch, .. } = &ed.doc.find(sw).unwrap().kind else {
            panic!()
        };
        let keys: Vec<(Frame, &str)> = switch.iter().map(|k| (k.frame, k.value.as_str())).collect();
        assert_eq!(keys, [(24, "rest"), (26, "AI"), (31, "MBP")]);
        assert_eq!(ed.key_frames(sw), [24, 26, 31]);
        assert!(fanimate_fnm::validate(&ed.doc).is_clean());
        assert!(parse_rhubarb("not json").is_err());
    }

    #[test]
    fn switching_by_hand_keys_the_current_frame() {
        let (mut ed, sw) = mouths();
        ed.go_to(5);
        // children are listed bottom first: E, AI, MBP, rest
        ed.run(Action::SetSwitch(1));
        let LayerKind::Switch { switch, .. } = &ed.doc.find(sw).unwrap().kind else {
            panic!()
        };
        assert_eq!(switch[0].value, "AI");
        ed.run(Action::Undo);
        let LayerKind::Switch { switch, .. } = &ed.doc.find(sw).unwrap().kind else {
            panic!()
        };
        assert!(switch.is_empty());
    }

    #[test]
    fn audio_start_and_volume_are_set_and_kept_in_range() {
        let mut ed = Editor::untitled();
        let id = ed.import_audio("line.wav", wav(0.5)).unwrap();
        ed.set_audio(id, -4, 7.0);
        let LayerKind::Audio(a) = &ed.doc.find(id).unwrap().kind else {
            panic!()
        };
        assert_eq!((a.start, a.volume), (-4, 2.0));
        ed.set_audio(id, 0, f32::NAN);
        let LayerKind::Audio(a) = &ed.doc.find(id).unwrap().kind else {
            panic!()
        };
        assert_eq!(a.volume, 1.0);
        assert!(fanimate_fnm::validate(&ed.doc).is_clean());
    }
}
