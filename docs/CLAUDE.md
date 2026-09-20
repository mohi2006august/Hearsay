# CLAUDE.md

Operational context for working in this repo. Read `docs/brain.md` first for
research decisions; this file is about the code and where to pick it up.

## What this is

A Rust proxy that stops instructions hidden inside images from being executed
by a vision-language agent. The contribution is the **provenance boundary**,
not the classifier: content the agent merely observed must never be executed
as an instruction.

16-week research project. Deliverables are a deployed proxy, an adversarial
corpus, results tables including an adaptive-attacker condition, and a 6–8
page report.

## Documents

| File | What it is |
| --- | --- |
| `docs/brain.md` | Working context and decision log. **Update it whenever a decision is made or reversed.** |
| `docs/prd.md` | Problem, threat model, requirements, evaluation plan |
| `docs/systemarchitecture.md` | Component decomposition |
| `docs/design.md` | Low-level design — crate layout, types, rules, latency budget, build order |

`docs/design.md` is the one to read before writing code. Its §14 is the build
order this work follows.

## Status — as of 2026-09-20

**Green.** Rust 1.98.1 MSVC installed; `cargo test --workspace` passes 57 tests,
`cargo clippy --workspace --all-targets` is warning-free, `cargo fmt --check`
is clean.

| Suite | Tests |
| --- | --- |
| `hearsay-core` unit | 20 |
| `hearsay-core` compile-fail (trybuild) | 5 cases, 1 harness test |
| `hearsay-core` property (proptest) | 5 |
| `hearsay-policy` unit | 6 |
| `hearsay-policy` rules | 19 |
| `hearsay-api` normalise | 6 |

The frontend typechecks clean (`astro check`: 0 errors, 0 warnings) and builds
to ~90 KB total.

The `.stderr` expectations in `crates/hearsay-core/tests/ui/` are generated and
committed. They now verify rather than regenerate — a change in what the
compiler refuses will fail the build.

### Built

```
Cargo.toml              workspace, pinned deps, shared lints
rust-toolchain.toml     pinned 1.98.1
crates/hearsay-core/       domain types + the provenance boundary
  src/provenance.rs       Provenanced<C, T>, Channel (sealed), Origin
  src/clearance.rs        ClearanceAuthority (unsafe trait), Declassification
  src/ids.rs              RequestId, DecisionId, RegionId, ContentHash, versions
  src/region.rs           BBox, TextRegion, Extraction, ExtractionOutcome
  src/text.rs             NormalizedText with offset map back to source
  src/decision.rs         RegionAction, RequestOutcome, Score
  tests/compile_fail.rs   trybuild driver
  tests/ui/*.rs           5 programs that must NOT compile
  tests/invariants.rs     proptest
crates/hearsay-policy/     deterministic engine
  src/ruleset.rs          thresholds + validation
  src/engine.rs           the 7 rules, aggregation, clearance minting
  tests/rules.rs          one test per rule + aggregation + degrade paths
crates/hearsay-api/     HTTP surface (NOT hearsay-proxy)
  src/record.rs           wire types, mirroring design.md §7
  src/pipeline.rs         normalise -> policy; where OCR/classify slot in
  src/normalize.rs        PLACEHOLDER for hearsay-normalize, see below
  src/state.rs            in-memory decision ring, 500 entries, not durable
  src/routes.rs           ruleset, decisions, stats, evaluate, metrics
  src/seed.rs             boot fixtures, real engine output, tagged `seed`
web/                    Astro dashboard, static output, no UI framework
  src/lib/types.ts        hand-maintained mirror of record.rs — drifts silently
  src/lib/api.ts          typed client, timeouts, polling with backoff
  src/lib/dom.ts          node construction, never innerHTML
  src/pages/              index (summary), decisions (browser), policy (evaluator)
```

### Not built

`hearsay-ocr`, `hearsay-normalize`, `hearsay-classify`, `hearsay-redact`,
`hearsay-store`, `hearsay-proxy`, `hearsay-corpus`, `hearsay-eval`, `xtask`.
The workspace `members` list names only the three crates that exist — add each
one as it lands.

**Two placeholders are standing in, and both must be replaced before any
number is reported:**

- `hearsay-api/src/normalize.rs` strips zero-width and bidi characters and
  collapses whitespace. It does **not** do NFKC or confusable folding, so
  homoglyph attacks walk straight through. There is a test asserting that
  failure so it stays visible.
