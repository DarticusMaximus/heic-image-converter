# HEIC source review: `heif-rs` and the codec stack it links

| | |
| --- | --- |
| Wrapper crate | `heif-rs` 26.10.3, vendored at `src-tauri/vendor/heif-rs/` and pinned exactly (`=26.10.3`) |
| Codec archive release | `26.10.1` of `github.com/vegidio/binaries-heif` |
| Committed archives | `bundle_linux_x64.zip`, `bundle_osx_x64.zip`, `bundle_osx_arm64.zip`, `bundle_windows_x64.zip` |
| Observed codec version | libheif 1.23.6, read from `include/libheif/heif_version.h` inside the archive |
| Reviewed upstream revision | `f8d7a972c59d5fb3f9a29c9669c377c93a08418a` (recorded in `src-tauri/vendor/heif-rs/VENDORED.md`) |
| Review date | 2026-11-19 |

What this review is not: it is not a vulnerability audit, not a review of the
libheif/libde265/x265 C/C++ sources, and not a compliance opinion.

## 1. Method and scope

This is a reading pass over the pinned bytes. It opens every Rust source in the
vendored crate and every call the product makes into it, and it inventories the
committed codec archive without opening the C/C++ objects. The review itself
runs no code under test: it is reading and inventory, not fuzzing or
instrumentation.

**The vendored tree.** Every `.rs` file under `src-tauri/vendor/heif-rs/`, the
fork record `VENDORED.md`, and the seam at `src-tauri/src/heic.rs` were read in
full. The file set and the line counts in §2 come from:

```bash
find src-tauri/vendor/heif-rs -name '*.rs' | sort
wc -l src-tauri/vendor/heif-rs/build.rs src-tauri/vendor/heif-rs/build_support.rs \
  src-tauri/vendor/heif-rs/src/decoder.rs src-tauri/vendor/heif-rs/src/encoder.rs \
  src-tauri/vendor/heif-rs/src/error.rs src-tauri/vendor/heif-rs/src/ffi.rs \
  src-tauri/vendor/heif-rs/src/info.rs src-tauri/vendor/heif-rs/src/lib.rs \
  src-tauri/vendor/heif-rs/src/sys.rs src-tauri/vendor/heif-rs/tests/common/mod.rs \
  src-tauri/vendor/heif-rs/tests/config.rs src-tauri/vendor/heif-rs/tests/decode.rs \
  src-tauri/vendor/heif-rs/tests/encode.rs src-tauri/vendor/heif-rs/tests/errors.rs \
  src-tauri/vendor/heif-rs/tests/roundtrip.rs src-tauri/vendor/heif-rs/VENDORED.md \
  src-tauri/src/heic.rs
```

The first command prints 15 `.rs` paths. With `VENDORED.md` and `heic.rs` that
makes 17 files, and §2 has one row for each.

**The archive inventory.** The codec arrives as an opaque prebuilt archive, so
the review inventories the committed Linux x64 zip instead of reading its
objects:

```bash
unzip -l src-tauri/vendor/heif-bundles/bundle_linux_x64.zip
sha256sum src-tauri/vendor/heif-bundles/bundle_linux_x64.zip
unzip -p src-tauri/vendor/heif-bundles/bundle_linux_x64.zip \
  include/libheif/heif_version.h | grep -n 'LIBHEIF_VERSION'
unzip -p src-tauri/vendor/heif-bundles/bundle_linux_x64.zip \
  lib/libheif.a | strings | grep -E '^1\.23\.[0-9]+$' | sort -u
```

Observed: 32 members — a header tree under `include/libheif/`, one static
library `lib/libheif.a` of 12,953,188 bytes, and no license or notice file. The
archive's SHA-256 is
`b5cbed5d08fc79f64b5171cd401fbd62da43333d26b5ad9905354704cf072f82`, equal to
the `bundle_linux_x64.zip` row in
`src-tauri/vendor/heif-bundles/manifest.json` (3,325,156 bytes). The version
header says `1.23.6` and the library carries the same bare string.

