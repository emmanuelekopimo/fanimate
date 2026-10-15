//! Uncompressed AVI: 32-bit frames (with transparency) and 16-bit stereo
//! sound, which any editor opens. Uncompressed frames are large (8 MB
//! each at 1080p), so files pass AVI's original 1 GB limit within seconds;
//! this writes the OpenDML (AVI 2.0) layout every current reader takes:
//!
//! ```text
//! RIFF AVI   hdrl (avih; strl video and sound, each with an indx super
//!            index; odml dmlh), movi (chunks, ix00, ix01), idx1
//! RIFF AVIX  movi (chunks, ix00, ix01)        one per further gigabyte
//! ```
//!
//! The legacy `idx1` index covers the first segment, for old readers.

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use crate::job::Settings;
use crate::run::{ExportError, Frames, Result, Temps, io, part};

/// How large one RIFF segment may grow before the next starts.
const SEGMENT: u64 = 1 << 30;
/// Super index entries reserved per stream: one per segment, so up to 256
/// segments (256 GB).
const SEGMENTS: usize = 256;
const SOUND_RATE: u32 = crate::run::SOUND_RATE;

const AVIF_HASINDEX: u32 = 0x10;
const AVIF_ISINTERLEAVED: u32 = 0x100;
const AVIIF_KEYFRAME: u32 = 0x10;

/// One stream's chunks in the current segment: where each chunk's data
/// starts and how long it is.
#[derive(Default)]
struct StreamIndex {
    chunks: Vec<(u64, u32)>,
    /// Super index entries: (index chunk offset, its size, its duration).
    segments: Vec<(u64, u32, u32)>,
    /// Where this stream's `indx` chunk sits, to fill in at the end.
    indx_at: u64,
}

pub(crate) struct AviWriter<W: Write + Seek> {
    w: W,
    width: u32,
    height: u32,
    sound: bool,
    segment_limit: u64,
    /// Start of the current RIFF and of its `movi` list.
    riff_at: u64,
    movi_at: u64,
    first_segment: bool,
    /// Legacy index entries for the first segment: id, offset from the
    /// `movi` fourcc, size.
    idx1: Vec<([u8; 4], u32, u32)>,
    video: StreamIndex,
    audio: StreamIndex,
    frames: u32,
    first_frames: u32,
    samples: u64,
    // header fields to fill in at the end
    avih_total_at: u64,
    video_length_at: u64,
    audio_length_at: u64,
    dmlh_at: u64,
}

fn u16le(v: u16) -> [u8; 2] {
    v.to_le_bytes()
}

