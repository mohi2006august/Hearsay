//! HTTP handlers.
//!
//! Every endpoint under `/api` returns JSON; failures share one body shape
//! ([`ApiError`]) so a client has a single error branch to write.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use hearsay_core::{RegionAction, RequestOutcome};
use hearsay_policy::RuleId;
use serde::Deserialize;

use crate::record::{
    ApiError, DecisionRecord, EvaluateRequest, LatencySummary, RecordSource, RuleCount, RuleDoc,
    RulesetView, Stats,
};
use crate::state::{now_ms, AppState};

/// Largest number of regions one request may carry. The proxy's own limit is
/// 8 images; a page of dense text can produce many regions per image, so this
/// is generous but bounded.
const MAX_REGIONS: usize = 64;

/// Longest single region string accepted, in bytes.
const MAX_REGION_BYTES: usize = 8 * 1024;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

fn bad_request(error: &str, detail: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiError {
            error: error.to_string(),
            detail: detail.into(),
        }),
    )
}

pub(crate) fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .route("/api/ruleset", get(ruleset))
        .route("/api/decisions", get(list_decisions))
        .route("/api/decisions/{decision_id}", get(get_decision))
        .route("/api/stats", get(stats))
        .route("/api/evaluate", post(evaluate))
        .with_state(state)
}

// ------------------------------------------------------------------ health

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Readiness.
///
/// Returns 200 because the policy engine is the only dependency and it is
/// built at startup. When `hearsay-classify` lands this must gate on the ONNX
/// session having run a warmup pass — see `design.md` §8, a cold first
/// inference would otherwise land in the p95 the NFR is measured against.
async fn readyz(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let body = serde_json::json!({
        "ready": true,
        "engine": "policy",
        "ruleset": state.engine.ruleset().version,
        "decisions_held": state.len(),
        "durable": false,
        "note": "decision log is an in-memory ring; hearsay-store is not built yet",
    });
    (StatusCode::OK, Json(body))
}

// ----------------------------------------------------------------- ruleset

/// The seven rules, in evaluation order. Served rather than hardcoded in the
/// UI so the dashboard cannot drift from the engine.
fn rule_docs() -> Vec<RuleDoc> {
    vec![
        RuleDoc {
            ordinal: 1,
            rule_id: RuleId::R1InstructionChannel,
            condition: "channel is Instruction",
            action: "allow, uninspected",
        },
        RuleDoc {
            ordinal: 2,
            rule_id: RuleId::R2ExtractionFailed,
            condition: "extraction returned nothing usable",
            action: "block image, forward remaining parts",
        },
        RuleDoc {
            ordinal: 3,
            rule_id: RuleId::R3ExtractionDegraded,
            condition: "extraction degraded (low contrast, tiny glyphs, rotation, timeout)",
            action: "escalate: tau_flag drops to tau_flag_degraded",
        },
        RuleDoc {
            ordinal: 4,
            rule_id: RuleId::R4HighConfidence,
            condition: "score >= tau_block",
            action: "redact",
        },
        RuleDoc {
            ordinal: 5,
            rule_id: RuleId::R5ImperativeToolRef,
            condition: "score >= tau_flag and matches the tool/system lexicon",
            action: "redact",
        },
        RuleDoc {
            ordinal: 6,
            rule_id: RuleId::R6LowConfidence,
            condition: "score >= tau_flag",
            action: "flag, forward annotated",
        },
        RuleDoc {
            ordinal: 7,
            rule_id: RuleId::R7Default,
            condition: "nothing fired",
            action: "allow",
        },
    ]
}

async fn ruleset(State(state): State<Arc<AppState>>) -> Json<RulesetView> {
    let rs = state.engine.ruleset();
    Json(RulesetView {
        version: rs.version.clone(),
        tau_block: rs.tau_block,
        tau_flag: rs.tau_flag,
        tau_flag_degraded: rs.tau_flag_degraded,
        max_redact_frac: rs.max_redact_frac,
        tool_lexicon: rs.tool_lexicon.clone(),
        rules: rule_docs(),
    })
}

// --------------------------------------------------------------- decisions

#[derive(Debug, Deserialize)]
pub(crate) struct ListQuery {
    /// Only decisions newer than this epoch-millis timestamp. The dashboard
    /// polls with the newest value it holds, so it never refetches.
    #[serde(default)]
    since_ms: Option<u64>,
    #[serde(default)]
    limit: Option<usize>,
}

async fn list_decisions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListQuery>,
) -> Json<Vec<DecisionRecord>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    Json(state.recent(q.since_ms, limit))
}