**Out of scope.** No vulnerability audit and no C/C++ codec source review happen
here. Reviewing libheif/libde265/x265 C/C++ sources contradicts the stage's
out-of-scope line; the review records the exact versions and their upstream
release dates, and hands the CVE re-check to feature 05's upgrade runbook.
Packaging stays out: this review sets content, never the shipping mechanism; the
notice is wired into a bundle by stage 04. Only `bundle_linux_x64.zip` was byte-
inspected. The other seven archives are hash-pinned in the manifest but not
inspected here — stages 05 and 06 own that.

## 2. Read log

One row per file read. Line counts are `wc -l` output for each path.

| Path | Lines | Purpose | Verdict |
| --- | --- | --- | --- |
| `src-tauri/vendor/heif-rs/build.rs` | 280 | Our replacement build script: manifest-driven archive resolution, SHA-256 before extraction, extracted-library hash, link directives, bindgen. | Reviewed — build-time only; the delta is recorded in `VENDORED.md`. |
| `src-tauri/vendor/heif-rs/build_support.rs` | 277 | Pure helpers shared by `build.rs` and the `codec_artifacts` tests: manifest parse, target select, URL render, hex SHA-256, checksum verify, local-vs-fetch policy. | Reviewed — build-time only, no side effects by construction. |
| `src-tauri/vendor/heif-rs/src/decoder.rs` | 268 | `HeifDecoder`: eager container parse, header facts, chroma and bit-depth selection, plane copy, manual `Drop`. | Reviewed — the product decode path. |
| `src-tauri/vendor/heif-rs/src/encoder.rs` | 551 | `HeifEncoder`: preset/chroma/config types, supported layouts, plane fill, libheif encode pipeline, writer callback. | Reviewed — the product encode path. |
| `src-tauri/vendor/heif-rs/src/error.rs` | 25 | `HeifError` variants and their `Display` strings, including the encoder-init prefix the seam matches. | Reviewed — read because the seam keys one classifier on it. |
| `src-tauri/vendor/heif-rs/src/ffi.rs` | 38 | `Once`-guarded `heif_init` and the `heif_error` → `Result` conversion. | Reviewed — on both product paths. |
| `src-tauri/vendor/heif-rs/src/info.rs` | 23 | `ImageInfo` and the `BitDepth` enum shared with the encoder and probe. | Reviewed — on the probe and encode paths. |
| `src-tauri/vendor/heif-rs/src/lib.rs` | 124 | Crate facade: module exports, byte-slice `encode`/`decode`/`probe`, `libheif_version`. | Reviewed — the product uses the exported types, not the byte-slice facade. |
| `src-tauri/vendor/heif-rs/src/sys.rs` | 5 | `include!` of the bindgen output plus lint allowances. | Reviewed — generated at build time; five lines. |
| `src-tauri/vendor/heif-rs/tests/common/mod.rs` | 28 | Shared asset paths and the `is_heif` brand check for the crate's integration tests. | Reviewed — test-only; not linked into the product. |
| `src-tauri/vendor/heif-rs/tests/config.rs` | 70 | Custom quality/preset/chroma, RGBA, and lossless encode tests. | Reviewed — test-only. |
| `src-tauri/vendor/heif-rs/tests/decode.rs` | 41 | Probe, bundled-asset decode, and explicit-thread decode tests. | Reviewed — test-only. |
| `src-tauri/vendor/heif-rs/tests/encode.rs` | 23 | Facade `encode` and `encode_buffer` tests. | Reviewed — test-only. |
| `src-tauri/vendor/heif-rs/tests/errors.rs` | 64 | Error-path tests: buffer length, zero dimensions, unsupported color, empty/garbage/truncated input. | Reviewed — test-only. |
| `src-tauri/vendor/heif-rs/tests/roundtrip.rs` | 45 | Encode→decode dimension round-trips for RGB, RGBA, grayscale, and 16-bit input. | Reviewed — test-only. |
| `src-tauri/vendor/heif-rs/VENDORED.md` | 120 | Fork record: upstream version and commit, the two edits and two additions, the archive release, the upgrade steps. | Reviewed — the delta record §3 cites. |
| `src-tauri/src/heic.rs` | 518 | The seam: error vocabulary, classifiers, panic guard, decode/dimensions/encode/probe, and the seam tests. | Reviewed — the only product consumer of the crate. |

