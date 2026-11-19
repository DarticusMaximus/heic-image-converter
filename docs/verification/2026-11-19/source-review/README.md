# Source review — recorded evidence, 2026-11-19

This is the dated evidence record for stage 01, feature 04 ("Source review and
notices"), Task 7. It holds two transcripts and this README, so a reviewer can
re-check the review's factual claims from the committed bytes without
downloading anything or repeating the runs.

Every value below was measured on the machine and at the commit named in the run
block. Nothing was copied from `docs/HEIC-SOURCE-REVIEW.md`, from the sibling
`docs/verification/2026-11-18/offline-build/` record, or from expectation: the
review and these transcripts agree because both read the same committed files.

## The claim under test

That two kinds of claim in `docs/HEIC-SOURCE-REVIEW.md` are mechanically
re-checkable against this repository:

1. **The archive inventory.** The committed `bundle_linux_x64.zip` has the
   SHA-256 the manifest pins, contains the members the review describes, carries
   the version banners it quotes, ships no license or notice file, and imports
   the dynamic-loading symbols and `LIBHEIF_PLUGIN_PATH` string it reports.
2. **The drift tests.** The nine `source_review` tests run green and cover the
   exact counts the review states: 17 vendored files, 20 `unsafe` sites, 17
   seeded questions, 14 findings with a disposition, 3 license texts, and 2
   superseded decisions.

## Run block

| Field | Value |
| --- | --- |
| Run date | 2026-11-19 |
| Machine | Omarchy, hostname `omarchy` (`/etc/os-release`: `NAME="Omarchy"`, `ID=omarchy`) |
| OS / kernel | `Linux 7.2.15-1-omarchy x86_64` (`uname -srm`) |
| CPU | 8 logical cores (`nproc`) |
| Memory | 30 GiB total, 14 GiB available at run time (`free -h`) |
| `rustc` | `rustc 1.99.1 (58cf8f9c3 2026-10-13) (Arch Linux rust 1:1.99.1-1)` |
| `cargo` | `cargo 1.99.1 (5f94df478 2026-08-27) (Arch Linux rust 1:1.99.1-1)` |
| `node` | `v26.13.1` |
| `npm` | `11.20.0` |
| Certified commit | `b26e6c3d3b4b750d169d2591a53d5e2a78e73c49` (`git rev-parse HEAD`) |
| Working-tree state at run time | clean — `git status --porcelain --untracked-files=all` returned 0 entries immediately before the gate |
| Coverage command | `cd src-tauri && cargo test source_review -- --nocapture` |
| Coverage exit status | **0** |
| Gate command | `cd src-tauri && cargo test source_review && cargo fmt --check && cd .. && scripts/ci.sh rust && scripts/ci.sh build && npm test` |
| Gate exit status | **0**, wall clock `1m58.468s` (`time`) |
| Archive transcript | `archive-inventory.txt`, 205 lines, commands run from the repository root |
| Coverage transcript | `coverage.txt`, 28 lines, the unedited stdout/stderr of the coverage command |

The archive commands are listed one per line in `archive-inventory.txt` with
their real output; they cover `unzip`, `sha256sum`, `grep`, `nm`, `strings`,
`ar`, `readelf`, and `objdump`. The one command that exits non-zero is the
deliberate empty search for license-like member names
(`unzip -Z1 … | grep -iE 'license|copying|notice'`), which exits 1 because it
matches nothing; the same count is then computed explicitly and prints `0`.

The extraction directory was `/tmp/opencode/f04-inventory`, outside the
repository. It is not committed and nothing reads it after the transcript was
captured.

## What each file proves

### `archive-inventory.txt`

A command-and-output transcript against
`src-tauri/vendor/heif-bundles/bundle_linux_x64.zip` and its extraction. It
establishes, with the commands visible beside every value:

- **Provenance.** The zip is 3,325,156 bytes, and `sha256sum` gives
  `b5cbed5d08fc79f64b5171cd401fbd62da43333d26b5ad9905354704cf072f82`. The
  `bundle_linux_x64.zip` row in `src-tauri/vendor/heif-bundles/manifest.json`
  carries the same `bytes` and `sha256`; an explicit string comparison prints
  `SHA-256 match: yes`. The extracted `lib/libheif.a` hashes to
  `1e308363b99ff5ef69711ff9a4eea3d39c3eb590ba145656e73b22efeaa390b3`, which is
  the row's `lib_sha256`.
- **Members.** `unzip -Z1 … | wc -l` gives 32 members: 29 files and 3 directory
  entries (`include/`, `include/libheif/`, `lib/`). Of the 29 files, 28 are
  headers under `include/libheif/` and one is the static library
  (`lib/libheif.a`, 12,953,188 bytes). `unzip -l` prints the full member list.
- **No license or notice member.** The search over the listing returns no rows,
  the count is `0`, and the derived line reads
  `archive contains license/notice files: 0`.
- **Version banners.** `include/libheif/heif_version.h:34` is
  `#define LIBHEIF_VERSION "1.23.6"`, and the library carries the bare string
  `1.23.6`. `de265.cc.o` carries `1.1.3`; `version.cpp.o` carries `4.3-vcpkg`
  and `[Linux][GCC 13.3.0][64 bit][noasm] 8bit`.
- **Renamed symbols.** `nm` counts 2,778 defined text symbols with the `heifrs_`
  prefix, 465 defined `heif_*` symbols left unprefixed, and 0 defined `de265_`
  or `x265_` text symbols. (`nm` prints `no symbols` for
  `compression_brotli.cc.o` and `compression_zlib.cc.o`; those warnings are left
  in the transcript.)
- **The dynamic-loading surface.** `plugins_unix.cc.o` imports `dlopen`,
  `dlsym`, `dlclose`, `dlerror`, and `getenv`; `api.cpp.o` imports `dlopen` and
  `dlsym`. `LIBHEIF_PLUGIN_PATH` occurs once in the archive, and in
  `plugins_unix.cc.o` it is `.LC8` at `.rodata.str1.1+0x3c` (section file offset
  `0x1159` plus `0x3c` is the `1195` printed by `strings -t x`). The disassembly
  shows `.LC8` loaded into `%rdi` immediately before `call getenv`, inside the
  symbol `heifrs__Z53get_plugin_directories_from_environment_variable_unix…`.

### `coverage.txt`

The unedited output of `cd src-tauri && cargo test source_review -- --nocapture`
at `b26e6c3`. It shows `running 9 tests` and
`test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 74 filtered out`,
plus the counts each completeness test printed:

| Printed line | Test that prints it |
| --- | --- |
| `vendored files covered: 17` | `review_covers_every_vendored_source_file` |
| `unsafe sites covered: 20` | `review_covers_every_unsafe_site` |
| `seeded questions covered: 17` | `every_seeded_question_is_answered` |
| `findings with disposition: 14` | `every_finding_has_a_disposition` |
| `license texts checked: 3` | `license_texts_match_their_recorded_hashes` |
| `superseded decisions marked: 2` | `decision_records_carry_the_update` |

These are the numbers `docs/HEIC-SOURCE-REVIEW.md` states in §2 (17 files), §5
(20 sites), §10 (17 questions), §11 (14 findings), and the license/decision
claims in §10 Q17 and Task 6. A green run is not proof the prose is correct; it
is proof the document has not drifted from the tree, which is what the tests
pin.

### The feature gate

The whole feature gate ran once, in that order, at `b26e6c3` with a clean tree,
and exited 0:

- `cargo test source_review` — the nine tests above, 74 filtered out.
- `cargo fmt --check` — no output, exit 0.
- `scripts/ci.sh rust` — the full Rust suite: `83 tests`,
  `82 passed; 0 failed; 1 ignored` (the ignored test is the fixture-manifest
  updater), plus the two 0-test targets, all green.
- `scripts/ci.sh build` — the Tauri release build compiled `heif-rs v26.10.3`
  from the vendored path, finished the release profile in `1m 55s`, and built
  the application binary. `grep -c "heif-rs: downloading"` over the gate output
  is `0`: it linked the committed archive with no fetch.
- `npm test` — Vitest, `2` test files and `37` tests passed.

## Which review section relies on which file

| Evidence | Supports |
| --- | --- |
| `archive-inventory.txt` | §1's method block, member counts, hash match, license/notice absence, version header, and §1's out-of-scope boundary; §7's plugin-registry analysis (`dlopen` imports, `LIBHEIF_PLUGIN_PATH`, `getenv` call site); §9's plugin-load surface; §10 Q13 and Q14; §11 F01, F02, and F11. |
| `coverage.txt` | §2's read-log count (17), §5's `unsafe` register count (20), §10's question count (17), §11's finding count and dispositions (14), the three license texts, and the two decision updates — the drift checks for all of them. |
| This README | The run context: which machine, which commit, which commands, and which exit statuses the two transcripts belong to. |

## The boundary

The archive claims are **linux x64 only**. Only
`src-tauri/vendor/heif-bundles/bundle_linux_x64.zip` was byte-inspected. The
other seven archives — `bundle_linux_arm64.zip`, `bundle_osx_x64.zip`,
`bundle_osx_arm64.zip`, `bundle_windows_x64.zip`, `bundle_windows_arm64.zip`,
`bundle_mingw_x64.zip`, and `bundle_mingw_arm64.zip` — are hash-pinned in
`manifest.json` but were not opened, listed, or inspected here; stages 05 and 06
own that work. Nothing in this directory says anything about Windows or macOS
builds, about the codecs as they are linked on those platforms, or about a
cross-platform runtime behaviour.

The **codec C/C++ sources were not reviewed**. The archive is an opaque prebuilt
static library: the transcript reads its member names, its version banners, and
its symbol table, and no libheif, libde265, or x265 source was read. This is
therefore **not a vulnerability audit**, not a compliance opinion, and not a
substitute for either. It records which versions were linked and what the bytes
say about themselves.

The gate above ran on this Linux machine only; it proves the Linux build links
the committed archive offline. It does not widen either boundary.

## Provenance convention

Nothing here was fetched. Every value comes from the committed bytes: the zip is
the file in `src-tauri/vendor/heif-bundles/`, the manifest is the committed JSON,
and the library is what `unzip` produced from the zip. No command in either
transcript contacts the network. A reviewer can re-run any `$` line from the
repository root and get the transcript's output for that command; the only paths
outside the repository are the `/tmp/opencode/f04-inventory` extraction shown in
the commands themselves.

## Commit relationship

`archive-inventory.txt` and `coverage.txt` were produced at
`b26e6c3d3b4b750d169d2591a53d5e2a78e73c49` with a clean tree, and the gate
certifies that commit. This directory is committed one commit later; the only
difference between the certified commit and the commit that carries these files
is the three files in this directory, which no build reads. A reviewer who wants
the certified tree can check out `b26e6c3` and run `git diff --name-only
b26e6c3 HEAD` to see exactly those three additions.
