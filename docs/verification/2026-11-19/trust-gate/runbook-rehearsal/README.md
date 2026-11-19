# Runbook rehearsal — vendor-diff half (S0, S1, S2, S5), 2026-11-19

This is the evidence record for Task 5 of feature `feature-05-trust-gate`: the
first four steps of `docs/UPGRADE-HEIC.md` executed against the current pins.

**No version was changed.** The rehearsal is same-version: the crates.io tarball
downloaded here is `heif-rs` `26.10.3`, the version already vendored at
`src-tauri/vendor/heif-rs/`, and the archive release stays `26.10.1`. Nothing
under `src-tauri/` was edited, no `rsync` ran into the repository, and the
published tree was extracted and diffed in `/tmp/opencode/heic-rehearsal/`. The
point of the run is that the vendor-diff commands work and the vendored tree is
exactly the published crate plus the four paths `VENDORED.md` records — so when a
real bump happens, a difference outside those four paths is a signal, not noise.

**What this file does not cover.** Steps S3, S4 and S6–S9, and the archive /
extracted-library hash re-check that S3 produces, are the other half of this
rehearsal (`hash-recheck.txt`, not written here). No `lib_sha256` was recomputed;
S5 hashed the committed Linux zip (an archive hash, not a `lib_sha256`).
`docs/UPGRADE-HEIC.md`'s header and §8 row still describe the rehearsal as
scheduled; the remaining steps and those cells are closed later in this feature.
This file claims only what ran on the date below.

## Machine and tools

| Field | Value |
| --- | --- |
| Date | 2026-11-19 (UTC `2026-11-19T17:22Z`) |
| Repository state | clean at start — `git status --porcelain --untracked-files=all` printed nothing |
| Commit | `8d3a6d8aa0e3947b8f8474805d8046be7a6b9091` (`8d3a6d8 Add the HEIC upgrade runbook and pin it with drift tests`) |
| Machine | Omarchy (Arch-based), hostname `omarchy` |
| OS / kernel | `Linux 7.2.15-1-omarchy x86_64` (`uname -srm`) |
| CPU | 8 logical cores (`nproc`) |
| `rustc` | `rustc 1.99.1 (58cf8f9c3 2026-10-13) (Arch Linux rust 1:1.99.1-1)` |
| `cargo` | `cargo 1.99.1 (5f94df478 2026-08-27) (Arch Linux rust 1:1.99.1-1)` |
| `sha256sum` | `sha256sum (GNU coreutils) 9.12` |
| `tar` | `tar (GNU tar) 1.35` |
| `curl` | `curl 8.23.0 (x86_64-pc-linux-gnu) libcurl/8.23.0 OpenSSL/3.6.5` |
| `gh` | `gh version 2.104.0 (2026-11-12)`, authenticated as `DarticusMaximus` — used by S1 (`gh release view`) |
| `python3` | `Python 3.14.8` |
| Scratch directory | `/tmp/opencode/heic-rehearsal/` (tarball, extracted tree, captured logs) |

## Steps run

