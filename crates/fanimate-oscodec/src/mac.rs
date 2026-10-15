//! AVFoundation: macOS's H.264 and AAC encoders and MP4 writer, driven
//! through an AVAssetWriter. Every Mac Fanimate runs on has them, and Apple
//! silicon and recent Intel Macs encode on their media engine.

use std::path::Path;
use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVAssetWriterStatus,
    AVFileTypeMPEG4, AVMediaTypeAudio, AVMediaTypeVideo, AVVideoAverageBitRateKey, AVVideoCodecKey,
    AVVideoCodecTypeH264, AVVideoColorPrimaries_ITU_R_709_2, AVVideoColorPrimariesKey,
    AVVideoColorPropertiesKey, AVVideoCompressionPropertiesKey, AVVideoHeightKey,
    AVVideoMaxKeyFrameIntervalKey, AVVideoProfileLevelH264HighAutoLevel, AVVideoProfileLevelKey,
    AVVideoTransferFunction_ITU_R_709_2, AVVideoTransferFunctionKey, AVVideoWidthKey,
    AVVideoYCbCrMatrix_ITU_R_709_2, AVVideoYCbCrMatrixKey,
};
use objc2_avf_audio::{AVEncoderBitRateKey, AVFormatIDKey, AVNumberOfChannelsKey, AVSampleRateKey};
use objc2_core_audio_types::{
    AudioStreamBasicDescription, kAudioFormatFlagIsPacked, kAudioFormatFlagIsSignedInteger,
    kAudioFormatLinearPCM, kAudioFormatMPEG4AAC,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::{
    CMAudioFormatDescriptionCreate, CMAudioSampleBufferCreateReadyWithPacketDescriptions, CMBlockBuffer,
    CMFormatDescription, CMSampleBuffer, CMTime, kCMBlockBufferAssureMemoryNowFlag,
};
use objc2_core_video::{
    CVAttachmentMode, CVPixelBuffer, CVPixelBufferCreate, CVPixelBufferGetBaseAddressOfPlane,
    CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVImageBufferColorPrimaries_ITU_R_709_2,
    kCVImageBufferColorPrimariesKey, kCVImageBufferTransferFunction_ITU_R_709_2,
    kCVImageBufferTransferFunctionKey, kCVImageBufferYCbCrMatrix_ITU_R_709_2, kCVImageBufferYCbCrMatrixKey,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, kCVReturnSuccess,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use crate::{Error, Options, Result};

/// How long to wait for an encoder to take more before giving up.
const STALL: Duration = Duration::from_secs(30);

fn system(what: &str) -> Error {
    Error::System(what.into())
}

/// A constant AVFoundation exports, which only a broken system lacks.
fn constant(value: Option<&'static NSString>) -> Result<&'static NSString> {
    value.ok_or(Error::Unsupported)
}

fn dict(pairs: &[(&NSString, &AnyObject)]) -> Retained<NSDictionary<NSString, AnyObject>> {
    let (keys, values): (Vec<&NSString>, Vec<&AnyObject>) = pairs.iter().copied().unzip();
    NSDictionary::from_slices(&keys, &values)
}

fn any<T: AsRef<AnyObject> + ?Sized>(object: &T) -> &AnyObject {
    object.as_ref()
}

pub fn h264_available() -> bool {
    // part of AVFoundation on every macOS version Fanimate supports
    true
}

pub struct Writer {
    writer: Retained<AVAssetWriter>,
    video: Retained<AVAssetWriterInput>,
    adaptor: Retained<AVAssetWriterInputPixelBufferAdaptor>,
    audio: Option<(Retained<AVAssetWriterInput>, CFRetained<CMFormatDescription>)>,
    width: usize,
    height: usize,
    /// Frame `i` is shown at `i * 1000 / timescale` seconds.
    timescale: i32,
    rate: u32,
    /// Sound samples (per channel) written so far.
    samples: i64,
}

impl Writer {
    pub fn create(path: &Path, o: Options) -> Result<Self> {
        // AVAssetWriter refuses to replace a file
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(Error::System(format!("the old file could not be replaced: {e}")));
            }
            _ => {}
        }
        let url = NSURL::from_file_path(path).ok_or_else(|| system("the file name is not usable"))?;
        let timescale = (o.fps.max(0.001) * 1000.0).round() as i32;
        // SAFETY: AVFoundation objects made and set up in the order its
        // documentation gives, with settings dictionaries of the documented
        // keys and value types; the constants are read once AVFoundation
        // is loaded, which linking it ensures
        unsafe {
            let writer = AVAssetWriter::assetWriterWithURL_fileType_error(&url, constant(AVFileTypeMPEG4)?)
                .map_err(|e| {
                Error::System(format!(
                    "the MP4 file could not be started: {}",
                    e.localizedDescription()
                ))
            })?;

            let w = NSNumber::new_u32(o.width);
            let h = NSNumber::new_u32(o.height);
            let bits = NSNumber::new_u32(o.bit_rate);
            let key_every = NSNumber::new_u32((o.fps.round() as u32).max(1) * 2);
            let compression = dict(&[
                (constant(AVVideoAverageBitRateKey)?, any(&*bits)),
                (
                    constant(AVVideoProfileLevelKey)?,
                    any(constant(AVVideoProfileLevelH264HighAutoLevel)?),
                ),
                (constant(AVVideoMaxKeyFrameIntervalKey)?, any(&*key_every)),
            ]);
            // BT.709, limited range: what `fanimate-export` converts frames to
            let colour = dict(&[
                (
                    constant(AVVideoColorPrimariesKey)?,
                    any(constant(AVVideoColorPrimaries_ITU_R_709_2)?),
                ),
                (
                    constant(AVVideoTransferFunctionKey)?,
                    any(constant(AVVideoTransferFunction_ITU_R_709_2)?),
                ),
                (
                    constant(AVVideoYCbCrMatrixKey)?,
                    any(constant(AVVideoYCbCrMatrix_ITU_R_709_2)?),
                ),
            ]);
            let settings = dict(&[
                (constant(AVVideoCodecKey)?, any(constant(AVVideoCodecTypeH264)?)),
                (constant(AVVideoWidthKey)?, any(&*w)),
                (constant(AVVideoHeightKey)?, any(&*h)),
                (constant(AVVideoCompressionPropertiesKey)?, any(&*compression)),
                (constant(AVVideoColorPropertiesKey)?, any(&*colour)),
            ]);
            let video = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
                constant(AVMediaTypeVideo)?,
                Some(&settings),
            );
            video.setExpectsMediaDataInRealTime(false);
            if !writer.canAddInput(&video) {
                return Err(system("the H.264 encoder refused the picture size or rate"));
            }
            writer.addInput(&video);
            let adaptor =
                AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
                    &video, None,
                );

            let mut audio = None;
            if let Some(rate) = o.sound_rate {
                let format = NSNumber::new_u32(kAudioFormatMPEG4AAC);
                let sample_rate = NSNumber::new_f64(f64::from(rate));
                let channels = NSNumber::new_u32(2);
                let bits = NSNumber::new_u32(192_000);
                let settings = dict(&[
                    (constant(AVFormatIDKey)?, any(&*format)),
                    (constant(AVSampleRateKey)?, any(&*sample_rate)),
                    (constant(AVNumberOfChannelsKey)?, any(&*channels)),
                    (constant(AVEncoderBitRateKey)?, any(&*bits)),
                ]);
                let input = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
                    constant(AVMediaTypeAudio)?,
                    Some(&settings),
                );
                input.setExpectsMediaDataInRealTime(false);
                if !writer.canAddInput(&input) {
                    return Err(system("the AAC encoder refused the sound"));
                }
                writer.addInput(&input);
                audio = Some((input, pcm_format(rate)?));
            }

            if !writer.startWriting() {
                return Err(failure(&writer, "the MP4 file could not be started"));
            }
            writer.startSessionAtSourceTime(CMTime::new(0, timescale));
            Ok(Self {
                writer,
                video,
                adaptor,
                audio,
                width: o.width as usize,
                height: o.height as usize,
                timescale,
                rate: o.sound_rate.unwrap_or(48_000),
                samples: 0,
            })
        }
    }

    pub fn frame(&mut self, index: u32, nv12: &[u8], sound: &[f32]) -> Result<()> {
        if let Some((input, format)) = &self.audio
            && !sound.is_empty()
        {
            let pcm: Vec<u8> = sound
                .iter()
                .flat_map(|s| ((s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16).to_le_bytes())
                .collect();
            let frames = (sound.len() / 2) as i64;
            // SAFETY: CMTime::new is a plain value constructor
            let at = unsafe { CMTime::new(self.samples, self.rate as i32) };
            let buffer = sound_buffer(&pcm, frames, format, at)?;
            wait_for(&self.writer, input)?;
            // SAFETY: appending a complete sample buffer to an input of
            // this writer, which has started
            if !unsafe { input.appendSampleBuffer(&buffer) } {
                return Err(failure(&self.writer, "the sound could not be encoded"));
            }
            self.samples += frames;
        }
        let picture = self.picture(nv12)?;
        wait_for(&self.writer, &self.video)?;
        // SAFETY: as above, with a filled pixel buffer the adaptor retains
        let added = unsafe {
            self.adaptor.appendPixelBuffer_withPresentationTime(
                &picture,
                CMTime::new(i64::from(index) * 1000, self.timescale),
            )
        };
        if !added {
            return Err(failure(&self.writer, "the frame could not be encoded"));
        }
        Ok(())
    }

    /// A pixel buffer holding `nv12`, tagged BT.709 so the encoder takes
    /// it as it is.
    fn picture(&self, nv12: &[u8]) -> Result<CFRetained<CVPixelBuffer>> {
        let (w, h) = (self.width, self.height);
        if nv12.len() < w * h * 3 / 2 {
            return Err(system("a frame was the wrong size"));
        }
        let mut out: *mut CVPixelBuffer = std::ptr::null_mut();
        // SAFETY: CoreVideo makes a buffer of the given size and format; it
        // is locked while the planes are filled, each row within the
        // plane's own row length, and unlocked before use
        unsafe {
            let made = CVPixelBufferCreate(
                None,
                w,
                h,
                kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
                None,
                NonNull::from(&mut out),
            );
            let buffer = match NonNull::new(out) {
                Some(p) if made == kCVReturnSuccess => CFRetained::from_raw(p),
                _ => return Err(system("a frame buffer could not be made")),
            };
            if CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags(0)) != kCVReturnSuccess {
                return Err(system("a frame buffer could not be filled"));
            }
            // luma rows, then half as many rows of interleaved chroma
            for (plane, rows, source) in [(0, h, &nv12[..w * h]), (1, h / 2, &nv12[w * h..w * h * 3 / 2])] {
                let base = CVPixelBufferGetBaseAddressOfPlane(&buffer, plane) as *mut u8;
                let stride = CVPixelBufferGetBytesPerRowOfPlane(&buffer, plane);
                if base.is_null() || stride < w {
                    CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
                    return Err(system("a frame buffer could not be filled"));
                }
                for (y, row) in source.chunks_exact(w).take(rows).enumerate() {
                    std::ptr::copy_nonoverlapping(row.as_ptr(), base.add(y * stride), w);
                }
            }
            CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
            for (key, value) in [
                (
                    kCVImageBufferColorPrimariesKey,
                    kCVImageBufferColorPrimaries_ITU_R_709_2,
                ),
                (
                    kCVImageBufferTransferFunctionKey,
                    kCVImageBufferTransferFunction_ITU_R_709_2,
                ),
                (
                    kCVImageBufferYCbCrMatrixKey,
                    kCVImageBufferYCbCrMatrix_ITU_R_709_2,
                ),
            ] {
                buffer.set_attachment(key, value, CVAttachmentMode::ShouldPropagate);
            }
            Ok(buffer)
        }
    }

    pub fn finish(self) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let done = RcBlock::new(move || {
            let _ = tx.send(());
        });
        // SAFETY: inputs of this writer, marked finished once; the
        // completion block only sends on a channel, so may run on any thread
        unsafe {
            self.video.markAsFinished();
            if let Some((input, _)) = &self.audio {
                input.markAsFinished();
            }
            self.writer.finishWritingWithCompletionHandler(&done);
        }
        if rx.recv_timeout(STALL).is_err() {
            return Err(system("the MP4 file took too long to finish"));
        }
        // SAFETY: reading the status of a writer that has finished
        if unsafe { self.writer.status() } != AVAssetWriterStatus::Completed {
            return Err(failure(&self.writer, "the MP4 file could not be finished"));
        }
        Ok(())
    }
}

