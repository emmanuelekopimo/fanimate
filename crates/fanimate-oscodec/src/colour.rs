//! Colour tags for the MP4 files Windows writes.
//!
//! Media Foundation's H.264 encoders leave the colour description out of
//! the stream, whatever the media type says, and do not take it through
//! their codec settings either. Players then guess, most often BT.601 for
//! pictures under 720 lines, which shifts every colour of frames converted
//! as BT.709. Once the file is finished this rewrites the sequence
//! parameter set in the `avcC` box to say BT.709 limited range, and adds a
//! `colr` box, which QuickTime reads. Windows keeps the parameter sets only
//! there and writes `moov` last, so no sample moves; a file laid out any
//! other way is left as it is, still playable, just untagged.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// BT.709 primaries, transfer and matrix, numbered alike in H.264 and MP4.
const BT709: u32 = 1;

/// Tag the MP4 file at `path` as BT.709 limited range. Returns whether it
/// was changed.
pub fn tag_bt709(path: &Path) -> std::io::Result<bool> {
    let mut f = File::options().read(true).write(true).open(path)?;
    let len = f.metadata()?.len();
    let mut at = 0u64;
    while at < len {
        f.seek(SeekFrom::Start(at))?;
        let mut h = [0u8; 16];
        f.read_exact(&mut h[..8])?;
        let size = match be32(&h[0..4]) {
            0 => len - at,
            1 => {
                f.read_exact(&mut h[8..16])?;
                u64::from_be_bytes([h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]])
            }
            n => u64::from(n),
        };
        if size < 8 || at + size > len {
            return Ok(false);
        }
        if &h[4..8] == b"moov" {
            if at + size != len {
                return Ok(false);
            }
            let mut moov = vec![0; size as usize];
            f.seek(SeekFrom::Start(at))?;
            f.read_exact(&mut moov)?;
            let header = if be32(&moov[0..4]) == 1 { 16 } else { 8 };
            let Some(body) = patch(b"moov", &moov[header..]) else {
                return Ok(false);
            };
            let mut out = Vec::with_capacity(body.len() + 8);
            if push_box(&mut out, b"moov", &body).is_none() {
                return Ok(false);
            }
            f.seek(SeekFrom::Start(at))?;
            f.write_all(&out)?;
            f.set_len(at + out.len() as u64)?;
            f.sync_all()?;
            return Ok(true);
        }
        at += size;
    }
    Ok(false)
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// Where a box's children start in its body, for the boxes on the way to
/// `avcC`.
fn children_offset(kind: &[u8; 4]) -> Option<usize> {
    match kind {
        b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" => Some(0),
        // version, flags and the entry count
        b"stsd" => Some(8),
        // the visual sample entry's own fields
        b"avc1" => Some(78),
        _ => None,
    }
}

/// The body of a `kind` box with its colour tags set, or `None` when there
/// was nothing to change (or it could not be read).
fn patch(kind: &[u8; 4], body: &[u8]) -> Option<Vec<u8>> {
    if kind == b"avcC" {
        return patch_avcc(body);
    }
    let skip = children_offset(kind)?;
    let mut out = body.get(..skip)?.to_vec();
    let (mut changed, mut has_colr) = (false, false);
    let mut at = skip;
    while at < body.len() {
        let rest = &body[at..];
        if rest.len() < 8 {
            return None;
        }
        let (size, header) = match be32(&rest[0..4]) {
            0 => (rest.len(), 8),
            1 if rest.len() >= 16 => (
                usize::try_from(u64::from_be_bytes(rest[8..16].try_into().ok()?)).ok()?,
                16,
            ),
            n => (n as usize, 8),
        };
        if size < header || size > rest.len() {
            return None;
        }
        let child: [u8; 4] = rest[4..8].try_into().ok()?;
        has_colr |= &child == b"colr";
        match patch(&child, &rest[header..size]) {
            Some(new) => {
                push_box(&mut out, &child, &new)?;
                changed = true;
            }
            None => out.extend_from_slice(&rest[..size]),
        }
        at += size;
    }
    if kind == b"avc1" && changed && !has_colr {
        let mut colr = b"nclx".to_vec();
        for _ in 0..3 {
            colr.extend_from_slice(&(BT709 as u16).to_be_bytes());
        }
        // full_range_flag clear: limited range
        colr.push(0);
        push_box(&mut out, b"colr", &colr)?;
    }
    changed.then_some(out)
}

