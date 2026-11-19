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

## 5. `unsafe` register

Twenty lines under `src-tauri/vendor/heif-rs/src/` contain `unsafe` and are not
comments: five in `decoder.rs`, twelve in `encoder.rs`, two in `ffi.rs`, one in
`lib.rs`, and none in `error.rs`, `info.rs`, or `sys.rs` (`sys.rs:1` mentions it
in a doc comment only). Both `unsafe {` blocks and `unsafe fn` items are rows, as
is the one `unsafe extern "C" fn`. The list comes from:

```bash
grep -n "unsafe" src-tauri/vendor/heif-rs/src/*.rs
```

`bindgen`'s generated bindings are the other half of the FFI surface. They are
not in this register because they are not in the reviewed tree: `sys.rs` is five
lines and `include!`s `$OUT_DIR/bindings.rs` at build time, so every `extern`
declaration in `bindings.rs` is generated from the bundled `heif.h` header and
carries no hand-written invariant. Every hand-written call through them is a row
below. The libheif contracts cited here come from the headers inside the
committed archive (`include/libheif/*.h` in `bundle_linux_x64.zip`), which is the
header the bindings were generated from; a guarantee the header does not state is
called out as such rather than assumed.

| ID | Site | Operation | Invariant | Evidence |
| --- | --- | --- | --- | --- |
| U01 | src-tauri/vendor/heif-rs/src/decoder.rs:70 | Block in `HeifDecoder::new`: allocate the context, point libheif at the owned bytes without copying, fetch the primary handle, read the header facts. | `heif_context_alloc` may return null and is checked before use. `read_from_memory_without_copy` borrows `data` and requires that buffer to outlive the context; it does, because the success path moves that same `Vec` into `_data` without touching its heap allocation and `_data` is freed after the context in `Drop`. Both failure paths free the context before returning, so no early return leaks it, and `handle` is only read after `ffi::check` accepted the call. | `heif_context.h:251-256` in the bundled archive ("you will have to keep the memory area alive as long as you use the heif_context"); `decoder.rs:71-76` null check, `:84` and `:90` context freed on both error paths, `:99-109` data moved into `_data`, `:251-262` frees. |
| U02 | src-tauri/vendor/heif-rs/src/decoder.rs:174 | Block in `read_image`: set the thread cap, decode the primary handle to an interleaved RGB image, copy the pixels, then release the image. | `self.context` and `self.handle` are the non-null pointers built in `new` and not yet freed; `read_image` consumes `self`, so no second owner exists. `image` starts null and is read only after `ffi::check` returned Ok. The release at `:190` runs after `copy_pixels` on both its success and error paths, so the decoded image is freed exactly once. One assumption is not documented: that a failed `heif_decode_image` leaves the out-parameter untouched. If libheif wrote a non-null image and also returned an error, the `?` at `:187` would skip the release and leak it; that is a bounded leak, not unsoundness. | `decoder.rs:62-111`, `:164-192`; `heif_decoding.h:198-213` (decode, options may be NULL) and `heif_image.h:326-328` (release). |
| U03 | src-tauri/vendor/heif-rs/src/decoder.rs:206 | `unsafe fn copy_pixels` declaration: the caller promises a valid decoded `heif_image` owned by it. | The signature moves the obligation to the caller; the only caller passes the pointer returned by a successful `heif_decode_image` and releases it after this returns, so the image is live for every dereference inside. Nothing else holds or mutates the image. | `decoder.rs:179-191`; `heif_image.h:326-328`. |
| U04 | src-tauri/vendor/heif-rs/src/decoder.rs:214 | Block in `copy_pixels`: read the interleaved plane and its stride, then walk `height` rows of `width * channels * sample_bytes` bytes. | The plane pointer is null-checked. **The rest is not fully established.** The loop is bounded by `self.width`/`self.height`, which are the *handle* values read in `new` (`:94-95`), while the bundled header states the handle size is the signalled item size, "is not guaranteed to match the size of the image you get back from heif_decode_image", and must not be used "to allocate or bound access to a decoded pixel buffer" — a container whose signalled size exceeds the plane is read past its end. Separately, `stride: i32` is cast to `usize` with no sign or lower-bound check, so a negative stride becomes ~1.8e19 and the row arithmetic leaves the allocation. The stride failure is not reachable through libheif's own plane API for any file it can decode — it reports a positive bytes-per-line value — but the size mismatch can occur for a conforming file: `heif_decode_image` applies the container's rotation, cropping, and mirroring (`heif_decoding.h:198-199`), so the decoded plane need not have the signalled dimensions this loop is bounded by, and nothing in the wrapper checks. | `heif_image_handle.h:56-79` (do not bound a decoded buffer from the handle size); `heif_image.h:252-260` (stride is bytes per line, no sign promise); `heif_image.h:268-282` (the `*2` variants exist because "the stride is often multiplied by the image height ... illegal memory accesses", and require stride >= bytes_per_pixel * width); `decoder.rs:209-244`. |
| U05 | src-tauri/vendor/heif-rs/src/decoder.rs:254 | `Drop::drop` body: release the image handle, then free the context. | Both pointers are non-null-checked and each is released exactly once: the fields are private, never reassigned after construction, and `Drop` runs once. The order is legal either way because the header states the handle is ref-counted and the context may be released once a handle has been obtained. | `decoder.rs:42-57` and `:251-263`; `heif_image_handle.h:41-47` ("you can already release the heif_context, since it is internally ref-counted"); `heif_context.h:130-137`. |
| U06 | src-tauri/vendor/heif-rs/src/encoder.rs:273 | Block in `EncoderConfig::encode`: create the interleaved RGB `heif_image` at the requested size and chroma. | `image` starts null; `ffi::check` gates every later use, so a failed create returns before the pointer is touched and a successful one has written a non-null handle. The release at `:285` runs after `encode_with_image` returns whatever its result, so the image is freed exactly once on every path through the block. | `encoder.rs:274-287`; `heif_image.h:341-358` (create returns an error and an out pointer); `heif_image.h:326-328`. |
| U07 | src-tauri/vendor/heif-rs/src/encoder.rs:296 | `unsafe fn encode_with_image` declaration: caller guarantees a fresh image and a buffer that matches the layout. | The caller discharges it before the call: `layout_for` rejects every unsupported color type (`:255`) and the equality check at `:257-265` makes `buf` exactly `width * height * src_channels * src_sample_bytes`, so every `read_sample` offset computed in `fill_plane` is in bounds by construction. `image` is the non-null pointer `heif_image_create` just returned and is owned by `encode` for the duration. | `encoder.rs:244-288`, `:437-486`, `:214-220`. |
| U08 | src-tauri/vendor/heif-rs/src/encoder.rs:308 | Block in `encode_with_image`: add the interleaved plane, get its pointer and stride, fill it, allocate a context, encode into it, free the context. | `heif_image_add_plane` is checked, so an error returns before any pointer is taken. The plane pointer is null-checked, and the stride handed to `fill_plane` is the one libheif reports for the plane it just allocated at exactly `width`×`height`. The context is null-checked and freed after `encode_into_context` on both paths, so it is freed exactly once. | `encoder.rs:309-348`, `:451-482`; `heif_image.h:390-406` ("The specified width can differ from the row stride ... Always use the result of heif_image_get_plane"); `heif_context.h:130-137`. |
| U09 | src-tauri/vendor/heif-rs/src/encoder.rs:356 | `unsafe fn encode_into_context` declaration: `ctx` and `image` are live handles owned by the caller. | Called only from `:346` with the context allocated at `:339` (non-null checked) and the image created at `:274`. The function neither frees nor stores either pointer, so the caller's lifetime is the entire obligation. | `encoder.rs:339-348`, `:356-375`. |
| U10 | src-tauri/vendor/heif-rs/src/encoder.rs:362 | Block in `encode_into_context`: acquire the HEVC encoder for the context, hand it on, release it. | `encoder` starts null and is only read after `ffi::check` accepted the acquisition. `heif_encoder_release` runs after `encode_with_encoder` returns on success and on error, so the encoder bought here is released exactly once, and `ctx` is live per the function contract. | `encoder.rs:363-374`; `heif_encoding.h:103-114`. |
| U11 | src-tauri/vendor/heif-rs/src/encoder.rs:381 | `unsafe fn encode_with_encoder` declaration: `ctx`, `image`, and `encoder` are live handles owned by the caller. | The three pointers are the ones allocated at `:339`, `:274`, and `:363`, and those frames release them. This function frees only what it allocates itself (`options`, `handle`), so no owner is duplicated. | `encoder.rs:346-374`, `:381-433`. |
| U12 | src-tauri/vendor/heif-rs/src/encoder.rs:388 | Block in `encode_with_encoder`: apply quality/lossless/preset/chroma, encode the image, then serialise the container through a callback writer. | Every `heif_encoder_set_*` and the `heif_context_encode_image` result is checked. `options` is freed when non-null and the out handle released when non-null *before* the encode result is propagated, so a failed encode frees both. `out` is a local that outlives the `heif_context_write` call and `userdata` is `&mut out` cast to `*mut c_void`. `heif_encoding_options_alloc` is not checked, but the bundled header documents NULL options as the expected input to `heif_context_encode_image`, so the null case is a permitted degenerate path and the guarded free keeps it leak-free. | `encoder.rs:389-431`; `heif_encoding.h:334-341`, `:344-346` ("'options' should be NULL for now"); `heif_context.h:316-340`. |
| U13 | src-tauri/vendor/heif-rs/src/encoder.rs:474 | Block in `fill_plane`: write one output sample into the interleaved plane, one byte for 8-bit and two little-endian bytes above it. | The last byte written is at `(height-1)*stride + width*out_channels*out_sample_bytes - 1`. The loops bound `x` by `width`, `c` by `out_channels`, and `y` by `height`; `stride` is libheif's own row stride for the plane allocated at exactly that size; the pointer came from `heif_image_get_plane` and is used before the image is released at `:285`. | `encoder.rs:327-336`, `:451-485`; `heif_image.h:341-358`, `:390-406`. |
| U14 | src-tauri/vendor/heif-rs/src/encoder.rs:493 | `unsafe fn set_string_param` declaration: `encoder` must be a live libheif encoder. | Both call sites pass the encoder acquired at `:363` and not yet released. The function allocates no libheif object and stores nothing, so it cannot outlive the handle it is given. | `encoder.rs:397-398`, `:363-373`, `:489-505`. |
| U15 | src-tauri/vendor/heif-rs/src/encoder.rs:497 | Block in `set_string_param`: hand two `CString` pointers to `heif_encoder_set_parameter_string`. | `CString::new` guarantees NUL termination with no interior NUL, and both strings are locals of this frame, so they are alive for the call and dropped after it returns. The bundled header states no retention contract for `value`, so this is sound only if libheif copies the string before returning; the wrapper assumes that copy and cannot enforce it. | `encoder.rs:494-504`; `heif_encoding.h:212-215` (signature only, no lifetime note). |
| U16 | src-tauri/vendor/heif-rs/src/encoder.rs:508 | `unsafe extern "C" fn write_callback` declaration: an FFI callback libheif invokes synchronously from `heif_context_write`. | `extern "C"` means an unwind cannot cross the boundary: Rust aborts instead of unwinding into C, so the callback cannot corrupt libheif's frames. The body touches only `userdata`, which is the `*mut Vec<u8>` from the single call site, and returns a zeroed `heif_error`, which the header explicitly permits for a writer. | `encoder.rs:424-429`, `:508-524`; `heif_context.h:316-340`. |
| U17 | src-tauri/vendor/heif-rs/src/encoder.rs:516 | Block in `write_callback`: reborrow `userdata` as `&mut Vec<u8>`, copy `data[..size]` into it, return a zeroed `heif_error`. | `userdata` is `&mut out` from `:424-428` and that `Vec` is alive for the whole write. `data` and `size` describe libheif's valid buffer for the duration of the call. The null and zero-length cases are skipped, so `from_raw_parts` is only built from a non-null pointer with a non-zero length. `zeroed::<heif_error>()` is valid because the struct is two integer enums plus a nullable pointer and all-zero is `heif_error_Ok`. | `encoder.rs:419-429`, `:516-523`; `heif_context.h:316-340`; `heif_error.h:291-309`. |
| U18 | src-tauri/vendor/heif-rs/src/ffi.rs:16 | Block in `ffi::init`: call `heif_init` with null parameters. | `Once::call_once` makes the call happen at most once per process even under concurrent callers. A null `heif_init_params*` is explicitly permitted, so the pointer argument is sound. The returned `heif_error` is discarded: no library state is dereferenced on the strength of that return, but an init failure is unobserved and would surface later as whatever each libheif call reports. | `ffi.rs:12-20`; `heif_library.h:113-131` ("You may pass nullptr to get default parameters"). |
| U19 | src-tauri/vendor/heif-rs/src/ffi.rs:34 | Block in `ffi::check`: build a `&CStr` from `err.message` and copy it into a `String`. | The branch runs only for a non-Ok code and only after a defensive null check. The bundled header states the message is always defined and NUL-terminated, owned by libheif, and valid only until the next call on the originating object. `check` copies it into an owned `String` immediately, before the caller frees the context at `decoder.rs:84`, `decoder.rs:90`, or `encoder.rs:347`, so the borrow ends inside the function. | `ffi.rs:24-37`; `heif_error.h:299-309`. |
| U20 | src-tauri/vendor/heif-rs/src/lib.rs:35 | Block in `libheif_version`: build a `&CStr` from `heif_get_version()` and copy it into a `String`. | The wrapper assumes the returned `const char*` points at a static, NUL-terminated string that lives for the process. The bundled header documents the return type and the value's meaning but says nothing about storage duration, so the lifetime rests on libheif's implementation — a version string baked in at build time, which the bare `1.23.6` string in the archive is consistent with — and not on a written guarantee. The pointer is dereferenced exactly once and the text copied out immediately; nothing is freed. | `lib.rs:32-39`; `heif_library.h:69-76`; the archive carries the bare version string (a `strings` run over `lib/libheif.a` in the committed zip). |

