//! What an import simplified or left out, counted by kind and turned into
//! short phrases for the status bar and the import report.

use std::collections::BTreeMap;

/// A kind of loss: how to say one, and how to say several. Each reads
/// after a count: "1 mask left out", "3 masks left out".
pub type Kind = (&'static str, &'static str);

/// Counts of things that did not come through exactly.
#[derive(Debug, Default)]
pub struct Notes {
    counts: BTreeMap<&'static str, (&'static str, usize)>,
}

impl Notes {
    /// Note one more of `kind`.
    pub fn add(&mut self, kind: Kind) {
        self.counts.entry(kind.1).or_insert((kind.0, 0)).1 += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// One phrase per kind, with its count.
    pub fn phrases(&self) -> Vec<String> {
        self.counts
            .iter()
            .map(|(many, (one, n))| {
                if *n == 1 {
                    format!("1 {one}")
                } else {
                    format!("{n} {many}")
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_each_kind_in_words() {
        const MASK: Kind = ("mask left out", "masks left out");
        const TEXT: Kind = ("text item left out", "text items left out");
        let mut n = Notes::default();
        assert!(n.is_empty());
        n.add(MASK);
        n.add(MASK);
        n.add(TEXT);
        assert_eq!(n.phrases(), ["2 masks left out", "1 text item left out"]);
    }
}
