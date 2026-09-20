/**
 * Wire types, mirroring `crates/hearsay-api/src/record.rs`.
 *
 * These are hand-maintained. The serde attributes on the Rust side decide the
 * shapes: newtype structs (`RegionId`, `DecisionId`, `RulesetVersion`)
 * serialise transparently to their inner value, and the tagged enums carry an
 * `outcome` discriminant. If a Rust type changes, this file changes with it —
 * `npm run check` will not catch the drift, only a failing request will.
 */

export type ChannelLabel = 'instruction' | 'data';

export type RegionAction = 'allow' | 'flag' | 'redact' | 'block';

/** Severity order, matching `RegionAction`'s derived `Ord` in the engine. */
export const ACTION_SEVERITY: Record<RegionAction, number> = {
  allow: 0,
  flag: 1,
  redact: 2,
  block: 3,
};

export type RuleId =
  | 'R1_INSTRUCTION_CHANNEL'
  | 'R2_EXTRACTION_FAILED'
  | 'R3_EXTRACTION_DEGRADED'
  | 'R4_HIGH_CONFIDENCE'
  | 'R5_IMPERATIVE_TOOL_REF'
  | 'R6_LOW_CONFIDENCE'
  | 'R7_DEFAULT';

/**
 * Whether a record was produced by a real call or seeded at boot.
 *
 * The UI must label `seed` rows. Their timings are budget figures from the
 * design, not measurements, and presenting them as observations would be
 * exactly the quiet dishonesty `brain.md` warns about.
 */
export type RecordSource = 'live' | 'seed';

export interface BBox {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type DegradeReason =
  | 'low_contrast'
  | 'tiny_glyphs'
  | 'rotation'
  | 'timeout'
  | 'partial_page';

export type ExtractionOutcome =
  | { outcome: 'complete' }
  | { outcome: 'degraded'; reason: DegradeReason }
  | { outcome: 'failed'; reason: string };

export type RequestOutcome =
  | { outcome: 'allow' }
  | { outcome: 'allow_redacted'; redactions: number[] }
  | { outcome: 'block'; rule_id: string; reason: string };

export interface RegionRecord {
  region_id: number;
  bbox: BBox | null;
  channel: ChannelLabel;
  /** Exactly what OCR returned, kept verbatim. */
  raw: string;
  /** What the engine actually matched against. */
  normalized: string;
  ocr_confidence: number | null;
  score: number | null;
  action: RegionAction;
  rule_id: RuleId;
  reason: string;
}

export interface ExtractionInfo {
  engine: string;
  outcome: ExtractionOutcome;
  ms: number;
}

export interface Versions {
  classifier: string;
  ruleset: string;
  pipeline: string;
}

export interface Timings {
  decode: number;
  ocr: number;
  classify: number;
  policy: number;
  redact: number;
  total: number;
}

export interface DecisionRecord {
  source: RecordSource;
  decision_id: string;
  request_id: string;
  ts_ms: number;
  input_hash: string;
  outcome: RequestOutcome;
  rule_id: RuleId;
  reason: string;
  regions: RegionRecord[];
  extraction: ExtractionInfo;
  versions: Versions;
  timings_ms: Timings;
}

export interface RuleDoc {
  ordinal: number;
  rule_id: RuleId;
  condition: string;
  action: string;
}

export interface RulesetView {
  version: string;
  tau_block: number;
  tau_flag: number;
  tau_flag_degraded: number;
  max_redact_frac: number;
  tool_lexicon: string[];
  rules: RuleDoc[];
}

export interface RuleCount {
  rule_id: RuleId;
  count: number;
}

export interface LatencySummary {
  p50: number;
  p95: number;
  max: number;
  /** The NFR from prd.md §6. Served so the UI never hardcodes it. */
  budget: number;
}

export interface Stats {
  total: number;
  allow: number;
  allow_redacted: number;
  block: number;
  regions_total: number;
  regions_redacted: number;
  regions_flagged: number;
  by_rule: RuleCount[];
  latency: LatencySummary;
  uptime_ms: number;
}

export interface EvaluateRegionInput {
  raw: string;
  score?: number | null;
  bbox?: BBox | null;
  ocr_confidence?: number | null;
  channel?: ChannelLabel | null;
}

export interface EvaluateRequestInput {
  regions: EvaluateRegionInput[];
  extraction: ExtractionOutcome;
  image_area?: number;
}

/** The single error shape every endpoint uses on failure. */
export interface ApiErrorBody {
  error: string;
  detail: string;
}

/** Short label for a request outcome. */
export function outcomeLabel(o: RequestOutcome): string {
  switch (o.outcome) {
    case 'allow':
      return 'Allowed';
    case 'allow_redacted':
      return `Redacted (${o.redactions.length})`;
    case 'block':
      return 'Blocked';
  }
}

/** Maps a request outcome onto the severity vocabulary used for colour. */
export function outcomeAction(o: RequestOutcome): RegionAction {
  switch (o.outcome) {
    case 'allow':
      return 'allow';
    case 'allow_redacted':
      return 'redact';
    case 'block':
      return 'block';
  }
}

export function extractionLabel(e: ExtractionOutcome): string {
  switch (e.outcome) {
    case 'complete':
      return 'complete';
    case 'degraded':
      return `degraded · ${e.reason.replace(/_/g, ' ')}`;
    case 'failed':
      return 'failed';
  }
}
