# Upgrading the HEIC stack

Covers `heif-rs`, currently `26.10.3` and vendored at `src-tauri/vendor/heif-rs/`
with an exact requirement, and the codec archive `26.10.1` of
`github.com/vegidio/binaries-heif` it links, pinned in
`src-tauri/vendor/heif-bundles/manifest.json`. Each archive merges libheif,
libde265, and x265 into one static library per target, so those three move
together while the crate and the archive release stay separate pins. It does not
cover the Rust and npm trees or the codec C/C++ sources, which are never built
here: the archive arrives prebuilt.

The same-version rehearsal is scheduled, not yet executed: Tasks 5–6 will
produce `docs/verification/2026-11-19/trust-gate/runbook-rehearsal/` by
re-deriving the vendored tree, every hash, and running every command below. It
changes no version, so even once run a real bump stays unproven end to end (§7).

## 1. When to upgrade

- **A security advisory** naming libheif, libde265, x265, or `heif-rs` on a path
  this product uses. S6 is that check.
- **An upstream decode bug that affects us**: a fix for a container or bit depth
  the app sends through. Name the file class in the log row.
- **End of life**: the wrapper or a carried codec version stops being maintained.
- **Never a cosmetic bump.** A new version number does not justify moving the
  trust record; the cost is a full source re-read (S5) and a re-recorded pin.
- **A bindings API change is a stage-level decision, not a runbook bump.** If the
  types the seam calls change, `src-tauri/src/heic.rs` moves too: feature spec.

## 2. What an upgrade touches

| Piece | Where | What changes on a bump |
| --- | --- | --- |
| Crate, vendor tree | `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/vendor/heif-rs/`, `VENDORED.md` | the exact requirement, the lock entry, new upstream files, and the four-path fork delta |
| Manifest, archives | `src-tauri/vendor/heif-bundles/manifest.json`, `bundle_*.zip` | the `release`, URL template, and each moved row's `bytes`, `sha256`, `lib_sha256`; the four committed archives |
| Source review | `docs/HEIC-SOURCE-REVIEW.md` | pins, read log, call surface, `unsafe` register, questions, findings, CVE row |
| Notices, licenses | `THIRD-PARTY-NOTICES.md`, `licenses/README.md`, `licenses/SHA256SUMS` | component versions and evidence; the mapping, and a text hash only if a text changed |
| Docs, literals | `docs/BUILD.md`, `docs/ARCHITECTURE.md`, `docs/README.md`, this file; `scripts/ci.sh`, `src-tauri/src/codec_artifacts.rs`, `src-tauri/src/trust_gate.rs`, `src-tauri/src/source_review.rs` | every stated version, path, and count; the expected lock version, the pinned manifest values, and `review_records_the_pinned_provenance`'s expected-version array (`26.10.3`, `26.10.1`, `1.23.6`) |

## 3. Pre-flight

```bash
git status --porcelain          # must print nothing
git switch -c heic-upgrade-<version>
```

Tools: `git`, `cargo`, `python3`, `sha256sum`, `unzip`, `gh`. S1–S3 and S9 need
the network. Disk: ~18 MB committed, 48 MB of scratch, and a cold build
(`scripts/ci.sh offline` needs 8 GiB free and honors `CI_OFFLINE_DIR`).

**The hash rule.** Cross-check a checksum against the digest metadata the release
publishes for the asset — GitHub's `digest` field or a checksum file — never
against the downloaded file's own output.

## 4. Steps S0–S9

### S0 — record the current state
```bash
python3 -c 'import json; m=json.load(open("src-tauri/vendor/heif-bundles/manifest.json")); print(m["release"], m["crate"])'
sha256sum src-tauri/vendor/heif-bundles/bundle_*.zip    # the four archive hashes
sed -n '/## 11. Findings/,/## 12./p' docs/HEIC-SOURCE-REVIEW.md | grep 'open obligation'
```
Open the §8 row now; it closes at the last gate.

