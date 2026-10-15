//! AV1 video in an MP4 file, made by Fanimate itself: rav1e (a pure-Rust
//! AV1 encoder) and muxide (a pure-Rust MP4 writer). AV1 is royalty-free,
//! so this needs nothing installed and no licence. No sound yet: there is
//! no mature pure-Rust Opus or AAC encoder to pair it with; the system
//! encoder's MP4 and the FFmpeg formats carry sound.

use std::io::BufWriter;
use std::path::Path;

use rav1e::prelude::*;

use crate::job::Settings;
use crate::run::{ExportError, Frames, Result, Temps, io, part};
use crate::yuv;

fn av1_err(e: impl std::fmt::Display) -> ExportError {
    ExportError::Video(format!("AV1: {e}"))
}

pub(crate) fn write(
    frames: &mut Frames,
    s: &Settings,
    fps: f32,
    out: &Path,
    temps: &mut Temps,
    progress: &mut dyn FnMut(u32, u32) -> bool,
) -> Result<()> {
    let (w, h) = (s.width as usize, s.height as usize);
    let fps = fps.max(0.001);
    // a fast preset: animation is flat colour, which AV1 handles well even
    // at speed, and an export should not take many times its length
    let mut enc = EncoderConfig::with_speed_preset(9);
    enc.width = w;
    enc.height = h;
    enc.time_base = Rational::new(1000, (fps * 1000.0).round() as u64);
    enc.bit_depth = 8;
    enc.chroma_sampling = ChromaSampling::Cs420;
    enc.pixel_range = PixelRange::Limited;
    enc.color_description = Some(ColorDescription {
        color_primaries: ColorPrimaries::BT709,
        transfer_characteristics: TransferCharacteristics::BT709,
        matrix_coefficients: MatrixCoefficients::BT709,
    });
    // 0 is lossless, 255 the worst: high quality, as for the other formats
    enc.quantizer = 60;
    enc.max_key_frame_interval = (fps * 2.0).round().max(1.0) as u64;
    // the frame rate in the stream itself; muxide 0.2.5 also misreads a
    // sequence header without it (it reads the decoder model flag that is
    // only there with timing info)
    enc.enable_timing_info = true;
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get());
    let cfg = Config::new().with_encoder_config(enc).with_threads(threads);
    let mut ctx: Context<u8> = cfg.new_context().map_err(av1_err)?;

    let tmp = temps.add(part(out));
    let file = std::fs::File::create(&tmp).map_err(io(&tmp))?;
    let mut mux = muxide::api::MuxerConfig::new(s.width, s.height, f64::from(fps))
        .into_builder(BufWriter::new(file), muxide::api::VideoCodec::Av1)
        .build()
        .map_err(av1_err)?;
    let write_packets = |ctx: &mut Context<u8>, mux: &mut muxide::api::Muxer<_>| -> Result<bool> {
        loop {
            match ctx.receive_packet() {
                Ok(packet) => {
                    let pts = packet.input_frameno as f64 / f64::from(fps);
                    let key = packet.frame_type == FrameType::KEY;
                    mux.write_video(pts, &packet.data, key).map_err(av1_err)?;
                }
                Err(EncoderStatus::Encoded) => {}
                Err(EncoderStatus::NeedMoreData) => return Ok(false),
                Err(EncoderStatus::LimitReached) => return Ok(true),
                Err(e) => return Err(av1_err(e)),
            }
        }
    };
    let total = s.frame_count();
    for i in 0..total {
        if !progress(i, total) {
            return Err(ExportError::Cancelled);
        }
        let pixels = frames.get(i)?;
        let pic = yuv::i420(&pixels, w, h);
        let mut frame = ctx.new_frame();
        frame.planes[0].copy_from_raw_u8(&pic.y, w, 1);
        frame.planes[1].copy_from_raw_u8(&pic.u, w / 2, 1);
        frame.planes[2].copy_from_raw_u8(&pic.v, w / 2, 1);
        ctx.send_frame(frame).map_err(av1_err)?;
        write_packets(&mut ctx, &mut mux)?;
    }
    ctx.flush();
    write_packets(&mut ctx, &mut mux)?;
    mux.finish().map_err(av1_err)?;
    temps.finish(&tmp, out)
}
