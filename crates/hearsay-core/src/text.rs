//! Normalised text, with the mapping back to its source preserved.
//!
//! Normalisation has to be offset-preserving or a flagged region cannot be
//! redacted — you would know *that* something was an injection but not *where*
//! on the image it was. See `design.md` §6.3.

use serde::{Deserialize, Serialize};

use crate::ids::RegionId;

/// Construction of a [`NormalizedText`] whose offset map does not describe its
/// text.
#[derive(Debug, thiserror::Error)]
pub enum TextError {
    /// One offset entry per byte of normalised text is required.
    #[error("offset map has {offsets} entries for {bytes} bytes of text")]
    OffsetMapLengthMismatch {
        /// Entries supplied.
        offsets: usize,
        /// Bytes of normalised text.
        bytes: usize,
    },
    /// Normalisation may merge or drop source bytes but never reorder them.
    #[error("offset map is not monotonic: entry {index} ({value}) precedes entry {prev_index} ({prev_value})")]
    OffsetMapNotMonotonic {
        /// Index of the offending entry.
        index: usize,
        /// Its value.
        value: u32,
        /// Index of the preceding entry.
        prev_index: usize,
        /// Its value.
        prev_value: u32,
    },
}

/// Text after the normalisation pipeline, carrying a byte-for-byte map back to
/// the region it came from.
///
/// The map is one `u32` per normalised byte. That is wasteful next to a
/// run-length encoding, and deliberately so for now: normalisation is not on
/// the hot path (1 ms of a 363 ms budget) and a dense map is far easier to
/// assert about. Revisit if the profile says otherwise, not before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedText {
    text: String,
    offsets: Vec<u32>,
    source: RegionId,
}

impl NormalizedText {
    /// Build a normalised text, validating the offset map against the text.
    ///
    /// # Errors
    ///
    /// Returns [`TextError`] if the map has the wrong length or is not
    /// monotonic non-decreasing.
    pub fn new(text: String, offsets: Vec<u32>, source: RegionId) -> Result<Self, TextError> {
        if offsets.len() != text.len() {
            return Err(TextError::OffsetMapLengthMismatch {
                offsets: offsets.len(),
                bytes: text.len(),
            });
        }
        for (i, pair) in offsets.windows(2).enumerate() {
            if pair[1] < pair[0] {
                return Err(TextError::OffsetMapNotMonotonic {
                    index: i + 1,
                    value: pair[1],
                    prev_index: i,
                    prev_value: pair[0],
                });
            }
        }
        Ok(Self {
            text,
            offsets,
            source,
        })
    }

    /// Build a normalised text for the identity transform, where every byte
    /// maps to itself.
    ///
    /// Useful in tests and for the no-op path when a region needs no
    /// normalisation.
    ///
    /// # Panics
    ///
    /// Panics if `text` is longer than `u32::MAX` bytes, which the request
    /// size limits in `design.md` §6.1 make unreachable.
    pub fn identity(text: String, source: RegionId) -> Self {
        let offsets = (0..text.len())
            .map(|i| u32::try_from(i).expect("text longer than u32::MAX bytes"))
            .collect();
        Self {
            text,
            offsets,
            source,
        }
    }

    /// The normalised text the classifier scores.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The region this text was extracted from.
    pub fn source(&self) -> RegionId {
        self.source
    }

    /// Map a byte offset in the normalised text back to a byte offset in the
    /// source region's `raw` string.
    ///
    /// Returns `None` for an out-of-range offset.
    pub fn source_offset(&self, normalized_offset: usize) -> Option<u32> {
        self.offsets.get(normalized_offset).copied()
    }

    /// Whether the normalised text is empty. An empty region is scored as
    /// benign rather than skipped, so the region still appears in the log.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_maps_every_byte_to_itself() {
        let t = NormalizedText::identity("abc".to_string(), RegionId(0));
        assert_eq!(t.source_offset(0), Some(0));
        assert_eq!(t.source_offset(2), Some(2));
        assert_eq!(t.source_offset(3), None);
    }

    #[test]
    fn a_map_shorter_than_the_text_is_rejected() {
        let err = NormalizedText::new("abcd".to_string(), vec![0, 1, 2], RegionId(0));
        assert!(matches!(
            err,
            Err(TextError::OffsetMapLengthMismatch {
                offsets: 3,
                bytes: 4
            })
        ));
    }

    #[test]
    fn a_non_monotonic_map_is_rejected() {
        // Normalisation may collapse bytes but must never reorder them; a
        // map that goes backwards means the redaction would point at the
        // wrong pixels.
        let err = NormalizedText::new("abc".to_string(), vec![0, 5, 3], RegionId(0));
        assert!(matches!(
            err,
            Err(TextError::OffsetMapNotMonotonic { index: 2, .. })
        ));
    }

    #[test]
    fn a_collapsing_map_is_accepted_because_normalisation_merges_bytes() {
        // "  a" -> " a": two source spaces collapse to one normalised byte,
        // so repeated offsets are legitimate.
        let t = NormalizedText::new(" a".to_string(), vec![0, 2], RegionId(7))
            .expect("collapsing maps are monotonic");
        assert_eq!(t.source_offset(1), Some(2));
        assert_eq!(t.source(), RegionId(7));
    }
}