| Step | Command (as executed) | Observed | Produced a diff? |
| --- | --- | --- | --- |
| **S0** | `python3 -c 'import json; m=json.load(open("src-tauri/vendor/heif-bundles/manifest.json")); print(m["release"], m["crate"])'`; `sha256sum` + `wc -c` on the four committed zips; `sed -n '/## 11. Findings/,/## 12./p' docs/HEIC-SOURCE-REVIEW.md \| grep 'open obligation'` | release `26.10.1`, crate `heif-rs` `26.10.3`; four hashes and four byte counts equal the manifest rows; three open obligations (`F01`, `F02`, `F03`, all `high`, all gated on stage 04) | none — S0 only records state |
| **S1** | `gh release view 26.10.1 --repo vegidio/binaries-heif --json tagName,body,assets --jq '.assets[] \| [.name, .size, .digest] \| @tsv'`; `curl -sSfL -H 'User-Agent: image-converter-rehearsal (darticus)' https://crates.io/api/v1/crates/heif-rs/26.10.3`, then read `version.num` / `.checksum` / `.dl_path` with `python3` | tag `26.10.1`, published `2026-10-24T09:53:23Z`, 24 assets in three families (8 `bundle_*`, 8 `dynamic_*`, 8 `static_*`); all eight `bundle_*` sizes and digests equal the manifest rows, including the four committed archives; crate `num` `26.10.3`, checksum `dc5863ce…c939253`, `dl_path` `/api/v1/crates/heif-rs/26.10.3/download`, created `2026-10-24T13:56:16Z`, not yanked, `license` `Apache-2.0` | none — S1 only reads the records |
| **S2** | `curl … -o heif-rs-26.10.3.crate` → `sha256sum` → `tar -xf` → `diff -rq` / `diff -u` against `src-tauri/vendor/heif-rs` | checksum matches S1; extracted tree is 20 files; **exactly four paths differ**: `build.rs`, `Cargo.toml`, `build_support.rs`, `VENDORED.md`; the other 18 are byte-identical | **yes** — the four expected paths and nothing else; see `vendor-diff.txt` |
| **S5** | `cd src-tauri && cargo test source_review`; `sha256sum src-tauri/vendor/heif-bundles/bundle_linux_x64.zip` | `9 passed; 0 failed`; the committed zip's hash equals the manifest row | none — the source review still matches the current bytes |

### S0 — the current state, as recorded

```
$ python3 -c 'import json; m=json.load(open("src-tauri/vendor/heif-bundles/manifest.json")); print(m["release"], m["crate"])'
26.10.1 {'name': 'heif-rs', 'version': '26.10.3'}

$ sha256sum src-tauri/vendor/heif-bundles/bundle_linux_x64.zip src-tauri/vendor/heif-bundles/bundle_osx_x64.zip src-tauri/vendor/heif-bundles/bundle_osx_arm64.zip src-tauri/vendor/heif-bundles/bundle_windows_x64.zip
b5cbed5d08fc79f64b5171cd401fbd62da43333d26b5ad9905354704cf072f82  src-tauri/vendor/heif-bundles/bundle_linux_x64.zip
35e17dfc5c6f84a38ddd0f86edfa2a979db7aa462e03431aa5f745946b25a4e2  src-tauri/vendor/heif-bundles/bundle_osx_x64.zip
18f3faa9abcc36a0ca316fde55848eb1815ca4066613c0451c498498d35dd1b1  src-tauri/vendor/heif-bundles/bundle_osx_arm64.zip
5909672143ca081e24f9d1a448932abf1b1d69c869f8b6daeefbda4bc99ae8ed  src-tauri/vendor/heif-bundles/bundle_windows_x64.zip

$ wc -c src-tauri/vendor/heif-bundles/bundle_{linux_x64,osx_x64,osx_arm64,windows_x64}.zip
 3325156 src-tauri/vendor/heif-bundles/bundle_linux_x64.zip
 3158467 src-tauri/vendor/heif-bundles/bundle_osx_x64.zip
 3036209 src-tauri/vendor/heif-bundles/bundle_osx_arm64.zip
 8910372 src-tauri/vendor/heif-bundles/bundle_windows_x64.zip
18430204 total
```

Every hash and every byte count equals the matching row in
`manifest.json` (`bytes` / `sha256`), which is also what `trust-green.log` shows
the gate checking. The four matching lines from §11:

```
$ sed -n '/## 11. Findings/,/## 12./p' docs/HEIC-SOURCE-REVIEW.md | grep 'open obligation'
<legend line: "is one of `fixed here`, `accepted risk`, `watch`, `open obligation` (Decision 4).">
| F01 | The codec archive is an opaque third-party build with no build recipe, patch, license, or notice inside it; … | high | open obligation | Gate: stage 04 (§12.1) … |
| F02 | The linked x265 and libde265 objects are symbol-renamed (`heifrs_`) modified builds with no published patch … | high | open obligation | Gate: stage 04 (§12.2) … |
| F03 | Statically linking LGPL-3.0 libheif and libde265 creates a §4 relink duty that a prebuilt archive cannot satisfy … | high | open obligation | Gate: stage 04 (§12.3) … |
```

