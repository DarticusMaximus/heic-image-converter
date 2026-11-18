# Fixture corpus

Small, synthetic images that the Rust test suite decodes and converts. Every
file is drawn locally by `generate.sh` from solid colours: nothing is
downloaded, no photograph or third-party asset is involved, and the corpus
carries no license obligations beyond this repository's own. The generator is
deterministic enough that re-running it reproduces the same bytes, which is why
the manifest hashes stay stable.

`manifest.json` is the contract and the single source of truth. For each fixture
it records the format, the tags that justify the fixture's place, a one-sentence
`about`, the generated `bytes` and `sha256`, the per-channel source bit depth,
EXIF orientation or frame count where relevant, the decoded
dimensions/colour/corners, and the expected PNG and JPEG output. The integrity
suite in `src-tauri/src/fixture_corpus.rs` reads the manifest and fails when the
directory contents, a file hash, a decode fact, a conversion output, the
coverage of formats and awkward cases, or the generated table below drifts from
it.

Each fixture is at most 32 KiB and the corpus totals at most 64 KiB, counting
fixtures only — this README, `manifest.json`, and `generate.sh` are not part of
the budget.

## Corner convention

Corners are sampled from the *decoded* image — after EXIF orientation and HEIC
container rotation have been applied — at top-left, top-right, bottom-left, and
bottom-right, in that order. Each corner is 8-bit RGBA; 12- and 16-bit inputs are
converted to 8-bit RGBA before sampling, so a corner is always four bytes.

The authoring palette is `R` = `#e02020`, `G` = `#20c020`, `B` = `#2040e0`,
`W` = `#f0f0f0`, the grays `#202020` / `#606060` / `#a0a0a0` / `#e0e0e0`, and
fully transparent `#00000000`. The two corrupt fixtures have no palette: they
exist to prove that undecodable input fails with a clear error instead of
producing partial output.

Corner comparisons are per-channel absolute differences and must be at most the
manifest's `corner_tolerance`, which is **8**. That margin absorbs the measured
drift — ±1–2 for JPEG and HEIC, ±4 for WebP at this quality — while staying
small enough that one palette entry cannot be mistaken for another. The 8-bit
RGBA palette and this tolerance are asserted by the suite, so widening the
tolerance or recording `16-bit` corners cannot quietly weaken every check.

## Generated table

The table below is rendered from `manifest.json` between the two marker
comments. Do not edit it by hand: `readme_table_is_current` fails when the block
disagrees with the manifest, and the ignored updater rewrites exactly this block
while leaving the prose untouched.

Corners render as `#rrggbb` when alpha is 255 and as `#rrggbbaa` otherwise.
`Bytes` is the generated file size; `Bits` is the per-channel source depth;
`Decoded`, `PNG out`, and `JPEG out` are dimensions plus the colour type where
the manifest records one; `Extras` carries the EXIF orientation or frame count;
and `Tags` are the manifest tags. An em dash (`—`) means the manifest records
nothing for that cell — a corrupt fixture has no decoded facts or outputs.

