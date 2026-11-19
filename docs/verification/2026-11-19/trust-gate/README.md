# Trust gate and upgrade rehearsal — evidence index, 2026-11-19

This directory is the dated evidence record for stage 01, feature 05 ("Trust
gate"). It holds the clean run of `scripts/ci.sh trust`, the four deliberate
breaks that show the gate has teeth, and the same-version rehearsal of
`docs/UPGRADE-HEIC.md`. Every value below was observed on the machine and at the
commits named; nothing is copied from an older document or from expectation.

## Machine and tools

Taken from the rehearsal's run block (`runbook-rehearsal/README.md`), which is
the machine the gate runs and breaks below were recorded on:

| Field | Value |
| --- | --- |
| Date | 2026-11-19 (S0–S2/S5 from `2026-11-19T17:22Z`; S3/S4/S6–S9 from `2026-11-19T17:42Z`) |
| Machine | Omarchy (Arch-based), hostname `omarchy` |
| OS / kernel | `Linux 7.2.15-1-omarchy x86_64` (`uname -srm`) |
| CPU | 8 logical cores (`nproc`) |
| `rustc` | `rustc 1.99.1 (58cf8f9c3 2026-10-13) (Arch Linux rust 1:1.99.1-1)` |
| `cargo` | `cargo 1.99.1 (5f94df478 2026-08-27) (Arch Linux rust 1:1.99.1-1)` |
| `sha256sum` | `sha256sum (GNU coreutils) 9.12` |
| `tar` | `tar (GNU tar) 1.35` |
| `unzip` | `UnZip 6.00 of 20 April 2009, by Info-ZIP` |
| `curl` | `curl 8.23.0 (x86_64-pc-linux-gnu) libcurl/8.23.0 OpenSSL/3.6.5` |
| `gh` | `gh version 2.104.0 (2026-11-12)`, authenticated as `DarticusMaximus` |
| `python3` | `Python 3.14.8` |
| Gate/rehearsal commits | `8d3a6d8` for the vendor-diff half; `3ade5e0` for the hash half |

## What each file proves

| File | What it proves |
| --- | --- |
| `trust-green.log` | `scripts/ci.sh trust` on the committed tree: four committed archives match `manifest.json`, three extracted `lib/libheif.a` copies match their manifest hashes, the lock entry is `heif-rs` `26.10.3` from `vendor/heif-rs`, and the gate prints `trust gate: PASS` (exit 0). It is the clean control for the breaks. |
| `tamper-archive.log` | Break (a): one byte appended to `bundle_linux_x64.zip` fails the gate with the wrong size and `failed its checksum`, expected/actual values and a `fix:` line (exit 1); `git checkout --` restores it byte for byte. |
| `tamper-library.log` | Break (b): one byte appended to an extracted `target/debug/build/heif-rs-*/out/binaries-heif-bundle-26.10.1/lib/libheif.a` fails the gate naming that path, with the candidate expected hashes, the actual hash, and a `fix:` line (exit 1); deleting the build directory and rebuilding re-extracts the pinned bytes and the gate is green again. |
| `pin-drift.log` | Break (c): the lock's `heif-rs` version set to `26.10.2` fails with `trust gate: heif-rs is locked to 26.10.2; expected 26.10.3` (exit 1). Break (d): the `Cargo.toml` requirement set to `"=26.10.2"` fails the `rust` gate with `error: failed to select a version` (exit 101); the published `"=26.10.1"` variant produces cargo's own `--locked` lock-file refusal (exit 101). Each restore is recorded and the final `cargo build --locked` exits 0. |
| `runbook-rehearsal/README.md` | One row per runbook step S0–S9, with the command, observed output, and whether it produced a diff; the machine table; and its own Proven/Not-proven sections. It records that the crates.io API needs a `User-Agent` header, that S9 is partially observed (the trust gate and its five tests passed; `all`/`offline`/hosted run were not run there), and that no version was changed. |
| `runbook-rehearsal/vendor-diff.txt` | S2: the crates.io tarball URL and checksums, the extraction, `diff -rq`, and the full `diff -u` showing exactly the four expected paths (`build.rs`, `build_support.rs`, `Cargo.toml`, `VENDORED.md`) and no other difference against `src-tauri/vendor/heif-rs/`. |
| `runbook-rehearsal/hash-recheck.txt` | S3/S4: for each of the four committed archives, the downloaded bytes and `sha256` against the manifest and the published release digest, and the extracted `lib_sha256` against the manifest only (the release publishes no library digest); four archives, no mismatch. |

The three break logs and `trust-green.log` are one set: the gate's clean run,
then each single mutation, its failure output, and its restore. The rehearsal
folder is the runbook's own evidence; `docs/UPGRADE-HEIC.md` §8 cites it.

## Full local gate

Observed on the clean committed tree at
`d055f24c1e31579ad1900ec979b24349811d1925` (`git status --porcelain` empty
before and after), from the repo root:

```
scripts/ci.sh all
```

Exit 0, 126 s wall clock. The four gates ran in the committed order — trust
first, then frontend, rust, build — and none was skipped:

| Gate | Observed |
| --- | --- |
| `trust` | `==> Codec trust gate` is the first line of the output; `heif-artifacts: verified 4 committed archives, 3 extracted libraries`; `trust gate: heif-rs 26.10.3 locked from vendor/heif-rs (path package)`; `trust gate: PASS` |
| `frontend` | `Test Files 2 passed (2)`, `Tests 37 passed (37)`; vite build `✓ built in 127ms` |
| `rust` | `cargo fmt --check` silent; `running 88 tests`, `test result: ok. 87 passed; 0 failed; 1 ignored` |
| `build` | `npm run tauri -- build --no-bundle -- --locked` → ``Finished `release` profile [optimized] target(s) in 2m 01s``, `Built application at: src-tauri/target/release/heic-image-converter` |

The offline gate needs a disk-backed working directory on this machine: the
default `/tmp` is a 16 GiB tmpfs with about 5 GiB free, under the gate's 8 GiB
floor. It ran as:

```
CI_OFFLINE_DIR="$HOME/.cache/offline-gate" scripts/ci.sh offline
```

Exit 0, 63 s wall clock. Observed
`offline gate: work dir: /home/darticus/.cache/offline-gate (55 GiB free)`,
`offline gate: isolation: unshare -rn`, and the positive control reaching
`1.1.1.1:443` outside the sandbox while failing inside it. The clone of HEAD
carried no `src-tauri/target`, so every dependency was compiled inside the
sandbox (416 lines beginning `   Compiling`) with `CARGO_NET_OFFLINE=true`, and
cargo reported
``Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 01s``.
The gate ended:

```
offline gate: PASS (HEAD d055f24c1e31579ad1900ec979b24349811d1925, isolation: unshare -rn)
```

The run reported `87 passed; 1 ignored` and no `heif-rs: downloading` line (that
string is an assertion in the gate, so a quiet download would still fail it).
The throwaway clone was removed on exit; `$HOME/.cache/offline-gate` held no
leftover `offline-gate.*` directory afterwards.

The feature-level checks were re-run on the same tree:
`cd src-tauri && cargo test trust_gate` → 5 passed, 0 failed;
`cd src-tauri && cargo fmt --check` → exit 0; `scripts/ci.sh trust` →
`trust gate: PASS`.

Full transcripts of the two gate runs were captured locally while they ran
(`/tmp/opencode/task8/ci-all.log`, `/tmp/opencode/task8/ci-offline.log`) and are
deliberately not committed; the commands above reproduce them.

## Hosted run

**Pending push.** The push to `main` triggers `.github/workflows/ci.yml`, which
runs four jobs: `Frontend`, `Rust`, and `Release build` on every change, and
`Offline build` on every event that is not a pull request. The run has not
happened yet — this evidence was committed locally and not pushed — so no
result, URL, or duration is claimed. The fields below are to be filled from the
run page once the push lands:

- Run URL: pending push
- Triggering commit: pending push (expected to be the commit that lands this
  evidence, a child of `d055f24`)
- Jobs and results: pending push (`Frontend`, `Rust`, `Release build`,
  `Offline build`)
- Durations: pending push

The local runs above are the reference the hosted run gets checked against: same
gates, same order, same `trust gate: PASS` and `offline gate: PASS` lines.

## Boundary

What this evidence does **not** establish:

- **No version bump.** The rehearsal is same-version: `heif-rs` stays `26.10.3`
  and the archive release stays `26.10.1`. The steps a real bump exercises —
  reading a new release, re-applying edits to a changed `build.rs`, a
  red-then-green source review, changed notices/licenses — were not run. A real
  bump remains unproven end to end.
- **The registry-source diff does not run in CI.** The `diff -r` against the
  published crate exists only in `runbook-rehearsal/vendor-diff.txt`, once, at
  one version. CI can catch archive bytes, extracted-library hashes, and the lock
  entry; it cannot catch a vendored tree that drifted from its published
  tarball.
- **The four uncommitted archives were not byte-inspected.** Only the four
  committed shipping archives were downloaded, hashed, and compared. The other
  four (`bundle_linux_arm64`, `bundle_windows_arm64`, `bundle_mingw_x64`,
  `bundle_mingw_arm64`) had their manifest sizes and digests checked against the
  release metadata at S1, which is a record check; no archive was downloaded and
  no `lib_sha256` was computed for them.
- **The trust gate does not cover the packaged artifact.** `THIRD-PARTY-NOTICES.md`
  and `licenses/` are not inside any bundle, and nothing here inspects a `.deb`,
  `.rpm`, or AppImage. Stage 04 owns packaging; the gate covers committed
  archives, extracted libraries, and the lock entry only.
