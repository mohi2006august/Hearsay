//! The seven rules, in order, first match wins.
//!
//! `decide_region` is synchronous, total and pure: it cannot await, cannot
//! fail and cannot call a model. That is what makes the decision explainable
//! in a live demo, and it is why this type is concrete rather than a trait —
//! the policy engine is deliberately not pluggable.

use hearsay_core::{
    ChannelLabel, ClearanceAuthority, DecisionId, Declassification, Extraction, ExtractionOutcome,
    RegionAction, RegionId, RequestOutcome, RulesetVersion,
};
use serde::{Deserialize, Serialize};

use crate::ruleset::Ruleset;

/// Stable rule identifiers. These strings go verbatim into the decision
/// record's `rule_id` field (FR-3), so they are part of the output contract
/// and must not be renamed without a ruleset version bump.
///
/// Serde round-trips through the wire string, not the variant name, so a log
/// row written today parses back into the same variant after a rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum RuleId {
    /// Instruction-channel content, forwarded uninspected.
    R1InstructionChannel,
    /// Extraction failed; the image is dropped.
    R2ExtractionFailed,
    /// Extraction degraded; thresholds escalated.
    R3ExtractionDegraded,
    /// Score at or above the block threshold.
    R4HighConfidence,
    /// Flag-level score plus a tool or system term.
    R5ImperativeToolRef,
    /// Flag-level score alone.
    R6LowConfidence,
    /// Nothing fired.
    R7Default,
}

impl RuleId {
    /// The stable wire form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::R1InstructionChannel => "R1_INSTRUCTION_CHANNEL",
            Self::R2ExtractionFailed => "R2_EXTRACTION_FAILED",
            Self::R3ExtractionDegraded => "R3_EXTRACTION_DEGRADED",
            Self::R4HighConfidence => "R4_HIGH_CONFIDENCE",
            Self::R5ImperativeToolRef => "R5_IMPERATIVE_TOOL_REF",
            Self::R6LowConfidence => "R6_LOW_CONFIDENCE",
            Self::R7Default => "R7_DEFAULT",
        }
    }
}

impl From<RuleId> for String {
    fn from(id: RuleId) -> Self {
        id.as_str().to_string()
    }
}

/// A `rule_id` string that does not name a known rule.
#[derive(Debug, thiserror::Error)]
#[error("unknown rule_id {0:?}")]
pub struct UnknownRuleId(String);

impl TryFrom<String> for RuleId {
    type Error = UnknownRuleId;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "R1_INSTRUCTION_CHANNEL" => Ok(Self::R1InstructionChannel),
            "R2_EXTRACTION_FAILED" => Ok(Self::R2ExtractionFailed),
            "R3_EXTRACTION_DEGRADED" => Ok(Self::R3ExtractionDegraded),
            "R4_HIGH_CONFIDENCE" => Ok(Self::R4HighConfidence),
            "R5_IMPERATIVE_TOOL_REF" => Ok(Self::R5ImperativeToolRef),
            "R6_LOW_CONFIDENCE" => Ok(Self::R6LowConfidence),
            "R7_DEFAULT" => Ok(Self::R7Default),
            _ => Err(UnknownRuleId(value)),
        }
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether a region's pixels can actually be destroyed.
///
/// `Redact` degrades to `Block` when they cannot: a region we can name but
/// not locate must not be forwarded just because redaction was the preferred
/// action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Redactable {
    /// The region has a bbox covering this fraction of the image.
    Yes {
        /// Share of the image area, `0.0..=1.0`.
        fraction: f32,
    },
    /// No usable bounding box, so there are no pixels to black out.
    NoBBox,
}

/// Everything the engine needs to judge one region.
#[derive(Debug, Clone, Copy)]
pub struct RegionInput<'a> {
    /// Which region.
    pub region: RegionId,
    /// Which side of the provenance boundary it arrived on.
    pub channel: ChannelLabel,
    /// How well extraction went for the image this region came from.
    pub extraction: &'a ExtractionOutcome,
    /// The classifier's score, or `None` when there was no text to score.
    pub score: Option<f32>,
    /// The normalised skeleton, for the lexicon check in `R5`.
    pub text: Option<&'a str>,
    /// Whether the region can be redacted.
    pub redactable: Redactable,
}

/// The engine's judgement on one region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionVerdict {
    /// Which region.
    pub region: RegionId,
    /// What to do with it.
    pub action: RegionAction,
    /// Which rule decided, for FR-3's machine-readable reason.
    pub rule_id: RuleId,
    /// Why, in a form a person can read in a demo.
    pub reason: String,
}

