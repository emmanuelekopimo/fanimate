//! Animated PNG: full colour and transparency, looping, without sound.
//! Plays in every current browser; viewers that do not know animation
//! show the first frame.

use std::io::BufWriter;
use std::path::Path;

use crate::job::Settings;
use crate::run::{ExportError, Frames, Result, Temps, io, part};

/// A frame's duration as the fraction of a second APNG stores in 16 bits.
fn frame_delay(fps: f32) -> (u16, u16) {
    let fps = fps.clamp(0.01, 655.0);
    (100, (fps * 100.0).round().clamp(1.0, f32::from(u16::MAX)) as u16)
}

pub(crate) fn write(
    frames: &mut Frames,
    s: &Settings,
    fps: f32,
    out: &Path,
    temps: &mut Temps,
    progress: &mut dyn FnMut(u32, u32) -> bool,
) -> Result<()> {
    let tmp = temps.add(part(out));
    let file = std::fs::File::create(&tmp).map_err(io(&tmp))?;
    let png_err = |e: png::EncodingError| ExportError::Io {
        path: out.to_path_buf(),
        source: std::io::Error::other(e),
    };
    let total = s.frame_count();
    {
        let mut enc = png::Encoder::new(BufWriter::new(file), s.width, s.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        // loops forever, as a GIF does
        enc.set_animated(total.max(1), 0).map_err(png_err)?;
        let (num, den) = frame_delay(fps);
        enc.set_frame_delay(num, den).map_err(png_err)?;
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc.write_header().map_err(png_err)?;
        for i in 0..total {
            if !progress(i, total) {
                return Err(ExportError::Cancelled);
            }
            let pixels = frames.get(i)?;
            writer.write_image_data(&pixels).map_err(png_err)?;
        }
        writer.finish().map_err(png_err)?;
    }
    temps.finish(&tmp, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_delays_fit_sixteen_bits() {
        assert_eq!(frame_delay(24.0), (100, 2400));
        assert_eq!(frame_delay(29.97), (100, 2997));
        // absurd rates are clamped rather than overflowing
        assert_eq!(frame_delay(1.0e9), (100, 65_500));
    }
}
