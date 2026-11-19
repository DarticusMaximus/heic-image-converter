# Build, test, and release

## Prerequisites

- **Rust** 1.85+ (edition 2024). Verified with 1.99.1.
- **Node** 22+ (Vite 8 requires 20.19+/22.12+). Verified with 26.13.1.
- **Tauri v2 system dependencies** for the host OS.
- **C/C++ toolchain** for linking the static codec bundle (the normal Rust
  toolchain is enough on Linux and macOS; Windows needs the MSVC build tools).

Linux (Arch, as used for development):

```bash
sudo pacman -S --needed webkit2gtk-4.1 libsoup3 gtk3 librsvg base-devel
```

Linux (Debian/Ubuntu, as used in CI):

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev \
  librsvg2-dev patchelf libgtk-3-dev libsoup-3.0-dev
```

Windows: WebView2 (present on current Windows 10/11), Visual Studio Build
Tools with the C++ workload. macOS: Xcode command line tools.

## The codec artifact

The codec is a prebuilt **static** archive (libheif + libde265 + x265) per
target, published as release `26.10.1` of
`github.com/vegidio/binaries-heif`. The pin is
`src-tauri/vendor/heif-bundles/manifest.json`: it records all eight target
archives with the archive's byte length and SHA-256, plus the SHA-256 of the
static library inside each one. The manifest is hand-authored; no script
rewrites it.

`heif-rs` itself is vendored at `src-tauri/vendor/heif-rs/` and wired in with
`[patch.crates-io]`. Upstream's build script downloads and extracts the archive
with no checksum, and it runs before our own `build.rs`, so patching the crate is
the only place the check can sit where a plain `cargo build` cannot step around
it. `VENDORED.md` records the delta from the published 26.10.3 source. The crate
requirement is exact (`"=26.10.3"`), so the wrapper cannot resolve to another
version.

What gets verified:

- **Before extraction** — the archive's SHA-256 against the manifest row. A
  mismatch fails the build with the archive name, both hashes, and the
  `scripts/heif-artifacts.sh fetch` command that re-fetches it.
- **Every time the codec build script runs** — the extracted `lib/libheif.a`
  (`lib/heif.lib` on MSVC) is hashed against the manifest's `lib_sha256`, so a
  stale or poisoned cache directory fails instead of being linked. Once cargo
  caches the script's result it does not re-run; `scripts/heif-artifacts.sh
  verify` is the on-demand re-check.

Four archives are committed in the repository — the targets this product ships
and CI builds: Linux x64, macOS x64, macOS arm64, and Windows x64 (about 18 MB
total). For those, the build resolves the archive from
`src-tauri/vendor/heif-bundles/` and never reaches for the network. The other
four (`linux_arm64`, `windows_arm64`, and the two MinGW targets) are hash-pinned
but not committed: the build downloads one from the pinned release on first use
and verifies it before extracting. MSVC targets use the `windows` archive; GNU
targets use `mingw`.

Two subcommands re-check the pin by hand:

```bash
scripts/heif-artifacts.sh verify
scripts/heif-artifacts.sh fetch bundle_linux_arm64.zip
```

`verify` checks each committed archive's byte length and SHA-256, then every
library already extracted under
`src-tauri/target/*/build/heif-rs-*/out/binaries-heif-bundle-*/` against the
manifest. Libraries are matched by hash, because the cache directory is named
for the codec release and not the target — Linux x64 and arm64 both ship
`lib/libheif.a`. A bundle directory that exists but holds none of the manifest's
libraries is a half-extracted cache and fails; only the complete absence of
bundle directories is a cold-cache pass. `verify` exits 1 on any mismatch,
printing expected and actual values plus a `fix:` line.
`fetch <archive>` downloads one archive from the pinned release into
`src-tauri/vendor/heif-bundles/` and keeps it only if its byte length and SHA-256
match; a bad download is deleted and the script exits 1. Neither subcommand
writes the manifest.

`HEIF_BINARIES_DIR` remains the documented local/custom-build override: point it
at a directory containing `include/` and `lib/`, and the build uses it as-is,
skipping the manifest. That directory sits outside the pin — the build checks
only that `include/` and `lib/` exist, and hashes nothing under it — so point the
override only at a codec build you trust.

```bash
HEIF_BINARIES_DIR=/path/to/extracted cargo build --release
```

The Rust and build gates in `scripts/ci.sh` pass `--locked`
(`cargo test --locked`, and `npm run tauri -- build --no-bundle -- --locked`),
so CI resolves only what `Cargo.lock` already records: a requirement change that
would shift the lockfile fails the gate instead of re-resolving.

## Develop

```bash
npm install
npm run tauri dev
```

Dev mode loads the frontend from Vite on `http://localhost:1420` with HMR. The
CSP includes `ws://localhost:1420` and `http://localhost:1420` for that reason;
production assets are served from the app itself.