fn u32le(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

impl<W: Write + Seek> AviWriter<W> {
    pub(crate) fn new(mut w: W, width: u32, height: u32, fps: f32, sound: bool) -> std::io::Result<Self> {
        let frame_bytes = width * height * 4;
        // a frame rate as a fraction, exact to a thousandth
        let (scale, rate) = (1000u32, (fps.max(0.001) * 1000.0).round() as u32);
        let streams = if sound { 2 } else { 1 };

        w.write_all(b"RIFF\0\0\0\0AVI ")?;
        let hdrl = Self::list_start(&mut w, b"hdrl")?;
        // main header
        w.write_all(b"avih")?;
        w.write_all(&u32le(56))?;
        w.write_all(&u32le((1_000_000.0 / fps.max(0.001)).round() as u32))?;
        w.write_all(&u32le(frame_bytes.saturating_mul(rate / scale.max(1) + 1)))?;
        w.write_all(&u32le(0))?;
        w.write_all(&u32le(AVIF_HASINDEX | AVIF_ISINTERLEAVED))?;
        let avih_total_at = w.stream_position()?;
        w.write_all(&u32le(0))?; // frames in the first segment
        w.write_all(&u32le(0))?;
        w.write_all(&u32le(streams))?;
        w.write_all(&u32le(frame_bytes))?;
        w.write_all(&u32le(width))?;
        w.write_all(&u32le(height))?;
        w.write_all(&[0; 16])?;

        // video stream
        let strl = Self::list_start(&mut w, b"strl")?;
        w.write_all(b"strh")?;
        w.write_all(&u32le(56))?;
        w.write_all(b"vidsDIB ")?;
        w.write_all(&[0; 12])?; // flags, priority, language, initial frames
        w.write_all(&u32le(scale))?;
        w.write_all(&u32le(rate))?;
        w.write_all(&u32le(0))?;
        let video_length_at = w.stream_position()?;
        w.write_all(&u32le(0))?;
        w.write_all(&u32le(frame_bytes))?;
        w.write_all(&u32le(u32::MAX))?; // quality: default
        w.write_all(&u32le(0))?;
        for v in [0u16, 0, width as u16, height as u16] {
            w.write_all(&u16le(v))?;
        }
        w.write_all(b"strf")?;
        w.write_all(&u32le(40))?;
        w.write_all(&u32le(40))?;
        w.write_all(&u32le(width))?;
        // positive height: rows bottom first, as DIBs are
        w.write_all(&u32le(height))?;
        w.write_all(&u16le(1))?;
        w.write_all(&u16le(32))?;
        w.write_all(&u32le(0))?; // BI_RGB: uncompressed
        w.write_all(&u32le(frame_bytes))?;
        w.write_all(&[0; 16])?;
        let video_indx = Self::super_index(&mut w, b"00db")?;
        Self::list_end(&mut w, strl)?;

        // sound stream: PCM, 16-bit stereo
        let mut audio_length_at = 0;
        let mut audio_indx = 0;
        if sound {
            let strl = Self::list_start(&mut w, b"strl")?;
            w.write_all(b"strh")?;
            w.write_all(&u32le(56))?;
            w.write_all(b"auds\0\0\0\0")?;
            w.write_all(&[0; 12])?;
            w.write_all(&u32le(4))?; // one sample frame is 4 bytes
            w.write_all(&u32le(SOUND_RATE * 4))?;
            w.write_all(&u32le(0))?;
            audio_length_at = w.stream_position()?;
            w.write_all(&u32le(0))?;
            w.write_all(&u32le(SOUND_RATE * 4))?;
            w.write_all(&u32le(u32::MAX))?;
            w.write_all(&u32le(4))?;
            w.write_all(&[0; 8])?;
            w.write_all(b"strf")?;
            w.write_all(&u32le(18))?;
            w.write_all(&u16le(1))?; // PCM
            w.write_all(&u16le(2))?;
            w.write_all(&u32le(SOUND_RATE))?;
            w.write_all(&u32le(SOUND_RATE * 4))?;
            w.write_all(&u16le(4))?;
            w.write_all(&u16le(16))?;
            w.write_all(&u16le(0))?;
            audio_indx = Self::super_index(&mut w, b"01wb")?;
            Self::list_end(&mut w, strl)?;
        }

        // OpenDML header: the frame count across every segment
        let odml = Self::list_start(&mut w, b"odml")?;
        w.write_all(b"dmlh")?;
        w.write_all(&u32le(248))?;
        let dmlh_at = w.stream_position()?;
        w.write_all(&[0; 248])?;
        Self::list_end(&mut w, odml)?;
        Self::list_end(&mut w, hdrl)?;

        let movi_at = Self::list_start(&mut w, b"movi")?;
        Ok(Self {
            w,
            width,
            height,
            sound,
            segment_limit: SEGMENT,
            riff_at: 0,
            movi_at,
            first_segment: true,
            idx1: Vec::new(),
            video: StreamIndex {
                indx_at: video_indx,
                ..Default::default()
            },
            audio: StreamIndex {
                indx_at: audio_indx,
                ..Default::default()
            },
            frames: 0,
            first_frames: 0,
            samples: 0,
            avih_total_at,
            video_length_at,
            audio_length_at,
            dmlh_at,
        })
    }

    /// Start a `LIST` of `kind`; returns where it starts.
    fn list_start(w: &mut W, kind: &[u8; 4]) -> std::io::Result<u64> {
        let at = w.stream_position()?;
        w.write_all(b"LIST\0\0\0\0")?;
        w.write_all(kind)?;
        Ok(at)
    }

    /// Fill in the size of the `LIST` (or `RIFF`) starting at `at`.
    fn list_end(w: &mut W, at: u64) -> std::io::Result<()> {
        let end = w.stream_position()?;
        w.seek(SeekFrom::Start(at + 4))?;
        w.write_all(&u32le((end - at - 8) as u32))?;
        w.seek(SeekFrom::Start(end))?;
        Ok(())
    }

    /// An empty super index with room for every segment; returns where it
    /// starts.
    fn super_index(w: &mut W, chunk_id: &[u8; 4]) -> std::io::Result<u64> {
        let at = w.stream_position()?;
        w.write_all(b"indx")?;
        w.write_all(&u32le(24 + 16 * SEGMENTS as u32))?;
        w.write_all(&u16le(4))?; // longs per entry
        w.write_all(&[0, 0])?; // sub type, AVI_INDEX_OF_INDEXES
        w.write_all(&u32le(0))?; // entries in use
        w.write_all(chunk_id)?;
        w.write_all(&[0; 12])?;
        w.write_all(&vec![0; 16 * SEGMENTS])?;
        Ok(at)
    }

    fn chunk(&mut self, id: &[u8; 4], data: &[u8], audio: bool) -> Result<()> {
        let at = self.w.stream_position().map_err(io_err)?;
        let len = data.len() as u32;
        self.w.write_all(id).map_err(io_err)?;
        self.w.write_all(&u32le(len)).map_err(io_err)?;
        self.w.write_all(data).map_err(io_err)?;
        if len % 2 == 1 {
            self.w.write_all(&[0]).map_err(io_err)?;
        }
        let index = if audio { &mut self.audio } else { &mut self.video };
        index.chunks.push((at + 8, len));
        if self.first_segment {
            self.idx1.push((*id, (at - (self.movi_at + 8)) as u32, len));
        }
        Ok(())
    }

    /// Add one frame of top-to-bottom RGBA `pixels`, and the stereo sound
    /// that plays during it.
    pub(crate) fn frame(&mut self, pixels: &[u8], sound: &[f32]) -> Result<()> {
        let pos = self.w.stream_position().map_err(io_err)?;
        let full = pos - self.riff_at + (self.width * self.height * 4) as u64 + 1024 > self.segment_limit;
        // every segment holds at least one frame
        if full && !self.video.chunks.is_empty() {
            self.next_segment()?;
        }
        if self.sound && !sound.is_empty() {
            let bytes: Vec<u8> = sound
                .iter()
                .flat_map(|s| ((s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16).to_le_bytes())
                .collect();
            self.chunk(b"01wb", &bytes, true)?;
            self.samples += (sound.len() / 2) as u64;
        }
        // DIB rows are BGRA, bottom row first
        let row = self.width as usize * 4;
        let mut dib = Vec::with_capacity(pixels.len());
        for line in pixels.chunks_exact(row).rev() {
            for p in line.as_chunks::<4>().0 {
                dib.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
            }
        }
        self.chunk(b"00db", &dib, false)?;
        self.frames += 1;
        if self.first_segment {
            self.first_frames += 1;
        }
        Ok(())
    }

    /// Write the current segment's standard indexes and close it.
    fn close_segment(&mut self) -> Result<()> {
        for (audio, id, ix) in [(false, b"00db", b"ix00"), (true, b"01wb", b"ix01")] {
            if audio && !self.sound {
                continue;
            }
            let index = if audio { &mut self.audio } else { &mut self.video };
            let chunks = std::mem::take(&mut index.chunks);
            let at = self.w.stream_position().map_err(io_err)?;
            let base = self.movi_at;
            let size = 24 + 8 * chunks.len() as u32;
            let mut b = Vec::with_capacity(size as usize + 8);
            b.extend_from_slice(ix);
            b.extend_from_slice(&u32le(size));
            b.extend_from_slice(&u16le(2));
            b.extend_from_slice(&[0, 1]); // sub type, AVI_INDEX_OF_CHUNKS
            b.extend_from_slice(&u32le(chunks.len() as u32));
            b.extend_from_slice(id);
            b.extend_from_slice(&base.to_le_bytes());
            b.extend_from_slice(&u32le(0));
            let mut duration = 0u32;
            for (data_at, len) in &chunks {
                b.extend_from_slice(&u32le((data_at - base) as u32));
                b.extend_from_slice(&u32le(*len));
                duration += if audio { len / 4 } else { 1 };
            }
            self.w.write_all(&b).map_err(io_err)?;
            if index.segments.len() >= SEGMENTS {
                return Err(ExportError::Render(
                    "the AVI file would pass 256 GB; export a shorter range".into(),
                ));
            }
            index.segments.push((at, size + 8, duration));
        }
        Self::list_end(&mut self.w, self.movi_at).map_err(io_err)?;
        if self.first_segment {
            let mut b = Vec::with_capacity(8 + 16 * self.idx1.len());
            b.extend_from_slice(b"idx1");
            b.extend_from_slice(&u32le(16 * self.idx1.len() as u32));
            for (id, offset, len) in &self.idx1 {
                b.extend_from_slice(id);
                b.extend_from_slice(&u32le(AVIIF_KEYFRAME));
                b.extend_from_slice(&u32le(*offset));
                b.extend_from_slice(&u32le(*len));
            }
            self.w.write_all(&b).map_err(io_err)?;
        }
        Self::list_end(&mut self.w, self.riff_at).map_err(io_err)
    }

    fn next_segment(&mut self) -> Result<()> {
        self.close_segment()?;
        self.first_segment = false;
        self.riff_at = self.w.stream_position().map_err(io_err)?;
        self.w.write_all(b"RIFF\0\0\0\0AVIX").map_err(io_err)?;
        self.movi_at = Self::list_start(&mut self.w, b"movi").map_err(io_err)?;
        Ok(())
    }

    /// Close the file: the last segment, then the totals and super indexes
    /// in the header.
    pub(crate) fn finish(mut self) -> Result<W> {
        self.close_segment()?;
        let end = self.w.stream_position().map_err(io_err)?;
        let mut put = |at: u64, bytes: &[u8]| -> Result<()> {
            self.w.seek(SeekFrom::Start(at)).map_err(io_err)?;
            self.w.write_all(bytes).map_err(io_err)
        };
        put(self.avih_total_at, &u32le(self.first_frames))?;
        put(self.video_length_at, &u32le(self.frames))?;
        put(self.dmlh_at, &u32le(self.frames))?;
        if self.sound {
            put(self.audio_length_at, &u32le(self.samples as u32))?;
        }
        for index in [&self.video, &self.audio] {
            if index.indx_at == 0 {
                continue;
            }
            let mut b = Vec::with_capacity(16 * index.segments.len());
            for (at, size, duration) in &index.segments {
                b.extend_from_slice(&at.to_le_bytes());
                b.extend_from_slice(&u32le(*size));
                b.extend_from_slice(&u32le(*duration));
            }
            self.w.seek(SeekFrom::Start(index.indx_at + 12)).map_err(io_err)?;
            self.w
                .write_all(&u32le(index.segments.len() as u32))
                .map_err(io_err)?;
            self.w.seek(SeekFrom::Start(index.indx_at + 32)).map_err(io_err)?;
            self.w.write_all(&b).map_err(io_err)?;
        }
        self.w.seek(SeekFrom::Start(end)).map_err(io_err)?;
        self.w.flush().map_err(io_err)?;
        Ok(self.w)
    }
}

fn io_err(source: std::io::Error) -> ExportError {
    ExportError::Io {
        path: "AVI file".into(),
        source,
    }
}

/// Export frames and sound as an uncompressed AVI.
pub(crate) fn write(
    frames: &mut Frames,
    s: &Settings,
    fps: f32,
    sound: Option<&[f32]>,
    out: &Path,
    temps: &mut Temps,
    progress: &mut dyn FnMut(u32, u32) -> bool,
) -> Result<()> {
    let tmp = temps.add(part(out));
    let file = File::create(&tmp).map_err(io(&tmp))?;
    let mut avi =
        AviWriter::new(BufWriter::new(file), s.width, s.height, fps, sound.is_some()).map_err(io(&tmp))?;
    let total = s.frame_count();
    for i in 0..total {
        if !progress(i, total) {
            return Err(ExportError::Cancelled);
        }
        let pixels = frames.get(i)?;
        avi.frame(&pixels, sound_for_frame(sound, i, fps))?;
    }
    avi.finish()?;
    temps.finish(&tmp, out)
}

/// The stereo samples that play during frame `i`.
pub(crate) fn sound_for_frame(sound: Option<&[f32]>, i: u32, fps: f32) -> &[f32] {
    let Some(sound) = sound else { return &[] };
    let at = |f: u32| {
        ((f64::from(f) * f64::from(SOUND_RATE) / f64::from(fps.max(0.001))).round() as usize * 2)
            .min(sound.len())
    };
    &sound[at(i)..at(i + 1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn u32_at(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    /// Walk the RIFF chunks at `at..end`, returning (id, start, size).
    fn chunks(b: &[u8], mut at: usize, end: usize) -> Vec<([u8; 4], usize, usize)> {
        let mut out = Vec::new();
        while at + 8 <= end {
            let id: [u8; 4] = b[at..at + 4].try_into().unwrap();
            let size = u32_at(b, at + 4) as usize;
            out.push((id, at, size));
            at += 8 + size + size % 2;
        }
        out
    }

    fn write(width: u32, height: u32, frames: u32, limit: u64, sound: bool) -> Vec<u8> {
        let mut avi = AviWriter::new(Cursor::new(Vec::new()), width, height, 24.0, sound).unwrap();
        avi.segment_limit = limit;
        let pixels: Vec<u8> = (0..width * height * 4).map(|i| i as u8).collect();
        let samples = vec![0.25f32; 2000 * 2];
        for _ in 0..frames {
            avi.frame(&pixels, if sound { &samples } else { &[] }).unwrap();
        }
        avi.finish().unwrap().into_inner()
    }

    #[test]
    fn a_small_file_is_one_well_formed_segment() {
        let b = write(4, 2, 3, SEGMENT, true);
        assert_eq!(&b[..4], b"RIFF");
        assert_eq!(&b[8..12], b"AVI ");
        assert_eq!(u32_at(&b, 4) as usize + 8, b.len());
        let top = chunks(&b, 12, b.len());
        let ids: Vec<_> = top.iter().map(|c| c.0).collect();
        assert_eq!(ids, [*b"LIST", *b"LIST", *b"idx1"]);
        // movi holds 3 sound and 3 picture chunks, then the two indexes
        let (_, movi, size) = top[1];
        assert_eq!(&b[movi + 8..movi + 12], b"movi");
        let inside: Vec<_> = chunks(&b, movi + 12, movi + 8 + size)
            .iter()
            .map(|c| c.0)
            .collect();
        assert_eq!(inside.iter().filter(|id| *id == b"00db").count(), 3);
        assert_eq!(inside.iter().filter(|id| *id == b"01wb").count(), 3);
        assert_eq!(&inside[inside.len() - 2..], [*b"ix00", *b"ix01"]);
        // idx1 lists all six chunks
        assert_eq!(top[2].2, 16 * 6);
        // the first picture is BGRA from the bottom row up
        let first = chunks(&b, movi + 12, movi + 8 + size)
            .into_iter()
            .find(|c| c.0 == *b"00db")
            .unwrap();
        let data = &b[first.1 + 8..first.1 + 8 + 32];
        // bottom row starts at pixel 4 (byte 16) of the top-down RGBA
        assert_eq!(&data[..4], &[18, 17, 16, 19]);
    }

    #[test]
    fn large_files_continue_in_avix_segments_with_super_indexes() {
        // a tiny limit forces a segment every frame or so
        let b = write(16, 16, 6, 2048, false);
        let top = chunks(&b, 0, b.len());
        let riffs: Vec<_> = top.iter().filter(|c| c.0 == *b"RIFF").collect();
        assert!(riffs.len() >= 3, "{} segments", riffs.len());
        assert_eq!(&b[riffs[1].1 + 8..riffs[1].1 + 12], b"AVIX");
        // the video super index counts every segment, and its durations
        // add up to every frame
        let indx = b.windows(4).position(|w| w == b"indx").unwrap();
        let in_use = u32_at(&b, indx + 12) as usize;
        assert_eq!(in_use, riffs.len());
        let frames: u32 = (0..in_use).map(|i| u32_at(&b, indx + 32 + 16 * i + 12)).sum();
        assert_eq!(frames, 6);
        // dmlh holds the total, avih only the first segment's
        let dmlh = b.windows(4).position(|w| w == b"dmlh").unwrap();
        assert_eq!(u32_at(&b, dmlh + 8), 6);
        let avih = b.windows(4).position(|w| w == b"avih").unwrap();
        assert!(u32_at(&b, avih + 8 + 16) < 6);
    }

    #[test]
    fn each_frame_gets_its_share_of_the_sound() {
        let sound = vec![0.0f32; 48_000 * 2];
        let total: usize = (0..24)
            .map(|i| sound_for_frame(Some(&sound), i, 24.0).len())
            .sum();
        assert_eq!(total, sound.len());
        assert_eq!(sound_for_frame(Some(&sound), 0, 24.0).len(), 2000 * 2);
        assert!(sound_for_frame(None, 3, 24.0).is_empty());
    }
}
