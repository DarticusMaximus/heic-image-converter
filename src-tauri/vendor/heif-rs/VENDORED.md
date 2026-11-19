# Vendored `heif-rs`

This directory is a **fork** of the published `heif-rs` crate, vendored so the
codec artifact it links can be checksum-pinned. It is wired into the build with
`[patch.crates-io]` in `src-tauri/Cargo.toml`; the crate is never consumed from
crates.io directly.

## Upstream

| | |
| --- | --- |
| crate | `heif-rs` |
| version | `26.10.3` |
| source | crates.io — <https://crates.io/crates/heif-rs/26.10.3> |
| upstream repo | <https://github.com/vegidio/heif-rs> |
| upstream commit | `f8d7a972c59d5fb3f9a29c9669c377c93a08418a` |
| licence | Apache-2.0 (see `LICENSE`, unmodified) |

Unpacked from the published crate tarball `heif-rs-26.10.3.crate`, whose
SHA-256 `dc5863cecbc91f3b87a0cb508e864f76bafac4b772420ab26b5fa7219c939253`
matches the `cksum` the crates.io index publishes for that version.

## What differs from upstream

**Exactly two published files are edited, and two files are added.** The delta
is the list below and nothing else; every other path in this directory is the
published crate, byte for byte.

### Edited: `Cargo.toml`

The `[build-dependencies]` block gains two entries. Nothing else changes — no
dependency requirement, no feature, no metadata.

```toml
[build-dependencies.serde_json]
version = "1"

[build-dependencies.sha2]
version = "0.10"
```

Both are *build* dependencies of this crate only. They let the build script
hash the codec archive before extracting it and hash the extracted library
afterwards. They do not enter the application's runtime dependency graph.

### Edited: `build.rs`

Replaced. Upstream's build script downloads the codec archive for the current
target with no checksum and extracts it. The replacement keeps upstream's
`main` directives, `emit_link_directives` (including the macOS
`link_compiler_rt_builtins` workaround) and `generate_bindings` — `src/sys.rs`
is `include!(concat!(env!("OUT_DIR"), "/bindings.rs"))`, so the bindgen step is
load-bearing — and changes only where the binaries come from and what the cache
that holds them is trusted for:

1. `HEIF_BINARIES_DIR`, when set, is used as-is (the documented local/custom
   override), after asserting `include/` and `lib/` exist.
2. Otherwise the pinned manifest at `../heif-bundles/manifest.json` is read and
   the entry for `CARGO_CFG_TARGET_OS` / `_ENV` / `_ARCH` is selected.
3. The archive bytes come from the committed zip at
   `../heif-bundles/<archive>` when present, otherwise from the pinned
   release URL for an artifact the manifest marks `committed: false`.
   The SHA-256 is verified **before** extraction, and the extracted
   `<lib>` is verified against `lib_sha256` on a cache hit.
4. Extraction goes into a directory keyed by the archive's own hash,
   `binaries-heif-bundle-<release>-<first 12 hex of the archive sha256>`, so a
   cache hit can only be the extraction of the archive this run verified. The
   release stays in the name for a readable `target/` tree; it carries no
   trust.
5. Every file in the extracted `include/` tree is hashed and compared with the
   same member of the archive *before* bindgen reads it. `heif.h` is only the
   entry point: bindgen parses the 17 sibling headers it includes, so each of
   them is a codegen input and each is checked. A mismatch, a missing file, or
   an extracted header the archive does not carry fails the build naming the
   extracted path, both hashes, and the cache directory to delete. Nothing is
   silently re-extracted: a poisoned header stays visible to whoever needs to
   know, and the archive is the only trusted source for it. `HEIF_BINARIES_DIR`
   is exempt, as it is for every other check.
6. `bindings.rs` is generated from the verified tree on every run of the build
   script and written with a `bindings.rs.sha256` sidecar recording the digest
   of what was written. Both files are written only when their bytes change, so
   an unchanged regeneration does not mark the crate dirty on every build. The
   script emits `cargo:rerun-if-changed` for the `include/` directory (so a
   sibling header edit is an input), for `bindings.rs`, and for the sidecar, so
   an edited or deleted cache file forces a rerun rather than being compiled
   as-is.
7. Feature 03's local-only rule: a `committed: true` archive that is missing
   from `../heif-bundles/` fails the build with the archive name and the
   `scripts/heif-artifacts.sh fetch <archive>` repair command — the pinned
   release URL is never a fallback for it.

A hash mismatch is a hard failure that names the archive, both hashes, and the
command that re-fetches the archive.

### Added: `build_support.rs`

Pure helpers (manifest parse, target selection, URL rendering, hex SHA-256,
checksum verification, cache-key naming, and the include/-tree comparison the
header check uses): no network, no extraction, and no filesystem access —
the build script reads the bytes and hands them in. Compiled twice — by
`build.rs`, and by `src-tauri/src/codec_artifacts.rs` through `#[path = ...]`
so the selection and comparison logic is unit-tested rather than trusted.

### Added: `VENDORED.md`

This file.

### Unchanged (byte-for-byte the published crate)

`src/`, `tests/`, `Cargo.toml.orig`, `Cargo.lock`, `.cargo_vcs_info.json`,
`LICENSE`, `README.md`. Do not edit them: a local fix belongs in a patch that
also bumps the recorded upstream version here, or upstream.

## The codec archives

`build.rs` links one of the prebuilt **bundle** archives published as release
`26.10.1` of <https://github.com/vegidio/binaries-heif>. Every archive's name,
size, SHA-256 and expected library hash is recorded in
`src-tauri/vendor/heif-bundles/manifest.json`, which is the authority. The
committed archives live next to it.

The manifest records no hash for the `include/` tree: the headers are read
from the archive itself, and the archive's own hash is what makes them trusted.
Recording a second set of hashes would only add values to drift.

Each `bundle_*` archive merges libheif, libde265 and x265 into a single static
library; that merge is why the encoder cannot be unlinked separately.

## Upgrading

Do not edit this directory in place. An upgrade re-vendors the crate from the
newly published version and re-applies the two edits above:

1. `cargo add heif-rs@=<new version>` — or bump `src-tauri/Cargo.toml` — so the
   crate is fetched into the local registry.
2. Unpack the new `heif-rs-<version>.crate` tarball over this directory,
   deleting the old contents first so removed files do not linger.
3. Re-apply the `[build-dependencies]` entries to `Cargo.toml`.
4. Re-apply the `build.rs` replacement and carry `build_support.rs` forward
   (or deliberately re-diff it, if upstream's layout moved).
5. Update the upstream table at the top of this file.
6. Update `src-tauri/vendor/heif-bundles/manifest.json` — the `crate.version`,
   and the artifact rows if the codec release moved. A hash is written once,
   from a download that was checked against the release's published digest
   metadata; no script derives it.
7. Rebuild and run `scripts/heif-artifacts.sh verify`.

Steps 3–6 are the edits this file exists to document. Everything else in a
re-vendor is upstream's.
