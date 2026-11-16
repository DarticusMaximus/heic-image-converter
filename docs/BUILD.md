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
npm run build        # tsc --noEmit equivalent + Vite production build
cd src-tauri && cargo test
```

Rust tests cover the conversion core, path planning, rotation, 8-bit PNG
normalization, and estimate math. They create real HEIC/JPEG/PNG files in
temporary directories. There are no frontend tests.

Formatting:

```bash
cd src-tauri && cargo fmt --check
```

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

- `.github/workflows/ci.yml` — frontend build + `cargo test` on Ubuntu.
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
│   ├── tests/fixtures/   rotated HEIC regression fixture
│   ├── Cargo.toml
│   └── tauri.conf.json
├── .github/workflows/    CI + release
├── LICENSE               GPL-3.0
└── README.md
```
