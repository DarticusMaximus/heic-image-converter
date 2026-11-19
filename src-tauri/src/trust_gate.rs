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
//! `.github/workflows/release.yml` is the one workflow that can publish, and a
//! tag can be pushed at any commit, so its trust step and its `--locked` build
//! are pinned here too — with the same text-reading approach, scoped to its
//! `build` job.
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
        .unwrap_or_else(|| panic!("the workflow has no `{job}` job (expected a line `{header}`)"));
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

/// The lines of one shell function's body, from its `<name>() {` header to the
/// first line that is exactly `}`. The `offline` gate's refusal checks, child
/// command, and transcript lines all live in that body; searching the whole file
/// would also match the usage text, the comments above it, and the other arms.
fn shell_function_lines<'a>(script: &'a str, name: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = script.lines().collect();
    let header = format!("{name}() {{");
    let start = lines
        .iter()
        .position(|line| line.trim() == header)
        .unwrap_or_else(|| panic!("scripts/ci.sh defines no `{name}()` function"));
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim() == "}")
        .map_or(lines.len(), |offset| start + 1 + offset);
    lines[start..end].to_vec()
}

/// The lines of `usage()`'s body, so the help text is checked where it is
/// written rather than anywhere `trust` might appear in the file.
fn usage_lines(script: &str) -> Vec<&str> {
    shell_function_lines(script, "usage")
}

// ---------------------------------------------------------------------------
// C2: the gate scripts stay runnable on the bash that ships with macOS
// ---------------------------------------------------------------------------

/// Names expanded as `"${name[@]}"` on one line, in the order they appear.
fn array_expansion_names(line: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("\"${") {
        let after = &rest[start + 3..];
        if let Some(end) = after.find("[@]}") {
            let name = &after[..end];
            let starts_well = name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
            if starts_well && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                names.push(name);
            }
        }
        rest = &rest[start + 3..];
    }
    names
}

#[test]
fn scripts_stay_within_bash_3_2_syntax() {
    // macOS ships bash 3.2 as `/bin/bash`, and the trust gate is meant to run on
    // a maintainer's Mac, so both scripts are held to 3.2 syntax. Two constructs
    // beyond 3.2 are banned:
    //   - `${var,,}` is bash 4.0's lowercase expansion. On 3.2 it is a fatal
    //     "bad substitution" partway through the run (this gate's predecessor
    //     printed no summary at all and still exited 0, so the breakage was
    //     silent). Lowercase with the `lower` helper instead.
    //   - `"${arr[@]}"` on an *empty* array under `set -u` aborts 3.2 with
    //     "unbound variable"; bash 4.4 made it expand to nothing instead.
    // The exact rules enforced here:
    //   1. neither script contains `,,}`;
    //   2. `scripts/heif-artifacts.sh` contains no `[@]` at all — it walks its
    //      arrays by index, where an empty array is not an error;
    //   3. in `scripts/ci.sh`, every line that expands `"${name[@]}"` is preceded
    //      in the file by a `${#name[@]}` length check for that same name, so the
    //      expansion can never see an empty array.
    let scripts = [
        (
            "scripts/heif-artifacts.sh",
            read_repo_file("scripts/heif-artifacts.sh"),
        ),
        ("scripts/ci.sh", read_repo_file("scripts/ci.sh")),
    ];

    for (path, script) in &scripts {
        assert!(
            !script.contains(",,"),
            "{path} must not carry bash-4 `${{var,,}}` case folding; stock macOS /bin/bash is 3.2, where it is a fatal `bad substitution`. Use the `lower` helper (printf | tr)"
        );
    }

    let artifacts = &scripts[0].1;
    assert!(
        !artifacts.contains("[@]"),
        "scripts/heif-artifacts.sh must not expand `\"${{arr[@]}}\"`; it walks arrays by index so an empty array cannot abort bash 3.2 under `set -u`"
    );

    let ci = &scripts[1].1;
    let lines: Vec<&str> = ci.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        for name in array_expansion_names(line) {
            let guard = format!("${{#{name}[@]}}");
            assert!(
                lines[..index]
                    .iter()
                    .any(|earlier| earlier.contains(&guard)),
                "scripts/ci.sh:{} expands `\"${{{name}[@]}}\"` with no `{guard}` length check earlier in the file; on bash 3.2 under `set -u` an empty array would abort the gate with `unbound variable`",
                index + 1
            );
        }
    }
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

// ---------------------------------------------------------------------------
// X1: the offline gate refuses the overrides and names what it proved
// ---------------------------------------------------------------------------

