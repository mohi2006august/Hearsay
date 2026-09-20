# brain.md — Hearsay

Working context for this project. Read this first in any new session. Update it
whenever a decision is made or reversed.

## What this project is

A proxy that stops instructions hidden inside images from being executed by a
vision-language agent. The contribution is the provenance boundary, not the
classifier.

## Decisions made

| Date | Decision | Reason |
| --- | --- | --- |
| — | Defence sits outside the model | Transfers across model versions; no training compute needed |
| — | Policy engine is deterministic | Must be explainable in a live demo |
| — | Redact regions rather than block requests | Keeps benign utility high |
| 2026-09-20 | Serving path is Rust; training stays Python | Predictable tail latency for the 400 ms p95 budget, one static binary, and the provenance boundary becomes an invariant the compiler checks rather than a convention |
| 2026-09-20 | **Reverses:** proxy is axum, not FastAPI | Follows from the Rust decision. `systemarchitecture.md` updated |
| 2026-09-20 | Declassification witness lives in `hearsay-core`, not `hearsay-policy` | `Provenanced::declassify` is defined in core, so naming a policy type there would close a dependency cycle. Policy holds the *right to mint* instead |
| 2026-09-20 | Tesseract ships before PaddleOCR | `leptess` is a working binding today; PP-OCRv4 needs ONNX export plus hand-written DB post-processing. Does **not** pre-empt the week-10 engine decision, which is still on recall |

## Decisions still open

- OCR engine: PaddleOCR vs Tesseract. Decide by week 4 on recall, not preference.
- Whether the classifier sees surrounding visual context or text alone.
- Corpus release: public, on request, or held. Ask the department.

## Conventions

- All experiments run under three seeds; anything reported as a single run is
  labelled provisional.
- Results live in `results/<experiment>/<seed>/metrics.json`. Never in a notebook only.
- Any number that goes in the report must be reproducible from a committed script.

## Known gotchas

- OCR recall on low-contrast rendered text is the silent failure mode. A missed
  region looks like a defence failure but is an extraction failure. Always report
  them separately.
- Benign utility degrades quietly. Run the benign suite every week, not at the end.
- The adaptive-attacker number will be much worse than the static number. That is
  expected and it belongs in the report, not hidden.
- The type-level provenance claim is easy to overstate. Rust has no friend
  modules, so the compiler prevents *implicit* flows across the boundary and
  reduces deliberate ones to a fixed, greppable set of call sites. It does not
  make laundering impossible for someone editing the workspace. Write the
  precise version in the report; the stronger version will not survive a
  reviewer who knows Rust. Full argument in `crates/hearsay-core/src/clearance.rs`.

## Current state

Week 0–1. `design.md` written. `hearsay-core` and `hearsay-policy` are built and
green: 51 tests passing on Rust 1.98.1, clippy warning-free. That includes five
compile-fail cases asserting the provenance boundary cannot be bypassed, which
is the evidence the write-up will lean on.

Next action: corpus taxonomy, and scope sign-off before any attack generation.
No OCR or classifier work until the taxonomy is frozen.

`CLAUDE.md` at the repo root has the operational detail — build commands, code
conventions, and where each divergence between these documents and the code
sits.

## Questions for the supervisor

- Is the department comfortable with an adversarial corpus being stored on lab
  machines, and under what retention rule?
- Target venue for the write-up, if any — it changes how much of the evaluation
  budget goes to baselines.
