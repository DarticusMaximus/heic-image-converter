#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/ci.sh <subcommand>

Subcommands:
  trust     Verify the committed codec archives, any extracted libraries, and the vendored heif-rs lock entry
  frontend  Run the frontend tests (npm test) and frontend build (npm run build)
  rust      Run Rust formatting checks (cargo fmt --check) and tests (cargo test --locked)
  build     Run the Tauri release build without bundling, passing --locked to cargo
  offline   Prove the Rust gate builds and tests with no route to the network: clone HEAD into a
            throwaway directory, then run scripts/ci.sh rust inside a network namespace (needs
            unshare -rn or bwrap). Not part of `all`. CI_OFFLINE_DIR picks the working directory
            and needs about 8 GiB free; point it at a disk-backed path when the default is a small
            tmpfs. Refuses to run when HEIF_BINARIES_DIR or CARGO_TARGET_DIR is set, because either
            one changes which codec bytes and which build tree a PASS would describe
  all       Run trust, then frontend, then rust, then build; stop at the first failure

Options:
  -h, --help  Show this help and exit

The script installs nothing: run npm ci first on a fresh checkout.
EOF
}

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
cd "$repo_root"

die() {
  printf '%s\n' "$*" >&2
  exit 1
}

trust() {
  echo "==> Codec trust gate"
  local log
  log="$(mktemp "${TMPDIR:-/tmp}/heif-trust.XXXXXX")"
  if ! scripts/heif-artifacts.sh verify 2>&1 | tee "$log"; then
    rm -f "$log"
    echo "trust gate: the artifact verifier failed" >&2
    return 1
  fi
  if ! grep -qE '^heif-artifacts: verified 4 committed archives, [0-9]+ extracted libraries$' "$log"; then
    rm -f "$log"
    echo "trust gate: the verifier did not report the four committed archives; refusing to certify" >&2
    return 1
  fi
  rm -f "$log"

  # Cargo.lock: exactly one heif-rs entry, version 26.10.3, no source or checksum. The block for a package runs from
  # its `name =` line to the next `[[package]]`, so this reads the lock as text and needs no jq, python3, or build.
  local lock="src-tauri/Cargo.lock"
  local blocks count version
  blocks="$(awk '
    /^name = "heif-rs"$/ { inside = 1 }
    inside && /^\[\[package\]\]$/ { inside = 0; next }
    inside { print }
  ' "$lock")"
  count="$(awk '/^name = "heif-rs"$/{n++} END{print n + 0}' <<<"$blocks")"

  if [[ "$count" -eq 0 ]]; then
    echo "trust gate: Cargo.lock has no heif-rs package entry" >&2
    return 1
  fi
  if [[ "$count" -ne 1 ]]; then
    echo "trust gate: Cargo.lock has $count heif-rs package entries; expected 1" >&2
    return 1
  fi
  version="$(awk -F'"' '/^version = /{print $2; exit}' <<<"$blocks")"
  if [[ "$version" != "26.10.3" ]]; then
    echo "trust gate: heif-rs is locked to $version; expected 26.10.3" >&2
    return 1
  fi
  if grep -q '^source = ' <<<"$blocks"; then
    echo "trust gate: heif-rs resolves from a registry source; expected the vendored path package" >&2
    return 1
  fi
  if grep -q '^checksum = ' <<<"$blocks"; then
    echo "trust gate: heif-rs carries a registry checksum; expected the vendored path package" >&2
    return 1
  fi

  echo "trust gate: heif-rs 26.10.3 locked from vendor/heif-rs (path package)"
  echo "trust gate: PASS"
}

frontend() {
  echo "==> Frontend tests"
  npm test
  echo "==> Frontend build"
  npm run build
}

rust() {
  echo "==> Rust formatting"
  ( cd src-tauri && cargo fmt --check )
  echo "==> Rust tests"
  ( cd src-tauri && cargo test --locked )
}

build() {
  echo "==> Tauri release build (no bundle)"
  npm run tauri -- build --no-bundle -- --locked
}

# State for the offline gate's EXIT trap. Both live at script scope because a trap
# cannot see a variable that only ever existed in a function's local scope: on a
# `set -e` exit out of `offline` such a trap would find nothing and leave the
# throwaway clone behind.
offline_clone_dir=""
offline_running=0
cleanup_offline() {
  local status=$?
  # One closing line on every path: a failure must never look like a run that
  # simply had nothing to say. The PASS line is printed by `offline` itself.
  if [[ "$status" -ne 0 && "$offline_running" -eq 1 ]]; then
    echo "offline gate: FAIL (exit $status)" >&2
  fi
  if [[ -n "$offline_clone_dir" && -d "$offline_clone_dir" ]]; then
    rm -rf -- "$offline_clone_dir"
  fi
}
trap cleanup_offline EXIT

