//! What can make each export format on this computer: Fanimate itself,
//! the system's video encoder, or the FFmpeg the user has. Found in the
//! background (asking FFmpeg what it can do takes a moment), and again
//! whenever the user chooses another FFmpeg or asks to look again.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use fanimate_export::ffmpeg::{self, Ffmpeg};
use fanimate_export::{ExportError, Format, Route};

/// Where to send someone who wants FFmpeg.
pub const HOW_TO_GET_FFMPEG: &str =
    "https://github.com/toonittmedia/fanimate/blob/main/docs/install.md#more-video-formats-with-ffmpeg";

#[derive(Default)]
pub struct VideoTools {
    /// The FFmpeg chosen in Preferences; `None` looks for one.
    pub chosen: Option<PathBuf>,
    /// Whether the system's H.264 encoder is there, once known.
    pub system_h264: bool,
    /// The FFmpeg found, and what it can make.
    pub ffmpeg: Option<Ffmpeg>,
    /// Why the chosen FFmpeg could not be used.
    pub ffmpeg_problem: Option<String>,
    looking: Option<Receiver<Found>>,
}

struct Found {
    system_h264: bool,
    ffmpeg: Option<Ffmpeg>,
    problem: Option<String>,
}

impl VideoTools {
    pub fn new(chosen: Option<PathBuf>) -> Self {
        let mut t = Self {
            chosen,
            ..Default::default()
        };
        t.look();
        t
    }

    /// Look for the encoders again, in the background.
    pub fn look(&mut self) {
        let (tx, rx) = channel();
        let chosen = self.chosen.clone();
        std::thread::spawn(move || {
            let system_h264 = fanimate_oscodec::h264_available();
            let (ffmpeg, problem) = match ffmpeg::find(chosen.as_deref()) {
                Some(path) => match Ffmpeg::probe(&path) {
                    Ok(f) => (Some(f), None),
                    Err(e) => (None, Some(e)),
                },
                None if chosen.is_some() => (None, Some("the chosen FFmpeg is no longer there".into())),
                None => (None, None),
            };
            let _ = tx.send(Found {
                system_h264,
                ffmpeg,
                problem,
            });
        });
        self.looking = Some(rx);
    }

    /// Whether a look is still going on.
    pub fn looking(&self) -> bool {
        self.looking.is_some()
    }

    /// Take in what a background look found. Returns whether it finished.
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.looking else {
            return false;
        };
        match rx.try_recv() {
            Ok(found) => {
                self.system_h264 = found.system_h264;
                self.ffmpeg = found.ffmpeg;
                self.ffmpeg_problem = found.problem;
                self.looking = None;
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.looking = None;
                true
            }
        }
    }

    pub fn route(&self, format: Format) -> Result<Route, ExportError> {
        fanimate_export::route(format, self.system_h264, self.ffmpeg.as_ref())
    }

    /// Who makes `format` here, in a few words, or why it cannot be made.
    pub fn maker(&self, format: Format) -> Result<String, String> {
        match self.route(format) {
            Ok(Route::BuiltIn) => Ok("Made by Fanimate".into()),
            Ok(Route::System) => Ok(format!("Made by {}'s built-in encoder", system_name())),
            Ok(Route::Ffmpeg { encoder, .. }) => Ok(format!(
                "Made by FFmpeg {} ({encoder})",
                self.ffmpeg.as_ref().map_or("", |f| f.version.as_str())
            )),
            Err(e) => Err(e.to_string()),
        }
    }

    /// The formats only FFmpeg makes that the FFmpeg found can, by name.
    pub fn ffmpeg_adds(&self) -> Vec<&'static str> {
        self.ffmpeg
            .as_ref()
            .map(|f| f.formats().iter().map(|f| short_name(*f)).collect())
            .unwrap_or_default()
    }
}

pub fn system_name() -> &'static str {
    if cfg!(windows) {
        "Windows"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "the system"
    }
}

/// A format's short name, for lists.
fn short_name(f: Format) -> &'static str {
    match f {
        Format::Mkv => "MKV",
        Format::Hevc => "H.265",
        Format::Webm => "WebM",
        Format::ProRes4444 => "ProRes 4444",
        Format::ProRes422 => "ProRes 422",
        Format::Dnxhr => "DNxHR",
        Format::Webp => "animated WebP",
        other => other.label(),
    }
}

/// The groups the Export Video window lists formats in.
pub fn groups() -> [(&'static str, &'static [Format]); 4] {
    [
        (
            "Video",
            &[Format::Mp4, Format::Hevc, Format::Av1, Format::Webm, Format::Mkv],
        ),
        (
            "For editing",
            &[Format::ProRes4444, Format::ProRes422, Format::Dnxhr, Format::Avi],
        ),
        ("Animated image", &[Format::Gif, Format::Apng, Format::Webp]),
        ("Picture sequence", &[Format::Png, Format::Exr, Format::Tiff]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_format_is_listed_once() {
        let listed: Vec<Format> = groups().iter().flat_map(|(_, f)| f.iter().copied()).collect();
        assert_eq!(listed.len(), Format::ALL.len());
        for f in Format::ALL {
            assert_eq!(listed.iter().filter(|g| **g == f).count(), 1, "{f:?}");
        }
    }

    #[test]
    fn makers_say_who_or_why_not() {
        let t = VideoTools::default();
        assert_eq!(t.maker(Format::Exr).unwrap(), "Made by Fanimate");
        let why = t.maker(Format::ProRes4444).unwrap_err();
        assert!(
            why.contains("needs FFmpeg") && why.contains("Preferences"),
            "{why}"
        );
        let with = VideoTools {
            system_h264: true,
            ..Default::default()
        };
        assert!(with.maker(Format::Mp4).unwrap().contains("built-in encoder"));
    }

    /// The link must land on the install guide's FFmpeg section.
    #[test]
    fn how_to_link_points_at_the_install_guide() {
        assert!(HOW_TO_GET_FFMPEG.ends_with("#more-video-formats-with-ffmpeg"));
    }
}
