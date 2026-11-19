#!/usr/bin/env bash
#
# Verify the pinned HEIC codec artifacts, and fetch the ones that are not committed.
#
# The pin lives in src-tauri/vendor/heif-bundles/manifest.json: each bundle_* archive carries the byte length and
# SHA-256 the build script enforces before it extracts anything, plus the SHA-256 of the static library inside it.
# This script re-checks both layers on demand:
#
#   1. every `committed: true` archive in src-tauri/vendor/heif-bundles/ against its recorded `bytes` and `sha256`;
#   2. every library extracted under any `*/build/heif-rs-*/out/binaries-heif-bundle-*/`, against the manifest's
#      recorded `lib_sha256`. Discovery is a `find` over `src-tauri/target` and, when it is set, `$CARGO_TARGET_DIR`, so
#      the plain layout (`target/<profile>/build/...`), the `--target` layout (`target/<triple>/<profile>/build/...`) and
#      deeper layouts cargo may add are all covered; a single glob only ever saw the plain one. Each root is resolved
#      with its symlinks first, because `find` does not descend a symlinked starting point, and a `find` that fails is
#      fatal: a skipped subtree must never pass as a cold cache. A root path containing a newline is refused (the script
#      cannot print it unambiguously), and `find` output is read NUL-delimited, so a bundle directory whose own name
#      contains a newline is checked instead of being merged with a neighbour or dropped. The cache directory is
#      named for the codec release and the archive hash, never for the target, so the library is matched by hash instead
#      of being attributed by name — linux x64 and arm64 both ship `lib/libheif.a`, and all eight `lib_sha256` values are
#      distinct;
#   3. every non-directory entry (file or symlink) in each extracted `include/` tree, against the same member of the
#      archive whose library matched that directory — `heif.h` is only the entry point, and bindgen parses all 17 sibling
#      headers it includes. The archive is the only trusted source for the tree: `target/` is writable by anything on
#      the machine, and a poisoned sibling header redefines the FFI types while every library hash stays green. An
#      extracted entry the archive does not hold is a failure too;
#   4. every `out/bindings.rs` that has a `out/bindings.rs.sha256` written next to it by the build script, against that
#      recorded digest. The build regenerates the bindings on each run; this catches a copy edited afterwards, and a
#      missing sidecar is a failure rather than a skip.
#
# A bundle directory that exists but holds none of the manifest's libraries is a half-extracted cache: that is a
# failure, because the build cannot link it. A `heif-rs-*/out/` directory with no bundle directory under it is a failure
# too: the build ran and left something unattestable there (a `HEIF_BINARIES_DIR` build lands exactly here). Only the
# complete absence of both is a cold cache, which passes and says so in its own words.
#
# `CARGO_TARGET_DIR` is honoured rather than ignored: cargo writes the build there *instead of* the default tree, so a
# set variable adds that tree to the scan (the default tree is still scanned). A relative value is resolved against the
# repository root here — this script runs from the root, while cargo resolves it from `src-tauri`. A set value that is
# not a readable directory is a refusal, not a skip: skipping it would print a green cold cache for the tree the build
# actually used.
#
# `verify` only reads — the manifest, the committed archives, and whatever is extracted under target/. `fetch` writes
# bytes, and only into the bundle directory; it deletes any download that does not match the manifest row rather than
# laundering a bad artifact into the tree.

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/heif-artifacts.sh <subcommand>

Subcommands:
  verify             Check every committed archive and every extracted codec library, include/ tree, and generated
                     bindings file against the pinned manifest
  fetch <archive>    Download one archive from the pinned release into src-tauri/vendor/heif-bundles/, accepting it
                     only if its byte length and SHA-256 match the manifest row

Options:
  -h, --help  Show this help and exit

`verify` exits 0 on a clean tree, including a cold cache with nothing extracted yet. It exits 1 on any mismatch,
naming the file and the pinned hash. It scans src-tauri/target, and $CARGO_TARGET_DIR too when that is set (a
relative value resolves against the repository root; an unreadable one is refused). `fetch` never edits the
manifest: the hashes are the authority.
EOF
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
cd "$repo_root"

bundle_dir="src-tauri/vendor/heif-bundles"
manifest="$bundle_dir/manifest.json"

die() {
  printf '%s\n' "$*" >&2
  exit 1
}

# `fetch` stages its download under a temporary name. The path lives in this script-level variable, and the trap that
# removes it is installed here once, because a trap cannot see a variable that only ever existed in a caller's local
# scope: on a `set -e` exit out of `fetch` such a trap would find nothing and the partial file would be left behind.
fetch_tmp=""
# `find_output` holds one NUL-delimited `find` listing at a time. NUL is the only separator that survives a directory
# name containing a newline, and bash cannot store NUL in a variable, so the listing lives in this temporary file.
# The trap below removes whichever of the two exists on every exit path.
find_output=""
cleanup_temp_files() {
  if [[ -n "$fetch_tmp" && -e "$fetch_tmp" ]]; then
    rm -f -- "$fetch_tmp"
  fi
  if [[ -n "${find_output-}" && -e "$find_output" ]]; then
    rm -f -- "$find_output"
  fi
}
trap cleanup_temp_files EXIT