Three rows carry an obligation instead of a proof. **U04** is the only site where
the wrapper's own bounds can disagree with the buffer libheif returned, and the
disagreement is one the bundled header warns about in as many words; it is
recorded as an incomplete invariant, not papered over. **U15** and **U20** rest
on behaviour libheif's header does not promise (that a parameter value is copied,
that the version string is static); both are latent contract gaps that a future
libheif could violate without changing its advertised API. §11 turns these into
findings.

## 6. Ownership and lifetime

Every libheif object the wrapper touches is either freed on the line after its
last use or owned by a struct with a manual `Drop`. There is no reference
counting of the wrapper's own and no arena: the ownership is per call, which is
what makes the seam's per-file isolation possible.

| Object | Allocated by | Freed by | Lifetime window |
| --- | --- | --- | --- |
| `heif_context` (decode) | `decoder.rs:71` | `decoder.rs:84` and `:90` on the two error paths, `:259` in `Drop` on success | From `new` until the decoder is dropped |
| `heif_image_handle` (decode) | libheif, written into `decoder.rs:88` | `decoder.rs:256` in `Drop` | From `new` until the decoder is dropped |
| `heif_image` (decode) | libheif, written into `decoder.rs:179` | `decoder.rs:190`, after `copy_pixels` | Only inside `read_image` |
| plane pointer (decode) | libheif, inside the image | with the image at `decoder.rs:190` | From the call at `:217` until `read_image` releases the image at `:190`, which is after `copy_pixels` returns |
| `heif_image` (encode) | `encoder.rs:275` | `encoder.rs:285` | The whole of `EncoderConfig::encode` |
| `heif_context` (encode) | `encoder.rs:339` | `encoder.rs:347` | The whole of `encode_with_image` after `:339` |
| `heif_encoder` | libheif, written into `encoder.rs:364` | `encoder.rs:372` | The whole of `encode_into_context` after `:364` |
| `heif_encoding_options` | `encoder.rs:401` | `encoder.rs:410-412`, only when non-null | From `:401` until the free, bracketing one `heif_context_encode_image` |
| `heif_image_handle` (encode) | libheif, written into `encoder.rs:403` | `encoder.rs:413-415`, only when non-null | The same bracket |
| `Vec<u8>` of encoded bytes | `encoder.rs:419` | `encoder.rs:548` (`write_all`) then normal drop | Local to `encode_with_encoder`, moved out as `Ok(out)` |

