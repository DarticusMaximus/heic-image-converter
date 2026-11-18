#!/usr/bin/env bash
#
# Local-only manual verification harness for "HEIC Image Converter".
#
# A developer tool: it never runs in CI, installs nothing and needs no network.
# It builds a synthetic sample corpus outside the repository, records a SHA-256
# manifest so a manual run can prove the app never touched its sources, and
# removes the directory again without leaving anything in the working tree.
#
# Safety: the scratch directory carries a `.heic-manual` marker. `sample` and
# `cleanup` refuse to touch a directory without that marker, refuse paths that
# contain the repository or the home directory, and refuse paths inside the
# repository. Never delete a scratch directory by hand; use `cleanup`.
#
# Window control: `launch` starts the app, waits for its window (matched by
# title in `hyprctl clients -j`), focuses it and records the PID; `focus`,
# `shot` and `close` drive that same window. Run artifacts (`app.log`,
# `app.pid`) live in the scratch directory, never in the repository.
#
# Usage:
#   scripts/manual-check.sh sample [dir]
#   scripts/manual-check.sh launch [path...]
#   scripts/manual-check.sh focus
#   scripts/manual-check.sh shot <name>
#   scripts/manual-check.sh close
#   scripts/manual-check.sh cleanup [dir]
#   scripts/manual-check.sh --help

set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
cd "$repo_root"

MANUAL_DIR="${MANUAL_DIR:-/tmp/heic-manual}"
MARKER=".heic-manual"
REQUIRED_FREE_KIB=$((3 * 1024 * 1024)) # roughly 3 GB
CORNER_TOLERANCE=8                     # HEIC is lossy; same tolerance as feature 02
CORPUS_DIRS=(ui edge heic estimate batch-cancel batch)

# Window control. The title is the app window title from src-tauri/tauri.conf.json
# ("HEIC Image Converter"); WebKitGTK derives the class "heic-image-converter".
APP_TITLE="HEIC Image Converter"
APP_CLASS="heic-image-converter"
APP_BIN_DEFAULT="src-tauri/target/release/heic-image-converter"
WINDOW_WAIT_SECONDS=15
CLOSE_WAIT_SECONDS=5

# Run artifacts live in the scratch directory, never in the repository.
run_dir=""
app_log=""
app_pid_file=""

# Feature-02 palette: solid 8-bit RGB quadrants.
RED='#e02020'
GREEN='#20c020'
BLUE='#2040e0'
WHITE='#f0f0f0'

work_dir=""

