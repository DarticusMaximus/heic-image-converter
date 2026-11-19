# Documentation

Project documentation for **HEIC Image Converter** 0.1.0. Written after the
prototype was built, as input for an SSC adoption pass.

| Document | What it is for |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the system is put together: modules, data flow, IPC contract, output rules, threading, security posture |
| [BEHAVIOR.md](BEHAVIOR.md) | What a user sees, edge cases, explicit limitations, and an honesty-first verification matrix (what was tested, what was not) |
| [DECISIONS.md](DECISIONS.md) | Decision log with rationale, consequences, alternatives, and open questions |
| [BUILD.md](BUILD.md) | Prerequisites, codec artifact pin, dev/test/build, the offline gate, packaging, CI, troubleshooting |
| [HEIC-SOURCE-REVIEW.md](HEIC-SOURCE-REVIEW.md) | Source review of the vendored `heif-rs` crate and the linked codec archive: what was read, the unsafe sites, the findings, and which risks are accepted |
| [UPGRADE-HEIC.md](UPGRADE-HEIC.md) | The upgrade runbook for `heif-rs` and its bundled codec archives: when to bump, the S0–S9 procedure, verification, rollback, and the upgrade log |
| [SSC-HANDOFF.md](SSC-HANDOFF.md) | How to convert this into an SSC project: seeds for PRODUCT/BASELINE, suggested stages and features, review checklist |

## Status

The project now uses SSC artifacts under `.ssc/`: product vision, the staged
plan, stage files, and feature specs. The documents in this folder describe the
prototype at commit `3077fb1` and are historical review input;
[BEHAVIOR.md](BEHAVIOR.md) carries the current dated verification matrix.

## Ground rules for these docs

- They mostly describe the prototype at commit `3077fb1` and are not a statement
  of current behavior. [HEIC-SOURCE-REVIEW.md](HEIC-SOURCE-REVIEW.md) and
  [UPGRADE-HEIC.md](UPGRADE-HEIC.md) are current records, kept up to date as the
  dependency moves.
- These documents are review input. They are not PRODUCT.md, BASELINE.md,
  stage files, or feature specs, and should not be copied into `.ssc/`.
- If the code and a document disagree, the code wins. Fix the document.

## Screenshot

`docs/screenshot.png` was captured from the running app with a generated
sample set. The sample images are not part of the repository.
