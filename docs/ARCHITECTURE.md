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
| Image decode | `image` 0.25.10 (JPEG/PNG/WebP/TIFF/BMP/GIF) and `heif-rs` 26.10.3 (HEIC/HEIF) |
| Codec payload | statically linked libheif 1.23.6 + libde265 + x265, downloaded prebuilt at build time |
| Test count | 42 Rust tests (plus 1 ignored manifest updater), 37 frontend tests (Vitest, 2 files) |
| Source size | ~4,000 lines total (543 Rust conversion, 574 App, 1,124 CSS) |

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
                                     │
                             formats.rs (settings,
                             quality → codec params)
```

Everything runs in one process. There is no sidecar, no helper binary, no
network call at runtime.

## Rust modules

### `src-tauri/src/lib.rs` (137 lines)

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

### `src-tauri/src/formats.rs` (182 lines)

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

HEIC is always 4:2:0, 8-bit output, RGB (alpha flattened onto white). PNG
ignores quality entirely (lossless). `ConvertSettings::sanitized()` clamps
`max_dimension < 16` to `None` and trims empty output paths.

### `src-tauri/src/scan.rs` (210 lines)

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
- Dimensions are probed in parallel with Rayon. For HEIC this loads the file
  into memory for `HeifDecoder::new` (the crate drains the reader); for other
  formats it reads headers only.
- `thumbnail(path, max=128)` decodes, downscales (never upscales),
  composites alpha onto white, and returns a `data:image/jpeg;base64,...` at
  quality 74. Downscaling uses the triangle filter.

Skipped files are returned with a reason only for explicitly dropped paths.
Unsupported files discovered *inside* a folder are ignored silently.

### `src-tauri/src/decode.rs` (124 lines)

- HEIC: `heif::HeifDecoder` → `DynamicImage`. libheif applies container
  transformations (`irot`/`imir`), which is verified by a committed fixture.
- Everything else: `image::ImageReader::with_guessed_format().decode()`.
- Non-HEIC images get EXIF orientation applied (2–8, including the mirrored
  cases). HEIC intentionally skips EXIF orientation to avoid double rotation.
- `dimensions()` reports post-rotation sizes for both paths so the list matches
  what conversion will produce.
- `flatten_to_rgb()` alpha-composites onto white for JPEG/HEIC output.

### `src-tauri/src/convert.rs` (543 lines)

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
- HEIC: `heif::HeifEncoder` with the quality/preset mapping above.

### `src-tauri/src/estimate.rs` (165 lines)

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

### `src/App.tsx` (574 lines)

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
`tauri-action`. `ci.yml` runs three independent jobs on `ubuntu-24.04` —
`frontend` (unit tests and production build), `rust` (`cargo fmt --check` and
`cargo test`), and `build` (release build with `--no-bundle`) — each delegating
to `./scripts/ci.sh <gate>`.

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
