//! Completeness and drift harness for the documents that record the HEIC
//! dependency: `docs/HEIC-SOURCE-REVIEW.md`, `THIRD-PARTY-NOTICES.md` at the
//! repository root, and the license texts under `licenses/`.
//!
//! A source review is a reading activity; no test can judge whether the prose is
//! right. What these tests can do is catch the ways the record rots: a reviewed
//! file dropping out of the read log, a component silently dropping out of the
//! notice, and a committed license text drifting from the hash recorded beside
//! it. Each reads committed files only, resolves the repository root from
//! `CARGO_MANIFEST_DIR`, and never touches the network.

use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
// S1: the read log covers every file under review
// ---------------------------------------------------------------------------

/// Every file the review is obliged to have read: the Rust sources of the
/// vendored crate, its `VENDORED.md`, and the seam that consumes it.
fn reviewed_files() -> Vec<PathBuf> {
    let root = repo_root();
    let vendor = root.join("src-tauri/vendor/heif-rs");

    let mut files: Vec<PathBuf> = WalkDir::new(&vendor)
        .into_iter()
        .map(|entry| entry.expect("walk the vendored crate"))
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect();

    files.push(vendor.join("VENDORED.md"));
    files.push(root.join("src-tauri/src/heic.rs"));
    files.sort();
    files
}

/// The `## 2. Read log` section, up to the next level-two heading. Scoping the
/// scan to the section keeps a path named in another table — §3 names the same
/// four fork files — from satisfying a read-log check.
fn read_log_section(review: &str) -> &str {
    let (_, after) = review
        .split_once("## 2. Read log")
        .expect("the review has a `## 2. Read log` section");
    after
        .split_once("\n## ")
        .map_or(after, |(section, _)| section)
}

/// The first cell of every read-log row, trimmed. A row is recognised by its
/// shape — the line starts with `|` and the first cell is a backticked path —
/// so prose that quotes the same paths cannot satisfy the check.
fn read_log_paths(section: &str) -> Vec<String> {
    section
        .lines()
        .filter_map(|line| {
            let mut cells = line.split('|');
            if !cells.next().is_some_and(|cell| cell.trim().is_empty()) {
                return None;
            }
            let first = cells.next()?.trim();
            (first.starts_with('`') && first.ends_with('`')).then(|| first.to_owned())
        })
        .collect()
}

#[test]
fn review_covers_every_vendored_source_file() {
    let review = read_repo_file("docs/HEIC-SOURCE-REVIEW.md");
    let rows = read_log_paths(read_log_section(&review));
    let files = reviewed_files();
    let root = repo_root();

    for path in &files {
        let relative = path
            .strip_prefix(&root)
            .expect("every reviewed file lives under the repository root")
            .to_string_lossy()
            .replace('\\', "/");
        let needle = format!("`{relative}`");
        assert!(
            rows.contains(&needle),
            "docs/HEIC-SOURCE-REVIEW.md has no read-log row for {needle}"
        );
    }

    assert_eq!(
        files.len(),
        17,
        "the review pins 17 files; the walk found {}",
        files.len()
    );
    assert_eq!(
        rows.len(),
        files.len(),
        "§2 should hold one read-log row per reviewed file"
    );

    println!("vendored files covered: {}", files.len());
}

// ---------------------------------------------------------------------------
// S5: the review records the versions and archives it is pinned to
// ---------------------------------------------------------------------------

#[test]
fn review_records_the_pinned_provenance() {
    let review = read_repo_file("docs/HEIC-SOURCE-REVIEW.md");

    let expected = [
        "26.10.3",
        "26.10.1",
        "1.23.6",
        "bundle_linux_x64.zip",
        "bundle_osx_x64.zip",
        "bundle_osx_arm64.zip",
        "bundle_windows_x64.zip",
    ];

    for needle in expected {
        assert!(
            review.contains(needle),
            "docs/HEIC-SOURCE-REVIEW.md must record `{needle}`"
        );
    }
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