# One SHA-256 implementation, picked from what the machine has: coreutils on Linux, perl's shasum elsewhere.
sha256_tool=""
require_sha256_tool() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256_tool=sha256sum
  elif command -v shasum >/dev/null 2>&1; then
    sha256_tool=shasum
  else
    die "heif-artifacts: neither sha256sum nor shasum is on PATH; cannot verify checksums"
  fi
}

sha256_of() {
  case "$sha256_tool" in
    sha256sum) sha256sum -- "$1" | cut -d' ' -f1 ;;
    shasum) shasum -a 256 -- "$1" | cut -d' ' -f1 ;;
    *) die "heif-artifacts: no SHA-256 tool selected" ;;
  esac
}

# Every regular file under `include/` in an archive, as `path<TAB>sha256` rows sorted by path. python3's stdlib
# zipfile is used because the checksum tools above cannot see inside a zip, python3 is already required to read the
# manifest, and jq would be a new CI dependency. Exits 1 with a message when the archive cannot be read.
zip_include_sha256s() {
  python3 - "$1" <<'PY'
import hashlib
import sys
import zipfile

archive = sys.argv[1]
try:
    with zipfile.ZipFile(archive) as bundle:
        rows = []
        for info in bundle.infolist():
            name = info.filename
            if info.is_dir() or not name.startswith("include/"):
                continue
            digest = hashlib.sha256()
            with bundle.open(info) as fh:
                for chunk in iter(lambda: fh.read(1 << 20), b""):
                    digest.update(chunk)
            rows.append((name, digest.hexdigest()))
except (OSError, KeyError, zipfile.BadZipFile) as exc:
    sys.exit(f"heif-artifacts: cannot read {archive}: {exc}")

for name, digest in sorted(rows):
    print(f"{name}\t{digest}")
PY
}

# python3 reads the manifest because it is a JSON parser every supported machine already has: the ubuntu-24.04
# GitHub runner ships it, and so does Arch. jq is not guaranteed there, so it would become a CI dependency.
require_json_reader() {
  command -v python3 >/dev/null 2>&1 ||
    die "heif-artifacts: python3 is required to read $manifest, but it is not on PATH"
}

# Prints the manifest's artifacts as tab-separated rows:
#   archive, committed, bytes, sha256, lib, lib_sha256
# A malformed manifest is an error, never a silently skipped check.
manifest_rows() {
  python3 - "$manifest" <<'PY'
import json
import sys

path = sys.argv[1]
try:
    with open(path, encoding="utf-8") as fh:
        manifest = json.load(fh)
except OSError as exc:
    sys.exit(f"heif-artifacts: cannot read {path}: {exc}")
except json.JSONDecodeError as exc:
    sys.exit(f"heif-artifacts: {path} is not valid JSON: {exc}")

artifacts = manifest.get("artifacts") if isinstance(manifest, dict) else None
if not isinstance(artifacts, list) or not artifacts:
    sys.exit(f"heif-artifacts: {path} has no artifacts list")

fields = ("archive", "committed", "bytes", "sha256", "lib", "lib_sha256")
for index, artifact in enumerate(artifacts):
    if not isinstance(artifact, dict):
        sys.exit(f"heif-artifacts: {path}: artifact {index} is not an object")
    for name in fields:
        if name not in artifact:
            sys.exit(f"heif-artifacts: {path}: artifact {index} has no `{name}`")
    if not isinstance(artifact["archive"], str) or not artifact["archive"]:
        sys.exit(f"heif-artifacts: {path}: artifact {index} has an empty `archive`")
    if not isinstance(artifact["committed"], bool):
        sys.exit(f"heif-artifacts: {path}: {artifact['archive']} has a non-boolean `committed`")
    if not isinstance(artifact["bytes"], int) or artifact["bytes"] <= 0:
        sys.exit(f"heif-artifacts: {path}: {artifact['archive']} has an invalid `bytes`")
    for name in ("sha256", "lib_sha256"):
        value = artifact[name]
        if not isinstance(value, str) or len(value) != 64:
            sys.exit(f"heif-artifacts: {path}: {artifact['archive']} has an invalid `{name}`")
        try:
            int(value, 16)
        except ValueError:
            sys.exit(f"heif-artifacts: {path}: {artifact['archive']} has a non-hex `{name}`")
    if not isinstance(artifact["lib"], str) or not artifact["lib"]:
        sys.exit(f"heif-artifacts: {path}: {artifact['archive']} has an empty `lib`")

    print(
        "\t".join(
            [
                artifact["archive"],
                "true" if artifact["committed"] else "false",
                str(artifact["bytes"]),
                artifact["sha256"],
                artifact["lib"],
                artifact["lib_sha256"],
            ]
        )
    )
PY
}

# Prints the release and the download URL template, tab-separated: release, url_template
manifest_meta() {
  python3 - "$manifest" <<'PY'
import json
import sys

path = sys.argv[1]
try:
    with open(path, encoding="utf-8") as fh:
        manifest = json.load(fh)
except OSError as exc:
    sys.exit(f"heif-artifacts: cannot read {path}: {exc}")
except json.JSONDecodeError as exc:
    sys.exit(f"heif-artifacts: {path} is not valid JSON: {exc}")

release = manifest.get("release")
template = manifest.get("url_template")
if not isinstance(release, str) or not release:
    sys.exit(f"heif-artifacts: {path} has no `release`")
if not isinstance(template, str) or "{release}" not in template or "{archive}" not in template:
    sys.exit(f"heif-artifacts: {path} has a `url_template` without {{release}} and {{archive}}")
print(f"{release}\t{template}")
PY
}

