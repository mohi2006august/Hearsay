//! Decision vocabulary shared between the policy engine and everything that
//! reads its output.
//!
//! These types live in `hearsay-core` rather than `hearsay-policy` so the proxy, the
//! store and the eval harness can name a decision without depending on the
//! engine that produced it.

use serde::{Deserialize, Serialize};

use crate::ids::{ModelVersion, RegionId};

/// A classifier score for one region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    /// Probability that the region is instruction-bearing content aimed at the
    /// model, in `0.0..=1.0`.
    pub injection: f32,
    /// Which classifier artefact produced it. Recorded per score, not per
    /// request, so a mid-run model swap is visible in the log.
    pub model: ModelVersion,
}

/// What the policy engine decided to do with one region.
///
/// The ordering is the severity ordering, and the derived `Ord` is load-bearing:
/// a request's outcome is the maximum over its regions, so `Allow < Flag <
/// Redact < Block` must stay in that order. `tests/` asserts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionAction {
    /// Forward unchanged.
    Allow,
    /// Forward, but annotate the decision record. Used for scores over the
    /// flag threshold but under the block threshold.
    Flag,
    /// Black out the region's pixels before forwarding.
    Redact,
    /// Redaction is not possible, so the whole input is refused.
    Block,
}

impl RegionAction {
    /// Whether this action changes the bytes sent upstream.
    pub fn mutates_input(&self) -> bool {
        matches!(self, Self::Redact)
    }
}

/// What happens to the request as a whole.
///
/// Preferring redaction over refusal is the decision recorded in `brain.md`:
/// it keeps benign utility high, which the PRD measures as a first-class
/// result rather than a footnote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum RequestOutcome {
    /// Forward as received. Some regions may still be flagged in the record.
    Allow,
    /// Forward with the listed regions blacked out.
    AllowRedacted {
        /// Regions whose pixels were destroyed, in ascending id order.
        redactions: Vec<RegionId>,
    },
    /// Refuse, with the rule that caused it.
    Block {
        /// Stable rule identifier, e.g. `R2_EXTRACTION_FAILED`.
        rule_id: String,
        /// Human-readable explanation for the refusal shown to the caller.
        reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ordering_is_allow_flag_redact_block() {
        // The request outcome is computed as a max over region actions, so
        // this ordering is not cosmetic.
        assert!(RegionAction::Allow < RegionAction::Flag);
        assert!(RegionAction::Flag < RegionAction::Redact);
        assert!(RegionAction::Redact < RegionAction::Block);

        let mut actions = [
            RegionAction::Redact,
            RegionAction::Allow,
            RegionAction::Block,
            RegionAction::Flag,
        ];
        actions.sort_unstable();
        assert_eq!(actions.last(), Some(&RegionAction::Block));
    }

    #[test]
    fn only_redaction_rewrites_the_forwarded_input() {
        assert!(RegionAction::Redact.mutates_input());
        assert!(!RegionAction::Allow.mutates_input());
        assert!(!RegionAction::Flag.mutates_input());
        // Block refuses the request rather than editing it.
        assert!(!RegionAction::Block.mutates_input());
    }

    #[test]
    fn outcomes_serialise_with_a_tag_the_log_can_filter_on() {
        let json = serde_json::to_string(&RequestOutcome::AllowRedacted {
            redactions: vec![RegionId(3)],
        })
        .expect("serialise");
        assert!(
            json.contains("\"outcome\":\"allow_redacted\""),
            "got {json}"
        );
    }
}
