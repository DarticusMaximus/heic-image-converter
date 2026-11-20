# Architecture

> Status: as-built description of the prototype. This is *review input* for an
> SSC adoption pass, not an SSC artifact. Claims here were checked against the
> code and the notes in `BEHAVIOR.md`; anything not actually exercised is
> marked as such there.

## Snapshot

| | |
|---|---|
| Product name | HEIC Image Converter |
| Version | 0.1.0 (implementation commit `3077fb1`) |
| Bundle identifier | `io.darticus.heicconverter` |
| License | GPL-3.0-or-later (forced by bundled x265) |
| Shell | Tauri v2 (2.12.6) |
| Frontend | React 19.3.1 + TypeScript 6.0.3 + Vite 8.3.8 |
| Backend | Rust 2024 edition |
| Image decode | `image` 0.25.10 (JPEG/PNG/WebP/TIFF/BMP/GIF) and `heif-rs` 26.10.3 (HEIC/HEIF, exact pin, vendored and patched) |
| Codec payload | statically linked libheif 1.23.6 + libde265 + x265, pinned by a committed manifest and checksum-verified before extraction |
| Test count | Whatever `cargo test` (in `src-tauri`) and `npm test` report; this table does not restate a total that would drift |
| Source size | ~10,600 lines total (7,654 Rust in `src-tauri/src`, 547 App, 1,124 CSS) |

## Shape

```
┌──────────────────────────────────────────────────────────────────┐
│ WebView (React 19)                                               │
│  App.tsx            queue, settings, phases, keyboard, drag-drop  │
│  components/        DropZone, FileList, SettingsRail, Summary     │
│  api.ts             typed wrappers over Tauri invoke/listen       │
└───────────────┬──────────────────────────────────────────────────┘
                │ invoke()                         ▲ event
                ▼                                  │ "convert-progress"
┌──────────────────────────────────────────────────┴──────────────┐
│ Tauri commands (src-tauri/src/lib.rs)                            │
│  scan_paths, load_thumbnail, estimate_size, convert_files,       │
│  cancel_conversion, reveal_path, startup_paths                   │
└───┬───────────────┬───────────────┬──────────────────────────────┘
    │               │               │
    ▼               ▼               ▼
 scan.rs         decode.rs       convert.rs          estimate.rs
 walk + probe    HEIC / image    resize + encode     sample encode
 + thumbnails    + EXIF orient   + path planning     + extrapolation
                      │               │
                      ▼               ▼
             heic.rs (decode,     formats.rs (settings,
             dimensions, encode,  quality → heic preset)
             test-only probe)
```

Every HEIC call — decode, dimension probe, encode, the test-only header probe
— goes through `heic.rs`. It is the only module that names the HEIC crate.

Everything runs in one process. There is no sidecar, no helper binary, no
network call at runtime.

## Rust modules

### `src-tauri/src/lib.rs` (151 lines)

Owns the Tauri builder, plugins, and command surface.

- `ConversionState { running: Arc<AtomicBool>, cancel: Arc<AtomicBool> }` is
  managed state. `running` is acquired with `compare_exchange` so a second
  conversion invocation returns an error instead of interleaving.
- Heavy commands (`scan_paths`, `load_thumbnail`, `estimate_size`,
  `convert_files`) are `async` and push blocking work onto
  `tauri::async_runtime::spawn_blocking`.
- `startup_paths()` returns existing, non-flag `std::env::args()` entries. This
  is how `heic-image-converter photo.heic` and "Open with" seed the queue.
- `reveal_path()` uses the opener plugin: a directory is opened, a file is
  revealed in the file manager.
- Plugins: `tauri_plugin_opener`, `tauri_plugin_dialog`.

### `src-tauri/src/formats.rs` (190 lines)

The settings vocabulary and the friendly-to-codec mapping.

| Setting | Values |
|---|---|
| output format | `png`, `jpeg`, `heic` |
| quality | `smallest`, `balanced`, `high`, `maximum` |
| max dimension | `null` (original) or ≥ 16 px, UI offers 4096/2560/1920/1280/1080 |
| output dir | `null` (next to source) or absolute path |
| overwrite | boolean (default false) |

| Quality | JPEG q | HEIC q | x265 preset |
|---|---|---|---|
| Smallest | 60 | 30 | fast |
| Balanced | 78 | 50 | fast |
| High | 88 | 68 | medium |
| Maximum | 95 | 82 | slow |

HEIC is always 4:2:0, 8-bit output, RGB (alpha flattened onto white). The HEIC
preset column comes from `Quality::heic_preset()`, which returns the seam's
`Preset` type. PNG ignores quality entirely (lossless).
`ConvertSettings::sanitized()` clamps `max_dimension < 16` to `None` and trims
empty output paths.

### `src-tauri/src/scan.rs` (216 lines)

