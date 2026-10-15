//! Media Foundation: Windows' H.264 and AAC encoders and MP4 writer,
//! driven through its Sink Writer. Present on every Windows 10 and 11
//! except the "N" editions without Microsoft's free Media Feature Pack.
//! A certified graphics card encoder is used when there is one.

use std::path::{Path, PathBuf};

use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize};
use windows::core::{GUID, HSTRING};

use crate::{Error, Options, Result};

fn err(context: &str) -> impl Fn(windows::core::Error) -> Error + '_ {
    move |e| Error::System(format!("{context}: {}", e.message()))
}

/// 100-nanosecond units, Media Foundation's clock.
const SECOND: f64 = 10_000_000.0;

/// COM and Media Foundation, started for as long as this lives.
struct Session {
    /// Whether this thread's COM was started here, so it is ended here.
    com: bool,
}

impl Session {
    fn start() -> Result<Self> {
        // SAFETY: plain initialisation calls; a thread already in another
        // COM mode (a window's thread) still works for Media Foundation
        let com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        let started = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) };
        if let Err(e) = started {
            if com {
                // SAFETY: balances the successful CoInitializeEx above
                unsafe { CoUninitialize() };
            }
            return Err(err("Media Foundation could not start")(e));
        }
        Ok(Self { com })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: balances MFStartup and, when made, CoInitializeEx
        unsafe {
            let _ = MFShutdown();
            if self.com {
                CoUninitialize();
            }
        }
    }
}

pub fn h264_available() -> bool {
    let Ok(_session) = Session::start() else {
        return false;
    };
    let output = MFT_REGISTER_TYPE_INFO {
        guidMajorType: MFMediaType_Video,
        guidSubtype: MFVideoFormat_H264,
    };
    let mut found: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count = 0u32;
    let flags =
        MFT_ENUM_FLAG_SYNCMFT | MFT_ENUM_FLAG_ASYNCMFT | MFT_ENUM_FLAG_HARDWARE | MFT_ENUM_FLAG_SORTANDFILTER;
    // SAFETY: Media Foundation fills `found` with `count` activation
    // objects in memory it allocates; each is released by reading it out
    // and dropping it, then the array is freed
    unsafe {
        if MFTEnumEx(
            MFT_CATEGORY_VIDEO_ENCODER,
            flags,
            None,
            Some(&output),
            &mut found,
            &mut count,
        )
        .is_err()
        {
            return false;
        }
        if !found.is_null() {
            for i in 0..count as usize {
                drop(found.add(i).read());
            }
            CoTaskMemFree(Some(found as *const _));
        }
    }
    count > 0
}

fn media_type() -> Result<IMFMediaType> {
    // SAFETY: creates an empty attribute store
    unsafe { MFCreateMediaType() }.map_err(err("a media type could not be made"))
}

fn set_guid(t: &IMFMediaType, key: &GUID, value: &GUID) -> Result<()> {
    // SAFETY: setting an attribute on a live media type
    unsafe { t.SetGUID(key, value) }.map_err(err("a media type setting"))
}

fn set_u32(t: &IMFMediaType, key: &GUID, value: u32) -> Result<()> {
    // SAFETY: as above
    unsafe { t.SetUINT32(key, value) }.map_err(err("a media type setting"))
}

/// Two 32-bit values packed as Media Foundation stores sizes and ratios.
fn set_pair(t: &IMFMediaType, key: &GUID, high: u32, low: u32) -> Result<()> {
    // SAFETY: as above
    unsafe { t.SetUINT64(key, (u64::from(high) << 32) | u64::from(low)) }.map_err(err("a media type setting"))
}

