//! Structural checks on the CI wiring for the HEIC trust gate.
//!
//! `.github/workflows/ci.yml` is the only thing that runs the trust gate on a
//! pull request, and a workflow that quietly loses its trust step or its
//! pull-request guard would leave the archives and the lock entry unchecked
//! while every other gate stayed green. There is no YAML parser here on
//! purpose: the workflow is a committed text file, so the tests read it and
//! assert on the shape of the job blocks a reviewer can see. The repository
//! root is resolved from `CARGO_MANIFEST_DIR`, the same way the other
//! committed-file suites do it.
//!
//! The upgrade runbook gets the same treatment. A procedure that quietly loses
//! a step, a file path, or the command that proves the result is a maintenance
//! trap nobody notices until a dependency bump, so `docs/UPGRADE-HEIC.md` and
//! the `scripts/ci.sh` dispatch that backs it are pinned here too.

use std::path::{Path, PathBuf};

/// The repository root, one level above `src-tauri`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must have a parent directory")
        .to_path_buf()
}

/// Reads a repo-relative file, panicking with the resolved path on failure so a
/// missing workflow is never reported as a mystery.
fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("could not read {}: {err}", path.display()))
}

/// A job key sits at two-space indentation (`  rust:`); a job's properties and
/// steps sit deeper (`    name: Rust`). So the first line after the header whose
/// indentation drops back to exactly two spaces starts the next job.
fn starts_a_new_job(line: &str) -> bool {
    line.starts_with("  ") && !line.starts_with("   ")
}

/// The lines of one `jobs:` entry, from its `  <job>:` header to the next job
/// header (or end of file). Scoping to the block is what lets the ordering
/// checks say "within the `rust` job" instead of "somewhere in the file".
fn job_lines<'a>(workflow: &'a str, job: &str) -> Vec<&'a str> {
    let header = format!("  {job}:");
    let lines: Vec<&str> = workflow.lines().collect();
    let start = lines
        .iter()
        .position(|line| *line == header)
        .unwrap_or_else(|| panic!("ci.yml has no `{job}` job (expected a line `{header}`)"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| starts_a_new_job(line))
        .map_or(lines.len(), |offset| start + 1 + offset);
    lines[start..end].to_vec()
}

/// The index of the first line in `lines` containing `needle`, ignoring lines
/// that are commented out. A commented-out call is not a call: without this, a
/// `# trust` line in the `all` arm would satisfy the ordering check.
fn line_index(lines: &[&str], needle: &str) -> Option<usize> {
    lines
        .iter()
        .position(|line| !is_commented_out(line) && line.contains(needle))
}