Seed the queue from the command line:

```bash
npm run tauri dev -- /path/to/photos
```

## Test

```bash
npm test             # frontend unit tests (Vitest + jsdom)
npm run build        # tsc --noEmit equivalent + Vite production build
cd src-tauri && cargo test
```

Rust tests cover the conversion core, path planning, rotation, 8-bit PNG
normalization, estimate math, and the pinned codec manifest. They create real
HEIC/JPEG/PNG files in temporary directories. A headless backend integration
suite covers folder scanning (recursion, hidden and system entries, dedupe, the
20,000-file cap, mixed roots), thumbnail generation, batch conversion with
mirrored output, cancellation through the pre-set flag, and per-file failure
isolation — no window, network, or external tool involved. A corpus integrity
suite also replays the committed images in `src-tauri/tests/fixtures/` against
their manifest.

Frontend tests cover the pure logic in `src/format.ts` and `src/rows.ts`: byte
formatting, target-pixel math, settings load/save/validation, and the
outcome-to-row mapping — 37 tests across 2 files. React components, DOM
interaction, and the UI state layer are not covered yet. `npm run test:watch`
keeps Vitest running and re-runs on change.

Formatting:

```bash
cd src-tauri && cargo fmt --check
```

That check is part of the Rust gate below: unformatted code fails
`scripts/ci.sh rust` and the `rust` CI job.

### Gates

`scripts/ci.sh` is the one entry point CI and a developer machine share. It
resolves the repository root from its own path, so it runs from any directory:

```bash
scripts/ci.sh frontend   # npm test, then npm run build
scripts/ci.sh rust       # cargo fmt --check, then cargo test --locked (in src-tauri)
scripts/ci.sh build      # npm run tauri -- build --no-bundle -- --locked
scripts/ci.sh all        # frontend, then rust, then build; stops at the first failure
```

The script installs nothing and deletes nothing. On a fresh checkout run
`npm ci` first; the script will not do it for you. `--help` lists the
subcommands; a missing or unknown one exits 2.

Three gates, one CI job each:

- **Frontend** — `npm test`, then `npm run build`. A failing expectation fails
  the test half; a `tsc` error fails the build half.
- **Rust** — `cargo fmt --check`, then `cargo test --locked`, both with
  `src-tauri` as the working directory. Formatting is a gate.
- **Release build** — `npm run tauri -- build --no-bundle -- --locked`. The
  binary compiles in release mode; `--no-bundle` means no `.deb`, `.rpm`, or
  AppImage is produced, which keeps the gate to the compile it is meant to
  prove.

`cargo clippy` is not a gate. `cargo clippy --all-targets` currently reports 5
pre-existing warnings in the backend (4 `collapsible_if` in `convert.rs`, 1
`redundant_closure` in `scan.rs`) and exits 0. Those are separate cleanup work;
gating on them now would fail every change until they are fixed.

### Deliberate-break drill

On 2026-11-18 each gate was broken on purpose in a clean copy, run, then
restored. The failure lines are observed output, not predictions:

| Break | Gate | Failure signature | Restored |
|---|---|---|---|
| `format.test.ts` expectation `1.5 KB` → `1.6 KB` | frontend tests | `FAIL  src/format.test.ts > formatBytes > …`, exit 1 | exit 0 |
| new `src/ci-drill-type-error.ts` with `export const drill: number = "not a number";` | frontend build | `error TS2322: Type 'string' is not assignable to type 'number'`, exit 2 | exit 0 |
| `"png"` match arm in `formats.rs` misindented | Rust formatting | `Diff in …/src-tauri/src/formats.rs:23:`, exit 1 | exit 0 |
| `assert!` inverted in the `quality_steps_increase` test | Rust tests | `test formats::tests::quality_steps_increase ... FAILED`, exit 101 | exit 0 |

The TypeScript break was a new file under `src/`, which does not match the
Vitest include pattern: the tests still passed and only `tsc` failed, so the
build half of the frontend gate is isolated. The inverted Rust assertion was
left correctly formatted on purpose, so rustfmt passed and the failure landed on
the test gate.

`all` was checked for short-circuiting too: with a failing `src/stop.test.ts`
added, it exited 1 after `==> Frontend tests` and printed no
`==> Rust formatting` banner — no later gate ran.

The drill ran in a copy made without local build output:

```bash
clean_dir="${CI_CLEAN_DIR:-${TMPDIR:-/tmp}/heic-ci-clean}"
rm -rf "$clean_dir"
rsync -a --exclude .git --exclude node_modules --exclude dist \
  --exclude src-tauri/target ./ "$clean_dir"/
cd "$clean_dir" && npm ci && scripts/ci.sh all
```

`CI_CLEAN_DIR` had to point at a disk-backed path, because `/tmp` on the
development machine is a 16 GB tmpfs with 8.2 GB free. In that copy the gates
passed cold: frontend 1 s, Rust 67 s, release build 236 s. The release build in
the copy produced `src-tauri/target/release/heic-image-converter` (15,093,664
bytes) and no `bundle/` directory.

What that proves and what it does not:

- It proves the gates need no local build artifacts: no `node_modules`, `dist`,
  or `src-tauri/target` were copied in, and both test gates and the release
  build pass from scratch.
- The `rsync` copy alone does not prove the set of committed files is sufficient,
  because it copies untracked files too. The hosted run below does prove it.
