# Hardening evidence — 2026-11-19

Dated evidence record for stage 01, feature 06 ("Hardening"), Task 11. It holds
re-runs of the mutation drills for Tasks 1 (S1, codegen inputs), 4 (C2, bash
3.2), 7 (T3, `--target`/`CARGO_TARGET_DIR`) and 10 (X1, offline overrides) on
the feature-06 working tree, each transcript in the stage's existing style:
header with HEAD and tree state, every command echoed with `$`, its real output,
and an explicit `[exit N]` line.

The feature-06 diff was uncommitted while these ran. HEAD is the last committed
state; the working tree carries the feature. Nothing here is committed.

## Machine and tools

| Field | Value |
| --- | --- |
| Date | 2026-11-19, 17:25–17:42 local (`America/New_York`) |
| Machine | Omarchy (Arch-based), hostname `omarchy` |
| OS / kernel | `Linux 7.2.15-1-omarchy x86_64` (`uname -srm`) |
| CPU | 8 logical cores (`nproc`) |
| `rustc` | `rustc 1.99.1 (58cf8f9c3 2026-10-13) (Arch Linux rust 1:1.99.1-1)` |
| `cargo` | `cargo 1.99.1 (5f94df478 2026-08-27) (Arch Linux rust 1:1.99.1-1)` |
| `node` / `npm` | `v26.13.1` / `11.20.0` |
| System `bash` | `GNU bash, version 5.3.20(1)-release (x86_64-pc-linux-gnu)` |
| bash 3.2 used for the portability log | `GNU bash, version 3.2.57(2)-release (x86_64-unknown-linux-gnu)`, locally built at `/tmp/bash32/bash-3.2.57/bash` |
| `python3` | `Python 3.14.8` |
| `sha256sum` | `sha256sum (GNU coreutils) 9.12` |
| HEAD | `b86cb998b364a90b6d126d63d8c160c7c65e199b` (`b86cb99`, "Record the green four-job hosted run") |

## Tree state