#[test]
fn the_offline_gate_refuses_environment_overrides_and_names_what_it_proves() {
    let script = read_repo_file("scripts/ci.sh");

    let dispatch = ci_case_arm(&script, "offline");
    assert!(
        dispatch
            .iter()
            .any(|line| !is_commented_out(line) && line.trim() == "offline"),
        "the `offline)` case arm must call the `offline` function; a commented-out call is not a call"
    );

    let gate = shell_function_lines(&script, "offline");
    let index_of = |needle: &str| {
        gate.iter()
            .position(|line| !is_commented_out(line) && line.contains(needle))
            .unwrap_or_else(|| {
                panic!(
                    "scripts/ci.sh's offline() must contain `{needle}` on a line that runs; a mention in a comment or in another function proves nothing"
                )
            })
    };

    // Set-ness is what is refused, so each check is a `-n` test on the variable
    // itself. A bare mention would pass any looser check.
    let heif_refusal = index_of("[[ -n \"${HEIF_BINARIES_DIR-}\" ]]");
    let target_refusal = index_of("[[ -n \"${CARGO_TARGET_DIR-}\" ]]");

    // A refusal printed after the clone would still have cloned and measured
    // free space, so both checks must sit above the disk check and the clone.
    let disk_check = index_of("df -Pk");
    let clone = index_of("git clone --quiet --no-hardlinks");
    assert!(
        heif_refusal < disk_check
            && target_refusal < disk_check
            && heif_refusal < clone
            && target_refusal < clone,
        "the offline gate must refuse a set HEIF_BINARIES_DIR or CARGO_TARGET_DIR before it measures free space and clones; a refusal printed after the work leaves a throwaway tree and a misleading transcript behind"
    );

    // The refusal must stop the run. A named message without an exit is a
    // warning, and the gate would clone and certify anyway.
    assert!(
        (target_refusal..disk_check)
            .any(|line| !is_commented_out(gate[line]) && gate[line].trim() == "exit 1"),
        "the offline gate's override refusal must `exit 1` before the disk check; a warning that does not stop the run still certifies the build it warned about"
    );

    // The child is scrubbed as well: a variable exported between the refusal
    // check and the child must still not reach the build.
    let child = index_of(
        "env -u HEIF_BINARIES_DIR -u CARGO_TARGET_DIR CARGO_NET_OFFLINE=true ./scripts/ci.sh rust",
    );

    // And the transcript states which codec bytes and which target directory a
    // run proved, before the build runs. A PASS without those lines cannot be
    // audited from the log alone.
    let codec_line = index_of("offline gate: codec source:");
    let target_line = index_of("offline gate: target:");
    assert!(
        codec_line < child && target_line < child,
        "the offline gate must print the codec source and target directory before the child build; otherwise the PASS line describes work whose inputs the transcript never named"
    );
    assert!(
        gate[codec_line].contains(
            "/src-tauri/vendor/heif-bundles/manifest.json (committed archives; HEIF_BINARIES_DIR unset)"
        ),
        "the offline gate's codec-source line must name the committed manifest and say HEIF_BINARIES_DIR is unset; found `{}`",
        gate[codec_line]
    );
    assert!(
        gate[target_line].contains("/src-tauri/target (inside the clone; CARGO_TARGET_DIR unset)"),
        "the offline gate's target line must name the clone's src-tauri/target and say CARGO_TARGET_DIR is unset; found `{}`",
        gate[target_line]
    );
}

// ---------------------------------------------------------------------------
// X2: the publishing workflow runs the trust gate and builds `--locked`
// ---------------------------------------------------------------------------

/// The lines of one step — a `      - ` list item — that contains `index`.
/// Assertions about "this step's shell" or "this action's input" would otherwise
/// be satisfied by the same key in a neighbouring step.
fn step_lines<'a>(lines: &[&'a str], index: usize) -> Vec<&'a str> {
    let starts_step = |line: &str| line.starts_with("      - ");
    let start = lines[..=index]
        .iter()
        .rposition(|line| starts_step(line))
        .unwrap_or_else(|| {
            panic!("no step item (a line starting `      - `) at or above index {index}")
        });
    let end = lines[index + 1..]
        .iter()
        .position(|line| starts_step(line))
        .map_or(lines.len(), |offset| index + 1 + offset);
    lines[start..end].to_vec()
}

/// True for a step item: six spaces of indentation then `- `. A `- name:` or
/// `- uses:` nested deeper is a mapping key inside some other structure (a
/// `with:` input, for instance) and is never executed as a step.
fn is_step_item(line: &str) -> bool {
    line.starts_with("      - ")
}

