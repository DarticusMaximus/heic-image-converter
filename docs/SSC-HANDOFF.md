# SSC handoff

> This project was built first and needs to be retrofitted into SSC. This
> document is the bridge: it tells a fresh session what exists, what is proven,
> what is assumed, and how to turn it into `.ssc/` artifacts without losing the
> working code or the original intent.
>
> **This file is not an SSC artifact.** Do not paste it into `.ssc/` verbatim.
> Treat it as review input and let `ssc-plan` / `ssc-spec` produce the real
> layers.

---

## State of play

- The app is a **working prototype** (implementation commit `3077fb1`; this
  documentation follows in a separate commit) with no `.ssc/` directory.
- The SSC entry point is **`ssc-plan` in brownfield/adopt mode**, followed by
  `ssc-spec` per feature.
- The code is real and mostly exercised; the *process* is what is missing.
- The largest risk is treating the prototype's behavior as requirements without
  challenging it.

### Suggested session prompt

> Read `docs/SSC-HANDOFF.md`, `docs/ARCHITECTURE.md`, `docs/BEHAVIOR.md`,
> `docs/DECISIONS.md`, and `docs/BUILD.md`. Run `ssc-plan` for brownfield
> adoption: no `.ssc/` exists yet. Verify the claims in these docs (run the
> tests, inspect the code) before treating anything as baseline. Keep the
> product's core constraints — fully offline, no uploads, safe file handling,
> simple quality controls — and be critical of the rest.

### Suggested reading order

1. `BEHAVIOR.md` — what a user experiences, and what was never tried.
2. `DECISIONS.md` — why it is shaped this way, and what each choice costs.
3. `ARCHITECTURE.md` — how it is built, module by module.
4. `BUILD.md` — how to run, test, and package it.
5. The code, starting at `src-tauri/src/convert.rs` and `src/App.tsx`.

### Verify before you trust

```bash
cd src-tauri && cargo test        # 13 tests, no network needed after first build
npm run build                     # typecheck + frontend bundle
npm run tauri build               # real installers (slow, LTO)
```

Then check the **Not run** rows in `BEHAVIOR.md`'s verification matrix. Those
are honest gaps, not claims of failure.

---

## Seeds for `PRODUCT.md` (Layer 1 — the north star)

Let the PM confirm or rewrite every line. This is a proposal distilled from the
original request.

**Why this exists**

Converting a HEIC photo is a five-second task that the software industry has
made annoying. Free GUI tools are scarce, Windows-centric, or bundled with
installers; online converters demand an upload, and "we convert on your device"
is a promise nobody can audit. This app exists so a normal person can convert a
folder of iPhone photos without sending them anywhere, and without learning a
CLI.

**Who it is for**

- Someone with a folder of iPhone photos who needs JPEGs for a website,
  printer, or older software.
- Privacy-conscious users who will not upload personal photos.
- Desktop users on Linux, Windows, or macOS who want one tool that behaves the
  same everywhere.

**What it must never become**

- A service with accounts, uploads, telemetry, or a cloud dependency.
- A tool that silently modifies or deletes originals.
- A settings maze. The default path must stay under a minute: add files, pick a
  format, convert.

**What "good" looks like**

- Zero decode/encode dependencies for the user to install.
- A batch of ordinary phone photos converts without babysitting.
- The estimate is close enough to trust before pressing Convert.
- The output opens in ordinary software and has the right orientation.

**Non-goals (for now)**

Photo editing, RAW development, format conversion for video, cloud sync,
library management, print layout.

---

## Seeds for `BASELINE.md` (brownfield territory map)

### Shape

Tauri v2 desktop app: React/TS frontend, Rust backend, all image work in Rust.
Supported input: HEIC/HEIF, JPEG, PNG, WebP, TIFF, BMP, GIF. Output: PNG, JPEG,
HEIC. Folder-aware queue, sample-based size estimates, four quality presets,
optional longest-edge resize.

### What works (evidence in `BEHAVIOR.md`)

- HEIC ⇄ JPEG/PNG conversion, including 10/12-bit HEIC input.
- HEIC container rotation and JPEG EXIF orientation.
- Folder recursion, structure mirroring, collision-safe naming.
- Sample-based size estimation with per-file and batch totals.
- Native pickers, reveal-in-file-manager, startup arguments.
- Linux release bundles build; UI exercised on Omarchy/Hyprland.

### What is unproven

