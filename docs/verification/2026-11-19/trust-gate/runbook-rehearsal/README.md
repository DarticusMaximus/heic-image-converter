# Runbook rehearsal — every step S0–S9 at the current pins, 2026-11-19

This is the evidence record for Tasks 5 and 6 of feature `feature-05-trust-gate`:
`docs/UPGRADE-HEIC.md` executed against the current pins. Task 5 recorded the
vendor-diff half (S0, S1, S2, S5); Task 6 recorded the hash half (S3, S4) and the
maintenance and gate steps (S6–S9), with the archive re-check in
`hash-recheck.txt`.

**No version was changed.** The rehearsal is same-version: the crates.io tarball
downloaded here is `heif-rs` `26.10.3`, the version already vendored at
`src-tauri/vendor/heif-rs/`, and the archive release stays `26.10.1`. Nothing
under `src-tauri/` was edited, no `rsync` ran into the repository, and the
published tree was extracted and diffed in `/tmp/opencode/heic-rehearsal/`; the
four archives were downloaded, extracted, and hashed in
`/tmp/opencode/heic-hash-rehearsal/`. The point of the run is that the vendor-diff
commands work and the vendored tree is exactly the published crate plus the four
paths `VENDORED.md` records — so when a real bump happens, a difference outside
those four paths is a signal, not noise — and that every recorded hash
re-derives from bytes that match the published release.

**What this file covers, and how far.** Every runbook step S0–S9 has a row below
and an observed result. Two boundaries are stated rather than papered over. S9 is
**partially observed**: the cheap gates ran here; `scripts/ci.sh all` and
`scripts/ci.sh offline`, the tamper drill rerun, and the hosted run belong to
Task 8's full-gate run and are not claimed. The archive half covers the four
committed shipping archives; the four uncommitted `bundle_*` archives were not
downloaded, and `hash-recheck.txt` says so. `docs/UPGRADE-HEIC.md`'s header and §8
row still describe the rehearsal as scheduled because this task may not edit the
runbook; a later task closes those cells. This file claims only what ran on the
date below.

## Machine and tools

| Field | Value |
| --- | --- |
| Date | 2026-11-19 (UTC `2026-11-19T17:22Z` for S0–S2/S5, from `2026-11-19T17:42Z` for S3/S4/S6–S9) |
| Repository state | clean at start — `git status --porcelain --untracked-files=all` printed nothing |
| Commit | `8d3a6d8aa0e3947b8f8474805d8046be7a6b9091` (`8d3a6d8 Add the HEIC upgrade runbook and pin it with drift tests`) for S0–S2/S5; `3ade5e0b862f5f16bb584c941ee56c7e06894985` (`3ade5e0 Record the vendor-diff half of the upgrade rehearsal`) for S3/S4/S6–S9 |
| Machine | Omarchy (Arch-based), hostname `omarchy` |
| OS / kernel | `Linux 7.2.15-1-omarchy x86_64` (`uname -srm`) |
| CPU | 8 logical cores (`nproc`) |
| `rustc` | `rustc 1.99.1 (58cf8f9c3 2026-10-13) (Arch Linux rust 1:1.99.1-1)` |
| `cargo` | `cargo 1.99.1 (5f94df478 2026-08-27) (Arch Linux rust 1:1.99.1-1)` |
| `sha256sum` | `sha256sum (GNU coreutils) 9.12` |
| `tar` | `tar (GNU tar) 1.35` |
| `unzip` | `UnZip 6.00 of 20 April 2009, by Info-ZIP` — used by S3 |
| `cmp` | `cmp (GNU diffutils)` — used by S4 to compare committed archives with the downloads |
| `curl` | `curl 8.23.0 (x86_64-pc-linux-gnu) libcurl/8.23.0 OpenSSL/3.6.5` |
| `gh` | `gh version 2.104.0 (2026-11-12)`, authenticated as `DarticusMaximus` — used by S1 (`gh release view`) |
| `python3` | `Python 3.14.8` |
| Scratch directory | `/tmp/opencode/heic-rehearsal/` (tarball, extracted tree, captured logs); `/tmp/opencode/heic-hash-rehearsal/` (four archives, four extractions, NVD/OSV responses, captured gate logs) |

## Steps run

