//! Rendering a project's frames and writing them out.
//!
//! Every format writes to a `.part` file (or one per frame) beside the
//! output and renames it into place only once complete, so a cancelled or
//! failed export never leaves a broken file under the name asked for, and
//! never replaces a good one.

use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use fanimate_core::Document;
use fanimate_render::{Gpu, Offscreen};

use crate::ffmpeg;
use crate::job::{Format, Settings};
use crate::sound;

/// The sound's sample rate in exported video.
const SOUND_RATE: u32 = 48_000;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("FFmpeg was not found; it ships with Fanimate, so reinstalling should bring it back")]
    NoFfmpeg,
    #[error("the export was cancelled")]
    Cancelled,
    #[error("the frames could not be rendered: {0}")]
    Render(String),
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: std::io::Error },
    #[error("FFmpeg could not encode the video: {0}")]
    Ffmpeg(String),
    #[error("the GIF could not be written: {0}")]
    Gif(String),
}

type Result<T> = std::result::Result<T, ExportError>;

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> ExportError + '_ {
    move |source| ExportError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Export `doc` as `settings` say to `out` (for a PNG sequence, the frames
/// are named after it: `scene.png` gives `scene_0000.png` and on).
/// `progress` hears how many of how many frames are done, and returning
/// `false` from it cancels the export.
pub fn export(
    doc: &Document,
    settings: &Settings,
    out: &Path,
    progress: &mut dyn FnMut(u32, u32) -> bool,
) -> Result<()> {
    let s = settings.sanitized();
    let encoder = if s.format.needs_ffmpeg() {
        Some(ffmpeg::find().ok_or(ExportError::NoFfmpeg)?)
    } else {
        None
    };
    let gpu = Gpu::headless().map_err(|e| ExportError::Render(e.to_string()))?;
    let mut frames = Frames {
        gpu: &gpu,
        target: Offscreen::new(&gpu, s.width, s.height),
        doc,
        start: s.start,
    };
    let total = s.frame_count();
    let mut temps = Temps::default();
    match (s.format, encoder) {
        (Format::Png, _) => png_sequence(&mut frames, total, out, &mut temps, progress)?,
        (Format::Gif, _) => gif(&mut frames, &s, doc.fps, out, &mut temps, progress)?,
        (_, Some(ffmpeg)) => video(&mut frames, &s, doc, &ffmpeg, out, &mut temps, progress)?,
        (_, None) => return Err(ExportError::NoFfmpeg),
    }
    progress(total, total);
    Ok(())
}

/// Renders frames one after another into the same target.
struct Frames<'a> {
    gpu: &'a Gpu,
    target: Offscreen,
    doc: &'a Document,
    start: i32,
}

impl Frames<'_> {
    /// Frame `i` of the export, as RGBA pixels.
    fn get(&mut self, i: u32) -> Result<Vec<u8>> {
        self.target
            .draw(self.gpu, self.doc, (self.start + i as i32) as f32);
        self.target
            .read_rgba(self.gpu)
            .map_err(|e| ExportError::Render(e.to_string()))
    }
}

/// Files to remove however the export ends, unless finished.
#[derive(Default)]
struct Temps(Vec<PathBuf>);

impl Temps {
    fn add(&mut self, path: PathBuf) -> PathBuf {
        self.0.push(path.clone());
        path
    }

    /// Move finished `part` into place as `out`.
    fn finish(&mut self, part: &Path, out: &Path) -> Result<()> {
        // a rename does not replace a file everywhere (Windows)
        if out.exists() {
            std::fs::remove_file(out).map_err(io(out))?;
        }
        std::fs::rename(part, out).map_err(io(out))?;
        self.0.retain(|p| p != part);
        Ok(())
    }
}

