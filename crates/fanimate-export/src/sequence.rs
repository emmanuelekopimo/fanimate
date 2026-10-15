//! Picture sequences: one PNG, TIFF or OpenEXR file per frame, named after
//! the output (`scene.exr` gives `scene_0000.exr` and on), each written to
//! a `.part` file and renamed when complete.
//!
//! PNG and TIFF keep the rendered sRGB pixels as they are. OpenEXR is for
//! compositing, which works in linear light with premultiplied alpha, so
//! its frames are converted to that, in 32-bit float.

use std::path::{Path, PathBuf};

use crate::job::Format;
use crate::run::{ExportError, Frames, Result, Temps, part};

/// Frame `number`'s file, for a sequence written to `out`.
pub(crate) fn frame_file(out: &Path, format: Format, number: i32, digits: usize) -> PathBuf {
    let stem = out.file_stem().unwrap_or_default().to_string_lossy();
    out.with_file_name(format!("{stem}_{number:0digits$}.{}", format.extension()))
}

pub(crate) fn write(
    frames: &mut Frames,
    format: Format,
    total: u32,
    out: &Path,
    temps: &mut Temps,
    progress: &mut dyn FnMut(u32, u32) -> bool,
) -> Result<()> {
    let last = frames.start + total.saturating_sub(1) as i32;
    let digits = last.to_string().len().max(4);
    let (w, h) = frames.target.size();
    for i in 0..total {
        if !progress(i, total) {
            return Err(ExportError::Cancelled);
        }
        let pixels = frames.get(i)?;
        let file = frame_file(out, format, frames.start + i as i32, digits);
        let tmp = temps.add(part(&file));
        let saved = match format {
            Format::Exr => {
                let img = image::Rgba32FImage::from_raw(w, h, linear_premultiplied(&pixels))
                    .ok_or_else(|| ExportError::Render("a frame came back the wrong size".into()))?;
                image::DynamicImage::ImageRgba32F(img).save_with_format(&tmp, image::ImageFormat::OpenExr)
            }
            Format::Tiff => image::save_buffer_with_format(
                &tmp,
                &pixels,
                w,
                h,
                image::ExtendedColorType::Rgba8,
                image::ImageFormat::Tiff,
            ),
            _ => image::save_buffer_with_format(
                &tmp,
                &pixels,
                w,
                h,
                image::ExtendedColorType::Rgba8,
                image::ImageFormat::Png,
            ),
        };
        saved.map_err(|e| ExportError::Io {
            path: file.clone(),
            source: std::io::Error::other(e),
        })?;
        temps.finish(&tmp, &file)?;
    }
    Ok(())
}

/// sRGB-encoded, straight-alpha 8-bit pixels as linear-light,
/// premultiplied floats.
fn linear_premultiplied(rgba: &[u8]) -> Vec<f32> {
    // one lookup per channel value instead of a power per sample
    let table: Vec<f32> = (0..=255u8)
        .map(|v| {
            let c = f32::from(v) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
        .collect();
    rgba.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = f32::from(p[3]) / 255.0;
            [
                table[p[0] as usize] * a,
                table[p[1] as usize] * a,
                table[p[2] as usize] * a,
                a,
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_numbered_with_the_formats_extension() {
        let out = Path::new("renders/walk.mp4");
        assert_eq!(
            frame_file(out, Format::Exr, 7, 4),
            Path::new("renders/walk_0007.exr")
        );
        assert_eq!(
            frame_file(out, Format::Tiff, 12345, 4),
            Path::new("renders/walk_12345.tif")
        );
    }

    #[test]
    fn exr_pixels_are_linear_and_premultiplied() {
        let px = linear_premultiplied(&[255, 188, 0, 255, 255, 255, 255, 0, 255, 255, 255, 128]);
        // white stays 1, sRGB 188 is about half linear light, black stays 0
        assert!((px[0] - 1.0).abs() < 1e-6);
        assert!((px[1] - 0.5).abs() < 0.01, "{}", px[1]);
        assert_eq!(px[2], 0.0);
        // fully transparent: nothing left of the colour
        assert_eq!(&px[4..8], &[0.0, 0.0, 0.0, 0.0]);
        // half transparent white: colour scaled by alpha
        assert!((px[8] - px[11]).abs() < 1e-6 && (px[11] - 128.0 / 255.0).abs() < 1e-6);
    }
}