fn video_type(o: &Options, subtype: &GUID, fps: (u32, u32)) -> Result<IMFMediaType> {
    let t = media_type()?;
    set_guid(&t, &MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
    set_guid(&t, &MF_MT_SUBTYPE, subtype)?;
    set_pair(&t, &MF_MT_FRAME_SIZE, o.width, o.height)?;
    set_pair(&t, &MF_MT_FRAME_RATE, fps.0, fps.1)?;
    set_pair(&t, &MF_MT_PIXEL_ASPECT_RATIO, 1, 1)?;
    set_u32(&t, &MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
    // BT.709, limited range: what `fanimate-export` converts frames to
    set_u32(&t, &MF_MT_VIDEO_PRIMARIES, MFVideoPrimaries_BT709.0 as u32)?;
    set_u32(&t, &MF_MT_TRANSFER_FUNCTION, MFVideoTransFunc_709.0 as u32)?;
    set_u32(&t, &MF_MT_YUV_MATRIX, MFVideoTransferMatrix_BT709.0 as u32)?;
    set_u32(&t, &MF_MT_VIDEO_NOMINAL_RANGE, MFNominalRange_16_235.0 as u32)?;
    Ok(t)
}

fn audio_type(subtype: &GUID, rate: u32) -> Result<IMFMediaType> {
    let t = media_type()?;
    set_guid(&t, &MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
    set_guid(&t, &MF_MT_SUBTYPE, subtype)?;
    set_u32(&t, &MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
    set_u32(&t, &MF_MT_AUDIO_SAMPLES_PER_SECOND, rate)?;
    set_u32(&t, &MF_MT_AUDIO_NUM_CHANNELS, 2)?;
    Ok(t)
}

/// A sample holding `data`, shown at `time` for `duration` (100 ns units).
fn sample(data: &[u8], time: i64, duration: i64) -> Result<IMFSample> {
    let len = data.len() as u32;
    // SAFETY: the buffer is locked for exactly its `len` bytes while the
    // data is copied in, and unlocked before use
    unsafe {
        let buffer = MFCreateMemoryBuffer(len).map_err(err("a frame buffer could not be made"))?;
        let mut ptr: *mut u8 = std::ptr::null_mut();
        buffer
            .Lock(&mut ptr, None, None)
            .map_err(err("a frame buffer could not be filled"))?;
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        buffer
            .Unlock()
            .map_err(err("a frame buffer could not be filled"))?;
        buffer
            .SetCurrentLength(len)
            .map_err(err("a frame buffer could not be filled"))?;
        let sample = MFCreateSample().map_err(err("a frame could not be made"))?;
        sample
            .AddBuffer(&buffer)
            .map_err(err("a frame could not be made"))?;
        sample
            .SetSampleTime(time)
            .map_err(err("a frame could not be timed"))?;
        sample
            .SetSampleDuration(duration)
            .map_err(err("a frame could not be timed"))?;
        Ok(sample)
    }
}

pub struct Writer {
    writer: IMFSinkWriter,
    path: PathBuf,
    video: u32,
    audio: Option<u32>,
    fps: f64,
    rate: u32,
    /// Sound samples (per channel) written so far.
    samples: u64,
    // dropped last: Media Foundation must outlive every object above
    _session: Session,
}

impl Writer {
    pub fn create(path: &Path, o: Options) -> Result<Self> {
        let session = Session::start()?;
        let fps = (o.fps.max(0.001) * 1000.0).round() as u32;
        let fps_ratio = (fps, 1000);
        // SAFETY: Media Foundation objects created and configured in the
        // order the Sink Writer documents; all live in `Self`
        unsafe {
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 3).map_err(err("the writer settings"))?;
            let attrs = attrs.ok_or(Error::System("the writer settings".into()))?;
            attrs
                .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
                .map_err(err("the writer settings"))?;
            // the output may be named `.part`, so say MP4 outright
            attrs
                .SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)
                .map_err(err("the writer settings"))?;
            let writer = MFCreateSinkWriterFromURL(&HSTRING::from(path.as_os_str()), None, &attrs)
                .map_err(err("the MP4 file could not be started"))?;

            let out = video_type(&o, &MFVideoFormat_H264, fps_ratio)?;
            set_u32(&out, &MF_MT_AVG_BITRATE, o.bit_rate)?;
            set_u32(&out, &MF_MT_MPEG2_PROFILE, eAVEncH264VProfile_High.0 as u32)?;
            let video = writer
                .AddStream(&out)
                .map_err(err("the H.264 encoder refused the picture size or rate"))?;
            let input = video_type(&o, &MFVideoFormat_NV12, fps_ratio)?;
            writer
                .SetInputMediaType(video, &input, None)
                .map_err(err("the H.264 encoder refused the frames"))?;

            let mut audio = None;
            if let Some(rate) = o.sound_rate {
                let out = audio_type(&MFAudioFormat_AAC, rate)?;
                // 192 kbit/s, the highest rate Windows' AAC encoder offers
                set_u32(&out, &MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24_000)?;
                let stream = writer
                    .AddStream(&out)
                    .map_err(err("the AAC encoder refused the sound"))?;
                let input = audio_type(&MFAudioFormat_PCM, rate)?;
                set_u32(&input, &MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
                set_u32(&input, &MF_MT_AUDIO_AVG_BYTES_PER_SECOND, rate * 4)?;
                writer
                    .SetInputMediaType(stream, &input, None)
                    .map_err(err("the AAC encoder refused the sound"))?;
                audio = Some(stream);
            }
            writer
                .BeginWriting()
                .map_err(err("the MP4 file could not be started"))?;
            Ok(Self {
                writer,
                path: path.to_owned(),
                video,
                audio,
                fps: f64::from(fps) / 1000.0,
                rate: o.sound_rate.unwrap_or(48_000),
                samples: 0,
                _session: session,
            })
        }
    }

    pub fn frame(&mut self, index: u32, nv12: &[u8], sound: &[f32]) -> Result<()> {
        let at = |i: u32| (f64::from(i) * SECOND / self.fps).round() as i64;
        let (time, next) = (at(index), at(index + 1));
        if let Some(stream) = self.audio
            && !sound.is_empty()
        {
            let pcm: Vec<u8> = sound
                .iter()
                .flat_map(|s| ((s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16).to_le_bytes())
                .collect();
            let frames = (sound.len() / 2) as u64;
            let start = (self.samples as f64 * SECOND / f64::from(self.rate)).round() as i64;
            let length = (frames as f64 * SECOND / f64::from(self.rate)).round() as i64;
            let s = sample(&pcm, start, length)?;
            // SAFETY: writing a sample to a stream this writer added
            unsafe { self.writer.WriteSample(stream, &s) }.map_err(err("the sound could not be encoded"))?;
            self.samples += frames;
        }
        let s = sample(nv12, time, next - time)?;
        // SAFETY: as above
        unsafe { self.writer.WriteSample(self.video, &s) }.map_err(err("a frame could not be encoded"))
    }

    pub fn finish(self) -> Result<()> {
        // SAFETY: completes the file; the writer is not used afterwards
        unsafe { self.writer.Finalize() }.map_err(err("the MP4 file could not be finished"))?;
        crate::colour::tag_bt709(&self.path)
            .map_err(|e| Error::System(format!("the MP4 file's colours could not be tagged: {e}")))?;
        Ok(())
    }
}