/// True for a step property line: exactly eight spaces of indentation and a
/// non-space character next. A `run:` nested under `with:` is deeper than the
/// step's own properties, so it is not the thing that runs.
fn is_step_property(line: &str) -> bool {
    line.strip_prefix("        ")
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with(' '))
}

/// True for a key under a step's `with:` mapping: exactly ten spaces of
/// indentation and a non-space character next. Action inputs live at this depth,
/// so `args:`/`releaseDraft:` read here is the value the action receives, while
/// the same text inside a `run:` block is not.
fn is_action_input(line: &str) -> bool {
    line.strip_prefix("          ")
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with(' '))
}

/// True for a `uses:` line that is the action of a step: the inline
/// `- uses: …` step item, or a `uses:` step property directly under a `- `
/// item (the `- name: …` form). A `uses:`-shaped string anywhere else — inside
/// a `run:` value, in a comment, as a `with:` input — is not a pin, so
/// `action_version` runs every candidate through this before reading a version
/// out of it.
fn is_step_uses(lines: &[&str], index: usize) -> bool {
    let line = lines[index];
    if line.starts_with("      - uses: ") {
        return true;
    }
    is_step_property(line)
        && line.trim_start().starts_with("uses: ")
        && index > 0
        && !is_commented_out(lines[index - 1])
        && is_step_item(lines[index - 1])
}

/// The single version suffix of every step-action `uses: <action>@<version>`
/// line in a workflow (see `is_step_uses` for what counts as one). Two
/// different versions for the same action in one file is the drift this check
/// exists to catch, so that is a panic, not a pick.
fn action_version(workflow: &str, action: &str) -> String {
    let needle = format!("uses: {action}@");
    let lines: Vec<&str> = workflow.lines().collect();
    let versions: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(index, line)| is_step_uses(&lines, *index) && !is_commented_out(line))
        .filter_map(|(_, line)| line.split(&needle).nth(1))
        .map(str::trim)
        .collect();
    assert!(
        !versions.is_empty(),
        "no workflow step carries `uses: {action}@<version>`; the action is either missing or unpinned"
    );
    let first = versions[0];
    assert!(
        versions.iter().all(|version| *version == first),
        "`{action}` is pinned at more than one version in one workflow: {versions:?}"
    );
    first.to_string()
}

/// The index of the `      - name: <name>` step item in a job's lines. The name
/// is the anchor on purpose: it is what a reviewer reads in the Actions UI, and
/// a step that runs the gate under some other name, or a step whose name looks
/// right but executes something else, must not satisfy the checks below.
///
/// Indentation is part of the anchor. A `- name: Codec trust gate` nested at
/// ten or twelve spaces is a key in another step's mapping, not a step, and is
/// named as such rather than reported as a missing step.
fn named_step(lines: &[&str], name: &str) -> usize {
    let header = format!("- name: {name}");
    let items: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            !is_commented_out(line) && is_step_item(line) && line.trim_start() == header
        })
        .map(|(index, _)| index)
        .collect();
    let misplaced: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            !is_commented_out(line) && !is_step_item(line) && line.trim_start() == header
        })
        .map(|(index, _)| index)
        .collect();
    assert!(
        misplaced.is_empty(),
        "release.yml:{} writes `{header}` at {} spaces of indentation, not as a step item (six spaces then `- `); a step nested inside another step's mapping is never executed, so the job has no `{name}` step",
        misplaced.first().map_or(0, |index| index + 1),
        misplaced.first().map_or(0, |index| lines[*index].len()
            - lines[*index].trim_start().len())
    );
    assert_eq!(
        items.len(),
        1,
        "release.yml's `build` job must have exactly one `{header}` step item; found {}",
        items.len()
    );
    items[0]
}

/// The index of the step item that runs exactly `command`, in either the inline
/// `- run: <command>` form or the `- name: …` + `run: <command>` form. Exact,
/// because `run: echo npm ci` and `run: false && npm ci` both contain the text
/// and execute something else, and indentation-anchored for the same reason as
/// `named_step`, because a nested `- run:` is not a step.
fn command_step(lines: &[&str], command: &str) -> Option<usize> {
    let inline = format!("      - run: {command}");
    let keyed = format!("run: {command}");
    lines.iter().enumerate().find_map(|(index, line)| {
        if is_commented_out(line) {
            return None;
        }
        let inline_match = *line == inline;
        let keyed_match = index > 0
            && is_step_property(line)
            && line.trim_start() == keyed
            && !is_commented_out(lines[index - 1])
            && is_step_item(lines[index - 1]);
        (inline_match || keyed_match).then_some(index)
    })
}

