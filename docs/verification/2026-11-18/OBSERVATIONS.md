# Baseline verification run — 2026-11-18

This is the dated evidence record for the stage-00 baseline verification pass
(feature 05). Every value below was measured on the machine described in the run
block during this run; nothing is copied from an older document, from the
previous matrix, or from expectation. Where a value could not be measured, the
text says so instead of guessing.

Provenance: the full `scripts/ci.sh all` output was captured to
`/tmp/opencode/ci-all-2026-11-18.log` (outside the repository, by design). Line
timestamps in that log supply the per-stage durations below. The log is scratch
state and is not committed; the numbers it produced are recorded here.

## Run block

| Field | Value |
|---|---|
| Run date | 2026-11-18 |
| Run window (local) | 11:28:10 → 11:30:12 EST |
| Machine | Omarchy 4.5.0, hostname `omarchy` |
| OS / kernel | Linux 7.2.15-1-omarchy x86_64 |
| Session / compositor | Hyprland 0.56.2 (tag `v0.56.2`, commit `efb50993780079460b0cbed1363e2166a2de1d9f`) |
| WebKitGTK | `webkit2gtk-4.1 2.54.1-1` |
| Frontend toolchain (from run output) | vitest 5.1.0, vite 8.3.8 |
| Commit | `972c3d9` — **plus uncommitted feature 01–04 work** (see note) |
| Working-tree state at run time | dirty: `git status --porcelain` had **47** entries (47 immediately before the run, 47 immediately after) |
| Binary | `src-tauri/target/release/heic-image-converter` |
| Binary size | 15,093,664 bytes (14.4 MiB) |
| Binary SHA-256 | `a7168d4b7dd5109e26c39810802ce96f4d331c0ee8ada75a61753613f8035bd3` |
| `npm test` | 2 test files passed, 37 tests passed |
| `cargo test` | 42 passed, 0 failed, 1 ignored |
| `scripts/ci.sh all` | exit code **0**, 122.2 s wall clock |
| Screenshot directory | `docs/verification/2026-11-18/` (seven PNGs, captured later in this feature) |
| Deferred rows | Windows build (stage 05), macOS build (stage 06), `.deb`/`.rpm`/AppImage launch (stage 04), no-network runtime capture (stage 04) |

**Dirty-tree note.** `HEAD` is `972c3d9`, but that commit does **not** contain
the code under test. Features 01–04 of this stage are intentionally uncommitted
at the PM's direction, so the tested tree is `972c3d9` plus 47 uncommitted
entries: modified CI workflow, docs, `package.json`/lockfile, `Cargo.toml`/lock,
`src-tauri/src/{convert,lib,scan}.rs`, `src/App.tsx`, `tsconfig.node.json`, a new
`scripts/manual-check.sh`, and the new test files and fixture corpus
(`backend_integration.rs`, `fixture_corpus.rs`, `src/format.test.ts`,
`src/rows.test.ts`, `src/rows.ts`, `vitest.config.ts`, `src-tauri/tests/fixtures/*`).
The commit alone is not a reproducing reference for this run.

## Build and automated evidence

Command: `scripts/ci.sh all` (runs `frontend`, then `rust`, then `build`; stops at
the first failure).

### Stage timings (single run, derived from captured line timestamps)

| Stage | Command | Duration |
|---|---|---|
| Frontend tests | `npm test` (`vitest run`) | 0.6 s |
| Frontend build | `npm run build` (`tsc && vite build`) | 0.9 s |
| Rust formatting | `cargo fmt --check` | 0.1 s |
| Rust tests | `cargo test` | 0.7 s |
| Tauri release build | `npm run tauri -- build --no-bundle` | 119.8 s |
| **Whole run** | `scripts/ci.sh all` | **122.2 s** |

