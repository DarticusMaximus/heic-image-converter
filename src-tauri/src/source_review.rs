//! Completeness and drift harness for the documents that record the HEIC
//! dependency: `THIRD-PARTY-NOTICES.md` at the repository root and the license
//! texts under `licenses/`.
//!
//! A source review is a reading activity; no test can judge whether the prose is
//! right. What these tests can do is catch the two ways a notice rots: a
//! component silently dropping out of it, and a committed license text drifting
//! from the hash recorded beside it. Both read committed files only, resolve the
//! repository root from `CARGO_MANIFEST_DIR`, and never touch the network.

use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The repository root, one level above `src-tauri`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a parent directory")
        .to_path_buf()
}

/// Reads a repo-relative file, panicking with the resolved path on failure so a
/// missing document is never reported as a mystery.
fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("could not read {}: {err}", path.display()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    hex
}

// ---------------------------------------------------------------------------
// S6: the notice names every bundled component and license identifier
// ---------------------------------------------------------------------------

#[test]
fn notice_names_every_bundled_component() {
    let notice = read_repo_file("THIRD-PARTY-NOTICES.md");

    let expected = [
        "libheif",
        "libde265",
        "x265",
        "heif-rs",
        "LGPL-3.0-or-later",
        "GPL-2.0-or-later",
        "Apache-2.0",
        "GPL-3.0-or-later",
    ];

    for needle in expected {
        assert!(
            notice.contains(needle),
            "THIRD-PARTY-NOTICES.md must name `{needle}`"
        );
    }
}

// ---------------------------------------------------------------------------
// S7: the committed license texts match the hashes recorded beside them
// ---------------------------------------------------------------------------

#[test]
fn license_texts_match_their_recorded_hashes() {
    let root = repo_root();
    let sums_path = root.join("licenses/SHA256SUMS");
    let sums = std::fs::read_to_string(&sums_path)
        .unwrap_or_else(|err| panic!("could not read {}: {err}", sums_path.display()));

    let mut checked = 0;
    for (index, raw) in sums.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let expected = fields.next().expect("a sha256sum line starts with a hash");
        let relative = fields.next().unwrap_or_else(|| {
            panic!(
                "{}:{}: expected `<hash>  <path>`, got {line:?}",
                sums_path.display(),
                index + 1
            )
        });
        assert!(
            fields.next().is_none(),
            "{}:{}: expected `<hash>  <path>`, got {line:?}",
            sums_path.display(),
            index + 1
        );
        assert!(
            expected.len() == 64 && expected.bytes().all(|b| b.is_ascii_hexdigit()),
            "{}:{}: not a SHA-256 digest: {expected:?}",
            sums_path.display(),
            index + 1
        );

        let path = root.join(relative);
        let bytes = std::fs::read(&path).unwrap_or_else(|err| {
            panic!(
                "{}:{}: {relative} is recorded in SHA256SUMS but cannot be read: {err}",
                sums_path.display(),
                index + 1
            )
        });
        let actual = sha256_hex(&bytes);
        assert_eq!(
            actual,
            expected.to_ascii_lowercase(),
            "{} does not match its recorded hash in {}",
            relative,
            sums_path.display()
        );
        checked += 1;
    }

    let markers = [
        ("licenses/Apache-2.0.txt", "Apache License", "Apache-2.0"),
        (
            "licenses/GPL-2.0.txt",
            "GNU GENERAL PUBLIC LICENSE",
            "GPL-2.0",
        ),
        (
            "licenses/LGPL-3.0.txt",
            "GNU LESSER GENERAL PUBLIC LICENSE",
            "LGPL-3.0",
        ),
    ];

    for (relative, marker, identifier) in markers {
        let text = read_repo_file(relative);
        assert!(
            text.lines().any(|line| line.trim() == marker),
            "{relative} is missing its `{marker}` marker line, so it is not the {identifier} text"
        );
        assert!(sums.contains(relative), "SHA256SUMS must record {relative}");
    }

    assert!(
        checked >= markers.len(),
        "SHA256SUMS covers only {checked} file(s)"
    );
    println!("license texts checked: {checked}");
}
