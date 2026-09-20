# Hearsay

A transparent proxy that stops instructions hidden inside images from being
executed by a vision-language agent.

> The hearsay rule excludes an out-of-court statement offered for the truth of
> the matter asserted. This proxy applies the same rule to an agent: content it
> merely observed is admitted, inspected and logged — but never granted the
> authority of an instruction. Declassification is the narrow exception, taken
> on the record.

Existing prompt-injection defences inspect the text channel only. An
instruction rendered as pixels — text inside a screenshot, a caption in a PDF,
a label in a UI — reaches the model as an instruction but is never seen by a
text-side filter. The gap is not detection accuracy. It is that no trust
boundary exists between content the user authored and content the model merely
observed.

This project builds that boundary. **The contribution is the provenance
boundary, not the classifier.**

## The idea

Every input is labelled at ingestion. The user's typed prompt is the only
instruction-bearing source; everything else — uploaded images, fetched pages,
tool output, and every string OCR pulls out of a picture — is data. The label
travels with the content and cannot be changed by the content.

In Rust that label is part of the type:

```rust
// Different types, identical layout. Only one of them can reach the model
// as an instruction.
Provenanced<Instruction, String>   // the user's prompt
Provenanced<Data, String>          // text OCR found in a screenshot
```

The upstream request builder accepts only the first. Crossing between them
requires a `Declassification` witness that only the policy engine can mint,
that cannot be forged or reused, and that is written to the audit log.

This is a narrower guarantee than "the compiler prevents laundering" — Rust
has no friend modules, so a determined author inside the workspace can still
do it deliberately. What it does buy: implicit and accidental flows are
impossible, and deliberate ones are reduced to a fixed, greppable set of call
sites. The precise argument is in
[`crates/hearsay-core/src/clearance.rs`](crates/hearsay-core/src/clearance.rs) and
[`docs/design.md`](docs/design.md) §4.

## Status

**Early. Weeks 0–1 of 16.** The provenance boundary, the policy engine, an
HTTP surface and a dashboard exist and are tested — 57 tests passing, clippy
clean, frontend typecheck clean. The extraction and classification stages do
not exist yet, so scores are supplied by the caller rather than measured.

| Crate | State |
| --- | --- |
| `hearsay-core` | Domain types, provenance boundary, clearance witness |
| `hearsay-policy` | Deterministic engine, 7 rules, ruleset validation |
| `hearsay-api` | HTTP surface over the engine and an in-memory decision log |
| `web/` | Astro dashboard — decision feed, rule table, live evaluator |
| `hearsay-ocr`, `hearsay-normalize`, `hearsay-classify` | Not started |
| `hearsay-redact`, `hearsay-store`, `hearsay-proxy` | Not started |
| `hearsay-corpus`, `hearsay-eval` | Not started |

[`CLAUDE.md`](CLAUDE.md) carries live build status and the current resume
point. [`docs/brain.md`](docs/brain.md) is the decision log.

## Quick start

Requires Rust 1.98.1, pinned by `rust-toolchain.toml` and installed
automatically by rustup on first use. On Windows you also need the MSVC
linker — Visual Studio Build Tools with the "Desktop development with C++"
workload.

```bash
cargo test --workspace -j 1                 # unit, property, and compile-fail tests
cargo clippy --workspace --all-targets -j 1
cargo test -p hearsay-policy --test rules -j 1 # just the policy rules
```

`-j 1` is not incidental: on a machine with no pagefile, cargo's default
parallelism exhausts system commit and rustc dies with errors that look like
toolchain corruption. See [`docs/CLAUDE.md`](docs/CLAUDE.md) for the full
diagnosis.

### Running the dashboard

```bash
cargo run -p hearsay-api -j 1     # terminal 1 — http://127.0.0.1:8787
cd web && npm install && npm run dev   # terminal 2 — http://localhost:4321
```

The dashboard is a static Astro bundle talking to the Rust service. Details,
including the OneDrive `node_modules` caveat, are in
[`web/README.md`](web/README.md).

If your checkout is inside a synced folder (OneDrive, Dropbox), point the
build output elsewhere first — a Rust `target/` directory is tens of thousands
of files:

```powershell
setx CARGO_TARGET_DIR "$env:LOCALAPPDATA\cargo-target\hearsay"
```

## Layout

```
crates/
  hearsay-core/      domain types + the provenance boundary. no I/O, no async
  hearsay-policy/    deterministic policy engine
  hearsay-api/       HTTP surface: ruleset, decisions, evaluate, metrics
web/                 Astro dashboard (see web/README.md)
docs/
  brain.md              working context and decision log — read first
  prd.md                problem, threat model, requirements, evaluation plan
  systemarchitecture.md component decomposition
  design.md             low-level design: types, rules, latency budget, build order
CLAUDE.md         operational context: build commands, conventions, resume point
```

## Tests worth knowing about

`crates/hearsay-core/tests/ui/` holds five programs that **must fail to
compile** — attempts to unwrap data-channel content, to pass it where an
instruction is expected, to forge a clearance, to mint one without authority,
and to invent a third channel. They are the strongest evidence the project
has: a passing runtime test shows the defence worked on inputs we thought of,
while these show a class of bypass is not expressible.

`crates/hearsay-policy/tests/rules.rs` has one test per policy rule. It is also
the demo's script — if a rule changes, it should fail there before anyone
notices in an evaluation run.

## Evaluation

Primary metric is attack success rate reduction on a held-out adversarial
corpus, reported against benign task completion on the same agent. Five
conditions: undefended, text-only filter, ours against static attacks, ours
against an adaptive attacker who knows the defence, and a benign suite.

Three things this project commits to reporting honestly:

- **OCR recall separately from ASR.** A missed region is an extraction
  failure, not a defence failure. Collapsing them inflates the headline
  number.
- **The adaptive-attacker condition.** It will be much worse than the static
  number. That belongs in the report, not hidden.
- **Utility cost.** Benign task completion must not drop more than 3 points,
  and it is tracked weekly rather than measured at the end.

Results live in `results/<experiment>/<seed>/metrics.json`, three seeds per
experiment, every number reproducible from a committed script.

## Scope and ethics

The adversarial corpus is generated against models and agents we control.
**No testing against third-party production agents.** The corpus is held
privately and released, if at all, only with the department's approval.
Written scope sign-off is required before week 3. See [`docs/prd.md`](docs/prd.md)
§9.

The threat model is a third party injecting content the user did not author.
Defending against a user who jailbreaks their own agent is explicitly out of
scope.

## License

MIT.