Caveat on the fast Rust stage: the captured output shows
``Finished `test` profile [unoptimized + debuginfo] target(s) in 0.13s``, so the
test binaries were already compiled from earlier feature-01–04 work; the 0.7 s
figure is test execution, not a cold compile. The release build did compile the
application crate (``Finished `release` profile [optimized] target(s) in 1m 58s``),
but its dependency crates were warm for the same reason. A cold-cache timing on a
fresh clone was not measured in this run.

### Suite summaries, verbatim from the captured output

`npm test`:

```
 Test Files  2 passed (2)
      Tests  37 passed (37)
```

`cargo test` (unit-test binary `src/lib.rs`):

```
running 43 tests
test result: ok. 42 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.47s
```

The other two cargo targets contributed nothing and were green:

```
     Running unittests src/main.rs (target/debug/deps/heic_image_converter-6eea68318399542c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests heic_image_converter_lib
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Per-module breakdown of the 42 passing tests

Counted from the individual `test <name> ... ok` lines in the captured run:

| Module | Passed |
|---|---|
| `backend_integration` | 21 |
| `convert::tests` | 8 |
| `decode::tests` | 1 |
| `estimate::tests` | 2 |
| `fixture_corpus` | 8 |
| `formats::tests` | 2 |
| **Total** | **42** |

Ignored (1): `fixture_corpus::update_fixture_manifest` — rewrites fixture bytes,
`sha256` values, and the README table; run on demand only.

### Build artifact

```
-rwxr-xr-x 2 darticus darticus 15093664 Nov 18 11:30 src-tauri/target/release/heic-image-converter
a7168d4b7dd5109e26c39810802ce96f4d331c0ee8ada75a61753613f8035bd3  src-tauri/target/release/heic-image-converter
```

`file` reports: ELF 64-bit LSB pie executable, x86-64, dynamically linked,
interpreter `/lib64/ld-linux-x86-64.so.2`, for GNU/Linux 4.4.0, stripped.

This is the binary used for every manual step below.

## Manual steps

Screenshots for the steps that require one are written to
`docs/verification/2026-11-18/`; the seven required names are `M2-drag`,
`M3-quality-note`, `M4-settings`, `M5-results`, `M7-errors`, `M9-estimate`, and
`M10-cancel`. These sections are the stable shape for the manual protocol; each
is filled only from what was observed during the run, at the moment it was
observed.

## M1 Picker

Commands:

```bash
scripts/manual-check.sh launch
scripts/manual-check.sh focus
wtype -M ctrl -k o -m ctrl
hyprctl clients -j | jq -r '.[] | "\(.title) :: class=\(.class)"'
```

Observation. A second client appeared alongside the app window:

```
Choose images :: class=heic-image-converter :: initialClass=heic-image-converter
HEIC Image Converter :: class=heic-image-converter :: initialClass=heic-image-converter
```

The dialog is a separate client titled `Choose images`, and it inherits the
application's class `heic-image-converter` rather than a GTK dialog class. No
screenshot: the dialog is a separate window and `shot` captures only the app.

Finding (recorded, not fixed — stage 03 owns the UI): `wtype -k Escape` sent to
the focused app window did **not** dismiss the dialog. The keystroke goes to the
app surface, not to the separate dialog client, so the dialog was still listed
after Escape and went away only when the app was closed with `close`
(pid gone in 112 ms). Closing the app is the reliable exit from the picker.

## M2 Drag-and-drop

The PM dragged `/tmp/heic-manual/ui` from the file manager into the focused app
window. Five rows appeared:

| Row | Name | Type | Dimensions | Size |
|---|---|---|---|---|
| 1 | `alpha.png` | PNG | 900 × 600 | 3.6 KB |
| 2 | `big.png` | PNG | 6000 × 4000 | 35.8 MB |
| 3 | `photo.png` | PNG | 900 × 600 | 896 KB |
| 4 | `portrait.jpg` | JPEG | 1600 × 1200 | 23.7 KB |
| 5 | `rotated.heic` | HEIC | 1200 × 1600 | 2.6 KB |

Counts: header reads `5 images · 36.7 MB`. `photo.png` is the nested file
`ui/nested/photo.png`, listed by basename.

The two rotation-sensitive rows report **post-rotation** dimensions exactly as
the corpus predicts: `portrait.jpg` is stored 1200 × 1600 with EXIF orientation
6 and is listed as 1600 × 1200; `rotated.heic` is stored 1600 × 1200 with an
`irot` 90° CW and is listed as 1200 × 1600. Scan therefore applies orientation
and container rotation when probing dimensions, matching
`backend_integration::scan_reports_post_rotation_dimensions`.

Screenshot: `M2-drag.png`

## M3 PNG note

State before the step: format `JPEG` (the app default), quality `High`, resize
`Keep original size`.

Tab path used (recorded because the row remove buttons are focusable and are
not obvious from the layout): `Tab` 9 times from an unfocused window reaches the
`PNG` button — 1 `Add images`, 2 `Add folder`, 3 `Clear`, 4–8 the five per-row
remove buttons, 9 `PNG`. Activated with `wtype -k space`.

Observation. The quality grid was replaced by a note reading, verbatim:

```
PNG is lossless, so there is no quality to trade away. Pick JPEG or
HEIC when file size matters.
```

Selecting PNG also repainted the format hint to "Lossless and universal.
Biggest files.", repriced every row estimate, and re-ran the estimator
(`9.9 MB`, `73% smaller` at the time). Keys: `Tab` ×9, `space`.

Screenshot: `M3-quality-note.png`

## M4 Settings persistence

With `ui/` still loaded, from the PNG button (Tab 9) three more `Tab` presses
reached the `Resize` select (Tab 12; the quality grid is absent under PNG),
then `wtype -k Down` three times selected `Longest edge 1920 px`
(`Keep original size` → 4096 → 2560 → 1920). Observed state before closing:
format `PNG`, resize `Longest edge 1920 px`, and the `big.png` estimate fell
from `8.3 MB` to `866 KB`.

The app was closed and relaunched with `scripts/manual-check.sh launch
/tmp/heic-manual/ui`. After relaunch, with no further input:

- `PNG` is still the selected format — the lifted panel, with no focus ring, so
  this is the persisted pressed state and not merely focus.
- `Resize` still reads `Longest edge 1920 px`.
- The estimate recomputed to `2.5 MB`, `93% smaller`, `sampled 3 of 5`.

Settings therefore survive a full process restart (webview local storage) and
are applied to a fresh argv-loaded queue. Keys: `Tab` ×3, `Down` ×3, then
`close` and `launch`.

Screenshot: `M4-settings.png`

## M5 Queue conversion

Commands:

```bash
scripts/manual-check.sh launch /tmp/heic-manual/ui
# PNG and Longest edge 1920 px were already persisted from M4
wtype -M ctrl -k Return -m ctrl
scripts/manual-check.sh shot M5-results
```

The summary banner read:

```
5 images converted
4.6 MB written · from 36.7 MB
```

The rail's result card read `4.6 MB`, `5 images written`. Every row showed a
tick and the size it wrote: `alpha.png` 11.6 KB, `big.png` 3.5 MB, `photo.png`
1013 KB, `portrait.jpg` 30.3 KB, `rotated.heic` 32.3 KB.

The wait was a fixed 6 s poll, not a measured completion time; the summary was
already present when the screenshot was taken.

Outputs, measured with `magick identify` from `/tmp/heic-manual/ui`:

| Output | Dimensions | Channels | Bits |
|---|---|---|---|
| `portrait.png` | 1600 × 1200 | srgb | 8 |
| `rotated.png` | 1200 × 1600 | srgb | 8 |
| `alpha (1).png` | 900 × 600 | srgba | 8 |
| `big (1).png` | 1920 × 1280 | srgb | 8 |
| `nested/photo (1).png` | 900 × 600 | srgb | 8 |

Corner samples, using the spec's command
(`magick identify -format '%[pixel:p{4,4}] %[pixel:p{W-5,4}] %[pixel:p{4,H-5}] %[pixel:p{W-5,H-5}]'`),
in TL / TR / BL / BR order:

```
portrait.png  srgb(32,64,224) | srgb(224,31,32) | srgb(240,240,240) | srgb(32,192,32)
rotated.png   srgb(32,65,224) | srgb(224,31,32) | srgb(240,240,240) | srgb(32,192,32)
```

Both decode to **B R / W G**, matching the corpus palette — so EXIF orientation 6
was applied for `portrait.jpg` and the HEIC `irot` 90° CW for `rotated.heic`.
`rotated.png`'s TL blue is 32,65,224 against the 32,64,224 of `portrait.png`,
one unit of lossy HEIC rounding.

`alpha (1).png` kept a real alpha channel: BR is `srgba(0,0,0,0)` (fully
transparent) with TL `srgba(224,32,32,1)`. `big (1).png` at 1920 × 1280 is the
resize cap applied to 6000 × 4000. Collisions produced `alpha (1).png`,
`big (1).png`, and `nested/photo (1).png`, while the extension changes produced
plain `portrait.png` and `rotated.png`.

Screenshot: `M5-results.png`

## M6 Reveal

From the completed run, `Tab` once reached `Show results` (the results banner is
earlier in the DOM than the file list), then `wtype -k space`.

Client classes before activating (`hyprctl clients -j | jq -r '.[].class' |
sort -u`) included `org.gnome.Nautilus` already, holding a window titled `Home`.
After activating, that window was titled **`ui`** — the app revealed the output
folder in the existing file manager rather than launching a new process, so the
class set was unchanged and the distinguishing observation is the title.

File-manager class: `org.gnome.Nautilus`.

Finding (environment, not an app defect): the classic Hyprland dispatch
`hyprctl dispatch closewindow` is rejected on this compositor —
`error: return hl.dispatch(closewindow):1: hl.dispatch: expected a dispatcher
(e.g. hl.dsp.window.close())`. Hyprland 0.56.2 takes the Lua form instead,
`hyprctl dispatch "hl.dsp.window.close()"`, which returned `ok` and closed the
file manager. The harness itself does not need this (it signals the app's PID),
but it is worth recording for anyone driving the window by hand.

The app was then closed with `close` (pid gone in 117 ms).

## M7 Corrupt and permission-denied

Commands:

```bash
scripts/manual-check.sh launch /tmp/heic-manual/edge
wtype -M ctrl -k Return -m ctrl
scripts/manual-check.sh shot M7-errors
```

The persisted `PNG` format applied; resize was still `Longest edge 1920 px`,
which does not touch these 900 × 600 inputs.

Banner: `0 converted, 2 failed` / `0 B written · hover a failed row for the
reason`. Result card: `0 B`, `0 images written`.

Both rows showed `Failed` with distinct messages, verbatim:

```
denied.png   could not open file: Permission denied (os error 13)
garbage.png  could not decode image: Format error decoding Png: Invalid PNG signature.
```

Permission-denied and corrupt are separate failure classes with separate
messages, and one bad file did not stop the other from being attempted — the
batch finished with both rows adjudicated.

Zero-output count: `find /tmp/heic-manual/edge -name '*.png' ! -name
'garbage.png' ! -name 'denied.png' | wc -l` printed `0`. `edge/` still held only
the two sources afterwards. The app then closed (pid gone in 113 ms).

Screenshot: `M7-errors.png`

## M8 HEIC output

Commands:

```bash
scripts/manual-check.sh launch /tmp/heic-manual/heic/source.png
# Tab x7 reaches HEIC (one queue row, so one row button before the formats)
wtype -k space
wtype -M ctrl -k Return -m ctrl
```

Tab path with a single queue row: 1 `Add images`, 2 `Add folder`, 3 `Clear`, 4
the row remove button, 5 `PNG`, 6 `JPEG`, 7 `HEIC`. Selecting HEIC repainted the
hint to "Tiny files. Apple devices and modern browsers." and restored the
quality grid (`High`).

Banner: `1 image converted` / `516 KB written · from 1.7 MB`; result card
`516 KB`, `1 image written`; the row's output size was `516 KB`. On disk,
`/tmp/heic-manual/heic/source.heic` is 528,866 bytes.

`heif-info /tmp/heic-manual/heic/source.heic`:

```
MIME type: image/heic
main brand: heic
compatible brands: mif1, heic, miaf

