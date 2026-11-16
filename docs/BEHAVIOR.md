# Behavior and verification

> What the prototype actually does, and what has actually been checked. This
> separates "works" from "was never tried" — the second list is the useful one
> for an SSC review.

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

Legend: **Auto** = covered by `cargo test`; **Manual** = exercised through the
built UI on this machine (Omarchy / Hyprland / WebKitGTK 2.54); **Not run** =
implemented but never exercised.

| Claim | Status | Evidence |
|---|---|---|
| PNG → HEIC → PNG/JPEG round trip, dimensions intact | Auto | `convert::tests::converts_png_to_heic_and_back` |
| HEIC output is standard-compliant | Manual | `heif-convert` (libheif CLI) decoded our HEIC to PNG at correct dimensions |
| HEIC container rotation (`irot`) applied | Auto | `decode::tests::applies_heic_container_rotation` with `tests/fixtures/rotated.heic` |
| EXIF JPEG orientation applied | Manual only | 1200×1600 JPEG tagged orientation 6 produced a 1600×1200 output |
| 12-bit HEIC input does not produce 16-bit PNG | Auto + Manual | `convert::tests::png_output_is_always_eight_bit`; a 12-bit HEIC converted to an 8-bit PNG (~8 MB instead of ~33 MB) |
| Resize caps the longest edge | Auto | `convert::tests::resize_caps_the_longest_edge` |
| Folder structure mirrored on output | Auto + Manual | `convert::tests::mirrors_dropped_folder_structure`; nested sample converted to `out/trips/kyoto/…` |
| No collision with an existing name | Auto | `single_file_goes_straight_into_output_dir`, `duplicate_names_get_distinct_targets` |
| Source never overwritten in place | Auto | `same_name_and_format_never_overwrites_the_source` |
| HEIC batch uses fewer workers | Auto (by inspection) | `convert.rs::convert_all` formula |
| Size estimate math | Auto | `estimate::tests::{target_pixels_respects_the_cap, samples_are_spread_across_a_long_list}` |
| Estimate accuracy on real photos | Manual, informal | 7-file demo: estimated 398 KB, actual 398 KB total |
| Format selection updates quality UI (PNG note) | Manual | exercised via keyboard in the built app |
| HEIC output through the UI | Manual | 6 HEIC files produced and decoded externally |
| Native file picker opens | Manual | GTK "Choose images" dialog opened |
| Reveal in file manager | Manual | Nautilus launched via "Show results" |
| Cancellation | **Not run** | requires a long batch and user timing |
| Drag-and-drop | **Not run** | only picker and startup-argument paths were exercised |
| Resize control through UI | **Not run** | only the Rust core is tested |
| Settings persistence | Observed, not systematic | format/quality survived app restarts |
| Corrupt / permission-denied inputs | **Not run** | error paths are implemented, not tested |
| Large batches (>100 files, >24 MP) | **Not run** | largest exercised batch: 7 files, ≤ 6 MP |
| Windows build | **Not run** | CI workflow exists; `heif-rs` publishes MSVC binaries |
| macOS build | **Not run** | CI workflow exists |
| `.deb` / `.rpm` / AppImage launched | **Not run** | all three bundles built; the unbundled release binary was run |

## Automated tests

```
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
src-tauri/src/formats.rs
  detects_input_kinds
  quality_steps_increase
```

Run with:

```bash
cd src-tauri && cargo test
```

Frontend: no tests. `npm run build` runs `tsc` + Vite, which is the only
automated check.

## How the manual verification was done

1. Built with `npm run tauri build` and launched
   `src-tauri/target/release/heic-image-converter <folder>` — startup arguments
   seed the queue, so no mouse automation was needed.
2. Used `wtype` for Ctrl+Enter and Tab/Enter navigation; `grim` for screenshots.
3. Generated fixtures with ImageMagick (`magick`), set EXIF orientation with
   `exiv2`, and produced rotated HEICs with `heif-enc --rotate-cw 90`.
4. Cross-checked HEIC output with `heif-info` and `heif-convert`.

Reproduce a representative batch:

```bash
mkdir -p /tmp/check/sub
magick -size 1600x1200 plasma:fractal -quality 90 /tmp/check/photo.jpg
magick -size 900x600 plasma:fractal /tmp/check/alpha.png
magick -size 3000x2000 plasma:fractal /tmp/check/big.png
magick -size 800x600 gradient:'#123d2d'-#8ff2bd /tmp/check/sub/nested.jpg
magick -size 1400x1400 radial-gradient:'#f9d976'-'#101820' /tmp/check/hdr.heic

# EXIF orientation case
exiv2 -M "set Exif.Image.Orientation 6" /tmp/check/photo.jpg

# rotated HEIC case
heif-enc -q 80 --rotate-cw 90 -o /tmp/check/rotated.heic /tmp/check/big.png
```

Then run the app on `/tmp/check` and confirm: portrait JPEG comes out
landscape, rotated HEIC comes out upright, nested output appears under
`<out>/sub/`, and nothing overwrites an original.
