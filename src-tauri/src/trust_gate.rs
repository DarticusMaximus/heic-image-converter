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

/// The index of the first line in `lines` containing `needle`.
fn line_index(lines: &[&str], needle: &str) -> Option<usize> {
    lines.iter().position(|line| line.contains(needle))
}

#[test]
fn ci_runs_the_trust_gate_before_the_rust_and_build_gates() {
    let workflow = read_repo_file(".github/workflows/ci.yml");

    let trust_runs = workflow.matches("./scripts/ci.sh trust").count();
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