### S1 — find and read the new release
```bash
gh release view <release> --repo vegidio/binaries-heif --json tagName,body,assets \
  --jq '.assets[] | [.name, .size, .digest] | @tsv'
curl -sSfL https://crates.io/api/v1/crates/heif-rs/<version> \
  | python3 -c 'import json,sys; v=json.load(sys.stdin)["version"]; print(v["num"], v["checksum"])'
```
Read both release notes; record the tag, crate version, published digests and
sizes, and the crate tarball's `checksum`.

### S2 — re-vendor the crate
```bash
curl -sSfL -o /tmp/heif-upgrade/heif-rs-<version>.crate \
  https://crates.io/api/v1/crates/heif-rs/<version>/download
sha256sum /tmp/heif-upgrade/heif-rs-<version>.crate   # must equal S1's checksum
tar -xf /tmp/heif-upgrade/heif-rs-<version>.crate -C /tmp/heif-upgrade
rsync -a --delete --exclude build.rs --exclude build_support.rs \
  --exclude VENDORED.md --exclude Cargo.toml \
  /tmp/heif-upgrade/heif-rs-<version>/ src-tauri/vendor/heif-rs/
diff -r /tmp/heif-upgrade/heif-rs-<version> src-tauri/vendor/heif-rs
sed -i 's/^heif-rs = "=.*"$/heif-rs = "=<version>"/' src-tauri/Cargo.toml
(cd src-tauri && cargo update -p heif-rs)
```
Re-apply the two edits `VENDORED.md` records — the `[build-dependencies]` entries
and the `build.rs` replacement — after diffing the fork's `build.rs` and
`build_support.rs` against the tarball. `diff -r` must report exactly those four
paths; update that table, and bump the expected version literal in
`scripts/ci.sh`, which does not read `Cargo.toml`.

### S3 — record the hashes by hand
```bash
cd /tmp/heif-upgrade
gh release download <release> --repo vegidio/binaries-heif --pattern 'bundle_*.zip'
sha256sum bundle_*.zip && wc -c bundle_*.zip
for a in bundle_*.zip; do d="extract-${a%.zip}"; unzip -q "$a" -d "$d" && sha256sum "$d"/lib/*; done
```
Stop at the first archive whose `sha256` or byte count disagrees with S1's
published digest and size. No published digest covers the library inside, so
`lib_sha256` is measured from the extraction after the archive's digest matched.
Edit `manifest.json` by hand: **no script writes a hash**, and a hash never comes
from the downloaded file's own output.

### S4 — commit the four shipping archives
```bash
cp /tmp/heif-upgrade/bundle_{linux_x64,osx_x64,osx_arm64,windows_x64}.zip \
  src-tauri/vendor/heif-bundles/
scripts/heif-artifacts.sh verify
```
The other four stay `committed: false`; if the set changes, the flags, the
expected count in `scripts/ci.sh trust`, and the archives move together.

### S5 — redo the source review against the new bytes
```bash
(cd src-tauri && cargo test source_review)
sha256sum src-tauri/vendor/heif-bundles/bundle_linux_x64.zip
```
The tests fail until `docs/HEIC-SOURCE-REVIEW.md` covers every file, `unsafe`
site, and seeded question again. Update `review_records_the_pinned_provenance`'s
expected-version array in `src-tauri/src/source_review.rs` (`26.10.3`, `26.10.1`,
`1.23.6`) in the same commit, or the bump fails S5. Re-check F05–F08, F10, and
F12 against the new bytes.

### S6 — vulnerability re-check
```bash
for term in libheif libde265 x265 heif-rs; do
  curl -sSfL "https://services.nvd.nist.gov/rest/json/cves/2.0?keywordSearch=$term" \
    -o "/tmp/heif-upgrade/cve-$term.json"
done
```
Record it in the review's F12 row and the §8 row, dated, sourced, and in this
shape; reading the release notes does not satisfy it:

```
CVE re-check 2026-11-19, against the versions in this commit:
  libheif  <version>  <advisory URL>  <none reachable | IDs + assessment>
  libde265 <version>  <advisory URL>  <...>
  x265     <version>  <advisory URL>  <...>
  heif-rs  <version>  <advisory URL>  <...>
```

### S7 — notices and licenses
```bash
sha256sum -c licenses/SHA256SUMS
```
Update versions and quoted evidence in `THIRD-PARTY-NOTICES.md`, the mapping in
`licenses/README.md`, and `licenses/SHA256SUMS` only if a text changed; a bump
does not change a license text.

### S8 — docs
```bash
grep -rn '26\.10\.3\|26\.10\.1\|1\.23\.6\|1\.1\.3\|4\.3-vcpkg' docs/ README.md   # update BUILD.md, ARCHITECTURE.md, README.md, this file
```

### S9 — gates
```bash
scripts/ci.sh trust && scripts/ci.sh rust && scripts/ci.sh build && scripts/ci.sh offline
```
Then the §5 tamper drill, then push and record the hosted run URL, job names, and
timings in §8. `all` runs trust first; `offline` stays separate.

## 5. Verification

On a good tree, `scripts/ci.sh trust` prints `heif-artifacts: verified 4
committed archives, <m> extracted libraries`, `trust gate: heif-rs <version>
locked from vendor/heif-rs (path package)`, and `trust gate: PASS`; the asserted
count means a verifier that checked nothing cannot pass. `rust` keeps rustfmt
silent, `build` links the committed archive, and `offline` prints `offline gate: PASS (HEAD <sha>, isolation: <wrapper>)`.

Then break each pin, watch it fail naming the file, and restore:

```bash
printf 'x' >> src-tauri/vendor/heif-bundles/bundle_linux_x64.zip
scripts/ci.sh trust      # `failed its checksum`, expected/actual, and a fix line
git restore src-tauri/vendor/heif-bundles/bundle_linux_x64.zip
```

The lock edit, requirement edit, and corrupted extracted library follow the same
shape; all four breaks are in `docs/verification/2026-11-19/trust-gate/`.

## 6. Rollback

One commit, so a rollback is a revert; archives and manifest revert together:

```bash
git revert --no-edit <bump commit>
rm -rf src-tauri/target/*/build/heif-rs-*
scripts/ci.sh trust
```
The `rm -rf` is not optional: the extracted tree is named for the codec release
that built it, and the gate hashes every library it finds against the reverted
manifest, so a stale extraction makes a correct revert look broken. The `trust`
run afterwards, ending in `trust gate: PASS`, is the proof.

## 7. What the gate cannot catch

- **A manifest and its bytes edited in the same commit.** The gate compares two things one commit can change together, so the reviewed commit is the control.
- **The registry-source diff.** CI never diffs `src-tauri/vendor/heif-rs/` against
  the published crate; only the rehearsal does, at one version.
- **CVE data freshness.** S6 is a dated observation, not a scanner; an advisory
  published the day after is invisible until the next bump, and the C/C++ codec
  sources are never scanned at all.
- **Any legal judgement.** Nothing here reads a license or decides whether a
  relink duty is met; `licenses/SHA256SUMS` checks the text, not the judgement.

## 8. Upgrade log

One row per bump, appended and never rewritten; the first is the scheduled rehearsal, filled in by Tasks 5–6.

| date | from | to | archive release | reviewer | trust | offline | hosted run | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-11-19 | `26.10.3` / `26.10.1` | `26.10.3` / `26.10.1` | `26.10.1` | feature-05 builder | not yet run | prior run PASS (`dfb2fe7`, 2026-11-18) | not attempted | rehearsal (no version change), not yet run |

## 9. Ownership

`feature-05-trust-gate` owns this runbook, including the literals the drift test
in `src-tauri/src/trust_gate.rs` pins, and the rule that every bump appends a row
to §8 in the same commit. A bump without a row is incomplete.