The pattern matches three finding rows plus one legend line that names the
disposition vocabulary; the three findings are `F01`–`F03`. They are unchanged by
this rehearsal: the codec archive was not touched, so its obligations cannot
have moved.

### S1 — the two release records

Both S1 commands were executed. The first reads the `binaries-heif` release
metadata:

```
$ gh release view 26.10.1 --repo vegidio/binaries-heif --json tagName,body,assets --jq '.assets[] | [.name, .size, .digest] | @tsv'
bundle_linux_arm64.zip	4011715	sha256:bbc73ef9c5d049cac16e124df33d2117330d689bc6e3688108052418016a91cf
bundle_linux_x64.zip	3325156	sha256:b5cbed5d08fc79f64b5171cd401fbd62da43333d26b5ad9905354704cf072f82
bundle_mingw_arm64.zip	3137542	sha256:c510a0d09d8e68fd41fe9a32bccb3ddbd9149e8d7f2de54cb448a8f168d1fcdf
bundle_mingw_x64.zip	4100236	sha256:90ff4a315e3fac6e4b97fc0d4a1591d2240b2c21dda5df28a321674e18a5ce29
bundle_osx_arm64.zip	3036209	sha256:18f3faa9abcc36a0ca316fde55848eb1815ca4066613c0451c498498d35dd1b1
bundle_osx_x64.zip	3158467	sha256:35e17dfc5c6f84a38ddd0f86edfa2a979db7aa462e03431aa5f745946b25a4e2
bundle_windows_arm64.zip	8254309	sha256:2456725011c9b1b5666540ff12cedf33de1e39b819b7d2ef2a61ae1a2f9b7ad7
bundle_windows_x64.zip	8910372	sha256:5909672143ca081e24f9d1a448932abf1b1d69c869f8b6daeefbda4bc99ae8ed
dynamic_linux_arm64.zip	3020980	sha256:a461c5605ad7c1ab94644cdeaa20ef745cd40238594047418ec08417b30d2457
dynamic_linux_x64.zip	2839051	sha256:c46785fd10fd10c5cfa168be16c689548a40de9624739a163fcdb4b4369dd091
dynamic_mingw_arm64.zip	1794052	sha256:21f328785244dac5ca9c716ee40a73affe9131733898a8cb65b7b7026cd2f1a8
dynamic_mingw_x64.zip	3112891	sha256:6d81071596b3c69a765dd2f2d987028e4b8815f397ca4d0159fcfbc7a2a04edb
dynamic_osx_arm64.zip	2318819	sha256:399d9b07b1fb34faa25aba7c139f17588e7b2912578ca825da5e53d7db13fddf
dynamic_osx_x64.zip	2347923	sha256:339a83f585ac0b0422482d7cdc04f0d8b8825226d92f3d73ea419fb659e94e48
dynamic_windows_arm64.zip	1312201	sha256:d5f61a92588d92171af196c9a40a183fa7f2536016c7fb2757e837d2f81d07d1
dynamic_windows_x64.zip	1692318	sha256:b7736bd9e1e25d68a85363f43815f823a2846e5428387bcc264623e307dcf1a3
static_linux_arm64.zip	4033112	sha256:1c243ea6512a72eba1b6c5384901db011f064a99c0d4a3a58899b64e85dd0885
static_linux_x64.zip	3345907	sha256:b541bfe6db84018534136f7e9095aa1e54255a2b4e9e6119386fd4701416d3c9
static_mingw_arm64.zip	3164259	sha256:fa11fc2520e6f8701e10c0e76c0e85e7fbe816fccd6c430a437815fd74a5cec8
static_mingw_x64.zip	4035312	sha256:fc7e28c948c058e4a35f21ec8fc86cd4f8e8227f372e9c9769bf0a6622a030a9
static_osx_arm64.zip	3057417	sha256:83d41edc40bf5e8d654964859daa16bee18f892efc5d564001c9952b2b408970
static_osx_x64.zip	3172652	sha256:0749ef83d2c464d52244970c3aadd99d67ca71b45b07a4fd7d178d8e83a31414
static_windows_arm64.zip	8166961	sha256:8592b54edf0d1e8639c05985b10853453f7c6174089f20efc47ca7727e5ee0ab
static_windows_x64.zip	8819585	sha256:0be4a0e376ea6413242272d3d11ceffc1a044fad2e534bd1db9f35d59d6c1731
```

