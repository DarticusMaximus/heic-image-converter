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
// S2: the unsafe register covers every unsafe site in the vendored crate
// ---------------------------------------------------------------------------

/// The `## 5. `unsafe` register` section, up to the next level-two heading.
/// Scoping matters here: §1 and §10 quote the same `src-tauri/vendor/...` paths,
/// so searching the whole document would pass on a document with no register.
fn unsafe_register_section(review: &str) -> &str {
    let (_, after) = review
        .split_once("## 5. `unsafe` register")
        .expect("the review has a `## 5. `unsafe` register` section");
    after
        .split_once("\n## ")
        .map_or(after, |(section, _)| section)
}

/// The second cell of every `U` row, trimmed. Register rows are recognised by
/// their shape — a line beginning with `| U` — so the prose around the table
/// cannot satisfy the check.
fn unsafe_row_sites(section: &str) -> Vec<String> {
    section
        .lines()
        .filter_map(|line| {
            let mut cells = line.split('|');
            cells.next()?;
            let id = cells.next()?.trim();
            if !(id.len() == 3
                && id.starts_with('U')
                && id[1..].bytes().all(|b| b.is_ascii_digit()))
            {
                return None;
            }
            Some(cells.next()?.trim().to_owned())
        })
        .collect()
}

/// Every `src-tauri/vendor/heif-rs/src/<file>:<line>` whose line contains
/// `unsafe`, in register order: file by file, ascending line. Comment-only
/// mentions (`sys.rs:1`) are excluded, matching the `grep` in the review.
fn unsafe_sites() -> Vec<String> {
    let vendor = repo_root().join("src-tauri/vendor/heif-rs/src");
    let mut sources: Vec<PathBuf> = std::fs::read_dir(&vendor)
        .unwrap_or_else(|err| panic!("read {}: {err}", vendor.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect();
    sources.sort();

    let mut sites = Vec::new();
    for path in sources {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        let name = path.file_name().expect("a source file has a name");
        for (index, line) in text.lines().enumerate() {
            if line.contains("unsafe") && !line.trim_start().starts_with("//") {
                sites.push(format!(
                    "src-tauri/vendor/heif-rs/src/{}:{}",
                    name.to_string_lossy(),
                    index + 1
                ));
            }
        }
    }
    sites
}

#[test]
fn review_covers_every_unsafe_site() {
    let review = read_repo_file("docs/HEIC-SOURCE-REVIEW.md");
    let rows = unsafe_row_sites(unsafe_register_section(&review));
    let sites = unsafe_sites();

    for site in &sites {
        assert!(
            rows.iter().any(|row| row == site),
            "docs/HEIC-SOURCE-REVIEW.md §5 has no U row for the unsafe site at {site}"
        );
    }
    for row in &rows {
        assert!(
            sites.iter().any(|site| site == row),
            "docs/HEIC-SOURCE-REVIEW.md §5 has a U row for {row}, which is not an unsafe site in the vendored tree"
        );
    }

    assert_eq!(
        rows.len(),
        20,
        "the vendored crate has 20 unsafe sites; §5 holds {} U rows",
        rows.len()
    );
    assert_eq!(
        sites.len(),
        rows.len(),
        "§5 should hold exactly one U row per unsafe site"
    );

    println!("unsafe sites covered: {}", sites.len());
}

// ---------------------------------------------------------------------------
// S3: every seeded question has an answer
// ---------------------------------------------------------------------------

/// The `## 10. Seeded questions` section, up to the next level-two heading.
/// Scoping keeps a question id quoted elsewhere in the review from counting as
/// an answer row.
fn seeded_questions_section(review: &str) -> &str {
    let (_, after) = review
        .split_once("## 10. Seeded questions")
        .expect("the review has a `## 10. Seeded questions` section");
    after
        .split_once("\n## ")
        .map_or(after, |(section, _)| section)
}

fn is_question_id(id: &str) -> bool {
    id.len() == 3 && id.starts_with('Q') && id[1..].bytes().all(|b| b.is_ascii_digit())
}

/// The first cell of every `Q` row, trimmed, in document order. A row is
/// recognised by its shape — a line beginning with `| Q` and a two-digit id —
/// so prose that cites a question id cannot satisfy the check.
fn seeded_question_ids(section: &str) -> Vec<String> {
    section
        .lines()
        .filter_map(|line| {
            let mut cells = line.split('|');
            cells.next()?;
            let id = cells.next()?.trim();
            is_question_id(id).then(|| id.to_owned())
        })
        .collect()
}

#[test]
fn every_seeded_question_is_answered() {
    let review = read_repo_file("docs/HEIC-SOURCE-REVIEW.md");
    let ids = seeded_question_ids(seeded_questions_section(&review));
    let expected: Vec<String> = (1..=17).map(|number| format!("Q{number:02}")).collect();

    for id in &expected {
        assert!(
            ids.contains(id),
            "docs/HEIC-SOURCE-REVIEW.md §10 has no row answering {id}"
        );
    }
    assert_eq!(
        ids.len(),
        expected.len(),
        "§10 should hold exactly one row per seeded question, Q01–Q17"
    );

    println!("seeded questions covered: {}", ids.len());
}

// ---------------------------------------------------------------------------
// S4: every finding carries exactly one disposition
// ---------------------------------------------------------------------------

/// The `## 11. Findings` section, up to the next level-two heading.
fn findings_section(review: &str) -> &str {
    let (_, after) = review
        .split_once("## 11. Findings")
        .expect("the review has a `## 11. Findings` section");
    after
        .split_once("\n## ")
        .map_or(after, |(section, _)| section)
}

fn is_finding_id(id: &str) -> bool {
    id.len() == 3 && id.starts_with('F') && id[1..].bytes().all(|b| b.is_ascii_digit())
}

/// One `F` row: its id, the parsed fourth cell (the disposition), and the full
/// line for checks that need the gate or owner named elsewhere in the row.
struct FindingRow {
    id: String,
    disposition: String,
    line: String,
}

/// Every `F` row, with the disposition read from its own cell rather than the
/// row text. A finding whose prose happens to contain `watch` cannot satisfy
/// the check, and a row that grows or loses a column fails loudly instead of
/// shifting the disposition under a substring search.
fn finding_rows(section: &str) -> Vec<FindingRow> {
    section
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').collect();
            if cells.len() < 6 {
                return None;
            }
            let id = cells[1].trim();
            if !is_finding_id(id) {
                return None;
            }
            Some(FindingRow {
                id: id.to_owned(),
                disposition: cells[4].trim().to_owned(),
                line: line.to_owned(),
            })
        })
        .collect()
}

const DISPOSITIONS: [&str; 4] = ["fixed here", "accepted risk", "watch", "open obligation"];

#[test]
fn every_finding_has_a_disposition() {
    let review = read_repo_file("docs/HEIC-SOURCE-REVIEW.md");
    let rows = finding_rows(findings_section(&review));

    assert!(
        rows.len() >= 8,
        "§11 records {} finding(s); the review requires at least eight",
        rows.len()
    );

    let mut blocking = 0;
    for row in &rows {
        assert!(
            DISPOSITIONS.contains(&row.disposition.as_str()),
            "{} has disposition {:?}, which is not one of {:?}",
            row.id,
            row.disposition,
            DISPOSITIONS
        );
        if row.disposition == "open obligation" && row.line.contains("stage 04") {
            blocking += 1;
        }
    }

    assert!(
        blocking >= 3,
        "at least three findings must pair `open obligation` with `stage 04`; found {blocking}"
    );

    println!("findings with disposition: {}", rows.len());
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
