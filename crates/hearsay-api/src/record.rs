//! Wire types for the HTTP surface.
//!
//! These mirror the decision record in `design.md` §7. Where a core type
//! already has the right serde shape it is reused rather than redeclared —
//! `ExtractionOutcome`, `RequestOutcome` and `RegionAction` all serialise the
//! way the log wants, so the API and the domain cannot drift apart.

use hearsay_core::{
    BBox, ChannelLabel, ContentHash, DecisionId, ExtractionOutcome, ModelVersion, RegionAction,
    RegionId, RequestId, RequestOutcome, RulesetVersion,
};
use hearsay_policy::RuleId;
use serde::{Deserialize, Serialize};

/// Pipeline version stamped into every record. Bumped by hand when the
/// request path changes shape.
pub(crate) const PIPELINE_VERSION: &str = "0.1.0";

/// One region as it appears in a stored decision.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct RegionRecord {
    pub(crate) region_id: RegionId,
    pub(crate) bbox: Option<BBox>,
    pub(crate) channel: ChannelLabel,
    /// Exactly what OCR returned, kept verbatim.
    pub(crate) raw: String,
    /// What the classifier actually scored.
    pub(crate) normalized: String,
    pub(crate) ocr_confidence: Option<f32>,
    pub(crate) score: Option<f32>,
    pub(crate) action: RegionAction,
    pub(crate) rule_id: RuleId,
    pub(crate) reason: String,
}

/// How extraction went for the image this decision covers.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ExtractionInfo {
    pub(crate) engine: String,
    pub(crate) outcome: ExtractionOutcome,
    pub(crate) ms: u32,
}

/// The three version fields. A result is reproducible only if all three are
/// pinned, so they travel together and the eval harness refuses to aggregate
/// across a mismatch.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Versions {
    pub(crate) classifier: ModelVersion,
    pub(crate) ruleset: RulesetVersion,
    pub(crate) pipeline: &'static str,
}

/// Per-stage latency, matching the budget table in `design.md` §9.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub(crate) struct Timings {
    pub(crate) decode: u32,
    pub(crate) ocr: u32,
    pub(crate) classify: u32,
    pub(crate) policy: u32,
    pub(crate) redact: u32,
    pub(crate) total: u32,
}

/// Where a record came from.
///
/// This exists so the dashboard never presents fixture numbers as measured
/// ones. `brain.md` is explicit that anything provisional must be labelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordSource {
    /// A real call to `/api/evaluate`. Policy timing is measured; the OCR and
    /// classifier timings are zero because those stages do not exist yet.
    Live,
    /// Seeded at boot so the dashboard opens in a working state. Timings are
    /// the budget figures from `design.md` §9, not observations.
    Seed,
}

/// A decision as stored and served.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct DecisionRecord {
    pub(crate) source: RecordSource,
    pub(crate) decision_id: DecisionId,
    pub(crate) request_id: RequestId,
    /// Unix epoch milliseconds. Formatting is the client's job — the server
    /// has no business guessing a timezone.
    pub(crate) ts_ms: u64,
    pub(crate) input_hash: ContentHash,
    pub(crate) outcome: RequestOutcome,
    /// The rule that decided the request as a whole.
    pub(crate) rule_id: RuleId,
    pub(crate) reason: String,
    pub(crate) regions: Vec<RegionRecord>,
    pub(crate) extraction: ExtractionInfo,
    pub(crate) versions: Versions,
    pub(crate) timings_ms: Timings,
}

// ---------------------------------------------------------------- requests

/// One region submitted for evaluation.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EvaluateRegion {
    /// The text as extracted. Required — everything else has a default.
    pub(crate) raw: String,
    #[serde(default)]
    pub(crate) score: Option<f32>,
    #[serde(default)]
    pub(crate) bbox: Option<BBox>,
    #[serde(default)]
    pub(crate) ocr_confidence: Option<f32>,
    /// Defaults to the data channel. Sending `instruction` is how a caller
    /// demonstrates rule R1.
    #[serde(default)]
    pub(crate) channel: Option<ChannelLabel>,
}

fn default_extraction() -> ExtractionOutcome {
    ExtractionOutcome::Complete
}

fn default_image_area() -> u64 {
    // 1080p, the size the latency budget is quoted against.
    1_920 * 1_080
}

/// Run the policy engine over a set of regions and record the decision.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EvaluateRequest {
    #[serde(default)]
    pub(crate) regions: Vec<EvaluateRegion>,
    #[serde(default = "default_extraction")]
    pub(crate) extraction: ExtractionOutcome,
    #[serde(default = "default_image_area")]
    pub(crate) image_area: u64,
}

// ---------------------------------------------------------------- responses

/// One row of the rule table, served so the UI never hardcodes policy.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuleDoc {
    pub(crate) ordinal: u8,
    pub(crate) rule_id: RuleId,
    pub(crate) condition: &'static str,
    pub(crate) action: &'static str,
}

/// The active ruleset, so the dashboard can show the thresholds a decision was
/// actually made under.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct RulesetView {
    pub(crate) version: RulesetVersion,
    pub(crate) tau_block: f32,
    pub(crate) tau_flag: f32,
    pub(crate) tau_flag_degraded: f32,
    pub(crate) max_redact_frac: f32,
    pub(crate) tool_lexicon: Vec<String>,
    pub(crate) rules: Vec<RuleDoc>,
}

/// How often each rule fired.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuleCount {
    pub(crate) rule_id: RuleId,
    pub(crate) count: usize,
}

/// Observed latency, from the same `timings_ms` the records carry.
#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct LatencySummary {
    pub(crate) p50: u32,
    pub(crate) p95: u32,
    pub(crate) max: u32,
    /// The NFR from `prd.md` §6, so the UI can draw the limit without
    /// hardcoding it.
    pub(crate) budget: u32,
}

/// Dashboard summary.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Stats {
    pub(crate) total: usize,
    pub(crate) allow: usize,
    pub(crate) allow_redacted: usize,
    pub(crate) block: usize,
    pub(crate) regions_total: usize,
    pub(crate) regions_redacted: usize,
    pub(crate) regions_flagged: usize,
    pub(crate) by_rule: Vec<RuleCount>,
    pub(crate) latency: LatencySummary,
    pub(crate) uptime_ms: u64,
}

/// Error body. One shape for every failure, so the client has one branch.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ApiError {
    pub(crate) error: String,
    pub(crate) detail: String,
}
