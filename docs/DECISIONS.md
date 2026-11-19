# Decision log

> Decisions made while building the prototype, with the reasoning and what each
> one costs. Written for a review pass: challenge any of these, but do it with
> the consequences in view.

---

## D1 — Tauri v2 with React, TypeScript, Vite

**Context.** Requirement: one app, installable on Windows, macOS, and Linux,
with a GUI simple enough for someone who does not use a terminal.

**Decision.** Tauri v2 shell; React 19 + TypeScript + Vite frontend. The PM
picked React over Svelte and vanilla TS when asked.

**Consequences.** ~6 MB Linux installers, native webview instead of a bundled
Chromium. Rust owns all image work. React adds a dependency tree and a build
step but keeps the UI state predictable and the ecosystem familiar.

**Alternatives.** Electron (ship 100 MB+, simpler to hire for), Flutter
(excellent UI, weaker native file/drag-drop ergonomics on Linux), native Qt
(more work for the same result).

**Revisit if.** Webview inconsistencies across platforms become a real support
burden, or the UI grows enough to justify a component library.

---

## D2 — `heif-rs` with statically linked libheif/libde265/x265

> **Superseded by Update 2026-11-18.**

**Context.** HEIC is the whole point. The Rust ecosystem has no mature pure-Rust
HEVC codec. Options: bind to system libheif, shell out to a CLI, bundle a
sidecar, or use a WASM decoder in the webview.

**Decision.** `heif-rs` 26.10.3, which downloads prebuilt **static** libheif +
libde265 + x265 at build time and links them into the binary. No runtime
dependency, no sidecar, identical code path on all platforms.

**Consequences.** Cross-compilation-independent: the crate publishes archives
for Linux, macOS, Windows, x64 and arm64, and MSVC vs MinGW. First build needs
network access; offline builds use `HEIF_BINARIES_DIR`. The crate is young
(one maintainer, small download count, versioning by date) — a supply-chain and
longevity risk worth watching. The prebuilt x265 is 8-bit, so HEIC output is
8-bit (fine for this product).

**Alternatives.** `libheif-rs` + system libheif (users would need codecs
installed, a dealbreaker on Windows); bundling ImageMagick or `heif-convert`
(huge, awkward, license murk); `libheif-js`/WASM in the webview (slow for
multi-megapixel photos, memory pressure).

**Revisit if.** We need 10/12-bit HEIC output, the crate stops being
maintained, or we want permissive licensing (then the encoder has to go).

**Update 2026-11-18.** The build-time download described above no longer
happens. `heif-rs` 26.10.3 is vendored at `src-tauri/vendor/heif-rs` and pinned
exactly, and the codec archives are pinned by the committed
`src-tauri/vendor/heif-bundles/manifest.json`, which records each archive's
length and SHA-256 and makes the build refuse a mismatch. Four archives ship in
the repository, so an offline build uses those committed bytes through
`scripts/ci.sh offline`; `HEIF_BINARIES_DIR` survives only as a local override
for a custom codec build. The crate is still young — one maintainer, small
download count, versioning by date — so the longevity risk stands. What changed
is that the bytes are pinned and the code has been read:
`docs/HEIC-SOURCE-REVIEW.md` is the current record of that reading and of which
risks are accepted, watched, or owed to stage 04.

---

## D3 — GPL-3.0-or-later for the whole app

> **Superseded by Update 2026-11-18.**

**Context.** x265 is GPLv2-or-later. Statically linking it into a distributed
binary makes the combined work GPL.

**Decision.** License the application GPL-3.0-or-later. The PM explicitly chose
this over shipping a permissive-license app with a GPL codec (legally gray) or
dropping HEIC encoding.

**Consequences.** Anyone distributing binaries must provide source under GPL.
This is compatible with the "free and open source" goal, but it blocks App
Store distribution without extra work and makes some corporate adoption harder.
A permissive build would need HEIC encoding removed or swapped for a
BSD-licensed encoder (e.g. kvazaar) with its own quality tradeoffs.

**Revisit if.** Store distribution or permissive licensing becomes a
requirement.