fn push_box(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) -> Option<()> {
    let size = u32::try_from(body.len() + 8).ok()?;
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    Some(())
}

/// An `avcC` body with every sequence parameter set tagged.
fn patch_avcc(body: &[u8]) -> Option<Vec<u8>> {
    let count = usize::from(*body.get(5)? & 0x1f);
    if count == 0 {
        return None;
    }
    let mut out = body[..6].to_vec();
    let mut at = 6;
    for _ in 0..count {
        let len = usize::from(u16::from_be_bytes(body.get(at..at + 2)?.try_into().ok()?));
        let sps = tag_sps(body.get(at + 2..at + 2 + len)?)?;
        out.extend_from_slice(&u16::try_from(sps.len()).ok()?.to_be_bytes());
        out.extend_from_slice(&sps);
        at += 2 + len;
    }
    out.extend_from_slice(body.get(at..)?);
    (out != body).then_some(out)
}

/// A sequence parameter set NAL unit whose VUI says BT.709 limited range.
fn tag_sps(nal: &[u8]) -> Option<Vec<u8>> {
    let (&header, rest) = nal.split_first()?;
    if header & 0x1f != 7 {
        return None;
    }
    let rbsp = unescape(rest);
    let mut r = to_vui(&rbsp)?;
    let mut w = BitWriter::default();
    w.copy(&rbsp, 0, r.pos);
    w.put(1, 1);
    if r.u(1)? == 0 {
        // no VUI: one with only the colour description
        w.put(0, 2); // aspect ratio, overscan
        signal_type(&mut w);
        w.put(0, 6); // chroma location, timing, both HRDs, picture structure, restrictions
        w.put(1, 1); // stop bit
    } else {
        let aspect = r.u(1)?;
        w.put(aspect, 1);
        if aspect == 1 {
            let idc = r.u(8)?;
            w.put(idc, 8);
            if idc == 255 {
                w.put(r.u(32)?, 32);
            }
        }
        let overscan = r.u(1)?;
        w.put(overscan, 1);
        if overscan == 1 {
            w.put(r.u(1)?, 1);
        }
        if r.u(1)? == 1 {
            r.u(4)?; // video format, full range
            if r.u(1)? == 1 {
                r.u(24)?;
            }
        }
        signal_type(&mut w);
        // the rest as it was, up to and including the stop bit
        let total = rbsp.len() * 8;
        let stop = (r.pos..total).rev().find(|&i| bit(&rbsp, i))?;
        w.copy(&rbsp, r.pos, stop + 1);
    }
    let mut out = vec![header];
    out.extend(escape(&w.finish()));
    Some(out)
}

/// video_signal_type: unspecified format, limited range, BT.709.
fn signal_type(w: &mut BitWriter) {
    w.put(1, 1);
    w.put(5, 3);
    w.put(0, 1);
    w.put(1, 1);
    for _ in 0..3 {
        w.put(BT709, 8);
    }
}

/// A reader over a sequence parameter set's payload, at
/// `vui_parameters_present_flag`.
fn to_vui(rbsp: &[u8]) -> Option<Bits<'_>> {
    let mut r = Bits { data: rbsp, pos: 0 };
    let profile = r.u(8)?;
    r.u(16)?; // constraint flags, level
    r.ue()?; // seq_parameter_set_id
    if matches!(
        profile,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        if r.ue()? == 3 {
            r.u(1)?;
        }
        r.ue()?;
        r.ue()?;
        r.u(1)?;
        // scaling matrices, which Windows never writes
        if r.u(1)? == 1 {
            return None;
        }
    }
    r.ue()?; // log2_max_frame_num
    match r.ue()? {
        0 => {
            r.ue()?;
        }
        1 => {
            r.u(1)?;
            r.ue()?;
            r.ue()?;
            for _ in 0..r.ue()? {
                r.ue()?;
            }
        }
        _ => {}
    }
    r.ue()?; // max_num_ref_frames
    r.u(1)?;
    r.ue()?; // width
    r.ue()?; // height
    if r.u(1)? == 0 {
        r.u(1)?;
    }
    r.u(1)?;
    if r.u(1)? == 1 {
        for _ in 0..4 {
            r.ue()?;
        }
    }
    Some(r)
}