**Decoder: field order and the manual `Drop`.** `HeifDecoder` declares nine
fields; the first three are the two raw pointers and the owned buffer, and the
six after them hold plain data with no destructor:

```
context: *mut sys::heif_context,       // decoder.rs:44
handle:  *mut sys::heif_image_handle,  // decoder.rs:46
_data:   Vec<u8>,                      // decoder.rs:49
config:  DecoderConfig,                // decoder.rs:50
width, height, depth: u32,             // decoder.rs:51-53
alpha_present: bool,                   // decoder.rs:54
_reader: PhantomData<R>,               // decoder.rs:56
```

Two independent facts put the buffer's free after the context's free. First,
`Drop::drop` (`:251-263`) runs its body *before* any field is dropped, and the
body releases the handle and then frees the context. Second, fields are then
dropped in declaration order: `context` and `handle` drop as no-ops, then
`_data` is destroyed, and only the inert fields follow it. `_data` is therefore
the last field with a destructor, not the last field declared — `config`, the
three integers, `alpha_present`, and `_reader` release nothing, so no memory
libheif references is freed after the buffer. That ordering is the whole point:
libheif's memory IO does not copy the bytes, and the header requires the buffer
to stay alive "as long as you use the heif_context" (`heif_context.h:251-252`).

The pointer stays valid across the move into the struct at `:106` because a
`Vec` move relocates its 24-byte header, not the heap allocation, and `_data` is
never mutated or reallocated afterwards. The doc comment at `:47-48` says "Never
moved out"; that is the rule, and the private field plus the absence of any
`mem::take` or `into_inner` is what enforces it.

The handle does not need to outlive the context. The header states a handle is
internally ref-counted and the context may already be released once the handle
exists (`heif_image_handle.h:41-43`); the wrapper's handle-then-context order in
`Drop` is the conservative choice, not a requirement. Handles obtained from a
context are the only thing that keeps the compressed bytes reachable, which is
why `_data` has to outlive *both*.

**Decoder: the plane pointer's validity window.** `copy_pixels` asks libheif for
the plane (`:217`), uses it for the whole row loop (`:225-244`), and returns;
`read_image` then releases the image at `:190`. The pointer is never stored, so
its window is exactly the body of `copy_pixels`. `read_image(self, ...)` takes
the decoder by value, so the decoder — and with it the context and handle — is
still alive while the plane is being read, and is dropped after `read_image`
returns. There is no path where the image is released before the copy and none
where the plane escapes.

**Decoder: the failure paths.** `new` allocates the context, then has two
`return Err` paths, at `:85` (container parse failed) and `:91` (no primary
handle). Both call `heif_context_free` first; neither has a handle to release at
that point, because the handle is only written on success. `read_image` has one
early return at `:187` (pixel decode failed), before an image exists to release,
on the assumption noted in U02 that libheif leaves the out-parameter untouched
when it reports an error.

