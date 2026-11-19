#!/usr/bin/env bash
#
# Verify the pinned HEIC codec artifacts, and fetch the ones that are not committed.
#
# The pin lives in src-tauri/vendor/heif-bundles/manifest.json: each bundle_* archive carries the byte length and
# SHA-256 the build script enforces before it extracts anything, plus the SHA-256 of the static library inside it.
# This script re-checks both layers on demand:
#
#   1. every `committed: true` archive in src-tauri/vendor/heif-bundles/ against its recorded `bytes` and `sha256`;
#   2. every library extracted under src-tauri/target/*/build/heif-rs-*/out/binaries-heif-bundle-*/, against the
#      manifest's recorded `lib_sha256`. The cache directory is named for the codec release, never for the target, so
#      the library is matched by hash instead of being attributed by name — linux x64 and arm64 both ship
#      `lib/libheif.a`, and all eight `lib_sha256` values are distinct.
#
# A bundle directory that exists but holds none of the manifest's libraries is a half-extracted cache: that is a
# failure, because the build cannot link it. Only the complete absence of bundle directories is a cold cache, which
# passes.
#
# It only ever reads the manifest. `fetch` writes bytes, and only into the bundle directory; it deletes any download
# that does not match the manifest row rather than laundering a bad artifact into the tree.

set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/heif-artifacts.sh <subcommand>

Subcommands:
  verify             Check every committed archive and every extracted codec library against the pinned manifest
  fetch <archive>    Download one archive from the pinned release into src-tauri/vendor/heif-bundles/, accepting it
                     only if its byte length and SHA-256 match the manifest row

Options:
  -h, --help  Show this help and exit

`verify` exits 0 on a clean tree, including a cold cache with nothing extracted yet. It exits 1 on any mismatch,
naming the file and the pinned hash. `fetch` never edits the manifest: the hashes are the authority.
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
cleanup_fetch_tmp() {
  if [[ -n "$fetch_tmp" && -e "$fetch_tmp" ]]; then
    rm -f -- "$fetch_tmp"
  fi
}
trap cleanup_fetch_tmp EXIT

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

# The build script's wording for a checksum failure, so the two layers report the same way.
report_checksum_failure() {
  local subject="$1" expected="$2" actual="$3" archive="$4"
  printf 'heif-rs: %s failed its checksum\n' "$subject" >&2
  printf '  expected sha256 %s\n' "$expected" >&2
  printf '  actual   sha256 %s\n' "$actual" >&2
  printf '  fix: scripts/heif-artifacts.sh fetch %s\n' "$archive" >&2
}

verify() {
  require_sha256_tool
  require_json_reader

  local rows
  rows="$(manifest_rows)" || exit 1

  local archive committed bytes sha256 lib lib_sha256
  local path actual_bytes actual_sha failures=0 archives_checked=0 libs_checked=0
  local expected_list matched actual
  local -a dirs=()

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

    if [[ "${actual_sha,,}" != "${sha256,,}" ]]; then
      report_checksum_failure "$archive" "$sha256" "$actual_sha" "$archive"
      failures=$((failures + 1))
    fi

    if [[ "$actual_bytes" == "$bytes" && "${actual_sha,,}" == "${sha256,,}" ]]; then
      printf 'ok %s %s bytes %s\n' "$archive" "$bytes" "$sha256"
    fi
  done <<<"$rows"

  # Layer two: what the build will link. The cache directory is named for the codec release, never for the target, so
  # it cannot be attributed by name — linux x64 and arm64 both ship `lib/libheif.a`. Every lib_sha256 in the manifest is
  # distinct, so the extracted library is identified by its hash: it must equal the lib_sha256 of an artifact whose
  # `lib` path matches. That is also why two extracted targets (say linux x64 and windows x64) both pass.
  echo "==> Extracted libraries (src-tauri/target/*/build/heif-rs-*/out/binaries-heif-bundle-*/)"

  shopt -s nullglob
  dirs=("src-tauri/target"/*/build/heif-rs-*/out/binaries-heif-bundle-*/)
  shopt -u nullglob

  # The distinct library paths the manifest knows about (two: lib/libheif.a and lib/heif.lib on MSVC).
  local -a libs=()
  local row_archive row_committed row_bytes row_sha row_lib row_lib_sha
  while IFS=$'\t' read -r archive committed bytes sha256 lib lib_sha256; do
    [[ -n "$lib" ]] || continue
    [[ " ${libs[*]-} " == *" $lib "* ]] || libs+=("$lib")
  done <<<"$rows"

  local dir
  local found
  local expected_libs
  for dir in "${dirs[@]}"; do
    found=0
    for lib in "${libs[@]}"; do
      path="${dir%/}/$lib"
      [[ -f "$path" ]] || continue
      found=1
      libs_checked=$((libs_checked + 1))
      actual="$(sha256_of "$path")"

      matched=""
      expected_list=""
      while IFS=$'\t' read -r row_archive row_committed row_bytes row_sha row_lib row_lib_sha; do
        [[ "$row_lib" == "$lib" ]] || continue
        if [[ "${row_lib_sha,,}" == "${actual,,}" ]]; then
          matched="$row_archive"
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
      for lib in "${libs[@]}"; do
        expected_libs="${expected_libs:+$expected_libs or }$lib"
      done
      printf 'heif-rs: %s has no pinned library\n' "${dir%/}" >&2
      printf '  expected %s (from the manifest)\n' "$expected_libs" >&2
      printf '  actual   none of them exist under %s\n' "${dir%/}" >&2
      printf '  fix: delete %s and rebuild (cd src-tauri && cargo build --locked)\n' "${dir%/}" >&2
      failures=$((failures + 1))
    fi
  done

  if [[ ${#dirs[@]} -eq 0 ]]; then
    echo "ok no extracted bundle directories under src-tauri/target/ (cold cache)"
  fi

  echo "==> Summary"
  if [[ "$failures" -ne 0 ]]; then
    printf 'failed: %s problem(s) against %s committed archive(s) and %s extracted librar(y/ies)\n' \
      "$failures" "$archives_checked" "$libs_checked" >&2
    exit 1
  fi
  # "Cold cache" is reserved for a tree with no bundle directories at all. A directory that exists but holds no
  # library was counted as a failure above, so it can never reach this line.
  if [[ ${#dirs[@]} -eq 0 ]]; then
    printf 'verified %s committed archive(s); no extracted bundle directories (cold cache)\n' "$archives_checked"
  else
    printf 'verified %s committed archive(s) and %s extracted librar(y/ies); all match the manifest\n' \
      "$archives_checked" "$libs_checked"
  fi
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

  if [[ "$actual_bytes" != "$bytes" || "${actual_sha,,}" != "${sha256,,}" ]]; then
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