| Step | Command (as executed) | Observed | Produced a diff? |
| --- | --- | --- | --- |
| **S0** | `python3 -c 'import json; m=json.load(open("src-tauri/vendor/heif-bundles/manifest.json")); print(m["release"], m["crate"])'`; `sha256sum` + `wc -c` on the four committed zips; `sed -n '/## 11. Findings/,/## 12./p' docs/HEIC-SOURCE-REVIEW.md \| grep 'open obligation'` | release `26.10.1`, crate `heif-rs` `26.10.3`; four hashes and four byte counts equal the manifest rows; three open obligations (`F01`, `F02`, `F03`, all `high`, all gated on stage 04) | none — S0 only records state |
| **S1** | `gh release view 26.10.1 --repo vegidio/binaries-heif --json tagName,body,assets --jq '.assets[] \| [.name, .size, .digest] \| @tsv'`; `curl -sSfL -H 'User-Agent: image-converter-rehearsal (darticus)' https://crates.io/api/v1/crates/heif-rs/26.10.3`, then read `version.num` / `.checksum` / `.dl_path` with `python3` | tag `26.10.1`, published `2026-10-24T09:53:23Z`, 24 assets in three families (8 `bundle_*`, 8 `dynamic_*`, 8 `static_*`); all eight `bundle_*` sizes and digests equal the manifest rows, including the four committed archives; crate `num` `26.10.3`, checksum `dc5863ce…c939253`, `dl_path` `/api/v1/crates/heif-rs/26.10.3/download`, created `2026-10-24T13:56:16Z`, not yanked, `license` `Apache-2.0` | none — S1 only reads the records |
| **S2** | `curl … -o heif-rs-26.10.3.crate` → `sha256sum` → `tar -xf` → `diff -rq` / `diff -u` against `src-tauri/vendor/heif-rs` | checksum matches S1; extracted tree is 20 files; **exactly four paths differ**: `build.rs`, `Cargo.toml`, `build_support.rs`, `VENDORED.md`; the other 18 are byte-identical | **yes** — the four expected paths and nothing else; see `vendor-diff.txt` |
| **S3** | `gh release download 26.10.1 --repo vegidio/binaries-heif` with the four committed `--pattern` flags; `sha256sum`; `wc -c`; `unzip -q` each into `extract-bundle_*`; `sha256sum extract-*/lib/*` | four downloaded byte counts, four `sha256`, and four extracted `lib_sha256` all equal the manifest; each `sha256` also equals the published release digest; the release publishes no digest for a library inside an archive, so `lib_sha256` has only the manifest value | none — repository untouched; record in `hash-recheck.txt` |
| **S4** | `cmp` each download against `src-tauri/vendor/heif-bundles/<archive>`; `git status --porcelain src-tauri` | all four committed archives are byte-identical to the downloaded assets; `git status --porcelain src-tauri` empty before and after | none — the four are already committed at this version; nothing was copied |
| **S5** | `cd src-tauri && cargo test source_review`; `sha256sum src-tauri/vendor/heif-bundles/bundle_linux_x64.zip` | `9 passed; 0 failed`; the committed zip's hash equals the manifest row | none — the source review still matches the current bytes |
| **S6** | `curl -sS 'https://services.nvd.nist.gov/rest/json/cves/2.0?keywordSearch=<libheif\|libde265\|x265\|heif-rs>'`; OSV `POST /v1/query` for crates.io `heif-rs`; `gh api '/advisories?ecosystem=rust&affects=heif-rs'` | NVD returned 50 libheif, 67 libde265, 3 x265, 0 heif-rs entries; no machine-readable range includes the pinned versions; one libde265 entry (`CVE-2026-88373`) is commit-level and cannot be mapped from the record; OSV and GitHub Advisories returned no `heif-rs` entries | none — S6 records an observation |
| **S7** | `sha256sum -c licenses/SHA256SUMS`; read `THIRD-PARTY-NOTICES.md` §1 and `licenses/README.md` mapping | three `OK` lines, exit 0; notice and mapping state libheif `1.23.6`, libde265 `1.1.3`, x265 `4.3-vcpkg`, `heif-rs` `26.10.3`, all equal to the current pins | none — no license text changed, so no `SHA256SUMS` edit |
| **S8** | `grep -rn '26\.10\.3\|26\.10\.1\|1\.23\.6\|1\.1\.3\|4\.3-vcpkg' docs/ README.md` | matched the current-version statements in `docs/ARCHITECTURE.md`, `docs/BUILD.md`, `docs/BEHAVIOR.md`, `docs/HEIC-SOURCE-REVIEW.md`, `docs/DECISIONS.md`, and `docs/UPGRADE-HEIC.md`, plus historical logs under `docs/verification/`; no `README.md` hit; every current statement already names the pinned versions | none — nothing needed an update at the same version |
| **S9** | `scripts/ci.sh trust`; `cd src-tauri && cargo test trust_gate` | trust exit 0, printing `heif-artifacts: verified 4 committed archives, 3 extracted libraries` and `trust gate: PASS`; `5 passed; 0 failed`. `scripts/ci.sh all`, `scripts/ci.sh offline`, the tamper drill, and the hosted run were **not** run here | none — **partially observed**; Task 8 closes `all`/`offline` |

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