/// The engine's judgement on the request as a whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestVerdict {
    /// Identifies this decision in the log.
    pub decision_id: DecisionId,
    /// What happens to the request.
    pub outcome: RequestOutcome,
    /// Per-region detail, in the order the regions were supplied.
    pub regions: Vec<RegionVerdict>,
    /// Which ruleset produced this. Pinned into the log row.
    pub ruleset: RulesetVersion,
}

/// The deterministic policy engine.
pub struct PolicyEngine {
    ruleset: Ruleset,
    lexicon: Vec<String>,
}

// The single `unsafe impl` in the workspace. It asserts that this type only
// mints a clearance as the result of a logged policy decision — see
// `PolicyEngine::clearance`, which is the only place a witness is created and
// which refuses for any action other than `Allow`.
//
// Every other crate sets `#![forbid(unsafe_code)]`, so a second implementation
// anywhere else is a compile error rather than a review question.
#[allow(unsafe_code)]
unsafe impl ClearanceAuthority for PolicyEngine {}

impl PolicyEngine {
    /// Build an engine from a validated ruleset.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ruleset::RulesetError`] if the ruleset is incoherent.
    pub fn new(ruleset: Ruleset) -> Result<Self, crate::ruleset::RulesetError> {
        ruleset.validate()?;
        let lexicon = ruleset.normalised_lexicon();
        Ok(Self { ruleset, lexicon })
    }

    /// The ruleset in force.
    pub fn ruleset(&self) -> &Ruleset {
        &self.ruleset
    }

    /// Judge one region. Pure and total.
    pub fn decide_region(&self, input: &RegionInput<'_>) -> RegionVerdict {
        let (action, rule_id, reason) = self.classify_region(input);

        // Redaction degrades to a block when the pixels cannot be destroyed.
        // Doing this here rather than during aggregation keeps the per-region
        // record honest about what would actually have happened.
        let (action, reason) = match (action, input.redactable) {
            (RegionAction::Redact, Redactable::NoBBox) => (
                RegionAction::Block,
                format!("{reason}; redaction impossible: region has no bounding box"),
            ),
            (RegionAction::Redact, Redactable::Yes { fraction })
                if fraction > self.ruleset.max_redact_frac =>
            {
                (
                    RegionAction::Block,
                    format!(
                        "{reason}; redaction impossible: region covers {:.0}% of the image, over the {:.0}% limit",
                        fraction * 100.0,
                        self.ruleset.max_redact_frac * 100.0
                    ),
                )
            }
            (other, _) => (other, reason),
        };

        RegionVerdict {
            region: input.region,
            action,
            rule_id,
            reason,
        }
    }

    /// The rule cascade, before the redactability degrade.
    fn classify_region(&self, input: &RegionInput<'_>) -> (RegionAction, RuleId, String) {
        // R1 — instruction-channel content is never inspected. This is the
        // provenance boundary doing its job: the user's own prompt is not
        // subject to the injection classifier.
        if input.channel == ChannelLabel::Instruction {
            return (
                RegionAction::Allow,
                RuleId::R1InstructionChannel,
                "instruction channel; not inspected".to_string(),
            );
        }

        // R2 — nothing was extracted, so nothing can be reasoned about. Fail
        // closed on this image; the proxy forwards the remaining parts.
        if let ExtractionOutcome::Failed { reason } = input.extraction {
            return (
                RegionAction::Block,
                RuleId::R2ExtractionFailed,
                format!("extraction failed ({reason}); image dropped"),
            );
        }

        // R3 — degraded extraction escalates by lowering the flag threshold.
        // This is not a terminal rule: it changes the bar that R4-R6 apply.
        //
        // Crediting: R3 is recorded as the deciding rule when escalation is
        // what moved a region from Allow to Flag. A region that escalated
        // over the bar and then hit the lexicon is credited to R5, because
        // the lexicon is what turned a flag into a redaction. So the R3 count
        // is a lower bound on how often escalation mattered — state it that
        // way in the ablation rather than as an exact count.
        let degraded = matches!(input.extraction, ExtractionOutcome::Degraded { .. });
        let tau_flag = if degraded {
            self.ruleset.tau_flag_degraded
        } else {
            self.ruleset.tau_flag
        };

        let Some(score) = input.score else {
            // No text in this region means nothing to score. The region still
            // appears in the record so the log shows it was considered.
            return (
                RegionAction::Allow,
                RuleId::R7Default,
                "no text extracted from this region".to_string(),
            );
        };

        // R4 — confident enough to act without further evidence.
        if score >= self.ruleset.tau_block {
            return (
                RegionAction::Redact,
                RuleId::R4HighConfidence,
                format!(
                    "score {score:.2} at or above tau_block {:.2}",
                    self.ruleset.tau_block
                ),
            );
        }

        if score >= tau_flag {
            // R5 — a flag-level score plus a term that names a tool, a system
            // surface or an exfiltration route. The combination is what
            // distinguishes "instructions aimed at the model" from prose that
            // merely reads as imperative.
            if let Some(term) = self.lexicon_hit(input.text) {
                return (
                    RegionAction::Redact,
                    RuleId::R5ImperativeToolRef,
                    format!("score {score:.2} at or above tau_flag {tau_flag:.2} and matched lexicon term {term:?}"),
                );
            }

            // R6 — flag-level score alone. Forward, but annotate.
            let rule_id = if degraded && score < self.ruleset.tau_flag {
                // Only fired because extraction was degraded; credit R3 so
                // the ablation can count how often escalation mattered.
                RuleId::R3ExtractionDegraded
            } else {
                RuleId::R6LowConfidence
            };
            return (
                RegionAction::Flag,
                rule_id,
                format!("score {score:.2} at or above tau_flag {tau_flag:.2}"),
            );
        }

        // R7 — nothing fired.
        (
            RegionAction::Allow,
            RuleId::R7Default,
            format!("score {score:.2} below tau_flag {tau_flag:.2}"),
        )
    }