/// Wait until `input` takes more, as long as the writer has not failed.
fn wait_for(writer: &AVAssetWriter, input: &AVAssetWriterInput) -> Result<()> {
    let start = Instant::now();
    // SAFETY: reading the state of live objects
    while !unsafe { input.isReadyForMoreMediaData() } {
        if unsafe { writer.status() } == AVAssetWriterStatus::Failed {
            return Err(failure(writer, "the encoder stopped"));
        }
        if start.elapsed() > STALL {
            return Err(system("the encoder stopped taking frames"));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}

/// `context`, and what AVFoundation says went wrong, if anything.
fn failure(writer: &AVAssetWriter, context: &str) -> Error {
    // SAFETY: reading the error of a live writer
    match unsafe { writer.error() } {
        Some(e) => Error::System(format!("{context}: {}", e.localizedDescription())),
        None => system(context),
    }
}

/// 16-bit interleaved stereo PCM at `rate`.
fn pcm_format(rate: u32) -> Result<CFRetained<CMFormatDescription>> {
    let asbd = AudioStreamBasicDescription {
        mSampleRate: f64::from(rate),
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
        mBytesPerPacket: 4,
        mFramesPerPacket: 1,
        mBytesPerFrame: 4,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 16,
        mReserved: 0,
    };
    let mut out: *const CMFormatDescription = std::ptr::null();
    // SAFETY: a valid description, no channel layout, cookie or
    // extensions; on success `out` holds one reference, taken over below
    unsafe {
        let made = CMAudioFormatDescriptionCreate(
            None,
            NonNull::from(&asbd),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            None,
            NonNull::from(&mut out),
        );
        match NonNull::new(out as *mut CMFormatDescription) {
            Some(p) if made == 0 => Ok(CFRetained::from_raw(p)),
            _ => Err(system("the sound format could not be described")),
        }
    }
}

/// A sample buffer holding `frames` stereo samples of `pcm`, played at `at`.
fn sound_buffer(
    pcm: &[u8],
    frames: i64,
    format: &CMFormatDescription,
    at: CMTime,
) -> Result<CFRetained<CMSampleBuffer>> {
    let mut block: *mut CMBlockBuffer = std::ptr::null_mut();
    let mut sample: *mut CMSampleBuffer = std::ptr::null_mut();
    // SAFETY: CoreMedia allocates a block of `pcm.len()` bytes, which is
    // filled from `pcm` before the sample buffer is made over it; each
    // object made is taken over by `from_raw` exactly once
    unsafe {
        let made = CMBlockBuffer::create_with_memory_block(
            None,
            std::ptr::null_mut(),
            pcm.len(),
            None,
            std::ptr::null(),
            0,
            pcm.len(),
            kCMBlockBufferAssureMemoryNowFlag,
            NonNull::from(&mut block),
        );
        let block = match NonNull::new(block) {
            Some(p) if made == 0 => CFRetained::from_raw(p),
            _ => return Err(system("a sound buffer could not be made")),
        };
        let source = NonNull::new(pcm.as_ptr() as *mut std::ffi::c_void).ok_or_else(|| system("no sound"))?;
        if CMBlockBuffer::replace_data_bytes(source, &block, 0, pcm.len()) != 0 {
            return Err(system("a sound buffer could not be filled"));
        }
        let made = CMAudioSampleBufferCreateReadyWithPacketDescriptions(
            None,
            &block,
            format,
            frames as _,
            at,
            std::ptr::null(),
            NonNull::from(&mut sample),
        );
        match NonNull::new(sample) {
            Some(p) if made == 0 => Ok(CFRetained::from_raw(p)),
            _ => Err(system("a sound buffer could not be made")),
        }
    }
}