**Encoder: local frees on every path.** The encoder holds no libheif object
between calls — `HeifEncoder<W>` is a `writer` and a `config` (`:118-121`) — so
every acquisition is a local whose release is written on the same line pair. In
`encode`, `image` is created at `:275` and released at `:285` *after*
`encode_with_image` returns, so both its success and error returns are covered.
In `encode_with_image`, `ctx` is allocated at `:339`, null-checked at `:340`, and
freed at `:347` after `encode_into_context` returns. In `encode_into_context`,
`encoder` is acquired at `:364` and released at `:372` after
`encode_with_encoder` returns. In `encode_with_encoder`, `options` and the out
handle are freed or released at `:410-415` before the encode result is
propagated with `?` at `:416`, so a failed encode still frees both. The only
early returns above those points are the argument checks at `:251-265` (nothing
allocated yet), the `add_plane` and null-plane returns at `:316` and `:323`
(caller still frees the image), and the null-context return at `:341` (nothing
else allocated). Walking every `?` in the four functions leaves no allocated
object behind.

**Encoder: the writer's `userdata`.** `out` is a local `Vec<u8>` at `:419`; the
`heif_writer` at `:420-423` and the `userdata` argument at `:427` are both
borrowed from it for the duration of `heif_context_write`. libheif calls
`write_callback` synchronously inside that call, and the callback reborrows the
same `Vec` as `&mut` (U17). Neither the writer struct nor the pointer outlives
the call, so `out` is a plain local again by `:431`, when it is returned as the
encoded bytes. `write_image` then copies them to the `Write` sink with
`write_all` (`:548`) and drops the encoder and its writer.

**Options.** `heif_encoding_options` is the one object whose allocation is not
checked, and it is also the one whose null value is documented as acceptable:
the bundled header says "`options` should be NULL for now"
(`heif_encoding.h:344-346`), so a failed allocation degrades to the documented
default rather than to a null dereference. The free is guarded by
`!options.is_null()` (`:410`), which makes the null branch leak-free. The pointer
is never stored past the `encode_image` call. On the decode side the options
argument is `ptr::null()` (`decoder.rs:185`), also documented as allowed
(`heif_decoding.h:206-207`).

**Process-global state.** `ffi::init` is the only object with a lifetime longer
than a call: `Once` calls `heif_init` at most once and there is no `heif_deinit`
anywhere in the crate, so libheif's globals live until the process exits. §7
covers the consequences; the ownership point is that nothing in the wrapper can
free them, and nothing tries.

## 7. Concurrency and global state

Every libheif object the wrapper owns is per call. What it shares between calls
is libheif's process-global state, and the only thing that touches it is one
`Once`.

**One initialisation.** `ffi::init` (`src-tauri/vendor/heif-rs/src/ffi.rs:12-20`)
holds `static INIT: Once` (`:13`) and runs `sys::heif_init(ptr::null_mut())`
inside `call_once` (`:14-19`). It is called on every decode (`src/decoder.rs:66`,
right after the file is read) and every encode (`src/encoder.rs:267`), so the
first HEIC operation in the process does the work and every later one is a
branch. The
returned `heif_error` is discarded (`ffi.rs:17`), and because `Once` does not
retry, an initialisation failure is permanent for the process and unobservable
where it happens: each later libheif call reports whatever its own entry point
says.

libheif's `heif_init` is itself reference counted under a mutex, so the `Once` is
belt and braces. Disassembling the committed archive's `init.cc.o` shows
`pthread_mutex_lock`, a counter in `.bss` that is tested and incremented, and
only on the zero branch the work: `register_default_plugins` (behind its own
guard flag) and then `heif_load_plugins(dir, NULL, NULL, 0)` for every directory
`get_plugin_paths` returns:

```
$ unzip -p src-tauri/vendor/heif-bundles/bundle_linux_x64.zip lib/libheif.a > /tmp/libheif.a
$ cd /tmp && ar x libheif.a init.cc.o plugins_unix.cc.o
$ objdump -dr --disassemble=heif_init init.cc.o \
    | grep -E 'heif_load_plugins|get_plugin_paths|register_default_plugins'
			e59: R_X86_64_PLT32	heifrs__Z16get_plugin_pathsB5cxx11v-0x4
			e8b: R_X86_64_PLT32	heif_load_plugins-0x4
			ee1: R_X86_64_PLT32	heifrs__Z24register_default_pluginsv-0x4
```

**No `heif_deinit`.** `grep -rn "heif_deinit" src-tauri/vendor/heif-rs/src` prints
only `ffi.rs:10`, the comment that explains the choice. The header says the pair
is reference counted, that each `heif_init` should be matched, and that some
library memory objects are never freed without it (`heif_library.h:116-120`).
What stays resident is the plugin registry built by that single initialisation,
the plugin instances it holds, and libheif's globals: one-time, bounded by the
library's own structures rather than by the number of files, and reclaimed by the
OS at exit. It also means the plugin path is scanned exactly once, at first use,
so a directory that appears later is never picked up.

**What `heif_init` loads, and from where.** The header is explicit that
`heif_init` "will load the external modules installed in the default plugin
path", and that codecs compiled into the library are available even without it
(`heif_library.h:122-125`). In this archive the scan is driven by the
environment, and by nothing else:

```
$ objdump -dr --disassemble=heifrs__Z16get_plugin_pathsB5cxx11v init.cc.o \
    | grep get_plugin_directories_from_environment_variable
			726: R_X86_64_PLT32	heifrs__Z53get_plugin_directories_from_environment_variable_unixB5cxx11v-0x4
$ readelf -p .rodata.str1.1 plugins_unix.cc.o | grep LIBHEIF
  [    3c]  LIBHEIF_PLUGIN_PATH
```

`get_plugin_paths` calls only that reader, the string handed to `getenv` is
`LIBHEIF_PLUGIN_PATH`, and `strings -a lib/libheif.a | grep -c "/usr/"` prints
`0`, so no built-in directory is compiled in. The bundled header, however, names
the variable `LIBHEIF_PLUGIN_PATHS` (`heif_library.h:181-185`). That
byte-versus-header disagreement is recorded here; §10 Q13 states what it means
for reachability.

**Rayon worker counts.** `convert_all` sizes a private pool from
`std::thread::available_parallelism()` (`src-tauri/src/convert.rs:65-67`) with two
formulas (`:69-73`): HEIC output gets `(cores / 6).clamp(1, 3)` workers, every
other output `(cores / 2).clamp(1, 6)`, built at `:75` and installed for the batch
at `:128-131` with a fallback to the global pool if the builder fails. A HEIC
batch therefore runs at most three `encode` calls at once, each one decode and one
x265 encode. Two paths are not sized by that formula: `scan::scan_with_limit`
describes candidates on the *global* Rayon pool (`src-tauri/src/scan.rs:132-135`),
calling `decode::dimensions` — a full file read and container parse per HEIC
(`scan.rs:155`, `src-tauri/src/heic.rs:108-115`) — and the frontend keeps at most
four `load_thumbnail` calls in flight (`src/App.tsx:235`), each a
`spawn_blocking` full decode (`src-tauri/src/lib.rs:36-40`). Scan, thumbnails,
and a conversion can overlap, so the process can hold more HEIC contexts than the
conversion cap suggests.