`git status --porcelain` shows 19 modified paths and the new untracked
`docs/verification/2026-11-19/hardening/` (the four `.log` files are themselves
hidden by `.gitignore`'s `*.log`). The tree the re-captured
`header-and-bindings-tamper.log` certifies is:

```
$ git diff | sha256sum
25c988494ebcac0ec4ab07a3ed69e248032b397ec01702979c96dcfb9e45eafe  -
```

The re-captured log's closing `git status` also prints
`?? docs/verification/2026-11-19/hardening/`. That is this README: unlike the
four `.log` files it is not ignored, so a draft of it existed, untracked, at
17:39 when the log was captured and was rewritten at 17:42. Nothing else in the
directory is un-ignored.

The other three logs were recorded earlier, on a tree that differs only by the
Task-11 edits below:

```
$ git diff | sha256sum
d32f490076d895a75cab2fc132d461ee4b3adbe7f2fc402bce62f8ba3d3cd797  -
```

`d32f4900…` names a tree that no longer exists and cannot be reconstructed: it
predates the two doc corrections and the lint fix, and no copy of it was kept.
It is the hash those three logs printed themselves. What makes it usable as
provenance is narrower: their closing `git status` lists show 18 modified paths
and no `docs/ARCHITECTURE.md`, so they predate both doc edits; and the only
tracked files their subjects read — `scripts/ci.sh`, `scripts/heif-artifacts.sh`,
the manifest and the four archives — carry the same bytes in that tree as in the
final one. The one tree whose hash could be reproduced, the tamper log's, is
`25c98849…`.

The delta between the `d32f4900…` tree and the final one is two doc truth
corrections and one lint fix:

1. `docs/ARCHITECTURE.md` — the snapshot's `Test count` row no longer restates a
   total (`71 Rust tests … 37 frontend tests`) that had drifted past the suite;
   it now points at what `cargo test` and `npm test` report.
2. `docs/BUILD.md` — the hosted-run sentence said the four-job rerun was
   "pending" after run `44587086777`; the rerun `44591752926` is green on all
   four jobs and `b86cb99` records it, so the sentence now links both runs.
3. The lint fix — `heic.rs`'s `PANIC_LOG` uses a `const` initializer, and
   `trust_gate.rs`'s `command_step` is one boolean condition plus
   `then_some(index)`. Behaviour is unchanged: the Task 2 release-workflow
   mutations still fail for the same reasons, and the Task 6 panic-sink mutation
   still fails. This removes the two warnings the feature had added, returning
   `cargo clippy --locked --all-targets` to its 5 pre-existing warnings.

`docs/BUILD.md`'s clippy sentence was briefly rewritten to "7 warnings" while
the feature still carried them, then reverted to its original 5-warning wording
once the lint fix made it true again. It is byte-identical in the two trees and
is not part of the delta.

The bash-3.2 and offline logs never invoke the Rust application code. The target
log builds the crate only to produce an extraction to corrupt; its result turns
on discovery of `binaries-heif-bundle-*`, not on the bytes of `heic.rs` or
`trust_gate.rs`. Everything those three checks read — the two shell scripts, the
manifest and the four archives — is byte-identical between the two trees, so
the checks and their failure paths are unchanged. What is not stable is what has
been built: `verify` reports the extracted-library count and the per-tree `ok`
lines for whatever exists under `target/` and `CARGO_TARGET_DIR`, and a re-run
today finds 8 libraries where the earlier transcripts record 7 (an extra
`heif-rs-ea96b0ea36cbffda` extraction appeared after more builds). Re-running
those three would exercise the same checks, not print the same lines.

## What each file proves

| File | Supports | Does not support |
| --- | --- | --- |
| `header-and-bindings-tamper.log` | On a clean warm tree, `verify` exits 0. Then each of five mutations makes the on-demand verifier fail naming the file: editing the entry header `include/libheif/heif.h` (`failed its checksum`, exit 1, build exit 101), editing a sibling `heif_image_handle.h` (same), adding an extra header the archive lacks (`is not part of bundle_linux_x64.zip`, exit 1, build 101), editing `out/bindings.rs` (`failed its checksum`, exit 1, rebuild regenerates it and `verify` is green), and deleting `out/bindings.rs.sha256` (`cannot be attested`, exit 1, rebuild writes it again). Every mutation is restored and the tree is green at the end; the final `git diff` hash equals the opening one. | That `src-tauri/target/` as a whole is trusted, or that Cargo's fingerprints, incremental data and final binaries are verified. Only the codegen inputs the build reads — the extracted `include/` tree and cached `bindings.rs` — are attested, and only against the archive the run hash-verified. It does not cover a build that used `HEIF_BINARIES_DIR`. |
| `bash-32-portability.log` | A locally built GNU bash 3.2.57 reports `BASH_VERSION=3.2.57(2)-release`; `bash -n` accepts both scripts; a checkout-like tree with the working-tree `scripts/` and the committed bundle but no `src-tauri/target` runs `verify` to exit 0 with `no extracted bundle directories found — nothing attested (cold cache)` and `verified 4 committed archives, 0 extracted libraries`; the warm tree exits 0 with 7 extracted libraries; a corrupted library exits 1 naming it and the restore is green; `scripts/ci.sh trust` run entirely under 3.2 (both scripts take `bash` from `PATH`) prints `trust gate: PASS`; the throwaway tree (`task11-cold`) is removed. | A real Mac. It is a 3.2-compatible shell on Linux: no Darwin codec archive, no macOS `target/`, no Xcode toolchain. It also runs only the two shell scripts — `cargo`, `rustc`, `find` and `python3` were the host's; the Rust suite was not run under 3.2. |
| `target-and-cargo-target-dir.log` | `verify` finds the extraction under the `--target` layout (`target/<triple>/<profile>/build/...`), fails with exit 1 when that library is corrupted, and is green after restore. A build with `CARGO_TARGET_DIR=/home/darticus/.cache/task11-alt-target` (brought up cold, `Finished … in 59.11s`) is discovered and scanned **alongside** the default tree (`scanning src-tauri/target` and `scanning /home/darticus/.cache/…`); corrupting its library exits 1 naming that path and with 8 libraries checked. A `CARGO_TARGET_DIR=/nonexistent` is refused with exit 1 and the variable, value and resolved path. A cold tree prints the distinct cold-cache line; a `heif-rs` `out/` with no bundle directory exits 1 (`has no extracted bundle directory`). All three scratch trees this log creates (`task11-alt-target`, `task11-cold2`, `task11-outonly`) are removed and the final tree is green. | Every layout cargo could ever invent, or a cross-compiled `--target` other than the host triple. Three shapes were exercised: default, explicit `--target x86_64-unknown-linux-gnu`, and a redirected tree. It does not prove a *release-profile* `--target` build or a second triple. |
| `offline-overrides.log` | Each of `HEIF_BINARIES_DIR` set, `CARGO_TARGET_DIR` set, and both set makes `scripts/ci.sh offline` exit 1 before the disk check or clone, naming the variable, its value and why a pass would not describe the committed bundle, then printing the one-line remedy and `offline gate: FAIL (exit 1)`. No `offline-gate.*` clone directory exists before or after. The PASS half is a clearly labelled **excerpt** (source SHA-256 recorded) from Task 10's real run on the same working tree at HEAD `b86cb99`: it shows the effective codec source and target lines, the positive control, `87 passed; 1 ignored` in the clone, and `offline gate: PASS (HEAD b86cb998…, isolation: unshare -rn)` with exit 0. | That this Task-11 session ran the offline gate end to end. It did not: the refusal paths were re-run, the PASS is copied from the Task 10 transcript and labelled as such. That PASS also proves HEAD's committed Rust gate, not the uncommitted feature-06 test set — the gate clones HEAD by design. |

## Full local gate

Run after the lint fix and doc corrections, from the repository root on the tree
whose `git diff` hash is `25c98849…`:

```
$ scripts/ci.sh all
```

Exit 0. The four gates ran in order and none was skipped. Millisecond build
timings are omitted: they vary between runs and the full-gate capture is not part
of this directory; the stable results are the counts and the exit status.

| Gate | Observed |
| --- | --- |
| `trust` | first line `==> Codec trust gate`; `heif-artifacts: verified 4 committed archives, 8 extracted libraries`; `trust gate: heif-rs 26.10.3 locked from vendor/heif-rs (path package)`; `trust gate: PASS` |
| `frontend` | `Test Files 2 passed (2)`, `Tests 37 passed (37)`; the production build exited 0 |
| `rust` | `cargo fmt --check` silent; `running 108 tests`, `test result: ok. 107 passed; 0 failed; 1 ignored` |
| `build` | `tauri build --no-bundle -- --locked` compiled and printed `Built application at: src-tauri/target/release/heic-image-converter` |
| `clippy` (not a gate) | `cargo clippy --locked --all-targets` exits 0 with **5 warnings, none introduced by this feature**: 4 `collapsible_if` in `convert.rs`, 1 `redundant_closure` in `scan.rs`. The two this feature had briefly added — a `thread_local` const initializer in `heic.rs` and an `if_same_then_else` in `trust_gate.rs`'s `command_step` — were fixed, not documented. Clippy is not part of `scripts/ci.sh`; it is reported here because the feature touched the code it checks. |

Feature-level re-checks on the same tree:

```
$ (cd src-tauri && cargo test --locked)
running 108 tests
test result: ok. 107 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
$ (cd src-tauri && cargo fmt --check)        # exit 0, silent
$ npm test
 Test Files  2 passed (2)
      Tests  37 passed (37)
$ bash -n scripts/ci.sh scripts/heif-artifacts.sh   # exit 0
$ scripts/heif-artifacts.sh verify | tail -2
==> Summary
heif-artifacts: verified 4 committed archives, 8 extracted libraries
$ scripts/ci.sh trust | tail -2
trust gate: heif-rs 26.10.3 locked from vendor/heif-rs (path package)
trust gate: PASS
```

The eight extracted libraries are the debug, release and `--target` trees that
exist on this machine; the count is whatever has been built, and `ci.sh trust`
asserts only the four committed archives.

## Boundary

What this evidence does **not** establish:

- **S1 narrows the trust boundary; it does not make `target/` trusted.** Cargo's
  fingerprints, incremental data and the final binary remain untrusted. The
  claim to keep is the narrower one the logs show: no *codegen input* is used
  without matching the hash-verified archive.
- **The release gate is wired, not exercised.** `.github/workflows/release.yml`
  now runs the trust gate after checkout and before `npm ci`, and the Tauri
  build passes `--locked`; that is proven by `trust_gate.rs`'s
  `release_runs_the_trust_gate_and_builds_locked` and by reading the YAML. No
  tag was pushed and no release job ran, so the tag path is untested.
- **macOS is a simulation.** The bash 3.2 transcript is a locally built GNU
  bash 3.2.57 on this Linux machine — the shell version macOS ships, not the
  macOS toolchain, filesystem or codec archive. The macOS trust gate on a Mac
  remains unrun.
- **No hosted CI run covers this tree.** The last green four-job run is
  `44591752926` at `c06fcea`, the commit before HEAD's bookkeeping commit; the
  feature-06 diff is uncommitted, so no runner has seen it. The release workflow
  change in particular has never executed.
- **The offline PASS is an excerpt, not a fresh run of this tree.** It is Task
  10's transcript on the same working tree, and it certifies HEAD's committed
  Rust gate because the gate clones HEAD. The three refusals *were* re-run here.
- **`.ssc/BASELINE.md` still lags the tree.** `ssc-finalize` owns it; it was not
  edited by this task.
- **Below-floor review findings are out of scope.** All ten Medium+ findings are
  in this feature; the eight validated-but-Low items (`A1`, `T1`, `T2`, `T4`–`T7`,
  `M1`) remain deferred and are recorded in the review report.

## Deviations from the spec's Files list

Three modified paths are not in the feature spec's Files list. They are
deliberate and acceptance-mandated by the task that touched them, and are kept:

- `docs/DECISIONS.md` — finding D1 says libheif must be named as a modified build
  "everywhere it matters"; this decision record was one of those places.
- `licenses/README.md` — finding D2 / Task 9 requires the root GPL-3.0 `LICENSE`
  alongside the binary, and this mapping is where a redistributor reads it.
- `docs/verification/2026-11-18/offline-build/README.md` — Task 10's instruction
  to update the evidence README with the new refusal.

Task 11 itself adds this directory (README plus four `.log` transcripts), which
is the spec's `Create: docs/verification/<YYYY-MM-DD>/hardening/`, and corrects
`docs/ARCHITECTURE.md` and `docs/BUILD.md`, both on the Files list.

## Committing the logs

The repository's `.gitignore` excludes `*.log`. The four transcripts need
`git add -f`; the README does not. The directory is currently untracked — this
task was not allowed to commit.