- Scores are supplied by the caller. Nothing classifies anything yet, so no
  ASR figure exists and none can.

## Pick up here

Next is **`hearsay-corpus`**. `design.md` §14 puts corpus taxonomy before method
work, and `prd.md` §10 flags a too-small corpus as a headline risk. Ground
truth is generated rather than annotated — the renderer knows where it drew
the text, so bbox and string are exact.

Do not start OCR or the classifier before the corpus taxonomy is frozen and
scope sign-off has landed (`prd.md` §9, required before week 3).

## Build environment — read before your first build

### REBOOT PENDING

A system-managed pagefile was enabled on 2026-09-20
(`AutomaticManagedPagefile = True`), but **Windows only creates it on
restart**. Until this machine reboots the commit limit is still 15.64 GB with
roughly 0.8 GB free, and the constraint below still applies. After the reboot,
confirm with:

```powershell
Get-CimInstance Win32_PageFileUsage | Select-Object Name, AllocatedBaseSize
```

then run `cargo test --workspace -j 1` and expect **57 passing**. The
`hearsay-core` compile-fail suite is the one thing that has not been run since
the workspace gained `hearsay-api` — it spawns a nested cargo build and needs
more commit headroom than currently exists. Its `.stderr` files were generated
and verified earlier in the session against identical `hearsay-core` sources,
so it is expected to pass, but it is unconfirmed.

### Why -j 1

This machine had **no pagefile**, so the commit limit equalled physical RAM
(15.64 GB) with nothing backing it, and ~14.8 GB was already committed by WSL,
Defender, VS Code and OneDrive. Builds hit the ceiling and fail with errors
that impersonate toolchain corruption:

| Symptom | Actually |
| --- | --- |
| `link.exe failed: exit code 0xc000012d` | `STATUS_COMMIT_LIMIT` — the paging file is too small |
| `exit code 0xc0000409` | `STATUS_STACK_BUFFER_OVERRUN` in rustc |
| `error 1453` | `ERROR_NO_SYSTEM_RESOURCES` |
| ICE in `DroplessArena::grow` | rustc's arena allocator failing |
| ``cannot find value `None` in this scope`` | the sysroot failed to map |
| `#[panic_handler] function required` | same |

None of these mean the toolchain is broken — a trivial `rustc` compile
succeeds throughout, which is the check that rules it out. Do not `cargo
clean` while diagnosing one of these: a full rebuild needs more commit than an
incremental one, and it makes the situation worse. Build in stages
(`-p hearsay-core`, then `-p hearsay-policy`, then `-p hearsay-api`) if a
whole-workspace build will not fit.

```powershell
$env:CARGO_BUILD_JOBS = "1"
```

Closing Ollama frees ~630 MB, and `wsl --shutdown` another ~840 MB, if a build
still struggles before the reboot.

**Toolchain is 1.98.1**, pinned in `rust-toolchain.toml`. The original 1.85
pin was too old — `trybuild` requires 1.88+.

**Target directory is outside OneDrive** via `CARGO_TARGET_DIR`; see below.

## Commands

```bash
cargo test --workspace -j 1                    # 57 tests
cargo clippy --workspace --all-targets -j 1
cargo fmt --all
cargo test -p hearsay-policy --test rules -j 1    # the demo's script

cargo run -p hearsay-api -j 1                  # :8787
cd web && npm run dev                          # :4321, proxies /api to 8787
cd web && npm run build                        # astro check && astro build
```

Regenerate the compile-fail expectations only after a rustc upgrade changes
diagnostic wording, and read the diff before committing:

```bash
TRYBUILD=overwrite cargo test -p hearsay-core --test compile_fail -j 1
```

### OneDrive

This repo lives under OneDrive. A Rust `target/` directory is tens of
thousands of files and will be synced continuously. `target/` is gitignored,
but that does not stop OneDrive. Before the first build:

```powershell
setx CARGO_TARGET_DIR "$env:LOCALAPPDATA\cargo-target\hearsay"
```

The same applies to `web/node_modules`, and worse: OneDrive's filter corrupts
npm's tar extraction outright. The symptom is `TAR_ENTRY_ERROR ... write`
during install, then `SyntaxError: Invalid or unexpected token` from Node
because files land with NUL bytes in them. `web/node_modules` is already a
junction to `%LOCALAPPDATA%\hearsay-web
ode_modules`; keep it that way, and
recreate the junction rather than letting npm write into the synced folder.