## 3. Diff from the published crate

The vendored tree is the published `heif-rs-26.10.3` crate with exactly four
paths changed: two edited files and two added files. `VENDORED.md` records the
whole delta under "What differs from upstream"; each row below names the
subsection that covers it.

| Path | Change | What the change does |
| --- | --- | --- |
| `src-tauri/vendor/heif-rs/Cargo.toml` | Edited (`Edited: Cargo.toml`) | Adds `serde_json` and `sha2` to `[build-dependencies]` so the build script can parse the manifest and hash bytes. No dependency requirement, feature, or metadata change; neither dependency enters the runtime graph. |
| `src-tauri/vendor/heif-rs/build.rs` | Replaced (`Edited: build.rs`) | Keeps upstream's `main`, `emit_link_directives` (including the macOS compiler-rt workaround), and `generate_bindings`; replaces the unchecked download with manifest-driven resolution, SHA-256 verification before extraction, a local-only rule for committed archives, and a library hash on every run. |
| `src-tauri/vendor/heif-rs/build_support.rs` | Added (`Added: build_support.rs`) | Pure helpers compiled by `build.rs` and by `src-tauri/src/codec_artifacts.rs` through `#[path = ...]`: manifest parse, target select, URL render, hex SHA-256, checksum verify, and the local-vs-fetch policy. |
| `src-tauri/vendor/heif-rs/VENDORED.md` | Added (`Added: VENDORED.md`) | The fork record: upstream version and commit, the tarball hash, the edits above, the archive release, and the re-vendor procedure. |

