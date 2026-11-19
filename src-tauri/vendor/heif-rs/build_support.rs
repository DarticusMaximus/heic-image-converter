//! Pure helpers shared by the vendored `heif-rs` build script and the manifest
//! test suite in `src-tauri/src/codec_artifacts.rs`.
//!
//! This file is compiled twice: by `src-tauri/vendor/heif-rs/build.rs`, and by
//! the test suite through `#[path = "../vendor/heif-rs/build_support.rs"]`. It
//! therefore must stay free of build-script side effects — no `cargo:`
//! directives, no network, no filesystem access, no extraction, no panics on
//! untrusted input. Everything outside `std` it needs is `serde_json` (parsing)
//! and `sha2` (hashing), which are build dependencies of the vendored crate and
//! dev-dependencies of `src-tauri`.
//!
//! `parse_manifest` reads the JSON field by field instead of deriving
//! `Deserialize`, because the vendored crate's build dependencies carry
//! `serde_json` but not `serde`'s derive feature, and this file has to compile
//! in both places. Field names are checked explicitly, so a misspelled or
//! missing key fails loudly rather than defaulting to empty.

use serde_json::{Map as JsonMap, Value as Json};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// The manifest root, as committed at `vendor/heif-bundles/manifest.json`.
///
/// `"crate"` is a Rust keyword, so the JSON key is read into [`Manifest::package`].
#[derive(Debug, Clone)]
pub struct Manifest {
    /// Schema version; `1` is the only one this code understands.
    pub version: u32,
    /// The `binaries-heif` release the archives come from.
    pub release: String,
    /// The JSON `crate` object: the wrapper the archives are built for.
    pub package: Package,
    /// URL template with `{release}` and `{archive}` holes.
    pub url_template: String,
    /// One entry per supported target, in commit order.
    pub artifacts: Vec<Artifact>,
}

/// The JSON `crate` object.
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
}

/// One pinned codec archive and the library inside it.
#[derive(Debug, Clone)]
pub struct Artifact {
    /// File name of the release asset, e.g. `bundle_linux_x64.zip`.
    pub archive: String,
    /// `CARGO_CFG_TARGET_OS` value: `linux`, `macos`, or `windows`.
    pub os: String,
    /// `CARGO_CFG_TARGET_ENV` value, or `any` when the archive is env-agnostic.
    pub env: String,
    /// `CARGO_CFG_TARGET_ARCH` value: `x86_64` or `aarch64`.
    pub arch: String,
    /// Whether the archive's bytes are committed next to the manifest.
    pub committed: bool,
    /// Byte length of the archive, as recorded by hand.
    pub bytes: u64,
    /// Lowercase hex SHA-256 of the archive, as recorded by hand.
    pub sha256: String,
    /// Path of the static library inside the archive, e.g. `lib/libheif.a`.
    pub lib: String,
    /// Lowercase hex SHA-256 of the extracted library, as recorded by hand.
    pub lib_sha256: String,
}

/// Parses the committed manifest.
///
/// Errors name the offending field; every key the build relies on is required.
pub fn parse_manifest(json: &str) -> Result<Manifest, String> {
    let root: Json = serde_json::from_str(json)
        .map_err(|err| format!("heif-rs: manifest is not valid JSON: {err}"))?;
    let root = json_object(&root, "manifest")?;
    let package = json_object(get(root, "crate", "manifest")?, "manifest.crate")?;
    let artifacts = get(root, "artifacts", "manifest")?
        .as_array()
        .ok_or_else(|| "heif-rs: manifest.artifacts must be a JSON array".to_owned())?;

    Ok(Manifest {
        version: number_u32(root, "version", "manifest")?,
        release: text(root, "release", "manifest")?,
        package: Package {
            name: text(package, "name", "manifest.crate")?,
            version: text(package, "version", "manifest.crate")?,
        },
        url_template: text(root, "url_template", "manifest")?,
        artifacts: artifacts
            .iter()
            .enumerate()
            .map(|(index, value)| parse_artifact(value, index))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

/// Picks the archive for a target.
///
/// An entry matches when its `os` and `arch` match exactly and its `env` is
/// either `any` or equal to `env` — the `windows`/`msvc` and `windows`/`gnu`
/// entries are distinct archives, so the target env is part of the key.
///
/// # Errors
///
/// When no entry matches, the error names the requested target and every
/// target the manifest does know, so a new target is a readable failure:
///
/// ```text
/// heif-rs: no pinned codec artifact for target linux/aarch64/x86_64
///   known targets: linux/any/x86_64, linux/any/aarch64, macos/any/x86_64, ...
/// ```
pub fn select<'a>(
    m: &'a Manifest,
    os: &str,
    env: &str,
    arch: &str,
) -> Result<&'a Artifact, String> {
    m.artifacts
        .iter()
        .find(|a| a.os == os && a.arch == arch && (a.env == "any" || a.env == env))
        .ok_or_else(|| {
            let known = m
                .artifacts
                .iter()
                .map(|a| format!("{}/{}/{}", a.os, a.env, a.arch))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "heif-rs: no pinned codec artifact for target {os}/{env}/{arch}\n  \
                 known targets: {known}"
            )
        })
}