- Windows and macOS runtime behavior.
- Drag-and-drop, cancellation, corrupt inputs, permission errors.
- Batches beyond a handful of images; performance characteristics.
- Multi-image HEIC, animated GIF, 16-bit TIFF specifics.
- Anything requiring a screen reader or keyboard-only navigation.
- Installer semantics (sidebar links, file associations, upgrades).

### Sacred (do not break without an explicit product decision)

1. **Fully offline.** No runtime network. No upload path, not even optional.
2. **Originals are never modified or deleted.**
3. **No silent overwrite.** Collisions rename; the source path is never a
   target.
4. **Plain-language quality.** No raw codec numbers in the primary UI.
5. **One obvious path**: add → format → convert.
6. **No metadata leaks.** If metadata preservation is ever added, it must be
   opt-in and must default to dropping GPS.

### Changeable (expect to refactor)

- The hand-rolled CSS and component structure; there are no frontend tests.
- The specific quality→codec mapping.
- Settings storage (localStorage vs config file).
- Window chrome, layout, empty-state copy, branding.
- The `heif-rs` dependency, if a better-maintained option appears.
- The single-image-per-HEIC behavior.
- The AppImage's size and packaging choices.

### Change appetite

The PM has not stated one yet. Ask. Current instinct: keep the conversion core
and its safety guarantees, refactor the UI freely, avoid adding features until
the baseline is specified and verified.

### Risk register

| Risk | Severity | Notes |
|---|---|---|
| `heif-rs` is a young single-maintainer crate with a date-based version and a build-time download | High | Pin it, vendor the archive for offline builds, watch for abandonment |
| GPL obligation from x265 | Medium | Intentional; affects distribution and store plans |
| Windows/macOS untested | High | CI builds it; nobody has run it |
| No frontend tests, UI state is intricate | Medium | Many interacting effects; refactor under specs |
| Large-batch behavior unmeasured | Medium | Estimator cost, memory, thread oversubscription with x265 |
| Metadata policy is implicit | Medium | Needs a product decision, not just a default |
| Settings live in webview storage | Low | Hard to support, lost with identifier changes |
| No single-instance handling | Low | Two launches can convert concurrently |

### Test coverage

13 Rust tests, concentrated on the conversion core and path planning. Zero
frontend tests. Manual verification was screenshot- and keyboard-driven on Linux
only. Coverage is good where data loss is possible, thin everywhere else.

---

## Suggested stage decomposition

Do not copy this blindly. It is a starting point that respects the existing
code instead of pretending the project is greenfield.

### Stage 00 — Adopt and prove the prototype

**Intent:** turn the working binary into a trustworthy baseline with a spec
trail, closing the verification gaps that could invalidate everything after it.

Candidate features:

1. **`feature-01-baseline-verification`**
   Intent: prove the documented behavior on every target platform before
   building on it.
   Tasks: reproduce the verification matrix on Linux; smoke-test Windows and
   macOS CI artifacts; add automated tests for JPEG EXIF orientation, corrupt
   input, and cancellation; record results in `BASELINE.md`.

2. **`feature-02-conversion-correctness`**
   Intent: make the core's safety and correctness properties executable.
   Tasks: property-style tests for output naming; golden tests for quality
   mappings; explicit behavior for multi-image HEIC and animated GIF (decode
   primary frame, or error clearly — decide); validate output with an external
   decoder in CI.

3. **`feature-03-batch-reliability`**
   Intent: batches of real phone libraries survive.
   Tasks: benchmark 100–500 mixed files; measure memory; tune worker counts;
   test cancel mid-batch; test permission-denied and read-only outputs; define
   a batch-size ceiling and communicate it.

4. **`feature-04-desktop-integration`**
   Intent: the app behaves like a desktop app, not a demo.
   Tasks: decide and implement single-instance behavior; `.desktop` file with
   MIME associations; "Open with" end-to-end; verify native dialogs and reveal
   on all platforms.

### Stage 01 — Product completeness

**Intent:** decisions the prototype deferred, made deliberately.

Candidate features:

1. **`feature-05-metadata-policy`** — opt-in preservation, default off, GPS
   never on by default; UI copy that matches the policy.
2. **`feature-06-target-size-mode`** — "make these under X MB" as the
   higher-value form of quality control; uses the existing sample encoder.
3. **`feature-07-quality-preview`** — encode one small preview per quality step
   so "smallest vs maximum" is visible, not described.
4. **`feature-08-accessibility-and-keyboard`** — full keyboard flow, focus
   management, screen-reader pass, contrast audit.
5. **`feature-09-packaging-and-signing`** — signed Windows/macOS builds,
   notarization, an update channel or an explicit "manual updates" decision,
   installer polish.