`VENDORED.md` records everything else as byte-for-byte the published crate:
`src/`, `tests/`, `Cargo.toml.orig`, `Cargo.lock`, `.cargo_vcs_info.json`,
`LICENSE`, and `README.md`. The upgrade section repeats the rule ("Do not edit
this directory in place"), so a local fix has to arrive as a re-vendor or an
upstream change. No line under `src/` is fork-specific, which is why §4 audits
the published crate's contracts and not a local variant.

Two checks back the table. `diff -u
src-tauri/vendor/heif-rs/Cargo.toml.orig src-tauri/vendor/heif-rs/Cargo.toml`
shows cargo's normal manifest normalization — the expanded `[package]`, the
generated `[[test]]` blocks, the rewritten dependency tables — plus the two
added `[build-dependencies]`; the published `.orig` carries neither `serde_json`
nor `sha2`. The review did not re-download `heif-rs-26.10.3.crate`, so the
`build.rs` replacement is checked by reading rather than by diff: it keeps the
upstream steps `VENDORED.md` names (`main` directives, `emit_link_directives`,
`generate_bindings`) and resolves the archive through the added `build_support`
module. The recorded tarball hash is
`dc5863cecbc91f3b87a0cb508e864f76bafac4b772420ab26b5fa7219c939253`.

## 4. Call surface

`grep -n 'heif::' src-tauri/src/heic.rs` returns 20 lines. Seventeen are code
references — ten in the product path and seven in the seam's `#[cfg(test)]`
probe and classifier tests — and the other three are strings and a comment in
the seam's own tests. `grep -rln 'heif::' src-tauri/src` prints only
`src-tauri/src/heic.rs`, so this file is the whole surface.

| Lines | Reference | Contract the seam relies on | Honored? |
| --- | --- | --- | --- |
| 100, 112 | `heif::HeifDecoder::new(reader)` | `R: Read`; drains the reader, allocates a libheif context, parses the container, and returns the primary handle. Failures are `image::ImageError`: I/O as `IoError`, every container-side failure as `Decoding` (parse and missing-handle specifically `HeifError::Decode`), which the seam maps to `CorruptContainer`. | Yes — `src/decoder.rs:62-111`; I/O at `:64`, container paths at `:85` and `:91`. |
| 101 | `DynamicImage::from_decoder(decoder)` | The decoder's `ImageDecoder` impl supplies header `dimensions`/`color_type` before pixels, and `read_image` checks the output buffer length and decodes into it. | Yes — `src/decoder.rs:150-198`; length check at `:165-171`. |
| 113 | `decoder.dimensions()` | Header-only dimensions; no pixel decode. | Yes — `src/decoder.rs:151-153`, values captured in `new` at `:94-95`. |
| 124-130 | `heif::EncoderConfig { .. }` with `Default::default()` | Public `quality`, `preset`, `chroma`, `bit_depth`; defaults are 60 / `Slow` / `Yuv420` / `Eight`, and `..Default` fills `lossless: false`. | Yes — `src/encoder.rs:76-104`. |
| 127 | `heif::Chroma::Yuv420` | The 4:2:0 variant, rendered to libheif as `"420"`. | Yes — `src/encoder.rs:52-70`. |
| 128 | `heif::BitDepth::Eight` | The 8-bit output variant. | Yes — `src/info.rs:16-23`. |
| 132 | `heif::HeifEncoder::new_with_config(&mut buf, config)` | Infallible constructor over `W: Write`; encode failures surface from `write_image`, not from construction. | Yes — `src/encoder.rs:132-135`, `:545-550`. |
| 135 | `write_with_encoder(encoder)` | `ImageEncoder::write_image` rejects zero dimensions, unsupported color types, and wrong buffer lengths, encodes, then writes every byte with `write_all`. | Yes — `src/encoder.rs:251-265`, `:545-550`. |
| 141-145 | `heif::Preset::{Fast, Medium, Slow}` | Those variants exist and map to libheif's `"fast"`, `"medium"`, `"slow"`. | Yes — `src/encoder.rs:21-50`. |
| 187 | `heif::probe(&data)` (test-only) | Takes `&[u8]` and returns `ImageInfo { width, height, color_type, bit_depth }` without a pixel decode. | Yes — `src/lib.rs:101-110`. |
| 194-195 | `heif::BitDepth::{Ten, Twelve}` (test-only) | 10- and 12-bit variants; `bit_depth()` collapses every other depth to `Eight`. | Yes — `src/decoder.rs:121-127`; the enum says so at `src/info.rs:16-23`. |
| 225, 237 | `heif::HeifError::Decode` (test-only) | The variant `to_image_error` wraps for container and pixel failures. | Yes — `src/error.rs:14-15`; `src/decoder.rs:85`, `:91`, `:187`. |
| 249 | `heif::HeifError::EncoderInit` (test-only) | Renders exactly `encoder initialization failed: {0}` and is wrapped in `ImageError::Encoding`. | Yes — `src/error.rs:8-9`; `src/encoder.rs:540`. |
| 260 | `heif::HeifError::Encode` (test-only) | The catch-all encode failure variant the other encode paths return. | Yes — `src/error.rs:12-13`; `src/encoder.rs:390-429`. |

The seam's one stringly-typed dependency is deliberate. `classify_encode_error`
(`src-tauri/src/heic.rs:80-89`) separates `EncoderUnavailable` from `Encode` by
looking for `encoder initialization failed`, which is exactly
`HeifError::EncoderInit`'s `#[error(...)]` prefix (`src/error.rs:8-9`). The crate
honors it, and `encoder_init_failure_is_encoder_unavailable`
(`src-tauri/src/heic.rs:247-256`) goes red if the prefix moves. The bucket is
wider than "no x265": the crate also maps `heif_image_create`,
`heif_image_add_plane`, a null plane, and a null context to `EncoderInit`
(`src/encoder.rs:282`, `:316`, `:323`, `:341`, `:369`), so an allocation failure
is reported as `EncoderUnavailable` too.

Every product call above runs inside `guard` (`src-tauri/src/heic.rs:61-64`),
which turns an unwinding panic into `HeicError::Panic { stage }`. What that guard
can and cannot cover is §8's subject; nothing in this section depends on it.

Three further `grep` hits are not call-surface uses: `src-tauri/src/heic.rs:320`
(a doc comment), `:325` (the scanner's filter expression), and `:370` (a string
literal in `scan_flags_an_offender`). They belong to the test that keeps the seam
the only consumer, not to the product.

<!-- sections 5-12: later tasks -->