### S3 and S4 — the four committed archives, hashed and compared

S3 (record the hashes by hand) and S4 (commit the shipping archives) were run in
`/tmp/opencode/heic-hash-rehearsal/` at `2026-11-19T17:42Z`. `hash-recheck.txt`
holds the per-archive record; the commands and the values they produced:

```
$ gh release view 26.10.1 --repo vegidio/binaries-heif --json tagName,publishedAt --jq '.tagName + " " + .publishedAt'
26.10.1 2026-10-24T09:53:23Z

$ gh release download 26.10.1 --repo vegidio/binaries-heif \
    --pattern 'bundle_linux_x64.zip' --pattern 'bundle_osx_x64.zip' \
    --pattern 'bundle_osx_arm64.zip' --pattern 'bundle_windows_x64.zip' --clobber
$ wc -c bundle_linux_x64.zip bundle_osx_x64.zip bundle_osx_arm64.zip bundle_windows_x64.zip
 3325156 bundle_linux_x64.zip
 3158467 bundle_osx_x64.zip
 3036209 bundle_osx_arm64.zip
 8910372 bundle_windows_x64.zip
18430204 total

$ sha256sum bundle_linux_x64.zip bundle_osx_x64.zip bundle_osx_arm64.zip bundle_windows_x64.zip
b5cbed5d08fc79f64b5171cd401fbd62da43333d26b5ad9905354704cf072f82  bundle_linux_x64.zip
35e17dfc5c6f84a38ddd0f86edfa2a979db7aa462e03431aa5f745946b25a4e2  bundle_osx_x64.zip
18f3faa9abcc36a0ca316fde55848eb1815ca4066613c0451c498498d35dd1b1  bundle_osx_arm64.zip
5909672143ca081e24f9d1a448932abf1b1d69c869f8b6daeefbda4bc99ae8ed  bundle_windows_x64.zip

$ for a in bundle_linux_x64 bundle_osx_x64 bundle_osx_arm64 bundle_windows_x64; do
    d="extract-$a"; rm -rf "$d"; mkdir -p "$d"; unzip -q "$a.zip" -d "$d"; echo "== $a =="; \
      find "$d/lib" -maxdepth 1 -type f -printf '%p %s bytes\n'; sha256sum "$d"/lib/*
  done
== bundle_linux_x64 ==
extract-bundle_linux_x64/lib/libheif.a 12953188 bytes
1e308363b99ff5ef69711ff9a4eea3d39c3eb590ba145656e73b22efeaa390b3  extract-bundle_linux_x64/lib/libheif.a
== bundle_osx_x64 ==
extract-bundle_osx_x64/lib/libheif.a 14025464 bytes
1d25b784dfc8bf256d365f73deff0071b201b4b71bc9e77b658db45443987992  extract-bundle_osx_x64/lib/libheif.a
== bundle_osx_arm64 ==
extract-bundle_osx_arm64/lib/libheif.a 10211264 bytes
f3a403a2bbba2074a006fbf51c7374489c71a9317be4b377b711d58c71459481  extract-bundle_osx_arm64/lib/libheif.a
== bundle_windows_x64 ==
extract-bundle_windows_x64/lib/heif.lib 59118046 bytes
0afbfa132069da692d185905bb736be00ae7e45a10a8d231fa7e0611001c500a  extract-bundle_windows_x64/lib/heif.lib
```

