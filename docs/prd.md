# PRD — Hearsay: Cross-Modal Prompt Injection Defence for Vision-Language Agents

**Status:** draft · **Owner:** <your name> · **Duration:** 16 weeks

## 1. Problem

Vision-language agents accept images, screenshots, scanned documents and rendered
web pages as input. Existing prompt-injection defences inspect the text channel
only. An instruction rendered as pixels — text inside a screenshot, a caption in a
PDF, a label in a UI — reaches the model as an instruction but is never seen by a
text-side filter.

The gap is not detection accuracy. It is that no trust boundary exists between
content the user authored and content the model merely observed.

## 2. Goal

Enforce a provenance boundary: content arriving through a data channel (any
retrieved or observed input, regardless of modality) must never be executed as an
instruction, while benign task performance is preserved.

## 3. Non-goals

- Defending against a user who jailbreaks their own agent. The threat model is a
  third party injecting content the user did not author.
- Text-only prompt injection. Existing work covers this; we build on it.
- Model retraining. The defence sits outside the model so it transfers.

## 4. Threat model

| Dimension | Assumption |
| --- | --- |
| Adversary goal | Cause the agent to execute an instruction the user did not issue |
| Capability | Controls content the agent will observe (a web page, a document, an image the user uploads from an untrusted source) |
| Knowledge | Gray-box: knows the agent's architecture and that a defence exists; does not have model weights or the defence's parameters |
| Access | No query access to the defence at training time; adaptive attacker at evaluation time |
| Out of scope | Compromise of the host, the model weights, or the user's own prompt |

## 5. Functional requirements

- **FR-1** Accept a multimodal request (text + one or more images) and label every
  region of every input as instruction-bearing or data-bearing.
- **FR-2** Extract text embedded in images and evaluate it under data-channel
  rules, never instruction-channel rules.
- **FR-3** Return an allow / flag / block decision with a machine-readable reason
  and the offending region.
- **FR-4** Expose a single HTTP endpoint that wraps any upstream VLM, so the
  defence is a drop-in proxy.
- **FR-5** Log every decision with the input hash for later audit and replay.

## 6. Non-functional requirements

- Added latency under 400 ms at p95 for a single 1080p image.
- Benign task completion rate must not drop more than 3 points versus undefended.
- Runs on a single consumer GPU; no dependence on a paid API for the defence path.

## 7. Evaluation plan

Primary metric: attack success rate (ASR) reduction on a held-out adversarial
corpus, reported against benign task completion rate on the same agent.

| Condition | What it measures |
| --- | --- |
| Undefended baseline | Raw ASR |
| Text-only filter baseline | How much of the problem existing work already solves |
| Ours, static attacks | Headline ASR reduction |
| Ours, adaptive attacker | The honest number — attacker knows the defence |
| Benign suite | Utility cost |

Report mean and standard deviation over three seeds. Report per-attack-family
breakdown, not one aggregate number.

## 8. Deliverables

- Proxy service and defence model, deployed and reachable.
- Adversarial corpus with generation scripts and a documented taxonomy.
- Results tables including the adaptive-attacker condition and a failure-mode section.
- 6–8 page technical report.

## 9. Scope and ethics

The adversarial corpus is generated against models and agents we control. No
testing against third-party production agents. The corpus is held privately and
released, if at all, only with the department's approval. Written scope sign-off
required before week 3.

## 10. Risks

| Risk | Mitigation |
| --- | --- |
| OCR quality dominates results | Report OCR recall separately; ablate OCR engine |
| Corpus too small to be convincing | Fix corpus size and taxonomy in weeks 3–4, before method work |
| Benign utility collapses | Track utility from day one, not at the end |