#[test]
fn release_runs_the_trust_gate_and_builds_locked() {
    let release = read_repo_file(".github/workflows/release.yml");
    let ci = read_repo_file(".github/workflows/ci.yml");

    // A duplicate `build:` job header is rejected rather than "first one wins":
    // GitHub either refuses the file or runs an ungated copy, and a text reader
    // that silently took the first block would certify the wrong job.
    let build_headers = release
        .lines()
        .filter(|line| !is_commented_out(line) && line.trim_end() == "  build:")
        .count();
    assert_eq!(
        build_headers, 1,
        "release.yml must declare exactly one `  build:` job header; found {build_headers}"
    );
    let build = job_lines(&release, "build");

    // Likewise one `steps:` key: a second one is a duplicate YAML key, so the
    // effective step list is not the one read below.
    let steps_keys = build
        .iter()
        .filter(|line| !is_commented_out(line) && line.trim() == "steps:")
        .count();
    assert_eq!(
        steps_keys, 1,
        "release.yml's `build` job must declare exactly one `steps:` key; found {steps_keys}"
    );

    // Structural anchors, not command substrings: the checkout step is a step
    // item carrying the action, and the trust and publish steps are found by
    // name. A `run: echo "./scripts/ci.sh trust"` line is not a step and cannot
    // stand in for one.
    let checkout = build
        .iter()
        .enumerate()
        .find(|(index, line)| {
            !is_commented_out(line)
                && is_step_uses(&build, *index)
                && line.contains("uses: actions/checkout@")
        })
        .map(|(index, _)| index)
        .unwrap_or_else(|| {
            panic!(
                "the `build` job in release.yml has no checkout step (a step item `- uses: actions/checkout@<version>`)"
            )
        });
    let trust = named_step(&build, "Codec trust gate");
    let install = command_step(&build, "npm ci").unwrap_or_else(|| {
        panic!("the `build` job in release.yml has no step running exactly `npm ci`")
    });
    let publish = named_step(&build, "Build and publish");

    assert!(
        checkout < trust,
        "release.yml runs the trust gate before checkout; the gate must read the tagged tree"
    );
    assert!(
        trust < install,
        "release.yml runs `npm ci` before the `Codec trust gate` step; the gate must come first so a poisoned tree fails before the install"
    );
    assert!(
        trust < publish,
        "release.yml reaches the `Build and publish` step before the `Codec trust gate` step; the publishing path must be gated"
    );

    let trust_step = step_lines(&build, trust);

    // The executed command, exactly, and at the step's own property
    // indentation. `run: echo ./scripts/ci.sh trust` and
    // `run: false && ./scripts/ci.sh trust` both contain the command text and
    // both execute no gate, so a substring match is not evidence. A `run:`
    // nested deeper than eight spaces belongs to a `with:` mapping and never
    // runs; it is not counted. No trailing shell comment is tolerated either:
    // the value must be the command and nothing else.
    let trust_runs: Vec<&str> = trust_step
        .iter()
        .filter(|line| !is_commented_out(line) && is_step_property(line))
        .map(|line| line.trim())
        .filter(|line| line.starts_with("run:"))
        .collect();
    assert_eq!(
        trust_runs,
        vec!["run: ./scripts/ci.sh trust"],
        "the `Codec trust gate` step must contain exactly one `run:` property whose value is exactly `./scripts/ci.sh trust` (no wrappers, no trailing tokens, no trailing comment, eight-space property indentation); found {trust_runs:?}"
    );

    // A conditional gate is a disabled gate. `if: false` skips the step and the
    // tag still publishes; the same is true of any other condition on a check
    // whose whole purpose is to run on every tag.
    assert!(
        !trust_step.iter().any(|line| {
            !is_commented_out(line)
                && is_step_property(line)
                && line.trim_start().starts_with("if:")
        }),
        "the `Codec trust gate` step must carry no `if:` key at all; a conditional gate is a disabled gate — `if: false` would skip the check and the tag would still publish"
    );
    // `continue-on-error: true` lets a failed trust check report green and the
    // publish step run anyway, which is the same hole as no gate.
    assert!(
        !trust_step.iter().any(|line| {
            !is_commented_out(line)
                && is_step_property(line)
                && line.trim_start().starts_with("continue-on-error:")
        }),
        "the `Codec trust gate` step must carry no `continue-on-error:` key; a gate allowed to fail would let the release publish after the trust check failed"
    );

    // The Windows matrix leg defaults to pwsh, and `scripts/ci.sh` is a bash
    // script that needs awk, sha256sum and python3. `shell: bash` on the step is
    // what makes Git Bash serve that leg; without it the job dies before it can
    // publish anything.
    assert!(
        trust_step.iter().any(|line| !is_commented_out(line)
            && is_step_property(line)
            && line.trim() == "shell: bash"),
        "the release trust step must carry `shell: bash`; the windows-latest leg defaults to pwsh and cannot run scripts/ci.sh trust"
    );

    // `--locked` reaches cargo through the Tauri CLI: `tauri build -- --locked`
    // forwards everything after `--` to cargo, and tauri-action consumes the
    // matrix `args` verbatim. So every matrix leg's quoted args must end in
    // `-- --locked`, and the action must still be fed `matrix.args`.
    let publish_step = step_lines(&build, publish);
    assert!(
        publish_step.iter().any(|line| {
            !is_commented_out(line)
                && is_step_property(line)
                && line
                    .trim_start()
                    .starts_with("uses: tauri-apps/tauri-action@")
        }),
        "the `Build and publish` step must actually use `tauri-apps/tauri-action@<version>` at its own property indentation; a correctly-named step that runs no action publishes nothing"
    );

    // The publish step must be unconditional. A trust gate only means something
    // if a failed gate stops the release, and `if: always()` or `if: failure()`
    // on the publish step runs it after the gate failed, creating a draft from
    // the drifted tree. `if: success()` is redundant with the default, so the
    // rule is strict: no `if:` at all.
    assert!(
        !publish_step.iter().any(|line| {
            !is_commented_out(line)
                && is_step_property(line)
                && line.trim_start().starts_with("if:")
        }),
        "the `Build and publish` step must carry no `if:` key; `if: always()` or `if: failure()` would publish after the trust gate failed, and `if: success()` is redundant with the default — the publish must be unconditional"
    );
    assert!(
        !publish_step.iter().any(|line| {
            !is_commented_out(line)
                && is_step_property(line)
                && line.trim_start().starts_with("continue-on-error:")
        }),
        "the `Build and publish` step must carry no `continue-on-error:` key; a publish allowed to fail would let the job report green with no release created, which the release path must never do"
    );

    // `--locked` reaches cargo through the Tauri CLI: `tauri build -- --locked`
    // forwards everything after `--` to cargo, and tauri-action consumes the
    // matrix `args` verbatim. So every matrix leg's quoted args must end in
    // `-- --locked`, and the action must still be fed `matrix.args`.
    assert!(
        publish_step.iter().any(|line| {
            !is_commented_out(line)
                && is_action_input(line)
                && line.trim() == "args: ${{ matrix.args }}"
        }),
        "the tauri-action step must pass `args: ${{{{ matrix.args }}}}`; otherwise the matrix leg values never reach the build"
    );

    let leg_args: Vec<&str> = build
        .iter()
        .filter(|line| !is_commented_out(line))
        .filter_map(|line| {
            line.trim()
                .strip_prefix("args: \"")
                .and_then(|value| value.strip_suffix('"'))
        })
        .collect();
    assert_eq!(
        leg_args.len(),
        4,
        "release.yml must declare one quoted `args: \"…\"` per matrix leg (four platforms); found {leg_args:?}"
    );
    for value in &leg_args {
        assert!(
            value.ends_with("-- --locked"),
            "matrix leg args `{value}` must end with `-- --locked`; without `--locked` the release re-resolves Cargo.lock and can publish a drifted HEIC stack"
        );
    }

    assert!(
        build
            .iter()
            .any(|line| !is_commented_out(line) && line.trim() == "contents: write"),
        "the release `build` job must keep `permissions: contents: write`; a publishing path without it is not the release workflow"
    );
    assert!(
        publish_step.iter().any(|line| {
            !is_commented_out(line) && is_action_input(line) && line.trim() == "releaseDraft: true"
        }),
        "the tauri-action step must keep publishing a draft release (`releaseDraft: true`)"
    );

    for action in ["actions/checkout", "actions/setup-node"] {
        let release_version = action_version(&release, action);
        let ci_version = action_version(&ci, action);
        assert_eq!(
            release_version, ci_version,
            "release.yml pins {action}@{release_version} but ci.yml pins @{ci_version}; a stale major in the publishing path is the same drift class as a missing gate"
        );
    }

    println!("release arms checked: {} matrix legs", leg_args.len());
}
