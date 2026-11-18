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

## The codec binary download

`heif-rs` fetches a prebuilt **static** archive from
`github.com/vegidio/binaries-heif` during the first build and caches it under
`OUT_DIR`. That means:

- The first `cargo build` needs internet access.
- CI runners re-download unless the Rust cache covers `OUT_DIR`; our cache
  configuration caches the target directory, which is enough in practice.
- Offline builds: download the matching `bundle_<os>_<arch>.zip`, extract it,
  and point `HEIF_BINARIES_DIR` at the directory containing `include/` and
  `lib/`:

```bash
HEIF_BINARIES_DIR=/path/to/extracted cargo build --release
```

Platform archive names: `bundle_linux_x64`, `bundle_osx_arm64`,
`bundle_windows_x64`, `bundle_mingw_x64`, plus arm64 variants. MSVC targets use
the `windows` archive; GNU targets use `mingw`.

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
normalization, and estimate math. They create real HEIC/JPEG/PNG files in
temporary directories. A headless backend integration suite covers folder
scanning (recursion, hidden and system entries, dedupe, the 20,000-file cap,
mixed roots), thumbnail generation, batch conversion with mirrored output,
cancellation through the pre-set flag, and per-file failure isolation — no
window, network, or external tool involved. A corpus integrity suite also
replays the committed images in `src-tauri/tests/fixtures/` against their
manifest.

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
scripts/ci.sh rust       # cargo fmt --check, then cargo test (in src-tauri)
scripts/ci.sh build      # npm run tauri -- build --no-bundle
scripts/ci.sh all        # frontend, then rust, then build; stops at the first failure
```

The script installs nothing and deletes nothing. On a fresh checkout run
`npm ci` first; the script will not do it for you. `--help` lists the
subcommands; a missing or unknown one exits 2.

Three gates, one CI job each:

- **Frontend** — `npm test`, then `npm run build`. A failing expectation fails
  the test half; a `tsc` error fails the build half.
- **Rust** — `cargo fmt --check`, then `cargo test`, both with `src-tauri` as
  the working directory. Formatting is a gate.
- **Release build** — `npm run tauri -- build --no-bundle`. The binary compiles
  in release mode; `--no-bundle` means no `.deb`, `.rpm`, or AppImage is
  produced, which keeps the gate to the compile it is meant to prove.

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
- It does not prove the set of committed files is sufficient. `rsync` copies
  untracked files too, so the copy is artifact-free, not a git checkout.
- No run of the new three-job workflow has been observed: it is uncommitted, so
  GitHub has not run it. The previous single-job workflow did run once on
  GitHub — a successful push run on 2026-11-17 at commit `972c3d9` — so the
  repository does have an `origin` remote
  (`https://github.com/DarticusMaximus/heic-image-converter.git`), but that run
  predates this feature and exercised the old workflow only. Every result above
  is local.
- The 236 s release build is a cold build on the development machine, about a
  tenth of the 45-minute review threshold, so the `build` job's 60-minute
  timeout stands. A GitHub `ubuntu-24.04` runner compiling the same dependency
  graph will be slower.

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
  `cargo test`), and `build` (release build with `--no-bundle`). Each job
  installs its own dependencies and delegates to `./scripts/ci.sh <gate>`;
  timeouts are 10, 40, and 60 minutes.
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
| `heif-rs` | 26.10.3 | HEIC encode/decode, prebuilt binaries, young crate |
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
| `heif-rs` build fails downloading | No network; use `HEIF_BINARIES_DIR` |
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
│   ├── Cargo.toml
│   └── tauri.conf.json
├── scripts/ci.sh         gate runner shared by local runs and CI
├── .github/workflows/    CI + release
├── LICENSE               GPL-3.0
└── README.md
```