### Stage 02 — Scope expansion (only if the product says so)

WebP/AVIF output, presets/profiles, a resumable job queue, photo-library
import, localization. All of these fight the "simple tool" constraint; require
an explicit product decision.

---

## Proposed feature intents for the *existing* code

If the PM prefers to retro-spec what already exists before hardening it, these
are the capabilities as they stand. Each intent should trace to the north star.

| Feature | Intent | Exists in |
|---|---|---|
| conversion core | Turn any supported input into a correct, oriented, appropriately sized output without touching the original | `convert.rs`, `decode.rs` |
| format & quality model | Give users four comprehensible tradeoffs instead of codec knobs | `formats.rs` |
| folder queue | Handle a whole library without the user selecting files one by one | `scan.rs` |
| size estimation | Let users predict the result before committing | `estimate.rs` |
| desktop shell & IPC | Keep the UI responsive while Rust does the work | `lib.rs`, `api.ts` |
| queue UI | Make progress, failures, and results obvious | `App.tsx`, `FileList` |
| settings rail | Keep the default path short while exposing the few options that matter | `SettingsRail` |
| packaging | Deliver one installable artifact per platform with no codec setup | `tauri.conf.json`, workflows |

The value of this option is a complete blueprint set; the cost is that it
declares unverified behavior "done". A hybrid works well: retro-spec the core
(a feature per row above), then make Stage 00's verification features the next
specs.

---

## Open questions for the planning session

Ask these; the answers change the stage map.

1. **Platform priority.** Is Windows a day-one target, or is Linux enough while
   the design settles? This decides whether Stage 00 includes Windows smoke
   tests or a separate platform stage.
2. **Metadata.** Feature or non-goal? If feature, opt-in or default-on (with
   GPS stripped)?
3. **HEIC output.** Keep it knowing the app is GPL, or drop it for a permissive
   license? The answer changes packaging and distribution plans.
4. **Scale.** What is the realistic largest batch — a camera roll (5k) or a
   handful of files? It decides whether the current all-at-once queue is
   acceptable or a resumable job model is required.
5. **Branding.** Stay literal ("HEIC Image Converter") or invest in a name
   before the first public release? The bundle identifier cannot change without
   losing stored settings.

Do not ask all five in one turn; pick the two that block planning.

---

## Review checklist for the new session

**Process**

- [ ] Confirm no `.ssc/` exists; run `ssc-plan` adopt path.
- [ ] Confirm the PM's actual goals, not just the prototype's behavior.
- [ ] Decide whether existing features are retro-specified or treated as
      unverified baseline.

**Code**

- [ ] `cargo test` and `npm run build` pass from a clean clone.
- [ ] Read `convert.rs::plan_targets` and argue about the collision logic.
- [ ] Read `decode.rs` and confirm the HEIC-vs-EXIF orientation split.
- [ ] Read `estimate.rs` and decide whether sample-based estimation is good
      enough for the product.
- [ ] Audit the IPC surface: only the listed commands exist; `startup_paths`
      filters arguments correctly.
- [ ] Check the CSP and capability lists against what the app actually uses.
- [ ] Look for dead code, unused settings, and TODO-free but unfinished edges.

**Safety**

- [ ] No path traversal or symlink surprises in output naming.
- [ ] A read-only output directory produces a clean per-file error.
- [ ] Cancel mid-batch leaves no partial files (encoding is in-memory first).
- [ ] The app cannot be tricked into overwriting an input.

**Product**

- [ ] The empty state explains the tool in one read.
- [ ] A first-time user can go from launch to converted file without help.
- [ ] Quality labels are comprehensible to a non-technical user.
- [ ] The estimate's uncertainty is communicated honestly.
- [ ] The privacy claim is true in every code path.

**Compliance**

- [ ] GPL-3.0 notices are present and correct; `heif-rs` and x265 obligations
      are understood.
- [ ] No code or assets with unclear licensing (the app icon is original SVG;
      the README screenshot is generated content).

---

## Known deviations to fold into specs

- Settings live in webview localStorage, not a config file.
- `README.md` links to a GitHub releases URL that does not exist yet.
- The release workflow creates *draft* releases and is untested in this repo.
- The app has no single-instance guard.
- The AppImage is ~105 MB; packaging has not been optimized.
- The UI was only reviewed at two window sizes on one compositor.
- Documentation claims in `README.md` should be re-checked against
  `BEHAVIOR.md` (especially "runs on Windows/macOS", which is a CI build claim,
  not a runtime test).
