#!/usr/bin/env bash
#
# Regenerate the synthetic fixture corpus in this directory.
#
# Every fixture is drawn locally from solid colors: nothing is downloaded and no
# third-party image is involved. Requires ImageMagick 7 (`magick`), `heif-enc`,
# `heif-info`, and `exiv2` on the developer machine. CI never runs this script;
# it only reads the committed fixtures and their manifest.
#
# Usage:
#   ./generate.sh
#
# Palette (from the feature spec): R = #e02020, G = #20c020, B = #2040e0,
# W = #f0f0f0, and the grays 32/96/160/224 = #202020/#606060/#a0a0a0/#e0e0e0.

set -euo pipefail
cd "$(dirname "$0")"

for tool in magick heif-enc heif-info exiv2; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "error: required tool not found: $tool" >&2
    exit 1
  fi
done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# ------------------------------------------------------------------ masters --

# PNGs are written without timestamp chunks so re-running the script reproduces
# the same bytes (and therefore the same manifest hashes).
png_opts=(-define png:exclude-chunks=date,time)

# 40x30 quadrant master: top-left red, top-right green, bottom-left blue,
# bottom-right white, stored as 8-bit RGB.
magick -size 20x15 xc:'#e02020' -size 20x15 xc:'#20c020' +append "$work/top.png"
magick -size 20x15 xc:'#2040e0' -size 20x15 xc:'#f0f0f0' +append "$work/bottom.png"
magick "$work/top.png" "$work/bottom.png" -append -depth 8 -type TrueColor \
  "${png_opts[@]}" PNG24:"$work/master.png"

# 40x30 grayscale master: 32 / 96 / 160 / 224 in the same quadrants.
magick -size 20x15 xc:'#202020' -size 20x15 xc:'#606060' +append "$work/gray-top.png"
magick -size 20x15 xc:'#a0a0a0' -size 20x15 xc:'#e0e0e0' +append "$work/gray-bottom.png"
magick "$work/gray-top.png" "$work/gray-bottom.png" -append -depth 8 "$work/gray-master.png"

# 40x30 JPEG base (quality 92, no chroma subsampling). Reused by the
# EXIF-orientation fixtures and by truncated.jpg.
magick "$work/master.png" -quality 92 -sampling-factor 4:4:4 "$work/base.jpg"

# --------------------------------------------------------------------- PNG ---

# Plain 8-bit RGB.
cp "$work/master.png" photo.png

# 8-bit RGBA: three quadrants painted, bottom-right left unpainted (transparent).
magick -size 40x30 xc:none \
  -fill '#e02020' -draw 'rectangle 0,0 19,14' \
  -fill '#20c020' -draw 'rectangle 20,0 39,14' \
  -fill '#2040e0' -draw 'rectangle 0,15 19,29' \
  "${png_opts[@]}" PNG32:alpha.png

# 8-bit grayscale (single channel, no alpha).
magick "$work/gray-master.png" -depth 8 -colorspace Gray \
  -define png:color-type=0 "${png_opts[@]}" gray.png

# 16-bit RGB: PNG48 / color type 2 / 16 bits per channel.
magick "$work/master.png" -depth 16 \
  -define png:bit-depth=16 -define png:color-type=2 \
  "${png_opts[@]}" PNG48:deep16.png

# ------------------------------------------------------------ animations ----

# Three uniform frames (red, green, blue), 100 ms each, looping forever.
magick -size 40x30 xc:'#e02020' -size 40x30 xc:'#20c020' -size 40x30 xc:'#2040e0' \
  -set delay 10 -loop 0 anim.gif

# ------------------------------------------------------- other still formats --

magick "$work/master.png" -quality 80 photo.webp
# Deflate keeps the file tiny; the image crate's TIFF decoder reads it.
magick "$work/master.png" -compress Zip photo.tiff
# BMP3 is the 24-bit Windows 3.x variant.
magick "$work/master.png" BMP3:photo.bmp

# ------------------------------------------------- EXIF orientation JPEGs ----

# Eight copies of the same 40x30 stored pixels, each tagged with a different
# Exif.Image.Orientation (1-8). The app applies the tag while decoding, so the
# stored image stays 40x30 and only orientations 5-8 decode to 30x40.
# exiv2 is deterministic here: copying the base and setting the tag reproduces
# the same bytes on every run.
for n in 1 2 3 4 5 6 7 8; do
  cp "$work/base.jpg" "orientation-$n.jpg"
  exiv2 -M "set Exif.Image.Orientation $n" "orientation-$n.jpg"
done

# -------------------------------------------------------------------- HEIC ---

