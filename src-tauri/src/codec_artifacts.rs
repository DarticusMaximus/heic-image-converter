//! Test-only integrity suite for the pinned codec artifact manifest.
//!
//! `vendor/heif-bundles/manifest.json` is the pin: the archive names, byte
//! lengths, and SHA-256 digests in it are hand-authored from the feature spec
//! and are the only trusted input for the build. Nothing in the tree writes
//! them, so these tests read the committed files and fail when the tree and the
//! pin disagree — which a build script cannot detect for an archive it never
//! downloads.
//!
//! The helpers under test are the exact file the vendored `heif-rs` build
//! script compiles (`vendor/heif-rs/build_support.rs`), pulled in here by
//! `#[path]` so the selection and verification logic is unit-tested rather than
//! trusted. Every path is resolved from `CARGO_MANIFEST_DIR` so the suite does
//! not care about the working directory.

#[path = "../vendor/heif-rs/build_support.rs"]
mod build_support;

use build_support::{
    Manifest, Source, archive_url, parse_manifest, resolve_source, select, sha256_hex,
    verify_checksum,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Every `(os, env, arch)` key the product ships or CI builds. `env` is part of
/// the key because the two Windows toolchains link different archives.
const EXPECTED_TARGETS: [(&str, &str, &str); 8] = [
    ("linux", "any", "x86_64"),
    ("linux", "any", "aarch64"),
    ("macos", "any", "x86_64"),
    ("macos", "any", "aarch64"),
    ("windows", "msvc", "x86_64"),
    ("windows", "msvc", "aarch64"),
    ("windows", "gnu", "x86_64"),
    ("windows", "gnu", "aarch64"),
];

/// The four archives whose bytes are committed. The other four are hash-pinned
/// and fetched on demand.
const COMMITTED_ARCHIVES: [&str; 4] = [
    "bundle_linux_x64.zip",
    "bundle_osx_arm64.zip",
    "bundle_osx_x64.zip",
    "bundle_windows_x64.zip",
];

/// The `binaries-heif` release the manifest pins.
const EXPECTED_RELEASE: &str = "26.10.1";

/// The wrapper crate version the requirement, the vendored copy, and the
/// manifest's `crate.version` must all agree on.
const EXPECTED_CRATE_VERSION: &str = "26.10.3";

/// Directory holding the manifest and any committed archives.
fn bundle_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/heif-bundles")
}

fn manifest_path() -> PathBuf {
    bundle_dir().join("manifest.json")
}

/// Directory holding the vendored `heif-rs` source the build actually compiles.
fn vendored_crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/heif-rs")
}