- The three-job workflow has now run on GitHub. Commit `469b77d` ("Add the stage
  00 test baseline and verification pass") was pushed to `main` on 2026-11-18 and
  [run 44364382930](https://github.com/DarticusMaximus/heic-image-converter/actions/runs/44364382930)
  went green on all three jobs from a fresh checkout: Frontend 12 s, Release
  build 6 m 16 s, Rust 22 m 32 s (cold). Before that, the previous single-job
  workflow ran once on 2026-11-17 at `972c3d9`; that run exercised the old
  workflow only.
- The release build took 236 s cold locally and 6 m 16 s on the hosted runner.
  The Rust job's cold 22 m 32 s is about 56 % of its 40-minute timeout, so both
  timeouts stand. Watch the Rust margin if the suite grows.

### Regenerating the fixture corpus

`src-tauri/tests/fixtures/` holds 22 synthetic images plus `manifest.json`
(pins each fixture's bytes and SHA-256), `README.md` (the corpus
documentation), and `generate.sh` (the generator). Regenerate only when a
fixture genuinely needs to change:

```bash
(cd src-tauri/tests/fixtures && ./generate.sh)
cd src-tauri
cargo test -- --ignored update_fixture_manifest
cargo test
```

`generate.sh` needs ImageMagick 7 (`magick`), `heif-enc`, `heif-info`, and
`exiv2`. Those are developer-machine tools only: CI never installs or runs
them, and `cargo test` reads the committed fixtures and manifest — no external
tooling, no network access.

## Build installers

```bash
npm run tauri build
```

Outputs (Linux):

```
src-tauri/target/release/heic-image-converter                      # binary
src-tauri/target/release/bundle/deb/HEIC Image Converter_*.deb     # ~6 MB
src-tauri/target/release/bundle/rpm/HEIC Image Converter-*.rpm     # ~6 MB
src-tauri/target/release/bundle/appimage/HEIC Image Converter_*.AppImage  # ~105 MB
```

The AppImage is large because it bundles the GTK/WebKit support libraries it
needs to run on systems without them. The deb/rpm rely on system packages.

Bundle settings live in `src-tauri/tauri.conf.json`: product name, identifier,
window size, CSP, targets, category, license, icons.

## Cross-platform builds

Cross-compiling Tauri from Linux is not practical (WebView2, platform SDKs,
signing). Use the GitHub Actions workflow:

- `.github/workflows/ci.yml` — three independent jobs on `ubuntu-24.04` with no
  `needs` between them, so one failure does not hide the others: `frontend`
  (unit tests and production build), `rust` (`cargo fmt --check` and
  `cargo test --locked`), and `build` (release build with `--no-bundle` and
  `--locked`). Each job installs its own dependencies and delegates to
  `./scripts/ci.sh <gate>`; timeouts are 10, 40, and 60 minutes.
- `.github/workflows/release.yml` — triggered by a `v*` tag or manually.
  Matrix: Ubuntu 24.04, Windows latest, macOS arm64, macOS x64. Uses
  `tauri-apps/tauri-action` to create a draft release with installers.

To cut a release:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Then fill in the draft release on GitHub. Signing/notarization is not
configured (see `DECISIONS.md` D13); users will see SmartScreen/Gatekeeper
warnings until it is.

## Version pins that matter

| Component | Version | Why it matters |
|---|---|---|
| `heif-rs` | 26.10.3 (exact, `=26.10.3`) | HEIC encode/decode, young crate; vendored and patched so the codec artifact is checksum-pinned |
| libheif (bundled) | 1.23.6 | reported by `heif::libheif_version()` |
| `image` | 0.25.10 | all non-HEIC codecs |
| `tauri` | 2.12.6 | shell, capabilities, CSP |
| `@tauri-apps/api` / `cli` | 2.12.5 | IPC + bundler |
| React | 19.3.1 | UI |
| Vite | 8.3.8 | frontend build |

`Cargo.lock` and `package-lock.json` are committed; keep them.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| Blank window in dev, CSP errors in console | CSP is missing the dev localhost origins |
| `heif-rs` fails with a checksum mismatch | A committed archive or an extracted library does not match `manifest.json`; run `scripts/heif-artifacts.sh verify` and follow its `fix:` line |
| `heif-rs: no pinned codec artifact for target …` | The target is not one of the eight in the manifest; `HEIF_BINARIES_DIR` is the escape hatch |
| `webkit2gtk-4.1` not found | Install the Tauri system dependencies |
| HEIC encode fails with an encoder error | x265 build mismatch; the bundled binary is 8-bit only |
| App opens, conversions do nothing | Check the WebView console; command errors surface as toasts |
| Window opens tiled full-screen | Compositor rule, not the app; the config asks for 1100×780 |

## Repository layout

```
.
├── docs/                 architecture, behavior, decisions, build, SSC handoff
├── public/icon.svg       app mark source (favicon)
├── src/                  React frontend
├── src-tauri/
│   ├── capabilities/     Tauri permissions
│   ├── icons/            generated app icons
│   ├── src/              Rust backend
│   ├── tests/fixtures/   22 synthetic fixtures, manifest.json, README.md, generate.sh
│   ├── vendor/heif-rs/   vendored, patched codec wrapper
│   ├── vendor/heif-bundles/  pinned codec manifest + 4 committed archives
│   ├── Cargo.toml
│   └── tauri.conf.json
├── scripts/ci.sh         gate runner shared by local runs and CI
├── scripts/heif-artifacts.sh  verify/fetch the pinned codec artifacts
├── .github/workflows/    CI + release
├── LICENSE               GPL-3.0
└── README.md
```