**Update 2026-11-18.** The identifiers are now pinned precisely. libheif 1.23.6
and libde265 1.1.3 are `LGPL-3.0-or-later` and are statically linked into the
executable, so the application's GPL-3.0-or-later does not absorb their duty: a
redistributor inherits the LGPL-3.0 §4 relink obligation — the effective means
to relink the application against a modified library, not just a source URL.
x265 is `GPL-2.0-or-later` rather than plain "GPLv2", and it is what makes the
combined work GPL-3.0-or-later. The linked libheif, libde265, and x265 are
symbol-renamed modified builds, so each one's corresponding source — the
upstream version plus the renaming patch — has to travel with any binary.
`THIRD-PARTY-NOTICES.md` is the authoritative record of all of this, including
the honest limit that this repository cannot satisfy the relink and
modified-source duties on its own until the archive's build recipe or patch is
obtained; that gap blocks a public binary release and belongs to stage 04.

---

## D4 — Name: "HEIC Image Converter"

**Context.** The PM dislikes naming and rejected vague options ("Image
Converter") and cute options ("Shrinkwrap", "HEICster").

**Decision.** Literal and searchable: HEIC Image Converter. Identifier
`io.darticus.heicconverter`.

**Consequences.** Excellent discoverability for the core problem, weak brand
and awkward once HEIC becomes one format among many. Renaming later means
changing the bundle identifier (and losing stored settings) — do it before any
public release if it is going to change.

**Revisit if.** The product scope widens beyond HEIC or a brand is desired.

---

## D5 — Four named quality levels instead of a numeric control

**Context.** Requirement: "simplistic quality settings that are easy to
understand." A 0–100 slider is meaningless to most users and maps differently
per codec.

**Decision.** Four presets with plain-language labels and tooltips, mapped
internally to JPEG quality and HEIC quality/preset. PNG hides the control.

**Consequences.** Less control for experts; a single mapping for every kind of
content. The UI never shows a raw codec number. Adding a "custom" escape hatch
later is easy.

**Alternatives.** A slider (precise, confusing), per-format labels (clearer but
inconsistent), bitrate/target-size mode (a bigger feature).

**Revisit if.** Users ask for target file size ("make this under 2 MB") — that
is the more useful form of control and would be a separate mode.

---

## D6 — Estimate by encoding real samples, not a heuristic

**Context.** Users want to know the result size before committing. A static
bits-per-pixel table is fast and often wrong; re-encoding every file just to
estimate defeats the purpose.

**Decision.** Encode up to 3 actual images from the batch at the chosen
settings, compute bytes-per-pixel, extrapolate over the batch's target pixel
count. Show `≈`, state how many images were sampled, and compare to the input
total. Reconcile with exact numbers after conversion.

**Consequences.** Costs 1–3 seconds on settings changes (debounced, cancellable
by sequence number). Accuracy is good for JPEG/HEIC photo mixes, weaker for
PNG where content dominates size. Sampling 3 spread images avoids estimating
from near-identical files.

**Alternatives.** A table (instant, unreliable), a dry-run of every file (too
slow for hundreds of images), no estimate (fails the requirement).

**Revisit if.** Estimating becomes a perceived performance problem on large
batches — a hybrid (sample plus cheap content heuristic) or a target-size mode
may fit better.

---

## D7 — Mirror dropped-folder structure; never overwrite the source

**Context.** Folder conversion of photos from different trips/cameras produces
colliding names. Losing an original is unacceptable. Overwriting an unrelated
file by accident is also unacceptable.

**Decision.** For a dropped folder `X`, output to `<out>/X/<relative path>`.
Single files go flat into `<out>`. Collisions get ` (n)`. The source path is
never a legal target, even with overwrite enabled.

**Consequences.** Predictable, safe, reversible. Duplicate root folder names
from different locations merge into one output tree, with ` (n)` suffixes —
rare, but a known edge.

**Alternatives.** Flat output with suffixes (simpler, messy), prompt per
collision (interrupts batches), always overwrite (dangerous).

**Revisit if.** Users convert two folders with identical names and expect them
kept apart.

---

## D8 — Strip metadata

**Context.** EXIF contains GPS. The product's pitch is privacy. Preserving EXIF
correctly across formats (and HEIC's container model) is real work.

**Decision.** Never copy EXIF/GPS/ICC/XMP. Read EXIF only to apply orientation,
then discard it.

**Consequences.** Smaller files, no accidental location leaks, and a legitimate
privacy feature to advertise. Photographers lose capture dates and camera info;
wide-gamut images lose their color profile. This is a product policy, not a
technical limitation, and should be an explicit toggle eventually.

**Alternatives.** Preserve by default (privacy risk), preserve everything
except GPS (better default for pros, more implementation work).

**Revisit if.** The target audience includes photographers/archivists.

---

## D9 — Normalize PNG output to 8-bit

**Context.** During testing, a 12-bit HEIC produced a 33 MB PNG. libheif decodes
high-bit-depth HEIC to 16-bit pixels; the PNG encoder faithfully preserved them.
No consumer screen shows the difference, and the size doubles.

**Decision.** Downshift PNG output to 8-bit (grayscale preserved, alpha
preserved). Document it.

**Consequences.** Predictable file sizes for a consumer tool. True HDR/10-bit
workflows lose precision. A future "keep 16-bit" option can be added without
changing the default.

**Alternatives.** Preserve bit depth (surprising sizes), fail on high-bit-depth
input (hostile).

---

## D10 — All pixel work in Rust, none in the webview

**Context.** Tauri makes it easy to do image work in JS/WASM. It also makes it
easy to freeze the UI.

**Decision.** The frontend only orchestrates: paths, settings, progress, and
presentation. Decoding, resizing, encoding, and estimation happen in Rust,
off the UI thread, via `spawn_blocking`.

**Consequences.** Consistent results and performance, no giant base64 payloads
(except 128 px thumbnails), testable core. The IPC boundary needs careful
typing (mirrored manually in `src/types.ts`).

**Revisit if.** We want live previews of quality settings — then a small
preview encode endpoint is the right compromise.

---

## D11 — Settings in webview localStorage

**Context.** Small set of user preferences; no accounts, no sync.

**Decision.** Persist `{format, quality, maxDimension, outputDir, overwrite}` in
`localStorage` under `heic-converter.settings.v1`.

**Consequences.** Zero backend work, survives restarts. Not portable across
machines, tied to the bundle identifier, invisible to support, and easy to
corrupt with a bad write (reads are validated with fallbacks). A JSON config
file would be more inspectable.

**Revisit if.** We add presets, CLI defaults, or enterprise deployment settings.

---

## D12 — One image per HEIC container

**Context.** HEIC can hold bursts, depth maps, thumbnails, and auxiliary
images. Handling all of them is a product of its own.

**Decision.** Convert the primary image only, using the crate's default handle.

**Consequences.** Correct for the common iPhone photo case. Burst shots and
portrait depth data are silently dropped. Needs to be stated in the UI or docs
before release.

**Revisit if.** Users report missing burst frames or want depth data.

---

## D13 — No signing, notarization, or auto-update yet

**Context.** The request was to make the app cross-platform, not to run a
release pipeline.

**Decision.** Produce unsigned bundles; provide a tag-driven GitHub Actions
workflow that builds all three platforms. No updater plugin, no certificates.

**Consequences.** Windows SmartScreen and macOS Gatekeeper will warn users.
Manual downloads only. Signing/notarization is a separate, credential-heavy
workstream.

**Revisit before any public release.**

---

## D14 — Command-line paths double as "Open with" support

**Context.** Testing the UI without mouse automation needed a way to seed the
queue; users benefit from the same mechanism via file-manager "Open with".

**Decision.** `startup_paths` returns existing non-flag argv entries; the
frontend adds them on mount, deduplicated.

**Consequences.** Free integration with desktop file managers once a `.desktop`
entry associates file types. No single-instance handling: a second launch opens
a second window (two conversions could run concurrently in separate processes).

**Revisit if.** We register as the default HEIC handler — then single-instance
behavior matters.

---

## D15 — Plan all output paths before the batch starts

**Context.** Parallel workers choosing collision-free names independently can
race: two files with the same target name both see it as free and write to it.

**Decision.** Resolve every target path sequentially up front, reserving names
in a set, then pass the planned path to each worker.

**Consequences.** Deterministic naming, no races, negligible cost. Adding a
file mid-conversion is blocked, which keeps the plan valid.

---

## Open questions for the PM

1. Is metadata preservation a feature or a non-goal? (Affects D8 and the
   privacy pitch.)
2. Is HEIC *output* actually required long-term, given the GPL constraint?
3. Should the app own the `.heic` file association (D14, single-instance)?
4. What is the realistic batch ceiling — 50 photos, 5,000? (Affects D6, D15,
   and whether a job queue with resume is needed.)
5. Are Windows and macOS first-class from day one, or is Linux the only target
   until the design settles?