<!-- BEGIN FIXTURE TABLE -->
| File | Kind | Bytes | Bits | Decoded | Corners (TL/TR/BL/BR) | PNG out | JPEG out | Extras | Tags |
| --- | --- | ---: | ---: | --- | --- | --- | --- | --- | --- |
| `photo.png` | png | 216 | 8 | 40x30 Rgb8 | #e02020 #20c020 #2040e0 #f0f0f0 | 40x30 Rgb8 | 40x30 | — | `format-matrix` |
| `alpha.png` | png | 202 | 8 | 40x30 Rgba8 | #e02020 #20c020 #2040e0 #00000000 | 40x30 Rgba8 | 40x30; TL #e02020; TR #20c020; BL #2040e0; BR #ffffff | — | `format-matrix`, `alpha` |
| `gray.png` | png | 181 | 8 | 40x30 L8 | #202020 #606060 #a0a0a0 #e0e0e0 | 40x30 L8 | 40x30 | — | `format-matrix`, `grayscale` |
| `deep16.png` | png | 244 | 16 | 40x30 Rgb16 | #e02020 #20c020 #2040e0 #f0f0f0 | 40x30 Rgb8 | 40x30 | — | `format-matrix`, `high-bit-depth` |
| `anim.gif` | gif | 213 | 8 | 40x30 Rgba8 | #e02020 #e02020 #e02020 #e02020 | 40x30 Rgba8 | 40x30 | 3 frames | `format-matrix`, `animated-gif` |
| `photo.webp` | webp | 214 | 8 | 40x30 Rgb8 | #e02020 #20c020 #2040e0 #f0f0f0 | 40x30 Rgb8 | 40x30 | — | `format-matrix` |
| `photo.tiff` | tiff | 338 | 8 | 40x30 Rgb8 | #e02020 #20c020 #2040e0 #f0f0f0 | 40x30 Rgb8 | 40x30 | — | `format-matrix` |
| `photo.bmp` | bmp | 3654 | 8 | 40x30 Rgb8 | #e02020 #20c020 #2040e0 #f0f0f0 | 40x30 Rgb8 | 40x30 | — | `format-matrix` |
| `photo.heic` | heic | 560 | 8 | 40x30 Rgb8 | #e02020 #20c020 #2040e0 #f0f0f0 | 40x30 Rgb8 | 40x30 | — | `format-matrix` |
| `deep12.heic` | heic | 553 | 12 | 40x30 Rgb16 | #e02020 #20c020 #2040e0 #f0f0f0 | 40x30 Rgb8 | 40x30 | — | `high-bit-depth` |
| `burst.heic` | heic | 690 | 8 | 40x30 Rgb8 | #e02020 #20c020 #2040e0 #f0f0f0 | 40x30 Rgb8 | 40x30 | — | `multi-image-heic` |
| `rotated.heic` | heic | 634 | 8 | 120x160 Rgb8 | #2040e0 #e02020 #f0f0f0 #20c020 | 120x160 Rgb8 | 120x160 | — | `heic-rotation` |
| `orientation-1.jpg` | jpeg | 675 | 8 | 40x30 Rgb8 | #e02020 #20c020 #2040e0 #f0f0f0 | 40x30 Rgb8 | 40x30 | orientation 1 | `format-matrix`, `exif-orientation` |
| `orientation-2.jpg` | jpeg | 675 | 8 | 40x30 Rgb8 | #20c020 #e02020 #f0f0f0 #2040e0 | 40x30 Rgb8 | 40x30 | orientation 2 | `format-matrix`, `exif-orientation` |
| `orientation-3.jpg` | jpeg | 675 | 8 | 40x30 Rgb8 | #f0f0f0 #2040e0 #20c020 #e02020 | 40x30 Rgb8 | 40x30 | orientation 3 | `format-matrix`, `exif-orientation` |
| `orientation-4.jpg` | jpeg | 675 | 8 | 40x30 Rgb8 | #2040e0 #f0f0f0 #e02020 #20c020 | 40x30 Rgb8 | 40x30 | orientation 4 | `format-matrix`, `exif-orientation` |
| `orientation-5.jpg` | jpeg | 675 | 8 | 30x40 Rgb8 | #e02020 #2040e0 #20c020 #f0f0f0 | 30x40 Rgb8 | 30x40 | orientation 5 | `format-matrix`, `exif-orientation` |
| `orientation-6.jpg` | jpeg | 675 | 8 | 30x40 Rgb8 | #2040e0 #e02020 #f0f0f0 #20c020 | 30x40 Rgb8 | 30x40 | orientation 6 | `format-matrix`, `exif-orientation` |
| `orientation-7.jpg` | jpeg | 675 | 8 | 30x40 Rgb8 | #f0f0f0 #20c020 #2040e0 #e02020 | 30x40 Rgb8 | 30x40 | orientation 7 | `format-matrix`, `exif-orientation` |
| `orientation-8.jpg` | jpeg | 675 | 8 | 30x40 Rgb8 | #20c020 #f0f0f0 #e02020 #2040e0 | 30x40 Rgb8 | 30x40 | orientation 8 | `format-matrix`, `exif-orientation` |
| `truncated.jpg` | jpeg | 200 | — | decode error | — | — | — | — | `truncated` |
| `garbage.png` | png | 10 | — | decode error | — | — | — | — | `corrupt` |
<!-- END FIXTURE TABLE -->

## Hash pinning

Every entry pins the fixture's `bytes` and `sha256`. The
`fixtures_match_recorded_bytes_and_hash` test recomputes both from disk, so any
change to a fixture file — an intentional regeneration, a one-byte edit, or a
stray checkout conversion — fails `cargo test` until the hashes are refreshed.
That is deliberate: a fixture is test input, and changing it without refreshing
the pin means the suite is testing something nobody recorded.

Refresh the pins only after a change you intended, with the ignored updater:

```sh
cargo test -- --ignored update_fixture_manifest
```

The updater rewrites only `bytes`, `sha256`, and the generated table block. It
never creates the README, never edits the prose, and never rewrites `about`,
tags, dimensions, corners, or outputs — those are hand-authored expectations. If
a regenerated fixture really does decode differently, that is a finding to
resolve deliberately (adjust the fixture or the expectation and say why), never
a value to auto-fit. The updater also refuses to run when the fixture directory
and the manifest list different files.

## Regenerating the corpus

Prerequisites, on a developer machine only: ImageMagick 7 (`magick`),
`heif-enc`, `heif-info`, and `exiv2`. CI never runs the generator and never
installs these tools; it only reads the committed fixtures and their manifest,
so `cargo test` needs no external tooling and no network access.

```sh
cd src-tauri/tests/fixtures
./generate.sh

# back in src-tauri/
cargo test -- --ignored update_fixture_manifest
cargo test
```

`generate.sh` fails if `heif-info` reports the wrong bit depth, image count, or
dimensions for a HEIF fixture, so a broken corpus cannot be committed silently.
The generator writes every fixture, including `rotated.heic`; the app must still
decode it to 120x160 with the container rotation applied.

## Format aliasing

`.heic`, `.heif`, and `.hif` all map to the same `InputKind::Heic` through one
match arm in `InputKind::from_path`. `detects_input_kinds` asserts the `.heic`
and `.heif` spellings; `.hif` shares that arm but has no separate assertion. A
duplicate `.heif` or `.hif` fixture would cost bytes without exercising a path
the corpus does not already prove, so the corpus keeps a single HEIC extension.