## Code conventions

- **`hearsay-core` has no I/O, no async and no workspace dependencies.** It is a
  leaf so the provenance invariant can be property-tested without a runtime.
  Keep it that way.
- **Dependency direction is strictly downward.** `hearsay-proxy` depends on
  everything; nothing depends on it.
- **`#![forbid(unsafe_code)]` in every crate** except `hearsay-core` and
  `hearsay-policy`, which `#![deny]` it and carry one documented `#[allow]` each.
  The audit:

  ```bash
  grep -rn "unsafe impl\|unsafe trait" crates/ --include=*.rs | grep -v "^.*tests/"
  ```

  Expect exactly three lines, and no more:
  - `hearsay-core/src/clearance.rs` — `unsafe trait ClearanceAuthority` (declaration)
  - `hearsay-core/src/provenance.rs` — `unsafe impl` for `TestAuthority`, inside
    `#[cfg(test)]`, so the core crate can test the mechanism without depending
    on `hearsay-policy`
  - `hearsay-policy/src/engine.rs` — `unsafe impl ClearanceAuthority for PolicyEngine`,
    the only real authority

  A fourth means the boundary has been widened and the report's claim is void.
- **`thiserror` in libraries, `anyhow` only in binaries.** No `anyhow` in a
  library signature.
- **The policy engine is sync, total and pure.** No `async`, no `Result`, no
  model call in `decide_region`. It is a concrete type, not a trait, on
  purpose — a swappable policy makes the audit log's ruleset stamp meaningless.
- Tests are named as sentences describing the property, not `test_foo`.

## Research conventions (from `docs/brain.md` — do not lose these)

- Three seeds for every experiment. A single run is labelled provisional.
- Results go to `results/<experiment>/<seed>/metrics.json`. Never a notebook.
- Any number in the report must be reproducible from a committed script.
- **OCR recall is reported separately from ASR.** A missed region is an
  extraction failure, not a defence failure. `ExtractionOutcome` exists to
  keep these separable — never collapse them into one number.
- Run the benign suite weekly from week 6, not at the end.
- The adaptive-attacker number will be much worse than the static one. That
  belongs in the report, not hidden.

## Divergences from the docs, found while implementing

These are real and already reflected in the code. `docs/design.md` has been
amended for the first two; the rest are noted here only.

1. **`Declassification` lives in `hearsay-core`, not `hearsay-policy`.**
   `Provenanced::declassify` is defined in core, so naming a `hearsay-policy`
   type there would close a dependency cycle. What `hearsay-policy` holds is the
   *right to mint* one, via the `ClearanceAuthority` unsafe trait.
2. **The type-level guarantee is narrower than `docs/design.md` §4 first
   claimed.** Rust has no friend modules, so no visibility arrangement makes a
   type constructible by exactly one other crate. What is actually enforced:
   implicit and accidental flows are impossible; deliberate ones are reduced
   to a fixed, greppable set of call sites (`Provenanced::user_prompt`,
   `declassify`, `unsafe impl ClearanceAuthority`). State it that way in the
   report — the precise version is still a strong claim, and the overstated
   version will not survive a reviewer. The full argument is in the module
   docs of `crates/hearsay-core/src/clearance.rs`.
3. **`ExtractionOutcome::Failed` carries a `String`, not an `OcrError`.**
   Same dependency-cycle reason. `hearsay-ocr` stringifies at the boundary.
4. **`docs/systemarchitecture.md` said FastAPI.** Superseded by axum; the
   table row has been updated. The rest of that document's decomposition
   still holds.

## Open decisions

Carried from `docs/brain.md`, with what the code already accommodates.

- **OCR engine** — Tesseract via `leptess` ships first; PP-OCRv4 via ONNX is
  roughly a week of work for the ablation. Decide on week-10 recall, not
  preference. The engine is a runtime config switch, never a code branch.
- **Classifier visual context** — `ClassifyInput::crop` reserves the shape.
  Text-only is the default until an ablation earns the latency.
- **Corpus release** — public, on request, or held. Ask the department.
- **Log retention** — decision rows contain adversarial strings. Supervisor
  question; determines whether the image store needs a TTL sweeper.

## Questions waiting on the supervisor

- Retention rule for an adversarial corpus on lab machines.
- Target venue for the write-up — it changes how much evaluation budget goes
  to baselines.
