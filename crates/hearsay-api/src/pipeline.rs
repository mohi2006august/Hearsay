//! The request path, such as it exists.
//!
//! Today this is: normalise, then policy. `design.md` §6 has six more stages
//! and they are not built. This module is where OCR, the classifier and
//! redaction slot in, and keeping it separate from the HTTP handlers is what
//! makes that a local change.
//!
//! Shared by the live endpoint and the boot fixtures, so a seeded decision is
//! genuine engine output rather than a hand-written verdict that could
//! silently disagree with the rules.

use std::time::Instant;

use hearsay_core::{ChannelLabel, ContentHash, DecisionId, ModelVersion, RegionId, RequestId};
use hearsay_policy::{PolicyEngine, Redactable, RegionInput, RuleId};

use crate::normalize::normalize;
use crate::record::{
    DecisionRecord, EvaluateRequest, ExtractionInfo, RecordSource, RegionRecord, Timings, Versions,
    PIPELINE_VERSION,
};

/// Run a request through the pipeline and build its decision record.
pub(crate) fn evaluate(
    engine: &PolicyEngine,
    req: &EvaluateRequest,
    source: RecordSource,
    ts_ms: u64,
) -> DecisionRecord {
    // Normalise first: the classifier and the R5 lexicon both see the
    // skeleton, never the raw string. The raw is kept for the record.
    let normalized: Vec<String> = req.regions.iter().map(|r| normalize(&r.raw)).collect();
    let extraction = req.extraction.clone();

    let inputs: Vec<RegionInput<'_>> = req
        .regions
        .iter()
        .enumerate()
        .map(|(i, r)| RegionInput {
            region: RegionId(u32::try_from(i).unwrap_or(u32::MAX)),
            channel: r.channel.unwrap_or(ChannelLabel::Data),
            extraction: &extraction,
            score: r.score,
            text: Some(normalized[i].as_str()),
            redactable: r.bbox.map_or(Redactable::NoBBox, |b| {
                b.fraction_of(req.image_area)
                    .map_or(Redactable::NoBBox, |fraction| Redactable::Yes { fraction })
            }),
        })
        .collect();

    let decision_id = DecisionId::new();
    let started = Instant::now();
    let verdict = engine.decide(decision_id, &inputs);
    #[allow(clippy::cast_possible_truncation)]
    let policy_ms = (started.elapsed().as_micros() / 1000) as u32;

    // The request-level rule is the one belonging to the most severe region,
    // which is exactly how the engine derived the outcome.
    let deciding = verdict.regions.iter().max_by_key(|v| v.action);
    let rule_id = deciding.map_or(RuleId::R7Default, |v| v.rule_id);
    let reason = deciding.map_or_else(|| "no regions submitted".to_string(), |v| v.reason.clone());

    let mut hasher = blake3::Hasher::new();
    for r in &req.regions {
        hasher.update(r.raw.as_bytes());
        hasher.update(b"\0");
    }
    let input_hash = ContentHash::from_bytes(*hasher.finalize().as_bytes());

    let regions = verdict
        .regions
        .iter()
        .enumerate()
        .map(|(i, v)| RegionRecord {
            region_id: v.region,
            bbox: req.regions[i].bbox,
            channel: req.regions[i].channel.unwrap_or(ChannelLabel::Data),
            raw: req.regions[i].raw.clone(),
            normalized: normalized[i].clone(),
            ocr_confidence: req.regions[i].ocr_confidence,
            score: req.regions[i].score,
            action: v.action,
            rule_id: v.rule_id,
            reason: v.reason.clone(),
        })
        .collect();

    DecisionRecord {
        source,
        decision_id,
        request_id: RequestId::new(),
        ts_ms,
        input_hash,
        outcome: verdict.outcome.clone(),
        rule_id,
        reason,
        regions,
        extraction: ExtractionInfo {
            engine: "none (caller-supplied)".to_string(),
            outcome: req.extraction.clone(),
            ms: 0,
        },
        versions: Versions {
            classifier: ModelVersion("none (caller-supplied scores)".to_string()),
            ruleset: verdict.ruleset.clone(),
            pipeline: PIPELINE_VERSION,
        },
        timings_ms: Timings {
            // Only policy is measured. The other stages are zero because they
            // do not exist — not because they are fast. The dashboard labels
            // this rather than charting it as a real profile.
            policy: policy_ms,
            total: policy_ms,
            ..Timings::default()
        },
    }
}