/// Reads a manifest's `[package]` version. Dependency tables carry their own
/// `version` keys, so only the `[package]` table is scanned.
fn package_version(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = line.strip_prefix("version")
            && let Some(value) = rest.trim_start().strip_prefix('=')
        {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Loads and parses the committed manifest, panicking with the path on failure
/// so a broken pin is never reported as a mystery.
fn load_manifest() -> Manifest {
    let path = manifest_path();
    let json = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("could not read {}: {err}", path.display()));
    parse_manifest(&json).unwrap_or_else(|err| panic!("{} does not parse: {err}", path.display()))
}

/// Reads an archive's bytes, or explains which file is missing.
fn read_archive(archive: &str) -> Vec<u8> {
    let path = bundle_dir().join(archive);
    std::fs::read(&path).unwrap_or_else(|err| panic!("could not read {}: {err}", path.display()))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// 1. Schema
// ---------------------------------------------------------------------------

#[test]
fn manifest_schema_is_intact() {
    let manifest = load_manifest();

    assert_eq!(manifest.version, 1, "manifest schema version");
    assert!(!manifest.release.trim().is_empty(), "release must be set");
    assert_eq!(manifest.package.name, "heif-rs", "crate.name");
    assert!(
        !manifest.package.version.trim().is_empty(),
        "crate.version must be set"
    );
    assert!(
        !manifest.artifacts.is_empty(),
        "manifest must record at least one artifact"
    );

    assert!(
        manifest.url_template.contains("{release}"),
        "url_template must template the release: {}",
        manifest.url_template
    );
    assert!(
        manifest.url_template.contains("{archive}"),
        "url_template must template the archive: {}",
        manifest.url_template
    );
}

// ---------------------------------------------------------------------------
// 2. Coverage
// ---------------------------------------------------------------------------

#[test]
fn manifest_covers_every_supported_target() {
    let manifest = load_manifest();

    let recorded: BTreeSet<(&str, &str, &str)> = manifest
        .artifacts
        .iter()
        .map(|a| (a.os.as_str(), a.env.as_str(), a.arch.as_str()))
        .collect();
    let expected: BTreeSet<(&str, &str, &str)> = EXPECTED_TARGETS.into_iter().collect();

    assert_eq!(
        manifest.artifacts.len(),
        EXPECTED_TARGETS.len(),
        "manifest must record exactly {} artifacts, duplicate keys are not allowed",
        EXPECTED_TARGETS.len()
    );
    assert_eq!(recorded, expected, "recorded (os, env, arch) keys");

    let archives: BTreeSet<&str> = manifest
        .artifacts
        .iter()
        .map(|a| a.archive.as_str())
        .collect();
    assert_eq!(
        archives.len(),
        manifest.artifacts.len(),
        "every artifact needs its own archive name"
    );
    assert!(
        archives.iter().all(|archive| !archive.is_empty()),
        "archive names must not be empty"
    );

    for artifact in &manifest.artifacts {
        let expected_lib = if artifact.os == "windows" && artifact.env == "msvc" {
            "lib/heif.lib"
        } else {
            "lib/libheif.a"
        };
        assert_eq!(
            artifact.lib, expected_lib,
            "{} library path inside the archive",
            artifact.archive
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Committed bytes
// ---------------------------------------------------------------------------

#[test]
fn committed_artifacts_match_their_recorded_bytes() {
    let manifest = load_manifest();

    let committed: BTreeSet<&str> = manifest
        .artifacts
        .iter()
        .filter(|a| a.committed)
        .map(|a| a.archive.as_str())
        .collect();
    assert_eq!(
        committed,
        COMMITTED_ARCHIVES.into_iter().collect::<BTreeSet<_>>(),
        "the shipping targets are the committed ones"
    );

    for artifact in manifest.artifacts.iter().filter(|a| a.committed) {
        let bytes = read_archive(&artifact.archive);
        assert_eq!(
            bytes.len() as u64,
            artifact.bytes,
            "{} byte length",
            artifact.archive
        );
        assert_eq!(
            sha256_hex(&bytes),
            artifact.sha256,
            "{} sha256",
            artifact.archive
        );
        assert!(
            verify_checksum(artifact, &sha256_hex(&bytes)).is_ok(),
            "{} must pass its own recorded checksum",
            artifact.archive
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Uncommitted entries
// ---------------------------------------------------------------------------

#[test]
fn uncommitted_artifacts_are_recorded_and_absent() {
    let manifest = load_manifest();

    let uncommitted: Vec<_> = manifest.artifacts.iter().filter(|a| !a.committed).collect();
    assert_eq!(uncommitted.len(), 4, "four archives are fetched on demand");

    for artifact in uncommitted {
        assert!(
            artifact.bytes > 0,
            "{} must record a byte length",
            artifact.archive
        );
        assert!(
            is_sha256_hex(&artifact.sha256),
            "{} must record a full sha256, got {:?}",
            artifact.archive,
            artifact.sha256
        );
        assert!(
            is_sha256_hex(&artifact.lib_sha256),
            "{} must record a full lib_sha256, got {:?}",
            artifact.archive,
            artifact.lib_sha256
        );

        let path = bundle_dir().join(&artifact.archive);
        assert!(
            !path.exists(),
            "{} is fetched on demand and must not sit in the tree ({})",
            artifact.archive,
            path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// 5. URLs
// ---------------------------------------------------------------------------

#[test]
fn artifact_urls_derive_from_the_pinned_release() {
    let manifest = load_manifest();

    for artifact in &manifest.artifacts {
        let url = archive_url(&manifest, artifact);
        let expected = format!(
            "https://github.com/vegidio/binaries-heif/releases/download/{EXPECTED_RELEASE}/{}",
            artifact.archive
        );
        assert_eq!(url, expected, "{} download url", artifact.archive);
        assert!(
            !url.contains('{'),
            "{} url still has a template hole: {url}",
            artifact.archive
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Selection
// ---------------------------------------------------------------------------

#[test]
fn select_picks_the_manifest_entry() {
    let manifest = load_manifest();

    let cases = [
        ("linux", "any", "x86_64", "bundle_linux_x64.zip"),
        ("windows", "msvc", "x86_64", "bundle_windows_x64.zip"),
        ("windows", "gnu", "x86_64", "bundle_mingw_x64.zip"),
        ("macos", "any", "aarch64", "bundle_osx_arm64.zip"),
    ];
    for (os, env, arch, expected) in cases {
        let artifact = select(&manifest, os, env, arch)
            .unwrap_or_else(|err| panic!("{os}/{env}/{arch} should select: {err}"));
        assert_eq!(artifact.archive, expected, "{os}/{env}/{arch}");
    }

    let err =
        select(&manifest, "freebsd", "any", "x86_64").expect_err("freebsd is not a pinned target");
    assert!(
        err.contains("no pinned codec artifact for target freebsd/any/x86_64"),
        "error must name the target, got: {err}"
    );
    assert!(
        err.contains("known targets:"),
        "error must list the known targets, got: {err}"
    );
    for (os, env, arch) in EXPECTED_TARGETS {
        assert!(
            err.contains(&format!("{os}/{env}/{arch}")),
            "error must name {os}/{env}/{arch}, got: {err}"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Checksums
// ---------------------------------------------------------------------------

#[test]
fn checksum_mismatch_reports_both_hashes() {
    let manifest = load_manifest();
    let artifact = select(&manifest, "linux", "any", "x86_64")
        .expect("linux/x86_64 must be pinned to an artifact");

    let wrong = "0".repeat(64);
    let err = verify_checksum(artifact, &wrong).expect_err("a wrong hash must fail");
    assert!(
        err.contains(&artifact.archive),
        "error must name the archive, got: {err}"
    );
    assert!(
        err.contains(&artifact.sha256),
        "error must name the expected hash, got: {err}"
    );
    assert!(
        err.contains(&wrong),
        "error must name the actual hash, got: {err}"
    );
    assert!(
        err.contains("fix: scripts/heif-artifacts.sh fetch bundle_linux_x64.zip"),
        "error must name the fix command, got: {err}"
    );

    let actual = sha256_hex(&read_archive(&artifact.archive));
    verify_checksum(artifact, &actual).expect("the committed bytes must match the pin");
}

// ---------------------------------------------------------------------------
// 8. Pins
// ---------------------------------------------------------------------------

#[test]
fn the_crate_pin_matches_the_vendored_copy() {
    let crate_manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let crate_manifest = std::fs::read_to_string(&crate_manifest_path)
        .unwrap_or_else(|err| panic!("could not read {}: {err}", crate_manifest_path.display()));

    let requirement = format!("heif-rs = \"={EXPECTED_CRATE_VERSION}\"");
    assert!(
        crate_manifest.contains(&requirement),
        "{} must pin the wrapper exactly: expected `{requirement}`, so no other version can resolve",
        crate_manifest_path.display()
    );
    assert!(
        crate_manifest.contains("path = \"vendor/heif-rs\""),
        "{} must point [patch.crates-io] at the vendored copy: expected `path = \"vendor/heif-rs\"`",
        crate_manifest_path.display()
    );

    let vendored_manifest_path = vendored_crate_dir().join("Cargo.toml");
    let vendored_manifest = std::fs::read_to_string(&vendored_manifest_path)
        .unwrap_or_else(|err| panic!("could not read {}: {err}", vendored_manifest_path.display()));
    let vendored_version = package_version(&vendored_manifest).unwrap_or_else(|| {
        panic!(
            "{} declares no [package] version",
            vendored_manifest_path.display()
        )
    });
    assert_eq!(
        vendored_version,
        EXPECTED_CRATE_VERSION,
        "{} version, which the patched build compiles",
        vendored_manifest_path.display()
    );

    let manifest = load_manifest();
    assert_eq!(
        manifest.package.version,
        EXPECTED_CRATE_VERSION,
        "{} crate.version",
        manifest_path().display()
    );
}

// ---------------------------------------------------------------------------
// 9. Archive family
// ---------------------------------------------------------------------------

#[test]
fn the_manifest_names_the_bundle_family() {
    let manifest = load_manifest();

    assert_eq!(manifest.release, EXPECTED_RELEASE, "pinned release");
    for artifact in &manifest.artifacts {
        assert!(
            artifact.archive.starts_with("bundle_"),
            "{} must be a bundle archive; static_/dynamic_ archives are not the \
             linked library",
            artifact.archive
        );
        assert!(
            artifact.archive.ends_with(".zip"),
            "{} must be a zip archive",
            artifact.archive
        );
    }
}

// ---------------------------------------------------------------------------
// 10. Committed artifacts are local-only
// ---------------------------------------------------------------------------

#[test]
fn a_committed_artifact_is_never_fetched() {
    let manifest = load_manifest();

    let committed: Vec<_> = manifest.artifacts.iter().filter(|a| a.committed).collect();
    assert_eq!(
        committed
            .iter()
            .map(|a| a.archive.as_str())
            .collect::<BTreeSet<_>>(),
        COMMITTED_ARCHIVES.into_iter().collect::<BTreeSet<_>>(),
        "the committed set must be exactly the archives the product ships, so this \
         test cannot pass on an empty or shrunken set"
    );

    for artifact in committed {
        let err = resolve_source(artifact, false)
            .expect_err("a missing committed artifact must never resolve to a fetch");
        assert!(
            err.contains(&artifact.archive),
            "error must name the archive, got: {err}"
        );
        assert!(
            err.contains(&format!(
                "src-tauri/vendor/heif-bundles/{}",
                artifact.archive
            )),
            "error must name the path the archive belongs at, got: {err}"
        );
        assert!(
            err.contains(&format!(
                "scripts/heif-artifacts.sh fetch {}",
                artifact.archive
            )),
            "error must name the repair command, got: {err}"
        );
        assert!(
            !err.contains("http"),
            "a missing committed artifact must not suggest a download, got: {err}"
        );
        assert_eq!(
            resolve_source(artifact, true),
            Ok(Source::Local),
            "{} present on disk must be read locally",
            artifact.archive
        );
    }

    let linux = manifest
        .artifacts
        .iter()
        .find(|a| a.archive == "bundle_linux_x64.zip")
        .expect("the linux x64 archive must be pinned");
    assert_eq!(
        resolve_source(linux, false).expect_err("bundle_linux_x64.zip must not be fetched"),
        concat!(
            "heif-rs: committed codec artifact bundle_linux_x64.zip is missing\n",
            "  expected at src-tauri/vendor/heif-bundles/bundle_linux_x64.zip\n",
            "  fix: scripts/heif-artifacts.sh fetch bundle_linux_x64.zip"
        ),
        "the missing-artifact message is part of the contract"
    );
}

// ---------------------------------------------------------------------------
// 11. Uncommitted artifacts
// ---------------------------------------------------------------------------

#[test]
fn an_uncommitted_artifact_may_be_fetched() {
    let manifest = load_manifest();

    let uncommitted: Vec<_> = manifest.artifacts.iter().filter(|a| !a.committed).collect();
    assert_eq!(
        uncommitted.len(),
        4,
        "the four non-shipping archives are the only ones that may be fetched"
    );
    assert!(
        uncommitted
            .iter()
            .all(|a| !COMMITTED_ARCHIVES.contains(&a.archive.as_str())),
        "an archive cannot be both committed and fetchable"
    );

    for artifact in uncommitted {
        assert_eq!(
            resolve_source(artifact, false),
            Ok(Source::Fetch),
            "{} is not committed, so a missing copy may be downloaded",
            artifact.archive
        );
        assert_eq!(
            resolve_source(artifact, true),
            Ok(Source::Local),
            "{} present on disk must be read locally rather than downloaded again",
            artifact.archive
        );
    }
}