# All four HEICs come from the PNG masters above through libheif's `heif-enc`.
# libheif writes no timestamps, so running this block again reproduces the same
# bytes. Note that `heif-enc` takes 8 bits from an 8-bit PNG; `-b` only selects
# how many of a 16-bit input's bits are kept.

# 8-bit HEIC: the format-matrix counterpart to photo.png (main brand heic).
heif-enc "$work/master.png" -o photo.heic

# 12-bit HEIC from the 16-bit master, so the container carries a 12-bit profile
# (main brand heix) and decodes to a 16-bit image. Proves high-bit-depth input.
heif-enc -b 12 deep16.png -o deep12.heic

# Two-image HEIC: the quadrant master is the primary image, a 20x15 black PNG is
# the auxiliary. The app converts the primary image only.
magick -size 20x15 xc:black "${png_opts[@]}" PNG24:"$work/black.png"
heif-enc "$work/master.png" "$work/black.png" -o burst.heic

# Rotated HEIC: a 160x120 quadrant master rotated 90 degrees clockwise at encode
# time, so the container carries an `irot` box. libheif reports and decodes
# 120x160 with the quadrants turned (TL blue, TR red, BL white, BR green), which
# is what decode::tests::applies_heic_container_rotation relies on.
magick -size 80x60 xc:'#e02020' -size 80x60 xc:'#20c020' +append "$work/wide-top.png"
magick -size 80x60 xc:'#2040e0' -size 80x60 xc:'#f0f0f0' +append "$work/wide-bottom.png"
magick "$work/wide-top.png" "$work/wide-bottom.png" -append -depth 8 -type TrueColor \
  "${png_opts[@]}" PNG24:"$work/wide.png"
heif-enc --rotate-cw 90 "$work/wide.png" -o rotated.heic

# ----------------------------------------------------------- corrupt inputs --

# A prefix of orientation-1.jpg: 200 bytes stops inside the frame headers, which
# neither ImageMagick nor the image crate can decode. Re-checked by the corpus
# integrity suite.
head -c 200 orientation-1.jpg > truncated.jpg

# ASCII bytes behind a .png name: valid content, invalid image.
printf 'not a png\n' > garbage.png

# ------------------------------------------------------- HEIF postconditions --

# These are assertions, not a report: a HEIF fixture with the wrong bit depth,
# image count, or dimensions must fail the script, so a broken corpus can never
# be committed without the failure being noticed.

fail() {
  echo "postcondition failed: $1" >&2
  exit 1
}

# Require `heif-info <file>` to report a line matching an extended regex.
assert_heif_line() {
  local file=$1 pattern=$2 what=$3 report
  report=$(heif-info "$file")
  if ! grep -qE "$pattern" <<<"$report"; then
    echo "postcondition failed: $file does not report $what" >&2
    echo "$report" >&2
    exit 1
  fi
  echo "postcondition: $file reports $what"
}

# photo.heic: 8-bit HEVC, one 40x30 primary image.
assert_heif_line photo.heic '^  bit depth: 8$' 'bit depth 8'

# deep12.heic: 12-bit HEVC from the 16-bit master.
assert_heif_line deep12.heic '^  bit depth: 12$' 'bit depth 12'

# burst.heic: exactly two images, primary (first) 40x30 and auxiliary 20x15.
burst_report=$(heif-info burst.heic)
burst_images=$(grep -c '^image:' <<<"$burst_report" || true)
[ "$burst_images" -eq 2 ] ||
  fail "burst.heic has $burst_images image(s), expected 2"
grep -qE '^image: 40x30 .*primary' <<<"$burst_report" ||
  fail "burst.heic primary image is not 40x30"
grep -qE '^image: 20x15 ' <<<"$burst_report" ||
  fail "burst.heic secondary image is not 20x15"
echo "postcondition: burst.heic reports exactly two images (primary 40x30, second 20x15)"

# rotated.heic: 120x160 after the container rotation is applied.
assert_heif_line rotated.heic '^image: 120x160 ' 'dimensions 120x160'

# ------------------------------------------------------------------- report --

echo "generated fixtures:"
total=0
for f in photo.png alpha.png gray.png deep16.png anim.gif photo.webp photo.tiff \
  photo.bmp photo.heic deep12.heic burst.heic rotated.heic \
  orientation-{1..8}.jpg truncated.jpg garbage.png; do
  bytes=$(wc -c < "$f")
  total=$((total + bytes))
  printf '  %-18s %6s bytes\n' "$f" "$bytes"
done
printf '  %-18s %6s bytes\n' "total (22 fixtures)" "$total"

cat <<'EOF'

Next steps (run from src-tauri/):
  cargo test -- --ignored update_fixture_manifest   # refresh bytes, sha256, README table
  cargo test                                        # verify the corpus integrity suite
EOF
