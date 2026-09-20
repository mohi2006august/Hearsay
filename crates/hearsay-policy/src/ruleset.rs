//! Thresholds and the lexicon, plus the validation that makes a misconfigured
//! ruleset a load-time failure rather than a silently inverted policy.

use hearsay_core::RulesetVersion;
use serde::{Deserialize, Serialize};

/// A ruleset that does not describe a coherent policy.
#[derive(Debug, thiserror::Error)]
pub enum RulesetError {
    /// A threshold fell outside `0.0..=1.0`.
    #[error("{field} must be in 0.0..=1.0, got {value}")]
    OutOfRange {
        /// Which field.
        field: &'static str,
        /// What it was set to.
        value: f32,
    },
    /// The flag threshold sits above the block threshold, which would make
    /// `R6` unreachable and silently disable flagging.
    #[error("tau_flag ({flag}) must not exceed tau_block ({block})")]
    FlagAboveBlock {
        /// The flag threshold.
        flag: f32,
        /// The block threshold.
        block: f32,
    },
    /// The degraded threshold sits above the normal one, which would make
    /// `R3` an de-escalation — the opposite of its stated purpose.
    #[error("tau_flag_degraded ({degraded}) must not exceed tau_flag ({flag})")]
    DegradedAboveFlag {
        /// The degraded threshold.
        degraded: f32,
        /// The normal flag threshold.
        flag: f32,
    },
    /// A threshold was NaN, which would make every comparison false and every
    /// region allowed.
    #[error("{field} is NaN")]
    NotANumber {
        /// Which field.
        field: &'static str,
    },
}

/// The tunable half of the policy. Everything here is frozen into
/// [`RulesetVersion`] and written to every decision row, so a reported number
/// cannot belong to a different policy than it claims.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ruleset {
    /// Version stamp, e.g. `v1@e81f`. The suffix is the content hash of the
    /// file this was loaded from.
    pub version: RulesetVersion,
    /// At or above this score a region is redacted outright (`R4`).
    pub tau_block: f32,
    /// At or above this score a region is flagged (`R6`), or redacted if it
    /// also hits the lexicon (`R5`).
    pub tau_flag: f32,
    /// The flag threshold used when extraction reported itself degraded
    /// (`R3`). Lower than `tau_flag`: when recall is known to be poor, less
    /// evidence is needed to act.
    pub tau_flag_degraded: f32,
    /// A region covering more than this fraction of the image cannot be
    /// redacted without destroying the image, so redaction degrades to a
    /// block.
    pub max_redact_frac: f32,
    /// Terms whose presence, combined with a flag-level score, is enough to
    /// redact rather than merely flag (`R5`). Matched case-insensitively
    /// against the normalised skeleton.
    pub tool_lexicon: Vec<String>,
}

impl Ruleset {
    /// Check the ruleset describes a coherent policy.
    ///
    /// # Errors
    ///
    /// Returns [`RulesetError`] for an out-of-range, NaN or inverted
    /// threshold.
    pub fn validate(&self) -> Result<(), RulesetError> {
        let bounded = [
            ("tau_block", self.tau_block),
            ("tau_flag", self.tau_flag),
            ("tau_flag_degraded", self.tau_flag_degraded),
            ("max_redact_frac", self.max_redact_frac),
        ];
        for (field, value) in bounded {
            if value.is_nan() {
                return Err(RulesetError::NotANumber { field });
            }
            if !(0.0..=1.0).contains(&value) {
                return Err(RulesetError::OutOfRange { field, value });
            }
        }
        if self.tau_flag > self.tau_block {
            return Err(RulesetError::FlagAboveBlock {
                flag: self.tau_flag,
                block: self.tau_block,
            });
        }
        if self.tau_flag_degraded > self.tau_flag {
            return Err(RulesetError::DegradedAboveFlag {
                degraded: self.tau_flag_degraded,
                flag: self.tau_flag,
            });
        }
        Ok(())
    }

    /// Lexicon terms, lowercased once so matching does not re-allocate per
    /// region.
    pub fn normalised_lexicon(&self) -> Vec<String> {
        self.tool_lexicon
            .iter()
            .map(|term| term.to_lowercase())
            .collect()
    }
}

impl Default for Ruleset {
    /// Placeholder thresholds for tests and first boot.
    ///
    /// These are *not* calibrated. Real values come from the calibration
    /// split in week 9 and arrive as `rulesets/v1.toml`; shipping a default
    /// that looks calibrated would be the easiest way to get an uncalibrated
    /// number into the report.
    fn default() -> Self {
        Self {
            version: RulesetVersion("uncalibrated@0000".to_string()),
            tau_block: 0.80,
            tau_flag: 0.50,
            tau_flag_degraded: 0.35,
            max_redact_frac: 0.40,
            tool_lexicon: vec![
                "ignore previous".to_string(),
                "system prompt".to_string(),
                "you are now".to_string(),
                "disregard".to_string(),
                "send to".to_string(),
                "email".to_string(),
                "curl".to_string(),
                "http://".to_string(),
                "https://".to_string(),
                "api key".to_string(),
                "tool".to_string(),
                "function call".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_ruleset_is_coherent() {
        Ruleset::default()
            .validate()
            .expect("the shipped default must validate");
    }

    #[test]
    fn the_default_ruleset_is_labelled_uncalibrated() {
        // Guards against someone pasting calibrated numbers into Default and
        // leaving the version stamp alone.
        assert!(
            Ruleset::default().version.0.starts_with("uncalibrated@"),
            "the default thresholds are placeholders and must say so"
        );
    }

    #[test]
    fn a_flag_threshold_above_the_block_threshold_is_rejected() {
        let rs = Ruleset {
            tau_flag: 0.9,
            tau_block: 0.5,
            ..Ruleset::default()
        };
        assert!(matches!(
            rs.validate(),
            Err(RulesetError::FlagAboveBlock { .. })
        ));
    }

    #[test]
    fn a_degraded_threshold_above_the_normal_one_is_rejected() {
        // R3 exists to lower the bar when recall is poor. Raising it instead
        // would quietly weaken the defence exactly when it is least reliable.
        let rs = Ruleset {
            tau_flag: 0.5,
            tau_flag_degraded: 0.7,
            ..Ruleset::default()
        };
        assert!(matches!(
            rs.validate(),
            Err(RulesetError::DegradedAboveFlag { .. })
        ));
    }

    #[test]
    fn nan_thresholds_are_rejected_rather_than_allowing_everything() {
        // Every `score >= NaN` comparison is false, so a NaN threshold would
        // allow every region while looking configured.
        let rs = Ruleset {
            tau_block: f32::NAN,
            ..Ruleset::default()
        };
        assert!(matches!(
            rs.validate(),
            Err(RulesetError::NotANumber { field: "tau_block" })
        ));
    }

    #[test]
    fn out_of_range_thresholds_are_rejected() {
        let rs = Ruleset {
            max_redact_frac: 1.5,
            ..Ruleset::default()
        };
        assert!(matches!(
            rs.validate(),
            Err(RulesetError::OutOfRange { .. })
        ));
    }
}