fn bit(data: &[u8], i: usize) -> bool {
    data[i / 8] >> (7 - i % 8) & 1 == 1
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn u(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            if self.pos >= self.data.len() * 8 {
                return None;
            }
            v = (v << 1) | u32::from(bit(self.data, self.pos));
            self.pos += 1;
        }
        Some(v)
    }

    /// Exp-Golomb; signed values read the same way, which is all skipping needs.
    fn ue(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while self.u(1)? == 0 {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        Some((1u32 << zeros) - 1 + self.u(zeros)?)
    }
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    len: usize,
}

impl BitWriter {
    fn put(&mut self, value: u32, n: u32) {
        for i in (0..n).rev() {
            self.push(value >> i & 1 == 1);
        }
    }

    fn push(&mut self, one: bool) {
        if self.len.is_multiple_of(8) {
            self.bytes.push(0);
        }
        if one {
            let last = self.bytes.len() - 1;
            self.bytes[last] |= 0x80 >> (self.len % 8);
        }
        self.len += 1;
    }

    fn copy(&mut self, data: &[u8], from: usize, to: usize) {
        for i in from..to {
            self.push(bit(data, i));
        }
    }

    /// The bytes, the last one padded with zero bits.
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// The payload without its emulation prevention bytes.
fn unescape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut zeros = 0;
    for &b in data {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        out.push(b);
    }
    out
}

