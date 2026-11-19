//! Build script for `heif-rs`.
//!
//! At build time this script obtains the prebuilt **bundle** libheif binaries for the current target, links them
//! statically into the crate, and generates the raw FFI bindings from the bundled `heif.h` header.
//!
//! Binaries come from: https://github.com/vegidio/binaries-heif/releases
//!
//! This is the vendored fork (see `VENDORED.md`). Upstream downloaded the archive for the current target with no
//! checksum; this script resolves the archive from the committed manifest at `../heif-bundles/manifest.json`, verifies
//! the archive's SHA-256 *before* extracting it, and verifies the extracted library's SHA-256 afterwards. A mismatch is
//! fatal, so a swapped or corrupted download fails the build instead of silently becoming the product.
//!
//! To build against a custom build of libheif (or fully offline), set the `HEIF_BINARIES_DIR` environment variable to a
//! directory that contains `include/` and `lib/` subdirectories laid out like the extracted `bundle_<os>_<arch>.zip`
//! archives. The override is used as-is and skips the manifest and its checksums entirely.

#[allow(dead_code)] // Shared with the test suite; the build script does not read every field.
mod build_support;

use build_support::{
    Artifact, Manifest, archive_url, parse_manifest, select, sha256_hex, verify_checksum,
};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The bundle archive ships a single static library (`libheif.a`, or `heif.lib` on MSVC) that merges libheif with x265
/// (HEVC encode) and libde265 (HEVC decode). Every non-API symbol is renamed with a `heifrs_` prefix, so it can't
/// collide with other static copies of those codecs (e.g. a static FFmpeg's x265) in the final program.
const STATIC_LIB: &str = "heif";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=HEIF_BINARIES_DIR");

    let binaries_dir = locate_binaries();
    let lib_dir = binaries_dir.join("lib");
    let include_dir = binaries_dir.join("include");

    emit_link_directives(&lib_dir);
    generate_bindings(&include_dir);
}

/// Returns the directory containing the `include/` and `lib/` subdirectories, either from the `HEIF_BINARIES_DIR`
/// override or from the pinned, checksum-verified bundle archive for the current target.
fn locate_binaries() -> PathBuf {
    if let Ok(dir) = env::var("HEIF_BINARIES_DIR") {
        let dir = PathBuf::from(dir);
        assert!(
            dir.join("include").is_dir() && dir.join("lib").is_dir(),
            "HEIF_BINARIES_DIR ({}) must contain `include/` and `lib/` subdirectories",
            dir.display()
        );
        return dir;
    }

    resolve_from_manifest()
}

/// The committed pin: `src-tauri/vendor/heif-bundles/`, holding `manifest.json` and the committed archives. This build
/// script lives in the vendored crate, so the bundle directory is its sibling.
fn bundle_dir() -> PathBuf {
    let crate_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    crate_dir.join("../heif-bundles")
}

/// Reads the pinned manifest, selects the archive for the current target, verifies it, extracts it into `OUT_DIR`, and
/// verifies the extracted library. Returns the directory holding the verified `include/` and `lib/` subdirectories.
fn resolve_from_manifest() -> PathBuf {
    let manifest_path = bundle_dir().join("manifest.json");
    println!("cargo:rerun-if-changed={}", manifest_path.display());

    let json = fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("heif-rs: cannot read {}: {e}", manifest_path.display()));
    let manifest = parse_manifest(&json).unwrap_or_else(|e| panic!("{e}"));

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let artifact =
        select(&manifest, &target_os, &target_env, &target_arch).unwrap_or_else(|e| panic!("{e}"));

    // The archive is hashed before a single byte of it is extracted.
    let bytes = load_archive(&manifest, artifact);
    if let Err(message) = verify_checksum(artifact, &sha256_hex(&bytes)) {
        panic!("{message}");
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    // The release number is part of the directory name, so a cache from another codec release is never reused.
    let cache_dir = out_dir.join(format!("binaries-heif-bundle-{}", manifest.release));

    // Idempotent: a fully extracted cache from a previous build is reused as-is, but only after re-checking the
    // library, so a poisoned or truncated cache directory fails here instead of being linked.
    if !(cache_dir.join("lib").is_dir() && cache_dir.join("include").is_dir()) {
        extract_zip(&bytes, &cache_dir);
    }

    verify_library(&cache_dir, artifact);
    cache_dir
}

/// Returns the archive bytes. A `committed` artifact is local-only: its zip is read from `../heif-bundles/`, and when
/// that file is absent the build fails naming the archive and the repair command — there is no fallback fetch. Only an
/// artifact the manifest marks `committed: false` may be downloaded from the pinned URL, and the caller verifies either
/// source's bytes before extraction.
fn load_archive(manifest: &Manifest, artifact: &Artifact) -> Vec<u8> {
    let local = bundle_dir().join(&artifact.archive);
    let source =
        build_support::resolve_source(artifact, local.is_file()).unwrap_or_else(|e| panic!("{e}"));

    match source {
        build_support::Source::Local => {
            println!("cargo:rerun-if-changed={}", local.display());
            fs::read(&local)
                .unwrap_or_else(|e| panic!("heif-rs: cannot read {}: {e}", local.display()))
        }
        build_support::Source::Fetch => {
            let url = archive_url(manifest, artifact);
            eprintln!("heif-rs: downloading {url}");
            download(&url)
        }
    }
}