impl Drop for Temps {
    fn drop(&mut self) {
        for p in &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}

fn part(out: &Path) -> PathBuf {
    let mut name = out.file_name().unwrap_or_default().to_owned();
    name.push(".part");
    out.with_file_name(name)
}

/// Remove the unfinished files an export to `out` left when its process
/// was killed before it could clean up (a stop or failure removes them
/// itself). Finished files are never touched.
pub(crate) fn remove_parts(out: &Path, format: Format) {
    if format != Format::Png {
        let _ = std::fs::remove_file(part(out));
        return;
    }
    // a PNG sequence writes one frame at a time, named after `out`
    let (Some(dir), Some(stem)) = (out.parent(), out.file_stem()) else {
        return;
    };
    let prefix = format!("{}_", stem.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(".png.part") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn png_sequence(
    frames: &mut Frames,
    total: u32,
    out: &Path,
    temps: &mut Temps,
    progress: &mut dyn FnMut(u32, u32) -> bool,
) -> Result<()> {
    let stem = out.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let last = frames.start + total.saturating_sub(1) as i32;
    let digits = last.to_string().len().max(4);
    let (w, h) = frames.target.size();
    for i in 0..total {
        if !progress(i, total) {
            return Err(ExportError::Cancelled);
        }
        let pixels = frames.get(i)?;
        let number = frames.start + i as i32;
        let file = out.with_file_name(format!("{stem}_{number:0digits$}.png"));
        let tmp = temps.add(part(&file));
        image::save_buffer_with_format(
            &tmp,
            &pixels,
            w,
            h,
            image::ExtendedColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| ExportError::Io {
            path: file.clone(),
            source: std::io::Error::other(e),
        })?;
        temps.finish(&tmp, &file)?;
    }
    Ok(())
}

fn gif(
    frames: &mut Frames,
    s: &Settings,
    fps: f32,
    out: &Path,
    temps: &mut Temps,
    progress: &mut dyn FnMut(u32, u32) -> bool,
) -> Result<()> {
    use image::codecs::gif::{GifEncoder, Repeat};
    let tmp = temps.add(part(out));
    let file = std::fs::File::create(&tmp).map_err(io(&tmp))?;
    let gif_err = |e: image::ImageError| ExportError::Gif(e.to_string());
    {
        // speed 10 of 30: close to the best palette at a fraction of the time
        let mut enc = GifEncoder::new_with_speed(BufWriter::new(file), 10);
        enc.set_repeat(Repeat::Infinite).map_err(gif_err)?;
        let delay = image::Delay::from_numer_denom_ms(1_000_000, (fps.max(0.001) * 1000.0) as u32);
        for i in 0..s.frame_count() {
            if !progress(i, s.frame_count()) {
                return Err(ExportError::Cancelled);
            }
            let pixels = frames.get(i)?;
            let img = image::RgbaImage::from_raw(s.width, s.height, pixels)
                .ok_or_else(|| ExportError::Render("a frame came back the wrong size".into()))?;
            enc.encode_frame(image::Frame::from_parts(img, 0, 0, delay))
                .map_err(gif_err)?;
        }
    }
    temps.finish(&tmp, out)
}

/// FFmpeg, stopped if the export ends early.
struct Encoder(Child);

impl Drop for Encoder {
    fn drop(&mut self) {
        // already exited after a finished export; harmless then
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn video(
    frames: &mut Frames,
    s: &Settings,
    doc: &Document,
    ffmpeg: &Path,
    out: &Path,
    temps: &mut Temps,
    progress: &mut dyn FnMut(u32, u32) -> bool,
) -> Result<()> {
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let audio = match sound::mix(doc, s.start, s.end, SOUND_RATE) {
        Some(samples) => {
            let path = temps.add(std::env::temp_dir().join(format!(
                "fanimate-export-{}-{}.wav",
                std::process::id(),
                RUNS.fetch_add(1, Ordering::Relaxed)
            )));
            sound::write_wav(&path, &samples, SOUND_RATE).map_err(|e| ExportError::Io {
                path: path.clone(),
                source: std::io::Error::other(e),
            })?;
            Some(path)
        }
        None => None,
    };
    let tmp = temps.add(part(out));
    let child = ffmpeg::command(ffmpeg)
        .args(ffmpeg::video_args(s, doc.fps, audio.as_deref(), &tmp))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| ExportError::Ffmpeg(format!("it could not start ({e})")))?;
    let mut enc = Encoder(child);
    // read FFmpeg's complaints as they come, so its pipe never fills
    let errors = enc.0.stderr.take().map(|mut err| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = err.read_to_string(&mut text);
            text
        })
    });
    let mut stdin = enc.0.stdin.take();
    for i in 0..s.frame_count() {
        if !progress(i, s.frame_count()) {
            return Err(ExportError::Cancelled);
        }
        let pixels = frames.get(i)?;
        // a write fails when FFmpeg has stopped; its message says why
        if stdin.as_mut().is_none_or(|w| w.write_all(&pixels).is_err()) {
            break;
        }
    }
    drop(stdin);
    let status = enc.0.wait().map_err(|e| ExportError::Ffmpeg(e.to_string()))?;
    let complaint = errors.and_then(|t| t.join().ok()).unwrap_or_default();
    if !status.success() {
        let line = complaint.lines().rev().find(|l| !l.trim().is_empty());
        return Err(ExportError::Ffmpeg(line.map_or_else(
            || format!("it stopped ({status})"),
            |l| l.trim().to_owned(),
        )));
    }
    temps.finish(&tmp, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fanimate_core::samples;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fanimate-export-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// GPU tests skip cleanly where no adapter exists.
    fn gpu_available() -> bool {
        let ok = Gpu::headless().is_ok();
        if !ok {
            eprintln!("no GPU adapter; skipping");
        }
        ok
    }

    fn small(format: Format, frames: i32) -> Settings {
        Settings {
            format,
            width: 64,
            height: 36,
            start: 0,
            end: frames - 1,
        }
    }

    #[test]
    fn png_sequences_number_their_frames() {
        if !gpu_available() {
            return;
        }
        let dir = temp_dir("png");
        let mut seen = Vec::new();
        export(
            &samples::hello(),
            &small(Format::Png, 3),
            &dir.join("hello.png"),
            &mut |d, t| {
                seen.push((d, t));
                true
            },
        )
        .unwrap();
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, ["hello_0000.png", "hello_0001.png", "hello_0002.png"]);
        assert_eq!(seen.last(), Some(&(3, 3)));
        let img = image::open(dir.join("hello_0001.png")).unwrap();
        assert_eq!((img.width(), img.height()), (64, 36));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn gifs_loop_their_frames() {
        if !gpu_available() {
            return;
        }
        let dir = temp_dir("gif");
        let out = dir.join("hello.gif");
        export(&samples::hello(), &small(Format::Gif, 4), &out, &mut |_, _| true).unwrap();
        let file = std::fs::File::open(&out).unwrap();
        let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file)).unwrap();
        use image::AnimationDecoder;
        assert_eq!(decoder.into_frames().count(), 4);
        assert!(!part(&out).exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancelling_leaves_nothing_behind_and_keeps_the_old_file() {
        if !gpu_available() {
            return;
        }
        let dir = temp_dir("cancel");
        let out = dir.join("hello.gif");
        std::fs::write(&out, b"the previous export").unwrap();
        let err = export(&samples::hello(), &small(Format::Gif, 10), &out, &mut |d, _| {
            d < 3
        })
        .unwrap_err();
        assert!(matches!(err, ExportError::Cancelled));
        assert_eq!(std::fs::read(&out).unwrap(), b"the previous export");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn videos_are_encoded_by_ffmpeg_with_their_sound() {
        if !gpu_available() {
            return;
        }
        if ffmpeg::find().is_none() {
            eprintln!("no ffmpeg; skipping");
            return;
        }
        let dir = temp_dir("video");
        let has = |bytes: &[u8], what: &[u8]| bytes.windows(what.len()).any(|w| w == what);
        // the containers name their codecs; read them from the file, as the
        // FFmpeg Fanimate ships cannot read video back
        for (format, magic, video, audio) in [
            (Format::Mp4, &b"ftyp"[..], &b"avc1"[..], &b"mp4a"[..]),
            (
                Format::Mkv,
                &[0x1A, 0x45, 0xDF, 0xA3][..],
                &b"V_MPEG4/ISO/AVC"[..],
                &b"A_AAC"[..],
            ),
        ] {
            let out = dir.join(format!("tone.{}", format.extension()));
            let doc = sound::tests::with_sound(0);
            let mut s = small(format, 10);
            s.width = 63; // made even
            export(&doc, &s, &out, &mut |_, _| true).unwrap();
            let bytes = std::fs::read(&out).unwrap();
            assert!(has(&bytes[..16], magic), "{format:?} header");
            assert!(has(&bytes, video), "{format:?} has H.264");
            assert!(has(&bytes, audio), "{format:?} has AAC sound");
            assert!(!part(&out).exists());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
