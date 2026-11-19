# License texts

This directory holds the canonical license texts the HEIC codec stack brings
into the binary, plus the record of where each text came from and which
component it covers.

The texts are **committed once, as published**. No build step, test, or script
fetches a license text, and nothing regenerates or reformats them. Their hashes
are pinned in [`SHA256SUMS`](SHA256SUMS) and re-checked by a test
(`license_texts_match_their_recorded_hashes` in
`src-tauri/src/source_review.rs`).

## The texts

| File | Identifier | Origin URL | Retrieved | Bytes | SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `Apache-2.0.txt` | Apache-2.0 | <https://www.apache.org/licenses/LICENSE-2.0.txt> | 2026-11-19 | 11358 | `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` |
| `GPL-2.0.txt` | GPL-2.0 | <https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt> | 2026-11-19 | 17984 | `edaef632cbb643e4e7a221717a6c441a4c1a7c918e6e4d56debc3d8739b233f6` |
| `LGPL-3.0.txt` | LGPL-3.0 | <https://www.gnu.org/licenses/lgpl-3.0.txt> | 2026-11-19 | 7652 | `e3a994d82e644b03a792a930f574002658412f62407f5fee083f2555c5f23118` |

Each text is stored byte-for-byte as served, with its original line endings and
without a trailing edit. The GPL-2.0 text is the copy under
`old-licenses/`, which is where GNU keeps the version-2-only text after moving
the current license set to GPL-3.0.

## Component to license mapping

The "identifier" column is the grant the upstream file states, not an
inference from the component's name. Versions are the ones the linked archive
reports; the evidence for each is in
[`../THIRD-PARTY-NOTICES.md`](../THIRD-PARTY-NOTICES.md).

| Component | Version | Identifier | Upstream file read |
| --- | --- | --- | --- |
| libheif | 1.23.6 | LGPL-3.0-or-later | <https://raw.githubusercontent.com/strukturag/libheif/v1.23.6/COPYING> |
| libde265 | 1.1.3 | LGPL-3.0-or-later | <https://raw.githubusercontent.com/strukturag/libde265/v1.1.3/COPYING> |
| x265 | 4.3-vcpkg | GPL-2.0-or-later | <https://bitbucket.org/multicoreware/x265_git/raw/4.2/COPYING> |
| heif-rs | 26.10.3 | Apache-2.0 | <https://raw.githubusercontent.com/vegidio/heif-rs/f8d7a972c59d5fb3f9a29c9669c377c93a08418a/LICENSE> |

Notes on what each read established:

- **libheif** — the tag's `COPYING` states the library is under the GNU Lesser
  General Public License and reproduces LGPL-3.0. The `or later` part is stated
  in the library's own file headers; the copy inside the committed archive,
  `include/libheif/heif_version.h`, says "either version 3 of the License, or
  (at your option) any later version".
- **libde265** — the tag's `COPYING` states the library is under the GNU Lesser
  General Public License and reproduces LGPL-3.0. The `libde265/de265.h` header
  at that tag grants "either version 3 of the License, or (at your option) any
  later version". The archive ships no headers for this component, so the text
  and the tag header are the record.
- **x265** — the upstream tag list published by `multicoreware/x265_git` ended
  at `4.2` when this was read, while the linked archive identifies itself as
  `4.3-vcpkg`; the `COPYING` read is the 4.2 tag's, which reproduces GPL-2.0.
  The "or later" grant comes from the `source/x265.h` header at that tag
  ("either version 2 of the License, or (at your option) any later version").
  The version the binary actually carries is the archive's own banner, quoted
  in the notices file.
- **heif-rs** — the upstream `LICENSE` at the recorded commit
  `f8d7a972c59d5fb3f9a29c9669c377c93a08418a` is byte-identical to the
  `src-tauri/vendor/heif-rs/LICENSE` this repository builds from, and is the
  Apache License, Version 2.0. `Cargo.toml` declares `license = "Apache-2.0"`.

## Why three texts and not four

The application itself is GPL-3.0-or-later; its text is the root
[`../LICENSE`](../LICENSE) and is not duplicated here. Apache-2.0, GPL-2.0, and
LGPL-3.0 are the texts a redistributor needs for the components on top of the
application's own license.

## Re-checking

From the repository root:

```sh
sha256sum -c licenses/SHA256SUMS
```

Three `OK` lines and exit status 0 mean the committed texts are the ones the
record above describes. A mismatch means the text was edited — restore it from
the origin URL and update this file and `SHA256SUMS` deliberately, in the same
commit.

Nothing here is legal advice. It is a record of what was read and where it
came from, so the notice's claims can be checked rather than trusted.