/// Hashes the extracted static library and compares it against the artifact's recorded `lib_sha256`. This is the
/// second layer of the pin: the archive hash proves what was downloaded, and this proves what is about to be linked.
fn verify_library(cache_dir: &Path, artifact: &Artifact) {
    let lib_path = cache_dir.join(&artifact.lib);
    let bytes = fs::read(&lib_path)
        .unwrap_or_else(|e| panic!("heif-rs: cannot read {}: {e}", lib_path.display()));
    let actual = sha256_hex(&bytes);
    if !artifact.lib_sha256.eq_ignore_ascii_case(&actual) {
        panic!(
            "heif-rs: {} failed its checksum\n  expected sha256 {}\n  actual   sha256 {}\n  \
             fix: scripts/heif-artifacts.sh fetch {}\n  \
             then delete {} to force a clean extraction",
            artifact.lib,
            artifact.lib_sha256,
            actual,
            artifact.archive,
            cache_dir.display()
        );
    }
}

/// Downloads the given URL into memory, following redirects.
fn download(url: &str) -> Vec<u8> {
    let mut reader = ureq::get(url)
        .call()
        .unwrap_or_else(|e| panic!("heif-rs: failed to download {url}: {e}"))
        .into_body()
        .into_reader();

    let mut bytes = Vec::new();
    io::copy(&mut reader, &mut bytes)
        .unwrap_or_else(|e| panic!("heif-rs: failed to read response body from {url}: {e}"));
    bytes
}

/// Extracts a zip archive (held entirely in memory) into `dest`.
fn extract_zip(bytes: &[u8], dest: &Path) {
    let reader = io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("heif-rs: invalid zip archive");

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).expect("heif-rs: corrupt zip entry");
        let Some(rel_path) = entry.enclosed_name() else {
            continue; // skip unsafe / absolute paths
        };
        let out_path = dest.join(rel_path);

        if entry.is_dir() {
            fs::create_dir_all(&out_path).unwrap();
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut out_file = fs::File::create(&out_path)
            .unwrap_or_else(|e| panic!("heif-rs: cannot create {}: {e}", out_path.display()));
        io::copy(&mut entry, &mut out_file).expect("heif-rs: failed to extract file");
    }
}

/// Tells Cargo/rustc where the bundled static library lives and links it, plus the C++ runtime and system libraries
/// the codecs depend on.
fn emit_link_directives(lib_dir: &Path) {
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static={STATIC_LIB}");

    // libheif, x265, and libde265 are compiled C++, so the C++ runtime and a few system
    // libs must be linked.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    match target_os.as_str() {
        "macos" => {
            println!("cargo:rustc-link-lib=dylib=c++");
            // On x86_64, libde265's runtime CPU-feature detection (`__builtin_cpu_supports`)
            // references `__cpu_model` / `__cpu_indicator_init`, which live in clang's
            // compiler-rt builtins rather than libSystem. rustc links with `-nodefaultlibs`, so
            // they must be pulled in explicitly. (arm64 has no such CPUID path.)
            if target_arch == "x86_64" {
                link_compiler_rt_builtins();
            }
        }
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=m");
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=dl");
        }
        "windows" if target_env == "gnu" => {
            // The mingw archive is built with GCC/libstdc++; libde265 uses winpthreads mutexes and x265's thread pool
            // reads the Windows registry (advapi32).
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=advapi32");
        }
        "windows" => {
            // The release ships MSVC `.lib` archives, which link the C/C++ runtime
            // automatically via the objects' default-lib directives. x265's thread pool, however,
            // reads the Windows registry (`RegOpenKeyExA` etc.), so its home — advapi32 — must be
            // linked explicitly.
            println!("cargo:rustc-link-lib=dylib=advapi32");
        }
        _ => {}
    }
}

/// Links clang's compiler-rt builtins so the x86 CPU-feature symbols (`__cpu_model` /
/// `__cpu_indicator_init`) that libde265 references resolve. Locates the archive via
/// `clang -print-runtime-dir`; silently does nothing if clang can't be queried, in which case the
/// linker simply reports the missing symbols as before.
fn link_compiler_rt_builtins() {
    let cc = env::var("CC").unwrap_or_else(|_| "clang".into());
    let Ok(output) = std::process::Command::new(cc)
        .arg("-print-runtime-dir")
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if dir.is_empty() {
        return;
    }
    // `libclang_rt.osx.a` provides the builtins for every macOS arch.
    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}

/// Generates raw FFI bindings from `include/libheif/heif.h` into `OUT_DIR/bindings.rs`.
fn generate_bindings(include_dir: &Path) {
    let header = include_dir.join("libheif").join("heif.h");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let bindings = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .clang_arg(format!("-I{}", include_dir.display()))
        // Keep the output focused on the libheif surface.
        .allowlist_function("heif_.*")
        .allowlist_type("heif_.*")
        .allowlist_var("heif_.*")
        .allowlist_var("LIBHEIF.*")
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("heif-rs: failed to generate bindings from heif.h");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("heif-rs: failed to write bindings.rs");
}