**Decode threading inside libheif.** `HeifDecoder::with_threads`
(`src/decoder.rs:113-117`) is the only way `config.threads` becomes `Some`, and
`grep -rn "with_threads" src-tauri/src` prints nothing, so the
`heif_context_set_max_decoding_threads` call at `src/decoder.rs:175-177` never
executes and libheif's default stands. The bundled header does not publish that
default: it documents 0 as main-thread tile decoding and 1 as a single background
thread (`heif_decoding.h:34-35`), says the getter returns "either the library
default or the value last passed" (`:42-47`), and warns that the setting "only
affects libheif itself. The codecs itself may still use multi-threaded decoding"
(`:36`). The pool formula therefore bounds files, not threads: three HEIC workers
can each have libde265's own pool running underneath, and nothing here sets a
ceiling on that.

**Per-call contexts.** No context, handle, plane, or encoder is cached or shared.
`HeifDecoder::new` allocates its context (`src/decoder.rs:71`) and frees it in
`Drop` (`:259`); `EncoderConfig::encode` allocates a context per encode
(`src/encoder.rs:339`) and frees it on both paths (`:347`). There is no
`static mut`, no thread-local, and no pooling, so two files never share libheif
state below the plugin registry: one file's parse or decode cannot observe
another's context. The shared surface is CPU and memory contention, plus the
process-global state §9 and Q13 discuss.

## 8. Panics and unwinding

Two guards are the whole policy: the seam's `guard` (`src-tauri/src/heic.rs:59-64`)
around each HEIC entry point, and `convert.rs`'s per-file `catch_unwind`
(`src-tauri/src/convert.rs:98-113`).

**The two guards.** `guard` wraps `decode_from` (`heic.rs:99`), `dimensions`
(`:110`), `encode` (`:120`), and the test-only `probe` (`:185`), turning an unwind
into `HeicError::Panic { stage }` (`:61-64`). `AssertUnwindSafe` is sound here
because the closure owns everything it touched and the caller gets an error
instead of the value, so no half-updated state is observed;
`a_panicking_reader_is_a_decoder_panic` (`heic.rs:272-276`) exercises it with a
`Read` impl that panics. The outer net in `convert.rs:98-100` wraps the whole
pipeline — `convert_one` → `encode_bytes` → `decode::load` → format encode →
`std::fs::write` — and reports `"internal error while converting this file"`
(`:110`) for anything that unwinds. A HEIC panic is caught twice; a PNG/JPEG
encode, resize, or `image`-crate panic is caught once.

**Every reachable panic in the wrapper.**

| Site | Panic | Reachable? |
| --- | --- | --- |
| `src/encoder.rs:494`, `:495` | `CString::new(..).expect(..)` on the parameter name and value. | Not through this product: the values are `Preset::as_str()` (`:36-49`) and `Chroma::as_str()` (`:64-70`), constant ASCII with no interior NUL. These two are the crate's only `expect`s; `grep -rn "unwrap" src/*.rs` prints nothing. A future variant with a NUL would change that. |
| `src/decoder.rs:211` | `16 - self.depth` underflows: debug panic, release wrap. | Only if a file reports a luma depth above 16. The linked codecs signal 8/10/12 (Q03), so this is latent, not reachable. |
| `src/decoder.rs:223-226` | `stride as usize` followed by `y * stride` overflows for a negative stride: debug panic, release wrap to a wild read. | Not reachable through libheif's plane API, which reports positive bytes per line (Q04). The result would be a read, not a write. |
| `src/decoder.rs:231`, `:240-241`; `src/encoder.rs:216-218` | Slice indexing into `buf` (`buf[dst_row..]`, `buf[off]`, `buf[off + 1]`, `read_sample`). | In bounds by construction: both sides require exact length equality first (`decoder.rs:166-171`, `encoder.rs:257-265`). |
| `src/decoder.rs:165`; `src/encoder.rs:257`, `:449` | `usize` products. | These *can* overflow a 64-bit `usize` at extreme dimensions — `(2^32 - 1)^2 * 3` already exceeds it — but they are never evaluated at those sizes. On the decode side `ImageDecoder::total_bytes` computes the product in `u64` with `saturating_mul` (`image-0.25.10/src/io/decoder.rs:66-71`) and `decoder_to_vec` rejects anything above `isize::MAX` before `read_image` runs (`src/io/free_functions.rs:309-314`), on 32- and 64-bit targets alike. On the encode side the dimensions come from an already-allocated `DynamicImage` buffer (`heic.rs:134`), so the same extreme sizes cannot be presented to begin with. |
| `src/encoder.rs:519` | `Vec::extend_from_slice` in `write_callback`: capacity overflow panics, allocation failure aborts. | The only panic on the encode path with an untrusted size input; see the `extern "C"` paragraph below. |

**Allocation failure is not a panic.** Rust's default allocation-error handler
aborts the process rather than unwinding, so `catch_unwind` at `heic.rs:62` and
`convert.rs:98` cannot cover an out-of-memory in `read_to_end` (`decoder.rs:64`),
in the `image` crate's decode buffer (`free_functions.rs:316`), or in the writer's
`Vec` (`encoder.rs:519`). `Cargo.toml` deliberately leaves `panic = "abort"` unset
(the comment above `[profile.release]`), which is what keeps ordinary panics
catchable; it does not change the allocator's behaviour.

**The `image` crate's own paths.** `DynamicImage::from_decoder`
(`image-0.25.10/src/images/dynimage.rs:243-245`) reads dimensions and color type,
then `decoder_to_image` (`:1557-1624`) saturates `total_bytes` at `u64::MAX`
(`src/io/decoder.rs:66-71`), rejects anything above `isize::MAX` with
`ImageError::Limits` (`src/io/free_functions.rs:309-314`), allocates a zeroed
buffer (`:316`), calls `read_image`, and maps a short buffer to
`ParameterError::DimensionMismatch` (`dynimage.rs:1612-1616`). Absurd dimensions
are therefore an error, not a panic; the failure mode that escapes is the
allocation.

