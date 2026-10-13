//! Sound: playing audio layers in time with playback, and their waveforms
//! for the timeline.
//!
//! Sounds are decoded once and kept. While the timeline plays, an output
//! stream mixes every audible audio layer on the fly from the frame
//! playback started at, so a long scene needs no big buffer. A missing
//! sound device is not an error: playback goes on silently.

use std::collections::HashMap;
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fanimate_core::{Document, LayerKind};

/// A sound, decoded to interleaved samples from -1 to 1.
pub struct Decoded {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl Decoded {
    fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    /// The sound at `t` seconds, mixed down to (left, right).
    fn at(&self, t: f64) -> (f32, f32) {
        let pos = t * self.rate as f64;
        if pos < 0.0 {
            return (0.0, 0.0);
        }
        let i = pos as usize;
        if i + 1 >= self.frames() {
            return (0.0, 0.0);
        }
        let frac = (pos - i as f64) as f32;
        let c = self.channels as usize;
        let get = |frame: usize, ch: usize| self.samples[frame * c + ch.min(c - 1)];
        let lerp = |ch| get(i, ch) + (get(i + 1, ch) - get(i, ch)) * frac;
        (lerp(0), lerp(1))
    }
}

/// Decode a WAV file. `None` if it cannot be read.
pub fn decode(bytes: &[u8]) -> Option<Decoded> {
    let mut reader = hound::WavReader::new(std::io::Cursor::new(bytes)).ok()?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().filter_map(Result::ok).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample.clamp(1, 32) - 1)) as f32;
            reader
                .samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 * scale)
                .collect()
        }
    };
    (spec.channels > 0 && spec.sample_rate > 0).then_some(Decoded {
        rate: spec.sample_rate,
        channels: spec.channels,
        samples,
    })
}

/// One audio layer to mix: its sound, when it starts, how loud.
#[derive(Clone)]
struct Voice {
    sound: Arc<Decoded>,
    start_seconds: f64,
    volume: f32,
}

/// Per timeline frame, the lowest and highest sample.
pub type Peaks = Arc<Vec<(f32, f32)>>;

pub struct Audio {
    decoded: HashMap<String, Option<Arc<Decoded>>>,
    /// Loudest and quietest sample per timeline frame, by sound and frame
    /// rate, for waveforms.
    peaks: HashMap<(String, u32), Peaks>,
    stream: Option<cpal::Stream>,
    /// The frame playback was last seen at, to notice loops and jumps.
    last_frame: f32,
    failed: bool,
}

impl Default for Audio {
    fn default() -> Self {
        Self {
            decoded: HashMap::new(),
            peaks: HashMap::new(),
            stream: None,
            last_frame: 0.0,
            failed: false,
        }
    }
}

impl Audio {
    fn sound(&mut self, doc: &Document, name: &str) -> Option<Arc<Decoded>> {
        self.decoded
            .entry(name.to_owned())
            .or_insert_with(|| doc.media.get(name).and_then(|f| decode(&f.bytes)).map(Arc::new))
            .clone()
    }

    /// The audible audio layers of `doc`.
    fn voices(&mut self, doc: &Document) -> Vec<Voice> {
        let mut layers = Vec::new();
        doc.walk(|l| {
            if let LayerKind::Audio(a) = &l.kind
                && l.visible
            {
                layers.push((a.media.clone(), a.start, a.volume));
            }
        });
        let fps = doc.fps.max(1.0) as f64;
        layers
            .into_iter()
            .filter_map(|(name, start, volume)| {
                Some(Voice {
                    sound: self.sound(doc, &name)?,
                    start_seconds: start as f64 / fps,
                    volume,
                })
            })
            .collect()
    }

    /// Each frame: start, restart or stop the sound to follow playback.
    pub fn follow(&mut self, doc: &Document, playing: bool, frame: f32) {
        let looped = frame < self.last_frame;
        self.last_frame = frame;
        if !playing {
            self.stream = None;
            return;
        }
        if self.stream.is_some() && !looped {
            return;
        }
        let voices = self.voices(doc);
        self.stream = None;
        if voices.is_empty() || self.failed {
            return;
        }
        match play(voices, frame as f64 / doc.fps.max(1.0) as f64) {
            Ok(stream) => self.stream = Some(stream),
            Err(e) => {
                // no device or no permission: keep animating, silently
                log::warn!("sound could not play: {e}");
                self.failed = true;
            }
        }
    }

