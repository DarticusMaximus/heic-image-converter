# HEIC Image Converter

A clean, fast, offline desktop app for converting images — built because
converting HEIC photos should not require uploading your camera roll to a
website that swears it only touches them "client side".

HEIC ⇄ JPEG ⇄ PNG. One file, a shortlist, or an entire folder tree. No
accounts, no cloud, no telemetry.

![Screenshot](docs/screenshot.png)

## Why this exists

- **HEIC is everywhere and nothing opens it.** Free GUI converters either
  don't exist, are Windows-only, or are buried in a shareware installer.
- **Online converters want your files.** Even the "client-side" ones are a
  promise you cannot verify. This app never opens a socket.
- **CLI tools are fine if you already live in a terminal.** Most people don't.

## Features

- Convert **HEIC/HEIF to JPEG or PNG**, and **JPEG/PNG/WebP/TIFF/BMP/GIF to
  HEIC** (or to each other).
- Drop **single files, a selection, or whole folders** — subfolders are walked
  recursively and the folder structure is mirrored on output.
- **Quality in plain language**: Smallest, Balanced, High, Maximum. The codec
  number is an implementation detail; you pick the trade-off.
- **Honest size estimates**: the app actually encodes a few of *your* images at
  the chosen settings and extrapolates by pixel count. Shown before you commit,
  with the exact size after.
- Optional **resize** (longest edge), preserve-subfolders output, and safe
  naming — it never silently clobbers an existing file.
- **EXIF orientation is respected**, so portrait phone photos don't come out
  sideways.
- Runs on **macOS, Windows and Linux** with the same Rust pipeline. No
  ImageMagick, no Homebrew, no "install these 4 codecs first".

## Install

Grab a build from [Releases](https://github.com/DarticusMaximus/heic-image-converter/releases)
(`.dmg`, `.msi`/`.exe`, `.AppImage`/`.deb`).

On Arch, build from source with `npm run tauri build` (see below) or wrap the
release artifacts in a PKGBUILD.

## Build from source

Prerequisites: [Rust](https://rustup.rs), Node 20+, and the
[Tauri v2 system dependencies](https://v2.tauri.app/start/prerequisites/) for
your platform (on Arch: `webkit2gtk-4.1 libsoup3 gtk3 librsvg`).

```bash
npm install
npm run tauri dev       # development window with hot reload
npm run tauri build     # installers in src-tauri/target/release/bundle
```

The first Rust build downloads prebuilt static libraries for **libheif,
libde265 and x265** from
[vegidio/binaries-heif](https://github.com/vegidio/binaries-heif) — an
internet connection is needed once. For offline builds, download the archive
for your target and set `HEIF_BINARIES_DIR=/path/to/extracted/libs`.

### Windows and macOS

Cross-compiling a Tauri app from Linux is not practical (WebView2, signing,
platform SDKs). Push a tag and let
[`.github/workflows/release.yml`](.github/workflows/release.yml) build all
three platforms on GitHub's runners, or build natively on the target OS.

## How it works

```
React + TypeScript (Vite)  ──invoke──▶  Rust / Tauri commands
                                            │
                              image crate ──┤  JPEG, PNG, WebP, TIFF, BMP, GIF
                              heif-rs ──────┘  HEIC/HEIF via libheif + libde265/x265
```

- `src-tauri/src/scan.rs` expands drops, filters hidden files, probes
  dimensions, and renders 128 px thumbnails.
- `src-tauri/src/decode.rs` decodes HEIC (libheif) or anything else (`image`)
  and applies EXIF orientation where the container doesn't already have it.
- `src-tauri/src/convert.rs` resizes, encodes, picks a collision-free output
  path, and reports progress per file over a Tauri event. PNG output is always
  8-bit, so a 10-bit HEIC does not silently double its file size.
- `src-tauri/src/estimate.rs` sample-encodes up to three images and returns
  bytes-per-pixel for the UI's estimate.
- Conversion runs on a bounded thread pool sized per output format; HEIC gets
  fewer workers because x265 already uses every core. Dimensions are probed in
  parallel during a scan.

## Privacy

All processing is local. The app makes no network requests at runtime and
ships with a Content Security Policy that only permits its own assets. It
deliberately does **not** copy EXIF metadata (including GPS location) to the
output.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

The strong copyleft is a direct consequence of bundling **x265** for HEIC
encoding, which is GPL-licensed. If you need a permissively licensed build,
remove the `Heic` output variant and the `heif-rs` encoder side.