offline() {
  offline_running=1

  # An inherited override steers the build away from the bytes this gate claims
  # to prove, and nothing later in the transcript would say so. Refuse before the
  # disk check and the clone: a run that cannot describe what it certified must
  # not start. Both variables are reported in one pass so a machine with both set
  # learns about both.
  local overrides_set=0
  if [[ -n "${HEIF_BINARIES_DIR-}" ]]; then
    echo "offline gate: HEIF_BINARIES_DIR is set to \"$HEIF_BINARIES_DIR\": it bypasses the committed bundle manifest and its hashes, so a green result would not prove the committed codec bytes" >&2
    overrides_set=1
  fi
  if [[ -n "${CARGO_TARGET_DIR-}" ]]; then
    echo "offline gate: CARGO_TARGET_DIR is set to \"$CARGO_TARGET_DIR\": it moves the build out of the clone, so the copy this gate proves is not the one the clone builds" >&2
    overrides_set=1
  fi
  if [[ "$overrides_set" -eq 1 ]]; then
    echo "offline gate: refusing to certify with an override set; rerun with: env -u HEIF_BINARIES_DIR -u CARGO_TARGET_DIR ./scripts/ci.sh offline" >&2
    exit 1
  fi

  local rev short_rev
  rev="$(git -C "$repo_root" rev-parse HEAD)"
  short_rev="$(git -C "$repo_root" rev-parse --short HEAD)"
  # The clone carries committed files only, so a dirty tree is not what gets
  # proven. Say so rather than silently certifying something else.
  if [[ -n "$(git -C "$repo_root" status --porcelain)" ]]; then
    echo "offline gate: working tree is dirty; this run proves HEAD $short_rev only" >&2
  fi

  # 8 GiB is the measured floor, not a guess: this checkout's accumulated
  # src-tauri/target/debug is 6.4 GB, of which 5.0 GB is deps/, so a cold test
  # build needs that order. An ENOSPC partway through must never be mistaken for
  # an offline failure, so the floor refuses before anything is cloned.
  local work_dir avail_kib avail_gib
  work_dir="${CI_OFFLINE_DIR:-${TMPDIR:-/tmp}}"
  [[ -d "$work_dir" ]] || die "offline gate: CI_OFFLINE_DIR is not a directory: $work_dir"
  # -P is the POSIX output format: one line per filesystem, so a long device name
  # cannot wrap the row and shift the fields. Field 4 is the available 1 KiB blocks.
  avail_kib="$(df -Pk "$work_dir" | awk 'NR == 2 { print $4 }')"
  [[ "$avail_kib" =~ ^[0-9]+$ ]] || die "offline gate: cannot read free space for $work_dir from df"
  avail_gib=$((avail_kib / 1048576))
  if ((avail_kib < 8 * 1048576)); then
    echo "offline gate: only ${avail_gib} GiB free in $work_dir; a cold Rust test build needs about 8 GiB" >&2
    echo "offline gate: set CI_OFFLINE_DIR to a disk-backed path with room, e.g. $HOME/.cache/offline-gate" >&2
    exit 1
  fi
  if ((avail_kib < 12 * 1048576)); then
    echo "offline gate: warning: only ${avail_gib} GiB free in $work_dir; a full cold target can reach 12 GiB" >&2
  fi
  echo "offline gate: work dir: $work_dir (${avail_gib} GiB free)"

  offline_clone_dir="$(mktemp -d "${work_dir%/}/offline-gate.XXXXXX")"
  echo "offline gate: clone: $offline_clone_dir/repo"
  # --no-hardlinks: the clone must not lean on the source repository's object
  # store, so the build cannot read a file the clone itself lacks.
  git clone --quiet --no-hardlinks "$repo_root" "$offline_clone_dir/repo"

  # A clone of HEAD carries no ignored files. If one of these appears anyway, the
  # clone is not the clean checkout this gate claims to prove.
  local path
  for path in node_modules dist src-tauri/target; do
    if [[ -e "$offline_clone_dir/repo/$path" ]]; then
      die "offline gate: the clone contains $path; it is not a clean checkout"
    fi
  done

  # The wrapper is kept as an array so the probe and the gate run through exactly
  # the same command. Each candidate is probed with a harmless command first: a
  # wrapper that cannot even start looks like isolation, and must not.
  local -a iso=()
  local iso_name=""
  if unshare -rn true >/dev/null 2>&1; then
    iso=(unshare -rn)
    iso_name="unshare -rn"
  elif bwrap --unshare-net --dev-bind / / true >/dev/null 2>&1; then
    iso=(bwrap --unshare-net --dev-bind / /)
    iso_name="bwrap --unshare-net"
  else
    die "offline gate: no network isolation available (need unshare -rn or bwrap)"
  fi

  # The probe uses a literal IP so neither DNS nor a proxy can fake the result.
  local probe_cmd="timeout 5 bash -c 'exec 3<>/dev/tcp/1.1.1.1/443'"
  local -a probe=(timeout 5 bash -c 'exec 3<>/dev/tcp/1.1.1.1/443')

  # Everything the positive control depends on, printed before it runs, so a
  # saved transcript explains itself.
  echo "offline gate: HEAD $rev"
  echo "offline gate: isolation: $iso_name"
  # Which bytes and which tree, stated before the build rather than inferred
  # after it. The clone path is printed so the two paths are checkable, and both
  # overrides are named as unset because the check at the top of this function is
  # what makes that true.
  echo "offline gate: codec source: $offline_clone_dir/repo/src-tauri/vendor/heif-bundles/manifest.json (committed archives; HEIF_BINARIES_DIR unset)"
  echo "offline gate: target: $offline_clone_dir/repo/src-tauri/target (inside the clone; CARGO_TARGET_DIR unset)"
  echo "offline gate: positive control (outside the sandbox): $probe_cmd"
  echo "offline gate: positive control (inside the sandbox): $iso_name $probe_cmd"

  # bash 3.2 — the /bin/bash macOS ships — aborts an array expansion under
  # `set -u` when the array holds no elements, where bash 4.4+ expands to
  # nothing. Both arrays are assigned by this point, so these are assertions as
  # much as guards: they turn a would-be `unbound variable` abort into a named
  # line, and they keep the expansions below safe on the older shell.
  if [[ ${#iso[@]} -eq 0 ]]; then
    die "offline gate: internal error: the isolation wrapper command is empty; refusing to certify"
  fi
  if [[ ${#probe[@]} -eq 0 ]]; then
    die "offline gate: internal error: the network probe command is empty; refusing to certify"
  fi

  # The outside run goes first on purpose. On a machine with no working network,
  # the inside probe would fail for a reason that has nothing to do with the
  # wrapper, and the run would certify nothing. It also proves the wrapper can
  # find bash and timeout, which an unreachable sandbox would otherwise hide.
  if ! "${probe[@]}" >/dev/null 2>&1; then
    die "offline gate: positive control cannot reach the network outside the sandbox; refusing to certify"
  fi
  if "${iso[@]}" "${probe[@]}" >/dev/null 2>&1; then
    die "offline gate: the sandbox still reaches the network; refusing to certify"
  fi
  echo "offline gate: sandbox verified unreachable (positive control)"

  # Two enforcement layers, and neither is sufficient alone. CARGO_NET_OFFLINE=true
  # makes Cargo refuse the network with a readable error, but it does not govern
  # what build.rs spawns. The namespace makes the refusal binding on everything
  # the build runs. Together they prove the claim.
  #
  # `-u` scrubs the two overrides in the child as well, even though the check at
  # the top of this function already refused them: a variable exported between
  # that check and this line would otherwise reach the build, and a build steered
  # by either is not the one this gate certifies.
  #
  # The clone's own committed scripts/ci.sh is what runs, so the committed gate is
  # what gets proven. The script's `set -o pipefail` makes this pipeline's status
  # the gate's status, so `tee` cannot launder a failure.
  local log="$offline_clone_dir/rust-gate.log"
  echo "==> Rust gate in the clone (CARGO_NET_OFFLINE=true)"
  if ! ( cd "$offline_clone_dir/repo" \
         && "${iso[@]}" env -u HEIF_BINARIES_DIR -u CARGO_TARGET_DIR CARGO_NET_OFFLINE=true ./scripts/ci.sh rust ) \
       2>&1 | tee "$log"; then
    tail -n 40 "$log" >&2
    exit 1
  fi
  # A committed artifact that quietly became a download makes the build pass for
  # the wrong reason. Feature 02's rule is an assertion here, not an eyeballed log.
  if grep -q "heif-rs: downloading" "$log"; then
    die "offline gate: a codec download was attempted"
  fi

  # Read HEAD from the clone, not from the value captured above, so this line can
  # never disagree with what was actually built.
  local clone_rev
  clone_rev="$(git -C "$offline_clone_dir/repo" rev-parse HEAD)"
  echo "offline gate: PASS (HEAD $clone_rev, isolation: $iso_name)"
}

case "${1-}" in
  trust)
    trust
    ;;
  frontend)
    frontend
    ;;
  rust)
    rust
    ;;
  build)
    build
    ;;
  offline)
    offline
    ;;
  all)
    trust
    frontend
    rust
    build
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