file_bytes() {
  wc -c <"$1" | tr -d ' '
}

# Lowercases a hex digest for comparison. Bash 4's built-in lowercase expansion is not in the 3.2 `/bin/bash` macOS
# ships, where it aborts the run with `bad substitution` partway through, so the fold goes through `tr` here. LC_ALL=C
# keeps it to ASCII, which is all a hex digest can contain, and the folded copy is only ever compared: the recorded and
# measured hashes are printed exactly as they were found.
lower() {
  printf '%s' "$1" | LC_ALL=C tr '[:upper:]' '[:lower:]'
}

# The build script's wording for a checksum failure, so the two layers report the same way.
report_checksum_failure() {
  local subject="$1" expected="$2" actual="$3" archive="$4"
  printf 'heif-rs: %s failed its checksum\n' "$subject" >&2
  printf '  expected sha256 %s\n' "$expected" >&2
  printf '  actual   sha256 %s\n' "$actual" >&2
  printf '  fix: scripts/heif-artifacts.sh fetch %s\n' "$archive" >&2
}

# Layer three: every file under the extracted `include/` tree must be the archive's copy of that file. `heif.h` is only
# the entry point — bindgen parses every sibling header it includes, so a sibling is as load-bearing as the root. The
# tree lives under target/, so it is re-derived from the archive every run instead of being trusted, and a file the
# archive does not hold fails rather than passing unseen. A directory whose archive is not checked locally
# (committed: false) is reported as unattested rather than passed: an archive the manifest does not check cannot vouch
# for anything. Increments `failures` in the calling `verify`.
check_extracted_headers() {
  local dir="$1" archive="$2" committed="$3"
  local root="${dir%/}"
  local archive_path="$bundle_dir/$archive"

  if [[ "$committed" != "true" ]]; then
    printf 'note: %s/include cannot be attested: %s is committed: false, so its bytes are not checked against the manifest\n' \
      "$root" "$archive"
    return 0
  fi
  if [[ ! -f "$archive_path" ]]; then
    # Layer one already failed for this archive; do not count the same missing file twice.
    printf 'note: %s/include cannot be attested: %s is missing from %s\n' "$root" "$archive" "$bundle_dir"
    return 0
  fi

  local failures_before="$failures"
  local listing=""
  if ! listing="$(zip_include_sha256s "$archive_path")"; then
    failures=$((failures + 1))
    return 0
  fi
  if [[ -z "$listing" ]]; then
    printf 'heif-rs: %s has no headers: %s holds no include/ entries\n' "$root/include" "$archive" >&2
    printf '  expected the bundle header tree under include/\n' >&2
    printf '  actual   none in %s\n' "$archive" >&2
    printf '  fix: delete %s to force a clean extraction\n' "$root" >&2
    failures=$((failures + 1))
    return 0
  fi
  if [[ ! -d "$root/include" ]]; then
    printf 'heif-rs: %s is missing from the extracted cache\n' "$root/include" >&2
    printf '  expected the header tree %s carries (%s files)\n' "$archive" \
      "$(printf '%s\n' "$listing" | wc -l | tr -d ' ')" >&2
    printf '  actual   no include/ directory under %s\n' "$root" >&2
    printf '  fix: delete %s to force a clean extraction\n' "$root" >&2
    failures=$((failures + 1))
    return 0
  fi

  local member expected header actual checked=0
  while IFS=$'\t' read -r member expected; do
    [[ -n "$member" ]] || continue
    checked=$((checked + 1))
    header="$root/$member"
    if [[ ! -f "$header" ]]; then
      printf 'heif-rs: %s is missing from the extracted cache\n' "$header" >&2
      printf '  expected sha256 %s (%s)\n' "$expected" "$archive" >&2
      printf '  actual   sha256 (no such file)\n' >&2
      printf '  fix: delete %s to force a clean extraction\n' "$root" >&2
      failures=$((failures + 1))
      continue
    fi
    actual="$(sha256_of "$header")"
    if [[ "$actual" != "$expected" ]]; then
      printf 'heif-rs: %s failed its checksum\n' "$header" >&2
      printf '  expected sha256 %s (%s)\n' "$expected" "$archive" >&2
      printf '  actual   sha256 %s\n' "$actual" >&2
      printf '  fix: delete %s to force a clean extraction\n' "$root" >&2
      failures=$((failures + 1))
    fi
  done <<<"$listing"

  # The extracted tree must not hold a header the archive does not: the cache should be exactly one extraction, and a
  # check that only looked one way would bless an unexpected file. Every non-directory entry counts, not just regular
  # files: the build reads anything that is not a directory (`collect_include_files`), so a symlink slipped into the tree
  # must fail here too instead of passing a `-type f` gate the build rejects.
  local member_set
  member_set=$'\n'"$(printf '%s\n' "$listing" | cut -f1)"$'\n'
  local extracted_listing extracted
  extracted_listing="$(cd "$root" && find include -mindepth 1 ! -type d | LC_ALL=C sort)"
  while IFS= read -r extracted; do
    [[ -n "$extracted" ]] || continue
    if [[ "$member_set" == *$'\n'"$extracted"$'\n'* ]]; then
      continue
    fi
    actual="(not a regular file)"
    if [[ -f "$root/$extracted" ]]; then
      actual="$(sha256_of "$root/$extracted" 2>/dev/null)" || actual="(cannot read)"
    fi
    printf 'heif-rs: %s is not part of %s\n' "$root/$extracted" "$archive" >&2
    printf '  expected sha256 (no such file in the archive)\n' >&2
    printf '  actual   sha256 %s\n' "$actual" >&2
    printf '  fix: delete %s to force a clean extraction\n' "$root" >&2
    failures=$((failures + 1))
  done <<<"$extracted_listing"

  if [[ "$failures" -eq "$failures_before" ]]; then
    printf 'ok %s matches %s (%s headers)\n' "$root/include" "$archive" "$checked"
  fi
}

