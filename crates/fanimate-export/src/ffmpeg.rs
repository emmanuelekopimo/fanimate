//! Finding and running FFmpeg, the separate program that encodes video.
//!
//! Fanimate ships an FFmpeg build next to itself and talks to it only
//! through its command line and standard input, so FFmpeg's licence never
//! reaches Fanimate's code (see `docs/legal/video-export.md`). It runs with
//! no console window.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::job::{Format, Settings};

fn exe_name() -> &'static str {
    if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }
}

/// FFmpeg's program: next to Fanimate's, in an `ffmpeg` folder there (or
/// its `bin` folder, as FFmpeg builds unpack), or else on PATH.
pub fn find() -> Option<PathBuf> {
    let exe = exe_name();
    let beside = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .into_iter()
        .flat_map(|dir| {
            [
                dir.join(exe),
                dir.join("ffmpeg").join(exe),
                dir.join("ffmpeg").join("bin").join(exe),
            ]
        });
    let on_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join(exe)).collect::<Vec<_>>())
        .unwrap_or_default();
    beside.chain(on_path).find(|p| p.is_file())
}

/// FFmpeg's arguments to encode raw RGBA frames from standard input, with
/// the sound in `audio` if any, into `out` (written as `settings.format`
/// whatever its extension).
pub fn video_args(settings: &Settings, fps: f32, audio: Option<&Path>, out: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    let mut push = |items: &[&str]| args.extend(items.iter().map(OsString::from));
    push(&["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
    let size = format!("{}x{}", settings.width, settings.height);
    let rate = format!("{fps}");
    push(&[
        "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &size, "-r", &rate, "-i", "-",
    ]);
    if let Some(audio) = audio {
        push(&["-i"]);
        args.push(audio.as_os_str().to_owned());
    }
    let mut push = |items: &[&str]| args.extend(items.iter().map(OsString::from));
    // yuv420p plays everywhere; high quality, as the animator will often
    // edit the file further
    push(&[
        "-c:v", "libx264", "-preset", "medium", "-crf", "17", "-pix_fmt", "yuv420p",
    ]);
    if audio.is_some() {
        push(&["-c:a", "aac", "-b:a", "192k"]);
    }
    let seconds = format!("{:.3}", settings.frame_count() as f32 / fps.max(1.0));
    push(&["-t", &seconds]);
    match settings.format {
        Format::Mkv => push(&["-f", "matroska"]),
        // start playing before the whole file has arrived
        _ => push(&["-movflags", "+faststart", "-f", "mp4"]),
    }
    args.push(out.as_os_str().to_owned());
    args
}

/// A command for `program` that opens no console window on Windows.
pub fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn arguments_describe_the_frames_and_the_file() {
        let s = Settings {
            format: Format::Mkv,
            width: 640,
            height: 360,
            start: 0,
            end: 47,
        };
        let a = strings(&video_args(
            &s,
            24.0,
            Some(Path::new("/t/sound.wav")),
            Path::new("/o/x.part"),
        ));
        let joined = a.join(" ");
        assert!(joined.contains("-s 640x360 -r 24 -i -"), "{joined}");
        assert!(joined.contains("-i /t/sound.wav"), "{joined}");
        assert!(joined.contains("-c:a aac"));
        assert!(joined.contains("-t 2.000"));
        assert!(joined.ends_with("-f matroska /o/x.part"), "{joined}");
        let silent = strings(&video_args(
            &Settings {
                format: Format::Mp4,
                ..s
            },
            24.0,
            None,
            Path::new("x"),
        ));
        assert!(!silent.iter().any(|a| a == "-c:a"));
        assert!(silent.join(" ").contains("-f mp4"));
    }
}