usage() {
  cat <<'EOF'
Usage: scripts/manual-check.sh <command> [args]

Commands:
  sample [dir]   Rebuild the synthetic sample corpus in the scratch directory,
                 write sources.sha256 and EXPECTED.md, print per-directory
                 counts, and exit 0. `sample` recreates the directory.
  launch [path...]
                 Start the app with these arguments, append its output to
                 <scratch>/app.log, wait up to 15 s for a window titled
                 "HEIC Image Converter", focus it, record its PID in
                 <scratch>/app.pid, and print the keyboard rules. Fails
                 non-zero when the binary is missing or no window appears.
                 Refuses to start while a window is already open.
  focus          Re-focus the app window. Fails non-zero when none is open.
  shot <name>    Focus the app window, wait briefly, and capture only that
                 window with `grim -g` into ${SHOT_DIR:-docs/verification/
                 <today>}/<name>.png. `.png` is appended (a trailing `.png` is
                 accepted); the name may not contain a directory.
  close          Terminate the recorded PID and wait up to 5 s. Succeeds with a
                 message when nothing is running.
  cleanup [dir]  Restore mode 644 on edge/denied.png and remove the scratch
                 directory.

Options:
  -h, --help     Show this help and exit 0.

Environment:
  MANUAL_DIR     Scratch directory (default: /tmp/heic-manual). It also holds
                 app.log and app.pid; `launch` creates it and its marker when
                 it does not exist yet, so window control works without
                 `sample`.
  APP_BIN        Binary to launch (default:
                 src-tauri/target/release/heic-image-converter).
  SHOT_DIR       Screenshot directory, relative to the repository root unless
                 absolute (default: docs/verification/<ISO date>).

Safety: the scratch directory must contain a .heic-manual marker. sample and
cleanup refuse to touch or delete any directory without it, and refuse paths
that contain the repository or the home directory or that sit inside the
repository. The corpus is synthetic and needs about 3 GB free in the scratch
parent for the corpus plus the app's outputs. Local-only: nothing is installed
and no network is used.

Manual run protocol (M1-M11)

Keyboard rules: `launch` focuses the app window. Move focus with
  wtype -k Tab                    (forward)
  wtype -M shift -k Tab -m shift  (reverse)
The app draws a focus-visible ring. Activate a button with wtype -k space, pick
a select entry with wtype -k Home, wtype -k Down or a digit key, toggle a
checkbox with wtype -k space. Ctrl+Enter is `wtype -M ctrl -k Return -m ctrl`,
Escape is `wtype -k Escape`. Pointer synthesis is not available: Tab is the
supported path. Record the Tab count and the visible result for every
interaction, and reuse a recorded sequence when a later step needs the same
target. Take a `shot` whenever the target state is uncertain; a screenshot
corroborates an observation and never replaces the written one.

  M1  Picker       scripts/manual-check.sh launch
                   wtype -M ctrl -k o -m ctrl; wait for the picker window; record
                   its title and class from `hyprctl clients -j`. It is a separate
                   client, so Escape sent to the app does not dismiss it; wtype -k
                   Escape then scripts/manual-check.sh close to leave. No
                   screenshot: the dialog is a separate window and `shot`
                   captures only the app.
  M2  Drag-drop    the PM drags $MANUAL_DIR/ui into the window; record the five
                   file names with dimensions; shot M2-drag (five rows). Needs
                   the PM: stop and escalate if the PM is unavailable.
  M3  PNG note     Tab to the Convert-to group, wtype -k space on PNG; record
                   the exact note text; shot M3-quality-note.
  M4  Persistence  focus the resize select and choose "Longest edge 1920 px";
                   close; launch $MANUAL_DIR/ui; confirm PNG is pressed and the
                   select still shows 1920 px; shot M4-settings; close.
  M5  Conversion   launch $MANUAL_DIR/ui; confirm PNG + 1920; Ctrl+Enter; wait
                   for the summary; check every output with `magick identify`
                   (dimensions and corners); shot M5-results.
  M6  Reveal       from the completed run activate "Show results"; record
                   hyprctl clients -j | jq -r '.[].class' | sort -u and the
                   file-manager class; close it; close the app.
  M7  Errors       launch $MANUAL_DIR/edge; set PNG if needed; Ctrl+Enter;
                   record both error strings verbatim and the zero-output
                   count; shot M7-errors; close.
  M8  HEIC output  launch $MANUAL_DIR/heic/source.png; select HEIC; Ctrl+Enter;
                   record `heif-info` fields, `heif-convert` exit status and the
                   decoded dimensions, plus both tool versions; close.
  M9  Estimate     launch $MANUAL_DIR/estimate; select JPEG; resize "Keep
                   original size"; record the estimate card; shot M9-estimate;
                   Ctrl+Enter; record the seven output lengths and deviation.
  M10 Cancellation launch $MANUAL_DIR/batch-cancel; select PNG; Ctrl+Enter;
                   wait for five completed rows; Escape; record the banner
                   counts; shot M10-cancel; cd "$MANUAL_DIR" && sha256sum -c
                   sources.sha256; close.
  M11 Large batch  launch $MANUAL_DIR/batch; confirm PNG + "Keep original
                   size"; time Ctrl+Enter until the summary; record the
                   duration, the 120 outputs and sampled dimensions;
                   sha256sum -c again; close.

Commands are literal: `wtype` sequences are typed into the focused window, so
run `focus` first if anything else took focus. `shot` and `close` name no
window: they act on the single window titled "HEIC Image Converter".
EOF
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

usage_error() {
  printf '%s\n\n' "$*" >&2
  usage >&2
  exit 2
}

require_tools() {
  local tool
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || die "required tool not found: $tool"
  done
}

# Normalise the scratch path and refuse anything that could destroy real data.
resolve_dir() {
  local dir=$1 protected

  [ -n "$dir" ] || die "empty scratch directory"
  dir=$(realpath -m -- "$dir")

  for protected in / /tmp /home "${HOME:-/nonexistent}" "$repo_root"; do
    if [ "$dir" = "$protected" ]; then
      die "refusing to use $dir as the scratch directory"
    fi
  done
  case "$repo_root/" in
    "$dir"/*) die "refusing to use $dir: it contains the repository" ;;
  esac
  case "${HOME:-/nonexistent}/" in
    "$dir"/*) die "refusing to use $dir: it contains the home directory" ;;
  esac
  case "$dir/" in
    "$repo_root"/*) die "refusing to keep scratch files inside the repository ($repo_root)" ;;
  esac

  printf '%s\n' "$dir"
}

# A directory is ours only if it carries the marker written by `sample`.
assert_marked() {
  local dir=$1
  [ -d "$dir" ] || die "refusing to touch $dir: not a directory"
  if [ ! -f "$dir/$MARKER" ]; then
    die "refusing to touch $dir: no $MARKER marker (run 'sample' there, or remove the directory yourself)"
  fi
}

check_free_space() {
  local dir=$1 parent available_kib
  parent=$(dirname -- "$dir")
  mkdir -p -- "$parent"
  available_kib=$(df -Pk "$parent" | awk 'NR == 2 { print $4 }')
  [ -n "${available_kib:-}" ] || die "could not read free space for $parent"
  if [ "$available_kib" -lt "$REQUIRED_FREE_KIB" ]; then
    die "not enough free space in $parent: $((available_kib / 1024 / 1024)) GiB available, roughly 3 GB (3 GiB) required for the corpus and its outputs"
  fi
  printf 'free space in %s: %d GiB (roughly 3 GB required)\n' \
    "$parent" "$((available_kib / 1024 / 1024))"
}

# ------------------------------------------------------------------ corpus ---

# Quadrant PNG in stored order top red/green, bottom blue/white (R G / B W),
# written as 8-bit RGB. Both rotated fixtures start from this order: after the
# app applies the orientation or the container rotation, the decoded corners
# are B R / W G, which is what EXPECTED.md and step M5 record.
quadrant_png() {
  local out=$1 width=$2 height=$3 half_w half_h
  half_w=$((width / 2))
  half_h=$((height / 2))
  magick -size "${half_w}x${half_h}" xc:"$RED" \
    -size "${half_w}x${half_h}" xc:"$GREEN" +append "$work_dir/quadrant-top.png"
  magick -size "${half_w}x${half_h}" xc:"$BLUE" \
    -size "${half_w}x${half_h}" xc:"$WHITE" +append "$work_dir/quadrant-bottom.png"
  magick "$work_dir/quadrant-top.png" "$work_dir/quadrant-bottom.png" \
    -append -depth 8 -type TrueColor "PNG24:$out"
}

# Plasma is content only: random per generation, never compared across runs.
plasma_png() {
  local out=$1 width=$2 height=$3
  magick -size "${width}x${height}" plasma: -depth 8 -type TrueColor "PNG24:$out"
}

plasma_jpeg() {
  local out=$1 width=$2 height=$3
  magick -size "${width}x${height}" plasma: -quality 85 -sampling-factor 4:4:4 "$out"
}

generate_corpus() {
  local dir=$1 index name

  mkdir -p -- "$dir/ui/nested" "$dir/edge" "$dir/heic" \
    "$dir/estimate" "$dir/batch-cancel" "$dir/batch"

  # -- ui/ ---------------------------------------------------------------
  # Stored 1200x1600 quadrant with EXIF orientation 6 (rotate 90 clockwise):
  # the app decodes it as 1600x1200 with corners B R / W G.
  quadrant_png "$work_dir/tall.png" 1200 1600
  magick "$work_dir/tall.png" -quality 92 -sampling-factor 4:4:4 "$dir/ui/portrait.jpg"
  exiv2 -M "set Exif.Image.Orientation 6" "$dir/ui/portrait.jpg"

  # Stored 1600x1200 quadrant rotated 90 clockwise at encode time, so the HEIC
  # container carries an irot box: libheif decodes it as 1200x1600, corners
  # B R / W G.
  quadrant_png "$work_dir/wide.png" 1600 1200
  heif-enc --rotate-cw 90 "$work_dir/wide.png" -o "$dir/ui/rotated.heic"

  # 900x600 RGBA: three quadrants painted, bottom-right left transparent.
  magick -size 900x600 xc:none \
    -fill "$RED" -draw 'rectangle 0,0 449,299' \
    -fill "$GREEN" -draw 'rectangle 450,0 899,299' \
    -fill "$BLUE" -draw 'rectangle 0,300 449,599' \
    PNG32:"$dir/ui/alpha.png"

  plasma_png "$dir/ui/big.png" 6000 4000
  plasma_png "$dir/ui/nested/photo.png" 900 600

  # -- edge/ -------------------------------------------------------------
  # Ten bytes of ASCII behind a .png name: valid content, invalid image.
  printf 'not a png\n' >"$dir/edge/garbage.png"
  # Permission-denied fixture. Mode 000 is applied after hashing.
  quadrant_png "$dir/edge/denied.png" 900 600

  # -- heic/ -------------------------------------------------------------
  plasma_png "$dir/heic/source.png" 1200 900

  # -- estimate/ ---------------------------------------------------------
  for index in $(seq 1 7); do
    name=$(printf 'photo-%02d.jpg' "$index")
    plasma_jpeg "$dir/estimate/$name" 1600 1200
  done

  # -- batch-cancel/ -----------------------------------------------------
  # 120 copies of one 1600x1200 JPEG, so Escape mid-batch has a long queue.
  plasma_jpeg "$work_dir/shot-master.jpg" 1600 1200
  for index in $(seq 1 120); do
    name=$(printf 'shot-%03d.jpg' "$index")
    cp -- "$work_dir/shot-master.jpg" "$dir/batch-cancel/$name"
  done

  # -- batch/ ------------------------------------------------------------
  # 116 files over 3000x2000 plus four at 6100x4100 (25.01 MP).
  plasma_jpeg "$work_dir/img-master.jpg" 3000 2000
  for index in $(seq 1 116); do
    name=$(printf 'img-%03d.jpg' "$index")
    cp -- "$work_dir/img-master.jpg" "$dir/batch/$name"
  done
  plasma_jpeg "$work_dir/big-master.jpg" 6100 4100
  for index in $(seq 1 4); do
    name=$(printf 'big-%03d.jpg' "$index")
    cp -- "$work_dir/big-master.jpg" "$dir/batch/$name"
  done
}

# --------------------------------------------------------------- self-check --

image_dims() {
  local file=$1
  if [ "${2-}" = auto-orient ]; then
    magick identify -auto-orient -format '%wx%h' "$file"
  else
    magick identify -format '%wx%h' "$file"
  fi
}

image_corners() {
  local file=$1 fmt='%[hex:p{4,4}] %[hex:p{W-5,4}] %[hex:p{4,H-5}] %[hex:p{W-5,H-5}]'
  if [ "${2-}" = auto-orient ]; then
    magick identify -auto-orient -format "$fmt" "$file"
  else
    magick identify -format "$fmt" "$file"
  fi
}

# Compare two hex colors channel by channel within CORNER_TOLERANCE.
channel_within() {
  local got=${1:0:6} want=${2:0:6} offset a b delta
  for offset in 0 2 4; do
    a=$((16#${got:offset:2}))
    b=$((16#${want:offset:2}))
    delta=$((a > b ? a - b : b - a))
    [ "$delta" -le "$CORNER_TOLERANCE" ] || return 1
  done
}

# assert_fixture <file> <decoded dims> <TL TR BL BR hex> [auto-orient]
assert_fixture() {
  local file=$1 want_dims=$2 want_corners=$3 orient=${4-} actual_dims index
  local -a want_list corners
  read -r -a want_list <<<"$want_corners"
  [ "${#want_list[@]}" -eq 4 ] || die "internal error: expected 4 corner colors for $file"

  actual_dims=$(image_dims "$file" "$orient")
  [ "$actual_dims" = "$want_dims" ] ||
    die "fixture check failed: $file decodes to $actual_dims, expected $want_dims"

  read -r -a corners <<<"$(image_corners "$file" "$orient")"
  [ "${#corners[@]}" -eq 4 ] || die "fixture check failed: $file returned ${#corners[@]} corners, expected 4"
  for index in 0 1 2 3; do
    channel_within "${corners[index]}" "${want_list[index]}" ||
      die "fixture check failed: $file corner $((index + 1)) is ${corners[index]}, expected ${want_list[index]} (tolerance $CORNER_TOLERANCE)"
  done
  printf 'fixture check: %s -> %s, corners %s\n' "$file" "$want_dims" "$want_corners"
}

# ------------------------------------------------------------------ sample ---

write_expected_md() {
  local dir=$1
  {
    printf '# Manual check corpus — expected facts\n\n'
    printf 'Scratch directory: `%s`\n\n' "$dir"
    cat <<'EOF'
Generated by `scripts/manual-check.sh sample`. Every file is synthetic: solid
color quadrants drawn with ImageMagick and ImageMagick plasma. Nothing is
downloaded, and `sample` rebuilds the whole corpus on every run.

`sources.sha256` lists 254 of the 255 images. `edge/denied.png` is left out
because it is the permission-denied fixture, is mode `000`, and must stay
unreadable. Plasma content is random per generation, so the manifest describes
this generation only: it exists to prove that a manual run did not modify its
sources, not to compare corpora across runs.

Check the manifest with:

    cd <scratch dir> && sha256sum -c sources.sha256

## Corner convention

Quadrant fixtures use the feature-02 palette as 8-bit sRGB hex:

| Quadrant     | Hex       | Letter |
| ------------ | --------- | ------ |
| top-left     | `#e02020` | R      |
| top-right    | `#20c020` | G      |
| bottom-left  | `#2040e0` | B      |
| bottom-right | `#f0f0f0` | W      |

Read the four corners of a decoded image in `TL TR / BL BR` order with:

    magick identify -format "%[pixel:p{4,4}] %[pixel:p{W-5,4}] %[pixel:p{4,H-5}] %[pixel:p{W-5,H-5}]" FILE

The expectations below describe the **decoded** image, because the app applies
EXIF orientation and the HEIC container rotation while decoding. ImageMagick
reports the *stored* pixels of a tagged JPEG, so add `-auto-orient` for
`ui/portrait.jpg`; libheif applies the `irot` box on decode, so
`ui/rotated.heic` needs no flag. HEIC is lossy: compare with a tolerance of 8
per channel, the same tolerance the feature-02 manifest uses.

## Inventory

| Path | Files | Stored | Decoded / expected |
| ---- | ----- | ------ | ------------------ |
| `ui/portrait.jpg` | 1 | 1200x1600 quadrant, EXIF orientation 6 | 1600x1200, corners B R / W G |
| `ui/rotated.heic` | 1 | 1600x1200 quadrant, `irot` 90 clockwise | 1200x1600, corners B R / W G |
| `ui/alpha.png` | 1 | 900x600 RGBA, three quadrants painted | bottom-right corner transparent |
| `ui/big.png` | 1 | 6000x4000 plasma | "Longest edge 1920 px" gives 1920x1280 |
| `ui/nested/photo.png` | 1 | 900x600 plasma | output mirrors the `nested/` folder |
| `edge/garbage.png` | 1 | ten bytes of ASCII named `.png` | decode failure |
| `edge/denied.png` | 1 | 900x600 quadrant, mode `000` | permission-denied failure; not hashed |
| `heic/source.png` | 1 | 1200x900 plasma | HEIC output decodes 1200x900 |
| `estimate/photo-01.jpg` ... `photo-07.jpg` | 7 | 1600x1200 plasma | seven outputs, `photo-0N (1).jpg` |
| `batch-cancel/shot-001.jpg` ... `shot-120.jpg` | 120 | 1600x1200 plasma copies | Escape mid-batch leaves a partial set |
| `batch/img-001.jpg` ... `img-116.jpg` | 116 | 3000x2000 plasma copies | "Keep original size" keeps 3000x2000 |
| `batch/big-001.jpg` ... `big-004.jpg` | 4 | 6100x4100 (25.01 MP) plasma copies | completed at 6100x4100 |
| **total** | **255** | | 254 hashed |

## Expected conversion results

With PNG selected and "Longest edge 1920 px":

- `ui/portrait.jpg` -> `portrait.png`, 1600x1200, corners B R / W G
- `ui/rotated.heic` -> `rotated.png`, 1200x1600, corners B R / W G
- `ui/alpha.png` -> `alpha (1).png`, 900x600, alpha preserved
- `ui/big.png` -> `big (1).png`, 1920x1280
- `ui/nested/photo.png` -> `nested/photo (1).png`, 900x600

With PNG selected and "Keep original size", `batch/` produces 120 outputs:
116 at 3000x2000 and 4 at 6100x4100.
EOF
  } >"$dir/EXPECTED.md"
}

print_counts() {
  local dir=$1 subdir files total=0
  for subdir in "${CORPUS_DIRS[@]}"; do
    files=$(find "$dir/$subdir" -type f | wc -l)
    files=${files// /}
    total=$((total + files))
    printf '  %-14s %3d file(s)\n' "$subdir/" "$files"
  done
  printf '  %-14s %3d file(s)\n' "total" "$total"
}

cmd_sample() {
  local dir hashed total_images existing=0

  [ $# -le 1 ] || usage_error "sample takes at most one directory"
  dir=$(resolve_dir "${1:-$MANUAL_DIR}")
  require_tools magick exiv2 heif-enc

  # Refuse an unmarked directory before anything else; only a directory that
  # carried the marker is ever deleted.
  if [ -e "$dir" ] || [ -L "$dir" ]; then
    assert_marked "$dir"
    existing=1
  fi
  check_free_space "$dir"
  [ "$existing" -eq 0 ] || rm -rf -- "$dir"
  mkdir -p -- "$dir"
  printf 'HEIC Image Converter manual-check scratch directory.\nCreated by scripts/manual-check.sh sample; remove it with scripts/manual-check.sh cleanup.\n' \
    >"$dir/$MARKER"

  work_dir=$(mktemp -d "${TMPDIR:-/tmp}/heic-manual-work.XXXXXX")
  printf 'rebuilding corpus in %s\n' "$dir"
  generate_corpus "$dir"

  # Prove the two rotation-sensitive fixtures decode as the corpus claims.
  assert_fixture "$dir/ui/portrait.jpg" 1600x1200 "2040E0 E01F20 F0F0F0 20C020" auto-orient
  assert_fixture "$dir/ui/rotated.heic" 1200x1600 "2040E0 E01F20 F0F0F0 20C020"

  # Hash every generated image except the mode-000 fixture, with paths
  # relative to the scratch directory.
  (
    cd "$dir"
    find "${CORPUS_DIRS[@]}" -type f \
      \( -name '*.png' -o -name '*.jpg' -o -name '*.heic' \) \
      ! -path 'edge/denied.png' -print0 |
      LC_ALL=C sort -z |
      xargs -0 -r sha256sum >sources.sha256
  )
  hashed=$(wc -l <"$dir/sources.sha256")
  hashed=${hashed// /}
  total_images=$(
    cd "$dir"
    find "${CORPUS_DIRS[@]}" -type f \
      \( -name '*.png' -o -name '*.jpg' -o -name '*.heic' \) | wc -l
  )
  total_images=${total_images// /}

  write_expected_md "$dir"

  # Last: make the permission-denied fixture unreadable. It is not in the
  # manifest, so `sha256sum -c` never hits it.
  chmod 000 "$dir/edge/denied.png"

  printf '\ncorpus ready: %s\n' "$dir"
  print_counts "$dir"
  printf '  hashed %d of %d images into sources.sha256 (edge/denied.png excluded, mode 000)\n' \
    "$hashed" "$total_images"
  printf '\nverify the sources before or after a manual run:\n'
  printf '  cd %q && sha256sum -c sources.sha256\n' "$dir"
  printf '\nwhen finished, remove the corpus:\n'
  printf '  scripts/manual-check.sh cleanup\n'
}

# ----------------------------------------------------------------- cleanup ---

cmd_cleanup() {
  local dir

  [ $# -le 1 ] || usage_error "cleanup takes at most one directory"
  dir=$(resolve_dir "${1:-$MANUAL_DIR}")

  if [ ! -e "$dir" ] && [ ! -L "$dir" ]; then
    printf 'nothing to clean up: %s does not exist\n' "$dir"
    return 0
  fi
  assert_marked "$dir"

  if [ -f "$dir/edge/denied.png" ]; then
    chmod 644 "$dir/edge/denied.png"
    printf 'restored mode 644: %s\n' "$dir/edge/denied.png"
  fi

  rm -rf -- "$dir"
  [ ! -e "$dir" ] || die "failed to remove $dir"
  printf 'removed %s\n' "$dir"
}

# --------------------------------------------------------------- app window --

# Milliseconds, for the wait/report timings below. Infallible on purpose: every
# caller runs after the app has been started, and a failed timestamp read must
# not take `set -e` down a path that leaves the child running.
now_ms() {
  date +%s%3N 2>/dev/null || printf '0'
}

pid_alive() {
  local pid=${1-}
  [ -n "$pid" ] || return 1
  case "$pid" in
    *[!0-9]*) return 1 ;;
  esac
  kill -0 "$pid" 2>/dev/null
}

# Hyprland is the only supported session: the harness drives the live window
# through hyprctl and grim, so a missing instance is a hard stop with a reason.
require_window_tools() {
  require_tools hyprctl jq
  if ! hyprctl clients -j >/dev/null 2>&1; then
    die "cannot reach Hyprland (HYPRLAND_INSTANCE_SIGNATURE=${HYPRLAND_INSTANCE_SIGNATURE:-unset}); launch, focus, shot and close drive the live window"
  fi
}

# Print the matching client as compact JSON, or nothing when no window matches.
#
# Matching rule: the title is authoritative. The app window is exactly
# "HEIC Image Converter" (tauri.conf.json); the class "heic-image-converter" is
# used only when a window reports an empty title, which is what a crashed or
# still-mapping window can look like. Matching on the class alone is avoided
# because it would also match a stale surface. jq turns "no window" into an
# empty result instead of an error, so absence is handled, not crashed on.
app_client_json() {
  hyprctl clients -j 2>/dev/null |
    jq -c --arg title "$APP_TITLE" --arg class "$APP_CLASS" '
      [ .[] | select(((.title // "") == $title)
                     or (((.title // "") == "") and ((.class // "") == $class))) ]
      | .[0] // empty' 2>/dev/null || true
}

# app_client_field <jq filter>: one scalar field of the matching client.
app_client_field() {
  local filter=$1 json
  json=$(app_client_json)
  [ -n "$json" ] || return 1
  printf '%s\n' "$json" | jq -r "$filter"
}

# Hyprland reports `at` as [x, y] and `size` as [w, h] in *global layout*
# coordinates. grim -g reads the same layout space, so the geometry is passed
# through unchanged - including negative coordinates (a monitor placed left of
# or above the primary one) and the monitor scale, which makes the PNG larger
# than the layout size on a HiDPI output.
app_geometry() {
  local json
  json=$(app_client_json)
  [ -n "$json" ] || return 1
  printf '%s\n' "$json" |
    jq -r '"\(.at[0]),\(.at[1]) \(.size[0])x\(.size[1])"'
}

wait_for_app_window() {
  local deadline
  deadline=$(($(now_ms) + WINDOW_WAIT_SECONDS * 1000))
  while [ -z "$(app_client_json)" ]; do
    [ "$(now_ms)" -lt "$deadline" ] || return 1
    sleep 0.2
  done
  return 0
}

wait_for_window_gone() {
  local seconds=$1 deadline
  deadline=$(($(now_ms) + seconds * 1000))
  while [ -n "$(app_client_json)" ]; do
    [ "$(now_ms)" -lt "$deadline" ] || return 1
    sleep 0.1
  done
  return 0
}

wait_for_pid_exit() {
  local pid=$1 seconds=$2 deadline
  deadline=$(($(now_ms) + seconds * 1000))
  while pid_alive "$pid"; do
    [ "$(now_ms)" -lt "$deadline" ] || return 1
    sleep 0.1
  done
  return 0
}

# A failed launch must not leave an orphan window behind or a pid file that
# claims a process we walked away from: stop the child, confirm it exited, wait
# (best effort) for its surface to go, and drop the record before reporting.
abort_launch() {
  local pid=$1
  kill -TERM "$pid" 2>/dev/null || true
  if ! wait_for_pid_exit "$pid" 2; then
    kill -KILL "$pid" 2>/dev/null || true
    wait_for_pid_exit "$pid" 2 || true
  fi
  # A surface can outlive its process by a frame; a lingering one must not turn
  # a cleanup into a second failure, so this wait is best effort.
  wait_for_window_gone 2 || true
  rm -f -- "$app_pid_file" 2>/dev/null || true
}

# The single reaping mechanism for `launch`. cmd_launch arms this immediately
# after the child starts and clears it only once the launch is committed, so
# every exit path - an explicit `die`, a `set -e` trip on a statement nobody
# thought to guard, or a signal - reaps the child and drops the pid record.
# No error path calls abort_launch itself, so the child cannot be killed twice.
# (This replaces the global cleanup_work_dir EXIT trap for the duration of a
# launch; that trap only has work during `sample`, which is a separate run.)
launch_child=""
launch_committed=0

launch_trap() {
  local status=$?
  trap - EXIT INT TERM HUP
  if [ "${launch_committed:-0}" -eq 0 ] && [ -n "${launch_child:-}" ]; then
    abort_launch "$launch_child"
  fi
  exit "$status"
}

app_is_focused() {
  local address=$1 current
  current=$(hyprctl activewindow -j 2>/dev/null | jq -r '.address // ""' 2>/dev/null) || current=""
  [ "$current" = "$address" ]
}

# Focus by address: unique even if two windows ever shared the title. Hyprland
# 0.56 evaluates dispatch arguments as Lua dispatchers, so the classic
# `focuswindow address:...` form is tried first and the Lua form is the
# fallback for newer sessions (older Hyprland only knows the classic form).
focus_app_window() {
  local address deadline
  address=$(app_client_field .address) || return 1
  [ -n "$address" ] || return 1
  if ! hyprctl dispatch focuswindow "address:$address" >/dev/null 2>&1; then
    hyprctl dispatch "hl.dsp.focus({ window = 'address:$address' })" >/dev/null 2>&1 || return 1
  fi
  deadline=$(($(now_ms) + 2000))
  while :; do
    if app_is_focused "$address"; then
      return 0
    fi
    [ "$(now_ms)" -lt "$deadline" ] || return 1
    sleep 0.1
  done
}

# The scratch directory holds the corpus and the run artifacts (app.log,
# app.pid). `launch` may run before `sample`, so create it with its marker when
# absent; it is still protected by the same resolve/marker rules.
ensure_scratch_dir() {
  run_dir=$(resolve_dir "$MANUAL_DIR")
  if [ -e "$run_dir" ] || [ -L "$run_dir" ]; then
    assert_marked "$run_dir"
  else
    mkdir -p -- "$run_dir"
    printf 'HEIC Image Converter manual-check scratch directory.\nCreated by scripts/manual-check.sh sample; remove it with scripts/manual-check.sh cleanup.\n' \
      >"$run_dir/$MARKER"
    printf 'created scratch directory %s with its marker (run `sample` to build the corpus there)\n' "$run_dir"
  fi
  app_log="$run_dir/app.log"
  app_pid_file="$run_dir/app.pid"
}

# ---------------------------------------------------------------- launch -----

cmd_launch() {
  local binary pid start elapsed address window_pid geometry_wait
  require_window_tools

  binary="${APP_BIN:-$APP_BIN_DEFAULT}"
  [ -e "$binary" ] ||
    die "binary not found: $binary (build it with 'scripts/ci.sh build', or point APP_BIN at one)"
  [ -f "$binary" ] || die "not a regular file: $binary"
  [ -x "$binary" ] || die "binary is not executable: $binary"

  ensure_scratch_dir

  # A window without a PID we recorded cannot be closed or screenshotted
  # reliably, and two instances would fight over the same title.
  if [ -n "$(app_client_json)" ]; then
    die "a window titled '$APP_TITLE' is already open (pid $(app_client_field .pid 2>/dev/null || printf 'unknown')); run 'scripts/manual-check.sh close' first"
  fi

  printf 'launching: %s' "$binary"
  [ $# -eq 0 ] || printf ' %q' "$@"
  printf '\n  log: %s\n' "$app_log"
  # stdin from /dev/null so the window does not hold the caller's terminal.
  "$binary" "$@" >>"$app_log" 2>&1 </dev/null &
  pid=$!

  # From here on the child is live and the trap owns it: any exit that does not
  # reach the commit below stops the child and removes the pid record. The
  # signal traps give the EXIT trap the right status (130/143/129) instead of
  # bash's interrupted-command status.
  launch_child=$pid
  launch_committed=0
  trap 'launch_trap' EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  trap 'exit 129' HUP

  if ! printf '%s\n' "$pid" >"$app_pid_file" 2>/dev/null; then
    # Without the record the child could not be found again; the trap stops it
    # rather than leaving a running window nobody knows about.
    die "cannot write the pid file $app_pid_file; the app was stopped again"
  fi

  start=$(now_ms)
  if ! wait_for_app_window; then
    die "no window titled '$APP_TITLE' appeared within ${WINDOW_WAIT_SECONDS} s; app output: $app_log"
  fi
  elapsed=$(($(now_ms) - start))

  if ! focus_app_window; then
    die "window appeared (pid $pid) but could not be focused; app output: $app_log"
  fi
  if ! address=$(app_client_field .address); then
    die "the '$APP_TITLE' window was focused but its address could not be read (hyprctl returned no matching client); app output: $app_log"
  fi
  window_pid=$(app_client_field .pid 2>/dev/null || printf '')
  if ! geometry_wait=$(app_geometry); then
    die "the '$APP_TITLE' window was focused but its geometry could not be read (hyprctl returned no matching client); app output: $app_log"
  fi

  # Reporting only: the app is up and its pid is recorded, so a broken stdout
  # (closed pipe, full disk) must neither kill it nor leave it unreported.
  (
    set +e
    printf 'window ready in %d ms\n' "$elapsed"
    printf '  pid:    %s (Hyprland reports %s)\n' "$pid" "${window_pid:-unknown}"
    printf '  window: %-28s class %s\n' "$APP_TITLE" "$APP_CLASS"
    printf '  focus:  %s\n' "$address"
    printf '  geometry: %s (global layout coordinates)\n' "$geometry_wait"
    printf '\nkeyboard rules (pointer synthesis is unavailable):\n'
    printf '  wtype -k Tab                    next control\n'
    printf '  wtype -M shift -k Tab -m shift  previous control\n'
    printf '  wtype -k space                  activate the focused button or checkbox\n'
    printf '  wtype -k Home / -k Down / 1-9   pick a select entry\n'
    printf '  wtype -M ctrl -k Return -m ctrl convert the queue\n'
    printf '  wtype -k Escape                 cancel a batch (the picker dialog is a\n'
    printf '                                  separate window; close the app to leave it)\n'
    printf '\nscreenshot the app window:  scripts/manual-check.sh shot M5-results\n'
    printf 'finish:                     scripts/manual-check.sh close\n'
    printf 'the full M1-M11 checklist:  scripts/manual-check.sh --help\n'
  ) || true

  # Committed: the pid file is written, the window is focused and recorded, and
  # the report has been attempted. The app is the caller's now; `close` owns it.
  launch_committed=1
  trap - EXIT INT TERM HUP
}

# ----------------------------------------------------------------- focus -----

cmd_focus() {
  local address
  [ $# -eq 0 ] || usage_error "focus takes no arguments"
  require_window_tools
  run_dir=$(resolve_dir "$MANUAL_DIR")
  app_log="$run_dir/app.log"
  app_pid_file="$run_dir/app.pid"

  if [ -z "$(app_client_json)" ]; then
    if [ -f "$app_pid_file" ]; then
      die "no window titled '$APP_TITLE' is open (recorded pid $(tr -dc '0-9' <"$app_pid_file") is not showing one); start the app with 'launch'"
    fi
    die "no window titled '$APP_TITLE' is open; start the app with 'launch'"
  fi

  focus_app_window || die "could not focus the '$APP_TITLE' window"
  address=$(app_client_field .address)
  printf 'focused %s (%s) at %s\n' "$APP_TITLE" "$address" "$(app_geometry)"
}

# ------------------------------------------------------------------ shot -----

cmd_shot() {
  local name dir file geometry capture_dims want_dims
  [ $# -eq 1 ] || usage_error "shot takes exactly one name (for example: shot M5-results)"
  name=${1%.png}
  case "$name" in
    "" | . | .. | */*) usage_error "shot name must be a plain name without directories: '$1'" ;;
  esac
  require_tools grim magick
  require_window_tools

  if [ -z "$(app_client_json)" ]; then
    die "no window titled '$APP_TITLE' is open; start the app with 'launch'"
  fi
  focus_app_window || die "could not focus the '$APP_TITLE' window"
  # Focus can switch workspaces and resize a tiled window, and the compositor
  # needs a moment for the focus animation and the repaint before grim reads
  # the output - so read the geometry *after* focusing, then settle.
  sleep 0.5
  geometry=$(app_geometry) || die "the '$APP_TITLE' window vanished before it could be captured"
  want_dims=$(app_client_json | jq -r '"\(.size[0])x\(.size[1])"')

  dir=${SHOT_DIR:-docs/verification/$(date +%F)}
  mkdir -p -- "$dir"
  file="$dir/$name.png"

  grim -g "$geometry" "$file" || die "grim failed to capture '$geometry' into $file"
  capture_dims=$(magick identify -format '%wx%h' "$file" 2>/dev/null) ||
    die "grim wrote $file but it is not a readable image"
  [ -s "$file" ] || die "grim wrote an empty file: $file"
  printf 'wrote %s (%s, window %s at %s)\n' "$file" "$capture_dims" "$want_dims" "$geometry"
  if [ "$capture_dims" != "$want_dims" ]; then
    printf 'note: the capture is %s and the window is %s in layout pixels; a monitor scale other than 1 makes the PNG larger\n' \
      "$capture_dims" "$want_dims"
  fi
}