Input expansion and list metadata.

- Accepts a mix of files and directories. Directories are walked with
  `walkdir`, symlinks not followed, entries sorted by file name.
- Supported input extensions: `heic/heif/hif`, `jpg/jpeg/jpe/jfif`, `png`,
  `webp`, `tif/tiff`, `bmp`, `gif` (case-insensitive).
- Hidden files/dirs (leading `.`), `Thumbs.db`, and `desktop.ini` are skipped.
  The *root* of a dropped folder is exempt (dropping `~/.photos` works).
- `MAX_FILES = 20_000`; exceeding it stops the scan and sets `truncated`.
  `scan` delegates to `scan_with_limit(paths, max_files)`, so tests can prove
  truncation without creating 20,000 files.
- Each accepted file records its canonical path, size, dimensions, plus the
  dropped folder (`root`) and the path relative to it (`relative`). Paths are
  deduplicated by canonical path.
- Dimensions are probed in parallel with Rayon through `decode::dimensions`.
  For HEIC that calls `heic::dimensions`, which has to read the file because
  the wrapper drains the reader, but skips the pixel decode; other formats read
  headers only.
- `thumbnail(path, max=128)` decodes, downscales (never upscales),
  composites alpha onto white, and returns a `data:image/jpeg;base64,...` at
  quality 74. Downscaling uses the triangle filter.

Skipped files are returned with a reason only for explicitly dropped paths.
Unsupported files discovered *inside* a folder are ignored silently.

### `src-tauri/src/decode.rs` (117 lines)

- HEIC: `heic::decode` → `DynamicImage`, with its size probe served by
  `heic::dimensions`. libheif applies container transformations (`irot`/`imir`),
  which is verified by a committed fixture.
- Everything else: `image::ImageReader::with_guessed_format().decode()`.
- Non-HEIC images get EXIF orientation applied (2–8, including the mirrored
  cases). HEIC intentionally skips EXIF orientation to avoid double rotation.
- `dimensions()` reports post-rotation sizes for both paths so the list matches
  what conversion will produce.
- `flatten_to_rgb()` alpha-composites onto white for JPEG/HEIC output.

### `src-tauri/src/convert.rs` (538 lines)

The pipeline and the only place files are written.

1. Plan all targets first, sequentially, reserving names in a `HashSet`
   (`plan_targets`), so parallel workers can never race for a file name.
2. Run the batch on a Rayon pool: `clamp(cores/6, 1, 3)` workers for HEIC
   (x265 is already multithreaded), `clamp(cores/2, 1, 6)` otherwise.
3. Per file: decode → optional Lanczos3 resize of the longest edge → encode →
   write. Each file is wrapped in `catch_unwind`, so a decoder panic becomes a
   per-file error, not a dead app.
4. Call the injected `progress` callback after each file. `lib.rs` adapts it to
   the `convert-progress` event, so this module has no Tauri dependency.
5. Cancellation is checked between files (an in-flight encode is not
   interrupted; the UI says "Finishing current image…").

Encoders:

- PNG: `image` PNG encoder, always normalized to 8-bit (`to_eight_bit`), alpha
  preserved, grayscale preserved.
- JPEG: `JpegEncoder::new_with_quality`, RGB8 after flattening.
- HEIC: `heic::encode` after alpha is flattened onto white; the seam owns the
  quality/preset mapping, 4:2:0 chroma, and 8-bit output.

### `src-tauri/src/heic.rs` (729 lines)

The single adapter between this product and the HEIC crate. It is the only file
under `src-tauri/src/` allowed to name the crate; `decode.rs`, `scan.rs`,
`convert.rs`, `formats.rs`, and the test-only fixture corpus all call the seam
instead. A test in this module walks the source tree and fails if another
module reaches for `heif::` or imports with `use heif`.

Contract:

- `Preset { Fast, Medium, Slow }` — our vocabulary for the codec's speed/size
  tradeoff. `formats::Quality::heic_preset()` produces it and `encode` maps it
  onto the crate's enum, so no caller names the crate's preset.
- `decode(path)` — decodes the primary image to a `DynamicImage`. Only the
  primary image is decoded; auxiliary images (burst frames, thumbnails, depth
  maps) are ignored. libheif applies container rotation (`irot`/`imir`), so the
  seam never applies EXIF orientation.
- `dimensions(path)` — reads the header only. The wrapper drains the reader, so
  this costs one file read but no pixel decode. `scan.rs` probes every file
  through `decode::dimensions`, which lands here for HEIC.
- `encode(image, quality, preset)` — HEIC bytes at 4:2:0, 8-bit, non-lossless,
  with the quality value and preset the caller passes.
- `#[cfg(test)] probe(path)`, `ProbeInfo`, `BitDepth` — header facts for the
  fixture corpus: dimensions plus the source depth (8, 10, or 12). Test-gated,
  so a release build carries none of it.

