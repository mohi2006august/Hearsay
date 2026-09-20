# hearsay-web

Decision dashboard for the Hearsay provenance proxy. Astro, static output, no
UI framework.

## Running it

Two processes. The API first, because the dashboard is useless without it:

```bash
# terminal 1 — from the repo root
cargo run -p hearsay-api -j 1          # http://127.0.0.1:8787

# terminal 2 — from web/
npm install
npm run dev                            # http://localhost:4321
```

In dev the Vite proxy forwards `/api` to the Rust service, so the browser
stays same-origin and CORS never comes up.

```bash
npm run build      # astro check && astro build -> dist/
npm run preview    # serve dist/ locally
```

A built bundle talks to `http://127.0.0.1:8787` unless you set
`PUBLIC_HEARSAY_API`. For a real deployment, serve `dist/` and the API from
one origin and leave the variable unset.

### If your checkout is inside OneDrive

`node_modules` must live outside the synced folder. OneDrive's filter
corrupts npm's tar extraction — the symptom is `TAR_ENTRY_ERROR ... write`
during install and then `SyntaxError: Invalid or unexpected token` from Node,
because several files land with NUL bytes in them. A junction fixes it:

```powershell
Remove-Item -Recurse -Force node_modules
New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\hearsay-web\node_modules"
New-Item -ItemType Junction -Path node_modules -Target "$env:LOCALAPPDATA\hearsay-web\node_modules"
npm install
```

This repo is already set up that way.

## Pages

| Route | What it is for |
| --- | --- |
| `/` | Summary tiles, latency against the 400 ms budget, which rules are firing, recent decisions |
| `/decisions` | Master-detail browser over the log — every region, score, rule and reason |
| `/policy` | Active thresholds and lexicon, the seven rules, and a live evaluator |

The evaluator on `/policy` posts to `/api/evaluate` and runs the real engine.
It is the demo `design.md` keeps referring to: pick a preset, move the score
slider, watch which rule fires and why. The presets cover a benign screenshot,
the textbook injection, an exfiltration instruction, a zero-width obfuscation,
a low-contrast page and the user's own prompt (which R1 allows however badly
it scores).

## How it is built

**No UI framework.** Astro renders the static shell; three small vanilla-TS
islands fetch and render. The whole site is ~90 KB including CSS, and the
largest island is 5 KB. React would have cost more than it returned for three
pages of tables.

**Everything comes from the API.** The rule table, the thresholds, the
severity vocabulary and even the latency budget are served by
`/api/ruleset` and `/api/stats` rather than hardcoded, so the dashboard cannot
drift from the engine. If a threshold changes in `ruleset.toml`, this UI shows
the new one without a redeploy.

**Nodes, not markup strings.** Every string rendered here is, by the
project's own threat model, attacker-controlled — it is text lifted out of an
image someone planted. `src/lib/dom.ts` builds DOM nodes and appends text
nodes; nothing interpolates into `innerHTML`. An XSS in the tool that exists
to display injections would be a poor look.

**Invisible characters are made visible.** A zero-width space is the entire
trick in several attack families, so the decision detail renders them as
`␣ ⇄ ·` and highlights the run. Rendering them as nothing would hide the
evidence the page exists to show.

**Fixtures are labelled.** Records the API seeded at boot carry a `seed` chip
and a notice saying their timings are budget figures from `design.md` §9, not
measurements. Live records say the opposite: only the policy stage is timed,
because OCR and the classifier are not built. `brain.md` is explicit that
anything provisional gets labelled, and a dashboard is exactly where that
slips.

**Failure is a state, not a spinner.** The API is a real dependency. The
header carries a connection indicator, polling pauses while the tab is hidden
and backs off to 30 s while the service is down, and every fetch has a
timeout. A dashboard showing stale numbers during an outage is worse than one
saying it cannot reach the service.

## Design

Tokens live in `src/styles/global.css`. Two colour systems, deliberately
separate:

- **Accent** (deep teal) is identity. It marks the product, never a state.
- **Severity** encodes `RegionAction`'s real ordering — allow, flag, redact,
  block. `block` is near-black because that is the colour of a redaction bar.

Type is Newsreader for display, IBM Plex Sans for body, IBM Plex Mono for
rule IDs, hashes and anything with digits that line up. Light and dark are
both defined token-level on bare `:root` first, so the un-stamped
system-preference case renders correctly.

## Layout

```
src/
  lib/
    types.ts     wire types, mirroring crates/hearsay-api/src/record.rs
    api.ts       typed client: timeouts, one error shape, polling with backoff
    dom.ts       node construction — no innerHTML
    format.ts    relative time, scores, hashes, invisible-character reveal
  layouts/Base.astro    shell, nav, theme toggle, connection indicator
  pages/                index, decisions, policy
  styles/global.css     tokens and primitives
```

`types.ts` is hand-maintained against the Rust. `npm run check` will not catch
drift there — only a failing request will. If you change a serde attribute in
`record.rs`, change it here too.