**The `extern "C"` boundary.** `write_callback` is `unsafe extern "C" fn`
(`src/encoder.rs:508-524`) and libheif calls it synchronously from
`heif_context_write` (`:424-429`). A Rust function declared `extern "C"` is a
non-unwinding boundary: if a panic starts inside it, the compiler's abort shim
runs and the process aborts on the spot. Unwinding into libheif's C++ frames is
not what happens and is not possible with this ABI — permitting it would require
`extern "C-unwind"`, which the function does not use. The consequence is that the
guards above sit *above* `heif_context_write` in the stack, so a panic raised in
the callback aborts below them and neither `catch_unwind` ever sees it. The
reachable trigger is `out.extend_from_slice(...)` (`:519`) with a size that
overflows `Vec` capacity; an allocation failure there aborts for the reason above.

**What the guards do and do not buy.** They cover Rust panics that unwind through
Rust frames: every arithmetic, index, and `expect` panic in the table, plus
anything the `image` crate raises. They do not cover allocation failure (abort),
stack overflow (abort), a segfault or other fault inside libheif/libde265/x265
(the process dies and no `FileOutcome` is ever built), a `panic = "abort"` build
(not configured), or the `extern "C"` boundary. So the per-file isolation
guarantee — one bad file costs one row in the results list — holds for the first
group and not for the others. One detail: `catch_unwind` does not silence the
log, because the default hook runs before unwinding; a caught panic still prints
to stderr while the batch continues.

## 9. Untrusted input and resource bounds

The product treats every input file as hostile, but it applies no limit of its own
to size, pixel count, or parse work. What it has is the read the wrapper does, the
checks `image` does, and whatever libheif enforces internally.

**Whole-file reads.** `HeifDecoder::new` reads the entire input into a fresh
`Vec<u8>` before parsing anything (`src/decoder.rs:63-64`), and the seam calls it
for `decode` (`heic.rs:94-100`) and for `dimensions` (`heic.rs:109-113`); the
test-only `probe` does the same with `std::fs::read` (`heic.rs:184`). There is no
size limit in the seam, in `decode.rs`, or in `scan.rs`: the only ceiling is
`MAX_FILES = 20_000` (`scan.rs:14`), a count, and `describe` calls `dimensions`
for every candidate (`scan.rs:132-135`, `:155`), so opening a folder of large
HEICs reads each file in full just to report a dimension. The buffer grows by
reallocation, so peak memory can exceed the final file size by the old block
still being live during a copy.

**The decoded-pixel multiplier.** After the container parses, the wrapper takes
`width` and `height` from the handle (`decoder.rs:94-95`), derives a `ColorType`
from `depth > 8` and alpha (`:155-162`), and the `image` crate allocates
`width * height * bytes_per_pixel` (`image-0.25.10/src/io/decoder.rs:66-71`,
`src/io/free_functions.rs:316`); `read_image` then demands exactly
`width * height * channels * sample_bytes` (`decoder.rs:165-171`, `channels` 3 or
4 at `:130-132`, `sample_bytes` 1 or 2 at `:135-137`). The cost is 3 bytes per
signalled pixel for RGB8, 4 for RGBA8, 6 for RGB16, and 8 for RGBA16. A container
a few hundred bytes long can signal 65,000 × 65,000 — 4.225 Gpx — which asks for
12.7 GB at 3 bytes and 33.8 GB at 8. libheif's own decoded `heif_image` is alive
at the same time (`decoder.rs:179-191`), and the `DynamicImage` wrap plus any
`to_rgb8`/`flatten_to_rgb` copy (`heic.rs:134`, `decode.rs:82-99`) allocate again,
so the true peak is a multiple of that figure. When the allocation cannot be
satisfied the process aborts (§8), so the outcome is a dead process rather than a
`FileOutcome` carrying an error.

libheif has limits aimed at exactly this: `heif_security_limits` carries
`max_image_size_pixels`, `max_number_of_tiles`, `max_items`,
`max_memory_block_size`, and `max_total_memory` (`heif_security.h:36-88`); a new
context gets the global limits by default (`:91-94`); and a context's limits can
be read and overwritten (`:100-108`). The bundled header does not state the
numeric defaults, and the wrapper never reads or tightens them
(`grep -rn "security" src-tauri/vendor/heif-rs/src` prints nothing), so this
review asserts only that libheif applies its own defaults and does not invent a
figure. Passing `ptr::null()` as the decoding options (`decoder.rs:185`) is
documented as allowed (`heif_decoding.h:206-207`) and changes no limit.

**Malformed containers.** A container parse failure and a missing primary handle
both become `HeifError::Decode` (`decoder.rs:78-92`) and then
`HeicError::CorruptContainer` at the seam (`heic.rs:66-70`, `:100`); a
pixel-decode failure becomes `HeifError::Decode` (`decoder.rs:180-187`) and
`HeicError::Decode` (`heic.rs:73-78`, `:101`). Truncated and garbage inputs are
covered by the existing seam tests (`heic.rs:432-457`). The wrapper validates
nothing about the box structure itself: it hands libheif the whole buffer and its
length (`decoder.rs:78-83`) and libheif's parser does the rest, bounded by the
security limits above. The one wrapper-side bound that is known to be wrong is the
one U04 records: the copy loop trusts the *header* dimensions
(`decoder.rs:225-226`) although the same header warns that the decoded plane can
differ after rotation, cropping, or mirroring (`heif_image_handle.h:56-79`,
`heif_decoding.h:198-199`), so a transformed or hostile file can read past the
plane. That is a crash path, not a bounded error.

**The plugin-load surface.** The first decode or encode calls `heif_init`, which
loads external modules from the plugin path (`heif_library.h:122-125`); on this
archive that means `dlopen`ing files found in the directory named by
`LIBHEIF_PLUGIN_PATH` (§7, Q13). Loading a shared object runs its constructors
inside this process, so whoever controls that variable and can write into the
named directory gets code execution in the app. The privileges required are
exactly those two: write access to the directory, and the ability to set the
process environment at launch — the same user, or a parent process that launches
the app. Nothing in this repository sets the variable
(`grep -rn "LIBHEIF" src-tauri/src scripts` prints nothing) and a HEIC file cannot
add a plugin path, so the surface is local: someone who can already act as the
user, or a distribution that ships a plugin directory an unprivileged user can
write. Two smaller notes: the scan happens once under `Once`, so a later change
to the directory is not observed; and libheif version-checks the plugin ABI (the
archive carries "Encoder plugin requires at least libheif version 1.23.6"), which
raises the bar for a file that merely happens to be named `*.so` but not for an
attacker who can write one.

