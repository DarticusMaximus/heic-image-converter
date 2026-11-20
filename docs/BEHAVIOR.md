# Behavior and verification

> What the prototype actually does, and what has actually been checked. This
> separates "works" from "was never tried" — the second list is the useful one
> for an SSC review.

## Last verification run

| Field | Value |
|---|---|
| Run date | 2026-11-18 |
| Machine / session | Omarchy 4.5.0 on Linux 7.2.15-1-omarchy x86_64; Hyprland 0.56.2; WebKitGTK `webkit2gtk-4.1 2.54.1-1` |
| Commit | `972c3d9` **plus 47 uncommitted entries** at run time; committed afterwards as `469b77d` (see the dirty-tree note below) |
| Binary | `src-tauri/target/release/heic-image-converter`, 15,093,664 bytes |
| Binary SHA-256 | `a7168d4b7dd5109e26c39810802ce96f4d331c0ee8ada75a61753613f8035bd3` |
| `npm test` | 2 test files passed, 37 tests passed |
| `cargo test` | 42 passed, 0 failed, 1 ignored |
| `scripts/ci.sh all` | exit code 0, 122.2 s wall clock |
| Hosted CI | [run 44364382930](https://github.com/DarticusMaximus/heic-image-converter/actions/runs/44364382930) at `469b77d` — Frontend 12 s, Release build 6 m 16 s, Rust 22 m 32 s cold; all three jobs green |
| Screenshot directory | `docs/verification/2026-11-18/` — seven app-window PNGs: `M2-drag`, `M3-quality-note`, `M4-settings`, `M5-results`, `M7-errors`, `M9-estimate`, `M10-cancel` |
| Deferred rows | Windows build (stage 05), macOS build (stage 06), `.deb`/`.rpm`/AppImage launch (stage 04), no-network runtime capture (stage 04) |

**Dirty-tree note.** The run tested `HEAD` `972c3d9` plus 47 uncommitted entries:
that commit did **not** contain the code under test, because features 01–04 of
stage 00 were left uncommitted at the PM's direction. The tested tree was
committed afterwards as `469b77d` ("Add the stage 00 test baseline and
verification pass"), so that commit — not `972c3d9` — is the reproducing
reference. The 47 entries were: the modified CI workflow, docs, `package.json`
and its lockfile, `Cargo.toml` and `Cargo.lock`,
`src-tauri/src/{convert,lib,scan}.rs`, `src/App.tsx`, `tsconfig.node.json`, the
new `scripts/manual-check.sh`, and the new test and fixture files
(`backend_integration.rs`, `fixture_corpus.rs`, `src/format.test.ts`,
`src/rows.test.ts`, `src/rows.ts`, `vitest.config.ts`,
`src-tauri/tests/fixtures/*`). The commit alone is not a reproducing reference
for this run. The dated record for the matrix's manual observations and the
suite summaries is `docs/verification/2026-11-18/OBSERVATIONS.md`.

## Core flows

### 1. Build a queue

- Empty window: full-window drop zone with "Choose images" and "Choose folder".
- Add by dragging files or folders anywhere in the window (native Tauri
  drag-drop), by the native pickers, or from command-line arguments.
- Anything supported can be mixed: HEIC, HEIF, JPEG, PNG, WebP, TIFF, BMP, GIF.
- Dropping a folder walks it recursively. Dropping several folders keeps each
  one's structure.
- Duplicate paths (same canonical file) are only added once.
- Unsupported files inside folders are ignored without noise. An explicitly
  dropped unsupported file produces "Nothing to add: not a supported image
  format."
- Files with readable headers show name, type, dimensions, size. A file whose
  header cannot be parsed is still listed (dimensions show "unknown size") and
  fails at conversion time with a readable error.
- Scan is capped at 20,000 images; the UI says so if it truncates.
- Adding is disabled while a conversion runs.

### 2. Choose output settings

- **Format**: PNG, JPEG, HEIC. A one-line hint explains the tradeoff.
- **Quality**: Smallest / Balanced / High / Maximum. PNG replaces the grid with
  a note that PNG is lossless and quality does not apply.
- **Resize**: keep original, or cap the longest edge at 4096/2560/1920/1280/1080.
  Downscale only; smaller images are left alone.
- **Save to**: "Next to the originals" (default) or a chosen folder. When a
  folder is chosen, dropped-folder structure is mirrored under
  `<output>/<dropped folder name>/<relative path>`.
- **Overwrite**: off by default. Off means collisions become `name (1).ext`.
  On means an existing target is replaced — except the source itself, which is
  never overwritten.
- Settings persist across launches in webview local storage.
- Changing a setting resets per-file results, because old results no longer
  correspond to the current settings.

### 3. Size estimate

- On any change of files/format/quality/resize, after a 450 ms pause, the app
  encodes up to 3 of the user's own images at the chosen settings and derives
  bytes-per-pixel.
- Each pending row shows `≈ <size>`; the rail shows the batch total, how many
  images were sampled, and a "N% smaller" / "N× larger" comparison against the
  input total.
- While sampling, the card shows a shimmer and "Sampling a few images…".
- Estimate is skipped while converting and while showing results.
- Accuracy: good on typical photo mixes; PNG estimates are content-sensitive
  and can be off on unusual images. It is explicitly labelled `≈`.

### 4. Convert

- Button shows the count ("Convert 7 images"); Ctrl/Cmd+Enter does the same.
- Top progress bar plus rail progress (`completed / total`) and Cancel.
- Per row: spinner "Converting", then a check and the exact output size, or
  "Failed" plus the error text, or "Skipped" after cancellation.
- Done rows get a reveal button ("Show in folder").
- On completion, a banner reports converted/failed counts, output bytes and
  input bytes, with "Show results" and dismiss.
- "Result" in the rail shows the measured total, not the estimate.
- Cancelling keeps already-written files and marks the rest cancelled. The
  in-flight image finishes first; the UI says so.
- A second conversion cannot start while one is running (two layers: frontend
  ref + Rust atomic flag).

### 5. Window behavior

- 1100×780 default, 860×620 minimum, centered, native decorations.
- Empty state collapses the settings rail; populated state is a two-column
  layout (scrollable list, fixed-width rail with a pinned action area).
- Respects system dark/light preference.
- Escape cancels an in-progress conversion.

## Supported conversions

| Input | PNG | JPEG | HEIC |
|---|---|---|---|
| HEIC / HEIF / HIF | yes | yes | yes (re-encode) |
| JPEG / JFIF | yes | yes | yes |
| PNG | yes | yes | yes |
| WebP | yes | yes | yes |
| TIFF | yes | yes | yes |
| BMP | yes | yes | yes |
| GIF (first frame) | yes | yes | yes |

Output details:

- PNG: lossless, 8-bit, alpha preserved, grayscale stays grayscale.
- JPEG: 8-bit RGB, alpha composited onto white.
- HEIC: 8-bit, 4:2:0, alpha composited onto white, x265.

## Explicit non-goals / current limitations

- **Metadata is dropped.** No EXIF, GPS, ICC, or XMP is copied to output.
- **One image per HEIC file.** Multi-image HEIC (bursts, depth maps) yields the
  primary image. Depth/auxiliary images are ignored.
- **Animated GIF → first frame.**
- No WebP or AVIF output.
- No presets/profiles, no resumable queue, no job history.
- No auto-update, code signing, or notarization.
- No localization; English only.
- No accessibility audit (aria-labels and focus-visible styles exist, but
  nothing has been tested with a screen reader).
- No installer metadata beyond Tauri defaults (no homepage, no screenshots in
  the bundle).
- No drag-and-drop *out* of the app.

## Verification matrix

Legend:

- Auto — a named test in `cargo test` or `npm test` exercises the claim.
- Auto + Manual — a named test covers the core path and a dated manual step
  confirms it through the built UI.
- Manual — exercised through the built UI in the 2026-11-18 run; the step is
  named in Evidence and recorded in `docs/verification/2026-11-18/OBSERVATIONS.md`.
- By inspection — read from the code; the code path and the reason no test
  covers it are stated.
- Deferred — not verified yet; the owning stage and the reason are stated.

| Claim | Status | Evidence |
|---|---|---|
| PNG → HEIC → PNG/JPEG round trip, dimensions intact | Auto | `convert::tests::converts_png_to_heic_and_back` |
| HEIC output is standard-compliant | Auto + Manual | `convert::tests::converts_png_to_heic_and_back`; M8 external decode (one libheif CLI accepts the file — not a conformance suite) |
| HEIC container rotation (`irot`) applied | Auto + Manual | `decode::tests::applies_heic_container_rotation`; M5 `rotated.png` 1200×1600 corners B R / W G |
| EXIF JPEG orientation applied | Auto + Manual | `fixture_corpus::decoded_fixtures_match_recorded_facts`, `backend_integration::scan_reports_post_rotation_dimensions`; M5 `portrait.png` 1600×1200 corners B R / W G |
| 12-bit HEIC input does not produce 16-bit PNG | Auto | `fixture_corpus::conversion_outputs_match_recorded_facts`, `convert::tests::png_output_is_always_eight_bit` |
| Resize caps the longest edge | Auto + Manual | `convert::tests::resize_caps_the_longest_edge`; M5 `big (1).png` 1920×1280 |
| Folder structure mirrored on output | Auto | `backend_integration::batch_converts_a_mixed_tree_and_mirrors_structure` |
| No collision with an existing name | Auto + Manual | `backend_integration::batch_avoids_collisions_between_duplicate_names`, `convert::tests::duplicate_names_get_distinct_targets`; M5 `alpha (1).png` |
| Source never overwritten in place | Auto + Manual | `backend_integration::batch_never_overwrites_the_source`, `convert::tests::same_name_and_format_never_overwrites_the_source`; M10/M11 `sha256sum -c` |
| HEIC batch uses fewer workers | By inspection | pool-sizing formula in `convert.rs::convert_all` (`(cores / 6).clamp(1, 3)` for HEIC against `(cores / 2).clamp(1, 6)` otherwise); no runtime test — stage 02 measures |
| Size estimate math | Auto | `estimate::tests::target_pixels_respects_the_cap`, `estimate::tests::samples_are_spread_across_a_long_list`; `src/format.test.ts` `targetPixels` cases |
| Estimate accuracy on real photos | Manual | M9 estimated vs actual totals and deviation |
| Format selection updates quality UI (PNG note) | Manual | M3 note text and screenshot |
| HEIC output through the UI | Manual | M8 output validated externally |
| Native file picker opens | Manual | M1 dialog title and class |
| Reveal in file manager | Manual | M6 file-manager class |
| Cancellation | Auto + Manual | `backend_integration::batch_can_be_cancelled_before_it_starts`; M10 counts |
| Drag-and-drop | Manual | M2 PM drag, five rows |
| Resize control through UI | Manual | M5 `big (1).png` |
| Settings persistence | Manual | M4 relaunch; M5 uses the persisted settings |
| Corrupt / permission-denied inputs | Auto + Manual | `fixture_corpus::corrupt_fixtures_fail_to_decode`, `backend_integration::batch_isolates_per_file_failures`; M7 verbatim messages |
| Large batches (>100 files, >24 MP) | Manual | M11 duration and counts; 200/1000 measurement deferred to stage 02 |
| Windows build | Deferred | stage 05; the workflow exists but no Windows build has been produced or run — the platform claim was corrected in the README during this run |
| macOS build | Deferred | stage 06; the workflow exists but no macOS build has been produced or run — the platform claim was corrected in the README during this run |
| `.deb` / `.rpm` / AppImage launched | Deferred | stage 04; bundles have been built but never launched — every manual step in this run used the unbundled release binary |
| No network at runtime | Deferred | code inspection finds no runtime HTTP client in the application code or its normal dependency graph; `ureq` is only a build-dependency of `heif-rs` (it fetches codec binaries at build time); no runtime capture has been performed — CSP tightening and traffic capture are stage 04 |

## Automated tests

Frontend (`npm test`, Vitest under jsdom):

```
src/format.test.ts
src/rows.test.ts
→ 2 test files passed, 37 tests passed
```

Rust (`cd src-tauri && cargo test`):

```
running 43 tests
test result: ok. 42 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

Per-module breakdown of the 42 passing tests, counted from the individual
`test <name> ... ok` lines in this run:

| Module | Passed |
|---|---|
| `backend_integration` | 21 |
| `convert::tests` | 8 |
| `decode::tests` | 1 |
| `estimate::tests` | 2 |
| `fixture_corpus` | 8 |
| `formats::tests` | 2 |
| **Total** | **42** |

The tests, by module:

```
src-tauri/src/backend_integration.rs
  scan_walks_recursively_with_roots_and_relatives
  scan_skips_hidden_and_system_entries
  scan_ignores_unsupported_files_inside_folders
  scan_reports_explicitly_dropped_files_with_reasons
  scan_deduplicates_canonical_paths
  scan_keeps_mixed_roots_separate_and_ordered
  scan_truncates_at_the_limit
  scan_reports_post_rotation_dimensions
  thumbnail_returns_a_jpeg_data_url
  thumbnail_never_upscales
  thumbnail_clamps_requested_edge
  thumbnail_composites_alpha_onto_white
  thumbnail_handles_heic_input
  thumbnail_rejects_unsupported_and_corrupt_inputs
  batch_converts_a_mixed_tree_and_mirrors_structure
  batch_never_overwrites_the_source
  batch_avoids_collisions_between_duplicate_names
  batch_isolates_per_file_failures
  batch_can_be_cancelled_before_it_starts
  batch_emits_one_progress_event_per_file
  progress_event_serializes_with_the_ipc_field_names
src-tauri/src/convert.rs
  mirrors_dropped_folder_structure
  duplicate_names_get_distinct_targets
  single_file_goes_straight_into_output_dir
  same_name_and_format_never_overwrites_the_source
  clamps_absurd_max_dimension
  converts_png_to_heic_and_back
  png_output_is_always_eight_bit
  resize_caps_the_longest_edge
src-tauri/src/decode.rs
  applies_heic_container_rotation
src-tauri/src/estimate.rs
  target_pixels_respects_the_cap
  samples_are_spread_across_a_long_list
src-tauri/src/fixture_corpus.rs
  manifest_covers_every_fixture_file
  fixtures_match_recorded_bytes_and_hash
  decoded_fixtures_match_recorded_facts
  conversion_outputs_match_recorded_facts
  corrupt_fixtures_fail_to_decode
  corpus_covers_every_format_and_awkward_case
  animated_gif_has_multiple_distinct_frames
  readme_table_is_current
src-tauri/src/formats.rs
  detects_input_kinds
  quality_steps_increase
```

Ignored by default (1): `fixture_corpus::update_fixture_manifest` — rewrites the
`bytes`/`sha256` fields in `src-tauri/tests/fixtures/manifest.json` and the table
block in `src-tauri/tests/fixtures/README.md` after a deliberate fixture change;
it never rewrites authored expectations. Run it on demand with
`cargo test -- --ignored update_fixture_manifest`.

The backend integration suite (`backend_integration.rs`) is test-only and
drives the real `scan`, `scan_with_limit`, `thumbnail`, and `convert_all` paths
with no Tauri `AppHandle`, window, network, or external tool; every write goes
into a `tempfile::tempdir()`, and committed fixtures are copied rather than
modified.

The corpus integrity suite (`fixture_corpus.rs`) is test-only and reads
`src-tauri/tests/fixtures/manifest.json`; it touches no external tool and no
network. `src-tauri/tests/fixtures/README.md` documents the corpus — 22
synthetic fixtures covering every supported format plus the awkward cases, a
manifest that pins each file's bytes and SHA-256, and the regeneration
workflow.

The frontend suite covers `formatBytes`, `targetPixels`, settings
load/save/validation, and the `toItem`/`applyOutcome` row mapping. React
components, DOM interaction, and the UI state layer are not covered yet.
`npm run build` runs `tsc` + Vite and is the frontend build gate in
`scripts/ci.sh all`.

Run commands:

```bash
npm test                          # Vitest, 2 files / 37 tests
cd src-tauri && cargo test        # 42 passed, 0 failed, 1 ignored
scripts/ci.sh all                 # frontend + rust + release build, exit 0
```

`scripts/ci.sh frontend|rust|build` runs one gate; `all` runs the three in that
order and stops at the first failure. CI delegates to the same script.

## Reproducing the manual run

Prerequisites are the local tools already used for manual verification:
`magick`, `exiv2`, `heif-enc`, `heif-info`, `heif-convert`, `hyprctl`, `jq`,
`grim`, `wtype`. The harness never runs in CI, installs nothing, and needs no
network. It asks for roughly 3 GB free under the scratch parent.

```bash
scripts/manual-check.sh sample            # corpus, sources.sha256, EXPECTED.md
scripts/manual-check.sh launch [path...]  # start, wait for the window, focus
scripts/manual-check.sh shot <name>       # app-window-only PNG into the dated dir
scripts/manual-check.sh close             # terminate the recorded PID
scripts/manual-check.sh cleanup           # restore mode 644, remove the scratch dir
```

The scratch directory defaults to `/tmp/heic-manual` (`MANUAL_DIR`) and must
hold a `.heic-manual` marker before `sample` or `cleanup` will touch it.
Screenshots go to `SHOT_DIR`, defaulting to `docs/verification/<ISO date>/`.

**Keyboard rules.** `launch` focuses the window. Move focus with
`wtype -k Tab` (reverse with `wtype -M shift -k Tab -m shift`); activate
buttons with `wtype -k space`; choose select entries with `wtype -k Home`,
`wtype -k Down`, or a digit key; toggle a checkbox with `wtype -k space`;
Ctrl+Enter is `wtype -M ctrl -k Return -m ctrl`; Escape is `wtype -k Escape`.
Pointer synthesis is not available, so Tab is the supported path. Tab counts
are recorded below because the per-row remove buttons are focusable and sit
between the action buttons and the format buttons.

The steps below are what the 2026-11-18 run actually executed; the observation
for each is in `docs/verification/2026-11-18/OBSERVATIONS.md`.

1. **M1 Picker.** `scripts/manual-check.sh launch` (no arguments);
   `wtype -M ctrl -k o -m ctrl`; record the dialog title and class from
   `hyprctl clients -j | jq -r '.[] | "\(.title) :: class=\(.class)"'`;
   `wtype -k Escape` (see Findings — it did not dismiss the dialog);
   `scripts/manual-check.sh close`. No screenshot: the dialog is a separate
   window and `shot` captures only the app.
2. **M2 Drag-and-drop (requires the PM).** The PM dragged
   `/tmp/heic-manual/ui` from the file manager into the focused window; five
   rows appeared. Record the names and dimensions (1600×1200, 1200×1600,
   900×600, 6000×4000, 900×600); `scripts/manual-check.sh shot M2-drag`.
3. **M3 PNG note.** From an unfocused window, `Tab` 9 times — 1 `Add images`,
   2 `Add folder`, 3 `Clear`, 4–8 the five row remove buttons, 9 `PNG` — then
   `wtype -k space`. Record the exact note text and the repainted format hint;
   `scripts/manual-check.sh shot M3-quality-note`.
4. **M4 Settings persistence.** From `PNG` (Tab 9), `Tab` 3 more times to the
   `Resize` select (Tab 12; the quality grid is absent under PNG), then
   `wtype -k Down` three times (`Keep original size` → 4096 → 2560 → 1920).
   `scripts/manual-check.sh close`; `scripts/manual-check.sh launch
   /tmp/heic-manual/ui`; confirm `PNG` is still pressed and the select shows
   `Longest edge 1920 px`; `scripts/manual-check.sh shot M4-settings`;
   `scripts/manual-check.sh close`.
5. **M5 Queue conversion.** `scripts/manual-check.sh launch
   /tmp/heic-manual/ui` (PNG and 1920 persisted from M4);
   `wtype -M ctrl -k Return -m ctrl`; wait for the summary;
   `scripts/manual-check.sh shot M5-results`. Check each output with
   `magick identify` and the four corners with
   `magick identify -format '%[pixel:p{4,4}] %[pixel:p{W-5,4}] %[pixel:p{4,H-5}] %[pixel:p{W-5,H-5}]'`
   in TL / TR / BL / BR order.
6. **M6 Reveal.** From the completed run, `Tab` once to `Show results` (the
   results banner precedes the file list in the DOM), `wtype -k space`; record
   `hyprctl clients -j | jq -r '.[].class' | sort -u` and the file-manager
   class; close the file manager (`hyprctl dispatch "hl.dsp.window.close()"` on
   Hyprland 0.56.2); `scripts/manual-check.sh close`.
7. **M7 Corrupt and permission-denied.** `scripts/manual-check.sh launch
   /tmp/heic-manual/edge`; `wtype -M ctrl -k Return -m ctrl`; record both
   error strings verbatim and the zero-output count
   (`find /tmp/heic-manual/edge -name '*.png' ! -name 'garbage.png' ! -name 'denied.png' | wc -l`);
   `scripts/manual-check.sh shot M7-errors`; `scripts/manual-check.sh close`.
8. **M8 HEIC output.** `scripts/manual-check.sh launch
   /tmp/heic-manual/heic/source.png`; with one queue row, `Tab` 7 times — 1
   `Add images`, 2 `Add folder`, 3 `Clear`, 4 the row remove button, 5 `PNG`,
   6 `JPEG`, 7 `HEIC` — then `wtype -k space`; `wtype -M ctrl -k Return -m
   ctrl`; record `heif-info` fields, `heif-convert` exit status and decoded
   dimensions, and both tool versions; `scripts/manual-check.sh close`.
9. **M9 Estimate accuracy.** `scripts/manual-check.sh launch
   /tmp/heic-manual/estimate`; with seven queue rows, `Tab` 12 times to `JPEG`
   — 1–3 the action buttons, 4–10 the seven row remove buttons, 11 `PNG`,
   12 `JPEG` — then `wtype -k space`; `Tab` 6 more times to the `Resize` select
   (13 `HEIC`, 14–17 the four quality buttons, 18 `Resize`); `wtype -k Home`
   for `Keep original size`; wait for the 450 ms debounce and the sample;
   `scripts/manual-check.sh shot M9-estimate`; `wtype -M ctrl -k Return -m
   ctrl`; record the seven output lengths and compute
   `deviation = |estimated − actual| / actual × 100`.
10. **M10 Cancellation.** `scripts/manual-check.sh launch
    /tmp/heic-manual/batch-cancel`; with 120 queue rows, `Tab` 124 times to
    `PNG` — 1–3 the action buttons, 4–123 the 120 row remove buttons, 124
    `PNG` — then `wtype -k space`; `wtype -M ctrl -k Return -m ctrl`; poll
    until at least five outputs exist on disk, then `wtype -k Escape`; record
    the banner counts and the output count; `scripts/manual-check.sh shot
    M10-cancel`; `cd "$MANUAL_DIR" && sha256sum -c sources.sha256`;
    `scripts/manual-check.sh close`.
11. **M11 Large batch.** `scripts/manual-check.sh launch /tmp/heic-manual/batch`
    (PNG and `Keep original size` persisted from M10); time `wtype -M ctrl -k
    Return -m ctrl` until the summary; record the duration, that 120 outputs
    exist (`img-*.png` + `big-*.png`), sampled dimensions from `magick
    identify`, and `sha256sum -c` again; `scripts/manual-check.sh close`.

Each run's evidence belongs in `docs/verification/<run-date>/`: the written
observations in `OBSERVATIONS.md` and exactly the seven screenshots above, all
app-window-only. The 2026-11-18 record is `docs/verification/2026-11-18/`.

## Findings from this run

Every item below was observed on 2026-11-18 and is recorded, not fixed. The
owning stage is named for each.

1. **The picker dialog ignores Escape sent to the app.** `wtype -k Escape` did
   not dismiss `Choose images`; `hyprctl clients -j` still listed the dialog
   afterwards, and it disappeared only when the app was closed (pid gone in
   112 ms). The dialog is a separate client whose class is
   `heic-image-converter`, not a GTK dialog class, so the keystroke goes to the
   app surface instead of the dialog. Closing the app is the reliable exit
   today. This also corrects the earlier "GTK dialog" description: the title
   and class are what the run measured. Evidence: OBSERVATIONS.md §M1. Owning
   stage: 03 (staff-facing app, which rebuilds the UI and its input handling).

2. **Cancellation leaves a non-contiguous partial output set.** M10's banner
   reported 15 images written and 15 `shot-*.png` files existed, but the
   completed set was `shot-001..004`, `shot-061..063`, `shot-091..094`,
   `shot-106..109`
   — the membership follows bounded worker scheduling, not queue order. The
   count was accurate; only an expectation of queue-ordered completion is
   wrong. Escape was sent when the poll saw five outputs, and the conversion had
   produced eight by then. Evidence: OBSERVATIONS.md §M10. Owning stage: 02
   (conversion correctness at scale, measured batch behavior).

3. **The 120-file duration is a tmpfs number, not a disk number.** M11 measured
   2.05 s wall from Ctrl+Enter to the 120th output (1.83 s of write mtimes) for
   1.0 GB written to `/tmp`, which is a 16 GiB tmpfs on this machine; PNG ran on
   four workers (`(cores / 2).clamp(1, 6)` on 8 cores) and the caches were
   warm. It is a real measurement of this run, but it is not a cold-cache or
   SSD-throughput result and must not be quoted as one. M5's completion time
   was likewise not measured — it used a fixed 6 s poll. The 200- and
   1,000-image measurements and any memory ceiling remain stage 02's work.
   Evidence: OBSERVATIONS.md §M5, §M11. Owning stage: 02.

4. **The estimate was accurate on a homogeneous batch only.** M9 estimated
   3.2 MB against an actual 3,293,212 bytes (3.141 MiB), a 1.89 % → 2 %
   deviation, far inside the 30 % threshold. The seven plasma sources share one
   renderer and similar entropy (actual sizes 461,850–475,825 bytes), so this
   does not measure accuracy on a heterogeneous real-photo mix. This replaces
   the earlier informal "estimated 398 KB, actual 398 KB" demo. Evidence:
   OBSERVATIONS.md §M9. Owning stage: 02.

5. **The production CSP carries the Vite dev origins.** `src-tauri/tauri.conf.json`
   ships `connect-src 'self' ipc: http://ipc.localhost ws://localhost:1420
   http://localhost:1420`. The two `localhost:1420` entries exist for Vite HMR
   in development, so the README's "only permits its own assets" sentence and
   the framing of those entries as dev-only are both incorrect for the built
   app. Separately, "no network at runtime" is a code-inspection claim: the
   application code and its normal dependency graph contain no HTTP client;
   `ureq` appears only as a build-dependency of `heif-rs` (it fetches codec
   binaries at build time), and `reqwest` appears only in Tauri's cross-target
   graph, not in the Linux build. No runtime capture has been performed.
   Evidence: `src-tauri/tauri.conf.json`, `cargo tree -i ureq`,
   `cargo tree --target all -i reqwest`, and this run's build record. Owning
   stage: 04 (portable Linux delivery owns CSP tightening, the runtime capture,
   and the IT one-pager).

6. **"HEIC output is standard-compliant" is narrower than the words imply.**
   M8 validated one file with one external decoder: `heif-info` read it as
   1200×900, YCbCr 4:2:0, 8-bit, and `heif-convert` 1.23.6 exited 0 and wrote a
   1200×900 PNG. That is compatibility evidence, not a conformance suite, and
   the row now states the scope. The earlier Manual-only wording ("decoded our
   HEIC to PNG at correct dimensions") overstated what had been checked; the
   round-trip test now carries the automated half. Evidence: OBSERVATIONS.md
   §M8. Owning stage: 02 (moved from stage 01 when it was re-scoped to the
   trust work).

7. **"HEIC batch uses fewer workers" was mislabelled as automated.** No test
   exercises the HEIC branch of the pool-sizing formula; the previous matrix
   called it Auto (by inspection), which is self-contradictory. The row is now
   By inspection, with the formula named, and stage 02 owns measuring it.
   Evidence: `src-tauri/src/convert.rs::convert_all`. Owning stage: 02.

8. **Smaller corrected observations.**
   - Reveal reuses the running file manager: `org.gnome.Nautilus` was already
     open before activation; "Show results" retitled its existing window to
     `ui` and launched no new process, so the earlier "Nautilus launched via
     'Show results'" wording is wrong. Evidence: OBSERVATIONS.md §M6. Owning
     stage: 03.
   - HEIC output through the UI was validated on one file this run (M8), not
     the six the previous matrix mentioned. Owning stage: 02 (moved from stage
     01 when it was re-scoped).
   - The matrix now cites only the fixtures and unit tests that exist in the
     executed suite; the previous scan/thumbnail rows are outside this
     26-row set and their tests remain listed under Automated tests.
     Owning stage: 00 (documentation of this pass).
   - `README.md` still says "This app never opens a socket" in "Why this
     exists". That is the same unverified runtime-network claim that was
     removed from the Privacy section, but Spec E scoped this pass to the
     Privacy sentence and forbade touching the rest of the file, so it was
     left in place rather than silently edited. It must be either backed by the
     runtime capture or softened when that work lands. Softened on 2026-11-20
     to "nothing is ever sent anywhere"; the runtime capture still belongs to
     stage 04.
   - Spec E asked the Install section to state that no release has been
     published "(the repository has no remote)". That parenthetical is false:
     `git remote -v` shows `origin` on GitHub. The verifiable part is stated
     instead — no tags, no published releases, and the tag-triggered release
     workflow has never been exercised end to end. The false parenthetical was
     not written. Owning stage: 00 (accuracy of this pass).

9. **Environment note, not a product defect.** On Hyprland 0.56.2,
   `hyprctl dispatch closewindow` is rejected and the Lua form
   `hyprctl dispatch "hl.dsp.window.close()"` works. This affects anyone
   driving the window by hand, not the app or the harness (which signals the
   app's PID). Evidence: OBSERVATIONS.md §M6. Owning stage: 00
   (harness/protocol); no product owner — `scripts/manual-check.sh` and these
   reproducing instructions are stage-00 artifacts, and stage 04 inherits the
   protocol.

10. **The suite and build timings were warm-cache.** The run block's 122.2 s is
    a single warm-cache run: the Rust test binaries were already compiled
    (`Finished test profile ... in 0.13 s`), so the 0.7 s Rust test stage is
    execution time, not a cold compile, and the release dependencies were warm
    for the same reason. No cold-cache timing on a fresh clone was measured;
    stage 00's CI gate (feature 04) runs the same gates on a fresh checkout, but
    this run did not time one. Evidence: OBSERVATIONS.md "Caveat on the fast
    Rust stage". Owning stage: 02 (performance measurement; the 200- and
    1,000-image work and any memory ceiling are stage 02's).
