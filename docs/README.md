# Documentation

Project documentation for **HEIC Image Converter** 0.1.0. Written after the
prototype was built, as input for an SSC adoption pass.

| Document | What it is for |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | How the system is put together: modules, data flow, IPC contract, output rules, threading, security posture |
| [BEHAVIOR.md](BEHAVIOR.md) | What a user sees, edge cases, explicit limitations, and an honesty-first verification matrix (what was tested, what was not) |
| [DECISIONS.md](DECISIONS.md) | Decision log with rationale, consequences, alternatives, and open questions |
| [BUILD.md](BUILD.md) | Prerequisites, codec binary download, dev/test/build, packaging, CI, troubleshooting |
| [SSC-HANDOFF.md](SSC-HANDOFF.md) | How to convert this into an SSC project: seeds for PRODUCT/BASELINE, suggested stages and features, review checklist |

## Status

This repository predates its SSC artifacts. There is no `.ssc/` directory yet;
the intended entry point is `ssc-plan` in brownfield/adopt mode, using
`SSC-HANDOFF.md` as the briefing.

## Ground rules for these docs

- They describe the code as it exists at commit `3077fb1`.
- Claims marked **Not run** in `BEHAVIOR.md` are genuinely unverified — do not
  promote them to baseline without evidence.
- These documents are review input. They are not PRODUCT.md, BASELINE.md,
  stage files, or feature specs, and should not be copied into `.ssc/`.
- If the code and a document disagree, the code wins. Fix the document.

## Screenshot

`docs/screenshot.png` was captured from the running app with a generated
sample set. The sample images are not part of the repository.