/// The payload with emulation prevention bytes, so it never looks like a
/// start code.
fn escape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 4);
    let mut zeros = 0;
    for &b in data {
        if zeros >= 2 && b <= 3 {
            out.push(3);
            zeros = 0;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        out.push(b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The parameter set Windows' H.264 encoder wrote for a 640x360 file:
    /// a VUI with timing but no colour.
    const WINDOWS_SPS: &str = "2764001eac2b405017fcb80a0400000fa00002ee03a10010e000043626f7be0ed0e175";

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// The colour description (primaries, transfer, matrix), if any.
    fn colour(nal: &[u8]) -> Option<(u32, u32, u32)> {
        let rbsp = unescape(&nal[1..]);
        let mut r = to_vui(&rbsp).unwrap();
        if r.u(1)? == 0 {
            return None;
        }
        if r.u(1)? == 1 && r.u(8)? == 255 {
            r.u(32);
        }
        if r.u(1)? == 1 {
            r.u(1);
        }
        if r.u(1)? == 0 {
            return None;
        }
        r.u(4);
        if r.u(1)? == 0 {
            return None;
        }
        Some((r.u(8)?, r.u(8)?, r.u(8)?))
    }

    #[test]
    fn a_windows_parameter_set_gains_bt709() {
        let sps = hex(WINDOWS_SPS);
        assert_eq!(colour(&sps), None, "untagged to begin with");
        let tagged = tag_sps(&sps).unwrap();
        assert_eq!(colour(&tagged), Some((1, 1, 1)));
        // tagging again changes nothing, so the rest of the VUI survived
        assert_eq!(tag_sps(&tagged).unwrap(), tagged);
        assert!(!tagged.windows(3).any(|w| w == [0, 0, 1]), "no start code inside");
    }

    #[test]
    fn a_parameter_set_without_vui_gains_one() {
        let mut w = BitWriter::default();
        w.put(66, 8); // baseline
        w.put(0, 16);
        for v in [0u32, 0, 0, 0, 1] {
            put_ue(&mut w, v);
        }
        w.put(0, 1);
        put_ue(&mut w, 39);
        put_ue(&mut w, 21);
        w.put(0b110, 3); // frame_mbs_only, direct_8x8, no cropping
        w.put(0, 1); // no VUI
        w.put(1, 1);
        let mut sps = vec![0x67];
        sps.extend(escape(&w.finish()));
        let tagged = tag_sps(&sps).unwrap();
        assert_eq!(colour(&tagged), Some((1, 1, 1)));
        assert_eq!(tag_sps(&tagged).unwrap(), tagged);
    }

    fn put_ue(w: &mut BitWriter, v: u32) {
        let n = 32 - (v + 1).leading_zeros();
        w.put(0, n - 1);
        w.put(v + 1, n);
    }

    #[test]
    fn escaping_round_trips() {
        let raw = [0, 0, 0, 1, 0, 0, 3, 0, 0, 2, 5];
        let e = escape(&raw);
        assert_eq!(e, [0, 0, 3, 0, 1, 0, 0, 3, 3, 0, 0, 3, 2, 5]);
        assert_eq!(unescape(&e), raw);
    }

    fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        push_box(&mut v, kind, body).unwrap();
        v
    }

    /// ftyp, mdat, then moov down to one `avcC`, as Windows lays it out.
    fn file(moov_last: bool) -> Vec<u8> {
        let sps = hex(WINDOWS_SPS);
        let mut avcc = vec![1, 0x64, 0, 0x1e, 0xff, 0xe1];
        avcc.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        avcc.extend_from_slice(&sps);
        avcc.extend_from_slice(&[1, 0, 4, 0x28, 0xee, 0x3c, 0xb0]);
        let mut avc1 = vec![7; 78];
        avc1.extend(mp4_box(b"avcC", &avcc));
        let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
        stsd.extend(mp4_box(b"avc1", &avc1));
        let mut stbl = mp4_box(b"stsd", &stsd);
        stbl.extend(mp4_box(b"stco", &[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 16]));
        let trak = mp4_box(
            b"trak",
            &mp4_box(b"mdia", &mp4_box(b"minf", &mp4_box(b"stbl", &stbl))),
        );
        let mut moov = mp4_box(b"mvhd", &[0; 100]);
        moov.extend(trak);
        let mut f = mp4_box(b"ftyp", b"mp42isom");
        let mdat = mp4_box(b"mdat", &[9; 40]);
        let moov = mp4_box(b"moov", &moov);
        if moov_last {
            f.extend(mdat);
            f.extend(moov);
        } else {
            f.extend(moov);
            f.extend(mdat);
        }
        f
    }

    /// Every box's size adds up, all the way down to `avcC`.
    fn check_sizes(b: &[u8], kind: &[u8; 4]) {
        let skip = children_offset(kind).unwrap_or(0);
        let mut at = skip;
        while at < b.len() {
            let size = be32(&b[at..]) as usize;
            assert!(
                size >= 8 && at + size <= b.len(),
                "a {:?} box fits",
                std::str::from_utf8(kind)
            );
            let child: [u8; 4] = b[at + 4..at + 8].try_into().unwrap();
            if children_offset(&child).is_some() {
                check_sizes(&b[at + 8..at + size], &child);
            }
            at += size;
        }
    }

    #[test]
    fn files_are_tagged_in_place() {
        let dir = std::env::temp_dir().join(format!("fanimate-colour-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.mp4");
        let before = file(true);
        std::fs::write(&path, &before).unwrap();
        assert!(tag_bt709(&path).unwrap());
        let after = std::fs::read(&path).unwrap();
        check_sizes(&after, b"moov");
        // ftyp and mdat untouched, so every sample offset still holds
        assert_eq!(after[..16 + 48], before[..16 + 48]);
        assert!(after.windows(8).any(|w| w == b"colrnclx"));
        let at = after.windows(4).position(|w| w == b"avcC").unwrap();
        let len = u16::from_be_bytes([after[at + 10], after[at + 11]]) as usize;
        assert_eq!(colour(&after[at + 12..at + 12 + len]), Some((1, 1, 1)));
        // once is enough
        assert!(!tag_bt709(&path).unwrap());

        // a layout Windows does not write is left alone
        std::fs::write(&path, file(false)).unwrap();
        assert!(!tag_bt709(&path).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), file(false));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