Failure taxonomy — `HeicError` and when each variant is raised:

| Variant | Raised when |
|---|---|
| `CorruptContainer` | the container cannot be read or has no primary image handle |
| `Decode` | the container parsed but pixel decoding failed |
| `EncoderUnavailable` | libheif could not initialise an HEVC encoder |
| `Encode` | any other encode failure |
| `Panic { stage }` | a panic escaped the decoder (`"decoder"`) or encoder (`"encoder"`) |
| `Io` | the file could not be read |

`EncoderUnavailable` is decided by matching the crate's rendered
`HeifError::EncoderInit` prefix (`encoder initialization failed`), so the
classification is deliberately coupled to that message. A reworded dependency
silently widens it to `Encode`, which a unit test catches on the bump.

Panic containment lives here too: every entry point runs its work through a
private `guard(stage, f)` built on `catch_unwind`, so an escaped libheif panic
becomes `HeicError::Panic` instead of unwinding through a Rayon worker. The
guard also downcasts the panic payload and emits one line carrying the cause
and its source location, so support can see why a file was rejected; the
returned error still renders as before.
`convert::convert_all` keeps its own `catch_unwind` as the backstop for
non-HEIC decode panics and for writes. `HeicError` never leaves the Rust core:
`decode.rs` renders it with `Display` into the `String` its callers already
return.

### `src-tauri/src/estimate.rs` (268 lines)

- Picks up to `MAX_SAMPLES = 3` images spread across the eligible list
  (indices `0`, midpoint, last), then appends the rest as fallbacks.
- Encodes each sample fully (decode + resize + encode, no write) via
  `convert::encode_bytes`, and skips samples that fail.
- Returns `bytes_per_pixel = Σ output bytes / Σ output pixels`, the total
  output pixel count for the batch after any resize cap, and the sampled
  details. The frontend multiplies by each file's target pixel count.
- This is a real measurement on real content; PNG estimates are still
  content-sensitive and can be off on unusual images.

### `src-tauri/src/backend_integration.rs` (test-only)

Declared as `#[cfg(test)] mod backend_integration;`, so it ships in no build.
It drives the real `scan`, `scan_with_limit`, `thumbnail`, and `convert_all`
paths with no `AppHandle`, window, network, or external CLI. Every write goes
into a `tempfile::tempdir()`; committed fixtures are copied in, never modified.

- 8 scanning tests: recursion with `root`/`relative`, hidden and system entries,
  unsupported files inside folders, explicit skip reasons, canonical dedupe,
  mixed-root grouping, `scan_with_limit` truncation, post-rotation dimensions.
- 6 thumbnail tests: data URL shape, downscale-only, edge clamp, alpha onto
  white, HEIC input, unsupported/corrupt errors.
- 7 batch tests: mixed tree with mirrored output, source protection, collision
  naming, per-file failure isolation, pre-set cancellation, one progress event
  per file, and the `convert-progress` camelCase payload.

Mid-batch cancellation timing is not tested here; only the pre-set flag path is.

## Frontend

### `src/App.tsx` (547 lines)

State: `files`, `settings`, `thumbs`, `phase` (`idle`/`converting`/`done`),
`progress`, `estimate`, `summary`, `notice`, `scanning`, `dragging`.

Notable behavior:

- **Settings** persist to `localStorage["heic-converter.settings.v1"]`; default
  is JPEG / High / original size / next to originals / no overwrite.
- **Adding**: drag-drop via `getCurrentWebview().onDragDropEvent`, native file
  and folder pickers via the dialog plugin, and `startup_paths()` on mount.
  Files are merged with dedupe by path. Adding is blocked while converting.
- **Thumbnails**: a queue with at most 4 in flight. Failures are remembered and
  not retried.
- **Estimating**: debounced 450 ms, keyed on file list + format + quality +
  max dimension (output dir and overwrite do not trigger it), with a sequence
  guard against stale responses. Skipped while converting or in the done state.
- **Converting**: `convertingRef` blocks duplicate invocations (the Rust guard
  is the second line of defense); progress events update rows by path; the
  final outcome reconciles rows and builds the summary.
- **Keyboard**: Ctrl/Cmd+O pick files, Ctrl/Cmd+Enter convert, Escape cancels.
- Changing format/quality/resize/output/overwrite resets rows to pending and
  clears the summary — the previous results no longer match the settings.

### Components

- `DropZone` — empty state, two native pickers, privacy note.
- `FileList` / `FileRow` — 44 px thumbnail, source metadata, per-file estimate
  or result, hover tools (reveal, remove). Rows are memoized.
- `SettingsRail` — format segment, 2×2 quality grid, resize select, output
  path button, overwrite checkbox, estimate/result card, convert/cancel.