Metadata for the same release:

```
$ gh release view 26.10.1 --repo vegidio/binaries-heif --json tagName,publishedAt --jq '.tagName + " " + .publishedAt'
26.10.1 2026-10-24T09:53:23Z
```

The release carries 24 assets in three families — `bundle_*`, `dynamic_*`,
`static_*`, eight each — and the release body is a single line, `New release`.
The manifest pins only the eight `bundle_*` archives, which are the ones the
vendored `build.rs` links. Every `bundle_*` row was compared against
`manifest.json`, and all eight agree on both size and digest:

| archive | manifest `bytes` | published size | `sha256` vs published digest |
| --- | --- | --- | --- |
| `bundle_linux_x64.zip` (committed) | 3325156 | 3325156 | match |
| `bundle_osx_x64.zip` (committed) | 3158467 | 3158467 | match |
| `bundle_osx_arm64.zip` (committed) | 3036209 | 3036209 | match |
| `bundle_windows_x64.zip` (committed) | 8910372 | 8910372 | match |
| `bundle_linux_arm64.zip` (`committed: false`) | 4011715 | 4011715 | match |
| `bundle_windows_arm64.zip` (`committed: false`) | 8254309 | 8254309 | match |
| `bundle_mingw_x64.zip` (`committed: false`) | 4100236 | 4100236 | match |
| `bundle_mingw_arm64.zip` (`committed: false`) | 3137542 | 3137542 | match |

This is the hash rule §3 states, applied at S1: the manifest's digests are
cross-checked against GitHub's published `digest` field rather than against any
local file. No archive was downloaded for this comparison, so it is a record
check, not S3's download-and-hash.

The second S1 command reads the crate record:

```
$ curl -sSfL -H 'User-Agent: image-converter-rehearsal (darticus)' https://crates.io/api/v1/crates/heif-rs/26.10.3
num      : 26.10.3
checksum : dc5863cecbc91f3b87a0cb508e864f76bafac4b772420ab26b5fa7219c939253
dl_path  : /api/v1/crates/heif-rs/26.10.3/download
crate    : heif-rs
created_at: 2026-10-24T13:56:16.331282Z
yanked   : False
license  : Apache-2.0
```

Tarball URL: `https://crates.io/api/v1/crates/heif-rs/26.10.3/download`, which
redirects to `https://static.crates.io/crates/heif-rs/heif-rs-26.10.3.crate`.

**Deviation observed.** The runbook's S1 curl, as written, omits a `User-Agent`,
and the crates.io **API** endpoint answers that with HTTP 403:

```
$ curl -sSfL https://crates.io/api/v1/crates/heif-rs/26.10.3
curl: (22) The requested URL returned error: 403
```

A `-H 'User-Agent: image-converter-rehearsal (darticus)'` header makes that call
succeed and is required for it.

The **download** endpoint S2 uses is not affected. Both UA variants were checked
on this machine:

```
$ curl -sSfL -o default-ua.crate https://crates.io/api/v1/crates/heif-rs/26.10.3/download    # curl's default UA
curl exit: 0; 33042 bytes; dc5863cecbc91f3b87a0cb508e864f76bafac4b772420ab26b5fa7219c939253

$ curl -sSfL -A '' -o empty-ua.crate https://crates.io/api/v1/crates/heif-rs/26.10.3/download  # empty UA
curl exit: 0; 33042 bytes; dc5863cecbc91f3b87a0cb508e864f76bafac4b772420ab26b5fa7219c939253
```

So the `User-Agent` requirement applies to the crates.io API record only. The S2
command in this record keeps the header for consistency with S1; it is optional
there and would have worked without it. The runbook was left unedited here (this
task changes no runbook text); the S1 header is worth adding in a later pass.

### S2 — download, verify, extract, diff

```
$ curl -sSfL -H 'User-Agent: image-converter-rehearsal (darticus)' \
    -o /tmp/opencode/heic-rehearsal/heif-rs-26.10.3.crate \
    -w '%{url_effective}\n' \
    https://crates.io/api/v1/crates/heif-rs/26.10.3/download
https://static.crates.io/crates/heif-rs/heif-rs-26.10.3.crate
downloaded: 33042 bytes

$ sha256sum /tmp/opencode/heic-rehearsal/heif-rs-26.10.3.crate
dc5863cecbc91f3b87a0cb508e864f76bafac4b772420ab26b5fa7219c939253  /tmp/opencode/heic-rehearsal/heif-rs-26.10.3.crate
published checksum (S1): dc5863cecbc91f3b87a0cb508e864f76bafac4b772420ab26b5fa7219c939253
verdict: MATCH

$ tar -xf /tmp/opencode/heic-rehearsal/heif-rs-26.10.3.crate -C /tmp/opencode/heic-rehearsal
$ ls -A /tmp/opencode/heic-rehearsal/heif-rs-26.10.3
.cargo_vcs_info.json  Cargo.lock  Cargo.toml  Cargo.toml.orig  LICENSE
README.md  build.rs  src  tests
```

The observed checksum equals the one S1 publishes, which is the hash rule §3 asks
for: the value is cross-checked against crates.io's record, not against the
downloaded file's own output alone. It also equals the hash `VENDORED.md` records
for the tarball the committed tree was unpacked from.

The diff, from the repository root:

```
$ diff -rq /tmp/opencode/heic-rehearsal/heif-rs-26.10.3 src-tauri/vendor/heif-rs
Files /tmp/opencode/heic-rehearsal/heif-rs-26.10.3/build.rs and src-tauri/vendor/heif-rs/build.rs differ
Only in src-tauri/vendor/heif-rs: build_support.rs
Files /tmp/opencode/heic-rehearsal/heif-rs-26.10.3/Cargo.toml and src-tauri/vendor/heif-rs/Cargo.toml differ
Only in src-tauri/vendor/heif-rs: VENDORED.md
exit status: 1
```

`vendor-diff.txt` holds this plus the full `diff -u` for the two edited files.
Both recorded edits are what the diff shows:

- `Cargo.toml` — one hunk adding `[build-dependencies.serde_json] version = "1"`
  and `[build-dependencies.sha2] version = "0.10"`, nothing else.
- `build.rs` — the replacement: `VERSION`, `download_and_extract()` and
  `archive_name()` gone; `build_support` module plus `resolve_from_manifest()`,
  `load_archive()` and `verify_library()` added; the archive is hashed before
  extraction and the extracted library after it; `committed: true` archives are
  local-only. `main()`, `emit_link_directives()` (with
  `link_compiler_rt_builtins()`) and `generate_bindings()` are present in both
  files and outside every hunk.

