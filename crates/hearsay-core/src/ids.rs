//! Identifiers and version stamps.
//!
//! Every decision row carries three version fields (classifier, ruleset,
//! pipeline). The evaluation harness refuses to aggregate rows whose versions
//! disagree, so these are values rather than strings scattered through the
//! code. See `design.md` §7.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identifies one inbound HTTP request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(Uuid);

impl RequestId {
    /// Generate a fresh request id.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifies one policy decision. One request produces exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecisionId(Uuid);

impl DecisionId {
    /// Generate a fresh decision id.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DecisionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DecisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifies a text region within a single request. Stable across the
/// pipeline so a classifier score, a policy verdict and a redaction all refer
/// to the same pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RegionId(pub u32);

impl fmt::Display for RegionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

/// A BLAKE3 hash of input bytes. Used as the cache key and the decision log
/// key, and rendered as `b3:<hex>` so a log row is greppable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    /// Wrap 32 raw hash bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The raw hash bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("b3:")?;
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl From<ContentHash> for String {
    fn from(hash: ContentHash) -> Self {
        hash.to_string()
    }
}

/// Failure to parse a [`ContentHash`] from its string form.
#[derive(Debug, thiserror::Error)]
pub enum ContentHashParseError {
    /// The string did not start with the `b3:` prefix.
    #[error("content hash must start with `b3:`")]
    MissingPrefix,
    /// The hex body was not exactly 64 characters.
    #[error("content hash must have 64 hex digits, got {0}")]
    WrongLength(usize),
    /// The hex body contained a non-hex character.
    #[error("content hash contains a non-hex character")]
    NotHex,
}

impl TryFrom<String> for ContentHash {
    type Error = ContentHashParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let hex = value
            .strip_prefix("b3:")
            .ok_or(ContentHashParseError::MissingPrefix)?;
        if hex.len() != 64 {
            return Err(ContentHashParseError::WrongLength(hex.len()));
        }
        let mut out = [0u8; 32];
        for (i, slot) in out.iter_mut().enumerate() {
            let pair = &hex[i * 2..i * 2 + 2];
            *slot = u8::from_str_radix(pair, 16).map_err(|_| ContentHashParseError::NotHex)?;
        }
        Ok(Self(out))
    }
}

/// Version of a fine-tuned classifier artefact, e.g.
/// `deberta-v3-base-hearsay@7a1c`. The suffix is the content hash of the ONNX
/// file, verified at load.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelVersion(pub String);

impl fmt::Display for ModelVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Version of the policy ruleset, e.g. `v3@e81f`. Changing any threshold
/// produces a new version, so no reported result can silently belong to a
/// different policy than it claims.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RulesetVersion(pub String);

impl fmt::Display for RulesetVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identifies an OCR engine and its version, e.g. `tesseract@5.3.4`. Part of
/// the extraction cache key so an engine swap cannot serve stale regions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EngineId(pub String);

impl fmt::Display for EngineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_round_trips_through_its_string_form() {
        let hash = ContentHash::from_bytes([0xab; 32]);
        let rendered = hash.to_string();
        assert!(rendered.starts_with("b3:"));
        assert_eq!(rendered.len(), 67);
        let parsed = ContentHash::try_from(rendered).expect("round trip");
        assert_eq!(parsed, hash);
    }

    #[test]
    fn content_hash_rejects_malformed_strings() {
        assert!(ContentHash::try_from("9f2c".to_string()).is_err());
        assert!(ContentHash::try_from(format!("b3:{}", "z".repeat(64))).is_err());
        assert!(ContentHash::try_from(format!("b3:{}", "a".repeat(63))).is_err());
    }

    #[test]
    fn content_hash_serialises_as_a_json_string_not_a_byte_array() {
        let hash = ContentHash::from_bytes([0x01; 32]);
        let json = serde_json::to_string(&hash).expect("serialise");
        assert!(json.starts_with("\"b3:"), "got {json}");
    }
}