Each archive's size and `sha256` equal both the manifest row **and** the `digest`
GitHub publishes for that asset. Each extracted library's `lib_sha256` equals the
manifest's `lib_sha256`. The library half has no published digest to appeal to:
the release publishes a digest for the archive, not for the static library inside
it, so the archive digest is the published anchor and the manifest is the only
recorded library hash. `hash-recheck.txt` states that on every library verdict
rather than implying a second published source.

S4 at this version changes nothing: the four shipping archives are already
committed, and `cmp` says the committed bytes are the downloaded bytes.

```
$ for a in bundle_linux_x64 bundle_osx_x64 bundle_osx_arm64 bundle_windows_x64; do
    if cmp -s "$a.zip" "/home/darticus/Code/work/image-converter/src-tauri/vendor/heif-bundles/$a.zip"; then
      echo "$a.zip: committed copy is byte-identical to the downloaded asset"
    else
      echo "$a.zip: committed copy differs"
    fi
  done
bundle_linux_x64.zip: committed copy is byte-identical to the downloaded asset
bundle_osx_x64.zip: committed copy is byte-identical to the downloaded asset
bundle_osx_arm64.zip: committed copy is byte-identical to the downloaded asset
bundle_windows_x64.zip: committed copy is byte-identical to the downloaded asset

$ git status --porcelain src-tauri
```

The runbook's S4 `cp` line was therefore not run: copying a file over its
identical self is a no-op. `scripts/heif-artifacts.sh verify` ran as part of S9's
`scripts/ci.sh trust`, not separately here.

**One scope difference from the runbook.** S3 downloads all eight `bundle_*`
archives. This rehearsal covers the four committed shipping archives — the set
feature 05's Task 6 names and the set `trust` certifies. The four uncommitted
archives were not downloaded; their manifest values were cross-checked against
the published metadata in S1, which is a record check, and no `lib_sha256` was
computed for them.

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

### S6 — vulnerability re-check

Run at `2026-11-19T17:44Z` against the pins in this commit. The runbook's four
NVD queries, plus the two public sources that actually index the Rust crate:

```
$ for term in libheif libde265 x265 heif-rs; do echo "== $term =="; \
    curl -sS -G --max-time 60 -w 'HTTP %{http_code}\n' \
      -H 'User-Agent: image-converter-rehearsal (darticus)' \
      --data-urlencode "keywordSearch=$term" \
      'https://services.nvd.nist.gov/rest/json/cves/2.0' -o "nvd/cve-$term.json"; done
== libheif ==
HTTP 200
== libde265 ==
HTTP 200
== x265 ==
HTTP 200
== heif-rs ==
HTTP 200

$ curl -sS -H 'Content-Type: application/json' \
    -d '{"package":{"name":"heif-rs","ecosystem":"crates.io"}}' \
    https://api.osv.dev/v1/query
{}
$ gh api '/advisories?ecosystem=rust&affects=heif-rs&per_page=20'
[]
```

Reading the four NVD files with `python3` counted `totalResults` and the number
of entries carrying a machine-readable `configurations` block:

| term | `totalResults` | with configurations |
| --- | --- | --- |
| `libheif` | 50 | 29 |
| `libde265` | 67 | 60 |
| `x265` | 3 | 3 |
| `heif-rs` | 0 | 0 |

The result, in the runbook's dated shape:

```
CVE re-check 2026-11-19, against the versions in this commit:
  libheif  1.23.6    https://services.nvd.nist.gov/rest/json/cves/2.0?keywordSearch=libheif
                    50 entries returned, 29 with configurations; no configuration range
                    includes 1.23.6 (the highest end-exclusive bound there is 1.22.1), and
                    the newer prose-only entries state upper bounds no higher than 1.23.3.
  libde265 1.1.3     https://services.nvd.nist.gov/rest/json/cves/2.0?keywordSearch=libde265
                    67 entries returned, 60 with configurations; no configuration range
                    includes 1.1.3 (the highest end-exclusive bound there is 1.1.0); the
                    newest prose-only entries say "prior to 1.1.1". One entry,
                    CVE-2026-88373, names commit 4d45a6b with no version range and is
                    recorded as unassessed.
  x265     4.3-vcpkg https://services.nvd.nist.gov/rest/json/cves/2.0?keywordSearch=x265
                    3 entries returned (2017, x265 through 2.5 and one libbpg 0.9.7 entry);
                    none reaches 4.3. The vcpkg build banner is not a version NVD maps.
  heif-rs  26.10.3   https://api.osv.dev/v1/query (crates.io package) and
                    https://api.github.com/advisories?ecosystem=rust&affects=heif-rs
                    NVD keyword: 0; OSV: none; GitHub Advisories: none.
```