    /// The waveform of sound `name` at `fps`: per timeline frame, the
    /// lowest and highest sample.
    pub fn peaks(&mut self, doc: &Document, name: &str) -> Option<Peaks> {
        let fps = doc.fps.max(1.0);
        let key = (name.to_owned(), fps.to_bits());
        if let Some(p) = self.peaks.get(&key) {
            return Some(p.clone());
        }
        let sound = self.sound(doc, name)?;
        let per_frame = (sound.rate as f32 / fps).max(1.0) as usize;
        let c = sound.channels.max(1) as usize;
        let peaks: Vec<(f32, f32)> = sound
            .samples
            .chunks(per_frame * c)
            .map(|chunk| {
                chunk
                    .iter()
                    .fold((0.0f32, 0.0f32), |(lo, hi), s| (lo.min(*s), hi.max(*s)))
            })
            .collect();
        let peaks = Arc::new(peaks);
        self.peaks.insert(key, peaks.clone());
        Some(peaks)
    }
}

/// Start playing `voices` from `from` seconds on the default device.
fn play(voices: Vec<Voice>, from: f64) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no sound output device")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();
    let stream = match format {
        cpal::SampleFormat::F32 => build::<f32>(&device, &config, voices, from),
        cpal::SampleFormat::I16 => build::<i16>(&device, &config, voices, from),
        cpal::SampleFormat::U16 => build::<u16>(&device, &config, voices, from),
        cpal::SampleFormat::I32 => build::<i32>(&device, &config, voices, from),
        other => return Err(format!("sound output format {other} is not supported")),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    voices: Vec<Voice>,
    from: f64,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let rate = config.sample_rate as f64;
    let channels = config.channels.max(1) as usize;
    let mut n: u64 = 0;
    device
        .build_output_stream(
            *config,
            move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
                for frame in out.chunks_mut(channels) {
                    let t = from + n as f64 / rate;
                    n += 1;
                    let (mut l, mut r) = (0.0f32, 0.0f32);
                    for v in &voices {
                        let (a, b) = v.sound.at(t - v.start_seconds);
                        l += a * v.volume;
                        r += b * v.volume;
                    }
                    for (i, s) in frame.iter_mut().enumerate() {
                        let x = if i % 2 == 0 { l } else { r };
                        *s = T::from_sample(x.clamp(-1.0, 1.0));
                    }
                }
            },
            |e| log::warn!("sound output: {e}"),
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(channels: u16, samples: &[i16]) -> Vec<u8> {
        let spec = hound::WavSpec {
            channels,
            sample_rate: 100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut bytes = Vec::new();
        {
            let mut w = hound::WavWriter::new(std::io::Cursor::new(&mut bytes), spec).unwrap();
            for s in samples {
                w.write_sample(*s).unwrap();
            }
            w.finalize().unwrap();
        }
        bytes
    }

    #[test]
    fn decodes_and_samples_between_frames() {
        let d = decode(&wav(1, &[0, 16384, 0, -16384])).unwrap();
        assert_eq!(d.frames(), 4);
        let (l, r) = d.at(0.005);
        assert!((l - 0.25).abs() < 1e-3 && l == r);
        assert_eq!(d.at(-1.0), (0.0, 0.0));
        assert_eq!(d.at(10.0), (0.0, 0.0));
        assert!(decode(b"nope").is_none());
    }

    #[test]
    fn waveform_peaks_per_frame() {
        let mut doc = Document::new("t", 10.0, 10.0, 50.0);
        let bytes = wav(1, &[0, 16384, 0, -16384]);
        doc.media
            .insert("media/s.wav".into(), Arc::new(fanimate_core::MediaFile { bytes }));
        let mut audio = Audio::default();
        let p = audio.peaks(&doc, "media/s.wav").unwrap();
        // 100 samples a second at 50 fps: two samples a frame
        assert_eq!(p.len(), 2);
        assert!(p[0].1 > 0.4 && p[1].0 < -0.4);
        assert!(audio.peaks(&doc, "media/missing.wav").is_none());
    }
}