# Layer four: the generated bindings and the digest the build script recorded when it wrote them. A `bindings.rs`
# without a digest sidecar cannot be attested and is a failure — deleting the sidecar must not turn a tampered file into
# a pass. A directory with no `bindings.rs` at all has no bindgen output to check and is skipped. Increments `failures`
# in the calling `verify`.
check_bindings() {
  local out_dir="$1"
  local bindings="$out_dir/bindings.rs"
  local recorded_file="$out_dir/bindings.rs.sha256"
  [[ -f "$bindings" ]] || return 0

  local recorded=""
  if [[ -f "$recorded_file" ]]; then
    recorded="$(tr -d '[:space:]' <"$recorded_file")"
  fi

  local actual
  actual="$(sha256_of "$bindings")"
  if [[ -z "$recorded" ]]; then
    printf 'heif-rs: %s cannot be attested: %s is missing or empty\n' "$bindings" "$recorded_file" >&2
    printf '  expected sha256 (recorded next to the file by the build script)\n' >&2
    printf '  actual   sha256 %s\n' "$actual" >&2
    printf '  fix: delete %s and rebuild (cd src-tauri && cargo build --locked); the build regenerates it\n' \
      "$bindings" >&2
    failures=$((failures + 1))
    return 0
  fi

  if [[ "$actual" != "$recorded" ]]; then
    printf 'heif-rs: %s failed its checksum\n' "$bindings" >&2
    printf '  expected sha256 %s (recorded in %s)\n' "$recorded" "$recorded_file" >&2
    printf '  actual   sha256 %s\n' "$actual" >&2
    printf '  fix: delete %s and rebuild (cd src-tauri && cargo build --locked); the build regenerates it\n' \
      "$bindings" >&2
    failures=$((failures + 1))
    return 0
  fi

  printf 'ok %s matches its recorded sha256\n' "$bindings"
}

# Target trees to scan for build-script output. The default tree is always scanned; a set CARGO_TARGET_DIR is scanned as
# well, because cargo writes there *instead of* the default tree and a warm default must not be the only thing checked.
# A relative CARGO_TARGET_DIR resolves against the repository root here (the script's cwd), while cargo resolves it from
# src-tauri; the refusal message says so. Roots are stored with their symlinks resolved, and a root whose path contains a
# newline or a carriage return is refused: the script cannot print such a path unambiguously, and attesting or refusing
# is its whole job.
#
# Lists are arrays walked by index, and `find` output is NUL-delimited. A directory name may itself contain a newline, so
# any newline-delimited list would either drop such an entry or merge it with a neighbour — the gate would then certify a
# tree it never walked. NUL cannot live in a shell variable, so each `find` run writes its NUL records to a temporary
# file that the EXIT trap removes.
scan_roots=()
bundle_dirs=()
out_dirs=()

# `pwd -P` appends a newline, and capturing its output strips trailing newlines — which a directory name may itself end
# with. Every physical-path lookup therefore ends its output with a \001 sentinel inside the substitution, removes
# exactly `newline + sentinel`, and stores the result in a global. No caller captures the path again, so no name is ever
# trimmed. The sentinel is a byte a target path cannot contain.
resolved_root=""
physical_path=""

