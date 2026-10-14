//! A lenient reader for ZIP archives, the container of `.moho` and `.fla`
//! files.
//!
//! Animate writes archives whose end record misstates the size of the
//! central directory, which strict readers refuse. This reader walks the
//! central directory entry by entry instead, checks each against its local
//! header, and only needs stored and deflated entries, which is all either
//! app writes. Damaged archives give an error, never a panic.

use std::collections::BTreeMap;

const LOCAL: &[u8; 4] = b"PK\x03\x04";
const CENTRAL: &[u8; 4] = b"PK\x01\x02";
const END: &[u8; 4] = b"PK\x05\x06";

/// The largest entry this reader inflates. Project files are far smaller;
/// the limit stops a hostile archive from exhausting memory.
const MAX_ENTRY: usize = 512 * 1024 * 1024;

/// An archive's entries by name. Entries are inflated when read.
pub struct Archive<'a> {
    bytes: &'a [u8],
    entries: BTreeMap<String, Entry>,
}

#[derive(Clone, Copy)]
struct Entry {
    method: u16,
    compressed: usize,
    size: usize,
    data: usize,
}

fn u16_at(b: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(u16::from_le_bytes(
        b.get(at..at.checked_add(2)?)?.try_into().ok()?,
    )))
}

fn u32_at(b: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?) as usize)
}

/// Whether `bytes` start like a ZIP archive.
pub fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(LOCAL)
}

impl<'a> Archive<'a> {
    pub fn read(bytes: &'a [u8]) -> Result<Self, String> {
        const NO_TABLE: &str = "the archive has no table of contents";
        const CUT: &str = "the archive's table of contents is cut short";
        let end = bytes.windows(4).rposition(|w| w == END).ok_or(NO_TABLE)?;
        let mut at = u32_at(bytes, end + 16).ok_or(CUT)?;
        if bytes.get(at..at + 4) != Some(CENTRAL) {
            // the stated offset is wrong: use the first directory entry
            at = bytes.windows(4).position(|w| w == CENTRAL).ok_or(NO_TABLE)?;
        }
        let mut entries = BTreeMap::new();
        while bytes.get(at..at + 4) == Some(CENTRAL) {
            let fields = (
                u16_at(bytes, at + 10),
                u32_at(bytes, at + 20),
                u32_at(bytes, at + 24),
                u16_at(bytes, at + 28),
                u16_at(bytes, at + 30),
                u16_at(bytes, at + 32),
                u32_at(bytes, at + 42),
            );
            let (
                Some(method),
                Some(compressed),
                Some(size),
                Some(name_len),
                Some(extra),
                Some(comment),
                Some(local),
            ) = fields
            else {
                return Err(CUT.into());
            };
            let name = bytes.get(at + 46..at + 46 + name_len).ok_or(CUT)?;
            let name = String::from_utf8_lossy(name).replace('\\', "/");
            at += 46 + name_len + extra + comment;
            if name.ends_with('/') || bytes.get(local..local.saturating_add(4)) != Some(LOCAL) {
                continue;
            }
            let (Some(local_name), Some(local_extra)) =
                (u16_at(bytes, local + 26), u16_at(bytes, local + 28))
            else {
                continue;
            };
            let entry = Entry {
                method: method as u16,
                compressed,
                size,
                data: local + 30 + local_name + local_extra,
            };
            entries.insert(name, entry);
        }
        if entries.is_empty() {
            return Err("the archive is empty or damaged".into());
        }
        Ok(Self { bytes, entries })
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// The entry called `name` (compared without regard to case, as both
    /// apps run on case-insensitive file systems), inflated.
    pub fn get(&self, name: &str) -> Option<Result<Vec<u8>, String>> {
        let entry = self
            .entries
            .get(name)
            .or_else(|| {
                self.entries
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(name))
                    .map(|(_, e)| e)
            })
            .copied()?;
        Some(self.inflate(entry))
    }

    fn inflate(&self, e: Entry) -> Result<Vec<u8>, String> {
        let raw = self
            .bytes
            .get(e.data..e.data.saturating_add(e.compressed))
            .ok_or("an archive entry is cut short")?;
        match e.method {
            0 => Ok(raw.to_vec()),
            8 => {
                let limit = if e.size > 0 {
                    e.size.min(MAX_ENTRY)
                } else {
                    MAX_ENTRY
                };
                miniz_oxide::inflate::decompress_to_vec_with_limit(raw, limit)
                    .map_err(|_| "an archive entry is damaged".to_owned())
            }
            m => Err(format!(
                "an archive entry uses compression method {m}, which is not supported"
            )),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A stored-only archive, for tests.
    pub fn build(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in files {
            let offset = out.len() as u32;
            let header = |sig: &[u8; 4], central: bool| {
                let mut h = sig.to_vec();
                if central {
                    h.extend_from_slice(&20u16.to_le_bytes());
                }
                // version, flags, method (stored), time, date, crc
                h.extend_from_slice(&20u16.to_le_bytes());
                h.extend_from_slice(&[0; 12]);
                h.extend_from_slice(&(data.len() as u32).to_le_bytes());
                h.extend_from_slice(&(data.len() as u32).to_le_bytes());
                h.extend_from_slice(&(name.len() as u16).to_le_bytes());
                h.extend_from_slice(&0u16.to_le_bytes());
                if central {
                    // comment length, disk, attributes, local header offset
                    h.extend_from_slice(&[0; 10]);
                    h.extend_from_slice(&offset.to_le_bytes());
                }
                h.extend_from_slice(name.as_bytes());
                h
            };
            out.extend(header(LOCAL, false));
            out.extend_from_slice(data);
            central.extend(header(CENTRAL, true));
        }
        let at = out.len() as u32;
        out.extend_from_slice(&central);
        out.extend_from_slice(END);
        out.extend_from_slice(&[0; 12]);
        out.extend_from_slice(&at.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    #[test]
    fn reads_stored_entries_case_insensitively() {
        let zip = build(&[("DOMDocument.xml", b"<x/>"), ("LIBRARY/", b"")]);
        let a = Archive::read(&zip).unwrap();
        assert_eq!(a.names().collect::<Vec<_>>(), ["DOMDocument.xml"]);
        assert_eq!(a.get("domdocument.xml").unwrap().unwrap(), b"<x/>");
        assert!(a.get("missing").is_none());
        assert!(is_zip(&zip));
    }

    #[test]
    fn a_wrong_directory_offset_is_recovered() {
        let mut zip = build(&[("a", b"1")]);
        let n = zip.len();
        zip[n - 6..n - 2].copy_from_slice(&999u32.to_le_bytes());
        assert_eq!(Archive::read(&zip).unwrap().get("a").unwrap().unwrap(), b"1");
    }

    #[test]
    fn damaged_archives_are_errors_not_panics() {
        assert!(Archive::read(b"PK\x03\x04garbage").is_err());
        let zip = build(&[("a", b"12345"), ("b", b"678")]);
        for cut in 0..zip.len() {
            if let Ok(a) = Archive::read(&zip[..cut]) {
                for name in ["a", "b"] {
                    let _ = a.get(name);
                }
            }
        }
        let mut flipped = zip.clone();
        for i in 0..flipped.len() {
            flipped[i] ^= 0xff;
            if let Ok(a) = Archive::read(&flipped) {
                let _ = a.get("a");
            }
            flipped[i] ^= 0xff;
        }
    }
}