    /// First lexicon term present in the text, if any.
    fn lexicon_hit(&self, text: Option<&str>) -> Option<&str> {
        let text = text?.to_lowercase();
        self.lexicon
            .iter()
            .find(|term| text.contains(term.as_str()))
            .map(String::as_str)
    }

    /// Judge a whole request by aggregating its regions.
    ///
    /// The outcome is the most severe region action, which is why
    /// [`RegionAction`]'s ordering is load-bearing.
    pub fn decide(&self, decision_id: DecisionId, regions: &[RegionInput<'_>]) -> RequestVerdict {
        let verdicts: Vec<RegionVerdict> = regions
            .iter()
            .map(|input| self.decide_region(input))
            .collect();

        // The most severe verdict decides the request. `max_by_key` returns
        // the last maximum, so for several equally severe regions the reason
        // reported is the last one's — deterministic either way, which is
        // what the decision log needs.
        let worst = verdicts.iter().max_by_key(|v| v.action);

        let outcome = match worst {
            None => RequestOutcome::Allow,
            Some(v) => match v.action {
                RegionAction::Allow | RegionAction::Flag => RequestOutcome::Allow,
                RegionAction::Redact => {
                    let mut redactions: Vec<RegionId> = verdicts
                        .iter()
                        .filter(|v| v.action == RegionAction::Redact)
                        .map(|v| v.region)
                        .collect();
                    redactions.sort_unstable();
                    RequestOutcome::AllowRedacted { redactions }
                }
                // Matching on the worst verdict directly, rather than
                // searching for a blocking region, keeps this total — there
                // is no unreachable branch to `expect` on.
                RegionAction::Block => RequestOutcome::Block {
                    rule_id: v.rule_id.as_str().to_string(),
                    reason: v.reason.clone(),
                },
            },
        };

        RequestVerdict {
            decision_id,
            outcome,
            regions: verdicts,
            ruleset: self.ruleset.version.clone(),
        }
    }

    /// Issue a clearance for a region the engine allowed.
    ///
    /// Returns `None` for any other action, which is the mechanical reason a
    /// flagged, redacted or blocked region can never be promoted to the
    /// instruction channel.
    ///
    /// The caller is expected to have logged `verdict` before calling this;
    /// the proxy's forward stage does so.
    pub fn clearance(
        &self,
        decision_id: DecisionId,
        verdict: &RegionVerdict,
    ) -> Option<Declassification> {
        if verdict.action != RegionAction::Allow {
            return None;
        }
        Some(Declassification::mint(
            self,
            decision_id,
            verdict.region,
            self.ruleset.version.clone(),
        ))
    }
}

/// Convenience for the common case of judging every region of one extraction.
///
/// Scores must be supplied in the same order as `extraction.regions`.
pub fn region_inputs<'a>(
    extraction: &'a Extraction,
    scores: &'a [Option<f32>],
    normalised: &'a [Option<&'a str>],
    image_area: u64,
) -> Vec<RegionInput<'a>> {
    extraction
        .regions
        .iter()
        .enumerate()
        .map(|(i, region)| RegionInput {
            region: region.id,
            channel: region.source.admitted_channel(),
            extraction: &extraction.outcome,
            score: scores.get(i).copied().flatten(),
            text: normalised.get(i).copied().flatten(),
            redactable: match region.bbox.fraction_of(image_area) {
                Some(fraction) => Redactable::Yes { fraction },
                None => Redactable::NoBBox,
            },
        })
        .collect()
}
