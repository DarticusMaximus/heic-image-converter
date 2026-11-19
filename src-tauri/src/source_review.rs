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

// ---------------------------------------------------------------------------
// S8: the decision records carry their dated correction
// ---------------------------------------------------------------------------

#[test]
fn decision_records_carry_the_update() {
    let decisions = read_repo_file("docs/DECISIONS.md");

    let marker = "Superseded by Update 2026-11-18";
    let markers = decisions.matches(marker).count();
    assert_eq!(
        markers, 2,
        "docs/DECISIONS.md must mark D2 and D3 with `> **{marker}.**`; found {markers} marker(s)"
    );

    let expected = [
        "vendor/heif-rs",
        "manifest.json",
        "scripts/ci.sh offline",
        "THIRD-PARTY-NOTICES.md",
        "LGPL-3.0-or-later",
    ];

    for needle in expected {
        assert!(
            decisions.contains(needle),
            "docs/DECISIONS.md must record `{needle}` in the update paragraphs"
        );
    }

    println!("superseded decisions marked: {markers}");
}

// ---------------------------------------------------------------------------
// S9: the review and the notice are reachable from the indexes
// ---------------------------------------------------------------------------

#[test]
fn the_review_is_linked_from_the_index() {
    let index = read_repo_file("docs/README.md");
    assert!(
        index.contains("HEIC-SOURCE-REVIEW.md"),
        "docs/README.md must link docs/HEIC-SOURCE-REVIEW.md"
    );

    let readme = read_repo_file("README.md");
    assert!(
        readme.contains("THIRD-PARTY-NOTICES.md"),
        "README.md must link THIRD-PARTY-NOTICES.md"
    );
}

// ---------------------------------------------------------------------------
// S10: the modified-build record names every library the archive renames
// ---------------------------------------------------------------------------

/// The libraries the archive's symbol renaming modifies. All three carry the
/// `heifrs_` prefix on their non-API symbols; libheif keeps its public `heif_*`
/// exports, which is why it reads as unmodified unless it is named explicitly.
const MODIFIED_LIBRARIES: [&str; 3] = ["libheif", "libde265", "x265"];

/// A level-two section's body, with the heading line dropped so a heading such
/// as `## 4. Modified codecs` cannot satisfy a check on the prose alone.
fn level_two_body<'a>(document: &'a str, heading: &str) -> &'a str {
    let (_, after) = document
        .split_once(heading)
        .unwrap_or_else(|| panic!("the document has a `{heading}` section"));
    let body = after.split_once('\n').map_or(after, |(_, rest)| rest);
    body.split_once("\n## ")
        .map_or(body, |(section, _)| section)
}

/// The sentences of a markdown section with line wraps collapsed, so a check on
/// a phrase does not depend on where the file happens to wrap.
fn section_sentences(section: &str) -> Vec<String> {
    let flattened = section
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");
    flattened
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .split(". ")
        .map(str::to_owned)
        .collect()
}

/// Every sentence that states a library is a symbol-renamed or modified build —
/// the exact claim a redistributor acts on.
fn modified_build_sentences(section: &str) -> Vec<String> {
    section_sentences(section)
        .into_iter()
        .filter(|sentence| {
            sentence.contains("modified build")
                || sentence.contains("modified version")
                || sentence.contains("symbol-renamed")
        })
        .collect()
}

fn assert_names_every_modified_library(label: &str, text: &str) {
    for library in MODIFIED_LIBRARIES {
        assert!(
            text.contains(library),
            "{label} must name `{library}` among the libraries the archive renames"
        );
    }
}

#[test]
fn modified_build_record_names_every_renamed_library() {
    let notice = read_repo_file("THIRD-PARTY-NOTICES.md");
    let review = read_repo_file("docs/HEIC-SOURCE-REVIEW.md");

    for (label, heading) in [
        (
            "THIRD-PARTY-NOTICES.md §3",
            "## 3. If you redistribute a build",
        ),
        ("THIRD-PARTY-NOTICES.md §4", "## 4. Modified codecs"),
    ] {
        let sentences = modified_build_sentences(level_two_body(&notice, heading));
        assert!(
            !sentences.is_empty(),
            "{label} must state that the linked libraries are modified builds"
        );
        for sentence in &sentences {
            assert_names_every_modified_library(label, sentence);
        }
    }

    let f02 = finding_rows(findings_section(&review))
        .into_iter()
        .find(|row| row.id == "F02")
        .expect("the review's §11 has an F02 row");
    assert_names_every_modified_library("the review's F02 row", &f02.line);

    let exit = level_two_body(&review, "## 12. Exit criteria");
    let renamed = exit
        .lines()
        .find(|line| line.starts_with('|') && line.contains("§12.2"))
        .expect("the review's §12 has a §12.2 obligation row");
    assert_names_every_modified_library("the review's §12.2 row", renamed);

    let decisions = read_repo_file("docs/DECISIONS.md");
    let sentences = modified_build_sentences(&decisions);
    assert!(
        !sentences.is_empty(),
        "docs/DECISIONS.md must record the linked libraries as symbol-renamed modified builds"
    );
    for sentence in &sentences {
        assert_names_every_modified_library("docs/DECISIONS.md", sentence);
    }
}

// ---------------------------------------------------------------------------
// S11: the redistributor checklist carries the whole of LGPL-3.0 §4
// ---------------------------------------------------------------------------

/// LGPL-3.0 §4 (`licenses/LGPL-3.0.txt`) is one opening condition plus (a)-(d),
/// not the (d)(0) relink duty alone. (b) needs the GNU GPL text, so x265's
/// `licenses/GPL-2.0.txt` cannot stand in for the root GPL-3.0 `LICENSE`, and
/// (c) applies only where the work displays copyright notices during execution.
#[test]
fn redistributor_checklist_covers_lgpl_section_4() {
    let notice = read_repo_file("THIRD-PARTY-NOTICES.md");
    let section = level_two_body(&notice, "## 3. If you redistribute a build");
    let sentences = section_sentences(section);

    let sentence_with = |clause: &str, needles: &[&str]| {
        let found = sentences
            .iter()
            .any(|sentence| needles.iter().all(|needle| sentence.contains(needle)));
        assert!(
            found,
            "THIRD-PARTY-NOTICES.md §3 must state LGPL-3.0 {clause}; \
             no sentence contains all of {needles:?}"
        );
    };

    sentence_with(
        "§4's opening condition",
        &["do not restrict modification", "reverse engineering"],
    );
    sentence_with("§4(a)", &["prominent notice", "covered by"]);
    sentence_with("§4(b), both texts", &["GNU GPL", "licenses/LGPL-3.0.txt"]);
    sentence_with("§4(c)'s condition", &["If", "displays copyright notices"]);
    sentence_with("§4(d)(0) as the relink duty", &["§4(d)(0)", "relink"]);

    // §4(b)'s copies travel from the repository root: the root `LICENSE` is the
    // GPL-3.0 text and is required in addition to the texts under `licenses/`.
    sentence_with(
        "§4(b), the root GPL-3.0 text",
        &["LICENSE", "GPL-3.0", "in addition to"],
    );
}