/// True for a line that carries no executable content. `#` is shell's and
/// YAML's comment marker; markdown headings use it too, so markdown assertions
/// go through `markdown_visible_lines` instead.
fn is_commented_out(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

#[test]
fn ci_runs_the_trust_gate_before_the_rust_and_build_gates() {
    let workflow = read_repo_file(".github/workflows/ci.yml");

    let trust_runs = workflow
        .lines()
        .filter(|line| !is_commented_out(line))
        .filter(|line| line.contains("./scripts/ci.sh trust"))
        .count();
    assert_eq!(
        trust_runs, 2,
        "ci.yml must run ./scripts/ci.sh trust exactly twice (the `rust` and `build` jobs); found {trust_runs}"
    );

    for (job, gate) in [
        ("rust", "./scripts/ci.sh rust"),
        ("build", "./scripts/ci.sh build"),
    ] {
        let lines = job_lines(&workflow, job);
        let trust = line_index(&lines, "./scripts/ci.sh trust")
            .unwrap_or_else(|| panic!("the `{job}` job has no step running ./scripts/ci.sh trust"));
        let gate_line = line_index(&lines, gate)
            .unwrap_or_else(|| panic!("the `{job}` job has no step running {gate}"));
        assert!(
            trust < gate_line,
            "the `{job}` job runs {gate} before ./scripts/ci.sh trust; the trust gate must come first"
        );
    }
}

#[test]
fn ci_runs_the_offline_gate_only_outside_pull_requests() {
    let workflow = read_repo_file(".github/workflows/ci.yml");

    let trigger = line_index(&workflow.lines().collect::<Vec<_>>(), "workflow_dispatch:")
        .unwrap_or_else(|| panic!("ci.yml must declare `workflow_dispatch:` as a trigger"));
    let jobs = workflow
        .lines()
        .collect::<Vec<_>>()
        .iter()
        .position(|line| *line == "jobs:")
        .expect("ci.yml has a `jobs:` block");
    assert!(
        trigger < jobs,
        "`workflow_dispatch:` must be a trigger under `on:`, not a line inside the jobs"
    );

    let lines = job_lines(&workflow, "offline");

    let guard = "if: github.event_name != 'pull_request'";
    assert!(
        lines.iter().any(|line| line.trim() == guard),
        "the `offline` job must carry the exact guard line `{guard}`"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.trim() == "timeout-minutes: 75"),
        "the `offline` job must carry `timeout-minutes: 75`"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("components: rustfmt")),
        "the `offline` job's toolchain must request `components: rustfmt` (the cloned tree runs `cargo fmt --check`)"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("./scripts/ci.sh offline")),
        "the `offline` job must run ./scripts/ci.sh offline"
    );
    assert!(
        lines.iter().any(|line| line.contains("unshare -rn")),
        "the `offline` job must probe `unshare -rn` before running the gate"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("kernel.apparmor_restrict_unprivileged_userns")),
        "the `offline` job must relax `kernel.apparmor_restrict_unprivileged_userns` before the probe"
    );

    let policy = line_index(&lines, "kernel.apparmor_restrict_unprivileged_userns");
    let probe = line_index(&lines, "unshare -rn");
    let gate = line_index(&lines, "./scripts/ci.sh offline");
    assert!(
        policy < probe && probe < gate,
        "the `offline` job must relax the sysctl, then probe, then run the gate, in that order"
    );
}

// ---------------------------------------------------------------------------
// T3: the upgrade runbook covers every required step
// ---------------------------------------------------------------------------

/// The literals the runbook must keep: the pins it moves, the files it edits,
/// and the commands that prove the result. A runbook that renames
/// `manifest.json` to "the manifest" is no longer executable by someone who has
/// never seen the pinning scheme.
const RUNBOOK_LITERALS: [&str; 14] = [
    "26.10.3",
    "26.10.1",
    "manifest.json",
    "VENDORED.md",
    "build_support.rs",
    "docs/HEIC-SOURCE-REVIEW.md",
    "THIRD-PARTY-NOTICES.md",
    "licenses/SHA256SUMS",
    "scripts/heif-artifacts.sh verify",
    "scripts/ci.sh trust",
    "scripts/ci.sh offline",
    "CVE",
    "rollback",
    "rehearsal (no version change)",
];

/// The lines of a markdown document outside `<!-- … -->` blocks. A literal that
/// only survives in an HTML comment is invisible to a reader, so the runbook
/// assertions run against these lines rather than the raw file. Headings are
/// kept: `#` is a heading in markdown, not a comment.
fn markdown_visible_lines(doc: &str) -> Vec<&str> {
    let mut visible = Vec::new();
    let mut in_comment = false;
    for line in doc.lines() {
        let mut rest = line;
        loop {
            if in_comment {
                match rest.find("-->") {
                    Some(end) => {
                        in_comment = false;
                        rest = &rest[end + 3..];
                    }
                    None => break,
                }
            } else {
                match rest.find("<!--") {
                    Some(start) => {
                        if !rest[..start].trim().is_empty() {
                            visible.push(&rest[..start]);
                        }
                        in_comment = true;
                        rest = &rest[start + 4..];
                    }
                    None => {
                        visible.push(rest);
                        break;
                    }
                }
            }
        }
    }
    visible
}