async fn get_decision(
    State(state): State<Arc<AppState>>,
    Path(decision_id): Path<String>,
) -> ApiResult<DecisionRecord> {
    state.get(&decision_id).map(Json).ok_or((
        StatusCode::NOT_FOUND,
        Json(ApiError {
            error: "not_found".to_string(),
            detail: format!("no decision {decision_id}"),
        }),
    ))
}

// ------------------------------------------------------------------- stats

fn percentile(sorted: &[u32], p: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    // Nearest-rank. With a few hundred samples the interpolation argument
    // does not earn its complexity.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

async fn stats(State(state): State<Arc<AppState>>) -> Json<Stats> {
    let all = state.all();

    let mut allow = 0;
    let mut allow_redacted = 0;
    let mut block = 0;
    let mut regions_total = 0;
    let mut regions_redacted = 0;
    let mut regions_flagged = 0;
    let mut totals: Vec<u32> = Vec::with_capacity(all.len());

    // Counts keyed by the rule's wire string, then mapped back, so the order
    // of the output follows the rule table rather than a hash.
    let mut counts: Vec<(RuleId, usize)> = rule_docs().iter().map(|r| (r.rule_id, 0)).collect();

    for d in &all {
        match d.outcome {
            RequestOutcome::Allow => allow += 1,
            RequestOutcome::AllowRedacted { .. } => allow_redacted += 1,
            RequestOutcome::Block { .. } => block += 1,
        }
        totals.push(d.timings_ms.total);
        for r in &d.regions {
            regions_total += 1;
            match r.action {
                RegionAction::Redact => regions_redacted += 1,
                RegionAction::Flag => regions_flagged += 1,
                _ => {}
            }
            if let Some(entry) = counts.iter_mut().find(|(id, _)| *id == r.rule_id) {
                entry.1 += 1;
            }
        }
    }

    totals.sort_unstable();

    Json(Stats {
        total: all.len(),
        allow,
        allow_redacted,
        block,
        regions_total,
        regions_redacted,
        regions_flagged,
        by_rule: counts
            .into_iter()
            .map(|(rule_id, count)| RuleCount { rule_id, count })
            .collect(),
        latency: LatencySummary {
            p50: percentile(&totals, 50.0),
            p95: percentile(&totals, 95.0),
            max: totals.last().copied().unwrap_or(0),
            budget: 400,
        },
        uptime_ms: now_ms().saturating_sub(state.started_ms),
    })
}

// ---------------------------------------------------------------- evaluate

async fn evaluate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EvaluateRequest>,
) -> ApiResult<DecisionRecord> {
    if req.regions.len() > MAX_REGIONS {
        return Err(bad_request(
            "too_many_regions",
            format!("{} regions, limit is {MAX_REGIONS}", req.regions.len()),
        ));
    }
    if let Some((i, r)) = req
        .regions
        .iter()
        .enumerate()
        .find(|(_, r)| r.raw.len() > MAX_REGION_BYTES)
    {
        return Err(bad_request(
            "region_too_large",
            format!(
                "region {i} is {} bytes, limit is {MAX_REGION_BYTES}",
                r.raw.len()
            ),
        ));
    }
    if req.image_area == 0 {
        return Err(bad_request(
            "invalid_image_area",
            "image_area must be greater than zero, or omitted for the 1080p default",
        ));
    }

    let record = crate::pipeline::evaluate(&state.engine, &req, RecordSource::Live, now_ms());
    state.record(record.clone());
    Ok(Json(record))
}

// ----------------------------------------------------------------- metrics

async fn metrics(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let all = state.all();
    let mut allow = 0;
    let mut redacted = 0;
    let mut blocked = 0;
    for d in &all {
        match d.outcome {
            RequestOutcome::Allow => allow += 1,
            RequestOutcome::AllowRedacted { .. } => redacted += 1,
            RequestOutcome::Block { .. } => blocked += 1,
        }
    }

    let body = format!(
        "# HELP hearsay_decisions_total Decisions held in the log.\n\
         # TYPE hearsay_decisions_total gauge\n\
         hearsay_decisions_total {}\n\
         # HELP hearsay_decisions_by_outcome Decisions by request outcome.\n\
         # TYPE hearsay_decisions_by_outcome gauge\n\
         hearsay_decisions_by_outcome{{outcome=\"allow\"}} {allow}\n\
         hearsay_decisions_by_outcome{{outcome=\"allow_redacted\"}} {redacted}\n\
         hearsay_decisions_by_outcome{{outcome=\"block\"}} {blocked}\n\
         # HELP hearsay_uptime_ms Milliseconds since start.\n\
         # TYPE hearsay_uptime_ms counter\n\
         hearsay_uptime_ms {}\n",
        all.len(),
        now_ms().saturating_sub(state.started_ms),
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        body,
    )
}