Every one of the four requests returned HTTP 200 on the first attempt; no
rate-limit response was seen. The pinned versions sit above every version range
the public record states, and the `heif-rs` package has no published advisory in
either Rust-aware source.

**Limits of this observation.** NVD keyword search is not an audit: it matches
descriptions, not a dependency graph, and NVD data lags publication. The codec
archive is opaque and no scanner covers its C/C++ sources. `4.3-vcpkg` is the
banner the archive carries, not an upstream release with a `COPYING` or a CPE
entry. `CVE-2026-88373` cannot be placed against `1.1.3` from the record alone,
so it is recorded as unassessed rather than cleared. This is the F12 row's dated
observation, not a claim that the stack is free of advisories. At this
same-version rehearsal there is no previous version, so the query is an
assessment of the current pins rather than a delta; on a real bump it becomes the
delta since the version being replaced.

### S7 — notices and licenses

```
$ sha256sum -c licenses/SHA256SUMS
licenses/Apache-2.0.txt: OK
licenses/GPL-2.0.txt: OK
licenses/LGPL-3.0.txt: OK
exit: 0
```

The versions still agree with the pins. `THIRD-PARTY-NOTICES.md` §1 lists
libheif `1.23.6`, libde265 `1.1.3`, x265 `4.3-vcpkg`, and `heif-rs` `26.10.3`;
`licenses/README.md`'s mapping table lists the same four versions;
`src-tauri/vendor/heif-bundles/manifest.json` records release `26.10.1` and crate
`26.10.3`. No license text changed, so nothing in `licenses/SHA256SUMS` or the
mapping needed an edit.

### S8 — docs

```
$ grep -rn '26\.10\.3\|26\.10\.1\|1\.23\.6\|1\.1\.3\|4\.3-vcpkg' docs/ README.md
```

The run returned 140 matching lines across 17 files. The current-version
statements live in `docs/ARCHITECTURE.md` (decode row and codec payload),
`docs/BUILD.md` (archive release, vendored crate, dependency table),
`docs/BEHAVIOR.md` (a recorded `heif-convert` 1.23.6 observation),
`docs/HEIC-SOURCE-REVIEW.md` (pins, banners, Q17), `docs/DECISIONS.md` (the
encoder decision), and `docs/UPGRADE-HEIC.md` (its own header and expected-version
array). The remaining hits are historical records under `docs/verification/` —
run logs, the archive inventory, and the Task 2 gate logs — which quote the
versions as they were observed and are not statements to update. `README.md` had
no hit. Every current statement already names the pinned versions, so nothing was
edited: at the same version there is no new literal to write.

### S9 — gates, partially observed

The cheap gates that this task can run without duplicating Task 8:

```
$ scripts/ci.sh trust
==> Codec trust gate
==> Committed archives (src-tauri/vendor/heif-bundles)
ok bundle_linux_x64.zip 3325156 bytes b5cbed5d08fc79f64b5171cd401fbd62da43333d26b5ad9905354704cf072f82
ok bundle_osx_x64.zip 3158467 bytes 35e17dfc5c6f84a38ddd0f86edfa2a979db7aa462e03431aa5f745946b25a4e2
ok bundle_osx_arm64.zip 3036209 bytes 18f3faa9abcc36a0ca316fde55848eb1815ca4066613c0451c498498d35dd1b1
ok bundle_windows_x64.zip 8910372 bytes 5909672143ca081e24f9d1a448932abf1b1d69c869f8b6daeefbda4bc99ae8ed
==> Extracted libraries (src-tauri/target/*/build/heif-rs-*/out/binaries-heif-bundle-*/)
ok src-tauri/target/debug/build/heif-rs-676f8025fc455570/out/binaries-heif-bundle-26.10.1/lib/libheif.a matches bundle_linux_x64.zip (lib/libheif.a)
ok src-tauri/target/debug/build/heif-rs-7c8605279021ca0e/out/binaries-heif-bundle-26.10.1/lib/libheif.a matches bundle_linux_x64.zip (lib/libheif.a)
ok src-tauri/target/release/build/heif-rs-520d9edfbf6098c8/out/binaries-heif-bundle-26.10.1/lib/libheif.a matches bundle_linux_x64.zip (lib/libheif.a)
==> Summary
heif-artifacts: verified 4 committed archives, 3 extracted libraries
trust gate: heif-rs 26.10.3 locked from vendor/heif-rs (path package)
trust gate: PASS
exit: 0

$ cd src-tauri && cargo test trust_gate
running 5 tests
test trust_gate::the_runbook_and_the_trust_gate_are_linked_from_the_docs ... ok
test trust_gate::ci_runs_the_offline_gate_only_outside_pull_requests ... ok
test trust_gate::ci_runs_the_trust_gate_before_the_rust_and_build_gates ... ok
test trust_gate::the_trust_gate_is_part_of_the_full_local_gate ... ok
test trust_gate::upgrade_runbook_covers_every_required_step ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 83 filtered out; finished in 0.00s
exit: 0
```

**Not run here, and not claimed.** `scripts/ci.sh all` and `scripts/ci.sh
offline` are the full-gate task's (Task 8), as are the hosted run and its URL.
The tamper drill was executed and recorded once in Task 2
(`../trust-green.log`, `../tamper-archive.log`, `../tamper-library.log`,
`../pin-drift.log`); it was not repeated. S9's full command line in the runbook
stays open until Task 8 runs it, and this record says "partially observed"
rather than green.

## What this proves, and what it does not

Proven on this machine, on this date, at `8d3a6d8` for the vendor-diff half and
`3ade5e0` for the hash half:

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
5. S3's downloaded bytes and `sha256` values for the four committed archives
   equal the manifest **and** the release's published digest, and the extracted
   `lib_sha256` values equal the manifest. This is a byte check, not the S1
   record check.
6. S4's committed archives are byte-identical to the published downloads, which
   is what a same-version S4 should find.
7. S6 ran against the public record and found no advisory range that includes the
   pins; the one unmappable entry is named rather than waved through.
8. S7's license hashes and version records are unchanged and consistent, S8
   found no stale version literal, and the S9 trust gate and its five tests pass
   on the committed tree.

Not proven:

- **A version bump.** Nothing moved, so the steps that only a bump exercises —
  re-reading release notes for a *new* version, re-applying edits to a changed
  `build.rs`, a red-then-green S5, a changed notices/license record — were not
  run.
- **The four uncommitted archives.** They were not downloaded; S1 compared their
  manifest values against the published digest metadata (a record check), and no
  `lib_sha256` was computed for them. The four committed shipping archives are
  covered at the byte level.
- **The full gate and the hosted run.** `scripts/ci.sh all`, `scripts/ci.sh
  offline`, a rerun of the tamper drill, and the hosted run belong to Task 8;
  S9 here is only `trust` plus `cargo test trust_gate`. The trust gate's own
  clean run is `../trust-green.log`.
- **CVE completeness.** S6 is a dated observation of public sources, not a
  scanner, and the codec sources are not in this repository to scan. A
  commit-level entry that cannot be mapped to a released version stays
  unassessed.

## Files

- `vendor-diff.txt` — the S2 record: tarball URL, published and observed
  checksums, the extract command, `diff -rq`, the full `diff -u` for `build.rs`
  and `Cargo.toml`, and the note on the two added files.
- `hash-recheck.txt` — the S3/S4 record: the download and extraction commands,
  and one block per committed archive with downloaded bytes, `sha256`, manifest
  bytes and `sha256`, the published release digest, the extracted library path
  and size, computed and manifest `lib_sha256`, and an explicit verdict for each.
- `README.md` — this file.
- `../trust-green.log` — the clean `scripts/ci.sh trust` run, for the gate's
  own baseline.