image: 1200x900 (id=1), primary
  colorspace: YCbCr, 4:2:0
  bit depth: 8
  color profile: no
  alpha channel: no
  depth channel: no
metadata:
  none
transformations:
  none
```

External decode:

```
$ heif-convert /tmp/heic-manual/heic/source.heic /tmp/manual-check.png
File contains 1 image
Written to /tmp/manual-check.png
heif-convert exit=0
```

The decoded PNG measured 1200x900, srgb, 8-bit — the same dimensions as the
1200x900 source, so the round trip through HEIC preserved geometry.

Tool versions:

```
heif-info --version     -> 1.23.6 / libheif: 1.23.6 / plugin path: /usr/lib/libheif
heif-convert --version  -> 1.23.6 / libheif: 1.23.6 / plugin path: /usr/lib/libheif
```

Scope: one external decoder (libheif's own CLI) accepts the file and reproduces
its dimensions. This is not a conformance suite and does not prove third-party
compatibility in general.

## M9 Estimate accuracy

Commands:

```bash
scripts/manual-check.sh launch /tmp/heic-manual/estimate
# Tab x12 reaches JPEG with seven queue rows
wtype -k space
# then Tab x6 reaches the Resize select (four quality buttons re-enter the order)
wtype -k Home
wtype -M ctrl -k Return -m ctrl
```

Tab path with seven rows: 1 `Add images`, 2 `Add folder`, 3 `Clear`, 4–10 the
seven row remove buttons, 11 `PNG`, 12 `JPEG`, 13 `HEIC`, then 14 `Smallest`,
15 `Balanced`, 16 `High`, 17 `Maximum`, 18 the `Resize` select. The quality grid
was already present at launch, because M8 left HEIC — a lossy format — as the
persisted selection; the PNG note is a `<p>` and never occupies a tab stop, so
it is absent here either way. `Home` chose the first option,
`Keep original size`.

Estimate observed after the 450 ms debounce and sampling settled:

```
ESTIMATED RESULT
3.2 MB
for 7 images · sampled 3 of 7   1.1x larger
```

Per-row estimates were `≈ 462 KB` for all seven, summing to 3234 KB.

After `Ctrl+Enter` the seven outputs were written as same-extension collisions,
`photo-01 (1).jpg` … `photo-07 (1).jpg`:

| Output | Bytes |
|---|---|
| `photo-01 (1).jpg` | 473,681 |
| `photo-02 (1).jpg` | 474,636 |
| `photo-03 (1).jpg` | 469,346 |
| `photo-04 (1).jpg` | 475,825 |
| `photo-05 (1).jpg` | 461,850 |
| `photo-06 (1).jpg` | 466,641 |
| `photo-07 (1).jpg` | 471,233 |

Actual total: **3,293,212 bytes = 3.141 MiB**. The UI displayed `3.2 MB`.
Treating the displayed figure as 3.2 × 1024² = 3,355,443 bytes:

```
deviation = |3,355,443 − 3,293,212| / 3,293,212 × 100 = 1.89%  ->  2%
```

2% is far inside the 30% threshold, so this row is a pass and not a finding.

Caveat worth stating: the seven `estimate/` sources are independently generated
plasma images but they share one renderer and similar entropy, so the spread of
actual sizes (461,850–475,825 bytes) is narrow. The result shows the estimator
is accurate on a homogeneous seven-photo batch; it does not measure accuracy on
a heterogeneous real-world mix.

Screenshot: `M9-estimate.png`

## M10 Cancellation

Commands:

```bash
scripts/manual-check.sh launch /tmp/heic-manual/batch-cancel
# Tab x124 reaches PNG: 3 action buttons, 120 row buttons, then PNG/JPEG/HEIC
wtype -k space
wtype -M ctrl -k Return -m ctrl
# poll until at least five outputs exist on disk, then:
wtype -k Escape
```

Tab path with 120 rows: 1 `Add images`, 2 `Add folder`, 3 `Clear`, 4–123 the
120 row remove buttons, 124 `PNG`. The persisted format at launch was JPEG (left
behind by M9), so the quality grid was already present and selecting PNG
replaced it with the lossless note; either way the grid sits after the format
segment, so PNG's index is unchanged.

`Escape` was sent as soon as the output count reached five, to guarantee a
genuinely partial batch rather than a lucky full one. The conversion had
produced 8 files by the time the poll noticed.

Banner:

```
Cancelled after 15 images
The images already written are safe.
```

Result card: `38.5 MB`, `15 images written`. Every remaining row showed
`Skipped`; the three completed rows visible at the captured scroll position
(`shot-107`, `shot-108`, `shot-109`) each read `✓ 2.6 MB`.

Output count: **15** `shot-*.png` files, matching the banner's 15. They are not
contiguous — the completed set was

```
shot-001..004, shot-061..063, shot-091..094, shot-106..109
```

so cancellation leaves a partial, scattered set whose size is reported
correctly but whose membership does not follow queue order. That is a
consequence of the bounded worker pool, worth knowing when checking results by
eye.

Sources unchanged:

```
$ cd /tmp/heic-manual && sha256sum -c sources.sha256
... 254 lines ...
254 x ": OK", zero failures
```

Screenshot: `M10-cancel.png`

## M11 Large batch

Commands:

```bash
scripts/manual-check.sh launch /tmp/heic-manual/batch
# PNG and Keep original size were persisted from M10
wtype -M ctrl -k Return -m ctrl
```

State at launch: `120 images · 157 MB`, PNG selected, `Keep original size`,
estimate `1004 MB`, `6.4x larger`.

Timing around `Ctrl+Enter`:

```
Ctrl+Enter sent  2026-11-18T11:51:38-05:00
120th output     2026-11-18T11:51:40.480-05:00
output mtimes    11:51:38.652 .. 11:51:40.480  (1.83 s of writing)
wall from key    2.05 s
```

Banner: `120 images converted` / `1.0 GB written · from 157 MB`. Result card
`1.0 GB`, `120 images written`. Per-row output sizes were `7.9 MB` for the
3000 × 2000 inputs and `30.9 MB` for the four 6100 × 4100 inputs.

Outputs: **120** files — 116 `img-*.png` plus 4 `big-*.png`, total
1,087,480,368 bytes (1.013 GiB). A sample `img-*.png` is 8,256,418 bytes and a
sample `big-*.png` is 32,433,970 bytes.

Sampled dimensions, `magick identify`:

```
img-001.png  3000x2000
img-058.png  3000x2000
big-001.png  6100x4100
big-004.png  6100x4100
```

All 120 inputs kept their original size — the four 25.01 MP images completed at
6100 × 4100 with no resize, and the 116 at 3000 × 2000. No output was truncated
or skipped: the count is exact.

Sources unchanged:

```
$ cd /tmp/heic-manual && sha256sum -c sources.sha256
254 x ": OK", zero failures
```

Caveat on the duration: `/tmp` is a 16 GiB tmpfs on this machine, so the 1 GiB
of output was written to RAM, and PNG conversion runs on a bounded pool of four
worker threads (`convert.rs` sizes the pool as `(cores/2).clamp(1, 6)`, which is
4 on this 8-core machine). The 2 s figure is a real measurement of this run but
it is not a disk-throughput or cold-cache result, and it should not be quoted as
a performance guarantee for a real SSD or a slower machine. The 200- and
1000-image measurements, and any memory ceiling, remain stage 02's work.