resolve_root() {
  local root="$1" raw
  if raw="$(cd -- "$root" 2>/dev/null && { pwd -P; printf '\001'; })"; then
    resolved_root="${raw%$'\n'$'\001'}"
  else
    resolved_root="$root"
    return 0
  fi
  # The path handed to `find` is the resolved one: `find` defaults to `-P` and does not descend a symlinked starting
  # point, so a lexical root can walk nothing while the tree sits right there. Inside the repository the path stays
  # relative, to keep messages readable; anything outside stays absolute.
  case "$resolved_root" in
    "$repo_root"/*) resolved_root="${resolved_root#"$repo_root"/}" ;;
  esac
}

# Sets `physical_path` to the physical identity of a directory, so one tree reached by two spellings (a relative root and
# an absolute one, or a root nested inside another) is checked and counted once. Falls back to the spelling given.
physical_path_of() {
  local raw
  if raw="$(cd -- "$1" 2>/dev/null && { pwd -P; printf '\001'; })"; then
    physical_path="${raw%$'\n'$'\001'}"
  else
    physical_path="$1"
  fi
}

# A scan root the script cannot show the reader is a refusal, not a guess: a path with a newline cannot be told apart
# from two paths in any line-based message.
refuse_unrepresentable_root() {
  local subject="$1" path="$2"
  case "$path" in
    *$'\n'* | *$'\r'*)
      die "heif-artifacts: $subject contains a newline; cannot scan a path this script cannot represent"
      ;;
  esac
}

collect_scan_roots() {
  local value="${CARGO_TARGET_DIR-}" root resolved key existing
  local i count

  scan_roots=()

  # A file or other entry where the default tree belongs is a broken checkout, not an empty cache.
  if [[ -e "src-tauri/target" && ! -d "src-tauri/target" ]]; then
    die "heif-artifacts: src-tauri/target exists but is not a directory; refusing to report a cold cache"
  fi
  if [[ -d "src-tauri/target" ]]; then
    resolve_root "src-tauri/target"
    refuse_unrepresentable_root "the default target root" "$resolved_root"
    scan_roots+=("$resolved_root")
  fi

  [[ -n "$value" ]] || return 0
  refuse_unrepresentable_root "CARGO_TARGET_DIR" "$value"
  case "$value" in
    /*) root="$value" ;;
    *) root="$repo_root/$value" ;;
  esac

  # Refusing here, rather than skipping the unusable tree, is the point: skipping it would print a green cold cache for
  # exactly the tree the build wrote to.
  if [[ ! -d "$root" || ! -r "$root" || ! -x "$root" ]]; then
    die "heif-artifacts: CARGO_TARGET_DIR is set to '$value' but '$root' is not a readable directory; refusing to report a cold cache. A relative value resolves against the repository root ($repo_root); cargo resolves it from src-tauri."
  fi

  resolve_root "$root"
  resolved="$resolved_root"
  refuse_unrepresentable_root "CARGO_TARGET_DIR (resolved to '$resolved')" "$resolved"

  # Same tree, two spellings: keep one. Nested roots are left to the per-entry dedupe below.
  physical_path_of "$root"
  key="$physical_path"
  i=0
  count=${#scan_roots[*]}
  while [[ "$i" -lt "$count" ]]; do
    existing="${scan_roots[$i]}"
    physical_path_of "$existing"
    if [[ "$physical_path" == "$key" ]]; then
      return 0
    fi
    i=$((i + 1))
  done
  scan_roots+=("$resolved")
}

# Collects every extracted bundle directory under the scanned roots into `bundle_dirs`, one entry per physical
# directory. A `find`, not a glob: the plain layout (`target/<profile>/build/...`), the `--target` layout
# (`target/<triple>/<profile>/build/...`) and anything deeper cargo invents all match. `-name` matches the directory's
# own name, and the trailing `! -path` keeps the result to direct children of `out/` — a `-path` pattern ending in `*`
# would also match `.../binaries-heif-bundle-*/lib`. Roots that do not exist (a never-built checkout) contribute nothing.
# A failed `find` is fatal: "found none" must only ever mean "walked successfully and found none", so an unreadable
# subtree cannot shrink the scan and pass as a cold cache.
discover_bundle_dirs() {
  local root entry key status dupe i count j seen_count
  local -a seen_keys=()

  bundle_dirs=()
  i=0
  count=${#scan_roots[*]}
  while [[ "$i" -lt "$count" ]]; do
    root="${scan_roots[$i]}"
    i=$((i + 1))
    [[ -d "$root" ]] || continue

    find_output="$(mktemp "${TMPDIR:-/tmp}/heif-find.XXXXXX")" ||
      die "heif-artifacts: cannot create a temporary file under ${TMPDIR:-/tmp} to scan $root"
    find "$root" -type d \
      -name 'binaries-heif-bundle-*' \
      -path '*/build/heif-rs-*/out/binaries-heif-bundle-*' \
      ! -path '*/build/heif-rs-*/out/binaries-heif-bundle-*/*' \
      -print0 >"$find_output" || {
      status=$?
      rm -f -- "$find_output"
      find_output=""
      die "heif-artifacts: cannot scan $root: find exited $status"
    }

    while IFS= read -r -d '' entry; do
      physical_path_of "$entry"
      key="$physical_path"
      dupe=0
      seen_count=${#seen_keys[*]}
      j=0
      while [[ "$j" -lt "$seen_count" ]]; do
        if [[ "${seen_keys[$j]}" == "$key" ]]; then
          dupe=1
          break
        fi
        j=$((j + 1))
      done
      if [[ "$dupe" -eq 0 ]]; then
        seen_keys+=("$key")
        bundle_dirs+=("$entry")
      fi
    done <"$find_output"

    rm -f -- "$find_output"
    find_output=""
  done
}

# Collects every `heif-rs` build-script `out/` directory under the scanned roots into `out_dirs`. An `out/` with no
# bundle directory under it is not a cold cache — the build ran and left output that cannot be attested — so `verify`
# needs this list as well as the bundle list. A failed `find` is fatal here too, and the NUL handling is the same.
discover_out_dirs() {
  local root entry key status dupe i count j seen_count
  local -a seen_keys=()

  out_dirs=()
  i=0
  count=${#scan_roots[*]}
  while [[ "$i" -lt "$count" ]]; do
    root="${scan_roots[$i]}"
    i=$((i + 1))
    [[ -d "$root" ]] || continue

    find_output="$(mktemp "${TMPDIR:-/tmp}/heif-find.XXXXXX")" ||
      die "heif-artifacts: cannot create a temporary file under ${TMPDIR:-/tmp} to scan $root"
    find "$root" -type d -name out -path '*/build/heif-rs-*/out' -print0 >"$find_output" || {
      status=$?
      rm -f -- "$find_output"
      find_output=""
      die "heif-artifacts: cannot scan $root: find exited $status"
    }

    while IFS= read -r -d '' entry; do
      physical_path_of "$entry"
      key="$physical_path"
      dupe=0
      seen_count=${#seen_keys[*]}
      j=0
      while [[ "$j" -lt "$seen_count" ]]; do
        if [[ "${seen_keys[$j]}" == "$key" ]]; then
          dupe=1
          break
        fi
        j=$((j + 1))
      done
      if [[ "$dupe" -eq 0 ]]; then
        seen_keys+=("$key")
        out_dirs+=("$entry")
      fi
    done <"$find_output"

    rm -f -- "$find_output"
    find_output=""
  done
}

# Names what a bundle-named entry actually is, so the failure says "symlink", not the false "no directory".
entry_kind() {
  if [[ -L "$1" ]]; then
    printf 'symlink to %s' "$(readlink "$1")"
  elif [[ -f "$1" ]]; then
    printf 'a regular file'
  else
    printf 'a special file'
  fi
}

verify() {
  require_sha256_tool
  require_json_reader

  local rows
  rows="$(manifest_rows)" || exit 1

  local archive committed bytes sha256 lib lib_sha256
  local path actual_bytes actual_sha failures=0 archives_checked=0 libs_checked=0
  local expected_list matched matched_committed actual

  echo "==> Committed archives ($bundle_dir)"

  while IFS=$'\t' read -r archive committed bytes sha256 lib lib_sha256; do
    [[ -n "$archive" ]] || continue
    [[ "$committed" == "true" ]] || continue
    archives_checked=$((archives_checked + 1))
    path="$bundle_dir/$archive"

    if [[ ! -f "$path" ]]; then
      printf 'heif-rs: %s is missing from %s\n' "$archive" "$bundle_dir" >&2
      printf '  expected sha256 %s\n' "$sha256" >&2
      printf '  actual   sha256 (no such file)\n' >&2
      printf '  fix: scripts/heif-artifacts.sh fetch %s\n' "$archive" >&2
      failures=$((failures + 1))
      continue
    fi

    actual_bytes="$(file_bytes "$path")"
    actual_sha="$(sha256_of "$path")"

    if [[ "$actual_bytes" != "$bytes" ]]; then
      printf 'heif-rs: %s has the wrong size\n' "$archive" >&2
      printf '  expected bytes %s\n' "$bytes" >&2
      printf '  actual   bytes %s\n' "$actual_bytes" >&2
      printf '  fix: scripts/heif-artifacts.sh fetch %s\n' "$archive" >&2
      failures=$((failures + 1))
    fi

    if [[ "$(lower "$actual_sha")" != "$(lower "$sha256")" ]]; then
      report_checksum_failure "$archive" "$sha256" "$actual_sha" "$archive"
      failures=$((failures + 1))
    fi

    if [[ "$actual_bytes" == "$bytes" && "$(lower "$actual_sha")" == "$(lower "$sha256")" ]]; then
      printf 'ok %s %s bytes %s\n' "$archive" "$bytes" "$sha256"
    fi
  done <<<"$rows"

  # Layer two: what the build will link. The cache directory is keyed by the codec release and the archive hash, never by
  # the target, so it cannot be attributed by name — linux x64 and arm64 both ship `lib/libheif.a`. Every lib_sha256 in
  # the manifest is distinct, so the extracted library is identified by its hash: it must equal the lib_sha256 of an
  # artifact whose `lib` path matches. That is also why two extracted targets (say linux x64 and windows x64) both pass.
  echo "==> Extracted libraries and include/ trees"
  collect_scan_roots
  local scan_root i count
  i=0
  count=${#scan_roots[*]}
  while [[ "$i" -lt "$count" ]]; do
    scan_root="${scan_roots[$i]}"
    i=$((i + 1))
    printf 'scanning %s\n' "$scan_root"
  done

  discover_bundle_dirs
  discover_out_dirs

  # The distinct library paths the manifest knows about (two: lib/libheif.a and lib/heif.lib on MSVC).
  local -a libs=()
  local lib row_archive row_committed row_bytes row_sha row_lib row_lib_sha
  local j lib_count lib_dup
  while IFS=$'\t' read -r archive committed bytes sha256 lib lib_sha256; do
    [[ -n "$lib" ]] || continue
    lib_dup=0
    lib_count=${#libs[*]}
    j=0
    while [[ "$j" -lt "$lib_count" ]]; do
      if [[ "${libs[$j]}" == "$lib" ]]; then
        lib_dup=1
        break
      fi
      j=$((j + 1))
    done
    if [[ "$lib_dup" -eq 0 ]]; then
      libs+=("$lib")
    fi
  done <<<"$rows"

  local dir found expected_libs
  i=0
  count=${#bundle_dirs[*]}
  while [[ "$i" -lt "$count" ]]; do
    dir="${bundle_dirs[$i]}"
    i=$((i + 1))
    printf 'checking %s (library and include/ tree)\n' "$dir"
    found=0
    j=0
    lib_count=${#libs[*]}
    while [[ "$j" -lt "$lib_count" ]]; do
      lib="${libs[$j]}"
      j=$((j + 1))
      path="${dir%/}/$lib"
      [[ -f "$path" ]] || continue
      found=1
      libs_checked=$((libs_checked + 1))
      actual="$(sha256_of "$path")"

      matched=""
      matched_committed=""
      expected_list=""
      while IFS=$'\t' read -r row_archive row_committed row_bytes row_sha row_lib row_lib_sha; do
        [[ "$row_lib" == "$lib" ]] || continue
        if [[ "$(lower "$row_lib_sha")" == "$(lower "$actual")" ]]; then
          matched="$row_archive"
          matched_committed="$row_committed"
          break
        fi
        expected_list+="  expected sha256 $row_lib_sha ($row_archive)"$'\n'
      done <<<"$rows"

      if [[ -n "$matched" ]]; then
        printf 'ok %s matches %s (%s)\n' "$path" "$matched" "$lib"
      else
        printf 'heif-rs: %s failed its checksum\n' "$path" >&2
        printf '%s' "$expected_list" >&2
        printf '  actual   sha256 %s\n' "$actual" >&2
        printf '  fix: scripts/heif-artifacts.sh fetch <archive>   # the pinned %s for this lib\n' "$lib" >&2
        printf '  then delete %s to force a clean extraction\n' "${dir%/}" >&2
        failures=$((failures + 1))
      fi
    done

    # The directory exists, so this is not a cold cache: the build would reuse this directory and fail to find the
    # library to link. A half-extracted or poisoned cache has to be a failure, never a silently skipped check.
    if [[ "$found" -eq 0 ]]; then
      expected_libs=""
      j=0
      lib_count=${#libs[*]}
      while [[ "$j" -lt "$lib_count" ]]; do
        lib="${libs[$j]}"
        j=$((j + 1))
        expected_libs="${expected_libs:+$expected_libs or }$lib"
      done
      printf 'heif-rs: %s has no pinned library\n' "${dir%/}" >&2
      printf '  expected %s (from the manifest)\n' "$expected_libs" >&2
      printf '  actual   none of them exist under %s\n' "${dir%/}" >&2
      printf '  fix: delete %s and rebuild (cd src-tauri && cargo build --locked)\n' "${dir%/}" >&2
      failures=$((failures + 1))
    elif [[ -n "$matched" ]]; then
      check_extracted_headers "$dir" "$matched" "$matched_committed"
    fi
  done

  # Every build-script `out/` directory is checked for its `bindings.rs` digest. One that holds no extraction directory
  # is a failure rather than a cold cache: the build ran there, so "nothing extracted" means the FFI inputs cannot be
  # attested (a HEIF_BINARIES_DIR build leaves exactly this shape). A bundle-named entry that is a symlink or a special
  # file fails too, and is named as what it is — `find -type d` does not see it, and calling it "no directory" would
  # describe the wrong problem.
  local out_dir under_bundle status stray count_bundles
  local -a strays=()
  i=0
  count=${#out_dirs[*]}
  while [[ "$i" -lt "$count" ]]; do
    out_dir="${out_dirs[$i]}"
    i=$((i + 1))

    strays=()
    find_output="$(mktemp "${TMPDIR:-/tmp}/heif-find.XXXXXX")" ||
      die "heif-artifacts: cannot create a temporary file under ${TMPDIR:-/tmp} to scan $out_dir"
    find "$out_dir" -mindepth 1 -maxdepth 1 -name 'binaries-heif-bundle-*' ! -type d -print0 \
      >"$find_output" || {
      status=$?
      rm -f -- "$find_output"
      find_output=""
      die "heif-artifacts: cannot scan $out_dir: find exited $status"
    }
    while IFS= read -r -d '' stray; do
      strays+=("$stray")
    done <"$find_output"
    rm -f -- "$find_output"
    find_output=""

    under_bundle=0
    j=0
    count_bundles=${#bundle_dirs[*]}
    while [[ "$j" -lt "$count_bundles" ]]; do
      if [[ "${bundle_dirs[$j]}" == "$out_dir/binaries-heif-bundle-"* ]]; then
        under_bundle=1
        break
      fi
      j=$((j + 1))
    done

    if [[ ${#strays[*]} -gt 0 ]]; then
      j=0
      while [[ "$j" -lt "${#strays[*]}" ]]; do
        stray="${strays[$j]}"
        j=$((j + 1))
        printf 'heif-rs: %s is not a directory (symlink or special file)\n' "$stray" >&2
        printf '  expected one binaries-heif-bundle-* directory (the extraction of the archive this build linked)\n' >&2
        printf '  actual   %s\n' "$(entry_kind "$stray")" >&2
        printf '  fix: delete %s and rebuild (cd src-tauri && cargo build --locked)\n' "$stray" >&2
        failures=$((failures + 1))
      done
    elif [[ "$under_bundle" -eq 0 ]]; then
      printf 'heif-rs: %s has no extracted bundle directory\n' "$out_dir" >&2
      printf '  expected one binaries-heif-bundle-* directory under it (the extraction of the archive this build linked)\n' >&2
      printf '  actual   none; the codegen inputs in this build cannot be attested\n' >&2
      printf '  fix: delete %s and rebuild (cd src-tauri && cargo build --locked); if this build used HEIF_BINARIES_DIR, delete it instead\n' \
        "$out_dir" >&2
      failures=$((failures + 1))
    fi
    check_bindings "$out_dir"
  done

  # Zero bundle directories *and* zero build-script output is a cold cache. This wording is deliberately distinct from
  # the "verified N extracted libraries" summary: an empty scan must never read as "checked zero, all good".
  if [[ ${#bundle_dirs[*]} -eq 0 && ${#out_dirs[*]} -eq 0 ]]; then
    echo "heif-artifacts: no extracted bundle directories found — nothing attested (cold cache)"
  fi

  echo "==> Summary"
  if [[ "$failures" -ne 0 ]]; then
    printf 'failed: %s problem(s) against %s committed archive(s) and %s extracted librar(y/ies)\n' \
      "$failures" "$archives_checked" "$libs_checked" >&2
    exit 1
  fi
  # Printed only on full success, after every archive and extracted-library check has passed. A tree with no heif-rs
  # build output at all is a cold cache: it reports zero extracted libraries and printed its own line above.
  # `scripts/ci.sh trust` asserts the archive count, so a verifier that checked nothing cannot be mistaken for a passing
  # gate.
  printf 'heif-artifacts: verified %s committed archives, %s extracted libraries\n' \
    "$archives_checked" "$libs_checked"
}

fetch() {
  local archive="${1-}"
  if [[ -z "$archive" ]]; then
    usage >&2
    exit 2
  fi

  require_sha256_tool
  require_json_reader

  [[ -d "$bundle_dir" ]] || die "heif-artifacts: bundle directory not found: $bundle_dir"

  local rows
  rows="$(manifest_rows)" || exit 1

  local meta release template
  meta="$(manifest_meta)" || exit 1
  IFS=$'\t' read -r release template <<<"$meta"

  local row_archive row_committed row_bytes row_sha row_lib row_lib_sha
  local bytes="" sha256="" committed=""
  while IFS=$'\t' read -r row_archive row_committed row_bytes row_sha row_lib row_lib_sha; do
    if [[ "$row_archive" == "$archive" ]]; then
      bytes="$row_bytes"
      sha256="$row_sha"
      committed="$row_committed"
      break
    fi
  done <<<"$rows"

  if [[ -z "$bytes" ]]; then
    printf 'heif-artifacts: unknown archive: %s\n' "$archive" >&2
    printf 'known archives:\n' >&2
    while IFS=$'\t' read -r row_archive row_committed row_bytes row_sha row_lib row_lib_sha; do
      [[ -n "$row_archive" ]] || continue
      printf '  %s\n' "$row_archive" >&2
    done <<<"$rows"
    exit 1
  fi

  local url="${template//\{release\}/$release}"
  url="${url//\{archive\}/$archive}"

  # Download next to the destination under a temporary name and move it into place only after the manifest accepts it,
  # so a partial or tampered download never appears under the real archive name. The path is the script-level
  # `fetch_tmp`, which the top-level EXIT trap owns, so every failure path — including the download command itself
  # exiting non-zero under `set -e` — removes it.
  fetch_tmp="$(mktemp "$bundle_dir/.${archive}.XXXXXX")"
  # An interrupt must not leave the partial file either: exit through the trap instead of dying on the signal.
  trap 'exit 130' INT
  trap 'exit 143' TERM

  echo "heif-artifacts: downloading $url"
  if command -v curl >/dev/null 2>&1; then
    curl --fail --location --silent --show-error --output "$fetch_tmp" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget --quiet --output-document="$fetch_tmp" "$url"
  else
    die "heif-artifacts: neither curl nor wget is on PATH; cannot download $url"
  fi

  local actual_bytes actual_sha
  actual_bytes="$(file_bytes "$fetch_tmp")"
  actual_sha="$(sha256_of "$fetch_tmp")"

  if [[ "$actual_bytes" != "$bytes" || "$(lower "$actual_sha")" != "$(lower "$sha256")" ]]; then
    rm -f -- "$fetch_tmp"
    fetch_tmp=""
    printf 'heif-artifacts: discarded the %s download; it does not match the manifest\n' "$archive" >&2
    printf '  expected bytes  %s\n' "$bytes" >&2
    printf '  actual   bytes  %s\n' "$actual_bytes" >&2
    printf '  expected sha256 %s\n' "$sha256" >&2
    printf '  actual   sha256 %s\n' "$actual_sha" >&2
    printf '  fix: check the pinned release %s and the manifest row for %s; the manifest is never written here\n' \
      "$release" "$archive" >&2
    exit 1
  fi

  mv -f -- "$fetch_tmp" "$bundle_dir/$archive"
  fetch_tmp=""
  trap - INT TERM
  # `mktemp` creates 0600; the committed archives are 0644, and a fetched archive may be committed by a later feature.
  chmod 0644 -- "$bundle_dir/$archive"

  printf 'ok %s %s bytes %s (downloaded from %s)\n' "$archive" "$actual_bytes" "$actual_sha" "$release"
  if [[ "$committed" == "false" ]]; then
    echo "note: $archive is committed: false; keep it out of git (the manifest asserts it is absent from the tree)"
  fi
}

case "${1-}" in
  verify)
    verify
    ;;
  fetch)
    fetch "${2-}"
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  "")
    usage >&2
    exit 2
    ;;
  *)
    echo "Unknown subcommand: $1" >&2
    usage >&2
    exit 2
    ;;
esac