**What S2 skipped, deliberately.** The runbook's S2 continues past the diff with
`rsync -a --delete --exclude … src-tauri/vendor/heif-rs/`, a `sed -i` on
`src-tauri/Cargo.toml`, and `cargo update -p heif-rs`. None of the three ran; the
pin is unchanged. `rsync` would rewrite the vendored tree (byte-identical at this
version), while `sed` and `cargo update` would touch the requirement/lock; the
diff proves the rsync content is identical, and at the same version the other two
are no-ops. Nothing was copied into the repository — the published tree stayed in
`/tmp/opencode/heic-rehearsal/` and was only compared.

### S5 — the source review against the current bytes

```
$ cd src-tauri && cargo test source_review
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.13s
     Running unittests src/lib.rs (target/debug/deps/heic_image_converter_lib-aa20e6904e508055)

running 9 tests
test source_review::decision_records_carry_the_update ... ok
test source_review::notice_names_every_bundled_component ... ok
test source_review::the_review_is_linked_from_the_index ... ok
test source_review::review_records_the_pinned_provenance ... ok
test source_review::every_seeded_question_is_answered ... ok
test source_review::every_finding_has_a_disposition ... ok
test source_review::review_covers_every_vendored_source_file ... ok
test source_review::review_covers_every_unsafe_site ... ok
test source_review::license_texts_match_their_recorded_hashes ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out; finished in 0.00s
exit status: 0

$ sha256sum src-tauri/vendor/heif-bundles/bundle_linux_x64.zip
b5cbed5d08fc79f64b5171cd401fbd62da43333d26b5ad9905354704cf072f82  src-tauri/vendor/heif-bundles/bundle_linux_x64.zip
```

Coverage state as observed (`cargo test source_review -- --nocapture` prints the
counts the assertions derive):

```
vendored files covered: 17
unsafe sites covered: 20
seeded questions covered: 17
findings with disposition: 14
test result: ok. 9 passed; 0 failed
```

That is the review's full recorded set at 26.10.3 — 17 files (15 vendored `.rs`
files plus `VENDORED.md` and the seam), `U01`–`U20`, `Q01`–`Q17`, `F01`–`F14` —
and it went green without an edit, which is the S5 claim for a same-version
rehearsal: nothing drifted. A real bump is expected to turn this red until the
review is rewritten; that expectation was not exercised here.

## What this proves, and what it does not

Proven on this machine, on this date, at `8d3a6d8`:

1. The S1 release records are reachable and agree with the pins: the eight
   `bundle_*` assets publish the sizes and digests `manifest.json` records, and
   crates.io still publishes `26.10.3` with the checksum `VENDORED.md` records.
   The crates.io API call needs the `User-Agent` header the deviation above
   records; with it, both S1 commands run as intended.
2. S0's four archive hashes and S5's source review hold without an edit: the
   committed archives match the manifest byte for byte, and the review still
   covers the current bytes.
3. S2's re-vendor from the published tarball produces exactly the committed
   tree plus the four recorded paths — no fifth difference, no missing file, no
   stray edit under `src/` or `tests/`.
4. The registry-source diff §7 says CI cannot run was run here, once, at this
   version.

Not proven:

- **A version bump.** Nothing moved, so the steps that only a bump exercises —
  re-reading release notes for a *new* version, re-applying edits to a changed
  `build.rs`, a red-then-green S5 — were not run.
- **The archive half.** No archive was downloaded; the four committed archives
  were hashed in S0 and the Linux zip again in S5, but no `lib_sha256` was
  recomputed and the four uncommitted archives were not fetched. S1 compared
  manifest values against the published digest metadata, which is a record check,
  not a byte check.
- **CI.** `scripts/ci.sh trust`, `rust`, `build`, `offline` and the hosted run
  were not part of this task; the trust gate's own clean run is
  `../trust-green.log`.

## Files

- `vendor-diff.txt` — the S2 record: tarball URL, published and observed
  checksums, the extract command, `diff -rq`, the full `diff -u` for `build.rs`
  and `Cargo.toml`, and the note on the two added files.
- `README.md` — this file.
- `../trust-green.log` — the clean `scripts/ci.sh trust` run, for the gate's
  own baseline.
