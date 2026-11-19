# Offline build — recorded run, 2026-11-18

This is the dated evidence record for stage 01, feature 03 ("Offline build"). It
holds the transcript of one full `scripts/ci.sh offline` run, the transcript of
the deliberate missing-artifact drill, and this README, which is meant to spare a
reviewer from repeating either. Every value below was measured on the machine and
at the commit named in the run block; nothing is copied from an older document or
from expectation.

The claim under test: the Rust half of a clean checkout — a `git clone` of
`dfb2fe7` with no `node_modules`, no `dist`, and no `src-tauri/target` — compiles
and tests with no route to the network.

## Run block

| Field | Value |
|---|---|
| Run date | 2026-11-18 |
| Machine | Omarchy (Arch-based), hostname `omarchy` |
| OS / kernel | `Linux 7.2.15-1-omarchy x86_64` (`uname -srm`) |
| CPU | 8 logical cores (`nproc`) |
| Memory | 30 GiB total (`free -h`) |
| `rustc` | `rustc 1.99.1 (58cf8f9c3 2026-10-13) (Arch Linux rust 1:1.99.1-1)` |
| `cargo` | `cargo 1.99.1 (5f94df478 2026-08-27) (Arch Linux rust 1:1.99.1-1)` |
| `node` | `v26.13.1` |
| `npm` | `11.20.0` |
| Certified commit | `dfb2fe7133c7fe52ffe0f0f66f9acbb373eea875` |
| Working-tree state at run time | clean — `git status --porcelain --untracked-files=all` returned 0 entries immediately before the run, and the gate printed no dirty-tree warning |
| Gate command | `CI_OFFLINE_DIR=/home/darticus/.cache/offline-gate scripts/ci.sh offline` |
| Gate exit status | **0** |
| Isolation primitive | `unshare -rn` |
| Clone footprint (`src-tauri/target`) | `4.5G` (see [Measured clone footprint](#measured-clone-footprint)) |
| Transcript | `run.log`, 520 lines, `run.log:520` carries the PASS line |

`CI_OFFLINE_DIR` was set because the default work directory is `${TMPDIR:-/tmp}`,
which is a 16 GiB tmpfs on this machine with 11 GiB free at run time: above the
gate's 8 GiB floor, below its 12 GiB warning, and RAM-backed. The script's own
usage text says to point `CI_OFFLINE_DIR` at a disk-backed path when the default
is a small tmpfs, and doing so removes an ENOSPC partway through a cold build —
the failure the floor exists to prevent, and one that must never be confused with
an offline failure. `run.log:1` records the work directory and the free space the
gate measured (`55 GiB free`).

## Isolation and the two-sided positive control

Both sides of the control were exercised with the gate's own probe, a literal IP
so neither DNS nor a proxy can fake the result. Observed outside the wrapper:

```
$ timeout 5 bash -c 'exec 3<>/dev/tcp/1.1.1.1/443'
outside: exit 0
```

Observed inside the wrapper:

```
$ unshare -rn timeout 5 bash -c 'exec 3<>/dev/tcp/1.1.1.1/443'
bash: connect: Network is unreachable
bash: line 1: /dev/tcp/1.1.1.1/443: Network is unreachable
inside: exit 1
```

The gate ran the same two probes itself before building, outside first and inside
second, and printed `run.log:7`:

```
offline gate: sandbox verified unreachable (positive control)
```

`bwrap` is installed at `/usr/bin/bwrap` on this machine, but `unshare -rn` is
probed first and its probe exited 0, so `unshare -rn` is what ran — for the
positive control and for the build, through one command array. The outside probe
also demonstrates that the wrapper can find `bash` and `timeout`; a wrapper that
could not start would otherwise look like isolation.

## What was proven

`run.log` is the unedited output of one command on this machine:

```
CI_OFFLINE_DIR=/home/darticus/.cache/offline-gate scripts/ci.sh offline
```

The gate cloned `dfb2fe7133c7fe52ffe0f0f66f9acbb373eea875` with
`git clone --no-hardlinks`, asserted the clone contained no `node_modules`,
`dist`, or `src-tauri/target`, then ran the clone's own committed
`scripts/ci.sh rust` — `cargo fmt --check`, then `cargo test --locked` — inside
`unshare -rn` with `CARGO_NET_OFFLINE=true`, from an empty `target/` directory.

What that establishes, concretely:

- The formatting check passed and the test build compiled from scratch
  (`Finished \`test\` profile [unoptimized + debuginfo] target(s) in 1m 00s`,
  `run.log:427`) inside a network namespace in which the same probe that works
  outside it reports `Network is unreachable`.
- The suite that ran was `running 74 tests` with
  `test result: ok. 73 passed; 0 failed; 1 ignored` (`run.log:506`); the ignored
  test is `fixture_corpus::update_fixture_manifest`, which rewrites fixture bytes
  on demand. The other two cargo targets contributed 0 tests and were green.
- No codec download was attempted: `grep -c "heif-rs: downloading" run.log`
  is `0`, and the gate refuses the run on its own if that string ever appears
  (`scripts/ci.sh`'s `grep -q` after the build), so the absence is an assertion
  rather than an observation.
- The clone is the clean checkout: the committed files of `dfb2fe7`, with the
  clone's own committed `scripts/ci.sh` being what ran.

`run.log:520` is the closing line:

```
offline gate: PASS (HEAD dfb2fe7133c7fe52ffe0f0f66f9acbb373eea875, isolation: unshare -rn)
```

The gate reads that SHA back from the clone rather than from a variable captured
earlier, so the line cannot name a different commit from the one that was built.

## Measured clone footprint

Inside a fresh clone of `dfb2fe7`, after the gate's build:

```
$ du -sh src-tauri/target
4.5G	src-tauri/target
$ du -sh src-tauri/target/debug
4.5G	src-tauri/target/debug
$ du -sh src-tauri/target/debug/deps
3.7G	src-tauri/target/debug/deps
$ du -sh src-tauri/target/debug/build
341M	src-tauri/target/debug/build
```

`du -sh` prints powers of 1024, so these are 4.5 GiB, 3.7 GiB, and 341 MiB.

Method: the gate removes its clone in an `EXIT` trap, so sampling the live clone
would race the build and could catch an incomplete `target/`. Instead the gate's
own child command was reproduced unchanged in a throwaway clone:
`git clone --quiet --no-hardlinks` of the same commit on the same disk-backed
filesystem, the same absent-`node_modules`/`dist`/`target` assertions, then
`unshare -rn env CARGO_NET_OFFLINE=true ./scripts/ci.sh rust`, then `du -sh`.
Same commit, same command, same empty starting point, so the resulting `target/`
is the one the gate produced; the throwaway clone was deleted afterwards.

This is smaller than the 6.4 GB figure quoted in the feature spec for this
checkout's `src-tauri/target/debug`. That figure was an accumulated target
carried across many builds and profiles; a one-shot cold test build of this
commit is 4.5 GiB. The gate's 8 GiB floor stays conservative against the larger
number and was not changed here.

## The boundary

The offline claim covers the Rust gate and nothing else. On a fresh machine
`npm ci` still needs the network to fill `node_modules`, Vite still needs it to
resolve and build the frontend, and the **Tauri release build**
(`scripts/ci.sh build`, which runs the Tauri CLI through npm) is still an online
operation: none of those were exercised offline, and this evidence says nothing
about them. Only the four committed codec archives build offline; the four that
are not committed — `bundle_linux_arm64`, `bundle_windows_arm64`,
`bundle_mingw_x64`, `bundle_mingw_arm64` — still fetch once from the pinned
release, hash-verified against the manifest, and a build that needs one fails
inside this gate by design. Two environment overrides change what a run
exercises and therefore what it proves: `HEIF_BINARIES_DIR` lets `build.rs`
resolve codec libraries from somewhere other than the committed bundle
directory, and `CARGO_TARGET_DIR` redirects the build away from
`src-tauri/target`, so the gate's "no `src-tauri/target` in the clone" assertion
no longer describes the directory Cargo writes to. Neither was set for the run
recorded here; `run.log` shows the build writing to `target/debug/deps` under the
clone. That had to be read off the log, though, which is why the gate now
**refuses** to run when either variable is non-empty: it exits 1 before the disk
check or the clone, naming the variable, its value, and why a pass under it would
not describe the committed bundle, and it also unsets both in the child command
and prints the effective codec source and target directory before the build. This
README's run predates that refusal — the results and `run.log` above are
unchanged and were produced by the older gate. The refusal is proven separately,
with a recorded run for each variable and for both, in
`docs/verification/2026-11-19/hardening/` (Task 11 of the hardening feature).

## Which log supports which claim

| File | Supports |
|---|---|
| `run.log` | The offline claim: a clone of `dfb2fe7` with no `node_modules`, `dist`, or `target` passed `cargo fmt --check` and `cargo test --locked` inside `unshare -rn` with `CARGO_NET_OFFLINE=true`, after the two-sided positive control showed the network reachable outside and unreachable inside, with no `heif-rs: downloading` line, closing with `offline gate: PASS`. |
| `missing-artifact.log` | The local-only rule and the repair path: a committed codec archive removed from the tree fails `cargo build --locked` with the three-line repair message and no network attempt, fails identically inside `unshare -rn` (so the failure is the code decision, not a network timeout), is restored by `scripts/heif-artifacts.sh fetch`, and the tree then verifies and builds again. |

`missing-artifact.log` details, all visible in that file: the archive's SHA-256
before the drill and after the fetch are identical
(`b5cbed5d08fc79f64b5171cd401fbd62da43333d26b5ad9905354704cf072f82`);
`scripts/heif-artifacts.sh verify` exited 1 while the archive was missing and 0
after the fetch; the build exited 101 outside the namespace and 101 inside it,
with the same message both times; `git status --porcelain --untracked-files=all`
and `git diff --stat` were empty afterwards, so the restored file is
byte-identical and nothing else moved.

## Commit relationship

`run.log` certifies commit `dfb2fe7133c7fe52ffe0f0f66f9acbb373eea875`. This
evidence directory is committed one commit *later*, and the only difference
between the certified commit and the commit that carries this file is the three
files in this directory:

```
$ git diff --name-only dfb2fe7133c7fe52ffe0f0f66f9acbb373eea875 HEAD
docs/verification/2026-11-18/offline-build/README.md
docs/verification/2026-11-18/offline-build/missing-artifact.log
docs/verification/2026-11-18/offline-build/run.log
```

One mechanical note: the repository's `.gitignore` excludes `*.log` for tool
output, so the two transcripts were added with `git add -f`. No ignore rule was
changed; `git ls-files` shows the three files are tracked, and `git status`
reports a clean tree.

The reason for the one-commit gap is structural: a commit cannot contain the
transcript of its own certification. To produce the transcript the tree must be
clean, and the run certifies `HEAD`; writing the transcript into a file makes the
tree dirty and, once committed, changes `HEAD`. So the PASS line names the commit
that was built, and the evidence commit is its child. A reviewer who wants the
certified tree can check out `dfb2fe7`; a reviewer reading this directory is
looking at that tree plus these three files, which no build reads.
