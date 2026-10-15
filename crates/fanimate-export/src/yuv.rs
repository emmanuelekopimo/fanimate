//! Rendered RGBA frames as the 4:2:0 YUV pictures video encoders take,
//! in BT.709 limited range: the HD video standard every player assumes.
//! Done here, once, so every encoder Fanimate drives gets the same colour.

/// Planar 4:2:0: full-size luma, then quarter-size blue and red
/// difference planes (I420). Sizes must be even.
pub struct I420 {
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

/// BT.709 coefficients, scaled to limited range (16..235, 16..240).
fn luma(r: f32, g: f32, b: f32) -> f32 {
    16.0 + 219.0 * (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
}

fn chroma(r: f32, g: f32, b: f32) -> (f32, f32) {
    let cb = 128.0 + 224.0 * (-0.114_572 * r - 0.385_428 * g + 0.5 * b) / 255.0;
    let cr = 128.0 + 224.0 * (0.5 * r - 0.454_153 * g - 0.045_847 * b) / 255.0;
    (cb, cr)
}

/// Convert `width` x `height` RGBA (transparency ignored, as video has
/// none) to I420. Each chroma sample averages a 2x2 block.
pub fn i420(rgba: &[u8], width: usize, height: usize) -> I420 {
    let (cw, ch) = (width / 2, height / 2);
    let mut y = Vec::with_capacity(width * height);
    for p in rgba.as_chunks::<4>().0 {
        y.push(luma(f32::from(p[0]), f32::from(p[1]), f32::from(p[2])).round() as u8);
    }
    let mut u = Vec::with_capacity(cw * ch);
    let mut v = Vec::with_capacity(cw * ch);
    for row in 0..ch {
        for col in 0..cw {
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let i = ((row * 2 + dy) * width + col * 2 + dx) * 4;
                r += f32::from(rgba[i]);
                g += f32::from(rgba[i + 1]);
                b += f32::from(rgba[i + 2]);
            }
            let (cb, cr) = chroma(r / 4.0, g / 4.0, b / 4.0);
            u.push(cb.round() as u8);
            v.push(cr.round() as u8);
        }
    }
    I420 { y, u, v }
}

impl I420 {
    /// The same picture as NV12: luma, then blue and red interleaved, as
    /// hardware and system encoders want it.
    pub fn nv12(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.y.len() + self.u.len() * 2);
        out.extend_from_slice(&self.y);
        for (u, v) in self.u.iter().zip(&self.v) {
            out.push(*u);
            out.push(*v);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_white_and_grey_are_limited_range_and_neutral() {
        let px = |r, g, b| [r, g, b, 255u8];
        let rgba: Vec<u8> = [px(0, 0, 0), px(255, 255, 255), px(0, 0, 0), px(255, 255, 255)].concat();
        let pic = i420(&rgba, 2, 2);
        assert_eq!(pic.y, [16, 235, 16, 235]);
        // grey on average: no colour difference
        assert_eq!((pic.u[0], pic.v[0]), (128, 128));
    }

    #[test]
    fn pure_red_has_the_bt709_values() {
        let rgba: Vec<u8> = [255u8, 0, 0, 255].repeat(4);
        let pic = i420(&rgba, 2, 2);
        // BT.709 limited-range red: Y 63, Cb 102, Cr 240
        assert_eq!((pic.y[0], pic.u[0], pic.v[0]), (63, 102, 240));
        assert_eq!(pic.nv12(), [63, 63, 63, 63, 102, 240]);
    }
}