# ----------------------------------------------------------------- close -----

cmd_close() {
  local pid window_pid start elapsed
  [ $# -eq 0 ] || usage_error "close takes no arguments"
  require_tools hyprctl jq

  run_dir=$(resolve_dir "$MANUAL_DIR")
  app_log="$run_dir/app.log"
  app_pid_file="$run_dir/app.pid"

  pid=""
  if [ -f "$app_pid_file" ]; then
    pid=$(tr -dc '0-9' <"$app_pid_file")
  fi
  # A crashed earlier run leaves a stale pid file (or none). Hyprland's own
  # record of the window's pid is the fallback: it is the same process.
  if ! pid_alive "$pid"; then
    window_pid=$(app_client_field .pid 2>/dev/null) || window_pid=""
    pid=$window_pid
  fi

  if ! pid_alive "$pid"; then
    rm -f "$app_pid_file"
    if [ -n "$(app_client_json)" ]; then
      die "a window titled '$APP_TITLE' is open but no live pid is recorded; close it by hand"
    fi
    printf 'nothing to close: no running app and no window titled %s\n' "$APP_TITLE"
    return 0
  fi

  start=$(now_ms)
  printf 'terminating pid %s\n' "$pid"
  kill -TERM "$pid" 2>/dev/null || true
  if ! wait_for_pid_exit "$pid" "$CLOSE_WAIT_SECONDS"; then
    printf 'pid %s is still running after %s s; sending SIGKILL\n' "$pid" "$CLOSE_WAIT_SECONDS"
    kill -KILL "$pid" 2>/dev/null || true
    wait_for_pid_exit "$pid" 2 || die "pid $pid survived SIGKILL"
  fi
  # The window can outlive the process by a frame; `launch` refuses to start
  # while one is open, so wait for the surface to go too.
  wait_for_window_gone 2 || true
  elapsed=$(($(now_ms) - start))
  rm -f "$app_pid_file"

  if [ -n "$(app_client_json)" ]; then
    die "pid $pid exited but a window titled '$APP_TITLE' is still mapped"
  fi
  printf 'closed: pid %s gone in %d ms, no window titled %s left\n' \
    "$pid" "$elapsed" "$APP_TITLE"
}

# -------------------------------------------------------------------- main ---

cleanup_work_dir() {
  if [ -n "$work_dir" ] && [ -d "$work_dir" ]; then
    rm -rf -- "$work_dir"
  fi
}
trap cleanup_work_dir EXIT

command="${1-}"
if [ $# -gt 0 ]; then
  shift
fi

case "$command" in
  sample)
    cmd_sample "$@"
    ;;
  launch)
    cmd_launch "$@"
    ;;
  focus)
    cmd_focus "$@"
    ;;
  shot)
    cmd_shot "$@"
    ;;
  close)
    cmd_close "$@"
    ;;
  cleanup)
    cmd_cleanup "$@"
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
    printf 'Unknown command: %s\n\n' "$command" >&2
    usage >&2
    exit 2
    ;;
esac