#[test]
fn upgrade_runbook_covers_every_required_step() {
    let runbook = read_repo_file("docs/UPGRADE-HEIC.md");
    let visible = markdown_visible_lines(&runbook);

    for needle in RUNBOOK_LITERALS {
        assert!(
            visible.iter().any(|line| line.contains(needle)),
            "docs/UPGRADE-HEIC.md must name `{needle}` on a line a reader sees; a bump that needs a file or command the runbook stopped naming cannot be executed from the document"
        );
    }

    for step in 0..=9 {
        let heading = format!("### S{step}");
        assert!(
            visible
                .iter()
                .any(|line| line.trim_start().starts_with(&heading)),
            "docs/UPGRADE-HEIC.md must carry a `{heading}` heading; the S0–S9 sequence is the procedure, and a missing step is a skipped check"
        );
    }

    println!("runbook literals covered: {}", RUNBOOK_LITERALS.len());
}

// ---------------------------------------------------------------------------
// T4: the runbook and the gate are reachable from the docs
// ---------------------------------------------------------------------------

#[test]
fn the_runbook_and_the_trust_gate_are_linked_from_the_docs() {
    let index = read_repo_file("docs/README.md");
    assert!(
        index.contains("UPGRADE-HEIC.md"),
        "docs/README.md must list docs/UPGRADE-HEIC.md; an unlinked runbook is one nobody finds at bump time"
    );

    let build = read_repo_file("docs/BUILD.md");
    assert!(
        build.contains("scripts/ci.sh trust"),
        "docs/BUILD.md must list `scripts/ci.sh trust` among the gates; otherwise the documented gate inventory is behind the script"
    );
}

// ---------------------------------------------------------------------------
// T5: the trust gate is part of the full local gate
// ---------------------------------------------------------------------------

/// One `case` arm of `scripts/ci.sh`: its `<name>)` header through the closing
/// `;;`. Scoping matters because `trust` also appears in the usage text and as
/// a function name, so a whole-file search would pass with the arm deleted.
fn ci_case_arm<'a>(script: &'a str, name: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = script.lines().collect();
    let case_start = lines
        .iter()
        .position(|line| line.trim() == "case \"${1-}\" in")
        .unwrap_or_else(|| panic!("scripts/ci.sh has no `case \"${{1-}}\" in` dispatch block"));
    let header = format!("{name})");
    let offset = lines[case_start..]
        .iter()
        .position(|line| line.trim() == header)
        .unwrap_or_else(|| panic!("scripts/ci.sh has no `{name})` case arm"));
    let start = case_start + offset;
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim() == ";;")
        .map_or(lines.len(), |offset| start + 1 + offset);
    lines[start..end].to_vec()
}

/// The lines of `usage()`'s body, so the help text is checked where it is
/// written rather than anywhere `trust` might appear in the file.
fn usage_lines(script: &str) -> Vec<&str> {
    let lines: Vec<&str> = script.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == "usage() {")
        .unwrap_or_else(|| panic!("scripts/ci.sh defines no usage() function"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim() == "}")
        .map_or(lines.len(), |offset| start + 1 + offset);
    lines[start..end].to_vec()
}

#[test]
fn the_trust_gate_is_part_of_the_full_local_gate() {
    let script = read_repo_file("scripts/ci.sh");

    let dispatch = ci_case_arm(&script, "trust");
    assert!(
        dispatch
            .iter()
            .any(|line| !is_commented_out(line) && line.trim() == "trust"),
        "the `trust)` case arm must call the `trust` function; a commented-out call is not a call"
    );

    let usage = usage_lines(&script);
    assert!(
        usage
            .iter()
            .any(|line| !is_commented_out(line) && line.trim_start().starts_with("trust ")),
        "scripts/ci.sh's usage text must list `trust` as a subcommand"
    );

    let all = ci_case_arm(&script, "all");
    let trust =
        line_index(&all, "trust").unwrap_or_else(|| panic!("the `all` arm must call trust"));
    let frontend =
        line_index(&all, "frontend").unwrap_or_else(|| panic!("the `all` arm must call frontend"));
    assert!(
        trust < frontend,
        "`all` must run trust before frontend; the archives and the pin are checked before anything that needs them"
    );
}
