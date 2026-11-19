# Third-party notices

This repository statically links a prebuilt HEIC codec archive into the
application binary. This file records what that archive contains, which license
each component grants, and what follows for anyone who redistributes a build.

It is a factual record, not legal advice, and it does not declare the project
compliant: section 4 names an obligation this repository cannot currently
satisfy on its own.

## 1. What this binary contains

| Component | Version | License | Evidence |
| --- | --- | --- | --- |
| libheif | 1.23.6 | LGPL-3.0-or-later | `include/libheif/heif_version.h` inside `bundle_linux_x64.zip`: `#define LIBHEIF_VERSION "1.23.6"`, `#define LIBHEIF_NUMERIC_VERSION ((1<<24) \| (23<<16) \| (6<<8) \| 0)` |
| libde265 | 1.1.3 | LGPL-3.0-or-later | archive member `de265.cc.o` carries the string `1.1.3`; libde265's `heifrs_de265_get_version` family is linked |
| x265 | 4.3-vcpkg | GPL-2.0-or-later | archive member `version.cpp.o`: `4.3-vcpkg`, `[Linux][GCC 13.3.0][64 bit][noasm] 8bit`, `GCC: (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0` |
| heif-rs | 26.10.3 | Apache-2.0 | `src-tauri/vendor/heif-rs/Cargo.toml` (`version = "26.10.3"`, `license = "Apache-2.0"`) and its `LICENSE` |

The version evidence above comes from the committed archive
`src-tauri/vendor/heif-bundles/bundle_linux_x64.zip`, inspected with `unzip -l`,
`unzip -Z1`, and `strings -a lib/libheif.a`. Version strings are quoted, not
recalled. `heif_get_version()` returns the `LIBHEIF_VERSION` string compiled
into that archive.

An archive-integrity check: the extracted `lib/libheif.a` hashes to
`1e308363b99ff5ef69711ff9a4eea3d39c3eb590ba145656e73b22efeaa390b3`, which is
the `lib_sha256` the manifest records for this target.

The archive itself ships **no license or notice files**: it has 32 members, all
under `include/` and `lib/`, and
`unzip -Z1 bundle_linux_x64.zip | grep -icE 'license|copying|notice'` prints
`0`. The texts under [`licenses/`](licenses/) are this repository's substitute
for what the archive omits.

## 2. Why the application is GPL-3.0-or-later

x265 is licensed GPL-2.0-or-later and is statically linked into the same
executable as the application, so the conveyed binary is a single combined
work. The application is therefore distributed under GPL-3.0-or-later: GPL-3.0
is compatible with x265's "or later" option, and the root [`LICENSE`](LICENSE)
holds that text.

libheif and libde265 are LGPL-3.0-or-later, which GPL-3.0 also accommodates.
Their relink duty is not absorbed by the application's license — see section 3.

## 3. If you redistribute a build, you must

- **Convey the complete corresponding source** of the application at the
  released commit, under GPL-3.0-or-later, as [`LICENSE`](LICENSE) requires.
  That includes the build scripts and the `src-tauri/` tree as released.
- **Ship this notice and every text under [`licenses/`](licenses/)**, including
  `licenses/Apache-2.0.txt`, `licenses/GPL-2.0.txt`, and
  `licenses/LGPL-3.0.txt`.
- **Satisfy LGPL-3.0 §4 for libheif and libde265.** They are statically linked
  into the executable, so a source URL is not enough: the recipient needs the
  effective means to relink the application against a modified library — for
  example the application's object files or a buildable source tree plus the
  scripts to rebuild the binary. Providing only a link to upstream sources
  leaves this duty unmet.
- **Mark the codec modifications and supply their source.** The linked x265 and
  libde265 are modified versions (section 4), and the modification's source
  must travel with the binary.
- **Make no warranty claim.** The components carry their upstream disclaimers;
  the application's own terms apply to it alone.
- Keep the version, archive, and hash record in section 5 accurate for the
  artifact you actually shipped.

## 4. Modified codecs

The archive renames every non-API symbol with a `heifrs_` prefix. Examples drawn
from `strings -a lib/libheif.a`: `heifrs_de265_new_decoder`,
`heifrs_de265_get_version`, `heifrs_x265_version_str`,
`heifrs__ZN4x2658Analysis6createEPNS_15ThreadLocalDataE`. That renaming makes
the linked libde265 and x265 modified versions of their upstreams.

**This repository does not carry that patch or the archive's build recipe.**
`src-tauri/vendor/heif-rs/VENDORED.md` documents the Rust wrapper fork and its
build-script changes; it does not contain, and has never contained, the C/C++
changes applied to produce the static library. A redistributor therefore cannot
fully satisfy section 3 from this repository alone. The missing input is the
`binaries-heif` build recipe or the symbol-renaming patch for the pinned
release. Obtaining it is an open obligation owned by stage 04, and it blocks a
public binary release until it is cleared or the product manager accepts the
gap in writing.

## 5. Where the exact bytes come from

- **Archive release:** `26.10.1` of <https://github.com/vegidio/binaries-heif>,
  as recorded in `src-tauri/vendor/heif-bundles/manifest.json`.
- **URL template:**
  `https://github.com/vegidio/binaries-heif/releases/download/{release}/{archive}`
  (same manifest).
- **Manifest:** `src-tauri/vendor/heif-bundles/manifest.json` is the authority
  for every archive's name, byte length, SHA-256, library path, and library
  hash.
- **The inspected artifact:** `bundle_linux_x64.zip`, SHA-256
  `b5cbed5d08fc79f64b5171cd401fbd62da43333d26b5ad9905354704cf072f82`,
  3325156 bytes, library `lib/libheif.a`.
- **The other seven platform archives are hash-pinned in the manifest but were
  not byte-inspected here.** Their contents are assumed equivalent in
  composition; stages 05 and 06 own inspecting them against their own targets.

Four archives are committed to the repository (`bundle_linux_x64.zip`,
`bundle_osx_x64.zip`, `bundle_osx_arm64.zip`, `bundle_windows_x64.zip`); the
other four are fetched on demand and verified against the manifest before use.

## 6. Not covered here

This notice covers the statically linked HEIC codec stack and the `heif-rs`
wrapper. It does **not** inventory the Rust crate dependency tree or the npm
dependency tree that also end up in, or alongside, a shipped build. That
inventory belongs to stage 04, where the first real binary is produced and the
bundle contents are known.