/// Renders the download URL for an artifact from the manifest's `url_template`.
pub fn archive_url(m: &Manifest, a: &Artifact) -> String {
    m.url_template
        .replace("{release}", &m.release)
        .replace("{archive}", &a.archive)
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Compares the hash of downloaded bytes against the artifact's recorded hash.
///
/// # Errors
///
/// The error names the archive, the expected hash, the actual hash, and the
/// command that re-fetches the archive:
///
/// ```text
/// heif-rs: bundle_linux_x64.zip failed its checksum
///   expected sha256 b5cbed5d…
///   actual   sha256 00000000…
///   fix: scripts/heif-artifacts.sh fetch bundle_linux_x64.zip
/// ```
pub fn verify_checksum(a: &Artifact, actual_hex: &str) -> Result<(), String> {
    if a.sha256.eq_ignore_ascii_case(actual_hex) {
        return Ok(());
    }

    Err(format!(
        "heif-rs: {} failed its checksum\n  expected sha256 {}\n  actual   sha256 {}\n  \
         fix: scripts/heif-artifacts.sh fetch {}",
        a.archive, a.sha256, actual_hex, a.archive
    ))
}

// ---------------------------------------------------------------------------
// JSON plumbing
// ---------------------------------------------------------------------------

/// One manifest artifact, read from `value` at `manifest.artifacts[index]`.
fn parse_artifact(value: &Json, index: usize) -> Result<Artifact, String> {
    let what = format!("manifest.artifacts[{index}]");
    let map = json_object(value, &what)?;

    Ok(Artifact {
        archive: text(map, "archive", &what)?,
        os: text(map, "os", &what)?,
        env: text(map, "env", &what)?,
        arch: text(map, "arch", &what)?,
        committed: flag(map, "committed", &what)?,
        bytes: number_u64(map, "bytes", &what)?,
        sha256: text(map, "sha256", &what)?,
        lib: text(map, "lib", &what)?,
        lib_sha256: text(map, "lib_sha256", &what)?,
    })
}

fn json_object<'a>(value: &'a Json, what: &str) -> Result<&'a JsonMap<String, Json>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("heif-rs: {what} must be a JSON object"))
}

fn get<'a>(map: &'a JsonMap<String, Json>, key: &str, what: &str) -> Result<&'a Json, String> {
    map.get(key)
        .ok_or_else(|| format!("heif-rs: {what} is missing `{key}`"))
}

fn text(map: &JsonMap<String, Json>, key: &str, what: &str) -> Result<String, String> {
    get(map, key, what)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("heif-rs: {what}.{key} must be a string"))
}

fn number_u32(map: &JsonMap<String, Json>, key: &str, what: &str) -> Result<u32, String> {
    let value = get(map, key, what)?
        .as_u64()
        .ok_or_else(|| format!("heif-rs: {what}.{key} must be a non-negative integer"))?;
    u32::try_from(value).map_err(|_| format!("heif-rs: {what}.{key} does not fit in a u32"))
}

fn number_u64(map: &JsonMap<String, Json>, key: &str, what: &str) -> Result<u64, String> {
    get(map, key, what)?
        .as_u64()
        .ok_or_else(|| format!("heif-rs: {what}.{key} must be a non-negative integer"))
}

fn flag(map: &JsonMap<String, Json>, key: &str, what: &str) -> Result<bool, String> {
    get(map, key, what)?
        .as_bool()
        .ok_or_else(|| format!("heif-rs: {what}.{key} must be true or false"))
}