Finally, one file can occupy every core without exceeding any app limit: the
header states the thread setting "only affects libheif itself. The codecs itself
may still use multi-threaded decoding" (`heif_decoding.h:36`), so the Rayon cap
bounds files in flight and not threads per file.

## 10. Seeded questions

`Q01`–`Q13` are answered here; the register, the lifetime map, and §7–§9 above
are the evidence base they cite. `Q14`–`Q17` follow in later sections of this
document.

| ID | Question | Anchor | Answer |
| --- | --- | --- | --- |
| Q01 | Does `bit_depth()` collapse every depth other than 10 and 12 to `Eight` while layout selection uses `depth > 8`? What does a 9-bit file report and decode as? | `src/decoder.rs:121-127`, `135-162`, `211` | Yes. `bit_depth()` matches `12 => Twelve, 10 => Ten, _ => Eight` (`:122-126`), while `sample_bytes()` (`:136`), `color_type()` (`:156-161`), `decode_chroma()` (`:141-146`) and the up-shift (`:211`) all key on `depth > 8`. A 9-bit file therefore reports `BitDepth::Eight` — `heif::probe` (`lib.rs:101-110`) and the seam's `probe` (`heic.rs:193-197`) both record Eight — yet decodes as `Rgb16` with two-byte samples and `up_shift = 16 - 9 = 7`. So the header report is wrong and the pixels are right: the values are 9-bit scaled into 16-bit lanes. The same collapse covers 11-bit and any depth libheif reports above 8 that is not 10 or 12. |
| Q02 | Do container-parse failures, a missing primary handle, and pixel-decode failures all surface as `HeifError::Decode`? What does that mean for the seam's two classifiers? | `src/decoder.rs:85`, `91`, `187`; `src/error.rs:8-24` | Yes: all three are `HeifError::Decode`, and `DecoderInit` (`:73`) joins them under the same `ImageError::Decoding` wrapper built by `to_image_error` (`:266-268`). The seam's split is therefore made by *call site*, not by error value: `heic.rs:100` sends every non-`IoError` from `HeifDecoder::new` to `CorruptContainer`, and `heic.rs:101` sends every non-`IoError` from `DynamicImage::from_decoder` to `Decode` (`heic.rs:66-78`). Consequences: an allocation failure in `heif_context_alloc` is reported to the user as "this file is damaged or not a HEIC image"; a pixel-decode failure that is really a missing decoder plugin is reported as a generic decode failure; and the `IoError` arm of `classify_decode_error` (`heic.rs:75`) is unreachable for this decoder, because the reader is drained in `new` and `read_image` performs no I/O. The container classifier's `IoError` arm is live — it catches a read error during `read_to_end` (`decoder.rs:64`). |
| Q03 | Is `heif_image_handle_get_luma_bits_per_pixel(...).max(0) as u32` safe, and can `16 - depth` underflow for an out-of-range luma depth? | `src/decoder.rs:96`, `211` | The `.max(0)` is what makes the cast safe. The header returns `int` with `-1` on error (`heif_image_handle.h:92-95`), and without the clamp `-1 as u32` would be 4294967295, putting `depth > 8` and then `16 - depth` straight into underflow. Clamped, a `-1` becomes `0`, which selects the 8-bit path and reports `Eight`. `16 - depth` can still underflow for any reported depth above 16, and nothing validates an upper bound, but no reachable value does that: the HEVC profiles the linked codec stack carries signal 8, 10, or 12 bits, which are exactly the depths `bit_depth()` distinguishes. If a future libheif reported 20, the subtraction would panic in a debug build (caught by the seam's `guard`) and wrap in release, producing a nonsense shift amount; `scaled` is truncated to `u16` and written to a fixed offset, so the result is wrong pixels, not an out-of-bounds access. Safe as shipped, with a latent debug-only panic path. |
| Q04 | Can a negative or absurd `stride` from libheif reach `stride as usize` in `copy_pixels`? | `src/decoder.rs:215-231` | Yes — the cast at `:223` has no check at all. `stride` is an `i32` filled by the deprecated `heif_image_get_plane_readonly` (`:215-217`); a negative value becomes ~1.8e19, `y * stride` then overflows `usize` (panic in debug, wrap in release), and `plane.add()` produces a wild pointer that the row copy reads from. A small *positive* stride is the quieter failure: `out_row_bytes` (`:209`) is copied per row from a source row advanced by `stride`, so a stride below `width * channels` reads the next row's bytes as padding and yields wrong pixels. Reads stay read-only and the destination is bounded by `buf`'s length (checked at `:166-171`), so the worst case is a process abort or corrupted output, not a write past the caller's buffer. libheif 1.23.6 returns the plane's positive bytes-per-line, so this is not reachable with well-formed input; the bundled header exists in the shape it does because this exact pattern is the known hazard (`heif_image.h:268-282`). |
| Q05 | Does the owned `Vec<u8>` really outlive the libheif context, and in what order do the manual `Drop` and field drops run? | `src/decoder.rs:47-49`, `106`, `251-263` | Yes, twice over. `Drop::drop` (`:252-262`) runs its body before any field is dropped, and the body releases the handle (`:256`) and then frees the context (`:259`); after that, fields drop in declaration order — `context`, `handle`, then `_data` (`:44-49`). `_data` is therefore freed strictly after the context on every path. The buffer also keeps its address across the move at `:106`, because moving a `Vec` moves its header, not its heap block, and `_data` is never mutated afterwards; that is what lets `heif_context_read_from_memory_without_copy` keep a bare pointer to it. The handle's release before the context's free is safe even though the wrapper does not require it: the header says handles are ref-counted (`heif_image_handle.h:41-43`). |
| Q06 | Is `heif_init` called once and never balanced by `heif_deinit`? What is leaked, and for how long? | `src/ffi.rs:12-20` | Yes. `ffi::init` calls `heif_init(NULL)` inside `Once::call_once` (`ffi.rs:13-19`), so it runs at most once per process and cannot be re-run; `heif_deinit` appears nowhere in the crate except the comment at `ffi.rs:10` that explains the choice. The bundled header says the pair is reference counted, that each `heif_init` should be matched, and that "some library memory objects will never be freed if you do not call heif_init()/heif_deinit()" (`heif_library.h:113-131`). What leaks is libheif's process-global state — the plugin registry populated by `heif_init`'s scan of the default plugin path, the codec plugin instances it holds, and library globals. It is bounded and one-time: it does not grow per file, per decoder, or per encode call, and the OS reclaims it at exit. The other consequence is that the default plugin path is scanned exactly once, at first use, which is the plugin question `Q13` takes up. |
| Q07 | Can a panic escape `write_callback`, and does per-file isolation survive it? | `src/encoder.rs:508-524`; `src-tauri/src/convert.rs:98`; `src-tauri/src/heic.rs:62` | It cannot unwind out. `write_callback` is `unsafe extern "C" fn` (`encoder.rs:508`), so a panic raised inside it reaches the abort shim at the FFI boundary and the process aborts; unwinding into libheif would need `extern "C-unwind"`, which the wrapper does not use. The reachable trigger is `out.extend_from_slice(...)` (`:519`), which could only panic on capacity overflow for an output above `isize::MAX`; the realistic abort at that line is allocation failure. The guards at `convert.rs:98` and `heic.rs:62` sit above `heif_context_write` (`encoder.rs:424-429`), so neither sees it: one file's encode abort kills the batch, the app, and every worker. Ordinary Rust panics on the encode path are still caught; OOM and C-level faults are not. |
| Q08 | Is the result of `heif_encoding_options_alloc` checked before use? | `src/encoder.rs:401-412` | No. `options` is stored unchecked at `:401` and passed straight to `heif_context_encode_image` (`:403-409`); only the free is guarded by `!options.is_null()` (`:410-412`). The bundled header makes NULL the expected input — "`options` should be NULL for now" (`heif_encoding.h:346`) — so a failed allocation degrades to the documented default instead of a null dereference, and the guarded free keeps it leak-free. U12 and §6 record the same, so the only cost of the missing check is that the allocation failure is invisible. |
| Q09 | Which encoder paths are reachable given the seam always requests 4:2:0, 8-bit, non-lossless? | `src/encoder.rs:232-241`, `389-398`; `src-tauri/src/heic.rs:124-130` | Exactly one: `interleaved_chroma` takes `(false, false)` and returns `heif_chroma_interleaved_RGB` (`:234-241`) because `image_depth(Eight) > 8` is false (`:204-210`), and `encode_with_encoder` takes the lossy branch (`:389-395`) with `chroma = "420"` and one of three presets (`:397-398`; `Preset::as_str` at `:36-49`, selected by `heic.rs:141-147`). Unreachable from this seam: `heif_encoder_set_lossless` (`:390`), the RGBA/RRGGBB/RRGGBBAA chromas (`:237-239`), the two-byte sample path and 10/12-bit output (`:304-305`, `:475-481`), and seven of `layout_for`'s eight rows (`:192-199`) because `heic.rs:134` always hands over `Rgb8`. Later re-enabling 10-bit output would enter all of them at once. |
| Q10 | Which color types does `layout_for` accept, and how are grayscale and alpha mapped into the interleaved plane? | `src/encoder.rs:178-202`, `459-470` | Eight: `L8`, `La8`, `Rgb8`, `Rgba8`, `L16`, `La16`, `Rgb16`, `Rgba16` (`:191-201`); anything else is `None` and becomes `ImageError::Unsupported` (`:255`, `:536-539`). Grayscale expands by replicating input channel 0 into R/G/B and, when alpha is present, reading input channel 1 into output channel 3 (`:459-470`); alpha is otherwise carried in the last output channel (`:193-199`). The product reaches none of it: `heic.rs:134` passes `ImageRgb8(image.to_rgb8())`, so the layout is always `(3, 1, 3, false, false)` (`encoder.rs:194`). |
| Q11 | Which libheif return values are used without a check, and which null pointers are checked? | `src/decoder.rs:94-97`, `218-222`; `src/encoder.rs:320-326`, `339-343`, `401-415` | Unchecked: the four header accessors at `decoder.rs:94-97` (`heif_image_handle_get_width`, `_get_height`, `_get_luma_bits_per_pixel`, `_has_alpha_channel`); the two strides at `decoder.rs:215-223` and `encoder.rs:320-329`; `heif_encoding_options_alloc` at `encoder.rs:401`, whose result is passed to `heif_context_encode_image` at `:407` with only the free null-guarded at `:410`; `heif_init`'s returned `heif_error`, discarded at `ffi.rs:17`; and `heif_get_version()` at `lib.rs:36` (analysed in U20). Every other call that returns a status goes through `ffi::check` (`ffi.rs:24-37`), which reads `err.code` and copies the message. |
| Q12 | `with_threads` is never called — what thread count does libheif use, and how does that interact with the Rayon HEIC worker cap? | `src/decoder.rs:113-117`, `174-177`; `heif_decoding.h:34-47` | `with_threads` only sets `config.threads` (`:113-117`), and `grep -rn "with_threads" src-tauri/src` prints nothing, so it stays `None` and the `heif_context_set_max_decoding_threads` call at `:175-177` never runs; libheif's default applies. The bundled header states no number: 0 means tiles decoded in the main thread and 1 means one background thread (`heif_decoding.h:34-35`), the getter returns "either the library default or the value last passed" (`:42-47`), and "the codecs itself may still use multi-threaded decoding" (`:36`) — so this review records the default as undocumented rather than guessing it. The Rayon cap (`convert.rs:69-73`) limits HEIC *files* to `(cores / 6).clamp(1, 3)`, not threads: worst case is that many libde265 pools, plus the global-pool scan (`scan.rs:132-135`) and up to four thumbnail decodes (`src/App.tsx:235`). |
| Q13 | Is dynamic plugin loading reachable at runtime with the codecs compiled in? | Archive inventory; `heif_library.h:122-125`, `181-185` | Yes, and compiled-in codecs do not close it. `heif_init` calls `register_default_plugins` and then `heif_load_plugins` per directory from `get_plugin_paths` (relocations in `init.cc.o`, quoted in §7), and `plugins_unix.cc.o` imports `dlopen`/`dlsym`/`dlclose`/`dlerror` (`nm -A` on the extracted `lib/libheif.a`). `get_plugin_paths` reads only the environment variable `LIBHEIF_PLUGIN_PATH` (the `getenv` call plus `.rodata.str1.1` offset `0x3c`), while the header documents `LIBHEIF_PLUGIN_PATHS` (`:181-185`), and no built-in directory string is compiled in (`strings -a | grep -c "/usr/"` → `0`). So: reachable by a process that can set that variable and write a `.so` into the named directory — loading runs its constructors in-process — but not by a HEIC file, and nothing here sets the variable, so the scan surface is empty by default. The `libx265.so`/`libx265_main10.so`/`libx265_main12.so` strings are a separate mechanism: they live in x265's own `api.cpp.o`, whose `heifrs_x265_api_get_217` imports `dlopen`/`dlsym`; they are x265's 8/10/12-bit library names, and the seam only ever asks for 8-bit (`heic.rs:128`). |

<!-- Q14-Q17: later task -->