- `SummaryBanner` — converted/failed/cancelled summary plus "Show results".
- `Icons` — inline SVGs; no icon dependency.

### Styling

`src/styles.css` (1,124 lines) is a hand-rolled design system: dark by default,
light via `prefers-color-scheme`, mint accent, system font stack, tabular
numerals for sizes, reduced-motion support. No CSS framework.

## IPC contract

All payloads use `camelCase` on the wire.

```ts
scanPaths(paths: string[]) -> {
  files: SourceFile[],
  skipped: { path: string; reason: string }[],
  truncated: boolean
}

// SourceFile
{ id, path, name, kind, bytes, width|null, height|null, root|null, relative|null }

loadThumbnail(path: string, size?: number) -> string   // data URL
estimateSize(items: {path,width,height}[], settings) -> {
  bytesPerPixel: number, estimatedTotalBytes: number, totalPixels: number,
  sampled: number, samples: {name,inputBytes,outputBytes,width,height}[]
}
convertFiles(items: {path,root,relative}[], settings) -> {
  results: FileOutcome[], cancelled: boolean
}
// FileOutcome
{ path, outputPath|null, outputBytes|null, inputBytes, width|null,
  height|null, error|null, cancelled }

cancelConversion() -> void
revealPath(path: string) -> void
startupPaths() -> string[]
```

Event: `convert-progress` with
`{ completed, total, path, outcome: FileOutcome }`, emitted once per file.

## Output path rules

Let `out = settings.output_dir`, `root` = dropped folder, `rel` = path under it.

- `out == null` → same directory as the source.
- `out != null` and `root == null` (single file) → `out/<name>.<ext>`.
- `out != null` and `root != null` → `out/<root basename>/<rel dirs>/<name>.<ext>`.
  Folder structure is mirrored per dropped root.
- Collisions append ` (1)`, ` (2)`, …; the source file itself is never
  overwritten, even with `overwrite` on (HEIC→HEIC in place renames instead).
- With `overwrite` on, an existing target that is not the source is replaced.

## Guards and failure handling

| Failure | Behavior |
|---|---|
| Unreadable/corrupt image | Per-file `error`, batch continues |
| Decoder panic | Caught per file, reported as an internal error |
| Output write failure | Per-file error; no partial file (encode is in-memory first) |
| Invalid output dir | `convert_files` fails before doing work |
| Second conversion invocation | Rejected by the Rust `running` flag |
| Cancel | Remaining files marked `cancelled`; written files are kept |
| Scan over 20k files | Truncated flag + toast |

## Privacy and security posture

- No network calls at runtime. CSP:
  `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline';
  script-src 'self'; connect-src 'self' ipc: http://ipc.localhost ws://localhost:1420 http://localhost:1420`
  (the localhost entries exist for Vite HMR in development).
- Capabilities: `core:default`, `dialog:default`, `opener:default`.
- Metadata (EXIF, GPS, ICC) is **not** copied to output. This is a privacy
  feature and a behavioral limitation.
- The only filesystem writes are output images and created output directories.

## Build and release

See `BUILD.md`. Summary: `npm run tauri build` produces a `.deb`, `.rpm`, and
AppImage on Linux; `.github/workflows/release.yml` builds Linux, Windows (MSVC
and arm64 via runner matrix), and macOS (x64 + arm64) on tag push with
`tauri-action`. `ci.yml` runs four independent jobs on `ubuntu-24.04` —
`frontend` (unit tests and production build), `rust` (`./scripts/ci.sh trust`,
then `cargo fmt --check` and `cargo test`), `build` (`./scripts/ci.sh trust`
before `npm ci`, then the release build with `--no-bundle`), and `offline` (the
same Rust gate from a `git clone` inside a network namespace with Cargo
offline) — each delegating to `./scripts/ci.sh <gate>`. The `offline` job is
guarded by `if: github.event_name != 'pull_request'`: it runs on pushes to
`main` and manual dispatch, never on a pull request, because of its cold-build
cost and the relaxed user-namespace sysctl it needs. The npm and Tauri release
builds are outside the offline claim.

## Places to change things

| Goal | Touch |
|---|---|
| Add an output format (WebP/AVIF) | `formats.rs` enum + `convert.rs::encode`, UI `FORMAT_INFO` |
| Change quality mapping | `formats.rs` only |
| Change default output/quality | `format.ts::DEFAULT_SETTINGS` |
| Alter naming/mirroring | `convert.rs::{base_target,plan_targets}` |
| Add metadata preservation | `decode.rs` (read) + a new EXIF writer in `convert.rs` |
| Change concurrency | `convert.rs::convert_all` |
| Change estimate sampling | `estimate.rs` |
| Add a command | `lib.rs` handler + `api.ts` + `types.ts` |
| Change window/CSP/permissions | `tauri.conf.json`, `capabilities/default.json` |
