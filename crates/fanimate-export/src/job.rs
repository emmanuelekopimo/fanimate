//! What to export: the format, picture size and frames.

use std::path::{Path, PathBuf};

use fanimate_core::{Document, Frame};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// H.264 video in an MP4 file, with sound. Plays almost anywhere.
    #[default]
    Mp4,
    /// H.264 video in a Matroska file, with sound.
    Mkv,
    /// An animated GIF, looping, without sound.
    Gif,
    /// One PNG file per frame, with transparency kept outside the canvas.
    Png,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Mp4, Format::Mkv, Format::Gif, Format::Png];

    pub fn label(self) -> &'static str {
        match self {
            Format::Mp4 => "MP4 video (H.264)",
            Format::Mkv => "MKV video (H.264)",
            Format::Gif => "Animated GIF",
            Format::Png => "PNG sequence",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::Mkv => "mkv",
            Format::Gif => "gif",
            Format::Png => "png",
        }
    }

    /// Video formats are encoded by FFmpeg.
    pub fn needs_ffmpeg(self) -> bool {
        matches!(self, Format::Mp4 | Format::Mkv)
    }

    /// Whether the format carries the project's sound.
    pub fn has_sound(self) -> bool {
        self.needs_ffmpeg()
    }

    /// `path` with this format's extension.
    pub fn output(self, path: &Path) -> PathBuf {
        path.with_extension(self.extension())
    }
}

/// The settings of one export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub format: Format,
    /// Picture size in pixels; the canvas is fitted to it.
    pub width: u32,
    pub height: u32,
    /// First and last frame, inclusive.
    pub start: Frame,
    pub end: Frame,
}

impl Settings {
    /// The whole animation at the project's own size.
    pub fn for_doc(doc: &Document, format: Format) -> Self {
        Self {
            format,
            width: doc.width.round().max(1.0) as u32,
            height: doc.height.round().max(1.0) as u32,
            start: doc.start,
            end: doc.end,
        }
    }

    pub fn frame_count(&self) -> u32 {
        (self.end - self.start + 1).max(0) as u32
    }

    /// Size and range made usable: video encoders need even sizes, and an
    /// empty or reversed range exports one frame.
    pub fn sanitized(&self) -> Self {
        let even = |v: u32| {
            let v = v.clamp(2, 16384);
            if self.format.needs_ffmpeg() { v + v % 2 } else { v }
        };
        Self {
            format: self.format,
            width: even(self.width),
            height: even(self.height),
            start: self.start.max(0),
            end: self.end.max(self.start.max(0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_sizes_become_even_and_ranges_usable() {
        let s = Settings {
            format: Format::Mp4,
            width: 641,
            height: 0,
            start: 5,
            end: 2,
        }
        .sanitized();
        assert_eq!((s.width, s.height, s.start, s.end), (642, 2, 5, 5));
        assert_eq!(s.frame_count(), 1);
        let png = Settings {
            format: Format::Png,
            ..s.clone()
        };
        let png = Settings { width: 641, ..png }.sanitized();
        assert_eq!(png.width, 641);
    }

    #[test]
    fn formats_name_their_files() {
        assert_eq!(
            Format::Mkv.output(Path::new("a/scene.mp4")),
            Path::new("a/scene.mkv")
        );
        assert!(Format::Mp4.needs_ffmpeg() && !Format::Gif.needs_ffmpeg());
        let json = serde_json::to_string(&Format::Gif).unwrap();
        assert_eq!(json, "\"gif\"");
    }
}
