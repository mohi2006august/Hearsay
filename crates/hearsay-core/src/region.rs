//! Text regions extracted from an image, and the health of that extraction.
//!
//! [`ExtractionOutcome`] exists because of the first gotcha in `brain.md`: a
//! missed region looks like a defence failure but is an extraction failure.
//! The two are only separable in the results if extraction declares its own
//! confidence, so it is a required field rather than an optional one.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ids::RegionId;
use crate::provenance::Origin;

/// An axis-aligned box in the *source* image's pixel space, before any resize
/// the OCR engine performs internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BBox {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

impl BBox {
    /// Area in pixels. `u64` because a 4096×4096 image overflows `u32` when
    /// multiplied out at the limits allowed in `design.md` §6.1.
    pub fn area(&self) -> u64 {
        u64::from(self.w) * u64::from(self.h)
    }

    /// This box's share of the image, used by the policy engine's
    /// `max_redact_frac` check.
    ///
    /// Returns `None` for a zero-area image rather than dividing by zero: the
    /// caller must treat an unmeasurable region as unredactable, which fails
    /// closed.
    // Precision is ample: both values are bounded by 4096*4096, well inside
    // f32's exactly-representable integer range.
    #[allow(clippy::cast_precision_loss)]
    pub fn fraction_of(&self, image_area: u64) -> Option<f32> {
        if image_area == 0 {
            return None;
        }
        Some(self.area() as f32 / image_area as f32)
    }
}

/// One run of text OCR found in an image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextRegion {
    /// Stable within a request.
    pub id: RegionId,
    /// Where in the source image this text sits.
    pub bbox: BBox,
    /// Exactly what the OCR engine returned, before normalisation.
    ///
    /// Kept verbatim: the report needs to show what an attacker wrote, not
    /// what the normaliser turned it into.
    pub raw: String,
    /// The engine's own confidence, in `0.0..=1.0`.
    pub ocr_confidence: f32,
    /// Which input this region came out of.
    pub source: Origin,
}

/// Why an extraction is considered degraded.
///
/// These are the conditions under which OCR recall is known to fall, and each
/// one escalates the policy thresholds (rule `R3`). They are reported
/// separately in the results so an ASR number is never inflated by an
/// extraction problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DegradeReason {
    /// Text/background contrast below the engine's reliable range. The silent
    /// failure mode named in `brain.md`.
    LowContrast,
    /// Glyphs too small at the source resolution to recognise reliably.
    TinyGlyphs,
    /// Text rotated beyond what the detector handles.
    Rotation,
    /// The engine did not finish within its budget and returned partial
    /// results.
    Timeout,
    /// The engine reported that it stopped before covering the whole image.
    PartialPage,
}

/// How well extraction went. Not an error type — a `Failed` extraction is a
/// normal, expected outcome that the policy engine has a rule for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ExtractionOutcome {
    /// The engine covered the image and is confident in its coverage.
    Complete,
    /// The engine returned results but flagged a condition that depresses
    /// recall.
    Degraded {
        /// What went wrong.
        reason: DegradeReason,
    },
    /// The engine returned nothing usable.
    ///
    /// Carries a `String` rather than the OCR crate's error type: `hearsay-ocr`
    /// depends on this crate, so naming its error here would close a
    /// dependency cycle. The engine stringifies at the boundary.
    Failed {
        /// The engine's error, rendered.
        reason: String,
    },
}

impl ExtractionOutcome {
    /// Whether any region list from this extraction can be trusted to be
    /// complete.
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }

    /// Whether extraction produced no usable regions at all.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

/// The full result of running OCR over one image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extraction {
    /// Regions found, possibly empty even when `outcome` is `Complete` — an
    /// image with no text is the common benign case.
    pub regions: Vec<TextRegion>,
    /// How well the extraction went.
    pub outcome: ExtractionOutcome,
    /// Wall-clock time the engine took, for the latency table in `design.md` §9.
    pub elapsed: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_fraction_is_none_for_a_zero_area_image() {
        let b = BBox {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        };
        assert_eq!(b.fraction_of(0), None);
    }

    #[test]
    fn bbox_fraction_matches_the_hand_computed_share() {
        let b = BBox {
            x: 0,
            y: 0,
            w: 100,
            h: 50,
        };
        let frac = b.fraction_of(10_000).expect("non-zero image");
        assert!((frac - 0.5).abs() < f32::EPSILON, "got {frac}");
    }

    #[test]
    fn bbox_area_does_not_overflow_at_the_documented_size_limit() {
        let b = BBox {
            x: 0,
            y: 0,
            w: 4096,
            h: 4096,
        };
        assert_eq!(b.area(), 16_777_216);
    }

    #[test]
    fn outcome_predicates_agree_with_their_variants() {
        assert!(ExtractionOutcome::Complete.is_complete());
        assert!(!ExtractionOutcome::Complete.is_failed());

        let degraded = ExtractionOutcome::Degraded {
            reason: DegradeReason::LowContrast,
        };
        assert!(!degraded.is_complete());
        assert!(!degraded.is_failed());

        let failed = ExtractionOutcome::Failed {
            reason: "engine panicked".to_string(),
        };
        assert!(!failed.is_complete());
        assert!(failed.is_failed());
    }
}
